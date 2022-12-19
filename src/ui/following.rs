use gtk::prelude::*;
use gtk::{Application, Window};

pub fn show_window(_app: &Application) {
    let following_window = Window::builder()
        .decorated(true)
        .title("Gossip: Following")
        .default_width(400)
        .default_height(600)
        .resizable(true)
        .build();

    following_window.show();
}
