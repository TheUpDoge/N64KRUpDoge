#include <libultraship.h>
#include <defines.h>

#include "main.h"
#include "menus.h"
#include "n64kr/hover.h"
#include "n64kr/movement.h"
#include "n64kr/vehicle_profile.h"
#include "n64kr/vehicle_select.h"

static const N64krVehicleProfile sProfiles[N64KR_PROFILE_COUNT] = {
    { "Mario", MARIO, N64KR_MOVEMENT_GROUND },
    { "Luigi", LUIGI, N64KR_MOVEMENT_GROUND },
    { "Yoshi", YOSHI, N64KR_MOVEMENT_GROUND },
    { "Toad", TOAD, N64KR_MOVEMENT_GROUND },
    { "DK", DK, N64KR_MOVEMENT_GROUND },
    { "Wario", WARIO, N64KR_MOVEMENT_GROUND },
    { "Peach", PEACH, N64KR_MOVEMENT_GROUND },
    { "Bowser", BOWSER, N64KR_MOVEMENT_GROUND },
    { "Floaty test (Mario stats)", MARIO, N64KR_MOVEMENT_TEST_FLOATY },
    { "Hovercraft (DKR)", N64KR_STATS_OWN_CHARACTER, N64KR_MOVEMENT_HOVER },
};

// -1 = not assigned yet.
static s32 sPlayerProfileId[N64KR_NUM_PLAYERS] = { -1, -1, -1, -1, -1, -1, -1, -1 };

s32 n64kr_player_index(const Player* player) {
    ptrdiff_t index = player - gPlayers;

    if ((index < 0) || (index >= N64KR_NUM_PLAYERS)) {
        return -1;
    }
    return (s32) index;
}

static bool is_valid_profile(s32 profileId) {
    return (profileId >= 0) && (profileId < N64KR_PROFILE_COUNT);
}

const N64krVehicleProfile* n64kr_get_vehicle_profile(s32 profileId) {
    if (!is_valid_profile(profileId)) {
        return NULL;
    }
    return &sProfiles[profileId];
}

void n64kr_assign_vehicle_profile(Player* player) {
    s32 index = n64kr_player_index(player);
    s32 profileId = player->characterId;

    if (index < 0) {
        return;
    }
    // Human players: vehicle picked on the character select screen.
    if ((index < gPlayerCount) && (n64kr_vehicle_choice(index) == N64KR_VEHICLE_HOVERCRAFT)) {
        profileId = N64KR_PROFILE_HOVERCRAFT;
    }
    // Debug override from the N64KR menu wins for Player 1.
    if (index == 0) {
        s32 override = CVarGetInteger(N64KR_CVAR_P1_PROFILE, 0) - 1;
        if (is_valid_profile(override)) {
            profileId = override;
        }
    }
    sPlayerProfileId[index] = profileId;
    n64kr_hover_reset(index);
}

s32 n64kr_vehicle_profile_id(const Player* player) {
    s32 index = n64kr_player_index(player);

    if ((index < 0) || !is_valid_profile(sPlayerProfileId[index])) {
        return player->characterId;
    }
    return sPlayerProfileId[index];
}

s32 n64kr_stats_id(const Player* player) {
    s32 profileId = n64kr_vehicle_profile_id(player);

    if (!is_valid_profile(profileId) || (sProfiles[profileId].statsId == N64KR_STATS_OWN_CHARACTER)) {
        return player->characterId;
    }
    return sProfiles[profileId].statsId;
}

s32 n64kr_movement_model(const Player* player) {
    s32 profileId = n64kr_vehicle_profile_id(player);

    if (!is_valid_profile(profileId)) {
        return N64KR_MOVEMENT_GROUND;
    }
    return sProfiles[profileId].movementModel;
}
