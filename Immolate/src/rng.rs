const K_MAX_U64: u64 = u64::MAX;
const K_DBL_EXPO: u64 = 0x7FF0_0000_0000_0000;
const K_DBL_MANT: u64 = 0x000F_FFFF_FFFF_FFFF;
const K_DBL_MANT_SIZE: u64 = 52;
const K_DBL_EXPO_SIZE: u64 = 11;
const K_DBL_EXPO_BIAS: u64 = 1023;
const PI_HASH: f64 = 3.141592653589793116;

#[derive(Clone, Copy, Debug)]
pub(crate) struct LuaRandom {
    state: [u64; 4],
}

impl Default for LuaRandom {
    fn default() -> Self {
        Self::new(0.0)
    }
}

impl LuaRandom {
    pub(crate) fn new(seed: f64) -> Self {
        let mut d = seed;
        let mut r = 0x1109_0601_u64;
        let mut state = [0_u64; 4];
        for slot in &mut state {
            let m = 1_u64 << (r & 255);
            r >>= 8;
            d = d * std::f64::consts::PI + std::f64::consts::E;
            let mut bits = d.to_bits();
            if bits < m {
                bits += m;
            }
            *slot = bits;
        }
        let mut rng = Self { state };
        for _ in 0..10 {
            rng.randint_raw();
        }
        rng
    }

    pub(crate) fn randint_raw(&mut self) -> u64 {
        let mut result = 0_u64;

        let mut z = self.state[0];
        z = (((z << 31) ^ z) >> 45) ^ ((z & (K_MAX_U64 << 1)) << 18);
        result ^= z;
        self.state[0] = z;

        z = self.state[1];
        z = (((z << 19) ^ z) >> 30) ^ ((z & (K_MAX_U64 << 6)) << 28);
        result ^= z;
        self.state[1] = z;

        z = self.state[2];
        z = (((z << 24) ^ z) >> 48) ^ ((z & (K_MAX_U64 << 9)) << 7);
        result ^= z;
        self.state[2] = z;

        z = self.state[3];
        z = (((z << 21) ^ z) >> 39) ^ ((z & (K_MAX_U64 << 17)) << 8);
        result ^= z;
        self.state[3] = z;

        result
    }

    pub(crate) fn randdblmem(&mut self) -> u64 {
        (self.randint_raw() & K_DBL_MANT) | 1.0_f64.to_bits()
    }

    pub(crate) fn random(&mut self) -> f64 {
        f64::from_bits(self.randdblmem()) - 1.0
    }

    pub(crate) fn randint(&mut self, min: i32, max: i32) -> i32 {
        (self.random() * f64::from(max - min + 1)) as i32 + min
    }
}

pub(crate) fn fract(x: f64) -> f64 {
    let x_int = x.to_bits();
    let expo = (x_int & K_DBL_EXPO) >> K_DBL_MANT_SIZE;
    if expo < K_DBL_EXPO_BIAS {
        return x;
    }
    if expo == (1_u64 << K_DBL_EXPO_SIZE) - 1 {
        return f64::NAN;
    }
    let expo_biased = expo - K_DBL_EXPO_BIAS;
    if expo_biased >= K_DBL_MANT_SIZE {
        return 0.0;
    }
    let mant = x_int & K_DBL_MANT;
    let frac_mant = mant & ((1_u64 << (K_DBL_MANT_SIZE - expo_biased)) - 1);
    if frac_mant == 0 {
        return 0.0;
    }
    let frac_lzcnt = u64::from(frac_mant.leading_zeros()) - (64 - K_DBL_MANT_SIZE);
    let res_expo = (expo - frac_lzcnt - 1) << K_DBL_MANT_SIZE;
    let res_mant = (frac_mant << (frac_lzcnt + 1)) & K_DBL_MANT;
    f64::from_bits(res_expo | res_mant)
}

pub(crate) fn pseudohash_from(bytes: &str, num: f64) -> f64 {
    pseudohash_from_bytes(bytes.as_bytes(), num)
}

pub(crate) fn pseudohash_from_bytes(bytes: &[u8], mut num: f64) -> f64 {
    for i in (0..bytes.len()).rev() {
        let pos = i + 1;
        num = fract(1.1239285023 / num * f64::from(bytes[i]) * PI_HASH + PI_HASH * pos as f64);
    }
    num
}

pub(crate) fn pseudostep(byte: u8, pos: usize, num: f64) -> f64 {
    fract(1.1239285023 / num * f64::from(byte) * PI_HASH + PI_HASH * pos as f64)
}

pub(crate) fn ante_to_string(ante: i32) -> String {
    if ante < 10 {
        return ((b'0' + ante as u8) as char).to_string();
    }
    let tens = (b'0' + (ante / 10) as u8) as char;
    let ones = (b'0' + (ante % 10) as u8) as char;
    let mut out = String::with_capacity(2);
    out.push(tens);
    out.push(ones);
    out
}

fn next_down_for_positive_hash(x: f64) -> f64 {
    if x == 0.0 {
        return -f64::from_bits(1);
    }
    if x.is_nan() {
        return x;
    }
    if x > 0.0 {
        f64::from_bits(x.to_bits() - 1)
    } else {
        f64::from_bits(x.to_bits() + 1)
    }
}

pub(crate) fn round13(x: f64) -> f64 {
    const INV_PREC: f64 = 10_000_000_000_000.0;
    const TWO_INV_PREC: f64 = 8192.0;
    const FIVE_INV_PREC: f64 = 1_220_703_125.0;

    let normal_case = (x * INV_PREC).round() / INV_PREC;
    let previous_case = (next_down_for_positive_hash(x) * INV_PREC).round() / INV_PREC;
    if normal_case == previous_case {
        return normal_case;
    }
    let truncated = fract(x * TWO_INV_PREC) * FIVE_INV_PREC;
    if fract(truncated) >= 0.5 {
        return ((x * INV_PREC).floor() + 1.0) / INV_PREC;
    }
    (x * INV_PREC).floor() / INV_PREC
}
