//! Namecoin identifier parsing and detection.
//!
//! Supports the following formats:
//! - `alice@example.bit` → NIP-05 style, resolves `d/example` for `alice` entry
//! - `example.bit` → Bare domain, resolves `d/example` for root `_` entry
//! - `d/example` → Direct domain namespace lookup
//! - `id/alice` → Direct identity namespace lookup

/// A parsed Namecoin identifier ready for resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamecoinIdentifier {
    /// The Namecoin name to look up (e.g., `d/testls`, `id/alice`)
    pub name: String,
    /// The local part to resolve within the name's JSON value.
    /// `_` for root lookups, otherwise the user portion.
    pub local_part: String,
}

impl NamecoinIdentifier {
    /// Parse a string into a Namecoin identifier, if it matches any supported format.
    pub fn parse(input: &str) -> Option<Self> {
        let input = input.trim();
        if input.is_empty() {
            return None;
        }

        // Format: d/name or id/name (direct namespace)
        if let Some(name) = input.strip_prefix("d/") {
            if !name.is_empty() && is_valid_name(name) {
                return Some(Self {
                    name: input.to_string(),
                    local_part: "_".to_string(),
                });
            }
        }
        if let Some(name) = input.strip_prefix("id/") {
            if !name.is_empty() && is_valid_name(name) {
                return Some(Self {
                    name: input.to_string(),
                    local_part: "_".to_string(),
                });
            }
        }

        // Format: user@domain.bit or domain.bit
        if input.ends_with(".bit") {
            if let Some(at_pos) = input.find('@') {
                // user@domain.bit
                let user = &input[..at_pos];
                let domain = &input[at_pos + 1..];
                let domain_name = domain.strip_suffix(".bit")?;
                if !user.is_empty()
                    && is_valid_local_part(user)
                    && !domain_name.is_empty()
                    && is_valid_name(domain_name)
                {
                    return Some(Self {
                        name: format!("d/{domain_name}"),
                        local_part: user.to_string(),
                    });
                }
            } else {
                // bare domain.bit
                let domain_name = input.strip_suffix(".bit")?;
                if !domain_name.is_empty() && is_valid_name(domain_name) {
                    return Some(Self {
                        name: format!("d/{domain_name}"),
                        local_part: "_".to_string(),
                    });
                }
            }
        }

        None
    }

    /// Returns true if the given string is any form of Namecoin identifier.
    pub fn is_namecoin_identifier(input: &str) -> bool {
        Self::parse(input).is_some()
    }

    /// Returns true if this is a root lookup (local_part == "_").
    pub fn is_root(&self) -> bool {
        self.local_part == "_"
    }
}

/// Check if a name component is valid (alphanumeric, hyphens, underscores, dots).
fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

/// Check if a local part (user portion of `user@domain.bit`) is valid.
/// Allows alphanumeric characters, hyphens, underscores, and dots.
fn is_valid_local_part(local: &str) -> bool {
    !local.is_empty()
        && local
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_direct_domain() {
        let id = NamecoinIdentifier::parse("d/testls").unwrap();
        assert_eq!(id.name, "d/testls");
        assert_eq!(id.local_part, "_");
    }

    #[test]
    fn test_parse_direct_identity() {
        let id = NamecoinIdentifier::parse("id/alice").unwrap();
        assert_eq!(id.name, "id/alice");
        assert_eq!(id.local_part, "_");
    }

    #[test]
    fn test_parse_nip05_bit() {
        let id = NamecoinIdentifier::parse("m@testls.bit").unwrap();
        assert_eq!(id.name, "d/testls");
        assert_eq!(id.local_part, "m");
    }

    #[test]
    fn test_parse_bare_domain() {
        let id = NamecoinIdentifier::parse("testls.bit").unwrap();
        assert_eq!(id.name, "d/testls");
        assert_eq!(id.local_part, "_");
    }

    #[test]
    fn test_is_namecoin_identifier() {
        assert!(NamecoinIdentifier::is_namecoin_identifier("d/testls"));
        assert!(NamecoinIdentifier::is_namecoin_identifier("id/alice"));
        assert!(NamecoinIdentifier::is_namecoin_identifier("testls.bit"));
        assert!(NamecoinIdentifier::is_namecoin_identifier("m@testls.bit"));

        assert!(!NamecoinIdentifier::is_namecoin_identifier(
            "user@example.com"
        ));
        assert!(!NamecoinIdentifier::is_namecoin_identifier("npub1abc"));
        assert!(!NamecoinIdentifier::is_namecoin_identifier("hello"));
        assert!(!NamecoinIdentifier::is_namecoin_identifier(""));
    }

    #[test]
    fn test_invalid_names() {
        assert!(NamecoinIdentifier::parse("d/").is_none());
        assert!(NamecoinIdentifier::parse("id/").is_none());
        assert!(NamecoinIdentifier::parse(".bit").is_none());
        assert!(NamecoinIdentifier::parse("@.bit").is_none());
    }

    #[test]
    fn test_invalid_local_part() {
        // Local part with spaces or special characters should be rejected
        assert!(NamecoinIdentifier::parse("bad user@testls.bit").is_none());
        assert!(NamecoinIdentifier::parse("bad!user@testls.bit").is_none());
        assert!(NamecoinIdentifier::parse("user name@testls.bit").is_none());
        // Valid local parts should still work
        assert!(NamecoinIdentifier::parse("valid-user@testls.bit").is_some());
        assert!(NamecoinIdentifier::parse("user_name@testls.bit").is_some());
        assert!(NamecoinIdentifier::parse("user.name@testls.bit").is_some());
    }

    #[test]
    fn test_root_detection() {
        let root = NamecoinIdentifier::parse("d/testls").unwrap();
        assert!(root.is_root());

        let non_root = NamecoinIdentifier::parse("m@testls.bit").unwrap();
        assert!(!non_root.is_root());
    }
}
