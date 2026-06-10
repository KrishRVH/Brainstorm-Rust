#ifndef UTIL_HPP
#define UTIL_HPP

#include <array>
#include <cmath>
#include <cstdint>
#include <limits>
#include <string>

constexpr uint64_t kMaxUint64 = std::numeric_limits<uint64_t>::max();

union DoubleLong {
    double dbl;
    uint64_t ulong;
};

struct LuaRandom {
    std::array<uint64_t, 4> state{};
    explicit LuaRandom(double seed);
    LuaRandom();
    uint64_t _randint();
    uint64_t randdblmem();
    double random();
    int randint(int min, int max);
};

constexpr uint64_t kDblExpo = 0x7FF0000000000000ull;
constexpr uint64_t kDblMant = 0x000FFFFFFFFFFFFFull;
constexpr int kDblExpoSize = 11;
constexpr int kDblMantSize = 52;
constexpr int kDblExpoBias = 1023;

#if defined(_MSC_VER)
    #include <intrin.h>
    #pragma intrinsic(_BitScanReverse64)
#endif

int portable_clzll(uint64_t x);
double fract(double x);
double pseudohash(const std::string& s);
double pseudohash_from(const std::string& s, double num);
double pseudostep(char s, int pos, double num);
std::string anteToString(int a);
double round13(double x);

#endif  // UTIL_HPP
