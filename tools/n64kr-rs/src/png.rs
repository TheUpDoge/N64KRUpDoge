//! Minimal RGBA8 / greyscale PNG writer (zlib via flate2, CRC-32 here).

use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io::Write;

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

fn chunk(out: &mut Vec<u8>, tag: &[u8; 4], payload: &[u8]) {
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(tag);
    out.extend_from_slice(payload);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

pub fn encode_rgba(width: usize, height: usize, rgba: &[u8]) -> Vec<u8> {
    encode(width, height, rgba, 4, 6)
}

/// 8-bit greyscale (the minimap: SpaghettiKart reads it as one channel).
pub fn encode_grey(width: usize, height: usize, pixels: &[u8]) -> Vec<u8> {
    encode(width, height, pixels, 1, 0)
}

fn encode(width: usize, height: usize, pixels: &[u8], channels: usize, colour_type: u8) -> Vec<u8> {
    let rgba = pixels;
    let mut raw = Vec::with_capacity((width * channels + 1) * height);
    for y in 0..height {
        raw.push(0);
        raw.extend_from_slice(&rgba[y * width * channels..(y + 1) * width * channels]);
    }
    let mut z = ZlibEncoder::new(Vec::new(), Compression::best());
    z.write_all(&raw).expect("in-memory write");
    let idat = z.finish().expect("in-memory write");
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&(width as u32).to_be_bytes());
    ihdr.extend_from_slice(&(height as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, colour_type, 0, 0, 0]);
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &idat);
    chunk(&mut out, b"IEND", &[]);
    out
}
