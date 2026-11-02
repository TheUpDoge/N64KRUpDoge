# RECON-009 — Circuit select: the Rare Cup

Project: N64 Kart Racing (N64KR)
Date: 2026-11-02
Status: built (engine build, 35 s launch: cup registered); in-game test pending

## Goal (user)
A circuit selection page so N64KR's tracks can be raced without Debug Mode. User, on filling the fourth slot: "we may also make another map … a diddy kong map or even something from f zero or crusin usa … anything you choose". F-Zero X and Cruis'n USA aren't among the user's ROMs, so the fourth track is DKR's **Fossil Canyon**, and with every track from a Rare game the cup is the **Rare Cup**: Ancient Lake, Fossil Canyon, Dam Speedway, Dam Speedway B.

## How the cup select works [VERIFIED: source]
- SpaghettiKart keeps cups in `World::Cups` (`src/engine/World.cpp`): Mushroom, Flower, Star, Special, Battle (`Game.cpp` CustomEngineInit). A `Cup` holds exactly 4 track resource names (`Cup.cpp` throws otherwise); races load `GetCurrentCup()->GetTrack()` (`TrackBrowser_SetTrackFromCup`).
- `World::NextCup` stepped up to `Cups.size() - 2` outside battle mode (assumes the battle cup is last); `PreviousCup` stepped down without a mode check. Battle mode sets the cup to `BATTLE_CUP` = 4 (`menus.c` COURSE_SELECT_MENU), so the battle cup must stay at index 4.
- The cup select screen (`menu_items.c`) is built for 4 cups: icons are items 0x53–0x56 at fixed slots (`D_800E7148`: y 59, x 23/93/162/232, 64×39), the course list is one image per cup (items 0x58–0x5B, `gMenuTexturesTrackSelection`) with 4 boxes (`D_800E7208`: x 157..296, rows from y 112), the preview picture comes from `D_800E7E34[gCupCourseOrder[cup][n]]` (items 0x5F–0x62, x 23..151, y 112..190), cup names from `gCupNames[cup]` (5 entries).
- Save data: time trial records and ghosts are keyed by `cup * 4 + course` (`save.c`: `cupRecords[track / 4]`, `onlyBestTimeTrialRecords[track / 8]`; written after time trials, `func_800B5020`): a 6th cup would write past the records. Grand Prix trophies are 2 bits per cup in a u8 per CC (`func_800B54EC`/`func_800B5508`): cup 5 reads 0 and its writes fall off the byte (harmless; the battle cup at 4 already works this way).
- `gCupSelection` is never updated in SpaghettiKart (stays 0), so `gCupCourseOrder[gCupSelection]` lookups stay in range.

## Design
- N64KR cups go after the battle cup (`src/n64kr/cups.{h,cpp}`, index 5+), added only if all 4 tracks are installed (`n64kr_register_cups`, Game.cpp hook).
- Cup cycling (`World::NextCup`/`PreviousCup`, hook) asks `n64kr_cup_selectable`: battle mode → battle cup only; otherwise the 4 MK64 cups plus N64KR cups, except in Time Trials (no record slots). Entering the course select in Time Trials while an N64KR cup is current falls back to Mushroom (menus.c hook).
- The page: MK64 cup icons hide (func_800AB164 hook); `n64kr_cup_page_draw` (from the map-select banner's draw) puts a banner where the icons were (flashing while choosing the cup), the course list as text inside MK64's own boxes (`func_800A8A98` at the list item's place; the cursor's row in red), and a preview box with the course's name and source game. MK64's preview pictures are skipped for N64KR cups (func_800AB290 / func_800A1500 hooks). `gCupNames[...]` lookups go through `n64kr_cup_name` (8 sites).

## Fossil Canyon [VERIFIED: tool output]
- Same pipeline as Ancient Lake: `dkr_level_to_gltf.py --mk64` (2,100 triangles, 35 materials, 31 textures, 1,091 path points, 46 checkpoints, 15 item rows), `blender_dkr_track.py` (export OK, texture loads pass), `dkr_track_scene.py` (minimap 80×80, sky from dome14). The converter warns the start grid spans z −29..53 (part of it ahead of the line). No music yet (Sequence −1); DKR music is a later task.

## Checks for the in-game test
Banner and text placement on the cup select; GP through all 4 Rare Cup tracks (race order, results screen cup name); VS; Time Trials never offers the Rare Cup; Battle unchanged; Fossil Canyon start grid and item boxes.

## First in-game test (user, 2026-11-02) [VERIFIED: user screenshot]
- The Rare Cup page works: banner, the four track names in MK64's boxes, preview box with the course and its game. GP runs.
- Fossil Canyon: laps didn't register properly and positions jumped (1st to 8th). Cause (plot of path, checkpoints and grid): DKR keeps a checkpoint set per vehicle (Fossil Canyon: car indices 0–34, another set 0–56 in steps of 2). `mk64_layout` filled indices missing from the car set with the other set's, so after car checkpoint 34 the path ran on through the other set's 36–56: a second, partial lap from mid-course (1,091 points; a jump across the infield). Fix: one vehicle's set only (the car's if it has one). Then the start line: placed from the front grid row toward checkpoint 0, it landed past the last checkpoint, so the path crossed the line forward and back. It is now placed on the lap itself (the checkpoint segment nearest the front row, 20 units ahead, stepping onto later segments past a checkpoint) and the checkpoints start right after it. Fossil Canyon: 720 points, one lap, grid z 15..61 (all behind the line); Ancient Lake: 469 points, grid z 21..56 (it moved ~1 path point; re-exported). Retest pending.
