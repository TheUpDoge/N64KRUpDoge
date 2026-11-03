//! scene.json and minimap for a converted track (`n64kr dkr-track-scene`),
//! run after tools/n64kr/blender_dkr_track.py: name, resource id
//! (n64kr:<prefix>_<track>), item boxes, extra actors, music, sky colours and
//! the minimap. If the game already wrote a scene.json (first load of a track
//! without one, src/engine/TrackBrowser.cpp), its Props are kept and only the
//! N64KR fields are replaced; otherwise CustomTrack's defaults are used.
//!
//! Item boxes: each DKR balloon becomes a row of ITEM_ROW boxes across the
//! track, perpendicular to the path there. spawn_item_box() drops them onto
//! the surface below (src/racing/actors.c); they don't appear in time trials.

use std::path::Path;

use super::round_half_even;
use crate::json::{self, obj, py_round, Value};
use crate::png;

const ITEM_ROW: usize = 3;
const ITEM_SPACING: f64 = 18.0; // MK64 units between boxes in a row
const MINIMAP_SIZE: f64 = 80.0; // pixels on the long side
const MINIMAP_MARGIN: f64 = 4.0;
const MINIMAP_LINE: f64 = 2.2; // half-width of the drawn track line, pixels

/// CustomTrack defaults, as the game writes them (src/engine/tracks/CustomTrack.cpp).
fn default_props() -> Value {
    let f = Value::Float;
    let i = Value::Int;
    obj(vec![
        ("AIDistance", json::ints(&[20, 5, 10, 15, 20, 25, 30, 35, 30, 25, 45, 65, 90, 115, 140, 165,
                                    40, 3, 6, 16, 46, 49, 59, 89, 50, 30, 60, 63, 73, 78, 108, 138])),
        ("AIMaximumSeparation", f(50.0)),
        ("AIMinimumSeparation", f(0.30000001192092896)),
        ("AISteeringSensitivity", i(48)),
        ("CurveTargetSpeed", json::floats(&[4.166666507720947, 5.583333492279053, 6.166666507720947, 6.75])),
        ("D_0D0096B8", json::floats(&[3.3333332538604736, 3.9166667461395264, 4.5, 5.083333492279053])),
        ("DebugName", Value::Str("blnktrck".into())),
        ("FarPersp", f(6800.0)),
        ("LakituTowType", i(0)),
        ("MinimapColour", json::ints(&[255, 255, 255])),
        ("MinimapFinishlineX", f(0.0)),
        ("MinimapFinishlineY", f(0.0)),
        ("MinimapPlayerScaleFactor", f(0.2199999988079071)),
        ("MinimapPlayerX", i(0)),
        ("MinimapPlayerY", i(0)),
        ("MinimapPosition", json::ints(&[257, 170])),
        ("MinimapPosition2P", json::ints(&[0, 0])),
        ("Name", Value::Str("Blank Track".into())),
        ("NearPersp", f(3.0)),
        ("NormalTargetSpeed", json::floats(&[3.75, 5.166666507720947, 5.75, 6.333333492279053])),
        ("OffTrackTargetSpeed", json::floats(&[3.75, 5.166666507720947, 5.75, 6.333333492279053])),
        ("ResourceName", Value::Str("mods:blank_track".into())),
        ("Sequence", i(-1)),
        ("Skybox", json::ints(&[205; 24])),
        ("TrackLength", Value::Str("100m".into())),
        ("WaterLevel", f(3.4028234663852886e+38)),
    ])
}

fn points(v: Option<&Value>) -> Vec<[f64; 3]> {
    v.and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|p| p.as_array().map(|c| [0, 1, 2].map(|k| c[k].as_f64().unwrap_or(0.0)))).collect())
        .unwrap_or_default()
}

/// Unit XZ direction of the path segment nearest (x, z).
fn path_direction(path: &[[f64; 3]], x: f64, z: f64) -> (f64, f64) {
    let (mut best, mut best_d) = (0usize, f64::INFINITY);
    for (i, p) in path.iter().enumerate() {
        let d = (p[0] - x).powi(2) + (p[2] - z).powi(2);
        if d < best_d {
            (best, best_d) = (i, d);
        }
    }
    let (a, b) = (path[best], path[(best + 1) % path.len()]);
    let (dx, dz) = (b[0] - a[0], b[2] - a[2]);
    let mut length = dx.hypot(dz);
    if length == 0.0 {
        length = 1.0;
    }
    (dx / length, dz / length)
}

fn item_boxes(path: &[[f64; 3]], boxes: &[[f64; 3]]) -> Vec<Value> {
    let mut actors = vec![];
    for &[x, y, z] in boxes {
        let (dx, dz) = path_direction(path, x, z);
        let (side_x, side_z) = (-dz, dx); // perpendicular to the path
        for k in 0..ITEM_ROW {
            let offset = (k as f64 - (ITEM_ROW as f64 - 1.0) / 2.0) * ITEM_SPACING;
            actors.push(obj(vec![
                ("Name", Value::Str("mk:item_box".into())),
                ("Location", obj(vec![
                    ("x", Value::Float(py_round(x + side_x * offset, 2))),
                    ("y", Value::Float(py_round(y, 2))),
                    ("z", Value::Float(py_round(z + side_z * offset, 2))),
                ])),
            ]));
        }
    }
    actors
}

/// Top-down track line as a greyscale PNG, plus the Props that place the
/// racer dots on it. The game draws a dot at image top-left + (PlayerX,
/// PlayerY) + (pos.x, pos.z) x PlayerScaleFactor (src/render_objects.c
/// draw_minimap_character), so the image uses the same mapping. The PNG is
/// read as one channel and packed to 4 bits (MinimapFactory.cpp).
fn minimap(path: &[[f64; 3]]) -> (Vec<u8>, Vec<(&'static str, Value)>, (usize, usize)) {
    let xs: Vec<f64> = path.iter().map(|p| p[0]).collect();
    let zs: Vec<f64> = path.iter().map(|p| p[2]).collect();
    let (xmin, xmax) = (xs.iter().cloned().fold(f64::INFINITY, f64::min), xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max));
    let (zmin, zmax) = (zs.iter().cloned().fold(f64::INFINITY, f64::min), zs.iter().cloned().fold(f64::NEG_INFINITY, f64::max));
    let span = (xmax - xmin).max(zmax - zmin);
    let scale = (MINIMAP_SIZE - 2.0 * MINIMAP_MARGIN) / span;
    let mut width = ((xmax - xmin) * scale) as usize + 2 * MINIMAP_MARGIN as usize;
    let height = ((zmax - zmin) * scale) as usize + 2 * MINIMAP_MARGIN as usize;
    // 4-bit (I4) textures need a width that's a multiple of 16 texels, or each
    // row is fetched from the wrong place and the minimap comes out striped.
    width = width.div_ceil(16) * 16;
    let off_x = (width as f64 - (xmax - xmin) * scale) / 2.0 - xmin * scale; // centred in the padded width
    let off_z = MINIMAP_MARGIN - zmin * scale;
    let pts: Vec<(f64, f64)> = xs.iter().zip(&zs).map(|(x, z)| (x * scale + off_x, z * scale + off_z)).collect();
    let mut pixels = vec![0u8; width * height];
    for i in 0..pts.len() {
        let ((ax, ay), (bx, by)) = (pts[i], pts[(i + 1) % pts.len()]);
        let y0 = ((ay.min(by) - 3.0) as i64).max(0);
        let y1 = ((ay.max(by) + 4.0) as i64).min(height as i64);
        let x0 = ((ax.min(bx) - 3.0) as i64).max(0);
        let x1 = ((ax.max(bx) + 4.0) as i64).min(width as i64);
        for y in y0..y1 {
            for x in x0..x1 {
                let (dx, dy) = (bx - ax, by - ay);
                let t = if dx == 0.0 && dy == 0.0 { 0.0 } else { (((x as f64 - ax) * dx + (y as f64 - ay) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0) };
                let d = (x as f64 - (ax + t * dx)).hypot(y as f64 - (ay + t * dy));
                if d <= MINIMAP_LINE {
                    pixels[y as usize * width + x as usize] = 255;
                }
            }
        }
    }
    let props = vec![
        ("MinimapPlayerX", Value::Int(round_half_even(off_x))),
        ("MinimapPlayerY", Value::Int(round_half_even(off_z))),
        ("MinimapPlayerScaleFactor", Value::Float(scale)),
        ("MinimapFinishlineX", Value::Float(0.0)),
        ("MinimapFinishlineY", Value::Float(-2.0)),
    ];
    (png::encode_grey(width, height, &pixels), props, (width, height))
}

/// `n64kr dkr-track-scene <converted dir> <mods dir> <track> <display name> [prefix]`
pub fn write(src_dir: &str, mods_dir: &str, track: &str, display_name: &str, prefix: &str) -> Result<(), String> {
    let layout_path = Path::new(src_dir).join(format!("{track}_mk64.json"));
    let layout = json::parse(&std::fs::read_to_string(&layout_path).map_err(|e| format!("{}: {e}", layout_path.display()))?)?;
    let mod_name = format!("n64kr-{prefix}-{}", track.replace('_', "-"));
    let track_dir = Path::new(mods_dir).join(mod_name).join("tracks").join(format!("{prefix}_{track}"));
    if !track_dir.is_dir() {
        return Err(format!("{} not found; run blender_dkr_track.py first", track_dir.display()));
    }
    let scene_path = track_dir.join("scene.json");
    let mut scene = if scene_path.exists() {
        json::parse(&std::fs::read_to_string(&scene_path).map_err(|e| e.to_string())?)?
    } else {
        Value::Obj(vec![])
    };

    let mut props = default_props();
    if let Some(Value::Obj(existing)) = scene.get("Props") {
        for (k, v) in existing.clone() {
            props.set(&k, v);
        }
    }
    props.set("Name", Value::Str(display_name.into()));
    props.set("DebugName", Value::Str(format!("{prefix} {track}").chars().take(15).collect()));
    props.set("ResourceName", Value::Str(format!("n64kr:{prefix}_{track}")));
    // Keys in the order the Python tool wrote them (Props first in a new file);
    // Props and Actors get their final values below.
    scene.set("Props", Value::Null);
    if scene.get("Fog").is_none() {
        scene.set("Fog", obj(vec![("EnableFog", Value::Bool(false))]));
    }
    if scene.get("StaticMeshActors").is_none() {
        scene.set("StaticMeshActors", Value::Null);
    }
    scene.set("Actors", Value::Null);
    let path = points(layout.get("path"));
    let boxes = points(layout.get("item_boxes"));
    let mut actors = item_boxes(&path, &boxes);
    let n_boxes = actors.len();
    // Extra actors from the layout, as written (N64KR's GoldenEye barrels and
    // guards, src/n64kr/ge_hazards.cpp), and the track's music: a sequence id,
    // or an N64KR song id played through HMAS (src/n64kr/music.cpp).
    let extra: Vec<Value> = layout.get("actors").and_then(Value::as_array).cloned().unwrap_or_default();
    actors.extend(extra.iter().cloned());
    if let Some(seq) = layout.get("sequence").and_then(Value::as_f64) {
        props.set("Sequence", Value::Int(seq as i64));
    }
    // DKR tracks have their own start banner, so MK64's finish line arch is off
    // unless the layout asks for it (built tracks like Dam Speedway). Read by
    // the N64KR hook in src/engine/editor/SceneManager.cpp.
    let finishline = matches!(layout.get("spawn_finishline"), Some(Value::Bool(true)));

    if let Some(sky) = layout.get("sky").filter(|s| !matches!(s, Value::Null)) {
        let colour = |k: &str| -> Vec<i64> {
            sky.get(k).and_then(Value::as_array).map(|a| a.iter().map(|v| v.as_f64().unwrap_or(0.0) as i64).collect()).unwrap_or_default()
        };
        let (top, horizon) = (colour("top"), colour("horizon"));
        let floor: Vec<i64> = horizon.iter().map(|c| c.div_euclid(2)).collect();
        // SkyboxColours order: TopRight, BottomRight, BottomLeft, TopLeft, then the
        // four floor colours (src/engine/tracks/Track.h).
        let skybox: Vec<i64> = [&top, &horizon, &horizon, &top, &horizon, &floor, &floor, &horizon].into_iter().flatten().copied().collect();
        props.set("Skybox", json::ints(&skybox));
        println!("Sky from {}: top {top:?}, horizon {horizon:?}", sky.get("dome").and_then(Value::as_str).unwrap_or("layout"));
    }

    let (png, minimap_props, (w, h)) = minimap(&path);
    std::fs::write(track_dir.join("minimap.png"), png).map_err(|e| e.to_string())?;
    let scale = minimap_props[2].1.as_f64().unwrap();
    for (k, v) in minimap_props {
        props.set(k, v);
    }
    println!("Wrote minimap.png {w}x{h}, scale {scale:.4}");

    let sequence = props.get("Sequence").cloned().unwrap_or(Value::Int(-1));
    scene.set("Props", props);
    scene.set("Actors", Value::Arr(actors));
    scene.set("SpawnFinishline", Value::Bool(finishline));
    std::fs::write(&scene_path, scene.to_string_indent(2)).map_err(|e| e.to_string())?;
    println!(
        "Wrote {}: '{display_name}', {n_boxes} item boxes ({} rows of {ITEM_ROW}), {} other actors, sequence {}.",
        scene_path.display(),
        boxes.len(),
        extra.len(),
        match sequence {
            Value::Int(i) => i.to_string(),
            v => v.to_string_indent(0),
        }
    );
    Ok(())
}
