use crate::storage::{PersonTable, Table};
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

pub fn best_name_from_pubkey_lookup(pubkey: &PublicKey) -> String {
    match PersonTable::read_record(*pubkey, None) {
        Ok(Some(person)) => person.best_name(),
        _ => pubkey_short(pubkey),
    }
}
