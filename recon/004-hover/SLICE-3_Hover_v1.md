# SLICE-3 — Hover model v1

Project: N64 Kart Racing (N64KR)
Date: 2026-10-31
Status: IN PROGRESS. v1 drives, speed calibrated; full pass test not yet run.
Base: branch `n64kr` on SpaghettiKart `ce0f935`. Plan: `RECON-004_Hover_Port_Map.md`.

## What it does
New vehicle profile **"Hovercraft (test stats)"** (Mario's MK64 stats for bounding box, hop and steering; movement model `N64KR_MOVEMENT_HOVER`). It runs `func_8002D268` as usual but replaces its force step with DKR hovercraft physics. MK64's steering, collision, walls, floors, landing and top-speed clamp stay in charge.

## Code
- `src/n64kr/hover.{h,c}`: velocity stage ported from DKR `func_80046524` (`racer.c` line refs in comments): throttle ramp (±0.016/tick), brake ramp (±0.046/tick, reverses when stopped), quadratic drag (0.004·v²), low lateral grip (0.07, R ×2, R+B ×4), mushroom boost = full thrust, weight-based gravity (halved when grounded) with 2.5%/tick vertical drag. DKR terms that aren't scaled by `updateRate` are halved per 60 Hz tick.
- `src/n64kr/movement.{h,c}`: `N64KR_MOVEMENT_HOVER`, `n64kr_movement_velocity_stage()` and `n64kr_record_input()`.
- `src/n64kr/vehicle_profile.{h,c}`: hovercraft profile, `n64kr_player_index()`, hover state reset on assign.

## Hooks in upstream files [VERIFIED: source]
| File | Hook |
|---|---|
| `src/player_controller.c`, `func_8002D268` | `n64kr_movement_velocity_stage(player, playerId, newVelocity)` after `newVelocity` is computed, before the Lakitu/hit zeroing and collision |
| `src/player_controller.c`, `handle_a_press_for_player_during_race` | `n64kr_record_input(player, controller)`, which records the controller actually driving the player (covers Time Trial replays on `gControllerSix/Seven/Eight`) |
| `src/port/ui/PortMenu.cpp` | menu entry 10, "Hovercraft (test stats)" |

## Speed calibration (bug found in playtest)
- First build: unit scale fixed at 0.75 (9 / 12), aimed at `gKartTopSpeedTable = 9`. **User playtest: "extremely fast".**
- Cause [VERIFIED: source]: 9 is only a clamp (`player_controller.c`, end of `func_8002D268`). Stock karts cruise where thrust `≈ topSpeed² / 25` (`player_accelerate_alternative` sets `kartPropulsionStrength`; `func_80030150` returns it) balances friction `v · 0.12 · kartFriction` (`func_8002D268` stage 2). At 150cc that's 320²/25 / 696 ≈ **5.9 units/tick** (≈ 70 on the speedometer). Hover was running at about 8.2–9.
- Fix: unit scale is computed per tick as `cruiseSpeed / HOVER_DKR_TOP_SPEED` from the player's own `topSpeed` and `kartFriction`, so hover top speed matches a stock kart in the same cc class (and custom CC).
- **User playtest after fix: "Speed feels right."** [VERIFIED]

## Placeholders (not DKR's real values)
| Value | Placeholder | Real source |
|---|---|---|
| Speed curve | flat 0.282 (DKR top speed 10.95) | ROM, `get_misc_asset(obj->header->miscAssetIds[0])` |
| Gravity | 0.3 MK64 units/tick² | ROM `ASSET_MISC_RACER_WEIGHT[characterId] · 0.45 · 0.75` |
| Handling | not used (MK64 steering) | ROM `ASSET_MISC_RACER_HANDLING` |

## Not yet verified
- [ ] Regression: "Own character" still drives identically to stock with the new hooks in place.
- [ ] Hover feel vs DKR hovercraft (slide, floaty landings), side by side in an emulator.
- [ ] Walls, slopes and landings with low grip (no sticking, jitter or sinking).

## Known gaps in v1
- Steering is still MK64's; DKR's handling-based turn is not ported.
- R also triggers MK64's kart hop; DKR's hover hop (+3.5 y velocity) is not ported.
- DKR's R-drift thrust ×0.8 with the stick centred is skipped (no stick value in the velocity stage).
- No water / buoyancy.
- CPU drivers can't take the hover profile (only the Player 1 override exists).
- Acceleration is DKR's (about 1.5 s to top speed), quicker than stock MK64.
