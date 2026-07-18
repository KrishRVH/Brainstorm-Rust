#[cfg(test)]
// The shared catalog has fields and selectors used by the benchmark binaries,
// but the library's source-oracle tests consume only part of it.
#[allow(dead_code)]
mod bench_cases;
mod engine;
mod ffi;
mod filters;
mod instance;
mod item;
mod rng;
mod search;
mod seed;

pub use engine::config::CompiledFilter;
pub use engine::search::brainstorm_search_core;
pub use filters::FilterConfig;
pub use seed::{SEED_SPACE, Seed};

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, unsafe_code)]

    use std::ffi::CStr;

    use super::*;
    use crate::engine::config::CompiledFilter;
    use crate::engine::kernels::apply_compiled_filter;
    use crate::engine::rng::RngKey;
    use crate::engine::seed::SearchState;
    use crate::engine::tables::{
        STANDARD_JOKER_POOLS, TAG_POOL, VOUCHER_POOL, is_ante1_locked_tag,
        is_first_shop_possible_joker, target_joker_pools,
    };
    use crate::ffi::{brainstorm_search, free_result};
    use crate::filters::FilterConfig;
    use crate::instance::Instance;
    use crate::item::{
        COMMON_JOKERS, COMMON_JOKERS_100, ITEM_COUNT, Item, LEGENDARY_JOKERS, RARE_JOKERS,
        RARE_JOKERS_100, UNCOMMON_JOKERS, UNCOMMON_JOKERS_100, item_to_string, string_to_item,
    };
    use crate::rng::{LuaRandom, fract, pseudohash_from, pseudostep, round13};
    use crate::search::resolve_seed_budget;
    use crate::seed::Seed;

    #[test]
    fn seed_order_starts_with_expected_prefix() {
        let mut seed = Seed::default();
        assert_eq!(seed.to_string(), "");
        seed.next();
        assert_eq!(seed.to_string(), "1");
        seed.next();
        assert_eq!(seed.to_string(), "11");
    }

    #[test]
    fn seed_id_roundtrip_for_basic_seed() {
        for id in [0, 1, 2, 35, 36, 37, 1_000, 1_000_000] {
            let seed = Seed::from_id(id);
            assert_eq!(Seed::from(seed.to_string().as_str()).id(), id);
        }
    }

    #[test]
    fn seed_id_strings_match_golden_vectors() {
        let cases = [
            (0, ""),
            (1, "1"),
            (2, "11"),
            (35, "S1111111"),
            (36, "T1111111"),
            (37, "U1111111"),
            (1_000, "LS111111"),
            (1_000_000, "ZZNN1111"),
        ];
        for (id, expected) in cases {
            assert_eq!(Seed::from_id(id).to_string(), expected);
        }
    }

    #[test]
    fn seed_id_normalizes_at_seed_space_boundary() {
        assert_eq!(Seed::from_id(crate::seed::SEED_SPACE).to_string(), "");
        assert_eq!(Seed::from_id(crate::seed::SEED_SPACE + 1).to_string(), "1");
        assert_eq!(Seed::from_id(-1).to_string(), "ZZZZZZZZ");
    }

    #[test]
    fn seed_budget_resolution_is_explicit() {
        assert_eq!(resolve_seed_budget(0), 100_000_000);
        assert_eq!(resolve_seed_budget(-1), 100_000_000);
        assert_eq!(resolve_seed_budget(42), 42);
        assert_eq!(
            resolve_seed_budget(crate::seed::SEED_SPACE + 1),
            crate::seed::SEED_SPACE
        );
    }

    #[test]
    fn no_filter_shapes_preserve_canonical_start_for_every_scheduler_input() {
        use crate::engine::config::KernelShape;

        let forced_buffoon = raw_cfg(
            "",
            "p_buffoon_normal_1",
            "",
            "",
            "",
            "any",
            0.0,
            false,
            false,
            "b_red",
            false,
            false,
            0,
            0.0,
        );
        for cfg in [FilterConfig::default(), forced_buffoon] {
            assert_eq!(CompiledFilter::compile(&cfg).shape, KernelShape::NoFilter);
            for (start, canonical) in [
                ("", ""),
                ("1", "1"),
                ("?", ""),
                ("1?Z", "1Z"),
                ("123456789ABC", "12345678"),
                ("é1234567", "123456"),
                ("ZZZZZZZZ", "ZZZZZZZZ"),
            ] {
                for budget in [-1, 0, 1, SEED_SPACE + 1] {
                    for threads in [i32::MIN, -1, 0, 1, 2, 16, i32::MAX] {
                        assert_eq!(
                            brainstorm_search_core(start, &cfg, budget, threads).as_deref(),
                            Some(canonical),
                            "start={start:?} budget={budget} threads={threads}",
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn parsers_match_current_defaults_for_unknowns() {
        let cfg = FilterConfig::from_raw(
            "unknown", "unknown", "unknown", "", "unknown", "weird", 0.0, false, false, "unknown",
            false, false, -1, 2.0,
        );
        assert_eq!(cfg.voucher, Item::RETRY);
        assert_eq!(cfg.pack, Item::RETRY);
        assert_eq!(cfg.tag1, Item::RETRY);
        assert_eq!(cfg.joker, Item::RETRY);
        assert_eq!(cfg.deck, Item::Red_Deck);
        assert_eq!(cfg.min_face_cards, 0);
        assert_eq!(cfg.suit_ratio, 1.0);

        assert_eq!(
            FilterConfig::from_raw(
                "", "", "", "", "j_caino", "any", 0.0, false, false, "b_red", false, false, 0, 0.0
            )
            .joker,
            Item::Canio
        );
        assert_eq!(
            FilterConfig::from_raw(
                "", "", "", "", "Caino", "any", 0.0, false, false, "b_red", false, false, 0, 0.0
            )
            .joker,
            Item::Canio
        );
        assert_eq!(
            FilterConfig::from_raw(
                "", "", "", "", "j_seance", "any", 0.0, false, false, "b_red", false, false, 0, 0.0
            )
            .joker,
            Item::Seance
        );
        assert_eq!(
            FilterConfig::from_raw(
                "", "", "", "", "Seance", "any", 0.0, false, false, "b_red", false, false, 0, 0.0
            )
            .joker,
            Item::Seance
        );
    }

    #[test]
    fn pack_keys_accept_numeric_instance_suffixes() {
        for (key, expected) in [
            ("p_arcana_normal", Item::Arcana_Pack),
            ("p_arcana_normal_1", Item::Arcana_Pack),
            ("p_arcana_normal_42", Item::Arcana_Pack),
            ("p_arcana_normal_", Item::RETRY),
            ("p_arcana_normal_1x", Item::RETRY),
            ("p_arcana_normal_1_2", Item::RETRY),
        ] {
            assert_eq!(
                FilterConfig::from_raw(
                    "", key, "", "", "", "any", 0.0, false, false, "b_red", false, false, 0, 0.0,
                )
                .pack,
                expected,
                "pack key {key:?}",
            );
        }
    }

    #[test]
    fn item_layout_preserves_catalog_discriminants() {
        assert_eq!(std::mem::size_of::<Item>(), std::mem::size_of::<u16>());
        assert_eq!(Item::Seance.idx(), 85);
        assert_eq!(ITEM_COUNT, 507);
    }

    #[test]
    fn seance_mapping_matches_the_runtime_catalog_name() {
        assert_eq!(item_to_string(Item::Seance), "Seance");
        assert_eq!(string_to_item("Seance"), Item::Seance);
        assert_eq!(string_to_item("S\u{398}ance"), Item::RETRY);
    }

    #[test]
    fn rng_smoke_is_stable() {
        assert_eq!(pseudohash_from("", 1.0), 1.0);
        let mut rng = LuaRandom::new(0.5);
        let first = rng.random();
        assert!((0.0..1.0).contains(&first));
    }

    #[test]
    fn rng_vectors_match_golden_vectors() {
        let hash_cases = [
            ("", 1.0),
            ("1", 0.15694342689690188),
            ("11", 0.68745689631282403),
            ("ABCDE", 0.55659692676272243),
            ("Tag1", 0.47049862973562995),
            ("shop_pack1", 0.39373360824367865),
            ("soul_Spectral1", 0.24677008613650742),
        ];
        for (input, expected) in hash_cases {
            assert_close(pseudohash_from(input, 1.0), expected);
        }

        let mut rng = LuaRandom::new(0.5);
        assert_close(rng.random(), 0.09657393438653461);
        assert_close(rng.random(), 0.96226945770684003);
    }

    #[test]
    fn rng_key_names_match_balatro_goldens() {
        let cases = [
            (RngKey::Tag1, "Tag1"),
            (RngKey::Voucher1, "Voucher1"),
            (RngKey::ShopPack1, "shop_pack1"),
            (RngKey::Cdt1, "cdt1"),
            (RngKey::RarityShop1, "rarity1sho"),
            (RngKey::RarityBuffoon1, "rarity1buf"),
            (RngKey::JokerCommonShop1, "Joker1sho1"),
            (RngKey::JokerUncommonShop1, "Joker2sho1"),
            (RngKey::JokerRareShop1, "Joker3sho1"),
            (RngKey::JokerCommonBuffoon1, "Joker1buf1"),
            (RngKey::JokerUncommonBuffoon1, "Joker2buf1"),
            (RngKey::JokerRareBuffoon1, "Joker3buf1"),
            (RngKey::JokerLegendary, "Joker4"),
            (RngKey::SoulTarot1, "soul_Tarot1"),
            (RngKey::SoulSpectral1, "soul_Spectral1"),
            (RngKey::Erratic, "erratic"),
        ];
        for (key, expected) in cases {
            assert_eq!(key.name(), expected);
        }
    }

    #[test]
    fn rng_primitive_goldens_are_stable() {
        assert_close(fract(12.3456789012345), 0.34567890123449985);
        assert_close(round13(0.12345678901234567), 0.1234567890123);
        assert_close(
            pseudohash_from("Tag1", 0.15694342689690188),
            0.61303326083861975,
        );
        assert_close(pseudostep(b'1', 1, 1.0), 0.15694342689690188);
    }

    #[test]
    fn search_vectors_match_golden_vectors() {
        let empty = FilterConfig::default();
        assert_eq!(
            brainstorm_search_core("", &empty, 1, 1).as_deref(),
            Some("")
        );
        assert_eq!(
            brainstorm_search_core("1", &empty, 1, 1).as_deref(),
            Some("1")
        );

        let tag = FilterConfig::from_raw(
            "",
            "",
            "tag_charm",
            "",
            "",
            "any",
            0.0,
            false,
            false,
            "b_red",
            false,
            false,
            0,
            0.0,
        );
        assert_eq!(
            brainstorm_search_core("", &tag, 10_000, 1).as_deref(),
            Some("21111111"),
        );

        let voucher = FilterConfig::from_raw(
            "v_telescope",
            "",
            "",
            "",
            "",
            "any",
            0.0,
            false,
            false,
            "b_red",
            false,
            false,
            0,
            0.0,
        );
        assert_eq!(
            brainstorm_search_core("", &voucher, 10_000, 1).as_deref(),
            Some("P1111111"),
        );

        let pack = FilterConfig::from_raw(
            "",
            "p_spectral_mega_1",
            "",
            "",
            "",
            "any",
            0.0,
            false,
            false,
            "b_red",
            false,
            false,
            0,
            0.0,
        );
        assert_eq!(
            brainstorm_search_core("", &pack, 10_000, 1).as_deref(),
            Some("Z2111111"),
        );

        let observatory = FilterConfig::from_raw(
            "", "", "", "", "", "any", 0.0, true, false, "b_red", false, false, 0, 0.0,
        );
        assert_eq!(
            brainstorm_search_core("", &observatory, 100_000, 1).as_deref(),
            Some("S111111"),
        );

        let erratic = FilterConfig::from_raw(
            "",
            "",
            "",
            "",
            "",
            "any",
            0.0,
            false,
            false,
            "b_erratic",
            true,
            false,
            12,
            0.0,
        );
        assert_eq!(
            brainstorm_search_core("", &erratic, 10_000, 1).as_deref(),
            Some("11"),
        );
    }

    #[test]
    fn current_core_matches_composite_goldens() {
        use crate::engine::config::{CompiledFilter, KernelShape};

        let cases = [
            (
                "souls+perkeo",
                FilterConfig::from_raw(
                    "", "", "", "", "", "any", 1.0, false, true, "b_red", false, false, 0, 0.0,
                ),
                100_000,
                Some("MZ111111"),
            ),
            (
                "shop-joker+perkeo",
                FilterConfig::from_raw(
                    "",
                    "",
                    "",
                    "",
                    "Burnt Joker",
                    "shop",
                    0.0,
                    false,
                    true,
                    "b_red",
                    false,
                    false,
                    0,
                    0.0,
                ),
                50_000,
                None,
            ),
            (
                "joker+observatory",
                FilterConfig::from_raw(
                    "",
                    "",
                    "",
                    "",
                    "Riff-raff",
                    "any",
                    0.0,
                    true,
                    false,
                    "b_red",
                    false,
                    false,
                    0,
                    0.0,
                ),
                100_000,
                Some("L3411111"),
            ),
            (
                "pack+shop-joker",
                FilterConfig::from_raw(
                    "",
                    "p_spectral_mega_1",
                    "",
                    "",
                    "Burnt Joker",
                    "shop",
                    0.0,
                    false,
                    false,
                    "b_red",
                    false,
                    false,
                    0,
                    0.0,
                ),
                50_000,
                None,
            ),
        ];

        for (name, cfg, budget, expected_seed) in cases {
            assert_eq!(
                CompiledFilter::compile(&cfg).shape,
                KernelShape::Composite,
                "{name} should use composite engine kernel",
            );
            assert_eq!(
                brainstorm_search_core("", &cfg, budget, 1).as_deref(),
                expected_seed,
                "current Rust mismatch for composite case {name}",
            );
        }
    }

    #[test]
    fn voucher_with_forced_buffoon_pack_matches_source_oracle() {
        use crate::engine::config::{CompiledFilter, KernelShape};

        let cfg = FilterConfig::from_raw(
            "v_telescope",
            "p_buffoon_normal_1",
            "",
            "",
            "",
            "any",
            0.0,
            false,
            false,
            "b_red",
            false,
            false,
            0,
            0.0,
        );

        assert_eq!(
            CompiledFilter::compile(&cfg).shape,
            KernelShape::VoucherOnly
        );
        let rolled_pack = filter_config_from_benchmark(&benchmark_case("ux-voucher-pack"));
        assert_eq!(
            CompiledFilter::compile(&rolled_pack).shape,
            KernelShape::VoucherSecondPack
        );
        assert_core_matches_source_oracle("voucher+forced Buffoon", "", &cfg, 100_000);
        assert_eq!(
            brainstorm_search_core("", &cfg, 100_000, 1).as_deref(),
            Some("P1111111"),
        );
    }

    #[test]
    fn forced_normal_buffoon_is_nonselective_across_specialized_shapes() {
        use crate::engine::config::KernelShape;
        use crate::filters::JokerLocation;

        let forced_buffoon = FilterConfig {
            pack: Item::Buffoon_Pack,
            ..FilterConfig::default()
        };

        let cases = [
            (
                "tag + forced Buffoon",
                "21111111",
                FilterConfig {
                    tag1: Item::Charm_Tag,
                    ..forced_buffoon
                },
                KernelShape::TagOnly,
            ),
            (
                "Observatory + forced Buffoon",
                "S111111",
                FilterConfig {
                    observatory: true,
                    ..forced_buffoon
                },
                KernelShape::Observatory,
            ),
            (
                "tag + Observatory + forced Buffoon",
                "U8411111",
                FilterConfig {
                    tag1: Item::Charm_Tag,
                    observatory: true,
                    ..forced_buffoon
                },
                KernelShape::TagObservatory,
            ),
            (
                "shop Joker + forced Buffoon",
                "Q2111111",
                FilterConfig {
                    joker: Item::Burnt_Joker,
                    joker_location: JokerLocation::Shop,
                    ..forced_buffoon
                },
                KernelShape::ShopJoker,
            ),
            (
                "Erratic + forced Buffoon",
                "11",
                FilterConfig {
                    deck: Item::Erratic_Deck,
                    erratic: true,
                    min_face_cards: 12,
                    ..forced_buffoon
                },
                KernelShape::Erratic,
            ),
        ];

        for (name, target, cfg, shape) in cases {
            assert_eq!(CompiledFilter::compile(&cfg).shape, shape, "{name}");
            assert_seed_passes_like_source(name, target, &cfg);
            for seed_start in ["", "KRVH1234", "ZZZYZZZZ"] {
                assert_predicate_matches_source_oracle_window(name, seed_start, &cfg, 256);
            }
        }
    }

    #[test]
    fn source_oracle_target_seeds_cover_every_immolate_modifier() {
        let cases = [
            ("no filters", "", FilterConfig::default()),
            (
                "tag filter in first selector",
                "21111111",
                raw_cfg(
                    "",
                    "",
                    "tag_charm",
                    "",
                    "",
                    "any",
                    0.0,
                    false,
                    false,
                    "b_red",
                    false,
                    false,
                    0,
                    0.0,
                ),
            ),
            (
                "tag filter in second selector",
                "21111111",
                raw_cfg(
                    "",
                    "",
                    "",
                    "tag_charm",
                    "",
                    "any",
                    0.0,
                    false,
                    false,
                    "b_red",
                    false,
                    false,
                    0,
                    0.0,
                ),
            ),
            (
                "ante-1 voucher filter",
                "P1111111",
                raw_cfg(
                    "v_telescope",
                    "",
                    "",
                    "",
                    "",
                    "any",
                    0.0,
                    false,
                    false,
                    "b_red",
                    false,
                    false,
                    0,
                    0.0,
                ),
            ),
            (
                "forced first Buffoon pack filter",
                "",
                raw_cfg(
                    "",
                    "p_buffoon_normal_1",
                    "",
                    "",
                    "",
                    "any",
                    0.0,
                    false,
                    false,
                    "b_red",
                    false,
                    false,
                    0,
                    0.0,
                ),
            ),
            (
                "weighted second pack filter",
                "Z2111111",
                raw_cfg(
                    "",
                    "p_spectral_mega_1",
                    "",
                    "",
                    "",
                    "any",
                    0.0,
                    false,
                    false,
                    "b_red",
                    false,
                    false,
                    0,
                    0.0,
                ),
            ),
            (
                "shop Joker filter",
                "Q2111111",
                raw_cfg(
                    "",
                    "",
                    "",
                    "",
                    "Burnt Joker",
                    "shop",
                    0.0,
                    false,
                    false,
                    "b_red",
                    false,
                    false,
                    0,
                    0.0,
                ),
            ),
            (
                "Buffoon pack Joker filter",
                "M8511111",
                raw_cfg(
                    "",
                    "p_buffoon_mega_1",
                    "",
                    "",
                    "Reserved Parking",
                    "pack",
                    0.0,
                    false,
                    false,
                    "b_red",
                    false,
                    false,
                    0,
                    0.0,
                ),
            ),
            (
                "any-location Joker filter",
                "44311111",
                raw_cfg(
                    "",
                    "p_buffoon_mega_1",
                    "",
                    "",
                    "Blueprint",
                    "any",
                    0.0,
                    false,
                    false,
                    "b_red",
                    false,
                    false,
                    0,
                    0.0,
                ),
            ),
            (
                "Arcana Soul count filter",
                "EF311111",
                raw_cfg(
                    "",
                    "p_arcana_mega_1",
                    "",
                    "",
                    "",
                    "any",
                    1.0,
                    false,
                    false,
                    "b_red",
                    false,
                    false,
                    0,
                    0.0,
                ),
            ),
            (
                "Spectral Soul count filter",
                "LO511111",
                raw_cfg(
                    "",
                    "p_spectral_mega_1",
                    "",
                    "",
                    "",
                    "any",
                    1.0,
                    false,
                    false,
                    "b_red",
                    false,
                    false,
                    0,
                    0.0,
                ),
            ),
            (
                "Perkeo via The Soul filter",
                "MZ111111",
                raw_cfg(
                    "", "", "", "", "", "any", 0.0, false, true, "b_red", false, false, 0, 0.0,
                ),
            ),
            (
                "Observatory instant filter",
                "S111111",
                raw_cfg(
                    "", "", "", "", "", "any", 0.0, true, false, "b_red", false, false, 0, 0.0,
                ),
            ),
            (
                "Erratic Deck face-count filter",
                "11",
                raw_cfg(
                    "",
                    "",
                    "",
                    "",
                    "",
                    "any",
                    0.0,
                    false,
                    false,
                    "b_erratic",
                    true,
                    false,
                    12,
                    0.0,
                ),
            ),
            (
                "Erratic Deck suit-ratio filter",
                "U4111111",
                raw_cfg(
                    "",
                    "",
                    "",
                    "",
                    "",
                    "any",
                    0.0,
                    false,
                    false,
                    "b_erratic",
                    true,
                    false,
                    0,
                    0.75,
                ),
            ),
            (
                "tag plus Observatory composite",
                "U8411111",
                raw_cfg(
                    "",
                    "",
                    "tag_charm",
                    "",
                    "",
                    "any",
                    0.0,
                    true,
                    false,
                    "b_red",
                    false,
                    false,
                    0,
                    0.0,
                ),
            ),
            (
                "Joker plus Observatory composite",
                "L3411111",
                raw_cfg(
                    "",
                    "",
                    "",
                    "",
                    "Riff-raff",
                    "any",
                    0.0,
                    true,
                    false,
                    "b_red",
                    false,
                    false,
                    0,
                    0.0,
                ),
            ),
        ];

        for (name, seed, cfg) in cases {
            assert_seed_passes_like_source(name, seed, &cfg);
        }
    }

    #[test]
    fn optimized_core_matches_source_oracle_for_edge_windows() {
        let cases = [
            (
                "duplicate tag requires both blind tags",
                "",
                100_000,
                raw_cfg(
                    "",
                    "",
                    "tag_charm",
                    "tag_charm",
                    "",
                    "any",
                    0.0,
                    false,
                    false,
                    "b_red",
                    false,
                    false,
                    0,
                    0.0,
                ),
            ),
            (
                "Magic Deck unlocks Omen Globe voucher",
                "",
                10_000,
                raw_cfg(
                    "v_omen_globe",
                    "",
                    "",
                    "",
                    "",
                    "any",
                    0.0,
                    false,
                    false,
                    "b_magic",
                    false,
                    false,
                    0,
                    0.0,
                ),
            ),
            (
                "Nebula Deck unlocks Observatory voucher",
                "",
                10_000,
                raw_cfg(
                    "v_observatory",
                    "",
                    "",
                    "",
                    "",
                    "any",
                    0.0,
                    false,
                    false,
                    "b_nebula",
                    false,
                    false,
                    0,
                    0.0,
                ),
            ),
            (
                "Zodiac Deck unlocks Overstock Plus voucher",
                "",
                10_000,
                raw_cfg(
                    "v_overstock_plus",
                    "",
                    "",
                    "",
                    "",
                    "any",
                    0.0,
                    false,
                    false,
                    "b_zodiac",
                    false,
                    false,
                    0,
                    0.0,
                ),
            ),
            (
                "no-face deck modifier affects Erratic suit-ratio analysis",
                "",
                10_000,
                raw_cfg(
                    "",
                    "",
                    "",
                    "",
                    "",
                    "any",
                    0.0,
                    false,
                    false,
                    "b_erratic",
                    true,
                    true,
                    0,
                    0.75,
                ),
            ),
            (
                "tag plus Soul count plus selected pack composite",
                "",
                100_000,
                raw_cfg(
                    "",
                    "p_arcana_mega_1",
                    "tag_charm",
                    "",
                    "",
                    "any",
                    1.0,
                    false,
                    false,
                    "b_red",
                    false,
                    false,
                    0,
                    0.0,
                ),
            ),
        ];

        for (name, seed_start, budget, cfg) in cases {
            assert_core_matches_source_oracle(name, seed_start, &cfg, budget);
        }
    }

    #[test]
    fn first_shop_impossibilities_are_rejected_and_hidden_from_target_pools() {
        let impossible = [
            Item::Canio,
            Item::Triboulet,
            Item::Yorick,
            Item::Chicot,
            Item::Perkeo,
            Item::Cavendish,
            Item::Steel_Joker,
            Item::Stone_Joker,
            Item::Lucky_Cat,
            Item::Golden_Ticket,
            Item::Glass_Joker,
        ];
        for item in impossible {
            assert!(!is_first_shop_possible_joker(item), "{item:?}");
            assert_eq!(target_joker_pools(item), 0, "{item:?}");
            assert_eq!(
                brainstorm_search_core(
                    "",
                    &raw_cfg(
                        "",
                        "",
                        "",
                        "",
                        item_to_string(item),
                        "any",
                        0.0,
                        false,
                        false,
                        "b_red",
                        false,
                        false,
                        0,
                        0.0,
                    ),
                    10_000,
                    1,
                ),
                None,
                "{item:?} should not be searchable in first-shop Joker filters",
            );
        }
        assert_eq!(
            brainstorm_search_core(
                "",
                &raw_cfg(
                    "", "", "", "", "Caino", "any", 0.0, false, false, "b_red", false, false, 0,
                    0.0,
                ),
                10_000,
                1,
            ),
            None,
            "Balatro's displayed Caino spelling should also be rejected",
        );

        for item in [
            Item::Gros_Michel,
            Item::Reserved_Parking,
            Item::Seance,
            Item::Burnt_Joker,
        ] {
            assert!(is_first_shop_possible_joker(item), "{item:?}");
            assert_ne!(
                target_joker_pools(item) & STANDARD_JOKER_POOLS,
                0,
                "{item:?}",
            );
        }
    }

    #[test]
    fn lua_joker_selector_rules_match_native_first_shop_impossibilities() {
        const UI_LUA: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../UI.lua"));

        let impossible = [
            "Steel Joker",
            "Stone Joker",
            "Lucky Cat",
            "Golden Ticket",
            "Glass Joker",
            "Cavendish",
            "Caino",
            "Canio",
            "Triboulet",
            "Yorick",
            "Chicot",
            "Perkeo",
        ];
        for name in impossible {
            let quoted_entry = format!("[\"{name}\"] = true");
            let bare_entry = format!("{name} = true");
            assert!(
                UI_LUA.contains(&quoted_entry) || UI_LUA.contains(&bare_entry),
                "UI.lua should hide impossible first-shop Joker target {name}",
            );
        }

        for gate in [
            "center.rarity == 4",
            "center.enhancement_gate",
            "center.yes_pool_flag",
            "first_shop_impossible_joker_names[center.name]",
        ] {
            assert!(
                UI_LUA.contains(gate),
                "UI.lua Joker selector should keep source-derived gate `{gate}`",
            );
        }
    }

    #[test]
    fn souls_greater_than_one_requires_soul_count_even_when_perkeo_rolls() {
        let seed = "MZ111111";
        let one_soul_perkeo = FilterConfig::from_raw(
            "", "", "", "", "", "any", 1.0, false, true, "b_red", false, false, 0, 0.0,
        );
        let two_souls_perkeo = FilterConfig::from_raw(
            "", "", "", "", "", "any", 2.0, false, true, "b_red", false, false, 0, 0.0,
        );

        assert_eq!(
            brainstorm_search_core(seed, &one_soul_perkeo, 1, 1).as_deref(),
            Some(seed),
        );
        let mut one_soul_instance = Instance::new(Seed::from(seed));
        assert!(crate::filters::apply_filters(
            &mut one_soul_instance,
            &one_soul_perkeo
        ));

        assert_eq!(brainstorm_search_core(seed, &two_souls_perkeo, 1, 1), None);
        let mut two_soul_instance = Instance::new(Seed::from(seed));
        assert!(!crate::filters::apply_filters(
            &mut two_soul_instance,
            &two_souls_perkeo
        ));
    }

    #[test]
    fn current_core_preserves_earliest_seed_across_thread_counts() {
        let direct_cases = [
            (
                "tag+soul+pack after parallel prefix",
                "",
                FilterConfig::from_raw(
                    "",
                    "p_arcana_mega_1",
                    "tag_charm",
                    "",
                    "",
                    "any",
                    1.0,
                    false,
                    false,
                    "b_red",
                    false,
                    false,
                    0,
                    0.0,
                ),
                100_000,
            ),
            (
                "tag+pack+joker after parallel prefix",
                "",
                FilterConfig::from_raw(
                    "",
                    "p_buffoon_mega_1",
                    "tag_charm",
                    "",
                    "Reserved Parking",
                    "pack",
                    0.0,
                    false,
                    false,
                    "b_red",
                    false,
                    false,
                    0,
                    0.0,
                ),
                100_000,
            ),
            (
                "composite miss",
                "",
                filter_config_from_benchmark(&benchmark_case("ux-tag-pack-joker")),
                100_000,
            ),
        ];

        for (name, seed_start, cfg, budget) in direct_cases {
            let expected = brainstorm_search_core(seed_start, &cfg, budget, 1);
            for threads in [0, 2, 4, 16] {
                assert_eq!(
                    brainstorm_search_core(seed_start, &cfg, budget, threads),
                    expected,
                    "{name} returned a different seed with {threads} threads",
                );
            }
        }
    }

    #[test]
    fn parallel_scheduler_covers_prefix_block_and_partial_tail_boundaries() {
        let cfg = filter_config_from_benchmark(&benchmark_case("ux-soul-perkeo-spectral"));
        let compiled = CompiledFilter::compile(&cfg);
        let prefix = compiled.serial_prefix_size();
        let block = compiled.chunk_size();
        let target = "1WR71111";
        let target_id = Seed::from(target).id();

        for offset in [
            prefix - 1,
            prefix,
            prefix + block - 1,
            prefix + block,
            prefix + block + 37,
        ] {
            let seed_start = Seed::from_id(target_id - offset).to_string();
            let budget = offset + 1;
            for threads in [1, 2, 16, i32::MAX] {
                assert_eq!(
                    brainstorm_search_core(&seed_start, &cfg, offset, threads),
                    None,
                    "scheduler examined the target beyond budget with {threads} threads",
                );
            }
            assert_eq!(
                brainstorm_search_core(&seed_start, &cfg, budget, 1).as_deref(),
                Some(target),
                "test fixture has an earlier match at offset {offset}",
            );
            for threads in [2, 16, i32::MAX] {
                assert_eq!(
                    brainstorm_search_core(&seed_start, &cfg, budget, threads).as_deref(),
                    Some(target),
                    "scheduler lost the match at offset {offset} with {threads} threads",
                );
            }
        }
    }

    #[test]
    fn scheduler_thread_inputs_preserve_the_earliest_result() {
        let cfg = filter_config_from_benchmark(&benchmark_case("ux-soul-perkeo-spectral"));
        let compiled = CompiledFilter::compile(&cfg);
        let offset = compiled.serial_prefix_size() + compiled.parallel_threshold() - 1;
        let target = "1WR71111";
        let seed_start = Seed::from_id(Seed::from(target).id() - offset).to_string();

        for threads in [i32::MIN, -1, 0, 1, 2, 16, i32::MAX] {
            assert_eq!(
                brainstorm_search_core(&seed_start, &cfg, offset + 1, threads).as_deref(),
                Some(target),
                "scheduler changed the earliest result for thread input {threads}",
            );
        }
    }

    #[test]
    fn any_joker_scheduler_preserves_source_earliest_result() {
        use crate::engine::config::KernelShape;

        let cfg =
            filter_config_from_benchmark(&benchmark_case("ux-mega-spectral-any-baseball-card"));
        let compiled = CompiledFilter::compile(&cfg);
        assert_eq!(compiled.shape, KernelShape::AnyJoker);
        assert_eq!(compiled.chunk_size(), 1_024);

        let budget = 147_920;
        assert_source_earliest_across_threads(
            "ux-mega-spectral-any-baseball-card",
            &cfg,
            budget,
            "CBD41111",
        );
    }

    #[test]
    fn perkeo_scheduler_preserves_source_earliest_result() {
        use crate::engine::config::KernelShape;

        let cfg = filter_config_from_benchmark(&benchmark_case("ux-perkeo-arcana-normal"));
        let compiled = CompiledFilter::compile(&cfg);
        assert_eq!(compiled.shape, KernelShape::Perkeo);
        assert_eq!(compiled.chunk_size(), 1_024);

        let budget = 6_221;
        let expected = assert_source_earliest_across_threads(
            "ux-perkeo-arcana-normal",
            &cfg,
            budget,
            "HX511111",
        );
        assert_eq!(
            brainstorm_search_core("", &cfg, 100_000, 0),
            expected,
            "Perkeo auto search changed its earliest result above parallel onset",
        );
    }

    #[test]
    fn normal_arcana_soul_joker_preserves_source_earliest_result() {
        use crate::engine::config::KernelShape;

        let cfg = filter_config_from_benchmark(&benchmark_case("ux-normal-arcana-soul-half-joker"));
        assert_eq!(CompiledFilter::compile(&cfg).shape, KernelShape::Composite);

        let budget = 41_112;
        assert_source_earliest_across_threads(
            "ux-normal-arcana-soul-half-joker",
            &cfg,
            budget,
            "WLX11111",
        );
    }

    #[test]
    fn jumbo_arcana_soul_joker_preserves_source_earliest_result() {
        use crate::engine::config::KernelShape;

        let cfg = filter_config_from_benchmark(&benchmark_case("ux-jumbo-arcana-soul-joker"));
        assert_eq!(CompiledFilter::compile(&cfg).shape, KernelShape::Composite);

        let budget = 52_862;
        assert_source_earliest_across_threads(
            "ux-jumbo-arcana-soul-joker",
            &cfg,
            budget,
            "X721111",
        );
    }

    #[test]
    fn voucher_with_unselected_soul_pack_preserves_source_earliest_results() {
        use crate::engine::config::KernelShape;

        for (case_name, budget, target) in [
            ("ux-voucher-soul-no-pack", 7_619, "92711111"),
            ("ux-voucher-perkeo-no-pack", 40_678, "U9X11111"),
        ] {
            let cfg = filter_config_from_benchmark(&benchmark_case(case_name));
            assert_eq!(CompiledFilter::compile(&cfg).shape, KernelShape::Composite);
            assert_source_earliest_across_threads(case_name, &cfg, budget, target);
        }
    }

    #[test]
    fn dual_tag_voucher_joker_preserves_source_earliest_result() {
        use crate::engine::config::KernelShape;

        let cfg = filter_config_from_benchmark(&benchmark_case("ux-dual-tag-voucher-blueprint"));
        assert_eq!(CompiledFilter::compile(&cfg).shape, KernelShape::Composite);

        let budget = 548_953;
        assert_source_earliest_across_threads(
            "ux-dual-tag-voucher-blueprint",
            &cfg,
            budget,
            "2CGD1111",
        );
    }

    #[test]
    fn erratic_scheduler_family_uses_the_short_serial_prefix() {
        for case in ["ux-erratic-suit-85", "ux-erratic-tag-suit"] {
            let cfg = filter_config_from_benchmark(&benchmark_case(case));
            assert_eq!(CompiledFilter::compile(&cfg).serial_prefix_size(), 256);
        }

        let ordinary_composite = filter_config_from_benchmark(&benchmark_case("ux-tag-pack-joker"));
        assert_eq!(
            CompiledFilter::compile(&ordinary_composite).serial_prefix_size(),
            4_096,
        );
    }

    #[test]
    fn current_core_matches_source_oracle_for_every_ux_benchmark_case() {
        for case in crate::bench_cases::bench_cases()
            .into_iter()
            .filter(|case| case.group == crate::bench_cases::BenchGroup::Ux)
        {
            let cfg = filter_config_from_benchmark(&case);
            assert_core_matches_source_oracle(case.name, case.seed_start, &cfg, 100_000);
        }
    }

    #[test]
    fn benchmark_static_shapes_match_filter_compiler() {
        use crate::bench_cases::BenchShape;
        use crate::engine::config::KernelShape;

        for case in crate::bench_cases::bench_cases() {
            let compiled = CompiledFilter::compile(&filter_config_from_benchmark(&case));
            assert_eq!(
                case.shape == BenchShape::Static,
                compiled.shape == KernelShape::NoMatch,
                "benchmark case {} has a stale shape label",
                case.name,
            );
        }
    }

    #[test]
    fn optimized_predicates_match_source_oracle_across_ux_windows() {
        for case in crate::bench_cases::bench_cases()
            .into_iter()
            .filter(|case| case.group == crate::bench_cases::BenchGroup::Ux)
        {
            let cfg = filter_config_from_benchmark(&case);
            for seed_start in ["", "KRVH1234", "ZZZYZZZZ"] {
                assert_predicate_matches_source_oracle_window(case.name, seed_start, &cfg, 256);
            }
        }
    }

    #[test]
    fn source_locked_tags_and_vouchers_are_static_no_match() {
        use crate::engine::config::KernelShape;

        for &tag in TAG_POOL {
            let cfg = FilterConfig {
                tag1: tag,
                ..FilterConfig::default()
            };
            if is_ante1_locked_tag(tag) {
                assert_eq!(
                    CompiledFilter::compile(&cfg).shape,
                    KernelShape::NoMatch,
                    "ante-1 locked tag {} should not scan seeds",
                    item_to_string(tag),
                );
            } else {
                assert_ne!(
                    CompiledFilter::compile(&cfg).shape,
                    KernelShape::NoMatch,
                    "unlocked ante-1 tag {} should remain searchable",
                    item_to_string(tag),
                );
            }
        }

        for voucher_pair in VOUCHER_POOL.chunks_exact(2) {
            let base = voucher_pair[0];
            let upgrade = voucher_pair[1];
            assert_ne!(
                CompiledFilter::compile(&FilterConfig {
                    voucher: base,
                    ..FilterConfig::default()
                })
                .shape,
                KernelShape::NoMatch,
                "base voucher {} should be searchable on Red Deck",
                item_to_string(base),
            );
            assert_eq!(
                CompiledFilter::compile(&FilterConfig {
                    voucher: upgrade,
                    ..FilterConfig::default()
                })
                .shape,
                KernelShape::NoMatch,
                "upgrade voucher {} should be locked without its prerequisite",
                item_to_string(upgrade),
            );
        }

        for (deck, active, upgrade) in [
            (Item::Magic_Deck, Item::Crystal_Ball, Item::Omen_Globe),
            (Item::Nebula_Deck, Item::Telescope, Item::Observatory),
            (Item::Zodiac_Deck, Item::Overstock, Item::Overstock_Plus),
            (Item::Zodiac_Deck, Item::Tarot_Merchant, Item::Tarot_Tycoon),
            (
                Item::Zodiac_Deck,
                Item::Planet_Merchant,
                Item::Planet_Tycoon,
            ),
        ] {
            assert_eq!(
                CompiledFilter::compile(&FilterConfig {
                    voucher: active,
                    deck,
                    ..FilterConfig::default()
                })
                .shape,
                KernelShape::NoMatch,
                "{} starts with {}, so it cannot roll as the ante-1 voucher",
                item_to_string(deck),
                item_to_string(active),
            );
            assert_ne!(
                CompiledFilter::compile(&FilterConfig {
                    voucher: upgrade,
                    deck,
                    ..FilterConfig::default()
                })
                .shape,
                KernelShape::NoMatch,
                "{} should unlock {} from its starting voucher",
                item_to_string(deck),
                item_to_string(upgrade),
            );
        }
    }

    #[test]
    fn current_core_rejects_static_no_match_filters() {
        use crate::engine::config::{CompiledFilter, KernelShape};

        let cases = [
            (
                "ante-1 locked tag",
                FilterConfig::from_raw(
                    "",
                    "",
                    "tag_buffoon",
                    "",
                    "",
                    "any",
                    0.0,
                    false,
                    false,
                    "b_red",
                    false,
                    false,
                    0,
                    0.0,
                ),
            ),
            (
                "voucher upgrade without prerequisite voucher",
                FilterConfig::from_raw(
                    "v_observatory",
                    "",
                    "",
                    "",
                    "",
                    "any",
                    0.0,
                    false,
                    false,
                    "b_red",
                    false,
                    false,
                    0,
                    0.0,
                ),
            ),
            (
                "observatory with a different voucher target",
                FilterConfig::from_raw(
                    "v_tarot_merchant",
                    "",
                    "",
                    "",
                    "",
                    "any",
                    0.0,
                    true,
                    false,
                    "b_red",
                    false,
                    false,
                    0,
                    0.0,
                ),
            ),
            (
                "observatory with Telescope already active",
                FilterConfig::from_raw(
                    "", "", "", "", "", "any", 0.0, true, false, "b_nebula", false, false, 0, 0.0,
                ),
            ),
            (
                "observatory with incompatible pack target",
                FilterConfig::from_raw(
                    "",
                    "p_spectral_mega_1",
                    "",
                    "",
                    "",
                    "any",
                    0.0,
                    true,
                    false,
                    "b_red",
                    false,
                    false,
                    0,
                    0.0,
                ),
            ),
            (
                "observatory with Soul count",
                FilterConfig::from_raw(
                    "", "", "", "", "", "any", 1.0, true, false, "b_red", false, false, 0, 0.0,
                ),
            ),
            (
                "observatory with Perkeo",
                FilterConfig::from_raw(
                    "", "", "", "", "", "any", 0.0, true, true, "b_red", false, false, 0, 0.0,
                ),
            ),
            (
                "legendary shop joker",
                FilterConfig::from_raw(
                    "", "", "", "", "Perkeo", "shop", 0.0, false, false, "b_red", false, false, 0,
                    0.0,
                ),
            ),
            (
                "legendary pack joker",
                FilterConfig::from_raw(
                    "", "", "", "", "Perkeo", "pack", 0.0, false, false, "b_red", false, false, 0,
                    0.0,
                ),
            ),
            (
                "legendary any joker",
                FilterConfig::from_raw(
                    "", "", "", "", "Perkeo", "any", 0.0, false, false, "b_red", false, false, 0,
                    0.0,
                ),
            ),
            (
                "pack-only joker in non-Buffoon pack",
                FilterConfig::from_raw(
                    "",
                    "p_spectral_mega_1",
                    "",
                    "",
                    "Blueprint",
                    "pack",
                    0.0,
                    false,
                    false,
                    "b_red",
                    false,
                    false,
                    0,
                    0.0,
                ),
            ),
            (
                "too many souls in selected pack",
                FilterConfig::from_raw(
                    "",
                    "p_spectral_normal_1",
                    "",
                    "",
                    "",
                    "any",
                    3.0,
                    false,
                    false,
                    "b_red",
                    false,
                    false,
                    0,
                    0.0,
                ),
            ),
            (
                "two souls in first shop",
                FilterConfig::from_raw(
                    "", "", "", "", "", "any", 2.0, false, false, "b_red", false, false, 0, 0.0,
                ),
            ),
            (
                "too many souls without selected pack",
                FilterConfig::from_raw(
                    "", "", "", "", "", "any", 6.0, false, false, "b_red", false, false, 0, 0.0,
                ),
            ),
            (
                "souls in non-soulable pack",
                FilterConfig::from_raw(
                    "",
                    "p_buffoon_mega_1",
                    "",
                    "",
                    "",
                    "any",
                    1.0,
                    false,
                    false,
                    "b_red",
                    false,
                    false,
                    0,
                    0.0,
                ),
            ),
            (
                "perkeo in non-soulable pack",
                FilterConfig::from_raw(
                    "",
                    "p_buffoon_mega_1",
                    "",
                    "",
                    "",
                    "any",
                    0.0,
                    false,
                    true,
                    "b_red",
                    false,
                    false,
                    0,
                    0.0,
                ),
            ),
            (
                "no faces with required face count",
                FilterConfig::from_raw(
                    "",
                    "",
                    "",
                    "",
                    "",
                    "any",
                    0.0,
                    false,
                    false,
                    "b_erratic",
                    true,
                    true,
                    1,
                    0.0,
                ),
            ),
            (
                "erratic impossible face count",
                FilterConfig::from_raw(
                    "",
                    "",
                    "",
                    "",
                    "",
                    "any",
                    0.0,
                    false,
                    false,
                    "b_erratic",
                    true,
                    false,
                    53,
                    0.0,
                ),
            ),
            ("erratic impossible suit ratio", {
                let mut cfg = FilterConfig::from_raw(
                    "",
                    "",
                    "",
                    "",
                    "",
                    "any",
                    0.0,
                    false,
                    false,
                    "b_erratic",
                    true,
                    false,
                    0,
                    1.0,
                );
                cfg.suit_ratio = 1.01;
                cfg
            }),
        ];

        for (name, cfg) in cases {
            assert_eq!(
                CompiledFilter::compile(&cfg).shape,
                KernelShape::NoMatch,
                "{name} should be rejected before seed scanning",
            );
            assert_eq!(
                brainstorm_search_core("", &cfg, 10_000, 1),
                None,
                "current Rust should reject {name}",
            );
        }
    }

    #[test]
    fn search_wraps_without_panicking_near_seed_space_end() {
        let cfg = FilterConfig::default();
        assert_eq!(
            brainstorm_search_core("ZZZZZZZZ", &cfg, 2, 1).as_deref(),
            Some("ZZZZZZZZ"),
        );
        assert_eq!(
            resolve_seed_budget(crate::seed::SEED_SPACE + 1),
            crate::seed::SEED_SPACE
        );
    }

    #[test]
    fn filtered_parallel_search_preserves_earliest_seed_across_wraparound() {
        let cfg = filter_config_from_benchmark(&benchmark_case("ux-soul-perkeo-spectral"));
        let expected = Some("1WR71111".to_owned());
        for threads in [1, 2, 4, 8, 16, 0] {
            assert_eq!(
                brainstorm_search_core("ZZZZZZZZ", &cfg, 310_000, threads),
                expected,
                "wraparound search changed its earliest result with {threads} threads",
            );
        }
    }

    #[test]
    fn current_joker_pools_match_current_version_boundaries() {
        assert_eq!(COMMON_JOKERS.len(), 61);
        assert_eq!(COMMON_JOKERS_100.len(), 60);
        assert_eq!(COMMON_JOKERS[47], Item::Reserved_Parking);
        assert_eq!(COMMON_JOKERS[48], Item::Mail_In_Rebate);

        assert_eq!(UNCOMMON_JOKERS.len(), 64);
        assert_eq!(UNCOMMON_JOKERS_100.len(), 66);
        assert_eq!(UNCOMMON_JOKERS[14], Item::Sixth_Sense);
        assert_eq!(UNCOMMON_JOKERS[19], Item::Seance);
        assert!(!UNCOMMON_JOKERS.contains(&Item::Vagabond));
        assert!(!UNCOMMON_JOKERS.contains(&Item::Reserved_Parking));
        assert!(!UNCOMMON_JOKERS.contains(&Item::Stuntman));
        assert!(!UNCOMMON_JOKERS.contains(&Item::Burnt_Joker));

        assert_eq!(RARE_JOKERS.len(), 20);
        assert_eq!(RARE_JOKERS_100.len(), 19);
        assert_eq!(RARE_JOKERS[1], Item::Vagabond);
        assert_eq!(RARE_JOKERS[15], Item::Stuntman);
        assert_eq!(RARE_JOKERS[19], Item::Burnt_Joker);
        assert!(!RARE_JOKERS.contains(&Item::Sixth_Sense));
        assert!(!RARE_JOKERS.contains(&Item::Seance));
    }

    #[test]
    fn joker_rarity_pools_are_disjoint() {
        let pools: &[(&str, &[Item])] = &[
            ("common", &COMMON_JOKERS),
            ("uncommon", &UNCOMMON_JOKERS),
            ("rare", &RARE_JOKERS),
            ("legendary", &LEGENDARY_JOKERS),
        ];
        for i in 0..pools.len() {
            for j in (i + 1)..pools.len() {
                let (left_name, left) = pools[i];
                let (right_name, right) = pools[j];
                for item in left {
                    assert!(
                        !right.contains(item),
                        "{item:?} appears in both {left_name} and {right_name} joker pools",
                    );
                }
            }
        }
    }

    #[test]
    fn ffi_contract_matches_empty_and_allocated_results() {
        let empty = c"";
        let one = c"1";

        // SAFETY: every pointer references a static, immutable C string.
        let empty_result = unsafe {
            brainstorm_search(
                empty.as_ptr(),
                empty.as_ptr(),
                empty.as_ptr(),
                empty.as_ptr(),
                empty.as_ptr(),
                empty.as_ptr(),
                empty.as_ptr(),
                0.0,
                false,
                false,
                empty.as_ptr(),
                false,
                false,
                0,
                0.0,
                1,
                1,
            )
        };
        assert!(empty_result.is_null());

        // SAFETY: every pointer references a static, immutable C string.
        let one_result = unsafe {
            brainstorm_search(
                one.as_ptr(),
                empty.as_ptr(),
                empty.as_ptr(),
                empty.as_ptr(),
                empty.as_ptr(),
                empty.as_ptr(),
                empty.as_ptr(),
                0.0,
                false,
                false,
                empty.as_ptr(),
                false,
                false,
                0,
                0.0,
                1,
                1,
            )
        };
        assert!(!one_result.is_null());
        // SAFETY: `brainstorm_search` returned this pointer and it is non-null.
        let result = unsafe { CStr::from_ptr(one_result) }
            .to_string_lossy()
            .into_owned();
        assert_eq!(result, "1");
        // SAFETY: the first pointer was returned above and has not been freed;
        // null is explicitly supported by `free_result`.
        unsafe {
            free_result(one_result);
            free_result(std::ptr::null_mut());
        }
    }

    #[allow(clippy::fn_params_excessive_bools, clippy::too_many_arguments)]
    fn raw_cfg(
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
    ) -> FilterConfig {
        FilterConfig::from_raw(
            voucher,
            pack,
            tag1,
            tag2,
            joker_name,
            joker_location,
            souls,
            observatory,
            perkeo,
            deck,
            erratic,
            no_faces,
            min_face_cards,
            suit_ratio,
        )
    }

    fn assert_seed_passes_like_source(name: &str, seed: &str, cfg: &FilterConfig) {
        assert!(
            source_oracle_passes_seed(seed, cfg),
            "{name}: source model rejected target seed {seed}",
        );

        let compiled = CompiledFilter::compile(cfg);
        let mut state = SearchState::from_id(Seed::from(seed).id());
        assert!(
            apply_compiled_filter(&mut state, &compiled),
            "{name}: optimized predicate rejected target seed {seed}",
        );

        for threads in [1, 2, 0] {
            assert_eq!(
                brainstorm_search_core(seed, cfg, 1, threads).as_deref(),
                Some(seed),
                "{name}: search rejected target seed {seed} with {threads} threads",
            );
        }
    }

    fn assert_predicate_matches_source_oracle_window(
        name: &str,
        seed_start: &str,
        cfg: &FilterConfig,
        count: usize,
    ) {
        let compiled = CompiledFilter::compile(cfg);
        let mut state = SearchState::from_id(Seed::from(seed_start).id());
        for offset in 0..count {
            let mut instance = Instance::new(state.seed.clone());
            let expected = crate::filters::apply_filters(&mut instance, cfg);
            let actual = apply_compiled_filter(&mut state, &compiled);
            assert_eq!(
                actual, expected,
                "{name} diverged from the source model at {seed_start} + {offset}",
            );
            state.next();
        }
    }

    fn assert_core_matches_source_oracle(
        name: &str,
        seed_start: &str,
        cfg: &FilterConfig,
        budget: i64,
    ) {
        let expected = source_oracle_search(seed_start, cfg, budget);
        for threads in [1, 2, 0] {
            assert_eq!(
                brainstorm_search_core(seed_start, cfg, budget, threads),
                expected,
                "{name}: optimized search disagreed with source model using {threads} threads",
            );
        }
    }

    fn source_oracle_search(seed_start: &str, cfg: &FilterConfig, budget: i64) -> Option<String> {
        let mut seed = Seed::from(seed_start);
        for _ in 0..resolve_seed_budget(budget) {
            let mut inst = Instance::new(seed.clone());
            if crate::filters::apply_filters(&mut inst, cfg) {
                return Some(seed.to_string());
            }
            seed.next();
        }
        None
    }

    fn assert_source_earliest_across_threads(
        case_name: &str,
        cfg: &FilterConfig,
        budget: i64,
        target: &str,
    ) -> Option<String> {
        let expected = source_oracle_search("", cfg, budget);
        assert_eq!(expected.as_deref(), Some(target));
        for threads in [0, 1, 2, 4, 8, 16, i32::MAX] {
            assert_eq!(
                brainstorm_search_core("", cfg, budget - 1, threads),
                None,
                "{case_name} crossed its budget with {threads} threads",
            );
            assert_eq!(
                brainstorm_search_core("", cfg, budget, threads),
                expected,
                "{case_name} changed its earliest result with {threads} threads",
            );
        }
        expected
    }

    fn source_oracle_passes_seed(seed: &str, cfg: &FilterConfig) -> bool {
        let mut inst = Instance::new(Seed::from(seed));
        crate::filters::apply_filters(&mut inst, cfg)
    }

    fn benchmark_case(name: &str) -> crate::bench_cases::BenchCase {
        crate::bench_cases::bench_cases()
            .into_iter()
            .find(|case| case.name == name)
            .expect("missing benchmark case")
    }

    fn filter_config_from_benchmark(case: &crate::bench_cases::BenchCase) -> FilterConfig {
        FilterConfig::from_raw(
            case.voucher,
            case.pack,
            case.tag1,
            case.tag2,
            case.joker,
            case.joker_location,
            case.souls,
            case.observatory,
            case.perkeo,
            case.deck,
            case.erratic,
            case.no_faces,
            case.min_face_cards,
            case.suit_ratio,
        )
    }

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() <= 1e-15,
            "actual={actual:.17} expected={expected:.17}",
        );
    }
}
