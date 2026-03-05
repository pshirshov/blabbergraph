mod model;
mod pw;
mod tray;
mod ui;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;

use adw::prelude::*;

use model::config::AppConfig;
use model::state::AppState;
use model::strip::StripKind;
use pw::message::{PwCommand, PwEvent};
use tray::service::TrayAction;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let start_hidden = std::env::args().any(|a| a == "--minimized" || a == "--tray");

    pipewire::init();

    let (event_tx, event_rx) = mpsc::channel::<PwEvent>();
    let (pw_sender, pw_receiver) = pipewire::channel::channel::<PwCommand>();

    // Wrap in Option so we can take() it on first activate (connect_activate requires Fn, not FnOnce)
    let event_rx = Rc::new(RefCell::new(Some(event_rx)));

    let pw_thread = std::thread::spawn({
        let event_tx = event_tx.clone();
        move || {
            pw::thread::run(pw_receiver, event_tx);
        }
    });

    let app = adw::Application::builder()
        .application_id("com.github.blabbergraph")
        .build();

    let pw_sender_activate = pw_sender.clone();
    app.connect_activate(move |app| {
        // Only set up on first activate
        let rx = event_rx.borrow_mut().take();
        let Some(event_rx) = rx else {
            // Already activated, just present the window
            if let Some(win) = app.active_window() {
                win.present();
            }
            return;
        };

        let config = AppConfig::load().unwrap_or_else(|e| {
            log::error!("Failed to load config: {}", e);
            AppConfig::default()
        });

        let state = Rc::new(RefCell::new(AppState::new(config)));
        let win = ui::window::BlabbergraphWindow::new(app, &pw_sender_activate, &state);

        // Trigger initial bus/virtual-input creation
        {
            let state_borrow = state.borrow();
            for bus in &state_borrow.config.buses {
                let _ = pw_sender_activate.send(PwCommand::CreateBus {
                    bus_id: bus.id,
                    name: bus.name.clone(),
                    channels: bus.channels,
                });
            }
            for strip in &state_borrow.config.strips {
                match &strip.kind {
                    StripKind::VirtualInput { ref name } => {
                        let _ = pw_sender_activate.send(PwCommand::CreateVirtualInput {
                            strip_id: strip.id,
                            name: name.clone(),
                            channels: strip.channels,
                        });
                    }
                    StripKind::VirtualOutput {
                        voutput_id,
                        ref name,
                    } => {
                        let _ = pw_sender_activate.send(PwCommand::CreateVirtualOutput {
                            voutput_id: *voutput_id,
                            name: name.clone(),
                            channels: strip.channels,
                        });
                    }
                    StripKind::HardwareInput { .. } => {}
                }
            }
        }

        // Set up PipeWire event handler
        ui::app::setup_event_handler(event_rx, pw_sender_activate.clone(), &win, &state);

        // Set up tray
        let (tray_tx, tray_rx) = mpsc::channel::<TrayAction>();
        let window_for_tray = win.window.clone();
        let app_for_tray = app.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
            while let Ok(action) = tray_rx.try_recv() {
                match action {
                    TrayAction::ShowWindow => {
                        window_for_tray.present();
                    }
                    TrayAction::Quit => {
                        app_for_tray.quit();
                    }
                }
            }
            glib::ControlFlow::Continue
        });
        tray::service::run_tray(tray_tx);

        if !start_hidden {
            win.window.present();
        }

        // Save config on window close
        let state_save = state.clone();
        win.window.connect_close_request(move |_| {
            let s = state_save.borrow();
            if let Err(e) = s.config.save() {
                log::error!("Failed to save config on close: {}", e);
            }
            glib::Propagation::Proceed
        });
    });

    app.run();

    let _ = pw_sender.send(PwCommand::Terminate);
    let _ = pw_thread.join();
}
