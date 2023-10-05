use crate::error::Error;
use crate::storage::Storage;
use heed::RwTxn;
use serde::{Deserialize, Serialize};
use speedy::{Readable, Writable};

// THIS IS HISTORICAL FOR MIGRATIONS AND SHOULD NOT BE EDITED

// note: if we store anything inside the variants, we can't use macro_rules.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize, Readable, Writable)]
pub enum ThemeVariant1 {
    Classic,
    Default,
    Roundy,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize, Readable, Writable)]
pub struct Theme1 {
    pub variant: ThemeVariant1,
    pub dark_mode: bool,
    pub follow_os_dark_mode: bool,
}

impl ThemeVariant1 {
    pub fn all() -> &'static [ThemeVariant1] {
        &[
            ThemeVariant1::Classic,
            ThemeVariant1::Default,
            ThemeVariant1::Roundy,
        ]
    }

    pub fn name(&self) -> &'static str {
        match *self {
            ThemeVariant1::Classic => "Classic",
            ThemeVariant1::Default => "Default",
            ThemeVariant1::Roundy => "Roundy",
        }
    }
}

impl Storage {
    #[allow(dead_code)]
    pub(crate) fn write_setting_theme1<'a>(
        &'a self,
        theme: &Theme1,
        rw_txn: Option<&mut RwTxn<'a>>,
    ) -> Result<(), Error> {
        let bytes = theme.write_to_vec()?;

        let f = |txn: &mut RwTxn<'a>| -> Result<(), Error> {
            Ok(self.general.put(txn, b"theme", &bytes)?)
        };

        match rw_txn {
            Some(txn) => {
                f(txn)?;
            }
            None => {
                let mut txn = self.env.write_txn()?;
                f(&mut txn)?;
                txn.commit()?;
            }
        };

        Ok(())
    }
}
