use super::Storage;
use crate::error::Error;
use nostr_types::Unixtime;

impl Storage {
    /// Read the user's last ContactList edit time
    /// DEPRECATED - use get_person_list_last_edit_time instead
    pub(in crate::storage) fn read_last_contact_list_edit(&self) -> Result<i64, Error> {
        let txn = self.env.read_txn()?;

        match self.general.get(&txn, b"last_contact_list_edit")? {
            None => {
                let now = Unixtime::now().unwrap();
                Ok(now.0)
            }
            Some(bytes) => Ok(i64::from_be_bytes(bytes[..8].try_into().unwrap())),
        }
    }

    /// Read the user's last MuteList edit time
    /// DEPRECATED - use get_person_list_last_edit_time instead
    pub(in crate::storage) fn read_last_mute_list_edit(&self) -> Result<i64, Error> {
        let txn = self.env.read_txn()?;

        match self.general.get(&txn, b"last_mute_list_edit")? {
            None => {
                let now = Unixtime::now().unwrap();
                Ok(now.0)
            }
            Some(bytes) => Ok(i64::from_be_bytes(bytes[..8].try_into().unwrap())),
        }
    }
}
