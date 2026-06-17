use crate::Error;
use std::ops::AddAssign;

/// HyperLogLog approximate counting mechanism
///
/// This uses a fixed set of 256 buckets (k=8, M=256), each holding a count up to 255.
/// https://algo.inria.fr/flajolet/Publications/FlFuGaMe07.pdf
///
/// This is used for NIP-45 PR #1561
#[derive(Debug, Clone, Copy)]
pub struct Hll8([u8; 256]);

impl Default for Hll8 {
    fn default() -> Self {
        Self::new()
    }
}

impl Hll8 {
    /// Create a new Hll8
    pub fn new() -> Hll8 {
        Hll8([0; 256])
    }

    /// Import from a (hex) string
    pub fn from_hex_string(s: &str) -> Result<Hll8, Error> {
        let vec: Vec<u8> = hex::decode(s)?;
        let arr: [u8; 256] = vec.try_into().map_err(|_| Error::InvalidHll)?;
        Ok(Hll8(arr))
    }

    /// Export to a (hex) string
    pub fn to_hex_string(&self) -> String {
        hex::encode(self.0)
    }

    /// Clear to zero counts
    pub fn clear(&mut self) {
        for i in 0..=255 {
            self.0[i] = 0
        }
    }

    /// Add an element to the count by value
    /// Returns false on error (e.g. offset is out of range)
    pub fn add_element(&mut self, input: &[u8; 32], offset: usize) -> Result<(), Error> {
        if offset >= 24 {
            return Err(Error::OutOfRange(offset));
        }

        // Use the byte at that offset as the bucket
        let index = input[offset];

        // Count zeros after that offset
        let zeros = {
            let mut zeros: u8 = 0;
            #[allow(clippy::needless_range_loop)]
            for i in offset + 1..=31 {
                let leading = input[i].leading_zeros();
                zeros += leading as u8;
                if leading < 8 {
                    break;
                }
            }
            zeros
        };

        let rho = zeros + 1;
        self.add_element_inner(index, rho);

        Ok(())
    }

    /// Add an element to the count by index and rho (position of the first 1)
    pub fn add_element_inner(&mut self, index: u8, rho: u8) {
        if rho > self.0[index as usize] {
            self.0[index as usize] = rho;
        }
    }

    /// Compute the approximate count
    pub fn estimate_count(&self) -> usize {
        // 2007 paper calls this 'V';  2016 paper calls this 'z'
        let zero_count = self.0.iter().filter(|&c| *c == 0).count();

        // Sum over the reciprocals (SUM of 2^-m)
        let mut sum: f64 = 0.0;
        for i in 0..=255 {
            let power: usize = 1 << self.0[i];
            sum += 1.0 / (power as f64);
        }

        let estimate = estimate_hyperloglog(sum, zero_count);
        // estimate_fiatjaf(sum, zero_count);

        estimate.round() as usize
    }
}

impl AddAssign for Hll8 {
    fn add_assign(&mut self, other: Self) {
        for i in 0..=255 {
            if other.0[i] > self.0[i] {
                self.0[i] = other.0[i];
            }
        }
    }
}

// The number of buckets we use = 2**k  ('m' from the papers)
const M: f64 = 256.0;

// The correction factor, 'α' from the paper for k=8,M=256
const ALPHA: f64 = 0.7213 / (1.0 + 1.079 / M);

// 2^32 as a floating point number
const TWO32: f64 = 4_294_967_296.0;

// HyperLogLog++ Threshold, when to switch from linear counting for M=256 (k=8)
#[allow(dead_code)]
const THRESHOLD: f64 = 220.0;

fn estimate_hyperloglog(sum: f64, zero_count: usize) -> f64 {
    let mut estimate = ALPHA * (M * M) / sum;
    if estimate <= (5.0 / 2.0) * M {
        // 640
        if zero_count != 0 {
            estimate = M * (M / (zero_count as f64)).ln(); // linear
        }
    } else if estimate > (1.0 / 30.0) * TWO32 {
        // 143165576
        estimate = -TWO32 * (1.0 - estimate / TWO32).log2();
    };
    estimate
}

#[allow(dead_code)]
fn estimate_fiatjaf(sum: f64, zero_count: usize) -> f64 {
    let estimate = ALPHA * M * M / sum;
    if zero_count == 0 {
        return estimate;
    }
    let linear = M * (M / zero_count as f64).ln();
    if linear <= THRESHOLD {
        linear
    } else if estimate < 256.0 * 3.0 {
        // 768
        linear
    } else {
        estimate
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_hll8() {
        use rand::prelude::IndexedRandom;
        use rand_core::RngCore;
        let mut rng = rand::rng();

        // Create 2500 well known different keys
        let mut input: Vec<[u8; 32]> = Vec::new();
        for _ in 0..2500 {
            let mut key = [0u8; 32];
            rng.fill_bytes(&mut key);
            input.push(key);
        }

        for numkeys in [
            1, 2, 3, 5, 9, 15, 33, 50, 89, 115, 150, 195, 260, 420, 1000, 2500,
        ] {
            // Test Hll8 using these keys
            let mut h = Hll8::new();
            for _ in 0..=100_000 {
                let random_key = input[0..numkeys].choose(&mut rng).unwrap();
                h.add_element(random_key, 16).unwrap();
            }

            // Even though we added 100,000 elements, there are only numkey distinct ones.

            println!("Actual: {}  Estimate: {}", numkeys, h.estimate_count());
        }
    }
}
