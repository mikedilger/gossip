use gtk::prelude::*;
use gtk::{Application, Window};

pub fn show_window(_app: &Application) {
    let relays_window = Window::builder()
        .decorated(true)
        .title("Gossip: Relays")
        .default_width(400)
        .default_height(600)
        .resizable(true)
        .build();

    relays_window.show();
}
