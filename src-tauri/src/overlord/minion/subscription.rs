
use nostr_proto::{ClientMessage, Filters, SubscriptionId};

#[derive(Debug)]
pub struct Subscription {
    id: String,
    filters: Vec<Filters>,
    eose: bool,
}

impl Subscription {
    pub fn new(id: String) -> Subscription {
        Subscription {
            id: id,
            filters: vec![],
            eose: false
        }
    }

    pub fn get_id(&self) -> String {
        self.id.clone()
    }

    pub fn get_mut<'a>(&'a mut self) -> &'a mut Vec<Filters> {
        &mut self.filters
    }

    pub fn set_eose(&mut self) {
        self.eose = true;
    }

    #[allow(dead_code)]
    pub fn eose(&self) -> bool {
        self.eose
    }

    pub fn req_message(&self) -> ClientMessage {
        ClientMessage::Req(
            SubscriptionId(self.get_id()),
            self.filters.clone(),
        )
    }

    pub fn close_message(&self) -> ClientMessage {
        ClientMessage::Close(
            SubscriptionId(self.get_id()),
        )
    }
}
