//! (ZFIN) ZETTAFLARE'S FINALE ON AN ELDEN RING CHARACTER: the character draw route ZETTA's finale state asks for
//! (kh-core `Targets::script_finale`, kh-er targets.rs / zfinale.rs; build/sora_tmp/party/IMPL_ZETTA_STATE.md 00.6). The
//! user: "the enemy to get immersed in the white light, with a black silhouette that crumbles leaving only the white light
//! ... universally on any enemy"; "a dynamic system that just works" on any enemy: NO per-enemy data.
//!
//! THE ROUTE (UNTESTED IN GAME; proven offline in examples/effects.rs `--erchars` / `--finale`): the game draws the
//! character itself (its own draws are never touched); after our effects, screen passes of ours (er_char.wgsl ER_FINALE,
//! materials/er/finale*.ps.dxil) find ITS pixels in the game's own buffers and paint them:
//! - THE PROBE (finale_probe.ps.dxil, one pixel per target of an 8 x 1 memory kept from frame to frame): ITS OWN STENCIL VALUE
//!   in the snapshot of the scene depth taken at the early split (gfx.rs, IMPL_ER53: the G-buffer's depth AND stencil, before
//!   any KH character is in it), learnt at run time where the snapshot's depth lies inside its capsule (8 heights x 3 across
//!   the view), and the ground's value around its feet. Nothing is assumed: a boss of another class is found by its own value
//!   (measured so far: characters 0x50, terrain 0x12 / 0xd2 / 0x90, map objects 0x10, sky 0). The value learnt is kept while
//!   no new one is seen (it fell over, something covers it).
//! - a pixel is its when the snapshot's stencil has that value's upper four bits (and, when that value is the ground's, it
//!   stands 8 cm above its feet); the scene depth now is not nearer (Sora, Donald, Goofy, an opaque effect are never painted);
//!   the position it gives lies within its reach (wide enough for its death fall) and nearer its capsule than any neighbour's.
//! The look (ZETTA's state, kh-app fixtures `zetta_look` in the test level): `silhouette` = pure black, unlit; `crumble`
//! 0..1 = broken away from the top down along a noise fixed to its body (seeded by the host's id of it: no state field), a
//! glowing rim, white light where it is gone; otherwise `immerse` 0..1 = engulfed in white.
//! THE BREAKING APART (round 3, the user: "it needs to look like the silhouette is actually breaking apart"; kh-app
//! zetta_crumble.rs mirrored: `timeline`, `crack_reveal`, `fly` below and their shader twins): while it is a silhouette not yet
//! crumbling, a SNAPSHOT of its own pixels (finale_snap.ps.dxil: each texel's place relative to its feet and its footprint)
//! is kept; from its first crumble the host hides its body (`breaks_apart`, kh-er zfinale.rs) and finale_frag.vs/ps draw the
//! snapshot as camera-facing splats at their places NOW (a moving camera sees them stay in the world): Voronoi cells of a
//! jittered lattice fixed to its body (seeded by its id), glowing cracks along the cells' borders (0 .. 0.2, from the side the
//! beam strikes), the cells released top-first (0.16 .. 0.66, 70 % height / 30 % noise) on `push`, tumbling under gravity,
//! landing, each splitting into three fragments 0.1 later, which shrink away (about 0.82 .. 1) shedding ash and embers.
//! Nothing at 1. Without a snapshot (its value never learnt) the earlier look stays (the noise dissolve, the fade below).
//! THE FADE: once it has crumbled the host hides its body (`hidden_for`); the last white shape (written into a ghost target by
//! finale_g.ps.dxil while it crumbles) fades out over `r.finale_fade` seconds (default 0.3) over the game's own picture.
//! THE PARTICLES OVER IT: the effects whose material name holds one of `r.finale_over`'s words (kh-er sheet / ctl; vfx.rs
//! `Stage::OverFinale`) are held back while a finale is drawn and drawn after it: ZETTA's crumble fragments stay visible over
//! the black.
//! FALLBACK (graceful): when its value has never been seen (no sample of it yet), no snapshot this frame, a depth without
//! stencil, `r.finale 2`, or none of the taps over its box finds it (decided on the GPU, every pixel alike), the pass draws a
//! WHITE-OUT of its screen box (an ellipse behind whatever is nearer than its front) that dissolves with `crumble`.
//! Nothing is drawn without a fresh finale state (`set` every frame; older than 0.5 s = gone), so any exit of the finale
//! leaves the picture as the game's. Keys (kh-er frame.rs): `r.finale` 0 off, 1 auto (default), 2 the white-out only, 3 the
//! mask as a debug view; `r.finale_white` the white (x the effects' scale, default 4); `r.finale_fade` (s); `r.finale_over`.

use std::ffi::c_void;
use std::mem::ManuallyDrop;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use glam::{Mat4, Vec3, Vec4};
use windows::core::Interface;
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;

use crate::rlog;

/// Targets drawn in one frame (Zettaflare has one; the rest are left out). Also the width of the probe's memory.
pub const MAX_TARGETS: usize = 4;
const MEMORY_WIDTH: u32 = 8;
/// Neighbours per target the shader keeps apart from it (nearest first).
pub const MAX_NEIGHBOURS: usize = 8;
/// Rows of the shader's block (er_char.wgsl `FinaleBlock`).
pub const ROWS: usize = 44;
const BLOCK_BYTES: usize = 768;
/// (round 3) The snapshot of the silhouette (er_char.wgsl finale_snap_ps): at most this many texels a side.
pub const ATLAS: u32 = 256;
/// The crumble's seconds (ZETTA's `zetta.finale_crumble_time`, kh-app zetta_crumble.rs SECS): the pieces' flights' clock.
pub const CRUMBLE_SECS: f32 = 2.2;
/// gfx.rs FRAME_SLOTS / SLOT_TEXTURES (the root signature's table t0..t22).
const SLOTS: usize = 3;
const TABLE: usize = 23;
/// A finale state older than this is gone (the host sends it every frame while it lasts).
const STALE: Duration = Duration::from_millis(500);
/// From this crumble on, the white shape is kept for its fade.
const GHOST_FROM: f32 = 0.8;

/// A standing character's capsule (game space, cm): its feet, radius and half height (the host's row of it).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Capsule {
    pub feet: Vec3,
    pub radius: f32,
    pub half_height: f32,
}

/// One target's finale this frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FinaleTarget {
    /// The host's id of it (the noise's seed, the memory of its stencil value, the log).
    pub id: u32,
    pub body: Capsule,
    /// ZETTA's look: engulfed in white (0..1), the black silhouette, crumbled away (0..1).
    pub immerse: f32,
    pub silhouette: bool,
    pub crumble: f32,
    /// The other characters near it (any team): their pixels are never its.
    pub neighbours: Vec<Capsule>,
    /// Its body hidden by the host since this many seconds (after the crumble): its last white shape fades.
    pub hidden_for: Option<f32>,
    /// (round 3) ZETTA's `ScriptFinale::push`: the beam's direction at it (unit, game space; zero: none known), the pieces'
    /// scatter.
    pub push: Vec3,
    /// (round 6) ZETTA's `ScriptFinale::dark`: how far the silhouette has turned from what the body showed to its full look
    /// (0..1, eased): its fill, rim and ragged outline grow in by it, never in one step.
    pub dark: f32,
}

struct Book {
    targets: Vec<FinaleTarget>,
    at: Instant,
}

static BOOK: Mutex<Option<Book>> = Mutex::new(None);
/// `r.finale` (1 = auto), `r.finale_white` and `r.finale_fade` (f32 bits).
static MODE: AtomicU32 = AtomicU32::new(1);
static WHITE: AtomicU32 = AtomicU32::new(0x4080_0000); // 4.0
static FADE: AtomicU32 = AtomicU32::new(0x3E99_999A); // 0.3
/// `r.finale_over`: words of the effect materials drawn over the finale.
static OVER: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// (for the log) frames drawn by the isolating route / by the white-out (host-side decision), fades drawn.
static FRAMES_ISOLATE: AtomicU64 = AtomicU64::new(0);
static FRAMES_WHITE_OUT: AtomicU64 = AtomicU64::new(0);
static FRAMES_FADE: AtomicU64 = AtomicU64::new(0);
static FRAMES_BREAK: AtomicU64 = AtomicU64::new(0);
/// (round 3) The target whose silhouette breaks apart from the snapshot (its stencil value learnt, the snapshot kept): the
/// host hides its body at the first crumble (kh-er zfinale.rs), the renderer draws its pieces.
static BREAKING: Mutex<Option<u32>> = Mutex::new(None);
/// The ids whose stencil value the probe had learnt (read back from the GPU a few frames late).
static LEARNT: Mutex<Vec<u32>> = Mutex::new(Vec::new());
/// (round 7) what the probe had learnt of each (id, stencil value, samples), for the log
static LEARNT_VALUES: Mutex<Vec<(u32, f32, f32)>> = Mutex::new(Vec::new());
/// (ZDRAW) the ids whose learnt class is the ground's: the screen cannot isolate them (er_char.wgsl fz_tap takes none of
/// their pixels), so they never break apart from a snapshot (their body stays as the game draws it until the crumble ends)
static LEARNT_GROUND: Mutex<Vec<u32>> = Mutex::new(Vec::new());

/// (round 3) Will target `id` break apart from its snapshot? (the host hides its body from its first crumble; else at 1)
pub fn breaks_apart(id: u32) -> bool {
    BREAKING.lock().ok().is_some_and(|b| *b == Some(id)) || crate::zdraw::owns(id) // (ZDRAW) its own model breaks apart
}

/// The host, every frame while a finale lasts: the targets and their looks (empty: none).
pub fn set(targets: Vec<FinaleTarget>) {
    if let Ok(mut book) = BOOK.lock() {
        *book = if targets.is_empty() { None } else { Some(Book { targets, at: Instant::now() }) };
    }
}

/// `r.finale` (0 off, 1 auto, 2 the white-out only, 3 the mask), `r.finale_white` (<= 0: the default 4), `r.finale_fade`
/// (seconds; < 0: the default 0.3; 0: no fade).
pub fn set_mode(mode: u32, white: f32, fade: f32) {
    MODE.store(mode.min(3), Ordering::Relaxed);
    WHITE.store(if white > 0.0 { white } else { 4.0 }.to_bits(), Ordering::Relaxed);
    FADE.store(if fade >= 0.0 { fade } else { 0.3 }.to_bits(), Ordering::Relaxed);
}

/// `r.finale_over`: the effects whose material name holds one of these words are drawn over the finale (ZETTA's fragments).
pub fn set_over(words: Vec<String>) {
    if let Ok(mut o) = OVER.lock() {
        *o = words.into_iter().map(|w| w.trim().to_ascii_lowercase()).filter(|w| !w.is_empty()).collect();
    }
}

/// Is an effect material drawn over the finale?
pub fn over_material(name: &str, words: &[String]) -> bool {
    let name = name.to_ascii_lowercase();
    words.iter().any(|w| name.contains(w.as_str()))
}

pub(crate) fn over_words() -> Vec<String> {
    OVER.lock().map(|o| o.clone()).unwrap_or_default()
}

/// The targets to draw now (a state older than 0.5 s is gone).
pub fn current() -> Vec<FinaleTarget> {
    let Ok(book) = BOOK.lock() else { return Vec::new() };
    match book.as_ref() {
        Some(b) if b.at.elapsed() < STALE && MODE.load(Ordering::Relaxed) != 0 => b.targets.iter().take(MAX_TARGETS).cloned().collect(),
        _ => Vec::new(),
    }
}

/// A finale is drawn this frame (vfx.rs holds the effects over it back for `Stage::OverFinale`).
pub fn active() -> bool {
    !current().is_empty()
}

pub fn status() -> String {
    format!("finale (ZFIN): mode {}, frames drawn {} isolating / {} white-out / {} fading / {} breaking apart", MODE.load(Ordering::Relaxed), FRAMES_ISOLATE.load(Ordering::Relaxed), FRAMES_WHITE_OUT.load(Ordering::Relaxed), FRAMES_FADE.load(Ordering::Relaxed), FRAMES_BREAK.load(Ordering::Relaxed))
}

// ------------------------------------------------------------------------------------------------ the pure part

/// How far around its feet its pixels may lie (cm, horizontal) and how high (cm above its feet): wide enough for its death
/// fall (a body lies up to its height from its root) and raised arms or weapons.
pub fn reach(body: &Capsule) -> (f32, f32) {
    let height = 2.0 * body.half_height.max(body.radius).max(1.0);
    ((body.radius * 2.0 + 40.0).max(height * 1.05 + 20.0).max(body.radius * 3.5), height * 1.3 + 40.0)
}

/// The screen box (pixels: x0, y0, x1, y1) of a box around `feet` (horizontal half size, from `below` under its feet to `top`
/// over them). A box wholly behind the camera: `None`. One that crosses the camera plane: the whole target when `crossing`
/// is true, else the box of its corners in front (the white-out's ellipse never fills the screen for a corner behind).
/// `None` when it lies off the screen.
#[allow(clippy::too_many_arguments)]
pub fn screen_box(clip: &Mat4, feet: Vec3, horizontal: f32, below: f32, top: f32, width: u32, height: u32, crossing: bool) -> Option<[i32; 4]> {
    let (w, h) = (width as f32, height as f32);
    let (mut lo, mut hi) = (glam::Vec2::splat(f32::MAX), glam::Vec2::splat(f32::MIN));
    let mut behind = 0;
    for k in 0..8 {
        let p = feet + Vec3::new(if k & 1 == 0 { -horizontal } else { horizontal }, if k & 2 == 0 { -horizontal } else { horizontal }, if k & 4 == 0 { -below } else { top });
        let c = *clip * p.extend(1.0);
        if c.w <= 1e-3 {
            behind += 1;
            continue;
        }
        let s = glam::Vec2::new((c.x / c.w * 0.5 + 0.5) * w, (0.5 - c.y / c.w * 0.5) * h);
        lo = lo.min(s);
        hi = hi.max(s);
    }
    if behind == 8 {
        return None;
    }
    if behind > 0 && crossing {
        return Some([0, 0, width as i32, height as i32]);
    }
    let x0 = (lo.x - 2.0).floor().clamp(0.0, w) as i32;
    let y0 = (lo.y - 2.0).floor().clamp(0.0, h) as i32;
    let x1 = (hi.x + 2.0).ceil().clamp(0.0, w) as i32;
    let y1 = (hi.y + 2.0).ceil().clamp(0.0, h) as i32;
    (x1 > x0 && y1 > y0).then_some([x0, y0, x1, y1])
}

/// The view of the frame the pass needs (game space, cm; the scene depth's view depth in metres = 1 / (device x a + b)).
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub clip: Mat4,
    pub camera: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    pub forward: Vec3,
    pub depth_a: f32,
    pub depth_b: f32,
    /// Projection [0][0] = 1 / tan(half horizontal fov).
    pub proj00: f32,
    pub width: u32,
    pub height: u32,
}

/// The ghost's alpha of a hidden body (1 at the hide, 0 after `fade` seconds; 0 at once with no fade).
pub fn fade_alpha(hidden_for: f32, fade: f32) -> f32 {
    if fade <= 0.0 {
        return 0.0;
    }
    let x = (1.0 - hidden_for / fade).clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// One target's draw: its scissor box and the shader's block. `slot`: its pixel of the probe's memory; `isolate`: the
/// snapshot (depth + stencil) of this frame is bound; `live`: the scene depth now is bound. `None`: it is off the screen.
#[allow(clippy::too_many_arguments)]
pub fn plan(t: &FinaleTarget, view: &View, slot: usize, mode: u32, white: f32, fade: f32, isolate: bool, live: bool) -> Option<([i32; 4], [f32; ROWS * 4])> {
    let body = &t.body;
    let (horizontal, top) = reach(body);
    let scissor = screen_box(&view.clip, body.feet, horizontal, 40.0, top, view.width, view.height, true)?;
    let height = 2.0 * body.half_height.max(body.radius).max(1.0);
    let core = screen_box(&view.clip, body.feet, body.radius * 1.2 + 10.0, 10.0, height * 1.12 + 15.0, view.width, view.height, false).unwrap_or(scissor);
    let tan_x = 1.0 / view.proj00.max(1e-4);
    let tan_y = tan_x * view.height as f32 / view.width.max(1) as f32;
    let centre = body.feet + Vec3::Z * (height * 0.5);
    // (its front: what it holds out toward the camera, a weapon, stays under the white-out)
    let front_m = ((centre - view.camera).dot(view.forward) - (body.radius * 1.5 + 30.0).max(height * 0.6)) / 100.0;
    let mut f = [0.0f32; ROWS * 4];
    let mut row = |k: usize, v: Vec4| f[k * 4..k * 4 + 4].copy_from_slice(&v.to_array());
    row(0, view.camera.extend(mode as f32));
    row(1, view.right.extend(tan_x));
    row(2, view.up.extend(tan_y));
    row(3, view.forward.extend(white));
    row(4, Vec4::new(view.width as f32, view.height as f32, 1.0 / view.width.max(1) as f32, 1.0 / view.height.max(1) as f32));
    row(5, Vec4::new(view.depth_a, view.depth_b, if live { 1.0 } else { 0.0 }, if isolate { 1.0 } else { 0.0 }));
    row(6, body.feet.extend(body.radius));
    row(7, Vec4::new(body.half_height, t.immerse.clamp(0.0, 1.0), if t.silhouette { 1.0 } else { 0.0 }, t.crumble.clamp(0.0, 1.0)));
    let seed = ((t.id as f32 * 0.618_034).fract()) * 97.0 + 3.0;
    // (round 7, Malenia's extra rows at her very place) one standing on the target itself is no neighbour: its pixels are the target's
    let mut near: Vec<&Capsule> = t.neighbours.iter().filter(|n| (n.feet - body.feet).length() < horizontal + n.radius * 2.0 + 200.0 && Vec3::new(n.feet.x - body.feet.x, n.feet.y - body.feet.y, 0.0).length() >= 0.75 * n.radius.max(body.radius)).collect();
    near.sort_by(|a, b| (a.feet - body.feet).length_squared().total_cmp(&(b.feet - body.feet).length_squared()));
    near.truncate(MAX_NEIGHBOURS);
    row(8, Vec4::new(seed, near.len() as f32, horizontal, top));
    row(9, Vec4::new(core[0] as f32, core[1] as f32, core[2] as f32, core[3] as f32));
    row(10, Vec4::new(front_m, 2.5, 0.0, 0.0));
    for (k, n) in near.iter().enumerate() {
        row(11 + 2 * k, n.feet.extend(n.radius));
        row(12 + 2 * k, Vec4::new(n.half_height, 0.0, 0.0, 0.0));
    }
    for (k, c) in view.clip.to_cols_array_2d().iter().enumerate() {
        row(27 + k, Vec4::from_array(*c));
    }
    row(31, Vec4::new(slot as f32, t.hidden_for.map_or(1.0, |h| fade_alpha(h, fade)), 0.0, t.id as f32));
    Some((scissor, f))
}

/// (round 3) The snapshot's grid over a screen box: (step in pixels, columns, rows), at most ATLAS a side.
pub fn atlas_grid(b: [i32; 4]) -> (u32, u32, u32) {
    let (w, h) = ((b[2] - b[0]).max(1) as u32, (b[3] - b[1]).max(1) as u32);
    let step = w.max(h).div_ceil(ATLAS).max(1);
    (step, w.div_ceil(step).min(ATLAS), h.div_ceil(step).min(ATLAS))
}

/// (round 3) What the snapshot of a silhouette keeps: whose, its grid and where it was taken.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snap {
    pub id: u32,
    pub x0: i32,
    pub y0: i32,
    pub step: u32,
    pub columns: u32,
    pub rows: u32,
    pub body: Capsule,
    /// (round 4) the camera's forward at the snapshot (game space): the pieces' thickness lies along it
    pub forward: Vec3,
}

/// (round 4) The layers a released piece is drawn in (er_char.wgsl FZ_LAYERS): its thickness.
pub const PIECE_LAYERS: u32 = 4;

/// (round 3) The block's rows 32..37 of the breaking apart (er_char.wgsl, the snapshot and its pieces).
pub fn snap_rows(f: &mut [f32; ROWS * 4], snap: &Snap, push: Vec3, white: f32) {
    let height = 2.0 * snap.body.half_height.max(snap.body.radius).max(1.0);
    let push = if push.length_squared() > 1e-6 { push.normalize() } else { Vec3::ZERO };
    let rows = [
        Vec4::new(snap.x0 as f32, snap.y0 as f32, snap.step as f32, snap.columns as f32),
        Vec4::new(snap.rows as f32, height / 9.0, height / 180.0, CRUMBLE_SECS),
        push.extend(snap.body.feet.z),
        snap.body.feet.extend(height),
        Vec4::new(white * 1.25, white * 1.05, white * 0.75, white * 1.5),
        snap.forward.normalize_or_zero().extend(0.0),
    ];
    for (k, r) in rows.iter().enumerate() {
        f[(32 + k) * 4..(32 + k) * 4 + 4].copy_from_slice(&r.to_array());
    }
}

/// (round 3) kh-app zetta_crumble.rs `timeline` (the shader's fz_timeline is its twin): a piece's (released, split, its
/// fragments' dissolve start and end) in crumble units; `height` its place up the body (0 feet .. 1 top), `r` / `r2` noise.
pub fn timeline(height: f32, r: f32, r2: f32) -> (f32, f32, f32, f32) {
    let release = 0.16 + 0.5 * (0.7 * (1.0 - height.clamp(0.0, 1.0)) + 0.3 * r.clamp(0.0, 1.0));
    let split = release + 0.1;
    let start = split + 0.06 + 0.08 * r2.clamp(0.0, 1.0);
    let end = (start + 0.18).min(1.0);
    (release, split, start.min(end - 0.02), end)
}

/// (round 3) kh-app zetta_crumble.rs `crack_reveal` (fz_crack_reveal): a crack's reveal from its distance to the struck side.
pub fn crack_reveal(dist: f32, r: f32) -> f32 {
    0.01 + 0.15 * dist.clamp(0.0, 1.0) + 0.03 * r.clamp(0.0, 1.0)
}

/// (round 3) kh-app zetta_crumble.rs `fly` (fz_fly) in game axes (metres, z up; (round 6) gravity 0.9: the beam carries the
/// pieces, they do not drop): place, velocity, landing time.
pub fn fly(p0: Vec3, v0: Vec3, floor: f32, t: f32) -> (Vec3, Vec3, f32) {
    let g = 0.9;
    let h = (p0.z - floor).max(0.0);
    let land = if h <= 1e-5 && v0.z <= 0.0 { 0.0 } else { (v0.z + (v0.z * v0.z + 2.0 * g * h).sqrt()) / g };
    if t <= land {
        return (p0 + v0 * t - Vec3::Z * (0.5 * g * t * t), v0 - Vec3::Z * (g * t), land);
    }
    let pc = p0 + v0 * land - Vec3::Z * (0.5 * g * land * land);
    let vh = Vec3::new(v0.x, v0.y, 0.0) * 0.45;
    let e = (-4.0 * (t - land)).exp();
    (Vec3::new(pc.x, pc.y, floor) + vh * ((1.0 - e) / 4.0), vh * e, land)
}

/// (round 5, the user's style reference) The beam's colour of the silhouette (display-linear, kh-app zetta_crumble.rs
/// `sil_overlay`): its deep core, its lighter edge, the light's pale colour (the rim, the cracks, what erodes).
pub const STYLE_CORE: Vec3 = Vec3::new(0.42, 0.04, 0.02);
pub const STYLE_EDGE: Vec3 = Vec3::new(0.95, 0.38, 0.18);
pub const STYLE_GLOW: Vec3 = Vec3::new(1.0, 0.9, 0.82);

/// (round 5) The rim band (pixels), at least a pixel and a half and at most 18 pixels of the test level's 960-wide mask
/// (scaled to the target's width).
/// (ZFIN round 10, the user's KILL_REF_1..3: the test level's band is 3.5 % of the silhouette's HEIGHT ON THE SCREEN, and its
/// target lies along the beam: that height is the body's thickness, a third of its length, so the rim is thin and every chunk
/// stays solid deep red with a thin pale rim while it streams away.) An Elden Ring target stands: 3.5 % of its height made the
/// band three times the reference's against its body and its chunks (pieces two bands wide came out pale: cov < 0.5 at their
/// centre). The band is now the reference's share of the BODY'S LENGTH (its height here): 3.5 % / 2.6 = 1.33 %, the same band
/// against the body, its cracks and its chunks (a cell is a ninth of the length) as in the reference.
pub const STYLE_BAND_SHARE: f32 = 0.035 / 2.6;

pub fn style_band(silhouette_px: f32, width: u32) -> f32 {
    let k = width.max(1) as f32 / 960.0;
    (silhouette_px * STYLE_BAND_SHARE).clamp(1.5 * k, 18.0 * k)
}

/// (round 5) The block's rows 38..42: the mask flag, the band, (round 6) how far it has darkened, the colours and the
/// light's scale.
pub fn style_rows(f: &mut [f32; ROWS * 4], band: f32, scale: f32, dark: f32) {
    let rows = [Vec4::new(1.0, band, dark.clamp(0.0, 1.0), 0.0), STYLE_CORE.extend(scale), STYLE_EDGE.extend(0.0), STYLE_GLOW.extend(0.0)];
    for (k, r) in rows.iter().enumerate() {
        f[(38 + k) * 4..(38 + k) * 4 + 4].copy_from_slice(&r.to_array());
    }
}

/// (round 6) kh-app zetta_crumble.rs `swept` (fz_swept): a flight swept on along the beam by its force (1.6 m/s^2 along
/// `sweep`), never under its floor: its place and landing time.
pub fn swept(p0: Vec3, v0: Vec3, sweep: Vec3, floor: f32, t: f32) -> (Vec3, f32) {
    let (p, _, land) = fly(p0, v0, floor, t);
    let p = p + sweep * (0.5 * 1.6 * t * t);
    (Vec3::new(p.x, p.y, p.z.max(floor)), land)
}

/// The views of a depth resource's format: (its depth plane, its stencil plane).
pub fn depth_views(format: DXGI_FORMAT) -> Option<(DXGI_FORMAT, Option<DXGI_FORMAT>)> {
    match format {
        DXGI_FORMAT_R32G8X24_TYPELESS | DXGI_FORMAT_D32_FLOAT_S8X24_UINT => Some((DXGI_FORMAT_R32_FLOAT_X8X24_TYPELESS, Some(DXGI_FORMAT_X32_TYPELESS_G8X24_UINT))),
        DXGI_FORMAT_R24G8_TYPELESS | DXGI_FORMAT_D24_UNORM_S8_UINT => Some((DXGI_FORMAT_R24_UNORM_X8_TYPELESS, Some(DXGI_FORMAT_X24_TYPELESS_G8_UINT))),
        DXGI_FORMAT_R32_TYPELESS | DXGI_FORMAT_D32_FLOAT => Some((DXGI_FORMAT_R32_FLOAT, None)),
        _ => None,
    }
}

// ------------------------------------------------------------------------------------------------ the GPU part

/// The pipelines of one scene format: the look (one target), the look + the ghost (two), the fade, the pieces.
struct Looks {
    format: i32,
    main: ID3D12PipelineState,
    ghost: ID3D12PipelineState,
    fade: ID3D12PipelineState,
    pieces: ID3D12PipelineState,
    /// (round 5) the same into the mask (R8G8B8A8, max blending), and the beam's colour over the picture from it
    main_mask: ID3D12PipelineState,
    pieces_mask: ID3D12PipelineState,
    comp: ID3D12PipelineState,
    /// (ZFIN round 9) the comp at the game's Present (display colours): the occlusion into the mask's blue, the full-screen
    /// vertex stage and the display pixel stage (None: no such files in the pack: the comp stays in the late pass)
    occl: Option<ID3D12PipelineState>,
    disp: Option<(Vec<u8>, Vec<u8>)>,
}

/// (ZFIN round 9) `r.finale_display` (default 1): the beam's colour over the silhouette is drawn at the game's Present, in
/// DISPLAY colours over the finished picture, as the test level draws it (a UI layer after its tone mapper and bloom): the test
/// level's colours exactly, whatever Elden Ring's tone curve, colour grading, exposure or bloom. 0: in the late pass through the
/// ACES fit's inverse, as before (also the fallback for a back buffer of another format, e.g. HDR).
static DISPLAY: AtomicBool = AtomicBool::new(true);
/// (ZFIN round 9) the display pass ran at the last Present (the late comp is left out while it does)
static DISPLAY_ARMED: AtomicBool = AtomicBool::new(false);

pub fn set_display(on: bool) {
    DISPLAY.store(on, Ordering::Relaxed);
    if !on {
        DISPLAY_ARMED.store(false, Ordering::Relaxed);
    }
}

/// (ZFIN round 9) What the display pass of this frame draws from (set by the late pass, taken at the Present).
struct Pending {
    root: ID3D12RootSignature,
    heap: ID3D12DescriptorHeap,
    table: u64,
    block: u64,
    /// the block's row 42 in the upload memory (written at the Present: the mode and the mask's scale)
    row42: usize,
    mask: ID3D12Resource,
    size: (u32, u32),
    shaders: (Vec<u8>, Vec<u8>),
}

// SAFETY: COM objects of a free-threaded device and a pointer into our own persistent upload memory.
unsafe impl Send for Pending {}

static PENDING: Mutex<Option<Pending>> = Mutex::new(None);

/// (ZFIN round 9) The display pass's own objects (made on the device and queue that present).
struct DispGpu {
    device: usize,
    queue: usize,
    fence: ID3D12Fence,
    fence_value: u64,
    slots: Vec<(ID3D12CommandAllocator, ID3D12GraphicsCommandList, u64)>,
    rtv_heap: ID3D12DescriptorHeap,
    rtv_size: usize,
    pipelines: Vec<(i32, ID3D12PipelineState)>,
}

// SAFETY: as Pending.
unsafe impl Send for DispGpu {}

static DISP_GPU: Mutex<Option<DispGpu>> = Mutex::new(None);

/// (ZFIN round 9) How the display pass writes into a back buffer of `format`: (the view's format, 1 = linear values / 0 =
/// display-encoded). None: not drawn there (HDR / float back buffers: the late comp stays).
pub fn display_mode(format: DXGI_FORMAT) -> Option<(DXGI_FORMAT, f32)> {
    match format {
        DXGI_FORMAT_R10G10B10A2_UNORM | DXGI_FORMAT_R8G8B8A8_UNORM | DXGI_FORMAT_B8G8R8A8_UNORM => Some((format, 0.0)),
        DXGI_FORMAT_R8G8B8A8_UNORM_SRGB | DXGI_FORMAT_B8G8R8A8_UNORM_SRGB => Some((format, 1.0)),
        _ => None,
    }
}

/// (ZFIN round 9) AT THE GAME'S PRESENT (gfx.rs, before the HUD overlay): the beam's colour over the silhouette, in display
/// colours, from this frame's mask. Nothing when the late pass left nothing.
pub(crate) fn at_present(device: &ID3D12Device, queue: &ID3D12CommandQueue, swap: &windows::Win32::Graphics::Dxgi::IDXGISwapChain3) {
    let Some(p) = PENDING.lock().ok().and_then(|mut g| g.take()) else { return };
    if !DISPLAY.load(Ordering::Relaxed) {
        return;
    }
    let ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe { present_comp(device, queue, swap, &p) }));
    match ok {
        Ok(Ok(true)) => DISPLAY_ARMED.store(true, Ordering::Relaxed),
        Ok(Ok(false)) => DISPLAY_ARMED.store(false, Ordering::Relaxed),
        Ok(Err(e)) => {
            DISPLAY_ARMED.store(false, Ordering::Relaxed);
            rlog!("render: (ZFIN round 9) the finale's display pass failed ({e}): the late pass draws the colour");
        }
        Err(_) => {
            DISPLAY.store(false, Ordering::Relaxed);
            DISPLAY_ARMED.store(false, Ordering::Relaxed);
            rlog!("render: (ZFIN round 9) PANIC in the finale's display pass: off for this run (the late pass draws the colour)");
        }
    }
}

unsafe fn present_comp(device: &ID3D12Device, queue: &ID3D12CommandQueue, swap: &windows::Win32::Graphics::Dxgi::IDXGISwapChain3, p: &Pending) -> windows::core::Result<bool> {
    let back: ID3D12Resource = swap.GetBuffer(swap.GetCurrentBackBufferIndex())?;
    let desc = back.GetDesc();
    let Some((view_format, mode)) = display_mode(desc.Format) else { return Ok(false) };
    if desc.Dimension != D3D12_RESOURCE_DIMENSION_TEXTURE2D || desc.SampleDesc.Count != 1 || desc.Width == 0 || desc.Height == 0 {
        return Ok(false);
    }
    let Ok(mut guard) = DISP_GPU.lock() else { return Ok(false) };
    if guard.as_ref().is_some_and(|g| g.device != device.as_raw() as usize || g.queue != queue.as_raw() as usize) {
        *guard = None;
    }
    if guard.is_none() {
        let fence: ID3D12Fence = device.CreateFence(0, D3D12_FENCE_FLAG_NONE)?;
        let mut slots = Vec::new();
        for _ in 0..3 {
            let allocator: ID3D12CommandAllocator = device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
            let list: ID3D12GraphicsCommandList = device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator, None)?;
            list.Close()?;
            slots.push((allocator, list, 0u64));
        }
        let rtv_heap: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV, NumDescriptors: 3, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE, NodeMask: 0 })?;
        let rtv_size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV) as usize;
        *guard = Some(DispGpu { device: device.as_raw() as usize, queue: queue.as_raw() as usize, fence, fence_value: 0, slots, rtv_heap, rtv_size, pipelines: Vec::new() });
        rlog!("render: (ZFIN round 9) the finale's display pass is made (back buffer {}x{} format {}, {}): the beam's colour over the silhouette in the test level's display colours", desc.Width, desc.Height, desc.Format.0, if mode > 0.5 { "linear values" } else { "display-encoded values" });
    }
    let g = guard.as_mut().unwrap();
    let state = match g.pipelines.iter().find(|(f, _)| *f == view_format.0) {
        Some((_, made)) => made.clone(),
        None => {
            let made = pipeline(device, &p.root, &p.shaders.0, &p.shaders.1, &[view_format], true)?;
            g.pipelines.push((view_format.0, made.clone()));
            made
        }
    };
    let completed = g.fence.GetCompletedValue();
    let Some(k) = (0..g.slots.len()).find(|k| g.slots[*k].2 <= completed) else { return Ok(true) };
    // the mode and the mask's pixels per back-buffer pixel (row 42: the late pass reads none of it)
    // (w: the testing aid KH_FZ_MASK=1, the mask itself shown: its red, green and blue as they are)
    let row = [mode, p.size.0 as f32 / desc.Width as f32, p.size.1 as f32 / desc.Height as f32, if std::env::var_os("KH_FZ_MASK").is_some() { 1.0 } else { 0.0 }];
    std::ptr::copy_nonoverlapping(row.as_ptr() as *const u8, p.row42 as *mut u8, 16);
    let (allocator, list, _) = &g.slots[k];
    allocator.Reset()?;
    list.Reset(allocator, None)?;
    let rtv = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: g.rtv_heap.GetCPUDescriptorHandleForHeapStart().ptr + k * g.rtv_size };
    let view = D3D12_RENDER_TARGET_VIEW_DESC { Format: view_format, ViewDimension: D3D12_RTV_DIMENSION_TEXTURE2D, Anonymous: D3D12_RENDER_TARGET_VIEW_DESC_0 { Texture2D: D3D12_TEX2D_RTV { MipSlice: 0, PlaneSlice: 0 } } };
    device.CreateRenderTargetView(&back, Some(&view), rtv);
    list.ResourceBarrier(&[transition(&back, D3D12_RESOURCE_STATE_PRESENT, D3D12_RESOURCE_STATE_RENDER_TARGET)]);
    list.OMSetRenderTargets(1, Some(&rtv), false, None);
    list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: desc.Width as f32, Height: desc.Height as f32, MinDepth: 0.0, MaxDepth: 1.0 }]);
    list.RSSetScissorRects(&[RECT { left: 0, top: 0, right: desc.Width as i32, bottom: desc.Height as i32 }]);
    list.SetGraphicsRootSignature(&p.root);
    list.SetDescriptorHeaps(&[Some(p.heap.clone())]);
    list.SetGraphicsRootDescriptorTable(2, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: p.table });
    list.SetGraphicsRootConstantBufferView(0, p.block);
    list.SetGraphicsRootConstantBufferView(1, p.block);
    list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
    list.SetPipelineState(&state);
    list.DrawInstanced(3, 1, 0, 0);
    list.ResourceBarrier(&[transition(&back, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_PRESENT)]);
    list.Close()?;
    queue.ExecuteCommandLists(&[Some(list.cast::<ID3D12CommandList>()?)]);
    g.fence_value += 1;
    queue.Signal(&g.fence, g.fence_value)?;
    g.slots[k].2 = g.fence_value;
    let _ = &p.mask; // (the mask lives in the kit; held here until the list is recorded)
    Ok(true)
}

struct Kit {
    device: usize,
    /// our own root signature: b0 / b1 (the block, every stage: the pieces' vertex stage reads it) and t0..t5
    root: ID3D12RootSignature,
    heap: ID3D12DescriptorHeap,
    srv_size: usize,
    rtv_heap: ID3D12DescriptorHeap,
    rtv_size: usize,
    upload: ID3D12Resource,
    mapped: *mut u8,
    /// the probe's memory (8 x 1, R32G32B32A32 float), written and read in turn; first use: cleared to -1
    memory: [ID3D12Resource; 2],
    memory_fresh: bool,
    flip: usize,
    /// per frame slot: the memory read back (256 bytes) and whether it holds a frame's copy
    readback: Vec<(ID3D12Resource, bool)>,
    probe: ID3D12PipelineState,
    /// (round 3) the silhouette's snapshot (ATLAS x ATLAS, R32G32B32A32 float) and its pipeline; whose it holds
    atlas: ID3D12Resource,
    snap_pipeline: ID3D12PipelineState,
    snap: Option<Snap>,
    looks: Option<Looks>,
    /// the ghost (the last white shape, R8): its size and whose shape it holds
    ghost: Option<(ID3D12Resource, u32, u32)>,
    /// (round 5) the silhouette's mask at the target's size (r its body / pieces, g its light)
    mask: Option<(ID3D12Resource, u32, u32)>,
    ghost_of: Option<u32>,
    told: Vec<u32>,
    /// (round 7) per target: frames seen, its stencil value reported (the log says what the probe found, in game)
    learn_told: Vec<(u32, u32, bool)>,
}

// SAFETY: the kit is used under its lock on the submission thread only; `mapped` points into `upload`, which it owns.
unsafe impl Send for Kit {}

static KIT: Mutex<Option<Kit>> = Mutex::new(None);
static FAILED: Mutex<Option<String>> = Mutex::new(None);

/// Drops the GPU objects (the renderer's release; the device's change makes them again).
pub(crate) fn release() {
    if let Ok(mut kit) = KIT.lock() {
        *kit = None;
    }
    if let Ok(mut b) = BREAKING.lock() {
        *b = None;
    }
}

fn transition(resource: &ID3D12Resource, before: D3D12_RESOURCE_STATES, after: D3D12_RESOURCE_STATES) -> D3D12_RESOURCE_BARRIER {
    D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
        Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
        Anonymous: D3D12_RESOURCE_BARRIER_0 {
            // SAFETY: a borrowed copy of the interface pointer: never released through this struct.
            Transition: ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER { pResource: unsafe { std::mem::transmute_copy(resource) }, Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, StateBefore: before, StateAfter: after }),
        },
    }
}

unsafe fn texture(device: &ID3D12Device, width: u32, height: u32, format: DXGI_FORMAT) -> windows::core::Result<ID3D12Resource> {
    let desc = D3D12_RESOURCE_DESC { Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D, Width: width as u64, Height: height, DepthOrArraySize: 1, MipLevels: 1, Format: format, SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 }, Flags: D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET, ..Default::default() };
    let mut out: Option<ID3D12Resource> = None;
    device.CreateCommittedResource(&D3D12_HEAP_PROPERTIES { Type: D3D12_HEAP_TYPE_DEFAULT, ..Default::default() }, D3D12_HEAP_FLAG_NONE, &desc, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, None, &mut out)?;
    out.ok_or_else(|| windows::core::Error::from_hresult(windows::Win32::Foundation::E_FAIL))
}

unsafe fn buffer(device: &ID3D12Device, kind: D3D12_HEAP_TYPE, bytes: u64, state: D3D12_RESOURCE_STATES) -> windows::core::Result<ID3D12Resource> {
    let desc = D3D12_RESOURCE_DESC { Dimension: D3D12_RESOURCE_DIMENSION_BUFFER, Width: bytes, Height: 1, DepthOrArraySize: 1, MipLevels: 1, SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 }, Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR, ..Default::default() };
    let mut out: Option<ID3D12Resource> = None;
    device.CreateCommittedResource(&D3D12_HEAP_PROPERTIES { Type: kind, ..Default::default() }, D3D12_HEAP_FLAG_NONE, &desc, state, None, &mut out)?;
    out.ok_or_else(|| windows::core::Error::from_hresult(windows::Win32::Foundation::E_FAIL))
}

/// A full-screen pipeline: `targets` formats, the first alpha-blended (or not, `blend` false), no depth.
unsafe fn pipeline(device: &ID3D12Device, root: &ID3D12RootSignature, vertex: &[u8], pixel: &[u8], targets: &[DXGI_FORMAT], blend: bool) -> windows::core::Result<ID3D12PipelineState> {
    pipeline_with(device, root, vertex, pixel, targets, blend, false)
}

/// As `pipeline`; `max`: the first target keeps the larger of what it holds and what is drawn (the mask).
unsafe fn pipeline_with(device: &ID3D12Device, root: &ID3D12RootSignature, vertex: &[u8], pixel: &[u8], targets: &[DXGI_FORMAT], blend: bool, max: bool) -> windows::core::Result<ID3D12PipelineState> {
    let mut formats = [DXGI_FORMAT_UNKNOWN; 8];
    let mut blend_desc = D3D12_BLEND_DESC { IndependentBlendEnable: true.into(), ..Default::default() };
    for (k, f) in targets.iter().enumerate() {
        formats[k] = *f;
        blend_desc.RenderTarget[k].RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8;
    }
    if blend {
        blend_desc.RenderTarget[0] = D3D12_RENDER_TARGET_BLEND_DESC {
            BlendEnable: true.into(),
            LogicOpEnable: false.into(),
            SrcBlend: D3D12_BLEND_SRC_ALPHA,
            DestBlend: D3D12_BLEND_INV_SRC_ALPHA,
            BlendOp: D3D12_BLEND_OP_ADD,
            SrcBlendAlpha: D3D12_BLEND_ZERO,
            DestBlendAlpha: D3D12_BLEND_ONE,
            BlendOpAlpha: D3D12_BLEND_OP_ADD,
            LogicOp: D3D12_LOGIC_OP_NOOP,
            RenderTargetWriteMask: (D3D12_COLOR_WRITE_ENABLE_RED.0 | D3D12_COLOR_WRITE_ENABLE_GREEN.0 | D3D12_COLOR_WRITE_ENABLE_BLUE.0) as u8,
        };
    }
    if max {
        blend_desc.RenderTarget[0] = D3D12_RENDER_TARGET_BLEND_DESC {
            BlendEnable: true.into(),
            LogicOpEnable: false.into(),
            SrcBlend: D3D12_BLEND_ONE,
            DestBlend: D3D12_BLEND_ONE,
            BlendOp: D3D12_BLEND_OP_MAX,
            SrcBlendAlpha: D3D12_BLEND_ONE,
            DestBlendAlpha: D3D12_BLEND_ONE,
            BlendOpAlpha: D3D12_BLEND_OP_MAX,
            LogicOp: D3D12_LOGIC_OP_NOOP,
            RenderTargetWriteMask: D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8,
        };
    }
    let pso = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
        pRootSignature: std::mem::transmute_copy(root),
        VS: D3D12_SHADER_BYTECODE { pShaderBytecode: vertex.as_ptr() as *const c_void, BytecodeLength: vertex.len() },
        PS: D3D12_SHADER_BYTECODE { pShaderBytecode: pixel.as_ptr() as *const c_void, BytecodeLength: pixel.len() },
        BlendState: blend_desc,
        SampleMask: u32::MAX,
        RasterizerState: D3D12_RASTERIZER_DESC { FillMode: D3D12_FILL_MODE_SOLID, CullMode: D3D12_CULL_MODE_NONE, DepthClipEnable: false.into(), ..Default::default() },
        DepthStencilState: D3D12_DEPTH_STENCIL_DESC { DepthEnable: false.into(), StencilEnable: false.into(), ..Default::default() },
        PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
        NumRenderTargets: targets.len() as u32,
        RTVFormats: formats,
        DSVFormat: DXGI_FORMAT_UNKNOWN,
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        ..Default::default()
    };
    device.CreateGraphicsPipelineState(&pso)
}

fn shader(name: &str) -> Option<Vec<u8>> {
    // (the offline harness: `KH_FINALE_SHADERS=DIR` reads the finale's files from DIR first, the pack stays as it is)
    if let Some(bytes) = std::env::var_os("KH_FINALE_SHADERS").and_then(|d| std::fs::read(std::path::Path::new(&d).join(name)).ok()) {
        return Some(bytes);
    }
    let pack = crate::SHARED.lock().ok().and_then(|g| g.as_ref().map(|s| s.pack.clone()))?;
    std::fs::read(pack.join("materials").join("er").join(name)).ok()
}

/// Descriptors per table: t0..t22 (the root signature's table); ours are t0..t5.
const RTV_MEMORY: usize = 0;
const RTV_GHOST: usize = 2;
const RTV_ATLAS: usize = 3;
const RTV_MASK: usize = 4;

unsafe fn make_kit(device: &ID3D12Device) -> Result<Kit, String> {
    let text = |what: &str, e: windows::core::Error| format!("{what}: {e}");
    let range = [D3D12_DESCRIPTOR_RANGE { RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SRV, NumDescriptors: 7, BaseShaderRegister: 0, RegisterSpace: 0, OffsetInDescriptorsFromTableStart: 0 }];
    let cbv = |register: u32| D3D12_ROOT_PARAMETER { ParameterType: D3D12_ROOT_PARAMETER_TYPE_CBV, Anonymous: D3D12_ROOT_PARAMETER_0 { Descriptor: D3D12_ROOT_DESCRIPTOR { ShaderRegister: register, RegisterSpace: 0 } }, ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL };
    let parameters = [cbv(0), cbv(1), D3D12_ROOT_PARAMETER { ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE, Anonymous: D3D12_ROOT_PARAMETER_0 { DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE { NumDescriptorRanges: 1, pDescriptorRanges: range.as_ptr() } }, ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL }];
    let desc = D3D12_ROOT_SIGNATURE_DESC { NumParameters: parameters.len() as u32, pParameters: parameters.as_ptr(), NumStaticSamplers: 0, pStaticSamplers: std::ptr::null(), Flags: D3D12_ROOT_SIGNATURE_FLAG_NONE };
    let mut blob: Option<windows::Win32::Graphics::Direct3D::ID3DBlob> = None;
    D3D12SerializeRootSignature(&desc, D3D_ROOT_SIGNATURE_VERSION_1, &mut blob, None).map_err(|e| text("root signature", e))?;
    let blob = blob.ok_or("no root signature")?;
    let root: ID3D12RootSignature = device.CreateRootSignature(0, std::slice::from_raw_parts(blob.GetBufferPointer() as *const u8, blob.GetBufferSize())).map_err(|e| text("root signature", e))?;
    let root = &root;
    let heap: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, NumDescriptors: (SLOTS * 2 * TABLE) as u32, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, NodeMask: 0 }).map_err(|e| text("views", e))?;
    let srv_size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV) as usize;
    // every entry a null view until a frame writes its own
    for k in 0..SLOTS * 2 * TABLE {
        let format = if k % TABLE == 2 { DXGI_FORMAT_R8_UINT } else { DXGI_FORMAT_R32_FLOAT };
        let desc = D3D12_SHADER_RESOURCE_VIEW_DESC { Format: format, ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D, Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING, Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 { Texture2D: D3D12_TEX2D_SRV { MostDetailedMip: 0, MipLevels: 1, PlaneSlice: 0, ResourceMinLODClamp: 0.0 } } };
        device.CreateShaderResourceView(None, Some(&desc), D3D12_CPU_DESCRIPTOR_HANDLE { ptr: heap.GetCPUDescriptorHandleForHeapStart().ptr + k * srv_size });
    }
    let rtv_heap: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV, NumDescriptors: 5, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE, NodeMask: 0 }).map_err(|e| text("target views", e))?;
    let rtv_size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV) as usize;
    let rtv = |k: usize| D3D12_CPU_DESCRIPTOR_HANDLE { ptr: rtv_heap.GetCPUDescriptorHandleForHeapStart().ptr + k * rtv_size };
    let memory = [texture(device, MEMORY_WIDTH, 1, DXGI_FORMAT_R32G32B32A32_FLOAT).map_err(|e| text("memory", e))?, texture(device, MEMORY_WIDTH, 1, DXGI_FORMAT_R32G32B32A32_FLOAT).map_err(|e| text("memory", e))?];
    for (k, m) in memory.iter().enumerate() {
        device.CreateRenderTargetView(m, None, rtv(RTV_MEMORY + k));
    }
    let atlas = texture(device, ATLAS, ATLAS, DXGI_FORMAT_R32G32B32A32_FLOAT).map_err(|e| text("snapshot", e))?;
    device.CreateRenderTargetView(&atlas, None, rtv(RTV_ATLAS));
    let mut readback = Vec::new();
    for _ in 0..SLOTS {
        readback.push((buffer(device, D3D12_HEAP_TYPE_READBACK, 256, D3D12_RESOURCE_STATE_COPY_DEST).map_err(|e| text("read-back", e))?, false));
    }
    let upload = buffer(device, D3D12_HEAP_TYPE_UPLOAD, (SLOTS * MAX_TARGETS * BLOCK_BYTES) as u64, D3D12_RESOURCE_STATE_GENERIC_READ).map_err(|e| text("constants", e))?;
    let mut at: *mut c_void = std::ptr::null_mut();
    upload.Map(0, None, Some(&mut at)).map_err(|e| text("constants", e))?;
    let (Some(vertex), Some(probe_ps), Some(snap_ps)) = (shader("blob.vs.dxil"), shader("finale_probe.ps.dxil"), shader("finale_snap.ps.dxil")) else { return Err("materials/er/finale_probe.ps.dxil / finale_snap.ps.dxil (or blob.vs.dxil) missing".to_string()) };
    let probe = pipeline(device, root, &vertex, &probe_ps, &[DXGI_FORMAT_R32G32B32A32_FLOAT], false).map_err(|e| text("the probe", e))?;
    let snap_pipeline = pipeline(device, root, &vertex, &snap_ps, &[DXGI_FORMAT_R32G32B32A32_FLOAT], false).map_err(|e| text("the snapshot", e))?;
    Ok(Kit { device: device.as_raw() as usize, root: root.clone(), heap, srv_size, rtv_heap, rtv_size, upload, mapped: at as *mut u8, memory, memory_fresh: true, flip: 0, readback, probe, atlas, snap_pipeline, snap: None, looks: None, ghost: None, mask: None, ghost_of: None, told: Vec::new(), learn_told: Vec::new() })
}

/// What gfx.rs hands over: its list with the scene target and the scene depth of the frame, the depth in DEPTH_WRITE on
/// entry and on return (this pass reads it as a texture in between), the snapshot of this frame (when the early depth ran).
pub(crate) struct FinaleCx<'a> {
    pub device: &'a ID3D12Device,
    pub list: &'a ID3D12GraphicsCommandList,
    /// (the caller's root signature: the finale draws with its own and the caller sets its own again afterwards)
    #[allow(dead_code)]
    pub root: &'a ID3D12RootSignature,
    pub slot: usize,
    pub target_view: D3D12_CPU_DESCRIPTOR_HANDLE,
    pub target_format: DXGI_FORMAT,
    pub depth: &'a ID3D12Resource,
    pub snapshot: Option<&'a ID3D12Resource>,
    pub view: View,
    /// What KH3's scene-linear 1.0 of an effect becomes in the target (vfx.rs `effect_scale`).
    pub scale: f32,
}

/// Records the finale's passes (nothing when no finale is asked). Returns true when something was recorded: the caller sets
/// its own root state, heaps and targets again.
pub(crate) unsafe fn draw(cx: &FinaleCx) -> bool {
    let targets = current();
    if targets.is_empty() || cx.slot >= SLOTS {
        if let Ok(mut b) = BREAKING.lock() {
            *b = None;
        }
        return false;
    }
    let Ok(mut guard) = KIT.try_lock() else { return false };
    if guard.as_ref().is_some_and(|k| k.device != cx.device.as_raw() as usize) {
        *guard = None;
    }
    if guard.is_none() {
        if FAILED.lock().ok().is_some_and(|f| f.is_some()) {
            return false;
        }
        match make_kit(cx.device) {
            Ok(kit) => *guard = Some(kit),
            Err(e) => {
                rlog!("render: (ZFIN) the finale's objects cannot be made ({e}): the beam's own white light only");
                if let Ok(mut f) = FAILED.lock() {
                    *f = Some(e);
                }
                return false;
            }
        }
    }
    let Some(kit) = guard.as_mut() else { return false };
    if kit.looks.as_ref().map(|l| l.format) != Some(cx.target_format.0) {
        kit.looks = None;
        let made = (|| {
            let vertex = shader("blob.vs.dxil")?;
            let main = pipeline(cx.device, &kit.root, &vertex, &shader("finale.ps.dxil")?, &[cx.target_format], true).ok()?;
            let ghost = pipeline(cx.device, &kit.root, &vertex, &shader("finale_g.ps.dxil")?, &[cx.target_format, DXGI_FORMAT_R8_UNORM], true).ok()?;
            let fade = pipeline(cx.device, &kit.root, &vertex, &shader("finale_fade.ps.dxil")?, &[cx.target_format], true).ok()?;
            let pieces = pipeline(cx.device, &kit.root, &shader("finale_frag.vs.dxil")?, &shader("finale_frag.ps.dxil")?, &[cx.target_format], true).ok()?;
            let main_mask = pipeline_with(cx.device, &kit.root, &vertex, &shader("finale.ps.dxil")?, &[DXGI_FORMAT_R8G8B8A8_UNORM], false, true).ok()?;
            let pieces_mask = pipeline_with(cx.device, &kit.root, &shader("finale_frag.vs.dxil")?, &shader("finale_frag.ps.dxil")?, &[DXGI_FORMAT_R8G8B8A8_UNORM], false, true).ok()?;
            let comp = pipeline(cx.device, &kit.root, &vertex, &shader("finale_comp.ps.dxil")?, &[cx.target_format], true).ok()?;
            // (ZFIN round 9) the display route's two stages (optional: without them the comp stays in the late pass)
            let occl = shader("finale_occl.ps.dxil").and_then(|ps| pipeline_with(cx.device, &kit.root, &vertex, &ps, &[DXGI_FORMAT_R8G8B8A8_UNORM], false, true).ok());
            let disp = shader("finale_disp.ps.dxil").map(|ps| (vertex.clone(), ps));
            Some(Looks { format: cx.target_format.0, main, ghost, fade, pieces, main_mask, pieces_mask, comp, occl, disp })
        })();
        match made {
            Some(l) => kit.looks = Some(l),
            None => {
                rlog!("render: (ZFIN) the finale's pipelines cannot be made (materials/er/finale*.dxil missing?): the beam's own white light only");
                if let Ok(mut f) = FAILED.lock() {
                    *f = Some("no pipelines".to_string());
                }
                *guard = None;
                return false;
            }
        }
    }
    // the ghost at the target's size
    let (w, h) = (cx.view.width, cx.view.height);
    if kit.ghost.as_ref().map(|g| (g.1, g.2)) != Some((w, h)) {
        kit.ghost = texture(cx.device, w.max(1), h.max(1), DXGI_FORMAT_R8_UNORM).ok().map(|t| (t, w, h));
        kit.ghost_of = None;
        if let Some((g, ..)) = kit.ghost.as_ref() {
            cx.device.CreateRenderTargetView(g, None, D3D12_CPU_DESCRIPTOR_HANDLE { ptr: kit.rtv_heap.GetCPUDescriptorHandleForHeapStart().ptr + RTV_GHOST * kit.rtv_size });
        }
    }
    // (round 5) the mask at the target's size
    if kit.mask.as_ref().map(|g| (g.1, g.2)) != Some((w, h)) {
        kit.mask = texture(cx.device, w.max(1), h.max(1), DXGI_FORMAT_R8G8B8A8_UNORM).ok().map(|t| (t, w, h));
        if let Some((m, ..)) = kit.mask.as_ref() {
            cx.device.CreateRenderTargetView(m, None, D3D12_CPU_DESCRIPTOR_HANDLE { ptr: kit.rtv_heap.GetCPUDescriptorHandleForHeapStart().ptr + RTV_MASK * kit.rtv_size });
        }
    }
    // what the probe had learnt (this slot's copy: its frame is done, the slot is being used again)
    if let Some((rb, true)) = kit.readback.get(cx.slot) {
        let mut at: *mut c_void = std::ptr::null_mut();
        if rb.Map(0, Some(&D3D12_RANGE { Begin: 0, End: 128 }), Some(&mut at)).is_ok() && !at.is_null() {
            let v = std::slice::from_raw_parts(at as *const f32, (MEMORY_WIDTH * 4) as usize);
            let learnt: Vec<u32> = v.chunks_exact(4).filter(|t| t[0] >= 0.0 && t[1] > 0.0).map(|t| t[1] as u32).collect();
            if let Ok(mut g) = LEARNT_GROUND.lock() {
                // (ZDRAW) x the value, y the id, z the ground's value around its feet (-1 none)
                *g = v.chunks_exact(4).filter(|t| t[0] >= 0.0 && t[1] > 0.0 && t[2] >= 0.0 && ((t[0] as u32) & 0xF0) == ((t[2] as u32) & 0xF0)).map(|t| t[1] as u32).collect();
            }
            if let Ok(mut lv) = LEARNT_VALUES.lock() {
                *lv = v.chunks_exact(4).filter(|t| t[0] >= 0.0 && t[1] > 0.0).map(|t| (t[1] as u32, t[0], t[3])).collect();
            }
            rb.Unmap(0, Some(&D3D12_RANGE { Begin: 0, End: 0 }));
            if let Ok(mut l) = LEARNT.lock() {
                *l = learnt;
            }
        }
    }
    let Some(looks) = kit.looks.as_ref() else { return false };
    let mode = MODE.load(Ordering::Relaxed);
    let white = f32::from_bits(WHITE.load(Ordering::Relaxed)) * cx.scale.max(1e-4);
    let fade = f32::from_bits(FADE.load(Ordering::Relaxed));
    // the views of this frame (written in the frame they are used): table A (the probe: t3 = the memory written last
    // frame), table B (the looks: t3 = the memory written now, t4 = the ghost, t5 = the snapshot)
    let depth_desc = cx.depth.GetDesc();
    let Some((depth_format, _)) = depth_views(depth_desc.Format) else { return false };
    let snap = cx.snapshot.filter(|s| {
        let d = s.GetDesc();
        d.Width == depth_desc.Width && d.Height == depth_desc.Height && d.Format == depth_desc.Format
    });
    let stencil_format = snap.and_then(|s| depth_views(s.GetDesc().Format)).and_then(|v| v.1);
    let isolate = mode != 2 && snap.is_some() && stencil_format.is_some();
    let (cur, prev) = (kit.flip, 1 - kit.flip);
    kit.flip = prev;
    let view = |format: DXGI_FORMAT, plane: u32| D3D12_SHADER_RESOURCE_VIEW_DESC { Format: format, ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D, Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING, Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 { Texture2D: D3D12_TEX2D_SRV { MostDetailedMip: 0, MipLevels: 1, PlaneSlice: plane, ResourceMinLODClamp: 0.0 } } };
    for (table, memory) in [(0usize, prev), (1, cur)] {
        let base = (cx.slot * 2 + table) * TABLE;
        let at = |k: usize| D3D12_CPU_DESCRIPTOR_HANDLE { ptr: kit.heap.GetCPUDescriptorHandleForHeapStart().ptr + (base + k) * kit.srv_size };
        cx.device.CreateShaderResourceView(cx.depth, Some(&view(depth_format, 0)), at(0));
        match (isolate, snap, stencil_format) {
            (true, Some(s), Some(sf)) => {
                cx.device.CreateShaderResourceView(s, Some(&view(depth_format, 0)), at(1));
                cx.device.CreateShaderResourceView(s, Some(&view(sf, 1)), at(2));
            }
            _ => {
                // (the white-out reads no snapshot: null views)
                cx.device.CreateShaderResourceView(cx.depth, Some(&view(depth_format, 0)), at(1));
                cx.device.CreateShaderResourceView(None, Some(&view(DXGI_FORMAT_R8_UINT, 0)), at(2));
            }
        }
        cx.device.CreateShaderResourceView(&kit.memory[memory], None, at(3));
        match kit.ghost.as_ref() {
            Some((g, ..)) => cx.device.CreateShaderResourceView(g, None, at(4)),
            None => cx.device.CreateShaderResourceView(None, Some(&view(DXGI_FORMAT_R8_UNORM, 0)), at(4)),
        }
        cx.device.CreateShaderResourceView(&kit.atlas, None, at(5));
        match kit.mask.as_ref() {
            Some((m, ..)) => cx.device.CreateShaderResourceView(m, None, at(6)),
            None => cx.device.CreateShaderResourceView(None, Some(&view(DXGI_FORMAT_R8G8B8A8_UNORM, 0)), at(6)),
        }
    }
    let table = |t: usize| D3D12_GPU_DESCRIPTOR_HANDLE { ptr: kit.heap.GetGPUDescriptorHandleForHeapStart().ptr + (((cx.slot * 2 + t) * TABLE) * kit.srv_size) as u64 };
    let rtv = |k: usize| D3D12_CPU_DESCRIPTOR_HANDLE { ptr: kit.rtv_heap.GetCPUDescriptorHandleForHeapStart().ptr + k * kit.rtv_size };
    // (round 3) THE BREAKING APART: the first killed target in the frame keeps a snapshot of its silhouette until its crumble
    // begins; from then on (the host hides its body) its pieces are drawn from that snapshot
    let learnt = LEARNT.lock().map(|l| l.clone()).unwrap_or_default();
    // (round 7) the log: whether the probe found each target's own pixels (its stencil value), once, or that it did not
    // within 45 frames (then a killed one is whited out, a survivor shows as the game draws it)
    {
        let values = LEARNT_VALUES.lock().map(|l| l.clone()).unwrap_or_default();
        kit.learn_told.retain(|x| targets.iter().any(|t| t.id == x.0));
        for t in &targets {
            if !kit.learn_told.iter().any(|x| x.0 == t.id) {
                kit.learn_told.push((t.id, 0, false));
            }
            let Some(x) = kit.learn_told.iter_mut().find(|x| x.0 == t.id) else { continue };
            x.1 += 1;
            if x.2 {
                continue;
            }
            if let Some(v) = values.iter().find(|v| v.0 == t.id) {
                x.2 = true;
                rlog!("render: (ZFIN) target {}: its own pixels FOUND (stencil value {:#04x} learnt from {} sample(s) along its capsule, r {:.0} cm, half height {:.0} cm, {} neighbour(s) kept apart)", t.id, v.1 as u32, v.2 as u32, t.body.radius, t.body.half_height, t.neighbours.len());
            } else if x.1 == 45 {
                x.2 = true;
                rlog!("render: (ZFIN) target {}: its own pixels NOT found in 45 frames (no stencil value inside its capsule, r {:.0} cm, half height {:.0} cm; isolating {}): {}", t.id, t.body.radius, t.body.half_height, isolate, if t.silhouette { "the white-out" } else { "it shows as the game draws it" });
            }
        }
    }
    let breaker = targets.iter().find(|t| t.silhouette).map(|t| t.id);
    if kit.snap.is_some_and(|s| Some(s.id) != breaker) {
        kit.snap = None;
    }
    let mut snapping: Option<usize> = None;
    // the blocks of this frame
    let mut blocks: Vec<(usize, &FinaleTarget, [i32; 4], u64, bool)> = Vec::new();
    for (k, t) in targets.iter().enumerate() {
        let Some((scissor, mut block)) = plan(t, &cx.view, k, mode, white, fade, isolate, true) else { continue };
        let mut pieces = false;
        if Some(t.id) == breaker && isolate {
            if t.crumble <= 0.0 && learnt.contains(&t.id) {
                // the snapshot of this frame, over its screen box
                let (step, columns, rows) = atlas_grid(scissor);
                kit.snap = Some(Snap { id: t.id, x0: scissor[0], y0: scissor[1], step, columns, rows, body: t.body, forward: cx.view.forward });
                snapping = Some(blocks.len());
            } else if t.crumble > 0.0 && kit.snap.is_some_and(|s| s.id == t.id) {
                pieces = true;
            }
        }
        if let Some(s) = kit.snap.filter(|s| s.id == t.id) {
            snap_rows(&mut block, &s, t.push, white);
        }
        // (round 5) a silhouette is the beam's colour: into the mask, coloured over the picture after
        if t.silhouette && kit.mask.is_some() {
            let tall = (block[9 * 4 + 3] - block[9 * 4 + 1]).max(0.0) / 1.12;
            style_rows(&mut block, style_band(tall, w), cx.scale.max(1e-4), t.dark);
            // (ZDRAW) drawn from its own model: the band from its silhouette as the camera sees it
            if let Some(b) = crate::zdraw::band(t.id, &cx.view.clip, w, h) {
                block[38 * 4 + 1] = b;
            }
        }
        let offset = (cx.slot * MAX_TARGETS + k) * BLOCK_BYTES;
        std::ptr::copy_nonoverlapping(block.as_ptr() as *const u8, kit.mapped.add(offset), block.len() * 4);
        blocks.push((k, t, scissor, kit.upload.GetGPUVirtualAddress() + offset as u64, pieces));
    }
    if let Ok(mut b) = BREAKING.lock() {
        *b = kit.snap.map(|s| s.id).filter(|id| learnt.contains(id) || blocks.iter().any(|b| b.4 && b.1.id == *id));
        // (ZDRAW) never a body the screen cannot isolate (its class is the ground's)
        if b.is_some_and(|id| LEARNT_GROUND.lock().ok().is_some_and(|g| g.contains(&id))) {
            *b = None;
        }
    }
    let list = cx.list;
    let read_state = D3D12_RESOURCE_STATE_DEPTH_READ | D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE;
    let shader_state = D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE;
    list.ResourceBarrier(&[transition(cx.depth, D3D12_RESOURCE_STATE_DEPTH_WRITE, read_state)]);
    list.SetGraphicsRootSignature(&kit.root);
    list.SetDescriptorHeaps(&[Some(kit.heap.clone())]);
    list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
    // 1. THE PROBE: its stencil value into the memory (this frame's half), from the snapshot and the memory of last frame
    if kit.memory_fresh {
        list.ResourceBarrier(&[transition(&kit.memory[0], shader_state, D3D12_RESOURCE_STATE_RENDER_TARGET), transition(&kit.memory[1], shader_state, D3D12_RESOURCE_STATE_RENDER_TARGET)]);
        list.ClearRenderTargetView(rtv(RTV_MEMORY), &[-1.0, -1.0, -1.0, 0.0], None);
        list.ClearRenderTargetView(rtv(RTV_MEMORY + 1), &[-1.0, -1.0, -1.0, 0.0], None);
        list.ResourceBarrier(&[transition(&kit.memory[0], D3D12_RESOURCE_STATE_RENDER_TARGET, shader_state), transition(&kit.memory[1], D3D12_RESOURCE_STATE_RENDER_TARGET, shader_state)]);
        kit.memory_fresh = false;
    }
    list.ResourceBarrier(&[transition(&kit.memory[cur], shader_state, D3D12_RESOURCE_STATE_RENDER_TARGET)]);
    list.SetGraphicsRootDescriptorTable(2, table(0));
    list.OMSetRenderTargets(1, Some(&rtv(RTV_MEMORY + cur)), false, None);
    list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: MEMORY_WIDTH as f32, Height: 1.0, MinDepth: 0.0, MaxDepth: 1.0 }]);
    list.SetPipelineState(&kit.probe);
    for (k, _, _, address, _) in &blocks {
        if !isolate {
            break;
        }
        list.SetGraphicsRootConstantBufferView(0, *address);
        list.SetGraphicsRootConstantBufferView(1, *address);
        list.RSSetScissorRects(&[RECT { left: *k as i32, top: 0, right: *k as i32 + 1, bottom: 1 }]);
        list.DrawInstanced(3, 1, 0, 0);
    }
    list.ResourceBarrier(&[transition(&kit.memory[cur], D3D12_RESOURCE_STATE_RENDER_TARGET, shader_state)]);
    // (when nothing was probed this frame the memory written now holds what the one before held two frames ago: copy the last)
    if !isolate {
        list.ResourceBarrier(&[transition(&kit.memory[cur], shader_state, D3D12_RESOURCE_STATE_COPY_DEST), transition(&kit.memory[prev], shader_state, D3D12_RESOURCE_STATE_COPY_SOURCE)]);
        list.CopyResource(&kit.memory[cur], &kit.memory[prev]);
        list.ResourceBarrier(&[transition(&kit.memory[cur], D3D12_RESOURCE_STATE_COPY_DEST, shader_state), transition(&kit.memory[prev], D3D12_RESOURCE_STATE_COPY_SOURCE, shader_state)]);
    }
    // (what was learnt goes to the CPU: this slot's read-back, read when the slot comes round again)
    if let Some((rb, valid)) = kit.readback.get_mut(cx.slot) {
        list.ResourceBarrier(&[transition(&kit.memory[cur], shader_state, D3D12_RESOURCE_STATE_COPY_SOURCE)]);
        let footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT { Offset: 0, Footprint: D3D12_SUBRESOURCE_FOOTPRINT { Format: DXGI_FORMAT_R32G32B32A32_FLOAT, Width: MEMORY_WIDTH, Height: 1, Depth: 1, RowPitch: 256 } };
        let to = D3D12_TEXTURE_COPY_LOCATION { pResource: std::mem::transmute_copy(&*rb), Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT, Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { PlacedFootprint: footprint } };
        let from = D3D12_TEXTURE_COPY_LOCATION { pResource: std::mem::transmute_copy(&kit.memory[cur]), Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX, Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { SubresourceIndex: 0 } };
        list.CopyTextureRegion(&to, 0, 0, 0, &from, None);
        list.ResourceBarrier(&[transition(&kit.memory[cur], D3D12_RESOURCE_STATE_COPY_SOURCE, shader_state)]);
        *valid = true;
    }
    list.SetGraphicsRootDescriptorTable(2, table(1));
    // 2. (round 3) THE SNAPSHOT of the silhouette that is about to break apart
    if let (Some(b), Some(s)) = (snapping, kit.snap) {
        let (_, _, _, address, _) = blocks[b];
        list.ResourceBarrier(&[transition(&kit.atlas, shader_state, D3D12_RESOURCE_STATE_RENDER_TARGET)]);
        list.ClearRenderTargetView(rtv(RTV_ATLAS), &[0.0, 0.0, 0.0, 0.0], None);
        list.OMSetRenderTargets(1, Some(&rtv(RTV_ATLAS)), false, None);
        list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: s.columns as f32, Height: s.rows as f32, MinDepth: 0.0, MaxDepth: 1.0 }]);
        list.RSSetScissorRects(&[RECT { left: 0, top: 0, right: s.columns as i32, bottom: s.rows as i32 }]);
        list.SetPipelineState(&kit.snap_pipeline);
        list.SetGraphicsRootConstantBufferView(0, address);
        list.SetGraphicsRootConstantBufferView(1, address);
        list.DrawInstanced(3, 1, 0, 0);
        list.ResourceBarrier(&[transition(&kit.atlas, D3D12_RESOURCE_STATE_RENDER_TARGET, shader_state)]);
    }
    // 3. THE LOOKS: the ghost's writer is the first killed target crumbling (its white shape kept), the hidden ones fade; a
    // target breaking apart draws its pieces
    let has_mask = kit.mask.is_some();
    let masked = |t: &FinaleTarget| t.silhouette && has_mask;
    let writer = blocks.iter().find(|(_, t, _, _, pieces)| !pieces && !masked(t) && t.silhouette && t.hidden_for.is_none() && t.crumble >= GHOST_FROM).map(|b| b.1.id);
    let mask = kit.mask.as_ref().map(|m| m.0.clone()).filter(|_| blocks.iter().any(|b| masked(b.1)));
    if let Some(m) = mask.as_ref() {
        list.ResourceBarrier(&[transition(m, shader_state, D3D12_RESOURCE_STATE_RENDER_TARGET)]);
        list.ClearRenderTargetView(rtv(RTV_MASK), &[0.0, 0.0, 0.0, 0.0], None);
    }
    let ghost = kit.ghost.as_ref().map(|g| g.0.clone());
    let writing = writer.is_some() && ghost.is_some();
    if let (true, Some(g)) = (writing, ghost.as_ref()) {
        list.ResourceBarrier(&[transition(g, shader_state, D3D12_RESOURCE_STATE_RENDER_TARGET)]);
        list.ClearRenderTargetView(rtv(RTV_GHOST), &[0.0, 0.0, 0.0, 0.0], None);
        kit.ghost_of = writer;
    }
    list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: w as f32, Height: h as f32, MinDepth: 0.0, MaxDepth: 1.0 }]);
    let mut fades = 0;
    let mut broke = 0;
    // (ZDRAW) the silhouettes and the bodies in the light drawn from their own models (zdraw.rs): none of the screen route's
    // looks for them
    let zd_mask: Vec<u32> = blocks.iter().filter(|b| masked(b.1) && crate::zdraw::owns(b.1.id)).map(|b| b.1.id).collect();
    let zd_light: Vec<u32> = blocks.iter().filter(|b| b.1.hidden_for.is_none() && crate::zdraw::lights(b.1.id)).map(|b| b.1.id).collect();
    for (_, t, scissor, address, pieces) in &blocks {
        if zd_mask.contains(&t.id) || (!t.silhouette && zd_light.contains(&t.id)) {
            continue;
        }
        list.SetGraphicsRootConstantBufferView(0, *address);
        list.SetGraphicsRootConstantBufferView(1, *address);
        if *pieces {
            // its pieces, anywhere on the screen they fly to
            let Some(s) = kit.snap else { continue };
            if masked(t) {
                list.OMSetRenderTargets(1, Some(&rtv(RTV_MASK)), false, None);
                list.SetPipelineState(&looks.pieces_mask);
            } else {
                list.OMSetRenderTargets(1, Some(&cx.target_view), false, None);
                list.SetPipelineState(&looks.pieces);
            }
            list.RSSetScissorRects(&[RECT { left: 0, top: 0, right: w as i32, bottom: h as i32 }]);
            list.DrawInstanced(6, s.columns * s.rows * PIECE_LAYERS, 0, 0);
            broke += 1;
            continue;
        }
        let pipeline = match t.hidden_for {
            Some(hidden) => {
                // its body hidden: only its last white shape, fading (none when the ghost holds another's or is being written)
                if writing || kit.ghost_of != Some(t.id) || fade_alpha(hidden, fade) <= 0.0 {
                    continue;
                }
                fades += 1;
                list.OMSetRenderTargets(1, Some(&cx.target_view), false, None);
                &looks.fade
            }
            None if writing && writer == Some(t.id) => {
                let views = [cx.target_view, rtv(RTV_GHOST)];
                list.OMSetRenderTargets(2, Some(views.as_ptr()), false, None);
                &looks.ghost
            }
            None if masked(t) => {
                list.OMSetRenderTargets(1, Some(&rtv(RTV_MASK)), false, None);
                &looks.main_mask
            }
            None => {
                list.OMSetRenderTargets(1, Some(&cx.target_view), false, None);
                &looks.main
            }
        };
        list.SetPipelineState(pipeline);
        list.RSSetScissorRects(&[RECT { left: scissor[0], top: scissor[1], right: scissor[2], bottom: scissor[3] }]);
        list.DrawInstanced(3, 1, 0, 0);
        if !kit.told.contains(&t.id) && t.hidden_for.is_none() {
            kit.told.push(t.id);
            if kit.told.len() > 64 {
                kit.told.remove(0);
            }
            let why = match (mode, snap.is_some(), stencil_format.is_some()) {
                (2, ..) => "r.finale 2",
                (_, false, _) => "no snapshot of the scene depth this frame (r.early_depth off?)",
                (_, true, false) => "the scene depth has no stencil plane",
                _ => "",
            };
            if isolate {
                rlog!("render: (ZFIN) finale of target {}: its own pixels (its stencil value learnt from the snapshot along its capsule + depth + its reach, {} neighbour(s) kept apart; white-out until its value is seen or when none is found), silhouette {}, white {:.2}", t.id, t.neighbours.len().min(MAX_NEIGHBOURS), t.silhouette, white);
            } else {
                rlog!("render: (ZFIN) finale of target {}: the WHITE-OUT of its screen box ({why})", t.id);
            }
        }
    }
    if let (true, Some(g)) = (writing, ghost.as_ref()) {
        list.ResourceBarrier(&[transition(g, D3D12_RESOURCE_STATE_RENDER_TARGET, shader_state)]);
    }
    // (ZDRAW) the bodies in the light from their own models: the test level's light grey, into the scene target
    if !zd_light.is_empty() {
        list.OMSetRenderTargets(1, Some(&cx.target_view), false, None);
        list.RSSetScissorRects(&[RECT { left: 0, top: 0, right: w as i32, bottom: h as i32 }]);
        if !crate::zdraw::draw_light(cx.device, list, &kit.root, cx.slot, cx.target_format, &cx.view, cx.scale, table(1), &zd_light).is_empty() {
            list.SetGraphicsRootDescriptorTable(2, table(1));
        }
    }
    // (ZDRAW) the silhouettes from their own models: their triangles (whole, then their pieces, cracks and ash) into the mask
    if mask.is_some() && !zd_mask.is_empty() {
        list.OMSetRenderTargets(1, Some(&rtv(RTV_MASK)), false, None);
        list.RSSetScissorRects(&[RECT { left: 0, top: 0, right: w as i32, bottom: h as i32 }]);
        if !crate::zdraw::draw_mask(cx.device, list, &kit.root, &kit.heap, cx.slot, &cx.view, &zd_mask, rtv(RTV_MASK), w, h).is_empty() {
            list.SetGraphicsRootDescriptorTable(2, table(1));
        }
    }
    // (round 5) THE BEAM'S COLOUR over the picture, from the mask (after our effects' bloom and screen passes)
    if let Some(m) = mask.as_ref() {
        // (ZFIN round 9) the display route: where the comp must not show goes into the mask's blue now (the scene depth is
        // readable here, not at the Present); the colour itself is drawn at the game's Present (`at_present`)
        let display = DISPLAY.load(Ordering::Relaxed) && looks.occl.is_some() && looks.disp.is_some();
        let first = blocks.iter().find(|b| masked(b.1)).map(|b| b.3);
        if let (true, Some(address), Some(occl)) = (display, first, looks.occl.as_ref()) {
            list.OMSetRenderTargets(1, Some(&rtv(RTV_MASK)), false, None);
            list.SetGraphicsRootConstantBufferView(0, address);
            list.SetGraphicsRootConstantBufferView(1, address);
            list.RSSetScissorRects(&[RECT { left: 0, top: 0, right: w as i32, bottom: h as i32 }]);
            list.SetPipelineState(occl);
            list.DrawInstanced(3, 1, 0, 0);
            let row42 = kit.mapped as usize + (address - kit.upload.GetGPUVirtualAddress()) as usize + 42 * 16;
            if let (Ok(mut pending), Some(shaders)) = (PENDING.lock(), looks.disp.clone()) {
                *pending = Some(Pending { root: kit.root.clone(), heap: kit.heap.clone(), table: table(1).ptr, block: address, row42, mask: m.clone(), size: (w, h), shaders });
            }
        }
        list.ResourceBarrier(&[transition(m, D3D12_RESOURCE_STATE_RENDER_TARGET, shader_state)]);
        if let Some((_, _, _, address, _)) = blocks.iter().find(|b| masked(b.1)).filter(|_| !(display && DISPLAY_ARMED.load(Ordering::Relaxed))) {
            list.OMSetRenderTargets(1, Some(&cx.target_view), false, None);
            list.SetGraphicsRootConstantBufferView(0, *address);
            list.SetGraphicsRootConstantBufferView(1, *address);
            list.RSSetScissorRects(&[RECT { left: 0, top: 0, right: w as i32, bottom: h as i32 }]);
            list.SetPipelineState(&looks.comp);
            list.DrawInstanced(3, 1, 0, 0);
        }
    }
    list.ResourceBarrier(&[transition(cx.depth, read_state, D3D12_RESOURCE_STATE_DEPTH_WRITE)]);
    if fades > 0 {
        FRAMES_FADE.fetch_add(1, Ordering::Relaxed);
    }
    if broke > 0 {
        FRAMES_BREAK.fetch_add(1, Ordering::Relaxed);
    }
    if blocks.iter().any(|b| b.1.hidden_for.is_none() && !b.4) {
        if isolate {
            FRAMES_ISOLATE.fetch_add(1, Ordering::Relaxed);
        } else {
            FRAMES_WHITE_OUT.fetch_add(1, Ordering::Relaxed);
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> View {
        // a camera 6 m behind the origin looking along +X, 1920 x 1080, 60 degrees across
        let camera = Vec3::new(-600.0, 0.0, 150.0);
        let (forward, right, up) = (Vec3::X, Vec3::Y, Vec3::Z);
        let tan_x = (30.0f32).to_radians().tan();
        let tan_y = tan_x * 1080.0 / 1920.0;
        // clip: x = right / tan_x, y = up / tan_y, w = forward (any z)
        let m = Mat4::from_cols(Vec4::new(0.0, 0.0, 0.0, 1.0), Vec4::new(1.0 / tan_x, 0.0, 0.0, 0.0), Vec4::new(0.0, 1.0 / tan_y, 0.0, 0.0), Vec4::new(0.0, 0.0, 1.0, 0.0));
        let clip = m * Mat4::from_translation(-camera);
        View { clip, camera, right, up, forward, depth_a: 1.0, depth_b: 0.0, proj00: 1.0 / tan_x, width: 1920, height: 1080 }
    }

    #[test]
    fn zfin_reach_covers_a_fallen_body_and_raised_arms() {
        let human = Capsule { feet: Vec3::ZERO, radius: 40.0, half_height: 90.0 };
        let (h, top) = reach(&human);
        assert!(h >= 180.0 && h < 260.0, "{h}");
        assert!(top >= 230.0, "{top}");
        // a giant: its reach grows with it
        let giant = Capsule { feet: Vec3::ZERO, radius: 150.0, half_height: 400.0 };
        assert!(reach(&giant).0 > 800.0);
    }

    #[test]
    fn zfin_screen_box_is_around_the_target_and_off_screen_is_none() {
        let v = view();
        let b = screen_box(&v.clip, Vec3::ZERO, 50.0, 0.0, 180.0, v.width, v.height, true).unwrap();
        assert!(b[0] < 960 && b[2] > 960 && b[1] < 540 + 100 && b[3] > 540, "{b:?}");
        assert!(b[2] - b[0] < 400, "{b:?}");
        // wholly behind the camera: nothing; across the camera plane: the whole screen for the pass, the front part for the ellipse
        assert_eq!(screen_box(&v.clip, Vec3::new(-2000.0, 0.0, 0.0), 50.0, 0.0, 180.0, v.width, v.height, true), None);
        assert_eq!(screen_box(&v.clip, Vec3::new(-600.0, 0.0, 0.0), 300.0, 0.0, 180.0, v.width, v.height, true), Some([0, 0, 1920, 1080]));
        let front = screen_box(&v.clip, Vec3::new(-600.0, 0.0, 0.0), 300.0, 0.0, 180.0, v.width, v.height, false).unwrap();
        assert!(front[0] >= 0 && front[2] <= 1920);
        assert_eq!(screen_box(&v.clip, Vec3::new(0.0, 5000.0, 0.0), 50.0, 0.0, 180.0, v.width, v.height, true), None);
    }

    #[test]
    fn zfin_block_rows_and_neighbours_nearest_first() {
        let v = view();
        let t = FinaleTarget {
            id: 7,
            body: Capsule { feet: Vec3::ZERO, radius: 40.0, half_height: 90.0 },
            immerse: 0.5,
            silhouette: true,
            crumble: 0.25,
            neighbours: (0..12).map(|k| Capsule { feet: Vec3::new(0.0, 100.0 + 30.0 * (11 - k) as f32, 0.0), radius: 35.0, half_height: 85.0 }).chain([Capsule { feet: Vec3::new(9000.0, 0.0, 0.0), radius: 35.0, half_height: 85.0 }]).collect(),
            hidden_for: None,
            push: Vec3::X,
            dark: 1.0,
        };
        let (scissor, f) = plan(&t, &v, 2, 1, 4.0, 0.3, true, true).unwrap();
        assert!(scissor[2] > scissor[0]);
        assert_eq!(&f[24..28], &[0.0, 0.0, 0.0, 40.0]); // feet + radius
        assert_eq!(&f[28..32], &[90.0, 0.5, 1.0, 0.25]);
        assert_eq!(f[33], 8.0); // eight neighbours kept, the far one dropped
        assert_eq!(f[44 + 1], 100.0); // the nearest first
        assert!(f[44 + 2 * 4 + 1] > f[44 + 1]);
        assert_eq!(f[23], 1.0); // isolating
        // the clip rows (the probe projects its points with them) and the memory slot / fade / id
        assert_eq!(&f[108..124], &v.clip.to_cols_array());
        assert_eq!(&f[124..128], &[2.0, 1.0, 0.0, 7.0]);
        let (_, f2) = plan(&t, &v, 0, 2, 4.0, 0.3, false, true).unwrap();
        assert_eq!((f2[3], f2[23]), (2.0, 0.0));
        let hidden = FinaleTarget { hidden_for: Some(0.15), ..t };
        let (_, f3) = plan(&hidden, &v, 0, 1, 4.0, 0.3, true, true).unwrap();
        assert!((f3[125] - 0.5).abs() < 1e-4, "half way through the fade: {}", f3[125]);
    }

    #[test]
    fn zfin_the_silhouette_cracks_then_breaks_top_first_and_is_gone_at_the_end() {
        let mut first = f32::MAX;
        for h in 0..=10 {
            for r in 0..=10 {
                for r2 in 0..=10 {
                    let (release, split, d0, d1) = timeline(h as f32 / 10.0, r as f32 / 10.0, r2 as f32 / 10.0);
                    first = first.min(release);
                    assert!(release < split && split < d0 && d0 < d1, "{release} {split} {d0} {d1}");
                    assert!(d1 <= 1.0 + 1e-6, "every fragment gone by the crumble's end ({d1})");
                }
            }
        }
        assert!(crack_reveal(0.0, 0.0) < first && crack_reveal(0.3, 0.5) < first, "cracks first");
        assert!(crack_reveal(1.0, 1.0) <= 0.2 + 1e-6, "the cracks spread over 0..0.2");
        assert!(timeline(1.0, 0.5, 0.5).0 < timeline(0.0, 0.5, 0.5).0, "the top goes before the feet");
        // the release window 0.16 .. 0.66
        assert!((timeline(1.0, 0.0, 0.0).0 - 0.16).abs() < 1e-6 && (timeline(0.0, 1.0, 0.0).0 - 0.66).abs() < 1e-6);
    }

    #[test]
    fn zfin_a_piece_flies_lands_and_comes_to_rest() {
        let p0 = Vec3::new(0.0, 0.0, 1.2);
        let v0 = Vec3::new(1.0, 0.5, 1.5);
        let (p, _, land) = fly(p0, v0, 0.0, 0.2);
        assert!(p.z > p0.z && land > 0.2, "it rises first ({p})");
        let (p, v, _) = fly(p0, v0, 0.0, land + 3.0);
        assert!(p.z.abs() < 1e-5 && v.length() < 0.01, "on the floor, at rest ({p}, {v})");
    }

    #[test]
    fn zfin_pieces_travel_along_the_beam_not_down() {
        // (round 6, the user: "needs to travel along the path of the beam, not just falling down")
        let push = Vec3::new(0.0, 1.0, 0.0);
        let p0 = Vec3::new(0.0, 0.0, 1.2);
        let v0 = push * 3.0 + Vec3::new(0.3, 0.0, 0.2);
        let (p, _) = swept(p0, v0, push, 0.0, 1.5);
        let d = p - p0;
        assert!(d.dot(push) > 5.0, "carried far down the beam ({d})");
        assert!(p0.z - p.z < d.dot(push) * 0.2, "it travels far more than it drops ({p})");
        assert!((d - push * d.dot(push)).length() < d.dot(push) * 0.35, "a slight spread off its axis ({d})");
    }

    #[test]
    fn zfin_snapshot_grid_and_rows() {
        assert_eq!(atlas_grid([100, 50, 300, 450]), (2, 100, 200));
        assert_eq!(atlas_grid([0, 0, 1920, 1080]), (8, 240, 135));
        assert_eq!(atlas_grid([10, 10, 60, 30]), (1, 50, 20));
        let snap = Snap { id: 3, x0: 100, y0: 50, step: 2, columns: 100, rows: 200, body: Capsule { feet: Vec3::new(1.0, 2.0, 3.0), radius: 40.0, half_height: 90.0 }, forward: Vec3::new(0.0, 2.0, 0.0) };
        let mut f = [0.0f32; ROWS * 4];
        snap_rows(&mut f, &snap, Vec3::new(0.0, 3.0, 0.0), 4.0);
        assert_eq!(&f[128..132], &[100.0, 50.0, 2.0, 100.0]);
        assert_eq!(&f[132..136], &[200.0, 20.0, 1.0, CRUMBLE_SECS]);
        assert_eq!(&f[136..140], &[0.0, 1.0, 0.0, 3.0], "the push made unit, the floor at its feet");
        assert_eq!(&f[140..144], &[1.0, 2.0, 3.0, 180.0]);
        assert_eq!(&f[148..152], &[0.0, 1.0, 0.0, 0.0], "the view at the snapshot: the pieces' thickness");
    }

    #[test]
    fn zfin_beam_style_rows_and_band() {
        // 3.5 % of the silhouette's height, within 1.5 .. 18 px of a 960-wide mask (scaled to the target)
        assert!((style_band(400.0, 960) - 400.0 * 0.035 / 2.6).abs() < 1e-4); // (ZFIN round 10)
        assert_eq!(style_band(10.0, 1920), 3.0);
        assert_eq!(style_band(5000.0, 1920), 36.0);
        let mut f = [0.0f32; ROWS * 4];
        style_rows(&mut f, 9.0, 2.0, 0.25);
        assert_eq!(&f[152..156], &[1.0, 9.0, 0.25, 0.0]);
        assert_eq!(&f[156..160], &[0.42, 0.04, 0.02, 2.0]);
        assert_eq!(&f[160..163], &[0.95, 0.38, 0.18]);
        assert_eq!(&f[164..167], &[1.0, 0.9, 0.82]);
    }

    #[test]
    fn zfin_fade_goes_from_one_to_zero_over_its_time() {
        assert_eq!(fade_alpha(0.0, 0.3), 1.0);
        assert!(fade_alpha(0.1, 0.3) > fade_alpha(0.2, 0.3));
        assert_eq!(fade_alpha(0.3, 0.3), 0.0);
        assert_eq!(fade_alpha(0.0, 0.0), 0.0, "no fade: gone at once");
    }

    #[test]
    fn zfin_over_materials_by_word() {
        let words = vec!["zfin_frag".to_string(), "c96_rit".to_string()];
        assert!(over_material("vfx_c96_RIT_00/M_dust", &words));
        assert!(over_material("party:zfin_frag_01", &words));
        assert!(!over_material("vfx_c96_zfr_00/M_beam", &words));
        assert!(!over_material("anything", &[]));
    }

    #[test]
    fn zfin_state_is_gone_when_not_sent_again() {
        set_mode(1, 0.0, -1.0);
        set(vec![FinaleTarget { id: 1, ..Default::default() }]);
        assert_eq!(current().len(), 1);
        assert!(active());
        set_mode(0, 0.0, -1.0);
        assert!(current().is_empty());
        set_mode(1, 0.0, -1.0);
        if let Ok(mut b) = BOOK.lock() {
            if let Some(b) = b.as_mut() {
                b.at -= Duration::from_millis(600);
            }
        }
        assert!(current().is_empty());
        set(Vec::new());
        assert!(!active());
    }

    #[test]
    fn zfin_depth_views_of_the_game_formats() {
        assert_eq!(depth_views(DXGI_FORMAT_R32G8X24_TYPELESS), Some((DXGI_FORMAT_R32_FLOAT_X8X24_TYPELESS, Some(DXGI_FORMAT_X32_TYPELESS_G8X24_UINT))));
        assert_eq!(depth_views(DXGI_FORMAT_R32_TYPELESS).unwrap().1, None);
        assert_eq!(depth_views(DXGI_FORMAT_R8G8B8A8_UNORM), None);
    }
}
