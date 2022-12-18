use gtk::prelude::*;
use gtk::{gio, glib};
use gtk::{Application, ApplicationWindow};

const APP_ID: &str = "com.mikedilger.gossip";

pub fn run() {
    // Create a new application
    let app = Application::builder().application_id(APP_ID).build();

    // Connect signals
    app.connect_startup(configure_app);
    app.connect_activate(build_ui);
    app.connect_shutdown(|_| {
        log::info!("UI shutting down");
    });

    // Run the application
    app.run();

    // Initiate shutdown
    if let Err(e) = crate::initiate_shutdown() {
        log::error!("{}", e);
    }
}

fn configure_app(app: &Application) {
    let quit = gio::SimpleAction::new("quit", None);
    quit.connect_activate(glib::clone!(@weak app => move |_action, _parameter| {
        app.quit();
    }));
    app.add_action(&quit);

    let about = gio::SimpleAction::new("about", None);
    about.connect_activate(glib::clone!(@weak app => move |_action, _parameter| {
        log::info!("About was pressed");
    }));
    app.add_action(&about);
}

fn build_ui(app: &Application) {
    // Create a window and set the title
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Gossip")
        .build();

    let menubar = {
        let main_menu = {
            let relays_menu_item = gio::MenuItem::new(Some("Relays"), None);
            let settings_menu_item = gio::MenuItem::new(Some("Settings"), None);
            let quit_menu_item = gio::MenuItem::new(Some("Quit"), Some("app.quit"));

            let main_menu = gio::Menu::new();
            main_menu.append_item(&relays_menu_item);
            main_menu.append_item(&settings_menu_item);
            main_menu.append_item(&quit_menu_item);
            main_menu
        };

        // People Menu
        let people_menu = {
            let identities_menu_item = gio::MenuItem::new(Some("Your Identities"), None);
            let following_menu_item = gio::MenuItem::new(Some("Following"), None);

            let people_menu = gio::Menu::new();
            people_menu.append_item(&identities_menu_item);
            people_menu.append_item(&following_menu_item);
            people_menu
        };

        // Help menu
        let help_menu = {
            let stats_menu_item = gio::MenuItem::new(Some("Statistics"), None);
            let about_menu_item = gio::MenuItem::new(Some("About"), Some("app.about"));

            let help_menu = gio::Menu::new();
            help_menu.append_item(&stats_menu_item);
            help_menu.append_item(&about_menu_item);
            help_menu
        };

        // Menubar
        let menubar = gio::Menu::new();
        menubar.append_submenu(Some("Main"), &main_menu);
        menubar.append_submenu(Some("People"), &people_menu);
        menubar.append_submenu(Some("Help"), &help_menu);

        menubar
    };

    app.set_menubar(Some(&menubar));
    window.set_show_menubar(true);

    // Present window
    window.present();
}
