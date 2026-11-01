# RECON-004 — Hover port map (SLICE-3, checkpoints 1–2)

Project: N64 Kart Racing (N64KR)
Date: 2026-10-31
Status: COMPLETE (plan for SLICE-3 implementation)

Tags: **[VERIFIED]** read in source at file:line · **[CONCLUSION]** follows from verified facts · **[HYPOTHESIS]** not yet checked.

| Repo | Commit | Notes |
|---|---|---|
| SpaghettiKart (`E:\Claude\N64KR`, branch `n64kr`) | `31587a7` (on `ce0f935`) | line numbers below include N64KR hooks |
| DKR decomp (`E:\Claude\refs\Diddy-Kong-Racing`, read-only) | `a5eb080` | same commit as RECON-003 |

**License [VERIFIED]:** the DKR decomp is CC0 1.0 (`LICENSE.md`), public domain. Porting its *logic* is fine. Its ROM data (misc assets) still never gets committed.

---

## 1. Timestep

### DKR [VERIFIED]
- `src/video.h:57-66`: `enum LogicUpdateRates { LOGIC_NULL, LOGIC_60FPS, LOGIC_30FPS, LOGIC_20FPS, ... }`, so `updateRate` = **number of 60 Hz ticks covered by one logic frame**. Comment: vanilla DKR defaults to `LOGIC_30FPS` (2).
- `src/thread3_main.c:357-362`: `sLogicUpdateRate = fb_update(...)`, capped at `LOGIC_10FPS` (6).

### SpaghettiKart [VERIFIED]
- `src/main.c:110`: `s32 gTickLogic = 2;`. `src/main.c:730-732` runs `process_game_tick()` `gTickLogic` times per game frame.
- `src/main.c:671`: each tick adds `TRACK_TIMER_ITER` to the race clock, and `include/defines.h:92` defines that as `0.01666666 // 1 / 60` (NTSC).
- `src/main.c:685`: the player controller (`func_80028F70`) runs **once per tick**.

**[CONCLUSION]** MK64 physics ticks at 60 Hz, which is the same unit as DKR's `updateRate = 1`. One call of our hover model per MK64 tick = `updateRate 1`, `updateRateF 1.0`.

### The catch [VERIFIED → CONCLUSION]
Not every DKR term is scaled by `updateRate`. In `func_80046524`:
- **Scaled** (correct at any rate): throttle/brake ramps (`racer.c:1637-1671`), gravity (`:1821-1823`), vertical drag (`:1827`), steering turn (`:1831`), position integration (`move_object(... * updateRateF)`, `:2062`).
- **Not scaled** (applied once per logic frame): lateral grip `lateral_velocity * 0.07` (`:1977`), thrust `ox1 * spF4` (`:1995-1997`), brake (`:1999-2001`), slope push (`:2003-2008`), quadratic drag `v² * 0.004` (`:2010-2021`).

**[CONCLUSION]** DKR was tuned at 30 Hz logic. Running the unscaled terms at 60 Hz would double their effect. In the port, multiply each unscaled per-frame term by **0.5 per MK64 tick** (first-order match for small coefficients). A 30 Hz substep is the fallback if the feel is off.

---

## 2. DKR hover movement — what to port

`func_80046524(updateRate, updateRateF, obj, racer)`, `src/racer.c:1560-2270` [VERIFIED].

### Port (movement only)
| Piece | Lines | Notes |
|---|---|---|
| Steer approach | 1613-1627 | `steerAngle += (stick − steerAngle)·dt / 8` (CPU / finished: `/32`) |
| Throttle ramp | 1636-1658 | ±`0.016·dt`, clamp 0..1; force `spF4 = (3t)²/9`; full force if already moving and A held; `×1.2` when `velocity > 0` |
| Brake ramp | 1659-1677 | B + stick down (or reversing): ±`0.046·dt`, clamp 0..1.2; `spF0 = (3b)²/9`, `×0.3` when `velocity > 0` |
| Speed curve | 1807-1818 | `xVelTemp` = lerp of a per-vehicle ROM table indexed by `|velocity|` (0..12) |
| Gravity | 1819-1827 | `weight *= 0.75`; `y_vel −= weight·dt` (`×0.5` if ≥3 wheels grounded); `y_vel −= y_vel·0.025·dt` |
| Steering turn | 1828-1838 | yaw `−= stickOrSteer·8·dt·handling`; stick down boosts turn (`1 + y/112`) and cuts speed (`1 − y/300`) |
| Top speed | 1883 | `xVelTemp *= handle_racer_top_speed() * 1.7`; boost sets `xVelTemp = 2` |
| Lateral grip | 1976-1993 | `v −= right · (lateral_velocity·0.07)`; R held: `×2` (and `×2` again with B); R changes thrust ×0.8 / ×1.5 |
| Thrust / brake | 1994-2001 | `v −= forward·(spF4·xVelTemp)`; `v += forward·(spF0·xVelTemp/2)` |
| Slope push | 2002-2009 | grounded: `weight·dt·0.25 ·(pitch·forward + roll·right)` |
| Drag | 2010-2021 | `v −= forward·(signed v²·0.004)` (×16 when braking at low speed) |
| Velocity from displacement | 2137-2142 | after collision, velocity = actual movement / dt |
| Local frame | 2203-2212 | `lateral_velocity`, `velocity` = world velocity rotated into kart frame |
| Hop (R press) | 2236-2256 | grounded (≥2 wheels) or in water: `y_vel += 3.5` (water: `+4.5`, cap 5.5); cooldown `unk1FB` |

Sign convention [VERIFIED]: forward thrust is `v −= ox1 · …` and `racer->velocity` is negative when moving forward (`:1712`, `:2011`). Flip on porting.

### Skip (not movement)
Items (`handle_racer_items`), attacks, head turning, zipper sounds/voice, rumble, particles (`:2148-2202`), camera (`cameraYaw`, `second_racer_camera_update`), animation frames, Taj pad, trick sounds, and water/buoyancy (`:2081-2124`), which is deferred to a later slice.

### ROM-only values [VERIFIED: loaded from misc assets, not in source]
| Value | Where | Placeholder for v1 |
|---|---|---|
| Weight | `racer.c:4232-4233`: `ASSET_MISC_RACER_WEIGHT[characterId] * 0.45` | tune in game |
| Handling | `racer.c:4237-4238`: `ASSET_MISC_RACER_HANDLING[characterId]` | 1.0 (AI uses 1 at `:8783`) |
| Speed curve | `racer.c:4257`: `get_misc_asset(obj->header->miscAssetIds[0])` (per vehicle) | flat curve |
| Stone factor | `racer.c:1900`: `ASSET_MISC_32` | unused in v1 |

**[CONCLUSION]** v1 uses labelled placeholder values. Extracting the real tables from the user's DKR ROM (never committed) is a follow-up.

---

## 3. MK64 side — where hover plugs in

`func_8002D268`, `src/player_controller.c:1835-2154` [VERIFIED] runs in three stages:

1. **Input, steering, effects** (`:1865-1930`): `func_80027EDC`, `func_8002C11C`, `func_8002A79C` (human), `func_8002B830`, `func_8002BF4C`, `apply_effect`, `func_80037BB4`, `func_8002AB70`, `func_8002FCA8`, slope gravity (`:1886-1928`), `func_8002C7E4`.
2. **Forces → `newVelocity`** (`:1931-1975`): thrust `sp184[2] = func_80030150()` rotated by `orientationMatrix`; `newVelocity += (thrust + gravity − v·0.12·kartFriction) / 6000 / divisor`; Lakitu/hit zeroing.
3. **Integrate, collide, land** (`:1977-2153`): `next = pos + velocity` (**previous** velocity) `+ kartHopVelocity`; `actor_terrain_collision`; orientation matrix; airtime `unk_0C2` and landing; floor/wall responses `func_8003F46C`, `func_8003F734`, `func_8003FBAC` (these also adjust `newVelocity`); surface height; `pos = next`; `velocity = newVelocity`; human top-speed clamp to `gKartTopSpeedTable` (`:2133-2139`); `update_player_environment_and_hazard_state`.

**[CONCLUSION]** Recommended hook: one thin call between stages 2 and 3, around `:1975`:

```c
n64kr_movement_velocity_stage(player, playerId, newVelocity); // N64KR HOOK
```

- GROUND: does nothing (stock behaviour, verifiable with the SLICE-1/2 pass test).
- HOVER: overwrites `newVelocity` with the DKR math from §2, using MK64's `rotation[1]` / `orientationMatrix` as the kart frame.
- Stage 3 (collision, walls, floors, landing, surfaces, top-speed clamp) stays MK64's. That matches the plan in `CLAUDE.md` ("keeping MK64's `actor_terrain_collision`") without copying 320 lines.

The hover movement model then becomes "`func_8002D268` with the hover velocity stage active", fitting the SLICE-2 table.

---

## 4. Open questions

- **[HYPOTHESIS] Units:** DKR and MK64 world units are probably not the same scale. MK64's human top speed is 9.0 units/tick (`src/data/kart_attributes.c:98-100`). DKR's top speed comes from the ROM speed curve, so it can't be computed from source. Plan: one `N64KR_HOVER_UNIT_SCALE` constant, tuned so hover top speed sits near MK64's, then checked against DKR video.
- **[HYPOTHESIS] Steering:** v1 keeps MK64's stage-1 steering (sets `rotation[1]`) and replaces only velocity. DKR's handling-based turn (`:1831-1838`) may be needed for the right feel; that would be a second hook.
- **[HYPOTHESIS] R button:** MK64 uses R for hop/drift in stage 1 (`kartHopVelocity`). DKR hover uses R for a stronger hop and a grip-loosening drift. v1 leaves MK64's hop in place and applies DKR's R grip change.
- **[UNKNOWN]** How MK64's input is read for the velocity stage (throttle/brake state per player). Find during implementation; `func_80030150` reads it for thrust.
- **[UNKNOWN]** Whether `func_8003F46C` / `func_8003F734` / `func_8003FBAC` behave well with low-friction velocities (sliding into walls).

## 5. SLICE-3 plan (from here)
1. Add `n64kr_movement_velocity_stage()` hook. Check GROUND is identical (pass test).
2. Write `N64KR_MOVEMENT_HOVER`: throttle/brake ramps, quadratic drag, low lateral grip, weight gravity, ×0.5 per-tick scaling for unscaled terms, labelled placeholder stats.
3. Add a "Hovercraft (test stats)" profile to the menu.
4. User test: drives, slides, floats; compare with DKR hovercraft in an emulator.
