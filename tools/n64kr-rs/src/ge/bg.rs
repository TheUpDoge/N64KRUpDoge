//! GoldenEye level backgrounds: the bg file's header and room table, and each
//! room's compressed vertex table and display lists.
//!
//! Layout (decomp assets/obseg/bg/bg_all_p.h, src/game/bg.c):
//! - bg_header @0: u32 reserved, room_data_table*, portal_data_table*,
//!   global_visibility_commands*, u32 padding. Pointers are segment 0x0F
//!   addresses: 0x0F000000 + offset in the bg file.
//! - room_data_table: 24-byte entries {pointtablebinary*, primappingbinary*,
//!   secmappingbinary*, f32 x, y, z}; entry 0 is empty.
//! - Each binary is 1172-compressed (bgLoadRoomVtxData, bgLoadRoomPrimaryGdl):
//!   the point table expands to N64 Vtx (16 bytes), the mappings to display
//!   lists (8-byte Gfx; primary = opaque, secondary = transparent).

use crate::rarezip;
use crate::rom::{f32_be, u32_be};

const SEGMENT: u32 = 0x0F00_0000;

pub struct Room {
    pub index: usize,
    pub position: [f32; 3],
    pub vertices: Vec<u8>,
    pub primary_dl: Vec<u8>,
    pub secondary_dl: Option<Vec<u8>>,
}

pub struct Bg {
    pub rooms: Vec<Room>,
}

fn seg_offset(addr: u32, file_len: usize) -> Result<usize, String> {
    if addr & 0xFF00_0000 != SEGMENT {
        return Err(format!("address {addr:#010x} is not in segment 0x0F"));
    }
    let o = (addr - SEGMENT) as usize;
    if o >= file_len {
        return Err(format!("address {addr:#010x} past the end of the bg file"));
    }
    Ok(o)
}

fn inflate_at(file: &[u8], addr: u32) -> Result<Vec<u8>, String> {
    rarezip::inflate_1172(&file[seg_offset(addr, file.len())?..])
}

pub fn parse(file: &[u8]) -> Result<Bg, String> {
    let room_table = seg_offset(u32_be(file, 4), file.len())?;
    let portal_table = seg_offset(u32_be(file, 8), file.len())?;
    let vis_table = seg_offset(u32_be(file, 12), file.len())?;
    // The room table ends where the next table starts.
    let end = [portal_table, vis_table].into_iter().filter(|&o| o > room_table).min().unwrap_or(file.len());

    let mut rooms = Vec::new();
    let mut o = room_table + 24; // entry 0 is empty
    let mut index = 1;
    while o + 24 <= end {
        let point = u32_be(file, o);
        let pri = u32_be(file, o + 4);
        let sec = u32_be(file, o + 8);
        // The table ends with an all-zero entry, preceded by a marker entry
        // whose point table is empty (not 1172 data; Dam's room 137).
        if point == 0 || !rarezip::is_1172(&file[seg_offset(point, file.len())?..]) {
            break;
        }
        rooms.push(Room {
            index,
            position: [f32_be(file, o + 12), f32_be(file, o + 16), f32_be(file, o + 20)],
            vertices: inflate_at(file, point).map_err(|e| format!("room {index} vertices: {e}"))?,
            primary_dl: inflate_at(file, pri).map_err(|e| format!("room {index} primary DL: {e}"))?,
            secondary_dl: if sec != 0 {
                Some(inflate_at(file, sec).map_err(|e| format!("room {index} secondary DL: {e}"))?)
            } else {
                None
            },
        });
        o += 24;
        index += 1;
    }
    Ok(Bg { rooms })
}

/// Texture numbers a room's display lists reference: GoldenEye reuses G_NOOP
/// (0xC0 in Fast3D) as "load texture" with the number in w1 bits 0-11, and a
/// detail texture's number in bits 12-23 for type 1 (tex.c texLoadFromGdl).
pub fn texture_refs(dl: &[u8]) -> Vec<usize> {
    let mut out = Vec::new();
    for cmd in dl.chunks_exact(8) {
        if cmd[0] == 0xC0 {
            let w0 = u32_be(cmd, 0);
            let w1 = u32_be(cmd, 4);
            out.push((w1 & 0xFFF) as usize);
            if w0 & 7 == 1 {
                out.push(((w1 >> 12) & 0xFFF) as usize);
            }
        }
    }
    out
}
