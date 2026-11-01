# SLICE-6 — First DKR track (Ancient Lake)

Project: N64 Kart Racing (N64KR)
Date: 2026-11-01
Status: DRIVABLE (user playtest 2026-11-01). Follow-up issues open.

## Converter: `tools/n64kr/dkr_level_to_gltf.py`
Reads the user's own DKR ROM (USA 1.1 / v80 only, SHA-1 checked) and writes glTF 2.0 + PNG textures under `build/dkr_tracks/<track>/` (git-ignored; ROM data, never committed). The script holds no game data.

```powershell
python -I tools/n64kr/dkr_level_to_gltf.py "<DKR .z64>" --list
python -I tools/n64kr/dkr_level_to_gltf.py "<DKR .z64>" ancient_lake build/dkr_tracks
```

### Format facts used [VERIFIED: DKR decomp a5eb080]
- Asset lookup table (see SLICE-5). Sections: 3D textures = 2 (table 3), level models = 27 (table 26). Level model table has 56 entries for v80, matching the decomp asset list minus v79-only variants.
- DKR compression = 5-byte header + raw deflate (`src/gzip.c:59-66`).
- Level model pointers are offsets from the model start (`src/tracks.c:2862-2873`).
- `LevelModelSegment` (0x44 bytes), `TriangleBatchInfo` (12 bytes; batch `i` spans faces `facesOffset[i] .. facesOffset[i+1]`, vertex indices are relative to `verticesOffset`), `Triangle` (16 bytes: flags, 3 indices, 3 × s16 UV pairs), `Vertex` (10 bytes: s16 xyz, RGBA) (`include/structs.h:574-736`).
- Vertices with `r == 1 && g == 1` carry alpha in `b` and render at 0x80 grey (`src/tracks.c:2900-2915`).
- UVs are 1/32-texel fixed point [CONCLUSION: Ancient Lake's UVs span ±560 texture widths raw, ±17 after ÷32].
- Batch render flags (`src/textures_sprites.h:41-78`): `RENDER_HIDDEN` 0x100 (invisible wall), `RENDER_NO_COLLISION` 0x200, `RENDER_WATER` 0x2000, semi-transparent 0x4, cutout 0x10.
- Textures: 32-byte header (width, height, format low nibble, flags be16 @6 with interlace 0x400 and clamp 0x40/0x80, frame count byte @0x12, compressed @0x1D); compressed data inflates to header + pixels; odd rows of interlaced textures swap 4-byte (8 for RGBA32) chunks. Matches the decomp asset tool (`tools/dkr_assets_tool_src/extract/extractTypes/extractTexture.cpp`, `helpers/imageHelper.cpp:191-221`). CI4/CI8 not decoded yet (magenta).

### Output layout
- Two objects: `<track>_visible` and `<track>_invisible_walls` (`RENDER_HIDDEN` batches).
- One material per texture + flags, named `dkr_tex####[_water][_nocollide][_transparent][_hidden]`, so collision and surface types can be assigned by material in Blender.
- Coordinates: DKR units, Y up, unscaled. Vertex colours in `COLOR_0`; textures not flipped (N64 orientation, UVs match).

### Ancient Lake check [VERIFIED: converter output + preview render]
- 1156 triangles (76 invisible walls), 26 materials, 25 textures (RGBA16 ×17, RGBA32 ×5, IA8 ×3; none interlaced, no CI).
- Bounds: x −5918..−23, y −56..885, z −12559..−2048.
- A top-down textured render shows Ancient Lake's figure-8 track, lake, grass, sand banks, start banner and props. Textures decode correctly (grass, sand, water, boost arrows, checkered flag, START banner).

## SpaghettiKart custom track format [VERIFIED: `src/engine/tracks/CustomTrack.cpp:99-158`]
A mod folder/archive with `scene.json` (props, minimap, actors, environment, read by `TrackEditor::LoadTrackDataFromJson`), `data_track_sections` (array of `TrackSections`: display-list CRC, draw layer, collision clip type, surface type, section id) and the display-list resources. Collision is generated from those display lists (`ParseMeshForCollision`). Fast64 (HarbourMasters fork) writes this from Blender.

## Blender setup [VERIFIED: command output, 2026-11-01]
- Blender 4.3.2 portable: `E:\Claude\tools\blender-4.3.2-windows-x64\blender.exe` (official `download.blender.org` zip, SHA-256 `933a62a7…d4f6` matched). Settings live in its `portable\` folder, so it doesn't touch any other Blender install.
- Fast64 (HM64) 2.5.4, HarbourMasters fork `1c919b6` (2026-10-18), installed at `portable\scripts\addons\fast64` and enabled (survives restart).
- Fast64 MK64 export (`fast64_internal/hm64/mk64`): select the Track Root empty (`hm64_mk64_obj_type == "Track Root"`), set `scene.hm64_mk64_export_name` / `hm64_mk64_export_path`, run `scene.mk64_export_course`. Per **object**: `hm64_mk64_surface_type`, `clip_type`, `draw_layer`, `section_id`. Paths are NURBS/Bezier curves with `hm64_mk64_path_type`. Export scale `scene.fast64.mk64.scale` = 100. HM64 feature set writes `data_track_sections` + `data_paths` XML.
- Ancient Lake glTF imports cleanly: `ancient_lake_visible` 1080 tris / 24 materials, `ancient_lake_invisible_walls` 76 tris; size 5895 × 10511 × 941 (DKR units), so it needs scaling for MK64.

## Next
- Work out the DKR → MK64 scale (compare road widths / kart sizes).
- In Blender: import the glTF, scale to MK64 size, set up Track Root, surfaces, path, spawn; export.
- **[HYPOTHESIS]** Later, a direct exporter (DKR data → `scene.json` + `data_track_sections` + display lists, path from DKR checkpoints) could skip Blender and convert every DKR track automatically.

## Automated pipeline (2026-11-01)
1. `python -I tools/n64kr/dkr_level_to_gltf.py "<DKR .z64>" ancient_lake build/dkr_tracks --mk64`
2. `blender.exe -b --python tools/n64kr/blender_dkr_track.py -- build/dkr_tracks/ancient_lake_mk64 build/x64/mods ancient_lake`
3. `python -I tools/n64kr/dkr_track_scene.py build/dkr_tracks/ancient_lake_mk64 build/x64/mods ancient_lake "Ancient Lake"`

Output: `build/x64/mods/n64kr-dkr-ancient-lake/` (git-ignored; ROM data).

### DKR level objects [VERIFIED: DKR decomp + probe]
- Level header (section 23, table 22; 200 bytes): `geometry` @0x34, collectables map @0x36, main object map @0xBA (`src/game.c:555` → `init_track` → `track_spawn_objects`).
- Object map (section 21, table 20): DKR-compressed; u32 byte size, entries from offset 16 (`gObjectMap` is `s32 *`, spawn list = `gObjectMap + 4`); entry length = `byte1 & 0x3F`, object id = `byte0 | (byte1 & 0x80) << 1`, mapped through the translation table (section 35) to an object header (internal name at 0x60).
- Ancient Lake: 33 `checkpoint` (18 unique non-alt indices), 8 `setuppoint`, 8 `WeaponBalloon`, 2 `GroundZipper`, plus scenery, coins, camera controls, AI nodes.
- `LevelObjectEntry_Checkpoint`: index @0x09, angleY @0x0A, isAltCheckpoint @0x17, vehicleType @0x1A. `LevelObjectEntry_SetupPoint`: racerIndex @0x08, angleY @0x0A (`include/level_object_entries.h`).

### MK64 layout (`--mk64`)
- MK64 builds the start grid from path point 0: karts at x ±20, z +30..+170 behind it, racing towards −Z (`src/spawn_players.c` `spawn_and_set_player_spawns`).
- So: start line = 20 MK64 units ahead of DKR's front grid row (racers 0+1), towards checkpoint 0; rotate so that direction is −Z; translate to origin; scale ×0.5 [HYPOTHESIS: matches the hover speed calibration, DKR 12 → MK64 5.9 units/tick].
- Path: start line + checkpoints in index order, closed Catmull-Rom loop every 20 units, Y snapped to the floor below. Ancient Lake: 470 points. Written with start grid / item boxes / zippers to `<track>_mk64.json`.
- Surfaces from the DKR texture's `surfaceType` (TextureInfo byte 7): default→ASPHALT, grass→GRASS, sand→SAND_OFFROAD, zip pad→BOOST_RAMP_ASPHALT, stone→DIRT_OFFROAD, water→WET_SAND, frozen water→ICE, snow→SNOW; invisible wall (17) and `RENDER_HIDDEN` batches → CLIP_DOUBLE_SIDED_WALL + DRAW_INVISIBLE; `RENDER_NO_COLLISION` → CLIP_NONE; transparent/water → DRAW_TRANSLUCENT. Stored as glTF material extras.

### Blender script (`tools/n64kr/blender_dkr_track.py`) [VERIFIED: runs headless, export FINISHED]
- Import glTF, weld duplicate vertices, split by material, set per-object `hm64_mk64_surface_type` / `clip_type` / `draw_layer` from material extras.
- `Col`/`Alpha` corner attributes = sRGB→linear of the imported colours, because Fast64 gamma-corrects on export (`f3d/f3d_writer.py` `getLoopColor`).
- glTF importer puts a vertex-colour multiply before Base Color; the script links the image node directly so `object.convert_bsdf` works, with presets `vertex_colored_texture` / `vertex_colored_texture_transparent`.
- Track Root empty + Bezier path (`TRACK_PATH_1`), `scene.fast64.mk64.scale = 1` (geometry already in MK64 units), `scene.mk64_export_course`. Saves the `.blend` for hand edits.
- Export check: `data_paths` 471 points starting `0,0,0` towards −Z; `data_track_sections` 19 sections with the expected surface / clip / draw layer.

### Game [VERIFIED: game log]
- `ModManager`: "Loaded mod: n64kr-dkr-ancient-lake", archive `./mods/n64kr-dkr-ancient-lake` added.
- Custom tracks are picked from the start-screen debug menu: Developer → Debug Mode (`gEnableDebugMode`; `src/menus.c:1875`), then the COURSE row cycles registered tracks (`TrackBrowser_NextTrack`).

### Not done yet
- Item boxes (DKR balloons) and zippers are in the JSON but not placed in the track (Fast64 HM64 leaves actors to the in-game editor).
- Scale 0.5 untested in game; skybox, minimap, music use CustomTrack defaults.

## First drive [VERIFIED: user playtest, 2026-11-01]
- Ancient Lake loads, spawns and drives: user completed all three laps; "it actually drives and feels really good". So the ×0.5 scale, start-line placement, path/lap counting and collision work.

### Issues found
1. **Minimap is off.** Expected: CustomTrack uses Mario Raceway's minimap texture and default positions (`src/engine/tracks/CustomTrack.cpp` constructor). Needs a generated minimap and scene.json minimap settings.
2. **Missing visuals.** [HYPOTHESIS] DKR scenery (trees, `Animation` objects, props) are separate object models placed by the object map, not level geometry, so the converter doesn't include them yet. Skybox is CustomTrack's default.
3. **No pickups.** Item boxes (8 DKR `WeaponBalloon` positions) and zippers are in `<track>_mk64.json` but not placed.
   - Fix: `tools/n64kr/dkr_track_scene.py` writes `scene.json` (keeps game-generated Props; sets Name, DebugName, ResourceName `n64kr:dkr_<track>`) with `Actors` = `{"Name": "mk:item_box", "Location": {x,y,z}}` (format: `src/engine/SpawnParams.h`, `src/engine/editor/SceneManager.cpp` LoadActors, `src/engine/registry/RegisterActors.cpp`). Each balloon → row of 3 boxes, 18 units apart, across the path. `spawn_item_box` snaps to the surface below `y + 10` and skips Time Trials (`src/racing/actors.c:1081-1096`). Ancient Lake: 24 boxes. [VERIFIED: user playtest] boxes spawn and give items in a race.
4. **Other racers overlap / disappear.** User screenshot at the start line shows MK64's normal staggered grid seen from the back row (racers in the same column line up in perspective) with start-line exhaust smoke; no rendering fault visible. Racers "disappear when way out in front": **stock MK64 culling, not N64KR.** [VERIFIED: source] `check_player_camera_collision` (`src/render_player.c:102`) only draws racers within a per-mode distance (`D_80165580`, set in `src/spawn_players.c:416-444`), and returns true unconditionally when Enhancements → General → Disable Culling (`gNoCulling`) is on (`:116-118`).

## Minimap, scenery, sky, transparency (2026-11-01, not yet tested in game)
- **Minimap** [VERIFIED: source]: without `<track dir>/minimap.png` CustomTrack falls back to Mario Raceway's minimap (`src/engine/editor/SceneManager.cpp` LoadMinimap / SetDefaultMinimap); the image is read 1-channel and packed to 4 bits (`src/port/resource/importers/MinimapFactory.cpp`). Dots are drawn at image top-left + (PlayerX, PlayerY) + (pos.x, pos.z) × PlayerScaleFactor (`src/render_objects.c` draw_minimap_character). `dkr_track_scene.py` now draws the path as a white line with that mapping (Ancient Lake: 40×80, scale 0.0191) and writes the matching Props.
- **Transparency**: DKR marks cutout/semi-transparency on the texture, not only on batch flags; several textures (decals, boost arrows, foliage) were exported opaque. The converter now classifies each texture from its decoded alpha (opaque / cutout / blend, header flag 0x10 = cutout) → Fast64 presets `vertex_colored_texture` / `_cutout` / `_transparent`, blend → DRAW_TRANSLUCENT.
- **Lost decals (bug)**: the Blender script welded duplicate vertices before splitting by material, which merged DKR's decals into the faces below and deleted them as duplicates (7 of 29 materials missing, incl. boost arrows). Now splits first, welds per object. Export: 29 sections, 7 translucent.
- **Scenery** [VERIFIED: DKR decomp]: scenery = objects with behaviour `BHV_SCENERY` (2); placement as `obj_init_scenery` (`src/object_functions.c:113`): model = header.modelIds[entry.modelIndex], scale = header.scale × max(radius, 10)/64, rotation Y = angleY << 8. Ancient Lake's 22 scenery objects (13 Beachtree, 7 RubberTree, 2 BlueBerryBush) are all `OBJECT_MODEL_TYPE_SPRITE_BILLBOARD` (sprites 107, 113, 108). Sprite = 2D texture pieces placed at (posX − anchor.x, anchor.y − posY) (`src/textures_sprites.c:1234`). The converter assembles frame 0 into one image and bakes each object as two crossed double-sided quads (no collision yet). 3D-model scenery (other tracks) is baked from its object model.
- **Sky** [VERIFIED: DKR decomp]: level header 0x38 = object header index of the sky dome, spawned at the origin (`src/tracks.c` skydome_spawn); Ancient Lake: `dome13` (model 13). Converter samples vertex colour × mean texture colour over the dome's top 20% / bottom 15%: top (53,139,249), horizon (116,163,206) → MK64 `Skybox` gradient (order TopRight, BottomRight, BottomLeft, TopLeft, floor ×4: `src/engine/tracks/Track.h`).

### Crash: `tile == 7` assertion (user report, 2026-11-01)
- [VERIFIED: user screenshot + source] Selecting the rebuilt track hit `SUPPORT_CHECK(tile == G_TX_LOADTILE)` in `libultraship/src/fast/interpreter.cpp` `GfxDpLoadTile` (line 2619): libultraship only supports `LoadTile` on tile 7.
- Cause [VERIFIED: exported XML]: only the three sprite materials used `LoadTile` (Fast64's large-texture path). The assembled sprites were 68×123, 121×113, 88×118: not power-of-two and above TMEM (2048 RGBA16 texels).
- Fix: converter resamples sprite images to the power-of-two size within 2048 texels with the closest aspect (palm 32×64, bush and rubber tree 32×32); quad world size unchanged. Re-export has no `LoadTile` at all. `blender_dkr_track.py` now warns after export if any material uses `LoadTile` outside tile 7.

### Layering issues (user report, 2026-11-01)
- Report: shells visible through hills; MK64 finish banner visible through walls and drawn over DKR's start banner; racers overlap / vanish far away.
- Depth is fine [VERIFIED: exported XML]: every material has `G_ZBUFFER` and a ZB render mode (opaque `G_RM_AA_ZB_OPA_SURF2`, cutout `G_RM_AA_ZB_TEX_EDGE2`, translucent `G_RM_AA_ZB_XLU_SURF2`).
- **Culling** [CONCLUSION]: every material had `G_CULL_BACK`. DKR draws many triangles double-sided (Triangle flag 0x40) and culls per triangle, so single-sided-looking faces seen from behind vanished and let objects show through. Fix: `blender_dkr_track.py` sets `g_cull_back = False` on all track materials and turns culling on in Fast64's world defaults, so each material emits an explicit `ClearGeometryMode G_CULL_BACK` (29/29) and its revert list restores it. [HYPOTHESIS] DKR's per-triangle flag could restore single-sided culling later as an optimisation.
- **MK64 finish arch** [VERIFIED: source]: `RaceManager.cpp:80-88` spawns `AFinishline` when `Track::bSpawnFinishline` (default true, `Track.h:333`); not configurable from scene.json. N64KR HOOK in `SceneManager.cpp` `LoadTrackDataFromJson`: reads optional `"SpawnFinishline"` bool. `dkr_track_scene.py` writes `false` (DKR tracks bring their own banner).
- Racers vanishing far away: stock MK64 kart culling (see issue 4 above); Disable Culling turns it off. "Overlapping" may have been the culling/see-through problem; recheck after this fix.

### Playtest after layering fix (user screenshots, 2026-11-01)
- [VERIFIED] Banner issue fixed; trees, sky and DKR start banner render.
- Minimap showed striped lines: the image was 40 px wide; 4-bit (I4) textures need widths that are multiples of 16, otherwise rows are fetched misaligned. Fix: width padded to a multiple of 16 (Ancient Lake: 48×80), track centred, dot offsets recomputed. Not yet re-tested.
- "Only stars and lightning": [VERIFIED: source] the user was 8th; human 8th-place odds are star 30, thunderbolt 20, triple red shell 20, blue shell 15, super mushroom 10, triple mushroom 5 (`src/engine/registry/RegisterItemTables.cpp:106-111`). Working as MK64 intends.
- Backing into a wall filled the screen with the sand cliff: the chase camera went inside DKR's cliff geometry (tall walls right beside the track). With culling now off its inside faces are drawn; previously they were culled, which is what looked like "shells through hills". [HYPOTHESIS] camera collision against walls would avoid it; not addressed yet.
