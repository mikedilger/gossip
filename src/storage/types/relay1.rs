use crate::error::{Error, ErrorKind};
use crate::globals::GLOBALS;
use crate::storage::Storage;
use gossip_relay_picker::Direction;
use heed::RwTxn;
use nostr_types::{Id, RelayInformationDocument, RelayUrl, Unixtime};
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

impl Relay1 {
    pub const READ: u64 = 1 << 0; // 1
    pub const WRITE: u64 = 1 << 1; // 2
    pub const ADVERTISE: u64 = 1 << 2; // 4
    pub const INBOX: u64 = 1 << 3; // 8            this is 'read' of kind 10002
    pub const OUTBOX: u64 = 1 << 4; // 16          this is 'write' of kind 10002
    pub const DISCOVER: u64 = 1 << 5; // 32

    pub fn new(url: RelayUrl) -> Relay1 {
        Relay1 {
            url,
            success_count: 0,
            failure_count: 0,
            last_connected_at: None,
            last_general_eose_at: None,
            rank: 3,
            hidden: false,
            usage_bits: 0,
            nip11: None,
            last_attempt_nip11: None,
        }
    }

    #[inline]
    pub fn set_usage_bits(&mut self, bits: u64) {
        self.usage_bits |= bits;
    }

    #[inline]
    pub fn clear_usage_bits(&mut self, bits: u64) {
        self.usage_bits &= !bits;
    }

    #[inline]
    pub fn adjust_usage_bit(&mut self, bit: u64, value: bool) {
        if value {
            self.set_usage_bits(bit);
        } else {
            self.clear_usage_bits(bit);
        }
    }

    #[inline]
    pub fn has_usage_bits(&self, bits: u64) -> bool {
        self.usage_bits & bits == bits
    }

    #[inline]
    pub fn attempts(&self) -> u64 {
        self.success_count + self.failure_count
    }

    #[inline]
    pub fn success_rate(&self) -> f32 {
        let attempts = self.attempts();
        if attempts == 0 {
            return 0.5;
        } // unknown, so we put it in the middle
        self.success_count as f32 / attempts as f32
    }

    /// This generates a "recommended_relay_url" for an 'e' tag.
    pub async fn recommended_relay_for_reply(reply_to: Id) -> Result<Option<RelayUrl>, Error> {
        let seen_on_relays: Vec<(RelayUrl, Unixtime)> =
            GLOBALS.storage.get_event_seen_on_relay(reply_to)?;

        let maybepubkey = GLOBALS.storage.read_setting_public_key();
        if let Some(pubkey) = maybepubkey {
            let my_inbox_relays: Vec<(RelayUrl, u64)> =
                GLOBALS.storage.get_best_relays(pubkey, Direction::Read)?;

            // Find the first-best intersection
            for mir in &my_inbox_relays {
                for sor in &seen_on_relays {
                    if mir.0 == sor.0 {
                        return Ok(Some(mir.0.clone()));
                    }
                }
            }

            // Else use my first inbox
            if let Some(mir) = my_inbox_relays.first() {
                return Ok(Some(mir.0.clone()));
            }

            // Else fall through to seen on relays only
        }

        if let Some(sor) = seen_on_relays.first() {
            return Ok(Some(sor.0.clone()));
        }

        Ok(None)
    }
}

impl Storage {
    pub fn get_relays1_len(&self) -> Result<u64, Error> {
        let txn = self.env.read_txn()?;
        Ok(self.relays.len(&txn)?)
    }

    #[allow(dead_code)]
    pub fn write_relay1<'a>(
        &'a self,
        relay: &Relay1,
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

    pub fn delete_relay1<'a>(
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
            // Delete any PersonRelay with this url
            self.delete_person_relays(|f| f.url == *url, Some(txn))?;

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

    pub fn modify_relay1<'a, M>(
        &'a self,
        url: &RelayUrl,
        mut modify: M,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error>
    where
        M: FnMut(&mut Relay1),
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

    pub fn modify_all_relays1<'a, M>(
        &'a self,
        mut modify: M,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error>
    where
        M: FnMut(&mut Relay1),
    {
        let mut f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            let mut iter = self.relays.iter_mut(txn)?;
            while let Some(result) = iter.next() {
                let (key, val) = result?;
                let mut dbrelay: Relay1 = serde_json::from_slice(val)?;
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

    pub fn read_relay1(&self, url: &RelayUrl) -> Result<Option<Relay1>, Error> {
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

    pub fn filter_relays1<F>(&self, f: F) -> Result<Vec<Relay1>, Error>
    where
        F: Fn(&Relay1) -> bool,
    {
        let txn = self.env.read_txn()?;
        let mut output: Vec<Relay1> = Vec::new();
        let iter = self.relays.iter(&txn)?;
        for result in iter {
            let (_key, val) = result?;
            let relay: Relay1 = serde_json::from_slice(val)?;
            if f(&relay) {
                output.push(relay);
            }
        }
        Ok(output)
    }
}
