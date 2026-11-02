//! libultraship resources, the port's mod format (Rust port of the writer in
//! tools/n64kr/dkr_racer_to_model.py):
//! - DisplayList and Vertex resources as XML (the format Fast64 writes).
//! - Textures: 0x40-byte header ("OTEX", version 0) + u32 type (2 = RGBA16),
//!   width, height, data size, then big-endian RGBA5551 texels.
//!
//! Materials here are one-cycle: colour = texel x shade (vertex colour), alpha
//! = texel alpha x shade alpha, or x the primitive colour instead (effects the
//! engine tints and fades); the engine sets the render mode before drawing.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// RGBA16 texels that fit one LoadBlock into TMEM (4 KB).
const TMEM_TEXELS: usize = 2048;
/// Vertices per G_VTX load (the port's limit is 32; triangles never straddle loads).
const VERTS_PER_LOAD: usize = 30;

/// A triangle corner: position, texture coordinate (10.5 fixed point texels), colour.
#[derive(Clone, Copy)]
pub struct Corner {
    pub pos: [i16; 3],
    pub st: [i16; 2],
    pub rgba: [u8; 4],
}

pub struct Writer {
    dir: PathBuf,
    prefix: String,
    materials: HashMap<String, String>,
}

fn header() -> Vec<u8> {
    let mut h = vec![0u8; 0x40];
    h[4..8].copy_from_slice(b"XETO");
    h[12..20].copy_from_slice(&[0xef, 0xbe, 0xad, 0xde, 0xef, 0xbe, 0xad, 0xde]);
    h
}

fn log2(n: usize) -> u32 {
    usize::BITS - 1 - n.leading_zeros()
}

/// Nearest-neighbour resize to power-of-two sides that fit TMEM.
fn fit_tmem(w: usize, h: usize, rgba: &[u8]) -> (usize, usize, Vec<u8>) {
    let pow2 = |n: usize| if n.is_power_of_two() { n } else { 1usize << log2(n.max(1)) };
    let (mut tw, mut th) = (pow2(w).max(8), pow2(h).max(8));
    while tw * th > TMEM_TEXELS {
        if tw >= th {
            tw /= 2;
        } else {
            th /= 2;
        }
    }
    if (tw, th) == (w, h) {
        return (w, h, rgba.to_vec());
    }
    let mut out = vec![0u8; tw * th * 4];
    for y in 0..th {
        for x in 0..tw {
            let (sx, sy) = (x * w / tw, y * h / th);
            out[(y * tw + x) * 4..(y * tw + x) * 4 + 4].copy_from_slice(&rgba[(sy * w + sx) * 4..(sy * w + sx) * 4 + 4]);
        }
    }
    (tw, th, out)
}

impl Writer {
    pub fn new(dir: &Path, prefix: &str) -> Result<Self, String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        Ok(Writer { dir: dir.to_path_buf(), prefix: prefix.to_string(), materials: HashMap::new() })
    }

    /// Resource path of a file in this writer's folder.
    pub fn res(&self, name: &str) -> String {
        format!("{}/{name}", self.prefix)
    }

    pub fn write(&self, name: &str, data: &[u8]) -> Result<(), String> {
        std::fs::write(self.dir.join(name), data).map_err(|e| format!("{name}: {e}"))
    }

    /// Writes an RGBA16 texture (resized to fit TMEM); returns its stored size.
    pub fn texture(&self, name: &str, w: usize, h: usize, rgba: &[u8]) -> Result<(usize, usize), String> {
        let (w, h, rgba) = fit_tmem(w, h, rgba);
        let mut data = header();
        for v in [2u32, w as u32, h as u32, (w * h * 2) as u32] {
            data.extend_from_slice(&v.to_le_bytes());
        }
        for p in rgba.chunks_exact(4) {
            let v = ((p[0] as u16 >> 3) << 11) | ((p[1] as u16 >> 3) << 6) | ((p[2] as u16 >> 3) << 1) | (p[3] >= 128) as u16;
            data.extend_from_slice(&v.to_be_bytes());
        }
        self.write(name, &data)?;
        Ok((w, h))
    }

    /// A material display list (texture `tex` = (resource name, width, height),
    /// or vertex colour only), modulated by the vertex colour or, with `prim`, the primitive colour.
    pub fn material(&mut self, tex: Option<(&str, usize, usize)>, prim: bool) -> Result<String, String> {
        let key = format!("{}{}", tex.map_or("vertex_colour", |t| t.0), if prim { "/prim" } else { "" });
        if let Some(name) = self.materials.get(&key) {
            return Ok(name.clone());
        }
        let name = format!("mat_{}", self.materials.len());
        let mut x = String::from("<DisplayList Version=\"0\">\n\t<PipeSync/>\n");
        x += "\t<SetGeometryMode G_SHADE=\"1\" G_SHADING_SMOOTH=\"1\" G_ZBUFFER=\"1\"/>\n";
        x += "\t<ClearGeometryMode G_LIGHTING=\"1\" G_FOG=\"1\" G_CULL_BACK=\"1\" G_CULL_FRONT=\"1\" G_TEXTURE_GEN=\"1\"/>\n";
        x += "\t<SetCycleType G_CYC_1CYCLE=\"1\"/>\n\t<SetTextureFilter Mode=\"G_TF_BILERP\"/>\n";
        x += "\t<SetTexturePersp Enable=\"G_TP_PERSP\"/>\n\t<SetTextureLUT Mode=\"G_TT_NONE\"/>\n";
        match tex {
            Some((t, w, h)) => {
                let m = if prim { "PRIMITIVE" } else { "SHADE" };
                let lerp = format!(
                    "A0=\"G_CCMUX_TEXEL0\" B0=\"G_CCMUX_0\" C0=\"G_CCMUX_{m}\" D0=\"G_CCMUX_0\" \
                     Aa0=\"G_ACMUX_TEXEL0\" Ab0=\"G_ACMUX_0\" Ac0=\"G_ACMUX_{m}\" Ad0=\"G_ACMUX_0\" \
                     A1=\"G_CCMUX_TEXEL0\" B1=\"G_CCMUX_0\" C1=\"G_CCMUX_{m}\" D1=\"G_CCMUX_0\" \
                     Aa1=\"G_ACMUX_TEXEL0\" Ab1=\"G_ACMUX_0\" Ac1=\"G_ACMUX_{m}\" Ad1=\"G_ACMUX_0\""
                );
                let line = w * 2 / 8;
                let wrap = "Cms0=\"G_TX_WRAP\" Cms1=\"G_TX_NOMIRROR\" Cmt0=\"G_TX_WRAP\" Cmt1=\"G_TX_NOMIRROR\"";
                let _ = write!(
                    x,
                    "\t<SetCombineLERP {lerp}/>\n\
                     \t<Texture S=\"65535\" T=\"65535\" Level=\"0\" Tile=\"0\" On=\"1\"/>\n\
                     \t<SetTextureImage Path=\"{}\" Format=\"G_IM_FMT_RGBA\" Size=\"G_IM_SIZ_16b_LOAD_BLOCK\" Width=\"1\"/>\n\
                     \t<SetTile Format=\"G_IM_FMT_RGBA\" Size=\"G_IM_SIZ_16b_LOAD_BLOCK\" Line=\"0\" TMem=\"0\" Tile=\"7\" Palette=\"0\" \
                     {wrap} MaskS=\"0\" ShiftS=\"0\" MaskT=\"0\" ShiftT=\"0\"/>\n\
                     \t<LoadSync/>\n\
                     \t<LoadBlock Tile=\"7\" Uls=\"0\" Ult=\"0\" Lrs=\"{}\" Dxt=\"{}\" />\n\
                     \t<PipeSync/>\n\
                     \t<SetTile Format=\"G_IM_FMT_RGBA\" Size=\"G_IM_SIZ_16b\" Line=\"{line}\" TMem=\"0\" Tile=\"0\" Palette=\"0\" \
                     {wrap} MaskS=\"{}\" ShiftS=\"0\" MaskT=\"{}\" ShiftT=\"0\"/>\n\
                     \t<SetTileSize T=\"0\" Uls=\"0\" Ult=\"0\" Lrs=\"{}\" Lrt=\"{}\"/>\n",
                    self.res(t),
                    w * h - 1,
                    (2048 + line - 1) / line,
                    log2(w),
                    log2(h),
                    (w - 1) << 2,
                    (h - 1) << 2
                );
            }
            None => {
                x += "\t<SetCombineLERP A0=\"G_CCMUX_0\" B0=\"G_CCMUX_0\" C0=\"G_CCMUX_0\" D0=\"G_CCMUX_SHADE\" \
                      Aa0=\"G_ACMUX_0\" Ab0=\"G_ACMUX_0\" Ac0=\"G_ACMUX_0\" Ad0=\"G_ACMUX_SHADE\" \
                      A1=\"G_CCMUX_0\" B1=\"G_CCMUX_0\" C1=\"G_CCMUX_0\" D1=\"G_CCMUX_SHADE\" \
                      Aa1=\"G_ACMUX_0\" Ab1=\"G_ACMUX_0\" Ac1=\"G_ACMUX_0\" Ad1=\"G_ACMUX_SHADE\"/>\n";
                x += "\t<Texture S=\"65535\" T=\"65535\" Level=\"0\" Tile=\"0\" On=\"0\"/>\n";
            }
        }
        x += "\t<EndDisplayList/>\n</DisplayList>\n";
        self.write(&name, x.as_bytes())?;
        self.materials.insert(key, name.clone());
        Ok(name)
    }

    /// Writes a Vertex resource for `corners` (a triangle list) and returns the
    /// display list lines that draw it.
    pub fn triangles(&self, name: &str, corners: &[Corner]) -> Result<Vec<String>, String> {
        let mut v = String::from("<Vertex Version=\"0\">\n");
        for c in corners {
            let _ = writeln!(
                v,
                "\t<Vtx X=\"{}\" Y=\"{}\" Z=\"{}\" S=\"{}\" T=\"{}\" R=\"{}\" G=\"{}\" B=\"{}\" A=\"{}\"/>",
                c.pos[0], c.pos[1], c.pos[2], c.st[0], c.st[1], c.rgba[0], c.rgba[1], c.rgba[2], c.rgba[3]
            );
        }
        v += "</Vertex>\n";
        self.write(name, v.as_bytes())?;
        let mut lines = Vec::new();
        for start in (0..corners.len()).step_by(VERTS_PER_LOAD) {
            let count = VERTS_PER_LOAD.min(corners.len() - start);
            lines.push(format!(
                "\t<LoadVertices Path=\"{}\" VertexBufferIndex=\"0\" VertexOffset=\"{start}\" Count=\"{count}\"/>",
                self.res(name)
            ));
            for t in (0..count).step_by(3) {
                lines.push(format!("\t<Triangle1 V00=\"{t}\" V01=\"{}\" V02=\"{}\" Flag0=\"0\"/>", t + 1, t + 2));
            }
        }
        Ok(lines)
    }

    /// Writes a display list resource from body lines.
    pub fn display_list(&self, name: &str, body: &[String]) -> Result<(), String> {
        let mut x = String::from("<DisplayList Version=\"0\">\n");
        for l in body {
            x += l;
            x.push('\n');
        }
        x += "\t<PipeSync/>\n\t<EndDisplayList/>\n</DisplayList>\n";
        self.write(name, x.as_bytes())
    }

    pub fn call(&self, name: &str) -> String {
        format!("\t<CallDisplayList Path=\"{}\"/>", self.res(name))
    }
}
