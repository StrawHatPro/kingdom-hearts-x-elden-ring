//! (KEYBLADES) PICTURE TESTS OF THE KEYBLADE PACK, offline (no GPU, no game): every model of a plan loaded through the
//! renderer's own model loader (`kh_er_render::model::Model::load`, the path the Elden Ring renderer reads glbs with),
//! rasterised on the CPU in its bind pose with its base colours under a plain light, the tiles laid out in a grid.
//!
//! The plan (JSON, written by the pack tools: tools/sora_keyblades_pack.js `kb_pics_plan.json`, tools/sora_outfit_formcolours.js
//! `form_pics_plan.json`): `{ "columns": N, "tile": [w, h], "view": "front" | "flat", "panels": [{ "label", "glb",
//! "slots": { "<material name>": "<base colour PNG>" } }] }`. A material without a `slots` entry takes the glb's own base
//! colour texture (an image `uri` beside the glb), else a flat grey. `view`: front = the character's front (+Z, the pack's
//! glTF facing), flat = along each model's thinnest extent (a blade seen flat).
//!
//! ```text
//! cargo run --release -p kh-er-render --example kb_pics -- --plan build/sora_tmp/keyblades/kb_pics_plan.json --out build/sora_tmp/keyblades/pics/keyblades.png
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use glam::{Vec2, Vec3};
use kh_er_render::model::Model;

const SS: usize = 2;

/// An RGBA8 image (sRGB).
struct Image {
    w: usize,
    h: usize,
    px: Vec<u8>,
}

fn load_png(path: &Path) -> Option<Image> {
    let data = std::fs::read(path).ok()?;
    let mut decoder = png::Decoder::new(std::io::Cursor::new(data));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width as usize, info.height as usize);
    let px = match info.color_type {
        png::ColorType::Rgba => buf[..w * h * 4].to_vec(),
        png::ColorType::Rgb => buf[..w * h * 3].chunks(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => buf[..w * h * 2].chunks(2).flat_map(|c| [c[0], c[0], c[0], c[1]]).collect(),
        _ => buf[..w * h].iter().flat_map(|&c| [c, c, c, 255]).collect(),
    };
    Some(Image { w, h, px })
}

fn linear(c: u8) -> f32 {
    let x = c as f32 / 255.0;
    if x <= 0.040_45 {
        x / 12.92
    } else {
        ((x + 0.055) / 1.055).powf(2.4)
    }
}

/// A material's base colour: an image or a flat colour.
struct Base {
    image: Option<Image>,
    flat: Vec3,
}

impl Base {
    fn at(&self, uv: Vec2) -> Vec3 {
        match &self.image {
            None => self.flat,
            Some(img) => {
                let x = ((uv.x.rem_euclid(1.0) * img.w as f32) as usize).min(img.w - 1);
                let y = ((uv.y.rem_euclid(1.0) * img.h as f32) as usize).min(img.h - 1);
                let p = &img.px[(y * img.w + x) * 4..(y * img.w + x) * 4 + 3];
                Vec3::new(linear(p[0]), linear(p[1]), linear(p[2]))
            }
        }
    }
}

/// The glb's own base colour image per material name (`uri` images beside the glb).
fn glb_bases(glb: &Path) -> HashMap<String, PathBuf> {
    let mut out = HashMap::new();
    let Ok(bytes) = std::fs::read(glb) else { return out };
    let Some(len) = bytes.get(12..16).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize) else { return out };
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(bytes.get(20..20 + len).unwrap_or(&[])) else { return out };
    for m in json["materials"].as_array().into_iter().flatten() {
        let Some(t) = m["pbrMetallicRoughness"]["baseColorTexture"]["index"].as_u64() else { continue };
        let Some(src) = json["textures"][t as usize]["source"].as_u64() else { continue };
        if let (Some(uri), Some(name), Some(dir)) = (json["images"][src as usize]["uri"].as_str(), m["name"].as_str(), glb.parent()) {
            out.insert(name.to_string(), dir.join(uri));
        }
    }
    out
}

/// One panel rendered into a tile (linear RGB, `w` x `h`): the model in its bind pose, fitted to the tile.
fn render(model: &Model, bases: &[Base], mat_index: &[usize], flat_view: bool, w: usize, h: usize) -> Vec<Vec3> {
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for v in &model.vertices {
        lo = lo.min(Vec3::from(v.position));
        hi = hi.max(Vec3::from(v.position));
    }
    let size = hi - lo;
    // the view: the character's front (+Z) or the thinnest axis of the model; screen x / y the other two axes
    let (fwd, up) = if flat_view {
        let thin = if size.x <= size.y && size.x <= size.z { Vec3::X } else if size.y <= size.z { Vec3::Y } else { Vec3::Z };
        let long = if thin != Vec3::Y && size.y >= size.x.max(size.z) { Vec3::Y } else if thin != Vec3::X && size.x >= size.z { Vec3::X } else { Vec3::Z };
        (thin, long)
    } else {
        (Vec3::Z, Vec3::Y)
    };
    let right = up.cross(fwd).normalize();
    let centre = (lo + hi) * 0.5;
    let extent_r = (size.dot(right.abs())).max(1e-4);
    let extent_u = (size.dot(up.abs())).max(1e-4);
    let scale = 0.9 * (w as f32 / extent_r).min(h as f32 / extent_u);
    let light = (fwd * 0.8 + up * 0.5 + right * 0.3).normalize();
    let mut depth = vec![f32::MAX; w * h];
    let mut col = vec![Vec3::splat(0.06); w * h];
    let project = |p: Vec3| -> Vec3 {
        let d = p - centre;
        Vec3::new(w as f32 * 0.5 + d.dot(right) * scale, h as f32 * 0.5 - d.dot(up) * scale, -d.dot(fwd))
    };
    for (pi, p) in model.primitives.iter().enumerate() {
        let base = &bases[mat_index[pi]];
        let first = p.first_index as usize;
        for t in model.indices[first..first + p.index_count as usize].chunks(3) {
            let vs: Vec<&kh_er_render::model::Vertex> = t.iter().map(|&i| &model.vertices[(i as i64 + p.base_vertex as i64) as usize]).collect();
            let s: Vec<Vec3> = vs.iter().map(|v| project(Vec3::from(v.position))).collect();
            let area = (s[1].x - s[0].x) * (s[2].y - s[0].y) - (s[1].y - s[0].y) * (s[2].x - s[0].x);
            if area.abs() < 1e-9 {
                continue;
            }
            let x0 = s.iter().map(|q| q.x).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
            let x1 = (s.iter().map(|q| q.x).fold(f32::MIN, f32::max).ceil() as usize).min(w.saturating_sub(1));
            let y0 = s.iter().map(|q| q.y).fold(f32::MAX, f32::min).floor().max(0.0) as usize;
            let y1 = (s.iter().map(|q| q.y).fold(f32::MIN, f32::max).ceil() as usize).min(h.saturating_sub(1));
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                    let w0 = ((s[1].x - px) * (s[2].y - py) - (s[1].y - py) * (s[2].x - px)) / area;
                    let w1 = ((s[2].x - px) * (s[0].y - py) - (s[2].y - py) * (s[0].x - px)) / area;
                    let w2 = 1.0 - w0 - w1;
                    if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                        continue;
                    }
                    let z = s[0].z * w0 + s[1].z * w1 + s[2].z * w2;
                    let at = y * w + x;
                    if z >= depth[at] {
                        continue;
                    }
                    depth[at] = z;
                    let uv = Vec2::from(vs[0].uv0) * w0 + Vec2::from(vs[1].uv0) * w1 + Vec2::from(vs[2].uv0) * w2;
                    let mut n = (Vec3::from(vs[0].normal) * w0 + Vec3::from(vs[1].normal) * w1 + Vec3::from(vs[2].normal) * w2).normalize_or_zero();
                    if n.dot(fwd) < 0.0 {
                        n = -n; // seen from behind (two-sided)
                    }
                    let lit = 0.35 + 0.85 * n.dot(light).max(0.0);
                    col[at] = base.at(uv) * lit;
                }
            }
        }
    }
    col
}

fn encode(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    (s * 255.0 + 0.5) as u8
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let plan_path = arg("--plan").map(PathBuf::from).unwrap_or_else(|| root.join("build/sora_tmp/keyblades/kb_pics_plan.json"));
    let out = arg("--out").map(PathBuf::from).unwrap_or_else(|| root.join("build/sora_tmp/keyblades/pics/keyblades.png"));
    let plan: serde_json::Value = match std::fs::read_to_string(&plan_path).map_err(|e| e.to_string()).and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string())) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("kb_pics: {}: {e}", plan_path.display());
            std::process::exit(1);
        }
    };
    let cols = plan["columns"].as_u64().unwrap_or(8).max(1) as usize;
    let tw = plan["tile"][0].as_u64().unwrap_or(220) as usize;
    let th = plan["tile"][1].as_u64().unwrap_or(220) as usize;
    let flat_view = plan["view"].as_str() == Some("flat");
    let panels = plan["panels"].as_array().cloned().unwrap_or_default();
    let rows = panels.len().div_ceil(cols).max(1);
    let (ow, oh) = (cols * tw, rows * th);
    let mut img = vec![20u8; ow * oh * 3];
    let mut failed = 0;
    let mut legend = String::new();
    let mut cache: HashMap<PathBuf, Option<std::rc::Rc<Image>>> = HashMap::new();
    for (k, panel) in panels.iter().enumerate() {
        let label = panel["label"].as_str().unwrap_or("");
        let glb = root.join(panel["glb"].as_str().unwrap_or(""));
        let (cx, cy) = (k % cols, k / cols);
        legend.push_str(&format!("[{cy},{cx}] {label} ({})\n", panel["glb"].as_str().unwrap_or("")));
        let model = match Model::load(&glb) {
            Ok(m) => m,
            Err(e) => {
                println!("  FAILED {label}: {e}");
                failed += 1;
                continue;
            }
        };
        let own = glb_bases(&glb);
        let mut names: Vec<String> = Vec::new();
        let mut mat_index = Vec::new();
        for p in &model.primitives {
            let at = names.iter().position(|n| *n == p.material).unwrap_or_else(|| {
                names.push(p.material.clone());
                names.len() - 1
            });
            mat_index.push(at);
        }
        let bases: Vec<Base> = names
            .iter()
            .map(|name| {
                let file = panel["slots"][name].as_str().map(|f| root.join(f)).or_else(|| own.get(name).cloned());
                let image = file.and_then(|f| {
                    cache
                        .entry(f.clone())
                        .or_insert_with(|| load_png(&f).map(std::rc::Rc::new))
                        .as_ref()
                        .map(|i| Image { w: i.w, h: i.h, px: i.px.clone() })
                });
                let flat = if name.contains("_H_") || name.contains("ksk") { Vec3::new(0.12, 0.07, 0.04) } else { Vec3::splat(0.35) };
                Base { image, flat }
            })
            .collect();
        let tile = render(&model, &bases, &mat_index, flat_view, tw * SS, th * SS);
        for y in 0..th {
            for x in 0..tw {
                let mut c = Vec3::ZERO;
                for sy in 0..SS {
                    for sx in 0..SS {
                        c += tile[(y * SS + sy) * tw * SS + x * SS + sx];
                    }
                }
                c /= (SS * SS) as f32;
                let at = ((cy * th + y) * ow + cx * tw + x) * 3;
                img[at..at + 3].copy_from_slice(&[encode(c.x), encode(c.y), encode(c.z)]);
            }
        }
        println!("  {label}: {} vertices, {} primitives, {} material(s) ({} with an image)", model.vertices.len(), model.primitives.len(), names.len(), bases.iter().filter(|b| b.image.is_some()).count());
    }
    if let Some(dir) = out.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let written = std::fs::File::create(&out).map_err(|e| e.to_string()).and_then(|f| {
        let mut encoder = png::Encoder::new(std::io::BufWriter::new(f), ow as u32, oh as u32);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header().and_then(|mut w| w.write_image_data(&img)).map_err(|e| e.to_string())
    });
    let _ = std::fs::write(out.with_extension("txt"), legend);
    match written {
        Ok(()) => println!("kb_pics: wrote {} ({ow} x {oh}, {} panels, {failed} not loaded; legend {})", out.display(), panels.len(), out.with_extension("txt").display()),
        Err(e) => {
            eprintln!("kb_pics: {}: {e}", out.display());
            std::process::exit(1);
        }
    }
    if failed > 0 {
        std::process::exit(2);
    }
}
