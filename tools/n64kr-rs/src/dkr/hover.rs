//! DKR hovercraft stats (`n64kr dkr-hover-stats`): a small text file that
//! src/n64kr/hover.c loads at runtime (ROM data: under build/, never committed).
//!
//! recon/004-hover/SLICE-5_DKR_Stats.md: misc assets (section 15, table 16:
//! word offsets) 9 and 10 are the racers' weight and handling (10 floats,
//! Character enum order); a racer object's header byte 0x5C (miscAssetIds[0])
//! is its acceleration curve (16 floats).

use super::{objects, s32, Rom, MISC, MISC_TABLE, OBJECTS, OBJECT_HEADERS_TABLE};
use crate::rom;

const MISC_RACER_WEIGHT: usize = 9;
const MISC_RACER_HANDLING: usize = 10;
/// SHA-1 of the two stat tables in the decomp's asset list, to confirm the read.
const MISC_SHA1: [(usize, &str); 2] = [
    (MISC_RACER_WEIGHT, "cd7675cbb4d678686cc7a6dd2c7dcf75034fbe7f"),
    (MISC_RACER_HANDLING, "0bd70b3abee14c180be4b21c120b506f09e839dc"),
];
const NUM_CHARACTERS: usize = 10;
const CURVE_LENGTH: usize = 16;
/// DKR Character enum order (include/enums.h) -> hovercraft object name.
const HOVER_OBJECTS: [&str; 10] = [
    "KremlinHover",  // Krunch
    "BadgerHover",   // Bumper
    "TortHover",     // Tiptup
    "ConkaHover",    // Conker
    "TigerHover",    // Timber
    "BanjoHover",    // Banjo
    "ChickenHover",  // Drumstick
    "MouseHover",    // Pipsy
    "ticktockhover", // T.T.
    "diddyhover",    // Diddy
];

/// An offset table read whole (terminator included), as the stats use it.
fn raw_table(raw: &[u8]) -> Vec<i64> {
    (0..raw.len().saturating_sub(3)).step_by(4).map(|i| s32(raw, i) as i64).collect()
}

/// Python-style slice: negative ends count from the back.
fn slice(data: &[u8], a: i64, b: i64) -> &[u8] {
    let n = data.len() as i64;
    let fix = |v: i64| if v < 0 { (n + v).max(0) } else { v.min(n) } as usize;
    let (a, b) = (fix(a), fix(b));
    if a < b { &data[a..b] } else { &[] }
}

fn floats(blob: &[u8], count: usize) -> Vec<f64> {
    (0..count).map(|i| super::f32_at(blob, i * 4) as f64).collect()
}

/// `n64kr dkr-hover-stats <rom> <out.txt>`
pub fn extract(rom: &Rom, out_path: &str) -> Result<(), String> {
    let misc = rom.section(MISC);
    let misc_table = raw_table(rom.section(MISC_TABLE));
    let misc_asset = |i: usize| slice(misc, misc_table[i] * 4, misc_table[i + 1] * 4);
    for (index, sha) in MISC_SHA1 {
        if rom::sha1_hex(misc_asset(index)) != sha {
            return Err(format!("misc asset {index} did not match the expected SHA-1; layout differs from this ROM"));
        }
    }
    let objs = rom.section(OBJECTS);
    let object_table = raw_table(rom.section(OBJECT_HEADERS_TABLE));
    let mut curve_ids: Vec<(String, usize)> = vec![];
    for i in 0..object_table.len().saturating_sub(1) {
        let header = slice(objs, object_table[i], object_table[i + 1]);
        if header.len() < 0x70 {
            continue;
        }
        let name = objects::header_name(header);
        if HOVER_OBJECTS.contains(&name.as_str()) && !curve_ids.iter().any(|(n, _)| *n == name) {
            curve_ids.push((name, header[0x5C] as usize));
        }
    }
    let missing: Vec<&str> = HOVER_OBJECTS.iter().filter(|n| !curve_ids.iter().any(|(c, _)| c == *n)).copied().collect();
    if !missing.is_empty() {
        return Err(format!("hovercraft objects not found: {missing:?}"));
    }
    let g = |v: &[f64]| v.iter().map(|&x| super::fmt_g(x, 9)).collect::<Vec<_>>().join(" ");
    let mut out = String::from("# N64KR: DKR hovercraft stats extracted from your own ROM. Do not commit.\n");
    out += "version us.v80\n";
    out += &format!("weight {}\n", g(&floats(misc_asset(MISC_RACER_WEIGHT), NUM_CHARACTERS)));
    out += &format!("handling {}\n", g(&floats(misc_asset(MISC_RACER_HANDLING), NUM_CHARACTERS)));
    for (character, name) in HOVER_OBJECTS.iter().enumerate() {
        let id = curve_ids.iter().find(|(n, _)| n == name).unwrap().1;
        out += &format!("curve {character} {}\n", g(&floats(misc_asset(id), CURVE_LENGTH)));
    }
    std::fs::write(out_path, out).map_err(|e| format!("{out_path}: {e}"))?;
    println!("Wrote {out_path} (us.v80, {NUM_CHARACTERS} characters).");
    Ok(())
}
