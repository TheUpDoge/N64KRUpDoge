//! DKR textures and sprites (include/structs.h TextureHeader, SpriteAsset;
//! src/textures_sprites.c).
//!
//! TextureHeader: u8 width, height, format (low 4 bits: 0 RGBA32, 1 RGBA16,
//! 2 I8, 3 I4, 4 IA16, 5 IA8, 6 IA4, 7 CI4, 8 CI8), s8 posX, posY, u8
//! instances, s16 flags (0x40 / 0x80 clamp S / T, 0x400 interlaced), ...,
//! u16 numOfTextures (frames << 8) at 0x12, s16 textureSize (bytes per frame,
//! header included) at 0x16, u8 isCompressed at 0x1D. Pixels follow the
//! 0x20-byte header; a compressed texture inflates to header + pixels.
//! Animated textures (racers' eyes: open .. shut) store each frame as header +
//! pixels, textureSize apart (load_texture).

use super::{inflate, s16, u16, Rom, SPRITES, SPRITES_TABLE, TEXTURES_2D, TEXTURES_2D_TABLE};

pub const TEX_FLAG_CLAMP_S: u16 = 0x40;
pub const TEX_FLAG_CLAMP_T: u16 = 0x80;
const TEX_FLAG_INTERLACED: u16 = 0x400;
/// TMEM holds 2048 RGBA16 texels (4 KB).
pub const TMEM_TEXELS: usize = 2048;

pub struct Texture {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
    pub flags: u16,
}

/// Frame count of a texture asset (numOfTextures >> 8, header 0x12).
pub fn frames(raw: &[u8]) -> usize {
    (raw[0x12] as usize).max(1)
}

/// One frame (0 = the first) of a texture asset as RGBA8.
pub fn decode(raw: &[u8], frame: usize) -> Result<Texture, String> {
    let (width, height, format) = (raw[0] as usize, raw[1] as usize, raw[2] & 0xF);
    let flags = u16(raw, 6);
    let compressed = raw[0x1D] != 0;
    let body = if compressed { inflate(&raw[0x20..])? } else { raw[0x20..].to_vec() };
    // Uncompressed: pixels follow the asset header. Compressed: the inflated
    // data starts with its own header.
    let mut pixels: &[u8] = if compressed { body.get(0x20..).unwrap_or(&[]) } else { &body };
    if frame > 0 {
        let stride = u16(raw, 0x16) as usize;
        let skip = frame.min(frames(raw) - 1) * stride;
        pixels = pixels.get(skip..).unwrap_or(&[]);
    }
    let bpp = match format {
        0 => 32,
        1 | 4 => 16,
        2 | 5 | 8 => 8,
        3 | 6 | 7 => 4,
        _ => return Err(format!("texture format {format} unknown")),
    };
    let row = width * bpp / 8;
    let mut data = vec![0u8; row * height];
    let n = data.len().min(pixels.len());
    data[..n].copy_from_slice(&pixels[..n]);
    if flags & TEX_FLAG_INTERLACED != 0 {
        let chunk = if bpp == 32 { 8 } else { 4 };
        for y in (1..height).step_by(2) {
            let o = y * row;
            let mut x = 0;
            while row >= 2 * chunk && x <= row - 2 * chunk {
                for k in 0..chunk {
                    data.swap(o + x + k, o + x + chunk + k);
                }
                x += 2 * chunk;
            }
        }
    }
    let mut rgba = vec![0u8; width * height * 4];
    for i in 0..width * height {
        let px: [u8; 4] = match format {
            0 => [data[i * 4], data[i * 4 + 1], data[i * 4 + 2], data[i * 4 + 3]],
            1 => {
                let p = ((data[i * 2] as u32) << 8) | data[i * 2 + 1] as u32;
                let c = |s: u32| (((p >> s) & 31) * 255 / 31) as u8;
                [c(11), c(6), c(1), if p & 1 != 0 { 255 } else { 0 }]
            }
            2 => [data[i]; 4],
            3 => {
                let v = (data[i / 2] >> if i % 2 == 0 { 4 } else { 0 }) & 15;
                [v * 17; 4]
            }
            4 => [data[i * 2], data[i * 2], data[i * 2], data[i * 2 + 1]],
            5 => {
                let v = data[i];
                let l = (v >> 4) * 17;
                [l, l, l, (v & 15) * 17]
            }
            6 => {
                let v = (data[i / 2] >> if i % 2 == 0 { 4 } else { 0 }) & 15;
                let l = ((v >> 1) as u32 * 255 / 7) as u8;
                [l, l, l, if v & 1 != 0 { 255 } else { 0 }]
            }
            // CI4 / CI8 not handled yet: magenta so it stands out.
            _ => [255, 0, 255, 255],
        };
        rgba[i * 4..i * 4 + 4].copy_from_slice(&px);
    }
    Ok(Texture { width, height, rgba, flags })
}

/// Nearest-neighbour resample to the power-of-two size (8..128 per side) that
/// fits TMEM with the closest aspect ratio (bigger on a tie). Fast64 and
/// libultraship need textures that load in one block.
pub fn fit_tmem(width: usize, height: usize, rgba: &[u8]) -> (usize, usize, Vec<u8>) {
    let target = width as f64 / height as f64;
    let mut best: Option<((f64, i64), (usize, usize))> = None;
    for a in 3..8 {
        for b in 3..8 {
            let (w, h) = (1usize << a, 1usize << b);
            if w * h > TMEM_TEXELS {
                continue;
            }
            let key = (((w as f64 / h as f64) / target).ln().abs(), -((w * h) as i64));
            if best.as_ref().map_or(true, |(k, _)| key.0 < k.0 || (key.0 == k.0 && key.1 < k.1)) {
                best = Some((key, (w, h)));
            }
        }
    }
    let (tw, th) = best.expect("sizes").1;
    let mut out = vec![0u8; tw * th * 4];
    for y in 0..th {
        let sy = (height - 1).min(y * height / th);
        for x in 0..tw {
            let sx = (width - 1).min(x * width / tw);
            out[(y * tw + x) * 4..(y * tw + x) * 4 + 4].copy_from_slice(&rgba[(sy * width + sx) * 4..(sy * width + sx) * 4 + 4]);
        }
    }
    (tw, th, out)
}

/// A power-of-two texture that fits TMEM as it is, else fit_tmem.
pub fn pow2_fit(width: usize, height: usize, rgba: &[u8]) -> (usize, usize, Vec<u8>) {
    if width.is_power_of_two() && height.is_power_of_two() && width * height <= TMEM_TEXELS && width >= 8 && height >= 8 {
        (width, height, rgba.to_vec())
    } else {
        fit_tmem(width, height, rgba)
    }
}

pub struct SpriteImage {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
    /// Top-left corner relative to the sprite anchor, pixels, Y up.
    pub left: i32,
    pub top: i32,
}

/// One frame of a sprite, assembled from its 2D texture pieces (piece at
/// x = posX - anchor.x, y = anchor.y - posY; SpriteAsset: s16 base texture,
/// s16 frame count, s16 anchor x, y, then u8 first-piece per frame at 0x0C).
pub fn sprite_image(rom: &Rom, sprite_id: usize, frame: usize) -> Result<SpriteImage, String> {
    let (sprites, stable) = (rom.section(SPRITES), rom.table(SPRITES_TABLE));
    let (tex2d, ttable) = (rom.section(TEXTURES_2D), rom.table(TEXTURES_2D_TABLE));
    let a = rom.entry(sprites, &stable, sprite_id);
    let (base, count) = (s16(a, 0) as i32, s16(a, 2) as i32);
    let (anchor_x, anchor_y) = (s16(a, 4) as i32, s16(a, 6) as i32);
    let frame = if count >= 1 { (frame as i32).clamp(0, count - 1) as usize } else { 0 };
    let first = a[0x0C + frame] as i32;
    let last = if count >= 1 { a[0x0C + frame + 1] as i32 } else { a[0x0C] as i32 };
    let mut pieces = Vec::new();
    for tid in (base + first)..(base + last) {
        let raw = rom.entry(tex2d, &ttable, tid as usize);
        let t = decode(raw, 0)?;
        let (pos_x, pos_y) = (raw[3] as i8 as i32, raw[4] as i8 as i32);
        pieces.push((pos_x - anchor_x, anchor_y - pos_y, t));
    }
    if pieces.is_empty() {
        return Err(format!("sprite {sprite_id} frame {frame} has no pieces"));
    }
    let left = pieces.iter().map(|p| p.0).min().unwrap();
    let right = pieces.iter().map(|p| p.0 + p.2.width as i32).max().unwrap();
    let top = pieces.iter().map(|p| p.1).max().unwrap();
    let bottom = pieces.iter().map(|p| p.1 - p.2.height as i32).min().unwrap();
    let (width, height) = ((right - left) as usize, (top - bottom) as usize);
    let mut out = vec![0u8; width * height * 4];
    for (px, py, t) in &pieces {
        let (ox, oy) = ((px - left) as usize, (top - py) as usize);
        for y in 0..t.height {
            let dst = ((oy + y) * width + ox) * 4;
            out[dst..dst + t.width * 4].copy_from_slice(&t.rgba[y * t.width * 4..(y + 1) * t.width * 4]);
        }
    }
    Ok(SpriteImage { width, height, rgba: out, left, top })
}
