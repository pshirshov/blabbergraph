use gtk::prelude::*;
use gtk::{self, Align, Orientation};

use crate::pw::message::PwCommand;

pub struct OutputWidget {
    pub container: gtk::Box,
    pub node_name: String,
    pub volume_scale: gtk::Scale,
    pub mute_button: gtk::ToggleButton,
    pub db_label: gtk::Label,
}

impl OutputWidget {
    pub fn new(
        node_name: &str,
        display_name: &str,
        node_id: u32,
        pw_sender: &pipewire::channel::Sender<PwCommand>,
    ) -> Self {
        let container = gtk::Box::new(Orientation::Vertical, 4);
        container.set_width_request(100);
        container.set_margin_start(4);
        container.set_margin_end(4);
        container.set_margin_top(8);
        container.set_margin_bottom(8);
        container.add_css_class("card");
        container.set_vexpand(true);

        let name_label = gtk::Label::new(Some(display_name));
        name_label.add_css_class("heading");
        name_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        name_label.set_max_width_chars(12);
        name_label.set_halign(Align::Center);
        container.append(&name_label);

        let volume_scale = gtk::Scale::with_range(Orientation::Vertical, 0.0, 150.0, 1.0);
        volume_scale.set_inverted(true);
        volume_scale.set_value(100.0);
        volume_scale.set_vexpand(true);
        volume_scale.set_size_request(-1, 150);
        volume_scale.add_mark(0.0, gtk::PositionType::Right, Some("-inf"));
        volume_scale.add_mark(100.0, gtk::PositionType::Right, Some("0dB"));
        volume_scale.add_mark(150.0, gtk::PositionType::Right, Some("+6dB"));
        container.append(&volume_scale);

        let db_label = gtk::Label::new(Some("0.0 dB"));
        db_label.add_css_class("caption");
        db_label.set_halign(Align::Center);
        container.append(&db_label);

        let mute_button = gtk::ToggleButton::with_label("M");
        mute_button.set_halign(Align::Center);
        mute_button.add_css_class("destructive-action");
        mute_button.set_has_frame(true);
        container.append(&mute_button);

        // Volume handler
        {
            let sender = pw_sender.clone();
            let db_label_ref = db_label.clone();
            volume_scale.connect_value_changed(move |scale| {
                let percent = scale.value();
                let linear = percent / 100.0;
                let db = if linear > 0.0 {
                    20.0 * (linear as f64).log10()
                } else {
                    -f64::INFINITY
                };
                db_label_ref.set_text(&format!("{:.1} dB", db));
                let volumes = vec![linear as f32; 2];
                let _ = sender.send(PwCommand::SetVolume {
                    node_id,
                    volumes,
                });
            });
        }

        // Mute handler
        {
            let sender = pw_sender.clone();
            mute_button.connect_toggled(move |btn| {
                let muted = btn.is_active();
                let _ = sender.send(PwCommand::SetMute {
                    node_id,
                    muted,
                });
            });
        }

        Self {
            container,
            node_name: node_name.to_string(),
            volume_scale,
            mute_button,
            db_label,
        }
    }

    pub fn update_volume(&self, volumes: &[f32]) {
        if let Some(&v) = volumes.first() {
            let percent = v as f64 * 100.0;
            self.volume_scale.set_value(percent);
        }
    }

    pub fn update_mute(&self, muted: bool) {
        self.mute_button.set_active(muted);
    }
}
