# Immolate Benchmarks

This is the operating guide for benchmarking the Rust DLL and comparing it
against the Original Brainstorm DLL where the older ABI can represent the same
fixture.

## Benchmark Philosophy

Correctness and speed are separate questions. `mise run check-rust` validates
the Rust implementation through unit tests, DLL export/import checks, and small
benchmark smoke tests. `mise run bench-compare` reports performance against the
historical DLL and reports comparable result mismatches. It skips fixtures the
older ABI cannot represent, and it normalizes Original DLL hits that land beyond
the selected `BENCH_BUDGET` to `<null>` before comparison.

Keep `BENCH_THREADS=1` for implementation comparisons. Use `BENCH_THREADS=0`
when measuring Lua auto-reroll UX, because the Lua UI passes `threads=0` to the
DLL.

## Canonical Commands

Build the Rust DLL used by Brainstorm:

```bash
mise run build-rust
```

Run the Rust DLL benchmark only:

```bash
BENCH_BUDGET=100000 BENCH_REPEAT=3 BENCH_CASE=all mise run bench
```

Compare Rust against the Original Brainstorm DLL:

```bash
BENCH_BUDGET=100000 BENCH_REPEAT=3 BENCH_CASE=all mise run bench-compare
```

`bench-compare` uses `Immolate/Immolate-OceanRamenandMathIsFun0.dll` by
default. Override it with `ORIGINAL_DLL=/path/to/Immolate.dll` or
`BENCH_ORIGINAL_DLL=/path/to/Immolate.dll`.

Run the full benchmark catalog:

```bash
mise run bench-full
```

Run the actual Lua UI UX report:

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

Wine may print a `wine32 is missing` warning on Linux. That warning is not a
failure for this project as long as the 64-bit harness continues and exits
successfully.

## Benchmark Knobs

The mise tasks read these environment variables:

- `BENCH_CASE=all|baseline|tags|vouchers|packs|jokers|souls|deck|ux|CASE_NAME`
- `BENCH_BUDGET=1000000`
- `BENCH_REPEAT=5`
- `BENCH_WARMUP=1`
- `BENCH_THREADS=1`
- `BENCH_MIN_RATIO=0.0`
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

`BENCH_MIN_RATIO=0.0` disables the speed threshold. Set `BENCH_MIN_RATIO` above
zero when you want the harness to fail if measured Rust/original speedup falls
below that threshold.

`BENCH_FAIL_ON_MISMATCH=0` keeps Rust/original result differences report-only,
which is the default because the Original DLL is a historical performance
baseline and its ABI/semantics do not cover every current Brainstorm behavior.
Set `BENCH_FAIL_ON_MISMATCH=1` only when intentionally auditing a fixture that
should still match the legacy DLL.

## Pretty Dashboard

`BENCH_FORMAT=pretty` is the default. When stdout is an interactive terminal,
the harness shows a live status line for the active DLL call with elapsed time.
The final report excludes rendering time and includes:

- per-case Rust and original throughput where available
- skipped original measurements for unsupported older-ABI fixtures
- informational Rust/original result mismatches where the historical DLL differs
- scanned percentage, so early-hit fixtures are obvious
- mean latency, p50/p95/p99 latency, min/max latency, and stdev
- `ns/seed`, which is often the clearest hot-path metric
- coefficient of variation (`cv`) to flag noisy measurements
- per-run sparklines for Rust and original sample stability
- Rust/original speedup ratio
- geometric-mean speedups per profiling group
- ranked Rust ahead/behind sections with fixture notes
- a high-variance section when either measured implementation has CV above 5%

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
- `ux-*`: UI-reachable combinations derived from the Lua controls.

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
use the native helper. It runs only the Rust implementation.

```bash
cargo run --manifest-path Immolate/Cargo.toml --release --bin brainstorm_bench -- \
  --case all --budget 1000000 --threads 1 --repeat 5
```

For UI-style profiling:

```bash
cargo run --manifest-path Immolate/Cargo.toml --release --bin brainstorm_bench -- \
  --case ux --budget 100000 --threads 0 --repeat 5 --warmup 2
```

## Agent Workflow

Before changing hot-path code:

1. Run `mise run check-rust`.
2. Run the complete dashboard with `BENCH_CASE=all`,
   `BENCH_BUDGET=1000000`, `BENCH_REPEAT=7`, and `BENCH_WARMUP=2`.
3. Run the UX report with `BENCH_CASE=ux`, `BENCH_BUDGET=100000`, and
   `BENCH_THREADS=0`.
4. Inspect the skipped Original measurements before drawing conclusions from
   the Rust/original ratio.

When adding a benchmark fixture, update `Immolate/src/bench_cases.rs`. Both
`Immolate/src/bin/immolate_dll_harness.rs` and
`Immolate/src/bin/brainstorm_bench.rs` read from that shared catalog.
