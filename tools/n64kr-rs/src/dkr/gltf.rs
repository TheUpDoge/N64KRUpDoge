//! glTF 2.0 output for DKR tracks and racers: triangle corners grouped by
//! material, PNG textures with DKR's wrap modes, MK64 surface / collision /
//! draw-layer tags as material extras (read by tools/n64kr/blender_dkr_track.py),
//! optional morph targets (Blender shape keys).

use std::collections::HashMap;
use std::path::Path;

use super::{tex, Rom, TEXTURES_3D, TEXTURES_3D_TABLE};
use crate::json::{self, obj, Value};
use crate::png;

/// A texture: a 3D texture id from the ROM, or an image made here (sprites).
#[derive(Clone)]
pub struct TexInfo {
    pub id: u32,
    pub surface: i8,
    pub image: Option<(usize, usize, Vec<u8>)>,
    pub file: Option<String>,
}

impl TexInfo {
    pub fn rom(id: u32, surface: i8) -> Self {
        TexInfo { id, surface, image: None, file: None }
    }
}

/// Material group: texture slot (None = vertex colour only), invisible wall,
/// water, no collision, transparent batch.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Key {
    pub tex: Option<usize>,
    pub hidden: bool,
    pub water: bool,
    pub no_collision: bool,
    pub transparent: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Corner {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub rgba: [u8; 4],
    /// Texture coordinates in 1/32 texel units of the original texture.
    pub u: f64,
    pub v: f64,
}

/// Corner lists by material group, in the order groups were first used.
#[derive(Default)]
pub struct Groups {
    pub order: Vec<Key>,
    pub map: HashMap<Key, Vec<Corner>>,
}

impl Groups {
    pub fn entry(&mut self, key: Key) -> &mut Vec<Corner> {
        if !self.map.contains_key(&key) {
            self.order.push(key);
        }
        self.map.entry(key).or_default()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Key, &Vec<Corner>)> {
        self.order.iter().map(move |k| (k, &self.map[k]))
    }

    pub fn for_each_mut(&mut self, mut f: impl FnMut(&Key, &mut Vec<Corner>)) {
        for k in &self.order {
            f(k, self.map.get_mut(k).expect("key"));
        }
    }
}

/// DKR SurfaceType (include/enums.h) -> MK64 surface name as Fast64 spells it.
fn mk64_surface(surface: i8) -> &'static str {
    match surface {
        1 => "GRASS",              // SURFACE_GRASS
        2 => "SAND_OFFROAD",       // SURFACE_SAND
        3 => "BOOST_RAMP_ASPHALT", // SURFACE_ZIP_PAD
        4 => "DIRT_OFFROAD",       // SURFACE_STONE (DKR slows racers on stone)
        10 => "ICE",               // SURFACE_FROZEN_WATER
        11 | 14 | 15 => "WET_SAND", // SURFACE_WATER_CALM / _WAVY / _UNK_F
        13 => "SNOW",              // SURFACE_SNOW
        _ => "ASPHALT",            // SURFACE_DEFAULT (track) and anything else
    }
}
const DKR_SURFACE_INVIS_WALL: i8 = 17;

fn align4(buffer: &mut Vec<u8>) {
    while buffer.len() % 4 != 0 {
        buffer.push(0);
    }
}

/// Writes <dir>/<name>.gltf, <name>.bin and textures/*.png.
pub fn write(
    rom: &Rom,
    dir: &Path,
    name: &str,
    groups: &Groups,
    textures: &[TexInfo],
    targets: Option<(&HashMap<Key, Vec<Vec<[f64; 3]>>>, &[String])>,
) -> Result<(), String> {
    std::fs::create_dir_all(dir.join("textures")).map_err(|e| e.to_string())?;
    let (tex3d, tex3d_table) = (rom.section(TEXTURES_3D), rom.table(TEXTURES_3D_TABLE));
    let (mut images, mut samplers, mut gl_textures) = (vec![], vec![], vec![]);
    let mut slots: Vec<(usize, usize, &'static str)> = vec![]; // decoded w, h, alpha class
    for (t, info) in textures.iter().enumerate() {
        let (w, h, rgba, flags, file) = match &info.image {
            Some((w, h, rgba)) => (*w, *h, rgba.clone(), 0u16, format!("textures/{}.png", info.file.as_deref().unwrap_or("image"))),
            None => {
                let tx = tex::decode(rom.entry(tex3d, &tex3d_table, info.id as usize), 0)?;
                (tx.width, tx.height, tx.rgba, tx.flags, format!("textures/dkr_tex{:04}.png", info.id))
            }
        };
        std::fs::write(dir.join(&file), png::encode_rgba(w, h, &rgba)).map_err(|e| format!("{file}: {e}"))?;
        images.push(obj(vec![("uri", Value::Str(file))]));
        let wrap = |clamp: bool| Value::Int(if clamp { 33071 } else { 10497 });
        samplers.push(obj(vec![
            ("magFilter", Value::Int(9729)),
            ("minFilter", Value::Int(9729)),
            ("wrapS", wrap(flags & tex::TEX_FLAG_CLAMP_S != 0)),
            ("wrapT", wrap(flags & tex::TEX_FLAG_CLAMP_T != 0)),
        ]));
        gl_textures.push(obj(vec![("source", Value::Int(t as i64)), ("sampler", Value::Int(t as i64))]));
        let alphas = || rgba.iter().skip(3).step_by(4);
        let alpha = if alphas().all(|&a| a == 255) {
            "opaque"
        } else if alphas().all(|&a| a == 0 || a == 255) || flags & 0x10 != 0 {
            "cutout" // 1-bit alpha (texture flag 0x10 = cutout)
        } else {
            "blend"
        };
        slots.push((w, h, alpha));
    }

    let mut buffer: Vec<u8> = Vec::new();
    let (mut accessors, mut views) = (vec![], vec![]);
    let (mut materials, mut material_index): (Vec<Value>, HashMap<String, usize>) = (vec![], HashMap::new());
    let (mut visible, mut hidden) = (vec![], vec![]);
    let push_blob = |buffer: &mut Vec<u8>, views: &mut Vec<Value>, blob: &[u8]| -> usize {
        align4(buffer);
        views.push(obj(vec![
            ("buffer", Value::Int(0)),
            ("byteOffset", Value::Int(buffer.len() as i64)),
            ("byteLength", Value::Int(blob.len() as i64)),
        ]));
        buffer.extend_from_slice(blob);
        views.len() - 1
    };
    for (key, corners) in groups.iter() {
        if corners.is_empty() {
            continue;
        }
        let mut mat_name = match key.tex {
            None => "dkr_vertex_colour".to_string(),
            Some(t) => match &textures[t].file {
                Some(f) => f.clone(),
                None => format!("dkr_tex{:04}", textures[t].id),
            },
        };
        for (on, suffix) in [(key.water, "_water"), (key.no_collision, "_nocollide"), (key.transparent, "_transparent"), (key.hidden, "_hidden")] {
            if on {
                mat_name.push_str(suffix);
            }
        }
        if !material_index.contains_key(&mat_name) {
            let mut pbr = obj(vec![("metallicFactor", Value::Float(0.0)), ("roughnessFactor", Value::Float(1.0))]);
            if let Some(t) = key.tex {
                pbr.set("baseColorTexture", obj(vec![("index", Value::Int(t as i64))]));
            }
            let surface = key.tex.map_or(0, |t| textures[t].surface);
            let mut extras = if key.hidden || surface == DKR_SURFACE_INVIS_WALL {
                obj(vec![
                    ("mk64_surface", Value::Str("SURFACE_DEFAULT".into())),
                    ("mk64_clip", Value::Str("CLIP_DOUBLE_SIDED_WALL".into())),
                    ("mk64_draw", Value::Str("DRAW_INVISIBLE".into())),
                ])
            } else {
                // DKR marks transparency on the texture (alpha / header flags), not
                // only on the batch, so classify from the decoded texture.
                let mut alpha = key.tex.map_or("opaque", |t| slots[t].2);
                if key.transparent || key.water {
                    alpha = if alpha != "cutout" || key.water { "blend" } else { alpha };
                }
                obj(vec![
                    ("mk64_surface", Value::Str(mk64_surface(surface).into())),
                    ("mk64_clip", Value::Str(if key.no_collision { "CLIP_NONE" } else { "CLIP_DEFAULT" }.into())),
                    ("mk64_draw", Value::Str(if alpha == "blend" { "DRAW_TRANSLUCENT" } else { "DRAW_OPAQUE" }.into())),
                    ("mk64_alpha", Value::Str(alpha.into())),
                ])
            };
            let alpha_mode = match extras.get("mk64_alpha").and_then(Value::as_str) {
                Some("blend") => Some("BLEND"),
                Some("cutout") => Some("MASK"),
                _ => None,
            };
            extras.set("dkr_surface", Value::Int(surface as i64));
            let mut mat = obj(vec![("name", Value::Str(mat_name.clone())), ("pbrMetallicRoughness", pbr), ("doubleSided", Value::Bool(true))]);
            mat.set("extras", extras);
            if let Some(m) = alpha_mode {
                mat.set("alphaMode", Value::Str(m.into()));
            }
            material_index.insert(mat_name.clone(), materials.len());
            materials.push(mat);
        }
        let (w, h) = key.tex.map_or((32, 32), |t| (slots[t].0, slots[t].1));

        let (mut pos, mut col, mut uv) = (vec![], vec![], vec![]);
        let (mut mins, mut maxs) = ([1e9f64; 3], [-1e9f64; 3]);
        for c in corners {
            for (i, val) in [c.x, c.y, c.z].into_iter().enumerate() {
                pos.extend_from_slice(&(val as f32).to_le_bytes());
                mins[i] = mins[i].min(val);
                maxs[i] = maxs[i].max(val);
            }
            col.extend_from_slice(&c.rgba);
            uv.extend_from_slice(&((c.u / 32.0 / w as f64) as f32).to_le_bytes());
            uv.extend_from_slice(&((c.v / 32.0 / h as f64) as f32).to_le_bytes());
        }
        let count = Value::Int(corners.len() as i64);
        let mut attrs = Vec::new();
        for (blob, kind, ctype, attr) in [(&pos, "VEC3", 5126, "POSITION"), (&col, "VEC4", 5121, "COLOR_0"), (&uv, "VEC2", 5126, "TEXCOORD_0")] {
            let view = push_blob(&mut buffer, &mut views, blob);
            let mut acc = obj(vec![
                ("bufferView", Value::Int(view as i64)),
                ("componentType", Value::Int(ctype)),
                ("count", count.clone()),
                ("type", Value::Str(kind.into())),
            ]);
            match attr {
                "POSITION" => {
                    acc.set("min", json::floats(&mins));
                    acc.set("max", json::floats(&maxs));
                }
                "COLOR_0" => acc.set("normalized", Value::Bool(true)),
                _ => {}
            }
            accessors.push(acc);
            attrs.push((attr, Value::Int(accessors.len() as i64 - 1)));
        }
        let mut prim = obj(vec![
            ("attributes", obj(attrs)),
            ("material", Value::Int(material_index[&mat_name] as i64)),
            ("mode", Value::Int(4)),
        ]);
        if let Some((targets, _)) = targets.filter(|(t, _)| !t.is_empty()) {
            let mut list = vec![];
            for deltas in &targets[key] {
                let blob: Vec<u8> = deltas.iter().flat_map(|d| d.iter().flat_map(|v| (*v as f32).to_le_bytes())).collect();
                let view = push_blob(&mut buffer, &mut views, &blob);
                let lo: Vec<f64> = (0..3).map(|i| deltas.iter().map(|d| d[i]).fold(f64::INFINITY, f64::min)).collect();
                let hi: Vec<f64> = (0..3).map(|i| deltas.iter().map(|d| d[i]).fold(f64::NEG_INFINITY, f64::max)).collect();
                accessors.push(obj(vec![
                    ("bufferView", Value::Int(view as i64)),
                    ("componentType", Value::Int(5126)),
                    ("count", count.clone()),
                    ("type", Value::Str("VEC3".into())),
                    ("min", json::floats(&lo)),
                    ("max", json::floats(&hi)),
                ]));
                list.push(obj(vec![("POSITION", Value::Int(accessors.len() as i64 - 1))]));
            }
            prim.set("targets", Value::Arr(list));
        }
        if key.hidden { hidden.push(prim) } else { visible.push(prim) }
    }

    let (mut meshes, mut nodes) = (vec![], vec![]);
    for (prims, node_name) in [(visible, format!("{name}_visible")), (hidden, format!("{name}_invisible_walls"))] {
        if prims.is_empty() {
            continue;
        }
        let mut mesh = obj(vec![("name", Value::Str(node_name.clone())), ("primitives", Value::Arr(prims))]);
        if let Some((t, names)) = targets {
            if !t.is_empty() && !names.is_empty() {
                mesh.set("extras", obj(vec![("targetNames", Value::Arr(names.iter().map(|n| Value::Str(n.clone())).collect()))]));
            }
        }
        meshes.push(mesh);
        nodes.push(obj(vec![("name", Value::Str(node_name)), ("mesh", Value::Int(meshes.len() as i64 - 1))]));
    }

    let bin_name = format!("{name}.bin");
    std::fs::write(dir.join(&bin_name), &buffer).map_err(|e| e.to_string())?;
    let node_ids: Vec<i64> = (0..nodes.len() as i64).collect();
    let (n_materials, n_images) = (materials.len(), images.len());
    let gltf = obj(vec![
        ("asset", obj(vec![("version", Value::Str("2.0".into())), ("generator", Value::Str("N64KR n64kr (Rust)".into()))])),
        ("scene", Value::Int(0)),
        ("scenes", Value::Arr(vec![obj(vec![("name", Value::Str(name.into())), ("nodes", json::ints(&node_ids))])])),
        ("nodes", Value::Arr(nodes)),
        ("meshes", Value::Arr(meshes)),
        ("materials", Value::Arr(materials)),
        ("textures", Value::Arr(gl_textures)),
        ("images", Value::Arr(images)),
        ("samplers", Value::Arr(samplers)),
        ("accessors", Value::Arr(accessors)),
        ("bufferViews", Value::Arr(views)),
        ("buffers", Value::Arr(vec![obj(vec![("uri", Value::Str(bin_name)), ("byteLength", Value::Int(buffer.len() as i64))])])),
    ]);
    let path = dir.join(format!("{name}.gltf"));
    std::fs::write(&path, gltf.to_string_indent(1)).map_err(|e| e.to_string())?;
    let tris: usize = groups.iter().map(|(_, c)| c.len()).sum::<usize>() / 3;
    let hidden_tris: usize = groups.iter().filter(|(k, _)| k.hidden).map(|(_, c)| c.len()).sum::<usize>() / 3;
    println!("Wrote {}: {tris} triangles ({hidden_tris} invisible walls), {n_materials} materials, {n_images} textures.", path.display());
    Ok(())
}
