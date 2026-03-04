use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;

use pipewire::core::Core;
use pipewire::node::Node;
use pipewire::properties::properties;

use super::message::{BusId, ChannelLayout, PwEvent, StripId};
use super::registry::ProxyStore;

pub fn create_bus(
    core: &Core,
    event_tx: &mpsc::Sender<PwEvent>,
    proxies: &Rc<RefCell<ProxyStore>>,
    bus_id: BusId,
    name: &str,
    channels: ChannelLayout,
) {
    let position = match channels {
        ChannelLayout::Mono => "MONO",
        ChannelLayout::Stereo => "FL,FR",
    };

    let pw_name = format!("blabbergraph.bus.{}", name);

    let props = properties! {
        "factory.name" => "support.null-audio-sink",
        "node.name" => pw_name.as_str(),
        "node.description" => name,
        "media.class" => "Audio/Duplex",
        "audio.position" => position,
        "node.virtual" => "true",
        "monitor.channel-volumes" => "true",
    };

    match core.create_object::<Node>("adapter", &props) {
        Ok(node) => {
            let sender = event_tx.clone();
            let proxies_clone = proxies.clone();
            let listener = node
                .add_listener_local()
                .info(move |info| {
                    let node_id = info.id();
                    log::info!("Bus created: bus_id={:?} node_id={}", bus_id, node_id);
                    proxies_clone.borrow_mut().bus_nodes.insert(bus_id, node_id);
                    let _ = sender.send(PwEvent::BusCreated { bus_id, node_id });
                })
                .register();

            proxies.borrow_mut().created_nodes.push((node, listener));
        }
        Err(e) => {
            log::error!("Failed to create bus {:?}: {}", bus_id, e);
        }
    }
}

pub fn create_virtual_input(
    core: &Core,
    event_tx: &mpsc::Sender<PwEvent>,
    proxies: &Rc<RefCell<ProxyStore>>,
    strip_id: StripId,
    name: &str,
    channels: ChannelLayout,
) {
    let position = match channels {
        ChannelLayout::Mono => "MONO",
        ChannelLayout::Stereo => "FL,FR",
    };

    let pw_name = format!("blabbergraph.vinput.{}", name);

    let props = properties! {
        "factory.name" => "support.null-audio-sink",
        "node.name" => pw_name.as_str(),
        "node.description" => name,
        "media.class" => "Audio/Sink",
        "audio.position" => position,
        "node.virtual" => "true",
        "monitor.channel-volumes" => "true",
    };

    match core.create_object::<Node>("adapter", &props) {
        Ok(node) => {
            let sender = event_tx.clone();
            let proxies_clone = proxies.clone();
            let listener = node
                .add_listener_local()
                .info(move |info| {
                    let node_id = info.id();
                    log::info!(
                        "Virtual input created: strip_id={:?} node_id={}",
                        strip_id,
                        node_id
                    );
                    proxies_clone
                        .borrow_mut()
                        .virtual_input_nodes
                        .insert(strip_id, node_id);
                    let _ = sender.send(PwEvent::VirtualInputCreated { strip_id, node_id });
                })
                .register();

            proxies.borrow_mut().created_nodes.push((node, listener));
        }
        Err(e) => {
            log::error!("Failed to create virtual input {:?}: {}", strip_id, e);
        }
    }
}

pub fn destroy_bus(core: &Core, proxies: &Rc<RefCell<ProxyStore>>, bus_id: BusId) {
    let mut store = proxies.borrow_mut();
    if let Some(node_id) = store.bus_nodes.remove(&bus_id) {
        if let Some(bound) = store.nodes.remove(&node_id) {
            let _ = core.destroy_object(bound.proxy);
        }
    }
}

pub fn destroy_virtual_input(core: &Core, proxies: &Rc<RefCell<ProxyStore>>, strip_id: StripId) {
    let mut store = proxies.borrow_mut();
    if let Some(node_id) = store.virtual_input_nodes.remove(&strip_id) {
        if let Some(bound) = store.nodes.remove(&node_id) {
            let _ = core.destroy_object(bound.proxy);
        }
    }
}
