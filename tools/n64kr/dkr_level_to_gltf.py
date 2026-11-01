"""Convert a DKR track from the user's own Diddy Kong Racing ROM to glTF 2.0.

The output (model + textures) is ROM data: it goes under build/ and must
never be committed. Open it in Blender, then export it with Fast64 as a
SpaghettiKart custom track (recon/005-content-merge/SLICE-6_First_DKR_Track.md).

Usage (from the repo root):
    python -I tools/n64kr/dkr_level_to_gltf.py "<DKR .z64>" --list
    python -I tools/n64kr/dkr_level_to_gltf.py "<DKR .z64>" ancient_lake build/dkr_tracks
    python -I tools/n64kr/dkr_level_to_gltf.py "<DKR .z64>" ancient_lake build/dkr_tracks --mk64

--mk64 lays the track out for SpaghettiKart: checkpoint 0 at the origin,
racing towards -Z, scaled to MK64 size, materials tagged with MK64 surface /
collision / draw layer (glTF material extras), and <track>_mk64.json with a
path through DKR's checkpoints, the start grid and item box positions.

Format notes (DKR decomp a5eb080):
- Asset lookup table: u32 count, then one u32 start offset per section,
  relative to the end of the table. Sections used: level models (+ table),
  3D textures (+ table).
- Level models and textures are DKR-compressed: a 5-byte header, then raw
  deflate (src/gzip.c gzip_inflate).
- LevelModel / LevelModelSegment / TriangleBatchInfo / Vertex / Triangle as in
  include/structs.h. Pointers inside a model are offsets from its start
  (src/tracks.c, LOCAL_OFFSET_TO_RAM_ADDRESS).
- Vertices with r == 1 and g == 1 store alpha in b (src/tracks.c, end of
  the level model load).
- UVs are s16 in 1/32 texel units.
"""
import json
import math
import os
import struct
import sys
import zlib

# DKR USA 1.1 (v80): assets offset from the decomp's dkr_assets_tool_extract.json.
SUPPORTED = {
    "6d96743d46f8c0cd0edb0ec5600b003c89b93755": {"name": "us.v80", "assets": 970976, "lut_size": 208},
}
# Section indices in the asset lookup table (position in the "sections" list
# of the decomp's tools/dkr_assets_tool_extract.json).
SECTION_INDEX = {
    "ASSET_TEXTURES_3D": 2,
    "ASSET_TEXTURES_3D_TABLE": 3,
    "ASSET_TEXTURES_2D": 4,
    "ASSET_TEXTURES_2D_TABLE": 5,
    "ASSET_SPRITES": 12,
    "ASSET_SPRITES_TABLE": 13,
    "ASSET_LEVEL_OBJECT_MAPS_TABLE": 20,
    "ASSET_LEVEL_OBJECT_MAPS": 21,
    "ASSET_LEVEL_HEADERS_TABLE": 22,
    "ASSET_LEVEL_HEADERS": 23,
    "ASSET_LEVEL_MODELS_TABLE": 26,
    "ASSET_LEVEL_MODELS": 27,
    "ASSET_OBJECT_MODELS_TABLE": 28,
    "ASSET_OBJECT_MODELS": 29,
    "ASSET_OBJECT_HEADERS_TABLE": 33,
    "ASSET_OBJECTS": 34,
    "ASSET_LEVEL_OBJECT_TRANSLATION_TABLE": 35,
}

# Level model names in v80 table order (decomp asset JSON LevelModel entries,
# minus the v79-only variants). 56 entries, matching the ROM's table.
LEVEL_MODELS = [
    "central_area_hub", "bluey", "dragon_forest_hub", "fossil_canyon", "pirate_lagoon", "ancient_lake",
    "walrus_cove", "hot_top_volcano", "whale_bay", "snowball_valley", "temple_track", "crescent_island",
    "fire_mountain", "dino_domain_hub", "everfrost_peak", "sherbet_island_hub", "front_end", "character_select",
    "spaceport_alpha", "horseshoe_gulch", "greenwood_village", "spacedust_alley", "title_screen",
    "boulder_canyon", "windmill_plains", "snowflake_mountain_hub", "smokey_castle", "darkwater_beach",
    "icicle_pyramid", "frosty_village", "jungle_falls", "treasure_caves", "haunted_woods", "darkmoon_caverns",
    "star_city", "trophy_race", "future_fun_land_hub", "intro_sequence", "smokey", "trickytops",
    "wizpig_mouth_sequence", "rocket_sequence", "wizpig1", "ocean_track", "volcano_track",
    "snow_mountain_track", "party_sequence", "bluey_animation", "wizpig1_animation", "bubbler",
    "bubbler_animation", "wizpig2_animation", "wizpig2", "ending_part1", "ending_part2", "spaceport_alpha_v80",
]

RENDER_SEMI_TRANSPARENT = 1 << 2
RENDER_CUTOUT = 1 << 4
RENDER_CLAMP_X = 1 << 6
RENDER_CLAMP_Y = 1 << 7
RENDER_HIDDEN = 1 << 8
RENDER_NO_COLLISION = 1 << 9
RENDER_WATER = 1 << 13

TEX_FLAG_CLAMP_S = 0x40
TEX_FLAG_CLAMP_T = 0x80
TEX_FLAG_INTERLACED = 0x400

SEGMENT_SIZE = 0x44
BATCH_SIZE = 12
TRIANGLE_SIZE = 16
VERTEX_SIZE = 10


def s16(b, o):
    return struct.unpack_from(">h", b, o)[0]


def u16(b, o):
    return struct.unpack_from(">H", b, o)[0]


def u32(b, o):
    return struct.unpack_from(">I", b, o)[0]


def s32(b, o):
    return struct.unpack_from(">i", b, o)[0]


def dkr_inflate(blob):
    return zlib.decompressobj(-15).decompress(blob[5:])


class Rom:
    def __init__(self, path):
        import hashlib

        self.data = open(path, "rb").read()
        sha = hashlib.sha1(self.data).hexdigest()
        if sha not in SUPPORTED:
            sys.exit(f"Unsupported DKR ROM (SHA-1 {sha}). Supported: USA 1.1 (v80), .z64 byte order.")
        info = SUPPORTED[sha]
        self.lut = info["assets"]
        self.lut_end = self.lut + info["lut_size"]

    def section(self, name):
        n = SECTION_INDEX[name]
        a = u32(self.data, self.lut + 4 * (n + 1))
        b = u32(self.data, self.lut + 4 * (n + 2))
        return self.data[self.lut_end + a:self.lut_end + b]

    def table(self, name):
        raw = self.section(name)
        out = [s32(raw, i) for i in range(0, len(raw) - 3, 4)]
        return out[:out.index(-1)] if -1 in out else out


# ---------------------------------------------------------------- textures

def decode_texture(raw):
    """Returns (width, height, rgba bytes, header flags) for the first frame."""
    width, height, fmt = raw[0], raw[1], raw[2] & 0xF
    flags = u16(raw, 6)
    body = dkr_inflate(raw[0x20:]) if raw[0x1D] else raw[0x20:]
    # Uncompressed: body follows the asset header. Compressed: the inflated
    # data starts with its own header.
    pixels = body[0x20:] if raw[0x1D] else body
    bpp = {0: 32, 1: 16, 2: 8, 3: 4, 4: 16, 5: 8, 6: 4, 7: 4, 8: 8}[fmt]
    row = (width * bpp) // 8
    data = bytearray(pixels[:row * height].ljust(row * height, b"\0"))
    if flags & TEX_FLAG_INTERLACED:
        chunk = 8 if bpp == 32 else 4
        for y in range(1, height, 2):
            o = y * row
            for x in range(0, row - 2 * chunk + 1, 2 * chunk):
                a = data[o + x:o + x + chunk]
                data[o + x:o + x + chunk] = data[o + x + chunk:o + x + 2 * chunk]
                data[o + x + chunk:o + x + 2 * chunk] = a
    out = bytearray(width * height * 4)
    for i in range(width * height):
        if fmt == 0:
            r, g, b, a = data[i * 4:i * 4 + 4]
        elif fmt == 1:
            p = (data[i * 2] << 8) | data[i * 2 + 1]
            r, g, b = ((p >> 11) & 31) * 255 // 31, ((p >> 6) & 31) * 255 // 31, ((p >> 1) & 31) * 255 // 31
            a = 255 if p & 1 else 0
        elif fmt == 2:
            r = g = b = a = data[i]
        elif fmt == 3:
            v = (data[i // 2] >> (4 if i % 2 == 0 else 0)) & 15
            r = g = b = a = v * 17
        elif fmt == 4:
            r = g = b = data[i * 2]
            a = data[i * 2 + 1]
        elif fmt == 5:
            v = data[i]
            r = g = b = (v >> 4) * 17
            a = (v & 15) * 17
        elif fmt == 6:
            v = (data[i // 2] >> (4 if i % 2 == 0 else 0)) & 15
            r = g = b = (v >> 1) * 255 // 7
            a = 255 if v & 1 else 0
        else:
            # CI4 / CI8 not handled yet: magenta so it stands out.
            r, g, b, a = 255, 0, 255, 255
        out[i * 4:i * 4 + 4] = bytes((r, g, b, a))
    return width, height, bytes(out), flags, fmt


def png_bytes(width, height, rgba):
    def chunk(tag, payload):
        c = struct.pack(">I", len(payload)) + tag + payload
        return c + struct.pack(">I", zlib.crc32(tag + payload) & 0xFFFFFFFF)

    rows = b"".join(b"\0" + rgba[y * width * 4:(y + 1) * width * 4] for y in range(height))
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(rows, 9)) + chunk(b"IEND", b""))


# ---------------------------------------------------------------- level model

# ---------------------------------------------------------------- MK64 mode

# DKR SurfaceType (include/enums.h) -> MK64 surface name as Fast64 spells it.
DKR_TO_MK64_SURFACE = {
    0: "ASPHALT",              # SURFACE_DEFAULT (track)
    1: "GRASS",                # SURFACE_GRASS
    2: "SAND_OFFROAD",         # SURFACE_SAND
    3: "BOOST_RAMP_ASPHALT",   # SURFACE_ZIP_PAD
    4: "DIRT_OFFROAD",         # SURFACE_STONE (DKR slows racers on stone)
    10: "ICE",                 # SURFACE_FROZEN_WATER
    11: "WET_SAND",            # SURFACE_WATER_CALM
    13: "SNOW",                # SURFACE_SNOW
    14: "WET_SAND",            # SURFACE_WATER_WAVY
    15: "WET_SAND",            # SURFACE_WATER_UNK_F
}
DKR_SURFACE_INVIS_WALL = 17

# DKR is roughly twice MK64's scale: hover calibration maps DKR top speed
# 12 units/tick to MK64 cruise 5.9 (recon/004-hover/SLICE-5_DKR_Stats.md).
DKR_TO_MK64_SCALE = 0.5
# Path point spacing in MK64 units (Mario Raceway: ~600 points per lap).
PATH_SPACING = 20.0
# MK64 units between DKR's front grid row and the start line (path point 0).
START_LINE_AHEAD = 20.0


BHV_SCENERY = 2  # include/object_behaviors.h
MODEL_TYPE_3D = 0  # ObjectHeader.modelType: 0 = 3D model, others = textures / sprites


def object_headers(rom):
    """Per object header: name, behaviour, model type, scale, model ids
    (ObjectHeader in include/structs.h; pointers are offsets from the header)."""
    objs = rom.section("ASSET_OBJECTS")
    table = rom.table("ASSET_OBJECT_HEADERS_TABLE")
    headers = []
    for i in range(len(table) - 1):
        h = objs[table[i]:table[i + 1]]
        if len(h) < 0x70:
            headers.append({"name": "", "behaviour": -1, "model_type": -1, "scale": 1.0, "models": []})
            continue
        count = struct.unpack_from("b", h, 0x55)[0]
        ids_off = u32(h, 0x10)
        models = [s32(h, ids_off + 4 * k) for k in range(max(0, count))] if 0 < ids_off < len(h) else []
        headers.append({"name": h[0x60:0x70].split(bytes(1))[0].decode("ascii", "replace"),
                        "behaviour": struct.unpack_from("b", h, 0x54)[0], "model_type": struct.unpack_from("b", h, 0x53)[0],
                        "scale": struct.unpack_from(">f", h, 0x0C)[0], "models": models})
    return headers


def level_objects(rom, model_index):
    """Checkpoints, start grid, item balloons and zippers for the first race
    level whose header uses this model (src/objects.c track_spawn_objects)."""
    headers = rom.section("ASSET_LEVEL_HEADERS")
    htable = rom.table("ASSET_LEVEL_HEADERS_TABLE")
    header = None
    for i in range(len(htable) - 1):
        h = headers[htable[i]:htable[i + 1]]
        if len(h) >= 0xBC and s16(h, 0x34) == model_index and h[0x4B] > 0:
            header = h
            break
    if header is None:
        sys.exit("No race level header uses this model.")
    map_ids = (s16(header, 0xBA), s16(header, 0x36))  # main objects, collectables
    sky_object = s16(header, 0x38)  # object header index of the sky dome (src/tracks.c skydome_spawn)

    trans_raw = rom.section("ASSET_LEVEL_OBJECT_TRANSLATION_TABLE")
    trans = [s16(trans_raw, i) for i in range(0, len(trans_raw) - 1, 2)]
    headers = object_headers(rom)
    names = [hd["name"] for hd in headers]
    maps = rom.section("ASSET_LEVEL_OBJECT_MAPS")
    mtable = rom.table("ASSET_LEVEL_OBJECT_MAPS_TABLE")
    found = {"checkpoint": [], "setuppoint": [], "WeaponBalloon": [], "GroundZipper": [], "scenery": []}
    for map_id in map_ids:
        data = dkr_inflate(maps[mtable[map_id]:mtable[map_id + 1]])
        # u32 byte size, then entries from offset 16 (gObjectMap is s32 *,
        # spawn list starts at gObjectMap + 4).
        size, pos = u32(data, 0), 16
        while pos < 16 + size:
            length = data[pos + 1] & 0x3F
            if length == 0:
                break
            object_id = data[pos] | ((data[pos + 1] & 0x80) << 1)
            header_index = trans[object_id] if object_id < len(trans) else -1
            name = names[header_index] if 0 <= header_index < len(names) else ""
            hd = headers[header_index] if 0 <= header_index < len(headers) else None
            if hd and hd["behaviour"] == BHV_SCENERY:
                # LevelObjectEntry_Scenery; placement as obj_init_scenery (src/object_functions.c)
                model_index = data[pos + 8]
                if model_index >= len(hd["models"]):
                    model_index = 0
                radius = max(data[pos + 9], 10) / 64.0
                found["scenery"].append({"name": name, "x": s16(data, pos + 2), "y": s16(data, pos + 4),
                                         "z": s16(data, pos + 6), "scale": hd["scale"] * radius,
                                         "angle": data[pos + 0xA] << 8, "solid": data[pos + 0xB],
                                         "model_type": hd["model_type"],
                                         "model": hd["models"][model_index] if hd["models"] else -1})
            if name in found:
                entry = {"x": s16(data, pos + 2), "y": s16(data, pos + 4), "z": s16(data, pos + 6)}
                if name == "checkpoint":
                    # LevelObjectEntry_Checkpoint (include/level_object_entries.h)
                    entry.update(index=data[pos + 9], alt=data[pos + 0x17], vehicle=struct.unpack_from("b", data, pos + 0x1A)[0])
                elif name == "setuppoint":
                    entry.update(racer=data[pos + 8], angle=data[pos + 0xA] * 256)
                found[name].append(entry)
            pos += length
    found["sky_object"] = sky_object
    return found


def sky_colours(rom, sky_object):
    """Top and horizon colour of the level's sky dome: vertex colour x mean
    texture colour, averaged over the top 20% and bottom 15% of the dome."""
    headers = object_headers(rom)
    if not (0 <= sky_object < len(headers)) or not headers[sky_object]["models"]:
        return None
    table = rom.table("ASSET_OBJECT_MODELS_TABLE")
    model_id = headers[sky_object]["models"][0]
    m = dkr_inflate(rom.section("ASSET_OBJECT_MODELS")[table[model_id]:table[model_id + 1]])
    tex3d, ttable = rom.section("ASSET_TEXTURES_3D"), rom.table("ASSET_TEXTURES_3D_TABLE")
    tex_off, v_off, t_off, b_off = u32(m, 0x00), u32(m, 0x04), u32(m, 0x08), u32(m, 0x38)
    means = []
    for t in range(s16(m, 0x22)):
        tid = u32(m, tex_off + t * 8) & 0x7FFF
        w, h, rgba, _, _ = decode_texture(tex3d[ttable[tid]:ttable[tid + 1]])
        means.append([sum(rgba[i * 4 + k] for i in range(w * h)) / (w * h) / 255.0 for k in range(3)])
    samples = []
    for b in range(s16(m, 0x28)):
        bo = b_off + b * BATCH_SIZE
        ti = m[bo]
        mean = means[ti] if ti < len(means) else [1.0, 1.0, 1.0]
        for f in range(s16(m, bo + 4), s16(m, bo + BATCH_SIZE + 4)):
            tr = t_off + f * TRIANGLE_SIZE
            for c in range(3):
                vo = v_off + (s16(m, bo + 2) + m[tr + 1 + c]) * VERTEX_SIZE
                samples.append((s16(m, vo + 2), [min(255.0, m[vo + 6 + k] * mean[k]) for k in range(3)]))
    lo, hi = min(sp[0] for sp in samples), max(sp[0] for sp in samples)

    def band(a, b):
        sel = [c for y, c in samples if lo + a * (hi - lo) <= y <= lo + b * (hi - lo)]
        return [int(sum(c[k] for c in sel) / len(sel)) for k in range(3)]

    return {"top": band(0.8, 1.0), "horizon": band(0.0, 0.15), "dome": headers[sky_object]["name"]}


MODEL_TYPE_BILLBOARD = 1
# A texture must fit N64 TMEM (4 KB = 2048 RGBA16 texels) at power-of-two
# sizes, or Fast64 falls back to multi-tile loads that libultraship rejects
# (SUPPORT_CHECK(tile == G_TX_LOADTILE) in GfxDpLoadTile).
TMEM_TEXELS = 2048


def fit_tmem(width, height, rgba):
    """Nearest-neighbour resample to the power-of-two size that fits TMEM
    with the closest aspect ratio."""
    sizes = [(1 << a, 1 << b) for a in range(3, 8) for b in range(3, 8) if (1 << a) * (1 << b) <= TMEM_TEXELS]
    target = width / height
    tw, th = min(sizes, key=lambda wh: (abs(math.log((wh[0] / wh[1]) / target)), -wh[0] * wh[1]))
    out = bytearray(tw * th * 4)
    for y in range(th):
        sy = min(height - 1, y * height // th)
        for x in range(tw):
            sx = min(width - 1, x * width // tw)
            out[(y * tw + x) * 4:(y * tw + x) * 4 + 4] = rgba[(sy * width + sx) * 4:(sy * width + sx) * 4 + 4]
    return tw, th, bytes(out)


def sprite_image(rom, sprite_id):
    """Frame 0 of a DKR sprite assembled from its 2D texture pieces.
    Returns (width, height, rgba, left, top) where (left, top) is the image's
    top-left corner relative to the sprite anchor, in pixels with Y up.
    Piece placement as src/textures_sprites.c: x = posX - anchor.x,
    y = anchor.y - posY (SpriteAsset in include/structs.h)."""
    sprites, stable = rom.section("ASSET_SPRITES"), rom.table("ASSET_SPRITES_TABLE")
    tex2d, ttable = rom.section("ASSET_TEXTURES_2D"), rom.table("ASSET_TEXTURES_2D_TABLE")
    a = sprites[stable[sprite_id]:stable[sprite_id + 1]]
    base, frames = s16(a, 0), s16(a, 2)
    anchor_x, anchor_y = s16(a, 4), s16(a, 6)
    first, last = a[0x0C], a[0x0C + 1] if frames >= 1 else a[0x0C]
    pieces = []
    for tid in range(base + first, base + last):
        raw = tex2d[ttable[tid]:ttable[tid + 1]]
        w, h, rgba, _, _ = decode_texture(raw)
        pos_x, pos_y = struct.unpack_from("bb", raw, 3)
        pieces.append((pos_x - anchor_x, anchor_y - pos_y, w, h, rgba))
    left = min(p[0] for p in pieces)
    right = max(p[0] + p[2] for p in pieces)
    top = max(p[1] for p in pieces)
    bottom = min(p[1] - p[3] for p in pieces)
    width, height = right - left, top - bottom
    out = bytearray(width * height * 4)
    for px, py, w, h, rgba in pieces:
        ox, oy = px - left, top - py
        for y in range(h):
            row = rgba[y * w * 4:(y + 1) * w * 4]
            out[((oy + y) * width + ox) * 4:((oy + y) * width + ox + w) * 4] = row
    return width, height, bytes(out), left, top


def scenery_groups(rom, scenery, textures, groups):
    """Bake 3D scenery objects into the level as decoration (no collision yet).
    Object model layout as src/object_models.c object_model_init."""
    import math

    models = rom.section("ASSET_OBJECT_MODELS")
    table = rom.table("ASSET_OBJECT_MODELS_TABLE")
    tex_slot = {t["id"]: i for i, t in enumerate(textures)}
    cache, placed, skipped = {}, 0, {}
    sprite_slot = {}
    for obj in scenery:
        if obj["model_type"] == MODEL_TYPE_BILLBOARD and obj["model"] >= 0:
            # Billboards become two crossed vertical quads (both sides) with
            # the sprite's frame 0 as the texture.
            if obj["model"] not in sprite_slot:
                w, h, rgba, left, top = sprite_image(rom, obj["model"])
                tw, th, trgba = fit_tmem(w, h, rgba)
                sprite_slot[obj["model"]] = (len(textures), w, h, left, top, tw, th)
                textures.append({"id": 0x10000 + obj["model"], "w": tw, "h": th, "surface": 0,
                                 "image": (tw, th, trgba), "file": f"dkr_sprite{obj['model']:04d}"})
            slot, w, h, left, top, tw, th = sprite_slot[obj["model"]]
            key = (slot, "visible", False, True, True)
            corners = groups.setdefault(key, [])
            sc = obj["scale"]
            x0, x1, y0, y1 = left * sc, (left + w) * sc, (top - h) * sc, top * sc
            for dx, dz in ((1.0, 0.0), (0.0, 1.0)):
                quad = [(x0, y1, 0, 0), (x1, y1, tw, 0), (x1, y0, tw, th), (x0, y0, 0, th)]
                pts = [(obj["x"] + qx * dx, obj["y"] + qy, obj["z"] + qx * dz, u * 32, v * 32) for qx, qy, u, v in quad]
                for tri in ((0, 1, 2), (0, 2, 3), (2, 1, 0), (3, 2, 0)):
                    for i in tri:
                        x, y, z, u, v = pts[i]
                        corners.append((x, y, z, 0xFF, 0xFF, 0xFF, 0xFF, u, v))
            placed += 1
            continue
        if obj["model_type"] != MODEL_TYPE_3D or not (0 <= obj["model"] < len(table) - 1):
            skipped[obj["name"]] = skipped.get(obj["name"], 0) + 1
            continue
        if obj["model"] not in cache:
            cache[obj["model"]] = dkr_inflate(models[table[obj["model"]]:table[obj["model"] + 1]])
        m = cache[obj["model"]]
        tex_off, v_off, t_off, b_off = u32(m, 0x00), u32(m, 0x04), u32(m, 0x08), u32(m, 0x38)
        num_tex, num_batches = s16(m, 0x22), s16(m, 0x28)
        local_tex = []
        for t in range(num_tex):
            o = tex_off + t * 8
            tid = u32(m, o) & 0x7FFF
            if tid not in tex_slot:
                tex_slot[tid] = len(textures)
                textures.append({"id": tid, "w": m[o + 4], "h": m[o + 5], "surface": 0})
            local_tex.append(tex_slot[tid])
        a = obj["angle"] / 65536.0 * 2.0 * math.pi
        ca, sa, sc = math.cos(a), math.sin(a), obj["scale"]
        for b in range(num_batches):
            bo = b_off + b * BATCH_SIZE
            tex_index = m[bo]
            v_base = s16(m, bo + 2)
            f_start, f_end = s16(m, bo + 4), s16(m, bo + BATCH_SIZE + 4)
            flags = u32(m, bo + 8)
            if flags & RENDER_HIDDEN:
                continue
            vertex_colours = m[bo + 6] == 0xFF  # BATCH_VTX_COL; otherwise lit by normals
            key = (None if tex_index == 0xFF or tex_index >= len(local_tex) else local_tex[tex_index],
                   "visible", False, True, bool(flags & (RENDER_SEMI_TRANSPARENT | RENDER_CUTOUT)))
            corners = groups.setdefault(key, [])
            for f in range(f_start, f_end):
                tr = t_off + f * TRIANGLE_SIZE
                for c in range(3):
                    vo = v_off + (v_base + m[tr + 1 + c]) * VERTEX_SIZE
                    lx, ly, lz = s16(m, vo) * sc, s16(m, vo + 2) * sc, s16(m, vo + 4) * sc
                    x = obj["x"] + lx * ca + lz * sa
                    z = obj["z"] - lx * sa + lz * ca
                    r, g, bl, al = m[vo + 6:vo + 10] if vertex_colours else (0xC8, 0xC8, 0xC8, 0xFF)
                    u, v = s16(m, tr + 4 + c * 4), s16(m, tr + 6 + c * 4)
                    corners.append((x, obj["y"] + ly, z, r, g, bl, al, u, v))
        placed += 1
    print(f"Scenery: {placed} objects placed" + (f"; skipped (sprites / no model): {skipped}" if skipped else ""))


def triangle_floor(tris, x, z, y_above):
    """Highest triangle surface at (x, z) below y_above, or None."""
    best = None
    for (ax, ay, az), (bx, by, bz), (cx, cy, cz) in tris:
        d = (bz - cz) * (ax - cx) + (cx - bx) * (az - cz)
        if abs(d) < 1e-9:
            continue
        w0 = ((bz - cz) * (x - cx) + (cx - bx) * (z - cz)) / d
        w1 = ((cz - az) * (x - cx) + (ax - cx) * (z - cz)) / d
        w2 = 1 - w0 - w1
        if w0 < -1e-6 or w1 < -1e-6 or w2 < -1e-6:
            continue
        y = w0 * ay + w1 * by + w2 * cy
        if y <= y_above and (best is None or y > best):
            best = y
    return best


def catmull_rom_loop(points, spacing):
    out = []
    n = len(points)
    for i in range(n):
        p0, p1, p2, p3 = points[i - 1], points[i], points[(i + 1) % n], points[(i + 2) % n]
        seg = sum((p2[k] - p1[k]) ** 2 for k in range(3)) ** 0.5
        steps = max(1, int(seg / spacing))
        for s in range(steps):
            t = s / steps
            t2, t3 = t * t, t * t * t
            out.append(tuple(0.5 * (2 * p1[k] + (-p0[k] + p2[k]) * t + (2 * p0[k] - 5 * p1[k] + 4 * p2[k] - p3[k]) * t2
                                    + (-p0[k] + 3 * p1[k] - 3 * p2[k] + p3[k]) * t3) for k in range(3)))
    return out


def mk64_layout(objects):
    """Transform that puts the start line at the origin, racing towards -Z
    (MK64 grids karts behind path point 0 at +Z: src/spawn_players.c
    spawn_and_set_player_spawns), scaled to MK64 size. Returns the transform
    and the path gates in DKR coordinates: start line, then the checkpoints."""
    import math

    cps = [c for c in objects["checkpoint"] if c["alt"] == 0]
    by_index = {}
    for c in sorted(cps, key=lambda c: c["vehicle"] != 0):
        by_index.setdefault(c["index"], c)
    ordered = [by_index[i] for i in sorted(by_index)]
    grid = sorted(objects["setuppoint"], key=lambda e: e["racer"])
    if len(ordered) < 3 or len(grid) < 2:
        sys.exit("Not enough checkpoints / start positions to build a path.")
    # Start line: just ahead of the front grid row (racers 0 and 1), towards checkpoint 0.
    front = {k: (grid[0][k] + grid[1][k]) / 2.0 for k in ("x", "y", "z")}
    fx, fz = ordered[0]["x"] - front["x"], ordered[0]["z"] - front["z"]
    length = math.hypot(fx, fz)
    ahead = (START_LINE_AHEAD / DKR_TO_MK64_SCALE) / length
    start = {"x": front["x"] + fx * ahead, "y": front["y"], "z": front["z"] + fz * ahead}
    angle = math.atan2(-fx, -fz)
    ca, sa = math.cos(angle), math.sin(angle)

    def transform(x, y, z):
        x, y, z = x - start["x"], y - start["y"], z - start["z"]
        return ((x * ca - z * sa) * DKR_TO_MK64_SCALE, y * DKR_TO_MK64_SCALE, (x * sa + z * ca) * DKR_TO_MK64_SCALE)

    return transform, [start] + ordered


def write_mk64_layout(out_dir, name, transform, ordered, objects, floor_tris, sky):
    gates = [transform(c["x"], c["y"], c["z"]) for c in ordered]
    path = []
    for x, y, z in catmull_rom_loop(gates, PATH_SPACING):
        floor = triangle_floor(floor_tris, x, z, y + 40.0)
        path.append([round(x, 2), round(floor if floor is not None else y, 2), round(z, 2)])
    # Path point 0 at the origin exactly (SpaghettiKart docs/tracks/trackoverview.md).
    oy = path[0][1]
    path = [[p[0], round(p[1] - oy, 2), p[2]] for p in path]
    floor_offset = -oy

    def place(entries):
        out = []
        for e in entries:
            x, y, z = transform(e["x"], e["y"], e["z"])
            out.append([round(x, 2), round(y + floor_offset, 2), round(z, 2)])
        return out

    layout = {
        "track": name,
        "units": "MK64 (N64 coordinates, Y up). Path point 0 at origin, racing towards -Z.",
        "scale_from_dkr": DKR_TO_MK64_SCALE,
        "y_offset": floor_offset,
        "path": path,
        "checkpoints": place(ordered[1:]),
        "start_grid": place(sorted(objects["setuppoint"], key=lambda e: e["racer"])),
        "item_boxes": place(objects["WeaponBalloon"]),
        "zippers": place(objects["GroundZipper"]),
        "sky": sky,
    }
    with open(os.path.join(out_dir, f"{name}_mk64.json"), "w") as f:
        json.dump(layout, f, indent=1)
    grid_z = [p[2] for p in layout["start_grid"]]
    print(f"MK64 layout: {len(path)} path points, {len(ordered) - 1} checkpoints, start grid z {min(grid_z):.0f}..{max(grid_z):.0f} "
          f"(should be > 0, behind the line), {len(layout['item_boxes'])} item boxes.")
    return floor_offset


def write_gltf(rom, out_dir, name, groups, textures):
    """Write groups of triangle corners as glTF 2.0 + PNG textures.
    groups: {(texture index or None, "visible"/"hidden", water, no_collision,
    transparent): [(x, y, z, r, g, b, a, u, v), ...]}; textures: list of
    {"id", "w", "h", "surface"} (3D texture ids) or {"image": (w, h, rgba), "file"}."""
    os.makedirs(os.path.join(out_dir, "textures"), exist_ok=True)
    tex3d = rom.section("ASSET_TEXTURES_3D")
    tex3d_table = rom.table("ASSET_TEXTURES_3D_TABLE")
    images, samplers, gl_textures, materials, material_index = [], [], [], [], {}
    texture_slot = {}
    for t, info in enumerate(textures):
        if "image" in info:
            (w, h, rgba), tflags, fmt = info["image"], 0, 0
            file = f"textures/{info['file']}.png"
        else:
            raw = tex3d[tex3d_table[info["id"]]:tex3d_table[info["id"] + 1]]
            w, h, rgba, tflags, fmt = decode_texture(raw)
            file = f"textures/dkr_tex{info['id']:04d}.png"
        with open(os.path.join(out_dir, file), "wb") as f:
            f.write(png_bytes(w, h, rgba))
        images.append({"uri": file})
        samplers.append({"magFilter": 9729, "minFilter": 9729,
                         "wrapS": 33071 if tflags & TEX_FLAG_CLAMP_S else 10497,
                         "wrapT": 33071 if tflags & TEX_FLAG_CLAMP_T else 10497})
        gl_textures.append({"source": t, "sampler": t})
        alphas = rgba[3::4]
        if all(a == 255 for a in alphas):
            alpha_mode = "opaque"
        elif all(a in (0, 255) for a in alphas) or tflags & 0x10:
            alpha_mode = "cutout"  # 1-bit alpha (texture flag 0x10 = cutout)
        else:
            alpha_mode = "blend"
        texture_slot[t] = (w, h, fmt, alpha_mode)

    buffer = bytearray()
    accessors, buffer_views, primitives = [], [], {"visible": [], "hidden": []}
    for key, corners in groups.items():
        tex_index, category, water, no_collision, transparent = key
        if not corners:
            continue
        if tex_index is None:
            mat_name = "dkr_vertex_colour"
        elif "file" in textures[tex_index]:
            mat_name = textures[tex_index]["file"]
        else:
            mat_name = "dkr_tex%04d" % textures[tex_index]["id"]
        mat_name += "_water" if water else ""
        mat_name += "_nocollide" if no_collision else ""
        mat_name += "_transparent" if transparent else ""
        mat_name += "_hidden" if category == "hidden" else ""
        if mat_name not in material_index:
            mat = {"name": mat_name, "pbrMetallicRoughness": {"metallicFactor": 0.0, "roughnessFactor": 1.0},
                   "doubleSided": True}
            if tex_index is not None:
                mat["pbrMetallicRoughness"]["baseColorTexture"] = {"index": tex_index}
            surface = textures[tex_index]["surface"] if tex_index is not None else 0
            if category == "hidden" or surface == DKR_SURFACE_INVIS_WALL:
                mk64_extras = {"mk64_surface": "SURFACE_DEFAULT", "mk64_clip": "CLIP_DOUBLE_SIDED_WALL",
                               "mk64_draw": "DRAW_INVISIBLE"}
            else:
                # DKR marks transparency on the texture (alpha / header flags), not
                # only on the batch, so classify from the decoded texture.
                alpha_mode = texture_slot[tex_index][3] if tex_index is not None else "opaque"
                if transparent or water:
                    alpha_mode = "blend" if alpha_mode != "cutout" or water else alpha_mode
                mk64_extras = {"mk64_surface": DKR_TO_MK64_SURFACE.get(surface, "ASPHALT"),
                               "mk64_clip": "CLIP_NONE" if no_collision else "CLIP_DEFAULT",
                               "mk64_draw": "DRAW_TRANSLUCENT" if alpha_mode == "blend" else "DRAW_OPAQUE",
                               "mk64_alpha": alpha_mode}
            mat["extras"] = dict(mk64_extras, dkr_surface=surface)
            if mk64_extras.get("mk64_alpha") == "blend":
                mat["alphaMode"] = "BLEND"
            elif mk64_extras.get("mk64_alpha") == "cutout":
                mat["alphaMode"] = "MASK"
            material_index[mat_name] = len(materials)
            materials.append(mat)
        w, h = (texture_slot[tex_index][0], texture_slot[tex_index][1]) if tex_index is not None else (32, 32)

        pos = bytearray()
        col = bytearray()
        uv = bytearray()
        mins = [1e9] * 3
        maxs = [-1e9] * 3
        for x, y, z, r, g, bl, a, u, v in corners:
            pos += struct.pack("<3f", x, y, z)
            col += struct.pack("<4B", r, g, bl, a)
            uv += struct.pack("<2f", u / 32.0 / w, v / 32.0 / h)
            for i, val in enumerate((x, y, z)):
                mins[i] = min(mins[i], val)
                maxs[i] = max(maxs[i], val)
        count = len(corners)
        attrs = {}
        for blob, kind, ctype, extra, attr in ((pos, "VEC3", 5126, {"min": mins, "max": maxs}, "POSITION"),
                                               (col, "VEC4", 5121, {"normalized": True}, "COLOR_0"),
                                               (uv, "VEC2", 5126, {}, "TEXCOORD_0")):
            while len(buffer) % 4:
                buffer += b"\0"
            buffer_views.append({"buffer": 0, "byteOffset": len(buffer), "byteLength": len(blob)})
            buffer += blob
            acc = {"bufferView": len(buffer_views) - 1, "componentType": ctype, "count": count, "type": kind}
            acc.update(extra)
            accessors.append(acc)
            attrs[attr] = len(accessors) - 1
        primitives[category].append({"attributes": attrs, "material": material_index[mat_name], "mode": 4})

    meshes, nodes = [], []
    for category, node_name in (("visible", f"{name}_visible"), ("hidden", f"{name}_invisible_walls")):
        if primitives[category]:
            meshes.append({"name": node_name, "primitives": primitives[category]})
            nodes.append({"name": node_name, "mesh": len(meshes) - 1})

    bin_name = f"{name}.bin"
    with open(os.path.join(out_dir, bin_name), "wb") as f:
        f.write(buffer)
    gltf = {
        "asset": {"version": "2.0", "generator": "N64KR dkr_level_to_gltf.py"},
        "scene": 0,
        "scenes": [{"name": name, "nodes": list(range(len(nodes)))}],
        "nodes": nodes, "meshes": meshes, "materials": materials,
        "textures": gl_textures, "images": images, "samplers": samplers,
        "accessors": accessors, "bufferViews": buffer_views,
        "buffers": [{"uri": bin_name, "byteLength": len(buffer)}],
    }
    with open(os.path.join(out_dir, f"{name}.gltf"), "w") as f:
        json.dump(gltf, f, indent=1)
    tris = sum(len(c) for c in groups.values()) // 3
    hidden = sum(len(c) for k, c in groups.items() if k[1] == "hidden") // 3
    print(f"Wrote {os.path.join(out_dir, name + '.gltf')}: {tris} triangles ({hidden} invisible walls), "
          f"{len(materials)} materials, {len(images)} textures.")


def convert(rom, name, out_dir, mk64=False):
    if name not in LEVEL_MODELS:
        sys.exit(f"Unknown track '{name}'. Use --list.")
    models = rom.section("ASSET_LEVEL_MODELS")
    table = rom.table("ASSET_LEVEL_MODELS_TABLE")
    index = LEVEL_MODELS.index(name)
    data = dkr_inflate(models[table[index]:table[index + 1]])

    tex_off, seg_off = u32(data, 0x00), u32(data, 0x04)
    num_textures, num_segments = s16(data, 0x18), s16(data, 0x1A)
    textures = []
    for t in range(num_textures):
        o = tex_off + t * 8
        textures.append({"id": u32(data, o) & 0x7FFF, "w": data[o + 4], "h": data[o + 5],
                         "surface": struct.unpack_from("b", data, o + 7)[0]})

    # (texture index or None, flags category) -> list of corner tuples
    groups = {}
    for k in range(num_segments):
        so = seg_off + k * SEGMENT_SIZE
        v_off, t_off, b_off = u32(data, so), u32(data, so + 4), u32(data, so + 0xC)
        num_batches = s16(data, so + 0x20)
        for b in range(num_batches):
            bo = b_off + b * BATCH_SIZE
            tex_index = data[bo]
            v_base = s16(data, bo + 2)
            f_start, f_end = s16(data, bo + 4), s16(data, bo + BATCH_SIZE + 4)
            flags = u32(data, bo + 8)
            hidden = bool(flags & RENDER_HIDDEN)
            key = (None if tex_index == 0xFF else tex_index,
                   "hidden" if hidden else "visible",
                   bool(flags & RENDER_WATER), bool(flags & RENDER_NO_COLLISION),
                   bool(flags & (RENDER_SEMI_TRANSPARENT | RENDER_CUTOUT)))
            corners = groups.setdefault(key, [])
            for f in range(f_start, f_end):
                tr = t_off + f * TRIANGLE_SIZE
                vis = (data[tr + 1], data[tr + 2], data[tr + 3])
                for c in range(3):
                    vo = v_off + (v_base + vis[c]) * VERTEX_SIZE
                    x, y, z = s16(data, vo), s16(data, vo + 2), s16(data, vo + 4)
                    r, g, bl, a = data[vo + 6:vo + 10]
                    if r == 1 and g == 1:
                        a, r, g, bl = bl, 0x80, 0x80, 0x80
                    u, v = s16(data, tr + 4 + c * 4), s16(data, tr + 6 + c * 4)
                    corners.append((x, y, z, r, g, bl, a, u, v))

    os.makedirs(os.path.join(out_dir, "textures"), exist_ok=True)
    if mk64:
        objects = level_objects(rom, index)
        scenery_groups(rom, objects["scenery"], textures, groups)
        transform, ordered = mk64_layout(objects)
        for key, corners in groups.items():
            groups[key] = [transform(c[0], c[1], c[2]) + tuple(c[3:]) for c in corners]
        floor_tris = []
        for key, corners in groups.items():
            if key[1] == "visible" and not key[3]:
                floor_tris += [(corners[i][:3], corners[i + 1][:3], corners[i + 2][:3]) for i in range(0, len(corners), 3)]
        sky = sky_colours(rom, objects["sky_object"])
        y_offset = write_mk64_layout(out_dir, name, transform, ordered, objects, floor_tris, sky)
        for key, corners in groups.items():
            groups[key] = [(c[0], c[1] + y_offset, c[2]) + tuple(c[3:]) for c in corners]
    write_gltf(rom, out_dir, name, groups, textures)


def main():
    args = sys.argv[1:]
    mk64 = "--mk64" in args
    args = [a for a in args if a != "--mk64"]
    if len(args) >= 2 and args[1] == "--list":
        print("\n".join(LEVEL_MODELS))
        return
    if len(args) != 3:
        sys.exit(__doc__)
    rom = Rom(args[0])
    if len(rom.table("ASSET_LEVEL_MODELS_TABLE")) != len(LEVEL_MODELS):
        sys.exit("Level model table size does not match this ROM.")
    out_dir = os.path.join(args[2], args[1] + ("_mk64" if mk64 else ""))
    convert(rom, args[1], out_dir, mk64)


if __name__ == "__main__":
    main()
