use gtk::prelude::*;
use gtk::{gio, glib};
use gtk::{
    Align, Application, ApplicationWindow, Box, Orientation, ScrolledWindow, Statusbar, Window,
};

const APP_ID: &str = "com.mikedilger.gossip";

pub fn run() {
    // Create a new application
    let app = Application::builder().application_id(APP_ID).build();

    // Connect startup to configure the app
    app.connect_startup(configure_app);

    // Connect activate to build (and show) the app window
    app.connect_activate(build_app_window);

    // Connect shutdown to initiate shutdown
    app.connect_shutdown(|_| {
        log::info!("UI shutting down");
        if let Err(e) = crate::initiate_shutdown() {
            log::error!("{}", e);
        }
    });

    app.run();
}

fn configure_app(app: &Application) {
    let quit = gio::SimpleAction::new("quit", None);
    quit.connect_activate(glib::clone!(@weak app => move |_action, _parameter| {
        app.quit();
    }));
    app.add_action(&quit);

    let about = gio::SimpleAction::new("about", None);
    about.connect_activate(glib::clone!(@weak app => move |_action, _parameter| {
        build_about_window(&app);
    }));
    app.add_action(&about);
}

fn build_app_window(app: &Application) {
    // Create a window and set the title
    let app_window = ApplicationWindow::builder()
        .application(app)
        .decorated(true)
        .default_width(700)
        .default_height(900)
        .resizable(true)
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
    app_window.set_show_menubar(true);

    let main_hbox = {
        let statusbar = Statusbar::builder().build();
        //let statusbar_context_id = statusbar.context_id("");

        let main_scrolled_window = ScrolledWindow::builder()
            .has_frame(true)
            .halign(Align::Fill)
            .hexpand(true)
            .valign(Align::Fill)
            .vexpand(true)
            .build();

        let main_hbox = Box::builder().orientation(Orientation::Vertical).build();

        main_hbox.append(&main_scrolled_window);
        main_hbox.append(&statusbar);

        main_hbox
    };

    app_window.set_child(Some(&main_hbox));

    // Present window
    app_window.present();
}

fn build_about_window(_app: &Application) {
    let about_window = Window::builder()
        .decorated(true)
        .title("Gossip: About")
        .default_width(400)
        .default_height(600)
        .resizable(true)
        .build();

    about_window.show();
}
