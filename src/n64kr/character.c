#include <libultraship.h>
#include <defines.h>

#include "n64kr/character.h"

// clang-format off
static const N64krCharacter sCharacters[] = {
    //  name      voice   sprites hud     body    ai
    { "Mario",  MARIO,  MARIO,  MARIO,  MARIO,  MARIO  },
    { "Luigi",  LUIGI,  LUIGI,  LUIGI,  LUIGI,  LUIGI  },
    { "Yoshi",  YOSHI,  YOSHI,  YOSHI,  YOSHI,  YOSHI  },
    { "Toad",   TOAD,   TOAD,   TOAD,   TOAD,   TOAD   },
    { "DK",     DK,     DK,     DK,     DK,     DK     },
    { "Wario",  WARIO,  WARIO,  WARIO,  WARIO,  WARIO  },
    { "Peach",  PEACH,  PEACH,  PEACH,  PEACH,  PEACH  },
    { "Bowser", BOWSER, BOWSER, BOWSER, BOWSER, BOWSER },
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
