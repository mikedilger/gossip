use nostr_types::{ClientMessage, Filter, SubscriptionId};

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

    pub fn set_filters(&mut self, filters: Vec<Filter>) {
        self.filters = filters;
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
