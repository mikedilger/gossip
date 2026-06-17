use crate::versioned::nip05::Nip05V1;

/// The content of a webserver's /.well-known/nostr.json file used in NIP-05 and NIP-35
/// This allows lookup and verification of a nostr user via a `user@domain` style identifier.
pub type Nip05 = Nip05V1;
