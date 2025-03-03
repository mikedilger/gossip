mod page;
use page::Page;

mod gtk;

mod message;
use message::Message;

pub struct App {
    pub page: Page,
}

impl App {
    fn new() -> App {
        App {
            page: Page::LoginPage,
        }
    }
}
