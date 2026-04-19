//! Namecoin NIP-05 resolution via ElectrumX.
//!
//! Provides censorship-resistant NIP-05 identity verification using the
//! Namecoin blockchain. Users can set their `nip05` field to a `.bit` domain
//! (e.g. `alice@example.bit`) or a direct Namecoin name (`d/example`,
//! `id/alice`) and gossip will resolve the pubkey mapping via ElectrumX
//! instead of HTTPS.
//!
//! Ported from the notedeck Namecoin module (PR #1314), originally derived
//! from Amethyst's Namecoin NIP-05 feature.

pub mod cache;
pub mod electrumx;
pub mod identifier;

use std::sync::Arc;

use nostr_types::PublicKey;
use parking_lot::Mutex as PMutex;

use self::cache::NamecoinLookupCache;
use self::electrumx::{default_servers, ElectrumxServer};
use self::identifier::NamecoinIdentifier;

/// Why a Namecoin resolution failed.
#[derive(Debug, Clone)]
pub enum NamecoinLookupError {
    /// The name does not exist on the blockchain.
    NameNotFound,
    /// The name is past its 36000-block expiry.
    NameExpired,
    /// All configured ElectrumX servers were unreachable or errored.
    ServersUnreachable(String),
    /// Value parse failure.
    Parse(String),
}

impl std::fmt::Display for NamecoinLookupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NameNotFound => write!(f, "Name not found"),
            Self::NameExpired => write!(f, "Name expired"),
            Self::ServersUnreachable(s) => write!(f, "Servers unreachable: {s}"),
            Self::Parse(s) => write!(f, "Parse error: {s}"),
        }
    }
}

impl std::error::Error for NamecoinLookupError {}

/// The resolved Namecoin identity for UI / caller display.
#[derive(Debug, Clone)]
pub struct NamecoinResolveResult {
    pub pubkey: PublicKey,
    pub identifier: NamecoinIdentifier,
}

/// High-level resolver. Cheap to clone (internal Arc).
#[derive(Clone)]
pub struct NamecoinResolver {
    inner: Arc<Inner>,
}

struct Inner {
    cache: PMutex<NamecoinLookupCache>,
    servers: PMutex<Vec<ElectrumxServer>>,
}

impl Default for NamecoinResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl NamecoinResolver {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                cache: PMutex::new(NamecoinLookupCache::new()),
                servers: PMutex::new(default_servers()),
            }),
        }
    }

    /// Override the ElectrumX server list.
    pub fn set_servers(&self, servers: Vec<ElectrumxServer>) {
        *self.inner.servers.lock() = servers;
    }

    /// Returns true if `input` looks like any supported Namecoin identifier.
    pub fn is_namecoin_identifier(input: &str) -> bool {
        NamecoinIdentifier::is_namecoin_identifier(input)
    }

    /// Resolve a Namecoin identifier to a Nostr pubkey.
    ///
    /// Cached lookups return immediately; network lookups are performed on a
    /// blocking threadpool via `tokio::task::spawn_blocking` so this future
    /// cooperates with the async runtime.
    pub async fn resolve(
        &self,
        input: &str,
    ) -> Result<NamecoinResolveResult, NamecoinLookupError> {
        let Some(identifier) = NamecoinIdentifier::parse(input) else {
            return Err(NamecoinLookupError::NameNotFound);
        };

        let cache_key = input.to_string();

        // Check cache first
        if let Some(entry) = self.inner.cache.lock().get(&cache_key) {
            if let Some(pk) = entry.pubkey {
                return Ok(NamecoinResolveResult {
                    pubkey: pk,
                    identifier,
                });
            }
            if let Some(err) = entry.error {
                return Err(err);
            }
        }

        let servers = self.inner.servers.lock().clone();
        let id_for_task = identifier.clone();

        let result = tokio::task::spawn_blocking(move || resolve_identifier(&servers, &id_for_task))
            .await
            .map_err(|e| NamecoinLookupError::ServersUnreachable(format!("join: {e}")))?;

        // Cache both success and failure
        self.inner.cache.lock().insert(cache_key, result.clone());

        result.map(|pubkey| NamecoinResolveResult { pubkey, identifier })
    }

    /// Verify that a NIP-05 `.bit` address resolves to the given pubkey on-chain.
    ///
    /// Returns `Ok(true)` if the on-chain record matches; `Ok(false)` if it
    /// exists but points to a different pubkey; `Err(_)` if the lookup failed.
    pub async fn verify_nip05(
        &self,
        nip05: &str,
        pubkey: PublicKey,
    ) -> Result<bool, NamecoinLookupError> {
        let resolved = self.resolve(nip05).await?;
        Ok(resolved.pubkey == pubkey)
    }
}

/// Resolve a parsed identifier via ElectrumX.
fn resolve_identifier(
    servers: &[ElectrumxServer],
    id: &NamecoinIdentifier,
) -> Result<PublicKey, NamecoinLookupError> {
    tracing::info!(
        "Namecoin: resolving '{}' (local_part: '{}')",
        id.name,
        id.local_part
    );
    let result = electrumx::name_show(servers, &id.name);

    match result {
        Ok(name_result) => {
            tracing::info!(
                "Namecoin: got value for '{}' at height {}: {}",
                id.name,
                name_result.height,
                &name_result.value
            );
            match extract_pubkey_from_value(&name_result.value, &id.local_part) {
                Some(pk) => {
                    tracing::info!("Namecoin: resolved '{}' → {}", id.name, pk.as_hex_string());
                    Ok(pk)
                }
                None => {
                    tracing::warn!(
                        "Namecoin: no pubkey found for local_part '{}' in value: {}",
                        id.local_part,
                        &name_result.value
                    );
                    Err(NamecoinLookupError::NameNotFound)
                }
            }
        }
        Err(e) => {
            tracing::warn!("Namecoin: resolution failed for '{}': {}", id.name, e);
            Err(match e {
                electrumx::ElectrumxError::NameNotFound => NamecoinLookupError::NameNotFound,
                electrumx::ElectrumxError::NameExpired => NamecoinLookupError::NameExpired,
                electrumx::ElectrumxError::ParseError(s) => NamecoinLookupError::Parse(s),
                other => NamecoinLookupError::ServersUnreachable(other.to_string()),
            })
        }
    }
}

/// Extract a Nostr pubkey from a Namecoin name's on-chain JSON value.
///
/// Supports two value formats:
///
/// **Simple format** (pubkey directly in `nostr` field):
/// ```json
/// {"nostr": "hexencodedpubkey"}
/// ```
///
/// **Extended NIP-05-like format** (with names mapping):
/// ```json
/// {"nostr": {"names": {"alice": "hexencodedpubkey", "_": "hexdefault"}}}
/// ```
///
/// For root lookups (local_part == "_"), falls back to the first available
/// entry if `_` is not present (fix from Amethyst PR #1771).
fn extract_pubkey_from_value(value: &str, local_part: &str) -> Option<PublicKey> {
    let json: serde_json::Value = serde_json::from_str(value).ok()?;

    let nostr_field = json.get("nostr")?;

    // Simple format: {"nostr": "hexkey"}
    if let Some(hex_str) = nostr_field.as_str() {
        return pubkey_from_hex(hex_str);
    }

    // Extended format: {"nostr": {"names": {"user": "hexkey"}}}
    if let Some(nostr_obj) = nostr_field.as_object() {
        if let Some(names) = nostr_obj.get("names").and_then(|n| n.as_object()) {
            // Try the requested local part first
            if let Some(pk_val) = names.get(local_part) {
                return pubkey_from_json_value(pk_val);
            }

            // Try "_" fallback for non-root lookups
            if local_part != "_" {
                if let Some(pk_val) = names.get("_") {
                    return pubkey_from_json_value(pk_val);
                }
            } else {
                // Root lookup: fall back to first available entry when "_" is
                // absent (Amethyst PR #1771 fix).
                if let Some((_, first_val)) = names.iter().next() {
                    return pubkey_from_json_value(first_val);
                }
            }
        }
    }

    None
}

fn pubkey_from_hex(hex_str: &str) -> Option<PublicKey> {
    PublicKey::try_from_hex_string(hex_str, false).ok()
}

fn pubkey_from_json_value(val: &serde_json::Value) -> Option<PublicKey> {
    val.as_str().and_then(pubkey_from_hex)
}

/// The process-wide default resolver. Shared across the UI and the nip05
/// verification path.
pub static NAMECOIN_RESOLVER: once_cell::sync::Lazy<NamecoinResolver> =
    once_cell::sync::Lazy::new(NamecoinResolver::new);

#[cfg(test)]
mod tests {
    use super::*;

    // A known-valid secp256k1 x-only pubkey for test purposes.
    // `PublicKey::try_from_hex_string` with `false` (don't verify) will accept
    // arbitrary 32-byte hex; we use distinguishable values.
    fn hex32(byte: u8) -> String {
        hex::encode(vec![byte; 32])
    }

    #[test]
    fn test_extract_simple_format_invalid_length() {
        let value = r#"{"nostr": "abcdef"}"#;
        assert!(extract_pubkey_from_value(value, "_").is_none());
    }

    #[test]
    fn test_extract_simple_format_valid() {
        let hex32 = hex32(0xaa);
        let value = format!(r#"{{"nostr": "{}"}}"#, hex32);
        let pk = extract_pubkey_from_value(&value, "_");
        assert!(pk.is_some());
    }

    #[test]
    fn test_extract_extended_format() {
        let hex32 = hex32(0xbb);
        let value = format!(r#"{{"nostr": {{"names": {{"m": "{}"}}}}}}"#, hex32);

        // Lookup by name
        let pk = extract_pubkey_from_value(&value, "m");
        assert!(pk.is_some());

        // Root lookup should fall back to first entry (PR #1771 fix)
        let pk_root = extract_pubkey_from_value(&value, "_");
        assert!(pk_root.is_some());
    }

    #[test]
    fn test_extract_with_underscore_key() {
        let hex_a = hex32(0x11);
        let hex_b = hex32(0x22);
        let value = format!(
            r#"{{"nostr": {{"names": {{"_": "{}", "alice": "{}"}}}}}}"#,
            hex_a, hex_b
        );

        let pk_root = extract_pubkey_from_value(&value, "_");
        assert!(pk_root.is_some());
        assert_eq!(pk_root.unwrap().as_hex_string(), hex_a);

        let pk_alice = extract_pubkey_from_value(&value, "alice");
        assert!(pk_alice.is_some());
        assert_eq!(pk_alice.unwrap().as_hex_string(), hex_b);
    }

    #[test]
    fn test_non_root_no_fallback() {
        // Non-root lookups must NOT fall back to the first entry.
        let hex = hex32(0x33);
        let value = format!(r#"{{"nostr": {{"names": {{"m": "{}"}}}}}}"#, hex);

        let pk = extract_pubkey_from_value(&value, "nonexistent");
        assert!(pk.is_none());
    }
}
