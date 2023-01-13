use crate::error::Error;
use crate::globals::GLOBALS;
use dashmap::{DashMap, DashSet};
use nostr_types::{Event, Id};
use tokio::task;

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

    /// Get the event from memory, and also try the database
    #[allow(dead_code)]
    pub async fn get_local(&self, id: Id) -> Result<Option<Event>, Error> {
        if let Some(e) = self.get(&id) {
            return Ok(Some(e));
        }

        if let Some(event) = task::spawn_blocking(move || {
            let maybe_db = GLOBALS.db.blocking_lock();
            let db = maybe_db.as_ref().unwrap();
            let mut stmt = db.prepare("SELECT raw FROM event WHERE id=?")?;
            stmt.raw_bind_parameter(1, id.as_hex_string())?;
            let mut rows = stmt.raw_query();
            if let Some(row) = rows.next()? {
                let s: String = row.get(0)?;
                Ok(Some(serde_json::from_str(&s)?))
            } else {
                Ok::<Option<Event>, Error>(None)
            }
        })
        .await??
        {
            self.insert(event.clone());
            Ok(Some(event))
        } else {
            Ok(None)
        }
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
