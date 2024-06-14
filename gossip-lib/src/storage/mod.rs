include!("macros");

const MAX_LMDB_KEY: usize = 511;

mod migrations;

// type implementations
pub mod types;

// table definition
pub mod table;
pub use table::Table;

// new tables
pub mod person3_table;
pub use person3_table::Person3Table;
pub type PersonTable = Person3Table;

// database implementations
mod event_akci_index;
use event_akci_index::AkciKey;
mod event_kci_index;
use event_kci_index::KciKey;

mod event_ek_c_index1;
mod event_ek_pk_index1;
mod event_seen_on_relay1;
mod event_tag_index1;
mod event_viewed1;
mod events1;
mod events2;
mod events3;
mod hashtags1;
mod nip46servers1;
mod nip46servers2;
mod people1;
mod people2;
mod person_lists1;
mod person_lists2;
mod person_lists_metadata1;
mod person_lists_metadata2;
mod person_lists_metadata3;
mod person_relays1;
mod person_relays2;
mod relationships1;
mod relationships_by_addr1;
mod relationships_by_addr2;
mod relationships_by_id1;
mod relationships_by_id2;
mod relays1;
mod relays2;
mod reprel1;
mod unindexed_giftwraps1;
mod versioned;

use crate::dm_channel::{DmChannel, DmChannelData};
use crate::error::{Error, ErrorKind};
use crate::globals::GLOBALS;
use crate::misc::Private;
use crate::nip46::{Nip46Server, Nip46UnconnectedServer};
use crate::people::{PersonList, PersonListMetadata};
use crate::person_relay::PersonRelay;
use crate::profile::Profile;
use crate::relationship::{RelationshipByAddr, RelationshipById};
use crate::relay::Relay;
use heed::types::{Bytes, Unit};
use heed::{Database, Env, EnvFlags, EnvOpenOptions, RoTxn, RwTxn};
use nostr_types::{
    EncryptedPrivateKey, Event, EventAddr, EventKind, EventReference, Filter, Id, MilliSatoshi,
    PublicKey, PublicKeyHex, RelayList, RelayUrl, RelayUsage, Unixtime,
};
use paste::paste;
use speedy::{Readable, Writable};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::ops::Bound;

use self::event_kci_index::INDEXED_KINDS;
use self::event_tag_index1::INDEXED_TAGS;

type RawDatabase = Database<Bytes, Bytes>;
type EmptyDatabase = Database<Bytes, Unit>;

/// The LMDB storage engine.
///
/// All calls are synchronous but fast so callers can just wait on them.
pub struct Storage {
    env: Env,

    // General database (settings, local_settings)
    general: RawDatabase,
}

impl Storage {
    pub(crate) fn new() -> Result<Storage, Error> {
        let mut builder = EnvOpenOptions::new();
        unsafe {
            builder.flags(EnvFlags::NO_TLS);
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

        let dir = Profile::current()?.lmdb_dir;
        let env = unsafe {
            match builder.open(&dir) {
                Ok(env) => env,
                Err(e) => {
                    tracing::error!("Unable to open LMDB at {}", dir.display());
                    return Err(e.into());
                }
            }
        };

        let mut txn = env.write_txn()?;

        let general = env
            .database_options()
            .types::<Bytes, Bytes>()
            .create(&mut txn)?;

        txn.commit()?;

        Ok(Storage { env, general })
    }

    /// Run this after GLOBALS lazy static initialisation, so functions within storage can
    /// access GLOBALS without hanging.
    pub fn init(&self) -> Result<(), Error> {
        // We have to trigger all of the current-version databases into existence
        // because otherwise there will be MVCC visibility problems later having
        // different transactions in parallel
        //
        // old-version databases will be handled by their migration code and only
        // triggered into existence if their migration is necessary.
        let _ = self.db_event_akci_index()?;
        let _ = self.db_event_kci_index()?;
        let _ = self.db_event_tag_index()?;
        let _ = self.db_events()?;
        let _ = self.db_event_seen_on_relay()?;
        let _ = self.db_event_viewed()?;
        let _ = self.db_hashtags()?;
        let _ = self.db_nip46servers()?;
        let _ = self.db_person_relays()?;
        let _ = self.db_relationships_by_id()?;
        let _ = self.db_relationships_by_addr()?;
        let _ = self.db_relays()?;
        let _ = self.db_unindexed_giftwraps()?;
        let _ = self.db_person_lists()?;
        let _ = self.db_person_lists_metadata()?;
        let _ = PersonTable::db()?;

        // Do migrations
        match self.read_migration_level()? {
            Some(level) => self.migrate(level)?,
            None => self.init_from_empty()?,
        }

        Ok(())
    }

    /// Get a write transaction. With it, you can do multiple writes before you commit it.
    /// Bundling multiple writes together is more efficient.
    pub fn get_write_txn(&self) -> Result<RwTxn<'_>, Error> {
        Ok(self.env.write_txn()?)
    }

    /// Get a read transaction.
    pub fn get_read_txn(&self) -> Result<RoTxn<'_>, Error> {
        Ok(self.env.read_txn()?)
    }

    /// Sync the data to disk. This happens periodically, but sometimes it's useful to force
    /// it.
    pub fn sync(&self) -> Result<(), Error> {
        self.env.force_sync()?;
        Ok(())
    }

    // Database getters ---------------------------------

    #[inline]
    pub(crate) fn db_event_tag_index(&self) -> Result<RawDatabase, Error> {
        self.db_event_tag_index1()
    }

    #[inline]
    pub(crate) fn db_events(&self) -> Result<RawDatabase, Error> {
        self.db_events3()
    }

    #[inline]
    pub(crate) fn db_event_seen_on_relay(&self) -> Result<RawDatabase, Error> {
        self.db_event_seen_on_relay1()
    }

    #[inline]
    pub(crate) fn db_event_viewed(&self) -> Result<RawDatabase, Error> {
        self.db_event_viewed1()
    }

    #[inline]
    pub(crate) fn db_hashtags(&self) -> Result<RawDatabase, Error> {
        self.db_hashtags1()
    }

    #[inline]
    pub(crate) fn db_nip46servers(&self) -> Result<RawDatabase, Error> {
        self.db_nip46servers2()
    }

    #[inline]
    pub(crate) fn db_person_relays(&self) -> Result<RawDatabase, Error> {
        self.db_person_relays2()
    }

    #[inline]
    pub(crate) fn db_relationships_by_addr(&self) -> Result<RawDatabase, Error> {
        self.db_relationships_by_addr2()
    }

    #[inline]
    pub(crate) fn db_relationships_by_id(&self) -> Result<RawDatabase, Error> {
        self.db_relationships_by_id2()
    }

    #[inline]
    pub(crate) fn db_relays(&self) -> Result<RawDatabase, Error> {
        self.db_relays2()
    }

    #[inline]
    pub(crate) fn db_unindexed_giftwraps(&self) -> Result<RawDatabase, Error> {
        self.db_unindexed_giftwraps1()
    }

    #[inline]
    pub(crate) fn db_person_lists(&self) -> Result<RawDatabase, Error> {
        self.db_person_lists2()
    }

    #[inline]
    pub(crate) fn db_person_lists_metadata(&self) -> Result<RawDatabase, Error> {
        self.db_person_lists_metadata3()
    }

    // Database length functions ---------------------------------

    /// The number of records in the general table
    pub fn get_general_len(&self) -> Result<u64, Error> {
        let txn = self.env.read_txn()?;
        Ok(self.general.len(&txn)?)
    }

    /// The number of records in the event_seen_on table
    #[inline]
    pub fn get_event_seen_on_relay_len(&self) -> Result<u64, Error> {
        self.get_event_seen_on_relay1_len()
    }

    /// The number of records in the event_viewed table
    #[inline]
    pub fn get_event_viewed_len(&self) -> Result<u64, Error> {
        self.get_event_viewed1_len()
    }

    /// The number of records in the hashtags table
    pub fn get_hashtags_len(&self) -> Result<u64, Error> {
        let txn = self.env.read_txn()?;
        Ok(self.db_hashtags()?.len(&txn)?)
    }

    /// The number of records in the nip46servers table
    pub fn get_nip46servers_len(&self) -> Result<u64, Error> {
        let txn = self.env.read_txn()?;
        Ok(self.db_nip46servers()?.len(&txn)?)
    }

    /// The number of records in the relays table
    #[inline]
    pub fn get_relays_len(&self) -> Result<u64, Error> {
        self.get_relays2_len()
    }

    /// The number of records in the event table
    pub fn get_event_len(&self) -> Result<u64, Error> {
        let txn = self.env.read_txn()?;
        Ok(self.db_events()?.len(&txn)?)
    }

    /// The number of records in the event_akci_index table
    pub fn get_event_akci_index_len(&self) -> Result<u64, Error> {
        let txn = self.env.read_txn()?;
        Ok(self.db_event_akci_index()?.len(&txn)?)
    }

    /// The number of records in the event_kci_index table
    pub fn get_event_kci_index_len(&self) -> Result<u64, Error> {
        let txn = self.env.read_txn()?;
        Ok(self.db_event_kci_index()?.len(&txn)?)
    }

    /// The number of records in the event_tag index table
    pub fn get_event_tag_index_len(&self) -> Result<u64, Error> {
        let txn = self.env.read_txn()?;
        Ok(self.db_event_tag_index()?.len(&txn)?)
    }

    /// The number of records in the relationships_by_addr table
    #[inline]
    pub fn get_relationships_by_addr_len(&self) -> Result<u64, Error> {
        let txn = self.env.read_txn()?;
        Ok(self.db_relationships_by_addr()?.len(&txn)?)
    }

    /// The number of records in the relationships_by_id table
    #[inline]
    pub fn get_relationships_by_id_len(&self) -> Result<u64, Error> {
        let txn = self.env.read_txn()?;
        Ok(self.db_relationships_by_id()?.len(&txn)?)
    }

    /// The number of records in the person_relays table
    #[inline]
    pub fn get_person_relays_len(&self) -> Result<u64, Error> {
        self.get_person_relays2_len()
    }

    /// The number of records in the person_lists table
    pub fn get_person_lists_len(&self) -> Result<u64, Error> {
        let txn = self.env.read_txn()?;
        Ok(self.db_person_lists()?.len(&txn)?)
    }

    // Prune -------------------------------------------------------

    /// Remove all events (and related data) with a created_at before `from`
    /// and all related indexes.
    pub fn prune(&self, from: Unixtime) -> Result<usize, Error> {
        // Extract the Ids to delete.
        let txn = self.env.read_txn()?;
        let mut ids: HashSet<Id> = HashSet::new();
        for result in self.db_events()?.iter(&txn)? {
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
            for result in self
                .db_event_seen_on_relay()?
                .prefix_iter(&txn, start_key)?
            {
                let (_key, val) = result?;
                deletions.push(val.to_owned());
            }
        }
        tracing::info!(
            "PRUNE: deleting {} records from event_seen_on_relay",
            deletions.len()
        );
        for deletion in deletions.drain(..) {
            self.db_event_seen_on_relay()?.delete(&mut txn, &deletion)?;
        }

        // Delete from event_viewed
        for id in &ids {
            let _ = self.db_event_viewed()?.delete(&mut txn, id.as_slice());
        }
        tracing::info!("PRUNE: deleted {} records from event_viewed", ids.len());

        // Delete from hashtags
        // (unfortunately since Ids are the values, we have to scan the whole thing)
        let mut deletions: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
        for result in self.db_hashtags()?.iter(&txn)? {
            let (key, val) = result?;
            let id = Id(val[0..32].try_into()?);
            if ids.contains(&id) {
                deletions.push((key.to_owned(), val.to_owned()));
            }
        }
        tracing::info!("PRUNE: deleting {} records from hashtags", deletions.len());
        for deletion in deletions.drain(..) {
            self.db_hashtags()?
                .delete_one_duplicate(&mut txn, &deletion.0, &deletion.1)?;
        }

        // Delete from relationships
        // (unfortunately because of the 2nd Id in the tag, we have to scan the whole thing)
        let mut deletions: Vec<Vec<u8>> = Vec::new();
        for result in self.db_relationships_by_id()?.iter(&txn)? {
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
            self.db_relationships_by_id()?.delete(&mut txn, &deletion)?;
        }

        // delete from events
        for id in &ids {
            let _ = self.db_events()?.delete(&mut txn, id.as_slice());
        }
        tracing::info!("PRUNE: deleted {} records from events", ids.len());

        txn.commit()?;

        Ok(ids.len())
    }

    // General key-value functions --------------------------------------------------

    pub(crate) fn write_migration_level<'a>(
        &'a self,
        migration_level: u32,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let bytes = migration_level.to_be_bytes();

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            Ok(self.general.put(txn, b"migration_level", &bytes)?)
        };

        write_transact!(self, rw_txn, f)
    }

    pub(crate) fn read_migration_level(&self) -> Result<Option<u32>, Error> {
        let txn = self.env.read_txn()?;

        Ok(self
            .general
            .get(&txn, b"migration_level")?
            .map(|bytes| u32::from_be_bytes(bytes[..4].try_into().unwrap())))
    }

    /// Write the user's encrypted private key
    pub fn write_encrypted_private_key<'a>(
        &'a self,
        epk: Option<&EncryptedPrivateKey>,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let bytes = epk.map(|e| &e.0).write_to_vec()?;

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            self.general.put(txn, b"encrypted_private_key", &bytes)?;
            Ok(())
        };

        write_transact!(self, rw_txn, f)
    }

    /// Read the user's encrypted private key
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

    /// Write NIP-46 unconnected server
    #[allow(dead_code)]
    pub fn write_nip46_unconnected_server<'a>(
        &'a self,
        server: &Nip46UnconnectedServer,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let bytes = server.write_to_vec()?;

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            self.general.put(txn, b"nip46_unconnected_server", &bytes)?;
            Ok(())
        };

        write_transact!(self, rw_txn, f)
    }

    /// Read NIP-46 unconnected server
    #[allow(dead_code)]
    pub fn read_nip46_unconnected_server(&self) -> Result<Option<Nip46UnconnectedServer>, Error> {
        let txn = self.env.read_txn()?;
        match self.general.get(&txn, b"nip46_unconnected_server")? {
            None => Ok(None),
            Some(bytes) => {
                let server = Nip46UnconnectedServer::read_from_buffer(bytes)?;
                Ok(Some(server))
            }
        }
    }

    /// Delete a NIP-46 unconnected server
    #[allow(dead_code)]
    pub fn delete_nip46_unconnected_server<'a>(
        &'a self,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            self.general.delete(txn, b"nip46_unconnected_server")?;
            Ok(())
        };

        write_transact!(self, rw_txn, f)
    }

    // Flags ------------------------------------------------------------

    def_flag!(following_only, b"following_only", false);
    def_flag!(wizard_complete, b"wizard_complete", false);
    def_flag!(
        rebuild_relationships_needed,
        b"rebuild_relationships_needed",
        false
    );
    def_flag!(rebuild_indexes_needed, b"rebuild_indexes_needed", false);
    def_flag!(
        reprocess_relay_lists_needed,
        b"reprocess_relay_lists_needed",
        true
    );

    // Settings ----------------------------------------------------------

    // This defines functions for read_{setting} and write_{setting} for each
    // setting value
    def_setting!(public_key, b"public_key", Option::<PublicKey>, None);
    def_setting!(log_n, b"log_n", u8, 18);
    def_setting!(login_at_startup, b"login_at_startup", bool, true);
    def_setting!(offline, b"offline", bool, false);
    def_setting!(load_avatars, b"load_avatars", bool, true);
    def_setting!(load_media, b"load_media", bool, true);
    def_setting!(check_nip05, b"check_nip05", bool, true);
    def_setting!(wgpu_renderer, b"wgpu_renderer", bool, false);
    def_setting!(
        automatically_fetch_metadata,
        b"automatically_fetch_metadata",
        bool,
        true
    );
    def_setting!(
        relay_connection_requires_approval,
        b"relay_connection_requires_approval",
        bool,
        false
    );
    def_setting!(
        relay_auth_requires_approval,
        b"relay_auth_requires_approval",
        bool,
        false
    );
    def_setting!(num_relays_per_person, b"num_relays_per_person", u8, 2);
    def_setting!(max_relays, b"max_relays", u8, 50);
    def_setting!(load_more_count, b"load_more_count", u64, 35);
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
    def_setting!(hide_mutes_entirely, b"hide_mutes_entirely", bool, true);
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
        feed_thread_scroll_to_main_event,
        b"feed_thread_scroll_to_main_event",
        bool,
        true
    );
    def_setting!(
        theme_variant,
        b"theme_variant",
        String,
        "Default".to_owned()
    );
    def_setting!(dark_mode, b"dark_mode", bool, false);
    def_setting!(follow_os_dark_mode, b"follow_os_dark_mode", bool, true);
    def_setting!(override_dpi, b"override_dpi", Option::<u32>, None);
    def_setting!(
        highlight_unread_events,
        b"highlight_unread_events",
        bool,
        true
    );
    def_setting!(feed_newest_at_bottom, b"feed_newest_at_bottom", bool, false);
    def_setting!(posting_area_at_top, b"posting_area_at_top", bool, true);
    def_setting!(status_bar, b"status_bar", bool, false);
    def_setting!(
        image_resize_algorithm,
        b"image_resize_algorithm",
        String,
        "CatmullRom".to_owned()
    );
    def_setting!(inertial_scrolling, b"inertial_scrolling", bool, true);
    def_setting!(mouse_acceleration, b"mouse_acceleration", f32, 1.0);
    def_setting!(
        relay_list_becomes_stale_minutes,
        b"relay_list_becomes_stale_minutes",
        u64,
        20
    );
    def_setting!(
        metadata_becomes_stale_minutes,
        b"metadata_becomes_stale_minutes",
        u64,
        20
    );
    def_setting!(
        nip05_becomes_stale_if_valid_hours,
        b"nip05_becomes_stale_if_valid_hours",
        u64,
        6
    );
    def_setting!(
        nip05_becomes_stale_if_invalid_minutes,
        b"nip05_becomes_stale_if_invalid_minutes",
        u64,
        15
    );
    def_setting!(
        avatar_becomes_stale_hours,
        b"avatar_becomes_stale_hours",
        u64,
        5
    );
    def_setting!(
        media_becomes_stale_hours,
        b"media_becomes_stale_hours",
        u64,
        5
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
        1750
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
    def_setting!(
        avoid_spam_on_unsafe_relays,
        b"avoid_spam_on_unsafe_relays",
        bool,
        false
    );

    // -------------------------------------------------------------------

    /// Get personlist metadata
    #[inline]
    pub fn get_person_list_metadata(
        &self,
        list: PersonList,
    ) -> Result<Option<PersonListMetadata>, Error> {
        self.get_person_list_metadata3(list)
    }

    /// Set personlist metadata
    #[inline]
    pub fn set_person_list_metadata<'a>(
        &'a self,
        list: PersonList,
        metadata: &PersonListMetadata,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        self.set_person_list_metadata3(list, metadata, rw_txn)
    }

    /// Get all person lists with their metadata
    #[inline]
    pub fn get_all_person_list_metadata(
        &self,
    ) -> Result<Vec<(PersonList, PersonListMetadata)>, Error> {
        self.get_all_person_list_metadata3()
    }

    /// Find a person list by "d" tag
    #[inline]
    pub fn find_person_list_by_dtag(
        &self,
        dtag: &str,
    ) -> Result<Option<(PersonList, PersonListMetadata)>, Error> {
        self.find_person_list_by_dtag3(dtag)
    }

    /// Allocate a new person list
    #[inline]
    pub fn allocate_person_list<'a>(
        &'a self,
        metadata: &PersonListMetadata,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<PersonList, Error> {
        self.allocate_person_list3(metadata, rw_txn)
    }

    /// Deallocate an empty person list
    #[inline]
    pub fn deallocate_person_list<'a>(
        &'a self,
        list: PersonList,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        self.deallocate_person_list3(list, rw_txn)
    }

    pub fn rename_person_list<'a>(
        &'a self,
        list: PersonList,
        newname: String,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let mut md = match self.get_person_list_metadata(list)? {
            Some(md) => md,
            None => return Err(ErrorKind::ListNotFound.into()),
        };
        md.title = newname;
        md.last_edit_time = Unixtime::now().unwrap();
        self.set_person_list_metadata(list, &md, rw_txn)?;
        Ok(())
    }

    /// Add event seen on relay
    #[inline]
    pub fn add_event_seen_on_relay<'a>(
        &'a self,
        id: Id,
        url: &RelayUrl,
        when: Unixtime,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        // Don't save banned relay URLs
        if Self::url_is_banned(url) {
            return Ok(());
        }

        self.add_event_seen_on_relay1(id, url, when, rw_txn)
    }

    /// Get event seen on relay
    #[inline]
    pub fn get_event_seen_on_relay(&self, id: Id) -> Result<Vec<(RelayUrl, Unixtime)>, Error> {
        self.get_event_seen_on_relay1(id)
    }

    /// Mark event viewed
    #[inline]
    pub fn mark_event_viewed<'a>(
        &'a self,
        id: Id,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        self.mark_event_viewed1(id, rw_txn)
    }

    /// Is an event viewed?
    #[inline]
    pub fn is_event_viewed(&self, id: Id) -> Result<bool, Error> {
        self.is_event_viewed1(id)
    }

    /// Associate a hashtag to an event
    #[inline]
    pub fn add_hashtag<'a>(
        &'a self,
        hashtag: &String,
        id: Id,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        self.add_hashtag1(hashtag, id, rw_txn)
    }

    /// Get events with a given hashtag
    #[inline]
    #[allow(dead_code)]
    pub fn get_event_ids_with_hashtag(&self, hashtag: &String) -> Result<Vec<Id>, Error> {
        self.get_event_ids_with_hashtag1(hashtag)
    }

    /// Write a relay record.
    ///
    /// NOTE: this overwrites. You may wish to read first, or you might prefer
    /// [modify_relay](Storage::modify_relay)
    #[inline]
    pub(crate) fn write_relay<'a>(
        &'a self,
        relay: &Relay,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        self.write_relay2(relay, rw_txn)
    }

    /// Delete a relay record
    #[inline]
    #[allow(dead_code)]
    pub fn delete_relay<'a>(
        &'a self,
        url: &RelayUrl,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        self.delete_relay2(url, rw_txn)
    }

    /// Write a new relay record only if it is missing
    pub fn write_relay_if_missing<'a>(
        &'a self,
        url: &RelayUrl,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        // Don't save banned relay URLs
        if Self::url_is_banned(url) {
            return Ok(());
        }

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            let rtxn = &**txn;
            if self.read_relay(url, Some(rtxn))?.is_none() {
                let dbrelay = Relay::new(url.to_owned());
                self.write_relay(&dbrelay, Some(txn))?;
            }
            Ok(())
        };

        write_transact!(self, rw_txn, f)
    }

    /// Modify a relay record
    #[inline]
    pub(crate) fn modify_relay<'a, M>(
        &'a self,
        url: &RelayUrl,
        modify: M,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error>
    where
        M: FnMut(&mut Relay),
    {
        self.modify_relay2(url, modify, rw_txn)
    }

    //// Modify all relay records
    #[inline]
    pub fn modify_all_relays<'a, M>(
        &'a self,
        modify: M,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error>
    where
        M: FnMut(&mut Relay),
    {
        self.modify_all_relays2(modify, rw_txn)
    }

    /// Read a relay record
    #[inline]
    pub fn read_relay<'a>(
        &'a self,
        url: &RelayUrl,
        txn: Option<&RoTxn<'a>>,
    ) -> Result<Option<Relay>, Error> {
        self.read_relay2(url, txn)
    }

    /// Read or create relay
    pub fn read_or_create_relay<'a>(
        &'a self,
        url: &RelayUrl,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<Relay, Error> {
        // Don't save banned relay URLs
        if Self::url_is_banned(url) {
            return Ok(Relay::new(url.to_owned()));
        }

        let f = |txn: &mut RwTxn<'a>| -> Result<Relay, Error> {
            let rtxn = &**txn;
            match self.read_relay(url, Some(rtxn))? {
                Some(relay) => Ok(relay),
                None => {
                    let relay = Relay::new(url.to_owned());
                    self.write_relay(&relay, Some(txn))?;
                    Ok(relay)
                }
            }
        };

        write_transact!(self, rw_txn, f)
    }

    /// Read matching relay records
    #[inline]
    pub fn filter_relays<F>(&self, f: F) -> Result<Vec<Relay>, Error>
    where
        F: Fn(&Relay) -> bool,
    {
        self.filter_relays2(f)
    }

    /// Load effective relay list
    pub fn load_effective_relay_list(&self) -> Result<RelayList, Error> {
        let mut relay_list: RelayList = Default::default();

        for relay in self.filter_relays(|_| true)? {
            if relay.has_usage_bits(Relay::READ | Relay::WRITE) {
                relay_list.0.insert(relay.url, RelayUsage::Both);
            } else if relay.has_usage_bits(Relay::WRITE) {
                relay_list.0.insert(relay.url, RelayUsage::Outbox);
            } else if relay.has_usage_bits(Relay::READ) {
                relay_list.0.insert(relay.url, RelayUsage::Inbox);
            }
        }

        Ok(relay_list)
    }

    /// Load advertised relay list
    pub fn load_advertised_relay_list(&self) -> Result<RelayList, Error> {
        let mut relay_list: RelayList = Default::default();

        for relay in self.filter_relays(|_| true)? {
            if relay.has_usage_bits(Relay::INBOX | Relay::OUTBOX) {
                relay_list.0.insert(relay.url, RelayUsage::Both);
            } else if relay.has_usage_bits(Relay::OUTBOX) {
                relay_list.0.insert(relay.url, RelayUsage::Outbox);
            } else if relay.has_usage_bits(Relay::INBOX) {
                relay_list.0.insert(relay.url, RelayUsage::Inbox);
            }
        }

        Ok(relay_list)
    }

    /// Process a DM relay list event
    pub fn process_dm_relay_list(&self, event: &Event) -> Result<(), Error> {
        let mut txn = self.env.write_txn()?;

        // Determine if this is our own DM relay list
        let mut ours = false;
        if let Some(pubkey) = self.read_setting_public_key() {
            if event.pubkey == pubkey {
                tracing::info!("Processing our own dm relay list");
                ours = true;
            }
        }

        // Update the person.dm_relay_list_created_at field
        {
            let mut person = PersonTable::read_or_create_record(event.pubkey, Some(&mut txn))?;

            // Bail out if this list wasn't newer than the last one we processed
            if let Some(prior_created_at) = person.dm_relay_list_created_at {
                if prior_created_at >= *event.created_at {
                    txn.commit()?; // because we may have created the person record.
                    return Ok(());
                }
            }

            person.dm_relay_list_created_at = Some(*event.created_at);

            PersonTable::write_record(&mut person, Some(&mut txn))?;
        }

        // Clear all current 'dm' flags in all matching person_relays
        {
            self.modify_all_persons_relays(event.pubkey, |pr| pr.dm = false, Some(&mut txn))?;
        }

        // Extract relays from event
        let mut relays: Vec<RelayUrl> = Vec::new();
        for tag in event.tags.iter() {
            if tag.tagname() == "relay" {
                if let Ok(relay_url) = RelayUrl::try_from_str(tag.value()) {
                    // Don't use banned relay URLs
                    if !Self::url_is_banned(&relay_url) {
                        relays.push(relay_url);
                    }
                }
            }
        }

        // Set 'dm' flags in person_relay record
        for relay_url in relays.iter() {
            self.modify_person_relay(event.pubkey, relay_url, |pr| pr.dm = true, Some(&mut txn))?;
        }

        if ours {
            // Clear all relay DM flags
            self.modify_all_relays(|relay| relay.clear_usage_bits(Relay::DM), Some(&mut txn))?;

            for relay_url in relays.iter() {
                // Set DM flag in relay
                self.modify_relay(
                    relay_url,
                    |relay| relay.set_usage_bits(Relay::DM),
                    Some(&mut txn),
                )?;
            }
        }

        txn.commit()?;
        Ok(())
    }

    /// Process a relay list event
    pub fn process_relay_list<'a>(
        &'a self,
        event: &Event,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            if let Some(mut person) = PersonTable::read_record(event.pubkey, Some(txn))? {
                // Check if this relay list is newer than the stamp we have for its author
                if let Some(previous_at) = person.relay_list_created_at {
                    if event.created_at.0 <= previous_at {
                        // This list is old.
                        return Ok(());
                    }
                }
                // If we got here, the list is new.

                // Mark when it was created
                person.relay_list_created_at = Some(event.created_at.0);

                // And save those marks in the Person record
                PersonTable::write_record(&mut person, Some(txn))?;
            }

            let mut ours = false;
            if let Some(pubkey) = self.read_setting_public_key() {
                if event.pubkey == pubkey {
                    tracing::info!("Processing our own relay list");
                    ours = true;
                }
            }

            let relay_list = RelayList::from_event(event);

            if ours {
                // If INBOX or OUTBOX is set, we also must turn on READ and WRITE
                // or they won't actually get used.  However, we don't turn OFF
                // these bits automatically.

                // Clear all current read/write bits (within the transaction)
                // note: inbox is kind10002 'read', outbox is kind10002 'write'
                self.modify_all_relays(
                    |relay| relay.clear_usage_bits(Relay::INBOX | Relay::OUTBOX),
                    Some(txn),
                )?;

                // Set or create read relays
                for (relay_url, usage) in relay_list.0.iter() {
                    let bits = match usage {
                        RelayUsage::Inbox => Relay::INBOX | Relay::READ,
                        RelayUsage::Outbox => Relay::OUTBOX | Relay::WRITE,
                        RelayUsage::Both => {
                            Relay::INBOX | Relay::OUTBOX | Relay::READ | Relay::WRITE
                        }
                    };

                    if let Some(mut dbrelay) = self.read_relay(relay_url, Some(txn))? {
                        dbrelay.set_usage_bits(bits);
                        self.write_relay(&dbrelay, Some(txn))?;
                    } else {
                        let mut dbrelay = Relay::new(relay_url.to_owned());
                        dbrelay.set_usage_bits(bits);
                        self.write_relay(&dbrelay, Some(txn))?;
                    }
                }
            }

            self.set_relay_list(event.pubkey, relay_list, Some(txn))?;

            Ok(())
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

    /// Set the user's relay list
    pub fn set_relay_list<'a>(
        &'a self,
        pubkey: PublicKey,
        relay_list: RelayList,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            // Clear all current relay settings for this person
            self.modify_all_persons_relays(
                pubkey,
                |pr| {
                    pr.read = false;
                    pr.write = false;
                },
                Some(txn),
            )?;

            // Apply relay list
            for (relay_url, usage) in relay_list.0.iter() {
                self.modify_person_relay(
                    pubkey,
                    relay_url,
                    |pr| {
                        pr.read = *usage == RelayUsage::Inbox || *usage == RelayUsage::Both;
                        pr.write = *usage == RelayUsage::Outbox || *usage == RelayUsage::Both;
                    },
                    Some(txn),
                )?;
            }

            Ok(())
        };

        write_transact!(self, rw_txn, f)
    }

    /// Write an event
    #[inline]
    pub fn write_event<'a>(
        &'a self,
        event: &Event,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        self.write_event3(event, rw_txn)
    }

    /// Read an event
    #[inline]
    pub fn read_event(&self, id: Id) -> Result<Option<Event>, Error> {
        self.read_event3(id)
    }

    /// If we have th event
    #[inline]
    pub fn has_event(&self, id: Id) -> Result<bool, Error> {
        self.has_event3(id)
    }

    #[inline]
    pub fn read_event_reference(&self, eref: &EventReference) -> Result<Option<Event>, Error> {
        match eref {
            EventReference::Id { id, .. } => self.read_event(*id),
            EventReference::Addr(ea) => self.get_replaceable_event(ea.kind, ea.author, &ea.d),
        }
    }

    /// Delete the event
    pub fn delete_event<'a>(&'a self, id: Id, rw_txn: Option<&mut RwTxn<'a>>) -> Result<(), Error> {
        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            // Delete from the events table
            self.delete_event3(id, Some(txn))?;

            // Delete from event_seen_on_relay
            {
                // save the actual keys to delete
                let mut deletions: Vec<Vec<u8>> = Vec::new();

                let start_key: &[u8] = id.as_slice();

                for result in self.db_event_seen_on_relay()?.prefix_iter(txn, start_key)? {
                    let (_key, val) = result?;
                    deletions.push(val.to_owned());
                }

                // actual deletion done in second pass
                // (deleting during iteration does not work in LMDB)
                for deletion in deletions.drain(..) {
                    self.db_event_seen_on_relay()?.delete(txn, &deletion)?;
                }
            }

            // Delete from event_viewed
            self.db_event_viewed()?.delete(txn, id.as_slice())?;

            // DO NOT delete from relationships. The related event still applies in case
            // this event comes back, ESPECIALLY deletion relationships!

            // We cannot delete from numerous indexes because the ID
            // is in the value, not in the key.
            //
            // These invalid entries will be deleted next time we
            // rebuild indexes.
            //
            // These include
            //   db_event_hashtags()
            //   db_relationships(), where the ID is the 2nd half of the key
            //   db_reprel()
            //   db_event_akci_index()
            //   db_event_kci_index()

            Ok(())
        };

        write_transact!(self, rw_txn, f)
    }

    /// Replace any existing event with the passed in event, if it is of a replaceable kind
    /// and is newer.
    pub fn replace_event<'a>(
        &'a self,
        event: &Event,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<bool, Error> {
        if !event.kind.is_replaceable() {
            return Err(ErrorKind::General("Event is not replaceable.".to_owned()).into());
        }

        let mut filter = Filter::new();
        filter.add_event_kind(event.kind);
        filter.add_author(&event.pubkey.into());
        let existing = self.find_events_by_filter(&filter, |e| {
            if event.kind.is_parameterized_replaceable() {
                e.parameter() == event.parameter()
            } else {
                true
            }
        })?;

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

    /// Get the matching replaceable event (possibly parameterized)
    /// TBD: optimize this by storing better event indexes
    pub fn get_replaceable_event(
        &self,
        kind: EventKind,
        pubkey: PublicKey,
        parameter: &str,
    ) -> Result<Option<Event>, Error> {
        if !kind.is_replaceable() {
            return Err(ErrorKind::General("Event kind is not replaceable".to_owned()).into());
        }

        let mut filter = Filter::new();
        filter.add_event_kind(kind);
        filter.add_author(&pubkey.into());

        Ok(self
            .find_events_by_filter(&filter, |e| {
                if kind.is_parameterized_replaceable() {
                    e.parameter().as_deref() == Some(parameter)
                } else {
                    true
                }
            })?
            .first()
            .cloned())
    }

    /// Find events by filter.
    ///
    /// This function may inefficiently scrape all of storage for some filters.
    /// To avoid an inefficient scrape, do one of these
    ///
    /// 1. Supply some ids
    /// 2. Supply some single-letter tag(s) that we index, or
    /// 3. Supply some authors and some kinds, or
    /// 4. Supply some kinds, all of which are INDEXED_KINDS,
    ///
    /// The output will be sorted in reverse time order.
    pub fn find_events_by_filter<F>(&self, filter: &Filter, screen: F) -> Result<Vec<Event>, Error>
    where
        F: Fn(&Event) -> bool,
    {
        let txn = self.env.read_txn()?;

        // We insert into a BTreeSet to keep them time-ordered
        let mut output: BTreeSet<Event> = BTreeSet::new();

        let mut since = filter.since.unwrap_or(Unixtime(0));
        let until = filter.until.unwrap_or(Unixtime(i64::MAX));
        let limit = filter.limit.unwrap_or(usize::MAX);

        if !filter.ids.is_empty() {
            // Use events table directly, we have specific ids

            for idhex in filter.ids.iter() {
                let id: Id = idhex.clone().into();
                if output.len() >= limit {
                    break;
                }
                if let Some(bytes) = self.db_events()?.get(&txn, id.as_slice())? {
                    let event = Event::read_from_buffer(bytes)?;
                    if filter.event_matches(&event) && screen(&event) {
                        output.insert(event);
                    }
                }
            }
        } else if !filter.tags.is_empty()
            && filter
                .tags
                .iter()
                .all(|t| INDEXED_TAGS.contains(&&*t.0.to_string()))
        {
            // event_tag_index
            for tag in &filter.tags {
                let mut start_key: Vec<u8> = tag.0.to_string().as_bytes().to_owned();
                start_key.push(b'\"'); // double quote separator, unlikely to be inside of a tagname
                if let Some(tv) = tag.1.first() {
                    start_key.extend(tv.as_bytes());
                }
                let start_key = key!(&start_key); // limit the size
                let iter = self.db_event_tag_index()?.prefix_iter(&txn, start_key)?;
                for result in iter {
                    let (_key, val) = result?;
                    // Take the event
                    let id = Id(val[0..32].try_into()?);
                    if let Some(bytes) = self.db_events()?.get(&txn, id.as_slice())? {
                        let event = Event::read_from_buffer(bytes)?;
                        if filter.event_matches(&event) && screen(&event) {
                            output.insert(event);
                        }
                    }
                }
            }
        } else if !filter.authors.is_empty() && !filter.kinds.is_empty() {
            // akci
            for pkh in &filter.authors {
                let author = PublicKey::try_from_hex_string(pkh.as_str(), true)?;
                for kind in &filter.kinds {
                    let iter = {
                        let start_prefix = AkciKey::from_parts(author, *kind, until, Id([0; 32]));
                        let end_prefix = AkciKey::from_parts(author, *kind, since, Id([255; 32]));
                        let range = (
                            Bound::Included(start_prefix.as_slice()),
                            Bound::Excluded(end_prefix.as_slice()),
                        );
                        self.db_event_akci_index()?.range(&txn, &range)?
                    };

                    // Count how many we have found of this author-kind pair, so we
                    // can possibly update `since`
                    let mut paircount = 0;

                    'per_event: for result in iter {
                        let (keybytes, _) = result?;
                        let key = AkciKey::from_bytes(keybytes)?;
                        let (_, _, created_at, id) = key.into_parts()?;
                        if let Some(bytes) = self.db_events()?.get(&txn, id.as_slice())? {
                            let event = Event::read_from_buffer(bytes)?;

                            // If we have gone beyond since, we can stop early
                            // (We have to check because `since` might change in this loop)
                            if created_at < since {
                                break 'per_event;
                            }

                            // check against the rest of the filter
                            if filter.event_matches(&event) && screen(&event) {
                                output.insert(event);
                                paircount += 1;

                                // Stop this pair if limited
                                if paircount >= limit {
                                    if created_at > since {
                                        since = created_at;
                                    }
                                    break 'per_event;
                                }

                                // If kind is replaceable (and not parameterized)
                                // then don't take any more events from this author-kind
                                // pair.
                                // NOTE that this optimization is difficult to implement
                                // for other replaceable event situations
                                if kind.is_replaceable() {
                                    break 'per_event;
                                }
                            }
                        }
                    }
                }
            }
        } else if !filter.kinds.is_empty() && filter.kinds.iter().all(|k| INDEXED_KINDS.contains(k))
        {
            for kind in &filter.kinds {
                let iter = {
                    let start_prefix = KciKey::from_parts(*kind, until, Id([0; 32]));
                    let end_prefix = KciKey::from_parts(*kind, since, Id([255; 32]));
                    let range = (
                        Bound::Included(start_prefix.as_slice()),
                        Bound::Excluded(end_prefix.as_slice()),
                    );
                    self.db_event_kci_index()?.range(&txn, &range)?
                };

                // Count how many we have found of this kind, can possibly update
                // `since`
                let mut kindcount = 0;

                'per_event: for result in iter {
                    let (keybytes, _) = result?;
                    let key = KciKey::from_bytes(keybytes)?;
                    let (_, created_at, id) = key.into_parts()?;
                    if let Some(bytes) = self.db_events()?.get(&txn, id.as_slice())? {
                        let event = Event::read_from_buffer(bytes)?;

                        // If we have gone beyond since, we can stop early
                        // (We have to check because `since` might change in this loop)
                        if created_at < since {
                            break 'per_event;
                        }

                        // check against the rest of the filter
                        if filter.event_matches(&event) && screen(&event) {
                            output.insert(event);
                            kindcount += 1;

                            // Stop this kind if limited
                            if kindcount >= limit {
                                if created_at > since {
                                    since = created_at;
                                }
                                break 'per_event;
                            }
                        }
                    }
                }
            }
        } else if !filter.kinds.is_empty() {
            // kind scrape (can't use kci since kinds include some that are not indexed)
            tracing::warn!("KINDS SCRAPE OF STORAGE");
            let iter = self.db_events()?.iter(&txn)?;
            for result in iter {
                let (_key, bytes) = result?;
                if let Some(kind) = Event::get_kind_from_speedy_bytes(bytes) {
                    if filter.kinds.contains(&kind) {
                        let event = Event::read_from_buffer(bytes)?;
                        if filter.event_matches(&event) && screen(&event) {
                            output.insert(event);
                            // We can't stop at a limit because our data is unsorted
                        }
                    }
                }
            }
        } else if !filter.authors.is_empty() {
            // author scrape
            tracing::warn!("AUTHOR SCRAPE OF STORAGE");
            let iter = self.db_events()?.iter(&txn)?;
            for result in iter {
                let (_key, bytes) = result?;
                if let Some(author) = Event::get_pubkey_from_speedy_bytes(bytes) {
                    let pkh: PublicKeyHex = author.into();
                    if filter.authors.contains(&pkh) {
                        let event = Event::read_from_buffer(bytes)?;
                        if filter.event_matches(&event) && screen(&event) {
                            output.insert(event);
                        }
                    }
                }
            }
        } else {
            // full scrape
            tracing::warn!("FULL SCRAPE OF STORAGE");
            let iter = self.db_events()?.iter(&txn)?;
            for result in iter {
                let (_key, bytes) = result?;
                let event = Event::read_from_buffer(bytes)?;
                if filter.event_matches(&event) && screen(&event) {
                    output.insert(event);
                }
            }
        }

        Ok(output
            .iter()
            .rev()
            .take(limit)
            .cloned() // FIXME when BTreeSet gets a drain() function
            .collect())
    }

    /// Search all events for the text, case insensitive. Both content and tags
    /// are searched.
    pub fn search_events(&self, text: &str) -> Result<Vec<Event>, Error> {
        let event_kinds = crate::feed::feed_displayable_event_kinds(true);

        let needle = regex::escape(text.to_lowercase().as_str());
        let re = regex::RegexBuilder::new(needle.as_str())
            .unicode(true)
            .case_insensitive(true)
            .build()?;

        let txn = self.env.read_txn()?;
        let iter = self.db_events()?.iter(&txn)?;
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
            b.created_at.cmp(&a.created_at).then(b.id.cmp(&a.id))
        });

        Ok(events)
    }

    fn switch_to_rumor<'a>(
        &'a self,
        event: &Event,
        txn: &mut RwTxn<'a>,
    ) -> Result<Option<Event>, Error> {
        self.switch_to_rumor3(event, txn)
    }

    // We don't call this externally. Whenever we write an event, we do this
    fn write_event_akci_index<'a>(
        &'a self,
        pubkey: PublicKey,
        kind: EventKind,
        created_at: Unixtime,
        id: Id,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            let key = AkciKey::from_parts(pubkey, kind, created_at, id);

            self.db_event_akci_index()?.put(txn, key.as_slice(), &())?;
            Ok(())
        };

        write_transact!(self, rw_txn, f)
    }

    // We don't call this externally. Whenever we write an event, we do this
    fn write_event_kci_index<'a>(
        &'a self,
        kind: EventKind,
        created_at: Unixtime,
        id: Id,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        // Only index if it is of an indexable kind
        if !INDEXED_KINDS.contains(&kind) {
            return Ok(());
        }

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            let key = KciKey::from_parts(kind, created_at, id);

            self.db_event_kci_index()?.put(txn, key.as_slice(), &())?;
            Ok(())
        };

        write_transact!(self, rw_txn, f)
    }

    // This should be called with the outer giftwrap
    fn write_event_tag_index<'a>(
        &'a self,
        event: &Event,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        self.write_event3_tag_index1(event, rw_txn)
    }

    #[inline]
    pub(crate) fn index_unindexed_giftwraps(&self) -> Result<(), Error> {
        self.index_unindexed_giftwraps1()
    }

    pub(crate) fn get_highest_local_parent_event_id(&self, id: Id) -> Result<Option<Id>, Error> {
        let event = match self.read_event(id)? {
            Some(event) => event,
            None => return Ok(None),
        };

        match event.replies_to() {
            Some(EventReference::Id { id: parent_id, .. }) => {
                self.get_highest_local_parent_event_id(parent_id)
            }
            Some(EventReference::Addr(ea)) => {
                match self.get_replaceable_event(ea.kind, ea.author, &ea.d)? {
                    Some(event) => self.get_highest_local_parent_event_id(event.id),
                    None => Ok(Some(event.id)),
                }
            }
            None => Ok(Some(event.id)),
        }
    }

    /// Write a relationship between two events
    ///
    /// The second Id relates to the first Id,
    /// e.g. related replies to id, or related deletes id
    #[inline]
    pub(crate) fn write_relationship_by_id<'a>(
        &'a self,
        id: Id,
        related: Id,
        relationship_by_id: RelationshipById,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        self.write_relationship_by_id2(id, related, relationship_by_id, rw_txn)
    }

    /// Find relationships belonging to the given event
    ///
    /// The found Ids relates to the passed in Id,
    /// e.g. result id replies to id, or result id deletes id
    #[inline]
    pub fn find_relationships_by_id(&self, id: Id) -> Result<Vec<(Id, RelationshipById)>, Error> {
        self.find_relationships_by_id2(id)
    }

    /// Write a relationship between an event and an EventAddr (replaceable)
    #[inline]
    pub(crate) fn write_relationship_by_addr<'a>(
        &'a self,
        addr: EventAddr,
        related: Id,
        relationship_by_addr: RelationshipByAddr,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        self.write_relationship_by_addr2(addr, related, relationship_by_addr, rw_txn)
    }

    /// Find relationships belonging to the given event to replaceable events
    #[inline]
    pub fn find_relationships_by_addr(
        &self,
        addr: &EventAddr,
    ) -> Result<Vec<(Id, RelationshipByAddr)>, Error> {
        self.find_relationships_by_addr2(addr)
    }

    /// Get replies to the given event
    pub fn get_replies(&self, event: &Event) -> Result<Vec<Id>, Error> {
        let mut output = self.get_non_replaceable_replies(event.id)?;
        output.extend(self.get_replaceable_replies(&EventAddr {
            d: event.parameter().unwrap_or("".to_string()),
            relays: vec![],
            kind: event.kind,
            author: event.pubkey,
        })?);

        let annotation_children = self.get_non_replaceable_annotates(event.id)?;
        for annotation in annotation_children.iter() {
            // Extend with children of annotation
            output.extend(self.get_non_replaceable_replies(*annotation)?);
        }

        Ok(output)
    }

    pub fn get_non_replaceable_annotates(&self, id: Id) -> Result<Vec<Id>, Error> {
        Ok(self
            .find_relationships_by_id(id)?
            .iter()
            .filter_map(|(id, rel)| {
                if *rel == RelationshipById::Annotates {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect())
    }

    pub fn get_non_replaceable_replies(&self, id: Id) -> Result<Vec<Id>, Error> {
        Ok(self
            .find_relationships_by_id(id)?
            .iter()
            .filter_map(|(id, rel)| {
                if *rel == RelationshipById::RepliesTo {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect())
    }

    pub fn get_replaceable_replies(&self, addr: &EventAddr) -> Result<Vec<Id>, Error> {
        Ok(self
            .find_relationships_by_addr(addr)?
            .iter()
            .filter_map(|(id, rel)| {
                if *rel == RelationshipByAddr::RepliesTo {
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

        // Get the event (once self-reactions get deleted we can remove this)
        let maybe_target_event = self.read_event(id)?;

        // Collect up to one reaction per pubkey
        let mut phase1: HashMap<PublicKey, char> = HashMap::new();
        for (_, rel) in self.find_relationships_by_id(id)? {
            if let RelationshipById::ReactsTo { by, reaction } = rel {
                if let Some(target_event) = &maybe_target_event {
                    if target_event.pubkey == by {
                        // Do not let people like their own post
                        continue;
                    }
                }
                let symbol: char = if let Some(ch) = reaction.chars().next() {
                    ch
                } else {
                    '+'
                };
                phase1.insert(by, symbol);
                if Some(by) == GLOBALS.identity.public_key() {
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

    /// Get the zap total of a given event
    pub fn get_zap_total(&self, id: Id) -> Result<MilliSatoshi, Error> {
        let mut total = MilliSatoshi(0);
        for (_, rel) in self.find_relationships_by_id(id)? {
            if let RelationshipById::Zaps { by: _, amount } = rel {
                total = total + amount;
            }
        }
        Ok(total)
    }

    /// Get whether an event was deleted, and if so the optional reason
    pub fn get_deletions(&self, maybe_deleted_event: &Event) -> Result<Vec<String>, Error> {
        let mut reasons: Vec<String> = Vec::new();

        for (deleting_id, rel) in self.find_relationships_by_id(maybe_deleted_event.id)? {
            if let RelationshipById::Deletes { by, reason } = rel {
                if maybe_deleted_event.delete_author_allowed(by) {
                    // We must have the deletion event to check it
                    if let Some(deleting_event) = self.read_event(deleting_id)? {
                        // Delete must come after event in question
                        if deleting_event.created_at > maybe_deleted_event.created_at {
                            reasons.push(reason);
                        }
                    }
                }
            }
        }

        // Deletes via 'a tags (entire parameterized groups)
        if let Some(parameter) = maybe_deleted_event.parameter() {
            let addr = EventAddr {
                d: parameter,
                relays: vec![],
                kind: maybe_deleted_event.kind,
                author: maybe_deleted_event.pubkey,
            };
            for (deleting_id, rel) in self.find_relationships_by_addr(&addr)? {
                // Must be a deletion relationship
                if let RelationshipByAddr::Deletes { by, reason } = rel {
                    if maybe_deleted_event.delete_author_allowed(by) {
                        // We must have the deletion event to check it
                        if let Some(deleting_event) = self.read_event(deleting_id)? {
                            // Delete must come after event in question
                            if deleting_event.created_at > maybe_deleted_event.created_at {
                                reasons.push(reason);
                            }
                        }
                    }
                }
            }
        }

        Ok(reasons)
    }

    /// Get annotations for an event
    pub fn get_annotations(&self, event: &Event) -> Result<Vec<(Unixtime, String)>, Error> {
        let mut annotations: Vec<(Unixtime, String)> = Vec::new();
        for (other_id, rel) in self.find_relationships_by_id(event.id)? {
            if rel == RelationshipById::Annotates {
                if let Some(event) = self.read_event(other_id)? {
                    annotations.push((event.created_at, event.content));
                }
            }
        }

        annotations.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

        Ok(annotations)
    }

    /// Read a PersonRelay record
    #[inline]
    pub fn read_person_relay(
        &self,
        pubkey: PublicKey,
        url: &RelayUrl,
    ) -> Result<Option<PersonRelay>, Error> {
        self.read_person_relay2(pubkey, url)
    }

    /// Write a PersonRelay record
    #[inline]
    pub fn write_person_relay<'a>(
        &'a self,
        person_relay: &PersonRelay,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        // Don't save banned relay URLs
        if Self::url_is_banned(&person_relay.url) {
            return Ok(());
        }

        self.write_person_relay2(person_relay, rw_txn)
    }

    /// Modify a specific person relay record
    pub fn modify_person_relay<'a, M>(
        &'a self,
        pubkey: PublicKey,
        url: &RelayUrl,
        modify: M,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error>
    where
        M: FnMut(&mut PersonRelay),
    {
        self.modify_person_relay2(pubkey, url, modify, rw_txn)
    }

    /// Read a person record, create if missing
    #[inline]
    pub fn read_or_create_person_relay<'a>(
        &'a self,
        pubkey: PublicKey,
        url: &RelayUrl,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<PersonRelay, Error> {
        // Don't save banned relay URLs
        if Self::url_is_banned(url) {
            return Ok(PersonRelay::new(pubkey.to_owned(), url.to_owned()));
        }

        match self.read_person_relay(pubkey, url)? {
            Some(pr) => Ok(pr),
            None => {
                let person_relay = PersonRelay::new(pubkey.to_owned(), url.to_owned());
                self.write_person_relay(&person_relay, rw_txn)?;
                Ok(person_relay)
            }
        }
    }

    /// get PersonRelay records for a person
    #[inline]
    pub fn get_person_relays(&self, pubkey: PublicKey) -> Result<Vec<PersonRelay>, Error> {
        self.get_person_relays2(pubkey)
    }

    /// Do we have any PersonRelay records for the person?
    #[inline]
    pub fn have_persons_relays(&self, pubkey: PublicKey) -> Result<bool, Error> {
        self.have_persons_relays2(pubkey)
    }

    /// Modify all person_relay records for a person
    pub fn modify_all_persons_relays<'a, M>(
        &'a self,
        pubkey: PublicKey,
        modify: M,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error>
    where
        M: FnMut(&mut PersonRelay),
    {
        self.modify_all_persons_relays2(pubkey, modify, rw_txn)
    }

    /// Delete PersonRelay records that match the filter
    #[inline]
    pub fn delete_person_relays<'a, F>(
        &'a self,
        filter: F,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error>
    where
        F: Fn(&PersonRelay) -> bool,
    {
        self.delete_person_relays2(filter, rw_txn)
    }

    /// Get the best relays for a person, given a direction (read/write/both).
    /// Does not handle DM usage, use get_dm_relays() for that.
    ///
    /// This returns the relays for a person, along with a score, in order of score.
    /// usage must not be RelayUsage::Both
    pub fn get_best_relays(
        &self,
        pubkey: PublicKey,
        usage: RelayUsage,
    ) -> Result<Vec<(RelayUrl, u64)>, Error> {
        let person_relays = self.get_person_relays(pubkey)?;

        // Note: the following read_rank and write_rank do not consider our own
        // rank or the success rate.
        let mut ranked_relays = match usage {
            RelayUsage::Outbox => PersonRelay::write_rank(person_relays),
            RelayUsage::Inbox => PersonRelay::read_rank(person_relays),
            RelayUsage::Both => {
                return Err(
                    ErrorKind::General("RelayUsage::Both is not allowed here".to_string()).into(),
                )
            }
        };

        // Remove banned relays
        ranked_relays = ranked_relays
            .drain(..)
            .filter(|(r, _score)| !Self::url_is_banned(r))
            .collect();

        // Modulate these scores with our local rankings
        for ranked_relay in ranked_relays.iter_mut() {
            let relay = self.read_or_create_relay(&ranked_relay.0, None)?;
            ranked_relay.1 = (ranked_relay.1 as f32
                * (relay.rank as f32 / 3.0)
                * (0.75 + 0.25 * relay.success_rate())) as u64;
        }

        // Resort
        ranked_relays.sort_by(|(_, score1), (_, score2)| score2.cmp(score1));

        let num_relays_per_person = self.read_setting_num_relays_per_person() as usize;

        // If we can't get enough of them, extend with some of our relays at score=2
        if ranked_relays.len() < (num_relays_per_person + 1) {
            let how_many_more = (num_relays_per_person + 1) - ranked_relays.len();
            let score = 2;
            match usage {
                RelayUsage::Outbox => {
                    // substitute our read relays
                    let additional: Vec<(RelayUrl, u64)> = self
                        .filter_relays(|r| {
                            // not already in their list
                            !ranked_relays.iter().any(|(url, _)| *url == r.url)
                                && r.has_usage_bits(Relay::READ)
                        })?
                        .iter()
                        .map(|r| (r.url.clone(), score))
                        .take(how_many_more)
                        .collect();

                    ranked_relays.extend(additional);
                }
                RelayUsage::Inbox => {
                    // substitute our write relays???
                    let additional: Vec<(RelayUrl, u64)> = self
                        .filter_relays(|r| {
                            // not already in their list
                            !ranked_relays.iter().any(|(url, _)| *url == r.url)
                                && r.has_usage_bits(Relay::WRITE)
                        })?
                        .iter()
                        .map(|r| (r.url.clone(), score))
                        .take(how_many_more)
                        .collect();

                    ranked_relays.extend(additional);
                }
                RelayUsage::Both => {
                    return Err(ErrorKind::General(
                        "RelayUsage::Both is not allowed here".to_string(),
                    )
                    .into());
                }
            }
        }

        Ok(ranked_relays)
    }

    /// This gets NIP-17 DM relays only.
    ///
    /// At the time of writing, not many people have these specified, in which case
    /// the caller should fallback to write relays and NIP-04.
    pub fn get_dm_relays(&self, pubkey: PublicKey) -> Result<Vec<RelayUrl>, Error> {
        let mut output: Vec<RelayUrl> = Vec::new();
        for pr in self.get_person_relays(pubkey)?.drain(..) {
            if pr.dm {
                output.push(pr.url)
            }
        }
        Ok(output)
    }

    /// This determines if a person has any NIP-17 DM relays, slightly faster
    /// than get_dm_relays() would.
    pub fn has_dm_relays(&self, pubkey: PublicKey) -> Result<bool, Error> {
        for pr in self.get_person_relays(pubkey)?.drain(..) {
            if pr.dm {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Get all the DM channels with associated data
    pub fn dm_channels(&self) -> Result<Vec<DmChannelData>, Error> {
        let my_pubkey = match GLOBALS.identity.public_key() {
            Some(pk) => pk,
            None => return Ok(Vec::new()),
        };

        let mut filter = Filter::new();
        filter.kinds = vec![EventKind::EncryptedDirectMessage, EventKind::GiftWrap];

        let events = self.find_events_by_filter(&filter, |event| {
            if event.kind == EventKind::EncryptedDirectMessage {
                event.pubkey == my_pubkey || event.is_tagged(&my_pubkey)
                // Make sure if it has tags, only author and my_pubkey
                // TBD
            } else {
                event.kind == EventKind::GiftWrap
            }
        })?;

        // Map from channel to latest-message-time and unread-count
        let mut map: HashMap<DmChannel, DmChannelData> = HashMap::new();

        for event in &events {
            let unread: usize = if event.pubkey == my_pubkey {
                // Do not count self-authored events as unread, irrespective of whether they are viewed
                0
            } else {
                1 - self.is_event_viewed(event.id)? as usize
            };
            if event.kind == EventKind::EncryptedDirectMessage {
                let time = event.created_at;
                let dmchannel = match DmChannel::from_event(event, Some(my_pubkey)) {
                    Some(dmc) => dmc,
                    None => continue,
                };
                if let Some(dmcdata) = map.get_mut(&dmchannel) {
                    if time > dmcdata.latest_message_created_at {
                        dmcdata.latest_message_created_at = time;
                        dmcdata.latest_message_content =
                            GLOBALS.identity.decrypt_event_contents(event).ok();
                    }
                    dmcdata.message_count += 1;
                    dmcdata.unread_message_count += unread;
                } else {
                    map.insert(
                        dmchannel.clone(),
                        DmChannelData {
                            dm_channel: dmchannel,
                            latest_message_created_at: time,
                            latest_message_content: GLOBALS
                                .identity
                                .decrypt_event_contents(event)
                                .ok(),
                            message_count: 1,
                            unread_message_count: unread,
                        },
                    );
                }
            } else if event.kind == EventKind::GiftWrap {
                if let Ok(rumor) = GLOBALS.identity.unwrap_giftwrap(event) {
                    let rumor_event = rumor.into_event_with_bad_signature();
                    let time = rumor_event.created_at;
                    let dmchannel = match DmChannel::from_event(&rumor_event, Some(my_pubkey)) {
                        Some(dmc) => dmc,
                        None => continue,
                    };
                    if let Some(dmcdata) = map.get_mut(&dmchannel) {
                        if time > dmcdata.latest_message_created_at {
                            dmcdata.latest_message_created_at = time;
                            dmcdata.latest_message_content = Some(rumor_event.content.clone());
                        }
                        dmcdata.message_count += 1;
                        dmcdata.unread_message_count += unread;
                    } else {
                        map.insert(
                            dmchannel.clone(),
                            DmChannelData {
                                dm_channel: dmchannel,
                                latest_message_created_at: time,
                                latest_message_content: Some(rumor_event.content.clone()),
                                message_count: 1,
                                unread_message_count: unread,
                            },
                        );
                    }
                }
            }
        }

        let mut output: Vec<DmChannelData> = map.drain().map(|e| e.1).collect();
        output.sort_by(|a, b| {
            b.latest_message_created_at
                .cmp(&a.latest_message_created_at)
                .then(b.unread_message_count.cmp(&a.unread_message_count))
        });
        Ok(output)
    }

    /// Get DM events (by id) in a channel
    pub fn dm_events(&self, channel: &DmChannel) -> Result<Vec<Id>, Error> {
        let my_pubkey = match GLOBALS.identity.public_key() {
            Some(pk) => pk,
            None => return Ok(Vec::new()),
        };

        let mut filter = Filter::new();
        filter.kinds = vec![EventKind::EncryptedDirectMessage, EventKind::GiftWrap];

        let mut output: Vec<Event> = self.find_events_by_filter(&filter, |event| {
            if let Some(event_dm_channel) = DmChannel::from_event(event, Some(my_pubkey)) {
                event_dm_channel == *channel
            } else {
                false
            }
        })?;

        // Sort by rumor's time, not giftwrap's time
        let mut sortable: Vec<(Unixtime, Event)> = output
            .drain(..)
            .map(|e| {
                if e.kind == EventKind::GiftWrap {
                    if let Ok(rumor) = GLOBALS.identity.unwrap_giftwrap(&e) {
                        (rumor.created_at, e)
                    } else {
                        (e.created_at, e)
                    }
                } else {
                    (e.created_at, e)
                }
            })
            .collect();

        sortable.sort();

        Ok(sortable.iter().map(|(_, e)| e.id).collect())
    }

    /// Rebuild all the event indices.
    pub fn rebuild_event_indices<'a>(
        &'a self,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            // Erase all indices first
            self.db_event_akci_index()?.clear(txn)?;
            self.db_event_kci_index()?.clear(txn)?;
            self.db_event_tag_index()?.clear(txn)?;
            self.db_hashtags()?.clear(txn)?;

            let loop_txn = self.env.read_txn()?;
            for result in self.db_events()?.iter(&loop_txn)? {
                let (_key, val) = result?;
                let event = Event::read_from_buffer(val)?;

                // If giftwrap:
                //   Use the id and kind of the giftwrap,
                //   Use the pubkey and created_at of the rumor
                let mut innerevent: &Event = &event;
                let rumor: Event;
                if let Some(r) = self.switch_to_rumor(&event, txn)? {
                    rumor = r;
                    innerevent = &rumor;
                }

                self.write_event_akci_index(
                    innerevent.pubkey,
                    event.kind,
                    innerevent.created_at,
                    event.id,
                    Some(txn),
                )?;
                self.write_event_kci_index(event.kind, innerevent.created_at, event.id, Some(txn))?;
                self.write_event_tag_index(
                    &event, // this handles giftwrap internally
                    Some(txn),
                )?;
                for hashtag in event.hashtags() {
                    if hashtag.is_empty() {
                        continue;
                    } // upstream bug
                    self.add_hashtag(&hashtag, event.id, Some(txn))?;
                }
            }
            self.set_flag_rebuild_indexes_needed(false, Some(txn))?;
            Ok(())
        };

        write_transact!(self, rw_txn, f)
    }

    pub fn rebuild_event_tags_index<'a>(
        &'a self,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            // Erase the index first
            self.db_event_tag_index()?.clear(txn)?;

            let loop_txn = self.env.read_txn()?;
            for result in self.db_events()?.iter(&loop_txn)? {
                let (_key, val) = result?;
                let event = Event::read_from_buffer(val)?;
                self.write_event_tag_index(&event, Some(txn))?;
            }
            Ok(())
        };

        write_transact!(self, rw_txn, f)
    }

    pub fn reprocess_relay_lists(&self) -> Result<(), Error> {
        let mut txn = self.env.write_txn()?;

        // Clear relay_list_created_at fields in person records so that
        // it will rebuild
        PersonTable::filter_modify(
            |_| true,
            |person| {
                person.relay_list_created_at = None;
            },
            Some(&mut txn),
        )?;

        // Commit this change, otherwise read_person (which takes no transaction)
        // will give stale data when it is called within process_relay_list()
        txn.commit()?;

        let mut txn = self.env.write_txn()?;

        // Load all RelayLists
        let mut filter = Filter::new();
        filter.add_event_kind(EventKind::RelayList);
        let relay_lists = self.find_events_by_filter(&filter, |_| true)?;

        // Process all RelayLists
        for event in relay_lists.iter() {
            self.process_relay_list(event, Some(&mut txn))?;
        }

        // Turn off the flag
        self.set_flag_reprocess_relay_lists_needed(false, Some(&mut txn))?;

        txn.commit()?;

        Ok(())
    }

    /// Read person lists
    pub fn read_person_lists(
        &self,
        pubkey: &PublicKey,
    ) -> Result<HashMap<PersonList, Private>, Error> {
        self.read_person_lists2(pubkey)
    }

    /// Write person lists
    pub fn write_person_lists<'a>(
        &'a self,
        pubkey: &PublicKey,
        lists: HashMap<PersonList, Private>,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        self.write_person_lists2(pubkey, lists, rw_txn)
    }

    /// Get people in a person list
    pub fn get_people_in_list(&self, list: PersonList) -> Result<Vec<(PublicKey, Private)>, Error> {
        let people = self.get_people_in_list2(list)?;

        // Update metadata.len if it is wrong
        if let Some(mut metadata) = self.get_person_list_metadata(list)? {
            if metadata.len != people.len() {
                metadata.len = people.len();
                let mut txn = self.env.write_txn()?;
                self.set_person_list_metadata(list, &metadata, Some(&mut txn))?;
                txn.commit()?;
            }
        }

        Ok(people)
    }

    /// Hash a person list
    pub fn hash_person_list(&self, list: PersonList) -> Result<u64, Error> {
        self.hash_person_list2(list)
    }

    pub fn get_people_in_all_followed_lists(&self) -> Result<Vec<PublicKey>, Error> {
        self.get_people_in_all_followed_lists2()
    }

    /// Empty a person list
    #[inline]
    pub fn clear_person_list<'a>(
        &'a self,
        list: PersonList,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            self.clear_person_list2(list, Some(txn))?;
            let now = Unixtime::now().unwrap();
            if let Some(mut metadata) = self.get_person_list_metadata(list)? {
                metadata.last_edit_time = now;
                metadata.len = 0;
                self.set_person_list_metadata(list, &metadata, Some(txn))?;
            }
            Ok(())
        };

        write_transact!(self, rw_txn, f)
    }

    /// Mark everybody in a list as private
    pub fn set_all_people_in_list_to_private<'a>(
        &'a self,
        list: PersonList,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            let people = self.get_people_in_list(list)?;
            for (pk, _) in &people {
                self.add_person_to_list(pk, list, Private(true), Some(txn))?
            }
            Ok(())
        };

        write_transact!(self, rw_txn, f)
    }

    /// Is a person in a list?
    pub fn is_person_in_list(&self, pubkey: &PublicKey, list: PersonList) -> Result<bool, Error> {
        let map = self.read_person_lists(pubkey)?;
        Ok(map.contains_key(&list))
    }

    /// Is the person in any list we subscribe to?
    pub fn is_person_subscribed_to(&self, pubkey: &PublicKey) -> Result<bool, Error> {
        let map = self.read_person_lists(pubkey)?;
        Ok(map.iter().any(|l| l.0.subscribe()))
    }

    /// Add a person to a list
    pub fn add_person_to_list<'a>(
        &'a self,
        pubkey: &PublicKey,
        list: PersonList,
        private: Private,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            let mut map = self.read_person_lists(pubkey)?;
            let had = map.contains_key(&list);
            map.insert(list, private);
            self.write_person_lists(pubkey, map, Some(txn))?;
            let now = Unixtime::now().unwrap();
            if let Some(mut metadata) = self.get_person_list_metadata(list)? {
                if !had {
                    metadata.len += 1;
                }
                metadata.last_edit_time = now;
                self.set_person_list_metadata(list, &metadata, Some(txn))?;
            }

            Ok(())
        };

        write_transact!(self, rw_txn, f)
    }

    /// Remove a person from a list
    pub fn remove_person_from_list<'a>(
        &'a self,
        pubkey: &PublicKey,
        list: PersonList,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            let mut map = self.read_person_lists(pubkey)?;
            let had = map.contains_key(&list);
            map.remove(&list);
            self.write_person_lists(pubkey, map, Some(txn))?;
            let now = Unixtime::now().unwrap();
            if let Some(mut metadata) = self.get_person_list_metadata(list)? {
                if had && metadata.len > 0 {
                    metadata.len -= 1;
                }
                metadata.last_edit_time = now;
                self.set_person_list_metadata(list, &metadata, Some(txn))?;
            }
            Ok(())
        };

        write_transact!(self, rw_txn, f)
    }

    /// Rebuild relationships
    pub fn rebuild_relationships<'a>(
        &'a self,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            // Iterate through all events
            let loop_txn = self.env.read_txn()?;
            for result in self.db_events()?.iter(&loop_txn)? {
                let (_key, val) = result?;
                let event = Event::read_from_buffer(val)?;
                crate::process::process_relationships_of_event(&event, Some(txn))?;
            }
            self.set_flag_rebuild_relationships_needed(false, Some(txn))?;
            Ok(())
        };

        write_transact!(self, rw_txn, f)
    }

    pub fn write_nip46server<'a>(
        &'a self,
        server: &Nip46Server,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        self.write_nip46server2(server, rw_txn)
    }

    pub fn read_nip46server(&self, pubkey: PublicKey) -> Result<Option<Nip46Server>, Error> {
        self.read_nip46server2(pubkey)
    }

    pub fn read_all_nip46servers(&self) -> Result<Vec<Nip46Server>, Error> {
        self.read_all_nip46servers2()
    }

    pub fn delete_nip46server<'a>(
        &'a self,
        pubkey: PublicKey,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        self.delete_nip46server2(pubkey, rw_txn)
    }

    fn url_is_banned(url: &RelayUrl) -> bool {
        url.as_str().contains("relay.nostr.band") || url.as_str().contains("filter.nostr.wine")
    }
}
