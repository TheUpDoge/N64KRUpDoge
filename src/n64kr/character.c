#include <libultraship.h>
#include <defines.h>

#include "n64kr/character.h"

// clang-format off
static const N64krCharacter sCharacters[] = {
    //  name      voice   sprites hud     body    ai      stats   kartFolder
    { "Mario",  MARIO,  MARIO,  MARIO,  MARIO,  MARIO,  MARIO,  NULL },
    { "Luigi",  LUIGI,  LUIGI,  LUIGI,  LUIGI,  LUIGI,  LUIGI,  NULL },
    { "Yoshi",  YOSHI,  YOSHI,  YOSHI,  YOSHI,  YOSHI,  YOSHI,  NULL },
    { "Toad",   TOAD,   TOAD,   TOAD,   TOAD,   TOAD,   TOAD,   NULL },
    { "DK",     DK,     DK,     DK,     DK,     DK,     DK,     NULL },
    { "Wario",  WARIO,  WARIO,  WARIO,  WARIO,  WARIO,  WARIO,  NULL },
    { "Peach",  PEACH,  PEACH,  PEACH,  PEACH,  PEACH,  PEACH,  NULL },
    { "Bowser", BOWSER, BOWSER, BOWSER, BOWSER, BOWSER, BOWSER, NULL },
    // Reserved: MK64's "no character" marker passes through as 8.
    { "(none)", N64KR_CHAR_NONE, N64KR_CHAR_NONE, N64KR_CHAR_NONE, N64KR_CHAR_NONE, N64KR_CHAR_NONE,
      N64KR_CHAR_NONE, NULL },
    // DKR. Own kart sprites (rendered from his DKR model, laid out like
    // Mario's); voice, HUD, size and stats borrowed from Toad, the closest
    // lightweight, until he has his own.
    { "Diddy",  TOAD,   MARIO,  TOAD,   TOAD,   TOAD,   TOAD,   "diddy_kart" },
};
// clang-format on

#define CHARACTER_COUNT ((s32) (sizeof(sCharacters) / sizeof(sCharacters[0])))

s32 n64kr_character_count(void) {
    return CHARACTER_COUNT;
}

const N64krCharacter* n64kr_get_character(s32 characterId) {
    if ((characterId < 0) || (characterId >= CHARACTER_COUNT)) {
        return NULL;
    }
    return &sCharacters[characterId];
}

s32 n64kr_char_voice(s32 characterId) {
    const N64krCharacter* c = n64kr_get_character(characterId);
    return (c != NULL) ? c->voice : characterId;
}

s32 n64kr_char_sprites(s32 characterId) {
    const N64krCharacter* c = n64kr_get_character(characterId);
    return (c != NULL) ? c->sprites : characterId;
}

s32 n64kr_char_hud(s32 characterId) {
    const N64krCharacter* c = n64kr_get_character(characterId);
    return (c != NULL) ? c->hud : characterId;
}

s32 n64kr_char_body(s32 characterId) {
    const N64krCharacter* c = n64kr_get_character(characterId);
    return (c != NULL) ? c->body : characterId;
}

s32 n64kr_char_ai(s32 characterId) {
    const N64krCharacter* c = n64kr_get_character(characterId);
    return (c != NULL) ? c->ai : characterId;
}

s32 n64kr_char_stats(s32 characterId) {
    const N64krCharacter* c = n64kr_get_character(characterId);
    return (c != NULL) ? c->stats : characterId;
}
