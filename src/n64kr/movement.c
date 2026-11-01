#include <libultraship.h>
#include <macros.h>
#include <defines.h>

#include "player_controller.h"
#include "n64kr/hover.h"
#include "n64kr/movement.h"
#include "n64kr/vehicle_profile.h"

// Per-frame multiplier on downward velocity for the floaty test model.
#define FLOATY_FALL_DAMPING 0.85f

// Held buttons from the controller driving each player this tick.
static u16 sPlayerButtons[N64KR_NUM_PLAYERS];

static void movement_ground(Player* player, Camera* camera, s8 screenId, s8 playerId) {
    func_8002D268(player, camera, screenId, playerId);
}

static void movement_test_floaty(Player* player, Camera* camera, s8 screenId, s8 playerId) {
    func_8002D268(player, camera, screenId, playerId);
    if (player->velocity[1] < 0.0f) {
        player->velocity[1] *= FLOATY_FALL_DAMPING;
    }
}

// The hover force step runs from n64kr_movement_velocity_stage(), inside
// func_8002D268.
static void movement_hover(Player* player, Camera* camera, s8 screenId, s8 playerId) {
    func_8002D268(player, camera, screenId, playerId);
}

static const N64krMovementFunc sMovementFuncs[N64KR_MOVEMENT_COUNT] = {
    movement_ground,      // N64KR_MOVEMENT_GROUND
    movement_test_floaty, // N64KR_MOVEMENT_TEST_FLOATY
    movement_hover,       // N64KR_MOVEMENT_HOVER
};

void n64kr_movement_update(Player* player, Camera* camera, s8 screenId, s8 playerId) {
    s32 model = n64kr_movement_model(player);

    if ((model < 0) || (model >= N64KR_MOVEMENT_COUNT)) {
        model = N64KR_MOVEMENT_GROUND;
    }
    sMovementFuncs[model](player, camera, screenId, playerId);
}

void n64kr_movement_velocity_stage(Player* player, UNUSED s8 playerId, Vec3f newVelocity) {
    s32 index;

    if (n64kr_movement_model(player) != N64KR_MOVEMENT_HOVER) {
        return;
    }
    index = n64kr_player_index(player);
    if (index < 0) {
        return;
    }
    n64kr_hover_velocity_stage(player, index, sPlayerButtons[index], newVelocity);
}

void n64kr_record_input(Player* player, struct Controller* controller) {
    s32 index = n64kr_player_index(player);

    if ((index >= 0) && (controller != NULL)) {
        sPlayerButtons[index] = controller->button;
    }
}
