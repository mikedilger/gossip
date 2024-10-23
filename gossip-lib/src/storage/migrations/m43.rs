use super::Storage;
use crate::error::Error;
use crate::storage::types::{Handler, HandlerKey};
use crate::storage::{HandlersTable, Table};
use heed::RwTxn;
use nostr_types::{EventKind, Filter};

impl Storage {
    pub(super) fn m43_trigger(&self) -> Result<(), Error> {
        let _ = self.db_configured_handlers()?;
        let _ = HandlersTable::db()?;
        Ok(())
    }

    pub(super) fn m43_migrate<'a>(
        &'a self,
        prefix: &str,
        txn: &mut RwTxn<'a>,
    ) -> Result<(), Error> {
        // Info message
        tracing::info!("{prefix}: reimporting event handlers...");

        // Load all configured handlers into memory
        let configured_handlers: Vec<(EventKind, HandlerKey, bool, bool)> =
            self.read_all_configured_handlers()?;

        // Delete all handlers
        HandlersTable::clear(Some(txn))?;

        // Delete all configured handlers
        self.clear_configured_handlers(Some(txn))?;

        // Load all 31990 events
        // This is a scrape (no index for this)
        let mut filter = Filter::new();
        filter.add_event_kind(EventKind::HandlerInformation);
        let events = self.find_events_by_filter(&filter, |_| true)?;

        // Convert them into Handlers and save them
        for event in events.iter() {
            if let Some(mut handler) = Handler::from_31990(event) {
                HandlersTable::write_record(&mut handler, Some(txn))?;

                // And also save their per-kind configuration data
                for kind in handler.kinds {
                    let mut enabled: bool = true;
                    let mut recommended: bool = false;

                    // If we already had it configured, use that data
                    if let Some((_, _, was_enabled, was_recommended)) = configured_handlers
                        .iter()
                        .find(|c| c.0 == kind && c.1 == handler.key)
                    {
                        enabled = *was_enabled;
                        recommended = *was_recommended;
                    }

                    // Write configured handler, enabled by default
                    self.write_configured_handler(
                        kind,
                        handler.key.clone(),
                        enabled,
                        recommended,
                        Some(txn),
                    )?;
                }
            }
        }

        Ok(())
    }
}
