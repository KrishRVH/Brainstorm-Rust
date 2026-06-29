# Immolate Benchmarks

This is the operating guide for benchmarking the Rust DLL, comparing Rust CPU
against Rust CUDA, and reporting the legacy Original Brainstorm DLL where the
older ABI can represent the same fixture.

## Benchmark Philosophy

Correctness and speed are separate questions. The Rust CPU implementation is
the correctness oracle for CUDA. `mise run check-rust` validates the Rust
implementation through unit tests, DLL export/import checks, and small benchmark
smoke tests. `mise run bench-compare` measures Rust CPU and Rust CUDA side by
side by default, fails if CUDA returns a different result from Rust CPU, and
also reports performance against the historical Original DLL. It skips legacy
fixtures the older ABI cannot represent, and it normalizes Original DLL hits
that land beyond the selected `BENCH_BUDGET` to `<null>` before comparison.

Use `BENCH_THREADS=0` for user-facing comparisons and UX reports. That is the
Lua auto-reroll call path, so it measures what players actually experience.
Use `BENCH_THREADS=1` only as a single-thread kernel diagnostic; it is not the
user-facing speed claim.

`mise run bench-ux` is a DLL-level UX-fixture report. It uses UI-reachable
filters and Lua-style thread selection, but it does not time in-game Lua work
such as config reads, seed-start generation, FFI argument setup, status text
updates, or Balatro frame scheduling. For true Lua wall time, profile
`Brainstorm.auto_reroll()` in game.

## Canonical Commands

Build the Rust DLL used by Brainstorm:

```bash
mise run build-rust
```

Run the Rust DLL benchmark only:

```bash
BENCH_BUDGET=100000 BENCH_REPEAT=3 BENCH_CASE=all mise run bench
```

Compare Rust CPU, Rust CUDA, and the Original Brainstorm DLL:

```bash
BENCH_BUDGET=100000 BENCH_REPEAT=3 BENCH_CASE=all mise run bench-compare
```

`bench-compare` uses `Immolate/Immolate-OceanRamenandMathIsFun0.dll` by
default. Override it with `ORIGINAL_DLL=/path/to/Immolate.dll` or
`BENCH_ORIGINAL_DLL=/path/to/Immolate.dll`.

Run only the Rust CPU side of the comparison:

```bash
BENCH_CUDA=off BENCH_BUDGET=100000 BENCH_REPEAT=3 mise run bench-compare
```

Run the CUDA-enabled Rust side against Rust CPU parity:

```bash
BENCH_CUDA=on BENCH_BUDGET=100000 BENCH_REPEAT=3 mise run bench-compare
```

Run the same comparison as a native Windows process from WSL. This is the
closest benchmark path to the in-game Windows DLL and Windows CUDA driver:

```bash
BENCH_BUDGET=100000 BENCH_REPEAT=3 BENCH_CASE=all mise run bench-compare-windows
```

Run the long CUDA fixture suite. These cases are UI-reachable Red Deck
tag/voucher/Spectral Soul filters and scan tens of millions of seeds. The
first case is representable by the legacy Original DLL, so it also reports an
Original timing:

```bash
mise run bench-cuda-long-windows
```

Run the full benchmark catalog:

```bash
mise run bench-full
```

Run the DLL UX-fixture report:

```bash
mise run bench-ux
```

Run one profiling group:

```bash
BENCH_CASE=jokers BENCH_BUDGET=100000 BENCH_REPEAT=5 mise run bench-compare
```

Run script-friendly TSV output:

```bash
BENCH_FORMAT=tsv BENCH_COLOR=never \
  BENCH_BUDGET=100000 BENCH_REPEAT=3 BENCH_CASE=all \
  mise run bench-compare
```

## Requirements

Use `mise run setup` for mise-managed tools plus local Lua lint tooling, then
`mise run doctor` to check the system dependencies.

- Rust with the Windows GNU target:
  `rustup target add x86_64-pc-windows-gnu`
- MinGW-w64 for Windows target linking and DLL inspection.
- Wine for running the Windows DLL harness.
- PowerShell from WSL for native-Windows benchmark runs.
- CUDA Toolkit with `nvcc` and a compatible host compiler for the GPU-enabled
  Rust DLL build. Set `BRAINSTORM_SKIP_CUDA_BUILD=1` only for intentional
  CPU-only diagnostic builds.

Wine may print a `wine32 is missing` warning on Linux. That warning is not a
failure for this project as long as the 64-bit harness continues and exits
successfully.

Wine does not exercise the Windows CUDA driver on every WSL setup. Treat Wine
benchmark runs as ABI/parity validation unless `rust-cuda` is clearly faster
than `rust-cpu`. For CUDA performance that reflects the game path, use
`mise run bench-compare-windows` or `mise run bench-cuda-long-windows`; those
launch the Windows harness through PowerShell with Windows paths.

## Benchmark Knobs

The mise tasks read these environment variables:

- `BENCH_CASE=all|cuda-long|baseline|tags|vouchers|packs|jokers|souls|deck|ux|CASE_NAME`
- `BENCH_BUDGET=1000000`
- `BENCH_REPEAT=5`
- `BENCH_WARMUP=1`
- `BENCH_THREADS=0`
- `BENCH_CUDA=both`
- `BENCH_MIN_RATIO=1.0`
- `BENCH_FAIL_ON_MISMATCH=0`
- `BENCH_FORMAT=pretty|tsv`
- `BENCH_COLOR=auto|always|never`
- `ORIGINAL_DLL=Immolate/Immolate-OceanRamenandMathIsFun0.dll`
- `BENCH_ORIGINAL_DLL=/path/to/original.dll`

`BENCH_BUDGET` is the search budget passed to the Rust DLL as `num_seeds`.
`BENCH_REPEAT` controls repeated measurements for each case. Use at least
`BENCH_REPEAT=3` for local comparisons and at least `BENCH_BUDGET=100000` when
looking for meaningful regressions. `BENCH_WARMUP` controls discarded warmup
calls before the measured samples.

`BENCH_CUDA=both` is the default for `bench-compare` and measures `rust-cpu`
with `immolate_set_cuda_enabled(false)` plus `rust-cuda` with
`immolate_set_cuda_enabled(true)`. `mise run bench` defaults to
`BENCH_CUDA=default`, while `mise run check-rust` uses `BENCH_CUDA=both` for
its benchmark smoke. Use `BENCH_CUDA=off`, `BENCH_CUDA=on`, or
`BENCH_CUDA=default` for narrower diagnostics. In `bench-compare`, `on` still
measures `rust-cpu` first so CUDA parity is checked. `rust-cuda` means CUDA was
enabled for that DLL call; unsupported filters or unavailable drivers still use
the Rust CPU fallback inside the DLL.

`BENCH_CASE=cuda-long` selects only the long CUDA fixtures. With
`BENCH_BUDGET=25000000`, the pinned cases currently scan 22,882,982,
20,776,082, and 25,000,000 seeds respectively before returning. These fixtures
are intentionally excluded from `BENCH_CASE=all` and `BENCH_CASE=ux`; use
`cuda-long` or an exact case name when you want the long run.

For CUDA timing on Windows, exact-case runs are often more trustworthy than the
aggregate `cuda-long` report when the harness flags high CUDA variance. Use
`BENCH_CASE=cuda-long-spectral-soul-voucher-tag` with `BENCH_FORMAT=tsv` to
inspect each run and p50/p95 timings for the game-like DLL path.

`BENCH_MIN_RATIO=1.0` makes the harness fail if any comparable Rust/original
or CUDA/original speedup drops below parity. Set `BENCH_MIN_RATIO=0.0` only
when you want a diagnostic report without a speed gate.

`BENCH_FAIL_ON_MISMATCH=0` keeps Rust/original result differences report-only,
which is the default because the Original DLL is a historical performance
baseline and its ABI/semantics do not cover every current Brainstorm behavior.
Set `BENCH_FAIL_ON_MISMATCH=1` only when intentionally auditing a fixture that
should still match the legacy DLL.

## Pretty Report

`BENCH_FORMAT=pretty` is the default. The human report is intentionally plain:
compact tables, optional ANSI color, and no terminal art or sparklines. It
includes:

- per-case Rust CPU, Rust CUDA, and original throughput where available
- skipped original measurements for unsupported older-ABI fixtures
- failing Rust CPU/CUDA result mismatches
- informational Rust/original result mismatches where the historical DLL differs
- scanned percentage, so early-hit fixtures are obvious
- mean latency, p50/p95/p99 latency, min/max latency, and stdev
- `ns/seed`, which is often the clearest hot-path metric
- coefficient of variation (`cv`) to flag noisy measurements
- Rust CUDA/Rust CPU speedup ratio
- Rust/original speedup ratio
- geometric-mean speedups per profiling group
- a potential-regressions section when Rust is slower than the target ratio
- a high-variance section when any measured implementation has CV above 5%

Use `BENCH_COLOR=always` when piping through a terminal renderer that preserves
ANSI color. Use `BENCH_COLOR=never` for plain logs.

## Fixture Groups

The benchmark suite is shared by the DLL harness and the native Rust-only
helper.

- `baseline-hit`: no filters; isolates ABI/result overhead.
- `tag-hit`, `dual-tag`: blind tag checks.
- `voucher-hit`: ante-1 voucher roll.
- `pack-hit`, `observatory`: pack slots plus voucher/pack coupling.
- `shop-hit`, `shop-miss`, `pack-joker`, `any-joker`: joker generation across
  shop and Buffoon pack paths.
- `pack-miss`, `souls-arcana`, `perkeo`: Soul counting and legendary-pool paths.
- `erratic`, `erratic-suit`: Erratic Deck opening-card filters.
- `ux-*`: UI-reachable combinations derived from the Lua controls, including
  no-pack Joker searches, Soul+Perkeo pack searches, and harder Erratic suit
  filters.

No-match/full-budget cases are the most useful for raw throughput. Early-hit
cases are still valuable because they catch overhead, result handling, and
short-circuit behavior. The `scan` column tells you which kind of fixture you
are looking at.

## TSV Output

`BENCH_FORMAT=tsv` keeps automation simple. It prints rows with this shape:

```text
kind    impl|status  case  group  shape  budget  scanned  scan_pct  threads  sample  elapsed_ms  seeds_per_sec  ns_per_seed  min_ms  p50_ms  p95_ms  p99_ms  max_ms  stdev_ms  cv_pct  result
run     ...
summary ...
compare ...
skip    ...
```

For `compare` rows, the `impl` column carries the row status (`ok` or
`below-target`). The relation is stored in the `result` field as
semicolon-delimited details such as `ratio`, `target_ratio`, `lhs`, `rhs`,
`lhs_sps`, `rhs_sps`, `lhs_ms`, `rhs_ms`, `lhs_result`, and `rhs_result`.

## Original Brainstorm Baseline

The Original Brainstorm DLL exposes the older
`brainstorm(seed, voucherName, packName, tagName, souls, observatory, perkeo)`
ABI. It expects localized names such as `Telescope`, `Mega Spectral Pack`, and
`Charm Tag`, so the harness translates the current benchmark keys before
calling it.

Because that ABI has no budget, thread, second-tag, joker, or deck-filter
parameters, the harness skips Original DLL measurements for unsupported
fixtures and for miss fixtures that would otherwise run to the original fixed
100M seed cap. For measured fixtures, Original DLL hits beyond the selected
benchmark budget are treated as `<null>` so the comparison uses the same
effective search window as the Rust DLL.

## Optional Native Rust-Only Benchmark

For quick Linux-side profiling of the Rust core without the Windows DLL ABI,
use the native helper. It runs only the Rust implementation. On Linux/WSL this
is the most direct way to time the CUDA kernel against the Linux CUDA driver;
the Wine DLL harness can still compare Windows ABI behavior against the legacy
Original DLL, but Wine may fall back to Rust CPU if `nvcuda.dll` reports no CUDA
device.

```bash
cargo run --manifest-path Immolate/Rust/Cargo.toml --release --bin brainstorm_bench -- \
  --case all --budget 1000000 --threads 0 --repeat 5
```

For UI-style profiling:

```bash
cargo run --manifest-path Immolate/Rust/Cargo.toml --release --bin brainstorm_bench -- \
  --case ux --budget 100000 --threads 0 --repeat 5 --warmup 2
```

For a CPU-only native comparison, set `BRAINSTORM_SKIP_CUDA_BUILD=1` and rerun
the same command. Rebuild without that variable afterward before producing a
GPU-enabled DLL.

Useful exact UX cases include `ux-pack-joker-no-pack`,
`ux-any-joker-no-pack`, `ux-soul-perkeo-arcana`,
`ux-soul-perkeo-spectral`, `ux-erratic-suit-85`,
`ux-erratic-no-faces-suit`, and `ux-erratic-tag-suit`.

## Agent Workflow

Before changing hot-path code:

1. Run `mise run check-rust`.
2. Run the complete report with `BENCH_CASE=all`,
   `BENCH_BUDGET=1000000`, `BENCH_REPEAT=7`, and `BENCH_WARMUP=2`.
3. Run the UX report with `BENCH_CASE=ux`, `BENCH_BUDGET=100000`, and
   `BENCH_THREADS=0`.
4. Inspect Rust CPU/CUDA result parity before drawing conclusions from the
   CUDA/CPU ratio.
5. Inspect the skipped Original measurements before drawing conclusions from
   the Rust/original ratio.

When adding a benchmark fixture, update `Immolate/Rust/src/bench_cases.rs`.
Both `Immolate/Rust/src/bin/immolate_dll_harness.rs` and
`Immolate/Rust/src/bin/brainstorm_bench.rs` read from that shared catalog.
