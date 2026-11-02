//! n64kr: N64KR's asset tools. Each subcommand reads your own ROM and writes
//! generated content under build/ (never committed).

mod ge;
mod rarezip;
mod rom;

use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
n64kr <command> [args]

Commands:
  ge-bg-info <GoldenEye ROM> <level>   List a GoldenEye level's rooms (vertices, display lists)
                                       levels: dam, facility, runway, surface, bunker, silo, ...
";

fn ge_bg_info(rom_path: &str, level: &str) -> Result<(), String> {
    let rom = rom::Rom::load(&PathBuf::from(rom_path))?;
    let sha1 = rom::sha1_hex(&rom.data);
    if sha1 != ge::US_SHA1 {
        return Err(format!("expected the US GoldenEye ROM (SHA-1 {}), got {sha1}", ge::US_SHA1));
    }
    let (bg_name, scale) = ge::level(level).ok_or_else(|| format!("unknown level '{level}'"))?;
    let (offset, size) = ge::bg_file(bg_name).ok_or_else(|| format!("no bg file {bg_name}"))?;
    let bg = ge::bg::parse(rom.slice(offset, size)?)?;

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
