use std::sync::mpsc;

use ksni;

pub enum TrayAction {
    ShowWindow,
    Quit,
}

#[derive(Debug)]
pub struct BlabbergraphTray {
    gtk_sender: mpsc::Sender<TrayAction>,
}

impl ksni::Tray for BlabbergraphTray {
    fn id(&self) -> String {
        "blabbergraph".into()
    }

    fn title(&self) -> String {
        "Blabbergraph".into()
    }

    fn icon_name(&self) -> String {
        "audio-volume-high".into()
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        vec![
            ksni::menu::StandardItem {
                label: "Show".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.gtk_sender.send(TrayAction::ShowWindow);
                }),
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
            ksni::menu::StandardItem {
                label: "Quit".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.gtk_sender.send(TrayAction::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

pub fn run_tray(gtk_sender: mpsc::Sender<TrayAction>) {
    let tray = BlabbergraphTray { gtk_sender };

    std::thread::spawn(move || {
        let service = ksni::TrayService::new(tray);
        let _ = service.run();
    });
}
