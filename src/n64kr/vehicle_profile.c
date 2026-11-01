#include <libultraship.h>
#include <defines.h>

#include "main.h"
#include "n64kr/vehicle_profile.h"

static const N64krVehicleProfile sProfiles[N64KR_PROFILE_COUNT] = {
    { "Mario", MARIO }, { "Luigi", LUIGI }, { "Yoshi", YOSHI },   { "Toad", TOAD },
    { "DK", DK },       { "Wario", WARIO }, { "Peach", PEACH },   { "Bowser", BOWSER },
};

// -1 = not assigned yet.
static s32 sPlayerProfileId[N64KR_NUM_PLAYERS] = { -1, -1, -1, -1, -1, -1, -1, -1 };

static s32 player_index(const Player* player) {
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
    s32 index = player_index(player);
    s32 profileId = player->characterId;

    if (index < 0) {
        return;
    }
    if (index == 0) {
        s32 override = CVarGetInteger(N64KR_CVAR_P1_PROFILE, 0) - 1;
        if (is_valid_profile(override)) {
            profileId = override;
        }
    }
    sPlayerProfileId[index] = profileId;
}

s32 n64kr_vehicle_profile_id(const Player* player) {
    s32 index = player_index(player);

    if ((index < 0) || !is_valid_profile(sPlayerProfileId[index])) {
        return player->characterId;
    }
    return sPlayerProfileId[index];
}

s32 n64kr_stats_id(const Player* player) {
    s32 profileId = n64kr_vehicle_profile_id(player);

    if (!is_valid_profile(profileId)) {
        return player->characterId;
    }
    return sProfiles[profileId].statsId;
}
