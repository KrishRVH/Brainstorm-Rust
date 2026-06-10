use immolate::brainstorm_search_core;
use immolate::filters::FilterConfig;
use immolate::rng::{LuaRandom, pseudohash};
use immolate::seed::Seed;

struct SearchCase {
    name: &'static str,
    seed_start: &'static str,
    voucher: &'static str,
    pack: &'static str,
    tag1: &'static str,
    tag2: &'static str,
    joker: &'static str,
    joker_location: &'static str,
    souls: f64,
    observatory: bool,
    perkeo: bool,
    deck: &'static str,
    erratic: bool,
    no_faces: bool,
    min_face_cards: i32,
    suit_ratio: f64,
    num_seeds: i64,
    threads: i32,
}

impl SearchCase {
    fn config(&self) -> FilterConfig {
        FilterConfig::from_raw(
            self.voucher,
            self.pack,
            self.tag1,
            self.tag2,
            self.joker,
            self.joker_location,
            self.souls,
            self.observatory,
            self.perkeo,
            self.deck,
            self.erratic,
            self.no_faces,
            self.min_face_cards,
            self.suit_ratio,
        )
    }
}

fn main() {
    for input in [
        "",
        "1",
        "11",
        "ABCDE",
        "Tag1",
        "shop_pack1",
        "soul_Spectral1",
    ] {
        println!("pseudohash({input})={:.17}", pseudohash(input));
    }

    let mut rng = LuaRandom::new(0.5);
    println!("lua_random_0.5_1={:.17}", rng.random());
    println!("lua_random_0.5_2={:.17}", rng.random());

    for id in [0, 1, 2, 35, 36, 37, 1_000, 1_000_000] {
        let seed = Seed::from_id(id);
        println!(
            "seed_id_{id}={},{}",
            seed,
            Seed::from_str(&seed.to_string()).id()
        );
    }

    for case in oracle_cases() {
        let cfg = case.config();
        let result = brainstorm_search_core(case.seed_start, &cfg, case.num_seeds, case.threads);
        let ffi_result = match result.as_deref() {
            Some("") | None => "<null>",
            Some(seed) => seed,
        };
        println!("search_{}={ffi_result}", case.name);
    }
}

fn oracle_cases() -> [SearchCase; 7] {
    [
        SearchCase {
            name: "empty_no_filter_1",
            seed_start: "",
            voucher: "",
            pack: "",
            tag1: "",
            tag2: "",
            joker: "",
            joker_location: "any",
            souls: 0.0,
            observatory: false,
            perkeo: false,
            deck: "b_red",
            erratic: false,
            no_faces: false,
            min_face_cards: 0,
            suit_ratio: 0.0,
            num_seeds: 1,
            threads: 1,
        },
        SearchCase {
            name: "1_no_filter_1",
            seed_start: "1",
            voucher: "",
            pack: "",
            tag1: "",
            tag2: "",
            joker: "",
            joker_location: "any",
            souls: 0.0,
            observatory: false,
            perkeo: false,
            deck: "b_red",
            erratic: false,
            no_faces: false,
            min_face_cards: 0,
            suit_ratio: 0.0,
            num_seeds: 1,
            threads: 1,
        },
        SearchCase {
            name: "tag_charm_10000",
            seed_start: "",
            voucher: "",
            pack: "",
            tag1: "tag_charm",
            tag2: "",
            joker: "",
            joker_location: "any",
            souls: 0.0,
            observatory: false,
            perkeo: false,
            deck: "b_red",
            erratic: false,
            no_faces: false,
            min_face_cards: 0,
            suit_ratio: 0.0,
            num_seeds: 10_000,
            threads: 1,
        },
        SearchCase {
            name: "v_telescope_10000",
            seed_start: "",
            voucher: "v_telescope",
            pack: "",
            tag1: "",
            tag2: "",
            joker: "",
            joker_location: "any",
            souls: 0.0,
            observatory: false,
            perkeo: false,
            deck: "b_red",
            erratic: false,
            no_faces: false,
            min_face_cards: 0,
            suit_ratio: 0.0,
            num_seeds: 10_000,
            threads: 1,
        },
        SearchCase {
            name: "pack_spectral_10000",
            seed_start: "",
            voucher: "",
            pack: "p_spectral_mega_1",
            tag1: "",
            tag2: "",
            joker: "",
            joker_location: "any",
            souls: 0.0,
            observatory: false,
            perkeo: false,
            deck: "b_red",
            erratic: false,
            no_faces: false,
            min_face_cards: 0,
            suit_ratio: 0.0,
            num_seeds: 10_000,
            threads: 1,
        },
        SearchCase {
            name: "observatory_100000",
            seed_start: "",
            voucher: "",
            pack: "",
            tag1: "",
            tag2: "",
            joker: "",
            joker_location: "any",
            souls: 0.0,
            observatory: true,
            perkeo: false,
            deck: "b_red",
            erratic: false,
            no_faces: false,
            min_face_cards: 0,
            suit_ratio: 0.0,
            num_seeds: 100_000,
            threads: 1,
        },
        SearchCase {
            name: "erratic_faces_10000",
            seed_start: "",
            voucher: "",
            pack: "",
            tag1: "",
            tag2: "",
            joker: "",
            joker_location: "any",
            souls: 0.0,
            observatory: false,
            perkeo: false,
            deck: "b_erratic",
            erratic: true,
            no_faces: false,
            min_face_cards: 12,
            suit_ratio: 0.0,
            num_seeds: 10_000,
            threads: 1,
        },
    ]
}
