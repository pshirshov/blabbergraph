use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::mpsc;

use pipewire::link::{Link, LinkListener};
use pipewire::node::{Node, NodeListener};
use pipewire::registry::{GlobalObject, Listener, Registry};
use pipewire::spa::param::ParamType;
use pipewire::spa::pod::deserialize::PodDeserializer;
use pipewire::spa::pod::{Value, ValueArray};
use pipewire::spa::sys;
use pipewire::spa::utils::dict::DictRef;
use pipewire::types::ObjectType;

use super::message::{BusId, LinkState, PortDirection, PwEvent, StripId};

pub struct BoundNode {
    pub proxy: Node,
    pub _listener: NodeListener,
}

pub struct BoundLink {
    pub proxy: Link,
    pub _listener: LinkListener,
}

pub struct ProxyStore {
    pub nodes: HashMap<u32, BoundNode>,
    pub links: HashMap<u32, BoundLink>,
    pub bus_nodes: HashMap<BusId, u32>,
    pub virtual_input_nodes: HashMap<StripId, u32>,
    pub created_nodes: Vec<(Node, NodeListener)>,
    pub created_links: Vec<(Link, LinkListener)>,
}

impl ProxyStore {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            links: HashMap::new(),
            bus_nodes: HashMap::new(),
            virtual_input_nodes: HashMap::new(),
            created_nodes: Vec::new(),
            created_links: Vec::new(),
        }
    }
}

pub fn setup_registry_listener(
    registry: Rc<Registry>,
    event_tx: &mpsc::Sender<PwEvent>,
    proxies: Rc<RefCell<ProxyStore>>,
) -> Listener {
    let listener = registry
        .add_listener_local()
        .global({
            let registry = registry.clone();
            let event_tx = event_tx.clone();
            let proxies = proxies.clone();
            move |global: &GlobalObject<&DictRef>| {
                handle_global(&registry, &event_tx, &proxies, global);
            }
        })
        .global_remove({
            let event_tx = event_tx.clone();
            let proxies = proxies.clone();
            move |id: u32| {
                handle_global_remove(&event_tx, &proxies, id);
            }
        })
        .register();

    listener
}

fn handle_global(
    registry: &Registry,
    event_tx: &mpsc::Sender<PwEvent>,
    proxies: &Rc<RefCell<ProxyStore>>,
    global: &GlobalObject<&DictRef>,
) {
    let props = global.props.as_ref();

    match global.type_ {
        ObjectType::Node => {
            let name = props
                .and_then(|p| p.get("node.name"))
                .unwrap_or("")
                .to_string();
            let description = props
                .and_then(|p| {
                    p.get("node.description")
                        .or_else(|| p.get("node.nick"))
                        .or_else(|| p.get("node.name"))
                })
                .unwrap_or("")
                .to_string();
            let media_class = props
                .and_then(|p| p.get("media.class"))
                .unwrap_or("")
                .to_string();

            let mut properties = HashMap::new();
            if let Some(p) = props {
                for item in p.iter() {
                    properties.insert(item.0.to_string(), item.1.to_string());
                }
            }

            let _ = event_tx.send(PwEvent::NodeAdded {
                id: global.id,
                name,
                media_class,
                description,
                properties,
            });

            bind_node(registry, event_tx, proxies, global);
        }
        ObjectType::Port => {
            let node_id = props
                .and_then(|p| p.get("node.id"))
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);
            let name = props
                .and_then(|p| p.get("port.name"))
                .unwrap_or("")
                .to_string();
            let direction = match props.and_then(|p| p.get("port.direction")) {
                Some("in") => PortDirection::Input,
                Some("out") => PortDirection::Output,
                _ => return,
            };

            let _ = event_tx.send(PwEvent::PortAdded {
                id: global.id,
                node_id,
                name,
                direction,
            });
        }
        ObjectType::Link => {
            let output_port = props
                .and_then(|p| p.get("link.output.port"))
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);
            let input_port = props
                .and_then(|p| p.get("link.input.port"))
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);

            let _ = event_tx.send(PwEvent::LinkAdded {
                id: global.id,
                output_port_id: output_port,
                input_port_id: input_port,
                state: LinkState::Negotiating,
            });

            bind_link(registry, event_tx, proxies, global);
        }
        _ => {}
    }
}

fn bind_node(
    registry: &Registry,
    event_tx: &mpsc::Sender<PwEvent>,
    proxies: &Rc<RefCell<ProxyStore>>,
    global: &GlobalObject<&DictRef>,
) {
    let node: Node = match registry.bind(global) {
        Ok(n) => n,
        Err(e) => {
            log::warn!("Failed to bind node {}: {}", global.id, e);
            return;
        }
    };

    let sender = event_tx.clone();
    let node_id = global.id;

    let listener = node
        .add_listener_local()
        .param(move |_seq, param_id, _index, _next, param| {
            if param_id != ParamType::Props {
                return;
            }
            let Some(param) = param else { return };

            let (_, value) = match PodDeserializer::deserialize_any_from(param.as_bytes()) {
                Ok(v) => v,
                Err(_) => return,
            };

            if let Value::Object(obj) = value {
                let mut volumes = None;
                let mut muted = None;

                for prop in &obj.properties {
                    match prop.key {
                        sys::SPA_PROP_channelVolumes => {
                            if let Value::ValueArray(ValueArray::Float(ref v)) = prop.value {
                                volumes = Some(v.clone());
                            }
                        }
                        sys::SPA_PROP_mute => {
                            if let Value::Bool(m) = prop.value {
                                muted = Some(m);
                            }
                        }
                        _ => {}
                    }
                }

                if volumes.is_some() || muted.is_some() {
                    let _ = sender.send(PwEvent::ParamsChanged {
                        node_id,
                        volumes,
                        muted,
                    });
                }
            }
        })
        .register();

    node.subscribe_params(&[ParamType::Props]);

    proxies.borrow_mut().nodes.insert(
        global.id,
        BoundNode {
            proxy: node,
            _listener: listener,
        },
    );
}

fn bind_link(
    registry: &Registry,
    event_tx: &mpsc::Sender<PwEvent>,
    proxies: &Rc<RefCell<ProxyStore>>,
    global: &GlobalObject<&DictRef>,
) {
    let link: Link = match registry.bind(global) {
        Ok(l) => l,
        Err(e) => {
            log::warn!("Failed to bind link {}: {}", global.id, e);
            return;
        }
    };

    let sender = event_tx.clone();
    let link_id = global.id;

    let listener = link
        .add_listener_local()
        .info(move |info| {
            let state = match info.state() {
                pipewire::link::LinkState::Error(_) => LinkState::Error,
                pipewire::link::LinkState::Unlinked => LinkState::Unlinked,
                pipewire::link::LinkState::Init => LinkState::Negotiating,
                pipewire::link::LinkState::Negotiating => LinkState::Negotiating,
                pipewire::link::LinkState::Allocating => LinkState::Allocating,
                pipewire::link::LinkState::Paused => LinkState::Paused,
                pipewire::link::LinkState::Active => LinkState::Active,
            };
            let _ = sender.send(PwEvent::LinkStateChanged {
                id: link_id,
                state,
            });
        })
        .register();

    proxies.borrow_mut().links.insert(
        global.id,
        BoundLink {
            proxy: link,
            _listener: listener,
        },
    );
}

fn handle_global_remove(
    event_tx: &mpsc::Sender<PwEvent>,
    proxies: &Rc<RefCell<ProxyStore>>,
    id: u32,
) {
    let mut store = proxies.borrow_mut();
    if store.nodes.remove(&id).is_some() {
        let _ = event_tx.send(PwEvent::NodeRemoved { id });
    } else if store.links.remove(&id).is_some() {
        let _ = event_tx.send(PwEvent::LinkRemoved { id });
    } else {
        let _ = event_tx.send(PwEvent::PortRemoved { id });
    }
}
