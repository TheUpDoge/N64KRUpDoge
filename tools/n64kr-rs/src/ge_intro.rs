//! `ge-intro`: GoldenEye assets for N64KR's boot intro (src/n64kr/intro.cpp),
//! a staging of GoldenEye's gun barrel (decomp src/game/title.c):
//! - Bond (CdjbondZ, CheadbrosnanZ, the PP7 in his right hand) posed through
//!   bond_eye_walk and bond_eye_fire, one display list per pose, feet on y = 0,
//!   and where the muzzle is in each fire pose (the engine draws the flash).
//! - The barrel's rifling, the 440 x 299 image GoldenEye draws behind the
//!   opening one row at a time with a black-to-white gradient (title2.c
//!   titleRenderFolderMenuBackgroundLines), as one quad.

use crate::ge::dl::Triangle;
use crate::ge::model::{apply, load, Model, IDENTITY};
use crate::ge::{self, anim::Anim};
use crate::{ensure_mod_manifest, ge_display_list, load_ge, lus, png};
use std::collections::HashMap;
use std::path::PathBuf;

/// The rifling image: ROM offset (decomp scripts/filelist.u.csv,
/// ge007.u.2A4D50.usedby7F008DE4.bin), raw RLE (rle.c rle_expand_8bit).
const RIFLING: usize = 0x2A4D50;
/// Fire poses: every FIRE_STEP-th frame from FIRE_FIRST (title.c starts
/// bond_eye_fire at frame 2: modelSetAnimation(..., 2.0f, 0.91f, 16.0f)).
const FIRE_FIRST: u16 = 2;
const FIRE_STEP: u16 = 2;
/// The joint GoldenEye hangs the gun on: title.c attaches the PP7 to Switches[3]
/// of the body, which is the group of joint 9 in CdjbondZ, with no extra turn.
const GUN_JOINT: u16 = 9;

struct Bond {
    body: Model,
    head: Model,
    gun: Model,
}

struct Pose {
    tris: Vec<Triangle>,
    muzzle: [f32; 3],
    grip: [f32; 3],
}

impl Bond {
    fn load(rom: &[u8]) -> Result<Self, String> {
        Ok(Bond { body: load(rom, "CdjbondZ")?, head: load(rom, "CheadbrosnanZ")?, gun: load(rom, "PchrwppkZ")? })
    }

    /// Bond at an animation frame, his root (hips) moved by the frame's root motion.
    fn pose(&self, rom: &[u8], anim: &Anim, frame: u16) -> Result<Pose, String> {
        let (root, _) = ge::anim::root_motion(rom, anim, frame);
        let mut place = IDENTITY;
        place[3][..3].copy_from_slice(&root);
        let angles = |joint: u16| ge::anim::guard_angles(rom, anim, frame, joint);
        let posed = self.body.pose(Some(&angles), &place);
        let mut tris = posed.triangles;
        if let Some(g) = self.body.head_group {
            tris.extend(self.head.pose(None, &posed.matrices[g]).triangles);
        }
        let hand = self.body.groups.iter().position(|g| g.joint == GUN_JOINT).ok_or("Bond has no gun hand")?;
        let grip = posed.matrices[hand];
        let gun = self.gun.pose(None, &grip);
        tris.extend(gun.triangles.iter().cloned());
        let muzzle = self
            .gun
            .gunfire
            .map(|(o, g)| apply(g.map_or(&grip, |g| &gun.matrices[g]), o))
            .unwrap_or_else(|| apply(&grip, [0.0; 3]));
        Ok(Pose { tris, muzzle, grip: apply(&grip, [0.0; 3]) })
    }
}

fn bounds(tris: &[Triangle]) -> ([f32; 3], [f32; 3]) {
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for v in tris.iter().flat_map(|t| t.v.iter()) {
        for k in 0..3 {
            lo[k] = lo[k].min(v.pos[k]);
            hi[k] = hi[k].max(v.pos[k]);
        }
    }
    (lo, hi)
}

/// rle_expand_8bit: u16 width, u16 height, then (count, value) byte pairs from byte 10.
fn rifling(rom: &[u8]) -> Result<(usize, usize, Vec<u8>), String> {
    let src = rom.get(RIFLING..).ok_or("rifling image outside the ROM")?;
    let (w, h) = (crate::rom::u16_be(src, 0) as usize, crate::rom::u16_be(src, 2) as usize);
    let mut out = Vec::with_capacity(w * h);
    let mut i = 10;
    while out.len() < w * h {
        let (count, value) = (*src.get(i).ok_or("rifling image truncated")? as usize, src[i + 1]);
        out.extend(std::iter::repeat(value).take(count.max(1)));
        i += 2;
    }
    out.truncate(w * h);
    Ok((w, h, out))
}

/// Quick software render for checking poses: orthographic from `yaw` (0: the
/// camera on +x looking down -x, screen right = -z), nearest texel x vertex colour.
fn render(rom: &[u8], textures: &mut HashMap<usize, Option<ge::tex::Texture>>, tris: &[Triangle], yaw: f32, view: f32, size: usize) -> Vec<u8> {
    let (s, c) = yaw.sin_cos();
    let mut img = vec![40u8; size * size * 4];
    let mut depth = vec![f32::MIN; size * size];
    let k = size as f32 / view;
    for t in tris {
        let p: Vec<[f32; 3]> = t
            .v
            .iter()
            .map(|v| {
                let (x, y, z) = (v.pos[0], v.pos[1], v.pos[2]);
                // right = (s, 0, -c), toward the camera = (c, 0, s)
                [size as f32 / 2.0 + (x * s - z * c) * k, size as f32 - 8.0 - y * k, x * c + z * s]
            })
            .collect();
        let area = (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[2][0] - p[0][0]) * (p[1][1] - p[0][1]);
        if area.abs() < 1e-6 {
            continue;
        }
        let tex = t.texture.and_then(|n| textures.entry(n).or_insert_with(|| ge::tex::decode(rom, n).ok()).as_ref());
        let x0 = p.iter().map(|q| q[0]).fold(f32::MAX, f32::min).max(0.0) as usize;
        let x1 = (p.iter().map(|q| q[0]).fold(f32::MIN, f32::max) as usize + 1).min(size);
        let y0 = p.iter().map(|q| q[1]).fold(f32::MAX, f32::min).max(0.0) as usize;
        let y1 = (p.iter().map(|q| q[1]).fold(f32::MIN, f32::max) as usize + 1).min(size);
        for py in y0..y1 {
            for px in x0..x1 {
                let (fx, fy) = (px as f32 + 0.5, py as f32 + 0.5);
                let e = |a: &[f32; 3], b: &[f32; 3]| (b[0] - a[0]) * (fy - a[1]) - (fx - a[0]) * (b[1] - a[1]);
                let (w0, w1, w2) = (e(&p[1], &p[2]) / area, e(&p[2], &p[0]) / area, e(&p[0], &p[1]) / area);
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                let d = w0 * p[0][2] + w1 * p[1][2] + w2 * p[2][2];
                let at = py * size + px;
                if d <= depth[at] {
                    continue;
                }
                let mut rgb = [0f32; 3];
                for (i, w) in [w0, w1, w2].iter().enumerate() {
                    for ch in 0..3 {
                        rgb[ch] += w * t.v[i].rgba[ch] as f32;
                    }
                }
                if let Some(tx) = tex {
                    let st = [0, 1].map(|j| w0 * t.v[0].st[j] + w1 * t.v[1].st[j] + w2 * t.v[2].st[j]);
                    let u = (st[0].floor() as i64).rem_euclid(tx.width as i64) as usize;
                    let v = (st[1].floor() as i64).rem_euclid(tx.height as i64) as usize;
                    let texel = &tx.rgba[(v * tx.width + u) * 4..];
                    if texel[3] < 128 {
                        continue;
                    }
                    for ch in 0..3 {
                        rgb[ch] *= texel[ch] as f32 / 255.0;
                    }
                }
                depth[at] = d;
                for ch in 0..3 {
                    img[at * 4 + ch] = rgb[ch].clamp(0.0, 255.0) as u8;
                }
                img[at * 4 + 3] = 255;
            }
        }
    }
    img
}

/// Writes <mod dir>/ge/intro/ (bond/, barrel/); with `preview`, contact sheets of the poses there.
pub fn ge_intro(rom_path: &str, mod_dir: &str, preview: Option<&str>) -> Result<(), String> {
    let rom = load_ge(rom_path)?;
    let rom = &rom.data;
    ensure_mod_manifest(&PathBuf::from(mod_dir))?;
    let root = PathBuf::from(mod_dir).join("ge/intro");

    // Bond: every walk frame, then every FIRE_STEP-th fire frame.
    let bond = Bond::load(rom)?;
    let walk = ge::anim::load(rom, "bond_eye_walk")?;
    let fire = ge::anim::load(rom, "bond_eye_fire")?;
    let mut poses = Vec::new();
    for f in 0..walk.frames {
        poses.push((format!("walk{f:02}"), bond.pose(rom, &walk, f)?));
    }
    let fire_frames: Vec<u16> = (FIRE_FIRST..fire.frames).step_by(FIRE_STEP as usize).collect();
    for &f in &fire_frames {
        poses.push((format!("fire{f:03}"), bond.pose(rom, &fire, f)?));
    }
    for (name, p) in poses.iter().step_by(6) {
        let (lo, hi) = bounds(&p.tris);
        println!("  {name}: feet {:.0}, top {:.0}, x {:.0}..{:.0}, z {:.0}..{:.0}", lo[1], hi[1], lo[0], hi[0], lo[2], hi[2]);
    }
    // Feet on y = 0 (the lowest point over the walk), centred on the walk's hips.
    let feet = poses[..walk.frames as usize].iter().map(|(_, p)| bounds(&p.tris).0[1]).fold(f32::MAX, f32::min);
    let top = poses.iter().map(|(_, p)| bounds(&p.tris).1[1]).fold(f32::MIN, f32::max);
    let (hips, _) = ge::anim::root_motion(rom, &walk, 0);
    let place = |p: [f32; 3]| [p[0] - hips[0], p[1] - feet, p[2] - hips[2]];
    let mut w = lus::Writer::new(&root.join("bond"), "ge/intro/bond")?;
    for (name, p) in &poses {
        ge_display_list(rom, &mut w, name, &p.tris, &place)?;
    }
    let shot = poses.len() - fire_frames.len();
    let muzzles: Vec<String> = poses[shot..]
        .iter()
        .map(|(_, p)| {
            let m = place(p.muzzle);
            format!("[{:.1},{:.1},{:.1}]", m[0], m[1], m[2])
        })
        .collect();
    let aim: Vec<String> = poses[shot..]
        .iter()
        .map(|(_, p)| format!("{:.4}", (p.muzzle[0] - p.grip[0]).atan2(p.muzzle[2] - p.grip[2])))
        .collect();
    let json = format!(
        "{{\"body\":\"CdjbondZ\",\"head\":\"CheadbrosnanZ\",\"gun\":\"PchrwppkZ\",\"height\":{:.1},\
         \"walk_count\":{},\"fire_first\":{FIRE_FIRST},\"fire_step\":{FIRE_STEP},\"fire_count\":{},\"fire_frames\":{},\
         \"muzzle\":[{}],\"aim_yaw\":[{}]}}\n",
        top - feet,
        walk.frames,
        fire_frames.len(),
        fire.frames,
        muzzles.join(","),
        aim.join(",")
    );
    w.write("model.json", json.as_bytes())?;
    println!(
        "bond: {} walk + {} fire poses, {:.0} tall, {} triangles in the first pose",
        walk.frames,
        fire_frames.len(),
        top - feet,
        poses[0].1.tris.len()
    );

    // The rifling: a stub texture (TMEM size) with the full image as a PNG next
    // to it (the port loads the PNG; texture coordinates follow the stub's tile).
    // The quad spans GoldenEye's 440 x 330 screen as 1280 x 960 units, rows 16..315,
    // shaded black at the top to white at the bottom.
    let (iw, ih, grey) = rifling(rom)?;
    let rgba: Vec<u8> = grey.iter().flat_map(|&g| [g, g, g, 255]).collect();
    let mut w = lus::Writer::new(&root.join("barrel"), "ge/intro/barrel")?;
    let (tw, th) = w.texture("tex_rifling", iw, ih, &rgba)?;
    w.write("tex_rifling.png", &png::encode_rgba(iw, ih, &rgba))?;
    let mat = w.material(Some(("tex_rifling", tw, th)), false)?;
    let (top_y, bottom_y) = ((960.0 - 16.0 * 960.0 / 330.0) as i16, (960.0 - 315.0 * 960.0 / 330.0) as i16);
    let (s1, t1) = ((tw * 32) as i16, (th * 32) as i16);
    let c = |x: i16, y: i16, s: i16, t: i16, l: u8| lus::Corner { pos: [x, y, 0], st: [s, t], rgba: [l, l, l, 255] };
    let quad = [c(0, bottom_y, 0, t1, 255), c(1280, bottom_y, s1, t1, 255), c(1280, top_y, s1, 0, 0), c(0, top_y, 0, 0, 0)];
    let corners = [quad[0], quad[1], quad[2], quad[0], quad[2], quad[3]];
    let mut body = vec![w.call(&mat)];
    body.extend(w.triangles("rifling_vtx", &corners)?);
    w.display_list("rifling", &body)?;
    println!("barrel: rifling {iw}x{ih} -> {}", root.display());

    if let Some(dir) = preview {
        std::fs::create_dir_all(dir).map_err(|e| format!("{dir}: {e}"))?;
        std::fs::write(PathBuf::from(dir).join("rifling.png"), png::encode_grey(iw, ih, &grey)).map_err(|e| e.to_string())?;
        let mut textures = HashMap::new();
        let cell = 200;
        for (sheet, yaw, pick) in [("side", 0.0f32, 1usize), ("front", std::f32::consts::FRAC_PI_2, 1)] {
            let chosen: Vec<&(String, Pose)> = poses.iter().step_by(4 * pick).collect();
            let cols = 10;
            let rows = chosen.len().div_ceil(cols);
            let mut img = vec![0u8; cols * cell * rows * cell * 4];
            for (i, (_, p)) in chosen.iter().enumerate() {
                let mut tris = p.tris.clone();
                for t in &mut tris {
                    for v in &mut t.v {
                        v.pos = place(v.pos);
                    }
                }
                let one = render(rom, &mut textures, &tris, yaw, 1900.0, cell);
                let (ox, oy) = ((i % cols) * cell, (i / cols) * cell);
                for y in 0..cell {
                    let row = ((oy + y) * cols * cell + ox) * 4;
                    img[row..row + cell * 4].copy_from_slice(&one[y * cell * 4..(y + 1) * cell * 4]);
                }
            }
            std::fs::write(PathBuf::from(dir).join(format!("bond_{sheet}.png")), png::encode_rgba(cols * cell, rows * cell, &img))
                .map_err(|e| e.to_string())?;
        }
        println!("preview: {dir}");
    }
    Ok(())
}
