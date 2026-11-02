//! GoldenEye character animations: joint angles for one frame.
//!
//! (decomp src/game/model.c loadAnimationFrame, sub_GAME_7F06DEC0,
//! modelAnimReadBitsAsU16Angle; assets/animationtable_data.c/.h.)
//! - The animation table (ROM DATA) holds ModelAnimation structs: u32 frame
//!   data address (offset into FRAMES), u16 frame count, u8 angle width in
//!   bits, u8 flags, root motion descriptors*, u16 root bits per frame, u16
//!   bits per frame, ...
//! - Frame f's bitstream starts at FRAMES + address + f * (bits per frame / 8).
//!   Joint j's rotation is three angles of `width` bits each (x, y, z), at bit
//!   offset mtxA(j) * width, read big-endian and shifted up to a u16 turn.
//!   Joint 0 (the position node) moves by root motion instead.

use crate::rom::{u16_be, u32_be};

/// Animation table: ROM offset and size (decomp scripts/filelist.u.csv animationtable_data.bin).
const DATA: (usize, usize) = (2681216, 59360);
/// Frame bitstreams (animationtable_entries.bin).
const FRAMES: (usize, usize) = (1198784, 1482432);

/// Offsets in the animation table (decomp assets/animationtable_data.h PTR_ANIM_*).
pub const ANIMS: &[(&str, usize)] = &[
    ("idle", 0x1C),
    ("fire_standing", 0x144),
    ("fire_standing_fast", 0x214),
    ("fire_hip", 0x318),
];

/// Guard skeleton: joint -> mtxA (decomp assets/embedded/skeletons/guard.inc.c JOINTLIST(guard)).
pub const GUARD_MTX_A: [u16; 16] = [0, 0, 3, 6, 9, 0xC, 0xF, 0x12, 0x15, 0x18, 0x1B, 0x1E, 0x21, 0x24, 0x27, 0x2A];

pub struct Anim {
    pub frames: u16,
    width: u8,
    frame_bytes: usize,
    address: usize,
}

pub fn load(rom: &[u8], name: &str) -> Result<Anim, String> {
    let &(_, offset) = ANIMS.iter().find(|a| a.0 == name).ok_or_else(|| format!("unknown animation {name}"))?;
    let table = rom.get(DATA.0..DATA.0 + DATA.1).ok_or("animation table outside the ROM")?;
    let s = table.get(offset..offset + 0x14).ok_or("animation entry outside the table")?;
    Ok(Anim {
        address: u32_be(s, 0) as usize,
        frames: u16_be(s, 4),
        width: s[6],
        frame_bytes: (u16_be(s, 14) >> 3) as usize,
    })
}

/// Reads `width` bits at `bit` (big-endian bit order) as a u16 turn.
fn read_angle(stream: &[u8], width: u8, bit: usize) -> u16 {
    let mut v: u32 = 0;
    for k in 0..width as usize {
        let b = bit + k;
        let byte = stream.get(b / 8).copied().unwrap_or(0);
        v = (v << 1) | ((byte >> (7 - b % 8)) & 1) as u32;
    }
    ((v << (16 - width as u32)) & 0xFFFF) as u16
}

/// Rotation (radians about x, y, z) of guard joint `joint` at `frame`.
pub fn guard_angles(rom: &[u8], anim: &Anim, frame: u16, joint: u16) -> [f32; 3] {
    let frame = frame.min(anim.frames.saturating_sub(1)) as usize;
    let start = FRAMES.0 + anim.address + frame * anim.frame_bytes;
    let Some(stream) = rom.get(start..(start + anim.frame_bytes + 1).min(FRAMES.0 + FRAMES.1)) else {
        return [0.0; 3];
    };
    let Some(&mtx) = GUARD_MTX_A.get(joint as usize) else { return [0.0; 3] };
    let w = anim.width as usize;
    let bit = mtx as usize * w;
    std::array::from_fn(|i| read_angle(stream, anim.width, bit + i * w) as f32 * std::f32::consts::TAU / 65535.0)
}
