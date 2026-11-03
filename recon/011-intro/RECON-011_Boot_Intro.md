# RECON-011 — The boot intro: credits, GoldenEye's gun barrel, a nuke flash

Project: N64 Kart Racing (N64KR)
Date: 2026-11-03
Status: built and checked in screenshots; skipping with a button and the sound not yet tested by hand

## Goal (user)
"Envisioned and produced by UpDoge, augmented and created by Claude" and then the GoldenEye music and gun barrel cam followed by a Duke nuke flash screen. For the flash the user chose "our own nuke flash": a white flash and shockwave we make ourselves, ending on an N64KR title.

## What plays
Before the Harbour Masters intro (`HARBOUR_MASTERS_MENU`):
1. **Credit card**, 4.8 s: "ENVISIONED AND PRODUCED BY / UPDOGE / AUGMENTED AND CREATED BY / CLAUDE" in MK64's menu font, fading in and out. MK64's font has no lowercase (`char_to_glyph_index` maps a–z onto the capitals) [VERIFIED: src/menu_items.c:1808].
2. **GoldenEye's gun barrel**, about 19.5 s, with GoldenEye's intro music (`N64KR_MUSIC_GE_INTRO`, rendered by `ge-audio`) starting on the dots as in GoldenEye (`init_menu03_gunbarrel` calls `musicTrack1Play(M_INTRO)` when the barrel starts) [VERIFIED: decomp src/game/front.c:1830].
3. **Nuke flash**, 6.2 s: a white core blows out to a white screen, GoldenEye's two explosion sounds pitched down; the white clears over a shockwave ring and an orange afterglow to "N64KR" in block letters with "N64 KART RACING" under it; fade to black.

Then the Harbour Masters intro plays as before. Any button fades straight to the Nintendo logo screen, as a button does during the Harbour Masters intro, and stops the intro's music. CVar `gN64KR.Intro` (default 1) turns it off. Without the `ge-intro` assets the credit card runs straight into the nuke flash.

## The gun barrel from the decomp
`src/game/title.c renderGunbarrelEyeIntroSequence` [VERIFIED: decomp source], NTSC values:

| Mode | GoldenEye | N64KR |
|---|---|---|
| 0 dots | `g_TitleX` from -30 by 6 a frame to 1390; a second dot jumps to `g_TitleX - 12` every 34 frames (counter from 66, then 200, minus 6) | same, 60 frames a second = 3.95 s [CONCLUSION: the dot phase runs at the full 60 Hz; PAL's 7 a frame at 50 Hz gives the same duration] |
| 1 slide | `g_TitleX` from 1276 down by 5.8183 a frame to -80; Bond drawn once below 600 | same, 30 frames a second = 7.77 s |
| 2 hold | 20 frames | 0.67 s |
| 3 blood | `die_blood_image_routine` (an animated 80 × 96 intensity image in code) | ours, 2.0 s (below) |
| 4 wobble | `g_TitleX = base + sins(+0x38E a frame) × 64 / 32768`, 108 frames; full-screen red (150, 0, 0, alpha 180) | same; ours also sinks the barrel 90 units |
| 5 fade | black, alpha +8 a frame to 247 | 1.03 s |
| 6 black | 30 frames | 0.5 s, then the nuke |

- **Screen space:** `guOrtho(0, 1280, 0, 960)`. The opening is GoldenEye's hole disk (radius 64, 16 rows, grey 143 + 111 cos t, bright at the bottom; title3.c `createGunbarrelRenderHole`) scaled 2.7 × 2.57 at (`g_TitleX` + 768, 442). The dots are the same disk in prim colour 0xE6 at y 482 [VERIFIED: decomp source].
- **The rifling:** a 440 × 299 8-bit intensity image, RLE (`rle_expand_8bit`: u16 width, u16 height, then (count, value) pairs from byte 10) at ROM 0x2A4D50 (`ge007.u.2A4D50.usedby7F008DE4.bin`). GoldenEye draws it as 299 one-row rectangles at screen rows 16..315 of its 440 × 330 front-end screen, shaded black at the top to white at the bottom, offset by `g_TitleX × 440 / 1280` (title2.c `titleRenderFolderMenuBackgroundLines`) [VERIFIED: decoded image shows the spiral with its opening at about (264, 160), where `g_TitleX + 768` puts the hole].
- **Bond's clock** (`sub_GAME_7F007F30`): +2 ticks a frame (60 a second). `bond_eye_walk` starts at frame `frames - 0x44` wrapped (= 2) at speed 0.91 × play speed 0.5; at tick 137 `bond_eye_fire` from frame 2 at 0.91 (merge 16); at 212 speed 1.6; at 230 the shot, `GUN_RIFLE7BIG_1_SFX` (111) [VERIFIED: decomp source].
- **Bond's models:** body `CdjbondZ` (`BODY_Brosnan_Tuxedo`), head `CheadbrosnanZ`, gun `PchrwppkZ` attached to the body's `Switches[3]`, both scaled 0.18779343 [VERIFIED: title.c `initializeGunBarrelIntro`]. `Switches[3]` is the group of joint 9 in both `CdjbondZ` and `ColiveguardZ` [VERIFIED: tool probe of the switch table]. With the PP7 hung on that group's matrix and no extra turn, the barrel points along +x, at the camera, on the shot [VERIFIED: muzzle − grip = (121, 24, −24) at fire frame 60].
- **Root motion** (`modelAnimReadRootMotionValue`, joint 0's fields x, y, z, turn): 6-byte descriptors (u16 bit offset, u8 bit count, pad, u16 value offset), values sign-extended then offset. With it applied, the lowest point stays within about 40 units (of 1870) across both animations [VERIFIED: tool output, feet −39..−81].

## Our staging (decision)
GoldenEye's camera (`gunbarrelPosition1` = (1758.3, 220, 684.3), direction (−0.97, 0, 0.24), 46° vertical) sees Bond at the origin about 35° off its axis, outside its 29.5° half-width [CONCLUSION: from the decomp's numbers; reproducing it would need GoldenEye's character movement from root motion]. N64KR's camera is level with Bond's middle on his left (+x), so he walks in place to screen left with the barrel tracking him and turns to fire straight into the lens; his picture is moved to the opening's centre with a clip-space shift. He is 27% of the screen tall (the opening is 34%).

## Assets (`n64kr ge-intro <GoldenEye ROM> <mod dir> [preview dir]`)
Into `<mod dir>/ge/intro/` (ROM data: stays under `build/`):
- `bond/walk00..34`, `bond/fire002..122` (every 2nd frame): display lists in GoldenEye model units, feet on y = 0; `model.json` (height 1872, counts, the muzzle position in each fire pose). About 30 MB of XML, loaded in the background during the credit card (`LoadResourcesAsync`).
- `barrel/rifling`: a quad 1280 wide over rows 16..315, vertex colours black at the top to white at the bottom, textured with `tex_rifling` (a 32 × 64 stub) and `tex_rifling.png` (the full 440 × 299 image, which the port loads in its place: `ResourceFactoryBinaryTextureV0` looks for `<path>.png` first) [VERIFIED: src/port/resource/importers/BetterTextureFactory.cpp].
- `ge-audio` now also writes `sound/n64kr/ge/rifle_big.wav` (SFX 111).
- With a preview folder: side and front contact sheets of the poses (a small software renderer in `ge_intro.rs`) and the rifling as PNG.

## Engine (`src/n64kr/intro.{h,cpp}`)
- Hooks [all `// N64KR HOOK`]: the `MENU_ITEM_UI_HARBOUR_MASTERS` draw and tick cases in `src/menu_items.c` skip `HM_DrawIntro` / `HM_TickIntro` while `n64kr_intro_draw` / `n64kr_intro_tick` return 1; `logo_intro_menu_act` (`src/menus.c`) calls `n64kr_intro_skip` before its usual fade.
- The timeline runs on a steady clock, so the music and the picture stay together whatever the frame rate.
- 2D parts are drawn in GoldenEye's 1280 × 960 ortho space from small vertex arrays built at start (hole disk, quad, drip end, glow, ring quarter, title cells). MK64's menu text needs the menus' screen projection loaded first (`GetScreenMatrix`, as `func_80094A64` does); without it the text came out tiny and upside down in the corner [VERIFIED: first screenshots].

## Checks
- [VERIFIED: in-game screenshots of the game window, 640 × 480] credit card; the two dots; the barrel sliding in; Bond walking in the opening; Bond turned and aiming; the muzzle flash at the PP7 on the shot (time frozen with a temporary switch, since removed); the blood curtain running down; the red wobble; the white clearing to the title; the title with its subtitle; the Harbour Masters intro afterwards.
- [VERIFIED: game log] `song music/n64kr/ge_intro registered as sequence 0x4e42`; no resource warnings.
- Not yet checked by hand: hearing the music, shot and boom; skipping with a button; a widescreen window (the 2D parts are laid out for 4:3; the black fills reach past the sides).
