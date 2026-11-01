"""Export a DKR racer (driver + vehicle) from the user's own DKR ROM.

Writes, under <out dir>/<object>/ (ROM data: keep under build/, never commit):
  <object>.gltf/.bin + textures/   body model (highest detail), DKR units, Y up
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
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))  # python -I drops the script dir
import dkr_level_to_gltf as dkr  # noqa: E402

BHV_RACER = 1


def racer_headers(rom):
    return [(i, h) for i, h in enumerate(dkr.object_headers(rom)) if h["behaviour"] == BHV_RACER]


def export(rom, object_name, out_root):
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

    groups = {}
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
        for f in range(f_start, f_end):
            tr = t_off + f * dkr.TRIANGLE_SIZE
            for c in range(3):
                vo = v_off + (v_base + m[tr + 1 + c]) * dkr.VERTEX_SIZE
                r, g, bl, a = m[vo + 6:vo + 10] if vertex_colours else (0xE0, 0xE0, 0xE0, 0xFF)
                corners.append((dkr.s16(m, vo), dkr.s16(m, vo + 2), dkr.s16(m, vo + 4), r, g, bl, a,
                                dkr.s16(m, tr + 4 + c * 4), dkr.s16(m, tr + 6 + c * 4)))

    out_dir = os.path.join(out_root, object_name)
    dkr.write_gltf(rom, out_dir, object_name, groups, textures)

    # Wheels: one vehicle part per attach point, positioned at the attach vertex.
    attach_vertices = [dkr.s16(m, attach_off + 2 * k) for k in range(max(0, attach_count))]
    part_count = raw_header[0x56]
    ids_off, idx_off = dkr.u32(raw_header, 0x14), dkr.u32(raw_header, 0x18)
    part_ids = [dkr.s32(raw_header, ids_off + 4 * k) for k in range(part_count)] if 0 < ids_off < len(raw_header) else []
    part_attach = list(raw_header[idx_off:idx_off + part_count]) if 0 < idx_off < len(raw_header) else []
    wheels, written = [], set()
    for k, part_id in enumerate(part_ids):
        if not (0 <= part_id < len(headers)) or k >= len(part_attach) or part_attach[k] >= len(attach_vertices):
            continue
        part = headers[part_id]
        vo = v_off + attach_vertices[part_attach[k]] * dkr.VERTEX_SIZE
        # Each sprite is one wheel-spin state (picked by the part's modelIndex);
        # its frames are the wheel seen from 0 deg (head-on, frame 0) to 90 deg
        # (side on, last frame) round from the vehicle's vertical plane
        # (render_sprite_billboard in the decomp's src/camera.c).
        frames = []
        for spin, sprite_id in enumerate(part["models"]):
            sprite = rom.section("ASSET_SPRITES")[rom.table("ASSET_SPRITES_TABLE")[sprite_id]:]
            angles = []
            for angle in range(max(1, dkr.s16(sprite, 2))):
                w, h, rgba, left, top = dkr.sprite_image(rom, sprite_id, angle)
                file = f"wheel_{part['name']}_s{spin}_a{angle:02d}.png"
                if file not in written:
                    with open(os.path.join(out_dir, file), "wb") as fh:
                        fh.write(dkr.png_bytes(w, h, rgba))
                    written.add(file)
                angles.append({"file": file, "w": w, "h": h, "left": left, "top": top})
            frames.append(angles)
        wheels.append({"part": part["name"], "scale": part["scale"],
                       "position": [dkr.s16(m, vo), dkr.s16(m, vo + 2), dkr.s16(m, vo + 4)], "frames": frames})

    info = {"object": object_name, "model": model_id, "object_scale": hd["scale"],
            "units": "DKR model units, Y up (same as the glTF)", "wheels": wheels}
    with open(os.path.join(out_dir, f"{object_name}_racer.json"), "w") as f:
        json.dump(info, f, indent=1)
    for old in os.listdir(out_dir):  # drop single-frame wheel images from older exports
        if old.startswith("wheel_") and old not in written:
            os.remove(os.path.join(out_dir, old))
    print(f"Racer {object_name}: model {model_id}, {sum(len(c) for c in groups.values()) // 3} triangles, "
          f"{len(textures)} textures, {len(wheels)} wheels ({len(written)} wheel frame images).")


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
