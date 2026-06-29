# Immolate Rust Current State

Last updated: 2026-06-29

This document records the current Rust DLL architecture and maintenance rules.
The Rust rewrite is complete. Brainstorm now has one Rust implementation, built
by default as `Immolate.dll`. Rust CPU is the correctness oracle for CUDA, and
the bundled legacy Original Brainstorm DLL is used only as a historical
benchmark baseline where its older ABI can represent the same fixture. The Rust
DLL embeds a CUDA PTX fast path for the highest-volume ante-1 filters.

For benchmark operation details, use `Immolate/Rust/BENCH.md`.

## Current Status

- `mise run build` and `mise run build-rust` build the Rust DLL and copy it to the repo
  root as `Immolate.dll`.
- Rust source lives under `Immolate/Rust/`.
- CUDA source lives under `Immolate/Rust/src/cuda/` and is compiled to embedded
  PTX by `Immolate/Rust/build.rs`.
- There is no C++ source tree or alternate native implementation in this
  branch.
- `mise run compare` is an alias for `mise run bench-compare`.
- `mise run bench-compare` benchmarks Rust CPU, Rust CUDA, and the legacy
  Original Brainstorm DLL through the Windows ABI harness.
- `mise run bench-compare-windows` launches the same harness as a native
  Windows process from WSL, which is the game-like CUDA benchmark path.
- `mise run bench-cuda-long-windows` runs the long CUDA fixtures with a
  25,000,000 seed budget.
- Benchmark comparison fails when Rust CUDA returns a different result than
  Rust CPU. A faster wrong seed is a failure.

## Runtime Contract

Lua loads `Brainstorm.PATH .. "/Immolate.dll"` and calls:

```c
const char* brainstorm_search(
    const char* seed_start,
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
    int threads
);

void immolate_set_log_path(const char* path);
void immolate_set_cuda_enabled(bool enabled);
const char* immolate_last_error(void);
void free_result(char* result);
```

FFI rules:

- Null C string pointers are treated as empty strings.
- Balatro keys are accepted for tags, vouchers, packs, and decks.
- Joker input accepts the UI-facing joker name and known internal aliases.
- No result returns a null pointer.
- A non-empty result is returned as an owned C string and must be released with
  `free_result`.
- `immolate_set_log_path` is a no-op while logging remains disabled.
- Rust exports only `brainstorm_search`, `free_result`, `immolate_last_error`,
  `immolate_set_log_path`, and `immolate_set_cuda_enabled`.
- `immolate_set_cuda_enabled` is the native side of the Brainstorm UI CUDA
  toggle. It does not affect unsupported filters, which still use CPU fallback.

## Source Layout

- `Immolate/Rust/src/ffi.rs`: C ABI boundary and string/result ownership.
- `Immolate/Rust/src/lib.rs`: public Rust API and unit tests.
- `Immolate/Rust/src/filters.rs`: raw UI/FFI filter parsing into `FilterConfig`.
- `Immolate/Rust/src/item.rs`: item, joker, pack, voucher, deck, and tag tables.
- `Immolate/Rust/src/rng.rs`: bit-compatible Lua/Balatro RNG primitives.
- `Immolate/Rust/src/seed.rs`: seed space and seed ordering.
- `Immolate/Rust/src/search.rs`: shared budget/thread helpers.
- `Immolate/Rust/src/engine/`: optimized search implementation.
- `Immolate/Rust/src/engine/cuda.rs`: dynamic CUDA Driver API loader and GPU
  search dispatch.
- `Immolate/Rust/src/cuda/brainstorm_cuda.cu`: GPU implementation of the
  supported fast-path filters.
- `Immolate/Rust/src/bench_cases.rs`: shared benchmark fixture catalog.
- `Immolate/Rust/src/bin/immolate_dll_harness.rs`: Windows ABI comparison and
  benchmark harness.
- `Immolate/Rust/src/bin/brainstorm_bench.rs`: native Rust-only profiling
  helper.

The public Rust entry point is `immolate::brainstorm_search_core`. The engine
uses specialized kernels where possible and a generic correctness fallback for
unsupported filter combinations.

## Search Architecture

The current implementation compiles a `FilterConfig` into a `CompiledFilter`
with a `KernelShape`. Shapes include:

- `NoMatch`
- `NoFilter`
- `TagOnly`
- `VoucherOnly`
- `PackOnly`
- `Observatory`
- `ShopJoker`
- `PackJoker`
- `AnyJoker`
- `Souls`
- `Perkeo`
- `Erratic`
- `TagObservatory`
- `SpectralSoulPerkeo`
- `Composite`
- `Generic`

Optimized CPU kernels live in `engine/kernels.rs`. `Composite` handles real UI
combinations such as tag+voucher+pack, voucher+pack, tag+joker, Souls+pack, and
Erratic+tag without falling all the way back to the instance-driven generic
path.
`Generic` remains as a correctness fallback for unsupported combinations.

`engine/search.rs` tries CUDA before CPU once a filter is compiled and the
Brainstorm UI has CUDA enabled. CUDA is currently supported for Red Deck filters
made from tags, voucher, pack, Observatory constraints, and one-Soul
Arcana/Spectral pack searches. Joker search, Perkeo, Erratic Deck, non-red
decks, and any unsupported composite continue through the Rust CPU engine. CUDA
failures are treated as unavailable and do not change search results.

The CUDA search kernels keep the game-facing path GPU-oriented: filter params
are passed as kernel params, selected Spectral Soul searches dispatch to a
narrower kernel, search launches do not cross the seed-space wrap boundary, and
the hot search kernels decode already-normalized seed ids so generated PTX does
not contain 64-bit integer divide/modulo instructions for seed decoding.

Parallel search preserves earliest-seed semantics. For parallel runs, Rust scans
the first chunk serially, then searches remaining chunks in parallel while
tracking the earliest hit by seed offset. This avoids returning a later seed
just because another worker found it first.

`threads=0` means "auto" and is what the Lua auto-reroll UI passes to the DLL.
Use `BENCH_THREADS=0` for actual user-experience benchmarking. Use
`BENCH_THREADS=1` for single-thread implementation comparisons.

## Correctness Invariants

The Rust CPU implementation is the oracle for CUDA unless an explicit product
decision says otherwise.

Important invariants:

- Preserve Balatro/Lua RNG behavior exactly; do not replace it with Rust RNGs or
  approximate floating-point math.
- Preserve CUDA/Rust parity for seed ordering, `pseudohash`, `pseudoseed`, Lua
  RNG initialization, tag resample state, voucher rolls, and pack weights.
- Preserve seed ordering and wrapping over the full seed space.
- Preserve the FFI signature and result ownership rules.
- Preserve first-shop behavior for vouchers, pack slots, Buffoon pack joker
  searches, Souls, Observatory, Perkeo, and Erratic Deck filters.
- Preserve earliest matching seed behavior for both single-thread and parallel
  search.
- Treat static impossible filters as `NoMatch` only when they are impossible
  from the UI/contract, not merely unlikely.

## Build And Validation

Routine validation:

```bash
mise run check-rust
```

That runs Rust formatting, clippy, unit tests, DLL export/import validation,
and small benchmark smoke tests.

CUDA build/runtime knobs:

- `BRAINSTORM_CUDA_ARCH=sm_89` selects the `nvcc` target architecture. `sm_89`
  is the default for RTX 4090.
- `NVCC=/path/to/nvcc` and `CUDAHOSTCXX=/path/to/gcc` override the CUDA compiler
  and host compiler.
- The Brainstorm settings tab controls whether the runtime tries CUDA. Do not
  add hidden runtime heuristics for deciding whether to use the GPU path.
- `BRAINSTORM_CUDA_LAUNCH_SEEDS` controls seeds per CUDA launch chunk.
- `BRAINSTORM_SKIP_CUDA_BUILD=1` embeds empty PTX for intentional CPU-only
  builds.

Strict full-suite performance gate:

```bash
mise run bench-full
```

DLL UX gate:

```bash
mise run bench-ux
```

Pretty full-suite dashboard:

```bash
mise run bench-pretty
```

## Current Local Validation State

The current branch was validated locally after installing CUDA Toolkit 12.4,
GCC 12, MinGW-w64, Wine, and using Windows PowerShell from WSL:

- `mise run doctor`
- `mise run check-rust`
- `WINEDEBUG=-all mise run bench-full`
- `WINEDEBUG=-all mise run bench-ux`
- `mise run bench-cuda-long-windows`
- Native Linux CUDA benchmark:
  `cargo run --manifest-path Immolate/Rust/Cargo.toml --release --bin brainstorm_bench -- --case ux-tag-voucher-pack --budget 1000000 --threads 0 --repeat 3 --warmup 1`
- Native Windows CUDA long benchmark:
  `mise run bench-cuda-long-windows` scanned 22,882,982, 20,776,082,
  and 25,000,000 seeds at a 25,000,000 seed budget.
- After the CUDA optimization pass, generated PTX was checked for
  `div.s64`, `div.u64`, `rem.s64`, and `rem.u64`; none remained in the emitted
  PTX.
- Native Windows exact-case CUDA checks on the game-like DLL path:
  - 100,000 seed Spectral Soul voucher+tag miss: CUDA mean 0.237 ms,
    p50 0.238 ms, Rust CPU mean 23.740 ms.
  - 1,000,000 seed Spectral Soul voucher+tag miss: CUDA p50 1.135 ms,
    Rust CPU p50 37.873 ms. The mean was 1.309 ms because of Windows GPU
    scheduling spikes.
  - 25,000,000 seed Spectral Soul strict-tag hit: CUDA mean 22.944 ms
    at 905M seeds/s, Rust CPU mean 526.862 ms.
  - 25,000,000 seed Spectral Soul voucher+tag miss: CUDA mean 27.918 ms
    at 895M seeds/s, Rust CPU mean 627.848 ms.
- CPU-only native comparison using `BRAINSTORM_SKIP_CUDA_BUILD=1`
- `git diff --check`

The committed legacy Original DLL SHA-256 is
`905990daa83a9d7eef491b1cbb84c480305eb8b6f436115178cfdf40d0ccc09e`.
The native CUDA benchmark found `Q8K1111` for `ux-tag-voucher-pack` after
848,319 scanned seeds in about 1.2 ms, versus about 34 ms for the CPU-only
native run. Wine loaded the host `nvcuda.dll`, but direct probing returned CUDA
driver error code 100 from `cuInit`, so Wine benchmark rows labeled
`rust-cuda` used the Rust CPU fallback in this environment. A Windows process
launched through PowerShell returned `cuInit=0` and saw one CUDA device, so
`mise run bench-cuda-long-windows` is the local game-like Windows CUDA
performance check.

## Release Notes For Maintainers

- Release and deploy consume the top-level `Immolate.dll`.
- Dev prereleases are managed by `.github/workflows/dev-release.yml` at tag
  `dev-release` with release title `dev release`.
- Do not commit `release/` artifacts.
- Note `Immolate.dll` in PRs whenever it changes.
- Keep `BalatroSource/` out of git; use it only as the local source of truth for
  game behavior.
- Update `Immolate/Rust/BENCH.md` whenever benchmark fixtures, output format,
  gates, or latest validated performance results change.
