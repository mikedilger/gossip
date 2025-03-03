mod page;
use page::Page;

mod gtk;

mod message;
use message::Message;

use gossip_lib::GLOBALS;
use std::sync::atomic::Ordering;

pub struct App {
    pub forced_page: Option<Page>,
    pub page: Page,
}

impl App {
    fn new() -> App {
        App {
            forced_page: None,
            page: Page::Tbd,
        }
    }

    pub fn maybe_force_page(&mut self) {
        if GLOBALS.wait_for_login.load(Ordering::Relaxed) {
            self.forced_page = Some(Page::LoginPage);
        }
        if GLOBALS.wait_for_data_migration.load(Ordering::Relaxed) {
            self.forced_page = Some(Page::WaitForMigration);
        }
        let optstatus = GLOBALS.prune_status.read();
        if let Some(status) = optstatus.as_ref() {
            self.forced_page = Some(Page::WaitForPruning(status.clone()));
        }
    }
}
