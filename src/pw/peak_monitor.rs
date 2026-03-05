use std::collections::HashMap;
use std::convert::TryInto;
use std::mem;
use std::sync::mpsc;

use pipewire as pw;
use pw::spa;
use pw::spa::param::format::{MediaSubtype, MediaType};
use pw::spa::param::format_utils;
use pw::spa::pod::Pod;
use pw::stream::{Stream, StreamFlags, StreamListener};

use super::message::PwEvent;

struct MonitorData {
    node_id: u32,
    event_tx: mpsc::Sender<PwEvent>,
    format: spa::param::audio::AudioInfoRaw,
}

struct MonitoredStream {
    _listener: StreamListener<MonitorData>,
    _stream: Stream,
}

pub struct PeakMonitor {
    streams: HashMap<u32, MonitoredStream>,
    core: pw::core::Core,
    event_tx: mpsc::Sender<PwEvent>,
}

impl PeakMonitor {
    pub fn new(core: &pw::core::Core, event_tx: &mpsc::Sender<PwEvent>) -> Self {
        Self {
            streams: HashMap::new(),
            core: core.clone(),
            event_tx: event_tx.clone(),
        }
    }

    pub fn set_monitored_nodes(&mut self, nodes: &[(u32, String, bool)]) {
        let desired: std::collections::HashSet<u32> = nodes.iter().map(|(id, _, _)| *id).collect();
        let current: std::collections::HashSet<u32> = self.streams.keys().copied().collect();

        let to_remove: Vec<u32> = current.difference(&desired).copied().collect();

        for id in to_remove {
            self.streams.remove(&id);
            log::debug!("Peak monitor: removed stream for node {}", id);
        }

        for (id, node_name, capture_sink) in nodes {
            if self.streams.contains_key(id) {
                continue;
            }
            match self.create_stream(*id, node_name, *capture_sink) {
                Ok(monitored) => {
                    self.streams.insert(*id, monitored);
                    log::debug!("Peak monitor: created stream for node {} ({})", id, node_name);
                }
                Err(e) => {
                    log::warn!("Peak monitor: failed to create stream for node {} ({}): {}", id, node_name, e);
                }
            }
        }
    }

    fn create_stream(
        &self,
        node_id: u32,
        node_name: &str,
        capture_sink: bool,
    ) -> Result<MonitoredStream, pw::Error> {
        let mut props = pw::properties::properties! {
            *pw::keys::MEDIA_TYPE => "Audio",
            *pw::keys::MEDIA_CATEGORY => "Capture",
            *pw::keys::MEDIA_ROLE => "DSP",
            "target.object" => node_name,
            *pw::keys::NODE_VIRTUAL => "true",
            *pw::keys::NODE_NAME => format!("blabbergraph.peak.{}", node_id),
        };
        if capture_sink {
            // Sinks/duplex: capture from monitor ports, passive to avoid driving the graph
            props.insert(*pw::keys::STREAM_CAPTURE_SINK, "true");
            props.insert(*pw::keys::NODE_PASSIVE, "true");
        }
        // Sources: no STREAM_CAPTURE_SINK (capture from output ports directly),
        // non-passive so WirePlumber will link us to the source

        let stream = Stream::new(
            &self.core,
            &format!("blabbergraph.peak.{}", node_id),
            props,
        )?;

        let data = MonitorData {
            node_id,
            event_tx: self.event_tx.clone(),
            format: Default::default(),
        };

        let listener = stream
            .add_local_listener_with_user_data(data)
            .param_changed(|_, user_data, id, param| {
                let Some(param) = param else { return };
                if id != spa::param::ParamType::Format.as_raw() {
                    return;
                }
                let (media_type, media_subtype) = match format_utils::parse_format(param) {
                    Ok(v) => v,
                    Err(_) => return,
                };
                if media_type != MediaType::Audio || media_subtype != MediaSubtype::Raw {
                    return;
                }
                if let Err(e) = user_data.format.parse(param) {
                    log::warn!("Peak monitor: failed to parse format: {}", e);
                }
            })
            .process(|stream, user_data| {
                let Some(mut buffer) = stream.dequeue_buffer() else {
                    return;
                };
                let datas = buffer.datas_mut();
                if datas.is_empty() {
                    return;
                }

                let data = &mut datas[0];
                let n_channels = user_data.format.channels();
                if n_channels == 0 {
                    return;
                }
                let n_samples = data.chunk().size() / (mem::size_of::<f32>() as u32);

                let Some(samples) = data.data() else { return };

                let mut peaks = Vec::with_capacity(n_channels as usize);
                for c in 0..n_channels {
                    let mut max: f32 = 0.0;
                    for n in (c..n_samples).step_by(n_channels as usize) {
                        let start = n as usize * mem::size_of::<f32>();
                        let end = start + mem::size_of::<f32>();
                        if end > samples.len() {
                            break;
                        }
                        let chan = &samples[start..end];
                        let f = f32::from_le_bytes(chan.try_into().unwrap());
                        max = max.max(f.abs());
                    }
                    peaks.push(max);
                }

                let _ = user_data.event_tx.send(PwEvent::PeakLevel {
                    node_id: user_data.node_id,
                    peaks,
                });
            })
            .register()?;

        let mut audio_info = spa::param::audio::AudioInfoRaw::new();
        audio_info.set_format(spa::param::audio::AudioFormat::F32LE);
        let obj = spa::pod::Object {
            type_: spa::utils::SpaTypes::ObjectParamFormat.as_raw(),
            id: spa::param::ParamType::EnumFormat.as_raw(),
            properties: audio_info.into(),
        };
        let values: Vec<u8> = spa::pod::serialize::PodSerializer::serialize(
            std::io::Cursor::new(Vec::new()),
            &spa::pod::Value::Object(obj),
        )
        .map_err(|_| pw::Error::CreationFailed)?
        .0
        .into_inner();

        let mut params = [Pod::from_bytes(&values).unwrap()];

        // For sources: pass explicit node_id so PipeWire links directly
        // For sinks: use None to let session manager route to monitor ports
        let target_id = if capture_sink { None } else { Some(node_id) };

        stream.connect(
            spa::utils::Direction::Input,
            target_id,
            StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS | StreamFlags::DONT_RECONNECT,
            &mut params,
        )?;

        Ok(MonitoredStream {
            _listener: listener,
            _stream: stream,
        })
    }
}
