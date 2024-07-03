use crate::error::{Error, ErrorKind};
use crate::GLOBALS;
use crate::RunState;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::time::Instant;

pub(crate) fn start_background_tasks() {
    tracing::info!("Starting general background tasks");

    tokio::task::spawn(async move {
        let mut read_runstate = GLOBALS.read_runstate.clone();
        read_runstate.mark_unchanged();
        if *read_runstate.borrow() == RunState::ShuttingDown {
            return;
        }

        let sleep = tokio::time::sleep(Duration::from_millis(1000));
        tokio::pin!(sleep);
        let mut tick: usize = 0;

        loop {
            tokio::select! {
                _ = &mut sleep => {
                    sleep.as_mut().reset(Instant::now() + Duration::from_millis(1000));
                },
                _ = read_runstate.wait_for(|runstate| *runstate == RunState::ShuttingDown) => break,
            }

            tick += 1;

            if ! GLOBALS.storage.read_setting_offline() && *read_runstate.borrow() == RunState::Online {
                if let Err(e) = do_online_tasks(tick).await {
                    tracing::error!("{}", e);
                }
            }

            if let Err(e) = do_general_tasks(tick).await {
                tracing::error!("{}", e);
            }
        }

        tracing::info!("Stopping general background tasks");
    });
}

async fn do_online_tasks(tick: usize) -> Result<(), Error> {
    // Do fetcher tasks (every 2 seconds)
    // FIXME: retire GLOBALS.storage.read_setting_fetcher_looptime_ms()
    if tick % 2 == 0 {
        GLOBALS.fetcher.process_queue().await;
    }

    // Do seeker tasks (every second)
    GLOBALS.seeker.run_once().await;

    // Update pending every 12 seconds
    if tick % 12 == 0 {
        if let Err(e) = GLOBALS.pending.compute_pending() {
            if ! matches!(e.kind, ErrorKind::NoPrivateKey) {
                tracing::error!("{:?}", e);
            }
        }
    }

    // Update people metadata every 2 seconds
    // FIXME: retire GLOBALS.storage.read_setting_fetcher_metadata_looptime_ms();
    if tick % 2 == 0 {
        GLOBALS.people.maybe_fetch_metadata().await;
    }

    Ok(())
}

async fn do_general_tasks(tick: usize) -> Result<(), Error> {

    // Update GLOBALS.unread_dms count (every 3 seconds)
    if tick % 3 == 0 {
        // Update unread dm channels, whether or not we are in that feed
        if let Ok(channels) = GLOBALS.storage.dm_channels() {
            let unread = channels.iter().map(|c| c.unread_message_count).sum();
            GLOBALS.unread_dms.store(unread, Ordering::Relaxed);
        }
    }

    Ok(())
}
