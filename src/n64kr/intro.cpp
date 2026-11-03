// N64KR: the boot intro (see intro.h).

#include <algorithm>
#include <array>
#include <chrono>
#include <cmath>
#include <future>
#include <string>
#include <vector>

#include <libultraship.h>
#include <libultra/gbi.h>
#include <libultra/gu.h>
#include <nlohmann/json.hpp>
#include <spdlog/spdlog.h>
#include <ship/Context.h>
#include "ship/resource/ResourceManager.h"
#include "port/audio/HMAS.h"

#include "n64kr/intro.h"
#include "n64kr/music.h"
#include "n64kr/sfx.h"

extern "C" {
#include <defines.h>
#include "main.h"
#include "menu_items.h"
#include "racing/math_util.h"
}
#include "engine/Matrix.h"

namespace {

using Clock = std::chrono::steady_clock;
constexpr float kPi = 3.14159265f;

// The 2D parts use GoldenEye's own space: a 1280 x 960 orthographic screen, y up
// (title.c guOrtho(matrixBufferGunbarrel0, 0, 1280, 0, 960, ...)).
constexpr float kW = 1280.0f, kH = 960.0f;

// Timeline, in seconds from the start. The gun barrel keeps GoldenEye's NTSC
// timing (title.c renderGunbarrelEyeIntroSequence): the dots move 6 units a
// frame at 60 frames a second, the barrel 5.818 a frame at 30, and Bond's
// clock (gunbarrelTimer) counts 60 ticks a second from when he first shows.
constexpr float kCreditIn = 0.8f, kCreditOut = 4.0f, kCreditEnd = 4.8f;
constexpr float kBarrel = 5.2f;                    // the dots and the music start
constexpr float kDots = 237.0f / 60.0f;            // g_TitleX -30 -> 1390, +6 a frame
constexpr float kSlideFrom = 1276.0f, kSlideTo = -80.0f;
constexpr float kSlideSpeed = 5.8183274f * 30.0f;  // XDEC3 a frame
constexpr float kSlide = (kSlideFrom - kSlideTo) / kSlideSpeed;
constexpr float kBondFrom = 600.0f;                // Bond shows once g_TitleX < 600
constexpr float kHold = 20.0f / 30.0f;
constexpr float kBlood = 2.0f;                     // ours: GoldenEye's blood is an animated image
constexpr float kWobble = 108.0f / 30.0f;
constexpr float kWobbleSpeed = 0x38E / 65536.0f * 2.0f * kPi * 30.0f; // INCVAL a frame, radians a second
constexpr float kFade = 31.0f / 30.0f;
constexpr float kBlack = 0.5f;
constexpr float kNuke = 6.2f;

constexpr float kSlideStart = kBarrel + kDots;
constexpr float kBondStart = kSlideStart + (kSlideFrom - kBondFrom) / kSlideSpeed;
constexpr float kHoldStart = kSlideStart + kSlide;
constexpr float kBloodStart = kHoldStart + kHold;
constexpr float kWobbleStart = kBloodStart + kBlood;
constexpr float kFadeStart = kWobbleStart + kWobble;
constexpr float kNukeStart = kFadeStart + kFade + kBlack;
constexpr float kEnd = kNukeStart + kNuke;

// Bond's clock (title.c sub_GAME_7F007F30): bond_eye_walk from frame 2 at
// 0.91 x play speed 0.5 frames a tick, bond_eye_fire from frame 2 at tick 137,
// speeded up to 1.6 at tick 212, the shot at tick 230.
constexpr float kTicks = 60.0f;
constexpr float kFireTick = 137.0f, kSpeedupTick = 212.0f, kShotTick = 230.0f;
constexpr float kAnimRate = 0.91f * 0.5f, kFastRate = 1.6f * 0.5f;
constexpr float kFlashTicks = 6.0f; // the muzzle flash shows for 0.1 s

// Bond's height as a share of the screen's (the opening is 34%).
constexpr float kBondScreen = 0.27f;
// The opening (title.c insert_sight_backdrop_eye_intro): centre g_TitleX + 768,
// 442; GoldenEye's hole radius 64 scaled 2.7 x 2.57.
constexpr float kHoleX = 768.0f, kHoleY = 442.0f, kHoleSX = 2.7f, kHoleSY = 2.57f;

const char* const kRifling = "ge/intro/barrel/rifling";
const char* const kBondDir = "ge/intro/bond";
const char* const kFlash = "ge/fx/flash"; // tools/n64kr-rs ge-hazards
const char* const kSfxShot = "sound/n64kr/ge/rifle_big.wav";
const char* const kSfxBoom = "sound/n64kr/ge/explosion.wav";
const char* const kSfxBoom2 = "sound/n64kr/ge/explosion2.wav";

struct BondInfo {
    bool ok = false;
    float height = 1.0f;
    int walk = 1, fireFirst = 2, fireStep = 2, fireCount = 1;
    std::vector<std::array<float, 3>> muzzle;
};

bool sStarted = false, sDone = false, sSkipped = false;
bool sMusic = false, sShot = false, sBoom = false;
bool sHaveBarrel = false;
Clock::time_point sStart;
float sTime = 0.0f;
BondInfo sBond;
std::shared_future<std::shared_ptr<std::vector<std::shared_ptr<Ship::IResource>>>> sPreload;

Mtx sOrtho;
Mtx sBondProj;
// Unit shapes, built once (z = 0, 2D).
Vtx sDisk[32];   // GoldenEye's hole: 16 rows, radius 64, shaded 254 (bottom) .. 32 (top)
Vtx sQuad[4];    // 0..1000 square, white
Vtx sCap[10];    // half disk below y = 0, 1000 wide (a blood drip's end)
Vtx sGlow[17];   // disk of radius 1000: white centre, clear rim
Vtx sRing[27];   // a quarter of a soft ring: radii 800 (clear), 920 (solid), 1000 (clear)
std::vector<Vtx> sTitle; // "N64KR" in block letters, 4 vertices a cell

// The title's letters on a 5 x 7 grid.
const char* const kGlyphs[5][7] = {
    { "#...#", "##..#", "#.#.#", "#..##", "#...#", "#...#", "#...#" },
    { ".###.", "#....", "#....", "####.", "#...#", "#...#", ".###." },
    { "...#.", "..##.", ".#.#.", "#..#.", "#####", "...#.", "...#." },
    { "#...#", "#..#.", "#.#..", "##...", "#.#..", "#..#.", "#...#" },
    { "####.", "#...#", "#...#", "####.", "#.#..", "#..#.", "#...#" },
};
constexpr float kCell = 34.0f;
constexpr float kTitleX = 640.0f, kTitleY = 520.0f; // the title's centre

float clamp01(float x) {
    return std::clamp(x, 0.0f, 1.0f);
}

float ease_out(float x) {
    x = clamp01(x);
    return 1.0f - (1.0f - x) * (1.0f - x);
}

void set_vtx(Vtx& v, float x, float y, u8 r, u8 g, u8 b, u8 a) {
    v.v.ob[0] = (s16) std::lround(x);
    v.v.ob[1] = (s16) std::lround(y);
    v.v.ob[2] = 0;
    v.v.flag = 0;
    v.v.tc[0] = v.v.tc[1] = 0;
    v.v.cn[0] = r;
    v.v.cn[1] = g;
    v.v.cn[2] = b;
    v.v.cn[3] = a;
}

void title_colour(float f, u8* c) {
    // Top to bottom: pale yellow, orange, deep red.
    const float stops[3][3] = { { 255, 255, 210 }, { 255, 185, 40 }, { 205, 35, 10 } };
    const float k = clamp01(f) * 2.0f;
    const int i = std::min((int) k, 1);
    const float u = k - (float) i;
    for (int ch = 0; ch < 3; ch++) {
        c[ch] = (u8) (stops[i][ch] + (stops[i + 1][ch] - stops[i][ch]) * u);
    }
}

void build_geometry() {
    // title3.c createGunbarrelRenderHole: rows at t = 0, 2pi/30 .. pi from the
    // bottom (y = -64 cos t, half width 64 sin t), grey 143 + 111 cos t.
    for (int k = 0; k < 16; k++) {
        const float t = (float) k * 2.0f * kPi / 30.0f;
        const float x = 64.0f * std::sin(t), y = -64.0f * std::cos(t);
        const u8 c = (u8) (143.0f + 111.0f * std::cos(t));
        set_vtx(sDisk[k * 2], -x, y, c, c, c, 255);
        set_vtx(sDisk[k * 2 + 1], x, y, c, c, c, 255);
    }
    set_vtx(sQuad[0], 0, 0, 255, 255, 255, 255);
    set_vtx(sQuad[1], 1000, 0, 255, 255, 255, 255);
    set_vtx(sQuad[2], 1000, 1000, 255, 255, 255, 255);
    set_vtx(sQuad[3], 0, 1000, 255, 255, 255, 255);
    set_vtx(sCap[0], 500, 0, 255, 255, 255, 255);
    for (int i = 0; i <= 8; i++) {
        const float a = kPi + kPi * (float) i / 8.0f;
        set_vtx(sCap[1 + i], 500.0f + 500.0f * std::cos(a), 500.0f * std::sin(a), 255, 255, 255, 255);
    }
    set_vtx(sGlow[0], 0, 0, 255, 255, 255, 255);
    for (int i = 0; i < 16; i++) {
        const float a = 2.0f * kPi * (float) i / 16.0f;
        set_vtx(sGlow[1 + i], 1000.0f * std::cos(a), 1000.0f * std::sin(a), 255, 255, 255, 0);
    }
    const float radii[3] = { 800, 920, 1000 };
    const u8 alpha[3] = { 0, 255, 0 };
    for (int c = 0; c <= 8; c++) {
        const float a = 0.5f * kPi * (float) c / 8.0f;
        for (int r = 0; r < 3; r++) {
            set_vtx(sRing[c * 3 + r], radii[r] * std::cos(a), radii[r] * std::sin(a), 255, 255, 255, alpha[r]);
        }
    }
    // Five letters, 6 cells apart, centred on the title's centre.
    const float left = kTitleX - 29.0f * kCell / 2.0f, top = kTitleY + 3.5f * kCell;
    for (int l = 0; l < 5; l++) {
        for (int row = 0; row < 7; row++) {
            for (int col = 0; col < 5; col++) {
                if (kGlyphs[l][row][col] != '#') {
                    continue;
                }
                const float x0 = left + (float) (l * 6 + col) * kCell, x1 = x0 + kCell;
                const float y1 = top - (float) row * kCell, y0 = y1 - kCell;
                u8 hi[3], lo[3];
                title_colour((float) row / 7.0f, hi);
                title_colour((float) (row + 1) / 7.0f, lo);
                Vtx v[4];
                set_vtx(v[0], x0, y0, lo[0], lo[1], lo[2], 255);
                set_vtx(v[1], x1, y0, lo[0], lo[1], lo[2], 255);
                set_vtx(v[2], x1, y1, hi[0], hi[1], hi[2], 255);
                set_vtx(v[3], x0, y1, hi[0], hi[1], hi[2], 255);
                sTitle.insert(sTitle.end(), v, v + 4);
            }
        }
    }
    guOrtho(&sOrtho, 0.0f, kW, 0.0f, kH, -100.0f, 100.0f, 1.0f);
}

void load_bond_info() {
    auto file = Ship::Context::GetRawInstance()->GetResourceManager()->LoadFileProcess(std::string(kBondDir) + "/model.json");
    if ((file == nullptr) || (file->Buffer == nullptr)) {
        SPDLOG_WARN("[n64kr] {} not installed (run tools/n64kr-rs ge-intro): the intro skips the gun barrel", kBondDir);
        return;
    }
    try {
        auto j = nlohmann::json::parse(std::string(file->Buffer->begin(), file->Buffer->end()));
        sBond.height = j.value("height", 1.0f);
        sBond.walk = std::max(1, j.value("walk_count", 1));
        sBond.fireFirst = j.value("fire_first", 2);
        sBond.fireStep = std::max(1, j.value("fire_step", 2));
        sBond.fireCount = std::max(1, j.value("fire_count", 1));
        for (const auto& m : j.value("muzzle", nlohmann::json::array())) {
            sBond.muzzle.push_back({ m[0].get<float>(), m[1].get<float>(), m[2].get<float>() });
        }
        sBond.ok = (sBond.height > 0.0f);
    } catch (const std::exception& e) {
        SPDLOG_ERROR("[n64kr] {}/model.json: {}", kBondDir, e.what());
    }
}

bool start() {
    if (sStarted) {
        return !sDone;
    }
    sStarted = true;
    if (!CVarGetInteger("gN64KR.Intro", 1)) {
        sDone = true;
        return false;
    }
    build_geometry();
    load_bond_info();
    sHaveBarrel = sBond.ok && (ResourceGetDataByName(kRifling) != nullptr);
    if (sHaveBarrel) {
        // Bond's 96 poses load in the background during the credit card.
        sPreload = Ship::Context::GetRawInstance()->GetResourceManager()->LoadResourcesAsync(
            Ship::ResourceFilter({ std::string(kBondDir) + "/*" }, { "*.json" }, 0, nullptr));
    }
    sStart = Clock::now();
    return true;
}

// Seconds into the intro; without the gun barrel, the credit card runs into the nuke.
float elapsed() {
    float t = std::chrono::duration<float>(Clock::now() - sStart).count();
    if (!sHaveBarrel && (t > kCreditEnd + 0.4f)) {
        t += kNukeStart - (kCreditEnd + 0.4f);
    }
    return t;
}

// ---- drawing ----

void state_2d(bool translucent) {
    gDPPipeSync(gDisplayListHead++);
    gDPSetCycleType(gDisplayListHead++, G_CYC_1CYCLE);
    gSPClearGeometryMode(gDisplayListHead++, G_ZBUFFER | G_CULL_BOTH | G_LIGHTING | G_FOG | G_TEXTURE_GEN);
    gSPSetGeometryMode(gDisplayListHead++, G_SHADE | G_SHADING_SMOOTH);
    gSPTexture(gDisplayListHead++, 0xFFFF, 0xFFFF, 0, G_TX_RENDERTILE, G_OFF);
    if (translucent) {
        gDPSetRenderMode(gDisplayListHead++, G_RM_XLU_SURF, G_RM_XLU_SURF2);
    } else {
        gDPSetRenderMode(gDisplayListHead++, G_RM_OPA_SURF, G_RM_OPA_SURF2);
    }
    gSPMatrix(gDisplayListHead++, &sOrtho, G_MTX_NOPUSH | G_MTX_LOAD | G_MTX_PROJECTION);
}

void shade() {
    gDPSetCombineMode(gDisplayListHead++, G_CC_SHADE, G_CC_SHADE);
}

void prim(u8 r, u8 g, u8 b, u8 a) {
    gDPSetCombineMode(gDisplayListHead++, G_CC_PRIMITIVE, G_CC_PRIMITIVE);
    gDPSetPrimColor(gDisplayListHead++, 0, 0, r, g, b, a);
}

// Vertex colour and alpha tinted by the primitive colour.
void shade_prim(u8 r, u8 g, u8 b, u8 a) {
    gDPSetCombineLERP(gDisplayListHead++, SHADE, 0, PRIMITIVE, 0, SHADE, 0, PRIMITIVE, 0, SHADE, 0, PRIMITIVE, 0,
                      SHADE, 0, PRIMITIVE, 0);
    gDPSetPrimColor(gDisplayListHead++, 0, 0, r, g, b, a);
}

// Model matrix: scale, turn (radians), then move (row vectors, as render_set_position takes them).
bool place(float x, float y, float sx, float sy, float angle = 0.0f) {
    const float c = std::cos(angle), s = std::sin(angle);
    Mat4 m = { { sx * c, sx * s, 0, 0 }, { -sy * s, sy * c, 0, 0 }, { 0, 0, 1, 0 }, { x, y, 0, 1 } };
    return render_set_position(m, 0) != 0;
}

void draw_disk() {
    gSPVertex(gDisplayListHead++, (uintptr_t) sDisk, 32, 0);
    for (int k = 0; k < 15; k++) {
        const int a = k * 2;
        gSP2Triangles(gDisplayListHead++, a, a + 1, a + 3, 0, a, a + 3, a + 2, 0);
    }
}

void draw_quad() {
    gSPVertex(gDisplayListHead++, (uintptr_t) sQuad, 4, 0);
    gSP2Triangles(gDisplayListHead++, 0, 1, 2, 0, 0, 2, 3, 0);
}

void draw_cap() {
    gSPVertex(gDisplayListHead++, (uintptr_t) sCap, 10, 0);
    for (int i = 0; i < 8; i++) {
        gSP1Triangle(gDisplayListHead++, 0, 1 + i, 2 + i, 0);
    }
}

void draw_glow() {
    gSPVertex(gDisplayListHead++, (uintptr_t) sGlow, 17, 0);
    for (int i = 0; i < 16; i++) {
        gSP1Triangle(gDisplayListHead++, 0, 1 + i, 1 + (i + 1) % 16, 0);
    }
}

void draw_ring_quarter() {
    gSPVertex(gDisplayListHead++, (uintptr_t) sRing, 27, 0);
    for (int c = 0; c < 8; c++) {
        for (int r = 0; r < 2; r++) {
            const int a = c * 3 + r, b = (c + 1) * 3 + r;
            gSP2Triangles(gDisplayListHead++, a, b, b + 1, 0, a, b + 1, a + 1, 0);
        }
    }
}

void draw_title_cells() {
    for (size_t i = 0; i < sTitle.size(); i += 32) {
        const int n = (int) std::min<size_t>(32, sTitle.size() - i);
        gSPVertex(gDisplayListHead++, (uintptr_t) &sTitle[i], n, 0);
        for (int q = 0; q < n; q += 4) {
            gSP2Triangles(gDisplayListHead++, q, q + 1, q + 2, 0, q, q + 2, q + 3, 0);
        }
    }
}

void rect(float x0, float y0, float x1, float y1) {
    if (place(x0, y0, (x1 - x0) / 1000.0f, (y1 - y0) / 1000.0f)) {
        draw_quad();
    }
}

// Past the 4:3 screen's edges too, for widescreen windows.
void fill(u8 r, u8 g, u8 b, u8 a) {
    state_2d(a < 255);
    prim(r, g, b, a);
    rect(-1000.0f, -200.0f, kW + 1000.0f, kH + 200.0f);
}

void ring(float x, float y, float radius, u8 r, u8 g, u8 b, u8 a) {
    state_2d(true);
    shade_prim(r, g, b, a);
    for (int q = 0; q < 4; q++) {
        if (place(x, y, radius / 1000.0f, radius / 1000.0f, (float) q * 0.5f * kPi)) {
            draw_ring_quarter();
        }
    }
}

void glow(float x, float y, float radius, u8 r, u8 g, u8 b, u8 a) {
    state_2d(true);
    shade_prim(r, g, b, a);
    if (place(x, y, radius / 1000.0f, radius / 1000.0f)) {
        draw_glow();
    }
}

// MK64's menu text draws under the menus' screen projection (func_80094A64).
void text_state() {
    gDPPipeSync(gDisplayListHead++);
    gSPMatrix(gDisplayListHead++, GetScreenMatrix(), G_MTX_NOPUSH | G_MTX_LOAD | G_MTX_PROJECTION);
}

void draw_credit(float t) {
    fill(0, 0, 0, 255);
    text_state();
    set_text_color(TEXT_BLUE);
    print_text1_center_mode_1(160, 82, (char*) "ENVISIONED AND PRODUCED BY", 0, 0.6f, 0.6f);
    set_text_color(TEXT_YELLOW);
    print_text1_center_mode_1(160, 104, (char*) "UPDOGE", 0, 1.0f, 1.0f);
    set_text_color(TEXT_BLUE);
    print_text1_center_mode_1(160, 140, (char*) "AUGMENTED AND CREATED BY", 0, 0.6f, 0.6f);
    set_text_color(TEXT_YELLOW);
    print_text1_center_mode_1(160, 162, (char*) "CLAUDE", 0, 1.0f, 1.0f);
    float dark = 0.0f;
    if (t < kCreditIn) {
        dark = 1.0f - t / kCreditIn;
    } else if (t > kCreditOut) {
        dark = clamp01((t - kCreditOut) / (kCreditEnd - kCreditOut));
    }
    if (dark > 0.0f) {
        fill(0, 0, 0, (u8) (255.0f * dark));
    }
}

// title.c manipulateGunbarrelAndLogoMatrices: a white dot running across, and
// one left behind where the runner was every 34 frames.
void draw_dots(float u) {
    fill(0, 0, 0, 255);
    const float frames = u * 60.0f;
    const float lead = -30.0f + 6.0f * frames;
    float trail = -100.0f;
    if (frames >= 12.0f) {
        const float jump = 12.0f + std::floor((frames - 12.0f) / 34.0f) * 34.0f;
        trail = -30.0f + 6.0f * jump - 12.0f;
    }
    state_2d(false);
    prim(0xE6, 0xE6, 0xE6, 255);
    for (float x : { lead, trail }) {
        if (place(x, 482.0f, 1.0f, 1.0f)) {
            draw_disk();
        }
    }
}

// Bond's pose at `tick` of his clock: the display list's name and, for fire
// poses, the index into the muzzle list.
std::string bond_pose(float tick, int* fire) {
    char name[32];
    if (tick < kFireTick) {
        const int f = (int) std::fmod(2.0f + tick * kAnimRate, (float) sBond.walk);
        snprintf(name, sizeof(name), "%s/walk%02d", kBondDir, f);
        *fire = -1;
        return name;
    }
    float frame = 2.0f + (std::min(tick, kSpeedupTick) - kFireTick) * kAnimRate;
    frame += std::max(tick - kSpeedupTick, 0.0f) * kFastRate;
    const int i = std::clamp((int) std::lround((frame - (float) sBond.fireFirst) / (float) sBond.fireStep), 0,
                             sBond.fireCount - 1);
    snprintf(name, sizeof(name), "%s/fire%03d", kBondDir, sBond.fireFirst + i * sBond.fireStep);
    *fire = i;
    return name;
}

void mat_mul(const float a[4][4], const float b[4][4], float out[4][4]) {
    for (int i = 0; i < 4; i++) {
        for (int j = 0; j < 4; j++) {
            out[i][j] = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j] + a[i][3] * b[3][j];
        }
    }
}

// Bond in the opening centred at (cx, cy): GoldenEye's lens (title.c
// insert_bond_eye_intro: 46 degrees, 4:3), the camera level with his middle on
// his left (+x), so he walks to screen left and turns to fire into the lens.
void draw_bond(float cx, float cy, float tick) {
    int fire = -1;
    const std::string pose = bond_pose(tick, &fire);
    Gfx* dl = (Gfx*) ResourceGetDataByName(pose.c_str());
    if (dl == nullptr) {
        return;
    }
    float proj[4][4], view[4][4], m[4][4];
    u16 norm;
    guPerspectiveF(proj, &norm, 46.0f, 4.0f / 3.0f, 100.0f, 40000.0f, 1.0f);
    const float dist = sBond.height / (kBondScreen * 2.0f * std::tan(23.0f * kPi / 180.0f));
    const float mid = sBond.height * 0.5f;
    guLookAtF(view, dist, mid, 0.0f, 0.0f, mid, 0.0f, 0.0f, 1.0f, 0.0f);
    mat_mul(view, proj, m);
    // Move the picture to the opening: clip x += ndc x * w (and y).
    const float nx = cx / (kW / 2.0f) - 1.0f, ny = cy / (kH / 2.0f) - 1.0f;
    for (int i = 0; i < 4; i++) {
        m[i][0] += nx * m[i][3];
        m[i][1] += ny * m[i][3];
    }
    guMtxF2L(m, &sBondProj);
    gDPPipeSync(gDisplayListHead++);
    gSPPerspNormalize(gDisplayListHead++, norm);
    gSPMatrix(gDisplayListHead++, &sBondProj, G_MTX_NOPUSH | G_MTX_LOAD | G_MTX_PROJECTION);
    gDPSetRenderMode(gDisplayListHead++, G_RM_AA_ZB_TEX_EDGE, G_RM_AA_ZB_TEX_EDGE2);
    Mat4 id = { { 1, 0, 0, 0 }, { 0, 1, 0, 0 }, { 0, 0, 1, 0 }, { 0, 0, 0, 1 } };
    if (render_set_position(id, 0) == 0) {
        return;
    }
    gSPDisplayList(gDisplayListHead++, dl);

    // The muzzle flash, facing the lens (+x), just in front of the muzzle.
    if ((fire >= 0) && (fire < (int) sBond.muzzle.size()) && (tick >= kShotTick) && (tick < kShotTick + kFlashTicks)) {
        Gfx* flash = (Gfx*) ResourceGetDataByName(kFlash);
        const auto& p = sBond.muzzle[fire];
        const float s = 1.6f;
        Mat4 f = { { 0, 0, -s, 0 }, { 0, s, 0, 0 }, { s, 0, 0, 0 }, { p[0] + 40.0f, p[1], p[2], 1 } };
        if ((flash != nullptr) && (render_set_position(f, 0) != 0)) {
            gDPPipeSync(gDisplayListHead++);
            gDPSetRenderMode(gDisplayListHead++, G_RM_XLU_SURF, G_RM_XLU_SURF2);
            gDPSetPrimColor(gDisplayListHead++, 0, 0, 255, 240, 200, 255);
            gSPDisplayList(gDisplayListHead++, flash);
        }
    }
    gDPPipeSync(gDisplayListHead++);
    gSPTexture(gDisplayListHead++, 1, 1, 0, G_TX_RENDERTILE, G_OFF);
}

// Ours: a curtain of blood running down from the top in columns of different
// widths and speeds, GoldenEye's colour (blood_animation.c: 150, 0, 0, alpha 180).
void draw_blood(float b) {
    state_2d(true);
    prim(150, 0, 0, 180);
    float x = -40.0f;
    for (int i = 0; x < kW + 40.0f; i++) {
        // Fixed pseudo-random widths, delays and speeds.
        const float r1 = std::fmod(std::sin((float) i * 12.9898f) * 43758.547f, 1.0f);
        const float r2 = std::fmod(std::sin((float) i * 78.233f) * 12345.679f, 1.0f);
        const float w = 26.0f + 30.0f * std::fabs(r1);
        const float delay = 0.45f * std::fabs(r2);
        const float run = clamp01((b - delay) / (kBlood - 0.1f - delay));
        const float bottom = kH - run * run * (kH + 120.0f) - 4.0f;
        rect(x, bottom, x + w, kH + 10.0f);
        if (place(x, bottom, w / 1000.0f, w / 1000.0f)) {
            draw_cap();
        }
        x += w;
    }
}

void draw_barrel(float t) {
    const float u = t - kBarrel;
    if (u < kDots) {
        draw_dots(u);
        return;
    }
    // g_TitleX: sliding in, then still, then the wobble (sins x 64), which ours
    // lets sink as the barrel dies.
    float titleX = std::max(kSlideFrom - (t - kSlideStart) * kSlideSpeed, kSlideTo);
    float sink = 0.0f;
    if (t >= kWobbleStart) {
        const float w = t - kWobbleStart;
        titleX = kSlideTo + 64.0f * std::sin(w * kWobbleSpeed);
        const float k = w / (kWobble + kFade);
        sink = 90.0f * k * k;
    }
    fill(0, 0, 0, 255);
    // The rifling (title.c insert_sniper_sight_eye_intro), its x offset g_TitleX.
    Gfx* rifling = (Gfx*) ResourceGetDataByName(kRifling);
    state_2d(false);
    if ((rifling != nullptr) && place(titleX, -sink, 1.0f, 1.0f)) {
        gSPDisplayList(gDisplayListHead++, rifling);
    }
    // The opening (insert_sight_backdrop_eye_intro).
    state_2d(false);
    shade();
    const float cx = titleX + kHoleX, cy = kHoleY - sink;
    if (place(cx, cy, kHoleSX, kHoleSY)) {
        draw_disk();
    }
    if (titleX < kBondFrom) {
        draw_bond(cx, cy, (t - kBondStart) * kTicks);
    }
    if (t >= kWobbleStart) {
        fill(150, 0, 0, 180); // sub_GAME_7F01CA18: the screen stays red
    } else if (t >= kBloodStart) {
        draw_blood(t - kBloodStart);
    }
    if (t >= kFadeStart) {
        fill(0, 0, 0, (u8) (247.0f * clamp01((t - kFadeStart) / kFade)));
    }
}

void draw_title(float x, float y, float scale) {
    // Block letters with a dark extrusion down and to the right.
    for (int k = 7; k >= 0; k--) {
        state_2d(false);
        if (k > 0) {
            const float f = (float) (8 - k) / 8.0f;
            prim((u8) (40 + 90 * f), (u8) (8 + 26 * f), (u8) (4 + 8 * f), 255);
        } else {
            shade();
        }
        const float ox = (float) k * 4.0f, oy = -(float) k * 4.0f;
        if (place(x + ox - kTitleX * scale, y + oy - kTitleY * scale, scale, scale)) {
            draw_title_cells();
        }
    }
}

void draw_nuke(float v) {
    fill(0, 0, 0, 255);
    const float shake = 26.0f * std::max(0.0f, 1.0f - v / 1.4f);
    const float cx = kTitleX + shake * std::sin(v * 71.0f), cy = 480.0f + shake * std::cos(v * 53.0f);
    // The afterglow behind the title, fading to embers.
    if (v > 0.25f) {
        const float heat = 1.0f - 0.55f * clamp01((v - 1.0f) / 3.0f);
        const float pulse = 0.92f + 0.08f * std::sin(v * 5.0f);
        glow(cx, cy, 400.0f + 1100.0f * ease_out((v - 0.25f) / 2.5f), 255, (u8) (70 + 60 * heat), 20,
             (u8) (230.0f * heat * pulse));
    }
    if (v > 0.3f) {
        const float settle = ease_out((v - 0.8f) / 1.6f);
        draw_title(cx, cy + (kTitleY - 480.0f), 1.0f + 0.15f * (1.0f - settle));
    }
    // The shockwave.
    if ((v > 0.2f) && (v < 2.6f)) {
        const float k = (v - 0.2f) / 2.4f;
        ring(cx, cy, 120.0f + 1150.0f * ease_out(k), 255, 235, 190, (u8) (255.0f * (1.0f - k)));
    }
    // The flash: a white core blowing out to fill the screen.
    if (v < 0.5f) {
        glow(cx, cy, 2200.0f * ease_out(v / 0.35f), 255, 255, 255, 255);
    }
    if (v >= 2.4f) {
        text_state();
        set_text_color(TEXT_YELLOW);
        print_text1_center_mode_1(160, 168, (char*) "N64 KART RACING", 0, 0.7f, 0.7f);
    }
    float white = 0.0f;
    if (v < 0.3f) {
        white = v / 0.3f;
    } else if (v < 0.8f) {
        white = 1.0f;
    } else {
        white = 1.0f - clamp01((v - 0.8f) / 1.6f);
    }
    if (white > 0.0f) {
        fill(255, 255, 255, (u8) (255.0f * white));
    }
    if (v > kNuke - 1.0f) {
        fill(0, 0, 0, (u8) (255.0f * clamp01(v - (kNuke - 1.0f))));
    }
}

void finish() {
    sDone = true;
    if (sMusic) {
        HMAS_Stop(HMAS_MUSIC);
    }
}

} // namespace

extern "C" s32 n64kr_intro_tick(void) {
    if (!start()) {
        return 0;
    }
    const float t = elapsed();
    sTime = t;
    if (!sSkipped) {
        if (!sMusic && sHaveBarrel && (t >= kBarrel) && (t < kNukeStart)) {
            n64kr_music_prepare(N64KR_MUSIC_GE_INTRO);
            if (HMAS_IsIDRegistered(N64KR_MUSIC_GE_INTRO)) {
                HMAS_Play(HMAS_MUSIC, N64KR_MUSIC_GE_INTRO, false);
            }
            sMusic = true;
        }
        if (!sShot && sHaveBarrel && (t >= kBondStart + kShotTick / kTicks) && (t < kNukeStart)) {
            n64kr_sfx_play(kSfxShot, nullptr, 0.9f, 1.0f);
            sShot = true;
        }
        if (!sBoom && (t >= kNukeStart)) {
            if (sMusic) {
                HMAS_Stop(HMAS_MUSIC);
            }
            n64kr_sfx_play(kSfxBoom, nullptr, 1.0f, 0.55f);
            n64kr_sfx_play(kSfxBoom2, nullptr, 0.8f, 0.7f);
            sBoom = true;
        }
    }
    if (t >= kEnd) {
        finish();
        return 0;
    }
    return 1;
}

extern "C" s32 n64kr_intro_draw(void) {
    if (!start()) {
        return 0;
    }
    const float t = sTime;
    if (t < kCreditEnd) {
        draw_credit(t);
    } else if ((t >= kBarrel) && (t < kNukeStart - kBlack)) {
        draw_barrel(t);
    } else if (t >= kNukeStart) {
        draw_nuke(t - kNukeStart);
    } else {
        fill(0, 0, 0, 255);
    }
    // Leave the state as HM_DrawIntro's Sync() does.
    gDPPipeSync(gDisplayListHead++);
    gDPSetTexturePersp(gDisplayListHead++, G_TP_NONE);
    gDPSetTextureFilter(gDisplayListHead++, G_TF_BILERP);
    return 1;
}

extern "C" void n64kr_intro_skip(void) {
    if (!sStarted || sDone || sSkipped) {
        return;
    }
    sSkipped = true;
    if (sMusic) {
        HMAS_Stop(HMAS_MUSIC);
    }
    n64kr_sfx_stop_all();
}
