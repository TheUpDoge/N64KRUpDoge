#ifndef N64KR_RACER3D_H
#define N64KR_RACER3D_H

// N64KR: racers drawn as real-time 3D models instead of MK64's pre-rendered
// kart sprites, for characters with a registry model3d (src/n64kr/character.c).
//
// The model is a set of display lists from a mod, one per driving pose
// (tools/n64kr/dkr_racer_to_model.py): model space faces +Z, Y up, tyres on
// y = 0, in source-model units. It is drawn at the kart's visual heading,
// rotation[1] + unk_0C0 (the same angle MK64 uses to pick sprite frames,
// player_controller.c func_8002934C); the driver's pose follows unk_0C0, the
// visual turn. Characters without a model, or with the CVar off, keep their
// sprites.
//
// CVar gN64KR.Racers3D (default 1): 0 = sprites for everyone.

#include <common_structs.h>

#ifdef __cplusplus
extern "C" {
#endif

// From render_kart (src/render_player.c), after the sprite position is known.
// Returns 1 if the racer was drawn as a 3D model (skip the sprite), 0 otherwise.
s32 n64kr_racer3d_draw(Player* player, s8 playerId, s8 screenId, Vec3f position);

#ifdef __cplusplus
}
#endif

#endif // N64KR_RACER3D_H
