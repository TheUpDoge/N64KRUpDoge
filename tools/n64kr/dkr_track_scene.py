"""Write scene.json for a converted DKR track: name, resource id and item boxes.

Run after blender_dkr_track.py:
    python -I tools/n64kr/dkr_track_scene.py build/dkr_tracks/ancient_lake_mk64 build/x64/mods ancient_lake "Ancient Lake"

If the game already generated a scene.json (first load of a track without
one, src/engine/TrackBrowser.cpp), its Props are kept and only the N64KR
fields are replaced; otherwise the CustomTrack defaults below are used.

Item boxes: each DKR WeaponBalloon becomes a row of ITEM_ROW boxes across
the track, perpendicular to the path at that point. spawn_item_box() drops
them onto the surface below (src/racing/actors.c), and they don't appear in
Time Trials.
"""
import json
import math
import os
import sys

ITEM_ROW = 3
ITEM_SPACING = 18.0  # MK64 units between boxes in a row

# CustomTrack defaults, as the game writes them (src/engine/tracks/CustomTrack.cpp).
DEFAULT_PROPS = {
    "AIDistance": [20, 5, 10, 15, 20, 25, 30, 35, 30, 25, 45, 65, 90, 115, 140, 165,
                   40, 3, 6, 16, 46, 49, 59, 89, 50, 30, 60, 63, 73, 78, 108, 138],
    "AIMaximumSeparation": 50.0,
    "AIMinimumSeparation": 0.30000001192092896,
    "AISteeringSensitivity": 48,
    "CurveTargetSpeed": [4.166666507720947, 5.583333492279053, 6.166666507720947, 6.75],
    "D_0D0096B8": [3.3333332538604736, 3.9166667461395264, 4.5, 5.083333492279053],
    "DebugName": "blnktrck",
    "FarPersp": 6800.0,
    "LakituTowType": 0,
    "MinimapColour": [255, 255, 255],
    "MinimapFinishlineX": 0.0,
    "MinimapFinishlineY": 0.0,
    "MinimapPlayerScaleFactor": 0.2199999988079071,
    "MinimapPlayerX": 0,
    "MinimapPlayerY": 0,
    "MinimapPosition": [257, 170],
    "MinimapPosition2P": [0, 0],
    "Name": "Blank Track",
    "NearPersp": 3.0,
    "NormalTargetSpeed": [3.75, 5.166666507720947, 5.75, 6.333333492279053],
    "OffTrackTargetSpeed": [3.75, 5.166666507720947, 5.75, 6.333333492279053],
    "ResourceName": "mods:blank_track",
    "Sequence": -1,
    "Skybox": [205] * 24,
    "TrackLength": "100m",
    "WaterLevel": 3.4028234663852886e+38,
}


def path_direction(path, x, z):
    """Unit XZ direction of the path segment nearest (x, z)."""
    best, best_d = 0, float("inf")
    for i, p in enumerate(path):
        d = (p[0] - x) ** 2 + (p[2] - z) ** 2
        if d < best_d:
            best, best_d = i, d
    a, b = path[best], path[(best + 1) % len(path)]
    dx, dz = b[0] - a[0], b[2] - a[2]
    length = math.hypot(dx, dz) or 1.0
    return dx / length, dz / length


def item_boxes(layout):
    actors = []
    for x, y, z in layout["item_boxes"]:
        dx, dz = path_direction(layout["path"], x, z)
        side_x, side_z = -dz, dx  # perpendicular to the path
        for k in range(ITEM_ROW):
            offset = (k - (ITEM_ROW - 1) / 2.0) * ITEM_SPACING
            actors.append({"Name": "mk:item_box",
                           "Location": {"x": round(x + side_x * offset, 2), "y": round(y, 2),
                                        "z": round(z + side_z * offset, 2)}})
    return actors


MINIMAP_SIZE = 80     # pixels on the long side
MINIMAP_MARGIN = 4
MINIMAP_LINE = 2.2    # half-width of the drawn track line, pixels


def png_grey(width, height, pixels):
    import struct
    import zlib

    def chunk(tag, payload):
        return struct.pack(">I", len(payload)) + tag + payload + struct.pack(">I", zlib.crc32(tag + payload) & 0xFFFFFFFF)

    rows = b"".join(b"\0" + bytes(pixels[y * width:(y + 1) * width]) for y in range(height))
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 0, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(rows, 9)) + chunk(b"IEND", b""))


def minimap(layout):
    """Top-down track line as a greyscale PNG, plus the Props that place the
    racer dots on it. The game draws a dot at
    image top-left + (PlayerX, PlayerY) + (pos.x, pos.z) * PlayerScaleFactor
    (src/render_objects.c draw_minimap_character), so the image uses the
    same mapping. The PNG is read as 1-channel and packed to 4 bits
    (src/port/resource/importers/MinimapFactory.cpp)."""
    xs = [p[0] for p in layout["path"]]
    zs = [p[2] for p in layout["path"]]
    span = max(max(xs) - min(xs), max(zs) - min(zs))
    scale = (MINIMAP_SIZE - 2 * MINIMAP_MARGIN) / span
    width = int((max(xs) - min(xs)) * scale) + 2 * MINIMAP_MARGIN
    height = int((max(zs) - min(zs)) * scale) + 2 * MINIMAP_MARGIN
    # 4-bit (I4) textures need a width that's a multiple of 16 texels, or each
    # row is fetched from the wrong place and the minimap comes out striped.
    width = (width + 15) // 16 * 16
    off_x = (width - (max(xs) - min(xs)) * scale) / 2.0 - min(xs) * scale  # centred in the padded width
    off_z = MINIMAP_MARGIN - min(zs) * scale
    pts = [(x * scale + off_x, z * scale + off_z) for x, z in zip(xs, zs)]
    pixels = [0] * (width * height)
    for i in range(len(pts)):
        (ax, ay), (bx, by) = pts[i], pts[(i + 1) % len(pts)]
        for y in range(max(0, int(min(ay, by) - 3)), min(height, int(max(ay, by) + 4))):
            for x in range(max(0, int(min(ax, bx) - 3)), min(width, int(max(ax, bx) + 4))):
                dx, dy = bx - ax, by - ay
                t = 0.0 if dx == dy == 0 else max(0.0, min(1.0, ((x - ax) * dx + (y - ay) * dy) / (dx * dx + dy * dy)))
                d = math.hypot(x - (ax + t * dx), y - (ay + t * dy))
                if d <= MINIMAP_LINE:
                    pixels[y * width + x] = 255
    props = {"MinimapPlayerX": int(round(off_x)), "MinimapPlayerY": int(round(off_z)),
             "MinimapPlayerScaleFactor": scale, "MinimapFinishlineX": 0.0, "MinimapFinishlineY": -2.0}
    return png_grey(width, height, pixels), props, (width, height)


def main():
    if len(sys.argv) != 5:
        sys.exit(__doc__)
    src_dir, mods_dir, track, display_name = sys.argv[1:]
    layout = json.load(open(os.path.join(src_dir, f"{track}_mk64.json")))
    mod_name = f"n64kr-dkr-{track.replace('_', '-')}"
    track_dir = os.path.join(mods_dir, mod_name, "tracks", f"dkr_{track}")
    if not os.path.isdir(track_dir):
        sys.exit(f"{track_dir} not found; run blender_dkr_track.py first.")
    scene_path = os.path.join(track_dir, "scene.json")
    scene = json.load(open(scene_path)) if os.path.exists(scene_path) else {}

    props = dict(DEFAULT_PROPS)
    props.update(scene.get("Props") or {})
    props["Name"] = display_name
    props["DebugName"] = f"dkr {track}"[:15]
    props["ResourceName"] = f"n64kr:dkr_{track}"
    scene["Props"] = props
    scene.setdefault("Fog", {"EnableFog": False})
    scene.setdefault("StaticMeshActors", None)
    scene["Actors"] = item_boxes(layout)
    # DKR tracks have their own start banner; skip MK64's finish line arch
    # (read by the N64KR hook in src/engine/editor/SceneManager.cpp).
    scene["SpawnFinishline"] = False

    sky = layout.get("sky")
    if sky:
        top, horizon = sky["top"], sky["horizon"]
        floor = [c // 2 for c in horizon]
        # SkyboxColours order: TopRight, BottomRight, BottomLeft, TopLeft, then the
        # four floor colours (src/engine/tracks/Track.h).
        props["Skybox"] = top + horizon + horizon + top + horizon + floor + floor + horizon
        print(f"Sky from DKR '{sky['dome']}': top {top}, horizon {horizon}")

    png, minimap_props, size = minimap(layout)
    with open(os.path.join(track_dir, "minimap.png"), "wb") as f:
        f.write(png)
    props.update(minimap_props)
    print(f"Wrote minimap.png {size[0]}x{size[1]}, scale {minimap_props['MinimapPlayerScaleFactor']:.4f}")

    with open(scene_path, "w", newline="\n") as f:
        json.dump(scene, f, indent=2)
    print(f"Wrote {scene_path}: '{display_name}', {len(scene['Actors'])} item boxes "
          f"({len(layout['item_boxes'])} rows of {ITEM_ROW}).")


if __name__ == "__main__":
    main()
