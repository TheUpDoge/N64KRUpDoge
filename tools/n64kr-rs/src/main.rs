//! n64kr: N64KR's asset tools. Each subcommand reads your own ROM and writes
//! generated content under build/ (never committed).

mod ge;
mod gltf;
mod png;
mod rarezip;
mod rom;

use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
n64kr <command> [args]

Commands:
  ge-bg-info <GoldenEye ROM> <level>         List a GoldenEye level's rooms (vertices, display lists)
  ge-textures <GoldenEye ROM> <level> <dir>  Decode the textures a level uses to <dir>/tex_NNNN.png
  ge-gltf <GoldenEye ROM> <level> <dir>      Whole level as glTF (+ textures) in GoldenEye units, Y up
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
