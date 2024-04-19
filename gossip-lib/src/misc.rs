use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::{Event, EventReference, Id, PayRequestData, PublicKey, UncheckedUrl};

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
