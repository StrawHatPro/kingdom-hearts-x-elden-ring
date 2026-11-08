//! (MIMIC SORA) THE MIMIC TEAR'S LOOK ON SORA, offline: a picture of the look src/mimic.rs defines from Elden Ring's rows
//! (THE USER 2026-11-02: "Make sure mimic Sora uses the shaders that mimic should use from Elden Ring."). No GPU and no game:
//! Sora's mesh (PACK/sora.glb) posed in KH3's battle idle (kh-anim, AnimId::IdleB) with the Kingdom Key in his hand, rasterised
//! on the CPU, three panels:
//! 1. his own base colours (the glb's textures) under a plain light: the reference;
//! 2. MIMIC SORA, the G-buffer surface the mimic pass writes (`mimic::mimic_surface`: WetAspectParam 18 over his base colour on
//!    the hybrid's gloss) shaded the way a deferred metal is (GGX specular from a sun and the sky, no diffuse);
//! 3. as 2 with PhantomParam 200 (`mimic::SPIRIT_PHANTOM`: the spirits' sheen and its glow).
//!
//! THE LIGHT IS A STAND-IN for the game's (its sky, its sun, its reflection probes cannot be read offline): a sky gradient with a
//! warm horizon and dark hills, a ground, a sun. The SURFACE (albedo, metalness, gloss) and the sheen are the game's rows.
//!
//! ```text
//! cargo run --release -p kh-er-render --example mimic_pic -- [--pack build/sora_pack] [--out build/sora_tmp/party/SUMMON/pics/mimic_sora.png]
//! cargo run --release -p kh-er-render --example mimic_pic -- --beside   (one picture: Sora, and Mimic Sora beside him;
//!                                                                      default out build/sora_tmp/party/SUMMON/pics/mimic_beside_sora.png)
//! ```

use std::path::{Path, PathBuf};

use glam::{Mat4, Vec2, Vec3};
use kh_anim::Trs;
use kh_er_render::math;
use kh_er_render::mimic;
use kh_er_render::model::Model;

/// One panel, the supersampling factor.
const PANEL_W: usize = 560;
const PANEL_H: usize = 900;
const SS: usize = 2;
const FOV_Y_DEG: f32 = 30.0;

/// A base colour source of a glTF material: a factor (linear) and maybe a texture (sRGB RGBA8).
#[derive(Clone)]
struct BaseColor {
    factor: [f32; 4],
    image: Option<(usize, usize, Vec<u8>)>,
    masked: bool,
}

impl BaseColor {
    /// The linear base colour and its alpha at a UV.
    fn at(&self, uv: Vec2) -> (Vec3, f32) {
        let f = Vec3::new(self.factor[0], self.factor[1], self.factor[2]);
        match &self.image {
            None => (f, self.factor[3]),
            Some((w, h, px)) => {
                let x = ((uv.x.rem_euclid(1.0) * *w as f32) as usize).min(w - 1);
                let y = ((uv.y.rem_euclid(1.0) * *h as f32) as usize).min(h - 1);
                let p = &px[(y * w + x) * 4..(y * w + x) * 4 + 4];
                (f * Vec3::new(mimic::linear_of(p[0]), mimic::linear_of(p[1]), mimic::linear_of(p[2])), self.factor[3] * p[3] as f32 / 255.0)
            }
        }
    }
}

/// The base colours of a glb's materials by name (embedded PNG images only).
fn base_colors(glb: &Path) -> Result<std::collections::HashMap<String, BaseColor>, String> {
    let bytes = std::fs::read(glb).map_err(|e| format!("{}: {e}", glb.display()))?;
    let len = u32::from_le_bytes(bytes[12..16].try_into().map_err(|_| "truncated")?) as usize;
    let json: serde_json::Value = serde_json::from_slice(&bytes[20..20 + len]).map_err(|e| e.to_string())?;
    let bin_start = 20 + len + 8;
    let image_of = |texture: u64| -> Option<(usize, usize, Vec<u8>)> {
        let source = json["textures"][texture as usize]["source"].as_u64()?;
        let image = &json["images"][source as usize];
        // the pack's images are files beside the glb (`uri`), an embedded one a buffer view
        let data: Vec<u8> = match image["uri"].as_str() {
            Some(uri) => std::fs::read(glb.parent()?.join(uri)).ok()?,
            None => {
                let view = &json["bufferViews"][image["bufferView"].as_u64()? as usize];
                let start = bin_start + view["byteOffset"].as_u64().unwrap_or(0) as usize;
                bytes.get(start..start + view["byteLength"].as_u64()? as usize)?.to_vec()
            }
        };
        let mut decoder = png::Decoder::new(std::io::Cursor::new(data));
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = decoder.read_info().ok()?;
        let mut buf = vec![0; reader.output_buffer_size()?];
        let info = reader.next_frame(&mut buf).ok()?;
        let (w, h) = (info.width as usize, info.height as usize);
        let rgba = match info.color_type {
            png::ColorType::Rgba => buf[..w * h * 4].to_vec(),
            png::ColorType::Rgb => buf[..w * h * 3].chunks(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
            png::ColorType::GrayscaleAlpha => buf[..w * h * 2].chunks(2).flat_map(|c| [c[0], c[0], c[0], c[1]]).collect(),
            _ => buf[..w * h].iter().flat_map(|&c| [c, c, c, 255]).collect(),
        };
        Some((w, h, rgba))
    };
    // (`--outfit`) an outfit's glb names no textures: its base colour map from its materials.json (the KH3 diffuse `*_0dNN`, an
    // eye's `*BaseColor*`), the image under its materials folder
    let library: serde_json::Value = glb.parent().and_then(|d| std::fs::read_to_string(d.join("materials").join("materials.json")).ok()).and_then(|t| serde_json::from_str(t.trim_start_matches('\u{feff}')).ok()).unwrap_or_default();
    let image_of_file = |file: &str| -> Option<(usize, usize, Vec<u8>)> {
        let data = std::fs::read(glb.parent()?.join("materials").join(file)).ok()?;
        let mut decoder = png::Decoder::new(std::io::Cursor::new(data));
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = decoder.read_info().ok()?;
        let mut buf = vec![0; reader.output_buffer_size()?];
        let info = reader.next_frame(&mut buf).ok()?;
        let (w, h) = (info.width as usize, info.height as usize);
        let rgba = match info.color_type {
            png::ColorType::Rgba => buf[..w * h * 4].to_vec(),
            png::ColorType::Rgb => buf[..w * h * 3].chunks(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
            png::ColorType::GrayscaleAlpha => buf[..w * h * 2].chunks(2).flat_map(|c| [c[0], c[0], c[0], c[1]]).collect(),
            _ => buf[..w * h].iter().flat_map(|&c| [c, c, c, 255]).collect(),
        };
        Some((w, h, rgba))
    };
    let diffuse_of = |name: &str| -> Option<(usize, usize, Vec<u8>)> {
        let files: Vec<&str> = library["materials"][name]["textures"].as_array()?.iter().filter_map(|t| t["file"].as_str()).collect();
        let stem = |f: &str| Path::new(f).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let pick = files.iter().find(|f| stem(f).contains("_0d")).or_else(|| files.iter().find(|f| stem(f).contains("BaseColor")))?;
        image_of_file(pick)
    };
    let mut out = std::collections::HashMap::new();
    for m in json["materials"].as_array().into_iter().flatten() {
        let pbr = &m["pbrMetallicRoughness"];
        let f = |i: usize| pbr["baseColorFactor"][i].as_f64().unwrap_or(1.0) as f32;
        let image = pbr["baseColorTexture"]["index"].as_u64().and_then(image_of).or_else(|| diffuse_of(m["name"].as_str().unwrap_or("")));
        let masked = m["extras"]["ue_blend_mode"].as_str() == Some("BLEND_Masked");
        out.insert(m["name"].as_str().unwrap_or("").to_string(), BaseColor { factor: [f(0), f(1), f(2), f(3)], image, masked });
    }
    Ok(out)
}

/// A triangle: its corners' positions, normals and UVs, its material.
type Tri = ([Vec3; 3], [Vec3; 3], [Vec2; 3], usize);

/// A triangle soup in the component space (cm, Z up, the model faces +Y).
struct Mesh {
    tris: Vec<Tri>,
    materials: Vec<BaseColor>,
    /// (`--beside`) the materials from this index on are Mimic Sora's (usize::MAX: none)
    mimic_from: usize,
}

impl Mesh {
    /// (`--beside`) Sora `apart` cm to the left and his copy, Mimic Sora, as far to the right (the same pose, the same mesh).
    fn beside(self, apart: f32) -> Mesh {
        let n = self.materials.len();
        let shift = |t: &Tri, dx: f32, m: usize| -> Tri { (t.0.map(|p| p + Vec3::X * dx), t.1, t.2, m) };
        let mut tris: Vec<Tri> = self.tris.iter().map(|t| shift(t, -apart, t.3)).collect();
        tris.extend(self.tris.iter().map(|t| shift(t, apart, t.3 + n)));
        let mut materials = self.materials.clone();
        materials.extend(self.materials);
        Mesh { tris, materials, mimic_from: n }
    }
}

/// Sora posed (component-space bones) and skinned; the Kingdom Key at his hand socket (sora.rs `weapon_socket`).
/// (MIMIC SORA, `--outfit ID`) the worn outfit's model (PACK/outfits/<id>, kh_er_render::outfit `model_file`) posed as the
/// renderer poses it: his frame retargeted onto the outfit's skeleton (`retarget_for` + `pose_full`, the outfit's own cloth
/// simulation left out, as for a mimic in game), the Keyblade moved into the outfit's hand (`weapon_to_world`).
fn build_mesh(pack: &Path, outfit: Option<&str>) -> Result<Mesh, String> {
    let body_file = outfit.map_or_else(|| pack.join("sora.glb"), |id| kh_er_render::outfit::model_file(pack, id));
    let body = Model::load(&body_file)?;
    let key = Model::load(&pack.join("weapon").join("kingdom_key.glb"))?;
    let mut colors = base_colors(&body_file)?;
    let key_colors = base_colors(&pack.join("weapon").join("kingdom_key.glb"))?;
    let animator = kh_anim::Animator::new(kh_anim::Pack::load(pack)?, kh_anim::Settings { leg_ik: false, ..kh_anim::Settings::default() })?;
    let sequence = kh_core::AnimId::IdleB.sequences().first().copied().unwrap_or("");
    let local = animator.sample(sequence, 0.4);
    let component = kh_anim::pose::component_space(animator.skeleton(), &local);
    println!("pose: {} bones, sequence {sequence}", component.len());
    let pack_to_component = math::component_from_pack();
    let mut mats: Vec<BaseColor> = Vec::new();
    let mut index_of = std::collections::HashMap::new();
    let mut tris = Vec::new();
    let mut add = |model: &Model, joint: &dyn Fn(usize) -> Mat4, place: Mat4, colors: &mut std::collections::HashMap<String, BaseColor>, prefix: &str| {
        for p in &model.primitives {
            let key_name = format!("{prefix}{}", p.material);
            let m = *index_of.entry(key_name).or_insert_with(|| {
                mats.push(colors.remove(&p.material).unwrap_or(BaseColor { factor: [0.5; 4], image: None, masked: false }));
                mats.len() - 1
            });
            let first = p.first_index as usize;
            for t in model.indices[first..first + p.index_count as usize].chunks(3) {
                let mut pos = [Vec3::ZERO; 3];
                let mut nor = [Vec3::ZERO; 3];
                let mut uv = [Vec2::ZERO; 3];
                for (k, &i) in t.iter().enumerate() {
                    let v = &model.vertices[(i as i64 + p.base_vertex as i64) as usize];
                    let mut skin = Mat4::ZERO;
                    for c in 0..4 {
                        if v.weights[c] > 0.0 {
                            skin += joint(v.joints[c] as usize) * v.weights[c];
                        }
                    }
                    if v.weights.iter().sum::<f32>() <= 0.0 {
                        skin = Mat4::IDENTITY;
                    }
                    let to = place * skin;
                    pos[k] = to.transform_point3(Vec3::from(v.position));
                    nor[k] = to.transform_vector3(Vec3::from(v.normal)).normalize_or_zero();
                    uv[k] = Vec2::from(v.uv0);
                }
                tris.push((pos, nor, uv, m));
            }
        }
    };
    let skeleton = animator.skeleton();
    let retarget = match outfit {
        Some(id) => kh_er_render::outfit::retarget_for(pack, id, skeleton)?,
        None => None,
    };
    let posed = retarget.as_ref().map(|r| r.pose_full(&local, &component).1);
    let bones: Vec<Option<usize>> = body.joints.iter().map(|(name, _)| match &retarget { Some(r) => r.index_of(name), None => skeleton.index_of(name) }).collect();
    let pose: &[Trs] = posed.as_deref().unwrap_or(&component);
    let body_joint = |j: usize| match bones.get(j).copied().flatten().and_then(|b| pose.get(b)) {
        Some(t) => math::joint_in_pack(t) * body.joints[j].1,
        None => Mat4::IDENTITY,
    };
    add(&body, &body_joint, pack_to_component, &mut colors, "");
    // the Kingdom Key in its reference pose at R_wep_skt (R_buki x the socket's roll -90); an outfit: in ITS hand
    if let Some(hand) = skeleton.index_of("R_buki").and_then(|b| component.get(b)) {
        let half = std::f32::consts::FRAC_1_SQRT_2;
        let socket = Trs { translation: Vec3::ZERO, rotation: glam::Quat::from_xyzw(half, 0.0, 0.0, half), scale: Vec3::ONE };
        let at = socket.then(hand);
        let at = match (&retarget, &posed) {
            (Some(r), Some(p)) => r.weapon_to_world(&at, &component, p, &Trs::IDENTITY),
            _ => at,
        };
        let place = math::matrix_of(&at) * pack_to_component;
        let mut key_colors = key_colors;
        add(&key, &|_| Mat4::IDENTITY, place, &mut key_colors, "key:");
    }
    Ok(Mesh { tris, materials: mats, mimic_from: usize::MAX })
}

/// A pixel of the raster: depth (view distance), normal, base colour.
#[derive(Clone, Copy)]
struct GPixel {
    depth: f32,
    normal: Vec3,
    albedo: Vec3,
    /// (`--beside`) a pixel of Mimic Sora's
    mimic: bool,
    /// (his forming) how deep in the liquid band under the cut the pixel lies (0 = none, 1 = at the cut)
    liquid: f32,
}

struct Camera {
    eye: Vec3,
    right: Vec3,
    up: Vec3,
    forward: Vec3,
    focal: f32,
}

impl Camera {
    fn looking(eye: Vec3, at: Vec3) -> Camera {
        let forward = (at - eye).normalize();
        // the component space is the pack's (x, z, y): facing him from +Y, the screen's right is +X as in the pack's glTF view
        let right = Vec3::Z.cross(forward).normalize();
        let up = forward.cross(right).normalize();
        Camera { eye, right, up, forward, focal: 1.0 / (FOV_Y_DEG.to_radians() * 0.5).tan() }
    }

    fn project(&self, p: Vec3, w: f32, h: f32) -> Option<Vec3> {
        let d = p - self.eye;
        let z = d.dot(self.forward);
        if z < 1.0 {
            return None;
        }
        let x = d.dot(self.right) / z * self.focal * (h * 0.5) + w * 0.5;
        let y = -d.dot(self.up) / z * self.focal * (h * 0.5) + h * 0.5;
        Some(Vec3::new(x, y, z))
    }

    fn ray(&self, x: f32, y: f32, w: f32, h: f32) -> Vec3 {
        let sx = (x - w * 0.5) / (h * 0.5) / self.focal;
        let sy = -(y - h * 0.5) / (h * 0.5) / self.focal;
        (self.forward + self.right * sx + self.up * sy).normalize()
    }
}

/// (`cut`: his forming, the game z above which nothing of the mimic is drawn: kh_er_render::mimic::MimicForm::cut_z)
fn raster(mesh: &Mesh, cam: &Camera, w: usize, h: usize, cut: f32) -> Vec<Option<GPixel>> {
    let mut g: Vec<Option<GPixel>> = vec![None; w * h];
    for (pos, nor, uv, m) in &mesh.tris {
        let Some(s) = pos.iter().map(|p| cam.project(*p, w as f32, h as f32)).collect::<Option<Vec<Vec3>>>() else { continue };
        let (a, b, c) = (s[0], s[1], s[2]);
        let area = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
        if area.abs() < 1e-9 {
            continue;
        }
        let x0 = a.x.min(b.x).min(c.x).floor().max(0.0) as usize;
        let x1 = (a.x.max(b.x).max(c.x).ceil() as usize).min(w.saturating_sub(1));
        let y0 = a.y.min(b.y).min(c.y).floor().max(0.0) as usize;
        let y1 = (a.y.max(b.y).max(c.y).ceil() as usize).min(h.saturating_sub(1));
        let material = &mesh.materials[*m];
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let w0 = ((b.x - px) * (c.y - py) - (b.y - py) * (c.x - px)) / area;
                let w1 = ((c.x - px) * (a.y - py) - (c.y - py) * (a.x - px)) / area;
                let w2 = 1.0 - w0 - w1;
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                // perspective-correct weights
                let (q0, q1, q2) = (w0 / a.z, w1 / b.z, w2 / c.z);
                let qs = q0 + q1 + q2;
                let (k0, k1, k2) = (q0 / qs, q1 / qs, q2 / qs);
                let depth = 1.0 / qs;
                let at = &mut g[y * w + x];
                if at.is_some_and(|p| p.depth <= depth) {
                    continue;
                }
                let t = uv[0] * k0 + uv[1] * k1 + uv[2] * k2;
                let (albedo, alpha) = material.at(t);
                if material.masked && alpha < 0.5 {
                    continue;
                }
                let position = pos[0] * k0 + pos[1] * k1 + pos[2] * k2;
                let mimic = *m >= mesh.mimic_from;
                // (his forming: nothing above the cut, the world shows there)
                if mimic && position.z > cut {
                    continue;
                }
                let liquid = if mimic && cut.is_finite() { (1.0 - (cut - position.z) / mimic::LIQUID_BAND_CM).clamp(0.0, 1.0) } else { 0.0 };
                let mut normal = (nor[0] * k0 + nor[1] * k1 + nor[2] * k2).normalize_or_zero();
                if normal.dot(cam.eye - position) < 0.0 {
                    normal = -normal; // a two-sided slot seen from behind
                }
                *at = Some(GPixel { depth, normal, albedo, mimic, liquid });
            }
        }
    }
    g
}

/// THE STAND-IN WORLD LIGHT: the sky in a direction (scene linear), the sun.
fn sky(d: Vec3) -> Vec3 {
    let az = d.y.atan2(d.x);
    // dark hills on the horizon (a few sines of the azimuth)
    let hill = 0.025 + 0.02 * (az * 3.0).sin() + 0.012 * (az * 7.0 + 1.3).sin() + 0.008 * (az * 13.0 + 0.4).sin();
    if d.z < -0.02 {
        let fade = (-d.z).min(1.0);
        Vec3::new(0.16, 0.13, 0.09).lerp(Vec3::new(0.05, 0.045, 0.035), fade.sqrt())
    } else if d.z < hill {
        Vec3::new(0.09, 0.10, 0.08)
    } else {
        let t = d.z.clamp(0.0, 1.0).powf(0.45);
        Vec3::new(1.05, 0.92, 0.72).lerp(Vec3::new(0.22, 0.36, 0.72), t)
    }
}

const SUN_DIR: Vec3 = Vec3::new(-0.45, 0.55, 0.70);
const SUN: Vec3 = Vec3::new(3.2, 2.9, 2.5);

/// The sky seen through a glossy lobe around `r` (a cone of samples as wide as the roughness).
fn sky_lobe(r: Vec3, roughness: f32) -> Vec3 {
    let spread = (roughness * roughness * 1.6).clamp(0.002, 1.0);
    let t = if r.z.abs() < 0.9 { Vec3::Z } else { Vec3::X };
    let u = t.cross(r).normalize();
    let v = r.cross(u);
    let mut sum = Vec3::ZERO;
    let n = 24;
    for i in 0..n {
        let a = i as f32 * 2.399_963;
        let rad = spread * ((i as f32 + 0.5) / n as f32).sqrt();
        sum += sky((r + u * (rad * a.cos()) + v * (rad * a.sin())).normalize());
    }
    sum / n as f32
}

fn irradiance(n: Vec3) -> Vec3 {
    sky_lobe(n, 1.0) * 0.9
}

fn ggx(n: Vec3, v: Vec3, l: Vec3, roughness: f32, f0: Vec3) -> Vec3 {
    let h = (v + l).normalize_or_zero();
    let (nl, nv, nh, vh) = (n.dot(l).max(0.0), n.dot(v).max(1e-4), n.dot(h).max(0.0), v.dot(h).max(0.0));
    if nl <= 0.0 {
        return Vec3::ZERO;
    }
    let a = (roughness * roughness).max(2e-3);
    let a2 = a * a;
    let d = a2 / (std::f32::consts::PI * (nh * nh * (a2 - 1.0) + 1.0).powi(2));
    let k = a * 0.5;
    let g = nl / (nl * (1.0 - k) + k) * nv / (nv * (1.0 - k) + k);
    let f = f0 + (Vec3::ONE - f0) * (1.0 - vh).powi(5);
    f * (d * g / (4.0 * nl * nv)) * nl
}

/// Panel 1: his base colour under the stand-in light (Lambert, a little sheen).
fn shade_plain(p: &GPixel, v: Vec3) -> Vec3 {
    let n = p.normal;
    let sun = SUN * n.dot(SUN_DIR.normalize()).max(0.0);
    p.albedo * (irradiance(n) + sun) + ggx(n, v, SUN_DIR.normalize(), 0.7, Vec3::splat(0.04)) * SUN
}

/// Panels 2 and 3: the mimic's surface as a deferred metal (diffuse x (1 - metalness) = 0, specular from the sun and the sky).
/// Returns (diffuse light term, specular light term).
fn shade_mimic(p: &GPixel, v: Vec3) -> (Vec3, Vec3) {
    let mut s = mimic::mimic_surface(p.albedo, 0.3);
    // (his forming) the liquid band: toward a mirror and the liquid's brighter grey (er_char.wgsl KH_MIMIC_FORM)
    // (the polish round: the band takes the POOL's liquid, MimicLook's albedo and gloss, as er_char.wgsl's misc.y / misc.z)
    let look = mimic::MimicLook::DEFAULT;
    s.gloss += (look.pool_gloss - s.gloss) * p.liquid;
    s.albedo = s.albedo.lerp(Vec3::splat(look.pool_albedo), p.liquid);
    let n = p.normal;
    let roughness = 1.0 - s.gloss;
    let f0 = Vec3::splat(0.04).lerp(s.albedo, s.metalness);
    let nv = n.dot(v).max(1e-4);
    let r = (n * (2.0 * n.dot(v)) - v).normalize();
    // the sky's reflection with a Schlick Fresnel toward the grazing angle (rougher: less of it)
    let fresnel = f0 + (Vec3::splat(s.gloss).max(f0) - f0) * (1.0 - nv).powi(5);
    let spec = sky_lobe(r, roughness) * fresnel + ggx(n, v, SUN_DIR.normalize(), roughness, f0) * SUN;
    let diffuse = s.albedo * (1.0 - s.metalness) * (irradiance(n) + SUN * n.dot(SUN_DIR.normalize()).max(0.0));
    (diffuse, spec)
}

/// The pool: a metal of its own albedo (= F0) and gloss shaded as a deferred metal (the sky's reflection, the sun's GGX), no sheen.
fn shade_metal(n: Vec3, v: Vec3, albedo: Vec3, gloss: f32) -> Vec3 {
    let roughness = 1.0 - gloss;
    let nv = n.dot(v).max(1e-4);
    let r = (n * (2.0 * n.dot(v)) - v).normalize();
    let fresnel = albedo + (Vec3::splat(gloss).max(albedo) - albedo) * (1.0 - nv).powi(5);
    sky_lobe(r, roughness) * fresnel + ggx(n, v, SUN_DIR.normalize(), roughness, albedo) * SUN
}

fn tone(c: Vec3) -> [u8; 3] {
    let x = c * 0.9;
    let aces = |x: f32| ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0);
    let enc = |c: f32| if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    [(enc(aces(x.x)) * 255.0 + 0.5) as u8, (enc(aces(x.y)) * 255.0 + 0.5) as u8, (enc(aces(x.z)) * 255.0 + 0.5) as u8]
}

fn beside_flag(args: &[String]) -> bool {
    args.iter().any(|a| matches!(a.as_str(), "--beside" | "--arrival" | "--dissolve"))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let pack = arg("--pack").map(PathBuf::from).unwrap_or_else(|| root.join("build/sora_pack"));
    // (MIMIC SORA, built 2026-11-03) `--beside`: ONE picture of Sora in his own colours and Mimic Sora beside him (the mimic's
    // G-buffer surface shaded as a deferred metal with the spirits' sheen, as panel 3), side by side on the ground
    // (MIMIC SORA, the user 2026-11-03: "mimic Sora also needs to use the same outfit that Sora is currently using") `--outfit
    // ID`: both in that outfit (PACK/outfits/<id>), default out build/sora_tmp/party/SUMMON/pics/mimic_outfit_<id>.png
    let outfit = arg("--outfit");
    let beside = beside_flag(&args) || outfit.is_some();
    let default_out = match &outfit {
        Some(id) => format!("build/sora_tmp/party/SUMMON/pics/mimic_outfit_{id}.png"),
        None if beside => "build/sora_tmp/party/SUMMON/pics/mimic_beside_sora.png".to_string(),
        None => "build/sora_tmp/party/SUMMON/pics/mimic_sora.png".to_string(),
    };
    let out = arg("--out").map(PathBuf::from).unwrap_or_else(|| root.join(default_out));
    let mesh = match build_mesh(&pack, outfit.as_deref()) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("mimic_pic: {e}");
            std::process::exit(1);
        }
    };
    let mesh = if beside { mesh.beside(70.0) } else { mesh };
    println!("mesh: {} triangles, {} materials", mesh.tris.len(), mesh.materials.len());
    // the camera: in front of him (+Y), a little to his left and above, at his chest (both of them: farther back)
    let (min_z, max_z) = mesh.tris.iter().flat_map(|t| t.0).fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.z), b.max(p.z)));
    let centre = Vec3::new(0.0, 0.0, (min_z + max_z) * 0.5);
    let cam = Camera::looking(centre + if beside { Vec3::new(90.0, 560.0, 40.0) } else { Vec3::new(140.0, 520.0, 40.0) }, centre);
    let mut scene = Scene { mesh: &mesh, cam: &cam, beside, min_z, time: 0.0 };
    // (MIMIC SORA, THE USER 2026-11-03: "mimic sora, upon first being summoned, needs to play the proper mimic tear animation
    // transforming into Sora"; his death "KH3 knockdown + silver dissolve") `--arrival` / `--dissolve`: the frames of his forming
    // (kh_er_render::mimic::MimicForm: the share of his height, the pool at his feet), each a picture beside Sora and one strip of
    // all of them: build/sora_tmp/party/SUMMON/pics/mimic_arrival_<k>.png, mimic_arrival_strip.png (dissolve: the same names)
    let sequence = if args.iter().any(|a| a == "--arrival") {
        Some(("arrival", ARRIVAL.to_vec()))
    } else if args.iter().any(|a| a == "--dissolve") {
        Some(("dissolve", ARRIVAL.iter().rev().copied().collect::<Vec<_>>()))
    } else {
        None
    };
    if let Some((name, frames)) = sequence.filter(|_| beside) {
        let tall = max_z - min_z;
        // the mimic's feet (Mesh::beside: 70 cm to the right)
        let feet = Vec3::new(70.0, 0.0, min_z);
        let mut strip: Vec<Vec<Vec3>> = Vec::new();
        for (k, (height, pool)) in frames.iter().enumerate() {
            scene.time = k as f32 * 0.6;
            let form = mimic::MimicForm { height: *height, feet_z: min_z, tall, pool: *pool };
            let cut = match form.body() {
                mimic::BodyRoute::None => f32::NEG_INFINITY,
                mimic::BodyRoute::Whole => f32::INFINITY,
                mimic::BodyRoute::Forming { cut_z, .. } => cut_z,
            };
            let panel = scene.render(PANEL_W * 2, PANEL_H, cut, form.pool().map(|r| (feet, r)));
            let file = root.join(format!("build/sora_tmp/party/SUMMON/pics/mimic_{name}_{k}.png"));
            write_png(&file, std::slice::from_ref(&panel), PANEL_W * 2, PANEL_H);
            println!("mimic_pic: wrote {} (height {height:.2}, pool {pool:.0} cm)", file.display());
            strip.push(panel);
        }
        // the strip: every frame at half its size, side by side
        let (hw, hh) = (PANEL_W, PANEL_H / 2);
        let halves: Vec<Vec<Vec3>> = strip.iter().map(|p| (0..hw * hh).map(|i| {
            let (x, y) = (i % hw, i / hw);
            let at = |dx: usize, dy: usize| p[(y * 2 + dy) * PANEL_W * 2 + x * 2 + dx];
            (at(0, 0) + at(1, 0) + at(0, 1) + at(1, 1)) * 0.25
        }).collect()).collect();
        let file = root.join(format!("build/sora_tmp/party/SUMMON/pics/mimic_{name}_strip.png"));
        write_png(&file, &halves, hw, hh);
        println!("mimic_pic: wrote {} ({} frames)", file.display(), halves.len());
        return;
    }
    let panel_w = if beside { PANEL_W * 2 } else { PANEL_W };
    let panels = scene.panels(panel_w, PANEL_H);
    write_png(&out, &panels, panel_w, PANEL_H);
    if beside {
        println!("mimic_pic: wrote {} ({} x {PANEL_H}: Sora in his own colours, Mimic Sora beside him in the same outfit ({}): WetAspectParam {} + PhantomParam {})", out.display(), panel_w, outfit.as_deref().unwrap_or("kh3"), mimic::MIMIC_WET_ROW, mimic::SPIRIT_PHANTOM_ROW);
    } else {
        println!("mimic_pic: wrote {} ({} x {PANEL_H}: his own colours | Mimic Sora, WetAspectParam {} | + PhantomParam {})", out.display(), panel_w * 3, mimic::MIMIC_WET_ROW, mimic::SPIRIT_PHANTOM_ROW);
    }
}

/// (his arrival, `--arrival`) (share of his height formed, pool radius cm) per frame: the pool alone, rising 20 / 40 / 60 / 80 %,
/// whole (the pool gone). `--dissolve` plays it backwards.
const ARRIVAL: [(f32, f32); 6] = [(0.0, 55.0), (0.2, 55.0), (0.4, 50.0), (0.6, 45.0), (0.8, 40.0), (1.0, 0.0)];

/// What a picture is made of.
struct Scene<'a> {
    mesh: &'a Mesh,
    cam: &'a Camera,
    beside: bool,
    min_z: f32,
    /// the clock of the pool's ripples (s)
    time: f32,
}

impl Scene<'_> {
    /// The backdrop of a pixel: the sky, or the ground plane under him (z = his feet).
    fn backdrop(&self, ray: Vec3) -> Vec3 {
        if ray.z < 0.0 {
            let t = (self.min_z - self.cam.eye.z) / ray.z;
            let p = self.cam.eye + ray * t;
            let ground = Vec3::new(0.16, 0.13, 0.09) * (SUN.x * SUN_DIR.normalize().z * 0.35 + 0.35);
            let r = (p.x * p.x + p.y * p.y).sqrt();
            ground * (0.55 + 0.45 * (r / 160.0).min(1.0))
        } else {
            sky(ray)
        }
    }

    /// The mimic's look at a pixel: his surface as a deferred metal with the spirits' sheen.
    fn mimic_colour(p: &GPixel, v: Vec3) -> Vec3 {
        let (diffuse, spec) = shade_mimic(p, v);
        let nv = p.normal.dot(v);
        mimic::SPIRIT_PHANTOM.shade(diffuse, spec, nv) + mimic::SPIRIT_PHANTOM.glow(nv)
    }

    /// ONE picture beside Sora (`w` x `h`, supersampled), the mimic cut at `cut` (game z; +inf whole, -inf none) and his pool
    /// (feet, radius) on the ground (er_char.wgsl ER_MIMIC_POOL: the same rippled rim, the liquid's surface).
    fn render(&self, w: usize, h: usize, cut: f32, pool: Option<(Vec3, f32)>) -> Vec<Vec3> {
        let (sw, sh) = (w * SS, h * SS);
        let g = raster(self.mesh, self.cam, sw, sh, cut);
        let mut out = vec![Vec3::ZERO; sw * sh];
        for y in 0..sh {
            for x in 0..sw {
                let ray = self.cam.ray(x as f32 + 0.5, y as f32 + 0.5, sw as f32, sh as f32);
                let i = y * sw + x;
                let v = -ray;
                out[i] = match &g[i] {
                    Some(p) if p.mimic => Self::mimic_colour(p, v),
                    Some(p) => shade_plain(p, v),
                    None => match pool.filter(|_| ray.z < 0.0) {
                        Some((feet, radius)) => {
                            let z = feet.z + mimic::POOL_LIFT_CM * 0.1;
                            let at = self.cam.eye + ray * ((z - self.cam.eye.z) / ray.z);
                            let rel = (at - feet).truncate();
                            // (the polish round) the pool's own look: MimicLook::pool_at, the CPU twin of er_char.wgsl mmpool_ps
                            match mimic::MimicLook::DEFAULT.pool_at(rel / radius, rel, self.time) {
                                Some((n, albedo, gloss)) => shade_metal(n, v, Vec3::splat(albedo), gloss),
                                None => self.backdrop(ray),
                            }
                        }
                        None => self.backdrop(ray),
                    },
                };
            }
        }
        downsample(&out, w, h)
    }

    /// The three panels of the plain run (his own colours | the mimic's surface | + the sheen), or the one `--beside` picture.
    fn panels(&self, w: usize, h: usize) -> Vec<Vec<Vec3>> {
        if self.beside {
            return vec![self.render(w, h, f32::INFINITY, None)];
        }
        let (sw, sh) = (w * SS, h * SS);
        let g = raster(self.mesh, self.cam, sw, sh, f32::INFINITY);
        let mut panels = vec![vec![Vec3::ZERO; sw * sh]; 3];
        for y in 0..sh {
            for x in 0..sw {
                let ray = self.cam.ray(x as f32 + 0.5, y as f32 + 0.5, sw as f32, sh as f32);
                let i = y * sw + x;
                match &g[i] {
                    None => {
                        let bg = self.backdrop(ray);
                        for panel in panels.iter_mut() {
                            panel[i] = bg;
                        }
                    }
                    Some(p) => {
                        let v = -ray;
                        panels[0][i] = shade_plain(p, v);
                        let (diffuse, spec) = shade_mimic(p, v);
                        panels[1][i] = diffuse + spec;
                        panels[2][i] = Self::mimic_colour(p, v);
                    }
                }
            }
        }
        panels.iter().map(|p| downsample(p, w, h)).collect()
    }
}

/// A supersampled picture to its size.
fn downsample(p: &[Vec3], w: usize, h: usize) -> Vec<Vec3> {
    let sw = w * SS;
    (0..w * h)
        .map(|i| {
            let (x, y) = (i % w, i / w);
            let mut c = Vec3::ZERO;
            for sy in 0..SS {
                for sx in 0..SS {
                    c += p[(y * SS + sy) * sw + x * SS + sx];
                }
            }
            c / (SS * SS) as f32
        })
        .collect()
}

/// The panels side by side (a thin black divider between them), tone mapped, as a PNG.
fn write_png(out: &Path, panels: &[Vec<Vec3>], w: usize, h: usize) {
    let ow = w * panels.len();
    let mut img = vec![0u8; ow * h * 3];
    for (k, panel) in panels.iter().enumerate() {
        for y in 0..h {
            for x in 0..w {
                let rgb = if k > 0 && x == 0 { [0, 0, 0] } else { tone(panel[y * w + x]) };
                let at = (y * ow + k * w + x) * 3;
                img[at..at + 3].copy_from_slice(&rgb);
            }
        }
    }
    if let Some(dir) = out.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let file = match std::fs::File::create(out) {
        Ok(f) => std::io::BufWriter::new(f),
        Err(e) => {
            eprintln!("mimic_pic: {}: {e}", out.display());
            std::process::exit(1);
        }
    };
    let mut encoder = png::Encoder::new(file, ow as u32, h as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    if let Err(e) = encoder.write_header().and_then(|mut w| w.write_image_data(&img)) {
        eprintln!("mimic_pic: {}: {e}", out.display());
        std::process::exit(1);
    }
}
