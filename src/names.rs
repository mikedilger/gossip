use crate::globals::GLOBALS;
use crate::people::Person;
use nostr_types::{IdHex, PublicKey};

/// A short rendering of a `PublicKey`
pub fn pubkey_short(pk: &PublicKey) -> String {
    let npub = pk.as_bech32_string();
    format!(
        "{}...{}",
        &npub.get(0..10).unwrap_or("??????????"),
        &npub
            .get(npub.len() - 10..npub.len())
            .unwrap_or("??????????")
    )
}

/// A short rendering of an `IdHex`
pub fn hex_id_short(idhex: &IdHex) -> String {
    idhex.as_str()[0..8].to_string()
}

/// A display name for a `Person`
pub fn display_name_from_person(person: &Person) -> String {
    match person.display_name() {
        Some(name) => name.to_owned(),
        None => pubkey_short(&person.pubkey),
    }
}

pub fn display_name_from_pubkey_lookup(pubkey: &PublicKey) -> String {
    match GLOBALS.storage.read_person(pubkey) {
        Ok(Some(person)) => display_name_from_person(&person),
        _ => pubkey_short(pubkey),
    }
}
