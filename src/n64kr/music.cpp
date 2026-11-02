// N64KR: other games' music through HMAS (see music.h).

#include <string>
#include <unordered_map>
#include <vector>

#include <libultraship.h>
#include <nlohmann/json.hpp>
#include <spdlog/spdlog.h>
#include <ship/Context.h>
#include "ship/resource/ResourceManager.h"
#include "port/Engine.h"
#include "port/audio/HMAS.h"

#include "n64kr/music.h"

namespace {

struct Song {
    u16 id;
    const char* file;    // music/n64kr/<file>.wav
    u16 finalLap;        // song for the final lap, or 0
};

const Song kSongs[] = {
    { N64KR_MUSIC_GE_DAM, "ge_dam", N64KR_MUSIC_GE_DAMX },
    { N64KR_MUSIC_GE_DAMX, "ge_damx", 0 },
    { N64KR_MUSIC_GE_INTRO, "ge_intro", 0 },
    { N64KR_MUSIC_GE_RUNWAY, "ge_runway", 0 },
    { N64KR_MUSIC_GE_FACILITY, "ge_facility", 0 },
};

// HMAS decodes from these buffers while the songs play, so they live for the session.
std::unordered_map<u16, std::vector<uint8_t>> sData;
u16 sCurrent = 0; // the N64KR song play_sequence last started, or 0
// MK64 holds the music for the final lap fanfare this many frames (func_800CA49C).
constexpr uint32_t kFanfareFrames = 110;

const Song* find(u16 id) {
    for (const auto& s : kSongs) {
        if (s.id == id) {
            return &s;
        }
    }
    return nullptr;
}

} // namespace

extern "C" void n64kr_music_prepare(u16 id) {
    const Song* song = find(id);
    sCurrent = (song != nullptr) ? id : 0;
    if ((song == nullptr) || (GameEngine::Instance == nullptr) || (GameEngine::Instance->gHMAS == nullptr) ||
        GameEngine::Instance->gHMAS->IsIDRegistered(id) || (sData.count(id) != 0)) {
        return;
    }
    auto* rm = Ship::Context::GetRawInstance()->GetResourceManager().get();
    const std::string base = std::string("music/n64kr/") + song->file;
    auto wav = rm->LoadFileProcess(base + ".wav");
    if ((wav == nullptr) || (wav->Buffer == nullptr) || wav->Buffer->empty()) {
        SPDLOG_WARN("[n64kr] song {} not installed (run tools/n64kr-rs ge-audio)", base);
        sData[id]; // don't look again
        return;
    }
    HMAS_Info info;
    info.name = song->file;
    auto meta = rm->LoadFileProcess(base + ".json");
    if ((meta != nullptr) && (meta->Buffer != nullptr)) {
        try {
            auto json = nlohmann::json::parse(std::string(meta->Buffer->begin(), meta->Buffer->end()));
            info.name = json.value("name", info.name);
            info.author = json.value("author", "");
            if (json.contains("loop")) {
                info.loop.start = json["loop"].value("start", (int64_t) -1);
                info.loop.end = json["loop"].value("end", (int64_t) -1);
            }
        } catch (const std::exception& e) {
            SPDLOG_WARN("[n64kr] {}.json: {}", base, e.what());
        }
    }
    auto& data = sData[id];
    data.assign(wav->Buffer->begin(), wav->Buffer->end());
    // Loop points are written in the WAV's frames; HMAS decodes at the
    // mixer's rate and miniaudio counts loop points in decoded frames.
    const uint32_t wavRate = (data.size() >= 28) ? (uint32_t) data[24] | ((uint32_t) data[25] << 8) |
                                                       ((uint32_t) data[26] << 16) | ((uint32_t) data[27] << 24)
                                                 : 0;
    const uint32_t mixRate = GameEngine_GetSampleRate();
    if ((wavRate != 0) && (mixRate != 0) && (wavRate != mixRate) && (info.loop.start >= 0)) {
        info.loop.start = info.loop.start * mixRate / wavRate;
        info.loop.end = info.loop.end * mixRate / wavRate;
    }
    GameEngine::Instance->gHMAS->RegisterSound(id, data.data(), (uint32_t) data.size(), info);
    SPDLOG_INFO("[n64kr] song {} registered as sequence {:#x} (loop {}..{})", base, id, info.loop.start,
                info.loop.end);
}

extern "C" s32 n64kr_music_final_lap(void) {
    const Song* song = find(sCurrent);
    if ((song == nullptr) || !HMAS_IsPlaying(HMAS_MUSIC)) {
        return 0;
    }
    if (song->finalLap != 0) {
        const u16 next = song->finalLap;
        n64kr_music_prepare(next);
        if (HMAS_IsIDRegistered(next)) {
            HMAS_Play(HMAS_MUSIC, next, true);
            HMAS_AddEffect(HMAS_MUSIC, HMAS_EFFECT_PAUSE, HMAS_INSTANT, kFanfareFrames, 0.0f);
            HMAS_AddEffect(HMAS_MUSIC, HMAS_EFFECT_PAUSE, HMAS_INSTANT, 1, 1.0f);
        }
    }
    return 1;
}
