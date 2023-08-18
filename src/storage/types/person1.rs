use crate::error::Error;
use crate::storage::Storage;
use heed::RwTxn;
use nostr_types::{Metadata, PublicKey};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Person1 {
    pub pubkey: PublicKey,
    pub petname: Option<String>,
    pub followed: bool,
    pub followed_last_updated: i64,
    pub muted: bool,
    pub metadata: Option<Metadata>,
    pub metadata_created_at: Option<i64>,
    pub metadata_last_received: i64,
    pub nip05_valid: bool,
    pub nip05_last_checked: Option<u64>,
    pub relay_list_created_at: Option<i64>,
    pub relay_list_last_received: i64,
}

impl Storage {
    #[allow(dead_code)]
    pub fn write_person1<'a>(
        &'a self,
        person: &Person1,
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
}
