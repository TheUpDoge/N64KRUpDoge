# SLICE-6 — First DKR track (Ancient Lake)

Project: N64 Kart Racing (N64KR)
Date: 2026-11-01
Status: IN PROGRESS. Converter done and checked; Blender/Fast64 step next.

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

## Next
- Blender 4.3 or older + HarbourMasters Fast64 (`docs/tracks/trackoverview.md`). Not found on this machine yet.
- In Blender: import the glTF, scale to MK64 size, set up Track Root, surfaces, path, spawn; export.
- **[HYPOTHESIS]** Later, a direct exporter (DKR data → `scene.json` + `data_track_sections` + display lists, path from DKR checkpoints) could skip Blender and convert every DKR track automatically.
