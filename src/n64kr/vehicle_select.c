#include <libultraship.h>
#include <defines.h>
#include <sounds.h>

#include "audio/external.h"
#include "menu_items.h"
#include "n64kr/vehicle_select.h"

static s32 sVehicleChoice[N64KR_MAX_HUMANS] = { N64KR_VEHICLE_KART, N64KR_VEHICLE_KART, N64KR_VEHICLE_KART,
                                                N64KR_VEHICLE_KART };

static const char* sVehicleLabels[N64KR_VEHICLE_COUNT] = {
    "KART",  // N64KR_VEHICLE_KART
    "HOVER", // N64KR_VEHICLE_HOVERCRAFT
};

// Character cells on the select screen are 0x40 x 0x4C (src/menu_items.c).
#define CELL_WIDTH 0x40
#define LABEL_ROW_OFFSET 0x46

void n64kr_vehicle_select_input(u16 controllerIdx, u16 buttonsPressed) {
    if ((controllerIdx >= N64KR_MAX_HUMANS) || !(buttonsPressed & R_TRIG)) {
        return;
    }
    sVehicleChoice[controllerIdx] = (sVehicleChoice[controllerIdx] + 1) % N64KR_VEHICLE_COUNT;
    play_sound2(SOUND_MENU_CURSOR_MOVE);
}

void n64kr_vehicle_select_draw(s32 column, s32 row, s32 playerIdx) {
    s32 choice;

    if ((playerIdx < 0) || (playerIdx >= N64KR_MAX_HUMANS)) {
        return;
    }
    choice = sVehicleChoice[playerIdx];
    set_text_color((choice == N64KR_VEHICLE_HOVERCRAFT) ? TEXT_GREEN : TEXT_YELLOW);
    print_text1_center_mode_1(column + (CELL_WIDTH / 2), row + LABEL_ROW_OFFSET, (char*) sVehicleLabels[choice], 0,
                              0.5f, 0.5f);
}

s32 n64kr_vehicle_choice(s32 playerIdx) {
    if ((playerIdx < 0) || (playerIdx >= N64KR_MAX_HUMANS)) {
        return N64KR_VEHICLE_KART;
    }
    return sVehicleChoice[playerIdx];
}
