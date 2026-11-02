//! GoldenEye textures: decoding images from the ROM's texture bank to RGBA.
//!
//! Ported from the decomp's src/game/image.c (texLoad, texInflateZlib,
//! texInflateNonZlib and helpers) and src/game/image_bank.c (bit reader).
//! Only the full-size image (LOD 0) is decoded; mip levels and the N64's
//! TMEM row interleave (texSwapAltRowBytes) aren't needed for export.
//!
//! Each texture starts with a flags byte: bit 7 = explicit LODs, bit 6 =
//! zlib (paletted, deflate-compressed indices), bits 0-5 = LOD count. The
//! rest is a big-endian bitstring.

use super::images_us::IMAGE_OFFSETS;
use flate2::read::DeflateDecoder;
use std::io::Read;

// Texture formats (image.h TEXFORMAT_*).
const RGBA32: usize = 0;
const RGBA16: usize = 1;
const RGB24: usize = 2;
const RGB15: usize = 3;
const IA16: usize = 4;
const IA8: usize = 5;
const IA4: usize = 6;
const I8: usize = 7;
const I4: usize = 8;
const RGBA16_CI8: usize = 9;
const RGBA16_CI4: usize = 10;
const IA16_CI8: usize = 11;
const IA16_CI4: usize = 12;

// Per-format tables (image.c g_TexFormat*).
const NUM_CHANNELS: [usize; 13] = [4, 3, 3, 3, 2, 2, 1, 1, 1, 1, 1, 1, 1];
const HAS_1BIT_ALPHA: [bool; 13] = [false, true, false, false, false, false, true, false, false, false, false, false, false];
const CHANNEL_SIZES: [usize; 13] = [0x100, 0x20, 0x100, 0x20, 0x100, 0x10, 8, 0x100, 0x10, 0x100, 0x10, 0x100, 0x10];
const BITS_PER_PIXEL: [u32; 13] = [0x20, 0x10, 0x18, 0xF, 0x10, 8, 4, 8, 4, 0x10, 0x10, 0x10, 0x10];

pub struct Texture {
    pub width: usize,
    pub height: usize,
    pub format: usize,
    pub rgba: Vec<u8>,
}

/// Big-endian bitstring reader (image_bank.c texReadBits).
struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    acc: u64,
    count: u32,
}

impl<'a> Bits<'a> {
    fn new(data: &'a [u8]) -> Self {
        Bits { data, pos: 0, acc: 0, count: 0 }
    }

    fn read(&mut self, n: u32) -> u32 {
        while self.count < n {
            let byte = *self.data.get(self.pos).unwrap_or(&0) as u64;
            self.acc = ((self.acc << 8) | byte) & 0xFFFF_FFFF_FFFF;
            self.pos += 1;
            self.count += 8;
        }
        self.count -= n;
        ((self.acc >> self.count) & ((1u64 << n) - 1)) as u32
    }
}

pub fn texture_count() -> usize {
    IMAGE_OFFSETS.len() - 1
}

/// Decodes texture `num` from the US ROM image.
pub fn decode(rom: &[u8], num: usize) -> Result<Texture, String> {
    if num >= texture_count() {
        return Err(format!("texture {num} out of range"));
    }
    let (start, end) = (IMAGE_OFFSETS[num] as usize, IMAGE_OFFSETS[num + 1] as usize);
    let data = rom.get(start..end).ok_or_else(|| format!("texture {num} past the ROM end"))?;
    let flags = *data.first().ok_or_else(|| format!("texture {num} is empty"))?;
    if flags & 0x40 != 0 {
        decode_zlib(&data[1..])
    } else {
        decode_rare(&data[1..])
    }
    .map_err(|e| format!("texture {num}: {e}"))
}

/// texInflateZlib: paletted. Header bitstring (format, palette, size), then a
/// deflate block (2-byte header skipped, as decompressdata does) of indices,
/// each row ceil(width / indices-per-byte) bytes.
fn decode_zlib(data: &[u8]) -> Result<Texture, String> {
    let mut bits = Bits::new(data);
    let format = bits.read(8) as usize;
    if !(RGBA16_CI8..=IA16_CI4).contains(&format) {
        return Err(format!("zlib texture with non-paletted format {format}"));
    }
    let numcolours = bits.read(8) as usize + 1;
    let palette: Vec<u32> = (0..numcolours).map(|_| bits.read(16)).collect();
    let width = bits.read(8) as usize;
    let height = bits.read(8) as usize;
    if width == 0 || height == 0 || width * height > 4096 {
        return Err(format!("bad size {width}x{height}"));
    }
    // The header is a whole number of bytes; the deflate block follows.
    let block = data.get(bits.pos + 2..).ok_or("truncated")?;
    let mut indices = Vec::new();
    DeflateDecoder::new(block).read_to_end(&mut indices).map_err(|e| format!("deflate: {e}"))?;
    let ci4 = format == RGBA16_CI4 || format == IA16_CI4;
    let row = if ci4 { (width + 1) / 2 } else { width };
    let mut rgba = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        for x in 0..width {
            let idx = if ci4 {
                let b = *indices.get(y * row + x / 2).unwrap_or(&0);
                if x % 2 == 0 { b >> 4 } else { b & 0xF }
            } else {
                *indices.get(y * row + x).unwrap_or(&0)
            } as usize;
            let c = *palette.get(idx).unwrap_or(&0);
            let px = if format <= RGBA16_CI4 { texel_to_rgba(RGBA16, c) } else { texel_to_rgba(IA16, c) };
            rgba.extend_from_slice(&px);
        }
    }
    Ok(Texture { width, height, format, rgba })
}

/// texInflateNonZlib: Rare's own methods. Pixels come out as N64 texel
/// values of the format (u32 each), then convert to RGBA.
fn decode_rare(data: &[u8]) -> Result<Texture, String> {
    let mut bits = Bits::new(data);
    let format = bits.read(4) as usize;
    let width = bits.read(8) as usize;
    let height = bits.read(8) as usize;
    let method = bits.read(4);
    if format > I4 {
        return Err(format!("non-zlib texture with format {format}"));
    }
    if width == 0 || height == 0 || width * height > 0x2000 {
        return Err(format!("bad size {width}x{height}"));
    }
    let n = width * height;
    let texels: Vec<u32> = match method {
        0 | 1 => read_uncompressed(&mut bits, width, height, format),
        2 | 3 | 4 | 8 | 9 => {
            let chans = NUM_CHANNELS[format];
            let size = CHANNEL_SIZES[format];
            // Channel planes, plus a 1-bit alpha plane at index 3 where used.
            let mut planes = vec![0u16; n * 4];
            match method {
                2 => huffman(&mut bits, &mut planes[..chans * n], size),
                3 => {
                    for c in 0..chans {
                        huffman(&mut bits, &mut planes[c * n..(c + 1) * n], size);
                    }
                }
                4 => rle(&mut bits, &mut planes[..chans * n]),
                8 => {
                    let blur_method = bits.read(3);
                    huffman(&mut bits, &mut planes[..chans * n], size);
                    blur(&mut planes[..chans * n], width, chans * height, blur_method, size as i32);
                }
                _ => {
                    let blur_method = bits.read(3);
                    rle(&mut bits, &mut planes[..chans * n]);
                    blur(&mut planes[..chans * n], width, chans * height, blur_method, size as i32);
                }
            }
            if HAS_1BIT_ALPHA[format] {
                for i in 0..n {
                    planes[3 * n + i] = bits.read(1) as u16;
                }
            }
            channels_to_texels(&planes, n, format)
        }
        5 | 6 | 7 => {
            let lookup = build_lookup(&mut bits, BITS_PER_PIXEL[format]);
            let indices: Vec<usize> = match method {
                5 => {
                    let b = bit_size(lookup.len());
                    (0..n).map(|_| bits.read(b) as usize).collect()
                }
                6 => {
                    let mut buf = vec![0u16; n];
                    huffman(&mut bits, &mut buf, lookup.len());
                    buf.into_iter().map(|v| v as usize).collect()
                }
                _ => {
                    let mut buf = vec![0u16; n];
                    rle(&mut bits, &mut buf);
                    buf.into_iter().map(|v| v as usize).collect()
                }
            };
            indices
                .into_iter()
                .map(|i| {
                    let v = *lookup.get(i).unwrap_or(&0);
                    match format {
                        RGB24 => (v << 8) | 0xFF,
                        RGB15 => (v << 1) | 1,
                        IA4 | I4 => v & 0xF,
                        _ => v,
                    }
                })
                .collect()
        }
        m => return Err(format!("unknown compression method {m}")),
    };
    let rgba = texels.iter().flat_map(|&t| texel_to_rgba(format, t)).collect();
    Ok(Texture { width, height, format, rgba })
}

/// texReadUncompressed, one texel per pixel (4-bit formats: two per byte).
fn read_uncompressed(bits: &mut Bits, width: usize, height: usize, format: usize) -> Vec<u32> {
    let mut out = Vec::with_capacity(width * height);
    for _y in 0..height {
        let mut x = 0;
        while x < width {
            match format {
                RGBA32 => out.push((bits.read(16) << 16) | bits.read(16)),
                RGB24 => out.push((bits.read(24) << 8) | 0xFF),
                RGBA16 | IA16 => out.push(bits.read(16)),
                RGB15 => out.push((bits.read(15) << 1) | 1),
                IA8 | I8 => out.push(bits.read(8)),
                _ => {
                    let b = bits.read(8);
                    out.push(b >> 4);
                    if x + 1 < width {
                        out.push(b & 0xF);
                    }
                    x += 1;
                }
            }
            x += 1;
        }
    }
    out
}

/// texChannelsToPixels.
fn channels_to_texels(p: &[u16], n: usize, format: usize) -> Vec<u32> {
    (0..n)
        .map(|i| {
            let c = |k: usize| p[k * n + i] as u32;
            match format {
                RGBA32 => (c(0) << 24) | (c(1) << 16) | (c(2) << 8) | c(3),
                RGB24 => (c(0) << 24) | (c(1) << 16) | (c(2) << 8) | 0xFF,
                RGBA16 => (c(0) << 11) | (c(1) << 6) | (c(2) << 1) | c(3),
                IA16 => (c(0) << 8) | c(1),
                RGB15 => (c(0) << 11) | (c(1) << 6) | (c(2) << 1) | 1,
                IA8 => (c(0) << 4) | c(1),
                IA4 => (c(0) << 1) | c(3),
                _ => c(0), // I8, I4
            }
        })
        .collect()
}

/// texBuildLookup: 11-bit colour count, then colours of the format's size.
fn build_lookup(bits: &mut Bits, bpp: u32) -> Vec<u32> {
    let count = bits.read(11) as usize;
    (0..count)
        .map(|_| if bpp <= 24 { bits.read(bpp) } else { (bits.read(24) << 8) | bits.read(bpp - 24) })
        .collect()
}

/// texGetBitSize: bits needed for indices 0..count-1.
fn bit_size(count: usize) -> u32 {
    let mut d = count as i64 - 1;
    let mut bits = 0;
    while d > 0 {
        d >>= 1;
        bits += 1;
    }
    bits
}

/// texInflateHuffman, including its exact tree construction (tie-breaking
/// matters: the tree shape decides the codes).
fn huffman(bits: &mut Bits, dst: &mut [u16], chansize: usize) {
    let mut freq = [0u16; 2048];
    let mut nodes = [[-1i32; 2]; 2048];
    for f in freq.iter_mut().take(chansize) {
        *f = bits.read(8) as u16;
    }
    let find_two = |freq: &[u16; 2048], first: bool| -> (u16, usize, u16, usize) {
        let (mut m1, mut m2) = (9999u16, 9999u16);
        let (mut i1, mut i2) = (0usize, 0usize);
        for i in 0..chansize {
            if freq[i] < m1 {
                // The initial pass and the later passes compare in opposite
                // orders (decomp: "minfreq2 < minfreq1" vs "minfreq1 > minfreq2").
                let swap = if first { m2 < m1 } else { m1 > m2 };
                if swap {
                    m1 = freq[i];
                    i1 = i;
                } else {
                    m2 = freq[i];
                    i2 = i;
                }
            } else if freq[i] < m2 {
                m2 = freq[i];
                i2 = i;
            }
        }
        (m1, i1, m2, i2)
    };
    let (_, mut i1, _, mut i2) = find_two(&freq, true);
    let mut root = 0usize;
    loop {
        let mut sum = freq[i1] as u32 + freq[i2] as u32;
        if sum == 0 {
            sum = 1;
        }
        freq[i1] = 9999;
        freq[i2] = 9999;
        let leaf = |n: &[[i32; 2]; 2048], i: usize| n[i][0] < 0 && n[i][1] < 0;
        if leaf(&nodes, i1) {
            nodes[i1][0] = i1 as i32 + 10000;
            root = i1;
            freq[i1] = sum as u16;
            nodes[i1][1] = if leaf(&nodes, i2) { i2 as i32 + 10000 } else { i2 as i32 };
        } else if leaf(&nodes, i2) {
            nodes[i2][0] = i2 as i32 + 10000;
            root = i2;
            freq[i2] = sum as u16;
            nodes[i2][1] = if leaf(&nodes, i1) { i1 as i32 + 10000 } else { i1 as i32 };
        } else {
            root = 0;
            while nodes[root][0] >= 0 || nodes[root][1] >= 0 || freq[root] < 9999 {
                root += 1;
            }
            freq[root] = sum as u16;
            nodes[root][0] = i1 as i32;
            nodes[root][1] = i2 as i32;
        }
        let (m1, n1, m2, n2) = find_two(&freq, false);
        i1 = n1;
        i2 = n2;
        if m1 == 9999 || m2 == 9999 {
            break;
        }
    }
    for d in dst.iter_mut() {
        let mut v = root as i32;
        while v < 10000 {
            v = nodes[v as usize][bits.read(1) as usize];
        }
        *d = (v - 10000) as u16;
    }
}

/// texInflateRle.
fn rle(bits: &mut Bits, dst: &mut [u16]) {
    let bt = bits.read(3);
    let rl = bits.read(3);
    let block = bits.read(4);
    let mut cost = (bt + rl + block + 1) as i32;
    let mut fudge = 0usize;
    while cost > 0 {
        cost -= block as i32 + 1;
        fudge += 1;
    }
    let total = dst.len();
    let mut done = 0usize;
    while done < total {
        if bits.read(1) == 0 {
            dst[done] = bits.read(block) as u16;
            done += 1;
        } else {
            let start = done.saturating_sub(bits.read(bt) as usize + 1);
            let run = bits.read(rl) as usize + fudge;
            for i in start..start + run {
                if done < total {
                    dst[done] = dst[i];
                    done += 1;
                }
            }
            if done < total {
                dst[done] = bits.read(block) as u16;
                done += 1;
            }
        }
    }
}

/// texBlur: undo a predictive filter, modulo the channel size.
fn blur(p: &mut [u16], width: usize, height: usize, method: u32, chansize: i32) {
    for y in 0..height {
        for x in 0..width {
            let at = |p: &[u16], xx: usize, yy: usize| p[yy * width + xx] as i32;
            let cur = at(p, x, y) + chansize * 2;
            let left = if x > 0 { at(p, x - 1, y) } else { 0 };
            let above = if y > 0 { at(p, x, y - 1) } else { 0 };
            let al = if x > 0 && y > 0 { at(p, x - 1, y - 1) } else { 0 };
            let v = match method {
                0 => (cur + left) % chansize,
                1 => (cur + above) % chansize,
                2 => (cur + al) % chansize,
                3 => (cur + (left + above - al)) % chansize,
                4 => (cur + ((above - al) / 2 + left)) % chansize,
                5 => (cur + ((left - al) / 2 + above)) % chansize,
                6 => (cur + ((left + above) / 2)) % chansize,
                _ => cur % chansize,
            };
            p[y * width + x] = v as u16;
        }
    }
}

/// N64 texel value of a format -> RGBA8. Intensity-only formats are opaque
/// (on the N64 their alpha is the intensity; levels tint them by vertex colour).
fn texel_to_rgba(format: usize, v: u32) -> [u8; 4] {
    let x5 = |c: u32| ((c & 31) * 255 / 31) as u8;
    match format {
        RGBA32 => [(v >> 24) as u8, (v >> 16) as u8, (v >> 8) as u8, v as u8],
        RGB24 => [(v >> 24) as u8, (v >> 16) as u8, (v >> 8) as u8, 255],
        RGBA16 | RGB15 => [x5(v >> 11), x5(v >> 6), x5(v >> 1), if v & 1 != 0 { 255 } else { 0 }],
        IA16 => [(v >> 8) as u8, (v >> 8) as u8, (v >> 8) as u8, v as u8],
        IA8 => {
            let i = ((v >> 4) & 15) as u8 * 17;
            [i, i, i, (v & 15) as u8 * 17]
        }
        IA4 => {
            let i = (((v >> 1) & 7) * 255 / 7) as u8;
            [i, i, i, if v & 1 != 0 { 255 } else { 0 }]
        }
        I8 => [v as u8, v as u8, v as u8, 255],
        _ => {
            let i = (v & 15) as u8 * 17;
            [i, i, i, 255]
        }
    }
}
