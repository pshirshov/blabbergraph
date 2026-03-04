use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;

use crate::model::config::AppConfig;
use crate::model::state::AppState;
use crate::pw::message::PwCommand;

use super::matrix_view::MatrixView;
use super::mixer_view::MixerView;

pub struct BlabbergraphWindow {
    pub window: adw::ApplicationWindow,
    pub mixer_view: Rc<RefCell<MixerView>>,
    pub matrix_view: Rc<RefCell<MatrixView>>,
    pub view_stack: adw::ViewStack,
}

impl BlabbergraphWindow {
    pub fn new(
        app: &adw::Application,
        pw_sender: &pipewire::channel::Sender<PwCommand>,
        state: &Rc<RefCell<AppState>>,
    ) -> Self {
        let mixer_view = MixerView::new(pw_sender, state);
        let matrix_view = MatrixView::new();

        let view_stack = adw::ViewStack::new();
        let mixer_page = view_stack.add_titled(&mixer_view.container, Some("mixer"), "Mixer");
        mixer_page.set_icon_name(Some("audio-volume-high-symbolic"));

        let matrix_page = view_stack.add_titled(&matrix_view.container, Some("matrix"), "Matrix");
        matrix_page.set_icon_name(Some("view-grid-symbolic"));

        let view_switcher = adw::ViewSwitcher::new();
        view_switcher.set_stack(Some(&view_stack));
        view_switcher.set_policy(adw::ViewSwitcherPolicy::Wide);

        let header_bar = adw::HeaderBar::new();
        header_bar.set_title_widget(Some(&view_switcher));

        // Hamburger menu
        let menu = gio::Menu::new();
        menu.append(Some("Drop config"), Some("app.drop-config"));
        menu.append(Some("Quit"), Some("app.quit"));

        let menu_button = gtk::MenuButton::new();
        menu_button.set_icon_name("open-menu-symbolic");
        menu_button.set_menu_model(Some(&menu));
        header_bar.pack_end(&menu_button);

        // Register actions
        let drop_config_action = gio::SimpleAction::new("drop-config", None);
        {
            let app_ref = app.clone();
            drop_config_action.connect_activate(move |_, _| {
                if let Err(e) = AppConfig::delete() {
                    log::error!("Failed to delete config: {}", e);
                }
                log::info!("Config dropped, quitting");
                app_ref.quit();
            });
        }
        app.add_action(&drop_config_action);

        let quit_action = gio::SimpleAction::new("quit", None);
        {
            let app_ref = app.clone();
            quit_action.connect_activate(move |_, _| {
                app_ref.quit();
            });
        }
        app.add_action(&quit_action);

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header_bar);
        toolbar_view.set_content(Some(&view_stack));

        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Blabbergraph")
            .default_width(900)
            .default_height(600)
            .content(&toolbar_view)
            .build();

        window.set_hide_on_close(true);

        let mixer_view = Rc::new(RefCell::new(mixer_view));
        let matrix_view = Rc::new(RefCell::new(matrix_view));

        Self {
            window,
            mixer_view,
            matrix_view,
            view_stack,
        }
    }
}
