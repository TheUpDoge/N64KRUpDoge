#include <ctype.h>
#include <libultraship.h>
#include <defines.h>
#include <sounds.h>

#include "audio/external.h"
#include "main.h"
#include "menus.h"
#include "menu_items.h"
#include "racing/math_util.h"
#include "n64kr/character.h"
#include "n64kr/character_select.h"
#include "n64kr/vehicle_select.h"

#define GRID_PAGES 2
#define GRID_CELLS 8

// Page 1, the DKR page: N64KR character per cell, -1 = empty.
static const s32 sDkrPage[GRID_CELLS] = {
    N64KR_CHAR_DIDDY, N64KR_CHAR_BANJO, N64KR_CHAR_CONKER, -1, -1, -1, -1, -1,
};

static s32 sViewPage = 0;                       // page the grid is showing
static s32 sPlayerPage[N64KR_MAX_HUMANS];       // page each human's cell is on
static s32 sAlternate[N64KR_MAX_HUMANS] = { -1, -1, -1, -1 }; // page character; -1 = stock
static s32 sCurrentController = 0;              // controller the movement code is handling
static s8 sCellShownPage[GRID_CELLS];           // page each cell's face sequence shows
// CPU seats DKR characters took over: stock id replaced -> DKR character.
static s32 sCpuSeatStock[GRID_CELLS];
static s32 sCpuSeatCharacter[GRID_CELLS];
static s32 sCpuSeats = 0;

// The DKR page's cells: face and blank nameplate from each character's mod
// (tools/n64kr/blender_dkr_racer_hud.py, textures named after the hudName);
// the name is printed over the plate.
#define FACE(name) "__OTR__textures/player_selection/" name "_face_00"
#define PLATE(name) "__OTR__textures/texture_tkmk00/texture_name_" name
#define CELL_TEXTURES(name)                                                         \
    { { { 0, (u64*) FACE(name), 64, 64, 0, 0, 0 }, { 0, NULL, 0, 0, 0, 0, 0 } },     \
      { { 0, (u64*) PLATE(name), 64, 12, 0, 64, 0 }, { 0, NULL, 0, 0, 0, 0, 0 } } }
#define DKR_CELLS 3
static MenuTexture sCellTextures[DKR_CELLS][2][2] = { CELL_TEXTURES("diddy"), CELL_TEXTURES("banjo"),
                                                      CELL_TEXTURES("conker") };
static MkAnimation sCellFaceAnim[DKR_CELLS][2] = {
    { { sCellTextures[0][0], 100 }, { NULL, 0 } },
    { { sCellTextures[1][0], 100 }, { NULL, 0 } },
    { { sCellTextures[2][0], 100 }, { NULL, 0 } },
};

// Character on (page, cell 1..8); -1 = nothing there.
static s32 page_cell_character(s32 page, s32 gridId) {
    if ((gridId < 1) || (gridId > GRID_CELLS)) {
        return -1;
    }
    if (page == 0) {
        return sCharacterGridOrder[gridId - 1];
    }
    return sDkrPage[gridId - 1];
}

static s32 spot_free_on_page(s32 page, s32 gridId) {
    size_t i;

    if (page_cell_character(page, gridId) < 0) {
        return false;
    }
    if (CVarGetInteger("gUniqueCharacterSelections", true) == false) {
        return true;
    }
    for (i = 0; i < N64KR_MAX_HUMANS; i++) {
        if ((gridId == gCharacterGridSelections[i]) && (sPlayerPage[i] == page)) {
            return false;
        }
    }
    return true;
}

void n64kr_grid_reset(void) {
    s32 i;

    sViewPage = 0;
    for (i = 0; i < N64KR_MAX_HUMANS; i++) {
        sPlayerPage[i] = 0;
        sAlternate[i] = -1;
    }
    for (i = 0; i < GRID_CELLS; i++) {
        sCellShownPage[i] = 0;
    }
}

s32 n64kr_grid_spot_free(s32 gridId) {
    return spot_free_on_page(sPlayerPage[sCurrentController], gridId);
}

void n64kr_character_select_input(u16 controllerIdx, u16 buttonsPressed) {
    // Entry cells per page edge: same row as the cell the player left first.
    static const s32 fromLeftTop[] = { 1, 5, 2, 6, 3, 7, 4, 8 };
    static const s32 fromLeftBottom[] = { 5, 1, 6, 2, 7, 3, 8, 4 };
    static const s32 fromRightTop[] = { 4, 8, 3, 7, 2, 6, 1, 5 };
    static const s32 fromRightBottom[] = { 8, 4, 7, 3, 6, 2, 5, 1 };
    const s32* candidates;
    s32 cell, page, target, i;

    if (controllerIdx >= N64KR_MAX_HUMANS) {
        return;
    }
    sCurrentController = controllerIdx;
    if (gCharacterGridIsSelected[controllerIdx]) {
        return;
    }
    cell = gCharacterGridSelections[controllerIdx];
    page = sPlayerPage[controllerIdx];
    if ((buttonsPressed & CONT_RIGHT) && ((cell == 4) || (cell == 8)) && (page + 1 < GRID_PAGES)) {
        page += 1;
        candidates = (cell == 4) ? fromLeftTop : fromLeftBottom;
    } else if ((buttonsPressed & CONT_LEFT) && ((cell == 1) || (cell == 5)) && (page > 0)) {
        page -= 1;
        candidates = (cell == 1) ? fromRightTop : fromRightBottom;
    } else {
        return;
    }
    target = 0;
    for (i = 0; i < GRID_CELLS; i++) {
        if (spot_free_on_page(page, candidates[i])) {
            target = candidates[i];
            break;
        }
    }
    if (target == 0) {
        return; // the page has no free cell for this player
    }
    sPlayerPage[controllerIdx] = page;
    sViewPage = page;
    gCharacterGridSelections[controllerIdx] = target;
    play_sound2(0x49008000); // the grid's cursor-move sound (menus.c)
}

s32 n64kr_grid_commit(s32 playerIdx, s32 gridId) {
    s32 pageCharacter;

    if ((playerIdx < 0) || (playerIdx >= N64KR_MAX_HUMANS) || (gridId < 1) || (gridId > GRID_CELLS)) {
        return MARIO;
    }
    pageCharacter = page_cell_character(sPlayerPage[playerIdx], gridId);
    sAlternate[playerIdx] = ((sPlayerPage[playerIdx] != 0) && (pageCharacter >= 0)) ? pageCharacter : -1;
    // Menu bookkeeping (CPU rosters, GP rivals) keeps the stock id of the
    // same cell; only the spawned racer gets the page character's id.
    return sCharacterGridOrder[gridId - 1];
}

s32 n64kr_grid_cell_update(MenuItem* item, s32 cell) {
    if ((cell < 0) || (cell >= GRID_CELLS)) {
        return 0;
    }
    if (sCellShownPage[cell] != sViewPage) {
        sCellShownPage[cell] = sViewPage;
        if (sViewPage == 0) {
            item->subState = 0; // restart the stock idle/blink state machine
            func_8009A594(item->D_8018DEE0_index, 0, D_800E8360[cell]);
        } else if ((sViewPage == 1) && (cell < DKR_CELLS) && (sDkrPage[cell] >= 0)) {
            func_8009A594(item->D_8018DEE0_index, 0, sCellFaceAnim[cell]);
        }
    }
    return sViewPage != 0;
}

s32 n64kr_grid_cell_draw(MenuItem* item, s32 cell) {
    if (cell == 0) { // one label per frame: the page being shown
        set_text_color((sViewPage == 0) ? TEXT_BLUE : TEXT_YELLOW);
        print_text1_center_mode_1(0xA0, 0x1C, (sViewPage == 0) ? "MK64" : "DKR", 0, 0.5f, 0.5f);
    }
    if (sViewPage == 0) {
        return 0;
    }
    if ((cell < DKR_CELLS) && (sDkrPage[cell] >= 0)) {
        const N64krCharacter* c = n64kr_get_character(sDkrPage[cell]);
        char label[16];
        s32 i;
        func_800A12BC(item, sCellTextures[cell][1]);
        func_8009A76C(item->D_8018DEE0_index, item->column, item->row, item->param1);
        for (i = 0; (c != NULL) && (c->name[i] != '\0') && (i < 15); i++) {
            label[i] = (char) toupper((unsigned char) c->name[i]);
        }
        label[i] = '\0';
        set_text_color(TEXT_YELLOW);
        print_text1_center_mode_1(item->column + 0x20, item->row + 0x40, label, 0, 0.45f, 0.45f);
    }
    return 1; // empty cells draw nothing
}

s32 n64kr_grid_player_on_view(s32 playerIdx) {
    if ((playerIdx < 0) || (playerIdx >= N64KR_MAX_HUMANS)) {
        return true;
    }
    return sPlayerPage[playerIdx] == sViewPage;
}

s32 n64kr_human_character(s32 playerIdx) {
    if ((playerIdx < 0) || (playerIdx >= N64KR_MAX_HUMANS)) {
        return MARIO;
    }
    return (sAlternate[playerIdx] >= 0) ? sAlternate[playerIdx] : gCharacterSelections[playerIdx];
}

s32 n64kr_select_voice(s32 playerIdx, s32 stockId) {
    if ((playerIdx < 0) || (playerIdx >= N64KR_MAX_HUMANS) || (sAlternate[playerIdx] < 0)) {
        return stockId;
    }
    return n64kr_char_voice_line(sAlternate[playerIdx]);
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

// CPU racers: every DKR page character no human picked takes a random stock
// CPU seat (MK64's roster, rivals and GP bookkeeping keep the stock id of
// the seat; n64kr_rival_character maps it back).
void n64kr_cpu_roster(s16* roster, s32 count) {
    s32 cell;
    s32 i;

    sCpuSeats = 0;
    for (cell = 0; (cell < GRID_CELLS) && (sCpuSeats < count); cell++) {
        const s32 dkr = sDkrPage[cell];
        s32 taken = 0;
        s32 seat;
        s32 tries;
        if (dkr < 0) {
            continue;
        }
        for (i = 0; (i < gPlayerCount) && (i < N64KR_MAX_HUMANS); i++) {
            taken |= (n64kr_human_character(i) == dkr);
        }
        if (taken) {
            continue;
        }
        // A random seat not already swapped.
        seat = random_int((u16) count);
        for (tries = 0; tries < count; tries++, seat = (seat + 1) % count) {
            s32 used = 0;
            for (i = 0; i < sCpuSeats; i++) {
                used |= (sCpuSeatStock[i] == roster[seat]);
            }
            if (!used) {
                break;
            }
        }
        if (tries == count) {
            break;
        }
        sCpuSeatStock[sCpuSeats] = roster[seat];
        sCpuSeatCharacter[sCpuSeats] = dkr;
        sCpuSeats++;
        roster[seat] = (s16) dkr;
    }
}

s32 n64kr_rival_character(s32 stockId) {
    s32 i;
    for (i = 0; i < sCpuSeats; i++) {
        if (sCpuSeatStock[i] == stockId) {
            return sCpuSeatCharacter[i];
        }
    }
    return stockId;
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
