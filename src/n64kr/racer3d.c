#include <libultraship.h>
#include <defines.h>
#include <math.h>

#include "main.h"
#include "effects.h"
#include "render_player.h"
#include "racing/math_util.h"
#include "engine/Matrix.h"
#include "engine/tracks/Track.h"
#include "port/Game.h"
#include "spawn_players.h"
#include "n64kr/character.h"
#include "n64kr/racer3d.h"

// Largest visual turn MK64 gives a kart: unk_0C0 is clamped to +-0x1C70
// (player_controller.c func_8002AE38).
#define FULL_TURN 0x1C70
// Front wheel lock: DKR turns them by stick x * 100, stick up to ~70
// (src/racer.c), i.e. 7000 / 65536 of a turn. The visual turn rarely nears its
// clamp in normal driving (front wheels barely moved in the user's test), so
// it reaches full lock at a third of it.
#define WHEEL_LOCK 7000
#define WHEEL_LOCK_AT_TURN (1.0f / 3.0f)
// Wheel roll: tyreSpeed counts 0..0x3FF per sprite spin cycle (kart_dma.c,
// wheel frame = tyreSpeed >> 8). One cycle = one wheel turn rolled twice too
// fast (user playtest), so one cycle = half a turn.
#define WHEEL_ROLL_SHIFT 5
#define BOOST_ALPHA 0x40 // func_80025DE8's afterimage alpha
#define SPRITE_DEPTH 6.0f // kart sprite quad's distance toward the camera (gPlayerOneVtx z = -6)

// Render modes (the model's materials are 2-cycle; the first cycle passes
// through). The translucent one is render_kart's for Boo and the Lakitu fizzle.
#define RM_TRANSLUCENT_2                                                                                         \
    (AA_EN | Z_CMP | Z_UPD | IM_RD | CVG_DST_WRAP | ZMODE_XLU | CVG_X_ALPHA | FORCE_BL |                           \
     GBL_c2(G_BL_CLR_IN, G_BL_A_IN, G_BL_CLR_MEM, G_BL_1MA))

// Wheel matrices (model space, multiplied onto the kart matrix): one set for
// the kart, one for its boost afterimage, per player per screen.
static Mtx sWheelMtx[2][NUM_PLAYERS * 4][N64KR_RACER3D_MAX_WHEELS];
// Accumulated wheel roll per player: tyreSpeed wraps every 0x400, which at
// half a turn per cycle would snap the wheel back; add its change instead.
static u16 sWheelRoll[NUM_PLAYERS];
static s16 sLastTyreSpeed[NUM_PLAYERS];
// Camera the racers are being drawn for (n64kr_racer3d_set_camera).
static Vec3f sCameraPos;
static s32 sCameraSet = false;

static u16 wheel_roll(const Player* player, s32 playerId) {
    if ((playerId < 0) || (playerId >= NUM_PLAYERS)) {
        return 0;
    }
    // Same value on every screen in a frame: the second draw adds nothing.
    sWheelRoll[playerId] += (u16) (((player->tyreSpeed - sLastTyreSpeed[playerId]) & 0x3FF) << WHEEL_ROLL_SHIFT);
    sLastTyreSpeed[playerId] = player->tyreSpeed;
    return sWheelRoll[playerId];
}

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

// MK64 draws a racer only inside a triangle ahead of the camera
// (check_player_camera_collision, render_player.c): far corners D_80165578
// units out at +-var_v0 of the camera's yaw (1P 0x293C = 58 deg, 2P 0x3058 =
// 68 deg, 3P/4P 0x1FFE = 45 deg), so straight ahead it ends at D_80165578 x
// cos(angle): 1200 x 0.53 = about 640 units in a 1P race (func_80039AE4).
// N64KR's tracks are drawn to 6800 (FarPersp) and much longer than MK64's, so
// racers vanished well ahead of the player (user playtest). Grow the triangle
// until it reaches the far plane straight ahead; never shrink MK64's.
void n64kr_racer_draw_distance(void) {
    Properties* props = CM_GetProps();
    u16 angle;
    f32 distance;

    if (!CVarGetInteger("gN64KR.FarRacers", 1) || (props == NULL)) {
        return;
    }
    switch (gActiveScreenMode) {
        case SCREEN_MODE_1P:
            angle = 0x293C;
            break;
        case SCREEN_MODE_2P_SPLITSCREEN_HORIZONTAL:
        case SCREEN_MODE_2P_SPLITSCREEN_VERTICAL:
            angle = 0x3058;
            break;
        default:
            angle = 0x1FFE;
            break;
    }
    distance = props->FarPersp / coss(angle);
    distance = (distance > 32000.0f) ? 32000.0f : distance;
    if (distance > D_80165578) {
        D_80165578 = (s16) distance;
    }
}

void n64kr_racer3d_set_camera(Vec3f cameraPos) {
    sCameraPos[0] = cameraPos[0];
    sCameraPos[1] = cameraPos[1];
    sCameraPos[2] = cameraPos[2];
    sCameraSet = true;
}

// The kart's matrix: model faces +Z; mtxf_translate_rotate maps +Z to
// (sin y, cos y), and the kart's forward is (-sin heading, cos heading).
//
// MK64 draws a kart sprite SPRITE_DEPTH units (x racer size) in front of the
// kart, toward the camera: gPlayerOneVtx has z = -6, and the sprite is turned
// by unk_048 = atan2s(kart - camera) (player_controller.c func_8002934C), so
// local -z points at the camera [CONCLUSION from source]. A sprite therefore
// wins the depth test against anything up to that far in front of its kart,
// and covered 3D racers just ahead of it (user playtest: "layer error"). The
// model is moved the same distance toward the camera and shrunk by the same
// perspective, (d - shift) / d, so it looks the same on screen but depth-sorts
// against sprites the way sprites do against each other.
static void kart_matrix(Mat4 mtx, const N64krRacerModel* model, const Player* player, Vec3f position) {
    Vec3s orientation;
    Vec3f at;
    f32 size = gCharacterSize[n64kr_char_body(player->characterId)] * player->size;
    f32 scale = model->scale * size;
    f32 shift = SPRITE_DEPTH * size;
    f32 dx = sCameraPos[0] - position[0];
    f32 dy = sCameraPos[1] - position[1];
    f32 dz = sCameraPos[2] - position[2];
    f32 d = sqrtf((dx * dx) + (dy * dy) + (dz * dz));

    at[0] = position[0];
    at[1] = position[1];
    at[2] = position[2];
    if (sCameraSet && (d > shift * 2.0f)) {
        at[0] += dx * (shift / d);
        at[1] += dy * (shift / d);
        at[2] += dz * (shift / d);
        scale *= (d - shift) / d;
    }
    orientation[0] = 0;
    orientation[1] = (s16) -(player->rotation[1] + player->unk_0C0);
    orientation[2] = 0;
    mtxf_translate_rotate(mtx, at, orientation);
    mtxf_scale(mtx, scale);
}

// Body pose and wheels, under the kart matrix already loaded.
static void draw_model(const N64krRacerModel* model, const Player* player, s32 playerId, s32 slot, s32 set,
                       u8 alpha, s32 translucent) {
    Gfx* dl;
    f32 turn = visual_turn(player);
    f32 lock = turn / WHEEL_LOCK_AT_TURN;
    s32 pose;
    s32 k;
    // Roll about the axle (x): rows of the row-vector rotation (N64 matrices
    // multiply v * M), positive = the tyre top moving forward (+Z).
    u16 roll = wheel_roll(player, playerId);
    f32 rs = sins(roll);
    f32 rc = coss(roll);

    lock = (lock > 1.0f) ? 1.0f : ((lock < -1.0f) ? -1.0f : lock);

    // Poses run full right .. straight .. full left; positive unk_0C0 turns
    // the nose toward -X, the kart's right.
    pose = model->straightPose - (s32) (turn * model->straightPose + ((turn >= 0.0f) ? 0.5f : -0.5f));
    pose = (pose < 0) ? 0 : ((pose >= model->poseCount) ? model->poseCount - 1 : pose);

    // MK64's racer colour effect (func_8004B614): prim = (R, G, B) added, env
    // = (C, M, Y) subtracted, prim alpha = the racer's alpha.
    gDPPipeSync(gDisplayListHead++);
    gDPSetPrimColor(gDisplayListHead++, 0, 0, gPlayerRedEffect[playerId], gPlayerGreenEffect[playerId],
                    gPlayerBlueEffect[playerId], alpha);
    gDPSetEnvColor(gDisplayListHead++, gPlayerCyanEffect[playerId], gPlayerMagentaEffect[playerId],
                   gPlayerYellowEffect[playerId], 0xFF);
    if (translucent) {
        gDPSetAlphaCompare(gDisplayListHead++, G_AC_NONE);
        gDPSetRenderMode(gDisplayListHead++, G_RM_PASS, RM_TRANSLUCENT_2);
    } else {
        gDPSetRenderMode(gDisplayListHead++, G_RM_PASS, G_RM_AA_ZB_TEX_EDGE2);
    }

    dl = (Gfx*) ResourceGetDataByName(model->poseDl[pose]);
    if (dl != NULL) {
        gSPDisplayList(gDisplayListHead++, dl);
    }

    // Wheels: v * roll(x) * steer(y, front wheels; the mtxf_translate_rotate
    // convention, same sign as the body turn) + centre.
    for (k = 0; k < model->wheelCount; k++) {
        Mat4 m = { { 0 } };
        f32 ss = model->wheelSteers[k] ? sins((u16) (s16) -(lock * WHEEL_LOCK)) : 0.0f;
        f32 sc = model->wheelSteers[k] ? coss((u16) (s16) -(lock * WHEEL_LOCK)) : 1.0f;

        dl = (Gfx*) ResourceGetDataByName(model->wheelDl[k]);
        if (dl == NULL) {
            continue;
        }
        // roll = [1 0 0; 0 rc rs; 0 -rs rc], steer = [sc 0 -ss; 0 1 0; ss 0 sc]
        m[0][0] = sc;
        m[0][2] = -ss;
        m[1][0] = rs * ss;
        m[1][1] = rc;
        m[1][2] = rs * sc;
        m[2][0] = rc * ss;
        m[2][1] = -rs;
        m[2][2] = rc * sc;
        m[3][0] = model->wheelCentre[k][0];
        m[3][1] = model->wheelCentre[k][1];
        m[3][2] = model->wheelCentre[k][2];
        m[3][3] = 1.0f;
        convert_to_fixed_point_matrix(&sWheelMtx[set][slot][k], m);
        gSPMatrix(gDisplayListHead++, &sWheelMtx[set][slot][k], G_MTX_PUSH | G_MTX_MUL | G_MTX_MODELVIEW);
        gSPDisplayList(gDisplayListHead++, dl);
        gSPPopMatrix(gDisplayListHead++, G_MTX_MODELVIEW);
    }

    // Leave the state MK64's own racer and shadow drawing expects: their setup
    // lists (common_setting_render_character, D_0D008D10) never set the cycle
    // type, so the model's 2-cycle mode would carry into every racer and
    // shadow drawn after it (they vanished / broke in the user's test).
    gDPPipeSync(gDisplayListHead++);
    gDPSetCycleType(gDisplayListHead++, G_CYC_1CYCLE);
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
    draw_model(model, player, playerId, slot, 0, alpha, translucent);
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
    draw_model(model, player, playerId, slot, 1, BOOST_ALPHA, true);
    gMatrixEffectCount += 1;
    return 1;
}
