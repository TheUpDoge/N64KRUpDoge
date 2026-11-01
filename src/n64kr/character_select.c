#include <libultraship.h>
#include <defines.h>
#include <sounds.h>

#include "audio/external.h"
#include "main.h"
#include "menus.h"
#include "menu_items.h"
#include "n64kr/character.h"
#include "n64kr/character_select.h"
#include "n64kr/vehicle_select.h"

// Alternate character per human player; -1 = none (race as the stock cell).
static s32 sAlternate[N64KR_MAX_HUMANS] = { -1, -1, -1, -1 };

// Character cells on the select screen are 0x40 x 0x4C (src/menu_items.c); the
// vehicle label sits at the bottom (vehicle_select.c), the name at the top.
#define CELL_WIDTH 0x40
#define NAME_ROW_OFFSET 0x04

void n64kr_character_select_input(u16 controllerIdx, u16 buttonsPressed) {
    if ((controllerIdx >= N64KR_MAX_HUMANS) || !(buttonsPressed & L_TRIG)) {
        return;
    }
    sAlternate[controllerIdx] = (sAlternate[controllerIdx] < 0) ? N64KR_CHAR_DIDDY : -1;
    play_sound2(SOUND_MENU_CURSOR_MOVE);
}

void n64kr_character_select_draw(s32 column, s32 row, s32 playerIdx) {
    const N64krCharacter* c;

    if ((playerIdx < 0) || (playerIdx >= N64KR_MAX_HUMANS) || (sAlternate[playerIdx] < 0)) {
        return;
    }
    c = n64kr_get_character(sAlternate[playerIdx]);
    if (c == NULL) {
        return;
    }
    set_text_color(TEXT_RED);
    print_text1_center_mode_1(column + (CELL_WIDTH / 2), row + NAME_ROW_OFFSET, (char*) c->name, 0, 0.6f, 0.6f);
}

s32 n64kr_human_character(s32 playerIdx) {
    if ((playerIdx < 0) || (playerIdx >= N64KR_MAX_HUMANS)) {
        return MARIO;
    }
    return (sAlternate[playerIdx] >= 0) ? sAlternate[playerIdx] : gCharacterSelections[playerIdx];
}

u16 n64kr_spawn_character(s32 playerIndex, u16 characterId, s32 playerType) {
    // Only a human racer spawned as their own stock selection; the podium
    // ceremony spawns everyone as CPUs with final ids already in place.
    if (!(playerType & PLAYER_HUMAN) || (playerIndex < 0) || (playerIndex >= N64KR_MAX_HUMANS) ||
        (characterId != (u16) gCharacterSelections[playerIndex])) {
        return characterId;
    }
    return (u16) n64kr_human_character(playerIndex);
}

s32 n64kr_gp_seed_character(s32 index) {
    s32 ids[NUM_PLAYERS];
    s32 i;
    s32 j;

    for (i = 0; i < NUM_PLAYERS; i++) {
        s32 id = gPlayers[i].characterId;
        for (j = i; (j > 0) && (ids[j - 1] > id); j--) {
            ids[j] = ids[j - 1];
        }
        ids[j] = id;
    }
    return ((index >= 0) && (index < NUM_PLAYERS)) ? ids[index] : index;
}
