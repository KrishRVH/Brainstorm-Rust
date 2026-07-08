use crate::engine::tables::{is_arcana_pack, is_buffoon_pack, is_soulable_pack, pack_info};
use crate::instance::{Instance, soul_yields_perkeo};
use crate::item::{CARDS, Item, is_joker_item, string_to_item};

const CARDS_PER_SUIT: usize = 13;
const SUIT_COUNT: usize = 4;
const RANK_ORDER: [Item; CARDS_PER_SUIT] = [
    Item::_2,
    Item::_3,
    Item::_4,
    Item::_5,
    Item::_6,
    Item::_7,
    Item::_8,
    Item::_9,
    Item::Ace,
    Item::Jack,
    Item::King,
    Item::Queen,
    Item::_10,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JokerLocation {
    Any,
    Shop,
    Pack,
}

#[derive(Clone, Copy, Debug)]
pub struct FilterConfig {
    pub voucher: Item,
    pub pack: Item,
    pub tag1: Item,
    pub tag2: Item,
    pub joker: Item,
    pub joker_location: JokerLocation,
    pub souls: i64,
    pub observatory: bool,
    pub perkeo: bool,
    pub deck: Item,
    pub erratic: bool,
    pub no_faces: bool,
    pub min_face_cards: i32,
    pub suit_ratio: f64,
}

impl Default for FilterConfig {
    fn default() -> Self {
        Self {
            voucher: Item::RETRY,
            pack: Item::RETRY,
            tag1: Item::RETRY,
            tag2: Item::RETRY,
            joker: Item::RETRY,
            joker_location: JokerLocation::Any,
            souls: 0,
            observatory: false,
            perkeo: false,
            deck: Item::Red_Deck,
            erratic: false,
            no_faces: false,
            min_face_cards: 0,
            suit_ratio: 0.0,
        }
    }
}

impl FilterConfig {
    pub fn from_raw(
        voucher: &str,
        pack: &str,
        tag1: &str,
        tag2: &str,
        joker_name: &str,
        joker_location: &str,
        souls: f64,
        observatory: bool,
        perkeo: bool,
        deck: &str,
        erratic: bool,
        no_faces: bool,
        min_face_cards: i32,
        suit_ratio: f64,
    ) -> Self {
        Self {
            voucher: parse_voucher_key(voucher),
            pack: parse_pack_key(pack),
            tag1: parse_tag_key(tag1),
            tag2: parse_tag_key(tag2),
            joker: parse_joker_name(joker_name),
            joker_location: parse_joker_location(joker_location),
            souls: if souls > 0.0 { souls as i64 } else { 0 },
            observatory,
            perkeo,
            deck: parse_deck_key(deck),
            erratic,
            no_faces,
            min_face_cards: min_face_cards.max(0),
            suit_ratio: if suit_ratio > 0.0 {
                suit_ratio.min(1.0)
            } else {
                0.0
            },
        }
    }
}

pub fn apply_filters(inst: &mut Instance, cfg: &FilterConfig) -> bool {
    const ANTE: i32 = 1;
    inst.init_locks(ANTE, false, false);
    inst.set_deck(cfg.deck);

    let wants_joker = cfg.joker != Item::RETRY;
    let wants_joker_shop =
        wants_joker && matches!(cfg.joker_location, JokerLocation::Shop | JokerLocation::Any);
    let wants_joker_pack =
        wants_joker && matches!(cfg.joker_location, JokerLocation::Pack | JokerLocation::Any);

    let needs_tags = cfg.tag1 != Item::RETRY || cfg.tag2 != Item::RETRY;
    let mut small_blind = Item::RETRY;
    let mut big_blind = Item::RETRY;
    if needs_tags {
        small_blind = inst.next_tag(ANTE);
        big_blind = inst.next_tag(ANTE);
    }

    if cfg.tag1 != Item::RETRY || cfg.tag2 != Item::RETRY {
        if cfg.tag1 == Item::RETRY {
            if small_blind != cfg.tag2 && big_blind != cfg.tag2 {
                return false;
            }
        } else if cfg.tag2 == Item::RETRY {
            if small_blind != cfg.tag1 && big_blind != cfg.tag1 {
                return false;
            }
        } else if cfg.tag1 != cfg.tag2 {
            let has_tag1 = small_blind == cfg.tag1 || big_blind == cfg.tag1;
            let has_tag2 = small_blind == cfg.tag2 || big_blind == cfg.tag2;
            if !has_tag1 || !has_tag2 {
                return false;
            }
        } else if small_blind != cfg.tag1 || big_blind != cfg.tag1 {
            return false;
        }
    }

    let needs_voucher = cfg.voucher != Item::RETRY || cfg.observatory;
    let mut first_voucher = Item::RETRY;
    if needs_voucher {
        first_voucher = inst.next_voucher(ANTE);
    }

    let needs_packs = cfg.pack != Item::RETRY
        || cfg.observatory
        || cfg.perkeo
        || cfg.souls > 0
        || wants_joker_pack;
    let mut pack_slots = [Item::RETRY; 2];
    if needs_packs {
        pack_slots[0] = inst.next_pack(ANTE);
        pack_slots[1] = inst.next_pack(ANTE);
    }

    if cfg.voucher != Item::RETRY && first_voucher != cfg.voucher {
        return false;
    }

    if cfg.pack != Item::RETRY && pack_slots[0] != cfg.pack && pack_slots[1] != cfg.pack {
        return false;
    }

    if cfg.observatory {
        if first_voucher != Item::Telescope {
            return false;
        }
        if pack_slots[0] != Item::Mega_Celestial_Pack && pack_slots[1] != Item::Mega_Celestial_Pack
        {
            return false;
        }
    }

    if wants_joker {
        let mut joker_found = false;
        if wants_joker_shop && shop_has_joker(inst, cfg.joker, ANTE) {
            joker_found = true;
        }
        if !joker_found && wants_joker_pack {
            for pack in pack_slots {
                if pack == Item::RETRY {
                    continue;
                }
                if cfg.pack != Item::RETRY && pack != cfg.pack {
                    continue;
                }
                if pack_has_joker(inst, pack, cfg.joker, ANTE) {
                    joker_found = true;
                    break;
                }
            }
        }
        if !joker_found {
            return false;
        }
    }

    if cfg.perkeo || cfg.souls > 0 {
        let mut souls_found = 0_i64;
        let mut perkeo_found = !cfg.perkeo;
        for pack in pack_slots {
            if pack == Item::RETRY {
                continue;
            }
            if cfg.pack != Item::RETRY && pack != cfg.pack {
                continue;
            }
            if !is_soulable_pack(pack) {
                continue;
            }

            let souls_in_pack = i64::from(count_souls_in_pack(inst, pack, ANTE));
            if souls_in_pack <= 0 {
                continue;
            }
            souls_found += souls_in_pack;

            if cfg.perkeo {
                let uses = (souls_in_pack as usize).min(pack_info(pack).choices);
                for _ in 0..uses {
                    if soul_yields_perkeo(inst, ANTE) {
                        perkeo_found = true;
                        break;
                    }
                }
            }
            if cfg.perkeo && perkeo_found && (cfg.souls <= 0 || souls_found >= cfg.souls) {
                break;
            }
        }

        if cfg.souls > 0 && souls_found < cfg.souls {
            return false;
        }
        if cfg.perkeo && !perkeo_found {
            return false;
        }
    }

    passes_erratic_filters(inst, cfg)
}

pub fn parse_tag_key(key: &str) -> Item {
    match key {
        "" => Item::RETRY,
        "tag_uncommon" => Item::Uncommon_Tag,
        "tag_rare" => Item::Rare_Tag,
        "tag_negative" => Item::Negative_Tag,
        "tag_foil" => Item::Foil_Tag,
        "tag_holo" => Item::Holographic_Tag,
        "tag_polychrome" => Item::Polychrome_Tag,
        "tag_investment" => Item::Investment_Tag,
        "tag_voucher" => Item::Voucher_Tag,
        "tag_boss" => Item::Boss_Tag,
        "tag_standard" => Item::Standard_Tag,
        "tag_charm" => Item::Charm_Tag,
        "tag_meteor" => Item::Meteor_Tag,
        "tag_buffoon" => Item::Buffoon_Tag,
        "tag_handy" => Item::Handy_Tag,
        "tag_garbage" => Item::Garbage_Tag,
        "tag_ethereal" => Item::Ethereal_Tag,
        "tag_coupon" => Item::Coupon_Tag,
        "tag_double" => Item::Double_Tag,
        "tag_juggle" => Item::Juggle_Tag,
        "tag_d_six" => Item::D6_Tag,
        "tag_top_up" => Item::Top_up_Tag,
        "tag_skip" => Item::Speed_Tag,
        "tag_orbital" => Item::Orbital_Tag,
        "tag_economy" => Item::Economy_Tag,
        _ => Item::RETRY,
    }
}

pub fn parse_pack_key(key: &str) -> Item {
    if key.is_empty() {
        return Item::RETRY;
    }
    match normalize_pack_key(key).as_str() {
        "p_arcana_normal" => Item::Arcana_Pack,
        "p_arcana_jumbo" => Item::Jumbo_Arcana_Pack,
        "p_arcana_mega" => Item::Mega_Arcana_Pack,
        "p_celestial_normal" => Item::Celestial_Pack,
        "p_celestial_jumbo" => Item::Jumbo_Celestial_Pack,
        "p_celestial_mega" => Item::Mega_Celestial_Pack,
        "p_standard_normal" => Item::Standard_Pack,
        "p_standard_jumbo" => Item::Jumbo_Standard_Pack,
        "p_standard_mega" => Item::Mega_Standard_Pack,
        "p_buffoon_normal" => Item::Buffoon_Pack,
        "p_buffoon_jumbo" => Item::Jumbo_Buffoon_Pack,
        "p_buffoon_mega" => Item::Mega_Buffoon_Pack,
        "p_spectral_normal" => Item::Spectral_Pack,
        "p_spectral_jumbo" => Item::Jumbo_Spectral_Pack,
        "p_spectral_mega" => Item::Mega_Spectral_Pack,
        _ => Item::RETRY,
    }
}

pub fn parse_voucher_key(key: &str) -> Item {
    match key {
        "" => Item::RETRY,
        "v_overstock_norm" => Item::Overstock,
        "v_overstock_plus" => Item::Overstock_Plus,
        "v_clearance_sale" => Item::Clearance_Sale,
        "v_liquidation" => Item::Liquidation,
        "v_hone" => Item::Hone,
        "v_glow_up" => Item::Glow_Up,
        "v_reroll_surplus" => Item::Reroll_Surplus,
        "v_reroll_glut" => Item::Reroll_Glut,
        "v_crystal_ball" => Item::Crystal_Ball,
        "v_omen_globe" => Item::Omen_Globe,
        "v_telescope" => Item::Telescope,
        "v_observatory" => Item::Observatory,
        "v_grabber" => Item::Grabber,
        "v_nacho_tong" => Item::Nacho_Tong,
        "v_wasteful" => Item::Wasteful,
        "v_recyclomancy" => Item::Recyclomancy,
        "v_tarot_merchant" => Item::Tarot_Merchant,
        "v_tarot_tycoon" => Item::Tarot_Tycoon,
        "v_planet_merchant" => Item::Planet_Merchant,
        "v_planet_tycoon" => Item::Planet_Tycoon,
        "v_seed_money" => Item::Seed_Money,
        "v_money_tree" => Item::Money_Tree,
        "v_blank" => Item::Blank,
        "v_antimatter" => Item::Antimatter,
        "v_magic_trick" => Item::Magic_Trick,
        "v_illusion" => Item::Illusion,
        "v_hieroglyph" => Item::Hieroglyph,
        "v_petroglyph" => Item::Petroglyph,
        "v_directors_cut" => Item::Directors_Cut,
        "v_paint_brush" => Item::Paint_Brush,
        "v_retcon" => Item::Retcon,
        "v_palette" => Item::Palette,
        _ => Item::RETRY,
    }
}

pub fn parse_deck_key(key: &str) -> Item {
    match key {
        "" | "b_red" => Item::Red_Deck,
        "b_blue" => Item::Blue_Deck,
        "b_yellow" => Item::Yellow_Deck,
        "b_green" => Item::Green_Deck,
        "b_black" => Item::Black_Deck,
        "b_magic" => Item::Magic_Deck,
        "b_nebula" => Item::Nebula_Deck,
        "b_ghost" => Item::Ghost_Deck,
        "b_abandoned" => Item::Abandoned_Deck,
        "b_checkered" => Item::Checkered_Deck,
        "b_zodiac" => Item::Zodiac_Deck,
        "b_painted" => Item::Painted_Deck,
        "b_anaglyph" => Item::Anaglyph_Deck,
        "b_plasma" => Item::Plasma_Deck,
        "b_erratic" => Item::Erratic_Deck,
        "b_challenge" => Item::Challenge_Deck,
        _ => Item::Red_Deck,
    }
}

pub fn parse_joker_name(name: &str) -> Item {
    if name.is_empty() {
        return Item::RETRY;
    }
    let item = match name {
        "j_caino" | "j_canio" => Item::Canio,
        "Caino" | "Canio" => Item::Canio,
        "j_seance" | "Seance" => Item::Seance,
        _ => Item::RETRY,
    };
    if is_joker_item(item) {
        return item;
    }
    let item = string_to_item(name);
    if is_joker_item(item) {
        item
    } else {
        Item::RETRY
    }
}

pub fn parse_joker_location(location: &str) -> JokerLocation {
    match location {
        "shop" => JokerLocation::Shop,
        "pack" => JokerLocation::Pack,
        _ => JokerLocation::Any,
    }
}

pub fn normalize_pack_key(key: &str) -> String {
    let Some(pos) = key.rfind('_') else {
        return key.to_owned();
    };
    if pos + 1 >= key.len() {
        return key.to_owned();
    }
    if key[pos + 1..].bytes().all(|b| b.is_ascii_digit()) {
        key[..pos].to_owned()
    } else {
        key.to_owned()
    }
}

fn is_face_rank(rank: Item) -> bool {
    matches!(rank, Item::Jack | Item::Queen | Item::King)
}

fn analyze_erratic_deck(inst: &mut Instance, no_faces: bool) -> DeckStats {
    let mut stats = DeckStats::default();
    for _ in 0..CARDS.len() {
        let card = inst.randchoice("erratic", &CARDS);
        let idx = card.idx() as isize - Item::C_2.idx() as isize;
        if idx < 0 || idx >= CARDS.len() as isize {
            continue;
        }
        let rank_idx = idx as usize % CARDS_PER_SUIT;
        let suit_idx = idx as usize / CARDS_PER_SUIT;
        let rank = RANK_ORDER[rank_idx];
        if no_faces && is_face_rank(rank) {
            continue;
        }
        stats.total += 1;
        if is_face_rank(rank) {
            stats.face_count += 1;
        }
        if suit_idx < SUIT_COUNT {
            stats.suit_count[suit_idx] += 1;
        }
    }
    stats
}

fn passes_erratic_filters(inst: &mut Instance, cfg: &FilterConfig) -> bool {
    if !cfg.erratic {
        return true;
    }
    if cfg.min_face_cards <= 0 && cfg.suit_ratio <= 0.0 {
        return true;
    }
    let stats = analyze_erratic_deck(inst, cfg.no_faces);
    if cfg.min_face_cards > 0 && stats.face_count < cfg.min_face_cards {
        return false;
    }
    if cfg.suit_ratio > 0.0 {
        if stats.total == 0 {
            return false;
        }
        let mut first = 0_i32;
        let mut second = 0_i32;
        for count in stats.suit_count {
            if count >= first {
                second = first;
                first = count;
            } else if count > second {
                second = count;
            }
        }
        let ratio = f64::from(first + second) / f64::from(stats.total);
        if ratio < cfg.suit_ratio {
            return false;
        }
    }
    true
}

fn count_souls_in_pack(inst: &mut Instance, pack: Item, ante: i32) -> i32 {
    if !is_soulable_pack(pack) {
        return 0;
    }
    let info = pack_info(pack);
    let cards = if is_arcana_pack(pack) {
        inst.next_arcana_pack(info.size, ante)
    } else {
        inst.next_spectral_pack(info.size, ante)
    };
    cards.iter().filter(|card| **card == Item::The_Soul).count() as i32
}

fn shop_has_joker(inst: &mut Instance, target: Item, ante: i32) -> bool {
    let mut found = false;
    for _ in 0..2 {
        let item = inst.next_shop_item(ante);
        if item.item_type == Item::T_Joker && item.item == target {
            found = true;
        }
    }
    found
}

fn pack_has_joker(inst: &mut Instance, pack: Item, target: Item, ante: i32) -> bool {
    if !is_buffoon_pack(pack) {
        return false;
    }
    let info = pack_info(pack);
    inst.next_buffoon_pack(info.size, ante)
        .iter()
        .any(|joker| joker.joker == target)
}

#[derive(Clone, Copy, Debug, Default)]
struct DeckStats {
    total: i32,
    face_count: i32,
    suit_count: [i32; SUIT_COUNT],
}
