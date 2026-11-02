//! Building race tracks: a banked stadium oval (two straights joined by two
//! semicircular turns), its road, edge walls and an out-of-bounds sheet, and
//! the layout JSON the SpaghettiKart track pipeline reads
//! (tools/n64kr/blender_dkr_track.py, tools/n64kr/dkr_track_scene.py).
//!
//! Everything here is in the source level's units with Y up; `Placement`
//! converts to MK64 units (start line at the origin, racing towards -Z).

use std::f32::consts::PI;
use std::fmt::Write as _;

/// Stadium oval in the x-z plane. The start line is the middle of the east
/// straight (x = cx + radius); racing goes north (-z) first, then turns west,
/// i.e. anticlockwise seen from above.
pub struct Oval {
    pub cx: f32,
    pub cz: f32,
    pub half_straight: f32,
    pub radius: f32,
    /// Road surface height (the inner edge in turns; banking raises the outside).
    pub base_y: f32,
    pub width: f32,
    pub bank_degrees: f32,
    /// Length over which banking fades in/out around each turn's ends.
    pub bank_transition: f32,
}

pub struct Sample {
    pub s: f32,
    /// Inner road edge, outward direction (horizontal), bank angle.
    pub inner: [f32; 3],
    pub outward: [f32; 3],
    pub bank: f32,
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Oval {
    pub fn lap_length(&self) -> f32 {
        4.0 * self.half_straight + 2.0 * PI * self.radius
    }

    /// Centre line point (on the flat road, before banking), outward
    /// direction, and how far inside a turn this is (negative = on a straight,
    /// distance to the nearest turn).
    fn centre(&self, s: f32) -> ([f32; 2], [f32; 2], f32) {
        let (l, r) = (self.half_straight, self.radius);
        let turn = PI * r;
        let s = s.rem_euclid(self.lap_length());
        // Segments: east half (L), north turn, west straight (2L), south turn, east half (L).
        if s < l {
            let into = -(l - s).min(s + l); // distance to the north turn start
            ([self.cx + r, self.cz - s], [1.0, 0.0], into)
        } else if s < l + turn {
            let a = (s - l) / r;
            let d = (s - l).min(l + turn - s);
            ([self.cx + r * a.cos(), self.cz - l - r * a.sin()], [a.cos(), -a.sin()], d)
        } else if s < 3.0 * l + turn {
            let t = s - l - turn;
            ([self.cx - r, self.cz - l + t], [-1.0, 0.0], -t.min(2.0 * l - t))
        } else if s < 3.0 * l + 2.0 * turn {
            let a = PI + (s - 3.0 * l - turn) / r;
            let d = (s - 3.0 * l - turn).min(3.0 * l + 2.0 * turn - s);
            ([self.cx + r * a.cos(), self.cz + l - r * a.sin()], [a.cos(), -a.sin()], d)
        } else {
            let t = s - 3.0 * l - 2.0 * turn;
            // Back half of the east straight: the south turn ended t ago; the
            // north turn starts after this half and the first half (2L - t).
            ([self.cx + r, self.cz + l - t], [1.0, 0.0], -t.min(2.0 * l - t))
        }
    }

    pub fn sample(&self, s: f32) -> Sample {
        let (c, out, inside) = self.centre(s);
        let f = smoothstep((inside + self.bank_transition / 2.0) / self.bank_transition);
        let bank = self.bank_degrees.to_radians() * f;
        let half = self.width / 2.0;
        Sample {
            s,
            inner: [c[0] - out[0] * half, self.base_y, c[1] - out[1] * half],
            outward: [out[0], 0.0, out[1]],
            bank,
        }
    }
}

impl Sample {
    /// Point `w` across the road from the inner edge (0..width), on the banked surface.
    pub fn across(&self, w: f32) -> [f32; 3] {
        let (h, v) = (w * self.bank.cos(), w * self.bank.sin());
        [self.inner[0] + self.outward[0] * h, self.inner[1] + v, self.inner[2] + self.outward[2] * h]
    }
}

/// Source units -> MK64 units: subtract the start point, scale.
pub struct Placement {
    pub origin: [f32; 3],
    pub scale: f32,
}

impl Placement {
    pub fn apply(&self, p: [f32; 3]) -> [f32; 3] {
        [
            (p[0] - self.origin[0]) * self.scale,
            (p[1] - self.origin[1]) * self.scale,
            (p[2] - self.origin[2]) * self.scale,
        ]
    }
}

/// A triangle list with UVs (normalised) and colours, in source units.
#[derive(Default)]
pub struct Mesh {
    pub positions: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub colors: Vec<[u8; 4]>,
}

impl Mesh {
    /// Quad a-b-c-d (two triangles), UVs per corner.
    pub fn quad(&mut self, p: [[f32; 3]; 4], uv: [[f32; 2]; 4], rgba: [u8; 4]) {
        for &i in &[0usize, 1, 2, 0, 2, 3] {
            self.positions.push(p[i]);
            self.uvs.push(uv[i]);
            self.colors.push(rgba);
        }
    }
}

pub struct OvalMeshes {
    pub road: Mesh,
    pub walls: Mesh,
    pub out_of_bounds: Mesh,
    pub samples: Vec<Sample>,
}

/// Road (2 strips across), walls on both edges, and an out-of-bounds sheet
/// `oob_y` covering the oval and its surroundings. Texture V restarts every
/// quad (keeps N64 texture coordinates in range); textures repeat.
pub fn build_oval(oval: &Oval, step: f32, wall_height: f32, oob_y: f32) -> OvalMeshes {
    let lap = oval.lap_length();
    let n = (lap / step).ceil() as usize;
    let samples: Vec<Sample> = (0..n).map(|i| oval.sample(i as f32 * lap / n as f32)).collect();
    let white = [255, 255, 255, 255];
    let mut road = Mesh::default();
    let mut walls = Mesh::default();
    let ds = lap / n as f32;
    let dv_road = ds / oval.width;
    let dv_wall = ds / (wall_height * 2.0);
    for i in 0..n {
        let (a, b) = (&samples[i], &samples[(i + 1) % n]);
        let v0 = (i as f32 * dv_road).fract();
        for k in 0..2 {
            let (w0, w1) = (oval.width * k as f32 / 2.0, oval.width * (k + 1) as f32 / 2.0);
            let (u0, u1) = (k as f32 / 2.0, (k + 1) as f32 / 2.0);
            // Counter-clockwise seen from above (normal up): MK64 collision
            // treats down-facing triangles as ceilings and karts fall through.
            road.quad(
                [a.across(w0), a.across(w1), b.across(w1), b.across(w0)],
                [[u0, v0], [u1, v0], [u1, v0 + dv_road], [u0, v0 + dv_road]],
                white,
            );
        }
        let w0 = (i as f32 * dv_wall).fract();
        for edge in [0.0, oval.width] {
            let (pa, pb) = (a.across(edge), b.across(edge));
            let up = |p: [f32; 3]| [p[0], p[1] + wall_height, p[2]];
            walls.quad([pa, pb, up(pb), up(pa)], [[w0, 1.0], [w0 + dv_wall, 1.0], [w0 + dv_wall, 0.0], [w0, 0.0]], white);
        }
    }
    // Out-of-bounds: one big sheet under everything, a road width past the oval.
    let m = oval.radius + oval.width * 2.0;
    let (x0, x1) = (oval.cx - m, oval.cx + m);
    let (z0, z1) = (oval.cz - oval.half_straight - m, oval.cz + oval.half_straight + m);
    let mut out_of_bounds = Mesh::default();
    out_of_bounds.quad(
        [[x0, oob_y, z0], [x0, oob_y, z1], [x1, oob_y, z1], [x1, oob_y, z0]],
        [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]],
        white,
    );
    OvalMeshes { road, walls, out_of_bounds, samples }
}

/// Horizontal distance from (x, z) to the oval's centre line.
pub fn centre_distance(oval: &Oval, x: f32, z: f32) -> f32 {
    let dz = z - oval.cz;
    if dz.abs() <= oval.half_straight {
        ((x - oval.cx).abs() - oval.radius).abs()
    } else {
        let tz = oval.cz + oval.half_straight * dz.signum();
        (((x - oval.cx).powi(2) + (z - tz).powi(2)).sqrt() - oval.radius).abs()
    }
}

/// A sentry pad: a square box `half` wide each way around `top` (its top
/// centre), down to `bottom_y`. Returns (top, sides).
pub fn pad(top: [f32; 3], half: f32, bottom_y: f32) -> (Mesh, Mesh) {
    let white = [255, 255, 255, 255];
    let (x0, x1, z0, z1, y1) = (top[0] - half, top[0] + half, top[2] - half, top[2] + half, top[1]);
    let mut t = Mesh::default();
    t.quad([[x0, y1, z0], [x0, y1, z1], [x1, y1, z1], [x1, y1, z0]], [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]], white);
    let mut sides = Mesh::default();
    let v = (y1 - bottom_y) / (half * 2.0);
    let corners = [[x0, z0], [x1, z0], [x1, z1], [x0, z1]];
    for i in 0..4 {
        let (a, b) = (corners[i], corners[(i + 1) % 4]);
        sides.quad(
            [[a[0], bottom_y, a[1]], [b[0], bottom_y, b[1]], [b[0], y1, b[1]], [a[0], y1, a[1]]],
            [[0.0, v], [1.0, v], [1.0, 0.0], [0.0, 0.0]],
            white,
        );
    }
    (t, sides)
}

/// The road's centre line in MK64 units, every `step` source units, for the
/// AI path and minimap.
pub fn path(oval: &Oval, place: &Placement, step: f32) -> Vec<[f32; 3]> {
    let lap = oval.lap_length();
    let n = (lap / step).ceil() as usize;
    (0..n).map(|i| place.apply(oval.sample(i as f32 * lap / n as f32).across(oval.width / 2.0))).collect()
}

/// `<track>_mk64.json` as blender_dkr_track.py / dkr_track_scene.py read it;
/// `extra` = more JSON members (e.g. `"actors":[...],"sequence":20032`).
pub fn layout_json(
    track: &str,
    scale: f32,
    path: &[[f32; 3]],
    item_boxes: &[[f32; 3]],
    sky_top: [u8; 3],
    sky_horizon: [u8; 3],
    extra: &str,
) -> String {
    let pts = |v: &[[f32; 3]]| {
        let mut s = String::new();
        for (i, p) in v.iter().enumerate() {
            let _ = write!(s, "{}[{:.2},{:.2},{:.2}]", if i > 0 { "," } else { "" }, p[0], p[1], p[2]);
        }
        s
    };
    format!(
        "{{\"track\":\"{track}\",\"units\":\"MK64 (N64 coordinates, Y up). Path point 0 at origin, racing towards -Z.\",\
         \"scale_from_source\":{scale},\"y_offset\":4.0,\"path\":[{}],\"checkpoints\":[],\"start_grid\":[],\
         \"item_boxes\":[{}],\"zippers\":[],\"spawn_finishline\":true,\"sky\":{{\"top\":[{},{},{}],\"horizon\":[{},{},{}]}}{}{extra}}}",
        pts(path),
        pts(item_boxes),
        sky_top[0],
        sky_top[1],
        sky_top[2],
        sky_horizon[0],
        sky_horizon[1],
        sky_horizon[2],
        if extra.is_empty() { "" } else { "," }
    )
}
