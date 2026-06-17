use crate::versioned::relay_information_document1::{FeeV1, RelayFeesV1, RelayRetentionV1};
use crate::versioned::relay_information_document2::{
    RelayInformationDocumentV2, RelayLimitationV2,
};

/// Relay limitations
pub type RelayLimitation = RelayLimitationV2;

/// Relay retention
pub type RelayRetention = RelayRetentionV1;

/// Fee
pub type Fee = FeeV1;

/// Relay fees
pub type RelayFees = RelayFeesV1;

/// Relay information document as described in NIP-11, supplied by a relay
pub type RelayInformationDocument = RelayInformationDocumentV2;
