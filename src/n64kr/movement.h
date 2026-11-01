#ifndef N64KR_MOVEMENT_H
#define N64KR_MOVEMENT_H

// N64KR: movement models. A vehicle profile picks one, and upstream calls
// n64kr_movement_update() where it used to call func_8002D268 (normal
// driving physics).
//
// MK64 still picks *state* physics itself (func_8002E594 when hit or spun,
// control_cpu_movement for off-screen CPUs). Only normal driving is routed.

#include <common_structs.h>
#include "camera.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef enum {
    // Stock MK64 driving: func_8002D268.
    N64KR_MOVEMENT_GROUND,
    // SLICE-2 test model: ground physics, then falls are damped so jumps
    // float. Proves the switch; not a real vehicle.
    N64KR_MOVEMENT_TEST_FLOATY,
    // SLICE-3: func_8002D268 with its force step replaced by DKR hovercraft
    // physics (n64kr/hover.h).
    N64KR_MOVEMENT_HOVER,
    N64KR_MOVEMENT_COUNT
} N64krMovementModel;

typedef void (*N64krMovementFunc)(Player* player, Camera* camera, s8 screenId, s8 playerId);

void n64kr_movement_update(Player* player, Camera* camera, s8 screenId, s8 playerId);

// Called inside func_8002D268 once MK64 has computed newVelocity, before
// collision. GROUND leaves it alone; HOVER replaces it.
void n64kr_movement_velocity_stage(Player* player, s8 playerId, Vec3f newVelocity);

// Records the controller MK64 is driving a player with this tick, from
// handle_a_press_for_player_during_race. Covers Time Trial replays too.
void n64kr_record_input(Player* player, struct Controller* controller);

#ifdef __cplusplus
}
#endif

#endif // N64KR_MOVEMENT_H
