use crate::error::ErrorKind;
use crate::RunState;
use crate::GLOBALS;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::time::Instant;

const TICK: u64 = 500;

pub(crate) fn start_background_tasks() {
    tracing::info!("Starting general background tasks");

    tokio::task::spawn(async move {
        let mut read_runstate = GLOBALS.read_runstate.clone();
        read_runstate.mark_unchanged();
        if *read_runstate.borrow() == RunState::ShuttingDown {
            return;
        }

        let sleep_future = tokio::time::sleep(Duration::from_millis(TICK));
        tokio::pin!(sleep_future);
        let mut tick: usize = 0;

        let recompute_bookmarks = GLOBALS.recompute_current_bookmarks.clone();

        loop {
            let recompute_bookmarks_future = recompute_bookmarks.notified();

            tokio::select! {
                _ = &mut sleep_future => {
                    sleep_future.as_mut().reset(Instant::now() + Duration::from_millis(TICK))
                },
                _ = read_runstate.wait_for(|runstate| *runstate == RunState::ShuttingDown) => break,
                _ = recompute_bookmarks_future => {
                    match GLOBALS.bookmarks.read_arc().get_bookmark_feed() {
                        Ok(feed) => *GLOBALS.current_bookmarks.write() = feed,
                        Err(e) => tracing::error!("{:?}", e),
                    }
                }
            }

            tick += 1;

            if !GLOBALS.db().read_setting_offline() && *read_runstate.borrow() == RunState::Online {
                do_online_tasks(tick).await;
            }

            do_general_tasks(tick).await;

            do_debug_tasks(tick).await;

            GLOBALS.feed.sync_maybe_periodic_recompute();
        }

        tracing::info!("Stopping general background tasks");
    });
}

async fn do_online_tasks(tick: usize) {
    // Do seeker tasks 2 ticks
    if tick % 2 == 0 {
        GLOBALS.seeker.run_once().await;
    }

    // Update pending every 5 ticks
    if tick % 5 == 0 {
        if let Err(e) = GLOBALS.pending.compute_pending() {
            if !matches!(e.kind, ErrorKind::NoPrivateKey) {
                tracing::error!("{:?}", e);
            }
        }
    }

    // Update people metadata every 3 ticks
    if tick % 3 == 0 {
        GLOBALS.people.maybe_fetch_metadata().await;
    }
}

async fn do_general_tasks(tick: usize) {
    // Update GLOBALS.unread_dms count every 2 ticks
    if tick % 2 == 0 {
        // Update unread dm channels, whether or not we are in that feed
        if let Ok(channels) = GLOBALS.db().dm_channels() {
            let unread = channels.iter().map(|c| c.unread_message_count).sum();
            GLOBALS.unread_dms.store(unread, Ordering::Relaxed);
        }

        update_inbox_indicator().await;
    }

    // Update handlers for quick menu rendering
    let _ = GLOBALS.update_handlers();
}

async fn update_inbox_indicator() {
    let ids = GLOBALS.feed.get_inbox_events();
    let mut count: usize = 0;
    for id in ids {
        if matches!(GLOBALS.db().is_event_viewed(id), Ok(false)) {
            count += 1;
        }
    }
    GLOBALS.unread_inbox.store(count, Ordering::Relaxed);
}

async fn do_debug_tasks(tick: usize) {
    if tick % 20 == 0 {
        tracing::debug!(target: "fetcher", "DEBUG FETCHER STATS: {}", GLOBALS.fetcher.stats());
    }
}
