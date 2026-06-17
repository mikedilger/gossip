use derive_more::{AsMut, AsRef, Deref, Display, From, Into};
use serde::{Deserialize, Serialize};
#[cfg(feature = "speedy")]
use speedy::{Readable, Writable};
use std::ops::{Add, Sub};
use std::time::Duration;

/// An integer count of the number of seconds from 1st January 1970.
/// This does not count any of the leap seconds that have occurred, it
/// simply presumes UTC never had leap seconds; yet it is well known
/// and well understood.
#[derive(
    AsMut,
    AsRef,
    Clone,
    Copy,
    Debug,
    Deref,
    Deserialize,
    Display,
    Eq,
    From,
    Into,
    Ord,
    PartialEq,
    PartialOrd,
    Serialize,
)]
#[cfg_attr(feature = "speedy", derive(Readable, Writable))]
pub struct Unixtime(pub i64);

impl Unixtime {
    /// Get the current unixtime (depends on the system clock being accurate)
    pub fn now() -> Unixtime {
        Unixtime(std::time::UNIX_EPOCH.elapsed().unwrap().as_secs() as i64)
    }

    // Mock data for testing
    #[allow(dead_code)]
    pub(crate) fn mock() -> Unixtime {
        Unixtime(1668572286)
    }
}

impl Add<Duration> for Unixtime {
    type Output = Self;

    fn add(self, rhs: Duration) -> Self::Output {
        Unixtime(self.0 + rhs.as_secs() as i64)
    }
}

impl Sub<Duration> for Unixtime {
    type Output = Self;

    fn sub(self, rhs: Duration) -> Self::Output {
        Unixtime(self.0 - rhs.as_secs() as i64)
    }
}

impl Sub<Unixtime> for Unixtime {
    type Output = Duration;

    fn sub(self, rhs: Unixtime) -> Self::Output {
        Duration::from_secs((self.0 - rhs.0).unsigned_abs())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    test_serde! {Unixtime, test_unixtime_serde}

    #[test]
    fn test_print_now() {
        println!("NOW: {}", Unixtime::now());
    }

    #[test]
    fn test_unixtime_math() {
        let now = Unixtime::now();
        let fut = now + Duration::from_secs(70);
        assert!(fut > now);
        assert_eq!(fut.0 - now.0, 70);
        let back = fut - Duration::from_secs(70);
        assert_eq!(now, back);
        assert_eq!(now - back, Duration::ZERO);
    }
}
