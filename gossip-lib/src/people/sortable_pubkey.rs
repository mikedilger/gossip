use nostr_types::PublicKey;
use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SortablePubkey(PublicKey);

impl PartialOrd for SortablePubkey {
    fn partial_cmp(&self, other: &SortablePubkey) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SortablePubkey {
    fn cmp(&self, other: &Self) -> Ordering {
        // FIXME don't create strings if we can avoid it
        let a = crate::names::best_name_from_pubkey_lookup(&self.0);
        let b = crate::names::best_name_from_pubkey_lookup(&other.0);
        a.cmp(&b)
    }
}

impl From<PublicKey> for SortablePubkey {
    fn from(pk: PublicKey) -> SortablePubkey {
        SortablePubkey(pk)
    }
}

impl From<SortablePubkey> for PublicKey {
    fn from(sp: SortablePubkey) -> PublicKey {
        sp.0
    }
}
