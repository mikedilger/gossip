// In order for this migration to work in the distant future after all kinds of
// other code has changed, it has to have it's own version of what Settings used
// to look like.

mod settings1;
use settings1::Settings1;

mod settings2;
use settings2::Settings2;

mod theme1;

use crate::error::Error;
use crate::storage::Storage;
use lmdb::{RwTransaction, Transaction, WriteFlags};
use speedy::{Readable, Writable};

impl Storage {
    pub(in crate::storage) fn try_migrate_settings1_settings2<'a>(
        &'a self,
        rw_txn: Option<&mut RwTransaction<'a>>,
    ) -> Result<(), Error> {
        let f = |txn: &mut RwTransaction<'a>| -> Result<(), Error> {
            // If something is under the old "settings" key
            if let Ok(bytes) = txn.get(self.general, b"settings") {
                let settings1 = Settings1::read_from_buffer(bytes)?;

                // Convert it to the new Settings2 structure
                let settings2: Settings2 = settings1.into();
                let bytes = settings2.write_to_vec()?;

                // And store it under the new "settings2" key
                txn.put(self.general, b"settings2", &bytes, WriteFlags::empty())?;

                // Then delete the old "settings" key
                txn.del(self.general, b"settings", None)?;
            }
            Ok(())
        };

        match rw_txn {
            Some(txn) => f(txn)?,
            None => {
                let mut txn = self.env.begin_rw_txn()?;
                f(&mut txn)?;
                txn.commit()?;
            }
        };

        Ok(())
    }
}
