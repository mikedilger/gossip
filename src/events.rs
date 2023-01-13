use dashmap::{DashMap, DashSet};
use nostr_types::{Event, Id};

pub struct Events {
    events: DashMap<Id, Event>,
    new_events: DashSet<Id>,
}

impl Events {
    pub fn new() -> Events {
        Events {
            events: DashMap::new(),
            new_events: DashSet::new(),
        }
    }

    pub fn insert(&self, event: Event) {
        let _ = self.new_events.insert(event.id);
        let _ = self.events.insert(event.id, event);
    }

    pub fn contains_key(&self, id: &Id) -> bool {
        self.events.contains_key(id)
    }

    pub fn get(&self, id: &Id) -> Option<Event> {
        self.events.get(id).map(|e| e.value().to_owned())
    }

    pub fn is_new(&self, id: &Id) -> bool {
        self.new_events.contains(id)
    }

    pub fn clear_new(&self) {
        self.new_events.clear();
    }

    pub fn iter(&self) -> dashmap::iter::Iter<Id, Event> {
        self.events.iter()
    }
}
