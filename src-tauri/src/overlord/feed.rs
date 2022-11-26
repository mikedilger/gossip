
use crate::overlord::JsEvent;

pub struct Feed {
    // sorted from oldest to newest
    pub inner: Vec<(i64, String)>
}

impl Feed {
    pub fn new() -> Feed {
        Feed { inner: Vec::new() }
    }

    #[allow(dead_code)]
    pub fn add_event(&mut self, e: &JsEvent) {
        self.inner.push((e.created_at, e.id.clone()));
        self.inner.sort_unstable_by(|a,b| a.0.cmp(&b.0))
    }

    pub fn add_events(&mut self, e: &[JsEvent]) {
        self.inner.extend(e.iter().map(|e| (e.created_at, e.id.clone())));
        self.inner.sort_unstable_by(|a,b| a.0.cmp(&b.0));
        self.inner.dedup();
    }

    pub fn as_id_vec(&self) -> Vec<String> {
        self.inner.iter().map(|(_,id)| id.clone()).collect()
    }
}
