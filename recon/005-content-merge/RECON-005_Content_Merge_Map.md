# RECON-005 — Merging DKR tracks and characters into N64KR

Project: N64 Kart Racing (N64KR)
Date: 2026-11-01
Status: COMPLETE (first pass). Plan, no code yet.

Tags: **[VERIFIED]** read in source/docs · **[CONCLUSION]** follows from verified facts · **[HYPOTHESIS]** not yet checked.

Sources: SpaghettiKart `ce0f935` (`E:\Claude\N64KR`, branch `n64kr`), DKR decomp `a5eb080` (`E:\Claude\refs\Diddy-Kong-Racing`).

**Rule that applies throughout:** anything converted from a DKR or MK64 ROM (models, textures, tracks, sprites) is built locally from the user's own ROM and **never committed**. Only the converter tools and N64KR code go into git.

---

## 1. What SpaghettiKart already supports

### Tracks [VERIFIED]
- Built-in tracks register in `src/engine/registry/RegisterTracks.cpp` (`Registry<TrackInfo>`, namespaced `ResourceName` such as `mk:mario_raceway`; "do not re-order").
- **Custom tracks are supported**: made in Blender with HarbourMasters Fast64 (`docs/tracks/*`), packed as `.o2r`/`.zip`/folder mods in `mods/` with a `mods.toml` (`docs/modding.md`). `src/engine/TrackBrowser.cpp` finds them (`mods:<name>`), and `src/engine/tracks/CustomTrack.cpp` runs them. They need a drivable mesh, a path (CPU driving and lap progress) and a spawn area.

### Characters [VERIFIED]
- `docs/characters/characteroverview.md`: "Custom characters can only replace existing characters for now." A character is **pre-rendered sprite frames** (`mario_kart/mario_kart_frame###` etc.) plus portrait, select-screen faces and nameplate. Listed as future plans: new characters beyond the 8, and 3D character models.
- 8 characters are wired deep into the engine: `#define NUM_PLAYERS 8`, ids `MARIO 0 … BOWSER 7` (`include/defines.h`), and `characterId` appears **195 times in 22 files** (physics tables, sprite tables in `src/kart_dma.c`, sounds, HUD, save data, menus).

### Menus [VERIFIED]
- `docs/modding.md`: "Custom menus are not supported yet." Character select is MK64's original C menu code (`src/menus.c`, `src/menu_items.c`). The ImGui menu bar (`src/port/ui/PortMenu.cpp`) is where N64KR's profile picker lives today.

---

## 2. What DKR's decomp gives us

### Tracks [VERIFIED]
- 40+ level models in the asset list (`tools/dkr_assets_tool_extract.json`): race tracks such as `ancient_lake`, `fossil_canyon`, `jungle_falls`, `hot_top_volcano`, `whale_bay`, `crescent_island`, `pirate_lagoon`, `walrus_cove`, `everfrost_peak`, `spaceport_alpha`, `star_city`…, plus hubs, bosses, menus.
- The decomp's asset tool extracts level models only as **raw `.bin`** (`extractLevelModel.cpp` writes JSON + raw data). Its builder reads glTF for *objects*, but there is no level-to-glTF exporter.
- The format is documented in `include/structs.h`: `LevelModel` (textures, segments, BSP, minimap data) → `LevelModelSegment` (vertices, triangles, triangle batches, collision planes) → `Vertex` (s16 x/y/z + RGBA colour), `Triangle` (flags + 3 vertex indices + UVs), `TriangleBatchInfo` (texture index, vertex/face offsets, render flags).
- Loaded with `gzip_inflate` after `asset_load(ASSET_LEVEL_MODELS, …)` (`src/tracks.c:2844-2860`), so it's deflate-compressed.
- DKR renders with its own microcode, **F3DDKR** (`include/f3ddkr.h`). **[HYPOTHESIS]** libultraship's renderer doesn't support F3DDKR, so DKR display lists can't be drawn as-is. Converting to plain geometry avoids that.

### Characters [VERIFIED]
- DKR racers are **3D object models** with vertex animation (object headers, `ObjectModel`, animation assets), one per character per vehicle (we already read the hovercraft headers in SLICE-5). Not sprites.

---

## 3. Options

### Tracks
| Option | What it takes | Verdict |
|---|---|---|
| **A. Convert DKR levels → glTF → Blender/Fast64 → SpaghettiKart custom track** | New local converter (DKR level `.bin` + textures → glTF). Then in Blender: add path, spawn, surfaces, export with Fast64. | **Recommended first.** Uses SpaghettiKart's supported pipeline; no engine changes to get a drivable track. |
| B. Native "DKR track" class that loads converted geometry directly | Converter + a new `Track` subclass + path/collision built from DKR data (checkpoints, AI nodes in object maps). | Later, to automate many tracks once A works. |
| C. Render DKR display lists directly | F3DDKR support in libultraship. | Not worth it. |

### Characters
| Option | What it takes | Verdict |
|---|---|---|
| **A. DKR characters as sprite replacements** | Convert DKR racer model → glTF, render the MK64 sprite frames in Blender (existing character pipeline). | **Fastest**, but limited to replacing the 8 MK64 slots. |
| **B. Expand the roster beyond 8** | Character registry; route `characterId` arrays (195 uses / 22 files) through it, like SLICE-1 did for physics; new select screen. | **Needed for a real combined game.** Large but the same seam technique as SLICE-1. |
| C. 3D model racers | New 3D racer renderer + animation playback. | Biggest; later. |

### Selection screens
| Option | Verdict |
|---|---|
| ImGui overlay (like the N64KR menu) | Quick to build and test; not game-like. Good for the first vehicle picker. |
| New in-game screen (game state, N64 style) | The real goal. Needs the character registry (Characters B) first, so it can list more than 8. |

---

## 4. Recommended order (one checkpoint at a time)
1. **SLICE-6: first DKR track.** Write the DKR level → glTF converter (local, output never committed). Convert one simple track (Ancient Lake: small oval). Bring it into Blender, add a path and spawn, export with Fast64 as a custom-track mod. Pass: drive laps on it with a kart and the hovercraft.
2. **SLICE-7: vehicle selection.** A pre-race "Vehicle: Kart / Hovercraft" choice for each human player, replacing the debug profile picker. Start as an ImGui screen, keep the logic separate so a real screen can replace it.
3. **SLICE-8: character registry.** Route `characterId` lookups through `n64kr` character definitions (sprites, portrait, name, sounds, stats link), still 8 entries. Pass: identical game.
4. **SLICE-9: more than 8 characters + character select screen.** Add a DKR character (sprite route first), new select screen listing the registry.
5. Later: more DKR tracks (automate with option B), 3D racers, other games.

## 5. Open questions
- **[UNKNOWN]** Exact `Triangle` UV layout and DKR texture formats (texture headers, `asset_table_load_zipped`). Read before writing the converter.
- **[UNKNOWN]** Whether Fast64's MK64 mode handles large DKR-scale meshes and vertex-coloured, multi-texture tracks without manual cleanup.
- **[UNKNOWN]** DKR → MK64 world scale (DKR levels may need scaling to fit MK64 kart sizes).
- **[UNKNOWN]** Whether a community DKR level exporter already exists (would save writing one). Not searched yet.
- **[HYPOTHESIS]** DKR checkpoint objects in level object maps can become the custom track's path.
