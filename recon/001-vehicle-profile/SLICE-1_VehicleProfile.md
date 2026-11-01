# SLICE-1 — VehicleProfile seam

Project: N64 Kart Racing (N64KR)
Date: 2026-10-31
Status: PASSED
Base: SpaghettiKart `ce0f935`, branch `n64kr`

## What it does
Physics no longer comes from `player->characterId`. Each player gets a vehicle profile when spawned, and MK64's per-kart physics tables are indexed by `n64kr_stats_id(player)`.

- Module: `src/n64kr/vehicle_profile.{h,c}`. 8 stock profiles (Mario … Bowser), each pointing at one kart's MK64 stat tables (`statsId`).
- Per-player profile id is stored in N64KR (indexed by `player - gPlayers`), so the upstream `Player` struct (0xDD8) is unchanged.
- Test override: CVar `gN64KR.P1VehicleProfile` (0 = own character, N = profile N−1), shown as **Enhancements → N64KR → Player 1 Vehicle Profile**. Applied at `spawn_player()`, so it takes effect from the next race start.

**Design note [CONCLUSION]:** profiles reference the stock tables instead of copying values into a struct. That keeps each upstream hook to a single index swap and also covers tables not yet understood (`D_800E2400`, `D_800E24B4`, `D_800E2568`, `gKartTable800E3650`). New movement models (SLICE-3 hover) will add their own fields to the profile.

## Hooks in upstream files [VERIFIED: source, script output]
Every index swap is greppable as `n64kr_stats_id(`. Includes and calls are marked `// N64KR HOOK`.

| File | Reads routed | Tables |
|---|---|---|
| `src/player_controller.c` | 105 | `gKart*` (88), surface grip `D_800E2A90/2AB0/2E90/2EB0/3210/3410/3610` (16), local `sp24` handling table (1) |
| `src/effects.c` | 19 | `gKartGravityTable`, `gKartBoundingBoxSizeTable`, hop tables `D_800E37x0`, `gTopSpeedTable[0]`; `temp_v1` in `func_8008C528` |
| `src/spawn_players.c` | 18 | friction, gravity, bounding box, `gTopSpeedTable` / `D_800E2400` / `D_800E24B4` / `D_800E2568` per CC, custom-CC `calc()` macro; plus the `n64kr_assign_vehicle_profile()` call |
| `src/engine/actors/Cloud.cpp` | 6 | hop and gravity writes now land on the profile's stats entry |
| `CMakeLists.txt` | — | adds `src/n64kr/*` to the source glob |
| `src/port/ui/PortMenu.cpp` | — | N64KR sidebar with the profile combobox |

Left keyed by `characterId` on purpose: `gCharacterSize` (driver body size, 7 reads in `player_controller.c`), kart textures (`kart_dma.c`), colours, sounds, HUD icons.

## Build [VERIFIED: build output]
- Incremental Debug build: 47 s, 0 errors, no warnings from N64KR code.
- Gotcha: editing `CMakeLists.txt` triggers a reconfigure that re-applies libultraship's imgui patch with `git`. Git must be on PATH in that shell, or the build fails with "Patch inapplyable in clean state".

## In-game test [VERIFIED: user playtest]
1. Profile = "Own character": driving identical to stock. **PASS**
2. Mario with profile = Bowser: drives like Bowser. **PASS**

## Known limits
- `Cloud.cpp` still edits shared stat tables, so two players on the same profile share the cloud effect (same fragility as upstream, now keyed by profile instead of character).
- Only Player 1 has an override. CPU and other human players use their character's own profile.
- **[HYPOTHESIS]** CPU off-screen movement (`control_cpu_movement`) reads no per-kart tables directly. Not checked in full.

## Next: SLICE-2
Route the call to `func_8002D268` (normal driving physics) through a movement selector chosen by the profile.
