mod import;
mod migrations;

use crate::error::{Error, ErrorKind};
use crate::globals::GLOBALS;
use crate::people::Person;
use crate::person_relay::PersonRelay;
use crate::profile::Profile;
use crate::relationship::Relationship;
use crate::relay::Relay;
use crate::settings::Settings;
use gossip_relay_picker::Direction;
use lmdb::{
    Cursor, Database, DatabaseFlags, Environment, EnvironmentFlags, Stat, Transaction, WriteFlags,
};
use nostr_types::{
    EncryptedPrivateKey, Event, EventKind, Id, MilliSatoshi, PublicKey, PublicKeyHex, RelayUrl,
    Tag, Unixtime,
};
use speedy::{Readable, Writable};
use std::collections::{HashMap, HashSet};

const MAX_LMDB_KEY: usize = 511;
macro_rules! key {
    ($slice:expr) => {
        if $slice.len() > 511 {
            &$slice[..511]
        } else {
            $slice
        }
    };
}

pub struct Storage {
    env: Environment,

    // General database (settings, local_settings)
    general: Database,

    // Id:Url -> Unixtime
    event_seen_on_relay: Database,

    // Id -> ()
    event_viewed: Database,

    // Hashtag -> Id
    // (dup keys, so multiple Ids per hashtag)
    hashtags: Database,

    // Url -> Relay
    relays: Database,

    // Tag -> Id
    // (dup keys, so multiple Ids per tag)
    event_tags: Database,

    // Id -> Event
    events: Database,

    // Id:Id -> Relationship
    relationships: Database,

    // PublicKey -> Person
    people: Database,

    // PublicKey:Url -> PersonRelay
    person_relays: Database,
}

impl Storage {
    pub fn new() -> Result<Storage, Error> {
        let mut builder = Environment::new();

        builder.set_flags(
            EnvironmentFlags::WRITE_MAP, // no nested transactions!
                                         // commits sync. we don't disable any syncing.
        );
        // builder.set_max_readers(126); // this is the default
        builder.set_max_dbs(32);

        // This has to be big enough for all the data.
        // Note that it is the size of the map in VIRTUAL address space,
        //   and that it doesn't all have to be paged in at the same time.
        builder.set_map_size(1048576 * 1024 * 128); // 128 GB

        let env = builder.open(&Profile::current()?.lmdb_dir)?;

        let general = env.create_db(None, DatabaseFlags::empty())?;

        let event_seen_on_relay =
            env.create_db(Some("event_seen_on_relay"), DatabaseFlags::empty())?;

        let event_viewed = env.create_db(Some("event_viewed"), DatabaseFlags::empty())?;

        let hashtags = env.create_db(
            Some("hashtags"),
            DatabaseFlags::DUP_SORT | DatabaseFlags::DUP_FIXED,
        )?;

        let relays = env.create_db(Some("relays"), DatabaseFlags::empty())?;

        let event_tags = env.create_db(
            Some("event_tags"),
            DatabaseFlags::DUP_SORT | DatabaseFlags::DUP_FIXED,
        )?;

        let events = env.create_db(Some("events"), DatabaseFlags::empty())?;

        let relationships = env.create_db(Some("relationships"), DatabaseFlags::empty())?;

        let people = env.create_db(Some("people"), DatabaseFlags::empty())?;

        let person_relays = env.create_db(Some("person_relays"), DatabaseFlags::empty())?;

        let storage = Storage {
            env,
            general,
            event_seen_on_relay,
            event_viewed,
            hashtags,
            relays,
            event_tags,
            events,
            relationships,
            people,
            person_relays,
        };

        // If migration level is missing, we need to import from legacy sqlite
        match storage.read_migration_level()? {
            None => {
                // Import from sqlite
                storage.import()?;
                storage.migrate(0)?;
            }
            Some(level) => {
                storage.migrate(level)?;
            }
        }

        Ok(storage)
    }

    // Remove all events (and related data) with a created_at before `from`
    pub fn prune(&self, from: Unixtime) -> Result<usize, Error> {
        let mut txn = self.env.begin_rw_txn()?;

        // Extract the Ids to delete.
        // We have to extract the Ids and release the cursor on the events database
        // in order to get a cursor on other databases since cursors mutably borrow
        // the transaction
        let mut ids: HashSet<Id> = HashSet::new();

        let mut cursor = txn.open_ro_cursor(self.events)?;
        let iter = cursor.iter_start();
        for result in iter {
            match result {
                Err(e) => return Err(e.into()),
                Ok((_key, val)) => {
                    if let Some(created_at) = Event::get_created_at_from_speedy_bytes(val) {
                        if created_at < from {
                            if let Some(id) = Event::get_id_from_speedy_bytes(val) {
                                ids.insert(id);
                                // Too bad but we can't delete it now, other threads
                                // might try to access it still. We have to delete it from
                                // all the other maps first.
                            }
                        }
                    }
                }
            }
        }
        drop(cursor);

        tracing::info!("PRUNE: deleting {} events", ids.len());

        // Delete from event_seen_on_relay
        let mut deletions: usize = 0;
        for id in &ids {
            let start_key: Vec<u8> = id.as_slice().to_owned();
            let mut cursor = txn.open_rw_cursor(self.event_seen_on_relay)?;
            let iter = cursor.iter_from(start_key.clone());
            for result in iter {
                match result {
                    Err(e) => return Err(e.into()),
                    Ok((key, _val)) => {
                        if !key.starts_with(&start_key) {
                            break;
                        }
                        let _ = cursor.del(WriteFlags::empty());
                        deletions += 1;
                    }
                }
            }
        }
        tracing::info!(
            "PRUNE: deleted {} records from event_seen_on_relay",
            deletions
        );

        // Delete from event_viewed
        for id in &ids {
            let _ = txn.del(self.event_viewed, &id.as_ref(), None);
        }
        tracing::info!("PRUNE: deleted {} records from event_viewed", ids.len());

        // Delete from hashtags
        // (unfortunately since Ids are the values, we have to scan the whole thing)
        let mut cursor = txn.open_rw_cursor(self.hashtags)?;
        let iter = cursor.iter_start();
        let mut deletions: usize = 0;
        for result in iter {
            match result {
                Err(e) => return Err(e.into()),
                Ok((_key, val)) => {
                    let id = Id::read_from_buffer(val)?;
                    if ids.contains(&id) {
                        let _ = cursor.del(WriteFlags::empty());
                        deletions += 1;
                    }
                }
            }
        }
        drop(cursor);
        tracing::info!(
            "PRUNE: deleted {} records from hashtags",
            deletions
        );

        // Delete from event_tags
        // (unfortunately since Ids are the values, we have to scan the whole thing)
        let mut cursor = txn.open_rw_cursor(self.event_tags)?;
        let iter = cursor.iter_start();
        let mut deletions: usize = 0;
        for result in iter {
            match result {
                Err(e) => return Err(e.into()),
                Ok((_key, val)) => {
                    let id = Id::read_from_buffer(val)?;
                    if ids.contains(&id) {
                        let _ = cursor.del(WriteFlags::empty());
                        deletions += 1;
                    }
                }
            }
        }
        drop(cursor);
        tracing::info!("PRUNE: deleted {} records from event_tags", deletions);

        // Delete from relationships
        // (unfortunately because of the 2nd Id in the tag, we have to scan the whole thing)
        let mut cursor = txn.open_rw_cursor(self.relationships)?;
        let iter = cursor.iter_start();
        let mut deletions: usize = 0;
        for result in iter {
            match result {
                Err(e) => return Err(e.into()),
                Ok((key, _val)) => {
                    let id = Id::read_from_buffer(key)?;
                    if ids.contains(&id) {
                        let _ = cursor.del(WriteFlags::empty());
                        deletions += 1;
                        continue;
                    }
                    let id2 = Id::read_from_buffer(&key[32..])?;
                    if ids.contains(&id2) {
                        let _ = cursor.del(WriteFlags::empty());
                        deletions += 1;
                    }
                }
            }
        }
        drop(cursor);
        tracing::info!("PRUNE: deleted {} relationships", deletions);

        // delete from events
        for id in &ids {
            let _ = txn.del(self.events, &id.as_ref(), None);
        }
        tracing::info!("PRUNE: deleted {} records from events", ids.len());

        Ok(ids.len())
    }

    pub fn write_migration_level(&self, migration_level: u32) -> Result<(), Error> {
        let bytes = &migration_level.to_be_bytes();
        let mut txn = self.env.begin_rw_txn()?;
        txn.put(
            self.general,
            b"migration_level",
            &bytes,
            WriteFlags::empty(),
        )?;
        txn.commit()?;
        Ok(())
    }

    pub fn read_migration_level(&self) -> Result<Option<u32>, Error> {
        let txn = self.env.begin_ro_txn()?;
        match txn.get(self.general, b"migration_level") {
            Ok(bytes) => Ok(Some(u32::from_be_bytes(bytes[..4].try_into()?))),
            Err(lmdb::Error::NotFound) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn write_encrypted_private_key(
        &self,
        epk: &Option<EncryptedPrivateKey>,
    ) -> Result<(), Error> {
        let bytes = epk.as_ref().map(|e| &e.0).write_to_vec()?;
        let mut txn = self.env.begin_rw_txn()?;
        txn.put(
            self.general,
            b"encrypted_private_key",
            &bytes,
            WriteFlags::empty(),
        )?;
        txn.commit()?;
        Ok(())
    }

    pub fn read_encrypted_private_key(&self) -> Result<Option<EncryptedPrivateKey>, Error> {
        let txn = self.env.begin_ro_txn()?;
        match txn.get(self.general, b"encrypted_private_key") {
            Ok(bytes) => {
                let os = Option::<String>::read_from_buffer(bytes)?;
                Ok(os.map(EncryptedPrivateKey))
            }
            Err(lmdb::Error::NotFound) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn write_last_contact_list_edit(&self, when: i64) -> Result<(), Error> {
        let bytes = &when.to_be_bytes();
        let mut txn = self.env.begin_rw_txn()?;
        txn.put(
            self.general,
            b"last_contact_list_edit",
            &bytes,
            WriteFlags::empty(),
        )?;
        txn.commit()?;
        Ok(())
    }

    pub fn read_last_contact_list_edit(&self) -> Result<Option<i64>, Error> {
        let txn = self.env.begin_ro_txn()?;
        match txn.get(self.general, b"last_contact_list_edit") {
            Ok(bytes) => Ok(Some(i64::from_be_bytes(bytes[..8].try_into()?))),
            Err(lmdb::Error::NotFound) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn write_settings(&self, settings: &Settings) -> Result<(), Error> {
        let bytes = settings.write_to_vec()?;
        let mut txn = self.env.begin_rw_txn()?;
        txn.put(self.general, b"settings", &bytes, WriteFlags::empty())?;
        txn.commit()?;
        Ok(())
    }

    pub fn read_settings(&self) -> Result<Option<Settings>, Error> {
        let txn = self.env.begin_ro_txn()?;
        match txn.get(self.general, b"settings") {
            Ok(bytes) => Ok(Some(Settings::read_from_buffer(bytes)?)),
            Err(lmdb::Error::NotFound) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn add_event_seen_on_relay(
        &self,
        id: Id,
        url: &RelayUrl,
        when: Unixtime,
    ) -> Result<(), Error> {
        let bytes = &when.0.to_be_bytes();
        let mut key: Vec<u8> = id.as_slice().to_owned();
        let mut txn = self.env.begin_rw_txn()?;
        key.extend(url.0.as_bytes());
        key.truncate(MAX_LMDB_KEY);
        txn.put(self.event_seen_on_relay, &key, &bytes, WriteFlags::empty())?;
        txn.commit()?;
        Ok(())
    }

    pub fn get_event_seen_on_relay(&self, id: Id) -> Result<Vec<(RelayUrl, Unixtime)>, Error> {
        let start_key: Vec<u8> = id.as_slice().to_owned();
        let txn = self.env.begin_ro_txn()?;
        let mut cursor = txn.open_ro_cursor(self.event_seen_on_relay)?;
        let iter = cursor.iter_from(start_key.clone());
        let mut output: Vec<(RelayUrl, Unixtime)> = Vec::new();
        for result in iter {
            match result {
                Err(e) => return Err(e.into()),
                Ok((key, val)) => {
                    // Stop once we get to a different Id
                    if !key.starts_with(&start_key) {
                        break;
                    }
                    // Extract off the Url
                    let url = RelayUrl(std::str::from_utf8(&key[32..])?.to_owned());
                    let time = Unixtime(i64::from_be_bytes(val[..8].try_into()?));
                    output.push((url, time));
                }
            }
        }
        Ok(output)
    }

    pub fn mark_event_viewed(&self, id: Id) -> Result<(), Error> {
        let bytes = vec![];
        let mut txn = self.env.begin_rw_txn()?;
        txn.put(self.event_viewed, &id.as_ref(), &bytes, WriteFlags::empty())?;
        txn.commit()?;
        Ok(())
    }

    pub fn is_event_viewed(&self, id: Id) -> Result<bool, Error> {
        let txn = self.env.begin_ro_txn()?;
        match txn.get(self.event_viewed, &id.as_ref()) {
            Ok(_bytes) => Ok(true),
            Err(lmdb::Error::NotFound) => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    pub fn add_hashtag(&self, hashtag: &String, id: Id) -> Result<(), Error> {
        let key = key!(hashtag.as_bytes());
        let bytes = id.as_slice().to_owned();
        let mut txn = self.env.begin_rw_txn()?;
        txn.put(self.hashtags, &key, &bytes, WriteFlags::empty())?;
        txn.commit()?;
        Ok(())
    }

    pub fn get_event_ids_with_hashtag(&self, hashtag: &String) -> Result<Vec<Id>, Error> {
        let key = key!(hashtag.as_bytes());
        let txn = self.env.begin_ro_txn()?;
        let mut cursor = txn.open_ro_cursor(self.hashtags)?;
        let iter = cursor.iter_from(key);
        let mut output: Vec<Id> = Vec::new();
        for result in iter {
            match result {
                Err(e) => return Err(e.into()),
                Ok((thiskey, val)) => {
                    // Stop once we get to a different key
                    if thiskey != key {
                        break;
                    }
                    let id = Id::read_from_buffer(val)?;
                    output.push(id);
                }
            }
        }
        Ok(output)
    }

    pub fn write_relay(&self, relay: &Relay) -> Result<(), Error> {
        // Note that we use serde instead of speedy because the complexity of the
        // serde_json::Value type makes it difficult. Any other serde serialization
        // should work though: Consider bincode.
        let key = key!(relay.url.0.as_bytes());
        let bytes = serde_json::to_vec(relay)?;
        let mut txn = self.env.begin_rw_txn()?;
        txn.put(self.relays, &key, &bytes, WriteFlags::empty())?;
        txn.commit()?;
        Ok(())
    }

    pub fn write_relay_if_missing(&self, url: &RelayUrl) -> Result<(), Error> {
        if self.read_relay(url)?.is_none() {
            let dbrelay = Relay::new(url.to_owned());
            self.write_relay(&dbrelay)?;
        }
        Ok(())
    }

    pub fn modify_all_relays<M>(&self, mut modify: M) -> Result<(), Error>
    where
        M: FnMut(&mut Relay),
    {
        let mut txn = self.env.begin_rw_txn()?;
        let mut cursor = txn.open_rw_cursor(self.relays)?;
        let iter = cursor.iter_start();
        for result in iter {
            match result {
                Err(e) => return Err(e.into()),
                Ok((key, val)) => {
                    let mut dbrelay: Relay = serde_json::from_slice(val)?;
                    modify(&mut dbrelay);
                    let bytes = serde_json::to_vec(&dbrelay)?;
                    cursor.put(&key, &bytes, WriteFlags::empty())?;
                }
            }
        }
        drop(cursor);
        txn.commit()?;
        Ok(())
    }

    pub fn read_relay(&self, url: &RelayUrl) -> Result<Option<Relay>, Error> {
        // Note that we use serde instead of speedy because the complexity of the
        // serde_json::Value type makes it difficult. Any other serde serialization
        // should work though: Consider bincode.
        let key = key!(url.0.as_bytes());
        let txn = self.env.begin_ro_txn()?;
        match txn.get(self.relays, &key) {
            Ok(bytes) => Ok(Some(serde_json::from_slice(bytes)?)),
            Err(lmdb::Error::NotFound) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn filter_relays<F>(&self, f: F) -> Result<Vec<Relay>, Error>
    where
        F: Fn(&Relay) -> bool,
    {
        let txn = self.env.begin_ro_txn()?;
        let mut cursor = txn.open_ro_cursor(self.relays)?;
        let iter = cursor.iter_start();
        let mut output: Vec<Relay> = Vec::new();
        for result in iter {
            match result {
                Err(e) => return Err(e.into()),
                Ok((_key, val)) => {
                    let relay: Relay = serde_json::from_slice(val)?;
                    if f(&relay) {
                        output.push(relay);
                    }
                }
            }
        }
        Ok(output)
    }

    pub fn write_event_tags(&self, event: &Event) -> Result<(), Error> {
        let mut txn = self.env.begin_rw_txn()?;
        for tag in &event.tags {
            let mut tagbytes = serde_json::to_vec(&tag)?;
            tagbytes.truncate(MAX_LMDB_KEY);
            let bytes = event.id.write_to_vec()?;
            txn.put(self.event_tags, &tagbytes, &bytes, WriteFlags::empty())?;
        }
        txn.commit()?;
        Ok(())
    }

    /// This finds events that have a tag starting with the values in the
    /// passed in tag, and potentially having more tag fields.
    pub fn find_events_with_tags(&self, tag: Tag) -> Result<Vec<Id>, Error> {
        let mut start_key = serde_json::to_vec(&tag)?;
        // remove trailing bracket so we match tags with addl fields
        let _ = start_key.pop();
        // remove any trailing empty fields
        while start_key.ends_with(b",\"\"") {
            start_key.truncate(start_key.len() - 3);
        }
        start_key.truncate(MAX_LMDB_KEY);
        let txn = self.env.begin_ro_txn()?;
        let mut cursor = txn.open_ro_cursor(self.event_tags)?;
        let iter = cursor.iter_from(start_key.clone());
        let mut output: Vec<Id> = Vec::new();
        for result in iter {
            match result {
                Err(e) => return Err(e.into()),
                Ok((key, val)) => {
                    // Stop once we get to a non-matching tag
                    if !key.starts_with(&start_key) {
                        break;
                    }
                    // Add the event
                    let id = Id::read_from_buffer(val)?;
                    output.push(id);
                }
            }
        }
        Ok(output)
    }

    pub fn write_event(&self, event: &Event) -> Result<(), Error> {
        let bytes = event.write_to_vec()?;
        let mut txn = self.env.begin_rw_txn()?;
        txn.put(self.events, &event.id.as_ref(), &bytes, WriteFlags::empty())?;
        txn.commit()?;
        Ok(())
    }

    pub fn read_event(&self, id: Id) -> Result<Option<Event>, Error> {
        let txn = self.env.begin_ro_txn()?;
        match txn.get(self.events, &id.as_ref()) {
            Ok(bytes) => Ok(Some(Event::read_from_buffer(bytes)?)),
            Err(lmdb::Error::NotFound) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn get_event_stats(&self) -> Result<Stat, Error> {
        let txn = self.env.begin_ro_txn()?;
        Ok(txn.stat(self.events)?)
    }

    pub fn delete_event(&self, id: Id) -> Result<(), Error> {
        let mut txn = self.env.begin_rw_txn()?;
        let _ = txn.del(self.events, &id.as_ref(), None);
        txn.commit()?;
        Ok(())
    }

    pub fn replace_event(&self, event: &Event) -> Result<bool, Error> {
        if !event.kind.is_replaceable() {
            return Err(ErrorKind::General("Event is not replaceable.".to_owned()).into());
        }

        let existing = self.find_events(&[event.pubkey], &[event.kind], None, |_| true, false)?;

        let mut found_newer = false;
        for old in existing {
            if old.created_at < event.created_at {
                self.delete_event(old.id)?;
            } else {
                found_newer = true;
            }
        }

        if found_newer {
            return Ok(false); // this event is not the latest one.
        }

        self.write_event(event)?;
        Ok(true)
    }

    pub fn replace_parameterized_event(&self, event: &Event) -> Result<bool, Error> {
        if !event.kind.is_parameterized_replaceable() {
            return Err(
                ErrorKind::General("Event is not parameterized replaceable.".to_owned()).into(),
            );
        }

        let param = match event.parameter() {
            None => {
                return Err(ErrorKind::General(
                    "Event of parameterized type does not have a parameter.".to_owned(),
                )
                .into())
            }
            Some(param) => param,
        };

        let existing = self.find_events(
            &[event.pubkey],
            &[event.kind],
            None,
            |e| e.parameter().as_ref() == Some(&param),
            false,
        )?;

        let mut found_newer = false;
        for old in existing {
            if old.created_at < event.created_at {
                self.delete_event(old.id)?;
            } else {
                found_newer = true;
            }
        }

        if found_newer {
            return Ok(false); // this event is not the latest one.
        }

        self.write_event(event)?;
        Ok(true)
    }

    // Find events of interest.
    //
    // If any of `pubkeys`, or `kinds` is not empty, the events must match
    // one of the values in the array. If empty, no match is performed.
    //
    // The function f is run after the matching-so-far events have been deserialized
    // to finish filtering.
    pub fn find_events<F>(
        &self,
        pubkeys: &[PublicKey],
        kinds: &[EventKind],
        since: Option<Unixtime>,
        f: F,
        sort: bool,
    ) -> Result<Vec<Event>, Error>
    where
        F: Fn(&Event) -> bool,
    {
        let txn = self.env.begin_ro_txn()?;
        let mut cursor = txn.open_ro_cursor(self.events)?;
        let iter = cursor.iter_start();
        let mut output: Vec<Event> = Vec::new();
        for result in iter {
            match result {
                Err(e) => return Err(e.into()),
                Ok((_key, val)) => {
                    // Keep only matching `PublicKey`s
                    if !pubkeys.is_empty() {
                        if let Some(pubkey) = Event::get_pubkey_from_speedy_bytes(val) {
                            if !pubkeys.contains(&pubkey) {
                                continue;
                            }
                        } else {
                            continue;
                        }
                    }

                    // Keep only matching `EventKind`s
                    if !kinds.is_empty() {
                        if let Some(kind) = Event::get_kind_from_speedy_bytes(val) {
                            if !kinds.contains(&kind) {
                                continue;
                            }
                        } else {
                            continue;
                        }
                    }

                    // Enforce since
                    if let Some(time) = since {
                        if let Some(created_at) = Event::get_created_at_from_speedy_bytes(val) {
                            if created_at < time {
                                continue;
                            }
                        } else {
                            continue;
                        }
                    }

                    // Passed all of that? Deserialize and apply user function
                    let event = Event::read_from_buffer(val)?;
                    if f(&event) {
                        output.push(event);
                    }
                }
            }
        }

        if sort {
            output.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        }

        Ok(output)
    }

    pub fn search_events(&self, text: &str) -> Result<Vec<Event>, Error> {
        let event_kinds = GLOBALS.settings.read().feed_displayable_event_kinds();

        let needle = regex::escape(text.to_lowercase().as_str());
        let re = regex::RegexBuilder::new(needle.as_str())
            .unicode(true)
            .case_insensitive(true)
            .build()?;

        let txn = self.env.begin_ro_txn()?;
        let mut cursor = txn.open_ro_cursor(self.events)?;
        let iter = cursor.iter_start();
        let mut events: Vec<Event> = Vec::new();
        for result in iter {
            match result {
                Err(e) => return Err(e.into()),
                Ok((_key, val)) => {
                    // event kind must match
                    if let Some(kind) = Event::get_kind_from_speedy_bytes(val) {
                        if !event_kinds.contains(&kind) {
                            continue;
                        }
                    } else {
                        continue;
                    }

                    if let Some(content) = Event::get_content_from_speedy_bytes(val) {
                        if re.is_match(content.as_ref()) {
                            let event = Event::read_from_buffer(val)?;
                            events.push(event);
                            continue;
                        }
                    }

                    if Event::tag_search_in_speedy_bytes(val, &re)? {
                        let event = Event::read_from_buffer(val)?;
                        events.push(event);
                    }
                }
            }
        }

        events.sort_unstable_by(|a, b| {
            // ORDER created_at desc
            b.created_at.cmp(&a.created_at)
        });

        Ok(events)
    }

    // TBD: optimize this by storing better event indexes
    // currently we stupidly scan every event (just to get LMDB up and running first)
    pub fn fetch_contact_list(&self, pubkey: &PublicKey) -> Result<Option<Event>, Error> {
        Ok(self
            .find_events(&[*pubkey], &[EventKind::ContactList], None, |_| true, false)?
            .iter()
            .max_by(|x, y| x.created_at.cmp(&y.created_at))
            .cloned())
    }

    // This is temporary to feed src/events.rs which will be going away in a future
    // code pass
    pub fn fetch_reply_related_events(&self, since: Unixtime) -> Result<Vec<Event>, Error> {
        let public_key: PublicKeyHex = match GLOBALS.signer.public_key() {
            None => return Ok(vec![]),
            Some(pk) => pk.into(),
        };

        let reply_related_kinds = GLOBALS.settings.read().feed_related_event_kinds();

        let tag = Tag::Pubkey {
            pubkey: public_key,
            recommended_relay_url: None,
            petname: None,
            trailing: vec![],
        };

        let tagged_event_ids = self.find_events_with_tags(tag)?;

        let events: Vec<Event> = tagged_event_ids
            .iter()
            .filter_map(|id| match self.read_event(*id) {
                Ok(Some(event)) => {
                    if event.created_at > since && reply_related_kinds.contains(&event.kind) {
                        Some(event)
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .collect();

        Ok(events)
    }

    // This is temporary to feed src/events.rs which will be going away in a future
    // code pass
    pub fn fetch_relay_lists(&self) -> Result<Vec<Event>, Error> {
        let mut relay_lists =
            self.find_events(&[], &[EventKind::RelayList], None, |_| true, false)?;

        let mut latest: HashMap<PublicKey, Event> = HashMap::new();
        for event in relay_lists.drain(..) {
            if let Some(current_best) = latest.get(&event.pubkey) {
                if current_best.created_at >= event.created_at {
                    continue;
                }
            }
            let _ = latest.insert(event.pubkey, event);
        }

        Ok(latest.values().map(|v| v.to_owned()).collect())
    }

    pub fn get_highest_local_parent_event_id(&self, id: Id) -> Result<Option<Id>, Error> {
        let event = match self.read_event(id)? {
            Some(event) => event,
            None => return Ok(None),
        };

        if let Some((parent_id, _opturl)) = event.replies_to() {
            self.get_highest_local_parent_event_id(parent_id)
        } else {
            Ok(Some(event.id))
        }
    }

    pub fn write_relationship(
        &self,
        id: Id,
        related: Id,
        relationship: Relationship,
    ) -> Result<(), Error> {
        let mut key = id.as_ref().as_slice().to_owned();
        key.extend(related.as_ref());
        let value = relationship.write_to_vec()?;
        let mut txn = self.env.begin_rw_txn()?;
        txn.put(self.relationships, &key, &value, WriteFlags::empty())?;
        txn.commit()?;
        Ok(())
    }

    pub fn find_relationships(&self, id: Id) -> Result<Vec<(Id, Relationship)>, Error> {
        let start_key = id.as_ref().to_owned();
        let txn = self.env.begin_ro_txn()?;
        let mut cursor = txn.open_ro_cursor(self.relationships)?;
        let iter = cursor.iter_from(start_key);
        let mut output: Vec<(Id, Relationship)> = Vec::new();
        for result in iter {
            match result {
                Err(e) => return Err(e.into()),
                Ok((key, val)) => {
                    if !key.starts_with(&start_key) {
                        break;
                    }
                    let id2 = Id(key[32..64].try_into().unwrap());
                    let relationship = Relationship::read_from_buffer(val)?;
                    output.push((id2, relationship));
                }
            }
        }
        Ok(output)
    }

    pub fn get_replies(&self, id: Id) -> Result<Vec<Id>, Error> {
        Ok(self
            .find_relationships(id)?
            .iter()
            .filter_map(|(id, rel)| {
                if *rel == Relationship::Reply {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect())
    }

    /// Returns the list of reactions and whether or not this account has already reacted to this event
    pub fn get_reactions(&self, id: Id) -> Result<(Vec<(char, usize)>, bool), Error> {
        // Whether or not the Gossip user already reacted to this event
        let mut self_already_reacted = false;

        // Collect up to one reaction per pubkey
        let mut phase1: HashMap<PublicKey, char> = HashMap::new();
        for (_, rel) in self.find_relationships(id)? {
            if let Relationship::Reaction(pubkey, reaction) = rel {
                let symbol: char = if let Some(ch) = reaction.chars().next() {
                    ch
                } else {
                    '+'
                };
                phase1.insert(pubkey, symbol);
                if Some(pubkey) == GLOBALS.signer.public_key() {
                    self_already_reacted = true;
                }
            }
        }

        // Collate by char
        let mut output: HashMap<char, usize> = HashMap::new();
        for (_, symbol) in phase1 {
            output
                .entry(symbol)
                .and_modify(|count| *count += 1)
                .or_insert_with(|| 1);
        }

        let mut v: Vec<(char, usize)> = output.drain().collect();
        v.sort();
        Ok((v, self_already_reacted))
    }

    pub fn get_zap_total(&self, id: Id) -> Result<MilliSatoshi, Error> {
        let mut total = MilliSatoshi(0);
        for (_, rel) in self.find_relationships(id)? {
            if let Relationship::ZapReceipt(_pk, millisats) = rel {
                total = total + millisats;
            }
        }
        Ok(total)
    }

    pub fn get_deletion(&self, id: Id) -> Result<Option<String>, Error> {
        for (_, rel) in self.find_relationships(id)? {
            if let Relationship::Deletion(deletion) = rel {
                return Ok(Some(deletion.clone()));
            }
        }
        Ok(None)
    }

    // This returns IDs that should be UI invalidated
    pub fn process_relationships_of_event(&self, event: &Event) -> Result<Vec<Id>, Error> {
        let mut invalidate: Vec<Id> = Vec::new();

        // replies to
        if let Some((id, _)) = event.replies_to() {
            self.write_relationship(id, event.id, Relationship::Reply)?;
        }

        // reacts to
        if let Some((id, reaction, _maybe_url)) = event.reacts_to() {
            self.write_relationship(id, event.id, Relationship::Reaction(event.pubkey, reaction))?;

            invalidate.push(id);
        }

        // deletes
        if let Some((ids, reason)) = event.deletes() {
            invalidate.extend(&ids);

            for id in ids {
                // since it is a delete, we don't actually desire the event.

                self.write_relationship(id, event.id, Relationship::Deletion(reason.clone()))?;
            }
        }

        // zaps
        match event.zaps() {
            Ok(Some(zapdata)) => {
                self.write_relationship(
                    zapdata.id,
                    event.id,
                    Relationship::ZapReceipt(event.pubkey, zapdata.amount),
                )?;

                invalidate.push(zapdata.id);
            }
            Err(e) => tracing::error!("Invalid zap receipt: {}", e),
            _ => {}
        }

        Ok(invalidate)
    }

    pub fn write_person(&self, person: &Person) -> Result<(), Error> {
        // Note that we use serde instead of speedy because the complexity of the
        // serde_json::Value type makes it difficult. Any other serde serialization
        // should work though: Consider bincode.
        let key: Vec<u8> = person.pubkey.as_bytes();
        let bytes = serde_json::to_vec(person)?;
        let mut txn = self.env.begin_rw_txn()?;
        txn.put(self.people, &key, &bytes, WriteFlags::empty())?;
        txn.commit()?;
        Ok(())
    }

    pub fn read_person(&self, pubkey: &PublicKey) -> Result<Option<Person>, Error> {
        // Note that we use serde instead of speedy because the complexity of the
        // serde_json::Value type makes it difficult. Any other serde serialization
        // should work though: Consider bincode.
        let key: Vec<u8> = pubkey.as_bytes();
        let txn = self.env.begin_ro_txn()?;
        match txn.get(self.people, &key) {
            Ok(bytes) => Ok(Some(serde_json::from_slice(bytes)?)),
            Err(lmdb::Error::NotFound) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn write_person_if_missing(&self, pubkey: &PublicKey) -> Result<(), Error> {
        if self.read_person(pubkey)?.is_none() {
            let person = Person::new(pubkey.to_owned());
            self.write_person(&person)?;
        }
        Ok(())
    }

    pub fn filter_people<F>(&self, f: F) -> Result<Vec<Person>, Error>
    where
        F: Fn(&Person) -> bool,
    {
        let txn = self.env.begin_ro_txn()?;
        let mut cursor = txn.open_ro_cursor(self.people)?;
        let iter = cursor.iter_start();
        let mut output: Vec<Person> = Vec::new();
        for result in iter {
            match result {
                Err(e) => return Err(e.into()),
                Ok((_key, val)) => {
                    let person: Person = serde_json::from_slice(val)?;
                    if f(&person) {
                        output.push(person);
                    }
                }
            }
        }
        Ok(output)
    }

    pub fn get_people_stats(&self) -> Result<Stat, Error> {
        let txn = self.env.begin_ro_txn()?;
        Ok(txn.stat(self.people)?)
    }

    pub fn write_person_relay(&self, person_relay: &PersonRelay) -> Result<(), Error> {
        let mut key = person_relay.pubkey.as_bytes();
        key.extend(person_relay.url.0.as_bytes());
        let bytes = person_relay.write_to_vec()?;
        let mut txn = self.env.begin_rw_txn()?;
        txn.put(self.person_relays, &key, &bytes, WriteFlags::empty())?;
        txn.commit()?;
        Ok(())
    }

    pub fn read_person_relay(
        &self,
        pubkey: PublicKey,
        url: &RelayUrl,
    ) -> Result<Option<PersonRelay>, Error> {
        let mut key = pubkey.as_bytes();
        key.extend(url.0.as_bytes());
        let txn = self.env.begin_ro_txn()?;
        match txn.get(self.person_relays, &key) {
            Ok(bytes) => Ok(Some(PersonRelay::read_from_buffer(bytes)?)),
            Err(lmdb::Error::NotFound) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn get_person_relays(&self, pubkey: PublicKey) -> Result<Vec<PersonRelay>, Error> {
        let start_key = pubkey.as_bytes();
        let txn = self.env.begin_ro_txn()?;
        let mut cursor = txn.open_ro_cursor(self.person_relays)?;
        let iter = cursor.iter_from(start_key.clone());
        let mut output: Vec<PersonRelay> = Vec::new();
        for result in iter {
            match result {
                Err(e) => return Err(e.into()),
                Ok((key, val)) => {
                    // Stop once we get to a different pubkey
                    if !key.starts_with(&start_key) {
                        break;
                    }
                    let person_relay = PersonRelay::read_from_buffer(val)?;
                    output.push(person_relay);
                }
            }
        }
        Ok(output)
    }

    pub fn set_relay_list(
        &self,
        pubkey: PublicKey,
        read_relays: Vec<RelayUrl>,
        write_relays: Vec<RelayUrl>,
    ) -> Result<(), Error> {
        let mut person_relays = self.get_person_relays(pubkey)?;
        for mut pr in person_relays.drain(..) {
            let orig_read = pr.read;
            let orig_write = pr.write;
            pr.read = read_relays.contains(&pr.url);
            pr.write = write_relays.contains(&pr.url);
            if pr.read != orig_read || pr.write != orig_write {
                self.write_person_relay(&pr)?;
            }
        }
        Ok(())
    }

    /*
    pub fn set_person_relay_manual_pairing(
        &self,
        pubkey: PublicKey,
        read_relays: Vec<RelayUrl>,
        write_relays: Vec<RelayUrl>,
    ) -> Result<(), Error> {
        let mut person_relays = self.get_person_relays(pubkey)?;
        for mut pr in person_relays.drain(..) {
            let orig_read = pr.manually_paired_read;
            let orig_write = pr.manually_paired_write;
            pr.manually_paired_read = read_relays.contains(&pr.url);
            pr.manually_paired_write = write_relays.contains(&pr.url);
            if pr.manually_paired_read != orig_read || pr.manually_paired_write != orig_write {
                self.write_person_relay(&pr)?;
            }
        }
        Ok(())
    }
     */

    /// This returns the relays for a person, along with a score, in order of score
    pub fn get_best_relays(
        &self,
        pubkey: PublicKey,
        dir: Direction,
    ) -> Result<Vec<(RelayUrl, u64)>, Error> {
        let person_relays = self.get_person_relays(pubkey)?;
        let mut ranked_relays = match dir {
            Direction::Write => PersonRelay::write_rank(person_relays),
            Direction::Read => PersonRelay::read_rank(person_relays),
        };

        let num_relays_per_person = GLOBALS.settings.read().num_relays_per_person as usize;

        // If we can't get enough of them, extend with some of our relays
        // at whatever the lowest score of their last one was
        if ranked_relays.len() < (num_relays_per_person + 1) {
            let how_many_more = (num_relays_per_person + 1) - ranked_relays.len();
            let last_score = if ranked_relays.is_empty() {
                20
            } else {
                ranked_relays[ranked_relays.len() - 1].1
            };
            match dir {
                Direction::Write => {
                    // substitute our read relays
                    let additional: Vec<(RelayUrl, u64)> = self
                        .filter_relays(|r| {
                            // not already in their list
                            !ranked_relays.iter().any(|(url, _)| *url == r.url)
                                && r.has_usage_bits(Relay::READ)
                        })?
                        .iter()
                        .map(|r| (r.url.clone(), last_score))
                        .take(how_many_more)
                        .collect();

                    ranked_relays.extend(additional);
                }
                Direction::Read => {
                    // substitute our write relays???
                    let additional: Vec<(RelayUrl, u64)> = self
                        .filter_relays(|r| {
                            // not already in their list
                            !ranked_relays.iter().any(|(url, _)| *url == r.url)
                                && r.has_usage_bits(Relay::WRITE)
                        })?
                        .iter()
                        .map(|r| (r.url.clone(), last_score))
                        .take(how_many_more)
                        .collect();

                    ranked_relays.extend(additional);
                }
            }
        }

        Ok(ranked_relays)
    }

    fn disable_sync(&self) -> Result<(), Error> {
        self.set_flags(
            EnvironmentFlags::NO_SYNC | EnvironmentFlags::NO_META_SYNC,
            true,
        )
    }

    fn enable_sync(&self) -> Result<(), Error> {
        // Sync the data. If we have a system crash before the migration level
        // is written in the next line, import will start over.
        self.env.sync(true)?;

        self.set_flags(
            EnvironmentFlags::NO_SYNC | EnvironmentFlags::NO_META_SYNC,
            false,
        )
    }

    fn set_flags(&self, flags: EnvironmentFlags, on: bool) -> Result<(), Error> {
        let result = unsafe {
            lmdb_sys::mdb_env_set_flags(self.env.env(), flags.bits(), if on { 1 } else { 0 })
        };
        if result != 0 {
            return Err(ErrorKind::General("Unable to set LMDB flags".to_owned()).into());
        }
        Ok(())
    }
}
