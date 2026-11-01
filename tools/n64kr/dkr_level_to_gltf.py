"""Convert a DKR track from the user's own Diddy Kong Racing ROM to glTF 2.0.

The output (model + textures) is ROM data: it goes under build/ and must
never be committed. Open it in Blender, then export it with Fast64 as a
SpaghettiKart custom track (recon/005-content-merge/SLICE-6_First_DKR_Track.md).

Usage (from the repo root):
    python -I tools/n64kr/dkr_level_to_gltf.py "<DKR .z64>" --list
    python -I tools/n64kr/dkr_level_to_gltf.py "<DKR .z64>" ancient_lake build/dkr_tracks

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
    "ASSET_LEVEL_MODELS_TABLE": 26,
    "ASSET_LEVEL_MODELS": 27,
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

def convert(rom, name, out_dir):
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
        textures.append({"id": u32(data, o) & 0x7FFF, "w": data[o + 4], "h": data[o + 5]})

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
    tex3d = rom.section("ASSET_TEXTURES_3D")
    tex3d_table = rom.table("ASSET_TEXTURES_3D_TABLE")
    images, samplers, gl_textures, materials, material_index = [], [], [], [], {}
    texture_slot = {}
    for t, info in enumerate(textures):
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
        texture_slot[t] = (w, h, fmt)

    buffer = bytearray()
    accessors, buffer_views, primitives = [], [], {"visible": [], "hidden": []}
    for key, corners in groups.items():
        tex_index, category, water, no_collision, transparent = key
        if not corners:
            continue
        mat_name = ("dkr_tex%04d" % textures[tex_index]["id"]) if tex_index is not None else "dkr_vertex_colour"
        mat_name += "_water" if water else ""
        mat_name += "_nocollide" if no_collision else ""
        mat_name += "_transparent" if transparent else ""
        mat_name += "_hidden" if category == "hidden" else ""
        if mat_name not in material_index:
            mat = {"name": mat_name, "pbrMetallicRoughness": {"metallicFactor": 0.0, "roughnessFactor": 1.0},
                   "doubleSided": True}
            if tex_index is not None:
                mat["pbrMetallicRoughness"]["baseColorTexture"] = {"index": tex_index}
            if transparent or water:
                mat["alphaMode"] = "BLEND"
            elif tex_index is not None and texture_slot[tex_index][2] in (0, 1, 4, 5, 6):
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


def main():
    args = sys.argv[1:]
    if len(args) >= 2 and args[1] == "--list":
        print("\n".join(LEVEL_MODELS))
        return
    if len(args) != 3:
        sys.exit(__doc__)
    rom = Rom(args[0])
    if len(rom.table("ASSET_LEVEL_MODELS_TABLE")) != len(LEVEL_MODELS):
        sys.exit("Level model table size does not match this ROM.")
    out_dir = os.path.join(args[2], args[1])
    convert(rom, args[1], out_dir)


if __name__ == "__main__":
    main()
