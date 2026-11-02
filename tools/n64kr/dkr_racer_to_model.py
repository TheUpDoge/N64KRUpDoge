"""Export a DKR racer as an in-game 3D model for N64KR's 3D racers.

Usage (from the repo root):
    python -I tools/n64kr/dkr_racer_to_model.py "<DKR .z64>" diddycar diddy build/x64/mods/n64kr-dkr-diddy

Writes libultraship resources into <mod dir>/racers/<name>/ (ROM data: keep
under build/, never commit):
  tex_<n>               RGBA16 texture resources (DKR textures, wheel faces)
  mat_<n>               material display lists (combiner, render mode, texture load)
  pose<p>_<group>_vtx   vertex buffers, one per driving pose and material group
  pose<p>               one display list per pose: body + all four wheels
  model.json            pose frames, scale, ground offset (for reference)

Model space: DKR model units, Y up, turned to face +Z (DKR models face -Z):
(x, y, z) -> (-x, y - ground, -z), so the tyres' bottom is y = 0. The engine
(src/n64kr/racer3d.c) applies the world scale, position and the kart's yaw.

Poses are frames of DKR's driving animation (animation 0, frame 40 straight,
0 full right, 73 full left; src/racer.c), baked per pose because the engine
draws display lists, not vertex animation.

Formats (read from the track export, tools/n64kr/blender_dkr_track.py output):
DisplayList / Vertex XML resources, texture resources = 0x40-byte header
("OTEX", version 0) + u32 type (2 = RGBA16), width, height, size, data
(big-endian RGBA5551).
"""
import json
import math
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))  # python -I drops the script dir
import dkr_level_to_gltf as dkr  # noqa: E402
import dkr_racer_to_gltf as racers  # noqa: E402

POSE_FRAMES = [0, 10, 20, 30, 40, 48, 56, 64, 73]  # full right .. straight .. full left
# World units per DKR model unit: the MK64 sprite quad is 18 x 18 units for a
# 64 x 64 frame (src/data/some_data.c gPlayerOneVtx) and the sprite fit put
# Diddy at ~15 units tall (SLICE-9). Applied by the engine, recorded here.
WORLD_SCALE = 0.12
VERTS_PER_LOAD = 30
WHEEL_SIDES = 10
# DKR draws wheels as flat camera-facing sprites; as solid cylinders at the
# sprite's size they read too big (user playtest), so scale them down.
WHEEL_RADIUS_SCALE = 0.75
WHEEL_WIDTH_SCALE = 0.6
HEADER = (b"\0\0\0\0" + b"XETO" + b"\0\0\0\0" + bytes.fromhex("efbeaddeefbeadde")).ljust(0x40, b"\0")


def rgba5551(rgba):
    out = bytearray()
    for i in range(0, len(rgba), 4):
        r, g, b, a = rgba[i:i + 4]
        out += struct.pack(">H", ((r >> 3) << 11) | ((g >> 3) << 6) | ((b >> 3) << 1) | (1 if a >= 128 else 0))
    return bytes(out)


def pow2_fit(w, h, rgba):
    if (w & (w - 1)) == 0 and (h & (h - 1)) == 0 and w * h <= dkr.TMEM_TEXELS and w >= 8 and h >= 8:
        return w, h, rgba
    return dkr.fit_tmem(w, h, rgba)


class Writer:
    def __init__(self, out_dir, res_prefix):
        self.out_dir, self.prefix = out_dir, res_prefix
        os.makedirs(out_dir, exist_ok=True)
        self.materials = {}  # key -> material resource name

    def res(self, name):
        return f"{self.prefix}/{name}"

    def write(self, name, data):
        with open(os.path.join(self.out_dir, name), "wb" if isinstance(data, bytes) else "w", newline=None if isinstance(data, bytes) else "\n") as fh:
            fh.write(data)

    def texture(self, name, w, h, rgba):
        w, h, rgba = pow2_fit(w, h, rgba)
        data = rgba5551(rgba)
        self.write(name, HEADER + struct.pack("<IIII", 2, w, h, len(data)) + data)
        return w, h

    def material(self, key, tex_name=None, w=0, h=0, cutout=False):
        if key in self.materials:
            return self.materials[key]
        name = f"mat_{len(self.materials)}"
        lines = ['<DisplayList Version="0">',
                 '\t<PipeSync/>',
                 '\t<SetGeometryMode G_SHADE="1" G_SHADING_SMOOTH="1" G_ZBUFFER="1"/>',
                 '\t<ClearGeometryMode G_LIGHTING="1" G_FOG="1" G_CULL_BACK="1" G_CULL_FRONT="1" G_TEXTURE_GEN="1"/>',
                 '\t<SetCycleType G_CYC_1CYCLE="1"/>',
                 '\t<SetTextureFilter Mode="G_TF_BILERP"/>',
                 '\t<SetTexturePersp Enable="G_TP_PERSP"/>',
                 '\t<SetTextureLUT Mode="G_TT_NONE"/>']
        if tex_name:
            # colour = texel x shade (vertex colour), alpha = texel; cutout edges
            lines += ['\t<SetCombineLERP A0="G_CCMUX_TEXEL0" B0="G_CCMUX_0" C0="G_CCMUX_SHADE" D0="G_CCMUX_0" '
                      'Aa0="G_ACMUX_0" Ab0="G_ACMUX_0" Ac0="G_ACMUX_0" Ad0="G_ACMUX_TEXEL0" '
                      'A1="G_CCMUX_TEXEL0" B1="G_CCMUX_0" C1="G_CCMUX_SHADE" D1="G_CCMUX_0" '
                      'Aa1="G_ACMUX_0" Ab1="G_ACMUX_0" Ac1="G_ACMUX_0" Ad1="G_ACMUX_TEXEL0"/>',
                      '\t<SetRenderMode Mode1="G_RM_AA_ZB_TEX_EDGE" Mode2="G_RM_AA_ZB_TEX_EDGE2" />',
                      '\t<Texture S="65535" T="65535" Level="0" Tile="0" On="1"/>',
                      f'\t<SetTextureImage Path="{self.res(tex_name)}" Format="G_IM_FMT_RGBA" Size="G_IM_SIZ_16b_LOAD_BLOCK" Width="1"/>',
                      '\t<SetTile Format="G_IM_FMT_RGBA" Size="G_IM_SIZ_16b_LOAD_BLOCK" Line="0" TMem="0" Tile="7" Palette="0" '
                      'Cms0="G_TX_WRAP" Cms1="G_TX_NOMIRROR" Cmt0="G_TX_WRAP" Cmt1="G_TX_NOMIRROR" MaskS="0" ShiftS="0" MaskT="0" ShiftT="0"/>',
                      '\t<LoadSync/>',
                      f'\t<LoadBlock Tile="7" Uls="0" Ult="0" Lrs="{w * h - 1}" Dxt="{(2048 + w * 2 // 8 - 1) // (w * 2 // 8)}" />',
                      '\t<PipeSync/>',
                      f'\t<SetTile Format="G_IM_FMT_RGBA" Size="G_IM_SIZ_16b" Line="{w * 2 // 8}" TMem="0" Tile="0" Palette="0" '
                      f'Cms0="G_TX_WRAP" Cms1="G_TX_NOMIRROR" Cmt0="G_TX_WRAP" Cmt1="G_TX_NOMIRROR" '
                      f'MaskS="{int(math.log2(w))}" ShiftS="0" MaskT="{int(math.log2(h))}" ShiftT="0"/>',
                      f'\t<SetTileSize T="0" Uls="0" Ult="0" Lrs="{(w - 1) << 2}" Lrt="{(h - 1) << 2}"/>']
        else:
            lines += ['\t<SetCombineLERP A0="G_CCMUX_0" B0="G_CCMUX_0" C0="G_CCMUX_0" D0="G_CCMUX_SHADE" '
                      'Aa0="G_ACMUX_0" Ab0="G_ACMUX_0" Ac0="G_ACMUX_0" Ad0="G_ACMUX_SHADE" '
                      'A1="G_CCMUX_0" B1="G_CCMUX_0" C1="G_CCMUX_0" D1="G_CCMUX_SHADE" '
                      'Aa1="G_ACMUX_0" Ab1="G_ACMUX_0" Ac1="G_ACMUX_0" Ad1="G_ACMUX_SHADE"/>',
                      '\t<SetRenderMode Mode1="G_RM_AA_ZB_OPA_SURF" Mode2="G_RM_AA_ZB_OPA_SURF2" />',
                      '\t<Texture S="65535" T="65535" Level="0" Tile="0" On="0"/>']
        lines += ['\t<EndDisplayList/>', '</DisplayList>', '']
        self.write(name, "\n".join(lines))
        self.materials[key] = name
        return name


def tri_lists(name, corners, writer):
    """Vertex resource + DisplayList XML lines drawing `corners` (triangle
    corner list of (x, y, z, s, t, r, g, b, a)) in chunks of VERTS_PER_LOAD."""
    vtx = ['<Vertex Version="0">']
    for x, y, z, s, t, r, g, b, a in corners:
        vtx.append(f'\t<Vtx X="{x}" Y="{y}" Z="{z}" S="{s}" T="{t}" R="{r}" G="{g}" B="{b}" A="{a}"/>')
    vtx += ['</Vertex>', '']
    writer.write(name, "\n".join(vtx))
    lines = []
    for start in range(0, len(corners), VERTS_PER_LOAD):
        count = min(VERTS_PER_LOAD, len(corners) - start)
        lines.append(f'\t<LoadVertices Path="{writer.res(name)}" VertexBufferIndex="0" VertexOffset="{start}" Count="{count}"/>')
        for t in range(0, count, 3):
            lines.append(f'\t<Triangle1 V00="{t}" V01="{t + 1}" V02="{t + 2}" Flag0="0"/>')
    return lines


def clamp16(v):
    return max(-32768, min(32767, int(round(v))))


def wheel_corners(wheel, side_size, tread_rgb):
    """A wheel as a WHEEL_SIDES-sided cylinder around the attach point: two
    textured discs (the sprite's side-on frame) and a vertex-coloured tread.
    Returns (disc corners, tread corners) in DKR model units, DKR axes."""
    (iw, ih), scale = side_size, wheel["scale"]
    radius = ih * scale / 2 * WHEEL_RADIUS_SCALE
    width = wheel["spins"][0][0][0] * scale * WHEEL_WIDTH_SCALE  # head-on frame width
    cx, cy, cz = wheel["position"]
    discs, tread = [], []
    ring = [(math.cos(2 * math.pi * k / WHEEL_SIDES), math.sin(2 * math.pi * k / WHEEL_SIDES)) for k in range(WHEEL_SIDES)]
    for side in (-1, 1):
        x = cx + side * width / 2
        centre = (x, cy, cz, iw * 16, ih * 16)
        for k in range(WHEEL_SIDES):
            (c0, s0), (c1, s1) = ring[k], ring[(k + 1) % WHEEL_SIDES]
            p0 = (x, cy + s0 * radius, cz + c0 * radius, (0.5 + c0 / 2) * iw * 32, (0.5 - s0 / 2) * ih * 32)
            p1 = (x, cy + s1 * radius, cz + c1 * radius, (0.5 + c1 / 2) * iw * 32, (0.5 - s1 / 2) * ih * 32)
            discs += [centre, p0, p1] if side > 0 else [centre, p1, p0]
    r, g, b = tread_rgb
    for k in range(WHEEL_SIDES):
        (c0, s0), (c1, s1) = ring[k], ring[(k + 1) % WHEEL_SIDES]
        a = (cx - width / 2, cy + s0 * radius, cz + c0 * radius)
        bq = (cx + width / 2, cy + s0 * radius, cz + c0 * radius)
        c = (cx + width / 2, cy + s1 * radius, cz + c1 * radius)
        d = (cx - width / 2, cy + s1 * radius, cz + c1 * radius)
        tread += [a, bq, c, a, c, d]
    return discs, [(p[0], p[1], p[2], 0, 0, r, g, b, 255) for p in tread], radius


def export(rom, object_name, name, mod_dir):
    racer = racers.load_racer(rom, object_name)
    out_dir = os.path.join(mod_dir, "racers", name)
    writer = Writer(out_dir, f"racers/{name}")
    tex3d, tex3d_table = rom.section("ASSET_TEXTURES_3D"), rom.table("ASSET_TEXTURES_3D_TABLE")

    # Body materials: one per texture (or vertex colour only).
    group_mat = {}
    for key in racer["groups"]:
        tex_index = key[0]
        if tex_index is None:
            group_mat[key] = writer.material("vertex_colour")
            continue
        tid = racer["textures"][tex_index]["id"]
        w, h, rgba, _flags, _fmt = dkr.decode_texture(tex3d[tex3d_table[tid]:tex3d_table[tid + 1]])
        tex_name = f"tex_{tid:04d}"
        tw, th = writer.texture(tex_name, w, h, rgba)
        # UVs are in texels * 32 of the original size; rescale if resampled.
        group_mat[key] = (writer.material(("tex", tid), tex_name, tw, th, cutout=bool(key[4])), tw / w, th / h)

    # Wheels: side-on frame (last angle, spin 0) on the discs, tread colour from the head-on frame.
    wheel_mats, wheel_geo = [], []
    ground = None
    for k, wheel in enumerate(racer["wheels"]):
        sw, sh, srgba, _l, _t = wheel["spins"][0][-1]
        tex_name = f"tex_wheel_{wheel['part']}"
        tw, th = writer.texture(tex_name, sw, sh, srgba)
        mat = writer.material(("wheel", wheel["part"]), tex_name, tw, th, cutout=True)
        hw, hh, hrgba, _l, _t = wheel["spins"][0][0]
        opaque = [hrgba[i:i + 3] for i in range(0, len(hrgba), 4) if hrgba[i + 3] >= 128]
        tread = tuple(sum(p[c] for p in opaque) // max(1, len(opaque)) for c in range(3)) if opaque else (60, 20, 20)
        discs, tread_corners, radius = wheel_corners(wheel, (tw, th), tread)
        wheel_mats.append(mat)
        wheel_geo.append((discs, tread_corners))
        bottom = wheel["position"][1] - radius
        ground = bottom if ground is None else min(ground, bottom)
    ground = ground if ground is not None else 0

    def to_model(x, y, z):  # DKR model space -> N64KR model space (face +Z, tyres on y = 0)
        return clamp16(-x), clamp16(y - ground), clamp16(-z)

    frames = racer["anim_frames"]
    for p, frame_index in enumerate(POSE_FRAMES):
        frame = frames[min(frame_index, len(frames) - 1)] if frames else {}
        dl = ['<DisplayList Version="0">']
        for gi, (key, corners) in enumerate(racer["groups"].items()):
            mat = group_mat[key]
            mat_name, su, sv = (mat, 1.0, 1.0) if isinstance(mat, str) else mat
            out = []
            for v, (x, y, z, r, g, b, a, s, t) in zip(racer["corner_vertex"][key], corners):
                x, y, z = frame.get(v, (x, y, z))
                out.append(to_model(x, y, z) + (clamp16(s * su), clamp16(t * sv), r, g, b, a))
            dl.append(f'\t<CallDisplayList Path="{writer.res(mat_name)}"/>')
            dl += tri_lists(f"pose{p}_g{gi}_vtx", out, writer)
        for k, (discs, tread_corners) in enumerate(wheel_geo):
            disc_out = [to_model(x, y, z) + (clamp16(s), clamp16(t), 255, 255, 255, 255) for x, y, z, s, t in discs]
            tread_out = [to_model(x, y, z) + (0, 0, r, g, b, a) for x, y, z, _s, _t, r, g, b, a in tread_corners]
            dl.append(f'\t<CallDisplayList Path="{writer.res(wheel_mats[k])}"/>')
            dl += tri_lists(f"wheel{k}_discs_vtx", disc_out, writer)
            dl.append(f'\t<CallDisplayList Path="{writer.res(writer.material("vertex_colour"))}"/>')
            dl += tri_lists(f"wheel{k}_tread_vtx", tread_out, writer)
        dl += ['\t<PipeSync/>', '\t<EndDisplayList/>', '</DisplayList>', '']
        writer.write(f"pose{p}", "\n".join(dl))

    info = {"object": object_name, "poses": POSE_FRAMES, "straight_pose": POSE_FRAMES.index(40),
            "world_scale": WORLD_SCALE, "ground_dkr_units": ground,
            "space": "DKR units, Y up, facing +Z, tyre bottom at y = 0"}
    with open(os.path.join(out_dir, "model.json"), "w") as fh:
        json.dump(info, fh, indent=1)
    tris = sum(len(c) for c in racer["groups"].values()) // 3
    print(f"Racer model '{name}': {tris} body triangles, {len(racer['wheels'])} wheels, {len(POSE_FRAMES)} poses, "
          f"{len(writer.materials)} materials -> {out_dir}")


def main():
    if len(sys.argv) != 5:
        sys.exit(__doc__)
    export(dkr.Rom(sys.argv[1]), sys.argv[2], sys.argv[3], sys.argv[4])


if __name__ == "__main__":
    main()
