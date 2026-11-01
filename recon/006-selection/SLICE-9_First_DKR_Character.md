# SLICE-9 — First DKR character

Project: N64 Kart Racing (N64KR)
Date: 2026-11-01
Status: IN PROGRESS. Step 1 (art pipeline) and step 2 (engine slot) pass in game; step 3 (grid > 8, CPU choice) and own HUD/voice not started.

## Plan
1. **Art pipeline**: DKR racer 3D model → MK64 kart sprite frames. Validate by replacing an existing character's sprites with a texture pack (supported by SpaghettiKart).
2. **Engine slot**: a 9th registry entry with its own sprite paths, portrait, name; GP points / no-character marker / time trial save bits (SLICE-8 list).
3. **Select screen**: grid larger than 8; CPU character choice.

Two long-term rendering routes for DKR racers (user note: DKR racers are 3D):
- **Pre-rendered sprites** (this slice): the DKR 3D model rendered from every MK64 sprite angle. Fits MK64's sprite-based racer engine as-is.
- **Real-time 3D racers** (later): a new racer renderer + DKR vertex animation. Same extracted models.

## MK64 kart sprites [VERIFIED: source + asset headers]
- 321 frames per character, `textures/karts/<slot>/<slot>_frameNNN_wheelW` for frames 0–288 × 4 wheel-spin variants (1156) and `<slot>_frameNNN` for tumble frames 289–320 (32), plus `<slot>_palette` (`include/assets/textures/karts/mario_kart.h`).
- Frames are grouped into 18 view groups (`DECLARE_KART_GROUP` / `DECLARE_KART_TABLE0/1` in `src/kart_dma.c`); the wheel variant is `tyreSpeed >> 8` (`kart_dma.c:607`).
- Texture packs replace textures by path with PNG (`docs/textures-pack.md`); SpaghettiKart's custom characters are texture packs over these frames (`docs/characters/characteroverview.md`).
- Render template: `docs/MK64_Spaghetti_Adjusted_Kart_Setup.blend`: EEVEE, 64×64, frames 0–320, fixed perspective camera behind the kart (y −3.29), kart turned by animated empties `Root` and `Kart Root`; Mario's armature animation (1–290) is only his steering lean. Stock wheel rims span x ±0.63; tyres' bottom z −0.26.

## DKR racers [VERIFIED: DKR decomp + ROM probe]
- Racer objects: behaviour `BHV_RACER` (1), model type 3D. Per character per vehicle: `diddycar`, `SWcar`, `KremCar`, `BadgerCar`, `TortCar`, `ConkaCar`, `TigerCar`, `BanjoCar`, `ChickenCar`, `MouseCar`, the `*hover` and `*plane` sets, plus bosses.
- Header lists 6 models = detail series; model 0 most detailed (`diddycar`: model 19, 257 triangles, 19 textures; 6th model is an 8-vertex placeholder).
- ObjectModel attach points: s16 vertex indices @0x14, count @0x18. Header vehiclePartIds @0x14 / vehiclePartIndices @0x18 (count @0x56): `diddycar` → `backtyre` ×2, `fronttyre` ×2, vehicle-part sprite objects (model type 2) with 4 sprite frames 179–182 (wheel spin, 12×14 px), part scale 2.9 / 2.4.
- Wheel positions (model units): back (±40, 0, +26), front (±38, −4, −42) → the model faces −Z, like MK64.

## Tools
- `tools/n64kr/dkr_racer_to_gltf.py`: racer body (model 0) → glTF + textures; wheel sprite frames → PNG; `<object>_racer.json` with wheel attach positions, part scales, sprite anchors. Shares `write_gltf()` with the track converter (refactored out of `convert()`; Ancient Lake output unchanged).
- `tools/n64kr/blender_dkr_racer_sprites.py`: opens the kart setup .blend, hides Mario + kart, imports the racer, fits wheel span to wheel span (DKR wheel centres ± half sprite width → stock rim span; wheel bottoms → stock tyre bottom), parents to `Kart Root`, unlit materials (vertex colour × texture), wheels as camera-facing quads with the 4 spin frames; renders 321 + 3 × 289 frames (8-bit RGBA) and packages them as a texture pack for a kart slot. `--preview` renders 12 sample frames.

## Results so far
- [VERIFIED: render] Diddy's car body renders correctly (textures, colours, cap, star shirt).
- [VERIFIED: preview frames] Fit scale 0.0109: Diddy's height 1.41 vs Mario's 1.49, wheels on the ground line; frames show rear views turning through the steering angles, front view at 288, tumble frames flipping the car.
- Bugs fixed on the way: fit by full bbox used the unsubdivided "Kart" cage (bottom −0.74, too low) and Diddy's arms (too small); preview PNGs were 16-bit (setup scene default), forced to 8-bit.
- **Wheels (fixed after the user's DKR screenshot)** [VERIFIED: decomp + render]: the user's screenshot of DKR shows Diddy's car with big red tyres. Each wheel sprite (179–182, one per spin state) has **16 frames**: the wheel seen from 0° (head-on, frame 0, a thin sliver) to 90° (side on, frame 15, round tyre with yellow hub). DKR picks the frame per draw from the camera position relative to the vehicle's vertical plane, `frame = floor(angle / 90° × 16)`, mirrored when the camera is on the vehicle's −X side (`render_sprite_billboard`, decomp `src/camera.c:1077-1112`); the spin state is the part's `modelIndex`. The exporter now writes all 4 × 16 frames; the renderer picks the frame per rendered frame. A second bug put all four wheels at one point (new objects' `matrix_world` is only valid after a scene update); fixed with `view_layer.update()` before fitting. Previews now show red tyres on both sides, changing with the angle.

### Front-wheel steering (user feedback: "back tires shouldn't pivot like the front")
- [VERIFIED: decomp] DKR cars steer only attach points 2 and 3: `obj[2]/obj[3]->trans.rotation.y_rotation = gCurrentStickX * 100` (`src/racer.c:6215-6218`); 0 and 1 stay straight. For Diddy's car, 2 and 3 are the `fronttyre` parts.
- [VERIFIED: setup .blend] Frames 0–188 are 9 blocks of 21: within a block `Kart Root` yaws the kart 3°/frame relative to the camera (frame 0: 180°, frame 20: 120°) and Mario's arm pose steers (pose period 21 frames). Frames 189–288 keep him straight; 289–320 tumble.
- Renderer: front wheels get a steering yaw that grows with the frame's position in its block, `steer = MAX × (frame % 21) / 20`, MAX = stick 70 × 100 = 7000/65536 turn (~38°), applied by rotating the camera offset into the wheel's yaw (as DKR does) before picking the angle frame. Rear wheels follow the body only. Frame 20 preview: rear wheels at the body's angle, front-left wheel turned further into the turn.
- Steering sign fix (user feedback: "the rear tires are turning"): per-wheel angle debug showed the front wheels turning *against* the body in frames 0–20 (front 50–57° vs rear 59–65° at frame 20), so the straight rear wheels looked like the turning ones. The steering rotation sign was changed. [VERIFIED: debug output]
- In game the user then saw "the front stays straight while the rear turns": the change had turned the fronts the wrong way. Debug at frame 20 (kart turned left, camera at its left-rear): rears 59–65°, fronts 12–19° (turned back toward the camera, reading as straight). Fixed by yawing the wheel by +steer (camera offset rotated by −steer): fronts now 80–81° vs rears 59–65°, 41–50° vs 26–37° at frame 10. [VERIFIED: debug output; in-game check pending]

### Driver pose (user feedback: "the character is doing a T-pose")
- [VERIFIED: decomp] The model's base vertices are a T-pose; DKR always draws racers through vertex animation. Driving uses animation 0 with frame `40 - steerAngle / 2`, clamped 0..73 (`src/racer.c:1794-1805`); steerAngle follows stick x, so 40 is straight, lower frames steer right, higher frames steer left.
- [VERIFIED: decomp asm + ROM probe] Vertex animation format, read from `obj_animate` (`src/hasm/obj_animate.s`) and `model_anim_init` (`src/object_models.c:892`):
  - `ASSET_ANIMATION_IDS` (section 30): s16 per object model, its range in `ASSET_OBJECT_ANIMATIONS` (31 table / 32 data). Each animation is DKR-compressed; s32 keyframe count first.
  - Keyframe 0: 12-byte header, then s16 xyz offset per animated vertex, added to the base vertex. Keyframes 1..: stride `12 + 3 × animated count`, 12-byte header then s8 xyz deltas, applied cumulatively. A frame is 1/16 keyframe; the fraction scales the next keyframe's delta (`delta × t >> 4`). The keyframe headers (s16s read at bytes 0/2/4/10) are not used yet.
  - Model 0x4A = animated vertex count, 0x4C = s16 animated slot per vertex (−1 static); several vertices can share a slot.
  - Diddy (model 19): 325 vertices, 232 animated using 111 slots; 5 animations; animation 0 has 6 keyframes = 80 frames.
- Exporter writes animation 0's frames as glTF morph targets (Blender shape keys `anim0_fNN`); the renderer picks the pose per sprite frame: frames 0–188 go from 40 (straight) to 73 (full left lock) over each 21-frame block, alongside the front-wheel steer; other frames stay at 40. MK64 mirrors these sprites for right turns.
- [VERIFIED: render] Front views of poses 0 / 40 / 73: leaning to Diddy's right / straight with both hands on the wheel / leaning to his left. The sprite previews show no T-pose.
- Next in-game report: "front is fixed but the rears are turning again". Cause [VERIFIED: debug output + renders]: each wheel picked its sprite angle from its own position, and the sprite camera is close (≈358 DKR units behind, wheels ±40 apart), so at frame 6 (body turned 18°) the rear-right tyre got 25° and the rear-left 12°. The far tyre's disc then faced sideways like a steered wheel; a 3D tyre would hide that inner face behind the body. Fix: all wheels take the view direction from the vehicle centre (rears now 17°/17° at frame 6, 58°/58° at frame 20; fronts add the steer: 29°, 79°).
- Also matched DKR's flip [VERIFIED: decomp src/camera.c:1097-1118]: on the negative side DKR turns the sprite 180° in its plane (`tiltAngle += 0x8000`), not a one-axis mirror; the side is the sign of the lateral offset, negated when the camera is in front. No visible change for Diddy's tyre (its sprite is vertically symmetric). DKR's small camera-height tilt is not applied.
- The user moved on after the centre-angle fix ("sure, lets move on"); art pipeline accepted.

## Step 2: engine slot (Diddy = character id 9)

### Decisions
- **Id 8 stays MK64's "no character" marker** (time trial records with no time, `menu_items.c` `render_lap_times`; minimap `render_objects.c:2633`). The registry keeps a pass-through entry at 8 and new characters start at 9, so neither marker site changes.
- **Diddy takes a seat.** Until the select grid grows, the player picks any of the 8 cells and presses **L**; "Diddy" shows over the cursor. Menu state (`gCharacterSelections`) keeps the stock cell, so everything keyed on it still works: CPU rosters (`cpu_forPlayer`, `cpu_forTwoPlayer`) and GP rivals (`code_80005FD0.c:3399`) treat Diddy as that cell's character, which then doesn't race. Only the human's racer spawns as id 9 (`spawn_player` hook).
- **Own kart sprites, borrowed rest.** Registry fields added: `stats` (stock MK64 stats when the vehicle profile is "own character"; read through `n64kr_char_stats` in `n64kr_stats_id`) and `kartFolder`. Diddy: sprites laid out like Mario's (`sprites = MARIO`, `kartFolder = "diddy_kart"`); voice, HUD, body, AI and stats from Toad (the closest lightweight) until he has his own. [CONCLUSION: placeholder choice]

### New texture paths [VERIFIED: source + o2r]
- SpaghettiKart reads a PNG only where a binary texture resource already exists: `MK64::ResourceFactoryBinaryTextureV0/V1::ReadResource` look for `<path>.png` after the resource loader found `<path>` (`src/port/resource/importers/BetterTextureFactory.cpp:28-41`). A brand-new path (`textures/karts/diddy_kart/...`) therefore needs a resource file too.
- Resource format (`mk64.o2r`): 0x40-byte header (`00000000`, `"XETO"` = OTEX, version 0, id `deadbeefdeadbeef`, zeros), then u32 type, width, height, data size, data. Kart frames are type 4 (CI8) 64×64.
- The packer (`blender_dkr_racer_sprites.py`, any non-stock slot) writes a stub resource (header + type 4, 64×64, size 0) next to each PNG; the PNG is what loads. New `--package-only` repackages existing renders. Output: `build/x64/mods/n64kr-dkr-diddy/` (1188 frames + 1188 stubs). The old pack over Mario moved to `build/dkr_racers/old-mario-replacement-pack/` (out of `mods/`).

### Code
- `src/n64kr/character.{h,c}`: reserved entry 8 `(none)`, Diddy at 9 (`N64KR_CHAR_NONE`, `N64KR_CHAR_DIDDY`), fields `stats`, `kartFolder`, accessor `n64kr_char_stats`.
- `src/n64kr/kart_sprites.cpp`: `n64kr_kart_texture(characterId, stockPath)` swaps the stock folder name for the character's `kartFolder` (cached strings).
- `src/n64kr/character_select.{h,c}`: L toggle + name label, `n64kr_human_character`, `n64kr_spawn_character`, `n64kr_gp_seed_character`.
- Hooks (`// N64KR HOOK`):
  - `kart_dma.c`: the 8 kart texture assignments (`load_kart_texture`, `load_kart_texture_non_blocking`) go through `n64kr_kart_texture`.
  - `spawn_players.c` `spawn_player`: human spawns as `n64kr_spawn_character(...)`.
  - `menus.c` / `menu_items.c`: L input and label beside the vehicle-select hooks.
  - `menu_items.c` `func_800A3A10`: GP standings seeded from the racers' ids (ascending; 0..7 for a stock roster, so stock order is unchanged) instead of assuming 0..7.
  - `gGPPointsByCharacterId[8]` → `[N64KR_MAX_CHARACTERS]` (reset loop uses `ARRAY_COUNT`); `gGetPlayerByCharacterId[10]` → `[N64KR_MAX_CHARACTERS]`.
  - Ending (`ending/code_80281780.c`): the human's podium place compares the character they raced as; `D_802874D8.unk1E` holds it, and its three portrait lookups in `menu_items.c` go through `n64kr_char_hud`.
  - `vehicle_profile.c`: "own character" stats via `n64kr_char_stats`.

### Build [VERIFIED: build output]
Debug and Release: 0 errors, no warnings from `src/n64kr`.

### Known limits (this step)
- Diddy is human-only; CPUs never pick him.
- HUD shows Toad's portrait, name and minimap icon for Diddy; voice is Toad's. With Toad also racing as a CPU, the standings show two "Toad"s.
- Time trial records and ghosts store the stock cell (3-bit save field), so a Diddy record shows as that character.
- The select screen shows the stock cell's face; only the label says Diddy.

### In-game test [VERIFIED: user playtest 2026-11-01]
- L on the select screen, race as Diddy with his own kart sprites: user reports "Works".

## Step 2b: own portrait, minimap icons, name

- [VERIFIED: o2r] Stock HUD per character: `common_texture_portrait_<c>` 32×32 CI8 + `common_tlut_portrait_<c>` (3 px blue frame, black inside, 26×26 art), `common_texture_minimap_kart_<c>` 8×8 RGBA16 (top-down kart in the character's colour), `common_texture_minimap_<c>` 8×8 progress dot (magenta for all). DK's suffix is `donkey_kong` for portrait/kart, `dk` for the dot. Kart icon colours taken: red, green, teal, cyan, brown, yellow, pink, purple; Diddy gets orange (248,120,0).
- Tool `tools/n64kr/blender_dkr_racer_hud.py`: renders the racer's head (driving pose frame 40, 3/4 view facing right like the stock portraits, 4× supersampled) into the template's frame; recolours the template kart icon; copies the dot. Writes PNG + stub resource each into the character mod. Output for Diddy: `build/x64/mods/n64kr-dkr-diddy/textures/common_data/*_diddy`.
- Registry field `hudName` ("diddy"). `n64kr_hud_texture(characterId, stockPath)` swaps the `hud` character's suffix for hudName; `n64kr_char_name(characterId)` returns MK64's name text for stock characters and the registry name in capitals otherwise ("DIDDY").
- Hooks: `render_objects.c` portraits (2) and minimap icons (4) through `n64kr_hud_texture`; `menu_items.c` name text (3: GP standings, results, time records) through `n64kr_char_name`.
- `kart_sprites.cpp` renamed `character_assets.cpp` (kart + HUD paths).
- [VERIFIED: build] Debug: 0 errors. In-game check pending.
- Not covered yet: the post-GP results face (`D_800E7D54`, menu textures) and the select screen face/nameplate (step 3, grid).

## Voice: plan [CONCLUSION from source reading]
- MK64 voices are N64 VADPCM samples in instrument banks (`sound/banks/*`, `sound/samples/*` in mk64.o2r; `src/port/resource/importers/AudioSampleFactory.cpp`), triggered as `voice * 0x10 + SOUND_ARG_LOAD(0x29, 0x00, 0x80, slot)` through the `sound_player` sequence. A 9th voice set inside that system needs new sequence bytecode and bank instruments.
- DKR has Diddy's full set (`include/sound_ids.h`: POSITIVE1–8, NEGATIVE1–8, LAUGH/LAUGH2, WOAH, SELECT "I'm Diddy"); audio assets are libultra banks the decomp tool keeps as raw blobs (`ASSET_AUDIO_*`).
- Plan: decode Diddy's DKR clips to WAV in his mod; at runtime mix them into the port's output buffer (`src/port/Engine.cpp` before `AudioPlayerPlayFrame`, one hook), with distance-based volume; route MK64's voice slots for characters with an own voice to DKR clips instead of `play_sound`.

## Step 2c: own voice (built)

### DKR side [VERIFIED: decomp source + ROM]
- `ASSET_AUDIO` = section 39, table 38. Entry 1 = sound-effect bank file (libultra `ALBankFile`, `include/PR/libaudio.h`, offsets from file start), entry 2 = wave data (`.tbl`), entry 6 = sound table (`SoundData`, 10 bytes per id) (`src/audio.c:90-114`). A sound id plays bank 0, instrument 0, sound `soundBite - 1` (`src/audiosfx.c:698`). Bank rate 22050 Hz.
- Voice ids (`include/sound_ids.h`): POSITIVE1..8 from 0x162 and NEGATIVE1..8 from 0x1C2, stride 12 over Krunch, Bumper, Tiptup, Conker, Timber, Banjo, Drumstick, Pipsy, T.T., Diddy, unused×2. Diddy also: LAUGH 0x42, LAUGH2 0x44, WOAH 0x9C, SELECT 0x90 ("I'm Diddy").
- Tool `tools/n64kr/dkr_voice_to_wav.py`: parses the bank, decodes VADPCM (N64 tools' algorithm: codebook expansion, 2×8-sample halves, floor division by 2048), writes 16-bit WAVs. Diddy: 20 clips, RMS ~7–14k with a few full-scale samples (hot mastering; smooth waveforms) → `build/x64/mods/n64kr-dkr-diddy/sound/n64kr/diddy/`.

### MK64 side
- [VERIFIED: source] All voice lines are `voice * 0x10 + SOUND_ARG_LOAD(0x29, 0x00, 0x80, slot)` and every request goes through `play_sound` (`src/audio/external.c:1278`) with a listener-relative position (CPU lines rotated into camera space, `func_800C1DA4`; a human's own lines sit at the listener). MK64 never uses bank-2 voice ids with low byte ≥ 0x80.
- New accessor `n64kr_char_voice_line`: stock voice index, or the character's own id (≥ 9) when it has a `voiceName`. 63 voice-line sites now use it (61 `n64kr_char_voice(x) * 0x10` + 2 results-menu lines on `D_802874D8.unk1E`); engine sound and its tuning stay on `n64kr_char_voice` (Toad).
- `src/n64kr/voice.{h,cpp}`: `n64kr_voice_play_sound` (hook at the top of `play_sound`) catches ids 0x90+, picks a clip for the slot, loads the WAV from the mod (`LoadFileProcess`, as custom music does), plays it through the port's HMAS mixer (`src/port/audio/HMAS.cpp`, already mixed into the output with master volume) at gain 0.6 × distance falloff (full within 100 units, silent at 1200). One line per character per 150 ms (split screen requests once per camera).
- Slot → clips [CONCLUSION from call sites]: 0x03 finished 5th+, 0x05 fell in water/lava ("woah"), 0x06 hit by item/object, 0x0B CPU hit → negative; 0x07 GP 1st, 0x04 podium → win (laughs + big positives); 0x08 fell/out of bounds → "woah"; 0x0E select confirm → "I'm Diddy" (hook in `menus.c` via `n64kr_select_voice`); others → positive.
- `src/port/Engine.cpp`: the HMAS + game audio sum is clamped (was a plain s16 add that wraps on loud overlaps).
- [VERIFIED: build] Debug: 0 errors. In-game check pending.
- In game the user heard the voice "super sped up". Cause [VERIFIED: decomp + ROM]: DKR plays a sound at the 22050 Hz synth output rate × sound-table pitch × `slideMult = cents2ratio(keyBase·100 − 6000 [+ detune])` (`src/audiosfx.c:10-15, 590-597`; DKR reuses the key map as sound parameters); the bank header rate is not used. Diddy's clips have keyBase 48 (one octave down): 11025 Hz, not 22050. Tool fixed; clips now 0.22–1.06 s ("I'm Diddy" 1.06 s). Only the mod's WAVs changed; no rebuild. In-game recheck pending.
- Next report: "I'm Diddy" plays, no other lines. [VERIFIED: game log, new per-request log line] Every race line arrived with distance exactly 100000 → volume 0 ("too far"). Cause [VERIFIED: source]: MK64 marks a finished sound by writing y = 100000 into its position (`src/audio/external.c` `func_800C41CC`, `func_800C4398`, `delete_sound_from_bank`), and a player's sounds all share `D_800E9F7C[player].pos`, so after the first finished sound in a race the shared position reads as far away. On the select screen nothing had finished yet. Fix: distance on the ground plane (x, z) only; MK64's listener-relative vectors keep y = 0 anyway (`func_800C1C88`). In-game recheck pending.
- [VERIFIED: user playtest] After the distance fix Diddy talks in races ("Yes"). Per-request log line moved to debug level.
