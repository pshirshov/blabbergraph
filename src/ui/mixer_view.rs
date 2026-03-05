use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{self, Orientation, ScrolledWindow};

use crate::model::bus::BusConfig;
use crate::model::routing;
use crate::model::state::AppState;
use crate::model::strip::{StripConfig, StripKind};
use crate::pw::message::{ChannelLayout, PwCommand, StripId};

use super::channel_strip::{ChannelStrip, StripRole};

pub struct MixerView {
    pub container: gtk::Box,
    inputs_box: gtk::Box,
    buses_box: gtk::Box,
    outputs_box: gtk::Box,
    pub strips: Vec<ChannelStrip>,
}

impl MixerView {
    pub fn new(
        pw_sender: &pipewire::channel::Sender<PwCommand>,
        state: &Rc<RefCell<AppState>>,
    ) -> Self {
        let container = gtk::Box::new(Orientation::Vertical, 0);
        container.set_vexpand(true);
        container.set_hexpand(true);

        // Toolbar with Add buttons
        let toolbar = gtk::Box::new(Orientation::Horizontal, 8);
        toolbar.set_margin_start(8);
        toolbar.set_margin_end(8);
        toolbar.set_margin_top(4);
        toolbar.set_margin_bottom(4);

        let add_input_btn = gtk::Button::with_label("+ Virtual Input");
        let add_bus_btn = gtk::Button::with_label("+ Bus");
        let add_voutput_btn = gtk::Button::with_label("+ Virtual Output");
        toolbar.append(&add_input_btn);
        toolbar.append(&add_bus_btn);
        toolbar.append(&add_voutput_btn);
        container.append(&toolbar);

        container.append(&gtk::Separator::new(Orientation::Horizontal));

        // Scrollable area with 3 expander sections
        let scroll = ScrolledWindow::new();
        scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        scroll.set_vexpand(true);

        let content = gtk::Box::new(Orientation::Vertical, 0);
        content.set_margin_start(4);
        content.set_margin_end(4);

        // Inputs section
        let inputs_expander = gtk::Expander::new(Some("Inputs"));
        inputs_expander.set_expanded(true);
        let inputs_box = gtk::Box::new(Orientation::Vertical, 0);
        inputs_expander.set_child(Some(&inputs_box));
        content.append(&inputs_expander);

        // Buses section
        let buses_expander = gtk::Expander::new(Some("Buses"));
        buses_expander.set_expanded(true);
        let buses_box = gtk::Box::new(Orientation::Vertical, 0);
        buses_expander.set_child(Some(&buses_box));
        content.append(&buses_expander);

        // Outputs section
        let outputs_expander = gtk::Expander::new(Some("Outputs"));
        outputs_expander.set_expanded(true);
        let outputs_box = gtk::Box::new(Orientation::Vertical, 0);
        outputs_expander.set_child(Some(&outputs_box));
        content.append(&outputs_expander);

        scroll.set_child(Some(&content));
        container.append(&scroll);

        // "Add Virtual Input" handler
        {
            let sender = pw_sender.clone();
            let state_ref = state.clone();
            add_input_btn.connect_clicked(move |_| {
                let mut state = state_ref.borrow_mut();
                let strip_id = state.config.allocate_strip_id();
                let name = format!("VInput {}", strip_id.0);
                let strip =
                    StripConfig::new_virtual(strip_id, name.clone(), ChannelLayout::Stereo);
                state.config.strips.push(strip);
                let _ = sender.send(PwCommand::CreateVirtualInput {
                    strip_id,
                    name,
                    channels: ChannelLayout::Stereo,
                });
            });
        }

        // "Add Bus" handler
        {
            let sender = pw_sender.clone();
            let state_ref = state.clone();
            add_bus_btn.connect_clicked(move |_| {
                let mut state = state_ref.borrow_mut();
                let bus_id = state.config.allocate_bus_id();
                let name = format!("Bus {}", bus_id.0);
                let bus = BusConfig::new(bus_id, name.clone(), ChannelLayout::Stereo);
                state.config.buses.push(bus);
                let _ = sender.send(PwCommand::CreateBus {
                    bus_id,
                    name,
                    channels: ChannelLayout::Stereo,
                });
            });
        }

        // "Add Virtual Output" handler
        {
            let sender = pw_sender.clone();
            let state_ref = state.clone();
            add_voutput_btn.connect_clicked(move |_| {
                let mut state = state_ref.borrow_mut();
                let voutput_id = state.config.allocate_voutput_id();
                let strip_id = state.config.allocate_strip_id();
                let name = format!("VOutput {}", voutput_id.0);
                let strip = StripConfig::new_virtual_output(
                    strip_id,
                    voutput_id,
                    name.clone(),
                    ChannelLayout::Stereo,
                );
                state.config.strips.push(strip);
                let _ = sender.send(PwCommand::CreateVirtualOutput {
                    voutput_id,
                    name,
                    channels: ChannelLayout::Stereo,
                });
            });
        }

        Self {
            container,
            inputs_box,
            buses_box,
            outputs_box,
            strips: Vec::new(),
        }
    }

    pub fn rebuild(
        &mut self,
        pw_sender: &pipewire::channel::Sender<PwCommand>,
        state: &Rc<RefCell<AppState>>,
    ) {
        // Clear all
        while let Some(child) = self.inputs_box.first_child() {
            self.inputs_box.remove(&child);
        }
        while let Some(child) = self.buses_box.first_child() {
            self.buses_box.remove(&child);
        }
        while let Some(child) = self.outputs_box.first_child() {
            self.outputs_box.remove(&child);
        }
        self.strips.clear();

        let state_borrow = state.borrow();

        // Bus names as route targets for input strips: key = BusId as string
        let bus_route_targets: Vec<(String, String)> = state_borrow
            .config
            .buses
            .iter()
            .map(|b| (b.id.0.to_string(), b.name.clone()))
            .collect();

        // Hardware output names as route targets for bus strips: key = node_name
        let mut output_route_targets: Vec<(String, String)> = state_borrow
            .graph
            .audio_sink_nodes()
            .into_iter()
            .filter(|n| !n.name.starts_with("blabbergraph."))
            .map(|n| {
                let display = if n.description.is_empty() {
                    n.name.clone()
                } else {
                    n.description.clone()
                };
                (n.name.clone(), display)
            })
            .collect();
        output_route_targets.sort_by(|a, b| a.1.cmp(&b.1));

        // --- Inputs section ---
        // Configured strips (non-voutput)
        for strip in &state_borrow.config.strips {
            if strip.is_virtual_output() {
                continue;
            }

            let name = match &strip.kind {
                StripKind::HardwareInput { node_name } => state_borrow
                    .find_hardware_node_by_name(node_name)
                    .and_then(|nid| state_borrow.graph.nodes.get(&nid))
                    .map(|n| n.description.clone())
                    .unwrap_or_else(|| node_name.clone()),
                StripKind::VirtualInput { name } => name.clone(),
                StripKind::VirtualOutput { .. } => unreachable!(),
            };

            let source_nid = match &strip.kind {
                StripKind::HardwareInput { node_name } => {
                    state_borrow.find_hardware_node_by_name(node_name)
                }
                StripKind::VirtualInput { .. } => state_borrow.strip_node_id(strip.id),
                StripKind::VirtualOutput { .. } => unreachable!(),
            };

            let active_routes = compute_active_input_routes(
                &state_borrow,
                source_nid,
                &bus_route_targets,
            );
            let is_deletable = strip.is_virtual();

            let is_renamable = strip.is_virtual();
            let widget = ChannelStrip::new(
                StripRole::Input {
                    strip_id: strip.id,
                },
                &name,
                &bus_route_targets,
                &active_routes,
                is_deletable,
                is_renamable,
                None,
                pw_sender,
                state,
            );
            let vol = strip.volume.first().copied().unwrap_or(1.0);
            widget.volume_scale.set_value(vol as f64 * 100.0);
            widget.update_mute(strip.muted);

            self.inputs_box.append(&widget.container);
            self.strips.push(widget);
        }

        // Discoverable hardware sources not in config
        for node in state_borrow.graph.nodes.values() {
            if node.media_class == "Audio/Source" || node.media_class == "Audio/Source/Virtual" {
                let already_configured = state_borrow.config.strips.iter().any(|s| {
                    matches!(&s.kind, StripKind::HardwareInput { node_name } if node_name == &node.name)
                });
                if !already_configured && !node.name.starts_with("blabbergraph.") {
                    let display_name = if node.description.is_empty() {
                        &node.name
                    } else {
                        &node.description
                    };
                    let active_routes = compute_active_input_routes(
                        &state_borrow,
                        Some(node.id),
                        &bus_route_targets,
                    );
                    let widget = ChannelStrip::new(
                        StripRole::Input {
                            strip_id: StripId(10000 + node.id),
                        },
                        display_name,
                        &bus_route_targets,
                        &active_routes,
                        false,
                        false,
                        Some(node.id),
                        pw_sender,
                        state,
                    );
                    self.inputs_box.append(&widget.container);
                    self.strips.push(widget);
                }
            }
        }

        // --- Buses section ---
        for bus in &state_borrow.config.buses {
            // Build per-bus route targets: hw outputs + other buses (excluding self)
            let mut bus_route_targets_for_bus = output_route_targets.clone();
            for other_bus in &state_borrow.config.buses {
                if other_bus.id == bus.id {
                    continue;
                }
                let node_name = format!("blabbergraph.bus.{}", other_bus.name);
                bus_route_targets_for_bus.push((node_name, format!("[Bus] {}", other_bus.name)));
            }

            let active_outputs: HashSet<String> =
                if let Some(bus_nid) = state_borrow.bus_node_id(bus.id) {
                    bus_route_targets_for_bus
                        .iter()
                        .filter(|(node_name, _)| {
                            if let Some(out_nid) =
                                state_borrow.find_hardware_node_by_name(node_name)
                            {
                                !routing::find_links_between(&state_borrow.graph, bus_nid, out_nid)
                                    .is_empty()
                            } else {
                                false
                            }
                        })
                        .map(|(node_name, _)| node_name.clone())
                        .collect()
                } else {
                    bus.output_targets.clone()
                };

            let widget = ChannelStrip::new(
                StripRole::Bus { bus_id: bus.id },
                &bus.name,
                &bus_route_targets_for_bus,
                &active_outputs,
                true,
                true,
                None,
                pw_sender,
                state,
            );
            let vol = bus.volume.first().copied().unwrap_or(1.0);
            widget.volume_scale.set_value(vol as f64 * 100.0);
            widget.update_mute(bus.muted);

            self.buses_box.append(&widget.container);
            self.strips.push(widget);
        }

        // --- Outputs section ---
        // Hardware outputs
        for (node_name, display_name) in &output_route_targets {
            if let Some(node) = state_borrow
                .graph
                .nodes
                .values()
                .find(|n| &n.name == node_name)
            {
                let widget = ChannelStrip::new(
                    StripRole::HardwareOutput {
                        node_name: node_name.clone(),
                    },
                    display_name,
                    &[],
                    &HashSet::new(),
                    false,
                    false,
                    None,
                    pw_sender,
                    state,
                );
                widget.update_volume(&node.volumes);
                widget.update_mute(node.muted);
                self.outputs_box.append(&widget.container);
                self.strips.push(widget);
            }
        }

        // Virtual outputs
        for strip in &state_borrow.config.strips {
            if let StripKind::VirtualOutput {
                voutput_id, name, ..
            } = &strip.kind
            {
                let widget = ChannelStrip::new(
                    StripRole::VirtualOutput {
                        voutput_id: *voutput_id,
                    },
                    name,
                    &[],
                    &HashSet::new(),
                    true,
                    true,
                    None,
                    pw_sender,
                    state,
                );
                let vol = strip.volume.first().copied().unwrap_or(1.0);
                widget.volume_scale.set_value(vol as f64 * 100.0);
                widget.update_mute(strip.muted);
                self.outputs_box.append(&widget.container);
                self.strips.push(widget);
            }
        }

        // Collect all PW (node_id, node_name, capture_sink) tuples for peak monitoring
        let mut monitored_nodes: Vec<(u32, String, bool)> = Vec::new();
        for strip in &self.strips {
            let node_id = match &strip.role {
                StripRole::Input { strip_id } => state_borrow
                    .strip_node_id(*strip_id)
                    .or_else(|| {
                        let s = state_borrow.config.strips.iter().find(|s| s.id == *strip_id)?;
                        match &s.kind {
                            StripKind::HardwareInput { node_name } => {
                                state_borrow.find_hardware_node_by_name(node_name)
                            }
                            _ => None,
                        }
                    })
                    .or(strip.pre_resolved_node_id),
                StripRole::Bus { bus_id } => state_borrow.bus_node_id(*bus_id),
                StripRole::HardwareOutput { node_name } => {
                    state_borrow.find_hardware_node_by_name(node_name)
                }
                StripRole::VirtualOutput { voutput_id } => {
                    state_borrow.voutput_node_id(*voutput_id)
                }
            };
            if let Some(nid) = node_id {
                if let Some(node) = state_borrow.graph.nodes.get(&nid) {
                    // Sources produce audio on output ports — capture directly.
                    // Sinks/duplex need STREAM_CAPTURE_SINK to tap monitor ports.
                    let capture_sink = node.media_class == "Audio/Sink"
                        || node.media_class == "Audio/Duplex";
                    monitored_nodes.push((nid, node.name.clone(), capture_sink));
                }
            }
        }

        drop(state_borrow);

        let _ = pw_sender.send(PwCommand::SetMonitoredNodes {
            nodes: monitored_nodes,
        });
    }

    pub fn update_peak_level(&self, state: &AppState, node_id: u32, peaks: &[f32]) {
        for strip in &self.strips {
            let matches = match &strip.role {
                StripRole::Input { strip_id } => {
                    state.node_strip_map.get(&node_id) == Some(strip_id)
                        || strip.pre_resolved_node_id == Some(node_id)
                }
                StripRole::Bus { bus_id } => state.node_bus_map.get(&node_id) == Some(bus_id),
                StripRole::HardwareOutput { node_name } => {
                    state
                        .graph
                        .nodes
                        .get(&node_id)
                        .map_or(false, |n| &n.name == node_name)
                }
                StripRole::VirtualOutput { voutput_id } => {
                    state.node_voutput_map.get(&node_id) == Some(voutput_id)
                }
            };

            if matches {
                strip.update_level(peaks);
            }
        }
    }

    pub fn update_node_params(&self, state: &AppState, node_id: u32) {
        let node = match state.graph.nodes.get(&node_id) {
            Some(n) => n,
            None => return,
        };

        for strip in &self.strips {
            let matches = match &strip.role {
                StripRole::Input { strip_id } => {
                    state.node_strip_map.get(&node_id) == Some(strip_id)
                }
                StripRole::Bus { bus_id } => state.node_bus_map.get(&node_id) == Some(bus_id),
                StripRole::HardwareOutput { node_name } => &node.name == node_name,
                StripRole::VirtualOutput { voutput_id } => {
                    state.node_voutput_map.get(&node_id) == Some(voutput_id)
                }
            };

            if matches {
                strip.update_volume(&node.volumes);
                strip.update_mute(node.muted);
            }
        }
    }
}

/// Derive active input routes from actual PW graph links rather than config state.
/// For each bus route target, check if links exist from `source_nid` to the bus node.
fn compute_active_input_routes(
    state: &AppState,
    source_nid: Option<u32>,
    bus_route_targets: &[(String, String)],
) -> HashSet<String> {
    let Some(src) = source_nid else {
        return HashSet::new();
    };
    bus_route_targets
        .iter()
        .filter(|(bus_id_str, _)| {
            let bus_id = match bus_id_str.parse::<u32>() {
                Ok(v) => crate::pw::message::BusId(v),
                Err(_) => return false,
            };
            if let Some(bus_nid) = state.bus_node_id(bus_id) {
                !routing::find_links_between(&state.graph, src, bus_nid).is_empty()
            } else {
                false
            }
        })
        .map(|(key, _)| key.clone())
        .collect()
}
