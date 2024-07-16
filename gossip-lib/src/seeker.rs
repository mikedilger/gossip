use crate::comms::ToOverlordMessage;
use crate::error::Error;
use crate::globals::GLOBALS;
use crate::misc::Freshness;
use crate::people::People;
use crate::relay;
use crate::relay::Relay;
use dashmap::DashMap;
use nostr_types::{Event, EventReference, Id, PublicKey, RelayUrl, Unixtime};
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum SeekState {
    WaitingRelayList(PublicKey),
    WaitingEvent,
}

#[derive(Debug, Clone)]
pub struct SeekData {
    /// When we started seeking the event
    pub start: Unixtime,

    /// If we are waiting on the event itself, or a relay list first
    pub state: SeekState,

    /// Once we get the event, should we climb it's parents to the root?
    pub climb: bool,
}

impl SeekData {
    fn new_event(climb: bool) -> SeekData {
        SeekData {
            start: Unixtime::now(),
            state: SeekState::WaitingEvent,
            climb,
        }
    }

    fn new_relay_list(pubkey: PublicKey, climb: bool) -> SeekData {
        SeekData {
            start: Unixtime::now(),
            state: SeekState::WaitingRelayList(pubkey),
            climb,
        }
    }
}

#[derive(Debug, Default)]
pub struct Seeker {
    events: DashMap<Id, SeekData>,
}

impl Seeker {
    pub(crate) fn new() -> Seeker {
        Seeker {
            ..Default::default()
        }
    }

    fn minion_seek_relay_list(author: PublicKey) {
        let _ = GLOBALS
            .to_overlord
            .send(ToOverlordMessage::SubscribeDiscover(vec![author], None));
    }

    fn minion_seek_event_at_our_read_relays(id: Id) {
        let _ = GLOBALS
            .to_overlord
            .send(ToOverlordMessage::FetchEvent(id, vec![]));
    }

    fn minion_seek_event_at_relays(id: Id, relays: Vec<RelayUrl>) {
        let _ = GLOBALS
            .to_overlord
            .send(ToOverlordMessage::FetchEvent(id, relays));
    }

    /// Seek an event when you only have the `Id`
    pub(crate) fn seek_id(
        &self,
        id: Id,
        speculative_relays: Vec<RelayUrl>,
        climb: bool,
    ) -> Result<(), Error> {
        if self.events.get(&id).is_some() {
            return Ok(()); // we are already seeking this event
        }

        tracing::debug!("Seeking id={}", id.as_hex_string());

        let mut relays: Vec<RelayUrl> = Relay::choose_relay_urls(Relay::READ, |_| true)?;
        relays.extend(speculative_relays);
        Self::minion_seek_event_at_relays(id, relays);

        // Remember when we asked
        self.events.insert(id, SeekData::new_event(climb));

        Ok(())
    }

    /// Seek an event when you have the `Id` and the author `PublicKey`
    /// Additional relays can be passed in and the event will also be sought there
    pub(crate) fn seek_id_and_author(
        &self,
        id: Id,
        author: PublicKey,
        speculative_relays: Vec<RelayUrl>,
        climb: bool,
    ) -> Result<(), Error> {
        // Start speculative seek (this is untracked. We will track the by author
        // seek process instead. BUT if the event comes in, it does cancel).
        if !speculative_relays.is_empty() {
            Self::minion_seek_event_at_relays(id, speculative_relays);
        }

        if self.events.get(&id).is_some() {
            return Ok(()); // we are already seeking this event
        }

        tracing::debug!(
            "Seeking id={} with author={}",
            id.as_hex_string(),
            author.as_hex_string()
        );

        // Check if we have the author's relay list
        match People::person_needs_relay_list(author) {
            Freshness::NeverSought => {
                Self::minion_seek_relay_list(author);
                self.events
                    .insert(id, SeekData::new_relay_list(author, climb));
            }
            Freshness::Stale => {
                // Seek the relay list because it is stale, but don't let that hold us up
                // using the stale data
                Self::minion_seek_relay_list(author);

                let relays = relay::get_best_relays_fixed(author, true)?;
                Self::minion_seek_event_at_relays(id, relays);
                self.events.insert(id, SeekData::new_event(climb));
            }
            Freshness::Fresh => {
                let relays = relay::get_best_relays_fixed(author, true)?;
                Self::minion_seek_event_at_relays(id, relays);
                self.events.insert(id, SeekData::new_event(climb));
            }
        }

        Ok(())
    }

    /// Seek an event when you have the `Id` and the relays to seek from
    pub(crate) fn seek_id_and_relays(&self, id: Id, relays: Vec<RelayUrl>, climb: bool) {
        if let Some(existing) = self.events.get(&id) {
            if matches!(existing.value().state, SeekState::WaitingEvent) {
                return; // Already seeking it
            }
        }
        Self::minion_seek_event_at_relays(id, relays);
        self.events.insert(id, SeekData::new_event(climb));
    }

    /// Inform the seeker that an author's relay list has just arrived
    pub(crate) fn found_author_relays(&self, pubkey: PublicKey) {
        // Instead of updating the map while we iterate (which could deadlock)
        // we save updates here and apply when the iterator is finished.
        let mut updates: Vec<(Id, SeekData)> = Vec::new();

        for refmutmulti in self.events.iter_mut() {
            let data = refmutmulti.value();
            if let SeekState::WaitingRelayList(author) = data.state {
                if author == pubkey {
                    let id = *refmutmulti.key();
                    if let Ok(relays) = relay::get_best_relays_fixed(author, true) {
                        Self::minion_seek_event_at_relays(id, relays);
                        updates.push((id, SeekData::new_event(data.climb)));
                    }
                }
            }
        }

        for (id, state) in updates.drain(..) {
            let _ = self.events.insert(id, state);
        }
    }

    /// An event was found (you can call this even if the seeker wasn't seeking it)
    pub(crate) fn found(&self, event: &Event) -> Result<(), Error> {
        // Remove the event
        if let Some((_, data)) = self.events.remove(&event.id) {
            // Possibly seek it's parent
            if data.climb {
                let mut eref = EventReference::Id {
                    id: event.id,
                    author: Some(event.pubkey),
                    relays: vec![],
                    marker: None,
                };
                while let Some(event) = GLOBALS.storage.read_event_reference(&eref)? {
                    if let Some(parent_eref) = event.replies_to() {
                        eref = parent_eref;
                        continue;
                    } else {
                        // no missing parent to fetch
                        return Ok(());
                    }
                }

                // FIXME make better use of hints, author hints, etc.
                // we have lost relay information along the way.
                if let EventReference::Id { id, .. } = eref {
                    self.seek_id(id, vec![], true)?;
                }
            }
        }

        Ok(())
    }

    pub(crate) async fn run_once(&self) {
        if self.events.is_empty() {
            return;
        }

        // Instead of updating the map while we iterate (which could deadlock)
        // we save updates here and apply when the iterator is finished.
        let mut updates: Vec<(Id, Option<SeekData>)> = Vec::new();

        let now = Unixtime::now();

        for refmulti in self.events.iter() {
            let id = *refmulti.key();
            let data = refmulti.value();
            match data.state {
                SeekState::WaitingRelayList(author) => {
                    // Check if we have their relays yet
                    match People::person_needs_relay_list(author) {
                        Freshness::Fresh | Freshness::Stale => {
                            if let Ok(relays) = relay::get_best_relays_fixed(author, true) {
                                Self::minion_seek_event_at_relays(id, relays);
                                updates.push((id, Some(SeekData::new_event(data.climb))));
                                continue;
                            }
                        }
                        _ => {}
                    }

                    // If it has been 15 seconds, give up the wait and seek from our READ relays
                    if now - data.start > Duration::from_secs(15) {
                        Self::minion_seek_event_at_our_read_relays(id);
                        updates.push((id, Some(SeekData::new_event(data.climb))));
                    }

                    // Otherwise keep waiting
                }
                SeekState::WaitingEvent => {
                    if now - data.start > Duration::from_secs(15) {
                        tracing::debug!("Failed to find id={}", id.as_hex_string());
                        updates.push((id, None));
                    }
                }
            }
        }

        // Apply updates
        for (id, replacement) in updates.drain(..) {
            match replacement {
                Some(state) => {
                    let _ = self.events.insert(id, state);
                }
                None => {
                    let _ = self.events.remove(&id);
                }
            }
        }
    }
}
