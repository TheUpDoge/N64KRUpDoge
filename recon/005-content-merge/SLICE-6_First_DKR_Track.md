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
4. **Other racers overlap and look transparent.** Cause unknown; needs a closer description/screenshot. [HYPOTHESIS] draw-layer ordering of DKR's translucent water/decals, or every section using id 0xFF.
