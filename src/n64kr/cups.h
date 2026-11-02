#ifndef N64KR_CUPS_H
#define N64KR_CUPS_H

// N64KR: cups of N64KR tracks in the cup select (recon/009-circuit-select).
//
// MK64's menus know four racing cups and the battle cup at index 4
// (BATTLE_CUP): course name lists and preview pictures are images per cup,
// cup icons sit in four slots, and time trial records are stored per
// cup * 4 + course. N64KR cups go after the battle cup (indices 5+), so the
// stock indices keep their meaning, and:
// - cup cycling skips the battle cup outside battle mode, and N64KR cups in
//   battle mode and time trials (their records have no save slots);
// - the cup select draws an N64KR cup as a page of its own: a banner in place
//   of the four MK64 cup icons, the course list as text, the source game in
//   the preview box;
// - lookups into MK64's per-cup tables go through the accessors below.

#include <common_structs.h>

#ifdef __cplusplus
extern "C" {
#endif

// The battle cup's index (MK64's BATTLE_CUP); N64KR cups come after it.
#define N64KR_FIRST_CUP 5

// 1 if `index` is an N64KR cup (index >= N64KR_FIRST_CUP and registered).
s32 n64kr_cup_index_is_custom(s32 index);
// 1 if the current cup is an N64KR cup.
s32 n64kr_cup_is_custom(void);

// Name of cup `index` for MK64's menus (gCupNames for the stock cups).
char* n64kr_cup_name(s32 index);

// World::NextCup / PreviousCup (N64KR HOOK): may the cursor stop on cup `index` in this mode?
s32 n64kr_cup_selectable(s32 index);

// Cup select screen: draws the current N64KR cup's page (banner, course list,
// preview box). Called from the map select banner's draw (menu_items.c
// N64KR HOOK); does nothing for MK64 cups.
void n64kr_cup_page_draw(void);

#ifdef __cplusplus
}

// Adds N64KR's cups after the stock ones (CustomEngineInit, N64KR HOOK). A cup
// is added only if all four of its tracks are installed.
void n64kr_register_cups(void);
void n64kr_unregister_cups(void);
#endif

#endif // N64KR_CUPS_H
