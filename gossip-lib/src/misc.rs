use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::{Event, EventReference, Id, PayRequestData, PublicKey, UncheckedUrl};
use std::ops::Deref;

/// The state that a Zap is in (it moves through 5 states before it is complete)
#[derive(Debug, Clone)]
pub enum ZapState {
    None,
    CheckingLnurl(Id, PublicKey, UncheckedUrl),
    SeekingAmount(Id, PublicKey, PayRequestData, UncheckedUrl),
    LoadingInvoice(Id, PublicKey),
    ReadyToPay(Id, String), // String is the Zap Invoice as a string, to be shown as a QR code
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Freshness {
    NeverSought,
    Stale,
    Fresh,
}

/// The ancestors of a note
pub struct EventAncestors {
    /// The root of the thread, if we know it (even if we don't have it)
    pub root: Option<EventReference>,

    /// Whether we have the root of the thread in local storage
    pub root_is_local: bool,

    /// The highest connected ancestor that is in local storage (we can render from it
    /// straight to the event in question without anything missing)
    pub highest_connected_local: Option<Event>,

    /// If set, the next event up that we don't have yet (or the initial event before
    /// we have even checked)
    pub highest_connected_remote: Option<EventReference>,
}

/// Get the ancestors of an event
pub(crate) fn get_event_ancestors(main: EventReference) -> Result<EventAncestors, Error> {
    let mut ancestors = EventAncestors {
        root: None,
        root_is_local: false,
        highest_connected_local: None,
        highest_connected_remote: Some(main),
    };

    loop {
        if let Some(ref remote) = ancestors.highest_connected_remote {
            // See if the remote is local
            if let Some(event) = GLOBALS.storage.read_event_reference(remote)? {
                // It is!
                ancestors.highest_connected_local = Some(event.clone());

                // Maybe there is one higher, if so we will try to climb when we loop
                ancestors.highest_connected_remote = None;
                if let Some(parent) = event.replies_to() {
                    ancestors.highest_connected_remote = Some(parent);
                }

                // Set root data if we now have it
                if let Some(root) = event.replies_to_root() {
                    ancestors.root_is_local =
                        GLOBALS.storage.read_event_reference(&root)?.is_some();
                    ancestors.root = Some(root);
                }
            } else {
                // We have nothing more locally.
                return Ok(ancestors);
            }
        } else {
            // We have everything, nothing more is needed from remote
            return Ok(ancestors);
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Private(pub bool);

impl Deref for Private {
    type Target = bool;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// We define the readable/writable impls to be exactly as if this was just a bool.
// This way we don't have to upgrade the database from when it actually was just a bool.

impl<'a, C: speedy::Context> speedy::Readable<'a, C> for Private {
    fn read_from<R: speedy::Reader<'a, C>>(reader: &mut R) -> Result<Self, C::Error> {
        Ok(Private(bool::read_from(reader)?))
    }
}

impl<C: speedy::Context> speedy::Writable<C> for Private {
    fn write_to<T: ?Sized + speedy::Writer<C>>(&self, writer: &mut T) -> Result<(), C::Error> {
        self.0.write_to(writer)
    }
}
