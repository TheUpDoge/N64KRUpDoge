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
    N64KR_MOVEMENT_COUNT
} N64krMovementModel;

typedef void (*N64krMovementFunc)(Player* player, Camera* camera, s8 screenId, s8 playerId);

void n64kr_movement_update(Player* player, Camera* camera, s8 screenId, s8 playerId);

#ifdef __cplusplus
}
#endif

#endif // N64KR_MOVEMENT_H
