//! (effects) SORA'S VISUAL EFFECTS in the host's frame: the draw lists of `kh-vfx` (sprites,
//! ribbons, mesh particles, decals) drawn with the game's own effect shaders
//! (PACK/vfx/er/*.dxil, built by `cargo run --release -p kh-shaderc -- --vfx`) in OUR command
//! list, right after Sora (gfx.rs `draw`, the scene arrangement only).
//! State note: build/sora_tmp/er/IMPL_ER_FX_STATE.md.
//!
//! THREE THREADS, as the rest of the renderer:
//! - the HOST's (the game's frame task): `submit` turns a draw list into GPU-ready arrays
//!   (`Prepared`: one instance per quad and per mesh particle, plain vertices for ribbons and
//!   decal boxes, batched by material) and names what it needs in the registry;
//! - OUR WORKER (gfx.rs `worker` calls `worker_tick`): everything slow: the shading table, the
//!   textures (PNG), the meshes (glb), shader files, pipeline states, on a queue of its own;
//! - the game's SUBMISSION thread (gfx.rs `draw` calls `draw`): copies the arrays into the frame
//!   slot's upload buffer and records the draws. A material or mesh that is not ready yet is
//!   simply not drawn that frame.
//!
//! THE PASSES, in order, all with the game's scene depth:
//! 1. opaque and masked materials (mesh particles mostly): depth test and depth WRITE;
//! 2. the depth goes read-only (DEPTH_READ | PIXEL_SHADER_RESOURCE, a read-only view) so that
//!    the shaders can read it: distortion twins (each reads the copy of the scene colour made
//!    just before them), then decals (their boxes shade the scene behind each pixel), then
//!    translucent, additive and modulate batches from far to near;
//! 3. the depth goes back to DEPTH_WRITE (gfx.rs puts it where the game had it).
//!
//! BINDING CONTRACT (crates/kh-shaderc/src/vfx.rs has the same numbers): root parameters
//! 0 `ErFrame` (b0), 1 `KhParams` (b1), 2 a table t0..t11 (the material's ten textures, the two
//! shared ones), 3 a table t12..t13 (scene depth, scene colour copy), 4 naga's sampler heap,
//! 5 the sampler index buffer (t0 space1): one entry per texture = its address modes.
//!
//! SAFETY RULES kept: nothing here fails a frame (what cannot be made is skipped); every view of
//! a game resource is written in the frame it is used; every state we change is put back; a
//! device change drops everything and it is made again.

use std::collections::HashMap;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use glam::{Mat3, Mat4, Vec3};
use serde_json::Value;
use windows::core::{Interface, PCSTR};
use windows::Win32::Graphics::Direct3D::{ID3DBlob, D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST};
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;

use crate::rlog;
use crate::texture::Pixels;

// (VFXR) the effect layer's post passes (KH3's bloom on our effects, the screen-wide tricks): vfxpost.rs
#[path = "vfxpost.rs"]
mod post;
pub use post::{eye_boost, set_eye_adapt, set_post};
// (VFX-2) the effects' skeletal mesh components (TypeDataSQEX_SkeletalMesh): skelfx.rs
#[path = "skelfx.rs"]
mod skelfx;
// (PARTY-NATIVE-LIGHT) Elden Ring's own light on the KH lit translucent effects: the copy of its light inputs (vfxlight.rs)
#[path = "vfxlight.rs"]
mod light_kit;
pub(crate) use light_kit::{set_sun, FxSun};

/// (ORBS) The colour gain of a prize orb's effect (`prize.glow`, kh-er prize.rs and the harness's `--orbs`): the glow over the
/// eye factor the renderer already gives the eye-adaptation materials, so the orbs do not get it twice.
pub fn prize_gain(glow: f32) -> f32 {
    glow / eye_boost().max(1.0e-6)
}
/// (VFXR) the post kit (its own lock: made on the worker after the GPU objects, used under a try-lock while drawing)
static POST: Mutex<Option<post::PostKit>> = Mutex::new(None);
static POST_FAILED: Mutex<Option<String>> = Mutex::new(None);

const FRAME_SLOTS: usize = 3;
/// Bytes of a frame slot's upload buffer (instances, plain vertices, constants).
const SLOT_BYTES: usize = 16 * 1024 * 1024;
const MATERIAL_TEXTURES: usize = 12;
const MAX_MATERIALS: usize = 2048;
const QUAD_FLOATS: usize = 36;
const MESH_INSTANCE_FLOATS: usize = 24;
const PLAIN_FLOATS: usize = 33;
const MESH_VERTEX_FLOATS: usize = 18;
/// Rows of `KhParams`: 64 of the material, misc, er, (PARTY-NATIVE) fx (the game's fog mode: `fog_mode`).
const PARAM_ROWS: usize = 67;
/// Sampler heap entry of (address U, address V) = 3 * U + V with wrap 0, clamp 1, mirror 2; then the scene's.
const SAMPLER_SCENE: u32 = 9;

/// What a material is drawn on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Geo {
    /// Camera-facing quads (the sprite vertex factory).
    Quad,
    /// A ribbon (the sprite factory's shader on a strip).
    Ribbon,
    Mesh,
    Decal,
}

/// The switches of the effect drawing (the host's control keys `vfx*`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VfxOptions {
    /// `vfx`: draw effects at all.
    pub on: bool,
    /// `vfx.distort`: the distortion (refraction) twins, which need a copy of the scene colour.
    pub distort: bool,
    /// `vfx.hdr`: a factor on the effects' brightness in the host's scene.
    pub hdr: f32,
    /// `vfx.solid`: diagnosis: what the geometry covers, in magenta.
    pub solid: bool,
    /// `vfx.decals`: the projected decals.
    pub decals: bool,
    /// `vfx.late`: decals, translucent and additive effects are drawn AFTER the game's forward
    /// stage (its own translucents, water, particles) instead of right after Sora (IMPL_ER7_STATE.md).
    pub late: bool,
}

impl Default for VfxOptions {
    fn default() -> VfxOptions {
        VfxOptions { on: true, distort: true, hdr: 1.0, solid: false, decals: true, late: true }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct MatKey {
    name: String,
    geo: Geo,
    blend: kh_vfx::Blend,
}

struct QuadBatch {
    /// (VFXR sort) the batch's draw group's key (`kh_vfx::DrawGroup::key`): the translucent order
    sort: SortKey,
    material: u32,
    /// (DON-MAGIC tint) the batch's recolour (r, g, b, intensity; zero: none) -> KhParams.er.yzw
    tint: [f32; 4],
    data: Vec<f32>,
    center: Vec3,
}

struct MeshBatch {
    sort: SortKey, // (VFXR sort)
    material: u32,
    tint: [f32; 4], // (DON-MAGIC tint)
    mesh: u32,
    data: Vec<f32>,
    center: Vec3,
}

struct PlainBatch {
    sort: SortKey, // (VFXR sort)
    material: u32,
    tint: [f32; 4], // (DON-MAGIC tint)
    vertices: Vec<f32>,
    indices: Vec<u32>,
    center: Vec3,
    decal: bool,
}

/// (VFXR sort) KH3's translucency order of a batch: (priority, -(distance - offset), system, emitter), ascending = drawn
/// first (kh_vfx::DrawGroup::key). A batch holds the draws of ONE emitter group (the maps below key by it), so a backing card
/// and the glow over it never share a batch whose centre decides their order.
type SortKey = (i32, f32, u64, u32);

/// (VFXR sort) `KH_ER_VFX_SORT=0` / ctl `vfx.sort = 0`: the batches by material across emitters and far-to-near by their centre,
/// as before.
pub static SORT_GROUPS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// (PARTY-NATIVE) ctl `vfx.fxlit = 0` (a TEST switch, default on): the lit translucent effects draw their fog variant instead of
/// their lit one (the A / B of Elden Ring's light on them).
pub static LIT_VARIANTS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// (VFXR sort) The group's key of item `index` (`table`: `DrawList::group_table`), or the plain distance key of `at`.
fn sort_of(list: &kh_vfx::DrawList, table: &[usize], index: usize, camera: Vec3, at: Vec3) -> (SortKey, usize) {
    let on = SORT_GROUPS.load(Ordering::Relaxed);
    match table.get(index).copied().filter(|g| on && *g != usize::MAX) {
        Some(gi) => (list.groups[gi].key(camera), gi),
        None => ((0, -at.distance(camera), 0, 0), usize::MAX),
    }
}

/// One frame of effects, ready for the GPU.
#[derive(Default)]
struct Prepared {
    quads: Vec<QuadBatch>,
    meshes: Vec<MeshBatch>,
    plain: Vec<PlainBatch>,
    /// The effect clock (seconds), for the few shaders that read the game time.
    seconds: f32,
    particles: usize,
    /// (VFXR) the screen-wide passes the effects ask for (one of each kind: kh_vfx::screen::pick)
    screen: Vec<kh_vfx::ScreenFx>,
    /// (ZFIN) the materials drawn over Zettaflare's finale (`r.finale_over`: ZETTA's crumble fragments; finale.rs)
    over: Vec<u32>,
    /// (VFX-2) the skeletal mesh components, posed and skinned (skelfx.rs)
    skel: Vec<skelfx::Posed>,
}

/// The instance arrays of frames that are over, for the next ones (a volley of the shotlock is
/// megabytes a frame: growing fresh vectors every frame cost more than filling them).
static POOL: Mutex<Vec<Vec<f32>>> = Mutex::new(Vec::new());

fn pooled() -> Vec<f32> {
    // (the largest first: the big batches come first in a list)
    let mut pool = POOL.lock().unwrap();
    let best = (0..pool.len()).max_by_key(|k| pool[*k].capacity());
    best.map(|k| pool.swap_remove(k)).unwrap_or_default()
}

impl Drop for Prepared {
    fn drop(&mut self) {
        if let Ok(mut pool) = POOL.lock() {
            for mut data in self.quads.drain(..).map(|b| b.data).chain(self.meshes.drain(..).map(|b| b.data)) {
                if pool.len() < 64 && data.capacity() > 0 {
                    data.clear();
                    pool.push(data);
                }
            }
        }
    }
}

#[derive(Default)]
struct Registry {
    keys: Vec<MatKey>,
    index: HashMap<MatKey, u32>,
    mesh_names: Vec<String>,
    mesh_index: HashMap<String, u32>,
}

impl Registry {
    fn material(&mut self, name: &str, geo: Geo, blend: kh_vfx::Blend) -> Option<u32> {
        let key = MatKey { name: name.to_string(), geo, blend };
        if let Some(id) = self.index.get(&key) {
            return Some(*id);
        }
        if self.keys.len() >= MAX_MATERIALS {
            return None;
        }
        let id = self.keys.len() as u32;
        self.keys.push(key.clone());
        self.index.insert(key, id);
        Some(id)
    }

    fn mesh(&mut self, name: &str) -> u32 {
        if let Some(id) = self.mesh_index.get(name) {
            return *id;
        }
        let id = self.mesh_names.len() as u32;
        self.mesh_names.push(name.to_string());
        self.mesh_index.insert(name.to_string(), id);
        id
    }
}

static REGISTRY: Mutex<Option<Registry>> = Mutex::new(None);
static LATEST: Mutex<Option<Arc<Prepared>>> = Mutex::new(None);
static OPTIONS: Mutex<Option<VfxOptions>> = Mutex::new(None);
static GPU: Mutex<Option<Gpu>> = Mutex::new(None);
/// The formats of the target and the depth the draws go to (the worker makes pipeline states for them).
static FORMATS: Mutex<Option<(i32, i32)>> = Mutex::new(None);

// ---------------------------------------------------------------------------------------------------- (PARTY-NATIVE)
// THE LIT (DEFERRED) EFFECTS ON THE CHARACTERS' NATIVE ROUTE (er_vfx.wgsl KH_FXNAT / KH_FX_GB; the user 2026-11-05: "Donald and
// Goofy need to be rendering how Sora is rendering as well, as well as all of their effects"). A material whose KH3 shader writes
// the G-buffer (outputs o1..o3) and has its native pair in its root (`n<letter><id>` / `g<letter><id>`, kh-shaderc --vfx --only
// fxnat) is drawn as KH3 draws it: its G-buffer twin goes into the game's G-buffer in the early list beside the characters'
// (`gbuffer_replay`, called by gfx.rs's G-buffer pass), the game's light passes light it, the characters' capture takes that
// light, and its colour pass composes KH3's deferred result from it. Without a capture this frame, or without the pair, the
// earlier shading stays.

/// The frame constants' native row (er_vfx.wgsl `ErFrame.fxn`), appended after the forward rows.
const FXN_FLOATS: usize = 4;
/// Views of each frame slot's table (t12 the scene depth, t13 the scene colour copy, t14 / t15 the capture of the game's light,
/// t16 / t17 the copy of the game's fog volume and its constants; (PARTY-NATIVE-LIGHT) t18 the lit rows, t19..t26 the irradiance
/// volumes, t27 the light buffers, t28 / t29 the sun's cascade atlas and constants: vfxlight.rs `write_views`).
const FRAME_VIEWS: usize = 18;
/// G-buffer draws a frame slot keeps for the replay.
const MAX_REPLAYS: usize = 4096;

/// (PARTY-NATIVE) This frame's capture of the game's light (gfx.rs `HyCapture`: the light and geometry targets, raw pointers, in
/// the native layout with the game's specular), the light-set mark of the KH G-buffer white, the normal match (cos).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FxCapture {
    pub light: usize,
    pub geo: usize,
    pub mark: f32,
    pub cos: f32,
}

static FX_CAPTURE: Mutex<Option<FxCapture>> = Mutex::new(None);
/// (PARTY-NATIVE) The formats of the game's three G-buffer targets the KH draws write and of its depth (gfx.rs, from the
/// hybrid's plan of the frame): the G-buffer twins' pipeline states are made for them.
static GB_FORMATS: Mutex<Option<GbFormats>> = Mutex::new(None);
/// (PARTY-NATIVE) The G-buffer draws recorded with this frame's colour pass, per frame slot (replayed by `gbuffer_replay`).
static REPLAYS: Mutex<Vec<(usize, Vec<Replay>)>> = Mutex::new(Vec::new());
static NATIVE_DRAWS: AtomicU32 = AtomicU32::new(0);
static GB_DRAWS: AtomicU32 = AtomicU32::new(0);
/// (PARTY-NATIVE) frames with the copy of the game's fog drawn, late batches drawn with their fog variant
static FOG_FRAMES: AtomicU32 = AtomicU32::new(0);
static FOG_BATCHES: AtomicU32 = AtomicU32::new(0);
/// (PARTY-NATIVE-LIGHT) frames with the copy of the game's light inputs drawn, late batches drawn with their lit variant
static LIT_FRAMES: AtomicU32 = AtomicU32::new(0);
static LIT_BATCHES: AtomicU32 = AtomicU32::new(0);
/// (round 3) late batches drawn with their plain variant (no lit / fog variant taken), and the lit-variant lines told
static PLAIN_BATCHES: AtomicU32 = AtomicU32::new(0);
static LIT_TOLD: AtomicU32 = AtomicU32::new(0);

/// (the three render targets' formats, the depth's)
pub(crate) type GbFormats = ([i32; 3], i32);

/// (PARTY-NATIVE) gfx.rs, once a frame after the capture: the capture for the lit effects (None: no native capture this frame).
pub(crate) fn set_capture(capture: Option<FxCapture>) {
    if let Ok(mut c) = FX_CAPTURE.lock() {
        *c = capture;
    }
}

/// (PARTY-NATIVE) gfx.rs, with the hybrid's plan of the frame: the G-buffer's formats.
pub(crate) fn set_gb_formats(formats: Option<GbFormats>) {
    if let Ok(mut f) = GB_FORMATS.lock() {
        if f.is_none() || formats.is_some() {
            *f = formats;
        }
    }
}

/// (PARTY-NATIVE) The native row of the frame constants (pure, tested).
fn fxn_row(capture: Option<FxCapture>) -> [f32; FXN_FLOATS] {
    match capture {
        Some(c) => [1.0, c.mark, c.cos.clamp(-1.0, 1.0), 0.0],
        None => [0.0; FXN_FLOATS],
    }
}

/// (PARTY-NATIVE) A G-buffer draw kept for the replay: its state, root arguments and geometry.
struct Replay {
    pso: ID3D12PipelineState,
    frame: u64,
    material: u64,
    samplers: u64,
    table: u64,
    vertex: Vec<D3D12_VERTEX_BUFFER_VIEW>,
    index: Option<D3D12_INDEX_BUFFER_VIEW>,
    count: u32,
    instances: u32,
}

// SAFETY: as Gpu (D3D12 objects are free-threaded; the views are plain GPU addresses).
unsafe impl Send for Replay {}

/// (PARTY-NATIVE) A material's native pair: the colour pass from the game's light (and its mirrored twin), and the G-buffer
/// twin's files with its states for the formats they were made for.
struct NativeFx {
    colour: ID3D12PipelineState,
    colour_mirrored: Option<ID3D12PipelineState>,
    dir: PathBuf,
    vertex: String,
    pixel: String,
    geo: Geo,
    cull: D3D12_CULL_MODE,
    one_sided: bool,
    gb: Option<(GbFormats, ID3D12PipelineState, Option<ID3D12PipelineState>)>,
    gb_failed: bool,
}

/// (PARTY-NATIVE) The native pair's file names of a translated variant (`<letter><id><suffix>`: an overlay's changed translation
/// carries its suffix, `_vxl` / `_xwd`), None for a decal or a shader without G-buffer outputs (pure, tested). Every KH3 decal of
/// the pack compiles to ONE target (FDeferredDecalPS `outputs` 1: o0 = its colour / the eye adaptation, DBM_SQEX_Overwrite /
/// Translucent / Emissive): KH3's lights never reach a decal, so it has no pair.
fn native_files(letter: &str, shader_id: u64, outputs: u64, suffix: &str) -> Option<(String, String)> {
    (letter != "d" && outputs >= 4).then(|| (format!("n{letter}{shader_id}{suffix}.ps.dxil"), format!("g{letter}{shader_id}{suffix}.ps.dxil")))
}
// Numbers for the log (since the last `status`).
static DRAWN_FRAMES: AtomicU32 = AtomicU32::new(0);
static DRAWN_BATCHES: AtomicU32 = AtomicU32::new(0);
static DRAWN_INSTANCES: AtomicU64 = AtomicU64::new(0);
static NOT_READY: AtomicU32 = AtomicU32::new(0);
static OVERFLOW: AtomicU32 = AtomicU32::new(0);
static PREPARE_MICROS: AtomicU64 = AtomicU64::new(0);
static PREPARE_MAX: AtomicU64 = AtomicU64::new(0);
static DRAW_MICROS: AtomicU64 = AtomicU64::new(0);
static DRAW_MAX: AtomicU64 = AtomicU64::new(0);
static PARTICLES_MAX: AtomicU64 = AtomicU64::new(0);

pub fn set_options(options: VfxOptions) {
    if let Ok(mut guard) = OPTIONS.lock() {
        if *guard != Some(options) {
            rlog!("vfx: options {options:?}");
            *guard = Some(options);
        }
    }
}

pub(crate) fn options() -> VfxOptions {
    OPTIONS.lock().ok().and_then(|g| *g).unwrap_or_default()
}

/// THE SCENE LIGHT the effects ask for (`SQEX_ChangeSceneColor`: Thunder, the grand spells; 1 =
/// untouched), as bits of an f32. The host sets it every frame; the late pass multiplies the
/// scene by its square (the test app's calibration) before the late effects are drawn.
static SCENE_LIGHT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0x3F80_0000);
pub static DIMMED_FRAMES: AtomicU32 = AtomicU32::new(0);

pub fn set_scene_light(light: f32) {
    SCENE_LIGHT.store(if light.is_finite() { light.clamp(0.0, 1.0) } else { 1.0 }.to_bits(), Ordering::Relaxed);
}

/// The factor the scene is multiplied by (1 = nothing to do).
pub(crate) fn scene_dim() -> f32 {
    let light = f32::from_bits(SCENE_LIGHT.load(Ordering::Relaxed));
    light * light * crate::darkscreen::coefficient() // (AUDIT-A CAMS) x KH3's master light coefficient (the finishes' dark screen)
}

/// Which part of the frame's effects one call of [`draw`] records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Stage {
    /// Everything, right after Sora (the first arrangement).
    All,
    /// Opaque and masked effects only (they write depth), right after Sora.
    Early,
    /// Decals, translucent, additive and the distortion twins: after the game's forward stage.
    Late,
    /// (IMPL_ER74) The decals alone, BEFORE Sora (he covers them): a decal draws with no depth test and puts its pattern
    /// on whatever surface the scene depth shows inside its box, and the scene depth holds him (his early depth). Drawn
    /// first in the frame's list; the other stages then leave the decals out.
    Decals,
    /// (ZFIN) The translucent / additive effects of the materials over Zettaflare's finale (`r.finale_over`), after it (gfx.rs:
    /// right after finale::draw); while a finale is drawn the stage that draws them otherwise (All / Late) leaves them out.
    OverFinale,
}

/// (IMPL_ER74) our decals are drawn before Sora (`Stage::Decals`); false = after him as before.
pub static DECALS_EARLY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// (VFXR occlusion) `vfx.decal_chars = 0` turns it off: our decals skip the pixels of the CHARACTERS, by the stencil class
/// Elden Ring's characters carry in the scene depth (gfx.rs `CHAR_STENCIL` 0x50: bits 6-7 = 01, measured; terrain 0x12 /
/// 0xd2 / 0x90, map objects 0x10, sky 0) and our KH characters' early depth writes too (r.taa_char): a ground decal
/// (Zettaflare's circle, Thundaga's cracks) no longer paints its pattern up an enemy, the Tarnished, Sora or the party,
/// nor feeds that pattern to the decals' bloom (KH3's characters take no decals). Read when the pipelines are made.
pub static DECAL_SKIP_CHARS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// (VFXR cinematic) `vfx.cinematic = 0` ignores a sequence's look (kh_core::host::cinematic). On: its bloom threshold drives
/// our effects' bloom, its exposure / key EV our KH characters' ambient / key light (gfx.rs, through `cine_options`).
/// Elden Ring's own world lighting and exposure are never touched (no safe, restorable global override is known here), and
/// our effects keep their brightness under an exposure change, as KH3's (they divide by the eye adaptation).
/// (VFXR decal eye) `vfx.eye_decals = 1`: the deferred decals take the eye-adaptation boost too (the shader rule); default
/// off: KH3 footage (the user's Zettaflare GIF, its circle orange with white lines) matches them without it
/// (build/sora_tmp/vfxr/shots/decal_eye). The test app's twin: KH_VFX_EYE_DECALS=1.
pub static EYE_DECALS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// (VFXR decal bloom) `vfx.decal_bloom = 1`: the decals' share feeds our effects' bloom; default off: in the user's KH3 footage
/// Zettaflare's circle fills the frame without veiling Donald (build/sora_tmp/vfxr/shots/bloom_cause). Test app twin:
/// KH_VFX_DECAL_BLOOM=1.
pub static DECAL_BLOOM: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub static CINEMATIC: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// (VFXR cinematic) The look this frame: (exposure EV, key EV, bloom threshold), handed over by the host once a frame
/// (kh-er fx.rs: kh_core::host::cinematic::step) through [`set_cine_look`].
static CINE_LOOK: Mutex<(f32, f32, f32)> = Mutex::new((0.0, 0.0, -1.0));

pub fn set_cine_look(exposure_ev: f32, key_ev: f32, bloom_threshold: f32) {
    if let Ok(mut l) = CINE_LOOK.lock() {
        *l = (exposure_ev, key_ev, bloom_threshold);
    }
}

/// (VFXR cinematic) The look to apply (neutral when `vfx.cinematic = 0`).
pub fn cine_look() -> (f32, f32, f32) {
    if CINEMATIC.load(std::sync::atomic::Ordering::Relaxed) { CINE_LOOK.lock().map(|l| *l).unwrap_or((0.0, 0.0, -1.0)) } else { (0.0, 0.0, -1.0) }
}

/// (VFXR cinematic) The render options with the look on our characters' light: key x 2^(exposure + key EV), ambient x
/// 2^exposure EV. Identity at the neutral look.
pub(crate) fn cine_options(mut options: crate::Options) -> crate::Options {
    let (exposure, key, _) = cine_look();
    if exposure != 0.0 || key != 0.0 {
        options.key *= (exposure + key).exp2();
        options.ambient *= exposure.exp2();
    }
    options
}

pub fn set_decal_skip_chars(on: bool) {
    DECAL_SKIP_CHARS.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// (VFXR occlusion) The stencil test of a decal pipeline: pass where the class bits differ from the characters'.
fn decal_stencil(depth_format: DXGI_FORMAT) -> Option<(u8, D3D12_DEPTH_STENCILOP_DESC)> {
    let has_stencil = matches!(depth_format, DXGI_FORMAT_D32_FLOAT_S8X24_UINT | DXGI_FORMAT_D24_UNORM_S8_UINT);
    (has_stencil && DECAL_SKIP_CHARS.load(std::sync::atomic::Ordering::Relaxed)).then_some((0xC0, D3D12_DEPTH_STENCILOP_DESC { StencilFailOp: D3D12_STENCIL_OP_KEEP, StencilDepthFailOp: D3D12_STENCIL_OP_KEEP, StencilPassOp: D3D12_STENCIL_OP_KEEP, StencilFunc: D3D12_COMPARISON_FUNC_NOT_EQUAL }))
}

/// Where each frame slot's upload buffer is filled to (the late call goes on behind the early one).
static CURSORS: Mutex<[usize; 8]> = Mutex::new([0; 8]);

/// Nothing is drawn until the host submits again.
pub fn clear() {
    if let Ok(mut guard) = LATEST.lock() {
        *guard = None;
    }
    if let Ok(mut r) = ROUTED.lock() {
        r.clear();
    }
}

/// (party VF-b) The mesh particles the character pipeline draws instead (xsiweapon.rs `shard_model`: the chandelier's
/// fragments with their own material), as forms of the frame; at most ROUTED_MAX (each costs a skin and a material block of
/// the character pipeline's upload buffer).
static ROUTED: Mutex<Vec<crate::xsiweapon::FormWeapon>> = Mutex::new(Vec::new());
const ROUTED_MAX: usize = 160;
static ROUTED_PEAK: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// (party VF-b) This frame's routed fragments (gfx.rs adds them to the frame's forms).
pub fn routed_forms() -> Vec<crate::xsiweapon::FormWeapon> {
    ROUTED.lock().map(|r| r.clone()).unwrap_or_default()
}

/// Makes ready beforehand: (material, what it is drawn on, the emitter's blend) and meshes.
pub fn preload(materials: &[(String, Geo, kh_vfx::Blend)], meshes: &[String]) {
    if let Ok(mut guard) = REGISTRY.lock() {
        let registry = guard.get_or_insert_with(Registry::default);
        for (name, geo, blend) in materials {
            registry.material(name, *geo, *blend);
        }
        for mesh in meshes {
            registry.mesh(mesh);
        }
    }
}

/// How many registered materials and meshes are not made yet (`None`: our objects do not exist
/// yet: no frame has been drawn in the scene arrangement). For a harness that waits for them.
pub fn pending() -> Option<usize> {
    let registry = REGISTRY.lock().ok()?;
    let (materials, meshes) = registry.as_ref().map_or((0, 0), |r| (r.keys.len(), r.mesh_names.len()));
    let guard = GPU.lock().ok()?;
    let gpu = guard.as_ref()?;
    let done = |k: usize| matches!(gpu.materials.get(k), Some(MatState::Ready(_) | MatState::Failed));
    let mesh_done = |k: usize| matches!(gpu.meshes.get(k), Some(MeshState::Ready(_) | MeshState::Failed));
    Some((0..materials).filter(|k| !done(*k)).count() + (0..meshes).filter(|k| !mesh_done(*k)).count())
}

/// (diagnosis) What the latest submitted frame holds: one line per batch (material, geometry,
/// blend, instances, how it is drawn or why not).
pub fn describe_latest() -> Vec<String> {
    let Some(prepared) = LATEST.lock().ok().and_then(|g| g.clone()) else { return Vec::new() };
    let registry = REGISTRY.lock().unwrap();
    let Some(registry) = registry.as_ref() else { return Vec::new() };
    let guard = GPU.lock().unwrap();
    let state = |id: u32| match guard.as_ref().and_then(|g| g.materials.get(id as usize)) {
        Some(MatState::Ready(m)) => format!("{:?}{}{}", m.pass, if m.distort.is_some() { " +distortion" } else { "" }, if m.timed { " timed" } else { "" }),
        Some(MatState::Failed) => "NOT DRAWABLE".to_string(),
        _ => "not ready".to_string(),
    };
    let key = |id: u32| registry.keys.get(id as usize).map_or("?".to_string(), |k| format!("{} {:?} {:?}", k.name, k.geo, k.blend));
    let mut lines = Vec::new();
    for b in &prepared.quads {
        lines.push(format!("quads x{:5}  {}  -> {}", b.data.len() / QUAD_FLOATS, key(b.material), state(b.material)));
    }
    for b in &prepared.meshes {
        lines.push(format!("mesh  x{:5}  {} on {}  -> {}", b.data.len() / MESH_INSTANCE_FLOATS, key(b.material), registry.mesh_names.get(b.mesh as usize).map_or("?", |n| n.as_str()), state(b.material)));
    }
    for b in &prepared.plain {
        lines.push(format!("{} {:5} vertices  {}  -> {}", if b.decal { "decal" } else { "strip" }, b.vertices.len() / PLAIN_FLOATS, key(b.material), state(b.material)));
    }
    lines
}

/// One line for the host's once-a-second log; resets the counters it reports.
pub fn status() -> String {
    let frames = DRAWN_FRAMES.swap(0, Ordering::Relaxed).max(1) as u64;
    let (ready, pending, failed, meshes, textures, bytes) = GPU.lock().ok().and_then(|g| g.as_ref().map(|g| (g.materials.iter().filter(|m| matches!(m, MatState::Ready(_))).count(), g.pending_materials, g.materials.iter().filter(|m| matches!(m, MatState::Failed)).count(), g.meshes.iter().filter(|m| matches!(m, MeshState::Ready(_))).count(), g.textures.len(), g.texture_bytes))).unwrap_or((0, 0, 0, 0, 0, 0));
    // (PARTY-NATIVE) the lit materials on the native route: with their pair, with its G-buffer states for the formats
    let (paired, gb_ready) = GPU.lock().ok().and_then(|g| g.as_ref().map(|g| {
        let natives = g.materials.iter().filter_map(|m| match m {
            MatState::Ready(m) => m.native.as_ref(),
            _ => None,
        });
        let formats = GB_FORMATS.lock().ok().and_then(|f| *f);
        (natives.clone().count(), natives.filter(|n| n.gb.as_ref().is_some_and(|x| Some(x.0) == formats)).count())
    })).unwrap_or((0, 0));
    // (PARTY-NATIVE-LIGHT) Elden Ring's own light on the lit translucent effects
    let lit_made = GPU.lock().ok().and_then(|g| g.as_ref().map(|g| g.materials.iter().filter(|m| matches!(m, MatState::Ready(m) if m.lit.is_some())).count())).unwrap_or(0);
    let lit_text = format!(
        "(PARTY-NATIVE-LIGHT) the lit translucent effects in Elden Ring's own light: {lit_made} material(s) with their lit variant; {} frame(s) with the copy of the game's light inputs; blended batches this second by variant: lit {}, fog {}, plain {}; {}",
        LIT_FRAMES.swap(0, Ordering::Relaxed),
        LIT_BATCHES.swap(0, Ordering::Relaxed),
        FOG_BATCHES.load(Ordering::Relaxed),
        PLAIN_BATCHES.swap(0, Ordering::Relaxed),
        GPU.lock().ok().and_then(|g| g.as_ref().map(|g| match (&g.lit, &g.lit_failed) {
            (Some(k), _) => format!("{}; {}", k.status(), light_kit::frames_status()),
            (None, Some(e)) => format!("not made: {e}"),
            _ => "not made yet".to_string(),
        })).unwrap_or_default()
    );
    let native = format!("(PARTY-NATIVE) lit effects on the native route: {paired} material(s) with their pair, {gb_ready} with G-buffer states; {} colour batch(es) from the game's light, {} G-buffer draw(s) this second; capture {}; the game's fog on the late effects: {} frame(s), {} batch(es) with their fog variant, {}; {}", NATIVE_DRAWS.swap(0, Ordering::Relaxed), GB_DRAWS.swap(0, Ordering::Relaxed), if FX_CAPTURE.lock().ok().and_then(|c| *c).is_some() { "bound" } else { "none" }, FOG_FRAMES.swap(0, Ordering::Relaxed), FOG_BATCHES.swap(0, Ordering::Relaxed), GPU.lock().ok().and_then(|g| g.as_ref().map(|g| match (&g.fog, &g.fog_failed) { (Some(k), _) => format!("volume {:?}{}", k.size, if k.volume.is_some() { "" } else { " (no copy yet)" }), (None, Some(e)) => format!("not made: {e}"), _ => "not made yet".to_string() })).unwrap_or_default(), crate::ffx::status());
    format!(
        "vfx: {} | per frame: {} batches, {} instances; prepare {} us (max {}), draw {} us (max {}); peak {} particles | {} not ready, {} cut (buffer full) | materials {ready} ready, {pending} waiting, {failed} not drawable; {meshes} meshes, {textures} textures ({} MB) | {native} | {lit_text} | (VFXR) {}",
        if options().on { "on" } else { "OFF" },
        DRAWN_BATCHES.swap(0, Ordering::Relaxed) as u64 / frames,
        DRAWN_INSTANCES.swap(0, Ordering::Relaxed) / frames,
        PREPARE_MICROS.swap(0, Ordering::Relaxed) / frames,
        PREPARE_MAX.swap(0, Ordering::Relaxed),
        DRAW_MICROS.swap(0, Ordering::Relaxed) / frames,
        DRAW_MAX.swap(0, Ordering::Relaxed),
        PARTICLES_MAX.swap(0, Ordering::Relaxed),
        NOT_READY.swap(0, Ordering::Relaxed),
        OVERFLOW.swap(0, Ordering::Relaxed),
        bytes / 1_048_576,
        post::status(),
    )
}

// ------------------------------------------------------------------------------------ the host's side

fn clamp_color(c: [f32; 4]) -> [f32; 4] {
    [c[0].max(0.0), c[1].max(0.0), c[2].max(0.0), c[3].clamp(0.0, 1.0e4)]
}

/// A ribbon as a triangle strip, smoothed with a Catmull-Rom curve between its samples (as the
/// test app's `ribbon_geometry`), in GAME space.
fn ribbon_geometry(vertices: &mut Vec<f32>, indices: &mut Vec<u32>, ribbon: &kh_vfx::Ribbon, view: &kh_vfx::Camera) {
    let points = &ribbon.points;
    if points.len() < 2 {
        return;
    }
    let spline = |p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f32| 0.5 * ((2.0 * p1) + (p2 - p0) * t + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t * t + (3.0 * p1 - p0 - 3.0 * p2 + p3) * t * t * t);
    let lerp4 = |a: [f32; 4], b: [f32; 4], t: f32| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t, a[3] + (b[3] - a[3]) * t];
    let first = (vertices.len() / PLAIN_FLOATS) as u32;
    let mut rows = 0u32;
    let last = points.len() - 1;
    let misc = [ribbon.time, 0.0, 0.0, 1.0];
    let mut row = |a: Vec3, b: Vec3, u: f32, color: [f32; 4], along: Vec3| {
        let mid = (a + b) * 0.5;
        let normal = (view.position - mid).normalize_or(-view.forward);
        let tangent = along.normalize_or(view.right);
        let particle = [mid.x, mid.y, mid.z, a.distance(b)];
        let (ua, ub) = if ribbon.uv_turned { ([0.0, u * ribbon.tiles], [ribbon.v_scale, u * ribbon.tiles]) } else { ([u * ribbon.tiles, 0.0], [u * ribbon.tiles, ribbon.v_scale]) };
        let shared = PlainShared { color, vcol: [1.0; 4], dynamic: ribbon.dynamic, misc, normal, tangent, particle };
        push_plain(vertices, a, [ua[0], ua[1], ua[0], ua[1]], &shared);
        push_plain(vertices, b, [ub[0], ub[1], ub[0], ub[1]], &shared);
    };
    for i in 0..last {
        let (p0, p1, p2, p3) = (&points[i.saturating_sub(1)], &points[i], &points[i + 1], &points[(i + 2).min(last)]);
        let span = p1.a.distance(p2.a).max(p1.b.distance(p2.b));
        let parts = ((span / 8.0).ceil() as usize).clamp(1, 8);
        for k in 0..parts {
            let t = k as f32 / parts as f32;
            let u = p1.u + (p2.u - p1.u) * t;
            row(spline(p0.a, p1.a, p2.a, p3.a, t), spline(p0.b, p1.b, p2.b, p3.b, t), u, lerp4(p1.color, p2.color, t), p2.a - p1.a);
            rows += 1;
        }
    }
    let end = &points[last];
    row(end.a, end.b, end.u, end.color, end.a - points[last - 1].a);
    rows += 1;
    for r in 0..rows - 1 {
        let b = first + r * 2;
        indices.extend([b, b + 1, b + 2, b + 1, b + 3, b + 2]);
    }
}

/// (VFX-RING S) An EXACT ribbon (`kh_vfx::RibbonExact`, a dynamic shape's strip): one straight row per point, no smoothing,
/// the A vertex with the point's colour and its own UV, the B vertex with the B colour and its UV (KH3's trail vertices,
/// 0x144320580), in GAME space.
fn ribbon_exact_geometry(vertices: &mut Vec<f32>, indices: &mut Vec<u32>, ribbon: &kh_vfx::Ribbon, exact: &kh_vfx::RibbonExact, view: &kh_vfx::Camera) {
    let points = &ribbon.points;
    if points.len() < 2 || exact.uvs.len() != points.len() || exact.b_colors.len() != points.len() {
        return;
    }
    let first = (vertices.len() / PLAIN_FLOATS) as u32;
    let misc = [ribbon.time, 0.0, 0.0, 1.0];
    let last = points.len() - 1;
    for (i, p) in points.iter().enumerate() {
        let along = if i < last { points[i + 1].a - p.a } else { p.a - points[i - 1].a };
        let mid = (p.a + p.b) * 0.5;
        let normal = (view.position - mid).normalize_or(-view.forward);
        let tangent = along.normalize_or(view.right);
        let particle = [mid.x, mid.y, mid.z, p.a.distance(p.b)];
        let uv = exact.uvs[i];
        push_plain(vertices, p.a, [uv[0], uv[1], uv[0], uv[1]], &PlainShared { color: p.color, vcol: [1.0; 4], dynamic: ribbon.dynamic, misc, normal, tangent, particle });
        push_plain(vertices, p.b, [uv[2], uv[3], uv[2], uv[3]], &PlainShared { color: exact.b_colors[i], vcol: [1.0; 4], dynamic: ribbon.dynamic, misc, normal, tangent, particle });
    }
    for r in 0..last as u32 {
        let b = first + r * 2;
        indices.extend([b, b + 1, b + 2, b + 1, b + 3, b + 2]);
    }
}

/// What the two vertices of a ribbon's row have in common (`push_plain`).
struct PlainShared {
    color: [f32; 4],
    vcol: [f32; 4],
    dynamic: [f32; 4],
    misc: [f32; 4],
    normal: Vec3,
    tangent: Vec3,
    particle: [f32; 4],
}

fn push_plain(vertices: &mut Vec<f32>, position: Vec3, uv: [f32; 4], shared: &PlainShared) {
    let PlainShared { color, vcol, dynamic, misc, normal, tangent, particle } = shared;
    vertices.extend_from_slice(&position.to_array());
    vertices.extend_from_slice(&uv);
    vertices.extend_from_slice(&clamp_color(*color));
    vertices.extend_from_slice(vcol);
    vertices.extend_from_slice(dynamic);
    vertices.extend_from_slice(misc);
    vertices.extend_from_slice(&normal.to_array());
    vertices.extend_from_slice(&tangent.to_array());
    vertices.extend_from_slice(particle);
}

/// THE HOST'S FRAME OF EFFECTS: the evaluator's draw list (GAME space) with the names behind its
/// material and mesh ids, the camera it was made for and the effect clock. Call once per game
/// frame; an empty list clears.
pub fn submit(list: &kh_vfx::DrawList, materials: &[String], meshes: &[String], view: &kh_vfx::Camera, seconds: f32) {
    let started = Instant::now();
    let on = options();
    // (IMPL_ER39) the particle lights light the world (pointlight.rs), with the effects switched on
    crate::pointlight::set_fx_lights(if on.on { &list.lights } else { &[] });
    if !on.on || (list.quads.is_empty() && list.meshes.is_empty() && list.ribbons.is_empty() && list.decals.is_empty() && list.screen.is_empty() /* (VFXR) */ && list.skeletal.is_empty() /* (VFX-2) */) {
        clear();
        return;
    }
    let mut prepared = Prepared::default();
    prepared.seconds = seconds;
    prepared.screen = kh_vfx::screen::pick(&list.screen); // (VFXR)
    {
        let Ok(mut guard) = REGISTRY.lock() else { return };
        let registry = guard.get_or_insert_with(Registry::default);
        // Quads, by (material, blend), the order inside a batch kept.
        type QuadKey = (u32, kh_vfx::Blend, [u32; 4], usize);
        let mut by_key: HashMap<QuadKey, usize> = HashMap::new(); // (VFXR sort) + the group
        // (a draw list comes emitter by emitter: runs of one key; the map is only asked when the key changes)
        let mut last: Option<(QuadKey, usize)> = None;
        let quad_groups = list.group_table(0, list.quads.len()); // (VFXR sort)
        for (qi, quad) in list.quads.iter().enumerate() {
            let tint = list.tint_of(0, qi).unwrap_or([0.0; 4]); // (DON-MAGIC tint) tinted quads in batches of their own
            let (sort, group) = sort_of(list, &quad_groups, qi, view.position, quad.center); // (VFXR sort)
            let key = (quad.material, quad.mode, tint.map(f32::to_bits), group);
            let slot = match last {
                Some((k, slot)) if k == key => slot,
                _ => {
                    let slot = match by_key.get(&key) {
                        Some(slot) => *slot,
                        None => {
                            let Some(material) = materials.get(quad.material as usize).and_then(|name| registry.material(name, Geo::Quad, quad.mode)) else { continue };
                            prepared.quads.push(QuadBatch { sort, material, tint, data: pooled(), center: Vec3::ZERO });
                            by_key.insert(key, prepared.quads.len() - 1);
                            prepared.quads.len() - 1
                        }
                    };
                    last = Some((key, slot));
                    slot
                }
            };
            let batch = &mut prepared.quads[slot];
            let c = &quad.corners;
            let color = clamp_color(quad.color);
            let (uv, uv2, d) = (quad.uv, quad.uv2, quad.dynamic);
            batch.data.extend_from_slice(&[
                c[0].x, c[0].y, c[0].z, quad.time, c[1].x, c[1].y, c[1].z, quad.rel, c[2].x, c[2].y, c[2].z, quad.blend, c[3].x, c[3].y, c[3].z, quad.size, uv[0], uv[1], uv[2], uv[3], uv2[0], uv2[1], uv2[2], uv2[3], color[0], color[1], color[2], color[3], d[0], d[1], d[2], d[3],
                quad.center.x, quad.center.y, quad.center.z, 0.0,
            ]);
            batch.center += quad.center;
        }
        for batch in &mut prepared.quads {
            batch.center /= (batch.data.len() / QUAD_FLOATS).max(1) as f32;
        }
        // Mesh particles, by (material, blend, mesh).
        let mut by_mesh: HashMap<(u32, kh_vfx::Blend, u32, [u32; 4], usize), usize> = HashMap::new(); // (DON-MAGIC tint) + the tint (VFXR sort) + the group
        let mesh_groups = list.group_table(2, list.meshes.len()); // (VFXR sort)
        // (party VF-b) the chandelier's fragments: to the character pipeline (their own material), not drawn here
        let mut routed: Vec<crate::xsiweapon::FormWeapon> = Vec::new();
        let mut routed_all = 0usize;
        let mut route_of: HashMap<(u32, u32), Option<&'static str>> = HashMap::new();
        for (mi, instance) in list.meshes.iter().enumerate() {
            let tint = list.tint_of(2, mi).unwrap_or([0.0; 4]); // (DON-MAGIC tint)
            let shard = *route_of.entry((instance.material, instance.mesh)).or_insert_with(|| match (materials.get(instance.material as usize), meshes.get(instance.mesh as usize)) {
                (Some(m), Some(n)) => crate::xsiweapon::shard_model(m, n),
                _ => None,
            });
            if let Some(model) = shard {
                routed_all += 1;
                let (scale, rotation, translation) = instance.transform.to_scale_rotation_translation();
                routed.push(crate::xsiweapon::FormWeapon { model: model.to_string(), to_world: kh_anim::Trs { translation, rotation, scale }, component: Vec::new() });
                continue;
            }
            // (HEROLIGHT-KB) a hero-light glow's weapon copy is drawn ON the weapon as drawn this frame (herolight.rs `lock_mesh`)
            let transform = crate::herolight::lock_mesh(meshes.get(instance.mesh as usize).map_or("", String::as_str), instance.transform, view.position);
            let (sort, group) = sort_of(list, &mesh_groups, mi, view.position, Vec3::from(transform.translation)); // (VFXR sort)
            let slot = match by_mesh.get(&(instance.material, instance.blend, instance.mesh, tint.map(f32::to_bits), group)) {
                Some(slot) => *slot,
                None => {
                    let Some(material) = materials.get(instance.material as usize).and_then(|name| registry.material(name, Geo::Mesh, instance.blend)) else { continue };
                    let Some(mesh) = meshes.get(instance.mesh as usize).map(|name| registry.mesh(name)) else { continue };
                    prepared.meshes.push(MeshBatch { sort, material, tint, mesh, data: pooled(), center: Vec3::ZERO });
                    by_mesh.insert((instance.material, instance.blend, instance.mesh, tint.map(f32::to_bits), group), prepared.meshes.len() - 1);
                    prepared.meshes.len() - 1
                }
            };
            let batch = &mut prepared.meshes[slot];
            let m3 = Mat3::from(transform.matrix3);
            let origin = Vec3::from(transform.translation);
            for k in 0..3 {
                batch.data.extend_from_slice(&[m3.x_axis[k], m3.y_axis[k], m3.z_axis[k], origin[k]]);
            }
            batch.data.extend_from_slice(&clamp_color(instance.color));
            batch.data.extend_from_slice(&instance.dynamic);
            batch.data.extend_from_slice(&[instance.time, instance.rel, instance.uv_mode, instance.sub /* (ZA) mesh sub-UV image */]);
            batch.center += origin;
        }
        for batch in &mut prepared.meshes {
            batch.center /= (batch.data.len() / MESH_INSTANCE_FLOATS).max(1) as f32;
        }
        // (VFX-2) the skeletal mesh components: posed for the mesh-particle shaders of their material, or (their own
        // materials) to the character pipeline with the routed forms
        let (skel, skel_forms) = skelfx::prepare(list, &mut |id, blend| materials.get(id as usize).and_then(|name| registry.material(name, Geo::Mesh, blend)), view.position);
        prepared.skel = skel;
        routed_all += skel_forms.len();
        routed.extend(skel_forms);
        // (the nearest ROUTED_MAX to the camera)
        if routed.len() > ROUTED_MAX {
            routed.sort_by(|a, b| (a.to_world.translation - view.position).length_squared().total_cmp(&(b.to_world.translation - view.position).length_squared()));
            routed.truncate(ROUTED_MAX);
        }
        if routed_all > ROUTED_PEAK.load(Ordering::Relaxed) {
            ROUTED_PEAK.store(routed_all, Ordering::Relaxed);
            rlog!("vfx: (party VF-b) {routed_all} chandelier fragment(s) in a frame to the character pipeline (their own material){}", if routed_all > ROUTED_MAX { format!(", the {ROUTED_MAX} nearest drawn") } else { String::new() });
        }
        if let Ok(mut r) = ROUTED.lock() {
            *r = routed;
        }
        // Ribbons: one batch per (material, blend).
        let mut by_ribbon: HashMap<(u32, kh_vfx::Blend, [u32; 4], usize), usize> = HashMap::new(); // (DON-MAGIC tint) + the tint (VFXR sort) + the group
        let ribbon_groups = list.group_table(1, list.ribbons.len()); // (VFXR sort)
        for (ri, ribbon) in list.ribbons.iter().enumerate() {
            let tint = list.tint_of(1, ri).unwrap_or([0.0; 4]);
            let (sort, group) = sort_of(list, &ribbon_groups, ri, view.position, ribbon.points.first().map_or(Vec3::ZERO, |p| (p.a + p.b) * 0.5)); // (VFXR sort)
            let slot = match by_ribbon.get(&(ribbon.material, ribbon.blend, tint.map(f32::to_bits), group)) {
                Some(slot) => *slot,
                None => {
                    let Some(material) = materials.get(ribbon.material as usize).and_then(|name| registry.material(name, Geo::Ribbon, ribbon.blend)) else { continue };
                    prepared.plain.push(PlainBatch { sort, material, tint, vertices: Vec::new(), indices: Vec::new(), center: Vec3::ZERO, decal: false });
                    by_ribbon.insert((ribbon.material, ribbon.blend, tint.map(f32::to_bits), group), prepared.plain.len() - 1);
                    prepared.plain.len() - 1
                }
            };
            let batch = &mut prepared.plain[slot];
            match list.ribbon_exact_of(ri) {
                Some(exact) => ribbon_exact_geometry(&mut batch.vertices, &mut batch.indices, ribbon, exact, view), // (VFX-RING S)
                None => ribbon_geometry(&mut batch.vertices, &mut batch.indices, ribbon, view),
            }
        }
        // Decals: the box [-1, 1]^3 of each, shaded by the material's deferred-decal shader.
        if on.decals {
            for (di, decal) in list.decals.iter().enumerate() {
                let tint = list.tint_of(3, di).unwrap_or([0.0; 4]); // (DON-MAGIC tint)
                let m = decal.transform;
                if Mat3::from(m.matrix3).determinant().abs() < 1.0e-9 {
                    continue;
                }
                let Some(material) = materials.get(decal.material as usize).and_then(|name| registry.material(name, Geo::Decal, kh_vfx::Blend::Material)) else { continue };
                // World (game space, cm) -> the decal's box space, one row per attribute.
                let inverse = m.inverse();
                let row = |k: usize| [inverse.matrix3.x_axis[k], inverse.matrix3.y_axis[k], inverse.matrix3.z_axis[k], inverse.translation[k]];
                let mut batch = PlainBatch { sort: (0, 0.0, 0, 0) /* (VFXR sort) decals: not sorted */, material, tint, vertices: Vec::with_capacity(8 * PLAIN_FLOATS), indices: Vec::with_capacity(36), center: Vec3::from(m.translation), decal: true };
                for corner in 0..8u32 {
                    let local = Vec3::new(if corner & 1 == 0 { -1.0 } else { 1.0 }, if corner & 2 == 0 { -1.0 } else { 1.0 }, if corner & 4 == 0 { -1.0 } else { 1.0 });
                    // (the colour is NOT clamped here: push_plain does; the rows ride as they are)
                    batch.vertices.extend_from_slice(&m.transform_point3(local).to_array());
                    batch.vertices.extend_from_slice(&[0.0, 0.0, decal.z_fade[0], decal.z_fade[1]]);
                    batch.vertices.extend_from_slice(&clamp_color(decal.color));
                    batch.vertices.extend_from_slice(&row(0));
                    batch.vertices.extend_from_slice(&row(1));
                    batch.vertices.extend_from_slice(&[decal.time, decal.custom[0], decal.custom[1], decal.custom[2]]);
                    batch.vertices.extend_from_slice(&[0.0, 0.0, 1.0]);
                    batch.vertices.extend_from_slice(&[1.0, 0.0, 0.0]);
                    batch.vertices.extend_from_slice(&row(2));
                }
                const FACES: [[u32; 4]; 6] = [[0, 2, 6, 4], [1, 5, 7, 3], [0, 4, 5, 1], [2, 3, 7, 6], [0, 1, 3, 2], [4, 6, 7, 5]];
                for f in FACES {
                    batch.indices.extend([f[0], f[1], f[2], f[0], f[2], f[3]]);
                }
                prepared.plain.push(batch);
            }
        }
        // (ZFIN) the materials whose name holds a word of `r.finale_over` (drawn after the finale while one is drawn)
        let words = crate::finale::over_words();
        if !words.is_empty() {
            prepared.over = registry.keys.iter().enumerate().filter(|(_, k)| crate::finale::over_material(&k.name, &words)).map(|(i, _)| i as u32).collect();
        }
    }
    for batch in &mut prepared.plain {
        if !batch.decal {
            let n = (batch.vertices.len() / PLAIN_FLOATS).max(1);
            let mut sum = Vec3::ZERO;
            for v in batch.vertices.as_chunks::<PLAIN_FLOATS>().0 {
                sum += Vec3::new(v[0], v[1], v[2]);
            }
            batch.center = sum / n as f32;
        }
    }
    prepared.plain.retain(|b| !b.indices.is_empty());
    prepared.particles = list.quads.len() + list.meshes.len();
    PARTICLES_MAX.fetch_max(prepared.particles as u64, Ordering::Relaxed);
    if let Ok(mut guard) = LATEST.lock() {
        *guard = Some(Arc::new(prepared));
    }
    let micros = started.elapsed().as_micros() as u64;
    PREPARE_MICROS.fetch_add(micros, Ordering::Relaxed);
    PREPARE_MAX.fetch_max(micros, Ordering::Relaxed);
}

// ------------------------------------------------------------------------------------ the GPU side

/// What a material's pipeline state is made for (`Gpu::pipeline`): its pass, geometry, the distortion shader's route, its cull
/// mode.
#[derive(Clone, Copy)]
struct PipeKind {
    pass: Pass,
    geo: Geo,
    distort: bool,
    cull: D3D12_CULL_MODE,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pass {
    /// Depth test and write, no blending, no scene depth read.
    Opaque,
    Additive,
    Translucent,
    Modulate,
    DecalAdd,
    DecalBlend,
}

struct GpuMaterial {
    pipeline: ID3D12PipelineState,
    /// The distortion twin.
    distort: Option<ID3D12PipelineState>,
    pass: Pass,
    /// `KhParams`.
    params: Box<[f32; PARAM_ROWS * 4]>,
    /// The sampler heap entry of each of the 16 index-buffer slots.
    samplers: [u32; 16],
    /// The translated shader reads the game clock: `misc.y` is refreshed every frame.
    timed: bool,
    /// (FP) A ONE-SIDED mesh-particle material (its record's `two_sided` false): `pipeline` / `distort` cull the faces turned
    /// away from the camera, and these twins do the same for a MIRRORED instance (negative determinant: its winding is
    /// reversed), as UE does. None: every face is drawn (two-sided materials, sprites, ribbons, decals: as before).
    mirrored: Option<(ID3D12PipelineState, Option<ID3D12PipelineState>)>,
    /// (VFXR) the material has `SQEXEnableEmissiveRevEyeAdaptation` (765 of the pack's 883 effect materials): KH3 divides its
    /// emissive by the eye adaptation, so it shows at the tone mapper's input whatever the exposure (emissive 1.0 = white).
    /// Drawn with the frame constants that put it there (post::eye_boost), not with the lit-surface rule of the others.
    eye: bool,
    /// (VFX-XLATE) the translated shader reads the frame behind it (a blur / warp material, its record's `scene`): drawn only
    /// in a frame with the scene colour copy (t13), which it then asks for.
    scene: bool,
    /// (VFX-2) the constants a parameter of the material feeds (its record's `params`: (first float, count, the
    /// translator's tree)): what a dynamic material instance (a skeletal mesh component's) sets is evaluated into them.
    mid: Vec<(usize, usize, Value)>,
    /// (PARTY-NATIVE) a lit (deferred) material's native pair (None: the earlier shading)
    native: Option<Box<NativeFx>>,
    /// (PARTY-NATIVE) its fog variant (plain, mirrored twin): drawn in the late stage with the game's fog (`FogKit`)
    fog: Option<(ID3D12PipelineState, Option<ID3D12PipelineState>)>,
    /// (PARTY-NATIVE-LIGHT) its lit variant (plain, mirrored twin; a forward-lit owner only): drawn in the late stage with Elden
    /// Ring's own light and fog (vfxlight.rs `LitKit`)
    lit: Option<(ID3D12PipelineState, Option<ID3D12PipelineState>)>,
}

/// (FP) `fx.cull_one_sided` (default on; read before the first material is made): one-sided mesh-particle materials cull
/// their back faces. Off: every mesh face is drawn (before FP: a one-sided material's back face had fresnel 1, so the
/// Chandelier's han03 crystal debris tmi_cry_00a and Myriad's nmi_cry_00a were opaque white shards).
pub static CULL_ONE_SIDED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
/// (FP) How many one-sided mesh-particle materials were made (for the log).
static ONE_SIDED_MADE: AtomicU32 = AtomicU32::new(0);

/// (FP) The cull mode of a one-sided mesh particle: the faces turned away from the camera under this pipeline's winding
/// (`FrontCounterClockwise` false; the pack's meshes face the camera counter-clockwise: D3D's FRONT faces are the far ones,
/// measured: IMPL_FINAL_STATE.md 7, final/R2), and BACK for a mirrored instance.
fn one_sided_cull(mirrored: bool) -> D3D12_CULL_MODE {
    if mirrored {
        D3D12_CULL_MODE_BACK
    } else {
        D3D12_CULL_MODE_FRONT
    }
}

/// (FP) A mesh instance's rows (vfx.rs `submit`: three rows x, y, z, origin) mirror it: the 3x3 part's determinant is negative.
fn instance_mirrored(rows: &[f32]) -> bool {
    let r = |k: usize| Vec3::new(rows[4 * k], rows[4 * k + 1], rows[4 * k + 2]);
    r(0).dot(r(1).cross(r(2))) < 0.0
}

enum MatState {
    Pending,
    Ready(GpuMaterial),
    /// Nothing of it can be drawn (no shader and no texture, a shader file missing ...).
    Failed,
}

struct GpuMesh {
    _vertices: ID3D12Resource,
    _indices: ID3D12Resource,
    vertex_view: D3D12_VERTEX_BUFFER_VIEW,
    index_view: D3D12_INDEX_BUFFER_VIEW,
    index_count: u32,
}

enum MeshState {
    Pending,
    Ready(GpuMesh),
    Failed,
}

struct SlotBuffer {
    buffer: ID3D12Resource,
    mapped: *mut u8,
    address: u64,
}

struct Uploader {
    queue: ID3D12CommandQueue,
    allocator: ID3D12CommandAllocator,
    list: ID3D12GraphicsCommandList,
    fence: ID3D12Fence,
    value: u64,
}

struct Gpu {
    device: ID3D12Device,
    root: ID3D12RootSignature,
    srv_heap: ID3D12DescriptorHeap,
    srv_size: usize,
    sampler_heap: ID3D12DescriptorHeap,
    dsv_heap: ID3D12DescriptorHeap,
    dsv_size: usize,
    white: ID3D12Resource,
    textures: HashMap<String, ID3D12Resource>,
    texture_bytes: u64,
    materials: Vec<MatState>,
    pending_materials: usize,
    meshes: Vec<MeshState>,
    slots: Vec<SlotBuffer>,
    /// The copy of the scene colour the distortion twins read: (texture, width, height, format).
    scene_copy: Option<(ID3D12Resource, u32, u32, i32)>,
    shaders: HashMap<String, Option<Arc<Vec<u8>>>>,
    /// (vertex file, pixel file, pass, distortion twin, (FP) cull mode) -> the state
    pipelines: HashMap<(String, String, u8, bool, i32), Option<ID3D12PipelineState>>,
    formats: (i32, i32),
    uploader: Uploader,
    told: Vec<String>,
    /// (PARTY-NATIVE) the copy of the game's fog for the late effects (`FogKit`), made on the worker; why it could not be made
    fog: Option<FogKit>,
    fog_failed: Option<String>,
    /// (PARTY-NATIVE-LIGHT) the copy of the game's light inputs for the lit effects (vfxlight.rs), made on the worker
    lit: Option<light_kit::LitKit>,
    lit_failed: Option<String>,
}

// SAFETY: D3D12 objects are free-threaded; the mapped pointers are only used under the mutex.
unsafe impl Send for Gpu {}

/// (PARTY-NATIVE) THE COPY OF THE GAME'S FOG for the late (blended / additive / modulate) effects: the fog composite's buffer and
/// volume (ffx.rs `fog_bind`, this frame's) copied by two passes of ours with a root signature shaped like the game's at its
/// pixel-stage tables b0..b3 / t0..t3 (shaders/er_fxfog.wgsl), into textures of ours the effects' fog variants read (t16 / t17).
struct FogKit {
    root: ID3D12RootSignature,
    rows_pso: ID3D12PipelineState,
    vol_pso: ID3D12PipelineState,
    /// CB_SHADER_COMMON_PARAM's 76 rows + the volume's size, raw words (77 x 1 RGBA32_UINT), and its RTV
    rows: ID3D12Resource,
    rows_rtv: ID3D12DescriptorHeap,
    /// per frame slot: the rows read back (the volume's size for the next use of the slot), its footprint
    readback: Vec<ID3D12Resource>,
    footprint: D3D12_PLACED_SUBRESOURCE_FOOTPRINT,
    /// the volume copy (RGBA16F, the game's size) and one RTV per slice; None until its size is read back
    volume: Option<(ID3D12Resource, ID3D12DescriptorHeap, [u32; 3])>,
    rtv_size: usize,
    /// the size read back most recently
    size: [u32; 3],
}

/// (PARTY-NATIVE) The texels of the fog rows (the buffer's 76 rows + the volume's size).
const FOG_TEXELS: u32 = 77;
/// The frame constants' fog row (er_vfx.wgsl `ErFrame.fxf`), after the native row.
const FXF_FLOATS: usize = 4;
/// The largest fog volume taken (a guard against a misread size).
const FOG_MAX: [u32; 3] = [512, 512, 256];

/// (PARTY-NATIVE) A pass's fog mode (er_vfx.wgsl `KhParams.fx.x`; pure, tested): blended 0 (attenuated + in-scatter), additive 1
/// (attenuated only), modulate 2, none 3 (opaque passes take the game's own composite; decals are drawn before it).
fn fog_mode(pass: Pass) -> f32 {
    match pass {
        Pass::Translucent => 0.0,
        Pass::Additive => 1.0,
        Pass::Modulate => 2.0,
        Pass::Opaque | Pass::DecalAdd | Pass::DecalBlend => 3.0,
    }
}

/// (PARTY-NATIVE) A volume size read back is taken when every side is between 1 and `FOG_MAX` (pure, tested).
fn fog_size_ok(size: [u32; 3]) -> bool {
    size.iter().zip(FOG_MAX.iter()).all(|(s, m)| *s >= 1 && *s <= *m)
}

impl FogKit {
    /// The kit for this device (its shaders: `<main root>/er/fxfog.vs.dxil`, `fxfog_rows.ps.dxil`, `fxfog_vol.ps.dxil`).
    unsafe fn new(device: &ID3D12Device, dir: &Path) -> Result<FogKit, String> {
        let text = |what: &str, e: windows::core::Error| format!("{what}: {e}");
        let read = |f: &str| std::fs::read(dir.join("er").join(f)).map_err(|e| format!("{f}: {e} (kh-shaderc --only fxfog)"));
        let (vs, rows_ps, vol_ps) = (read("fxfog.vs.dxil")?, read("fxfog_rows.ps.dxil")?, read("fxfog_vol.ps.dxil")?);
        // the root signature: the game's pixel-stage tables at b0..b3 and t0..t3 (its GX root signature's 28 / 30), a slice at b5
        let buffers = [D3D12_DESCRIPTOR_RANGE { RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_CBV, NumDescriptors: 4, BaseShaderRegister: 0, RegisterSpace: 0, OffsetInDescriptorsFromTableStart: 0 }];
        let textures = [D3D12_DESCRIPTOR_RANGE { RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SRV, NumDescriptors: 4, BaseShaderRegister: 0, RegisterSpace: 0, OffsetInDescriptorsFromTableStart: 0 }];
        let table = |ranges: &[D3D12_DESCRIPTOR_RANGE]| D3D12_ROOT_PARAMETER { ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE, Anonymous: D3D12_ROOT_PARAMETER_0 { DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE { NumDescriptorRanges: ranges.len() as u32, pDescriptorRanges: ranges.as_ptr() } }, ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL };
        let parameters = [
            table(&buffers),
            table(&textures),
            D3D12_ROOT_PARAMETER { ParameterType: D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS, Anonymous: D3D12_ROOT_PARAMETER_0 { Constants: D3D12_ROOT_CONSTANTS { ShaderRegister: 5, RegisterSpace: 0, Num32BitValues: 4 } }, ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL },
        ];
        let desc = D3D12_ROOT_SIGNATURE_DESC { NumParameters: parameters.len() as u32, pParameters: parameters.as_ptr(), NumStaticSamplers: 0, pStaticSamplers: std::ptr::null(), Flags: D3D12_ROOT_SIGNATURE_FLAG_NONE };
        let mut blob: Option<ID3DBlob> = None;
        D3D12SerializeRootSignature(&desc, D3D_ROOT_SIGNATURE_VERSION_1, &mut blob, None).map_err(|e| text("fog root signature", e))?;
        let blob = blob.ok_or("fog root signature: no blob")?;
        let root: ID3D12RootSignature = device.CreateRootSignature(0, std::slice::from_raw_parts(blob.GetBufferPointer() as *const u8, blob.GetBufferSize())).map_err(|e| text("CreateRootSignature (fog)", e))?;
        let pso = |pixel: &[u8], format: DXGI_FORMAT| -> Result<ID3D12PipelineState, String> {
            let mut formats = [DXGI_FORMAT_UNKNOWN; 8];
            formats[0] = format;
            let mut blend = D3D12_BLEND_DESC::default();
            blend.RenderTarget[0].RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8;
            let d = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
                // SAFETY: a borrowed copy of the interface pointer for the duration of the call.
                pRootSignature: std::mem::transmute_copy(&root),
                VS: D3D12_SHADER_BYTECODE { pShaderBytecode: vs.as_ptr() as *const c_void, BytecodeLength: vs.len() },
                PS: D3D12_SHADER_BYTECODE { pShaderBytecode: pixel.as_ptr() as *const c_void, BytecodeLength: pixel.len() },
                BlendState: blend,
                SampleMask: u32::MAX,
                RasterizerState: D3D12_RASTERIZER_DESC { FillMode: D3D12_FILL_MODE_SOLID, CullMode: D3D12_CULL_MODE_NONE, DepthClipEnable: false.into(), ..Default::default() },
                DepthStencilState: D3D12_DEPTH_STENCIL_DESC { DepthEnable: false.into(), StencilEnable: false.into(), ..Default::default() },
                PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
                NumRenderTargets: 1,
                RTVFormats: formats,
                DSVFormat: DXGI_FORMAT_UNKNOWN,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                ..Default::default()
            };
            device.CreateGraphicsPipelineState(&d).map_err(|e| text("CreateGraphicsPipelineState (fog)", e))
        };
        let rows_pso = pso(&rows_ps, DXGI_FORMAT_R32G32B32A32_UINT)?;
        let vol_pso = pso(&vol_ps, DXGI_FORMAT_R16G16B16A16_FLOAT)?;
        let rows_desc = D3D12_RESOURCE_DESC { Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D, Width: FOG_TEXELS as u64, Height: 1, DepthOrArraySize: 1, MipLevels: 1, Format: DXGI_FORMAT_R32G32B32A32_UINT, SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 }, Flags: D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET, ..Default::default() };
        let rows = committed(device, D3D12_HEAP_TYPE_DEFAULT, &rows_desc, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE).map_err(|e| text("fog rows", e))?;
        let rows_rtv: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV, NumDescriptors: 1, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE, NodeMask: 0 }).map_err(|e| text("fog rows rtv", e))?;
        crate::gpudebug::name(&rows_rtv, "kh:vfx:rows_rtv");
        device.CreateRenderTargetView(&rows, None, rows_rtv.GetCPUDescriptorHandleForHeapStart());
        let mut footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default();
        let mut total = 0u64;
        device.GetCopyableFootprints(&rows_desc, 0, 1, 0, Some(&mut footprint), None, None, Some(&mut total));
        let mut readback = Vec::new();
        for _ in 0..FRAME_SLOTS {
            readback.push(committed(device, D3D12_HEAP_TYPE_READBACK, &buffer_desc(total), D3D12_RESOURCE_STATE_COPY_DEST).map_err(|e| text("fog readback", e))?);
        }
        let rtv_size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV) as usize;
        Ok(FogKit { root, rows_pso, vol_pso, rows, rows_rtv, readback, footprint, volume: None, rtv_size, size: [0; 3] })
    }

    /// The volume's size this slot's readback holds (written by its previous use: the GPU is past it), and the volume copy
    /// made again when it differs.
    unsafe fn take_size(&mut self, device: &ID3D12Device, slot: usize) {
        let Some(readback) = self.readback.get(slot) else { return };
        let mut at: *mut c_void = std::ptr::null_mut();
        if readback.Map(0, Some(&D3D12_RANGE { Begin: 0, End: (self.footprint.Offset as usize) + FOG_TEXELS as usize * 16 }), Some(&mut at)).is_err() || at.is_null() {
            return;
        }
        let texel = (at as *const u8).add(self.footprint.Offset as usize + (FOG_TEXELS as usize - 1) * 16) as *const u32;
        let size = [*texel, *texel.add(1), *texel.add(2)];
        readback.Unmap(0, Some(&D3D12_RANGE { Begin: 0, End: 0 }));
        self.size = size;
        if !fog_size_ok(size) || self.volume.as_ref().is_some_and(|v| v.2 == size) {
            return;
        }
        let desc = D3D12_RESOURCE_DESC { Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE3D, Width: size[0] as u64, Height: size[1], DepthOrArraySize: size[2] as u16, MipLevels: 1, Format: DXGI_FORMAT_R16G16B16A16_FLOAT, SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 }, Flags: D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET, ..Default::default() };
        let (Ok(volume), Ok(heap)) = (committed(device, D3D12_HEAP_TYPE_DEFAULT, &desc, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE), device.CreateDescriptorHeap::<ID3D12DescriptorHeap>(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV, NumDescriptors: size[2], Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE, NodeMask: 0 })) else { return };
        let start = heap.GetCPUDescriptorHandleForHeapStart();
        for z in 0..size[2] {
            let rtv = D3D12_RENDER_TARGET_VIEW_DESC { Format: DXGI_FORMAT_R16G16B16A16_FLOAT, ViewDimension: D3D12_RTV_DIMENSION_TEXTURE3D, Anonymous: D3D12_RENDER_TARGET_VIEW_DESC_0 { Texture3D: D3D12_TEX3D_RTV { MipSlice: 0, FirstWSlice: z, WSize: 1 } } };
            device.CreateRenderTargetView(&volume, Some(&rtv), D3D12_CPU_DESCRIPTOR_HANDLE { ptr: start.ptr + z as usize * self.rtv_size });
        }
        // (the one before may be named by a frame in flight: kept until the GPU is past it, GPUSAFE)
        if let Some((old, old_heap, _)) = self.volume.replace((volume, heap, size)) {
            crate::outfit::retire(Box::new(old));
            crate::outfit::retire(Box::new(old_heap));
        }
        rlog!("vfx: (PARTY-NATIVE) the game's fog volume is {}x{}x{}: its copy for the late effects is made", size[0], size[1], size[2]);
    }

    /// The copy of this frame's fog (`bind`: the game's views heap and its composite's tables) into our rows and volume,
    /// recorded into `list`; true when the volume copy was drawn (the effects may read it). Leaves the fog root signature and
    /// the game's heap bound: the caller binds its own again.
    unsafe fn copy(&self, list: &ID3D12GraphicsCommandList, slot: usize, bind: crate::ffx::FogBind) -> bool {
        let raw = bind.heaps[0] as *mut c_void;
        let Some(views) = ID3D12DescriptorHeap::from_raw_borrowed(&raw) else { return false };
        list.SetGraphicsRootSignature(&self.root);
        list.SetDescriptorHeaps(&[Some(views.clone())]);
        list.SetGraphicsRootDescriptorTable(0, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: bind.buffers });
        list.SetGraphicsRootDescriptorTable(1, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: bind.textures });
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        // the rows (and the volume's size), then into this slot's readback
        list.ResourceBarrier(&[transition(&self.rows, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, D3D12_RESOURCE_STATE_RENDER_TARGET)]);
        let rtv = self.rows_rtv.GetCPUDescriptorHandleForHeapStart();
        list.OMSetRenderTargets(1, Some(&rtv), false, None);
        list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: FOG_TEXELS as f32, Height: 1.0, MinDepth: 0.0, MaxDepth: 1.0 }]);
        list.RSSetScissorRects(&[windows::Win32::Foundation::RECT { left: 0, top: 0, right: FOG_TEXELS as i32, bottom: 1 }]);
        list.SetPipelineState(&self.rows_pso);
        list.DrawInstanced(3, 1, 0, 0);
        list.ResourceBarrier(&[transition(&self.rows, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_COPY_SOURCE)]);
        if let Some(readback) = self.readback.get(slot) {
            let to = D3D12_TEXTURE_COPY_LOCATION { pResource: std::mem::transmute_copy(readback), Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT, Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { PlacedFootprint: self.footprint } };
            let from = D3D12_TEXTURE_COPY_LOCATION { pResource: std::mem::transmute_copy(&self.rows), Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX, Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { SubresourceIndex: 0 } };
            list.CopyTextureRegion(&to, 0, 0, 0, &from, None);
        }
        list.ResourceBarrier(&[transition(&self.rows, D3D12_RESOURCE_STATE_COPY_SOURCE, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE)]);
        // the volume, slice by slice, at the size the readback gave (None: the next frames, once it is known)
        let Some((volume, heap, size)) = self.volume.as_ref().filter(|v| v.2 == self.size) else { return false };
        list.ResourceBarrier(&[transition(volume, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, D3D12_RESOURCE_STATE_RENDER_TARGET)]);
        list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: size[0] as f32, Height: size[1] as f32, MinDepth: 0.0, MaxDepth: 1.0 }]);
        list.RSSetScissorRects(&[windows::Win32::Foundation::RECT { left: 0, top: 0, right: size[0] as i32, bottom: size[1] as i32 }]);
        list.SetPipelineState(&self.vol_pso);
        let start = heap.GetCPUDescriptorHandleForHeapStart();
        for z in 0..size[2] {
            let rtv = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: start.ptr + z as usize * self.rtv_size };
            list.OMSetRenderTargets(1, Some(&rtv), false, None);
            list.SetGraphicsRoot32BitConstants(2, 4, [z, 0u32, 0, 0].as_ptr() as *const c_void, 0);
            list.DrawInstanced(3, 1, 0, 0);
        }
        list.ResourceBarrier(&[transition(volume, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE)]);
        true
    }
}

/// `PACK/vfx/shaders/shading.json` + which shader files exist.
struct Shading {
    dir: PathBuf,
    json: Value,
    built: Value,
}

static SHADING: OnceLock<Option<Shading>> = OnceLock::new();

/// (XW-c) The main table's overlay is laid over it (`fx.main_fix`, default on; read before the table is first read).
pub static MAIN_OVERLAY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// (party VF-b) `fx.trig_fix` (default on; read before the tables are first read): the main and party tables' overlays
/// `shaders/shading_vfb.json` (material records only, new files): the TrigMath uniform expressions (the UV turn of the
/// two-layer masters jmb_2layer_m(s)k) evaluated; without them the turned UV collapses to the texture's centre and the
/// particles are flat white cards (Donald's staff hit vw_do090_hit0, Sora's own crs sparks).
pub static TRIG_OVERLAY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// (XW-d) `fx.xs_fix` (default on; read before the tables are first read): the main and XS tables' overlays
/// `shaders/shading_xwd.json` + `er/shaders_xwd.json` (new files): owner tmt_cry_00a translated (TEXCOORD9 = the translated
/// world position; the Chandelier's han03 crystal debris tmi_cry_00a, Myriad's nmi_cry_00a: drawn with the generic variant
/// before) under new shader names; the XS table's also the records of the XS assets' own materials (kh_host effects_xwd/).
pub static XWD_OVERLAY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// (XW-d) Lays a root's shading_xwd.json + er/shaders_xwd.json over its table when `fx.xs_fix` is on; returns the count.
fn overlay_xwd(json: &mut Value, built: &mut Value, dir: &Path) -> usize {
    if !XWD_OVERLAY.load(Ordering::Relaxed) {
        return 0;
    }
    let read = |path: PathBuf| std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok());
    // (RSF round 21, the offline harness: `KH_XWD_DIR=PACK` reads PACK/<this root>/shaders/shading_xwd.json + er/shaders_xwd.json
    // first (a staged overlay; its shader files are read from PACK/<this root>/er when the pack has none), the pack stays as it is)
    if let Some(stage) = xwd_stage(dir) {
        if let (Some(over), Some(over_built)) = (read(stage.join("shaders").join("shading_xwd.json")), read(stage.join("er").join("shaders_xwd.json"))) {
            return overlay_xw(json, built, Some(over), Some(over_built));
        }
    }
    overlay_xw(json, built, read(dir.join("shaders").join("shading_xwd.json")), read(dir.join("er").join("shaders_xwd.json")))
}

/// (RSF round 21) `KH_XWD_DIR`'s folder for a root (vfx, party/vfx or xsi/vfx), the offline harness's staged XW-d overlay.
fn xwd_stage(dir: &Path) -> Option<PathBuf> {
    let sub = if dir.ends_with("party/vfx") { "party/vfx" } else if dir.ends_with("xsi/vfx") { "xsi/vfx" } else { "vfx" };
    std::env::var_os("KH_XWD_DIR").map(|d| Path::new(&d).join(sub))
}

/// (RSF round 20) The material records a table still flags `bad_uniform_expression` after its overlays (a uniform expression
/// the translator could not evaluate: its constants are 0 there, e.g. a two-layer master's UV turn: a flat white card), for the
/// log: the count and the first names (a material added to a table without its overlay made again shows here).
pub fn bad_expression_materials(json: &Value) -> Vec<String> {
    let mut out: Vec<String> = json.get("materials").and_then(Value::as_object).map(|m| m.iter().filter(|(_, r)| r.get("bad_uniform_expression").and_then(Value::as_bool).unwrap_or(false)).map(|(k, _)| k.clone()).collect()).unwrap_or_default();
    out.sort();
    out
}

/// (RSF round 21) The owners a table (after its overlays) still has no translated sprite / mesh shader for, with the reason
/// (their materials draw with the generic variant), for the log: "owner.class (reason)". The reasons "no ... shaders" (the
/// material was never cooked for that class) are not listed.
pub fn untranslated_owners(json: &Value) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(owners) = json.get("owners").and_then(Value::as_object) {
        for (name, rec) in owners {
            for class in ["sprite", "mesh"] {
                let Some(c) = rec.get(class) else { continue };
                if c.get("ok").and_then(Value::as_bool).unwrap_or(false) {
                    continue;
                }
                let why = c.get("why").and_then(Value::as_str).unwrap_or("?");
                if why.starts_with("no ") {
                    continue;
                }
                out.push(format!("{name}.{class} ({})", why.split("  (").next().unwrap_or(why).trim()));
            }
        }
    }
    out.sort();
    out
}

fn tell_untranslated(json: &Value, which: &str) {
    let left = untranslated_owners(json);
    if !left.is_empty() {
        rlog!("vfx: (RSF round 21) the {which} table: {} owner shader(s) still untranslated (their materials draw with the generic variant): {}", left.len(), left.join(", "));
    }
}

fn tell_bad_expressions(json: &Value, which: &str) {
    let bad = bad_expression_materials(json);
    if !bad.is_empty() {
        rlog!("vfx: (RSF round 20) the {which} table: {} material(s) with a uniform expression the translator could not evaluate (their constants read 0; a turned UV collapses to a flat card): {}{}", bad.len(), bad.iter().take(12).cloned().collect::<Vec<_>>().join(", "), if bad.len() > 12 { ", ..." } else { "" });
    }
}

/// (party VF-b) Lays a root's shading_vfb.json (records only) over its table when `on`; returns the count.
fn overlay_vfb(json: &mut Value, built: &mut Value, dir: &Path, on: bool) -> usize {
    if !on {
        return 0;
    }
    let read = |path: PathBuf| std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok());
    let over_built = read(dir.join("er").join("shaders_vfb.json")).or_else(|| Some(serde_json::json!({ "shaders": [] })));
    // (RSF round 20, the offline harness: `KH_VFB_DIR=PACK` reads PACK/<this root>/shaders/shading_vfb.json first (a staged
    // overlay), the pack stays as it is)
    let sub = if dir.ends_with("party/vfx") { "party/vfx" } else { "vfx" };
    let staged = std::env::var_os("KH_VFB_DIR").and_then(|d| read(Path::new(&d).join(sub).join("shaders").join("shading_vfb.json")));
    overlay_xw(json, built, staged.or_else(|| read(dir.join("shaders").join("shading_vfb.json"))), over_built)
}

/// (PARTY-NATIVE) Lays a root's `shaders/shading_fwl.json` + `er/shaders_fwl.json` (tools/vfx_xlate_overlay.js --take light over
/// tools/sora_vfx_shaders.js --fwl: the lit translucents translated WITH their light input, the forward lighting or the indirect
/// lighting cache, under `<letter><id>_fwl`; their fog / lit variants `f…_fwl` / `l…_fwl` are found by file name) over its table,
/// after every other overlay; returns the count. Without the two files nothing changes.
fn overlay_fwl(json: &mut Value, built: &mut Value, dir: &Path) -> usize {
    let read = |path: PathBuf| std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok());
    let n = overlay_xw(json, built, read(dir.join("shaders").join("shading_fwl.json")), read(dir.join("er").join("shaders_fwl.json")));
    if n > 0 {
        rlog!("vfx: (PARTY-NATIVE) {n} shader(s) / record(s) of shading_fwl.json laid over {} (the lit translucents with their light input: lit variants l<pixel file> in the native mode)", dir.display());
    }
    n
}

fn shading() -> Option<&'static Shading> {
    SHADING
        .get_or_init(|| {
            let pack = crate::SHARED.lock().ok().and_then(|g| g.as_ref().map(|s| s.pack.clone()))?;
            let dir = pack.join("vfx");
            let read = |path: PathBuf| std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok());
            let json = read(dir.join("shaders").join("shading.json"));
            let built = read(dir.join("er").join("shaders.json"));
            match (json, built) {
                (Some(mut json), Some(mut built)) => {
                    rlog!("vfx: shading table and {} built shader variants read from {}", built.get("shaders").and_then(Value::as_array).map_or(0, Vec::len), dir.display());
                    // (XW-c) the main table's overlay (shading_xwb.json + er/shaders_xwb.json, new files: the main table stays as it
                    // is): the shaders whose translation was wrong (smt_blz_ice_lit_05o's mask switch) from their new files;
                    // `fx.main_fix = 0`: without it
                    if MAIN_OVERLAY.load(Ordering::Relaxed) {
                        let n = overlay_xw(&mut json, &mut built, read(dir.join("shaders").join("shading_xwb.json")), read(dir.join("er").join("shaders_xwb.json")));
                        if n > 0 {
                            rlog!("vfx: (XW-c) {n} shader(s) / record(s) of shading_xwb.json laid over the main table (the Blizzard ice mask's switch); fx.main_fix = 0 switches it off");
                        }
                    }
                    // (party VF-b) the main table's trig overlay (shading_vfb.json); fx.trig_fix = 0: without it
                    let n = overlay_vfb(&mut json, &mut built, &dir, TRIG_OVERLAY.load(Ordering::Relaxed));
                    if n > 0 {
                        rlog!("vfx: (party VF-b) {n} record(s) of shading_vfb.json laid over the main table (TrigMath evaluated: the two-layer UV turn); fx.trig_fix = 0 switches it off");
                    }
                    // (XW-d) the main table's shading_xwd.json (tmt_cry_00a translated); fx.xs_fix = 0: without it
                    let n = overlay_xwd(&mut json, &mut built, &dir);
                    if n > 0 {
                        rlog!("vfx: (XW-d) {n} shader(s) / record(s) of shading_xwd.json laid over the main table (TEXCOORD9 / Primitive[10] translated: tmt_cry_00a, (RSF round 21) jmb_smk_imp_00sa, tmt_cyl_pil_00sa); fx.xs_fix = 0 switches it off");
                    }
                    // (VFX-XLATE) the main table's shading_vxl.json + er/shaders_vxl.json (new files, tools/vfx_xlate_overlay.js): the
                    // owners the translator's scene-colour / collection / Primitive / View[160] rules now take (the blur and warp
                    // materials, the Ghost Horde afterimage smt_so050_stf_tsn_00k ...) under new shader names; fx.main_fix = 0: without it
                    if MAIN_OVERLAY.load(Ordering::Relaxed) {
                        let n = overlay_xw(&mut json, &mut built, read(dir.join("shaders").join("shading_vxl.json")), read(dir.join("er").join("shaders_vxl.json")));
                        if n > 0 {
                            rlog!("vfx: (VFX-XLATE) {n} shader(s) / record(s) of shading_vxl.json laid over the main table (scene colour, collections, Primitive, View[160]); fx.main_fix = 0 switches it off");
                        }
                    }
                    overlay_fwl(&mut json, &mut built, &dir); // (PARTY-NATIVE) the lit translucents with their light input, last
                    tell_untranslated(&json, "main"); // (RSF round 21)
                    tell_bad_expressions(&json, "main"); // (RSF round 20)
                    Some(Shading { dir, json, built })
                }
                (json, built) => {
                    rlog!("vfx: NOT AVAILABLE: {} (node tools/sora_vfx_shaders.js; cargo run --release -p kh-shaderc -- --vfx): no effects are drawn", if json.is_none() { "PACK/vfx/shaders/shading.json is missing" } else if built.is_none() { "PACK/vfx/er/shaders.json is missing" } else { "?" });
                    None
                }
            }
        })
        .as_ref()
}

/// (ER party S5) THE PARTY'S EFFECT ROOT (PACK/party/vfx: Donald's, Goofy's and the team attacks' effects): its own
/// shading table (`shaders/shading.json`), its own built shaders (`er/`, `kh-shaderc --vfx --pack PACK/party`), its own
/// materials, meshes and textures. The effect side names its materials / meshes `party:<name>` (kh_host::vfx::
/// PARTY_NAMESPACE); every other name is the main pack's and is made from the main table only: the two tables' ids are
/// never mixed (the party table gives 769 of the main materials other ids).
pub const PARTY_NAMESPACE: &str = "party:";

static PARTY_SHADING: OnceLock<Option<Shading>> = OnceLock::new();

fn party_shading() -> Option<&'static Shading> {
    PARTY_SHADING
        .get_or_init(|| {
            let pack = crate::SHARED.lock().ok().and_then(|g| g.as_ref().map(|s| s.pack.clone()))?;
            let dir = pack.join("party").join("vfx");
            let read = |path: PathBuf| std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok());
            match (read(dir.join("shaders").join("shading.json")), read(dir.join("er").join("shaders.json"))) {
                (Some(mut json), Some(mut built)) => {
                    rlog!("vfx: (party) the party root's own shading table and {} built shader variants read from {}", built.get("shaders").and_then(Value::as_array).map_or(0, Vec::len), dir.display());
                    // (XW-b) shading_xwb.json / er/shaders_xwb.json: the shaders whose translation changed, under new names
                    let n = overlay_xw(&mut json, &mut built, read(dir.join("shaders").join("shading_xwb.json")), read(dir.join("er").join("shaders_xwb.json")));
                    if n > 0 {
                        rlog!("vfx: (XW-b) the party table: {n} shader(s) / record(s) of shading_xwb.json laid over it (decals' emitter arithmetic, switch / break)");
                    }
                    // (party VF-b) the party table's trig overlay (shading_vfb.json: vw_do090_hit0's crs cards); fx.trig_fix = 0: without it
                    let n = overlay_vfb(&mut json, &mut built, &dir, TRIG_OVERLAY.load(Ordering::Relaxed));
                    if n > 0 {
                        rlog!("vfx: (party VF-b) the party table: {n} record(s) of shading_vfb.json laid over it (TrigMath evaluated); fx.trig_fix = 0 switches it off");
                    }
                    // (RSF round 21) the party table's shading_xwd.json (the owners whose shaders read TEXCOORD9 / Primitive[10]:
                    // omt_star_00o, tmt_cry_00a, tmt_cyl_pil_00sa translated); fx.xs_fix = 0: without it
                    let n = overlay_xwd(&mut json, &mut built, &dir);
                    if n > 0 {
                        rlog!("vfx: (RSF round 21) the party table: {n} shader(s) / record(s) of shading_xwd.json laid over it (TEXCOORD9 / Primitive[10] translated); fx.xs_fix = 0 switches it off");
                    }
                    overlay_fwl(&mut json, &mut built, &dir); // (PARTY-NATIVE) the lit translucents with their light input, last
                    tell_untranslated(&json, "party"); // (RSF round 21)
                    tell_bad_expressions(&json, "party"); // (RSF round 20)
                    Some(Shading { dir, json, built })
                }
                (json, _) => {
                    rlog!("vfx: (party) NO PARTY EFFECT ROOT: {} (cargo run --release -p kh-shaderc -- --vfx --pack PACK/party): the party's own effect materials are not drawn", if json.is_none() { "PACK/party/vfx/shaders/shading.json is missing" } else { "PACK/party/vfx/er/shaders.json is missing" });
                    None
                }
            }
        })
        .as_ref()
}

/// (ER90, XSI request 1) THE XS ABILITIES' EFFECT ROOT (PACK/xsi/vfx: the XS moves' form effects, tools/sora_vfx_shaders.js
/// `--xsi` + `kh-shaderc --vfx --pack PACK/xsi`): its own shading table, built shaders, materials, meshes and textures,
/// as the party's. The effect side names its materials / meshes `xsi:<name>` (kh_host::vfx::XSI_NAMESPACE); its table's
/// ids are never mixed with the main or the party table's. Without such a name nothing of it is read.
pub const XSI_NAMESPACE: &str = "xsi:";

static XSI_SHADING: OnceLock<Option<Shading>> = OnceLock::new();

fn xsi_shading() -> Option<&'static Shading> {
    XSI_SHADING
        .get_or_init(|| {
            let pack = crate::SHARED.lock().ok().and_then(|g| g.as_ref().map(|s| s.pack.clone()))?;
            let dir = pack.join("xsi").join("vfx");
            let read = |path: PathBuf| std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok());
            match (read(dir.join("shaders").join("shading.json")), read(dir.join("er").join("shaders.json"))) {
                (Some(mut json), Some(mut built)) => {
                    rlog!("vfx: (XSI) the XS effect root's own shading table and {} built shader variants read from {}", built.get("shaders").and_then(Value::as_array).map_or(0, Vec::len), dir.display());
                    // (XW) shading_xw.json / er/shaders_xw.json laid over them: the records of the decals XW translated
                    let n = overlay_xw(&mut json, &mut built, read(dir.join("shaders").join("shading_xw.json")), read(dir.join("er").join("shaders_xw.json")));
                    if n > 0 {
                        rlog!("vfx: (XW) {n} record(s) of shading_xw.json laid over the XS table (Giant Stamp's and Myriad's decals)");
                    }
                    // (XW-b) and shading_xwb.json / er/shaders_xwb.json: the shaders whose translation changed, under new names
                    let n = overlay_xw(&mut json, &mut built, read(dir.join("shaders").join("shading_xwb.json")), read(dir.join("er").join("shaders_xwb.json")));
                    if n > 0 {
                        rlog!("vfx: (XW-b) {n} shader(s) / record(s) of shading_xwb.json laid over the XS table (decals' emitter arithmetic, switch / break)");
                    }
                    // (XW-c) shading_xwc.json: the records of the materials the chandelier's own effects add (no new shader)
                    let n = overlay_xw(&mut json, &mut built, read(dir.join("shaders").join("shading_xwc.json")), Some(serde_json::json!({ "shaders": [] })));
                    if n > 0 {
                        rlog!("vfx: (XW-c) {n} record(s) of shading_xwc.json laid over the XS table (the chandelier's own effects)");
                    }
                    // (XW-d) shading_xwd.json: tmt_cry_00a translated, the XS assets' own materials (effects_xwd/); fx.xs_fix = 0: without it
                    let n = overlay_xwd(&mut json, &mut built, &dir);
                    if n > 0 {
                        rlog!("vfx: (XW-d) {n} shader(s) / record(s) of shading_xwd.json laid over the XS table (tmt_cry_00a, the XS assets' own materials); fx.xs_fix = 0 switches it off");
                    }
                    overlay_fwl(&mut json, &mut built, &dir); // (PARTY-NATIVE) the lit translucents with their light input, last
                    tell_untranslated(&json, "XS"); // (RSF round 21)
                    Some(Shading { dir, json, built })
                }
                (json, _) => {
                    rlog!("vfx: (XSI) NO XS EFFECT ROOT: {} (cargo run --release -p kh-shaderc -- --vfx --pack PACK/xsi): the XS moves' own effect materials are not drawn", if json.is_none() { "PACK/xsi/vfx/shaders/shading.json is missing" } else { "PACK/xsi/vfx/er/shaders.json is missing" });
                    None
                }
            }
        })
        .as_ref()
}

/// (P5 WEAPON DRAW) THE KEYBLADES' EFFECT ROOT (PACK/keyblades/vfx: every keyblade's trails, appear effects and form
/// effects, tools/sora_keyblades_pack.js + `kh-shaderc --vfx --pack PACK/keyblades`): its own shading table, built shaders,
/// materials, meshes and textures, as the XSI root's. The effect side names its materials / meshes `kbfx:<name>`
/// (kh_host::vfx::KB_NAMESPACE). Without such a name nothing of it is read.
pub const KB_NAMESPACE: &str = "kbfx:";

static KB_SHADING: OnceLock<Option<Shading>> = OnceLock::new();

fn kb_shading() -> Option<&'static Shading> {
    KB_SHADING
        .get_or_init(|| {
            let pack = crate::SHARED.lock().ok().and_then(|g| g.as_ref().map(|s| s.pack.clone()))?;
            let dir = pack.join("keyblades").join("vfx");
            let read = |path: PathBuf| std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok());
            match (read(dir.join("shaders").join("shading.json")), read(dir.join("er").join("shaders.json"))) {
                (Some(json), Some(built)) => {
                    rlog!("vfx: (P5) the keyblades' effect root's own shading table and {} built shader variants read from {}", built.get("shaders").and_then(Value::as_array).map_or(0, Vec::len), dir.display());
                    tell_untranslated(&json, "keyblades");
                    Some(Shading { dir, json, built })
                }
                (json, _) => {
                    rlog!("vfx: (P5) NO KEYBLADE EFFECT ROOT: {} (cargo run --release -p kh-shaderc -- --vfx --pack PACK/keyblades): the keyblades' own effect materials are not drawn", if json.is_none() { "PACK/keyblades/vfx/shaders/shading.json is missing" } else { "PACK/keyblades/vfx/er/shaders.json is missing" });
                    None
                }
            }
        })
        .as_ref()
}

/// (P5) The main pack's folder for a texture file the keyblades' root (`dir`) names but does not carry (its table names
/// 238 files that only the main pack has); None for another root or a file the root has.
fn shading_main_dir_for(dir: &Path, file: &str) -> Option<PathBuf> {
    if !KB_SHADING.get().and_then(|s| s.as_ref()).is_some_and(|k| k.dir == dir) || dir.join(file).is_file() {
        return None;
    }
    shading().map(|m| m.dir.clone()).filter(|m| m.join(file).is_file())
}

/// (P5) For a material of the keyblades' root whose owner's shader that root did not build: the main pack's built shader of
/// the owner of the same name (its folder, its pixel file `<letter><id>[suffix].ps.dxil`). None for another root, an owner
/// the main table lacks or did not build either.
fn same_owner_in_main(root: &Shading, owner: &str, letter: &str) -> Option<(PathBuf, String)> {
    if !KB_SHADING.get().and_then(|s| s.as_ref()).is_some_and(|k| k.dir == root.dir) || owner.is_empty() {
        return None;
    }
    let main = shading()?;
    let id = main.json.get("owners")?.get(owner)?.get("id")?.as_u64()?;
    let suffix = main.json.get("pixel_suffix").and_then(|p| p.get(format!("{letter}{id}"))).and_then(Value::as_str).unwrap_or("");
    let file = format!("{letter}{id}{suffix}.ps.dxil");
    main.built.get("shaders").and_then(Value::as_array)?.iter().any(|x| x.get("file").and_then(Value::as_str) == Some(file.as_str())).then(|| (main.dir.clone(), file))
}

/// (XW) Lays an overlay table (`shading_xw.json`: owners / materials records by name) and its built shaders
/// (`er/shaders_xw.json`: more entries of `shaders`) over a root's own; returns the number of records laid over. An
/// overlay whose shaders file is missing changes nothing (its records would name shaders that are not there).
pub fn overlay_xw(json: &mut Value, built: &mut Value, over: Option<Value>, over_built: Option<Value>) -> usize {
    let (Some(over), Some(over_built)) = (over, over_built) else { return 0 };
    let mut n = 0;
    // (XW-b) `pixel_suffix`: { "<letter><id>": "_xwb" }: those pixel shaders are taken from their new files
    if let Some(src) = over.get("pixel_suffix").and_then(Value::as_object) {
        if json.get("pixel_suffix").is_none() {
            json["pixel_suffix"] = Value::Object(Default::default());
        }
        if let Some(dst) = json.get_mut("pixel_suffix").and_then(Value::as_object_mut) {
            for (k, v) in src {
                dst.insert(k.clone(), v.clone());
                n += 1;
            }
        }
    }
    for kind in ["owners", "materials"] {
        if let (Some(dst), Some(src)) = (json.get_mut(kind).and_then(Value::as_object_mut), over.get(kind).and_then(Value::as_object)) {
            for (name, record) in src {
                dst.insert(name.clone(), record.clone());
                n += 1;
            }
        }
    }
    if let (Some(dst), Some(src)) = (built.get_mut("shaders").and_then(Value::as_array_mut), over_built.get("shaders").and_then(Value::as_array)) {
        dst.extend(src.iter().cloned());
    }
    n
}

/// (ER party S5) The root a material / mesh name of the effect side is made from, and its name there: `party:<name>` =
/// the party root (None when there is none), (ER90) `xsi:<name>` = the XSI root, anything else = the main pack.
fn split_ns<'a>(name: &'a str, main: &'static Shading) -> Option<(&'static Shading, &'a str)> {
    if let Some(rest) = name.strip_prefix(XSI_NAMESPACE) {
        return xsi_shading().map(|s| (s, rest));
    }
    // (P5) `kbfx:<name>` = the keyblades' root
    if let Some(rest) = name.strip_prefix(KB_NAMESPACE) {
        return kb_shading().map(|s| (s, rest));
    }
    match name.strip_prefix(PARTY_NAMESPACE) {
        Some(rest) => party_shading().map(|s| (s, rest)),
        None => Some((main, name)),
    }
}

/// (ER party S5) The cache key of a file of a root: the main pack's as before (the bare file name), the party root's
/// with the namespace in front. (ER90) The XSI root's with its namespace.
fn ns_key(dir: &Path, file: &str) -> String {
    if PARTY_SHADING.get().and_then(|s| s.as_ref()).is_some_and(|p| p.dir == dir) {
        return format!("{PARTY_NAMESPACE}{file}");
    }
    if XSI_SHADING.get().and_then(|s| s.as_ref()).is_some_and(|p| p.dir == dir) {
        return format!("{XSI_NAMESPACE}{file}");
    }
    if KB_SHADING.get().and_then(|s| s.as_ref()).is_some_and(|p| p.dir == dir) {
        return format!("{KB_NAMESPACE}{file}");
    }
    file.to_string()
}

/// (ER90) Which second root a name that found none asked for (the log's words).
fn root_words(name: &str) -> &'static str {
    if name.starts_with(KB_NAMESPACE) {
        "the keyblades' effect root (PACK/keyblades/vfx with er/shaders.json)"
    } else if name.starts_with(XSI_NAMESPACE) {
        "the XS effect root (PACK/xsi/vfx with er/shaders.json)"
    } else {
        "the party effect root (PACK/party/vfx with er/shaders.json)"
    }
}

fn heap(kind: D3D12_HEAP_TYPE) -> D3D12_HEAP_PROPERTIES {
    D3D12_HEAP_PROPERTIES { Type: kind, ..Default::default() }
}

fn buffer_desc(bytes: u64) -> D3D12_RESOURCE_DESC {
    D3D12_RESOURCE_DESC { Dimension: D3D12_RESOURCE_DIMENSION_BUFFER, Width: bytes.max(1), Height: 1, DepthOrArraySize: 1, MipLevels: 1, SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 }, Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR, ..Default::default() }
}

fn transition(resource: &ID3D12Resource, before: D3D12_RESOURCE_STATES, after: D3D12_RESOURCE_STATES) -> D3D12_RESOURCE_BARRIER {
    D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
        Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
        Anonymous: D3D12_RESOURCE_BARRIER_0 {
            // SAFETY: a borrowed copy of the interface pointer: never released through this struct.
            Transition: std::mem::ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER { pResource: unsafe { std::mem::transmute_copy(resource) }, Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, StateBefore: before, StateAfter: after }),
        },
    }
}

unsafe fn committed(device: &ID3D12Device, kind: D3D12_HEAP_TYPE, desc: &D3D12_RESOURCE_DESC, state: D3D12_RESOURCE_STATES) -> windows::core::Result<ID3D12Resource> {
    let mut out: Option<ID3D12Resource> = None;
    device.CreateCommittedResource(&heap(kind), D3D12_HEAP_FLAG_NONE, desc, state, None, &mut out)?;
    out.ok_or_else(|| windows::core::Error::from_hresult(windows::Win32::Foundation::E_FAIL))
}

unsafe fn upload_buffer(device: &ID3D12Device, bytes: &[u8]) -> windows::core::Result<ID3D12Resource> {
    let buffer = committed(device, D3D12_HEAP_TYPE_UPLOAD, &buffer_desc(bytes.len() as u64), D3D12_RESOURCE_STATE_GENERIC_READ)?;
    let mut at: *mut c_void = std::ptr::null_mut();
    buffer.Map(0, None, Some(&mut at))?;
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), at as *mut u8, bytes.len());
    buffer.Unmap(0, None);
    Ok(buffer)
}

impl Uploader {
    /// One RGBA8 texture (no mips: every effect shader samples level 0), left readable by pixel shaders.
    unsafe fn texture(&mut self, device: &ID3D12Device, pixels: &Pixels, srgb: bool) -> Result<(ID3D12Resource, u64), String> {
        let text = |what: &str, e: windows::core::Error| format!("{what}: {e}");
        let format = if srgb { DXGI_FORMAT_R8G8B8A8_UNORM_SRGB } else { DXGI_FORMAT_R8G8B8A8_UNORM };
        let desc = D3D12_RESOURCE_DESC { Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D, Width: pixels.width as u64, Height: pixels.height, DepthOrArraySize: 1, MipLevels: 1, Format: format, SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 }, ..Default::default() };
        let texture = committed(device, D3D12_HEAP_TYPE_DEFAULT, &desc, D3D12_RESOURCE_STATE_COPY_DEST).map_err(|e| text("texture", e))?;
        let mut footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default();
        let mut total = 0u64;
        device.GetCopyableFootprints(&desc, 0, 1, 0, Some(&mut footprint), None, None, Some(&mut total));
        let staging = committed(device, D3D12_HEAP_TYPE_UPLOAD, &buffer_desc(total), D3D12_RESOURCE_STATE_GENERIC_READ).map_err(|e| text("staging", e))?;
        let mut at: *mut c_void = std::ptr::null_mut();
        staging.Map(0, None, Some(&mut at)).map_err(|e| text("Map", e))?;
        let row = pixels.width as usize * 4;
        for y in 0..pixels.height as usize {
            std::ptr::copy_nonoverlapping(pixels.data.as_ptr().add(y * row), (at as *mut u8).add(footprint.Offset as usize + y * footprint.Footprint.RowPitch as usize), row);
        }
        staging.Unmap(0, None);
        self.allocator.Reset().map_err(|e| text("allocator Reset", e))?;
        self.list.Reset(&self.allocator, None).map_err(|e| text("list Reset", e))?;
        let to = D3D12_TEXTURE_COPY_LOCATION { pResource: std::mem::transmute_copy(&texture), Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX, Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { SubresourceIndex: 0 } };
        let from = D3D12_TEXTURE_COPY_LOCATION { pResource: std::mem::transmute_copy(&staging), Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT, Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { PlacedFootprint: footprint } };
        self.list.CopyTextureRegion(&to, 0, 0, 0, &from, None);
        // (GPU-VALIDATE) readable by EVERY shader stage: some effect materials sample their textures in the vertex stage (the
        // effect root table is visible to all stages); PIXEL_SHADER_RESOURCE alone made those reads invalid (GPU-based validation
        // 1358 in the hero-light run with Crystal Snow: a 128 x 128 effect texture read by a vertex shader)
        self.list.ResourceBarrier(&[transition(&texture, D3D12_RESOURCE_STATE_COPY_DEST, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE | D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE)]);
        self.list.Close().map_err(|e| text("Close", e))?;
        self.queue.ExecuteCommandLists(&[Some(self.list.cast::<ID3D12CommandList>().map_err(|e| text("cast", e))?)]);
        self.value += 1;
        self.queue.Signal(&self.fence, self.value).map_err(|e| text("Signal", e))?;
        let waited = Instant::now();
        while self.fence.GetCompletedValue() < self.value {
            if waited.elapsed() > Duration::from_secs(10) || device.GetDeviceRemovedReason().is_err() {
                return Err("the GPU did not finish a texture upload".to_string());
            }
            std::thread::sleep(Duration::from_micros(300));
        }
        Ok((texture, total))
    }
}

impl Gpu {
    unsafe fn new(device: &ID3D12Device, formats: (i32, i32)) -> Result<Gpu, String> {
        let text = |what: &str, e: windows::core::Error| format!("{what}: {e}");
        // ---- the root signature (the contract at the head of this file)
        let material_textures = [D3D12_DESCRIPTOR_RANGE { RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SRV, NumDescriptors: MATERIAL_TEXTURES as u32, BaseShaderRegister: 0, RegisterSpace: 0, OffsetInDescriptorsFromTableStart: 0 }];
        // (PARTY-NATIVE) t12..t15: + the capture of the game's light (FRAME_VIEWS)
        let frame_textures = [D3D12_DESCRIPTOR_RANGE { RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SRV, NumDescriptors: FRAME_VIEWS as u32, BaseShaderRegister: MATERIAL_TEXTURES as u32, RegisterSpace: 0, OffsetInDescriptorsFromTableStart: 0 }];
        let samplers = [D3D12_DESCRIPTOR_RANGE { RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SAMPLER, NumDescriptors: 2048, BaseShaderRegister: 0, RegisterSpace: 0, OffsetInDescriptorsFromTableStart: 0 }];
        let constants = |register: u32| D3D12_ROOT_PARAMETER { ParameterType: D3D12_ROOT_PARAMETER_TYPE_CBV, Anonymous: D3D12_ROOT_PARAMETER_0 { Descriptor: D3D12_ROOT_DESCRIPTOR { ShaderRegister: register, RegisterSpace: 0 } }, ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL };
        let table = |ranges: &[D3D12_DESCRIPTOR_RANGE]| D3D12_ROOT_PARAMETER { ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE, Anonymous: D3D12_ROOT_PARAMETER_0 { DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE { NumDescriptorRanges: ranges.len() as u32, pDescriptorRanges: ranges.as_ptr() } }, ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL };
        let parameters = [
            constants(0),
            constants(1),
            table(&material_textures),
            table(&frame_textures),
            table(&samplers),
            D3D12_ROOT_PARAMETER { ParameterType: D3D12_ROOT_PARAMETER_TYPE_SRV, Anonymous: D3D12_ROOT_PARAMETER_0 { Descriptor: D3D12_ROOT_DESCRIPTOR { ShaderRegister: 0, RegisterSpace: 1 } }, ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL },
        ];
        let desc = D3D12_ROOT_SIGNATURE_DESC { NumParameters: parameters.len() as u32, pParameters: parameters.as_ptr(), NumStaticSamplers: 0, pStaticSamplers: std::ptr::null(), Flags: D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT };
        let mut blob: Option<ID3DBlob> = None;
        D3D12SerializeRootSignature(&desc, D3D_ROOT_SIGNATURE_VERSION_1, &mut blob, None).map_err(|e| text("root signature", e))?;
        let blob = blob.ok_or("root signature: no blob")?;
        let root: ID3D12RootSignature = device.CreateRootSignature(0, std::slice::from_raw_parts(blob.GetBufferPointer() as *const u8, blob.GetBufferSize())).map_err(|e| text("CreateRootSignature", e))?;

        // ---- the samplers: every pair of address modes, then the scene's
        let sampler_heap: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER, NumDescriptors: 2048, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, NodeMask: 0 }).map_err(|e| text("sampler heap", e))?;
        crate::gpudebug::name(&sampler_heap, "kh:vfx:sampler_heap");
        let sampler_size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER) as usize;
        let sampler_start = sampler_heap.GetCPUDescriptorHandleForHeapStart();
        let modes = [D3D12_TEXTURE_ADDRESS_MODE_WRAP, D3D12_TEXTURE_ADDRESS_MODE_CLAMP, D3D12_TEXTURE_ADDRESS_MODE_MIRROR];
        for k in 0..2048usize {
            let (u, v) = if k < 9 { (modes[k / 3], modes[k % 3]) } else { (D3D12_TEXTURE_ADDRESS_MODE_CLAMP, D3D12_TEXTURE_ADDRESS_MODE_CLAMP) };
            // (as the test app: linear, no anisotropy; the shaders sample level 0)
            let desc = D3D12_SAMPLER_DESC { Filter: D3D12_FILTER_MIN_MAG_MIP_LINEAR, AddressU: u, AddressV: v, AddressW: D3D12_TEXTURE_ADDRESS_MODE_CLAMP, MipLODBias: 0.0, MaxAnisotropy: 1, ComparisonFunc: D3D12_COMPARISON_FUNC_NEVER, BorderColor: [0.0; 4], MinLOD: 0.0, MaxLOD: D3D12_FLOAT32_MAX };
            device.CreateSampler(&desc, D3D12_CPU_DESCRIPTOR_HANDLE { ptr: sampler_start.ptr + k * sampler_size });
        }
        let srv_heap: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, NumDescriptors: (MAX_MATERIALS * MATERIAL_TEXTURES + FRAME_SLOTS * FRAME_VIEWS /* (PARTY-NATIVE) */ + post::POST_DESCRIPTORS /* (VFXR) */) as u32, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, NodeMask: 0 }).map_err(|e| text("srv heap", e))?;
        crate::gpudebug::name(&srv_heap, "kh:vfx:srv_heap");
        let srv_size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV) as usize;
        let dsv_heap: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_DSV, NumDescriptors: FRAME_SLOTS as u32, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE, NodeMask: 0 }).map_err(|e| text("dsv heap", e))?;
        crate::gpudebug::name(&dsv_heap, "kh:vfx:dsv_heap");
        let dsv_size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_DSV) as usize;
        let queue: ID3D12CommandQueue = device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC { Type: D3D12_COMMAND_LIST_TYPE_DIRECT, ..Default::default() }).map_err(|e| text("upload queue", e))?;
        let allocator: ID3D12CommandAllocator = device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT).map_err(|e| text("allocator", e))?;
        let list: ID3D12GraphicsCommandList = device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator, None).map_err(|e| text("list", e))?;
        list.Close().map_err(|e| text("Close", e))?;
        let fence: ID3D12Fence = device.CreateFence(0, D3D12_FENCE_FLAG_NONE).map_err(|e| text("fence", e))?;
        let mut uploader = Uploader { queue, allocator, list, fence, value: 0 };
        let (white, _) = uploader.texture(device, &Pixels { width: 1, height: 1, mips: vec![(0, 1, 1)], data: vec![255; 4] }, false)?;
        // The frame descriptors of every slot start as the white texture.
        let start = srv_heap.GetCPUDescriptorHandleForHeapStart();
        for k in 0..FRAME_SLOTS * FRAME_VIEWS {
            device.CreateShaderResourceView(&white, None, D3D12_CPU_DESCRIPTOR_HANDLE { ptr: start.ptr + (MAX_MATERIALS * MATERIAL_TEXTURES + k) * srv_size });
        }
        let mut slots = Vec::new();
        for _ in 0..FRAME_SLOTS {
            let buffer = committed(device, D3D12_HEAP_TYPE_UPLOAD, &buffer_desc(SLOT_BYTES as u64), D3D12_RESOURCE_STATE_GENERIC_READ).map_err(|e| text("slot buffer", e))?;
            let mut at: *mut c_void = std::ptr::null_mut();
            buffer.Map(0, None, Some(&mut at)).map_err(|e| text("Map", e))?;
            slots.push(SlotBuffer { address: buffer.GetGPUVirtualAddress(), buffer, mapped: at as *mut u8 });
        }
        Ok(Gpu { device: device.clone(), root, srv_heap, srv_size, sampler_heap, dsv_heap, dsv_size, white, textures: HashMap::new(), texture_bytes: 0, materials: Vec::new(), pending_materials: 0, meshes: Vec::new(), slots, scene_copy: None, shaders: HashMap::new(), pipelines: HashMap::new(), formats, uploader, told: Vec::new(), fog: None, fog_failed: None, lit: None, lit_failed: None /* (PARTY-NATIVE-LIGHT) */ })
    }

    fn tell(&mut self, text: String) {
        if !self.told.contains(&text) && self.told.len() < 200 {
            rlog!("vfx: {text}");
            self.told.push(text);
        }
    }

    fn shader(&mut self, dir: &Path, file: &str) -> Option<Arc<Vec<u8>>> {
        // (ER party S5) the party root's files are kept apart from the main pack's (the same names, other shaders)
        let key = ns_key(dir, file);
        if let Some(found) = self.shaders.get(&key) {
            return found.clone();
        }
        let read = std::fs::read(dir.join("er").join(file)).ok().or_else(|| xwd_stage(dir).and_then(|s| std::fs::read(s.join("er").join(file)).ok())).map(Arc::new); // (RSF round 21) KH_XWD_DIR
        if read.is_none() {
            self.tell(format!("shader file {file} is not in PACK/vfx/er (cargo run --release -p kh-shaderc -- --vfx)"));
        }
        self.shaders.insert(key, read.clone());
        read
    }

    /// A texture of the pack as a GPU texture (once per file and colour space).
    unsafe fn texture(&mut self, dir: &Path, file: &str, srgb: bool) -> Option<ID3D12Resource> {
        let key = format!("{}|{srgb}", ns_key(dir, file)); // (ER party S5) the party root's apart
        if let Some(found) = self.textures.get(&key) {
            return Some(found.clone());
        }
        // (ER party S5) a party material's texture that only the main pack has (most of them: the shared ones)
        let mut path = dir.join(file);
        if !path.is_file() {
            if let Some(main) = SHADING.get().and_then(|s| s.as_ref()).filter(|m| m.dir != dir) {
                if main.dir.join(file).is_file() {
                    path = main.dir.join(file);
                }
            }
        }
        let pixels = match Pixels::load_png(&path, false) {
            Ok(pixels) => pixels,
            Err(e) => {
                self.tell(format!("texture not read ({e}): white is used"));
                return None;
            }
        };
        let device = self.device.clone();
        match self.uploader.texture(&device, &pixels, srgb) {
            Ok((texture, bytes)) => {
                self.texture_bytes += bytes;
                self.textures.insert(key, texture.clone());
                Some(texture)
            }
            Err(e) => {
                self.tell(format!("texture {file} not uploaded ({e}): white is used"));
                None
            }
        }
    }

    /// The pipeline state of (vertex shader file, pixel shader file, `kind`).
    /// (FP) `kind.cull`: the cull mode of a non-decal draw (NONE but for one-sided mesh particles); a decal keeps `decal_cull`.
    unsafe fn pipeline(&mut self, dir: &Path, vertex_file: &str, pixel_file: &str, kind: PipeKind) -> Option<ID3D12PipelineState> {
        let PipeKind { pass, geo, distort, cull } = kind;
        let key = (ns_key(dir, vertex_file), ns_key(dir, pixel_file), pass as u8, distort, cull.0); // (ER party S5) the party root's apart
        if let Some(found) = self.pipelines.get(&key) {
            return found.clone();
        }
        let (vertex, pixel) = (self.shader(dir, vertex_file)?, self.shader(dir, pixel_file)?);
        let element = |index: u32, format: DXGI_FORMAT, slot: u32, offset: u32, instanced: bool| D3D12_INPUT_ELEMENT_DESC { SemanticName: PCSTR(c"LOC".as_ptr().cast()), SemanticIndex: index, Format: format, InputSlot: slot, AlignedByteOffset: offset, InputSlotClass: if instanced { D3D12_INPUT_CLASSIFICATION_PER_INSTANCE_DATA } else { D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA }, InstanceDataStepRate: u32::from(instanced) };
        let (f3, f4) = (DXGI_FORMAT_R32G32B32_FLOAT, DXGI_FORMAT_R32G32B32A32_FLOAT);
        let elements: Vec<D3D12_INPUT_ELEMENT_DESC> = match geo {
            Geo::Quad => (0..9).map(|k| element(k, f4, 0, k * 16, true)).collect(),
            Geo::Ribbon | Geo::Decal => vec![element(0, f3, 0, 0, false), element(1, f4, 0, 12, false), element(2, f4, 0, 28, false), element(3, f4, 0, 44, false), element(4, f4, 0, 60, false), element(5, f4, 0, 76, false), element(6, f3, 0, 92, false), element(7, f3, 0, 104, false), element(8, f4, 0, 116, false)],
            Geo::Mesh => {
                let mut v = vec![element(0, f3, 0, 0, false), element(1, f3, 0, 12, false), element(2, f4, 0, 24, false), element(3, f4, 0, 40, false), element(4, f4, 0, 56, false)];
                v.extend((0..6).map(|k| element(5 + k, f4, 1, k * 16, true)));
                v
            }
        };
        let rgb = (D3D12_COLOR_WRITE_ENABLE_RED.0 | D3D12_COLOR_WRITE_ENABLE_GREEN.0 | D3D12_COLOR_WRITE_ENABLE_BLUE.0) as u8;
        let mut blend = D3D12_BLEND_DESC::default();
        let (enable, source, dest) = match (distort, pass) {
            (true, _) | (false, Pass::Opaque) => (false, D3D12_BLEND_ONE, D3D12_BLEND_ZERO),
            // (the additive shaders hand over colour x alpha with alpha 0: premultiplied)
            (false, Pass::Additive | Pass::DecalAdd) => (true, D3D12_BLEND_ONE, D3D12_BLEND_INV_SRC_ALPHA),
            (false, Pass::Translucent | Pass::DecalBlend) => (true, D3D12_BLEND_SRC_ALPHA, D3D12_BLEND_INV_SRC_ALPHA),
            (false, Pass::Modulate) => (true, D3D12_BLEND_DEST_COLOR, D3D12_BLEND_INV_SRC_ALPHA),
        };
        blend.RenderTarget[0] = D3D12_RENDER_TARGET_BLEND_DESC { BlendEnable: enable.into(), LogicOpEnable: false.into(), SrcBlend: source, DestBlend: dest, BlendOp: D3D12_BLEND_OP_ADD, SrcBlendAlpha: D3D12_BLEND_ZERO, DestBlendAlpha: D3D12_BLEND_ONE, BlendOpAlpha: D3D12_BLEND_OP_ADD, LogicOp: D3D12_LOGIC_OP_NOOP, RenderTargetWriteMask: rgb };
        let decal = matches!(pass, Pass::DecalAdd | Pass::DecalBlend);
        let skip_chars = if decal { decal_stencil(DXGI_FORMAT(self.formats.1)) } else { None }; // (VFXR occlusion)
        let mut formats = [DXGI_FORMAT_UNKNOWN; 8];
        formats[0] = DXGI_FORMAT(self.formats.0);
        let desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            // SAFETY: a borrowed copy of the interface pointer for the duration of the call.
            pRootSignature: std::mem::transmute_copy(&self.root),
            VS: D3D12_SHADER_BYTECODE { pShaderBytecode: vertex.as_ptr() as *const c_void, BytecodeLength: vertex.len() },
            PS: D3D12_SHADER_BYTECODE { pShaderBytecode: pixel.as_ptr() as *const c_void, BytecodeLength: pixel.len() },
            BlendState: blend,
            SampleMask: u32::MAX,
            RasterizerState: D3D12_RASTERIZER_DESC {
                FillMode: D3D12_FILL_MODE_SOLID,
                // A decal: only the box's far faces, whatever is in front of them (the shader finds
                // the scene point itself), so that it also draws with the camera inside the box.
                // (FP) a one-sided mesh particle: its faces turned away (`one_sided_cull`); everything else: none
                CullMode: if decal { decal_cull() } else { cull },
                FrontCounterClockwise: false.into(),
                DepthClipEnable: (!decal).into(),
                ..Default::default()
            },
            DepthStencilState: D3D12_DEPTH_STENCIL_DESC {
                DepthEnable: true.into(),
                DepthWriteMask: if pass == Pass::Opaque && !distort { D3D12_DEPTH_WRITE_MASK_ALL } else { D3D12_DEPTH_WRITE_MASK_ZERO },
                // Reversed Z: nearer is larger.
                DepthFunc: if decal { D3D12_COMPARISON_FUNC_ALWAYS } else { D3D12_COMPARISON_FUNC_GREATER_EQUAL },
                // (VFXR occlusion) a decal skips the characters' stencil class (`decal_stencil`; the ref is set around the decals)
                StencilEnable: skip_chars.is_some().into(),
                StencilReadMask: skip_chars.map_or(0, |s| s.0),
                StencilWriteMask: 0,
                FrontFace: skip_chars.map(|s| s.1).unwrap_or_default(),
                BackFace: skip_chars.map(|s| s.1).unwrap_or_default(),
            },
            InputLayout: D3D12_INPUT_LAYOUT_DESC { pInputElementDescs: elements.as_ptr(), NumElements: elements.len() as u32 },
            PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
            NumRenderTargets: 1,
            RTVFormats: formats,
            DSVFormat: DXGI_FORMAT(self.formats.1),
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            ..Default::default()
        };
        let made: windows::core::Result<ID3D12PipelineState> = self.device.CreateGraphicsPipelineState(&desc);
        let state = match made {
            Ok(state) => Some(state),
            Err(e) => {
                self.tell(format!("pipeline state {vertex_file} + {pixel_file} ({pass:?}) could not be made ({e}): not drawn"));
                None
            }
        };
        self.pipelines.insert(key, state.clone());
        state
    }

    /// (PARTY-NATIVE) A G-buffer twin's pipeline state: the material's vertex stage and its `g<letter><id>` pixel stage into the
    /// game's three G-buffer targets (`formats`), depth test and write as the opaque pass, the characters' stencil class written
    /// (the game's light passes and its TAA treat the texel as a character's, as they do the KH characters').
    unsafe fn gb_pipeline(&mut self, n: &NativeFx, formats: GbFormats, cull: D3D12_CULL_MODE) -> Option<ID3D12PipelineState> {
        let (vertex, pixel) = (self.shader(&n.dir, &n.vertex)?, self.shader(&n.dir, &n.pixel)?);
        let element = |index: u32, format: DXGI_FORMAT, slot: u32, offset: u32, instanced: bool| D3D12_INPUT_ELEMENT_DESC { SemanticName: PCSTR(c"LOC".as_ptr().cast()), SemanticIndex: index, Format: format, InputSlot: slot, AlignedByteOffset: offset, InputSlotClass: if instanced { D3D12_INPUT_CLASSIFICATION_PER_INSTANCE_DATA } else { D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA }, InstanceDataStepRate: u32::from(instanced) };
        let (f3, f4) = (DXGI_FORMAT_R32G32B32_FLOAT, DXGI_FORMAT_R32G32B32A32_FLOAT);
        // (the layouts of `pipeline`)
        let elements: Vec<D3D12_INPUT_ELEMENT_DESC> = match n.geo {
            Geo::Quad => (0..9).map(|k| element(k, f4, 0, k * 16, true)).collect(),
            Geo::Ribbon | Geo::Decal => vec![element(0, f3, 0, 0, false), element(1, f4, 0, 12, false), element(2, f4, 0, 28, false), element(3, f4, 0, 44, false), element(4, f4, 0, 60, false), element(5, f4, 0, 76, false), element(6, f3, 0, 92, false), element(7, f3, 0, 104, false), element(8, f4, 0, 116, false)],
            Geo::Mesh => {
                let mut v = vec![element(0, f3, 0, 0, false), element(1, f3, 0, 12, false), element(2, f4, 0, 24, false), element(3, f4, 0, 40, false), element(4, f4, 0, 56, false)];
                v.extend((0..6).map(|k| element(5 + k, f4, 1, k * 16, true)));
                v
            }
        };
        let mut formats_rt = [DXGI_FORMAT_UNKNOWN; 8];
        for (k, f) in formats.0.iter().enumerate() {
            formats_rt[k] = DXGI_FORMAT(*f);
        }
        let stencil = matches!(DXGI_FORMAT(formats.1), DXGI_FORMAT_D32_FLOAT_S8X24_UINT | DXGI_FORMAT_D24_UNORM_S8_UINT);
        let replace = D3D12_DEPTH_STENCILOP_DESC { StencilFailOp: D3D12_STENCIL_OP_KEEP, StencilDepthFailOp: D3D12_STENCIL_OP_KEEP, StencilPassOp: D3D12_STENCIL_OP_REPLACE, StencilFunc: D3D12_COMPARISON_FUNC_ALWAYS };
        let desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            // SAFETY: a borrowed copy of the interface pointer for the duration of the call.
            pRootSignature: std::mem::transmute_copy(&self.root),
            VS: D3D12_SHADER_BYTECODE { pShaderBytecode: vertex.as_ptr() as *const c_void, BytecodeLength: vertex.len() },
            PS: D3D12_SHADER_BYTECODE { pShaderBytecode: pixel.as_ptr() as *const c_void, BytecodeLength: pixel.len() },
            BlendState: D3D12_BLEND_DESC::default(),
            SampleMask: u32::MAX,
            RasterizerState: D3D12_RASTERIZER_DESC { FillMode: D3D12_FILL_MODE_SOLID, CullMode: cull, FrontCounterClockwise: false.into(), DepthClipEnable: true.into(), ..Default::default() },
            DepthStencilState: D3D12_DEPTH_STENCIL_DESC { DepthEnable: true.into(), DepthWriteMask: D3D12_DEPTH_WRITE_MASK_ALL, DepthFunc: D3D12_COMPARISON_FUNC_GREATER_EQUAL, StencilEnable: stencil.into(), StencilReadMask: 0xFF, StencilWriteMask: 0xFF, FrontFace: replace, BackFace: replace },
            InputLayout: D3D12_INPUT_LAYOUT_DESC { pInputElementDescs: elements.as_ptr(), NumElements: elements.len() as u32 },
            PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
            NumRenderTargets: formats.0.len() as u32,
            RTVFormats: formats_rt,
            DSVFormat: DXGI_FORMAT(formats.1),
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            ..Default::default()
        };
        match self.device.CreateGraphicsPipelineState(&desc) {
            Ok(state) => Some(state),
            Err(e) => {
                self.tell(format!("(PARTY-NATIVE) the G-buffer twin {} + {} could not be made ({e}): the material keeps its earlier shading", n.vertex, n.pixel));
                None
            }
        }
    }

    /// (PARTY-NATIVE) The G-buffer twins' states of the ready materials for this frame's G-buffer formats, a few per call (the
    /// worker's; the drawing thread only tries the lock). Returns how many were made.
    unsafe fn make_gb_states(&mut self, formats: GbFormats, most: usize) -> usize {
        let wanted: Vec<usize> = (0..self.materials.len()).filter(|k| matches!(&self.materials[*k], MatState::Ready(m) if m.native.as_ref().is_some_and(|n| !n.gb_failed && n.gb.as_ref().is_none_or(|g| g.0 != formats)))).take(most).collect();
        for k in &wanted {
            let MatState::Ready(m) = &mut self.materials[*k] else { continue };
            let Some(n) = m.native.take() else { continue };
            let plain = self.gb_pipeline(&n, formats, n.cull);
            let turned = if n.one_sided { self.gb_pipeline(&n, formats, one_sided_cull(true)) } else { None };
            let mut n = n;
            match plain {
                Some(p) if !n.one_sided || turned.is_some() => n.gb = Some((formats, p, turned)),
                _ => n.gb_failed = true,
            }
            if let MatState::Ready(m) = &mut self.materials[*k] {
                m.native = Some(n);
            }
        }
        wanted.len()
    }

    /// One material of the registry made ready (the test app's `build_material`, for this host).
    unsafe fn build_material(&mut self, id: usize, key: &MatKey, shading: &Shading) -> MatState {
        let dir = shading.dir.clone();
        let name = key.name.as_str();
        let shaded = shading.json.get("materials").and_then(|m| m.get(name)).filter(|m| m.get("owner").is_some_and(|o| !o.is_null())).cloned();
        let owner_class = |owner: &str, class: &str| shading.json.get("owners")?.get(owner)?.get(class).filter(|c| c.get("ok").and_then(Value::as_bool) == Some(true)).cloned();
        let built = |file: &str| shading.built.get("shaders").and_then(Value::as_array).is_some_and(|list| list.iter().any(|s| s.get("file").and_then(Value::as_str) == Some(file)));
        let translated = shaded.as_ref().and_then(|s| {
            let owner = s.get("owner")?.as_str()?;
            let classes: &[&str] = match key.geo {
                Geo::Decal => &["decal"],
                Geo::Mesh => &["mesh", "sprite"],
                Geo::Quad | Geo::Ribbon => &["sprite"],
            };
            classes.iter().find_map(|class| owner_class(owner, class).map(|c| (*class, c)))
        });
        let mut params: Box<[f32; PARAM_ROWS * 4]> = Box::new([0.0; PARAM_ROWS * 4]);
        // (slot -> (file, srgb, address u, address v)); None = white
        let mut textures: Vec<Option<(String, bool, u32, u32)>> = vec![None; 10];
        let address = |mode: &str| match mode {
            "TA_Clamp" => 1u32,
            "TA_Mirror" => 2,
            _ => 0,
        };
        let blend: String;
        let mut timed = false;
        // (FP) the material's `two_sided` (its table record; the generic variant: its material file); unknown: two-sided
        let mut two_sided = shaded.as_ref().and_then(|s| s.get("two_sided")).and_then(Value::as_bool).unwrap_or(true);
        let (mut pixel_file, mut vertex_file, mut distort_file);
        // (P5) the folder the shaders are read from (the root's own; see the keyblades' fallback below)
        let mut shader_dir = dir.clone();
        let mut scene = false; // (VFX-XLATE)
        let mut first_pixel = String::new(); // (PARTY-NATIVE-LIGHT round 3) the pixel file the table names, before any fallback
        let mut native_pair: Option<(String, String)> = None; // (PARTY-NATIVE)
        // (VFX-2) the record's parameter trees (only a --root / --vxl table has them)
        let mid: Vec<(usize, usize, Value)> = shaded.as_ref().and_then(|s| s.get("params")).and_then(Value::as_array).map(|a| a.iter().filter_map(|p| Some((p.get("at")?.as_u64()? as usize, p.get("n")?.as_u64()? as usize, p.get("tree")?.clone()))).filter(|(at, n, _)| at + n <= 64 * 4).collect()).unwrap_or_default();
        match (&shaded, &translated) {
            (Some(s), Some((class, c))) => {
                blend = s.get("blend_mode").and_then(Value::as_str).unwrap_or("BLEND_Translucent").to_string();
                let shader_id = s.get("id").and_then(Value::as_u64).unwrap_or(0);
                let letter = match *class {
                    "mesh" => "m",
                    "decal" => "d",
                    _ => "s",
                };
                // (XW-b) a shader whose translation changed: its new file (`pixel_suffix` of an overlay)
                let suffix = shading.json.get("pixel_suffix").and_then(|p| p.get(format!("{letter}{shader_id}"))).and_then(Value::as_str).unwrap_or("");
                pixel_file = format!("{letter}{shader_id}{suffix}.ps.dxil");
                first_pixel.clone_from(&pixel_file); // (PARTY-NATIVE-LIGHT round 3)
                native_pair = native_files(letter, shader_id, c.get("outputs").and_then(Value::as_u64).unwrap_or(1), suffix); // (PARTY-NATIVE)
                timed = c.get("uses_time").and_then(Value::as_bool).unwrap_or(false);
                scene = c.get("scene").and_then(Value::as_bool).unwrap_or(false); // (VFX-XLATE)
                // The owner's own vertex shader for mesh particles (world position offset), when it was built.
                // (XW-d) `pixel_suffix` names the vertex / distortion shaders' new files too ("v<id>", "x<letter><id>")
                let suffix_of = |k: &str| shading.json.get("pixel_suffix").and_then(|p| p.get(k)).and_then(Value::as_str).unwrap_or("").to_string();
                let own_vs = format!("v{shader_id}{}.vs.dxil", suffix_of(&format!("v{shader_id}")));
                vertex_file = match key.geo {
                    Geo::Quad => "quad.vs.dxil".to_string(),
                    Geo::Ribbon | Geo::Decal => "plain.vs.dxil".to_string(),
                    Geo::Mesh if *class == "mesh" && c.get("vs").and_then(Value::as_bool).unwrap_or(false) && built(&own_vs) => own_vs,
                    Geo::Mesh => "mesh.vs.dxil".to_string(),
                };
                let x = format!("x{letter}{shader_id}{}.ps.dxil", suffix_of(&format!("x{letter}{shader_id}")));
                distort_file = (c.get("distort").and_then(Value::as_bool).unwrap_or(false) && key.geo != Geo::Decal && built(&x)).then_some(x);
                for (k, v) in s.get("cb").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]).iter().enumerate().take(64 * 4) {
                    params[k] = v.as_f64().unwrap_or(0.0) as f32;
                }
                params[64 * 4] = s.get("clip").and_then(Value::as_f64).unwrap_or(0.333) as f32;
                let used: Vec<u64> = c.get("textures").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_u64).collect()).unwrap_or_default();
                for (slot, t) in s.get("textures").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]).iter().enumerate().take(10) {
                    if !used.contains(&(slot as u64)) {
                        continue;
                    }
                    if let Some(file) = t.get("file").and_then(Value::as_str) {
                        textures[slot] = Some((file.to_string(), t.get("srgb").and_then(Value::as_bool).unwrap_or(false), address(t.get("address_x").and_then(Value::as_str).unwrap_or("")), address(t.get("address_y").and_then(Value::as_str).unwrap_or(""))));
                    }
                }
                if !built(&pixel_file) {
                    // (P5) the keyblades' root: an owner its own build lacks (jmt_loc_00sa, the beam trails of most keyblades)
                    // takes the main pack's build of THE SAME OWNER (the same shader and parameter layout); this material's
                    // values and textures stay its own
                    match same_owner_in_main(shading, s.get("owner").and_then(Value::as_str).unwrap_or(""), letter) {
                        Some((main_dir, file)) => {
                            self.tell(format!("{name}: its shader {pixel_file} was not built in its root: the main pack's {file} of the same owner"));
                            (shader_dir, pixel_file, distort_file, scene) = (main_dir, file, None, false);
                            native_pair = None; // (PARTY-NATIVE) another root's build: not its native pair
                            if key.geo == Geo::Mesh {
                                vertex_file = "mesh.vs.dxil".to_string();
                            }
                        }
                        None => {
                            self.tell(format!("{name}: its shader {pixel_file} was not built: not drawn"));
                            return MatState::Failed;
                        }
                    }
                }
            }
            _ => {
                // ---- the generic variant: emissive texture (or two-colour ramp) x colour, opacity texture x alpha
                if key.geo == Geo::Decal {
                    self.tell(format!("{name}: a decal without a translated shader: not drawn"));
                    return MatState::Failed;
                }
                let json = std::fs::read_to_string(dir.join("materials").join(format!("{name}.json"))).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).unwrap_or(Value::Null);
                if shaded.is_none() {
                    two_sided = json.get("two_sided").and_then(Value::as_bool).unwrap_or(true);
                }
                let scalar =|k: &str, default: f32| json.get("scalars").and_then(|s| s.get(k)).and_then(|s| s.get("value")).and_then(Value::as_f64).map_or(default, |v| v as f32);
                let vector = |k: &str| json.get("vectors").and_then(|s| s.get(k)).and_then(|s| s.get("value")).map(|v| Vec3::new(v["r"].as_f64().unwrap_or(0.0) as f32, v["g"].as_f64().unwrap_or(0.0) as f32, v["b"].as_f64().unwrap_or(0.0) as f32));
                let tex = |keys: &[&str]| keys.iter().find_map(|k| json.get("textures").and_then(|t| t.get(*k)).cloned());
                let named: Vec<(String, Value)> = json.get("textures").and_then(Value::as_object).map(|o| o.iter().map(|(k, t)| (k.clone(), t.clone())).collect()).unwrap_or_default();
                let is_data = |k: &str| {
                    let k = k.to_lowercase();
                    k.contains("nml") || k.contains("normal") || k.contains("wpo") || k.contains("dist") || k.contains("break") || k.contains("constraint") || k.contains("opa") || k.contains("mask")
                };
                let emissive = tex(&["BaseColorTex" /* (loot) a character-type material without a translated shader (the item boxes): its base colour, unlit */, "Tex Emi", "Tex Main", "Tex Core", "Emi Tex", "texture", "base_tex", "tex_A", "Tex Glow", "1 Grayscale Texture A"]).or_else(|| named.iter().find(|(k, t)| !is_data(k) && t.get("file").and_then(Value::as_str).is_some() /* (P2 hitfx) one with a packed file: Dead of Night's matcap shards (vw_so120_str00) name a cube map first, which the pack cannot decode */).map(|(_, t)| t.clone())).or_else(|| json.get("unnamed_textures").and_then(Value::as_array).and_then(|a| a.first().cloned()));
                let opacity = tex(&["Tex Opa", "1 Tex Opa", "Tex Opq", "Tex Mask", "Tex_mask1"]);
                let mono = emissive.as_ref().is_some_and(|t| t.get("sampler_type").and_then(Value::as_str) == Some("SAMPLERTYPE_Alpha"));
                let file_of = |t: &Option<Value>| t.as_ref().and_then(|t| t.get("file")).and_then(Value::as_str).map(str::to_string);
                let (e, o) = (file_of(&emissive), file_of(&opacity));
                blend = json.get("blend_mode").and_then(Value::as_str).unwrap_or("BLEND_Translucent").to_string();
                if e.is_none() && o.is_none() {
                    self.tell(format!("{name}: no translated shader and no texture recognised: not drawn"));
                    return MatState::Failed;
                }
                let (white, black) = vector("2c White Color").zip(vector("2c Black Color")).unwrap_or((vector("Emi Color").unwrap_or(Vec3::ONE), Vec3::ZERO));
                params[0..4].copy_from_slice(&[if e.is_some() { 1.0 } else { 0.0 }, if mono { 1.0 } else { 0.0 }, if o.is_some() { 1.0 } else { 0.0 }, if blend == "BLEND_Additive" { 1.0 } else { 0.0 }]);
                params[4..8].copy_from_slice(&[white.x, white.y, white.z, 1.0]);
                params[8..12].copy_from_slice(&[black.x, black.y, black.z, 1.0]);
                params[12..16].copy_from_slice(&[scalar("U Tiling", scalar("U Tiling Emi", 1.0)), scalar("V Tiling", scalar("V Tiling Emi", 1.0)), scalar("Bright Multi Emi", 1.0), scalar("Bright Add Emi", 0.0)]);
                textures[0] = e.map(|f| (f, !mono, 0, 0));
                textures[1] = o.map(|f| (f, false, 0, 0));
                pixel_file = "generic.ps.dxil".to_string();
                vertex_file = match key.geo {
                    Geo::Quad => "quad.vs.dxil".to_string(),
                    Geo::Mesh => "mesh.vs.dxil".to_string(),
                    _ => "plain.vs.dxil".to_string(),
                };
                distort_file = None;
                self.tell(format!("{name}: drawn with the generic variant (no translated shader for this kind of geometry)"));
            }
        }
        if vertex_file.is_empty() {
            vertex_file = "plain.vs.dxil".to_string();
        }
        // ---- blending: the shader's own mode, and the emitter's override of it
        let mut pass = match blend.as_str() {
            "BLEND_Additive" => Pass::Additive,
            "BLEND_Modulate" => Pass::Modulate,
            "BLEND_Opaque" | "BLEND_Masked" => Pass::Opaque,
            _ => Pass::Translucent,
        };
        if key.geo == Geo::Decal {
            // DecalBlendMode: DBM_Emissive adds emissive x opacity; the others blend the colour over the scene.
            let emissive = shaded.as_ref().and_then(|s| s.get("decal_blend")).and_then(Value::as_str) == Some("DBM_Emissive");
            pass = if emissive { Pass::DecalAdd } else { Pass::DecalBlend };
            params[64 * 4 + 2] = if emissive { 1.0 } else { 0.0 };
        }
        match (key.blend, blend.as_str()) {
            (kh_vfx::Blend::Additive, "BLEND_Translucent") if key.geo != Geo::Decal => {
                pass = Pass::Additive;
                params[64 * 4 + 2] = 1.0;
            }
            (kh_vfx::Blend::Translucent, "BLEND_Additive") if key.geo != Geo::Decal => {
                pass = Pass::Translucent;
                params[64 * 4 + 3] = 1.0;
            }
            _ => {}
        }
        // (er.x: no scene depth is read while the depth is being written)
        params[65 * 4] = if pass == Pass::Opaque { 1.0 } else { 0.0 };
        params[66 * 4] = fog_mode(pass); // (PARTY-NATIVE) KhParams.fx.x
        // (FP) a one-sided mesh particle culls the faces turned away from the camera (a mirrored instance: its own twins)
        let one_sided = key.geo == Geo::Mesh && !two_sided && CULL_ONE_SIDED.load(Ordering::Relaxed);
        let cull = if one_sided { one_sided_cull(false) } else { D3D12_CULL_MODE_NONE };
        let Some(pipeline) = self.pipeline(&shader_dir, &vertex_file, &pixel_file, PipeKind { pass, geo: key.geo, distort: false, cull }) else { return MatState::Failed };
        let distort = match &distort_file {
            Some(file) => self.pipeline(&shader_dir, &vertex_file, file, PipeKind { pass, geo: key.geo, distort: true, cull }),
            None => None,
        };
        let mirrored = if one_sided {
            let Some(twin) = self.pipeline(&shader_dir, &vertex_file, &pixel_file, PipeKind { pass, geo: key.geo, distort: false, cull: one_sided_cull(true) }) else { return MatState::Failed };
            let distort_twin = match &distort_file {
                Some(file) => self.pipeline(&shader_dir, &vertex_file, file, PipeKind { pass, geo: key.geo, distort: true, cull: one_sided_cull(true) }),
                None => None,
            };
            Some((twin, distort_twin))
        } else {
            None
        };
        // ---- the material's table: ten textures, then the two shared ones (white)
        let start = self.srv_heap.GetCPUDescriptorHandleForHeapStart();
        let mut samplers = [0u32; 16];
        samplers[10] = 0;
        samplers[11] = 4;
        samplers[12] = SAMPLER_SCENE;
        for (slot, sampler) in samplers.iter_mut().enumerate().take(MATERIAL_TEXTURES) {
            let at = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: start.ptr + (id * MATERIAL_TEXTURES + slot) * self.srv_size };
            let resource = match textures.get(slot).and_then(|t| t.as_ref()) {
                Some((file, srgb, u, v)) => {
                    *sampler = u * 3 + v;
                    // (P5) a texture the keyblades' table names but its root does not carry: the main pack's file
                    let tex_dir = match shading_main_dir_for(&dir, file) {
                        Some(main) => main,
                        None => dir.clone(),
                    };
                    self.texture(&tex_dir, file, *srgb)
                }
                None => None,
            };
            let white = self.white.clone();
            self.device.CreateShaderResourceView(resource.as_ref().unwrap_or(&white), None, at);
        }
        if one_sided {
            let n = ONE_SIDED_MADE.fetch_add(1, Ordering::Relaxed) + 1;
            if n == 1 || n.is_power_of_two() {
                rlog!("vfx: (FP) {n} one-sided mesh-particle material(s) so far cull the faces turned away from the camera (latest: {name}); fx.cull_one_sided = 0 switches it off");
            }
        }
        // (VFXR) its eye-adaptation flag (the material's own file)
        let eye = std::fs::read_to_string(dir.join("materials").join(format!("{name}.json"))).is_ok_and(|t| t.contains("\"SQEXEnableEmissiveRevEyeAdaptation\":true") || t.contains("\"SQEXEnableEmissiveRevEyeAdaptation\": true"));
        // (PARTY-NATIVE) the fog variant of a late (blended / additive / modulate) material, when its root has it (`f<pixel file>`)
        let fog_file = format!("f{pixel_file}");
        let fog = if fog_mode(pass) < 2.5 && shader_dir.join("er").join(&fog_file).is_file() {
            let plain = self.pipeline(&shader_dir, &vertex_file, &fog_file, PipeKind { pass, geo: key.geo, distort: false, cull });
            let turned = if one_sided { self.pipeline(&shader_dir, &vertex_file, &fog_file, PipeKind { pass, geo: key.geo, distort: false, cull: one_sided_cull(true) }) } else { None };
            plain.filter(|_| !one_sided || turned.is_some()).map(|p| (p, turned))
        } else {
            None
        };
        // (PARTY-NATIVE-LIGHT) the lit variant of a late forward-lit material (its owner reads the forward light), when its root has
        // it (`l<pixel file>`: Elden Ring's own light and fog at the effect pixel)
        let lit_file = format!("l{pixel_file}");
        let lit = if fog_mode(pass) < 2.5 && shader_dir.join("er").join(&lit_file).is_file() {
            let plain = self.pipeline(&shader_dir, &vertex_file, &lit_file, PipeKind { pass, geo: key.geo, distort: false, cull });
            let turned = if one_sided { self.pipeline(&shader_dir, &vertex_file, &lit_file, PipeKind { pass, geo: key.geo, distort: false, cull: one_sided_cull(true) }) } else { None };
            plain.filter(|_| !one_sided || turned.is_some()).map(|p| (p, turned))
        } else {
            None
        };
        // (PARTY-NATIVE-LIGHT round 3) why a material of a lit owner has or lacks its lit variant (each such material once, at most
        // LIT_TOLD lines): its pass and fog mode (a lit variant only for a late pass), the file the table named and the one taken
        // (a fallback to the main pack's same owner has no _fwl), the folder, and whether the lit file is there
        if (first_pixel.contains("_fwl") || lit.is_some() || shader_dir.join("er").join(&lit_file).is_file()) && LIT_TOLD.fetch_add(1, Ordering::Relaxed) < 160 {
            let found = shader_dir.join("er").join(&lit_file).is_file();
            let state = match (lit.is_some(), found) {
                (true, _) => "taken",
                (false, true) if fog_mode(pass) >= 2.5 => "found, not for this pass",
                (false, true) => "found, its pipeline state failed",
                (false, false) => "absent",
            };
            rlog!("vfx: (PARTY-NATIVE-LIGHT) {name}: pass {pass:?} (fog mode {}), shader {first_pixel} -> {pixel_file} in {}: lit variant {lit_file} {state}", fog_mode(pass), shader_dir.display());
        }
        // (PARTY-NATIVE) the native pair of a lit (deferred) opaque / masked material, when its root has it; its G-buffer twin's
        // states follow on the worker once the G-buffer's formats are known (`make_gb_states`)
        let native = match native_pair {
            Some((n_file, g_file)) if pass == Pass::Opaque && shader_dir.join("er").join(&n_file).is_file() && shader_dir.join("er").join(&g_file).is_file() => {
                let colour = self.pipeline(&shader_dir, &vertex_file, &n_file, PipeKind { pass, geo: key.geo, distort: false, cull });
                let colour_mirrored = if one_sided { self.pipeline(&shader_dir, &vertex_file, &n_file, PipeKind { pass, geo: key.geo, distort: false, cull: one_sided_cull(true) }) } else { None };
                colour.filter(|_| !one_sided || colour_mirrored.is_some()).map(|colour| Box::new(NativeFx { colour, colour_mirrored, dir: shader_dir.clone(), vertex: vertex_file.clone(), pixel: g_file, geo: key.geo, cull, one_sided, gb: None, gb_failed: false }))
            }
            _ => None,
        };
        MatState::Ready(GpuMaterial { pipeline, distort, pass, params, samplers, timed, mirrored, eye, scene, mid /* (VFX-2) */, native, fog, lit /* (PARTY-NATIVE-LIGHT) */ })
    }

    unsafe fn build_mesh(&mut self, name: &str, dir: &Path) -> MeshState {
        let Some(mesh) = read_glb(&dir.join("meshes").join(format!("{name}.glb"))) else {
            self.tell(format!("mesh {name} could not be read from PACK/vfx/meshes: its particles are not drawn"));
            return MeshState::Failed;
        };
        let bytes = |v: &[f32]| std::slice::from_raw_parts(v.as_ptr() as *const u8, v.len() * 4).to_vec();
        let index_bytes = std::slice::from_raw_parts(mesh.indices.as_ptr() as *const u8, mesh.indices.len() * 4).to_vec();
        let vertex_bytes = bytes(&mesh.vertices);
        match (upload_buffer(&self.device, &vertex_bytes), upload_buffer(&self.device, &index_bytes)) {
            (Ok(vertices), Ok(indices)) => MeshState::Ready(GpuMesh {
                vertex_view: D3D12_VERTEX_BUFFER_VIEW { BufferLocation: vertices.GetGPUVirtualAddress(), SizeInBytes: vertex_bytes.len() as u32, StrideInBytes: (MESH_VERTEX_FLOATS * 4) as u32 },
                index_view: D3D12_INDEX_BUFFER_VIEW { BufferLocation: indices.GetGPUVirtualAddress(), SizeInBytes: index_bytes.len() as u32, Format: DXGI_FORMAT_R32_UINT },
                index_count: mesh.indices.len() as u32,
                _vertices: vertices,
                _indices: indices,
            }),
            _ => {
                self.tell(format!("mesh {name}: its buffers could not be made: not drawn"));
                MeshState::Failed
            }
        }
    }
}

/// The decal box's cull mode: its far faces are drawn (measured in the standalone harness; the
/// environment variable KH_ER_VFX_DECAL_CULL = front | back | none overrides for diagnosis).
fn decal_cull() -> D3D12_CULL_MODE {
    match std::env::var("KH_ER_VFX_DECAL_CULL").as_deref() {
        Ok("front") => D3D12_CULL_MODE_FRONT,
        Ok("back") => D3D12_CULL_MODE_BACK,
        Ok("none") => D3D12_CULL_MODE_NONE,
        // MEASURED in the harness (Waterga's ground swirl, the camera inside the box and outside it):
        // with BACK the swirl draws from both places; with FRONT only from outside (the near faces).
        _ => D3D12_CULL_MODE_BACK,
    }
}

struct CpuMesh {
    /// position 3, normal 3, tangent 4, uv 4, colour 4 per vertex (UE cm, as authored).
    vertices: Vec<f32>,
    indices: Vec<u32>,
}

/// A static mesh of the effect pack: every primitive of every mesh of the file, as one triangle
/// list in UE space (the pack bakes `(x, y, z) cm -> (x, z, y) * 0.01 m` into the vertices;
/// undone here so that `kh_vfx`'s mesh transforms apply). As the test app's `read_glb`.
fn read_glb(path: &Path) -> Option<CpuMesh> {
    let bytes = std::fs::read(path).ok()?;
    let u32_at = |at: usize| bytes.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize);
    if bytes.get(0..4)? != b"glTF" {
        return None;
    }
    let json_len = u32_at(12)?;
    let json: Value = serde_json::from_slice(bytes.get(20..20 + json_len)?).ok()?;
    let bin_at = 20 + json_len;
    let bin_len = u32_at(bin_at)?;
    let bin = bytes.get(bin_at + 8..bin_at + 8 + bin_len)?;
    // (bytes, component type, components, count, stride) of an accessor.
    let accessor = |index: u64| -> Option<(&[u8], u64, usize, usize, usize)> {
        let a = json.get("accessors")?.get(index as usize)?;
        let view = json.get("bufferViews")?.get(a.get("bufferView")?.as_u64()? as usize)?;
        let offset = (view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) + a.get("byteOffset").and_then(Value::as_u64).unwrap_or(0)) as usize;
        let components = match a.get("type")?.as_str()? {
            "SCALAR" => 1,
            "VEC2" => 2,
            "VEC3" => 3,
            "VEC4" => 4,
            _ => return None,
        };
        let kind = a.get("componentType")?.as_u64()?;
        let size = match kind {
            5120 | 5121 => 1,
            5122 | 5123 => 2,
            _ => 4,
        };
        let stride = view.get("byteStride").and_then(Value::as_u64).map_or(size * components, |s| s as usize);
        Some((bin.get(offset..)?, kind, components, a.get("count")?.as_u64()? as usize, stride))
    };
    let floats = |index: u64| -> Option<(Vec<f32>, usize)> {
        let (data, kind, components, count, stride) = accessor(index)?;
        let mut out = Vec::with_capacity(count * components);
        for i in 0..count {
            for c in 0..components {
                let at = i * stride;
                out.push(match kind {
                    5126 => f32::from_le_bytes(data.get(at + c * 4..at + c * 4 + 4)?.try_into().ok()?),
                    5121 => *data.get(at + c)? as f32 / 255.0,
                    5123 => u16::from_le_bytes(data.get(at + c * 2..at + c * 2 + 2)?.try_into().ok()?) as f32 / 65535.0,
                    _ => return None,
                });
            }
        }
        Some((out, components))
    };
    let mut mesh = CpuMesh { vertices: Vec::new(), indices: Vec::new() };
    for m in json.get("meshes")?.as_array()? {
        for primitive in m.get("primitives")?.as_array()? {
            let attributes = primitive.get("attributes")?;
            let (positions, _) = floats(attributes.get("POSITION")?.as_u64()?)?;
            let base = (mesh.vertices.len() / MESH_VERTEX_FLOATS) as u32;
            let count = positions.len() / 3;
            let normals = attributes.get("NORMAL").and_then(Value::as_u64).and_then(floats).filter(|(_, n)| *n == 3).map(|(v, _)| v);
            let tangents = attributes.get("TANGENT").and_then(Value::as_u64).and_then(floats).filter(|(_, n)| *n == 4).map(|(v, _)| v);
            let uv0 = attributes.get("TEXCOORD_0").and_then(Value::as_u64).and_then(floats).filter(|(_, n)| *n == 2).map(|(u, _)| u);
            let uv1 = attributes.get("TEXCOORD_1").and_then(Value::as_u64).and_then(floats).filter(|(_, n)| *n == 2).map(|(u, _)| u);
            let colors = attributes.get("COLOR_0").and_then(Value::as_u64).and_then(floats);
            for i in 0..count {
                let p = &positions[i * 3..i * 3 + 3];
                mesh.vertices.extend_from_slice(&[p[0] * 100.0, p[2] * 100.0, p[1] * 100.0]);
                match &normals {
                    Some(n) => mesh.vertices.extend_from_slice(&[n[i * 3], n[i * 3 + 2], n[i * 3 + 1]]),
                    None => mesh.vertices.extend_from_slice(&[0.0, 0.0, 1.0]),
                }
                match &tangents {
                    // The axis swap is a reflection: the binormal's sign flips with it.
                    Some(t) => mesh.vertices.extend_from_slice(&[t[i * 4], t[i * 4 + 2], t[i * 4 + 1], -t[i * 4 + 3]]),
                    None => mesh.vertices.extend_from_slice(&[1.0, 0.0, 0.0, 1.0]),
                }
                let a = uv0.as_ref().map_or([0.0, 0.0], |u| [u[i * 2], u[i * 2 + 1]]);
                let b = uv1.as_ref().map_or(a, |u| [u[i * 2], u[i * 2 + 1]]);
                mesh.vertices.extend_from_slice(&[a[0], a[1], b[0], b[1]]);
                match &colors {
                    Some((c, 4)) => mesh.vertices.extend_from_slice(&c[i * 4..i * 4 + 4]),
                    Some((c, 3)) => mesh.vertices.extend_from_slice(&[c[i * 3], c[i * 3 + 1], c[i * 3 + 2], 1.0]),
                    _ => mesh.vertices.extend_from_slice(&[1.0; 4]),
                }
            }
            match primitive.get("indices").and_then(Value::as_u64) {
                Some(index) => {
                    let (data, kind, _, n, _) = accessor(index)?;
                    for i in 0..n {
                        mesh.indices.push(
                            base + match kind {
                                5121 => *data.get(i)? as u32,
                                5123 => u16::from_le_bytes(data.get(i * 2..i * 2 + 2)?.try_into().ok()?) as u32,
                                _ => u32::from_le_bytes(data.get(i * 4..i * 4 + 4)?.try_into().ok()?),
                            },
                        );
                    }
                }
                None => mesh.indices.extend(base..base + count as u32),
            }
        }
    }
    (!mesh.indices.is_empty()).then_some(mesh)
}

// ------------------------------------------------------------------------------------ the worker

/// Called by the renderer's worker thread when it has nothing else to do: makes our objects on
/// `device`, then a few of the materials and meshes the registry names. True: something was made
/// (call again at once).
pub(crate) fn worker_tick(device: &ID3D12Device) -> bool {
    let Some(shading) = shading() else { return false };
    let Some(formats) = FORMATS.lock().ok().and_then(|g| *g) else { return false };
    // ---- our objects, for this device and these formats
    {
        let mut guard = GPU.lock().unwrap();
        let stale = guard.as_ref().is_some_and(|g| g.device.as_raw() != device.as_raw() || g.formats != formats);
        if stale {
            // (a new device, or other formats: everything is made again)
            let old = guard.take();
            drop(guard);
            drop(POST.lock().unwrap().take()); // (VFXR)
            release_gpu(old);
            return true;
        }
        // (VFXR) the post kit, once per device and formats (its shaders: PACK/vfx/er/fxp_*.ps.dxil)
        if let Some(gpu) = guard.as_mut() {
            let mut post = POST.lock().unwrap();
            let fits = post.as_ref().is_some_and(|k| k.device == gpu.device.as_raw() as usize && k.formats == gpu.formats);
            let failed = POST_FAILED.lock().unwrap().is_some();
            if !fits && !failed {
                // SAFETY: object creation on the host's device from our own thread (free-threaded API).
                match crate::hook::as_ours(|| unsafe { post::PostKit::new(gpu, &shading.dir) }) {
                    Ok(kit) => {
                        rlog!("vfx: (VFXR) the effect layer's post passes are ready (KH3's bloom on the effects, the screen-wide passes)");
                        *post = Some(kit);
                    }
                    Err(e) => {
                        rlog!("vfx: (VFXR) no post passes ({e}): the effects are drawn without KH3's bloom and screen passes");
                        *POST_FAILED.lock().unwrap() = Some(e);
                    }
                }
            }
        }
        if guard.is_none() {
            drop(guard);
            // SAFETY: object creation on the host's device from our own thread (free-threaded API).
            match crate::hook::as_ours(|| unsafe { Gpu::new(device, formats) }) {
                Ok(gpu) => {
                    rlog!("vfx: our Direct3D 12 objects are made (target format {}, depth format {}); materials and meshes follow in the background", formats.0, formats.1);
                    *GPU.lock().unwrap() = Some(gpu);
                    return true;
                }
                Err(e) => {
                    rlog!("vfx: our Direct3D 12 objects could not be made ({e}): tried again in 5 s");
                    std::thread::sleep(Duration::from_secs(5));
                    return false;
                }
            }
        }
    }
    // ---- what is wanted and not made yet
    type Jobs<T> = Vec<(usize, T)>;
    let (material_jobs, mesh_jobs, total): (Jobs<MatKey>, Jobs<String>, usize) = {
        let registry = REGISTRY.lock().unwrap();
        let Some(registry) = registry.as_ref() else { return false };
        let guard = GPU.lock().unwrap();
        let Some(gpu) = guard.as_ref() else { return false };
        let pending_material = |id: &usize| !matches!(gpu.materials.get(*id), Some(MatState::Ready(_) | MatState::Failed));
        let pending_mesh = |id: &usize| !matches!(gpu.meshes.get(*id), Some(MeshState::Ready(_) | MeshState::Failed));
        let all: Vec<usize> = (0..registry.keys.len()).filter(pending_material).collect();
        (all.iter().take(8).map(|id| (*id, registry.keys[*id].clone())).collect(), (0..registry.mesh_names.len()).filter(pending_mesh).take(16).map(|id| (id, registry.mesh_names[id].clone())).collect(), all.len())
    };
    // (PARTY-NATIVE) the copy of the game's fog for the late effects (once, in the native mode)
    if crate::hybrid::native_now() {
        let mut guard = GPU.lock().unwrap();
        if let Some(gpu) = guard.as_mut().filter(|g| g.fog.is_none() && g.fog_failed.is_none()) {
            // SAFETY: object creation on the host's device from our own thread (free-threaded API).
            match crate::hook::as_ours(|| unsafe { FogKit::new(&gpu.device, &shading.dir) }) {
                Ok(kit) => {
                    rlog!("vfx: (PARTY-NATIVE) the game's fog for the late effects is ready (its composite's buffer and volume are copied each frame)");
                    gpu.fog = Some(kit);
                }
                Err(e) => {
                    rlog!("vfx: (PARTY-NATIVE) no copy of the game's fog ({e}): the late effects are drawn without it");
                    gpu.fog_failed = Some(e);
                }
            }
            return true;
        }
    }
    // (PARTY-NATIVE-LIGHT) the copy of the game's light inputs for the lit translucent effects (once, in the native mode)
    if crate::hybrid::native_now() {
        let mut guard = GPU.lock().unwrap();
        if let Some(gpu) = guard.as_mut().filter(|g| g.lit.is_none() && g.lit_failed.is_none()) {
            // SAFETY: object creation on the host's device from our own thread (free-threaded API).
            match crate::hook::as_ours(|| unsafe { light_kit::LitKit::new(&gpu.device, &shading.dir) }) {
                Ok(kit) => {
                    rlog!("vfx: (PARTY-NATIVE-LIGHT) Elden Ring's own light for the lit translucent effects is ready (its light passes' constants, irradiance volumes and light buffers are copied each frame)");
                    gpu.lit = Some(kit);
                }
                Err(e) => {
                    rlog!("vfx: (PARTY-NATIVE-LIGHT) no copy of the game's light inputs ({e}): the lit translucent effects keep their earlier light");
                    gpu.lit_failed = Some(e);
                }
            }
            return true;
        }
    }
    // (PARTY-NATIVE) the lit effects' G-buffer twins for this frame's G-buffer formats (a few per tick)
    if let Some(gb) = GB_FORMATS.lock().ok().and_then(|g| *g) {
        let mut guard = GPU.lock().unwrap();
        if let Some(gpu) = guard.as_mut() {
            // SAFETY: as above.
            if crate::hook::as_ours(|| unsafe { gpu.make_gb_states(gb, 4) }) > 0 {
                return true;
            }
        }
    }
    if material_jobs.is_empty() && mesh_jobs.is_empty() {
        if let Ok(mut guard) = GPU.lock() {
            if let Some(gpu) = guard.as_mut() {
                gpu.pending_materials = 0;
            }
        }
        return false;
    }
    // (the lock is held while one item is made, never over the whole batch: the drawing thread
    // gets in between; it never waits longer than one texture or pipeline state takes ... and it
    // only TRIES the lock: a busy frame draws no effects rather than wait)
    for (id, name) in mesh_jobs {
        let mut guard = GPU.lock().unwrap();
        let Some(gpu) = guard.as_mut() else { return false };
        while gpu.meshes.len() <= id {
            gpu.meshes.push(MeshState::Pending);
        }
        // SAFETY: as above.
        // (ER party S5) a "party:" name from the party root
        gpu.meshes[id] = match split_ns(&name, shading) {
            Some((root, name)) => crate::hook::as_ours(|| unsafe { gpu.build_mesh(name, &root.dir) }),
            None => {
                gpu.tell(format!("mesh {name}: {} is not there: not drawn", root_words(&name)));
                MeshState::Failed
            }
        };
    }
    for (id, key) in material_jobs {
        let mut guard = GPU.lock().unwrap();
        let Some(gpu) = guard.as_mut() else { return false };
        while gpu.materials.len() <= id {
            gpu.materials.push(MatState::Pending);
        }
        // SAFETY: as above.
        // (ER party S5) a "party:" name: the party root's own table, shaders, materials and meshes (never the main table)
        gpu.materials[id] = match split_ns(&key.name, shading) {
            Some((root, name)) => {
                let key = MatKey { name: name.to_string(), ..key.clone() };
                crate::hook::as_ours(|| unsafe { gpu.build_material(id, &key, root) })
            }
            None => {
                gpu.tell(format!("{}: {} is not there: not drawn", key.name, root_words(&key.name)));
                MatState::Failed
            }
        };
        gpu.pending_materials = total;
    }
    true
}

fn release_gpu(gpu: Option<Gpu>) {
    if let Some(gpu) = gpu {
        // SAFETY: our own buffers.
        unsafe {
            for slot in &gpu.slots {
                slot.buffer.Unmap(0, None);
            }
        }
        drop(gpu);
    }
}

/// Frees everything (shutdown, a lost device).
pub(crate) fn release() {
    clear();
    drop(POST.lock().ok().and_then(|mut p| p.take())); // (VFXR)
    if let Ok(mut f) = POST_FAILED.lock() {
        *f = None;
    }
    let old = GPU.lock().ok().and_then(|mut g| g.take());
    release_gpu(old);
}

// ------------------------------------------------------------------------------------ the draw

/// What gfx.rs hands over for one frame: its list (Sora is drawn; the target is bound as the
/// render target and the scene depth, in DEPTH_WRITE, as the depth target), and the frame's view.
pub(crate) struct DrawCx<'a> {
    pub device: &'a ID3D12Device,
    pub list: &'a ID3D12GraphicsCommandList,
    pub slot: usize,
    pub target: &'a ID3D12Resource,
    pub target_view: D3D12_CPU_DESCRIPTOR_HANDLE,
    pub depth: &'a ID3D12Resource,
    /// The depth-stencil view format of the scene depth.
    pub depth_format: DXGI_FORMAT,
    /// The read-write view gfx.rs made for this frame.
    pub depth_view: D3D12_CPU_DESCRIPTOR_HANDLE,
    pub width: u32,
    pub height: u32,
    pub clip_from_game: Mat4,
    pub camera: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    pub forward: Vec3,
    /// view depth (m) = 1 / (device * a + b)
    pub depth_a: f32,
    pub depth_b: f32,
    /// Projection [0][0].
    pub proj00: f32,
    /// What KH3's scene-linear 1.0 of an effect becomes in the target.
    pub scale: f32,
    /// What a white lit surface shows in the target's units.
    pub lit: Vec3,
    pub stage: Stage,
}

/// Records the effects of the latest submitted frame into `cx.list`. Nothing here can fail the
/// frame: what is not ready, does not fit or cannot be made is left out.
///
/// # Safety
/// `cx.list` is open, with `cx.target` bound as the render target (RENDER_TARGET state) and
/// `cx.depth` (every plane in DEPTH_WRITE) as its depth target; both are alive.
pub(crate) unsafe fn draw(cx: &DrawCx) {
    let started = Instant::now();
    let options = options();
    // (the worker makes pipeline states for these formats)
    let target_desc = cx.target.GetDesc();
    if let Ok(mut formats) = FORMATS.lock() {
        *formats = Some((target_desc.Format.0, cx.depth_format.0));
    }
    if !options.on {
        return;
    }
    let Some(prepared) = LATEST.lock().ok().and_then(|g| g.clone()) else { return };
    // (never waited for: while the worker is making something this frame draws no effects)
    let Ok(mut guard) = GPU.try_lock() else {
        NOT_READY.fetch_add(1, Ordering::Relaxed);
        return;
    };
    let Some(gpu) = guard.as_mut() else { return };
    if gpu.device.as_raw() != cx.device.as_raw() || gpu.formats != (target_desc.Format.0, cx.depth_format.0) || cx.slot >= gpu.slots.len() {
        return;
    }
    let depth_srv_format = match cx.depth.GetDesc().Format {
        DXGI_FORMAT_R32G8X24_TYPELESS | DXGI_FORMAT_D32_FLOAT_S8X24_UINT => DXGI_FORMAT_R32_FLOAT_X8X24_TYPELESS,
        DXGI_FORMAT_R24G8_TYPELESS | DXGI_FORMAT_D24_UNORM_S8_UINT => DXGI_FORMAT_R24_UNORM_X8_TYPELESS,
        DXGI_FORMAT_R32_TYPELESS => DXGI_FORMAT_R32_FLOAT,
        DXGI_FORMAT_R16_TYPELESS => DXGI_FORMAT_R16_UNORM,
        // (a depth that cannot be read as a texture: no effects)
        _ => return,
    };
    let has_stencil = matches!(cx.depth_format, DXGI_FORMAT_D32_FLOAT_S8X24_UINT | DXGI_FORMAT_D24_UNORM_S8_UINT);

    // ---- the scene colour copy of the distortion twins (made when first needed, or another size)
    let ready = |id: u32| match gpu.materials.get(id as usize) {
        Some(MatState::Ready(m)) => Some(m),
        _ => None,
    };
    // (VFXR) the post kit of this frame (KH3's bloom on the effects, the screen-wide passes): a float HDR scene only
    let frame_id = Arc::as_ptr(&prepared) as usize;
    let mut post_guard = if post::hdr_target(&target_desc) { POST.try_lock().ok() } else { None };
    let mut post_kit = post_guard.as_mut().and_then(|g| g.as_mut()).filter(|k| k.device == cx.device.as_raw() as usize && k.formats == (target_desc.Format.0, cx.depth_format.0));
    let late_stage = matches!(cx.stage, Stage::All | Stage::Late);
    let post_bloom = post_kit.is_some() && post::bloom_factor() > 0.0;
    let post_screen = post_kit.is_some() && late_stage && post::screen_on() && !prepared.screen.is_empty();
    let wants_copy = (post_bloom && late_stage && (!prepared.quads.is_empty() || !prepared.meshes.is_empty() || !prepared.plain.is_empty())) || prepared.quads.iter().map(|b| b.material).chain(prepared.meshes.iter().map(|b| b.material)).chain(prepared.plain.iter().map(|b| b.material)).any(|id| ready(id).is_some_and(|m| (options.distort && m.distort.is_some()) || m.scene /* (VFX-XLATE) */));
    // (only of a plain single-sample 2D target: the copy is made with the target's own description, so that
    // CopyResource is always between two resources of one shape)
    let copyable = target_desc.Dimension == D3D12_RESOURCE_DIMENSION_TEXTURE2D && target_desc.SampleDesc.Count == 1 && target_desc.DepthOrArraySize == 1;
    if wants_copy && copyable {
        let wanted = (target_desc.Width as u32, target_desc.Height, target_desc.Format.0);
        if gpu.scene_copy.as_ref().map(|(_, w, h, f)| (*w, *h, *f)) != Some(wanted) {
            let desc = D3D12_RESOURCE_DESC { Flags: D3D12_RESOURCE_FLAG_NONE, Alignment: 0, ..target_desc };
            let made = committed(cx.device, D3D12_HEAP_TYPE_DEFAULT, &desc, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE).ok();
            // (the one before may be named by a frame in flight: kept until the GPU is past it, GPUSAFE)
            if let Some((old, ..)) = std::mem::replace(&mut gpu.scene_copy, made.map(|t| (t, wanted.0, wanted.1, wanted.2))) {
                crate::outfit::retire(Box::new(old));
            }
        }
    }
    // (PARTY-NATIVE) THE GAME'S FOG for the late effects, the native mode only: this frame's composite bindings (ffx.rs), the
    // volume's size this slot read back last time; `fog_ready` = its copy is drawn below and the fog variants may read it
    let fog_bind = crate::ffx::fog_bind().filter(|_| crate::hybrid::native_now() && matches!(cx.stage, Stage::Late | Stage::OverFinale));
    if fog_bind.is_some() {
        let device = gpu.device.clone();
        if let Some(kit) = gpu.fog.as_mut() {
            kit.take_size(&device, cx.slot);
        }
    }
    // (PARTY-NATIVE-LIGHT) ELDEN RING'S OWN LIGHT for the lit translucent effects, the native mode only: this frame's light inputs
    // (ffx.rs `lit_binds`: the tile lighting at least), the sizes this slot read back last time
    let lit_late = crate::hybrid::native_now() && matches!(cx.stage, Stage::Late | Stage::OverFinale); // (round 2)
    let lit_binds = if lit_late { Some(crate::ffx::lit_binds()).filter(|b| b.tile.is_some()) } else { None };
    if let Some(binds) = lit_binds.as_ref() {
        let device = gpu.device.clone();
        if let Some(kit) = gpu.lit.as_mut() {
            kit.prepare(&device, cx.slot, binds);
        }
    }
    let gpu: &Gpu = gpu;
    // (PARTY-NATIVE-LIGHT) what the copy below draws (the frame's lit rows say so), the sun of this frame
    let lit_plan = match (lit_binds.as_ref(), gpu.lit.as_ref()) {
        (Some(b), Some(k)) => k.plan(b),
        _ => light_kit::LitPlan::default(),
    };
    let lit_sun = light_kit::sun();
    let fog_ready = fog_bind.is_some() && gpu.fog.as_ref().is_some_and(|k| k.volume.as_ref().is_some_and(|v| v.2 == k.size));
    let copy_ready = wants_copy && copyable && gpu.scene_copy.as_ref().is_some_and(|(_, w, h, f)| (*w, *h, *f) == (target_desc.Width as u32, target_desc.Height, target_desc.Format.0));

    // ---- what is drawn, by pass
    enum Item<'a> {
        Quad(&'a QuadBatch),
        Mesh(&'a MeshBatch),
        Plain(&'a PlainBatch),
        Skel(&'a skelfx::Posed), // (VFX-2)
    }
    let mut opaque: Vec<(Item, &GpuMaterial)> = Vec::new();
    let mut decals: Vec<(Item, &GpuMaterial)> = Vec::new();
    let mut blended: Vec<(SortKey, Item, &GpuMaterial)> = Vec::new(); // (VFXR sort) KH3's order
    let mut not_ready = 0u32;
    let meshes = &gpu.meshes;
    let mut items: Vec<(Item, u32, Vec3, SortKey)> = Vec::new();
    items.extend(prepared.quads.iter().map(|b| (Item::Quad(b), b.material, b.center, b.sort)));
    for batch in &prepared.meshes {
        match meshes.get(batch.mesh as usize) {
            Some(MeshState::Ready(_)) => items.push((Item::Mesh(batch), batch.material, batch.center, batch.sort)),
            Some(MeshState::Failed) => {}
            _ => not_ready += 1,
        }
    }
    items.extend(prepared.plain.iter().map(|b| (Item::Plain(b), b.material, b.center, b.sort)));
    items.extend(prepared.skel.iter().map(|b| (Item::Skel(b), b.material, b.center, b.sort))); // (VFX-2)
    let grouped = SORT_GROUPS.load(Ordering::Relaxed); // (VFXR sort)
    for (item, material, center, sort) in items {
        match gpu.materials.get(material as usize) {
            Some(MatState::Ready(m)) => match m.pass {
                Pass::Opaque => opaque.push((item, m)),
                Pass::DecalAdd | Pass::DecalBlend => decals.push((item, m)),
                // (VFXR sort) KH3's order (the group's key); `vfx.sort = 0`: far to near by the batch's centre, as before
                _ => blended.push((if grouped { sort } else { (0, -(center - cx.camera).dot(cx.forward), 0, 0) }, item, m)),
            },
            Some(MatState::Failed) => {}
            _ => not_ready += 1,
        }
    }
    NOT_READY.fetch_add(not_ready, Ordering::Relaxed);
    // (IMPL_ER74) `vfx.decals_early = 0`: the decals where they were (after him: on him), for comparing
    let early_decals = DECALS_EARLY.load(Ordering::Relaxed);
    match cx.stage {
        Stage::All if !early_decals => {}
        Stage::Late if !early_decals => opaque.clear(),
        Stage::Decals if !early_decals => {
            opaque.clear();
            decals.clear();
            blended.clear();
        }
        Stage::All => decals.clear(),
        Stage::Early => {
            decals.clear();
            blended.clear();
        }
        Stage::Late => {
            opaque.clear();
            decals.clear();
        }
        Stage::Decals => {
            opaque.clear();
            blended.clear();
        }
        // (ZFIN)
        Stage::OverFinale => {
            opaque.clear();
            decals.clear();
        }
    }
    // (ZFIN) the materials over Zettaflare's finale: held back for `Stage::OverFinale` while one is drawn, only them there
    if cx.stage == Stage::OverFinale || (matches!(cx.stage, Stage::All | Stage::Late) && !prepared.over.is_empty() && crate::finale::active()) {
        let over_stage = cx.stage == Stage::OverFinale;
        blended.retain(|(_, item, _)| {
            let id = match item {
                Item::Quad(b) => b.material,
                Item::Mesh(b) => b.material,
                Item::Plain(b) => b.material,
                Item::Skel(b) => b.material, // (VFX-2)
            };
            prepared.over.contains(&id) == over_stage
        });
        if over_stage && blended.is_empty() {
            return;
        }
    }
    // (the upload buffer of the slot: the first call of the frame (the decals) starts it, the later ones continue behind)
    let start_at = if cx.stage != Stage::Decals { CURSORS.lock().ok().and_then(|c| c.get(cx.slot).copied()).unwrap_or(0) } else { 0 };
    if cx.stage == Stage::Decals {
        if let Ok(mut c) = CURSORS.lock() {
            if let Some(at) = c.get_mut(cx.slot) {
                *at = 0;
            }
        }
    }
    if opaque.is_empty() && decals.is_empty() && blended.is_empty() && !post_screen /* (VFXR) */ {
        return;
    }
    // (VFXR sort) KH3's translucency order: priority, then far to near, then the emitters of one component in their order
    blended.sort_by(|a, b| a.0 .0.cmp(&b.0 .0).then(a.0 .1.total_cmp(&b.0 .1)).then(a.0 .2.cmp(&b.0 .2)).then(a.0 .3.cmp(&b.0 .3)));
    // (VFX-XLATE) a material that reads the frame behind it is not drawn without the copy (it would read white)
    if !copy_ready {
        blended.retain(|(_, _, m)| !m.scene);
    }
    let distorted: Vec<usize> = if copy_ready && options.distort /* (VFX-XLATE) the copy may be made for a scene reader alone */ { (0..blended.len()).filter(|k| blended[*k].2.distort.is_some()).collect() } else { Vec::new() };

    // ---- the slot's upload buffer
    let slot = &gpu.slots[cx.slot];
    let mut cursor = start_at;
    let mut overflow = 0u32;
    let mut put = |bytes: &[u8], align: usize| -> Option<u64> {
        let at = (cursor + align - 1) & !(align - 1);
        if at + bytes.len() > SLOT_BYTES {
            overflow += 1;
            return None;
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), slot.mapped.add(at), bytes.len());
        cursor = at + bytes.len();
        Some(slot.address + at as u64)
    };
    fn as_bytes<T>(v: &[T]) -> &[u8] {
        // SAFETY: plain floats / words.
        unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, std::mem::size_of_val(v)) }
    }
    let mut frame: Vec<f32> = Vec::with_capacity(48);
    frame.extend_from_slice(&cx.clip_from_game.to_cols_array());
    frame.extend_from_slice(&[cx.camera.x, cx.camera.y, cx.camera.z, 0.0]);
    frame.extend_from_slice(&[cx.right.x, cx.right.y, cx.right.z, 0.0]);
    frame.extend_from_slice(&[cx.up.x, cx.up.y, cx.up.z, 0.0]);
    frame.extend_from_slice(&[cx.forward.x, cx.forward.y, cx.forward.z, 0.0]);
    frame.extend_from_slice(&[cx.width as f32, cx.height as f32, 1.0 / cx.width as f32, 1.0 / cx.height as f32]);
    frame.extend_from_slice(&[cx.depth_a, cx.depth_b, 1.0, cx.proj00]);
    frame.extend_from_slice(&[cx.scale * options.hdr, if options.solid { 1.0 } else { 0.0 }, 1.0, 0.0]);
    let lit = cx.lit / (cx.scale * options.hdr).max(1.0e-9);
    frame.extend_from_slice(&[lit.x, lit.y, lit.z, 1.0]);
    crate::fwdlight::extend_frame(&mut frame, cx.scale * options.hdr); // (VFX-2 lit) ErFrame.fwd_*: Elden Ring's light for the forward-lit effects
    // (PARTY-NATIVE) ErFrame.fxn: this frame's capture of the game's light for the lit effects (gfx.rs `set_capture`)
    let capture = FX_CAPTURE.lock().ok().and_then(|c| *c);
    frame.resize(48 + crate::fwdlight::FRAME_FLOATS, 0.0);
    frame.extend_from_slice(&fxn_row(capture));
    // (PARTY-NATIVE) ErFrame.fxf: the copy of the game's fog is bound at t16 / t17 this frame
    frame.extend_from_slice(&[if fog_ready { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0][..FXF_FLOATS]);
    // (PARTY-NATIVE-LIGHT) ErFrame.fxl: what the copy of the game's light inputs holds this frame, the sun's colour and cascades
    frame.extend_from_slice(&light_kit::fxl_row(&lit_plan, lit_sun.as_ref()));
    let Some(frame_constants) = put(as_bytes(&frame), 256) else { return };
    // (VFXR) the frame constants of the eye-adaptation materials: KH3's emissive 1.0 at the host tone mapper's 1.0
    let boost = post::eye_boost();
    let frame_eye = if boost != 1.0 {
        let mut f = frame.clone();
        let k = boost.max(1.0e-6);
        // (out_scale.x is float 40, lit.xyz 44..46: see `frame` above)
        f[40] *= k;
        f[44] /= k;
        f[45] /= k;
        f[46] /= k;
        crate::fwdlight::eye(&mut f[48..], k); // (VFX-2 lit) the forward lights' colours as lit
        match put(as_bytes(&f), 256) {
            Some(at) => at,
            None => return,
        }
    } else {
        frame_constants
    };

    // ---- the frame's two views: the scene depth (read-only) and the scene colour copy
    let start = gpu.srv_heap.GetCPUDescriptorHandleForHeapStart();
    let gpu_start = gpu.srv_heap.GetGPUDescriptorHandleForHeapStart();
    let frame_table = MAX_MATERIALS * MATERIAL_TEXTURES + cx.slot * FRAME_VIEWS; // (PARTY-NATIVE) FRAME_VIEWS
    let depth_srv = D3D12_SHADER_RESOURCE_VIEW_DESC { Format: depth_srv_format, ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D, Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING, Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 { Texture2D: D3D12_TEX2D_SRV { MostDetailedMip: 0, MipLevels: 1, PlaneSlice: 0, ResourceMinLODClamp: 0.0 } } };
    cx.device.CreateShaderResourceView(cx.depth, Some(&depth_srv), D3D12_CPU_DESCRIPTOR_HANDLE { ptr: start.ptr + frame_table * gpu.srv_size });
    match gpu.scene_copy.as_ref().filter(|_| copy_ready) {
        Some((copy, ..)) => cx.device.CreateShaderResourceView(copy, None, D3D12_CPU_DESCRIPTOR_HANDLE { ptr: start.ptr + (frame_table + 1) * gpu.srv_size }),
        None => cx.device.CreateShaderResourceView(&gpu.white, None, D3D12_CPU_DESCRIPTOR_HANDLE { ptr: start.ptr + (frame_table + 1) * gpu.srv_size }),
    }
    // (PARTY-NATIVE) t14 / t15: the capture of the game's light (the characters' own targets; white without one)
    for (k, raw) in [capture.map(|c| c.light), capture.map(|c| c.geo)].into_iter().enumerate() {
        let at = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: start.ptr + (frame_table + 2 + k) * gpu.srv_size };
        let raw = raw.unwrap_or(0) as *mut c_void;
        match ID3D12Resource::from_raw_borrowed(&raw) {
            Some(resource) if !raw.is_null() => cx.device.CreateShaderResourceView(resource, None, at),
            _ => cx.device.CreateShaderResourceView(&gpu.white, None, at),
        }
    }
    // (PARTY-NATIVE) t16 / t17: the copy of the game's fog (volume, rows); white without one (no shader reads them then)
    let fog_kit = gpu.fog.as_ref();
    let fog_views: [Option<&ID3D12Resource>; 2] = [fog_kit.and_then(|k| k.volume.as_ref().map(|v| &v.0)).filter(|_| fog_ready), fog_kit.map(|k| &k.rows).filter(|_| fog_ready)];
    for (k, resource) in fog_views.into_iter().enumerate() {
        let at = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: start.ptr + (frame_table + 4 + k) * gpu.srv_size };
        cx.device.CreateShaderResourceView(resource.unwrap_or(&gpu.white), None, at);
    }
    // (PARTY-NATIVE-LIGHT) t18..t29: the copy of the game's light inputs and the sun's cascades (null views of the right kind without)
    light_kit::write_views(cx.device, gpu.lit.as_ref(), &lit_plan, lit_sun.as_ref(), |k| D3D12_CPU_DESCRIPTOR_HANDLE { ptr: start.ptr + (frame_table + 6 + k) * gpu.srv_size });
    let read_only_view = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: gpu.dsv_heap.GetCPUDescriptorHandleForHeapStart().ptr + cx.slot * gpu.dsv_size };
    let flags = if has_stencil { D3D12_DSV_FLAG_READ_ONLY_DEPTH | D3D12_DSV_FLAG_READ_ONLY_STENCIL } else { D3D12_DSV_FLAG_READ_ONLY_DEPTH };
    cx.device.CreateDepthStencilView(cx.depth, Some(&D3D12_DEPTH_STENCIL_VIEW_DESC { Format: cx.depth_format, ViewDimension: D3D12_DSV_DIMENSION_TEXTURE2D, Flags: flags, Anonymous: D3D12_DEPTH_STENCIL_VIEW_DESC_0 { Texture2D: D3D12_TEX2D_DSV { MipSlice: 0 } } }), read_only_view);

    // ---- recording
    let list = cx.list;
    // (PARTY-NATIVE) the copy of this frame's fog first (its own root signature and the game's heap; ours are bound below). The
    // rows (and the volume's size for the readback) are copied whenever the composite was seen; the volume once its size is known.
    let fog_drawn = match (fog_bind, fog_kit) {
        (Some(bind), Some(kit)) if !blended.is_empty() || !fog_ready => kit.copy(list, cx.slot, bind),
        _ => false,
    };
    let fog_ready = fog_ready && fog_drawn;
    if fog_ready {
        FOG_FRAMES.fetch_add(1, Ordering::Relaxed);
    }
    // (PARTY-NATIVE-LIGHT) the copy of this frame's light inputs (its own root signature and the game's heap; ours are bound below)
    // (round 2: in every late frame with this frame's tile lighting, whatever else the frame has: without an irradiance-volume source
    // the SH holds the local lights alone, the sun is per pixel; the lit variants are drawn whenever this copy is)
    let lit_ready = match (lit_binds.as_ref(), gpu.lit.as_ref()) {
        (Some(b), Some(k)) if lit_plan.scene => k.copy(list, cx.slot, b, &lit_plan),
        _ => false,
    };
    if lit_ready {
        LIT_FRAMES.fetch_add(1, Ordering::Relaxed);
    }
    if lit_late {
        light_kit::note_frame(lit_ready, &lit_plan, lit_sun.as_ref(), blended.iter().any(|(_, _, m)| m.lit.is_some()));
    }
    list.SetGraphicsRootSignature(&gpu.root);
    list.SetDescriptorHeaps(&[Some(gpu.srv_heap.clone()), Some(gpu.sampler_heap.clone())]);
    list.SetGraphicsRootConstantBufferView(0, frame_constants);
    list.SetGraphicsRootDescriptorTable(3, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: gpu_start.ptr + (frame_table * gpu.srv_size) as u64 });
    list.SetGraphicsRootDescriptorTable(4, gpu.sampler_heap.GetGPUDescriptorHandleForHeapStart());
    list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
    list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: cx.width as f32, Height: cx.height as f32, MinDepth: 0.0, MaxDepth: 1.0 }]);
    list.RSSetScissorRects(&[windows::Win32::Foundation::RECT { left: 0, top: 0, right: cx.width as i32, bottom: cx.height as i32 }]);
    let (mut batches, mut instances) = (0u32, 0u64);
    let seconds = prepared.seconds;
    // (FP) `mirror`: a one-sided mesh material's twin of `pipeline` for its mirrored instances (GpuMaterial::mirrored)
    // (PARTY-NATIVE) `gb`: the G-buffer twin's states (plain, mirrored) of a lit material drawn on the native route: each draw is
    // also kept for `gbuffer_replay` with the same constants and geometry
    let mut replays: Vec<Replay> = Vec::new();
    let mut emit = |item: &Item, m: &GpuMaterial, material_id: u32, pipeline: &ID3D12PipelineState, mirror: Option<&ID3D12PipelineState>, gb: Option<(&ID3D12PipelineState, Option<&ID3D12PipelineState>)>, put: &mut dyn FnMut(&[u8], usize) -> Option<u64>| {
        let table = gpu_start.ptr + (material_id as usize * MATERIAL_TEXTURES * gpu.srv_size) as u64;
        let mut keep = |turned: bool, frame: u64, material: u64, samplers: u64, vertex: Vec<D3D12_VERTEX_BUFFER_VIEW>, index: Option<D3D12_INDEX_BUFFER_VIEW>, count: u32, instances: u32| {
            let pso = gb.and_then(|(plain, twin)| if turned { twin } else { Some(plain) });
            if let Some(pso) = pso.filter(|_| replays.len() < MAX_REPLAYS) {
                replays.push(Replay { pso: pso.clone(), frame, material, samplers, table, vertex, index, count, instances });
            }
        };
        let mut params = *m.params;
        if m.timed {
            params[64 * 4 + 1] = seconds;
        }
        // (DON-MAGIC tint) the batch's recolour: KhParams.er.yzw = tint x intensity (er_vfx.wgsl recolours the final colour)
        let tint = match item {
            Item::Quad(b) => b.tint,
            Item::Mesh(b) => b.tint,
            Item::Plain(b) => b.tint,
            Item::Skel(_) => [0.0; 4], // (VFX-2)
        };
        // (VFX-2) a skeletal mesh component's dynamic material: its parameters evaluated into the material's constants
        if let Item::Skel(b) = item {
            for (at, n, tree) in &m.mid {
                let v = skelfx::eval_tree(tree, seconds, &b.params);
                params[*at..*at + *n].copy_from_slice(&v[..*n]);
            }
        }
        // (AUDIT-C) a DECAL's constants that are arithmetic over what its emitter drives (Decal_CustomParam0..2, Decal_Color,
        // Decal_Opacity: TypeDataSQEX_Decal, every frame) evaluated per decal, as UE evaluates uniform expressions per draw;
        // the translator puts only a bare parameter into the pixel input (sora_vfx_shaders.js DECAL_PARAMS), so e.g. Rage's
        // finisher pool gdi_ant_fin_dcl00, whose dissolve is clamp(CustomParam1) x (smooth range + 1) ..., stayed solid at rest
        // and popped off at its 5 s end instead of dissolving at 2 .. 3 s. Inputs read from the decal's own vertex (the layout
        // written above: colour at 7, time / custom at 19).
        if let Item::Plain(b) = item {
            if b.decal && b.vertices.len() >= PLAIN_FLOATS {
                let v = &b.vertices;
                let inputs = [
                    ("Decal_Color".to_string(), [v[7], v[8], v[9], v[10]], true),
                    ("Decal_Opacity".to_string(), [v[10]; 4], false),
                    ("Decal_CustomParam0".to_string(), [v[20]; 4], false),
                    ("Decal_CustomParam1".to_string(), [v[21]; 4], false),
                    ("Decal_CustomParam2".to_string(), [v[22]; 4], false),
                ];
                for (at, n, tree) in m.mid.iter().filter(|(_, _, t)| t.to_string().contains("\"Decal_")) {
                    let value = skelfx::eval_tree(tree, seconds, &inputs);
                    params[*at..*at + *n].copy_from_slice(&value[..*n]);
                }
            }
        }
        if tint[3] > 0.0 {
            params[65 * 4 + 1] = tint[0] * tint[3];
            params[65 * 4 + 2] = tint[1] * tint[3];
            params[65 * 4 + 3] = tint[2] * tint[3];
        }
        let (Some(material_constants), Some(sampler_index)) = (put(as_bytes(&params), 256), put(as_bytes(&m.samplers), 256)) else { return };
        // (VFXR) the eye-adaptation materials' frame constants (not for modulate: it multiplies the scene)
        // (VFXR decal eye) not for the deferred decals either, unless `vfx.eye_decals = 1` (EYE_DECALS)
        let eye_here = m.eye && m.pass != Pass::Modulate && (EYE_DECALS.load(std::sync::atomic::Ordering::Relaxed) || !matches!(m.pass, Pass::DecalAdd | Pass::DecalBlend));
        let frame_now = if eye_here { frame_eye } else { frame_constants };
        list.SetGraphicsRootConstantBufferView(0, frame_now);
        match item {
            Item::Quad(batch) => {
                let Some(at) = put(as_bytes(&batch.data), 16) else { return };
                let count = (batch.data.len() / QUAD_FLOATS) as u32;
                list.SetPipelineState(pipeline);
                list.SetGraphicsRootConstantBufferView(1, material_constants);
                list.SetGraphicsRootDescriptorTable(2, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: gpu_start.ptr + (material_id as usize * MATERIAL_TEXTURES * gpu.srv_size) as u64 });
                list.SetGraphicsRootShaderResourceView(5, sampler_index);
                let view = D3D12_VERTEX_BUFFER_VIEW { BufferLocation: at, SizeInBytes: (batch.data.len() * 4) as u32, StrideInBytes: (QUAD_FLOATS * 4) as u32 };
                list.IASetVertexBuffers(0, Some(&[view]));
                list.DrawInstanced(6, count, 0, 0);
                keep(false, frame_now, material_constants, sampler_index, vec![view], None, 6, count); // (PARTY-NATIVE)
                instances += count as u64;
            }
            Item::Mesh(batch) => {
                let Some(MeshState::Ready(mesh)) = meshes.get(batch.mesh as usize) else { return };
                // (FP) a one-sided material: its mirrored instances apart, with the twin that culls the other winding (the
                // order inside each part kept); a batch of one kind is drawn whole, as before
                let split: Option<(Vec<f32>, Vec<f32>)> = mirror.and_then(|_| {
                    let rows = batch.data.as_chunks::<MESH_INSTANCE_FLOATS>().0.iter();
                    let flipped = rows.clone().filter(|r| instance_mirrored(r.as_slice())).count();
                    (flipped > 0).then(|| {
                        let (mut plain, mut turned) = (Vec::new(), Vec::new());
                        for r in rows {
                            if instance_mirrored(r) { &mut turned } else { &mut plain }.extend_from_slice(r);
                        }
                        (plain, turned)
                    })
                });
                // (PARTY-NATIVE) + whether a part is the mirrored one (its G-buffer twin's state)
                let parts: Vec<(&[f32], &ID3D12PipelineState, bool)> = match (&split, mirror) {
                    (Some((plain, turned)), Some(twin)) => [(plain.as_slice(), pipeline, false), (turned.as_slice(), twin, true)].into_iter().filter(|(d, _, _)| !d.is_empty()).collect(),
                    _ => vec![(batch.data.as_slice(), pipeline, false)],
                };
                for (data, state, turned) in parts {
                    let Some(at) = put(as_bytes(data), 16) else { return };
                    let count = (data.len() / MESH_INSTANCE_FLOATS) as u32;
                    list.SetPipelineState(state);
                    list.SetGraphicsRootConstantBufferView(1, material_constants);
                    list.SetGraphicsRootDescriptorTable(2, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: gpu_start.ptr + (material_id as usize * MATERIAL_TEXTURES * gpu.srv_size) as u64 });
                    list.SetGraphicsRootShaderResourceView(5, sampler_index);
                    let views = [mesh.vertex_view, D3D12_VERTEX_BUFFER_VIEW { BufferLocation: at, SizeInBytes: (data.len() * 4) as u32, StrideInBytes: (MESH_INSTANCE_FLOATS * 4) as u32 }];
                    list.IASetVertexBuffers(0, Some(&views));
                    list.IASetIndexBuffer(Some(&mesh.index_view));
                    list.DrawIndexedInstanced(mesh.index_count, count, 0, 0, 0);
                    keep(turned, frame_now, material_constants, sampler_index, views.to_vec(), Some(mesh.index_view), mesh.index_count, count); // (PARTY-NATIVE)
                    instances += count as u64;
                }
            }
            // (VFX-2) a skeletal mesh component: its skinned vertices (skelfx.rs buffers) as one mesh instance
            Item::Skel(batch) => {
                let Some((vertex_view, index_view, count)) = skelfx::gpu_mesh(cx.device, frame_id, batch) else { return };
                let Some(at) = put(as_bytes(&batch.rows), 16) else { return };
                list.SetPipelineState(if batch.mirrored { mirror.unwrap_or(pipeline) } else { pipeline });
                list.SetGraphicsRootConstantBufferView(1, material_constants);
                list.SetGraphicsRootDescriptorTable(2, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: gpu_start.ptr + (material_id as usize * MATERIAL_TEXTURES * gpu.srv_size) as u64 });
                list.SetGraphicsRootShaderResourceView(5, sampler_index);
                let views = [vertex_view, D3D12_VERTEX_BUFFER_VIEW { BufferLocation: at, SizeInBytes: (MESH_INSTANCE_FLOATS * 4) as u32, StrideInBytes: (MESH_INSTANCE_FLOATS * 4) as u32 }];
                list.IASetVertexBuffers(0, Some(&views));
                list.IASetIndexBuffer(Some(&index_view));
                list.DrawIndexedInstanced(count, 1, 0, 0, 0);
                keep(batch.mirrored && mirror.is_some(), frame_now, material_constants, sampler_index, views.to_vec(), Some(index_view), count, 1); // (PARTY-NATIVE)
                instances += 1;
            }
            Item::Plain(batch) => {
                let (Some(vertices), Some(indices)) = (put(as_bytes(&batch.vertices), 16), put(as_bytes(&batch.indices), 16)) else { return };
                list.SetPipelineState(pipeline);
                list.SetGraphicsRootConstantBufferView(1, material_constants);
                list.SetGraphicsRootDescriptorTable(2, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: gpu_start.ptr + (material_id as usize * MATERIAL_TEXTURES * gpu.srv_size) as u64 });
                list.SetGraphicsRootShaderResourceView(5, sampler_index);
                let view = D3D12_VERTEX_BUFFER_VIEW { BufferLocation: vertices, SizeInBytes: (batch.vertices.len() * 4) as u32, StrideInBytes: (PLAIN_FLOATS * 4) as u32 };
                let index = D3D12_INDEX_BUFFER_VIEW { BufferLocation: indices, SizeInBytes: (batch.indices.len() * 4) as u32, Format: DXGI_FORMAT_R32_UINT };
                list.IASetVertexBuffers(0, Some(&[view]));
                list.IASetIndexBuffer(Some(&index));
                list.DrawIndexedInstanced(batch.indices.len() as u32, 1, 0, 0, 0);
                keep(false, frame_now, material_constants, sampler_index, vec![view], Some(index), batch.indices.len() as u32, 1); // (PARTY-NATIVE)
                instances += 1;
            }
        }
        batches += 1;
    };
    let id_of = |item: &Item| match item {
        Item::Quad(b) => b.material,
        Item::Mesh(b) => b.material,
        Item::Plain(b) => b.material,
        Item::Skel(b) => b.material, // (VFX-2)
    };
    // 1. opaque and masked: depth test and write (the targets are bound as gfx.rs left them)
    list.OMSetRenderTargets(1, Some(&cx.target_view), false, Some(&cx.depth_view));
    // (PARTY-NATIVE) a lit material with its native pair ready for this frame's G-buffer formats, in a frame with the capture:
    // its colour pass from the game's light, its G-buffer twin kept for the replay
    let gb_formats = GB_FORMATS.lock().ok().and_then(|g| *g);
    let native_stage = capture.is_some() && matches!(cx.stage, Stage::All | Stage::Early);
    let mut native_drawn = 0u32;
    for (item, m) in &opaque {
        match m.native.as_ref().filter(|_| native_stage).and_then(|n| n.gb.as_ref().filter(|g| Some(g.0) == gb_formats).map(|g| (n, g))) {
            Some((n, (_, gb, gb_turned))) => {
                native_drawn += 1;
                emit(item, m, id_of(item), &n.colour, n.colour_mirrored.as_ref(), Some((gb, gb_turned.as_ref())), &mut put);
            }
            None => emit(item, m, id_of(item), &m.pipeline, m.mirrored.as_ref().map(|t| &t.0), None, &mut put),
        }
    }
    NATIVE_DRAWS.fetch_add(native_drawn, Ordering::Relaxed);
    if !decals.is_empty() || !blended.is_empty() {
        // 2. the depth is read by the shaders from here on: read-only, and a texture
        let read_state = D3D12_RESOURCE_STATE_DEPTH_READ | D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE;
        list.ResourceBarrier(&[transition(cx.depth, D3D12_RESOURCE_STATE_DEPTH_WRITE, read_state)]);
        if copy_ready {
            if let Some((copy, ..)) = gpu.scene_copy.as_ref() {
                // The scene as it stands (the game's deferred picture, Sora, the opaque effects).
                list.ResourceBarrier(&[transition(cx.target, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_COPY_SOURCE), transition(copy, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, D3D12_RESOURCE_STATE_COPY_DEST)]);
                list.CopyResource(copy, cx.target);
                list.ResourceBarrier(&[transition(cx.target, D3D12_RESOURCE_STATE_COPY_SOURCE, D3D12_RESOURCE_STATE_RENDER_TARGET), transition(copy, D3D12_RESOURCE_STATE_COPY_DEST, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE)]);
            }
        }
        list.OMSetRenderTargets(1, Some(&cx.target_view), false, Some(&read_only_view));
        if copy_ready {
            for k in &distorted {
                let (_, item, m) = &blended[*k];
                if let Some(pipeline) = &m.distort {
                    emit(item, m, id_of(item), pipeline, m.mirrored.as_ref().and_then(|t| t.1.as_ref()), None, &mut put);
                }
            }
        }
        // (VFXR) the decals' share of the bloom when they are drawn in a stage of their own (post::decals_before / _after)
        // (VFXR decal bloom) only with `vfx.decal_bloom = 1` (DECAL_BLOOM): by default the decals do not bloom (KH3 footage)
        let decal_bloom = cx.stage == Stage::Decals && post_bloom && DECAL_BLOOM.load(std::sync::atomic::Ordering::Relaxed) && !decals.is_empty() && {
            let mut rec = post::Rec { cx, gpu, put: &mut put, tables: 0, samplers: None };
            let ok = post_kit.as_mut().is_some_and(|k| post::decals_before(&mut rec, k, &target_desc));
            list.OMSetRenderTargets(1, Some(&cx.target_view), false, Some(&read_only_view));
            list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: cx.width as f32, Height: cx.height as f32, MinDepth: 0.0, MaxDepth: 1.0 }]);
            list.RSSetScissorRects(&[windows::Win32::Foundation::RECT { left: 0, top: 0, right: cx.width as i32, bottom: cx.height as i32 }]);
            ok
        };
        // (VFXR occlusion) the characters' stencil class for the decals' test (our own list; put back to 0 afterwards)
        let decal_ref = !decals.is_empty() && decal_stencil(cx.depth_format).is_some();
        if decal_ref {
            list.OMSetStencilRef(u32::from(crate::gfx::CHAR_STENCIL & 0xC0));
        }
        for (item, m) in &decals {
            emit(item, m, id_of(item), &m.pipeline, m.mirrored.as_ref().map(|t| &t.0), None, &mut put);
        }
        if decal_ref {
            list.OMSetStencilRef(0);
        }
        if decal_bloom {
            let mut rec = post::Rec { cx, gpu, put: &mut put, tables: post::DECAL_TABLES, samplers: None };
            if let Some(k) = post_kit.as_mut() {
                post::decals_after(&mut rec, k, frame_id);
            }
            list.OMSetRenderTargets(1, Some(&cx.target_view), false, Some(&read_only_view));
            list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: cx.width as f32, Height: cx.height as f32, MinDepth: 0.0, MaxDepth: 1.0 }]);
            list.RSSetScissorRects(&[windows::Win32::Foundation::RECT { left: 0, top: 0, right: cx.width as i32, bottom: cx.height as i32 }]);
        }
        // (PARTY-NATIVE) with this frame's copy of the game's fog: each material's fog variant (its pixels fogged at their depth)
        // (PARTY-NATIVE-LIGHT) with this frame's copy of the game's light inputs: a forward-lit material's lit variant (Elden Ring's light
        // and fog at its pixels) before its fog variant
        let mut fogged = 0u32;
        let mut lit_drawn = 0u32;
        let mut plain = 0u32; // (round 3)
        for (_, item, m) in &blended {
            match (m.lit.as_ref().filter(|_| lit_ready && LIT_VARIANTS.load(Ordering::Relaxed)), m.fog.as_ref().filter(|_| fog_ready)) { // (PARTY-NATIVE) vfx.fxlit
                (Some((plain, turned)), _) => {
                    lit_drawn += 1;
                    emit(item, m, id_of(item), plain, turned.as_ref(), None, &mut put);
                }
                (None, Some((plain, turned))) => {
                    fogged += 1;
                    emit(item, m, id_of(item), plain, turned.as_ref(), None, &mut put);
                }
                (None, None) => {
                    plain += 1;
                    emit(item, m, id_of(item), &m.pipeline, m.mirrored.as_ref().map(|t| &t.0), None, &mut put);
                }
            }
        }
        FOG_BATCHES.fetch_add(fogged, Ordering::Relaxed);
        LIT_BATCHES.fetch_add(lit_drawn, Ordering::Relaxed);
        PLAIN_BATCHES.fetch_add(plain, Ordering::Relaxed);
        // 3. as gfx.rs had it
        list.ResourceBarrier(&[transition(cx.depth, read_state, D3D12_RESOURCE_STATE_DEPTH_WRITE)]);
        list.OMSetRenderTargets(1, Some(&cx.target_view), false, Some(&cx.depth_view));
    }
    drop(emit);
    // (PARTY-NATIVE) this slot's G-buffer draws for gfx.rs's G-buffer pass (recorded after this colour pass, executed before it)
    if matches!(cx.stage, Stage::All | Stage::Early) {
        if let Ok(mut r) = REPLAYS.lock() {
            r.retain(|(s, _)| *s != cx.slot);
            if !replays.is_empty() {
                r.push((cx.slot, replays));
            }
        }
    }
    // (VFXR) 4. the post passes of the late stage: KH3's bloom of what the effects added, the screen passes they ask for
    if late_stage && (post_bloom || post_screen) {
        if let Some(k) = post_kit.as_mut() {
            let pre = gpu.scene_copy.as_ref().filter(|_| copy_ready && !blended.is_empty()).map(|(c, ..)| c.clone());
            let mut rec = post::Rec { cx, gpu, put: &mut put, tables: 0, samplers: None };
            post::after_effects(&mut rec, k, &target_desc, pre.as_ref(), frame_id, &prepared.screen);
            list.SetGraphicsRootConstantBufferView(0, frame_constants);
            list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: cx.width as f32, Height: cx.height as f32, MinDepth: 0.0, MaxDepth: 1.0 }]);
            list.RSSetScissorRects(&[windows::Win32::Foundation::RECT { left: 0, top: 0, right: cx.width as i32, bottom: cx.height as i32 }]);
            list.OMSetRenderTargets(1, Some(&cx.target_view), false, Some(&cx.depth_view));
        }
    }
    if let Ok(mut c) = CURSORS.lock() {
        if let Some(at) = c.get_mut(cx.slot) {
            *at = cursor;
        }
    }
    OVERFLOW.fetch_add(overflow, Ordering::Relaxed);
    DRAWN_FRAMES.fetch_add(1, Ordering::Relaxed);
    DRAWN_BATCHES.fetch_add(batches, Ordering::Relaxed);
    DRAWN_INSTANCES.fetch_add(instances, Ordering::Relaxed);
    let micros = started.elapsed().as_micros() as u64;
    DRAW_MICROS.fetch_add(micros, Ordering::Relaxed);
    DRAW_MAX.fetch_max(micros, Ordering::Relaxed);
}

/// (PARTY-NATIVE) gfx.rs at the start of a frame's recording (before the decals' stage): no capture yet, and nothing of this
/// slot's earlier frame is replayed.
pub(crate) fn begin_frame(slot: usize) {
    set_capture(None);
    if let Ok(mut r) = REPLAYS.lock() {
        r.retain(|(s, _)| *s != slot);
    }
}

/// (PARTY-NATIVE) gfx.rs before the capture: some opaque lit effect of the latest draw list has its native pair ready for the
/// G-buffer's formats (the capture then covers the picture, so that their texels are captured with the characters').
pub(crate) fn native_wanted() -> bool {
    let Some(prepared) = LATEST.lock().ok().and_then(|g| g.clone()) else { return false };
    let Ok(guard) = GPU.try_lock() else { return false };
    let Some(gpu) = guard.as_ref() else { return false };
    let formats = GB_FORMATS.lock().ok().and_then(|g| *g);
    let ready = |id: u32| matches!(gpu.materials.get(id as usize), Some(MatState::Ready(m)) if m.pass == Pass::Opaque && m.native.as_ref().is_some_and(|n| n.gb.as_ref().is_some_and(|g| Some(g.0) == formats)));
    prepared.quads.iter().map(|b| b.material).chain(prepared.meshes.iter().map(|b| b.material)).chain(prepared.plain.iter().map(|b| b.material)).chain(prepared.skel.iter().map(|b| b.material)).any(ready)
}

/// (PARTY-NATIVE) gfx.rs's G-buffer pass: this slot has lit effects' G-buffer draws to replay.
pub(crate) fn replay_pending(slot: usize) -> bool {
    REPLAYS.lock().is_ok_and(|r| r.iter().any(|(s, v)| *s == slot && !v.is_empty()))
}

/// (PARTY-NATIVE) What `gbuffer_replay` draws into.
pub(crate) struct GbReplayCx<'a> {
    pub list: &'a ID3D12GraphicsCommandList,
    pub slot: usize,
    /// the first of the three G-buffer targets' views (consecutive), the scene depth's writable view
    pub targets: D3D12_CPU_DESCRIPTOR_HANDLE,
    pub depth: D3D12_CPU_DESCRIPTOR_HANDLE,
    pub width: u32,
    pub height: u32,
}

/// (PARTY-NATIVE) THE LIT EFFECTS' G-BUFFER TWINS, in gfx.rs's G-buffer pass (the early list, after the characters' writes and
/// before the game's light passes): every draw `draw` kept for this slot, with its G-buffer state, the same constants and
/// geometry, the characters' stencil class. Leaves this module's root signature and heaps bound (the caller binds its own
/// again). Returns the draws.
pub(crate) unsafe fn gbuffer_replay(cx: &GbReplayCx) -> u32 {
    let replays = match REPLAYS.lock() {
        Ok(mut r) => match r.iter().position(|(s, _)| *s == cx.slot) {
            Some(k) => r.swap_remove(k).1,
            None => return 0,
        },
        Err(_) => return 0,
    };
    let Ok(guard) = GPU.try_lock() else { return 0 };
    let Some(gpu) = guard.as_ref() else { return 0 };
    let list = cx.list;
    let gpu_start = gpu.srv_heap.GetGPUDescriptorHandleForHeapStart();
    let frame_table = MAX_MATERIALS * MATERIAL_TEXTURES + cx.slot * FRAME_VIEWS;
    list.SetGraphicsRootSignature(&gpu.root);
    list.SetDescriptorHeaps(&[Some(gpu.srv_heap.clone()), Some(gpu.sampler_heap.clone())]);
    list.SetGraphicsRootDescriptorTable(3, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: gpu_start.ptr + (frame_table * gpu.srv_size) as u64 });
    list.SetGraphicsRootDescriptorTable(4, gpu.sampler_heap.GetGPUDescriptorHandleForHeapStart());
    list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
    list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: cx.width as f32, Height: cx.height as f32, MinDepth: 0.0, MaxDepth: 1.0 }]);
    list.RSSetScissorRects(&[windows::Win32::Foundation::RECT { left: 0, top: 0, right: cx.width as i32, bottom: cx.height as i32 }]);
    list.OMSetRenderTargets(3, Some(&cx.targets), true, Some(&cx.depth));
    list.OMSetStencilRef(u32::from(crate::gfx::CHAR_STENCIL));
    let mut n = 0u32;
    for r in &replays {
        list.SetPipelineState(&r.pso);
        list.SetGraphicsRootConstantBufferView(0, r.frame);
        list.SetGraphicsRootConstantBufferView(1, r.material);
        list.SetGraphicsRootDescriptorTable(2, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: r.table });
        list.SetGraphicsRootShaderResourceView(5, r.samplers);
        list.IASetVertexBuffers(0, Some(&r.vertex));
        match r.index {
            Some(index) => {
                list.IASetIndexBuffer(Some(&index));
                list.DrawIndexedInstanced(r.count, r.instances, 0, 0, 0);
            }
            None => list.DrawInstanced(r.count, r.instances, 0, 0),
        }
        n += 1;
    }
    list.OMSetStencilRef(0);
    GB_DRAWS.fetch_add(n, Ordering::Relaxed);
    n
}

/// What KH3's scene-linear 1.0 of an effect becomes in the host's scene target.
///
/// THE RULE (IMPL_ER_FX_STATE.md): effects are EMISSIVE: their brightness on screen should be
/// what it is in KH3 whatever light the host's scene is in, so the factor follows the host's
/// EXPOSURE and not the light at his feet: KH3's reference white (what a white surface shows
/// under the test level's rig: `light::reference_white()` = 3.27) is put where the host's tone
/// mapper gets 1.0, i.e. `1 / (reference white x SC_SceneExposure)`: 0.034 at exposure 9 (day),
/// 0.016 at 18.9 (night by a fire). Without the game's exposure (the light path is off): the
/// character path's own scale (`fallback`).
pub fn effect_scale(exposure: Option<f32>, fallback: f32) -> f32 {
    match exposure.filter(|e| e.is_finite() && *e > 1.0e-3) {
        Some(e) => 1.0 / (crate::light::reference_white() * e),
        None => fallback,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (PARTY-NATIVE) the lit effects' native route: the pair's names (never for a decal, a shader without G-buffer outputs or
    /// an overlay's changed translation), the frame row, and the harness's frame layout the row lands in
    #[test]
    fn party_native_lit_effects_take_the_native_pair_and_row() {
        assert_eq!(native_files("m", 65, 4, ""), Some(("nm65.ps.dxil".to_string(), "gm65.ps.dxil".to_string())));
        assert_eq!(native_files("s", 15, 5, ""), Some(("ns15.ps.dxil".to_string(), "gs15.ps.dxil".to_string())));
        assert_eq!(native_files("d", 7, 4, ""), None, "a decal");
        assert_eq!(native_files("m", 3, 1, ""), None, "an unlit (forward) shader");
        assert_eq!(native_files("m", 319, 4, "_vxl"), Some(("nm319_vxl.ps.dxil".to_string(), "gm319_vxl.ps.dxil".to_string())), "an overlay's new translation: its own pair");
        assert_eq!(fxn_row(None), [0.0; FXN_FLOATS]);
        let c = FxCapture { light: 1, geo: 2, mark: 1.0 / 255.0, cos: 3.0 };
        assert_eq!(fxn_row(Some(c)), [1.0, 1.0 / 255.0, 1.0, 0.0]);
        // the harness: the native row follows the forward rows, behind the 48 floats of the frame's head
        let wgsl = include_str!("../shaders/er_vfx.wgsl");
        let head = &wgsl[wgsl.find("struct ErFrame {").unwrap()..];
        let head = &head[..head.find("\n}").unwrap()];
        assert!(head.find("fwd_l: array<vec4<f32>, 64>,").unwrap() < head.find("fxn: vec4<f32>,").unwrap(), "fxn after the forward rows");
        assert_eq!(crate::fwdlight::FRAME_FLOATS, (8 + 5 + 64) * 4, "the forward rows' size the row is written behind");
        // the colour pass keeps the earlier shading where the texel is not its own; the G-buffer twin writes the KH white
        assert!(wgsl.contains("if (fx.ok > 0.5) {") && wgsl.contains("g.albedo = vec4<f32>(1.0, 1.0, 1.0, er.fxn.y);"));
        assert!(wgsl.contains("@group(1) @binding(14) var er_hy_light") && wgsl.contains("@group(1) @binding(15) var er_hy_geo"));
    }

    /// (PARTY-NATIVE) the game's fog on the late effects: each pass's mode, the volume sizes taken, and the harness's layout (the
    /// fog row after the native row, the material's fog row after `er`, t16 / t17, the composite's function applied at the end)
    #[test]
    fn party_native_late_effects_take_the_games_fog() {
        assert_eq!([Pass::Translucent, Pass::Additive, Pass::Modulate, Pass::Opaque, Pass::DecalAdd, Pass::DecalBlend].map(fog_mode), [0.0, 1.0, 2.0, 3.0, 3.0, 3.0]);
        assert!(fog_size_ok([160, 90, 64]) && !fog_size_ok([0, 90, 64]) && !fog_size_ok([160, 90, 4096]));
        assert_eq!(PARAM_ROWS, 67, "64 material rows, misc, er, fx");
        let wgsl = include_str!("../shaders/er_vfx.wgsl");
        let frame = &wgsl[wgsl.find("struct ErFrame {").unwrap()..];
        let frame = &frame[..frame.find("\n}").unwrap()];
        assert!(frame.find("fxn: vec4<f32>,").unwrap() < frame.find("fxf: vec4<f32>,").unwrap());
        let params = &wgsl[wgsl.find("struct KhParams {").unwrap()..];
        let params = &params[..params.find("\n}").unwrap()];
        assert!(params.find("er: vec4<f32>,").unwrap() < params.find("fx: vec4<f32>,").unwrap());
        assert!(wgsl.contains("@group(1) @binding(16) var er_fog_vol: texture_3d<f32>;") && wgsl.contains("@group(1) @binding(17) var er_fog_rows: texture_2d<u32>;"));
        assert!(wgsl.contains("scene = scene * fog.transmit + fog.inscatter;"));
        assert_eq!(FRAME_VIEWS, 18, "(PARTY-NATIVE-LIGHT) + t18..t29");
    }

    /// (XW) the overlay replaces exactly the records it names and adds its shaders; without its shaders file nothing changes
    #[test]
    fn the_xw_overlay_lays_its_records_over_the_table() {
        let mut json = serde_json::json!({ "owners": { "a": { "decal": { "ok": false } }, "b": { "x": 1 } }, "materials": { "m": { "owner": "a" } } });
        let mut built = serde_json::json!({ "shaders": [{ "file": "s1.ps.dxil" }] });
        let over = serde_json::json!({ "owners": { "a": { "decal": { "ok": true } } } });
        let over_built = serde_json::json!({ "shaders": [{ "file": "d1.ps.dxil" }] });
        let (j0, b0) = (json.clone(), built.clone());
        assert_eq!(overlay_xw(&mut json, &mut built, Some(over.clone()), None), 0);
        assert_eq!((&json, &built), (&j0, &b0));
        assert_eq!(overlay_xw(&mut json, &mut built, Some(over), Some(over_built)), 1);
        assert_eq!(json["owners"]["a"]["decal"]["ok"], true);
        assert_eq!((json["owners"]["b"].clone(), json["materials"].clone()), (j0["owners"]["b"].clone(), j0["materials"].clone()));
        assert_eq!(built["shaders"].as_array().unwrap().len(), 2);
        // (XW-b) a pixel_suffix map is taken in
        let over = serde_json::json!({ "pixel_suffix": { "d9": "_xwb" }, "owners": {} });
        assert_eq!(overlay_xw(&mut json, &mut built, Some(over), Some(serde_json::json!({ "shaders": [{ "file": "d9_xwb.ps.dxil" }] }))), 1);
        assert_eq!(json["pixel_suffix"]["d9"], "_xwb");
    }

    /// (PARTY-NATIVE) a root's _fwl overlay: inert without its two files; with them its owners, records and `<letter><id>_fwl`
    /// shaders are laid over the table (the fog / lit variants are then `f` / `l` + that pixel file); the harness gives the
    /// translated ILC rows Elden Ring's SH in a lit variant and 0 elsewhere
    #[test]
    fn party_native_the_fwl_overlay_is_inert_without_its_files() {
        let dir = std::env::temp_dir().join(format!("kh_fwl_overlay_test_{}", std::process::id()));
        std::fs::create_dir_all(dir.join("shaders")).unwrap();
        std::fs::create_dir_all(dir.join("er")).unwrap();
        let mut json = serde_json::json!({ "owners": { "hmt_wat_lit_00a": { "id": 32, "sprite": { "ok": true } } }, "materials": { "hmi_wat": { "owner": "hmt_wat_lit_00a", "cb": [0] } } });
        let mut built = serde_json::json!({ "shaders": [{ "name": "s32", "file": "s32.ps.dxil", "ok": true }] });
        let (j0, b0) = (json.clone(), built.clone());
        assert_eq!(overlay_fwl(&mut json, &mut built, &dir), 0);
        assert_eq!((&json, &built), (&j0, &b0), "no files: nothing changes");
        std::fs::write(dir.join("shaders").join("shading_fwl.json"), r#"{ "pixel_suffix": { "s32": "_fwl" }, "owners": { "hmt_wat_lit_00a": { "id": 32, "sprite": { "ok": true, "ilc": true } } }, "materials": { "hmi_wat": { "owner": "hmt_wat_lit_00a", "cb": [1] } } }"#).unwrap();
        assert_eq!(overlay_fwl(&mut json, &mut built, &dir), 0, "the records without the shaders file: nothing changes");
        assert_eq!((&json, &built), (&j0, &b0));
        std::fs::write(dir.join("er").join("shaders_fwl.json"), r#"{ "shaders": [{ "name": "s32_fwl", "file": "s32_fwl.ps.dxil", "ok": true }] }"#).unwrap();
        assert_eq!(overlay_fwl(&mut json, &mut built, &dir), 3);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(json["pixel_suffix"]["s32"], "_fwl");
        assert_eq!(json["owners"]["hmt_wat_lit_00a"]["sprite"]["ilc"], true);
        assert_eq!(json["materials"]["hmi_wat"]["cb"][0], 1);
        assert_eq!(built["shaders"].as_array().unwrap().len(), 2);
        let pixel_file = format!("s32{}.ps.dxil", json["pixel_suffix"]["s32"].as_str().unwrap());
        assert_eq!((format!("f{pixel_file}"), format!("l{pixel_file}")), ("fs32_fwl.ps.dxil".to_string(), "ls32_fwl.ps.dxil".to_string()));
        let wgsl = include_str!("../shaders/er_vfx.wgsl");
        let at = wgsl.find("fn kh_fwd_ilc(k: i32)").unwrap();
        let lit = wgsl[..at].rfind("#ifdef KH_FXLIT").unwrap();
        assert!(wgsl[lit..at].lines().all(|l| l.trim() != "#endif"), "the SH version is the lit variant's");
        assert!(wgsl[at..].contains("let s = kh_lit_sh[k - 8];") && wgsl[at..].contains("s.w / 0.886228"));
        assert_eq!(wgsl.matches("fn kh_fwd_ilc(k: i32) -> vec4<f32> {").count(), 2, "and a 0 version for the other variants");
    }

    /// (VFX-XLATE) the harness carries what the translator's --root / --vxl tables call (the scene-colour read outside the
    /// fragment-only block, so the vertex variants that splice the same owners compile; the scene particle colours), and the
    /// filter's edge reads the scene depth only when the pass says the depth is readable
    #[test]
    fn vfx_xlate_the_harness_has_the_scene_colour_read_and_the_particle_colours() {
        let shader = include_str!("../shaders/er_vfx.wgsl");
        let at = shader.find("fn kh_scene_color(").expect("kh_scene_color");
        let generated = shader.find("//KH_GENERATED").expect("the splice marker");
        assert!(at < generated, "defined before the translated shaders");
        assert!(!shader[..at].trim_end().ends_with("#ifdef ER_FRAGMENT"), "not fragment-only");
        for field in ["pbase", "pbright", "pdark", "psynth"] {
            assert!(shader.contains(&format!("i.{field} = ")), "{field} is set");
        }
        assert!(shader.contains("kh.m[3].w > 0.5"), "the edge needs the readable-depth flag");
    }

    #[test]
    fn fp_a_mirrored_instance_is_told_by_its_rows_and_culls_the_other_winding() {
        // rows as submit writes them: [x_k, y_k, z_k, origin_k] for k = 0..2
        let rows = |m: Mat3| -> Vec<f32> { (0..3).flat_map(|k| [m.x_axis[k], m.y_axis[k], m.z_axis[k], 7.0]).collect() };
        let turn = Mat3::from_rotation_z(0.7) * Mat3::from_diagonal(Vec3::new(2.0, 0.5, 3.0));
        assert!(!instance_mirrored(&rows(turn)));
        assert!(instance_mirrored(&rows(turn * Mat3::from_diagonal(Vec3::new(-1.0, 1.0, 1.0)))));
        assert!(instance_mirrored(&rows(Mat3::from_diagonal(Vec3::new(1.0, 1.0, -0.2)))));
        assert!(!instance_mirrored(&rows(Mat3::from_diagonal(Vec3::new(-1.0, -1.0, 1.0)))));
        assert_eq!(one_sided_cull(false), D3D12_CULL_MODE_FRONT);
        assert_eq!(one_sided_cull(true), D3D12_CULL_MODE_BACK);
    }

    #[test]
    fn the_effect_scale_follows_the_exposure_and_falls_back_without_one() {
        let white = crate::light::reference_white();
        assert!((effect_scale(Some(9.0), 1.0) - 1.0 / (white * 9.0)).abs() < 1.0e-7);
        assert!(effect_scale(Some(18.9), 1.0) < effect_scale(Some(9.0), 1.0));
        assert_eq!((effect_scale(None, 0.25), effect_scale(Some(0.0), 0.5), effect_scale(Some(f32::NAN), 2.0)), (0.25, 0.5, 2.0));
    }

    /// (ORBS) the orbs' gain is prize.glow over the eye factor (never twice), and their lit balls have their own shading
    /// branch in the effect shader (kh-shaderc defines KH_ORB for the omt_prz_* owners)
    #[test]
    fn the_prize_orbs_get_their_gain_once_and_their_own_lit_shading() {
        assert!((prize_gain(3.27) - 3.27 / eye_boost()).abs() < 1.0e-6);
        let shader = include_str!("../shaders/er_vfx.wgsl");
        let orb = shader.find("fn kh_orb_shade(").expect("kh_orb_shade");
        assert!(shader[..orb].rfind("#ifdef KH_ORB").is_some() && shader.contains("color = vec4<f32>(kh_orb_shade(o, normal, light, normalize(-i.twp), er.lit.xyz), 1.0);"));
    }

    /// (ZFIN round 8) the tests that go through the global frame (`submit` -> LATEST, REGISTRY) take turns
    static FRAME_TESTS: Mutex<()> = Mutex::new(());

    /// (ZFIN round 8, the user: "the summoning pentagram should not flicker") Zettaflare's circle is several decals around
    /// one centre (fills, lines, the rune parts of its sub-effects): they are drawn in the draw list's order whatever the
    /// camera, so a moving / shaking camera can never swap a fill over the lines (kh-app's Bevy sort did: the strobe).
    #[test]
    fn the_decals_keep_the_draw_lists_order_from_any_camera() {
        let _turn = FRAME_TESTS.lock().unwrap_or_else(|e| e.into_inner());
        set_options(VfxOptions::default());
        let mut list = kh_vfx::DrawList::default();
        // fills (big boxes) and lines / runes (smaller, a few mm off the centre), alternately: list order = their index
        for k in 0..6 {
            let size = if k % 2 == 0 { 480.0 } else { 150.0 };
            let at = Vec3::new(0.3 * k as f32, -0.2 * k as f32, 0.1 * (k % 3) as f32);
            list.decals.push(kh_vfx::Decal { material: (k % 3) as u32, transform: glam::Affine3A::from_scale_rotation_translation(Vec3::new(150.0, size, size), glam::Quat::IDENTITY, at), color: [1.0; 4], custom: [0.0; 3], z_fade: [1.0, 0.0], time: 0.0 });
        }
        let names = ["fill".to_string(), "lines".to_string(), "runes".to_string()];
        let want: Vec<Vec3> = list.decals.iter().map(|d| Vec3::from(d.transform.translation)).collect();
        let order = |camera: &kh_vfx::Camera| {
            submit(&list, &names, &[], camera, 1.0);
            let prepared = LATEST.lock().unwrap().clone().expect("a frame");
            prepared.plain.iter().filter(|b| b.decal).map(|b| b.center).collect::<Vec<Vec3>>()
        };
        for (k, eye) in [Vec3::new(-400.0, 0.0, 150.0), Vec3::new(400.0, 30.0, 120.0), Vec3::new(0.5, -0.4, 140.0), Vec3::new(0.0, 380.0, 60.0), Vec3::new(-0.2, 0.7, 20.0)].into_iter().enumerate() {
            let forward = (Vec3::ZERO - eye).normalize();
            let right = forward.cross(Vec3::Z).normalize_or(Vec3::Y);
            let camera = kh_vfx::Camera { position: eye, forward, right, up: right.cross(forward) };
            assert_eq!(order(&camera), want, "camera {k} at {eye}: the decals in the draw list's order");
        }
        clear();
    }

    /// (RSF round 20) the translator flags a record whose uniform expression it could not evaluate; the overlay's records replace
    /// them (Stitch's star omi_crs_3: a flat white square while it stayed flagged)
    #[test]
    fn rsf21_untranslated_owners_are_listed() {
        let json = serde_json::json!({ "owners": {
            "a": { "sprite": { "ok": true }, "mesh": { "ok": false, "why": "input PARTICLE_BRIGHTCOLOR0  (add r6.xyz, v7.xyzx, -v8.xyzx)" } },
            "b": { "sprite": { "ok": false, "why": "no FParticleSpriteVertexFactory shaders" }, "mesh": { "ok": true } },
        } });
        assert_eq!(untranslated_owners(&json), vec!["a.mesh (input PARTICLE_BRIGHTCOLOR0)".to_string()]);
    }

    #[test]
    fn rsf20_flagged_materials_are_listed() {
        let json = serde_json::json!({ "materials": { "omi_crs_3": { "bad_uniform_expression": true }, "zmi_ok": { "cb": [1.0] }, "a_bad": { "bad_uniform_expression": true } } });
        assert_eq!(bad_expression_materials(&json), vec!["a_bad".to_string(), "omi_crs_3".to_string()]);
    }

    #[test]
    fn a_draw_list_becomes_instances_and_plain_vertices() {
        let _turn = FRAME_TESTS.lock().unwrap_or_else(|e| e.into_inner()); // (ZFIN round 8)
        set_options(VfxOptions::default());
        let mut list = kh_vfx::DrawList::default();
        let quad = kh_vfx::Quad { corners: [Vec3::new(0.0, -1.0, 1.0), Vec3::new(0.0, 1.0, 1.0), Vec3::new(0.0, 1.0, -1.0), Vec3::new(0.0, -1.0, -1.0)], uv: [0.0, 0.0, 1.0, 1.0], uv2: [0.0; 4], blend: 0.25, color: [2.0, -1.0, 0.5, 3.0], material: 0, dynamic: [1.0, 2.0, 3.0, 4.0], time: 0.5, rel: 0.75, center: Vec3::new(10.0, 20.0, 30.0), size: 2.0, mode: kh_vfx::Blend::Material };
        list.quads.push(quad);
        list.quads.push(kh_vfx::Quad { center: Vec3::new(30.0, 20.0, 30.0), ..quad });
        list.quads.push(kh_vfx::Quad { mode: kh_vfx::Blend::Additive, ..quad });
        list.meshes.push(kh_vfx::MeshInstance { mesh: 0, material: 1, transform: glam::Affine3A::from_translation(Vec3::new(1.0, 2.0, 3.0)), color: [1.0; 4], dynamic: [0.0; 4], time: 1.0, rel: 0.5, blend: kh_vfx::Blend::Material, uv_mode: 5.0, sub: 0.0 /* (ZA) */ });
        list.ribbons.push(kh_vfx::Ribbon { points: vec![kh_vfx::RibbonPoint { a: Vec3::ZERO, b: Vec3::Z, u: 0.0, color: [1.0; 4] }, kh_vfx::RibbonPoint { a: Vec3::X * 4.0, b: Vec3::X * 4.0 + Vec3::Z, u: 1.0, color: [1.0; 4] }], material: 0, dynamic: [0.0; 4], time: 0.0, blend: kh_vfx::Blend::Material, tiles: 1.0, v_scale: 1.0, uv_turned: false });
        list.decals.push(kh_vfx::Decal { material: 1, transform: glam::Affine3A::from_scale(Vec3::splat(50.0)), color: [1.0; 4], custom: [0.0; 3], z_fade: [1.0, 0.0], time: 0.0 });
        submit(&list, &["mat_a".to_string(), "mat_b".to_string()], &["mesh_a".to_string()], &kh_vfx::Camera::DEFAULT, 1.5);
        let prepared = LATEST.lock().unwrap().clone().expect("a frame");
        // Two quad batches (the emitter's blend splits them), the first with two instances.
        assert_eq!(prepared.quads.len(), 2);
        assert_eq!(prepared.quads[0].data.len(), 2 * QUAD_FLOATS);
        let first = &prepared.quads[0].data;
        assert_eq!(&first[0..4], &[0.0, -1.0, 1.0, 0.5]);
        assert_eq!((first[7], first[11], first[15]), (0.75, 0.25, 2.0));
        // (the colour: negative channels cut, alpha kept)
        assert_eq!(&first[24..28], &[2.0, 0.0, 0.5, 3.0]);
        assert_eq!(prepared.quads[0].center, Vec3::new(20.0, 20.0, 30.0));
        assert_eq!(prepared.meshes.len(), 1);
        let rows = &prepared.meshes[0].data;
        assert_eq!((rows.len(), rows[3], rows[7], rows[11], rows[22]), (MESH_INSTANCE_FLOATS, 1.0, 2.0, 3.0, 5.0));
        // A ribbon of two points: one segment = two rows of two vertices, two triangles; the decal's box.
        let ribbon = prepared.plain.iter().find(|b| !b.decal).expect("the ribbon");
        assert_eq!((ribbon.vertices.len() / PLAIN_FLOATS, ribbon.indices.len()), (4, 6));
        let decal = prepared.plain.iter().find(|b| b.decal).expect("the decal");
        assert_eq!((decal.vertices.len() / PLAIN_FLOATS, decal.indices.len()), (8, 36));
        // The registry named what it needs: mat_a on quads (two blends) and on a ribbon, mat_b on a mesh and as a decal.
        let registry = REGISTRY.lock().unwrap();
        let registry = registry.as_ref().unwrap();
        assert!(registry.index.contains_key(&MatKey { name: "mat_a".into(), geo: Geo::Quad, blend: kh_vfx::Blend::Additive }) && registry.index.contains_key(&MatKey { name: "mat_b".into(), geo: Geo::Decal, blend: kh_vfx::Blend::Material }) && registry.mesh_index.contains_key("mesh_a"));
        clear();
    }
}

// ------------------------------------------------------------------------------------ (VFXR) character drives

/// (VFXR) One character-material drive of the frame (`SQEX_Material_InLight / _Aura / _Scalar / _Vector`, resolved by the
/// host: kh_host::vfx::EffectHost::char_drives): `owner` 0 = Sora, 1 Donald, 2 Goofy.
#[derive(Clone, Debug, PartialEq)]
pub struct CharDrive {
    pub owner: u8,
    pub name: String,
    pub value: [f32; 4],
    pub vector: bool,
    /// (AUDIT-B W-VFX) the form weapon / pawn model the effect hangs on (its `kb:<model>` slots of Sora's library), None: the
    /// owner's body. KH3's SQEX_Material_Scalar / _Vector (0x144378F10 / 0x144379F10) write the mesh the component is
    /// attached to (0x1442871A0's last case): Clock Drill's vw_so114_fns_mnk00 greys the Clock and the drill (to_gray_scale).
    pub mesh: Option<String>,
    /// (FREEZE-FX) Every slot of the owner's library, his held weapons' too (KH3's SetFrozen sends the freeze rows to the pawn's
    /// equipment: kh-host `CharDrive::equipment`).
    pub equipment: bool,
}

static CHAR_DRIVES: Mutex<Vec<CharDrive>> = Mutex::new(Vec::new());
/// What every slot of a library held of one parameter: (scalar, vector).
type HeldValues = Vec<HeldValue>;
/// (MIMIC-BOSS drives fix) One slot's held (scalar, vector) of one parameter.
type HeldValue = (Option<f32>, Option<[f32; 4]>);
/// (owner, parameter, mesh) -> the values every slot had before the drive (given back when it ends)
/// (AUDIT-B W-VFX) keyed by the mesh too: a drive of a form weapon's slots is held apart from the body's
type HeldKey = (u8, String, String);
static CHAR_HELD: Mutex<Vec<(HeldKey, HeldValues)>> = Mutex::new(Vec::new());
/// `vfx.char_fx` (default on): the drives reach the characters.
pub static CHAR_FX: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// (VFXR) The host hands the frame's drives over (every frame; an empty list ends them).
pub fn set_char_drives(drives: Vec<CharDrive>) {
    if let Ok(mut d) = CHAR_DRIVES.lock() {
        *d = drives;
    }
}

/// (VFXR) Puts `owner`'s drives on its material library (Sora: the body "sora"; a friend: every mesh of his), and gives the
/// parameters a drive no longer holds back to what the slots had. The materials translated from KH3 read them (their
/// uniform expressions: InLight, InLightColor, InLightCenterPosition ..., Aura, OpacityFade).
pub(crate) fn apply_char_drives(lib: &mut kh_look::library::Library, owner: u8) {
    let drives: Vec<CharDrive> = if CHAR_FX.load(Ordering::Relaxed) { CHAR_DRIVES.lock().map(|d| d.iter().filter(|x| x.owner == owner).cloned().collect()).unwrap_or_default() } else { Vec::new() };
    // (AUDIT-B W-VFX) the slots a drive reaches: the body ("sora"; a friend: every mesh), or the form weapon it hangs on
    let mesh_of = |d: &CharDrive| -> String { if owner == 0 && !d.equipment { sora_mesh_of(d.mesh.as_deref()) } else { String::new() } };
    let Ok(mut held) = CHAR_HELD.lock() else { return };
    // the ended ones back
    held.retain(|((o, name, mesh), before)| {
        if *o != owner || drives.iter().any(|d| &d.name == name && &mesh_of(d) == mesh) {
            return true;
        }
        for (slot, (s, v)) in lib.slots.iter_mut().filter(|s| mesh.is_empty() || &s.mesh == mesh).zip(before.iter()) {
            match s {
                Some(x) => drop(slot.scalars.insert(name.clone(), *x)),
                None => drop(slot.scalars.remove(name)),
            }
            match v {
                Some(x) => drop(slot.vectors.insert(name.clone(), *x)),
                None => drop(slot.vectors.remove(name)),
            }
        }
        false
    });
    for d in &drives {
        let mesh = mesh_of(d);
        if !held.iter().any(|((o, n, m), _)| *o == owner && n == &d.name && m == &mesh) {
            let before = lib.slots.iter().filter(|s| mesh.is_empty() || s.mesh == mesh).map(|s| (s.scalars.get(&d.name).copied(), s.vectors.get(&d.name).copied())).collect();
            held.push(((owner, d.name.clone(), mesh.clone()), before));
        }
        for slot in lib.slots.iter_mut().filter(|s| mesh.is_empty() || s.mesh == mesh) {
            if d.vector {
                slot.vectors.insert(d.name.clone(), d.value);
            } else {
                slot.scalars.insert(d.name.clone(), d.value[0]);
            }
        }
    }
}

/// (AUDIT-B W-VFX) The slots of Sora's library a drive on Sora (or a copy of Sora) reaches: the body "sora", or the form
/// weapon it hangs on.
fn sora_mesh_of(d: Option<&str>) -> String {
    match d {
        // (HAIR-PULSE) the Kingdom Key is drawn from the main pack: its slots are "kingdom_key" (never `kb:w_so010`)
        Some(crate::keyblade::KINGDOM_KEY) => "kingdom_key".to_string(),
        Some(m) => format!("{}{m}", crate::keyblade::PREFIX),
        None => "sora".to_string(),
    }
}

/// (MIMIC-BOSS drives fix) A slot of Sora's library as a COPY of Sora draws it (copy member m: `owner` = m + 1, kh-host
/// `char_drives`): the player's (owner 0) drives taken back to what the slot held before them, then the copy's own drives
/// put on (the same body / form-weapon mesh rule). His form glow, in-light and aura are HIS, never the player's. None = the
/// slot as it is (neither touches it).
pub(crate) fn copy_drive_slot(lib: &kh_look::library::Library, index: usize, owner: u8) -> Option<kh_look::library::Slot> {
    let slot = lib.slots.get(index)?;
    let drives: Vec<CharDrive> = if owner != 0 && CHAR_FX.load(Ordering::Relaxed) { CHAR_DRIVES.lock().map(|d| d.iter().filter(|x| x.owner == owner && (x.equipment || sora_mesh_of(x.mesh.as_deref()) == slot.mesh)).cloned().collect()).unwrap_or_default() } else { Vec::new() };
    // the player's held values of this slot: its place among the slots of that mesh (CHAR_HELD's order)
    let undo: Vec<(String, HeldValue)> = CHAR_HELD
        .lock()
        .map(|held| {
            held.iter()
                .filter(|((o, _, mesh), _)| *o == 0 && (mesh.is_empty() || *mesh == slot.mesh))
                .filter_map(|((_, name, mesh), before)| {
                    let at = lib.slots[..index].iter().filter(|s| mesh.is_empty() || s.mesh == *mesh).count();
                    before.get(at).map(|b| (name.clone(), *b))
                })
                .collect()
        })
        .unwrap_or_default();
    if drives.is_empty() && undo.is_empty() {
        return None;
    }
    let mut out = slot.clone();
    for (name, (s, v)) in undo {
        match s {
            Some(x) => drop(out.scalars.insert(name.clone(), x)),
            None => drop(out.scalars.remove(&name)),
        }
        match v {
            Some(x) => drop(out.vectors.insert(name, x)),
            None => drop(out.vectors.remove(&name)),
        }
    }
    for d in drives {
        if d.vector {
            out.vectors.insert(d.name, d.value);
        } else {
            out.scalars.insert(d.name, d.value[0]);
        }
    }
    Some(out)
}

/// (HAIR-PULSE) A weapon slot's in-light centres for its draw: the host hands a drive on a weapon its centre in the world (w 1,
/// kh-host `char_drives`); the weapon materials measure it from their own object position (`p8` = `kh.obj`, the drawn mesh's
/// bounds origin: w_so200_mat0_Inst's KH_C554 197-230, `distance(p8 + centre - view x CameraOffset, pixel)`), so it becomes
/// `world - object.obj` with w 0 (the camera offset is applied by kh_vfx when it samples). Some(copy) when the slot has one.
pub(crate) fn weapon_in_light(slot: &kh_look::library::Slot, object: &kh_look::library::ObjectRows) -> Option<kh_look::library::Slot> {
    if slot.mesh == "sora" {
        return None;
    }
    let world = |n: &str| slot.vectors.get(n).filter(|v| v[3] == 1.0).copied();
    let centres = [kh_vfx::screen::in_light_names(0)[3], kh_vfx::screen::in_light_names(1)[3]];
    if centres.iter().all(|n| world(n).is_none()) {
        return None;
    }
    let mut out = slot.clone();
    for name in centres {
        if let Some(w) = world(name) {
            out.vectors.insert(name.to_string(), [w[0] - object.obj.x, w[1] - object.obj.y, w[2] - object.obj.z, 0.0]);
        }
    }
    Some(out)
}

// (VFXR occlusion) what the decals' stencil test lets through, as D3D12 evaluates it: (ref & mask) NOT_EQUAL (value & mask)
#[cfg(test)]
mod vfxr_occlusion_tests {
    use super::*;

    fn passes(stencil: u8) -> bool {
        let (mask, op) = decal_stencil(DXGI_FORMAT_D32_FLOAT_S8X24_UINT).expect("a stencil format gets the test");
        assert_eq!(op.StencilFunc, D3D12_COMPARISON_FUNC_NOT_EQUAL);
        (crate::gfx::CHAR_STENCIL & 0xC0 & mask) != (stencil & mask)
    }

    #[test]
    fn decals_skip_the_characters_only() {
        // Elden Ring's characters and ours (0x50), any value of their class
        assert!(!passes(0x50) && !passes(0x40) && !passes(0x7f));
        // terrain, grass, map objects, sky (the values measured in IMPL_ER56 / ER88)
        for v in [0x12u8, 0xd2, 0x90, 0x10, 0x00] {
            assert!(passes(v), "{v:#04x}");
        }
        // no stencil plane: no test (the decals draw as before)
        assert!(decal_stencil(DXGI_FORMAT_D32_FLOAT).is_none());
    }
}

// (HAIR-PULSE) every form through the core, the effect side and these drives on every outfit's library: none leaves Sora pulsing
#[cfg(test)]
#[path = "vfx_form_pulse_tests.rs"]
mod form_pulse_tests;
