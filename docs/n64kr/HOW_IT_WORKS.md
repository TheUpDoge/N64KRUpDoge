# How N64KR works

N64KR is a fork of [SpaghettiKart](https://github.com/HarbourMasters/SpaghettiKart), the native PC port of Mario Kart 64. It adds racers, tracks, physics, music and hazards taken from **Diddy Kong Racing** (DKR) and **GoldenEye 007**, read at build time from ROMs you own. This page explains what changed, why, and how each piece was done, so others can check the work, build on it and bring in more games.

Nothing from any ROM is in this repository. The tools read your ROMs and write their output into `build/`, which git ignores.

Claims below carry the project's evidence labels: **VERIFIED** (read in source, seen in tool or build output, or tested in game), **CONCLUSION** (follows from verified facts) and **HYPOTHESIS** (not yet checked). The step-by-step records with file and line references are in [`recon/`](../../recon).

- [The three big hurdles](#the-three-big-hurdles)
- [1. The idea: a character doesn't own its physics](#1-the-idea-a-character-doesnt-own-its-physics)
- [2. Living inside SpaghettiKart: thin hooks](#2-living-inside-spaghettikart-thin-hooks)
- [3. Physics: profiles, movement models and DKR's hovercraft](#3-physics-profiles-movement-models-and-dkrs-hovercraft)
- [4. Characters from other games](#4-characters-from-other-games)
- [5. DKR tracks](#5-dkr-tracks)
- [6. DKR racers](#6-dkr-racers)
- [7. GoldenEye: a track, hazards and music](#7-goldeneye-a-track-hazards-and-music)
- [8. Menus: pages, cups, arrows](#8-menus-pages-cups-arrows)
- [9. Lessons: bugs, causes and fixes](#9-lessons-bugs-causes-and-fixes)
- [10. How the work was done](#10-how-the-work-was-done)

---

## The three big hurdles

Three things Mario Kart 64's code was never built for, and that looked impossible to add without rewriting large parts of the game:

| Hurdle | Why it was hard [VERIFIED: source] | How N64KR does it |
|---|---|---|
| **More than 8 racers** | MK64 assumes exactly 8 characters with ids 0–7: the select grid, CPU rosters, GP standings (`gGPPointsByCharacterId[8]`), rivals, voice ids, HUD and record saves, all in tables sized for 8. | A **character registry** (ids up to 15) that every character-keyed read goes through ([section 4](#4-characters-from-other-games)). **Select screen pages** keep the menus' 8-seat bookkeeping untouched; only the racer that spawns becomes the new character. Unpicked new racers take CPU seats and map back for rivals and standings. |
| **More than 4 cups** | The cup select draws per-cup images (icons, course lists, previews), the battle cup is fixed at index 4 (`BATTLE_CUP`), and time trial records are keyed by `cup × 4 + course`. | N64KR cups go **after** the battle cup, so every stock index keeps its meaning. Cup cycling skips cups a mode can't use, and an N64KR cup draws its own page (banner, course list, source game) instead of the per-cup images ([section 8](#8-menus-pages-cups-arrows)). A cup is one short entry in `src/n64kr/cups.cpp`. |
| **Content from other games** | There were no tools to bring another game's racers, tracks or music into SpaghettiKart; DKR's and GoldenEye's formats differ from MK64's at every level (compression, models, animation, audio). | Formats decoded from the community decomps and turned into SpaghettiKart-native assets ([sections 5–7](#5-dkr-tracks)): tracks through Fast64, racers as sprites, 3D models, faces and voices, music rendered from the original sequences. |

All three are done with thin hooks into SpaghettiKart's own files ([section 2](#2-living-inside-spaghettikart-thin-hooks)), so upstream updates can still be merged.

## 1. The idea: a character doesn't own its physics

In Mario Kart 64 a racer's character id selects everything at once: sprites, voice, size, AI and kart stats (`gKart*Table[player->characterId]`). That makes "Diddy Kong driving a DKR hovercraft on a GoldenEye track" impossible, because character, vehicle and handling are one number.

N64KR splits them:

| Layer | What it owns | Code |
|---|---|---|
| **Character** | looks, voice, HUD, size, AI personality | `src/n64kr/character.{h,c}` (registry) |
| **Vehicle profile** | stats: top speed, acceleration curve, gravity, friction, handling, bounding box, hop | `src/n64kr/vehicle_profile.{h,c}` |
| **Movement model** | the physics code that turns stats and input into motion | `src/n64kr/movement.{h,c}`, `hover.{h,c}` |

Any combination works: Mario on a DKR hovercraft, Banjo in an MK64 kart, MK64 racers on a DKR track.

## 2. Living inside SpaghettiKart: thin hooks

SpaghettiKart keeps moving, and N64KR wants its updates. So all N64KR code lives in `src/n64kr/`, and SpaghettiKart's own files only get **one-line hooks**, each marked `// N64KR HOOK` (`grep -rn "N64KR HOOK" src` lists them all). A hook either calls into `src/n64kr/` or swaps a table lookup for a registry accessor, for example `gCharacterSize[n64kr_char_body(player->characterId)]`. Merging an upstream update means re-applying a few dozen one-liners, not untangling a rewrite.

## 3. Physics: profiles, movement models and DKR's hovercraft

**Vehicle profiles (SLICE-1).** Every place MK64 reads `gKart*Table[characterId]` for physics (mostly `src/player_controller.c`) now reads the racer's profile. A profile can say "own character", which returns MK64's exact stats, so stock races are unchanged [VERIFIED: identical driving in game]. Putting Mario on Bowser's profile makes him drive like Bowser [VERIFIED].

**Movement switch (SLICE-2).** The call to MK64's driving physics (`func_8002D268`) goes through a selector chosen by the profile: MK64 ground, a test "floaty" model, or DKR hover.

**DKR hover (SLICE-3 to 5).** The hovercraft is a port of the movement-only parts of DKR's `func_80046524` (`src/racer.c` in the [DKR decomp](https://github.com/DavidSM64/Diddy-Kong-Racing), which is CC0), keeping MK64's own collision. The key finding was the **timestep**:

- MK64 runs physics at 60 Hz, which is DKR's `updateRate = 1` [VERIFIED: `main.c` `gTickLogic`, `TRACK_TIMER_ITER`].
- DKR scales *some* terms by `updateRate` (throttle ramps, gravity, position) but applies others once per logic frame: lateral grip `0.07`, thrust, brake, slope push, quadratic drag `0.004·v²` [VERIFIED: decomp line refs in `recon/004-hover`].
- DKR was tuned at 30 Hz logic, so at 60 Hz those unscaled terms would act twice as hard. The port halves each one per tick [CONCLUSION, then tuned in playtests].

Hovercraft float on water and lava (buoyancy ported from the same function) and use **DKR's real stats**, read from your DKR ROM by `tools/n64kr/extract_dkr_hover_stats.py` (weights, handling, speed curves). **R** on the character select screen switches each player between Kart and Hover.

## 4. Characters from other games

**The registry (SLICE-8).** `characterId` now indexes N64KR's registry instead of MK64's 8-entry tables. Each entry names, per category, which stock character it borrows from until it has its own assets:

```c
//  name     voice   sprites hud     body    ai      stats   kartFolder     hudName   voiceName  model3d
{ "Banjo",  DK,     MARIO,  TOAD,   DK,     DK,     DK,     "banjo_kart", "banjo",  "banjo",   "racers/banjo" },
```

MK64's 8 map to themselves. Id 8 stays MK64's "no character" marker, and new characters start at 9. Any MK64 table read keyed by character goes through an accessor (`n64kr_char_voice`, `n64kr_char_hud`, ...), so a new character never indexes past an 8-entry table.

**Select pages.** MK64's menus use 8 stock ids everywhere: CPU rosters, GP rivals, time trial saves. Rewriting that would touch a huge amount of upstream code. Instead the 8-cell grid shows one **page** at a time, and pushing past the grid edge turns to the DKR page. The menu keeps the stock id of the cell, and only the spawned racer gets the page character's registry id (`n64kr_spawn_character`). DKR characters nobody picked take random CPU seats, and the swapped seats are recorded so GP rivals map back.

## 5. DKR tracks

Pipeline: `dkr_level_to_gltf.py --mk64`, then `blender_dkr_track.py` (Blender 4.3 + HarbourMasters' Fast64), then `dkr_track_scene.py`. The result is a standard SpaghettiKart custom track mod.

- **Reading DKR.** One asset lookup table points at sections (level models, textures, objects, audio, ...). Most assets use DKR's compression: a 5-byte header, then raw deflate (`src/gzip.c`). Level models contain segments, triangle batches and vertices exactly as in the decomp's `include/structs.h` [VERIFIED].
- **To glTF.** Textures are decoded from DKR's formats (RGBA16/32, I4/8, IA4/8/16, interlaced rows), and triangle batches become materials tagged with MK64 surface, collision and draw layer.
- **Laying it out for MK64.** MK64 counts laps and positions along an AI path. DKR has **checkpoints, a separate set per vehicle**. The converter builds the path from one vehicle's checkpoints (mixing sets added a second, partial lap; see lessons). It also places the start line on the lap just ahead of the front row and rotates the path to start there.
- **Fast64 export.** The Blender script imports the glTF, splits by material, sets surfaces and draw layers, assigns Fast64 presets (opaque / cutout / translucent, classified from each texture's decoded alpha), draws faces double-sided where DKR does, and runs Fast64's MK64 "Export Track".
- **Scene.** `scene.json` gets the name, item boxes (each DKR balloon becomes a row of boxes), minimap, sky colours and music id.

Ancient Lake and Fossil Canyon are converted and tested [VERIFIED: in game].

## 6. DKR racers

One DKR racer object (`diddycar`, `BanjoCar`, `ConkaCar`, ...) becomes a full N64KR character:

| Output | How |
|---|---|
| **Racer glTF** (`dkr_racer_to_gltf.py`) | Body model 0 (most detailed), wheels from the object's vehicle parts, and DKR's **vertex animation** decoded: keyframe 0 holds an s16 offset per animated vertex, later keyframes hold s8 deltas, 16 frames per keyframe [VERIFIED: `obj_animate.s`]. Animation 0 (steering) is exported as shape keys, plus the horn taunt (4) and the crash (3). |
| **Kart sprites** (`blender_dkr_racer_sprites.py`) | All **1188** MK64 sprite frames (321 angles × wheel spin), rendered in SpaghettiKart's kart setup scene so they line up with MK64's camera. |
| **3D model** (`dkr_racer_to_model.py`) | libultraship display lists, one per steering pose, plus separate wheels. The engine (`racer3d.c`) draws them instead of sprites, picks the pose from the visual turn, rolls the wheels, steers the front ones, and keeps MK64's star / Boo / Lakitu effects. |
| **Faces** (`blender_dkr_racer_faces.py`) | 23 select faces at **4× MK64's detail** (256 px PNGs in the 64×64 slot): idle, blinks from DKR's own animated eye textures, a "picked" animation (DKR's horn taunt, turning to face you), and a "lost" one (DKR's crash) for the cup award screen. |
| **HUD** (`blender_dkr_racer_hud.py`) | Race portrait at 4×, minimap icons, a blank nameplate (the name is printed over it). |
| **Voice** (`dkr_voice_to_wav.py`) | DKR's sound bank parsed (libultra `ALBankFile`), N64 **VADPCM** decoded, pitch from the key map. Each racer gets eight "positive" and eight "negative" lines plus one-offs ("I'm Banjo!"). |

Two engine facts made the HD faces possible [VERIFIED: libultraship source, screenshots]: SpaghettiKart loads a PNG next to a texture resource at whatever size it is, and libultraship takes texture coordinates from the **tile** MK64 loads, not from the image. So a 256 px face fills the 64×64 face slot with four times the detail, with no change to MK64's menu code.

## 7. GoldenEye: a track, hazards and music

GoldenEye tools are in Rust (`tools/n64kr-rs`), using the [GoldenEye decomp](https://github.com/n64decomp/007) as the format reference.

- **Levels.** Each level's background is a set of rooms with their own vertices and display lists (`ge-bg-info`, `ge-rooms-gltf`).
- **Models and animation.** Model files are "1172"-compressed, and their part counts live in the game's model header table, not the file. Guards are animated by bit-packed joint angles per frame (`ModelAnimation` table). Each joint matrix = parent × rotation_xyz [VERIFIED: posed renders match the game].
- **Dam Speedway** (`ge-dam-speedway`). The track is built over Dam's reservoir:
  - **Road and scenery:** a banked stadium oval at the water level (the height with the most flat area), using Dam's own asphalt and red/white striped walls. Dam's outdoor rooms stay as scenery, minus anything inside the road corridor.
  - **Watchtowers:** two of Dam's real watchtowers (rooms 78–80) are copied beside the track.
  - **Variant B:** two Dams facing each other across the water, joined into a longer oval.
- **Hazards** (`ge_hazards.cpp`). Track actors for oil drums (explode on touch, chain reactions, racers tumble) and Dam guards: the real guard model and animation, raising his KF7 and firing bursts at racers in range.
- **Music and sound** (`ge-audio`). An offline port of libultra's compact sequence player renders GoldenEye's own sequences with its own instruments (ALBank + VADPCM) to looping WAV. The engine plays them through SpaghettiKart's HMAS mixer under N64KR sequence ids (the Dam theme, and DamX on the final lap). Sound effects go through a small positional mixer with a soft limiter.

## 8. Menus: pages, cups, arrows

- **Cups.** MK64 knows four racing cups plus the battle cup at index 4. N64KR cups go after it (index 5+), so stock indices keep their meaning. Cup cycling skips cups a mode can't use (time trial records have no save slots for them). An N64KR cup draws as its own page: a banner instead of the four cup icons, the course list as text, and the source game in the preview box. The **Rare Cup** holds Ancient Lake, Fossil Canyon, Dam Speedway and Dam Speedway B.
- **Arrows.** Pulsing arrows beside the character grid point to the other page, with its name. On the cup select they point to the N64KR cups. The Kart/Hover label reads "R KART" at the top of the cell, so players know R switches it.
- **Award screen.** A DKR racer who wins a cup shows their own face and nameplate, with the picked or lost animation.

## 9. Lessons: bugs, causes and fixes

These cost the most time to find, and most apply to anyone porting content into SpaghettiKart.

| Symptom (from playtests) | Cause | Fix |
|---|---|---|
| **Items (bananas, shells) visible through hills** on converted tracks | Fast64 ends every mesh with a material "revert" list that **clears `G_ZBUFFER`** (and shading, filtering). MK64's item lists never set depth testing themselves, because MK64's own tracks leave it on [VERIFIED: exported lists, banana display list]. | `n64kr_restore_track_state()` restores MK64's track state before items draw (`src/n64kr/render_state.c`, one hook in `racing/actors.c`). Covers every Fast64-built track. |
| Race started **under the road**, countdown stuck | Generated road quads were wound so they faced down. MK64 builds collision from display lists and treats down-facing triangles as ceilings. | Wind road triangles counter-clockwise seen from above. |
| A **3D racer covered by a sprite kart** behind it | MK64 draws each kart sprite 6 units (× racer size) **toward the camera** from the kart (`gPlayerOneVtx` z = −6, turned to face the camera) [CONCLUSION from source]. | 3D models move the same distance toward the camera and shrink by the same perspective: they look identical but depth-sort like sprites. |
| **Racers vanish** far ahead | MK64 only draws racers inside a triangle ahead of the camera: 1200 units deep in a 1P race, but its corners sit 58° out, so straight ahead it ends at about 640 [VERIFIED: `check_player_camera_collision`]. Fine for MK64's tracks, too short for bigger ones. | Grow the triangle to reach the track's far plane (`gN64KR.FarRacers`). |
| DKR racer models **"scuffed"**, extra eyes on Conker's forehead | Nearly every DKR racer texture has **clamp** flags (header 0x40 / 0x80), and UVs run far outside the texture (−31..48 texels on 16-texel textures). The exporter tiled them. | Clamp S/T from the texture header. |
| **Laps didn't count**, positions jumped 1st → 8th (Fossil Canyon) | DKR keeps a checkpoint set per vehicle, and mixing two sets added a partial second lap. The start line was also placed past the last checkpoint, so it was crossed twice. | One vehicle's set; start line on the lap. |
| Banjo said **"woah!"** when he hit someone | DKR has fewer Banjo recordings than voice slots, so it repeats them across lists: his positive2 is his "woah", positive5 = negative4 [VERIFIED: sound table]. | Keep each recording once, in the list it belongs to; never play the same clip twice in a row. |
| **Music silent** on the second track | A fade effect from the previous race survived `HMAS::Stop` and muted the next song. | Clear pending effects on stop (one hook). |
| Sound **loud and crackly** | The port casts HMAS's float mix buffer straight to int16, so values past ±1 wrap around. | Soft limiter, lower gains, per-clip voice cap. |
| Crash `SUPPORT_CHECK(tile == 7)` on a converted track | libultraship only supports `LoadTile` on tile 7. Fast64 used `LoadTile` for big non-power-of-two sprites. | Resample sprites to power-of-two sizes that fit TMEM. |
| Karts **slid to the inside** of banked turns and stuck | 25° banking was more than MK64's grip handles. | 10°. |
| **Item boxes out of reach** | MK64 floats boxes 8.66 units above their spawn point on its own. | Spawn at road height. |
| **Minimap** striped | 4-bit textures need widths that are multiples of 16. | Pad the minimap width. |

## 10. How the work was done

N64KR was built AI-assisted: Claude Code wrote the code and tools, and Mitchel Friend (UpDoge) directed the work and play-tested every step on real hardware and screens. The workflow that made it reliable:

1. **Read the source first.** The MK64, DKR and GoldenEye decomps are the ground truth. Every format fact is cited to a decomp file and line in `recon/`.
2. **One slice at a time.** Each step has a pass/fail test, a record of what was found, what changed, and what was seen in game.
3. **Label every claim** VERIFIED, CONCLUSION or HYPOTHESIS, and never present a guess as fact.
4. **Play-test, then fix the cause.** Each symptom in the lessons table above came from a playtest report and was traced to a specific line or data value before it was changed.
5. **Never commit ROM data.** Everything game-specific is generated locally from the user's own ROMs.
