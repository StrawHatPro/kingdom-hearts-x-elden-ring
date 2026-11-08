//! (F-RAGE) RAGE FORM'S LOOK ON EVERY OUTFIT (the user 2026-11-03: "make sure KH1 and KH2 Sora can look like rage form as
//! well"; documentation/KH3_WEAPON_ASSETS.md 6b).
//!
//! HOW KH3 DRAWS IT [A, the translated shaders, PACK/materials/materials.wgsl KH_C8 lines ~5470..5600 and ~5726..5740]: wear
//! form 7 keeps the default costume; every material of Sora (and of his weapons) runs its ANTI MODE (static switch UseAntiMode,
//! compiled into all twelve of the main pack's owners) driven by ONE run-time scalar, `AntiVolume`:
//! 1. darkness: `lum = dot(albedo, (0.3, 0.59, 0.11))`, `albedo = lerp(albedo, lum * AntiDarknessColorMulti (0.04, 0.5, 1),
//!    grade)`, `grade` a front from the head point (atama + AntiVolumePositionOffsetUD 5 / MU -10) over AntiVolumeGradeLength
//!    170 cm ([`grade`]);
//! 2. the face: `lum` pulled to 0.008 (UseAntiFaceDark); 3. the eye lens: AntiEyeEmissiv_L / _H, red / orange;
//! 4. the pattern: AntiPointMaskTex's alpha at the second UV set in tile (1, 0), emitted as `AntiMaskEmissiv (10, 15, 30) *
//!    x (1 - x)` with `x = fract(time * AntiMaskEmissivSpeed (0.2) + 1 - |p - head| / 170)` (a pulse that runs down from the head).
//!
//! THE HSV STEP after step 1 (the open item of 6b) is NOT part of Rage: lines 5599..5635 convert to HSV, keep H and S, take
//! `V * (0.8 - 0.3 S)` and blend by a weight made of Wetness / Wetness_CS / WetnessLength / InWater (rows m[96], m[97], the feet's
//! L_ashi1 / R_ashi1): KH3's WET LOOK. With Wetness 0 (Rage Form sets none) it does nothing, so the dark-blue shade is step 1 alone.
//!
//! THE SETTER [A, decompiled C]: UTresEquipmentComponent 0x14177A820 writes the pawn's `AntiVolume` = clamp(clamp(1 - a) * v)
//! (FName at 0x149A99050) and, for each of m_WeaponEquips, 0x14178A2E0 -> 0x14178A380 the weapon's (FName 0x149A99090): the
//! KEYBLADE darkens too; 0x141790A50 is its timed variant (the weapon's start / target / duration at +0x6B4 / 0x6B8 / 0x6C0).
//! (F-POLISH) THE FADE TIME, traced: the equipment's fade 0x1417915A0 (equip vt+0x5F8(wear kind, time): target 1.0 for kind
//! 7, else 0; time <= 0 sets at once, else a linear ramp element ticked by 0x141790C80) is reached through pawn vt+0x1498 /
//! vt+0x13E0. In: 1.0 s (StyleChange 0x1413CBA70, its tick 0x1413D3AC0, 0x1413CE760: [0x145C87D0C] = 1.0). Out: 0 s
//! (EndStyle 0x14141CA20 loads 0.2 at 0x14141CAED; case 0x8B zeroes it, `xorps xmm8, xmm8` at 0x14141CD44).
//!
//! WHAT THIS MODULE DOES (keyed on lib.rs `wear_form() == 7`, its own fade clock; nothing for the host to add):
//! - every slot that carries KH3's anti mode gets `AntiVolume` (KH3 Sora's twelve, the Keyblade, the outfits' eyes, KH2's
//!   mouth and hair): KH3's own code does the rest;
//! - an outfit slot whose base colour wear form 7 bakes (wear_extra.json `direct["7"]`, tools/lib/rage_bake.js: step 1 at
//!   AntiVolume 1, the pattern in the AntiMaskEmissiv hue) keeps `AntiVolume` 0 (no second darkening); its baked texture goes
//!   on when the fade passes [`SNAP`] (the form handed to kh-look `step_wear` waits: [`Rage::library_form`]) and its
//!   `<name>_kbf7_glow.png` (red = the pattern) is bound as material texture 13 at load ([`prepare`]) and EMITTED by
//!   er_char.wgsl `er_rage_glow` (rows [`ROW_HEAD`] / [`ROW_RAGE`] of the material block, [`rows`]) with KH3's formula above;
//! - a hair slot WITHOUT the anti mode (KH1's SQEX_KHHairBaseMat) gets its HairColor / HairPointEndColor through step 1
//!   ([`darken`]) by the fade;
//! - every slot whose anti mode does not run (baked, or none) gets KH3's AntiDarknesAOMax on its own occlusion selector
//!   `AOChannel` ([`ao_factor`]): most of Rage Form's darkness is that occlusion, not step 1.
//!
//! The aura (ead_p_ant_com) is the gameplay side's (named effects).

use std::path::Path;
use std::sync::Mutex;

use glam::Vec4;
use kh_look::library::{Library, Slot, TextureRef};
use serde_json::Value;

/// The wear form of Rage Form (KH3's "Anti", WearformSet m_Forms[7]).
pub const RAGE_WEAR_FORM: u8 = 7;
/// The fade's lengths (s), traced (module doc; F-POLISH: out is Rage's 0 s, not the other forms' 0.2).
pub const IN_SECONDS: f32 = kh_look::library::WEAR_FORM_ON_SECONDS;
pub const OUT_SECONDS: f32 = kh_look::library::WEAR_FORM_RAGE_OFF_SECONDS;
/// The fade at which an outfit's baked Rage textures go on / come off. (AUDIT-C) 0.6, was 0.5: KH3's grade ([`grade`]) is 1 from
/// the head down to 170 cm (the feet) from AntiVolume 0.581 on ((2v - 1)(2v + 5) = 1), so the bake goes on where KH3's front
/// has covered the body; below it er_char.wgsl `er_rage_glow` (tag [`TAG`] 7778) gives the base colour the bake's own step by
/// that grade, so the darkness SPREADS FROM THE HEAD as KH3's anti mode does instead of switching at once.
pub const SNAP: f32 = 0.6;
/// KH3's step 1: the luminance weights and AntiDarknessColorMulti (every owner's default).
pub const LUMA: [f32; 3] = [0.3, 0.59, 0.11];
pub const DARKNESS: [f32; 3] = [0.04, 0.5, 1.0];
/// KH3 Sora's cloth (p_ex001_mat0 / mat1 instances): AntiMaskEmissiv and AntiMaskEmissivSpeed.
pub const MASK_EMISSIVE: [f32; 3] = [10.0, 15.0, 30.0];
pub const MASK_SPEED: f32 = 0.2;
/// The grade front: AntiVolumeGradeLength (cm), AntiVolumePositionOffsetUD / MU (cm along atama_mtr_x / atama_mtr_y).
pub const GRADE_LENGTH: f32 = 170.0;
pub const OFFSET_UD: f32 = 5.0;
pub const OFFSET_MU: f32 = -10.0;
/// The material block's rows the glow reads (free in every owner: the largest constant buffer is 139 rows of 160), and the
/// tag in [`ROW_RAGE`].w that tells er_char.wgsl `er_rage_glow` they are ours.
pub const ROW_HEAD: usize = 158;
pub const ROW_RAGE: usize = 159;
/// (AUDIT-C) 7778 (was 7777): [`ROW_HEAD`].w carries the slot's face darkness ([`FACE_DARK`]) and the shader runs the spreading
/// front below [`SNAP`]; a shader of before knows only 7777 and draws nothing of ours (the release rebuilds every shader set).
pub const TAG: f32 = 7778.0;
/// (AUDIT-C) The face darkness the bake gives a face texture (tools/sora_outfit_formcolours.js FACE_DARK, the slots `*_F_mat*`),
/// for the shader's step before the bake is bound.
pub const FACE_DARK: f32 = 0.85;
/// The material texture slot a glow mask is bound to (unused by every outfit owner: they bind at most 11).
pub const GLOW_TEXTURE: usize = 13;
/// Our own keys in a slot's parameter maps (no material reads them).
const KEY_BAKED: &str = "(F-RAGE) baked";
const KEY_GLOW: &str = "(F-RAGE) glow";
const KEY_HAIR: &str = "(F-RAGE) hair";
const KEY_AO: &str = "(F-RAGE) AOChannel";
const KEY_FACE: &str = "(AUDIT-C) face";
/// The hair shader's colour parameters (it has no base colour texture): its colour, the strand ends', its subsurface tint and the
/// roots' (KH3's anti mode darkens the hair's final colour, so every term of it goes through step 1).
const HAIR_COLOURS: [&str; 4] = ["HairColor", "HairPointEndColor", "HairSSColor", "HairStartColor"];

/// KH3's step 1 on a linear colour by weight `w` (0 = as is, 1 = the dark-blue monochrome).
pub fn darken(c: [f32; 3], w: f32) -> [f32; 3] {
    let lum = c[0] * LUMA[0] + c[1] * LUMA[1] + c[2] * LUMA[2];
    let w = w.clamp(0.0, 1.0);
    [0, 1, 2].map(|k| c[k] + (lum * DARKNESS[k] - c[k]) * w)
}

/// AntiDarknesAOMax (every owner's default): KH3's anti mode takes the light's occlusion to AO x 0.4 (KH_C8 lines ~5442 /
/// 5486: `lerp(AO, AO * AntiDarknesAOMax, grade)`, the pattern's texels excepted). It is most of Rage Form's darkness: KH3
/// Sora's forearm at AntiVolume 1 shows (0, 12, 42) with it and (0, 64, 128) with AntiDarknesAOMax 1 (offline, same frame).
pub const AO_MAX: f32 = 0.4;
/// The occlusion's factor at `volume`: 1 .. AO_MAX. On a slot without the anti mode it goes onto the material's own
/// occlusion channel selector `AOChannel` (AO = dot(AOTexture, AOChannel) in the SQEX base materials): the same AO x 0.4.
pub fn ao_factor(volume: f32) -> f32 {
    1.0 + (AO_MAX - 1.0) * volume.clamp(0.0, 1.0)
}
const AO_CHANNEL: &str = "AOChannel";
/// (AUDIT-C, was our calibrated 0.3) The extra factor er_char.wgsl `er_rage_glow` puts on a baked (cloth) slot: 1, i.e.
/// none. KH3's Rage step on a material is (1) step 1 on the base colour (the bake) and (2) the occlusion pulled toward
/// AntiDarknesAOMax (`ao_factor`); a face / skin / hair's further darkening is INSIDE its anti-mode material (the face darkness
/// driving the toon occlusion to the shadow colour, KH_C2 1940 / 2076..2095), which those slots now get from KH3's own owners
/// (their `@anti` twins, or the outfit's own anti-mode owners: [`mark`]). Measured (PART.md L8): same-albedo black cloth fits
/// KH3 at 1, and was 30 % too dark at 0.3.
pub const BAKED_SHADE: f32 = 1.0;
/// The hair's distance from the head point (cm) for its grade (the hair darkens as the front leaves the head).
pub const HAIR_DISTANCE: f32 = 10.0;

/// KH3's darkness front at `distance` (cm) from the head point for `volume` (AntiVolumeGradeReverse 1, Contrast 2): 1 = dark.
pub fn grade(distance: f32, volume: f32) -> f32 {
    let v = volume.clamp(0.0, 1.0);
    let g = 1.0 - distance / GRADE_LENGTH;
    let mut a = ((g - (1.0 - 2.0 * v)) * (v * 2.0 + 5.0)).clamp(0.0, 1.0);
    a = a * (1.0 - a) + a;
    a * (2.0 * v).clamp(0.0, 1.0)
}

/// The fade one step on: toward 1 while Rage Form is on, toward 0 after, at the lengths above.
pub fn step_volume(volume: f32, on: bool, dt: f32) -> f32 {
    let dt = dt.max(0.0);
    if on {
        (volume + dt / IN_SECONDS.max(1.0e-3)).min(1.0)
    } else if OUT_SECONDS <= 0.0 {
        // (F-POLISH) time <= 0: set at once (the equipment's fade 0x1417915A0).
        0.0
    } else {
        (volume - dt / OUT_SECONDS).max(0.0)
    }
}

/// The fade's state.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rage {
    /// AntiVolume now (0..1).
    pub volume: f32,
    /// The game clock of the last step.
    pub clock: Option<f32>,
    /// The last wear form other than Rage's (what kh-look keeps on until the fade passes [`SNAP`]).
    pub before: u8,
    /// (harness) a fixed AntiVolume.
    pub test: Option<f32>,
}

impl Rage {
    /// One frame: `wear` = the wear form the pawn has applied (lib.rs `wear_form`), `clock` the game clock (s).
    pub fn step(&mut self, wear: u8, clock: f32) -> f32 {
        let dt = self.clock.map_or(0.0, |before| (clock - before).clamp(0.0, 0.1));
        self.clock = Some(clock);
        if wear != RAGE_WEAR_FORM {
            self.before = wear;
        }
        self.volume = match self.test {
            Some(v) => v.clamp(0.0, 1.0),
            None => step_volume(self.volume, wear == RAGE_WEAR_FORM, dt),
        };
        self.volume
    }

    /// The wear form kh-look's `step_wear` is handed: Rage's once the fade has passed [`SNAP`] (also while it fades out),
    /// else the form before it / the one wanted.
    pub fn library_form(&self, wear: u8) -> u8 {
        if self.volume >= SNAP {
            RAGE_WEAR_FORM
        } else if wear == RAGE_WEAR_FORM {
            self.before
        } else {
            wear
        }
    }
}

static STATE: Mutex<Rage> = Mutex::new(Rage { volume: 0.0, clock: None, before: 0, test: None });
/// The head point of the frame (game space, cm) and the glow's phase, for [`rows`].
static GLOW: Mutex<Option<Glow>> = Mutex::new(None);


/// (AUDIT-C) The frame's head for the shader: the grade's origin, AntiVolume and the pulse's phase.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Glow {
    head: [f32; 3],
    volume: f32,
    phase: f32,
}

/// (offline harness) a fixed AntiVolume (`None`: the fade by the wear form).
pub fn set_test_volume(volume: Option<f32>) {
    if let Ok(mut s) = STATE.lock() {
        s.test = volume;
    }
}

/// AntiVolume now.
pub fn volume() -> f32 {
    STATE.lock().map_or(0.0, |s| s.volume)
}

/// The owners of a library folder that compile KH3's anti mode (materials.json `static_switches_true` has UseAntiMode).
fn anti_owners(json: &Value) -> Vec<String> {
    json.get("owners").and_then(Value::as_object).into_iter().flatten().filter(|(_, o)| o.get("static_switches_true").and_then(Value::as_array).is_some_and(|s| s.iter().any(|x| x.as_str() == Some("UseAntiMode")))).map(|(name, _)| name.clone()).collect()
}

/// wear_extra.json `direct["7"]`: (material instance, glow mask file).
fn baked_rows(json: &Value) -> Vec<(String, Option<String>)> {
    json.get("direct").and_then(|d| d.get("7")).and_then(Value::as_array).into_iter().flatten().filter_map(|r| Some((r.get("material")?.as_str()?.to_string(), r.get("glow_mask").and_then(Value::as_str).map(str::to_string)))).collect()
}

fn read_json(path: &Path) -> Option<Value> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()
}

/// The marks [`prepare`] puts on the slots of a library (pure: the two files' JSON).
pub fn mark(library: &mut Library, materials: &Value, wear_extra: Option<&Value>) -> (usize, usize, usize) {
    let anti = anti_owners(materials);
    let baked = wear_extra.map(baked_rows).unwrap_or_default();
    let (mut n_baked, mut n_glow, mut n_hair) = (0, 0, 0);
    // (AUDIT-C) KH3's own darkening for faces, skin, eyelashes, mouths and hair is INSIDE their anti-mode materials (the face
    // darkness drives the toon occlusion to the shadow colour before step 1: KH_C2 1940 / 2076..2095; the hair's occlusion
    // pull KH_C0 289..325). A slot drawn by such a material is never baked: (1) an outfit slot whose OWN owner compiles
    // UseAntiMode with one of those roots (KH2's face, skin, eyelashes: `native`), (2) a slot that has a `<slot>@anti` twin
    // (KH1: KH3's anti owner of the same root, tools/stage_anti_owners.js), drawn instead of it while the look is on.
    let root_of = |slot: &str| materials.get("materials").and_then(|m| m.get(slot)).and_then(|r| r.get("root")).and_then(Value::as_str).unwrap_or("").to_string();
    let twins: Vec<String> = library.slots.iter().filter_map(|s| s.slot.strip_suffix(ALT_SUFFIX).map(str::to_string)).collect();
    let mut unbaked: Vec<String> = Vec::new();
    for slot in library.slots.iter_mut().filter(|s| s.mesh == kh_look::library::WEAR_MESH) {
        let native = anti.contains(&slot.owner) && NATIVE_ROOTS.iter().any(|r| root_of(&slot.slot) == *r);
        if slot.slot.ends_with(ALT_SUFFIX) || native || twins.contains(&slot.slot) {
            if baked.iter().any(|(m, _)| *m == slot.instance_name) && !unbaked.contains(&slot.instance_name) {
                unbaked.push(slot.instance_name.clone());
            }
        } else if let Some((_, glow)) = baked.iter().find(|(m, _)| *m == slot.instance_name) {
            slot.scalars.insert(KEY_BAKED.to_string(), 1.0);
            if slot.instance_name.contains("_F_mat") {
                slot.scalars.insert(KEY_FACE.to_string(), FACE_DARK); // (AUDIT-C)
            }
            n_baked += 1;
            if let Some(glow) = glow.as_ref().filter(|_| slot.texture_names.len() <= GLOW_TEXTURE) {
                if slot.textures.len() <= GLOW_TEXTURE {
                    slot.textures.resize(GLOW_TEXTURE + 1, None);
                }
                if slot.textures[GLOW_TEXTURE].is_none() {
                    slot.textures[GLOW_TEXTURE] = Some(TextureRef { file: glow.clone(), srgb: false, mips: true });
                    slot.scalars.insert(KEY_GLOW.to_string(), 1.0);
                    n_glow += 1;
                }
            }
        }
        if !anti.contains(&slot.owner) && HAIR_COLOURS.iter().any(|n| slot.vectors.contains_key(*n)) {
            // (AUDIT-C) a hair slot without KH3's anti mode: er_char.wgsl runs KH3's step 1 on its FINAL colour (mode 2 of
            // [`rows`]), as KH3's hair does (KH_C0 ~470, after the cuticle / RGBTable mix); its HairColor inputs stay as they are
            slot.scalars.insert(KEY_HAIR.to_string(), 1.0);
            n_hair += 1;
        }
        // (the occlusion of a slot whose anti mode is not run: baked, or without one)
        let own_anti = anti.contains(&slot.owner) && !slot.scalars.contains_key(KEY_BAKED);
        if let (false, Some(c)) = (own_anti, slot.vectors.get(AO_CHANNEL).copied()) {
            slot.vectors.insert(KEY_AO.to_string(), c);
        }
    }
    // (AUDIT-C) their bake is never bound (kh-look `step_wear` would swap the twin's / the native slot's base colour too)
    if let Some(rows) = library.wear.direct.get_mut(&RAGE_WEAR_FORM) {
        rows.retain(|(material, _, _)| !unbaked.contains(material));
    }
    // (AUDIT-C) the twins: (slot, twin) indices
    let pairs: Vec<(usize, usize)> = library.slots.iter().enumerate().filter_map(|(i, s)| library.slots.iter().position(|t| t.mesh == s.mesh && t.slot.strip_suffix(ALT_SUFFIX) == Some(s.slot.as_str())).map(|k| (i, k))).collect();
    if let Ok(mut t) = TWINS.lock() {
        *t = pairs;
    }
    (n_baked, n_glow, n_hair)
}

/// (AUDIT-C) The suffix of a slot drawn with KH3's anti-mode owner instead of its own (tools/stage_anti_owners.js).
pub const ALT_SUFFIX: &str = "@anti";
/// (AUDIT-C) The roots whose KH3 anti-mode darkening lives in the material (no bake for an outfit owner that compiles it).
const NATIVE_ROOTS: [&str; 5] = [
    "/Game/Shader/Material_set/SQEX_BaseMaterialSet/SQEX_CHRBaseMat/pc_skinbasemat.pc_skinbasemat",
    "/Game/Shader/Material_set/SQEX_BaseEyelashes.SQEX_BaseEyelashes",
    "/Game/Shader/Material_set/SQEX_Mouth.SQEX_Mouth",
    "/Game/Shader/Material_set/SQEX_EyeLens.SQEX_EyeLens",
    "/Game/Shader/Material_set/SQEX_KHHairBaseMat.SQEX_KHHairBaseMat",
];
/// (AUDIT-C) (slot, its `@anti` twin) of the drawn library, and whether Rage Form's look is on (AntiVolume > 0).
static TWINS: Mutex<Vec<(usize, usize)>> = Mutex::new(Vec::new());
static TWINS_ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// (AUDIT-C) gfx.rs, every draw of the body: the slot to draw with (its KH3 anti-mode twin while Rage Form's look is on).
pub(crate) fn draw_slot(slot: usize) -> usize {
    if !TWINS_ON.load(std::sync::atomic::Ordering::Relaxed) {
        return slot;
    }
    twin_of(slot).unwrap_or(slot)
}

/// (MIMIC-BOSS hotfix 3, MB3-FORMLEAK) A slot's KH3 anti-mode twin in the drawn library, whoever's Rage look is on (a copy of
/// Sora draws it by HIS AntiVolume, never the player's [`draw_slot`]).
pub(crate) fn twin_of(slot: usize) -> Option<usize> {
    TWINS.lock().ok().and_then(|t| t.iter().find(|(s, _)| *s == slot).map(|(_, k)| *k))
}

/// AT LOAD (gfx.rs, right after `Library::load` of the drawn outfit / the main pack): marks the slots whose base colour wear
/// form 7 bakes, binds their glow masks as material texture 13 (loaded with the slot's own textures) and remembers the
/// original colours of a hair slot without the anti mode.
pub(crate) fn prepare(look: &Path, library: &mut Library) {
    let dir = look.join("materials");
    let Some(materials) = read_json(&dir.join("materials.json")) else { return };
    let extra = read_json(&dir.join("wear_extra.json"));
    let (baked, glow, hair) = mark(library, &materials, extra.as_ref());
    if baked + glow + hair > 0 {
        crate::rlog!("render: (F-RAGE) Rage Form on {}: {baked} baked slot(s) (wear form 7), {glow} glow mask(s) bound, {hair} hair slot(s) darkened by the fade", look.display());
    }
}

/// THE FRAME's parameters (pure): AntiVolume `volume` on every slot that is not baked, the hair colours through step 1.
pub fn apply(library: &mut Library, volume: f32) {
    for slot in library.slots.iter_mut() {
        apply_slot(slot, volume);
    }
}

/// (MIMIC-BOSS hotfix 3, MB3-FORMLEAK) [`apply`] on ONE slot: a copy of Sora's slot copy takes HIS AntiVolume (copy_wear.rs).
pub fn apply_slot(slot: &mut Slot, volume: f32) {
    let baked = slot.scalars.contains_key(KEY_BAKED);
    set(&mut slot.scalars, "AntiVolume", if baked { 0.0 } else { volume });
    let hair = slot.scalars.contains_key(KEY_HAIR);
    // (the hair: KH3's grade at the head, which is dark long before the feet)
    let w = if hair { grade(HAIR_DISTANCE, volume) } else { volume };
    if let Some(c) = slot.vectors.get(KEY_AO).copied() {
        slot.vectors.insert(AO_CHANNEL.to_string(), c.map(|x| x * ao_factor(w)));
    }
}

fn set(map: &mut std::collections::HashMap<String, f32>, name: &str, value: f32) {
    match map.get_mut(name) {
        Some(v) => *v = value,
        None => drop(map.insert(name.to_string(), value)),
    }
}

/// The head point KH3's grade starts from (game space, cm): atama_WP + atama_mtr_x * UD + atama_mtr_y * MU, from the first
/// slot the frame's bone parameters were fed to.
pub(crate) fn head_point(library: &Library) -> Option<[f32; 3]> {
    let slot = library.slots.iter().find(|s| s.vectors.contains_key("atama_WP"))?;
    let p = slot.vectors["atama_WP"];
    let x = slot.vectors.get("atama_mtr_x").copied().unwrap_or([0.0; 4]);
    let y = slot.vectors.get("atama_mtr_y").copied().unwrap_or([0.0; 4]);
    Some([0, 1, 2].map(|k| p[k] + x[k] * OFFSET_UD + y[k] * OFFSET_MU))
}

/// ONE FRAME (gfx.rs, the costume change): steps the fade, writes the slots' parameters and returns the wear form to hand to
/// kh-look's `step_wear` (see [`Rage::library_form`]).
pub(crate) fn frame(library: &mut Library, clock: f32) -> u8 {
    let wear = crate::wear_form();
    let (volume, form, before) = match STATE.lock() {
        Ok(mut s) => {
            let before = s.volume;
            let v = s.step(wear, clock);
            (v, s.library_form(wear), before)
        }
        Err(_) => return wear,
    };
    if (volume > 0.0) != (before > 0.0) {
        crate::rlog!("render: (F-RAGE) Rage Form's look {} (wear form {wear}, AntiVolume {volume:.2})", if volume > 0.0 { "fades in" } else { "is off" });
    }
    apply(library, volume);
    TWINS_ON.store(volume > 0.0, std::sync::atomic::Ordering::Relaxed); // (AUDIT-C)
    if let Ok(mut g) = GLOW.lock() {
        *g = head_point(library).filter(|_| volume > 0.0).map(|head| Glow { head, volume, phase: clock * MASK_SPEED });
    }
    form
}

/// The glow's rows of a slot's material block (gfx.rs, the colour draws): zeros unless the slot has a glow mask and Rage
/// Form's look is on. [`ROW_HEAD`] = the head point + (AUDIT-C) the slot's face darkness; [`ROW_RAGE`] = (AntiVolume, the
/// pulse's phase, the shade, TAG).
pub(crate) fn rows(slot: &Slot, m: &mut [Vec4]) {
    if !slot.scalars.contains_key(KEY_GLOW) && !slot.scalars.contains_key(KEY_HAIR) {
        return;
    }
    let Some(g) = GLOW.lock().ok().and_then(|g| *g) else { return };
    glow_rows(slot, m, g);
}

/// (MIMIC-BOSS hotfix 3, MB3-FORMLEAK) [`rows`] for a copy of Sora: HIS head point (his bone parameters fed: [`head_point`]),
/// HIS AntiVolume and the frame's clock; zeros (nothing written) while his look is off.
pub(crate) fn rows_with(slot: &Slot, m: &mut [Vec4], head: Option<[f32; 3]>, volume: f32, clock: f32) {
    if !slot.scalars.contains_key(KEY_GLOW) && !slot.scalars.contains_key(KEY_HAIR) {
        return;
    }
    if let Some(head) = head.filter(|_| volume > 0.0) {
        glow_rows(slot, m, Glow { head, volume, phase: clock * MASK_SPEED });
    }
}

fn glow_rows(slot: &Slot, m: &mut [Vec4], g: Glow) {
    let hair = slot.scalars.contains_key(KEY_HAIR); // (AUDIT-C)
    if m.len() > ROW_RAGE {
        // (AUDIT-C) the slot's mode: 2 hair, -F a baked face slot (F the bake's face darkness), 0 a baked body slot
        let mode = if hair { 2.0 } else { -slot.scalars.get(KEY_FACE).copied().unwrap_or(0.0) };
        m[ROW_HEAD] = Vec4::new(g.head[0], g.head[1], g.head[2], mode);
        m[ROW_RAGE] = Vec4::new(g.volume, g.phase.fract(), BAKED_SHADE, TAG);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn darkening_is_kh3_step_one() {
        // a white texel: lum 1 -> (0.04, 0.5, 1.0); a red one: lum 0.3
        let white = darken([1.0, 1.0, 1.0], 1.0);
        assert!((white[0] - 0.04).abs() < 1e-6 && (white[1] - 0.5).abs() < 1e-6 && (white[2] - 1.0).abs() < 1e-6);
        let red = darken([1.0, 0.0, 0.0], 1.0);
        assert!((red[0] - 0.012).abs() < 1e-6 && (red[1] - 0.15).abs() < 1e-6 && (red[2] - 0.3).abs() < 1e-6);
        // weight 0 = as is, half way = the mean
        assert_eq!(darken([0.2, 0.4, 0.6], 0.0), [0.2, 0.4, 0.6]);
        let half = darken([1.0, 1.0, 1.0], 0.5);
        assert!((half[0] - 0.52).abs() < 1e-6 && (half[1] - 0.75).abs() < 1e-6 && (half[2] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn hair_goes_dark_blue() {
        let _one = GLOW_TESTS.lock().unwrap_or_else(std::sync::PoisonError::into_inner); // (AUDIT-C) GLOW is shared
        // KH3 / KH1 Sora's HairColor: lum 0.0324 -> (0.0013, 0.0162, 0.0324) (what er_char.wgsl's step 1 gives its final colour)
        let d = darken([0.05, 0.026687, 0.015], 1.0);
        let lum = 0.05 * 0.3 + 0.026687 * 0.59 + 0.015 * 0.11;
        assert!((d[2] - lum).abs() < 1e-6 && d[0] < d[1] && d[1] < d[2]);
        // (AUDIT-C) a hair slot of a library without the anti mode: its inputs stay, its rows say "hair" (mode 2): the shader
        // darkens its FINAL colour as KH3's hair does (the cuticle / RGBTable band included)
        let mut library = test_library();
        let materials = json!({"owners": {"Hair": {"static_switches_true": []}, "Cloth": {"static_switches_true": ["UseAntiMode"]}}});
        let (_, _, hair) = mark(&mut library, &materials, None);
        assert_eq!(hair, 1);
        apply(&mut library, 1.0);
        assert_eq!(library.slots[0].vectors["HairColor"], [0.05, 0.026687, 0.015, 1.0]);
        *GLOW.lock().unwrap() = Some(Glow { head: [1.0, 2.0, 150.0], volume: 1.0, phase: 0.5 });
        let mut m = [Vec4::ZERO; kh_look::MAX_CB];
        rows(&library.slots[0], &mut m);
        assert_eq!(m[ROW_HEAD], Vec4::new(1.0, 2.0, 150.0, 2.0));
        assert_eq!(m[ROW_RAGE].w, TAG);
        *GLOW.lock().unwrap() = None;
        // its occlusion to AntiDarknesAOMax with the fade; the anti-mode slot's own AOChannel untouched (its shader does it)
        let ao = library.slots[0].vectors["AOChannel"];
        assert!(ao[0] == 0.0 && (ao[1] - AO_MAX).abs() < 1e-6 && ao[2] == 0.0 && ao[3] == 0.0);
        assert_eq!(library.slots[2].vectors["AOChannel"], [0.0, 1.0, 0.0, 0.0]);
        // the hair follows KH3's grade at the head: all dark by AntiVolume 0.5, partly at 0.1
        apply(&mut library, 0.5);
        assert!((library.slots[0].vectors["AOChannel"][1] - AO_MAX).abs() < 1e-6);
        apply(&mut library, 0.1);
        let early = library.slots[0].vectors["AOChannel"][1];
        assert!(early > AO_MAX + 0.05 && early < 0.95);
        apply(&mut library, 0.0);
        assert_eq!(library.slots[0].vectors["AOChannel"], [0.0, 1.0, 0.0, 0.0]);
    }

    #[test]
    fn fade_curve() {
        // in: 1.0 s, linear; out: at once (F-POLISH: EndStyle case 0x8B's 0 s)
        let mut v = 0.0;
        for _ in 0..30 {
            v = step_volume(v, true, 1.0 / 60.0);
        }
        assert!((v - 0.5).abs() < 1e-4);
        v = step_volume(v, true, 2.0);
        assert_eq!(v, 1.0);
        assert_eq!(step_volume(v, false, 1.0 / 60.0), 0.0);
        assert_eq!(step_volume(v, false, 0.0), 0.0);
        assert_eq!(step_volume(0.3, true, -1.0), 0.3);
    }

    #[test]
    fn grade_front_spreads_from_the_head() {
        // nothing at 0, all of the body (2 m) at 1; at 0.25 the head is dark before the feet
        assert_eq!(grade(0.0, 0.0), 0.0);
        assert!(grade(0.0, 1.0) > 0.999 && grade(200.0, 1.0) > 0.999);
        assert!(grade(10.0, 0.25) > grade(150.0, 0.25));
        assert_eq!(grade(150.0, 0.25), 0.0);
    }

    #[test]
    fn keyed_on_wear_form_seven() {
        let mut r = Rage::default();
        // from Second Form (5) into Rage: the fade rises, kh-look keeps 5 until it passes SNAP
        r.step(5, 0.0);
        r.step(RAGE_WEAR_FORM, 0.05);
        assert!(r.volume > 0.0 && r.volume < SNAP);
        assert_eq!(r.library_form(RAGE_WEAR_FORM), 5);
        for k in 2..30 {
            r.step(RAGE_WEAR_FORM, 0.05 * k as f32);
        }
        assert!(r.volume > SNAP);
        assert_eq!(r.library_form(RAGE_WEAR_FORM), RAGE_WEAR_FORM);
        // out to normal: (F-POLISH) at once, EndStyle case 0x8B's 0 s: the first frame out has AntiVolume 0 and the normal form
        r.step(0, 1.5 + 0.05);
        assert_eq!((r.volume, r.library_form(0)), (0.0, 0));
        // any other form leaves it at 0
        let mut n = Rage::default();
        for k in 0..100 {
            n.step(5, 0.02 * k as f32);
        }
        assert_eq!((n.volume, n.library_form(5)), (0.0, 5));
        // the harness's fixed value
        let mut t = Rage { test: Some(0.7), ..Rage::default() };
        assert_eq!(t.step(0, 0.0), 0.7);
    }

    #[test]
    fn baked_slots_keep_anti_volume_off_and_take_their_glow() {
        let _one = GLOW_TESTS.lock().unwrap_or_else(std::sync::PoisonError::into_inner); // (AUDIT-C) GLOW is shared
        let mut library = test_library();
        let materials = json!({"owners": {"Hair": {"static_switches_true": []}, "Cloth": {"static_switches_true": ["UseAntiMode"]}}});
        let extra = json!({"direct": {"7": [{"material": "cloth_inst", "parameter": "BaseColorTex", "file": "textures/c_kbf7.png", "glow_mask": "textures/c_kbf7_glow.png"}]}});
        let (baked, glow, _) = mark(&mut library, &materials, Some(&extra));
        assert_eq!((baked, glow), (1, 1));
        assert_eq!(library.slots[1].textures[GLOW_TEXTURE].as_ref().map(|t| t.file.as_str()), Some("textures/c_kbf7_glow.png"));
        apply(&mut library, 0.8);
        assert_eq!(library.slots[1].scalars["AntiVolume"], 0.0);
        assert_eq!(library.slots[2].scalars["AntiVolume"], 0.8);
        // the glow's rows: only on the glow slot, tagged
        *GLOW.lock().unwrap() = Some(Glow { head: [1.0, 2.0, 150.0], volume: 0.8, phase: 3.25 });
        let mut m = [Vec4::ZERO; kh_look::MAX_CB];
        rows(&library.slots[2], &mut m);
        assert_eq!(m[ROW_RAGE], Vec4::ZERO);
        rows(&library.slots[1], &mut m);
        assert_eq!(m[ROW_RAGE], Vec4::new(0.8, 0.25, BAKED_SHADE, TAG));
        assert_eq!(m[ROW_HEAD], Vec4::new(1.0, 2.0, 150.0, 0.0)); // (AUDIT-C) no face slot: face darkness 0
        *GLOW.lock().unwrap() = None;
    }

    #[test]
    fn faces_skin_and_hair_take_kh3_anti_owners_never_the_bake() {
        // (AUDIT-C) KH1: the face has an `@anti` twin (KH3's anti owner): neither is baked, the bake's direct row is dropped and
        // the twin is drawn while the look is on; KH2: a face whose own owner compiles UseAntiMode is native (not baked)
        let _one = GLOW_TESTS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let face_root = NATIVE_ROOTS[0];
        let rec = |owner: &str, inst: &str| json!({"mesh": "sora", "owner": owner, "asset": format!("/Game/x/{inst}.{inst}"), "root": face_root, "blend_mode": "BLEND_Opaque", "textures": [{"file": "textures/a.png"}], "scalars": {}, "vectors": {}});
        let owner = |id: u64, anti: bool| json!({"id": id, "base": {"ok": true}, "slots": [{"parameter": "BaseColorTex"}], "vec": [], "sca": [], "static_switches_true": if anti { json!(["UseAntiMode"]) } else { json!([]) }});
        let json = json!({
            "format": "kh3-sora-materials/1",
            "owners": {"skin": owner(0, false), "skin@kh3": owner(1, true), "own_anti": owner(2, true)},
            "materials": {"f_face": rec("skin", "f_inst"), "f_face@anti": rec("skin@kh3", "f_inst"), "g_face": rec("own_anti", "g_inst")}
        });
        let mut library = Library::from_json(std::path::PathBuf::from("."), &json).unwrap();
        let extra = json!({"direct": {"7": [{"material": "f_inst", "parameter": "BaseColorTex", "file": "textures/f_kbf7.png"}, {"material": "g_inst", "parameter": "BaseColorTex", "file": "textures/g_kbf7.png"}]}});
        library.add_wear_extra(&extra);
        let (baked, _, _) = mark(&mut library, &json, Some(&extra));
        assert_eq!(baked, 0, "no face slot is baked");
        assert!(library.wear.direct.get(&RAGE_WEAR_FORM).is_none_or(Vec::is_empty), "their bake rows are dropped");
        let (face, twin) = (library.slots.iter().position(|s| s.slot == "f_face").unwrap(), library.slots.iter().position(|s| s.slot == "f_face@anti").unwrap());
        TWINS_ON.store(false, std::sync::atomic::Ordering::Relaxed);
        assert_eq!(draw_slot(face), face);
        TWINS_ON.store(true, std::sync::atomic::Ordering::Relaxed);
        assert_eq!(draw_slot(face), twin, "the twin is drawn while the look is on");
        TWINS_ON.store(false, std::sync::atomic::Ordering::Relaxed);
        apply(&mut library, 1.0);
        assert!(library.slots.iter().all(|s| s.scalars["AntiVolume"] == 1.0), "every one takes AntiVolume itself");
    }

    /// (AUDIT-C) the tests that set GLOW run one at a time.
    static GLOW_TESTS: Mutex<()> = Mutex::new(());

    fn test_library() -> Library {
        let slot = |name: &str, owner: &str, inst: &str| {
            json!({"mesh": "sora", "owner": owner, "asset": format!("/Game/x/{inst}.{inst}"), "blend_mode": "BLEND_Opaque", "textures": [{"file": "textures/a.png"}], "scalars": {}, "vectors": if owner == "Hair" { json!({"HairColor": [0.05, 0.026687, 0.015, 1.0], "AOChannel": [0.0, 1.0, 0.0, 0.0]}) } else { json!({"AOChannel": [0.0, 1.0, 0.0, 0.0]}) }, "slot": name})
        };
        let owner = |id: u64| json!({"id": id, "base": {"ok": true}, "slots": [{"parameter": "BaseColorTex"}], "vec": [], "sca": []});
        let json = json!({
            "format": "kh3-sora-materials/1",
            "owners": {"Hair": owner(0), "Cloth": owner(1)},
            "materials": {"a_hair": slot("a_hair", "Hair", "hair_inst"), "b_cloth": slot("b_cloth", "Cloth", "cloth_inst"), "c_eye": slot("c_eye", "Cloth", "eye_inst")}
        });
        Library::from_json(std::path::PathBuf::from("."), &json).unwrap()
    }
}
