#include <libultraship.h>
#include <defines.h>
#include <math.h>

#include "n64kr/hover.h"
#include "n64kr/vehicle_profile.h"

// One MK64 tick is 1/60 s, the same as DKR's updateRate 1. DKR terms that
// are scaled by updateRate use HOVER_DT; terms DKR applies once per logic
// frame were tuned at 30 Hz, so they are halved per tick (HOVER_PER_FRAME).
#define HOVER_DT 1.0f
#define HOVER_PER_FRAME 0.5f

// PLACEHOLDERS. DKR reads these from ROM misc assets (weight, handling and
// the per-vehicle speed curve). Replace once they are extracted from the
// user's own DKR ROM; never commit the real values.
// Speed curve is flat here (DKR top speed then = HOVER_DKR_TOP_SPEED).
#define HOVER_PLACEHOLDER_SPEED_CURVE 0.282f
// Downward acceleration in MK64 units per tick^2 (MK64 karts: ~0.43).
#define HOVER_PLACEHOLDER_GRAVITY 0.3f

// DKR top speed for the placeholder curve: full thrust (curve * 1.7)
// balances drag (0.004 * v^2). The x0.5 per-tick factor cancels out.
#define HOVER_DKR_TOP_SPEED 10.95f

// HYPOTHESIS: MK64 units per DKR unit, set each tick so the hover's top
// speed equals the speed this kart cruises at in MK64. Stock cruise speed
// is where thrust (topSpeed^2 / 25, func_80030150 / player_accelerate_alternative)
// balances friction (v * 0.12 * kartFriction, func_8002D268): about 5.9
// units/tick at 150cc. Fallback for a kart with no stats yet.
#define HOVER_FALLBACK_UNIT_SCALE 0.54f

static f32 hover_unit_scale(const Player* player) {
    f32 friction = 0.12f * player->kartFriction;
    f32 cruiseSpeed;

    if ((friction <= 0.0f) || (player->topSpeed <= 0.0f)) {
        return HOVER_FALLBACK_UNIT_SCALE;
    }
    cruiseSpeed = ((player->topSpeed * player->topSpeed) / 25.0f) / friction;
    return cruiseSpeed / HOVER_DKR_TOP_SPEED;
}

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
    s32 i;

    if ((playerIndex < 0) || (playerIndex >= N64KR_NUM_PLAYERS)) {
        return;
    }
    hover = &sHover[playerIndex];
    unitScale = hover_unit_scale(player);

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
    speedCurve = HOVER_PLACEHOLDER_SPEED_CURVE * 1.7f;
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
    vertical -= HOVER_PLACEHOLDER_GRAVITY * HOVER_DT * (grounded ? 0.5f : 1.0f);
    vertical -= vertical * 0.025f * HOVER_DT;
    newVelocity[1] = vertical + ((velocity[1] * unitScale) - player->velocity[1]);
}
