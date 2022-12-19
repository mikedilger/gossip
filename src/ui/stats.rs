use gtk::prelude::*;
use gtk::{Application, Window};

pub fn show_window(_app: &Application) {
    let stats_window = Window::builder()
        .decorated(true)
        .title("Gossip: Stats")
        .default_width(400)
        .default_height(600)
        .resizable(true)
        .build();

    stats_window.show();
}
