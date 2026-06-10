#ifndef INSTANCE_HPP
#define INSTANCE_HPP

#include "items.hpp"
#include "seed.hpp"
#include "util.hpp"
#include <array>
#include <map>
#include <string>

struct Cache {
    std::map<std::string, double> nodes;
    bool generatedFirstPack = false;
};

struct InstParams {
    Item deck{Item::Red_Deck};
    Item stake{Item::White_Stake};
    bool showman{false};
    int sixesFactor{1};
    long version{10103};  // 1.0.1c
    std::array<bool, 32> vouchers{};
    InstParams() = default;
    InstParams(Item d, Item s, bool show, long v)
        : deck(d), stake(s), showman(show), sixesFactor(1), version(v) {}
};

struct Instance {
    static constexpr std::size_t kItemCount = static_cast<std::size_t>(Item::ITEMS_END);
    std::array<bool, kItemCount> locked{};
    Seed& seed;
    double hashedSeed;
    Cache cache;
    InstParams params;
    LuaRandom rng;
    explicit Instance(Seed& s) : seed(s), hashedSeed(s.pseudohash(0)), params(), rng(0) {}
    void reset(Seed& s) {  // This is slow, use next() unless necessary
        seed = s;
        hashedSeed = s.pseudohash(0);
        params = InstParams();
        cache.nodes.clear();  // Somehow `clear` is faster than swapping with empty map
        cache.generatedFirstPack = false;
    }
    void next() {
        seed.next();
        hashedSeed = seed.pseudohash(0);
        params = InstParams();
        cache.nodes.clear();
        cache.generatedFirstPack = false;
    }
    double get_node(const std::string& id) {
        auto it = cache.nodes.find(id);
        if (it == cache.nodes.end()) {
            it = cache.nodes
                     .emplace(id, pseudohash_from(id, seed.pseudohash(static_cast<int>(id.size()))))
                     .first;
        }
        it->second = round13(fract(it->second * 1.72431234 + 2.134453429141));
        return (it->second + hashedSeed) / 2;
    }
    double random(const std::string& id) {
        rng = LuaRandom(get_node(id));
        return rng.random();
    }
    int randint(const std::string& id, int min, int max) {
        rng = LuaRandom(get_node(id));
        return rng.randint(min, max);
    }
    template <std::size_t N>
    Item randchoice(const std::string& id, const std::array<Item, N>& items) {
        rng = LuaRandom(get_node(id));
        Item item = items[rng.randint(0, static_cast<int>(items.size() - 1))];
        if ((!params.showman && isLocked(item)) || item == Item::RETRY) {
            int resample = 2;
            while (true) {
                rng = LuaRandom(get_node(id + "_resample" + anteToString(resample)));
                Item candidate = items[rng.randint(0, static_cast<int>(items.size() - 1))];
                resample++;
                if ((candidate != Item::RETRY && !isLocked(candidate)) || resample > 1000)
                    return candidate;
            }
        }
        return item;
    }
    template <std::size_t N>
    Item randweightedchoice(const std::string& id, const std::array<WeightedItem, N>& items) {
        rng = LuaRandom(get_node(id));
        double poll = rng.random() * items[0].weight;
        std::size_t idx = 1;
        double weight = 0.0;
        while (weight < poll) {
            weight += items[idx].weight;
            idx++;
        }
        return items[idx - 1].item;
    }

    // Functions defined in functions.cpp
    void lock(Item item);
    void unlock(Item item);
    bool isLocked(Item item) const;
    void initLocks(int ante, bool freshProfile, bool freshRun);
    void initUnlocks(int ante, bool freshProfile);
    Item nextTarot(const std::string& source, int ante, bool soulable);
    Item nextPlanet(const std::string& source, int ante, bool soulable);
    Item nextSpectral(const std::string& source, int ante, bool soulable);
    JokerData nextJoker(const std::string& source, int ante, bool hasStickers);
    ShopInstance getShopInstance() const;
    ShopItem nextShopItem(int ante);
    Item nextPack(int ante);
    std::vector<Item> nextArcanaPack(int size, int ante);
    std::vector<Item> nextCelestialPack(int size, int ante);
    std::vector<Item> nextSpectralPack(int size, int ante);
    std::vector<JokerData> nextBuffoonPack(int size, int ante);
    std::vector<Card> nextStandardPack(int size, int ante);
    Card nextStandardCard(int ante);
    bool isVoucherActive(Item voucher) const;
    void activateVoucher(Item voucher);
    Item nextVoucher(int ante);
    void setDeck(Item deck);
    void setStake(Item stake);
    Item nextTag(int ante);
    Item nextBoss(int ante);
};

#endif  // INSTANCE_HPP
