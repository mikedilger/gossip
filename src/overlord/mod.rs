use crate::comms::BusMessage;
use crate::{Error, GLOBALS};
use tokio::sync::broadcast::Sender;
use tokio::sync::mpsc::UnboundedReceiver;

pub struct Overlord {
    to_minions: Sender<BusMessage>,
    #[allow(dead_code)]
    from_minions: UnboundedReceiver<BusMessage>,
}

impl Overlord {
    pub fn new(from_minions: UnboundedReceiver<BusMessage>) -> Overlord {
        let to_minions = GLOBALS.to_minions.clone();
        Overlord {
            to_minions,
            from_minions,
        }
    }

    pub async fn run(&mut self) {
        if let Err(e) = self.run_inner().await {
            log::error!("{}", e);
            if let Err(e) = self.to_minions.send(BusMessage {
                target: "all".to_string(),
                kind: "shutdown".to_string(),
                json_payload: serde_json::to_string("shutdown").unwrap(),
            }) {
                log::error!("Unable to send shutdown: {}", e);
            }
        }

        // FIXME wait for minions to finish here
    }

    pub async fn run_inner(&mut self) -> Result<(), Error> {
        // Setup the database (possibly create, possibly upgrade)
        crate::db::setup_database().await?;

        // FIXME, do something real here
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }
}
