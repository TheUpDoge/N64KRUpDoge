#ifndef N64KR_CHARACTER_H
#define N64KR_CHARACTER_H

// N64KR: character registry. A racer's characterId indexes this registry, not
// MK64's 8-entry tables directly. Each character names, per category, which
// stock MK64 character (MARIO 0 ... BOWSER 7, defines.h) to use for assets and
// behaviour it doesn't have of its own yet. Upstream code reads MK64 tables
// through the accessors below instead of [player->characterId].
//
// Ids 0..7 are MK64's 8 characters, each mapped to itself. Id 8 is reserved:
// MK64 uses 8 as a "no character" marker (time trial records, minimap), so it
// stays a pass-through entry. New characters start at 9 (SLICE-9: Diddy).
//
// The vehicle profile decides physics (n64kr/vehicle_profile.h); `stats` only
// names the stock stats a profile uses when it says "own character".

#include <common_structs.h>

#ifdef __cplusplus
extern "C" {
#endif

// Capacity for per-character bookkeeping arrays (GP points etc.).
#define N64KR_MAX_CHARACTERS 16

#define N64KR_CHAR_NONE 8 // MK64's "no character" marker
#define N64KR_CHAR_DIDDY 9

typedef struct {
    const char* name;
    // Stock MK64 character used for each category, 0..7.
    s32 voice;   // voice clips and engine sound (characterId * 0x10 + sound, engine tuning)
    s32 sprites; // kart sprite frames, tumbles, palettes, wheel palettes (kart_dma.c, render_player.c)
    s32 hud;     // portrait, minimap icons, name text
    s32 body;    // body size (gCharacterSize), particle colours, racer-vs-racer bump weight
    s32 ai;      // CPU behaviour (item strategy, heavyweight exceptions)
    s32 stats;   // MK64 kart stats for the "own character" vehicle profile
    // Kart sprite folder under textures/karts/ (a mod), laid out like the
    // `sprites` character's frames; NULL = use that character's own sprites.
    const char* kartFolder;
    // Suffix of the character's own HUD textures (textures/common_data/
    // common_texture_portrait_<hudName>, ..._minimap_kart_<hudName>,
    // ..._minimap_<hudName>, from a mod); NULL = the `hud` character's.
    const char* hudName;
} N64krCharacter;

s32 n64kr_character_count(void);
const N64krCharacter* n64kr_get_character(s32 characterId);

// Stock MK64 character id to use for each category. Ids outside the registry
// pass through unchanged (MK64 uses 8 as a "no character" marker in places).
s32 n64kr_char_voice(s32 characterId);
s32 n64kr_char_sprites(s32 characterId);
s32 n64kr_char_hud(s32 characterId);
s32 n64kr_char_body(s32 characterId);
s32 n64kr_char_ai(s32 characterId);
s32 n64kr_char_stats(s32 characterId);

// Kart sprite texture path for this character: the stock path from MK64's
// tables (looked up with n64kr_char_sprites), moved to the character's own
// kartFolder if it has one. The returned string stays valid.
const char* n64kr_kart_texture(s32 characterId, const char* stockPath);

// HUD texture path (portrait, minimap icons): the stock path from MK64's tables
// (looked up with n64kr_char_hud), moved to the character's hudName if set.
const char* n64kr_hud_texture(s32 characterId, const char* stockPath);

// Name shown in results and records: MK64's text for stock characters, the
// registry name in capitals otherwise.
const char* n64kr_char_name(s32 characterId);

#ifdef __cplusplus
}
#endif

#endif // N64KR_CHARACTER_H
