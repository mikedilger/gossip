use gtk::prelude::*;
use gtk::{Application, ApplicationWindow};

const APP_ID: &str = "com.mikedilger.gossip";

pub fn run() {
    // Create a new application
    let app = Application::builder().application_id(APP_ID).build();

    // Connect to "activate" signal of `app`
    app.connect_activate(build_ui);

    // Run the application
    app.run();

    // Initiate shutdown
    if let Err(e) = crate::initiate_shutdown() {
        log::error!("{}", e);
    }
}

fn build_ui(app: &Application) {
    // Create a window and set the title
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Gossip")
        .build();

    // Present window
    window.present();
}
