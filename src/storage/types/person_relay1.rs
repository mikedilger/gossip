use crate::error::Error;
use crate::storage::{MAX_LMDB_KEY, Storage};
use heed::RwTxn;
use nostr_types::{PublicKey, RelayUrl};
use speedy::{Readable, Writable};

#[derive(Debug, Readable, Writable)]
pub struct PersonRelay1 {
    // The person
    pub pubkey: PublicKey,

    // The relay associated with that person
    pub url: RelayUrl,

    // The last time we fetched one of the person's events from this relay
    pub last_fetched: Option<u64>,

    // When we follow someone at a relay
    pub last_suggested_kind3: Option<u64>,

    // When we get their nip05 and it specifies this relay
    pub last_suggested_nip05: Option<u64>,

    // Updated when a 'p' tag on any event associates this person and relay via the
    // recommended_relay_url field
    pub last_suggested_bytag: Option<u64>,

    pub read: bool,

    pub write: bool,

    // When we follow someone at a relay, this is set true
    pub manually_paired_read: bool,

    // When we follow someone at a relay, this is set true
    pub manually_paired_write: bool,
}

impl Storage {
    #[allow(dead_code)]
    pub fn write_person_relay1<'a>(
        &'a self,
        person_relay: &PersonRelay1,
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
}
