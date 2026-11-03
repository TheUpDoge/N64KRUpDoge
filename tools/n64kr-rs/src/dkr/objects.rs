//! DKR objects: headers (include/structs.h ObjectHeader), the objects a race
//! level places (level object maps, src/objects.c track_spawn_objects) and the
//! sky dome's colours.

use super::{inflate, s16, s32, s8, tex, u32, Rom, BATCH_SIZE, LEVEL_HEADERS, LEVEL_HEADERS_TABLE,
            LEVEL_OBJECT_MAPS, LEVEL_OBJECT_MAPS_TABLE, LEVEL_OBJECT_TRANSLATION_TABLE, OBJECTS,
            OBJECT_HEADERS_TABLE, OBJECT_MODELS, OBJECT_MODELS_TABLE, TEXTURES_3D, TEXTURES_3D_TABLE,
            TRIANGLE_SIZE, VERTEX_SIZE};

pub const BHV_SCENERY: i8 = 2; // include/object_behaviors.h
pub const BHV_RACER: i8 = 1;
pub const MODEL_TYPE_3D: i8 = 0; // ObjectHeader.modelType: 0 = 3D model, others = textures / sprites
pub const MODEL_TYPE_BILLBOARD: i8 = 1;

#[derive(Clone)]
pub struct Header {
    pub name: String,
    pub behaviour: i8,
    pub model_type: i8,
    pub scale: f64,
    pub models: Vec<i32>,
}

/// The NUL-terminated name at 0x60..0x70.
pub fn header_name(h: &[u8]) -> String {
    let raw = &h[0x60..0x70];
    let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    raw[..end].iter().map(|&b| if b.is_ascii() { b as char } else { '\u{fffd}' }).collect()
}

/// Every object header: name, behaviour, model type, scale, model ids
/// (pointers in a header are offsets from its start).
pub fn headers(rom: &Rom) -> Vec<Header> {
    let objs = rom.section(OBJECTS);
    let table = rom.table(OBJECT_HEADERS_TABLE);
    let mut out = Vec::new();
    for i in 0..table.len().saturating_sub(1) {
        let h = rom.entry(objs, &table, i);
        if h.len() < 0x70 {
            out.push(Header { name: String::new(), behaviour: -1, model_type: -1, scale: 1.0, models: vec![] });
            continue;
        }
        let count = s8(h, 0x55).max(0) as usize;
        let ids_off = u32(h, 0x10) as usize;
        let models = if ids_off > 0 && ids_off < h.len() { (0..count).map(|k| s32(h, ids_off + 4 * k)).collect() } else { vec![] };
        out.push(Header {
            name: header_name(h),
            behaviour: s8(h, 0x54),
            model_type: s8(h, 0x53),
            scale: super::f32_at(h, 0x0C) as f64,
            models,
        });
    }
    out
}

#[derive(Clone, Debug)]
pub struct Point {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

pub struct Checkpoint {
    pub at: Point,
    pub index: u8,
    pub alt: u8,
    pub vehicle: i8,
}

pub struct SetupPoint {
    pub at: Point,
    pub racer: u8,
}

pub struct Scenery {
    pub name: String,
    pub at: Point,
    pub scale: f64,
    pub angle: u32,
    pub model_type: i8,
    pub model: i32,
}

pub struct LevelObjects {
    pub checkpoints: Vec<Checkpoint>,
    pub setup_points: Vec<SetupPoint>,
    pub balloons: Vec<Point>,
    pub zippers: Vec<Point>,
    pub scenery: Vec<Scenery>,
    pub sky_object: i16,
}

/// Checkpoints, start grid, item balloons, zippers and scenery of the first
/// race level whose header uses level model `model_index`.
pub fn level_objects(rom: &Rom, model_index: usize) -> Result<LevelObjects, String> {
    let lheaders = rom.section(LEVEL_HEADERS);
    let htable = rom.table(LEVEL_HEADERS_TABLE);
    let header = (0..htable.len().saturating_sub(1))
        .map(|i| rom.entry(lheaders, &htable, i))
        .find(|h| h.len() >= 0xBC && s16(h, 0x34) as i64 == model_index as i64 && h[0x4B] > 0)
        .ok_or("no race level header uses this model")?;
    let map_ids = [s16(header, 0xBA), s16(header, 0x36)]; // main objects, collectables
    let sky_object = s16(header, 0x38); // object header index of the sky dome (src/tracks.c skydome_spawn)

    let trans_raw = rom.section(LEVEL_OBJECT_TRANSLATION_TABLE);
    let trans: Vec<i16> = (0..trans_raw.len().saturating_sub(1)).step_by(2).map(|i| s16(trans_raw, i)).collect();
    let hdrs = headers(rom);
    let maps = rom.section(LEVEL_OBJECT_MAPS);
    let mtable = rom.table(LEVEL_OBJECT_MAPS_TABLE);
    let mut found = LevelObjects {
        checkpoints: vec![],
        setup_points: vec![],
        balloons: vec![],
        zippers: vec![],
        scenery: vec![],
        sky_object,
    };
    for map_id in map_ids {
        let data = inflate(rom.entry(maps, &mtable, map_id as usize))?;
        // u32 byte size, then entries from offset 16 (gObjectMap is s32 *,
        // the spawn list starts at gObjectMap + 4).
        let (size, mut pos) = (u32(&data, 0) as usize, 16usize);
        while pos < 16 + size {
            let length = (data[pos + 1] & 0x3F) as usize;
            if length == 0 {
                break;
            }
            let object_id = data[pos] as usize | (((data[pos + 1] & 0x80) as usize) << 1);
            let header_index = trans.get(object_id).map_or(-1, |&t| t as i32);
            let hd = if header_index >= 0 { hdrs.get(header_index as usize) } else { None };
            let name = hd.map_or("", |h| h.name.as_str());
            let at = Point { x: s16(&data, pos + 2) as f64, y: s16(&data, pos + 4) as f64, z: s16(&data, pos + 6) as f64 };
            if let Some(hd) = hd.filter(|h| h.behaviour == BHV_SCENERY) {
                // LevelObjectEntry_Scenery; placement as obj_init_scenery (src/object_functions.c)
                let mut model_index = data[pos + 8] as usize;
                if model_index >= hd.models.len() {
                    model_index = 0;
                }
                let radius = (data[pos + 9].max(10)) as f64 / 64.0;
                found.scenery.push(Scenery {
                    name: name.to_string(),
                    at: at.clone(),
                    scale: hd.scale * radius,
                    angle: (data[pos + 0xA] as u32) << 8,
                    model_type: hd.model_type,
                    model: hd.models.get(model_index).copied().unwrap_or(-1),
                });
            }
            match name {
                // LevelObjectEntry_Checkpoint (include/level_object_entries.h)
                "checkpoint" => found.checkpoints.push(Checkpoint {
                    at,
                    index: data[pos + 9],
                    alt: data[pos + 0x17],
                    vehicle: s8(&data, pos + 0x1A),
                }),
                "setuppoint" => found.setup_points.push(SetupPoint { at, racer: data[pos + 8] }),
                "WeaponBalloon" => found.balloons.push(at),
                "GroundZipper" => found.zippers.push(at),
                _ => {}
            }
            pos += length;
        }
    }
    Ok(found)
}

pub struct Sky {
    pub top: [i64; 3],
    pub horizon: [i64; 3],
    pub dome: String,
}

/// Top and horizon colour of the level's sky dome: vertex colour x mean
/// texture colour, averaged over the top 20% and bottom 15% of the dome.
pub fn sky_colours(rom: &Rom, sky_object: i16) -> Result<Option<Sky>, String> {
    let hdrs = headers(rom);
    let hd = match usize::try_from(sky_object).ok().and_then(|i| hdrs.get(i)) {
        Some(h) if !h.models.is_empty() => h,
        _ => return Ok(None),
    };
    let table = rom.table(OBJECT_MODELS_TABLE);
    let m = inflate(rom.entry(rom.section(OBJECT_MODELS), &table, hd.models[0] as usize))?;
    let (tex3d, ttable) = (rom.section(TEXTURES_3D), rom.table(TEXTURES_3D_TABLE));
    let (tex_off, v_off, t_off, b_off) = (u32(&m, 0) as usize, u32(&m, 4) as usize, u32(&m, 8) as usize, u32(&m, 0x38) as usize);
    let mut means = Vec::new();
    for t in 0..s16(&m, 0x22).max(0) as usize {
        let tid = (u32(&m, tex_off + t * 8) & 0x7FFF) as usize;
        let tx = tex::decode(rom.entry(tex3d, &ttable, tid), 0)?;
        let n = tx.width * tx.height;
        means.push([0, 1, 2].map(|k| (0..n).map(|i| tx.rgba[i * 4 + k] as u64).sum::<u64>() as f64 / n as f64 / 255.0));
    }
    let mut samples: Vec<(i16, [f64; 3])> = Vec::new();
    for b in 0..s16(&m, 0x28).max(0) as usize {
        let bo = b_off + b * BATCH_SIZE;
        let ti = m[bo] as usize;
        let mean = means.get(ti).copied().unwrap_or([1.0; 3]);
        for f in s16(&m, bo + 4)..s16(&m, bo + BATCH_SIZE + 4) {
            let tr = t_off + f as usize * TRIANGLE_SIZE;
            for c in 0..3 {
                let vo = v_off + (s16(&m, bo + 2) as usize + m[tr + 1 + c] as usize) * VERTEX_SIZE;
                samples.push((s16(&m, vo + 2), [0, 1, 2].map(|k| (m[vo + 6 + k] as f64 * mean[k]).min(255.0))));
            }
        }
    }
    let lo = samples.iter().map(|s| s.0).min().ok_or("sky dome has no triangles")? as f64;
    let hi = samples.iter().map(|s| s.0).max().unwrap() as f64;
    let band = |a: f64, b: f64| -> [i64; 3] {
        let sel: Vec<&[f64; 3]> =
            samples.iter().filter(|s| lo + a * (hi - lo) <= s.0 as f64 && s.0 as f64 <= lo + b * (hi - lo)).map(|s| &s.1).collect();
        [0, 1, 2].map(|k| (sel.iter().fold(0.0, |acc, c| acc + c[k]) / sel.len() as f64) as i64)
    };
    Ok(Some(Sky { top: band(0.8, 1.0), horizon: band(0.0, 0.15), dome: hd.name.clone() }))
}
