# Repository Guidelines

Brainstorm is a Balatro mod with Lua UI/hooks and a native Rust DLL named
Immolate. Keep agent work source-faithful, scoped, and validated.

## Non-Negotiables
- Credits must stay intact: Brainstorm by OceanRamen, fork/rewrite by KRVH,
  Immolate by MathIsFun0. Steamodded metadata and shipped notices must credit
  OceanRamen and KRVH.
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
- Mod metadata/compat: `lovely.toml`, `steamodded_compat.lua`, `nativefs.lua`.
- Rust crate: `Immolate/`; implementation in `Immolate/src/`.
- Benchmark catalog: `Immolate/src/bench_cases.rs`.
- Rust DLL artifact: `target/rust/Immolate.dll`, staged as `Immolate.dll`.
- Version source of truth: `[manifest].version` in `lovely.toml`; keep
  `steamodded_compat.lua` in sync with `VERSION=x.y mise run bump-version`.
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
- Version bump: `VERSION=3.2 mise run bump-version`.
- Bench current DLL: `BENCH_CASE=ux BENCH_BUDGET=100000 mise run bench`.
- Compare to Original DLL: `mise run bench-compare`.
- Full reports: `mise run bench-full` for TSV automation and
  `mise run bench-pretty` for a compact human-readable report. Both use
  `threads=0` and fail if any comparable Rust/original case drops below parity.
- UX-fixture report: `mise run bench-ux` measures DLL calls using
  UI-reachable cases and `threads=0`; it is not an in-game Lua profiler.
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

## Style
- Lua: Stylua, 2-space indent, minimal comments, no accidental globals, return
  tables explicitly where modules do so.
- Rust: rustfmt + clippy; keep unsafe isolated at FFI/harness boundaries.
- Prefer local patterns over new abstractions. Add helpers only when they remove
  real duplication or clarify source-parity rules.
- Preserve user changes in a dirty worktree. Never reset/revert unrelated work.

## Release Notes
- `.github/workflows/release.yml` updates the production `latest` release on
  `master` pushes and manual dispatch.
- PRs should state intent, validation run, and whether binary artifacts changed
  (`target/rust/Immolate.dll` or staged release DLLs). Attach UI screenshots for
  visual changes.
