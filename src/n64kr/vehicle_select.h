#ifndef N64KR_VEHICLE_SELECT_H
#define N64KR_VEHICLE_SELECT_H

// N64KR: per-player vehicle choice on MK64's character select screen.
// Press R to switch between Kart and Hovercraft; the choice shows under the
// player's cursor and is applied when racers spawn (n64kr_assign_vehicle_profile).

#include <common_structs.h>

#ifdef __cplusplus
extern "C" {
#endif

#define N64KR_MAX_HUMANS 4

typedef enum {
    N64KR_VEHICLE_KART,
    N64KR_VEHICLE_HOVERCRAFT,
    N64KR_VEHICLE_COUNT
} N64krVehicle;

// From player_select_menu_act (src/menus.c), once per controller per frame.
void n64kr_vehicle_select_input(u16 controllerIdx, u16 buttonsPressed);

// From the character select cursor drawing (src/menu_items.c). (column, row)
// is the top-left of the character cell the cursor is on.
void n64kr_vehicle_select_draw(s32 column, s32 row, s32 playerIdx);

// Vehicle chosen by human player `playerIdx` (0..3).
s32 n64kr_vehicle_choice(s32 playerIdx);

#ifdef __cplusplus
}
#endif

#endif // N64KR_VEHICLE_SELECT_H
