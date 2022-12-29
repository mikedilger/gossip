use crate::globals::GLOBALS;
use nostr_types::{Event, EventKind, Id};
use std::time::{Duration, Instant};

pub struct Feed {
    feed: Vec<Id>,

    // We only recompute the feed at specified intervals
    interval_ms: u32,
    last_computed: Instant,
}

impl Feed {
    pub fn new() -> Feed {
        Feed {
            feed: Vec::new(),
            interval_ms: 1000, // Every second, until we load from settings
            last_computed: Instant::now(),
        }
    }

    pub fn get(&mut self) -> Vec<Id> {
        let now = Instant::now();
        if self.last_computed + Duration::from_millis(self.interval_ms as u64) < now {
            self.recompute();
            self.last_computed = now;
        }

        self.feed.clone()
    }

    fn recompute(&mut self) {
        let settings = GLOBALS.settings.blocking_read().clone();
        self.interval_ms = settings.feed_recompute_interval_ms;

        let mut events: Vec<Event> = GLOBALS
            .events
            .blocking_read()
            .iter()
            .map(|(_, e)| e)
            .filter(|e| e.kind == EventKind::TextNote)
            .filter(|e| !GLOBALS.dismissed.blocking_read().contains(&e.id))
            .filter(|e| {
                if settings.view_threaded {
                    e.replies_to().is_none()
                } else {
                    true
                }
            })
            .cloned()
            .collect();

        if settings.view_threaded {
            events.sort_unstable_by(|a, b| {
                let a_last = GLOBALS.last_reply.blocking_read().get(&a.id).cloned();
                let b_last = GLOBALS.last_reply.blocking_read().get(&b.id).cloned();
                let a_time = a_last.unwrap_or(a.created_at);
                let b_time = b_last.unwrap_or(b.created_at);
                b_time.cmp(&a_time)
            });
        } else {
            events.sort_unstable_by(|a, b| b.created_at.cmp(&a.created_at));
        }

        self.feed = events.iter().map(|e| e.id).collect();
    }
}
