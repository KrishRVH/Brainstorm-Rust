use crate::engine::config::{CompiledFilter, KernelShape};
use crate::engine::rng::RngKey;
use crate::engine::seed::SearchState;
use crate::engine::tables::{
    COMMON_JOKER_POOL, LEGENDARY_JOKER_POOL, Locks, POOL_COMMON, POOL_RARE, POOL_UNCOMMON,
    RARE_JOKER_POOL, STANDARD_JOKER_POOLS, TAG_POOL, UNCOMMON_JOKER_POOL, VOUCHER_POOL,
    is_ante1_locked_tag, is_arcana_pack, is_buffoon_pack, is_soulable_pack, is_spectral_pack,
    pack_info, shop_item_type, shop_rates_for_deck, weighted_packs,
};
use crate::filters::apply_filters;
use crate::instance::Instance;
use crate::item::Item;

pub fn apply_compiled_filter(state: &mut SearchState, cfg: &CompiledFilter) -> bool {
    match cfg.shape {
        KernelShape::NoMatch => false,
        KernelShape::NoFilter => true,
        KernelShape::TagOnly => tag_only(state, cfg),
        KernelShape::VoucherOnly => voucher_only(state, cfg),
        KernelShape::VoucherSecondPack => voucher_second_pack(state, cfg),
        KernelShape::PackOnly => pack_only(state, cfg),
        KernelShape::Observatory => observatory(state, cfg),
        KernelShape::ShopJoker => shop_joker(state, cfg),
        KernelShape::PackJoker => pack_joker(state, cfg),
        KernelShape::AnyJoker => any_joker(state, cfg),
        KernelShape::Souls => souls(state, cfg),
        KernelShape::Perkeo => perkeo(state, cfg),
        KernelShape::Erratic => erratic(state, cfg),
        KernelShape::TagObservatory => tag_observatory(state, cfg),
        KernelShape::SpectralSoulPerkeo => spectral_soul_perkeo(state, cfg),
        KernelShape::Composite => composite(state, cfg),
        KernelShape::Generic => generic_fallback(state, cfg),
    }
}

fn generic_fallback(state: &SearchState, cfg: &CompiledFilter) -> bool {
    let mut inst = Instance::new(state.seed.clone());
    apply_filters(&mut inst, &cfg.raw)
}

fn tag_only(state: &mut SearchState, cfg: &CompiledFilter) -> bool {
    let small = randchoice_tag(state);
    match (cfg.raw.tag1, cfg.raw.tag2) {
        (Item::RETRY, Item::RETRY) => true,
        (Item::RETRY, tag) | (tag, Item::RETRY) => small == tag || randchoice_tag(state) == tag,
        (tag1, tag2) if tag1 != tag2 => {
            if small == tag1 {
                randchoice_tag(state) == tag2
            } else if small == tag2 {
                randchoice_tag(state) == tag1
            } else {
                false
            }
        },
        (tag, _) => small == tag && randchoice_tag(state) == tag,
    }
}

fn voucher_only(state: &mut SearchState, cfg: &CompiledFilter) -> bool {
    next_voucher(state, &cfg.base_locks) == cfg.raw.voucher
}

fn voucher_second_pack(state: &mut SearchState, cfg: &CompiledFilter) -> bool {
    debug_assert_ne!(cfg.raw.pack, Item::Buffoon_Pack);
    second_pack_is(state, cfg.raw.pack) && voucher_only(state, cfg)
}

fn pack_only(state: &mut SearchState, cfg: &CompiledFilter) -> bool {
    cfg.raw.pack == Item::Buffoon_Pack || second_pack_is(state, cfg.raw.pack)
}

fn observatory(state: &mut SearchState, cfg: &CompiledFilter) -> bool {
    if !second_pack_is(state, Item::Mega_Celestial_Pack) {
        return false;
    }
    if next_voucher(state, &cfg.base_locks) != Item::Telescope {
        return false;
    }
    true
}

fn tag_observatory(state: &mut SearchState, cfg: &CompiledFilter) -> bool {
    if !second_pack_is(state, Item::Mega_Celestial_Pack) {
        return false;
    }
    if next_voucher(state, &cfg.base_locks) != Item::Telescope {
        return false;
    }
    tag_only(state, cfg)
}

fn shop_joker(state: &mut SearchState, cfg: &CompiledFilter) -> bool {
    if cfg.target_joker_pools & STANDARD_JOKER_POOLS == 0 {
        return false;
    }
    shop_has_joker(
        state,
        cfg.raw.joker,
        cfg.target_joker_pools,
        cfg.raw.deck,
        &cfg.base_locks,
    )
}

fn pack_joker(state: &mut SearchState, cfg: &CompiledFilter) -> bool {
    if cfg.target_joker_pools & STANDARD_JOKER_POOLS == 0 {
        return false;
    }

    if cfg.raw.pack == Item::RETRY || cfg.raw.pack == Item::Buffoon_Pack {
        if buffoon_pack_has_joker(
            state,
            Item::Buffoon_Pack,
            cfg.raw.joker,
            cfg.target_joker_pools,
            &cfg.base_locks,
        ) {
            return true;
        }

        let second_pack = roll_second_pack(state);
        if cfg.raw.pack == Item::Buffoon_Pack && second_pack != Item::Buffoon_Pack {
            return false;
        }
        return is_buffoon_pack(second_pack)
            && buffoon_pack_has_joker(
                state,
                second_pack,
                cfg.raw.joker,
                cfg.target_joker_pools,
                &cfg.base_locks,
            );
    }

    if !second_pack_is(state, cfg.raw.pack) {
        return false;
    }
    buffoon_pack_has_joker(
        state,
        cfg.raw.pack,
        cfg.raw.joker,
        cfg.target_joker_pools,
        &cfg.base_locks,
    )
}

fn any_joker(state: &mut SearchState, cfg: &CompiledFilter) -> bool {
    if cfg.raw.pack != Item::RETRY && cfg.raw.pack != Item::Buffoon_Pack {
        if !second_pack_is(state, cfg.raw.pack) {
            return false;
        }
        if cfg.wants_joker_shop && shop_joker(state, cfg) {
            return true;
        }
        return cfg.wants_joker_pack
            && is_buffoon_pack(cfg.raw.pack)
            && buffoon_pack_has_joker(
                state,
                cfg.raw.pack,
                cfg.raw.joker,
                cfg.target_joker_pools,
                &cfg.base_locks,
            );
    }

    if cfg.wants_joker_shop && shop_joker(state, cfg) {
        return true;
    }
    if !cfg.wants_joker_pack {
        return false;
    }
    pack_joker(state, cfg)
}

fn souls(state: &mut SearchState, cfg: &CompiledFilter) -> bool {
    if !cfg.selected_soulable_pack {
        return false;
    }
    let pack = if cfg.raw.pack == Item::RETRY {
        roll_second_pack(state)
    } else {
        if !second_pack_is(state, cfg.raw.pack) {
            return false;
        }
        cfg.raw.pack
    };
    if !is_soulable_pack(pack) {
        return false;
    }
    pack_contains_soul(state, pack, &cfg.base_locks)
}

fn perkeo(state: &mut SearchState, cfg: &CompiledFilter) -> bool {
    let pack = if cfg.raw.pack == Item::RETRY {
        roll_second_pack(state)
    } else {
        if !second_pack_is(state, cfg.raw.pack) {
            return false;
        }
        cfg.raw.pack
    };
    if !is_soulable_pack(pack) || !pack_contains_soul(state, pack, &cfg.base_locks) {
        return false;
    }
    soul_yields_perkeo(state, &cfg.base_locks)
}

fn spectral_soul_perkeo(state: &mut SearchState, cfg: &CompiledFilter) -> bool {
    if !second_pack_is(state, cfg.raw.pack) {
        return false;
    }
    if !spectral_pack_contains_soul(state, pack_info(cfg.raw.pack).size, &cfg.base_locks) {
        return false;
    }
    soul_yields_perkeo(state, &cfg.base_locks)
}

fn erratic(state: &mut SearchState, cfg: &CompiledFilter) -> bool {
    if !cfg.raw.erratic {
        return true;
    }
    if cfg.raw.min_face_cards <= 0 && cfg.raw.suit_ratio <= 0.0 {
        return true;
    }
    if cfg.raw.no_faces && cfg.raw.min_face_cards > 0 {
        return false;
    }
    if cfg.raw.suit_ratio <= 0.0 {
        return erratic_faces_only(state, cfg.raw.min_face_cards);
    }
    if !cfg.raw.no_faces && cfg.raw.min_face_cards <= 0 {
        return erratic_suits_only(state, cfg.raw.suit_ratio);
    }

    let mut total = 0_i32;
    let mut face_count = 0_i32;
    let mut suit_count = [0_i32; 4];
    let fixed_total_suit_requirement = if cfg.raw.suit_ratio > 0.0 && !cfg.raw.no_faces {
        (cfg.raw.suit_ratio * 52.0).ceil() as i32
    } else {
        0
    };
    let mut draws = state.rng.erratic_draws(&mut state.seed, state.hashed_seed);
    for drawn in 0..52 {
        let (is_face, suit) = draws.next_card_properties();
        if cfg.raw.no_faces && is_face {
            continue;
        }
        total += 1;
        if is_face {
            face_count += 1;
        }
        if suit < suit_count.len() {
            suit_count[suit] += 1;
        }
        if cfg.raw.no_faces && cfg.raw.suit_ratio <= 0.5 {
            return true;
        }
        let remaining = 51 - drawn;
        if cfg.raw.min_face_cards > 0 && face_count + remaining < cfg.raw.min_face_cards {
            return false;
        }
        if cfg.raw.suit_ratio <= 0.0 && face_count >= cfg.raw.min_face_cards {
            return true;
        }

        if cfg.raw.suit_ratio > 0.0 {
            let top_two = top_two_suit_count(suit_count);
            if fixed_total_suit_requirement > 0 {
                if top_two + remaining < fixed_total_suit_requirement {
                    return false;
                }
                if top_two >= fixed_total_suit_requirement && face_count >= cfg.raw.min_face_cards {
                    return true;
                }
            } else {
                let maximum_total = total + remaining;
                if maximum_total > 0
                    && f64::from(top_two + remaining) / f64::from(maximum_total)
                        < cfg.raw.suit_ratio
                {
                    return false;
                }
                if maximum_total > 0
                    && f64::from(top_two) / f64::from(maximum_total) >= cfg.raw.suit_ratio
                {
                    return true;
                }
            }
        }
    }

    if cfg.raw.min_face_cards > 0 && face_count < cfg.raw.min_face_cards {
        return false;
    }
    if cfg.raw.suit_ratio > 0.0 {
        if total == 0 {
            return false;
        }
        return f64::from(top_two_suit_count(suit_count)) / f64::from(total) >= cfg.raw.suit_ratio;
    }
    true
}

fn erratic_faces_only(state: &mut SearchState, required_faces: i32) -> bool {
    let mut faces_needed = required_faces;
    let mut misses_left = 52 - required_faces;
    let mut draws = state.rng.erratic_draws(&mut state.seed, state.hashed_seed);
    for _ in 0..52 {
        let is_face = i32::from(draws.next_is_face());
        faces_needed -= is_face;
        misses_left -= 1 - is_face;
        if faces_needed == 0 {
            return true;
        }
        if misses_left < 0 {
            return false;
        }
    }
    false
}

fn erratic_suits_only(state: &mut SearchState, suit_ratio: f64) -> bool {
    let required_cards = (suit_ratio * 52.0).ceil() as i32;
    let mut suit_count = [0_i32; 4];
    let mut draws = state.rng.erratic_draws(&mut state.seed, state.hashed_seed);
    for drawn in 0..52 {
        let suit = draws.next_suit_index();
        suit_count[suit] += 1;
        let top_two = top_two_suit_count(suit_count);
        if top_two >= required_cards {
            return true;
        }
        if top_two + 51 - drawn < required_cards {
            return false;
        }
    }
    false
}

fn top_two_suit_count(suit_count: [i32; 4]) -> i32 {
    // Pairwise comparisons expose independent work in this per-draw hot path.
    let [a, b, c, d] = suit_count;
    let ab_high = a.max(b);
    let cd_high = c.max(d);
    let ab_low = a.min(b);
    let cd_low = c.min(d);
    ab_high.max(cd_high) + ab_high.min(cd_high).max(ab_low.max(cd_low))
}

fn composite(state: &mut SearchState, cfg: &CompiledFilter) -> bool {
    let mut packs = [Item::Buffoon_Pack, Item::RETRY];
    let selective_pack = if cfg.raw.observatory {
        Item::Mega_Celestial_Pack
    } else if cfg.raw.pack != Item::RETRY && cfg.raw.pack != Item::Buffoon_Pack {
        cfg.raw.pack
    } else {
        Item::RETRY
    };
    if selective_pack != Item::RETRY {
        if !second_pack_is(state, selective_pack) {
            return false;
        }
        packs[1] = selective_pack;
    }

    let mut souls_checked_early = false;
    if cfg.raw.pack == Item::RETRY && (cfg.raw.souls > 0 || cfg.raw.perkeo) {
        packs[1] = roll_second_pack(state);
        if !composite_has_souls_and_perkeo(state, cfg, packs) {
            return false;
        }
        souls_checked_early = true;
    }

    let mut first_voucher = Item::RETRY;
    if cfg.raw.voucher != Item::RETRY || cfg.raw.observatory {
        first_voucher = next_voucher(state, &cfg.base_locks);
        if cfg.raw.voucher != Item::RETRY && first_voucher != cfg.raw.voucher {
            return false;
        }
    }

    if (cfg.raw.tag1 != Item::RETRY || cfg.raw.tag2 != Item::RETRY) && !tag_only(state, cfg) {
        return false;
    }

    if packs[1] == Item::RETRY && (cfg.wants_joker_pack || cfg.raw.souls > 0 || cfg.raw.perkeo) {
        packs[1] = roll_second_pack(state);
    }

    if cfg.raw.observatory {
        if first_voucher != Item::Telescope {
            return false;
        }
        if !packs.contains(&Item::Mega_Celestial_Pack) {
            return false;
        }
    }

    if cfg.raw.joker != Item::RETRY && !composite_has_joker(state, cfg, packs) {
        return false;
    }

    if !souls_checked_early
        && (cfg.raw.souls > 0 || cfg.raw.perkeo)
        && !composite_has_souls_and_perkeo(state, cfg, packs)
    {
        return false;
    }

    erratic(state, cfg)
}

fn composite_has_joker(state: &mut SearchState, cfg: &CompiledFilter, packs: [Item; 2]) -> bool {
    if cfg.wants_joker_shop
        && shop_has_joker(
            state,
            cfg.raw.joker,
            cfg.target_joker_pools,
            cfg.raw.deck,
            &cfg.base_locks,
        )
    {
        return true;
    }
    if !cfg.wants_joker_pack {
        return false;
    }

    packs.into_iter().any(|pack| {
        pack_matches_filter(pack, cfg.raw.pack)
            && is_buffoon_pack(pack)
            && buffoon_pack_has_joker(
                state,
                pack,
                cfg.raw.joker,
                cfg.target_joker_pools,
                &cfg.base_locks,
            )
    })
}

fn composite_has_souls_and_perkeo(
    state: &mut SearchState,
    cfg: &CompiledFilter,
    packs: [Item; 2],
) -> bool {
    let mut soul_found = cfg.raw.souls <= 0;
    let mut perkeo_found = !cfg.raw.perkeo;
    for pack in packs {
        if !pack_matches_filter(pack, cfg.raw.pack) || !is_soulable_pack(pack) {
            continue;
        }

        if !pack_contains_soul(state, pack, &cfg.base_locks) {
            continue;
        }
        soul_found = true;

        if cfg.raw.perkeo && soul_yields_perkeo(state, &cfg.base_locks) {
            perkeo_found = true;
        }
        if soul_found && perkeo_found {
            break;
        }
    }

    soul_found && perkeo_found
}

fn pack_matches_filter(pack: Item, selected_pack: Item) -> bool {
    pack != Item::RETRY && (selected_pack == Item::RETRY || pack == selected_pack)
}

fn roll_second_pack(state: &mut SearchState) -> Item {
    let poll = state
        .rng
        .random(RngKey::ShopPack1, &mut state.seed, state.hashed_seed)
        * weighted_packs()[0].weight;
    roll_second_pack_for_poll(poll)
}

fn second_pack_is(state: &mut SearchState, target: Item) -> bool {
    let poll = state
        .rng
        .random(RngKey::ShopPack1, &mut state.seed, state.hashed_seed)
        * weighted_packs()[0].weight;
    match target {
        Item::Arcana_Pack => poll <= 4.0,
        Item::Jumbo_Arcana_Pack => poll > 4.0 && poll <= 6.0,
        Item::Mega_Arcana_Pack => poll > 6.0 && poll <= 6.5,
        Item::Celestial_Pack => poll > 6.5 && poll <= 10.5,
        Item::Jumbo_Celestial_Pack => poll > 10.5 && poll <= 12.5,
        Item::Mega_Celestial_Pack => poll > 12.5 && poll <= 13.0,
        Item::Standard_Pack => poll > 13.0 && poll <= 17.0,
        Item::Jumbo_Standard_Pack => poll > 17.0 && poll <= 19.0,
        Item::Mega_Standard_Pack => poll > 19.0 && poll <= 19.5,
        Item::Buffoon_Pack => poll > 19.5 && poll <= 20.7,
        Item::Jumbo_Buffoon_Pack => poll > 20.7 && poll <= 21.3,
        Item::Mega_Buffoon_Pack => poll > 21.3 && poll <= 21.45,
        Item::Spectral_Pack => poll > 21.45 && poll <= 22.05,
        Item::Jumbo_Spectral_Pack => poll > 22.05 && poll <= 22.35,
        Item::Mega_Spectral_Pack => poll > 22.35 && poll <= 22.42,
        _ => roll_second_pack_for_poll(poll) == target,
    }
}

fn roll_second_pack_for_poll(poll: f64) -> Item {
    let mut weight = 0.0;
    for entry in &weighted_packs()[1..] {
        weight += entry.weight;
        if weight >= poll {
            return entry.item;
        }
    }
    weighted_packs()
        .last()
        .map_or(Item::RETRY, |entry| entry.item)
}

fn next_voucher(state: &mut SearchState, locks: &Locks) -> Item {
    randchoice(state, RngKey::Voucher1, VOUCHER_POOL, locks)
}

fn shop_has_joker(
    state: &mut SearchState,
    target: Item,
    target_pools: u8,
    deck: Item,
    locks: &Locks,
) -> bool {
    let rates = shop_rates_for_deck(deck);
    for _ in 0..2 {
        let poll = state
            .rng
            .random(RngKey::Cdt1, &mut state.seed, state.hashed_seed)
            * rates.total();
        if shop_item_type(rates, poll) != Item::T_Joker {
            continue;
        }
        if next_joker_item(state, JokerSource::Shop, target_pools, locks) == target {
            return true;
        }
    }
    false
}

fn buffoon_pack_has_joker(
    state: &mut SearchState,
    pack: Item,
    target: Item,
    target_pools: u8,
    base_locks: &Locks,
) -> bool {
    let info = pack_info(pack);
    let mut locks = *base_locks;
    for _ in 0..info.size {
        let joker = next_joker_item(state, JokerSource::Buffoon, target_pools, &locks);
        if joker == target {
            return true;
        }
        locks.lock(joker);
    }
    false
}

fn pack_contains_soul(state: &mut SearchState, pack: Item, base_locks: &Locks) -> bool {
    let info = pack_info(pack);
    if is_spectral_pack(pack) {
        return spectral_pack_contains_soul(state, info.size, base_locks);
    }
    is_arcana_pack(pack)
        && !base_locks.is_locked(Item::The_Soul)
        && (0..info.size).any(|_| {
            state
                .rng
                .random(RngKey::SoulTarot1, &mut state.seed, state.hashed_seed)
                > 0.997
        })
}

fn spectral_pack_contains_soul(state: &mut SearchState, size: usize, locks: &Locks) -> bool {
    let soul_locked = locks.is_locked(Item::The_Soul);
    let mut black_hole_locked = locks.is_locked(Item::Black_Hole);
    for _ in 0..size {
        let mut item = Item::RETRY;
        if !soul_locked
            && state
                .rng
                .random(RngKey::SoulSpectral1, &mut state.seed, state.hashed_seed)
                > 0.997
        {
            item = Item::The_Soul;
        }
        if !black_hole_locked
            && state
                .rng
                .random(RngKey::SoulSpectral1, &mut state.seed, state.hashed_seed)
                > 0.997
        {
            item = Item::Black_Hole;
        }
        match item {
            Item::The_Soul => return true,
            Item::Black_Hole => black_hole_locked = true,
            _ => {},
        }
    }
    false
}

fn soul_yields_perkeo(state: &mut SearchState, locks: &Locks) -> bool {
    randchoice(state, RngKey::JokerLegendary, LEGENDARY_JOKER_POOL, locks) == Item::Perkeo
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum JokerSource {
    Shop,
    Buffoon,
}

fn next_joker_item(
    state: &mut SearchState,
    source: JokerSource,
    target_pools: u8,
    locks: &Locks,
) -> Item {
    let rarity_pool = {
        let rarity_key = match source {
            JokerSource::Shop => RngKey::RarityShop1,
            JokerSource::Buffoon => RngKey::RarityBuffoon1,
        };
        let poll = state
            .rng
            .random(rarity_key, &mut state.seed, state.hashed_seed);
        if poll > 0.95 {
            POOL_RARE
        } else if poll > 0.7 {
            POOL_UNCOMMON
        } else {
            POOL_COMMON
        }
    };

    if target_pools != 0 && rarity_pool & target_pools == 0 {
        return Item::RETRY;
    }

    match (source, rarity_pool) {
        (JokerSource::Shop, POOL_COMMON) => {
            randchoice(state, RngKey::JokerCommonShop1, COMMON_JOKER_POOL, locks)
        },
        (JokerSource::Shop, POOL_UNCOMMON) => randchoice(
            state,
            RngKey::JokerUncommonShop1,
            UNCOMMON_JOKER_POOL,
            locks,
        ),
        (JokerSource::Shop, POOL_RARE) => {
            randchoice(state, RngKey::JokerRareShop1, RARE_JOKER_POOL, locks)
        },
        (JokerSource::Buffoon, POOL_COMMON) => {
            randchoice(state, RngKey::JokerCommonBuffoon1, COMMON_JOKER_POOL, locks)
        },
        (JokerSource::Buffoon, POOL_UNCOMMON) => randchoice(
            state,
            RngKey::JokerUncommonBuffoon1,
            UNCOMMON_JOKER_POOL,
            locks,
        ),
        (JokerSource::Buffoon, POOL_RARE) => {
            randchoice(state, RngKey::JokerRareBuffoon1, RARE_JOKER_POOL, locks)
        },
        _ => Item::RETRY,
    }
}

fn randchoice(state: &mut SearchState, key: RngKey, items: &[Item], locks: &Locks) -> Item {
    let idx = state.rng.randint(
        key,
        &mut state.seed,
        state.hashed_seed,
        0,
        items.len() as i32 - 1,
    ) as usize;
    let item = items[idx];
    if item != Item::RETRY && !locks.is_locked(item) {
        return item;
    }

    let mut resample = 2_u16;
    loop {
        let idx = state.rng.randint_resample(
            key,
            resample,
            &mut state.seed,
            state.hashed_seed,
            0,
            items.len() as i32 - 1,
        ) as usize;
        let candidate = items[idx];
        resample += 1;
        if (candidate != Item::RETRY && !locks.is_locked(candidate)) || resample > 1000 {
            return candidate;
        }
    }
}

fn randchoice_tag(state: &mut SearchState) -> Item {
    let idx = state.rng.randint(
        RngKey::Tag1,
        &mut state.seed,
        state.hashed_seed,
        0,
        TAG_POOL.len() as i32 - 1,
    ) as usize;
    let item = TAG_POOL[idx];
    if !is_ante1_locked_tag(item) {
        return item;
    }

    let mut resample = 2_u16;
    loop {
        let idx = state.rng.randint_resample(
            RngKey::Tag1,
            resample,
            &mut state.seed,
            state.hashed_seed,
            0,
            TAG_POOL.len() as i32 - 1,
        ) as usize;
        let candidate = TAG_POOL[idx];
        resample += 1;
        if !is_ante1_locked_tag(candidate) || resample > 1000 {
            return candidate;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{roll_second_pack, second_pack_is, top_two_suit_count, weighted_packs};
    use crate::engine::seed::SearchState;

    #[test]
    fn direct_second_pack_intervals_match_weighted_rolls() {
        for seed_id in 0..4_096 {
            let state = SearchState::from_id(seed_id);
            let mut rolled = state.clone();
            let expected = roll_second_pack(&mut rolled);
            for entry in &weighted_packs()[1..] {
                let mut direct = state.clone();
                assert_eq!(
                    second_pack_is(&mut direct, entry.item),
                    entry.item == expected,
                    "pack interval mismatch for seed id {seed_id}",
                );
            }
        }
    }

    #[test]
    fn pairwise_top_two_matches_sorted_counts() {
        for a in 0_i32..=52 {
            for b in 0_i32..=52 - a {
                for c in 0_i32..=52 - a - b {
                    for d in 0_i32..=52 - a - b - c {
                        let mut counts = [a, b, c, d];
                        counts.sort_unstable();
                        assert_eq!(top_two_suit_count([a, b, c, d]), counts[2] + counts[3]);
                    }
                }
            }
        }
    }
}
