use crate::error::Error;
use crate::globals::GLOBALS;
use async_recursion::async_recursion;
use dashmap::DashMap;
use nostr_types::{Event, Filter, Id};
use std::fmt::Display;

pub struct Events {
    events: DashMap<Id, Event>,
}

impl Events {
    pub fn new() -> Events {
        Events {
            events: DashMap::new(),
        }
    }

    pub fn insert(&self, event: Event) {
        let _ = self.events.insert(event.id, event);
    }

    /*
    pub fn contains_key(&self, id: &Id) -> bool {
        self.events.contains_key(id)
    }
    */

    pub fn get(&self, id: &Id) -> Option<Event> {
        self.events.get(id).map(|e| e.value().to_owned())
    }

    /// Get the event from memory, and also try the database, by Id
    pub async fn get_local(&self, id: Id) -> Result<Option<Event>, Error> {
        if let Some(e) = self.get(&id) {
            return Ok(Some(e));
        }

        let pool = GLOBALS.db.clone();
        let db = pool.get()?;
        let opt_event: Option<Event> = {
            let mut stmt = db.prepare("SELECT raw FROM event WHERE id=?")?;
            stmt.raw_bind_parameter(1, id.as_hex_string())?;
            let mut rows = stmt.raw_query();
            match rows.next()? {
                None => None,
                Some(row) => {
                    let s: String = row.get(0)?;
                    Some(serde_json::from_str(&s)?)
                }
            }
        };

        match opt_event {
            None => Ok(None),
            Some(event) => {
                // Process that event
                crate::process::process_new_event(&event, false, None, None).await?;

                self.insert(event.clone());
                Ok(Some(event))
            }
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
                let k: u64 = (*kind).into();
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

        let pool = GLOBALS.db.clone();
        let db = pool.get()?;
        let mut stmt = db.prepare(&sql)?;
        let mut rows = stmt.raw_query();
        let mut events: Vec<Event> = Vec::new();
        while let Some(row) = rows.next()? {
            let s: String = row.get(0)?;
            events.push(serde_json::from_str(&s)?);
        }

        for event in events.iter() {
            // Process that event
            crate::process::process_new_event(event, false, None, None).await?;

            // Add to memory
            self.insert(event.clone());
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
