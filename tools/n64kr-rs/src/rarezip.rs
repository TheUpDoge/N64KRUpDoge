//! Rare's "1172" compression (GoldenEye, Perfect Dark): the bytes 0x11 0x72,
//! then a raw deflate stream (no stored size). decompressdata
//! (src/game/decompress.c in the GoldenEye decomp) checks the two bytes,
//! skips them (`rz_inbuf + 2`) and inflates the rest.

use flate2::read::DeflateDecoder;
use std::io::Read;

pub fn is_1172(data: &[u8]) -> bool {
    data.len() >= 2 && data[0] == 0x11 && data[1] == 0x72
}

/// Decompresses a 1172 block. Trailing bytes after the deflate stream's end
/// (the next block, padding) are ignored.
pub fn inflate_1172(data: &[u8]) -> Result<Vec<u8>, String> {
    if !is_1172(data) {
        return Err(format!("not a 1172 block (starts {:02x?})", &data[..data.len().min(4)]));
    }
    let mut out = Vec::new();
    DeflateDecoder::new(&data[2..])
        .read_to_end(&mut out)
        .map_err(|e| format!("1172 inflate: {e}"))?;
    Ok(out)
}
