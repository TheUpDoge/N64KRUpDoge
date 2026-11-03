# Tutorial: GoldenEye tracks, hazards and music

How Dam Speedway is generated from GoldenEye, how to put GoldenEye's guards, barrels and music on **any** track (no code), and how to build a new GoldenEye track (Rust). You need the setup from [TUTORIAL_SETUP.md](TUTORIAL_SETUP.md), including the Rust tool:

```bash
cd tools/n64kr-rs && cargo build --release && cd ../..
N64KR=tools/n64kr-rs/target/release/n64kr
GE="path/to/007 - GoldenEye (USA).z64"
```

Run `$N64KR` with no arguments for the command list. Levels are named `dam`, `facility`, `runway`, `surface`, `bunker`, `silo`, `frigate`, `statue`, `archives`, `streets`, and so on.

## 1. Explore a level

| Command | What you get |
|---|---|
| `$N64KR ge-bg-info "$GE" runway` | Every room of the level: position, vertex count, bounding box |
| `$N64KR ge-gltf "$GE" runway build/ge/runway` | The whole level as glTF with textures (GoldenEye units, Y up) to open in Blender |
| `$N64KR ge-rooms-gltf "$GE" dam build/ge/towers 78 79 80` | Only the rooms you list (78–80 are Dam's watchtower) |
| `$N64KR ge-textures "$GE" dam build/ge/dam_tex` | The level's textures as `tex_NNNN.png`, to pick road and wall textures by number |
| `$N64KR ge-model "$GE" PgasbarrelZ build/ge/barrel` | An object model (props, guards, heads, guns) as glTF |
| `$N64KR ge-model "$GE" ColiveguardZ build/ge/guard fire_standing 16` | A guard posed at an animation frame |
| `$N64KR ge-images "$GE" build/ge/fx 2119 2128` | Images by number (muzzle flash, tracer, ...) |

## 2. How Dam Speedway is built

`ge-dam-speedway` (`fn ge_dam_speedway` in `tools/n64kr-rs/src/main.rs`) writes the same kind of converted folder the DKR converter does, so the rest of the pipeline is shared:

1. **Load the level** and keep the outdoor rooms (rooms below the bunker depth are skipped). Scale GoldenEye units by 0.35 to MK64 size.
2. **Find the water:** the height with the most flat, upward-facing area is the reservoir.
3. **Fit the oval** (`track.rs`, `Oval`) inside the reservoir's shores, measured on a height map. A = one oval; B = a second Dam turned 180° and joined across the water, with a longer oval.
4. **Build the road:** banked turns (10°), Dam's own asphalt (texture 22), red/white striped walls (texture 18), 60 units high. Road triangles are wound to face up, because MK64 treats down-facing triangles as ceilings.
5. **Clear the corridor:** scenery triangles that would poke through the road are dropped. The rest of the level stays as non-solid scenery.
6. **Watchtowers:** rooms 78–80 are copied to two spots beside the road, with a guard on each balcony.
7. **Layout JSON:** the AI path, start grid, item boxes, hazards (`"actors"`) and the music (`"sequence"`).

Then `blender_dkr_track.py` and `n64kr dkr-track-scene` turn it into a mod, with `ge` as the prefix, so the track id is `n64kr:ge_dam_speedway`.

## 3. Guards, barrels and music on any track (no code)

Generate the shared GoldenEye assets once:

```bash
$N64KR ge-hazards "$GE" build/x64/mods/n64kr-ge-assets
$N64KR ge-audio "$GE" build/x64/mods/n64kr-ge-assets
$N64KR ge-intro "$GE" build/x64/mods/n64kr-ge-assets   # the boot intro's gun barrel
```

`ge-intro <ROM> <mod dir> [preview dir]` also writes contact sheets of Bond's poses (side and front, a quick software render) and the rifling image into the preview folder, so you can check poses without starting the game.

Any custom track's `scene.json` can then use them: a converted DKR track, or a track you made in SpaghettiKart's editor.

**Hazards:** add entries to the `"Actors"` list. Positions are in the track's MK64 units, and `yaw` is 0–65535:

```json
{ "Name": "n64kr:ge_barrel", "Location": { "x": 120.0, "y": 4.0, "z": -800.0 } },
{ "Name": "n64kr:ge_guard", "Location": { "x": -300.0, "y": 60.0, "z": -1200.0 },
  "Rotation": { "pitch": 0, "yaw": 16384, "roll": 0 }, "Skin": "karl" }
```

- Barrels explode when a racer touches them (racers tumble), set off barrels nearby, and come back after 10 seconds.
- Guards raise their KF7 and fire three-shot bursts at racers within range. A hit spins the racer out like a banana.
- `Skin` picks the head: `dave`, `karl`, `mark` or `jim`.
- Re-running `dkr-track-scene` rewrites the `Actors` list from the converted layout. To keep your additions, put them in the layout JSON's `"actors"` list (`build/.../<track>_mk64.json`), which the scene tool copies in.

**Music:** set `"Sequence"` in the track's `"Props"` to an N64KR song id:

| Song | id (decimal) | id (hex) |
|---|---|---|
| Dam (switches to Dam X on the final lap) | 20032 | `0x4E40` |
| Dam X (action version) | 20033 | `0x4E41` |
| Intro | 20034 | `0x4E42` |
| Runway | 20035 | `0x4E43` |
| Facility | 20036 | `0x4E44` |

## 4. Build your own GoldenEye track (Rust)

Say you want a Runway Speedway:

1. **Explore Runway** (section 1): `ge-bg-info` for the rooms and their bounding boxes, `ge-gltf` to look at it in Blender, `ge-textures` to pick road and wall textures.
2. **Copy `ge_dam_speedway`** in `tools/n64kr-rs/src/main.rs` to `ge_runway_speedway`, and add a line to the command match in `main()`:
   ```rust
   ["ge-runway-speedway", rom, dir] => ge_runway_speedway(rom, dir),
   ```
3. **Change what's Dam-specific:**
   - the level name passed to `load_level`;
   - the track name;
   - `ROAD_TEXTURE` and `WALL_TEXTURE`;
   - how the ground height is found (Runway has no reservoir: use the runway's flat area);
   - the oval's centre, half-length and radius;
   - the tower rooms, or none;
   - where barrels go;
   - `"sequence"`: 20035 for the Runway theme.
4. **Run it,** then the shared steps:
   ```bash
   $N64KR ge-runway-speedway "$GE" build/ge_tracks/runway_speedway
   "$BLENDER" -b -P tools/n64kr/blender_dkr_track.py -- build/ge_tracks/runway_speedway build/x64/mods runway_speedway ge
   $N64KR dkr-track-scene build/ge_tracks/runway_speedway build/x64/mods runway_speedway "Runway Speedway" ge
   ```
5. **Add `n64kr:ge_runway_speedway`** to a cup ([TUTORIAL_ADD_DKR_TRACK.md](TUTORIAL_ADD_DKR_TRACK.md), section 3).

Lessons from Dam that will save you time:
- Keep the road **counter-clockwise wound** (seen from above), or racers fall through.
- Keep banking low (10°). At 25°, karts slid down to the inside wall and stuck.
- Spawn item boxes at road height; MK64 floats them 8.66 units up on its own.
- Clear scenery out of the road corridor: rocks and towers poking through the road read as collision bugs.

## 5. Adding another GoldenEye song

Songs are listed in `SONGS` in `tools/n64kr-rs/src/ge/audio.rs` (name, ROM offset, size and default volume of each `Mxxx.bin`, from the decomp's `scripts/filelist.u.csv` and `src/music.c`), and in `kSongs` in `src/n64kr/music.cpp` with an N64KR id from `src/n64kr/music.h`. Add the song in both places, re-run `ge-audio`, and use the new id in a track's `"Sequence"`.
