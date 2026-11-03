//! DKR racers (driver + vehicle) to glTF (`n64kr dkr-racer`), for the sprite,
//! HUD and face renderers (tools/n64kr/blender_dkr_racer_*.py); the parsed
//! racer is shared with the in-game model export (`n64kr dkr-racer-model`).
//!
//! - Racer objects have behaviour BHV_RACER (1); their header lists 6 models,
//!   a detail series, model 0 the most detailed.
//! - ObjectModel (include/structs.h): textures @0x00, vertices @0x04,
//!   triangles @0x08, attach points @0x14 (s16 vertex indices, count @0x18),
//!   counts @0x22.., batches @0x38; offsets from the model start (src/object_models.c).
//! - Header vehiclePartIds @0x14 / vehiclePartIndices @0x18 (count @0x56): one
//!   vehicle part object per attach point (the wheels), sprite objects whose
//!   frames show the wheel from head-on (frame 0) to side-on (last).
//! - Vertex animation: ASSET_ANIMATION_IDS (s16 per model) gives the model's
//!   range in ASSET_OBJECT_ANIMATIONS (model_anim_init). Each is compressed:
//!   s32 keyframe count, keyframe 0 (12-byte header + s16 xyz offset per
//!   animated vertex, added to the base vertex), keyframes 1.. at stride
//!   12 + 3 x count (12-byte header + s8 xyz deltas, cumulative); a frame is
//!   1/16 keyframe. Model 0x4A = animated vertex count, 0x4C = s16 animated
//!   slot per vertex (-1 = static). Read from obj_animate (src/hasm/obj_animate.s).
//! - Racers drive with animation 0, frame 40 - steerAngle / 2 clamped 0..73
//!   (src/racer.c): 40 straight, below 40 steering right, above 40 left.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use super::gltf::{self, Corner, Groups, Key, TexInfo};
use super::objects::{self, Header, BHV_RACER};
use super::{inflate, s16, s32, tex, u32, Rom, ANIMATION_IDS, BATCH_SIZE, OBJECTS, OBJECT_ANIMATIONS,
            OBJECT_ANIMATIONS_TABLE, OBJECT_HEADERS_TABLE, OBJECT_MODELS, OBJECT_MODELS_TABLE, RENDER_CUTOUT,
            RENDER_HIDDEN, RENDER_SEMI_TRANSPARENT, SPRITES, SPRITES_TABLE, TEXTURES_3D, TEXTURES_3D_TABLE,
            TRIANGLE_SIZE, VERTEX_SIZE};
use crate::json::{self, obj, Value};
use crate::png;

/// Extra animations exported as shape keys (animation index, frames used),
/// from DKR's racer actions (src/racer.c func_80052988 calls): 4 = horn (the
/// Z-button taunt, 48 frames), 3 = crash (32 frames).
const EXTRA_ANIMS: [(usize, usize); 2] = [(4, 48), (3, 32)];
const EXTRA_ANIM_STEP: usize = 4;

/// One animation frame: animated vertex index -> position.
pub type Frame = HashMap<usize, [i64; 3]>;

pub struct Wheel {
    pub part: String,
    pub scale: f64,
    pub steers: bool,
    pub position: [i64; 3],
    /// Per spin state, per view angle: the sprite frame.
    pub spins: Vec<Vec<tex::SpriteImage>>,
}

pub struct Racer {
    pub header: Header,
    pub model_id: usize,
    pub model: Vec<u8>,
    pub textures: Vec<TexInfo>,
    pub groups: Groups,
    /// Per group, the model vertex index of each corner.
    pub corner_vertex: HashMap<Key, Vec<usize>>,
    pub anim_frames: Vec<Frame>,
    pub wheels: Vec<Wheel>,
}

/// Vertex positions of every frame of one model animation; empty if none.
pub fn animation_frames(rom: &Rom, model_id: usize, m: &[u8], anim_index: usize) -> Result<Vec<Frame>, String> {
    let ids = rom.section(ANIMATION_IDS);
    let (start, end) = (s16(ids, 2 * model_id) as i64, s16(ids, 2 * model_id + 2) as i64);
    let (count, v_off) = (s16(m, 0x4A) as i64, u32(m, 0x04) as usize);
    if anim_index as i64 >= end - start || count < 1 {
        return Ok(vec![]);
    }
    let count = count as usize;
    let table = rom.table(OBJECT_ANIMATIONS_TABLE);
    let data = inflate(rom.entry(rom.section(OBJECT_ANIMATIONS), &table, start as usize + anim_index))?;
    let (keyframes, anim) = (s32(&data, 0), &data[4..]);
    let index_off = u32(m, 0x4C) as usize;
    let mut slots: Vec<(usize, usize)> = vec![]; // (vertex index, animated slot)
    for v in 0..s16(m, 0x24).max(0) as usize {
        let slot = s16(m, index_off + 2 * v);
        if slot >= 0 {
            slots.push((v, slot as usize));
        }
    }
    let mut pos = vec![[0i64; 3]; count];
    for &(v, slot) in &slots {
        // keyframe 0: base vertex + s16 offset
        let vo = v_off + v * VERTEX_SIZE;
        pos[slot] = [0, 1, 2].map(|k| s16(m, vo + k * 2) as i64 + s16(anim, 12 + slot * 6 + k * 2) as i64);
    }
    let stride = 12 + 3 * count;
    let delta = |keyframe: usize, slot: usize, k: usize| anim[stride * (keyframe + 1) + slot * 3 + k] as i8 as i64; // s8 deltas
    let mut frames = vec![];
    for keyframe in 0..(keyframes - 1).max(0) as usize {
        for t in 0..16 {
            let cur: Vec<[i64; 3]> = pos.iter().enumerate().map(|(s, p)| [0, 1, 2].map(|k| p[k] + ((delta(keyframe + 1, s, k) * t) >> 4))).collect();
            frames.push(slots.iter().map(|&(v, slot)| (v, cur[slot])).collect());
        }
        pos = pos.iter().enumerate().map(|(s, p)| [0, 1, 2].map(|k| p[k] + delta(keyframe + 1, s, k))).collect();
    }
    Ok(frames)
}

pub fn racer_headers(rom: &Rom) -> Vec<(usize, Header)> {
    objects::headers(rom).into_iter().enumerate().filter(|(_, h)| h.behaviour == BHV_RACER).collect()
}

/// Parses a racer: body model 0 (corners grouped by material), its driving
/// animation, and its wheels (attach position, part scale, sprite frames).
pub fn load(rom: &Rom, object_name: &str) -> Result<Racer, String> {
    let headers = objects::headers(rom);
    let index = headers
        .iter()
        .position(|h| h.name == object_name && h.behaviour == BHV_RACER)
        .ok_or_else(|| format!("no racer object '{object_name}' (use --list)"))?;
    let hd = headers[index].clone();
    let raw_header = rom.entry(rom.section(OBJECTS), &rom.table(OBJECT_HEADERS_TABLE), index).to_vec();
    let model_id = *hd.models.first().ok_or("racer has no model")? as usize;
    let m = inflate(rom.entry(rom.section(OBJECT_MODELS), &rom.table(OBJECT_MODELS_TABLE), model_id))?;
    let (tex_off, v_off, t_off) = (u32(&m, 0) as usize, u32(&m, 4) as usize, u32(&m, 8) as usize);
    let (attach_off, attach_count) = (u32(&m, 0x14) as usize, s16(&m, 0x18));
    let (num_tex, b_off, num_batches) = (s16(&m, 0x22).max(0) as usize, u32(&m, 0x38) as usize, s16(&m, 0x28).max(0) as usize);
    let textures: Vec<TexInfo> = (0..num_tex).map(|t| TexInfo::rom(u32(&m, tex_off + t * 8) & 0x7FFF, 0)).collect();

    let (mut groups, mut corner_vertex) = (Groups::default(), HashMap::<Key, Vec<usize>>::new());
    for b in 0..num_batches {
        let bo = b_off + b * BATCH_SIZE;
        let tex_index = m[bo] as usize;
        let v_base = s16(&m, bo + 2) as usize;
        let (f_start, f_end) = (s16(&m, bo + 4), s16(&m, bo + BATCH_SIZE + 4));
        let flags = u32(&m, bo + 8);
        if flags & RENDER_HIDDEN != 0 {
            continue;
        }
        let vertex_colours = m[bo + 6] == 0xFF; // otherwise lit by normals in DKR
        let key = Key {
            tex: if tex_index == 0xFF || tex_index >= num_tex { None } else { Some(tex_index) },
            hidden: false,
            water: false,
            no_collision: true,
            transparent: flags & (RENDER_SEMI_TRANSPARENT | RENDER_CUTOUT) != 0,
        };
        let corners = groups.entry(key);
        let vertex_ids = corner_vertex.entry(key).or_default();
        for f in f_start..f_end {
            let tr = t_off + f as usize * TRIANGLE_SIZE;
            for c in 0..3 {
                let vid = v_base + m[tr + 1 + c] as usize;
                vertex_ids.push(vid);
                let vo = v_off + vid * VERTEX_SIZE;
                let rgba = if vertex_colours { [m[vo + 6], m[vo + 7], m[vo + 8], m[vo + 9]] } else { [0xE0, 0xE0, 0xE0, 0xFF] };
                corners.push(Corner {
                    x: s16(&m, vo) as f64,
                    y: s16(&m, vo + 2) as f64,
                    z: s16(&m, vo + 4) as f64,
                    rgba,
                    u: s16(&m, tr + 4 + c * 4) as f64,
                    v: s16(&m, tr + 6 + c * 4) as f64,
                });
            }
        }
    }

    // Wheels: one vehicle part per attach point, positioned at the attach vertex.
    let attach_vertices: Vec<i16> = (0..attach_count.max(0) as usize).map(|k| s16(&m, attach_off + 2 * k)).collect();
    let part_count = raw_header[0x56] as usize;
    let (ids_off, idx_off) = (u32(&raw_header, 0x14) as usize, u32(&raw_header, 0x18) as usize);
    let part_ids: Vec<i32> = if ids_off > 0 && ids_off < raw_header.len() { (0..part_count).map(|k| s32(&raw_header, ids_off + 4 * k)).collect() } else { vec![] };
    let part_attach: Vec<u8> = if idx_off > 0 && idx_off < raw_header.len() {
        raw_header[idx_off..(idx_off + part_count).min(raw_header.len())].to_vec()
    } else {
        vec![]
    };
    let (sprites, stable) = (rom.section(SPRITES), rom.table(SPRITES_TABLE));
    let mut wheels = vec![];
    for (k, &part_id) in part_ids.iter().enumerate() {
        if part_id < 0 || part_id as usize >= headers.len() || k >= part_attach.len() || part_attach[k] as usize >= attach_vertices.len() {
            continue;
        }
        let part = &headers[part_id as usize];
        let vo = v_off + attach_vertices[part_attach[k] as usize] as usize * VERTEX_SIZE;
        // Each sprite is one wheel-spin state (picked by the part's modelIndex);
        // its frames are the wheel seen from 0 deg (head-on, frame 0) to 90 deg
        // (side on, last frame) round from the vehicle's vertical plane
        // (render_sprite_billboard, src/camera.c).
        let mut spins = vec![];
        for &sprite_id in &part.models {
            let sprite = &sprites[stable[sprite_id as usize] as usize..];
            let angles = (s16(sprite, 2).max(1)) as usize;
            spins.push((0..angles).map(|a| tex::sprite_image(rom, sprite_id as usize, a)).collect::<Result<Vec<_>, _>>()?);
        }
        // DKR steers attach points 2 and 3 (y_rotation = stick x * 100, src/racer.c
        // update_car_wheels); 0 and 1 stay straight.
        wheels.push(Wheel {
            part: part.name.clone(),
            scale: part.scale,
            steers: k >= 2,
            position: [s16(&m, vo) as i64, s16(&m, vo + 2) as i64, s16(&m, vo + 4) as i64],
            spins,
        });
    }
    let anim_frames = animation_frames(rom, model_id, &m, 0)?;
    Ok(Racer { header: hd, model_id, model: m, textures, groups, corner_vertex, anim_frames, wheels })
}

/// `n64kr dkr-racer <rom> <object> <out root>`: writes <out root>/<object>/.
pub fn export(rom: &Rom, object_name: &str, out_root: &str) -> Result<(), String> {
    let racer = load(rom, object_name)?;
    let frames = &racer.anim_frames;

    // Morph targets (shape keys), each the offset of every corner from the
    // base pose: every frame of the driving animation, and every
    // EXTRA_ANIM_STEP-th frame of the extra animations (select-screen faces,
    // blender_dkr_racer_faces.py).
    let mut named: Vec<(String, &Frame)> = frames.iter().enumerate().map(|(i, f)| (format!("anim0_f{i:02}"), f)).collect();
    let mut extra_frames: Vec<Vec<Frame>> = vec![];
    let mut extra: Vec<(String, Vec<usize>)> = vec![];
    for (anim, length) in EXTRA_ANIMS {
        let mut f = animation_frames(rom, racer.model_id, &racer.model, anim)?;
        f.truncate(length);
        extra.push((anim.to_string(), (0..f.len()).step_by(EXTRA_ANIM_STEP).collect()));
        extra_frames.push(f);
    }
    for ((anim, picked), f) in extra.iter().zip(&extra_frames) {
        for &i in picked {
            named.push((format!("anim{anim}_f{i:02}"), &f[i]));
        }
    }
    let mut targets: HashMap<Key, Vec<Vec<[f64; 3]>>> = HashMap::new();
    for (key, corners) in racer.groups.iter() {
        let vids = &racer.corner_vertex[key];
        targets.insert(
            *key,
            named
                .iter()
                .map(|(_, f)| {
                    vids.iter().zip(corners).map(|(v, c)| match f.get(v) {
                        Some(p) => [p[0] as f64 - c.x, p[1] as f64 - c.y, p[2] as f64 - c.z],
                        None => [0.0; 3],
                    }).collect()
                })
                .collect(),
        );
    }
    let dir = Path::new(out_root).join(object_name);
    let names: Vec<String> = named.iter().map(|(n, _)| n.clone()).collect();
    gltf::write(rom, &dir, object_name, &racer.groups, &racer.textures, if named.is_empty() { None } else { Some((&targets, &names)) })?;

    // Later frames of animated textures (the eyes: open .. shut), next to the
    // first frame gltf::write saved.
    let (tex3d, tex3d_table) = (rom.section(TEXTURES_3D), rom.table(TEXTURES_3D_TABLE));
    let mut texture_frames: Vec<(String, usize)> = vec![];
    for info in &racer.textures {
        let raw = rom.entry(tex3d, &tex3d_table, info.id as usize);
        let count = tex::frames(raw);
        if count < 2 || texture_frames.iter().any(|(id, _)| *id == info.id.to_string()) {
            continue;
        }
        texture_frames.push((info.id.to_string(), count));
        for frame in 1..count {
            let t = tex::decode(raw, frame)?;
            let file = dir.join("textures").join(format!("dkr_tex{:04}_f{frame}.png", info.id));
            std::fs::write(&file, png::encode_rgba(t.width, t.height, &t.rgba)).map_err(|e| e.to_string())?;
        }
    }

    let (mut wheels, mut written) = (vec![], HashSet::new());
    for wheel in &racer.wheels {
        let mut spin_files = vec![];
        for (spin, images) in wheel.spins.iter().enumerate() {
            let mut angles = vec![];
            for (angle, img) in images.iter().enumerate() {
                let file = format!("wheel_{}_s{spin}_a{angle:02}.png", wheel.part);
                if written.insert(file.clone()) {
                    std::fs::write(dir.join(&file), png::encode_rgba(img.width, img.height, &img.rgba)).map_err(|e| e.to_string())?;
                }
                angles.push(obj(vec![
                    ("file", Value::Str(file)),
                    ("w", Value::Int(img.width as i64)),
                    ("h", Value::Int(img.height as i64)),
                    ("left", Value::Int(img.left as i64)),
                    ("top", Value::Int(img.top as i64)),
                ]));
            }
            spin_files.push(Value::Arr(angles));
        }
        wheels.push(obj(vec![
            ("part", Value::Str(wheel.part.clone())),
            ("scale", Value::Float(wheel.scale)),
            ("steers", Value::Bool(wheel.steers)),
            ("position", json::ints(&wheel.position)),
            ("frames", Value::Arr(spin_files)),
        ]));
    }
    let info = obj(vec![
        ("object", Value::Str(object_name.into())),
        ("model", Value::Int(racer.model_id as i64)),
        ("object_scale", Value::Float(racer.header.scale)),
        ("drive_animation", obj(vec![
            ("frames", Value::Int(frames.len() as i64)),
            ("straight", Value::Int(40)),
            ("full_right", Value::Int(0)),
            ("full_left", Value::Int(73)),
            ("shape_key", Value::Str("anim0_fNN".into())),
        ])),
        ("extra_animations", obj(vec![
            ("frames", Value::Obj(extra.iter().map(|(a, p)| (a.clone(), json::ints(&p.iter().map(|&i| i as i64).collect::<Vec<_>>()))).collect())),
            ("shape_key", Value::Str("anim<A>_fNN".into())),
            ("4", Value::Str("horn".into())),
            ("3", Value::Str("crash".into())),
        ])),
        ("texture_frames", Value::Obj(texture_frames.iter().map(|(id, n)| (id.clone(), Value::Int(*n as i64))).collect())),
        ("units", Value::Str("DKR model units, Y up (same as the glTF)".into())),
        ("wheels", Value::Arr(wheels)),
    ]);
    std::fs::write(dir.join(format!("{object_name}_racer.json")), info.to_string_indent(1)).map_err(|e| e.to_string())?;
    // Drop single-frame wheel images from older exports.
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("wheel_") && !written.contains(&name) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    let tris: usize = racer.groups.iter().map(|(_, c)| c.len()).sum::<usize>() / 3;
    println!(
        "Racer {object_name}: model {}, {tris} triangles, {} textures, {} wheels ({} wheel frame images), {} animation frames.",
        racer.model_id,
        racer.textures.len(),
        racer.wheels.len(),
        written.len(),
        frames.len()
    );
    Ok(())
}

/// `n64kr dkr-racer <rom> --list`
pub fn list(rom: &Rom) {
    for (_, h) in racer_headers(rom) {
        println!("{:16} models {:?}", h.name, h.models);
    }
}
