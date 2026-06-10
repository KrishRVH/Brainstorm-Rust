#include <stdint.h>

struct CudaFilterParams {
    uint32_t tag1;
    uint32_t tag2;
    uint32_t voucher;
    uint32_t pack;
    uint32_t flags;
};

static constexpr uint32_t FLAG_TAGS = 1u << 0;
static constexpr uint32_t FLAG_VOUCHER = 1u << 1;
static constexpr uint32_t FLAG_PACKS = 1u << 2;
static constexpr uint32_t FLAG_OBSERVATORY = 1u << 3;

static constexpr int64_t SEED_SPACE = 2318107019761LL;
static constexpr uint64_t K_MAX_U64 = 0xffffffffffffffffULL;
static constexpr uint64_t K_DBL_EXPO = 0x7ff0000000000000ULL;
static constexpr uint64_t K_DBL_MANT = 0x000fffffffffffffULL;
static constexpr uint64_t K_DBL_MANT_SIZE = 52ULL;
static constexpr uint64_t K_DBL_EXPO_SIZE = 11ULL;
static constexpr uint64_t K_DBL_EXPO_BIAS = 1023ULL;
static constexpr double PI_HASH = 3.141592653589793116;

static __device__ __constant__ int64_t ID_COEFF[8] = {
    66231629136LL, 1892332261LL, 54066636LL, 1544761LL,
    44136LL,       1261LL,       36LL,       1LL,
};

static __device__ __constant__ uint8_t SEED_CHARS[35] = {
    '1','2','3','4','5','6','7','8','9','A','B','C','D','E','F','G','H','I',
    'J','K','L','M','N','O','P','Q','R','S','T','U','V','W','X','Y','Z',
};

static __device__ __constant__ uint32_t TAG_POOL[24] = {
    310, 311, 312, 313, 314, 315, 316, 317, 318, 319, 320, 321,
    322, 323, 324, 325, 326, 327, 328, 329, 330, 331, 332, 333,
};

static __device__ __constant__ uint32_t VOUCHER_POOL[32] = {
    162, 163, 164, 165, 166, 167, 168, 169,
    170, 171, 172, 173, 174, 175, 176, 177,
    178, 179, 180, 181, 182, 183, 184, 185,
    186, 187, 188, 189, 190, 191, 192, 193,
};

static __device__ __constant__ uint32_t PACK_ITEMS[16] = {
    0, 293, 294, 295, 296, 297, 298, 299,
    300, 301, 302, 303, 304, 305, 306, 307,
};

static __device__ __constant__ double PACK_WEIGHTS[16] = {
    22.42, 4.0, 2.0, 0.5, 4.0, 2.0, 0.5, 4.0,
    2.0,   0.5, 1.2, 0.6, 0.15, 0.6, 0.3, 0.07,
};

struct SeedState {
    int16_t seed[8];
    uint32_t length;
};

struct LuaRandom {
    uint64_t state[4];
};

static __device__ __forceinline__ uint64_t double_to_bits(double x) {
    return static_cast<uint64_t>(__double_as_longlong(x));
}

static __device__ __forceinline__ double bits_to_double(uint64_t x) {
    return __longlong_as_double(static_cast<long long>(x));
}

static __device__ double fract_bits(double x) {
    uint64_t x_int = double_to_bits(x);
    uint64_t expo = (x_int & K_DBL_EXPO) >> K_DBL_MANT_SIZE;
    if (expo < K_DBL_EXPO_BIAS) {
        return x;
    }
    if (expo == ((1ULL << K_DBL_EXPO_SIZE) - 1ULL)) {
        return bits_to_double(0x7ff8000000000000ULL);
    }
    uint64_t expo_biased = expo - K_DBL_EXPO_BIAS;
    if (expo_biased >= K_DBL_MANT_SIZE) {
        return 0.0;
    }
    uint64_t mant = x_int & K_DBL_MANT;
    uint64_t frac_mant = mant & ((1ULL << (K_DBL_MANT_SIZE - expo_biased)) - 1ULL);
    if (frac_mant == 0ULL) {
        return 0.0;
    }
    uint64_t frac_lzcnt = static_cast<uint64_t>(__clzll(frac_mant)) - (64ULL - K_DBL_MANT_SIZE);
    uint64_t res_expo = (expo - frac_lzcnt - 1ULL) << K_DBL_MANT_SIZE;
    uint64_t res_mant = (frac_mant << (frac_lzcnt + 1ULL)) & K_DBL_MANT;
    return bits_to_double(res_expo | res_mant);
}

static __device__ double next_down_for_positive_hash(double x) {
    if (x == 0.0) {
        return -bits_to_double(1ULL);
    }
    uint64_t bits = double_to_bits(x);
    if (x > 0.0) {
        return bits_to_double(bits - 1ULL);
    }
    return bits_to_double(bits + 1ULL);
}

static __device__ double round13(double x) {
    constexpr double INV_PREC = 10000000000000.0;
    constexpr double TWO_INV_PREC = 8192.0;
    constexpr double FIVE_INV_PREC = 1220703125.0;

    double normal_case = floor(x * INV_PREC + 0.5) / INV_PREC;
    double previous_case = floor(next_down_for_positive_hash(x) * INV_PREC + 0.5) / INV_PREC;
    if (normal_case == previous_case) {
        return normal_case;
    }
    double truncated = fract_bits(x * TWO_INV_PREC) * FIVE_INV_PREC;
    if (fract_bits(truncated) >= 0.5) {
        return (floor(x * INV_PREC) + 1.0) / INV_PREC;
    }
    return floor(x * INV_PREC) / INV_PREC;
}

static __device__ __forceinline__ double pseudostep(uint8_t byte, uint32_t pos, double num) {
    return fract_bits(1.1239285023 / num * static_cast<double>(byte) * PI_HASH +
                      PI_HASH * static_cast<double>(pos));
}

static __device__ SeedState seed_from_id(int64_t raw_id) {
    int64_t id = raw_id % SEED_SPACE;
    if (id < 0) {
        id += SEED_SPACE;
    }

    SeedState out{};
    out.length = 0;
    #pragma unroll
    for (int i = 0; i < 8; ++i) {
        if (id > 0) {
            out.length += 1;
            out.seed[i] = static_cast<int16_t>((id - 1) / ID_COEFF[i]);
            id -= 1 + static_cast<int64_t>(out.seed[i]) * ID_COEFF[i];
        } else {
            out.seed[i] = -1;
        }
    }
    return out;
}

static __device__ __forceinline__ uint8_t seed_char(const SeedState& seed, uint32_t index) {
    int16_t idx = seed.seed[index];
    if (idx < 0 || idx >= 35) {
        return '?';
    }
    return SEED_CHARS[idx];
}

static __device__ double seed_pseudohash(const SeedState& seed, uint32_t prefix_len) {
    if (seed.length == 0) {
        return 1.0;
    }
    double num = 1.0;
    for (uint32_t j = 0; j < seed.length; ++j) {
        num = pseudostep(seed_char(seed, j), prefix_len + seed.length - j, num);
    }
    return num;
}

template <uint32_t N>
static __device__ double pseudohash_from_key(const uint8_t (&key)[N], double num) {
    for (int i = static_cast<int>(N) - 2; i >= 0; --i) {
        num = pseudostep(key[i], static_cast<uint32_t>(i + 1), num);
    }
    return num;
}

static __device__ double initial_node_tag1(const SeedState& seed) {
    static constexpr uint8_t KEY[] = "Tag1";
    return pseudohash_from_key(KEY, seed_pseudohash(seed, 4));
}

static __device__ double initial_node_voucher1(const SeedState& seed) {
    static constexpr uint8_t KEY[] = "Voucher1";
    return pseudohash_from_key(KEY, seed_pseudohash(seed, 8));
}

static __device__ double initial_node_shop_pack1(const SeedState& seed) {
    static constexpr uint8_t KEY[] = "shop_pack1";
    return pseudohash_from_key(KEY, seed_pseudohash(seed, 10));
}

static __device__ double initial_node_tag1_resample(const SeedState& seed, int resample) {
    uint8_t key[32];
    const uint8_t prefix[] = "Tag1_resample";
    uint32_t len = 0;
    for (uint32_t i = 0; i < sizeof(prefix) - 1; ++i) {
        key[len++] = prefix[i];
    }
    if (resample >= 100) {
        key[len++] = static_cast<uint8_t>('0' + (resample / 100) % 10);
    }
    if (resample >= 10) {
        key[len++] = static_cast<uint8_t>('0' + (resample / 10) % 10);
    }
    key[len++] = static_cast<uint8_t>('0' + resample % 10);

    double num = seed_pseudohash(seed, len);
    for (int i = static_cast<int>(len) - 1; i >= 0; --i) {
        num = pseudostep(key[i], static_cast<uint32_t>(i + 1), num);
    }
    return num;
}

static __device__ void lua_random_init(LuaRandom* rng, double seed) {
    double d = seed;
    uint64_t r = 0x11090601ULL;
    #pragma unroll
    for (int i = 0; i < 4; ++i) {
        uint64_t m = 1ULL << (r & 255ULL);
        r >>= 8;
        d = d * 3.14159265358979323846264338327950288 + 2.71828182845904523536028747135266250;
        uint64_t bits = double_to_bits(d);
        if (bits < m) {
            bits += m;
        }
        rng->state[i] = bits;
    }
    #pragma unroll
    for (int i = 0; i < 10; ++i) {
        uint64_t result = 0ULL;
        uint64_t z = rng->state[0];
        z = (((z << 31) ^ z) >> 45) ^ ((z & (K_MAX_U64 << 1)) << 18);
        result ^= z;
        rng->state[0] = z;
        z = rng->state[1];
        z = (((z << 19) ^ z) >> 30) ^ ((z & (K_MAX_U64 << 6)) << 28);
        result ^= z;
        rng->state[1] = z;
        z = rng->state[2];
        z = (((z << 24) ^ z) >> 48) ^ ((z & (K_MAX_U64 << 9)) << 7);
        result ^= z;
        rng->state[2] = z;
        z = rng->state[3];
        z = (((z << 21) ^ z) >> 39) ^ ((z & (K_MAX_U64 << 17)) << 8);
        result ^= z;
        rng->state[3] = z;
    }
}

static __device__ uint64_t lua_randint_raw(LuaRandom* rng) {
    uint64_t result = 0ULL;
    uint64_t z = rng->state[0];
    z = (((z << 31) ^ z) >> 45) ^ ((z & (K_MAX_U64 << 1)) << 18);
    result ^= z;
    rng->state[0] = z;

    z = rng->state[1];
    z = (((z << 19) ^ z) >> 30) ^ ((z & (K_MAX_U64 << 6)) << 28);
    result ^= z;
    rng->state[1] = z;

    z = rng->state[2];
    z = (((z << 24) ^ z) >> 48) ^ ((z & (K_MAX_U64 << 9)) << 7);
    result ^= z;
    rng->state[2] = z;

    z = rng->state[3];
    z = (((z << 21) ^ z) >> 39) ^ ((z & (K_MAX_U64 << 17)) << 8);
    result ^= z;
    rng->state[3] = z;
    return result;
}

static __device__ double lua_random(double seed) {
    LuaRandom rng;
    lua_random_init(&rng, seed);
    uint64_t bits = (lua_randint_raw(&rng) & 4503599627370495ULL) | 4607182418800017408ULL;
    return bits_to_double(bits) - 1.0;
}

static __device__ int lua_randint(double seed, int min, int max) {
    return static_cast<int>(lua_random(seed) * static_cast<double>(max - min + 1)) + min;
}

static __device__ double advance_node(double* node, double hashed_seed) {
    *node = round13(fract_bits((*node) * 1.72431234 + 2.134453429141));
    return ((*node) + hashed_seed) / 2.0;
}

static __device__ uint32_t randchoice_tag(
    const SeedState& seed,
    double hashed_seed,
    double* tag_node,
    int previous_resample_max,
    int* resample_max
) {
    double seed_value = advance_node(tag_node, hashed_seed);
    int idx = lua_randint(seed_value, 0, 23);
    uint32_t item = TAG_POOL[idx];
    if (!(item == 312 || item == 319 || item == 321 || item == 322 || item == 323 ||
          item == 324 || item == 325 || item == 330 || item == 332)) {
        return item;
    }

    for (int resample = 2; resample <= 1000; ++resample) {
        double node = initial_node_tag1_resample(seed, resample);
        if (resample <= previous_resample_max) {
            advance_node(&node, hashed_seed);
        }
        seed_value = advance_node(&node, hashed_seed);
        if (resample > *resample_max) {
            *resample_max = resample;
        }
        idx = lua_randint(seed_value, 0, 23);
        item = TAG_POOL[idx];
        if (!(item == 312 || item == 319 || item == 321 || item == 322 || item == 323 ||
              item == 324 || item == 325 || item == 330 || item == 332) ||
            resample > 1000) {
            return item;
        }
    }
    return item;
}

static __device__ uint32_t next_voucher(const SeedState& seed, double hashed_seed) {
    double node = initial_node_voucher1(seed);
    double seed_value = advance_node(&node, hashed_seed);
    int idx = lua_randint(seed_value, 0, 31);
    uint32_t item = VOUCHER_POOL[idx];
    if (item != 0) {
        return item;
    }
    return item;
}

static __device__ uint32_t roll_second_pack(const SeedState& seed, double hashed_seed) {
    double node = initial_node_shop_pack1(seed);
    double seed_value = advance_node(&node, hashed_seed);
    double poll = lua_random(seed_value) * PACK_WEIGHTS[0];
    double weight = 0.0;
    for (int i = 1; i < 16; ++i) {
        weight += PACK_WEIGHTS[i];
        if (weight >= poll) {
            return PACK_ITEMS[i];
        }
    }
    return PACK_ITEMS[15];
}

static __device__ bool passes_filter(int64_t seed_id, const CudaFilterParams* params) {
    SeedState seed = seed_from_id(seed_id);
    double hashed_seed = seed_pseudohash(seed, 0);

    if (params->flags & FLAG_TAGS) {
        double tag_node = initial_node_tag1(seed);
        int first_resample_max = 1;
        uint32_t small = randchoice_tag(seed, hashed_seed, &tag_node, 1, &first_resample_max);
        int second_resample_max = first_resample_max;
        uint32_t big = randchoice_tag(seed, hashed_seed, &tag_node, first_resample_max, &second_resample_max);
        uint32_t tag1 = params->tag1;
        uint32_t tag2 = params->tag2;
        if (tag1 == 0 && tag2 != 0) {
            if (small != tag2 && big != tag2) return false;
        } else if (tag2 == 0 && tag1 != 0) {
            if (small != tag1 && big != tag1) return false;
        } else if (tag1 != 0 && tag2 != 0 && tag1 != tag2) {
            bool has_tag1 = small == tag1 || big == tag1;
            bool has_tag2 = small == tag2 || big == tag2;
            if (!has_tag1 || !has_tag2) return false;
        } else if (tag1 != 0) {
            if (small != tag1 || big != tag1) return false;
        }
    }

    uint32_t first_voucher = 0;
    if (params->flags & FLAG_VOUCHER) {
        first_voucher = next_voucher(seed, hashed_seed);
        if (params->voucher != 0 && first_voucher != params->voucher) {
            return false;
        }
    }

    uint32_t pack1 = 0;
    uint32_t pack2 = 0;
    if (params->flags & FLAG_PACKS) {
        pack1 = 302; // First ante-1 shop slot is Buffoon Pack in the current CPU oracle.
        pack2 = roll_second_pack(seed, hashed_seed);
        if (params->pack != 0 && pack1 != params->pack && pack2 != params->pack) {
            return false;
        }
    }

    if (params->flags & FLAG_OBSERVATORY) {
        if (first_voucher != 172) return false;
        if (pack1 != 298 && pack2 != 298) return false;
    }

    return true;
}

extern "C" __global__ void brainstorm_search_kernel(
    int64_t start_seed,
    int64_t count,
    const CudaFilterParams* params,
    uint64_t* best_offset
) {
    uint64_t tid = static_cast<uint64_t>(blockIdx.x) * blockDim.x + threadIdx.x;
    uint64_t stride = static_cast<uint64_t>(gridDim.x) * blockDim.x;
    uint64_t n = static_cast<uint64_t>(count);

    for (uint64_t offset = tid; offset < n; offset += stride) {
        uint64_t current_best = *best_offset;
        if (offset >= current_best) {
            continue;
        }
        int64_t seed_id = (start_seed + static_cast<int64_t>(offset)) % SEED_SPACE;
        if (seed_id < 0) {
            seed_id += SEED_SPACE;
        }
        if (passes_filter(seed_id, params)) {
            atomicMin(reinterpret_cast<unsigned long long*>(best_offset),
                      static_cast<unsigned long long>(offset));
        }
    }
}

extern "C" __global__ void brainstorm_debug_seed_kernel(int64_t seed_id, uint64_t* out) {
    if (threadIdx.x != 0 || blockIdx.x != 0) {
        return;
    }
    SeedState seed = seed_from_id(seed_id);
    double hashed_seed = seed_pseudohash(seed, 0);
    double tag_node = initial_node_tag1(seed);
    int first_resample_max = 1;
    uint32_t small = randchoice_tag(seed, hashed_seed, &tag_node, 1, &first_resample_max);
    int second_resample_max = first_resample_max;
    uint32_t big = randchoice_tag(seed, hashed_seed, &tag_node, first_resample_max, &second_resample_max);
    uint32_t voucher = next_voucher(seed, hashed_seed);
    uint32_t pack = roll_second_pack(seed, hashed_seed);
    out[0] = double_to_bits(hashed_seed);
    out[1] = small;
    out[2] = big;
    out[3] = voucher;
    out[4] = pack;
}
