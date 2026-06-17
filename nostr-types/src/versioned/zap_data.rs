use crate::types::{EventReference, Id, MilliSatoshi, PublicKey};

/// Data about a Zap
#[derive(Clone, Debug)]
pub struct ZapDataV2 {
    /// The event that was zapped. If missing we can't use the zap receipt event.
    pub zapped_event: EventReference,

    /// The amount that the event was zapped
    pub amount: MilliSatoshi,

    /// The public key of the person who received the zap
    pub payee: PublicKey,

    /// The public key of the person who paid the zap, if it was in the receipt
    pub payer: PublicKey,

    /// The public key of the zap provider, for verification purposes
    pub provider_pubkey: PublicKey,
}

/// Data about a Zap
#[derive(Clone, Debug, Copy)]
pub struct ZapDataV1 {
    /// The event that was zapped
    pub id: Id,

    /// The amount that the event was zapped
    pub amount: MilliSatoshi,

    /// The public key of the person who provided the zap
    pub pubkey: PublicKey,

    /// The public key of the zap provider, for verification purposes
    pub provider_pubkey: PublicKey,
}
