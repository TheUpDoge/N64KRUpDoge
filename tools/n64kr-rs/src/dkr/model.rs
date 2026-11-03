//! A DKR racer as an in-game 3D model for N64KR's 3D racers
//! (`n64kr dkr-racer-model`), drawn by src/n64kr/racer3d.c.
//!
//! Writes libultraship resources into <mod dir>/racers/<name>/ (ROM data:
//! keep under build/, never commit):
//!   tex_<n>               RGBA16 textures (DKR textures, wheel faces)
//!   mat_<n>               material display lists (combiner, texture load)
//!   pose<p>_<group>_vtx   vertex buffers, one per driving pose and material group
//!   pose<p>               one display list per pose: the body
//!   wheel<k>              wheels, each built around its centre (rolled / steered by the engine)
//!   model.json            pose count, scale, wheel centres (read by the engine)
//!
//! Model space: DKR model units, Y up, turned to face +Z (DKR models face -Z):
//! (x, y, z) -> (-x, y - ground, -z), so the tyres' bottom is y = 0. Poses are
//! frames of DKR's driving animation (0 full right, 40 straight, 73 full left),
//! baked per pose because the engine draws display lists, not vertex animation.
//!
//! Texture wrap follows the DKR texture header (0x40 / 0x80 clamp S / T).
//! Nearly every racer texture clamps and many UVs run far past the texture;
//! repeating them put extra eyes and fur patches on the racers (user playtest).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::racer;
use super::{tex, Rom, TEXTURES_3D, TEXTURES_3D_TABLE};
use crate::json::{self, obj, py_round, Value};

const POSE_FRAMES: [usize; 9] = [0, 10, 20, 30, 40, 48, 56, 64, 73]; // full right .. straight .. full left
/// World units per DKR model unit, applied by the engine (from model.json):
/// tuned on Diddy in playtests (0.06 x 1.3). A per-racer size multiplies it
/// (Banjo 1.2: DKR's own object scales hardly differ, but he races on DK's
/// heavyweight stats and read "a bit small").
const WORLD_SCALE: f64 = 0.078;
const VERTS_PER_LOAD: usize = 30;
const WHEEL_SIDES: usize = 10;
// DKR draws wheels as flat camera-facing sprites; as solid cylinders at the
// sprite's size they read too big (user playtests), so scale them down.
const WHEEL_RADIUS_SCALE: f64 = 0.6;
const WHEEL_WIDTH_SCALE: f64 = 0.45;

struct Writer {
    dir: PathBuf,
    prefix: String,
    materials: HashMap<String, String>,
}

/// Python's int(round(v)) clamped to s16.
fn clamp16(v: f64) -> i64 {
    super::round_half_even(v).clamp(-32768, 32767)
}

type Vtx = [i64; 9]; // x, y, z, s, t, r, g, b, a

impl Writer {
    fn res(&self, name: &str) -> String {
        format!("{}/{name}", self.prefix)
    }

    fn write(&self, name: &str, data: &[u8]) -> Result<(), String> {
        std::fs::write(self.dir.join(name), data).map_err(|e| format!("{name}: {e}"))
    }

    fn texture(&self, name: &str, w: usize, h: usize, rgba: &[u8]) -> Result<(usize, usize), String> {
        let (w, h, rgba) = tex::pow2_fit(w, h, rgba);
        let mut data = vec![0u8; 0x40];
        data[4..8].copy_from_slice(b"XETO");
        data[12..20].copy_from_slice(&[0xef, 0xbe, 0xad, 0xde, 0xef, 0xbe, 0xad, 0xde]);
        for v in [2u32, w as u32, h as u32, (w * h * 2) as u32] {
            data.extend_from_slice(&v.to_le_bytes());
        }
        for p in rgba.chunks_exact(4) {
            let v = ((p[0] as u16 >> 3) << 11) | ((p[1] as u16 >> 3) << 6) | ((p[2] as u16 >> 3) << 1) | (p[3] >= 128) as u16;
            data.extend_from_slice(&v.to_be_bytes());
        }
        self.write(name, &data)?;
        Ok((w, h))
    }

    /// A two-cycle material: cycle 1 = texel x shade colour, texel x primitive
    /// alpha (or shade only); cycle 2 = MK64's racer colour effect
    /// (func_8004B614: (1 - env) x colour + prim). No render mode: the engine
    /// sets it with the primitive alpha.
    fn material(&mut self, key: &str, tex: Option<(&str, usize, usize)>, clamp: (bool, bool)) -> Result<String, String> {
        if let Some(name) = self.materials.get(key) {
            return Ok(name.clone());
        }
        let name = format!("mat_{}", self.materials.len());
        let mut lines: Vec<String> = [
            "<DisplayList Version=\"0\">",
            "\t<PipeSync/>",
            "\t<SetGeometryMode G_SHADE=\"1\" G_SHADING_SMOOTH=\"1\" G_ZBUFFER=\"1\"/>",
            "\t<ClearGeometryMode G_LIGHTING=\"1\" G_FOG=\"1\" G_CULL_BACK=\"1\" G_CULL_FRONT=\"1\" G_TEXTURE_GEN=\"1\"/>",
            "\t<SetCycleType G_CYC_2CYCLE=\"1\"/>",
            "\t<SetTextureFilter Mode=\"G_TF_BILERP\"/>",
            "\t<SetTexturePersp Enable=\"G_TP_PERSP\"/>",
            "\t<SetTextureLUT Mode=\"G_TT_NONE\"/>",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let cycle2 = "A1=\"G_CCMUX_1\" B1=\"G_CCMUX_ENVIRONMENT\" C1=\"G_CCMUX_COMBINED\" D1=\"G_CCMUX_PRIMITIVE\" \
                      Aa1=\"G_ACMUX_0\" Ab1=\"G_ACMUX_0\" Ac1=\"G_ACMUX_0\" Ad1=\"G_ACMUX_COMBINED\"/>";
        match tex {
            Some((tex_name, w, h)) => {
                let cms = if clamp.0 { "G_TX_CLAMP" } else { "G_TX_WRAP" };
                let cmt = if clamp.1 { "G_TX_CLAMP" } else { "G_TX_WRAP" };
                let line = w * 2 / 8;
                lines.push(format!(
                    "\t<SetCombineLERP A0=\"G_CCMUX_TEXEL0\" B0=\"G_CCMUX_0\" C0=\"G_CCMUX_SHADE\" D0=\"G_CCMUX_0\" \
                     Aa0=\"G_ACMUX_TEXEL0\" Ab0=\"G_ACMUX_0\" Ac0=\"G_ACMUX_PRIMITIVE\" Ad0=\"G_ACMUX_0\" {cycle2}"
                ));
                lines.push("\t<Texture S=\"65535\" T=\"65535\" Level=\"0\" Tile=\"0\" On=\"1\"/>".into());
                lines.push(format!(
                    "\t<SetTextureImage Path=\"{}\" Format=\"G_IM_FMT_RGBA\" Size=\"G_IM_SIZ_16b_LOAD_BLOCK\" Width=\"1\"/>",
                    self.res(tex_name)
                ));
                lines.push(
                    "\t<SetTile Format=\"G_IM_FMT_RGBA\" Size=\"G_IM_SIZ_16b_LOAD_BLOCK\" Line=\"0\" TMem=\"0\" Tile=\"7\" Palette=\"0\" \
                     Cms0=\"G_TX_WRAP\" Cms1=\"G_TX_NOMIRROR\" Cmt0=\"G_TX_WRAP\" Cmt1=\"G_TX_NOMIRROR\" MaskS=\"0\" ShiftS=\"0\" MaskT=\"0\" ShiftT=\"0\"/>"
                        .into(),
                );
                lines.push("\t<LoadSync/>".into());
                lines.push(format!("\t<LoadBlock Tile=\"7\" Uls=\"0\" Ult=\"0\" Lrs=\"{}\" Dxt=\"{}\" />", w * h - 1, (2048 + line - 1) / line));
                lines.push("\t<PipeSync/>".into());
                lines.push(format!(
                    "\t<SetTile Format=\"G_IM_FMT_RGBA\" Size=\"G_IM_SIZ_16b\" Line=\"{line}\" TMem=\"0\" Tile=\"0\" Palette=\"0\" \
                     Cms0=\"{cms}\" Cms1=\"G_TX_NOMIRROR\" Cmt0=\"{cmt}\" Cmt1=\"G_TX_NOMIRROR\" \
                     MaskS=\"{}\" ShiftS=\"0\" MaskT=\"{}\" ShiftT=\"0\"/>",
                    w.trailing_zeros(),
                    h.trailing_zeros()
                ));
                lines.push(format!("\t<SetTileSize T=\"0\" Uls=\"0\" Ult=\"0\" Lrs=\"{}\" Lrt=\"{}\"/>", (w - 1) << 2, (h - 1) << 2));
            }
            None => {
                lines.push(format!(
                    "\t<SetCombineLERP A0=\"G_CCMUX_0\" B0=\"G_CCMUX_0\" C0=\"G_CCMUX_0\" D0=\"G_CCMUX_SHADE\" \
                     Aa0=\"G_ACMUX_SHADE\" Ab0=\"G_ACMUX_0\" Ac0=\"G_ACMUX_PRIMITIVE\" Ad0=\"G_ACMUX_0\" {cycle2}"
                ));
                lines.push("\t<Texture S=\"65535\" T=\"65535\" Level=\"0\" Tile=\"0\" On=\"0\"/>".into());
            }
        }
        lines.extend(["\t<EndDisplayList/>".to_string(), "</DisplayList>".to_string(), String::new()]);
        self.write(&name, lines.join("\n").as_bytes())?;
        self.materials.insert(key.to_string(), name.clone());
        Ok(name)
    }

    /// Vertex resource + display list lines drawing `corners` in loads of VERTS_PER_LOAD.
    fn tri_lists(&self, name: &str, corners: &[Vtx]) -> Result<Vec<String>, String> {
        let mut vtx = vec!["<Vertex Version=\"0\">".to_string()];
        for [x, y, z, s, t, r, g, b, a] in corners {
            vtx.push(format!("\t<Vtx X=\"{x}\" Y=\"{y}\" Z=\"{z}\" S=\"{s}\" T=\"{t}\" R=\"{r}\" G=\"{g}\" B=\"{b}\" A=\"{a}\"/>"));
        }
        vtx.extend(["</Vertex>".to_string(), String::new()]);
        self.write(name, vtx.join("\n").as_bytes())?;
        let mut lines = vec![];
        for start in (0..corners.len()).step_by(VERTS_PER_LOAD) {
            let count = VERTS_PER_LOAD.min(corners.len() - start);
            lines.push(format!(
                "\t<LoadVertices Path=\"{}\" VertexBufferIndex=\"0\" VertexOffset=\"{start}\" Count=\"{count}\"/>",
                self.res(name)
            ));
            for t in (0..count).step_by(3) {
                lines.push(format!("\t<Triangle1 V00=\"{t}\" V01=\"{}\" V02=\"{}\" Flag0=\"0\"/>", t + 1, t + 2));
            }
        }
        Ok(lines)
    }
}

type Point5 = (f64, f64, f64, f64, f64);

/// A wheel as a WHEEL_SIDES-sided cylinder round its attach point: two
/// textured discs (the sprite's side-on frame) and a vertex-coloured tread.
/// Returns (disc corners, tread corners, radius) in DKR model units and axes.
fn wheel_corners(wheel: &racer::Wheel, side_size: (usize, usize)) -> (Vec<Point5>, Vec<[f64; 3]>, f64) {
    let ((iw, ih), scale) = ((side_size.0 as f64, side_size.1 as f64), wheel.scale);
    let radius = ih * scale / 2.0 * WHEEL_RADIUS_SCALE;
    let width = wheel.spins[0][0].width as f64 * scale * WHEEL_WIDTH_SCALE; // head-on frame width
    let [cx, cy, cz] = wheel.position.map(|v| v as f64);
    let ring: Vec<(f64, f64)> = (0..WHEEL_SIDES)
        .map(|k| {
            let a = 2.0 * std::f64::consts::PI * k as f64 / WHEEL_SIDES as f64;
            (a.cos(), a.sin())
        })
        .collect();
    let mut discs = vec![];
    for side in [-1.0, 1.0] {
        let x = cx + side * width / 2.0;
        let centre = (x, cy, cz, iw * 16.0, ih * 16.0);
        for k in 0..WHEEL_SIDES {
            let ((c0, s0), (c1, s1)) = (ring[k], ring[(k + 1) % WHEEL_SIDES]);
            let p0 = (x, cy + s0 * radius, cz + c0 * radius, (0.5 + c0 / 2.0) * iw * 32.0, (0.5 - s0 / 2.0) * ih * 32.0);
            let p1 = (x, cy + s1 * radius, cz + c1 * radius, (0.5 + c1 / 2.0) * iw * 32.0, (0.5 - s1 / 2.0) * ih * 32.0);
            if side > 0.0 { discs.extend([centre, p0, p1]) } else { discs.extend([centre, p1, p0]) }
        }
    }
    let mut tread = vec![];
    for k in 0..WHEEL_SIDES {
        let ((c0, s0), (c1, s1)) = (ring[k], ring[(k + 1) % WHEEL_SIDES]);
        let a = [cx - width / 2.0, cy + s0 * radius, cz + c0 * radius];
        let b = [cx + width / 2.0, cy + s0 * radius, cz + c0 * radius];
        let c = [cx + width / 2.0, cy + s1 * radius, cz + c1 * radius];
        let d = [cx - width / 2.0, cy + s1 * radius, cz + c1 * radius];
        tread.extend([a, b, c, a, c, d]);
    }
    (discs, tread, radius)
}

/// `n64kr dkr-racer-model <rom> <object> <name> <mod dir> [size]`
pub fn export(rom: &Rom, object_name: &str, name: &str, mod_dir: &str, size: f64) -> Result<(), String> {
    let racer = racer::load(rom, object_name)?;
    let dir = Path::new(mod_dir).join("racers").join(name);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut writer = Writer { dir: dir.clone(), prefix: format!("racers/{name}"), materials: HashMap::new() };
    let (tex3d, tex3d_table) = (rom.section(TEXTURES_3D), rom.table(TEXTURES_3D_TABLE));

    // Body materials: one per texture (or vertex colour only); UVs are in
    // texels x 32 of the original size, rescaled if the texture was resampled.
    let mut group_mat: HashMap<super::gltf::Key, (String, f64, f64)> = HashMap::new();
    for (key, _) in racer.groups.iter() {
        let entry = match key.tex {
            None => (writer.material("vertex_colour", None, (false, false))?, 1.0, 1.0),
            Some(t) => {
                let tid = racer.textures[t].id;
                let tx = tex::decode(rom.entry(tex3d, &tex3d_table, tid as usize), 0)?;
                let tex_name = format!("tex_{tid:04}");
                let (tw, th) = writer.texture(&tex_name, tx.width, tx.height, &tx.rgba)?;
                let clamp = (tx.flags & tex::TEX_FLAG_CLAMP_S != 0, tx.flags & tex::TEX_FLAG_CLAMP_T != 0);
                let mat = writer.material(&format!("tex:{tid}"), Some((&tex_name, tw, th)), clamp)?;
                (mat, tw as f64 / tx.width as f64, th as f64 / tx.height as f64)
            }
        };
        group_mat.insert(*key, entry);
    }

    // Wheels: side-on frame (last angle, spin 0) on the discs, tread colour from the head-on frame.
    let (mut wheel_mats, mut wheel_geo) = (vec![], vec![]);
    let mut ground: Option<f64> = None;
    for wheel in &racer.wheels {
        let side = wheel.spins[0].last().ok_or("wheel sprite has no frames")?;
        let tex_name = format!("tex_wheel_{}", wheel.part);
        let (tw, th) = writer.texture(&tex_name, side.width, side.height, &side.rgba)?;
        let mat = writer.material(&format!("wheel:{}", wheel.part), Some((&tex_name, tw, th)), (false, false))?;
        let head = &wheel.spins[0][0];
        let opaque: Vec<&[u8]> = head.rgba.chunks_exact(4).filter(|p| p[3] >= 128).map(|p| &p[..3]).collect();
        let tread = if opaque.is_empty() {
            [60, 20, 20]
        } else {
            [0, 1, 2].map(|c| opaque.iter().map(|p| p[c] as i64).sum::<i64>() / opaque.len() as i64)
        };
        let (discs, tread_pts, radius) = wheel_corners(wheel, (tw, th));
        wheel_mats.push(mat);
        wheel_geo.push((discs, tread_pts, tread, wheel));
        let bottom = wheel.position[1] as f64 - radius;
        ground = Some(ground.map_or(bottom, |g| g.min(bottom)));
    }
    let ground = ground.unwrap_or(0.0);
    let to_model = |x: f64, y: f64, z: f64| [clamp16(-x), clamp16(y - ground), clamp16(-z)];

    let frames = &racer.anim_frames;
    for (p, &frame_index) in POSE_FRAMES.iter().enumerate() {
        let frame = if frames.is_empty() { None } else { Some(&frames[frame_index.min(frames.len() - 1)]) };
        let mut dl = vec!["<DisplayList Version=\"0\">".to_string()];
        for (gi, (key, corners)) in racer.groups.iter().enumerate() {
            let (mat_name, su, sv) = &group_mat[key];
            let out: Vec<Vtx> = racer.corner_vertex[key]
                .iter()
                .zip(corners)
                .map(|(v, c)| {
                    let (x, y, z) = match frame.and_then(|f| f.get(v)) {
                        Some(p) => (p[0] as f64, p[1] as f64, p[2] as f64),
                        None => (c.x, c.y, c.z),
                    };
                    let [mx, my, mz] = to_model(x, y, z);
                    let [r, g, b, a] = c.rgba.map(|v| v as i64);
                    [mx, my, mz, clamp16(c.u * su), clamp16(c.v * sv), r, g, b, a]
                })
                .collect();
            dl.push(format!("\t<CallDisplayList Path=\"{}\"/>", writer.res(mat_name)));
            dl.extend(writer.tri_lists(&format!("pose{p}_g{gi}_vtx"), &out)?);
        }
        dl.extend(["\t<PipeSync/>", "\t<EndDisplayList/>", "</DisplayList>", ""].map(String::from));
        writer.write(&format!("pose{p}"), dl.join("\n").as_bytes())?;
    }

    // Wheels: one display list each, built round the wheel centre so the
    // engine can roll it (about its axle, x) and steer the front ones (DKR
    // steers attach points 2 and 3).
    let mut wheels = vec![];
    for (k, (discs, tread_pts, tread, wheel)) in wheel_geo.iter().enumerate() {
        let centre = to_model(wheel.position[0] as f64, wheel.position[1] as f64, wheel.position[2] as f64);
        let rel = |x: f64, y: f64, z: f64| {
            let m = to_model(x, y, z);
            [m[0] - centre[0], m[1] - centre[1], m[2] - centre[2]]
        };
        let disc_out: Vec<Vtx> = discs
            .iter()
            .map(|&(x, y, z, s, t)| {
                let [a, b, c] = rel(x, y, z);
                [a, b, c, clamp16(s), clamp16(t), 255, 255, 255, 255]
            })
            .collect();
        let tread_out: Vec<Vtx> = tread_pts
            .iter()
            .map(|&[x, y, z]| {
                let [a, b, c] = rel(x, y, z);
                [a, b, c, 0, 0, tread[0], tread[1], tread[2], 255]
            })
            .collect();
        let mut dl = vec!["<DisplayList Version=\"0\">".to_string(), format!("\t<CallDisplayList Path=\"{}\"/>", writer.res(&wheel_mats[k]))];
        dl.extend(writer.tri_lists(&format!("wheel{k}_discs_vtx"), &disc_out)?);
        let vc = writer.material("vertex_colour", None, (false, false))?;
        dl.push(format!("\t<CallDisplayList Path=\"{}\"/>", writer.res(&vc)));
        dl.extend(writer.tri_lists(&format!("wheel{k}_tread_vtx"), &tread_out)?);
        dl.extend(["\t<PipeSync/>", "\t<EndDisplayList/>", "</DisplayList>", ""].map(String::from));
        writer.write(&format!("wheel{k}"), dl.join("\n").as_bytes())?;
        wheels.push(obj(vec![
            ("dl", Value::Str(format!("wheel{k}"))),
            ("centre", json::ints(&centre)),
            ("steers", Value::Bool(wheel.steers)),
        ]));
    }

    let info = obj(vec![
        ("object", Value::Str(object_name.into())),
        ("poses", json::ints(&POSE_FRAMES.map(|p| p as i64))),
        ("pose_count", Value::Int(POSE_FRAMES.len() as i64)),
        ("straight_pose", Value::Int(POSE_FRAMES.iter().position(|&p| p == 40).unwrap() as i64)),
        ("world_scale", Value::Float(py_round(WORLD_SCALE * size, 5))),
        ("size", Value::Float(size)),
        ("dkr_object_scale", Value::Float(racer.header.scale)),
        ("ground_dkr_units", Value::Float(ground)),
        ("wheels", Value::Arr(wheels)),
        ("space", Value::Str("DKR units, Y up, facing +Z, tyre bottom at y = 0".into())),
    ]);
    std::fs::write(dir.join("model.json"), info.to_string_indent(1)).map_err(|e| e.to_string())?;
    let tris: usize = racer.groups.iter().map(|(_, c)| c.len()).sum::<usize>() / 3;
    println!(
        "Racer model '{name}': {tris} body triangles, {} wheels, {} poses, {} materials -> {}",
        racer.wheels.len(),
        POSE_FRAMES.len(),
        writer.materials.len(),
        dir.display()
    );
    Ok(())
}
