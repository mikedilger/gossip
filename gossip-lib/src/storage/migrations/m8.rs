use crate::error::Error;
use crate::storage::types::PersonRelay1;
use crate::storage::Storage;
use heed::RwTxn;
use nostr_types::{Id, RelayUrl};

impl Storage {
    pub(super) fn m8_trigger(&self) -> Result<(), Error> {
        let _ = self.db_relays1()?;
        Ok(())
    }

    pub(super) fn m8_migrate<'a>(&'a self, prefix: &str, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: populating missing last_fetched data...");

        // Migrate
        self.m8_populate_last_fetched(txn)?;

        Ok(())
    }

    fn m8_populate_last_fetched<'a>(&'a self, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        let total = self.get_event_seen_on_relay1_len()?;
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
                let (mut pr, update) = match self.read_person_relay1(event.pubkey, &url)? {
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
                    self.write_person_relay1(&pr, Some(txn))?;
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
}
