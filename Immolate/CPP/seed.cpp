#include "seed.hpp"
#include "util.hpp"

Seed::Seed() {
    seed.fill(-1);
    length = 0;
    for (int i = 0; i < 8; i++) {
        cache[i].fill(-1);
    }
}

Seed::Seed(const std::string& strSeed) {
    seed.fill(-1);
    length = static_cast<int>(strSeed.size());
    for (int i = 0; i < 8; i++) {
        cache[i].fill(-1);
    }
    // Note: Assumes this is safe
    for (std::size_t i = 0; i < strSeed.size(); i++) {
        seed[strSeed.size() - 1 - i] = kCharSeeds[static_cast<unsigned char>(strSeed[i])];
    }
}

Seed::Seed(long long id) {
    length = 0;
    for (int i = 0; i < 8; i++) {
        cache[i].fill(-1);
    }
    for (int i = 0; i < 8; i++) {
        if (id > 0) {
            length++;
            seed[i] = (id - 1) / kIdCoeff[i];
            id -= 1 + seed[i] * kIdCoeff[i];
        } else {
            seed[i] = -1;
        }
    }
}

std::string Seed::tostring() const {
    std::string strSeed;
    for (int i = 7; i >= 0; i--) {
        if (seed[i] != -1) {
            strSeed.push_back(kSeedChars[seed[i]]);
        }
    }
    return strSeed;
}

long long Seed::getID() const {
    long long id = 0;
    for (int i = 0; i <= 7; i++) {
        if (seed[i] >= 0) {
            id += kIdCoeff[i] * seed[i] + 1;
        }
    }
    return id;
}

void Seed::next() {
    if (length < 8) {
        seed[length] = 0;
        length++;
    } else {
        int i = 7;
        while (i >= 0) {
            cache[i].fill(-1);
            if (seed[i] == 34) {
                seed[i] = -1;
                length--;
            } else {
                seed[i]++;
                break;
            }
            i--;
        }
    }
}

// Not optimized for performance
// I don't think this will need to be implemented in searching
void Seed::next(int x) {
    constexpr long long kSeedSpace = 2318107019761LL;
    long long newID = (getID() + x) % kSeedSpace;
    *this = Seed(newID);
}

double Seed::pseudohash(int prefixLength) {
    if (length == 0)
        return 1;  // Empty seed edge case

    if (cache[length - 1][prefixLength + length - 1] == -1) {
        int i = length - 2;
        while (i >= 0 && cache[i][prefixLength + length - 1] == -1) {
            i--;
        }
        if (i == -1) {
            cache[0][prefixLength + length - 1] =
                pseudostep(kSeedChars[seed[0]], prefixLength + length, 1);
            i = 0;
        }
        for (int j = i + 1; j < length; j++) {
            cache[j][prefixLength + length - 1] =
                pseudostep(kSeedChars[seed[j]],
                           prefixLength + length - j,
                           cache[j - 1][prefixLength + length - 1]);
        }
    }
    return cache[length - 1][prefixLength + length - 1];
}
