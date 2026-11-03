//! DKR tracks to glTF (`n64kr dkr-level`), optionally laid out for
//! SpaghettiKart (`--mk64`): start line at the origin racing toward -Z, scaled
//! to MK64 size, materials tagged with MK64 surface / collision / draw layer,
//! and <track>_mk64.json with the AI path through one vehicle's checkpoints,
//! the start grid and item box positions. The result goes to
//! tools/n64kr/blender_dkr_track.py and `n64kr dkr-track-scene`.
//!
//! Level models (include/structs.h LevelModel / LevelModelSegment /
//! TriangleBatchInfo / Vertex / Triangle; src/tracks.c): pointers are offsets
//! from the model's start; vertices with r == 1 and g == 1 store alpha in b;
//! UVs are s16 in 1/32 texel units.

use std::collections::HashMap;
use std::path::Path;

use super::gltf::{self, Corner, Groups, Key, TexInfo};
use super::objects::{self, LevelObjects, Point, MODEL_TYPE_3D, MODEL_TYPE_BILLBOARD};
use super::{inflate, s16, s8, tex, u32, Rom, BATCH_SIZE, LEVEL_MODELS, LEVEL_MODELS_TABLE, OBJECT_MODELS,
            OBJECT_MODELS_TABLE, RENDER_CUTOUT, RENDER_HIDDEN, RENDER_NO_COLLISION, RENDER_SEMI_TRANSPARENT,
            RENDER_WATER, SEGMENT_SIZE, TRIANGLE_SIZE, VERTEX_SIZE};
use crate::json::{self, obj, py_round, Value};

/// Level model names in v80 table order (decomp asset JSON LevelModel entries,
/// minus the v79-only variants): 56 entries, matching the ROM's table.
pub const LEVEL_NAMES: [&str; 56] = [
    "central_area_hub", "bluey", "dragon_forest_hub", "fossil_canyon", "pirate_lagoon", "ancient_lake",
    "walrus_cove", "hot_top_volcano", "whale_bay", "snowball_valley", "temple_track", "crescent_island",
    "fire_mountain", "dino_domain_hub", "everfrost_peak", "sherbet_island_hub", "front_end", "character_select",
    "spaceport_alpha", "horseshoe_gulch", "greenwood_village", "spacedust_alley", "title_screen",
    "boulder_canyon", "windmill_plains", "snowflake_mountain_hub", "smokey_castle", "darkwater_beach",
    "icicle_pyramid", "frosty_village", "jungle_falls", "treasure_caves", "haunted_woods", "darkmoon_caverns",
    "star_city", "trophy_race", "future_fun_land_hub", "intro_sequence", "smokey", "trickytops",
    "wizpig_mouth_sequence", "rocket_sequence", "wizpig1", "ocean_track", "volcano_track",
    "snow_mountain_track", "party_sequence", "bluey_animation", "wizpig1_animation", "bubbler",
    "bubbler_animation", "wizpig2_animation", "wizpig2", "ending_part1", "ending_part2", "spaceport_alpha_v80",
];

/// DKR is roughly twice MK64's scale: hover calibration maps DKR top speed
/// 12 units/tick to MK64 cruise 5.9 (recon/004-hover/SLICE-5_DKR_Stats.md).
const DKR_TO_MK64_SCALE: f64 = 0.5;
/// Path point spacing in MK64 units (Mario Raceway: ~600 points per lap).
const PATH_SPACING: f64 = 20.0;
/// MK64 units between DKR's front grid row and the start line (path point 0).
const START_LINE_AHEAD: f64 = 20.0;

fn corner(x: f64, y: f64, z: f64, rgba: [u8; 4], u: f64, v: f64) -> Corner {
    Corner { x, y, z, rgba, u, v }
}

/// Bakes the level's scenery objects in as decoration: 3D models (object
/// model layout as src/object_models.c object_model_init) and billboards as
/// two crossed double-sided quads with the sprite's frame 0.
fn scenery_groups(rom: &Rom, scenery: &[objects::Scenery], textures: &mut Vec<TexInfo>, groups: &mut Groups) -> Result<(), String> {
    let models = rom.section(OBJECT_MODELS);
    let table = rom.table(OBJECT_MODELS_TABLE);
    let mut tex_slot: HashMap<u32, usize> = HashMap::new();
    for (i, t) in textures.iter().enumerate() {
        tex_slot.insert(t.id, i);
    }
    let mut cache: HashMap<i32, Vec<u8>> = HashMap::new();
    let mut sprite_slot: HashMap<i32, (usize, usize, usize, i32, i32, usize, usize)> = HashMap::new();
    let (mut placed, mut skipped): (usize, Vec<(String, usize)>) = (0, vec![]);
    for obj in scenery {
        if obj.model_type == MODEL_TYPE_BILLBOARD && obj.model >= 0 {
            if !sprite_slot.contains_key(&obj.model) {
                let s = tex::sprite_image(rom, obj.model as usize, 0)?;
                let (tw, th, trgba) = tex::fit_tmem(s.width, s.height, &s.rgba);
                sprite_slot.insert(obj.model, (textures.len(), s.width, s.height, s.left, s.top, tw, th));
                textures.push(TexInfo {
                    id: 0x10000 + obj.model as u32,
                    surface: 0,
                    image: Some((tw, th, trgba)),
                    file: Some(format!("dkr_sprite{:04}", obj.model)),
                });
            }
            let (slot, w, h, left, top, tw, th) = sprite_slot[&obj.model];
            let corners = groups.entry(Key { tex: Some(slot), hidden: false, water: false, no_collision: true, transparent: true });
            let sc = obj.scale;
            let (x0, x1, y0, y1) = (left as f64 * sc, (left as f64 + w as f64) * sc, (top as f64 - h as f64) * sc, top as f64 * sc);
            for (dx, dz) in [(1.0, 0.0), (0.0, 1.0)] {
                let quad = [(x0, y1, 0.0, 0.0), (x1, y1, tw as f64, 0.0), (x1, y0, tw as f64, th as f64), (x0, y0, 0.0, th as f64)];
                let pts: Vec<(f64, f64, f64, f64, f64)> =
                    quad.iter().map(|&(qx, qy, u, v)| (obj.at.x + qx * dx, obj.at.y + qy, obj.at.z + qx * dz, u * 32.0, v * 32.0)).collect();
                for tri in [[0, 1, 2], [0, 2, 3], [2, 1, 0], [3, 2, 0]] {
                    for i in tri {
                        let (x, y, z, u, v) = pts[i];
                        corners.push(corner(x, y, z, [0xFF; 4], u, v));
                    }
                }
            }
            placed += 1;
            continue;
        }
        if obj.model_type != MODEL_TYPE_3D || obj.model < 0 || obj.model as usize >= table.len().saturating_sub(1) {
            match skipped.iter_mut().find(|(n, _)| *n == obj.name) {
                Some(e) => e.1 += 1,
                None => skipped.push((obj.name.clone(), 1)),
            }
            continue;
        }
        if !cache.contains_key(&obj.model) {
            cache.insert(obj.model, inflate(rom.entry(models, &table, obj.model as usize))?);
        }
        let m = &cache[&obj.model];
        let (tex_off, v_off, t_off, b_off) = (u32(m, 0) as usize, u32(m, 4) as usize, u32(m, 8) as usize, u32(m, 0x38) as usize);
        let (num_tex, num_batches) = (s16(m, 0x22).max(0) as usize, s16(m, 0x28).max(0) as usize);
        let mut local_tex = vec![];
        for t in 0..num_tex {
            let tid = u32(m, tex_off + t * 8) & 0x7FFF;
            let slot = *tex_slot.entry(tid).or_insert_with(|| {
                textures.push(TexInfo::rom(tid, 0));
                textures.len() - 1
            });
            local_tex.push(slot);
        }
        let a = obj.angle as f64 / 65536.0 * 2.0 * std::f64::consts::PI;
        let (ca, sa, sc) = (a.cos(), a.sin(), obj.scale);
        for b in 0..num_batches {
            let bo = b_off + b * BATCH_SIZE;
            let tex_index = m[bo] as usize;
            let v_base = s16(m, bo + 2) as usize;
            let (f_start, f_end) = (s16(m, bo + 4), s16(m, bo + BATCH_SIZE + 4));
            let flags = u32(m, bo + 8);
            if flags & RENDER_HIDDEN != 0 {
                continue;
            }
            let vertex_colours = m[bo + 6] == 0xFF; // BATCH_VTX_COL; otherwise lit by normals
            let key = Key {
                tex: if tex_index == 0xFF || tex_index >= local_tex.len() { None } else { Some(local_tex[tex_index]) },
                hidden: false,
                water: false,
                no_collision: true,
                transparent: flags & (RENDER_SEMI_TRANSPARENT | RENDER_CUTOUT) != 0,
            };
            let corners = groups.entry(key);
            for f in f_start..f_end {
                let tr = t_off + f as usize * TRIANGLE_SIZE;
                for c in 0..3 {
                    let vo = v_off + (v_base + m[tr + 1 + c] as usize) * VERTEX_SIZE;
                    let (lx, ly, lz) = (s16(m, vo) as f64 * sc, s16(m, vo + 2) as f64 * sc, s16(m, vo + 4) as f64 * sc);
                    let x = obj.at.x + lx * ca + lz * sa;
                    let z = obj.at.z - lx * sa + lz * ca;
                    let rgba = if vertex_colours { [m[vo + 6], m[vo + 7], m[vo + 8], m[vo + 9]] } else { [0xC8, 0xC8, 0xC8, 0xFF] };
                    let (u, v) = (s16(m, tr + 4 + c * 4) as f64, s16(m, tr + 6 + c * 4) as f64);
                    corners.push(corner(x, obj.at.y + ly, z, rgba, u, v));
                }
            }
        }
        placed += 1;
    }
    let skipped_text = if skipped.is_empty() {
        String::new()
    } else {
        format!("; skipped (sprites / no model): {{{}}}", skipped.iter().map(|(n, c)| format!("'{n}': {c}")).collect::<Vec<_>>().join(", "))
    };
    println!("Scenery: {placed} objects placed{skipped_text}");
    Ok(())
}

/// Highest triangle surface at (x, z) below y_above, or None.
fn triangle_floor(tris: &[[[f64; 3]; 3]], x: f64, z: f64, y_above: f64) -> Option<f64> {
    let mut best: Option<f64> = None;
    for [[ax, ay, az], [bx, by, bz], [cx, cy, cz]] in tris.iter().copied() {
        let d = (bz - cz) * (ax - cx) + (cx - bx) * (az - cz);
        if d.abs() < 1e-9 {
            continue;
        }
        let w0 = ((bz - cz) * (x - cx) + (cx - bx) * (z - cz)) / d;
        let w1 = ((cz - az) * (x - cx) + (ax - cx) * (z - cz)) / d;
        let w2 = 1.0 - w0 - w1;
        if w0 < -1e-6 || w1 < -1e-6 || w2 < -1e-6 {
            continue;
        }
        let y = w0 * ay + w1 * by + w2 * cy;
        if y <= y_above && best.map_or(true, |b| y > b) {
            best = Some(y);
        }
    }
    best
}

fn catmull_rom_loop(points: &[[f64; 3]], spacing: f64) -> Vec<[f64; 3]> {
    let n = points.len();
    let mut out = vec![];
    for i in 0..n {
        let (p0, p1, p2, p3) = (points[(i + n - 1) % n], points[i], points[(i + 1) % n], points[(i + 2) % n]);
        let seg = (0..3).map(|k| (p2[k] - p1[k]).powi(2)).fold(0.0, |a, b| a + b).powf(0.5);
        let steps = ((seg / spacing) as i64).max(1);
        for s in 0..steps {
            let t = s as f64 / steps as f64;
            let (t2, t3) = (t * t, t * t * t);
            out.push([0, 1, 2].map(|k| {
                0.5 * (2.0 * p1[k] + (-p0[k] + p2[k]) * t + (2.0 * p0[k] - 5.0 * p1[k] + 4.0 * p2[k] - p3[k]) * t2
                    + (-p0[k] + 3.0 * p1[k] - 3.0 * p2[k] + p3[k]) * t3)
            }));
        }
    }
    out
}

/// Puts the start line at the origin racing toward -Z (MK64 grids karts
/// behind path point 0 at +Z: src/spawn_players.c spawn_and_set_player_spawns),
/// scaled to MK64 size. Returns the transform and the path gates in DKR
/// coordinates: the start line, then the checkpoints from the next one on.
fn mk64_layout(objects: &LevelObjects) -> Result<(Box<dyn Fn(f64, f64, f64) -> [f64; 3]>, Vec<Point>), String> {
    // One vehicle's checkpoints only: each vehicle has its own numbered set
    // (Fossil Canyon: car 0..34, another set 0..56 in steps of 2), and mixing
    // sets appended a second, partial lap to the path (laps and positions
    // broke). The car's (0) if it has at least 3, else the vehicle with the most.
    let cps: Vec<&objects::Checkpoint> = objects.checkpoints.iter().filter(|c| c.alt == 0).collect();
    let mut counts: Vec<(i8, usize)> = vec![];
    for c in &cps {
        match counts.iter_mut().find(|(v, _)| *v == c.vehicle) {
            Some(e) => e.1 += 1,
            None => counts.push((c.vehicle, 1)),
        }
    }
    let car = counts.iter().find(|(v, _)| *v == 0).map_or(0, |e| e.1);
    let vehicle = if car >= 3 {
        0
    } else {
        // max() keeps the first of equal counts, like Python's max(counts, key=counts.get).
        counts.iter().fold(None, |best: Option<(i8, usize)>, &(v, n)| match best {
            Some((_, bn)) if bn >= n => best,
            _ => Some((v, n)),
        }).map_or(0, |e| e.0)
    };
    let mut by_index: Vec<(u8, &objects::Checkpoint)> = vec![];
    for c in cps.iter().filter(|c| c.vehicle == vehicle) {
        if !by_index.iter().any(|(i, _)| *i == c.index) {
            by_index.push((c.index, c));
        }
    }
    by_index.sort_by_key(|(i, _)| *i);
    let ordered: Vec<Point> = by_index.iter().map(|(_, c)| c.at.clone()).collect();
    let mut grid: Vec<&objects::SetupPoint> = objects.setup_points.iter().collect();
    grid.sort_by_key(|g| g.racer);
    if ordered.len() < 3 || grid.len() < 2 {
        return Err("not enough checkpoints / start positions to build a path".into());
    }
    // Start line: on the lap, just ahead of the front grid row (racers 0 and
    // 1), on the checkpoint-to-checkpoint segment nearest that row; the path
    // starts there and runs through the following checkpoints.
    let front = Point { x: (grid[0].at.x + grid[1].at.x) / 2.0, y: (grid[0].at.y + grid[1].at.y) / 2.0, z: (grid[0].at.z + grid[1].at.z) / 2.0 };
    let n = ordered.len();
    let segment = |i: usize| {
        let (a, b) = (&ordered[i % n], &ordered[(i + 1) % n]);
        let (dx, dz) = (b.x - a.x, b.z - a.z);
        (a.clone(), dx, dz, dx.hypot(dz))
    };
    let mut best: Option<((f64, bool), usize, f64)> = None;
    for i in 0..n {
        let (a, dx, dz, length) = segment(i);
        if length == 0.0 {
            continue;
        }
        let t = ((front.x - a.x) * dx + (front.z - a.z) * dz) / (length * length);
        let tc = t.clamp(0.0, 1.0);
        let d = (front.x - (a.x + dx * tc)).hypot(front.z - (a.z + dz * tc));
        let key = (d, tc != t); // on a tie (the row level with a checkpoint), the segment it is really on
        if best.map_or(true, |(k, _, _)| key.0 < k.0 || (key.0 == k.0 && !key.1 && k.1)) {
            best = Some((key, i, tc));
        }
    }
    let (_, mut i, t) = best.ok_or("checkpoints all coincide")?;
    // Walk START_LINE_AHEAD along the lap, onto later segments if it passes a checkpoint.
    let (mut a, mut fx, mut fz, mut length) = segment(i);
    let mut along = t * length + START_LINE_AHEAD / DKR_TO_MK64_SCALE;
    while along > length && length > 0.0 {
        along -= length;
        i += 1;
        (a, fx, fz, length) = segment(i);
    }
    let t = if length != 0.0 { along / length } else { 0.0 };
    let start = Point { x: a.x + fx * t, y: front.y, z: a.z + fz * t };
    let mut gates = vec![start.clone()];
    gates.extend((0..n).map(|k| ordered[(i + 1 + k) % n].clone())); // next checkpoint first, the one behind the line last
    let angle = (-fx).atan2(-fz);
    let (ca, sa) = (angle.cos(), angle.sin());
    let transform = move |x: f64, y: f64, z: f64| -> [f64; 3] {
        let (x, y, z) = (x - start.x, y - start.y, z - start.z);
        [(x * ca - z * sa) * DKR_TO_MK64_SCALE, y * DKR_TO_MK64_SCALE, (x * sa + z * ca) * DKR_TO_MK64_SCALE]
    };
    Ok((Box::new(transform), gates))
}

fn rounded(p: [f64; 3]) -> Value {
    json::floats(&[py_round(p[0], 2), py_round(p[1], 2), py_round(p[2], 2)])
}

#[allow(clippy::too_many_arguments)]
fn write_mk64_layout(
    dir: &Path,
    name: &str,
    transform: &dyn Fn(f64, f64, f64) -> [f64; 3],
    gates_dkr: &[Point],
    objects: &LevelObjects,
    floor_tris: &[[[f64; 3]; 3]],
    sky: Option<objects::Sky>,
) -> Result<f64, String> {
    let gates: Vec<[f64; 3]> = gates_dkr.iter().map(|c| transform(c.x, c.y, c.z)).collect();
    let mut path: Vec<[f64; 3]> = catmull_rom_loop(&gates, PATH_SPACING)
        .into_iter()
        .map(|[x, y, z]| {
            let floor = triangle_floor(floor_tris, x, z, y + 40.0);
            [py_round(x, 2), py_round(floor.unwrap_or(y), 2), py_round(z, 2)]
        })
        .collect();
    // Path point 0 at the origin exactly (SpaghettiKart docs/tracks/trackoverview.md).
    let oy = path[0][1];
    for p in path.iter_mut() {
        p[1] = py_round(p[1] - oy, 2);
    }
    let floor_offset = -oy;
    let place = |points: &[&Point]| -> Value {
        Value::Arr(points.iter().map(|e| {
            let [x, y, z] = transform(e.x, e.y, e.z);
            json::floats(&[py_round(x, 2), py_round(y + floor_offset, 2), py_round(z, 2)])
        }).collect())
    };
    let mut grid: Vec<&objects::SetupPoint> = objects.setup_points.iter().collect();
    grid.sort_by_key(|g| g.racer);
    let grid_points: Vec<&Point> = grid.iter().map(|g| &g.at).collect();
    let sky_value = match &sky {
        Some(s) => obj(vec![("top", json::ints(&s.top)), ("horizon", json::ints(&s.horizon)), ("dome", Value::Str(s.dome.clone()))]),
        None => Value::Null,
    };
    let start_grid = place(&grid_points);
    let item_boxes = place(&objects.balloons.iter().collect::<Vec<_>>());
    let layout = obj(vec![
        ("track", Value::Str(name.into())),
        ("units", Value::Str("MK64 (N64 coordinates, Y up). Path point 0 at origin, racing towards -Z.".into())),
        ("scale_from_dkr", Value::Float(DKR_TO_MK64_SCALE)),
        ("y_offset", Value::Float(floor_offset)),
        ("path", Value::Arr(path.iter().map(|&p| rounded(p)).collect())),
        ("checkpoints", place(&gates_dkr[1..].iter().collect::<Vec<_>>())),
        ("start_grid", start_grid.clone()),
        ("item_boxes", item_boxes.clone()),
        ("zippers", place(&objects.zippers.iter().collect::<Vec<_>>())),
        ("sky", sky_value),
    ]);
    std::fs::write(dir.join(format!("{name}_mk64.json")), layout.to_string_indent(1)).map_err(|e| e.to_string())?;
    let grid_z: Vec<f64> = start_grid.as_array().unwrap().iter().map(|p| p.as_array().unwrap()[2].as_f64().unwrap()).collect();
    let (zmin, zmax) = (grid_z.iter().cloned().fold(f64::INFINITY, f64::min), grid_z.iter().cloned().fold(f64::NEG_INFINITY, f64::max));
    println!(
        "MK64 layout: {} path points, {} checkpoints, start grid z {zmin:.0}..{zmax:.0} (should be > 0, behind the line), {} item boxes.",
        path.len(),
        gates_dkr.len() - 1,
        item_boxes.as_array().unwrap().len()
    );
    Ok(floor_offset)
}

/// `n64kr dkr-level <rom> <track> <out root> [--mk64]`: writes <out root>/<track>[_mk64]/.
pub fn convert(rom: &Rom, name: &str, out_root: &str, mk64: bool) -> Result<(), String> {
    let index = LEVEL_NAMES.iter().position(|n| *n == name).ok_or_else(|| format!("unknown track '{name}' (use --list)"))?;
    let table = rom.table(LEVEL_MODELS_TABLE);
    if table.len() != LEVEL_NAMES.len() {
        return Err("level model table size does not match this ROM".into());
    }
    let data = inflate(rom.entry(rom.section(LEVEL_MODELS), &table, index))?;
    let (tex_off, seg_off) = (u32(&data, 0) as usize, u32(&data, 4) as usize);
    let (num_textures, num_segments) = (s16(&data, 0x18).max(0) as usize, s16(&data, 0x1A).max(0) as usize);
    let mut textures: Vec<TexInfo> =
        (0..num_textures).map(|t| TexInfo::rom(u32(&data, tex_off + t * 8) & 0x7FFF, s8(&data, tex_off + t * 8 + 7))).collect();

    let mut groups = Groups::default();
    for k in 0..num_segments {
        let so = seg_off + k * SEGMENT_SIZE;
        let (v_off, t_off, b_off) = (u32(&data, so) as usize, u32(&data, so + 4) as usize, u32(&data, so + 0xC) as usize);
        for b in 0..s16(&data, so + 0x20).max(0) as usize {
            let bo = b_off + b * BATCH_SIZE;
            let tex_index = data[bo] as usize;
            let v_base = s16(&data, bo + 2) as usize;
            let (f_start, f_end) = (s16(&data, bo + 4), s16(&data, bo + BATCH_SIZE + 4));
            let flags = u32(&data, bo + 8);
            let key = Key {
                tex: if tex_index == 0xFF { None } else { Some(tex_index) },
                hidden: flags & RENDER_HIDDEN != 0,
                water: flags & RENDER_WATER != 0,
                no_collision: flags & RENDER_NO_COLLISION != 0,
                transparent: flags & (RENDER_SEMI_TRANSPARENT | RENDER_CUTOUT) != 0,
            };
            let corners = groups.entry(key);
            for f in f_start..f_end {
                let tr = t_off + f as usize * TRIANGLE_SIZE;
                for c in 0..3 {
                    let vo = v_off + (v_base + data[tr + 1 + c] as usize) * VERTEX_SIZE;
                    let (x, y, z) = (s16(&data, vo) as f64, s16(&data, vo + 2) as f64, s16(&data, vo + 4) as f64);
                    let mut rgba = [data[vo + 6], data[vo + 7], data[vo + 8], data[vo + 9]];
                    if rgba[0] == 1 && rgba[1] == 1 {
                        rgba = [0x80, 0x80, 0x80, rgba[2]];
                    }
                    let (u, v) = (s16(&data, tr + 4 + c * 4) as f64, s16(&data, tr + 6 + c * 4) as f64);
                    corners.push(corner(x, y, z, rgba, u, v));
                }
            }
        }
    }

    let dir = Path::new(out_root).join(format!("{name}{}", if mk64 { "_mk64" } else { "" }));
    std::fs::create_dir_all(dir.join("textures")).map_err(|e| e.to_string())?;
    if mk64 {
        let objects = objects::level_objects(rom, index)?;
        scenery_groups(rom, &objects.scenery, &mut textures, &mut groups)?;
        let (transform, gates) = mk64_layout(&objects)?;
        groups.for_each_mut(|_, corners| {
            for c in corners.iter_mut() {
                let [x, y, z] = transform(c.x, c.y, c.z);
                (c.x, c.y, c.z) = (x, y, z);
            }
        });
        let mut floor_tris = vec![];
        for (key, corners) in groups.iter() {
            if !key.hidden && !key.no_collision {
                for t in corners.chunks_exact(3) {
                    floor_tris.push([0, 1, 2].map(|i| [t[i].x, t[i].y, t[i].z]));
                }
            }
        }
        let sky = objects::sky_colours(rom, objects.sky_object)?;
        let y_offset = write_mk64_layout(&dir, name, transform.as_ref(), &gates, &objects, &floor_tris, sky)?;
        groups.for_each_mut(|_, corners| corners.iter_mut().for_each(|c| c.y += y_offset));
    }
    gltf::write(rom, &dir, name, &groups, &textures, None)
}
