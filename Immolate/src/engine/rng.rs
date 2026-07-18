use crate::rng::{LuaRandom, fract, pseudohash_from_bytes, round13};
use crate::seed::Seed;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RngKey {
    Tag1,
    Voucher1,
    ShopPack1,
    Cdt1,
    RarityShop1,
    RarityBuffoon1,
    JokerCommonShop1,
    JokerUncommonShop1,
    JokerRareShop1,
    JokerCommonBuffoon1,
    JokerUncommonBuffoon1,
    JokerRareBuffoon1,
    JokerLegendary,
    SoulTarot1,
    SoulSpectral1,
    TarotArcana1,
    SpectralPack1,
    Erratic,
}

const KEY_COUNT: usize = 18;
// Lua's 0..51 draw splits the 52-bit mantissa into four sorted 13-card suits.
// Face cards occupy ranks 9..=11 inside each exact quarter-width suit interval.
const ERRATIC_MANTISSA_MASK: u64 = (1_u64 << 52) - 1;
const ERRATIC_SUIT_INTERVAL: u64 = 1_u64 << 50;
const ERRATIC_FACE_START: u64 = (9 * ERRATIC_SUIT_INTERVAL).div_ceil(13);
const ERRATIC_FACE_END: u64 = (12 * ERRATIC_SUIT_INTERVAL).div_ceil(13);

fn erratic_suit_from_mantissa(mantissa: u64) -> usize {
    (mantissa >> 50) as usize
}

fn erratic_is_face_from_mantissa(mantissa: u64) -> bool {
    let rank_mantissa = mantissa & (ERRATIC_SUIT_INTERVAL - 1);
    (ERRATIC_FACE_START..ERRATIC_FACE_END).contains(&rank_mantissa)
}

impl RngKey {
    const fn idx(self) -> usize {
        self as usize
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Tag1 => "Tag1",
            Self::Voucher1 => "Voucher1",
            Self::ShopPack1 => "shop_pack1",
            Self::Cdt1 => "cdt1",
            Self::RarityShop1 => "rarity1sho",
            Self::RarityBuffoon1 => "rarity1buf",
            Self::JokerCommonShop1 => "Joker1sho1",
            Self::JokerUncommonShop1 => "Joker2sho1",
            Self::JokerRareShop1 => "Joker3sho1",
            Self::JokerCommonBuffoon1 => "Joker1buf1",
            Self::JokerUncommonBuffoon1 => "Joker2buf1",
            Self::JokerRareBuffoon1 => "Joker3buf1",
            Self::JokerLegendary => "Joker4",
            Self::SoulTarot1 => "soul_Tarot1",
            Self::SoulSpectral1 => "soul_Spectral1",
            Self::TarotArcana1 => "Tarotar11",
            Self::SpectralPack1 => "Spectralspe1",
            Self::Erratic => "erratic",
        }
    }
}

#[derive(Clone, Debug)]
pub struct RngState {
    nodes: [f64; KEY_COUNT],
    initialized_mask: u32,
    resample_nodes: Vec<ResampleNode>,
    active_resample_nodes: usize,
}

#[derive(Clone, Copy, Debug)]
struct ResampleNode {
    key: RngKey,
    resample: u16,
    value: f64,
}

impl Default for RngState {
    fn default() -> Self {
        Self {
            nodes: [0.0; KEY_COUNT],
            initialized_mask: 0,
            resample_nodes: Vec::new(),
            active_resample_nodes: 0,
        }
    }
}

impl RngState {
    pub fn clear(&mut self) {
        self.initialized_mask = 0;
        self.active_resample_nodes = 0;
    }

    pub fn random(&mut self, key: RngKey, seed: &mut Seed, hashed_seed: f64) -> f64 {
        let node = self.get_fixed_node(key, seed, hashed_seed);
        LuaRandom::new(node).random()
    }

    pub fn randint(
        &mut self,
        key: RngKey,
        seed: &mut Seed,
        hashed_seed: f64,
        min: i32,
        max: i32,
    ) -> i32 {
        let node = self.get_fixed_node(key, seed, hashed_seed);
        LuaRandom::new(node).randint(min, max)
    }

    pub(crate) fn erratic_draws<'a>(
        &'a mut self,
        seed: &mut Seed,
        hashed_seed: f64,
    ) -> ErraticDraws<'a> {
        let idx = RngKey::Erratic.idx();
        let initialized_bit = 1_u32 << idx;
        // Keep this local: sharing the initializer outlined this hot path in release builds.
        if self.initialized_mask & initialized_bit == 0 {
            self.nodes[idx] = initial_node(seed, RngKey::Erratic.name().as_bytes());
            self.initialized_mask |= initialized_bit;
        }
        ErraticDraws {
            node: &mut self.nodes[idx],
            hashed_seed,
        }
    }

    pub fn randint_resample(
        &mut self,
        key: RngKey,
        resample: u16,
        seed: &mut Seed,
        hashed_seed: f64,
        min: i32,
        max: i32,
    ) -> i32 {
        let node = self.get_resample_node(key, resample, seed, hashed_seed);
        LuaRandom::new(node).randint(min, max)
    }

    fn get_fixed_node(&mut self, key: RngKey, seed: &mut Seed, hashed_seed: f64) -> f64 {
        let idx = key.idx();
        let initialized_bit = 1_u32 << idx;
        let node = &mut self.nodes[idx];
        if self.initialized_mask & initialized_bit == 0 {
            *node = initial_node(seed, key.name().as_bytes());
            self.initialized_mask |= initialized_bit;
        }
        advance_node(node, hashed_seed)
    }

    fn get_resample_node(
        &mut self,
        key: RngKey,
        resample: u16,
        seed: &mut Seed,
        hashed_seed: f64,
    ) -> f64 {
        let position = self.resample_nodes[..self.active_resample_nodes]
            .iter()
            .position(|node| node.key == key && node.resample == resample);
        let value = if let Some(position) = position {
            &mut self.resample_nodes[position].value
        } else {
            let value = initial_resample_node(seed, key, resample);
            let position = self.active_resample_nodes;
            self.active_resample_nodes += 1;
            if position == self.resample_nodes.len() {
                self.resample_nodes.push(ResampleNode {
                    key,
                    resample,
                    value,
                });
            } else {
                self.resample_nodes[position] = ResampleNode {
                    key,
                    resample,
                    value,
                };
            }
            &mut self.resample_nodes[position].value
        };
        advance_node(value, hashed_seed)
    }
}

pub(crate) struct ErraticDraws<'a> {
    node: &'a mut f64,
    hashed_seed: f64,
}

impl ErraticDraws<'_> {
    pub(crate) fn next_suit_index(&mut self) -> usize {
        erratic_suit_from_mantissa(self.next_mantissa())
    }

    pub(crate) fn next_is_face(&mut self) -> bool {
        erratic_is_face_from_mantissa(self.next_mantissa())
    }

    pub(crate) fn next_card_properties(&mut self) -> (bool, usize) {
        let mantissa = self.next_mantissa();
        (
            erratic_is_face_from_mantissa(mantissa),
            erratic_suit_from_mantissa(mantissa),
        )
    }

    #[inline]
    fn next_mantissa(&mut self) -> u64 {
        let node = advance_node(self.node, self.hashed_seed);
        LuaRandom::new(node).randint_raw() & ERRATIC_MANTISSA_MASK
    }
}

fn initial_node(seed: &mut Seed, key: &[u8]) -> f64 {
    let seed_hash = seed.pseudohash(key.len());
    pseudohash_from_bytes(key, seed_hash)
}

fn initial_resample_node(seed: &mut Seed, key: RngKey, resample: u16) -> f64 {
    const SUFFIX: &[u8] = b"_resample";

    let base = key.name().as_bytes();
    let mut bytes = [0_u8; 40];
    bytes[..base.len()].copy_from_slice(base);
    let mut len = base.len();
    bytes[len..len + SUFFIX.len()].copy_from_slice(SUFFIX);
    len += SUFFIX.len();

    let mut digits = [0_u8; 5];
    let mut digit_count = 0;
    let mut value = resample;
    loop {
        digits[digit_count] = b'0' + (value % 10) as u8;
        digit_count += 1;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    for digit in digits[..digit_count].iter().rev() {
        bytes[len] = *digit;
        len += 1;
    }

    initial_node(seed, &bytes[..len])
}

fn advance_node(node: &mut f64, hashed_seed: f64) -> f64 {
    *node = round13(fract(*node * 1.72431234 + 2.134453429141));
    (*node + hashed_seed) / 2.0
}

#[cfg(test)]
mod tests {
    use super::{
        ERRATIC_FACE_END, ERRATIC_FACE_START, ERRATIC_SUIT_INTERVAL, RngKey, RngState,
        erratic_is_face_from_mantissa, erratic_suit_from_mantissa,
    };
    use crate::seed::Seed;

    fn float_card_index(mantissa: u64) -> usize {
        let unit = f64::from_bits((1023_u64 << 52) | mantissa) - 1.0;
        (unit * 52.0) as usize
    }

    fn float_suit_index(mantissa: u64) -> usize {
        float_card_index(mantissa) / 13
    }

    #[test]
    fn integer_erratic_suit_index_matches_float_boundaries_and_samples() {
        const UNIT: u64 = 1_u64 << 52;

        for suit in 0_u64..=4 {
            let boundary = suit * (UNIT / 4);
            for mantissa in boundary.saturating_sub(16)..=(boundary + 16).min(UNIT - 1) {
                assert_eq!(
                    erratic_suit_from_mantissa(mantissa),
                    float_suit_index(mantissa)
                );
            }
        }

        let mut mantissa = 0x1234_5678_9abc_u64;
        for _ in 0..1_000_000 {
            mantissa ^= mantissa << 13;
            mantissa ^= mantissa >> 7;
            mantissa ^= mantissa << 17;
            mantissa &= UNIT - 1;
            assert_eq!(
                erratic_suit_from_mantissa(mantissa),
                float_suit_index(mantissa)
            );
        }
    }

    #[test]
    fn integer_erratic_face_test_matches_float_boundaries_and_samples() {
        const UNIT: u64 = 1_u64 << 52;
        for suit in 0_u64..4 {
            for boundary in [
                suit * ERRATIC_SUIT_INTERVAL + ERRATIC_FACE_START,
                suit * ERRATIC_SUIT_INTERVAL + ERRATIC_FACE_END,
            ] {
                for mantissa in boundary - 16..=boundary + 16 {
                    assert_eq!(
                        erratic_is_face_from_mantissa(mantissa),
                        matches!(float_card_index(mantissa), 9..=11 | 22..=24 | 35..=37 | 48..=50)
                    );
                }
            }
        }

        let mut mantissa = 0xfedc_ba98_7654_u64;
        for _ in 0..1_000_000 {
            mantissa ^= mantissa << 13;
            mantissa ^= mantissa >> 7;
            mantissa ^= mantissa << 17;
            mantissa &= UNIT - 1;
            assert_eq!(
                erratic_is_face_from_mantissa(mantissa),
                matches!(float_card_index(mantissa), 9..=11 | 22..=24 | 35..=37 | 48..=50)
            );
        }
    }

    #[test]
    fn erratic_cursor_matches_scalar_draws_across_its_lifecycle() {
        fn seed_and_hash(value: &str) -> (Seed, f64) {
            let mut seed = Seed::from_str(value);
            let hash = seed.pseudohash(0);
            (seed, hash)
        }

        fn next_card(state: &mut RngState, seed: &mut Seed, hash: f64) -> usize {
            state.randint(RngKey::Erratic, seed, hash, 0, 51) as usize
        }

        let (mut cursor_seed, cursor_hash) = seed_and_hash("R9TEST");
        let (mut scalar_seed, scalar_hash) = seed_and_hash("R9TEST");
        let mut cursor_state = RngState::default();
        let mut scalar_state = RngState::default();

        {
            let mut cursor = cursor_state.erratic_draws(&mut cursor_seed, cursor_hash);
            assert_eq!(
                cursor.next_suit_index(),
                next_card(&mut scalar_state, &mut scalar_seed, scalar_hash) / 13
            );
            assert_eq!(
                cursor.next_is_face(),
                matches!(
                    next_card(&mut scalar_state, &mut scalar_seed, scalar_hash),
                    9..=11 | 22..=24 | 35..=37 | 48..=50
                )
            );
            let card = next_card(&mut scalar_state, &mut scalar_seed, scalar_hash);
            assert_eq!(
                cursor.next_card_properties(),
                (
                    matches!(card, 9..=11 | 22..=24 | 35..=37 | 48..=50),
                    card / 13,
                )
            );
        }

        {
            let card = next_card(&mut scalar_state, &mut scalar_seed, scalar_hash);
            let mut resumed = cursor_state.erratic_draws(&mut cursor_seed, cursor_hash);
            assert_eq!(
                resumed.next_card_properties(),
                (
                    matches!(card, 9..=11 | 22..=24 | 35..=37 | 48..=50),
                    card / 13,
                )
            );
        }

        cursor_state.clear();
        scalar_state.clear();
        let (mut cursor_seed, cursor_hash) = seed_and_hash("NEWSEED");
        let (mut scalar_seed, scalar_hash) = seed_and_hash("NEWSEED");
        let card = next_card(&mut scalar_state, &mut scalar_seed, scalar_hash);
        assert_eq!(
            cursor_state
                .erratic_draws(&mut cursor_seed, cursor_hash)
                .next_card_properties(),
            (
                matches!(card, 9..=11 | 22..=24 | 35..=37 | 48..=50),
                card / 13,
            )
        );
    }
}
