# Tutorial: build N64KR and generate its content

This walks you from nothing to racing Diddy, Banjo and Conker in the Rare Cup. Everything game-specific is generated from **your own ROMs** on your machine. Nothing from a ROM is downloaded, and nothing you generate should ever be committed or shared.

Tested on Windows 10/11. Linux should work for the engine (see SpaghettiKart's `docs/BUILDING.md`), but the commands below are written for Windows with Git Bash.

## 1. What you need

| Need | For | Notes |
|---|---|---|
| Visual Studio 2022 (or Build Tools) with "Desktop development with C++" and MSVC v143 | the engine | as in SpaghettiKart's [`docs/BUILDING.md`](../BUILDING.md) |
| CMake, Git, Python 3 | the engine build | as in SpaghettiKart's docs |
| [Blender 4.3](https://download.blender.org/release/Blender4.3/) (portable zip is fine) | racer sprites, faces, HUD; track export | |
| [HarbourMasters' Fast64](https://github.com/HarbourMasters/fast64) installed in Blender | track export only | install the add-on and enable it |
| [Rust](https://rustup.rs) | the `n64kr` content tool (DKR and GoldenEye) | `cargo` on your PATH |

ROMs (all `.z64`, big-endian):

| Game | Version | Check |
|---|---|---|
| Mario Kart 64 | US | SHA-1 `579C48E211AE952530FFC8738709F078D5DD215E` |
| Diddy Kong Racing | US Rev 1 (v1.1) | `n64kr` checks the SHA-1 and says if it's unsupported |
| GoldenEye 007 | US | `n64kr` checks the SHA-1 |

In PowerShell: `Get-FileHash .\yourrom.z64 -Algorithm SHA1`. A different hash usually means the wrong byte order (`.n64`/`.v64`) or region.

## 2. Get the code and build

```bash
git clone --recursive -b n64kr https://github.com/TheUpDoge/N64KRUpDoge.git
cd N64KRUpDoge
cmake -S . -B build/x64 -G "Visual Studio 17 2022" -T v143 -A x64
cmake --build build/x64 --config Release
```

Success looks like `Spaghettify.exe` in `build/x64/Release/` (or `Debug/`). Run it from `build/x64` (that folder holds `mk64.o2r` and `mods/`, as in SpaghettiKart's `docs/BUILDING.md`):

```bash
cd build/x64 && ./Release/Spaghettify.exe
```

Pick your **Mario Kart 64** ROM when asked. That generates `mk64.o2r`, and the game should start as plain SpaghettiKart. Close it and go back to the repository folder for the next steps.

## 3. Build the content tool

All the ROM reading and converting is one Rust program, `n64kr`:

```bash
cd tools/n64kr-rs && cargo build --release && cd ../..
```

Run `tools/n64kr-rs/target/release/n64kr` with no arguments to see every command.

## 4. Generate the DKR racers

The commands below use these placeholders; set them for your machine:

```bash
N64KR=tools/n64kr-rs/target/release/n64kr
DKR="path/to/Diddy Kong Racing (USA) (Rev 1).z64"
BLENDER="path/to/blender-4.3/blender.exe"
```

For each racer, run the six steps. Diddy:

```bash
$N64KR dkr-racer "$DKR" diddycar build/dkr_racers
"$BLENDER" -b docs/MK64_Spaghetti_Adjusted_Kart_Setup.blend --python tools/n64kr/blender_dkr_racer_sprites.py -- build/dkr_racers/diddycar build/x64/mods/n64kr-dkr-diddy diddy_kart
"$BLENDER" -b --python tools/n64kr/blender_dkr_racer_hud.py -- build/dkr_racers/diddycar build/x64/mods/n64kr-dkr-diddy diddy build/x64/mk64.o2r toad 248,120,0
"$BLENDER" -b --python tools/n64kr/blender_dkr_racer_faces.py -- build/dkr_racers/diddycar build/x64/mods/n64kr-dkr-diddy diddy
$N64KR dkr-voice "$DKR" diddy build/x64/mods/n64kr-dkr-diddy
$N64KR dkr-racer-model "$DKR" diddycar diddy build/x64/mods/n64kr-dkr-diddy
```

Banjo and Conker are the same with these values:

| Racer | object | hud / voice name | kart folder | minimap colour | model size |
|---|---|---|---|---|---|
| Diddy | `diddycar` | `diddy` | `diddy_kart` | `248,120,0` | 1.0 |
| Banjo | `BanjoCar` | `banjo` | `banjo_kart` | `70,130,230` | 1.2 (add `1.2` at the end of the last command) |
| Conker | `ConkaCar` | `conker` | `conker_kart` | `220,50,40` | 1.0 |

The sprite render is the slow step (a few minutes per racer; add `--preview` for 12 test frames). The `n64kr` steps take a second or two.

## 5. Generate the DKR tracks

```bash
$N64KR dkr-level "$DKR" ancient_lake build/dkr_tracks --mk64
"$BLENDER" -b --python tools/n64kr/blender_dkr_track.py -- build/dkr_tracks/ancient_lake_mk64 build/x64/mods ancient_lake
$N64KR dkr-track-scene build/dkr_tracks/ancient_lake_mk64 build/x64/mods ancient_lake "Ancient Lake"

$N64KR dkr-level "$DKR" fossil_canyon build/dkr_tracks --mk64
"$BLENDER" -b --python tools/n64kr/blender_dkr_track.py -- build/dkr_tracks/fossil_canyon_mk64 build/x64/mods fossil_canyon
$N64KR dkr-track-scene build/dkr_tracks/fossil_canyon_mk64 build/x64/mods fossil_canyon "Fossil Canyon"
```

DKR's hovercraft stats (used by the Hover vehicle):

```bash
$N64KR dkr-hover-stats "$DKR" build/x64/n64kr_dkr_hover.txt
```

## 6. Generate the GoldenEye content

```bash
GE="path/to/007 - GoldenEye (USA).z64"
$N64KR ge-dam-speedway "$GE" build/ge_tracks/dam_speedway a
$N64KR ge-dam-speedway "$GE" build/ge_tracks/dam_speedway_b b
"$BLENDER" -b --factory-startup -P tools/n64kr/blender_dkr_track.py -- build/ge_tracks/dam_speedway build/x64/mods dam_speedway ge
"$BLENDER" -b --factory-startup -P tools/n64kr/blender_dkr_track.py -- build/ge_tracks/dam_speedway_b build/x64/mods dam_speedway_b ge
$N64KR dkr-track-scene build/ge_tracks/dam_speedway build/x64/mods dam_speedway "Dam Speedway" ge
$N64KR dkr-track-scene build/ge_tracks/dam_speedway_b build/x64/mods dam_speedway_b "Dam Speedway B" ge
$N64KR ge-hazards "$GE" build/x64/mods/n64kr-ge-assets
$N64KR ge-audio "$GE" build/x64/mods/n64kr-ge-assets
```

The track export loads Fast64 from your Blender add-ons; if `--factory-startup` keeps it from loading on your setup, drop that flag.

## 7. Play

Start the game from `build/x64` again; mods are picked up from `build/x64/mods/` at start.

- **Characters:** push right past the character grid (or follow the arrow) to the **DKR page** with Diddy, Banjo and Conker. **R** switches Kart / Hover. DKR racers nobody picks join as CPUs.
- **Tracks:** in Grand Prix or VS, move past the Special Cup (follow the arrow) to the **Rare Cup**: Ancient Lake, Fossil Canyon, Dam Speedway, Dam Speedway B. The Rare Cup only appears when all four tracks are installed.

## Troubleshooting

| Problem | Check |
|---|---|
| A DKR racer shows as a stock character | Its mod folder exists in `build/x64/mods/` and has `racers/`, `textures/`, `sound/`. The game log (`build/x64/logs/`) prints `Loaded mod: n64kr-dkr-<name>`. |
| No Rare Cup | All four track mods must be installed; the log says which one is missing (`[n64kr] n64kr:rare_cup not added: track ... is not installed`). |
| Blender script fails at import | Use Blender 4.3; for tracks, enable Fast64. |
| "Unsupported ROM" | Check the SHA-1 and byte order (section 1). |
