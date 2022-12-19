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
    let show_relays_window_action = gio::SimpleAction::new("show_relays_window", None);
    show_relays_window_action.connect_activate(
        glib::clone!(@weak app => move |_action, _parameter| {
            show_relays_window(&app);
        }),
    );
    app.add_action(&show_relays_window_action);

    let show_settings_window_action = gio::SimpleAction::new("show_settings_window", None);
    show_settings_window_action.connect_activate(
        glib::clone!(@weak app => move |_action, _parameter| {
            show_settings_window(&app);
        }),
    );
    app.add_action(&show_settings_window_action);

    let quit_action = gio::SimpleAction::new("quit", None);
    quit_action.connect_activate(glib::clone!(@weak app => move |_action, _parameter| {
        app.quit();
    }));
    app.add_action(&quit_action);

    let show_identities_window_action = gio::SimpleAction::new("show_identities_window", None);
    show_identities_window_action.connect_activate(
        glib::clone!(@weak app => move |_action, _parameter| {
            show_identities_window(&app);
        }),
    );
    app.add_action(&show_identities_window_action);

    let show_following_window_action = gio::SimpleAction::new("show_following_window", None);
    show_following_window_action.connect_activate(
        glib::clone!(@weak app => move |_action, _parameter| {
            show_following_window(&app);
        }),
    );
    app.add_action(&show_following_window_action);

    let show_stats_window_action = gio::SimpleAction::new("show_stats_window", None);
    show_stats_window_action.connect_activate(
        glib::clone!(@weak app => move |_action, _parameter| {
            show_stats_window(&app);
        }),
    );
    app.add_action(&show_stats_window_action);

    let show_about_window_action = gio::SimpleAction::new("show_about_window", None);
    show_about_window_action.connect_activate(
        glib::clone!(@weak app => move |_action, _parameter| {
            show_about_window(&app);
        }),
    );
    app.add_action(&show_about_window_action);
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
            let relays_menu_item =
                gio::MenuItem::new(Some("Relays"), Some("app.show_relays_window"));
            let settings_menu_item =
                gio::MenuItem::new(Some("Settings"), Some("app.show_settings_window"));
            let quit_menu_item = gio::MenuItem::new(Some("Quit"), Some("app.quit"));

            let main_menu = gio::Menu::new();
            main_menu.append_item(&relays_menu_item);
            main_menu.append_item(&settings_menu_item);
            main_menu.append_item(&quit_menu_item);
            main_menu
        };

        // People Menu
        let people_menu = {
            let identities_menu_item =
                gio::MenuItem::new(Some("Your Identities"), Some("app.show_identities_window"));
            let following_menu_item =
                gio::MenuItem::new(Some("Following"), Some("app.show_following_window"));

            let people_menu = gio::Menu::new();
            people_menu.append_item(&identities_menu_item);
            people_menu.append_item(&following_menu_item);
            people_menu
        };

        // Help menu
        let help_menu = {
            let stats_menu_item =
                gio::MenuItem::new(Some("Statistics"), Some("app.show_stats_window"));
            let about_menu_item = gio::MenuItem::new(Some("About"), Some("app.show_about_window"));

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

fn show_relays_window(_app: &Application) {
    let relays_window = Window::builder()
        .decorated(true)
        .title("Gossip: Relays")
        .default_width(400)
        .default_height(600)
        .resizable(true)
        .build();

    relays_window.show();
}

fn show_settings_window(_app: &Application) {
    let settings_window = Window::builder()
        .decorated(true)
        .title("Gossip: Settings")
        .default_width(400)
        .default_height(600)
        .resizable(true)
        .build();

    settings_window.show();
}

fn show_identities_window(_app: &Application) {
    let identities_window = Window::builder()
        .decorated(true)
        .title("Gossip: Identities")
        .default_width(400)
        .default_height(600)
        .resizable(true)
        .build();

    identities_window.show();
}

fn show_following_window(_app: &Application) {
    let following_window = Window::builder()
        .decorated(true)
        .title("Gossip: Following")
        .default_width(400)
        .default_height(600)
        .resizable(true)
        .build();

    following_window.show();
}

fn show_stats_window(_app: &Application) {
    let stats_window = Window::builder()
        .decorated(true)
        .title("Gossip: Stats")
        .default_width(400)
        .default_height(600)
        .resizable(true)
        .build();

    stats_window.show();
}

fn show_about_window(_app: &Application) {
    let about_window = Window::builder()
        .decorated(true)
        .title("Gossip: About")
        .default_width(400)
        .default_height(600)
        .resizable(true)
        .build();

    about_window.show();
}
