# RECON-003 — Movement & Vehicle Source Map

Project: N64 Kart Racing (N64KR)
Date: 2026-10-31
Status: COMPLETE (first pass)

Every claim is tagged:
- **[VERIFIED]**: read directly in source at the file:line given.
- **[CONCLUSION]**: follows from several verified facts but isn't proven by any one of them.
- **[HYPOTHESIS]**: plausible, not yet checked.

Source snapshots read (shallow clones):

| Repo | Commit | Date |
|---|---|---|
| HarbourMasters/SpaghettiKart (MK64 PC port, our host) | `ce0f935` | 2026-10-22 |
| DavidSM64/Diddy-Kong-Racing (DKR decomp) | `a5eb080` | 2026-10-01 |

Line numbers drift as upstream changes. Re-check them against your own clone before editing.

---

## 1. MK64 (SpaghettiKart): how a kart moves each frame

### Call chain [VERIFIED]

```
src/main.c:685                     func_80028F70()            // "Player controller", once per frame
src/player_controller.c:435          for each player:
                                       func_80028E70(player, camera, i, 0)
src/player_controller.c:406            switch (gModeSelection)
                                         TIME_TRIALS / VERSUS -> func_80028C44   (:368)
                                         BATTLE               -> func_80028D3C   (:386)
                                         default (GP)         -> func_80028864   (:305)
                                       each one picks a physics routine:
                                         func_8002D268  = normal driving physics   (:1833)
                                         func_8002E594  = "effect" physics (hit, spin, squish...) (:2170)
                                         control_cpu_movement = off-screen CPU kart (:2416)
```

So MK64 does have a choice point, but it chooses by **state** (normal / hit / off-screen CPU), not by **vehicle type**. There is only one vehicle. [VERIFIED]

### Inside `func_8002D268`, the normal driving physics (≈320 lines) [VERIFIED]

1. Input, steering, drift and effect handlers run first (`func_80027EDC`, `func_8002A79C`, `func_8002B830`, `func_8002BF4C`, `apply_effect`).
2. **Gravity and slope force.** The amount depends on whether both rear tyres are on `ASPHALT`. Off asphalt, a speed-indexed falloff table `sp104[]` scales it.
3. **Propulsion.** `func_80030150()` returns forward thrust, which is rotated by `player->orientationMatrix`.
4. **Velocity integration.** `v += (thrust + gravity − v·0.12·kartFriction) / 6000 / divisor`, with separate cases for boost and drift.
5. **Collision.** `actor_terrain_collision(&player->collision, boundingBoxSize, next, old)`, then wall and floor responses (`func_8003F46C`, `func_8003F734`, `func_8003FBAC`) for each of the three `surfaceDistance[]` slots.
6. **Landing.** `unk_0C2` counts airtime. Landing triggers the "POOMP" graphics flag, rumble and post-tumble gas.
7. **Commit.** It writes `pos` and `velocity`, then **clamps speed to `gKartTopSpeedTable[player->characterId]`** (`:2131–2136`).

### The coupling that matters most [VERIFIED]

Every kart physics stat is a table with **8 entries indexed by `player->characterId`**, declared in `include/kart_attributes.h`:

`gKartTopSpeedTable`, `gKartGravityTable`, `gKartFrictionTable`, `gKartAccelerationTables`, `gKartHandlingTable`, `gKartBoundingBoxSizeTable`, `gKartTurnSpeedReductionTable0/1`, `gKartHopInitialVelocityTable`, `gKartHopJerkTable`, `gKartTable800E3650`.

Uses of these tables, counted by grep:

| Table | Reads in player_controller.c + effects.c + spawn_players.c |
|---|---|
| gKartAccelerationTables | 50 |
| gKartTopSpeedTable | 15 |
| gKartGravityTable | 6 |
| gKartBoundingBoxSizeTable | 5 |
| gKartTable800E3650 | 4 |
| others | 1–2 each |

Kart **graphics** are keyed by character too: `gKartTextureTable0/1[player->characterId]` in `src/kart_dma.c:613–662`.

**[CONCLUSION]** In MK64 the character *is* the vehicle. That's exactly the "character owns physics" rule N64KR exists to break. It also makes the cleanest first seam: route those table reads through a **vehicle profile id** instead of `characterId`.

### A warning sign in SpaghettiKart's own C++ [VERIFIED]

`src/engine/actors/Cloud.cpp:97–102` changes the global tables directly (`gKartGravityTable[characterId] = Gravity`) and restores them later (`:68–69`). That works, but it's fragile because two players with the same character share one entry. **[CONCLUSION]** Our vehicle-profile layer would replace this pattern.

---

## 2. DKR: per-vehicle dispatch (the thing MK64 lacks)

### The dispatch [VERIFIED]

Both the human and the AI update paths `switch (racer->vehicleID)`:

- Player: `update_player_racer`, `src/racer.c:4121`, switch at `:4400`
- AI: `update_AI_racer`, `src/racer.c:8664`, switch at `:8811`

| vehicleID | Update function | Defined at |
|---|---|---|
| VEHICLE_CAR | `update_car` | racer.c:4703 |
| VEHICLE_HOVERCRAFT | `func_80046524` (unnamed upstream) | racer.c:1560 |
| VEHICLE_PLANE | `func_80049794` (unnamed upstream) | racer.c:2521 |
| VEHICLE_LOOPDELOOP | `func_8004CC20` | racer.c:3654 |
| VEHICLE_CARPET / FLYING_CAR | `update_carpet` | racer.c:3978 |
| Bosses (Tricky, Bluey, Smokey, Bubbler, Wizpig, Rocket) | `update_<boss>` | `src/vehicle_*.c` |

Enum: `include/enums.h:35–46`. Bosses start at `VEHICLE_BOSSES`.

### It isn't fully clean [VERIFIED]

`vehicleID` is also checked in **dozens of places outside the dispatch** in `racer.c`, including collision candidates (`:6693`), water and hover handling (`:6700`, `:6719`, `:7188`) and wheel logic (`:3154`, `:3161`). Every update function also takes the whole `Object_Racer`.

### Hovercraft update `func_80046524` (≈711 lines) — what it uses [VERIFIED]

- **Input:** `gCurrentStickX/Y`, `gCurrentRacerInput` (A = throttle, B = brake), `updateRate` / `updateRateF`.
- **Throttle and brake ramps:** throttle moves by `0.016·dt` toward 0..1 and brake by `0.046·dt` toward 0..1.2. Force is roughly the square of each (`(x·3)²/9`).
- **Steering:** `steerAngle` approaches the stick by `/8` per tick for humans and `/32` for CPU or finished racers.
- **Vertical:** gravity is `gCurrentRacerWeightStat` (×0.5 in one branch) with drag `y_vel −= y_vel·0.025·dt`. Buoyancy and water come from `get_wave_properties`, `rotate_racer_in_water` and `racer->buoyancy`.
- **Surfaces:** `racer->wheel_surfaces[4]`, `groundedWheels`, and special handling for `SURFACE_STONE`.
- **Top speed:** `handle_racer_top_speed(obj, racer) * 1.7` (`racer.c:7451`).
- **Lateral slide:** `racer->lateral_velocity`, with thresholds at ±2.5.
- **Also inside:** items, attacks, head turning, sound, particles and camera. These are not movement and must be cut away when we extract it.

### DKR's character stats ALSO come from characterId [VERIFIED]

`racer.c:4232–4239` and `:8700–8705`:

```c
gCurrentRacerWeightStat   = misc_asset(ASSET_MISC_RACER_WEIGHT)[characterId] * 0.45;
gCurrentRacerHandlingStat = misc_asset(ASSET_MISC_RACER_HANDLING)[characterId];
```

These values are **ROM assets, not constants in the source**. **[CONCLUSION]** To reproduce DKR hover handling for a given character we must extract those tables from the user's own DKR ROM. We never commit them.

**[CONCLUSION]** DKR separates *movement style* by vehicle but still takes *stats* from the character. N64KR should hold both: `VehicleDefinition` sets the movement model and base stats, and an optional `CharacterModifier` scales them.

---

## 3. Side by side

| Concern | MK64 / SpaghettiKart | DKR |
|---|---|---|
| Choice point | by state (normal / hit / CPU) | by vehicle (`switch vehicleID`) |
| Stats keyed by | characterId (8 tables) | characterId (ROM misc assets) |
| Units / timestep | fixed per-frame, ÷6000 scaling | `updateRate` ticks (variable) |
| Collision call | `actor_terrain_collision(&player->collision, …)` | `update_racer_collision`, wheel surfaces |
| Water | none for karts | buoyancy and wave properties |
| Racer struct | `Player`, ≈0xDD8 bytes | `Object_Racer`, ≈0x224 bytes |

---

## 4. What this means for the first slice

**[CONCLUSION]** Recommended order. Each step can be tested in the running game.

1. **SLICE-0: baseline.** Build SpaghettiKart unmodified on Windows and drive a race.
2. **SLICE-1: VehicleProfile seam.** Add `n64kr::VehicleProfile`, which holds top speed, gravity, friction, acceleration curve, handling and bounding box, plus a `vehicleProfileId` for each player. Point the characterId-indexed table reads in `player_controller.c` at the profile. **Pass test:** with profile = character's own stats, driving is identical. Swap Mario onto Bowser's profile and Mario drives like Bowser.
3. **SLICE-2: movement switch.** Wrap the call to `func_8002D268` so a profile can choose a different movement function (`n64kr_movement_ground` = the original, `n64kr_movement_hover` = new).
4. **SLICE-3: hover model.** Port the movement-only parts of DKR's `func_80046524` (throttle and brake ramps, steering approach, low-friction lateral slide, weight-based vertical) into `n64kr_movement_hover`. Keep MK64's `actor_terrain_collision` for collision. **Pass test:** the kart floats, slides and responds to throttle the way DKR hover does. Compare against side-by-side video of DKR.
5. Later: water and buoyancy for hover, using MK64 water surfaces such as Koopa Troopa Beach.

## 5. Still unverified

- **[HYPOTHESIS]** `func_80030150` is the propulsion/acceleration curve lookup (it reads `gKartAccelerationTables`). Not yet read in full.
- **[HYPOTHESIS]** MK64's per-frame timestep is fixed at 30 or 60 Hz in SpaghettiKart. DKR scales by `updateRate`. We need a conversion factor before porting hover constants.
- **[UNKNOWN]** SpaghettiKart's license. No LICENSE file was found in the clone. Check the GitHub "About" panel before publishing anything.
- **[UNKNOWN]** Whether SpaghettiKart or the DKR decomp accept AI-assisted contributions. No written policy was found in their repo files. Our work stays in our own fork.
