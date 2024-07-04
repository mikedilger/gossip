use crate::error::ErrorKind;
use crate::RunState;
use crate::GLOBALS;
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

        let sleep_future = tokio::time::sleep(Duration::from_millis(1000));
        tokio::pin!(sleep_future);
        let mut tick: usize = 0;

        let recompute_bookmarks = GLOBALS.recompute_current_bookmarks.clone();

        loop {
            let recompute_bookmarks_future = recompute_bookmarks.notified();

            tokio::select! {
                _ = &mut sleep_future => {
                    sleep_future.as_mut().reset(Instant::now() + Duration::from_millis(1000))
                },
                _ = read_runstate.wait_for(|runstate| *runstate == RunState::ShuttingDown) => break,
                _ = recompute_bookmarks_future => {
                    match GLOBALS.bookmarks.read().get_bookmark_feed() {
                        Ok(feed) => *GLOBALS.current_bookmarks.write() = feed,
                        Err(e) => tracing::error!("{:?}", e),
                    }
                }
            }

            tick += 1;

            if !GLOBALS.storage.read_setting_offline()
                && *read_runstate.borrow() == RunState::Online
            {
                do_online_tasks(tick).await;
            }

            do_general_tasks(tick).await;
        }

        tracing::info!("Stopping general background tasks");
    });
}

async fn do_online_tasks(tick: usize) {
    // Do fetcher tasks (every 2 seconds)
    if tick % 2 == 0 {
        GLOBALS.fetcher.process_queue().await;
    }

    // Do seeker tasks (every second)
    GLOBALS.seeker.run_once().await;

    // Update pending every 12 seconds
    if tick % 12 == 0 {
        if let Err(e) = GLOBALS.pending.compute_pending() {
            if !matches!(e.kind, ErrorKind::NoPrivateKey) {
                tracing::error!("{:?}", e);
            }
        }
    }

    // Update people metadata every 2 seconds
    if tick % 2 == 0 {
        GLOBALS.people.maybe_fetch_metadata().await;
    }
}

async fn do_general_tasks(tick: usize) {
    // Update GLOBALS.unread_dms count (every 3 seconds)
    if tick % 3 == 0 {
        // Update unread dm channels, whether or not we are in that feed
        if let Ok(channels) = GLOBALS.storage.dm_channels() {
            let unread = channels.iter().map(|c| c.unread_message_count).sum();
            GLOBALS.unread_dms.store(unread, Ordering::Relaxed);
        }
    }
}
