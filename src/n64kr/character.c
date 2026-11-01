#include <libultraship.h>
#include <defines.h>

#include <ctype.h>

#include "menu_items.h"
#include "n64kr/character.h"

// clang-format off
static const N64krCharacter sCharacters[] = {
    //  name      voice   sprites hud     body    ai      stats   kartFolder    hudName  voiceName
    { "Mario",  MARIO,  MARIO,  MARIO,  MARIO,  MARIO,  MARIO,  NULL,         NULL,    NULL },
    { "Luigi",  LUIGI,  LUIGI,  LUIGI,  LUIGI,  LUIGI,  LUIGI,  NULL,         NULL,    NULL },
    { "Yoshi",  YOSHI,  YOSHI,  YOSHI,  YOSHI,  YOSHI,  YOSHI,  NULL,         NULL,    NULL },
    { "Toad",   TOAD,   TOAD,   TOAD,   TOAD,   TOAD,   TOAD,   NULL,         NULL,    NULL },
    { "DK",     DK,     DK,     DK,     DK,     DK,     DK,     NULL,         NULL,    NULL },
    { "Wario",  WARIO,  WARIO,  WARIO,  WARIO,  WARIO,  WARIO,  NULL,         NULL,    NULL },
    { "Peach",  PEACH,  PEACH,  PEACH,  PEACH,  PEACH,  PEACH,  NULL,         NULL,    NULL },
    { "Bowser", BOWSER, BOWSER, BOWSER, BOWSER, BOWSER, BOWSER, NULL,         NULL,    NULL },
    // Reserved: MK64's "no character" marker passes through as 8.
    { "(none)", N64KR_CHAR_NONE, N64KR_CHAR_NONE, N64KR_CHAR_NONE, N64KR_CHAR_NONE, N64KR_CHAR_NONE,
      N64KR_CHAR_NONE, NULL, NULL, NULL },
    // DKR. Own kart sprites (rendered from his DKR model, laid out like
    // Mario's), HUD textures (laid out like Toad's) and voice clips (from DKR);
    // engine sound, size and stats borrowed from Toad, the closest lightweight.
    { "Diddy",  TOAD,   MARIO,  TOAD,   TOAD,   TOAD,   TOAD,   "diddy_kart", "diddy", "diddy" },
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

s32 n64kr_char_voice_line(s32 characterId) {
    const N64krCharacter* c = n64kr_get_character(characterId);
    if (c == NULL) {
        return characterId;
    }
    return (c->voiceName != NULL) ? characterId : c->voice;
}

const char* n64kr_char_name(s32 characterId) {
    static char sNames[N64KR_MAX_CHARACTERS][16];
    const N64krCharacter* c = n64kr_get_character(characterId);
    s32 i;

    if ((c == NULL) || (c->hudName == NULL) || (characterId >= N64KR_MAX_CHARACTERS)) {
        return D_800E76A8[n64kr_char_hud(characterId)];
    }
    if (sNames[characterId][0] == '\0') {
        for (i = 0; (c->name[i] != '\0') && (i < 15); i++) {
            sNames[characterId][i] = toupper((unsigned char) c->name[i]);
        }
    }
    return sNames[characterId];
}

s32 n64kr_char_stats(s32 characterId) {
    const N64krCharacter* c = n64kr_get_character(characterId);
    return (c != NULL) ? c->stats : characterId;
}
