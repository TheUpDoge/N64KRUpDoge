//! n64kr: N64KR's asset tools. Each subcommand reads your own ROM and writes
//! generated content under build/ (never committed).

mod ge;
mod gltf;
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
  ge-gltf <GoldenEye ROM> <level> <dir>      Whole level as glTF (+ textures) in GoldenEye units, Y up
  ge-dam-speedway <GoldenEye ROM> <dir>      Dam Speedway: banked oval over Dam's reservoir, with the
                                             outdoor level as scenery, ready for blender_dkr_track.py
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

fn ge_gltf(rom_path: &str, level: &str, dir: &str) -> Result<(), String> {
    use std::collections::BTreeMap;
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
    std::fs::create_dir_all(out.join("textures")).map_err(|e| format!("{dir}: {e}"))?;

    // One material per (texture, translucent).
    let mut groups: BTreeMap<(Option<usize>, bool), Vec<&ge::dl::Triangle>> = BTreeMap::new();
    for t in &tris {
        groups.entry((t.texture, t.translucent)).or_default().push(t);
    }
    let mut sizes: BTreeMap<usize, (usize, usize, bool)> = BTreeMap::new();
    let mut materials = Vec::new();
    let mut primitives = Vec::new();
    for ((tex, translucent), list) in &groups {
        let (image, size, cutout) = match tex {
            Some(n) => {
                if !sizes.contains_key(n) {
                    match ge::tex::decode(&rom.data, *n) {
                        Ok(t) => {
                            let file = format!("textures/tex_{n:04}.png");
                            std::fs::write(out.join(&file), png::encode_rgba(t.width, t.height, &t.rgba))
                                .map_err(|e| format!("{file}: {e}"))?;
                            let cut = t.rgba.chunks_exact(4).any(|p| p[3] < 128);
                            sizes.insert(*n, (t.width, t.height, cut));
                        }
                        Err(e) => {
                            eprintln!("warning: {e}");
                            sizes.insert(*n, (0, 0, false));
                        }
                    }
                }
                let (w, h, cut) = sizes[n];
                if w == 0 {
                    (None, (1, 1), false)
                } else {
                    (Some(format!("textures/tex_{n:04}.png")), (w, h), cut)
                }
            }
            None => (None, (1, 1), false),
        };
        let alpha_mode = if *translucent { "BLEND" } else if cutout { "MASK" } else { "OPAQUE" };
        materials.push(gltf::Material {
            name: format!("ge_{}{}", tex.map_or("untextured".into(), |n| format!("tex{n:04}")), if *translucent { "_xlu" } else { "" }),
            image,
            alpha_mode,
            extras: String::new(),
        });
        let mut p = gltf::Primitive { material: materials.len() - 1, positions: vec![], uvs: vec![], colors: vec![] };
        for t in list {
            for v in &t.v {
                p.positions.push(v.pos);
                p.uvs.push([v.st[0] / size.0 as f32, v.st[1] / size.1 as f32]);
                p.colors.push(v.rgba);
            }
        }
        primitives.push(p);
    }
    gltf::write(&out, level, &materials, &primitives)?;
    println!(
        "{level} ({bg_name}, level scale {scale}): {} rooms, {} triangles, {} materials, {} textures -> {}",
        bg.rooms.len(),
        tris.len(),
        materials.len(),
        sizes.len(),
        out.join(format!("{level}.gltf")).display()
    );
    Ok(())
}

/// Dam Speedway (RECON-008, the user's oval A): a banked stadium oval over
/// Dam's reservoir; the level's outdoor rooms are non-solid scenery.
fn ge_dam_speedway(rom_path: &str, dir: &str) -> Result<(), String> {
    use std::collections::BTreeMap;
    const TRACK: &str = "dam_speedway";
    const SCALE: f32 = 0.35; // GoldenEye units -> MK64 units (lap ~6,500, road ~126 wide)
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

    // The oval: long axis north-south, inside the reservoir's shore (measured
    // on the top-down map: water z -9187 .. -938 at the dam's inner face),
    // ~250 units clear of the north cliffs and of the dam crest.
    let oval = track::Oval {
        cx: -345.0,
        cz: -5062.0,
        half_straight: 1960.0,
        radius: 1700.0,
        base_y: water_y + 120.0,
        width: 360.0,
        bank_degrees: 25.0,
        bank_transition: 1500.0,
    };
    let meshes = track::build_oval(&oval, 60.0, 60.0, water_y);
    let start = oval.sample(0.0).across(oval.width / 2.0);
    let place = track::Placement { origin: start, scale: SCALE };

    let out = PathBuf::from(dir);
    std::fs::create_dir_all(out.join("textures")).map_err(|e| format!("{dir}: {e}"))?;
    let mut materials: Vec<gltf::Material> = Vec::new();
    let mut primitives: Vec<gltf::Primitive> = Vec::new();
    let extras = |surface: &str, clip: &str, draw: &str, alpha: &str| {
        format!("\"mk64_surface\":\"{surface}\",\"mk64_clip\":\"{clip}\",\"mk64_draw\":\"{draw}\",\"mk64_alpha\":\"{alpha}\"")
    };

    // Scenery: one material per (texture, translucent), non-solid.
    let mut groups: BTreeMap<(Option<usize>, bool), Vec<&ge::dl::Triangle>> = BTreeMap::new();
    for t in &tris {
        groups.entry((t.texture, t.translucent)).or_default().push(t);
    }
    let mut decoded: BTreeMap<usize, Option<(usize, usize, bool)>> = BTreeMap::new();
    let mut texture = |n: usize| -> Result<Option<(usize, usize, bool)>, String> {
        if !decoded.contains_key(&n) {
            let info = match ge::tex::decode(&rom.data, n) {
                Ok(t) => {
                    let file = format!("textures/tex_{n:04}.png");
                    std::fs::write(out.join(&file), png::encode_rgba(t.width, t.height, &t.rgba))
                        .map_err(|e| format!("{file}: {e}"))?;
                    Some((t.width, t.height, t.rgba.chunks_exact(4).any(|p| p[3] < 128)))
                }
                Err(e) => {
                    eprintln!("warning: {e}");
                    None
                }
            };
            decoded.insert(n, info);
        }
        Ok(decoded[&n])
    };
    // Fast64 only converts textured materials, so untextured geometry gets a
    // plain white texture (vertex colour x white = the vertex colour).
    std::fs::write(out.join("textures/n64kr_white.png"), png::encode_rgba(8, 8, &[255u8; 8 * 8 * 4]))
        .map_err(|e| e.to_string())?;
    for ((tex, translucent), list) in &groups {
        let info = match tex {
            Some(n) => texture(*n)?,
            None => None,
        };
        let (w, h) = info.map_or((8, 8), |(w, h, _)| (w, h));
        let cut = info.map_or(false, |i| i.2);
        let (alpha, gl_alpha) = if *translucent { ("blend", "BLEND") } else if cut { ("cutout", "MASK") } else { ("opaque", "OPAQUE") };
        let draw = if *translucent { "DRAW_TRANSLUCENT" } else { "DRAW_OPAQUE" };
        materials.push(gltf::Material {
            name: format!("ge_{}{}", tex.map_or("untextured".into(), |n| format!("tex{n:04}")), if *translucent { "_xlu" } else { "" }),
            image: match (tex, info) {
                (Some(n), Some(_)) => Some(format!("textures/tex_{n:04}.png")),
                _ => Some("textures/n64kr_white.png".into()),
            },
            alpha_mode: gl_alpha,
            extras: extras("ASPHALT", "CLIP_NONE", draw, alpha),
        });
        let mut p = gltf::Primitive { material: materials.len() - 1, positions: vec![], uvs: vec![], colors: vec![] };
        for t in list {
            for v in &t.v {
                p.positions.push(place.apply(v.pos));
                p.uvs.push([v.st[0] / w as f32, v.st[1] / h as f32]);
                p.colors.push(v.rgba);
            }
        }
        primitives.push(p);
    }

    // Road (Dam's asphalt), walls (Dam's red/white stripes, solid both
    // sides), out-of-bounds sheet at the water (Lakitu rescue).
    let road_image = texture(ROAD_TEXTURE)?.map(|_| format!("textures/tex_{ROAD_TEXTURE:04}.png"));
    let wall_image = texture(WALL_TEXTURE)?.map(|_| format!("textures/tex_{WALL_TEXTURE:04}.png"));
    let parts = [
        ("n64kr_road", road_image.clone(), extras("ASPHALT", "CLIP_DEFAULT", "DRAW_OPAQUE", "opaque"), meshes.road),
        ("n64kr_wall", wall_image, extras("SURFACE_DEFAULT", "CLIP_DOUBLE_SIDED_WALL", "DRAW_OPAQUE", "opaque"), meshes.walls),
        // Never drawn, but Fast64 only converts textured materials (an untextured
        // one stops the export), so it borrows the road's.
        ("n64kr_out_of_bounds", road_image, extras("OUT_OF_BOUNDS", "CLIP_DEFAULT", "DRAW_INVISIBLE", "opaque"), meshes.out_of_bounds),
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

    gltf::write(&out, TRACK, &materials, &primitives)?;

    // Layout: AI path along the centre; item box rows on both straights and turn apexes.
    let path = track::path(&oval, &place, 33.0);
    let (l, r) = (oval.half_straight, oval.radius);
    let turn = std::f32::consts::PI * r;
    let rows: Vec<[f32; 3]> = [0.5 * l, l + turn / 2.0, l + turn + 0.66 * l, l + turn + 1.33 * l, 3.0 * l + 1.5 * turn, 3.5 * l + 2.0 * turn]
        .iter()
        .map(|&s| {
            let p = place.apply(oval.sample(s).across(oval.width / 2.0));
            [p[0], p[1] + 15.0, p[2]]
        })
        .collect();
    let layout = track::layout_json(TRACK, SCALE, &path, &rows, [92, 120, 150], [178, 186, 196]);
    std::fs::write(out.join(format!("{TRACK}_mk64.json")), layout).map_err(|e| e.to_string())?;
    println!(
        "{TRACK}: lap {:.0} MK64 units, road {:.0} wide, banking {} deg; water at y {water_y} (GE); {} scenery triangles, \
         {} materials, path {} points -> {}",
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
        ["ge-dam-speedway", rom, dir] => ge_dam_speedway(rom, dir),
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
