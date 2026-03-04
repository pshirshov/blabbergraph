use std::cell::RefCell;
use std::rc::Rc;

use pipewire::spa::param::ParamType;
use pipewire::spa::pod::serialize::PodSerializer;
use pipewire::spa::pod::{self, PropertyFlags, Value, ValueArray};
use pipewire::spa::sys;

use super::registry::ProxyStore;

pub fn set_volume(proxies: &Rc<RefCell<ProxyStore>>, node_id: u32, volumes: &[f32]) {
    let store = proxies.borrow();
    let Some(bound) = store.nodes.get(&node_id) else {
        log::warn!("set_volume: node {} not found in proxy store", node_id);
        return;
    };

    let values = pod::Value::Object(pod::Object {
        type_: sys::SPA_TYPE_OBJECT_Props,
        id: sys::SPA_PARAM_Props,
        properties: vec![pod::Property {
            key: sys::SPA_PROP_channelVolumes,
            flags: PropertyFlags::empty(),
            value: Value::ValueArray(ValueArray::Float(volumes.to_vec())),
        }],
    });

    let mut buf = vec![0u8; 1024];
    let result = PodSerializer::serialize(std::io::Cursor::new(&mut buf[..]), &values);
    match result {
        Ok((_, _)) => {
            let pod = pod::Pod::from_bytes(&buf);
            match pod {
                Some(pod) => {
                    bound.proxy.set_param(ParamType::Props, 0, pod);
                }
                None => {
                    log::error!("set_volume: failed to parse serialized pod");
                }
            }
        }
        Err(e) => {
            log::error!("set_volume: serialization failed: {:?}", e);
        }
    }
}

pub fn set_mute(proxies: &Rc<RefCell<ProxyStore>>, node_id: u32, muted: bool) {
    let store = proxies.borrow();
    let Some(bound) = store.nodes.get(&node_id) else {
        log::warn!("set_mute: node {} not found in proxy store", node_id);
        return;
    };

    let values = pod::Value::Object(pod::Object {
        type_: sys::SPA_TYPE_OBJECT_Props,
        id: sys::SPA_PARAM_Props,
        properties: vec![pod::Property {
            key: sys::SPA_PROP_mute,
            flags: PropertyFlags::empty(),
            value: Value::Bool(muted),
        }],
    });

    let mut buf = vec![0u8; 1024];
    let result = PodSerializer::serialize(std::io::Cursor::new(&mut buf[..]), &values);
    match result {
        Ok((_, _)) => {
            let pod = pod::Pod::from_bytes(&buf);
            match pod {
                Some(pod) => {
                    bound.proxy.set_param(ParamType::Props, 0, pod);
                }
                None => {
                    log::error!("set_mute: failed to parse serialized pod");
                }
            }
        }
        Err(e) => {
            log::error!("set_mute: serialization failed: {:?}", e);
        }
    }
}
