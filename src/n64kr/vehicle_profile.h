#ifndef N64KR_VEHICLE_PROFILE_H
#define N64KR_VEHICLE_PROFILE_H

// N64KR: vehicle profiles. The character does not own physics; a profile does.
//
// SLICE-1: a profile picks which stock MK64 kart's stat tables drive the
// physics (gKart*Table, gTopSpeedTable, the per-surface grip tables, the hop
// tables in effects.c). Upstream code reads those tables with
// [n64kr_stats_id(player)] instead of [player->characterId].
// Graphics, sounds and character size stay keyed by characterId.

#include <common_structs.h>

#ifdef __cplusplus
extern "C" {
#endif

#define N64KR_NUM_PLAYERS 8

// Stock profiles, one per MK64 kart. Values match the character ids in
// defines.h (MARIO 0 ... BOWSER 7).
typedef enum {
    N64KR_PROFILE_MARIO,
    N64KR_PROFILE_LUIGI,
    N64KR_PROFILE_YOSHI,
    N64KR_PROFILE_TOAD,
    N64KR_PROFILE_DK,
    N64KR_PROFILE_WARIO,
    N64KR_PROFILE_PEACH,
    N64KR_PROFILE_BOWSER,
    N64KR_PROFILE_COUNT
} N64krVehicleProfileId;

typedef struct {
    const char* name;
    // Index into the MK64 per-kart stat tables (0..7).
    s32 statsId;
} N64krVehicleProfile;

// CVar holding Player 1's profile override: 0 = the character's own kart,
// N = profile N - 1.
#define N64KR_CVAR_P1_PROFILE "gN64KR.P1VehicleProfile"

const N64krVehicleProfile* n64kr_get_vehicle_profile(s32 profileId);

// Called from spawn_player() once characterId is set. gPlayers[0] (Player 1)
// takes the N64KR_CVAR_P1_PROFILE override when it is set.
void n64kr_assign_vehicle_profile(Player* player);

s32 n64kr_vehicle_profile_id(const Player* player);

// Index for MK64's per-kart physics tables. Falls back to characterId for a
// player that was never assigned a profile.
s32 n64kr_stats_id(const Player* player);

#ifdef __cplusplus
}
#endif

#endif // N64KR_VEHICLE_PROFILE_H
