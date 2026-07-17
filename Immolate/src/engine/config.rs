use crate::engine::tables::{
    Locks, STANDARD_JOKER_POOLS, is_ante1_locked_tag, is_buffoon_pack, is_soulable_pack,
    is_spectral_pack, pack_info, target_joker_pools,
};
use crate::filters::{FilterConfig, JokerLocation};
use crate::item::Item;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KernelShape {
    NoMatch,
    NoFilter,
    TagOnly,
    VoucherOnly,
    PackOnly,
    Observatory,
    ShopJoker,
    PackJoker,
    AnyJoker,
    Souls,
    Perkeo,
    Erratic,
    TagObservatory,
    SpectralSoulPerkeo,
    Composite,
    Generic,
    VoucherSecondPack,
}

#[derive(Clone, Debug)]
pub struct CompiledFilter {
    pub raw: FilterConfig,
    pub shape: KernelShape,
    pub wants_joker_shop: bool,
    pub wants_joker_pack: bool,
    pub target_joker_pools: u8,
    pub selected_soulable_pack: bool,
    pub base_locks: Locks,
}

impl CompiledFilter {
    pub fn compile(raw: &FilterConfig) -> Self {
        let mut raw = *raw;
        if raw.erratic && !raw.no_faces && raw.suit_ratio <= 0.5 {
            raw.suit_ratio = 0.0;
        }
        let wants_joker = raw.joker != Item::RETRY;
        let wants_joker_shop =
            wants_joker && matches!(raw.joker_location, JokerLocation::Shop | JokerLocation::Any);
        let wants_joker_pack =
            wants_joker && matches!(raw.joker_location, JokerLocation::Pack | JokerLocation::Any);
        let target_joker_pools = target_joker_pools(raw.joker);
        let base_locks = Locks::for_deck(raw.deck);

        Self {
            raw,
            shape: classify(
                &raw,
                wants_joker_shop,
                wants_joker_pack,
                target_joker_pools,
                &base_locks,
            ),
            wants_joker_shop,
            wants_joker_pack,
            target_joker_pools,
            selected_soulable_pack: raw.pack == Item::RETRY || is_soulable_pack(raw.pack),
            base_locks,
        }
    }

    pub const fn chunk_size(&self) -> i64 {
        match self.shape {
            KernelShape::Erratic => 512,
            KernelShape::Composite => 512,
            // Soul+Perkeo work is heavy enough for finer participation and cancellation.
            KernelShape::SpectralSoulPerkeo => 1_024,
            // Nearby voucher/second-pack hits benefit from tighter cancellation granularity.
            KernelShape::VoucherSecondPack => 1_024,
            // Long-tail dual-tag Observatory searches need fine cancellation granularity.
            KernelShape::TagObservatory => 1_024,
            KernelShape::ShopJoker
            | KernelShape::PackJoker
            | KernelShape::AnyJoker
            | KernelShape::Souls
            | KernelShape::Perkeo => 4_096,
            _ => 8_192,
        }
    }

    pub const fn serial_prefix_size(&self) -> i64 {
        // Erratic checks dominate per-seed cost even when cheaper filters run first.
        if self.raw.erratic {
            return 256;
        }
        match self.shape {
            KernelShape::SpectralSoulPerkeo => 1_024,
            KernelShape::PackJoker | KernelShape::Souls | KernelShape::TagObservatory => 8_192,
            _ => 4_096,
        }
    }

    pub const fn auto_thread_limit(&self) -> usize {
        match self.shape {
            KernelShape::Erratic => 16,
            KernelShape::Composite
            | KernelShape::SpectralSoulPerkeo
            | KernelShape::ShopJoker
            | KernelShape::PackJoker
            | KernelShape::AnyJoker
            | KernelShape::Souls
            | KernelShape::Perkeo => 16,
            // Voucher + rolled second-pack searches find nearby hits where more workers cost extra.
            KernelShape::VoucherSecondPack => 8,
            // Dual-tag Observatory checks have enough work to use the full complex-shape cap.
            KernelShape::TagObservatory => 16,
            _ => 4,
        }
    }

    pub const fn parallel_threshold(&self) -> i64 {
        match self.shape {
            KernelShape::Erratic => 8_192,
            KernelShape::Composite if self.raw.erratic => 8_192,
            _ => 32_768,
        }
    }
}

fn classify(
    raw: &FilterConfig,
    wants_joker_shop: bool,
    wants_joker_pack: bool,
    target_joker_pools: u8,
    deck_locks: &Locks,
) -> KernelShape {
    let has_tags = raw.tag1 != Item::RETRY || raw.tag2 != Item::RETRY;
    let has_voucher = raw.voucher != Item::RETRY;
    let has_pack = raw.pack != Item::RETRY;
    let has_joker = raw.joker != Item::RETRY;
    let has_souls = raw.souls > 0;
    let has_erratic = raw.erratic && (raw.min_face_cards > 0 || raw.suit_ratio > 0.0);

    if is_static_no_match(
        raw,
        has_joker,
        wants_joker_shop,
        wants_joker_pack,
        target_joker_pools,
        deck_locks,
    ) {
        return KernelShape::NoMatch;
    }

    if !has_tags
        && !has_voucher
        && !has_pack
        && !raw.observatory
        && !has_joker
        && !has_souls
        && !raw.perkeo
        && !has_erratic
    {
        return KernelShape::NoFilter;
    }
    if has_tags {
        if raw.observatory
            && !has_voucher
            && !has_pack
            && !has_joker
            && !has_souls
            && !raw.perkeo
            && !has_erratic
        {
            return KernelShape::TagObservatory;
        }
        return if has_voucher
            || has_pack
            || raw.observatory
            || has_joker
            || has_souls
            || raw.perkeo
            || has_erratic
        {
            KernelShape::Composite
        } else {
            KernelShape::TagOnly
        };
    }
    if has_voucher {
        return if has_pack
            && !raw.observatory
            && !has_joker
            && !has_souls
            && !raw.perkeo
            && !has_erratic
        {
            if raw.pack == Item::Buffoon_Pack {
                // The first shop slot is always this pack, so only the voucher can reject.
                KernelShape::VoucherOnly
            } else {
                KernelShape::VoucherSecondPack
            }
        } else if has_pack || raw.observatory || has_joker || has_souls || raw.perkeo || has_erratic
        {
            KernelShape::Composite
        } else {
            KernelShape::VoucherOnly
        };
    }
    if has_erratic {
        return if has_pack || raw.observatory || has_joker || has_souls || raw.perkeo {
            KernelShape::Composite
        } else {
            KernelShape::Erratic
        };
    }
    if raw.observatory {
        return if has_pack || has_joker || has_souls || raw.perkeo {
            KernelShape::Composite
        } else {
            KernelShape::Observatory
        };
    }
    if has_joker {
        if has_souls || raw.perkeo {
            return KernelShape::Composite;
        }
        if has_pack && wants_joker_shop && !wants_joker_pack {
            return KernelShape::Composite;
        }
        return if wants_joker_shop && wants_joker_pack {
            KernelShape::AnyJoker
        } else if wants_joker_shop {
            KernelShape::ShopJoker
        } else {
            KernelShape::PackJoker
        };
    }
    if raw.perkeo && has_pack && is_spectral_pack(raw.pack) {
        return KernelShape::SpectralSoulPerkeo;
    }
    if has_souls {
        return if raw.perkeo {
            KernelShape::Composite
        } else {
            KernelShape::Souls
        };
    }
    if raw.perkeo {
        return KernelShape::Perkeo;
    }
    if has_pack {
        return KernelShape::PackOnly;
    }
    KernelShape::Generic
}

fn is_static_no_match(
    raw: &FilterConfig,
    has_joker: bool,
    wants_joker_shop: bool,
    wants_joker_pack: bool,
    target_joker_pools: u8,
    deck_locks: &Locks,
) -> bool {
    if has_joker
        && (wants_joker_shop || wants_joker_pack)
        && target_joker_pools & STANDARD_JOKER_POOLS == 0
    {
        return true;
    }

    if is_ante1_locked_tag(raw.tag1) || is_ante1_locked_tag(raw.tag2) {
        return true;
    }

    if raw.voucher != Item::RETRY && deck_locks.is_locked(raw.voucher) {
        return true;
    }

    if raw.observatory {
        if deck_locks.is_locked(Item::Telescope) {
            return true;
        }
        if raw.voucher != Item::RETRY && raw.voucher != Item::Telescope {
            return true;
        }
        if raw.pack != Item::RETRY
            && !matches!(raw.pack, Item::Buffoon_Pack | Item::Mega_Celestial_Pack)
        {
            return true;
        }
        if raw.souls > 0 || raw.perkeo {
            return true;
        }
    }

    if has_joker
        && wants_joker_pack
        && !wants_joker_shop
        && raw.pack != Item::RETRY
        && !is_buffoon_pack(raw.pack)
    {
        return true;
    }

    if raw.souls > 0 {
        if raw.souls > 1 {
            return true;
        }
        if raw.pack != Item::RETRY {
            return !is_soulable_pack(raw.pack) || raw.souls > pack_info(raw.pack).size as i64;
        }
    }

    if raw.perkeo && raw.pack != Item::RETRY && !is_soulable_pack(raw.pack) {
        return true;
    }

    if raw.erratic && (raw.min_face_cards > 52 || raw.suit_ratio > 1.0) {
        return true;
    }

    raw.erratic && raw.no_faces && raw.min_face_cards > 0
}
