# SLICE-4 — Hovercraft on water

Project: N64 Kart Racing (N64KR)
Date: 2026-10-31
Status: FLOATS (core goal verified); remaining checks open
Base: branch `n64kr`, on top of SLICE-3 hover v1 (`6622a20`)

## MK64 water handling [VERIFIED: source]
- One function: `update_player_environment_and_hazard_state`, `src/player_controller.c:1551`. Called at the end of `func_8002D268`, `control_cpu_movement` and one more path (`:2154`, `:2416`, `:4739`).
  - `gPlayerWaterLevel[i] = get_water_level(player)` → `CM_GetWaterLevel` → `Track::GetWaterLevel` (`src/engine/tracks/Track.cpp:553`): highest `WaterVolumes` entry the player is over, else `Props.WaterLevel`.
  - `pos[1] <= waterLevel` → `PASS_OOB_OR_FLUID_LEVEL`; submerged by more than `boundingBoxSize` → `UNDER_OOB_OR_FLUID_LEVEL` (+ splash sound).
  - **Lakitu:** submerged by more than `boundingBoxSize` **and** `surfaceDistance[2] >= 600` → `LAKITU_RETRIEVAL` (`:1588-1591`).
- **[CONCLUSION]** If the hover stays on the surface it is never "submerged", so no Lakitu and **no new upstream hook is needed**. All SLICE-4 code is in `src/n64kr/hover.c`.

## The water plane is not always water [VERIFIED: source]
`Props.WaterLevel` is also MK64's fall-out plane (`include/defines.h:363` comment: "Set while mostly under the plane").

| Track | `Props.WaterLevel` | What it is |
|---|---|---|
| Koopa Troopa Beach | 0.0, tide moves it (`KoopaTroopaBeach.cpp:259-264`); volume at 0.8 | water |
| Royal Raceway | -60 | water (lake) |
| DK Jungle | -475 / -33.9 by section (`DKJungle.cpp:137`) | water (river) |
| Banshee Boardwalk | -80 | water |
| Sherbet Land | -18 (min Y − 10 over snow) | water |
| Bowser's Castle | -50, volume at 20 | lava |
| Big Donut | 100 | lava |
| Rainbow Road, Skyscraper, most others | 0 / -480 / min Y − 10 | void / out of bounds |

**[CONCLUSION]** v1 allowlists the five water tracks (`track_has_floatable_water()`). Lava and voids keep MK64's Lakitu behaviour.

## Buoyancy port [from DKR `func_80046524`]
- DKR: `buoyancy = waterHeight − (y − clip)`; `y += buoyancy · unkC4` with `unkC4 → 0.75` (`racer.c:2096-2124`); climb out capped at `y_velocity ≤ 4` (`:2145-2147`).
- Port: `depth = (waterLevel + boundingBoxSize) − pos[1]`; if `depth > 0`, `newVelocity[1] = depth · 0.375` (0.75 per 30 Hz frame → 0.375 per 60 Hz tick), capped at `4 · unitScale`.
- Uses `gPlayerWaterLevel` from the end of the previous tick.

## Build [VERIFIED: build output]
Incremental: 4 s, 0 errors, no warnings in `hover.c`.

## In-game test [VERIFIED: user playtest]
- Hovercraft on water: **"It floats"**. PASS.

## Not yet verified
- [ ] Steering on water (MK64 may treat the craft as airborne, effects bit 8, since there's no floor under deep water).
- [ ] Bobbing at the surface; Koopa Troopa Beach tide.
- [ ] Leaving water onto land (no bounce or stall; no big landing "POOMP" from MK64's airtime counter `unk_0C2`).
- [ ] Lava on Bowser's Castle / Big Donut still triggers Lakitu.
- [ ] Normal karts still sink (should, since buoyancy only runs for the hover profile).

## Not ported
DKR's water tilt (`rotate_racer_in_water`), wave properties, splash particles, water hop (+4.5 with R, `racer.c:2241-2245`).
