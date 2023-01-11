use crate::db::{DbPerson, DbPersonRelay, DbRelay};
use crate::error::Error;
use crate::globals::GLOBALS;
use nostr_types::{Nip05, Unixtime, Url};

// This updates the people map and the database with the result
#[allow(dead_code)]
pub async fn validate_nip05(person: DbPerson) -> Result<(), Error> {
    let now = Unixtime::now().unwrap();

    // invalid if their nip-05 is not set
    if person.dns_id.is_none() {
        GLOBALS
            .people
            .write()
            .await
            .upsert_nip05_validity(&person.pubkey, person.dns_id, false, now.0 as u64)
            .await?;
        return Ok(());
    }

    // Split their DNS ID
    let dns_id = person.dns_id.clone().unwrap();
    let (user, domain) = match parse_dns_id(&dns_id) {
        Ok(pair) => pair,
        Err(_) => {
            GLOBALS
                .people
                .write()
                .await
                .upsert_nip05_validity(&person.pubkey, person.dns_id, false, now.0 as u64)
                .await?;
            return Ok(());
        }
    };

    // Fetch NIP-05
    let nip05 = match fetch_nip05(&user, &domain).await {
        Ok(content) => content,
        Err(e) => {
            tracing::error!("NIP-05 fetch issue with {}@{}", user, domain);
            return Err(e);
        }
    };

    // Check if the response matches their public key
    match nip05.names.get(&user) {
        Some(pk) => {
            if pk.as_hex_string() == person.pubkey.0 {
                // Validated
                GLOBALS
                    .people
                    .write()
                    .await
                    .upsert_nip05_validity(&person.pubkey, person.dns_id, true, now.0 as u64)
                    .await?;
            }
        }
        None => {
            // Failed
            GLOBALS
                .people
                .write()
                .await
                .upsert_nip05_validity(&person.pubkey, person.dns_id, false, now.0 as u64)
                .await?;
        }
    }

    Ok(())
}

pub async fn get_and_follow_nip05(dns_id: String) -> Result<(), Error> {
    // Split their DNS ID
    let (user, domain) = parse_dns_id(&dns_id)?;

    // Fetch NIP-05
    let nip05 = fetch_nip05(&user, &domain).await?;

    // Get their pubkey
    let pubkey = match nip05.names.get(&user) {
        Some(pk) => pk,
        None => return Err(Error::Nip05KeyNotFound),
    };

    // Save person
    GLOBALS
        .people
        .write()
        .await
        .upsert_nip05_validity(
            &(*pubkey).into(),
            Some(dns_id.clone()),
            true,
            Unixtime::now().unwrap().0 as u64,
        )
        .await?;

    // Mark as followed
    GLOBALS
        .people
        .write()
        .await
        .async_follow(&(*pubkey).into(), true)
        .await?;

    tracing::info!("Followed {}", &dns_id);

    // Set their relays
    let relays = match nip05.relays.get(pubkey) {
        Some(relays) => relays,
        None => return Err(Error::Nip05RelaysNotFound),
    };
    for relay in relays.iter() {
        // Save relay
        let relay_url = Url::new(relay);
        if relay_url.is_valid_relay_url() {
            let db_relay = DbRelay::new(relay_url.inner().to_owned())?;
            DbRelay::insert(db_relay).await?;

            // Save person_relay
            DbPersonRelay::upsert_last_suggested_nip05(
                (*pubkey).into(),
                relay.inner().to_owned(),
                Unixtime::now().unwrap().0 as u64,
            )
            .await?;
        }
    }

    tracing::info!("Setup {} relays for {}", relays.len(), &dns_id);

    Ok(())
}

// returns user and domain
fn parse_dns_id(dns_id: &str) -> Result<(String, String), Error> {
    let mut parts: Vec<&str> = dns_id.split('@').collect();

    // Add the underscore as a username if they just specified a domain name.
    if parts.len() == 1 {
        parts = Vec::from(["_", parts.first().unwrap()])
    }

    // Require two parts
    if parts.len() != 2 {
        Err(Error::InvalidDnsId)
    } else {
        let domain = parts.pop().unwrap();
        let user = parts.pop().unwrap();
        Ok((user.to_string(), domain.to_string()))
    }
}

async fn fetch_nip05(user: &str, domain: &str) -> Result<Nip05, Error> {
    let nip05_future = reqwest::Client::builder()
        .timeout(std::time::Duration::new(60, 0))
        .redirect(reqwest::redirect::Policy::none()) // see NIP-05
        .build()?
        .get(format!(
            "https://{}/.well-known/nostr.json?name={}",
            domain, user
        ))
        .header("Host", domain)
        .send();
    let response = nip05_future.await?;
    Ok(response.json::<Nip05>().await?)
}
