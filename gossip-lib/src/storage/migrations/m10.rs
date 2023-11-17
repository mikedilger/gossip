use crate::storage::Storage;
use crate::storage::types::{Theme1, ThemeVariant1};
use crate::error::Error;
use heed::RwTxn;
use speedy::Readable;

impl Storage {
    pub(super) fn m10_trigger(&self) -> Result<(), Error> {
        let _ = self.db_relays1()?;
        Ok(())
    }

    pub(super) fn m10_migrate<'a>(&'a self, prefix: &str, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: rewriting theme settings...");

        // Migrate
        self.m10_rewrite_theme_settings(txn)?;

        Ok(())
    }

    fn m10_rewrite_theme_settings<'a>(&'a self, txn: &mut RwTxn<'a>) -> Result<(), Error> {
        const DEF: Theme1 = Theme1 {
            variant: ThemeVariant1::Default,
            dark_mode: false,
            follow_os_dark_mode: true,
        };

        let theme = match self.general.get(txn, b"theme") {
            Err(_) => DEF,
            Ok(None) => DEF,
            Ok(Some(bytes)) => match Theme1::read_from_buffer(bytes) {
                Ok(val) => val,
                Err(_) => DEF,
            },
        };

        self.write_setting_theme_variant(&theme.variant.name().to_owned(), Some(txn))?;
        self.write_setting_dark_mode(&theme.dark_mode, Some(txn))?;
        self.write_setting_follow_os_dark_mode(&theme.follow_os_dark_mode, Some(txn))?;

        Ok(())
    }

}
