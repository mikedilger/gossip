use crate::globals::GLOBALS;
use tokio::sync::mpsc;

pub struct Syncer {
    incoming: mpsc::UnboundedReceiver<String>,
}

impl Syncer {
    pub fn new(incoming: mpsc::UnboundedReceiver<String>) -> Syncer {
        Syncer { incoming }
    }

    pub async fn run(&mut self) {
        loop {
            let message = self.incoming.recv().await;

            if message.is_none() {
                return;
            }

            let message = message.unwrap();

            match &*message {
                "sync_fetcher" => {
                    if let Err(e) = GLOBALS.fetcher.sync().await {
                        tracing::error!("Problem fetching from web: {}", e);
                    }
                }
                _ => {
                    tracing::debug!("Syncer received unknown message: {}", message);
                }
            }
        }
    }
}
