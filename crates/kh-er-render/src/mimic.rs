//! (MIMIC SORA) THE MIMIC TEAR'S LOOK FOR A KH CHARACTER, from Elden Ring's own data (THE USER 2026-11-02: "Make sure mimic
//! Sora uses the shaders that mimic should use from Elden Ring."). Pure numbers and functions: the renderer's mimic pass, the
//! offline picture (examples/mimic_pic.rs) and the tests share them. Nothing here draws yet (the second Sora it is for is not
//! drawn by the renderer yet: kh-partyhost mimic.rs lists what is missing, step 4 is this crate's).
//!
//! WHERE ELDEN RING KEEPS IT (regulation 2.7.2.0, `build/sora_tmp/er/port272/reg/regulation_2720.bin`, read offline with
//! `build/sora_tmp/party/SUMMON/reg2.js` and the paramdex defs in `vendor/fromsoftware-rs/tools/param-generator/params/eldenring/`):
//! - goods 207000 (Mimic Tear, `refId_default` 207000, `consumeHP` 660) -> BuddyParam 20700001 (`triggerSpEffectId` 207000,
//!   `npcParamId` 100000010 = the c0000 copy of the player, `npcPlayerInitParamId` 26050, `generateAnimId` 60500);
//! - NpcParam 100000010, RESIDENT special effect 24 (`spEffectID24`, "常駐特殊効果24") = SpEffect 16316, which no other NpcParam
//!   row carries -> its `vfxId` 53202, which no other SpEffect row uses -> SpEffectVfx 53202: `wetAspectType` 18 (no other row
//!   uses 18; "水濡れ効果: ウェットパラメータを参照して水濡れ表現を発生させる", the wet look from WetAspectParam), `isMidstFullbody` 1,
//!   `isVisibleDeadChr` 1, no effect or sound of its own;
//! - WetAspectParam 18 = THE LIQUID SILVER ([`MIMIC_WET`]): base colour (200, 200, 200) at an override rate of 0.8, metallic 255
//!   at 1.0, shininess 255 at 0.85 (each field's description: "...のオーバーライド率", the override rate);
//! - every spirit's resident SpEffect 295000 (NpcParam 100000010 `spEffectID26`, as the other ashes') -> SpEffectVfx 57000:
//!   `phantomParamOverwriteType` 2, `phantomParamOverwriteId` 200 -> PhantomParam 200 = THE SPIRIT SHEEN ([`SPIRIT_PHANTOM`]).
//!
//! HOW IT IS REPRODUCED (our reading; the game's shader code for the two was NOT read):
//! - the wet aspect overrides the SURFACE the game's G-buffer receives (its targets 1 = metalness, gloss, translucency, occlusion
//!   and 2 = albedo, hybrid.rs): each value goes `rate` of the way to the row's value ([`WetAspect::apply`]). Our KH characters
//!   already write that G-buffer (the hybrid's `_gb3` pass: white, metalness 0, gloss `r.hybrid_gloss`); a mimic writes the
//!   overridden surface instead ([`WetAspect::gbuffer`]) and the GAME'S OWN light passes then shade it as they shade the Mimic
//!   Tear (its reflections, its specular); his KH colour pass is not drawn over those pixels;
//! - the phantom row adds a view-dependent tint over the lit surface ([`Phantom::shade`]): `edgeColor` toward the silhouette
//!   (`(1 - N.V) ^ edgePower`), `frontColor` where he faces the camera, `diffMulColor` / `specMulColor` on the two light terms
//!   (A = the strength of each colour), added (`isEdgeSubtract` / `isFrontSubtract` 0), opaque (`alpha` 1); `glowScale` of the
//!   tint into the glow ([`Phantom::glow`]).
//! - colours: the rows' bytes are read as sRGB and mixed in linear light (an assumption: the G-buffer's albedo target is an
//!   sRGB view, hybrid.rs).
//!
//! THE DRAW (kh-partyhost mimic.rs step 4, built 2026-11-03): the host hands its Mimic Soras over with [`submit_mimic`] once a
//! frame; gfx.rs (`gfx/mimic_draw.rs`) draws each as a SECOND INSTANCE of Sora's own models (the main pack's `sora` mesh, or the
//! worn outfit's, and the Keyblade: the same assets, his own skin constants and his own skin of the frame before) with a
//! PER-DRAW MATERIAL OVERRIDE ([`slot_pass`]): his early depth (+ stencil) and his glow mask as Sora's, his G-BUFFER with the
//! Mimic Tear writers (`<variant>_gb3_mm.ps.dxil`, kh-shaderc `--only mimic`: er_char.wgsl gb_main KH_MIMIC, WetAspectParam 18
//! over the material's base colour, the subsurface / emissive targets written as none), his motion vectors and his real
//! shadow as Sora's; his KH COLOUR DRAW, the hair fringe and the translucent slots are NOT drawn: Elden Ring's own deferred light
//! passes and composite shade the liquid-silver surface, as they shade the Mimic Tear. The hybrid's capture of the game's
//! light (hycap) takes only the KH characters' white G-buffer albedo (er_char.wgsl `hc_kh`, min > 0.996): the mimic's grey metal
//! (about 0.66) is never one of its texels, so the player's capture is the same with a mimic beside him.
//! (THE USER 2026-11-03: "mimic Sora also needs to use the same outfit that Sora is currently using") THE OUTFIT: the mimic is
//! drawn with the renderer's WORN outfit (KH1, KH2, the world costumes, an own set such as Verum Rex's): its model, its
//! retarget (his pose onto its skeleton, without its cloth simulation, which is the player's), the Keyblade in its hand, and
//! its own mimic writers (`PACK/outfits/<id>/materials/er/<variant>[_S]_gb3_mm`, built for every outfit and own set). One
//! outfit is held at a time: a change of it while a mimic is out redraws him in the new one once its writers are made.
//! THE FALLBACK ([`frame_route`]): when the hybrid's G-buffer route cannot run this frame (`r.hybrid 0`, no early split, the
//! game's targets not as known, the pack or the worn outfit without the mimic set) the mimic is NOT DRAWN that frame, and the
//! reason is in [`mimic_status`] (never Sora's own colours on him). NOT DONE: PhantomParam 200's sheen ([`Phantom`], er_mimic.wgsl
//! `mm_tint_ps`) is not drawn in the game yet (it needs a per-material skinned additive set of its own); the picture has it.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use glam::Vec3;

/// One Mimic Sora of this frame: Sora's skeleton posed exactly as FrameInput::body is (component_to_world in GAME space, local +
/// component bones of the main pack's skeleton), and his Keyblade as FrameInput::weapon (None = not drawn).
#[derive(Clone, Debug)]
pub struct MimicDraw {
    pub body: crate::Body,
    pub weapon: Option<crate::Weapon>,
    /// (THE USER 2026-11-03: his arrival "transforming into Sora" and his dissolve) how much of him is formed ([`MimicForm::WHOLE`]:
    /// all of him, the draw as before).
    pub form: MimicForm,
    /// (MIMIC-BOSS hotfix 3, MB3-FORMLEAK) HIS core's wear form (kh-core `combat.form.wear_form`): his costume, slide and Rage look are his own
    /// (crate::copy_wear), never the player's. None (with `owner`) = the library as the player wears it (as before).
    pub wear_form: Option<u8>,
    /// (MIMIC-BOSS hotfix 3, MB3-FORMLEAK) the owner of his material drives and his look (kh-host `char_drives`: copy member m = m + 1; Mimic Sora 13).
    pub owner: Option<u8>,
}

impl MimicDraw {
    /// A whole mimic (as before the forming).
    pub fn new(body: crate::Body, weapon: Option<crate::Weapon>) -> MimicDraw {
        MimicDraw { body, weapon, form: MimicForm::WHOLE, wear_form: None, owner: None }
    }
}

/// (MIMIC SORA, THE USER 2026-11-03: "mimic sora, upon first being summoned, needs to play the proper mimic tear animation
/// transforming into Sora"; his death "KH3 knockdown + silver dissolve") HOW MUCH OF HIM IS FORMED this frame, the host's timing.
/// Everything of him above the cut leaves NOTHING (no depth, no G-buffer, no motion vectors, no shadow, no glow mask): the
/// world shows there; a band of [`LIQUID_BAND_CM`] under the cut is the liquid (glossier, a little brighter); the pool is a flat
/// liquid-silver disc on the ground at his feet with the Mimic Tear's surface (drawn even with nothing of him formed).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MimicForm {
    /// 0..1: the share of his height formed from the feet up (1 = whole, 0 = nothing above his feet).
    pub height: f32,
    /// GAME-space z of his feet (cm) and his full height (cm, ~ 180 x his body scale): where the cut is.
    pub feet_z: f32,
    pub tall: f32,
    /// Radius (cm) of a liquid-silver pool at his feet, 0 = none (the puddle he rises from / melts into).
    pub pool: f32,
}

impl MimicForm {
    /// All of him, no pool: the draw as before.
    pub const WHOLE: MimicForm = MimicForm { height: 1.0, feet_z: 0.0, tall: 180.0, pool: 0.0 };

    /// The cut (game z, cm): nothing of him above it.
    pub fn cut_z(&self) -> f32 {
        self.feet_z + self.height.clamp(0.0, 1.0) * self.tall.max(0.0)
    }

    /// How his body is drawn this frame.
    pub fn body(&self) -> BodyRoute {
        if !(self.height.is_finite()) || self.height <= 0.0 {
            BodyRoute::None
        } else if self.height >= 1.0 {
            BodyRoute::Whole
        } else {
            BodyRoute::Forming { cut_z: self.cut_z(), band: LIQUID_BAND_CM }
        }
    }

    /// The pool's radius when one is drawn.
    pub fn pool(&self) -> Option<f32> {
        (self.pool.is_finite() && self.pool > 0.5).then_some(self.pool)
    }

    /// Nothing of him goes to the GPU this frame.
    pub fn nothing(&self) -> bool {
        self.body() == BodyRoute::None && self.pool().is_none()
    }
}

impl Default for MimicForm {
    fn default() -> MimicForm {
        MimicForm::WHOLE
    }
}

/// The liquid band under the cut (cm; OUR CHOICE).
pub const LIQUID_BAND_CM: f32 = 6.0;
/// The pool lies this far above his feet (cm), so the ground does not cover it (1 cm was covered offline: his lowest bone lies
/// under the floor his soles stand on).
pub const POOL_LIFT_CM: f32 = 4.0;
/// Its rim's ripple (a share of its radius).
pub const POOL_RIPPLE: f32 = 0.06;

/// How a mimic's body is drawn this frame ([`MimicForm::body`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BodyRoute {
    /// Nothing of his body (the pool may still be drawn).
    None,
    /// All of him: his early depth + the Mimic Tear writers, his motion vectors, shadow and glow mask (the draw as before).
    Whole,
    /// Forming: his own depth + G-buffer in one pass with the forming writers (`_gb3_mf`), nothing above `cut_z` (game cm), the
    /// liquid `band` cm under it; no early depth, no motion vectors, no shadow, no glow mask.
    Forming { cut_z: f32, band: f32 },
}

/// The pool's block (er_char.wgsl ER_MIMIC_POOL, in the skin's slot): column 0 its centre (game cm) and radius; 1 the lift, the
/// rim's ripple, the soft edge's share, the meniscus; 2 the liquid's albedo, gloss, ripple strength, ripple wavelength (cm); 3 the
/// ripples' speed (cm/s).
pub fn pool_block(feet: Vec3, radius: f32, look: &MimicLook) -> [f32; 16] {
    [feet.x, feet.y, feet.z, radius, POOL_LIFT_CM, POOL_RIPPLE, look.edge, look.meniscus, look.pool_albedo, look.pool_gloss, look.ripple, look.ripple_cm, look.ripple_speed, 0.0, 0.0, 0.0]
}

/// (THE USER 2026-11-03, the coordinator's polish round after the in-game strips: "the pool reads as a flat WHITE blob ... it should
/// look exactly like Elden Ring's Mimic Tear liquid silver: darker reflective metal that mirrors the surroundings, a liquid surface
/// with moving ripples, a SOFT EDGE into the ground") THE LIQUID'S LOOK: the pool and the liquid band of his forming edge. OUR
/// numbers (the game has no pool: its Mimic Tear rises out of its own effect), chosen to read as a dark chrome mirror; the body keeps
/// the game's own row (WetAspectParam 18). The host's keys (`r.mimic_pool_*`, [`MimicLook::with_keys`]) change them while it runs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MimicLook {
    /// `r.mimic_pool_albedo` (0.2): the metal's colour = its reflectance F0 (linear grey; the body's is about 0.66).
    pub pool_albedo: f32,
    /// `r.mimic_pool_gloss` (0.8): the G-buffer gloss (1 = a perfect mirror).
    pub pool_gloss: f32,
    /// `r.mimic_pool_ripple` (0.35): the ripples' slope (0 = a flat mirror).
    pub ripple: f32,
    /// `r.mimic_pool_ripple_cm` (14): their wavelength (cm).
    pub ripple_cm: f32,
    /// `r.mimic_pool_ripple_speed` (16): how fast they run outward (cm/s).
    pub ripple_speed: f32,
    /// `r.mimic_pool_edge` (0.3): the share of the radius over which the rim breaks up into the ground (0 = a hard rim).
    pub edge: f32,
    /// `r.mimic_pool_meniscus` (0.8): how far the normal bends outward and down at the rim (the liquid's meniscus).
    pub meniscus: f32,
}

impl MimicLook {
    pub const DEFAULT: MimicLook = MimicLook { pool_albedo: 0.2, pool_gloss: 0.8, ripple: 0.35, ripple_cm: 14.0, ripple_speed: 16.0, edge: 0.3, meniscus: 0.8 };

    /// The look with the host's keys (`key("r.mimic_pool_albedo")` ...; absent = the default), each kept in its range.
    pub fn with_keys(key: &dyn Fn(&str) -> Option<f32>) -> MimicLook {
        let d = MimicLook::DEFAULT;
        let k = |name: &str, default: f32, lo: f32, hi: f32| key(name).filter(|v| v.is_finite()).unwrap_or(default).clamp(lo, hi);
        MimicLook {
            pool_albedo: k("r.mimic_pool_albedo", d.pool_albedo, 0.0, 1.0),
            pool_gloss: k("r.mimic_pool_gloss", d.pool_gloss, 0.0, 1.0),
            ripple: k("r.mimic_pool_ripple", d.ripple, 0.0, 4.0),
            ripple_cm: k("r.mimic_pool_ripple_cm", d.ripple_cm, 1.0, 500.0),
            ripple_speed: k("r.mimic_pool_ripple_speed", d.ripple_speed, -500.0, 500.0),
            edge: k("r.mimic_pool_edge", d.edge, 0.0, 1.0),
            meniscus: k("r.mimic_pool_meniscus", d.meniscus, 0.0, 4.0),
        }
    }

    /// THE POOL AT A POINT (the CPU twin of er_char.wgsl `mmpool_ps`: the offline picture and the tests): `local` = the point on
    /// the disc in units of its radius, `rel_cm` = the same from its centre in cm (game axes), `time` s. None = no pool there (outside
    /// the rippled rim, or broken away in the soft edge); else (normal in game axes, albedo, gloss).
    pub fn pool_at(&self, local: glam::Vec2, rel_cm: glam::Vec2, time: f32) -> Option<(Vec3, f32, f32)> {
        let a = local.y.atan2(local.x);
        let rim = 1.0 - POOL_RIPPLE * (0.5 + 0.5 * (a * 7.0 + 1.3).sin() * (a * 3.0).sin());
        let d = local.length() / rim;
        if d > 1.0 {
            return None;
        }
        // the soft edge: over the outer `edge` of the radius the pool breaks up into the ground (a still noise in world space,
        // its threshold rising to the rim: no blending in a G-buffer writer, the game's TAA softens what is left)
        let e = if self.edge > 0.0 { ((d - (1.0 - self.edge)) / self.edge).clamp(0.0, 1.0) } else { 0.0 };
        if e > 0.0 && pool_noise(rel_cm / POOL_NOISE_CM) < e {
            return None;
        }
        // the ripples: one ring running outward and two crossing trains, their slopes
        let k = std::f32::consts::TAU / self.ripple_cm.max(1.0);
        let w = k * self.ripple_speed;
        let r = rel_cm.length().max(1.0e-3);
        let out = rel_cm / r;
        let mut grad = out * (k * r - w * time).cos();
        for (dir, kk, s) in [(glam::Vec2::new(0.8, 0.6), 0.7, 0.6), (glam::Vec2::new(-0.42, 0.91), 1.3, 0.4)] {
            grad += dir * ((k * kk * rel_cm.dot(dir) - w * kk * time + 1.7).cos() * s);
        }
        let mut n = Vec3::new(-grad.x * self.ripple * 0.5, -grad.y * self.ripple * 0.5, 1.0);
        // the meniscus: the normal bends outward and down toward the rim
        n += Vec3::new(out.x, out.y, 0.0) * (self.meniscus * e);
        Some((n.normalize(), self.pool_albedo * (1.0 - 0.4 * e), self.pool_gloss * (1.0 - 0.3 * e)))
    }
}

impl Default for MimicLook {
    fn default() -> MimicLook {
        MimicLook::DEFAULT
    }
}

/// The soft edge's noise cell (cm).
pub const POOL_NOISE_CM: f32 = 3.0;

/// A smooth value noise in 0..1 (the same hash and blend as er_char.wgsl `mmpool_noise`).
pub fn pool_noise(p: glam::Vec2) -> f32 {
    let hash = |q: glam::Vec2| ((q.dot(glam::Vec2::new(127.1, 311.7))).sin() * 43_758.547).fract().abs();
    let i = p.floor();
    let f = p - i;
    let u = f * f * (glam::Vec2::splat(3.0) - f * 2.0);
    let (a, b, c, d) = (hash(i), hash(i + glam::Vec2::X), hash(i + glam::Vec2::Y), hash(i + glam::Vec2::ONE));
    a + (b - a) * u.x + (c - a) * u.y + (a - b - c + d) * u.x * u.y
}

static LOOK: Mutex<MimicLook> = Mutex::new(MimicLook::DEFAULT);

/// The liquid's look from now on (the host, when its keys change; any thread).
pub fn set_look(look: MimicLook) {
    if let Ok(mut l) = LOOK.lock() {
        *l = look;
    }
}

/// The liquid's look in force.
pub fn look() -> MimicLook {
    LOOK.lock().map(|l| *l).unwrap_or(MimicLook::DEFAULT)
}

/// (THE COORDINATOR 2026-11-03: "the first summon of a session loses ~1 s of the 3.6 s arrival while the mimic states are made")
/// PRE-BUILD: the renderer's worker makes the mimic's states (the writers, the forming writers, the pool) for the frame's target and
/// the worn outfit as if a mimic were submitted, and keeps them (made again after an outfit change). Any thread, cheap, idempotent.
pub fn preload_mimic() {
    WANTED.store(true, Ordering::Relaxed);
}

static READY: AtomicBool = AtomicBool::new(false);

/// A mimic submitted now would be drawn this frame: his states are made for the frame's target and the worn outfit, and the
/// hybrid's G-buffer route ran in the latest frame drawn.
pub fn mimic_ready() -> bool {
    READY.load(Ordering::Relaxed)
}

pub(crate) fn set_ready(ready: bool) {
    READY.store(ready, Ordering::Relaxed);
}

/// A list lapses this long after the latest submission (as Sora's frames and the party's do).
pub const LAPSE: Duration = Duration::from_millis(500);

static MIMICS: Mutex<Option<(Arc<Vec<MimicDraw>>, Instant)>> = Mutex::new(None);
/// A mimic was ever submitted: only then are his pipeline states made (none submitted = nothing of this is made or drawn).
static WANTED: AtomicBool = AtomicBool::new(false);
static STATUS: Mutex<String> = Mutex::new(String::new());

/// THE MIMICS OF THIS FRAME (any thread, once a frame after crate::submit); lapses 500 ms after the latest call; an empty list
/// draws none.
pub fn submit_mimic(draws: Vec<MimicDraw>) {
    if !draws.is_empty() {
        WANTED.store(true, Ordering::Relaxed);
    }
    if let Ok(mut g) = MIMICS.lock() {
        *g = if draws.is_empty() { None } else { Some((Arc::new(draws), Instant::now())) };
    }
}

/// No mimic is drawn until the next submission.
pub fn clear_mimic() {
    if let Ok(mut g) = MIMICS.lock() {
        *g = None;
    }
}

/// One line for the host's once-a-second log (drawn / not drawn and why).
pub fn mimic_status() -> String {
    let s = STATUS.lock().map(|s| s.clone()).unwrap_or_default();
    format!("(MIMIC SORA) mimic drawing: {}", if s.is_empty() { if WANTED.load(Ordering::Relaxed) { "submitted, not drawn yet".to_string() } else { "none submitted".to_string() } } else { s })
}

/// A list submitted at `at` is still in force `now`.
pub fn in_force(at: Instant, now: Instant) -> bool {
    now.saturating_duration_since(at) < LAPSE
}

pub(crate) fn draws_now() -> Option<Arc<Vec<MimicDraw>>> {
    let g = MIMICS.lock().ok()?;
    g.as_ref().filter(|(d, at)| !d.is_empty() && in_force(*at, Instant::now())).map(|(d, _)| d.clone())
}

pub(crate) fn wanted() -> bool {
    WANTED.load(Ordering::Relaxed) || test_offset().is_some()
}

/// (testing aid, the offline harnesses only: environment `KH_MIMIC_TEST = X,Y` in cm) a Mimic Sora drawn as a copy of the
/// frame's own Sora moved by (X, Y) in game space, without any host submitting one: the real renderer's mimic path offline.
fn test_offset() -> Option<Vec3> {
    static OFFSET: std::sync::OnceLock<Option<Vec3>> = std::sync::OnceLock::new();
    *OFFSET.get_or_init(|| {
        let text = std::env::var("KH_MIMIC_TEST").ok()?;
        let v: Vec<f32> = text.split(',').filter_map(|p| p.trim().parse().ok()).collect();
        (v.len() == 2).then(|| Vec3::new(v[0], v[1], 0.0))
    })
}

/// The frame's mimics: the host's, else (testing aid) the copy of `input`'s Sora `KH_MIMIC_TEST` asks for.
pub(crate) fn draws_or_test(input: &crate::FrameInput) -> Option<Arc<Vec<MimicDraw>>> {
    draws_now().or_else(|| {
        let off = test_offset()?;
        let mut body = input.body.clone()?;
        body.component_to_world.translation += off;
        let weapon = input.weapon.clone().map(|mut w| {
            w.to_world.translation += off;
            w
        });
        // (`KH_MIMIC_FORM = HEIGHT,POOL`: his forming, the cut from his feet = his lowest bone)
        let feet_z = body.component.iter().map(|t| t.then(&body.component_to_world).translation.z).fold(f32::INFINITY, f32::min);
        let feet_z = if feet_z.is_finite() { feet_z } else { body.component_to_world.translation.z };
        let form = std::env::var("KH_MIMIC_FORM").ok().map(|t| t.split(',').filter_map(|p| p.trim().parse::<f32>().ok()).collect::<Vec<f32>>()).filter(|v| v.len() == 2).map_or(MimicForm::WHOLE, |v| MimicForm { height: v[0], feet_z, tall: 180.0, pool: v[1] });
        Some(Arc::new(vec![MimicDraw { body, weapon, form, wear_form: None, owner: None }]))
    })
}

pub(crate) fn set_status(text: String) {
    if let Ok(mut s) = STATUS.lock() {
        *s = text;
    }
}

/// THE PER-DRAW MATERIAL OVERRIDE of one material slot of a mimic's model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotPass {
    /// Not drawn at all (a translucent slot: an eye's lens, the lashes; or a slot without a mimic writer).
    Skip,
    /// His early depth (+ stencil) with the slot's own shaders, then the slot's Mimic Tear G-buffer writer (`_gb3_mm`); the
    /// glow mask, the motion vectors and the shadow as Sora's. NEVER his KH colour draw.
    DepthAndMimicGbuffer,
}

/// The pass of a slot: `has_mimic_state` = its `_gb3_mm` pipeline state exists for this frame's target.
pub fn slot_pass(translucent: bool, has_mimic_state: bool) -> SlotPass {
    if translucent || !has_mimic_state {
        SlotPass::Skip
    } else {
        SlotPass::DepthAndMimicGbuffer
    }
}

/// Is the colour pass ever drawn for a mimic's slot? Never (the game's light passes shade his G-buffer surface).
pub const COLOUR_DRAWN: bool = false;

/// THE FRAME'S ROUTE: `hybrid` = why the hybrid's G-buffer route does not run this frame (None: it runs), `set` = why his mimic
/// set is not there (None: it is), `targets` = the game's subsurface / emissive targets are of the known formats. Ok = drawn;
/// Err = the reason he is not drawn this frame (the fallback: not drawn, never in Sora's own colours).
pub fn frame_route(hybrid: Option<&str>, set: Option<&str>, targets: bool) -> Result<(), String> {
    if let Some(why) = hybrid {
        return Err(format!("not drawn: the hybrid's G-buffer route does not run this frame ({why})"));
    }
    if let Some(why) = set {
        return Err(format!("not drawn: {why}"));
    }
    if !targets {
        return Err("not drawn: the game's subsurface / emissive G-buffer targets are not of the known formats".to_string());
    }
    Ok(())
}

/// A WetAspectParam row (the paramdex def `WetAspectParam.xml`; `baseColorA` / `metallicRate` / `shininessRate` are the
/// override rates of the three values).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WetAspect {
    pub base_color: [u8; 3],
    pub base_rate: f32,
    pub metallic: u8,
    pub metallic_rate: f32,
    pub shininess: u8,
    pub shininess_rate: f32,
}

/// WetAspectParam 18, the Mimic Tear's (regulation 2.7.2.0; see the module's chain).
pub const MIMIC_WET: WetAspect = WetAspect { base_color: [200, 200, 200], base_rate: 0.8, metallic: 255, metallic_rate: 1.0, shininess: 255, shininess_rate: 0.85 };

/// The wet aspect row the Mimic Tear's SpEffectVfx 53202 names (`wetAspectType`).
pub const MIMIC_WET_ROW: u8 = 18;

/// A PhantomParam row (the paramdex def `PhantomParam.xml`; colours as bytes, each with its strength `*_a`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Phantom {
    pub edge: [u8; 3],
    pub edge_a: f32,
    pub front: [u8; 3],
    pub front_a: f32,
    pub diff_mul: [u8; 3],
    pub diff_mul_a: f32,
    pub spec_mul: [u8; 3],
    pub spec_mul_a: f32,
    pub light: [u8; 3],
    pub light_a: f32,
    pub alpha: f32,
    pub blend_rate: f32,
    pub blend_type: u8,
    pub edge_subtract: bool,
    pub front_subtract: bool,
    pub no_2pass: bool,
    pub edge_power: f32,
    pub glow_scale: f32,
}

/// PhantomParam 200, every spirit's sheen (SpEffect 295000 -> SpEffectVfx 57000; regulation 2.7.2.0).
pub const SPIRIT_PHANTOM: Phantom = Phantom {
    edge: [250, 250, 255],
    edge_a: 0.1,
    front: [35, 35, 45],
    front_a: 0.25,
    diff_mul: [255, 255, 255],
    diff_mul_a: 0.9,
    spec_mul: [255, 255, 255],
    spec_mul_a: 1.0,
    light: [255, 255, 255],
    light_a: 0.0,
    alpha: 1.0,
    blend_rate: 1.0,
    blend_type: 0,
    edge_subtract: false,
    front_subtract: false,
    no_2pass: false,
    edge_power: 0.5,
    glow_scale: 0.3,
};

/// The PhantomParam row the spirits' SpEffectVfx 57000 names (`phantomParamOverwriteId`).
pub const SPIRIT_PHANTOM_ROW: u32 = 200;

/// An sRGB byte as linear light.
pub fn linear_of(byte: u8) -> f32 {
    let c = byte as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Three sRGB bytes as linear light.
pub fn linear3(c: [u8; 3]) -> Vec3 {
    Vec3::new(linear_of(c[0]), linear_of(c[1]), linear_of(c[2]))
}

/// The surface the game's G-buffer takes: linear albedo, metalness and gloss (0..1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Surface {
    pub albedo: Vec3,
    pub metalness: f32,
    pub gloss: f32,
}

impl WetAspect {
    /// The surface with the row's override: each value `rate` of the way to the row's.
    pub fn apply(&self, own: Surface) -> Surface {
        let rate = |r: f32| r.clamp(0.0, 1.0);
        Surface {
            albedo: own.albedo.lerp(linear3(self.base_color), rate(self.base_rate)),
            metalness: own.metalness + (self.metallic as f32 / 255.0 - own.metalness) * rate(self.metallic_rate),
            gloss: own.gloss + (self.shininess as f32 / 255.0 - own.gloss) * rate(self.shininess_rate),
        }
    }

    /// The G-buffer texels of a KH pixel under this row: target 1 (metalness, gloss, translucency, occlusion) and target 2
    /// (albedo in linear light, the light set / 255), in the packing of the hybrid's G-buffer pass (er_char.wgsl `gb_main`).
    /// `kh_albedo`: the KH material's own base colour (linear); `gloss`: the hybrid's surface gloss (`r.hybrid_gloss`).
    pub fn gbuffer(&self, kh_albedo: Vec3, gloss: f32, translucency: f32, lightset: u8) -> ([f32; 4], [f32; 4]) {
        let s = self.apply(Surface { albedo: kh_albedo, metalness: 0.0, gloss });
        ([s.metalness, s.gloss, translucency, 1.0], [s.albedo.x, s.albedo.y, s.albedo.z, lightset as f32 / 255.0])
    }
}

impl Phantom {
    /// The silhouette weight at a pixel: `(1 - N.V) ^ edgePower` (0 facing the camera, 1 at the edge).
    pub fn edge_weight(&self, n_dot_v: f32) -> f32 {
        (1.0 - n_dot_v.clamp(0.0, 1.0)).powf(self.edge_power.max(1.0e-3))
    }

    /// The tint the row adds (or takes away) at a pixel: edge colour toward the silhouette, front colour facing the camera.
    pub fn tint(&self, n_dot_v: f32) -> Vec3 {
        let f = self.edge_weight(n_dot_v);
        let sign = |subtract: bool| if subtract { -1.0 } else { 1.0 };
        linear3(self.edge) * (self.edge_a * f * sign(self.edge_subtract)) + linear3(self.front) * (self.front_a * (1.0 - f) * sign(self.front_subtract))
    }

    /// The lit colour with the row: the diffuse and specular light terms by their multipliers, the tint added, kept >= 0.
    pub fn shade(&self, diffuse: Vec3, specular: Vec3, n_dot_v: f32) -> Vec3 {
        let lit = diffuse * linear3(self.diff_mul) * self.diff_mul_a + specular * linear3(self.spec_mul) * self.spec_mul_a;
        (lit + self.tint(n_dot_v) * self.blend_rate).max(Vec3::ZERO)
    }

    /// What the row adds to the glow (bloom) source at a pixel: `glowScale` of its tint.
    pub fn glow(&self, n_dot_v: f32) -> Vec3 {
        self.tint(n_dot_v).max(Vec3::ZERO) * self.glow_scale
    }
}

/// THE MIMIC'S SURFACE of a KH pixel: the Mimic Tear's wet aspect over the KH material's base colour on the hybrid's surface.
pub fn mimic_surface(kh_albedo: Vec3, hybrid_gloss: f32) -> Surface {
    MIMIC_WET.apply(Surface { albedo: kh_albedo, metalness: 0.0, gloss: hybrid_gloss })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mimic_the_rows_are_the_regulation_rows() {
        // WetAspectParam 18 and PhantomParam 200 as reg2.js printed them (regulation 2.7.2.0)
        assert_eq!(MIMIC_WET_ROW, 18);
        assert_eq!((MIMIC_WET.base_color, MIMIC_WET.base_rate), ([200, 200, 200], 0.8));
        assert_eq!((MIMIC_WET.metallic, MIMIC_WET.metallic_rate, MIMIC_WET.shininess, MIMIC_WET.shininess_rate), (255, 1.0, 255, 0.85));
        assert_eq!(SPIRIT_PHANTOM_ROW, 200);
        assert_eq!((SPIRIT_PHANTOM.edge, SPIRIT_PHANTOM.edge_a, SPIRIT_PHANTOM.front, SPIRIT_PHANTOM.front_a), ([250, 250, 255], 0.1, [35, 35, 45], 0.25));
        assert_eq!((SPIRIT_PHANTOM.diff_mul_a, SPIRIT_PHANTOM.spec_mul_a, SPIRIT_PHANTOM.alpha, SPIRIT_PHANTOM.edge_power, SPIRIT_PHANTOM.glow_scale), (0.9, 1.0, 1.0, 0.5, 0.3));
    }

    #[test]
    fn mimic_any_kh_surface_becomes_bright_glossy_metal() {
        // Sora's red shirt, his black trousers and the white of the hybrid's G-buffer all come out as the same silver metal
        for own in [Vec3::new(0.6, 0.05, 0.04), Vec3::splat(0.02), Vec3::ONE] {
            let s = mimic_surface(own, 0.3);
            assert_eq!(s.metalness, 1.0, "metallic 255 at rate 1: full metal");
            assert!((s.gloss - (0.3 + (1.0 - 0.3) * 0.85)).abs() < 1e-6, "gloss {}", s.gloss);
            let grey = linear_of(200);
            assert!((s.albedo - (own * 0.2 + Vec3::splat(grey * 0.8))).abs().max_element() < 1e-6, "a fifth of his own colour stays: {}", s.albedo);
        }
        let (params, albedo) = MIMIC_WET.gbuffer(Vec3::ONE, 0.3, 0.0, 0);
        assert_eq!(params[0], 1.0);
        assert!((params[1] - 0.895).abs() < 1e-6 && params[3] == 1.0);
        assert!((albedo[0] - (0.2 + 0.8 * linear_of(200))).abs() < 1e-6 && albedo[3] == 0.0);
        assert!((linear_of(200) - 0.578).abs() < 1e-3, "sRGB 200 is about 58 % linear");
    }

    #[test]
    fn mimic_the_spirit_sheen_is_white_at_the_edge_and_dark_blue_in_front() {
        let p = SPIRIT_PHANTOM;
        assert_eq!(p.edge_weight(1.0), 0.0);
        assert_eq!(p.edge_weight(0.0), 1.0);
        assert!((p.edge_weight(0.75) - 0.5).abs() < 1e-6, "edgePower 0.5: the square root");
        let edge = p.tint(0.0);
        let front = p.tint(1.0);
        assert!(edge.min_element() > 0.09 && edge.z >= edge.x, "the edge: a tenth of a near white: {edge}");
        assert!(front.max_element() < 0.01 && front.z > front.x, "the front: a quarter of a dark blue grey: {front}");
        // the light terms: diffuse x 0.9, specular x 1, the tint on top; the glow a third of the tint
        let c = p.shade(Vec3::ONE, Vec3::splat(0.5), 1.0);
        assert!((c.x - (0.9 + 0.5 + front.x)).abs() < 1e-5, "{c}");
        assert!((p.glow(0.0) - edge * 0.3).abs().max_element() < 1e-6);
    }

    #[test]
    fn mimic_the_wgsl_pass_matches_these_numbers() {
        // the GPU side (shaders/er_mimic.wgsl) carries the same rows: the file names them so a change of one shows in both
        let wgsl = include_str!("../shaders/er_mimic.wgsl");
        for needle in ["200.0 / 255.0", "0.8", "0.85", "0.5", "0.3", "250.0 / 255.0", "35.0 / 255.0", "45.0 / 255.0", "0.25", "0.9"] {
            assert!(wgsl.contains(needle), "er_mimic.wgsl lacks {needle}");
        }
    }

    #[test]
    fn mimic_a_mimic_draw_takes_the_mimic_gbuffer_set_and_never_the_colour_pass() {
        // (THE USER 2026-11-02: "Make sure mimic Sora uses the shaders that mimic should use from Elden Ring.")
        assert_eq!(slot_pass(false, true), SlotPass::DepthAndMimicGbuffer, "an opaque / masked slot: early depth + the _gb3_mm writer");
        assert_eq!(slot_pass(true, true), SlotPass::Skip, "a translucent slot is not drawn");
        assert_eq!(slot_pass(false, false), SlotPass::Skip, "no mimic writer for it (an XS form weapon's slot): not drawn");
        const { assert!(!COLOUR_DRAWN) };
        // the er_char.wgsl KH_MIMIC path carries WetAspectParam 18 (the numbers of MIMIC_WET)
        let wgsl = include_str!("../shaders/er_char.wgsl");
        for needle in ["#ifdef KH_MIMIC", "MM_WET_GREY: f32 = 200.0 / 255.0", "MM_WET_BASE_RATE: f32 = 0.8", "MM_WET_METAL: f32 = 255.0 / 255.0", "MM_WET_METAL_RATE: f32 = 1.0", "MM_WET_SHINE_RATE: f32 = 0.85", "@location(3) sss", "@location(4) emissive"] {
            assert!(wgsl.contains(needle), "er_char.wgsl lacks {needle}");
        }
        assert_eq!((MIMIC_WET.base_rate, MIMIC_WET.metallic_rate, MIMIC_WET.shininess_rate), (0.8, 1.0, 0.85));
        // the hybrid's capture takes only the KH white (hc_kh: min > 0.996): the mimic's G-buffer grey never counts as a KH texel
        let s = mimic_surface(Vec3::ONE, 0.3);
        assert!(s.albedo.min_element() < 0.996 && s.albedo.min_element() > 0.5, "{}", s.albedo);
    }

    #[test]
    fn mimic_his_forming_cuts_at_the_share_of_his_height_and_keeps_the_pool() {
        // (THE USER 2026-11-03: his arrival "transforming into Sora" and his dissolve)
        let f = |height: f32, pool: f32| MimicForm { height, feet_z: 1000.0, tall: 180.0, pool };
        // height 1 = exactly today's mimic draw (the whole route: early depth + the _gb3_mm writers), the default too
        assert_eq!(MimicForm::WHOLE.body(), BodyRoute::Whole);
        assert_eq!(MimicForm::default(), MimicForm::WHOLE);
        assert_eq!(f(1.0, 0.0).body(), BodyRoute::Whole);
        assert_eq!(MimicDraw::new(crate::Body { component_to_world: kh_anim::Trs::IDENTITY, local: Vec::new(), component: Vec::new() }, None).form, MimicForm::WHOLE);
        // the cut: the feet + the share of his height; the liquid band under it
        assert_eq!(f(0.25, 0.0).cut_z(), 1045.0);
        assert_eq!(f(0.25, 0.0).body(), BodyRoute::Forming { cut_z: 1045.0, band: LIQUID_BAND_CM });
        assert_eq!(f(0.999, 0.0).body(), BodyRoute::Forming { cut_z: 1000.0 + 0.999 * 180.0, band: LIQUID_BAND_CM });
        assert_eq!(f(2.0, 0.0).body(), BodyRoute::Whole, "over 1: whole");
        // height 0 + pool 0 = nothing submitted to the GPU; a pool alone is drawn
        assert!(f(0.0, 0.0).nothing() && f(-1.0, 0.0).nothing() && f(f32::NAN, 0.0).nothing());
        assert_eq!(f(0.0, 60.0).body(), BodyRoute::None);
        assert_eq!(f(0.0, 60.0).pool(), Some(60.0));
        assert!(!f(0.0, 60.0).nothing());
        assert_eq!(f(0.5, 0.2).pool(), None, "a pool under half a centimetre is none");
        // the pool's block (er_char.wgsl MmPool): centre, radius, lift, ripple
        let l = MimicLook::DEFAULT;
        assert_eq!(pool_block(Vec3::new(1.0, 2.0, 3.0), 50.0, &l), [1.0, 2.0, 3.0, 50.0, POOL_LIFT_CM, POOL_RIPPLE, l.edge, l.meniscus, l.pool_albedo, l.pool_gloss, l.ripple, l.ripple_cm, l.ripple_speed, 0.0, 0.0, 0.0]);
        // the shaders carry the cut (fade.w), the band (misc.x) and the pool's pass
        let wgsl = include_str!("../shaders/er_char.wgsl");
        for needle in ["#ifdef KH_MIMIC_FORM", "in.world.z > kh.fade.w", "kh.misc.x", "#ifdef ER_MIMIC_POOL", "fn mmpool_vs", "fn mmpool_ps"] {
            assert!(wgsl.contains(needle), "er_char.wgsl lacks {needle}");
        }
    }

    #[test]
    fn mimic_the_pool_is_a_dark_rippled_mirror_with_a_soft_edge() {
        // (the coordinator's polish round 2026-11-03: never a flat white disc)
        let l = MimicLook::DEFAULT;
        assert!(l.pool_albedo < 0.66 * 0.5 && l.pool_gloss < 0.9, "dark chrome: F0 well under the body's 0.66");
        // the keys, each in its range; absent = the default
        assert_eq!(MimicLook::with_keys(&|_| None), l);
        let tuned = MimicLook::with_keys(&|k| match k { "r.mimic_pool_albedo" => Some(0.35), "r.mimic_pool_gloss" => Some(7.0), "r.mimic_pool_edge" => Some(f32::NAN), _ => None });
        assert_eq!((tuned.pool_albedo, tuned.pool_gloss, tuned.edge), (0.35, 1.0, l.edge));
        // the middle: there, rippled (the normal leans, and moves with the time), at the liquid's own albedo / gloss
        let (n0, albedo, gloss) = l.pool_at(glam::Vec2::new(0.2, 0.1), glam::Vec2::new(10.0, 5.0), 0.0).unwrap();
        assert_eq!((albedo, gloss), (l.pool_albedo, l.pool_gloss));
        assert!(n0.z > 0.8 && n0.z < 0.9999, "a ripple's slope: {n0}");
        let (n1, ..) = l.pool_at(glam::Vec2::new(0.2, 0.1), glam::Vec2::new(10.0, 5.0), 0.37).unwrap();
        assert!((n1 - n0).length() > 1.0e-3, "the ripples move");
        let flat = MimicLook { ripple: 0.0, ..l };
        assert_eq!(flat.pool_at(glam::Vec2::new(0.2, 0.1), glam::Vec2::new(10.0, 5.0), 0.0).unwrap().0, Vec3::Z, "no ripple: a flat mirror");
        // outside the rim: nothing; the soft edge: broken into the ground (part kept, part not), thinning toward the rim
        assert!(l.pool_at(glam::Vec2::new(1.01, 0.0), glam::Vec2::new(50.0, 0.0), 0.0).is_none());
        let ring: Vec<Option<(Vec3, f32, f32)>> = (0..200).map(|k| {
            let a = k as f32 * 0.0314;
            let p = glam::Vec2::new(a.cos(), a.sin()) * 0.86;
            l.pool_at(p, p * 60.0, 0.0)
        }).collect();
        let kept = ring.iter().filter(|r| r.is_some()).count();
        assert!(kept > 20 && kept < 180, "the rim breaks up: {kept} of 200 kept");
        assert!(ring.iter().flatten().all(|(n, a, _)| *a < l.pool_albedo && n.z < 0.999), "near the rim: thinner, the meniscus bends");
        let hard = MimicLook { edge: 0.0, ..l };
        assert!((0..200).all(|k| { let a = k as f32 * 0.0314; let p = glam::Vec2::new(a.cos(), a.sin()) * 0.86; hard.pool_at(p, p * 60.0, 0.0).is_some() }), "edge 0: a hard rim");
        assert!((0.0..=1.0).contains(&pool_noise(glam::Vec2::new(3.3, -7.1))));
        // the shader carries the same mechanism
        let wgsl = include_str!("../shaders/er_char.wgsl");
        for needle in ["fn mmpool_noise", "er.camera.w", "@interpolate(flat)"] {
            assert!(wgsl.contains(needle), "er_char.wgsl lacks {needle}");
        }
        // the pre-build
        preload_mimic();
        assert!(wanted());
    }

    #[test]
    fn mimic_the_fallback_is_not_drawn_with_the_reason() {
        assert_eq!(frame_route(None, None, true), Ok(()));
        assert!(frame_route(Some("r.hybrid = 0"), None, true).is_err_and(|w| w.contains("r.hybrid = 0") && w.starts_with("not drawn")));
        assert!(frame_route(None, Some("the worn outfit has no mimic set"), true).is_err_and(|w| w.contains("outfit")));
        assert!(frame_route(None, None, false).is_err());
        assert!(frame_route(Some("a"), Some("b"), false).is_err_and(|w| w.contains("(a)")), "the hybrid's reason first");
    }

    #[test]
    fn mimic_the_list_lapses_and_none_submitted_draws_nothing() {
        let t0 = Instant::now();
        assert!(in_force(t0, t0 + Duration::from_millis(499)));
        assert!(!in_force(t0, t0 + LAPSE), "500 ms after the latest call: lapsed");
        let body = crate::Body { component_to_world: kh_anim::Trs::IDENTITY, local: Vec::new(), component: Vec::new() };
        submit_mimic(Vec::new());
        assert!(draws_now().is_none(), "an empty list draws none");
        submit_mimic(vec![MimicDraw::new(body.clone(), None)]);
        assert!(draws_now().is_some_and(|d| d.len() == 1) && wanted());
        clear_mimic();
        assert!(draws_now().is_none());
        assert!(mimic_status().starts_with("(MIMIC SORA)"));
    }

    #[test]
    fn mimic_the_wgsl_pass_parses_and_validates() {
        let module = naga::front::wgsl::parse_str(include_str!("../shaders/er_mimic.wgsl")).map_err(|e| e.emit_to_string(include_str!("../shaders/er_mimic.wgsl"))).expect("er_mimic.wgsl parses");
        let entries: Vec<&str> = module.entry_points.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(entries, ["mm_gb_ps", "mm_tint_ps"]);
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all()).validate(&module).expect("er_mimic.wgsl validates");
    }
}
