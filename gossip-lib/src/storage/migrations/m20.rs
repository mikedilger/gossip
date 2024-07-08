use crate::error::{Error, ErrorKind};
use crate::globals::GLOBALS;
use crate::storage::types::PersonList1;
use crate::storage::Storage;
use heed::RwTxn;
use nostr_types::{EventKind, EventV2, Id, PublicKey, TagV2, Unixtime};
use speedy::Readable;
use std::collections::HashSet;
use std::ops::Bound;

impl Storage {
    pub(super) fn m20_trigger(&self) -> Result<(), Error> {
        let _ = self.db_person_lists_metadata1()?;
        let _ = self.db_events2()?;
        Ok(())
    }

    pub(super) fn m20_migrate<'a>(
        &'a self,
        prefix: &str,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: initializing person list event metadata...");

        // Migrate
        self.m20_initialize_person_list_event_metadata(txn)?;

        Ok(())
    }

    /// Get the matching replaceable event (possibly parameterized)
    /// TBD: optimize this by storing better event indexes
    pub fn m20_get_replaceable_event2(
        &self,
        kind: EventKind,
        pubkey: PublicKey,
        parameter: &str,
    ) -> Result<Option<EventV2>, Error> {
        if !kind.is_replaceable() {
            return Err(ErrorKind::General("Event kind is not replaceable".to_owned()).into());
        }

        Ok(self
            .m20_find_events2(
                &[kind],
                &[pubkey],
                None, // any time
                |e| {
                    if kind.is_parameterized_replaceable() {
                        e.parameter().as_deref() == Some(parameter)
                    } else {
                        true
                    }
                },
                true, // sorted in reverse time order
            )?
            .first()
            .cloned())
    }

    pub fn m20_find_events2<F>(
        &self,
        kinds: &[EventKind],
        pubkeys: &[PublicKey],
        since: Option<Unixtime>,
        f: F,
        sort: bool,
    ) -> Result<Vec<EventV2>, Error>
    where
        F: Fn(&EventV2) -> bool,
    {
        let ids = self.m20_find_event_ids(kinds, pubkeys, since)?;

        // Now that we have that Ids, fetch the events
        let txn = self.env.read_txn()?;
        let mut events: Vec<EventV2> = Vec::new();
        for id in ids {
            // this is like self.read_event(), but we supply our existing transaction
            if let Some(bytes) = self.db_events2()?.get(&txn, id.as_slice())? {
                let event = EventV2::read_from_buffer(bytes)?;
                if f(&event) {
                    events.push(event);
                }
            }
        }

        if sort {
            events.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(b.id.cmp(&a.id)));
        }

        Ok(events)
    }

    fn m20_initialize_person_list_event_metadata<'a>(
        &'a self,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        // Get public key, or give up
        let pk = match self.read_setting_public_key() {
            Some(pk) => pk,
            None => return Ok(()),
        };

        for (list, mut metadata) in self.get_all_person_list_metadata1()? {
            if let Ok(Some(event)) =
                self.m20_get_replaceable_event2(list.event_kind(), pk, &metadata.dtag)
            {
                metadata.event_created_at = event.created_at;
                metadata.event_public_len = event
                    .tags
                    .iter()
                    .filter(|t| matches!(t, TagV2::Pubkey { .. }))
                    .count();
                metadata.event_private_len = {
                    let mut private_len: Option<usize> = None;
                    if !matches!(list, PersonList1::Followed) && GLOBALS.identity.is_unlocked() {
                        if let Ok(bytes) = GLOBALS.identity.decrypt(&pk, &event.content) {
                            if let Ok(vectags) = serde_json::from_str::<Vec<TagV2>>(&bytes) {
                                private_len = Some(vectags.len());
                            }
                        }
                    }
                    private_len
                };
                self.set_person_list_metadata1(list, &metadata, Some(txn))?;
            }
        }

        Ok(())
    }

    pub fn m20_find_event_ids(
        &self,
        kinds: &[EventKind],
        pubkeys: &[PublicKey],
        since: Option<Unixtime>,
    ) -> Result<HashSet<Id>, Error> {
        if kinds.is_empty() {
            return Err(ErrorKind::General(
                "find_events() requires some event kinds to be specified.".to_string(),
            )
            .into());
        }

        // Get the Ids
        let ids = match (pubkeys.is_empty(), since) {
            (true, None) => self.m20_find_ek_pk_events(kinds, pubkeys)?,
            (true, Some(when)) => self.m20_find_ek_c_events(kinds, when)?,
            (false, None) => self.m20_find_ek_pk_events(kinds, pubkeys)?,
            (false, Some(when)) => {
                let group1 = self.m20_find_ek_pk_events(kinds, pubkeys)?;
                let group2 = self.m20_find_ek_c_events(kinds, when)?;
                group1.intersection(&group2).copied().collect()
            }
        };

        Ok(ids)
    }

    /// Find events of given kinds and pubkeys.
    /// You must supply kinds. You can skip the pubkeys and then only kinds will matter.
    fn m20_find_ek_pk_events(
        &self,
        kinds: &[EventKind],
        pubkeys: &[PublicKey],
    ) -> Result<HashSet<Id>, Error> {
        if kinds.is_empty() {
            return Err(ErrorKind::General(
                "find_ek_pk_events() requires some event kinds to be specified.".to_string(),
            )
            .into());
        }

        let mut ids: HashSet<Id> = HashSet::new();
        let txn = self.env.read_txn()?;

        for kind in kinds {
            let ek: u32 = (*kind).into();
            if pubkeys.is_empty() {
                let start_key = ek.to_be_bytes().as_slice().to_owned();
                let iter = self
                    .db_event_ek_pk_index1()?
                    .prefix_iter(&txn, &start_key)?;
                for result in iter {
                    let (_key, val) = result?;
                    // Take the event
                    let id = Id(val[0..32].try_into()?);
                    ids.insert(id);
                }
            } else {
                for pubkey in pubkeys {
                    let mut start_key = ek.to_be_bytes().as_slice().to_owned();
                    start_key.extend(pubkey.as_bytes());
                    let iter = self
                        .db_event_ek_pk_index1()?
                        .prefix_iter(&txn, &start_key)?;
                    for result in iter {
                        let (_key, val) = result?;
                        // Take the event
                        let id = Id(val[0..32].try_into()?);
                        ids.insert(id);
                    }
                }
            }
        }

        Ok(ids)
    }

    /// Find events of given kinds and after the given time.
    fn m20_find_ek_c_events(
        &self,
        kinds: &[EventKind],
        since: Unixtime,
    ) -> Result<HashSet<Id>, Error> {
        if kinds.is_empty() {
            return Err(ErrorKind::General(
                "find_ek_c_events() requires some event kinds to be specified.".to_string(),
            )
            .into());
        }

        let now = Unixtime::now();
        let mut ids: HashSet<Id> = HashSet::new();
        let txn = self.env.read_txn()?;

        for kind in kinds {
            let ek: u32 = (*kind).into();
            let mut start_key = ek.to_be_bytes().as_slice().to_owned();
            let mut end_key = start_key.clone();
            start_key.extend((i64::MAX - now.0).to_be_bytes().as_slice()); // work back from now
            end_key.extend((i64::MAX - since.0).to_be_bytes().as_slice()); // until since
            let range = (Bound::Included(&*start_key), Bound::Excluded(&*end_key));
            let iter = self.db_event_ek_c_index1()?.range(&txn, &range)?;
            for result in iter {
                let (_key, val) = result?;
                // Take the event
                let id = Id(val[0..32].try_into()?);
                ids.insert(id);
            }
        }

        Ok(ids)
    }
}
