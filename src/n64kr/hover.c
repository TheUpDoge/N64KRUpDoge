#include <libultraship.h>
#include <defines.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "spawn_players.h"
#include "port/Game.h"
#include "n64kr/hover.h"
#include "n64kr/vehicle_profile.h"

// One MK64 tick is 1/60 s, the same as DKR's updateRate 1. DKR terms that
// are scaled by updateRate use HOVER_DT; terms DKR applies once per logic
// frame were tuned at 30 Hz, so they are halved per tick (HOVER_PER_FRAME).
#define HOVER_DT 1.0f
#define HOVER_PER_FRAME 0.5f

// Real DKR stats come from n64kr_dkr_hover.txt, written next to the game by
// `n64kr dkr-hover-stats` (tools/n64kr-rs) from the user's own DKR ROM. The
// file holds ROM data and is never committed. Without it, the placeholders
// below are used.
#define DKR_STATS_FILE "n64kr_dkr_hover.txt"
#define DKR_NUM_CHARACTERS 10
#define DKR_CURVE_LENGTH 16
// DKR Character enum (include/enums.h in the DKR decomp): 9 = Diddy.
#define DKR_DEFAULT_CHARACTER 9

// PLACEHOLDERS (no stats file). Flat speed curve; gravity in MK64 units
// per tick^2 (MK64 karts: ~0.43).
#define HOVER_PLACEHOLDER_SPEED_CURVE 0.282f
#define HOVER_PLACEHOLDER_GRAVITY 0.3f
// DKR top speed for the placeholder curve: full thrust (curve * 1.7)
// balances drag (0.004 * v^2). The x0.5 per-tick factor cancels out.
#define HOVER_PLACEHOLDER_DKR_TOP_SPEED 10.95f

typedef struct {
    bool loaded;
    f32 weight[DKR_NUM_CHARACTERS];
    f32 handling[DKR_NUM_CHARACTERS];
    f32 curve[DKR_NUM_CHARACTERS][DKR_CURVE_LENGTH];
    // DKR units/tick where full thrust balances drag, per character.
    f32 topSpeed[DKR_NUM_CHARACTERS];
} DkrHoverStats;

static DkrHoverStats sDkr;
static bool sDkrLoadTried;

// Hovercraft speed curve lookup (racer.c:1807-1818): lerp by |velocity|, 0..12.
static f32 dkr_speed_curve(const f32* curve, f32 speed) {
    s32 index;
    f32 frac;

    if (speed < 0.0f) {
        speed = -speed;
    }
    if (speed > 12.0f) {
        speed = 12.0f;
    }
    index = (s32) speed;
    frac = speed - index;
    return (curve[index + 1] * frac) + (curve[index] * (1.0f - frac));
}

// Above 12 the curve lookup is clamped, so thrust stays at curve[12] and the
// balance point can sit past it (13.5 at most for the stock tables).
#define DKR_TOP_SPEED_SEARCH_MAX 40.0f

static f32 dkr_top_speed(const f32* curve) {
    f32 speed;

    for (speed = 0.0f; speed < DKR_TOP_SPEED_SEARCH_MAX; speed += 0.01f) {
        if ((0.004f * speed * speed) >= (dkr_speed_curve(curve, speed) * 1.7f)) {
            return speed;
        }
    }
    return DKR_TOP_SPEED_SEARCH_MAX;
}

static bool parse_floats(const char* text, f32* out, s32 count) {
    char* end;
    s32 i;

    for (i = 0; i < count; i++) {
        out[i] = strtof(text, &end);
        if (end == text) {
            return false;
        }
        text = end;
    }
    return true;
}

static void load_dkr_stats(void) {
    FILE* file;
    char line[512];
    char key[16];
    s32 offset;
    s32 character;
    s32 curves = 0;
    bool weight = false;
    bool handling = false;

    sDkrLoadTried = true;
    file = fopen(DKR_STATS_FILE, "r");
    if (file == NULL) {
        return;
    }
    while (fgets(line, sizeof(line), file) != NULL) {
        if (sscanf(line, "%15s%n", key, &offset) != 1) {
            continue;
        }
        if (strcmp(key, "weight") == 0) {
            weight = parse_floats(line + offset, sDkr.weight, DKR_NUM_CHARACTERS);
        } else if (strcmp(key, "handling") == 0) {
            handling = parse_floats(line + offset, sDkr.handling, DKR_NUM_CHARACTERS);
        } else if (strcmp(key, "curve") == 0) {
            char* rest;
            character = (s32) strtol(line + offset, &rest, 10);
            if ((character >= 0) && (character < DKR_NUM_CHARACTERS) &&
                parse_floats(rest, sDkr.curve[character], DKR_CURVE_LENGTH)) {
                sDkr.topSpeed[character] = dkr_top_speed(sDkr.curve[character]);
                curves++;
            }
        }
    }
    fclose(file);
    sDkr.loaded = weight && handling && (curves == DKR_NUM_CHARACTERS);
}

static s32 dkr_character(void) {
    s32 character = CVarGetInteger(N64KR_CVAR_HOVER_CHARACTER, DKR_DEFAULT_CHARACTER);

    if ((character < 0) || (character >= DKR_NUM_CHARACTERS)) {
        return DKR_DEFAULT_CHARACTER;
    }
    return character;
}

bool n64kr_hover_has_dkr_stats(void) {
    if (!sDkrLoadTried) {
        load_dkr_stats();
    }
    return sDkr.loaded;
}

// HYPOTHESIS: MK64 units per DKR unit, set each tick so the hover's top
// speed equals the speed this kart cruises at in MK64. Stock cruise speed
// is where thrust (topSpeed^2 / 25, func_80030150 / player_accelerate_alternative)
// balances friction (v * 0.12 * kartFriction, func_8002D268): about 5.9
// units/tick at 150cc. Fallback for a kart with no stats yet.
#define HOVER_FALLBACK_UNIT_SCALE 0.54f

static f32 hover_unit_scale(const Player* player, f32 dkrTopSpeed) {
    f32 friction = 0.12f * player->kartFriction;
    f32 cruiseSpeed;

    if ((friction <= 0.0f) || (player->topSpeed <= 0.0f) || (dkrTopSpeed <= 0.0f)) {
        return HOVER_FALLBACK_UNIT_SCALE;
    }
    cruiseSpeed = ((player->topSpeed * player->topSpeed) / 25.0f) / friction;
    return cruiseSpeed / dkrTopSpeed;
}

// Water (SLICE-4). DKR lifts the craft by 0.75 of its depth per 30 Hz logic
// frame (racer.c:2121-2124), so 0.375 per tick, and caps the climb out of
// water at 4 DKR units/tick (racer.c:2145-2147).
#define HOVER_BUOYANCY_PULL 0.375f
#define HOVER_WATER_MAX_RISE 4.0f

typedef struct {
    f32 throttle; // 0 .. 1
    f32 brake;    // 0 .. 1.2
} HoverState;

static HoverState sHover[N64KR_NUM_PLAYERS];

void n64kr_hover_reset(s32 playerIndex) {
    if ((playerIndex >= 0) && (playerIndex < N64KR_NUM_PLAYERS)) {
        sHover[playerIndex].throttle = 0.0f;
        sHover[playerIndex].brake = 0.0f;
    }
}

// MK64's water level doubles as the fall-out plane: lava on Bowser's Castle
// and Big Donut, a void on Rainbow Road, Skyscraper and the rest. Only
// these tracks have real water under it.
static bool track_has_floatable_water(void) {
    return IsKoopaTroopaBeach() || IsRoyalRaceway() || IsDkJungle() || IsBansheeBoardwalk() || IsSherbetLand();
}

// Height of water under the player, or false if there is none to float on.
// gPlayerWaterLevel is refreshed at the end of each tick by
// update_player_environment_and_hazard_state().
static bool water_surface(s32 playerIndex, f32* level) {
    if (!track_has_floatable_water()) {
        return false;
    }
    *level = gPlayerWaterLevel[playerIndex];
    return true;
}

static f32 dot3(const f32* a, const f32* b) {
    return (a[0] * b[0]) + (a[1] * b[1]) + (a[2] * b[2]);
}

void n64kr_hover_velocity_stage(Player* player, s32 playerIndex, u16 buttons, Vec3f newVelocity) {
    HoverState* hover;
    bool throttleHeld;
    bool brakeHeld;
    bool driftHeld;
    bool grounded;
    Vec3f forward;
    Vec3f right;
    f32 velocity[3];
    f32 forwardVel;
    f32 lateralVel;
    f32 thrust;
    f32 brakeForce;
    f32 speedCurve;
    f32 grip;
    f32 drag;
    f32 dragSq;
    f32 vertical;
    f32 unitScale;
    f32 waterLevel;
    f32 depth;
    f32 gravity;
    f32 dkrTopSpeed = HOVER_PLACEHOLDER_DKR_TOP_SPEED;
    const f32* curve = NULL;
    s32 character = 0;
    s32 i;

    if ((playerIndex < 0) || (playerIndex >= N64KR_NUM_PLAYERS)) {
        return;
    }
    // MK64 boost ramps (DK Jungle's wooden ramp, the asphalt boost ramps)
    // launch with their own thrust (func_80030150 returns boostPower +
    // kartPropulsionStrength) and a long, slow fall (func_8002AB70 raises the
    // vertical divisor unk_DAC to 20-25). Leave MK64's newVelocity untouched
    // until the ramp effect ends so those jumps clear their gaps.
    if (player->effects & (BOOST_RAMP_WOOD_EFFECT | BOOST_RAMP_ASPHALT_EFFECT)) {
        return;
    }
    hover = &sHover[playerIndex];
    if (n64kr_hover_has_dkr_stats()) {
        character = dkr_character();
        curve = sDkr.curve[character];
        // One scale for every character (from Diddy) keeps DKR's speed
        // differences between characters, e.g. T.T. ~12% faster than Diddy.
        dkrTopSpeed = sDkr.topSpeed[DKR_DEFAULT_CHARACTER];
    }
    unitScale = hover_unit_scale(player, dkrTopSpeed);
    if (curve != NULL) {
        // racer.c:4233 (weight * 0.45) and :1819 (x0.75 for hovercraft),
        // DKR units/tick^2 -> MK64 units.
        gravity = sDkr.weight[character] * 0.45f * 0.75f * unitScale;
    } else {
        gravity = HOVER_PLACEHOLDER_GRAVITY;
    }

    if (player->type & PLAYER_HUMAN) {
        throttleHeld = (buttons & A_BUTTON) != 0;
        brakeHeld = (buttons & B_BUTTON) != 0;
        driftHeld = (buttons & R_TRIG) != 0;
    } else {
        // CPU drivers don't go through a controller.
        throttleHeld = player->kartPropulsionStrength > 0.0f;
        brakeHeld = false;
        driftHeld = false;
    }
    // MK64 sets effects bit 8 while airborne (func_8002D268, stage 3).
    grounded = (player->effects & 8) == 0;

    // Kart frame: columns of orientationMatrix (MK64 thrust is local +Z).
    for (i = 0; i < 3; i++) {
        forward[i] = player->orientationMatrix[i][2];
        right[i] = player->orientationMatrix[i][0];
    }

    // Work in DKR units from here on. Start from the current velocity:
    // hover ignores MK64's force step entirely.
    for (i = 0; i < 3; i++) {
        velocity[i] = player->velocity[i] / unitScale;
    }
    forwardVel = dot3(velocity, forward);
    lateralVel = dot3(velocity, right);

    // Throttle ramp (racer.c:1636-1658). DKR's velocity sign is flipped:
    // its "velocity > 0" means reversing.
    if (throttleHeld) {
        hover->throttle += HOVER_DT * 0.016f;
        if (hover->throttle > 1.0f) {
            hover->throttle = 1.0f;
        }
    } else {
        hover->throttle -= HOVER_DT * 0.016f;
        if (hover->throttle < 0.0f) {
            hover->throttle = 0.0f;
        }
    }
    thrust = hover->throttle;
    // Reversing with A held: full force at once to pull out of it.
    if ((forwardVel < 0.0f) && throttleHeld) {
        thrust = 1.0f;
    }
    thrust *= 3.0f;
    thrust = (thrust * thrust) / 9.0f;
    if (forwardVel < 0.0f) {
        thrust *= 1.2f;
    }

    // Brake ramp (racer.c:1659-1677).
    if (brakeHeld) {
        hover->brake += HOVER_DT * 0.046f;
        if (hover->brake > 1.2f) {
            hover->brake = 1.2f;
        }
    } else {
        hover->brake -= HOVER_DT * 0.046f;
        if (hover->brake < 0.0f) {
            hover->brake = 0.0f;
        }
    }
    brakeForce = hover->brake * 3.0f;
    brakeForce = (brakeForce * brakeForce) / 9.0f;
    if (forwardVel < 0.0f) {
        brakeForce *= 0.3f;
    }

    // Speed curve x top speed (racer.c:1807-1818, 1883-1895). A mushroom
    // boost forces full thrust, like DKR's boostTimer.
    speedCurve = ((curve != NULL) ? dkr_speed_curve(curve, forwardVel) : HOVER_PLACEHOLDER_SPEED_CURVE) * 1.7f;
    if (player->effects & BOOST_EFFECT) {
        thrust = 1.0f;
        brakeForce = 0.0f;
        speedCurve = 2.0f;
    }

    // Lateral grip, the hovercraft slide (racer.c:1976-1993). DKR's R-drift
    // also cuts thrust to x0.8 with the stick centred; skipped (no stick here).
    grip = lateralVel * 0.07f;
    if (driftHeld) {
        grip *= 2.0f;
        if (lateralVel > 2.5f || lateralVel < -2.5f) {
            thrust *= 1.5f;
        }
        if (brakeHeld) {
            grip *= 2.0f;
        }
    }
    grip *= HOVER_PER_FRAME;

    // Thrust and brake along the kart's forward axis (racer.c:1994-2001).
    thrust *= speedCurve * HOVER_PER_FRAME;
    brakeForce *= (speedCurve / 2.0f) * HOVER_PER_FRAME;

    // Quadratic drag (racer.c:2010-2021).
    dragSq = forwardVel * forwardVel;
    if (forwardVel < 0.0f) {
        dragSq = -dragSq;
    }
    if ((dragSq < 1.0f && dragSq > -1.0f) && !throttleHeld) {
        drag = forwardVel * 0.004f * 8.0f;
    } else {
        drag = dragSq * 0.004f;
    }
    drag *= HOVER_PER_FRAME;

    for (i = 0; i < 3; i++) {
        velocity[i] -= right[i] * grip;
        velocity[i] += forward[i] * (thrust - brakeForce - drag);
    }

    // Back to MK64 units for the horizontal part.
    newVelocity[0] = velocity[0] * unitScale;
    newVelocity[2] = velocity[2] * unitScale;

    // Vertical: weight-based gravity, halved when grounded, plus DKR's
    // vertical drag (racer.c:1819-1827). Kept in MK64 units.
    vertical = player->velocity[1];
    vertical -= gravity * HOVER_DT * (grounded ? 0.5f : 1.0f);
    vertical -= vertical * 0.025f * HOVER_DT;
    newVelocity[1] = vertical + ((velocity[1] * unitScale) - player->velocity[1]);

    // Buoyancy: below the water surface the craft is pulled back up so its
    // underside rides on the water. Staying afloat keeps MK64 from ever
    // flagging it as submerged, so no Lakitu rescue.
    if (water_surface(playerIndex, &waterLevel)) {
        depth = (waterLevel + player->boundingBoxSize) - player->pos[1];
        if (depth > 0.0f) {
            newVelocity[1] = depth * HOVER_BUOYANCY_PULL;
            if (newVelocity[1] > HOVER_WATER_MAX_RISE * unitScale) {
                newVelocity[1] = HOVER_WATER_MAX_RISE * unitScale;
            }
        }
    }
}
