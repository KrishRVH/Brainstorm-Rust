# Repository Guidelines

Quick reference for contributing to Brainstorm (Balatro mod with a native DLL).

## Credits
- Brainstorm created by OceanRamen. Fork and rewrite by KRVH.
- Immolate created by MathIsFun0.
- Steamodded metadata and shipped notices must credit OceanRamen for the original Brainstorm and KRVH for this rewrite.

## Project Structure & Module Organization
- Lua entry/UI: `Brainstorm.lua`, `UI.lua`; mod metadata/compat in `lovely.toml`, `nativefs.lua`, `steamodded_compat.lua`.
- Native sources: `Immolate/` is the Rust DLL crate. `Immolate/src/` contains the implementation and benchmark harnesses.
- Artifacts: build/lint/format/deploy all use `mise.toml`. The current Rust DLL artifact is kept under `target/rust/Immolate.dll` and staged into release/deploy payloads as `Immolate.dll`.
- Version source of truth: `[manifest].version` in `lovely.toml`. `steamodded_compat.lua` must stay in sync; use `VERSION=x.y mise run bump-version`.
- `BalatroSource/` is the literal game source; never commit it to git and always use it as the source of truth for understanding game behavior.
- `BalatroSource_Guide.md` summarizes seed/search-relevant mechanics verified from `BalatroSource/`.
- Logging is currently disabled in Lua and the Rust `immolate_set_log_path` export is a no-op; keep it off unless explicitly re-enabled.

## Build and Development Commands
- First run in a checkout: `mise trust`.
- Check required tools and install local Lua lint tools: `mise run setup`.
- Dependency check: `mise run doctor`.
- Build: `mise run build` outputs `target/rust/Immolate.dll`.
- Rust validation: `mise run check-rust`.
- Full validation: `mise run check`.
- Benchmarks: `mise run bench-compare` compares Rust against the Original Brainstorm DLL where the older ABI can represent the fixture.
- Full-suite benchmark report: `mise run bench-full`.
- Actual Lua UI UX benchmark report: `mise run bench-ux`.
- Pretty full-suite dashboard: `mise run bench-pretty`.
- Deploy: `mise run deploy` auto-detects the WSL Windows Balatro mods path when possible; otherwise use `TARGET=/path/to/Balatro/Mods/Brainstorm mise run deploy`.
- Release: `mise run release` (runs validation, builds the DLL, stages `Brainstorm/`, and zips `release/Brainstorm_v<VERSION>.zip`).
- Release workflow: `.github/workflows/release.yml` publishes/updates the `latest` production release on `master` pushes and manual dispatch.
- Version bump: `VERSION=3.2 mise run bump-version`.
- Formatting: `mise run format` (runs stylua and rustfmt).
- Lint: `mise run lint` (stylua, LuaJIT bytecode syntax, luacheck, rustfmt, and clippy checks).
- Clean: `mise run clean`.
- No standalone scripts or test runners; use the mise tasks and validate in-game.

## Architecture & FFI Safety
- DLL entry: `immolate.brainstorm_search(seed_start, voucher_key, pack_key, tag1_key, tag2_key, joker_name, joker_location, souls, observatory, perkeo, deck_key, erratic, no_faces, min_face_cards, suit_ratio, num_seeds, threads)`; pass Balatro keys (e.g. `v_telescope`, `tag_charm`, `p_spectral_mega_1`), always `free_result()` on non-empty returns, and wrap FFI in `pcall`.
- Lua loads `Immolate.dll`.
- Runtime config is generated/migrated into Balatro's Love save directory; do not ship or commit generated `config.lua`.
- The Brainstorm settings tab includes an `Enable Brainstorm` toggle. When disabled, config/UI remain available, but runtime actions should not run.
- Pack filter simulates both shop pack slots; voucher check is ante-1 voucher; observatory reuses the same pack/voucher rolls, and Perkeo requires The Soul to roll Perkeo (legendary pool).
- Joker search checks the first shop: location `shop` scans shop slots, `pack` scans Buffoon packs, `any` checks both (pack search respects the selected pack filter).
- Soul checks only apply to Arcana/Spectral packs in the current shop slots.
- Auto-reroll UI shows live scanned seed counts; SPF options go from 1,000 to 1,000,000 seeds per pass.
- Rust search must preserve earliest matching seed semantics for both single-thread and parallel searches. The Original DLL is a historical performance baseline, not the current correctness oracle; comparable result mismatches are reported by default and fail only with `BENCH_FAIL_ON_MISMATCH=1`. Legacy Original DLL results are normalized to the selected benchmark budget.

## Coding Style & Naming Conventions
- Lua: Stylua (`stylua.toml`) — 2-space indent, ~80 cols. Avoid globals, return tables explicitly.
- Rust: rustfmt and clippy; keep unsafe isolated at FFI/harness boundaries.
- Naming: Lua locals/functions lower_snake; constants upper snake (`Brainstorm.VERSION`).

## Commit & Pull Request Guidelines
- Use short, imperative subjects (scope prefix optional: `core:`, `ui:`, `dll:`). Do not commit `release/` artifacts.
- In PRs, state intent and note binary artifacts touched (`target/rust/Immolate.dll` or release payload DLLs). Attach UI screenshots for visual changes.
