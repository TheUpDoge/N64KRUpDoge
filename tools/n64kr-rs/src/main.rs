//! n64kr: N64KR's asset tools. Each subcommand reads your own ROM and writes
//! generated content under build/ (never committed).

mod ge;
mod gltf;
mod lus;
mod n64audio;
mod png;
mod rarezip;
mod rom;
mod track;

use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
n64kr <command> [args]

Commands:
  ge-bg-info <GoldenEye ROM> <level>         List a GoldenEye level's rooms (vertices, display lists)
  ge-textures <GoldenEye ROM> <level> <dir>  Decode the textures a level uses to <dir>/tex_NNNN.png
  ge-images <GoldenEye ROM> <dir> <n>...    Decode images by number to <dir>/textures/tex_NNNN.png
  ge-gltf <GoldenEye ROM> <level> <dir>      Whole level as glTF (+ textures) in GoldenEye units, Y up
  ge-model <GoldenEye ROM> <file> <dir> [<anim> <frame>]
                                             An object model (e.g. PgasbarrelZ, ColiveguardZ): node tree, glTF;
                                             guards posed by an animation frame (idle, fire_standing, ...)
  ge-hazards <GoldenEye ROM> <mod dir>       Dam hazards for the engine: barrel, posed guards, effect sprites
                                             (libultraship resources in <mod dir>/ge/)
  ge-audio <GoldenEye ROM> <mod dir>         Music (rendered, looping) and sound effects as WAV for the engine
  ge-guard <GoldenEye ROM> <dir> <anim> <frame> [head]
                                             A Dam guard with head and KF7, posed, as glTF
  ge-dam-speedway <GoldenEye ROM> <dir> [a|b]
                                             Dam Speedway: banked oval over Dam's reservoir (b: twin Dams,
                                             longer oval), the outdoor level as scenery, for blender_dkr_track.py
  levels: dam, facility, runway, surface, bunker, silo, frigate, statue, archives, streets, ...
";

fn load_ge(rom_path: &str) -> Result<rom::Rom, String> {
    let rom = rom::Rom::load(&PathBuf::from(rom_path))?;
    let sha1 = rom::sha1_hex(&rom.data);
    if sha1 != ge::US_SHA1 {
        return Err(format!("expected the US GoldenEye ROM (SHA-1 {}), got {sha1}", ge::US_SHA1));
    }
    Ok(rom)
}

fn load_level(rom: &rom::Rom, level: &str) -> Result<(ge::bg::Bg, &'static str, f32), String> {
    let (bg_name, scale) = ge::level(level).ok_or_else(|| format!("unknown level '{level}'"))?;
    let (offset, size) = ge::bg_file(bg_name).ok_or_else(|| format!("no bg file {bg_name}"))?;
    Ok((ge::bg::parse(rom.slice(offset, size)?)?, bg_name, scale))
}

fn ge_textures(rom_path: &str, level: &str, dir: &str) -> Result<(), String> {
    let rom = load_ge(rom_path)?;
    let (bg, _, _) = load_level(&rom, level)?;
    let mut nums: Vec<usize> = bg
        .rooms
        .iter()
        .flat_map(|r| {
            let mut t = ge::bg::texture_refs(&r.primary_dl);
            if let Some(s) = &r.secondary_dl {
                t.extend(ge::bg::texture_refs(s));
            }
            t
        })
        .collect();
    nums.sort_unstable();
    nums.dedup();
    std::fs::create_dir_all(dir).map_err(|e| format!("{dir}: {e}"))?;
    let (mut ok, mut failed) = (0, 0);
    for n in &nums {
        match ge::tex::decode(&rom.data, *n) {
            Ok(t) => {
                let path = PathBuf::from(dir).join(format!("tex_{n:04}.png"));
                std::fs::write(&path, png::encode_rgba(t.width, t.height, &t.rgba))
                    .map_err(|e| format!("{}: {e}", path.display()))?;
                println!("tex {n:4}: {}x{} format {}", t.width, t.height, t.format);
                ok += 1;
            }
            Err(e) => {
                println!("tex {n:4}: FAILED {e}");
                failed += 1;
            }
        }
    }
    println!("{level}: {} textures, {ok} decoded, {failed} failed -> {dir}", nums.len());
    Ok(())
}

/// Writes `<mod dir>/mods.toml` (the port's mod manifest) if it is missing.
fn ensure_mod_manifest(mod_dir: &std::path::Path) -> Result<(), String> {
    let path = mod_dir.join("mods.toml");
    if path.exists() {
        return Ok(());
    }
    let name = mod_dir.file_name().and_then(|n| n.to_str()).unwrap_or("n64kr-mod");
    std::fs::create_dir_all(mod_dir).map_err(|e| format!("{}: {e}", mod_dir.display()))?;
    std::fs::write(&path, format!("[mod]
name = \"{name}\"
version = \"0.1.0\"
")).map_err(|e| format!("{}: {e}", path.display()))
}

/// Decoded GoldenEye textures written to `<dir>/textures/tex_NNNN.png` on
/// first use: (width, height, has cut-out alpha), None if undecodable.
struct TexCache<'a> {
    rom: &'a [u8],
    dir: PathBuf,
    info: std::collections::BTreeMap<usize, Option<(usize, usize, bool)>>,
}

impl<'a> TexCache<'a> {
    fn new(rom: &'a [u8], dir: &std::path::Path) -> Result<Self, String> {
        std::fs::create_dir_all(dir.join("textures")).map_err(|e| format!("{}: {e}", dir.display()))?;
        // Fast64 only converts textured materials, so untextured geometry gets a
        // plain white texture (vertex colour x white = the vertex colour).
        std::fs::write(dir.join("textures/n64kr_white.png"), png::encode_rgba(8, 8, &[255u8; 8 * 8 * 4]))
            .map_err(|e| e.to_string())?;
        Ok(TexCache { rom, dir: dir.to_path_buf(), info: Default::default() })
    }

    fn get(&mut self, n: usize) -> Result<Option<(usize, usize, bool)>, String> {
        if !self.info.contains_key(&n) {
            let info = match ge::tex::decode(self.rom, n) {
                Ok(t) => {
                    let file = format!("textures/tex_{n:04}.png");
                    std::fs::write(self.dir.join(&file), png::encode_rgba(t.width, t.height, &t.rgba))
                        .map_err(|e| format!("{file}: {e}"))?;
                    Some((t.width, t.height, t.rgba.chunks_exact(4).any(|p| p[3] < 128)))
                }
                Err(e) => {
                    eprintln!("warning: {e}");
                    None
                }
            };
            self.info.insert(n, info);
        }
        Ok(self.info[&n])
    }

    fn image(&mut self, n: usize) -> Result<Option<String>, String> {
        Ok(self.get(n)?.map(|_| format!("textures/tex_{n:04}.png")))
    }
}

/// glTF materials and primitives for GoldenEye triangles: one material per
/// (texture, translucent), positions through `place`, material extras from
/// `extras(translucent, alpha)` with alpha "opaque" / "cutout" / "blend".
fn ge_parts(
    tris: &[ge::dl::Triangle],
    cache: &mut TexCache,
    place: &dyn Fn([f32; 3]) -> [f32; 3],
    extras: &dyn Fn(bool, &str) -> String,
) -> Result<(Vec<gltf::Material>, Vec<gltf::Primitive>), String> {
    use std::collections::BTreeMap;
    let mut groups: BTreeMap<(Option<usize>, bool), Vec<&ge::dl::Triangle>> = BTreeMap::new();
    for t in tris {
        groups.entry((t.texture, t.translucent)).or_default().push(t);
    }
    let (mut materials, mut primitives) = (Vec::new(), Vec::new());
    for ((tex, translucent), list) in &groups {
        let info = match tex {
            Some(n) => cache.get(*n)?,
            None => None,
        };
        let (w, h) = info.map_or((8, 8), |(w, h, _)| (w, h));
        let cut = info.map_or(false, |i| i.2);
        let (alpha, gl_alpha) = if *translucent { ("blend", "BLEND") } else if cut { ("cutout", "MASK") } else { ("opaque", "OPAQUE") };
        materials.push(gltf::Material {
            name: format!("ge_{}{}", tex.map_or("untextured".into(), |n| format!("tex{n:04}")), if *translucent { "_xlu" } else { "" }),
            image: match (tex, info) {
                (Some(n), Some(_)) => Some(format!("textures/tex_{n:04}.png")),
                _ => Some("textures/n64kr_white.png".into()),
            },
            alpha_mode: gl_alpha,
            extras: extras(*translucent, alpha),
        });
        let mut p = gltf::Primitive { material: materials.len() - 1, positions: vec![], uvs: vec![], colors: vec![] };
        for t in list {
            for v in &t.v {
                p.positions.push(place(v.pos));
                p.uvs.push([v.st[0] / w as f32, v.st[1] / h as f32]);
                p.colors.push(v.rgba);
            }
        }
        primitives.push(p);
    }
    Ok((materials, primitives))
}

fn ge_gltf(rom_path: &str, level: &str, dir: &str) -> Result<(), String> {
    let rom = load_ge(rom_path)?;
    let (bg, bg_name, scale) = load_level(&rom, level)?;
    let mut tris = Vec::new();
    for room in &bg.rooms {
        ge::dl::room_triangles(&room.primary_dl, &room.vertices, room.position, false, &mut tris);
        if let Some(sec) = &room.secondary_dl {
            ge::dl::room_triangles(sec, &room.vertices, room.position, true, &mut tris);
        }
    }
    let out = PathBuf::from(dir);
    let mut cache = TexCache::new(&rom.data, &out)?;
    let (materials, primitives) = ge_parts(&tris, &mut cache, &|p| p, &|_, _| String::new())?;
    gltf::write(&out, level, &materials, &primitives)?;
    println!(
        "{level} ({bg_name}, level scale {scale}): {} rooms, {} triangles, {} materials, {} textures -> {}",
        bg.rooms.len(),
        tris.len(),
        materials.len(),
        cache.info.len(),
        out.join(format!("{level}.gltf")).display()
    );
    Ok(())
}

/// One object model: its node tree printed, posed (bind pose, or a guard
/// animation frame) as glTF.
fn ge_model(rom_path: &str, name: &str, dir: &str, anim: Option<(&str, &str)>) -> Result<(), String> {
    let rom = load_ge(rom_path)?;
    let model = ge::model::load(&rom.data, name)?;
    println!("{name}: textures {:?}", model.textures);
    for (i, g) in model.groups.iter().enumerate() {
        println!(
            "  group {i:2}: joint {:2} matrices {:?} parent {:?} origin ({:.1} {:.1} {:.1})",
            g.joint, g.matrices, g.parent, g.origin[0], g.origin[1], g.origin[2]
        );
    }
    for (group, segs, mtx) in model.dl_summary() {
        println!("  display list: group {group:?} vtx segments {segs:02x?} G_MTX {mtx}");
    }
    let posed = match anim {
        Some((anim_name, frame)) => {
            let a = ge::anim::load(&rom.data, anim_name)?;
            let frame: u16 = frame.parse().map_err(|_| format!("frame '{frame}'"))?;
            println!("  {anim_name}: {} frames, posing frame {frame}", a.frames);
            let angles = |joint: u16| ge::anim::guard_angles(&rom.data, &a, frame, joint);
            model.pose(Some(&angles), &ge::model::IDENTITY)
        }
        None => model.pose(None, &ge::model::IDENTITY),
    };
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for t in &posed.triangles {
        for v in &t.v {
            for k in 0..3 {
                lo[k] = lo[k].min(v.pos[k]);
                hi[k] = hi[k].max(v.pos[k]);
            }
        }
    }
    let head = model.head_group.map(|g| ge::model::apply(&posed.matrices[g], [0.0; 3]));
    let gunfire = model
        .gunfire
        .map(|(o, g)| ge::model::apply(g.map_or(&ge::model::IDENTITY, |g| &posed.matrices[g]), o));
    println!(
        "  {} triangles, bounds ({:.0} {:.0} {:.0}) .. ({:.0} {:.0} {:.0}), head at {head:?}, gunfire at {gunfire:?}",
        posed.triangles.len(),
        lo[0],
        lo[1],
        lo[2],
        hi[0],
        hi[1],
        hi[2]
    );
    let out = PathBuf::from(dir);
    let mut cache = TexCache::new(&rom.data, &out)?;
    let (materials, primitives) = ge_parts(&posed.triangles, &mut cache, &|p| p, &|_, _| String::new())?;
    gltf::write(&out, name, &materials, &primitives)
}

/// A Dam guard (olive uniform, body 2 in the Dam setup) posed by a guard
/// animation frame, with a head and a KF7 Soviet in the right hand, in
/// GoldenEye model units (feet near y = -1040, facing +Z). Returns the
/// triangles, the muzzle position and the grip (the barrel points grip -> muzzle).
fn guard_triangles(rom: &[u8], anim: &str, frame: u16, head: &str) -> Result<(Vec<ge::dl::Triangle>, [f32; 3], [f32; 3]), String> {
    use ge::model::{apply, load, IDENTITY};
    const RIGHT_WRIST: u16 = 8; // guard skeleton joint (SKEL_RT_WRIST)
    let body = load(rom, "ColiveguardZ")?;
    let a = ge::anim::load(rom, anim)?;
    let angles = |joint: u16| ge::anim::guard_angles(rom, &a, frame, joint);
    let posed = body.pose(Some(&angles), &IDENTITY);
    let mut tris = posed.triangles;
    if let Some(g) = body.head_group {
        tris.extend(load(rom, head)?.pose(None, &posed.matrices[g]).triangles);
    }
    // The KF7 in the hand's frame, turned by GUN_IN_HAND (its barrel lies along -x).
    let hand = body.groups.iter().position(|g| g.joint == RIGHT_WRIST).ok_or("guard has no right wrist")?;
    let grip = ge::model::mul(&posed.matrices[hand], &GUN_IN_HAND);
    let gun = load(rom, "PchrkalashZ")?;
    let gun_posed = gun.pose(None, &grip);
    tris.extend(gun_posed.triangles);
    let muzzle = gun
        .gunfire
        .map(|(o, g)| apply(g.map_or(&grip, |g| &gun_posed.matrices[g]), o))
        .unwrap_or_else(|| apply(&grip, [0.0; 3]));
    Ok((tris, muzzle, apply(&grip, [0.0; 3])))
}

fn ge_guard(rom_path: &str, dir: &str, anim: &str, frame: &str, head: &str) -> Result<(), String> {
    let rom = load_ge(rom_path)?;
    let frame: u16 = frame.parse().map_err(|_| format!("frame '{frame}'"))?;
    let (tris, muzzle, _) = guard_triangles(&rom.data, anim, frame, head)?;
    println!("guard {anim} frame {frame}: {} triangles, muzzle at ({:.0} {:.0} {:.0})", tris.len(), muzzle[0], muzzle[1], muzzle[2]);
    let out = PathBuf::from(dir);
    let mut cache = TexCache::new(&rom.data, &out)?;
    let (materials, primitives) = ge_parts(&tris, &mut cache, &|p| p, &|_, _| String::new())?;
    gltf::write(&out, "guard", &materials, &primitives)
}

/// GoldenEye triangles as one display list resource `name` (one material per
/// texture), positions through `place` into model space (16-bit).
fn ge_display_list(
    rom: &[u8],
    w: &mut lus::Writer,
    name: &str,
    tris: &[ge::dl::Triangle],
    place: &dyn Fn([f32; 3]) -> [f32; 3],
) -> Result<(), String> {
    use std::collections::BTreeMap;
    let mut by_tex: BTreeMap<Option<usize>, Vec<&ge::dl::Triangle>> = BTreeMap::new();
    for t in tris {
        by_tex.entry(t.texture).or_default().push(t);
    }
    let clamp16 = |v: f32| v.round().clamp(-32768.0, 32767.0) as i16;
    let mut body = Vec::new();
    for (k, (tex, list)) in by_tex.iter().enumerate() {
        // Texture coordinates: triangles carry texels of the original size; the
        // stored texture may be smaller (TMEM), so scale by stored / original.
        let (mat, su, sv) = match tex.and_then(|n| ge::tex::decode(rom, n).ok().map(|t| (n, t))) {
            Some((n, t)) => {
                let file = format!("tex_{n:04}");
                let (tw, th) = w.texture(&file, t.width, t.height, &t.rgba)?;
                (w.material(Some((&file, tw, th)), false)?, tw as f32 / t.width as f32, th as f32 / t.height as f32)
            }
            None => (w.material(None, false)?, 1.0, 1.0),
        };
        let corners: Vec<lus::Corner> = list
            .iter()
            .flat_map(|t| t.v.iter())
            .map(|v| {
                let p = place(v.pos);
                lus::Corner {
                    pos: [clamp16(p[0]), clamp16(p[1]), clamp16(p[2])],
                    st: [clamp16(v.st[0] * 32.0 * su), clamp16(v.st[1] * 32.0 * sv)],
                    rgba: v.rgba,
                }
            })
            .collect();
        body.push(w.call(&mat));
        body.extend(w.triangles(&format!("{name}_vtx{k}"), &corners)?);
    }
    w.display_list(name, &body)
}

/// The KF7 in the right hand's frame. The hand's +x runs along the fingers
/// (forward in the aim) and +y out of the palm (down); the gun's barrel lies
/// along its -x and the magazine along -z. Rows map gun x, y, z to hand
/// -x, -z, -y: barrel forward, magazine down [CONCLUSION: axes measured on
/// fire_standing frame 25, hand x (-0.63, 0.02, 0.77), y (0.21, -0.96, 0.20)].
const GUN_IN_HAND: ge::model::Mat = [[-1.0, 0.0, 0.0, 0.0], [0.0, 0.0, -1.0, 0.0], [0.0, -1.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0]];

/// Guard poses exported for the engine: idle, then fire_standing raising the
/// KF7 to the aim (frames picked from renders: 0 lowered .. 25 aimed).
const GUARD_POSES: &[(&str, u16)] = &[("idle", 0), ("fire_standing", 0), ("fire_standing", 8), ("fire_standing", 16), ("fire_standing", 25)];
/// Guard heads (GoldenEye gives Dam's guards random male heads).
const GUARD_HEADS: &[(&str, &str)] = &[("dave", "CheaddaveZ"), ("karl", "CheadkarlZ"), ("mark", "CheadmarkZ"), ("jim", "CheadjimZ")];
/// Effect sprites: (resource name, GoldenEye image) (decomp assets/images.def order).
const GE_FX: &[(&str, usize)] = &[("flash", 2119), ("tracer", 2128), ("fire", 2115), ("burst", 2084)];

/// GoldenEye models and effects for N64KR's Dam hazards (src/n64kr/ge_hazards.cpp),
/// as libultraship resources under <mod dir>/ge/ (ROM data: keep under build/).
fn ge_hazards(rom_path: &str, mod_dir: &str) -> Result<(), String> {
    let rom = load_ge(rom_path)?;
    ensure_mod_manifest(&PathBuf::from(mod_dir))?;
    let root = PathBuf::from(mod_dir).join("ge");
    let bounds = |tris: &[ge::dl::Triangle]| {
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for v in tris.iter().flat_map(|t| t.v.iter()) {
            for k in 0..3 {
                lo[k] = lo[k].min(v.pos[k]);
                hi[k] = hi[k].max(v.pos[k]);
            }
        }
        (lo, hi)
    };

    // Exploding barrel: GoldenEye's oil drum with hazard stripes, standing on y = 0, centred.
    let drum = ge::model::load(&rom.data, "Poil_drum1Z")?.pose(None, &ge::model::IDENTITY).triangles;
    let (lo, hi) = bounds(&drum);
    let (cx, cz) = ((lo[0] + hi[0]) / 2.0, (lo[2] + hi[2]) / 2.0);
    let mut w = lus::Writer::new(&root.join("barrel"), "ge/barrel")?;
    ge_display_list(&rom.data, &mut w, "model", &drum, &|p| [p[0] - cx, p[1] - lo[1], p[2] - cz])?;
    let radius = ((hi[0] - lo[0]).max(hi[2] - lo[2])) / 2.0;
    w.write("model.json", format!("{{\"file\":\"Poil_drum1Z\",\"height\":{:.1},\"radius\":{radius:.1}}}\n", hi[1] - lo[1]).as_bytes())?;
    println!("barrel: {} triangles, {:.0} tall, radius {radius:.0}", drum.len(), hi[1] - lo[1]);

    // Guards: feet on y = 0 (lowest point over all poses), facing +Z.
    for (head_name, head_file) in GUARD_HEADS {
        let mut poses = Vec::new();
        for (anim, frame) in GUARD_POSES {
            poses.push(guard_triangles(&rom.data, anim, *frame, head_file)?);
        }
        let feet = poses.iter().map(|p| bounds(&p.0).0[1]).fold(f32::MAX, f32::min);
        let top = poses.iter().map(|p| bounds(&p.0).1[1]).fold(f32::MIN, f32::max);
        let mut w = lus::Writer::new(&root.join(format!("guard_{head_name}")), &format!("ge/guard_{head_name}"))?;
        for (k, (tris, _, _)) in poses.iter().enumerate() {
            ge_display_list(&rom.data, &mut w, &format!("pose{k}"), tris, &|p| [p[0], p[1] - feet, p[2]])?;
        }
        let (_, muzzle, grip) = poses.last().unwrap();
        let aim_yaw = (muzzle[0] - grip[0]).atan2(muzzle[2] - grip[2]);
        let json = format!(
            "{{\"head\":\"{head_file}\",\"pose_count\":{},\"aim_pose\":{},\"height\":{:.1},\"muzzle\":[{:.1},{:.1},{:.1}],\"aim_yaw\":{aim_yaw:.4},\
             \"poses\":[{}]}}\n",
            poses.len(),
            poses.len() - 1,
            top - feet,
            muzzle[0],
            muzzle[1] - feet,
            muzzle[2],
            GUARD_POSES.iter().map(|(a, f)| format!("\"{a} {f}\"")).collect::<Vec<_>>().join(",")
        );
        w.write("model.json", json.as_bytes())?;
        println!("guard_{head_name}: {} poses, {:.0} tall, muzzle ({:.0} {:.0} {:.0}), aim yaw {:.1} deg",
            poses.len(), top - feet, muzzle[0], muzzle[1] - feet, muzzle[2], aim_yaw.to_degrees());
    }

    // Effects: one quad per sprite, 200 x 200 model units. Billboards (flash,
    // fire, burst) lie in the XY plane centred on the origin; the tracer runs
    // from z = 0 to z = 200 as two crossed quads, so it shows from any side.
    let mut w = lus::Writer::new(&root.join("fx"), "ge/fx")?;
    for (name, image) in GE_FX {
        let t = ge::tex::decode(&rom.data, *image)?;
        let file = format!("tex_{name}");
        let (tw, th) = w.texture(&file, t.width, t.height, &t.rgba)?;
        let mat = w.material(Some((&file, tw, th)), true)?;
        let (s1, t1) = ((tw * 32) as i16, (th * 32) as i16);
        let c = |x: i16, y: i16, z: i16, s: i16, t: i16| lus::Corner { pos: [x, y, z], st: [s, t], rgba: [255; 4] };
        let quads: Vec<[lus::Corner; 4]> = if *name == "tracer" {
            vec![
                [c(-12, 0, 0, 0, t1), c(12, 0, 0, s1, t1), c(12, 0, 200, s1, 0), c(-12, 0, 200, 0, 0)],
                [c(0, -12, 0, 0, t1), c(0, 12, 0, s1, t1), c(0, 12, 200, s1, 0), c(0, -12, 200, 0, 0)],
            ]
        } else {
            vec![[c(-100, -100, 0, 0, t1), c(100, -100, 0, s1, t1), c(100, 100, 0, s1, 0), c(-100, 100, 0, 0, 0)]]
        };
        let corners: Vec<lus::Corner> = quads.iter().flat_map(|q| [q[0], q[1], q[2], q[0], q[2], q[3]]).collect();
        let mut body = vec![w.call(&mat)];
        body.extend(w.triangles(&format!("{name}_vtx"), &corners)?);
        w.display_list(name, &body)?;
    }
    println!("effects: {} sprites -> {}", GE_FX.len(), root.display());
    Ok(())
}

/// Dam Speedway (RECON-008). A, the user's oval A: a banked stadium oval over
/// Dam's reservoir. B: two Dams facing each other, the second turned 180
/// degrees about the middle of the reservoir and joined to the first on a
/// seam across the water, with a longer oval and a dam wall behind each turn.
/// The level's outdoor rooms are non-solid scenery.
fn ge_dam_speedway(rom_path: &str, dir: &str, variant: &str) -> Result<(), String> {
    use std::collections::BTreeMap;
    let twin = match variant {
        "a" | "A" => false,
        "b" | "B" => true,
        _ => return Err(format!("variant '{variant}': expected a or b")),
    };
    let track = if twin { "dam_speedway_b" } else { "dam_speedway" };
    const SCALE: f32 = 0.35; // GoldenEye units -> MK64 units (road 126 wide)
    const INTERIOR_BELOW: f32 = -1500.0; // rooms placed lower are the underground bunker
    const WALL_TEXTURE: usize = 18; // red/white stripes
    // Pavement: Dam's own asphalt (64 x 32), the texture covering the most flat
    // ground in the level (the access road and walkways), per the user's ask.
    const ROAD_TEXTURE: usize = 22;
    let rom = load_ge(rom_path)?;
    let (bg, _, _) = load_level(&rom, "dam")?;
    let mut tris = Vec::new();
    for room in bg.rooms.iter().filter(|r| r.position[1] >= INTERIOR_BELOW) {
        ge::dl::room_triangles(&room.primary_dl, &room.vertices, room.position, false, &mut tris);
        if let Some(sec) = &room.secondary_dl {
            ge::dl::room_triangles(sec, &room.vertices, room.position, true, &mut tris);
        }
    }

    // Water level: the height with the most flat, upward-facing area (the reservoir).
    let mut area_at: BTreeMap<i32, f32> = BTreeMap::new();
    for t in &tris {
        let [a, b, c] = [t.v[0].pos, t.v[1].pos, t.v[2].pos];
        let (u, v) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
        let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if len > 0.0 && (n[1] / len).abs() > 0.99 {
            *area_at.entry(((a[1] + b[1] + c[1]) / 3.0).round() as i32).or_default() += len / 2.0;
        }
    }
    let water_y = *area_at.iter().max_by(|x, y| x.1.total_cmp(y.1)).map(|(y, _)| y).ok_or("no flat surfaces")? as f32;

    // Reservoir shores, measured on a 100-unit height map of the level (RECON-008):
    // water x -2,100 .. 2,900 at its narrowest (z -6,000), centred near x +200
    // from z -8,000 to -5,700; the dam's crest reaches z -1,000 at x ~0. Both
    // ovals keep the south turn's outer edge at z -1,222 (clear of the crest)
    // and the straights >= 170 units inside the water's edge.
    let oval = if twin {
        // Seam at z -7,400 (water -2,400 .. 2,600, mid +100): the second Dam is
        // this one turned about (250, -7,400), so the shores meet within 150.
        track::Oval {
            cx: 250.0,
            cz: -7400.0,
            half_straight: 3998.0,
            radius: 2000.0,
            base_y: water_y + 120.0,
            width: 360.0,
            bank_degrees: 25.0,
            bank_transition: 1500.0,
        }
    } else {
        track::Oval {
            cx: -150.0,
            cz: -5062.0,
            half_straight: 1960.0,
            radius: 1700.0,
            base_y: water_y + 120.0,
            width: 360.0,
            bank_degrees: 25.0,
            bank_transition: 1500.0,
        }
    };
    if twin {
        let (cx, cz) = (oval.cx, oval.cz);
        let south = ge::dl::clip(&tris, |p| p[2] - cz);
        let turned = south.iter().map(|t| {
            let mut t = t.clone();
            for v in &mut t.v {
                v.pos = [2.0 * cx - v.pos[0], v.pos[1], 2.0 * cz - v.pos[2]];
            }
            t
        });
        tris = south.iter().cloned().chain(turned).collect();
    }
    let meshes = track::build_oval(&oval, 60.0, 60.0, water_y);
    let start = oval.sample(0.0).across(oval.width / 2.0);
    let place = track::Placement { origin: start, scale: SCALE };

    let out = PathBuf::from(dir);
    let mut cache = TexCache::new(&rom.data, &out)?;
    let extras = |surface: &str, clip: &str, draw: &str, alpha: &str| {
        format!("\"mk64_surface\":\"{surface}\",\"mk64_clip\":\"{clip}\",\"mk64_draw\":\"{draw}\",\"mk64_alpha\":\"{alpha}\"")
    };
    // Scenery: non-solid.
    let (mut materials, mut primitives) = ge_parts(&tris, &mut cache, &|p| place.apply(p), &|translucent, alpha| {
        extras("ASPHALT", "CLIP_NONE", if translucent { "DRAW_TRANSLUCENT" } else { "DRAW_OPAQUE" }, alpha)
    })?;

    let (l, r) = (oval.half_straight, oval.radius);
    let turn = std::f32::consts::PI * r;
    let west = l + turn; // start of the west straight

    // Hazards (src/n64kr/ge_hazards.cpp): guards on sentry pads beside the
    // straights and on the dam crest(s) behind the turns; oil drum clusters
    // on the road, clear of the start line and the item box rows. Positions
    // along the lap are fractions of a straight (east: s from -L to L through
    // the start line at 0.5; west: from the end of the north turn).
    let east = |f: f32| (-l + 2.0 * l * f).rem_euclid(oval.lap_length());
    let west_at = |f: f32| west + 2.0 * l * f;
    // (s, outside the outer wall?)
    let pads: Vec<(f32, bool)> = if twin {
        vec![
            (east(0.1), true),
            (east(0.36), true),
            (east(0.62), false),
            (east(0.9), true),
            (west_at(0.12), true),
            (west_at(0.38), false),
            (west_at(0.62), true),
            (west_at(0.88), true),
        ]
    } else {
        vec![(east(0.12), true), (east(0.38), true), (west_at(0.2), false), (west_at(0.8), true)]
    };
    // Dam crest walkway (y -25, 55 above the road), measured on the height map:
    // cells along the arch from x -1,150 to 1,050.
    let mut crest: Vec<[f32; 3]> = vec![[-1150.0, -25.0, -650.0], [-550.0, -25.0, -750.0], [50.0, -25.0, -850.0], [650.0, -25.0, -750.0], [1050.0, -25.0, -650.0]];
    if twin {
        let turned: Vec<[f32; 3]> = crest.iter().map(|p| [2.0 * oval.cx - p[0], p[1], 2.0 * oval.cz - p[2]]).collect();
        crest.extend(turned);
    }
    const PAD_HALF: f32 = 40.0;
    const HEADS: [&str; 4] = ["dave", "karl", "mark", "jim"];
    let yaw_u16 = |from: [f32; 3], to: [f32; 3]| {
        let a = (to[0] - from[0]).atan2(to[2] - from[2]);
        ((a / std::f32::consts::TAU * 65536.0).round() as i64).rem_euclid(65536)
    };
    let mut actors: Vec<String> = Vec::new();
    let mut pad_top = track::Mesh::default();
    let mut pad_sides = track::Mesh::default();
    let mut guard = |at: [f32; 3], face: [f32; 3], n: usize| {
        let p = place.apply(at);
        actors.push(format!(
            "{{\"Name\":\"n64kr:ge_guard\",\"Location\":{{\"x\":{:.1},\"y\":{:.1},\"z\":{:.1}}},\
             \"Rotation\":{{\"pitch\":0,\"yaw\":{},\"roll\":0}},\"Skin\":\"{}\"}}",
            p[0],
            p[1],
            p[2],
            yaw_u16(at, face),
            HEADS[n % HEADS.len()]
        ));
    };
    let mut n_guards = 0;
    for &(s, outside) in &pads {
        let sm = oval.sample(s);
        let (edge, dir) = if outside { (sm.across(oval.width), 1.0) } else { (sm.across(0.0), -1.0) };
        let c = [edge[0] + sm.outward[0] * dir * (8.0 + PAD_HALF), edge[1], edge[2] + sm.outward[2] * dir * (8.0 + PAD_HALF)];
        let (t, sd) = track::pad(c, PAD_HALF, water_y);
        pad_top.positions.extend(t.positions);
        pad_top.uvs.extend(t.uvs);
        pad_top.colors.extend(t.colors);
        pad_sides.positions.extend(sd.positions);
        pad_sides.uvs.extend(sd.uvs);
        pad_sides.colors.extend(sd.colors);
        guard(c, sm.across(oval.width / 2.0), n_guards);
        n_guards += 1;
    }
    for &c in &crest {
        guard(c, [oval.cx, c[1], oval.cz], n_guards);
        n_guards += 1;
    }
    // Barrels: (s, lateral positions as fractions of the road width), patterns alternating the open lane.
    let clusters: Vec<f32> = if twin {
        vec![east(0.64), east(0.86), l + turn * 0.85, west_at(0.12), west_at(0.38), west_at(0.62), west_at(0.88), 3.0 * l + turn * 1.85]
    } else {
        vec![east(0.64), l + turn * 0.85, west_at(0.5), 3.0 * l + turn * 1.85]
    };
    const PATTERNS: [[f32; 3]; 2] = [[0.18, 0.31, 0.72], [0.28, 0.67, 0.81]];
    let mut n_barrels = 0;
    for (i, &s) in clusters.iter().enumerate() {
        let sm = oval.sample(s);
        for &f in &PATTERNS[i % 2] {
            let p = place.apply(sm.across(oval.width * f));
            actors.push(format!("{{\"Name\":\"n64kr:ge_barrel\",\"Location\":{{\"x\":{:.1},\"y\":{:.1},\"z\":{:.1}}}}}", p[0], p[1], p[2]));
            n_barrels += 1;
        }
    }
    let extra = format!("\"actors\":[{}],\"sequence\":{}", actors.join(","), 0x4E40); // N64KR_MUSIC_GE_DAM

    // Road (Dam's asphalt), walls (Dam's red/white stripes, solid both
    // sides), out-of-bounds sheet at the water (Lakitu rescue), sentry pads
    // (asphalt tops, striped sides, scenery: the walls keep karts off them).
    let road_image = cache.image(ROAD_TEXTURE)?;
    let wall_image = cache.image(WALL_TEXTURE)?;
    let parts = [
        ("n64kr_road", road_image.clone(), extras("ASPHALT", "CLIP_DEFAULT", "DRAW_OPAQUE", "opaque"), meshes.road),
        ("n64kr_wall", wall_image, extras("SURFACE_DEFAULT", "CLIP_DOUBLE_SIDED_WALL", "DRAW_OPAQUE", "opaque"), meshes.walls),
        // Never drawn, but Fast64 only converts textured materials (an untextured
        // one stops the export), so it borrows the road's.
        ("n64kr_out_of_bounds", road_image.clone(), extras("OUT_OF_BOUNDS", "CLIP_DEFAULT", "DRAW_INVISIBLE", "opaque"), meshes.out_of_bounds),
        ("n64kr_pad_top", road_image, extras("ASPHALT", "CLIP_NONE", "DRAW_OPAQUE", "opaque"), pad_top),
        ("n64kr_pad_side", cache.image(WALL_TEXTURE)?, extras("SURFACE_DEFAULT", "CLIP_NONE", "DRAW_OPAQUE", "opaque"), pad_sides),
    ];
    for (name, image, ex, mesh) in parts {
        materials.push(gltf::Material { name: name.into(), image, alpha_mode: "OPAQUE", extras: ex });
        primitives.push(gltf::Primitive {
            material: materials.len() - 1,
            positions: mesh.positions.iter().map(|&p| place.apply(p)).collect(),
            uvs: mesh.uvs,
            colors: mesh.colors,
        });
    }

    gltf::write(&out, track, &materials, &primitives)?;

    // Layout: AI path along the centre; item box rows on the straights and turn apexes.
    let path = track::path(&oval, &place, 33.0);
    let row_s: Vec<f32> = if twin {
        vec![0.5 * l, l + turn / 2.0, west + 0.5 * l, west + l, west + 1.5 * l, west + 2.0 * l + turn / 2.0, 3.5 * l + 2.0 * turn]
    } else {
        vec![0.5 * l, l + turn / 2.0, west + 0.66 * l, west + 1.33 * l, 3.0 * l + 1.5 * turn, 3.5 * l + 2.0 * turn]
    };
    let rows: Vec<[f32; 3]> = row_s
        .iter()
        .map(|&s| {
            let p = place.apply(oval.sample(s).across(oval.width / 2.0));
            [p[0], p[1] + 15.0, p[2]]
        })
        .collect();
    let layout = track::layout_json(track, SCALE, &path, &rows, [92, 120, 150], [178, 186, 196], &extra);
    std::fs::write(out.join(format!("{track}_mk64.json")), layout).map_err(|e| e.to_string())?;
    println!(
        "{track}: lap {:.0} MK64 units, road {:.0} wide, banking {} deg; water at y {water_y} (GE); {} scenery triangles, \
         {} materials, path {} points, {n_guards} guards, {n_barrels} barrels -> {}",
        oval.lap_length() * SCALE,
        oval.width * SCALE,
        oval.bank_degrees,
        tris.len(),
        materials.len(),
        path.len(),
        out.display()
    );
    Ok(())
}

fn ge_images(rom_path: &str, dir: &str, nums: &[&str]) -> Result<(), String> {
    let rom = load_ge(rom_path)?;
    let mut cache = TexCache::new(&rom.data, &PathBuf::from(dir))?;
    for n in nums {
        let n: usize = n.parse().map_err(|_| format!("image number '{n}'"))?;
        match cache.get(n)? {
            Some((w, h, cut)) => println!("image {n}: {w}x{h}{}", if cut { " (cut-out alpha)" } else { "" }),
            None => println!("image {n}: not decodable"),
        }
    }
    Ok(())
}

/// GoldenEye music and sound effects for N64KR (src/n64kr/sfx.cpp, music.cpp):
/// songs rendered to <mod>/music/n64kr/ge_<song>.wav with a loop file
/// (HMAS reads ge_<song>.json), effects to <mod>/sound/n64kr/ge/<name>.wav.
fn ge_audio(rom_path: &str, mod_dir: &str) -> Result<(), String> {
    use ge::audio::*;
    let rom = load_ge(rom_path)?;
    let out = PathBuf::from(mod_dir);
    ensure_mod_manifest(&out)?;
    let music_dir = out.join("music/n64kr");
    let sfx_dir = out.join("sound/n64kr/ge");
    for d in [&music_dir, &sfx_dir] {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }

    // Sound effects (with their chained layers), mixed at 22050 Hz mono.
    let sfx = n64audio::parse_bank(rom.slice(SFX_CTL.0, SFX_CTL.1)?, rom.slice(SFX_TBL.0, SFX_TBL.1)?, 0)?;
    let inst = sfx.instruments.first().and_then(|i| i.as_ref()).ok_or("sfx bank has no instrument 0")?;
    const RATE: f64 = 22050.0;
    for (name, id) in SFX {
        let mut mixed: Vec<f32> = Vec::new();
        let (mut next, mut at, mut layers) = (*id, 0.0f64, 0);
        while next != 0 && layers < 8 {
            let s = inst.sounds.get(next).ok_or_else(|| format!("sfx {next} not in the bank"))?;
            let k = s.keymap;
            let ratio = n64audio::cents_to_ratio((k.key_base as i32 * 100 + k.detune as i32 + SFX_PITCH_SHIFT_CENTS) as f64);
            let wave = &sfx.waves[s.wave];
            // One pass (and up to 2 s of a looping tail), stepped from the bank rate to RATE.
            let step = sfx.sample_rate as f64 * ratio / RATE;
            let len = match wave.looping {
                Some((ls, le, c)) if c < 0 => le + ((2.0 * RATE * step) as usize).min((le - ls) * 8),
                _ => wave.pcm.len(),
            };
            let gain = s.volume as f32 / 127.0 * s.envelope.attack_volume.max(1) as f32 / 127.0;
            let start = (at * RATE) as usize;
            let mut pos = 0.0f64;
            let mut f = 0;
            while (pos as usize) + 1 < len {
                let mut i = pos as usize;
                if let Some((ls, le, _)) = wave.looping {
                    while i >= le && le > ls {
                        i -= le - ls;
                    }
                }
                let v = *wave.pcm.get(i).unwrap_or(&0) as f32 / 32768.0 * gain;
                if mixed.len() <= start + f {
                    mixed.resize(start + f + 1, 0.0);
                }
                mixed[start + f] += v;
                pos += step;
                f += 1;
            }
            layers += 1;
            at += k.velocity_max as f64 * 0.033;
            next = k.velocity_min as usize + ((k.key_min & 0xC0) as usize) * 4;
            if next == *id {
                break;
            }
        }
        let peak = mixed.iter().fold(0.0f32, |m, &v| m.max(v.abs())).max(1e-6);
        let scale = if peak > 0.98 { 0.98 / peak } else { 1.0 };
        let pcm: Vec<i16> = mixed.iter().map(|&v| ((v * scale) * 32767.0).clamp(-32768.0, 32767.0) as i16).collect();
        std::fs::write(sfx_dir.join(format!("{name}.wav")), n64audio::wav_mono(RATE as u32, &pcm)).map_err(|e| e.to_string())?;
        println!("sfx {name} ({id}): {layers} layer(s), {:.2} s", pcm.len() as f64 / RATE);
    }

    // Music: each song rendered through its loop twice; the second pass is the loop.
    let bank = n64audio::parse_bank(rom.slice(MUSIC_CTL.0, MUSIC_CTL.1)?, rom.slice(MUSIC_TBL.0, MUSIC_TBL.1)?, 0)?;
    for (name, offset, size, volume) in SONGS {
        let seq = rarezip::inflate_1172(rom.slice(*offset, *size)?)?;
        let song = n64audio::parse_song(&seq)?;
        let (until, loop_ticks) = match song.song_loop {
            Some((s, e)) => (2 * e - s, Some((e, 2 * e - s))),
            None => (song.end, None),
        };
        let (samples, marks) = n64audio::render(&song, &bank, until, *volume as f64);
        let frame_at = |tick: u64| marks.iter().filter(|m| m.0 <= tick).last().map_or(0, |m| m.1);
        let peak = samples.iter().fold(0.0f32, |m, &v| m.max(v.abs())).max(1e-6);
        let gain = 0.89 / peak; // about -1 dB
        let file = format!("ge_{name}");
        std::fs::write(music_dir.join(format!("{file}.wav")), n64audio::wav(bank.sample_rate, &samples, gain)).map_err(|e| e.to_string())?;
        let frames = samples.len() / 2;
        let (ls, le) = match loop_ticks {
            Some((a, b)) => (frame_at(a), frame_at(b).min(frames)),
            None => (0, frames),
        };
        let json = format!(
            "{{\"name\":\"GoldenEye 007: {name}\",\"author\":\"Rare (from your ROM)\",\"loop\":{{\"start\":{ls},\"end\":{le}}}}}\n"
        );
        std::fs::write(music_dir.join(format!("{file}.json")), json).map_err(|e| e.to_string())?;
        println!(
            "music {name}: {} events, {:.1} s at {} Hz, loop {:?} -> frames {ls}..{le}, peak {peak:.2}",
            song.events.len(),
            frames as f64 / bank.sample_rate as f64,
            bank.sample_rate,
            song.song_loop
        );
    }
    Ok(())
}

fn ge_bg_info(rom_path: &str, level: &str) -> Result<(), String> {
    let rom = load_ge(rom_path)?;
    let (bg, bg_name, scale) = load_level(&rom, level)?;

    let (mut verts, mut pri, mut sec) = (0usize, 0usize, 0usize);
    for room in &bg.rooms {
        let v = room.vertices.len() / 16;
        let p = room.primary_dl.len() / 8;
        let s = room.secondary_dl.as_ref().map_or(0, |d| d.len() / 8);
        verts += v;
        pri += p;
        sec += s;
        println!(
            "room {:3}  at ({:8.0} {:8.0} {:8.0})  {:5} vertices  {:5} + {:4} DL commands",
            room.index, room.position[0], room.position[1], room.position[2], v, p, s
        );
    }
    println!(
        "{level} ({bg_name}, level scale {scale}): {} rooms, {verts} vertices, {pri} primary + {sec} secondary DL commands",
        bg.rooms.len()
    );
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["ge-bg-info", rom, level] => ge_bg_info(rom, level),
        ["ge-textures", rom, level, dir] => ge_textures(rom, level, dir),
        ["ge-gltf", rom, level, dir] => ge_gltf(rom, level, dir),
        ["ge-images", rom, dir, nums @ ..] if !nums.is_empty() => ge_images(rom, dir, nums),
        ["ge-model", rom, name, dir] => ge_model(rom, name, dir, None),
        ["ge-model", rom, name, dir, anim, frame] => ge_model(rom, name, dir, Some((anim, frame))),
        ["ge-hazards", rom, dir] => ge_hazards(rom, dir),
        ["ge-audio", rom, dir] => ge_audio(rom, dir),
        ["ge-guard", rom, dir, anim, frame] => ge_guard(rom, dir, anim, frame, "CheaddaveZ"),
        ["ge-guard", rom, dir, anim, frame, head] => ge_guard(rom, dir, anim, frame, head),
        ["ge-dam-speedway", rom, dir] => ge_dam_speedway(rom, dir, "a"),
        ["ge-dam-speedway", rom, dir, variant] => ge_dam_speedway(rom, dir, variant),
        _ => {
            eprint!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
