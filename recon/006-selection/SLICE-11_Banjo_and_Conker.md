# SLICE-11 — Banjo and Conker: more DKR racers

Project: N64 Kart Racing (N64KR)
Date: 2026-11-02
Status: tested in game (first round); fixes built, retest pending

## Goal (user)
"We should maybe add a new character or 2 from diddy kong to see if the character select and game can accommodate it."

## What was added [VERIFIED: tool output, engine build]
- **Banjo** (DKR object `BanjoCar`, registry id 10) and **Conker** (`ConkaCar`, id 11), made with the same pipeline as Diddy (N64KR.md "Making Diddy"): racer glTF, 1,188 kart sprite frames, HUD (portrait, minimap icons, select face, nameplate), voice clips, 3D model. Mods `n64kr-dkr-banjo`, `n64kr-dkr-conker`.
- Voice: their own DKR clips (groups "banjo"/"conker" in `dkr_voice_to_wav.py`) plus one-off lines from the DKR decomp's `include/sound_ids.h`: Banjo GUHHUH 0x43, YAHOO 0x78, WOAH 0x98, "I'm Banjo" 0x8C; Conker YEHAHA 0x2E, AHH 0x96, "I'm Conker" 0x8A.
- Registry (`character.c`): Banjo borrows DK for engine sound, body, AI and stats (heavy); Conker borrows Yoshi (light). Both use Mario's sprite layout and Toad's HUD layout, like Diddy. MK64's body sizes are all 0.75, so the 3D models keep DKR's relative sizes.
- DKR select page (`character_select.c`): cells 1–3 = Diddy, Banjo, Conker. Faces and nameplates come from a per-cell table (textures named after each character's hudName) instead of Diddy-only code; the label is the registry name in capitals.
- CPU racers: every DKR page character no human picked takes a random stock CPU seat (was: Diddy only). Swapped seats are recorded so GP rivals map back (`n64kr_rival_character`); character-id tables were already sized for 16 (SLICE-8).

## Checks for the in-game test
The DKR page shows three faces and names; picking each one; CPU Banjo/Conker in GP and VS; their voices ("I'm Banjo/Conker" on select, lines in races); 3D models and sizes; GP results and podium with them.

## First in-game test (user, 2026-11-02) [VERIFIED: user report and screenshots]
Worked: the DKR page (three faces and names, each pickable), voices including "I'm Banjo/Conker", CPU Banjo and Conker in a Rare Cup GP, results and the podium (Banjo 1st, Conker 3rd on the podium screenshot). Fossil Canyon lap counting fixed (RECON-009).

Reported, and what was done:
| Report | Cause | Fix |
|---|---|---|
| Banjo "pretty scuffed", Conker "odd in the face" | `dkr_racer_to_model.py` set every texture to wrap. Nearly all DKR racer textures have header flags 0x40/0x80 (clamp S/T) and many UVs run far outside the texture (Banjo -31..48 texels on 16-texel textures, Conker -497..124), so the textures repeated: extra eyes on Conker's forehead, eye and fur patches on Banjo [VERIFIED: UV ranges printed from the ROM; Blender renders of the exported straight pose before and after]. The glTF export (sprites, portraits) already clamped. | Materials clamp S/T from the texture header. Diddy re-exported too (same issue). |
| Banjo "a bit small" | All three used one world scale tuned on Diddy. DKR's own object scales hardly differ (Diddy 0.26, Banjo 0.27, Conker 0.22). | Per-racer size argument; Banjo 1.2 (`model.json` world_scale 0.0936). Conker unchanged (no report). |
| "Layer error", seen here and there (a 3D racer's head poking over a sprite racer) | MK64 draws kart sprites 6 units (x 0.75 racer size) toward the camera from the kart (`gPlayerOneVtx` z = -6, turned by unk_048 = atan2s(kart - camera), `func_8002934C`), so a sprite wins depth tests against anything that close in front of its kart; the 3D models sat at the real position [CONCLUSION from source]. | `racer3d.c kart_matrix` moves the model the same distance toward the camera and scales it by (d - shift)/d, so it looks the same on screen but depth-sorts like a sprite. Camera from `render_players` (N64KR HOOK). |
| Cup award screen showed Toad when Banjo won | The award portrait (MENU_ITEM_TYPE_130, `func_800AF270`) used the `hud` character's (Toad's) face animations and nameplate. | `n64kr_char_face` / `n64kr_char_nameplate` / `n64kr_char_nameplate_label` give a DKR page character its select face and plate with the name printed over it (N64KR HOOKs in `menu_items.c`). |
| Banjo "may have made the wrong" sound landing a shell on someone | DKR fills Banjo's 8 + 8 positive/negative voice slots from fewer recordings, crossing lists: his positive2 is his "woah", positive5 = negative4, negative1 = "guh-huh"; Conker's positive7 = negative2 [VERIFIED: sound table bites]. N64KR plays the positive list when the racer hits someone. | `dkr_voice_to_wav.py crossed_clips` keeps each recording once per list, keeps negative recordings out of the positive list (unless also a laugh line) and laughs out of the negative list. Banjo: positive = yahoo, guh-huh and two more; no "woah". `voice.cpp` picks evenly among the clips present and not the same one twice in a row. |
| Small graphical glitch on the select page switching from Diddy to Banjo | Not found from the code: cell update/draw, face animation (`func_8009A878` loops to frame 0) and nameplate loading (the port draws textures straight from their paths) change nothing when the cursor moves. | Open: needs a screenshot or description. |
