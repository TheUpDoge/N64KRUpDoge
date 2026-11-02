//! Reading N64 ROM images and big-endian data.

use std::fs;
use std::path::Path;

/// A ROM image in .z64 (big-endian) byte order.
pub struct Rom {
    pub data: Vec<u8>,
}

impl Rom {
    /// Loads a ROM, converting .v64 (byte-swapped) and .n64 (little-endian)
    /// images to .z64 order.
    pub fn load(path: &Path) -> Result<Rom, String> {
        let mut data = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        match data.get(0..4) {
            Some([0x80, 0x37, 0x12, 0x40]) => {}
            Some([0x37, 0x80, 0x40, 0x12]) => data.chunks_exact_mut(2).for_each(|c| c.swap(0, 1)),
            Some([0x40, 0x12, 0x37, 0x80]) => data.chunks_exact_mut(4).for_each(|c| c.reverse()),
            _ => return Err(format!("{}: not an N64 ROM image", path.display())),
        }
        Ok(Rom { data })
    }

    pub fn slice(&self, offset: usize, len: usize) -> Result<&[u8], String> {
        self.data
            .get(offset..offset + len)
            .ok_or_else(|| format!("ROM range {offset:#x}+{len:#x} out of bounds"))
    }
}

pub fn u16_be(b: &[u8], o: usize) -> u16 {
    u16::from_be_bytes([b[o], b[o + 1]])
}

pub fn s16_be(b: &[u8], o: usize) -> i16 {
    i16::from_be_bytes([b[o], b[o + 1]])
}

pub fn u32_be(b: &[u8], o: usize) -> u32 {
    u32::from_be_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

pub fn f32_be(b: &[u8], o: usize) -> f32 {
    f32::from_bits(u32_be(b, o))
}

/// SHA-1 of a byte slice as lowercase hex (for checking ROM versions).
pub fn sha1_hex(data: &[u8]) -> String {
    // Small self-contained SHA-1 (FIPS 180-1); only used to identify ROMs.
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let mut msg = data.to_vec();
    let bit_len = (data.len() as u64) * 8;
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for block in msg.chunks_exact(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([block[i * 4], block[i * 4 + 1], block[i * 4 + 2], block[i * 4 + 3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let t = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(*wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}
