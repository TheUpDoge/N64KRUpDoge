# N64KR — N64 Kart Racing

A fork of [SpaghettiKart](https://github.com/HarbourMasters/SpaghettiKart) (the Mario Kart 64 PC port) that mixes in **Diddy Kong Racing** and **GoldenEye 007**, and is built to take content from more N64 games.

**Core idea:** a character does not own its physics. Character, vehicle and movement model are separate and swappable: Mario can drive a DKR hovercraft, Diddy can drive an MK64 kart, and a DKR track can host MK64 racers.

All game content comes from **your own ROMs** at build or run time. This repository contains no ROM data, no extracted assets and no `.o2r` files; the tools below generate them locally into `build/` (git-ignored).

> Built AI-assisted (Claude Code) and play-tested by hand. Every change is recorded in `recon/` with evidence labels: **VERIFIED** (source, build output or in-game test), **CONCLUSION**, or **HYPOTHESIS**.

---

## What's new

| Feature | What it does | Status |
|---|---|---|
| Vehicle profiles | Physics (top speed, gravity, friction, acceleration, handling, size) come from a profile, not the character. "Own character" profiles keep MK64's stats exactly. | Tested in game |
| Movement models | Driving physics goes through a selectable movement model: MK64 ground, a test "floaty" model, and DKR hover. | Tested in game |
| DKR hovercraft | Hover physics ported from DKR's movement code, with DKR's real stats read from your DKR ROM. Floats on water and lava, works with boost ramps. | Tested in game |
| Vehicle select | On the character select screen, **R** switches each player between Kart and Hovercraft. | Tested in game |
| First DKR track | **Ancient Lake** converted from your DKR ROM into a playable SpaghettiKart custom track: geometry, textures, scenery, sky, item boxes, minimap, AI path. | Tested in game |
| Character registry | Racers are registry entries (`src/n64kr/character.c`): each names which MK64 character it borrows voice, HUD, size, AI and stats from until it has its own, plus optional own assets. MK64's 8 map to themselves, so the game is unchanged. Id 8 stays MK64's "no character" marker; new characters start at 9. | Tested in game |
| **Diddy Kong** | A full new character from DKR: kart sprites rendered from his DKR 3D model (driving pose, steering wheels), portrait, minimap icon, name, his own DKR voice clips, a select screen cell, and CPU appearances in Grand Prix. | Tested in game |
| Select screen pages | Push past the edge of the character grid to turn to the **DKR page**. MK64's 8-cell layout and menu code stay as they are; a page layer maps (page, cell) to a character. | Tested in game |
| **Dam Speedway** (GoldenEye) | A banked oval built over the reservoir of GoldenEye's Dam level, from your GoldenEye ROM: the level's scenery, Dam's own asphalt and striped walls, banked turns, water rescue. | Tested in game |
| **Dam Speedway B** | Two Dams facing each other, joined across the water: a longer oval with a dam wall behind each turn. | Tested in game |
| GoldenEye hazards | Oil drums on the road explode when touched (chain reactions, racers tumble); two of Dam's own watchtowers per track, each with a Dam guard (real GoldenEye model and animation) who raises his KF7 and fires bursts at passing racers. | Tested in game |
| GoldenEye music and sound | The Dam theme plays on the Dam tracks (rendered from GoldenEye's own sequences and instruments, looping; the action version on the final lap); gunfire, explosions, ricochets. | Tested in game |
| **Rare Cup** | A fifth cup in the cup select (after Special Cup, for Grand Prix and VS): Ancient Lake, Fossil Canyon (DKR), Dam Speedway, Dam Speedway B. Drawn as its own page: banner, course list and preview box as text. | Built; in-game test pending |
| **3D racers** | Characters with a 3D model are drawn as real-time 3D instead of sprites (Diddy, from his DKR model, with steering poses). Boo/Lakitu translucency, boost afterimage and steering front wheels in progress. Sprites stay as the fallback. | Body tested in game; effects built, not yet tested |

### Known limits
- Menu state still uses MK64's 8 stock ids, so a page character takes a stock character's "seat" (that character sits out of the race).
- Time trial records and ghosts save a page character as the stock character of its cell.
- Diddy's select cell is a still image (no blink/celebrate animation yet).
- 3D racers don't show star/lightning colour effects, slope tilt or wheel spin yet.

---

## Tools (`tools/n64kr/`)

All tools read **your own ROM** and write into `build/` (never committed). Python 3 (run with `python -I`); the Blender scripts need Blender 4.3 (headless, `-b`), and the track export also needs the HarbourMasters fork of Fast64.

| Tool | Input → output |
|---|---|
| `extract_dkr_hover_stats.py` | DKR ROM → `n64kr_dkr_hover.txt`: per-character weight, handling and hover curves used by the hover movement model. |
| `dkr_level_to_gltf.py` | DKR ROM → a DKR track as glTF + textures (`--mk64`: rescaled, start line and AI path laid out for MK64). Also the shared DKR ROM reader (asset tables, DKR compression, textures, sprites). |
| `blender_dkr_track.py` | Converted track → SpaghettiKart custom track mod via Fast64 (materials, transparency classes, surfaces). |
| `dkr_track_scene.py` | Track mod → `scene.json`: item boxes, minimap, sky colours. |
| `dkr_racer_to_gltf.py` | DKR ROM → a DKR racer (e.g. `diddycar`) as glTF with its driving animation as shape keys, plus wheel sprite frames. Decodes DKR's vertex animation format. |
| `blender_dkr_racer_sprites.py` | Racer glTF → the 1188 MK64 kart sprite frames (321 angles × wheel spin), rendered in SpaghettiKart's kart setup scene; packaged as a texture pack over a stock character or as a new character's folder. |
| `blender_dkr_racer_hud.py` | Racer glTF → race portrait, minimap icons, select screen face and nameplate. |
| `dkr_voice_to_wav.py` | DKR ROM → a racer's voice clips as WAV (parses DKR's sound bank, decodes N64 VADPCM). |
| `dkr_racer_to_model.py` | DKR ROM → an in-game 3D model: libultraship display lists for each driving pose, steering front wheels, `model.json`. |

### Rust tools (`tools/n64kr-rs/`)
New tools are written in Rust: one `n64kr` program, `cargo build --release` in that folder. GoldenEye formats come from the [GoldenEye decomp](https://github.com/n64decomp/007); the tool checks for the US ROM.

| Command | Input → output |
|---|---|
| `ge-bg-info`, `ge-textures`, `ge-gltf`, `ge-rooms-gltf` | A GoldenEye level's rooms (with bounding boxes), textures, the whole level or chosen rooms as glTF. |
| `ge-images` | GoldenEye images by number (sprites, effects) as PNG. |
| `ge-model` | An object model (props, guards, heads, guns): node tree, glTF; guards posed by an animation frame. |
| `ge-guard` | A posed Dam guard with head and KF7, as glTF (for checking poses). |
| `ge-dam-speedway [a\|b]` | Dam Speedway (A) or the twin-Dam B: track glTF + layout (AI path, item boxes, hazards, music) for `blender_dkr_track.py` / `dkr_track_scene.py`. |
| `ge-hazards` | Barrel, guard poses and effect sprites as in-game resources (mod `n64kr-ge-assets`). |
| `ge-audio` | GoldenEye songs rendered to looping WAV (an offline port of libultra's sequence player) and sound effects. |

### Making the Dam tracks
```bash
cd tools/n64kr-rs && cargo build --release && cd ../..
tools/n64kr-rs/target/release/n64kr ge-dam-speedway "<GoldenEye ROM>.z64" build/ge_tracks/dam_speedway a
tools/n64kr-rs/target/release/n64kr ge-dam-speedway "<GoldenEye ROM>.z64" build/ge_tracks/dam_speedway_b b
blender -b --factory-startup -P tools/n64kr/blender_dkr_track.py -- build/ge_tracks/dam_speedway build/x64/mods dam_speedway ge
blender -b --factory-startup -P tools/n64kr/blender_dkr_track.py -- build/ge_tracks/dam_speedway_b build/x64/mods dam_speedway_b ge
python -I tools/n64kr/dkr_track_scene.py build/ge_tracks/dam_speedway build/x64/mods dam_speedway "Dam Speedway" ge
python -I tools/n64kr/dkr_track_scene.py build/ge_tracks/dam_speedway_b build/x64/mods dam_speedway_b "Dam Speedway B" ge
tools/n64kr-rs/target/release/n64kr ge-hazards "<GoldenEye ROM>.z64" build/x64/mods/n64kr-ge-assets
tools/n64kr-rs/target/release/n64kr ge-audio "<GoldenEye ROM>.z64" build/x64/mods/n64kr-ge-assets
```

### Making Diddy (example)
```bash
python -I tools/n64kr/dkr_racer_to_gltf.py "<DKR ROM>.z64" diddycar build/dkr_racers
blender -b docs/MK64_Spaghetti_Adjusted_Kart_Setup.blend --python tools/n64kr/blender_dkr_racer_sprites.py -- build/dkr_racers/diddycar build/x64/mods/n64kr-dkr-diddy diddy_kart
blender -b --python tools/n64kr/blender_dkr_racer_hud.py -- build/dkr_racers/diddycar build/x64/mods/n64kr-dkr-diddy diddy build/x64/mk64.o2r toad 248,120,0
python -I tools/n64kr/dkr_voice_to_wav.py "<DKR ROM>.z64" diddy build/x64/mods/n64kr-dkr-diddy
python -I tools/n64kr/dkr_racer_to_model.py "<DKR ROM>.z64" diddycar diddy build/x64/mods/n64kr-dkr-diddy
```
The full render takes a few minutes; `--preview` renders 12 sample frames.

---

## Engine code (`src/n64kr/`)

| Module | Role |
|---|---|
| `vehicle_profile.{h,c}` | Vehicle profiles and the stats a racer uses. |
| `movement.{h,c}` | Movement model switch (ground / floaty / hover). |
| `hover.{h,c}` | DKR hover physics, water/lava buoyancy. |
| `vehicle_select.{h,c}` | Kart/Hover choice on the select screen. |
| `character.{h,c}` | Character registry and accessors (voice, sprites, HUD, body, AI, stats, names). |
| `character_assets.cpp` | Kart sprite and HUD texture paths for characters with their own assets. |
| `character_select.{h,c}` | Select screen pages, page characters, CPU roster and GP rival handling. |
| `voice.{h,cpp}` | Own voice clips, played through the port's audio mixer with distance falloff. |
| `racer3d.{h,c}`, `racer3d_model.cpp` | Real-time 3D racers. |
| `ge_hazards.{h,cpp}` | GoldenEye barrels and guards as track actors (`n64kr:ge_barrel`, `n64kr:ge_guard` in scene.json). |
| `sfx.{h,cpp}` | Positional sound effects with their own voices, mixed next to the port's HMAS mixer. |
| `music.{h,cpp}` | Other games' songs played through HMAS under N64KR sequence ids (a track's scene.json `Sequence`). |
| `cups.{h,cpp}` | N64KR cups after the battle cup: registration, which cups each mode can pick, the cup page in the cup select. |

### Changes to SpaghettiKart's own files
Upstream files only get thin one-line hooks, each marked `// N64KR HOOK`, so SpaghettiKart updates can still be merged. By file: `menu_items.c` 18, `player_controller.c` 9, `kart_dma.c` 9, `menus.c` 8, `render_objects.c` 7, `spawn_players.c` 6, `ending/code_80281780.c` 5, `render_player.c` 4, `code_80005FD0.c` 4, `audio/external.c` 3, `effects.c` 2, and one each in `racing/race_logic.c`, `racing/actors.c`, `racing/actors_extended.c`, `port/ui/PortMenu.cpp`, `port/Engine.cpp`, `menu_items.h`, `engine/editor/SceneManager.cpp`, `engine/actors/Cloud.cpp`, `ending/ceremony_and_credits.c`, `code_80057C60.c`. Besides that, the voice-line sites call `n64kr_char_voice_line` and per-character table reads go through the registry accessors (see `recon/006-selection/SLICE-8_Character_Registry.md`).

---

## Building and playing
1. Build SpaghettiKart as usual (`docs/BUILDING.md`) and generate `mk64.o2r` from your US Mario Kart 64 ROM.
2. Run the tools above with your DKR ROM (US Rev 1 tested) to generate the mods into `build/x64/mods/`.
3. For GoldenEye content, build the Rust tool and run the Dam commands above with your GoldenEye ROM (US).
4. Launch. Diddy is on the DKR page (push right past the character grid). The Rare Cup (Ancient Lake, Fossil Canyon, Dam Speedway, Dam Speedway B) is past Special Cup in Grand Prix and VS; Fossil Canyon converts like Ancient Lake (`dkr_level_to_gltf.py … fossil_canyon build/dkr_tracks --mk64`, then the Blender and scene steps).

## Project records (`recon/`)
Each step ("slice") has a record with what was found in the source, what was changed, and what was verified in game:
`000-baseline` · `001-vehicle-profile` · `002-movement-switch` · `003-movement` (movement source map) · `004-hover` · `005-content-merge` (first DKR track) · `006-selection` (vehicle select, character registry, Diddy) · `007-3d-racers` · `008-goldeneye-dam` (Dam Speedway A/B, hazards, GoldenEye audio) · `009-circuit-select` (Rare Cup).

## Roadmap
- 3D racers: finish effects, then Mario (rigged model in SpaghettiKart's kart setup file) and the rest of the MK64 cast from other N64 games the user owns (Mario Party, Mario Tennis, Smash Bros.).
- More DKR racers and tracks; DKR music (the Rust sequence renderer reads DKR's libultra banks too).
- GoldenEye's gun-barrel intro and an UpDoge Studios splash; planes and tanks.
- Data-driven select pages (characters from mods without code changes).

## Credits
- [HarbourMasters / SpaghettiKart](https://github.com/HarbourMasters/SpaghettiKart) and the [MK64 decomp](https://github.com/n64decomp/mk64).
- The [Diddy Kong Racing decomp](https://github.com/DavidSM64/Diddy-Kong-Racing) (CC0) — reference for DKR's formats and movement.
- The [GoldenEye 007 decomp](https://github.com/n64decomp/007) — reference for GoldenEye's level, model, animation, texture and audio formats.
- Overkart 64 community for the kart setup Blender scene (shipped in SpaghettiKart's docs).
- N64KR by Mitchel Friend (UpDoge).
