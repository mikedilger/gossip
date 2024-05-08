use crate::comms::ToOverlordMessage;
use crate::error::Error;
use crate::globals::GLOBALS;
use crate::misc::Freshness;
use crate::people::People;
use crate::relay::Relay;
use dashmap::DashMap;
use nostr_types::{EventAddr, Id, PublicKey, RelayUrl, RelayUsage, Unixtime};
use std::time::Duration;
use tokio::time::Instant;

#[derive(Debug, Clone)]
pub enum SeekState {
    WaitingRelayList(PublicKey),
    WaitingEvent,
}

#[derive(Debug, Clone)]
pub struct SeekData {
    pub start: Unixtime,
    pub state: SeekState,
}

impl SeekData {
    fn new_event() -> SeekData {
        SeekData {
            start: Unixtime::now().unwrap(),
            state: SeekState::WaitingEvent,
        }
    }

    fn new_relay_list(pubkey: PublicKey) -> SeekData {
        SeekData {
            start: Unixtime::now().unwrap(),
            state: SeekState::WaitingRelayList(pubkey),
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

    fn get_relays(author: PublicKey) -> Result<Vec<RelayUrl>, Error> {
        Ok(GLOBALS
            .storage
            .get_best_relays(author, RelayUsage::Outbox)?
            .iter()
            .map(|(r, _)| r.to_owned())
            .collect())
    }

    fn seek_relay_list(author: PublicKey) {
        let _ = GLOBALS
            .to_overlord
            .send(ToOverlordMessage::SubscribeDiscover(vec![author], None));
    }

    fn seek_event_at_our_read_relays(id: Id) {
        let _ = GLOBALS
            .to_overlord
            .send(ToOverlordMessage::FetchEvent(id, vec![]));
    }

    fn seek_event_at_relays(id: Id, relays: Vec<RelayUrl>) {
        let _ = GLOBALS
            .to_overlord
            .send(ToOverlordMessage::FetchEvent(id, relays));
    }

    /// Seek an event when you only have the `Id`
    pub(crate) fn seek_id(&self, id: Id, speculative_relays: Vec<RelayUrl>) -> Result<(), Error> {
        if self.events.get(&id).is_some() {
            return Ok(()); // we are already seeking this event
        }

        tracing::debug!("Seeking id={}", id.as_hex_string());

        let mut relays: Vec<RelayUrl> = GLOBALS
            .storage
            .filter_relays(|r| r.has_usage_bits(Relay::READ) && r.rank != 0)?
            .iter()
            .map(|relay| relay.url.clone())
            .collect();
        relays.extend(speculative_relays);
        Self::seek_event_at_relays(id, relays);

        // Remember when we asked
        self.events.insert(id, SeekData::new_event());

        Ok(())
    }

    /// Seek an event when you have the `Id` and the author `PublicKey`
    /// Additional relays can be passed in and the event will also be sought there
    pub(crate) fn seek_id_and_author(
        &self,
        id: Id,
        author: PublicKey,
        speculative_relays: Vec<RelayUrl>,
    ) -> Result<(), Error> {
        // Start speculative seek (this is untracked. We will track the by author
        // seek process instead. BUT if the event comes in, it does cancel).
        if !speculative_relays.is_empty() {
            Self::seek_event_at_relays(id, speculative_relays);
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
                Self::seek_relay_list(author);
                self.events
                    .insert(id, SeekData::new_relay_list(author));
            }
            Freshness::Stale => {
                // Seek the relay list because it is stale, but don't let that hold us up
                // using the stale data
                Self::seek_relay_list(author);

                let relays = Self::get_relays(author)?;
                Self::seek_event_at_relays(id, relays);
                self.events.insert(id, SeekData::new_event());
            }
            Freshness::Fresh => {
                let relays = Self::get_relays(author)?;
                Self::seek_event_at_relays(id, relays);
                self.events.insert(id, SeekData::new_event());
            }
        }

        Ok(())
    }

    /// Seek an event when you have the `Id` and the relays to seek from
    pub(crate) fn seek_id_and_relays(&self, id: Id, relays: Vec<RelayUrl>) {
        if let Some(existing) = self.events.get(&id) {
            if matches!(existing.value().state, SeekState::WaitingEvent) {
                return; // Already seeking it
            }
        }
        Self::seek_event_at_relays(id, relays);
        self.events.insert(id, SeekData::new_event());
    }

    /// Seek an event when you have an EventAddr
    pub(crate) fn seek_event_addr(&self, addr: EventAddr) {
        let _ = GLOBALS
            .to_overlord
            .send(ToOverlordMessage::FetchEventAddr(addr));
    }

    /// Inform the seeker that an author's relay list has just arrived
    pub(crate) fn found_author_relays(&self, pubkey: PublicKey) {
        // Instead of updating the map while we iterate (which could deadlock)
        // we save updates here and apply when the iterator is finished.
        let mut updates: Vec<(Id, SeekData)> = Vec::new();

        for refmutmulti in self.events.iter_mut() {
            if let SeekData { state: SeekState::WaitingRelayList(author), .. } = refmutmulti.value() {
                if *author == pubkey {
                    let id = *refmutmulti.key();
                    if let Ok(relays) = Self::get_relays(*author) {
                        Self::seek_event_at_relays(id, relays);
                        updates.push((id, SeekData::new_event()));
                    }
                }
            }
        }

        for (id, state) in updates.drain(..) {
            let _ = self.events.insert(id, state);
        }
    }

    pub(crate) fn found_or_cancel(&self, id: Id) {
        self.events.remove(&id);
    }

    pub(crate) fn start() {
        tracing::info!("Seeker startup");

        // Setup periodic queue management
        tokio::task::spawn(async move {
            let mut read_runstate = GLOBALS.read_runstate.clone();
            read_runstate.mark_unchanged();
            if read_runstate.borrow().going_offline() {
                return;
            }

            let sleep = tokio::time::sleep(Duration::from_millis(1000));
            tokio::pin!(sleep);

            loop {
                tokio::select! {
                    _ = &mut sleep => {
                        sleep.as_mut().reset(Instant::now() + Duration::from_millis(1000));
                    },
                    _ = read_runstate.wait_for(|runstate| runstate.going_offline()) => break,
                }

                GLOBALS.seeker.run_once().await;
            }

            tracing::info!("Seeker shutdown");
        });
    }

    pub(crate) async fn run_once(&self) {
        if self.events.is_empty() {
            return;
        }

        // Instead of updating the map while we iterate (which could deadlock)
        // we save updates here and apply when the iterator is finished.
        let mut updates: Vec<(Id, Option<SeekData>)> = Vec::new();

        let now = Unixtime::now().unwrap();

        for refmulti in self.events.iter() {
            let id = *refmulti.key();
            let data = refmulti.value();
            match data.state {
                SeekState::WaitingRelayList(author) => {
                    // Check if we have their relays yet
                    match People::person_needs_relay_list(author) {
                        Freshness::Fresh | Freshness::Stale => {
                            if let Ok(relays) = Self::get_relays(author) {
                                Self::seek_event_at_relays(id, relays);
                                updates.push((id, Some(SeekData::new_event())));
                                continue;
                            }
                        }
                        _ => {}
                    }

                    // If it has been 15 seconds, give up the wait and seek from our READ relays
                    if now - data.start > Duration::from_secs(15) {
                        Self::seek_event_at_our_read_relays(id);
                        updates.push((id, Some(SeekData::new_event())));
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
