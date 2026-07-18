use std::fmt::Write as _;

use crate::rng::pseudostep;

pub const SEED_SPACE: i64 = 2_318_107_019_761;
const SEED_CHARS: &[u8; 35] = b"123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const ID_COEFF: [i64; 8] = [
    66_231_629_136,
    1_892_332_261,
    54_066_636,
    1_544_761,
    44_136,
    1_261,
    36,
    1,
];

#[derive(Clone, Debug)]
pub struct Seed {
    seed: [i16; 8],
    length: usize,
    cache: [[f64; 48]; 8],
    cache_valid: [u64; 8],
}

impl Default for Seed {
    fn default() -> Self {
        Self {
            seed: [-1; 8],
            length: 0,
            cache: [[0.0; 48]; 8],
            cache_valid: [0; 8],
        }
    }
}

impl From<&str> for Seed {
    fn from(seed: &str) -> Self {
        let mut out = Self::default();
        let bytes = seed.as_bytes();
        out.length = bytes.len().min(8);
        for i in 0..out.length {
            out.seed[out.length - 1 - i] = char_seed(bytes[i]);
        }
        out
    }
}

impl Seed {
    pub fn from_id(mut id: i64) -> Self {
        id = id.rem_euclid(SEED_SPACE);
        let mut out = Self::default();
        for i in 0..8 {
            if id > 0 {
                out.length += 1;
                out.seed[i] = ((id - 1) / ID_COEFF[i]) as i16;
                id -= 1 + i64::from(out.seed[i]) * ID_COEFF[i];
            } else {
                out.seed[i] = -1;
            }
        }
        out
    }

    pub fn id(&self) -> i64 {
        let mut id = 0_i64;
        for i in 0..8 {
            if self.seed[i] >= 0 {
                id += ID_COEFF[i] * i64::from(self.seed[i]) + 1;
            }
        }
        id
    }

    pub(crate) fn next(&mut self) {
        if self.length < 8 {
            self.seed[self.length] = 0;
            self.length += 1;
            return;
        }

        let mut i = 7_i32;
        while i >= 0 {
            self.cache_valid[i as usize] = 0;
            if self.seed[i as usize] == 34 {
                self.seed[i as usize] = -1;
                self.length -= 1;
            } else {
                self.seed[i as usize] += 1;
                break;
            }
            i -= 1;
        }
    }

    pub(crate) fn next_and_pseudohash_zero(&mut self) -> f64 {
        const CACHE_KEY: usize = 7;
        const CACHE_BIT: u64 = 1 << CACHE_KEY;

        // A full-length non-carry changes only row 7, so row 6/key 7 is the exact
        // input to the final position-1 pseudostep.
        if self.length == 8 && self.seed[7] != 34 && self.cache_valid[6] & CACHE_BIT != 0 {
            self.seed[7] += 1;
            self.cache_valid[7] = 0;
            let hash = pseudostep(self.seed_char(7), 1, self.cache[6][CACHE_KEY]);
            self.cache[7][CACHE_KEY] = hash;
            self.cache_valid[7] = CACHE_BIT;
            return hash;
        }

        self.next();
        self.pseudohash(0)
    }

    pub(crate) fn pseudohash(&mut self, prefix_length: usize) -> f64 {
        if self.length == 0 {
            return 1.0;
        }

        let cache_key = prefix_length + self.length - 1;
        if cache_key >= self.cache[0].len() {
            return self.pseudohash_uncached(prefix_length);
        }

        let cache_bit = 1_u64 << cache_key;
        if self.cache_valid[self.length - 1] & cache_bit == 0 {
            let mut i = self.length as i32 - 2;
            while i >= 0 && self.cache_valid[i as usize] & cache_bit == 0 {
                i -= 1;
            }
            if i == -1 {
                self.cache[0][cache_key] =
                    pseudostep(self.seed_char(0), prefix_length + self.length, 1.0);
                self.cache_valid[0] |= cache_bit;
                i = 0;
            }
            for j in (i as usize + 1)..self.length {
                self.cache[j][cache_key] = pseudostep(
                    self.seed_char(j),
                    prefix_length + self.length - j,
                    self.cache[j - 1][cache_key],
                );
                self.cache_valid[j] |= cache_bit;
            }
        }
        self.cache[self.length - 1][cache_key]
    }

    fn pseudohash_uncached(&self, prefix_length: usize) -> f64 {
        let mut num = 1.0;
        for j in 0..self.length {
            num = pseudostep(self.seed_char(j), prefix_length + self.length - j, num);
        }
        num
    }

    fn seed_char(&self, index: usize) -> u8 {
        let idx = self.seed[index];
        if idx < 0 {
            return b'?';
        }
        SEED_CHARS[idx as usize]
    }
}

impl std::fmt::Display for Seed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for i in (0..8).rev() {
            if self.seed[i] != -1 {
                let idx = self.seed[i] as usize;
                let ch = SEED_CHARS.get(idx).copied().unwrap_or(b'?') as char;
                f.write_char(ch)?;
            }
        }
        Ok(())
    }
}

fn char_seed(byte: u8) -> i16 {
    match byte {
        b'1'..=b'9' => i16::from(byte - b'1'),
        b'A'..=b'Z' => i16::from(byte - b'A' + 9),
        _ => -1,
    }
}

#[cfg(test)]
mod tests {
    use super::Seed;

    fn assert_cache_matches_uncached(seed: &mut Seed) {
        let last_cached_prefix = 48_usize.saturating_sub(seed.length);
        for prefix_length in [0, 1, last_cached_prefix, last_cached_prefix + 1, 64] {
            let expected = seed.pseudohash_uncached(prefix_length).to_bits();
            assert_eq!(seed.pseudohash(prefix_length).to_bits(), expected);
            assert_eq!(seed.pseudohash(prefix_length).to_bits(), expected);
        }
    }

    #[test]
    fn cache_validity_survives_carry_wrap_and_growth() {
        let mut seed = Seed::from("ZYZZZZZZ");
        for expected in ["ZYZZZZZZ", "ZZZZZZZ", "1ZZZZZZZ"] {
            assert_eq!(seed.to_string(), expected);
            assert_cache_matches_uncached(&mut seed);
            seed.next();
        }

        let mut seed = Seed::from("ZZZZZZZZ");
        for expected in ["ZZZZZZZZ", "", "1"] {
            assert_eq!(seed.to_string(), expected);
            assert_cache_matches_uncached(&mut seed);
            seed.next();
        }
    }

    #[test]
    fn fused_next_hash_matches_separate_operations() {
        for start in ["", "1", "Z", "11111111", "Z1111111", "ZZ111111", "ZZZZZZZZ"] {
            for warm_cache in [false, true] {
                let mut expected = Seed::from(start);
                let mut actual = expected.clone();
                if warm_cache {
                    expected.pseudohash(0);
                    actual.pseudohash(0);
                }

                for _ in 0..128 {
                    expected.next();
                    let expected_hash = expected.pseudohash(0);
                    let actual_hash = actual.next_and_pseudohash_zero();
                    assert_eq!(actual.to_string(), expected.to_string(), "start={start:?}");
                    assert_eq!(
                        actual_hash.to_bits(),
                        expected_hash.to_bits(),
                        "start={start:?}"
                    );
                    assert_cache_matches_uncached(&mut actual);
                    assert_cache_matches_uncached(&mut expected);
                }
            }
        }
    }
}
