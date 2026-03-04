use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;

use glib::prelude::ObjectExt;

use crate::model::state::AppState;
use crate::pw::graph::{LinkInfo, NodeInfo, PortInfo};
use crate::pw::message::{PwCommand, PwEvent};

use super::window::BlabbergraphWindow;

pub fn setup_event_handler(
    event_rx: mpsc::Receiver<PwEvent>,
    pw_sender: pipewire::channel::Sender<PwCommand>,
    win: &BlabbergraphWindow,
    state: &Rc<RefCell<AppState>>,
) {
    let mixer_view = win.mixer_view.clone();
    let matrix_view = win.matrix_view.clone();
    let state_ref = state.clone();
    let pw_sender_ref = pw_sender.clone();

    let rebuild_mixer_sender = pw_sender.clone();
    let rebuild_mixer_state = state.clone();
    let rebuild_mixer_view = win.mixer_view.clone();

    let rebuild_matrix_sender = pw_sender.clone();
    let rebuild_matrix_state = state.clone();
    let rebuild_matrix_view = win.matrix_view.clone();

    let debounce_rebuild: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));

    // Poll the mpsc receiver from a GLib timer at ~60fps
    glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
        while let Ok(event) = event_rx.try_recv() {
            let mut state = state_ref.borrow_mut();
            let needs_rebuild = process_event(
                &event,
                &mut state,
                &mixer_view,
                &matrix_view,
                &pw_sender_ref,
            );
            drop(state);

            if needs_rebuild {
                schedule_rebuild(
                    &debounce_rebuild,
                    &rebuild_mixer_sender,
                    &rebuild_mixer_state,
                    &rebuild_mixer_view,
                    &rebuild_matrix_sender,
                    &rebuild_matrix_state,
                    &rebuild_matrix_view,
                );
            }
        }
        glib::ControlFlow::Continue
    });

    // Rebuild matrix when the Matrix tab becomes visible
    {
        let matrix_sender = pw_sender.clone();
        let matrix_state = state.clone();
        let matrix_view = win.matrix_view.clone();
        let mixer_sender = pw_sender.clone();
        let mixer_state = state.clone();
        let mixer_view = win.mixer_view.clone();
        win.view_stack
            .connect_notify_local(Some("visible-child-name"), move |stack: &adw::ViewStack, _| {
                if let Some(name) = stack.visible_child_name() {
                    if name == "matrix" {
                        log::debug!("Matrix tab activated, rebuilding");
                        matrix_view
                            .borrow_mut()
                            .rebuild(&matrix_sender, &matrix_state);
                    } else if name == "mixer" {
                        log::debug!("Mixer tab activated, rebuilding");
                        mixer_view
                            .borrow_mut()
                            .rebuild(&mixer_sender, &mixer_state);
                    }
                }
            });
    }
}

fn process_event(
    event: &PwEvent,
    state: &mut AppState,
    mixer_view: &Rc<RefCell<super::mixer_view::MixerView>>,
    matrix_view: &Rc<RefCell<super::matrix_view::MatrixView>>,
    pw_sender: &pipewire::channel::Sender<PwCommand>,
) -> bool {
    match event {
        PwEvent::NodeAdded {
            id,
            name,
            media_class,
            description,
            properties,
        } => {
            state.graph.add_node(NodeInfo {
                id: *id,
                name: name.clone(),
                description: description.clone(),
                media_class: media_class.clone(),
                volumes: vec![1.0, 1.0],
                muted: false,
                properties: properties.clone(),
            });
            true
        }
        PwEvent::NodeRemoved { id } => {
            state.graph.remove_node(*id);
            true
        }
        PwEvent::PortAdded {
            id,
            node_id,
            name,
            direction,
        } => {
            state.graph.add_port(PortInfo {
                id: *id,
                node_id: *node_id,
                name: name.clone(),
                direction: *direction,
            });
            true
        }
        PwEvent::PortRemoved { id } => {
            state.graph.remove_port(*id);
            true
        }
        PwEvent::LinkAdded {
            id,
            output_port_id,
            input_port_id,
            state: link_state,
        } => {
            state.graph.add_link(LinkInfo {
                id: *id,
                output_port_id: *output_port_id,
                input_port_id: *input_port_id,
                state: *link_state,
            });
            let mat = matrix_view.borrow();
            let updated = mat.update_link_added(*output_port_id, *input_port_id);
            !updated
        }
        PwEvent::LinkStateChanged {
            id,
            state: new_state,
        } => {
            if let Some(link) = state.graph.links.get_mut(id) {
                link.state = *new_state;
            }
            false
        }
        PwEvent::LinkRemoved { id } => {
            let updated = if let Some(link) = state.graph.links.get(id).cloned() {
                let mat = matrix_view.borrow();
                mat.update_link_removed(state, link.output_port_id, link.input_port_id)
            } else {
                false
            };
            state.graph.remove_link(*id);
            !updated
        }
        PwEvent::ParamsChanged {
            node_id,
            volumes,
            muted,
        } => {
            if let Some(node) = state.graph.nodes.get_mut(node_id) {
                if let Some(ref v) = volumes {
                    node.volumes = v.clone();
                }
                if let Some(m) = muted {
                    node.muted = *m;
                }
            }
            let mixer = mixer_view.borrow();
            mixer.update_node_params(state, *node_id);
            false
        }
        PwEvent::BusCreated { bus_id, node_id } => {
            state.register_bus(*bus_id, *node_id);
            log::info!("Bus registered: {:?} -> node {}", bus_id, node_id);
            check_and_restore_routing(state, pw_sender);
            true
        }
        PwEvent::VirtualInputCreated { strip_id, node_id } => {
            state.register_strip(*strip_id, *node_id);
            log::info!(
                "Virtual input registered: {:?} -> node {}",
                strip_id,
                node_id
            );
            check_and_restore_routing(state, pw_sender);
            true
        }
    }
}

fn schedule_rebuild(
    debounce: &Rc<RefCell<Option<glib::SourceId>>>,
    mixer_sender: &pipewire::channel::Sender<PwCommand>,
    mixer_state: &Rc<RefCell<AppState>>,
    mixer_view: &Rc<RefCell<super::mixer_view::MixerView>>,
    matrix_sender: &pipewire::channel::Sender<PwCommand>,
    matrix_state: &Rc<RefCell<AppState>>,
    matrix_view: &Rc<RefCell<super::matrix_view::MatrixView>>,
) {
    let mut current = debounce.borrow_mut();
    if let Some(id) = current.take() {
        id.remove();
    }

    let mixer_sender = mixer_sender.clone();
    let mixer_state = mixer_state.clone();
    let mixer_view = mixer_view.clone();
    let matrix_sender = matrix_sender.clone();
    let matrix_state = matrix_state.clone();
    let matrix_view = matrix_view.clone();
    let debounce_clear = debounce.clone();

    let source_id = glib::timeout_add_local_once(std::time::Duration::from_millis(200), move || {
        debounce_clear.borrow_mut().take();

        mixer_view.borrow_mut().rebuild(&mixer_sender, &mixer_state);
        matrix_view
            .borrow_mut()
            .rebuild(&matrix_sender, &matrix_state);
    });

    *current = Some(source_id);
}

fn check_and_restore_routing(
    state: &mut AppState,
    pw_sender: &pipewire::channel::Sender<PwCommand>,
) {
    if !state.restore_pending {
        return;
    }

    let all_buses_ready = state
        .config
        .buses
        .iter()
        .all(|b| state.bus_node_map.contains_key(&b.id));
    let all_vinputs_ready = state.config.strips.iter().all(|s| {
        if s.is_virtual() {
            state.strip_node_map.contains_key(&s.id)
        } else {
            true
        }
    });

    if !all_buses_ready || !all_vinputs_ready {
        return;
    }

    log::info!("All buses and virtual inputs ready, restoring routing...");
    state.restore_pending = false;

    // Destroy stale blabbergraph nodes from previous runs (lingering nodes)
    if !state.cleanup_done {
        state.cleanup_done = true;
        let stale_ids: Vec<u32> = state
            .graph
            .nodes
            .values()
            .filter(|n| n.name.starts_with("blabbergraph."))
            .filter(|n| {
                !state.bus_node_map.values().any(|&nid| nid == n.id)
                    && !state.strip_node_map.values().any(|&nid| nid == n.id)
            })
            .map(|n| n.id)
            .collect();
        for id in stale_ids {
            log::info!("Destroying stale blabbergraph node {}", id);
            let _ = pw_sender.send(PwCommand::DestroyGlobal { id });
        }
    }

    for bus in &state.config.buses {
        if let Some(nid) = state.bus_node_id(bus.id) {
            let _ = pw_sender.send(PwCommand::SetVolume {
                node_id: nid,
                volumes: bus.volume.clone(),
            });
            let _ = pw_sender.send(PwCommand::SetMute {
                node_id: nid,
                muted: bus.muted,
            });
        }
    }

    for strip in state.config.strips.clone() {
        let node_id = if strip.is_virtual() {
            state.strip_node_id(strip.id)
        } else {
            match &strip.kind {
                crate::model::strip::StripKind::HardwareInput { node_name } => {
                    state.find_hardware_node_by_name(node_name)
                }
                _ => None,
            }
        };

        if let Some(nid) = node_id {
            let _ = pw_sender.send(PwCommand::SetVolume {
                node_id: nid,
                volumes: strip.volume.clone(),
            });
            let _ = pw_sender.send(PwCommand::SetMute {
                node_id: nid,
                muted: strip.muted,
            });

            for &bus_id in &strip.routed_to {
                if let Some(bus_nid) = state.bus_node_id(bus_id) {
                    let pairs =
                        crate::model::routing::compute_link_pairs(&state.graph, nid, bus_nid);
                    for (out_port, in_port) in pairs {
                        let _ = pw_sender.send(PwCommand::CreateLink {
                            output_port_id: out_port,
                            input_port_id: in_port,
                        });
                    }
                }
            }
        }
    }

    // Restore bus → output routing (multiple outputs per bus)
    for bus in state.config.buses.clone() {
        for target_name in &bus.output_targets {
            if let (Some(bus_nid), Some(target_nid)) = (
                state.bus_node_id(bus.id),
                state.find_hardware_node_by_name(target_name),
            ) {
                let pairs =
                    crate::model::routing::compute_link_pairs(&state.graph, bus_nid, target_nid);
                for (out_port, in_port) in pairs {
                    let _ = pw_sender.send(PwCommand::CreateLink {
                        output_port_id: out_port,
                        input_port_id: in_port,
                    });
                }
            }
        }
    }
}
