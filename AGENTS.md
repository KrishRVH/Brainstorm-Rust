# Repository Guidelines

Quick reference for contributing to Brainstorm (Balatro mod with a native DLL).

## Credits
- Brainstorm created by OceanRamen. Rewrite by KRVH.
- Immolate created by MathIsFun0.

## Project Structure & Module Organization
- Lua entry/UI: `Brainstorm.lua`, `UI.lua`; config/compat in `config.lua`, `lovely.toml`, `nativefs.lua`, `steamodded_compat.lua`.
- Native sources: `Immolate/Rust/` is the Rust DLL implementation and Rust CPU
  is the correctness oracle for CUDA.
- Artifacts: DLL is `Immolate.dll` (default). Build/lint/format/deploy all use
  `mise.toml`; the current Rust artifact is kept under `target/rust/`.
- Legacy benchmark artifact: `Immolate/Immolate-OceanRamenandMathIsFun0.dll`
  is the committed Original Brainstorm DLL from MathIsFun0/OceanRamen and is
  used only as a historical benchmark baseline.
- `BalatroSource/` is the literal game source; never commit it to git and always use it as the source of truth for understanding game behavior.
- `BalatroSource_Guide.md` summarizes seed/search-relevant mechanics verified from `BalatroSource/`.
- Logging is currently disabled in Lua and the Rust `immolate_set_log_path`
  export is a no-op; keep it off unless explicitly re-enabled.

## Build and Development Commands
- First run in a checkout: `mise trust`.
- Install mise-managed tools and local Lua lint tools: `mise run setup`.
- Dependency check: `mise run doctor`.
- Build: `mise run build` outputs the Rust `Immolate.dll`.
- Rust validation: `mise run check-rust`.
- Legacy/Rust benchmark comparison: `mise run compare` or `mise run bench-compare`.
- Benchmarks: `mise run bench-compare`.
- Strict full-suite benchmark gate: `mise run bench-full`.
- DLL UX benchmark gate: `mise run bench-ux`.
- Native Windows CUDA long-scan benchmark from WSL: `mise run bench-cuda-long-windows`.
- Pretty full-suite dashboard: `mise run bench-pretty`.
- Deploy: `TARGET=/mnt/c/Users/Krish/AppData/Roaming/Balatro/Mods/Brainstorm mise run deploy`.
- Release: `mise run release` (runs `mise run check`, builds the DLL, and zips `release/Brainstorm_v3.1.zip`).
- Dev release workflow: `.github/workflows/dev-release.yml` publishes/updates the `dev-release` prerelease on `master` pushes and manual dispatch.
- Formatting: `mise run format` (runs stylua and rustfmt when available).
- Lint: `mise run lint` (stylua, LuaJIT bytecode syntax, luacheck, rustfmt,
  and clippy checks).
- Clean: `mise run clean`.
- No standalone scripts or test runners; use the mise tasks and validate in-game.

## Architecture & FFI Safety
- DLL entry: `immolate.brainstorm_search(seed_start, voucher_key, pack_key, tag1_key, tag2_key, joker_name, joker_location, souls, observatory, perkeo, deck_key, erratic, no_faces, min_face_cards, suit_ratio, num_seeds, threads)`; pass Balatro keys (e.g. `v_telescope`, `tag_charm`, `p_spectral_mega_1`), always `free_result()` on non-empty returns, and wrap FFI in `pcall`.
- Lua loads `Immolate.dll`.
- CUDA is controlled only by the `AP: USE CUDA` setting in the Lua UI, which
  calls `immolate_set_cuda_enabled`; do not add hidden runtime heuristics for
  whether to try the GPU path. Unsupported filters and unavailable CUDA drivers
  fall back to the Rust CPU engine.
- Pack filter simulates both shop pack slots; voucher check is ante-1 voucher; observatory reuses the same pack/voucher rolls, and Perkeo requires The Soul to roll Perkeo (legendary pool).
- Joker search checks the first shop: location `shop` scans shop slots, `pack` scans Buffoon packs, `any` checks both (pack search respects the selected pack filter).
- Soul checks only apply to Arcana/Spectral packs in the current shop slots.
- Auto-reroll UI shows live scanned seed counts; SPF options go from 1,000 to 1,000,000 seeds per pass.
- Rust search must preserve earliest matching seed semantics for both single-thread
  and parallel searches. CUDA benchmark result mismatches against Rust CPU fail
  the harness, even when the timing is faster.

## Coding Style & Naming Conventions
- Lua: Stylua (`stylua.toml`) — 2-space indent, ~80 cols. Avoid globals, return tables explicitly.
- Rust: rustfmt and clippy; keep unsafe isolated at FFI/harness boundaries.
- Naming: Lua locals/functions lower_snake; constants upper snake
  (`Brainstorm.VERSION`).

## Commit & Pull Request Guidelines
- Use short, imperative subjects (scope prefix optional: `core:`, `ui:`, `dll:`). Do not commit `release/` artifacts.
- In PRs, state intent and note binary artifacts touched (`Immolate.dll`). Attach UI screenshots for visual changes.
