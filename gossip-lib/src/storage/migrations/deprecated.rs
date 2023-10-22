use super::Storage;
use crate::error::Error;
use heed::RwTxn;
use nostr_types::Unixtime;

impl Storage {
    /// Write the user's last ContactList edit time
    /// DEPRECATED - use set_person_list_last_edit_time instead
    pub(in crate::storage) fn write_last_contact_list_edit<'a>(
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

    /// Read the user's last ContactList edit time
    /// DEPRECATED - use get_person_list_last_edit_time instead
    pub(in crate::storage) fn read_last_contact_list_edit(&self) -> Result<i64, Error> {
        let txn = self.env.read_txn()?;

        match self.general.get(&txn, b"last_contact_list_edit")? {
            None => {
                let now = Unixtime::now().unwrap();
                self.write_last_contact_list_edit(now.0, None)?;
                Ok(now.0)
            }
            Some(bytes) => Ok(i64::from_be_bytes(bytes[..8].try_into().unwrap())),
        }
    }

    /// Write the user's last MuteList edit time
    /// DEPRECATED - use set_person_list_last_edit_time instead
    pub(in crate::storage) fn write_last_mute_list_edit<'a>(
        &'a self,
        when: i64,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let bytes = when.to_be_bytes();

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            self.general
                .put(txn, b"last_mute_list_edit", bytes.as_slice())?;
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

    /// Read the user's last MuteList edit time
    /// DEPRECATED - use get_person_list_last_edit_time instead
    pub(in crate::storage) fn read_last_mute_list_edit(&self) -> Result<i64, Error> {
        let txn = self.env.read_txn()?;

        match self.general.get(&txn, b"last_mute_list_edit")? {
            None => {
                let now = Unixtime::now().unwrap();
                self.write_last_mute_list_edit(now.0, None)?;
                Ok(now.0)
            }
            Some(bytes) => Ok(i64::from_be_bytes(bytes[..8].try_into().unwrap())),
        }
    }
}
