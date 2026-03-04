use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{self, Orientation, ScrolledWindow};

use crate::model::bus::BusConfig;
use crate::model::routing;
use crate::model::state::AppState;
use crate::model::strip::StripConfig;
use crate::pw::message::{BusId, ChannelLayout, PwCommand, StripId};

use super::bus_widget::BusWidget;
use super::output_widget::OutputWidget;
use super::strip_widget::StripWidget;

pub struct MixerView {
    pub container: gtk::Box,
    pub strips_box: gtk::Box,
    pub buses_box: gtk::Box,
    pub outputs_box: gtk::Box,
    pub strip_widgets: Vec<StripWidget>,
    pub bus_widgets: Vec<BusWidget>,
    pub output_widgets: Vec<OutputWidget>,
    pub add_strip_button: gtk::Button,
    pub add_bus_button: gtk::Button,
}

impl MixerView {
    pub fn new(
        pw_sender: &pipewire::channel::Sender<PwCommand>,
        state: &Rc<RefCell<AppState>>,
    ) -> Self {
        let container = gtk::Box::new(Orientation::Horizontal, 0);
        container.set_vexpand(true);
        container.set_hexpand(true);

        // Left: Input Strips
        let strips_frame = gtk::Frame::new(Some("Input Strips"));
        strips_frame.set_hexpand(true);
        let strips_scroll = ScrolledWindow::new();
        strips_scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
        strips_scroll.set_vexpand(true);
        let strips_box = gtk::Box::new(Orientation::Horizontal, 0);
        strips_box.set_vexpand(true);
        strips_scroll.set_child(Some(&strips_box));

        let strips_outer = gtk::Box::new(Orientation::Vertical, 4);
        strips_outer.set_hexpand(true);
        strips_outer.set_margin_start(4);
        strips_outer.set_margin_end(4);
        strips_outer.append(&strips_scroll);

        let add_strip_button = gtk::Button::with_label("+ Add Virtual Input");
        add_strip_button.set_halign(gtk::Align::Start);
        add_strip_button.set_margin_start(8);
        add_strip_button.set_margin_bottom(8);
        strips_outer.append(&add_strip_button);

        strips_frame.set_child(Some(&strips_outer));
        container.append(&strips_frame);

        container.append(&gtk::Separator::new(Orientation::Vertical));

        // Center: Buses
        let buses_frame = gtk::Frame::new(Some("Buses"));
        buses_frame.set_hexpand(true);
        let buses_scroll = ScrolledWindow::new();
        buses_scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
        buses_scroll.set_vexpand(true);
        let buses_box = gtk::Box::new(Orientation::Horizontal, 0);
        buses_box.set_vexpand(true);
        buses_scroll.set_child(Some(&buses_box));

        let buses_outer = gtk::Box::new(Orientation::Vertical, 4);
        buses_outer.set_hexpand(true);
        buses_outer.set_margin_start(4);
        buses_outer.set_margin_end(4);
        buses_outer.append(&buses_scroll);

        let add_bus_button = gtk::Button::with_label("+ Add Bus");
        add_bus_button.set_halign(gtk::Align::Start);
        add_bus_button.set_margin_start(8);
        add_bus_button.set_margin_bottom(8);
        buses_outer.append(&add_bus_button);

        buses_frame.set_child(Some(&buses_outer));
        container.append(&buses_frame);

        container.append(&gtk::Separator::new(Orientation::Vertical));

        // Right: Output Strips
        let outputs_frame = gtk::Frame::new(Some("Outputs"));
        outputs_frame.set_hexpand(true);
        let outputs_scroll = ScrolledWindow::new();
        outputs_scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
        outputs_scroll.set_vexpand(true);
        let outputs_box = gtk::Box::new(Orientation::Horizontal, 0);
        outputs_box.set_vexpand(true);
        outputs_scroll.set_child(Some(&outputs_box));

        let outputs_outer = gtk::Box::new(Orientation::Vertical, 4);
        outputs_outer.set_hexpand(true);
        outputs_outer.set_margin_start(4);
        outputs_outer.set_margin_end(4);
        outputs_outer.append(&outputs_scroll);

        outputs_frame.set_child(Some(&outputs_outer));
        container.append(&outputs_frame);

        // "Add Virtual Input" button handler
        {
            let sender = pw_sender.clone();
            let state_ref = state.clone();
            add_strip_button.connect_clicked(move |_| {
                let mut state = state_ref.borrow_mut();
                let strip_id = state.config.allocate_strip_id();
                let name = format!("VInput {}", strip_id.0);
                let strip = StripConfig::new_virtual(
                    strip_id,
                    name.clone(),
                    ChannelLayout::Stereo,
                );
                state.config.strips.push(strip);
                let _ = sender.send(PwCommand::CreateVirtualInput {
                    strip_id,
                    name,
                    channels: ChannelLayout::Stereo,
                });
            });
        }

        // "Add Bus" button handler
        {
            let sender = pw_sender.clone();
            let state_ref = state.clone();
            add_bus_button.connect_clicked(move |_| {
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

        Self {
            container,
            strips_box,
            buses_box,
            outputs_box,
            strip_widgets: Vec::new(),
            bus_widgets: Vec::new(),
            output_widgets: Vec::new(),
            add_strip_button,
            add_bus_button,
        }
    }

    pub fn rebuild(
        &mut self,
        pw_sender: &pipewire::channel::Sender<PwCommand>,
        state: &Rc<RefCell<AppState>>,
    ) {
        // Clear existing widgets
        while let Some(child) = self.strips_box.first_child() {
            self.strips_box.remove(&child);
        }
        while let Some(child) = self.buses_box.first_child() {
            self.buses_box.remove(&child);
        }
        while let Some(child) = self.outputs_box.first_child() {
            self.outputs_box.remove(&child);
        }
        self.strip_widgets.clear();
        self.bus_widgets.clear();
        self.output_widgets.clear();

        let state_borrow = state.borrow();

        let bus_names: Vec<(BusId, String)> = state_borrow
            .config
            .buses
            .iter()
            .map(|b| (b.id, b.name.clone()))
            .collect();

        // Collect hardware outputs (sinks not managed by us)
        let mut output_names: Vec<(String, String)> = state_borrow
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
        output_names.sort_by(|a, b| a.1.cmp(&b.1));

        // Create strip widgets
        for strip in &state_borrow.config.strips {
            let name = match &strip.kind {
                crate::model::strip::StripKind::HardwareInput { node_name } => {
                    state_borrow
                        .find_hardware_node_by_name(node_name)
                        .and_then(|nid| state_borrow.graph.nodes.get(&nid))
                        .map(|n| n.description.clone())
                        .unwrap_or_else(|| node_name.clone())
                }
                crate::model::strip::StripKind::VirtualInput { name } => name.clone(),
            };

            let widget = StripWidget::new(strip.id, &name, &bus_names, pw_sender, state);
            widget.volume_scale.set_value(
                strip.volume.first().copied().unwrap_or(1.0) as f64 * 100.0,
            );
            widget.mute_button.set_active(strip.muted);

            for (bus_id, btn) in &widget.route_buttons {
                btn.set_active(strip.routed_to.contains(bus_id));
            }

            self.strips_box.append(&widget.container);
            self.strip_widgets.push(widget);
        }

        // Also show hardware source nodes not in config as discoverable strips
        for node in state_borrow.graph.nodes.values() {
            if node.media_class == "Audio/Source" || node.media_class == "Audio/Source/Virtual" {
                let already_configured = state_borrow.config.strips.iter().any(|s| {
                    if let crate::model::strip::StripKind::HardwareInput { ref node_name } =
                        s.kind
                    {
                        node_name == &node.name
                    } else {
                        false
                    }
                });
                if !already_configured && !node.name.starts_with("blabbergraph.") {
                    let display_name = if node.description.is_empty() {
                        &node.name
                    } else {
                        &node.description
                    };
                    let widget = StripWidget::new(
                        StripId(10000 + node.id),
                        display_name,
                        &bus_names,
                        pw_sender,
                        state,
                    );
                    self.strips_box.append(&widget.container);
                    self.strip_widgets.push(widget);
                }
            }
        }

        // Create bus widgets with output route toggles
        for bus in &state_borrow.config.buses {
            let active_outputs: HashSet<String> =
                if let Some(bus_nid) = state_borrow.bus_node_id(bus.id) {
                    output_names
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

            let widget = BusWidget::new(
                bus.id,
                &bus.name,
                &output_names,
                &active_outputs,
                pw_sender,
                state,
            );
            widget
                .volume_scale
                .set_value(bus.volume.first().copied().unwrap_or(1.0) as f64 * 100.0);
            widget.mute_button.set_active(bus.muted);

            self.buses_box.append(&widget.container);
            self.bus_widgets.push(widget);
        }

        // Create output widgets for hardware sinks
        for (node_name, display_name) in &output_names {
            if let Some(node) = state_borrow
                .graph
                .nodes
                .values()
                .find(|n| &n.name == node_name)
            {
                let widget = OutputWidget::new(node_name, display_name, node.id, pw_sender);
                widget.update_volume(&node.volumes);
                widget.update_mute(node.muted);
                self.outputs_box.append(&widget.container);
                self.output_widgets.push(widget);
            }
        }

        drop(state_borrow);
    }

    pub fn update_node_params(&self, state: &AppState, node_id: u32) {
        if let Some(&bus_id) = state.node_bus_map.get(&node_id) {
            if let Some(node) = state.graph.nodes.get(&node_id) {
                for bw in &self.bus_widgets {
                    if bw.bus_id == bus_id {
                        bw.update_volume(&node.volumes);
                        bw.update_mute(node.muted);
                    }
                }
            }
        }
        if let Some(&strip_id) = state.node_strip_map.get(&node_id) {
            if let Some(node) = state.graph.nodes.get(&node_id) {
                for sw in &self.strip_widgets {
                    if sw.strip_id == strip_id {
                        sw.update_volume(&node.volumes);
                        sw.update_mute(node.muted);
                    }
                }
            }
        }
        // Update output widgets (hardware sinks)
        if let Some(node) = state.graph.nodes.get(&node_id) {
            for ow in &self.output_widgets {
                if ow.node_name == node.name {
                    ow.update_volume(&node.volumes);
                    ow.update_mute(node.muted);
                }
            }
        }
    }
}
