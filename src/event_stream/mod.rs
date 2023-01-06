
mod event_stream_data;

use event_stream_data::EventStreamData;
use futures::stream::Stream;
use futures::task::Context;
use nostr_types::{Event, Url};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::task::Poll;

pub struct EventStream {
    pub url: Url,
    pub sub_handle: String,
    pub data: Arc<EventStreamData>
}

impl EventStream {
    // WARNING: just creating one of these does not setup any fulfillment.
    // you need to take a copy of the 'data' part and get a minion to fulfill
    // it for you. Generally this should only be called by
    // Overlord.query_event_stream() which does that.
    #[allow(dead_code)]
    pub fn new(url: Url, sub_handle: String) -> EventStream {
        EventStream {
            url,
            sub_handle,
            data: Arc::new(EventStreamData::new())
        }
    }
}

impl Stream for EventStream {
    type Item = Event;

    fn poll_next(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Make sure we have a waker, so we can be polled again in the future
        match self.data.waker.try_lock() {
            Err(_) => {
                // This is locked. It must already have a waker because we put a waker in right away.
                return Poll::Pending;
            },
            Ok(mut guard) => {
                *guard = Some(ctx.waker().to_owned());
            }
        }

        // Check if the stream has ended
        if self.data.end.load(Ordering::Relaxed) {
            return Poll::Ready(None);
        }

        // Check if we have an event
        match self.data.events.try_lock() {
            Err(_) => {
                // The minion is writing data for us. Exciting! Let's return pending. The
                // minion should wake us when it has unlocked the events VecDeque.
                return Poll::Pending;
            },
            Ok(mut events) => {
                if events.is_empty() {
                    return Poll::Pending;
                } else {
                    return Poll::Ready(events.pop_front());
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

