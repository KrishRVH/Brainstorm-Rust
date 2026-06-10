use crate::item::{
    BOSSES, CARDS, COMMON_JOKERS, COMMON_JOKERS_100, Card, ENHANCEMENTS, ITEM_COUNT, Item,
    JokerData, JokerStickers, LEGENDARY_JOKERS, PACKS, PLANETS, Pack, RARE_JOKERS, RARE_JOKERS_100,
    SPECTRALS, ShopItem, TAGS, TAROTS, UNCOMMON_JOKERS, UNCOMMON_JOKERS_100, VOUCHERS,
};
use crate::rng::{LuaRandom, ante_to_string, fract, pseudohash_from, round13};
use crate::seed::Seed;

const SOURCE_SHOP: &str = "sho";
const SOURCE_ARCANA_PACK: &str = "ar1";
const SOURCE_OMEN_GLOBE: &str = "ar2";
const SOURCE_CELESTIAL_PACK: &str = "pl1";
const SOURCE_SPECTRAL_PACK: &str = "spe";
const SOURCE_STANDARD_PACK: &str = "sta";
const SOURCE_BUFFOON_PACK: &str = "buf";
const SOURCE_SOUL: &str = "sou";
const SOURCE_WRAITH: &str = "wra";
const SOURCE_RARE_TAG: &str = "rta";
const SOURCE_UNCOMMON_TAG: &str = "uta";

const RANDOM_JOKER_COMMON: &str = "Joker1";
const RANDOM_JOKER_UNCOMMON: &str = "Joker2";
const RANDOM_JOKER_RARE: &str = "Joker3";
const RANDOM_JOKER_LEGENDARY: &str = "Joker4";
const RANDOM_JOKER_RARITY: &str = "rarity";
const RANDOM_JOKER_EDITION: &str = "edi";
const RANDOM_STANDARD_HAS_ENHANCEMENT: &str = "stdset";
const RANDOM_ENHANCEMENT: &str = "Enhanced";
const RANDOM_CARD: &str = "front";
const RANDOM_STANDARD_EDITION: &str = "standard_edition";
const RANDOM_STANDARD_HAS_SEAL: &str = "stdseal";
const RANDOM_STANDARD_SEAL: &str = "stdsealtype";
const RANDOM_SHOP_PACK: &str = "shop_pack";
const RANDOM_TAROT: &str = "Tarot";
const RANDOM_SPECTRAL: &str = "Spectral";
const RANDOM_TAGS: &str = "Tag";
const RANDOM_CARD_TYPE: &str = "cdt";
const RANDOM_PLANET: &str = "Planet";
const RANDOM_VOUCHER: &str = "Voucher";
const RANDOM_SOUL: &str = "soul_";
const RANDOM_ETERNAL: &str = "stake_shop_joker_eternal";
const RANDOM_RENTAL: &str = "ssjr";
const RANDOM_ETERNAL_PERISHABLE: &str = "etperpoll";
const RANDOM_RENTAL_PACK: &str = "packssjr";
const RANDOM_ETERNAL_PERISHABLE_PACK: &str = "packetper";
const RANDOM_BOSS: &str = "boss";
const RANDOM_OMEN_GLOBE: &str = "omen_globe";

const KEY_CARD_TYPE_ANTE1: &str = "cdt1";
const KEY_SHOP_PACK_ANTE1: &str = "shop_pack1";
const KEY_TAG_ANTE1: &str = "Tag1";
const KEY_VOUCHER_ANTE1: &str = "Voucher1";
const KEY_JOKER_RARITY_SHOP_ANTE1: &str = "rarity1sho";
const KEY_JOKER_RARITY_BUFFOON_ANTE1: &str = "rarity1buf";
const KEY_JOKER_EDITION_SHOP_ANTE1: &str = "edisho1";
const KEY_JOKER_EDITION_BUFFOON_ANTE1: &str = "edibuf1";
const KEY_JOKER_COMMON_SHOP_ANTE1: &str = "Joker1sho1";
const KEY_JOKER_COMMON_BUFFOON_ANTE1: &str = "Joker1buf1";
const KEY_JOKER_UNCOMMON_SHOP_ANTE1: &str = "Joker2sho1";
const KEY_JOKER_UNCOMMON_BUFFOON_ANTE1: &str = "Joker2buf1";
const KEY_JOKER_RARE_SHOP_ANTE1: &str = "Joker3sho1";
const KEY_JOKER_RARE_BUFFOON_ANTE1: &str = "Joker3buf1";

#[derive(Clone, Copy, Debug)]
pub struct ShopInstance {
    pub joker_rate: f64,
    pub tarot_rate: f64,
    pub planet_rate: f64,
    pub playing_card_rate: f64,
    pub spectral_rate: f64,
}

impl ShopInstance {
    fn total_rate(self) -> f64 {
        self.joker_rate
            + self.tarot_rate
            + self.planet_rate
            + self.playing_card_rate
            + self.spectral_rate
    }
}

#[derive(Clone, Debug)]
struct Cache {
    nodes: Vec<CacheNode>,
    active: usize,
    generated_first_pack: bool,
}

#[derive(Clone, Debug)]
struct CacheNode {
    key: String,
    value: f64,
}

impl Cache {
    fn new() -> Self {
        Self {
            nodes: Vec::with_capacity(32),
            active: 0,
            generated_first_pack: false,
        }
    }

    fn clear(&mut self) {
        self.active = 0;
        self.generated_first_pack = false;
    }
}

#[derive(Clone, Debug)]
struct InstParams {
    deck: Item,
    stake: Item,
    showman: bool,
    version: i64,
    vouchers: [bool; 32],
}

impl Default for InstParams {
    fn default() -> Self {
        Self {
            deck: Item::Red_Deck,
            stake: Item::White_Stake,
            showman: false,
            version: 10103,
            vouchers: [false; 32],
        }
    }
}

#[derive(Clone, Debug)]
pub struct Instance {
    locked: [bool; ITEM_COUNT],
    pub seed: Seed,
    hashed_seed: f64,
    cache: Cache,
    params: InstParams,
}

impl Instance {
    pub fn new(mut seed: Seed) -> Self {
        let hashed_seed = seed.pseudohash(0);
        Self {
            locked: [false; ITEM_COUNT],
            seed,
            hashed_seed,
            cache: Cache::new(),
            params: InstParams::default(),
        }
    }

    pub fn next(&mut self) {
        self.seed.next();
        self.hashed_seed = self.seed.pseudohash(0);
        self.params = InstParams::default();
        self.cache.clear();
    }

    pub fn get_node(&mut self, id: &str) -> f64 {
        let position = self.cache.nodes[..self.cache.active]
            .iter()
            .position(|node| node.key == id);
        let node = if let Some(position) = position {
            &mut self.cache.nodes[position].value
        } else {
            let seed_hash = self.seed.pseudohash(id.len());
            let initial = pseudohash_from(id, seed_hash);
            let position = self.cache.active;
            self.cache.active += 1;
            if position == self.cache.nodes.len() {
                self.cache.nodes.push(CacheNode {
                    key: id.to_owned(),
                    value: initial,
                });
            } else {
                let node = &mut self.cache.nodes[position];
                node.key.clear();
                node.key.push_str(id);
                node.value = initial;
            }
            &mut self.cache.nodes[position].value
        };
        *node = round13(fract(*node * 1.72431234 + 2.134453429141));
        (*node + self.hashed_seed) / 2.0
    }

    pub fn random(&mut self, id: &str) -> f64 {
        let mut rng = LuaRandom::new(self.get_node(id));
        rng.random()
    }

    pub fn randint(&mut self, id: &str, min: i32, max: i32) -> i32 {
        let mut rng = LuaRandom::new(self.get_node(id));
        rng.randint(min, max)
    }

    pub fn randchoice(&mut self, id: &str, items: &[Item]) -> Item {
        let mut rng = LuaRandom::new(self.get_node(id));
        let idx = rng.randint(0, items.len() as i32 - 1) as usize;
        let item = items[idx];
        if (!self.params.showman && self.is_locked(item)) || item == Item::RETRY {
            let mut resample = 2;
            loop {
                let resample_key = format!("{id}_resample{}", ante_to_string(resample));
                let mut rng = LuaRandom::new(self.get_node(&resample_key));
                let candidate = items[rng.randint(0, items.len() as i32 - 1) as usize];
                resample += 1;
                if (candidate != Item::RETRY && !self.is_locked(candidate)) || resample > 1000 {
                    return candidate;
                }
            }
        }
        item
    }

    fn randweightedchoice(&mut self, id: &str, items: &[crate::item::WeightedItem]) -> Item {
        let mut rng = LuaRandom::new(self.get_node(id));
        let poll = rng.random() * items[0].weight;
        let mut idx = 1_usize;
        let mut weight = 0.0;
        while weight < poll {
            weight += items[idx].weight;
            idx += 1;
        }
        items[idx - 1].item
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

    pub fn is_locked(&self, item: Item) -> bool {
        item.idx() < self.locked.len() && self.locked[item.idx()]
    }

    pub fn init_locks(&mut self, ante: i32, fresh_profile: bool, fresh_run: bool) {
        for pair in VOUCHERS.chunks_exact(2) {
            self.lock(pair[1]);
        }
        for item in [
            Item::Cavendish,
            Item::Steel_Joker,
            Item::Stone_Joker,
            Item::Lucky_Cat,
            Item::Golden_Ticket,
            Item::Glass_Joker,
        ] {
            self.lock(item);
        }
        if ante < 2 {
            self.lock_many(&[
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
            ]);
        }
        if ante < 3 {
            self.lock_many(&[Item::The_Tooth, Item::The_Eye]);
        }
        if ante < 4 {
            self.lock(Item::The_Plant);
        }
        if ante < 5 {
            self.lock(Item::The_Serpent);
        }
        if ante < 6 {
            self.lock(Item::The_Ox);
        }
        if fresh_profile {
            self.lock_many(&[
                Item::Negative_Tag,
                Item::Foil_Tag,
                Item::Holographic_Tag,
                Item::Polychrome_Tag,
                Item::Rare_Tag,
                Item::Golden_Ticket,
                Item::Mr_Bones,
                Item::Acrobat,
                Item::Sock_and_Buskin,
                Item::Swashbuckler,
                Item::Troubadour,
                Item::Certificate,
                Item::Smeared_Joker,
                Item::Throwback,
                Item::Hanging_Chad,
                Item::Rough_Gem,
                Item::Bloodstone,
                Item::Arrowhead,
                Item::Onyx_Agate,
                Item::Glass_Joker,
                Item::Showman,
                Item::Flower_Pot,
                Item::Blueprint,
                Item::Wee_Joker,
                Item::Merry_Andy,
                Item::Oops_All_6s,
                Item::The_Idol,
                Item::Seeing_Double,
                Item::Matador,
                Item::Hit_the_Road,
                Item::The_Duo,
                Item::The_Trio,
                Item::The_Family,
                Item::The_Order,
                Item::The_Tribe,
                Item::Stuntman,
                Item::Invisible_Joker,
                Item::Brainstorm,
                Item::Satellite,
                Item::Shoot_the_Moon,
                Item::Drivers_License,
                Item::Cartomancer,
                Item::Astronomer,
                Item::Burnt_Joker,
                Item::Bootstraps,
                Item::Overstock_Plus,
                Item::Liquidation,
                Item::Glow_Up,
                Item::Reroll_Glut,
                Item::Omen_Globe,
                Item::Observatory,
                Item::Nacho_Tong,
                Item::Recyclomancy,
                Item::Tarot_Tycoon,
                Item::Planet_Tycoon,
                Item::Money_Tree,
                Item::Antimatter,
                Item::Illusion,
                Item::Petroglyph,
                Item::Retcon,
                Item::Palette,
            ]);
        }
        if fresh_run {
            self.lock_many(&[
                Item::Planet_X,
                Item::Ceres,
                Item::Eris,
                Item::Five_of_a_Kind,
                Item::Flush_House,
                Item::Flush_Five,
                Item::Stone_Joker,
                Item::Steel_Joker,
                Item::Glass_Joker,
                Item::Golden_Ticket,
                Item::Lucky_Cat,
                Item::Cavendish,
                Item::Overstock_Plus,
                Item::Liquidation,
                Item::Glow_Up,
                Item::Reroll_Glut,
                Item::Omen_Globe,
                Item::Observatory,
                Item::Nacho_Tong,
                Item::Recyclomancy,
                Item::Tarot_Tycoon,
                Item::Planet_Tycoon,
                Item::Money_Tree,
                Item::Antimatter,
                Item::Illusion,
                Item::Petroglyph,
                Item::Retcon,
                Item::Palette,
            ]);
        }
    }

    fn lock_many(&mut self, items: &[Item]) {
        for item in items {
            self.lock(*item);
        }
    }

    pub fn next_tarot(&mut self, source: &str, ante: i32, soulable: bool) -> Item {
        let ante_str = ante_to_string(ante);
        if soulable
            && (self.params.showman || !self.is_locked(Item::The_Soul))
            && self.random(&format!("{RANDOM_SOUL}{RANDOM_TAROT}{ante_str}")) > 0.997
        {
            return Item::The_Soul;
        }
        self.randchoice(&format!("{RANDOM_TAROT}{source}{ante_str}"), &TAROTS)
    }

    pub fn next_planet(&mut self, source: &str, ante: i32, soulable: bool) -> Item {
        let ante_str = ante_to_string(ante);
        if soulable
            && (self.params.showman || !self.is_locked(Item::Black_Hole))
            && self.random(&format!("{RANDOM_SOUL}{RANDOM_PLANET}{ante_str}")) > 0.997
        {
            return Item::Black_Hole;
        }
        self.randchoice(&format!("{RANDOM_PLANET}{source}{ante_str}"), &PLANETS)
    }

    pub fn next_spectral(&mut self, source: &str, ante: i32, soulable: bool) -> Item {
        let ante_str = ante_to_string(ante);
        if soulable {
            let mut forced = Item::RETRY;
            if (self.params.showman || !self.is_locked(Item::The_Soul))
                && self.random(&format!("{RANDOM_SOUL}{RANDOM_SPECTRAL}{ante_str}")) > 0.997
            {
                forced = Item::The_Soul;
            }
            if (self.params.showman || !self.is_locked(Item::Black_Hole))
                && self.random(&format!("{RANDOM_SOUL}{RANDOM_SPECTRAL}{ante_str}")) > 0.997
            {
                forced = Item::Black_Hole;
            }
            if forced != Item::RETRY {
                return forced;
            }
        }
        self.randchoice(&format!("{RANDOM_SPECTRAL}{source}{ante_str}"), &SPECTRALS)
    }

    pub fn next_joker(&mut self, source: &str, ante: i32, has_stickers: bool) -> JokerData {
        let ante_str = ante_to_string(ante);
        let rarity = if source == SOURCE_SOUL {
            Item::Legendary
        } else if source == SOURCE_WRAITH || source == SOURCE_RARE_TAG {
            Item::Rare
        } else if source == SOURCE_UNCOMMON_TAG {
            Item::Uncommon
        } else {
            let poll = if let Some(key) = joker_rarity_key(source, ante) {
                self.random(key)
            } else {
                self.random(&format!("{RANDOM_JOKER_RARITY}{ante_str}{source}"))
            };
            if poll > 0.95 {
                Item::Rare
            } else if poll > 0.7 {
                Item::Uncommon
            } else {
                Item::Common
            }
        };

        let edition_rate = if self.is_voucher_active(Item::Glow_Up) {
            4.0
        } else if self.is_voucher_active(Item::Hone) {
            2.0
        } else {
            1.0
        };
        let edition_poll = if let Some(key) = joker_edition_key(source, ante) {
            self.random(key)
        } else {
            self.random(&format!("{RANDOM_JOKER_EDITION}{source}{ante_str}"))
        };
        let edition = if edition_poll > 0.997 {
            Item::Negative
        } else if edition_poll > 1.0 - 0.006 * edition_rate {
            Item::Polychrome
        } else if edition_poll > 1.0 - 0.02 * edition_rate {
            Item::Holographic
        } else if edition_poll > 1.0 - 0.04 * edition_rate {
            Item::Foil
        } else {
            Item::No_Edition
        };

        let joker = match rarity {
            Item::Legendary if self.params.version > 10099 => {
                self.randchoice(RANDOM_JOKER_LEGENDARY, &LEGENDARY_JOKERS)
            },
            Item::Legendary => self.randchoice(
                &format!("{RANDOM_JOKER_LEGENDARY}{source}{ante_str}"),
                &LEGENDARY_JOKERS,
            ),
            Item::Rare if self.params.version > 10099 => {
                self.randchoice_joker_pool(RANDOM_JOKER_RARE, source, ante, &ante_str, &RARE_JOKERS)
            },
            Item::Rare => self.randchoice(
                &format!("{RANDOM_JOKER_RARE}{source}{ante_str}"),
                &RARE_JOKERS_100,
            ),
            Item::Uncommon if self.params.version > 10099 => self.randchoice_joker_pool(
                RANDOM_JOKER_UNCOMMON,
                source,
                ante,
                &ante_str,
                &UNCOMMON_JOKERS,
            ),
            Item::Uncommon => self.randchoice(
                &format!("{RANDOM_JOKER_UNCOMMON}{source}{ante_str}"),
                &UNCOMMON_JOKERS_100,
            ),
            _ if self.params.version > 10099 => self.randchoice_joker_pool(
                RANDOM_JOKER_COMMON,
                source,
                ante,
                &ante_str,
                &COMMON_JOKERS,
            ),
            _ => self.randchoice(
                &format!("{RANDOM_JOKER_COMMON}{source}{ante_str}"),
                &COMMON_JOKERS_100,
            ),
        };

        let mut stickers = JokerStickers::default();
        if has_stickers {
            if self.params.version > 10099 {
                let sticker_key = if source == SOURCE_BUFFOON_PACK {
                    RANDOM_ETERNAL_PERISHABLE_PACK
                } else {
                    RANDOM_ETERNAL_PERISHABLE
                };
                let sticker_poll = self.random(&format!("{sticker_key}{ante_str}"));
                if sticker_poll > 0.7
                    && self.params.stake >= Item::Black_Stake
                    && can_be_eternal(joker)
                {
                    stickers.eternal = true;
                }
                if sticker_poll > 0.4
                    && sticker_poll <= 0.7
                    && self.params.stake >= Item::Orange_Stake
                    && can_be_perishable(joker)
                {
                    stickers.perishable = true;
                }
                if self.params.stake >= Item::Gold_Stake {
                    let rental_key = if source == SOURCE_BUFFOON_PACK {
                        RANDOM_RENTAL_PACK
                    } else {
                        RANDOM_RENTAL
                    };
                    stickers.rental = self.random(&format!("{rental_key}{ante_str}")) > 0.7;
                }
            } else if self.params.stake >= Item::Black_Stake && can_be_eternal(joker) {
                stickers.eternal = self.random(&format!("{RANDOM_ETERNAL}{ante_str}")) > 0.7;
            }
        }

        JokerData {
            joker,
            rarity,
            edition,
            stickers,
        }
    }

    fn randchoice_joker_pool(
        &mut self,
        prefix: &str,
        source: &str,
        ante: i32,
        ante_str: &str,
        items: &[Item],
    ) -> Item {
        if let Some(key) = joker_pool_key(prefix, source, ante) {
            self.randchoice(key, items)
        } else {
            self.randchoice(&format!("{prefix}{source}{ante_str}"), items)
        }
    }

    pub fn shop_instance(&self) -> ShopInstance {
        let mut tarot_rate = 4.0;
        let mut planet_rate = 4.0;
        let mut playing_card_rate = 0.0;
        let mut spectral_rate = 0.0;
        if self.params.deck == Item::Ghost_Deck {
            spectral_rate = 2.0;
        }
        if self.is_voucher_active(Item::Tarot_Tycoon) {
            tarot_rate = 32.0;
        } else if self.is_voucher_active(Item::Tarot_Merchant) {
            tarot_rate = 9.6;
        }
        if self.is_voucher_active(Item::Planet_Tycoon) {
            planet_rate = 32.0;
        } else if self.is_voucher_active(Item::Planet_Merchant) {
            planet_rate = 9.6;
        }
        if self.is_voucher_active(Item::Magic_Trick) {
            playing_card_rate = 4.0;
        }
        ShopInstance {
            joker_rate: 20.0,
            tarot_rate,
            planet_rate,
            playing_card_rate,
            spectral_rate,
        }
    }

    pub fn next_shop_item(&mut self, ante: i32) -> ShopItem {
        let ante_str = ante_to_string(ante);
        let shop = self.shop_instance();
        let cdt_poll = if ante == 1 {
            self.random(KEY_CARD_TYPE_ANTE1)
        } else {
            self.random(&format!("{RANDOM_CARD_TYPE}{ante_str}"))
        } * shop.total_rate();
        let item_type = shop_item_type(shop, cdt_poll);
        match item_type {
            Item::T_Joker => {
                let joker = self.next_joker(SOURCE_SHOP, ante, true);
                ShopItem {
                    item_type,
                    item: joker.joker,
                    joker_data: joker,
                }
            },
            Item::T_Tarot => ShopItem {
                item_type,
                item: self.next_tarot(SOURCE_SHOP, ante, false),
                joker_data: JokerData::default(),
            },
            Item::T_Planet => ShopItem {
                item_type,
                item: self.next_planet(SOURCE_SHOP, ante, false),
                joker_data: JokerData::default(),
            },
            Item::T_Spectral => ShopItem {
                item_type,
                item: self.next_spectral(SOURCE_SHOP, ante, false),
                joker_data: JokerData::default(),
            },
            _ => ShopItem::default(),
        }
    }

    pub fn next_pack(&mut self, ante: i32) -> Item {
        if ante <= 2 && !self.cache.generated_first_pack && self.params.version > 10099 {
            self.cache.generated_first_pack = true;
            return Item::Buffoon_Pack;
        }
        if ante == 1 {
            self.randweightedchoice(KEY_SHOP_PACK_ANTE1, &PACKS)
        } else {
            let ante_str = ante_to_string(ante);
            self.randweightedchoice(&format!("{RANDOM_SHOP_PACK}{ante_str}"), &PACKS)
        }
    }

    pub fn next_arcana_pack(&mut self, size: usize, ante: i32) -> Vec<Item> {
        let mut pack = Vec::with_capacity(size);
        for _ in 0..size {
            let item = if self.is_voucher_active(Item::Omen_Globe)
                && self.random(RANDOM_OMEN_GLOBE) > 0.8
            {
                self.next_spectral(SOURCE_OMEN_GLOBE, ante, true)
            } else {
                self.next_tarot(SOURCE_ARCANA_PACK, ante, true)
            };
            if !self.params.showman {
                self.lock(item);
            }
            pack.push(item);
        }
        for item in &pack {
            self.unlock(*item);
        }
        pack
    }

    pub fn next_celestial_pack(&mut self, size: usize, ante: i32) -> Vec<Item> {
        let mut pack = Vec::with_capacity(size);
        for _ in 0..size {
            let item = self.next_planet(SOURCE_CELESTIAL_PACK, ante, true);
            if !self.params.showman {
                self.lock(item);
            }
            pack.push(item);
        }
        for item in &pack {
            self.unlock(*item);
        }
        pack
    }

    pub fn next_spectral_pack(&mut self, size: usize, ante: i32) -> Vec<Item> {
        let mut pack = Vec::with_capacity(size);
        for _ in 0..size {
            let item = self.next_spectral(SOURCE_SPECTRAL_PACK, ante, true);
            if !self.params.showman {
                self.lock(item);
            }
            pack.push(item);
        }
        for item in &pack {
            self.unlock(*item);
        }
        pack
    }

    pub fn next_buffoon_pack(&mut self, size: usize, ante: i32) -> Vec<JokerData> {
        let mut pack = Vec::with_capacity(size);
        for _ in 0..size {
            let joker = self.next_joker(SOURCE_BUFFOON_PACK, ante, true);
            if !self.params.showman {
                self.lock(joker.joker);
            }
            pack.push(joker);
        }
        for joker in &pack {
            self.unlock(joker.joker);
        }
        pack
    }

    pub fn next_standard_card(&mut self, ante: i32) -> Card {
        let ante_str = ante_to_string(ante);
        let enhancement =
            if self.random(&format!("{RANDOM_STANDARD_HAS_ENHANCEMENT}{ante_str}")) <= 0.6 {
                Item::No_Enhancement
            } else {
                self.randchoice(
                    &format!("{RANDOM_ENHANCEMENT}{SOURCE_STANDARD_PACK}{ante_str}"),
                    &ENHANCEMENTS,
                )
            };
        let base = self.randchoice(
            &format!("{RANDOM_CARD}{SOURCE_STANDARD_PACK}{ante_str}"),
            &CARDS,
        );
        let edition_poll = self.random(&format!("{RANDOM_STANDARD_EDITION}{ante_str}"));
        let edition = if edition_poll > 0.988 {
            Item::Polychrome
        } else if edition_poll > 0.96 {
            Item::Holographic
        } else if edition_poll > 0.92 {
            Item::Foil
        } else {
            Item::No_Edition
        };
        let seal = if self.random(&format!("{RANDOM_STANDARD_HAS_SEAL}{ante_str}")) <= 0.8 {
            Item::No_Seal
        } else {
            let seal_poll = self.random(&format!("{RANDOM_STANDARD_SEAL}{ante_str}"));
            if seal_poll > 0.75 {
                Item::Red_Seal
            } else if seal_poll > 0.5 {
                Item::Blue_Seal
            } else if seal_poll > 0.25 {
                Item::Gold_Seal
            } else {
                Item::Purple_Seal
            }
        };
        Card {
            base,
            enhancement,
            edition,
            seal,
        }
    }

    pub fn is_voucher_active(&self, voucher: Item) -> bool {
        let start = Item::Overstock.idx();
        let idx = voucher.idx().saturating_sub(start);
        idx < self.params.vouchers.len() && self.params.vouchers[idx]
    }

    pub fn activate_voucher(&mut self, voucher: Item) {
        let start = Item::Overstock.idx();
        let idx = voucher.idx().saturating_sub(start);
        if idx < self.params.vouchers.len() {
            self.params.vouchers[idx] = true;
        }
        self.lock(voucher);
        for pair in VOUCHERS.chunks_exact(2) {
            if pair[0] == voucher {
                self.unlock(pair[1]);
            }
        }
    }

    pub fn next_voucher(&mut self, ante: i32) -> Item {
        if ante == 1 {
            self.randchoice(KEY_VOUCHER_ANTE1, &VOUCHERS)
        } else {
            self.randchoice(
                &format!("{RANDOM_VOUCHER}{}", ante_to_string(ante)),
                &VOUCHERS,
            )
        }
    }

    pub fn set_deck(&mut self, deck: Item) {
        self.params.deck = deck;
        if deck == Item::Magic_Deck {
            self.activate_voucher(Item::Crystal_Ball);
        }
        if deck == Item::Nebula_Deck {
            self.activate_voucher(Item::Telescope);
        }
        if deck == Item::Zodiac_Deck {
            self.activate_voucher(Item::Tarot_Merchant);
            self.activate_voucher(Item::Planet_Merchant);
            self.activate_voucher(Item::Overstock);
        }
    }

    pub fn next_tag(&mut self, ante: i32) -> Item {
        if ante == 1 {
            self.randchoice(KEY_TAG_ANTE1, &TAGS)
        } else {
            self.randchoice(&format!("{RANDOM_TAGS}{}", ante_to_string(ante)), &TAGS)
        }
    }

    pub fn next_boss(&mut self, ante: i32) -> Item {
        let mut boss_pool = Vec::with_capacity(16);
        for boss in BOSSES {
            if !self.is_locked(boss)
                && ((ante % 8 == 0 && boss > Item::B_F_BEGIN)
                    || (ante % 8 != 0 && boss < Item::B_F_BEGIN))
            {
                boss_pool.push(boss);
            }
        }
        if boss_pool.is_empty() {
            for boss in BOSSES {
                if (ante % 8 == 0 && boss > Item::B_F_BEGIN)
                    || (ante % 8 != 0 && boss < Item::B_F_BEGIN)
                {
                    self.unlock(boss);
                }
            }
            return self.next_boss(ante);
        }
        let chosen = self.randchoice(RANDOM_BOSS, &boss_pool);
        self.lock(chosen);
        chosen
    }
}

pub fn pack_info(pack: Item) -> Pack {
    const PACK_INFO: [Pack; 15] = [
        Pack {
            pack_type: Item::Arcana_Pack,
            size: 3,
            choices: 1,
        },
        Pack {
            pack_type: Item::Arcana_Pack,
            size: 5,
            choices: 1,
        },
        Pack {
            pack_type: Item::Arcana_Pack,
            size: 5,
            choices: 2,
        },
        Pack {
            pack_type: Item::Celestial_Pack,
            size: 3,
            choices: 1,
        },
        Pack {
            pack_type: Item::Celestial_Pack,
            size: 5,
            choices: 1,
        },
        Pack {
            pack_type: Item::Celestial_Pack,
            size: 5,
            choices: 2,
        },
        Pack {
            pack_type: Item::Standard_Pack,
            size: 3,
            choices: 1,
        },
        Pack {
            pack_type: Item::Standard_Pack,
            size: 5,
            choices: 1,
        },
        Pack {
            pack_type: Item::Standard_Pack,
            size: 5,
            choices: 2,
        },
        Pack {
            pack_type: Item::Buffoon_Pack,
            size: 2,
            choices: 1,
        },
        Pack {
            pack_type: Item::Buffoon_Pack,
            size: 4,
            choices: 1,
        },
        Pack {
            pack_type: Item::Buffoon_Pack,
            size: 4,
            choices: 2,
        },
        Pack {
            pack_type: Item::Spectral_Pack,
            size: 2,
            choices: 1,
        },
        Pack {
            pack_type: Item::Spectral_Pack,
            size: 4,
            choices: 1,
        },
        Pack {
            pack_type: Item::Spectral_Pack,
            size: 4,
            choices: 2,
        },
    ];
    let idx = pack.idx() - Item::Arcana_Pack.idx();
    PACK_INFO[idx]
}

pub fn soul_yields_perkeo(inst: &mut Instance, ante: i32) -> bool {
    inst.random(&format!(
        "{RANDOM_JOKER_RARITY}{}{SOURCE_SOUL}",
        ante_to_string(ante),
    ));
    inst.next_joker(SOURCE_SOUL, ante, false).joker == Item::Perkeo
}

fn joker_rarity_key(source: &str, ante: i32) -> Option<&'static str> {
    if ante != 1 {
        return None;
    }
    match source {
        SOURCE_SHOP => Some(KEY_JOKER_RARITY_SHOP_ANTE1),
        SOURCE_BUFFOON_PACK => Some(KEY_JOKER_RARITY_BUFFOON_ANTE1),
        _ => None,
    }
}

fn joker_edition_key(source: &str, ante: i32) -> Option<&'static str> {
    if ante != 1 {
        return None;
    }
    match source {
        SOURCE_SHOP => Some(KEY_JOKER_EDITION_SHOP_ANTE1),
        SOURCE_BUFFOON_PACK => Some(KEY_JOKER_EDITION_BUFFOON_ANTE1),
        _ => None,
    }
}

fn joker_pool_key(prefix: &str, source: &str, ante: i32) -> Option<&'static str> {
    if ante != 1 {
        return None;
    }
    match (prefix, source) {
        (RANDOM_JOKER_COMMON, SOURCE_SHOP) => Some(KEY_JOKER_COMMON_SHOP_ANTE1),
        (RANDOM_JOKER_COMMON, SOURCE_BUFFOON_PACK) => Some(KEY_JOKER_COMMON_BUFFOON_ANTE1),
        (RANDOM_JOKER_UNCOMMON, SOURCE_SHOP) => Some(KEY_JOKER_UNCOMMON_SHOP_ANTE1),
        (RANDOM_JOKER_UNCOMMON, SOURCE_BUFFOON_PACK) => Some(KEY_JOKER_UNCOMMON_BUFFOON_ANTE1),
        (RANDOM_JOKER_RARE, SOURCE_SHOP) => Some(KEY_JOKER_RARE_SHOP_ANTE1),
        (RANDOM_JOKER_RARE, SOURCE_BUFFOON_PACK) => Some(KEY_JOKER_RARE_BUFFOON_ANTE1),
        _ => None,
    }
}

fn shop_item_type(shop: ShopInstance, mut cdt_poll: f64) -> Item {
    if cdt_poll < shop.joker_rate {
        return Item::T_Joker;
    }
    cdt_poll -= shop.joker_rate;
    if cdt_poll < shop.tarot_rate {
        return Item::T_Tarot;
    }
    cdt_poll -= shop.tarot_rate;
    if cdt_poll < shop.planet_rate {
        return Item::T_Planet;
    }
    cdt_poll -= shop.planet_rate;
    if cdt_poll < shop.playing_card_rate {
        return Item::T_Playing_Card;
    }
    Item::T_Spectral
}

fn can_be_eternal(joker: Item) -> bool {
    !matches!(
        joker,
        Item::Gros_Michel
            | Item::Ice_Cream
            | Item::Cavendish
            | Item::Luchador
            | Item::Turtle_Bean
            | Item::Diet_Cola
            | Item::Popcorn
            | Item::Ramen
            | Item::Seltzer
            | Item::Mr_Bones
            | Item::Invisible_Joker
    )
}

fn can_be_perishable(joker: Item) -> bool {
    !matches!(
        joker,
        Item::Ceremonial_Dagger
            | Item::Ride_the_Bus
            | Item::Runner
            | Item::Constellation
            | Item::Green_Joker
            | Item::Red_Card
            | Item::Madness
            | Item::Square_Joker
            | Item::Vampire
            | Item::Rocket
            | Item::Obelisk
            | Item::Lucky_Cat
            | Item::Flash_Card
            | Item::Spare_Trousers
            | Item::Castle
            | Item::Wee_Joker
    )
}
