"""Blender 4.3 script: animated select-screen faces for a new N64KR character
from a DKR racer export (dkr_racer_to_gltf.py), for the DKR select page and the
cup award screen (src/n64kr/character_select.c).

Run headless:
    blender.exe -b --python tools/n64kr/blender_dkr_racer_faces.py -- <racer dir> <mod dir> <hud name>
e.g.
    ... -- build/dkr_racers/diddycar build/x64/mods/n64kr-dkr-diddy diddy

Writes <mod dir>/textures/player_selection/<hud name>_face_NN.png, FACE_PX
square, each with a stub resource at its path without the extension (see
blender_dkr_racer_sprites.py). The engine draws them in MK64's 64x64 face slot:
SpaghettiKart loads a PNG at its own size (BetterTextureFactory.cpp) and takes
texture coordinates from the 64x64 tile, so they show at 4x MK64's detail.

Frames (N64KR_FACE_* in src/n64kr/character_select.c):
  00       idle: straight driving pose, eyes open, three-quarter view toward
           screen right, head and shoulders, like MK64's faces
  01, 02   blink: eyes half shut, shut (DKR's animated eye texture frames)
  03..14   picked: DKR's horn taunt (animation 4, frames 0..44), turning to
           face the viewer over the first four frames, as MK64's faces turn
  15..22   lost: DKR's crash animation (animation 3, frames 0..28)
Built from ROM data: keep under build/, never commit.
"""
import json
import math
import os
import sys

import bpy
import mathutils

argv = sys.argv[sys.argv.index("--") + 1:]
if len(argv) != 3:
    sys.exit(__doc__)
racer_dir, mod_dir, hud_name = os.path.abspath(argv[0]), os.path.abspath(argv[1]), argv[2]
name = os.path.basename(racer_dir.rstrip("/\\"))
info = json.load(open(os.path.join(racer_dir, f"{name}_racer.json")))

FACE_PX = 256                 # 4 x MK64's 64 x 64 faces
STRAIGHT_POSE = "anim0_f40"   # DKR driving animation, straight (racer.c)
TURN_DEGREES = 30.0           # three-quarter view: camera this far round from the front
ELEVATION = 0.18              # camera slightly above the face
FRAME_SHARE = 0.85            # framing: the frame shows this share of the model's height
EYES_ABOVE = 0.08             # eyes this share of the frame above its middle
HEAD_DEPTH = 0.08             # aim this share of the model's height behind the eyes
FILL = 0.5                    # half the frame's height = this share of the framed box
TURN_FRAMES = 4               # picked frames spent turning to the front
STUB = (b"\0\0\0\0" + b"XETO" + b"\0\0\0\0" + bytes.fromhex("efbeaddeefbeadde")).ljust(0x40, b"\0") + \
    (2).to_bytes(4, "little") + (64).to_bytes(4, "little") + (64).to_bytes(4, "little") + bytes(4)

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=os.path.join(racer_dir, f"{name}.gltf"))
scene = bpy.context.scene
body = [o for o in scene.objects if o.type == "MESH"]

# Unlit: vertex colour x texture as emission, alpha kept (as the sprite and HUD renderers).
eye_nodes = []  # (image node, texture id) of the animated eye textures
eye_ids = {int(t) for t, n in info.get("texture_frames", {}).items() if 2 <= n <= 4}
for mat in bpy.data.materials:
    if not mat.use_nodes:
        continue
    nt = mat.node_tree
    for node in nt.nodes:
        if node.type == "TEX_IMAGE" and node.image is not None:
            base = os.path.basename(node.image.filepath)
            if base.startswith("dkr_tex") and base[7:11].isdigit() and int(base[7:11]) in eye_ids:
                eye_nodes.append((node, int(base[7:11])))
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

eye_images = {}


def eyes(frame):
    """Eye texture frame: 0 open .. last shut (clamped per texture)."""
    for node, tid in eye_nodes:
        count = info["texture_frames"][str(tid)]
        k = min(frame, count - 1)
        key = (tid, k)
        if key not in eye_images:
            file = f"dkr_tex{tid:04d}.png" if k == 0 else f"dkr_tex{tid:04d}_f{k}.png"
            eye_images[key] = bpy.data.images.load(os.path.join(racer_dir, "textures", file))
        node.image = eye_images[key]


def eye_frame_count():
    return min((info["texture_frames"][str(tid)] for _, tid in eye_nodes), default=1)


def pose(shape_key):
    for o in body:
        if o.data.shape_keys:
            for kb in o.data.shape_keys.key_blocks[1:]:
                kb.value = 1.0 if kb.name == shape_key else 0.0


# Framing from the straight pose, fixed for every frame so the face doesn't
# jump: a box FRAME_SHARE of the model's height, centred just behind and below
# the eyes (the triangles with the eye texture), so the eyes sit a little above
# the middle as in MK64's faces. A tall cap or tail (Diddy, Conker) can't push
# the face out of frame, as framing on the top of the model did.
pose(STRAIGHT_POSE)
depsgraph = bpy.context.evaluated_depsgraph_get()
eye_materials = {node.id_data for node, _ in eye_nodes}
points, eye_points = [], []
for o in body:
    ev = o.evaluated_get(depsgraph)
    mesh = ev.to_mesh()
    world = [o.matrix_world @ v.co for v in mesh.vertices]
    points += world
    for poly in mesh.polygons:
        slot = o.material_slots[poly.material_index] if poly.material_index < len(o.material_slots) else None
        if slot is not None and slot.material is not None and slot.material.node_tree in eye_materials:
            eye_points += [world[i] for i in poly.vertices]
    ev.to_mesh_clear()
top = max(p.z for p in points)
bottom = min(p.z for p in points)
height = top - bottom
size = height * FRAME_SHARE
if eye_points:
    eye = sum(eye_points, mathutils.Vector()) / len(eye_points)
else:  # no animated eyes: the top of the model
    eye = mathutils.Vector((sum(p.x for p in points) / len(points), sum(p.y for p in points) / len(points),
                            top - height * 0.2))
centre = eye + mathutils.Vector((0, -height * HEAD_DEPTH, -size * EYES_ABOVE))

cam_data = bpy.data.cameras.new("face")
cam_data.lens = 70
cam_data.sensor_fit = "VERTICAL"
cam = bpy.data.objects.new("face", cam_data)
scene.collection.objects.link(cam)
scene.camera = cam
fov = 2 * math.atan(cam_data.sensor_height / 2 / cam_data.lens)
distance = size * FILL / math.tan(fov / 2)


def aim(turn_degrees):
    """The model faces +Y (glTF -Z); a camera on the +X side of its front
    shows it looking toward screen right, as MK64's faces do (checked in a
    render: -X showed it looking left)."""
    a = math.radians(turn_degrees)
    view = mathutils.Vector((math.sin(a), math.cos(a), ELEVATION)).normalized()
    cam.location = centre + view * distance
    cam.rotation_euler = (centre - cam.location).to_track_quat("-Z", "Y").to_euler()


scene.render.engine = "BLENDER_EEVEE_NEXT"
scene.render.film_transparent = True
scene.render.resolution_x = scene.render.resolution_y = FACE_PX
scene.render.image_settings.file_format = "PNG"
scene.render.image_settings.color_mode = "RGBA"
scene.render.image_settings.color_depth = "8"
scene.view_settings.view_transform = "Standard"
scene.eevee.taa_render_samples = 32

out_dir = os.path.join(mod_dir, "textures", "player_selection")
os.makedirs(out_dir, exist_ok=True)
frame_number = 0


def render():
    global frame_number
    base = f"{hud_name}_face_{frame_number:02d}"
    scene.render.filepath = os.path.join(out_dir, base + ".png")
    bpy.ops.render.render(write_still=True)
    with open(os.path.join(out_dir, base), "wb") as fh:
        fh.write(STUB)
    frame_number += 1


# 00 idle, 01-02 blink (half shut, shut).
pose(STRAIGHT_POSE)
aim(TURN_DEGREES)
eyes(0)
render()
shut = eye_frame_count() - 1
eyes(max(1, shut - 1))
render()
eyes(shut)
render()
eyes(0)

# 03-14 picked: horn taunt, turning to the front.
horn = info["extra_animations"]["frames"]["4"]
for i, f in enumerate(horn):
    pose(f"anim4_f{f:02d}")
    aim(TURN_DEGREES * max(0.0, 1.0 - (i + 1) / TURN_FRAMES))
    render()

# 15-22 lost: crash animation, three-quarter view.
aim(TURN_DEGREES)
for f in info["extra_animations"]["frames"]["3"]:
    pose(f"anim3_f{f:02d}")
    render()

print(f"[n64kr] {frame_number} select faces for '{hud_name}' ({FACE_PX} px) in {out_dir}")
