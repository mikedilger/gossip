use crate::error::Error;
use serde::{Deserialize, Serialize};
#[cfg(feature = "speedy")]
use speedy::{Readable, Writable};
use std::fmt;

/// A string that is supposed to represent a URL but which might be invalid
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, PartialOrd, Serialize, Ord)]
#[cfg_attr(feature = "speedy", derive(Readable, Writable))]
pub struct UncheckedUrl(pub String);

impl fmt::Display for UncheckedUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl UncheckedUrl {
    /// Create an UncheckedUrl from a &str
    // note - this from_str cannot error, so we don't impl std::str::FromStr which by
    //        all rights should be called TryFromStr anyway
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> UncheckedUrl {
        UncheckedUrl(s.to_owned())
    }

    /// Create an UncheckedUrl from a String
    pub fn from_string(s: String) -> UncheckedUrl {
        UncheckedUrl(s)
    }

    /// As &str
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// As nrelay
    pub fn as_bech32_string(&self) -> String {
        bech32::encode::<bech32::Bech32>(*crate::HRP_NRELAY, self.0.as_bytes()).unwrap()
    }

    /// Import from a bech32 encoded string ("nrelay")
    pub fn try_from_bech32_string(s: &str) -> Result<UncheckedUrl, Error> {
        let data = bech32::decode(s)?;
        if data.0 != *crate::HRP_NRELAY {
            Err(Error::WrongBech32(
                crate::HRP_NRELAY.to_lowercase(),
                data.0.to_lowercase(),
            ))
        } else {
            let s = std::str::from_utf8(&data.1)?.to_owned();
            Ok(UncheckedUrl(s))
        }
    }

    // Mock data for testing
    #[allow(dead_code)]
    pub(crate) fn mock() -> UncheckedUrl {
        UncheckedUrl("/home/user/file.txt".to_string())
    }
}

/// A String representing a valid URL with an authority present including an
/// Internet based host.
///
/// We don't serialize/deserialize these directly, see `UncheckedUrl` for that
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[cfg_attr(feature = "speedy", derive(Readable, Writable))]
pub struct Url(String);

impl fmt::Display for Url {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Url {
    /// Create a new Url from an UncheckedUrl
    pub fn try_from_unchecked_url(u: &UncheckedUrl) -> Result<Url, Error> {
        Url::try_from_str(&u.0)
    }

    /// Create a new Url from a string
    pub fn try_from_str(s: &str) -> Result<Url, Error> {
        // We use the url crate to parse and normalize
        let url = url::Url::parse(s.trim())?;

        if !url.has_authority() {
            return Err(Error::InvalidUrlMissingAuthority);
        }

        if let Some(host) = url.host() {
            match host {
                url::Host::Domain(_) => {
                    // Strange that we can't access as a string
                    let s = format!("{host}");
                    if s != s.trim() || s.starts_with("localhost") {
                        return Err(Error::InvalidUrlHost(s));
                    }
                }
                url::Host::Ipv4(addr) => {
                    let addrx = core_net::Ipv4Addr::from(addr.octets());
                    if !addrx.is_global() {
                        return Err(Error::InvalidUrlHost(format!("{host}")));
                    }
                }
                url::Host::Ipv6(addr) => {
                    let addrx = core_net::Ipv6Addr::from(addr.octets());
                    if !addrx.is_global() {
                        return Err(Error::InvalidUrlHost(format!("{host}")));
                    }
                }
            }
        } else {
            return Err(Error::InvalidUrlHost("".to_string()));
        }

        Ok(Url(url.as_str().to_owned()))
    }

    /// Convert into a UncheckedUrl
    pub fn to_unchecked_url(&self) -> UncheckedUrl {
        UncheckedUrl(self.0.clone())
    }

    /// As &str
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Into String
    pub fn into_string(self) -> String {
        self.0
    }

    /// As url crate Url
    pub fn as_url_crate_url(&self) -> url::Url {
        url::Url::parse(&self.0).unwrap()
    }

    // Mock data for testing
    #[allow(dead_code)]
    pub(crate) fn mock() -> Url {
        Url("http://example.com/avatar.png".to_string())
    }
}

/// A Url validated as a nostr relay url in canonical form
/// We don't serialize/deserialize these directly, see `UncheckedUrl` for that
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[cfg_attr(feature = "speedy", derive(Readable, Writable))]
pub struct RelayUrl(String);

impl fmt::Display for RelayUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl RelayUrl {
    /// Create a new RelayUrl from a Url
    pub fn try_from_url(u: &Url) -> Result<RelayUrl, Error> {
        // Verify we aren't looking at a comma-separated-list of URLs
        // (technically they might be valid URLs but just about 100% of the time
        // it's somebody else's bad data)
        if u.0.contains(",wss://") || u.0.contains(",ws://") {
            return Err(Error::Url(format!(
                "URL appears to be a list of multiple URLs: {}",
                u.0
            )));
        }

        let url = url::Url::parse(&u.0)?;

        // Verify the scheme is websockets
        if url.scheme() != "wss" && url.scheme() != "ws" {
            return Err(Error::InvalidUrlScheme(url.scheme().to_owned()));
        }

        // Verify host is some
        if !url.has_host() {
            return Err(Error::Url(format!("URL has no host: {}", u.0)));
        }

        Ok(RelayUrl(url.as_str().to_owned()))
    }

    /// Create a new RelayUrl from an UncheckedUrl
    pub fn try_from_unchecked_url(u: &UncheckedUrl) -> Result<RelayUrl, Error> {
        Self::try_from_str(&u.0)
    }

    /// Construct a new RelayUrl from a Url
    pub fn try_from_str(s: &str) -> Result<RelayUrl, Error> {
        let url = Url::try_from_str(s)?;
        RelayUrl::try_from_url(&url)
    }

    /// Convert into a Url
    // fixme should be 'as_url'
    pub fn to_url(&self) -> Url {
        Url(self.0.clone())
    }

    /// As url crate Url
    pub fn as_url_crate_url(&self) -> url::Url {
        url::Url::parse(&self.0).unwrap()
    }

    /// As nrelay
    pub fn as_bech32_string(&self) -> String {
        bech32::encode::<bech32::Bech32>(*crate::HRP_NRELAY, self.0.as_bytes()).unwrap()
    }

    /// Convert into a UncheckedUrl
    pub fn to_unchecked_url(&self) -> UncheckedUrl {
        UncheckedUrl(self.0.clone())
    }

    /// Host
    pub fn host(&self) -> String {
        self.as_url_crate_url().host_str().unwrap().to_owned()
    }

    /// As &str
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Into String
    pub fn into_string(self) -> String {
        self.0
    }

    // Mock data for testing
    #[allow(dead_code)]
    pub(crate) fn mock() -> Url {
        Url("wss://example.com".to_string())
    }
}

impl TryFrom<Url> for RelayUrl {
    type Error = Error;

    fn try_from(u: Url) -> Result<RelayUrl, Error> {
        RelayUrl::try_from_url(&u)
    }
}

impl TryFrom<&Url> for RelayUrl {
    type Error = Error;

    fn try_from(u: &Url) -> Result<RelayUrl, Error> {
        RelayUrl::try_from_url(u)
    }
}

impl From<RelayUrl> for Url {
    fn from(ru: RelayUrl) -> Url {
        ru.to_url()
    }
}

/// A canonical URL representing just a relay's origin
/// (without path/query/fragment or username/password)
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, PartialOrd, Serialize, Ord)]
#[cfg_attr(feature = "speedy", derive(Readable, Writable))]
pub struct RelayOrigin(String);

impl RelayOrigin {
    /// Convert a RelayUrl into a RelayOrigin
    pub fn from_relay_url(url: RelayUrl) -> RelayOrigin {
        let mut xurl = url::Url::parse(url.as_str()).unwrap();
        xurl.set_fragment(None);
        xurl.set_query(None);
        xurl.set_path("/");
        let _ = xurl.set_username("");
        let _ = xurl.set_password(None);
        RelayOrigin(xurl.into())
    }

    /// Construct a new RelayOrigin from a string
    pub fn try_from_str(s: &str) -> Result<RelayOrigin, Error> {
        let url = RelayUrl::try_from_str(s)?;
        Ok(RelayOrigin::from_relay_url(url))
    }

    /// Create a new Url from an UncheckedUrl
    pub fn try_from_unchecked_url(u: &UncheckedUrl) -> Result<RelayOrigin, Error> {
        let relay_url = RelayUrl::try_from_str(&u.0)?;
        Ok(relay_url.into())
    }

    /// Convert this RelayOrigin into a RelayUrl
    pub fn into_relay_url(self) -> RelayUrl {
        RelayUrl(self.0)
    }

    /// Get a RelayUrl matching this RelayOrigin
    pub fn as_relay_url(&self) -> RelayUrl {
        RelayUrl(self.0.clone())
    }

    /// Convert into a UncheckedUrl
    pub fn to_unchecked_url(&self) -> UncheckedUrl {
        UncheckedUrl(self.0.clone())
    }

    /// As &str
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Into String
    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for RelayOrigin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<RelayUrl> for RelayOrigin {
    fn from(ru: RelayUrl) -> RelayOrigin {
        RelayOrigin::from_relay_url(ru)
    }
}

impl From<RelayOrigin> for RelayUrl {
    fn from(ru: RelayOrigin) -> RelayUrl {
        ru.into_relay_url()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    test_serde! {UncheckedUrl, test_unchecked_url_serde}

    #[test]
    fn test_url_case() {
        let url = Url::try_from_str("Wss://MyRelay.example.COM/PATH?Query").unwrap();
        assert_eq!(url.as_str(), "wss://myrelay.example.com/PATH?Query");
    }

    #[test]
    fn test_relay_url_slash() {
        let input = "Wss://MyRelay.example.COM";
        let url = RelayUrl::try_from_str(input).unwrap();
        assert_eq!(url.as_str(), "wss://myrelay.example.com/");
    }

    #[test]
    fn test_relay_origin() {
        let input = "wss://user:pass@filter.nostr.wine:444/npub1234?x=y#z";
        let relay_url = RelayUrl::try_from_str(input).unwrap();
        let origin: RelayOrigin = relay_url.into();
        assert_eq!(origin.as_str(), "wss://filter.nostr.wine:444/");
    }
}
