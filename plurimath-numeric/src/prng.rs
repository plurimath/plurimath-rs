//! Philox 4x32-10 counter-based generator (Salmon et al., SC11).
//!
//! Output is a pure function of `(key, counter)`, so replicate blocks seeded
//! by disjoint counter ranges give identical streams whether they are run
//! sequentially or on any number of threads. This module is independent of
//! the prescribed `gum-ref` (Wichmann-Hill) oracle, which it does not touch.

const M0: u32 = 0xD251_1F53;
const M1: u32 = 0xCD9E_8D57;
const W0: u32 = 0x9E37_79B9;
const W1: u32 = 0xBB67_AE85;
const ROUNDS: usize = 10;

/// Philox 4x32-10 keyed by a 64-bit seed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Philox4x32 {
    key: [u32; 2],
}

fn mulhilo(a: u32, b: u32) -> (u32, u32) {
    let p = u64::from(a) * u64::from(b);
    ((p >> 32) as u32, p as u32)
}

impl Philox4x32 {
    pub fn new(seed: u64) -> Self {
        Self {
            key: [seed as u32, (seed >> 32) as u32],
        }
    }

    /// The four 32-bit words for a 128-bit counter (little-endian words).
    pub fn block(&self, counter: u128) -> [u32; 4] {
        let mut c = [
            counter as u32,
            (counter >> 32) as u32,
            (counter >> 64) as u32,
            (counter >> 96) as u32,
        ];
        let mut k = self.key;
        for round in 0..ROUNDS {
            let (hi0, lo0) = mulhilo(M0, c[0]);
            let (hi1, lo1) = mulhilo(M1, c[2]);
            c = [hi1 ^ c[1] ^ k[0], lo1, hi0 ^ c[3] ^ k[1], lo0];
            if round + 1 < ROUNDS {
                k = [k[0].wrapping_add(W0), k[1].wrapping_add(W1)];
            }
        }
        c
    }

    /// Two uniform doubles in [0, 1) with 53 random bits each, from one counter value.
    pub fn uniform_pair(&self, counter: u128) -> [f64; 2] {
        let w = self.block(counter);
        let to_unit = |hi: u32, lo: u32| {
            let bits = (u64::from(hi) << 32 | u64::from(lo)) >> 11;
            bits as f64 * (1.0 / 9_007_199_254_740_992.0)
        };
        [to_unit(w[0], w[1]), to_unit(w[2], w[3])]
    }

    /// Fill `out` with uniforms for the counter range starting at `first_counter`;
    /// element `i` depends only on `first_counter + i / 2`, never on how the
    /// range was split into blocks.
    pub fn fill_uniform(&self, first_counter: u128, out: &mut [f64]) {
        for (i, pair) in out.chunks_mut(2).enumerate() {
            let u = self.uniform_pair(first_counter + i as u128);
            pair.copy_from_slice(&u[..pair.len()]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(seed_key: [u32; 2], ctr: [u32; 4]) -> [u32; 4] {
        let g = Philox4x32 { key: seed_key };
        let counter = ctr
            .iter()
            .rev()
            .fold(0u128, |a, &w| a << 32 | u128::from(w));
        g.block(counter)
    }

    // Known-answer vectors from the Random123 reference (kat_vectors).
    #[test]
    fn random123_known_answers() {
        assert_eq!(
            words([0, 0], [0; 4]),
            [0x6627e8d5, 0xe169c58d, 0xbc57ac4c, 0x9b00dbd8]
        );
        assert_eq!(
            words([u32::MAX; 2], [u32::MAX; 4]),
            [0x408f276d, 0x41c83b0e, 0xa20bc7c6, 0x6d5451fd]
        );
        assert_eq!(
            words(
                [0xa4093822, 0x299f31d0],
                [0x243f6a88, 0x85a308d3, 0x13198a2e, 0x03707344]
            ),
            [0xd16cfe09, 0x94fdcceb, 0x5001e420, 0x24126ea1]
        );
    }

    #[test]
    fn disjoint_ranges_match_single_stream() {
        let g = Philox4x32::new(20260101);
        let mut whole = vec![0.0; 1000];
        g.fill_uniform(0, &mut whole);
        let mut parts = vec![0.0; 1000];
        std::thread::scope(|s| {
            for (n, chunk) in parts.chunks_mut(100).enumerate() {
                s.spawn(move || g.fill_uniform((n * 50) as u128, chunk));
            }
        });
        assert_eq!(whole, parts);
        assert!(whole.iter().all(|u| (0.0..1.0).contains(u)));
    }

    #[test]
    fn different_seeds_differ() {
        assert_ne!(Philox4x32::new(1).block(0), Philox4x32::new(2).block(0));
    }
}
