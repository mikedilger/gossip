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
                _ => {
                    tracing::debug!("Syncer received unknown message: {}", message);
                }
            }
        }
    }
}
