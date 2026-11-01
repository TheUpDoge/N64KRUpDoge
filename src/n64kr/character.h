#ifndef N64KR_CHARACTER_H
#define N64KR_CHARACTER_H

// N64KR: character registry. A racer's characterId indexes this registry, not
// MK64's 8-entry tables directly. Each character names, per category, which
// stock MK64 character (MARIO 0 ... BOWSER 7, defines.h) to use for assets and
// behaviour it doesn't have of its own yet. Upstream code reads MK64 tables
// through the accessors below instead of [player->characterId].
//
// SLICE-8: the registry holds exactly MK64's 8 characters, each mapped to
// itself, so the game is unchanged. New characters (SLICE-9) add entries.
//
// Physics stats are not here: they come from the vehicle profile
// (n64kr/vehicle_profile.h).

#include <common_structs.h>

#ifdef __cplusplus
extern "C" {
#endif

// Capacity for per-character bookkeeping arrays (GP points etc.).
#define N64KR_MAX_CHARACTERS 16

typedef struct {
    const char* name;
    // Stock MK64 character used for each category, 0..7.
    s32 voice;   // voice clips and engine sound (characterId * 0x10 + sound, engine tuning)
    s32 sprites; // kart sprite frames, tumbles, palettes, wheel palettes (kart_dma.c, render_player.c)
    s32 hud;     // portrait, minimap icons, name text
    s32 body;    // body size (gCharacterSize), particle colours, racer-vs-racer bump weight
    s32 ai;      // CPU behaviour (item strategy, heavyweight exceptions)
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

#ifdef __cplusplus
}
#endif

#endif // N64KR_CHARACTER_H
