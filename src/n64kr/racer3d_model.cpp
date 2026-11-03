// N64KR: 3D racer model info (model.json written by `n64kr dkr-racer-model`, tools/n64kr-rs),
// read once per character from the mod.

#include <string>
#include <unordered_map>

#include <libultraship.h>
#include <nlohmann/json.hpp>
#include <ship/Context.h>
#include "ship/resource/ResourceManager.h"
#include <spdlog/spdlog.h>

#include "n64kr/racer3d.h"

namespace {

struct Entry {
    N64krRacerModel model = {};
    std::string poses[N64KR_RACER3D_MAX_POSES];
    std::string wheels[N64KR_RACER3D_MAX_WHEELS];
};

std::unordered_map<std::string, Entry> sModels;

} // namespace

extern "C" const N64krRacerModel* n64kr_racer3d_model(const char* base) {
    auto it = sModels.find(base);
    if (it != sModels.end()) {
        return it->second.model.ok ? &it->second.model : nullptr;
    }
    Entry& entry = sModels[base];
    const std::string dir = base;
    auto file = Ship::Context::GetRawInstance()->GetResourceManager()->LoadFileProcess(dir + "/model.json");
    if ((file == nullptr) || (file->Buffer == nullptr)) {
        SPDLOG_WARN("[n64kr] 3D racer model {} not installed; using sprites", dir);
        return nullptr;
    }
    try {
        auto json = nlohmann::json::parse(std::string(file->Buffer->begin(), file->Buffer->end()));
        N64krRacerModel& m = entry.model;
        m.scale = json.value("world_scale", 0.12f);
        m.poseCount = std::min(json.value("pose_count", 1), N64KR_RACER3D_MAX_POSES);
        m.straightPose = std::min(json.value("straight_pose", 0), m.poseCount - 1);
        for (s32 p = 0; p < m.poseCount; p++) {
            entry.poses[p] = dir + "/pose" + std::to_string(p);
            m.poseDl[p] = entry.poses[p].c_str();
        }
        for (const auto& wheel : json.value("wheels", nlohmann::json::array())) {
            if (m.wheelCount >= N64KR_RACER3D_MAX_WHEELS) {
                break;
            }
            s32 k = m.wheelCount++;
            entry.wheels[k] = dir + "/" + wheel.value("dl", std::string());
            m.wheelDl[k] = entry.wheels[k].c_str();
            m.wheelSteers[k] = wheel.value("steers", false) ? 1 : 0;
            for (s32 i = 0; i < 3; i++) {
                m.wheelCentre[k][i] = wheel["centre"][i].get<f32>();
            }
        }
        m.ok = (m.poseCount > 0);
    } catch (const std::exception& e) {
        SPDLOG_ERROR("[n64kr] 3D racer model {}: bad model.json ({})", dir, e.what());
        entry.model.ok = 0;
    }
    return entry.model.ok ? &entry.model : nullptr;
}
