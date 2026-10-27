//! Sora drawn by our own Direct3D 12 code inside another game's frame (see Cargo.toml and
//! `build/sora_tmp/er/IMPL_ER2_STATE.md`).
//!
//! The host (kh-er) calls [`install`] once, [`submit`] every game frame with the camera and the
//! pose, [`set_options`] when its control file changes and [`shutdown`] before it goes away. The
//! drawing itself happens on whichever thread the game presents from, inside our hook of
//! `IDXGISwapChain::Present`: one command list of our own, on the game's device and queue.
//!
//! Rules kept here: no panic unwinds into the game (after one, the renderer is inert); every
//! failure of a Direct3D call turns drawing off with a log line instead of going on; the hooks
//! (hook.rs) are put back at shutdown; nothing of the game's command lists is
//! touched (our draws are on our own list, so there is no state to restore).
#![cfg(windows)]

pub mod light;
pub mod math;
pub mod model;
pub mod texture;
// (hud) The 2D overlay: the HUD's layer pictures over the finished frame at Present (hud.rs).
pub mod hud;
// (effects) Sora's visual effects: the draw lists of kh-vfx in our command list (vfx.rs).
pub mod vfx;
// (IMPL_ER39) Light from the effects' particle lights and the hero light in the dark (pointlight.rs).
pub mod pointlight;
// (IMPL_ER60) The blood on Sora and the Keyblade (the *_bl pixel shaders' block).
pub mod blood;
// (IMPL_ER78) The hybrid: KH3's toon look lit by Elden Ring's own world light (the *_hy / *_gb pixel shaders).
pub mod hybrid;

mod cbcap;
mod detour;
mod gfx;
mod hook;
mod passtime; // (HAIRPERF) the GPU time of our main list by section
// (IMPL_ER59) Sora's outfits (Boc's service): which model is drawn, loaded on demand; the pose onto its skeleton.
pub mod outfit;
/// (ER83) KH1 Sora's hands on the Keyblade and on his body (contact IK after the retarget).
pub mod handik;
// (ER81) An outfit's own shader set (Verum Rex: KHS_CG, its world's post-process material) from its pack's data.
pub mod ownset;
// (ER85) a world outfit's own set in the field's light; the harness's stand-in for the game's light
pub mod cgfield;
// (IMPL_ER86) Sora's light world-anchored and steady (the camera no longer changes it)
pub mod stable;
mod shadowcaps; // (IMPL_ER23A) the Keyblade in the stand-in sun shadow
mod shscan;
mod survey;
// (IMPL_ER80b) the game's light shafts kept off his pixels
pub mod shaft;
// (ER88) Sora and the game's temporal anti-aliasing: the check of the previous view-projection his motion vectors use
pub mod taa;
// (ER87) the hair: every dither phase of its mask, the fully covered core opaque, the soft fringe blended after him
pub mod hair;
pub mod hair91;
// (ER90, XSI request 1) the XS moves' form weapons (Hyper Hammer, Blizzard Blades, Ultima) in his hands (PACK/xsi/weapons)
pub mod xsiweapon;
pub use xsiweapon::FormWeapon;
// (ER party S4) Donald, Goofy and the team props drawn by the same passes as Sora (cargo feature `party`, key `r.party`)
#[cfg(feature = "party")]
pub mod party;
// (NF) KH3's camera fade of every character drawn (TresCameraManager's shut-out fade; kh-camera camfade.rs)
pub mod camfade;
// (ZFIN) Zettaflare's finale on an Elden Ring character: the black silhouette that crumbles (finale.rs)
pub mod finale;
// (ZDRAW) Zettaflare's target drawn from its own model into the finale's mask
pub mod zdraw;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use glam::Vec3;
use kh_anim::Trs;

pub use math::Camera;

/// What the host says about one game frame.
#[derive(Clone, Debug)]
pub struct FrameInput {
    pub camera: Camera,
    /// Where the origin of GAME space (KH3's world) lies in HOST space.
    pub origin: Vec3,
    /// The game clock (s): the materials' time.
    pub clock: f32,
    pub body: Option<Body>,
    pub weapon: Option<Weapon>,
    /// (ER90, XSI request 1) The form weapons of an XS move drawn this frame (xsiweapon.rs `place`; empty: none).
    pub forms: Vec<FormWeapon>,
}

/// Sora's pose.
#[derive(Clone, Debug)]
pub struct Body {
    /// The mesh component in GAME space.
    pub component_to_world: Trs,
    /// Every bone relative to its parent / to the component, in the skeleton's order.
    pub local: Vec<Trs>,
    pub component: Vec<Trs>,
}

/// The Keyblade.
#[derive(Clone, Debug)]
pub struct Weapon {
    /// The weapon's component in GAME space (the hand's socket).
    pub to_world: Trs,
    /// The weapon's bones relative to its component, in the weapon skeleton's order (empty: the
    /// reference pose).
    pub component: Vec<Trs>,
    /// The material's `OpacityFade` (0 = not drawn).
    pub opacity: f32,
}

/// Switches of the renderer (the host's control file, keys `r.*`).
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    /// `r.draw`: draw at all.
    pub draw: bool,
    /// `r.depth`: 0 = no scene depth (drawn over everything), 1 = tested against the game's own
    /// depth buffer when it was found (else as 0).
    pub depth: u32,
    /// `r.fovx`: read the camera's field of view as horizontal (experiment).
    pub fov_horizontal: bool,
    /// `r.ccw`: triangles that are counter-clockwise on screen are front faces (default: the
    /// pack's winding after the mirror into game space; seen in the standalone harness).
    pub front_ccw: bool,
    /// `r.cull`: cull back faces of one-sided materials.
    pub cull: bool,
    /// `r.flat`: 1 = a diffuse grey of 18 % under the light he is given instead of the materials
    /// (for measuring him against the game's surfaces), 4 = the same in white, 2 = pure magenta
    /// (for measuring where he is drawn in a screenshot).
    pub flat: u32,
    /// `r.debug`: a G-buffer channel instead of the picture (the harness lists them).
    pub debug: f32,
    /// `r.khs`: the preset of the post-process material ("off" = none).
    pub khs: String,
    /// `r.exposure`: a factor in front of the tone curve.
    pub exposure: f32,
    /// `r.key` / `r.ambient`: scales of the stand-in rig.
    pub key: f32,
    pub ambient: f32,
    /// `r.env`: scale of the measured level of the reflection environment (1 = as measured from
    /// the game's picture; 0 = the old behaviour: the stand-in cube at the level of his own light).
    pub env: f32,
    /// `r.only`: draw this one material slot (-1 = all).
    pub only: i32,
    /// `r.lag`: use the CAMERA of the host's frame of this many frames ago, when the host gave no
    /// camera source or with `r.present` (the game latches its render camera AFTER the phase our
    /// host submits in, so a submitted camera is the previous scene's: IMPL_ER2_STATE.md step 3).
    pub lag: u32,
    /// `r.poselag`: use the POSE of this many frames ago (0: the scene the game finishes in a
    /// frame is that frame's; 1 for `r.present`, which draws into the previous one).
    pub pose_lag: u32,
    /// `r.livecam`: read the camera from the host's source at the moment of drawing.
    pub live_camera: bool,
    /// `r.present`: draw at Present (the first arrangement: the colour is then one picture older
    /// than the depth buffer, and the Steam overlay is under us) instead of right after the game's
    /// own submission that finishes the back buffer.
    pub at_present: bool,
    /// `r.at`: where in the game's frame he is drawn. `Scene` (default): into the HDR scene target
    /// right after the game's deferred stage, before its forward stage (water, effects), TAA,
    /// tone mapping and HUD. `Composite`: into the back buffer after the game's final composite
    /// (over effects and HUD; our own tone curve). `r.present = 1` overrides both.
    pub at: At,
    /// `r.hdr`: what scene-linear 1.0 of ours becomes in the game's HDR scene target (its exposure
    /// is not read yet: STAND-IN, tuned by eye).
    pub hdr_scale: f32,
    /// `r.survey`: log the render targets and depth targets of this many frames.
    pub survey: u32,
    /// `r.cbcap` (discovery): a change of the number writes one frame of the game's bindings and
    /// constant buffers to the dump folder; `r.shscan`: a change scans memory for the game's shaders.
    pub cbcap: u32,
    pub shscan: u32,
    /// `r.bias`: metres a pixel of ours may lie behind the scene depth (copied-depth mode).
    pub bias: f32,
    /// `r.light` (default 1; scene arrangement only): his light is the GAME's (light.rs): its
    /// directional light's direction and colour, the light on the ground at his feet, its shadow
    /// mask there; `r.key` / `r.ambient` scale the two parts. 0 = the stand-in rig with `r.hdr`.
    pub light: bool,
    /// `r.jitter` (default 1): his vertices go through the game's own view-projection of the
    /// frame (its temporal anti-aliasing jitter included) instead of one rebuilt from the camera.
    pub jitter: bool,
    /// `r.velocity` (default 1): his motion is written to the game's velocity target, in its own
    /// encoding, right after the game's velocity pass (0 = not; -1 = with the sign turned round, for testing).
    pub velocity: i32,
    /// `r.shadow`: strength of the contact shadow under him (0 = none).
    pub shadow: f32,
    /// `r.shadow_sun`: strength of the shadow he throws from the game's directional light (a
    /// capsule's shadow: STAND-IN for his own silhouette in the game's shadow maps); 0 = none.
    pub shadow_sun: f32,
    /// `r.ghost`: opacity of the masked slots (nearly all of Sora), to see what is behind him
    /// (the debug skeleton); 1 = as the materials say.
    pub ghost: f32,
}

/// See `Options::at`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum At {
    Scene,
    Composite,
}

impl Default for Options {
    fn default() -> Options {
        Options { draw: true, depth: 1, fov_horizontal: false, front_ccw: true, cull: true, flat: 0, debug: 0.0, khs: kh_look::KHS_WORLD_PRESET.to_string(), exposure: kh_look::stand_in::EXPOSURE, key: 1.0, ambient: 1.0, env: 1.0, only: -1, lag: 0, pose_lag: 0, live_camera: true, at_present: false, at: At::Scene, hdr_scale: 1.0, survey: 0, cbcap: 0, shscan: 0, bias: 0.02, light: true, jitter: true, velocity: 1, shadow: 0.45, shadow_sun: 1.0, ghost: 1.0 }
    }
}

/// (IMPL_ER15, game over) THE VOID of KH3`s game-over screen: while the host keeps handing one over, the WHOLE back
/// buffer is cleared to black at Present and only this is drawn into it (the centred 16:9 rectangle), with a depth
/// buffer of our own: the posed Sora under the given point lights (no key light, no ambient, no reflection), our own
/// tone curve. The 2D overlay`s layers and plate lie over it. Nothing of his is drawn into the game`s own picture
/// meanwhile.
#[derive(Clone, Debug)]
pub struct VoidFrame {
    /// Camera (host space; `fov` vertical over the 16:9 rectangle), origin, clock and the body`s pose (no weapon).
    pub input: FrameInput,
    /// Game space, cm.
    pub lights: Vec<kh_look::PointLight>,
    /// The factor in front of the tone curve, and the post-process preset of the materials.
    pub exposure: f32,
    pub khs: String,
}

static VOID: Mutex<Option<(Arc<VoidFrame>, std::time::Instant)>> = Mutex::new(None);

/// (IMPL_ER15) The void of this frame (`None`: off at once). It lapses by itself half a second after the latest call.
pub fn set_void(frame: Option<VoidFrame>) {
    if let Ok(mut guard) = VOID.lock() {
        *guard = frame.map(|f| (Arc::new(f), std::time::Instant::now()));
    }
}

pub(crate) fn void_now() -> Option<Arc<VoidFrame>> {
    let guard = VOID.lock().ok()?;
    guard.as_ref().filter(|(_, at)| at.elapsed() < std::time::Duration::from_millis(500)).map(|(f, _)| f.clone())
}

/// (IMPL_ER21) THE CAMP MENU'S PORTRAIT: while the host keeps handing one over, the posed Sora is drawn at Present into
/// a colour + depth target of our own (gfx/portrait.rs) and the 2D overlay lays that picture into the back buffer
/// at `visible`, right before its layer `before_layer` (over the camp menu's picture, under its dialogs). Nothing of
/// the game's frame is read or written for it; nothing is done while none is handed over.
#[derive(Clone, Debug)]
pub struct PortraitFrame {
    /// The capture's camera (host space; `fov` vertical, `aspect` of the WHOLE capture), origin, clock, the body's pose.
    pub input: FrameInput,
    /// The UI actor's lights: two spots, the ambient band (game space, cm). Its `env` is NOT used: the renderer
    /// has one reflection cube (the stand-in sky); `env_level` scales it.
    pub rig: kh_look::Rig,
    pub env_level: f32,
    /// The factor in front of the tone curve, and the post-process preset of the materials.
    pub exposure: f32,
    pub khs: String,
    /// The part of the picture that shows: x, y, width, height in back buffer pixels (the target's size).
    pub visible: (i32, i32, u32, u32),
    /// Where the WHOLE capture lies in the back buffer: x, y, width, height (it may stick out of `visible`).
    pub picture: (f32, f32, f32, f32),
    /// The picture's opacity (the panel's open / close animation).
    pub alpha: f32,
    pub before_layer: usize,
    /// (IMPL_ER23B) The overlay layer of the camp menu's own picture: the portrait is only drawn while that layer is.
    pub with_layer: usize,
    /// What the target is cleared to (display-encoded; alpha 0).
    pub clear: [f32; 4],
}

static PORTRAIT: Mutex<Option<(Arc<PortraitFrame>, std::time::Instant)>> = Mutex::new(None);

/// (IMPL_ER21) The portrait of this frame (`None`: off at once). It lapses by itself half a second after the latest call.
pub fn set_portrait(frame: Option<PortraitFrame>) {
    let off = frame.is_none();
    if let Ok(mut guard) = PORTRAIT.lock() {
        *guard = frame.map(|f| (Arc::new(f), std::time::Instant::now()));
    }
    // (IMPL_ER23B) off = OFF AT ONCE: the overlay's copy of the picture goes in this very call (it used to stay half a second)
    if off {
        hud::set_picture(None);
    }
}

pub(crate) fn portrait_now() -> Option<Arc<PortraitFrame>> {
    let guard = PORTRAIT.lock().ok()?;
    // (IMPL_ER23B) 150 ms (was 500): a host that stops handing portraits over loses the picture within a few frames
    guard.as_ref().filter(|(_, at)| at.elapsed() < std::time::Duration::from_millis(150)).map(|(f, _)| f.clone())
}

/// (IMPL_ER23B) THE WEAR FORM the pawn has applied (`kh_core` `combat.form.wear_form`: 0 NORMAL, 5 Second Form): the
/// character's materials slide into that form's costume (gfx.rs, `kh_look::library::Library::step_wear`). The host
/// sets it whenever it likes (every frame is fine); it stays until set again.
static WEAR_FORM: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

pub fn set_wear_form(form: u8) {
    WEAR_FORM.store(form, Ordering::Relaxed);
}

pub(crate) fn wear_form() -> u8 {
    WEAR_FORM.load(Ordering::Relaxed)
}

/// (ER69) KH1 Sora's Second Form look: 1 = the styled one (regions, trim), 0 = the plain recolour.
static FORM_STYLE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(1);

pub fn set_form_style(style: u8) {
    FORM_STYLE.store(style, Ordering::Relaxed);
}

pub(crate) fn form_style() -> u8 {
    FORM_STYLE.load(Ordering::Relaxed)
}

/// (IMPL_ER59) SORA'S OUTFIT: "kh3" / "" = the main pack, else a folder `PACK/outfits/<id>` (tools/sora_outfit_pack.js).
/// Loaded on demand by the worker while the drawn one stays; set it whenever (every frame is fine). outfit.rs.
pub fn set_outfit(id: &str) {
    outfit::set_wanted(id);
}

/// (IMPL_ER59) The outfit drawn now ("" = the main pack) and the one wanted.
pub fn outfit_state() -> (String, String) {
    (outfit::loaded(), outfit::wanted())
}

pub type LogFn = fn(&str);
/// The host's camera as it is at the moment of the call (any thread).
pub type CameraFn = fn() -> Option<Camera>;

static LOG: AtomicUsize = AtomicUsize::new(0);
static CAMERA: AtomicUsize = AtomicUsize::new(0);
/// Set after a panic or at shutdown: every hook passes straight through from then on.
pub(crate) static DEAD: AtomicBool = AtomicBool::new(false);
static INSTALLED: AtomicBool = AtomicBool::new(false);
/// A shader scan was asked for (the worker runs it).
pub(crate) static SCAN: AtomicBool = AtomicBool::new(false);
/// Where discovery output goes (the host's log folder).
pub(crate) static DUMP_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Where the discovery tools (`r.cbcap`, `r.shscan`) write.
pub fn set_dump_dir(dir: &Path) {
    *DUMP_DIR.lock().unwrap() = Some(dir.to_path_buf());
}

pub(crate) struct Shared {
    pub pack: PathBuf,
    pub options: Options,
    /// The latest frames of the host, newest last (a few are kept for `r.lag`), each with when
    /// and on which thread it was submitted.
    pub inputs: Vec<(Arc<FrameInput>, std::time::Instant, u32)>,
    /// The host has handed over its switches once. ONE-SHOT KEYS (survey, cbcap, shscan) are only
    /// carried out on a CHANGE after that first hand-over: a value that is already in the control
    /// file when a payload starts must not fire (a hot reload under a player: IMPL_ER3_STATE.md).
    pub primed: bool,
}

pub(crate) static SHARED: Mutex<Option<Shared>> = Mutex::new(None);

pub(crate) fn log(text: &str) {
    let f = LOG.load(Ordering::Relaxed);
    if f != 0 {
        // SAFETY: only ever stored from a `LogFn`.
        let f: LogFn = unsafe { std::mem::transmute(f) };
        f(text);
    }
}

/// The camera to draw with: read from the host NOW when it gave a source (the game latches its
/// render camera after the phase our host submits in: see `Options::lag`).
pub(crate) fn camera_now() -> Option<Camera> {
    let f = CAMERA.load(Ordering::Relaxed);
    if f == 0 {
        return None;
    }
    // SAFETY: only ever stored from a `CameraFn`.
    let f: CameraFn = unsafe { std::mem::transmute(f) };
    f()
}

/// Gives the renderer a way to read the host's camera at the moment it draws.
pub fn set_camera_source(source: CameraFn) {
    CAMERA.store(source as *const () as usize, Ordering::SeqCst);
}

macro_rules! rlog {
    ($($arg:tt)*) => { $crate::log(&format!($($arg)*)) };
}
pub(crate) use rlog;

/// Starts the renderer: hooks the swap chain and the command queue, loads the pack's models,
/// textures and shaders in the background. `pack`: the Sora pack's folder. Returns at once.
pub fn install(pack: &Path, log_fn: LogFn) {
    if INSTALLED.swap(true, Ordering::SeqCst) {
        return;
    }
    LOG.store(log_fn as usize, Ordering::SeqCst);
    DEAD.store(false, Ordering::SeqCst);
    *SHARED.lock().unwrap() = Some(Shared { pack: pack.to_path_buf(), options: Options::default(), inputs: Vec::new(), primed: false });
    cbcap::WATCHING.store(1, Ordering::SeqCst);
    std::thread::spawn(|| {
        if std::panic::catch_unwind(gfx::worker).is_err() {
            DEAD.store(true, Ordering::SeqCst);
            rlog!("render: PANIC in the worker thread; the renderer is inert");
        }
    });
}

/// The host's frame: call once per game frame, after the camera moved.
pub fn submit(input: FrameInput) {
    if let Ok(mut guard) = SHARED.lock() {
        if let Some(shared) = guard.as_mut() {
            // SAFETY: plain getter.
            let thread = unsafe { windows::Win32::System::Threading::GetCurrentThreadId() };
            shared.inputs.push((Arc::new(input), std::time::Instant::now(), thread));
            let extra = shared.inputs.len().saturating_sub(4);
            shared.inputs.drain(..extra);
        }
    }
}

/// Nothing is drawn until the host submits again (the player is gone, a loading screen).
pub fn clear() {
    if let Ok(mut guard) = SHARED.lock() {
        if let Some(shared) = guard.as_mut() {
            shared.inputs.clear();
        }
    }
}

/// (IMPL_ER17) `r.shadow_real`: Sora's skinned mesh and the Keyblade are drawn depth-only into the game's cascaded shadow
/// atlas (each cascade with the game's own light matrices), so he casts a true shadow that the game filters as any other.
pub fn set_real_shadow(on: bool) {
    gfx::set_cast(on);
}

/// (IMPL_ER78) The hybrid's keys (`r.hybrid*`, hybrid.rs `Settings`): the host hands them over every second.
pub fn set_hybrid(settings: hybrid::Settings) {
    hybrid::set(settings);
    gfx::refresh_cast();
}

/// (IMPL_ER78) `r.hybrid_dump`: one dump of the game's G-buffer and light targets around him at the next hybrid frame.
pub fn hybrid_dump() {
    hybrid::request_dump();
}

/// (IMPL_ER53) `r.early_depth` (default on): his depth goes into the game's scene depth inside its deferred stage, right
/// after the G-buffer (gfx.rs `early_pass`), so that what the game makes from its depth later knows he is there.
pub fn set_early_depth(on: bool) {
    gfx::set_early(on);
}

/// (RSF2 round 14) `r.effects_depth` (default 1): the KH characters' depth into the game's half-size effect depth (its fire,
/// embers and glows behind them are hidden); 0 = as before; 2 = a test (every half-size effect hidden).
/// (RSF2 round 16, testing aid) `r.party_pin_test` (metres; 0 = off): the first friend drawn that far from Sora along the sun.
pub fn set_party_pin_test(metres: f32) {
    #[cfg(feature = "party")]
    if let Ok(mut m) = gfx::party::PIN_METRES.lock() {
        *m = if metres.is_finite() { metres.clamp(-20.0, 20.0) } else { 0.0 };
    }
    #[cfg(not(feature = "party"))]
    let _ = metres;
}

pub fn set_effects_depth(mode: u32) {
    gfx::set_effects_depth(mode);
}

/// (IMPL_ER56) `r.early_stencil` (default on) / `r.stencil_ref` (-1 = the ground's at his feet, else 0..255): the stencil
/// his early depth writes (gfx.rs).
pub fn set_early_stencil(on: bool, fixed: i32) {
    gfx::set_stencil(on, fixed);
}

/// (ER88) `r.taa_char` (default on): his stencil is the value Elden Ring's own characters carry (0x50: the game's TAA treats
/// him as a character); off = the ground's at his feet (IMPL_ER56).
pub fn set_taa_char(on: bool) {
    gfx::set_taa_char(on);
}

/// (IMPL_ER65) `r.late_sora` (default on) / `r.late_sora_far` (m, default 100; 0 = wherever he is): the late re-composite
/// of his colour after the game's forward stage (gfx.rs).
pub fn set_late_sora(on: bool, far_m: f32) {
    gfx::set_late(on, far_m);
}

/// (IMPL_ER80b) `r.shaft_mask` (default on): the game's light-shaft composite is drawn with a stencil test against his mask
/// (shaft.rs): the radial blur of the sky beside him never adds over his pixels. Off = as before.
pub fn set_shaft_mask(on: bool) {
    shaft::set_wanted(on);
}

/// (IMPL_ER80) `r.late_sora_taa` (default on): the late re-composite (`r.late_sora 1`) is drawn at the game's velocity split,
/// BEFORE its temporal anti-aliasing (recomp2.ps.dxil, a smooth far weight); off = after the forward submission (= after the
/// TAA) with recomp.ps.dxil, as IMPL_ER65 (its hair flicker).
pub fn set_late_sora_taa(on: bool) {
    gfx::set_late_taa(on);
}

/// (IMPL_ER80) `r.glow_mask` (default on): his opaque / masked pixels go out of the game's glow source (the deferred
/// composite's second target, blurred and added over the picture by the post chain) right after his colour draw, so that
/// the glow of what lies behind him (the Erdtree) is no longer added over him (gfx.rs `GlowKit`). Off = as before.
pub fn set_glow_mask(on: bool) {
    gfx::set_glow_mask(on);
}

/// (FP, offline harness only: `effects --upload-kb N`) the character pipeline's per-frame upload buffer capped at `bytes`
/// (0 = its full 2 MB), to show that a routed chandelier fragment that does not fit skips itself and never fails the frame.
pub fn set_upload_test_cap(bytes: usize) {
    gfx::set_upload_test_cap(bytes);
}

/// (IMPL_ER78, offline harness only) the resource the survey takes as the game's diffuse light target; the hybrid then
/// runs without the game's light constants (the stand-in rig's level).
pub fn test_hybrid_light(resource: usize) {
    survey::TEST_LIGHT.store(resource, std::sync::atomic::Ordering::Relaxed);
    hybrid::TEST.store(resource != 0, std::sync::atomic::Ordering::Relaxed);
}

/// (HY, offline harness only) the resource the survey takes as the game's screen-space shadow mask (0 = none).
pub fn test_hybrid_mask(resource: usize) {
    survey::TEST_MASK.store(resource, std::sync::atomic::Ordering::Relaxed);
}

/// (HY, offline harness only) the capsules of everybody the renderer drew in its last frame (Sora's body and Keyblade, his
/// form weapons' balls, the friends'), game space (cm): the harness's light pass casts and occludes with them.
pub fn test_capsules() -> Vec<(glam::Vec3, glam::Vec3, f32)> {
    hybrid::TEST_CAPS.lock().map(|c| c.clone()).unwrap_or_default()
}

/// (IMPL_ER65, offline harness only) the resource the survey takes as the game's velocity target.
pub fn test_velocity_target(resource: usize) {
    survey::TEST_VELOCITY.store(resource, std::sync::atomic::Ordering::Relaxed);
}

/// (testing aid) `r.shadow_bias`: see gfx.rs `CAST_PUSH_BIAS` (takes effect at the next payload start).
pub fn debug_shadow_bias(n: i32) {
    gfx::CAST_PUSH_BIAS.store(n, std::sync::atomic::Ordering::Relaxed);
}

/// (testing aid) `r.shadow_dbg = n`: see gfx.rs `CAST_DEBUG`.
pub fn debug_real_shadow(pass: u32) {
    gfx::CAST_FLOOD.store(pass == 99, std::sync::atomic::Ordering::Relaxed);
    gfx::CAST_PUSH.store(pass == 98, std::sync::atomic::Ordering::Relaxed);
    if pass < 98 {
        gfx::CAST_DEBUG.store(pass, std::sync::atomic::Ordering::Relaxed);
    }
}

pub fn set_options(options: Options) {
    if let Ok(mut guard) = SHARED.lock() {
        if let Some(shared) = guard.as_mut() {
            if shared.options != options {
                rlog!("render: options {options:?}");
                let primed = std::mem::replace(&mut shared.primed, true);
                if !primed && (options.survey > 0 || options.cbcap > 0 || options.shscan > 0) {
                    rlog!("render: one-shot keys present at the start (r.survey {}, r.cbcap {}, r.shscan {}) are NOT carried out: change the value to fire", options.survey, options.cbcap, options.shscan);
                }
                if primed && options.survey != shared.options.survey && options.survey > 0 {
                    survey::request(options.survey);
                }
                if primed && options.cbcap != shared.options.cbcap && options.cbcap > 0 {
                    cbcap::request();
                }
                if primed && options.shscan != shared.options.shscan && options.shscan > 0 {
                    SCAN.store(true, Ordering::SeqCst);
                }
                cbcap::WATCHING.store(u32::from(options.light || options.jitter), Ordering::SeqCst);
                shared.options = options;
            }
            shared.primed = true;
        }
    }
}

/// (TITLEPERF) Our time in the game's Present and ExecuteCommandLists since the last call:
/// ((Present ns, Present calls), (submission survey ns, submissions)); the counters start again.
pub fn hook_perf() -> ((u64, u64), (u64, u64)) {
    hook::take_perf()
}

/// (TITLEPERF) The game's adapter's local video memory (in use, budget) in bytes, as last read at Present.
pub fn video_memory() -> Option<(u64, u64)> {
    hook::video_memory()
}

/// One line for the host's once-a-second log; resets the counters it reports.
pub fn status() -> String {
    // (effects) the effect drawing's line follows the renderer's
    format!("{}
  {}
  {}
  {}
  {}", gfx::status(), vfx::status(), pointlight::status(), stable::status(), taa::status()) + &format!("\n  {}", passtime::status()) /* (HAIRPERF) */ + &format!("\n  {}", camfade::status()) /* (NF) */ + &format!("\n  {}", finale::status()) /* (ZFIN) */ + &format!("
  {}", zdraw::status()) // (ZDRAW)
}

/// An inline hook of the host's own on any function (detour.rs: prologue decoder, relay and
/// trampoline; refuses what it cannot decode). It is undone by [`shutdown`] together with the
/// renderer's hooks. `original` receives the trampoline BEFORE the jump goes in.
///
/// # Safety
/// `target` must be the start of a function and `hook` a function of its exact signature that
/// stays loaded for the life of the process.
pub unsafe fn inline_hook(target: usize, hook: usize, original: &AtomicUsize, what: &'static str) -> Result<(), String> {
    detour::install(target, hook, original, what)
}

/// Puts the vtable entries back and frees what was made. The DLL may stay loaded afterwards.
pub fn shutdown() {
    if !INSTALLED.swap(false, Ordering::SeqCst) {
        return;
    }
    DEAD.store(true, Ordering::SeqCst);
    hook::remove();
    gfx::release();
    *SHARED.lock().unwrap() = None;
    rlog!("render: shut down (hooks removed)");
}
