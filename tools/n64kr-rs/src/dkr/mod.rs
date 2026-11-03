//! Diddy Kong Racing: reading the user's own ROM (US v1.1 / v80). Formats
//! from the DKR decomp (DavidSM64/Diddy-Kong-Racing, a5eb080, CC0); the notes
//! with each piece cite its source files.
//!
//! - Asset lookup table at the ROM's assets offset: u32 count, then one u32
//!   start offset per section, relative to the end of the table.
//! - Most assets are DKR-compressed: a 5-byte header, then raw deflate
//!   (src/gzip.c gzip_inflate).
//!
//! Rust port of the shared reader in the former tools/n64kr/dkr_level_to_gltf.py.

pub mod gltf;
pub mod hover;
pub mod level;
pub mod model;
pub mod objects;
pub mod racer;
pub mod scene;
pub mod tex;
pub mod voice;

use std::io::Read;
use std::path::Path;

use crate::rom;

/// SHA-1 of the supported ROM: DKR USA 1.1 (v80), .z64 byte order.
pub const US_V80_SHA1: &str = "6d96743d46f8c0cd0edb0ec5600b003c89b93755";
/// Asset lookup table offset and size (the decomp's dkr_assets_tool_extract.json).
const ASSETS: usize = 970976;
const LUT_SIZE: usize = 208;

// Section indices in the asset lookup table (position in the "sections" list
// of the decomp's tools/dkr_assets_tool_extract.json).
pub const AUDIO_TABLE: usize = 38;
pub const AUDIO: usize = 39;
pub const TEXTURES_3D: usize = 2;
pub const TEXTURES_3D_TABLE: usize = 3;
pub const TEXTURES_2D: usize = 4;
pub const TEXTURES_2D_TABLE: usize = 5;
pub const SPRITES: usize = 12;
pub const SPRITES_TABLE: usize = 13;
pub const MISC: usize = 15;
pub const MISC_TABLE: usize = 16;
pub const LEVEL_OBJECT_MAPS_TABLE: usize = 20;
pub const LEVEL_OBJECT_MAPS: usize = 21;
pub const LEVEL_HEADERS_TABLE: usize = 22;
pub const LEVEL_HEADERS: usize = 23;
pub const LEVEL_MODELS_TABLE: usize = 26;
pub const LEVEL_MODELS: usize = 27;
pub const OBJECT_MODELS_TABLE: usize = 28;
pub const OBJECT_MODELS: usize = 29;
pub const ANIMATION_IDS: usize = 30;
pub const OBJECT_ANIMATIONS_TABLE: usize = 31;
pub const OBJECT_ANIMATIONS: usize = 32;
pub const OBJECT_HEADERS_TABLE: usize = 33;
pub const OBJECTS: usize = 34;
pub const LEVEL_OBJECT_TRANSLATION_TABLE: usize = 35;

// Batch flags (include/structs.h TriangleBatchInfo).
pub const RENDER_SEMI_TRANSPARENT: u32 = 1 << 2;
pub const RENDER_CUTOUT: u32 = 1 << 4;
pub const RENDER_HIDDEN: u32 = 1 << 8;
pub const RENDER_NO_COLLISION: u32 = 1 << 9;
pub const RENDER_WATER: u32 = 1 << 13;

pub const SEGMENT_SIZE: usize = 0x44;
pub const BATCH_SIZE: usize = 12;
pub const TRIANGLE_SIZE: usize = 16;
pub const VERTEX_SIZE: usize = 10;

pub fn s8(b: &[u8], o: usize) -> i8 {
    b[o] as i8
}

pub fn s16(b: &[u8], o: usize) -> i16 {
    rom::s16_be(b, o)
}

pub fn u16(b: &[u8], o: usize) -> u16 {
    rom::u16_be(b, o)
}

pub fn u32(b: &[u8], o: usize) -> u32 {
    rom::u32_be(b, o)
}

pub fn s32(b: &[u8], o: usize) -> i32 {
    rom::u32_be(b, o) as i32
}

pub fn f32_at(b: &[u8], o: usize) -> f32 {
    rom::f32_be(b, o)
}

/// A DKR-compressed asset: 5-byte header, then raw deflate.
pub fn inflate(blob: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    flate2::read::DeflateDecoder::new(blob.get(5..).ok_or("DKR asset shorter than its header")?)
        .read_to_end(&mut out)
        .map_err(|e| format!("DKR inflate: {e}"))?;
    Ok(out)
}

pub struct Rom {
    pub data: Vec<u8>,
}

impl Rom {
    pub fn load(path: &str) -> Result<Rom, String> {
        let r = rom::Rom::load(Path::new(path))?;
        let sha = rom::sha1_hex(&r.data);
        if sha != US_V80_SHA1 {
            return Err(format!("unsupported DKR ROM (SHA-1 {sha}); supported: USA 1.1 (v80)"));
        }
        Ok(Rom { data: r.data })
    }

    /// The bytes of asset section `n`.
    pub fn section(&self, n: usize) -> &[u8] {
        let a = u32(&self.data, ASSETS + 4 * (n + 1)) as usize;
        let b = u32(&self.data, ASSETS + 4 * (n + 2)) as usize;
        &self.data[ASSETS + LUT_SIZE + a..ASSETS + LUT_SIZE + b]
    }

    /// An offset table section as s32 entries, up to its -1 terminator.
    pub fn table(&self, n: usize) -> Vec<i32> {
        let raw = self.section(n);
        let mut out = Vec::new();
        for i in (0..raw.len().saturating_sub(3)).step_by(4) {
            let v = s32(raw, i);
            if v == -1 {
                break;
            }
            out.push(v);
        }
        out
    }

    /// Entry `i` of a section addressed by its offset table.
    pub fn entry<'a>(&'a self, section: &'a [u8], table: &[i32], i: usize) -> &'a [u8] {
        &section[table[i] as usize..table[i + 1] as usize]
    }
}

/// Python's round() to an integer: round half to even.
pub fn round_half_even(x: f64) -> i64 {
    let r = x.round();
    if (x - x.trunc()).abs() == 0.5 {
        (2.0 * (x / 2.0).round()) as i64
    } else {
        r as i64
    }
}

/// C's "%.<precision>g" (Python's f"{v:.9g}").
pub fn fmt_g(v: f64, precision: usize) -> String {
    if v == 0.0 {
        return if v.is_sign_negative() { "-0".into() } else { "0".into() };
    }
    if !v.is_finite() {
        return format!("{v}");
    }
    let sci = format!("{:.*e}", precision - 1, v);
    let (mant, exp) = sci.split_once('e').expect("exponent");
    let exp: i32 = exp.parse().expect("exponent");
    let trim = |s: String| -> String {
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            s
        }
    };
    if exp < -4 || exp >= precision as i32 {
        format!("{}e{}{:02}", trim(mant.to_string()), if exp < 0 { '-' } else { '+' }, exp.abs())
    } else {
        let decimals = (precision as i32 - 1 - exp).max(0) as usize;
        trim(format!("{:.*}", decimals, v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn printf_g() {
        for (v, want) in [(1.0, "1"), (0.0625, "0.0625"), (1.0e-5, "1e-05"), (123456789.0, "123456789"),
                          (1234567890.0, "1.23456789e+09"), (0.30000001192092896, "0.300000012"), (-2.5, "-2.5")] {
            assert_eq!(fmt_g(v, 9), want);
        }
    }

    #[test]
    fn python_round() {
        assert_eq!(round_half_even(2.5), 2);
        assert_eq!(round_half_even(3.5), 4);
        assert_eq!(round_half_even(-2.5), -2);
        assert_eq!(round_half_even(2.4), 2);
        assert_eq!(round_half_even(2.6), 3);
    }
}
