use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;

use pipewire::channel::Receiver;
use pipewire::context::Context;
use pipewire::main_loop::MainLoop;
use pipewire::registry::Registry;

use super::message::{PwCommand, PwEvent};
use super::registry::ProxyStore;
use super::{link_factory, node_factory, registry, volume};

pub fn run(pw_receiver: Receiver<PwCommand>, event_tx: mpsc::Sender<PwEvent>) {
    let mainloop = MainLoop::new(None).expect("Failed to create PipeWire MainLoop");
    let context = Context::new(&mainloop).expect("Failed to create PipeWire Context");
    let core = context.connect(None).expect("Failed to connect to PipeWire");
    let pw_registry = Rc::new(core.get_registry().expect("Failed to get PipeWire Registry"));

    let proxies: Rc<RefCell<ProxyStore>> = Rc::new(RefCell::new(ProxyStore::new()));

    let _reg_listener =
        registry::setup_registry_listener(pw_registry.clone(), &event_tx, proxies.clone());

    let _core_listener = core
        .add_listener_local()
        .error(|id, seq, res, msg| {
            log::error!("PipeWire error: id={} seq={} res={}: {}", id, seq, res, msg);
        })
        .register();

    let _receiver = pw_receiver.attach(mainloop.loop_(), {
        let mainloop = mainloop.clone();
        let pw_registry = pw_registry.clone();
        let proxies = proxies.clone();
        let event_tx = event_tx.clone();
        move |command| {
            handle_command(
                &mainloop,
                &core,
                &pw_registry,
                &proxies,
                &event_tx,
                command,
            );
        }
    });

    mainloop.run();
    log::info!("PipeWire thread exiting");
}

fn handle_command(
    mainloop: &MainLoop,
    core: &pipewire::core::Core,
    registry: &Registry,
    proxies: &Rc<RefCell<ProxyStore>>,
    event_tx: &mpsc::Sender<PwEvent>,
    command: PwCommand,
) {
    match command {
        PwCommand::CreateBus {
            bus_id,
            name,
            channels,
        } => {
            node_factory::create_bus(core, event_tx, proxies, bus_id, &name, channels);
        }
        PwCommand::DestroyBus { bus_id } => {
            node_factory::destroy_bus(core, proxies, bus_id);
        }
        PwCommand::CreateVirtualInput {
            strip_id,
            name,
            channels,
        } => {
            node_factory::create_virtual_input(
                core, event_tx, proxies, strip_id, &name, channels,
            );
        }
        PwCommand::DestroyVirtualInput { strip_id } => {
            node_factory::destroy_virtual_input(core, proxies, strip_id);
        }
        PwCommand::CreateLink {
            output_port_id,
            input_port_id,
        } => {
            link_factory::create_link(core, proxies, output_port_id, input_port_id);
        }
        PwCommand::DestroyLink { link_id } => {
            link_factory::destroy_link(registry, link_id);
        }
        PwCommand::SetVolume { node_id, volumes } => {
            volume::set_volume(proxies, node_id, &volumes);
        }
        PwCommand::SetMute { node_id, muted } => {
            volume::set_mute(proxies, node_id, muted);
        }
        PwCommand::Terminate => {
            mainloop.quit();
        }
    }
}
