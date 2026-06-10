# Repository Guidelines

Brainstorm Supercharged is a Balatro mod with Lua UI/hooks and a native Rust
DLL named Immolate. Keep agent work source-faithful, scoped, and validated.

## Non-Negotiables

- Credits must stay intact: full rewrite by KRVH; original Brainstorm by
  OceanRamen; original Immolate by MathIsFun0. Steamodded metadata and shipped
  notices must credit OceanRamen and KRVH.
- `BalatroSource/` is the literal game source. Never commit it. Use it as the
  source of truth for game mechanics.
- `BalatroSource_Guide.md` is the verified map of source mechanics; update it
  only after checking `BalatroSource/`.
- Runtime config is generated in Balatro's Love save directory. Do not ship or
  commit generated `config.lua`.
- Logging is intentionally off. `immolate_set_log_path` remains a Rust no-op
  ABI export unless explicitly re-enabled.
- Do not commit `release/` payloads or generated zips.

## Project Map

- Lua entry/UI: `Brainstorm.lua`, `UI.lua`.
- Mod metadata/compat: `lovely.toml`, `steamodded_compat.lua`.
- Rust crate: `Immolate/`; implementation in `Immolate/src/`.
- Benchmark catalog: `Immolate/src/bench_cases.rs`.
- CUDA bridge/kernel: `Immolate/src/engine/cuda.rs` and
  `Immolate/src/cuda/brainstorm_cuda.cu`; Rust CPU remains the correctness
  oracle and fallback.
- Rust DLL artifact: `target/rust/Immolate.dll`, staged as `Immolate.dll`.
- Version source of truth: `[manifest].version` in `lovely.toml`; keep
  `steamodded_compat.lua`, `Immolate/Cargo.toml`, `Immolate/Cargo.lock`, and
  README's current-release line in sync with
  `VERSION=x.y mise run bump-version`.
- Docs: `README.md`, `AGENTS.md`, `BalatroSource_Guide.md`,
  `Immolate/BENCH.md`, `NOTICE.md`.

## Commands

- First checkout: `mise trust`.
- Tooling/deps: `mise run setup`, then `mise run doctor`.
- Build DLL: `mise run build`.
- Rust validation: `mise run check-rust`.
- Full validation: `mise run check`.
- Format: `mise run format`.
- Lint only: `mise run lint`.
- Clean: `mise run clean`.
- Deploy: `mise run deploy`, or
  `TARGET=/path/to/Balatro/Mods/Brainstorm mise run deploy`.
- Release: `mise run release`.
- Version bump: `VERSION=<VERSION> mise run bump-version`.
- Bench current DLL: `BENCH_CASE=ux BENCH_BUDGET=100000 mise run bench`.
- Strict current regression check:
  `BENCH_BASELINE_DLL=/path/to/frozen/Immolate.dll mise run bench-current-compare`.
  It runs natively on Windows by default; `BENCH_EXECUTOR=wine` is diagnostic.
- Compare to Original DLL: `mise run bench-compare`.
- Full reports: `mise run bench-full` for TSV automation and
  `mise run bench-pretty` for a compact human-readable historical report. Both
  use `threads=0`; only the one-candidate baseline is strict-comparable because
  current and Original DLLs traverse seeds in different orders.
- UX-fixture report: `mise run bench-ux` measures DLL calls using
  UI-reachable cases and `threads=0`; it is not an in-game Lua profiler.
- Native-Windows CUDA parity/performance report from WSL:
  `mise run bench-cuda-long-windows`.
- Intentional CPU-only experimental build:
  `BRAINSTORM_SKIP_CUDA_BUILD=1 mise run check`.
- Native core benchmark:
  `cargo run --manifest-path Immolate/Cargo.toml --release --bin brainstorm_bench -- --case ux --budget 100000 --threads 0 --repeat 5 --warmup 2`.

## Current Search Semantics

- FFI entry:
  `brainstorm_search(seed_start, voucher_key, pack_key, tag1_key, tag2_key, joker_name, joker_location, souls, observatory, perkeo, deck_key, erratic, no_faces, min_face_cards, suit_ratio, num_seeds, threads)`.
- Pass Balatro keys such as `v_telescope`, `tag_charm`,
  `p_spectral_mega_1`; always `free_result()` non-empty FFI results and wrap
  Lua FFI calls in `pcall`.
- First-shop model: first booster slot is forced normal Buffoon; second booster
  slot is rolled from the shop pack pool. Pack filters check these two slots.
- Voucher filter checks the ante-1 voucher and respects deck-start vouchers and
  voucher upgrade locks.
- Observatory means ante-1 Telescope plus a Mega Celestial pack in the first
  shop. It reuses the same voucher/pack rolls; it is not the voucher's scoring
  effect.
- Perkeo search means a soulable pack produces The Soul and the legendary roll
  yields Perkeo. It does not simulate Perkeo's later copy effect.
- Soul filters apply only to Arcana/Spectral packs in the first shop. Because
  only one of the two first-shop packs can be soulable and The Soul locks after
  generation, `souls > 1` is impossible and rejected statically.
- Joker search checks the first shop: `shop` scans shop Joker slots, `pack`
  scans Buffoon packs, and `any` checks both. Pack Joker search respects the
  selected pack filter.
- Direct Joker targets exclude first-shop impossibilities: Legendary/Soul-only
  Jokers, enhancement-gated Jokers, pool-flag-gated Jokers, and the native
  first-shop blocked pool targets such as Cavendish, Steel Joker, Stone Joker,
  Lucky Cat, Golden Ticket, and Glass Joker.
- Erratic Deck filters simulate 52 fixed source-order draws. `no_faces` discards
  face samples after sampling; they are not replaced.
- Rust search must preserve earliest matching seed semantics for single-thread
  and parallel searches.
- CUDA is enabled only by the `AR: Use CUDA` setting through
  `immolate_set_cuda_enabled`. Unsupported filters or an unavailable driver
  must fall back to Rust CPU without changing result or scanned-count
  semantics. Initialization and runtime failures latch that process onto CPU;
  toggling the setting does not retry a failed device.
- `immolate_last_search_used_cuda` is thread-local and must be queried
  immediately after a search. It is true only when CUDA handles the remaining
  search window; CPU prefix hits and every fallback path leave it false.

## Testing Expectations

- Immolate has source-oracle tests that compare optimized Rust predicates and
  searches against the source-faithful `Instance` model for target seeds and
  edge windows. Keep these tests broad when changing RNG, filters, locks, pack
  generation, Joker pools, Soul/Perkeo, Observatory, or Erratic logic.
- Add/update benchmark fixtures in `Immolate/src/bench_cases.rs` when a user
  workflow or hot path changes.
- The Original Brainstorm DLL is a historical performance baseline, not the
  correctness oracle. `BENCH_FAIL_ON_MISMATCH=1` is only for intentional legacy
  parity audits.
- For Lua behavior, validate with `mise run lint-lua`; for full confidence run
  `mise run check`.
- CUDA changes require CPU/GPU result and scanned-count parity, unsupported-path
  fallback coverage, the CPU-only build, and native-Windows long-window tests.

## Style

- Lua: Stylua, 2-space indent, minimal comments, no accidental globals, return
  tables explicitly where modules do so.
- Rust: rustfmt + clippy; keep unsafe isolated at FFI/harness boundaries.
- Prefer local patterns over new abstractions. Add helpers only when they remove
  real duplication or clarify source-parity rules.
- Preserve user changes in a dirty worktree. Never reset/revert unrelated work.

## Release Invariants

- Releases are immutable and version-tag driven. Never recreate a movable
  `latest` tag, force-move a release tag, edit an existing release in place, or
  upload release assets with `--clobber`. GitHub's **Latest** marker is release
  metadata, not a Git tag.
- Keep GitHub release immutability enabled. Create each release as a draft,
  attach every asset, then publish it so GitHub locks the assets and tag.
- Ordinary `master` pushes do not publish releases. To release, run
  `VERSION=x.y mise run bump-version`, commit every resulting metadata change,
  then create and push the exact matching `vX.Y` tag. The tag must point to the
  commit containing that version bump.
- `mise run bump-version` must update every maintained version surface:
  `lovely.toml`, `steamodded_compat.lua`, `Immolate/Cargo.toml`,
  `Immolate/Cargo.lock`, and README's current-release line. Mod version `x.y`
  maps to Cargo version `x.y.0`; `mise run check-version` enforces the mapping.
- `.github/workflows/release.yml` must reject a tag/version mismatch and an
  already-published tag. If a published release has a defect, fix it under a
  new version and tag; do not mutate the old release.
- Generate `SHA256SUMS.txt` from inside the artifact directory so it records
  asset basenames, not CI staging paths. Verify the downloaded assets with
  `sha256sum --check SHA256SUMS.txt` before considering a release complete.
- Release titles, zip names, payload `VERSION`, and GitHub tags are derived from
  the checked canonical version. Never hard-code a separate release version in
  the workflow.
- Keep attribution concise and unambiguous everywhere it ships:
  `Full rewrite by KRVH. Originals: Brainstorm by OceanRamen; Immolate by
  MathIsFun0.` Do not imply that the original authors created this rewrite.
  Steamodded `MOD_AUTHOR` must continue to credit OceanRamen and KRVH.
- Before tagging, run `mise run check` and `mise run release`, inspect the
  packaged metadata, and validate the workflow syntax. After publishing, verify
  the GitHub Actions run, **Latest** designation, versioned assets, downloaded
  checksum, and packaged `VERSION` from the remote release.
- Do not publish a CUDA build until the release workflow provisions pinned
  `nvcc` and host-compiler versions and intentionally packages its supported GPU
  architectures. The experimental single-architecture module is branch-only.
- Do not commit release payloads, generated zips, or staged DLLs. PRs should
  state validation run and whether binary artifacts changed. Attach UI
  screenshots for visual changes.
