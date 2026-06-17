use super::TagV3;
use crate::types::{
    EventDelegation, EventKind, EventReference, FileMetadata, Id, KeySigner, MilliSatoshi, NAddr,
    NostrBech32, NostrUrl, ParsedTag, PrivateKey, PublicKey, RelayUrl, Signature, Signer, Unixtime,
    ZapData,
};
use crate::{Error, IntoVec};
use lightning_invoice::Bolt11Invoice;
#[cfg(feature = "speedy")]
use regex::Regex;
use serde::{Deserialize, Serialize};
#[cfg(feature = "speedy")]
use speedy::{Readable, Writable};
use std::cmp::Ordering;
use std::str::FromStr;

/// The main event type
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "speedy", derive(Readable, Writable))]
pub struct EventV3 {
    /// The Id of the event, generated as a SHA256 of the inner event data
    pub id: Id,

    /// The public key of the actor who created the event
    pub pubkey: PublicKey,

    /// The (unverified) time at which the event was created
    pub created_at: Unixtime,

    /// The kind of event
    pub kind: EventKind,

    /// The signature of the event, which cryptographically verifies that the holder of
    /// the PrivateKey matching the event's PublicKey generated (or authorized) this event.
    /// The signature is taken over the id field only, but the id field is taken over
    /// the rest of the event data.
    pub sig: Signature,

    /// The content of the event
    pub content: String,

    /// A set of tags that apply to the event
    pub tags: Vec<TagV3>,
}

macro_rules! serialize_inner_event {
    ($pubkey:expr, $created_at:expr, $kind:expr, $tags:expr,
     $content:expr) => {{
        format!(
            "[0,{},{},{},{},{}]",
            serde_json::to_string($pubkey)?,
            serde_json::to_string($created_at)?,
            serde_json::to_string($kind)?,
            serde_json::to_string($tags)?,
            serde_json::to_string($content)?
        )
    }};
}

/// Data used to construct an event
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "speedy", derive(Readable, Writable))]
pub struct PreEventV3 {
    /// The public key of the actor who is creating the event
    pub pubkey: PublicKey,
    /// The time at which the event was created
    pub created_at: Unixtime,
    /// The kind of event
    pub kind: EventKind,
    /// A set of tags that apply to the event
    pub tags: Vec<TagV3>,
    /// The content of the event
    pub content: String,
}

impl PreEventV3 {
    /// Generate an ID from this PreEvent for use in an Event or a Rumor
    pub fn hash(&self) -> Result<Id, Error> {
        use secp256k1::hashes::Hash;

        let serialized: String = serialize_inner_event!(
            &self.pubkey,
            &self.created_at,
            &self.kind,
            &self.tags,
            &self.content
        );

        // Hash
        let hash = secp256k1::hashes::sha256::Hash::hash(serialized.as_bytes());
        let id: [u8; 32] = hash.to_byte_array();
        Ok(Id(id))
    }
}

/// A Rumor is an Event without a signature
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "speedy", derive(Readable, Writable))]
pub struct RumorV3 {
    /// The Id of the event, generated as a SHA256 of the inner event data
    pub id: Id,

    /// The public key of the actor who created the event
    pub pubkey: PublicKey,

    /// The (unverified) time at which the event was created
    pub created_at: Unixtime,

    /// The kind of event
    pub kind: EventKind,

    /// The content of the event
    pub content: String,

    /// A set of tags that apply to the event
    pub tags: Vec<TagV3>,
}

impl RumorV3 {
    /// Create a new rumor
    pub fn new(input: PreEventV3) -> Result<RumorV3, Error> {
        // Generate Id
        let id = input.hash()?;

        Ok(RumorV3 {
            id,
            pubkey: input.pubkey,
            created_at: input.created_at,
            kind: input.kind,
            tags: input.tags,
            content: input.content,
        })
    }

    /// Turn into an Event (the signature will be all zeroes)
    pub fn into_event_with_bad_signature(self) -> EventV3 {
        EventV3 {
            id: self.id,
            pubkey: self.pubkey,
            created_at: self.created_at,
            kind: self.kind,
            sig: Signature::zeroes(),
            content: self.content,
            tags: self.tags,
        }
    }
}

impl EventV3 {
    /// Check the validity of an event. This is useful if you deserialize an event
    /// from the network. If you create an event using new() it should already be
    /// trustworthy.
    pub fn verify(&self, maxtime: Option<Unixtime>) -> Result<(), Error> {
        use secp256k1::hashes::Hash;

        let serialized: String = serialize_inner_event!(
            &self.pubkey,
            &self.created_at,
            &self.kind,
            &self.tags,
            &self.content
        );

        // Verify the signature
        self.pubkey.verify(serialized.as_bytes(), &self.sig)?;

        // Also verify the ID is the SHA256
        // (the above verify function also does it internally,
        //  so there is room for improvement here)
        let hash = secp256k1::hashes::sha256::Hash::hash(serialized.as_bytes());
        let id: [u8; 32] = hash.to_byte_array();

        // Optional verify that the message was in the past
        if let Some(mt) = maxtime {
            if self.created_at > mt {
                return Err(Error::EventInFuture);
            }
        }

        if id != self.id.0 {
            Err(Error::HashMismatch)
        } else {
            Ok(())
        }
    }

    /// Mock data for testing
    #[allow(dead_code)]
    pub(crate) async fn mock() -> EventV3 {
        let signer = {
            let private_key = PrivateKey::mock();
            KeySigner::from_private_key(private_key, "", 1).unwrap()
        };
        let public_key = signer.public_key();
        let pre = PreEventV3 {
            pubkey: public_key,
            created_at: Unixtime::mock(),
            kind: EventKind::mock(),
            tags: vec![TagV3::mock(), TagV3::mock()],
            content: "This is a test".to_string(),
        };
        signer.sign_event(pre).await.unwrap()
    }

    /// Get the k-tag kind, if any
    pub fn k_tag_kind(&self) -> Option<EventKind> {
        for tag in self.tags.iter() {
            if let Ok(ParsedTag::Kind(kind)) = tag.parse() {
                return Some(kind);
            }
        }
        None
    }

    /// If the event refers to people by tag, get all the PublicKeys it refers to
    /// along with recommended relay URL and petname for each
    pub fn people(&self) -> Vec<(PublicKey, Option<RelayUrl>, Option<String>)> {
        let mut output: Vec<(PublicKey, Option<RelayUrl>, Option<String>)> = Vec::new();
        // All 'p' tags
        for tag in self.tags.iter() {
            if let Ok(ParsedTag::Pubkey {
                pubkey,
                recommended_relay_url,
                petname,
            }) = tag.parse()
            {
                output.push((
                    pubkey.to_owned(),
                    recommended_relay_url
                        .as_ref()
                        .and_then(|rru| RelayUrl::try_from_unchecked_url(rru).ok()),
                    petname.to_owned(),
                ));
            }
        }

        output
    }

    /// If the pubkey is tagged in the event
    pub fn is_tagged(&self, pk: &PublicKey) -> bool {
        for tag in self.tags.iter() {
            if let Ok(ParsedTag::Pubkey { pubkey, .. }) = tag.parse() {
                if pubkey == *pk {
                    return true;
                }
            }
        }

        false
    }

    /// If the event refers to people within the contents, get all the PublicKeys it refers
    /// to within the contents.
    pub fn people_referenced_in_content(&self) -> Vec<PublicKey> {
        let mut output = Vec::new();
        for nurl in NostrUrl::find_all_in_string(&self.content).drain(..) {
            if let NostrBech32::Pubkey(pk) = nurl.0 {
                output.push(pk);
            }
            if let NostrBech32::Profile(prof) = nurl.0 {
                output.push(prof.pubkey);
            }
        }
        output
    }

    /// All events IDs that this event refers to, whether root, reply, mention, or otherwise
    /// along with optional recommended relay URLs
    pub fn referred_events(&self) -> Vec<EventReference> {
        let mut output: Vec<EventReference> = Vec::new();

        // Collect every 'e' tag and 'a' tag
        for tag in self.tags.iter() {
            if let Ok(ParsedTag::Event {
                id,
                recommended_relay_url: rurl,
                marker,
                author_pubkey,
            }) = tag.parse()
            {
                output.push(EventReference::Id {
                    id,
                    author: author_pubkey,
                    relays: rurl
                        .as_ref()
                        .and_then(|rru| RelayUrl::try_from_unchecked_url(rru).ok())
                        .into_vec(),
                    marker,
                });
            } else if let Ok(ParsedTag::Address { address, .. }) = tag.parse() {
                output.push(EventReference::Addr(address))
            }
        }

        output
    }

    /// Get a reference to another event that this event replies to.
    /// An event can only reply to one other event via 'e' or 'a' tag from a feed-displayable
    /// event that is not a Repost.
    pub fn replies_to(&self) -> Option<EventReference> {
        if !self.kind.is_feed_displayable() {
            return None;
        }

        // Repost 'e' and 'a' tags are always considered mentions, not replies.
        if self.kind == EventKind::Repost || self.kind == EventKind::GenericRepost {
            return None;
        }

        // In kind 1111, use the 'e' tag
        if self.kind == EventKind::Comment {
            let tags: Vec<&TagV3> = self.tags.iter().filter(|t| t.tagname() == "e").collect();
            if !tags.is_empty() {
                if let Ok(parsed) = tags[0].parse() {
                    return parsed.into_event_reference();
                }
            }
        }

        // 'e' tags marked "reply"
        let reply_tags: Vec<&TagV3> = self
            .tags
            .iter()
            .filter(|t| (t.tagname() == "e" || t.tagname() == "a") && t.marker() == "reply")
            .collect();
        if !reply_tags.is_empty() {
            if let Ok(parsed) = reply_tags[0].parse() {
                return parsed.into_event_reference();
            }
        }

        // 'e' tags marked "root" serve as replies if none were marked "reply"
        let root_tags: Vec<&TagV3> = self
            .tags
            .iter()
            .filter(|t| (t.tagname() == "e" || t.tagname() == "a") && t.marker() == "root")
            .collect();
        if !root_tags.is_empty() {
            if let Ok(parsed) = root_tags[0].parse() {
                return parsed.into_event_reference();
            }
        }

        // deprecated positional logic:  Use the last 'e' or 'a' tag that doesn't
        // have a marker, preferring 'a' tags (this function doesn't give you both).
        let a_tags: Vec<&TagV3> = self
            .tags
            .iter()
            .rev()
            .filter(|t| t.tagname() == "a" && t.marker() == "")
            .collect();
        if !a_tags.is_empty() {
            if let Ok(parsed) = a_tags[0].parse() {
                return parsed.into_event_reference();
            }
        }

        let e_tags: Vec<&TagV3> = self
            .tags
            .iter()
            .rev()
            .filter(|t| t.tagname() == "e" && t.marker() == "")
            .collect();
        if !e_tags.is_empty() {
            if let Ok(parsed) = e_tags[0].parse() {
                return parsed.into_event_reference();
            }
        }

        None
    }

    /// If this event replies to a thread, get that threads root event Id if
    /// available, along with an optional recommended_relay_url
    pub fn replies_to_root(&self) -> Option<EventReference> {
        if !self.kind.is_feed_displayable() {
            return None;
        }

        let root_referencing_tags: Vec<&TagV3> = self
            .tags
            .iter()
            .filter(|t| t.tagname() == "E" || t.tagname() == "A" || t.marker() == "root")
            .collect();

        if !root_referencing_tags.is_empty() {
            if let Ok(parsed) = root_referencing_tags[0].parse() {
                return parsed.into_event_reference();
            }
        } else {
            // deprecated positional logic:  Use the first 'e' or 'a' tag, but only if there are
            // multiple event referencing tags (not including 'q'), and don't mix 'e' and 'a'
            // when counting multiples.

            let e_tags: Vec<&TagV3> = self
                .tags
                .iter()
                .filter(|t| t.tagname() == "e" && t.marker() == "")
                .collect();
            if e_tags.len() > 1 {
                if let Ok(parsed) = e_tags[0].parse() {
                    return parsed.into_event_reference();
                }
            }

            let a_tags: Vec<&TagV3> = self
                .tags
                .iter()
                .filter(|t| t.tagname() == "a" && t.marker() == "")
                .collect();
            if a_tags.len() > 1 {
                if let Ok(parsed) = a_tags[0].parse() {
                    return parsed.into_event_reference();
                }
            }
        }

        None
    }

    /// If this event quotes others, get those other events
    pub fn quotes(&self) -> Vec<EventReference> {
        if self.kind != EventKind::TextNote && self.kind != EventKind::Comment {
            return vec![];
        }

        let mut output: Vec<EventReference> = Vec::new();

        for tag in self.tags.iter() {
            if let Ok(ParsedTag::Quote {
                id,
                recommended_relay_url,
                author_pubkey,
            }) = tag.parse()
            {
                output.push(EventReference::Id {
                    id,
                    author: author_pubkey,
                    relays: recommended_relay_url
                        .as_ref()
                        .and_then(|rru| RelayUrl::try_from_unchecked_url(rru).ok())
                        .into_vec(),
                    marker: None,
                });
            }
        }

        output
    }

    /// If this event mentions others, get those other event Ids
    /// and optional recommended relay Urls
    pub fn mentions(&self) -> Vec<EventReference> {
        if !self.kind.is_feed_displayable() {
            return vec![];
        }

        let mut output: Vec<EventReference> = Vec::new();

        // For kind=6 and kind=16, all 'e' and 'a' tags are mentions
        if self.kind == EventKind::Repost || self.kind == EventKind::GenericRepost {
            for tag in self.tags.iter() {
                if let Ok(ParsedTag::Event {
                    id,
                    recommended_relay_url,
                    marker,
                    author_pubkey,
                }) = tag.parse()
                {
                    output.push(EventReference::Id {
                        id,
                        author: author_pubkey,
                        relays: recommended_relay_url
                            .as_ref()
                            .and_then(|rru| RelayUrl::try_from_unchecked_url(rru).ok())
                            .into_vec(),
                        marker,
                    });
                } else if let Ok(ParsedTag::Address { address, .. }) = tag.parse() {
                    output.push(EventReference::Addr(address));
                }
            }

            return output;
        }

        // Look for nostr links within the content

        for tag in self.tags.iter() {
            // Collect every 'e' tag marked as 'mention'
            if let Ok(ParsedTag::Event {
                id,
                recommended_relay_url,
                marker,
                author_pubkey,
            }) = tag.parse()
            {
                if marker.is_some() && marker.as_deref().unwrap() == "mention" {
                    output.push(EventReference::Id {
                        id,
                        author: author_pubkey,
                        relays: recommended_relay_url
                            .as_ref()
                            .and_then(|rru| RelayUrl::try_from_unchecked_url(rru).ok())
                            .into_vec(),
                        marker,
                    });
                }
            }

            // Collect every 'q' tag
            if let Ok(ParsedTag::Quote {
                id,
                recommended_relay_url,
                author_pubkey,
            }) = tag.parse()
            {
                output.push(EventReference::Id {
                    id,
                    author: author_pubkey,
                    relays: recommended_relay_url
                        .as_ref()
                        .and_then(|rru| RelayUrl::try_from_unchecked_url(rru).ok())
                        .into_vec(),
                    marker: None,
                });
            }
        }

        // Collect every unmarked 'e' or 'a' tag that is not the first (root) or the last (reply)
        let e_tags: Vec<&TagV3> = self
            .tags
            .iter()
            .filter(|t| (t.tagname() == "e" || t.tagname() == "a") && t.marker() == "")
            .collect();
        if e_tags.len() > 2 {
            // mentions are everything other than first and last
            for tag in &e_tags[1..e_tags.len() - 1] {
                if let Ok(ParsedTag::Event {
                    id,
                    recommended_relay_url,
                    marker,
                    author_pubkey,
                }) = tag.parse()
                {
                    output.push(EventReference::Id {
                        id,
                        author: author_pubkey,
                        relays: recommended_relay_url
                            .as_ref()
                            .and_then(|rru| RelayUrl::try_from_unchecked_url(rru).ok())
                            .into_vec(),
                        marker,
                    });
                } else if let Ok(ParsedTag::Address { address, .. }) = tag.parse() {
                    output.push(EventReference::Addr(address));
                }
            }
        }

        output
    }

    /// If this event reacts to another, get that other event's Id,
    /// the reaction content, and an optional Recommended relay Url
    pub fn reacts_to(&self) -> Option<(EventReference, String)> {
        if self.kind != EventKind::Reaction {
            return None;
        }

        // The last 'e' tag is it
        for tag in self.tags.iter().rev() {
            if let Ok(ParsedTag::Event {
                id,
                recommended_relay_url,
                marker,
                author_pubkey,
            }) = tag.parse()
            {
                return Some((
                    EventReference::Id {
                        id,
                        author: author_pubkey,
                        relays: recommended_relay_url
                            .as_ref()
                            .and_then(|rru| RelayUrl::try_from_unchecked_url(rru).ok())
                            .into_vec(),
                        marker,
                    },
                    self.content.clone(),
                ));
            }
        }

        None
    }

    /// If this event deletes others, get all the EventReferences of the events that it
    /// deletes along with the reason for the deletion
    pub fn deletes(&self) -> Option<(Vec<EventReference>, String)> {
        if self.kind != EventKind::EventDeletion {
            return None;
        }

        let mut erefs: Vec<EventReference> = Vec::new();

        for tag in self.tags.iter() {
            if let Ok(ParsedTag::Event {
                id,
                recommended_relay_url,
                marker,
                author_pubkey,
            }) = tag.parse()
            {
                // All 'e' tags are deleted
                erefs.push(EventReference::Id {
                    id,
                    author: author_pubkey,
                    relays: recommended_relay_url
                        .as_ref()
                        .and_then(|rru| RelayUrl::try_from_unchecked_url(rru).ok())
                        .into_vec(),
                    marker,
                });
            } else if let Ok(ParsedTag::Address { address, .. }) = tag.parse() {
                erefs.push(EventReference::Addr(address));
            }
        }

        if erefs.is_empty() {
            None
        } else {
            Some((erefs, self.content.clone()))
        }
    }

    /// Can this event be deleted by the given public key?
    pub fn delete_author_allowed(&self, by: PublicKey) -> bool {
        // Author can always delete
        if self.pubkey == by {
            return true;
        }

        if self.kind == EventKind::GiftWrap {
            for tag in self.tags.iter() {
                if let Ok(ParsedTag::Pubkey { pubkey, .. }) = tag.parse() {
                    return by == pubkey;
                }
            }
        }

        false
    }

    /// If this event zaps another event, get data about that.
    ///
    /// Errors returned from this are not fatal, but may be useful for
    /// explaining to a user why a zap receipt is invalid.
    pub fn zaps(&self) -> Result<Option<ZapData>, Error> {
        if self.kind != EventKind::Zap {
            return Ok(None);
        }

        let (zap_request, bolt11invoice, payer_p_tag, target_event_from_tags): (
            EventV3,
            Bolt11Invoice,
            Option<PublicKey>,
            Option<EventReference>,
        ) = {
            let mut zap_request: Option<EventV3> = None;
            let mut bolt11invoice: Option<Bolt11Invoice> = None;
            let mut payer_p_tag: Option<PublicKey> = None;
            let mut target_event_from_tags: Option<EventReference> = None;

            for tag in self.tags.iter() {
                if tag.tagname() == "description" {
                    let request_string = tag.value();
                    if let Ok(e) = serde_json::from_str::<EventV3>(request_string) {
                        zap_request = Some(e);
                    }
                }
                // we ignore the "p" tag, we have that data from two other places (invoice and request)
                else if tag.tagname() == "P" {
                    if let Ok(ParsedTag::Pubkey { pubkey, .. }) = tag.parse() {
                        payer_p_tag = Some(pubkey);
                    }
                } else if tag.tagname() == "bolt11" {
                    if tag.value() == "" {
                        return Err(Error::ZapReceipt("missing bolt11 tag value".to_string()));
                    }

                    // Extract as an Invoice
                    let invoice = match Bolt11Invoice::from_str(tag.value()) {
                        Ok(inv) => inv,
                        Err(e) => {
                            return Err(Error::ZapReceipt(format!("bolt11 failed to parse: {}", e)))
                        }
                    };

                    // Verify the signature
                    if let Err(e) = invoice.check_signature() {
                        return Err(Error::ZapReceipt(format!(
                            "bolt11 signature check failed: {}",
                            e
                        )));
                    }

                    bolt11invoice = Some(invoice);
                }
            }

            let re = self.referred_events();
            if re.len() == 1 {
                target_event_from_tags = Some(re[0].clone());
            }

            // "The zap receipt MUST contain a description tag which is the JSON-encoded zap request."
            if zap_request.is_none() {
                return Ok(None);
            }
            let zap_request = zap_request.unwrap();

            // "The zap receipt MUST have a bolt11 tag containing the description hash bolt11 invoice."
            if bolt11invoice.is_none() {
                return Ok(None);
            }
            let bolt11invoice = bolt11invoice.unwrap();

            (
                zap_request,
                bolt11invoice,
                payer_p_tag,
                target_event_from_tags,
            )
        };

        // Extract data from the invoice
        let (payee_from_invoice, amount_from_invoice): (PublicKey, MilliSatoshi) = {
            // Get the public key
            let secpk = match bolt11invoice.payee_pub_key() {
                Some(pubkey) => pubkey.to_owned(),
                None => bolt11invoice.recover_payee_pub_key(),
            };
            let (xonlypk, _) = secpk.x_only_public_key();
            let pubkeybytes = xonlypk.serialize();
            let pubkey = match PublicKey::from_bytes(&pubkeybytes, false) {
                Ok(pubkey) => pubkey,
                Err(e) => return Err(Error::ZapReceipt(format!("payee public key error: {}", e))),
            };

            if let Some(u) = bolt11invoice.amount_milli_satoshis() {
                (pubkey, MilliSatoshi(u))
            } else {
                return Err(Error::ZapReceipt(
                    "Amount missing from zap receipt".to_string(),
                ));
            }
        };

        // Extract data from request
        let (payer_from_request, amount_from_request, target_event_from_request): (
            PublicKey,
            MilliSatoshi,
            EventReference,
        ) = {
            let mut amount_from_request: Option<MilliSatoshi> = None;
            let mut target_event_from_request: Option<EventReference> = None;
            for tag in zap_request.tags.iter() {
                if tag.tagname() == "amount" {
                    if let Ok(m) = tag.value().parse::<u64>() {
                        amount_from_request = Some(MilliSatoshi(m));
                    }
                }
            }

            let re = zap_request.referred_events();
            if re.len() == 1 {
                target_event_from_request = Some(re[0].clone());
            }

            if amount_from_request.is_none() {
                return Err(Error::ZapReceipt("zap request had no amount".to_owned()));
            }
            let amount_from_request = amount_from_request.unwrap();

            if target_event_from_request.is_none() {
                return Ok(None);
            }
            let target_event_from_request = target_event_from_request.unwrap();

            (
                zap_request.pubkey,
                amount_from_request,
                target_event_from_request,
            )
        };

        if let Some(p) = payer_p_tag {
            if p != payer_from_request {
                return Err(Error::ZapReceipt(
                    "Payer Mismatch between receipt P-tag and invoice".to_owned(),
                ));
            }
        }

        if amount_from_invoice != amount_from_request {
            return Err(Error::ZapReceipt(
                "Amount Mismatch between request and invoice".to_owned(),
            ));
        }

        if let Some(te) = target_event_from_tags {
            if te != target_event_from_request {
                return Err(Error::ZapReceipt(
                    "Zapped event Mismatch receipt and request".to_owned(),
                ));
            }
        }

        Ok(Some(ZapData {
            zapped_event: target_event_from_request,
            amount: amount_from_invoice,
            payee: payee_from_invoice,
            payer: payer_from_request,
            provider_pubkey: self.pubkey,
        }))
    }

    /// If this event specifies the client that created it, return that client string
    pub fn client(&self) -> Option<String> {
        for tag in self.tags.iter() {
            if tag.tagname() == "client" && !tag.value().is_empty() {
                return Some(tag.value().to_owned());
            }
        }

        None
    }

    /// If this event specifies a subject, return that subject string
    pub fn subject(&self) -> Option<String> {
        for tag in self.tags.iter() {
            if let Ok(ParsedTag::Subject(subject)) = tag.parse() {
                return Some(subject);
            }
        }

        None
    }

    /// If this event specifies a title, return that title string
    pub fn title(&self) -> Option<String> {
        for tag in self.tags.iter() {
            if let Ok(ParsedTag::Title(title)) = tag.parse() {
                return Some(title);
            }
        }

        None
    }

    /// If this event specifies a summary, return that summary string
    pub fn summary(&self) -> Option<String> {
        for tag in self.tags.iter() {
            if let Ok(ParsedTag::Summary(summary)) = tag.parse() {
                return Some(summary);
            }
        }

        None
    }

    /// Is this event an annotation
    pub fn is_annotation(&self) -> bool {
        for tag in self.tags.iter() {
            if tag.get_index(0) == "annotation" {
                return true;
            }
        }
        false
    }

    /// If this event specifies a content warning, return that content warning
    pub fn content_warning(&self) -> Option<Option<String>> {
        for tag in self.tags.iter() {
            if let Ok(ParsedTag::ContentWarning(contentwarning)) = tag.parse() {
                return Some(contentwarning);
            }
        }

        None
    }

    /// If this is a parameterized event, get the parameter
    pub fn parameter(&self) -> Option<String> {
        if self.kind.is_parameterized_replaceable() {
            for tag in self.tags.iter() {
                if let Ok(ParsedTag::Identifier(ident)) = tag.parse() {
                    return Some(ident);
                }
            }
            Some("".to_owned()) // implicit
        } else {
            None
        }
    }

    /// If this event has an address, get it
    pub fn address(&self) -> Option<NAddr> {
        self.parameter().map(|parameter| NAddr {
            d: parameter,
            relays: vec![],
            kind: self.kind,
            author: self.pubkey,
        })
    }

    /// Return all the hashtags this event refers to
    pub fn hashtags(&self) -> Vec<String> {
        if !self.kind.is_feed_displayable() {
            return vec![];
        }

        let mut output: Vec<String> = Vec::new();

        for tag in self.tags.iter() {
            if let Ok(ParsedTag::Hashtag(hashtag)) = tag.parse() {
                output.push(hashtag);
            }
        }

        output
    }

    /// Return all attached FileMetadata objects
    pub fn file_metadata(&self) -> Vec<FileMetadata> {
        let mut output: Vec<FileMetadata> = Vec::new();

        for tag in self.tags.iter() {
            if tag.tagname() == "imeta" {
                if let Some(fm) = FileMetadata::from_imeta_tag(tag) {
                    output.push(fm);
                }
            }
        }

        output
    }

    /// Return all the URLs this event refers to
    pub fn urls(&self) -> Vec<RelayUrl> {
        if !self.kind.is_feed_displayable() {
            return vec![];
        }

        let mut output: Vec<RelayUrl> = Vec::new();

        for tag in self.tags.iter() {
            if let Ok(ParsedTag::RelayUsage { url, .. }) = tag.parse() {
                if let Ok(relay_url) = RelayUrl::try_from_unchecked_url(&url) {
                    output.push(relay_url);
                }
            }
        }

        output
    }

    /// Get the proof-of-work count of leading bits
    pub fn pow(&self) -> u8 {
        // Count leading bits in the Id field
        let zeroes: u8 = crate::get_leading_zero_bits(&self.id.0);

        // Check that they meant it
        for tag in self.tags.iter() {
            if let Ok(ParsedTag::Nonce {
                target: Some(targ), ..
            }) = tag.parse()
            {
                let target_zeroes = targ as u8;
                return zeroes.min(target_zeroes);
            }
        }

        0
    }

    /// Was this event delegated, was that valid, and if so what is the pubkey of
    /// the delegator?
    pub fn delegation(&self) -> EventDelegation {
        for tag in self.tags.iter() {
            if let Ok(ParsedTag::Delegation {
                pubkey,
                conditions,
                sig,
            }) = tag.parse()
            {
                // Verify the delegation tag
                match conditions.verify_signature(&pubkey, &self.pubkey, &sig) {
                    Ok(_) => {
                        // Check conditions
                        if let Some(kind) = conditions.kind {
                            if self.kind != kind {
                                return EventDelegation::InvalidDelegation(
                                    "Event Kind not delegated".to_owned(),
                                );
                            }
                        }
                        if let Some(created_after) = conditions.created_after {
                            if self.created_at < created_after {
                                return EventDelegation::InvalidDelegation(
                                    "Event created before delegation started".to_owned(),
                                );
                            }
                        }
                        if let Some(created_before) = conditions.created_before {
                            if self.created_at > created_before {
                                return EventDelegation::InvalidDelegation(
                                    "Event created after delegation ended".to_owned(),
                                );
                            }
                        }
                        return EventDelegation::DelegatedBy(pubkey);
                    }
                    Err(e) => {
                        return EventDelegation::InvalidDelegation(format!("{e}"));
                    }
                }
            }
        }

        EventDelegation::NotDelegated
    }

    /// If the event came through a proxy, get the (Protocol, Id)
    pub fn proxy(&self) -> Option<(String, String)> {
        for tag in self.tags.iter() {
            if let Ok(ParsedTag::Proxy { id, protocol }) = tag.parse() {
                return Some((protocol, id));
            }
        }
        None
    }
}

impl Ord for EventV3 {
    fn cmp(&self, other: &Self) -> Ordering {
        self.created_at
            .cmp(&other.created_at)
            .then(self.id.cmp(&other.id))
    }
}

impl PartialOrd for EventV3 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// Direct access into speedy-serialized bytes, to avoid alloc-deserialize just to peek
// at one of these fields
#[cfg(feature = "speedy")]
impl EventV3 {
    /// Read the ID of the event from a speedy encoding without decoding
    /// (zero allocation)
    ///
    /// Note this function is fragile, if the Event structure is reordered,
    /// or if speedy code changes, this will break.  Neither should happen.
    pub fn get_id_from_speedy_bytes(bytes: &[u8]) -> Option<Id> {
        if bytes.len() < 32 {
            None
        } else if let Ok(arr) = <[u8; 32]>::try_from(&bytes[0..32]) {
            Some(unsafe { std::mem::transmute::<[u8; 32], Id>(arr) })
        } else {
            None
        }
    }

    /// Read the pubkey of the event from a speedy encoding without decoding
    /// (close to zero allocation, VerifyingKey does stuff I didn't check)
    ///
    /// Note this function is fragile, if the Event structure is reordered,
    /// or if speedy code changes, this will break.  Neither should happen.
    pub fn get_pubkey_from_speedy_bytes(bytes: &[u8]) -> Option<PublicKey> {
        if bytes.len() < 64 {
            None
        } else {
            PublicKey::from_bytes(&bytes[32..64], false).ok()
        }
    }

    /// Read the created_at of the event from a speedy encoding without decoding
    /// (zero allocation)
    ///
    /// Note this function is fragile, if the Event structure is reordered,
    /// or if speedy code changes, this will break.  Neither should happen.
    pub fn get_created_at_from_speedy_bytes(bytes: &[u8]) -> Option<Unixtime> {
        if bytes.len() < 72 {
            None
        } else if let Ok(i) = i64::read_from_buffer(&bytes[64..72]) {
            Some(Unixtime(i))
        } else {
            None
        }
    }

    /// Read the kind of the event from a speedy encoding without decoding
    /// (zero allocation)
    ///
    /// Note this function is fragile, if the Event structure is reordered,
    /// or if speedy code changes, this will break.  Neither should happen.
    pub fn get_kind_from_speedy_bytes(bytes: &[u8]) -> Option<EventKind> {
        if bytes.len() < 76 {
            None
        } else if let Ok(u) = u32::read_from_buffer(&bytes[72..76]) {
            Some(u.into())
        } else {
            None
        }
    }

    // Read the sig of the event from a speedy encoding without decoding
    // (offset would be 76..140

    /// Read the content of the event from a speedy encoding without decoding
    /// (zero allocation)
    ///
    /// Note this function is fragile, if the Event structure is reordered,
    /// or if speedy code changes, this will break.  Neither should happen.
    pub fn get_content_from_speedy_bytes(bytes: &[u8]) -> Option<&str> {
        let len = u32::from_ne_bytes(bytes[140..140 + 4].try_into().unwrap());

        unsafe {
            Some(std::str::from_utf8_unchecked(
                &bytes[140 + 4..140 + 4 + len as usize],
            ))
        }
    }

    /// Check if any human-readable tag matches the Regex in the speedy encoding
    /// without decoding the whole thing (because our TagV3 representation is so complicated,
    /// we do deserialize the tags for now)
    ///
    /// Note this function is fragile, if the Event structure is reordered,
    /// or if speedy code changes, this will break.  Neither should happen.
    pub fn tag_search_in_speedy_bytes(bytes: &[u8], re: &Regex) -> Result<bool, Error> {
        if bytes.len() < 140 {
            return Ok(false);
        }

        // skip content
        let len = u32::from_ne_bytes(bytes[140..140 + 4].try_into().unwrap());
        let offset = 140 + 4 + len as usize;

        // Deserialize the tags
        let tags: Vec<TagV3> = Vec::<TagV3>::read_from_buffer(&bytes[offset..])?;

        // Search through them
        for tag in &tags {
            match tag.tagname() {
                "content-warning" => {
                    if let Ok(ParsedTag::ContentWarning(Some(w))) = tag.parse() {
                        if re.is_match(w.as_ref()) {
                            return Ok(true);
                        }
                    }
                }
                "t" => {
                    if let Ok(ParsedTag::Hashtag(hashtag)) = tag.parse() {
                        if re.is_match(hashtag.as_ref()) {
                            return Ok(true);
                        }
                    }
                }
                "subject" => {
                    if let Ok(ParsedTag::Subject(subject)) = tag.parse() {
                        if re.is_match(subject.as_ref()) {
                            return Ok(true);
                        }
                    }
                }
                "title" => {
                    if let Ok(ParsedTag::Title(title)) = tag.parse() {
                        if re.is_match(title.as_ref()) {
                            return Ok(true);
                        }
                    }
                }
                _ => {
                    if tag.tagname() == "summary" && re.is_match(tag.value()) {
                        return Ok(true);
                    }
                }
            }
        }

        Ok(false)
    }
}

impl From<EventV3> for RumorV3 {
    fn from(e: EventV3) -> RumorV3 {
        RumorV3 {
            id: e.id,
            pubkey: e.pubkey,
            created_at: e.created_at,
            kind: e.kind,
            content: e.content,
            tags: e.tags,
        }
    }
}

impl From<RumorV3> for PreEventV3 {
    fn from(r: RumorV3) -> PreEventV3 {
        PreEventV3 {
            pubkey: r.pubkey,
            created_at: r.created_at,
            kind: r.kind,
            content: r.content,
            tags: r.tags,
        }
    }
}

impl TryFrom<PreEventV3> for RumorV3 {
    type Error = Error;
    fn try_from(e: PreEventV3) -> Result<RumorV3, Error> {
        RumorV3::new(e)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::types::{DelegationConditions, Signer, SignerExt, UncheckedUrl};

    test_serde_async! {EventV3, test_event_serde}

    #[tokio::test]
    async fn test_event_new_and_verify() {
        let signer = {
            let privkey = PrivateKey::mock();
            KeySigner::from_private_key(privkey, "", 1).unwrap()
        };
        let pubkey = signer.public_key();
        let preevent = PreEventV3 {
            pubkey,
            created_at: Unixtime::mock(),
            kind: EventKind::TextNote,
            tags: vec![ParsedTag::Event {
                id: Id::mock(),
                recommended_relay_url: Some(UncheckedUrl::mock()),
                marker: None,
                author_pubkey: None,
            }
            .into_tag()],
            content: "Hello World!".to_string(),
        };
        let mut event = signer.sign_event(preevent).await.unwrap();

        assert!(event.verify(None).is_ok());

        // Now make sure it fails when the message has been modified
        event.content = "I'm changing this message".to_string();
        let result = event.verify(None);
        assert!(result.is_err());

        // Change it back
        event.content = "Hello World!".to_string();
        let result = event.verify(None);
        assert!(result.is_ok());

        // Tweak the id only
        event.id = Id([
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            24, 25, 26, 27, 28, 29, 30, 31,
        ]);
        let result = event.verify(None);
        assert!(result.is_err());
    }

    // helper
    async fn create_event_with_delegation<S>(created_at: Unixtime, real_signer: &S) -> EventV3
    where
        S: Signer + SignerExt,
    {
        let delegated_signer = {
            let privkey = PrivateKey::mock();
            KeySigner::from_private_key(privkey, "", 1).unwrap()
        };

        let conditions = DelegationConditions::try_from_str(
            "kind=1&created_at>1680000000&created_at<1680050000",
        )
        .unwrap();

        let sig = real_signer
            .generate_delegation_signature(delegated_signer.public_key(), &conditions)
            .await
            .unwrap();

        let preevent = PreEventV3 {
            pubkey: delegated_signer.public_key(),
            created_at,
            kind: EventKind::TextNote,
            tags: vec![
                ParsedTag::Event {
                    id: Id::mock(),
                    recommended_relay_url: Some(UncheckedUrl::mock()),
                    marker: None,
                    author_pubkey: None,
                }
                .into_tag(),
                ParsedTag::Delegation {
                    pubkey: real_signer.public_key(),
                    conditions,
                    sig,
                }
                .into_tag(),
            ],
            content: "Hello World!".to_string(),
        };
        delegated_signer.sign_event(preevent).await.unwrap()
    }

    #[tokio::test]
    async fn test_event_with_delegation_ok() {
        let delegator_signer = {
            let delegator_privkey = PrivateKey::mock();
            KeySigner::from_private_key(delegator_privkey, "", 1).unwrap()
        };
        let delegator_pubkey = delegator_signer.public_key();

        let event = create_event_with_delegation(Unixtime(1680000012), &delegator_signer).await;
        assert!(event.verify(None).is_ok());

        // check delegation
        if let EventDelegation::DelegatedBy(pk) = event.delegation() {
            // expected type, check returned delegator key
            assert_eq!(pk, delegator_pubkey);
        } else {
            panic!("Expected DelegatedBy result, got {:?}", event.delegation());
        }
    }

    #[tokio::test]
    async fn test_event_with_delegation_invalid_created_after() {
        let delegator_privkey = PrivateKey::mock();
        let signer = KeySigner::from_private_key(delegator_privkey, "", 1).unwrap();

        let event = create_event_with_delegation(Unixtime(1690000000), &signer).await;
        assert!(event.verify(None).is_ok());

        // check delegation
        if let EventDelegation::InvalidDelegation(reason) = event.delegation() {
            // expected type, check returned delegator key
            assert_eq!(reason, "Event created after delegation ended");
        } else {
            panic!(
                "Expected InvalidDelegation result, got {:?}",
                event.delegation()
            );
        }
    }

    #[tokio::test]
    async fn test_event_with_delegation_invalid_created_before() {
        let signer = {
            let delegator_privkey = PrivateKey::mock();
            KeySigner::from_private_key(delegator_privkey, "", 1).unwrap()
        };

        let event = create_event_with_delegation(Unixtime(1610000000), &signer).await;
        assert!(event.verify(None).is_ok());

        // check delegation
        if let EventDelegation::InvalidDelegation(reason) = event.delegation() {
            // expected type, check returned delegator key
            assert_eq!(reason, "Event created before delegation started");
        } else {
            panic!(
                "Expected InvalidDelegation result, got {:?}",
                event.delegation()
            );
        }
    }

    #[test]
    fn test_realworld_event_with_naddr_tag() {
        let raw = r##"{"id":"7760408f6459b9546c3a4e70e3e56756421fba34526b7d460db3fcfd2f8817db","pubkey":"460c25e682fda7832b52d1f22d3d22b3176d972f60dcdc3212ed8c92ef85065c","created_at":1687616920,"kind":1,"tags":[["p","1bc70a0148b3f316da33fe3c89f23e3e71ac4ff998027ec712b905cd24f6a411","","mention"],["a","30311:1bc70a0148b3f316da33fe3c89f23e3e71ac4ff998027ec712b905cd24f6a411:1687612774","","mention"]],"content":"Watching Karnage's stream to see if I learn something about design. \n\nnostr:naddr1qq9rzd3cxumrzv3hxu6qygqmcu9qzj9n7vtd5vl78jyly037wxkyl7vcqflvwy4eqhxjfa4yzypsgqqqwens0qfplk","sig":"dbc5d05a24bfe990a1faaedfcb81a98940d86a105711dbdad9145d05b0ad0f46e3e24eaa3fc283818f27e057fe836a029fd9a68e7f1de06ff477493199d64064"}"##;
        let _: EventV3 = serde_json::from_str(raw).unwrap();
    }

    #[cfg(feature = "speedy")]
    #[tokio::test]
    async fn test_speedy_encoded_direct_field_access() {
        use speedy::Writable;

        let signer = {
            let privkey = PrivateKey::mock();
            KeySigner::from_private_key(privkey, "", 1).unwrap()
        };

        let preevent = PreEventV3 {
            pubkey: signer.public_key(),
            created_at: Unixtime(1680000012),
            kind: EventKind::TextNote,
            tags: vec![
                ParsedTag::Event {
                    id: Id::mock(),
                    recommended_relay_url: Some(UncheckedUrl::mock()),
                    marker: None,
                    author_pubkey: None,
                }
                .into_tag(),
                ParsedTag::Hashtag("foodstr".to_string()).into_tag(),
            ],
            content: "Hello World!".to_string(),
        };
        let event = signer.sign_event(preevent).await.unwrap();
        let bytes = event.write_to_vec().unwrap();

        let id = EventV3::get_id_from_speedy_bytes(&bytes).unwrap();
        assert_eq!(id, event.id);

        let pubkey = EventV3::get_pubkey_from_speedy_bytes(&bytes).unwrap();
        assert_eq!(pubkey, event.pubkey);

        let created_at = EventV3::get_created_at_from_speedy_bytes(&bytes).unwrap();
        assert_eq!(created_at, Unixtime(1680000012));

        let kind = EventV3::get_kind_from_speedy_bytes(&bytes).unwrap();
        assert_eq!(kind, event.kind);

        let content = EventV3::get_content_from_speedy_bytes(&bytes);
        assert_eq!(content, Some(&*event.content));

        let re = regex::Regex::new("foodstr").unwrap();
        let found_foodstr = EventV3::tag_search_in_speedy_bytes(&bytes, &re).unwrap();
        assert!(found_foodstr);

        // Print to work out encoding
        //   test like this to see printed data:
        //   cargo test --features=speedy test_speedy_encoded_direct_field_access -- --nocapture
        println!("EVENT BYTES: {:?}", bytes);
        println!("ID: {:?}", event.id.0);
        println!("PUBKEY: {:?}", event.pubkey.as_slice());
        println!("CREATED AT: {:?}", event.created_at.0.to_ne_bytes());
        let kind32: u32 = event.kind.into();
        println!("KIND: {:?}", kind32.to_ne_bytes());
        println!("SIG: {:?}", event.sig.0.as_ref());
        println!(
            "CONTENT: [len={:?}] {:?}",
            (event.content.as_bytes().len() as u32).to_ne_bytes(),
            event.content.as_bytes()
        );
        println!("TAGS: [len={:?}]", (event.tags.len() as u32).to_ne_bytes());
    }

    #[tokio::test]
    async fn test_event_gift_wrap() {
        let signer1 = {
            let sec1 = PrivateKey::try_from_hex_string(
                "0000000000000000000000000000000000000000000000000000000000000001",
            )
            .unwrap();
            KeySigner::from_private_key(sec1, "", 1).unwrap()
        };

        let signer2 = {
            let sec2 = PrivateKey::try_from_hex_string(
                "0000000000000000000000000000000000000000000000000000000000000002",
            )
            .unwrap();
            KeySigner::from_private_key(sec2, "", 1).unwrap()
        };

        let pre = PreEventV3 {
            pubkey: signer1.public_key(),
            created_at: Unixtime(1_692_000_000),
            kind: EventKind::TextNote,
            content: "Hey man, this rocks! Please reply for a test.".to_string(),
            tags: vec![],
        };

        let gift_wrap = signer1
            .giftwrap(pre.clone(), signer2.public_key())
            .await
            .unwrap();
        let rumor = signer2.unwrap_giftwrap(&gift_wrap).await.unwrap();
        let output_pre: PreEventV3 = rumor.into();

        assert_eq!(pre, output_pre);
    }

    #[test]
    fn test_a_tags_as_replies() {
        let raw = r#"{"id":"d4fb3aeae033baa4a9504027bff8fd065ba1bbd635c501a5e4f8c7ab0bd37c34","pubkey":"7bdef7be22dd8e59f4600e044aa53a1cf975a9dc7d27df5833bc77db784a5805","created_at":1716980987,"kind":1,"sig":"903ae95893082835a42706eda1328ea85a8bf6fbb172bb2f8696b66fccfebfae8756992894a0fb7bb592cb3f78939bdd5fac4cd1eb49138cbf3ea8069574a1dc","content":"The article is interesting, but why compiling everything when configuring meta tags in dist/index.html is sufficient? (like you did in the first version, if I'm not wrong)\nOne main selling point of Oracolo is that it does not require complex server side setup.\n\n> Every time you access the web page, the web page is compiled\n\nThis is not technically correct :)\nJavaScript code is not compiled, it is simply executed; it fetches Nostr data and so builds the page.","tags":[["p","b12b632c887f0c871d140d37bcb6e7c1e1a80264d0b7de8255aa1951d9e1ff79"],["a","30023:b12b632c887f0c871d140d37bcb6e7c1e1a80264d0b7de8255aa1951d9e1ff79:1716928135712","","root"],["r","index.html"]]}"#;
        let event: EventV3 = serde_json::from_str(raw).unwrap();
        if let Some(parent) = event.replies_to() {
            assert!(matches!(parent, EventReference::Addr(_)));
        } else {
            panic!("a tag reply not recognized");
        }
    }
}
