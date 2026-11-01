# SLICE-7 — Vehicle select on the character select screen

Project: N64 Kart Racing (N64KR)
Date: 2026-11-01
Status: BUILT, not yet tested in game

## Design
Instead of a separate screen or the debug menu, the vehicle is picked on MK64's own character select screen, per human player: **R toggles Kart ↔ Hover**, and the choice shows as a label (yellow KART / green HOVER) at the bottom of the player's character cell. The choice persists between races.

The hovercraft now uses the **driver's own MK64 stats** for bounding box, hop and steering tables (`N64KR_STATS_OWN_CHARACTER`), instead of always Mario's. Its DKR stats still come from the "Hovercraft DKR Stats" menu choice.

## Code
- `src/n64kr/vehicle_select.{h,c}`: `n64kr_vehicle_select_input` (R toggles, cursor sound), `n64kr_vehicle_select_draw` (label via `set_text_color` + `print_text1_center_mode_1`), `n64kr_vehicle_choice`.
- `src/n64kr/vehicle_profile.{h,c}`: human players (`index < gPlayerCount`) who picked Hover get `N64KR_PROFILE_HOVERCRAFT`; the Player 1 debug override (Enhancements → N64KR) still wins when set. Hovercraft profile `statsId = N64KR_STATS_OWN_CHARACTER` → `n64kr_stats_id` returns `characterId`.

## Hooks in upstream files [VERIFIED: source]
| File | Hook |
|---|---|
| `src/menus.c` `player_select_menu_act` (PLAYER_SELECT_MENU_MAIN, after the "no cursor" early return) | `n64kr_vehicle_select_input(controllerIdx, btnAndStick)` |
| `src/menu_items.c` cursor case (`CHARACTER_SELECT_MENU_1P..4P_CURSOR`, after `render_cursor_player`) | `n64kr_vehicle_select_draw(arg0->column, arg0->row, temp_a0)` |

Character select facts: `gCharacterGridSelections[4]` (cursor cell, 0 = none), `gCharacterGridIsSelected[4]` (A confirmed), cells 0x40 × 0x4C, cursor drawn at the cell's top-left (`render_cursor_player`). R is unused on this screen.

## Build [VERIFIED: build output]
0 errors, 200 s (reconfigure for the new file).

## Test plan
1. Character select: each player's cell shows KART; R switches to HOVER (sound plays), again back to KART.
2. Race with HOVER: hover physics with that character; with KART: stock.
3. 2-player: each player's choice is independent.
4. Debug override "Own character" (0) so it doesn't override Player 1.
