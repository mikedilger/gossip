mod imp;

use glib::Object;
use gtk::glib;
use nostr_proto::Id;

glib::wrapper! {
    pub struct Post(ObjectSubclass<imp::Post>);
}

impl Post {
    pub fn new(id: Id) -> Self {
        Object::new(&[("id", &id.as_hex_string())])
    }
}
