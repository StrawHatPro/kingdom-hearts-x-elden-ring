//! (IMPL_ER60) THE BLOOD ON SORA AND THE KEYBLADE: what the host (kh-er `blood.rs`, the amounts by Elden Ring's rules)
//! hands over, and the block of the frame constants the `<variant>_bl` pixel shaders read (er_char.wgsl `bl_*`,
//! `kh_blood`): the amounts, his feet, his hands and the Keyblade's blade, in game space (cm).
//!
//! The `*_bl` set is the `*_pl` set plus the blood layer; gfx.rs takes it when the pack has ALL of them (and the `*_pl`
//! set is in use), else everything stays as before. With both amounts 0 the shaders return the material's values
//! untouched (the picture of the `*_pl` set).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use glam::Vec3;

use crate::FrameInput;

/// What the host says about the blood on him (all 0..1).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BloodLook {
    /// On his body and clothes (the region rule decides where first: boots and legs, hands, then the rest).
    pub body: f32,
    /// On the Keyblade.
    pub blade: f32,
    /// 1 = fresh (wet, glossy), 0 = dried (dull). (BLOOD-RED) the colour is the same: Elden Ring's blood paint ([`ER_BLOOD_COLOR`]).
    pub fresh: f32,
    /// The splatter pattern's seed (a new one after each wash).
    pub seed: f32,
}

static LOOK: Mutex<BloodLook> = Mutex::new(BloodLook { body: 0.0, blade: 0.0, fresh: 0.0, seed: 0.0 });
/// The host lets the renderer take the `*_bl` set (read when the shaders are loaded).
static WANTED: AtomicBool = AtomicBool::new(true);
/// The `*_bl` set is the one in use.
static IN_USE: AtomicBool = AtomicBool::new(false);

/// (BLOOD-RED, the user 2026-11-07: "blood on the KH characters needs to be a bit redder it's very brown right now") THE
/// COLOUR OF THE BLOOD: Elden Ring's own blood paint on characters, read offline from the game's data (read only):
/// - DecalParam 600109000..600109004 (regulation 2.7.2.0; `usePaintDecal` 1, `bloodTypeEnable` 1, life 180 s + 30 s fade):
///   `diffuseColor` (255, 15, 15) over `diffuseTextureId` 100000, `reflecColor` (255, 200, 200), mask 170100;
/// - texture 100000 = `/other/decaltex.tpf.dcx` `dp000100000_a` (Data0.bdt; 256 x 256 BC1 sRGB, a grey cloud): in linear
///   light its mean is 0.0526, its 10th percentile 0.0307, its 90th 0.0782.
///
/// The blood's albedo is that colour (its bytes read as sRGB, in linear light: the convention of mimic.rs) x the texture,
/// from its light end (90th percentile) where a splat is thin to its dark end (10th) in the core, and it REPLACES the
/// cloth's colour under the mask as the game's paint does. The game's paint keeps one colour for its whole life, so drying
/// takes only the wet gloss. The shader holds the same numbers (er_char.wgsl `BL_ER_*`; the test reads them there) and the
/// sheet rows `blood.look.*` of sora-er-host.json carry them with this evidence (the tests compare all three).
/// Before: an own palette MULTIPLIED over the cloth (green / blue 15..35 % of red, dried toward (0.34, 0.12, 0.08)): brown,
/// and orange / brick on yellow / white cloth.
pub const ER_BLOOD_COLOR_SRGB: [u8; 3] = [255, 15, 15];
/// The texture's light end (thin blood) and dark end (thick blood), linear.
pub const ER_BLOOD_TEX_THIN: f32 = 0.0782;
pub const ER_BLOOD_TEX_THICK: f32 = 0.0307;

/// An sRGB byte in linear light.
pub fn srgb_to_linear(byte: u8) -> f32 {
    let c = f32::from(byte) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// [`ER_BLOOD_COLOR_SRGB`] in linear light (er_char.wgsl `BL_ER_COLOR`).
pub fn er_blood_color() -> Vec3 {
    let [r, g, b] = ER_BLOOD_COLOR_SRGB;
    Vec3::new(srgb_to_linear(r), srgb_to_linear(g), srgb_to_linear(b))
}

/// The floats of the block (7 x vec4).
pub const BLOCK_FLOATS: usize = 28;
/// His height for the region rule (cm; the thrown shadow's `blob_fade.z` uses the same).
const HEIGHT_CM: f32 = 150.0;
/// The reach of a hand's blood (cm from the hand bone).
const HAND_REACH_CM: f32 = 16.0;
/// The blade from the start of the shaft to the tip (cm along the bone `wep`, shadowcaps.rs) and its reach off the axis
/// (the teeth stand 13.6 cm off it).
const BLADE_FROM_CM: f32 = 8.0;
const BLADE_TO_CM: f32 = 79.0;
const BLADE_REACH_CM: f32 = 15.0;
/// The head: its middle this far beyond the bone `atama` (away from the neck `kubi`), and the reach of the head rule
/// (as the thrown shadow's head ball: 6 .. 16 cm beyond the bone, radius 17).
const HEAD_BEYOND_CM: f32 = 9.0;
const HEAD_REACH_CM: f32 = 20.0;

/// (ER85, the user: "Sora's face should be able to get bloody as well") THE FACE TAKES BLOOD: the head rule (fewer, only
/// droplets on the face and hair) is not written, so the head takes it by the same region rule as the rest of him (the
/// upper body's share); the eyes and the mouth never do (`spared`: their draws read the block with the amounts at 0).
/// kh-er key `blood.face` (1; 0 = the head rule as before ER85).
static FACE: AtomicBool = AtomicBool::new(true);

pub fn set_face(on: bool) {
    FACE.store(on, Ordering::Relaxed);
}

pub fn face() -> bool {
    FACE.load(Ordering::Relaxed)
}

/// (ER85) A slot that never takes blood: the eyes (eyeball, iris, lens, wet line, lashes) and the mouth's inside (teeth,
/// tongue). (BLOOD-RED 2026-11-07, the coordinator: "identify mouth/eye slots by what they really are ... so no cloth slot is
/// spared") by WHAT THE MATERIAL IS, from KH3's own data, never by a slot's name:
/// - its KH3 base material (the root of the instance's parent chain, [`EYE_MOUTH_ROOTS`]): KH3's eye and mouth shaders;
/// - or its colour texture is KH3's eye / mouth sheet (`<char>_E_0dNN` / `<char>_M_0dNN`, `Mouth_M_0dNN`): the outfits
///   whose eyes / mouth use a body base material (Toy Box, Monstropolis's mouth, Caribbean's realistic mouth).
pub fn spared(slot: &kh_look::library::Slot) -> bool {
    EYE_MOUTH_ROOTS.iter().any(|r| slot.root.eq_ignore_ascii_case(r)) || slot.textures.iter().flatten().any(|t| eye_mouth_sheet(&t.file))
}

/// (BLOOD-RED) KH3's base materials of eyes and mouths (Material_set: SQEX_* and the DLC's), as the packs' materials.json
/// `root` names them.
pub const EYE_MOUTH_ROOTS: [&str; 8] = ["SQEX_Mouth", "SQEX_DLC_Mouth", "SQEX_EyeLens", "SQEX_IrisTexEyeMaterial", "SQEX_WhiteEyeMaterial", "SQEX_BaseEyelashes", "SQEX_DLC_EyeBaseMat", "SQEX_DLC_MatsugeBaseMat"];

/// (BLOOD-RED) A texture file that is KH3's eye or mouth colour sheet: `<a>_<b><3 digits>_E_0d<NN>` / `_M_0d<NN>` (the
/// character's eye / mouth part letter before the colour map kind `0d`), or the shared `Mouth_M_0d<NN>`.
pub fn eye_mouth_sheet(file: &str) -> bool {
    let stem = file.rsplit(['/', '\\']).next().unwrap_or(file);
    let stem = stem.split('.').next().unwrap_or(stem).to_ascii_lowercase();
    let parts: Vec<&str> = stem.split('_').collect();
    let colour = |p: &str| p.len() == 4 && p.starts_with("0d") && p[2..].chars().all(|c| c.is_ascii_digit());
    match parts.as_slice() {
        ["mouth", "m", c] => colour(c),
        [a, b, part, c] => a.len() <= 2 && b.len() == 5 && b[2..].chars().all(|ch| ch.is_ascii_digit()) && (*part == "e" || *part == "m") && colour(c),
        _ => false,
    }
}

/// (BLOOD-RED, the user 2026-11-07: "the whites of Donald and Goofy's eyes should not be able to display blood") THE WHITES OF
/// THE EYES WHERE THEY ARE THE HEAD'S OWN SURFACE. Donald's and Goofy's eye slots (`SQEX_IrisTexEyeMaterial`, MASKED) draw only
/// the iris / pupil; the whites are their head's surface, marked by KH3's own colour mask (the head slot's `MaskTexture`):
/// Donald `n_ex001_mat0` / `n_ex001_0m00` blue channel (his material's `MetalMaskChannell`: the glossy eyeball), one island
/// u 0.312..0.437, v 0.201..0.302; Goofy `n_ex002_mat1` / `n_ex002_0m01` green channel, island u 0.394..0.480, v 0.200..0.312
/// (both eyes on one island: their UVs one unit apart in u, or mirrored). Found from that data for any character: the
/// masked spared slots of `mesh` give the iris (rest pose, mirrored onto +x); every other slot of `mesh` with a `MaskTexture`:
/// each mask channel's texel ISLAND (4-connected, > 128) that holds a vertex within 2 cm of the iris across the face, whose
/// vertices all lie within `EYE_WHITES_REACH_CM` of the iris, enclose it across the face and sit behind its front (mean depth
/// at most the iris's + 0.5 cm: not an eyelid in front); of those, the deepest. The island's rectangle in UV and its vertices'
/// rest-pose ball mark the whites (both together, so a weapon's own UVs cannot match). er_char.wgsl kh_blood reads them from
/// `bl_face` = (u0, v0, u1, -(1 + v1)) and `bl_blade_b` = (ball centre, radius) while the block has no blade (`bl_blade_a.w` =
/// 0: a party member's block). Sora's whites are their own spared slots (`SQEX_WhiteEyeMaterial`): none needed.
pub const EYE_WHITES_REACH_CM: f32 = 15.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EyeWhites {
    /// The mask island in UV (u0, v0, u1, v1), sampled with the UV wrapped.
    pub uv: [f32; 4],
    /// The eye whites' vertices in the rest pose (game axes, cm): their ball.
    pub centre: Vec3,
    pub radius: f32,
}

impl EyeWhites {
    /// The two vec4s of the block: `bl_blade_b` (the ball) and `bl_face` (the island, w negative).
    pub fn rows(&self) -> ([f32; 4], [f32; 4]) {
        ([self.centre.x, self.centre.y, self.centre.z, self.radius], [self.uv[0], self.uv[1], self.uv[2], -(1.0 + self.uv[3])])
    }
}

fn rest_cm(v: &crate::model::Vertex) -> Vec3 {
    Vec3::new(v.position[0], v.position[2], v.position[1]) * 100.0
}

/// A UV's texel (wrapped).
fn texel_at(width: u32, height: u32, uv: [f32; 2]) -> usize {
    let x = ((uv[0].rem_euclid(1.0) * width as f32) as u32).min(width - 1);
    let y = ((uv[1].rem_euclid(1.0) * height as f32) as u32).min(height - 1);
    (y * width + x) as usize
}

/// The 4-connected island of texels with channel `c` > 128 holding texel `start`: its texels and its rectangle (x0, y0, x1, y1).
fn island(px: &crate::texture::Pixels, c: usize, start: usize) -> (Vec<usize>, [u32; 4]) {
    let (w, h) = (px.width as usize, px.height as usize);
    let on = |i: usize| px.data[i * 4 + c] > 128;
    let mut seen = std::collections::HashSet::from([start]);
    let mut stack = vec![start];
    let mut rect = [u32::MAX, u32::MAX, 0, 0];
    while let Some(i) = stack.pop() {
        let (x, y) = (i % w, i / w);
        rect = [rect[0].min(x as u32), rect[1].min(y as u32), rect[2].max(x as u32 + 1), rect[3].max(y as u32 + 1)];
        let mut push = |j: usize| {
            if on(j) && seen.insert(j) {
                stack.push(j);
            }
        };
        if x + 1 < w {
            push(i + 1);
        }
        if x > 0 {
            push(i - 1);
        }
        if y + 1 < h {
            push(i + w);
        }
        if y > 0 {
            push(i - w);
        }
    }
    (seen.into_iter().collect(), rect)
}

pub fn eye_whites(model: &crate::model::Model, library: &kh_look::library::Library, mesh: &str) -> Option<EyeWhites> {
    let verts = |p: &crate::model::Primitive| model.vertices.iter().skip(p.base_vertex.max(0) as usize).take(p.vertex_count as usize);
    let slot_of = |p: &crate::model::Primitive| library.slots.iter().find(|s| s.mesh == mesh && s.slot == p.material);
    let mirrored = |v: &crate::model::Vertex| { let r = rest_cm(v); Vec3::new(r.x.abs(), r.y, r.z) };
    // the iris: the masked spared slots' vertices, mirrored onto +x (the two eyes)
    let irises: Vec<Vec3> = model.primitives.iter().filter(|p| slot_of(p).is_some_and(|s| s.masked && spared(s))).flat_map(|p| verts(p).map(mirrored)).collect();
    if irises.is_empty() {
        return None;
    }
    let iris = irises.iter().copied().sum::<Vec3>() / irises.len() as f32;
    let across = |r: Vec3| Vec3::new(r.x - iris.x, 0.0, r.z - iris.z).length();
    let mut best: Option<(f32, EyeWhites)> = None;
    for p in &model.primitives {
        let Some(slot) = slot_of(p).filter(|s| !spared(s)) else { continue };
        let Some(file) = slot.texture_names.iter().position(|n| n == "MaskTexture").and_then(|k| slot.textures.get(k)?.as_ref()).map(|t| library.dir.join(&t.file)) else { continue };
        let Ok(mask) = crate::texture::Pixels::load_png(&file, false) else { continue };
        let at = |v: &crate::model::Vertex| texel_at(mask.width, mask.height, v.uv0);
        for c in 0..4 {
            let mut done: std::collections::HashSet<usize> = std::collections::HashSet::new();
            for start in verts(p).filter(|v| across(mirrored(v)) < 2.0 && mask.data[at(v) * 4 + c] > 128).map(at) {
                if done.contains(&start) {
                    continue;
                }
                let (texels, rect) = island(&mask, c, start);
                let set: std::collections::HashSet<usize> = texels.into_iter().collect();
                done.extend(set.iter().copied());
                let on: Vec<Vec3> = verts(p).filter(|v| set.contains(&at(v))).map(mirrored).collect();
                let (lo, hi) = on.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), r| (lo.min(*r), hi.max(*r)));
                let depth = on.iter().map(|r| r.y).sum::<f32>() / on.len().max(1) as f32;
                let encloses = lo.x <= iris.x && hi.x >= iris.x && lo.z <= iris.z && hi.z >= iris.z;
                if on.is_empty() || !encloses || depth > iris.y + 0.5 || !on.iter().all(|r| (*r - iris).length() <= EYE_WHITES_REACH_CM) {
                    continue;
                }
                if best.as_ref().is_some_and(|(d, _)| *d <= depth) {
                    continue;
                }
                // the ball of its vertices, unmirrored (both eyes)
                let both: Vec<Vec3> = verts(p).filter(|v| set.contains(&at(v))).map(rest_cm).collect();
                let centre = both.iter().copied().sum::<Vec3>() / both.len() as f32;
                let radius = both.iter().map(|r| (*r - centre).length()).fold(0.0, f32::max) + 1.0;
                let (w, h) = (mask.width as f32, mask.height as f32);
                best = Some((depth, EyeWhites { uv: [rect[0] as f32 / w, rect[1] as f32 / h, rect[2] as f32 / w, rect[3] as f32 / h], centre, radius }));
            }
        }
    }
    best.map(|(_, w)| w)
}

/// (ER85) The face's share of the body's amount (a splat there is large against the face: its UV island is dense).
pub const FACE_SHARE: f32 = 0.4;

/// (ER85) How much of the body's blood a slot takes: 0 the eyes and the mouth, `FACE_SHARE` the face (`_F_mat`), 1 the rest.
/// (ER90, ER89 finding 1) only with `blood.face` on: `blood.face = 0` gives every slot 1, the exact old head rule (E6D6:
/// the eyes and the mouth took droplets as the rest of the head).
pub fn share(slot: &kh_look::library::Slot) -> f32 {
    share_with(slot, face())
}

/// (ER90) `share` with the switch given (the tests: no shared switch).
pub fn share_with(slot: &kh_look::library::Slot, face: bool) -> f32 {
    if face && spared(slot) {
        0.0
    } else if face && (slot.instance_name.to_ascii_lowercase().contains("_f_mat") || slot.owner.to_ascii_lowercase().contains("_f_mat")) {
        FACE_SHARE
    } else {
        1.0
    }
}



/// The blood of this frame (any thread; the next drawn frame takes it).
pub fn set(look: BloodLook) {
    let clean = |v: f32| if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 };
    let look = BloodLook { body: clean(look.body), blade: clean(look.blade), fresh: clean(look.fresh), seed: if look.seed.is_finite() { look.seed } else { 0.0 } };
    if let Ok(mut guard) = LOOK.lock() {
        *guard = look;
    }
}

pub fn look() -> BloodLook {
    LOOK.lock().map(|g| *g).unwrap_or_default()
}

/// Whether the renderer may load the `*_bl` set (default yes). Takes effect when the shaders are (re)loaded.
pub fn set_shaders_wanted(on: bool) {
    WANTED.store(on, Ordering::Relaxed);
}

pub(crate) fn shaders_wanted() -> bool {
    WANTED.load(Ordering::Relaxed)
}

pub(crate) fn set_in_use(on: bool) {
    IN_USE.store(on, Ordering::Relaxed);
}

/// The `*_bl` set is drawn with (the blood can show).
pub fn in_use() -> bool {
    IN_USE.load(Ordering::Relaxed)
}

/// The frame block (er_char.wgsl `bl_head`, `bl_feet`, `bl_hand_l`, `bl_hand_r`, `bl_blade_a`, `bl_blade_b`, `bl_face`).
/// `bone`: a bone's place in game space (cm) by name, from this frame's pose.
pub(crate) fn block(input: &FrameInput, bone: impl Fn(&str) -> Option<Vec3>) -> [f32; BLOCK_FLOATS] {
    block_for(look(), input, bone)
}

pub(crate) fn block_for(look: BloodLook, input: &FrameInput, bone: impl Fn(&str) -> Option<Vec3>) -> [f32; BLOCK_FLOATS] {
    let mut out = [0.0f32; BLOCK_FLOATS];
    let Some(body) = input.body.as_ref() else { return out };
    if look.body <= 0.0 && look.blade <= 0.0 {
        return out;
    }
    let mut put = |k: usize, v: Vec3, w: f32| out[4 * k..4 * k + 4].copy_from_slice(&[v.x, v.y, v.z, w]);
    put(0, Vec3::new(look.body, look.blade, look.fresh), look.seed);
    put(1, body.component_to_world.translation, HEIGHT_CM);
    for (k, name) in [(2, "L_te"), (3, "R_te")] {
        if let Some(p) = bone(name).filter(|p| p.is_finite()) {
            put(k, p, HAND_REACH_CM);
        }
    }
    if let Some(weapon) = input.weapon.as_ref().filter(|w| w.opacity > 0.5) {
        if let (Some(a), Some(b)) = (crate::shadowcaps::blade_point(weapon, BLADE_FROM_CM), crate::shadowcaps::blade_point(weapon, BLADE_TO_CM)) {
            put(4, a, BLADE_REACH_CM);
            put(5, b, 1.0);
        }
    }
    if let (Some(neck), Some(head)) = (bone("kubi"), bone("atama")) {
        let centre = head + (head - neck).normalize_or(Vec3::Z) * HEAD_BEYOND_CM;
        if centre.is_finite() && !face() {
            put(6, centre, HEAD_REACH_CM);
        }
    }
    out
}

pub fn status() -> String {
    let l = look();
    format!("blood: body {:.2} blade {:.2} fresh {:.2} seed {:.0}; shaders {}", l.body, l.blade, l.fresh, l.seed, if in_use() { "the *_bl set" } else { "WITHOUT the blood layer (no *_bl set in the pack, or the *_pl set is not in use)" })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Body, Camera, Weapon};
    use kh_anim::Trs;

    fn input(weapon: bool) -> FrameInput {
        let feet = Trs { translation: Vec3::new(100.0, 200.0, 30.0), ..Trs::IDENTITY };
        let camera = Camera { right: Vec3::X, up: Vec3::Y, forward: Vec3::Z, position: Vec3::ZERO, fov: 1.0, aspect: 1.6, near: 0.1, far: 1000.0 };
        FrameInput {
            camera,
            origin: Vec3::ZERO,
            clock: 0.0,
            body: Some(Body { component_to_world: feet, local: Vec::new(), component: Vec::new() }),
            weapon: weapon.then(|| Weapon { to_world: Trs { translation: Vec3::new(120.0, 200.0, 120.0), ..Trs::IDENTITY }, component: Vec::new(), opacity: 1.0 }),
            forms: Vec::new(),
        }
    }

    /// (BLOOD-RED) the shader's blood colour is Elden Ring's (the numbers here), the sheet rows `blood.look.*` carry the same,
    /// and it is red, not brown: green and blue under 1 % of red (before: 15..35 %)
    #[test]
    fn the_blood_is_elden_rings_blood_paint_in_the_shader_and_the_sheet() {
        let wgsl = include_str!("../shaders/er_char.wgsl");
        let number = |name: &str| -> Vec<f32> {
            let line = wgsl.lines().find(|l| l.trim_start().starts_with(&format!("const {name}:"))).unwrap_or_else(|| panic!("{name} not in er_char.wgsl"));
            let value = line.split_once('=').map(|(_, v)| v).unwrap_or_default();
            // (a vector's numbers inside its constructor's parentheses, past `vec3<f32>`)
            let value = value.rsplit_once('(').map_or(value, |(_, v)| v);
            value.split(|c: char| !(c.is_ascii_digit() || c == '.' || c == 'e' || c == '-')).filter_map(|s| s.parse::<f32>().ok()).collect()
        };
        let color = er_blood_color();
        let in_shader = number("BL_ER_COLOR");
        assert_eq!(in_shader.len(), 3, "{in_shader:?}");
        for (a, b) in in_shader.iter().zip(color.to_array()) {
            assert!((a - b).abs() < 1.0e-5, "BL_ER_COLOR {in_shader:?} vs {color:?}");
        }
        assert_eq!(number("BL_ER_TEX_THIN"), vec![ER_BLOOD_TEX_THIN]);
        assert_eq!(number("BL_ER_TEX_THICK"), vec![ER_BLOOD_TEX_THICK]);
        let sheet: serde_json::Value = serde_json::from_str(include_str!("../../../sheets/port/sora-er-host.json")).unwrap();
        let row = |id: &str| sheet["rows"].as_array().unwrap().iter().find(|r| r[0] == id).and_then(|r| r[1].as_f64()).unwrap_or_else(|| panic!("sheet row {id}")) as f32;
        let [r, g, b] = ER_BLOOD_COLOR_SRGB;
        assert_eq!([row("blood.look.color_r"), row("blood.look.color_g"), row("blood.look.color_b")], [f32::from(r), f32::from(g), f32::from(b)]);
        assert_eq!((row("blood.look.tex_thin"), row("blood.look.tex_thick")), (ER_BLOOD_TEX_THIN, ER_BLOOD_TEX_THICK));
        // red: green and blue under 1 % of red at both ends; the thin end the lighter
        for level in [ER_BLOOD_TEX_THIN, ER_BLOOD_TEX_THICK] {
            let c = color * level;
            assert!(c.y < 0.01 * c.x && c.z < 0.01 * c.x, "{c:?}");
        }
        assert!(row("blood.look.tex_thin") > row("blood.look.tex_thick"));
    }

    /// (BLOOD-RED) Donald's eye whites are his head's blue mask island (KH3's n_ex001_0m00), Goofy's his head's green one
    /// (n_ex002_0m01), both eyes in one ball each
    #[test]
    fn donald_and_goofy_eye_whites_are_their_heads_mask_islands() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/party");
        let load = |npc: &str| Some((kh_look::library::Library::load(&root.join(npc)).ok()?, crate::model::Model::load(&root.join(npc).join("model.glb")).ok()?));
        if let Some((library, model)) = load("n_ex001") {
            let w = eye_whites(&model, &library, "n_ex001").expect("Donald: eye whites");
            for (a, b) in w.uv.iter().zip([0.312, 0.201, 0.437, 0.302]) {
                assert!((a - b).abs() < 0.002, "{w:?}");
            }
            // both eyes (x about 0), at his eyes' height (the irises: z 73.9 .. 79.0)
            assert!(w.centre.x.abs() < 0.5 && (70.0..85.0).contains(&w.centre.z) && w.radius < EYE_WHITES_REACH_CM, "{w:?}");
            let (ball, face) = w.rows();
            assert!(face[3] < 0.0 && (-face[3] - 1.0 - w.uv[3]).abs() < 1.0e-6 && ball[3] == w.radius);
        }
        if let Some((library, model)) = load("n_ex002") {
            // Goofy: his head's (n_ex002_mat1) mask green island round his eyes
            let w = eye_whites(&model, &library, "n_ex002").expect("Goofy: eye whites");
            for (a, b) in w.uv.iter().zip([0.394, 0.200, 0.480, 0.312]) {
                assert!((a - b).abs() < 0.002, "{w:?}");
            }
            assert!(w.centre.x.abs() < 0.5 && (145.0..160.0).contains(&w.centre.z) && w.radius < EYE_WHITES_REACH_CM, "{w:?}");
        }
        let wgsl = include_str!("../shaders/er_char.wgsl");
        assert!(wgsl.contains("let v1 = -er.bl_face.w - 1.0;"), "the shader decodes the island as rows() packs it");
    }

    /// (ER85) the eyes and the mouth are spared by name (every outfit's naming), the face is not
    #[test]
    fn the_eyes_and_mouth_are_spared() {
        // (BLOOD-RED) by KH3's base material or KH3's eye / mouth colour sheet, never by the slot's name
        for r in EYE_MOUTH_ROOTS {
            assert!(spared(&slot_of("cloth_name_M_mat0", "x_Inst", r, &[])), "{r}");
        }
        for f in ["textures/p_mi001_M_0d00.png", "textures/p_ts001_E_0d00.png", "textures/p_ca001_M_0d00.png", "textures/Mouth_M_0d00.png", "textures/p_dl001_M_0d00.png"] {
            assert!(eye_mouth_sheet(f) && spared(&slot_of("s", "x_Inst", "SQEX_ToyPC_Basemat", &[f])), "{f}");
        }
        for f in ["textures/p_mi001_0d01.png", "textures/p_ca001_0d03.png", "textures/p_ts001_F_0d00.png", "textures/p_ex001_H_0d00.png", "textures/p_mi001_M_0n00.png", "textures/EyeWhiteBaseColor.png"] {
            assert!(!eye_mouth_sheet(f), "{f}");
        }
        // a cloth slot NAMED like a mouth / eye is not spared (the Caribbean coat's glTF slot is p_ca001_M_mat0)
        assert!(!spared(&slot_of("p_ca001_M_mat0", "p_ca001_coat_Inst", "p_ca001_CoatMat", &["textures/p_ca001_0d01.png"])));
        assert!(!spared(&slot_of("p_mi001_M_mat0_Inst2", "p_mi001_M_mat0_Inst", "p_mi001_mat0", &["textures/p_mi001_0d01.png"])));
    }

    fn slot_of(slot: &str, instance: &str, root: &str, files: &[&str]) -> kh_look::library::Slot {
        kh_look::library::Slot { mesh: "sora".into(), slot: slot.into(), owner: instance.into(), owner_id: 0, instance_name: instance.into(), root: root.into(), masked: false, translucent: false, two_sided: false, subsurface: false, textures: files.iter().map(|f| Some(kh_look::library::TextureRef { file: (*f).into(), srgb: true, mips: false })).collect(), texture_names: Vec::new(), uv2: None, scalars: Default::default(), vectors: Default::default() }
    }

    /// (BLOOD-RED) EVERY PACK'S SPARED SLOTS (the eyes, lashes and mouths, by what their materials are), checked against the
    /// list from KH3's own data (base material / eye-mouth sheet per slot); a pack not built here is skipped
    #[test]
    fn every_packs_spared_slots_are_its_eyes_and_mouth() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack");
        let expected: [(&str, &[&str]); 10] = [
            ("", &["p_ex001_E_mat0", "p_ex001_E_mat1", "p_ex001_E_mat2", "p_ex001_E_mat3", "p_ex001_M_mat0"]),
            ("outfits/caribbean", &["p_ca001_E_mat0", "p_ca001_E_mat1", "p_ca001_E_mat2", "p_ca001_mat3", "p_mi001_furmat0_inst1"]),
            ("outfits/kh1", &["p_ex005_E_mat0", "p_ex005_E_mat1", "p_ex005_E_mat2", "p_ex005_E_mat2@anti", "p_ex005_E_mat3", "p_ex005_E_mat3@anti", "p_ex005_M_mat0", "p_ex005_M_mat0@anti"]),
            ("outfits/kh2", &["p_ex011_E_mat0", "p_ex011_E_mat1", "p_ex011_E_mat2", "p_ex011_E_mat3", "p_ex011_M_mat0"]),
            ("outfits/monstropolis", &["p_mi001_mat2_Inst1_6", "slot1", "slot2"]),
            ("outfits/sanfransokyo", &["p_ex001_E_mat0", "p_ex001_E_mat1", "p_ex001_E_mat2", "p_ex001_E_mat3", "p_ex001_M_mat0"]),
            ("outfits/toybox", &["p_ts001_E_mat0", "p_ts001_M_mat0"]),
            ("outfits/verumrex", &["p_dl001_E_mat0_Inst", "p_dl001_E_mat3_Inst", "p_dl001_F_mat1_Inst", "p_dl001_M_mat0_Inst"]),
            ("party/n_ex001", &["n_ex001_E_mat0"]),
            ("party/n_ex002", &["n_ex002_E_mat0"]),
        ];
        let mut checked = 0;
        for (dir, want) in expected {
            let Ok(library) = kh_look::library::Library::load(&root.join(dir)) else { continue };
            let mut got: Vec<&str> = library.slots.iter().filter(|s| spared(s)).map(|s| s.slot.as_str()).collect();
            got.sort_unstable();
            got.dedup();
            assert_eq!(got, want, "{dir}: the spared slots");
            checked += 1;
        }
        // the two cloth slots with mouth-like glTF names take blood (when their packs are here)
        for (dir, cloth) in [("outfits/caribbean", "p_ca001_M_mat0"), ("outfits/monstropolis", "p_mi001_M_mat0_Inst2")] {
            if let Ok(library) = kh_look::library::Library::load(&root.join(dir)) {
                assert!(library.slots.iter().filter(|s| s.slot == cloth).all(|s| share_with(s, true) == 1.0), "{dir} {cloth}");
            }
        }
        println!("blood: spared slots checked in {checked} packs");
    }

    /// (ER90, ER89 finding 1) `blood.face = 0` = the old head rule: every slot takes the body's blood (eyes and mouth
    /// included); on: eyes / mouth 0, the face FACE_SHARE, the rest 1
    #[test]
    fn face_off_gives_every_slot_the_whole_share() {
        let (eyes, mouth, face, body) = (slot_of("p_ex001_E_mat0", "p_ex001_E_mat0_Inst", "SQEX_WhiteEyeMaterial", &[]), slot_of("p_ex001_M_mat0", "p_ex001_M_mat0_Inst", "SQEX_Mouth", &["textures/Mouth_M_0d00.png"]), slot_of("p_ex001_F_mat0", "p_ex001_F_mat0_Inst", "SQEX_BaseMaterial", &[]), slot_of("p_ex001_mat0", "p_ex001_mat0_Inst", "p_ex001_Basemat", &[]));
        for s in [&eyes, &mouth, &face, &body] {
            assert_eq!(share_with(s, false), 1.0, "{}", s.owner);
        }
        assert_eq!((share_with(&eyes, true), share_with(&mouth, true), share_with(&face, true), share_with(&body, true)), (0.0, 0.0, FACE_SHARE, 1.0));
    }

    #[test]
    fn no_blood_is_an_all_zero_block_and_blood_fills_the_places() {
        let hands = |name: &str| match name {
            "L_te" => Some(Vec3::new(90.0, 180.0, 110.0)),
            "R_te" => Some(Vec3::new(110.0, 220.0, 110.0)),
            _ => None,
        };
        assert_eq!(block_for(BloodLook::default(), &input(true), hands), [0.0; BLOCK_FLOATS]);
        let b = block_for(BloodLook { body: 0.5, blade: 0.8, fresh: 1.0, seed: 3.0 }, &input(true), hands);
        assert_eq!(&b[0..4], &[0.5, 0.8, 1.0, 3.0]);
        assert_eq!(&b[4..8], &[100.0, 200.0, 30.0, HEIGHT_CM]);
        assert_eq!(&b[8..12], &[90.0, 180.0, 110.0, HAND_REACH_CM]);
        assert_eq!(&b[12..16], &[110.0, 220.0, 110.0, HAND_REACH_CM]);
        // the blade: from the shaft's start to the tip along the weapon's +X (no pose: the reference turn about X)
        assert!((Vec3::new(b[16], b[17], b[18]) - Vec3::new(128.0, 200.0, 120.0)).length() < 1e-3 && b[19] == BLADE_REACH_CM);
        assert!((Vec3::new(b[20], b[21], b[22]) - Vec3::new(199.0, 200.0, 120.0)).length() < 1e-3 && b[23] == 1.0);
        // no head bones given: no head rule
        assert_eq!(&b[24..28], &[0.0; 4]);
        let with_head = |name: &str| match name {
            "kubi" => Some(Vec3::new(100.0, 200.0, 150.0)),
            "atama" => Some(Vec3::new(100.0, 200.0, 160.0)),
            _ => None,
        };
        // (ER85) the face takes blood by default: no head rule; `blood.face = 0`: the head rule as before
        let b = block_for(BloodLook { body: 0.5, blade: 0.0, fresh: 1.0, seed: 0.0 }, &input(true), with_head);
        assert_eq!(&b[24..28], &[0.0; 4]);
        set_face(false);
        let b = block_for(BloodLook { body: 0.5, blade: 0.0, fresh: 1.0, seed: 0.0 }, &input(true), with_head);
        set_face(true);
        assert_eq!(&b[24..28], &[100.0, 200.0, 160.0 + HEAD_BEYOND_CM, HEAD_REACH_CM]);
        // no Keyblade out: no blade
        let b = block_for(BloodLook { body: 0.5, blade: 0.8, fresh: 1.0, seed: 3.0 }, &input(false), hands);
        assert_eq!(&b[16..24], &[0.0; 8]);
        assert_eq!(b.len(), BLOCK_FLOATS);
    }
}
