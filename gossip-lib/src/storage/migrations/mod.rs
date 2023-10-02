use super::types::{Person2, PersonRelay1, Settings1, Settings2, Theme1, ThemeVariant1};
use super::Storage;
use crate::error::{Error, ErrorKind};
use crate::people::PersonList;
use heed::RwTxn;
use nostr_types::{Event, Id, RelayUrl, Signature};
use speedy::{Readable, Writable};

impl Storage {
    const MAX_MIGRATION_LEVEL: u32 = 11;

    pub(super) fn migrate(&self, mut level: u32) -> Result<(), Error> {
        if level > Self::MAX_MIGRATION_LEVEL {
            return Err(ErrorKind::General(format!(
                "Migration level {} unknown: This client is older than your data.",
                level
            ))
            .into());
        }

        while level < Self::MAX_MIGRATION_LEVEL {
            // Trigger needed databases into existence before the migrate transaction starts
            self.migrate_trigger(level)?;

            let mut txn = self.env.write_txn()?;
            self.migrate_inner(level, &mut txn)?;
            level += 1;
            self.write_migration_level(level, Some(&mut txn))?;
            txn.commit()?;
        }

        Ok(())
    }

    fn migrate_trigger(&self, level: u32) -> Result<(), Error> {
        // Trigger database into existance BEFORE the migration rw_txn starts, so that they
        // are visible within that transaction
        match level {
            1 => {
                let _ = self.db_events1()?;
            }
            2 => {
                let _ = self.db_relays1()?;
            }
            4 => {
                let _ = self.db_events1()?;
            }
            5 => {
                let _ = self.db_people1()?;
                let _ = self.db_person_lists1()?;
            }
            6 => {
                let _ = self.db_people1()?;
                let _ = self.db_people2()?;
            }
            8 => {
                let _ = self.db_events1()?;
                let _ = self.db_event_ek_pk_index1()?;
                let _ = self.db_event_ek_c_index1()?;
                let _ = self.db_event_references_person1()?;
                let _ = self.db_hashtags1()?;
            }
            10 => {
                let _ = self.db_events1()?;
                let _ = self.db_event_tag_index1()?;
            }
            _ => {}
        };
        Ok(())
    }

    fn migrate_inner<'a>(&'a self, level: u32, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        let prefix = format!("LMDB Migration {} -> {}", level, level + 1);
        match level {
            0 => {
                let total = self.get_event_len()? as usize;
                tracing::info!(
                    "{prefix}: Computing and storing event relationships for {total} events..."
                );
                self.compute_relationships(total, Some(txn))?;
            }
            1 => {
                tracing::info!("{prefix}: Updating Settings...");
                self.try_migrate_settings1_settings2(Some(txn))?;
            }
            2 => {
                tracing::info!("{prefix}: Removing invalid relays...");
                self.remove_invalid_relays(txn)?;
            }
            3 => {
                tracing::info!("{prefix}: Using kv for settings...");
                self.use_kv_for_settings(txn)?;
            }
            4 => {
                tracing::info!("{prefix}: deleting decrypted rumors...");
                self.delete_rumors(txn)?;
            }
            5 => {
                tracing::info!("{prefix}: populating new lists...");
                self.populate_new_lists(txn)?;
            }
            6 => {
                tracing::info!("{prefix}: migrating person records...");
                self.migrate_people(txn)?;
            }
            7 => {
                tracing::info!("{prefix}: populating missing last_fetched data...");
                self.populate_last_fetched(txn)?;
            }
            8 => {
                tracing::info!("{prefix}: rebuilding event indices...");
                self.rebuild_event_indices(Some(txn))?;
            }
            9 => {
                tracing::info!("{prefix}: rewriting theme settings...");
                self.rewrite_theme_settings(txn)?;
            }
            10 => {
                tracing::info!("{prefix}: populating event tag index...");
                self.populate_event_tag_index(txn)?;
            }
            _ => panic!("Unreachable migration level"),
        };

        tracing::info!("done.");

        Ok(())
    }

    // Load and process every event in order to generate the relationships data
    fn compute_relationships<'a>(
        &'a self,
        total: usize,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            // track progress
            let mut count = 0;

            let event_txn = self.env.read_txn()?;
            for result in self.db_events1()?.iter(&event_txn)? {
                let pair = result?;
                let event = Event::read_from_buffer(pair.1)?;
                let _ = self.process_relationships_of_event(&event, Some(txn))?;

                // track progress
                count += 1;
                for checkpoint in &[10, 20, 30, 40, 50, 60, 70, 80, 90] {
                    if count == checkpoint * total / 100 {
                        tracing::info!("{}% done", checkpoint);
                    }
                }
            }

            tracing::info!("syncing...");

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

    fn remove_invalid_relays<'a>(&'a self, rw_txn: &mut RwTxn<'a>) -> Result<(), Error> {
        let bad_relays =
            self.filter_relays1(|relay| RelayUrl::try_from_str(relay.url.as_str()).is_err())?;

        for relay in &bad_relays {
            tracing::info!("Deleting bad relay: {}", relay.url);

            self.delete_relay1(&relay.url, Some(rw_txn))?;
        }

        tracing::info!("Deleted {} bad relays", bad_relays.len());

        Ok(())
    }

    fn try_migrate_settings1_settings2<'a>(
        &'a self,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            // If something is under the old "settings" key
            if let Ok(Some(bytes)) = self.general.get(txn, b"settings") {
                let settings1 = match Settings1::read_from_buffer(bytes) {
                    Ok(s1) => s1,
                    Err(_) => {
                        tracing::error!("Settings are not deserializing. This is probably a code issue (although I have not found the bug yet). The best I can do is reset your settings to the default. This is better than the other option of wiping your entire database and starting over.");
                        Settings1::default()
                    }
                };

                // Convert it to the new Settings2 structure
                let settings2: Settings2 = settings1.into();
                let bytes = settings2.write_to_vec()?;

                // And store it under the new "settings2" key
                self.general.put(txn, b"settings2", &bytes)?;

                // Then delete the old "settings" key
                self.general.delete(txn, b"settings")?;
            } else {
                return Err(ErrorKind::General("Settings missing.".to_string()).into());
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

    fn use_kv_for_settings<'a>(&'a self, rw_txn: &mut RwTxn<'a>) -> Result<(), Error> {
        let settings = match self.read_settings2()? {
            Some(settings) => settings,
            None => match self.read_settings2_from_wrong_key()? {
                Some(settings) => settings,
                None => {
                    if 4 >= Self::MAX_MIGRATION_LEVEL {
                        // At migraiton level < 4 we know this is safe to do:
                        let settings = crate::settings::Settings::default();
                        settings.save()?;
                        crate::globals::GLOBALS.status_queue.write().write(
                            "Settings missing or corrupted. We had to reset to defaults. Sorry about that."
                                .to_owned(),
                        );
                        return Ok(());
                    } else {
                        return Err(ErrorKind::General("Settings missing.".to_string()).into());
                    }
                }
            },
        };

        self.write_setting_public_key(&settings.public_key, Some(rw_txn))?;
        self.write_setting_log_n(&settings.log_n, Some(rw_txn))?;
        self.write_setting_offline(&settings.offline, Some(rw_txn))?;
        self.write_setting_load_avatars(&settings.load_avatars, Some(rw_txn))?;
        self.write_setting_load_media(&settings.load_media, Some(rw_txn))?;
        self.write_setting_check_nip05(&settings.check_nip05, Some(rw_txn))?;
        self.write_setting_automatically_fetch_metadata(
            &settings.automatically_fetch_metadata,
            Some(rw_txn),
        )?;
        self.write_setting_num_relays_per_person(&settings.num_relays_per_person, Some(rw_txn))?;
        self.write_setting_max_relays(&settings.max_relays, Some(rw_txn))?;
        self.write_setting_feed_chunk(&settings.feed_chunk, Some(rw_txn))?;
        self.write_setting_replies_chunk(&settings.replies_chunk, Some(rw_txn))?;
        self.write_setting_person_feed_chunk(&settings.person_feed_chunk, Some(rw_txn))?;
        self.write_setting_overlap(&settings.overlap, Some(rw_txn))?;
        self.write_setting_reposts(&settings.reposts, Some(rw_txn))?;
        self.write_setting_show_long_form(&settings.show_long_form, Some(rw_txn))?;
        self.write_setting_show_mentions(&settings.show_mentions, Some(rw_txn))?;
        self.write_setting_direct_messages(&settings.direct_messages, Some(rw_txn))?;
        self.write_setting_future_allowance_secs(&settings.future_allowance_secs, Some(rw_txn))?;
        self.write_setting_reactions(&settings.reactions, Some(rw_txn))?;
        self.write_setting_enable_zap_receipts(&settings.enable_zap_receipts, Some(rw_txn))?;
        self.write_setting_show_media(&settings.show_media, Some(rw_txn))?;
        self.write_setting_pow(&settings.pow, Some(rw_txn))?;
        self.write_setting_set_client_tag(&settings.set_client_tag, Some(rw_txn))?;
        self.write_setting_set_user_agent(&settings.set_user_agent, Some(rw_txn))?;
        self.write_setting_delegatee_tag(&settings.delegatee_tag, Some(rw_txn))?;
        self.write_setting_max_fps(&settings.max_fps, Some(rw_txn))?;
        self.write_setting_recompute_feed_periodically(
            &settings.recompute_feed_periodically,
            Some(rw_txn),
        )?;
        self.write_setting_feed_recompute_interval_ms(
            &settings.feed_recompute_interval_ms,
            Some(rw_txn),
        )?;
        self.write_setting_theme1(&settings.theme, Some(rw_txn))?;
        self.write_setting_override_dpi(&settings.override_dpi, Some(rw_txn))?;
        self.write_setting_highlight_unread_events(
            &settings.highlight_unread_events,
            Some(rw_txn),
        )?;
        self.write_setting_posting_area_at_top(&settings.posting_area_at_top, Some(rw_txn))?;
        self.write_setting_status_bar(&settings.status_bar, Some(rw_txn))?;
        self.write_setting_image_resize_algorithm(&settings.image_resize_algorithm, Some(rw_txn))?;
        self.write_setting_relay_list_becomes_stale_hours(
            &settings.relay_list_becomes_stale_hours,
            Some(rw_txn),
        )?;
        self.write_setting_metadata_becomes_stale_hours(
            &settings.metadata_becomes_stale_hours,
            Some(rw_txn),
        )?;
        self.write_setting_nip05_becomes_stale_if_valid_hours(
            &settings.nip05_becomes_stale_if_valid_hours,
            Some(rw_txn),
        )?;
        self.write_setting_nip05_becomes_stale_if_invalid_minutes(
            &settings.nip05_becomes_stale_if_invalid_minutes,
            Some(rw_txn),
        )?;
        self.write_setting_avatar_becomes_stale_hours(
            &settings.avatar_becomes_stale_hours,
            Some(rw_txn),
        )?;
        self.write_setting_media_becomes_stale_hours(
            &settings.media_becomes_stale_hours,
            Some(rw_txn),
        )?;
        self.write_setting_max_websocket_message_size_kb(
            &settings.max_websocket_message_size_kb,
            Some(rw_txn),
        )?;
        self.write_setting_max_websocket_frame_size_kb(
            &settings.max_websocket_frame_size_kb,
            Some(rw_txn),
        )?;
        self.write_setting_websocket_accept_unmasked_frames(
            &settings.websocket_accept_unmasked_frames,
            Some(rw_txn),
        )?;
        self.write_setting_websocket_connect_timeout_sec(
            &settings.websocket_connect_timeout_sec,
            Some(rw_txn),
        )?;
        self.write_setting_websocket_ping_frequency_sec(
            &settings.websocket_ping_frequency_sec,
            Some(rw_txn),
        )?;
        self.write_setting_fetcher_metadata_looptime_ms(
            &settings.fetcher_metadata_looptime_ms,
            Some(rw_txn),
        )?;
        self.write_setting_fetcher_looptime_ms(&settings.fetcher_looptime_ms, Some(rw_txn))?;
        self.write_setting_fetcher_connect_timeout_sec(
            &settings.fetcher_connect_timeout_sec,
            Some(rw_txn),
        )?;
        self.write_setting_fetcher_timeout_sec(&settings.fetcher_timeout_sec, Some(rw_txn))?;
        self.write_setting_fetcher_max_requests_per_host(
            &settings.fetcher_max_requests_per_host,
            Some(rw_txn),
        )?;
        self.write_setting_fetcher_host_exclusion_on_low_error_secs(
            &settings.fetcher_host_exclusion_on_low_error_secs,
            Some(rw_txn),
        )?;
        self.write_setting_fetcher_host_exclusion_on_med_error_secs(
            &settings.fetcher_host_exclusion_on_med_error_secs,
            Some(rw_txn),
        )?;
        self.write_setting_fetcher_host_exclusion_on_high_error_secs(
            &settings.fetcher_host_exclusion_on_high_error_secs,
            Some(rw_txn),
        )?;
        self.write_setting_nip11_lines_to_output_on_error(
            &settings.nip11_lines_to_output_on_error,
            Some(rw_txn),
        )?;
        self.write_setting_prune_period_days(&settings.prune_period_days, Some(rw_txn))?;

        self.general.delete(rw_txn, b"settings2")?;

        Ok(())
    }

    pub(crate) fn delete_rumors<'a>(&'a self, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        let mut ids: Vec<Id> = Vec::new();
        let iter = self.db_events1()?.iter(txn)?;
        for result in iter {
            let (_key, val) = result?;
            let event = Event::read_from_buffer(val)?;
            if event.sig == Signature::zeroes() {
                ids.push(event.id);
            }
        }

        for id in ids {
            self.db_events1()?.delete(txn, id.as_slice())?;
        }

        Ok(())
    }

    pub(crate) fn populate_new_lists<'a>(&'a self, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        let mut count: usize = 0;
        let mut followed_count: usize = 0;
        for person1 in self.filter_people1(|_| true)?.iter() {
            let mut lists: Vec<PersonList> = Vec::new();
            if person1.followed {
                lists.push(PersonList::Followed);
                followed_count += 1;
            }
            if person1.muted {
                lists.push(PersonList::Muted);
            }
            if !lists.is_empty() {
                self.write_person_lists1(&person1.pubkey, lists, Some(txn))?;
                count += 1;
            }
        }

        tracing::info!(
            "{} people added to new lists, {} followed",
            count,
            followed_count
        );

        // This migration does not remove the old data. The next one will.
        Ok(())
    }

    pub(crate) fn migrate_people<'a>(&'a self, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        let mut count: usize = 0;
        for person1 in self.filter_people1(|_| true)?.drain(..) {
            let person2 = Person2 {
                pubkey: person1.pubkey,
                petname: person1.petname,
                metadata: person1.metadata,
                metadata_created_at: person1.metadata_created_at,
                metadata_last_received: person1.metadata_last_received,
                nip05_valid: person1.nip05_valid,
                nip05_last_checked: person1.nip05_last_checked,
                relay_list_created_at: person1.relay_list_created_at,
                relay_list_last_received: person1.relay_list_last_received,
            };
            self.write_person2(&person2, Some(txn))?;
            count += 1;
        }

        tracing::info!("Migrated {} people", count);

        // delete people1 database
        self.db_people1()?.clear(txn)?;
        // self.general.delete(txn, b"people")?; // LMDB doesn't allow this.

        Ok(())
    }

    pub(crate) fn populate_last_fetched<'a>(&'a self, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        let total = self.get_event_seen_on_relay_len()?;
        let mut count = 0;

        // Since we failed to properly collect person_relay.last_fetched, we will
        // use seen_on data to reconstruct it
        let loop_txn = self.env.read_txn()?;

        for result in self.db_event_seen_on_relay1()?.iter(&loop_txn)? {
            let (key, val) = result?;

            // Extract out the data
            let id = Id(key[..32].try_into().unwrap());
            let url = match RelayUrl::try_from_str(std::str::from_utf8(&key[32..])?) {
                Ok(url) => url,
                Err(_) => continue, // skip if relay url is bad. We will prune these in the future.
            };

            let time = u64::from_be_bytes(val[..8].try_into()?);

            // Read event to get the person
            if let Some(event) = self.read_event(id)? {
                // Read (or create) person_relay
                let (mut pr, update) = match self.read_person_relay(event.pubkey, &url)? {
                    Some(pr) => match pr.last_fetched {
                        Some(lf) => (pr, lf < time),
                        None => (pr, true),
                    },
                    None => {
                        let pr = PersonRelay1::new(event.pubkey, url.clone());
                        (pr, true)
                    }
                };

                if update {
                    pr.last_fetched = Some(time);
                    self.write_person_relay(&pr, Some(txn))?;
                }
            }

            count += 1;
            for checkpoint in &[10, 20, 30, 40, 50, 60, 70, 80, 90] {
                if count == checkpoint * total / 100 {
                    tracing::info!("{}% done", checkpoint);
                }
            }
        }

        Ok(())
    }

    pub(crate) fn rewrite_theme_settings<'a>(&'a self, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        const DEF: Theme1 = Theme1 {
            variant: ThemeVariant1::Default,
            dark_mode: false,
            follow_os_dark_mode: true,
        };

        let theme = match self.general.get(txn, b"theme") {
            Err(_) => DEF,
            Ok(None) => DEF,
            Ok(Some(bytes)) => match Theme1::read_from_buffer(bytes) {
                Ok(val) => val,
                Err(_) => DEF,
            },
        };

        self.write_setting_theme_variant(&theme.variant.name().to_owned(), Some(txn))?;
        self.write_setting_dark_mode(&theme.dark_mode, Some(txn))?;
        self.write_setting_follow_os_dark_mode(&theme.follow_os_dark_mode, Some(txn))?;

        Ok(())
    }

    pub fn populate_event_tag_index<'a>(&'a self, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        let loop_txn = self.env.read_txn()?;
        for result in self.db_events1()?.iter(&loop_txn)? {
            let (_key, val) = result?;
            let event = Event::read_from_buffer(val)?;
            self.write_event_tag_index(&event, Some(txn))?;
        }

        Ok(())
    }
}
