#ifndef N64KR_RACER3D_H
#define N64KR_RACER3D_H

// N64KR: racers drawn as real-time 3D models instead of MK64's pre-rendered
// kart sprites, for characters with a registry model3d (src/n64kr/character.c).
//
// The model comes from a mod (tools/n64kr/dkr_racer_to_model.py): one display
// list per driving pose (body + rear wheels), one per front wheel built around
// its centre, and model.json (pose count, scale, front wheel centres). Model
// space faces +Z, Y up, tyres on y = 0, in source-model units. It is drawn at
// the kart's visual heading, rotation[1] + unk_0C0 (the angle MK64 picks
// sprite frames by, player_controller.c func_8002934C); the driver's pose and
// the front wheels follow unk_0C0, the visual turn.
//
// The model's materials take their alpha from texel x primitive alpha and set
// no render mode, so the engine can draw them opaque or translucent (Boo,
// Lakitu fizzle, the boost afterimage) the way render_kart does for sprites.
//
// CVar gN64KR.Racers3D (default 1): 0 = sprites for everyone.

#include <common_structs.h>

#ifdef __cplusplus
extern "C" {
#endif

#define N64KR_RACER3D_MAX_POSES 16
#define N64KR_RACER3D_MAX_WHEELS 2

typedef struct {
    s32 ok;
    f32 scale;        // world units per model unit
    s32 poseCount;
    s32 straightPose;
    const char* poseDl[N64KR_RACER3D_MAX_POSES];
    s32 frontWheelCount;
    const char* frontWheelDl[N64KR_RACER3D_MAX_WHEELS];
    f32 frontWheelCentre[N64KR_RACER3D_MAX_WHEELS][3];
} N64krRacerModel;

// Model info for a model3d base path (e.g. "racers/diddy"); NULL if the mod
// doesn't have it. Cached. (racer3d_model.cpp)
const N64krRacerModel* n64kr_racer3d_model(const char* base);

// From render_kart (src/render_player.c), after the sprite position is known.
// Returns 1 if the racer was drawn as a 3D model (skip the sprite), 0 otherwise.
s32 n64kr_racer3d_draw(Player* player, s8 playerId, s8 screenId, Vec3f position);

// From func_80025DE8 (the boost afterimage, src/render_player.c): the faint
// copy behind a boosting kart. Returns 1 if drawn as a 3D model.
s32 n64kr_racer3d_draw_boost(Player* player, s8 playerId, s8 screenId, Vec3f position);

#ifdef __cplusplus
}
#endif

#endif // N64KR_RACER3D_H
