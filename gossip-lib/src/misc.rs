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

/// Get the highest local and remote ancestors in a thread
/// This never returns (None, None) but the other three cases get returned:
///
///    (Some(event), None) --> we have the top event, nothing to seek
///    (None, Some(eventref)) --> we have no event, but something to seek
///    (Some(event), Some(eventref)) --> we have a top local event, and something higher to seek
///
pub(crate) fn get_thread_highest_ancestors(
    eref: EventReference,
) -> Result<(Option<Event>, Option<EventReference>), Error> {
    let mut highest_local: Option<Event> = None;
    let mut highest_remote: Option<EventReference> = Some(eref);

    loop {
        match highest_remote {
            None => break,
            Some(EventReference::Id { id, .. }) => match GLOBALS.storage.read_event(id)? {
                None => break,
                Some(event) => {
                    highest_remote = event.replies_to();
                    highest_local = Some(event);
                }
            },
            Some(EventReference::Addr(ref ea)) => {
                match GLOBALS
                    .storage
                    .get_replaceable_event(ea.kind, ea.author, &ea.d)?
                {
                    None => break,
                    Some(event) => {
                        highest_remote = event.replies_to();
                        highest_local = Some(event);
                    }
                }
            }
        }
    }

    Ok((highest_local, highest_remote))
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
