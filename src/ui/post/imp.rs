use std::cell::Cell;

use glib::{ParamSpec, ParamSpecString, Value};
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use nostr_proto::Id;
use once_cell::sync::Lazy;

// Object holding the state
pub struct Post {
    id: Cell<Id>,
}

impl Default for Post {
    fn default() -> Post {
        Post {
            id: Cell::new(
                Id::try_from_hex_string(
                    "0000000000000000000000000000000000000000000000000000000000000000",
                )
                .unwrap(),
            ),
        }
    }
}

// The central trait for subclassing a GObject
#[glib::object_subclass]
impl ObjectSubclass for Post {
    const NAME: &'static str = "Post";
    type Type = super::Post;
}

// Trait shared by all GObjects
impl ObjectImpl for Post {
    fn properties() -> &'static [ParamSpec] {
        static PROPERTIES: Lazy<Vec<ParamSpec>> =
            Lazy::new(|| vec![ParamSpecString::builder("id").build()]);
        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &Value, pspec: &ParamSpec) {
        match pspec.name() {
            "id" => {
                let s: String = value
                    .get::<String>()
                    .expect("The value needs to be Some<String>");
                let id = Id::try_from_hex_string(&s).expect("Id must be valid hex ID");
                self.id.replace(id);
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
        match pspec.name() {
            "id" => {
                let id = self.id.get();
                id.as_hex_string().to_value()
            }
            _ => unimplemented!(),
        }
    }
}
