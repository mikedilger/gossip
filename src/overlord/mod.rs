use crate::comms::BusMessage;
use crate::settings::Settings;
use crate::{Error, GLOBALS};
use tokio::select;
use tokio::sync::broadcast::Sender;
use tokio::sync::mpsc::UnboundedReceiver;

pub struct Overlord {
    settings: Settings,
    to_minions: Sender<BusMessage>,
    #[allow(dead_code)]
    from_minions: UnboundedReceiver<BusMessage>,
}

impl Overlord {
    pub fn new(from_minions: UnboundedReceiver<BusMessage>) -> Overlord {
        let to_minions = GLOBALS.to_minions.clone();
        Overlord {
            settings: Settings::default(),
            to_minions,
            from_minions,
        }
    }

    pub async fn run(&mut self) {
        if let Err(e) = self.run_inner().await {
            log::error!("{}", e);
        }

        // Send shutdown message to all minions (and ui)
        // If this fails, it's probably because there are no more listeners
        // so just ignore it and keep shutting down.
        let _ = self.to_minions.send(BusMessage {
            target: "all".to_string(),
            kind: "shutdown".to_string(),
            json_payload: serde_json::to_string("shutdown").unwrap(),
        });

        // Wait on all minions to finish. When there are no more senders
        // sending to `from_minions` then they are all completed.
        // In that case this call will return an error, but we don't care we
        // just finish.
        let _ = self.from_minions.recv();
    }

    pub async fn run_inner(&mut self) -> Result<(), Error> {
        // Setup the database (possibly create, possibly upgrade)
        crate::db::setup_database().await?;

        // Load settings
        self.settings = Settings::load().await?;

        'mainloop: loop {
            match self.loop_handler().await {
                Ok(keepgoing) => {
                    if !keepgoing {
                        break 'mainloop;
                    }
                }
                Err(e) => {
                    // Log them and keep looping
                    log::error!("{}", e);
                }
            }
        }

        Ok(())
    }

    #[allow(unused_assignments)]
    async fn loop_handler(&mut self) -> Result<bool, Error> {
        let mut keepgoing: bool = true;

        select! {
            bus_message = self.from_minions.recv() => {
                let bus_message = match bus_message {
                    Some(bm) => bm,
                    None => {
                        // All senders dropped, or one of them closed.
                        return Ok(false);
                    }
                };
                keepgoing = self.handle_bus_message(bus_message).await?;
            },
        }

        Ok(keepgoing)
    }

    async fn handle_bus_message(&mut self, bus_message: BusMessage) -> Result<bool, Error> {
        #[allow(clippy::single_match)] // because temporarily so
        match &*bus_message.target {
            "all" => match &*bus_message.kind {
                "shutdown" => {
                    log::info!("Overlord shutting down");
                    return Ok(false);
                }
                "settings_changed" => {
                    self.settings = serde_json::from_str(&bus_message.json_payload)?;
                    // We need to inform the minions
                    self.to_minions.send(BusMessage {
                        target: "all".to_string(),
                        kind: "settings_changed".to_string(),
                        json_payload: bus_message.json_payload.clone(),
                    })?;
                }
                _ => {}
            },
            //"overlord" => match &*bus_message.kind {
            //}
            _ => {}
        }

        Ok(true)
    }
}
