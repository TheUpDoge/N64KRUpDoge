#ifndef N64KR_HOVER_H
#define N64KR_HOVER_H

// N64KR: hovercraft velocity stage, ported from the movement-only parts of
// DKR's func_80046524 (src/racer.c:1560-2270 in the DKR decomp).
// See recon/004-hover/RECON-004_Hover_Port_Map.md.
//
// It replaces MK64's force step inside func_8002D268 and leaves MK64's
// steering, collision, walls, floors and landing in charge.

#include <common_structs.h>

#ifdef __cplusplus
extern "C" {
#endif

// CVar: which DKR character's hovercraft stats to use (DKR Character enum,
// 0 = Krunch ... 9 = Diddy). Only matters once real stats are loaded.
#define N64KR_CVAR_HOVER_CHARACTER "gN64KR.HoverDkrCharacter"

// True once n64kr_dkr_hover.txt (extracted from the user's DKR ROM) has
// loaded; otherwise hover runs on placeholder stats.
bool n64kr_hover_has_dkr_stats(void);

// Clears a player's throttle/brake ramps. Called when a profile is assigned.
void n64kr_hover_reset(s32 playerIndex);

// Overwrites newVelocity (world space, MK64 units per 60 Hz tick).
void n64kr_hover_velocity_stage(Player* player, s32 playerIndex, u16 buttons, Vec3f newVelocity);

#ifdef __cplusplus
}
#endif

#endif // N64KR_HOVER_H
