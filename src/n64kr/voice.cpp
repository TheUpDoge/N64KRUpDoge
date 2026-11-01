// N64KR: own voice clips, played through the port's HMAS mixer (see voice.h).

#include <chrono>
#include <cmath>
#include <cstdlib>
#include <string>
#include <unordered_map>
#include <vector>

#include <libultraship.h>
#include <spdlog/spdlog.h>
#include <ship/Context.h>
#include "ship/resource/ResourceManager.h"
#include "port/Engine.h"
#include "port/audio/HMAS.h"

#include <defines.h>
#include "n64kr/character.h"
#include "n64kr/voice.h"

namespace {

// MK64 voice slots (the low nibble of a voice line) and the DKR clips used for
// them. Slot meanings read from the call sites [CONCLUSION]:
//   0x00 shell hit someone         0x01/0x02 (func_800C9250)   0x03 finished 5th or worse
//   0x04 podium ceremony           0x05 fell into water/lava    0x06 hit by an item or object
//   0x07 GP 1st place              0x08 fell/out of bounds      0x09 CPU uses an item
//   0x0A CPU boosting              0x0B CPU hit or tumbling     0x0D finished top 4, overtook,
//   0x0E character confirmed on the select screen (menus.c)     bumped scenery
enum ClipGroup { POSITIVE, NEGATIVE, WIN, FALL, SELECT };

const ClipGroup kSlotGroup[16] = {
    POSITIVE, POSITIVE, POSITIVE, NEGATIVE, WIN,      NEGATIVE, NEGATIVE, WIN,
    FALL,     POSITIVE, POSITIVE, NEGATIVE, POSITIVE, POSITIVE, SELECT,   POSITIVE,
};

const std::vector<std::string> kGroupClips[] = {
    { "positive1", "positive2", "positive3", "positive4", "positive5", "positive6", "positive7", "positive8" },
    { "negative1", "negative2", "negative3", "negative4", "negative5", "negative6", "negative7", "negative8" },
    { "laugh", "laugh2", "positive5", "positive8" },
    { "woah" },
    { "select" },
};

// Loudness: full within kNear of the listener, silent beyond kFar (MK64 world
// units, listener-relative positions). DKR voices are mastered hot.
constexpr float kGain = 0.6f;
constexpr float kNear = 100.0f;
constexpr float kFar = 1200.0f;
// One clip per character at a time: split screen requests a line once per camera.
constexpr auto kRepeatGap = std::chrono::milliseconds(150);
constexpr HMAS_AudioId kIdBase = 0x4E4B0000; // clear of the music ids (sequence numbers)

struct Clip {
    std::vector<uint8_t> wav; // HMAS decodes from this memory; must outlive the sound
    HMAS_AudioId id = 0;
    bool loaded = false;
};

struct Voice {
    std::unordered_map<std::string, Clip> clips;
    std::chrono::steady_clock::time_point lastStart;
};

std::unordered_map<s32, Voice> sVoices;

// Stable number for a clip name (its first place in kGroupClips), for its HMAS id.
size_t clip_number(const std::string& name) {
    size_t number = 0;
    for (const auto& group : kGroupClips) {
        for (const auto& n : group) {
            if (n == name) {
                return number;
            }
            number++;
        }
    }
    return number;
}

Clip* load_clip(s32 characterId, const char* voiceName, Voice& voice, const std::string& name, size_t index) {
    Clip& clip = voice.clips[name];
    if (clip.loaded) {
        return clip.id != 0 ? &clip : nullptr;
    }
    clip.loaded = true;
    const std::string path = std::string("sound/n64kr/") + voiceName + "/" + name + ".wav";
    auto file = Ship::Context::GetRawInstance()->GetResourceManager()->LoadFileProcess(path);
    if ((file == nullptr) || (file->Buffer == nullptr) || file->Buffer->empty()) {
        SPDLOG_WARN("[n64kr] voice clip {} not found", path);
        return nullptr;
    }
    clip.wav.assign(file->Buffer->begin(), file->Buffer->end());
    clip.id = kIdBase + (characterId << 8) + (HMAS_AudioId) index;
    GameEngine::Instance->gHMAS->RegisterSound(clip.id, clip.wav.data(), (uint32_t) clip.wav.size());
    return &clip;
}

} // namespace

extern "C" s32 n64kr_voice_play_sound(u32 soundBits, const f32* position, u8 cameraId) {
    if (((soundBits & 0xFFFFFF00) != 0x29008000) || ((soundBits & 0xFF) < 0x90)) {
        return 0;
    }
    const s32 characterId = (s32) ((soundBits & 0xFF) >> 4);
    const N64krCharacter* c = n64kr_get_character(characterId);
    if ((c == NULL) || (c->voiceName == NULL) || (GameEngine::Instance == nullptr) ||
        (GameEngine::Instance->gHMAS == nullptr)) {
        return 1; // not a stock sound id either: never hand it to MK64's banks
    }

    Voice& voice = sVoices[characterId];
    const auto now = std::chrono::steady_clock::now();
    if (now - voice.lastStart < kRepeatGap) {
        return 1;
    }

    // Random clip from the slot's group; skip clips the mod doesn't have.
    const auto& names = kGroupClips[kSlotGroup[soundBits & 0xF]];
    const size_t pick = (size_t) std::rand() % names.size();
    Clip* clip = nullptr;
    for (size_t tries = 0; (tries < names.size()) && (clip == nullptr); tries++) {
        const std::string& name = names[(pick + tries) % names.size()];
        clip = load_clip(characterId, c->voiceName, voice, name, clip_number(name));
    }
    if (clip == nullptr) {
        return 1;
    }

    // Ground distance only: MK64's listener-relative vectors keep y = 0
    // (func_800C1C88), and y = 100000 is its "sound finished" marker, written
    // into positions shared by all of a player's sounds (external.c
    // delete_sound_from_bank), so y says nothing about distance.
    float distance = 0.0f;
    if (position != NULL) {
        distance = std::sqrt(position[0] * position[0] + position[2] * position[2]);
    }
    float volume = 1.0f - (distance - kNear) / (kFar - kNear);
    volume = kGain * std::fmin(1.0f, std::fmax(0.0f, volume));
    // One debug line per voice request (how the SLICE-9 distance bug was found).
    SPDLOG_DEBUG("[n64kr] voice 0x{:08X} char {} slot {:X} camera {} distance {:.0f} volume {:.2f} -> {}", soundBits,
                characterId, soundBits & 0xF, cameraId, distance, volume, volume > 0.0f ? "play" : "too far");
    if (volume <= 0.0f) {
        return 1;
    }

    voice.lastStart = now;
    GameEngine::Instance->gHMAS->Play(HMAS_SFX, clip->id, false);
    GameEngine::Instance->gHMAS->SetVolume(HMAS_SFX, volume);
    SPDLOG_DEBUG("[n64kr] voice clip id {} playing {}", clip->id, GameEngine::Instance->gHMAS->IsPlaying(HMAS_SFX));
    return 1;
}
