// N64KR: GoldenEye hazards for the Dam tracks (see ge_hazards.h).
//
// Units: MK64 world units; ticks are AActor::Tick calls (twice per game frame,
// 60 a second). Model space (tools/n64kr-rs ge-hazards): GoldenEye model
// units, feet / base on y = 0, facing +Z; model.json gives the height (and for
// guards the muzzle in the aim pose and the barrel's yaw from +Z).

#include <algorithm>
#include <chrono>
#include <cmath>
#include <cstdlib>
#include <string>
#include <unordered_map>
#include <vector>

#include <libultraship.h>
#include <nlohmann/json.hpp>
#include <spdlog/spdlog.h>
#include <ship/Context.h>
#include "ship/resource/ResourceManager.h"
#include "port/interpolation/FrameInterpolation.h"

#include "engine/Actor.h"
#include "n64kr/ge_hazards.h"
#include "n64kr/sfx.h"

extern "C" {
#include <defines.h>
#include "main.h"
#include "camera.h"
#include "code_800029B0.h"
#include "racing/math_util.h"
}

namespace {

constexpr float kPi = 3.14159265f;
constexpr float kKartRadius = 5.5f;

// Barrel: 9 units tall; any racer within kBlastRadius of the explosion tumbles.
constexpr float kBarrelHeight = 9.0f;
constexpr float kBlastRadius = 30.0f;
constexpr float kChainRadius = 45.0f;   // other drums this close go up too
constexpr s32 kChainDelay = 9;
constexpr s32 kExplodeTicks = 54;
constexpr s32 kBarrelRespawn = 600;     // 10 s

// Guard: 16 units tall; sees racers within kRange; raises the KF7 (5 poses),
// aims, fires a burst of kBurst shots, cools down, and lowers it again when
// nobody is near.
constexpr float kGuardHeight = 16.0f;
constexpr float kRange = 650.0f;
constexpr s32 kRaiseTicksPerPose = 6;
constexpr s32 kAimTicks = 28;
constexpr s32 kBurst = 3;
constexpr s32 kShotGap = 7;
constexpr s32 kCooldown = 130;
constexpr s32 kTurnRate = 0x500;        // binary angle per tick
constexpr auto kHitImmunity = std::chrono::seconds(4); // a racer can't be shot again for 4 s
constexpr float kHitChanceNear = 0.32f; // per burst, at 150 units
constexpr float kHitChanceFar = 0.10f;  // at kRange

const char* const kSfxKf7 = "sound/n64kr/ge/kf7.wav";
const char* const kSfxExplosion = "sound/n64kr/ge/explosion.wav";
const char* const kSfxRico[] = { "sound/n64kr/ge/rico1.wav", "sound/n64kr/ge/rico2.wav" };
const char* const kSfxWhiz[] = { "sound/n64kr/ge/whiz1.wav", "sound/n64kr/ge/whiz2.wav" };
const char* const kSfxHit = "sound/n64kr/ge/hit_metal.wav";

const char* const kFxFlash = "ge/fx/flash";
const char* const kFxTracer = "ge/fx/tracer";
const char* const kFxFire = "ge/fx/fire";
const char* const kFxBurst = "ge/fx/burst";
constexpr float kFxQuad = 200.0f; // sprite quads are 200 model units

// When each racer may be shot again (shared by all guards).
std::chrono::steady_clock::time_point sHitImmunity[NUM_PLAYERS];

float frand() {
    return (float) std::rand() / (float) RAND_MAX;
}

struct ModelInfo {
    bool ok = false;
    float height = 1.0f;
    float radius = 1.0f;
    s32 poses = 1;
    s32 aimPose = 0;
    float muzzle[3] = { 0, 0, 0 };
    float aimYaw = 0.0f; // radians, the barrel's direction from +Z in the aim pose
};

const ModelInfo& model_info(const std::string& dir) {
    static std::unordered_map<std::string, ModelInfo> cache;
    auto it = cache.find(dir);
    if (it != cache.end()) {
        return it->second;
    }
    ModelInfo& m = cache[dir];
    auto file = Ship::Context::GetRawInstance()->GetResourceManager()->LoadFileProcess(dir + "/model.json");
    if ((file == nullptr) || (file->Buffer == nullptr)) {
        SPDLOG_WARN("[n64kr] {} not installed (run tools/n64kr-rs ge-hazards)", dir);
        return m;
    }
    try {
        auto j = nlohmann::json::parse(std::string(file->Buffer->begin(), file->Buffer->end()));
        m.height = j.value("height", 1.0f);
        m.radius = j.value("radius", m.height / 4.0f);
        m.poses = std::max(1, j.value("pose_count", 1));
        m.aimPose = std::clamp(j.value("aim_pose", 0), 0, m.poses - 1);
        m.aimYaw = j.value("aim_yaw", 0.0f);
        if (j.contains("muzzle")) {
            for (int i = 0; i < 3; i++) {
                m.muzzle[i] = j["muzzle"][i].get<float>();
            }
        }
        m.ok = (m.height > 0.0f);
    } catch (const std::exception& e) {
        SPDLOG_ERROR("[n64kr] {}/model.json: {}", dir, e.what());
    }
    return m;
}

// Matrices are built directly (row vectors, translation in row 3, as Mat4 is
// used by render_set_position): local +Z faces (sin yaw, cos yaw).
void yaw_matrix(Mat4 m, const Vec3f pos, float yaw, float scale) {
    float s = std::sin(yaw), c = std::cos(yaw);
    m[0][0] = c * scale;  m[0][1] = 0;     m[0][2] = -s * scale; m[0][3] = 0;
    m[1][0] = 0;          m[1][1] = scale; m[1][2] = 0;          m[1][3] = 0;
    m[2][0] = s * scale;  m[2][1] = 0;     m[2][2] = c * scale;  m[2][3] = 0;
    m[3][0] = pos[0];     m[3][1] = pos[1]; m[3][2] = pos[2];    m[3][3] = 1;
}

// A sprite quad (XY plane, facing +Z) turned toward the camera about y.
void billboard_matrix(Mat4 m, const Vec3f pos, const Camera* cam, float size) {
    float fx = cam->pos[0] - pos[0], fz = cam->pos[2] - pos[2];
    float l = std::sqrt(fx * fx + fz * fz);
    if (l < 0.001f) {
        fx = 0, fz = 1, l = 1;
    }
    fx /= l, fz /= l;
    float s = size / kFxQuad;
    m[0][0] = fz * s;  m[0][1] = 0; m[0][2] = -fx * s; m[0][3] = 0;
    m[1][0] = 0;       m[1][1] = s; m[1][2] = 0;       m[1][3] = 0;
    m[2][0] = fx * s;  m[2][1] = 0; m[2][2] = fz * s;  m[2][3] = 0;
    m[3][0] = pos[0];  m[3][1] = pos[1]; m[3][2] = pos[2]; m[3][3] = 1;
}

// The tracer quads (z = 0..200) stretched from `a` to `b`, `width` wide.
bool beam_matrix(Mat4 m, const Vec3f a, const Vec3f b, float width) {
    float d[3] = { b[0] - a[0], b[1] - a[1], b[2] - a[2] };
    float len = std::sqrt(d[0] * d[0] + d[1] * d[1] + d[2] * d[2]);
    if (len < 0.01f) {
        return false;
    }
    for (float& v : d) {
        v /= len;
    }
    // right = up x dir, then up' = dir x right
    float r[3] = { d[2], 0.0f, -d[0] };
    float rl = std::sqrt(r[0] * r[0] + r[2] * r[2]);
    if (rl < 0.001f) {
        r[0] = 1, r[2] = 0, rl = 1;
    }
    r[0] /= rl, r[2] /= rl;
    float u[3] = { d[1] * r[2] - d[2] * r[1], d[2] * r[0] - d[0] * r[2], d[0] * r[1] - d[1] * r[0] };
    float w = width / 24.0f, k = len / kFxQuad;
    for (int i = 0; i < 3; i++) {
        m[0][i] = r[i] * w;
        m[1][i] = u[i] * w;
        m[2][i] = d[i] * k;
        m[3][i] = a[i];
    }
    m[0][3] = m[1][3] = m[2][3] = 0;
    m[3][3] = 1;
    return true;
}

void draw_resource(const char* path, Mat4 m) {
    Gfx* dl = (Gfx*) ResourceGetDataByName(path);
    if ((dl == nullptr) || (render_set_position(m, 0) == 0)) {
        return;
    }
    gSPDisplayList(gDisplayListHead++, dl);
}

void begin_models() {
    gDPPipeSync(gDisplayListHead++);
    gDPSetRenderMode(gDisplayListHead++, G_RM_AA_ZB_TEX_EDGE, G_RM_AA_ZB_TEX_EDGE2);
}

void begin_fx(u8 r, u8 g, u8 b, u8 a) {
    gDPPipeSync(gDisplayListHead++);
    gDPSetRenderMode(gDisplayListHead++, G_RM_ZB_XLU_SURF, G_RM_ZB_XLU_SURF2);
    gDPSetPrimColor(gDisplayListHead++, 0, 0, r, g, b, a);
}

void end_draw() {
    gDPPipeSync(gDisplayListHead++);
    gDPSetCycleType(gDisplayListHead++, G_CYC_1CYCLE);
    gSPTexture(gDisplayListHead++, 1, 1, 0, G_TX_RENDERTILE, G_OFF);
    gDPSetAlphaCompare(gDisplayListHead++, G_AC_NONE);
}

bool racer_active(const Player* p) {
    return (p->type & PLAYER_EXISTS) && !(p->effects & BOO_EFFECT);
}

class AGeBarrel;
std::vector<AGeBarrel*> sBarrels;

class AGeBarrel : public AActor {
  public:
    explicit AGeBarrel(const SpawnParams& params) : AActor(params) {
        Name = "GoldenEye Barrel";
        ResourceName = "n64kr:ge_barrel";
        FVector loc = params.Location.value_or(FVector{ 0, 0, 0 });
        Pos[0] = loc.x * gTrackDirection;
        Pos[1] = loc.y;
        Pos[2] = loc.z;
        mYaw = params.Rotation.has_value() ? params.Rotation->yaw * kPi / 32768.0f : frand() * 2 * kPi;
        mIdx = sCount++;
        sBarrels.push_back(this);
    }

    ~AGeBarrel() override {
        sBarrels.erase(std::remove(sBarrels.begin(), sBarrels.end(), this), sBarrels.end());
    }

    bool IsMod() override {
        return true;
    }

    void Ignite(s32 delay) {
        if (mState == State::Standing && (mFuse < 0 || delay < mFuse)) {
            mFuse = delay;
        }
    }

    void Tick() override {
        switch (mState) {
            case State::Standing:
                if (mFuse >= 0 && mFuse-- == 0) {
                    Explode();
                    break;
                }
                for (s32 i = 0; i < NUM_PLAYERS; i++) {
                    const Player* p = &gPlayers[i];
                    if (!racer_active(p)) {
                        continue;
                    }
                    float dx = p->pos[0] - Pos[0], dz = p->pos[2] - Pos[2], dy = p->pos[1] - Pos[1];
                    float reach = Radius() + kKartRadius;
                    if ((dx * dx + dz * dz < reach * reach) && (dy > -10.0f) && (dy < 20.0f)) {
                        Explode();
                        break;
                    }
                }
                break;
            case State::Exploding:
                if (++mAge >= kExplodeTicks) {
                    mState = State::Gone;
                    mAge = 0;
                }
                break;
            case State::Gone:
                if (++mAge >= kBarrelRespawn) {
                    mState = State::Standing;
                    mAge = 0;
                    mFuse = -1;
                }
                break;
        }
    }

    void Draw(Camera* camera) override {
        FrameInterpolation_RecordOpenChild("n64kr_ge_barrel", (mIdx << 4) | camera->cameraId);
        Mat4 m;
        if (mState == State::Standing) {
            const ModelInfo& info = model_info("ge/barrel");
            if (info.ok) {
                begin_models();
                yaw_matrix(m, Pos, mYaw, kBarrelHeight / info.height);
                draw_resource("ge/barrel/model", m);
                end_draw();
            }
        } else if (mState == State::Exploding) {
            DrawExplosion(camera);
        }
        FrameInterpolation_RecordCloseChild();
    }

  private:
    enum class State { Standing, Exploding, Gone };

    float Radius() const {
        const ModelInfo& info = model_info("ge/barrel");
        return info.ok ? info.radius * kBarrelHeight / info.height : 2.5f;
    }

    void Explode() {
        mState = State::Exploding;
        mAge = 0;
        mFuse = -1;
        n64kr_sfx_play(kSfxExplosion, Pos, 1.0f, 0.9f + 0.2f * frand());
        for (s32 i = 0; i < NUM_PLAYERS; i++) {
            Player* p = &gPlayers[i];
            if (!racer_active(p) || (p->effects & STAR_EFFECT)) {
                continue;
            }
            float dx = p->pos[0] - Pos[0], dz = p->pos[2] - Pos[2], dy = p->pos[1] - Pos[1];
            if ((dx * dx + dz * dz < kBlastRadius * kBlastRadius) && (std::fabs(dy) < kBlastRadius)) {
                p->triggers |= VERTICAL_TUMBLE_TRIGGER;
            }
        }
        // Chain reaction: nearby drums go up a moment later.
        for (AGeBarrel* b : sBarrels) {
            if (b == this) {
                continue;
            }
            float dx = b->Pos[0] - Pos[0], dz = b->Pos[2] - Pos[2];
            if (dx * dx + dz * dz < kChainRadius * kChainRadius) {
                b->Ignite(kChainDelay + (s32) (frand() * 6));
            }
        }
    }

    void DrawExplosion(Camera* camera) {
        float t = (float) mAge / kExplodeTicks;
        Mat4 m;
        Vec3f p;
        // Shock ring: fast and short.
        if (mAge < 14) {
            float k = mAge / 14.0f;
            begin_fx(255, 240, 200, (u8) (255 * (1.0f - k)));
            p[0] = Pos[0], p[1] = Pos[1] + 4.0f, p[2] = Pos[2];
            billboard_matrix(m, p, camera, 8.0f + 50.0f * k);
            draw_resource(kFxBurst, m);
        }
        // Fireballs: grow, rise, cool from yellow to red, fade.
        const float off[3][3] = { { 0, 0, 0 }, { 5, -2, 3 }, { -4, 1, -5 } };
        for (int i = 0; i < 3; i++) {
            float ti = std::clamp(t - i * 0.08f, 0.0f, 1.0f);
            if (ti <= 0.0f) {
                continue;
            }
            float grow = 1.0f - (1.0f - ti) * (1.0f - ti);
            u8 alpha = (u8) (255 * std::clamp((1.0f - ti) * 1.6f, 0.0f, 1.0f));
            begin_fx(255, (u8) (230 - 140 * ti), (u8) (140 - 120 * ti), alpha);
            p[0] = Pos[0] + off[i][0];
            p[1] = Pos[1] + 6.0f + off[i][1] + 14.0f * ti;
            p[2] = Pos[2] + off[i][2];
            billboard_matrix(m, p, camera, (i == 0 ? 14.0f : 9.0f) + 26.0f * grow);
            draw_resource(kFxFire, m);
        }
        end_draw();
    }

    State mState = State::Standing;
    s32 mAge = 0;
    s32 mFuse = -1;
    float mYaw = 0.0f;
    size_t mIdx = 0;
    static size_t sCount;
};
size_t AGeBarrel::sCount = 0;

class AGeGuard : public AActor {
  public:
    explicit AGeGuard(const SpawnParams& params) : AActor(params) {
        Name = "GoldenEye Guard";
        ResourceName = "n64kr:ge_guard";
        FVector loc = params.Location.value_or(FVector{ 0, 0, 0 });
        Pos[0] = loc.x * gTrackDirection;
        Pos[1] = loc.y;
        Pos[2] = loc.z;
        static const char* const kHeads[] = { "dave", "karl", "mark", "jim" };
        mIdx = sCount++;
        mDir = std::string("ge/guard_") + params.Skin.value_or(kHeads[mIdx % 4]);
        mYaw = params.Rotation.has_value() ? params.Rotation->yaw * kPi / 32768.0f : 0.0f;
        if (gTrackDirection < 0) {
            mYaw = -mYaw;
        }
        // Stagger so a row of guards doesn't move in step.
        mTimer = (s32) (frand() * 40);
    }

    bool IsMod() override {
        return true;
    }

    void Tick() override {
        const ModelInfo& info = model_info(mDir);
        if (!info.ok) {
            return;
        }
        if (mFlash > 0) {
            mFlash--;
        }
        if (mImpact > 0) {
            mImpact--;
        }

        Player* target = (mTarget >= 0) ? &gPlayers[mTarget] : nullptr;
        if ((target != nullptr) && (!racer_active(target) || Distance(target) > kRange * 1.2f)) {
            mTarget = -1;
            target = nullptr;
        }
        if (target != nullptr) {
            TurnToward(target, info);
        }

        switch (mState) {
            case State::Idle:
                mPose = 0;
                if ((--mTimer <= 0) && (gRaceState == RACE_IN_PROGRESS)) {
                    mTimer = 10;
                    mTarget = FindTarget();
                    if (mTarget >= 0) {
                        mState = State::Raising;
                        mTimer = 0;
                    }
                }
                break;
            case State::Raising:
                mPose = std::min(1 + ++mTimer / kRaiseTicksPerPose, info.aimPose);
                if (mPose >= info.aimPose) {
                    mState = State::Aiming;
                    mTimer = kAimTicks;
                }
                break;
            case State::Aiming:
                mPose = info.aimPose;
                if (target == nullptr) {
                    mTarget = FindTarget();
                    if (mTarget < 0) {
                        mState = State::Lowering;
                        mTimer = 0;
                    }
                } else if (--mTimer <= 0) {
                    mState = State::Firing;
                    mShots = 0;
                    mTimer = 0;
                    mHits = ShouldHit(target);
                }
                break;
            case State::Firing:
                mPose = info.aimPose;
                if (--mTimer <= 0) {
                    Fire(target, info);
                    mTimer = kShotGap;
                    if (++mShots >= kBurst) {
                        mState = State::Cooldown;
                        mTimer = kCooldown + (s32) (frand() * 40);
                    }
                }
                break;
            case State::Cooldown:
                mPose = info.aimPose;
                if (--mTimer <= 0) {
                    if (mTarget < 0 || gRaceState != RACE_IN_PROGRESS) {
                        mTarget = FindTarget();
                    }
                    if ((mTarget >= 0) && (gRaceState == RACE_IN_PROGRESS)) {
                        mState = State::Aiming;
                        mTimer = kAimTicks / 2;
                    } else {
                        mState = State::Lowering;
                        mTimer = 0;
                    }
                }
                break;
            case State::Lowering:
                mPose = std::max(info.aimPose - ++mTimer / kRaiseTicksPerPose, 0);
                if (mPose <= 0) {
                    mState = State::Idle;
                    mTimer = 30;
                }
                break;
        }
    }

    void Draw(Camera* camera) override {
        const ModelInfo& info = model_info(mDir);
        if (!info.ok) {
            return;
        }
        FrameInterpolation_RecordOpenChild("n64kr_ge_guard", (mIdx << 4) | camera->cameraId);
        Mat4 m;
        // Recoil: a small kick back along the aim while the flash is up.
        Vec3f at = { Pos[0], Pos[1], Pos[2] };
        if (mFlash > 0) {
            float aim = mYaw + info.aimYaw;
            at[0] -= std::sin(aim) * 0.6f;
            at[2] -= std::cos(aim) * 0.6f;
        }
        begin_models();
        yaw_matrix(m, at, mYaw, kGuardHeight / info.height);
        std::string pose = mDir + "/pose" + std::to_string(std::clamp(mPose, 0, info.poses - 1));
        draw_resource(pose.c_str(), m);
        if (mFlash > 0) {
            Vec3f muzzle;
            Muzzle(info, muzzle);
            begin_fx(255, 230, 160, 255);
            billboard_matrix(m, muzzle, camera, 5.0f + 2.0f * mFlash);
            draw_resource(kFxFlash, m);
            begin_fx(255, 200, 90, 220);
            if (beam_matrix(m, muzzle, mShotEnd, 1.6f)) {
                draw_resource(kFxTracer, m);
            }
        }
        if (mImpact > 0) {
            begin_fx(255, 250, 220, (u8) (40 * mImpact));
            billboard_matrix(m, mShotEnd, camera, 4.0f + 1.5f * (6 - mImpact));
            draw_resource(kFxBurst, m);
        }
        end_draw();
        FrameInterpolation_RecordCloseChild();
    }

  private:
    enum class State { Idle, Raising, Aiming, Firing, Cooldown, Lowering };

    float Distance(const Player* p) const {
        float dx = p->pos[0] - Pos[0], dz = p->pos[2] - Pos[2];
        return std::sqrt(dx * dx + dz * dz);
    }

    s32 FindTarget() const {
        s32 best = -1;
        float bestD = kRange;
        for (s32 i = 0; i < NUM_PLAYERS; i++) {
            const Player* p = &gPlayers[i];
            if (!racer_active(p) || (std::fabs(p->pos[1] - Pos[1]) > 120.0f)) {
                continue;
            }
            float d = Distance(p);
            if (d < bestD) {
                bestD = d;
                best = i;
            }
        }
        return best;
    }

    void TurnToward(const Player* p, const ModelInfo& info) {
        float want = std::atan2(p->pos[0] - Pos[0], p->pos[2] - Pos[2]) - info.aimYaw;
        float diff = std::remainder(want - mYaw, 2 * kPi);
        float step = kTurnRate * kPi / 32768.0f;
        mYaw += std::clamp(diff, -step, step);
    }

    bool ShouldHit(const Player* p) const {
        if ((p->effects & STAR_EFFECT) || (std::chrono::steady_clock::now() < sHitImmunity[p - gPlayers])) {
            return false;
        }
        float k = std::clamp((Distance(p) - 150.0f) / (kRange - 150.0f), 0.0f, 1.0f);
        return frand() < kHitChanceNear + (kHitChanceFar - kHitChanceNear) * k;
    }

    void Muzzle(const ModelInfo& info, Vec3f out) const {
        float s = kGuardHeight / info.height;
        float sn = std::sin(mYaw), cs = std::cos(mYaw);
        out[0] = Pos[0] + (info.muzzle[0] * cs + info.muzzle[2] * sn) * s;
        out[1] = Pos[1] + info.muzzle[1] * s;
        out[2] = Pos[2] + (-info.muzzle[0] * sn + info.muzzle[2] * cs) * s;
    }

    void Fire(Player* target, const ModelInfo& info) {
        Vec3f muzzle;
        Muzzle(info, muzzle);
        mFlash = 3;
        n64kr_sfx_play(kSfxKf7, muzzle, 0.75f, 0.95f + 0.1f * frand());
        if (target == nullptr) {
            // Shooting at where the racer was: straight ahead along the aim.
            float aim = mYaw + info.aimYaw;
            mShotEnd[0] = muzzle[0] + std::sin(aim) * kRange * 0.6f;
            mShotEnd[1] = muzzle[1] - 6.0f;
            mShotEnd[2] = muzzle[2] + std::cos(aim) * kRange * 0.6f;
            return;
        }
        const s32 id = (s32) (target - gPlayers);
        // The second shot of a hitting burst lands; the others go wide.
        bool lands = mHits && (mShots == 1);
        if (lands) {
            mShotEnd[0] = target->pos[0];
            mShotEnd[1] = target->pos[1] + 3.0f;
            mShotEnd[2] = target->pos[2];
            mImpact = 6;
            target->triggers |= HIT_BANANA_TRIGGER;
            sHitImmunity[id] = std::chrono::steady_clock::now() + kHitImmunity;
            n64kr_sfx_play(kSfxHit, mShotEnd, 1.0f, 1.0f);
        } else {
            mShotEnd[0] = target->pos[0] + (frand() - 0.5f) * 30.0f;
            mShotEnd[1] = target->pos[1] + frand() * 6.0f - 1.0f;
            mShotEnd[2] = target->pos[2] + (frand() - 0.5f) * 30.0f;
            if (target->effects & STAR_EFFECT) {
                mImpact = 4; // bounces off the star
            }
            n64kr_sfx_play((frand() < 0.5f) ? kSfxWhiz[std::rand() % 2] : kSfxRico[std::rand() % 2], mShotEnd, 0.7f,
                           0.9f + 0.2f * frand());
        }
    }

    std::string mDir;
    State mState = State::Idle;
    s32 mTimer = 0;
    s32 mPose = 0;
    s32 mTarget = -1;
    s32 mShots = 0;
    bool mHits = false;
    s32 mFlash = 0;
    s32 mImpact = 0;
    Vec3f mShotEnd = { 0, 0, 0 };
    float mYaw = 0.0f;
    size_t mIdx = 0;
    static size_t sCount;
};
size_t AGeGuard::sCount = 0;

} // namespace

void n64kr_register_actors(Registry<ActorInfo, const SpawnParams&>& r) {
    ActorInfo info;
    info = { .ResourceName = "n64kr:ge_barrel", .Name = "GoldenEye Barrel (N64KR)" };
    r.Add(info, AddActorToWorld<AGeBarrel>);
    info = { .ResourceName = "n64kr:ge_guard", .Name = "GoldenEye Guard (N64KR)" };
    r.Add(info, AddActorToWorld<AGeGuard>);
}
