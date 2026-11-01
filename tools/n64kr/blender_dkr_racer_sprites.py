"""Blender 4.3 script: render a DKR racer (from dkr_racer_to_gltf.py) into MK64
kart sprite frames, using SpaghettiKart's kart setup scene
(docs/MK64_Spaghetti_Adjusted_Kart_Setup.blend: fixed camera, 64x64, frames
0-320 animated by the "Root" / "Kart Root" empties).

Run headless:
    blender.exe -b docs/MK64_Spaghetti_Adjusted_Kart_Setup.blend --python tools/n64kr/blender_dkr_racer_sprites.py -- <racer dir> <mod dir> <kart slot>
e.g.
    ... -- build/dkr_racers/diddycar build/x64/mods/n64kr-dkr-racer-test mario_kart

Output: <mod dir>/mods.toml + textures/karts/<slot>/<slot>_frameNNN_wheelW.png
(frames 0-288, wheel spin 0-3) and <slot>_frameNNN.png (289-320, tumble), the
names MK64's kart textures use (include/assets/textures/karts/*.h). A
texture pack replaces that character's sprites. Built from ROM data: keep
under build/, never commit.
"""
import json
import os
import shutil
import sys

import bpy
import mathutils

argv = sys.argv[sys.argv.index("--") + 1:]
preview = "--preview" in argv  # render a few sample frames only, no packaging
argv = [a for a in argv if a != "--preview"]
if len(argv) != 3:
    sys.exit(__doc__)
racer_dir, mod_dir, slot = os.path.abspath(argv[0]), os.path.abspath(argv[1]), argv[2]
name = os.path.basename(racer_dir.rstrip("/\\"))
info = json.load(open(os.path.join(racer_dir, f"{name}_racer.json")))

FRAMES_WITH_WHEELS = range(0, 289)  # 4 wheel-spin variants each
TUMBLE_FRAMES = range(289, 321)     # single image each
WHEEL_VARIANTS = 4

scene = bpy.context.scene
kart_root = bpy.data.objects["Kart Root"]
scene.frame_set(0)


def descendants(obj):
    for child in obj.children:
        yield child
        yield from descendants(child)


def world_bbox(objs):
    lo = mathutils.Vector((1e9,) * 3)
    hi = mathutils.Vector((-1e9,) * 3)
    for o in objs:
        for corner in o.bound_box:
            w = o.matrix_world @ mathutils.Vector(corner)
            lo = mathutils.Vector(map(min, lo, w))
            hi = mathutils.Vector(map(max, hi, w))
    return lo, hi


# Mario + his kart: measure, then hide from the render. Fit on the wheels:
# their outer edges (rims) and the tyres' bottom (the ground). Bounding boxes
# of subdivided meshes like "Kart" are their unsubdivided cages, so they are
# not used.
stock = [o for o in descendants(kart_root) if o.type == "MESH" and not o.hide_render]
target_lo, target_hi = world_bbox(stock)
wheel_lo, wheel_hi = world_bbox([o for o in stock if o.name.startswith(("wheelrim", "wheeltyre"))])
for o in stock:
    o.hide_render = True

# Import the DKR racer body. glTF (Y up, -Z forward) arrives as Blender
# (Z up, +Y forward), the same way the setup's kart faces.
before = set(bpy.data.objects)
bpy.ops.import_scene.gltf(filepath=os.path.join(racer_dir, f"{name}.gltf"))
body = [o for o in bpy.data.objects if o not in before and o.type == "MESH"]


def dkr_to_blender(p):
    return mathutils.Vector((p[0], -p[2], p[1]))


# Fit wheel span to wheel span: DKR wheel centres +- half the wheel sprite
# width -> the stock rims' x extent; wheel bottoms on the stock tyres' bottom;
# wheelbase centres aligned.
wheel_pts = [dkr_to_blender(w["position"]) for w in info["wheels"]]
half_w = max(w["frames"][0]["w"] * w["scale"] for w in info["wheels"]) / 2
half_h = max(w["frames"][0]["h"] * w["scale"] for w in info["wheels"]) / 2
dkr_span = (max(p.x for p in wheel_pts) + half_w) - (min(p.x for p in wheel_pts) - half_w)
scale = (wheel_hi.x - wheel_lo.x) / max(1e-6, dkr_span)
dkr_centre = mathutils.Vector(((max(p.x for p in wheel_pts) + min(p.x for p in wheel_pts)) / 2,
                               (max(p.y for p in wheel_pts) + min(p.y for p in wheel_pts)) / 2,
                               min(p.z for p in wheel_pts) - half_h))
stock_centre = mathutils.Vector(((wheel_lo.x + wheel_hi.x) / 2, (wheel_lo.y + wheel_hi.y) / 2, wheel_lo.z))
offset = stock_centre - dkr_centre * scale


def place(obj):
    obj.matrix_world = mathutils.Matrix.Translation(offset) @ mathutils.Matrix.Scale(scale, 4) @ obj.matrix_world
    world = obj.matrix_world.copy()
    obj.parent = kart_root
    obj.matrix_world = world  # keep the fitted placement; follow Kart Root's animation


def unlit(mat, image=None):
    """DKR models are pre-lit with vertex colours: render texture x colour
    unshaded, alpha from the texture (Emission + Transparent mix)."""
    mat.use_nodes = True
    nt = mat.node_tree
    if image is not None:
        nt.nodes.clear()
        tex = nt.nodes.new("ShaderNodeTexImage")
        tex.image = image
        tex.interpolation = "Closest"
        colour, alpha = tex.outputs["Color"], tex.outputs["Alpha"]
    else:
        bsdf = next((n for n in nt.nodes if n.type == "BSDF_PRINCIPLED"), None)
        if bsdf is None:
            return
        colour = bsdf.inputs["Base Color"].links[0].from_socket if bsdf.inputs["Base Color"].links else None
        alpha = bsdf.inputs["Alpha"].links[0].from_socket if bsdf.inputs["Alpha"].links else None
    out = next((n for n in nt.nodes if n.type == "OUTPUT_MATERIAL"), None) or nt.nodes.new("ShaderNodeOutputMaterial")
    emit = nt.nodes.new("ShaderNodeEmission")
    if colour is not None:
        nt.links.new(colour, emit.inputs["Color"])
    transparent = nt.nodes.new("ShaderNodeBsdfTransparent")
    mix = nt.nodes.new("ShaderNodeMixShader")
    if alpha is not None:
        nt.links.new(alpha, mix.inputs["Fac"])
    else:
        mix.inputs["Fac"].default_value = 1.0
    nt.links.new(transparent.outputs[0], mix.inputs[1])
    nt.links.new(emit.outputs[0], mix.inputs[2])
    nt.links.new(mix.outputs[0], out.inputs["Surface"])


for o in body:
    place(o)
    for mat in o.data.materials:
        if mat is not None:
            unlit(mat)

# Wheels: camera-facing quads with the vehicle part's sprite frame, sized
# pixel x part scale (DKR model units), anchored like DKR sprites.
camera = scene.camera
wheel_images = {}
wheel_mats = []
for k, wheel in enumerate(info["wheels"]):
    frame0 = wheel["frames"][0]
    w_units = frame0["w"] * wheel["scale"]
    h_units = frame0["h"] * wheel["scale"]
    mesh = bpy.data.meshes.new(f"dkr_wheel{k}")
    mesh.from_pydata([(-w_units / 2, -h_units / 2, 0), (w_units / 2, -h_units / 2, 0),
                      (w_units / 2, h_units / 2, 0), (-w_units / 2, h_units / 2, 0)], [], [(0, 1, 2, 3)])
    uv = mesh.uv_layers.new()
    for loop, coord in zip(uv.data, ((0, 0), (1, 0), (1, 1), (0, 1))):
        loop.uv = coord
    obj = bpy.data.objects.new(f"dkr_wheel{k}", mesh)
    scene.collection.objects.link(obj)
    obj.location = dkr_to_blender(wheel["position"])
    place(obj)
    con = obj.constraints.new("TRACK_TO")
    con.target = camera
    con.track_axis = "TRACK_Z"
    con.up_axis = "UP_Y"
    frames = []
    for fr in wheel["frames"]:
        path = os.path.join(racer_dir, fr["file"])
        if path not in wheel_images:
            wheel_images[path] = bpy.data.images.load(path)
        frames.append(wheel_images[path])
    mat = bpy.data.materials.new(f"dkr_wheel{k}")
    unlit(mat, frames[0])
    mesh.materials.append(mat)
    wheel_mats.append((mat, frames))


def set_wheel_variant(variant):
    for mat, frames in wheel_mats:
        tex = next(n for n in mat.node_tree.nodes if n.type == "TEX_IMAGE")
        tex.image = frames[variant % len(frames)]


# Render.
render_dir = os.path.join(racer_dir, "frames")
os.makedirs(render_dir, exist_ok=True)
scene.render.image_settings.file_format = "PNG"
scene.render.image_settings.color_mode = "RGBA"
scene.render.image_settings.color_depth = "8"
scene.render.film_transparent = True
if preview:
    scene.render.resolution_percentage = 400
    lo, hi = world_bbox(body)
    print(f"[n64kr] body world bbox at frame 0: {[round(v, 2) for v in lo]} .. {[round(v, 2) for v in hi]}; "
          f"stock {[round(v, 2) for v in target_lo]} .. {[round(v, 2) for v in target_hi]}")
    for f in (0, 10, 20, 35, 60, 100, 150, 200, 250, 288, 295, 310):
        scene.frame_set(f)
        scene.render.filepath = os.path.join(render_dir, f"preview_{f:03d}.png")
        bpy.ops.render.render(write_still=True)
    print(f"[n64kr] preview frames in {render_dir} (fit scale {scale:.4f})")
    sys.exit(0)
for variant in range(WHEEL_VARIANTS):
    set_wheel_variant(variant)
    scene.frame_start = 0
    scene.frame_end = 320 if variant == 0 else FRAMES_WITH_WHEELS.stop - 1
    scene.render.filepath = os.path.join(render_dir, f"w{variant}_###")
    bpy.ops.render.render(animation=True)
    print(f"[n64kr] rendered wheel variant {variant}")

# Package as a texture pack for the chosen kart slot.
out_dir = os.path.join(mod_dir, "textures", "karts", slot)
os.makedirs(out_dir, exist_ok=True)
for f in FRAMES_WITH_WHEELS:
    for variant in range(WHEEL_VARIANTS):
        shutil.copyfile(os.path.join(render_dir, f"w{variant}_{f:03d}.png"),
                        os.path.join(out_dir, f"{slot}_frame{f:03d}_wheel{variant}.png"))
for f in TUMBLE_FRAMES:
    shutil.copyfile(os.path.join(render_dir, f"w0_{f:03d}.png"), os.path.join(out_dir, f"{slot}_frame{f:03d}.png"))
with open(os.path.join(mod_dir, "mods.toml"), "w", newline="\n") as fh:
    fh.write(f'[mod]\nname = "{os.path.basename(mod_dir)}"\nversion = "0.1.0"\n')
print(f"[n64kr] wrote {len(os.listdir(out_dir))} sprite frames to {out_dir} (fit scale {scale:.4f})")
