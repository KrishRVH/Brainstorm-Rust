# Brainstorm Supercharged for Balatro

Current release: **v3.4**.

**Just want to install it?** Open this repository's **Releases** page, select
the release marked **Latest**, download the Brainstorm Supercharged zip, and
follow the installation guide written in that release.

<img width="1829" height="1662" alt="Brainstorm Supercharged settings interface"
src="https://github.com/user-attachments/assets/185f0fab-16f0-431e-9573-97138cac9c28" />

Brainstorm Supercharged is a Balatro mod that rapidly searches for seeds
matching voucher/pack/tag/Joker/Erratic Deck filters and integrates directly
into the game loop through Lua plus a native Rust DLL.

This repository is a full rewrite by KRVH, based on the original Brainstorm by
OceanRamen and the original Immolate by MathIsFun0. It adds the Rust native
search engine, first-shop Joker search, dual-tag filters, Erratic Deck filters,
save/load state slots, searchable Joker UI, resettable preferences, live
auto-reroll scan counts, benchmark automation, release packaging, Steamodded
metadata, and compatibility fixes for the current Balatro mod stack.

## Setup (Required First)

1. Install [`smods-1.0.0-beta` (Steamodded)](https://github.com/Steamodded/smods/wiki/Installing-Steamodded-windows#step-3-installing-steamodded) for Balatro.
2. Install [Lovely](https://github.com/ethangreen-dev/lovely-injector).
3. Build the DLL from source:

   ```bash
   mise trust
   mise run build
   ```

4. Deploy the mod from source:

   ```bash
   mise run deploy
   ```

If auto-detection cannot find your Balatro mods folder, set `TARGET` to the
full `.../Balatro/Mods/Brainstorm` path.
If you do not want to build from source, skip to "Installation (no build)" below.

## Credits

This project is licensed under CC BY-NC-SA 4.0.

- This repository is a full rewrite by KRVH.
- The original Brainstorm was created by OceanRamen:
  https://github.com/OceanRamen/Brainstorm. It is licensed under the Mozilla
  Public License Version 2.0.
- The original Immolate native search engine was created by MathIsFun0:
  https://github.com/SpectralPack/Immolate/tree/26f41efcc313f045bc8bdbf49e5851c56ac40b31.

## Features

- Auto-reroll with dual-tag support (order-agnostic or same-tag-twice).
- First-shop filters: voucher, two pack slots (e.g., Mega Spectral), specific
  Joker in shop slots or Buffoon packs, observatory (Telescope + Mega
  Celestial), Perkeo (The Soul rolls Perkeo).
- Erratic Deck filters for face-card count, no-face searches, and suit-ratio
  searches.
- Joker list is alphabetized, searchable, and excludes first-shop impossible
  targets such as Legendary/Soul-only Jokers, enhancement-gated Jokers, and
  pool-flag-gated Jokers.
- Enable toggle, save/load state (Z/X + 1-5), reroll hotkeys (Ctrl+R,
  Ctrl+A), settings UI (Ctrl+T), and throttled live scan-count text during
  auto-reroll.
- Rust benchmark harness compares current speed against the Original Brainstorm
  DLL where the older ABI supports the same fixture, and reports comparable
  result mismatches.
- Production release automation publishes immutable versioned releases and
  marks the newest one as **Latest**.

## Requirements

- Balatro (Steam, Windows 64-bit).
- Lovely injector (required): https://github.com/ethangreen-dev/lovely-injector
- WSL2 for building/deploying from this repo on Windows.
- mise for development tasks: https://mise.jdx.dev/
- Rust 1.96+ with the Windows GNU target:

  ```bash
  rustup target add x86_64-pc-windows-gnu
  ```

- MinGW-w64 and Wine are required for Windows DLL builds, DLL validation, and
  benchmarks.
- Write access to `%AppData%\Roaming\Balatro\Mods`.

## Build & Deploy (from source)

`mise.toml` is the development interface. Run `mise trust` once per checkout,
then use `mise run <task>`.

`mise run build` builds the Rust native DLL and writes
`target/rust/Immolate.dll`.

`mise run lint` runs Lua formatting, LuaJIT bytecode syntax checks, luacheck,
rustfmt, and clippy. `mise run check-rust` runs Rust formatting, clippy, unit
tests, DLL export/import validation, and hit/composite benchmark smokes.
`mise run check` runs Lua lint plus the Rust validation gate.

Strict user-facing full-suite benchmark report:

```bash
mise run bench-full
```

This uses the same `threads=0` path as Lua auto-reroll and fails if any
comparable Rust/original case drops below parity.

DLL UX-fixture benchmark report using UI-reachable cases and Lua-style
`threads=0`:

```bash
mise run bench-ux
```

For true in-game Lua timing, profile `Brainstorm.auto_reroll()` inside Balatro.
For native Linux-side Rust profiling without the Windows DLL ABI, use
`brainstorm_bench` as described in `Immolate/BENCH.md`.

See `Immolate/BENCH.md` for benchmark workflows.

## Versioning & Release

The source of truth for the mod version is `[manifest].version` in
`lovely.toml`. `steamodded_compat.lua`, the Immolate crate metadata, and the
current-release line at the top of this README carry the same release version
and are checked by `mise run check-version`. Cargo records the corresponding
patch version (for example, release `3.2` uses crate version `3.2.0`).

Use this when bumping versions:

```bash
VERSION=<VERSION> mise run bump-version
```

`mise run release` runs validation, builds `target/rust/Immolate.dll`, stages a
`Brainstorm/` install folder, and creates
`release/Brainstorm_Supercharged_v<VERSION>.zip`.

Commit the synchronized version bump, create the matching `v<VERSION>` tag,
and push both. `.github/workflows/release.yml` validates that the tag and all
version metadata agree, then creates an immutable release titled
`Brainstorm Supercharged v<VERSION>`. Existing releases are never overwritten.

## Documentation

- `AGENTS.md`: contributor and agent-facing project rules.
- `BalatroSource_Guide.md`: verified Balatro source mechanics relevant to
  search parity and future mod work.
- `Immolate/BENCH.md`: benchmark harness, gates, and fixture groups.
- `NOTICE.md`: project, rewrite, Immolate, and third-party attribution notices.

## Installation (no build)

Download the latest release zip from
https://github.com/KrishRVH/Brainstorm-Rust/releases/latest and extract it into
`%AppData%\Roaming\Balatro\Mods\Brainstorm\` (same payload as
`mise run deploy`).
The folder name must be exactly `Brainstorm`.
Reload the game to activate the mod.

Copy the mod files into `%AppData%\Roaming\Balatro\Mods\Brainstorm\` if you
are assembling the payload manually:

```
Brainstorm/
├── Brainstorm.lua
├── UI.lua
├── Immolate.dll           # Native DLL
├── lovely.toml
├── LICENSE
├── NOTICE.md
├── nativefs.lua
├── steamodded_compat.lua
└── VERSION
```

User settings are generated at runtime in Balatro's Love save directory and are
not part of the release payload. Existing legacy `config.lua` files in a mod
folder are migrated into that save-directory config.

## Usage

- Open settings: Ctrl+T. Toggle auto-reroll: Ctrl+A. Manual reroll: Ctrl+R.
- Save/load state: Z/X + 1-5.
- Configure filters: dual tags, voucher, pack (two shop slots), Joker
  (searchable list + location), souls, observatory, Perkeo.
- Impossible first-shop Joker targets are hidden from the Joker selector, and
  impossible native filter combinations return no match immediately.
- Configure Erratic Deck filters when searching for opening hands by face-card
  count, no faces, or suit concentration.
- Use "Enable Brainstorm Supercharged" to disable runtime actions without
  losing settings.
- Use "Reset All" in the Brainstorm Supercharged tab to restore filter and
  Erratic deck settings to defaults.

## Troubleshooting

- Missing DLL or wrong build: rerun `mise run build` and
  `mise run deploy`. If auto-detection fails, set `TARGET` to the full
  `.../Balatro/Mods/Brainstorm` path.
