use crate::rng::pseudostep;
use std::fmt::Write as _;

pub const SEED_SPACE: i64 = 2_318_107_019_761;
pub const SEED_CHARS: &[u8; 35] = b"123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
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
}

impl Default for Seed {
    fn default() -> Self {
        Self {
            seed: [-1; 8],
            length: 0,
            cache: [[-1.0; 48]; 8],
        }
    }
}

impl Seed {
    pub fn from_str(seed: &str) -> Self {
        let mut out = Self::default();
        let bytes = seed.as_bytes();
        out.length = bytes.len().min(8);
        for i in 0..out.length {
            out.seed[out.length - 1 - i] = char_seed(bytes[i]);
        }
        out
    }

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

    pub fn next(&mut self) {
        if self.length < 8 {
            self.seed[self.length] = 0;
            self.length += 1;
            return;
        }

        let mut i = 7_i32;
        while i >= 0 {
            self.cache[i as usize] = [-1.0; 48];
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

    pub fn pseudohash(&mut self, prefix_length: usize) -> f64 {
        if self.length == 0 {
            return 1.0;
        }

        let cache_key = prefix_length + self.length - 1;
        if cache_key >= self.cache[0].len() {
            return self.pseudohash_uncached(prefix_length);
        }

        if self.cache[self.length - 1][cache_key] == -1.0 {
            let mut i = self.length as i32 - 2;
            while i >= 0 && self.cache[i as usize][cache_key] == -1.0 {
                i -= 1;
            }
            if i == -1 {
                self.cache[0][cache_key] =
                    pseudostep(self.seed_char(0), prefix_length + self.length, 1.0);
                i = 0;
            }
            for j in (i as usize + 1)..self.length {
                self.cache[j][cache_key] = pseudostep(
                    self.seed_char(j),
                    prefix_length + self.length - j,
                    self.cache[j - 1][cache_key],
                );
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
