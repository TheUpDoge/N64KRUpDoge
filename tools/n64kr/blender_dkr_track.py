"""Blender (4.3 + HarbourMasters Fast64) script: build a SpaghettiKart custom
track from a DKR track converted with `dkr_level_to_gltf.py --mk64`.

Run headless:
    blender.exe -b --python tools/n64kr/blender_dkr_track.py -- <converted dir> <mods dir> <track name>
e.g.
    blender.exe -b --python tools/n64kr/blender_dkr_track.py -- build/dkr_tracks/ancient_lake_mk64 build/x64/mods ancient_lake

Writes <mods dir>/n64kr-dkr-<track>/ (mods.toml + tracks/dkr_<track>/...). The
output is built from ROM data: keep it under build/, never commit it.
Optionally saves the .blend next to the converted files for hand edits.

Steps: import glTF -> Fast64 MK64 scene settings -> split by material ->
per-object surface / collision / draw layer from the material extras written
by the converter -> F3D materials (vertex-coloured texture presets) -> Track
Root + path curve from <track>_mk64.json -> Fast64 "Export Track".
"""
import json
import os
import sys

import bpy

argv = sys.argv[sys.argv.index("--") + 1:]
if len(argv) != 3:
    sys.exit(__doc__)
src_dir, mods_dir, track = (os.path.abspath(argv[0]), os.path.abspath(argv[1]), argv[2])
gltf_path = os.path.join(src_dir, f"{track}.gltf")
layout = json.load(open(os.path.join(src_dir, f"{track}_mk64.json")))
mod_name = f"n64kr-dkr-{track.replace('_', '-')}"
export_name = f"dkr_{track}"
mod_dir = os.path.join(mods_dir, mod_name)


def n64_to_blender(p):
    """N64/glTF (x, y-up, z) -> Blender (x, -z, y), the glTF importer's axis conversion."""
    return (p[0], -p[2], p[1])


def srgb_to_linear(c):
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


# ------------------------------------------------------------- scene setup
bpy.ops.wm.read_factory_settings(use_empty=True)
import addon_utils  # noqa: E402

addon_utils.enable("fast64", default_set=True)
scene = bpy.context.scene
scene.gameEditorMode = "MK64"
# Geometry is already in MK64 units (converter --mk64), so export 1:1.
scene.fast64.mk64.scale = 1.0
scene.hm64_mk64_feature_set = "HM64"
scene.hm64_mk64_export_name = export_name
scene.hm64_mk64_export_path = mod_dir

bpy.ops.import_scene.gltf(filepath=gltf_path)
imported = [o for o in bpy.context.scene.objects if o.type == "MESH"]
print(f"[n64kr] imported {[o.name for o in imported]}")

# ------------------------------------------------------- split by material
for obj in imported:
    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.mesh.separate(type="MATERIAL")
    bpy.ops.object.mode_set(mode="OBJECT")

meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]

# Every triangle corner was written as its own vertex; weld positions per
# material object. (Welding before the split would merge DKR's decals into the
# surface they sit on and delete them as duplicate faces.) UVs and colours live
# on face corners, so they survive.
for obj in meshes:
    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.mesh.remove_doubles(threshold=0.05)
    bpy.ops.object.mode_set(mode="OBJECT")

# ------------------------------------- vertex colours, collision, materials
for obj in meshes:
    mesh = obj.data
    src = mesh.color_attributes.get("Color") or (mesh.color_attributes[0] if mesh.color_attributes else None)
    if src is not None:
        # Fast64 gamma-corrects "Col"/"Alpha" on export (f3d_writer.getLoopColor);
        # store the inverse so DKR's original bytes come out unchanged.
        col = mesh.color_attributes.new("Col", "FLOAT_COLOR", "CORNER")
        alpha = mesh.color_attributes.new("Alpha", "FLOAT_COLOR", "CORNER")
        values = [0.0] * (len(src.data) * 4)
        src.data.foreach_get("color", values)
        if src.domain == "POINT":
            per_loop = []
            for loop in mesh.loops:
                i = loop.vertex_index * 4
                per_loop += values[i:i + 4]
            values = per_loop
        col_vals, alpha_vals = [], []
        for i in range(0, len(values), 4):
            r, g, b, a = values[i:i + 4]
            col_vals += [srgb_to_linear(r), srgb_to_linear(g), srgb_to_linear(b), 1.0]
            la = srgb_to_linear(a)
            alpha_vals += [la, la, la, 1.0]
        col.data.foreach_set("color", col_vals)
        alpha.data.foreach_set("color", alpha_vals)

    mat = mesh.materials[0] if mesh.materials else None
    extras = dict(mat.items()) if mat is not None else {}
    obj.hm64_mk64_surface_type = extras.get("mk64_surface", "ASPHALT")
    obj.hm64_mk64_clip_type = extras.get("mk64_clip", "CLIP_DEFAULT")
    obj.hm64_mk64_draw_layer = extras.get("mk64_draw", "DRAW_OPAQUE")
    if mat is not None:
        mat["convert_preset"] = {
            "blend": "vertex_colored_texture_transparent",
            "cutout": "vertex_colored_texture_cutout",
        }.get(extras.get("mk64_alpha"), "vertex_colored_texture")
        # The glTF importer multiplies the texture by the vertex colours before
        # Base Color; Fast64's converter needs the image node linked directly.
        if mat.use_nodes:
            nodes = mat.node_tree.nodes
            image = next((n for n in nodes if n.type == "TEX_IMAGE"), None)
            bsdf = next((n for n in nodes if n.type == "BSDF_PRINCIPLED"), None)
            if image is not None and bsdf is not None:
                mat.node_tree.links.new(image.outputs["Color"], bsdf.inputs["Base Color"])

bpy.ops.object.select_all(action="DESELECT")
for obj in meshes:
    obj.select_set(True)
bpy.context.view_layer.objects.active = meshes[0]
scene.bsdf_conv_all = True
bpy.ops.object.convert_bsdf()

# ------------------------------------------------------- track root + path
root = bpy.data.objects.new(f"{export_name}_root", None)
scene.collection.objects.link(root)
root.hm64_mk64_obj_type = "Track Root"
for obj in meshes:
    obj.parent = root

curve_data = bpy.data.curves.new(f"{export_name}_path", type="CURVE")
curve_data.dimensions = "3D"
spline = curve_data.splines.new("BEZIER")
points = layout["path"]
spline.bezier_points.add(len(points) - 1)
for bp, p in zip(spline.bezier_points, points):
    bp.co = n64_to_blender(p)
    bp.handle_left_type = bp.handle_right_type = "VECTOR"
path_obj = bpy.data.objects.new(f"{export_name}_path", curve_data)
scene.collection.objects.link(path_obj)
path_obj.parent = root
path_obj.hm64_mk64_path_type = "TRACK_PATH_1"

blend_path = os.path.join(src_dir, f"{track}_mk64.blend")
bpy.ops.wm.save_as_mainfile(filepath=blend_path)
print(f"[n64kr] saved {blend_path}")

# ------------------------------------------------------------------ export
os.makedirs(mod_dir, exist_ok=True)
with open(os.path.join(mod_dir, "mods.toml"), "w", newline="\n") as f:
    f.write(f'[mod]\nname = "{mod_name}"\nversion = "0.1.0"\n')
bpy.ops.object.select_all(action="DESELECT")
root.select_set(True)
bpy.context.view_layer.objects.active = root
result = bpy.ops.scene.mk64_export_course()
print(f"[n64kr] export result {result} -> {mod_dir}")

# libultraship only supports LoadTile on G_TX_LOADTILE (7); anything else
# asserts in GfxDpLoadTile when the track is drawn. Fast64 emits it for
# textures that don't fit TMEM or aren't power-of-two.
import re  # noqa: E402

track_dir = os.path.join(mod_dir, "tracks", export_name)
bad = [f for f in os.listdir(track_dir) if f.startswith("mat_")
       and re.search(r'<LoadTile Tile="(?!7")', open(os.path.join(track_dir, f), errors="ignore").read())]
if bad:
    print(f"[n64kr] WARNING: materials load textures on a tile other than 7 (will crash in game): {bad}")
else:
    print("[n64kr] texture loads OK (no LoadTile outside tile 7)")
