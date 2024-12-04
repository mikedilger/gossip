use nostr_types::PublicKey;
use std::collections::HashSet;

pub struct Followers {
    pub who: Option<PublicKey>,
    pub set: HashSet<PublicKey>,
}

impl Default for Followers {
    fn default() -> Followers {
        Followers {
            who: None,
            set: HashSet::new(),
        }
    }
}

impl Followers {
    pub fn reset(&mut self, pubkey: PublicKey) {
        self.who = Some(pubkey);
        self.set = HashSet::new();
    }

    pub fn add_follower(&mut self, follower: PublicKey) {
        self.set.insert(follower);
    }

    pub fn get(&mut self) -> Vec<PublicKey> {
        self.set.iter().map(|&k| k).collect()
    }
}
