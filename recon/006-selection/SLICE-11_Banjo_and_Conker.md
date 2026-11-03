# SLICE-11 — Banjo and Conker: more DKR racers

Project: N64 Kart Racing (N64KR)
Date: 2026-11-02
Status: built; in-game test pending

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
