# Immolate Benchmarks

This is the operating guide for benchmarking the Rust DLL and comparing it
against the Original Brainstorm DLL where the older ABI can represent the same
fixture.

## Benchmark Philosophy

Correctness and speed are separate questions. `mise run check-rust` validates
the Rust implementation through unit tests, DLL export/import checks, and small
benchmark smoke tests. `mise run bench-compare` reports performance against the
historical DLL and reports comparable result mismatches. It skips fixtures the
older ABI cannot represent. For measured legacy hits, it preserves the raw
result and computes scanned work using the Original DLL's length-major
lexicographic seed order, which differs from the current Rust search order.
The Rust CPU result and scanned count are the correctness oracle for every CUDA
measurement; any CPU/GPU mismatch is a hard failure.

Use `BENCH_THREADS=0` for user-facing comparisons and UX reports. That is the
Lua auto-reroll call path, so it measures what players actually experience.
Use `BENCH_THREADS=1` only as a single-thread kernel diagnostic; it is not the
user-facing speed claim.

`mise run bench-ux` is a DLL-level UX-fixture report. It uses UI-reachable
filters and Lua-style thread selection, but it does not time in-game Lua work
such as config reads, seed-start generation, FFI argument setup, status text
updates, or Balatro frame scheduling. For true Lua wall time, profile
`Brainstorm.auto_reroll()` in game.

DLL harness samples start after benchmark-argument conversion and, for the
Original DLL, stdout redirection. They include the DLL call, result copy, and
`free_result`, then stop before stdout restoration. This keeps harness-only
setup out of both sides while retaining the ABI/result work paid by callers.

## Canonical Commands

Build the Rust DLL used by Brainstorm Supercharged:

```bash
mise run build-rust
```

Run the Rust DLL benchmark only:

```bash
BENCH_BUDGET=100000 BENCH_REPEAT=3 BENCH_CASE=all mise run bench
```

Compare a frozen current-ABI DLL against the current build:

```bash
BENCH_BASELINE_DLL=/path/to/frozen/Immolate.dll \
  BENCH_CASE=ux BENCH_BUDGET=100000 BENCH_THREADS=0 \
  mise run bench-current-compare
```

This is the strict native-Windows performance-regression command. It freezes
input DLLs before building, stages all three artifacts on the Windows-local
temporary filesystem with equal DLL basenames and path lengths, records
settings and pre-run hashes, runs alternating `A/B/B/A` and `B/A/A/B` cycles,
and rejects any mid-run artifact change. Every result and scanned count must
match. It retains the per-cycle ratios and deltas, and hard-gates p50, p95, and
mean latency on stable majority-cycle losses or concordant paired-cycle median
and pooled losses over both the configured ratio and absolute noise floor. P99
is always reported, but becomes a hard gate only with at least 1,000 samples per
arm per cycle.

The defaults are four cycles and 31 repeats. Treat any tail `watch` row as a
prompt for a targeted confirmation with at least
`BENCH_CURRENT_CYCLES=8 BENCH_REPEAT=501 BENCH_WARMUP=10`. Each arm runs twice
per cycle, so 501 repeats provide 1,002 samples and enable the p99 hard gate.
Do not relax the threshold after seeing the data. Keep other CPU work stopped.
Set `BENCH_CANDIDATE_DLL` to compare two already-built artifacts. Set
`BENCH_EXECUTOR=wine` only for a portability diagnostic; Wine timings are not
release evidence because its scheduler behavior can differ from native
Windows.

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

Run the DLL UX-fixture report:

```bash
mise run bench-ux
```

Run long CUDA windows through the native Windows DLL and driver from WSL:

```bash
mise run bench-cuda-long-windows
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

`mise run setup` installs the Windows Rust target and local Lua lint tooling,
then runs `mise run doctor` to check the system dependencies.

- Rust with the Windows GNU target:
  `rustup target add x86_64-pc-windows-gnu`
- MinGW-w64 for Windows target linking and DLL inspection.
- Wine for running the Windows DLL harness.
- `sha256sum` for benchmark artifact integrity checks.
- WSL interoperability (`wslpath`, `cmd.exe`, and `powershell.exe`) for
  native-Windows timing.
- CUDA Toolkit with `nvcc` for the GPU-enabled build. Set
  `BRAINSTORM_SKIP_CUDA_BUILD=1` only for an intentional CPU-fallback build.
  The defaults are `BRAINSTORM_CUDA_ARCH=sm_89` and `CUDAHOSTCXX=gcc-12`;
  `NVCC` can select a non-default compiler binary. Each build embeds one
  precompiled GPU architecture, so set `BRAINSTORM_CUDA_ARCH` to the test GPU's
  `sm_*` architecture. An incompatible module falls back to Rust CPU and
  therefore fails strict `cuda-long` backend attestation. Initialization and
  runtime failures remain latched until the harness process restarts.

Wine may print a `wine32 is missing` warning on Linux. That warning is not a
failure for this project as long as the 64-bit harness continues and exits
successfully.

## Benchmark Knobs

The mise tasks read these environment variables:

- `BENCH_CASE=all|cuda-long|baseline|tags|vouchers|packs|jokers|souls|deck|ux|CASE_NAME`
- `BENCH_BUDGET=1000000`
- `BENCH_REPEAT=5`
- `BENCH_WARMUP=1`
- `BENCH_THREADS=0`
- `BENCH_CUDA=default|off|both`
- `BENCH_MIN_RATIO=1.0`
- `BENCH_FAIL_ON_MISMATCH=0`
- `BENCH_FORMAT=pretty|tsv`
- `BENCH_COLOR=auto|always|never`
- `ORIGINAL_DLL=Immolate/Immolate-OceanRamenandMathIsFun0.dll`
- `BENCH_ORIGINAL_DLL=/path/to/original.dll`
- `BENCH_BASELINE_DLL=/path/to/frozen/current-ABI.dll`
- `BENCH_CANDIDATE_DLL=/path/to/candidate/current-ABI.dll`
- `BENCH_EXECUTOR=native|wine`
- `BENCH_NATIVE_STAGE_DIR=/mnt/c/path/to/local/temp`
- `BENCH_CURRENT_CYCLES=4`
- `BENCH_CURRENT_MIN_RATIO=0.99`
- `BENCH_CURRENT_MIN_REGRESSION_MS=0.005`

`BENCH_BUDGET` is the search budget passed to the Rust DLL as `num_seeds`.
`BENCH_REPEAT` controls repeated measurements for each case. Use at least
`BENCH_REPEAT=3` for local comparisons and at least `BENCH_BUDGET=100000` when
looking for meaningful regressions. `BENCH_WARMUP` controls discarded warmup
calls before the measured samples. The current/current comparator invokes each
arm twice per cycle: its default 31 repeats produce 62 samples, making the
reported p99 that cycle's maximum rather than a stable percentile. P99 remains
report-only below 1,000 samples per arm per cycle and hard-gates without any
threshold change at `BENCH_REPEAT>=500`.

`BENCH_MIN_RATIO=1.0` makes the harness fail if any comparable user-facing
Rust/original speedup drops below parity. Only the explicit `baseline-hit`
fixture is strict-comparable: both implementations return the same non-empty
starting seed after exactly one candidate. Other measured ratios are marked
informational and never enter the threshold because the two DLLs traverse seeds
in different orders. Set `BENCH_MIN_RATIO=0.0` only when you want a diagnostic
report without a speed gate.
`BENCH_MIN_RATIO` must be finite and non-negative.

`BENCH_CUDA=both` alternates paired CPU/CUDA-enabled samples, uses the p50
latency ratio, and requires exact result and scanned-count parity. Every call's
reported backend is queried outside the timed region: CPU arms must report CPU,
`rust-cuda` means the GPU actually ran, and `rust-cuda-not-used` means an
enabled regular fixture completed without GPU work, including serial-prefix
hits and CPU fallback. The `cuda-long` fixtures fail
if any CUDA-enabled probe, warmup, or measured call falls back. `default`
preserves the DLL's current setting for historical current-ABI measurements;
`off` explicitly attests the CPU path. Wine is useful for ABI and fallback
checks, but native Windows is the performance authority for the Windows CUDA
driver. The harness reads this status through the
`immolate_last_search_used_cuda` export; it is not inferred from timing.
The harness reports phase-zero calls that actually use the GPU as CUDA probe
rows. In a fresh harness process, the first such row includes lazy driver,
context, and precompiled-module startup; later probes are warm. The paired
summary describes steady-state search latency.
CUDA speed ratios are informational; CPU/GPU result, scanned-count, and backend
attestation are the hard gates.

`BENCH_FAIL_ON_MISMATCH=0` keeps Rust/original result differences report-only,
which is the default because the Original DLL is a historical performance
baseline and its ABI/semantics do not cover every current Brainstorm
Supercharged behavior. Set `BENCH_FAIL_ON_MISMATCH=1` only when intentionally
auditing a fixture that should still match the legacy DLL. An empty legacy
result can mean either a hit on the empty seed or a fixed-cap miss, so only a
non-empty current seed proves a mismatch against it.

## Pretty Report

`BENCH_FORMAT=pretty` is the default. The human report is intentionally plain:
compact tables, optional ANSI color, and no terminal art or sparklines. It
includes:

- per-case Rust and original throughput where available
- skipped original measurements for unsupported older-ABI fixtures
- informational Rust/original result mismatches where the historical DLL differs
- scanned percentage, so early-hit fixtures are obvious
- mean and p95 latency (full distribution metrics remain available in TSV)
- counterbalanced CPU/CUDA-enabled p50 ratios, actual backend, and CV for both
  arms
- `ns/seed`, which is often the clearest hot-path metric
- coefficient of variation (`cv`) to flag noisy measurements
- Rust/original speedup ratio
- a potential-regressions section when Rust is slower than the target ratio
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
- `ux-*`: UI-reachable combinations derived from the Lua controls, including
  duplicate tags, forced Buffoon packs, no-pack Soul/Joker searches, special
  deck shop rates, Soul+Perkeo searches, and harder Erratic combinations.
- `cuda-long-*`: supported full-window GPU fixtures intended for native-Windows
  CPU/CUDA parity and throughput measurements.

No-match/full-budget cases are the most useful for raw throughput. Early-hit
cases are still valuable because they catch overhead, result handling, and
short-circuit behavior. The `scan` column tells you which kind of fixture you
are looking at.

Static cases are filter combinations rejected before seed evaluation. They
report zero scanned seeds and zero throughput/cost, and are skipped for the
Original DLL because its ABI cannot perform the same zero-work rejection.

## TSV Output

`BENCH_FORMAT=tsv` keeps automation simple. It prints rows with this shape:

```text
kind    impl|status  case  group  shape  budget  scanned  scan_pct  threads  sample  elapsed_ms  seeds_per_sec  ns_per_seed  min_ms  p50_ms  p95_ms  p99_ms  max_ms  stdev_ms  cv_pct  result
run     ...
summary ...
compare ...
skip    ...
```

For `compare` rows, the `impl` column carries the row status (`ok`,
`below-target`, or `informational`). The relation is stored in the `result`
field as semicolon-delimited details such as `ratio`, `target_ratio`, `strict`,
`ratio_basis`, `lhs`, `rhs`, `lhs_sps`, `rhs_sps`, `lhs_mean_ms`,
`rhs_mean_ms`, `lhs_p50_ms`, `rhs_p50_ms`, `lhs_result`, and `rhs_result`.

## Original Brainstorm Baseline

The Original Brainstorm DLL exposes the older
`brainstorm(seed, voucherName, packName, tagName, souls, observatory, perkeo)`
ABI. It expects localized names such as `Telescope`, `Mega Spectral Pack`, and
`Charm Tag`, so the harness translates the current benchmark keys before
calling it.

Because that ABI has no budget, thread, second-tag, joker, or deck-filter
parameters, the harness skips Original DLL measurements for unsupported
fixtures and for miss fixtures that would otherwise run to the original fixed
100M seed cap. It also skips an empty legacy result because the old ABI uses the
same empty string for a successful empty seed and for no hit, so the work is
ambiguous. Each measured current/legacy case gets one untimed probe before its
configured warmups, and every measured repeat must return exactly the probe's
raw seed and scan count.

Most historical timings are intentionally informational: the implementations
can use different seed orders and model different mechanics even when the old
ABI accepts the same inputs. Strict parity applies only to the proven non-empty,
one-candidate `baseline-hit` fixture. Use the frozen current-build versus
candidate benchmark for performance regression decisions across current
semantics.

## Performance Policies

These are measured policies, not general abstractions. Keep the independent
`Instance` source oracle unchanged when tuning them.

| Surface | Retained policy | Reopen only with |
| --- | --- | --- |
| Current regression gate | Compare two current-ABI DLLs natively on Windows with `bench-current-compare`; the default hard-gates p50/p95/mean and reports p99. Historical and Wine ratios are context only. | Exact result/scanned equality plus frozen, counterbalanced native-Windows data; confirm p99 with at least 1,000 samples per arm per cycle. |
| Scheduler shapes | Expensive mixed/Joker/Soul/Perkeo/Erratic work may use 16 auto threads; nearby voucher/second-pack hits stay capped at 8. Prefixes and chunks remain shape-local in `CompiledFilter`. | Windows data across early hits, full misses, `threads=0`, and `threads=1`; no stable tail loss. |
| Erratic draws | `ErraticDraws` owns one initialized RNG cursor and derives only face/suit properties from the exact mantissa. | Boundary and million-sample parity against the float/source path. |
| Seed progression | Fuse sequential seed increment and hash update, but keep arbitrary-ID construction normalized and independently tested. | Carry, growth, wrap, cache-validity, and earliest-result proofs. |
| Predicate order | Order independent keyed checks by measured rejection cost; preserve source generation order when state or locks are shared. | Source-oracle windows and held-out controls, not one favorable fixture. |
| Lua active loop | One synchronous batch per active `Game.update`; status text uses the native ref-backed text node. | In-game frame/cancellation evidence and the tracked lifecycle smoke. |
| CUDA dispatch | Try CUDA only when the user enables it and the compiled filter is supported; otherwise use the Rust CPU path. | Exact CPU/GPU result and scanned-count parity across long, wraparound, and earliest-hit windows. |
| Settings pips | Suppress only the unreadable 140-choice Joker and 36-choice face-count pip rows. | A supported native UI alternative or visual regression evidence. |

Avoid repeating already falsified families without new evidence: blanket
inlining, generic integer replacements for Lua float rounding, shared helpers
that outline hot RNG initialization, smaller universal scheduler blocks,
cross-target PGO profiles, or process-lifetime worker pools inside an unloadable
DLL. Each either regressed held-out paths, failed exact numeric parity, or broke
the unload contract.

## Optional Native Rust-Only Benchmark

For quick Linux-side profiling of the Rust core without the Windows DLL ABI,
use the native helper. It runs only the Rust implementation.

```bash
cargo run --manifest-path Immolate/Cargo.toml --release --bin brainstorm_bench -- \
  --case all --budget 1000000 --threads 0 --repeat 5
```

For UI-style profiling:

```bash
cargo run --manifest-path Immolate/Cargo.toml --release --bin brainstorm_bench -- \
  --case ux --budget 100000 --threads 0 --repeat 5 --warmup 2
```

Useful exact UX cases include `ux-pack-joker-no-pack`,
`ux-any-joker-no-pack`, `ux-soul-perkeo-arcana`,
`ux-soul-perkeo-spectral`, `ux-erratic-suit-85`,
`ux-erratic-no-faces-suit`, and `ux-erratic-tag-suit`.

## Agent Workflow

Before changing hot-path code:

1. Run `mise run check-rust`.
2. Freeze the current DLL under a unique name before editing.
3. Run native `bench-current-compare` for the complete and UX catalogs at
   realistic budgets with `BENCH_THREADS=0`; investigate every regression or
   tail `watch`. Confirm a p99 signal with
   `BENCH_REPEAT=501 BENCH_CURRENT_CYCLES=8 BENCH_WARMUP=10`; use
   `BENCH_THREADS=1` only as a diagnostic.
4. Inspect the historical report only for context; all non-baseline legacy
   ratios are informational because the seed orders differ.

When adding a benchmark fixture, update `Immolate/src/bench_cases.rs`. Both
`Immolate/src/bin/immolate_dll_harness.rs` and
`Immolate/src/bin/brainstorm_bench.rs` read from that shared catalog.
