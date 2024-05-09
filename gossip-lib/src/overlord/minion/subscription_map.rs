use std::collections::HashMap;

use nostr_types::Filter;

use super::subscription::Subscription;

// handle is a coder-friendly string like "general_feed"
// id is a short numeric string like "0", counting up from 0.
pub struct SubscriptionMap {
    handle_to_id: HashMap<String, String>,
    by_id: HashMap<String, Subscription>,
    count: usize,
}

impl SubscriptionMap {
    pub fn new() -> SubscriptionMap {
        SubscriptionMap {
            handle_to_id: HashMap::new(),
            by_id: HashMap::new(),
            count: 0,
        }
    }

    pub fn add(&mut self, handle: &str, job_id: u64, filters: Vec<Filter>) -> String {
        let id = format!("{}", self.count);
        let mut sub = Subscription::new(&id, job_id);
        self.count += 1;
        sub.set_filters(filters);
        self.handle_to_id.insert(handle.to_owned(), id.clone());
        self.by_id.insert(id.clone(), sub);
        id
    }

    pub fn has(&self, handle: &str) -> bool {
        match self.handle_to_id.get(handle) {
            None => false,
            Some(id) => self.by_id.contains_key(id),
        }
    }

    pub fn get(&self, handle: &str) -> Option<Subscription> {
        match self.handle_to_id.get(handle) {
            None => None,
            Some(id) => self.by_id.get(id).cloned(),
        }
    }

    pub fn get_all_handles_matching(&self, substr: &str) -> Vec<String> {
        let mut output: Vec<String> = Vec::new();
        for handle in self.handle_to_id.keys() {
            if handle.contains(substr) {
                output.push(handle.clone());
            }
        }
        output
    }

    /*
    pub fn get_by_id(&self, id: &str) -> Option<Subscription> {
        self.by_id.get(id).cloned()
    }
     */

    pub fn get_handle_by_id(&self, id: &str) -> Option<String> {
        for (handle, xid) in self.handle_to_id.iter() {
            if id == xid {
                return Some(handle.to_string());
            }
        }
        None
    }

    pub fn get_mut(&mut self, handle: &str) -> Option<&mut Subscription> {
        match self.handle_to_id.get(handle) {
            None => None,
            Some(id) => self.by_id.get_mut(id),
        }
    }

    pub fn get_mut_by_id(&mut self, id: &str) -> Option<&mut Subscription> {
        self.by_id.get_mut(id)
    }

    pub fn remove(&mut self, handle: &str) -> Option<String> {
        if let Some(id) = self.handle_to_id.get(handle) {
            let id = id.to_owned();
            self.by_id.remove(&id);
            self.handle_to_id.remove(handle);
            Some(id)
        } else {
            None
        }
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    /*
        pub fn remove_by_id(&mut self, id: &str) {
            self.by_id.remove(id);
    }
        */
}
