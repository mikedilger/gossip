use nostr_types::{Profile, PublicKey, PublicKeyHex, RelayUrl, UncheckedUrl, Unixtime};
use crate::db::{DbPersonRelay, DbRelay};
use crate::error::{Error, ErrorKind};
use crate::globals::GLOBALS;
use crate::nip05;
use crate::overlord::Overlord;

impl Overlord {

    ///
    /// Automatically follow the user as provided in the query. It may be a npub, nprofile, or nip05.
    /// If a relay is provided, follow that relay.
    pub(super) async fn follow_auto(
        &mut self,
        query: String,
        relay: Option<RelayUrl>
    ) -> Result<(), Error> {
        let target = FollowTarget::new_auto(query, relay).await?;
        let mut follow = FollowAction::new(self);
        follow.follow(target).await
    }

}

///
/// Check the provided relays to make sure they can be user. Returns only the valid ones.
fn check_relays(unchecked: Vec<UncheckedUrl>) -> Vec<RelayUrl> {
    return unchecked.iter()
        .map(|ur| RelayUrl::try_from_unchecked_url(ur))
        .filter(|r| r.is_ok())
        .map(|r| r.unwrap())
        .collect()
}

///
/// Parsed query for following a user.
struct FollowTarget {
    /// The public key of the user to follow.
    pubkey: PublicKey,

    /// Optional list of relays where the user is available.
    relays: Vec<RelayUrl>,

    /// Base details to store into the database.
    /// It's automatically populated with user details and flags before being stored, and only other external details such as timestamp are used.
    person: Option<DbPersonRelay>,
}

impl FollowTarget {

    ///
    /// Create a new target to follow automatically deciding on the provided data. I.e., it parses the input and decides what is the target.
    pub(super) async fn new_auto(
        query: String,
        relay: Option<RelayUrl>
    ) -> Result<FollowTarget, Error> {
        let now = Unixtime::now().unwrap().0 as u64;
        let target: FollowTarget = if let Ok(pk) = PublicKey::try_from_bech32_string(&query) {
            FollowTarget {
                pubkey: pk,
                relays: relay.map(|r| vec![r]).unwrap_or_default(),
                person: Some(DbPersonRelay {
                    last_suggested_kind3: Some(now), // consider it our claim in our contact list
                    ..DbPersonRelay::default()
                }),
            }
        } else if let Ok(pk) = PublicKey::try_from_hex_string(&query) {
            FollowTarget {
                pubkey: pk,
                relays:relay.map(|r| vec![r]).unwrap_or_default(),
                person: Some(DbPersonRelay {
                    last_suggested_kind3: Some(now), // consider it our claim in our contact list
                    ..DbPersonRelay::default()
                }),
            }
        } else if let Ok(nip05) = nip05::parse_nip05(&query) {
            FollowTarget::from_nip05(nip05.0, nip05.1).await?
        } else if let Ok(profile) = Profile::try_from_bech32_string(&query) {
            let mut relays = check_relays(profile.relays);
            if let Some(relay) = relay {
                relays.push(relay);
            }
            FollowTarget {
                pubkey: profile.pubkey,
                relays,
                person: Some(DbPersonRelay {
                    last_suggested_kind3: Some(now), // consider it our claim in our contact list
                    ..DbPersonRelay::default()
                }),
            }
        } else {
            return Err((ErrorKind::InvalidPublicKey(query), file!(), line!()).into())
        };

        Ok(target)
    }

    async fn from_nip05(user: String, domain: String) -> Result<FollowTarget, Error> {
        let now = Unixtime::now().unwrap().0 as u64;
        let nip05 = nip05::fetch_nip05(user.as_str(), domain.as_str()).await?;

        // Get their pubkey
        let pubkeyhex = match nip05.names.get(&user) {
            Some(pk) => pk.to_owned().into(),
            None => return Err((ErrorKind::Nip05KeyNotFound, file!(), line!()).into()),
        };

        // Set their relays
        let relays = match nip05.relays.get(&pubkeyhex) {
            Some(relays) => relays.clone(),
            None => vec![],
        };

        let target = FollowTarget {
            pubkey: pubkeyhex.try_into()?,
            relays: check_relays(relays),
            person: Some(DbPersonRelay {
                last_suggested_nip05: Some(now),
                ..DbPersonRelay::default()
            }),
        };

        Ok(target)
    }
}

///
/// Implementation of the cation that follows a user.
/// Supposed to be a short living object, created just to follow a user and then dropped.
struct FollowAction<'a> {
    overlord: &'a mut Overlord,
}

impl<'a> FollowAction<'a> {

    fn new(overlord: &'a mut Overlord) -> Self {
        Self { overlord }
    }

    async fn follow(
        &mut self,
        target: FollowTarget,
    ) -> Result<(), Error> {
        self.follow_pubkey(target.pubkey.clone()).await?;

        for relay in target.relays {
            self.connect_relay(relay, target.pubkey, target.person.clone()).await?;
        }

        Ok(())
    }

    async fn follow_pubkey<P: Into<PublicKeyHex>>(&self, pk: P) -> Result<(), Error> {
        let pkhex: PublicKeyHex = pk.into();
        GLOBALS.people.async_follow(&pkhex, true).await?;

        tracing::debug!("Followed {}", &pkhex);
        Ok(())
    }

    async fn connect_relay<P: Into<PublicKeyHex>>(&mut self, relay: RelayUrl, pk: P, person: Option<DbPersonRelay>) -> Result<(), Error> {
        self.add_relay(relay.clone()).await?;
        let pkhex: PublicKeyHex = pk.into();

        // Save person_relay
        let person = DbPersonRelay {
            person: pkhex.to_string(),
            relay,
            read: false,
            write: false,
            manually_paired_read: true,
            manually_paired_write: true,

            // if a base profile is not provided use the default
            ..person.or(Some(DbPersonRelay::default())).unwrap()
        };
        DbPersonRelay::insert(person).await?;

        // async_follow added them to the relay tracker.
        // Pick relays to start tracking them now
        self.overlord.pick_relays().await;

        Ok(())
    }

    async fn add_relay(&mut self, relay: RelayUrl) -> Result<(), Error> {
        let db_relay = DbRelay::new(relay.clone());
        DbRelay::insert(db_relay).await?;
        Ok(())
    }

}
