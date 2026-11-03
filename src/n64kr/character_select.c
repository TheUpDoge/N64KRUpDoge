#include <ctype.h>
#include <stdio.h>
#include <string.h>
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
#define ARROW_HALF 7 // menu arrow: half its height at the base, in pixels

// Page 1, the DKR page: N64KR character per cell, -1 = empty. A character
// listed here needs its mod's faces and nameplate, found by its registry
// hudName (docs/n64kr/TUTORIAL_ADD_DKR_RACER.md).
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

// Animated faces from each character's mod (tools/n64kr/blender_dkr_racer_faces.py,
// <hudName>_face_NN, 256 px PNGs drawn in MK64's 64x64 face slot): 00 idle,
// 01-02 blink (half shut, shut), 03-14 picked (DKR's horn taunt, turning to the
// viewer), 15-22 lost (DKR's crash). Sequences and timing follow MK64's faces
// (src/data/textures.c, e.g. Toad: 1-tick frames, idle 50, blink then 10).
// Blank nameplate: texture_name_<hudName> (tools/n64kr/blender_dkr_racer_hud.py);
// the name is printed over it.
#define FACE_BLINK 1
#define FACE_PICKED 3
#define FACE_PICKED_FRAMES 12
#define FACE_LOST 15
#define FACE_LOST_FRAMES 8
#define FACE_FRAMES 23
#define FACE_HOLD 30000 // ticks: a last frame that stays up (sequences loop at their end)
#define BLINK_END 3       // sequence index where each blink is over
#define DOUBLE_BLINK_END 7

typedef struct {
    char path[FACE_FRAMES][64];
    MenuTexture tex[FACE_FRAMES][2];
    char platePath[64];
    MenuTexture plate[2];
    MkAnimation idle[2];
    MkAnimation blink[BLINK_END + 2];
    MkAnimation doubleBlink[DOUBLE_BLINK_END + 2];
    MkAnimation picked[FACE_PICKED_FRAMES + 1];
    MkAnimation hold[2];
    MkAnimation unpick[FACE_PICKED_FRAMES + 1];
    MkAnimation lost[FACE_LOST_FRAMES + 1];
} DkrFaces;

static DkrFaces sFaces[GRID_CELLS];
static s32 sFacesReady = false;

static void face_frame(MkAnimation* step, DkrFaces* faces, s32 frame, s32 ticks) {
    step->mk64Texture = faces->tex[frame];
    step->frame_length = ticks;
}

static void build_faces(void) {
    static const s32 blink[][2] = { { 1, 2 }, { 2, 3 }, { 1, 2 }, { 0, 10 } };
    static const s32 doubleBlink[][2] = { { 1, 2 }, { 2, 3 }, { 1, 2 }, { 0, 3 },
                                          { 1, 2 }, { 2, 3 }, { 1, 2 }, { 0, 10 } };
    s32 cell, i;

    if (sFacesReady) {
        return;
    }
    memset(sFaces, 0, sizeof(sFaces)); // zero entries end each list
    for (cell = 0; cell < GRID_CELLS; cell++) {
        const N64krCharacter* c = n64kr_get_character(sDkrPage[cell]);
        DkrFaces* f = &sFaces[cell];

        if ((c == NULL) || (c->hudName == NULL)) {
            continue;
        }
        for (i = 0; i < FACE_FRAMES; i++) {
            snprintf(f->path[i], sizeof(f->path[i]), "__OTR__textures/player_selection/%s_face_%02d", c->hudName,
                     (int) i);
            f->tex[i][0].textureData = (u64*) f->path[i];
            f->tex[i][0].width = 64;
            f->tex[i][0].height = 64;
        }
        snprintf(f->platePath, sizeof(f->platePath), "__OTR__textures/texture_tkmk00/texture_name_%s", c->hudName);
        f->plate[0].textureData = (u64*) f->platePath;
        f->plate[0].width = 64;
        f->plate[0].height = 12;
        f->plate[0].dY = 64;
        face_frame(&f->idle[0], f, 0, 50);
        for (i = 0; i <= BLINK_END; i++) {
            face_frame(&f->blink[i], f, blink[i][0], blink[i][1]);
        }
        for (i = 0; i <= DOUBLE_BLINK_END; i++) {
            face_frame(&f->doubleBlink[i], f, doubleBlink[i][0], doubleBlink[i][1]);
        }
        for (i = 0; i < FACE_PICKED_FRAMES; i++) {
            face_frame(&f->picked[i], f, FACE_PICKED + i, 2);
            face_frame(&f->unpick[i], f, FACE_PICKED + FACE_PICKED_FRAMES - 1 - i, 1);
        }
        face_frame(&f->hold[0], f, FACE_PICKED + FACE_PICKED_FRAMES - 1, FACE_HOLD);
        // Lost: the crash animation backwards, upright to slumped, and stay slumped.
        for (i = 0; i < FACE_LOST_FRAMES; i++) {
            face_frame(&f->lost[i], f, FACE_LOST + FACE_LOST_FRAMES - 1 - i, 2);
        }
        f->lost[FACE_LOST_FRAMES - 1].frame_length = FACE_HOLD;
    }
    sFacesReady = true;
}

// DKR page cell of a character; -1 if it has none.
static s32 dkr_cell(s32 characterId) {
    s32 cell;

    for (cell = 0; (cell < GRID_CELLS) && (characterId >= 0); cell++) {
        if (sDkrPage[cell] == characterId) {
            return cell;
        }
    }
    return -1;
}

static MkAnimation* face_anim(s32 cell, s32 which) {
    build_faces();
    switch (which) {
        case N64KR_FACE_PICKED:
            return sFaces[cell].picked;
        case N64KR_FACE_HOLD:
            return sFaces[cell].hold;
        case N64KR_FACE_LOST:
            return sFaces[cell].lost;
        default:
            return sFaces[cell].idle;
    }
}

MkAnimation* n64kr_char_face(s32 characterId, s32 which, MkAnimation* stock) {
    s32 cell = dkr_cell(characterId);
    return (cell >= 0) ? face_anim(cell, which) : stock;
}

s32 n64kr_char_face_picked_end(s32 characterId, s32 stock) {
    return (dkr_cell(characterId) >= 0) ? (FACE_PICKED_FRAMES - 1) : stock;
}

MenuTexture* n64kr_char_nameplate(s32 characterId, MenuTexture* stock) {
    s32 cell = dkr_cell(characterId);
    build_faces();
    return (cell >= 0) ? sFaces[cell].plate : stock;
}

void n64kr_char_nameplate_label(s32 characterId, s32 column, s32 row) {
    const N64krCharacter* c = n64kr_get_character(characterId);
    char label[16];
    s32 i;

    if ((dkr_cell(characterId) < 0) || (c == NULL)) {
        return;
    }
    for (i = 0; (c->name[i] != '\0') && (i < 15); i++) {
        label[i] = (char) toupper((unsigned char) c->name[i]);
    }
    label[i] = '\0';
    // On the plate (it spans row + 0x40 .. 0x4C; the text's y is its foot).
    set_text_color(TEXT_YELLOW);
    print_text1_center_mode_1(column + 0x20, row + 0x4A, label, 0, 0.45f, 0.45f);
}

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

// A DKR cell's face, run the way func_800AA69C (src/menu_items.c) runs a stock
// cell's: idle with random single and double blinks; the picked animation when
// the player on the cell confirms, then held; played backwards when they back
// out (from where it got to, if they change their mind half way).
static void dkr_face_update(MenuItem* item, s32 cell) {
    const s32 end = FACE_PICKED_FRAMES - 1;
    DkrFaces* f = &sFaces[cell];
    s32 index = item->D_8018DEE0_index;
    s32 at = D_8018DEE0[index].sequenceIndex;
    s32 player = func_800AAFCC(cell);
    s32 picked = (player >= 0) && gCharacterGridIsSelected[player];
    s32 unpicked = (player >= 0) && !gCharacterGridIsSelected[player];
    s32 roll;

    switch (item->subState) {
        case 0:
            if (picked) {
                item->subState = 1;
                func_8009A594(index, 0, f->picked);
            } else {
                roll = random_int(200);
                if (roll >= 198) {
                    item->subState = 4;
                    func_8009A594(index, 0, f->blink);
                } else if (roll >= 197) {
                    item->subState = 5;
                    func_8009A594(index, 0, f->doubleBlink);
                }
            }
            break;
        case 1:
            if (at >= end) {
                item->subState = 2;
                func_8009A594(index, 0, f->hold);
            } else if (unpicked) {
                item->subState = 3;
                func_8009A594(index, end - at, f->unpick);
            }
            break;
        case 2:
            if (unpicked) {
                item->subState = 3;
                func_8009A594(index, 0, f->unpick);
            }
            break;
        case 3:
            if (at >= end) {
                item->subState = 0;
                func_8009A594(index, 0, f->idle);
            } else if (picked) {
                item->subState = 1;
                func_8009A594(index, end - at, f->picked);
            }
            break;
        case 4:
        case 5:
            if (picked) {
                item->subState = 1;
                func_8009A594(index, 0, f->picked);
            } else if (at >= ((item->subState == 4) ? BLINK_END : DOUBLE_BLINK_END)) {
                item->subState = 0;
                func_8009A594(index, 0, f->idle);
            }
            break;
        default:
            break;
    }
}

s32 n64kr_grid_cell_update(MenuItem* item, s32 cell) {
    s32 dkrCell;

    if ((cell < 0) || (cell >= GRID_CELLS)) {
        return 0;
    }
    dkrCell = (sViewPage == 1) && (sDkrPage[cell] >= 0);
    if (sCellShownPage[cell] != sViewPage) {
        sCellShownPage[cell] = sViewPage;
        item->subState = 0; // restart the idle/blink state machine
        if (sViewPage == 0) {
            func_8009A594(item->D_8018DEE0_index, 0, D_800E8360[cell]);
        } else if (dkrCell) {
            build_faces();
            func_8009A594(item->D_8018DEE0_index, 0, sFaces[cell].idle);
        }
    } else if (dkrCell) {
        dkr_face_update(item, cell);
    }
    return sViewPage != 0;
}

void n64kr_menu_arrow(s32 x, s32 y, s32 dir) {
    // Bob a couple of pixels toward the way it points, about twice a second.
    s32 bob = ((gGlobalTimer >> 4) & 1) ? 2 * dir : 0;
    s32 tip = x + bob;
    s32 i;

    // One column per pixel (draw_box_fill's corners are inclusive), 1 px at
    // the tip growing 2 px per column: a black triangle one column longer and
    // 1 px bigger all round, yellow inside it.
    for (i = 0; i <= ARROW_HALF + 1; i++) {
        s32 col = tip - (dir * i);
        gDisplayListHead = draw_box_fill(gDisplayListHead, col, y - i, col, y + i, 0, 0, 0, 255);
    }
    for (i = 1; i <= ARROW_HALF; i++) {
        s32 col = tip - (dir * i);
        gDisplayListHead = draw_box_fill(gDisplayListHead, col, y - i + 1, col, y + i - 1, 255, 220, 0, 255);
    }
}

// Arrows in the margins beside the grid (cells span x 24..295, rows y 63..221),
// pointing to the other page with its name under them, while players are
// choosing: the DKR page is otherwise only found by pushing past the edge.
static void page_arrows(void) {
    const s32 y = 142; // between the two rows
    if (gPlayerSelectMenuSelection != PLAYER_SELECT_MENU_MAIN) {
        return;
    }
    if (sViewPage + 1 < GRID_PAGES) {
        n64kr_menu_arrow(312, y, 1);
        set_text_color(TEXT_YELLOW);
        print_text1_center_mode_1(306, y + 0x16, "DKR", 0, 0.45f, 0.45f);
    }
    if (sViewPage > 0) {
        n64kr_menu_arrow(7, y, -1);
        set_text_color(TEXT_BLUE);
        print_text1_center_mode_1(13, y + 0x16, "MK64", 0, 0.45f, 0.45f);
    }
}

s32 n64kr_grid_cell_draw(MenuItem* item, s32 cell) {
    if (cell == 0) { // once per frame: the page being shown (under the banner), arrows to the others
        set_text_color((sViewPage == 0) ? TEXT_BLUE : TEXT_YELLOW);
        print_text1_center_mode_1(0xA0, 0x3A, (sViewPage == 0) ? "MK64" : "DKR", 0, 0.5f, 0.5f);
        page_arrows();
    }
    if (sViewPage == 0) {
        return 0;
    }
    if (sDkrPage[cell] >= 0) {
        build_faces();
        func_800A12BC(item, sFaces[cell].plate);
        func_8009A76C(item->D_8018DEE0_index, item->column, item->row, item->param1);
        // States 1 and 3 fade the cell out (func_800A12BC), as unpicked cells do
        // once everyone has chosen: no name floating where it was.
        if ((item->state != 1) && (item->state != 3)) {
            n64kr_char_nameplate_label(sDkrPage[cell], item->column, item->row);
        }
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
