# SLICE-2 — Movement switch

Project: N64 Kart Racing (N64KR)
Date: 2026-10-31
Status: PASSED
Base: SpaghettiKart `ce0f935`, branch `n64kr` (on top of SLICE-1)

## What it does
Normal driving physics is no longer hard-wired to `func_8002D268`. A vehicle profile now picks a **movement model**, and upstream calls `n64kr_movement_update()` instead.

- Module: `src/n64kr/movement.{h,c}`. A function table indexed by `N64krMovementModel`:
  - `N64KR_MOVEMENT_GROUND`: calls `func_8002D268` unchanged.
  - `N64KR_MOVEMENT_TEST_FLOATY`: runs ground physics, then multiplies downward `velocity[1]` by 0.85 each frame. Test only. Proves the switch and previews where hover will plug in.
- `N64krVehicleProfile` gained `movementModel`. The 8 stock profiles use GROUND. New profile 8 = "Floaty test (Mario stats)" (stats MARIO, model TEST_FLOATY), selectable in **Enhancements → N64KR → Player 1 Vehicle Profile**.

## Hooks in upstream files [VERIFIED: source]
| File:line (at time of edit) | Caller | Mode |
|---|---|---|
| `src/player_controller.c:351` | `func_80028864` | Grand Prix, normal driving |
| `src/player_controller.c:360` | `func_80028864` | `PLAYER_UNKNOWN_0x80` branch |
| `src/player_controller.c:380` | `func_80028C44` | Time Trials / Versus |
| `src/player_controller.c:400` | `func_80028D3C` | Battle |

Each call is `n64kr_movement_update(player, camera, screenId, playerId); // N64KR HOOK`.

Left unchanged on purpose:
- `func_8002E594` (hit / spin / squish "effect" physics) and `control_cpu_movement` (off-screen CPU). MK64 still chooses those by **state**; the profile only chooses what "normal driving" means.
- `src/spawn_players.c:1164` in `func_8003CD98`: the one-off call that settles karts at race start. Stays on ground physics. **[CONCLUSION]** A hover model may need its own settle step later.

## Build [VERIFIED: build output]
Incremental Debug build: 164 s (new files trigger a re-glob/reconfigure), 0 errors, no warnings from N64KR code.

## In-game test [VERIFIED: user playtest]
1. Profile = "Own character": driving identical to stock. **PASS**
2. Profile = "Floaty test (Mario stats)": normal driving on the ground, floaty descents when airborne. **PASS**

## Next: SLICE-3
Real hover model (`n64kr_movement_hover`) ported from the movement-only parts of DKR's `func_80046524` (`src/racer.c`), keeping MK64's `actor_terrain_collision`. First open question from RECON-003: the timestep conversion between MK64's fixed per-frame update and DKR's `updateRate`.
