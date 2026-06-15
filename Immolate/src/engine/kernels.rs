use crate::engine::config::{CompiledFilter, KernelShape};
use crate::engine::rng::RngKey;
use crate::engine::seed::SearchState;
use crate::engine::tables::{
    COMMON_JOKER_POOL, LEGENDARY_JOKER_POOL, Locks, POOL_COMMON, POOL_RARE, POOL_UNCOMMON,
    RARE_JOKER_POOL, SPECTRAL_POOL, TAG_POOL, TAROT_POOL, UNCOMMON_JOKER_POOL, VOUCHER_POOL,
    card_face_and_suit, is_ante1_locked_tag, is_arcana_pack, is_buffoon_pack, is_soulable_pack,
    is_spectral_pack, pack_info, shop_item_type, shop_rates_for_deck, target_joker_pools,
    weighted_packs,
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
        KernelShape::PackOnly => pack_only(state, cfg),
        KernelShape::Observatory => observatory(state, cfg),
        KernelShape::ShopJoker => shop_joker(state, cfg),
        KernelShape::PackJoker => pack_joker(state, cfg),
        KernelShape::AnyJoker => any_joker(state, cfg),
        KernelShape::Souls => souls(state, cfg),
        KernelShape::Perkeo => perkeo(state, cfg),
        KernelShape::Erratic => erratic(state, cfg),
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
    let big = randchoice_tag(state);
    match (cfg.raw.tag1, cfg.raw.tag2) {
        (Item::RETRY, Item::RETRY) => true,
        (Item::RETRY, tag) | (tag, Item::RETRY) => small == tag || big == tag,
        (tag1, tag2) if tag1 != tag2 => {
            (small == tag1 || big == tag1) && (small == tag2 || big == tag2)
        },
        (tag, _) => small == tag && big == tag,
    }
}

fn voucher_only(state: &mut SearchState, cfg: &CompiledFilter) -> bool {
    next_voucher(state, cfg.raw.deck) == cfg.raw.voucher
}

fn pack_only(state: &mut SearchState, cfg: &CompiledFilter) -> bool {
    cfg.raw.pack == Item::Buffoon_Pack || roll_second_pack(state) == cfg.raw.pack
}

fn observatory(state: &mut SearchState, cfg: &CompiledFilter) -> bool {
    if next_voucher(state, cfg.raw.deck) != Item::Telescope {
        return false;
    }
    roll_second_pack(state) == Item::Mega_Celestial_Pack
}

fn shop_joker(state: &mut SearchState, cfg: &CompiledFilter) -> bool {
    if cfg.target_joker_pools & (POOL_COMMON | POOL_UNCOMMON | POOL_RARE) == 0 {
        return false;
    }
    shop_has_joker(state, cfg.raw.joker, cfg.target_joker_pools, cfg.raw.deck)
}

fn pack_joker(state: &mut SearchState, cfg: &CompiledFilter) -> bool {
    if cfg.target_joker_pools & (POOL_COMMON | POOL_UNCOMMON | POOL_RARE) == 0 {
        return false;
    }
    let packs = pack_slots(state, cfg.needs_packs);
    packs.into_iter().any(|pack| {
        pack != Item::RETRY
            && (cfg.raw.pack == Item::RETRY || pack == cfg.raw.pack)
            && is_buffoon_pack(pack)
            && buffoon_pack_has_joker(state, pack, cfg.raw.joker, cfg.raw.deck)
    })
}

fn any_joker(state: &mut SearchState, cfg: &CompiledFilter) -> bool {
    let packs = if cfg.raw.pack == Item::RETRY {
        [Item::RETRY; 2]
    } else {
        let packs = pack_slots(state, cfg.needs_packs);
        if !packs.contains(&cfg.raw.pack) {
            return false;
        }
        packs
    };

    if cfg.wants_joker_shop && shop_joker(state, cfg) {
        return true;
    }
    if !cfg.wants_joker_pack {
        return false;
    }
    if cfg.raw.pack == Item::RETRY {
        return pack_joker(state, cfg);
    }
    packs.into_iter().any(|pack| {
        pack != Item::RETRY
            && pack == cfg.raw.pack
            && is_buffoon_pack(pack)
            && buffoon_pack_has_joker(state, pack, cfg.raw.joker, cfg.raw.deck)
    })
}

fn souls(state: &mut SearchState, cfg: &CompiledFilter) -> bool {
    if !cfg.selected_soulable_pack {
        return false;
    }
    let packs = pack_slots(state, cfg.needs_packs);
    let mut souls_found = 0_i64;
    for pack in packs {
        if pack == Item::RETRY || (cfg.raw.pack != Item::RETRY && pack != cfg.raw.pack) {
            continue;
        }
        if !is_soulable_pack(pack) {
            continue;
        }
        let max_possible = pack_info(pack).size as i64;
        if souls_found + max_possible < cfg.raw.souls {
            continue;
        }
        souls_found += i64::from(count_souls_in_pack(state, pack, cfg.raw.deck));
        if souls_found >= cfg.raw.souls {
            return true;
        }
    }
    souls_found >= cfg.raw.souls
}

fn perkeo(state: &mut SearchState, cfg: &CompiledFilter) -> bool {
    let packs = pack_slots(state, cfg.needs_packs);
    for pack in packs {
        if pack == Item::RETRY || (cfg.raw.pack != Item::RETRY && pack != cfg.raw.pack) {
            continue;
        }
        if !is_soulable_pack(pack) {
            continue;
        }
        let souls_in_pack = count_souls_in_pack(state, pack, cfg.raw.deck);
        if souls_in_pack == 0 {
            continue;
        }
        let uses = usize::from(souls_in_pack).min(pack_info(pack).choices);
        for _ in 0..uses {
            if soul_yields_perkeo(state, cfg.raw.deck) {
                return true;
            }
        }
    }
    false
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

    let mut total = 0_i32;
    let mut face_count = 0_i32;
    let mut suit_count = [0_i32; 4];
    for drawn in 0..52 {
        let idx = state
            .rng
            .randint(RngKey::Erratic, &mut state.seed, state.hashed_seed, 0, 51)
            as usize;
        let (is_face, suit) = card_face_and_suit(idx);
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
        let remaining = 51 - drawn;
        if cfg.raw.min_face_cards > 0 && face_count + remaining < cfg.raw.min_face_cards {
            return false;
        }
    }

    if cfg.raw.min_face_cards > 0 && face_count < cfg.raw.min_face_cards {
        return false;
    }
    if cfg.raw.suit_ratio > 0.0 {
        if total == 0 {
            return false;
        }
        let mut first = 0_i32;
        let mut second = 0_i32;
        for count in suit_count {
            if count >= first {
                second = first;
                first = count;
            } else if count > second {
                second = count;
            }
        }
        return f64::from(first + second) / f64::from(total) >= cfg.raw.suit_ratio;
    }
    true
}

fn composite(state: &mut SearchState, cfg: &CompiledFilter) -> bool {
    if (cfg.raw.tag1 != Item::RETRY || cfg.raw.tag2 != Item::RETRY) && !tag_only(state, cfg) {
        return false;
    }

    let mut first_voucher = Item::RETRY;
    if cfg.raw.voucher != Item::RETRY || cfg.raw.observatory {
        first_voucher = next_voucher(state, cfg.raw.deck);
        if cfg.raw.voucher != Item::RETRY && first_voucher != cfg.raw.voucher {
            return false;
        }
    }

    let packs = pack_slots(state, cfg.needs_packs);
    if cfg.raw.pack != Item::RETRY && !packs.contains(&cfg.raw.pack) {
        return false;
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

    if (cfg.raw.souls > 0 || cfg.raw.perkeo) && !composite_has_souls_and_perkeo(state, cfg, packs) {
        return false;
    }

    erratic(state, cfg)
}

fn composite_has_joker(state: &mut SearchState, cfg: &CompiledFilter, packs: [Item; 2]) -> bool {
    if cfg.wants_joker_shop
        && shop_has_joker(state, cfg.raw.joker, cfg.target_joker_pools, cfg.raw.deck)
    {
        return true;
    }
    if !cfg.wants_joker_pack {
        return false;
    }

    packs.into_iter().any(|pack| {
        pack != Item::RETRY
            && (cfg.raw.pack == Item::RETRY || pack == cfg.raw.pack)
            && is_buffoon_pack(pack)
            && buffoon_pack_has_joker(state, pack, cfg.raw.joker, cfg.raw.deck)
    })
}

fn composite_has_souls_and_perkeo(
    state: &mut SearchState,
    cfg: &CompiledFilter,
    packs: [Item; 2],
) -> bool {
    let mut souls_found = 0_i64;
    let mut perkeo_found = !cfg.raw.perkeo;
    for pack in packs {
        if pack == Item::RETRY || (cfg.raw.pack != Item::RETRY && pack != cfg.raw.pack) {
            continue;
        }
        if !is_soulable_pack(pack) {
            continue;
        }

        let souls_in_pack = count_souls_in_pack(state, pack, cfg.raw.deck);
        if souls_in_pack == 0 {
            continue;
        }
        souls_found += i64::from(souls_in_pack);

        if cfg.raw.perkeo {
            let uses = usize::from(souls_in_pack).min(pack_info(pack).choices);
            for _ in 0..uses {
                if soul_yields_perkeo(state, cfg.raw.deck) {
                    perkeo_found = true;
                    break;
                }
            }
            if perkeo_found && (cfg.raw.souls <= 0 || souls_found >= cfg.raw.souls) {
                break;
            }
        }
    }

    (cfg.raw.souls <= 0 || souls_found >= cfg.raw.souls) && perkeo_found
}

fn pack_slots(state: &mut SearchState, needed: bool) -> [Item; 2] {
    if needed {
        [Item::Buffoon_Pack, roll_second_pack(state)]
    } else {
        [Item::RETRY; 2]
    }
}

fn roll_second_pack(state: &mut SearchState) -> Item {
    let poll = state
        .rng
        .random(RngKey::ShopPack1, &mut state.seed, state.hashed_seed)
        * weighted_packs()[0].weight;
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

fn next_voucher(state: &mut SearchState, deck: Item) -> Item {
    if deck == Item::Red_Deck {
        return randchoice_unlocked(state, RngKey::Voucher1, VOUCHER_POOL);
    }
    randchoice(
        state,
        RngKey::Voucher1,
        VOUCHER_POOL,
        &Locks::for_deck(deck),
    )
}

fn shop_has_joker(state: &mut SearchState, target: Item, target_pools: u8, deck: Item) -> bool {
    let rates = shop_rates_for_deck(deck);
    for _ in 0..2 {
        let poll = state
            .rng
            .random(RngKey::Cdt1, &mut state.seed, state.hashed_seed)
            * rates.total();
        if shop_item_type(rates, poll) != Item::T_Joker {
            continue;
        }
        if next_joker_item(state, JokerSource::Shop, target_pools, deck) == target {
            return true;
        }
    }
    false
}

fn buffoon_pack_has_joker(state: &mut SearchState, pack: Item, target: Item, deck: Item) -> bool {
    let info = pack_info(pack);
    let target_pools = target_joker_pools(target);
    let mut locks = Locks::for_deck(deck);
    let mut generated = [Item::RETRY; 5];
    for slot in 0..info.size {
        let joker = next_joker_item_with_locks(state, JokerSource::Buffoon, target_pools, &locks);
        if joker == target {
            return true;
        }
        locks.lock(joker);
        generated[slot] = joker;
    }
    for joker in generated.into_iter().take(info.size) {
        locks.unlock(joker);
    }
    false
}

fn count_souls_in_pack(state: &mut SearchState, pack: Item, deck: Item) -> u8 {
    let info = pack_info(pack);
    let mut locks = Locks::for_deck(deck);
    let mut generated = [Item::RETRY; 5];
    let mut count = 0_u8;
    for slot in 0..info.size {
        let item = if is_arcana_pack(pack) {
            next_tarot(state, &locks)
        } else if is_spectral_pack(pack) {
            next_spectral(state, &locks)
        } else {
            Item::RETRY
        };
        if item == Item::The_Soul {
            count += 1;
        }
        locks.lock(item);
        generated[slot] = item;
    }
    for item in generated.into_iter().take(info.size) {
        locks.unlock(item);
    }
    count
}

fn next_tarot(state: &mut SearchState, locks: &Locks) -> Item {
    if !locks.is_locked(Item::The_Soul)
        && state
            .rng
            .random(RngKey::SoulTarot1, &mut state.seed, state.hashed_seed)
            > 0.997
    {
        return Item::The_Soul;
    }
    randchoice(state, RngKey::TarotArcana1, TAROT_POOL, locks)
}

fn next_spectral(state: &mut SearchState, locks: &Locks) -> Item {
    let mut forced = Item::RETRY;
    if !locks.is_locked(Item::The_Soul)
        && state
            .rng
            .random(RngKey::SoulSpectral1, &mut state.seed, state.hashed_seed)
            > 0.997
    {
        forced = Item::The_Soul;
    }
    if !locks.is_locked(Item::Black_Hole)
        && state
            .rng
            .random(RngKey::SoulSpectral1, &mut state.seed, state.hashed_seed)
            > 0.997
    {
        forced = Item::Black_Hole;
    }
    if forced != Item::RETRY {
        return forced;
    }
    randchoice(state, RngKey::SpectralPack1, SPECTRAL_POOL, locks)
}

fn soul_yields_perkeo(state: &mut SearchState, deck: Item) -> bool {
    randchoice(
        state,
        RngKey::JokerLegendary,
        LEGENDARY_JOKER_POOL,
        &Locks::for_deck(deck),
    ) == Item::Perkeo
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
    deck: Item,
) -> Item {
    next_joker_item_with_locks(state, source, target_pools, &Locks::for_deck(deck))
}

fn next_joker_item_with_locks(
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

    let base = key.name();
    let mut resample = 2;
    loop {
        let resample_key = format!("{base}_resample{resample}");
        let idx = state.rng.randint_dynamic(
            &resample_key,
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

    let mut resample = 2;
    loop {
        let resample_key = format!("Tag1_resample{resample}");
        let idx = state.rng.randint_dynamic(
            &resample_key,
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

fn randchoice_unlocked(state: &mut SearchState, key: RngKey, items: &[Item]) -> Item {
    let idx = state.rng.randint(
        key,
        &mut state.seed,
        state.hashed_seed,
        0,
        items.len() as i32 - 1,
    ) as usize;
    let item = items[idx];
    if item != Item::RETRY {
        return item;
    }

    let base = key.name();
    let mut resample = 2;
    loop {
        let resample_key = format!("{base}_resample{resample}");
        let idx = state.rng.randint_dynamic(
            &resample_key,
            &mut state.seed,
            state.hashed_seed,
            0,
            items.len() as i32 - 1,
        ) as usize;
        let candidate = items[idx];
        resample += 1;
        if candidate != Item::RETRY || resample > 1000 {
            return candidate;
        }
    }
}
