use futures::task::Waker;
use nostr_types::Event;
use std::collections::VecDeque;
use std::sync::atomic::AtomicBool;
use tokio::sync::Mutex;

#[derive(Debug)]
pub struct EventStreamData {
    /// The subscription handle
    pub handle: String,

    /// These are the events streaming in
    pub events: Mutex<VecDeque<Event>>,

    /// This is a flag meaning the future ends. Either the relay returned EOSE, or
    /// there was a timeout, or there was an error condition. We don't return precisely
    /// what happened to the stream, but the stream ends now.
    pub end: AtomicBool,

    pub waker: Mutex<Option<Waker>>,
}

impl EventStreamData {
    #[allow(dead_code)]
    pub fn new(handle: String) -> EventStreamData {
        EventStreamData {
            handle,
            events: Mutex::new(VecDeque::new()),
            end: AtomicBool::new(false),
            waker: Mutex::new(None)
        }
    }
}
