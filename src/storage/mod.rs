mod import;

use crate::db::DbRelay;
use crate::error::Error;
use crate::profile::Profile;
use crate::settings::Settings;
use lmdb::{
    Cursor, Database, DatabaseFlags, Environment, EnvironmentFlags, Transaction, WriteFlags,
};
use nostr_types::{EncryptedPrivateKey, Event, Id, RelayUrl, Tag, Unixtime};
use speedy::{Readable, Writable};

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

    // Url -> DbRelay
    relays: Database,

    // Tag -> Id
    // (dup keys, so multiple Ids per tag)
    event_tags: Database,

    // Id -> Event
    events: Database,
}

impl Storage {
    pub fn new() -> Result<Storage, Error> {
        let mut builder = Environment::new();

        builder.set_flags(
            EnvironmentFlags::WRITE_MAP | // no nested transactions!
            EnvironmentFlags::NO_META_SYNC |
            EnvironmentFlags::MAP_ASYNC,
        );
        // builder.set_max_readers(126); // this is the default
        builder.set_max_dbs(32);

        // This has to be big enough for all the data.
        // Note that it is the size of the map in VIRTUAL address space,
        //   and that it doesn't all have to be paged in at the same time.
        builder.set_map_size(1048576 * 1024 * 2); // 2 GB (probably too small)

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

        let storage = Storage {
            env,
            general,
            event_seen_on_relay,
            event_viewed,
            hashtags,
            relays,
            event_tags,
            events,
        };

        // If migration level is missing, we need to import from legacy sqlite
        match storage.read_migration_level()? {
            None => {
                // Import from sqlite
                storage.import()?;
            }
            Some(_level) => {
                // migrations happen here
            }
        }

        Ok(storage)
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

    pub fn write_relay(&self, relay: &DbRelay) -> Result<(), Error> {
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
            let dbrelay = DbRelay::new(url.to_owned());
            self.write_relay(&dbrelay)?;
        }
        Ok(())
    }

    pub fn modify_all_relays<M>(&self, mut modify: M) -> Result<(), Error>
    where
        M: FnMut(&mut DbRelay),
    {
        let mut txn = self.env.begin_rw_txn()?;
        let mut cursor = txn.open_rw_cursor(self.relays)?;
        let iter = cursor.iter_start();
        for result in iter {
            match result {
                Err(e) => return Err(e.into()),
                Ok((key, val)) => {
                    let mut dbrelay: DbRelay = serde_json::from_slice(val)?;
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

    pub fn read_relay(&self, url: &RelayUrl) -> Result<Option<DbRelay>, Error> {
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

    pub fn filter_relays<F>(&self, f: F) -> Result<Vec<DbRelay>, Error>
    where
        F: Fn(&DbRelay) -> bool,
    {
        let txn = self.env.begin_ro_txn()?;
        let mut cursor = txn.open_ro_cursor(self.relays)?;
        let iter = cursor.iter_start();
        let mut output: Vec<DbRelay> = Vec::new();
        for result in iter {
            match result {
                Err(e) => return Err(e.into()),
                Ok((_key, val)) => {
                    let relay: DbRelay = serde_json::from_slice(val)?;
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

    pub fn filter_events<F>(&self, f: F) -> Result<Vec<Event>, Error>
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
                    let event = Event::read_from_buffer(val)?;
                    if f(&event) {
                        output.push(event);
                    }
                }
            }
        }
        Ok(output)
    }
}
