// N64KR: asset paths for characters with their own textures (from mods).
//
// Kart sprites:
// MK64's kart texture tables (src/kart_dma.c) hold resource paths such as
// "__OTR__textures/karts/mario_kart/mario_kart_frame000_wheel0". A character
// with a kartFolder reuses the layout of its `sprites` character's tables and
// swaps the folder name, e.g. ".../diddy_kart/diddy_kart_frame000_wheel0". The
// frames come from a mod (tools/n64kr/blender_dkr_racer_sprites.py).
//
// HUD: portrait and minimap paths end in the `hud` character's name
// (common_texture_portrait_toad); a character with a hudName swaps that suffix
// (common_texture_portrait_diddy). From tools/n64kr/blender_dkr_racer_hud.py.

#include <cstring>
#include <string>
#include <unordered_map>

#include <libultraship.h>
#include <defines.h>
#include "n64kr/character.h"

// Stock folder per MK64 character id (MARIO 0 ... BOWSER 7, defines.h).
static const char* sStockFolders[] = {
    "mario_kart", "luigi_kart", "yoshi_kart",  "toad_kart",
    "donkeykong_kart", "wario_kart", "peach_kart", "bowser_kart",
};

// Stock HUD texture suffixes per MK64 character id (DK's differ by texture).
static const char* sStockHudNames[][2] = {
    { "mario", NULL }, { "luigi", NULL }, { "yoshi", NULL },       { "toad", NULL },
    { "donkey_kong", "dk" }, { "wario", NULL }, { "peach", NULL }, { "bowser", NULL },
};

extern "C" const char* n64kr_hud_texture(s32 characterId, const char* stockPath) {
    static std::unordered_map<s32, std::unordered_map<const char*, std::string>> sPaths;
    const N64krCharacter* c = n64kr_get_character(characterId);

    if ((stockPath == NULL) || (c == NULL) || (c->hudName == NULL) || (c->hud < 0) ||
        (c->hud >= (s32) (sizeof(sStockHudNames) / sizeof(sStockHudNames[0])))) {
        return stockPath;
    }

    std::string& path = sPaths[characterId][stockPath];
    if (path.empty()) {
        path = stockPath;
        for (const char* stock : sStockHudNames[c->hud]) {
            const std::string suffix = std::string("_") + (stock != NULL ? stock : "");
            if ((stock != NULL) && (path.size() > suffix.size()) &&
                (path.compare(path.size() - suffix.size(), suffix.size(), suffix) == 0)) {
                path.replace(path.size() - suffix.size(), suffix.size(), std::string("_") + c->hudName);
                break;
            }
        }
    }
    return path.c_str();
}

extern "C" const char* n64kr_kart_texture(s32 characterId, const char* stockPath) {
    static std::unordered_map<s32, std::unordered_map<const char*, std::string>> sPaths;
    const N64krCharacter* c = n64kr_get_character(characterId);

    if ((stockPath == NULL) || (c == NULL) || (c->kartFolder == NULL) || (c->sprites < 0) ||
        (c->sprites >= (s32) (sizeof(sStockFolders) / sizeof(sStockFolders[0])))) {
        return stockPath;
    }

    std::string& path = sPaths[characterId][stockPath];
    if (path.empty()) {
        const std::string from = sStockFolders[c->sprites];
        path = stockPath;
        for (size_t at = path.find(from); at != std::string::npos; at = path.find(from, at)) {
            path.replace(at, from.size(), c->kartFolder);
            at += strlen(c->kartFolder);
        }
    }
    return path.c_str();
}
