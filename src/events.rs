use crate::db::DbEventRelay;
use crate::error::Error;
use crate::globals::GLOBALS;
use async_recursion::async_recursion;
use dashmap::mapref::entry::Entry;
use dashmap::{DashMap, DashSet};
use nostr_types::{Event, Filter, Id, RelayUrl};
use std::fmt::Display;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use tokio::task;
use vecmap::VecSet;

pub struct Events {
    events: DashMap<Id, Event>,

    // Seen on data uses relay_map to compress it's data
    seen_on: DashMap<Id, VecSet<usize>>,
    relay_map: RelayMap,

    // Events we are currently seeking from the database
    sought_events: DashSet<Id>,
}

impl Events {
    pub fn new() -> Events {
        Events {
            events: DashMap::new(),
            seen_on: DashMap::new(),
            relay_map: RelayMap::new(),
            sought_events: DashSet::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn insert(&self, event: Event, seen_on: Option<RelayUrl>) {
        // Add seen on
        if let Some(url) = seen_on {
            self.add_seen_on(event.id, &url);
        }

        // this will just replace if already seen
        let _ = self.events.insert(event.id, event);
    }

    pub fn add_seen_on(&self, id: Id, url: &RelayUrl) {
        let relay_index = self.relay_map.relay_to_index(url);
        match self.seen_on.entry(id) {
            Entry::Occupied(mut oentry) => {
                oentry.get_mut().insert(relay_index);
            }
            Entry::Vacant(ventry) => {
                let mut set = VecSet::new();
                set.insert(relay_index);
                ventry.insert(set);
            }
        }
    }

    /*
        pub fn contains_key(&self, id: &Id) -> bool {
            self.events.contains_key(id)
    }
        */

    pub fn get(&self, id: &Id) -> Option<Event> {
        self.events.get(id).map(|e| e.value().to_owned())
    }

    pub fn get_seen_on(&self, id: &Id) -> Option<Vec<RelayUrl>> {
        self.seen_on.get(id).map(|set| {
            set.iter()
                .map(|index| self.relay_map.index_to_relay(*index).unwrap())
                .collect()
        })
    }

    /// Get the event from memory, and also try the database, by Id
    pub async fn get_local(&self, id: Id) -> Result<Option<Event>, Error> {
        if let Some(e) = self.get(&id) {
            return Ok(Some(e));
        }

        // Don't go seeking in the database if some other thread already is.
        if self.sought_events.contains(&id) {
            return Ok(None);
        }
        // Mark that we are handling this one
        self.sought_events.insert(id);

        if let Some(event) = task::spawn_blocking(move || {
            let db = GLOBALS.db.blocking_lock();
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
            // Process that event
            crate::process::process_new_event(&event, false, None, None).await?;
            self.insert(event.clone(), None);
            self.sought_events.remove(&id);
            Ok(Some(event))
        } else {
            self.sought_events.remove(&id);
            Ok(None)
        }
    }

    /// Get event from database, by Filter
    pub async fn get_local_events_by_filter(&self, filter: Filter) -> Result<Vec<Event>, Error> {
        let mut conditions: Vec<String> = Vec::new();
        if !filter.ids.is_empty() {
            conditions.push(build_prefix_condition("id", &filter.ids));
        }
        if !filter.authors.is_empty() {
            conditions.push(build_prefix_condition("pubkey", &filter.authors));
        }
        if !filter.kinds.is_empty() {
            let mut c = "kind in (".to_owned();
            let mut virgin = true;
            for kind in filter.kinds.iter() {
                if !virgin {
                    c.push(',');
                }
                virgin = false;
                let k: u32 = (*kind).into();
                c.push_str(&format!("{}", k));
            }
            c.push(')');
            conditions.push(c);
        }
        if !filter.a.is_empty() {
            conditions.push(build_tag_condition("a", &filter.a));
        }
        if !filter.d.is_empty() {
            conditions.push(build_tag_condition("d", &filter.d));
        }
        if !filter.e.is_empty() {
            conditions.push(build_tag_condition("e", &filter.e));
        }
        if !filter.g.is_empty() {
            conditions.push(build_tag_condition("g", &filter.g));
        }
        if !filter.p.is_empty() {
            conditions.push(build_tag_condition("p", &filter.p));
        }
        if !filter.r.is_empty() {
            conditions.push(build_tag_condition("r", &filter.r));
        }
        if !filter.t.is_empty() {
            conditions.push(build_tag_condition("t", &filter.t));
        }
        if let Some(since) = filter.since {
            conditions.push(format!("created_at >= {}", since.0));
        }
        if let Some(until) = filter.until {
            conditions.push(format!("created_at <= {}", until.0));
        }

        let mut sql = format!("SELECT raw FROM event WHERE {}", conditions.join(" AND "));

        if let Some(limit) = filter.limit {
            sql.push_str(&(format!(" LIMIT {}", limit)));
        }

        tracing::trace!("get_local_events_by_filter SQL={}", &sql);

        let events_result = task::spawn_blocking(move || -> Result<Vec<Event>, Error> {
            let db = GLOBALS.db.blocking_lock();
            let mut stmt = db.prepare(&sql)?;
            let mut rows = stmt.raw_query();
            let mut events: Vec<Event> = Vec::new();
            while let Some(row) = rows.next()? {
                let s: String = row.get(0)?;
                events.push(serde_json::from_str(&s)?);
            }
            Ok(events)
        })
        .await?;
        let events = events_result?;

        for event in events.iter() {
            // Process that event
            crate::process::process_new_event(event, false, None, None).await?;

            // Add to memory
            self.insert(event.clone(), None);
        }

        Ok(events)
    }

    #[async_recursion]
    pub async fn get_highest_local_parent(&self, id: &Id) -> Result<Option<Id>, Error> {
        if let Some(event) = self.get_local(*id).await? {
            if let Some((parent_id, _opturl)) = event.replies_to() {
                match self.get_highest_local_parent(&parent_id).await? {
                    Some(top_id) => Ok(Some(top_id)), // went higher
                    None => Ok(Some(*id)),            // couldn't go higher, stay here
                }
            } else {
                Ok(Some(*id)) // is a root
            }
        } else {
            Ok(None) // not present locally
        }
    }

    pub fn iter(&self) -> dashmap::iter::Iter<Id, Event> {
        self.events.iter()
    }

    pub async fn load_event_seen_data(&self) -> Result<(), Error> {
        tracing::info!("Loading event seen-on data...");
        for dashref in self.events.iter() {
            let id = dashref.key();
            let vecurl = DbEventRelay::get_relays_for_event(*id)?;
            for url in vecurl.iter() {
                self.add_seen_on(*id, url);
            }
        }
        Ok(())
    }
}

fn build_prefix_condition<T: Display>(field: &str, values: &[T]) -> String {
    let mut c = "(".to_owned();
    let mut virgin = true;
    for val in values.iter() {
        if !virgin {
            c.push_str(" OR ");
        }
        virgin = false;
        c.push_str(&format!("{} LIKE '{}%'", field, val));
    }
    c.push(')');
    c
}

fn build_tag_condition<T: Display>(tag: &str, values: &[T]) -> String {
    // "id in (SELECT event FROM event_tag WHERE label='e' AND field0={})";
    let mut c = format!(
        "id in (SELECT event FROM event_tag WHERE label='{}' AND field0 IN (",
        tag
    );
    let mut virgin = true;
    for val in values.iter() {
        if !virgin {
            c.push(',');
        }
        virgin = false;
        c.push_str(&format!("'{}'", val));
    }
    c.push_str("))");
    c
}

// This maps a RelayURL to a usize, and the inverse as well.
struct RelayMap {
    relay_to_index: DashMap<RelayUrl, usize>,
    index_to_relay: DashMap<usize, RelayUrl>,
    next: AtomicUsize,
}

impl RelayMap {
    pub fn new() -> RelayMap {
        RelayMap {
            relay_to_index: DashMap::new(),
            index_to_relay: DashMap::new(),
            next: AtomicUsize::new(0),
        }
    }

    pub fn insert(&self, url: &RelayUrl) -> usize {
        // If we already have it, return it
        if let Some(u) = self.relay_to_index.get(url) {
            return *u;
        }

        let index = self.next.fetch_add(1, Ordering::SeqCst);
        let _ = self.index_to_relay.insert(index, url.to_owned());
        let _ = self.relay_to_index.insert(url.to_owned(), index);
        index
    }

    #[inline]
    pub fn relay_to_index(&self, url: &RelayUrl) -> usize {
        if let Some(index) = self.relay_to_index.get(url) {
            return *index.value();
        }

        self.insert(url)
    }

    #[inline]
    pub fn index_to_relay(&self, index: usize) -> Option<RelayUrl> {
        self.index_to_relay
            .get(&index)
            .map(|r| r.value().to_owned())
    }
}
