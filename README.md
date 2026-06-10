# Brainstorm for Balatro
<img width="1536" height="864" alt="Brainstorm 3" src="https://github.com/user-attachments/assets/68a97977-3c80-4259-9378-58540a7b749e" />

Brainstorm is a Balatro mod that rapidly searches for seeds matching
voucher/pack/tag/Joker/Erratic Deck filters and integrates directly into the
game loop through Lua plus a native Rust DLL. The current DLL includes a
user-toggleable CUDA fast path for Red Deck tag/voucher/pack/Observatory
searches and falls back to the Rust CPU engine for the rest.

## Setup (Required First)
1. Install `smods-1.0.0-beta` (Steamodded) for Balatro.
2. Install Lovely.
3. Build the DLL (from source):
```bash
mise trust
mise run setup
mise run build
```
4. Deploy the mod (from source):
```bash
TARGET=/mnt/c/Users/Krish/AppData/Roaming/Balatro/Mods/Brainstorm mise run deploy
```
If `/mnt/c` write permissions fail, run with `sudo` or adjust mount permissions.
If you do not want to build from source, skip to "Installation (no build)" below.

## Credits
This project is licensed under CC BY-NC-SA 4.0.

- Brainstorm was created by OceanRamen. This rewrite by KRVH derives from
  https://github.com/OceanRamen/Brainstorm, which is licensed under the Mozilla
  Public License Version 2.0.
- The Immolate native DLL source was created by MathIsFun0. This project uses
  CC BY-NC-SA 4.0 to remain compatible with the original Immolate source:
  https://github.com/SpectralPack/Immolate/tree/26f41efcc313f045bc8bdbf49e5851c56ac40b31.
- KRVH completed the Immolate C++ rewrite, ported unfinished functionality, and
  added the Joker search workflow.

## Features
- Auto-reroll with dual-tag support (order-agnostic or same-tag-twice).
- First-shop filters: voucher, two pack slots (e.g., Mega Spectral), specific
  Joker in shop slots or Buffoon packs, observatory (Telescope + Mega
  Celestial), Perkeo (The Soul rolls Perkeo).
- Erratic Deck filters for face-card count, no-face searches, and suit-ratio searches.
- Joker list is alphabetized with a name filter for quick searching; Reset All
  clears filters and preferences back to defaults.
- Save/load state (Z/X + 1-5), reroll hotkeys (Ctrl+R, Ctrl+A), settings UI (Ctrl+T).
- Settings include `AP: USE CUDA`, which controls whether supported filters try
  the GPU fast path.

## Requirements
- Balatro (Steam, Windows 64-bit).
- Lovely injector (required): https://github.com/ethangreen-dev/lovely-injector
- WSL2 for building/deploying from this repo.
- mise for development tasks: https://mise.jdx.dev/
- Mise-managed tools:
```bash
mise install
```
- LuaJIT and LuaRocks for Lua syntax/lint checks. Install them with your OS
  package manager, then run:
```bash
mise run setup-lua
```
- Rust 1.96+ with the Windows GNU target:
```bash
rustup target add x86_64-pc-windows-gnu
```
- MinGW-w64 and Wine are required for C++ oracle builds, DLL comparison, and
  benchmarks.
- CUDA Toolkit with `nvcc` and a compatible host compiler is required for the
  GPU-enabled build. On Ubuntu/WSL, `nvidia-cuda-toolkit` plus `gcc-12` matches
  the default `CUDAHOSTCXX=gcc-12`. The default CUDA target is `sm_89` for RTX
  4090; override with `BRAINSTORM_CUDA_ARCH` if needed. Set
  `BRAINSTORM_SKIP_CUDA_BUILD=1` only when intentionally building a CPU-only DLL.
- Write access to `%AppData%\Roaming\Balatro\Mods`.

## Build & Deploy (from source)
`mise.toml` is the development interface. Run `mise trust` once per checkout,
then use `mise run <task>`. `mise run setup` installs mise-managed tools,
installs local Lua lint tools through LuaRocks, and runs `mise run doctor` to
check the remaining WSL/system dependencies.

`mise run build` builds the current Rust native DLL and writes `Immolate.dll`.
There is one Rust implementation now. `mise run build-cpp` still builds the C++
oracle from `Immolate/CPP/` for parity checks, but the game uses the Rust DLL.
The Rust build embeds PTX compiled from `Immolate/Rust/src/cuda/brainstorm_cuda.cu`;
at runtime the DLL loads the CUDA Driver API dynamically (`nvcuda.dll` on
Windows, `libcuda` under WSL/Linux). The Brainstorm settings tab controls
whether the DLL tries CUDA. If CUDA is enabled but unavailable, or if a filter
is not GPU-supported, search automatically uses the Rust CPU path.

`mise run lint` runs Lua formatting, LuaJIT bytecode syntax checks, luacheck,
C++ formatting checks, rustfmt, and clippy. `mise run check-rust` runs Rust
formatting, clippy, unit tests, DLL export/import validation, C++ vs Rust
parity, and a benchmark regression smoke. `mise run check` runs both.

Strict full-suite benchmark gate:

```bash
mise run bench-full
```

Actual Lua UI UX benchmark gate:

```bash
mise run bench-ux
```

See `Immolate/Rust/BENCH.md` for benchmark workflows.

**Release packaging:** `mise run release` (runs `mise run check`, then
creates `release/Brainstorm_v3.1.zip`).

**Development release:** `.github/workflows/dev-release.yml` runs on pushes to
`master` and can also be triggered manually. It rebuilds the release zip and
updates the prerelease titled `dev release` at tag `dev-release`.

## Documentation
- `AGENTS.md`: contributor and agent-facing project rules.
- `BalatroSource_Guide.md`: verified Balatro source mechanics relevant to
  search parity and future mod work.
- `Immolate/Rust/PLAN.md`: current Rust DLL architecture, FFI contract, and
  maintenance invariants.
- `Immolate/Rust/BENCH.md`: benchmark harness, gates, and fixture groups.

## Installation (no build)
Download the v3.1 release zip from
https://github.com/KrishRVH/Brainstorm/releases/tag/3.1 and extract it into
`%AppData%\Roaming\Balatro\Mods\Brainstorm\` (same payload as
`mise run deploy`).
The folder name must be `Brainstorm`.
Reload the game to activate the mod.

Copy the mod files into `%AppData%\Roaming\Balatro\Mods\Brainstorm\` (same
payload as `mise run deploy`) if you are assembling the payload manually:
```
Brainstorm/
├── Brainstorm.lua
├── UI.lua
├── Immolate.dll           # Native DLL
├── config.lua
├── lovely.toml
├── nativefs.lua
└── steamodded_compat.lua
```
You can copy these from a release zip (e.g. `release/Brainstorm_v3.1.zip`) or
from the repo after someone provides `Immolate.dll`.

## Usage
- Open settings: Ctrl+T. Toggle auto-reroll: Ctrl+A. Manual reroll: Ctrl+R.
- Save/load state: Z/X + 1-5.
- Configure filters: dual tags, voucher, pack (two shop slots), Joker
  (searchable list + location), souls, observatory, Perkeo.
- Configure Erratic Deck filters when searching for opening hands by face-card
  count, no faces, or suit concentration.
- Use "Reset All" in the Brainstorm tab to restore filter and Erratic deck
  settings to defaults.

## Troubleshooting
- Missing DLL or wrong build: rerun `mise run build` and
  `TARGET=/mnt/c/Users/Krish/AppData/Roaming/Balatro/Mods/Brainstorm mise run deploy`.
