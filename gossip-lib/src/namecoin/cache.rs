//! LRU cache with TTL for Namecoin name lookups.
//!
//! Caches resolved pubkeys to avoid repeated ElectrumX queries.
//! Entries expire after 1 hour.
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use nostr_types::PublicKey;

use super::NamecoinLookupError;

const CACHE_TTL: Duration = Duration::from_secs(3600); // 1 hour
const MAX_CACHE_SIZE: usize = 256;

#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub pubkey: Option<PublicKey>,
    pub error: Option<NamecoinLookupError>,
    pub inserted_at: Instant,
}

impl CacheEntry {
    pub fn is_expired(&self) -> bool {
        self.inserted_at.elapsed() >= CACHE_TTL
    }
}

pub struct NamecoinLookupCache {
    entries: HashMap<String, CacheEntry>,
    /// Insertion order for LRU eviction (front = oldest)
    insertion_order: VecDeque<String>,
}

impl Default for NamecoinLookupCache {
    fn default() -> Self {
        Self::new()
    }
}

impl NamecoinLookupCache {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            insertion_order: VecDeque::new(),
        }
    }

    /// Get a cached entry if it exists and hasn't expired.
    pub fn get(&self, key: &str) -> Option<CacheEntry> {
        self.entries
            .get(key)
            .filter(|entry| !entry.is_expired())
            .cloned()
    }

    /// Insert or update a cache entry.
    ///
    /// Automatically evicts expired entries and the oldest entry when at
    /// capacity. Uses `VecDeque` for O(1) front eviction.
    pub fn insert(&mut self, key: String, result: Result<PublicKey, NamecoinLookupError>) {
        // Evict expired entries first to reclaim space
        self.evict_expired();

        // Evict oldest if still at capacity
        while self.entries.len() >= MAX_CACHE_SIZE {
            if let Some(oldest) = self.insertion_order.pop_front() {
                self.entries.remove(&oldest);
            } else {
                break;
            }
        }

        // Remove old entry from insertion order if updating
        self.insertion_order.retain(|k| k != &key);

        let (pubkey, error) = match result {
            Ok(pk) => (Some(pk), None),
            Err(e) => (None, Some(e)),
        };

        self.entries.insert(
            key.clone(),
            CacheEntry {
                pubkey,
                error,
                inserted_at: Instant::now(),
            },
        );
        self.insertion_order.push_back(key);
    }

    /// Remove expired entries from both `entries` and `insertion_order`.
    pub fn evict_expired(&mut self) {
        self.entries.retain(|_, entry| !entry.is_expired());
        self.insertion_order
            .retain(|key| self.entries.contains_key(key));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_pubkey() -> PublicKey {
        // 32 bytes of 0x01 — a syntactically valid secp256k1 x-only pubkey is
        // not required for cache tests, but PublicKey::try_from_hex_string
        // won't accept arbitrary bytes. Use a known-good hex constant.
        let hex = "0000000000000000000000000000000000000000000000000000000000000001";
        PublicKey::try_from_hex_string(hex, false).unwrap()
    }

    #[test]
    fn test_cache_insert_and_get() {
        let mut cache = NamecoinLookupCache::new();
        let pk = fake_pubkey();

        cache.insert("d/test".to_string(), Ok(pk));

        let entry = cache.get("d/test");
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().pubkey, Some(pk));
    }

    #[test]
    fn test_cache_miss() {
        let cache = NamecoinLookupCache::new();
        assert!(cache.get("d/nonexistent").is_none());
    }

    #[test]
    fn test_cache_negative_result() {
        let mut cache = NamecoinLookupCache::new();
        cache.insert(
            "d/expired".to_string(),
            Err(NamecoinLookupError::NameNotFound),
        );

        let entry = cache.get("d/expired");
        assert!(entry.is_some());
        assert!(entry.unwrap().pubkey.is_none());
    }
}
