//! (ER85, the user's 58.jpg / 59.jpg: "Verum rex and sanfransokyo Sora look FUCKED FIX IT. Verum Rex must look perfect.")
//! THE FIELD LIGHT OF A WORLD OUTFIT'S OWN SET, and the harness's stand-in for the game's light.
//!
//! 1. `set_test_light` / `parse_log_line`: the offline harness hands the renderer a frame of Elden Ring's light as the
//!    live log printed it (the `light:` line of the status), so the field's code path (light.rs `rig`, out_scale, the
//!    exposure, the effect lights) runs offline exactly as in the game. Never set in the DLL.

//! 2. THE FIELD LIGHT OF A WORLD OUTFIT'S OWN SET (`shape`). Found with (1), the 58.jpg light reproduced offline
//!    (build/sora_tmp/er/IMPL_ER85_STATE.md): Elden Ring's overcast day hands him a rig whose undirected part is ~40 % of
//!    the white (key 0.029 against ambient 0.019 per white surface; the test level's rig, which KH3's materials are graded
//!    for, has 12 %), blue-grey, with the sun behind him from the camera. KH3's CG and Real worlds compose the pixel
//!    from that (KHS_CG / KHS_Real: the light buffer, the indirect term times the reflection boards, no colour table):
//!    with the indirect term that large and nothing directional on the face, the skin's base pass (its scatter colour in
//!    the indirect term) reads flat, dark and red, the hair loses its strand light, and a translucent part lit by the
//!    indirect term alone (San Fransokyo's visor, SQEX_VisorGrass: TLM volumetric, no directional term) turns into an
//!    opaque-looking blue over that red skin. KH3 never lights its characters like that: its worlds put a key light on
//!    them and its controllers add camera-relative boards. So for an outfit drawn with its world's own set the game's
//!    light is RESHAPED, its level kept (he still sits in the world's brightness: out_scale is untouched):
//!    - the undirected share is held at `ambient_max` of the white (the rest goes to the key, same white level);
//!    - the key comes from a key position in front of HIM (his eased facing; never the camera: ER86) as far as the sun
//!      lights him from behind (`studio_min` .. `studio_max`), from the sun itself when it lights his front;
//!    - the colours are moved toward neutral by `neutral_key` / `neutral_ambient` (the world's tint kept in part).
//!    kh-er keys: `r.own_field` (1; 0 = the game's light as before), `r.own_field_ambient`, `r.own_field_studio`,
//!    `r.own_field_neutral`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use glam::{Vec3, Vec4};

use crate::light::{GameLight, RigLight};

/// How the game's light is reshaped for an outfit drawn with its world's own set (module doc, 2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FieldShape {
    pub on: bool,
    /// The most of the white the undirected light may be (the test level's rig: 0.12).
    pub ambient_max: f32,
    /// How far the key moves to the key position in front of him: with the sun in front of him / behind him.
    pub studio_min: f32,
    pub studio_max: f32,
    /// How far the key's and the undirected light's colours move toward neutral (their own luminance).
    pub neutral_key: f32,
    pub neutral_ambient: f32,
    /// The key position in front of him: degrees to the side of his facing (toward the sun) and above the horizon.
    pub side_deg: f32,
    pub up_deg: f32,
    /// His brightness against the world's (out_scale times this): 1 = a white surface of his shows what the ground shows.
    pub lift: f32,
    /// (with `lift` > 1) the key's share of the white at or below which the whole lift applies; none from 0.3 above it.
    /// The share is the WORLD's (the sun against the sky and the ground's light: an overcast day low, a clear one high), never
    /// the camera's or the exposure's (ER86). 0 = always `lift`.
    pub lift_share: f32,
    /// The least lift whatever the world's share (1 = none): his brightness against the world's in a clear day.
    pub lift_min: f32,
}

impl Default for FieldShape {
    fn default() -> Self {
        FieldShape { on: true, ambient_max: 0.2, studio_min: 0.25, studio_max: 0.85, neutral_key: 0.6, neutral_ambient: 0.5, side_deg: 35.0, up_deg: 40.0, lift: 1.0, lift_share: 0.0, lift_min: 1.0 }
    }
}

static SHAPE: Mutex<Option<FieldShape>> = Mutex::new(None);
/// The drawn outfit is drawn with its own set (ownset.rs `apply`).
static ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn set_shape(s: FieldShape) {
    if let Ok(mut x) = SHAPE.lock() {
        *x = Some(s);
    }
}

pub fn shape_settings() -> FieldShape {
    SHAPE.lock().ok().and_then(|x| *x).unwrap_or_default()
}

pub fn set_active(on: bool) {
    ACTIVE.store(on, Ordering::Relaxed);
}

pub fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

/// 3. AN OUTFIT'S OWN LOOK FILE (`PACK/outfits/<id>/materials/own_look.json`, NEW, beside its own set): material
/// parameter values written over the instances' (by instance name), and the field light's settings for that outfit.
/// Values a pass chose against a reference (the official render), NOT game data: each file says so per value. Applied
/// only while the outfit is drawn with its own set and `boc.<id>.own_look` is on (absent = on).
pub const LOOK_FILE: &str = "own_look.json";
static LOOK_OFF: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// The field light of the outfit drawn (its own look file's `field`, else the defaults).
static LOOK_FIELD: Mutex<Option<FieldShape>> = Mutex::new(None);

/// The host's keys: the outfits whose own look file is not wanted (`*` = all).
pub fn set_look_off(off: Vec<String>) {
    if let Ok(mut o) = LOOK_OFF.lock() {
        *o = off;
    }
}

pub fn look_wanted(outfit: &str) -> bool {
    LOOK_OFF.lock().map(|o| !o.iter().any(|x| x == "*" || x == outfit)).unwrap_or(true)
}

/// The field light for this frame: the host's settings (`set_shape`), with the drawn outfit's own values over them.
pub fn shape_now() -> FieldShape {
    let host = shape_settings();
    match LOOK_FIELD.lock().ok().and_then(|f| *f) {
        Some(own) => FieldShape { on: host.on && own.on, lift: own.lift * host.lift, lift_min: own.lift_min * host.lift, ..own },
        None => host,
    }
}

fn field_of(v: &serde_json::Value, base: FieldShape) -> FieldShape {
    let f = |k: &str, d: f32| v.get(k).and_then(|x| x.as_f64()).map_or(d, |x| x as f32);
    FieldShape {
        on: v.get("on").and_then(|x| x.as_bool()).unwrap_or(base.on),
        ambient_max: f("ambient_max", base.ambient_max),
        studio_min: f("studio_min", base.studio_min),
        studio_max: f("studio_max", base.studio_max),
        neutral_key: f("neutral_key", base.neutral_key),
        neutral_ambient: f("neutral_ambient", base.neutral_ambient),
        side_deg: f("side_deg", base.side_deg),
        up_deg: f("up_deg", base.up_deg),
        lift: f("lift", base.lift),
        lift_share: f("lift_share", base.lift_share),
        lift_min: f("lift_min", base.lift_min),
    }
}

/// Reads `look/materials/own_look.json` for an outfit drawn with its own set (ownset.rs) and writes its material values
/// over the library's slots. Returns how many values were written. Without the file, or with the key off: nothing.
pub fn apply_look(look: &std::path::Path, outfit: &str, library: &mut kh_look::library::Library) -> usize {
    if let Ok(mut f) = LOOK_FIELD.lock() {
        *f = None;
    }
    let file = look.join("materials").join(LOOK_FILE);
    let Ok(text) = std::fs::read_to_string(&file) else { return 0 };
    if !look_wanted(outfit) {
        crate::rlog!("render: (ER85) {outfit}'s own look file switched off (boc.{outfit}.own_look = 0)");
        return 0;
    }
    let json: serde_json::Value = match serde_json::from_str(&text) {
        Ok(j) => j,
        Err(e) => {
            crate::rlog!("render: (ER85) {}: {e}: not used", file.display());
            return 0;
        }
    };
    let mut written = 0;
    for (instance, values) in json.get("materials").and_then(|m| m.as_object()).into_iter().flatten() {
        for slot in library.slots.iter_mut().filter(|s| s.instance_name == *instance || s.slot == *instance) {
            for (name, value) in values.as_object().into_iter().flatten() {
                let value = value.get("value").unwrap_or(value);
                if let Some(x) = value.as_f64() {
                    slot.scalars.insert(name.clone(), x as f32);
                    written += 1;
                } else if let Some(a) = value.as_array().filter(|a| a.len() == 4) {
                    let v = [0, 1, 2, 3].map(|k| a[k].as_f64().unwrap_or(0.0) as f32);
                    slot.vectors.insert(name.clone(), v);
                    written += 1;
                }
            }
        }
    }
    if let Some(field) = json.get("field") {
        if let Ok(mut f) = LOOK_FIELD.lock() {
            *f = Some(field_of(field, FieldShape::default()));
        }
    }
    crate::rlog!("render: (ER85) {outfit}'s own look ({}): {written} material value(s){}", file.display(), if json.get("field").is_some() { ", its field light" } else { "" });
    written
}

/// The lift of this frame: `lift`, or with `lift_share` by the key's share of the white of the light as the world gives it.
/// `sun_lit`: how much of the sun reaches his feet (the steady probes' shadow mask, 1 = in the open): the share is taken
/// of the OPEN sky's light, so a tree's shadow does not lift him above the ground beside him.
pub fn lift_for(r: &RigLight, sun_lit: f32, s: &FieldShape) -> f32 {
    let least = s.lift_min.clamp(0.25, 4.0);
    let most = s.lift.clamp(0.25, 4.0).max(least);
    if s.lift_share <= 0.0 || most <= least {
        return most;
    }
    let key = luma(r.key) / std::f32::consts::PI / sun_lit.clamp(0.05, 1.0);
    let white = key + luma(r.ambient + r.ambient_up * 0.5);
    if !(white > 1.0e-9) {
        return most;
    }
    let share = key / white;
    most + (least - most) * smoothstep(s.lift_share, s.lift_share + 0.3, share)
}

fn luma(c: Vec3) -> f32 {
    c.dot(Vec3::new(0.2126, 0.7152, 0.0722))
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The game's light for this frame as an outfit with its world's own set gets it (module doc, 2): `r` the rig of light.rs
/// (key = colour x pi as a white surface facing it shows, ambient, ambient_up; directions in HOST space, Y up),
/// `front`: HIS eased facing, level (host space; ZERO = none: the key keeps the sun's direction). Nothing here depends on the
/// camera (ER86's rule, the user: "his lighting changes when the camera moves"). The white level and `out_scale` are kept.
pub fn shape(r: RigLight, front: Vec3, sun_lit: f32, s: &FieldShape) -> RigLight {
    let lift = lift_for(&r, sun_lit, s);
    shape_lifted(r, front, lift, s)
}

/// `shape` with the lift given (eased by the caller: `LiftEase`).
pub fn shape_lifted(r: RigLight, front: Vec3, lift: f32, s: &FieldShape) -> RigLight {
    if !s.on {
        return r;
    }
    let pi = std::f32::consts::PI;
    let mut out = r;
    out.out_scale = r.out_scale * lift;
    // 1. the undirected share held at ambient_max, the rest into the key (same white)
    let undirected = luma(r.ambient + r.ambient_up * 0.5);
    let key_white = luma(r.key) / pi;
    let white = (undirected + key_white).max(1.0e-6);
    let keep = if undirected / white > s.ambient_max { s.ambient_max * white / undirected.max(1.0e-9) } else { 1.0 };
    let moved = undirected * (1.0 - keep);
    let neutral = |c: Vec3, k: f32| c.lerp(Vec3::splat(luma(c)), k.clamp(0.0, 1.0));
    let mut key = r.key;
    if moved > 0.0 {
        if key_white > 1.0e-6 {
            key *= 1.0 + moved / key_white;
        } else {
            // (no directional light: a cave, an interior) the key takes the undirected light's colour
            let c = r.ambient + r.ambient_up * 0.5;
            key = c / luma(c).max(1.0e-9) * moved * pi;
        }
    }
    out.ambient = neutral(r.ambient * keep, s.neutral_ambient);
    out.ambient_up = neutral(r.ambient_up * keep, s.neutral_ambient);
    // (the white is kept through the neutralisation: luma is unchanged by it)
    out.key = neutral(key, s.neutral_key);
    // 2. the key's direction: toward a key position in front of HIM (his eased facing) as far as the sun lights him from behind
    let up = Vec3::Y;
    let ahead = Vec3::new(front.x, 0.0, front.z).normalize_or_zero();
    if ahead != Vec3::ZERO {
        let sun = r.key_dir.normalize_or_zero();
        let sun_h = Vec3::new(sun.x, 0.0, sun.z).normalize_or_zero();
        let side = up.cross(ahead).normalize_or_zero();
        // (to the sun's side of his front, continuously: no switch of sides as he turns through the sun's line)
        let toward = (sun_h.dot(side) * 3.0).clamp(-1.0, 1.0);
        let (sa, ca) = s.side_deg.to_radians().sin_cos();
        let (su, cu) = s.up_deg.to_radians().sin_cos();
        let studio = ((ahead * ca + side * toward * sa) * cu + up * su).normalize_or_zero();
        // d: 1 = the sun in front of him (it lights his front), -1 = behind him; a sun low or below the horizon: behind
        let d = if sun.y < 0.05 { -1.0 } else { sun_h.dot(ahead) };
        let w = s.studio_max + (s.studio_min - s.studio_max) * smoothstep(-0.5, 0.5, d);
        let dir = sun.lerp(studio, w.clamp(0.0, 1.0)).normalize_or_zero();
        if dir != Vec3::ZERO {
            out.key_dir = dir;
        }
    }
    out
}

/// (ER85) THE LIFT, EASED: it follows the world's light share over `LIFT_TAU_S`, so the probes' own settling (ER86's tap
/// memory filling as the camera first turns) does not show in his brightness.
pub const LIFT_TAU_S: f32 = 2.0;

#[derive(Clone, Copy, Debug, Default)]
pub struct LiftEase(pub Option<f32>);

impl LiftEase {
    pub fn step(&mut self, target: f32, dt: f32) -> f32 {
        let next = match self.0 {
            Some(before) => before + (target - before) * crate::stable::blend(dt, LIFT_TAU_S),
            None => target,
        };
        self.0 = Some(next);
        next
    }
}

static LIFT: Mutex<(LiftEase, Option<std::time::Instant>)> = Mutex::new((LiftEase(None), None));

/// His facing, eased with ER86's facing time (stable.rs `hero.anchor_tau_s`), level, HOST space.
static FACING: Mutex<Option<(Vec3, std::time::Instant)>> = Mutex::new(None);

/// (gfx.rs) `shape` with his facing: `facing` his forward axis in GAME space (the component's +Y), eased here.
pub fn shape_for_him(r: RigLight, facing: Option<Vec3>, sun_lit: f32, s: &FieldShape) -> RigLight {
    let now = facing.map(|g| Vec3::new(g.y, 0.0, g.x)).and_then(|h| h.try_normalize());
    let eased = match FACING.lock() {
        Ok(mut st) => {
            let tau = crate::stable::settings().tau_facing.max(0.0);
            let next = match (*st, now) {
                (Some((before, at)), Some(n)) => {
                    let dt = at.elapsed().as_secs_f32().clamp(0.0, 0.25);
                    before.lerp(n, crate::stable::blend(dt, tau)).try_normalize().unwrap_or(n)
                }
                (None, Some(n)) => n,
                (Some((before, _)), None) => before,
                (None, None) => Vec3::ZERO,
            };
            *st = (next != Vec3::ZERO).then(|| (next, std::time::Instant::now()));
            next
        }
        Err(_) => now.unwrap_or(Vec3::ZERO),
    };
    let target = lift_for(&r, sun_lit, s);
    let lift = match LIFT.lock() {
        Ok(mut st) => {
            let dt = st.1.map_or(0.0, |at| at.elapsed().as_secs_f32().clamp(0.0, 0.25));
            st.1 = Some(std::time::Instant::now());
            st.0.step(target, dt)
        }
        Err(_) => target,
    };
    shape_lifted(r, eased, lift, s)
}

/// (HY) The lift of the latest `shape_for_him` while an own set is drawn (1 otherwise): the hybrid lifts the game's light on
/// him by it as the rig's (gfx.rs: the rig's out scale carries it, the hybrid's scene -> KH factor takes it out again).
pub fn lift_now() -> f32 {
    if !active() {
        return 1.0;
    }
    LIFT.lock().ok().and_then(|st| st.0 .0).filter(|l| l.is_finite() && *l > 0.0).unwrap_or(1.0)
}

static TEST_LIGHT: Mutex<Option<GameLight>> = Mutex::new(None);

/// (harness only) The game's light the renderer uses in place of the read-back one; `None` = the game's own.
pub fn set_test_light(light: Option<GameLight>) {
    if let Ok(mut t) = TEST_LIGHT.lock() {
        *t = light;
    }
}

pub fn test_light() -> Option<GameLight> {
    TEST_LIGHT.lock().ok().and_then(|t| *t)
}

/// The numbers after `key` up to the next `)` or `]` (the log prints `(a, b, c)` and `[a, b, c]`).
fn numbers_after(line: &str, key: &str) -> Option<Vec<f32>> {
    let at = line.find(key)? + key.len();
    let rest = &line[at..];
    let open = rest.find(['(', '['])?;
    let close = rest[open..].find([')', ']'])? + open;
    let v: Vec<f32> = rest[open + 1..close].split(',').filter_map(|x| x.trim().parse().ok()).collect();
    (!v.is_empty()).then_some(v)
}

fn vec3(v: &[f32]) -> Option<Vec3> {
    (v.len() >= 3).then(|| Vec3::new(v[0], v[1], v[2]))
}

/// A frame of the game's light from the status line the DLL logs (gfx.rs: `light: sun dir (..) colour (..) sky [..] | at
/// his feet: scene [..] direct [..] surface [..] mask [..] .. exposure E pre P .. | fog ambient [..] toward the light [..]
/// opposite [..]`). The sky light's direction is straight up (the game's Vec_1); the shadow colour is not logged: 1.
pub fn parse_log_line(line: &str) -> Option<GameLight> {
    let mut l = GameLight { shadow_color: Vec3::ONE, scene: true, ..GameLight::default() };
    if let (Some(d), Some(c)) = (numbers_after(line, "sun dir").as_deref().and_then(vec3), line.find("sun dir").and_then(|k| numbers_after(&line[k..], "colour")).as_deref().and_then(vec3)) {
        l.sun = Some((d.normalize_or_zero(), c));
    }
    if let Some(s) = numbers_after(line, " sky ").as_deref().and_then(vec3) {
        l.sky = (Vec3::Y, s);
    }
    l.scene_px = numbers_after(line, "at his feet: scene").as_deref().and_then(vec3)?;
    l.direct_px = numbers_after(line, "direct").as_deref().and_then(vec3)?;
    l.albedo_px = numbers_after(line, "surface").as_deref().and_then(vec3)?;
    let m = numbers_after(line, "mask")?;
    l.mask = Vec4::new(m[0], *m.get(1).unwrap_or(&1.0), *m.get(2).unwrap_or(&1.0), *m.get(3).unwrap_or(&1.0));
    l.taps = 9.0;
    let after = |key: &str| line.find(key).map(|k| line[k + key.len()..].trim_start().split([' ', '|']).next().unwrap_or("").parse::<f32>().ok()).flatten();
    l.exposure = after("exposure")?;
    l.pre_exposure = after(" pre").unwrap_or(1.0);
    if let Some(e) = numbers_after(line, "top of the picture").as_deref().and_then(vec3) {
        (l.env_px, l.env_taps) = (e, 1.0);
    }
    for (k, key) in ["fog ambient", "toward the light", "opposite"].iter().enumerate() {
        if let Some(f) = numbers_after(line, key).as_deref().and_then(vec3) {
            l.fog[k] = f;
        }
    }
    Some(l)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The live log's line of the 58.jpg session (Limgrave, rain, 09:54) parses into the frame the rig was made from.
    #[test]
    fn the_logged_light_parses() {
        let line = "  light: sun dir (0.550, 0.697, 0.461) colour (0.1557, 0.1699, 0.2000) sky [0.0, 0.0, 0.0] | at his feet: scene [0.0069, 0.0071, 0.0044] direct [0.0047, 0.0049, 0.0021] surface [0.1209, 0.1254, 0.0476] mask [0.66, 1.0, 1.0, 1.0] probed true | scene true exposure 10.311 pre 1.00 jitter (-0.375, 0.056) px | he gets (white surface): key [0.0287, 0.0313, 0.0369] ambient [0.019, 0.0169, 0.0187] + up [0.0, 0.0, 0.0] (rig x67.2; ground / formula this frame 1.046) | top of the picture [0.0182, 0.0173, 0.0145] (1 taps): reflection environment x3.87 | fog ambient [0.0064, 0.0097, 0.0108] toward the light [0.3018, 0.3641, 0.3742] opposite [0.0317, 0.0535, 0.0597]";
        let l = parse_log_line(line).expect("parses");
        let (d, c) = l.sun.unwrap();
        assert!((d.y - 0.697 / Vec3::new(0.55, 0.697, 0.461).length()).abs() < 1e-3 && (c.z - 0.2).abs() < 1e-6);
        assert!((l.exposure - 10.311).abs() < 1e-4 && l.pre_exposure == 1.0);
        assert!((l.direct_px.x - 0.0047).abs() < 1e-6 && (l.albedo_px.z - 0.0476).abs() < 1e-6 && l.mask.x == 0.66);
        assert!((l.fog[1].y - 0.3641).abs() < 1e-6 && (l.env_px.x - 0.0182).abs() < 1e-6);
        assert!(l.probed());
        // the rig of light.rs from it: what the log said he got (rig x67.2)
        let r = crate::light::rig(&l, Vec3::Y, crate::light::CALIBRATION_DEFAULT, 1.0);
        assert!((1.0 / r.out_scale - 67.2).abs() < 0.5, "{}", 1.0 / r.out_scale);
    }

    /// The staged Verum Rex look file (er85/pack) writes its values over the outfit's eye and hair instances; off by key:
    /// nothing. Skipped without the files.
    #[test]
    fn the_verum_rex_look_file_applies() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        let staged = root.join("build/sora_tmp/er/er85/pack/outfits/verumrex");
        let Ok(mut lib) = kh_look::library::Library::load(&root.join("build/sora_pack/outfits/verumrex")) else { return };
        if !staged.join("materials").join(LOOK_FILE).is_file() {
            return;
        }
        let eye = |lib: &kh_look::library::Library| lib.slots.iter().find(|s| s.instance_name == "p_dl001_E_mat0_Inst").and_then(|s| s.scalars.get("EyelsisSSCMulti").copied());
        let before = eye(&lib);
        set_look_off(vec!["verumrex".into()]);
        assert_eq!(apply_look(&staged, "verumrex", &mut lib), 0);
        assert_eq!(eye(&lib), before);
        set_look_off(Vec::new());
        assert!(apply_look(&staged, "verumrex", &mut lib) >= 7);
        assert_eq!(eye(&lib), Some(25.0));
        let f = LOOK_FIELD.lock().unwrap().unwrap();
        assert!((f.lift - 1.9).abs() < 1e-6 && (f.lift_share - 0.6).abs() < 1e-6 && (f.lift_min - 1.4).abs() < 1e-6);
    }

    /// The reshaped light keeps the white and the level, holds the undirected share, and turns a key from behind him toward
    /// the camera; with the sun behind the camera it barely moves; off: the light as given.
    #[test]
    fn the_shape_keeps_the_white_and_turns_a_back_light() {
        let line = "light: sun dir (0.550, 0.697, 0.461) colour (0.1557, 0.1699, 0.2000) sky [0.0, 0.0, 0.0] | at his feet: scene [0.0069, 0.0071, 0.0044] direct [0.0047, 0.0049, 0.0021] surface [0.1209, 0.1254, 0.0476] mask [0.66, 1.0, 1.0, 1.0] probed true | scene true exposure 10.311 pre 1.00";
        let l = parse_log_line(line).unwrap();
        let r = crate::light::rig(&l, Vec3::Y, crate::light::CALIBRATION_DEFAULT, 1.0);
        let s = FieldShape::default();
        let white = |x: &RigLight| luma(x.key) / std::f32::consts::PI + luma(x.ambient + x.ambient_up * 0.5);
        // he faces away from the sun: the sun is behind HIM
        let fwd = Vec3::new(0.550, 0.0, 0.461).normalize();
        let out = shape(r, -fwd, 1.0, &s);
        assert!((white(&out) - white(&r)).abs() < 1e-3 * white(&r), "{} {}", white(&out), white(&r));
        assert_eq!(out.out_scale, r.out_scale);
        assert!((shape(r, -fwd, 1.0, &FieldShape { lift: 1.5, ..s }).out_scale - 1.5 * r.out_scale).abs() < 1e-9);
        let share = luma(out.ambient + out.ambient_up * 0.5) / white(&out);
        assert!(share <= s.ambient_max + 1e-4 && luma(r.ambient) / white(&r) > 0.3, "{share}");
        // the key now lights his front
        assert!(out.key_dir.dot(-fwd) > 0.3 && r.key_dir.dot(-fwd) < 0.0, "{:?}", out.key_dir);
        // he faces the sun: the key stays near the sun
        let back = shape(r, fwd, 1.0, &s);
        assert!(back.key_dir.dot(r.key_dir) > 0.95);
        // off: as given
        assert_eq!(shape(r, -fwd, 1.0, &FieldShape { on: false, ..s }), r);
        // nothing of the camera is an input: the same facing gives the same light; turning him by a degree moves the key by
        // about as much (no switch of sides through the sun's line)
        let turn = |deg: f32| { let (s1, c1) = deg.to_radians().sin_cos(); Vec3::new(-fwd.x * c1 + fwd.z * s1, 0.0, -fwd.z * c1 - fwd.x * s1) };
        let a = shape(r, turn(-0.5), 1.0, &s).key_dir;
        let b = shape(r, turn(0.5), 1.0, &s).key_dir;
        assert!(a.dot(b) > 0.999, "{a:?} {b:?}");
        // the lift by the world's key share: 58's overcast light (the key ~60 % of the white) takes nearly all of it; a light
        // that is 90 % key (a clear day) none
        let t = FieldShape { lift: 1.8, lift_share: 0.6, ..s };
        let l = lift_for(&r, 1.0, &t);
        assert!(l > 1.6 && l <= 1.8, "{l}");
        let clear = RigLight { key: r.key * 9.0, ..r };
        assert_eq!(lift_for(&clear, 1.0, &t), 1.0);
        // in a shadow (half the sun) the share is the open sky's: the same lift
        let shaded = RigLight { key: r.key * 0.5, ..r };
        assert!((lift_for(&shaded, 0.5, &t) - l).abs() < 0.05);
    }
}
