// In order for this migration to work in the distant future after all kinds of
// other code has changed, it has to have it's own version of what Settings used
// to look like.

mod settings1;
pub use settings1::Settings1;

mod settings2;
pub use settings2::Settings2;

mod theme1;
pub use theme1::{Theme1, ThemeVariant1};


use crate::error::Error;
use crate::storage::Storage;
use heed::RwTxn;
use speedy::{Readable, Writable};

impl Storage {
    pub(in crate::storage) fn try_migrate_settings1_settings2<'a>(
        &'a self,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            // If something is under the old "settings" key
            if let Ok(Some(bytes)) = self.general.get(txn, b"settings") {
                let settings1 = Settings1::read_from_buffer(bytes)?;

                // Convert it to the new Settings2 structure
                let settings2: Settings2 = settings1.into();
                let bytes = settings2.write_to_vec()?;

                // And store it under the new "settings2" key
                self.general.put(txn, b"settings2", &bytes)?;

                // Then delete the old "settings" key
                self.general.delete(txn, b"settings")?;
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
}
