#include <libultraship.h>
#include <defines.h>

#include "main.h"
#include "effects.h"
#include "render_player.h"
#include "racing/math_util.h"
#include "engine/Matrix.h"
#include "n64kr/character.h"
#include "n64kr/racer3d.h"

// Largest visual turn MK64 gives a kart: unk_0C0 is clamped to +-0x1C70
// (player_controller.c func_8002AE38).
#define FULL_TURN 0x1C70
// Front wheel lock at full turn: DKR turns them by stick x * 100, stick up to
// ~70 (src/racer.c), i.e. 7000 / 65536 of a turn.
#define WHEEL_LOCK 7000
#define BOOST_ALPHA 0x40 // func_80025DE8's afterimage alpha

// Render modes from render_kart (src/render_player.c): the translucent one is
// what MK64 uses for Boo and the Lakitu fizzle.
#define RM_TRANSLUCENT_1                                                                                         \
    (AA_EN | Z_CMP | Z_UPD | IM_RD | CVG_DST_WRAP | ZMODE_XLU | CVG_X_ALPHA | FORCE_BL |                           \
     GBL_c1(G_BL_CLR_IN, G_BL_A_IN, G_BL_CLR_MEM, G_BL_1MA))
#define RM_TRANSLUCENT_2                                                                                         \
    (AA_EN | Z_CMP | Z_UPD | IM_RD | CVG_DST_WRAP | ZMODE_XLU | CVG_X_ALPHA | FORCE_BL |                           \
     GBL_c2(G_BL_CLR_IN, G_BL_A_IN, G_BL_CLR_MEM, G_BL_1MA))

// Front wheel matrices (model space, multiplied onto the kart matrix): one set
// for the kart, one for its boost afterimage, per player per screen.
static Mtx sWheelMtx[2][NUM_PLAYERS * 4][N64KR_RACER3D_MAX_WHEELS];

static const N64krRacerModel* model_for(const Player* player) {
    const N64krCharacter* c = n64kr_get_character(player->characterId);
    if ((c == NULL) || (c->model3d == NULL) || !CVarGetInteger("gN64KR.Racers3D", 1)) {
        return NULL;
    }
    return n64kr_racer3d_model(c->model3d);
}

static f32 visual_turn(const Player* player) {
    f32 turn = (f32) player->unk_0C0 / FULL_TURN;
    return (turn > 1.0f) ? 1.0f : ((turn < -1.0f) ? -1.0f : turn);
}

// The kart's matrix: model faces +Z; mtxf_translate_rotate maps +Z to
// (sin y, cos y), and the kart's forward is (-sin heading, cos heading).
static void kart_matrix(Mat4 mtx, const N64krRacerModel* model, const Player* player, Vec3f position) {
    Vec3s orientation;

    orientation[0] = 0;
    orientation[1] = (s16) -(player->rotation[1] + player->unk_0C0);
    orientation[2] = 0;
    mtxf_translate_rotate(mtx, position, orientation);
    mtxf_scale(mtx, model->scale * gCharacterSize[n64kr_char_body(player->characterId)] * player->size);
}

// Body pose and front wheels, under the kart matrix already loaded.
static void draw_model(const N64krRacerModel* model, const Player* player, s32 slot, s32 set, u8 alpha,
                       s32 translucent) {
    Gfx* dl;
    f32 turn = visual_turn(player);
    s32 pose;
    s32 k;

    // Poses run full right .. straight .. full left; positive unk_0C0 turns
    // the nose toward -X, the kart's right.
    pose = model->straightPose - (s32) (turn * model->straightPose + ((turn >= 0.0f) ? 0.5f : -0.5f));
    pose = (pose < 0) ? 0 : ((pose >= model->poseCount) ? model->poseCount - 1 : pose);

    gDPPipeSync(gDisplayListHead++);
    gDPSetPrimColor(gDisplayListHead++, 0, 0, 255, 255, 255, alpha);
    if (translucent) {
        gDPSetAlphaCompare(gDisplayListHead++, G_AC_NONE);
        gDPSetRenderMode(gDisplayListHead++, RM_TRANSLUCENT_1, RM_TRANSLUCENT_2);
    } else {
        gDPSetRenderMode(gDisplayListHead++, G_RM_AA_ZB_TEX_EDGE, G_RM_AA_ZB_TEX_EDGE2);
    }

    dl = (Gfx*) ResourceGetDataByName(model->poseDl[pose]);
    if (dl != NULL) {
        gSPDisplayList(gDisplayListHead++, dl);
    }

    // Front wheels: steer about their own centre (y rotation, the
    // mtxf_translate_rotate convention), same sign as the body turn.
    for (k = 0; k < model->frontWheelCount; k++) {
        Mat4 m = { { 0 } };
        f32 s = sins((u16) (s16) -(turn * WHEEL_LOCK));
        f32 c = coss((u16) (s16) -(turn * WHEEL_LOCK));

        dl = (Gfx*) ResourceGetDataByName(model->frontWheelDl[k]);
        if (dl == NULL) {
            continue;
        }
        m[0][0] = c;
        m[0][2] = -s;
        m[1][1] = 1.0f;
        m[2][0] = s;
        m[2][2] = c;
        m[3][0] = model->frontWheelCentre[k][0];
        m[3][1] = model->frontWheelCentre[k][1];
        m[3][2] = model->frontWheelCentre[k][2];
        m[3][3] = 1.0f;
        convert_to_fixed_point_matrix(&sWheelMtx[set][slot][k], m);
        gSPMatrix(gDisplayListHead++, &sWheelMtx[set][slot][k], G_MTX_PUSH | G_MTX_MUL | G_MTX_MODELVIEW);
        gSPDisplayList(gDisplayListHead++, dl);
        gSPPopMatrix(gDisplayListHead++, G_MTX_MODELVIEW);
    }

    gDPPipeSync(gDisplayListHead++);
    gSPTexture(gDisplayListHead++, 1, 1, 0, G_TX_RENDERTILE, G_OFF);
    gDPSetAlphaCompare(gDisplayListHead++, G_AC_NONE);
}

s32 n64kr_racer3d_draw(Player* player, s8 playerId, s8 screenId, Vec3f position) {
    const N64krRacerModel* model = model_for(player);
    s32 slot = playerId + (screenId * 8);
    Mat4 mtx;
    u8 alpha = 255;
    s32 translucent = false;

    if ((model == NULL) || (slot < 0) || (slot >= NUM_PLAYERS * 4)) {
        return 0;
    }
    // Transparency as render_kart: Boo (own screen sees player->alpha, other
    // screens a fainter one), Lakitu fizzle, battle bomb / loss.
    if ((player->effects & BOO_EFFECT) == BOO_EFFECT) {
        alpha = (u8) ((screenId == playerId) ? player->alpha : gPlayerOtherScreensAlpha[playerId]);
        translucent = true;
    } else if (((player->lakituProps & LAKITU_FIZZLE) == LAKITU_FIZZLE) || (player->triggers & BECOME_BOMB_EFFECT) ||
               (player->triggers & LOSE_BATTLE_EFFECT)) {
        alpha = (u8) player->alpha;
        translucent = true;
    }

    kart_matrix(mtx, model, player, position);
    convert_to_fixed_point_matrix(GetKartMatrix(slot), mtx);
    gSPMatrix(gDisplayListHead++, GetKartMatrix(slot), G_MTX_NOPUSH | G_MTX_LOAD | G_MTX_MODELVIEW);
    draw_model(model, player, slot, 0, alpha, translucent);
    return 1;
}

s32 n64kr_racer3d_draw_boost(Player* player, s8 playerId, s8 screenId, Vec3f position) {
    const N64krRacerModel* model = model_for(player);
    s32 slot = playerId + (screenId * 8);
    Mat4 mtx;

    if ((model == NULL) || (slot < 0) || (slot >= NUM_PLAYERS * 4)) {
        return 0;
    }
    kart_matrix(mtx, model, player, position);
    AddEffectMatrix(mtx, G_MTX_NOPUSH | G_MTX_LOAD | G_MTX_MODELVIEW);
    draw_model(model, player, slot, 1, BOOST_ALPHA, true);
    gMatrixEffectCount += 1;
    return 1;
}
