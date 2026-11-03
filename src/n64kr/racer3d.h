#ifndef N64KR_RACER3D_H
#define N64KR_RACER3D_H

// N64KR: racers drawn as real-time 3D models instead of MK64's pre-rendered
// kart sprites, for characters with a registry model3d (src/n64kr/character.c).
//
// The model comes from a mod (tools/n64kr/dkr_racer_to_model.py): one display
// list per driving pose (the body), one per wheel built around its centre, and
// model.json (pose count, scale, wheel centres, which wheels steer). Model
// space faces +Z, Y up, tyres on y = 0, in source-model units. It is drawn at
// the kart's visual heading, rotation[1] + unk_0C0 (the angle MK64 picks
// sprite frames by, player_controller.c func_8002934C); the driver's pose and
// the front wheels follow unk_0C0, the visual turn; wheels roll with
// tyreSpeed (the counter MK64 animates sprite wheels with).
//
// The model's materials (2-cycle) take alpha from texel x primitive alpha and
// apply MK64's racer colour effect in cycle 2 ((1 - env) x colour + prim, as
// func_8004B614 does for sprites), and set no render mode: the engine sets the
// effect colours, alpha and an opaque or translucent mode (Boo, Lakitu fizzle,
// the boost afterimage) the way render_kart does for sprites.
//
// CVar gN64KR.Racers3D (default 1): 0 = sprites for everyone.

#include <common_structs.h>

#ifdef __cplusplus
extern "C" {
#endif

#define N64KR_RACER3D_MAX_POSES 16
#define N64KR_RACER3D_MAX_WHEELS 4

typedef struct {
    s32 ok;
    f32 scale;        // world units per model unit
    s32 poseCount;
    s32 straightPose;
    const char* poseDl[N64KR_RACER3D_MAX_POSES];
    s32 wheelCount;
    const char* wheelDl[N64KR_RACER3D_MAX_WHEELS];
    f32 wheelCentre[N64KR_RACER3D_MAX_WHEELS][3];
    s32 wheelSteers[N64KR_RACER3D_MAX_WHEELS];
} N64krRacerModel;

// Model info for a model3d base path (e.g. "racers/diddy"); NULL if the mod
// doesn't have it. Cached. (racer3d_model.cpp)
const N64krRacerModel* n64kr_racer3d_model(const char* base);

// From render_players (src/render_player.c): the camera this screen's racers
// are drawn for. Models are pushed toward it like MK64's kart sprites
// (racer3d.c kart_matrix).
void n64kr_racer3d_set_camera(Vec3f cameraPos);

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
