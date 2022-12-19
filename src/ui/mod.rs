pub mod about;
pub mod following;
pub mod identities;
pub mod post;
pub mod relays;
pub mod settings;
pub mod stats;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use gtk::{
    AboutDialog, Align, Application, ApplicationWindow, Box, Label, License,
    ListView, Orientation, PolicyType, ScrolledWindow, SignalListItemFactory, SingleSelection,
    Statusbar
};
use post::Post;

const APP_ID: &str = "com.mikedilger.gossip";

pub fn run() {
    // Create a new application
    let app = Application::builder().application_id(APP_ID).build();

    // Connect startup to configure the app
    app.connect_startup(configure_app);

    // Connect activate to build (and show) the app window
    app.connect_activate(build_ui);

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
            relays::show_window(&app);
        }),
    );
    app.add_action(&show_relays_window_action);

    let show_settings_window_action = gio::SimpleAction::new("show_settings_window", None);
    show_settings_window_action.connect_activate(
        glib::clone!(@weak app => move |_action, _parameter| {
            settings::show_window(&app);
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
            identities::show_window(&app);
        }),
    );
    app.add_action(&show_identities_window_action);

    let show_following_window_action = gio::SimpleAction::new("show_following_window", None);
    show_following_window_action.connect_activate(
        glib::clone!(@weak app => move |_action, _parameter| {
            following::show_window(&app);
        }),
    );
    app.add_action(&show_following_window_action);

    let show_stats_window_action = gio::SimpleAction::new("show_stats_window", None);
    show_stats_window_action.connect_activate(
        glib::clone!(@weak app => move |_action, _parameter| {
            stats::show_window(&app);
        }),
    );
    app.add_action(&show_stats_window_action);

    let show_about_window_action = gio::SimpleAction::new("show_about_window", None);
    show_about_window_action.connect_activate(
        glib::clone!(@weak app => move |_action, _parameter| {
            let about = crate::ui::about::about();

            let comments = format!(
                "{}

Nostr is a protocol and specification for storing and retrieving social media events onto servers called relays. Many users store their events onto multiple relays for reliability, censorship resistance, and to spread their reach. If you didn't store an event on a particular relay, don't expect anyone to find it there because relays normally don't share events with each other.

Users are defined by their keypair, and are known by the public key of that pair. All events they generate are signed by their private key, and verifiable by their public key.

Learn more about nostr at https://github.com/nostr-protocol/nostr
", about.description);

            let about_dialog = AboutDialog::builder()
                .program_name(&about.name)
                .version(&about.version)
                .comments(&comments)
                .authors(vec![about.authors])
                .website(&about.homepage)
                .website_label("Source Code")
                .license_type(License::MitX11)
                .system_information(
                    &format!("We are storing data on your system at {}.
This data is only used locally by this client.
The nostr protocol does not use clients as a store of other people's data.", about.database_path)
                )
                .build();

            // FIXME - best to compile this in somehow so finding it isn't a nightmare,
            // and it won't even spin their disk to do so.
            let logo_file = gio::File::for_path("./gossip.svg");
            if let Ok(logo) = gdk::Texture::from_file(&logo_file) {
                about_dialog.set_logo(Some(&logo));
            }

            about_dialog.show();
        }),
    );
    app.add_action(&show_about_window_action);
}

fn build_ui(app: &Application) {
    let ui = Ui::new(app);

    // FIXME: stick this into GLOBALS under an Arc<Mutex<>>

    ui.present();
}

pub struct Ui {
    app_window: ApplicationWindow,
}

impl Ui {
    pub fn new(app: &Application) -> Ui {
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
                let about_menu_item =
                    gio::MenuItem::new(Some("About"), Some("app.show_about_window"));

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

            let main_scrolled_window = {

                let list_view = {
                    // Create a `Vec<Post>` with numbers from 0 to 100_000
                    let vector: Vec<Post> =
                        (0..=1000).into_iter().map(Post::new).collect();

                    // Create new model
                    let model = gio::ListStore::new(Post::static_type());

                    // Add the vector to the model
                    model.extend_from_slice(&vector);

                    let factory = SignalListItemFactory::new();
                    factory.connect_setup(move |_, list_item| {
                        let label = Label::new(None);
                        list_item.set_child(Some(&label)); // Cannot .set_child() on glib::Object
                    });

                    factory.connect_bind(move |_, list_item| {
                        // Get `Post` from `ListItem`
                        let post = list_item
                            .item() // Cannot .item() on glib::Object
                            .expect("The item has to exist.")
                            .downcast::<Post>()
                            .expect("The item has to be an `Post`.");

                        // Get `Label` from `ListItem`
                        let label = list_item
                            .child() // Cannot .child() on glib::Object()
                            .expect("The child has to exist.")
                            .downcast::<Label>()
                            .expect("The child has to be a `Label`.");

                        // Bind "label" to "number"
                        post
                            .bind_property("number", &label, "label")
                            .flags(glib::BindingFlags::SYNC_CREATE)
                            .build();
                    });

                    let selection_model = SingleSelection::new(Some(&model));
                    let list_view = ListView::new(Some(&selection_model), Some(&factory));

                    list_view.connect_activate(move |list_view, position| {
                        // Get `Post` from model
                        let model = list_view.model().expect("The model has to exist.");
                        let post = model
                            .item(position)
                            .expect("The item has to exist.")
                            .downcast::<Post>()
                            .expect("The item has to be a `Post`.");

                        // Increase "number" of `Post`
                        post.increase_number();
                    });

                    list_view
                };

                ScrolledWindow::builder()
                    .hscrollbar_policy(PolicyType::Never)
                    .has_frame(true)
                    .min_content_width(360)
                    .halign(Align::Fill)
                    .hexpand(true)
                    .valign(Align::Fill)
                    .vexpand(true)
                    .child(&list_view)
                    .build()
            };

            let main_hbox = Box::builder().orientation(Orientation::Vertical).build();

            main_hbox.append(&main_scrolled_window);
            main_hbox.append(&statusbar);

            main_hbox
        };

        app_window.set_child(Some(&main_hbox));

        Ui { app_window }
    }

    pub fn present(&self) {
        self.app_window.present();
    }
}
