use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{self, Align, Orientation};

use crate::model::state::AppState;
use crate::pw::message::{BusId, PwCommand, StripId};

pub struct StripWidget {
    pub container: gtk::Box,
    pub strip_id: StripId,
    pub volume_scale: gtk::Scale,
    pub mute_button: gtk::ToggleButton,
    pub db_label: gtk::Label,
    pub route_buttons: Vec<(BusId, gtk::ToggleButton)>,
    name_label: gtk::Label,
}

impl StripWidget {
    pub fn new(
        strip_id: StripId,
        name: &str,
        bus_names: &[(BusId, String)],
        pw_sender: &pipewire::channel::Sender<PwCommand>,
        state: &Rc<RefCell<AppState>>,
    ) -> Self {
        let container = gtk::Box::new(Orientation::Vertical, 4);
        container.set_width_request(100);
        container.set_margin_start(4);
        container.set_margin_end(4);
        container.set_margin_top(8);
        container.set_margin_bottom(8);
        container.add_css_class("card");
        container.set_vexpand(true);

        let name_label = gtk::Label::new(Some(name));
        name_label.add_css_class("heading");
        name_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        name_label.set_max_width_chars(12);
        name_label.set_halign(Align::Center);
        container.append(&name_label);

        let volume_scale = gtk::Scale::with_range(Orientation::Vertical, 0.0, 150.0, 1.0);
        volume_scale.set_inverted(true);
        volume_scale.set_value(100.0);
        volume_scale.set_vexpand(true);
        volume_scale.set_size_request(-1, 150);
        volume_scale.add_mark(0.0, gtk::PositionType::Right, Some("-inf"));
        volume_scale.add_mark(100.0, gtk::PositionType::Right, Some("0dB"));
        volume_scale.add_mark(150.0, gtk::PositionType::Right, Some("+6dB"));
        container.append(&volume_scale);

        let db_label = gtk::Label::new(Some("0.0 dB"));
        db_label.add_css_class("caption");
        db_label.set_halign(Align::Center);
        container.append(&db_label);

        let mute_button = gtk::ToggleButton::with_label("M");
        mute_button.set_halign(Align::Center);
        mute_button.add_css_class("destructive-action");
        mute_button.set_has_frame(true);
        container.append(&mute_button);

        let routes_label = gtk::Label::new(Some("Route:"));
        routes_label.add_css_class("caption");
        routes_label.set_margin_top(4);
        container.append(&routes_label);

        let route_box = gtk::Box::new(Orientation::Vertical, 2);
        let mut route_buttons = Vec::new();

        for (bus_id, bus_name) in bus_names {
            let btn = gtk::ToggleButton::with_label(bus_name);
            btn.set_halign(Align::Fill);
            route_box.append(&btn);

            let sender = pw_sender.clone();
            let state_ref = state.clone();
            let sid = strip_id;
            let bid = *bus_id;
            btn.connect_toggled(move |btn| {
                let active = btn.is_active();
                let state = state_ref.borrow();

                let source_node_id = state.strip_node_id(sid).or_else(|| {
                    let strip = state.config.strips.iter().find(|s| s.id == sid)?;
                    match &strip.kind {
                        crate::model::strip::StripKind::HardwareInput { node_name } => {
                            state.find_hardware_node_by_name(node_name)
                        }
                        crate::model::strip::StripKind::VirtualInput { .. } => {
                            state.strip_node_id(sid)
                        }
                    }
                });

                let dest_node_id = state.bus_node_id(bid);

                if let (Some(src), Some(dst)) = (source_node_id, dest_node_id) {
                    if active {
                        let pairs =
                            crate::model::routing::compute_link_pairs(&state.graph, src, dst);
                        for (out_port, in_port) in pairs {
                            let _ = sender.send(PwCommand::CreateLink {
                                output_port_id: out_port,
                                input_port_id: in_port,
                            });
                        }
                    } else {
                        let link_ids =
                            crate::model::routing::find_links_between(&state.graph, src, dst);
                        for link_id in link_ids {
                            let _ = sender.send(PwCommand::DestroyLink { link_id });
                        }
                    }
                }
            });

            route_buttons.push((*bus_id, btn));
        }
        container.append(&route_box);

        // Volume change handler
        {
            let sender = pw_sender.clone();
            let state_ref = state.clone();
            let sid = strip_id;
            let db_label_ref = db_label.clone();
            volume_scale.connect_value_changed(move |scale| {
                let percent = scale.value();
                let linear = percent / 100.0;
                let db = if linear > 0.0 {
                    20.0 * (linear as f64).log10()
                } else {
                    -f64::INFINITY
                };
                db_label_ref.set_text(&format!("{:.1} dB", db));

                let state = state_ref.borrow();
                let node_id = state.strip_node_id(sid).or_else(|| {
                    let strip = state.config.strips.iter().find(|s| s.id == sid)?;
                    match &strip.kind {
                        crate::model::strip::StripKind::HardwareInput { node_name } => {
                            state.find_hardware_node_by_name(node_name)
                        }
                        _ => state.strip_node_id(sid),
                    }
                });
                if let Some(nid) = node_id {
                    let strip = state.config.strips.iter().find(|s| s.id == sid);
                    let ch_count = strip
                        .map(|s| match s.channels {
                            crate::pw::message::ChannelLayout::Mono => 1,
                            crate::pw::message::ChannelLayout::Stereo => 2,
                        })
                        .unwrap_or(2);
                    let volumes = vec![linear as f32; ch_count];
                    let _ = sender.send(PwCommand::SetVolume {
                        node_id: nid,
                        volumes,
                    });
                }
            });
        }

        // Mute handler
        {
            let sender = pw_sender.clone();
            let state_ref = state.clone();
            let sid = strip_id;
            mute_button.connect_toggled(move |btn| {
                let muted = btn.is_active();
                let state = state_ref.borrow();
                let node_id = state.strip_node_id(sid).or_else(|| {
                    let strip = state.config.strips.iter().find(|s| s.id == sid)?;
                    match &strip.kind {
                        crate::model::strip::StripKind::HardwareInput { node_name } => {
                            state.find_hardware_node_by_name(node_name)
                        }
                        _ => state.strip_node_id(sid),
                    }
                });
                if let Some(nid) = node_id {
                    let _ = sender.send(PwCommand::SetMute {
                        node_id: nid,
                        muted,
                    });
                }
            });
        }

        Self {
            container,
            strip_id,
            volume_scale,
            mute_button,
            db_label,
            route_buttons,
            name_label,
        }
    }

    pub fn update_volume(&self, volumes: &[f32]) {
        if let Some(&v) = volumes.first() {
            let percent = v as f64 * 100.0;
            self.volume_scale.set_value(percent);
        }
    }

    pub fn update_mute(&self, muted: bool) {
        self.mute_button.set_active(muted);
    }

    pub fn update_name(&self, name: &str) {
        self.name_label.set_text(name);
    }
}
