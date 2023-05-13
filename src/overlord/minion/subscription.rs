use nostr_types::{ClientMessage, Filter, SubscriptionId};
use std::collections::HashMap;

pub struct Subscriptions {
    handle_to_id: HashMap<String, String>,
    by_id: HashMap<String, Subscription>,
    count: usize,
}

impl Subscriptions {
    pub fn new() -> Subscriptions {
        Subscriptions {
            handle_to_id: HashMap::new(),
            by_id: HashMap::new(),
            count: 0,
        }
    }

    pub fn add(&mut self, handle: &str, job_id: u64, filters: Vec<Filter>) -> String {
        let id = format!("{}", self.count);
        let mut sub = Subscription::new(&id, job_id);
        self.count += 1;
        sub.filters = filters;
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

#[derive(Clone, Debug)]
pub struct Subscription {
    id: String,
    job_id: u64,
    filters: Vec<Filter>,
    eose: bool,
}

impl Subscription {
    pub fn new(id: &str, job_id: u64) -> Subscription {
        Subscription {
            id: id.to_owned(),
            job_id,
            filters: vec![],
            eose: false,
        }
    }

    pub fn get_id(&self) -> String {
        self.id.clone()
    }

    pub fn get_job_id(&self) -> u64 {
        self.job_id
    }

    pub fn set_eose(&mut self) {
        self.eose = true;
    }

    pub fn eose(&self) -> bool {
        self.eose
    }

    pub fn req_message(&self) -> ClientMessage {
        ClientMessage::Req(SubscriptionId(self.get_id()), self.filters.clone())
    }

    pub fn close_message(&self) -> ClientMessage {
        ClientMessage::Close(SubscriptionId(self.get_id()))
    }
}
