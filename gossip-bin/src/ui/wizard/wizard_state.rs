use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use gossip_lib::{Person, PersonList, Relay, GLOBALS};
use nostr_types::{Event, EventKind, Filter, PublicKey, PublicKeyHex, RelayUrl};

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum WizardPath {
    /// the user wants to import an existing account from
    /// a private (true) or public key (false)
    ImportFromKey(bool),
    /// the user wants to create a new account
    CreateNewAccount,
    /// the user only wants to create a local follow list
    /// without setting up any keys
    FollowOnlyNoKeys,
}

#[derive(Debug)]
pub struct WizardState {
    pub path: WizardPath,
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
    pub metadata_should_publish: bool,
    pub pubkey: Option<PublicKey>,
    pub has_private_key: bool,
    pub metadata_events: Vec<Event>,
    pub contact_list_events: Vec<Event>,
    pub relay_list_events: Vec<Event>,
    pub relays: Vec<Relay>,
    pub relays_should_publish: bool,
    #[allow(clippy::type_complexity)]
    pub followed: Vec<(Option<PublicKey>, Option<Rc<RefCell<Person>>>)>,
    pub followed_last_try: f64,
    pub followed_getting_metadata: HashSet<PublicKey>,
    pub follow_list_should_publish: bool,
    pub contacts_sought: bool,
    pub generating: bool,
}

impl Default for WizardState {
    fn default() -> WizardState {
        WizardState {
            path: WizardPath::CreateNewAccount,
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
            metadata_should_publish: true,
            pubkey: None,
            has_private_key: false,
            metadata_events: Vec::new(),
            contact_list_events: Vec::new(),
            relay_list_events: Vec::new(),
            relays: Vec::new(),
            relays_should_publish: true,
            followed: Vec::new(),
            followed_last_try: 0.0,
            followed_getting_metadata: HashSet::new(),
            follow_list_should_publish: true,
            contacts_sought: true,
            generating: false,
        }
    }
}
impl WizardState {
    pub fn init(&mut self) {
        if self.need_discovery_relays() {
            let purplepages = RelayUrl::try_from_str("wss://purplepag.es/").unwrap();
            super::modify_relay(&purplepages, |relay| relay.set_usage_bits(Relay::DISCOVER));
        }
    }

    pub fn update(&mut self) {
        self.follow_only = GLOBALS.storage.get_flag_following_only();

        self.pubkey = GLOBALS.identity.public_key();
        self.has_private_key = GLOBALS.identity.is_unlocked();

        if let Some(pk) = self.pubkey {
            let pkh: PublicKeyHex = pk.into();
            let mut filter = Filter::new();
            filter.add_author(&pkh);

            filter.kinds = vec![EventKind::Metadata];
            self.metadata_events = GLOBALS
                .storage
                .find_events_by_filter(&filter, |_| true)
                .unwrap_or_default();

            filter.kinds = vec![EventKind::ContactList];
            self.contact_list_events = GLOBALS
                .storage
                .find_events_by_filter(&filter, |_| true)
                .unwrap_or_default();

            filter.kinds = vec![EventKind::RelayList];
            self.relay_list_events = GLOBALS
                .storage
                .find_events_by_filter(&filter, |_| true)
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
            .map(|(pk, _)| (Some(pk), None))
            .collect();

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
