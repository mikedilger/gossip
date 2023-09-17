use crate::globals::GLOBALS;
use crate::relay::Relay;
use nostr_types::{Event, EventKind, PublicKey, RelayUrl};
use std::collections::HashSet;

#[derive(Debug, Default)]
pub struct WizardState {
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
}

impl WizardState {
    pub fn update(&mut self) {
        self.follow_only = GLOBALS.storage.read_following_only();

        self.pubkey = GLOBALS.signer.public_key();
        self.has_private_key = GLOBALS.signer.is_ready();

        if let Some(pk) = self.pubkey {
            self.metadata_events = GLOBALS
                .storage
                .find_events(&[EventKind::Metadata], &[pk], None, |_| true, true)
                .unwrap_or(Vec::new());

            self.contact_list_events = GLOBALS
                .storage
                .find_events(&[EventKind::ContactList], &[pk], None, |_| true, true)
                .unwrap_or(Vec::new());

            self.relay_list_events = GLOBALS
                .storage
                .find_events(&[EventKind::RelayList], &[pk], None, |_| true, true)
                .unwrap_or(Vec::new());

            self.relays = GLOBALS
                .storage
                .filter_relays(|relay| relay.usage_bits != 0)
                .unwrap_or(Vec::new());
        }

        self.followed = GLOBALS.people.get_followed_pubkeys();

        if self.need_discovery_relays() {
            let purplepages = RelayUrl::try_from_str("wss://purplepag.es/").unwrap();
            let _ = GLOBALS.storage.modify_relay(
                &purplepages,
                |relay| relay.set_usage_bits(Relay::DISCOVER),
                None,
            );
        }
    }

    #[inline]
    pub fn need_discovery_relays(&self) -> bool {
        self.relays
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
