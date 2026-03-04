use std::cell::RefCell;
use std::rc::Rc;

use pipewire::core::Core;
use pipewire::link::Link;
use pipewire::properties::properties;
use pipewire::registry::Registry;

use super::registry::ProxyStore;

pub fn create_link(
    core: &Core,
    proxies: &Rc<RefCell<ProxyStore>>,
    output_port_id: u32,
    input_port_id: u32,
) {
    let props = properties! {
        "link.output.port" => output_port_id.to_string(),
        "link.input.port" => input_port_id.to_string(),
        "object.linger" => "true",
    };

    match core.create_object::<Link>("link-factory", &props) {
        Ok(link) => {
            let listener = link.add_listener_local().register();
            proxies.borrow_mut().created_links.push((link, listener));
            log::info!(
                "Link creation requested: {} -> {}",
                output_port_id,
                input_port_id
            );
        }
        Err(e) => {
            log::error!(
                "Failed to create link {} -> {}: {}",
                output_port_id,
                input_port_id,
                e
            );
        }
    }
}

pub fn destroy_link(registry: &Registry, link_id: u32) {
    let result = registry.destroy_global(link_id);
    match result.into_result() {
        Ok(_) => {
            log::info!("Link {} destroyed", link_id);
        }
        Err(e) => {
            log::error!("Failed to destroy link {}: {}", link_id, e);
        }
    }
}
