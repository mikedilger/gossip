use gtk::prelude::*;
use gtk::{Application, Window};

pub fn show_window(_app: &Application) {
    let settings_window = Window::builder()
        .decorated(true)
        .title("Gossip: Settings")
        .default_width(400)
        .default_height(600)
        .resizable(true)
        .build();

    settings_window.show();
}
