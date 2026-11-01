#ifndef N64KR_CHARACTER_SELECT_H
#define N64KR_CHARACTER_SELECT_H

// N64KR: picking a non-MK64 character on MK64's 8-cell select screen, until
// the grid itself grows. Press L to put Diddy in the seat of the cell you are
// on; his name shows over your cursor. Menu code keeps the stock cell id in
// gCharacterSelections (so CPU rosters and GP rivals treat him as that
// character's seat); only the racer spawned for that player gets his id.

#include <common_structs.h>

#ifdef __cplusplus
extern "C" {
#endif

// From player_select_menu_act (src/menus.c), once per controller per frame.
void n64kr_character_select_input(u16 controllerIdx, u16 buttonsPressed);

// From the character select cursor drawing (src/menu_items.c). (column, row)
// is the top-left of the character cell the cursor is on.
void n64kr_character_select_draw(s32 column, s32 row, s32 playerIdx);

// Character human player `playerIdx` (0..3) races as: their alternate if one
// is chosen, else their stock selection.
s32 n64kr_human_character(s32 playerIdx);

// From the select screen's confirm voice (player_select_menu_act, src/menus.c):
// voice index for the character human player `playerIdx` confirms on stock
// cell `stockId` (n64kr_char_voice_line of their alternate, if any).
s32 n64kr_select_voice(s32 playerIdx, s32 stockId);

// From spawn_player (src/spawn_players.c): the character a racer spawns as.
// Swaps a human's stock selection for their alternate; anything else is kept.
u16 n64kr_spawn_character(s32 playerIndex, u16 characterId, s32 playerType);

// GP standings seed (func_800A3A10, src/menu_items.c): the racers' character
// ids in ascending order, which is 0..7 for a stock roster.
s32 n64kr_gp_seed_character(s32 index);

#ifdef __cplusplus
}
#endif

#endif // N64KR_CHARACTER_SELECT_H
