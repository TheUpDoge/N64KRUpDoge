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

    with open(scene_path, "w", newline="\n") as f:
        json.dump(scene, f, indent=2)
    print(f"Wrote {scene_path}: '{display_name}', {len(scene['Actors'])} item boxes "
          f"({len(layout['item_boxes'])} rows of {ITEM_ROW}).")


if __name__ == "__main__":
    main()
