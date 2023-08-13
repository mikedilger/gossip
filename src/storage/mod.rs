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
    Cursor, Database, DatabaseFlags, Environment, EnvironmentFlags, RwTransaction, Stat,
    Transaction, WriteFlags,
};
use nostr_types::{
    EncryptedPrivateKey, Event, EventAddr, EventKind, Id, MilliSatoshi, PublicKey, RelayUrl,
    Unixtime,
};
use speedy::{Readable, Writable};
use std::collections::{HashMap, HashSet};

const MAX_LMDB_KEY: usize = 511;
macro_rules! key {
    ($slice:expr) => {
        if $slice.len() > 511 {
            &$slice[..=510]
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
    //   key: key!(id.as_slice(), url.0.as_bytes())
    //   val: unixtime.0.to_be_bytes()
    event_seen_on_relay: Database,

    // Id -> ()
    //   key: id.as_slice()
    //   val: vec![]
    event_viewed: Database,

    // Hashtag -> Id
    // (dup keys, so multiple Ids per hashtag)
    //   key: key!(hashtag.as_bytes())
    //   val: id.as_slice() | Id(val[0..32].try_into()?)
    hashtags: Database,

    // Url -> Relay
    //   key: key!(url.0.as_bytes())
    //   val: serde_json::to_vec(relay) | serde_json::from_slice(bytes)
    relays: Database,

    // Id -> Event
    //   key: id.as_slice() | Id(val[0..32].try_into()?)
    //   val: event.write_to_vec() | Event::read_from_buffer(val)
    events: Database,

    // EventKind:PublicKey -> Id
    // (pubkey is event author)
    // (dup keys, so multiple Ids per key)
    //   val: id.as_slice() | Id(val[0..32].try_into()?)
    event_ek_pk_index: Database,

    // EventKind::ReverseUnixtime -> Id
    // (dup keys, so multiple Ids per key)
    //   val: id.as_slice() | Id(val[0..32].try_into()?)
    event_ek_c_index: Database,

    // PublicKey:ReverseUnixtime -> Id
    // (pubkey is referenced by the event somehow)
    // (only feed-displayable events are included)
    // (dup keys, so multiple Ids per key)
    // NOTE: this may be far too much data. Maybe we should only build this for the
    //       user's pubkey as their inbox.
    event_references_person: Database,

    // Id:Id -> Relationship
    //   key: id.as_slice(), id.as_slice() | Id(val[32..64].try_into()?)
    //   val:  relationship.write_to_vec() | Relationship::read_from_buffer(val)
    relationships: Database,

    // PublicKey -> Person
    //   key: pubkey.as_bytes()
    //   val: serde_json::to_vec(person) | serde_json::from_slice(bytes)
    people: Database,

    // PublicKey:Url -> PersonRelay
    //   key: key!(pubkey.as_bytes + url.0.as_bytes)
    //   val: person_relay.write_to_vec) | PersonRelay::read_from_buffer(bytes)
    person_relays: Database,
}

impl Storage {
    pub fn new() -> Result<Storage, Error> {
        let mut builder = Environment::new();

        builder.set_flags(EnvironmentFlags::NO_SYNC);
        // builder.set_max_readers(126); // this is the default
        builder.set_max_dbs(32);

        // This has to be big enough for all the data.
        // Note that it is the size of the map in VIRTUAL address space,
        //   and that it doesn't all have to be paged in at the same time.
        // Some filesystem that doesn't handle sparse files may allocate all
        //   of this, so we don't go too crazy big.

        builder.set_map_size(1048576 * 1024 * 24); // 24 GB

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

        let events = env.create_db(Some("events"), DatabaseFlags::empty())?;

        let event_ek_pk_index = env.create_db(
            Some("event_ek_pk_index"),
            DatabaseFlags::DUP_SORT | DatabaseFlags::DUP_FIXED,
        )?;

        let event_ek_c_index = env.create_db(
            Some("event_ek_c_index"),
            DatabaseFlags::DUP_SORT | DatabaseFlags::DUP_FIXED,
        )?;

        let event_references_person = env.create_db(
            Some("event_references_person"),
            DatabaseFlags::DUP_SORT | DatabaseFlags::DUP_FIXED,
        )?;
        let relationships = env.create_db(Some("relationships"), DatabaseFlags::empty())?;

        let people = env.create_db(Some("people"), DatabaseFlags::empty())?;

        let person_relays = env.create_db(Some("person_relays"), DatabaseFlags::empty())?;

        Ok(Storage {
            env,
            general,
            event_seen_on_relay,
            event_viewed,
            hashtags,
            relays,
            events,
            event_ek_pk_index,
            event_ek_c_index,
            event_references_person,
            relationships,
            people,
            person_relays,
        })
    }

    // Run this after GLOBALS lazy static initialisation, so functions within storage can
    // access GLOBALS without hanging.
    pub fn init(&self) -> Result<(), Error> {
        // If migration level is missing, we need to import from legacy sqlite
        match self.read_migration_level()? {
            None => {
                // Import from sqlite
                self.import()?;
                self.migrate(0)?;
            }
            Some(level) => {
                self.migrate(level)?;
            }
        }

        Ok(())
    }

    pub fn get_general_stats(&self) -> Result<Stat, Error> {
        let txn = self.env.begin_ro_txn()?;
        Ok(txn.stat(self.general)?)
    }

    pub fn get_event_seen_on_relay_stats(&self) -> Result<Stat, Error> {
        let txn = self.env.begin_ro_txn()?;
        Ok(txn.stat(self.event_seen_on_relay)?)
    }

    pub fn get_event_viewed_stats(&self) -> Result<Stat, Error> {
        let txn = self.env.begin_ro_txn()?;
        Ok(txn.stat(self.event_viewed)?)
    }

    pub fn get_hashtags_stats(&self) -> Result<Stat, Error> {
        let txn = self.env.begin_ro_txn()?;
        Ok(txn.stat(self.hashtags)?)
    }

    pub fn get_relays_stats(&self) -> Result<Stat, Error> {
        let txn = self.env.begin_ro_txn()?;
        Ok(txn.stat(self.relays)?)
    }

    pub fn get_event_stats(&self) -> Result<Stat, Error> {
        let txn = self.env.begin_ro_txn()?;
        Ok(txn.stat(self.events)?)
    }

    pub fn get_event_ek_pk_index_stats(&self) -> Result<Stat, Error> {
        let txn = self.env.begin_ro_txn()?;
        Ok(txn.stat(self.event_ek_pk_index)?)
    }

    pub fn get_event_ek_c_index_stats(&self) -> Result<Stat, Error> {
        let txn = self.env.begin_ro_txn()?;
        Ok(txn.stat(self.event_ek_c_index)?)
    }

    pub fn get_event_references_person_stats(&self) -> Result<Stat, Error> {
        let txn = self.env.begin_ro_txn()?;
        Ok(txn.stat(self.event_references_person)?)
    }

    pub fn get_relationships_stats(&self) -> Result<Stat, Error> {
        let txn = self.env.begin_ro_txn()?;
        Ok(txn.stat(self.relationships)?)
    }

    pub fn get_people_stats(&self) -> Result<Stat, Error> {
        let txn = self.env.begin_ro_txn()?;
        Ok(txn.stat(self.people)?)
    }

    pub fn get_person_relays_stats(&self) -> Result<Stat, Error> {
        let txn = self.env.begin_ro_txn()?;
        Ok(txn.stat(self.person_relays)?)
    }

    // Remove all events (and related data) with a created_at before `from`
    pub fn prune(&self, from: Unixtime) -> Result<usize, Error> {
        // Extract the Ids to delete.
        // We have to extract the Ids and release the cursor on the events database
        // in order to get a cursor on other databases since cursors mutably borrow
        // the transaction
        let txn = self.env.begin_ro_txn()?;
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
        txn.commit()?;

        let mut txn = self.env.begin_rw_txn()?;

        // Delete from event_seen_on_relay
        let mut deletions: Vec<Vec<u8>> = Vec::new();
        for id in &ids {
            let start_key: &[u8] = id.as_slice();
            let mut cursor = txn.open_rw_cursor(self.event_seen_on_relay)?;
            let iter = cursor.iter_from(start_key);
            for result in iter {
                match result {
                    Err(e) => return Err(e.into()),
                    Ok((key, _val)) => {
                        if !key.starts_with(start_key) {
                            break;
                        }
                        deletions.push(key.to_owned());
                    }
                }
            }
        }
        tracing::info!(
            "PRUNE: deleting {} records from event_seen_on_relay",
            deletions.len()
        );
        for deletion in deletions.drain(..) {
            txn.del(self.event_seen_on_relay, &deletion, None)?;
        }

        // Delete from event_viewed
        for id in &ids {
            let _ = txn.del(self.event_viewed, &id.as_slice(), None);
        }
        tracing::info!("PRUNE: deleted {} records from event_viewed", ids.len());

        // Delete from hashtags
        // (unfortunately since Ids are the values, we have to scan the whole thing)
        let mut cursor = txn.open_rw_cursor(self.hashtags)?;
        let iter = cursor.iter_start();
        let mut deletions: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
        for result in iter {
            match result {
                Err(e) => return Err(e.into()),
                Ok((key, val)) => {
                    let id = Id(val[0..32].try_into()?);
                    if ids.contains(&id) {
                        deletions.push((key.to_owned(), val.to_owned()));
                    }
                }
            }
        }
        drop(cursor);
        tracing::info!("PRUNE: deleting {} records from hashtags", deletions.len());
        for deletion in deletions.drain(..) {
            txn.del(self.hashtags, &deletion.0, Some(&deletion.1))?;
        }

        // Delete from relationships
        // (unfortunately because of the 2nd Id in the tag, we have to scan the whole thing)
        let mut cursor = txn.open_rw_cursor(self.relationships)?;
        let iter = cursor.iter_start();
        let mut deletions: Vec<Vec<u8>> = Vec::new();
        for result in iter {
            match result {
                Err(e) => return Err(e.into()),
                Ok((key, _val)) => {
                    let id = Id(key[0..32].try_into()?);
                    if ids.contains(&id) {
                        deletions.push(key.to_owned());
                        continue;
                    }
                    let id2 = Id(key[32..64].try_into()?);
                    if ids.contains(&id2) {
                        deletions.push(key.to_owned());
                    }
                }
            }
        }
        drop(cursor);
        tracing::info!("PRUNE: deleting {} relationships", deletions.len());
        for deletion in deletions.drain(..) {
            txn.del(self.relationships, &deletion, None)?;
        }

        // delete from events
        for id in &ids {
            let _ = txn.del(self.events, &id.as_ref(), None);
        }
        tracing::info!("PRUNE: deleted {} records from events", ids.len());

        txn.commit()?;

        Ok(ids.len())
    }

    pub fn write_migration_level<'a>(
        &'a self,
        migration_level: u32,
        rw_txn: Option<&mut RwTransaction<'a>>,
    ) -> Result<(), Error> {
        let bytes = &migration_level.to_be_bytes();

        let f = |txn: &mut RwTransaction<'a>| -> Result<(), Error> {
            Ok(txn.put(
                self.general,
                b"migration_level",
                &bytes,
                WriteFlags::empty(),
            )?)
        };

        match rw_txn {
            Some(txn) => {
                f(txn)?;
            }
            None => {
                let mut txn = self.env.begin_rw_txn()?;
                f(&mut txn)?;
                txn.commit()?;
            }
        };

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

    pub fn write_encrypted_private_key<'a>(
        &'a self,
        epk: &Option<EncryptedPrivateKey>,
        rw_txn: Option<&mut RwTransaction<'a>>,
    ) -> Result<(), Error> {
        let bytes = epk.as_ref().map(|e| &e.0).write_to_vec()?;

        let f = |txn: &mut RwTransaction<'a>| -> Result<(), Error> {
            txn.put(
                self.general,
                b"encrypted_private_key",
                &bytes,
                WriteFlags::empty(),
            )?;
            Ok(())
        };

        match rw_txn {
            Some(txn) => f(txn)?,
            None => {
                let mut txn = self.env.begin_rw_txn()?;
                f(&mut txn)?;
                txn.commit()?;
            }
        };

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

    pub fn write_last_contact_list_edit<'a>(
        &'a self,
        when: i64,
        rw_txn: Option<&mut RwTransaction<'a>>,
    ) -> Result<(), Error> {
        let bytes = &when.to_be_bytes();

        let f = |txn: &mut RwTransaction<'a>| -> Result<(), Error> {
            txn.put(
                self.general,
                b"last_contact_list_edit",
                &bytes,
                WriteFlags::empty(),
            )?;
            Ok(())
        };

        match rw_txn {
            Some(txn) => f(txn)?,
            None => {
                let mut txn = self.env.begin_rw_txn()?;
                f(&mut txn)?;
                txn.commit()?;
            }
        };

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

    pub fn write_settings<'a>(
        &'a self,
        settings: &Settings,
        rw_txn: Option<&mut RwTransaction<'a>>,
    ) -> Result<(), Error> {
        let bytes = settings.write_to_vec()?;

        let f = |txn: &mut RwTransaction<'a>| -> Result<(), Error> {
            txn.put(self.general, b"settings", &bytes, WriteFlags::empty())?;
            Ok(())
        };

        match rw_txn {
            Some(txn) => f(txn)?,
            None => {
                let mut txn = self.env.begin_rw_txn()?;
                f(&mut txn)?;
                txn.commit()?;
            }
        };

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

    pub fn add_event_seen_on_relay<'a>(
        &'a self,
        id: Id,
        url: &RelayUrl,
        when: Unixtime,
        rw_txn: Option<&mut RwTransaction<'a>>,
    ) -> Result<(), Error> {
        let mut key: Vec<u8> = id.as_slice().to_owned();
        key.extend(url.0.as_bytes());
        key.truncate(MAX_LMDB_KEY);
        let bytes = &when.0.to_be_bytes();

        let f = |txn: &mut RwTransaction<'a>| -> Result<(), Error> {
            txn.put(self.event_seen_on_relay, &key, &bytes, WriteFlags::empty())?;
            Ok(())
        };

        match rw_txn {
            Some(txn) => f(txn)?,
            None => {
                let mut txn = self.env.begin_rw_txn()?;
                f(&mut txn)?;
                txn.commit()?;
            }
        };

        Ok(())
    }

    pub fn get_event_seen_on_relay(&self, id: Id) -> Result<Vec<(RelayUrl, Unixtime)>, Error> {
        let start_key: Vec<u8> = id.as_slice().to_owned();
        let txn = self.env.begin_ro_txn()?;
        let mut cursor = txn.open_ro_cursor(self.event_seen_on_relay)?;
        let iter = cursor.iter_from(&start_key);
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

    pub fn mark_event_viewed<'a>(
        &'a self,
        id: Id,
        rw_txn: Option<&mut RwTransaction<'a>>,
    ) -> Result<(), Error> {
        let bytes = vec![];

        let f = |txn: &mut RwTransaction<'a>| -> Result<(), Error> {
            txn.put(
                self.event_viewed,
                &id.as_slice(),
                &bytes,
                WriteFlags::empty(),
            )?;
            Ok(())
        };

        match rw_txn {
            Some(txn) => f(txn)?,
            None => {
                let mut txn = self.env.begin_rw_txn()?;
                f(&mut txn)?;
                txn.commit()?;
            }
        };

        Ok(())
    }

    pub fn is_event_viewed(&self, id: Id) -> Result<bool, Error> {
        let txn = self.env.begin_ro_txn()?;
        match txn.get(self.event_viewed, &id.as_slice()) {
            Ok(_bytes) => Ok(true),
            Err(lmdb::Error::NotFound) => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    pub fn add_hashtag<'a>(
        &'a self,
        hashtag: &String,
        id: Id,
        rw_txn: Option<&mut RwTransaction<'a>>,
    ) -> Result<(), Error> {
        let key = key!(hashtag.as_bytes());
        if key.is_empty() {
            return Err(ErrorKind::Empty("hashtag".to_owned()).into());
        }
        let bytes = id.as_slice();

        let f = |txn: &mut RwTransaction<'a>| -> Result<(), Error> {
            txn.put(self.hashtags, &key, &bytes, WriteFlags::empty())?;
            Ok(())
        };

        match rw_txn {
            Some(txn) => f(txn)?,
            None => {
                let mut txn = self.env.begin_rw_txn()?;
                f(&mut txn)?;
                txn.commit()?;
            }
        };

        Ok(())
    }

    pub fn get_event_ids_with_hashtag(&self, hashtag: &String) -> Result<Vec<Id>, Error> {
        let key = key!(hashtag.as_bytes());
        if key.is_empty() {
            return Err(ErrorKind::Empty("hashtag".to_owned()).into());
        }
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
                    let id = Id(val[0..32].try_into()?);
                    output.push(id);
                }
            }
        }
        Ok(output)
    }

    pub fn write_relay<'a>(
        &'a self,
        relay: &Relay,
        rw_txn: Option<&mut RwTransaction<'a>>,
    ) -> Result<(), Error> {
        // Note that we use serde instead of speedy because the complexity of the
        // serde_json::Value type makes it difficult. Any other serde serialization
        // should work though: Consider bincode.
        let key = key!(relay.url.0.as_bytes());
        if key.is_empty() {
            return Err(ErrorKind::Empty("relay url".to_owned()).into());
        }
        let bytes = serde_json::to_vec(relay)?;

        let f = |txn: &mut RwTransaction<'a>| -> Result<(), Error> {
            txn.put(self.relays, &key, &bytes, WriteFlags::empty())?;
            Ok(())
        };

        match rw_txn {
            Some(txn) => f(txn)?,
            None => {
                let mut txn = self.env.begin_rw_txn()?;
                f(&mut txn)?;
                txn.commit()?;
            }
        };

        Ok(())
    }

    pub fn write_relay_if_missing<'a>(
        &'a self,
        url: &RelayUrl,
        rw_txn: Option<&mut RwTransaction<'a>>,
    ) -> Result<(), Error> {
        if self.read_relay(url)?.is_none() {
            let dbrelay = Relay::new(url.to_owned());
            self.write_relay(&dbrelay, rw_txn)?;
        }
        Ok(())
    }

    pub fn modify_relay<'a, M>(
        &'a self,
        url: &RelayUrl,
        mut modify: M,
        rw_txn: Option<&mut RwTransaction<'a>>,
    ) -> Result<(), Error>
    where
        M: FnMut(&mut Relay),
    {
        let key = key!(url.0.as_bytes());
        if key.is_empty() {
            return Err(ErrorKind::Empty("relay url".to_owned()).into());
        }

        let mut f = |txn: &mut RwTransaction<'a>| -> Result<(), Error> {
            match txn.get(self.relays, &key) {
                Ok(bytes) => {
                    let mut relay = serde_json::from_slice(bytes)?;
                    modify(&mut relay);
                    let bytes = serde_json::to_vec(&relay)?;
                    txn.put(self.relays, &key, &bytes, WriteFlags::empty())?;
                    Ok(())
                }
                Err(lmdb::Error::NotFound) => Ok(()),
                Err(e) => Err(e.into()),
            }
        };

        match rw_txn {
            Some(txn) => f(txn)?,
            None => {
                let mut txn = self.env.begin_rw_txn()?;
                f(&mut txn)?;
                txn.commit()?;
            }
        };

        Ok(())
    }

    pub fn modify_all_relays<'a, M>(
        &'a self,
        mut modify: M,
        rw_txn: Option<&mut RwTransaction<'a>>,
    ) -> Result<(), Error>
    where
        M: FnMut(&mut Relay),
    {
        let mut f = |txn: &mut RwTransaction<'a>| -> Result<(), Error> {
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
            Ok(())
        };

        match rw_txn {
            Some(txn) => f(txn)?,
            None => {
                let mut txn = self.env.begin_rw_txn()?;
                f(&mut txn)?;
                txn.commit()?;
            }
        };

        Ok(())
    }

    pub fn read_relay(&self, url: &RelayUrl) -> Result<Option<Relay>, Error> {
        // Note that we use serde instead of speedy because the complexity of the
        // serde_json::Value type makes it difficult. Any other serde serialization
        // should work though: Consider bincode.
        let key = key!(url.0.as_bytes());
        if key.is_empty() {
            return Err(ErrorKind::Empty("relay url".to_owned()).into());
        }

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

    pub fn write_event<'a>(
        &'a self,
        event: &Event,
        rw_txn: Option<&mut RwTransaction<'a>>,
    ) -> Result<(), Error> {
        // write to lmdb 'events'
        let bytes = event.write_to_vec()?;

        let f = |txn: &mut RwTransaction<'a>| -> Result<(), Error> {
            txn.put(
                self.events,
                &event.id.as_slice(),
                &bytes,
                WriteFlags::empty(),
            )?;

            // also index the event
            self.write_event_ek_pk_index(event, Some(txn))?;
            self.write_event_ek_c_index(event, Some(txn))?;
            self.write_event_references_person(event, Some(txn))?;
            Ok(())
        };

        match rw_txn {
            Some(txn) => f(txn)?,
            None => {
                let mut txn = self.env.begin_rw_txn()?;
                f(&mut txn)?;
                txn.commit()?;
            }
        };

        Ok(())
    }

    pub fn read_event(&self, id: Id) -> Result<Option<Event>, Error> {
        let txn = self.env.begin_ro_txn()?;
        match txn.get(self.events, &id.as_slice()) {
            Ok(bytes) => Ok(Some(Event::read_from_buffer(bytes)?)),
            Err(lmdb::Error::NotFound) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn has_event(&self, id: Id) -> Result<bool, Error> {
        let txn = self.env.begin_ro_txn()?;
        match txn.get(self.events, &id.as_slice()) {
            Ok(_bytes) => Ok(true),
            Err(lmdb::Error::NotFound) => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    pub fn delete_event<'a>(
        &'a self,
        id: Id,
        rw_txn: Option<&mut RwTransaction<'a>>,
    ) -> Result<(), Error> {
        let f = |txn: &mut RwTransaction<'a>| -> Result<(), Error> {
            let _ = txn.del(self.events, &id.as_slice(), None);
            Ok(())
        };

        match rw_txn {
            Some(txn) => f(txn)?,
            None => {
                let mut txn = self.env.begin_rw_txn()?;
                f(&mut txn)?;
                txn.commit()?;
            }
        };

        Ok(())
    }

    pub fn replace_event<'a>(
        &'a self,
        event: &Event,
        rw_txn: Option<&mut RwTransaction<'a>>,
    ) -> Result<bool, Error> {
        if !event.kind.is_replaceable() {
            return Err(ErrorKind::General("Event is not replaceable.".to_owned()).into());
        }

        let existing = self.find_events(&[event.kind], &[event.pubkey], None, |_| true, false)?;

        let mut found_newer = false;
        for old in existing {
            if old.created_at < event.created_at {
                // here is some reborrow magic we needed to appease the borrow checker
                if let Some(&mut ref mut v) = rw_txn {
                    self.delete_event(old.id, Some(v))?;
                } else {
                    self.delete_event(old.id, None)?;
                }
            } else {
                found_newer = true;
            }
        }

        if found_newer {
            return Ok(false); // this event is not the latest one.
        }

        self.write_event(event, rw_txn)?;

        Ok(true)
    }

    pub fn get_parameterized_replaceable_event(
        &self,
        event_addr: &EventAddr,
    ) -> Result<Option<Event>, Error> {
        if !event_addr.kind.is_parameterized_replaceable() {
            return Err(ErrorKind::General(
                "Invalid EventAddr, kind is not parameterized replaceable.".to_owned(),
            )
            .into());
        }

        let mut events = self.find_events(
            &[event_addr.kind],
            &[event_addr.author],
            None, // any time
            |e| e.parameter().as_ref() == Some(&event_addr.d),
            true, // sorted in reverse time order
        )?;

        let maybe_event = events.drain(..).take(1).next();
        Ok(maybe_event)
    }

    pub fn replace_parameterized_event<'a>(
        &'a self,
        event: &Event,
        rw_txn: Option<&mut RwTransaction<'a>>,
    ) -> Result<bool, Error> {
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
            &[event.kind],
            &[event.pubkey],
            None,
            |e| e.parameter().as_ref() == Some(&param),
            false,
        )?;

        let mut found_newer = false;
        for old in existing {
            if old.created_at < event.created_at {
                // here is some reborrow magic we needed to appease the borrow checker
                if let Some(&mut ref mut v) = rw_txn {
                    self.delete_event(old.id, Some(v))?;
                } else {
                    self.delete_event(old.id, None)?;
                }
            } else {
                found_newer = true;
            }
        }

        if found_newer {
            return Ok(false); // this event is not the latest one.
        }

        self.write_event(event, rw_txn)?;
        Ok(true)
    }

    // You must supply kinds.
    // You can skip the pubkeys and then only kinds will matter.
    fn find_ek_pk_events(
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
        let txn = self.env.begin_ro_txn()?;
        let mut cursor = txn.open_ro_cursor(self.event_ek_pk_index)?;

        'kindloop: for kind in kinds {
            let ek: u32 = (*kind).into();
            if pubkeys.is_empty() {
                let start_key = ek.to_be_bytes().as_slice().to_owned();
                let iter = cursor.iter_from(start_key);
                for result in iter {
                    match result {
                        Err(e) => return Err(e.into()),
                        Ok((key, val)) => {
                            // Break if we moved to a different event kind
                            let this_ek = u32::from_be_bytes(key[0..4].try_into().unwrap());
                            if this_ek != ek {
                                continue 'kindloop;
                            }

                            // Take the event
                            let id = Id(val[0..32].try_into()?);
                            ids.insert(id);
                        }
                    }
                }
            } else {
                'pubkeyloop: for pubkey in pubkeys {
                    let mut start_key = ek.to_be_bytes().as_slice().to_owned();
                    start_key.extend(pubkey.as_bytes());
                    let iter = cursor.iter_from(start_key);
                    for result in iter {
                        match result {
                            Err(e) => return Err(e.into()),
                            Ok((key, val)) => {
                                // Break if we moved to a different event kind
                                let this_ek = u32::from_be_bytes(key[0..4].try_into().unwrap());
                                if this_ek != ek {
                                    continue 'kindloop;
                                }

                                // Break if we moved to a different public key
                                let this_pubkey =
                                    match PublicKey::from_bytes(&key[4..4 + 32], false) {
                                        Err(_) => continue,
                                        Ok(pk) => pk,
                                    };
                                if this_pubkey != *pubkey {
                                    continue 'pubkeyloop;
                                }

                                // Take the event
                                let id = Id(val[0..32].try_into()?);
                                ids.insert(id);
                            }
                        }
                    }
                }
            }
        }

        Ok(ids)
    }

    // You must supply kinds and since
    fn find_ek_c_events(&self, kinds: &[EventKind], since: Unixtime) -> Result<HashSet<Id>, Error> {
        if kinds.is_empty() {
            return Err(ErrorKind::General(
                "find_ek_c_events() requires some event kinds to be specified.".to_string(),
            )
            .into());
        }

        let now = Unixtime::now().unwrap();
        let mut ids: HashSet<Id> = HashSet::new();
        let txn = self.env.begin_ro_txn()?;
        let mut cursor = txn.open_ro_cursor(self.event_ek_c_index)?;

        'kindloop: for kind in kinds {
            let ek: u32 = (*kind).into();
            let mut start_key = ek.to_be_bytes().as_slice().to_owned();
            start_key.extend((i64::MAX - now.0).to_be_bytes().as_slice()); // work back from now
            let iter = cursor.iter_from(start_key);
            for result in iter {
                match result {
                    Err(e) => return Err(e.into()),
                    Ok((key, val)) => {
                        // Break if we moved to a different event kind
                        let this_ek = u32::from_be_bytes(key[0..4].try_into().unwrap());
                        if this_ek != ek {
                            continue 'kindloop;
                        }

                        // Break if these events are getting to old
                        let this_time = i64::from_be_bytes(key[4..4 + 8].try_into().unwrap());
                        if this_time > (i64::MAX - since.0) {
                            continue 'kindloop;
                        }

                        // Take the event
                        let id = Id(val[0..32].try_into()?);
                        ids.insert(id);
                    }
                }
            }
        }

        Ok(ids)
    }

    // Find events of interest.
    //
    // You must specify some event kinds.
    // If pubkeys is empty, they won't matter.
    // If since is None, it won't matter.
    //
    // The function f is run after the matching-so-far events have been deserialized
    // to finish filtering, and optionally they are sorted in reverse chronological
    // order.
    pub fn find_events<F>(
        &self,
        kinds: &[EventKind],
        pubkeys: &[PublicKey],
        since: Option<Unixtime>,
        f: F,
        sort: bool,
    ) -> Result<Vec<Event>, Error>
    where
        F: Fn(&Event) -> bool,
    {
        let ids = self.find_event_ids(kinds, pubkeys, since)?;

        // Now that we have that Ids, fetch the events
        let txn = self.env.begin_ro_txn()?;
        let mut events: Vec<Event> = Vec::new();
        for id in ids {
            // this is like self.read_event(), but we supply our existing transaction
            match txn.get(self.events, &id.as_slice()) {
                Ok(bytes) => {
                    let event = Event::read_from_buffer(bytes)?;
                    if f(&event) {
                        events.push(event);
                    }
                }
                Err(lmdb::Error::NotFound) => continue,
                Err(e) => return Err(e.into()),
            }
        }

        if sort {
            events.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        }

        Ok(events)
    }

    // Find events of interest. This is just like find_events() but it just gives the Ids,
    // unsorted.
    //
    // You must specify some event kinds.
    // If pubkeys is empty, they won't matter.
    // If since is None, it won't matter.
    //
    // The function f is run after the matching-so-far events have been deserialized
    // to finish filtering, and optionally they are sorted in reverse chronological
    // order.
    pub fn find_event_ids(
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
            (true, None) => self.find_ek_pk_events(kinds, pubkeys)?,
            (true, Some(when)) => self.find_ek_c_events(kinds, when)?,
            (false, None) => self.find_ek_pk_events(kinds, pubkeys)?,
            (false, Some(when)) => {
                let group1 = self.find_ek_pk_events(kinds, pubkeys)?;
                let group2 = self.find_ek_c_events(kinds, when)?;
                group1.intersection(&group2).copied().collect()
            }
        };

        Ok(ids)
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

    // We don't call this externally. Whenever we write an event, we do this.
    fn write_event_ek_pk_index<'a>(
        &'a self,
        event: &Event,
        rw_txn: Option<&mut RwTransaction<'a>>,
    ) -> Result<(), Error> {
        let ek: u32 = event.kind.into();
        let mut key: Vec<u8> = ek.to_be_bytes().as_slice().to_owned(); // event kind
        key.extend(event.pubkey.as_bytes()); // pubkey
        let bytes = event.id.as_slice();

        let f = |txn: &mut RwTransaction<'a>| -> Result<(), Error> {
            txn.put(self.event_ek_pk_index, &key, &bytes, WriteFlags::empty())?;
            Ok(())
        };

        match rw_txn {
            Some(txn) => f(txn)?,
            None => {
                let mut txn = self.env.begin_rw_txn()?;
                f(&mut txn)?;
                txn.commit()?;
            }
        };

        Ok(())
    }

    // We don't call this externally. Whenever we write an event, we do this.
    fn write_event_ek_c_index<'a>(
        &'a self,
        event: &Event,
        rw_txn: Option<&mut RwTransaction<'a>>,
    ) -> Result<(), Error> {
        let ek: u32 = event.kind.into();
        let mut key: Vec<u8> = ek.to_be_bytes().as_slice().to_owned(); // event kind
        key.extend((i64::MAX - event.created_at.0).to_be_bytes().as_slice()); // reverse created_at
        let bytes = event.id.as_slice();

        let f = |txn: &mut RwTransaction<'a>| -> Result<(), Error> {
            txn.put(self.event_ek_c_index, &key, &bytes, WriteFlags::empty())?;
            Ok(())
        };

        match rw_txn {
            Some(txn) => f(txn)?,
            None => {
                let mut txn = self.env.begin_rw_txn()?;
                f(&mut txn)?;
                txn.commit()?;
            }
        };

        Ok(())
    }

    // We don't call this externally. Whenever we write an event, we do this.
    fn write_event_references_person<'a>(
        &'a self,
        event: &Event,
        rw_txn: Option<&mut RwTransaction<'a>>,
    ) -> Result<(), Error> {
        if !event.kind.is_feed_displayable() {
            return Ok(());
        }

        let bytes = event.id.as_slice();

        let mut pubkeys: HashSet<PublicKey> = HashSet::new();
        for (pubkeyhex, _, _) in event.people() {
            let pubkey = match PublicKey::try_from_hex_string(pubkeyhex.as_str(), false) {
                Ok(pk) => pk,
                Err(_) => continue,
            };
            pubkeys.insert(pubkey);
        }
        for pubkey in event.people_referenced_in_content() {
            pubkeys.insert(pubkey);
        }
        if !pubkeys.is_empty() {
            let mut f = |txn: &mut RwTransaction<'a>| -> Result<(), Error> {
                for pubkey in pubkeys.drain() {
                    let mut key: Vec<u8> = pubkey.to_bytes();
                    key.extend((i64::MAX - event.created_at.0).to_be_bytes().as_slice()); // reverse created_at
                    txn.put(
                        self.event_references_person,
                        &key,
                        &bytes,
                        WriteFlags::empty(),
                    )?;
                }
                Ok(())
            };

            match rw_txn {
                Some(txn) => f(txn)?,
                None => {
                    let mut txn = self.env.begin_rw_txn()?;
                    f(&mut txn)?;
                    txn.commit()?;
                }
            };
        }

        Ok(())
    }

    // Read all events referencing a given person in reverse time order
    pub fn read_events_referencing_person<F>(
        &self,
        pubkey: &PublicKey,
        since: Unixtime,
        f: F,
    ) -> Result<Vec<Event>, Error>
    where
        F: Fn(&Event) -> bool,
    {
        let txn = self.env.begin_ro_txn()?;
        let mut cursor = txn.open_ro_cursor(self.event_references_person)?;
        let now = Unixtime::now().unwrap();
        let mut start_key: Vec<u8> = pubkey.to_bytes();
        start_key.extend((i64::MAX - now.0).to_be_bytes().as_slice()); // work back from now
        let iter = cursor.iter_from(start_key);
        let mut events: Vec<Event> = Vec::new();
        for result in iter {
            match result {
                Err(e) => return Err(e.into()),
                Ok((key, val)) => {
                    // Break if we moved to a different pubkey
                    let this_pubkey = match PublicKey::from_bytes(&key[..32], false) {
                        Err(_) => continue,
                        Ok(pk) => pk,
                    };
                    if this_pubkey != *pubkey {
                        break;
                    }

                    // Break if these events are getting to old
                    let this_time = i64::from_be_bytes(key[32..32 + 8].try_into().unwrap());
                    if this_time > (i64::MAX - since.0) {
                        break;
                    }

                    // Take the event
                    let id = Id(val[0..32].try_into()?);
                    // (like read_event, but we supply our on transaction)
                    match txn.get(self.events, &id.as_slice()) {
                        Ok(bytes) => {
                            let event = Event::read_from_buffer(bytes)?;
                            if f(&event) {
                                events.push(event);
                            }
                        }
                        Err(lmdb::Error::NotFound) => continue,
                        Err(e) => return Err(e.into()),
                    }
                }
            }
        }
        Ok(events)
    }

    // TBD: optimize this by storing better event indexes
    // currently we stupidly scan every event (just to get LMDB up and running first)
    pub fn fetch_contact_list(&self, pubkey: &PublicKey) -> Result<Option<Event>, Error> {
        Ok(self
            .find_events(&[EventKind::ContactList], &[*pubkey], None, |_| true, false)?
            .iter()
            .max_by(|x, y| x.created_at.cmp(&y.created_at))
            .cloned())
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

    pub fn write_relationship<'a>(
        &'a self,
        id: Id,
        related: Id,
        relationship: Relationship,
        rw_txn: Option<&mut RwTransaction<'a>>,
    ) -> Result<(), Error> {
        let mut key = id.as_ref().as_slice().to_owned();
        key.extend(related.as_ref());
        let value = relationship.write_to_vec()?;

        match rw_txn {
            Some(txn) => {
                txn.put(self.relationships, &key, &value, WriteFlags::empty())?;
            }
            None => {
                let mut txn = self.env.begin_rw_txn()?;
                txn.put(self.relationships, &key, &value, WriteFlags::empty())?;
                txn.commit()?;
            }
        };

        Ok(())
    }

    pub fn find_relationships(&self, id: Id) -> Result<Vec<(Id, Relationship)>, Error> {
        let start_key = id.as_slice();
        let txn = self.env.begin_ro_txn()?;
        let mut cursor = txn.open_ro_cursor(self.relationships)?;
        let iter = cursor.iter_from(start_key);
        let mut output: Vec<(Id, Relationship)> = Vec::new();
        for result in iter {
            match result {
                Err(e) => return Err(e.into()),
                Ok((key, val)) => {
                    if !key.starts_with(start_key) {
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
                return Ok(Some(deletion));
            }
        }
        Ok(None)
    }

    // This returns IDs that should be UI invalidated
    pub fn process_relationships_of_event<'a>(
        &'a self,
        event: &Event,
        rw_txn: Option<&mut RwTransaction<'a>>,
    ) -> Result<Vec<Id>, Error> {
        let mut invalidate: Vec<Id> = Vec::new();

        let mut f = |txn: &mut RwTransaction<'a>| -> Result<(), Error> {
            // replies to
            if let Some((id, _)) = event.replies_to() {
                self.write_relationship(id, event.id, Relationship::Reply, Some(txn))?;
            }

            // reacts to
            if let Some((id, reaction, _maybe_url)) = event.reacts_to() {
                self.write_relationship(
                    id,
                    event.id,
                    Relationship::Reaction(event.pubkey, reaction),
                    Some(txn),
                )?;

                invalidate.push(id);
            }

            // deletes
            if let Some((ids, reason)) = event.deletes() {
                invalidate.extend(&ids);

                for id in ids {
                    // since it is a delete, we don't actually desire the event.

                    self.write_relationship(
                        id,
                        event.id,
                        Relationship::Deletion(reason.clone()),
                        Some(txn),
                    )?;
                }
            }

            // zaps
            match event.zaps() {
                Ok(Some(zapdata)) => {
                    self.write_relationship(
                        zapdata.id,
                        event.id,
                        Relationship::ZapReceipt(event.pubkey, zapdata.amount),
                        Some(txn),
                    )?;

                    invalidate.push(zapdata.id);
                }
                Err(e) => tracing::error!("Invalid zap receipt: {}", e),
                _ => {}
            }

            Ok(())
        };

        match rw_txn {
            Some(txn) => f(txn)?,
            None => {
                let mut txn = self.env.begin_rw_txn()?;
                f(&mut txn)?;
                txn.commit()?;
            }
        };

        Ok(invalidate)
    }

    pub fn write_person<'a>(
        &'a self,
        person: &Person,
        rw_txn: Option<&mut RwTransaction<'a>>,
    ) -> Result<(), Error> {
        // Note that we use serde instead of speedy because the complexity of the
        // serde_json::Value type makes it difficult. Any other serde serialization
        // should work though: Consider bincode.
        let key: Vec<u8> = person.pubkey.to_bytes();
        let bytes = serde_json::to_vec(person)?;

        let f = |txn: &mut RwTransaction<'a>| -> Result<(), Error> {
            txn.put(self.people, &key, &bytes, WriteFlags::empty())?;
            Ok(())
        };

        match rw_txn {
            Some(txn) => f(txn)?,
            None => {
                let mut txn = self.env.begin_rw_txn()?;
                f(&mut txn)?;
                txn.commit()?;
            }
        };

        Ok(())
    }

    pub fn read_person(&self, pubkey: &PublicKey) -> Result<Option<Person>, Error> {
        // Note that we use serde instead of speedy because the complexity of the
        // serde_json::Value type makes it difficult. Any other serde serialization
        // should work though: Consider bincode.
        let key: Vec<u8> = pubkey.to_bytes();
        let txn = self.env.begin_ro_txn()?;
        match txn.get(self.people, &key) {
            Ok(bytes) => Ok(Some(serde_json::from_slice(bytes)?)),
            Err(lmdb::Error::NotFound) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn write_person_if_missing<'a>(
        &'a self,
        pubkey: &PublicKey,
        rw_txn: Option<&mut RwTransaction<'a>>,
    ) -> Result<(), Error> {
        if self.read_person(pubkey)?.is_none() {
            let person = Person::new(pubkey.to_owned());
            self.write_person(&person, rw_txn)?;
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

    pub fn write_person_relay<'a>(
        &'a self,
        person_relay: &PersonRelay,
        rw_txn: Option<&mut RwTransaction<'a>>,
    ) -> Result<(), Error> {
        let mut key = person_relay.pubkey.to_bytes();
        key.extend(person_relay.url.0.as_bytes());
        key.truncate(MAX_LMDB_KEY);
        let bytes = person_relay.write_to_vec()?;

        let f = |txn: &mut RwTransaction<'a>| -> Result<(), Error> {
            txn.put(self.person_relays, &key, &bytes, WriteFlags::empty())?;
            Ok(())
        };

        match rw_txn {
            Some(txn) => f(txn)?,
            None => {
                let mut txn = self.env.begin_rw_txn()?;
                f(&mut txn)?;
                txn.commit()?;
            }
        };

        Ok(())
    }

    pub fn read_person_relay(
        &self,
        pubkey: PublicKey,
        url: &RelayUrl,
    ) -> Result<Option<PersonRelay>, Error> {
        let mut key = pubkey.to_bytes();
        key.extend(url.0.as_bytes());
        key.truncate(MAX_LMDB_KEY);
        let txn = self.env.begin_ro_txn()?;
        match txn.get(self.person_relays, &key) {
            Ok(bytes) => Ok(Some(PersonRelay::read_from_buffer(bytes)?)),
            Err(lmdb::Error::NotFound) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn get_person_relays(&self, pubkey: PublicKey) -> Result<Vec<PersonRelay>, Error> {
        let start_key = pubkey.to_bytes();
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

    pub fn set_relay_list<'a>(
        &'a self,
        pubkey: PublicKey,
        read_relays: Vec<RelayUrl>,
        write_relays: Vec<RelayUrl>,
        rw_txn: Option<&mut RwTransaction<'a>>,
    ) -> Result<(), Error> {
        let mut person_relays = self.get_person_relays(pubkey)?;
        for mut pr in person_relays.drain(..) {
            let orig_read = pr.read;
            let orig_write = pr.write;
            pr.read = read_relays.contains(&pr.url);
            pr.write = write_relays.contains(&pr.url);
            if pr.read != orig_read || pr.write != orig_write {
                // here is some reborrow magic we needed to appease the borrow checker
                if let Some(&mut ref mut v) = rw_txn {
                    self.write_person_relay(&pr, Some(v))?;
                } else {
                    self.write_person_relay(&pr, None)?;
                }
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

        // Note: the following read_rank and write_rank do not consider our own
        // rank or the success rate.
        let mut ranked_relays = match dir {
            Direction::Write => PersonRelay::write_rank(person_relays),
            Direction::Read => PersonRelay::read_rank(person_relays),
        };

        // Modulate these scores with our local rankings
        for ranked_relay in ranked_relays.iter_mut() {
            match self.read_relay(&ranked_relay.0)? {
                None => ranked_relay.1 = 0,
                Some(relay) => {
                    let success_rate = relay.success_rate();
                    let rank = (relay.rank as f32 * success_rate * 0.66666) as u64;
                    ranked_relay.1 *= rank;
                }
            }
        }

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

    pub fn sync(&self) -> Result<(), Error> {
        self.env.sync(true)?;
        Ok(())
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
