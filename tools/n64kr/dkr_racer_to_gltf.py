"""Export a DKR racer (driver + vehicle) from the user's own DKR ROM.

Writes, under <out dir>/<object>/ (ROM data: keep under build/, never commit):
  <object>.gltf/.bin + textures/   body model (highest detail), DKR units, Y up;
                                    animation 0 (driving) frames as morph targets,
                                    plus horn and crash frames (EXTRA_ANIMS);
                                    later eye frames as dkr_texNNNN_fK.png
  wheel_<part>_<frame>.png          vehicle part sprite frames (wheel spin)
  <object>_racer.json               wheel attach points, part scales, sprite anchors

Usage (from the repo root):
    python -I tools/n64kr/dkr_racer_to_gltf.py "<DKR .z64>" --list
    python -I tools/n64kr/dkr_racer_to_gltf.py "<DKR .z64>" diddycar build/dkr_racers

Format notes (DKR decomp a5eb080):
- Racer objects have behaviour BHV_RACER (1) and model type 3D; their header
  lists 6 models, a detail series; model 0 is the most detailed.
- ObjectModel (include/structs.h): textures @0x00, vertices @0x04, triangles
  @0x08, attachPoints @0x14 (s16 vertex indices, count @0x18), counts @0x22..,
  batches @0x38. Offsets are relative to the model start (src/object_models.c).
- Header vehiclePartIds @0x14 / vehiclePartIndices @0x18 (attachPointCount @0x56):
  one vehicle part object per attach point (wheels). Parts are sprite objects
  (model type 2) whose sprite frames animate the wheel.
- Vertex animation: ASSET_ANIMATION_IDS (s16 per model) gives the model's range
  in ASSET_OBJECT_ANIMATIONS (model_anim_init, src/object_models.c). Each one is
  compressed; s32 keyframe count, then keyframe 0 (12-byte header + s16 xyz
  offset per animated vertex, added to the base vertex), then keyframes 1.. at
  stride 12 + 3 * animated count (12-byte header + s8 xyz deltas, cumulative).
  A frame is 1/16 keyframe; the fraction scales the next keyframe's delta.
  Model 0x4A = animated vertex count, 0x4C = s16 animated index per vertex (-1 =
  static). Read from obj_animate (src/hasm/obj_animate.s).
- Racers drive with animation 0, frame 40 - steerAngle / 2 clamped 0..73
  (src/racer.c): 40 straight, below 40 steering right, above 40 left.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))  # python -I drops the script dir
import dkr_level_to_gltf as dkr  # noqa: E402

BHV_RACER = 1
# Extra animations exported as shape keys (animation index: frames used),
# from DKR's racer actions (src/racer.c func_80052988 calls): 4 = horn (Z
# button taunt, 48 frames), 3 = crash (32 frames).
EXTRA_ANIMS = {4: 48, 3: 32}
EXTRA_ANIM_STEP = 4


def racer_headers(rom):
    return [(i, h) for i, h in enumerate(dkr.object_headers(rom)) if h["behaviour"] == BHV_RACER]


def animation_frames(rom, model_id, m, anim_index=0):
    """Vertex positions of every frame of one model animation: a list (per frame)
    of {vertex index: (x, y, z)} for the animated vertices; [] if none."""
    ids = rom.section("ASSET_ANIMATION_IDS")
    start, end = dkr.s16(ids, 2 * model_id), dkr.s16(ids, 2 * model_id + 2)
    count, v_off = dkr.s16(m, 0x4A), dkr.u32(m, 0x04)
    if anim_index >= end - start or count < 1:
        return []
    table = rom.table("ASSET_OBJECT_ANIMATIONS_TABLE")
    a = start + anim_index
    data = dkr.dkr_inflate(rom.section("ASSET_OBJECT_ANIMATIONS")[table[a]:table[a + 1]])
    keyframes, anim = dkr.s32(data, 0), data[4:]
    index_off = dkr.u32(m, 0x4C)
    slots = {}  # vertex index -> animated slot
    for v in range(dkr.s16(m, 0x24)):
        slot = dkr.s16(m, index_off + 2 * v)
        if slot >= 0:
            slots[v] = slot
    pos = [None] * count
    for v, slot in slots.items():  # keyframe 0: base vertex + s16 offset
        vo = v_off + v * dkr.VERTEX_SIZE
        pos[slot] = [dkr.s16(m, vo + k * 2) + dkr.s16(anim, 12 + slot * 6 + k * 2) for k in range(3)]
    stride = 12 + 3 * count

    def delta(keyframe, slot, k):  # keyframes 1..: s8 deltas
        b = anim[stride * (keyframe + 1) + slot * 3 + k]
        return b - 256 if b > 127 else b

    frames = []
    for keyframe in range(keyframes - 1):
        for t in range(16):
            cur = [[p[k] + (delta(keyframe + 1, s, k) * t >> 4) for k in range(3)] for s, p in enumerate(pos)]
            frames.append({v: tuple(cur[slot]) for v, slot in slots.items()})
        pos = [[p[k] + delta(keyframe + 1, s, k) for k in range(3)] for s, p in enumerate(pos)]
    return frames


def load_racer(rom, object_name):
    """Parse a racer from the ROM: body model 0 (triangle corners grouped by
    material), its driving animation frames, and its wheels (attach position,
    part scale, sprite frames as images). Shared by the glTF export and the
    in-game model export (dkr_racer_to_model.py)."""
    headers = dkr.object_headers(rom)
    index = next((i for i, h in enumerate(headers) if h["name"] == object_name and h["behaviour"] == BHV_RACER), None)
    if index is None:
        sys.exit(f"No racer object '{object_name}'. Use --list.")
    hd = headers[index]
    objs = rom.section("ASSET_OBJECTS")
    otable = rom.table("ASSET_OBJECT_HEADERS_TABLE")
    raw_header = objs[otable[index]:otable[index + 1]]

    models = rom.section("ASSET_OBJECT_MODELS")
    mtable = rom.table("ASSET_OBJECT_MODELS_TABLE")
    model_id = hd["models"][0]
    m = dkr.dkr_inflate(models[mtable[model_id]:mtable[model_id + 1]])
    tex_off, v_off, t_off = dkr.u32(m, 0x00), dkr.u32(m, 0x04), dkr.u32(m, 0x08)
    attach_off, attach_count = dkr.u32(m, 0x14), dkr.s16(m, 0x18)
    num_tex, b_off, num_batches = dkr.s16(m, 0x22), dkr.u32(m, 0x38), dkr.s16(m, 0x28)

    textures = []
    for t in range(num_tex):
        o = tex_off + t * 8
        textures.append({"id": dkr.u32(m, o) & 0x7FFF, "w": m[o + 4], "h": m[o + 5], "surface": 0})

    groups, corner_vertex = {}, {}
    for b in range(num_batches):
        bo = b_off + b * dkr.BATCH_SIZE
        tex_index = m[bo]
        v_base = dkr.s16(m, bo + 2)
        f_start, f_end = dkr.s16(m, bo + 4), dkr.s16(m, bo + dkr.BATCH_SIZE + 4)
        flags = dkr.u32(m, bo + 8)
        if flags & dkr.RENDER_HIDDEN:
            continue
        vertex_colours = m[bo + 6] == 0xFF  # otherwise lit by normals in DKR
        key = (None if tex_index == 0xFF or tex_index >= num_tex else tex_index, "visible", False, True,
               bool(flags & (dkr.RENDER_SEMI_TRANSPARENT | dkr.RENDER_CUTOUT)))
        corners = groups.setdefault(key, [])
        vertex_ids = corner_vertex.setdefault(key, [])
        for f in range(f_start, f_end):
            tr = t_off + f * dkr.TRIANGLE_SIZE
            for c in range(3):
                vertex_ids.append(v_base + m[tr + 1 + c])
                vo = v_off + vertex_ids[-1] * dkr.VERTEX_SIZE
                r, g, bl, a = m[vo + 6:vo + 10] if vertex_colours else (0xE0, 0xE0, 0xE0, 0xFF)
                corners.append((dkr.s16(m, vo), dkr.s16(m, vo + 2), dkr.s16(m, vo + 4), r, g, bl, a,
                                dkr.s16(m, tr + 4 + c * 4), dkr.s16(m, tr + 6 + c * 4)))

    # Wheels: one vehicle part per attach point, positioned at the attach vertex.
    attach_vertices = [dkr.s16(m, attach_off + 2 * k) for k in range(max(0, attach_count))]
    part_count = raw_header[0x56]
    ids_off, idx_off = dkr.u32(raw_header, 0x14), dkr.u32(raw_header, 0x18)
    part_ids = [dkr.s32(raw_header, ids_off + 4 * k) for k in range(part_count)] if 0 < ids_off < len(raw_header) else []
    part_attach = list(raw_header[idx_off:idx_off + part_count]) if 0 < idx_off < len(raw_header) else []
    wheels = []
    for k, part_id in enumerate(part_ids):
        if not (0 <= part_id < len(headers)) or k >= len(part_attach) or part_attach[k] >= len(attach_vertices):
            continue
        part = headers[part_id]
        vo = v_off + attach_vertices[part_attach[k]] * dkr.VERTEX_SIZE
        # Each sprite is one wheel-spin state (picked by the part's modelIndex);
        # its frames are the wheel seen from 0 deg (head-on, frame 0) to 90 deg
        # (side on, last frame) round from the vehicle's vertical plane
        # (render_sprite_billboard in the decomp's src/camera.c).
        spins = []
        for sprite_id in part["models"]:
            sprite = rom.section("ASSET_SPRITES")[rom.table("ASSET_SPRITES_TABLE")[sprite_id]:]
            spins.append([dkr.sprite_image(rom, sprite_id, angle) for angle in range(max(1, dkr.s16(sprite, 2)))])
        # DKR steers attach points 2 and 3 (y_rotation = stick x * 100, src/racer.c
        # update_car_wheels); 0 and 1 stay straight.
        wheels.append({"part": part["name"], "scale": part["scale"], "steers": k >= 2,
                       "position": [dkr.s16(m, vo), dkr.s16(m, vo + 2), dkr.s16(m, vo + 4)], "spins": spins})

    return {"header": hd, "model_id": model_id, "model": m, "textures": textures, "groups": groups,
            "corner_vertex": corner_vertex, "anim_frames": animation_frames(rom, model_id, m), "wheels": wheels}


def export(rom, object_name, out_root):
    racer = load_racer(rom, object_name)
    groups, textures, frames = racer["groups"], racer["textures"], racer["anim_frames"]

    # Morph targets (shape keys), each the offset of every corner from the
    # base pose: every frame of the driving animation, and every
    # EXTRA_ANIM_STEP-th frame of the extra animations (select-screen faces,
    # blender_dkr_racer_faces.py).
    named = [(f"anim0_f{i:02d}", f) for i, f in enumerate(frames)]
    extra = {}
    for anim, length in EXTRA_ANIMS.items():
        anim_frames = animation_frames(rom, racer["model_id"], racer["model"], anim)[:length]
        picked = list(range(0, len(anim_frames), EXTRA_ANIM_STEP))
        named += [(f"anim{anim}_f{i:02d}", anim_frames[i]) for i in picked]
        extra[str(anim)] = picked
    targets = {}
    for key, corners in groups.items():
        targets[key] = [[tuple(f[v][k] - c[k] for k in range(3)) if v in f else (0.0, 0.0, 0.0)
                         for v, c in zip(racer["corner_vertex"][key], corners)] for _, f in named]
    out_dir = os.path.join(out_root, object_name)
    dkr.write_gltf(rom, out_dir, object_name, groups, textures, targets if named else None,
                   [name for name, _ in named])

    # Later frames of animated textures (the eyes: open .. shut), next to the
    # first frame write_gltf saved.
    tex3d, tex3d_table = rom.section("ASSET_TEXTURES_3D"), rom.table("ASSET_TEXTURES_3D_TABLE")
    texture_frames = {}
    for info in textures:
        raw = tex3d[tex3d_table[info["id"]]:tex3d_table[info["id"] + 1]]
        count = dkr.texture_frames(raw)
        if count < 2 or str(info["id"]) in texture_frames:
            continue
        texture_frames[str(info["id"])] = count
        for frame in range(1, count):
            w, h, rgba, _flags, _fmt = dkr.decode_texture(raw, frame)
            with open(os.path.join(out_dir, "textures", f"dkr_tex{info['id']:04d}_f{frame}.png"), "wb") as fh:
                fh.write(dkr.png_bytes(w, h, rgba))

    wheels, written = [], set()
    for wheel in racer["wheels"]:
        spin_files = []
        for spin, images in enumerate(wheel["spins"]):
            angles = []
            for angle, (w, h, rgba, left, top) in enumerate(images):
                file = f"wheel_{wheel['part']}_s{spin}_a{angle:02d}.png"
                if file not in written:
                    with open(os.path.join(out_dir, file), "wb") as fh:
                        fh.write(dkr.png_bytes(w, h, rgba))
                    written.add(file)
                angles.append({"file": file, "w": w, "h": h, "left": left, "top": top})
            spin_files.append(angles)
        wheels.append({"part": wheel["part"], "scale": wheel["scale"], "steers": wheel["steers"],
                       "position": wheel["position"], "frames": spin_files})

    info = {"object": object_name, "model": racer["model_id"], "object_scale": racer["header"]["scale"],
            "drive_animation": {"frames": len(frames), "straight": 40, "full_right": 0, "full_left": 73,
                                "shape_key": "anim0_fNN"},
            "extra_animations": {"frames": extra, "shape_key": "anim<A>_fNN", "4": "horn", "3": "crash"},
            "texture_frames": texture_frames,
            "units": "DKR model units, Y up (same as the glTF)", "wheels": wheels}
    with open(os.path.join(out_dir, f"{object_name}_racer.json"), "w") as f:
        json.dump(info, f, indent=1)
    for old in os.listdir(out_dir):  # drop single-frame wheel images from older exports
        if old.startswith("wheel_") and old not in written:
            os.remove(os.path.join(out_dir, old))
    print(f"Racer {object_name}: model {racer['model_id']}, {sum(len(c) for c in groups.values()) // 3} triangles, "
          f"{len(textures)} textures, {len(wheels)} wheels ({len(written)} wheel frame images), "
          f"{len(frames)} animation frames.")


def main():
    args = sys.argv[1:]
    if len(args) >= 2 and args[1] == "--list":
        rom = dkr.Rom(args[0])
        for i, h in racer_headers(rom):
            print(f"{h['name']:16s} models {h['models']}")
        return
    if len(args) != 3:
        sys.exit(__doc__)
    export(dkr.Rom(args[0]), args[1], args[2])


if __name__ == "__main__":
    main()
