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
    /// 1 = fresh (wet, red), 0 = dried (brown, dull).
    pub fresh: f32,
    /// The splatter pattern's seed (a new one after each wash).
    pub seed: f32,
}

static LOOK: Mutex<BloodLook> = Mutex::new(BloodLook { body: 0.0, blade: 0.0, fresh: 0.0, seed: 0.0 });
/// The host lets the renderer take the `*_bl` set (read when the shaders are loaded).
static WANTED: AtomicBool = AtomicBool::new(true);
/// The `*_bl` set is the one in use.
static IN_USE: AtomicBool = AtomicBool::new(false);

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

/// (ER85) A slot that never takes blood: the eyes (eyeball, iris, lens, wet line) and the mouth's inside (teeth,
/// tongue), by the material instance's / owner's name (KH3's naming: `_E_mat` eyes, `_M_mat` mouth, Eye..., Mouth).
pub fn spared(slot: &kh_look::library::Slot) -> bool {
    spared_name(&slot.instance_name) || spared_name(&slot.owner)
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

pub fn spared_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("_e_mat") || n.contains("_m_mat") || n.contains("eye") || n.contains("mouth") || n.contains("_eyelens")
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

    /// (ER85) the eyes and the mouth are spared by name (every outfit's naming), the face is not
    #[test]
    fn the_eyes_and_mouth_are_spared() {
        for n in ["p_ex001_E_mat0_Inst", "p_ex001_Eyelens_Inst", "p_dl001_E_mat0_Inst", "p_dl001_M_mat0_Inst", "SQEX_DLC_Mouth", "p_mi001_M_mat0_Inst2", "SQEX_EyeLens"] {
            assert!(spared_name(n), "{n}");
        }
        for n in ["p_ex001_F_mat0_Inst", "p_dl001_F_mat0_Inst", "p_dl001_skin_mat0_Inst", "p_ex001_hair_Inst", "p_ex001_mat0_Inst", "p_mi001_mat1_Inst"] {
            assert!(!spared_name(n), "{n}");
        }
    }

    /// (ER90, ER89 finding 1) `blood.face = 0` = the old head rule: every slot takes the body's blood (eyes and mouth
    /// included); on: eyes / mouth 0, the face FACE_SHARE, the rest 1
    #[test]
    fn face_off_gives_every_slot_the_whole_share() {
        let slot = |owner: &str, instance: &str| kh_look::library::Slot { mesh: "sora".into(), slot: "s".into(), owner: owner.into(), owner_id: 0, instance_name: instance.into(), masked: false, translucent: false, two_sided: false, subsurface: false, textures: Vec::new(), texture_names: Vec::new(), uv2: None, scalars: Default::default(), vectors: Default::default() };
        let (eyes, mouth, face, body) = (slot("p_ex001_E_mat0_Inst", "p_ex001_E_mat0_Inst"), slot("p_ex001_M_mat0_Inst", "p_ex001_M_mat0_Inst"), slot("p_ex001_F_mat0_Inst", "p_ex001_F_mat0_Inst"), slot("p_ex001_mat0_Inst", "p_ex001_mat0_Inst"));
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
