//! GoldenEye room display lists -> triangles.
//!
//! The room lists use Rare's Fast3D variant (decomp include/gbi_extension.h):
//! - 0x04 G_VTX: n = ((w0 >> 20) & 0xF) + 1 vertices into slot (w0 >> 16) & 0xF,
//!   from the room's vertex table (segment 0x0E, w1 & 0xFFFFFF).
//! - 0xBF G_TRI1: w1 = [_, v0 * 10, v1 * 10, v2 * 10].
//! - 0xB1 G_TRI4: four triangles (x, y, z) as nibbles: z1..z4 in w0 bits 0-15,
//!   x1 y1 .. x4 y4 in w1 (x low nibble, y high nibble of each byte, from bit 0).
//! - 0xC0 (G_NOOP, reused): load texture w1 & 0xFFF (tex.c texLoadFromGdl);
//!   w0 & 7 = texture type, type 0 carries tile shifts in w0 bits 14-17 / 10-13.
//! - 0xBB G_TEXTURE: s/t scale in w1 (0xFFFF = 1.0).
//! - 0xB8 end. Other commands (render modes, combiners, colours) don't change geometry.
//!
//! Vertices (N64 Vtx, 16 bytes: s16 x, y, z, flag, s16 s, t, u8 r, g, b, a) are
//! relative to the room's position in the room table.

use crate::rom::{s16_be, u32_be};

#[derive(Clone, Copy, Default)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub st: [f32; 2], // raw s, t (10.5 fixed point); triangles carry texel coordinates
    pub rgba: [u8; 4],
}

pub struct Triangle {
    pub texture: Option<usize>,
    pub translucent: bool,
    pub v: [Vertex; 3],
}

/// N64 tile shift: 1-10 divide by 2^n, 11-15 multiply by 2^(16 - n).
fn shift_factor(shift: u32) -> f32 {
    match shift {
        0 => 1.0,
        1..=10 => 1.0 / (1u32 << shift) as f32,
        _ => (1u32 << (16 - shift)) as f32,
    }
}

pub fn room_triangles(dl: &[u8], vertices: &[u8], room_pos: [f32; 3], translucent: bool, out: &mut Vec<Triangle>) {
    let mut slots = [Vertex::default(); 16];
    let mut texture: Option<usize> = None;
    let (mut scale_s, mut scale_t) = (1.0f32, 1.0f32);
    let (mut shift_s, mut shift_t) = (1.0f32, 1.0f32);

    // Texture scale and tile shift apply when a triangle is drawn: texel = st / 32 * scale * shift.
    let mut emit = |slots: &[Vertex; 16], texture: Option<usize>, f: [f32; 2], a: usize, b: usize, c: usize| {
        if a == b || b == c || a == c {
            return;
        }
        let texel = |v: Vertex| Vertex { st: [v.st[0] * f[0] / 32.0, v.st[1] * f[1] / 32.0], ..v };
        out.push(Triangle { texture, translucent, v: [texel(slots[a]), texel(slots[b]), texel(slots[c])] });
    };

    for cmd in dl.chunks_exact(8) {
        let w0 = u32_be(cmd, 0);
        let w1 = u32_be(cmd, 4);
        match cmd[0] {
            0x04 => {
                let n = (((w0 >> 20) & 0xF) + 1) as usize;
                let first = ((w0 >> 16) & 0xF) as usize;
                let base = (w1 & 0xFF_FFFF) as usize;
                for i in 0..n {
                    let o = base + i * 16;
                    if first + i >= 16 || o + 16 > vertices.len() {
                        break;
                    }
                    slots[first + i] = Vertex {
                        pos: [
                            s16_be(vertices, o) as f32 + room_pos[0],
                            s16_be(vertices, o + 2) as f32 + room_pos[1],
                            s16_be(vertices, o + 4) as f32 + room_pos[2],
                        ],
                        st: [s16_be(vertices, o + 8) as f32, s16_be(vertices, o + 10) as f32],
                        rgba: [vertices[o + 12], vertices[o + 13], vertices[o + 14], vertices[o + 15]],
                    };
                }
            }
            0xBF => {
                let (a, b, c) = (((w1 >> 16) & 0xFF) / 10, ((w1 >> 8) & 0xFF) / 10, (w1 & 0xFF) / 10);
                if a < 16 && b < 16 && c < 16 {
                    emit(&slots, texture, [scale_s * shift_s, scale_t * shift_t], a as usize, b as usize, c as usize);
                }
            }
            0xB1 => {
                for k in 0..4 {
                    let x = ((w1 >> (k * 8)) & 0xF) as usize;
                    let y = ((w1 >> (k * 8 + 4)) & 0xF) as usize;
                    let z = ((w0 >> (k * 4)) & 0xF) as usize;
                    emit(&slots, texture, [scale_s * shift_s, scale_t * shift_t], x, y, z);
                }
            }
            0xC0 => {
                texture = Some((w1 & 0xFFF) as usize);
                if w0 & 7 == 0 {
                    shift_s = shift_factor((w0 >> 14) & 0xF);
                    shift_t = shift_factor((w0 >> 10) & 0xF);
                } else {
                    shift_s = 1.0;
                    shift_t = 1.0;
                }
            }
            0xBB => {
                let s = (w1 >> 16) & 0xFFFF;
                let t = w1 & 0xFFFF;
                scale_s = if s == 0xFFFF { 1.0 } else { s as f32 / 65536.0 };
                scale_t = if t == 0xFFFF { 1.0 } else { t as f32 / 65536.0 };
            }
            0xB8 => break,
            _ => {}
        }
    }
}
