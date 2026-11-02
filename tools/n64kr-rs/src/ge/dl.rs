//! GoldenEye display lists -> triangles (level rooms and object models).
//!
//! Both use Rare's Fast3D variant (decomp include/gbi_extension.h):
//! - 0x01 G_MTX: w1 = matrix address (segment 3, 64 bytes per matrix); object
//!   models switch joints this way.
//! - 0x04 G_VTX: n = ((w0 >> 20) & 0xF) + 1 vertices into slot (w0 >> 16) & 0xF,
//!   from w1 (rooms: segment 0x0E, the room's vertex table; models: segment 4/5).
//! - 0xBF G_TRI1: w1 = [_, v0 * 10, v1 * 10, v2 * 10].
//! - 0xB1 G_TRI4: four triangles (x, y, z) as nibbles: z1..z4 in w0 bits 0-15,
//!   x1 y1 .. x4 y4 in w1 (x low nibble, y high nibble of each byte, from bit 0).
//! - 0xC0 (G_NOOP, reused): load texture w1 & 0xFFF (tex.c texLoadFromGdl);
//!   w0 & 7 = texture type, type 0 carries tile shifts in w0 bits 14-17 / 10-13.
//! - 0xBB G_TEXTURE: s/t scale in w1 (0xFFFF = 1.0).
//! - 0xB8 end. Other commands (render modes, combiners, colours) don't change geometry.
//!
//! Vertices (N64 Vtx, 16 bytes: s16 x, y, z, flag, s16 s, t, u8 r, g, b, a) are
//! relative to the room's position (rooms) or the current joint (models).

use crate::rom::{s16_be, u32_be};

#[derive(Clone, Copy, Default)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub st: [f32; 2], // raw s, t (10.5 fixed point); triangles carry texel coordinates
    pub rgba: [u8; 4],
}

#[derive(Clone)]
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

/// Where a display list's vertices live and how they are placed.
pub struct Source<'a> {
    /// Memory the G_VTX addresses point into.
    pub memory: &'a [u8],
    /// G_VTX address -> offset in `memory` (None: skip the load).
    pub resolve: &'a dyn Fn(u32) -> Option<usize>,
    /// Transform for vertices loaded under a matrix (None before any G_MTX).
    pub place: &'a dyn Fn(Option<u32>, [f32; 3]) -> [f32; 3],
}

pub fn triangles(dl: &[u8], src: &Source, translucent: bool, out: &mut Vec<Triangle>) {
    let mut slots = [Vertex::default(); 16];
    let mut texture: Option<usize> = None;
    let mut matrix: Option<u32> = None;
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

    let vertices = src.memory;
    for cmd in dl.chunks_exact(8) {
        let w0 = u32_be(cmd, 0);
        let w1 = u32_be(cmd, 4);
        match cmd[0] {
            0x01 => matrix = Some((w1 & 0xFF_FFFF) / 0x40),
            0x04 => {
                let n = (((w0 >> 20) & 0xF) + 1) as usize;
                let first = ((w0 >> 16) & 0xF) as usize;
                let Some(base) = (src.resolve)(w1) else { continue };
                for i in 0..n {
                    let o = base + i * 16;
                    if first + i >= 16 || o + 16 > vertices.len() {
                        break;
                    }
                    let local = [s16_be(vertices, o) as f32, s16_be(vertices, o + 2) as f32, s16_be(vertices, o + 4) as f32];
                    slots[first + i] = Vertex {
                        pos: (src.place)(matrix, local),
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

/// A level room's display list: vertices from the room's table (segment 0x0E), placed at the room's position.
pub fn room_triangles(dl: &[u8], vertices: &[u8], room_pos: [f32; 3], translucent: bool, out: &mut Vec<Triangle>) {
    let resolve = |a: u32| Some((a & 0xFF_FFFF) as usize);
    let place = |_: Option<u32>, p: [f32; 3]| [p[0] + room_pos[0], p[1] + room_pos[1], p[2] + room_pos[2]];
    triangles(dl, &Source { memory: vertices, resolve: &resolve, place: &place }, translucent, out);
}

/// Keeps the part of each triangle where `side(position) >= 0`, cutting
/// triangles that cross the boundary (texture coordinates and colours interpolated).
pub fn clip(tris: &[Triangle], side: impl Fn([f32; 3]) -> f32) -> Vec<Triangle> {
    let lerp = |a: Vertex, b: Vertex, k: f32| {
        let mix = |x: f32, y: f32| x + (y - x) * k;
        Vertex {
            pos: [mix(a.pos[0], b.pos[0]), mix(a.pos[1], b.pos[1]), mix(a.pos[2], b.pos[2])],
            st: [mix(a.st[0], b.st[0]), mix(a.st[1], b.st[1])],
            rgba: std::array::from_fn(|i| mix(a.rgba[i] as f32, b.rgba[i] as f32).round() as u8),
        }
    };
    let mut out = Vec::with_capacity(tris.len());
    for t in tris {
        let d = t.v.map(|v| side(v.pos));
        if d.iter().all(|&x| x >= 0.0) {
            out.push(t.clone());
            continue;
        }
        if d.iter().all(|&x| x < 0.0) {
            continue;
        }
        let mut poly: Vec<Vertex> = Vec::with_capacity(4);
        for i in 0..3 {
            let j = (i + 1) % 3;
            if d[i] >= 0.0 {
                poly.push(t.v[i]);
            }
            if (d[i] >= 0.0) != (d[j] >= 0.0) {
                poly.push(lerp(t.v[i], t.v[j], d[i] / (d[i] - d[j])));
            }
        }
        for k in 1..poly.len().saturating_sub(1) {
            out.push(Triangle { texture: t.texture, translucent: t.translucent, v: [poly[0], poly[k], poly[k + 1]] });
        }
    }
    out
}
