#include "functions.hpp"
#include "immolate.hpp"
#include "instance.hpp"
#include "items.hpp"
#include "rng.hpp"
#include "search.hpp"
#include <algorithm>
#include <array>
#include <atomic>
#include <cctype>
#include <cstdlib>
#include <cstring>
#include <ctime>
#include <fstream>
#include <iomanip>
#include <mutex>
#include <sstream>
#include <string>
#include <thread>

namespace {

std::mutex g_log_mutex;
std::string g_log_path;
std::atomic<long long> g_log_seq{0};

constexpr int kCardsPerSuit = 13;
constexpr int kSuitCount = 4;
constexpr long long kDefaultSeedBudget = 100000000;

const std::array<Item, kCardsPerSuit> kRankOrder = {
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
};

const std::array<Item, kSuitCount> kSuitOrder = {
    Item::Clubs,
    Item::Diamonds,
    Item::Hearts,
    Item::Spades,
};

enum class JokerLocation {
    Any,
    Shop,
    Pack,
};

struct FilterConfig {
    Item voucher = Item::RETRY;
    Item pack = Item::RETRY;
    Item tag1 = Item::RETRY;
    Item tag2 = Item::RETRY;
    Item joker = Item::RETRY;
    JokerLocation joker_location = JokerLocation::Any;
    long souls = 0;
    bool observatory = false;
    bool perkeo = false;
    Item deck = Item::Red_Deck;
    bool erratic = false;
    bool no_faces = false;
    int min_face_cards = 0;
    double suit_ratio = 0.0;
};

struct DeckStats {
    int total = 0;
    int face_count = 0;
    std::array<int, kSuitCount> suit_count = {0, 0, 0, 0};
};

std::string format_bool(bool value) {
    return value ? "true" : "false";
}

std::string safe_cstr(const char* value) {
    return value ? std::string(value) : std::string("<null>");
}

std::string log_prefix() {
    const long long seq = g_log_seq.fetch_add(1);
    std::time_t now = std::time(nullptr);
    std::tm tm = *std::localtime(&now);
    std::ostringstream oss;
    oss << "[CPP] " << std::put_time(&tm, "%Y-%m-%d %H:%M:%S") << " #" << seq << " ";
    return oss.str();
}

void cpp_log(const std::string& message) {
    // Logging disabled.
    // if (g_log_path.empty()) {
    //     return;
    // }
    // std::lock_guard<std::mutex> lock(g_log_mutex);
    // std::ofstream file(g_log_path, std::ios::app);
    // if (!file) {
    //     return;
    // }
    // file << log_prefix() << message << "\n";
}

bool is_face_rank(Item rank) {
    return rank == Item::Jack || rank == Item::Queen || rank == Item::King;
}

std::string normalize_pack_key(const std::string& key) {
    if (key.empty()) {
        return key;
    }
    const std::size_t pos = key.find_last_of('_');
    if (pos == std::string::npos || pos + 1 >= key.size()) {
        return key;
    }
    for (std::size_t i = pos + 1; i < key.size(); ++i) {
        if (!std::isdigit(static_cast<unsigned char>(key[i]))) {
            return key;
        }
    }
    return key.substr(0, pos);
}

Item parse_tag_key(const std::string& key) {
    if (key.empty()) {
        return Item::RETRY;
    }
    if (key == "tag_uncommon") {
        return Item::Uncommon_Tag;
    }
    if (key == "tag_rare") {
        return Item::Rare_Tag;
    }
    if (key == "tag_negative") {
        return Item::Negative_Tag;
    }
    if (key == "tag_foil") {
        return Item::Foil_Tag;
    }
    if (key == "tag_holo") {
        return Item::Holographic_Tag;
    }
    if (key == "tag_polychrome") {
        return Item::Polychrome_Tag;
    }
    if (key == "tag_investment") {
        return Item::Investment_Tag;
    }
    if (key == "tag_voucher") {
        return Item::Voucher_Tag;
    }
    if (key == "tag_boss") {
        return Item::Boss_Tag;
    }
    if (key == "tag_standard") {
        return Item::Standard_Tag;
    }
    if (key == "tag_charm") {
        return Item::Charm_Tag;
    }
    if (key == "tag_meteor") {
        return Item::Meteor_Tag;
    }
    if (key == "tag_buffoon") {
        return Item::Buffoon_Tag;
    }
    if (key == "tag_handy") {
        return Item::Handy_Tag;
    }
    if (key == "tag_garbage") {
        return Item::Garbage_Tag;
    }
    if (key == "tag_ethereal") {
        return Item::Ethereal_Tag;
    }
    if (key == "tag_coupon") {
        return Item::Coupon_Tag;
    }
    if (key == "tag_double") {
        return Item::Double_Tag;
    }
    if (key == "tag_juggle") {
        return Item::Juggle_Tag;
    }
    if (key == "tag_d_six") {
        return Item::D6_Tag;
    }
    if (key == "tag_top_up") {
        return Item::Top_up_Tag;
    }
    if (key == "tag_skip") {
        return Item::Speed_Tag;
    }
    if (key == "tag_orbital") {
        return Item::Orbital_Tag;
    }
    if (key == "tag_economy") {
        return Item::Economy_Tag;
    }
    return Item::RETRY;
}

Item parse_pack_key(const std::string& key) {
    if (key.empty()) {
        return Item::RETRY;
    }
    const std::string normalized = normalize_pack_key(key);
    if (normalized == "p_arcana_normal") {
        return Item::Arcana_Pack;
    }
    if (normalized == "p_arcana_jumbo") {
        return Item::Jumbo_Arcana_Pack;
    }
    if (normalized == "p_arcana_mega") {
        return Item::Mega_Arcana_Pack;
    }
    if (normalized == "p_celestial_normal") {
        return Item::Celestial_Pack;
    }
    if (normalized == "p_celestial_jumbo") {
        return Item::Jumbo_Celestial_Pack;
    }
    if (normalized == "p_celestial_mega") {
        return Item::Mega_Celestial_Pack;
    }
    if (normalized == "p_standard_normal") {
        return Item::Standard_Pack;
    }
    if (normalized == "p_standard_jumbo") {
        return Item::Jumbo_Standard_Pack;
    }
    if (normalized == "p_standard_mega") {
        return Item::Mega_Standard_Pack;
    }
    if (normalized == "p_buffoon_normal") {
        return Item::Buffoon_Pack;
    }
    if (normalized == "p_buffoon_jumbo") {
        return Item::Jumbo_Buffoon_Pack;
    }
    if (normalized == "p_buffoon_mega") {
        return Item::Mega_Buffoon_Pack;
    }
    if (normalized == "p_spectral_normal") {
        return Item::Spectral_Pack;
    }
    if (normalized == "p_spectral_jumbo") {
        return Item::Jumbo_Spectral_Pack;
    }
    if (normalized == "p_spectral_mega") {
        return Item::Mega_Spectral_Pack;
    }
    return Item::RETRY;
}

Item parse_voucher_key(const std::string& key) {
    if (key.empty()) {
        return Item::RETRY;
    }
    if (key == "v_overstock_norm") {
        return Item::Overstock;
    }
    if (key == "v_overstock_plus") {
        return Item::Overstock_Plus;
    }
    if (key == "v_clearance_sale") {
        return Item::Clearance_Sale;
    }
    if (key == "v_liquidation") {
        return Item::Liquidation;
    }
    if (key == "v_hone") {
        return Item::Hone;
    }
    if (key == "v_glow_up") {
        return Item::Glow_Up;
    }
    if (key == "v_reroll_surplus") {
        return Item::Reroll_Surplus;
    }
    if (key == "v_reroll_glut") {
        return Item::Reroll_Glut;
    }
    if (key == "v_crystal_ball") {
        return Item::Crystal_Ball;
    }
    if (key == "v_omen_globe") {
        return Item::Omen_Globe;
    }
    if (key == "v_telescope") {
        return Item::Telescope;
    }
    if (key == "v_observatory") {
        return Item::Observatory;
    }
    if (key == "v_grabber") {
        return Item::Grabber;
    }
    if (key == "v_nacho_tong") {
        return Item::Nacho_Tong;
    }
    if (key == "v_wasteful") {
        return Item::Wasteful;
    }
    if (key == "v_recyclomancy") {
        return Item::Recyclomancy;
    }
    if (key == "v_tarot_merchant") {
        return Item::Tarot_Merchant;
    }
    if (key == "v_tarot_tycoon") {
        return Item::Tarot_Tycoon;
    }
    if (key == "v_planet_merchant") {
        return Item::Planet_Merchant;
    }
    if (key == "v_planet_tycoon") {
        return Item::Planet_Tycoon;
    }
    if (key == "v_seed_money") {
        return Item::Seed_Money;
    }
    if (key == "v_money_tree") {
        return Item::Money_Tree;
    }
    if (key == "v_blank") {
        return Item::Blank;
    }
    if (key == "v_antimatter") {
        return Item::Antimatter;
    }
    if (key == "v_magic_trick") {
        return Item::Magic_Trick;
    }
    if (key == "v_illusion") {
        return Item::Illusion;
    }
    if (key == "v_hieroglyph") {
        return Item::Hieroglyph;
    }
    if (key == "v_petroglyph") {
        return Item::Petroglyph;
    }
    if (key == "v_directors_cut") {
        return Item::Directors_Cut;
    }
    if (key == "v_paint_brush") {
        return Item::Paint_Brush;
    }
    if (key == "v_retcon") {
        return Item::Retcon;
    }
    if (key == "v_palette") {
        return Item::Palette;
    }
    return Item::RETRY;
}

Item parse_deck_key(const std::string& key) {
    if (key.empty()) {
        return Item::Red_Deck;
    }
    if (key == "b_red") {
        return Item::Red_Deck;
    }
    if (key == "b_blue") {
        return Item::Blue_Deck;
    }
    if (key == "b_yellow") {
        return Item::Yellow_Deck;
    }
    if (key == "b_green") {
        return Item::Green_Deck;
    }
    if (key == "b_black") {
        return Item::Black_Deck;
    }
    if (key == "b_magic") {
        return Item::Magic_Deck;
    }
    if (key == "b_nebula") {
        return Item::Nebula_Deck;
    }
    if (key == "b_ghost") {
        return Item::Ghost_Deck;
    }
    if (key == "b_abandoned") {
        return Item::Abandoned_Deck;
    }
    if (key == "b_checkered") {
        return Item::Checkered_Deck;
    }
    if (key == "b_zodiac") {
        return Item::Zodiac_Deck;
    }
    if (key == "b_painted") {
        return Item::Painted_Deck;
    }
    if (key == "b_anaglyph") {
        return Item::Anaglyph_Deck;
    }
    if (key == "b_plasma") {
        return Item::Plasma_Deck;
    }
    if (key == "b_erratic") {
        return Item::Erratic_Deck;
    }
    if (key == "b_challenge") {
        return Item::Challenge_Deck;
    }
    return Item::Red_Deck;
}

bool is_joker_item(Item item) {
    if (item <= Item::J_BEGIN || item >= Item::J_END) {
        return false;
    }
    if (item == Item::J_C_BEGIN || item == Item::J_C_END || item == Item::J_U_BEGIN ||
        item == Item::J_U_END || item == Item::J_R_BEGIN || item == Item::J_R_END ||
        item == Item::J_L_BEGIN || item == Item::J_L_END) {
        return false;
    }
    return true;
}

Item parse_joker_name(const std::string& name) {
    if (name.empty()) {
        return Item::RETRY;
    }
    Item item = stringToItem(name);
    if (!is_joker_item(item)) {
        return Item::RETRY;
    }
    return item;
}

JokerLocation parse_joker_location(const std::string& location) {
    if (location == "shop") {
        return JokerLocation::Shop;
    }
    if (location == "pack") {
        return JokerLocation::Pack;
    }
    return JokerLocation::Any;
}

FilterConfig make_config(const std::string& voucher,
                         const std::string& pack,
                         const std::string& tag1,
                         const std::string& tag2,
                         const std::string& joker_name,
                         const std::string& joker_location,
                         double souls,
                         bool observatory,
                         bool perkeo,
                         const std::string& deck,
                         bool erratic,
                         bool no_faces,
                         int min_face_cards,
                         double suit_ratio) {
    FilterConfig cfg;
    cfg.voucher = parse_voucher_key(voucher);
    cfg.pack = parse_pack_key(pack);
    cfg.tag1 = parse_tag_key(tag1);
    cfg.tag2 = parse_tag_key(tag2);
    cfg.joker = parse_joker_name(joker_name);
    cfg.joker_location = parse_joker_location(joker_location);
    cfg.souls = (souls > 0) ? static_cast<long>(souls) : 0;
    cfg.observatory = observatory;
    cfg.perkeo = perkeo;
    cfg.deck = parse_deck_key(deck);
    cfg.erratic = erratic;
    cfg.no_faces = no_faces;
    cfg.min_face_cards = std::max(0, min_face_cards);
    if (suit_ratio > 0.0) {
        cfg.suit_ratio = std::min(suit_ratio, 1.0);
    }
    return cfg;
}

DeckStats analyze_erratic_deck(Instance& inst, bool no_faces) {
    DeckStats stats;
    for (int i = 0; i < static_cast<int>(CARDS.size()); ++i) {
        Item card = inst.randchoice(RandomType::Erratic, CARDS);
        const int idx = static_cast<int>(card) - static_cast<int>(Item::C_2);
        if (idx < 0 || idx >= static_cast<int>(CARDS.size())) {
            continue;
        }
        const int rank_idx = idx % kCardsPerSuit;
        const int suit_idx = idx / kCardsPerSuit;
        const Item rank = kRankOrder[rank_idx];
        if (no_faces && is_face_rank(rank)) {
            continue;
        }
        stats.total++;
        if (is_face_rank(rank)) {
            stats.face_count++;
        }
        if (suit_idx >= 0 && suit_idx < kSuitCount) {
            stats.suit_count[suit_idx]++;
        }
    }
    return stats;
}

bool passes_erratic_filters(Instance& inst, const FilterConfig& cfg) {
    if (!cfg.erratic) {
        return true;
    }
    if (cfg.min_face_cards <= 0 && cfg.suit_ratio <= 0.0) {
        return true;
    }
    const DeckStats stats = analyze_erratic_deck(inst, cfg.no_faces);
    if (cfg.min_face_cards > 0 && stats.face_count < cfg.min_face_cards) {
        return false;
    }
    if (cfg.suit_ratio > 0.0) {
        if (stats.total == 0) {
            return false;
        }
        int first = 0;
        int second = 0;
        for (int count : stats.suit_count) {
            if (count >= first) {
                second = first;
                first = count;
            } else if (count > second) {
                second = count;
            }
        }
        const double ratio = static_cast<double>(first + second) / static_cast<double>(stats.total);
        if (ratio < cfg.suit_ratio) {
            return false;
        }
    }
    return true;
}

bool is_arcana_pack(Item pack) {
    return pack == Item::Arcana_Pack || pack == Item::Jumbo_Arcana_Pack ||
           pack == Item::Mega_Arcana_Pack;
}

bool is_spectral_pack(Item pack) {
    return pack == Item::Spectral_Pack || pack == Item::Jumbo_Spectral_Pack ||
           pack == Item::Mega_Spectral_Pack;
}

bool is_soulable_pack(Item pack) {
    return is_arcana_pack(pack) || is_spectral_pack(pack);
}

bool is_buffoon_pack(Item pack) {
    return pack == Item::Buffoon_Pack || pack == Item::Jumbo_Buffoon_Pack ||
           pack == Item::Mega_Buffoon_Pack;
}

int count_souls_in_pack(Instance& inst, Item pack, int ante) {
    if (!is_soulable_pack(pack)) {
        return 0;
    }
    const Pack info = packInfo(pack);
    std::vector<Item> cards;
    if (is_arcana_pack(pack)) {
        cards = inst.nextArcanaPack(info.size, ante);
    } else {
        cards = inst.nextSpectralPack(info.size, ante);
    }
    return static_cast<int>(std::count(cards.begin(), cards.end(), Item::The_Soul));
}

bool shop_has_joker(Instance& inst, Item target, int ante) {
    constexpr int kShopJokerSlots = 2;
    bool found = false;
    for (int i = 0; i < kShopJokerSlots; ++i) {
        ShopItem item = inst.nextShopItem(ante);
        if (item.type == Item::T_Joker && item.item == target) {
            found = true;
        }
    }
    return found;
}

bool pack_has_joker(Instance& inst, Item pack, Item target, int ante) {
    if (!is_buffoon_pack(pack)) {
        return false;
    }
    const Pack info = packInfo(pack);
    const std::vector<JokerData> jokers = inst.nextBuffoonPack(info.size, ante);
    for (const JokerData& joker : jokers) {
        if (joker.joker == target) {
            return true;
        }
    }
    return false;
}

bool soul_yields_perkeo(Instance& inst, int ante) {
    inst.random(RandomType::Joker_Rarity + anteToString(ante) + ItemSource::Soul);
    const JokerData joker = inst.nextJoker(ItemSource::Soul, ante, false);
    return joker.joker == Item::Perkeo;
}

int resolve_threads(int threads) {
    if (threads > 0) {
        return std::max(1, std::min(threads, 4));
    }
    const unsigned int hw = std::thread::hardware_concurrency();
    if (hw == 0) {
        return 1;
    }
    return static_cast<int>(std::min(4u, hw));
}

long long resolve_seed_budget(long long num_seeds) {
    if (num_seeds <= 0) {
        return kDefaultSeedBudget;
    }
    return num_seeds;
}

int apply_filters(Instance& inst, const FilterConfig& cfg) {
    constexpr int kAnte = 1;
    inst.initLocks(kAnte, false, false);
    inst.setDeck(cfg.deck);

    const bool wants_joker = (cfg.joker != Item::RETRY);
    const bool wants_joker_shop = wants_joker && (cfg.joker_location == JokerLocation::Shop ||
                                                  cfg.joker_location == JokerLocation::Any);
    const bool wants_joker_pack = wants_joker && (cfg.joker_location == JokerLocation::Pack ||
                                                  cfg.joker_location == JokerLocation::Any);

    const bool needs_tags = (cfg.tag1 != Item::RETRY || cfg.tag2 != Item::RETRY);
    Item small_blind = Item::RETRY;
    Item big_blind = Item::RETRY;
    if (needs_tags) {
        small_blind = inst.nextTag(kAnte);
        big_blind = inst.nextTag(kAnte);
    }

    if (cfg.tag1 != Item::RETRY || cfg.tag2 != Item::RETRY) {
        if (cfg.tag1 == Item::RETRY) {
            if (small_blind != cfg.tag2 && big_blind != cfg.tag2) {
                return 0;
            }
        } else if (cfg.tag2 == Item::RETRY) {
            if (small_blind != cfg.tag1 && big_blind != cfg.tag1) {
                return 0;
            }
        } else if (cfg.tag1 != cfg.tag2) {
            const bool has_tag1 = small_blind == cfg.tag1 || big_blind == cfg.tag1;
            const bool has_tag2 = small_blind == cfg.tag2 || big_blind == cfg.tag2;
            if (!has_tag1 || !has_tag2) {
                return 0;
            }
        } else {
            if (small_blind != cfg.tag1 || big_blind != cfg.tag1) {
                return 0;
            }
        }
    }

    const bool needs_voucher = (cfg.voucher != Item::RETRY || cfg.observatory);
    Item first_voucher = Item::RETRY;
    if (needs_voucher) {
        first_voucher = inst.nextVoucher(kAnte);
    }

    const bool needs_packs = (cfg.pack != Item::RETRY || cfg.observatory || cfg.perkeo ||
                              cfg.souls > 0 || wants_joker_pack);
    Item pack_slot_1 = Item::RETRY;
    Item pack_slot_2 = Item::RETRY;
    if (needs_packs) {
        pack_slot_1 = inst.nextPack(kAnte);
        pack_slot_2 = inst.nextPack(kAnte);
    }
    const std::array<Item, 2> pack_slots = {pack_slot_1, pack_slot_2};

    if (cfg.voucher != Item::RETRY && first_voucher != cfg.voucher) {
        return 0;
    }

    if (cfg.pack != Item::RETRY) {
        const bool pack_match = (pack_slot_1 == cfg.pack) || (pack_slot_2 == cfg.pack);
        if (!pack_match) {
            return 0;
        }
    }

    if (cfg.observatory) {
        if (first_voucher != Item::Telescope) {
            return 0;
        }
        const bool has_celestial = (pack_slot_1 == Item::Mega_Celestial_Pack) ||
                                   (pack_slot_2 == Item::Mega_Celestial_Pack);
        if (!has_celestial) {
            return 0;
        }
    }

    if (wants_joker) {
        bool joker_found = false;
        if (wants_joker_shop && shop_has_joker(inst, cfg.joker, kAnte)) {
            joker_found = true;
        }
        if (!joker_found && wants_joker_pack) {
            for (Item pack : pack_slots) {
                if (pack == Item::RETRY) {
                    continue;
                }
                if (cfg.pack != Item::RETRY && pack != cfg.pack) {
                    continue;
                }
                if (pack_has_joker(inst, pack, cfg.joker, kAnte)) {
                    joker_found = true;
                    break;
                }
            }
        }
        if (!joker_found) {
            return 0;
        }
    }

    if (cfg.perkeo || cfg.souls > 0) {
        long souls_found = 0;
        bool perkeo_found = !cfg.perkeo;
        for (Item pack : pack_slots) {
            if (pack == Item::RETRY) {
                continue;
            }
            if (cfg.pack != Item::RETRY && pack != cfg.pack) {
                continue;
            }
            if (!is_soulable_pack(pack)) {
                continue;
            }

            const int souls_in_pack = count_souls_in_pack(inst, pack, kAnte);
            if (souls_in_pack <= 0) {
                continue;
            }
            souls_found += souls_in_pack;

            if (cfg.perkeo) {
                const int uses = std::min(souls_in_pack, packInfo(pack).choices);
                for (int i = 0; i < uses; ++i) {
                    if (soul_yields_perkeo(inst, kAnte)) {
                        perkeo_found = true;
                        break;
                    }
                }
            }
            if (cfg.perkeo && perkeo_found) {
                break;
            }
        }

        if (cfg.souls > 0 && souls_found < cfg.souls) {
            return 0;
        }
        if (cfg.perkeo && !perkeo_found) {
            return 0;
        }
    }

    if (!passes_erratic_filters(inst, cfg)) {
        return 0;
    }

    return 1;
}

std::string
search_filters(const std::string& seed, const FilterConfig& cfg, long long num_seeds, int threads) {
    auto filter_fn = [&cfg](Instance& inst) -> int { return apply_filters(inst, cfg); };
    Search search(filter_fn, seed, threads, num_seeds);
    search.exitOnFind = true;
    return search.search();
}

}  // namespace

extern "C" {

IMMOLATE_API void immolate_set_log_path(const char* path) {
    // Logging disabled.
    // {
    //     std::lock_guard<std::mutex> lock(g_log_mutex);
    //     g_log_path = path ? path : "";
    // }
    // if (!g_log_path.empty()) {
    //     cpp_log(std::string("log path set to ") + g_log_path);
    // }
}

IMMOLATE_API void immolate_set_cuda_enabled(bool enabled) {
    (void)enabled;
}

IMMOLATE_API const char* brainstorm_search(const char* seed_start,
                                           const char* voucher_key,
                                           const char* pack_key,
                                           const char* tag1_key,
                                           const char* tag2_key,
                                           const char* joker_name,
                                           const char* joker_location,
                                           double souls,
                                           bool observatory,
                                           bool perkeo,
                                           const char* deck_key,
                                           bool erratic,
                                           bool no_faces,
                                           int min_face_cards,
                                           double suit_ratio,
                                           long long num_seeds,
                                           int threads) {
    // cpp_log("brainstorm_search begin");
    {
        std::ostringstream oss;
        oss << "raw args seed_start=" << safe_cstr(seed_start)
            << " voucher=" << safe_cstr(voucher_key) << " pack=" << safe_cstr(pack_key)
            << " tag1=" << safe_cstr(tag1_key) << " tag2=" << safe_cstr(tag2_key)
            << " joker=" << safe_cstr(joker_name) << " joker_location=" << safe_cstr(joker_location)
            << " souls=" << souls << " observatory=" << format_bool(observatory)
            << " perkeo=" << format_bool(perkeo) << " deck_key=" << safe_cstr(deck_key)
            << " erratic=" << format_bool(erratic) << " no_faces=" << format_bool(no_faces)
            << " min_face_cards=" << min_face_cards << " suit_ratio=" << suit_ratio
            << " num_seeds=" << num_seeds << " threads=" << threads;
        // cpp_log(oss.str());
    }
    const std::string cpp_seed(seed_start ? seed_start : "");
    const std::string cpp_voucher(voucher_key ? voucher_key : "");
    const std::string cpp_pack(pack_key ? pack_key : "");
    const std::string cpp_tag1(tag1_key ? tag1_key : "");
    const std::string cpp_tag2(tag2_key ? tag2_key : "");
    const std::string cpp_joker(joker_name ? joker_name : "");
    const std::string cpp_joker_location(joker_location ? joker_location : "");
    const std::string cpp_deck(deck_key ? deck_key : "");

    FilterConfig cfg = make_config(cpp_voucher,
                                   cpp_pack,
                                   cpp_tag1,
                                   cpp_tag2,
                                   cpp_joker,
                                   cpp_joker_location,
                                   souls,
                                   observatory,
                                   perkeo,
                                   cpp_deck,
                                   erratic,
                                   no_faces,
                                   min_face_cards,
                                   suit_ratio);
    {
        std::ostringstream oss;
        oss << "parsed config voucher=" << itemToString(cfg.voucher)
            << " pack=" << itemToString(cfg.pack) << " tag1=" << itemToString(cfg.tag1)
            << " tag2=" << itemToString(cfg.tag2) << " joker=" << itemToString(cfg.joker)
            << " souls=" << cfg.souls << " observatory=" << format_bool(cfg.observatory)
            << " perkeo=" << format_bool(cfg.perkeo) << " deck=" << itemToString(cfg.deck)
            << " erratic=" << format_bool(cfg.erratic) << " no_faces=" << format_bool(cfg.no_faces)
            << " min_face_cards=" << cfg.min_face_cards << " suit_ratio=" << cfg.suit_ratio;
        // cpp_log(oss.str());
    }

    const long long budget = resolve_seed_budget(num_seeds);
    const int thread_count = resolve_threads(threads);
    // cpp_log("search budget=" + std::to_string(budget) + " threads=" +
    // std::to_string(thread_count));

    const std::string result = search_filters(cpp_seed, cfg, budget, thread_count);
    if (result.empty()) {
        // cpp_log("search complete: no result");
        return nullptr;
    }
    // cpp_log("search complete: result=" + result);

    char* output = static_cast<char*>(std::malloc(result.size() + 1));
    if (!output) {
        // cpp_log("allocation failed for result");
        return nullptr;
    }
    std::memcpy(output, result.c_str(), result.size() + 1);

    return output;
}

IMMOLATE_API void free_result(const char* result) {
    if (result) {
        std::free(const_cast<char*>(result));
    }
}

}  // extern "C"
