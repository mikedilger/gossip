use crate::error::{Error, ErrorKind};
use crate::globals::GLOBALS;
use crate::misc::Private;
use crate::people::{Person, PersonList};
use nostr_types::{Metadata, Nip05, PublicKey, RelayUrl, Unixtime};
use std::sync::atomic::Ordering;

// This updates the people map and the database with the result
pub async fn validate_nip05(person: Person) -> Result<(), Error> {
    if !GLOBALS.db().read_setting_check_nip05() {
        return Ok(());
    }

    let now = Unixtime::now();

    // invalid if their nip-05 is not set
    if person.metadata().is_none()
        || matches!(person.metadata(), Some(Metadata { nip05: None, .. }))
    {
        GLOBALS
            .people
            .upsert_nip05_validity(&person.pubkey, None, false, now.0 as u64)?;
        return Ok(());
    }

    let metadata = person.metadata().as_ref().unwrap().to_owned();
    let nip05 = metadata.nip05.as_ref().unwrap().to_owned();

    // Split their DNS ID
    let (user, domain) = match parse_nip05(&nip05) {
        Ok(pair) => pair,
        Err(_) => {
            GLOBALS.people.upsert_nip05_validity(
                &person.pubkey,
                Some(nip05),
                false,
                now.0 as u64,
            )?;
            return Ok(());
        }
    };

    // Fetch NIP-05
    let nip05file = match fetch_nip05(&user, &domain).await {
        Ok(content) => content,
        Err(e) => {
            tracing::warn!("NIP-05 fetch issue with {}@{}", user, domain);
            return Err(e);
        }
    };

    // Check if the response matches their public key
    let mut valid = false;
    match nip05file.names.get(&user) {
        Some(pk) => {
            if let Ok(pubkey) = PublicKey::try_from_hex_string(pk, true) {
                if pubkey == person.pubkey {
                    // Validated
                    GLOBALS.people.upsert_nip05_validity(
                        &person.pubkey,
                        Some(nip05.clone()),
                        true,
                        now.0 as u64,
                    )?;
                    valid = true;
                }
            } else {
                // Failed
                GLOBALS.people.upsert_nip05_validity(
                    &person.pubkey,
                    Some(nip05.clone()),
                    false,
                    now.0 as u64,
                )?;
            }
        }
        None => {
            // Failed
            GLOBALS.people.upsert_nip05_validity(
                &person.pubkey,
                Some(nip05.clone()),
                false,
                now.0 as u64,
            )?;
        }
    }

    GLOBALS.ui_invalidate_person(person.pubkey);

    if valid {
        update_relays(&nip05, nip05file, &person.pubkey)?;
    }

    Ok(())
}

pub async fn get_and_follow_nip05(
    nip05: String,
    list: PersonList,
    private: Private,
) -> Result<(), Error> {
    // Split their DNS ID
    let (user, domain) = parse_nip05(&nip05)?;

    // Fetch NIP-05
    let nip05file = fetch_nip05(&user, &domain).await?;

    // Get their pubkey
    let pubkey = match nip05file.names.get(&user) {
        Some(pk) => PublicKey::try_from_hex_string(pk, true)?,
        None => return Err(ErrorKind::Nip05KeyNotFound.into()),
    };

    // Save person
    GLOBALS.people.upsert_nip05_validity(
        &pubkey,
        Some(nip05.clone()),
        true,
        Unixtime::now().0 as u64,
    )?;

    update_relays(&nip05, nip05file, &pubkey)?;

    // Follow
    GLOBALS.people.follow(&pubkey, true, list, private)?;

    tracing::info!("Followed {}", &nip05);

    Ok(())
}

fn update_relays(nip05: &str, nip05file: Nip05, pubkey: &PublicKey) -> Result<(), Error> {
    // Set their relays
    let relays = match nip05file.relays.get(&(*pubkey).into()) {
        Some(relays) => relays,
        None => return Ok(()),
    };
    for relay in relays.iter() {
        // Save relay
        if let Ok(relay_url) = RelayUrl::try_from_unchecked_url(relay) {
            GLOBALS.db().write_relay_if_missing(&relay_url, None)?;

            // Update person_relay
            GLOBALS.db().modify_person_relay(
                *pubkey,
                &relay_url,
                |pr| {
                    pr.read = true;
                    pr.write = true;
                },
                None,
            )?;
        }
    }

    tracing::info!("Setup {} relays for {}", relays.len(), nip05);

    Ok(())
}

// returns user and domain
pub fn parse_nip05(nip05: &str) -> Result<(String, String), Error> {
    let mut parts: Vec<&str> = nip05.split('@').collect();

    // Add the underscore as a username if they just specified a domain name.
    if parts.len() == 1 {
        parts = Vec::from(["_", parts.first().unwrap()])
    }

    // Require two parts
    if parts.len() != 2 {
        Err(ErrorKind::InvalidDnsId.into())
    } else {
        let domain = parts.pop().unwrap();
        let user = parts.pop().unwrap();
        if domain.len() < 4 {
            // smallest non-TLD domain is like 't.co'
            return Err(ErrorKind::InvalidDnsId.into());
        }
        Ok((user.to_string(), domain.to_string()))
    }
}

async fn fetch_nip05(user: &str, domain: &str) -> Result<Nip05, Error> {
    // FIXME add user-agent if configured

    let nip05_future = reqwest::Client::builder()
        .timeout(std::time::Duration::new(60, 0))
        .redirect(reqwest::redirect::Policy::none()) // see NIP-05
        .gzip(true)
        .brotli(true)
        .deflate(true)
        .build()?
        .get(format!(
            "https://{}/.well-known/nostr.json?name={}",
            domain, user
        ))
        .send();
    let response = nip05_future.await?;
    let bytes = response.bytes().await?;
    GLOBALS.bytes_read.fetch_add(bytes.len(), Ordering::Relaxed);
    Ok(serde_json::from_slice(&bytes)?)
}
