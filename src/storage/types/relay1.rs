use crate::error::{Error, ErrorKind};
use crate::storage::Storage;
use heed::RwTxn;
use nostr_types::{RelayInformationDocument, RelayUrl};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relay1 {
    pub url: RelayUrl,
    pub success_count: u64,
    pub failure_count: u64,
    pub last_connected_at: Option<u64>,
    pub last_general_eose_at: Option<u64>,
    pub rank: u64,
    pub hidden: bool,
    pub usage_bits: u64,
    pub nip11: Option<RelayInformationDocument>,
    pub last_attempt_nip11: Option<u64>,
}

impl Storage {
    #[allow(dead_code)]
    pub fn write_relay1<'a>(
        &'a self,
        relay: &Relay1,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        // Note that we use serde instead of speedy because the complexity of the
        // serde_json::Value type makes it difficult. Any other serde serialization
        // should work though: Consider bincode.
        let key = key!(relay.url.as_str().as_bytes());
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
}
