#include <libultraship.h>
#include <defines.h>

#include "player_controller.h"
#include "n64kr/movement.h"
#include "n64kr/vehicle_profile.h"

// Per-frame multiplier on downward velocity for the floaty test model.
#define FLOATY_FALL_DAMPING 0.85f

static void movement_ground(Player* player, Camera* camera, s8 screenId, s8 playerId) {
    func_8002D268(player, camera, screenId, playerId);
}

static void movement_test_floaty(Player* player, Camera* camera, s8 screenId, s8 playerId) {
    func_8002D268(player, camera, screenId, playerId);
    if (player->velocity[1] < 0.0f) {
        player->velocity[1] *= FLOATY_FALL_DAMPING;
    }
}

static const N64krMovementFunc sMovementFuncs[N64KR_MOVEMENT_COUNT] = {
    movement_ground,      // N64KR_MOVEMENT_GROUND
    movement_test_floaty, // N64KR_MOVEMENT_TEST_FLOATY
};

void n64kr_movement_update(Player* player, Camera* camera, s8 screenId, s8 playerId) {
    s32 model = n64kr_movement_model(player);

    if ((model < 0) || (model >= N64KR_MOVEMENT_COUNT)) {
        model = N64KR_MOVEMENT_GROUND;
    }
    sMovementFuncs[model](player, camera, screenId, playerId);
}
