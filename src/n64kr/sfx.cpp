// N64KR: positional sound effects with their own voices (see sfx.h).

#include <algorithm>
#include <cmath>
#include <cstring>
#include <memory>
#include <mutex>
#include <string>
#include <unordered_map>
#include <vector>

#include <libultraship.h>
#include <spdlog/spdlog.h>
#include <ship/Context.h>
#include "ship/resource/ResourceManager.h"

#include "n64kr/sfx.h"

extern "C" {
#include <defines.h>
#include "camera.h"
#include "main.h"
uint32_t GameEngine_GetSampleRate();
}

namespace {

// Loudness: full within kNear of the nearest camera, silent beyond kFar (MK64 units).
constexpr float kNear = 80.0f;
constexpr float kFar = 1500.0f;
constexpr size_t kMaxVoices = 24;
constexpr size_t kMaxPerClip = 4;  // a row of guards firing doesn't stack up the same shot
// Overall level against MK64's own sound (user test: too loud).
constexpr float kMasterGain = 0.45f;
// Soft limiter knee: the port turns the HMAS buffer into 16-bit by casting
// (Engine.cpp), so anything past +-1.0 wraps around and crackles.
constexpr float kKnee = 0.75f;

struct Clip {
    std::vector<float> left, right; // at the output rate
    bool ok = false;
};

struct Voice {
    std::shared_ptr<const Clip> clip;
    double pos = 0.0;
    double step = 1.0;
    float gainL = 0.0f, gainR = 0.0f;
};

std::mutex sMutex;
std::unordered_map<std::string, std::shared_ptr<Clip>> sClips;
std::vector<Voice> sVoices;
uint32_t sRate = 0;

uint32_t output_rate() {
    if (sRate == 0) {
        sRate = GameEngine_GetSampleRate();
    }
    return (sRate != 0) ? sRate : 44100;
}

// Minimal RIFF reader: PCM 16-bit, mono or stereo; resampled to the output rate.
std::shared_ptr<Clip> load_clip(const std::string& path) {
    auto it = sClips.find(path);
    if (it != sClips.end()) {
        return it->second;
    }
    auto clip = std::make_shared<Clip>();
    sClips[path] = clip;
    auto file = Ship::Context::GetRawInstance()->GetResourceManager()->LoadFileProcess(path);
    if ((file == nullptr) || (file->Buffer == nullptr) || (file->Buffer->size() < 44)) {
        SPDLOG_WARN("[n64kr] sound {} not found", path);
        return clip;
    }
    const auto& b = *file->Buffer;
    auto rd16 = [&](size_t o) { return (uint32_t) (uint8_t) b[o] | ((uint32_t) (uint8_t) b[o + 1] << 8); };
    auto rd32 = [&](size_t o) { return rd16(o) | (rd16(o + 2) << 16); };
    if (std::memcmp(b.data(), "RIFF", 4) != 0 || std::memcmp(b.data() + 8, "WAVE", 4) != 0) {
        SPDLOG_WARN("[n64kr] sound {} is not a WAV", path);
        return clip;
    }
    uint32_t channels = 1, rate = 22050, bits = 16;
    size_t data = 0, dataLen = 0;
    for (size_t o = 12; o + 8 <= b.size();) {
        uint32_t len = rd32(o + 4);
        if (std::memcmp(b.data() + o, "fmt ", 4) == 0 && o + 24 <= b.size()) {
            channels = rd16(o + 10);
            rate = rd32(o + 12);
            bits = rd16(o + 22);
        } else if (std::memcmp(b.data() + o, "data", 4) == 0) {
            data = o + 8;
            dataLen = std::min<size_t>(len, b.size() - data);
        }
        o += 8 + len + (len & 1);
    }
    if ((data == 0) || (bits != 16) || (channels < 1) || (channels > 2) || (rate == 0)) {
        SPDLOG_WARN("[n64kr] sound {}: unsupported WAV ({} ch, {} bits)", path, channels, bits);
        return clip;
    }
    const size_t frames = dataLen / (2 * channels);
    auto sample = [&](size_t f, uint32_t c) {
        size_t o = data + (f * channels + c) * 2;
        return (float) (int16_t) rd16(o) / 32768.0f;
    };
    const double step = (double) rate / output_rate();
    const size_t outFrames = (size_t) (frames / step);
    clip->left.resize(outFrames);
    clip->right.resize(outFrames);
    for (size_t i = 0; i < outFrames; i++) {
        double p = i * step;
        size_t f = (size_t) p;
        float t = (float) (p - f);
        size_t g = std::min(f + 1, frames - 1);
        for (uint32_t c = 0; c < 2; c++) {
            uint32_t ch = std::min(c, channels - 1);
            float v = sample(f, ch) * (1.0f - t) + sample(g, ch) * t;
            (c == 0 ? clip->left : clip->right)[i] = v;
        }
    }
    clip->ok = true;
    return clip;
}

// Distance (x/z) and direction to the nearest active camera: 1P has one
// camera, 2P two, 3P/4P four (cameras[], camera.h).
void listener_gains(const f32* pos, float volume, float* gl, float* gr) {
    if (pos == nullptr) {
        *gl = *gr = volume * 0.7071f;
        return;
    }
    s32 count = (gActiveScreenMode == SCREEN_MODE_1P) ? 1 : ((gActiveScreenMode == SCREEN_MODE_3P_4P_SPLITSCREEN) ? 4 : 2);
    float best = 1e30f, pan = 0.0f;
    for (s32 i = 0; i < count; i++) {
        const Camera* cam = &cameras[i];
        float dx = pos[0] - cam->pos[0], dz = pos[2] - cam->pos[2];
        float d = std::sqrt(dx * dx + dz * dz);
        if (d < best) {
            best = d;
            float fx = cam->lookAt[0] - cam->pos[0], fz = cam->lookAt[2] - cam->pos[2];
            float fl = std::sqrt(fx * fx + fz * fz);
            if ((fl > 0.0f) && (d > 1.0f)) {
                // Sideways component of the source direction: +1 = to the camera's right.
                pan = (fx * dz - fz * dx) / (fl * d);
            } else {
                pan = 0.0f;
            }
        }
    }
    float v = 1.0f - (best - kNear) / (kFar - kNear);
    v = volume * std::clamp(v, 0.0f, 1.0f);
    float p = (std::clamp(pan, -1.0f, 1.0f) * 0.6f + 1.0f) * 0.25f * 3.14159265f; // keep some of each side
    *gl = v * std::cos(p);
    *gr = v * std::sin(p);
}

} // namespace

extern "C" void n64kr_sfx_play(const char* path, const f32* pos, f32 volume, f32 pitch) {
    if (path == nullptr) {
        return;
    }
    float gl, gr;
    listener_gains(pos, volume, &gl, &gr);
    if (gl + gr < 0.01f) {
        return;
    }
    std::lock_guard<std::mutex> lock(sMutex);
    auto clip = load_clip(path);
    if (!clip->ok) {
        return;
    }
    size_t same = 0;
    for (const auto& v : sVoices) {
        same += (v.clip == clip) ? 1 : 0;
    }
    if (same >= kMaxPerClip) {
        for (auto it = sVoices.begin(); it != sVoices.end(); ++it) {
            if (it->clip == clip) {
                sVoices.erase(it); // drop the oldest of this clip
                break;
            }
        }
    }
    if (sVoices.size() >= kMaxVoices) {
        sVoices.erase(sVoices.begin()); // drop the oldest
    }
    Voice v;
    v.clip = clip;
    v.step = (pitch > 0.0f) ? pitch : 1.0;
    v.gainL = gl * kMasterGain;
    v.gainR = gr * kMasterGain;
    sVoices.push_back(v);
}

extern "C" void n64kr_sfx_mix(f32* stereo, u32 frames) {
    std::lock_guard<std::mutex> lock(sMutex);
    for (auto& v : sVoices) {
        const Clip& c = *v.clip;
        const size_t len = c.left.size();
        for (u32 f = 0; f < frames; f++) {
            size_t i = (size_t) v.pos;
            if (i >= len) {
                break;
            }
            stereo[f * 2] += c.left[i] * v.gainL;
            stereo[f * 2 + 1] += c.right[i] * v.gainR;
            v.pos += v.step;
        }
    }
    sVoices.erase(std::remove_if(sVoices.begin(), sVoices.end(),
                                 [](const Voice& v) { return (size_t) v.pos >= v.clip->left.size(); }),
                  sVoices.end());
    // Limit the whole buffer (HMAS music and voice clips too): unchanged below
    // the knee, then eased so it never reaches 1.0.
    for (u32 i = 0; i < frames * 2; i++) {
        float x = stereo[i];
        float a = std::fabs(x);
        if (a > kKnee) {
            float over = (a - kKnee) / (1.0f - kKnee);
            a = kKnee + (1.0f - kKnee) * over / (1.0f + over);
            stereo[i] = (x < 0.0f) ? -a : a;
        }
    }
}

extern "C" void n64kr_sfx_stop_all(void) {
    std::lock_guard<std::mutex> lock(sMutex);
    sVoices.clear();
}
