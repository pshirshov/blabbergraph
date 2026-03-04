use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{self, Align, Orientation};

use crate::model::routing;
use crate::model::state::AppState;
use crate::pw::message::{BusId, PwCommand};

pub struct BusWidget {
    pub container: gtk::Box,
    pub bus_id: BusId,
    pub volume_scale: gtk::Scale,
    pub mute_button: gtk::ToggleButton,
    pub db_label: gtk::Label,
    pub output_buttons: Vec<(String, gtk::ToggleButton)>,
}

impl BusWidget {
    pub fn new(
        bus_id: BusId,
        name: &str,
        output_names: &[(String, String)],
        active_outputs: &HashSet<String>,
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

        let output_label = gtk::Label::new(Some("Output:"));
        output_label.add_css_class("caption");
        output_label.set_margin_top(4);
        container.append(&output_label);

        let route_box = gtk::Box::new(Orientation::Vertical, 2);
        let mut output_buttons = Vec::new();

        for (node_name, display_name) in output_names {
            let btn = gtk::ToggleButton::with_label(display_name);
            btn.set_halign(Align::Fill);
            route_box.append(&btn);

            // Set initial state BEFORE connecting signal to avoid spurious handler calls
            btn.set_active(active_outputs.contains(node_name));

            let sender = pw_sender.clone();
            let state_ref = state.clone();
            let bid = bus_id;
            let out_node_name = node_name.clone();
            btn.connect_toggled(move |btn| {
                let active = btn.is_active();
                let bus_nid;
                let output_nid;
                {
                    let state = state_ref.borrow();
                    bus_nid = state.bus_node_id(bid);
                    output_nid = state.find_hardware_node_by_name(&out_node_name);
                }

                if let (Some(bus_nid), Some(output_nid)) = (bus_nid, output_nid) {
                    let state = state_ref.borrow();
                    if active {
                        let pairs =
                            routing::compute_link_pairs(&state.graph, bus_nid, output_nid);
                        for (out_port, in_port) in pairs {
                            let _ = sender.send(PwCommand::CreateLink {
                                output_port_id: out_port,
                                input_port_id: in_port,
                            });
                        }
                    } else {
                        let link_ids =
                            routing::find_links_between(&state.graph, bus_nid, output_nid);
                        for link_id in link_ids {
                            let _ = sender.send(PwCommand::DestroyLink { link_id });
                        }
                    }
                    drop(state);

                    let mut state = state_ref.borrow_mut();
                    if let Some(bus) = state.config.buses.iter_mut().find(|b| b.id == bid) {
                        if active {
                            bus.output_targets.insert(out_node_name.clone());
                        } else {
                            bus.output_targets.remove(&out_node_name);
                        }
                    }
                }
            });

            output_buttons.push((node_name.clone(), btn));
        }
        container.append(&route_box);

        // Volume handler
        {
            let sender = pw_sender.clone();
            let state_ref = state.clone();
            let bid = bus_id;
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
                if let Some(nid) = state.bus_node_id(bid) {
                    let bus = state.config.buses.iter().find(|b| b.id == bid);
                    let ch_count = bus
                        .map(|b| match b.channels {
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
            let bid = bus_id;
            mute_button.connect_toggled(move |btn| {
                let muted = btn.is_active();
                let state = state_ref.borrow();
                if let Some(nid) = state.bus_node_id(bid) {
                    let _ = sender.send(PwCommand::SetMute {
                        node_id: nid,
                        muted,
                    });
                }
            });
        }

        Self {
            container,
            bus_id,
            volume_scale,
            mute_button,
            db_label,
            output_buttons,
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
}
