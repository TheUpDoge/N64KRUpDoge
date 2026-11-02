# RECON-008 — GoldenEye Dam as a race track

Project: N64 Kart Racing (N64KR)
Date: 2026-11-01
Status: RECON + first extraction step (VERIFIED below)

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
