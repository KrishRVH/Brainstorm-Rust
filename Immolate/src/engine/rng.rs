use crate::rng::{LuaRandom, fract, pseudohash_from_bytes, round13};
use crate::seed::Seed;

#[derive(Clone, Copy, Debug)]
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
    dynamic_nodes: Vec<DynamicNode>,
}

#[derive(Clone, Debug)]
struct DynamicNode {
    key: String,
    value: f64,
}

impl Default for RngState {
    fn default() -> Self {
        Self {
            nodes: [0.0; KEY_COUNT],
            initialized_mask: 0,
            dynamic_nodes: Vec::with_capacity(8),
        }
    }
}

impl RngState {
    pub fn clear(&mut self) {
        self.initialized_mask = 0;
        self.dynamic_nodes.clear();
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

    pub fn randint_dynamic(
        &mut self,
        key: &str,
        seed: &mut Seed,
        hashed_seed: f64,
        min: i32,
        max: i32,
    ) -> i32 {
        let node = self.get_dynamic_node(key, seed, hashed_seed);
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

    fn get_dynamic_node(&mut self, key: &str, seed: &mut Seed, hashed_seed: f64) -> f64 {
        let position = self.dynamic_nodes.iter().position(|node| node.key == key);
        let value = if let Some(position) = position {
            &mut self.dynamic_nodes[position].value
        } else {
            let value = initial_node(seed, key.as_bytes());
            let position = self.dynamic_nodes.len();
            self.dynamic_nodes.push(DynamicNode {
                key: key.to_owned(),
                value,
            });
            &mut self.dynamic_nodes[position].value
        };
        advance_node(value, hashed_seed)
    }
}

fn initial_node(seed: &mut Seed, key: &[u8]) -> f64 {
    let seed_hash = seed.pseudohash(key.len());
    pseudohash_from_bytes(key, seed_hash)
}

fn advance_node(node: &mut f64, hashed_seed: f64) -> f64 {
    *node = round13(fract(*node * 1.72431234 + 2.134453429141));
    (*node + hashed_seed) / 2.0
}
