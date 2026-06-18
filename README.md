# Brainstorm for Balatro

**Just want to install it?** Open this repository's **Releases** page, click
the `latest` release, download the Brainstorm zip, and follow the installation
guide written in that release.

<img width="1829" height="1662" alt="image" src="https://github.com/user-attachments/assets/185f0fab-16f0-431e-9573-97138cac9c28" />

Brainstorm is a Balatro mod that rapidly searches for seeds matching
voucher/pack/tag/Joker/Erratic Deck filters and integrates directly into the
game loop through Lua plus a native Rust DLL.

KRVH's rewrite is a substantial expansion of OceanRamen's original Brainstorm
mod: it adds the Rust native search engine, first-shop Joker search, dual-tag
filters, Erratic Deck filters, save/load state slots, searchable Joker UI,
resettable preferences, live auto-reroll scan counts, benchmark automation,
release packaging, Steamodded metadata, and compatibility fixes for the current
Balatro mod stack.

## Setup (Required First)
1. Install `smods-1.0.0-beta` (Steamodded) for Balatro.
2. Install Lovely.
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

- Brainstorm was created by OceanRamen. KRVH rewrote and maintains this fork,
  which derives from
  https://github.com/OceanRamen/Brainstorm, which is licensed under the Mozilla
  Public License Version 2.0.
- The Immolate native DLL source was created in C++ by MathIsFun0. KRVH rewrote
  that native code in Rust, ported unfinished functionality, and added the Joker
  search workflow. This project uses CC BY-NC-SA 4.0 to remain compatible with
  the original Immolate source:
  https://github.com/SpectralPack/Immolate/tree/26f41efcc313f045bc8bdbf49e5851c56ac40b31.
- Steamodded metadata credits OceanRamen for the original Brainstorm and KRVH
  for this rewrite.

## Features
- Auto-reroll with dual-tag support (order-agnostic or same-tag-twice).
- First-shop filters: voucher, two pack slots (e.g., Mega Spectral), specific
  Joker in shop slots or Buffoon packs, observatory (Telescope + Mega
  Celestial), Perkeo (The Soul rolls Perkeo).
- Erratic Deck filters for face-card count, no-face searches, and suit-ratio searches.
- Joker list is alphabetized with a name filter for quick searching; Reset All
  clears filters and preferences back to defaults.
- Enable toggle, save/load state (Z/X + 1-5), reroll hotkeys (Ctrl+R,
  Ctrl+A), settings UI (Ctrl+T).
- Rust benchmark harness compares current speed against the Original Brainstorm
  DLL where the older ABI supports the same fixture, and reports comparable
  result mismatches.
- Production release automation publishes the `latest` release with a versioned
  title and versioned zip artifact.

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

Strict full-suite benchmark report:

```bash
mise run bench-full
```

Actual Lua UI UX benchmark report:

```bash
mise run bench-ux
```

See `Immolate/BENCH.md` for benchmark workflows.

## Versioning & Release
The source of truth for the mod version is `[manifest].version` in
`lovely.toml`. `steamodded_compat.lua` carries the same version for Steamodded
metadata and is checked by `mise run check-version`.

Use this when bumping versions:

```bash
VERSION=3.2 mise run bump-version
```

`mise run release` runs validation, builds `target/rust/Immolate.dll`, stages a
`Brainstorm/` install folder, and creates `release/Brainstorm_v<VERSION>.zip`.

`.github/workflows/release.yml` runs on pushes to `master` and can also be
triggered manually. It rebuilds the release zip and updates the production
release titled `Brainstorm Supercharged v<VERSION>` at tag `latest`.

## Documentation
- `AGENTS.md`: contributor and agent-facing project rules.
- `BalatroSource_Guide.md`: verified Balatro source mechanics relevant to
  search parity and future mod work.
- `Immolate/BENCH.md`: benchmark harness, gates, and fixture groups.
- `NOTICE.md`: project, rewrite, Immolate, and third-party attribution notices.

## Installation (no build)
Download the latest release zip from
https://github.com/KrishRVH/Brainstorm/releases/tag/latest and extract it into
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
- Configure Erratic Deck filters when searching for opening hands by face-card
  count, no faces, or suit concentration.
- Use "Enable Brainstorm" to disable runtime actions without losing settings.
- Use "Reset All" in the Brainstorm tab to restore filter and Erratic deck
  settings to defaults.

## Troubleshooting
- Missing DLL or wrong build: rerun `mise run build` and
  `mise run deploy`. If auto-detection fails, set `TARGET` to the full
  `.../Balatro/Mods/Brainstorm` path.
