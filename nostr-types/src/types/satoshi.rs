use derive_more::{AsMut, AsRef, Deref, Display, From, Into};
use serde::{Deserialize, Serialize};
#[cfg(feature = "speedy")]
use speedy::{Readable, Writable};
use std::ops::Add;

/// Bitcoin amount measured in millisatoshi
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
pub struct MilliSatoshi(pub u64);

impl MilliSatoshi {
    // Mock data for testing
    #[allow(dead_code)]
    pub(crate) fn mock() -> MilliSatoshi {
        MilliSatoshi(15423000)
    }
}

impl Add<MilliSatoshi> for MilliSatoshi {
    type Output = Self;

    fn add(self, rhs: MilliSatoshi) -> Self::Output {
        MilliSatoshi(self.0 + rhs.0)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    test_serde! {MilliSatoshi, test_millisatoshi_serde}

    #[test]
    fn test_millisatoshi_math() {
        let a = MilliSatoshi(15000);
        let b = MilliSatoshi(3000);
        let c = a + b;
        assert_eq!(c.0, 18000);
    }
}
