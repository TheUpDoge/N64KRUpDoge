#include <libultraship.h>
#include <defines.h>
#include <stdio.h>

#include "main.h"
#include "render_player.h"
#include "racing/math_util.h"
#include "engine/Matrix.h"
#include "n64kr/character.h"
#include "n64kr/racer3d.h"

#define POSE_COUNT 9     // dkr_racer_to_model.py POSE_FRAMES: full right .. straight .. full left
#define POSE_STRAIGHT 4
// World units per model unit (dkr_racer_to_model.py WORLD_SCALE: the 18 x 18
// unit sprite quad and the SLICE-9 sprite fit).
#define MODEL_SCALE 0.12f
// Largest visual turn MK64 gives a kart: unk_0C0 is clamped to +-0x1C70
// (player_controller.c func_8002AE38).
#define FULL_TURN 0x1C70

static char sPosePaths[N64KR_MAX_CHARACTERS][POSE_COUNT][96];

static Gfx* pose_display_list(s32 characterId, const char* model, s32 pose) {
    char* path = sPosePaths[characterId][pose];
    if (path[0] == '\0') {
        snprintf(path, sizeof(sPosePaths[0][0]), "%s/pose%d", model, (int) pose);
    }
    return (Gfx*) ResourceGetDataByName(path);
}

s32 n64kr_racer3d_draw(Player* player, s8 playerId, s8 screenId, Vec3f position) {
    const N64krCharacter* c = n64kr_get_character(player->characterId);
    Gfx* dl;
    Mat4 mtx;
    Vec3s orientation;
    s32 pose;
    f32 turn;

    if ((c == NULL) || (c->model3d == NULL) || (player->characterId >= N64KR_MAX_CHARACTERS) ||
        !CVarGetInteger("gN64KR.Racers3D", 1)) {
        return 0;
    }

    // Visual turn -> pose: positive unk_0C0 turns the nose toward -X, the
    // kart's right (forward is (-sin yaw, cos yaw)); poses run right .. left.
    turn = (f32) player->unk_0C0 / FULL_TURN;
    turn = (turn > 1.0f) ? 1.0f : ((turn < -1.0f) ? -1.0f : turn);
    pose = POSE_STRAIGHT - (s32) (turn * POSE_STRAIGHT + ((turn >= 0.0f) ? 0.5f : -0.5f));
    if (pose < 0) {
        pose = 0;
    } else if (pose >= POSE_COUNT) {
        pose = POSE_COUNT - 1;
    }

    dl = pose_display_list(player->characterId, c->model3d, pose);
    if (dl == NULL) {
        return 0; // model not installed: keep the sprite
    }

    // Model faces +Z; mtxf_translate_rotate maps +Z to (sin y, cos y), and the
    // kart's forward is (-sin heading, cos heading): y = -heading.
    orientation[0] = 0;
    orientation[1] = (s16) -(player->rotation[1] + player->unk_0C0);
    orientation[2] = 0;
    mtxf_translate_rotate(mtx, position, orientation);
    mtxf_scale(mtx, MODEL_SCALE * gCharacterSize[n64kr_char_body(player->characterId)] * player->size);
    convert_to_fixed_point_matrix(GetKartMatrix(playerId + (screenId * 8)), mtx);

    gSPMatrix(gDisplayListHead++, GetKartMatrix(playerId + (screenId * 8)), G_MTX_NOPUSH | G_MTX_LOAD | G_MTX_MODELVIEW);
    gSPDisplayList(gDisplayListHead++, dl);
    gDPPipeSync(gDisplayListHead++);
    gSPTexture(gDisplayListHead++, 1, 1, 0, G_TX_RENDERTILE, G_OFF);
    return 1;
}
