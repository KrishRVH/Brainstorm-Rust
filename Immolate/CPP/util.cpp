#include "util.hpp"
#include <cstring>
#include <limits>

LuaRandom::LuaRandom(double seed) {
    double d = seed;
    uint64_t r = 0x11090601;
    for (int i = 0; i < 4; i++) {
        uint64_t m = 1ull << (r & 255);
        r >>= 8;
        d = d * 3.14159265358979323846 + 2.7182818284590452354;
        DoubleLong u;
        u.dbl = d;
        if (u.ulong < m)
            u.ulong += m;
        state[i] = u.ulong;
    }
    for (int i = 0; i < 10; i++) {
        _randint();
    }
}

LuaRandom::LuaRandom() : LuaRandom(0) {}

uint64_t LuaRandom::_randint() {
    uint64_t z = 0;
    uint64_t r = 0;
    z = state[0];
    z = (((z << 31ull) ^ z) >> 45ull) ^ ((z & (kMaxUint64 << 1ull)) << 18ull);
    r ^= z;
    state[0] = z;
    z = state[1];
    z = (((z << 19ull) ^ z) >> 30ull) ^ ((z & (kMaxUint64 << 6ull)) << 28ull);
    r ^= z;
    state[1] = z;
    z = state[2];
    z = (((z << 24ull) ^ z) >> 48ull) ^ ((z & (kMaxUint64 << 9ull)) << 7ull);
    r ^= z;
    state[2] = z;
    z = state[3];
    z = (((z << 21ull) ^ z) >> 39ull) ^ ((z & (kMaxUint64 << 17ull)) << 8ull);
    r ^= z;
    state[3] = z;
    return r;
}

uint64_t LuaRandom::randdblmem() {
    return (_randint() & 4503599627370495ull) | 4607182418800017408ull;
}

double LuaRandom::random() {
    DoubleLong u;
    u.ulong = randdblmem();
    return u.dbl - 1.0;
}

int LuaRandom::randint(int min, int max) {
    return static_cast<int>(random() * (max - min + 1)) + min;
}

int portable_clzll(uint64_t x) {
    if (x == 0)
        return 64;  // Undefined for 0, by convention we return 64

#if defined(__GNUC__) || defined(__clang__)
    return __builtin_clzll(x);
#elif defined(_MSC_VER)
    unsigned long index;
    if (_BitScanReverse64(&index, x)) {
        return 63 - index;
    }
    return 64;
#else
    // Fallback for other compilers (manual bit manipulation)
    int n = 0;
    if (x <= 0x00000000FFFFFFFF) {
        n += 32;
        x <<= 32;
    }
    if (x <= 0x0000FFFFFFFFFFFF) {
        n += 16;
        x <<= 16;
    }
    if (x <= 0x00FFFFFFFFFFFFFF) {
        n += 8;
        x <<= 8;
    }
    if (x <= 0x0FFFFFFFFFFFFFFF) {
        n += 4;
        x <<= 4;
    }
    if (x <= 0x3FFFFFFFFFFFFFFF) {
        n += 2;
        x <<= 2;
    }
    if (x <= 0x7FFFFFFFFFFFFFFF) {
        n += 1;
    }
    return n;
#endif
}

double fract(double x) {
    uint64_t x_int;

    std::memcpy(&x_int, &x, sizeof(x_int));

    uint64_t expo = (x_int & kDblExpo) >> kDblMantSize;
    if (expo < kDblExpoBias) {
        return x;
    }
    if (expo == ((1 << kDblExpoSize) - 1)) {
        return std::numeric_limits<double>::quiet_NaN();
    }
    uint64_t expo_biased = expo - kDblExpoBias;
    if (expo_biased >= kDblMantSize) {
        return 0;
    }
    uint64_t mant = x_int & kDblMant;
    uint64_t frac_mant = mant & ((1ull << (kDblMantSize - expo_biased)) - 1);
    if (frac_mant == 0) {
        return 0;
    }
    uint64_t frac_lzcnt = portable_clzll(frac_mant) - (64 - kDblMantSize);
    uint64_t res_expo = (expo - frac_lzcnt - 1) << kDblMantSize;
    uint64_t res_mant = (frac_mant << (frac_lzcnt + 1)) & kDblMant;
    uint64_t res = res_expo | res_mant;

    double result;
    std::memcpy(&result, &res, sizeof(result));
    return result;
}

double pseudohash(const std::string& s) {
    double num = 1;
    for (std::size_t i = s.length(); i > 0; i--) {
        num =
            fract(1.1239285023 / num * static_cast<unsigned char>(s[i - 1]) * 3.141592653589793116 +
                  3.141592653589793116 * i);
    }
    return num;
}

double pseudohash_from(const std::string& s, double num) {
    for (std::size_t i = s.length(); i > 0; i--) {
        num =
            fract(1.1239285023 / num * static_cast<unsigned char>(s[i - 1]) * 3.141592653589793116 +
                  3.141592653589793116 * i);
    }
    return num;
}

double pseudostep(char s, int pos, double num) {
    return fract(1.1239285023 / num * s * 3.141592653589793116 + 3.141592653589793116 * pos);
}

std::string anteToString(int a) {
    if (a < 10) {
        return {static_cast<char>('0' + a)};
    }
    return {static_cast<char>('0' + a / 10), static_cast<char>('0' + a % 10)};
}

const double kInvPrec = std::pow(10.0, 13);
const double kTwoInvPrec = std::pow(2.0, 13);
const double kFiveInvPrec = std::pow(5.0, 13);

double round13(double x) {
    double normal_case = std::round(x * kInvPrec) / kInvPrec;
    if (normal_case == (std::round(std::nextafter(x, -1) * kInvPrec) / kInvPrec)) {
        return normal_case;
    }
    double truncated = fract(x * kTwoInvPrec) * kFiveInvPrec;
    if (fract(truncated) >= 0.5) {
        return (std::floor(x * kInvPrec) + 1) / kInvPrec;
    }
    return std::floor(x * kInvPrec) / kInvPrec;
}
