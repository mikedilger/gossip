use gtk::prelude::*;
use gtk::{Application, Window};

pub fn show_window(_app: &Application) {
    let about_window = Window::builder()
        .decorated(true)
        .title("Gossip: About")
        .default_width(400)
        .default_height(600)
        .resizable(true)
        .build();

    about_window.show();
}
