#ifndef N64KR_CHARACTER_SELECT_H
#define N64KR_CHARACTER_SELECT_H

// N64KR: pages on MK64's character select screen.
//
// The 8-cell grid stays as it is; it now shows one page at a time. Page 0 is
// MK64's roster, page 1 is the DKR page (Diddy, with room for more). Pushing
// right past the grid's right edge turns the page; pushing left past the left
// edge turns back. Menu state (gCharacterSelections, the grid tables) keeps
// stock ids 0..7, so CPU rosters and GP rivals are untouched: a player on a
// DKR cell races the page's character through spawn_player (the racer's
// characterId becomes the registry id, e.g. Diddy 9).
//
// Each human has their own page; the grid draws the page of whoever turned it
// last, and cursors of players on the other page are hidden until it turns
// back. CPUs can race DKR characters too (n64kr_cpu_roster).

#include <common_structs.h>
#include "menu_items.h"

#ifdef __cplusplus
extern "C" {
#endif

// From player_select_menu_act (src/menus.c), once per controller per frame:
// page turning at the grid edges.
void n64kr_character_select_input(u16 controllerIdx, u16 buttonsPressed);

// From the select screen init (src/menus.c): back to page 0, no alternates.
void n64kr_grid_reset(void);

// Replaces is_character_spot_free's body (src/menus.c): a cell can be landed
// on when the mover's page has a character there and no player on that page
// holds the cell.
s32 n64kr_grid_spot_free(s32 gridId);

// From the commit at the end of player_select_menu_act: the stock id menu
// bookkeeping keeps for this player's (page, cell); remembers the page
// character the player actually picked.
s32 n64kr_grid_commit(s32 playerIdx, s32 gridId);

// From the character cell update and draw switches (src/menu_items.c),
// cell = item type - CHARACTER_SELECT_MENU_MARIO (0..7). Both return 1 when
// the page module handled the cell (a non-stock page is shown), 0 to run the
// stock path.
s32 n64kr_grid_cell_update(MenuItem* item, s32 cell);
s32 n64kr_grid_cell_draw(MenuItem* item, s32 cell);

// A small pulsing yellow arrow with a black outline, tip at (x, y), pointing
// right (dir 1) or left (-1): shows where more menu pages are (the grid's
// pages here, the cup select's N64KR cups in n64kr/cups.cpp).
void n64kr_menu_arrow(s32 x, s32 y, s32 dir);

// Whether this player's cursor (and cell highlight) is on the page being
// shown. From the cursor draw and func_800AAFCC (src/menu_items.c).
s32 n64kr_grid_player_on_view(s32 playerIdx);

// Character human player `playerIdx` (0..3) races as: the page character
// under their cursor, or their stock selection on page 0.
s32 n64kr_human_character(s32 playerIdx);

// From the select screen's confirm voice (player_select_menu_act, src/menus.c):
// voice index for the character human player `playerIdx` confirms on stock
// cell `stockId` (n64kr_char_voice_line of their page character, if any).
s32 n64kr_select_voice(s32 playerIdx, s32 stockId);

// From spawn_player (src/spawn_players.c): the character a racer spawns as.
// Swaps a human's stock selection for their page character; anything else is
// kept.
u16 n64kr_spawn_character(s32 playerIndex, u16 characterId, s32 playerType);

// From the CPU roster fills (src/spawn_players.c): one CPU races a DKR
// character when no human does.
void n64kr_cpu_roster(s16* roster, s32 count);

// From the GP rival lookup (code_80005FD0.c): a rival whose seat a DKR CPU
// took hands the rival role to that CPU.
s32 n64kr_rival_character(s32 stockId);

// The cup award screen's portrait (MENU_ITEM_TYPE_130 and func_800AF270,
// src/menu_items.c) shows the winner's select-screen face and nameplate. A
// DKR page character has its own animated faces (idle, picked = celebrate,
// its held last frame, lost = defeat) and a blank plate with the name printed
// over it, as on its cell; `stock` is returned for everyone else.
enum { N64KR_FACE_IDLE, N64KR_FACE_PICKED, N64KR_FACE_HOLD, N64KR_FACE_LOST };
MkAnimation* n64kr_char_face(s32 characterId, s32 which, MkAnimation* stock);
// Sequence index where the picked animation ends (D_800E8440 for stock faces).
s32 n64kr_char_face_picked_end(s32 characterId, s32 stock);
MenuTexture* n64kr_char_nameplate(s32 characterId, MenuTexture* stock);
void n64kr_char_nameplate_label(s32 characterId, s32 column, s32 row);

// GP standings seed (func_800A3A10, src/menu_items.c): the racers' character
// ids in ascending order, which is 0..7 for a stock roster.
s32 n64kr_gp_seed_character(s32 index);

#ifdef __cplusplus
}
#endif

#endif // N64KR_CHARACTER_SELECT_H
