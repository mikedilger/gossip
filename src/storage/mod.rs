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

mod import;
mod migrations;
mod types;

use crate::dm_channel::{DmChannel, DmChannelData};
use crate::error::{Error, ErrorKind};
use crate::globals::GLOBALS;
use crate::people::Person;
use crate::person_relay::PersonRelay;
use crate::profile::Profile;
use crate::relationship::Relationship;
use crate::relay::Relay;
use crate::ui::{Theme, ThemeVariant};
use gossip_relay_picker::Direction;
use heed::types::UnalignedSlice;
use heed::{Database, DatabaseFlags, Env, EnvFlags, EnvOpenOptions, RwTxn};
use nostr_types::{
    EncryptedPrivateKey, Event, EventAddr, EventKind, Id, MilliSatoshi, PublicKey, RelayUrl, Tag,
    Unixtime,
};
use paste::paste;
use speedy::{Readable, Writable};
use std::collections::{HashMap, HashSet};
use std::ops::Bound;

// Macro to define read-and-write into "general" database, largely for settings
// The type must implemented Speedy Readable and Writable
macro_rules! def_setting {
    ($field:ident, $string:literal, $type:ty, $default:expr) => {
        paste! {
            #[allow(dead_code)]
            pub fn [<write_setting_ $field>]<'a>(
                &'a self,
                $field: &$type,
                rw_txn: Option<&mut RwTxn<'a>>,
            ) -> Result<(), Error> {
                let bytes = $field.write_to_vec()?;

                let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
                    Ok(self.general.put(txn, $string, &bytes)?)
                };

                match rw_txn {
                    Some(txn) => {
                        f(txn)?;
                    }
                    None => {
                        let mut txn = self.env.write_txn()?;
                        f(&mut txn)?;
                        txn.commit()?;
                    }
                };

                Ok(())
            }

            #[allow(dead_code)]
            pub fn [<read_setting_ $field>](&self) -> $type {
                let txn = match self.env.read_txn() {
                    Ok(txn) => txn,
                    Err(_) => return $default,
                };

                match self.general.get(&txn, $string) {
                    Err(_) => $default,
                    Ok(None) => $default,
                    Ok(Some(bytes)) => match $type::read_from_buffer(bytes) {
                        Ok(val) => val,
                        Err(_) => $default,
                    }
                }
            }

            #[allow(dead_code)]
            pub fn [<set_default_setting_ $field>]<'a>(
                &'a self,
                rw_txn: Option<&mut RwTxn<'a>>
            ) -> Result<(), Error> {
                self.[<write_setting_ $field>](&$default, rw_txn)
            }

            #[allow(dead_code)]
            pub fn [<get_default_setting_ $field>]() -> $type {
                $default
            }
        }
    };
}

type RawDatabase = Database<UnalignedSlice<u8>, UnalignedSlice<u8>>;

pub struct Storage {
    env: Env,

    // General database (settings, local_settings)
    general: RawDatabase,

    // Id:Url -> Unixtime
    //   key: key!(id.as_slice(), url.0.as_bytes())
    //   val: unixtime.0.to_be_bytes()
    event_seen_on_relay: RawDatabase,

    // Id -> ()
    //   key: id.as_slice()
    //   val: vec![]
    event_viewed: RawDatabase,

    // Hashtag -> Id
    // (dup keys, so multiple Ids per hashtag)
    //   key: key!(hashtag.as_bytes())
    //   val: id.as_slice() | Id(val[0..32].try_into()?)
    hashtags: RawDatabase,

    // Url -> Relay
    //   key: key!(url.0.as_bytes())
    //   val: serde_json::to_vec(relay) | serde_json::from_slice(bytes)
    relays: RawDatabase,

    // Id -> Event
    //   key: id.as_slice() | Id(val[0..32].try_into()?)
    //   val: event.write_to_vec() | Event::read_from_buffer(val)
    events: RawDatabase,

    // EventKind:PublicKey -> Id
    // (pubkey is event author)
    // (dup keys, so multiple Ids per key)
    //   val: id.as_slice() | Id(val[0..32].try_into()?)
    event_ek_pk_index: RawDatabase,

    // EventKind::ReverseUnixtime -> Id
    // (dup keys, so multiple Ids per key)
    //   val: id.as_slice() | Id(val[0..32].try_into()?)
    event_ek_c_index: RawDatabase,

    // PublicKey:ReverseUnixtime -> Id
    // (pubkey is referenced by the event somehow)
    // (only feed-displayable events are included)
    // (dup keys, so multiple Ids per key)
    // NOTE: this may be far too much data. Maybe we should only build this for the
    //       user's pubkey as their inbox.
    event_references_person: RawDatabase,

    // Id:Id -> Relationship
    //   key: id.as_slice(), id.as_slice() | Id(val[32..64].try_into()?)
    //   val:  relationship.write_to_vec() | Relationship::read_from_buffer(val)
    relationships: RawDatabase,

    // PublicKey -> Person
    //   key: pubkey.as_bytes()
    //   val: serde_json::to_vec(person) | serde_json::from_slice(bytes)
    people: RawDatabase,

    // PublicKey:Url -> PersonRelay
    //   key: key!(pubkey.as_bytes + url.0.as_bytes)
    //   val: person_relay.write_to_vec) | PersonRelay::read_from_buffer(bytes)
    person_relays: RawDatabase,

    // Id -> ()
    //   key: id.as_slice()
    //   val: vec![]
    unindexed_giftwraps: RawDatabase,
}

impl Storage {
    pub fn new() -> Result<Storage, Error> {
        let mut builder = EnvOpenOptions::new();
        unsafe {
            builder.flags(EnvFlags::NO_SYNC);
        }
        // builder.max_readers(126); // this is the default
        builder.max_dbs(32);

        // This has to be big enough for all the data.
        // Note that it is the size of the map in VIRTUAL address space,
        //   and that it doesn't all have to be paged in at the same time.
        // Some filesystem that doesn't handle sparse files may allocate all
        //   of this, so we don't go too crazy big.
        // NOTE: this cannot be a setting because settings are only available
        //       after the database has been launched.
        builder.map_size(1048576 * 1024 * 24); // 24 GB

        let env = builder.open(Profile::current()?.lmdb_dir)?;

        let mut txn = env.write_txn()?;

        let general = env
            .database_options()
            .types::<UnalignedSlice<u8>, UnalignedSlice<u8>>()
            .create(&mut txn)?;

        let event_seen_on_relay = env
            .database_options()
            .types::<UnalignedSlice<u8>, UnalignedSlice<u8>>()
            .name("event_seen_on_relay")
            .create(&mut txn)?;

        let event_viewed = env
            .database_options()
            .types::<UnalignedSlice<u8>, UnalignedSlice<u8>>()
            .name("event_viewed")
            .create(&mut txn)?;

        let hashtags = env
            .database_options()
            .types::<UnalignedSlice<u8>, UnalignedSlice<u8>>()
            .flags(DatabaseFlags::DUP_SORT | DatabaseFlags::DUP_FIXED)
            .name("hashtags")
            .create(&mut txn)?;

        let relays = env
            .database_options()
            .types::<UnalignedSlice<u8>, UnalignedSlice<u8>>()
            .name("relays")
            .create(&mut txn)?;

        let events = env
            .database_options()
            .types::<UnalignedSlice<u8>, UnalignedSlice<u8>>()
            .name("events")
            .create(&mut txn)?;

        let event_ek_pk_index = env
            .database_options()
            .types::<UnalignedSlice<u8>, UnalignedSlice<u8>>()
            .flags(DatabaseFlags::DUP_SORT | DatabaseFlags::DUP_FIXED)
            .name("event_ek_pk_index")
            .create(&mut txn)?;

        let event_ek_c_index = env
            .database_options()
            .types::<UnalignedSlice<u8>, UnalignedSlice<u8>>()
            .flags(DatabaseFlags::DUP_SORT | DatabaseFlags::DUP_FIXED)
            .name("event_ek_c_index")
            .create(&mut txn)?;

        let event_references_person = env
            .database_options()
            .types::<UnalignedSlice<u8>, UnalignedSlice<u8>>()
            .flags(DatabaseFlags::DUP_SORT | DatabaseFlags::DUP_FIXED)
            .name("event_references_person")
            .create(&mut txn)?;

        let relationships = env
            .database_options()
            .types::<UnalignedSlice<u8>, UnalignedSlice<u8>>()
            .name("relationships")
            .create(&mut txn)?;

        let people = env
            .database_options()
            .types::<UnalignedSlice<u8>, UnalignedSlice<u8>>()
            .name("people")
            .create(&mut txn)?;

        let person_relays = env
            .database_options()
            .types::<UnalignedSlice<u8>, UnalignedSlice<u8>>()
            .name("person_relays")
            .create(&mut txn)?;

        let unindexed_giftwraps = env
            .database_options()
            .types::<UnalignedSlice<u8>, UnalignedSlice<u8>>()
            .name("unindexed_giftwraps")
            .create(&mut txn)?;

        txn.commit()?;

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
            unindexed_giftwraps,
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

    pub fn get_write_txn(&self) -> Result<RwTxn<'_>, Error> {
        Ok(self.env.write_txn()?)
    }

    pub fn get_general_len(&self) -> Result<u64, Error> {
        let txn = self.env.read_txn()?;
        Ok(self.general.len(&txn)?)
    }

    pub fn get_event_seen_on_relay_len(&self) -> Result<u64, Error> {
        let txn = self.env.read_txn()?;
        Ok(self.event_seen_on_relay.len(&txn)?)
    }

    pub fn get_event_viewed_len(&self) -> Result<u64, Error> {
        let txn = self.env.read_txn()?;
        Ok(self.event_viewed.len(&txn)?)
    }

    pub fn get_hashtags_len(&self) -> Result<u64, Error> {
        let txn = self.env.read_txn()?;
        Ok(self.hashtags.len(&txn)?)
    }

    pub fn get_relays_len(&self) -> Result<u64, Error> {
        let txn = self.env.read_txn()?;
        Ok(self.relays.len(&txn)?)
    }

    pub fn get_event_len(&self) -> Result<u64, Error> {
        let txn = self.env.read_txn()?;
        Ok(self.events.len(&txn)?)
    }

    pub fn get_event_ek_pk_index_len(&self) -> Result<u64, Error> {
        let txn = self.env.read_txn()?;
        Ok(self.event_ek_pk_index.len(&txn)?)
    }

    pub fn get_event_ek_c_index_len(&self) -> Result<u64, Error> {
        let txn = self.env.read_txn()?;
        Ok(self.event_ek_c_index.len(&txn)?)
    }

    pub fn get_event_references_person_len(&self) -> Result<u64, Error> {
        let txn = self.env.read_txn()?;
        Ok(self.event_references_person.len(&txn)?)
    }

    pub fn get_relationships_len(&self) -> Result<u64, Error> {
        let txn = self.env.read_txn()?;
        Ok(self.relationships.len(&txn)?)
    }

    pub fn get_people_len(&self) -> Result<u64, Error> {
        let txn = self.env.read_txn()?;
        Ok(self.people.len(&txn)?)
    }

    pub fn get_person_relays_len(&self) -> Result<u64, Error> {
        let txn = self.env.read_txn()?;
        Ok(self.person_relays.len(&txn)?)
    }

    // Remove all events (and related data) with a created_at before `from`
    pub fn prune(&self, from: Unixtime) -> Result<usize, Error> {
        // Extract the Ids to delete.
        let txn = self.env.read_txn()?;
        let mut ids: HashSet<Id> = HashSet::new();
        for result in self.events.iter(&txn)? {
            let (_key, val) = result?;

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
        drop(txn);

        let mut txn = self.env.write_txn()?;

        // Delete from event_seen_on_relay
        let mut deletions: Vec<Vec<u8>> = Vec::new();
        for id in &ids {
            let start_key: &[u8] = id.as_slice();
            for result in self.events.prefix_iter_mut(&mut txn, start_key)? {
                let (_key, val) = result?;
                deletions.push(val.to_owned());
            }
        }
        tracing::info!(
            "PRUNE: deleting {} records from event_seen_on_relay",
            deletions.len()
        );
        for deletion in deletions.drain(..) {
            self.event_seen_on_relay.delete(&mut txn, &deletion)?;
        }

        // Delete from event_viewed
        for id in &ids {
            let _ = self.event_viewed.delete(&mut txn, id.as_slice());
        }
        tracing::info!("PRUNE: deleted {} records from event_viewed", ids.len());

        // Delete from hashtags
        // (unfortunately since Ids are the values, we have to scan the whole thing)
        let mut deletions: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
        for result in self.hashtags.iter(&txn)? {
            let (key, val) = result?;
            let id = Id(val[0..32].try_into()?);
            if ids.contains(&id) {
                deletions.push((key.to_owned(), val.to_owned()));
            }
        }
        tracing::info!("PRUNE: deleting {} records from hashtags", deletions.len());
        for deletion in deletions.drain(..) {
            self.hashtags
                .delete_one_duplicate(&mut txn, &deletion.0, &deletion.1)?;
        }

        // Delete from relationships
        // (unfortunately because of the 2nd Id in the tag, we have to scan the whole thing)
        let mut deletions: Vec<Vec<u8>> = Vec::new();
        for result in self.relationships.iter(&txn)? {
            let (key, _val) = result?;
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
        tracing::info!("PRUNE: deleting {} relationships", deletions.len());
        for deletion in deletions.drain(..) {
            self.relationships.delete(&mut txn, &deletion)?;
        }

        // delete from events
        for id in &ids {
            let _ = self.events.delete(&mut txn, id.as_slice());
        }
        tracing::info!("PRUNE: deleted {} records from events", ids.len());

        txn.commit()?;

        Ok(ids.len())
    }

    pub fn write_migration_level<'a>(
        &'a self,
        migration_level: u32,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let bytes = migration_level.to_be_bytes();

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            Ok(self.general.put(txn, b"migration_level", &bytes)?)
        };

        match rw_txn {
            Some(txn) => {
                f(txn)?;
            }
            None => {
                let mut txn = self.env.write_txn()?;
                f(&mut txn)?;
                txn.commit()?;
            }
        };

        Ok(())
    }

    pub fn read_migration_level(&self) -> Result<Option<u32>, Error> {
        let txn = self.env.read_txn()?;

        Ok(self
            .general
            .get(&txn, b"migration_level")?
            .map(|bytes| u32::from_be_bytes(bytes[..4].try_into().unwrap())))
    }

    // This defines functions for read_{setting} and write_{setting} for each
    // setting value
    def_setting!(public_key, b"public_key", Option::<PublicKey>, None);
    def_setting!(log_n, b"log_n", u8, 18);
    def_setting!(offline, b"offline", bool, false);
    def_setting!(load_avatars, b"load_avatars", bool, true);
    def_setting!(load_media, b"load_media", bool, true);
    def_setting!(check_nip05, b"check_nip05", bool, true);
    def_setting!(
        automatically_fetch_metadata,
        b"automatically_fetch_metadata",
        bool,
        true
    );
    def_setting!(num_relays_per_person, b"num_relays_per_person", u8, 2);
    def_setting!(max_relays, b"max_relays", u8, 50);
    def_setting!(feed_chunk, b"feed_chunk", u64, 60 * 60 * 12);
    def_setting!(replies_chunk, b"replies_chunk", u64, 60 * 60 * 24 * 7);
    def_setting!(
        person_feed_chunk,
        b"person_feed_chunk",
        u64,
        60 * 60 * 24 * 30
    );
    def_setting!(overlap, b"overlap", u64, 300);
    def_setting!(reposts, b"reposts", bool, true);
    def_setting!(show_long_form, b"show_long_form", bool, false);
    def_setting!(show_mentions, b"show_mentions", bool, true);
    def_setting!(direct_messages, b"direct_messages", bool, true);
    def_setting!(
        future_allowance_secs,
        b"future_allowance_secs",
        u64,
        60 * 15
    );
    def_setting!(reactions, b"reactions", bool, true);
    def_setting!(enable_zap_receipts, b"enable_zap_receipts", bool, true);
    def_setting!(show_media, b"show_media", bool, true);
    def_setting!(
        approve_content_warning,
        b"approve_content_warning",
        bool,
        false
    );
    def_setting!(show_deleted_events, b"show_deleted_events", bool, false);
    def_setting!(pow, b"pow", u8, 0);
    def_setting!(set_client_tag, b"set_client_tag", bool, false);
    def_setting!(set_user_agent, b"set_user_agent", bool, false);
    def_setting!(delegatee_tag, b"delegatee_tag", String, String::new());
    def_setting!(max_fps, b"max_fps", u32, 12);
    def_setting!(
        recompute_feed_periodically,
        b"recompute_feed_periodically",
        bool,
        true
    );
    def_setting!(
        feed_recompute_interval_ms,
        b"feed_recompute_interval_ms",
        u32,
        8000
    );
    def_setting!(
        theme,
        b"theme",
        Theme,
        Theme {
            variant: ThemeVariant::Default,
            dark_mode: false,
            follow_os_dark_mode: false
        }
    );
    def_setting!(override_dpi, b"override_dpi", Option::<u32>, None);
    def_setting!(
        highlight_unread_events,
        b"highlight_unread_events",
        bool,
        true
    );
    def_setting!(posting_area_at_top, b"posting_area_at_top", bool, true);
    def_setting!(status_bar, b"status_bar", bool, false);
    def_setting!(
        image_resize_algorithm,
        b"image_resize_algorithm",
        String,
        "CatmullRom".to_owned()
    );
    def_setting!(
        relay_list_becomes_stale_hours,
        b"relay_list_becomes_stale_hours",
        u64,
        8
    );
    def_setting!(
        metadata_becomes_stale_hours,
        b"metadata_becomes_stale_hours",
        u64,
        8
    );
    def_setting!(
        nip05_becomes_stale_if_valid_hours,
        b"nip05_becomes_stale_if_valid_hours",
        u64,
        8
    );
    def_setting!(
        nip05_becomes_stale_if_invalid_minutes,
        b"nip05_becomes_stale_if_invalid_minutes",
        u64,
        30
    );
    def_setting!(
        avatar_becomes_stale_hours,
        b"avatar_becomes_stale_hours",
        u64,
        8
    );
    def_setting!(
        media_becomes_stale_hours,
        b"media_becomes_stale_hours",
        u64,
        8
    );
    def_setting!(
        max_websocket_message_size_kb,
        b"max_websocket_message_size_kb",
        usize,
        1024
    );
    def_setting!(
        max_websocket_frame_size_kb,
        b"max_websocket_frame_size_kb",
        usize,
        1024
    );
    def_setting!(
        websocket_accept_unmasked_frames,
        b"websocket_accept_unmasked_frames",
        bool,
        false
    );
    def_setting!(
        websocket_connect_timeout_sec,
        b"websocket_connect_timeout_sec",
        u64,
        15
    );
    def_setting!(
        websocket_ping_frequency_sec,
        b"websocket_ping_frequency_sec",
        u64,
        55
    );
    def_setting!(
        fetcher_metadata_looptime_ms,
        b"fetcher_metadata_looptime_ms",
        u64,
        3000
    );
    def_setting!(fetcher_looptime_ms, b"fetcher_looptime_ms", u64, 1800);
    def_setting!(
        fetcher_connect_timeout_sec,
        b"fetcher_connect_timeout_sec",
        u64,
        15
    );
    def_setting!(fetcher_timeout_sec, b"fetcher_timeout_sec", u64, 30);
    def_setting!(
        fetcher_max_requests_per_host,
        b"fetcher_max_requests_per_host",
        usize,
        3
    );
    def_setting!(
        fetcher_host_exclusion_on_low_error_secs,
        b"fetcher_host_exclusion_on_low_error_secs",
        u64,
        30
    );
    def_setting!(
        fetcher_host_exclusion_on_med_error_secs,
        b"fetcher_host_exclusion_on_med_error_secs",
        u64,
        60
    );
    def_setting!(
        fetcher_host_exclusion_on_high_error_secs,
        b"fetcher_host_exclusion_on_high_error_secs",
        u64,
        600
    );
    def_setting!(
        nip11_lines_to_output_on_error,
        b"nip11_lines_to_output_on_error",
        usize,
        10
    );
    def_setting!(prune_period_days, b"prune_period_days", u64, 90);
    def_setting!(cache_prune_period_days, b"cache_prune_period_days", u64, 90);

    pub fn write_encrypted_private_key<'a>(
        &'a self,
        epk: &Option<EncryptedPrivateKey>,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let bytes = epk.as_ref().map(|e| &e.0).write_to_vec()?;

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            self.general.put(txn, b"encrypted_private_key", &bytes)?;
            Ok(())
        };

        match rw_txn {
            Some(txn) => f(txn)?,
            None => {
                let mut txn = self.env.write_txn()?;
                f(&mut txn)?;
                txn.commit()?;
            }
        };

        Ok(())
    }

    pub fn read_encrypted_private_key(&self) -> Result<Option<EncryptedPrivateKey>, Error> {
        let txn = self.env.read_txn()?;

        match self.general.get(&txn, b"encrypted_private_key")? {
            None => Ok(None),
            Some(bytes) => {
                let os = Option::<String>::read_from_buffer(bytes)?;
                Ok(os.map(EncryptedPrivateKey))
            }
        }
    }

    pub fn write_last_contact_list_edit<'a>(
        &'a self,
        when: i64,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let bytes = when.to_be_bytes();

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            self.general
                .put(txn, b"last_contact_list_edit", bytes.as_slice())?;
            Ok(())
        };

        match rw_txn {
            Some(txn) => f(txn)?,
            None => {
                let mut txn = self.env.write_txn()?;
                f(&mut txn)?;
                txn.commit()?;
            }
        };

        Ok(())
    }

    pub fn read_last_contact_list_edit(&self) -> Result<Option<i64>, Error> {
        let txn = self.env.read_txn()?;

        match self.general.get(&txn, b"last_contact_list_edit")? {
            None => Ok(None),
            Some(bytes) => Ok(Some(i64::from_be_bytes(bytes[..8].try_into().unwrap()))),
        }
    }

    pub fn add_event_seen_on_relay<'a>(
        &'a self,
        id: Id,
        url: &RelayUrl,
        when: Unixtime,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let mut key: Vec<u8> = id.as_slice().to_owned();
        key.extend(url.0.as_bytes());
        key.truncate(MAX_LMDB_KEY);
        let bytes = when.0.to_be_bytes();

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            self.event_seen_on_relay.put(txn, &key, &bytes)?;
            Ok(())
        };

        match rw_txn {
            Some(txn) => f(txn)?,
            None => {
                let mut txn = self.env.write_txn()?;
                f(&mut txn)?;
                txn.commit()?;
            }
        };

        Ok(())
    }

    pub fn get_event_seen_on_relay(&self, id: Id) -> Result<Vec<(RelayUrl, Unixtime)>, Error> {
        let start_key: Vec<u8> = id.as_slice().to_owned();
        let txn = self.env.read_txn()?;
        let mut output: Vec<(RelayUrl, Unixtime)> = Vec::new();
        for result in self.event_seen_on_relay.prefix_iter(&txn, &start_key)? {
            let (key, val) = result?;

            // Extract off the Url
            let url = RelayUrl(std::str::from_utf8(&key[32..])?.to_owned());
            let time = Unixtime(i64::from_be_bytes(val[..8].try_into()?));
            output.push((url, time));
        }
        Ok(output)
    }

    pub fn mark_event_viewed<'a>(
        &'a self,
        id: Id,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let bytes = vec![];

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            self.event_viewed.put(txn, id.as_slice(), &bytes)?;
            Ok(())
        };

        match rw_txn {
            Some(txn) => f(txn)?,
            None => {
                let mut txn = self.env.write_txn()?;
                f(&mut txn)?;
                txn.commit()?;
            }
        };

        Ok(())
    }

    pub fn is_event_viewed(&self, id: Id) -> Result<bool, Error> {
        let txn = self.env.read_txn()?;
        Ok(self.event_viewed.get(&txn, id.as_slice())?.is_some())
    }

    pub fn add_hashtag<'a>(
        &'a self,
        hashtag: &String,
        id: Id,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let key = key!(hashtag.as_bytes());
        if key.is_empty() {
            return Err(ErrorKind::Empty("hashtag".to_owned()).into());
        }
        let bytes = id.as_slice();

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            self.hashtags.put(txn, key, bytes)?;
            Ok(())
        };

        match rw_txn {
            Some(txn) => f(txn)?,
            None => {
                let mut txn = self.env.write_txn()?;
                f(&mut txn)?;
                txn.commit()?;
            }
        };

        Ok(())
    }

    #[allow(dead_code)]
    pub fn get_event_ids_with_hashtag(&self, hashtag: &String) -> Result<Vec<Id>, Error> {
        let key = key!(hashtag.as_bytes());
        if key.is_empty() {
            return Err(ErrorKind::Empty("hashtag".to_owned()).into());
        }
        let txn = self.env.read_txn()?;
        let mut output: Vec<Id> = Vec::new();
        let iter = match self.hashtags.get_duplicates(&txn, key)? {
            Some(i) => i,
            None => return Ok(vec![]),
        };
        for result in iter {
            let (_key, val) = result?;
            let id = Id(val[0..32].try_into()?);
            output.push(id);
        }
        Ok(output)
    }

    pub fn write_relay<'a>(
        &'a self,
        relay: &Relay,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        // Note that we use serde instead of speedy because the complexity of the
        // serde_json::Value type makes it difficult. Any other serde serialization
        // should work though: Consider bincode.
        let key = key!(relay.url.0.as_bytes());
        if key.is_empty() {
            return Err(ErrorKind::Empty("relay url".to_owned()).into());
        }
        let bytes = serde_json::to_vec(relay)?;

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            self.relays.put(txn, key, &bytes)?;
            Ok(())
        };

        match rw_txn {
            Some(txn) => f(txn)?,
            None => {
                let mut txn = self.env.write_txn()?;
                f(&mut txn)?;
                txn.commit()?;
            }
        };

        Ok(())
    }

    pub fn delete_relay<'a>(
        &'a self,
        url: &RelayUrl,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        // Note that we use serde instead of speedy because the complexity of the
        // serde_json::Value type makes it difficult. Any other serde serialization
        // should work though: Consider bincode.
        let key = key!(url.0.as_bytes());
        if key.is_empty() {
            return Err(ErrorKind::Empty("relay url".to_owned()).into());
        }

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            // Delete any person_relay with this relay
            let mut deletions: Vec<Vec<u8>> = Vec::new();
            {
                for result in self.person_relays.iter(txn)? {
                    let (key, val) = result?;
                    if let Ok(person_relay) = PersonRelay::read_from_buffer(val) {
                        if person_relay.url == *url {
                            deletions.push(key.to_owned());
                        }
                    }
                }
            }
            for deletion in deletions.drain(..) {
                self.person_relays.delete(txn, &deletion)?;
            }

            // Delete the relay
            self.relays.delete(txn, key)?;
            Ok(())
        };

        match rw_txn {
            Some(txn) => f(txn)?,
            None => {
                let mut txn = self.env.write_txn()?;
                f(&mut txn)?;
                txn.commit()?;
            }
        };

        Ok(())
    }

    pub fn write_relay_if_missing<'a>(
        &'a self,
        url: &RelayUrl,
        rw_txn: Option<&mut RwTxn<'a>>,
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
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error>
    where
        M: FnMut(&mut Relay),
    {
        let key = key!(url.0.as_bytes());
        if key.is_empty() {
            return Err(ErrorKind::Empty("relay url".to_owned()).into());
        }

        let mut f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            let bytes = self.relays.get(txn, key)?;
            if let Some(bytes) = bytes {
                let mut relay = serde_json::from_slice(bytes)?;
                modify(&mut relay);
                let bytes = serde_json::to_vec(&relay)?;
                self.relays.put(txn, key, &bytes)?;
            }
            Ok(())
        };

        match rw_txn {
            Some(txn) => f(txn)?,
            None => {
                let mut txn = self.env.write_txn()?;
                f(&mut txn)?;
                txn.commit()?;
            }
        };

        Ok(())
    }

    pub fn modify_all_relays<'a, M>(
        &'a self,
        mut modify: M,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error>
    where
        M: FnMut(&mut Relay),
    {
        let mut f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            let mut iter = self.relays.iter_mut(txn)?;
            while let Some(result) = iter.next() {
                let (key, val) = result?;
                let mut dbrelay: Relay = serde_json::from_slice(val)?;
                modify(&mut dbrelay);
                let bytes = serde_json::to_vec(&dbrelay)?;
                // to deal with the unsafety of put_current
                let key = key.to_owned();
                unsafe {
                    iter.put_current(&key, &bytes)?;
                }
            }
            Ok(())
        };

        match rw_txn {
            Some(txn) => f(txn)?,
            None => {
                let mut txn = self.env.write_txn()?;
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
        let txn = self.env.read_txn()?;
        match self.relays.get(&txn, key)? {
            Some(bytes) => Ok(Some(serde_json::from_slice(bytes)?)),
            None => Ok(None),
        }
    }

    pub fn filter_relays<F>(&self, f: F) -> Result<Vec<Relay>, Error>
    where
        F: Fn(&Relay) -> bool,
    {
        let txn = self.env.read_txn()?;
        let mut output: Vec<Relay> = Vec::new();
        let iter = self.relays.iter(&txn)?;
        for result in iter {
            let (_key, val) = result?;
            let relay: Relay = serde_json::from_slice(val)?;
            if f(&relay) {
                output.push(relay);
            }
        }
        Ok(output)
    }

    pub fn process_relay_list(&self, event: &Event) -> Result<(), Error> {
        let mut txn = self.env.write_txn()?;

        // Check if this relay list is newer than the stamp we have for its author
        if let Some(mut person) = self.read_person(&event.pubkey)? {
            // Mark that we received it (changes fetch duration for next time)
            person.relay_list_last_received = Unixtime::now().unwrap().0;

            if let Some(previous_at) = person.relay_list_created_at {
                if event.created_at.0 <= previous_at {
                    return Ok(());
                }
            }

            // Mark when it was created
            person.relay_list_created_at = Some(event.created_at.0);

            // And save those marks in the Person record
            self.write_person(&person, Some(&mut txn))?;
        }

        let mut ours = false;
        if let Some(pubkey) = GLOBALS.storage.read_setting_public_key() {
            if event.pubkey == pubkey {
                tracing::info!("Processing our own relay list");
                ours = true;

                // Clear all current read/write bits (within the transaction)
                // note: inbox is kind10002 'read', outbox is kind10002 'write'
                self.modify_all_relays(
                    |relay| relay.usage_bits &= !(Relay::INBOX | Relay::OUTBOX),
                    Some(&mut txn),
                )?;
            }
        }

        // Collect the URLs for inbox(read) and outbox(write) specified in the event
        let mut inbox_relays: Vec<RelayUrl> = Vec::new();
        let mut outbox_relays: Vec<RelayUrl> = Vec::new();
        for tag in event.tags.iter() {
            if let Tag::Reference { url, marker, .. } = tag {
                if let Ok(relay_url) = RelayUrl::try_from_unchecked_url(url) {
                    if let Some(m) = marker {
                        match &*m.trim().to_lowercase() {
                            "read" => {
                                // 'read' means inbox and not outbox
                                inbox_relays.push(relay_url.clone());
                                if ours {
                                    if let Some(mut dbrelay) = self.read_relay(&relay_url)? {
                                        // Update
                                        dbrelay.set_usage_bits(Relay::INBOX);
                                        dbrelay.clear_usage_bits(Relay::OUTBOX);
                                        self.write_relay(&dbrelay, Some(&mut txn))?;
                                    } else {
                                        // Insert missing relay
                                        let mut dbrelay = Relay::new(relay_url.to_owned());
                                        // Since we are creating, we add READ
                                        dbrelay.set_usage_bits(Relay::INBOX | Relay::READ);
                                        self.write_relay(&dbrelay, Some(&mut txn))?;
                                    }
                                }
                            }
                            "write" => {
                                // 'write' means outbox and not inbox
                                outbox_relays.push(relay_url.clone());
                                if ours {
                                    if let Some(mut dbrelay) = self.read_relay(&relay_url)? {
                                        // Update
                                        dbrelay.set_usage_bits(Relay::OUTBOX);
                                        dbrelay.clear_usage_bits(Relay::INBOX);
                                        self.write_relay(&dbrelay, Some(&mut txn))?;
                                    } else {
                                        // Create
                                        let mut dbrelay = Relay::new(relay_url.to_owned());
                                        // Since we are creating, we add WRITE
                                        dbrelay.set_usage_bits(Relay::OUTBOX | Relay::WRITE);
                                        self.write_relay(&dbrelay, Some(&mut txn))?;
                                    }
                                }
                            }
                            _ => {} // ignore unknown marker
                        }
                    } else {
                        // No marker means both inbox and outbox
                        inbox_relays.push(relay_url.clone());
                        outbox_relays.push(relay_url.clone());
                        if ours {
                            if let Some(mut dbrelay) = self.read_relay(&relay_url)? {
                                // Update
                                dbrelay.set_usage_bits(Relay::INBOX | Relay::OUTBOX);
                                self.write_relay(&dbrelay, Some(&mut txn))?;
                            } else {
                                // Create
                                let mut dbrelay = Relay::new(relay_url.to_owned());
                                // Since we are creating, we add READ and WRITE
                                dbrelay.set_usage_bits(
                                    Relay::INBOX | Relay::OUTBOX | Relay::READ | Relay::WRITE,
                                );
                                self.write_relay(&dbrelay, Some(&mut txn))?;
                            }
                        }
                    }
                }
            }
        }

        self.set_relay_list(event.pubkey, inbox_relays, outbox_relays, Some(&mut txn))?;

        txn.commit()?;
        Ok(())
    }

    pub fn set_relay_list<'a>(
        &'a self,
        pubkey: PublicKey,
        read_relays: Vec<RelayUrl>,
        write_relays: Vec<RelayUrl>,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let mut person_relays = self.get_person_relays(pubkey)?;

        'for_read_relays: for relay in &read_relays {
            for pr in &person_relays {
                if pr.url == *relay {
                    continue 'for_read_relays;
                }
            }
            // Not found. Create a new person relay for this
            // (last loop below will set and save)
            let pr = PersonRelay::new(pubkey, relay.clone());
            person_relays.push(pr);
        }

        'for_write_relays: for relay in &write_relays {
            for pr in &person_relays {
                if pr.url == *relay {
                    continue 'for_write_relays;
                }
            }
            // Not found. Create a new person relay for this
            // (last loop below will set and save)
            let pr = PersonRelay::new(pubkey, relay.clone());
            person_relays.push(pr);
        }

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

    pub fn write_event<'a>(
        &'a self,
        event: &Event,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        // write to lmdb 'events'
        let bytes = event.write_to_vec()?;

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            self.events.put(txn, event.id.as_slice(), &bytes)?;

            // also index the event
            self.write_event_ek_pk_index(event, Some(txn))?;
            self.write_event_ek_c_index(event, Some(txn))?;
            self.write_event_references_person(event, Some(txn))?;
            Ok(())
        };

        match rw_txn {
            Some(txn) => f(txn)?,
            None => {
                let mut txn = self.env.write_txn()?;
                f(&mut txn)?;
                txn.commit()?;
            }
        };

        Ok(())
    }

    pub fn read_event(&self, id: Id) -> Result<Option<Event>, Error> {
        let txn = self.env.read_txn()?;
        match self.events.get(&txn, id.as_slice())? {
            None => Ok(None),
            Some(bytes) => Ok(Some(Event::read_from_buffer(bytes)?)),
        }
    }

    pub fn has_event(&self, id: Id) -> Result<bool, Error> {
        let txn = self.env.read_txn()?;
        match self.events.get(&txn, id.as_slice())? {
            None => Ok(false),
            Some(_) => Ok(true),
        }
    }

    pub fn delete_event<'a>(&'a self, id: Id, rw_txn: Option<&mut RwTxn<'a>>) -> Result<(), Error> {
        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            let _ = self.events.delete(txn, id.as_slice());
            Ok(())
        };

        match rw_txn {
            Some(txn) => f(txn)?,
            None => {
                let mut txn = self.env.write_txn()?;
                f(&mut txn)?;
                txn.commit()?;
            }
        };

        Ok(())
    }

    pub fn replace_event<'a>(
        &'a self,
        event: &Event,
        rw_txn: Option<&mut RwTxn<'a>>,
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
        rw_txn: Option<&mut RwTxn<'a>>,
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
        let txn = self.env.read_txn()?;

        for kind in kinds {
            let ek: u32 = (*kind).into();
            if pubkeys.is_empty() {
                let start_key = ek.to_be_bytes().as_slice().to_owned();
                let iter = self.event_ek_pk_index.prefix_iter(&txn, &start_key)?;
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
                    let iter = self.event_ek_pk_index.prefix_iter(&txn, &start_key)?;
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
        let txn = self.env.read_txn()?;

        for kind in kinds {
            let ek: u32 = (*kind).into();
            let mut start_key = ek.to_be_bytes().as_slice().to_owned();
            let mut end_key = start_key.clone();
            start_key.extend((i64::MAX - now.0).to_be_bytes().as_slice()); // work back from now
            end_key.extend((i64::MAX - since.0).to_be_bytes().as_slice()); // until since
            let range = (Bound::Included(&*start_key), Bound::Excluded(&*end_key));
            let iter = self.event_ek_c_index.range(&txn, &range)?;
            for result in iter {
                let (_key, val) = result?;
                // Take the event
                let id = Id(val[0..32].try_into()?);
                ids.insert(id);
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
        let txn = self.env.read_txn()?;
        let mut events: Vec<Event> = Vec::new();
        for id in ids {
            // this is like self.read_event(), but we supply our existing transaction
            if let Some(bytes) = self.events.get(&txn, id.as_slice())? {
                let event = Event::read_from_buffer(bytes)?;
                if f(&event) {
                    events.push(event);
                }
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
        let event_kinds = crate::feed::feed_displayable_event_kinds(true);

        let needle = regex::escape(text.to_lowercase().as_str());
        let re = regex::RegexBuilder::new(needle.as_str())
            .unicode(true)
            .case_insensitive(true)
            .build()?;

        let txn = self.env.read_txn()?;
        let iter = self.events.iter(&txn)?;
        let mut events: Vec<Event> = Vec::new();
        for result in iter {
            let (_key, val) = result?;

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

        events.sort_by(|a, b| {
            // ORDER created_at desc
            b.created_at.cmp(&a.created_at)
        });

        Ok(events)
    }

    // We don't call this externally. Whenever we write an event, we do this.
    fn write_event_ek_pk_index<'a>(
        &'a self,
        event: &Event,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            let mut event = event;

            // If giftwrap, index the inner rumor instead
            let mut rumor_event: Event;
            if event.kind == EventKind::GiftWrap {
                match GLOBALS.signer.unwrap_giftwrap(event) {
                    Ok(rumor) => {
                        rumor_event = rumor.into_event_with_bad_signature();
                        rumor_event.id = event.id; // lie, so it indexes it under the giftwrap
                        event = &rumor_event;
                    }
                    Err(e) => {
                        if matches!(e.kind, ErrorKind::NoPrivateKey) {
                            // Store as unindexed for later indexing
                            let bytes = vec![];
                            self.unindexed_giftwraps
                                .put(txn, event.id.as_slice(), &bytes)?;
                        }
                    }
                }
            }

            let ek: u32 = event.kind.into();
            let mut key: Vec<u8> = ek.to_be_bytes().as_slice().to_owned(); // event kind
            key.extend(event.pubkey.as_bytes()); // pubkey
            let bytes = event.id.as_slice();

            self.event_ek_pk_index.put(txn, &key, bytes)?;
            Ok(())
        };

        match rw_txn {
            Some(txn) => f(txn)?,
            None => {
                let mut txn = self.env.write_txn()?;
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
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            let mut event = event;

            // If giftwrap, index the inner rumor instead
            let mut rumor_event: Event;
            if event.kind == EventKind::GiftWrap {
                match GLOBALS.signer.unwrap_giftwrap(event) {
                    Ok(rumor) => {
                        rumor_event = rumor.into_event_with_bad_signature();
                        rumor_event.id = event.id; // lie, so it indexes it under the giftwrap
                        event = &rumor_event;
                    }
                    Err(e) => {
                        if matches!(e.kind, ErrorKind::NoPrivateKey) {
                            // Store as unindexed for later indexing
                            let bytes = vec![];
                            self.unindexed_giftwraps
                                .put(txn, event.id.as_slice(), &bytes)?;
                        }
                    }
                }
            }

            let ek: u32 = event.kind.into();
            let mut key: Vec<u8> = ek.to_be_bytes().as_slice().to_owned(); // event kind
            key.extend((i64::MAX - event.created_at.0).to_be_bytes().as_slice()); // reverse created_at
            let bytes = event.id.as_slice();

            self.event_ek_c_index.put(txn, &key, bytes)?;
            Ok(())
        };

        match rw_txn {
            Some(txn) => f(txn)?,
            None => {
                let mut txn = self.env.write_txn()?;
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
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            let mut event = event;

            // If giftwrap, index the inner rumor instead
            let mut rumor_event: Event;
            if event.kind == EventKind::GiftWrap {
                match GLOBALS.signer.unwrap_giftwrap(event) {
                    Ok(rumor) => {
                        rumor_event = rumor.into_event_with_bad_signature();
                        rumor_event.id = event.id; // lie, so it indexes it under the giftwrap
                        event = &rumor_event;
                    }
                    Err(e) => {
                        if matches!(e.kind, ErrorKind::NoPrivateKey) {
                            // Store as unindexed for later indexing
                            let bytes = vec![];
                            self.unindexed_giftwraps
                                .put(txn, event.id.as_slice(), &bytes)?;
                        }
                    }
                }
            }

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
                for pubkey in pubkeys.drain() {
                    let mut key: Vec<u8> = pubkey.to_bytes();
                    key.extend((i64::MAX - event.created_at.0).to_be_bytes().as_slice()); // reverse created_at
                    self.event_references_person.put(txn, &key, bytes)?;
                }
            }

            Ok(())
        };

        match rw_txn {
            Some(txn) => f(txn)?,
            None => {
                let mut txn = self.env.write_txn()?;
                f(&mut txn)?;
                txn.commit()?;
            }
        };

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
        let txn = self.env.read_txn()?;
        let now = Unixtime::now().unwrap();
        let mut start_key: Vec<u8> = pubkey.to_bytes();
        let mut end_key: Vec<u8> = start_key.clone();
        start_key.extend((i64::MAX - now.0).to_be_bytes().as_slice()); // work back from now
        end_key.extend((i64::MAX - since.0).to_be_bytes().as_slice()); // until since
        let range = (Bound::Included(&*start_key), Bound::Excluded(&*end_key));
        let iter = self.event_references_person.range(&txn, &range)?;
        let mut events: Vec<Event> = Vec::new();
        for result in iter {
            let (_key, val) = result?;

            // Take the event
            let id = Id(val[0..32].try_into()?);
            // (like read_event, but we supply our on transaction)
            if let Some(bytes) = self.events.get(&txn, id.as_slice())? {
                let event = Event::read_from_buffer(bytes)?;
                if f(&event) {
                    events.push(event);
                }
            }
        }
        Ok(events)
    }

    pub fn index_unindexed_giftwraps(&self) -> Result<(), Error> {
        if !GLOBALS.signer.is_ready() {
            return Err(ErrorKind::NoPrivateKey.into());
        }

        let mut ids: Vec<Id> = Vec::new();
        let txn = self.env.read_txn()?;
        let iter = self.unindexed_giftwraps.iter(&txn)?;
        for result in iter {
            let (key, _val) = result?;
            let a: [u8; 32] = key.try_into()?;
            let id = Id(a);
            ids.push(id);
        }

        let mut txn = self.env.write_txn()?;
        for id in ids {
            if let Some(event) = self.read_event(id)? {
                self.write_event_ek_pk_index(&event, Some(&mut txn))?;
                self.write_event_ek_c_index(&event, Some(&mut txn))?;
                self.write_event_references_person(&event, Some(&mut txn))?;
            }
            self.unindexed_giftwraps.delete(&mut txn, id.as_slice())?;
        }

        txn.commit()?;

        Ok(())
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
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let mut key = id.as_ref().as_slice().to_owned();
        key.extend(related.as_ref());
        let value = relationship.write_to_vec()?;

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            self.relationships.put(txn, &key, &value)?;
            Ok(())
        };

        match rw_txn {
            Some(txn) => f(txn)?,
            None => {
                let mut txn = self.env.write_txn()?;
                f(&mut txn)?;
                txn.commit()?;
            }
        };

        Ok(())
    }

    pub fn find_relationships(&self, id: Id) -> Result<Vec<(Id, Relationship)>, Error> {
        let start_key = id.as_slice();
        let txn = self.env.read_txn()?;
        let iter = self.relationships.prefix_iter(&txn, start_key)?;
        let mut output: Vec<(Id, Relationship)> = Vec::new();
        for result in iter {
            let (key, val) = result?;
            let id2 = Id(key[32..64].try_into().unwrap());
            let relationship = Relationship::read_from_buffer(val)?;
            output.push((id2, relationship));
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
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<Vec<Id>, Error> {
        let mut invalidate: Vec<Id> = Vec::new();

        let mut f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
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
                let mut txn = self.env.write_txn()?;
                f(&mut txn)?;
                txn.commit()?;
            }
        };

        Ok(invalidate)
    }

    pub fn write_person<'a>(
        &'a self,
        person: &Person,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        // Note that we use serde instead of speedy because the complexity of the
        // serde_json::Value type makes it difficult. Any other serde serialization
        // should work though: Consider bincode.
        let key: Vec<u8> = person.pubkey.to_bytes();
        let bytes = serde_json::to_vec(person)?;

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            self.people.put(txn, &key, &bytes)?;
            Ok(())
        };

        match rw_txn {
            Some(txn) => f(txn)?,
            None => {
                let mut txn = self.env.write_txn()?;
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
        let txn = self.env.read_txn()?;
        Ok(match self.people.get(&txn, &key)? {
            Some(bytes) => Some(serde_json::from_slice(bytes)?),
            None => None,
        })
    }

    pub fn write_person_if_missing<'a>(
        &'a self,
        pubkey: &PublicKey,
        rw_txn: Option<&mut RwTxn<'a>>,
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
        let txn = self.env.read_txn()?;
        let iter = self.people.iter(&txn)?;
        let mut output: Vec<Person> = Vec::new();
        for result in iter {
            let (_key, val) = result?;
            let person: Person = serde_json::from_slice(val)?;
            if f(&person) {
                output.push(person);
            }
        }
        Ok(output)
    }

    pub fn write_person_relay<'a>(
        &'a self,
        person_relay: &PersonRelay,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let mut key = person_relay.pubkey.to_bytes();
        key.extend(person_relay.url.0.as_bytes());
        key.truncate(MAX_LMDB_KEY);
        let bytes = person_relay.write_to_vec()?;

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            self.person_relays.put(txn, &key, &bytes)?;
            Ok(())
        };

        match rw_txn {
            Some(txn) => f(txn)?,
            None => {
                let mut txn = self.env.write_txn()?;
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
        let txn = self.env.read_txn()?;
        Ok(match self.person_relays.get(&txn, &key)? {
            Some(bytes) => Some(PersonRelay::read_from_buffer(bytes)?),
            None => None,
        })
    }

    pub fn get_person_relays(&self, pubkey: PublicKey) -> Result<Vec<PersonRelay>, Error> {
        let start_key = pubkey.to_bytes();
        let txn = self.env.read_txn()?;
        let iter = self.person_relays.prefix_iter(&txn, &start_key)?;
        let mut output: Vec<PersonRelay> = Vec::new();
        for result in iter {
            let (_key, val) = result?;
            let person_relay = PersonRelay::read_from_buffer(val)?;
            output.push(person_relay);
        }
        Ok(output)
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
                    ranked_relay.1 = (ranked_relay.1 as f32
                        * (relay.rank as f32 / 3.0)
                        * (relay.success_rate() * 2.0)) as u64;
                }
            }
        }

        // Resort
        ranked_relays.sort_by(|(_, score1), (_, score2)| score2.cmp(score1));

        let num_relays_per_person = self.read_setting_num_relays_per_person() as usize;

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

    /// Get all the DM channels with associated data
    pub fn dm_channels(&self) -> Result<Vec<DmChannelData>, Error> {
        let my_pubkey = match GLOBALS.signer.public_key() {
            Some(pk) => pk,
            None => return Ok(Vec::new()),
        };

        let events = self.find_events(
            &[EventKind::EncryptedDirectMessage, EventKind::GiftWrap],
            &[],
            Some(Unixtime(0)),
            |event| {
                if event.kind == EventKind::EncryptedDirectMessage {
                    event.pubkey == my_pubkey || event.is_tagged(&my_pubkey)
                    // Make sure if it has tags, only author and my_pubkey
                    // TBD
                } else {
                    event.kind == EventKind::GiftWrap
                }
            },
            false,
        )?;

        // Map from channel to latest-message-time and unread-count
        let mut map: HashMap<DmChannel, DmChannelData> = HashMap::new();

        for event in &events {
            let unread = 1 - self.is_event_viewed(event.id)? as usize;
            if event.kind == EventKind::EncryptedDirectMessage {
                let time = event.created_at;
                let dmchannel = {
                    if event.pubkey != my_pubkey {
                        // DM sent to me
                        DmChannel::new(&[event.pubkey])
                    } else {
                        // DM sent from me
                        let mut maybe_channel: Option<DmChannel> = None;
                        for tag in event.tags.iter() {
                            if let Tag::Pubkey { pubkey, .. } = tag {
                                if let Ok(pk) = PublicKey::try_from(pubkey) {
                                    if pk != my_pubkey {
                                        maybe_channel = Some(DmChannel::new(&[pk]));
                                    }
                                }
                            }
                        }
                        match maybe_channel {
                            Some(dmchannel) => dmchannel,
                            None => continue,
                        }
                    }
                };
                map.entry(dmchannel.clone())
                    .and_modify(|d| {
                        d.latest_message = d.latest_message.max(time);
                        d.message_count += 1;
                        d.unread_message_count += unread;
                    })
                    .or_insert(DmChannelData {
                        dm_channel: dmchannel,
                        latest_message: time,
                        message_count: 1,
                        unread_message_count: unread,
                    });
            } else if event.kind == EventKind::GiftWrap {
                if let Ok(rumor) = GLOBALS.signer.unwrap_giftwrap(event) {
                    let rumor_event = rumor.into_event_with_bad_signature();
                    let time = rumor_event.created_at;
                    let dmchannel = {
                        let mut people: Vec<PublicKey> = rumor_event
                            .people()
                            .iter()
                            .filter_map(|(pk, _, _)| PublicKey::try_from(pk).ok())
                            .filter(|pk| *pk != my_pubkey)
                            .collect();
                        people.push(rumor_event.pubkey); // include author too
                        DmChannel::new(&people)
                    };
                    map.entry(dmchannel.clone())
                        .and_modify(|d| {
                            d.latest_message = d.latest_message.max(time);
                            d.message_count += 1;
                            d.unread_message_count += unread;
                        })
                        .or_insert(DmChannelData {
                            dm_channel: dmchannel,
                            latest_message: time,
                            message_count: 1,
                            unread_message_count: unread,
                        });
                }
            }
        }

        let mut output: Vec<DmChannelData> = map.drain().map(|e| e.1).collect();
        output.sort_by(|a, b| b.latest_message.cmp(&a.latest_message));
        Ok(output)
    }

    /// Get DM events (by id) in a channel
    pub fn dm_events(&self, channel: &DmChannel) -> Result<Vec<Id>, Error> {
        let my_pubkey = match GLOBALS.signer.public_key() {
            Some(pk) => pk,
            None => return Ok(Vec::new()),
        };

        let mut pass1 = self.find_events(
            &[EventKind::EncryptedDirectMessage, EventKind::GiftWrap],
            &[],
            Some(Unixtime(0)),
            |event| {
                if event.kind == EventKind::EncryptedDirectMessage {
                    let other = &channel.keys()[0];
                    let people = event.people();
                    channel.keys().len() == 1
                        && ((event.pubkey == my_pubkey
                            && event.is_tagged(other)
                            && (people.len() == 1
                                || (people.len() == 2 && event.is_tagged(&my_pubkey))))
                            || (event.pubkey == *other
                                && event.is_tagged(&my_pubkey)
                                && (people.len() == 1
                                    || (people.len() == 2 && event.is_tagged(other)))))
                } else if event.kind == EventKind::GiftWrap {
                    // Decrypt in next pass, else we would have to decrypt twice
                    true
                } else {
                    false
                }
            },
            false,
        )?;

        let mut pass2: Vec<Event> = Vec::new();

        for event in pass1.drain(..) {
            if event.kind == EventKind::EncryptedDirectMessage {
                pass2.push(event); // already validated
            } else if event.kind == EventKind::GiftWrap {
                if let Ok(rumor) = GLOBALS.signer.unwrap_giftwrap(&event) {
                    let mut rumor_event = rumor.into_event_with_bad_signature();
                    rumor_event.id = event.id; // lie, so it indexes it under the giftwrap
                    let mut tagged: Vec<PublicKey> = rumor_event
                        .people()
                        .drain(..)
                        .filter_map(|(pkh, _, _)| PublicKey::try_from(pkh).ok())
                        .collect();
                    tagged.push(rumor_event.pubkey); // include author
                    tagged.retain(|pk| *pk != my_pubkey); // never include user
                    let this_channel = DmChannel::new(&tagged);
                    if this_channel == *channel {
                        pass2.push(event);
                    }
                }
            }
        }

        // sort
        pass2.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(pass2.iter().map(|e| e.id).collect())
    }

    pub fn rebuild_event_indices(&self) -> Result<(), Error> {
        let mut wtxn = self.env.write_txn()?;
        let mut last_key = Id([0; 32]);
        while let Some((key, val)) = self.events.get_greater_than(&wtxn, last_key.as_slice())? {
            let id = Id::read_from_buffer(key)?;
            let event = Event::read_from_buffer(val)?;
            self.write_event_ek_pk_index(&event, Some(&mut wtxn))?;
            self.write_event_ek_c_index(&event, Some(&mut wtxn))?;
            self.write_event_references_person(&event, Some(&mut wtxn))?;
            last_key = id;
        }
        wtxn.commit()?;
        GLOBALS.storage.sync()?;
        Ok(())
    }

    pub fn sync(&self) -> Result<(), Error> {
        self.env.force_sync()?;
        Ok(())
    }
}
