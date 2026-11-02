# RECON-008 — GoldenEye Dam as a race track

Project: N64 Kart Racing (N64KR)
Date: 2026-11-01
Status: Dam Speedway A and B with hazards, music and sound built; in-game test pending

## Goal (user, 2026-11-01)
A custom race track built from GoldenEye 007's Dam level, "maybe 2 to 4 of them linked together, just to push the limits". Later phases asked for: GoldenEye guards kept on the track and shooting at racers; GoldenEye sounds; the GoldenEye intro.

## Sources
- GoldenEye decomp: github.com/n64decomp/007 (cloned read-only to `E:\Claude\refs\goldeneye-007`, commit c435646). Complete, matching decomp; level data structures in `assets/obseg/bg/bg_all_p.h`, loaders in `src/game/bg.c`, Rare inflate in `src/game/decompress.c`, ROM file offsets in `scripts/filelist.u.csv`.
- A native GoldenEye PC port built from the decomp exists (github.com/jkdansereau/goldeneye-pc-port) — reference only.
- User's ROM: `E:\Claude\roms\007 - GoldenEye (USA).z64`, SHA-1 abe01e4aeb033b6c0836819f549c791b26cfde83 = the decomp's US ROM (`ge007.u.sha1`). [VERIFIED]

## Format facts [VERIFIED: decomp source + extraction]
- Level geometry is a "bg" file: Dam = `bg_dam_all_p`, ROM offset 6290512, 197,024 bytes, stored uncompressed (`filelist.u.csv`). Level scale 0.23363999 (`bg.c` levelinfotable).
- bg header @0: reserved, room_data_table*, portal_data_table*, global_visibility_commands*; pointers are segment 0x0F addresses (0x0F000000 + file offset). Room table: 24-byte entries {pointtablebinary*, primappingbinary*, secmappingbinary*, f32 x, y, z}; entry 0 empty; ends with a marker entry pointing at an empty block (Dam's 137), then all zeros.
- Each binary is Rare "1172" compressed: bytes 0x11 0x72, then a raw deflate stream, no stored size (`decompressdata`: checks the 2 bytes, `rz_inbuf + 2`, zlib_inflate). Point table → N64 Vtx (16 bytes); primary mapping → opaque display list, secondary → transparent display list (`bgLoadRoomVtxData`, `bgLoadRoomPrimaryGdl`). Textures are resolved from GoldenEye's texture bank when a room loads (`texLoadFromGdl`) — next to decode.
- Collision is a separate "stan" file (`Tbg_dam_all_p_stanZ`); for a kart track MK64 collision comes from the exported track mesh instead.
- Extraction result (Rust tool, `n64kr ge-bg-info`): **Dam = 136 rooms, 21,722 vertices, 7,004 primary + 1,728 secondary display list commands.** Bytes in the ROM match the decomp's own copy of room 1.

## Tool
`tools/n64kr-rs/` — the first Rust tool (user preference: new tools in Rust). One `n64kr` program with subcommands; `cargo build --release` in that folder. Modules: `rom` (ROM loading incl. .v64/.n64 byte orders, SHA-1 check), `rarezip` (1172), `ge` (US ROM tables), `ge::bg` (bg parse + room decompression).

## Plan
1. Display list interpreter (GoldenEye's F3D variant: vertex loads, triangles, texture state) + texture bank decoding (`tex.c`) → glTF of the whole level with textures. Top-down render to see the drivable route.
2. Track design: Dam is a one-way route (access road and tunnels, dam top, down to the bunker area), not a loop. Join 2–4 copies end to start (rotated, with connecting pieces we add) so the route closes into a lap; pick the layout from the top-down render with the user.
3. Into the existing track pipeline (Fast64 export, `scene.json`: start line, AI path, item boxes, minimap), as Ancient Lake was; then a "DKR Cup"/custom cup page with it.
4. Later: guards as track hazards (positions from Dam's setup file, guard models/animations from the ROM, shots spin racers out), GoldenEye sounds, intro.

## Step 1 results [VERIFIED: tool runs + renders]
- Textures: `src/ge/tex.rs` ports image.c's decoders (zlib paletted path; Rare's uncompressed / Huffman / per-channel Huffman / RLE / lookup / Huffman-lookup / RLE-lookup / blur methods; the bit reader; texel → RGBA for all 13 formats). Texture N's ROM range from `src/ge/images_us.rs` (generated from the decomp's `imagelist.u.csv`: 2,699 textures, offsets only). Dam uses 74 textures (27 zlib, 47 Rare); all decode, and a contact sheet shows them right (concrete, hazard stripes, the painted "5344", road markings, grass, wood, water).
- Texture references: GoldenEye reuses G_NOOP (0xC0) as "load texture": number in w1 bits 0-11 (detail texture in 12-23 for type 1), type in w0 & 7, wrap and shift bits in w0 (`tex.c` texLoadFromGdl).
- Display lists: `src/ge/dl.rs` follows G_VTX (0x04, segment 0x0E = room vertex table), G_TRI1 (0xBF, indices × 10), Rare's G_TRI4 (0xB1, nibbles; `include/gbi_extension.h`), textures (0xC0), G_TEXTURE scale (0xBB). Vertices are room-relative, offset by the room table position. Texture scale/shift apply at triangle time.
- `n64kr ge-gltf <ROM> dam <dir>`: Dam = 12,400 triangles, 76 materials (texture × opaque/translucent), 74 textures, GoldenEye units (bbox x −6950..6344, y −8422..10439 in Blender's Z-up after import, height −8455..2811). Renders show the reservoir, curved dam wall with towers, cliffs and bunker; textures map correctly.
- Next: floor map (drivable surfaces only, coloured by height) to trace the route; then the lap layout with the user.

## Step 2: Dam Speedway (user chose oval A) [VERIFIED: tool output, renders, Fast64 export]
- User direction: a giant stretched, banked oval; drop the underground and interiors; pavement from Dam's own assets.
- `n64kr ge-dam-speedway <ROM> <dir>` (`src/track.rs` + `ge_dam_speedway` in `main.rs`): stadium oval over the reservoir — centre (−345, −5062) GE, straights 3,920, turn radius 1,700, road 360 wide, banked 25° in the turns with 1,500-unit smoothstep transitions (inner edge pivots, outside rises); 60-unit walls both sides (Dam's red/white stripes, tex 18, double-sided wall collision); road texture = Dam's asphalt (tex 22, the most flat ground area in the level); invisible OUT_OF_BOUNDS sheet at the water (y −200 GE, found as the height with the most flat area) for Lakitu rescue. Scenery: Dam's rooms above y −1,500 (drops the underground bunker), non-solid (CLIP_NONE). Scale 0.35 → lap 6,482 MK64 units, road 126 wide; start mid east straight racing north (−Z already in GE axes). First fit (radius 1,981) ran the south turn over the dam crest; refitted ~250 units clear of the dam's inner face and the north cliffs.
- Layout JSON: path 562 points along the road centre, 6 item rows (straights and turn apexes), Dam sky colours, `spawn_finishline: true`.
- Pipeline changes: `blender_dkr_track.py` and `dkr_track_scene.py` take a source-game prefix (`ge` → mod `n64kr-ge-dam-speedway`, resource `n64kr:ge_dam_speedway`); the scene script reads `spawn_finishline` from the layout (DKR tracks keep it off) and accepts skies without a DKR dome name. Fast64 only converts textured materials, so untextured geometry gets a white texture and the invisible out-of-bounds sheet borrows the road's.
- Output: Fast64 export OK (489 files; texture loads pass the tile-7 check), scene.json with 18 item boxes, minimap 48×80. In-game test pending.

## Step 3: B variant, hazards, music and sound (user: "go all out on this map, add the B variant as well, exploding barrels and npcs that shoot, along with music and sfx")

### Dam Speedway B — twin Dams [VERIFIED: tool output, top-down render, Fast64 export]
- Reservoir measured on a 100-unit height map of the level (max height per cell): water x −2,100..2,900 at its narrowest (z −6,000), midline near x +200 from z −8,000 to −5,700; dam crest walkway at y −25 (55 above the road) along the arch z −650..−850; the arch's centre reaches z −900.
- B = the Dam level clipped at z −7,400 (water −2,400..2,600 across) and a copy turned 180° about (250, −7,400), so the two halves meet on a seam across open water with the shores within ~150 units. Triangles crossing the seam are cut (`ge::dl::clip`, texture coordinates and colours interpolated).
- Oval: centre (250, −7,400), straights 7,996, radius 2,000, road 360, 25° banking; the south turn's outer edge stays at z −1,222 (as A), so each turn has a dam behind it. Lap 9,995 MK64 units (A: 6,482); 23,140 scenery triangles; 7 item rows.
- A moved 195 units east (cx −150) to stay ≥ 170 inside the water's edge on the west straight (the old line ran within 75 of the shore rock).

### Hazards [VERIFIED: build; in-game test pending]
- Models from the ROM (`n64kr ge-model`, `ge::model`): object model files (1172-compressed; switch pointers and the texture table counts come from the game's ModelFileHeader, recorded per file in `MODEL_FILES`); node tree (group 2, display lists 4/22/24, gunfire 12, head placeholder 23); same Rare F3D as rooms plus G_MTX (segment 3, 64 bytes/matrix) to pick the joint a vertex batch belongs to.
- Guards: Dam's setup gives body 2 = `ColiveguardZ` (olive uniform) with random heads. Bind pose has every joint unrotated (limbs laid out flat), so poses come from GoldenEye's animations (`ge::anim`, from model.c): ModelAnimation table at ROM 2681216 (PTR_ANIM_* offsets), frame bitstreams at 1198784 + address + frame × bits/8; joint j's x/y/z angles are `width`-bit fields at bit mtxA(j) × width (guard skeleton joint table from `assets/embedded/skeletons/guard.inc.c`); joint matrix = parent × (rotation xyz, origin); matrix 1 = half rotation for skin across joints. Renders of `fire_standing` frame 25 show the real aiming pose.
- The KF7 (`PchrkalashZ`) hangs off the right wrist joint. Its barrel lies along −x, magazine along −z; measured hand axes in the aim pose (x along the fingers, y out of the palm) give the mapping gun x,y,z → hand −x,−z,−y (`GUN_IN_HAND`). Renders: stock at the shoulder, magazine down, barrel forward; idle pose holds it low across the body.
- `n64kr ge-hazards <ROM> <mod>` writes libultraship resources (`lus.rs`, a Rust port of the racer model writer) into mod `n64kr-ge-assets`: `ge/barrel` (oil drum with hazard stripes, `Poil_drum1Z`), `ge/guard_{dave,karl,mark,jim}` (5 poses: idle 0, fire_standing 0/8/16/25 = rifle raised to the aim; muzzle and aim yaw in model.json), `ge/fx` (muzzle flash 2119, tracer 2128, fire cloud 2115, starburst 2084 from `assets/images.def` order).
- Engine (`src/n64kr/ge_hazards.cpp`): actor types `n64kr:ge_barrel` / `n64kr:ge_guard`, registered after the stock ones (RegisterActors hook). Barrel: touching it explodes it (fireballs, shock ring, sound), racers within 30 tumble (`VERTICAL_TUMBLE_TRIGGER`, stars immune), drums within 45 go up after 9–15 ticks (chain reaction), back after 10 s. Guard: idle until a racer is within 650; raises the KF7 through the 5 poses, turns to aim (barrel yaw from model.json), fires 3-shot bursts with muzzle flash, tracer and GoldenEye's KF7 sound, 130+ tick cooldown, lowers it when nobody is near. One shot per burst may land (32% near → 10% at range): banana-style spin (`HIT_BANANA_TRIGGER`), then 4 s of immunity; misses whiz/ricochet. Only fires during RACE_IN_PROGRESS.
- Placement (layout `actors`, passed through by `dkr_track_scene.py`): guards on sentry pads (80×80 boxes outside or inside the walls, asphalt top, striped sides, scenery) along both straights and on the dam crest(s) facing the oval; oil drum clusters of 3 across the road (alternating open lane) on straights and turn exits, clear of the start line and item rows. A: 9 guards, 12 drums; B: 18 guards, 24 drums.
- Sizes: guard 16 MK64 units tall, drum 9 (kart ≈ 10 across) [CONCLUSION: to be tuned in game].

### Music and sound [VERIFIED: tool output; decoded audio checked numerically (diff/RMS 0.28–0.51, real audio; gunshot and explosion decay), not yet heard in game]
- `n64audio.rs`: libultra audio rendered offline, ported from the decomp's libultra sources: bank files (bnkf.c layout), VADPCM decoding, compact MIDI sequences (cseq.c: running status, note durations, 0xFE back-references, loop meta events 0x2E/0x2D with counts written back), and a renderer following csplayer.c (keymap lookup, pitch = cents2ratio((key − keyBase)·100 + detune) × bend, attack/decay/release ramps, volume = velocity × sample × channel × track volume, pan law, sustain, pitch bend range, tempo). GoldenEye passes no oscillators (music.c initOsc = NULL), so no vibrato/tremolo. Songs whose tracks all loop forever to the same tick are rendered through the loop twice; the second pass is the loop (seamless, tails included).
- GoldenEye (`ge::audio`): music bank `instruments.ctl/.tbl` (22,050 Hz), songs 1172-compressed (`Mdam` = level music, `Mdamx` = its action version, `Mintro_eye` = the gun-barrel intro, plus Runway and Facility). Sound effects: instrument 0's sound n of `sfx.ctl/.tbl` (snd.c), pitched by cents2ratio(keyBase·100 + detune − 6000), chained layers via Rare's reuse of the keymap fields.
- `n64kr ge-audio <ROM> <mod>`: `music/n64kr/ge_*.wav` + `.json` loop points (Dam: 267 s file, loop 121.7 s), `sound/n64kr/ge/*.wav` (kf7, explosion, explosion2, rico1/2, whiz1/2, hit_metal, alarm, cart).
- Engine: `src/n64kr/music.cpp` registers N64KR song ids (0x4E40 = Dam …) with HMAS from `play_sequence` (external.c hook), so a track's scene.json `"Sequence": 20032` plays GoldenEye's Dam theme, looping. `src/n64kr/sfx.cpp`: polyphonic positional effects (24 voices, distance falloff and pan from the nearest camera), mixed in the audio thread next to HMAS (Engine.cpp hook) — HMAS plays one sound per channel, too few for gunfire over explosions.

### First in-game test (user, 2026-11-02) [VERIFIED: user screenshots + glTF check]
- Both tracks load (scenery, walls, finish banner, racers). But the race started under the road with Lakitu fishing a racer out, and the countdown never began.
- Cause: every road triangle faced down (quad corners ran inner→ahead→outer, clockwise from above; all 1,236 on A, 1,904 on B), while the out-of-bounds sheet faced up. MK64 builds collision from the display lists, and a down-facing triangle is a ceiling, so karts dropped through the road onto the out-of-bounds sheet and the rescue held up the start.
- Fix: road quads wound counter-clockwise from above (`track::build_oval`); the regenerated glTF has all road triangles facing up. Both tracks re-exported. Retest pending.
