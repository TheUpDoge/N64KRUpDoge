# N64KR (N64 Kart Racing): project instructions for Claude Code

## What this project is
A modular kart-racing platform where **character, vehicle and movement model are separate and swappable**.
- **Host:** a fork of SpaghettiKart (HarbourMasters/SpaghettiKart), the native PC port of Mario Kart 64. It builds from the MK64 decomp and generates assets from the user's own ROM.
- **Repo model (user decision 2026-11-01):** N64KR is its own project on the user's public GitHub repo (remote `origin`, branch `n64kr`) and pulls SpaghettiKart updates from remote `upstream` (HarbourMasters). Never push to `upstream`.
- **Donor:** Diddy Kong Racing decomp (DavidSM64/Diddy-Kong-Racing), used as the reference for hovercraft and plane movement.
- **Core rule:** the CHARACTER DOES NOT OWN PHYSICS. A VehicleProfile and MovementModel do.
- **Long-term goal:** one PC port that combines MK64 and DKR, and can later take content (characters, vehicles, movement models, tracks) from more games we reverse engineer. Each game's content comes from the user's own ROM at runtime, never from the repo. The slices below are the first step toward this.

Read `recon/003-movement/RECON-003_Movement_Source_Map.md` before touching movement code. It maps the real call chain with file:line references.

## Machine
- Windows 10/11 with Visual Studio 2022 ("Desktop development with C++" plus MSVC v143), CMake, Python 3 and Git.
- No WSL needed for SpaghettiKart. WSL is only needed later, to build the DKR decomp.
- Optional plugin: `/plugin marketplace add rehan-remade/universal-modder`, then `/plugin install universal-modder@universal-modder`.

## Hard rules
1. **Never commit ROMs, extracted assets, `.o2r` files or DKR misc-asset values.** Keep them out of git: `*.z64`, `*.n64`, `*.v64`, `*.o2r`, `baseroms/`, `build/`.
2. **Label claims** in docs and commit messages as VERIFIED (source, build output or in-game test), CONCLUSION or HYPOTHESIS. Never present a hypothesis as fact.
3. **Don't invent function names.** Many functions are still `func_8002xxxx`. Refer to them by that name and file:line.
4. **Keep upstream patches small.** Put N64KR code in `src/n64kr/` and limit edits to upstream files to thin hooks. Mark each hook with `// N64KR HOOK`, so merging upstream updates stays easy.
5. **Verify in the running game** before calling a slice done, and record what was observed.
6. Don't open PRs to SpaghettiKart or the DKR decomp without the user's say-so, and read their contribution rules first. Some decomp communities ban AI-generated contributions.

## Expected MK64 ROM
US version, `.z64` byte order, SHA-1 `579C48E211AE952530FFC8738709F078D5DD215E`.
Check it with `Get-FileHash .\mk64.us.z64 -Algorithm SHA1` in PowerShell. A different hash usually means `.n64`/`.v64` byte order or a non-US version.

## Milestones (do them in order, one checkpoint at a time)
Done: SLICE-0 (`recon/000-baseline/`), SLICE-1 (`recon/001-vehicle-profile/`), SLICE-2 (`recon/002-movement-switch/`).
Hover (SLICE-3/4/5, `recon/004-hover/`): works with real DKR stats; open checks listed in each record.
SLICE-6 first DKR track (`recon/005-content-merge/`): Ancient Lake converts from the DKR ROM and drives. Pipeline: `tools/n64kr/dkr_level_to_gltf.py --mk64` → `blender_dkr_track.py` (Blender 4.3.2 + HarbourMasters Fast64 in `E:\Claude\tools\`) → `dkr_track_scene.py`. Output goes to `build/x64/mods/` and is never committed.
SLICE-7 vehicle select (`recon/006-selection/`): R on the character select screen toggles Kart/Hover per player.
SLICE-8 character registry (`src/n64kr/character.{h,c}`): characterId indexes N64KR characters; MK64 lookups go through n64kr_char_voice/sprites/hud/body/ai. Watch item: one unexplained intermittent crash (null I4 texture, Wario Stadium race start).
SLICE-9 first DKR character (`recon/006-selection/SLICE-9_First_DKR_Character.md`): Diddy rendered from his DKR model into kart sprites (`dkr_racer_to_gltf.py` → `blender_dkr_racer_sprites.py`), registry id 9 (8 stays MK64's "no character" marker); L on the select screen puts him in a cell's seat.

- **SLICE-0, baseline:** clone with `--recursive`, configure and build per SpaghettiKart's `docs/BUILDING.md`, generate `mk64.o2r` from the ROM, then drive one race. Record build time, warnings and anything broken.
- **SLICE-1, VehicleProfile seam:** add `n64kr::VehicleProfile` (top speed, gravity, friction, acceleration curve, handling, bounding box, hop). Add a per-player `vehicleProfileId`. Route the `gKart*Table[player->characterId]` physics reads (mostly `src/player_controller.c`, also `effects.c` and `spawn_players.c`) through it. Leave the `kart_dma.c` texture lookups keyed by character.
  - Pass if: identical driving when profile = own character; swapping Mario onto Bowser's profile makes him drive like Bowser.
- **SLICE-2, movement switch:** make the call to `func_8002D268` (normal driving physics) go through a movement selector chosen by the profile.
- **SLICE-3, hover:** a new `n64kr_movement_hover`, ported from the movement-only parts of DKR's `func_80046524` (`src/racer.c`), keeping MK64's `actor_terrain_collision`. Work out the timestep conversion first (DKR uses `updateRate`).

## Working style with the user
The user is a hands-on builder learning this stack, not a full-time C++ developer. Give exact commands, one checkpoint at a time, and say what output means success. Stop and ask before anything destructive or hard to undo.

## Repo layout (target)
```
N64KR/                 <- the user's SpaghettiKart fork
  CLAUDE.md
  src/n64kr/           <- our code
  recon/003-movement/  <- evidence docs
  labs/                <- throwaway experiments
```
