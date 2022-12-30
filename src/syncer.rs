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
                "test" => {
                    tracing::debug!("Syncer received test message.");
                }
                "sync_people" => {
                    if let Err(e) = GLOBALS.people.write().await.sync().await {
                        tracing::error!("Problem syncing people: {}", e);
                    }
                }
                _ => {
                    tracing::debug!("Syncer received unknown message: {}", message);
                }
            }
        }
    }
}
