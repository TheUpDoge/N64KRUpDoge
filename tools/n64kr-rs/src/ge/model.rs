//! GoldenEye object models: props ("P...Z"), characters ("C...Z": bodies and
//! heads), posed in their bind pose or by an animation frame (ge::anim).
//!
//! Format (decomp scripts/generate_prop_model_c.py, src/bondtypes.h): the file
//! is 1172-compressed; inside, pointers are 0x05000000 + file offset.
//! - numSwitches u32 switch pointers, then numtextures 12-byte texture records
//!   (u32 image number, u8 width, height, ...). Both counts live in the game's
//!   ModelFileHeader, not in the file (MODEL_FILES below).
//! - The root ModelNode follows. ModelNode (24 bytes): u16 opcode (low byte;
//!   the high byte flags characters' extra matrices), data*, parent*, next*,
//!   prev*, child*.
//! - Opcodes read here: 2 group (f32 origin x, y, z in the parent's frame,
//!   u16 joint, s16 matrix ids 0/1/2), 4 display list (primary*, secondary*,
//!   base, vertices*, u16 count), 12 gunfire (f32 offset x, y, z), 22 primary
//!   display list (s32 count, vertices*, primary*), 23 head placeholder,
//!   24 collision display list (primary*, secondary*, vertices*, s16 count).
//!
//! Posing (src/game/model.c modelBuildGroupMatrices, matrixmath.c): a group's
//! matrix 0 = parent x (rotation about x, y, z by the joint's animation angles,
//! translated by the origin); matrix 1 is the same with half the rotation
//! (the skin across a joint) [matrix 2, a half y bend with stretch, is read as
//! matrix 1: CONCLUSION, close enough for a held pose]. Display list vertices
//! are in the frame of the current group, or after a G_MTX, of the matrix it names.

use crate::ge::dl::{self, Triangle};
use crate::rarezip;
use crate::rom::{f32_be, s16_be, u16_be, u32_be};

/// Model files used by N64KR: (file name, ROM offset, size, numSwitches,
/// numtextures) (decomp scripts/filelist.u.csv, assets/obseg/*/ModelFileHeader).
pub const MODEL_FILES: &[(&str, usize, usize, usize, usize)] = &[
    ("PgasbarrelZ", 8254544, 528, 0, 2),
    ("PgasbarrelsZ", 8255072, 1344, 0, 2),
    ("Poil_drum1Z", 8358976, 624, 0, 2),
    ("PexplosionbitZ", 8248928, 288, 0, 0),
    ("PchrkalashZ", 8149728, 1008, 3, 8),
    ("ColiveguardZ", 7585152, 9808, 7, 16),
    ("CcamguardZ", 7347824, 9952, 7, 14),
    ("CheadbalaclavaZ", 7443184, 976, 2, 2),
    ("CheaddaveZ", 7463760, 1408, 2, 4),
    ("CheadduncanZ", 7466496, 1312, 2, 5),
    ("CheadgrantZ", 7470608, 1328, 2, 4),
    ("CheadjimZ", 7471936, 1328, 2, 4),
    ("CheadkarlZ", 7477008, 1376, 2, 6),
    ("CheadmarkZ", 7483200, 1328, 2, 5),
    ("CheadmartinZ", 7484528, 1376, 2, 6),
    ("CheadpeteZ", 7488592, 1424, 2, 4),
    ("CheadscottZ", 7492336, 1408, 2, 5),
    ("CheadshaunZ", 7493744, 1504, 2, 4),
    // The gun barrel intro: Bond in his tuxedo, Brosnan's head, the PP7.
    ("CdjbondZ", 7382992, 11920, 7, 13),
    ("CheadbrosnanZ", 7445856, 3408, 2, 5),
    ("PchrwppkZ", 8189712, 576, 3, 5),
];

/// Row-vector 4x4 matrix as GoldenEye keeps them: p' = p.x m[0] + p.y m[1] + p.z m[2] + m[3].
pub type Mat = [[f32; 4]; 4];

pub const IDENTITY: Mat = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];

/// matrix_4x4_set_rotation_around_xyz + set_position.
pub fn rotation_xyz(a: [f32; 3], pos: [f32; 3]) -> Mat {
    let (sx, cx) = a[0].sin_cos();
    let (sy, cy) = a[1].sin_cos();
    let (sz, cz) = a[2].sin_cos();
    [
        [cy * cz, cy * sz, -sy, 0.0],
        [sx * cz * sy - cx * sz, sx * sz * sy + cx * cz, sx * cy, 0.0],
        [cx * cz * sy + sx * sz, cx * sz * sy - sx * cz, cx * cy, 0.0],
        [pos[0], pos[1], pos[2], 1.0],
    ]
}

/// matrix_4x4_multiply_homogeneous(lhs, rhs): rhs applied first, then lhs.
pub fn mul(lhs: &Mat, rhs: &Mat) -> Mat {
    let mut r = IDENTITY;
    for i in 0..3 {
        for j in 0..4 {
            r[j][i] = lhs[0][i] * rhs[j][0] + lhs[1][i] * rhs[j][1] + lhs[2][i] * rhs[j][2] + if j == 3 { lhs[3][i] } else { 0.0 };
        }
    }
    r
}

pub fn apply(m: &Mat, p: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| p[0] * m[0][i] + p[1] * m[1][i] + p[2] * m[2][i] + m[3][i])
}

pub struct Group {
    pub joint: u16,
    pub matrices: [i16; 3],
    pub parent: Option<usize>,
    pub origin: [f32; 3],
}

struct DlNode {
    group: Option<usize>,
    lists: Vec<(usize, bool)>,
    vertices: usize,
}

pub struct Model {
    data: Vec<u8>,
    pub textures: Vec<usize>,
    pub groups: Vec<Group>,
    dls: Vec<DlNode>,
    /// Group holding the head placeholder (characters).
    pub head_group: Option<usize>,
    /// Muzzle flash offset and its group (weapons).
    pub gunfire: Option<([f32; 3], Option<usize>)>,
}

/// A posed model: triangles and each group's matrix 0.
pub struct Posed {
    pub triangles: Vec<Triangle>,
    pub matrices: Vec<Mat>,
}

pub fn file(name: &str) -> Option<(usize, usize, usize, usize)> {
    MODEL_FILES.iter().find(|f| f.0 == name).map(|f| (f.1, f.2, f.3, f.4))
}

fn ptr(v: u32) -> Option<usize> {
    match v {
        0 => None,
        v if v >= 0x0500_0000 => Some((v - 0x0500_0000) as usize),
        v => Some(v as usize),
    }
}

/// Parses a compressed model file from the ROM.
pub fn load(rom: &[u8], name: &str) -> Result<Model, String> {
    let (offset, size, switches, textures) = file(name).ok_or_else(|| format!("unknown model file {name}"))?;
    let packed = rom.get(offset..offset + size).ok_or("model file outside the ROM")?;
    parse(rarezip::inflate_1172(packed)?, switches, textures)
}

pub fn parse(data: Vec<u8>, num_switches: usize, num_textures: usize) -> Result<Model, String> {
    let tex_table = num_switches * 4;
    let root = tex_table + num_textures * 12;
    if root + 24 > data.len() {
        return Err("model file too short".into());
    }
    let textures = (0..num_textures).map(|i| (u32_be(&data, tex_table + i * 12) & 0xFFF) as usize).collect();

    // Walk the node tree (child first, then next), noting groups and display lists.
    let mut groups: Vec<Group> = Vec::new();
    let mut dls: Vec<DlNode> = Vec::new();
    let (mut head_group, mut gunfire) = (None, None);
    let mut seen = std::collections::HashSet::new();
    let mut stack = vec![(root, None::<usize>)];
    let d = &data;
    while let Some((node, group)) = stack.pop() {
        if node + 24 > d.len() || !seen.insert(node) {
            continue;
        }
        let opcode = d[node + 1];
        let rec = ptr(u32_be(d, node + 4)).filter(|&r| r < d.len());
        let (next, child) = (ptr(u32_be(d, node + 12)), ptr(u32_be(d, node + 20)));
        let mut child_group = group;
        let list = |o: usize| ptr(u32_be(d, o));
        match (opcode, rec) {
            (2, Some(r)) if r + 20 <= d.len() => {
                groups.push(Group {
                    joint: u16_be(d, r + 12),
                    matrices: [s16_be(d, r + 14), s16_be(d, r + 16), s16_be(d, r + 18)],
                    parent: group,
                    origin: [f32_be(d, r), f32_be(d, r + 4), f32_be(d, r + 8)],
                });
                child_group = Some(groups.len() - 1);
            }
            (4, Some(r)) if r + 18 <= d.len() => dls.push(DlNode {
                group,
                lists: [(list(r), false), (list(r + 4), true)].iter().filter_map(|&(p, x)| p.map(|p| (p, x))).collect(),
                vertices: list(r + 12).unwrap_or(0),
            }),
            (22, Some(r)) if r + 12 <= d.len() => {
                if let Some(p) = list(r + 8) {
                    dls.push(DlNode { group, lists: vec![(p, false)], vertices: list(r + 4).unwrap_or(0) });
                }
            }
            (24, Some(r)) if r + 12 <= d.len() => dls.push(DlNode {
                group,
                lists: [(list(r), false), (list(r + 4), true)].iter().filter_map(|&(p, x)| p.map(|p| (p, x))).collect(),
                vertices: list(r + 8).unwrap_or(0),
            }),
            (12, Some(r)) if r + 12 <= d.len() => gunfire = Some(([f32_be(d, r), f32_be(d, r + 4), f32_be(d, r + 8)], group)),
            (23, _) => head_group = group,
            _ => {}
        }
        if let Some(n) = next {
            stack.push((n, group));
        }
        if let Some(c) = child {
            stack.push((c, child_group));
        }
    }
    Ok(Model { data, textures, groups, dls, head_group, gunfire })
}

impl Model {
    /// Display list nodes: (group, G_VTX segments, G_MTX count), for inspection.
    pub fn dl_summary(&self) -> Vec<(Option<usize>, Vec<u8>, usize)> {
        self.dls
            .iter()
            .map(|n| {
                let (mut segs, mut mtx) = (Vec::new(), 0);
                for &(list, _) in &n.lists {
                    for c in self.data.get(list..).unwrap_or(&[]).chunks_exact(8) {
                        match c[0] {
                            0x04 if !segs.contains(&c[4]) => segs.push(c[4]),
                            0x01 => mtx += 1,
                            0xB8 => break,
                            _ => {}
                        }
                    }
                }
                (n.group, segs, mtx)
            })
            .collect()
    }

    /// Poses the model: `angles(joint)` = rotation about x, y, z in radians
    /// (None for the bind pose), under `root` (e.g. a placement or a hand's matrix).
    pub fn pose(&self, angles: Option<&dyn Fn(u16) -> [f32; 3]>, root: &Mat) -> Posed {
        use std::f32::consts::{PI, TAU};
        // Groups are listed parents first (the walk pushes a group before its children).
        let mut full: Vec<Mat> = Vec::with_capacity(self.groups.len());
        let mut half: Vec<Mat> = Vec::with_capacity(self.groups.len());
        // Half rotation for the skin across a joint: each angle halved the short way round.
        let halve = |x: f32| {
            let x = x.rem_euclid(TAU);
            if x < PI { x / 2.0 } else { x / 2.0 + PI }
        };
        for g in &self.groups {
            let parent = g.parent.map_or(*root, |p| full[p]);
            let a = angles.map_or([0.0; 3], |f| f(g.joint));
            full.push(mul(&parent, &rotation_xyz(a, g.origin)));
            half.push(mul(&parent, &rotation_xyz(a.map(halve), g.origin)));
        }
        // Matrix id -> matrix (0: full rotation, 1 and 2: half). Matrix 0 wins a clash.
        let mut by_id: std::collections::HashMap<u32, Mat> = std::collections::HashMap::new();
        for (i, g) in self.groups.iter().enumerate() {
            for &id in &g.matrices[1..] {
                if id >= 0 {
                    by_id.insert(id as u32, half[i]);
                }
            }
        }
        for (i, g) in self.groups.iter().enumerate() {
            if g.matrices[0] >= 0 {
                by_id.insert(g.matrices[0] as u32, full[i]);
            }
        }

        let mut triangles = Vec::new();
        for n in &self.dls {
            let base = n.group.map_or(*root, |g| full[g]);
            let resolve = |a: u32| -> Option<usize> {
                match a >> 24 {
                    4 => Some(n.vertices + (a & 0xFF_FFFF) as usize),
                    _ => Some((a & 0xFF_FFFF) as usize),
                }
            };
            let by_id = &by_id;
            let place = move |m: Option<u32>, p: [f32; 3]| apply(m.and_then(|m| by_id.get(&m)).unwrap_or(&base), p);
            for &(list, translucent) in &n.lists {
                let Some(cmds) = self.data.get(list..) else { continue };
                let end = cmds.chunks_exact(8).position(|c| c[0] == 0xB8).map_or(cmds.len(), |i| (i + 1) * 8);
                dl::triangles(
                    &cmds[..end],
                    &dl::Source { memory: &self.data, resolve: &resolve, place: &place },
                    translucent,
                    &mut triangles,
                );
            }
        }
        Posed { triangles, matrices: full }
    }
}
