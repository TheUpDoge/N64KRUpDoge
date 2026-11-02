// N64KR: cups of N64KR tracks in the cup select (see cups.h).

#include <memory>
#include <string>
#include <vector>

#include <libultraship.h>
#include <spdlog/spdlog.h>

#include "port/Game.h"
#include "engine/Cup.h"
#include "engine/World.h"
#include "n64kr/cups.h"

extern "C" {
#include <defines.h>
#include "main.h"
#include "menus.h"
#include "menu_items.h"
extern char* gCupNames[];
}

namespace {

constexpr s32 kBattleCup = 4; // MK64's BATTLE_CUP; N64KR cups follow it

struct CupDef {
    const char* id;
    const char* name; // MK64's menus print cup names in lower case
    const char* subtitle;
    std::vector<std::string> tracks;
};

// The Rare Cup: N64KR's tracks so far all come from Rare games.
const CupDef kCups[] = {
    { "n64kr:rare_cup", "rare cup", "diddy kong racing + goldeneye 007",
      { "n64kr:dkr_ancient_lake", "n64kr:dkr_fossil_canyon", "n64kr:ge_dam_speedway", "n64kr:ge_dam_speedway_b" } },
};

std::vector<std::unique_ptr<Cup>> sCups;
std::vector<const CupDef*> sDefs; // parallel to sCups

const CupDef* current_def() {
    World* w = GetWorld();
    if (w == nullptr) {
        return nullptr;
    }
    const s32 i = (s32) w->GetCupIndex() - N64KR_FIRST_CUP;
    return ((i >= 0) && (i < (s32) sDefs.size())) ? sDefs[i] : nullptr;
}

// Source game of a track, from its resource name ("n64kr:<game>_<track>").
const char* source_game(const std::string& resource) {
    if (resource.rfind("n64kr:dkr_", 0) == 0) {
        return "diddy kong racing";
    }
    if (resource.rfind("n64kr:ge_", 0) == 0) {
        return "goldeneye 007";
    }
    return "";
}

char* track_name(const std::string& resource) {
    const TrackInfo* info = gTrackRegistry.GetInfo(resource);
    return const_cast<char*>((info != nullptr) ? info->Name.c_str() : resource.c_str());
}

void box(s32 x0, s32 y0, s32 x1, s32 y1, s32 r, s32 g, s32 b) {
    gDPPipeSync(gDisplayListHead++);
    gDPSetRenderMode(gDisplayListHead++, G_RM_OPA_SURF, G_RM_OPA_SURF2);
    gDPSetCombineMode(gDisplayListHead++, G_CC_DECALRGBA, G_CC_DECALRGBA);
    gDisplayListHead = draw_box_fill(gDisplayListHead, x0, y0, x1, y1, r, g, b, 255);
}

} // namespace

void n64kr_register_cups(void) {
    for (const auto& def : kCups) {
        bool ok = true;
        for (const auto& t : def.tracks) {
            if (gTrackRegistry.GetInfo(t) == nullptr) {
                SPDLOG_INFO("[n64kr] {} not added: track {} is not installed", def.id, t);
                ok = false;
                break;
            }
        }
        if (!ok) {
            continue;
        }
        sCups.push_back(std::make_unique<Cup>(def.id, def.name, def.tracks));
        sDefs.push_back(&def);
        GetWorld()->AddCup(sCups.back().get());
        SPDLOG_INFO("[n64kr] {} added as cup {}", def.id, GetWorld()->Cups.size() - 1);
    }
}

void n64kr_unregister_cups(void) {
    sCups.clear();
    sDefs.clear();
}

extern "C" s32 n64kr_cup_index_is_custom(s32 index) {
    const s32 i = index - N64KR_FIRST_CUP;
    return (i >= 0) && (i < (s32) sDefs.size());
}

extern "C" s32 n64kr_cup_is_custom(void) {
    return current_def() != nullptr;
}

extern "C" char* n64kr_cup_name(s32 index) {
    if ((index >= 0) && (index <= kBattleCup)) {
        return gCupNames[index];
    }
    const s32 i = index - N64KR_FIRST_CUP;
    return const_cast<char*>(((i >= 0) && (i < (s32) sDefs.size())) ? sDefs[i]->name : "");
}

extern "C" s32 n64kr_cup_selectable(s32 index) {
    World* w = GetWorld();
    if ((w == nullptr) || (index < 0) || (index >= (s32) w->Cups.size())) {
        return 0;
    }
    if (gModeSelection == BATTLE) {
        return index == kBattleCup;
    }
    if (index == kBattleCup) {
        return 0;
    }
    if (index >= N64KR_FIRST_CUP) {
        // Time trial records are saved per cup * 4 + course: no slots for N64KR cups.
        return gModeSelection != TIME_TRIALS;
    }
    return 1;
}

extern "C" void n64kr_cup_page_draw(void) {
    const CupDef* def = current_def();
    Cup* cup = (def != nullptr) ? GetWorld()->GetCurrentCup() : nullptr;
    if (cup == nullptr) {
        return;
    }

    // Banner where the four MK64 cup icons sit (they hide for N64KR cups,
    // func_800AB164): x 23..296, y 59..98; flashing while the cup is being chosen.
    if (gSubMenuSelection == SUB_MENU_MAP_SELECT_CUP) {
        gDPPipeSync(gDisplayListHead++);
        gDPSetRenderMode(gDisplayListHead++, G_RM_OPA_SURF, G_RM_OPA_SURF2);
        gDPSetCombineMode(gDisplayListHead++, G_CC_DECALRGBA, G_CC_DECALRGBA);
        gDisplayListHead = draw_flash_select_case_slow(gDisplayListHead, 0x17, 0x3B, 0x128, 0x62);
    } else {
        box(0x17, 0x3B, 0x128, 0x62, 255, 249, 220);
    }
    box(0x19, 0x3D, 0x126, 0x60, 1, 1, 1);
    set_text_color(TEXT_YELLOW);
    print_text1_center_mode_1(0xA0, 0x52, const_cast<char*>(def->name), 0, 1.0f, 1.0f);
    set_text_color(TEXT_GREEN);
    print_text1_center_mode_1(0xA0, 0x5D, const_cast<char*>(def->subtitle), 0, 0.5f, 0.5f);

    // Course list: MK64's boxes (func_800A8A98 at the course list item's
    // place) with the track names as text instead of the per-cup image.
    MenuItem* list = find_menu_items_dupe(MENU_ITEM_TYPE_058);
    if (list != nullptr) {
        func_800A8A98(list);
        for (s32 i = 0; i < (s32) cup->GetSize(); i++) {
            const bool lit = (i == (s32) cup->CursorPosition) && (gSubMenuSelection > SUB_MENU_MAP_SELECT_CUP) &&
                             (gModeSelection != GRAND_PRIX);
            set_text_color(lit ? TEXT_RED : TEXT_YELLOW);
            print_text_mode_1(list->column + D_800E7208[i][0].column + 6, list->row + D_800E7208[i][0].row + 13,
                              track_name(cup->mTracks[i]), 0, 0.6f, 0.7f);
        }
    }

    // Preview box (MK64 shows the course picture here, x 23..151, y 112..190):
    // the course under the cursor and its game.
    const std::string& track = cup->mTracks[cup->CursorPosition];
    box(0x17, 0x70, 0x97, 0xBE, 255, 249, 220);
    box(0x19, 0x72, 0x95, 0xBC, 1, 1, 1);
    set_text_color(TEXT_YELLOW);
    print_text1_center_mode_1(0x57, 0x88, track_name(track), 0, 0.55f, 0.65f);
    set_text_color(TEXT_GREEN);
    print_text1_center_mode_1(0x57, 0xA0, const_cast<char*>(source_game(track)), 0, 0.5f, 0.55f);
}
