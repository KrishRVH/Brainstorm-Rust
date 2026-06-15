use crate::item::{
    CARDS, COMMON_JOKERS, ITEM_COUNT, Item, LEGENDARY_JOKERS, PACKS, Pack, RARE_JOKERS, SPECTRALS,
    TAGS, TAROTS, UNCOMMON_JOKERS, VOUCHERS, WeightedItem,
};

pub const POOL_COMMON: u8 = 1 << 0;
pub const POOL_UNCOMMON: u8 = 1 << 1;
pub const POOL_RARE: u8 = 1 << 2;
pub const POOL_LEGENDARY: u8 = 1 << 3;

pub const TAG_POOL: &[Item] = &TAGS;
pub const VOUCHER_POOL: &[Item] = &VOUCHERS;
pub const TAROT_POOL: &[Item] = &TAROTS;
pub const SPECTRAL_POOL: &[Item] = &SPECTRALS;
pub const COMMON_JOKER_POOL: &[Item] = &COMMON_JOKERS;
pub const UNCOMMON_JOKER_POOL: &[Item] = &UNCOMMON_JOKERS;
pub const RARE_JOKER_POOL: &[Item] = &RARE_JOKERS;
pub const LEGENDARY_JOKER_POOL: &[Item] = &LEGENDARY_JOKERS;

#[derive(Clone, Copy, Debug)]
pub struct ShopRates {
    pub joker: f64,
    pub tarot: f64,
    pub planet: f64,
    pub playing_card: f64,
    pub spectral: f64,
}

impl ShopRates {
    pub const fn total(self) -> f64 {
        self.joker + self.tarot + self.planet + self.playing_card + self.spectral
    }
}

#[derive(Clone, Debug)]
pub struct Locks {
    locked: [bool; ITEM_COUNT],
}

impl Default for Locks {
    fn default() -> Self {
        let mut out = Self {
            locked: [false; ITEM_COUNT],
        };
        out.init_ante1();
        out
    }
}

impl Locks {
    pub fn for_deck(deck: Item) -> Self {
        let mut out = Self::default();
        match deck {
            Item::Magic_Deck => out.activate_voucher(Item::Crystal_Ball),
            Item::Nebula_Deck => out.activate_voucher(Item::Telescope),
            Item::Zodiac_Deck => {
                out.activate_voucher(Item::Tarot_Merchant);
                out.activate_voucher(Item::Planet_Merchant);
                out.activate_voucher(Item::Overstock);
            },
            _ => {},
        }
        out
    }

    pub fn is_locked(&self, item: Item) -> bool {
        item.idx() < self.locked.len() && self.locked[item.idx()]
    }

    pub fn lock(&mut self, item: Item) {
        if item.idx() < self.locked.len() {
            self.locked[item.idx()] = true;
        }
    }

    pub fn unlock(&mut self, item: Item) {
        if item.idx() < self.locked.len() {
            self.locked[item.idx()] = false;
        }
    }

    fn init_ante1(&mut self) {
        for item in [
            Item::The_Mouth,
            Item::The_Fish,
            Item::The_Wall,
            Item::The_House,
            Item::The_Mark,
            Item::The_Wheel,
            Item::The_Arm,
            Item::The_Water,
            Item::The_Needle,
            Item::The_Flint,
            Item::Negative_Tag,
            Item::Standard_Tag,
            Item::Meteor_Tag,
            Item::Buffoon_Tag,
            Item::Handy_Tag,
            Item::Garbage_Tag,
            Item::Ethereal_Tag,
            Item::Top_up_Tag,
            Item::Orbital_Tag,
            Item::The_Tooth,
            Item::The_Eye,
            Item::The_Plant,
            Item::The_Serpent,
            Item::The_Ox,
        ] {
            self.lock(item);
        }
    }

    fn activate_voucher(&mut self, voucher: Item) {
        self.lock(voucher);
        for pair in VOUCHERS.chunks_exact(2) {
            if pair[0] == voucher {
                self.unlock(pair[1]);
            }
        }
    }
}

pub fn target_joker_pools(target: Item) -> u8 {
    let mut pools = 0;
    if COMMON_JOKERS.contains(&target) {
        pools |= POOL_COMMON;
    }
    if UNCOMMON_JOKERS.contains(&target) {
        pools |= POOL_UNCOMMON;
    }
    if RARE_JOKERS.contains(&target) {
        pools |= POOL_RARE;
    }
    if LEGENDARY_JOKERS.contains(&target) {
        pools |= POOL_LEGENDARY;
    }
    pools
}

pub fn pack_info(pack: Item) -> Pack {
    if PACKS.iter().any(|entry| entry.item == pack) {
        crate::instance::pack_info(pack)
    } else {
        Pack {
            pack_type: Item::RETRY,
            size: 0,
            choices: 0,
        }
    }
}

pub fn is_buffoon_pack(pack: Item) -> bool {
    matches!(
        pack,
        Item::Buffoon_Pack | Item::Jumbo_Buffoon_Pack | Item::Mega_Buffoon_Pack
    )
}

pub fn is_arcana_pack(pack: Item) -> bool {
    matches!(
        pack,
        Item::Arcana_Pack | Item::Jumbo_Arcana_Pack | Item::Mega_Arcana_Pack
    )
}

pub fn is_spectral_pack(pack: Item) -> bool {
    matches!(
        pack,
        Item::Spectral_Pack | Item::Jumbo_Spectral_Pack | Item::Mega_Spectral_Pack
    )
}

pub fn is_soulable_pack(pack: Item) -> bool {
    is_arcana_pack(pack) || is_spectral_pack(pack)
}

pub fn is_ante1_locked_tag(item: Item) -> bool {
    matches!(
        item,
        Item::Negative_Tag
            | Item::Standard_Tag
            | Item::Meteor_Tag
            | Item::Buffoon_Tag
            | Item::Handy_Tag
            | Item::Garbage_Tag
            | Item::Ethereal_Tag
            | Item::Top_up_Tag
            | Item::Orbital_Tag
    )
}

pub fn shop_rates_for_deck(deck: Item) -> ShopRates {
    let mut rates = ShopRates {
        joker: 20.0,
        tarot: 4.0,
        planet: 4.0,
        playing_card: 0.0,
        spectral: 0.0,
    };
    if deck == Item::Ghost_Deck {
        rates.spectral = 2.0;
    }
    if deck == Item::Zodiac_Deck {
        rates.tarot = 9.6;
        rates.planet = 9.6;
    }
    rates
}

pub fn shop_item_type(rates: ShopRates, mut poll: f64) -> Item {
    if poll < rates.joker {
        return Item::T_Joker;
    }
    poll -= rates.joker;
    if poll < rates.tarot {
        return Item::T_Tarot;
    }
    poll -= rates.tarot;
    if poll < rates.planet {
        return Item::T_Planet;
    }
    poll -= rates.planet;
    if poll < rates.playing_card {
        return Item::T_Playing_Card;
    }
    Item::T_Spectral
}

pub fn card_face_and_suit(index: usize) -> (bool, usize) {
    let rank_idx = (CARDS[index].idx() - Item::C_2.idx()) % 13;
    let suit_idx = (CARDS[index].idx() - Item::C_2.idx()) / 13;
    let is_face = matches!(rank_idx, 9..=11);
    (is_face, suit_idx)
}

pub fn weighted_packs() -> &'static [WeightedItem; 16] {
    &PACKS
}
