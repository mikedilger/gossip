use crate::error::Error;
use crate::storage::types::{PersonList1, PersonListMetadata1};
use crate::storage::Storage;
use heed::RwTxn;
use nostr_types::Unixtime;
use speedy::Readable;
use std::collections::{BTreeMap, HashMap};

impl Storage {
    pub(super) fn m19_trigger(&self) -> Result<(), Error> {
        let _ = self.db_person_lists_metadata1()?;
        Ok(())
    }

    pub(super) fn m19_migrate<'a>(
        &'a self,
        prefix: &str,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: populating person list metadata...");

        // Migrate
        self.m19_populate_person_list_metadata(txn)?;

        Ok(())
    }

    fn m19_populate_person_list_metadata<'a>(&'a self, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        // read custom_person_list_map setting
        let name_map: BTreeMap<u8, String> = {
            let maybe_map = match self.db_general()?.get(txn, b"custom_person_list_map") {
                Err(_) => None,
                Ok(None) => None,
                Ok(Some(bytes)) => match <BTreeMap<u8, String>>::read_from_buffer(bytes) {
                    Ok(val) => Some(val),
                    Err(_) => None,
                },
            };
            maybe_map.unwrap_or_else(|| {
                let mut m = BTreeMap::new();
                m.insert(0, "Muted".to_owned());
                m.insert(1, "Followed".to_owned());
                m
            })
        };

        let last_edit_times: HashMap<PersonList1, i64> =
            self.m19_read_person_lists_last_edit_times()?;

        let mut lists: Vec<PersonList1> =
            name_map.keys().map(|k| PersonList1::from_u8(*k)).collect();

        for list in lists.drain(..) {
            let mut metadata = PersonListMetadata1 {
                last_edit_time: Unixtime(
                    last_edit_times
                        .get(&list)
                        .copied()
                        .unwrap_or(Unixtime::now().0),
                ),
                ..Default::default()
            };
            if list == PersonList1::Muted {
                metadata.dtag = "muted".to_string();
                metadata.title = "Muted".to_string();
            } else if list == PersonList1::Followed {
                metadata.dtag = "followed".to_string();
                metadata.title = "Followed".to_string();
            } else {
                let name = name_map
                    .get(&u8::from(list))
                    .map(|s| s.as_str())
                    .unwrap_or("Unnamed");
                metadata.dtag = name.to_string();
                metadata.title = name.to_string();
            }

            self.set_person_list_metadata1(list, &metadata, Some(txn))?;
        }

        // Now remove the two maps
        self.db_general()?.delete(txn, b"custom_person_list_map")?;
        self.db_general()?
            .delete(txn, b"person_lists_last_edit_times")?;

        Ok(())
    }

    /// Read the user's old last ContactList edit time
    pub fn m19_read_person_lists_last_edit_times(
        &self,
    ) -> Result<HashMap<PersonList1, i64>, Error> {
        let txn = self.env.read_txn()?;

        match self
            .db_general()?
            .get(&txn, b"person_lists_last_edit_times")?
        {
            None => Ok(HashMap::new()),
            Some(bytes) => Ok(HashMap::<PersonList1, i64>::read_from_buffer(bytes)?),
        }
    }
}
