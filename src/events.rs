use crate::error::Error;
use crate::globals::GLOBALS;
use dashmap::DashMap;
use nostr_types::{Event, Filter, Id};
use std::fmt::Display;
use tokio::task;

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
        // this will just replace if already seen
        let _ = self.events.insert(event.id, event);
    }

    pub fn get(&self, id: &Id) -> Option<Event> {
        self.events.get(id).map(|e| e.value().to_owned())
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
            self.insert(event.clone());
        }

        Ok(events)
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
