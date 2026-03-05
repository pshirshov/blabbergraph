use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{self, Align, Orientation};

use crate::model::routing;
use crate::model::state::AppState;
use crate::model::strip::StripKind;
use crate::pw::message::{BusId, PwCommand, StripId, VirtualOutputId};

const PEAK_SMOOTHING: f64 = 0.3;

#[derive(Clone, Debug)]
pub enum StripRole {
    Input { strip_id: StripId },
    Bus { bus_id: BusId },
    HardwareOutput { node_name: String },
    VirtualOutput { voutput_id: VirtualOutputId },
}

pub struct ChannelStrip {
    pub container: gtk::Box,
    pub role: StripRole,
    pub volume_scale: gtk::Scale,
    pub mute_switch: gtk::Switch,
    pub level_bar: gtk::LevelBar,
    pub route_buttons: Vec<(String, gtk::ToggleButton)>,
    pub delete_button: Option<gtk::Button>,
    name_stack: gtk::Stack,
    name_label: gtk::Label,
    name_entry: gtk::Entry,
    is_renamable: bool,
    /// Pre-resolved PW node ID for discovered (unconfigured) hardware inputs
    pub pre_resolved_node_id: Option<u32>,
    /// Guard flag to suppress signal handlers during programmatic updates
    updating: Rc<Cell<bool>>,
}

impl ChannelStrip {
    pub fn new(
        role: StripRole,
        display_name: &str,
        route_targets: &[(String, String)],
        active_routes: &HashSet<String>,
        is_deletable: bool,
        is_renamable: bool,
        pre_resolved_node_id: Option<u32>,
        pw_sender: &pipewire::channel::Sender<PwCommand>,
        state: &Rc<RefCell<AppState>>,
    ) -> Self {
        let container = gtk::Box::new(Orientation::Vertical, 2);
        container.add_css_class("card");
        container.set_margin_start(4);
        container.set_margin_end(4);
        container.set_margin_top(2);
        container.set_margin_bottom(2);

        // Top row: name + level bar + volume + mute + delete
        let top_row = gtk::Box::new(Orientation::Horizontal, 6);
        top_row.set_margin_start(8);
        top_row.set_margin_end(8);
        top_row.set_margin_top(4);
        top_row.set_margin_bottom(2);

        // Name: a Stack switching between Label (display) and Entry (edit)
        let name_stack = gtk::Stack::new();
        name_stack.set_width_request(120);
        name_stack.set_halign(Align::Start);

        let name_label = gtk::Label::new(Some(display_name));
        name_label.add_css_class("heading");
        name_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        name_label.set_width_chars(14);
        name_label.set_max_width_chars(14);
        name_label.set_xalign(0.0);
        name_stack.add_named(&name_label, Some("label"));

        let name_entry = gtk::Entry::new();
        name_entry.set_text(display_name);
        name_entry.set_width_chars(14);
        name_entry.set_max_width_chars(14);
        name_stack.add_named(&name_entry, Some("entry"));

        name_stack.set_visible_child_name("label");
        top_row.append(&name_stack);

        let level_bar = gtk::LevelBar::new();
        level_bar.set_min_value(0.0);
        level_bar.set_max_value(1.0);
        level_bar.set_value(0.0);
        level_bar.set_hexpand(false);
        level_bar.set_width_request(60);
        level_bar.set_valign(Align::Center);
        top_row.append(&level_bar);

        let volume_scale = gtk::Scale::with_range(Orientation::Horizontal, 0.0, 150.0, 1.0);
        volume_scale.set_value(100.0);
        volume_scale.set_hexpand(true);
        volume_scale.add_mark(0.0, gtk::PositionType::Bottom, Some("-inf"));
        volume_scale.add_mark(100.0, gtk::PositionType::Bottom, Some("0dB"));
        volume_scale.add_mark(150.0, gtk::PositionType::Bottom, Some("+6"));
        top_row.append(&volume_scale);

        let mute_switch = gtk::Switch::new();
        mute_switch.set_valign(Align::Center);
        mute_switch.set_active(false); // active = NOT muted (switch ON = unmuted)
        mute_switch.set_tooltip_text(Some("Mute"));
        top_row.append(&mute_switch);

        // Delete button or fixed-width spacer for alignment
        let delete_button = if is_deletable {
            let btn = gtk::Button::from_icon_name("user-trash-symbolic");
            btn.set_valign(Align::Center);
            btn.add_css_class("flat");
            top_row.append(&btn);
            Some(btn)
        } else {
            let spacer = gtk::Box::new(Orientation::Horizontal, 0);
            spacer.set_width_request(34);
            top_row.append(&spacer);
            None
        };

        container.append(&top_row);

        // Bottom row: route buttons (if any)
        let mut route_buttons = Vec::new();
        if !route_targets.is_empty() {
            let route_row = gtk::Box::new(Orientation::Horizontal, 4);
            route_row.set_margin_start(8);
            route_row.set_margin_end(8);
            route_row.set_margin_bottom(4);

            let route_label = gtk::Label::new(Some("Route:"));
            route_label.add_css_class("caption");
            route_label.set_margin_end(4);
            route_row.append(&route_label);

            for (key, label) in route_targets {
                let btn = gtk::ToggleButton::with_label(label);
                btn.add_css_class("flat");
                // Set initial state BEFORE connecting signal
                btn.set_active(active_routes.contains(key));
                route_row.append(&btn);
                route_buttons.push((key.clone(), btn));
            }

            container.append(&route_row);
        }

        let updating = Rc::new(Cell::new(false));

        // Connect signal handlers
        Self::connect_volume_handler(&role, &volume_scale, pre_resolved_node_id, pw_sender, state, &updating);
        Self::connect_mute_handler(&role, &mute_switch, pre_resolved_node_id, pw_sender, state, &updating);
        Self::connect_route_handlers(&role, &route_buttons, pre_resolved_node_id, pw_sender, state);

        if let Some(ref del_btn) = delete_button {
            Self::connect_delete_handler(&role, del_btn, pw_sender, state);
        }

        if is_renamable {
            Self::connect_rename_handlers(
                &role,
                &name_stack,
                &name_label,
                &name_entry,
                state,
            );
        }

        Self {
            container,
            role,
            volume_scale,
            mute_switch,
            level_bar,
            route_buttons,
            delete_button,
            name_stack,
            name_label,
            name_entry,
            is_renamable,
            pre_resolved_node_id,
            updating,
        }
    }

    fn connect_rename_handlers(
        role: &StripRole,
        stack: &gtk::Stack,
        label: &gtk::Label,
        entry: &gtk::Entry,
        state: &Rc<RefCell<AppState>>,
    ) {
        // Double-click on label -> switch to entry
        let gesture = gtk::GestureClick::new();
        gesture.set_button(1);
        let stack_ref = stack.clone();
        let entry_ref = entry.clone();
        let label_ref = label.clone();
        gesture.connect_released(move |gesture, n_press, _x, _y| {
            if n_press == 2 {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                entry_ref.set_text(&label_ref.text());
                stack_ref.set_visible_child_name("entry");
                entry_ref.grab_focus();
            }
        });
        label.add_controller(gesture);

        // Enter in entry -> commit rename
        let stack_ref = stack.clone();
        let label_ref = label.clone();
        let state_ref = state.clone();
        let role_clone = role.clone();
        entry.connect_activate(move |entry| {
            let new_name = entry.text().to_string();
            if !new_name.is_empty() {
                label_ref.set_text(&new_name);
                apply_rename(&role_clone, &new_name, &state_ref);
            }
            stack_ref.set_visible_child_name("label");
        });

        // Focus-out -> commit rename
        let focus_controller = gtk::EventControllerFocus::new();
        let stack_ref = stack.clone();
        let label_ref = label.clone();
        let state_ref = state.clone();
        let role_clone = role.clone();
        let entry_ref = entry.clone();
        focus_controller.connect_leave(move |_| {
            if stack_ref.visible_child_name().as_deref() == Some("entry") {
                let new_name = entry_ref.text().to_string();
                if !new_name.is_empty() {
                    label_ref.set_text(&new_name);
                    apply_rename(&role_clone, &new_name, &state_ref);
                }
                stack_ref.set_visible_child_name("label");
            }
        });
        entry.add_controller(focus_controller);

        // Escape -> cancel rename
        let key_controller = gtk::EventControllerKey::new();
        let stack_ref = stack.clone();
        key_controller.connect_key_pressed(move |_, key, _, _| {
            if key == gtk::gdk::Key::Escape {
                stack_ref.set_visible_child_name("label");
                return gtk::glib::Propagation::Stop;
            }
            gtk::glib::Propagation::Proceed
        });
        entry.add_controller(key_controller);
    }

    fn connect_volume_handler(
        role: &StripRole,
        scale: &gtk::Scale,
        pre_resolved_node_id: Option<u32>,
        pw_sender: &pipewire::channel::Sender<PwCommand>,
        state: &Rc<RefCell<AppState>>,
        updating: &Rc<Cell<bool>>,
    ) {
        let sender = pw_sender.clone();
        let state_ref = state.clone();
        let role = role.clone();
        let guard = updating.clone();
        scale.connect_value_changed(move |scale| {
            if guard.get() {
                return;
            }
            let percent = scale.value();
            let linear = (percent / 100.0) as f32;

            let state = state_ref.borrow();
            let (node_id, ch_count) = resolve_node_and_channels(&role, &state, pre_resolved_node_id);
            if let Some(nid) = node_id {
                let volumes = vec![linear; ch_count];
                let _ = sender.send(PwCommand::SetVolume {
                    node_id: nid,
                    volumes,
                });
            }
        });
    }

    fn connect_mute_handler(
        role: &StripRole,
        switch: &gtk::Switch,
        pre_resolved_node_id: Option<u32>,
        pw_sender: &pipewire::channel::Sender<PwCommand>,
        state: &Rc<RefCell<AppState>>,
        updating: &Rc<Cell<bool>>,
    ) {
        let sender = pw_sender.clone();
        let state_ref = state.clone();
        let role = role.clone();
        let guard = updating.clone();
        // Switch ON = unmuted, OFF = muted
        switch.connect_state_set(move |_, switch_active| {
            if guard.get() {
                return gtk::glib::Propagation::Proceed;
            }
            let muted = !switch_active;
            let state = state_ref.borrow();
            let (node_id, _) = resolve_node_and_channels(&role, &state, pre_resolved_node_id);
            if let Some(nid) = node_id {
                let _ = sender.send(PwCommand::SetMute {
                    node_id: nid,
                    muted,
                });
            }
            gtk::glib::Propagation::Proceed
        });
    }

    fn connect_route_handlers(
        role: &StripRole,
        buttons: &[(String, gtk::ToggleButton)],
        pre_resolved_node_id: Option<u32>,
        pw_sender: &pipewire::channel::Sender<PwCommand>,
        state: &Rc<RefCell<AppState>>,
    ) {
        match role {
            StripRole::Input { strip_id } => {
                let sid = *strip_id;
                for (key, btn) in buttons {
                    let sender = pw_sender.clone();
                    let state_ref = state.clone();
                    let bus_id_val: u32 = key.parse().unwrap_or(0);
                    let bid = BusId(bus_id_val);
                    btn.connect_toggled(move |btn| {
                        let active = btn.is_active();
                        let state = state_ref.borrow();

                        let source_node_id = state.strip_node_id(sid).or_else(|| {
                            let strip = state.config.strips.iter().find(|s| s.id == sid)?;
                            match &strip.kind {
                                StripKind::HardwareInput { node_name } => {
                                    state.find_hardware_node_by_name(node_name)
                                }
                                StripKind::VirtualInput { .. } => state.strip_node_id(sid),
                                StripKind::VirtualOutput { .. } => None,
                            }
                        }).or(pre_resolved_node_id);
                        let dest_node_id = state.bus_node_id(bid);

                        if let (Some(src), Some(dst)) = (source_node_id, dest_node_id) {
                            if active {
                                let pairs =
                                    routing::compute_link_pairs(&state.graph, src, dst);
                                log::info!("Route input {} -> bus {:?}: src_node={}, dst_node={}, pairs={}", sid.0, bid, src, dst, pairs.len());
                                for (out_port, in_port) in pairs {
                                    let _ = sender.send(PwCommand::CreateLink {
                                        output_port_id: out_port,
                                        input_port_id: in_port,
                                    });
                                }
                            } else {
                                let link_ids =
                                    routing::find_links_between(&state.graph, src, dst);
                                for link_id in link_ids {
                                    let _ = sender.send(PwCommand::DestroyLink { link_id });
                                }
                            }
                        } else {
                            log::warn!("Route input {} -> bus {:?}: failed to resolve nodes (src={:?}, dst={:?})", sid.0, bid, source_node_id, dest_node_id);
                        }
                        drop(state);

                        let mut state = state_ref.borrow_mut();
                        if let Some(strip) =
                            state.config.strips.iter_mut().find(|s| s.id == sid)
                        {
                            if active {
                                strip.routed_to.insert(bid);
                            } else {
                                strip.routed_to.remove(&bid);
                            }
                        }
                    });
                }
            }
            StripRole::Bus { bus_id } => {
                let bid = *bus_id;
                for (key, btn) in buttons {
                    let sender = pw_sender.clone();
                    let state_ref = state.clone();
                    let out_node_name = key.clone();
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
                                let pairs = routing::compute_link_pairs(
                                    &state.graph, bus_nid, output_nid,
                                );
                                for (out_port, in_port) in pairs {
                                    let _ = sender.send(PwCommand::CreateLink {
                                        output_port_id: out_port,
                                        input_port_id: in_port,
                                    });
                                }
                            } else {
                                let link_ids = routing::find_links_between(
                                    &state.graph, bus_nid, output_nid,
                                );
                                for link_id in link_ids {
                                    let _ = sender.send(PwCommand::DestroyLink { link_id });
                                }
                            }
                            drop(state);

                            let mut state = state_ref.borrow_mut();
                            if let Some(bus) =
                                state.config.buses.iter_mut().find(|b| b.id == bid)
                            {
                                if active {
                                    bus.output_targets.insert(out_node_name.clone());
                                } else {
                                    bus.output_targets.remove(&out_node_name);
                                }
                            }
                        }
                    });
                }
            }
            StripRole::HardwareOutput { .. } | StripRole::VirtualOutput { .. } => {}
        }
    }

    fn connect_delete_handler(
        role: &StripRole,
        button: &gtk::Button,
        pw_sender: &pipewire::channel::Sender<PwCommand>,
        state: &Rc<RefCell<AppState>>,
    ) {
        let sender = pw_sender.clone();
        let state_ref = state.clone();
        let role = role.clone();
        button.connect_clicked(move |_| {
            let mut state = state_ref.borrow_mut();
            match &role {
                StripRole::Input { strip_id } => {
                    if let Some(pos) = state.config.strips.iter().position(|s| s.id == *strip_id) {
                        let strip = state.config.strips.remove(pos);
                        if let StripKind::VirtualInput { ref name } = strip.kind {
                            let _ = sender.send(PwCommand::DestroyVirtualInput {
                                strip_id: *strip_id,
                            });
                            log::info!("Deleted virtual input: {}", name);
                        }
                    }
                }
                StripRole::Bus { bus_id } => {
                    if let Some(pos) = state.config.buses.iter().position(|b| b.id == *bus_id) {
                        state.config.buses.remove(pos);
                        for strip in &mut state.config.strips {
                            strip.routed_to.remove(bus_id);
                        }
                        let _ = sender.send(PwCommand::DestroyBus { bus_id: *bus_id });
                        log::info!("Deleted bus: {:?}", bus_id);
                    }
                }
                StripRole::VirtualOutput { voutput_id } => {
                    if let Some(pos) = state.config.strips.iter().position(|s| {
                        matches!(&s.kind, StripKind::VirtualOutput { voutput_id: vid, .. } if *vid == *voutput_id)
                    }) {
                        state.config.strips.remove(pos);
                        let _ = sender.send(PwCommand::DestroyVirtualOutput {
                            voutput_id: *voutput_id,
                        });
                        log::info!("Deleted virtual output: {:?}", voutput_id);
                    }
                }
                StripRole::HardwareOutput { .. } => {}
            }
        });
    }

    pub fn update_volume(&self, volumes: &[f32]) {
        if let Some(&v) = volumes.first() {
            self.updating.set(true);
            let percent = v as f64 * 100.0;
            self.volume_scale.set_value(percent);
            self.updating.set(false);
        }
    }

    pub fn update_mute(&self, muted: bool) {
        self.updating.set(true);
        // Switch ON = unmuted, OFF = muted
        self.mute_switch.set_active(!muted);
        self.updating.set(false);
    }

    pub fn update_level(&self, peaks: &[f32]) {
        if let Some(&v) = peaks.first() {
            let prev = self.level_bar.value();
            let smoothed = (v as f64 * PEAK_SMOOTHING) + (prev * (1.0 - PEAK_SMOOTHING));
            self.level_bar.set_value(smoothed.clamp(0.0, 1.0));
        }
    }
}

fn apply_rename(role: &StripRole, new_name: &str, state: &Rc<RefCell<AppState>>) {
    let mut state = state.borrow_mut();
    match role {
        StripRole::Input { strip_id } => {
            if let Some(strip) = state.config.strips.iter_mut().find(|s| s.id == *strip_id) {
                match &mut strip.kind {
                    StripKind::VirtualInput { name } => *name = new_name.to_string(),
                    StripKind::VirtualOutput { name, .. } => *name = new_name.to_string(),
                    StripKind::HardwareInput { .. } => {}
                }
            }
        }
        StripRole::Bus { bus_id } => {
            if let Some(bus) = state.config.buses.iter_mut().find(|b| b.id == *bus_id) {
                bus.name = new_name.to_string();
            }
        }
        StripRole::VirtualOutput { voutput_id } => {
            if let Some(strip) = state.config.strips.iter_mut().find(|s| {
                matches!(&s.kind, StripKind::VirtualOutput { voutput_id: vid, .. } if *vid == *voutput_id)
            }) {
                if let StripKind::VirtualOutput { name, .. } = &mut strip.kind {
                    *name = new_name.to_string();
                }
            }
        }
        StripRole::HardwareOutput { .. } => {}
    }
}

fn resolve_node_and_channels(role: &StripRole, state: &AppState, pre_resolved: Option<u32>) -> (Option<u32>, usize) {
    match role {
        StripRole::Input { strip_id } => {
            let node_id = state.strip_node_id(*strip_id).or_else(|| {
                let strip = state.config.strips.iter().find(|s| s.id == *strip_id)?;
                match &strip.kind {
                    StripKind::HardwareInput { node_name } => {
                        state.find_hardware_node_by_name(node_name)
                    }
                    StripKind::VirtualInput { .. } => state.strip_node_id(*strip_id),
                    StripKind::VirtualOutput { .. } => None,
                }
            }).or(pre_resolved);
            let ch_count = state
                .config
                .strips
                .iter()
                .find(|s| s.id == *strip_id)
                .map(|s| match s.channels {
                    crate::pw::message::ChannelLayout::Mono => 1,
                    crate::pw::message::ChannelLayout::Stereo => 2,
                })
                .unwrap_or(2);
            (node_id, ch_count)
        }
        StripRole::Bus { bus_id } => {
            let node_id = state.bus_node_id(*bus_id);
            let ch_count = state
                .config
                .buses
                .iter()
                .find(|b| b.id == *bus_id)
                .map(|b| match b.channels {
                    crate::pw::message::ChannelLayout::Mono => 1,
                    crate::pw::message::ChannelLayout::Stereo => 2,
                })
                .unwrap_or(2);
            (node_id, ch_count)
        }
        StripRole::HardwareOutput { node_name } => {
            let node_id = state.find_hardware_node_by_name(node_name);
            (node_id, 2)
        }
        StripRole::VirtualOutput { voutput_id } => {
            let node_id = state.voutput_node_id(*voutput_id);
            (node_id, 2)
        }
    }
}
