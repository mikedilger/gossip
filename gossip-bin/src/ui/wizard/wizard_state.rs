use gossip_lib::{PersonList, Relay, GLOBALS};
use nostr_types::{Event, EventKind, PublicKey, RelayUrl};
use std::collections::HashSet;

#[derive(Debug)]
pub struct WizardState {
    pub error: Option<String>,
    pub last_status_queue_message: String,
    pub new_user: bool,
    pub follow_only: bool,
    pub relay_url: String,
    pub relay_list_sought: bool,
    pub metadata_copied: bool,
    pub metadata_name: String,
    pub metadata_about: String,
    pub metadata_picture: String,
    pub pubkey: Option<PublicKey>,
    pub has_private_key: bool,
    pub metadata_events: Vec<Event>,
    pub contact_list_events: Vec<Event>,
    pub relay_list_events: Vec<Event>,
    pub relays: Vec<Relay>,
    pub followed: Vec<PublicKey>,
    pub followed_getting_metadata: HashSet<PublicKey>,
    pub contacts_sought: bool,
    pub generating: bool,
}

impl Default for WizardState {
    fn default() -> WizardState {
        WizardState {
            error: None,
            last_status_queue_message: "".to_owned(),
            new_user: false,
            follow_only: false,
            relay_url: "".to_owned(),
            relay_list_sought: true,
            metadata_copied: false,
            metadata_name: "".to_owned(),
            metadata_about: "".to_owned(),
            metadata_picture: "".to_owned(),
            pubkey: None,
            has_private_key: false,
            metadata_events: Vec::new(),
            contact_list_events: Vec::new(),
            relay_list_events: Vec::new(),
            relays: Vec::new(),
            followed: Vec::new(),
            followed_getting_metadata: HashSet::new(),
            contacts_sought: true,
            generating: false,
        }
    }
}
impl WizardState {
    pub fn update(&mut self) {
        self.follow_only = GLOBALS.storage.get_flag_following_only();

        self.pubkey = GLOBALS.identity.public_key();
        self.has_private_key = GLOBALS.identity.is_unlocked();

        if let Some(pk) = self.pubkey {
            self.metadata_events = GLOBALS
                .storage
                .find_events(&[EventKind::Metadata], &[pk], None, |_| true, true)
                .unwrap_or_default();

            self.contact_list_events = GLOBALS
                .storage
                .find_events(&[EventKind::ContactList], &[pk], None, |_| true, true)
                .unwrap_or_default();

            self.relay_list_events = GLOBALS
                .storage
                .find_events(&[EventKind::RelayList], &[pk], None, |_| true, true)
                .unwrap_or_default();

            self.relays = GLOBALS
                .storage
                .filter_relays(|relay| relay.has_any_usage_bit())
                .unwrap_or_default();
        }

        self.followed = GLOBALS
            .storage
            .get_people_in_list(PersonList::Followed)
            .unwrap_or_default()
            .drain(..)
            .map(|(pk, _)| pk)
            .collect();

        if self.need_discovery_relays() {
            let purplepages = RelayUrl::try_from_str("wss://purplepag.es/").unwrap();
            let _ = GLOBALS.storage.modify_relay(
                &purplepages,
                |relay| relay.set_usage_bits(Relay::DISCOVER),
                None,
            );
        }

        // Copy any new status queue messages into our local error variable
        let last_status_queue_message = GLOBALS.status_queue.read().read_last();
        if last_status_queue_message != self.last_status_queue_message {
            if !last_status_queue_message.starts_with("Welcome to Gossip") {
                self.error = Some(last_status_queue_message.clone());
                self.last_status_queue_message = last_status_queue_message;
            }
        }
    }

    #[inline]
    pub fn need_discovery_relays(&self) -> bool {
        !self
            .relays
            .iter()
            .any(|relay| relay.has_usage_bits(Relay::DISCOVER))
    }

    #[inline]
    pub fn need_relay_list(&self) -> bool {
        self.relay_list_events.is_empty()
    }

    #[inline]
    pub fn need_user_data(&self) -> bool {
        self.metadata_events.is_empty() || self.contact_list_events.is_empty()
    }
}
