"""Blender 4.3 script: HUD textures for a new N64KR character from a DKR racer
export (dkr_racer_to_gltf.py): race portrait and minimap icons, styled after a
stock character's (frame, background, icon shapes read from mk64.o2r).

Run headless:
    blender.exe -b --python tools/n64kr/blender_dkr_racer_hud.py -- <racer dir> <mod dir> <hud name> <mk64.o2r> <template> <r,g,b>
e.g.
    ... -- build/dkr_racers/diddycar build/x64/mods/n64kr-dkr-diddy diddy build/x64/mk64.o2r toad 248,120,0

Writes into <mod dir>/textures/common_data/ (PNG + stub resource each, see
blender_dkr_racer_sprites.py for why new paths need a stub):
  common_texture_portrait_<hud name>      32x32: the racer's head (driving pose,
                                          3/4 view) inside the template's frame
  common_texture_minimap_kart_<hud name>  8x8: template kart icon recoloured
  common_texture_minimap_<hud name>       8x8: template progress dot
and into <mod dir>/textures/player_selection|texture_tkmk00/:
  diddy_face_00 (named <hud name>)        64x64: select screen face
  texture_name_<hud name>                 64x12: blank select nameplate (the
                                          page draws the name over it in text)
The engine finds the first through the registry's hudName
(src/n64kr/character.c); the select page draws the last two
(src/n64kr/character_select.c).
Built from ROM data: keep under build/, never commit.
"""
import math
import os
import struct
import sys
import zipfile
import zlib

import bpy
import mathutils

argv = sys.argv[sys.argv.index("--") + 1:]
if len(argv) != 6:
    sys.exit(__doc__)
racer_dir, mod_dir, hud_name = os.path.abspath(argv[0]), os.path.abspath(argv[1]), argv[2]
o2r_path, template = argv[3], argv[4]
kart_rgb = tuple(int(v) for v in argv[5].split(","))
name = os.path.basename(racer_dir.rstrip("/\\"))
out_dir = os.path.join(mod_dir, "textures", "common_data")
os.makedirs(out_dir, exist_ok=True)

PORTRAIT = 32
FRAME = 3                      # stock portraits: 3 px blue frame, black inside
INNER = PORTRAIT - 2 * FRAME   # 26
SUPERSAMPLE = 4
STRAIGHT_POSE = "anim0_f40"    # DKR driving animation, straight (racer.c)
STUB = (b"\0\0\0\0" + b"XETO" + b"\0\0\0\0" + bytes.fromhex("efbeaddeefbeadde")).ljust(0x40, b"\0") + \
    (4).to_bytes(4, "little") + (32).to_bytes(4, "little") + (32).to_bytes(4, "little") + bytes(4)


# --- mk64.o2r textures (libultraship resource: 0x40 header, type, w, h, size, data)
o2r = zipfile.ZipFile(o2r_path)


def o2r_texture(path):
    d = o2r.read(path)
    kind, w, h, size = struct.unpack("<IIII", d[0x40:0x50])
    return kind, w, h, d[0x50:0x50 + size]


def rgba16(v):
    return [((v >> 11) & 31) * 255 // 31, ((v >> 6) & 31) * 255 // 31, ((v >> 1) & 31) * 255 // 31,
            255 if v & 1 else 0]


def decode(path, tlut=None):
    kind, w, h, raw = o2r_texture(path)
    if kind == 4:  # CI8 + RGBA16 palette
        pal = [struct.unpack(">H", tlut[i * 2:i * 2 + 2])[0] for i in range(256)]
        return w, h, [rgba16(pal[i]) for i in raw]
    return w, h, [rgba16(struct.unpack(">H", raw[i:i + 2])[0]) for i in range(0, len(raw), 2)]


def png_bytes(w, h, px):
    rows = b"".join(b"\0" + bytes(c for p in px[y * w:(y + 1) * w] for c in p) for y in range(h))
    chunk = lambda t, d: struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)) +
            chunk(b"IDAT", zlib.compress(rows, 9)) + chunk(b"IEND", b""))


def write(base, w, h, px):
    with open(os.path.join(out_dir, base + ".png"), "wb") as fh:
        fh.write(png_bytes(w, h, px))
    with open(os.path.join(out_dir, base), "wb") as fh:
        fh.write(STUB)


# --- portrait: render the head
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=os.path.join(racer_dir, f"{name}.gltf"))
scene = bpy.context.scene
body = [o for o in scene.objects if o.type == "MESH"]
for o in body:
    if o.data.shape_keys:
        for kb in o.data.shape_keys.key_blocks[1:]:
            kb.value = 1.0 if kb.name == STRAIGHT_POSE else 0.0
# Unlit: vertex colour x texture as emission, alpha kept (same idea as the sprite renderer).
for mat in bpy.data.materials:
    if not mat.use_nodes:
        continue
    nt = mat.node_tree
    bsdf = next((n for n in nt.nodes if n.type == "BSDF_PRINCIPLED"), None)
    if bsdf is None:
        continue
    emit, mix, transp = nt.nodes.new("ShaderNodeEmission"), nt.nodes.new("ShaderNodeMixShader"), \
        nt.nodes.new("ShaderNodeBsdfTransparent")
    if bsdf.inputs["Base Color"].links:
        nt.links.new(bsdf.inputs["Base Color"].links[0].from_socket, emit.inputs["Color"])
    if bsdf.inputs["Alpha"].links:
        nt.links.new(bsdf.inputs["Alpha"].links[0].from_socket, mix.inputs["Fac"])
    else:
        mix.inputs["Fac"].default_value = 1.0
    nt.links.new(transp.outputs[0], mix.inputs[1])
    nt.links.new(emit.outputs[0], mix.inputs[2])
    nt.links.new(mix.outputs[0], next(n for n in nt.nodes if n.type == "OUTPUT_MATERIAL").inputs["Surface"])

# Head = posed vertices in the top part of the model (cap and face).
depsgraph = bpy.context.evaluated_depsgraph_get()
points = []
for o in body:
    ev = o.evaluated_get(depsgraph)
    mesh = ev.to_mesh()
    points += [o.matrix_world @ v.co for v in mesh.vertices]
    ev.to_mesh_clear()
top = max(p.z for p in points)
bottom = min(p.z for p in points)
head = [p for p in points if p.z > top - (top - bottom) * 0.36]
lo = mathutils.Vector([min(p[i] for p in head) for i in range(3)])
hi = mathutils.Vector([max(p[i] for p in head) for i in range(3)])
centre = (lo + hi) / 2
size = max(hi - lo)

cam_data = bpy.data.cameras.new("portrait")
cam_data.type = "ORTHO"
cam_data.ortho_scale = size * 1.0
cam = bpy.data.objects.new("portrait", cam_data)
scene.collection.objects.link(cam)
scene.camera = cam
# The model faces +Y (glTF -Z). Stock portraits look 3/4 toward the viewer's
# right, slightly from above.
view = mathutils.Vector((0.55, 1.0, 0.25)).normalized()
cam.location = centre + view * size * 4
cam.rotation_euler = (centre - cam.location).to_track_quat("-Z", "Y").to_euler()

scene.render.engine = "BLENDER_EEVEE_NEXT"
scene.render.film_transparent = True
scene.render.resolution_x = scene.render.resolution_y = INNER * SUPERSAMPLE
scene.render.image_settings.file_format = "PNG"
scene.render.image_settings.color_mode = "RGBA"
scene.render.image_settings.color_depth = "8"
scene.view_settings.view_transform = "Standard"


def render(file, raise_frames=0.0):
    """Render the head; raise_frames > 0 shows it higher in the frame (the
    camera moves down along its own up axis by that share of the view)."""
    up = cam.rotation_euler.to_matrix() @ mathutils.Vector((0, 1, 0))
    home = cam.location.copy()
    cam.location = home - up * cam_data.ortho_scale * raise_frames
    path = os.path.join(racer_dir, "frames", file)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    scene.render.filepath = path
    bpy.ops.render.render(write_still=True)
    cam.location = home
    image = bpy.data.images.load(path)
    return image.size[0], image.size[1], list(image.pixels)  # floats, bottom row first


# Race portrait: face a third of the frame higher than centred (user request:
# the centred head read low in the 26x26 window).
PORTRAIT_RAISE = 1.0 / 3.0
rw, rh, src = render("portrait_render.png", PORTRAIT_RAISE)


def sample(x, y):  # box filter SUPERSAMPLE x SUPERSAMPLE, premultiplied
    acc = [0.0, 0.0, 0.0, 0.0]
    for dy in range(SUPERSAMPLE):
        for dx in range(SUPERSAMPLE):
            sx, sy = x * SUPERSAMPLE + dx, rh - 1 - (y * SUPERSAMPLE + dy)
            i = (sy * rw + sx) * 4
            a = src[i + 3]
            for k in range(3):
                acc[k] += src[i + k] * a
            acc[3] += a
    n = SUPERSAMPLE * SUPERSAMPLE
    a = acc[3] / n
    return [acc[k] / acc[3] if acc[3] else 0.0 for k in range(3)] + [a]


tlut = o2r_texture(f"textures/common_data/common_tlut_portrait_{template}")[3]
w, h, frame_px = decode(f"textures/common_data/common_texture_portrait_{template}", tlut)
portrait = [list(p) for p in frame_px]
for y in range(INNER):
    for x in range(INNER):
        r, g, b, a = sample(x, y)
        dst = portrait[(y + FRAME) * w + x + FRAME]
        bg = [0, 0, 0]  # stock portraits: black inside the frame
        dst[:] = [round((c * a + bg[k] / 255 * (1 - a)) * 255) for k, c in enumerate((r, g, b))] + [255]
write(f"common_texture_portrait_{hud_name}", w, h, portrait)

# --- minimap icons
template_kart = {"dk": "donkey_kong"}.get(template, template)
w, h, kart = decode(f"textures/common_data/common_texture_minimap_kart_{template_kart}")
counts = {}
for p in kart:
    if p[3] and sum(p[:3]) > 60 and p[:3] != [255, 255, 255]:
        counts[tuple(p[:3])] = counts.get(tuple(p[:3]), 0) + 1
body_colour = max(counts, key=counts.get)
kart = [list(kart_rgb) + [255] if (p[3] and tuple(p[:3]) == body_colour) else p for p in kart]
write(f"common_texture_minimap_kart_{hud_name}", w, h, kart)
template_dot = {"donkey_kong": "dk"}.get(template, template)
w, h, dot = decode(f"textures/common_data/common_texture_minimap_{template_dot}")
write(f"common_texture_minimap_{hud_name}", w, h, dot)
print(f"[n64kr] HUD textures for '{hud_name}' in {out_dir}")

# --- select screen face: the head centred, 64x64, transparent background
FACE = 64
rw, rh, src = render("face_render.png")
face_dir = os.path.join(mod_dir, "textures", "player_selection")
os.makedirs(face_dir, exist_ok=True)
face = []  # whole render box-filtered to 64x64 (sizes need not divide)
for y in range(FACE):
    for x in range(FACE):
        acc, n = [0.0, 0.0, 0.0, 0.0], 0
        for sy in range(y * rh // FACE, max(y * rh // FACE + 1, (y + 1) * rh // FACE)):
            for sx in range(x * rw // FACE, max(x * rw // FACE + 1, (x + 1) * rw // FACE)):
                i = ((rh - 1 - sy) * rw + sx) * 4
                alpha = src[i + 3]
                for k in range(3):
                    acc[k] += src[i + k] * alpha
                acc[3] += alpha
                n += 1
        rgb = [acc[k] / acc[3] if acc[3] else 0.0 for k in range(3)]
        face.append([round(c * 255) for c in rgb] + [round(acc[3] / n * 255)])
out_dir = face_dir
write(f"{hud_name}_face_00", FACE, FACE, face)

# --- nameplate: the stock plate with its letters erased (the plate's
# gradient runs horizontally; row 10 is letter-free, so it is the background
# of every column). The select page prints the name over it in menu text
# (src/n64kr/character_select.c); cutting MK64's embossed letters apart fails
# because they touch through their shadows.
plate_w, plate_h, base = decode("textures/texture_tkmk00/texture_name_toad")
base = [list(p) for p in base]
for y in range(1, plate_h - 1):
    for x in range(1, plate_w - 1):
        p, b = base[y * plate_w + x], base[10 * plate_w + x]
        if p[3] and max(abs(p[k] - b[k]) for k in range(3)) > 7:
            base[y * plate_w + x] = list(b)
out_dir = os.path.join(mod_dir, "textures", "texture_tkmk00")
os.makedirs(out_dir, exist_ok=True)
write(f"texture_name_{hud_name}", plate_w, plate_h, base)
print(f"[n64kr] select face and blank nameplate written")
