#ifndef N64KR_INTRO_H
#define N64KR_INTRO_H

// N64KR: the boot intro, played before the Harbour Masters intro:
// 1. a credit card ("Envisioned and produced by UpDoge, augmented and
//    created by Claude");
// 2. GoldenEye's gun barrel with its intro music: the dots, the barrel sliding
//    in, Bond walking, turning and firing, the blood, the barrel's wobble
//    (staged from the decomp's src/game/title.c, with Bond, the PP7, the
//    rifling, the music and the shot generated from your GoldenEye ROM by
//    tools/n64kr-rs ge-intro and ge-audio);
// 3. our own nuke flash: a white flash and shockwave that clear to the N64KR title.
// Any button skips it (with the Harbour Masters intro). Without the GoldenEye
// assets the gun barrel is left out. CVar gN64KR.Intro = 0 turns it off.

#include <common_structs.h>

#ifdef __cplusplus
extern "C" {
#endif

// Harbour Masters intro hooks (menu_items.c, N64KR HOOK): each returns 1 while
// the N64KR intro runs, in which case the caller skips the Harbour Masters
// intro's own tick or draw.
s32 n64kr_intro_tick(void);
s32 n64kr_intro_draw(void);

// A button was pressed on the intro screen (logo_intro_menu_act, N64KR HOOK):
// stops the intro's music; MK64 then fades to the next screen as it does for
// the Harbour Masters intro.
void n64kr_intro_skip(void);

#ifdef __cplusplus
}
#endif

#endif // N64KR_INTRO_H
