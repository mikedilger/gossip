use crate::error::Error;
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

            if *read_runstate.borrow() == RunState::Online {
                if let Err(e) = do_online_tasks(tick) {
                    tracing::error!("{}", e);
                }
            }

            if let Err(e) = do_general_tasks(tick) {
                tracing::error!("{}", e);
            }
        }

        tracing::info!("Stopping general background tasks");
    });
}

fn do_online_tasks(tick: usize) -> Result<(), Error> {
    Ok(())
}

fn do_general_tasks(tick: usize) -> Result<(), Error> {
    Ok(())
}
