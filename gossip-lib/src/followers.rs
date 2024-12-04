use crate::people::SortablePubkey;
use nostr_types::PublicKey;
use std::collections::BTreeSet;

pub struct Followers {
    pub who: Option<PublicKey>,
    pub set: BTreeSet<SortablePubkey>,
}

impl Default for Followers {
    fn default() -> Followers {
        Followers {
            who: None,
            set: BTreeSet::new(),
        }
    }
}

impl Followers {
    pub fn reset(&mut self, pubkey: PublicKey) {
        self.who = Some(pubkey);
        self.set = BTreeSet::new();
    }

    pub fn add_follower(&mut self, follower: PublicKey) {
        self.set.insert(follower.into());
    }

    pub fn get_range(&self, start: usize, amount: usize) -> Vec<PublicKey> {
        self.set
            .iter()
            .skip(start)
            .take(amount)
            .map(|k| (*k).into())
            .collect()
    }
}
