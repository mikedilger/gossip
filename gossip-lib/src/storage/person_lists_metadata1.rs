use super::types::{PersonList1, PersonListMetadata1};
use crate::error::{Error, ErrorKind};
use crate::storage::{RawDatabase, Storage};
use heed::types::UnalignedSlice;
use heed::RwTxn;
use speedy::{Readable, Writable};
use std::sync::Mutex;

// PersonList1 -> PersonListMetadata1 // bool is if private or not

static PERSON_LISTS_METADATA1_DB_CREATE_LOCK: Mutex<()> = Mutex::new(());
static mut PERSON_LISTS_METADATA1_DB: Option<RawDatabase> = None;

impl Storage {
    pub(super) fn db_person_lists_metadata1(&self) -> Result<RawDatabase, Error> {
        unsafe {
            if let Some(db) = PERSON_LISTS_METADATA1_DB {
                Ok(db)
            } else {
                // Lock.  This drops when anything returns.
                let _lock = PERSON_LISTS_METADATA1_DB_CREATE_LOCK.lock();

                // In case of a race, check again
                if let Some(db) = PERSON_LISTS_METADATA1_DB {
                    return Ok(db);
                }

                // Create it. We know that nobody else is doing this and that
                // it cannot happen twice.
                let mut txn = self.env.write_txn()?;
                let db = self
                    .env
                    .database_options()
                    .types::<UnalignedSlice<u8>, UnalignedSlice<u8>>()
                    // no .flags needed
                    .name("person_lists_metadata1")
                    .create(&mut txn)?;
                txn.commit()?;
                PERSON_LISTS_METADATA1_DB = Some(db);
                Ok(db)
            }
        }
    }

    pub(crate) fn get_person_list_metadata1(
        &self,
        list: PersonList1,
    ) -> Result<Option<PersonListMetadata1>, Error> {
        let key: Vec<u8> = list.write_to_vec()?;
        let txn = self.env.read_txn()?;
        Ok(match self.db_person_lists_metadata1()?.get(&txn, &key)? {
            None => None,
            Some(bytes) => Some(PersonListMetadata1::read_from_buffer(bytes)?),
        })
    }

    pub(crate) fn set_person_list_metadata1<'a>(
        &'a self,
        list: PersonList1,
        metadata: &PersonListMetadata1,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let key: Vec<u8> = list.write_to_vec()?;

        // Do not allow overwriting dtag or title of well defined lists:
        let bytes: Vec<u8> = if list == PersonList1::Muted {
            let mut md = metadata.to_owned();
            md.dtag = "muted".to_owned();
            md.title = "Muted".to_owned();
            md.write_to_vec()?
        } else if list == PersonList1::Followed {
            let mut md = metadata.to_owned();
            md.dtag = "followed".to_owned();
            md.title = "Followed".to_owned();
            md.write_to_vec()?
        } else {
            metadata.write_to_vec()?
        };

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            self.db_person_lists_metadata1()?.put(txn, &key, &bytes)?;
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

    pub(crate) fn get_all_person_list_metadata1(
        &self,
    ) -> Result<Vec<(PersonList1, PersonListMetadata1)>, Error> {
        let txn = self.env.read_txn()?;
        let mut output: Vec<(PersonList1, PersonListMetadata1)> = Vec::new();
        for result in self.db_person_lists_metadata1()?.iter(&txn)? {
            let (key, val) = result?;
            let list = PersonList1::read_from_buffer(key)?;
            let metadata = PersonListMetadata1::read_from_buffer(val)?;
            output.push((list, metadata));
        }
        Ok(output)
    }

    pub(crate) fn find_person_list_by_dtag1(
        &self,
        dtag: &str,
    ) -> Result<Option<(PersonList1, PersonListMetadata1)>, Error> {
        let txn = self.env.read_txn()?;
        for result in self.db_person_lists_metadata1()?.iter(&txn)? {
            let (key, val) = result?;
            let list = PersonList1::read_from_buffer(key)?;
            let metadata = PersonListMetadata1::read_from_buffer(val)?;
            if metadata.dtag == dtag {
                return Ok(Some((list, metadata)));
            }
        }
        Ok(None)
    }

    pub(crate) fn allocate_person_list1<'a>(
        &'a self,
        metadata: &PersonListMetadata1,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<PersonList1, Error> {
        // Do not allocate for well-known names
        if &metadata.title == "Followed"
            || &metadata.title == "Muted"
            || &metadata.dtag == "followed"
            || &metadata.dtag == "muted"
        {
            return Err(ErrorKind::ListIsWellKnown.into());
        }

        // Check if it exists first (by dtag match)
        if let Some((found_list, _)) = self.find_person_list_by_dtag1(&metadata.dtag)? {
            return Err(ErrorKind::ListAlreadyExists(found_list).into());
        }

        let f = |txn: &mut RwTxn<'a>| -> Result<PersonList1, Error> {
            let mut slot: u8 = 0;

            for i in 2..=255 {
                let key: Vec<u8> = PersonList1::Custom(i).write_to_vec()?;
                if self.db_person_lists_metadata1()?.get(txn, &key)?.is_none() {
                    slot = i;
                    break;
                }
            }

            if slot < 2 {
                return Err(ErrorKind::ListAllocationFailed.into());
            }

            let list = PersonList1::Custom(slot);
            let key: Vec<u8> = list.write_to_vec()?;
            let val: Vec<u8> = metadata.write_to_vec()?;
            self.db_person_lists_metadata1()?.put(txn, &key, &val)?;

            Ok(list)
        };

        match rw_txn {
            Some(txn) => Ok(f(txn)?),
            None => {
                let mut txn = self.env.write_txn()?;
                let list = f(&mut txn)?;
                txn.commit()?;
                Ok(list)
            }
        }
    }

    /// Deallocate this PersonList1
    pub(crate) fn deallocate_person_list1<'a>(
        &'a self,
        list: PersonList1,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        if !self.get_people_in_list(list)?.is_empty() {
            return Err(ErrorKind::ListIsNotEmpty.into());
        }

        if u8::from(list) < 2 {
            return Err(ErrorKind::ListIsWellKnown.into());
        }

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            // note: we dont have to delete the list of people because those
            //       lists are keyed by pubkey, and we already checked that
            //       this list is not referenced.
            let key: Vec<u8> = list.write_to_vec()?;
            self.db_person_lists_metadata1()?.delete(txn, &key)?;
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
