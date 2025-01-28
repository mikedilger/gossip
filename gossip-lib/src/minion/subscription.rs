use crate::globals::GLOBALS;
use nostr_types::{ClientMessage, Filter, SubscriptionId};
use std::sync::atomic::Ordering;

#[derive(Debug)]
pub struct Subscription {
    id: String,
    job_id: u64,
    filter: Filter,
    eose: bool,
    clone: bool,
}

impl Subscription {
    pub fn new(id: &str, job_id: u64, filter: Filter) -> Subscription {
        GLOBALS.open_subscriptions.fetch_add(1, Ordering::SeqCst);
        Subscription {
            id: id.to_owned(),
            job_id,
            filter,
            eose: false,
            clone: false,
        }
    }

    pub fn set_filter(&mut self, filter: Filter) {
        self.filter = filter;
    }

    pub fn get_filter(&self) -> &Filter {
        &self.filter
    }

    pub fn get_id(&self) -> String {
        self.id.clone()
    }

    pub fn get_job_id(&self) -> u64 {
        self.job_id
    }

    pub fn change_job_id(&mut self, job_id: u64) -> u64 {
        let old = self.job_id;
        self.job_id = job_id;
        old
    }

    pub fn set_eose(&mut self) {
        if !self.clone && !self.eose {
            GLOBALS.open_subscriptions.fetch_sub(1, Ordering::SeqCst);
        }
        self.eose = true;
    }

    pub fn eose(&self) -> bool {
        self.eose
    }

    pub fn req_message(&self) -> ClientMessage {
        ClientMessage::Req(SubscriptionId(self.get_id()), self.filter.clone())
    }

    pub fn close_message(&self) -> ClientMessage {
        ClientMessage::Close(SubscriptionId(self.get_id()))
    }
}

impl Clone for Subscription {
    fn clone(&self) -> Self {
        Subscription {
            id: self.id.clone(),
            job_id: self.job_id,
            filter: self.filter.clone(),
            eose: self.eose,
            clone: true,
        }
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        if !self.clone && !self.eose {
            GLOBALS.open_subscriptions.fetch_sub(1, Ordering::SeqCst);
        }
    }
}
