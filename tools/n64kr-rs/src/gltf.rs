//! Minimal glTF 2.0 writer: one mesh, one primitive per material, positions +
//! UVs + vertex colours, PNG textures beside the .gltf.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

pub struct Material {
    pub name: String,
    /// PNG file name relative to the .gltf, if textured.
    pub image: Option<String>,
    /// "OPAQUE", "MASK" or "BLEND".
    pub alpha_mode: &'static str,
}

pub struct Primitive {
    pub material: usize,
    pub positions: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub colors: Vec<[u8; 4]>,
}

fn align4(buf: &mut Vec<u8>) {
    while buf.len() % 4 != 0 {
        buf.push(0);
    }
}

pub fn write(dir: &Path, name: &str, materials: &[Material], primitives: &[Primitive]) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut bin = Vec::new();
    let mut views = String::new();
    let mut accessors = String::new();
    let mut prims = String::new();
    let mut n_views = 0;

    let mut add_view = |bin: &mut Vec<u8>, data: &[u8], views: &mut String| -> usize {
        align4(bin);
        let _ = write!(views, "{}{{\"buffer\":0,\"byteOffset\":{},\"byteLength\":{}}}", if n_views > 0 { "," } else { "" }, bin.len(), data.len());
        bin.extend_from_slice(data);
        n_views += 1;
        n_views - 1
    };
    let mut n_acc = 0;
    for (pi, p) in primitives.iter().enumerate() {
        let count = p.positions.len();
        let mut pos = Vec::with_capacity(count * 12);
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for v in &p.positions {
            for k in 0..3 {
                pos.extend_from_slice(&v[k].to_le_bytes());
                lo[k] = lo[k].min(v[k]);
                hi[k] = hi[k].max(v[k]);
            }
        }
        let uv: Vec<u8> = p.uvs.iter().flat_map(|t| t.iter().flat_map(|c| c.to_le_bytes())).collect();
        let col: Vec<u8> = p.colors.iter().flat_map(|c| c.iter().copied()).collect();
        let vp = add_view(&mut bin, &pos, &mut views);
        let vu = add_view(&mut bin, &uv, &mut views);
        let vc = add_view(&mut bin, &col, &mut views);
        let sep = if n_acc > 0 { "," } else { "" };
        let _ = write!(
            accessors,
            "{sep}{{\"bufferView\":{vp},\"componentType\":5126,\"count\":{count},\"type\":\"VEC3\",\"min\":[{},{},{}],\"max\":[{},{},{}]}}\
             ,{{\"bufferView\":{vu},\"componentType\":5126,\"count\":{count},\"type\":\"VEC2\"}}\
             ,{{\"bufferView\":{vc},\"componentType\":5121,\"normalized\":true,\"count\":{count},\"type\":\"VEC4\"}}",
            lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]
        );
        let _ = write!(
            prims,
            "{}{{\"attributes\":{{\"POSITION\":{},\"TEXCOORD_0\":{},\"COLOR_0\":{}}},\"material\":{},\"mode\":4}}",
            if pi > 0 { "," } else { "" },
            n_acc,
            n_acc + 1,
            n_acc + 2,
            p.material
        );
        n_acc += 3;
    }

    let mut mats = String::new();
    let mut images = String::new();
    let mut textures = String::new();
    let mut n_img = 0;
    for (i, m) in materials.iter().enumerate() {
        let tex = if let Some(img) = &m.image {
            let _ = write!(images, "{}{{\"uri\":\"{img}\"}}", if n_img > 0 { "," } else { "" });
            let _ = write!(textures, "{}{{\"source\":{n_img},\"sampler\":0}}", if n_img > 0 { "," } else { "" });
            n_img += 1;
            format!(",\"baseColorTexture\":{{\"index\":{}}}", n_img - 1)
        } else {
            String::new()
        };
        let cutoff = if m.alpha_mode == "MASK" { ",\"alphaCutoff\":0.5" } else { "" };
        let _ = write!(
            mats,
            "{}{{\"name\":\"{}\",\"pbrMetallicRoughness\":{{\"metallicFactor\":0,\"roughnessFactor\":1{tex}}},\"doubleSided\":true,\"alphaMode\":\"{}\"{cutoff}}}",
            if i > 0 { "," } else { "" },
            m.name,
            m.alpha_mode
        );
    }

    let bin_name = format!("{name}.bin");
    fs::write(dir.join(&bin_name), &bin).map_err(|e| format!("{bin_name}: {e}"))?;
    let json = format!(
        "{{\"asset\":{{\"version\":\"2.0\",\"generator\":\"N64KR n64kr\"}},\"scene\":0,\"scenes\":[{{\"nodes\":[0]}}],\
         \"nodes\":[{{\"name\":\"{name}\",\"mesh\":0}}],\"meshes\":[{{\"name\":\"{name}\",\"primitives\":[{prims}]}}],\
         \"materials\":[{mats}],\"images\":[{images}],\"textures\":[{textures}],\
         \"samplers\":[{{\"magFilter\":9729,\"minFilter\":9729,\"wrapS\":10497,\"wrapT\":10497}}],\
         \"accessors\":[{accessors}],\"bufferViews\":[{views}],\"buffers\":[{{\"uri\":\"{bin_name}\",\"byteLength\":{}}}]}}",
        bin.len()
    );
    fs::write(dir.join(format!("{name}.gltf")), json).map_err(|e| format!("{name}.gltf: {e}"))?;
    Ok(())
}
