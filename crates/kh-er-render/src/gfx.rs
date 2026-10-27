//! The renderer: our own Direct3D 12 objects on the game's device, and the one command list a
//! frame that draws Sora and the Keyblade just before the game presents.
//!
//! Threads: `present` runs on the game's presenting thread (inside the hook); `worker` is our own
//! thread, which installs the hooks, reads the pack and creates everything slow (textures,
//! buffers, pipeline states) on a command queue of its own, so the game's frame never waits for
//! it. They meet in `CORE`.
//!
//! THE BINDING CONTRACT with the offline-built shaders (crates/kh-shaderc has the same numbers):
//! root parameters 0 frame constants (b0), 1 material constants (b1), 2 skin constants (b2),
//! 3 a table of 17 textures (t0..t16: 14 of the material, the colour table, the environment
//! cube, the scene depth copy), 4 naga's sampler heap (s0, space 0; 2048 entries, the first three
//! used: wrap, clamp, cube), 5 the sampler index buffer (t0, space 1).

use std::collections::HashMap;
use std::ffi::c_void;
use std::mem::ManuallyDrop;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use glam::{Mat4, Vec3, Vec4};
use kh_anim::Trs;
use kh_look::library::{Library, ObjectRows};
use kh_look::KhsState;
use windows::core::{Interface, PCSTR};
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Direct3D::{ID3DBlob, D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST};
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::{IDXGISwapChain, IDXGISwapChain3};

use crate::model::Model;
use crate::texture::{Compressed, Pixels};
use crate::{hook, math, rlog, survey, FrameInput, Options, DEAD, SHARED};

// (IMPL_ER59) 21: + 17..20 an outfit's material textures 14..17 (er_char.wgsl KH_TEX_EXTRA); the main pack's shaders read 0..16
// (IMPL_ER78) 23: + 21 the game's diffuse light accumulation, 22 its screen-space shadow mask (the *_hy shaders read them)
const SLOT_TEXTURES: usize = 23;
const HY_LIGHT_TEXTURE: usize = 21;
const HY_MASK_TEXTURE: usize = 22;
/// (IMPL_ER59) The descriptor of a slot's material texture `k` (14.. go after the depth copy).
fn texture_descriptor(k: usize) -> usize {
    if k < TABLE_TEXTURE { k } else { k + 3 }
}
const TABLE_TEXTURE: usize = 14;
const CUBE_TEXTURE: usize = 15;
const DEPTH_TEXTURE: usize = 16;
const FRAME_SLOTS: usize = 3;
// (FINAL) 2 MB in both builds (was 256 KB without the party, 1 MB with it). VF-b's routed chandelier fragments (vfx.rs
// ROUTED_MAX 160, each a two-matrix skin, its copy a frame before and a material block: about 3 KB, 0.5 MB in all) come on
// top of Sora (about 0.25 MB) and, with the party, the friends' skins and frame copies (ER party S4: about 0.4 MB). A put that
// does not fit fails the whole frame (E_OUTOFMEMORY): at 256 KB Sora was not drawn at all while the Chandelier's shards
// lived (party off); at 1 MB the same from about 130 shards with Donald and Goofy (party on). IMPL_FINAL_STATE.md.
const UPLOAD_BYTES: usize = 2 * 1024 * 1024;
/// (FP) A lower cap on the upload buffer for the offline harness only (`effects --upload-kb N`, 0 = none): the proof that a
/// routed fragment that does not fit skips itself instead of failing the frame. Never set by kh-er.
static UPLOAD_TEST_CAP: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
/// (FP) The harness's `--upload-kb` (bytes; 0 = the full UPLOAD_BYTES). A cap above UPLOAD_BYTES changes nothing.
pub fn set_upload_test_cap(bytes: usize) {
    UPLOAD_TEST_CAP.store(bytes, Ordering::Relaxed);
}
fn upload_cap() -> usize {
    match UPLOAD_TEST_CAP.load(Ordering::Relaxed) {
        0 => UPLOAD_BYTES,
        cap => cap.min(UPLOAD_BYTES),
    }
}
/// (FP) Routed fragments skipped because the upload buffer was full: in all, the frames with any, the most in one frame.
static ROUTED_SKIPPED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static ROUTED_SKIP_FRAMES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static ROUTED_SKIP_PEAK: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
/// (FP) What a routed fragment may not take of the upload buffer: room for every put the frame still needs once the routed
/// fragments have started (the later models' skins, the material blocks of the later draws and of the translucent pass, the
/// party's material blocks), plus a margin. Those puts fail the frame when they do not fit; a routed fragment only skips itself.
fn routed_reserve(later_skins: usize, later_blocks: usize) -> usize {
    let align = |b: usize| (b + 255) & !255;
    let skin = align(16 * (1 + MAX_JOINTS_OUTFIT) * 4);
    let block = align(kh_look::PARAM_ROWS * 16);
    2 * skin * later_skins + block * later_blocks + 64 * 1024
}
/// (FP) The log line of a frame that skipped routed fragments: the first such frame, a new most-in-one-frame, then every 600th.
fn routed_skipped(skipped: usize, of: usize) {
    if skipped == 0 {
        return;
    }
    let total = ROUTED_SKIPPED.fetch_add(skipped as u64, Ordering::Relaxed) + skipped as u64;
    let frames = ROUTED_SKIP_FRAMES.fetch_add(1, Ordering::Relaxed) + 1;
    let peak = ROUTED_SKIP_PEAK.fetch_max(skipped, Ordering::Relaxed);
    if frames == 1 || skipped > peak || frames % 600 == 0 {
        rlog!("render: (FP) {skipped} of {of} routed chandelier fragment(s) skipped this frame: the upload buffer is full (the frame is drawn without them); {total} skipped in {frames} frame(s) so far");
    }
}
/// The skin's joint array in the shader.
const MAX_JOINTS: usize = 200;
/// (ER69) An outfit's skin: its own Bonamik bones stay in the palette (er_char.wgsl KH_JOINTS_EXTRA, its pack's shaders).
const MAX_JOINTS_OUTFIT: usize = 320;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct PsoKey {
    target: i32,
    depth: i32,
    front_ccw: bool,
    cull: bool,
    /// (IMPL_ER21) The target's alpha is OURS and written (the camp menu's portrait, gfx/portrait.rs): coverage over
    /// what is there. False: the game's target, its alpha left alone.
    alpha: bool,
}

// (IMPL_ER21) the camp menu's portrait: Sora into a target of our own at Present
mod portrait;
// (ER party S4) Donald, Goofy and the team props: their libraries, pipeline states and passes (crate::party)
#[cfg(feature = "party")]
pub(crate) mod party;
/// (ER party I) The game-data texture's rows: light.rs's, and with the feature the friends' own probe rows
/// (gfx/party_probe.rs; light.rs's rows and their draws are unchanged).
#[cfg(feature = "party")]
const GAME_ROWS: usize = crate::light::HEIGHT + party::probe::ROWS;
#[cfg(not(feature = "party"))]
const GAME_ROWS: usize = crate::light::HEIGHT;

struct FrameSlot {
    allocator: ID3D12CommandAllocator,
    list: ID3D12GraphicsCommandList,
    upload: ID3D12Resource,
    mapped: *mut u8,
    /// The fence value that says the GPU is done with this slot's last frame.
    busy_until: u64,
    /// The slot's last frame carried timestamp queries that were not read yet.
    timed: bool,
    /// The game-data texture of the slot's last frame, copied for reading back; not read yet.
    readback: Option<ID3D12Resource>,
    read_pending: bool,
    /// A third list for the frame: the late effects and the scene darkening, after the game's forward stage.
    late_allocator: ID3D12CommandAllocator,
    late_list: ID3D12GraphicsCommandList,
    /// A second list for the frame: his motion vectors, submitted inside a later submission of the game.
    velo_allocator: ID3D12CommandAllocator,
    velo_list: ID3D12GraphicsCommandList,
    /// (IMPL_ER53) A fourth list: his EARLY DEPTH, submitted inside the game's deferred stage (after its G-buffer).
    early_allocator: ID3D12CommandAllocator,
    early_list: ID3D12GraphicsCommandList,
    /// (IMPL_ER56) the stencil plane of the scene depth as the G-buffer left it (before his early depth), copied for the
    /// CPU now and then: the buffer, its footprint, the tap pixels to read; a copy waits to be read.
    stencil_rb: Option<(ID3D12Resource, D3D12_PLACED_SUBRESOURCE_FOOTPRINT)>,
    stencil_taps: Vec<(u32, u32)>,
    stencil_pending: bool,
}

/// What the velocity draw of a frame needs from its colour draw (the constants are still in the
/// frame slot's upload buffer).
#[derive(Clone)]
struct VeloFrame {
    slot: usize,
    key: PsoKey,
    width: u32,
    height: u32,
    frame_constants: u64,
    /// Per model drawn: its index, its skin constants of this frame and of the one before.
    skins: Vec<(usize, u64, u64)>,
    /// The scene depth the colour draw made this slot's depth view for (pointer).
    depth: usize,
    /// (ER party S4) the party's members drawn in that frame (their frame constants and skins)
    #[cfg(feature = "party")]
    party: party::VeloParty,
}

/// What the late pass (the late effects, the scene darkening) needs of the frame's colour draw.
struct LateFx {
    slot: usize,
    /// The scene target and the scene depth the colour draw used (pointers).
    target: usize,
    depth: usize,
    depth_format: DXGI_FORMAT,
    width: u32,
    height: u32,
    frame_constants: u64,
    clip: glam::Mat4,
    camera: Vec3,
    right: Vec3,
    up: Vec3,
    forward: Vec3,
    depth_a: f32,
    depth_b: f32,
    proj00: f32,
    scale: f32,
    lit: Vec3,
    /// The effects' late stage is this pass's to draw (the colour draw drew the early stage only).
    effects: bool,
    /// (IMPL_ER65) the colour draw saved his colour and depth for the late re-composite.
    recomp: bool,
    /// (ZFIN) the scene depth's snapshot of this frame (the early depth ran): the finale's pass finds the target's pixels in it
    snap: Option<ID3D12Resource>,
}

/// (IMPL_ER65) THE LATE RE-COMPOSITE's objects: his colour and the scene depth right after his colour draw (copies), a
/// table of their views (+ the snapshot without him, the scene depth now), the pipeline by target format.
struct LateKit {
    saved: ID3D12Resource,
    him: ID3D12Resource,
    heap: ID3D12DescriptorHeap,
    /// the target, the scene depth and the snapshot the views were made for (pointers); the copies are still in their
    /// first state (COPY_DEST)
    target: usize,
    depth: usize,
    snapshot: usize,
    fresh: bool,
    pipeline: Option<(i32, ID3D12PipelineState)>,
    pipeline_failed: bool,
    /// (IMPL_ER80) the re-composite before the game's TAA (recomp2.ps.dxil, alpha-blended), by target format
    pipeline2: Option<(i32, ID3D12PipelineState)>,
    pipeline2_failed: bool,
}

/// What reads the game's light (light.rs has the layout of the texture).
struct GameKit {
    texture: ID3D12Resource,
    rtv: ID3D12DescriptorHeap,
    footprint: D3D12_PLACED_SUBRESOURCE_FOOTPRINT,
    bytes: u64,
    root: ID3D12RootSignature,
    copy_light: ID3D12PipelineState,
    copy_scene: ID3D12PipelineState,
    probe: ID3D12PipelineState,
    /// The resources whose views are in the probe table (pointers; 0 = white).
    probed: [usize; 5],
    /// (IMPL_ER64) the probe pass is probe2.ps.dxil (the ring of taps around his feet; its table's entry 4 = the depth).
    ring: bool,
    /// (IMPL_ER86) the probe pass is probe3.ps.dxil: probe2 + the world probe (`ER_PROBE_WORLD`: `probe.w = 2`).
    world: bool,
}

struct Draw {
    slot: usize,
    first_index: u32,
    index_count: u32,
    base_vertex: i32,
}

struct GpuModel {
    mesh: &'static str,
    _vertices: ID3D12Resource,
    _indices: ID3D12Resource,
    vertex_view: D3D12_VERTEX_BUFFER_VIEW,
    index_view: D3D12_INDEX_BUFFER_VIEW,
    draws: Vec<Draw>,
    /// Per joint of the skin: the bone of the skeleton, and the inverse bind matrix.
    joint_bones: Vec<Option<usize>>,
    inverse_bind: Vec<Mat4>,
    /// The skeleton's reference pose in component space (when the host gives no pose).
    reference: Vec<Trs>,
    /// (IMPL_ER59) An outfit's body: its own skeleton, posed from his (`joint_bones` then index the outfit's bones).
    retarget: Option<crate::outfit::Retarget>,
}

/// Everything made from the pack.
struct Assets {
    library: Library,
    srv_heap: ID3D12DescriptorHeap,
    srv_size: usize,
    white: ID3D12Resource,
    _textures: Vec<ID3D12Resource>,
    /// (IMPL_ER23B) The pack's textures by file: the wear form (costume change) binds other ones into the slots' tables.
    by_file: HashMap<String, ID3D12Resource>,
    /// (IMPL_ER23B) The clock of the frame the wear form was stepped with last.
    wear_clock: Option<f32>,
    models: Vec<GpuModel>,
    /// Sora's bones by name.
    bones: HashMap<String, usize>,
    khs: Option<(String, KhsState)>,
    texture_bytes: u64,
    /// (HY) the resources whose views are in every slot's table at 21 / 22 (the hybrid's capture; 0 = white)
    hy_bound: [usize; 2],
    /// (RSF2 round 8) the resource whose view is in every slot's table at DEPTH_TEXTURE (the game-data texture: the game's own
    /// jittered view-projection and its previous one for his vertex and velocity shaders; 0 = white). PER ASSETS, as the friends'
    /// `Member.data_bound`: a new outfit's tables are bound again. (It was ONE flag on the game kit: after any reload of his
    /// assets (an outfit, a form's look, `r.hybrid` switched in game) his tables kept white there for good, his shaders lost the
    /// game's view-projection: drawn without the TAA's jitter (no anti-aliasing) and with no camera motion in his motion
    /// vectors (the TAA's history of the background pulled into him: "see-through"). Donald and Goofy never had it.)
    data_bound: usize,
}

#[derive(Default)]
struct Stats {
    presents: u32,
    drawn: u32,
    busy: u32,
    waiting: u32,
    micros: u64,
    micros_max: u64,
    draws: u32,
    with_scene_depth: u32,
    /// Pictures the game finished without drawing its world (a loading screen): nothing of ours drawn.
    no_scene: u32,
    /// HARDENING: frames in which nothing of ours was drawn because the frame was not of the shape
    /// we know (a state not observed this frame, a contradicted record, an odd resting state).
    skipped: u32,
    /// How old the host's frame was when it was drawn (microseconds): sum, least, most; and how
    /// often the presenting thread was the submitting one.
    age: u64,
    age_min: u64,
    age_max: u64,
    same_thread: u32,
    /// GPU time of our command list (timestamp queries), microseconds: sum, most, frames measured.
    gpu: u64,
    gpu_max: u64,
    gpu_frames: u32,
}

pub(crate) struct Core {
    /// (RSF2 round 15) the KH caster maps (one depth slice per KH character, the finest cascade's light matrices) and the viewport
    /// of that cascade in the atlas with the time they were drawn
    kh_maps: Option<KhMaps>,
    kh_maps_vp: Option<([f32; 4], Instant)>,
    /// (RSF2 round 14) the KH characters' depth into the game's half-size effect depth: its pipelines, view heaps and lists
    fx_kit: Option<FxKit>,
    fx_failed: bool,
    swap: usize,
    device: ID3D12Device,
    queue: ID3D12CommandQueue,
    root: ID3D12RootSignature,
    sampler_heap: ID3D12DescriptorHeap,
    sampler_index: ID3D12Resource,
    rtv_heap: ID3D12DescriptorHeap,
    dsv_heap: ID3D12DescriptorHeap,
    dsv_size: usize,
    fence: ID3D12Fence,
    fence_value: u64,
    /// Two timestamps a frame slot, resolved into `query_results` (16 bytes a slot).
    queries: Option<(ID3D12QueryHeap, ID3D12Resource, u64)>,
    slots: Vec<FrameSlot>,
    own_depth: Option<(ID3D12Resource, u32, u32)>,
    /// Resources the GPU may still be using: freed once the fence passes the value.
    retired: Vec<(u64, ID3D12Resource)>,
    assets: Option<Assets>,
    assets_failed: bool,
    pso_sets: HashMap<PsoKey, Vec<ID3D12PipelineState>>,
    pso_failed: Vec<PsoKey>,
    wanted: Option<PsoKey>,
    /// (IMPL_ER21) A second set the worker makes when the first is there (the portrait's).
    wanted_more: Option<PsoKey>,
    portrait: Option<portrait::Kit>,
    portrait_failed: Option<Instant>,
    failures: u32,
    disabled: bool,
    /// HARDENING: after a failed frame nothing is drawn until this moment (then it is tried again).
    retry_at: Option<Instant>,
    /// SELF-RECOVERY (IMPL_ER4_STATE.md, the invisible-Sora bug): frames drawn in total and at the
    /// Present before; Presents in a row without a draw while the host submits frames; whether
    /// we are drawing (transitions are logged once); when the survey was last made to learn afresh.
    drawn_total: u64,
    drawn_at_present: u64,
    idle_presents: u32,
    drawing: bool,
    relearnt_at: Option<Instant>,
    /// Why the last attempt of a frame drew nothing (for the transition's log line).
    why_not: &'static str,
    /// The device is lost: the objects are dropped at the next Present and made again when a
    /// working swap chain presents.
    lost: bool,
    stats: Stats,
    told: Vec<String>,
    /// The swap chain's buffers as of the latest Present (pointers, for comparing only).
    back_buffers: Vec<usize>,
    /// The depth resource whose view is in the contact shadow's table (pointer; 0 = none yet).
    blob_depth: usize,
    /// The shadow mask whose view is in the contact shadow's table (pointer; 0 = white).
    blob_mask: usize,
    /// The light target whose view is in every slot's table (pointer; 0 = white).
    probe_light: usize,
    /// (discovery) What copies constant buffers of the game into a texture of ours.
    dump: Option<DumpKit>,
    game: Option<GameKit>,
    game_failed: bool,
    /// The game's light as last read back, and the rig made of it (for the log).
    light: Option<crate::light::GameLight>,
    rig_light: Option<crate::light::RigLight>,
    /// The level of the reflection environment this frame (for the log).
    env_now: f32,
    /// light.rs `calibration` of the latest frame (0 = the frame gave none), for the log.
    calibration: f32,
    velo: Option<VeloFrame>,
    /// (IMPL_ER17) HIS REAL SHADOW: the latest colour draw's constants (the frame slot they are in) and when it was, the
    /// kit, passes drawn since the last log line, when a pass last ran, the GPU-free time of our list this second.
    cast: Option<(VeloFrame, Instant)>,
    cast_kit: Option<CastKit>,
    cast_failed: bool,
    /// (RSF2 round 6) the hybrid's sun from the game's cascades: the kit that copies each cascade's constants, whether it
    /// failed, what was read this frame (atlas, viewports), the scene depth with the KH characters in it (our copy, and the
    /// scene depth it was made for)
    sun_kit: Option<SunKit>,
    sun_failed: bool,
    sun_now: Option<SunNow>,
    kh_depth: Option<(ID3D12Resource, usize)>,
    cast_drawn: u32,
    cast_at: Option<Instant>,
    cast_micros: u64,
    /// What the late pass of this frame needs (the colour draw leaves it), the frame counter of the
    /// colour draws, the count at which a late pass last ran, the darkening pipeline by target format.
    late: Option<LateFx>,
    frame_no: u64,
    late_seen: u64,
    dim: Option<(i32, ID3D12PipelineState)>,
    late_drawn: u32,
    /// The skin constants and the origin of the frame drawn before (per model: body, weapon).
    /// (ER90) and per form weapon (index 2 + its model index)
    /// (RSF) with the renderer frame it was drawn in and where its component stood (motion_continues: every drawn character)
    prev_skin: Vec<(Vec<f32>, u64, Trs)>,
    prev_origin: Option<Vec3>,
    velocity_drawn: u32,
    /// (IMPL_ER53) THE EARLY DEPTH. `early_now`: set by `early_pass` for the draw it makes (the scene depth's two planes'
    /// states at the split); `pending`: the frame slot whose colour list was recorded with an early depth and waits for
    /// the end of the deferred submission (`after_submission` executes it); `snapshot`: a copy of the scene depth
    /// taken just before his depth went in (our light pass and contact shadow read it: the world without him, as
    /// before), with the scene depth it copies (pointer); frames drawn with an early depth since the last log line.
    early_now: Option<[i32; 2]>,
    /// (IMPL_ER56) his stencil value (the ground's at his feet, last read) and the taps it came from (for the log).
    stencil_now: Option<u8>,
    stencil_seen: Vec<u8>,
    pending: Option<usize>,
    snapshot: Option<(ID3D12Resource, usize, bool)>,
    early_drawn: u32,
    /// (IMPL_ER65) the late re-composite (see `LateKit`), frames re-composited since the last log line.
    late_kit: Option<LateKit>,
    recomp_drawn: u32,
    /// (IMPL_ER78) the hybrid's objects (see `HybridKit`).
    hy: HybridKit,
    /// (IMPL_ER80) the glow mask's objects (see `GlowKit`).
    glow: GlowKit,
    /// (ER party S4) the party's members (gfx/party.rs)
    #[cfg(feature = "party")]
    party: party::PartyKit,
}

/// (IMPL_ER78) THE HYBRID's objects: render target views of the game's G-buffer (five per frame slot, written every frame
/// for the targets of that frame), the game resources whose views are in the slots' tables at 21 / 22 (pointers), a dump
/// waiting for its frame to finish on the GPU.
#[derive(Default)]
struct HybridKit {
    rtv: Option<ID3D12DescriptorHeap>,
    views: [usize; 2],
    dump: Option<HybridDump>,
    /// (HY) the capture of the game's light (`HyCapture`), made for the scene's size; why it could not be made
    cap: Option<HyCapture>,
    cap_failed: Option<String>,
    /// (HY) the game targets' states last logged (a change is logged once)
    told: String,
}

/// (HY) THE CAPTURE OF THE GAME'S LIGHT (hycap.ps.dxil): two targets of our own of the scene's size, (the light a white surface
/// got, the occlusion) and (the normal, the sun's visibility), filled in his colour list right after the game's deferred stage
/// over the box of the G-buffer pass; every *_hy3 shader reads them at 21 / 22, never the game's targets. Its pass's input
/// tables (five views of the game's targets, one table per frame slot: a slot's table is rewritten only once its lists are
/// done), its render target views (two per frame slot), its pipeline state, and the box drawn last (cleared to zero next frame).
struct HyCapture {
    light: ID3D12Resource,
    geo: ID3D12Resource,
    width: u32,
    height: u32,
    inputs: ID3D12DescriptorHeap,
    rtv: ID3D12DescriptorHeap,
    pso: ID3D12PipelineState,
    last_box: Option<[i32; 4]>,
    /// (RSF2 round 11) the G-buffer's targets 3 / 4 under the KH characters (hygb.ps.dxil, targets R8G8_UNORM + R16G16B16A16_FLOAT),
    /// its render target views (two per frame slot); None = the file is not in the pack (as before)
    gb_pso: Option<ID3D12PipelineState>,
    gb_rtv: ID3D12DescriptorHeap,
    /// (RSF round 19) the light probes of the KH surfaces (er_char.wgsl hcprobe / hcprobedown, read by hycap at t9); None = the
    /// files are not in the pack (hycap keeps hc_blur)
    probe: Option<HyProbe>,
}

/// (RSF round 19) THE LIGHT PROBES (the user: "lighting and even some detail behind Donald is appearing thru him like he's a
/// ghost"; "fix it for EVERY character"): a 6-slice array (one per direction), level 0 a texel per 8 x 8 px tile of the capture,
/// HY_PROBE_LEVELS levels; their render target views (a view per slice and level) and the views of each level alone for the next
/// level's pass (tables after the frame slots' in the capture's input heap).
struct HyProbe {
    tex: ID3D12Resource,
    w0: u32,
    h0: u32,
    rtv: ID3D12DescriptorHeap,
    pso0: ID3D12PipelineState,
    down: ID3D12PipelineState,
}

const HY_PROBE_SLICES: usize = 6;
const HY_PROBE_LEVELS: usize = 5;
const HY_PROBE_TILE: u32 = 8;

/// (IMPL_ER80) THE GLOW MASK's objects: per target set (the glow source's format as `target`, the depth, the rasteriser) one
/// pipeline state per material slot (`glow_pipelines`), made on the worker; the render target views of the glow source (one
/// per frame slot); frames masked / skipped since the last log line and the latest reason for a skip.
/// (ER87) What the hair fringe at the velocity split needs of the frame's colour draw (its frame slot's constants stay in
/// that slot's upload buffer until the slot's lists are done).
struct HairLate {
    slot: usize,
    key: PsoKey,
    target: usize,
    depth: usize,
    width: u32,
    height: u32,
    frame_constants: u64,
    draws: Vec<crate::hair::HairDraw>,
    /// his translucent draws that go after the fringe (hair.rs `defer_translucent`)
    translucent: Vec<crate::hair::HairDraw>,
    /// (ER91) the 4x hair pass of this frame (hair91.rs): the box around his head (scene pixels), its parameter block, his
    /// stencil (for the TAA class of the strands' own pixels)
    /// and the early depth's snapshot of this frame (the scene depth without him), when there
    h91: Option<([u32; 4], u64, Option<u8>, Option<usize>)>,
}

#[derive(Default)]
struct GlowKit {
    /// (ER87) the hair fringe's pipeline states per (key, for the glow mask), one per slot (None: not a hair slot), hair.rs
    hair_sets: HashMap<(PsoKey, bool), Vec<Option<ID3D12PipelineState>>>,
    hair_failed: Vec<(PsoKey, bool)>,
    /// (ER87) the hair fringe of this frame, drawn at the game's velocity split (after its forward stage, before its TAA:
    /// hair.rs `late`), and the scene target's state there
    hair_late: Option<HairLate>,
    hair_split_state: i32,
    /// (ER91) the 4x hair pass's states per key (one (solid, thin) pair per slot; hair91.rs) and its targets
    hair_msaa: HashMap<PsoKey, Vec<Option<crate::hair91::HairStates>>>,
    h91: crate::hair91::State,
    sets: HashMap<PsoKey, Vec<ID3D12PipelineState>>,
    failed: Vec<PsoKey>,
    wanted: Option<PsoKey>,
    rtv: Option<ID3D12DescriptorHeap>,
    drawn: u32,
    skipped: u32,
    reason: &'static str,
    /// (IMPL_ER80b) HIS MASK for the game's light-shaft composite (shaft.rs): a copy of the scene depth (same description)
    /// whose stencil is 1 where his surfaces are the visible ones; its two views (writable, read-only); the scene depth it
    /// was made for (pointer). Masks of an earlier scene depth are kept (the game's lists may still name them).
    shaft: Option<(ID3D12Resource, ID3D12DescriptorHeap, usize)>,
    old_masks: Vec<(ID3D12Resource, ID3D12DescriptorHeap)>,
    shaft_frames: u32,
    /// (IMPL_ER80b) his masks at the reduced sizes the game's composite draws at: (width, height, resource, its two views)
    small_masks: Vec<(u32, u32, ID3D12Resource, ID3D12DescriptorHeap)>,
    /// the reduced masks' pipeline state (maskdown.ps.dxil) and the view of the full mask's stencil it reads (for which mask)
    maskdown: Option<(ID3D12PipelineState, ID3D12DescriptorHeap, usize)>,
    maskdown_failed: bool,
}

/// (IMPL_ER78, `r.hybrid_dump`) copies of a screen box of the game's targets around him, waiting in read-back buffers.
struct HybridDump {
    slot: usize,
    frame: u64,
    rect: [u32; 4],
    items: Vec<(String, ID3D12Resource, D3D12_PLACED_SUBRESOURCE_FOOTPRINT, i32, String)>,
}

// SAFETY: D3D12 objects are free-threaded; the mapped pointers are only used under the mutex.
unsafe impl Send for Core {}

static CORE: Mutex<Option<Core>> = Mutex::new(None);
/// Set when Present saw a swap chain that is not usable (not D3D12, no queue yet): text for the log.
static WAITING: Mutex<String> = Mutex::new(String::new());
/// HARDENING: our objects are not made again before this moment (after a lost device / a failure).
static NEW_CORE_AT: Mutex<Option<Instant>> = Mutex::new(None);

fn heap(kind: D3D12_HEAP_TYPE) -> D3D12_HEAP_PROPERTIES {
    D3D12_HEAP_PROPERTIES { Type: kind, ..Default::default() }
}

fn buffer_desc(bytes: u64) -> D3D12_RESOURCE_DESC {
    D3D12_RESOURCE_DESC { Dimension: D3D12_RESOURCE_DIMENSION_BUFFER, Width: bytes.max(1), Height: 1, DepthOrArraySize: 1, MipLevels: 1, SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 }, Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR, ..Default::default() }
}

fn texture_desc(width: u32, height: u32, layers: u16, mips: u16, format: DXGI_FORMAT, flags: D3D12_RESOURCE_FLAGS) -> D3D12_RESOURCE_DESC {
    D3D12_RESOURCE_DESC { Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D, Width: width as u64, Height: height, DepthOrArraySize: layers, MipLevels: mips, Format: format, SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 }, Flags: flags, ..Default::default() }
}

fn transition(resource: &ID3D12Resource, subresource: u32, before: D3D12_RESOURCE_STATES, after: D3D12_RESOURCE_STATES) -> D3D12_RESOURCE_BARRIER {
    D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
        Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
        Anonymous: D3D12_RESOURCE_BARRIER_0 {
            // SAFETY: a borrowed copy of the interface pointer: never released through this struct.
            Transition: ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER { pResource: unsafe { std::mem::transmute_copy(resource) }, Subresource: subresource, StateBefore: before, StateAfter: after }),
        },
    }
}

/// (HAIRPERF, passtime.rs) A buffer the CPU reads back (`bytes`), left as a copy destination.
pub(crate) unsafe fn readback_buffer(device: &ID3D12Device, bytes: u64) -> windows::core::Result<ID3D12Resource> {
    committed(device, D3D12_HEAP_TYPE_READBACK, &buffer_desc(bytes), D3D12_RESOURCE_STATE_COPY_DEST, None)
}

unsafe fn committed(device: &ID3D12Device, kind: D3D12_HEAP_TYPE, desc: &D3D12_RESOURCE_DESC, state: D3D12_RESOURCE_STATES, clear: Option<&D3D12_CLEAR_VALUE>) -> windows::core::Result<ID3D12Resource> {
    let mut out: Option<ID3D12Resource> = None;
    device.CreateCommittedResource(&heap(kind), D3D12_HEAP_FLAG_NONE, desc, state, clear.map(|c| c as *const _), &mut out)?;
    out.ok_or_else(|| windows::core::Error::from_hresult(windows::Win32::Foundation::E_FAIL))
}

/// An upload buffer holding `bytes`.
unsafe fn upload_buffer(device: &ID3D12Device, bytes: &[u8]) -> windows::core::Result<ID3D12Resource> {
    let buffer = committed(device, D3D12_HEAP_TYPE_UPLOAD, &buffer_desc(bytes.len() as u64), D3D12_RESOURCE_STATE_GENERIC_READ, None)?;
    let mut at: *mut c_void = std::ptr::null_mut();
    buffer.Map(0, None, Some(&mut at))?;
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), at as *mut u8, bytes.len());
    buffer.Unmap(0, None);
    Ok(buffer)
}

/// The depth-stencil view format of a depth resource's format.
fn depth_view_format(format: DXGI_FORMAT) -> Option<DXGI_FORMAT> {
    Some(match format {
        DXGI_FORMAT_R32G8X24_TYPELESS | DXGI_FORMAT_D32_FLOAT_S8X24_UINT => DXGI_FORMAT_D32_FLOAT_S8X24_UINT,
        DXGI_FORMAT_R24G8_TYPELESS | DXGI_FORMAT_D24_UNORM_S8_UINT => DXGI_FORMAT_D24_UNORM_S8_UINT,
        DXGI_FORMAT_R32_TYPELESS | DXGI_FORMAT_D32_FLOAT => DXGI_FORMAT_D32_FLOAT,
        DXGI_FORMAT_R16_TYPELESS | DXGI_FORMAT_D16_UNORM => DXGI_FORMAT_D16_UNORM,
        _ => return None,
    })
}

/// How our colour goes into a back buffer of this format: 1 = filmic curve + sRGB encoding,
/// 2 = filmic curve, linear (a float back buffer); `None`: a format we do not draw into.
fn output_mode(format: DXGI_FORMAT) -> Option<f32> {
    match format {
        DXGI_FORMAT_R8G8B8A8_UNORM | DXGI_FORMAT_B8G8R8A8_UNORM | DXGI_FORMAT_R10G10B10A2_UNORM => Some(1.0),
        DXGI_FORMAT_R8G8B8A8_UNORM_SRGB | DXGI_FORMAT_B8G8R8A8_UNORM_SRGB | DXGI_FORMAT_R16G16B16A16_FLOAT => Some(2.0),
        _ => None,
    }
}

// ---------------------------------------------------------------------------- the worker

struct Shaders {
    vertex: Vec<u8>,
    /// The contact shadow's vertex and pixel shader, when the pack has them.
    blob: Option<(Vec<u8>, Vec<u8>)>,
    /// The pixel shader that writes his motion vectors.
    velo: Option<Vec<u8>>,
    /// variant -> the pixel shader.
    pixel: HashMap<String, Vec<u8>>,
    /// (IMPL_ER78) variant -> the pixel shader that writes the game's G-buffer (`<variant>_gb`); empty without the hybrid.
    gbuffer: HashMap<String, Vec<u8>>,
    /// (ER87) variant -> the hair FRINGE pixel shader (`..._hairf`, hair.rs); `pixel` then holds the hair's core shader.
    /// Empty without the hair path.
    fringe: HashMap<String, Vec<u8>>,
    /// (ER91) variant -> the 4x hair pass's (solid, thin) pixel shaders (`..._h91m` / `_h91n`, hair91.rs). Empty without them.
    msaa: HashMap<String, (Vec<u8>, Vec<u8>)>,
    /// (RSF) true for SORA's sets only: their pipelines() make taa.rs's own-record velocity state (from Sora's skin_v88). A
    /// party set's description is not Sora's (its skin has 320 joints and the pre-skinned position): it never goes there.
    sora_velocity: bool,
    /// (RSF) a PARTY set's own-record velocity vertex shader (its pack's `skin_v88.vs.dxil`; party.rs `velocity_own`); None
    /// for Sora's sets and for a party pack without it.
    #[cfg_attr(not(feature = "party"), allow(dead_code))]
    velo_own: Option<Vec<u8>>,
}

/// (ER81) `suffix`: an outfit's own set (ownset.rs: `_cg` = `<variant>_cg[_pl|_bl|_bli|_hy2|_gb2]`), "" = the pack's sets as before.
fn read_shaders(pack: &Path, library: &Library, suffix: &str) -> Result<Shaders, String> {
    let dir = pack.join("materials").join("er");
    let read = |name: String| std::fs::read(dir.join(&name)).map_err(|e| format!("{}: {e} (cargo run --release -p kh-shaderc)", dir.join(&name).display()));
    // (XW) a slot's own folder and suffix: the XS form weapons' own materials (`xsi:` slots) are built in
    // PACK/xsi/weapons/materials/er without an outfit's suffix (xsiweapon.rs `shader_dir`); every other slot as before
    let has = |slot: &kh_look::library::Slot, set: &str| {
        let (d, s) = crate::xsiweapon::shader_dir(slot, &dir, suffix);
        d.join(format!("{}{s}{set}.ps.dxil", slot.variant())).is_file()
    };
    let read_of = |slot: &kh_look::library::Slot, set: &str| {
        let (d, s) = crate::xsiweapon::shader_dir(slot, &dir, suffix);
        let file = d.join(format!("{}{s}{set}.ps.dxil", slot.variant()));
        std::fs::read(&file).map_err(|e| format!("{}: {e} (cargo run --release -p kh-shaderc)", file.display()))
    };
    let mut pixel = HashMap::new();
    // (IMPL_ER44) Sora's own point lights (the hero light, the fire lights on him): the `<variant>_pl` shaders, which read
    // a longer frame block (pointlight.rs `sora_block`), when the pack has ALL of them; otherwise the older set as before
    // (his ambient is then lifted instead).
    let own = crate::pointlight::own_shaders_wanted() && library.slots.iter().all(|slot| has(slot, "_pl"));
    // (IMPL_ER60) the *_pl set + the blood layer, when the pack has ALL of them (blood.rs)
    let blood = own && crate::blood::shaders_wanted() && library.slots.iter().all(|slot| has(slot, "_bl"));
    // (IMPL_ER78b) THE HYBRID: the *_bl code + the game's light (`<variant>_hy2`; with the hybrid off for a frame it draws
    // exactly what *_bl draws: proven pixel-identical offline) and the G-buffer writers (`<variant>_gb2`, three targets),
    // when the pack has BOTH sets complete (whatever r.hybrid is: the key can then be switched while the game runs)
    // (HY) THE THIRD VERSION in place of the ER78b set: `<variant>[_S]_hy3` (the *_bli code + the game's light from the
    // renderer's own capture) and `_gb3` (+ `_gb3_hair` for a hair slot's core), complete for every slot, and the capture's
    // own shader (hycap.ps.dxil); USED only while `r.hybrid` is on (a change reloads the outfit: hybrid.rs `reload_wanted`), so
    // `r.hybrid 0` reads exactly today's files. (The *_hy2 / *_gb2 files are no longer read.)
    let hy3_complete = own && library.slots.iter().all(|slot| has(slot, "_hy3") && has(slot, "_gb3"));
    let hybrid = hy3_complete && crate::hybrid::settings().on;
    crate::hybrid::note_read(hy3_complete, hybrid);
    // (ER79) the *_bl set + the cloth's inside (`<variant>_bli`), when a slot is two-sided cloth and the pack has the whole set
    let interior = blood && library.slots.iter().any(|s| s.scalars.contains_key(crate::outfit::INTERIOR_SCALAR)) && library.slots.iter().all(|slot| has(slot, "_bli"));
    if library.slots.iter().any(|s| s.scalars.contains_key(crate::outfit::INTERIOR_SCALAR)) {
        rlog!("render: (ER79) cloth drawn two-sided: {}", if interior { "the *_bli set (the back faces shaded as the inside)" } else { "both faces, the *_bl set (no complete *_bli set: the inside lit as the outside)" });
    }
    let mut gbuffer = HashMap::new();
    // (ER87) the hair path (hair.rs): a masked slot whose chosen shader has `_hair` (core) + `_hairf` (fringe) beside it
    let mut fringe = HashMap::new();
    let mut msaa = HashMap::new();
    // (ER91) which hair files the outfit has (bit 0 ER87's, bit 1 ER91's): the mode for the whole outfit (hair.rs `mode_for`)
    let stem_of = |variant: &str| if hybrid { format!("{variant}{suffix}_hy3") } else if interior { format!("{variant}{suffix}_bli") } else if blood { format!("{variant}{suffix}_bl") } else if own { format!("{variant}{suffix}_pl") } else { format!("{variant}{suffix}") };
    let mut hair_files = 0u8;
    for slot in library.slots.iter().filter(|s| !(s.mesh.starts_with(crate::xsiweapon::PREFIX) && crate::xsiweapon::shader_dir(s, &dir, suffix).0 != dir)) {
        let stem = stem_of(&slot.variant());
        if crate::hair::pick(&dir, &stem, slot.masked && !slot.translucent).is_some() {
            hair_files |= 1;
        }
        if crate::hair::pick91(&dir, &stem, slot.masked && !slot.translucent).is_some() {
            hair_files |= 2;
        }
    }
    let hair_mode = crate::hair::mode_for(hair_files);
    for slot in &library.slots {
        let variant = slot.variant();
        // (XW) a form weapon's own material: its set from its own folder (no hair path)
        if slot.mesh.starts_with(crate::xsiweapon::PREFIX) && crate::xsiweapon::shader_dir(slot, &dir, suffix).0 != dir {
            if !pixel.contains_key(&variant) {
                let set = if hybrid { "_hy3" } else if interior { "_bli" } else if blood { "_bl" } else if own { "_pl" } else { "" };
                pixel.insert(variant.clone(), read_of(slot, set)?);
            }
            if hybrid && !gbuffer.contains_key(&variant) {
                gbuffer.insert(variant.clone(), read_of(slot, "_gb3")?);
            }
            continue;
        }
        if !pixel.contains_key(&variant) {
            let stem = stem_of(&variant);
            let masked = slot.masked && !slot.translucent;
            match (hair_mode, crate::hair::pick91(&dir, &stem, masked), crate::hair::pick(&dir, &stem, masked)) {
                // (ER91) ER91's core / fringe (+ the 4x pass's pair when there)
                (3, Some(((core, edge), pair)), _) => {
                    pixel.insert(variant.clone(), read(core)?);
                    fringe.insert(variant.clone(), read(edge)?);
                    if let Some((solid, thin)) = pair {
                        msaa.insert(variant.clone(), (read(solid)?, read(thin)?));
                    }
                }
                (1, _, Some((core, edge))) => {
                    pixel.insert(variant.clone(), read(core)?);
                    fringe.insert(variant.clone(), read(edge)?);
                }
                _ => {
                    pixel.insert(variant.clone(), read(format!("{stem}.ps.dxil"))?);
                }
            }
        }
        if hybrid && !gbuffer.contains_key(&variant) {
            // (HY) a hair slot whose colour shader is the hair path's core: its G-buffer writer takes exactly the core's pixels
            // (ER91) on ER91's files, ER91's writer (`_gb3_h91`: the continuous coverage at least 1/2), when there
            let hair_gb = if hair_mode == 3 { format!("{variant}{suffix}_gb3_h91.ps.dxil") } else { format!("{variant}{suffix}_gb3_hair.ps.dxil") };
            let core = fringe.contains_key(&variant) && dir.join(&hair_gb).is_file();
            gbuffer.insert(variant.clone(), read(if core { hair_gb } else { format!("{variant}{suffix}_gb3.ps.dxil") })?);
        }
    }
    crate::hybrid::SHADERS.store(hybrid, Ordering::Relaxed);
    rlog!(
        "render: (HY) the hybrid (Elden Ring's own render pass lights him): {}",
        if hybrid {
            "the *_hy3 + *_gb3 sets (r.hybrid 0 reloads today's)"
        } else if hy3_complete {
            "today's sets (r.hybrid = 0; the *_hy3 set is there)"
        } else {
            "today's sets (no complete *_hy3 + *_gb3 set in this pack, or no *_pl set)"
        }
    );
    // (the *_hy set carries the blood layer as well)
    let blood = blood || (hybrid && crate::blood::shaders_wanted());
    crate::blood::set_in_use(blood);
    rlog!("render: the blood layer (IMPL_ER60): {}", if blood { "the *_bl set" } else { "none (no complete *_bl set, or the *_pl set is not in use)" });
    crate::pointlight::set_own_shaders(own);
    rlog!("render: Sora's pixel shaders: {}", if own { "the *_pl set (IMPL_ER44: the hero light and the fire lights are point lights on him)" } else { "the set without his own point lights (no *_pl in the pack: the lights only lift his ambient)" });
    let blob = read("blob.vs.dxil".to_string()).and_then(|v| Ok((v, read("blob.ps.dxil".to_string())?))).ok();
    let velo = read("velo.ps.dxil".to_string()).ok();
    // (ER79) an outfit's skin with its leg guard (outfit.rs `write_leg_guard`; off: `boc.leg_guard = 0`), when its pack has it
    let vertex = if crate::outfit::leg_guard_on() && dir.join("skin_cg.vs.dxil").is_file() { read("skin_cg.vs.dxil".to_string())? } else { read("skin.vs.dxil".to_string())? };
    // (ER88) the velocity vertex shader of the own-record route (taa.rs), the same skin kind as `vertex`
    crate::taa::load_velocity_vs(&dir, crate::outfit::leg_guard_on() && dir.join("skin_cg.vs.dxil").is_file());
    crate::hair::note_read(hair_files, &fringe);
    if hair_mode == 3 {
        rlog!("render: (ER91) the 4x hair pass: {} hair variant(s) with its shaders{}", msaa.len(), if msaa.is_empty() { " (none: the fringe at the split as before)" } else { "" });
    }
    Ok(Shaders { vertex, blob, velo, pixel, gbuffer, fringe, msaa, sora_velocity: true, velo_own: None })
}

/// Our thread: the hooks, then whatever `CORE` needs made.
pub fn worker() {
    if let Err(e) = hook::install().and_then(|_| survey::install()).and_then(|_| crate::cbcap::install()) {
        rlog!("render: the hooks could not be installed ({e}): nothing is drawn");
        hook::remove();
        return;
    }
    // (IMPL_ER80b) the light-shaft watch: not fatal
    if let Err(e) = crate::shaft::install() {
        rlog!("render: (IMPL_ER80b) the light-shaft watch could not be installed ({e}): the game's light shafts stay as they are");
    }
    let Some(pack) = SHARED.lock().ok().and_then(|g| g.as_ref().map(|s| s.pack.clone())) else { return };
    let mut shaders: Option<Shaders> = None;
    // (IMPL_ER59) the folder the shaders were read from (the outfit's)
    let mut shaders_for = PathBuf::new();
    // (ER81) and the own set's suffix they were read with ("" = the pack's sets)
    let mut shaders_suffix = String::new();
    let mut upload_queue: Option<(usize, ID3D12CommandQueue)> = None;
    loop {
        if DEAD.load(Ordering::Relaxed) {
            return;
        }
        enum Job {
            Load(ID3D12Device),
            Pipelines(ID3D12Device, ID3D12RootSignature, PsoKey, Vec<(String, bool, bool, bool)>),
            /// (IMPL_ER80) the glow mask's set for this key
            Glow(ID3D12Device, ID3D12RootSignature, PsoKey, Vec<(String, bool, bool, bool)>),
            /// (ER87) the hair fringe's set for this key (true: the glow mask's)
            Hair(ID3D12Device, ID3D12RootSignature, PsoKey, bool, Vec<(String, bool, bool, bool)>),
        }
        let job = {
            let guard = CORE.lock().unwrap();
            match guard.as_ref() {
                Some(core) if core.assets.is_none() && !core.assets_failed => Some(Job::Load(core.device.clone())),
                // (IMPL_ER59) another outfit is wanted: made here while the drawn one stays, then swapped in
                Some(core) if core.assets.is_some() && !core.lost && crate::outfit::change_wanted() => Some(Job::Load(core.device.clone())),
                // (ER81) the drawn outfit's own shader set was switched on / off: made again
                Some(core) if core.assets.is_some() && !core.lost && crate::ownset::reload_wanted() => Some(Job::Load(core.device.clone())),
                // (ER87) `r.hair` was switched: the drawn outfit made again with / without the hair path
                Some(core) if core.assets.is_some() && !core.lost && crate::hair::reload_wanted() => Some(Job::Load(core.device.clone())),
                // (HY) `r.hybrid` was switched: the drawn outfit made again with / without the *_hy3 set (today's files when off)
                Some(core) if core.assets.is_some() && !core.lost && crate::hybrid::reload_wanted() => Some(Job::Load(core.device.clone())),
                // (IMPL_ER21) the frame's own set first, then the portrait's
                Some(core) => match ([core.wanted, core.wanted_more].into_iter().flatten().find(|key| !core.pso_sets.contains_key(key) && !core.pso_failed.contains(key)), core.assets.as_ref()) {
                    (Some(key), Some(assets)) => {
                        Some(Job::Pipelines(core.device.clone(), core.root.clone(), key, assets.library.slots.iter().map(|s| (s.variant(), s.two_sided, s.translucent, s.masked)).collect()))
                    }
                    // (IMPL_ER80) the glow mask's set, once the frame's own set is there
                    (None, Some(assets)) => core.glow.wanted.filter(|key| !core.glow.sets.contains_key(key) && !core.glow.failed.contains(key)).map(|key| Job::Glow(core.device.clone(), core.root.clone(), key, assets.library.slots.iter().map(|s| (s.variant(), s.two_sided, s.translucent, s.masked)).collect()))
                        // (ER87) the hair fringe's states (colour, then the glow mask's), once the others are there
                        .or_else(|| {
                            let colour = [core.wanted, core.wanted_more].into_iter().flatten().filter(|k| core.pso_sets.contains_key(k)).map(|k| (k, false));
                            let glow = core.glow.wanted.filter(|k| core.glow.sets.contains_key(k)).map(|k| (k, true));
                            colour.chain(glow).find(|(k, g)| !core.glow.hair_sets.contains_key(&(*k, *g)) && !core.glow.hair_failed.contains(&(*k, *g)))
                        }.filter(|_| crate::hair::fringe_ready()).map(|(key, glow)| Job::Hair(core.device.clone(), core.root.clone(), key, glow, assets.library.slots.iter().map(|s| (s.variant(), s.two_sided, s.translucent, s.masked)).collect()))),
                    _ => None,
                },
                None => None,
            }
        };
        match job {
            Some(Job::Load(device)) => {
                let started = Instant::now();
                let outfit = crate::outfit::begin_load();
                // SAFETY: object creation on the game's device from our own thread (free-threaded API).
                let made = hook::as_ours(|| unsafe {
                    let queue = match &upload_queue {
                        Some((d, q)) if *d == device.as_raw() as usize => q.clone(),
                        _ => {
                            let q: ID3D12CommandQueue = device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC { Type: D3D12_COMMAND_LIST_TYPE_DIRECT, ..Default::default() }).map_err(|e| format!("CreateCommandQueue: {e}"))?;
                            upload_queue = Some((device.as_raw() as usize, q.clone()));
                            q
                        }
                    };
                    // (IMPL_ER59) the outfit's folder (the main pack for KH3's own); its shaders are read with it
                    let look = crate::outfit::look_dir(&pack, &outfit);
                    let mut library = Library::load(&look)?;
                    // (ER79) its cloth drawn two-sided with the inside shaded (twosided.json, `boc.cloth_twosided`)
                    crate::outfit::apply_cloth_twosided(&look, &mut library);
                    // (ER81) its own shader set (own_shaders.json, complete, `boc.<outfit>.own_shaders` on): its post-process block
                    // and presets go into the library, its suffix onto every shader name
                    let suffix = crate::ownset::apply(&look, &outfit, &mut library).map(|o| o.suffix).unwrap_or_default();
                    // (ER90, XSI request 1) the XS moves' form weapons: copies of the Kingdom Key's slot with their own maps,
                    // appended after every slot of the library (xsiweapon.rs; none without PACK/xsi/weapons/er_slots.json)
                    let forms = crate::xsiweapon::add_slots(&mut library);
                    if !forms.is_empty() {
                        rlog!("render: (XSI) form weapon slots added: {}", forms.iter().map(|m| m.model.as_str()).collect::<Vec<_>>().join(", "));
                    }
                    // (ER87) also when `r.hair` was switched since they were read (hair.rs)
                    let fresh = if shaders.is_none() || shaders_for != look || shaders_suffix != suffix || crate::hair::reload_wanted() || crate::hybrid::reload_wanted() { Some(read_shaders(&look, &library, &suffix)?) } else { None };
                    let assets = Assets::create(&device, &queue, &pack, library, &outfit)?;
                    // (IMPL_ER59) A SWAP: the frame's pipeline states for the new slots are made HERE, before it, so that he
                    // is not left undrawn while the worker makes them afterwards (a form change wears another outfit)
                    let swap = CORE.lock().ok().and_then(|g| g.as_ref().filter(|c| c.assets.is_some()).map(|c| (c.root.clone(), [c.wanted, c.wanted_more].into_iter().flatten().collect::<Vec<PsoKey>>())));
                    let mut sets = Vec::new();
                    if let (Some((root, keys)), Some(sh)) = (swap, fresh.as_ref().or(shaders.as_ref())) {
                        let slots: Vec<(String, bool, bool, bool)> = assets.library.slots.iter().map(|s| (s.variant(), s.two_sided, s.translucent, s.masked)).collect();
                        for key in keys {
                            if let Ok(set) = pipelines(&device, &root, key, &slots, sh) {
                                sets.push((key, set));
                            }
                        }
                    }
                    Ok::<_, String>((assets, fresh, (look, suffix), sets))
                });
                let mut guard = CORE.lock().unwrap();
                crate::outfit::end_load(&outfit, made.is_ok());
                let mut made_sets = Vec::new();
                let made = made.map(|(assets, fresh, (look, suffix), sets)| {
                    if let Some(fresh) = fresh {
                        (shaders, shaders_for, shaders_suffix) = (Some(fresh), look, suffix);
                    }
                    made_sets = sets;
                    assets
                });
                if let Some(core) = guard.as_mut() {
                    // (IMPL_ER59) an outfit that cannot be made: the drawn one stays
                    if core.assets.is_some() {
                        match made {
                            Ok(assets) => {
                                rlog!("render: OUTFIT {} ready in {:.2} s: {} material slots, {} textures ({} MB)", if outfit.is_empty() { "kh3" } else { &outfit }, started.elapsed().as_secs_f32(), assets.library.slots.len(), assets._textures.len(), assets.texture_bytes / 1_048_576);
                                if let Some(old) = core.assets.replace(assets) {
                                    crate::outfit::retire(Box::new(old));
                                }
                                // (RSF2 round 8) the views the contact shadows keep in the tables: written again into the new ones
                                core.blob_depth = 0;
                                core.blob_mask = 0;
                                crate::outfit::retire(Box::new(std::mem::take(&mut core.pso_sets)));
                                core.pso_failed.clear();
                                // (IMPL_ER80) the glow mask's sets follow the slots: made again for the new outfit
                                crate::outfit::retire(Box::new(std::mem::take(&mut core.glow.sets)));
                                core.glow.failed.clear();
                                // (ER87) and the hair fringe's
                                crate::outfit::retire(Box::new(std::mem::take(&mut core.glow.hair_sets)));
                                core.glow.hair_failed.clear();
                                // (ER91) and the 4x hair pass's
                                crate::outfit::retire(Box::new(std::mem::take(&mut core.glow.hair_msaa)));
                                core.pso_sets.extend(made_sets.drain(..));
                            }
                            Err(e) => rlog!("render: OUTFIT {outfit} could not be made ready ({e}): the drawn one stays"),
                        }
                        continue;
                    }
                    match made {
                        Ok(assets) => {
                            rlog!(
                                "render: pack read in {:.2} s: {} material slots, {} textures ({} MB), models {}",
                                started.elapsed().as_secs_f32(),
                                assets.library.slots.len(),
                                assets._textures.len(),
                                assets.texture_bytes / 1_048_576,
                                assets.models.iter().map(|m| format!("{} ({} draws, {} joints)", m.mesh, m.draws.len(), m.joint_bones.len())).collect::<Vec<_>>().join(", ")
                            );
                            core.assets = Some(assets);
                            core.blob_depth = 0;
                            core.blob_mask = 0;
                        }
                        Err(e) => {
                            rlog!("render: the pack could not be made ready ({e}): nothing is drawn");
                            // (IMPL_ER59) a saved outfit that cannot be made: the main pack is tried next
                            core.assets_failed = outfit.is_empty();
                        }
                    }
                }
            }
            Some(Job::Pipelines(device, root, key, slots)) => {
                let started = Instant::now();
                let made = match shaders.as_ref() {
                    // SAFETY: as above.
                    Some(shaders) => hook::as_ours(|| unsafe { pipelines(&device, &root, key, &slots, shaders) }),
                    None => Err("no shaders".to_string()),
                };
                let mut guard = CORE.lock().unwrap();
                if let Some(core) = guard.as_mut() {
                    match made {
                        Ok(set) => {
                            rlog!("render: {} pipeline states for {key:?} in {:.2} s", set.len(), started.elapsed().as_secs_f32());
                            core.pso_sets.insert(key, set);
                        }
                        Err(e) => {
                            rlog!("render: pipeline states for {key:?} failed ({e}): nothing is drawn with them");
                            core.pso_failed.push(key);
                        }
                    }
                }
            }
            Some(Job::Glow(device, root, key, slots)) => {
                let started = Instant::now();
                let made = match shaders.as_ref() {
                    // SAFETY: as above.
                    Some(shaders) => hook::as_ours(|| unsafe { glow_pipelines(&device, &root, key, &slots, shaders) }),
                    None => Err("no shaders".to_string()),
                };
                let mut guard = CORE.lock().unwrap();
                if let Some(core) = guard.as_mut() {
                    // (the slots may have changed meanwhile: an outfit swap clears the sets; a set for other slots is dropped)
                    let same = core.assets.as_ref().is_some_and(|a| a.library.slots.len() == slots.len());
                    match made {
                        Ok(set) if same => {
                            rlog!("render: (IMPL_ER80) {} glow-mask pipeline states for {key:?} in {:.2} s", set.len(), started.elapsed().as_secs_f32());
                            core.glow.sets.insert(key, set);
                        }
                        Ok(_) => {}
                        Err(e) => {
                            rlog!("render: (IMPL_ER80) the glow-mask pipeline states for {key:?} failed ({e}): no glow mask (today's path)");
                            core.glow.failed.push(key);
                        }
                    }
                }
            }
            Some(Job::Hair(device, root, key, glow, slots)) => {
                let started = Instant::now();
                let made = match shaders.as_ref() {
                    // SAFETY: as above.
                    Some(shaders) => hook::as_ours(|| unsafe {
                        let elements = layout();
                        let k = crate::hair::Target { format: key.target, depth: key.depth, front_ccw: key.front_ccw, cull: key.cull, alpha: key.alpha };
                        let set = crate::hair::fringe_pipelines(&device, &root, k, glow, &slots, &shaders.vertex, &shaders.fringe, &elements)?;
                        // (ER91) with the colour set, the 4x hair pass's (a failure there leaves the fringe as before)
                        let msaa = if glow || shaders.msaa.is_empty() {
                            None
                        } else {
                            match crate::hair91::msaa_pipelines(&device, &root, key.front_ccw, key.cull, DXGI_FORMAT(key.depth), &slots, &shaders.vertex, &shaders.msaa, &shaders.pixel, &elements) {
                                Ok(m) => Some(m),
                                Err(e) => {
                                    rlog!("render: (ER91) the 4x hair pass's states for {key:?} failed ({e}): the fringe as before");
                                    None
                                }
                            }
                        };
                        Ok::<_, String>((set, msaa))
                    }),
                    None => Err("no shaders".to_string()),
                };
                let mut guard = CORE.lock().unwrap();
                if let Some(core) = guard.as_mut() {
                    let same = core.assets.as_ref().is_some_and(|a| a.library.slots.len() == slots.len());
                    match made {
                        Ok((set, msaa)) if same => {
                            rlog!("render: (ER87) {} hair fringe pipeline state(s){} for {key:?} in {:.2} s{}", set.iter().flatten().count(), if glow { " (glow mask)" } else { "" }, started.elapsed().as_secs_f32(), msaa.as_ref().map_or(String::new(), |m| format!("; (ER91) {} 4x hair pair(s)", m.iter().flatten().count())));
                            core.glow.hair_sets.insert((key, glow), set);
                            if let Some(m) = msaa {
                                core.glow.hair_msaa.insert(key, m);
                            }
                        }
                        Ok(_) => {}
                        Err(e) => {
                            rlog!("render: (ER87) the hair fringe pipeline states for {key:?} failed ({e}): the hair's core only");
                            core.glow.hair_failed.push((key, glow));
                        }
                    }
                }
            }
            None => {
                // (ER party S4) the party's libraries and pipeline states, when nothing of Sora's own is waiting
                #[cfg(feature = "party")]
                if party::worker_tick(&pack) {
                    continue;
                }
                // (vfx) the effects' materials and meshes, when nothing of Sora's own is waiting
                let device = CORE.lock().ok().and_then(|g| g.as_ref().filter(|c| !c.lost && !c.disabled && c.assets.is_some()).map(|c| c.device.clone()));
                if device.is_some_and(|d| crate::vfx::worker_tick(&d)) {
                    continue;
                }
                if crate::SCAN.swap(false, Ordering::SeqCst) {
                    if let Some(dir) = crate::DUMP_DIR.lock().ok().and_then(|d| d.clone()) {
                        crate::shscan::run(&dir);
                    }
                }
                std::thread::sleep(Duration::from_millis(30))
            }
        }
    }
}

/// The vertex layout of `model::Vertex` (the shader's inputs are LOC0..LOC7).
fn layout() -> [D3D12_INPUT_ELEMENT_DESC; 8] {
    let element = |index: u32, format: DXGI_FORMAT, offset: u32| D3D12_INPUT_ELEMENT_DESC { SemanticName: PCSTR(b"LOC\0".as_ptr()), SemanticIndex: index, Format: format, InputSlot: 0, AlignedByteOffset: offset, InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA, InstanceDataStepRate: 0 };
    [
        element(0, DXGI_FORMAT_R32G32B32_FLOAT, 0),
        element(1, DXGI_FORMAT_R32G32B32_FLOAT, 12),
        element(2, DXGI_FORMAT_R32G32B32A32_FLOAT, 24),
        element(3, DXGI_FORMAT_R32G32_FLOAT, 40),
        element(4, DXGI_FORMAT_R32G32_FLOAT, 48),
        element(5, DXGI_FORMAT_R32G32_FLOAT, 56),
        element(6, DXGI_FORMAT_R16G16B16A16_UINT, 64),
        element(7, DXGI_FORMAT_R32G32B32A32_FLOAT, 72),
    ]
}

/// One pipeline state per material slot: (variant, two sided, translucent, masked).
unsafe fn pipelines(device: &ID3D12Device, root: &ID3D12RootSignature, key: PsoKey, slots: &[(String, bool, bool, bool)], shaders: &Shaders) -> Result<Vec<ID3D12PipelineState>, String> {
    let elements = layout();
    let mut out = Vec::new();
    for (variant, two_sided, translucent, masked) in slots {
        let pixel = shaders.pixel.get(variant).ok_or_else(|| format!("no pixel shader {variant}"))?;
        // Translucent slots blend by their opacity; masked slots by the coverage of their dither
        // (four patterns -> alpha in quarters: there is no multisampling at Present).
        let blended = *translucent || *masked;
        let mut blend = D3D12_BLEND_DESC::default();
        blend.RenderTarget[0] = D3D12_RENDER_TARGET_BLEND_DESC {
            BlendEnable: blended.into(),
            LogicOpEnable: false.into(),
            SrcBlend: D3D12_BLEND_SRC_ALPHA,
            DestBlend: D3D12_BLEND_INV_SRC_ALPHA,
            BlendOp: D3D12_BLEND_OP_ADD,
            SrcBlendAlpha: if key.alpha { D3D12_BLEND_ONE } else { D3D12_BLEND_ZERO },
            DestBlendAlpha: if key.alpha { D3D12_BLEND_INV_SRC_ALPHA } else { D3D12_BLEND_ONE },
            BlendOpAlpha: D3D12_BLEND_OP_ADD,
            LogicOp: D3D12_LOGIC_OP_NOOP,
            // (the target's alpha is the game's: left alone; (IMPL_ER21) a target of our own: written)
            RenderTargetWriteMask: if key.alpha { D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8 } else { (D3D12_COLOR_WRITE_ENABLE_RED.0 | D3D12_COLOR_WRITE_ENABLE_GREEN.0 | D3D12_COLOR_WRITE_ENABLE_BLUE.0) as u8 },
        };
        let mut formats = [DXGI_FORMAT_UNKNOWN; 8];
        formats[0] = DXGI_FORMAT(key.target);
        let desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            // SAFETY: a borrowed copy of the interface pointer for the duration of the call.
            pRootSignature: std::mem::transmute_copy(root),
            VS: D3D12_SHADER_BYTECODE { pShaderBytecode: shaders.vertex.as_ptr() as *const c_void, BytecodeLength: shaders.vertex.len() },
            PS: D3D12_SHADER_BYTECODE { pShaderBytecode: pixel.as_ptr() as *const c_void, BytecodeLength: pixel.len() },
            BlendState: blend,
            SampleMask: u32::MAX,
            RasterizerState: D3D12_RASTERIZER_DESC {
                FillMode: D3D12_FILL_MODE_SOLID,
                CullMode: if *two_sided || !key.cull { D3D12_CULL_MODE_NONE } else { D3D12_CULL_MODE_BACK },
                FrontCounterClockwise: key.front_ccw.into(),
                DepthClipEnable: true.into(),
                ..Default::default()
            },
            DepthStencilState: D3D12_DEPTH_STENCIL_DESC {
                DepthEnable: true.into(),
                DepthWriteMask: if *translucent { D3D12_DEPTH_WRITE_MASK_ZERO } else { D3D12_DEPTH_WRITE_MASK_ALL },
                // Reversed Z: nearer is larger.
                DepthFunc: D3D12_COMPARISON_FUNC_GREATER_EQUAL,
                StencilEnable: false.into(),
                ..Default::default()
            },
            InputLayout: D3D12_INPUT_LAYOUT_DESC { pInputElementDescs: elements.as_ptr(), NumElements: elements.len() as u32 },
            PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
            NumRenderTargets: 1,
            RTVFormats: formats,
            DSVFormat: DXGI_FORMAT(key.depth),
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            ..Default::default()
        };
        let state: ID3D12PipelineState = device.CreateGraphicsPipelineState(&desc).map_err(|e| format!("CreateGraphicsPipelineState({variant}): {e}"))?;
        out.push(state);
    }
    // The contact shadow, after the slots' states: one triangle, no depth, the target multiplied.
    if let Some((vertex, pixel)) = &shaders.blob {
        let mut blend = D3D12_BLEND_DESC::default();
        blend.RenderTarget[0] = D3D12_RENDER_TARGET_BLEND_DESC {
            BlendEnable: true.into(),
            LogicOpEnable: false.into(),
            SrcBlend: D3D12_BLEND_ZERO,
            DestBlend: D3D12_BLEND_SRC_COLOR,
            BlendOp: D3D12_BLEND_OP_ADD,
            SrcBlendAlpha: D3D12_BLEND_ZERO,
            DestBlendAlpha: D3D12_BLEND_ONE,
            BlendOpAlpha: D3D12_BLEND_OP_ADD,
            LogicOp: D3D12_LOGIC_OP_NOOP,
            RenderTargetWriteMask: (D3D12_COLOR_WRITE_ENABLE_RED.0 | D3D12_COLOR_WRITE_ENABLE_GREEN.0 | D3D12_COLOR_WRITE_ENABLE_BLUE.0) as u8,
        };
        let mut formats = [DXGI_FORMAT_UNKNOWN; 8];
        formats[0] = DXGI_FORMAT(key.target);
        let desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            pRootSignature: std::mem::transmute_copy(root),
            VS: D3D12_SHADER_BYTECODE { pShaderBytecode: vertex.as_ptr() as *const c_void, BytecodeLength: vertex.len() },
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
        let state: ID3D12PipelineState = device.CreateGraphicsPipelineState(&desc).map_err(|e| format!("CreateGraphicsPipelineState(blob): {e}"))?;
        out.push(state);
        // His motion vectors, after the contact shadow's state: the same vertex shader, the
        // game's velocity format, only where the colour pass left his depth (EQUAL, no write).
        if let Some(pixel) = &shaders.velo {
            let mut blend = D3D12_BLEND_DESC::default();
            blend.RenderTarget[0].RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8;
            let mut formats = [DXGI_FORMAT_UNKNOWN; 8];
            formats[0] = DXGI_FORMAT_R16G16_FLOAT;
            let desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
                pRootSignature: std::mem::transmute_copy(root),
                VS: D3D12_SHADER_BYTECODE { pShaderBytecode: shaders.vertex.as_ptr() as *const c_void, BytecodeLength: shaders.vertex.len() },
                PS: D3D12_SHADER_BYTECODE { pShaderBytecode: pixel.as_ptr() as *const c_void, BytecodeLength: pixel.len() },
                BlendState: blend,
                SampleMask: u32::MAX,
                RasterizerState: D3D12_RASTERIZER_DESC { FillMode: D3D12_FILL_MODE_SOLID, CullMode: D3D12_CULL_MODE_NONE, FrontCounterClockwise: key.front_ccw.into(), DepthClipEnable: true.into(), ..Default::default() },
                DepthStencilState: D3D12_DEPTH_STENCIL_DESC { DepthEnable: true.into(), DepthWriteMask: D3D12_DEPTH_WRITE_MASK_ZERO, DepthFunc: D3D12_COMPARISON_FUNC_EQUAL, StencilEnable: false.into(), ..Default::default() },
                InputLayout: D3D12_INPUT_LAYOUT_DESC { pInputElementDescs: elements.as_ptr(), NumElements: elements.len() as u32 },
                PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
                NumRenderTargets: 1,
                RTVFormats: formats,
                DSVFormat: DXGI_FORMAT(key.depth),
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                ..Default::default()
            };
            let state: ID3D12PipelineState = device.CreateGraphicsPipelineState(&desc).map_err(|e| format!("CreateGraphicsPipelineState(velocity): {e}"))?;
            out.push(state);
            // (ER88) the same with the own-record vertex shader (taa.rs; not fatal, kept beside the set)
            // (RSF) Sora's sets only: a party set's velocity description with Sora's skin_v88 is not a valid pair (the log's
            // "own-record velocity state failed ... 0x80070057" of the party's sets); the party's own-record states are made
            // from their own skin_v88 (velocity_own_pipeline, gfx/party.rs)
            if shaders.sora_velocity {
                crate::taa::make_velocity(device, &desc, key_bits(key));
            }
            // (IMPL_ER53) HIS EARLY DEPTH, after the velocity state: one per slot, the slot's own shaders (the masked
            // slots' pixel shader decides which pixels are his, exactly as in the colour draw; the same pair keeps the
            // depth identical), NO target (the colour goes nowhere), the slot's rasteriser, depth written as the colour
            // draw writes it. (The translucent slots' are made too, to keep the indices, and never used.)
            // (NOT FATAL: without them the set is as before and no early depth is drawn)
            let mut early: Vec<ID3D12PipelineState> = Vec::new();
            let mut early_failed: Option<String> = None;
            // (IMPL_ER56) a second set that also writes his STENCIL (a value given per frame: the one the game's G-buffer
            // left on the ground at his feet), when the depth format has a stencil plane
            let has_stencil = matches!(DXGI_FORMAT(key.depth), DXGI_FORMAT_D32_FLOAT_S8X24_UINT | DXGI_FORMAT_D24_UNORM_S8_UINT);
            let stencil_op = D3D12_DEPTH_STENCILOP_DESC { StencilFailOp: D3D12_STENCIL_OP_KEEP, StencilDepthFailOp: D3D12_STENCIL_OP_KEEP, StencilPassOp: D3D12_STENCIL_OP_REPLACE, StencilFunc: D3D12_COMPARISON_FUNC_ALWAYS };
            for (stencil, (variant, two_sided, _, _)) in [false, true].into_iter().filter(|s| !s || has_stencil).flat_map(|s| slots.iter().map(move |slot| (s, slot))) {
                let Some(pixel) = shaders.pixel.get(variant) else {
                    early_failed = Some(format!("no pixel shader {variant}"));
                    break;
                };
                let desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
                    pRootSignature: std::mem::transmute_copy(root),
                    VS: D3D12_SHADER_BYTECODE { pShaderBytecode: shaders.vertex.as_ptr() as *const c_void, BytecodeLength: shaders.vertex.len() },
                    PS: D3D12_SHADER_BYTECODE { pShaderBytecode: pixel.as_ptr() as *const c_void, BytecodeLength: pixel.len() },
                    BlendState: D3D12_BLEND_DESC::default(),
                    SampleMask: u32::MAX,
                    RasterizerState: D3D12_RASTERIZER_DESC {
                        FillMode: D3D12_FILL_MODE_SOLID,
                        CullMode: if *two_sided || !key.cull { D3D12_CULL_MODE_NONE } else { D3D12_CULL_MODE_BACK },
                        FrontCounterClockwise: key.front_ccw.into(),
                        DepthClipEnable: true.into(),
                        ..Default::default()
                    },
                    DepthStencilState: D3D12_DEPTH_STENCIL_DESC { DepthEnable: true.into(), DepthWriteMask: D3D12_DEPTH_WRITE_MASK_ALL, DepthFunc: D3D12_COMPARISON_FUNC_GREATER_EQUAL, StencilEnable: stencil.into(), StencilReadMask: 0xFF, StencilWriteMask: 0xFF, FrontFace: stencil_op, BackFace: stencil_op },
                    InputLayout: D3D12_INPUT_LAYOUT_DESC { pInputElementDescs: elements.as_ptr(), NumElements: elements.len() as u32 },
                    PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
                    NumRenderTargets: 0,
                    RTVFormats: [DXGI_FORMAT_UNKNOWN; 8],
                    DSVFormat: DXGI_FORMAT(key.depth),
                    SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                    ..Default::default()
                };
                match device.CreateGraphicsPipelineState::<ID3D12PipelineState>(&desc) {
                    Ok(state) => early.push(state),
                    Err(e) => {
                        early_failed = Some(format!("CreateGraphicsPipelineState(early depth): {e}"));
                        break;
                    }
                }
            }
            let early_complete = early_failed.is_none() && has_stencil;
            match early_failed {
                None => out.extend(early),
                Some(why) => rlog!("render: (IMPL_ER53) the early-depth pipeline states could not be made ({why}): no early depth"),
            }
            // (IMPL_ER78) THE G-BUFFER WRITERS, after the stencil-writing early-depth states: one per slot, the slot's `_gb`
            // pixel shader into the game's five G-buffer targets (role order, hybrid.rs `role_of`), depth EQUAL to what his
            // early depth left (no depth or stencil write), the slot's rasteriser, no blending. Only with the complete
            // early-depth set before them (the set's length tells the parts apart: gbuffer_base). NOT FATAL.
            if early_complete && !shaders.gbuffer.is_empty() {
                let mut gb: Vec<ID3D12PipelineState> = Vec::new();
                let mut gb_failed: Option<String> = None;
                let mut formats = [DXGI_FORMAT_UNKNOWN; 8];
                for (k, role) in [0usize, 1, 2].iter().enumerate() {
                    let typed = match role {
                        0 => crate::hybrid::fmt::R10G10B10A2_UNORM,
                        1 => crate::hybrid::fmt::R8G8B8A8_UNORM,
                        2 => crate::hybrid::fmt::R8G8B8A8_UNORM_SRGB,
                        3 => crate::hybrid::fmt::R8G8_UNORM,
                        _ => crate::hybrid::fmt::R16G16B16A16_FLOAT,
                    };
                    formats[k] = DXGI_FORMAT(typed);
                }
                let mut blend = D3D12_BLEND_DESC { IndependentBlendEnable: false.into(), ..Default::default() };
                for t in blend.RenderTarget.iter_mut() {
                    t.RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8;
                }
                for (variant, two_sided, _, _) in slots {
                    let Some(pixel) = shaders.gbuffer.get(variant) else {
                        gb_failed = Some(format!("no G-buffer pixel shader {variant}"));
                        break;
                    };
                    let desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
                        pRootSignature: std::mem::transmute_copy(root),
                        VS: D3D12_SHADER_BYTECODE { pShaderBytecode: shaders.vertex.as_ptr() as *const c_void, BytecodeLength: shaders.vertex.len() },
                        PS: D3D12_SHADER_BYTECODE { pShaderBytecode: pixel.as_ptr() as *const c_void, BytecodeLength: pixel.len() },
                        BlendState: blend,
                        SampleMask: u32::MAX,
                        RasterizerState: D3D12_RASTERIZER_DESC {
                            FillMode: D3D12_FILL_MODE_SOLID,
                            CullMode: if *two_sided || !key.cull { D3D12_CULL_MODE_NONE } else { D3D12_CULL_MODE_BACK },
                            FrontCounterClockwise: key.front_ccw.into(),
                            DepthClipEnable: true.into(),
                            ..Default::default()
                        },
                        DepthStencilState: D3D12_DEPTH_STENCIL_DESC { DepthEnable: true.into(), DepthWriteMask: D3D12_DEPTH_WRITE_MASK_ZERO, DepthFunc: D3D12_COMPARISON_FUNC_EQUAL, StencilEnable: false.into(), ..Default::default() },
                        InputLayout: D3D12_INPUT_LAYOUT_DESC { pInputElementDescs: elements.as_ptr(), NumElements: elements.len() as u32 },
                        PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
                        NumRenderTargets: crate::hybrid::WRITTEN as u32,
                        RTVFormats: formats,
                        DSVFormat: DXGI_FORMAT(key.depth),
                        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                        ..Default::default()
                    };
                    match device.CreateGraphicsPipelineState::<ID3D12PipelineState>(&desc) {
                        Ok(state) => gb.push(state),
                        Err(e) => {
                            gb_failed = Some(format!("CreateGraphicsPipelineState(G-buffer {variant}): {e}"));
                            break;
                        }
                    }
                }
                match gb_failed {
                    None => out.extend(gb),
                    Some(why) => rlog!("render: (IMPL_ER78) the G-buffer pipeline states could not be made ({why}): no hybrid with this set"),
                }
            }
        }
    }
    Ok(out)
}

/// (RSF) A PARTY member's OWN-RECORD VELOCITY STATE (ER88's route for the friends, the coordinator's request after the user's
/// "the motion blur on Donald and Goofy moving is INSANE"): the set's velocity state of `pipelines` (the game's velocity
/// format, no blend, no cull, his skin's vertex layout, the set's velocity pixel shader) with his pack's `skin_v88` vertex
/// shader and taa.rs's depth test (GREATER_EQUAL + bias). Without it his frame copy carries the plain previous matrix
/// (gfx/party.rs `prepare`), never the own-record one the plain vertex shader would read as a plain matrix.
#[cfg_attr(not(feature = "party"), allow(dead_code))]
unsafe fn velocity_own_pipeline(device: &ID3D12Device, root: &ID3D12RootSignature, key: PsoKey, vertex: &[u8], pixel: &[u8]) -> Result<ID3D12PipelineState, String> {
    let elements = layout();
    let mut blend = D3D12_BLEND_DESC::default();
    blend.RenderTarget[0].RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8;
    let mut formats = [DXGI_FORMAT_UNKNOWN; 8];
    formats[0] = DXGI_FORMAT_R16G16_FLOAT;
    let mut desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
        // SAFETY: a borrowed copy of the interface pointer for the duration of the call.
        pRootSignature: std::mem::transmute_copy(root),
        VS: D3D12_SHADER_BYTECODE { pShaderBytecode: vertex.as_ptr() as *const c_void, BytecodeLength: vertex.len() },
        PS: D3D12_SHADER_BYTECODE { pShaderBytecode: pixel.as_ptr() as *const c_void, BytecodeLength: pixel.len() },
        BlendState: blend,
        SampleMask: u32::MAX,
        RasterizerState: D3D12_RASTERIZER_DESC { FillMode: D3D12_FILL_MODE_SOLID, CullMode: D3D12_CULL_MODE_NONE, FrontCounterClockwise: key.front_ccw.into(), DepthClipEnable: true.into(), ..Default::default() },
        DepthStencilState: D3D12_DEPTH_STENCIL_DESC { DepthEnable: true.into(), DepthWriteMask: D3D12_DEPTH_WRITE_MASK_ZERO, DepthFunc: D3D12_COMPARISON_FUNC_EQUAL, StencilEnable: false.into(), ..Default::default() },
        InputLayout: D3D12_INPUT_LAYOUT_DESC { pInputElementDescs: elements.as_ptr(), NumElements: elements.len() as u32 },
        PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
        NumRenderTargets: 1,
        RTVFormats: formats,
        DSVFormat: DXGI_FORMAT(key.depth),
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        ..Default::default()
    };
    crate::taa::own_route_depth(&mut desc);
    device.CreateGraphicsPipelineState(&desc).map_err(|e| format!("CreateGraphicsPipelineState(own-record velocity): {e}"))
}

/// (IMPL_ER80) THE GLOW MASK's pipeline states, one per material slot (the translucent slots' too, to keep the indices; they
/// are never drawn): the slot's COLOUR pixel shader and rasteriser (so a masked slot covers exactly the pixels, and with
/// exactly the coverage, of his colour draw), the glow source's format (`key.target`), depth tested GREATER_EQUAL against
/// what his colour draw left (not written), no stencil. The blend writes ZERO where an opaque slot covers and keeps
/// (1 - coverage) of what is there under a masked slot (the coverage his colour draw blends with): the glow of whatever
/// lies BEHIND him leaves his pixels. Nothing of what the pixel shader computes reaches the target.
unsafe fn glow_pipelines(device: &ID3D12Device, root: &ID3D12RootSignature, key: PsoKey, slots: &[(String, bool, bool, bool)], shaders: &Shaders) -> Result<Vec<ID3D12PipelineState>, String> {
    let elements = layout();
    let mut out = Vec::new();
    for (variant, two_sided, _, masked) in slots {
        let pixel = shaders.pixel.get(variant).ok_or_else(|| format!("no pixel shader {variant}"))?;
        let keep = if *masked { D3D12_BLEND_INV_SRC_ALPHA } else { D3D12_BLEND_ZERO };
        let mut blend = D3D12_BLEND_DESC::default();
        blend.RenderTarget[0] = D3D12_RENDER_TARGET_BLEND_DESC {
            BlendEnable: true.into(),
            LogicOpEnable: false.into(),
            SrcBlend: D3D12_BLEND_ZERO,
            DestBlend: keep,
            BlendOp: D3D12_BLEND_OP_ADD,
            SrcBlendAlpha: D3D12_BLEND_ZERO,
            DestBlendAlpha: keep,
            BlendOpAlpha: D3D12_BLEND_OP_ADD,
            LogicOp: D3D12_LOGIC_OP_NOOP,
            RenderTargetWriteMask: D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8,
        };
        let mut formats = [DXGI_FORMAT_UNKNOWN; 8];
        formats[0] = DXGI_FORMAT(key.target);
        let desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            // SAFETY: a borrowed copy of the interface pointer for the duration of the call.
            pRootSignature: std::mem::transmute_copy(root),
            VS: D3D12_SHADER_BYTECODE { pShaderBytecode: shaders.vertex.as_ptr() as *const c_void, BytecodeLength: shaders.vertex.len() },
            PS: D3D12_SHADER_BYTECODE { pShaderBytecode: pixel.as_ptr() as *const c_void, BytecodeLength: pixel.len() },
            BlendState: blend,
            SampleMask: u32::MAX,
            RasterizerState: D3D12_RASTERIZER_DESC {
                FillMode: D3D12_FILL_MODE_SOLID,
                CullMode: if *two_sided || !key.cull { D3D12_CULL_MODE_NONE } else { D3D12_CULL_MODE_BACK },
                FrontCounterClockwise: key.front_ccw.into(),
                DepthClipEnable: true.into(),
                ..Default::default()
            },
            DepthStencilState: D3D12_DEPTH_STENCIL_DESC { DepthEnable: true.into(), DepthWriteMask: D3D12_DEPTH_WRITE_MASK_ZERO, DepthFunc: D3D12_COMPARISON_FUNC_GREATER_EQUAL, StencilEnable: false.into(), ..Default::default() },
            InputLayout: D3D12_INPUT_LAYOUT_DESC { pInputElementDescs: elements.as_ptr(), NumElements: elements.len() as u32 },
            PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
            NumRenderTargets: 1,
            RTVFormats: formats,
            DSVFormat: DXGI_FORMAT(key.depth),
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            ..Default::default()
        };
        let state: ID3D12PipelineState = device.CreateGraphicsPipelineState(&desc).map_err(|e| format!("CreateGraphicsPipelineState(glow mask {variant}): {e}"))?;
        out.push(state);
    }
    Ok(out)
}

/// (HY) The hybrid's capture objects (`HyCapture`) for a scene of this size: made once, again when the size changes (the old
/// ones retired: a frame in flight may still read them). `Err` = why not (logged once per reason; the hybrid stays off).
unsafe fn ensure_capture(device: &ID3D12Device, root: &ID3D12RootSignature, hy: &mut HybridKit, width: u32, height: u32) -> Result<(), String> {
    if hy.cap.as_ref().is_some_and(|c| c.width == width && c.height == height) {
        return Ok(());
    }
    if let Some(why) = hy.cap_failed.as_ref().filter(|w| w.starts_with(&format!("{width}x{height}:"))) {
        return Err(why.clone());
    }
    let made = (|| -> Result<HyCapture, String> {
        let pack = SHARED.lock().ok().and_then(|g| g.as_ref().map(|s| s.pack.clone())).ok_or("no pack")?;
        let dir = pack.join("materials").join("er");
        let vertex = std::fs::read(dir.join("blob.vs.dxil")).map_err(|e| format!("blob.vs.dxil: {e}"))?;
        // (RSF round 19, the offline harness: `KH_HYCAP_DIR=DIR` reads hycap.ps.dxil from DIR first, the pack stays as it is)
        let pixel = std::env::var_os("KH_HYCAP_DIR").and_then(|d| std::fs::read(std::path::Path::new(&d).join("hycap.ps.dxil")).ok()).map_or_else(|| std::fs::read(dir.join("hycap.ps.dxil")), Ok).map_err(|e| format!("hycap.ps.dxil: {e} (kh-shaderc --only hycap)"))?;
        let format = DXGI_FORMAT_R16G16B16A16_FLOAT;
        let clear = D3D12_CLEAR_VALUE { Format: format, Anonymous: D3D12_CLEAR_VALUE_0 { Color: [0.0; 4] } };
        let make = || committed(device, D3D12_HEAP_TYPE_DEFAULT, &texture_desc(width, height, 1, 1, format, D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET), D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, Some(&clear)).map_err(|e| format!("the capture's target: {e}"));
        let (light, geo) = (make()?, make()?);
        // (RSF round 19) + a table per probe level (its level below alone at t9)
        let tables = FRAME_SLOTS + HY_PROBE_LEVELS;
        let inputs: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, NumDescriptors: (tables * SLOT_TEXTURES) as u32, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, NodeMask: 0 }).map_err(|e| format!("the capture's input heap: {e}"))?;
        // (every entry a valid view: null views of a 2-D texture until the frame writes its five)
        let size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV) as usize;
        let null = D3D12_SHADER_RESOURCE_VIEW_DESC { Format: DXGI_FORMAT_R8G8B8A8_UNORM, ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D, Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING, Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 { Texture2D: D3D12_TEX2D_SRV { MostDetailedMip: 0, MipLevels: 1, PlaneSlice: 0, ResourceMinLODClamp: 0.0 } } };
        for k in 0..tables * SLOT_TEXTURES {
            device.CreateShaderResourceView(None, Some(&null), D3D12_CPU_DESCRIPTOR_HANDLE { ptr: inputs.GetCPUDescriptorHandleForHeapStart().ptr + k * size });
        }
        // (RSF round 19) t8 / t9 are arrays: their null views must be array views
        let null_array = D3D12_SHADER_RESOURCE_VIEW_DESC { Format: DXGI_FORMAT_R16G16B16A16_FLOAT, ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2DARRAY, Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING, Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 { Texture2DArray: D3D12_TEX2D_ARRAY_SRV { MostDetailedMip: 0, MipLevels: 1, FirstArraySlice: 0, ArraySize: HY_PROBE_SLICES as u32, PlaneSlice: 0, ResourceMinLODClamp: 0.0 } } };
        for t in 0..tables {
            device.CreateShaderResourceView(None, Some(&null_array), D3D12_CPU_DESCRIPTOR_HANDLE { ptr: inputs.GetCPUDescriptorHandleForHeapStart().ptr + (t * SLOT_TEXTURES + 9) * size });
        }
        let rtv: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV, NumDescriptors: 2, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE, NodeMask: 0 }).map_err(|e| format!("the capture's view heap: {e}"))?;
        let rtv_size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV) as usize;
        device.CreateRenderTargetView(&light, None, rtv.GetCPUDescriptorHandleForHeapStart());
        device.CreateRenderTargetView(&geo, None, D3D12_CPU_DESCRIPTOR_HANDLE { ptr: rtv.GetCPUDescriptorHandleForHeapStart().ptr + rtv_size });
        let mut formats = [DXGI_FORMAT_UNKNOWN; 8];
        (formats[0], formats[1]) = (format, format);
        let mut blend = D3D12_BLEND_DESC { IndependentBlendEnable: false.into(), ..Default::default() };
        for t in blend.RenderTarget.iter_mut() {
            t.RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8;
        }
        let desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            // SAFETY: a borrowed copy of the interface pointer for the duration of the call.
            pRootSignature: std::mem::transmute_copy(root),
            VS: D3D12_SHADER_BYTECODE { pShaderBytecode: vertex.as_ptr() as *const c_void, BytecodeLength: vertex.len() },
            PS: D3D12_SHADER_BYTECODE { pShaderBytecode: pixel.as_ptr() as *const c_void, BytecodeLength: pixel.len() },
            BlendState: blend,
            SampleMask: u32::MAX,
            RasterizerState: D3D12_RASTERIZER_DESC { FillMode: D3D12_FILL_MODE_SOLID, CullMode: D3D12_CULL_MODE_NONE, DepthClipEnable: false.into(), ..Default::default() },
            DepthStencilState: D3D12_DEPTH_STENCIL_DESC { DepthEnable: false.into(), StencilEnable: false.into(), ..Default::default() },
            PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
            NumRenderTargets: 2,
            RTVFormats: formats,
            DSVFormat: DXGI_FORMAT_UNKNOWN,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            ..Default::default()
        };
        let pso: ID3D12PipelineState = device.CreateGraphicsPipelineState(&desc).map_err(|e| format!("the capture's pipeline state: {e}"))?;
        // (RSF2 round 11) the G-buffer's targets 3 / 4 cleared under the KH characters: not fatal (without the file: as before)
        let gb_rtv: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV, NumDescriptors: (2 * FRAME_SLOTS) as u32, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE, NodeMask: 0 }).map_err(|e| format!("the G-buffer clear's view heap: {e}"))?;
        let gb_pso = match std::fs::read(dir.join("hygb.ps.dxil")) {
            Ok(gb_pixel) => {
                let mut gb_desc: std::mem::ManuallyDrop<D3D12_GRAPHICS_PIPELINE_STATE_DESC> = std::mem::ManuallyDrop::new(std::ptr::read(&desc));
                gb_desc.PS = D3D12_SHADER_BYTECODE { pShaderBytecode: gb_pixel.as_ptr() as *const c_void, BytecodeLength: gb_pixel.len() };
                let mut f = [DXGI_FORMAT_UNKNOWN; 8];
                (f[0], f[1]) = (DXGI_FORMAT_R8G8_UNORM, DXGI_FORMAT_R16G16B16A16_FLOAT);
                gb_desc.RTVFormats = f;
                // (a bitwise copy: the root signature pointer is the caller's, never released here)
                let made = device.CreateGraphicsPipelineState::<ID3D12PipelineState>(&*gb_desc);

                match made {
                    Ok(p) => {
                        rlog!("render: (RSF2 round 11) the G-buffer's subsurface / emissive targets are cleared under the KH characters (hygb.ps.dxil, r.hybrid_gbclear)");
                        Some(p)
                    }
                    Err(e) => {
                        rlog!("render: (RSF2 round 11) the G-buffer clear's pipeline state failed ({e}): targets 3 / 4 as before");
                        None
                    }
                }
            }
            Err(_) => {
                rlog!("render: (RSF2 round 11) hygb.ps.dxil is not in the pack: the G-buffer's targets 3 / 4 keep the background under the KH characters (as before)");
                None
            }
        };
        // (RSF round 19) the light probes (not fatal: without the files hycap keeps hc_blur)
        let probe = (|| -> Result<HyProbe, String> {
            let read = |name: &str| std::env::var_os("KH_HYCAP_DIR").and_then(|d| std::fs::read(std::path::Path::new(&d).join(name)).ok()).map_or_else(|| std::fs::read(dir.join(name)), Ok).map_err(|e| format!("{name}: {e}"));
            let (p0, pd) = (read("hcprobe.ps.dxil")?, read("hcprobedown.ps.dxil")?);
            let (w0, h0) = (width.div_ceil(HY_PROBE_TILE).max(1), height.div_ceil(HY_PROBE_TILE).max(1));
            let pf = DXGI_FORMAT_R16G16B16A16_FLOAT;
            let pclear = D3D12_CLEAR_VALUE { Format: pf, Anonymous: D3D12_CLEAR_VALUE_0 { Color: [0.0; 4] } };
            let tex = committed(device, D3D12_HEAP_TYPE_DEFAULT, &texture_desc(w0, h0, HY_PROBE_SLICES as u16, HY_PROBE_LEVELS as u16, pf, D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET), D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, Some(&pclear)).map_err(|e| format!("the probes: {e}"))?;
            let prtv: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV, NumDescriptors: (HY_PROBE_SLICES * HY_PROBE_LEVELS) as u32, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE, NodeMask: 0 }).map_err(|e| format!("the probes' view heap: {e}"))?;
            for l in 0..HY_PROBE_LEVELS {
                for k in 0..HY_PROBE_SLICES {
                    let view = D3D12_RENDER_TARGET_VIEW_DESC { Format: pf, ViewDimension: D3D12_RTV_DIMENSION_TEXTURE2DARRAY, Anonymous: D3D12_RENDER_TARGET_VIEW_DESC_0 { Texture2DArray: D3D12_TEX2D_ARRAY_RTV { MipSlice: l as u32, FirstArraySlice: k as u32, ArraySize: 1, PlaneSlice: 0 } } };
                    device.CreateRenderTargetView(&tex, Some(&view), D3D12_CPU_DESCRIPTOR_HANDLE { ptr: prtv.GetCPUDescriptorHandleForHeapStart().ptr + (l * HY_PROBE_SLICES + k) * rtv_size });
                }
            }
            // t9: every level in the frame slots' tables (hycap), the level below alone in each level's table (the next pass)
            let view = |most: u32, levels: u32| D3D12_SHADER_RESOURCE_VIEW_DESC { Format: pf, ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2DARRAY, Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING, Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 { Texture2DArray: D3D12_TEX2D_ARRAY_SRV { MostDetailedMip: most, MipLevels: levels, FirstArraySlice: 0, ArraySize: HY_PROBE_SLICES as u32, PlaneSlice: 0, ResourceMinLODClamp: 0.0 } } };
            for slot in 0..FRAME_SLOTS {
                device.CreateShaderResourceView(&tex, Some(&view(0, HY_PROBE_LEVELS as u32)), D3D12_CPU_DESCRIPTOR_HANDLE { ptr: inputs.GetCPUDescriptorHandleForHeapStart().ptr + (slot * SLOT_TEXTURES + 9) * size });
            }
            for l in 1..HY_PROBE_LEVELS {
                device.CreateShaderResourceView(&tex, Some(&view(l as u32 - 1, 1)), D3D12_CPU_DESCRIPTOR_HANDLE { ptr: inputs.GetCPUDescriptorHandleForHeapStart().ptr + ((FRAME_SLOTS + l) * SLOT_TEXTURES + 9) * size });
            }
            let make_pso = |pixel: &[u8]| -> Result<ID3D12PipelineState, String> {
                let mut pd_desc: std::mem::ManuallyDrop<D3D12_GRAPHICS_PIPELINE_STATE_DESC> = std::mem::ManuallyDrop::new(std::ptr::read(&desc));
                pd_desc.PS = D3D12_SHADER_BYTECODE { pShaderBytecode: pixel.as_ptr() as *const c_void, BytecodeLength: pixel.len() };
                let mut f = [DXGI_FORMAT_UNKNOWN; 8];
                for slot in f.iter_mut().take(HY_PROBE_SLICES) {
                    *slot = pf;
                }
                pd_desc.RTVFormats = f;
                pd_desc.NumRenderTargets = HY_PROBE_SLICES as u32;
                // (a bitwise copy: the root signature pointer is the caller's, never released here)
                device.CreateGraphicsPipelineState::<ID3D12PipelineState>(&*pd_desc).map_err(|e| format!("the probes' pipeline state: {e}"))
            };
            Ok(HyProbe { tex, w0, h0, rtv: prtv, pso0: make_pso(&p0)?, down: make_pso(&pd)? })
        })();
        let probe = match probe {
            Ok(p) => {
                rlog!("render: (RSF round 19) the KH surfaces' light probes made ({}x{} x {HY_PROBE_SLICES} directions, {HY_PROBE_LEVELS} levels): the game's light on them prefiltered, nothing of the background printed (r.hybrid_fit 6)", p.w0, p.h0);
                Some(p)
            }
            Err(e) => {
                rlog!("render: (RSF round 19) no light probes ({e}): the capture keeps hc_blur (r.hybrid_fit 5)");
                None
            }
        };
        Ok(HyCapture { light, geo, width, height, inputs, rtv, pso, last_box: None, gb_pso, gb_rtv, probe })
    })();
    match made {
        Ok(cap) => {
            rlog!("render: (HY) the hybrid's capture made: two {width}x{height} RGBA16F targets (the game's light on his surfaces, the normal and the sun's visibility)");
            if let Some(old) = hy.cap.replace(cap) {
                crate::outfit::retire(Box::new(old));
            }
            hy.cap_failed = None;
            Ok(())
        }
        Err(why) => {
            let why = format!("{width}x{height}: {why}");
            rlog!("render: (HY) the hybrid's capture could not be made ({why}): no hybrid (today's light)");
            hy.cap_failed = Some(why.clone());
            Err(why)
        }
    }
}

/// (HY) THE CAPTURE PASS (in his colour list, right after the game's deferred stage): the game's light target, its shadow mask
/// (0 = none: white) and its G-buffer's normal / parameters / colour (`inputs`, all resting readable: the plan checked their
/// states) are read by hycap.ps.dxil into our two targets over `bx`; the box drawn the frame before is cleared first. Our
/// targets go RENDER_TARGET -> PIXEL_SHADER_RESOURCE (they rest readable for every later draw of the frame, the hair fringe's
/// at the velocity split included); no barrier on any resource of the game.
unsafe fn record_capture(device: &ID3D12Device, list: &ID3D12GraphicsCommandList, root: &ID3D12RootSignature, cap: &mut HyCapture, slot_index: usize, inputs: [usize; 8], kh_maps: usize, white: &ID3D12Resource, frame_constants: u64, bx: [i32; 4]) {
    let size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV) as usize;
    let table = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: cap.inputs.GetCPUDescriptorHandleForHeapStart().ptr + slot_index * SLOT_TEXTURES * size };
    // (RSF2 round 15) entry 8: the KH caster maps (a 2D array, R16); none = a null array view (reads 0: no KH caster)
    {
        let raw = kh_maps as *mut c_void;
        let desc = D3D12_SHADER_RESOURCE_VIEW_DESC { Format: DXGI_FORMAT_R16_UNORM, ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2DARRAY, Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING, Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 { Texture2DArray: D3D12_TEX2D_ARRAY_SRV { MostDetailedMip: 0, MipLevels: 1, FirstArraySlice: 0, ArraySize: KH_SLICES as u32, PlaneSlice: 0, ResourceMinLODClamp: 0.0 } } };
        device.CreateShaderResourceView(ID3D12Resource::from_raw_borrowed(&raw), Some(&desc), D3D12_CPU_DESCRIPTOR_HANDLE { ptr: table.ptr + 8 * size });
    }
    // (the light R11G11B10, the mask RGBA8 (or white), the normal R10G10B10A2, the parameters RGBA8, the colour through an sRGB view)
    // (RSF2 round 6) + the depth with the KH characters (its depth plane), the sun's cascade atlas (16-bit), the cascades' rows
    let depth_format = |r: &ID3D12Resource| depth_srv_format(r.GetDesc().Format);
    for (k, pointer) in inputs.iter().enumerate() {
        let at = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: table.ptr + k * size };
        let raw = *pointer as *mut c_void;
        let resource = ID3D12Resource::from_raw_borrowed(&raw);
        let format = match k {
            2 => Some(DXGI_FORMAT_R10G10B10A2_UNORM),
            3 => Some(DXGI_FORMAT_R8G8B8A8_UNORM),
            4 => Some(DXGI_FORMAT_R8G8B8A8_UNORM_SRGB),
            5 => resource.and_then(depth_format),
            6 => Some(DXGI_FORMAT_R16_UNORM),
            _ => None,
        };
        // (a depth of an unknown format: not read)
        let resource = resource.filter(|_| k != 5 || format.is_some());
        match (resource, format) {
            (Some(resource), Some(f)) => {
                let desc = D3D12_SHADER_RESOURCE_VIEW_DESC { Format: f, ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D, Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING, Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 { Texture2D: D3D12_TEX2D_SRV { MostDetailedMip: 0, MipLevels: 1, PlaneSlice: 0, ResourceMinLODClamp: 0.0 } } };
                device.CreateShaderResourceView(resource, Some(&desc), at);
            }
            (Some(resource), None) => device.CreateShaderResourceView(resource, None, at),
            (None, _) => device.CreateShaderResourceView(white, None, at),
        }
    }
    let first = cap.rtv.GetCPUDescriptorHandleForHeapStart();
    let rtv_size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV) as usize;
    let second = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: first.ptr + rtv_size };
    // (made resting readable; every list takes them to RENDER_TARGET and back, so a list that is never executed leaves the
    // record right; cleared whole (to their clear value: a fast clear) so that nothing of an earlier frame stays outside the box)
    list.ResourceBarrier(&[transition(&cap.light, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, D3D12_RESOURCE_STATE_RENDER_TARGET), transition(&cap.geo, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, D3D12_RESOURCE_STATE_RENDER_TARGET)]);
    for view in [first, second] {
        list.ClearRenderTargetView(view, &[0.0; 4], None);
    }
    list.OMSetRenderTargets(2, Some(&first), true, None);
    list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: cap.width as f32, Height: cap.height as f32, MinDepth: 0.0, MaxDepth: 1.0 }]);
    let bx = [bx[0].clamp(0, cap.width as i32), bx[1].clamp(0, cap.height as i32), bx[2].clamp(0, cap.width as i32), bx[3].clamp(0, cap.height as i32)];
    list.RSSetScissorRects(&[RECT { left: bx[0], top: bx[1], right: bx[2], bottom: bx[3] }]);
    list.SetGraphicsRootSignature(root);
    list.SetDescriptorHeaps(&[Some(cap.inputs.clone())]);
    list.SetGraphicsRootConstantBufferView(0, frame_constants);
    list.SetGraphicsRootDescriptorTable(3, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: cap.inputs.GetGPUDescriptorHandleForHeapStart().ptr + (slot_index * SLOT_TEXTURES * size) as u64 });
    list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
    // (RSF round 19) THE LIGHT PROBES of this frame, level by level (each level's slices to RENDER_TARGET and back), from the
    // inputs bound above; then the capture's targets and table again
    if let Some(pr) = cap.probe.as_ref() {
        let prtv0 = pr.rtv.GetCPUDescriptorHandleForHeapStart();
        for l in 0..HY_PROBE_LEVELS {
            let (w, h) = ((pr.w0 >> l).max(1), (pr.h0 >> l).max(1));
            let subs: Vec<u32> = (0..HY_PROBE_SLICES).map(|k| (l + k * HY_PROBE_LEVELS) as u32).collect();
            list.ResourceBarrier(&subs.iter().map(|s| transition(&pr.tex, *s, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, D3D12_RESOURCE_STATE_RENDER_TARGET)).collect::<Vec<_>>());
            let views: Vec<D3D12_CPU_DESCRIPTOR_HANDLE> = (0..HY_PROBE_SLICES).map(|k| D3D12_CPU_DESCRIPTOR_HANDLE { ptr: prtv0.ptr + (l * HY_PROBE_SLICES + k) * rtv_size }).collect();
            list.OMSetRenderTargets(HY_PROBE_SLICES as u32, Some(views.as_ptr()), false, None);
            list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: w as f32, Height: h as f32, MinDepth: 0.0, MaxDepth: 1.0 }]);
            list.RSSetScissorRects(&[RECT { left: 0, top: 0, right: w as i32, bottom: h as i32 }]);
            if l == 0 {
                list.SetPipelineState(&pr.pso0);
            } else {
                list.SetGraphicsRootDescriptorTable(3, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: cap.inputs.GetGPUDescriptorHandleForHeapStart().ptr + ((FRAME_SLOTS + l) * SLOT_TEXTURES * size) as u64 });
                list.SetPipelineState(&pr.down);
            }
            list.DrawInstanced(3, 1, 0, 0);
            list.ResourceBarrier(&subs.iter().map(|s| transition(&pr.tex, *s, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE)).collect::<Vec<_>>());
        }
        list.OMSetRenderTargets(2, Some(&first), true, None);
        list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: cap.width as f32, Height: cap.height as f32, MinDepth: 0.0, MaxDepth: 1.0 }]);
        list.RSSetScissorRects(&[RECT { left: bx[0], top: bx[1], right: bx[2], bottom: bx[3] }]);
        list.SetGraphicsRootDescriptorTable(3, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: cap.inputs.GetGPUDescriptorHandleForHeapStart().ptr + (slot_index * SLOT_TEXTURES * size) as u64 });
    }
    list.SetPipelineState(&cap.pso);
    if bx[2] > bx[0] && bx[3] > bx[1] {
        list.DrawInstanced(3, 1, 0, 0);
    }
    list.ResourceBarrier(&[transition(&cap.light, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE), transition(&cap.geo, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE)]);
    cap.last_box = Some(bx);
}

/// (RSF2 round 6) The view format of a depth resource's depth plane (None: not a depth format we read).
fn depth_srv_format(format: DXGI_FORMAT) -> Option<DXGI_FORMAT> {
    match format {
        DXGI_FORMAT_R32G8X24_TYPELESS | DXGI_FORMAT_D32_FLOAT_S8X24_UINT => Some(DXGI_FORMAT_R32_FLOAT_X8X24_TYPELESS),
        DXGI_FORMAT_R32_TYPELESS | DXGI_FORMAT_D32_FLOAT => Some(DXGI_FORMAT_R32_FLOAT),
        DXGI_FORMAT_R24G8_TYPELESS | DXGI_FORMAT_D24_UNORM_S8_UINT => Some(DXGI_FORMAT_R24_UNORM_X8_TYPELESS),
        _ => None,
    }
}

/// (IMPL_ER80) THE GLOW MASK'S PER-FRAME CHECK: the glow source may be written in this frame only when it is not the scene
/// target itself, was transitioned by the game in this frame with nothing contradicted, is an R11G11B10 / RGBA16 float
/// target, and rests in a state we can leave and return to (not COMMON, not a depth or copy-destination state).
/// `None` = go; `Some(reason)` = today's path for the frame (the reason goes to the log).
pub(crate) fn glow_check(glow: usize, state: i32, format: i32, scene_target: usize, usable: bool) -> Option<&'static str> {
    if glow == 0 || glow == scene_target {
        Some("the composite has no second target apart from the scene")
    } else if !usable {
        Some("the glow source was not transitioned by the game in this frame, or a barrier contradicted its record")
    } else if format != DXGI_FORMAT_R11G11B10_FLOAT.0 && format != DXGI_FORMAT_R16G16B16A16_FLOAT.0 {
        Some("the glow source is not an R11G11B10 / RGBA16 float target")
    } else if state == 0 || state & (D3D12_RESOURCE_STATE_DEPTH_WRITE.0 | D3D12_RESOURCE_STATE_DEPTH_READ.0 | D3D12_RESOURCE_STATE_COPY_DEST.0) != 0 {
        Some("the glow source rests in a state we do not know how to leave and restore")
    } else {
        None
    }
}

/// (RSF2, the offline harness only) a party member's probe reading (crate::party::test_probe).
#[cfg(feature = "party")]
pub(crate) fn test_party_probe(chara: u8, light: crate::light::GameLight) {
    party::probe::set_test(chara, light);
}

/// (RSF) INT's discontinuity rule (kh-er interp.rs `int_snap_check`: a jump of more than 3 m or a turn of more than 90 degrees
/// between two steps is never blended) for the motion vectors of EVERY drawn KH character (Sora's skins and each party
/// member's): such a frame, and a frame that does not follow the one the previous skin was drawn in (hidden for a cutscene, a
/// load, the void), get no skinned motion (the skin a frame ago = now).
pub(crate) const SNAP_CM: f32 = 300.0;
pub(crate) const SNAP_DEG: f32 = 90.0;

pub(crate) fn motion_continues(prev_frame: u64, prev: &Trs, frame: u64, now: &Trs) -> bool {
    prev_frame + 1 == frame && (now.translation - prev.translation).length() <= SNAP_CM && prev.rotation.angle_between(now.rotation).to_degrees() <= SNAP_DEG
}

/// (IMPL_ER53) Where a set's early-depth states start (`pipelines`: the slots', the contact shadow's, the velocity's, then
/// one per slot); `None` for a set without them.
fn early_base(set_len: usize, slots: usize) -> Option<usize> {
    (slots > 0 && (set_len == 2 * slots + 2 || set_len == 3 * slots + 2 || set_len == 4 * slots + 2)).then_some(slots + 2)
}

/// (IMPL_ER56) Where the stencil-writing early-depth states start (after the plain ones), when the set has them.
fn early_stencil_base(set_len: usize, slots: usize) -> Option<usize> {
    (slots > 0 && (set_len == 3 * slots + 2 || set_len == 4 * slots + 2)).then_some(2 * slots + 2)
}

/// (IMPL_ER78) Where the G-buffer writers start (after the stencil-writing early-depth states), when the set has them.
fn gbuffer_base(set_len: usize, slots: usize) -> Option<usize> {
    (slots > 0 && set_len == 4 * slots + 2).then_some(3 * slots + 2)
}

/// (IMPL_ER56) The stencil value for his pixels from the taps the game's G-buffer left around his feet (read before his
/// early depth): the most frequent non-zero one (the ground's; 0 is "nothing drawn there", the sky); `None` without one.
pub(crate) fn ground_stencil(taps: &[u8]) -> Option<u8> {
    let mut best: Option<(u8, usize)> = None;
    for &v in taps.iter().filter(|v| **v != 0) {
        let n = taps.iter().filter(|t| **t == v).count();
        if best.is_none_or(|(_, m)| n > m) {
            best = Some((v, n));
        }
    }
    best.map(|(v, _)| v)
}

impl Assets {
    unsafe fn create(device: &ID3D12Device, queue: &ID3D12CommandQueue, pack: &Path, library: Library, outfit: &str) -> Result<Assets, String> {
        Self::create_full(device, queue, pack, library, outfit, true)
    }

    /// (ER party S4) `sora` = false: a party library (gfx/party.rs): its textures and tables as Sora's, no models of Sora's
    /// (the caller adds its own), its skeleton (`pack/manifest.json`) when it has one.
    unsafe fn create_full(device: &ID3D12Device, queue: &ID3D12CommandQueue, pack: &Path, library: Library, outfit: &str, sora: bool) -> Result<Assets, String> {
        let text = |what: &str, e: windows::core::Error| format!("{what}: {e}");
        // ---- the skeletons (bone names in the order the host's poses come in)
        let skeleton_of = |manifest: &Path| -> Result<kh_anim::Skeleton, String> {
            let text = std::fs::read_to_string(manifest).map_err(|e| format!("{}: {e}", manifest.display()))?;
            kh_anim::Skeleton::from_manifest(&kh_anim::json::Json::parse(&text)?)
        };
        let reference = |skeleton: &kh_anim::Skeleton| -> Vec<Trs> {
            let mut component: Vec<Trs> = Vec::with_capacity(skeleton.len());
            for bone in &skeleton.bones {
                let t = match bone.parent {
                    Some(parent) => bone.reference.then(&component[parent]),
                    None => bone.reference,
                };
                component.push(t);
            }
            component
        };
        let body_skeleton = match skeleton_of(&pack.join("manifest.json")) {
            Ok(s) => s,
            // (ER party S4) the team props' library has no skeleton
            Err(_) if !sora => kh_anim::Skeleton::default(),
            Err(e) => return Err(e),
        };
        // (IMPL_ER59) an outfit's body: its own skeleton, posed from his
        let outfit_retarget = crate::outfit::retarget_for(pack, outfit, &body_skeleton)?;
        let bones: HashMap<String, usize> = body_skeleton.bones.iter().enumerate().map(|(i, b)| (b.name.clone(), i)).collect();

        // ---- upload machinery of our own
        let allocator: ID3D12CommandAllocator = device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT).map_err(|e| text("CreateCommandAllocator", e))?;
        let list: ID3D12GraphicsCommandList = device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator, None).map_err(|e| text("CreateCommandList", e))?;
        list.Close().map_err(|e| text("Close", e))?;
        let fence: ID3D12Fence = device.CreateFence(0, D3D12_FENCE_FLAG_NONE).map_err(|e| text("CreateFence", e))?;
        let mut fence_value = 0u64;
        let mut texture_bytes = 0u64;
        let mut textures: Vec<ID3D12Resource> = Vec::new();
        // One texture: created, filled through an upload buffer, left readable by pixel shaders.
        // (`block_bytes` > 0: a block-compressed format; a row is then a row of 4 x 4 blocks)
        let mut make = |width: u32, height: u32, layers: u16, format: DXGI_FORMAT, levels: &[(usize, u32, u32)], data: &[u8], bytes_per_texel: usize, block_bytes: usize| -> Result<ID3D12Resource, String> {
            let mips = (levels.len() / layers as usize) as u16;
            let desc = texture_desc(width, height, layers, mips, format, D3D12_RESOURCE_FLAG_NONE);
            let texture = committed(device, D3D12_HEAP_TYPE_DEFAULT, &desc, D3D12_RESOURCE_STATE_COPY_DEST, None).map_err(|e| text("CreateCommittedResource(texture)", e))?;
            let count = levels.len();
            let mut footprints = vec![D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default(); count];
            let mut total = 0u64;
            device.GetCopyableFootprints(&desc, 0, count as u32, 0, Some(footprints.as_mut_ptr()), None, None, Some(&mut total));
            let staging = committed(device, D3D12_HEAP_TYPE_UPLOAD, &buffer_desc(total), D3D12_RESOURCE_STATE_GENERIC_READ, None).map_err(|e| text("CreateCommittedResource(upload)", e))?;
            let mut at: *mut c_void = std::ptr::null_mut();
            staging.Map(0, None, Some(&mut at)).map_err(|e| text("Map", e))?;
            for (k, (offset, w, h)) in levels.iter().enumerate() {
                let f = &footprints[k];
                let (row, rows) = if block_bytes > 0 { ((*w as usize).div_ceil(4) * block_bytes, (*h as usize).div_ceil(4)) } else { (*w as usize * bytes_per_texel, *h as usize) };
                for y in 0..rows {
                    std::ptr::copy_nonoverlapping(data.as_ptr().add(offset + y * row), (at as *mut u8).add(f.Offset as usize + y * f.Footprint.RowPitch as usize), row);
                }
            }
            staging.Unmap(0, None);
            allocator.Reset().map_err(|e| text("allocator Reset", e))?;
            list.Reset(&allocator, None).map_err(|e| text("list Reset", e))?;
            for (k, f) in footprints.iter().enumerate() {
                let to = D3D12_TEXTURE_COPY_LOCATION { pResource: std::mem::transmute_copy(&texture), Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX, Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { SubresourceIndex: k as u32 } };
                let from = D3D12_TEXTURE_COPY_LOCATION { pResource: std::mem::transmute_copy(&staging), Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT, Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { PlacedFootprint: *f } };
                list.CopyTextureRegion(&to, 0, 0, 0, &from, None);
            }
            list.ResourceBarrier(&[transition(&texture, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_COPY_DEST, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE)]);
            list.Close().map_err(|e| text("Close", e))?;
            queue.ExecuteCommandLists(&[Some(list.cast::<ID3D12CommandList>().map_err(|e| text("cast", e))?)]);
            fence_value += 1;
            queue.Signal(&fence, fence_value).map_err(|e| text("Signal", e))?;
            let waited = Instant::now();
            while fence.GetCompletedValue() < fence_value {
                if waited.elapsed() > Duration::from_secs(10) || device.GetDeviceRemovedReason().is_err() {
                    return Err("the GPU did not finish a texture upload".to_string());
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            texture_bytes += total;
            Ok(texture)
        };

        // ---- the textures of the pack, each once
        let white = make(1, 1, 1, DXGI_FORMAT_R8G8B8A8_UNORM, &[(0, 1, 1)], &[255, 255, 255, 255], 4, 0)?;
        let mut wanted: Vec<kh_look::library::TextureRef> = Vec::new();
        for t in library.slots.iter().flat_map(|s| s.textures.iter().flatten()) {
            if !wanted.iter().any(|w| w.file == t.file) {
                wanted.push(t.clone());
            }
        }
        // (IMPL_ER23B) the wear forms' textures (Second Form's costume): bound when the form is put on
        for t in library.wear_textures() {
            if !wanted.iter().any(|w| w.file == t.file) {
                wanted.push(t);
            }
        }
        if let Some((file, srgb)) = library.khs.table.clone() {
            if !wanted.iter().any(|w| w.file == file) {
                wanted.push(kh_look::library::TextureRef { file, srgb, mips: false });
            }
        }
        // Decoding on a few threads, uploading here.
        let dir = library.dir.clone();
        // A texture comes as the game's own block-compressed file when the pack has it
        // (PACK/materials/er/tex/<name>.dds, put there by kh-shaderc: a quarter of the memory and
        // the game's own mips), else as the PNG. KH_ER_TEXTURES=png takes the PNGs regardless.
        enum Loaded {
            Blocks(Compressed),
            Texels(Pixels),
        }
        let png_only = std::env::var("KH_ER_TEXTURES").is_ok_and(|v| v == "png");
        let load = |t: &kh_look::library::TextureRef| -> Result<Loaded, String> {
            let stem = Path::new(&t.file).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let dds = dir.join("er").join("tex").join(format!("{stem}.dds"));
            if !png_only && t.mips && dds.is_file() {
                if let Ok(blocks) = Compressed::load_dds(&dds, t.srgb) {
                    return Ok(Loaded::Blocks(blocks));
                }
            }
            Pixels::load_png(&dir.join(&t.file), t.mips).map(Loaded::Texels)
        };
        let decoded: Vec<(kh_look::library::TextureRef, Result<Loaded, String>)> = std::thread::scope(|scope| {
            let jobs: Vec<_> = wanted.chunks(wanted.len().div_ceil(6).max(1)).map(|chunk| { let load = &load; scope.spawn(move || chunk.iter().map(|t| (t.clone(), load(t))).collect::<Vec<_>>()) }).collect();
            jobs.into_iter().flat_map(|j| j.join().unwrap_or_default()).collect()
        });
        let mut by_file: HashMap<String, ID3D12Resource> = HashMap::new();
        let mut compressed = 0u32;
        for (t, loaded) in decoded {
            match loaded {
                Ok(Loaded::Texels(p)) => {
                    let format = if t.srgb { DXGI_FORMAT_R8G8B8A8_UNORM_SRGB } else { DXGI_FORMAT_R8G8B8A8_UNORM };
                    let texture = make(p.width, p.height, 1, format, &p.mips, &p.data, 4, 0)?;
                    textures.push(texture.clone());
                    by_file.insert(t.file.clone(), texture);
                }
                Ok(Loaded::Blocks(b)) => {
                    let texture = make(b.width, b.height, 1, DXGI_FORMAT(b.format), &b.mips, &b.data, 0, b.block_bytes)?;
                    textures.push(texture.clone());
                    by_file.insert(t.file.clone(), texture);
                    compressed += 1;
                }
                Err(e) => rlog!("render: WARNING: texture not read ({e}): white is used"),
            }
        }
        rlog!("render: {compressed} texture(s) taken block-compressed from the pack's DDS files");
        // ---- the reflection environment of the stand-in rig
        let (cube_data, cube_mips) = kh_look::stand_in::rig().env.cube_rgba16f();
        let mut cube_levels = Vec::new();
        let mut offset = 0usize;
        for _face in 0..6 {
            for mip in 0..cube_mips {
                let n = (kh_look::ENV_SIZE >> mip).max(1);
                cube_levels.push((offset, n, n));
                offset += (n * n * 8) as usize;
            }
        }
        let cube = make(kh_look::ENV_SIZE, kh_look::ENV_SIZE, 6, DXGI_FORMAT_R16G16B16A16_FLOAT, &cube_levels, &cube_data, 8, 0)?;

        // ---- the descriptor table of every slot
        let srv_heap: ID3D12DescriptorHeap = device
            .CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, NumDescriptors: ((library.slots.len() + 3) * SLOT_TEXTURES) as u32 /* (IMPL_ER39) + the light pass table */, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, NodeMask: 0 })
            .map_err(|e| text("CreateDescriptorHeap(srv)", e))?;
        let srv_size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV) as usize;
        let start = srv_heap.GetCPUDescriptorHandleForHeapStart();
        let table = library.khs.table.as_ref().and_then(|(file, _)| by_file.get(file)).unwrap_or(&white);
        let cube_view = D3D12_SHADER_RESOURCE_VIEW_DESC {
            Format: DXGI_FORMAT_R16G16B16A16_FLOAT,
            ViewDimension: D3D12_SRV_DIMENSION_TEXTURECUBE,
            Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
            Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 { TextureCube: D3D12_TEXCUBE_SRV { MostDetailedMip: 0, MipLevels: cube_mips, ResourceMinLODClamp: 0.0 } },
        };
        for (k, slot) in library.slots.iter().enumerate() {
            let at = |texture: usize| D3D12_CPU_DESCRIPTOR_HANDLE { ptr: start.ptr + (k * SLOT_TEXTURES + texture) * srv_size };
            for texture in TABLE_TEXTURE..kh_look::library::SLOT_TEXTURES_MAX {
                let resource = slot.textures.get(texture).and_then(|t| t.as_ref()).and_then(|t| by_file.get(&t.file)).unwrap_or(&white);
                device.CreateShaderResourceView(resource, None, at(texture_descriptor(texture)));
            }
            for texture in 0..TABLE_TEXTURE {
                let resource = slot.textures.get(texture).and_then(|t| t.as_ref()).and_then(|t| by_file.get(&t.file)).unwrap_or(&white);
                device.CreateShaderResourceView(resource, None, at(texture));
            }
            device.CreateShaderResourceView(table, None, at(TABLE_TEXTURE));
            device.CreateShaderResourceView(&cube, Some(&cube_view), at(CUBE_TEXTURE));
            device.CreateShaderResourceView(&white, None, at(DEPTH_TEXTURE));
            // (IMPL_ER78) the game's light and shadow mask: white until the hybrid binds them
            device.CreateShaderResourceView(&white, None, at(HY_LIGHT_TEXTURE));
            device.CreateShaderResourceView(&white, None, at(HY_MASK_TEXTURE));
        }
        // (the table after the slots' is the contact shadow's: its last entry becomes the scene depth;
        // the one after that is the light probes': the game's targets around his feet)
        for texture in 0..2 * SLOT_TEXTURES {
            device.CreateShaderResourceView(&white, None, D3D12_CPU_DESCRIPTOR_HANDLE { ptr: start.ptr + (library.slots.len() * SLOT_TEXTURES + texture) * srv_size });
        }
        textures.push(white.clone());
        textures.push(cube);

        // ---- the models
        let mut models = Vec::new();
        let weapon_dir = pack.join("weapon");
        let sources: [(&'static str, std::path::PathBuf, Option<kh_anim::Skeleton>); 2] = [
            ("sora", crate::outfit::model_file(pack, outfit), Some(body_skeleton)),
            ("kingdom_key", weapon_dir.join("kingdom_key.glb"), skeleton_of(&weapon_dir.join("manifest.json")).ok()),
        ];
        // (ER party S4) a party library's models are added by gfx/party.rs
        for (mesh, file, skeleton) in sources.into_iter().filter(|_| sora) {
            if !file.is_file() {
                rlog!("render: {} is not in the pack: not drawn", file.display());
                continue;
            }
            let Some(skeleton) = skeleton else {
                rlog!("render: no skeleton for {mesh}: not drawn");
                continue;
            };
            let model = Model::load(&file)?;
            let room = if mesh == "sora" && outfit_retarget.is_some() { MAX_JOINTS_OUTFIT } else { MAX_JOINTS };
            if model.joints.len() > room {
                return Err(format!("{mesh}: {} joints, the shader has room for {room}", model.joints.len()));
            }
            let vertices = upload_buffer(device, model.vertex_bytes()).map_err(|e| text("vertex buffer", e))?;
            let indices = upload_buffer(device, model.index_bytes()).map_err(|e| text("index buffer", e))?;
            let draws = model
                .primitives
                .iter()
                .filter_map(|p| {
                    let slot = library.slots.iter().position(|s| s.mesh == mesh && s.slot == p.material);
                    if slot.is_none() {
                        rlog!("render: {mesh}: primitive {} has no material slot in the pack: not drawn", p.material);
                    }
                    Some(Draw { slot: slot?, first_index: p.first_index, index_count: p.index_count, base_vertex: p.base_vertex })
                })
                .collect();
            let missing = model.joints.iter().filter(|(name, _)| skeleton.index_of(name).is_none()).count();
            if missing > 0 {
                rlog!("render: {mesh}: {missing} joint(s) of the skin are not bones of the skeleton");
            }
            models.push(GpuModel {
                mesh,
                vertex_view: D3D12_VERTEX_BUFFER_VIEW { BufferLocation: vertices.GetGPUVirtualAddress(), SizeInBytes: model.vertex_bytes().len() as u32, StrideInBytes: std::mem::size_of::<crate::model::Vertex>() as u32 },
                index_view: D3D12_INDEX_BUFFER_VIEW { BufferLocation: indices.GetGPUVirtualAddress(), SizeInBytes: model.index_bytes().len() as u32, Format: DXGI_FORMAT_R32_UINT },
                _vertices: vertices,
                _indices: indices,
                draws,
                joint_bones: model.joints.iter().map(|(name, _)| match (mesh, &outfit_retarget) { ("sora", Some(r)) => r.index_of(name), _ => skeleton.index_of(name) }).collect(),
                inverse_bind: model.joints.iter().map(|(_, m)| *m).collect(),
                reference: reference(&skeleton),
                retarget: if mesh == "sora" { outfit_retarget.clone().map(|r| r.with_guard_joints(&model.joints)) } else { None },
            });
        }
        // (ER90, XSI request 1) the form weapons whose slots the library got (xsiweapon.rs `add_slots`), placed by the host's
        // `FrameInput::forms`: (XW) skinned on their own skeleton (the joints' bones, its reference pose; the host hands the
        // clip's pose); ER90's table: their bind pose (every joint's matrix the identity)
        for form in crate::xsiweapon::table().map(|t| t.models.as_slice()).unwrap_or(&[]).iter().filter(|m| sora && library.slots.iter().any(|s| s.mesh == m.mesh)) {
            let model = match Model::load(&form.glb) {
                Ok(m) => m,
                Err(e) => {
                    rlog!("render: (XSI) form weapon {} not read ({e}): not drawn", form.model);
                    continue;
                }
            };
            if model.joints.len() > MAX_JOINTS {
                rlog!("render: (XSI) form weapon {}: {} joints, the shader has room for {MAX_JOINTS}: not drawn", form.model, model.joints.len());
                continue;
            }
            let vertices = upload_buffer(device, model.vertex_bytes()).map_err(|e| text("vertex buffer", e))?;
            let indices = upload_buffer(device, model.index_bytes()).map_err(|e| text("index buffer", e))?;
            let draws: Vec<Draw> = model.primitives.iter().filter_map(|p| Some(Draw { slot: library.slots.iter().position(|s| s.mesh == form.mesh && s.slot == p.material)?, first_index: p.first_index, index_count: p.index_count, base_vertex: p.base_vertex })).collect();
            if draws.len() != model.primitives.len() {
                rlog!("render: (XSI) form weapon {}: {} of {} primitive(s) have no slot: not drawn", form.model, model.primitives.len() - draws.len(), model.primitives.len());
            }
            models.push(GpuModel {
                mesh: form.mesh.as_str(),
                vertex_view: D3D12_VERTEX_BUFFER_VIEW { BufferLocation: vertices.GetGPUVirtualAddress(), SizeInBytes: model.vertex_bytes().len() as u32, StrideInBytes: std::mem::size_of::<crate::model::Vertex>() as u32 },
                index_view: D3D12_INDEX_BUFFER_VIEW { BufferLocation: indices.GetGPUVirtualAddress(), SizeInBytes: model.index_bytes().len() as u32, Format: DXGI_FORMAT_R32_UINT },
                _vertices: vertices,
                _indices: indices,
                draws,
                joint_bones: match &form.rig {
                    Some(rig) => model.joints.iter().map(|(name, _)| rig.skeleton.index_of(name)).collect(),
                    None => vec![None; model.joints.len()],
                },
                inverse_bind: model.joints.iter().map(|(_, m)| *m).collect(),
                reference: form.rig.as_ref().map(|r| r.reference()).unwrap_or_default(),
                retarget: None,
            });
        }
        Ok(Assets { library, srv_heap, srv_size, white: white.clone(), _textures: textures, by_file, wear_clock: None, models, bones, khs: None, texture_bytes, hy_bound: [0; 2], data_bound: 0 })
    }
}

// ---------------------------------------------------------------------------- the core

impl Core {
    unsafe fn new(swap: usize, device: ID3D12Device, queue: ID3D12CommandQueue) -> windows::core::Result<Core> {
        // ---- the root signature (the contract at the head of this file)
        let textures = [D3D12_DESCRIPTOR_RANGE { RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SRV, NumDescriptors: SLOT_TEXTURES as u32, BaseShaderRegister: 0, RegisterSpace: 0, OffsetInDescriptorsFromTableStart: 0 }];
        let samplers = [D3D12_DESCRIPTOR_RANGE { RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SAMPLER, NumDescriptors: 2048, BaseShaderRegister: 0, RegisterSpace: 0, OffsetInDescriptorsFromTableStart: 0 }];
        let constants = |register: u32, visibility: D3D12_SHADER_VISIBILITY| D3D12_ROOT_PARAMETER { ParameterType: D3D12_ROOT_PARAMETER_TYPE_CBV, Anonymous: D3D12_ROOT_PARAMETER_0 { Descriptor: D3D12_ROOT_DESCRIPTOR { ShaderRegister: register, RegisterSpace: 0 } }, ShaderVisibility: visibility };
        let table = |ranges: &[D3D12_DESCRIPTOR_RANGE], visibility: D3D12_SHADER_VISIBILITY| D3D12_ROOT_PARAMETER { ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE, Anonymous: D3D12_ROOT_PARAMETER_0 { DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE { NumDescriptorRanges: ranges.len() as u32, pDescriptorRanges: ranges.as_ptr() } }, ShaderVisibility: visibility };
        let parameters = [
            constants(0, D3D12_SHADER_VISIBILITY_ALL),
            constants(1, D3D12_SHADER_VISIBILITY_PIXEL),
            constants(2, D3D12_SHADER_VISIBILITY_VERTEX),
            // (the vertex stage reads the game-data texture: t16)
            table(&textures, D3D12_SHADER_VISIBILITY_ALL),
            table(&samplers, D3D12_SHADER_VISIBILITY_PIXEL),
            D3D12_ROOT_PARAMETER { ParameterType: D3D12_ROOT_PARAMETER_TYPE_SRV, Anonymous: D3D12_ROOT_PARAMETER_0 { Descriptor: D3D12_ROOT_DESCRIPTOR { ShaderRegister: 0, RegisterSpace: 1 } }, ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL },
            // 6: the skin constants of the frame before (b3: his motion vectors).
            constants(3, D3D12_SHADER_VISIBILITY_VERTEX),
        ];
        let desc = D3D12_ROOT_SIGNATURE_DESC { NumParameters: parameters.len() as u32, pParameters: parameters.as_ptr(), NumStaticSamplers: 0, pStaticSamplers: std::ptr::null(), Flags: D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT };
        let mut blob: Option<ID3DBlob> = None;
        let mut errors: Option<ID3DBlob> = None;
        if let Err(e) = D3D12SerializeRootSignature(&desc, D3D_ROOT_SIGNATURE_VERSION_1, &mut blob, Some(&mut errors)) {
            if let Some(errors) = errors {
                rlog!("render: root signature: {}", String::from_utf8_lossy(std::slice::from_raw_parts(errors.GetBufferPointer() as *const u8, errors.GetBufferSize())));
            }
            return Err(e);
        }
        let blob = blob.ok_or_else(|| windows::core::Error::from_hresult(windows::Win32::Foundation::E_FAIL))?;
        let root: ID3D12RootSignature = device.CreateRootSignature(0, std::slice::from_raw_parts(blob.GetBufferPointer() as *const u8, blob.GetBufferSize()))?;

        // ---- the samplers: naga's heap of 2048, the first three used
        let sampler_heap: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER, NumDescriptors: 2048, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, NodeMask: 0 })?;
        let sampler_size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER) as usize;
        let sampler_start = sampler_heap.GetCPUDescriptorHandleForHeapStart();
        let sampler = |address: D3D12_TEXTURE_ADDRESS_MODE, filter: D3D12_FILTER, anisotropy: u32| D3D12_SAMPLER_DESC { Filter: filter, AddressU: address, AddressV: address, AddressW: address, MipLODBias: 0.0, MaxAnisotropy: anisotropy, ComparisonFunc: D3D12_COMPARISON_FUNC_NEVER, BorderColor: [0.0; 4], MinLOD: 0.0, MaxLOD: D3D12_FLOAT32_MAX };
        let three = [
            // (as the test app: trilinear, anisotropy 8, wrap / clamp)
            sampler(D3D12_TEXTURE_ADDRESS_MODE_WRAP, D3D12_FILTER_ANISOTROPIC, 8),
            // (VFXR red) the clamp sampler reads the colour tables (RGBTable, 16 slices side by side): anisotropic taps
            // along the screen footprint spanned slices where a pixel quad's lookup jumps one (red / magenta 2 x 2 specks on
            // Donald's blue clothes and eyes, test app proof build/sora_tmp/vfxr/shots/donald_red10): trilinear, no
            // anisotropy, as the test app now
            sampler(D3D12_TEXTURE_ADDRESS_MODE_CLAMP, D3D12_FILTER_MIN_MAG_MIP_LINEAR, 1),
            sampler(D3D12_TEXTURE_ADDRESS_MODE_CLAMP, D3D12_FILTER_MIN_MAG_MIP_LINEAR, 1),
        ];
        for k in 0..2048usize {
            device.CreateSampler(&three[k.min(2)], D3D12_CPU_DESCRIPTOR_HANDLE { ptr: sampler_start.ptr + k * sampler_size });
        }
        let index_words: [u32; 4] = [0, 1, 2, 0];
        let sampler_index = upload_buffer(&device, std::slice::from_raw_parts(index_words.as_ptr() as *const u8, 16))?;

        let rtv_heap: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV, NumDescriptors: 3 * FRAME_SLOTS as u32, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE, NodeMask: 0 })?;
        let dsv_heap: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_DSV, NumDescriptors: 1 + FRAME_SLOTS as u32, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE, NodeMask: 0 })?;
        let dsv_size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_DSV) as usize;
        let fence: ID3D12Fence = device.CreateFence(0, D3D12_FENCE_FLAG_NONE)?;
        let mut slots = Vec::new();
        for _ in 0..FRAME_SLOTS {
            let allocator: ID3D12CommandAllocator = device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
            let list: ID3D12GraphicsCommandList = device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator, None)?;
            list.Close()?;
            let upload = committed(&device, D3D12_HEAP_TYPE_UPLOAD, &buffer_desc(UPLOAD_BYTES as u64), D3D12_RESOURCE_STATE_GENERIC_READ, None)?;
            let mut at: *mut c_void = std::ptr::null_mut();
            upload.Map(0, None, Some(&mut at))?;
            let velo_allocator: ID3D12CommandAllocator = device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
            let velo_list: ID3D12GraphicsCommandList = device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &velo_allocator, None)?;
            velo_list.Close()?;
            let late_allocator: ID3D12CommandAllocator = device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
            let late_list: ID3D12GraphicsCommandList = device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &late_allocator, None)?;
            late_list.Close()?;
            let early_allocator: ID3D12CommandAllocator = device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
            let early_list: ID3D12GraphicsCommandList = device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &early_allocator, None)?;
            early_list.Close()?;
            slots.push(FrameSlot { allocator, list, upload, mapped: at as *mut u8, busy_until: 0, timed: false, readback: None, read_pending: false, velo_allocator, velo_list, late_allocator, late_list, early_allocator, early_list, stencil_rb: None, stencil_taps: Vec::new(), stencil_pending: false });
        }
        // Timestamp queries for the GPU time of our list (optional: without them nothing is measured).
        let queries = (|| -> windows::core::Result<(ID3D12QueryHeap, ID3D12Resource, u64)> {
            let mut heap: Option<ID3D12QueryHeap> = None;
            device.CreateQueryHeap(&D3D12_QUERY_HEAP_DESC { Type: D3D12_QUERY_HEAP_TYPE_TIMESTAMP, Count: 2 * FRAME_SLOTS as u32, NodeMask: 0 }, &mut heap)?;
            let results = committed(&device, D3D12_HEAP_TYPE_READBACK, &buffer_desc(16 * FRAME_SLOTS as u64), D3D12_RESOURCE_STATE_COPY_DEST, None)?;
            let frequency = queue.GetTimestampFrequency()?;
            Ok((heap.ok_or_else(|| windows::core::Error::from_hresult(windows::Win32::Foundation::E_FAIL))?, results, frequency.max(1)))
        })()
        .ok();
        Ok(Core { swap, device, queue, root, queries, sampler_heap, sampler_index, rtv_heap, dsv_heap, dsv_size, fence, fence_value: 0, slots, own_depth: None, retired: Vec::new(), assets: None, assets_failed: false, pso_sets: HashMap::new(), pso_failed: Vec::new(), wanted: None, wanted_more: None, portrait: None, portrait_failed: None, failures: 0, disabled: false, retry_at: None, drawn_total: 0, drawn_at_present: 0, idle_presents: 0, drawing: false, relearnt_at: None, why_not: "the game's deferred stage (G-buffer + HDR target with the scene depth) was not recognised in the frame", lost: false, stats: Stats::default(), told: Vec::new(), back_buffers: Vec::new(), blob_depth: 0, blob_mask: 0, probe_light: 0, dump: None, game: None, game_failed: false, light: None, rig_light: None, env_now: 1.0, calibration: crate::light::CALIBRATION_DEFAULT, velo: None, late: None, frame_no: 0, late_seen: 0, dim: None, late_drawn: 0, prev_skin: vec![(Vec::new(), 0, Trs::IDENTITY), (Vec::new(), 0, Trs::IDENTITY)], prev_origin: None, velocity_drawn: 0, cast: None, cast_kit: None, cast_failed: false, sun_kit: None, sun_failed: false, sun_now: None, kh_depth: None, cast_drawn: 0, cast_at: None, cast_micros: 0, early_now: None, late_kit: None, recomp_drawn: 0, stencil_now: None, stencil_seen: Vec::new(), pending: None, snapshot: None, early_drawn: 0, kh_maps: None, kh_maps_vp: None, fx_kit: None, fx_failed: false, hy: HybridKit::default(), glow: GlowKit::default(), #[cfg(feature = "party")] party: party::PartyKit::default() })
    }

    /// Logs a text once.
    fn tell(&mut self, text: String) {
        if !self.told.contains(&text) {
            rlog!("render: {text}");
            self.told.push(text);
        }
    }

    /// The game presents: bookkeeping, and the draw itself only in the `r.present` arrangement.
    unsafe fn at_present(&mut self, swap: &IDXGISwapChain3) -> windows::core::Result<()> {
        self.stats.presents += 1;
        let desc = swap.GetDesc1()?;
        survey::end_frame(desc.Width, desc.Height);
        crate::cbcap::end_frame();
        // Which resources are the swap chain's buffers (the survey watches for the submission
        // that finishes one).
        let mut buffers = Vec::with_capacity(desc.BufferCount as usize);
        for k in 0..desc.BufferCount {
            buffers.push(swap.GetBuffer::<ID3D12Resource>(k)?.as_raw() as usize);
        }
        // (the main scene depth, whatever its size: the render resolution need not be the window's)
        if survey::hold_main_depth().is_none() {
            let _ = survey::scene_depth(desc.Width, desc.Height);
        }
        // SELF-RECOVERY: is he being drawn? A change either way is logged once; after 3 s without
        // a draw while the host keeps submitting frames, everything learnt about the game's resources
        // is dropped and learnt afresh (at most every 5 s), whatever the reason was.
        let host_live = SHARED.lock().ok().and_then(|g| g.as_ref().and_then(|s| s.inputs.last().map(|(_, at, _)| at.elapsed() < Duration::from_millis(500)))).unwrap_or(false);
        let drew = self.drawn_total != self.drawn_at_present;
        self.drawn_at_present = self.drawn_total;
        if drew {
            if !self.drawing {
                rlog!("render: DRAWING (he is in the picture again)");
            }
            self.drawing = true;
            self.idle_presents = 0;
        } else if host_live && !at_present_option(self) {
            self.idle_presents += 1;
            if self.drawing && self.idle_presents == 30 {
                rlog!("render: NOT DRAWING for half a second although the host submits frames: {}", self.why_not);
                self.drawing = false;
            }
            if self.idle_presents >= 180 && self.relearnt_at.is_none_or(|at| at.elapsed() > Duration::from_secs(5)) {
                rlog!("render: nothing drawn for {} frames ({}): everything learnt about the game's render targets is dropped and learnt afresh", self.idle_presents, self.why_not);
                survey::forget();
                // (the survey lost the swap chain's buffers too: they are handed over again below)
                self.back_buffers.clear();
                self.told.clear();
                self.relearnt_at = Some(Instant::now());
            }
            self.why_not = "the game's deferred stage (G-buffer + HDR target with the scene depth) was not recognised in the frame";
        } else {
            self.idle_presents = 0;
        }
        let recreated = survey::RECREATED.swap(0, Ordering::Relaxed);
        if recreated > 0 {
            rlog!("render: {recreated} resource(s) of the game were made again at a known address (a resolution or screen-mode change): read afresh");
        }
        if buffers != self.back_buffers {
            survey::set_back_buffers(buffers.clone());
            self.back_buffers = buffers;
        }
        // (IMPL_ER15, game over) THE VOID: the whole picture is ours: black, and the posed Sora with a depth buffer of our own
        if let Some(void) = crate::void_now() {
            let back: ID3D12Resource = swap.GetBuffer(swap.GetCurrentBackBufferIndex())?;
            return self.draw(&back, D3D12_RESOURCE_STATE_PRESENT, false, None, Some(&void));
        }
        let at_present = SHARED.lock().ok().and_then(|g| g.as_ref().map(|s| s.options.at_present)).unwrap_or(false);
        if !at_present {
            return Ok(());
        }
        let back: ID3D12Resource = swap.GetBuffer(swap.GetCurrentBackBufferIndex())?;
        self.draw(&back, D3D12_RESOURCE_STATE_PRESENT, false, None, None)
    }

    /// Our command list for one picture: Sora and the Keyblade into `back`, tested against the
    /// game's depth. `back` is either the game's HDR scene target between its deferred and its
    /// forward stage (`hdr`: we write scene-linear colour and the game's own tone mapping follows)
    /// or a swap chain buffer with the game's finished picture; `rest` is the state it is in.
    /// `light`: the game's diffuse light accumulation target (readable), for the light probe.
    unsafe fn draw(&mut self, back: &ID3D12Resource, rest: D3D12_RESOURCE_STATES, hdr: bool, sub: Option<&survey::Submission>, void: Option<&Arc<crate::VoidFrame>>) -> windows::core::Result<()> {
        let started = Instant::now();
        // (IMPL_ER53) a colour list recorded with an early depth whose submission never ended in our hook: dropped
        if self.pending.take().is_some() {
            self.tell("an early-depth frame's colour list was never executed: dropped (once)".to_string());
        }
        // (IMPL_ER15, game over) while THE VOID is up nothing of his is drawn into the game`s own picture
        if void.is_none() && crate::void_now().is_some() {
            (self.velo, self.late) = (None, None);
            return Ok(());
        }
        let back_desc = back.GetDesc();
        let (width, height) = (back_desc.Width as u32, back_desc.Height);
        let scene = survey::scene_depth(width, height);
        self.device.GetDeviceRemovedReason()?;
        let completed = self.fence.GetCompletedValue();
        self.retired.retain(|(value, _)| *value > completed);
        // (GPUSAFE) an old outfit, capture or effect target goes once the GPU is past every frame that read it
        crate::outfit::frame_begins(&self.fence, self.fence_value);
        // (IMPL_ER78, r.hybrid_dump) a dump whose frame the GPU has finished: read back and written (on a thread of its own)
        if self.hy.dump.as_ref().is_some_and(|d| self.frame_no > d.frame + 2 && self.slots.get(d.slot).is_none_or(|s| s.busy_until <= completed)) {
            if let Some(dump) = self.hy.dump.take() {
                self.write_hybrid_dump(dump);
            }
        }

        // The host's frames: the pose of one (`r.poselag` frames ago), the camera of another (`r.lag`).
        let (options, input, camera_of): (Options, Option<(Arc<FrameInput>, Instant, u32)>, Option<Arc<FrameInput>>) = {
            let guard = SHARED.lock().unwrap();
            match guard.as_ref() {
                Some(shared) => {
                    let ago = |frames: u32| {
                        let lag = (frames as usize).min(shared.inputs.len().saturating_sub(1));
                        shared.inputs.len().checked_sub(1 + lag).and_then(|k| shared.inputs.get(k)).cloned()
                    };
                    (shared.options.clone(), ago(shared.options.pose_lag), ago(shared.options.lag).map(|(input, ..)| input))
                }
                None => return Ok(()),
            }
        };
        let (options, input, camera_of) = match void {
            Some(v) => {
                let frame = Arc::new(v.input.clone());
                (Options { draw: true, depth: 0, light: false, jitter: false, at_present: true, live_camera: false, fov_horizontal: false, khs: v.khs.clone(), exposure: v.exposure, shadow: 0.0, shadow_sun: 0.0, flat: 0, ghost: 1.0, only: -1, velocity: 0, ..options }, Some((frame.clone(), Instant::now(), 0)), Some(frame))
            }
            None => (crate::vfx::cine_options(options) /* (VFXR cinematic) a sequence's look on our characters' light */, input, camera_of),
        };
        let (Some((input, submitted, submitter)), Some(camera_of)) = (input, camera_of) else { return Ok(()) };
        // (party VF-b) the effect side's chandelier fragments routed to the character pipeline (vfx.rs `routed_forms`: their
        // own material w_so088_mat0, one form per fragment instance) join the frame's forms
        let routed = if void.is_some() { Vec::new() } else { crate::vfx::routed_forms() };
        let routed_count = routed.len();
        let input = if routed.is_empty() {
            input
        } else {
            let mut merged = (*input).clone();
            merged.forms.extend(routed);
            Arc::new(merged)
        };
        let age = submitted.elapsed().as_micros() as u64;
        self.stats.age += age;
        self.stats.age_min = if self.stats.age_max == 0 { age } else { self.stats.age_min.min(age) };
        self.stats.age_max = self.stats.age_max.max(age);
        if submitter == windows::Win32::System::Threading::GetCurrentThreadId() {
            self.stats.same_thread += 1;
        }
        if !options.draw || self.assets.is_none() {
            return Ok(());
        }
        let Some(out_mode) = (if hdr { Some(0.0) } else { output_mode(back_desc.Format) }) else {
            self.tell(format!("the back buffer's format {} is not one we draw into: nothing is drawn", back_desc.Format.0));
            return Ok(());
        };
        self.tell(format!("back buffer {width}x{height} format {} ({} buffers); output transform {out_mode}; drawing {}", back_desc.Format.0, self.back_buffers.len(), if options.at_present { "at Present" } else if hdr { "into the game's HDR scene target right after its deferred stage" } else { "right after the game's submission that finishes the back buffer" }));

        // ---- which depth: the game's own scene depth (found by the survey) or one of ours
        let scene = scene.filter(|s| options.depth == 1 && s.info.width == width as u64 && s.info.height == height).and_then(|s| depth_view_format(s.info.format).map(|f| (s, f)));
        if options.depth == 1 && scene.is_none() {
            // The game did not draw its world into this picture (a loading screen, a movie): he
            // is not drawn over it. (r.depth = 0 draws regardless, over everything.)
            self.stats.no_scene += 1;
            self.why_not = "no scene depth of the target's size is known";
            return Ok(());
        }
        // HARDENING (IMPL_ER4_STATE.md step 0): our barriers name the BEFORE state of the game's
        // resources from what its own barriers told. That is only trusted when it was observed in
        // this very frame and nothing contradicted it; any other frame (a resolution change, a
        // window-mode switch, alt-tab, a cutscene or weather arranging the passes differently) is
        // skipped, and tried again the next frame.
        if let Some((s, _)) = &scene {
            let legacy = sub.is_none() && options.at_present;
            let odd = |state: i32| state == 0 || state & D3D12_RESOURCE_STATE_RENDER_TARGET.0 != 0;
            let why = if !(s.fresh || legacy) {
                Some("the scene depth was not transitioned by the game in this frame")
            } else if sub.is_some_and(|sub| !sub.clean) || (hdr && sub.is_some_and(|sub| !sub.depth_seen)) {
                Some("a barrier of the game contradicted the recorded state of a target, or the depth was not seen in the deferred submission")
            } else if s.info.states.iter().take(2).any(|st| odd(*st)) {
                Some("the scene depth rests in an unexpected state")
            } else if rest.0 & (D3D12_RESOURCE_STATE_RENDER_TARGET.0 | D3D12_RESOURCE_STATE_DEPTH_WRITE.0) != 0 || (hdr && rest.0 == 0) {
                Some("the target rests in an unexpected state")
            } else if back_desc.Dimension != D3D12_RESOURCE_DIMENSION_TEXTURE2D || back_desc.SampleDesc.Count != 1 || (back_desc.Flags & D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET).0 == 0 {
                Some("the target is not a plain render target")
            } else {
                None
            };
            if let Some(why) = why {
                self.stats.skipped += 1;
                self.velo = None;
                self.why_not = why;
                self.tell(format!("frame skipped (it is tried again every frame): {why}"));
                return Ok(());
            }
        }
        let depth_format = scene.as_ref().map_or(DXGI_FORMAT_D32_FLOAT, |(_, f)| *f);
        let key = PsoKey { target: back_desc.Format.0, depth: depth_format.0, front_ccw: options.front_ccw, cull: options.cull, alpha: false };
        self.wanted = Some(key);
        if !self.pso_sets.contains_key(&key) {
            self.why_not = "the pipeline states for the target's formats are still being made";
            self.stats.waiting += 1;
            return Ok(());
        }
        let Some(slot_index) = (0..self.slots.len()).find(|k| self.slots[*k].busy_until <= completed) else {
            self.stats.busy += 1;
            return Ok(());
        };
        if scene.is_none() && self.own_depth.as_ref().map(|(_, w, h)| (*w, *h)) != Some((width, height)) {
            if let Some((old, ..)) = self.own_depth.take() {
                self.retired.push((self.fence_value, old));
            }
            let clear = D3D12_CLEAR_VALUE { Format: DXGI_FORMAT_D32_FLOAT, Anonymous: D3D12_CLEAR_VALUE_0 { DepthStencil: D3D12_DEPTH_STENCIL_VALUE { Depth: 0.0, Stencil: 0 } } };
            let depth = committed(&self.device, D3D12_HEAP_TYPE_DEFAULT, &texture_desc(width, height, 1, 1, DXGI_FORMAT_D32_FLOAT, D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL), D3D12_RESOURCE_STATE_DEPTH_WRITE, Some(&clear))?;
            self.device.CreateDepthStencilView(&depth, None, self.dsv_heap.GetCPUDescriptorHandleForHeapStart());
            self.own_depth = Some((depth, width, height));
            rlog!("render: own depth buffer {width}x{height}");
        }

        // ---- the game's light: the kit, once; the read-back of this slot's last frame
        let game_on = hdr && (options.light || options.jitter) && sub.is_some() && !self.game_failed;
        if game_on && self.game.is_none() {
            match self.make_game_kit() {
                Ok(kit) => self.game = Some(kit),
                Err(e) => {
                    self.game_failed = true;
                    self.tell(format!("the game's light cannot be read ({e}): the stand-in rig is used"));
                }
            }
        }
        let game_on = game_on && self.game.is_some();
        if !game_on {
            self.light = None;
        }
        let self_prev_origin = self.prev_origin;
        self.prev_origin = Some(camera_of.origin);
        self.velo = None;
        self.late = None;
        self.frame_no += 1;
        // The late stage is left to the late pass only when one ran in the frame before (the frame shape is known).
        let late_stage = crate::vfx::options().late && self.late_seen + 1 == self.frame_no;
        // (IMPL_ER53) THE EARLY DEPTH of this frame (early_pass): the scene depth's planes at the split must rest in a
        // read state (not written, not a target), the set must have the early-depth states, and a snapshot of the scene
        // depth is kept for our own passes that read the world without him (made once per scene depth).
        let early_states = self.early_now.filter(|_| hdr && void.is_none() && sub.is_some()).filter(|planes| planes.iter().all(|s| *s != 0 && s & (D3D12_RESOURCE_STATE_RENDER_TARGET.0 | D3D12_RESOURCE_STATE_DEPTH_WRITE.0) == 0));
        let early_at = match (&scene, early_states, self.assets.as_ref(), self.pso_sets.get(&key)) {
            (Some(_), Some(_), Some(assets), Some(set)) => early_base(set.len(), assets.library.slots.len()),
            _ => None,
        };
        let early_states = early_states.filter(|_| early_at.is_some());
        if let (Some(_), Some((scene_now, _))) = (early_states, &scene) {
            let pointer = scene_now.resource.as_raw() as usize;
            if self.snapshot.as_ref().map(|s| s.1) != Some(pointer) {
                if let Some((old, ..)) = self.snapshot.take() {
                    self.retired.push((self.fence_value, old));
                }
                let desc = scene_now.resource.GetDesc();
                match committed(&self.device, D3D12_HEAP_TYPE_DEFAULT, &desc, D3D12_RESOURCE_STATE_COPY_DEST, None) {
                    Ok(copy) => {
                        rlog!("render: (IMPL_ER53) the scene depth's snapshot for our light pass and contact shadow: {}x{} format {}", desc.Width, desc.Height, desc.Format.0);
                        self.snapshot = Some((copy, pointer, true));
                    }
                    Err(e) => self.tell(format!("the scene depth's snapshot cannot be made ({e}): our light pass and contact shadow read the depth with him in it")),
                }
            }
        }
        // (RSF2 round 6) the scene depth WITH the KH characters (copied at the end of the early list) for the hybrid's sun lookup
        if let (true, Some(_), Some((scene_now, _))) = (sun_read_wanted(), early_states, &scene) {
            let pointer = scene_now.resource.as_raw() as usize;
            if self.kh_depth.as_ref().map(|s| s.1) != Some(pointer) {
                if let Some((old, _)) = self.kh_depth.take() {
                    self.retired.push((self.fence_value, old));
                }
                let desc = scene_now.resource.GetDesc();
                match committed(&self.device, D3D12_HEAP_TYPE_DEFAULT, &desc, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE | D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE, None) {
                    Ok(copy) => {
                        rlog!("render: (RSF2 round 6) the scene depth with the KH characters, for the hybrid's sun from the game's cascades: {}x{} format {}", desc.Width, desc.Height, desc.Format.0);
                        self.kh_depth = Some((copy, pointer));
                    }
                    Err(e) => self.tell(format!("(RSF2 round 6) the depth copy for the sun lookup cannot be made ({e}): the round-5 mean")),
                }
            }
        }
        let early_reads: Option<ID3D12Resource> = early_states.and(self.snapshot.as_ref().filter(|s| scene.as_ref().is_some_and(|(sc, _)| sc.resource.as_raw() as usize == s.1)).map(|s| s.0.clone()));
        // (IMPL_ER65) the late re-composite needs the snapshot (the depth without him): its copies and views, made again when
        // the target, the scene depth or the snapshot change
        let recomp_now = LATE_SORA.load(Ordering::Relaxed) && hdr && void.is_none() && early_reads.is_some();
        if let (true, Some((scene_now, view_format)), Some(snap)) = (recomp_now, &scene, early_reads.as_ref()) {
            let (target, depth, snapshot) = (back.as_raw() as usize, scene_now.resource.as_raw() as usize, snap.as_raw() as usize);
            if self.late_kit.as_ref().map(|k| (k.target, k.depth, k.snapshot)) != Some((target, depth, snapshot)) {
                if let Some(old) = self.late_kit.take() {
                    self.retired.push((self.fence_value, old.saved));
                    self.retired.push((self.fence_value, old.him));
                }
                let depth_view = match scene_now.info.format {
                    DXGI_FORMAT_R32G8X24_TYPELESS | DXGI_FORMAT_D32_FLOAT_S8X24_UINT => Some(DXGI_FORMAT_R32_FLOAT_X8X24_TYPELESS),
                    DXGI_FORMAT_R24G8_TYPELESS | DXGI_FORMAT_D24_UNORM_S8_UINT => Some(DXGI_FORMAT_R24_UNORM_X8_TYPELESS),
                    DXGI_FORMAT_R32_TYPELESS => Some(DXGI_FORMAT_R32_FLOAT),
                    _ => None,
                };
                let _ = view_format;
                let made = (|| -> windows::core::Result<Option<LateKit>> {
                    let Some(depth_view) = depth_view else { return Ok(None) };
                    let mut colour_desc = back_desc;
                    colour_desc.Flags = D3D12_RESOURCE_FLAG_NONE;
                    let saved = committed(&self.device, D3D12_HEAP_TYPE_DEFAULT, &colour_desc, D3D12_RESOURCE_STATE_COPY_DEST, None)?;
                    let him = committed(&self.device, D3D12_HEAP_TYPE_DEFAULT, &scene_now.resource.GetDesc(), D3D12_RESOURCE_STATE_COPY_DEST, None)?;
                    let heap: ID3D12DescriptorHeap = self.device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, NumDescriptors: SLOT_TEXTURES as u32, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, NodeMask: 0 })?;
                    let size = self.device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV) as usize;
                    let at = |k: usize| D3D12_CPU_DESCRIPTOR_HANDLE { ptr: heap.GetCPUDescriptorHandleForHeapStart().ptr + k * size };
                    let dview = D3D12_SHADER_RESOURCE_VIEW_DESC { Format: depth_view, ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D, Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING, Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 { Texture2D: D3D12_TEX2D_SRV { MostDetailedMip: 0, MipLevels: 1, PlaneSlice: 0, ResourceMinLODClamp: 0.0 } } };
                    for k in 0..SLOT_TEXTURES {
                        self.device.CreateShaderResourceView(&saved, None, at(k));
                    }
                    self.device.CreateShaderResourceView(&him, Some(&dview), at(1));
                    self.device.CreateShaderResourceView(snap, Some(&dview), at(2));
                    self.device.CreateShaderResourceView(&scene_now.resource, Some(&dview), at(3));
                    Ok(Some(LateKit { saved, him, heap, target, depth, snapshot, fresh: true, pipeline: None, pipeline_failed: false, pipeline2: None, pipeline2_failed: false }))
                })();
                match made {
                    Ok(kit) => {
                        if kit.is_some() {
                            rlog!("render: (IMPL_ER65) the late re-composite's copies are made ({}x{})", back_desc.Width, back_desc.Height);
                        }
                        self.late_kit = kit;
                    }
                    Err(e) => self.tell(format!("the late re-composite's copies cannot be made ({e}): no late re-composite")),
                }
            }
        }
        let recomp_now = recomp_now && self.late_kit.as_ref().is_some_and(|k| k.target == back.as_raw() as usize);
        // (IMPL_ER80) THE GLOW MASK of this frame: the game's glow source (the composite's second target) gets 0 under his
        // opaque / masked pixels, right after his colour draw (the post chain's glow is made from it later). Every check per
        // frame (glow_check); any failure = today's path for the frame, counted with its reason for the log.
        let glow_plan: Option<(usize, i32, PsoKey)> = if GLOW_MASK.load(Ordering::Relaxed) && hdr && void.is_none() && scene.is_some() {
            match sub.and_then(|s| s.glow) {
                None => {
                    self.glow.skipped += 1;
                    self.glow.reason = "the deferred composite drew no second (glow) target in this frame";
                    None
                }
                Some((g, state)) => {
                    let format = survey::format_of(g).map_or(0, |f| f.0);
                    match glow_check(g, state, format, back.as_raw() as usize, survey::usable_now(g, width, height)) {
                        Some(why) => {
                            self.glow.skipped += 1;
                            self.glow.reason = why;
                            None
                        }
                        None => {
                            let gkey = PsoKey { target: format, ..key };
                            self.glow.wanted = Some(gkey);
                            let slots_now = self.assets.as_ref().map_or(0, |a| a.library.slots.len());
                            if self.glow.sets.get(&gkey).is_some_and(|set| set.len() == slots_now) {
                                Some((g, state, gkey))
                            } else {
                                self.glow.skipped += 1;
                                self.glow.reason = if self.glow.failed.contains(&gkey) { "its pipeline states could not be made" } else { "its pipeline states are still being made" };
                                None
                            }
                        }
                    }
                }
            }
        } else {
            None
        };
        let mut glow_draws: Vec<(usize, usize, u32, u32, i32, u64, u64, u64)> = Vec::new();
        // (IMPL_ER80b) his mask for the game's light-shaft composite is made this frame (a twin of it exists, r.shaft_mask)
        let shaft_on = hdr && void.is_none() && scene.is_some() && crate::shaft::wanted();
        let Core { device, queue, root, sampler_heap, sampler_index, rtv_heap, dsv_heap, dsv_size, fence, fence_value, slots, assets, pso_sets, stats, queries, blob_depth, blob_mask, game, light: light_now, rig_light, env_now, calibration, prev_skin, velo, late, cast, cast_at, pending, snapshot, early_drawn, sun_kit, sun_now, kh_depth, kh_maps, kh_maps_vp, stencil_now, stencil_seen, frame_no, late_kit, hy, glow, #[cfg(feature = "party")] party, .. } = self;
        let assets = assets.as_mut().unwrap();
        let pipelines = &pso_sets[&key];
        let slot = &mut slots[slot_index];
        // The GPU time of this slot's previous frame (it is finished: the slot is free).
        crate::passtime::collect(slot_index); // (HAIRPERF) and its sections'
        if let (true, Some((_, results, frequency))) = (slot.timed, queries.as_ref()) {
            let range = D3D12_RANGE { Begin: slot_index * 16, End: slot_index * 16 + 16 };
            let mut at: *mut c_void = std::ptr::null_mut();
            if results.Map(0, Some(&range), Some(&mut at)).is_ok() {
                let stamps = std::slice::from_raw_parts((at as *const u8).add(slot_index * 16) as *const u64, 2);
                let micros = stamps[1].saturating_sub(stamps[0]).saturating_mul(1_000_000) / *frequency;
                results.Unmap(0, Some(&D3D12_RANGE { Begin: 0, End: 0 }));
                if micros < 1_000_000 {
                    stats.gpu += micros;
                    stats.gpu_max = stats.gpu_max.max(micros);
                    stats.gpu_frames += 1;
                }
            }
            slot.timed = false;
        }
        if let (true, Some(readback), Some(kit)) = (slot.read_pending, slot.readback.as_ref(), game.as_ref()) {
            let mut at: *mut c_void = std::ptr::null_mut();
            if readback.Map(0, Some(&D3D12_RANGE { Begin: 0, End: kit.bytes as usize }), Some(&mut at)).is_ok() {
                let mut words = vec![0.0f32; crate::light::WIDTH * crate::light::HEIGHT * 4];
                for row in 0..crate::light::HEIGHT {
                    let line = (at as *const u8).add(kit.footprint.Offset as usize + row * kit.footprint.Footprint.RowPitch as usize) as *const f32;
                    std::ptr::copy_nonoverlapping(line, words.as_mut_ptr().add(row * crate::light::WIDTH * 4), crate::light::WIDTH * 4);
                }
                // (ER party I) the friends' own probe rows (after light.rs's)
                #[cfg(feature = "party")]
                let party_rows = {
                    let mut v = vec![0.0f32; crate::light::WIDTH * party::probe::ROWS * 4];
                    for k in 0..party::probe::ROWS {
                        let line = (at as *const u8).add(kit.footprint.Offset as usize + (crate::light::HEIGHT + k) * kit.footprint.Footprint.RowPitch as usize) as *const f32;
                        std::ptr::copy_nonoverlapping(line, v.as_mut_ptr().add(k * crate::light::WIDTH * 4), crate::light::WIDTH * 4);
                    }
                    v
                };
                readback.Unmap(0, Some(&D3D12_RANGE { Begin: 0, End: 0 }));
                if game_on {
                    // (ER party I) each friend's probe of that frame (gfx/party_probe.rs)
                    #[cfg(feature = "party")]
                    party::probe::read(&party_rows, (width as f32, height as f32));
                    let mut now = crate::light::parse(&words, (width as f32, height as f32));
                    // (ER88) the game's previous view-projection against the frame before's (taa.rs, logged)
                    crate::taa::check(&words, (width as f32, height as f32), *frame_no);
                    // (IMPL_ER86) the world probe's taps of that frame into their memory (stable.rs TapMemory)
                    crate::stable::read_taps(&words, input.body.as_ref().map(|b| b.component_to_world.translation));
                    // (his feet were not in the picture that frame: the probes of the last frame they were)
                    // (IMPL_ER86) with the steady light the raw frame is kept here; stable.rs eases it below (after the harness hook)
                    if let Some(before) = light_now.as_ref().filter(|b| b.probed()).filter(|_| !crate::stable::settings().on) {
                        if !now.probed() {
                            // (his feet were not on a surface in that picture: the last probes that were)
                            (now.scene_px, now.direct_px, now.albedo_px, now.mask, now.taps) = (before.scene_px, before.direct_px, before.albedo_px, before.mask, before.taps);
                        } else {
                            // The probes are a few pixels of a moving picture: eased over about a
                            // fifth of a second so that he does not flicker with the grass under him.
                            let ease = 0.08;
                            now.scene_px = before.scene_px.lerp(now.scene_px, ease);
                            now.direct_px = before.direct_px.lerp(now.direct_px, ease);
                            now.albedo_px = before.albedo_px.lerp(now.albedo_px, ease);
                            now.mask = before.mask.lerp(now.mask, ease);
                        }
                        // (the environment taps: eased over about half a second)
                        if before.env_taps >= 1.0 {
                            now.env_px = if now.env_taps >= 1.0 { before.env_px.lerp(now.env_px, 0.04) } else { before.env_px };
                            now.env_taps = now.env_taps.max(before.env_taps);
                        }
                    }
                    // (for the log only: the rig uses the measured constant; see light.rs)
                    *calibration = crate::light::calibration(&now).unwrap_or(0.0);
                    *light_now = Some(now);
                }
            }
            slot.read_pending = false;
        }
        // (IMPL_ER56) the stencil taps of this slot's last early depth
        if let (true, Some((rb, footprint))) = (slot.stencil_pending, slot.stencil_rb.as_ref()) {
            let mut at: *mut c_void = std::ptr::null_mut();
            let bytes = footprint.Footprint.RowPitch as usize * (footprint.Footprint.Height as usize).saturating_sub(1) + footprint.Footprint.Width as usize * stencil_texel(footprint.Footprint.Format).0;
            let mapped = rb.Map(0, Some(&D3D12_RANGE { Begin: 0, End: bytes }), Some(&mut at));
            if mapped.is_ok() {
                let (texel, offset) = stencil_texel(footprint.Footprint.Format);
                let values: Vec<u8> = slot.stencil_taps.iter().filter(|(x, y)| *x < footprint.Footprint.Width && *y < footprint.Footprint.Height).map(|(x, y)| *(at as *const u8).add(*y as usize * footprint.Footprint.RowPitch as usize + *x as usize * texel + offset)).collect();
                rb.Unmap(0, Some(&D3D12_RANGE { Begin: 0, End: 0 }));
                if let Some(v) = ground_stencil(&values) {
                    *stencil_now = Some(v);
                }
                *stencil_seen = values;
            }
            slot.stencil_pending = false;
        }

        // ---- the frame's constants
        let mut camera = Some(()).filter(|_| options.live_camera && !options.at_present).and_then(|_| crate::camera_now()).filter(|c| c.right.is_finite() && c.position.is_finite() && c.fov > 0.01 && c.aspect > 0.1 && c.near > 0.0 && c.far > c.near).unwrap_or(camera_of.camera);
        if options.fov_horizontal {
            camera.fov = 2.0 * ((camera.fov * 0.5).tan() / camera.aspect).atan();
        }
        let clip = math::clip_from_game(&camera, camera_of.origin);
        let camera_game = math::game_pos_of_host(camera.position, camera_of.origin);
        let tan_y = (camera.fov * 0.5).tan();
        // (ER85, the offline harness only: cgfield.rs) a frame of the game's light as the live log printed it: the field's path
        let test_light = crate::cgfield::test_light();
        if test_light.is_some() {
            *light_now = test_light;
        }
        let game_on = game_on || test_light.is_some();
        let options_light = options.light || test_light.is_some();
        // (IMPL_ER86) THE STEADY LIGHT (stable.rs; `r.light_stable = 0`: the light as read, eased as before at the read-back):
        // the probes eased by his speed, the sun and the scene constants kept through frames without them, the exposure eased
        // in the log domain; everything below takes it in place of the frame as read
        let steady_light: Option<crate::light::GameLight> = if crate::stable::settings().on { crate::stable::global_step(light_now.as_ref().filter(|_| game_on), input.body.as_ref().map(|b| b.component_to_world.translation)) } else { *light_now };
        let light_now = &steady_light;
        // The light: the game's (read back a frame or two ago) or the stand-in rig.
        let mut rig = kh_look::stand_in::rig();
        if let Some(v) = void {
            // (the void is lit by the effect`s point lights alone: no key light, no ambient, no reflection of a sky)
            (rig.key, rig.ambient, rig.shadows) = (Vec3::ZERO, Vec3::ZERO, false);
            rig.points = v.lights.clone();
        }
        let fallback = Vec3::new(kh_look::stand_in::KEY_DIR.y, kh_look::stand_in::KEY_DIR.z, kh_look::stand_in::KEY_DIR.x).normalize();
        let from_game = light_now.as_ref().filter(|_| options_light && game_on).map(|l| crate::light::rig_for(l, fallback, options.key * crate::light::CALIBRATION_DEFAULT, options.ambient, crate::light::open_level_now()));
        // (ER85) an outfit drawn with its world's own set: the game's light reshaped as KH3 lights its characters (cgfield.rs)
        let from_game = from_game.map(|r| if crate::cgfield::active() { crate::cgfield::shape_for_him(r, input.body.as_ref().map(|b| b.component_to_world.rotation * Vec3::Y), light_now.as_ref().map_or(1.0, |l| l.mask.w), &crate::cgfield::shape_now()) } else { r });
        *rig_light = from_game;
        let (key_dir, key_tint, out_scale, ambient_up) = match from_game {
            Some(r) => {
                (rig.key, rig.ambient) = (r.key, r.ambient);
                (math::game_dir_of_host(r.key_dir), 1.0, r.out_scale, r.ambient_up)
            }
            None => {
                rig.ambient *= options.ambient;
                (kh_look::stand_in::KEY_DIR.normalize(), options.key, if hdr { options.hdr_scale } else { 1.0 }, Vec3::ZERO)
            }
        };
        // (IMPL_ER39) THE LIGHT PASS's lights of this frame (pointlight.rs): the effects' particle lights and the hero
        // light behind its darkness gate (the game's light at his feet, read before any of ours); his AMBIENT takes a
        // share of their light at his chest (no direction: he is not lamp-lit; the pass itself is drawn before him).
        let mut sora_lights = vec![0.0f32; crate::pointlight::SORA_BLOCK_FLOATS];
        #[cfg(feature = "party")]
        let mut party_lights: Vec<crate::pointlight::PassLight> = Vec::new();
        let lights_now = if hdr && void.is_none() {
            let exposure = light_now.as_ref().filter(|l| l.scene && game_on).map(|l| l.exposure);
            let to_target = crate::vfx::effect_scale(exposure, out_scale);
            let level = light_now.as_ref().filter(|_| game_on && options_light).and_then(crate::pointlight::white_level);
            // (IMPL_ER86) the darkness gate follows a reading only once it has held `hero.gate_hold_s` (stable.rs `Gate`)
            let gate_reading = if crate::stable::settings().on {
                let s = crate::pointlight::settings();
                crate::stable::gate_level(level, s.hero_dark_on, s.hero_dark_off)
            } else {
                level
            };
            let feet = input.body.as_ref().map(|b| b.component_to_world.translation);
            // (IMPL_ER44) the Keyblade light: a point on the blade (from the grip along it) and the weapon's opacity
            let blade = input.weapon.as_ref().filter(|w| w.opacity > 0.0).and_then(|w| crate::shadowcaps::blade_point(w, crate::pointlight::settings().hero_blade).map(|p| (p, w.opacity)));
            // (ER party S4, U5) the hero light at the friend leader who carries it (his weapon tip) when Sora does not
            #[cfg(feature = "party")]
            let (hero_feet, blade) = match crate::party::hero_carrier_now() {
                Some(h) => (Some(h.feet), h.tip),
                None => (feet, blade),
            };
            #[cfg(not(feature = "party"))]
            let hero_feet = feet;
            let planned = crate::pointlight::frame(gate_reading, to_target * crate::vfx::options().hdr, to_target * crate::light::reference_white(), hero_feet, blade);
            // (IMPL_ER44) with the pack's *_pl shaders the lights are point lights on his materials (the block of the
            // frame constants below) and only a small share remains in his ambient; without them the ambient lift as before
            // (IMPL_ER64) THE WORLD'S LIGHTS ON HIM: one point light fitted to the game's direct light on the ground around
            // his feet (the ring of probe2), for his materials only (the game lit the world)
            let world = match crate::pointlight::test_ring() {
                Some((ring, at)) => {
                    let mut g = crate::light::GameLight::default();
                    for k in 0..crate::light::RING.min(ring.len()).min(at.len()) {
                        (g.ring[k], g.ring_at[k]) = (ring[k], at[k]);
                    }
                    crate::pointlight::world_light_now(Some(&g), feet, level)
                }
                None => crate::pointlight::world_light_now(light_now.as_ref().filter(|_| game_on), feet, level),
            };
            let mut for_him = planned.lights.clone();
            for_him.extend(world);
            // (ER party S4) the same lights for the friends' own point lights (gfx/party.rs `prepare`)
            #[cfg(feature = "party")]
            party_lights.clone_from(&for_him);
            // (IMPL_ER86) the placed hero light toward a point of HIS (stable.rs `hero_toward`, `hero.anchor`), not the camera
            let (toward, toward_cm) = match feet {
                Some(f) => crate::stable::hero_toward(camera_game, f, input.body.as_ref().map(|b| b.component_to_world.rotation * Vec3::Y), crate::pointlight::settings().hero_sora_toward),
                None => (camera_game, crate::pointlight::settings().hero_sora_toward),
            };
            let (block, lift) = crate::pointlight::for_sora_toward(&for_him, feet, toward, toward_cm, out_scale);
            if out_scale > 0.0 {
                rig.ambient += lift / out_scale;
            }
            sora_lights = block;
            planned.lights
        } else {
            crate::pointlight::for_sora(&[], None, camera_game, out_scale);
            Vec::new()
        };
        // THE REFLECTION ENVIRONMENT'S LEVEL: what the game's picture shows along its top (light.rs).
        // (IMPL_ER86) with the steady light: the game's sky constants and the world probes, not the picture's top (stable.rs `env_view`)
        let env_light = light_now.as_ref().map(|l| if crate::stable::settings().on { crate::stable::env_view(l, &crate::stable::settings()) } else { *l });
        let env_level = match (from_game, env_light.as_ref()) {
            (Some(r), Some(l)) => crate::light::env_level(l, r.out_scale, options.env),
            _ => 1.0,
        };
        let env_level = if void.is_some() { 0.0 } else { env_level };
        *env_now = env_level;
        // ITS HUE: the game's sky of the hour and the light at his feet (warm by a fire); `r.envtint = 0`: the cube's own.
        let env_tint = match (from_game, env_light.as_ref()) {
            (Some(r), Some(l)) if options.env > 0.0 => crate::light::env_tint(l, r.ambient),
            _ => Vec3::ONE,
        };
        // The indirect term's rows (as kh_look::Rig::sh, with a coloured upward part).
        let mut sh = rig.sh();
        for c in 0..3 {
            sh[c].z = ambient_up[c] / 1.023_328;
        }
        // HIS SHADOW from the directional light (the contact shadow's pass): how much of the light
        // on lit, level ground is that light's = how dark his shadow is there.
        let sun_shadow = match (from_game, light_now.as_ref().and_then(|l| l.sun)) {
            (Some(r), Some((dir, colour))) => {
                let luma = |c: Vec3| c.dot(Vec3::new(0.2126, 0.7152, 0.0722));
                let sun = luma(colour) * crate::light::CALIBRATION_DEFAULT * options.key * std::f32::consts::FRAC_1_PI * dir.y.max(0.0);
                let other = luma(r.ambient + r.ambient_up) * r.out_scale;
                let modelled = if sun > 0.0 { (sun / (sun + other)).clamp(0.0, 1.0) } else { 0.0 };
                // (IMPL_ER9: the user saw no shadow: the modelled share came out far too small) MEASURED INSTEAD: around
                // his feet the game's own targets say how much of the ground's light is its direct light (the direct
                // light accumulation against the composed scene colour, both read without him): that share is what
                // his shadow takes away. In the shade it is ~0 (no shadow), at night the moon's small share.
                let measured = light_now.as_ref().filter(|l| l.taps >= 1.0 && luma(l.scene_px) > 1.0e-6).map(|l| (luma(l.direct_px) / luma(l.scene_px)).clamp(0.0, 0.85));
                let _ = dir;
                // (IMPL_ER17) where his real shadow was drawn into the game's cascades a moment ago the stand-in is not needed
                // (RSF2 round 15) the stand-in stays with the real cast: in game on 2.7.2.0 the game's light ignores what we draw into its sun
                // atlas (r.shadow_dbg=99 flood: nothing), so the cast is only read by our own lookup (the KH caster maps)
                { let _ = &cast_at; measured.unwrap_or(modelled) * options.shadow_sun }
            }
            _ => 0.0,
        };
        // (for `r.flat`: what a white diffuse surface shows facing the key light / away from it)
        let (lit_key, lit_ambient) = (rig.key * key_tint / std::f32::consts::PI * out_scale, rig.ambient * out_scale);
        let (depth_a, depth_b) = math::depth_to_view(camera.near, camera.far);
        // The light probes: around the pixel of his feet, when they are in front of us.
        let feet_clip = input.body.as_ref().map(|b| clip * b.component_to_world.translation.extend(1.0));
        // (when his feet are outside the picture the taps are taken at its edge nearest to them)
        let probe_on = game_on && feet_clip.is_some_and(|f| f.w > 0.3);
        let probe_at = match (probe_on, feet_clip) {
            (true, Some(f)) => Vec4::new(((f.x / f.w * 0.5 + 0.5) * width as f32).clamp(0.04 * width as f32, 0.96 * width as f32), ((0.5 - f.y / f.w * 0.5) * height as f32).clamp(0.04 * height as f32, 0.96 * height as f32), (0.35 / (f.w * tan_y) * height as f32 * 0.5).clamp(2.0, 200.0), 1.0),
            _ => Vec4::ZERO,
        };
        // (IMPL_ER86) THE WORLD PROBE (probe3.ps.dxil, stable.rs): w = 2, z = the radius (cm) of its taps around his feet
        let probe_at = {
            let s = crate::stable::settings();
            if probe_at.w == 1.0 && s.on && s.probe_world && game.as_ref().is_some_and(|k| k.world) { Vec4::new(probe_at.x, probe_at.y, s.probe_radius.clamp(20.0, 400.0), 2.0) } else { probe_at }
        };
        // (IMPL_ER78) THE HYBRID THIS FRAME: drawn into the game's G-buffer and lit by the game only when EVERY check passes
        // (shaders, pipeline states, the early depth, the five G-buffer targets by format, size, state and this frame's
        // record, the light target by format and size, the game's light constants); else today's light for this frame.
        let hy_settings = crate::hybrid::settings();
        // (HY) the capture's objects for this size (made once; again when the size changes; not made: no hybrid)
        let hy_cap_ready: Result<(), String> = if hy_settings.on && crate::hybrid::SHADERS.load(Ordering::Relaxed) && hdr && void.is_none() { ensure_capture(device, root, hy, width, height) } else { Err(String::new()) };
        let hy_plan: Result<([crate::hybrid::GbTarget; crate::hybrid::ROLES], usize, usize, usize), &'static str> = (|| {
            if !hy_settings.on {
                return Err("r.hybrid = 0");
            }
            if !crate::hybrid::SHADERS.load(Ordering::Relaxed) {
                return Err("the pack's *_hy3 / *_gb3 shaders are not in use");
            }
            if void.is_some() || !hdr {
                return Err("not drawn into the game's scene (game over, or another arrangement)");
            }
            if early_states.is_none() || early_at.is_none() {
                return Err("no early depth this frame (r.early_depth = 0, or the deferred stage was not of the known shape)");
            }
            let base = gbuffer_base(pipelines.len(), assets.library.slots.len()).ok_or("no G-buffer pipeline states (none made for this target, or they failed)")?;
            let sub = sub.ok_or("no deferred submission")?;
            let (scene_now, _) = scene.as_ref().ok_or("no scene depth")?;
            if sub.gbuffer_count != crate::hybrid::ROLES {
                return Err("the game's last G-buffer list did not have five targets");
            }
            let ordered = crate::hybrid::order(&sub.gbuffer, scene_now.info.width, scene_now.info.height)?;
            for t in &ordered {
                if !survey::usable_now(t.resource, width, height) || survey::format_of(t.resource).is_none_or(|(f, mips, layers)| f != t.format || mips != 1 || layers != 1) {
                    return Err("a G-buffer target was not seen whole in this frame");
                }
            }
            let light = sub.light2.filter(|l| survey::usable_now(*l, width, height)).ok_or("the game's diffuse light target was not found readable")?;
            if !survey::format_of(light).is_some_and(|(f, ..)| crate::hybrid::light_ok(f, width as u64, height, (width as u64, height))) {
                return Err("the game's diffuse light target is not R11G11B10 of the scene's size");
            }
            let target = back.as_raw() as usize;
            if ordered.iter().any(|t| t.resource == light || t.resource == target) || light == target {
                return Err("the G-buffer, light and scene targets overlap");
            }
            if from_game.is_none() && !crate::hybrid::TEST.load(Ordering::Relaxed) {
                return Err("the game's light constants were not read this frame (r.light)");
            }
            // (HY) what the capture reads after the deferred stage must REST READABLE there (the game's own barriers this frame,
            // nothing contradicted): the light target (survey: readable), the G-buffer's normal, parameters and colour
            for t in &ordered[..crate::hybrid::WRITTEN] {
                if !survey::states_now(t.resource).is_some_and(|s| crate::hybrid::readable(s[0]) && crate::hybrid::readable(s[1])) {
                    return Err("a G-buffer target does not rest readable after the deferred stage");
                }
            }
            if hy_cap_ready.is_err() {
                return Err("the capture of the game's light could not be made (materials/er/hycap.ps.dxil? see the log)");
            }
            let mask = sub.mask.filter(|m| survey::usable_now(*m, width, height) && *m != light && *m != target).unwrap_or(0);
            // (HY, for the log: the states the game left, logged once when they change)
            let states = ordered.iter().map(|t| survey::state_text(t.state)).collect::<Vec<_>>().join(" / ");
            if hy.told != states {
                rlog!("render: (HY) the hybrid's G-buffer pass: the game's G-buffer at the split rests {states} (normal / params / colour / sss / emissive; the pass writes the first three, from and back to that state)");
                hy.told = states;
            }
            Ok((ordered, light, mask, base))
        })();
        let hy_on = hy_plan.is_ok();
        match &hy_plan {
            Ok(_) => {
                crate::hybrid::FRAMES_ON.fetch_add(1, Ordering::Relaxed);
            }
            Err(why) if hy_settings.on => crate::hybrid::fell_back(why),
            Err(_) => {}
        }
        let hy_fog = light_now.as_ref().map_or(Vec3::ZERO, |l| l.fog[0]);
        // (HY) the sun for his own shadow and KH3's sun specular: the game's directional light (game space; its light per unit N.L
        // on a white surface in scene units: Col_0 x the calibration / pi, as the rig's key before its shadow); the own sets'
        // lift (cgfield.rs) on the game's light as on the rig's
        let hy_sun = light_now.as_ref().filter(|_| game_on && options_light).and_then(|l| l.sun).map(|(dir, colour)| (math::game_dir_of_host(dir), colour * crate::light::CALIBRATION_DEFAULT * options.key * std::f32::consts::FRAC_1_PI));
        let hy_frame = crate::hybrid::FrameIn { white: crate::light::reference_white(), k: 1.0 / out_scale.max(1.0e-9), fog: hy_fog, sun: hy_sun, own_set: crate::cgfield::active(), lift: crate::cgfield::lift_now() };
        let hy_block = crate::hybrid::block(&hy_settings, hy_on, &hy_frame);
        let mut frame: Vec<f32> = Vec::with_capacity(64);
        frame.extend_from_slice(&clip.to_cols_array());
        for v in [
            camera_game.extend(input.clock),
            math::game_dir_of_host(camera.right).extend(0.0),
            math::game_dir_of_host(camera.up).extend(0.0),
            math::game_dir_of_host(camera.forward).extend(0.0),
            Vec4::new(width as f32, height as f32, 1.0 / width as f32, 1.0 / height as f32),
            key_dir.extend(0.0),
            Vec4::new(key_tint, key_tint, key_tint, 0.0),
            Vec4::new(tan_y * camera.aspect, tan_y, camera.near, camera.far),
            // (x: IMPL_ER65 the late re-composite's distance (m) behind which what lies behind him counts as far field; 0 = all)
            Vec4::new(f32::from_bits(LATE_FAR.load(Ordering::Relaxed)), options.bias, depth_a, depth_b),
            // (w: the sign of his motion vectors)
            Vec4::new(1.0, 1.0, options.ghost, if options.velocity < 0 { -1.0 } else { 1.0 }),
            // (out_mode.w = 1: a masked material's dither coverage becomes alpha)
            Vec4::new(out_mode, 0.0, options.flat as f32, 1.0),
            // The contact shadow: at his feet, 45 cm wide, gone when he is 2.5 m above a surface.
            input.body.as_ref().map_or(Vec4::ZERO, |b| b.component_to_world.translation.extend(45.0)),
            // (z: his height (cm) for the thrown shadow, w: that shadow's strength)
            Vec4::new(options.shadow, 250.0, 150.0, sun_shadow),
            // The light probe: around the pixel of his feet (filled in below when it is on).
            probe_at,
            // What the shaded colour is multiplied with; w: 1 = the game's own view-projection is used.
            Vec4::new(out_scale, out_scale, out_scale, if game_on && options.jitter { 1.0 } else { 0.0 }),
            lit_key.extend(0.0),
            lit_ambient.extend(0.0),
            (ambient_up * out_scale).extend(0.0),
        ] {
            frame.extend_from_slice(&v.to_array());
        }
        frame.extend_from_slice(&math::host_from_game(camera_of.origin).to_cols_array());
        // (ER88) a frame ago: with the own-record velocity state, R x (game -> host) flagged (taa.rs); else as before
        let own_route = options.velocity != 0 && crate::taa::velocity_state(key_bits(key)).is_some();
        frame.extend_from_slice(&crate::taa::prev_host(math::host_from_game(self_prev_origin.unwrap_or(camera_of.origin)), &camera, camera_of.origin, *frame_no, own_route).to_cols_array());
        // (IMPL_ER9) HIS BODY FOR THE THROWN SHADOW: capsules along his main bones (from this frame's pose) and the
        // Keyblade, game space (cm): 16 x (end a + radius, end b + 1 when present). The shader's `er.caps`.
        // (IMPL_ER78b) the same capsules bound the G-buffer pass's scissor box (`hy_caps`)
        let mut hy_caps: Vec<(Vec3, Vec3, f32)> = Vec::new();
        {
            let mut caps: Vec<(Vec3, Vec3, f32)> = Vec::with_capacity(16);
            if let Some(body) = &input.body {
                let at = |name: &str| -> Option<Vec3> {
                    let bone = *assets.bones.get(name)?;
                    Some(body.component.get(bone)?.then(&body.component_to_world).translation).filter(|p| p.is_finite())
                };
                const SEGMENTS: [(&str, &str, f32); 12] = [
                    ("koshi", "mune", 15.0),
                    ("mune", "kubi", 14.0),
                    ("L_ude1", "L_ude2", 6.5),
                    ("L_ude2", "L_te", 6.0),
                    ("R_ude1", "R_ude2", 6.5),
                    ("R_ude2", "R_te", 6.0),
                    ("L_momo", "L_sune", 10.0),
                    ("L_sune", "L_ashi1", 9.0),
                    ("L_ashi1", "L_ashi2", 9.0),
                    ("R_momo", "R_sune", 10.0),
                    ("R_sune", "R_ashi1", 9.0),
                    ("R_ashi1", "R_ashi2", 9.0),
                ];
                for (from, to, radius) in SEGMENTS {
                    if let (Some(a), Some(b)) = (at(from), at(to)) {
                        caps.push((a, b, radius));
                    }
                }
                // The head (with the hair): a ball above the neck.
                if let (Some(neck), Some(head)) = (at("kubi"), at("atama")) {
                    let up = (head - neck).normalize_or(Vec3::Z);
                    caps.push((head + up * 6.0, head + up * 16.0, 17.0));
                }
                if caps.is_empty() {
                    // (no pose: the old single capsule)
                    let feet = body.component_to_world.translation;
                    caps.push((feet + Vec3::Z * 24.0, feet + Vec3::Z * 126.0, 24.0));
                }
            }
            // The Keyblade. (IMPL_ER23A, the user: "the keyblade doesn't cast a shadow") IT WAS "from the hand to its bone
            // farthest from it": the weapon's skeleton is its root, the body bone `wep` and the SEVEN LINKS OF THE KEYCHAIN,
            // so that bone was the end of the dangling chain: a stub at his hand, and nothing at all without a chain pose.
            // Now: the blade's own measures in the frame of the bone `wep` (the pack's weapon manifest, `geometry`: +X from
            // the grip to the tip at 79 cm, the guard around the grip, the teeth on -Z near the tip): shadowcaps.rs.
            let body_caps = caps.len();
            if let Some(weapon) = input.weapon.as_ref().filter(|w| w.opacity > 0.5) {
                for (a, b, radius) in crate::shadowcaps::keyblade_capsules(weapon) {
                    if caps.len() < 16 {
                        caps.push((a, b, radius));
                    }
                }
            }
            hy_caps.clone_from(&caps);
            // (HY) the form weapons of an XS move (drawn where the host hung them, sometimes away from his hands) in the box of
            // the G-buffer pass too: a ball of 1.5 m around each (a pixel of theirs outside it keeps the world's G-buffer: its
            // capture does not match its normal and that pixel takes today's light)
            for f in &input.forms {
                let p = f.to_world.translation;
                if p.is_finite() {
                    hy_caps.push((p, p, 150.0));
                }
            }
            for k in 0..16 {
                match caps.get(k) {
                    Some((a, b, radius)) => {
                        frame.extend_from_slice(&a.extend(*radius).to_array());
                        // (IMPL_ER44) the Keyblade's capsules are marked 2 (present for the thrown shadow as before; the light
                        // pass leaves them out of his shadow from the Keyblade light, which sits on them)
                        frame.extend_from_slice(&b.extend(if k >= body_caps { 2.0 } else { 1.0 }).to_array());
                    }
                    None => frame.extend_from_slice(&[0.0; 8]),
                }
            }
        }
        // (IMPL_ER44) Sora's own point lights (er_char.wgsl `xl_*`, read by the *_pl shaders only).
        frame.extend_from_slice(&sora_lights);
        // (IMPL_ER60) the blood on him (er_char.wgsl `bl_*`, read by the *_bl shaders only; blood.rs)
        // (IMPL_ER59) with an outfit on: its own feet, hands and head (KH1 Sora is shorter)
        let outfit_bones = match (assets.models.iter().find_map(|m| m.retarget.as_ref()), input.body.as_ref()) {
            (Some(r), Some(body)) => Some((r, r.frame(input.clock, &body.component_to_world, &body.local, &body.component))),
            _ => None,
        };
        // (ER85) where the blood block starts (the eyes / mouth read a copy with its amounts at 0)
        let blood_at = frame.len();
        frame.extend_from_slice(&crate::blood::block(&input, |name| {
            let body = input.body.as_ref()?;
            if let Some((r, posed)) = &outfit_bones {
                if let Some(t) = r.index_of(name).and_then(|k| posed.1.get(k)) {
                    return Some(t.then(&body.component_to_world).translation);
                }
            }
            let bone = *assets.bones.get(name)?;
            Some(body.component.get(bone)?.then(&body.component_to_world).translation)
        }));
        // (IMPL_ER78) the hybrid's block (er_char.wgsl `hy`, read by the *_hy / *_gb shaders only; zeros = today's light)
        frame.extend_from_slice(&hy_block);

        // ---- the materials' state for this frame
        if assets.khs.as_ref().map(|(name, _)| name.as_str()) != Some(options.khs.as_str()) {
            let on = options.khs != "off" && !assets.library.khs.presets.is_empty();
            assets.khs = Some((options.khs.clone(), assets.library.khs.preset_state(&options.khs, on)));
        }
        if let Some(body) = &input.body {
            let table = assets.library.bone_table.clone();
            // (IMPL_ER59) with an outfit on: its own bones (an eyelid of its own place drives its eye's mask)
            let outfit = assets.models.iter().find_map(|m| m.retarget.as_ref()).map(|r| (r, r.frame(input.clock, &body.component_to_world, &body.local, &body.component)));
            let values = kh_look::bone_parameters(&table, |name| {
                if let Some((r, posed)) = &outfit {
                    let (locals, comps) = &**posed;
                    if let Some(k) = r.index_of(name) {
                        let component = *comps.get(k)?;
                        return Some((*locals.get(k)?, component, component.then(&body.component_to_world)));
                    }
                }
                let bone = *assets.bones.get(name)?;
                let component = *body.component.get(bone)?;
                Some((*body.local.get(bone)?, component, component.then(&body.component_to_world)))
            });
            assets.library.feed("sora", &values);
            // (IMPL_ER23B) THE COSTUME CHANGE (the user: "Sora doesn't change clothes when he form changes"): the wear form the
            // host reports (lib.rs `set_wear_form`), stepped by the frames' game clock; a changed binding is written into the
            // slot's table (the test app's rule: kh_look `Library::step_wear`).
            {
                let dt = assets.wear_clock.map_or(0.0, |before| (input.clock - before).clamp(0.0, 0.1));
                assets.wear_clock = Some(input.clock);
                let before = (assets.library.wear.worn, assets.library.wear.target);
                assets.library.wear.plain = crate::form_style() == 0; // (ER69) KH1's Second Form look
                let changes = assets.library.step_wear(crate::wear_form(), dt);
                let start = assets.srv_heap.GetCPUDescriptorHandleForHeapStart();
                for change in &changes {
                    let resource = assets.by_file.get(&change.file.file).unwrap_or(&assets.white);
                    device.CreateShaderResourceView(resource, None, D3D12_CPU_DESCRIPTOR_HANDLE { ptr: start.ptr + (change.slot * SLOT_TEXTURES + texture_descriptor(change.texture)) * assets.srv_size });
                }
                let now = (assets.library.wear.worn, assets.library.wear.target);
                if now != before {
                    rlog!("render: WEAR FORM {} -> {}: {} (worn {}, {} texture binding(s) changed{})", before.1, now.1, if now.0 == now.1 { "the costume is on" } else { "the costume slides in (FormChangeSlide)" }, now.0, changes.len(), if changes.iter().any(|c| !assets.by_file.contains_key(&c.file.file)) { "; A TEXTURE OF THE FORM IS MISSING: white" } else { "" });
                }
            }
        }
        if let Some(weapon) = &input.weapon {
            assets.library.set_scalar("kingdom_key", "OpacityFade", weapon.opacity);
        }
        // (VFXR) the effects' drives of Sora's materials (SQEX_Material_InLight ...: vfx.rs apply_char_drives)
        crate::vfx::apply_char_drives(&mut assets.library, 0);
        // (ER90) a form weapon is drawn whole
        for f in &input.forms {
            assets.library.set_scalar(&format!("{}{}", crate::xsiweapon::PREFIX, f.model), "OpacityFade", 1.0);
        }

        // ---- the upload buffer of the slot: constants as bytes, 256-aligned
        let mut cursor = 0usize;
        let upload_address = slot.upload.GetGPUVirtualAddress();
        // (FP) where this put must end: the buffer's end (`upload_cap`), and for a routed fragment's puts that less the reserve
        // of what the frame still needs (`routed_reserve`)
        let cap = upload_cap();
        let limit = std::cell::Cell::new(cap);
        let mut put = |floats: &[f32]| -> Option<u64> {
            let bytes = floats.len() * 4;
            if cursor + bytes > limit.get() {
                return None;
            }
            std::ptr::copy_nonoverlapping(floats.as_ptr() as *const u8, slot.mapped.add(cursor), bytes);
            let at = upload_address + cursor as u64;
            cursor = (cursor + bytes + 255) & !255;
            Some(at)
        };
        let overflow = || windows::core::Error::from_hresult(windows::Win32::Foundation::E_OUTOFMEMORY);
        // (NF) KH3'S CAMERA FADE (camfade.rs): the fade list stepped once a frame over every KH character drawn (Sora with his
        // weapons, every party member with his), each by his own volumes on his posed bones; Sora's value into his block (the
        // members' go into theirs in party::prepare). Not in the void (its own camera and pose: everyone whole).
        {
            let mut drawn: Vec<crate::camfade::Drawn> = Vec::new();
            if void.is_none() {
                if let Some(body) = &input.body {
                    let volumes = crate::camfade::volumes_of(crate::camfade::sora_chara(), &assets.bones, body);
                    drawn.push(crate::camfade::Drawn { key: crate::camfade::SORA, middle: crate::camfade::middle_of(&volumes, body.component_to_world.translation), volumes });
                }
                #[cfg(feature = "party")]
                if let Some(list) = crate::party::draws_now() {
                    for d in list.iter() {
                        if let (Some(body), Some(m)) = (&d.body, party.members.iter().find(|m| m.chara == d.chara)) {
                            let volumes = crate::camfade::volumes_of(m.npc, &m.bones, body);
                            drawn.push(crate::camfade::Drawn { key: crate::camfade::member_key(d.chara), middle: crate::camfade::middle_of(&volumes, body.component_to_world.translation), volumes });
                        }
                    }
                }
            }
            crate::camfade::step(input.clock, camera_game, *frame_no, game_on && options.jitter, &drawn);
            crate::camfade::write_block(&mut frame, crate::camfade::SORA);
        }
        let frame_constants = put(&frame).ok_or_else(overflow)?;
        // (ER85) the eyes and the mouth never take blood (blood.rs `spared`): their draws read this copy of the block
        let bleeding = frame.get(blood_at..blood_at + 2).is_some_and(|b| b.iter().any(|x| *x > 0.0));
        let spared_constants = if bleeding {
            let mut f = frame.clone();
            f[blood_at..blood_at + 2].fill(0.0);
            put(&f).unwrap_or(frame_constants)
        } else {
            frame_constants
        };
        // (ER85) the face: its share of the body's amount (blood.rs `share`)
        let face_constants = if bleeding {
            let mut f = frame.clone();
            f[blood_at] *= crate::blood::FACE_SHARE;
            put(&f).unwrap_or(frame_constants)
        } else {
            frame_constants
        };
        // (IMPL_ER78) the early list's depth draws run the colour shaders inside the deferred stage, where the game's light and
        // mask targets are not readable yet: their copy of the block has the hybrid off (the G-buffer pass sets the real one)
        let early_constants = if hy_on {
            let mut early = frame.clone();
            let n = early.len();
            early[n - crate::hybrid::BLOCK_FLOATS..].fill(0.0);
            put(&early).ok_or_else(overflow)?
        } else {
            frame_constants
        };

        // (ER party S4) THE PARTY OF THIS FRAME (gfx/party.rs): nothing at all unless the host submitted one
        #[cfg(feature = "party")]
        let party_started = Instant::now();
        // (RSF2 round 16, testing aid r.party_pin_test) the first friend's place: N metres from Sora's feet along the sun (horizontal)
        #[cfg(feature = "party")]
        {
            let n = *party::PIN_METRES.lock().unwrap_or_else(|e| e.into_inner());
            let target = match (n != 0.0, hy_sun, input.body.as_ref()) {
                (true, Some((dir, _)), Some(b)) => {
                    let h = Vec3::new(dir.x, dir.y, 0.0).normalize_or_zero();
                    (h.length() > 0.5).then(|| b.component_to_world.translation - h * n * 100.0)
                }
                _ => None,
            };
            *party::PIN_TARGET.lock().unwrap_or_else(|e| e.into_inner()) = target;
        }
        #[cfg(feature = "party")] // (RSF round 18: the build without the party feature; the cfg had moved onto the pin block above)
        let mut party_plan = {
            // (ER party I) the friends' own light: Sora's steady game light, the rig's inputs, the probe mode
            let party_light = light_now.as_ref().filter(|_| options_light && game_on).map(|l| {
                let s = crate::stable::settings();
                party::LightIn { steady: *l, fallback, key_gain: options.key * crate::light::CALIBRATION_DEFAULT, ambient_gain: options.ambient, probe_on: game_on && game.is_some(), world: (s.on && s.probe_world && game.as_ref().is_some_and(|k| k.world)).then_some(s.probe_radius) }
            });
            let cx = party::PrepCx { device: &*device, key, frame: &frame, early_hy_off: hy_on, upload_base: upload_address, lights: &party_lights, camera_game, out_scale, clock: input.clock, input: &input, khs: &options.khs, void: void.is_some(), data: game.as_ref().filter(|_| game_on).map(|k| &k.texture), hy_views: hy.cap.as_ref().map(|c| [c.light.as_raw() as usize, c.geo.as_raw() as usize]), white: &assets.white, light: party_light, frame_no: *frame_no, prev_plain: math::host_from_game(self_prev_origin.unwrap_or(camera_of.origin)), env_light, env_scale: options.env, sora_drawn: input.body.is_some() };
            party::prepare(party, &cx, &mut put)
        };
        // (ER party S4, for the log) the CPU time of the party's own work this frame: its preparation and its draw passes
        #[cfg(feature = "party")]
        let mut party_us = party_started.elapsed().as_micros() as u64;
        // (RSF2) THE LIGHT LEVEL OUR EFFECTS ARE LIT AT (decals, lit effect meshes): Sora's when he is drawn; when he is not (a
        // friend leads, Sora out of the party: his probe is empty and his level at its floor) the leader's own level (the member
        // nearest the hero light's carrier, else the first with a light of his own)
        #[cfg(feature = "party")]
        let fx_level = match (input.body.is_some(), party_plan.as_ref()) {
            (false, Some(plan)) => party::leader_level(plan, crate::party::level_anchor_now()).unwrap_or(out_scale), // (ERT4) the leader's feet, lit or not
            _ => out_scale,
        };
        #[cfg(not(feature = "party"))]
        let fx_level = out_scale;
        // (ER party I, S4's request) the hero light's body shadow from the friends too: the light pass reads a copy of the
        // frame block with their capsules in the slots it does not read as Sora's body (no friend drawn: Sora's block)
        #[cfg(feature = "party")]
        let light_constants = party_plan.as_ref().and_then(|p| party::light_pass_frame(p, &frame)).and_then(|f| put(&f)).unwrap_or(frame_constants);
        slot.allocator.Reset()?;
        slot.list.Reset(&slot.allocator, None)?;
        let list = &slot.list;
        // (IMPL_ER53) the early depth's own list (executed inside the deferred submission, before `list`)
        let early_list = if early_states.is_some() {
            slot.early_allocator.Reset()?;
            slot.early_list.Reset(&slot.early_allocator, None)?;
            Some(&slot.early_list)
        } else {
            None
        };
        if let Some((heap, ..)) = queries.as_ref() {
            list.EndQuery(heap, D3D12_QUERY_TYPE_TIMESTAMP, slot_index as u32 * 2);
        }
        crate::passtime::stamp(device, queue, list, slot_index, 0); // (HAIRPERF) the sections' timing
        // ---- THE GAME'S LIGHT into the game-data texture (light.rs): its constant buffers through
        // its own descriptor tables, then the probes around his feet through ours; a copy for the CPU.
        if let (true, Some(kit), Some(sub)) = (game_on, game.as_mut(), sub) {
            let start = assets.srv_heap.GetCPUDescriptorHandleForHeapStart();
            // (RSF2 round 8) bound per assets, the friends' rule (party.rs `m.data_bound`): a reloaded outfit is bound again
            let data = kit.texture.as_raw() as usize;
            if assets.data_bound != data {
                for k in 0..assets.library.slots.len() {
                    device.CreateShaderResourceView(&kit.texture, None, D3D12_CPU_DESCRIPTOR_HANDLE { ptr: start.ptr + (k * SLOT_TEXTURES + DEPTH_TEXTURE) * assets.srv_size });
                }
                assets.data_bound = data;
            }
            let probe_table = assets.library.slots.len() + 1;
            let wanted = [back.as_raw() as usize, sub.light2.unwrap_or(0), sub.mask.unwrap_or(0), sub.albedo.unwrap_or(0), 0];
            for (k, pointer) in wanted.iter().enumerate() {
                // (HARDENING: the views are written every frame: a resource made again at the same
                // address, after a resolution change, must not keep a view of the dead one; and only
                // a target the game transitioned in this frame is taken)
                let pointer = &(if k == 0 || survey::usable_now(*pointer, width, height) { *pointer } else { 0 });
                let at = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: start.ptr + (probe_table * SLOT_TEXTURES + k) * assets.srv_size };
                let raw = *pointer as *mut c_void;
                match ID3D12Resource::from_raw_borrowed(&raw) {
                    // (the surface colour target is typeless: read as sRGB, so the shader gets linear values)
                    Some(resource) if k == 3 => {
                        let desc = D3D12_SHADER_RESOURCE_VIEW_DESC { Format: DXGI_FORMAT_R8G8B8A8_UNORM_SRGB, ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D, Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING, Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 { Texture2D: D3D12_TEX2D_SRV { MostDetailedMip: 0, MipLevels: 1, PlaneSlice: 0, ResourceMinLODClamp: 0.0 } } };
                        device.CreateShaderResourceView(resource, Some(&desc), at)
                    }
                    Some(resource) => device.CreateShaderResourceView(resource, None, at),
                    None => device.CreateShaderResourceView(&assets.white, None, at),
                }
                kit.probed[k] = *pointer;
            }
            // (IMPL_ER64) the ring reads the scene depth (the snapshot without him when there is one) at entry 4
            if let (true, Some((scene_now, _))) = (kit.ring, &scene) {
                let reads = early_reads.as_ref().unwrap_or(&scene_now.resource);
                let view_format = match scene_now.info.format {
                    DXGI_FORMAT_R32G8X24_TYPELESS | DXGI_FORMAT_D32_FLOAT_S8X24_UINT => Some(DXGI_FORMAT_R32_FLOAT_X8X24_TYPELESS),
                    DXGI_FORMAT_R24G8_TYPELESS | DXGI_FORMAT_D24_UNORM_S8_UINT => Some(DXGI_FORMAT_R24_UNORM_X8_TYPELESS),
                    DXGI_FORMAT_R32_TYPELESS => Some(DXGI_FORMAT_R32_FLOAT),
                    _ => None,
                };
                let readable = scene_now.info.states.iter().all(|s| *s & D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE.0 != 0) || early_reads.is_some();
                if let (Some(format), true) = (view_format, readable) {
                    let desc = D3D12_SHADER_RESOURCE_VIEW_DESC { Format: format, ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D, Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING, Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 { Texture2D: D3D12_TEX2D_SRV { MostDetailedMip: 0, MipLevels: 1, PlaneSlice: 0, ResourceMinLODClamp: 0.0 } } };
                    device.CreateShaderResourceView(reads, Some(&desc), D3D12_CPU_DESCRIPTOR_HANDLE { ptr: start.ptr + (probe_table * SLOT_TEXTURES + 4) * assets.srv_size });
                }
            }
            let view = kit.rtv.GetCPUDescriptorHandleForHeapStart();
            let game_viewport = D3D12_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: crate::light::WIDTH as f32, Height: crate::light::HEIGHT as f32, MinDepth: 0.0, MaxDepth: 1.0 };
            // (IMPL_ER53) with an early depth the game's constants of this frame are copied in ITS list (his vertex shader
            // reads the game's view-projection from them there); the probes stay in the colour list (they read the
            // targets the rest of the deferred stage fills)
            let copier = early_list.unwrap_or(list);
            copier.ResourceBarrier(&[transition(&kit.texture, 0, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, D3D12_RESOURCE_STATE_RENDER_TARGET)]);
            copier.OMSetRenderTargets(1, Some(&view), false, None);
            copier.ClearRenderTargetView(view, &[0.0; 4], None);
            copier.RSSetViewports(&[game_viewport]);
            copier.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            let row = |y: usize| [RECT { left: 0, top: y as i32, right: crate::light::WIDTH as i32, bottom: y as i32 + 1 }];
            copier.SetGraphicsRootSignature(&kit.root);
            let mut heap_set = 0usize;
            for (k, table) in sub.tables[..sub.table_count.min(crate::light::CANDIDATES)].iter().enumerate() {
                let Some(heap) = views_heap(table.heaps) else { continue };
                if heap_set != heap.as_raw() as usize {
                    heap_set = heap.as_raw() as usize;
                    copier.SetDescriptorHeaps(&[Some(heap)]);
                }
                copier.SetGraphicsRootDescriptorTable(1, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: table.handle });
                copier.RSSetScissorRects(&row(k));
                copier.SetPipelineState(&kit.copy_light);
                copier.DrawInstanced(3, 1, 0, 0);
                if k == 0 {
                    copier.RSSetScissorRects(&row(crate::light::ROW_SCENE));
                    copier.SetPipelineState(&kit.copy_scene);
                    copier.DrawInstanced(3, 1, 0, 0);
                }
            }
            if early_list.is_some() {
                copier.ResourceBarrier(&[transition(&kit.texture, 0, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE)]);
                list.ResourceBarrier(&[transition(&kit.texture, 0, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, D3D12_RESOURCE_STATE_RENDER_TARGET)]);
                list.OMSetRenderTargets(1, Some(&view), false, None);
                list.RSSetViewports(&[game_viewport]);
                list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            }
            list.SetGraphicsRootSignature(&*root);
            list.SetDescriptorHeaps(&[Some(assets.srv_heap.clone()), Some(sampler_heap.clone())]);
            list.SetGraphicsRootConstantBufferView(0, frame_constants);
            list.SetGraphicsRootDescriptorTable(3, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: assets.srv_heap.GetGPUDescriptorHandleForHeapStart().ptr + (probe_table * SLOT_TEXTURES * assets.srv_size) as u64 });
            list.RSSetScissorRects(&row(crate::light::ROW_PROBE));
            list.SetPipelineState(&kit.probe);
            list.DrawInstanced(3, 1, 0, 0);
            // (ER party I) each friend's own probe into his row (his frame copy: er.probe / blob / caps are his)
            #[cfg(feature = "party")]
            if let Some(plan) = party_plan.as_ref() {
                let all = D3D12_VIEWPORT { Height: GAME_ROWS as f32, ..game_viewport };
                if party::probe_draws(plan, list, all, crate::light::WIDTH as u32) > 0 {
                    list.SetGraphicsRootConstantBufferView(0, frame_constants);
                    list.RSSetViewports(&[game_viewport]);
                }
            }
            if slot.readback.is_none() {
                slot.readback = committed(device, D3D12_HEAP_TYPE_READBACK, &buffer_desc(kit.bytes), D3D12_RESOURCE_STATE_COPY_DEST, None).ok();
            }
            list.ResourceBarrier(&[transition(&kit.texture, 0, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_COPY_SOURCE)]);
            if let Some(readback) = slot.readback.as_ref() {
                let to = D3D12_TEXTURE_COPY_LOCATION { pResource: std::mem::transmute_copy(readback), Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT, Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { PlacedFootprint: kit.footprint } };
                let from = D3D12_TEXTURE_COPY_LOCATION { pResource: std::mem::transmute_copy(&kit.texture), Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX, Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { SubresourceIndex: 0 } };
                list.CopyTextureRegion(&to, 0, 0, 0, &from, None);
                slot.read_pending = true;
            }
            list.ResourceBarrier(&[transition(&kit.texture, 0, D3D12_RESOURCE_STATE_COPY_SOURCE, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE)]);
        }
        crate::passtime::stamp(device, queue, list, slot_index, 1); // (HAIRPERF)
        // (HY) THE CAPTURE OF THE GAME'S LIGHT, right after its deferred stage, over the box of every G-buffer draw of this frame
        // (his, his form weapons', the friends'): our own two targets (hycap.ps.dxil reads the game's light, mask and G-buffer as
        // they rest readable now: no barrier on any of the game's resources), then every slot's table reads them at 21 / 22
        // (written when they are made again, and for an outfit's new tables). Nothing of the hybrid reads the game's targets
        // after this point.
        // (RSF) his box: his capsules, his form weapons and a ball round every bone of his pose (hybrid.rs bone_balls: the same
        // reach for every drawn KH character)
        let hy_reach: Vec<(Vec3, Vec3, f32)> = {
            let mut r = hy_caps.clone();
            if let Some(body) = &input.body {
                r.extend(crate::hybrid::bone_balls(&body.component, &body.component_to_world));
            }
            r
        };
        let mut hy_box = crate::hybrid::scissor_box(&hy_reach, &clip, tan_y, width, height, 24.0);
        // (CAPPERF) each KH character's own box (the capture's level of detail goes by their pixels, not by the union's)
        #[allow(unused_mut)]
        let mut hy_boxes: Vec<[i32; 4]> = hy_box.into_iter().collect();
        #[allow(unused_mut)]
        let mut test_caps: Vec<(Vec3, Vec3, f32)> = if crate::hybrid::TEST.load(Ordering::Relaxed) { hy_caps.clone() } else { Vec::new() };
        #[cfg(feature = "party")]
        if let Some(plan) = party_plan.as_ref() {
            for pm in &plan.members {
                let own = crate::hybrid::scissor_box(&pm.reach, &clip, tan_y, width, height, 24.0);
                hy_boxes.extend(own);
                hy_box = crate::hybrid::union_box(hy_box, own);
                if crate::hybrid::TEST.load(Ordering::Relaxed) {
                    test_caps.extend(pm.caps.iter().copied());
                }
            }
        }
        // (HY, the offline harness only) everybody's capsules for its stand-in of the game's light passes
        if crate::hybrid::TEST.load(Ordering::Relaxed) {
            if let Ok(mut c) = crate::hybrid::TEST_CAPS.lock() {
                *c = test_caps;
            }
        }
        // (RSF2 round 6) this frame's early list copies the scene depth with the KH characters in it (for the sun lookup)
        let kh_copy = sun_read_wanted() && early_list.is_some() && early_states.is_some() && scene.as_ref().is_some_and(|(sc, _)| kh_depth.as_ref().is_some_and(|k| k.1 == sc.resource.as_raw() as usize));
        let hy_on = match (&hy_plan, hy.cap.as_mut(), hy_box) {
            (Ok((ordered, light, mask, _)), Some(cap), Some(bx)) => {
                // (RSF2 round 5) the capture's box into Sora's block (row 9): the capture shader's mean of the game's mask over
                // the KH pixels in it (er_char.wgsl hycap `hc_mean`); the colour list has not run yet
                let at = (frame_constants - slot.upload.GetGPUVirtualAddress()) as usize + (frame.len() - crate::hybrid::BLOCK_FLOATS + crate::hybrid::BOX_ROW * 4) * 4;
                let row = crate::hybrid::box_row(bx);
                std::ptr::copy_nonoverlapping(row.as_ptr() as *const u8, slot.mapped.add(at), 16);
                // (CAPPERF) the capture's level of detail (row 17.z: hybrid.rs lod_frame, r.hybrid_lod_px): the boxes clamped to the
                // picture, summed
                let clamped: Vec<[i32; 4]> = hy_boxes.iter().map(|b| [b[0].clamp(0, width as i32), b[1].clamp(0, height as i32), b[2].clamp(0, width as i32), b[3].clamp(0, height as i32)]).collect();
                let lod: f32 = if crate::hybrid::lod_frame(crate::hybrid::boxes_px(&clamped)) { 1.0 } else { 0.0 };
                let at = (frame_constants - slot.upload.GetGPUVirtualAddress()) as usize + (frame.len() - crate::hybrid::BLOCK_FLOATS + crate::hybrid::LOD_ROW * 4 + crate::hybrid::LOD_COMPONENT) * 4;
                std::ptr::copy_nonoverlapping(lod.to_ne_bytes().as_ptr(), slot.mapped.add(at), 4);
                // (RSF2 round 6) the cascades read at this frame's cast split, with the depth copy this frame's early list makes:
                // their rows (else zeros: the capture keeps the round-5 mean) and the three inputs
                let sun_frame = sun_now.as_ref().filter(|s| kh_copy && s.at.elapsed() < Duration::from_millis(40));
                let rows = sun_frame.map_or([0.0f32; 24], |s| { let h = crate::hybrid::settings(); crate::hybrid::sun_rows(h.sun_soft, h.sun_bias, &s.viewports) });
                let at = (frame_constants - slot.upload.GetGPUVirtualAddress()) as usize + (frame.len() - crate::hybrid::BLOCK_FLOATS + crate::hybrid::SUN_ROW * 4) * 4;
                std::ptr::copy_nonoverlapping(rows.as_ptr() as *const u8, slot.mapped.add(at), rows.len() * 4);
                let sun_inputs = match (sun_frame, kh_depth.as_ref(), sun_kit.as_ref()) {
                    (Some(s), Some((depth, _)), Some(kit)) => [depth.as_raw() as usize, s.atlas, kit.texture.as_raw() as usize],
                    // (RSF round 19) the depth with the KH characters all the same (no sun: hc_blur's reach on their bodies)
                    (_, Some((depth, _)), _) => [depth.as_raw() as usize, 0, 0],
                    _ => [0; 3],
                };
                SUN_FRAMES.fetch_add(u32::from(sun_inputs[1] != 0), Ordering::Relaxed);
                // (RSF2 round 15) the KH caster maps: their rows (the maps' cascade must be among this frame's sun rows) and view
                let maps_row = match (sun_frame, kh_maps_vp.as_ref().filter(|(_, at)| at.elapsed() < Duration::from_millis(60)), kh_maps.as_ref()) {
                    (Some(s), Some((vp, _)), Some(_)) if sun_inputs[1] != 0 => s.viewports.iter().take(crate::hybrid::CASC_MAX).position(|v| v == vp).map(|r| (*vp, r)),
                    _ => None,
                };
                #[cfg(feature = "party")]
                let party_feet: Vec<Vec3> = party_plan.as_ref().map(|p| p.members.iter().map(|m| m.feet.unwrap_or(Vec3::splat(1.0e9))).collect()).unwrap_or_default();
                #[cfg(not(feature = "party"))]
                let party_feet: Vec<Vec3> = Vec::new();
                let rows = crate::hybrid::kh_rows(maps_row, &party_feet);
                let at = (frame_constants - slot.upload.GetGPUVirtualAddress()) as usize + (frame.len() - crate::hybrid::BLOCK_FLOATS + crate::hybrid::KH_ROW * 4) * 4;
                std::ptr::copy_nonoverlapping(rows.as_ptr() as *const u8, slot.mapped.add(at), rows.len() * 4);
                let maps_ptr = if maps_row.is_some() { kh_maps.as_ref().map_or(0, |m| m.texture.as_raw() as usize) } else { 0 };
                KH_MAPS_USED.fetch_add(u32::from(maps_ptr != 0), Ordering::Relaxed);
                record_capture(device, list, root, cap, slot_index, [*light, *mask, ordered[0].resource, ordered[1].resource, ordered[2].resource, sun_inputs[0], sun_inputs[1], sun_inputs[2]], maps_ptr, &assets.white, frame_constants, bx);
                *crate::hybrid::CAPTURE.lock().unwrap_or_else(|e| e.into_inner()) = format!("{}x{} box {:?}, light {}, mask {}", cap.width, cap.height, bx, survey::describe_resource(*light), if *mask == 0 { "(none: white)".to_string() } else { survey::describe_resource(*mask) });
                true
            }
            (Ok(_), _, _) => {
                crate::hybrid::FRAMES_ON.fetch_sub(1, Ordering::Relaxed);
                crate::hybrid::fell_back("no screen box for the G-buffer pass (nothing of his in front of the camera)");
                false
            }
            _ => false,
        };
        if !hy_on && hy_plan.is_ok() {
            // (the colour list, not executed yet, reads the block with the hybrid off: today's light; the party's copies too)
            let at = (frame_constants - slot.upload.GetGPUVirtualAddress()) as usize + (frame.len() - crate::hybrid::BLOCK_FLOATS) * 4;
            std::ptr::write_bytes(slot.mapped.add(at), 0, crate::hybrid::BLOCK_FLOATS * 4);
            #[cfg(feature = "party")]
            if let Some(plan) = party_plan.as_ref() {
                party::hybrid_off(plan, slot.mapped, frame.len());
            }
        }
        if let Some(cap) = hy.cap.as_ref() {
            let want = [cap.light.as_raw() as usize, cap.geo.as_raw() as usize];
            if assets.hy_bound != want {
                let start = assets.srv_heap.GetCPUDescriptorHandleForHeapStart();
                for k in 0..assets.library.slots.len() {
                    for (entry, resource) in [(HY_LIGHT_TEXTURE, &cap.light), (HY_MASK_TEXTURE, &cap.geo)] {
                        device.CreateShaderResourceView(resource, None, D3D12_CPU_DESCRIPTOR_HANDLE { ptr: start.ptr + (k * SLOT_TEXTURES + entry) * assets.srv_size });
                    }
                }
                assets.hy_bound = want;
                rlog!("render: (HY) the hybrid's capture ({}x{}) bound into the {} slot table(s)", cap.width, cap.height, assets.library.slots.len());
            }
        }
        hy.views = [0, 0];
        // (IMPL_ER78, r.hybrid_dump) the game's targets around him as the colour list finds them (the end of the deferred
        // stage: what its lights made of the G-buffer he wrote), before his colour goes in; read back a few frames later
        let mut hy_dump_rect: Option<[u32; 4]> = None;
        if let (Ok((ordered, light, mask, _)), true) = (&hy_plan, hy.dump.is_none() && crate::hybrid::DUMP_WANTED.swap(false, Ordering::Relaxed)) {
            let px = |c: Vec4| (c.w > 0.3).then(|| ((c.x / c.w * 0.5 + 0.5) * width as f32, (0.5 - c.y / c.w * 0.5) * height as f32));
            let head_clip = input.body.as_ref().map(|b| clip * (b.component_to_world.translation + Vec3::Z * 170.0).extend(1.0));
            // (RSF round 19, testing aid) `KH_HY_DUMP_FULL`: the whole picture (every KH character), not only Sora's box
            let full = std::env::var_os("KH_HY_DUMP_FULL").is_some().then_some([0u32, 0, width, height]);
            if let Some(rect) = full.or_else(|| crate::hybrid::dump_box(feet_clip.and_then(px), head_clip.and_then(px), width, height, 640)) {
                let mut items: Vec<(String, ID3D12Resource, D3D12_PLACED_SUBRESOURCE_FOOTPRINT, i32, String)> = Vec::new();
                let mut wanted: Vec<(String, usize, i32)> = ordered.iter().enumerate().map(|(k, t)| (crate::hybrid::ROLE_NAMES[k].to_string(), t.resource, t.format)).collect();
                wanted.push(("light_diffuse".to_string(), *light, crate::hybrid::fmt::R11G11B10_FLOAT));
                if *mask != 0 {
                    wanted.push(("shadow_mask".to_string(), *mask, crate::hybrid::fmt::R8G8B8A8_UNORM));
                }
                wanted.push(("scene_before_sora".to_string(), back.as_raw() as usize, back_desc.Format.0));
                // (HY) our capture of them (what his shaders read): the light a white surface got + occlusion, normal + sun visibility
                let ours: Vec<usize> = hy.cap.as_ref().map(|c| vec![c.light.as_raw() as usize, c.geo.as_raw() as usize]).unwrap_or_default();
                for (k, name) in ["capture_light_ao", "capture_normal_sun"].iter().enumerate() {
                    if let Some(p) = ours.get(k) {
                        wanted.push((name.to_string(), *p, crate::hybrid::fmt::R16G16B16A16_FLOAT));
                    }
                }
                for (name, pointer, format) in wanted {
                    let raw = pointer as *mut c_void;
                    let Some(resource) = ID3D12Resource::from_raw_borrowed(&raw) else { continue };
                    let state = if pointer == back.as_raw() as usize { Some(rest.0) } else if ours.contains(&pointer) { Some(D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE.0) } else { survey::states_now(pointer).map(|s| s[0]) };
                    let Some(state) = state.filter(|s| *s != 0 && s & (D3D12_RESOURCE_STATE_RENDER_TARGET.0 | D3D12_RESOURCE_STATE_DEPTH_WRITE.0 | D3D12_RESOURCE_STATE_UNORDERED_ACCESS.0 | D3D12_RESOURCE_STATE_COPY_DEST.0) == 0) else { continue };
                    if crate::hybrid::texel_bytes(format).is_none() {
                        continue;
                    }
                    let mut desc = resource.GetDesc();
                    (desc.Width, desc.Height, desc.MipLevels, desc.DepthOrArraySize) = ((rect[2] - rect[0]) as u64, rect[3] - rect[1], 1, 1);
                    let mut footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default();
                    let mut total = 0u64;
                    device.GetCopyableFootprints(&desc, 0, 1, 0, Some(&mut footprint), None, None, Some(&mut total));
                    let Ok(buffer) = committed(device, D3D12_HEAP_TYPE_READBACK, &buffer_desc(total.max(256)), D3D12_RESOURCE_STATE_COPY_DEST, None) else { continue };
                    list.ResourceBarrier(&[transition(resource, 0, D3D12_RESOURCE_STATES(state), D3D12_RESOURCE_STATE_COPY_SOURCE)]);
                    let to = D3D12_TEXTURE_COPY_LOCATION { pResource: std::mem::transmute_copy(&buffer), Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT, Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { PlacedFootprint: footprint } };
                    let from = D3D12_TEXTURE_COPY_LOCATION { pResource: std::mem::transmute_copy(resource), Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX, Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { SubresourceIndex: 0 } };
                    let region = D3D12_BOX { left: rect[0], top: rect[1], front: 0, right: rect[2], bottom: rect[3], back: 1 };
                    list.CopyTextureRegion(&to, 0, 0, 0, &from, Some(&region));
                    list.ResourceBarrier(&[transition(resource, 0, D3D12_RESOURCE_STATE_COPY_SOURCE, D3D12_RESOURCE_STATES(state))]);
                    items.push((name, buffer, footprint, format, format!("{} state {}", survey::describe_resource(pointer), survey::state_text(state))));
                }
                hy_dump_rect = Some(rect);
                hy.dump = Some(HybridDump { slot: slot_index, frame: *frame_no, rect, items });
            }
        }
        let target_view = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: rtv_heap.GetCPUDescriptorHandleForHeapStart().ptr + slot_index * device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV) as usize };
        device.CreateRenderTargetView(back, None, target_view);
        let mut before: Vec<D3D12_RESOURCE_BARRIER> = vec![transition(back, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, rest, D3D12_RESOURCE_STATE_RENDER_TARGET)];
        let mut after: Vec<D3D12_RESOURCE_BARRIER> = vec![transition(back, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_RENDER_TARGET, rest)];
        let depth_view = match &scene {
            Some((scene, format)) => {
                // The game's depth target: taken to DEPTH_WRITE for our draws and put back as it was.
                let view = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: dsv_heap.GetCPUDescriptorHandleForHeapStart().ptr + (1 + slot_index) * *dsv_size };
                let desc = D3D12_DEPTH_STENCIL_VIEW_DESC { Format: *format, ViewDimension: D3D12_DSV_DIMENSION_TEXTURE2D, Flags: D3D12_DSV_FLAG_NONE, Anonymous: D3D12_DEPTH_STENCIL_VIEW_DESC_0 { Texture2D: D3D12_TEX2D_DSV { MipSlice: 0 } } };
                device.CreateDepthStencilView(&scene.resource, Some(&desc), view);
                for (subresource, state) in scene.info.states.iter().enumerate() {
                    if *state != D3D12_RESOURCE_STATE_DEPTH_WRITE.0 {
                        before.push(transition(&scene.resource, subresource as u32, D3D12_RESOURCE_STATES(*state), D3D12_RESOURCE_STATE_DEPTH_WRITE));
                        after.push(transition(&scene.resource, subresource as u32, D3D12_RESOURCE_STATE_DEPTH_WRITE, D3D12_RESOURCE_STATES(*state)));
                    }
                }
                stats.with_scene_depth += 1;
                view
            }
            None => dsv_heap.GetCPUDescriptorHandleForHeapStart(),
        };
        crate::passtime::stamp(device, queue, list, slot_index, 2); // (HAIRPERF)
        // ---- the contact shadow first: the scene depth is read as a texture (it rests in a shader
        // resource state between the game's passes), the finished picture is multiplied.
        list.ResourceBarrier(&before[..1]);
        // (the void: the whole back buffer black, his picture in the centred 16:9 stage rectangle)
        let stage = if void.is_some() { let (w, h) = (width as f32, height as f32); let (sw, sh) = if w * 9.0 > h * 16.0 { (h * 16.0 / 9.0, h) } else { (w, w * 9.0 / 16.0) }; (((w - sw) * 0.5).floor(), ((h - sh) * 0.5).floor(), sw, sh) } else { (0.0, 0.0, width as f32, height as f32) };
        if void.is_some() {
            list.ClearRenderTargetView(target_view, &[0.0, 0.0, 0.0, 1.0], None);
        }
        list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: stage.0, TopLeftY: stage.1, Width: stage.2, Height: stage.3, MinDepth: 0.0, MaxDepth: 1.0 }]);
        list.SetGraphicsRootSignature(&*root);
        list.SetDescriptorHeaps(&[Some(assets.srv_heap.clone()), Some(sampler_heap.clone())]);
        list.SetGraphicsRootConstantBufferView(0, frame_constants);
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        // (IMPL_ER39) THE LIGHT PASS (pointlight.rs): additive, from the scene depth (read as a texture, as the contact
        // shadow does) and the game's surface colour; after the probes read the scene, before his contact shadow (which
        // then darkens it too) and before him. Its own table: the heap's third extra one.
        if let (false, Some((scene, _))) = (lights_now.is_empty(), &scene) {
            let readable = scene.info.states.iter().all(|s| *s & D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE.0 != 0);
            let view_format = match scene.info.format {
                DXGI_FORMAT_R32G8X24_TYPELESS | DXGI_FORMAT_D32_FLOAT_S8X24_UINT => Some(DXGI_FORMAT_R32_FLOAT_X8X24_TYPELESS),
                DXGI_FORMAT_R24G8_TYPELESS | DXGI_FORMAT_D24_UNORM_S8_UINT => Some(DXGI_FORMAT_R24_UNORM_X8_TYPELESS),
                DXGI_FORMAT_R32_TYPELESS => Some(DXGI_FORMAT_R32_FLOAT),
                DXGI_FORMAT_R16_TYPELESS => Some(DXGI_FORMAT_R16_UNORM),
                _ => None,
            };
            let rect = crate::pointlight::screen_box(&lights_now, &clip, width, height, tan_y);
            if let (true, Some(view_format), Some(rect)) = (readable, view_format, rect) {
                let albedo_at = sub.and_then(|s| s.albedo).filter(|a| survey::usable_now(*a, width, height)).unwrap_or(0) as *mut c_void;
                let albedo = ID3D12Resource::from_raw_borrowed(&albedo_at);
                let block = crate::pointlight::block(&lights_now, albedo.is_some(), 0.18, crate::pointlight::settings().occlusion_steps);
                if let Some(constants) = put(&block) {
                    let table = assets.library.slots.len() + 2;
                    list.OMSetRenderTargets(1, Some(&target_view), false, None);
                    let cx = crate::pointlight::PassCx {
                        device: &*device,
                        list,
                        root: &*root,
                        format: back_desc.Format,
                        table_cpu: D3D12_CPU_DESCRIPTOR_HANDLE { ptr: assets.srv_heap.GetCPUDescriptorHandleForHeapStart().ptr + table * SLOT_TEXTURES * assets.srv_size },
                        table_gpu: D3D12_GPU_DESCRIPTOR_HANDLE { ptr: assets.srv_heap.GetGPUDescriptorHandleForHeapStart().ptr + (table * SLOT_TEXTURES * assets.srv_size) as u64 },
                        view_size: assets.srv_size,
                        depth: early_reads.as_ref().unwrap_or(&scene.resource),
                        depth_view_format: view_format,
                        albedo,
                        white: &assets.white,
                        constants,
                        scissor: rect,
                        lights: lights_now.len(),
                    };
                    // (ER party I) the friends' capsules for the light pass only (Sora's block bound again after it)
                    #[cfg(feature = "party")]
                    if light_constants != frame_constants {
                        list.SetGraphicsRootConstantBufferView(0, light_constants);
                    }
                    crate::pointlight::draw(&cx);
                    #[cfg(feature = "party")]
                    if light_constants != frame_constants {
                        list.SetGraphicsRootConstantBufferView(0, frame_constants);
                    }
                }
            }
        }
        let blob_table = assets.library.slots.len();
        if let (Some((scene, _)), Some(body), Some(pipeline), true) = (&scene, &input.body, pipelines.get(blob_table), options.shadow > 0.0 && options.flat == 0) {
            let readable = scene.info.states.iter().all(|s| *s & D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE.0 != 0);
            let view_format = match scene.info.format {
                DXGI_FORMAT_R32G8X24_TYPELESS | DXGI_FORMAT_D32_FLOAT_S8X24_UINT => Some(DXGI_FORMAT_R32_FLOAT_X8X24_TYPELESS),
                DXGI_FORMAT_R24G8_TYPELESS | DXGI_FORMAT_D24_UNORM_S8_UINT => Some(DXGI_FORMAT_R24_UNORM_X8_TYPELESS),
                DXGI_FORMAT_R32_TYPELESS => Some(DXGI_FORMAT_R32_FLOAT),
                DXGI_FORMAT_R16_TYPELESS => Some(DXGI_FORMAT_R16_UNORM),
                _ => None,
            };
            // Where his surroundings are on screen: a box around the projected feet.
            let feet = clip * body.component_to_world.translation.extend(1.0);
            if let (true, Some(view_format), true) = (readable, view_format, feet.w > 0.3) {
                let table = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: assets.srv_heap.GetCPUDescriptorHandleForHeapStart().ptr + (blob_table * SLOT_TEXTURES + DEPTH_TEXTURE) * assets.srv_size };
                {
                    let desc = D3D12_SHADER_RESOURCE_VIEW_DESC { Format: view_format, ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D, Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING, Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 { Texture2D: D3D12_TEX2D_SRV { MostDetailedMip: 0, MipLevels: 1, PlaneSlice: 0, ResourceMinLODClamp: 0.0 } } };
                    // (IMPL_ER53) with an early depth: the snapshot without him (as before)
                    let reads = early_reads.as_ref().unwrap_or(&scene.resource);
                    device.CreateShaderResourceView(reads, Some(&desc), table);
                    *blob_depth = reads.as_raw() as usize;
                }
                // The game's shadow mask beside the depth (where the ground is in a shadow already he throws none).
                let wanted_mask = sub.and_then(|s| s.mask).filter(|m| survey::usable_now(*m, width, height)).unwrap_or(0);
                {
                    let at = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: assets.srv_heap.GetCPUDescriptorHandleForHeapStart().ptr + (blob_table * SLOT_TEXTURES + CUBE_TEXTURE) * assets.srv_size };
                    let raw = wanted_mask as *mut c_void;
                    match ID3D12Resource::from_raw_borrowed(&raw) {
                        Some(resource) => device.CreateShaderResourceView(resource, None, at),
                        None => device.CreateShaderResourceView(&assets.white, None, at),
                    }
                    *blob_mask = wanted_mask;
                }
                // The box on screen: around his feet, and with a thrown shadow around his body and
                // the place its tip falls on level ground (at most 12 m away).
                let mut points = vec![(body.component_to_world.translation, 0.6f32)];
                if sun_shadow > 0.0 {
                    let throw = (Vec3::new(-key_dir.x, -key_dir.y, 0.0) / key_dir.z.max(0.12) * 150.0).clamp_length_max(1200.0);
                    points.push((body.component_to_world.translation + Vec3::Z * 150.0, 0.6));
                    points.push((body.component_to_world.translation + throw, 0.9));
                }
                let (mut low, mut high) = (glam::Vec2::splat(f32::MAX), glam::Vec2::splat(f32::MIN));
                for (point, radius) in points {
                    let c = clip * point.extend(1.0);
                    if c.w < 0.3 {
                        (low, high) = (glam::Vec2::ZERO, glam::Vec2::new(width as f32, height as f32));
                        break;
                    }
                    let centre = glam::Vec2::new((c.x / c.w * 0.5 + 0.5) * width as f32, (0.5 - c.y / c.w * 0.5) * height as f32);
                    let reach = (radius / (c.w * tan_y) * height as f32 * 0.5).clamp(8.0, height as f32);
                    low = low.min(centre - glam::Vec2::splat(reach));
                    high = high.max(centre + glam::Vec2::splat(reach));
                }
                let scissor = RECT { left: (low.x as i32).clamp(0, width as i32), top: (low.y as i32).clamp(0, height as i32), right: (high.x as i32).clamp(0, width as i32), bottom: (high.y as i32).clamp(0, height as i32) };
                if scissor.right > scissor.left && scissor.bottom > scissor.top {
                    list.OMSetRenderTargets(1, Some(&target_view), false, None);
                    list.RSSetScissorRects(&[scissor]);
                    list.SetPipelineState(pipeline);
                    list.SetGraphicsRootDescriptorTable(3, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: assets.srv_heap.GetGPUDescriptorHandleForHeapStart().ptr + (blob_table * SLOT_TEXTURES * assets.srv_size) as u64 });
                    list.DrawInstanced(3, 1, 0, 0);
                }
            }
        }
        // (ER party S4) the friends' contact / thrown shadows (Sora's blob pipeline and table, each friend's frame block)
        #[cfg(feature = "party")]
        if let (Some(plan), Some((scene, _)), Some(pipeline), true) = (party_plan.as_ref(), &scene, pipelines.get(blob_table), options.shadow > 0.0 && options.flat == 0) {
            let readable = scene.info.states.iter().all(|s| *s & D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE.0 != 0);
            let view_format = match scene.info.format {
                DXGI_FORMAT_R32G8X24_TYPELESS | DXGI_FORMAT_D32_FLOAT_S8X24_UINT => Some(DXGI_FORMAT_R32_FLOAT_X8X24_TYPELESS),
                DXGI_FORMAT_R24G8_TYPELESS | DXGI_FORMAT_D24_UNORM_S8_UINT => Some(DXGI_FORMAT_R24_UNORM_X8_TYPELESS),
                DXGI_FORMAT_R32_TYPELESS => Some(DXGI_FORMAT_R32_FLOAT),
                DXGI_FORMAT_R16_TYPELESS => Some(DXGI_FORMAT_R16_UNORM),
                _ => None,
            };
            if let (true, Some(view_format)) = (readable, view_format) {
                let start = assets.srv_heap.GetCPUDescriptorHandleForHeapStart();
                let reads = early_reads.as_ref().unwrap_or(&scene.resource);
                if *blob_depth != reads.as_raw() as usize {
                    let desc = D3D12_SHADER_RESOURCE_VIEW_DESC { Format: view_format, ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D, Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING, Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 { Texture2D: D3D12_TEX2D_SRV { MostDetailedMip: 0, MipLevels: 1, PlaneSlice: 0, ResourceMinLODClamp: 0.0 } } };
                    device.CreateShaderResourceView(reads, Some(&desc), D3D12_CPU_DESCRIPTOR_HANDLE { ptr: start.ptr + (blob_table * SLOT_TEXTURES + DEPTH_TEXTURE) * assets.srv_size });
                    *blob_depth = reads.as_raw() as usize;
                }
                let wanted_mask = sub.and_then(|s| s.mask).filter(|m| survey::usable_now(*m, width, height)).unwrap_or(0);
                if *blob_mask != wanted_mask {
                    let at = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: start.ptr + (blob_table * SLOT_TEXTURES + CUBE_TEXTURE) * assets.srv_size };
                    let raw = wanted_mask as *mut c_void;
                    match ID3D12Resource::from_raw_borrowed(&raw) {
                        Some(resource) => device.CreateShaderResourceView(resource, None, at),
                        None => device.CreateShaderResourceView(&assets.white, None, at),
                    }
                    *blob_mask = wanted_mask;
                }
                let cx = party::PassCx { key, sampler_heap: &*sampler_heap, sora_heap: &assets.srv_heap, frame_constants, early_constants, rig: &rig, sh, env_level, env_tint, clock: input.clock, exposure: options.exposure, debug: options.debug };
                list.OMSetRenderTargets(1, Some(&target_view), false, None);
                party::contact_shadows(plan, list, pipeline, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: assets.srv_heap.GetGPUDescriptorHandleForHeapStart().ptr + (blob_table * SLOT_TEXTURES * assets.srv_size) as u64 }, &clip, tan_y, width, height, key_dir, sun_shadow > 0.0, &cx);
            }
        }
        if before.len() > 1 {
            list.ResourceBarrier(&before[1..]);
        }
        list.OMSetRenderTargets(1, Some(&target_view), false, Some(&depth_view));
        if scene.is_none() {
            list.ClearDepthStencilView(depth_view, D3D12_CLEAR_FLAG_DEPTH, 0.0, 0, None);
        }
        list.RSSetScissorRects(&[RECT { left: 0, top: 0, right: width as i32, bottom: height as i32 }]);
        // (IMPL_ER74) OUR DECALS FIRST, before him (he covers them where he stands; they project onto the surfaces the
        // scene depth shows, his own included since his early depth: drawn after him they painted their pattern on his
        // body, 34.jpg / 38.jpg / 47.jpg), then the root state of his draws again
        if let (true, Some((scene_now, format))) = (hdr, &scene) {
            let exposure = light_now.as_ref().filter(|l| l.scene && game_on).map(|l| l.exposure);
            crate::vfx::draw(&crate::vfx::DrawCx { device: &*device, list, slot: slot_index, target: back, target_view, depth: &scene_now.resource, depth_format: *format, depth_view, width, height, clip_from_game: clip, camera: camera_game, right: math::game_dir_of_host(camera.right), up: math::game_dir_of_host(camera.up), forward: math::game_dir_of_host(camera.forward), depth_a, depth_b, proj00: 1.0 / (tan_y * camera.aspect), scale: crate::vfx::effect_scale(exposure, out_scale), lit: Vec3::splat(fx_level), stage: crate::vfx::Stage::Decals });
            list.OMSetRenderTargets(1, Some(&target_view), false, Some(&depth_view));
            list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: stage.0, TopLeftY: stage.1, Width: stage.2, Height: stage.3, MinDepth: 0.0, MaxDepth: 1.0 }]);
            list.RSSetScissorRects(&[RECT { left: 0, top: 0, right: width as i32, bottom: height as i32 }]);
            list.SetGraphicsRootSignature(&*root);
            list.SetDescriptorHeaps(&[Some(assets.srv_heap.clone()), Some(sampler_heap.clone())]);
            list.SetGraphicsRootConstantBufferView(0, frame_constants);
            list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        }
        list.SetGraphicsRootDescriptorTable(4, sampler_heap.GetGPUDescriptorHandleForHeapStart());
        list.SetGraphicsRootShaderResourceView(5, sampler_index.GetGPUVirtualAddress());
        // (IMPL_ER53) THE EARLY DEPTH's list: the scene depth from its state at the split (a snapshot copied first) to
        // DEPTH_WRITE, no target, the same root state as the colour draw; its draws are recorded beside the colour
        // draws below (the same constants: the same depth); put back afterwards.
        let mut early_back: Vec<D3D12_RESOURCE_BARRIER> = Vec::new();
        if let (Some(la), Some(planes), Some((scene_now, _))) = (early_list, early_states, &scene) {
            let pointer = scene_now.resource.as_raw() as usize;
            match snapshot.as_mut().filter(|s| s.1 == pointer) {
                Some(snap) => {
                    let mut first: Vec<D3D12_RESOURCE_BARRIER> = Vec::new();
                    for (k, state) in planes.iter().enumerate() {
                        first.push(transition(&scene_now.resource, k as u32, D3D12_RESOURCE_STATES(*state), D3D12_RESOURCE_STATE_COPY_SOURCE));
                    }
                    if !snap.2 {
                        first.push(transition(&snap.0, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE | D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE, D3D12_RESOURCE_STATE_COPY_DEST));
                    }
                    la.ResourceBarrier(&first);
                    la.CopyResource(&snap.0, &scene_now.resource);
                    snap.2 = false;
                    let mut then: Vec<D3D12_RESOURCE_BARRIER> = (0..planes.len()).map(|k| transition(&scene_now.resource, k as u32, D3D12_RESOURCE_STATE_COPY_SOURCE, D3D12_RESOURCE_STATE_DEPTH_WRITE)).collect();
                    then.push(transition(&snap.0, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_COPY_DEST, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE | D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE));
                    // (IMPL_ER56) every eighth frame the stencil plane of the snapshot (the G-buffer's, without him) goes to
                    // the CPU: the taps around his feet give the value his pixels take
                    let feet_px = feet_clip.filter(|f| f.w > 0.3).map(|f| ((f.x / f.w * 0.5 + 0.5) * width as f32, (0.5 - f.y / f.w * 0.5) * height as f32));
                    let stencil_copy = STENCIL_WANTED.load(Ordering::Relaxed) && *frame_no % 8 == 0 && feet_px.is_some() && snap.0.GetDesc().Format == DXGI_FORMAT_R32G8X24_TYPELESS;
                    if stencil_copy {
                        then[planes.len()] = transition(&snap.0, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_COPY_DEST, D3D12_RESOURCE_STATE_COPY_SOURCE);
                    }
                    la.ResourceBarrier(&then);
                    if let (true, Some((fx, fy))) = (stencil_copy, feet_px) {
                        if slot.stencil_rb.is_none() {
                            let desc = snap.0.GetDesc();
                            let mut footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default();
                            let mut total = 0u64;
                            device.GetCopyableFootprints(&desc, 1, 1, 0, Some(&mut footprint), None, None, Some(&mut total));
                            footprint.Offset = 0;
                            slot.stencil_rb = committed(device, D3D12_HEAP_TYPE_READBACK, &buffer_desc(total.max(256)), D3D12_RESOURCE_STATE_COPY_DEST, None).ok().map(|rb| (rb, footprint));
                        }
                        if let Some((rb, footprint)) = slot.stencil_rb.as_ref() {
                            let to = D3D12_TEXTURE_COPY_LOCATION { pResource: std::mem::transmute_copy(rb), Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT, Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { PlacedFootprint: *footprint } };
                            let from = D3D12_TEXTURE_COPY_LOCATION { pResource: std::mem::transmute_copy(&snap.0), Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX, Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { SubresourceIndex: 1 } };
                            la.CopyTextureRegion(&to, 0, 0, 0, &from, None);
                            let scale = height as f32 / 1440.0;
                            slot.stencil_taps = [(0.0, 4.0), (-16.0, 6.0), (16.0, 6.0), (0.0, 18.0), (-8.0, 12.0), (8.0, 12.0), (0.0, -4.0)].iter().map(|(dx, dy)| (((fx + dx * scale).clamp(0.0, width as f32 - 1.0)) as u32, ((fy + dy * scale).clamp(0.0, height as f32 - 1.0)) as u32)).collect();
                            slot.stencil_pending = true;
                        }
                        la.ResourceBarrier(&[transition(&snap.0, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_COPY_SOURCE, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE | D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE)]);
                    }
                }
                None => {
                    let first: Vec<D3D12_RESOURCE_BARRIER> = planes.iter().enumerate().map(|(k, state)| transition(&scene_now.resource, k as u32, D3D12_RESOURCE_STATES(*state), D3D12_RESOURCE_STATE_DEPTH_WRITE)).collect();
                    la.ResourceBarrier(&first);
                }
            }
            early_back = planes.iter().enumerate().map(|(k, state)| transition(&scene_now.resource, k as u32, D3D12_RESOURCE_STATE_DEPTH_WRITE, D3D12_RESOURCE_STATES(*state))).collect();
            la.OMSetRenderTargets(0, None, false, Some(&depth_view));
            la.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: stage.0, TopLeftY: stage.1, Width: stage.2, Height: stage.3, MinDepth: 0.0, MaxDepth: 1.0 }]);
            la.RSSetScissorRects(&[RECT { left: 0, top: 0, right: width as i32, bottom: height as i32 }]);
            la.SetGraphicsRootSignature(&*root);
            la.SetDescriptorHeaps(&[Some(assets.srv_heap.clone()), Some(sampler_heap.clone())]);
            la.SetGraphicsRootConstantBufferView(0, early_constants);
            la.SetGraphicsRootDescriptorTable(4, sampler_heap.GetGPUDescriptorHandleForHeapStart());
            la.SetGraphicsRootShaderResourceView(5, sampler_index.GetGPUVirtualAddress());
            la.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        }
        let mut early_draws = 0u32;
        // (IMPL_ER56) his STENCIL: `r.stencil_ref` when set, else the ground's at his feet (none known yet: not written)
        // (ER88) `r.taa_char` (default 1): the value Elden Ring's own characters carry (his_stencil)
        let stencil_value = his_stencil(STENCIL_FIXED.load(Ordering::Relaxed), TAA_CHAR.load(Ordering::Relaxed), *stencil_now)
        .filter(|_| STENCIL_WANTED.load(Ordering::Relaxed));
        let early_at = match (early_at, stencil_value, early_stencil_base(pipelines.len(), assets.library.slots.len())) {
            (Some(_), Some(v), Some(base)) => {
                if let Some(la) = early_list {
                    la.OMSetStencilRef(v as u32);
                }
                STENCIL_USED.store(v as u32 + 1, Ordering::Relaxed);
                Some(base)
            }
            (at, ..) => {
                STENCIL_USED.store(0, Ordering::Relaxed);
                at
            }
        };

        // Opaque and masked slots first, the translucent ones (eye highlight, lashes) over them.
        let srv_start = assets.srv_heap.GetGPUDescriptorHandleForHeapStart();
        let mut drawn = 0u32;
        let mut skin: Vec<f32> = Vec::with_capacity(16 * (1 + MAX_JOINTS_OUTFIT));
        // (ER90) 0 Sora, 1 the Keyblade, 2 + model index a form weapon
        let mut skin_made: Vec<Option<u64>> = vec![None; 2 + assets.models.len() + routed_count];
        // (party VF-b) the draw items: every model once, a routed fragment model once per routed instance (its form's index)
        let items: Vec<(usize, Option<usize>)> = assets
            .models
            .iter()
            .enumerate()
            .flat_map(|(k, m)| match m.mesh.strip_prefix(crate::xsiweapon::PREFIX).filter(|n| crate::xsiweapon::is_shard_model(n)) {
                Some(n) => input.forms.iter().enumerate().filter(|(_, f)| f.model == n).map(|(i, _)| (k, Some(i))).collect::<Vec<_>>(),
                None => vec![(k, None)],
            })
            .collect();
        let routed_base = input.forms.len() - routed_count;
        // (FP) a routed fragment's puts end `routed_reserve` before the buffer's end: what the frame still needs once the
        // fragments start (the skins of the models after the first fragment and of those drawn in the translucent pass only,
        // the material blocks of every model's draws, the party's), so that a fragment that does not fit skips itself (counted,
        // `routed_skipped`) and never fails the frame
        let routed_limit = match items.iter().position(|(_, i)| i.is_some()) {
            Some(first) => {
                let has_opaque = |k: usize| assets.models[k].draws.iter().any(|d| !assets.library.slots[d.slot].translucent);
                let later_skins = items.iter().enumerate().filter(|(n, (k, i))| i.is_none() && (*n > first || !has_opaque(*k))).count();
                #[allow(unused_mut)]
                let mut later_blocks: usize = items.iter().filter(|(_, i)| i.is_none()).map(|(k, _)| assets.models[*k].draws.len()).sum();
                #[cfg(feature = "party")]
                if let Some(plan) = party_plan.as_ref() {
                    later_blocks += party::material_blocks(plan, party);
                }
                cap.saturating_sub(routed_reserve(later_skins, later_blocks))
            }
            None => cap,
        };
        let mut skipped_routed = vec![false; routed_count];
        if prev_skin.len() < skin_made.len() {
            prev_skin.resize(skin_made.len(), (Vec::new(), 0, Trs::IDENTITY));
        }
        let mut velo_skins: Vec<(usize, u64, u64)> = Vec::new();
        // (IMPL_ER78) the G-buffer draws of this frame: (model, slot, index count, first index, base vertex, skin, skin a frame
        // ago, material constants) — PER DRAW: every opaque / masked slot of every model drawn (Sora, the Keyblade, an outfit,
        // and the party's models when they join `assets.models`)
        let mut gb_draws: Vec<(usize, usize, u32, u32, i32, u64, u64, u64)> = Vec::new();
        // (ER87) THE HAIR FRINGE (hair.rs): the hair slots' draws, drawn once more after all of his own with the fringe states
        let hair_set: Option<Vec<Option<ID3D12PipelineState>>> = glow.hair_sets.get(&(key, false)).filter(|s| crate::hair::on() && s.len() == assets.library.slots.len()).cloned();
        let mut hair_draws: Vec<(usize, usize, u32, u32, i32, u64, u64, u64)> = Vec::new();
        // (hair.rs `late`: when his motion vectors (the game: r.velocity) or the late re-composite (r.late_sora) split the game's
        // forward submission and he is nearer than `r.hair_late_far` m, the fringe and his translucent draws go to that split:
        // after the game's forward stage (its depth-tested sky and far field would cover pixels that carry no depth of his),
        // before its TAA. Farther away the fringe stays here (the forward stage's fog then lies over it as over him).)
        let split_ok = hdr && ((options.velocity != 0 && options.jitter && options.at == crate::At::Scene && !options.at_present && light_now.as_ref().is_some_and(|l| l.scene)) || recomp_split_wanted());
        let near = input.body.as_ref().is_some_and(|b| (b.component_to_world.translation - camera_game).length() * 0.01 < crate::hair::late_far()); // (game space, cm)
        let hair_late_now = hair_set.is_some() && crate::hair::late() && split_ok && near && scene.is_some();
        // (ER91) THE 4x HAIR PASS (hair91.rs): at the same split, nearer than `r.hair_msaa_far` m, when the outfit has its
        // states; the box around his head (the head joint's ball); off the screen: none
        let h91_box: Option<[u32; 4]> = if hair_set.is_some() && crate::hair91::on() && split_ok && scene.is_some() && glow.hair_msaa.contains_key(&key) {
            input.body.as_ref().filter(|b| (b.component_to_world.translation - camera_game).length() * 0.01 < crate::hair91::far()).and_then(|body| {
                let bone = *assets.bones.get("atama")?;
                let head = body.component.get(bone)?.then(&body.component_to_world).translation;
                let neck = assets.bones.get("kubi").and_then(|b| body.component.get(*b)).map(|t| t.then(&body.component_to_world).translation);
                let up = neck.map_or(Vec3::Z, |n| (head - n).normalize_or(Vec3::Z));
                head.is_finite().then(|| crate::hair91::head_box(&clip, head + up * crate::hair91::ABOVE_CM, width, height)).flatten()
            })
        } else {
            None
        };
        let hair_late_now = hair_late_now || (h91_box.is_some() && hair_set.is_some());
        // (HAIRPERF) his hair's coverage at one point a pixel while he is big on screen: the pixels a centimetre at his head
        // covers (his feet without the joint), `r.hair_point_px` (hair.rs); handed to the hair shaders as kh.fade.z
        let hair_point = hair_set.is_some()
            && input.body.as_ref().is_some_and(|body| {
                let head = assets.bones.get("atama").and_then(|b| body.component.get(*b)).map(|t| t.then(&body.component_to_world).translation).filter(|h| h.is_finite());
                let d = (head.unwrap_or(body.component_to_world.translation) - camera_game).length().max(1.0); // (game space, cm)
                crate::hair::point_wanted(height as f32 / (2.0 * tan_y.max(1.0e-3) * d))
            });
        let mut late_translucent: Vec<crate::hair::HairDraw> = Vec::new();
        crate::passtime::stamp(device, queue, list, slot_index, 3); // (HAIRPERF) his colour draws
        for translucent in [false, true] {
            // (ER87) not late: the hair fringe between his opaque / masked and his translucent draws
            if let (true, Some(set), false) = (translucent, hair_set.as_ref(), hair_late_now) {
                crate::hair::draw_fringe(list, set, &hair_draws, &|m| assets.models.get(m).map(|model| (model.vertex_view, model.index_view)), srv_start.ptr, (SLOT_TEXTURES * assets.srv_size) as u64);
            }
            for &(model_index, inst) in &items {
                let model = &assets.models[model_index];
                // The model's pose: component-space bones and where the component stands.
                let (to_world, pose): (Trs, &[Trs]) = match model.mesh {
                    "sora" => match &input.body {
                        Some(body) => (body.component_to_world, &body.component),
                        None => continue,
                    },
                    // (ER90, XSI request 1) a form weapon of an XS move: where the host hung it this frame (in an outfit's
                    // hand as the Keyblade is); (XW) the pose of its own clip (none: its reference / bind pose)
                    m if m.starts_with(crate::xsiweapon::PREFIX) => match inst.and_then(|i| input.forms.get(i)).or_else(|| input.forms.iter().find(|f| m.strip_prefix(crate::xsiweapon::PREFIX) == Some(f.model.as_str()))) {
                        Some(f) => (
                            match (assets.models.iter().find_map(|m| m.retarget.as_ref()), &input.body) {
                                (Some(r), Some(body)) => r.weapon_to_world(&f.to_world, &body.component, &r.frame(input.clock, &body.component_to_world, &body.local, &body.component).1, &body.component_to_world),
                                _ => f.to_world,
                            },
                            if !f.component.is_empty() && f.component.len() == model.reference.len() { &f.component } else { &model.reference },
                        ),
                        None => continue,
                    },
                    _ => match &input.weapon {
                        // (IMPL_ER59) with an outfit on: the Keyblade in the outfit's hand (its arm has its own length)
                        Some(weapon) if weapon.opacity > 0.0 => (
                            match (assets.models.iter().find_map(|m| m.retarget.as_ref()), &input.body) {
                                (Some(r), Some(body)) => r.weapon_to_world(&weapon.to_world, &body.component, &r.frame(input.clock, &body.component_to_world, &body.local, &body.component).1, &body.component_to_world),
                                _ => weapon.to_world,
                            },
                            if weapon.component.len() == model.reference.len() { &weapon.component } else { &model.reference },
                        ),
                        _ => continue,
                    },
                };
                if !model.draws.iter().any(|d| assets.library.slots[d.slot].translucent == translucent) {
                    continue;
                }
                // (IMPL_ER59) an outfit: its own skeleton, posed from his frame
                let outfit_pose;
                let pose: &[Trs] = match (&model.retarget, &input.body) {
                    (Some(r), Some(body)) if skin_made[if model.mesh == "sora" { 0 } else { 1 }].is_none() => {
                        outfit_pose = r.frame(input.clock, &body.component_to_world, &body.local, &body.component);
                        &outfit_pose.1
                    }
                    _ => pose,
                };
                // The skin's constants: once a model and frame (both passes use them).
                let which = match (model.mesh, inst) {
                    ("sora", _) => 0,
                    // (party VF-b) a routed fragment instance: its own skin
                    (_, Some(i)) => 2 + assets.models.len() + i.saturating_sub(routed_base),
                    (m, None) if m.starts_with(crate::xsiweapon::PREFIX) => 2 + model_index,
                    _ => 1,
                };
                let mut this_before = None;
                // (FP) a routed fragment's puts end at `routed_limit`; one that does not fit is skipped, not the frame
                let routed_at = inst.map(|i| i.saturating_sub(routed_base));
                limit.set(if routed_at.is_some() { routed_limit } else { cap });
                let skin_constants = match skin_made[which] {
                    Some(at) => at,
                    None => {
                        skin.clear();
                        skin.extend_from_slice(&math::game_from_pack(&to_world).to_cols_array());
                        for (joint, bone) in model.joint_bones.iter().enumerate() {
                            let matrix = match bone.and_then(|b| pose.get(b)) {
                                Some(t) => math::joint_in_pack(t) * model.inverse_bind[joint],
                                None => Mat4::IDENTITY,
                            };
                            skin.extend_from_slice(&matrix.to_cols_array());
                        }
                        // (party VF-b) a routed fragment (a static mesh: every vertex on its one joint) uploads its two matrices only
                        if inst.is_none() {
                            skin.resize(16 * (1 + if model.retarget.is_some() { MAX_JOINTS_OUTFIT } else { MAX_JOINTS }), 0.0);
                        }
                        // (ER79) the outfit's leg guard in the palette's last entries (the skin_cg shader reads them)
                        if let Some(r) = &model.retarget {
                            r.write_leg_guard(&mut skin, pose);
                        }
                        let at = match (put(&skin), routed_at) {
                            (Some(at), _) => at,
                            (None, Some(r)) => {
                                skipped_routed[r] = true;
                                continue;
                            }
                            (None, None) => return Err(overflow()),
                        };
                        // The same of the frame before (his motion vectors); none yet: no motion.
                        // (RSF) only the skin drawn in the frame just before, and not across a jump or a turn (INT's snap rule:
                        // the same rule as every other drawn KH character's, motion_continues)
                        let before = if prev_skin[which].0.len() == skin.len() && motion_continues(prev_skin[which].1, &prev_skin[which].2, *frame_no, &to_world) {
                            match (put(&prev_skin[which].0), routed_at) {
                                (Some(b), _) => b,
                                (None, Some(r)) => {
                                    skipped_routed[r] = true;
                                    continue;
                                }
                                (None, None) => return Err(overflow()),
                            }
                        } else {
                            at
                        };
                        skin_made[which] = Some(at);
                        prev_skin[which].0.clone_from(&skin);
                        (prev_skin[which].1, prev_skin[which].2) = (*frame_no, to_world);
                        velo_skins.push((model_index, at, before));
                        this_before = Some(before);
                        at
                    }
                };
                // (ER90) a form weapon takes the Keyblade's cooked bounds (its material's); (XW) with its own materials its own
                let object = ObjectRows::new(&to_world, assets.library.bounds_of(if model.mesh.starts_with(crate::xsiweapon::PREFIX) && crate::xsiweapon::table().is_none_or(|t| t.own_materials.is_none()) { "kingdom_key" } else { model.mesh }));
                list.IASetVertexBuffers(0, Some(&[model.vertex_view]));
                list.IASetIndexBuffer(Some(&model.index_view));
                let skin_before = this_before.unwrap_or_else(|| velo_skins.iter().find(|(m, at, _)| *m == model_index && (inst.is_none() || *at == skin_constants)).map_or(skin_constants, |(_, _, before)| *before));
                list.SetGraphicsRootConstantBufferView(2, skin_constants);
                list.SetGraphicsRootConstantBufferView(6, skin_before);
                if let (Some(la), false) = (early_list, translucent) {
                    la.IASetVertexBuffers(0, Some(&[model.vertex_view]));
                    la.IASetIndexBuffer(Some(&model.index_view));
                    la.SetGraphicsRootConstantBufferView(2, skin_constants);
                    la.SetGraphicsRootConstantBufferView(6, skin_before);
                }
                for draw in &model.draws {
                    let material = &assets.library.slots[draw.slot];
                    if material.translucent != translucent || (options.only >= 0 && options.only as usize != draw.slot) {
                        continue;
                    }
                    let mut params = assets.library.params(material, &object, &rig, assets.khs.as_ref().map(|(_, s)| s), input.clock, options.exposure, options.debug);
                    params.sh = sh;
                    // (ER79) a two-sided cloth slot: its interior factor (the _bli shaders shade its back faces as the inside)
                    params.khs.w = material.scalars.get(crate::outfit::INTERIOR_SCALAR).copied().unwrap_or(0.0);
                    params.fade.z = f32::from(u8::from(hair_point)); // (HAIRPERF) read by the ER91 hair shaders only
                    params.env.y = env_level;
                    (params.env.z, params.env.w) = (env_tint.x - 1.0, env_tint.z - 1.0);
                    let material_constants = match (put(params.as_floats()), routed_at) {
                        (Some(at), _) => at,
                        (None, Some(r)) => {
                            skipped_routed[r] = true;
                            continue;
                        }
                        (None, None) => return Err(overflow()),
                    };
                    // (ER87) late: a translucent cover that hair can lie behind (San Fransokyo's visor) after the fringe at the
                    // velocity split (draw_split); the eyes' lenses and lashes stay here (hair.rs `defer_translucent`)
                    if translucent && hair_late_now && crate::hair::defer_translucent(&material.owner) {
                        late_translucent.push((model_index, draw.slot, draw.index_count, draw.first_index, draw.base_vertex, skin_constants, skin_before, material_constants));
                        continue;
                    }
                    list.SetPipelineState(&pipelines[draw.slot]);
                    list.SetGraphicsRootConstantBufferView(1, material_constants);
                    list.SetGraphicsRootDescriptorTable(3, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: srv_start.ptr + (draw.slot * SLOT_TEXTURES * assets.srv_size) as u64 });
                    let share = if spared_constants != frame_constants { crate::blood::share(material) } else { 1.0 };
                    let spared = share < 1.0;
                    if spared {
                        list.SetGraphicsRootConstantBufferView(0, if share <= 0.0 { spared_constants } else { face_constants });
                    }
                    list.DrawIndexedInstanced(draw.index_count, 1, draw.first_index, draw.base_vertex, 0);
                    if spared {
                        list.SetGraphicsRootConstantBufferView(0, frame_constants);
                    }
                    drawn += 1;
                    // (ER87) a hair slot: its fringe after all of his draws (below)
                    if !translucent && hair_set.as_ref().is_some_and(|s| s.get(draw.slot).is_some_and(Option::is_some)) {
                        hair_draws.push((model_index, draw.slot, draw.index_count, draw.first_index, draw.base_vertex, skin_constants, skin_before, material_constants));
                    }
                    // (IMPL_ER80) the opaque / masked draws once more into the glow source (below)
                    if (glow_plan.is_some() || shaft_on) && !translucent {
                        glow_draws.push((model_index, draw.slot, draw.index_count, draw.first_index, draw.base_vertex, skin_constants, skin_before, material_constants));
                    }
                    // (IMPL_ER53) the same draw into the early depth (opaque and masked slots: those that write depth)
                    if let (Some(la), Some(base), false) = (early_list, early_at, translucent) {
                        if let Some(state) = pipelines.get(base + draw.slot) {
                            la.SetPipelineState(state);
                            la.SetGraphicsRootConstantBufferView(1, material_constants);
                            la.SetGraphicsRootDescriptorTable(3, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: srv_start.ptr + (draw.slot * SLOT_TEXTURES * assets.srv_size) as u64 });
                            la.DrawIndexedInstanced(draw.index_count, 1, draw.first_index, draw.base_vertex, 0);
                            early_draws += 1;
                            // (IMPL_ER78) and once more into the game's G-buffer, after all of his depth is in (below)
                            if hy_on {
                                gb_draws.push((model_index, draw.slot, draw.index_count, draw.first_index, draw.base_vertex, skin_constants, skin_before, material_constants));
                            }
                        }
                    }
                }
            }
        }
        crate::passtime::stamp(device, queue, list, slot_index, 4); // (HAIRPERF) the rest
        // (FP) the whole buffer again for what follows; the routed fragments that did not fit, counted in the log
        limit.set(cap);
        routed_skipped(skipped_routed.iter().filter(|s| **s).count(), routed_count);
        // (ER87) THE HAIR FRINGE: the partly covered hair pixels, alpha = their coverage, depth GREATER than what his opaque /
        // masked draws left (only where they lie in front of his own surface or of the background), no depth write; after his
        // translucent draws (a strand end in front of an eye lies over it)
        // (hair.rs `late`, default: when his motion vectors are drawn at the game's velocity split, the fringe is drawn there
        // instead: after the game's forward stage (its sky, far field and translucents, which are depth tested and would
        // cover the fringe's pixels: they carry no depth of his), before its TAA)
        // (late: the fringe and then his translucent draws (eye lenses, lashes, a visor), so that a strand behind a visor stays
        // behind it; not late: the fringe was drawn before his translucent draws, in the loop above)
        glow.hair_late = None;
        if let (true, Some((scene_now, _))) = (hair_late_now && !(hair_draws.is_empty() && late_translucent.is_empty()), &scene) {
            // (ER91) the 4x pass's parameter block (its box) in this slot's upload buffer
            let snapshot = early_reads.as_ref().map(|s| s.as_raw() as usize);
            let h91 = h91_box.and_then(|b| put(&crate::hair91::params(b, snapshot.is_some())).map(|at| (b, at, stencil_value, snapshot)));
            glow.hair_late = Some(HairLate { slot: slot_index, key, target: back.as_raw() as usize, depth: scene_now.resource.as_raw() as usize, width, height, frame_constants, draws: hair_draws.clone(), translucent: std::mem::take(&mut late_translucent), h91 });
        }
        // (ER party S4) THE PARTY'S DRAWS (opaque / masked, then translucent), each friend's heap, frame block and pipeline
        // states; Sora's root state again after them
        #[cfg(feature = "party")]
        if let Some(plan) = party_plan.as_mut() {
            let passes_started = Instant::now();
            let cx = party::PassCx { key, sampler_heap: &*sampler_heap, sora_heap: &assets.srv_heap, frame_constants, early_constants, rig: &rig, sh, env_level, env_tint, clock: input.clock, exposure: options.exposure, debug: options.debug };
            let early_kind = early_at.map(|_| stencil_value.is_some() && early_stencil_base(pipelines.len(), assets.library.slots.len()).is_some());
            for translucent in [false, true] {
                if party::draw_pass(plan, party, translucent, list, early_list, early_kind, stencil_value, hy_on, &cx, &mut put).is_err() {
                    return Err(overflow());
                }
            }
            drawn += plan.drawn;
            party.draws += plan.drawn;
            if early_kind.is_some() {
                party.early_draws += plan.glow.len() as u32;
            }
            party::tally_frame(plan, party);
            party_us += passes_started.elapsed().as_micros() as u64;
        }
        #[cfg(feature = "party")]
        let party_none = party_plan.as_ref().is_none_or(|p| p.glow.is_empty());
        #[cfg(not(feature = "party"))]
        let party_none = true;
        // (IMPL_ER80) THE GLOW MASK: his opaque / masked draws once more, into the game's glow source (the deferred composite's
        // second target, which the post chain blurs and adds over the picture), with his colour shaders (the same pixels, the
        // same coverage), depth GREATER_EQUAL against what his colour draw just left, blend to 0 (opaque) or x (1 - coverage)
        // (masked): what glowed BEHIND him (the Erdtree) no longer glows through him; anything the forward stage draws IN FRONT
        // of him later adds its glow as before. From its resting state to RENDER_TARGET and back; the scene target again after.
        if let (Some((g, state, gkey)), false) = (glow_plan.filter(|_| GLOW_MASK.load(Ordering::Relaxed)), glow_draws.is_empty() && party_none) {
            let raw = g as *mut c_void;
            let mut done = false;
            if let (Some(resource), Some(set)) = (ID3D12Resource::from_raw_borrowed(&raw), glow.sets.get(&gkey)) {
                if glow.rtv.is_none() {
                    glow.rtv = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV, NumDescriptors: FRAME_SLOTS as u32, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE, NodeMask: 0 }).ok();
                }
                if let Some(heap) = glow.rtv.as_ref() {
                    let view = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: heap.GetCPUDescriptorHandleForHeapStart().ptr + slot_index * device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV) as usize };
                    device.CreateRenderTargetView(resource, None, view);
                    let barrier = state != D3D12_RESOURCE_STATE_RENDER_TARGET.0;
                    if barrier {
                        list.ResourceBarrier(&[transition(resource, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATES(state), D3D12_RESOURCE_STATE_RENDER_TARGET)]);
                    }
                    list.OMSetRenderTargets(1, Some(&view), false, Some(&depth_view));
                    let mut bound_model = usize::MAX;
                    for (model_index, slot_k, count, first_index, base_vertex, skin_now, skin_then, material) in &glow_draws {
                        let (Some(model), Some(pso)) = (assets.models.get(*model_index), set.get(*slot_k)) else { continue };
                        if bound_model != *model_index {
                            list.IASetVertexBuffers(0, Some(&[model.vertex_view]));
                            list.IASetIndexBuffer(Some(&model.index_view));
                            bound_model = *model_index;
                        }
                        list.SetGraphicsRootConstantBufferView(2, *skin_now);
                        list.SetGraphicsRootConstantBufferView(6, *skin_then);
                        list.SetPipelineState(pso);
                        list.SetGraphicsRootConstantBufferView(1, *material);
                        list.SetGraphicsRootDescriptorTable(3, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: srv_start.ptr + (slot_k * SLOT_TEXTURES * assets.srv_size) as u64 });
                        list.DrawIndexedInstanced(*count, 1, *first_index, *base_vertex, 0);
                    }
                    // (ER87) the hair fringe out of the glow source too: x (1 - its coverage), in front of what is masked above
                    if let Some(set) = glow.hair_sets.get(&(gkey, true)).filter(|s| crate::hair::on() && s.len() == assets.library.slots.len() && !hair_draws.is_empty()) {
                        crate::hair::draw_fringe(list, set, &hair_draws, &|m| assets.models.get(m).map(|model| (model.vertex_view, model.index_view)), srv_start.ptr, (SLOT_TEXTURES * assets.srv_size) as u64);
                    }
                    // (ER party S4) the friends' opaque draws out of the glow source too (their own glow-mask states)
                    #[cfg(feature = "party")]
                    if let Some(plan) = party_plan.as_ref() {
                        let cx = party::PassCx { key, sampler_heap: &*sampler_heap, sora_heap: &assets.srv_heap, frame_constants, early_constants, rig: &rig, sh, env_level, env_tint, clock: input.clock, exposure: options.exposure, debug: options.debug };
                        party::glow_pass(plan, party, gkey, list, &cx);
                    }
                    if barrier {
                        list.ResourceBarrier(&[transition(resource, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATES(state))]);
                    }
                    list.OMSetRenderTargets(1, Some(&target_view), false, Some(&depth_view));
                    glow.drawn += 1;
                    done = true;
                }
            }
            if !done {
                glow.skipped += 1;
                glow.reason = "the glow source or its view could not be taken in this frame";
            }
        }
        // (IMPL_ER80b) HIS MASK for the game's light-shaft composite (shaft.rs): the scene depth copied right after his draw,
        // its stencil cleared, then 1 written by his opaque / masked draws with his stencil-writing early-depth states (depth
        // GREATER_EQUAL: only where his surface is the visible one); left in DEPTH_READ, named to shaft.rs (the game's
        // composite draw is made with a stencil test against it). Any failure: no mask this frame (the shafts as they are).
        if let (true, false, Some((scene_now, dsv_format)), Some(base)) = (shaft_on, glow_draws.is_empty() && party_none, &scene, early_stencil_base(pipelines.len(), assets.library.slots.len())) {
            let pointer = scene_now.resource.as_raw() as usize;
            let dsv_size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_DSV) as usize;
            if glow.shaft.as_ref().map(|k| k.2) != Some(pointer) {
                if let Some((old, heap, _)) = glow.shaft.take() {
                    glow.old_masks.push((old, heap));
                }
                let desc = scene_now.resource.GetDesc();
                let made = (|| -> windows::core::Result<(ID3D12Resource, ID3D12DescriptorHeap)> {
                    let mask = committed(device, D3D12_HEAP_TYPE_DEFAULT, &desc, D3D12_RESOURCE_STATE_DEPTH_READ | D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, None)?;
                    let heap: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_DSV, NumDescriptors: 2, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE, NodeMask: 0 })?;
                    let start = heap.GetCPUDescriptorHandleForHeapStart();
                    let view = |flags: D3D12_DSV_FLAGS| D3D12_DEPTH_STENCIL_VIEW_DESC { Format: *dsv_format, ViewDimension: D3D12_DSV_DIMENSION_TEXTURE2D, Flags: flags, Anonymous: D3D12_DEPTH_STENCIL_VIEW_DESC_0 { Texture2D: D3D12_TEX2D_DSV { MipSlice: 0 } } };
                    device.CreateDepthStencilView(&mask, Some(&view(D3D12_DSV_FLAG_NONE)), start);
                    device.CreateDepthStencilView(&mask, Some(&view(D3D12_DSV_FLAG_READ_ONLY_DEPTH | D3D12_DSV_FLAG_READ_ONLY_STENCIL)), D3D12_CPU_DESCRIPTOR_HANDLE { ptr: start.ptr + dsv_size });
                    Ok((mask, heap))
                })();
                match made {
                    Ok((mask, heap)) => {
                        rlog!("render: (IMPL_ER80b) his mask for the game's light shafts: {}x{} format {}", desc.Width, desc.Height, desc.Format.0);
                        glow.shaft = Some((mask, heap, pointer));
                    }
                    Err(e) => rlog!("render: (IMPL_ER80b) his mask for the game's light shafts cannot be made ({e}): the shafts stay as they are"),
                }
            }
            if let Some((mask, heap, _)) = glow.shaft.as_ref() {
                let write_view = heap.GetCPUDescriptorHandleForHeapStart();
                let read_view = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: write_view.ptr + dsv_size };
                let mut first = vec![transition(mask, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_DEPTH_READ | D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, D3D12_RESOURCE_STATE_COPY_DEST)];
                let mut then = vec![transition(mask, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_COPY_DEST, D3D12_RESOURCE_STATE_DEPTH_WRITE)];
                for plane in 0..2u32 {
                    first.push(transition(&scene_now.resource, plane, D3D12_RESOURCE_STATE_DEPTH_WRITE, D3D12_RESOURCE_STATE_COPY_SOURCE));
                    then.push(transition(&scene_now.resource, plane, D3D12_RESOURCE_STATE_COPY_SOURCE, D3D12_RESOURCE_STATE_DEPTH_WRITE));
                }
                list.ResourceBarrier(&first);
                list.CopyResource(mask, &scene_now.resource);
                list.ResourceBarrier(&then);
                list.ClearDepthStencilView(write_view, D3D12_CLEAR_FLAG_STENCIL, 0.0, 0, None);
                list.OMSetRenderTargets(0, None, false, Some(&write_view));
                list.OMSetStencilRef(1);
                let mut bound_model = usize::MAX;
                for (model_index, slot_k, count, first_index, base_vertex, skin_now, skin_then, material) in &glow_draws {
                    let (Some(model), Some(pso)) = (assets.models.get(*model_index), pipelines.get(base + slot_k)) else { continue };
                    if bound_model != *model_index {
                        list.IASetVertexBuffers(0, Some(&[model.vertex_view]));
                        list.IASetIndexBuffer(Some(&model.index_view));
                        bound_model = *model_index;
                    }
                    list.SetGraphicsRootConstantBufferView(2, *skin_now);
                    list.SetGraphicsRootConstantBufferView(6, *skin_then);
                    list.SetPipelineState(pso);
                    list.SetGraphicsRootConstantBufferView(1, *material);
                    list.SetGraphicsRootDescriptorTable(3, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: srv_start.ptr + (slot_k * SLOT_TEXTURES * assets.srv_size) as u64 });
                    list.DrawIndexedInstanced(*count, 1, *first_index, *base_vertex, 0);
                }
                // (ER party S4) the friends' surfaces into the mask too
                #[cfg(feature = "party")]
                if let Some(plan) = party_plan.as_ref() {
                    let cx = party::PassCx { key, sampler_heap: &*sampler_heap, sora_heap: &assets.srv_heap, frame_constants, early_constants, rig: &rig, sh, env_level, env_tint, clock: input.clock, exposure: options.exposure, debug: options.debug };
                    party::shaft_pass(plan, party, list, &cx);
                }
                list.OMSetStencilRef(0);
                list.ResourceBarrier(&[transition(mask, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_DEPTH_WRITE, D3D12_RESOURCE_STATE_DEPTH_READ | D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE)]);
                let mut published = vec![(read_view.ptr, width, height, dsv_format.0)];
                // (IMPL_ER80b) THE REDUCED SIZES the game's composite was seen drawing at (shaft.rs wanted_sizes): each a small
                // depth-stencil target whose stencil is 1 where ANY full-size texel under its pixel is his (maskdown.ps.dxil
                // reading the full mask's stencil). Without maskdown.ps.dxil: the full size only (the reason in the log).
                let wanted = crate::shaft::wanted_sizes();
                let stencil_view = match *dsv_format {
                    DXGI_FORMAT_D32_FLOAT_S8X24_UINT => Some(DXGI_FORMAT_X32_TYPELESS_G8X24_UINT),
                    DXGI_FORMAT_D24_UNORM_S8_UINT => Some(DXGI_FORMAT_X24_TYPELESS_G8_UINT),
                    _ => None,
                };
                if let (false, Some(stencil_view)) = (wanted.is_empty(), stencil_view) {
                    if glow.maskdown.as_ref().map(|m| m.2) != Some(mask.as_raw() as usize) && !glow.maskdown_failed {
                        let pack = SHARED.lock().ok().and_then(|g| g.as_ref().map(|s| s.pack.clone()));
                        let made = pack.and_then(|pack| {
                            let dir = pack.join("materials").join("er");
                            let (vertex, pixel) = (std::fs::read(dir.join("blob.vs.dxil")).ok()?, std::fs::read(dir.join("maskdown.ps.dxil")).ok()?);
                            let replace = D3D12_DEPTH_STENCILOP_DESC { StencilFailOp: D3D12_STENCIL_OP_KEEP, StencilDepthFailOp: D3D12_STENCIL_OP_KEEP, StencilPassOp: D3D12_STENCIL_OP_REPLACE, StencilFunc: D3D12_COMPARISON_FUNC_ALWAYS };
                            let pso = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
                                pRootSignature: std::mem::transmute_copy(&*root),
                                VS: D3D12_SHADER_BYTECODE { pShaderBytecode: vertex.as_ptr() as *const c_void, BytecodeLength: vertex.len() },
                                PS: D3D12_SHADER_BYTECODE { pShaderBytecode: pixel.as_ptr() as *const c_void, BytecodeLength: pixel.len() },
                                BlendState: D3D12_BLEND_DESC::default(),
                                SampleMask: u32::MAX,
                                RasterizerState: D3D12_RASTERIZER_DESC { FillMode: D3D12_FILL_MODE_SOLID, CullMode: D3D12_CULL_MODE_NONE, DepthClipEnable: false.into(), ..Default::default() },
                                DepthStencilState: D3D12_DEPTH_STENCIL_DESC { DepthEnable: false.into(), DepthWriteMask: D3D12_DEPTH_WRITE_MASK_ZERO, DepthFunc: D3D12_COMPARISON_FUNC_ALWAYS, StencilEnable: true.into(), StencilReadMask: 0xFF, StencilWriteMask: 0xFF, FrontFace: replace, BackFace: replace },
                                PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
                                NumRenderTargets: 0,
                                DSVFormat: *dsv_format,
                                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                                ..Default::default()
                            };
                            let state: ID3D12PipelineState = device.CreateGraphicsPipelineState(&pso).ok()?;
                            let heap: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, NumDescriptors: SLOT_TEXTURES as u32, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, NodeMask: 0 }).ok()?;
                            let size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV) as usize;
                            let view = D3D12_SHADER_RESOURCE_VIEW_DESC { Format: stencil_view, ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D, Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING, Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 { Texture2D: D3D12_TEX2D_SRV { MostDetailedMip: 0, MipLevels: 1, PlaneSlice: 1, ResourceMinLODClamp: 0.0 } } };
                            for k in 0..SLOT_TEXTURES {
                                device.CreateShaderResourceView(mask, Some(&view), D3D12_CPU_DESCRIPTOR_HANDLE { ptr: heap.GetCPUDescriptorHandleForHeapStart().ptr + k * size });
                            }
                            Some((state, heap, mask.as_raw() as usize))
                        });
                        match made {
                            Some(m) => glow.maskdown = Some(m),
                            None => {
                                glow.maskdown_failed = true;
                                rlog!("render: (IMPL_ER80b) his reduced-size masks cannot be made (materials/er/maskdown.ps.dxil missing?): the light shafts are masked only where the game composes them at full size");
                            }
                        }
                    }
                    if let Some((md, md_heap, _)) = glow.maskdown.as_ref() {
                        for (w, h) in wanted.iter().copied() {
                            if !glow.small_masks.iter().any(|m| m.0 == w && m.1 == h) {
                                let mut desc = mask.GetDesc();
                                (desc.Width, desc.Height) = (w as u64, h);
                                let made = (|| -> windows::core::Result<(ID3D12Resource, ID3D12DescriptorHeap)> {
                                    let small = committed(device, D3D12_HEAP_TYPE_DEFAULT, &desc, D3D12_RESOURCE_STATE_DEPTH_READ, None)?;
                                    let heap: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_DSV, NumDescriptors: 2, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE, NodeMask: 0 })?;
                                    let start = heap.GetCPUDescriptorHandleForHeapStart();
                                    let view = |flags: D3D12_DSV_FLAGS| D3D12_DEPTH_STENCIL_VIEW_DESC { Format: *dsv_format, ViewDimension: D3D12_DSV_DIMENSION_TEXTURE2D, Flags: flags, Anonymous: D3D12_DEPTH_STENCIL_VIEW_DESC_0 { Texture2D: D3D12_TEX2D_DSV { MipSlice: 0 } } };
                                    device.CreateDepthStencilView(&small, Some(&view(D3D12_DSV_FLAG_NONE)), start);
                                    device.CreateDepthStencilView(&small, Some(&view(D3D12_DSV_FLAG_READ_ONLY_DEPTH | D3D12_DSV_FLAG_READ_ONLY_STENCIL)), D3D12_CPU_DESCRIPTOR_HANDLE { ptr: start.ptr + dsv_size });
                                    Ok((small, heap))
                                })();
                                match made {
                                    Ok((small, heap)) => {
                                        rlog!("render: (IMPL_ER80b) his mask for the game's light shafts at {w}x{h} is made");
                                        glow.small_masks.push((w, h, small, heap));
                                    }
                                    Err(e) => rlog!("render: (IMPL_ER80b) his mask at {w}x{h} cannot be made ({e})"),
                                }
                            }
                        }
                        list.SetDescriptorHeaps(&[Some(md_heap.clone()), Some(sampler_heap.clone())]);
                        list.SetGraphicsRootDescriptorTable(3, md_heap.GetGPUDescriptorHandleForHeapStart());
                        list.SetPipelineState(md);
                        list.OMSetStencilRef(1);
                        for (w, h, small, heap) in glow.small_masks.iter().filter(|m| wanted.contains(&(m.0, m.1))) {
                            let mut block = frame.clone();
                            if block.len() >= 36 {
                                block[32..36].copy_from_slice(&[*w as f32, *h as f32, 1.0 / *w as f32, 1.0 / *h as f32]);
                            }
                            let Some(constants) = put(&block) else { break };
                            let write = heap.GetCPUDescriptorHandleForHeapStart();
                            list.ResourceBarrier(&[transition(small, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_DEPTH_READ, D3D12_RESOURCE_STATE_DEPTH_WRITE)]);
                            list.ClearDepthStencilView(write, D3D12_CLEAR_FLAG_STENCIL, 0.0, 0, None);
                            list.OMSetRenderTargets(0, None, false, Some(&write));
                            list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: *w as f32, Height: *h as f32, MinDepth: 0.0, MaxDepth: 1.0 }]);
                            list.RSSetScissorRects(&[RECT { left: 0, top: 0, right: *w as i32, bottom: *h as i32 }]);
                            list.SetGraphicsRootConstantBufferView(0, constants);
                            list.DrawInstanced(3, 1, 0, 0);
                            list.ResourceBarrier(&[transition(small, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_DEPTH_WRITE, D3D12_RESOURCE_STATE_DEPTH_READ)]);
                            published.push((write.ptr + dsv_size, *w, *h, dsv_format.0));
                        }
                        // (the root state of the colour list again)
                        list.SetDescriptorHeaps(&[Some(assets.srv_heap.clone()), Some(sampler_heap.clone())]);
                        list.SetGraphicsRootConstantBufferView(0, frame_constants);
                        list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: stage.0, TopLeftY: stage.1, Width: stage.2, Height: stage.3, MinDepth: 0.0, MaxDepth: 1.0 }]);
                        list.RSSetScissorRects(&[RECT { left: 0, top: 0, right: width as i32, bottom: height as i32 }]);
                        list.OMSetStencilRef(0);
                    }
                }
                list.OMSetRenderTargets(1, Some(&target_view), false, Some(&depth_view));
                crate::shaft::set_masks(&published);
                glow.shaft_frames += 1;
            }
        }
        // (vfx) SORA'S EFFECTS, in the same list right after him (vfx.rs): the scene arrangement only,
        // with the game's own depth. Nothing in there can fail the frame; it leaves the depth as it found it.
        if let (true, Some((scene, format))) = (hdr, &scene) {
            let exposure = light_now.as_ref().filter(|l| l.scene && game_on).map(|l| l.exposure);
            crate::vfx::draw(&crate::vfx::DrawCx { device: &*device, list, slot: slot_index, target: back, target_view, depth: &scene.resource, depth_format: *format, depth_view, width, height, clip_from_game: clip, camera: camera_game, right: math::game_dir_of_host(camera.right), up: math::game_dir_of_host(camera.up), forward: math::game_dir_of_host(camera.forward), depth_a, depth_b, proj00: 1.0 / (tan_y * camera.aspect), scale: crate::vfx::effect_scale(exposure, out_scale), lit: Vec3::splat(fx_level), stage: if late_stage { crate::vfx::Stage::Early } else { crate::vfx::Stage::All } });
            survey::WATCH.store(back.as_raw() as usize, Ordering::Relaxed);
            *late = Some(LateFx { slot: slot_index, target: back.as_raw() as usize, depth: scene.resource.as_raw() as usize, depth_format: *format, width, height, frame_constants, clip, camera: camera_game, right: math::game_dir_of_host(camera.right), up: math::game_dir_of_host(camera.up), forward: math::game_dir_of_host(camera.forward), depth_a, depth_b, proj00: 1.0 / (tan_y * camera.aspect), scale: crate::vfx::effect_scale(exposure, out_scale), lit: Vec3::splat(fx_level), effects: late_stage, recomp: false, snap: early_reads.clone() /* (ZFIN) */ });
            // (ZFIN) ZETTAFLARE'S FINALE on an Elden Ring character (finale.rs), after the effects: here when the late pass does
            // not draw the effects' late stage (else draw_late does, after them)
            if !late_stage {
                let view = crate::finale::View { clip, camera: camera_game, right: math::game_dir_of_host(camera.right), up: math::game_dir_of_host(camera.up), forward: math::game_dir_of_host(camera.forward), depth_a, depth_b, proj00: 1.0 / (tan_y * camera.aspect), width, height };
                let fcx = crate::finale::FinaleCx { device: &*device, list, root: &*root, slot: slot_index, target_view, target_format: back_desc.Format, depth: &scene.resource, snapshot: early_reads.as_ref(), view, scale: crate::vfx::effect_scale(exposure, out_scale) };
                if crate::finale::draw(&fcx) {
                    list.SetGraphicsRootSignature(&*root);
                    list.SetDescriptorHeaps(&[Some(assets.srv_heap.clone()), Some(sampler_heap.clone())]);
                    list.SetGraphicsRootConstantBufferView(0, frame_constants);
                    list.OMSetRenderTargets(1, Some(&target_view), false, Some(&depth_view));
                    list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: stage.0, TopLeftY: stage.1, Width: stage.2, Height: stage.3, MinDepth: 0.0, MaxDepth: 1.0 }]);
                    list.RSSetScissorRects(&[RECT { left: 0, top: 0, right: width as i32, bottom: height as i32 }]);
                }
                // (ZFIN) the effects drawn over the finale (ZETTA's fragments, `r.finale_over`), held back by the stage above
                if crate::finale::active() {
                    crate::vfx::draw(&crate::vfx::DrawCx { device: &*device, list, slot: slot_index, target: back, target_view, depth: &scene.resource, depth_format: *format, depth_view, width, height, clip_from_game: clip, camera: camera_game, right: math::game_dir_of_host(camera.right), up: math::game_dir_of_host(camera.up), forward: math::game_dir_of_host(camera.forward), depth_a, depth_b, proj00: 1.0 / (tan_y * camera.aspect), scale: crate::vfx::effect_scale(exposure, out_scale), lit: Vec3::splat(fx_level), stage: crate::vfx::Stage::OverFinale });
                }
            }
        }
        // (IMPL_ER65) his colour and the depth with him, for the late re-composite
        let mut recomp_saved = false;
        if let (true, Some(kit), Some((scene_now, _))) = (recomp_now, late_kit.as_mut(), &scene) {
            let shader = D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE | D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE;
            let mut first = vec![transition(back, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_COPY_SOURCE)];
            let mut then = vec![transition(back, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_COPY_SOURCE, D3D12_RESOURCE_STATE_RENDER_TARGET)];
            for plane in 0..2u32 {
                first.push(transition(&scene_now.resource, plane, D3D12_RESOURCE_STATE_DEPTH_WRITE, D3D12_RESOURCE_STATE_COPY_SOURCE));
                then.push(transition(&scene_now.resource, plane, D3D12_RESOURCE_STATE_COPY_SOURCE, D3D12_RESOURCE_STATE_DEPTH_WRITE));
            }
            if !kit.fresh {
                first.push(transition(&kit.saved, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, shader, D3D12_RESOURCE_STATE_COPY_DEST));
                first.push(transition(&kit.him, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, shader, D3D12_RESOURCE_STATE_COPY_DEST));
            }
            then.push(transition(&kit.saved, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_COPY_DEST, shader));
            then.push(transition(&kit.him, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_COPY_DEST, shader));
            list.ResourceBarrier(&first);
            list.CopyResource(&kit.saved, back);
            list.CopyResource(&kit.him, &scene_now.resource);
            list.ResourceBarrier(&then);
            kit.fresh = false;
            recomp_saved = true;
        }
        // (IMPL_ER78, r.hybrid_dump) and the scene with him drawn
        if let (Some(rect), Some(dump)) = (hy_dump_rect, hy.dump.as_mut()) {
            let mut desc = back.GetDesc();
            (desc.Width, desc.Height, desc.MipLevels, desc.DepthOrArraySize) = ((rect[2] - rect[0]) as u64, rect[3] - rect[1], 1, 1);
            let mut footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default();
            let mut total = 0u64;
            device.GetCopyableFootprints(&desc, 0, 1, 0, Some(&mut footprint), None, None, Some(&mut total));
            if let (Ok(buffer), true) = (committed(device, D3D12_HEAP_TYPE_READBACK, &buffer_desc(total.max(256)), D3D12_RESOURCE_STATE_COPY_DEST, None), crate::hybrid::texel_bytes(back_desc.Format.0).is_some()) {
                list.ResourceBarrier(&[transition(back, 0, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_COPY_SOURCE)]);
                let to = D3D12_TEXTURE_COPY_LOCATION { pResource: std::mem::transmute_copy(&buffer), Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT, Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { PlacedFootprint: footprint } };
                let from = D3D12_TEXTURE_COPY_LOCATION { pResource: std::mem::transmute_copy(back), Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX, Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { SubresourceIndex: 0 } };
                list.CopyTextureRegion(&to, 0, 0, 0, &from, Some(&D3D12_BOX { left: rect[0], top: rect[1], front: 0, right: rect[2], bottom: rect[3], back: 1 }));
                list.ResourceBarrier(&[transition(back, 0, D3D12_RESOURCE_STATE_COPY_SOURCE, D3D12_RESOURCE_STATE_RENDER_TARGET)]);
                dump.items.push(("scene_with_sora".to_string(), buffer, footprint, back_desc.Format.0, format!("{} after his colour draw", survey::describe_resource(back.as_raw() as usize))));
            }
        }
        list.ResourceBarrier(&after);
        if let Some((heap, results, _)) = queries.as_ref() {
            list.EndQuery(heap, D3D12_QUERY_TYPE_TIMESTAMP, slot_index as u32 * 2 + 1);
            list.ResolveQueryData(heap, D3D12_QUERY_TYPE_TIMESTAMP, slot_index as u32 * 2, 2, results, slot_index as u64 * 16);
            slot.timed = true;
        }
        crate::passtime::stamp(device, queue, list, slot_index, 5); // (HAIRPERF)
        crate::passtime::finish(device, list, slot_index);
        if let Some(l) = late.as_mut() {
            l.recomp = recomp_saved;
        }
        list.Close()?;
        // (IMPL_ER78) THE G-BUFFER PASS, at the end of the early list (all of his depth is in): every opaque / masked draw once
        // more with its `_gb` shader into the game's five G-buffer targets, depth EQUAL. Barriers from the states the game's own
        // barriers left them in at the split, and back.
        let mut gb_recorded = false;
        #[cfg(feature = "party")]
        let party_gb = party_plan.as_ref().is_some_and(|p| !p.gb.is_empty());
        #[cfg(not(feature = "party"))]
        let party_gb = false;
        if let (Some(la), Ok((ordered, _, _, base)), false) = (early_list, &hy_plan, gb_draws.is_empty() && !party_gb) {
            let rtv_size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV) as usize;
            if hy.rtv.is_none() {
                hy.rtv = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV, NumDescriptors: (crate::hybrid::ROLES * FRAME_SLOTS) as u32, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE, NodeMask: 0 }).ok();
            }
            let resources: Vec<ID3D12Resource> = ordered.iter().filter_map(|t| {
                let raw = t.resource as *mut c_void;
                ID3D12Resource::from_raw_borrowed(&raw).cloned()
            }).collect();
            // (IMPL_ER78b) SECOND LIMIT: the screen box of his capsules (body + Keyblade); none = no pass
            let scissor = crate::hybrid::scissor_box(&hy_reach, &clip, tan_y, width, height, 24.0);
            // (ER party S4) the friends' draws set their own boxes: the pass runs with them even when Sora has none
            let scissor = scissor.or(if party_gb { Some([0, 0, width as i32, height as i32]) } else { None });
            if let (Some(heap), true, Some(sc)) = (hy.rtv.as_ref(), resources.len() == crate::hybrid::ROLES, scissor) {
                la.RSSetScissorRects(&[RECT { left: sc[0], top: sc[1], right: sc[2], bottom: sc[3] }]);
                let first = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: heap.GetCPUDescriptorHandleForHeapStart().ptr + slot_index * crate::hybrid::ROLES * rtv_size };
                let mut to_rt: Vec<D3D12_RESOURCE_BARRIER> = Vec::new();
                let mut back_from_rt: Vec<D3D12_RESOURCE_BARRIER> = Vec::new();
                for (k, (t, resource)) in ordered.iter().zip(resources.iter()).take(crate::hybrid::WRITTEN).enumerate() {
                    let typed = crate::hybrid::role_of(t.format).map_or(t.format, |(_, f)| f);
                    let desc = D3D12_RENDER_TARGET_VIEW_DESC { Format: DXGI_FORMAT(typed), ViewDimension: D3D12_RTV_DIMENSION_TEXTURE2D, Anonymous: D3D12_RENDER_TARGET_VIEW_DESC_0 { Texture2D: D3D12_TEX2D_RTV { MipSlice: 0, PlaneSlice: 0 } } };
                    device.CreateRenderTargetView(resource, Some(&desc), D3D12_CPU_DESCRIPTOR_HANDLE { ptr: first.ptr + k * rtv_size });
                    if t.state != D3D12_RESOURCE_STATE_RENDER_TARGET.0 {
                        to_rt.push(transition(resource, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATES(t.state), D3D12_RESOURCE_STATE_RENDER_TARGET));
                        back_from_rt.push(transition(resource, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATES(t.state)));
                    }
                }
                if !to_rt.is_empty() {
                    la.ResourceBarrier(&to_rt);
                }
                la.OMSetRenderTargets(crate::hybrid::WRITTEN as u32, Some(&first), true, Some(&depth_view));
                // (the frame block with the hybrid's values: gloss, translucency, light set)
                la.SetGraphicsRootConstantBufferView(0, frame_constants);
                let mut bound_model = usize::MAX;
                for (model_index, slot_k, count, first_index, base_vertex, skin_now, skin_then, material) in &gb_draws {
                    let (Some(model), Some(state)) = (assets.models.get(*model_index), pipelines.get(base + slot_k)) else { continue };
                    if bound_model != *model_index {
                        la.IASetVertexBuffers(0, Some(&[model.vertex_view]));
                        la.IASetIndexBuffer(Some(&model.index_view));
                        bound_model = *model_index;
                    }
                    la.SetGraphicsRootConstantBufferView(2, *skin_now);
                    la.SetGraphicsRootConstantBufferView(6, *skin_then);
                    la.SetPipelineState(state);
                    la.SetGraphicsRootConstantBufferView(1, *material);
                    la.SetGraphicsRootDescriptorTable(3, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: srv_start.ptr + (slot_k * SLOT_TEXTURES * assets.srv_size) as u64 });
                    la.DrawIndexedInstanced(*count, 1, *first_index, *base_vertex, 0);
                }
                // (ER party S4) the friends' G-buffer writes (each his own frame block and scissor box)
                #[cfg(feature = "party")]
                if let Some(plan) = party_plan.as_ref() {
                    let cx = party::PassCx { key, sampler_heap: &*sampler_heap, sora_heap: &assets.srv_heap, frame_constants, early_constants, rig: &rig, sh, env_level, env_tint, clock: input.clock, exposure: options.exposure, debug: options.debug };
                    party::gbuffer_pass(plan, party, la, &cx, &clip, tan_y, width, height);
                }
                if !back_from_rt.is_empty() {
                    la.ResourceBarrier(&back_from_rt);
                }
                gb_recorded = true;
                // (RSF2 round 11) THE REST OF HIS G-BUFFER: targets 3 (subsurface) and 4 (emissive) set to none where target 2 holds the
                // KH characters' white (hygb.ps.dxil), over the capture's box: the game's subsurface blur no longer runs over our
                // pixels with the background's profile. Only with the capture recorded this frame (its table holds target 2's view).
                if let (true, Some(cap), Some(bx), true) = (hy_on, hy.cap.as_ref(), hy_box, crate::hybrid::settings().gbclear > 0.5) {
                    if let Some(gb_pso) = cap.gb_pso.as_ref() {
                        let typed = |k: usize| crate::hybrid::role_of(ordered[k].format).map_or(ordered[k].format, |(_, f)| f);
                        if typed(3) == DXGI_FORMAT_R8G8_UNORM.0 && typed(4) == DXGI_FORMAT_R16G16B16A16_FLOAT.0 {
                            let rtv_size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV) as usize;
                            let first = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: cap.gb_rtv.GetCPUDescriptorHandleForHeapStart().ptr + slot_index * 2 * rtv_size };
                            let mut to_rt = Vec::new();
                            let mut back = Vec::new();
                            for (n, k) in [3usize, 4].iter().enumerate() {
                                let desc = D3D12_RENDER_TARGET_VIEW_DESC { Format: DXGI_FORMAT(typed(*k)), ViewDimension: D3D12_RTV_DIMENSION_TEXTURE2D, Anonymous: D3D12_RENDER_TARGET_VIEW_DESC_0 { Texture2D: D3D12_TEX2D_RTV { MipSlice: 0, PlaneSlice: 0 } } };
                                device.CreateRenderTargetView(&resources[*k], Some(&desc), D3D12_CPU_DESCRIPTOR_HANDLE { ptr: first.ptr + n * rtv_size });
                                let state = ordered[*k].state;
                                if state != D3D12_RESOURCE_STATE_RENDER_TARGET.0 {
                                    to_rt.push(transition(&resources[*k], D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATES(state), D3D12_RESOURCE_STATE_RENDER_TARGET));
                                    back.push(transition(&resources[*k], D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATES(state)));
                                }
                            }
                            if !to_rt.is_empty() {
                                la.ResourceBarrier(&to_rt);
                            }
                            la.OMSetRenderTargets(2, Some(&first), true, None);
                            let size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV) as u64;
                            la.SetDescriptorHeaps(&[Some(cap.inputs.clone())]);
                            la.SetGraphicsRootSignature(&*root);
                            la.SetGraphicsRootConstantBufferView(0, frame_constants);
                            la.SetGraphicsRootDescriptorTable(3, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: cap.inputs.GetGPUDescriptorHandleForHeapStart().ptr + slot_index as u64 * SLOT_TEXTURES as u64 * size });
                            la.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: width as f32, Height: height as f32, MinDepth: 0.0, MaxDepth: 1.0 }]);
                            la.RSSetScissorRects(&[RECT { left: bx[0].clamp(0, width as i32), top: bx[1].clamp(0, height as i32), right: bx[2].clamp(0, width as i32), bottom: bx[3].clamp(0, height as i32) }]);
                            la.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
                            la.SetPipelineState(gb_pso);
                            la.DrawInstanced(3, 1, 0, 0);
                            if !back.is_empty() {
                                la.ResourceBarrier(&back);
                            }
                            crate::hybrid::GB_CLEARED.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            }
        }
        // (IMPL_ER78) HARDENING: planned but not written into the G-buffer (no view heap, a target gone): the colour list (not
        // executed yet) reads the block with the hybrid off, i.e. today's light
        if hy_on && !gb_recorded {
            let at = (frame_constants - slot.upload.GetGPUVirtualAddress()) as usize + (frame.len() - crate::hybrid::BLOCK_FLOATS) * 4;
            std::ptr::write_bytes(slot.mapped.add(at), 0, crate::hybrid::BLOCK_FLOATS * 4);
            #[cfg(feature = "party")]
            if let Some(plan) = party_plan.as_ref() {
                party::hybrid_off(plan, slot.mapped, frame.len());
            }
            crate::hybrid::FRAMES_ON.fetch_sub(1, Ordering::Relaxed);
            crate::hybrid::fell_back("the G-buffer pass could not be recorded");
        }
        if let Some(la) = early_list {
            // (RSF2 round 6) the scene depth as it is now (the world, the KH characters' early depth) into our copy, for the
            // capture's sun lookup; the depth goes back to DEPTH_WRITE before its own way back
            if let (true, Some((copy, _)), Some((scene_now, _)), false) = (kh_copy, kh_depth.as_ref(), &scene, early_back.is_empty()) {
                let planes = early_back.len() as u32;
                let mut first: Vec<D3D12_RESOURCE_BARRIER> = (0..planes).map(|k| transition(&scene_now.resource, k, D3D12_RESOURCE_STATE_DEPTH_WRITE, D3D12_RESOURCE_STATE_COPY_SOURCE)).collect();
                first.push(transition(copy, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE | D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE, D3D12_RESOURCE_STATE_COPY_DEST));
                la.ResourceBarrier(&first);
                la.CopyResource(copy, &scene_now.resource);
                let mut then: Vec<D3D12_RESOURCE_BARRIER> = (0..planes).map(|k| transition(&scene_now.resource, k, D3D12_RESOURCE_STATE_COPY_SOURCE, D3D12_RESOURCE_STATE_DEPTH_WRITE)).collect();
                then.push(transition(copy, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_COPY_DEST, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE | D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE));
                la.ResourceBarrier(&then);
            }
            if !early_back.is_empty() {
                la.ResourceBarrier(&early_back);
            }
            la.Close()?;
            queue.ExecuteCommandLists(&[Some(la.cast::<ID3D12CommandList>()?)]);
            *pending = Some(slot_index);
            slot.busy_until = *fence_value + 1;
            *early_drawn += 1;
            EARLY_DRAWS.store(early_draws, Ordering::Relaxed);
        } else {
            queue.ExecuteCommandLists(&[Some(list.cast::<ID3D12CommandList>()?)]);
            *fence_value += 1;
            queue.Signal(&*fence, *fence_value)?;
            slot.busy_until = *fence_value;
        }
        if hdr && scene.is_some() && light_now.as_ref().is_some_and(|l| l.scene) {
            *velo = Some(VeloFrame { slot: slot_index, key, width, height, frame_constants, skins: velo_skins, depth: scene.as_ref().map_or(0, |(s, _)| s.resource.as_raw() as usize), #[cfg(feature = "party")] party: party_plan.as_ref().map(|p| p.velo()).unwrap_or_default() });
            // (IMPL_ER17) the next frame's shadow pass draws him from these constants (they stay in this slot's buffer)
            *cast = velo.clone().map(|v| (v, Instant::now()));
        } else {
            *cast = None;
        }
        let micros = started.elapsed().as_micros() as u64;
        #[cfg(feature = "party")]
        if party_plan.is_some() {
            let party_micros = party_us;
            self.party.frames += 1;
            self.party.micros += party_micros;
            self.party.micros_max = self.party.micros_max.max(party_micros);
            if void.is_some() {
                self.party.void_frames += 1;
            }
        }
        self.drawn_total += 1;
        let stats = &mut self.stats;
        stats.drawn += 1;
        stats.draws += drawn;
        stats.micros += micros;
        stats.micros_max = stats.micros_max.max(micros);
        Ok(())
    }
}

/// The hook's entry: the game is about to present `swap`.
pub fn present(swap: *mut c_void) {
    // SAFETY: `swap` is `this` of a Present call: a live swap chain for the duration of it.
    unsafe {
        let Some(chain) = IDXGISwapChain::from_raw_borrowed(&swap) else { return };
        let Ok(chain3) = chain.cast::<IDXGISwapChain3>() else { return };
        let mut guard = CORE.lock().unwrap();
        if guard.as_ref().is_some_and(|core| core.swap != swap as usize) {
            // Another swap chain presents (a new one after a device change): start again on it.
            if let Ok(device) = chain.GetDevice::<ID3D12Device>() {
                let old = guard.take();
                rlog!("render: a different swap chain presents: starting again on it");
                drop_core(old);
                let _ = device;
            }
        }
        if guard.as_ref().is_some_and(|core| core.lost) {
            // (no waiting for the GPU: the device is gone)
            let old = guard.take();
            if let Some(core) = &old {
                for slot in &core.slots {
                    slot.upload.Unmap(0, None);
                }
                if let Some(kit) = &core.portrait {
                    kit.unmap();
                }
                #[cfg(feature = "party")]
                for (_, kit) in &core.party.pics {
                    kit.unmap();
                }
            }
            drop(old);
            survey::forget();
            *NEW_CORE_AT.lock().unwrap() = Some(Instant::now() + Duration::from_secs(3));
            return;
        }
        if guard.is_none() {
            if NEW_CORE_AT.lock().unwrap().is_some_and(|at| Instant::now() < at) {
                return;
            }
            let Ok(device) = chain.GetDevice::<ID3D12Device>() else {
                *WAITING.lock().unwrap() = "the presenting swap chain is not Direct3D 12".to_string();
                return;
            };
            let Some(queue) = hook::queue_of(swap, &device) else {
                *WAITING.lock().unwrap() = "no command queue of the game seen yet".to_string();
                return;
            };
            match Core::new(swap as usize, device, queue) {
                Ok(core) => {
                    rlog!("render: our Direct3D 12 objects are made on the game's device");
                    *guard = Some(core);
                }
                Err(e) => {
                    // HARDENING: not fatal: tried again in 5 s (a device being replaced).
                    rlog!("render: our Direct3D 12 objects could not be made ({e}): tried again in 5 s");
                    *NEW_CORE_AT.lock().unwrap() = Some(Instant::now() + Duration::from_secs(5));
                    return;
                }
            }
        }
        let core = guard.as_mut().unwrap();
        if core.disabled {
            // (the survey's frame still has to end)
            let _ = chain3.GetDesc1().map(|d| survey::end_frame(d.Width, d.Height));
            return;
        }
        let result = core.at_present(&chain3);
        core.outcome(result);
        core.portrait_at_present(); // (IMPL_ER21) the camp menu's portrait into its own target, for the overlay
        #[cfg(feature = "party")]
        core.party_portraits_at_present(); // (ER party S4) the friends' camp pictures, for the overlay
        crate::finale::at_present(&core.device, &core.queue, &chain3); // (ZFIN round 9) Zettaflare's colour in display colours
        crate::hud::at_present(&core.device, &core.queue, &chain3); // (hud) the 2D overlay, after everything of the 3D path
    }
}

/// The draw is made at Present itself (`r.present`): the self-recovery check does not apply.
fn at_present_option(_core: &Core) -> bool {
    SHARED.lock().ok().and_then(|g| g.as_ref().map(|s| s.options.at_present || !s.options.draw)).unwrap_or(false)
}

/// (IMPL_ER17) `r.shadow_real`: his skinned mesh is drawn depth-only into the game's cascaded shadow atlas.
static CAST_WANTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// (IMPL_ER78) `r.shadow_real` as the host gave it (the hybrid with `r.hybrid_cast` switches the cast on as well).
static CAST_REAL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_cast(on: bool) {
    CAST_REAL.store(on, std::sync::atomic::Ordering::Relaxed);
    refresh_cast();
}

/// (IMPL_ER78) His real shadow is drawn when `r.shadow_real = 1`, or while the hybrid is on with `r.hybrid_cast` (default).
pub fn refresh_cast() {
    let hy = crate::hybrid::settings();
    let on = CAST_REAL.load(std::sync::atomic::Ordering::Relaxed) || (hy.on && hy.cast);
    CAST_WANTED.store(on, std::sync::atomic::Ordering::Relaxed);
    // (RSF2 round 6) the caster passes are followed for the hybrid's sun lookup too (read only)
    crate::cbcap::CASTING.store((on || sun_read_wanted()) as u32, std::sync::atomic::Ordering::Relaxed);
}

/// (RSF2 round 6) The hybrid reads the sun's visibility from the game's cascades: `r.hybrid`, `r.hybrid_mask` and
/// `r.hybrid_sun` on. Nothing of the game's is written for it: the cascades' constants are copied into a texture of ours at
/// the cast split, the atlas is only read, the scene depth is copied (inside our own early list).
pub fn sun_read_wanted() -> bool {
    let hy = crate::hybrid::settings();
    hy.on && hy.mask > 0.5 && hy.sun > 0.5
}

/// (RSF2 round 6) frames whose capture read the cascades / cascade reads since the last status line (hybrid.rs status)
pub static SUN_FRAMES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
pub static SUN_READS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
/// (RSF2 round 13) the cascade sets read this second (their viewports) and how often each: a set that changes from frame to frame
/// is a lookup that switches cascades
pub static SUN_SETS: std::sync::Mutex<Vec<(String, u32)>> = std::sync::Mutex::new(Vec::new());

/// (testing aid, `r.shadow_dbg = n`: a change) the next shadow pass stops after cascade pass n - 1, waits for the GPU and
/// logs what it copied of that pass's cbSceneParam, with every pass's viewport.
pub static CAST_DEBUG: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
pub static CAST_FLOOD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub static CAST_PUSH: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// (testing aid, `r.shadow_bias = n`, read when the kit is made: |n| - 1 = the depth bias of the debug pipeline in D16 steps; n < 0 = drawn whatever is there)
pub static CAST_PUSH_BIAS: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-60001);

pub fn cast_wanted() -> bool {
    (CAST_WANTED.load(std::sync::atomic::Ordering::Relaxed) || sun_read_wanted()) && SHARED.lock().ok().and_then(|g| g.as_ref().map(|s| s.options.draw && s.options.at == crate::At::Scene && !s.options.at_present)).unwrap_or(false)
}

/// HIS REAL SHADOW: called by the queue hook in the middle of the game's deferred submission, right after the last list
/// that fills the sun's cascade atlas and before the G-buffer and the screen-space shadow mask.
pub fn cast_pass(queue: usize, sub: survey::Submission) {
    let mut guard = CORE.lock().unwrap();
    let Some(core) = guard.as_mut() else { return };
    let Some(sun) = sub.sun else { return };
    if core.disabled || core.lost || core.queue.as_raw() as usize != queue || core.retry_at.is_some_and(|at| Instant::now() < at) {
        return;
    }
    // (RSF2 round 6) the cascades' constants for the hybrid's sun lookup (read only)
    if sun_read_wanted() && !core.sun_failed {
        let started = Instant::now();
        // SAFETY: our own objects; the game's tables and atlas were used by lists the game has just submitted.
        let result = unsafe { core.read_cascades(&sun) };
        core.cast_micros += started.elapsed().as_micros() as u64;
        core.outcome(result);
    }
    if core.cast_failed || !CAST_WANTED.load(std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    // (his constants of the colour draw a frame ago; nothing when he was not drawn lately)
    let Some((frame, at)) = core.cast.clone() else { return };
    if at.elapsed() > Duration::from_millis(150) {
        return;
    }
    // HARDENING: the atlas must rest in a shader-resource state the game itself named in this submission.
    if sun.state == 0 || sun.state & (D3D12_RESOURCE_STATE_DEPTH_WRITE.0 | D3D12_RESOURCE_STATE_RENDER_TARGET.0) != 0 || sun.view == 0 || sun.count == 0 {
        core.stats.skipped += 1;
        return;
    }
    let started = Instant::now();
    // SAFETY: our own objects; the atlas and its view were used by lists the game has just submitted.
    let result = unsafe { core.draw_cast(&frame, &sun) };
    core.cast_micros += started.elapsed().as_micros() as u64;
    core.outcome(result);
}

/// The velocity draw is wanted (the control file's switch).
pub fn velocity_wanted() -> bool {
    SHARED.lock().ok().and_then(|g| g.as_ref().map(|s| s.options.velocity != 0 && s.options.jitter && s.options.at == crate::At::Scene && !s.options.at_present)).unwrap_or(false)
}

/// HIS MOTION VECTORS: called by the queue hook in the middle of the game's submission that
/// holds its velocity pass, right after the list of that pass. The game's velocity target gets,
/// where his pixels are (depth EQUAL to what his colour draw left), the difference of his
/// position on screen now and a frame ago, in the game's own encoding (its velocity shaders:
/// (now - before) in clip units / w, times (aspect / 2, -1 / 2), times FC_VelocityEncodeParam).
pub fn velocity_pass(queue: usize, sub: survey::Submission) {
    let mut guard = CORE.lock().unwrap();
    let Some(core) = guard.as_mut() else { return };
    let frame = core.velo.take().filter(|_| velocity_wanted());
    let Some((target, target_state)) = sub.velocity else { return };
    if core.disabled || core.lost || core.queue.as_raw() as usize != queue {
        return;
    }
    // (IMPL_ER80) THE LATE RE-COMPOSITE BEFORE THE GAME'S TAA (r.late_sora + r.late_sora_taa): every check here; any failure =
    // no re-composite in this frame (the late pass does not paste after the TAA either), counted with its reason
    // SAFETY: our own objects and the pack's file.
    let recomp = unsafe { core.recomp_at_split(&sub) };
    // (ER87) the hair fringe of this frame is drawn in the split's list (draw_split), from the scene target's state here
    core.glow.hair_split_state = if sub.clean && sub.depth_seen { sub.watch_state } else { 0 };
    let Some(frame) = frame else {
        if recomp.is_some() || core.glow.hair_late.is_some() {
            // SAFETY: as below.
            let result = unsafe { core.draw_split(None, recomp, sub.depth_states) };
            core.outcome(result);
        }
        return;
    };
    // HARDENING: only with states observed in this submission and nothing contradicted.
    let odd = |state: i32| state == 0 || state & D3D12_RESOURCE_STATE_RENDER_TARGET.0 != 0;
    if !sub.clean || !sub.depth_seen || sub.depth_states.iter().any(|s| odd(*s)) || target_state == 0 || target_state & D3D12_RESOURCE_STATE_DEPTH_WRITE.0 != 0 {
        core.stats.skipped += 1;
        core.tell("velocity draw skipped (tried again every frame): the frame is not of the known shape".to_string());
        // (IMPL_ER80) the re-composite has its own checks (recomp_at_split): it is drawn without the motion vectors
        if let Some(scene_state) = recomp {
            // SAFETY: as below.
            let result = unsafe { core.draw_split(None, Some(scene_state), sub.depth_states) };
            core.outcome(result);
        }
        return;
    }
    // SAFETY: our own objects; the target and the depth were used by lists the game has just submitted.
    let result = unsafe { core.draw_split(Some((&frame, target, target_state)), recomp, sub.depth_states) };
    core.outcome(result);
}

/// THE LATE PASS: called by the queue hook right after the game's forward submission (the one
/// with its velocity pass) has gone in. MEASURED (r.survey, IMPL_ER7_STATE.md): all through that
/// submission the game draws into the SAME HDR scene target the deferred stage composed, pass by
/// pass (target SRV -> RENDER_TARGET -> SRV, depth SRV -> DEPTH_WRITE -> SRV), to its very last
/// list; anti-aliasing and the post chain come in later submissions. So here the scene holds the
/// game's own translucents, water and particles, and nothing has been resolved yet: the scene is
/// darkened when the effects ask for it, then Sora's translucent / additive effects are drawn.
pub fn late_pass(queue: usize, sub: survey::Submission) {
    let mut guard = CORE.lock().unwrap();
    let Some(core) = guard.as_mut() else { return };
    let Some(late) = core.late.take() else { return };
    if core.disabled || core.lost || core.queue.as_raw() as usize != queue {
        return;
    }
    if !sub.clean || !sub.depth_seen {
        return;
    }
    // (states observed in this frame only; a render target or depth-write state at rest is not the known shape)
    let (Some(target_states), Some(depth_states)) = (survey::states_now(late.target), survey::states_now(late.depth)) else { return };
    let odd = |state: i32| state == 0 || state & (D3D12_RESOURCE_STATE_RENDER_TARGET.0 | D3D12_RESOURCE_STATE_DEPTH_WRITE.0) != 0;
    if odd(target_states[0]) || depth_states.iter().any(|s| odd(*s)) {
        return;
    }
    core.late_seen = core.frame_no;
    let dim = crate::vfx::scene_dim();
    if !late.effects && dim >= 0.999 && !late.recomp {
        return;
    }
    // SAFETY: our own objects; the target and the depth were used by lists the game has just submitted.
    let result = unsafe { core.draw_late(&late, target_states[0], depth_states, dim) };
    core.outcome(result);
}

/// The hook's other entry: the submission the game just made on `queue` was its deferred stage
/// (`scene`: the HDR scene target and its state) and / or finished a back buffer (`finished`).
/// Which one we draw after is `Options::at`.
pub fn after_submission(queue: usize, submission: survey::Submission) {
    let mut guard = CORE.lock().unwrap();
    let Some(core) = guard.as_mut() else { return };
    // (IMPL_ER53) the colour list recorded with this frame's early depth: it goes now, whatever else holds (his depth is
    // in the scene already: his picture must follow it)
    if let Some(slot) = core.pending.take() {
        if core.queue.as_raw() as usize == queue && !core.lost {
            // SAFETY: our own list, closed; the game's resources it names were used by the submission that just went in.
            let result = unsafe { core.run_pending(slot) };
            core.outcome(result);
        }
        return;
    }
    if core.disabled || core.lost || core.queue.as_raw() as usize != queue {
        return;
    }
    if core.retry_at.is_some_and(|at| Instant::now() < at) {
        return;
    }
    let Some((at_present, at)) = SHARED.lock().ok().and_then(|g| g.as_ref().map(|s| (s.options.at_present, s.options.at))) else { return };
    if at_present {
        return;
    }
    let (target, rest, hdr) = match (at, submission.scene, submission.finished) {
        (crate::At::Scene, Some((target, state)), _) => (target, D3D12_RESOURCE_STATES(state), true),
        (crate::At::Composite, _, Some(back)) => (back, D3D12_RESOURCE_STATE_PRESENT, false),
        _ => return,
    };
    let raw = target as *mut c_void;
    // SAFETY: the game used this resource in the lists it just submitted: it is alive.
    unsafe {
        let Some(resource) = ID3D12Resource::from_raw_borrowed(&raw) else { return };
        let result = core.draw(resource, rest, hdr, Some(&submission).filter(|_| hdr), None);
        core.outcome(result);
    }
}

/// (IMPL_ER53) `r.early_depth` (default 1): his depth goes into the game's scene depth inside its deferred stage, right
/// after the G-buffer, so that the depth copies, downsamples and screen-space passes the game makes later (and its forward
/// stage reads: decals, glow volumes, soft particles, fog) know he is there. 0 = as before (his depth only after the stage).
static EARLY_WANTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
/// (IMPL_ER65) `r.late_sora` (default 1): the late re-composite of his colour after the game's forward stage, before its
/// bloom / exposure / tone mapping; `r.late_sora_far` (m, default 100): only where what lies behind him is the sky or
/// farther than this (0 = wherever he is).
static LATE_SORA: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// (IMPL_ER80) `r.glow_mask` (default 1): his opaque / masked pixels are written as 0 into the game's glow source right after
/// his colour draw (`GlowKit`); 0 = as before (the glow of what lies behind him is blurred and added over him).
static GLOW_MASK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub fn set_glow_mask(on: bool) {
    GLOW_MASK.store(on, Ordering::Relaxed);
}

/// (IMPL_ER80) `r.late_sora_taa` (default 1): the late re-composite at the velocity split, before the game's TAA.
static LATE_TAA: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
/// (for the log) re-composites drawn before the TAA, frames that wanted it and fell back, the latest reason
static RECOMP_SPLIT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static RECOMP_SPLIT_MISSED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static RECOMP_SPLIT_WHY: Mutex<&'static str> = Mutex::new("none");

pub fn set_late_taa(on: bool) {
    LATE_TAA.store(on, Ordering::Relaxed);
}

/// (IMPL_ER80) The queue hook splits the forward submission at its velocity list for the late re-composite as well.
pub fn recomp_split_wanted() -> bool {
    LATE_SORA.load(Ordering::Relaxed) && LATE_TAA.load(Ordering::Relaxed)
}

fn recomp_split_missed(why: &'static str) {
    RECOMP_SPLIT_MISSED.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut w) = RECOMP_SPLIT_WHY.lock() {
        *w = why;
    }
}
static LATE_FAR: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0x42C8_0000);

pub fn set_late(on: bool, far_m: f32) {
    LATE_SORA.store(on, Ordering::Relaxed);
    LATE_FAR.store(far_m.max(0.0).to_bits(), Ordering::Relaxed);
}

/// (IMPL_ER56) `r.early_stencil` (default 1): his early depth also writes his STENCIL with the value the game's G-buffer
/// left on the ground at his feet (so the game's stencil-masked passes treat him as geometry, not as sky); `r.stencil_ref`
/// (0..255) a fixed value instead (-1 = the ground's).
static STENCIL_WANTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
static STENCIL_FIXED: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);
/// (for the log) the stencil value of the latest early depth + 1 (0 = none written).
static STENCIL_USED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub fn set_stencil(on: bool, fixed: i32) {
    STENCIL_WANTED.store(on, Ordering::Relaxed);
    STENCIL_FIXED.store(fixed, Ordering::Relaxed);
}

/// (ER88) `r.taa_char` (default 1): his early depth writes the stencil Elden Ring's own CHARACTERS carry (`CHAR_STENCIL`)
/// instead of the ground's. The game's TAA (StencilAndMotionVectorPreprocess -> TempAA[StencilMask], IMPL_ER88_STATE.md) takes
/// its per-pixel class from stencil bits 6-7: the class picks the history weight (FeedbackWeightScale(None/0/1/2)), and where
/// the motion changes by more than 1.5 px a class that differs from the history's rejects the history and blurs the pixel
/// (RejectedPixelBlurIntensity). With the ground's value he was "scenery" (0x12: class 0) or "grass" (0xd2: class 3), and the
/// class flipped with the ground under his feet: his face and body were smeared / blurred in motion. 0 = the ground's (ER56).
static TAA_CHAR: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
/// (ER88) The stencil Elden Ring's characters leave in the scene depth. MEASURED (ER56's stencil taps around his feet in the
/// user's logs): 0x50 only where a tap fell on a character (the vanilla player at the spawn, 36 s; an enemy in combat, 498 s);
/// terrain 0x12 / 0xd2 / 0x90, map objects 0x10, sky 0. Bits 6-7 = 01: TAA class 1 = FeedbackWeightScale(0).
pub(crate) const CHAR_STENCIL: u8 = 0x50;

pub fn set_taa_char(on: bool) {
    TAA_CHAR.store(on, Ordering::Relaxed);
}

/// (ER88) His stencil value: `r.stencil_ref` when set (0..255), else the characters' value (`r.taa_char`), else the
/// ground's at his feet (IMPL_ER56; `None` until it is known).
pub(crate) fn his_stencil(fixed: i32, taa_char: bool, ground: Option<u8>) -> Option<u8> {
    match fixed {
        fixed if (0..=255).contains(&fixed) => Some(fixed as u8),
        _ if taa_char => Some(CHAR_STENCIL),
        _ => ground,
    }
}

/// (ER88) For taa.rs's log line: the stencil his latest early depth wrote and its TAA class.
pub(crate) fn taa_status() -> String {
    let fixed = STENCIL_FIXED.load(Ordering::Relaxed);
    let choice = if (0..=255).contains(&fixed) { "r.stencil_ref" } else if TAA_CHAR.load(Ordering::Relaxed) { "r.taa_char 1: the characters' value" } else { "r.taa_char 0: the ground's" };
    match STENCIL_USED.load(Ordering::Relaxed) {
        0 => format!("his stencil not written ({choice})"),
        v => format!("his stencil {:#04x} = class {} ({choice})", v - 1, taa_class((v - 1) as u8)),
    }
}

/// (ER88) A pipeline key as one number (taa.rs keeps the own-record velocity states by it).
fn key_bits(k: PsoKey) -> u64 {
    (k.target as u32 as u64) | ((k.depth as u32 as u64) << 32) ^ (u64::from(k.front_ccw) << 61) ^ (u64::from(k.cull) << 62) ^ (u64::from(k.alpha) << 63)
}

/// (ER88) The game's TAA class of a stencil value (StencilAndMotionVectorPreprocess: (stencil & 0xC0) >> 6).
pub(crate) fn taa_class(stencil: u8) -> u8 {
    (stencil & 0xC0) >> 6
}

/// (IMPL_ER56) Bytes per texel and the stencil byte's offset in a copy of a depth resource's stencil plane.
fn stencil_texel(format: DXGI_FORMAT) -> (usize, usize) {
    match format {
        DXGI_FORMAT_R32G8X24_TYPELESS | DXGI_FORMAT_D32_FLOAT_S8X24_UINT | DXGI_FORMAT_X32_TYPELESS_G8X24_UINT | DXGI_FORMAT_R32_FLOAT_X8X24_TYPELESS => (8, 4),
        DXGI_FORMAT_R24G8_TYPELESS | DXGI_FORMAT_D24_UNORM_S8_UINT | DXGI_FORMAT_X24_TYPELESS_G8_UINT => (4, 3),
        _ => (1, 0),
    }
}

/// (for the log) draws in the latest early depth.
static EARLY_DRAWS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub fn set_early(on: bool) {
    EARLY_WANTED.store(on, Ordering::Relaxed);
}

/// (RSF2 round 14) `r.effects_depth`: 0 (default, round 14: no effect found in game for the user's fire) / 1 the KH characters' depth into the game's half-size effect depth (its fire,
/// embers and glows behind them are hidden); 0 = as before; 2 = a test: the nearest depth everywhere (every half-size effect hidden).
static EFFECTS_DEPTH: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1); // (RSF round 18) on by default (kh-er frame.rs)
/// (for the status line) passes drawn since the last line, and why the latest was skipped
pub static FX_DRAWN: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
pub static FX_SPLITS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub fn set_effects_depth(mode: u32) {
    EFFECTS_DEPTH.store(mode.min(2), Ordering::Relaxed);
}

pub fn effects_depth_wanted() -> bool {
    EFFECTS_DEPTH.load(Ordering::Relaxed) > 0 && SHARED.lock().ok().and_then(|g| g.as_ref().map(|s| s.options.draw && s.options.at == crate::At::Scene && !s.options.at_present)).unwrap_or(false)
}

/// (RSF2 round 14) The objects of the effect-depth pass.
struct FxKit {
    merge: ID3D12PipelineState,
    fill: Option<ID3D12PipelineState>,
    srv: ID3D12DescriptorHeap,
    dsv: ID3D12DescriptorHeap,
    lists: Vec<(ID3D12CommandAllocator, ID3D12GraphicsCommandList, u64)>,
}

/// (RSF2 round 14) THE GAME'S HALF-SIZE EFFECT DEPTH GETS THE KH CHARACTERS: called by the queue hook in the game's forward
/// submission right after the list that rebuilt its half-size depth (survey.rs effects_split), before its half-size effect
/// passes: hdmerge_ps writes, at half size, the nearest of each 2 x 2 of the scene depth (which holds every KH character since the
/// G-buffer stage) where it is nearer than what is there. The world's depth is unchanged; an effect behind a KH character is
/// hidden by him, one in front still draws over him. Own list, own views; the game's two depths go back to the states it left.
pub fn effects_depth_pass(queue: usize, sub: survey::Submission) {
    let mut guard = CORE.lock().unwrap();
    let Some(core) = guard.as_mut() else { return };
    let Some((half, half_planes, scene, scene_planes)) = sub.effects_half else { return };
    if core.disabled || core.lost || core.fx_failed || core.queue.as_raw() as usize != queue || core.retry_at.is_some_and(|at| Instant::now() < at) || core.assets.is_none() {
        return;
    }
    FX_SPLITS.fetch_add(1, Ordering::Relaxed);
    // SAFETY: our own objects; the two depths were used by lists the game has just submitted.
    let result = unsafe { core.draw_effects_depth(half, half_planes, scene, scene_planes) };
    core.outcome(result);
}

pub fn early_wanted() -> bool {
    EARLY_WANTED.load(Ordering::Relaxed) && SHARED.lock().ok().and_then(|g| g.as_ref().map(|s| s.options.draw && s.options.depth == 1 && s.options.at == crate::At::Scene && !s.options.at_present)).unwrap_or(false)
}

/// (IMPL_ER53) The deferred stage's G-buffer lists have just been submitted (hook.rs splits the submission there): the
/// whole frame of his is made now (the same draw as `after_submission` makes), his depth-only list is executed at once,
/// his colour list waits for the end of the submission.
pub fn early_pass(queue: usize, submission: survey::Submission) {
    let mut guard = CORE.lock().unwrap();
    let Some(core) = guard.as_mut() else { return };
    if core.disabled || core.lost || core.queue.as_raw() as usize != queue || core.retry_at.is_some_and(|at| Instant::now() < at) {
        return;
    }
    let Some((target, state)) = submission.scene else { return };
    let raw = target as *mut c_void;
    // SAFETY: the game uses this resource in the lists it submits now: it is alive.
    unsafe {
        let Some(resource) = ID3D12Resource::from_raw_borrowed(&raw) else { return };
        core.early_now = Some(submission.early_states);
        let result = core.draw(resource, D3D12_RESOURCE_STATES(state), true, Some(&submission), None);
        core.early_now = None;
        core.outcome(result);
    }
}

impl Core {
    /// (IMPL_ER78, `r.hybrid_dump`) Reads the dump's copies back and writes them under the log folder: one PNG per target
    /// (`hybrid_dump_<n>_<target>.png`; float targets scaled to their bright end, the scale in the text), the float targets
    /// also as raw little-endian RGBA f32 (`.f32`), and `hybrid_dump_<n>.txt` (what each one is, its state, the box, the
    /// keys, and the values over his pixels: those where the G-buffer holds the white surface he writes).
    unsafe fn write_hybrid_dump(&mut self, dump: HybridDump) {
        let Some(dir) = crate::DUMP_DIR.lock().ok().and_then(|d| d.clone()) else { return };
        let n = crate::hybrid::DUMPS.fetch_add(1, Ordering::Relaxed) + 1;
        let (w, h) = ((dump.rect[2] - dump.rect[0]) as usize, (dump.rect[3] - dump.rect[1]) as usize);
        let mut images: Vec<(String, i32, String, Vec<[f32; 4]>)> = Vec::new();
        for (name, buffer, footprint, format, what) in &dump.items {
            let Some(bytes) = crate::hybrid::texel_bytes(*format) else { continue };
            let mut at: *mut c_void = std::ptr::null_mut();
            let size = footprint.Footprint.RowPitch as usize * h;
            if buffer.Map(0, Some(&D3D12_RANGE { Begin: 0, End: size }), Some(&mut at)).is_err() {
                continue;
            }
            let mut texels = Vec::with_capacity(w * h);
            for y in 0..h {
                let row = std::slice::from_raw_parts((at as *const u8).add(footprint.Offset as usize + y * footprint.Footprint.RowPitch as usize), w * bytes);
                for x in 0..w {
                    texels.push(crate::hybrid::decode(*format, &row[x * bytes..x * bytes + bytes]));
                }
            }
            buffer.Unmap(0, Some(&D3D12_RANGE { Begin: 0, End: 0 }));
            images.push((name.clone(), *format, what.clone(), texels));
        }
        for (_, buffer, ..) in dump.items {
            self.retired.push((self.fence_value, buffer));
        }
        let rect = dump.rect;
        let settings = crate::hybrid::settings();
        rlog!("render: (IMPL_ER78) hybrid dump {n}: {} target(s), box {:?}: written to {}", images.len(), rect, dir.display());
        std::thread::spawn(move || {
            let mut text = format!("(IMPL_ER78) hybrid dump {n}: screen box {rect:?} ({w} x {h} px)\nsettings: {settings:?}\n");
            // his pixels: the G-buffer's surface colour is the white he writes (the sRGB bytes 255, 255, 255)
            let his: Vec<bool> = images.iter().find(|i| i.0 == "gbuffer2_albedo").map(|i| i.3.iter().map(|t| t[0] > 0.995 && t[1] > 0.995 && t[2] > 0.995).collect()).unwrap_or_default();
            text += &format!("his pixels (white in G-buffer 2): {}\n", his.iter().filter(|b| **b).count());
            for (name, format, what, texels) in &images {
                let (rgba, scale) = crate::hybrid::picture(*format, texels);
                let path = dir.join(format!("hybrid_dump_{n}_{name}.png"));
                let written = crate::hybrid::write_png(&path, w as u32, h as u32, &rgba);
                let mean = |pick: &dyn Fn(usize) -> bool| {
                    let (mut sum, mut count) = ([0.0f64; 4], 0usize);
                    for (k, t) in texels.iter().enumerate().filter(|(k, t)| pick(*k) && t.iter().all(|v| v.is_finite())) {
                        let _ = k;
                        for c in 0..4 {
                            sum[c] += t[c] as f64;
                        }
                        count += 1;
                    }
                    if count == 0 { "none".to_string() } else { format!("({:.5}, {:.5}, {:.5}, {:.5})", sum[0] / count as f64, sum[1] / count as f64, sum[2] / count as f64, sum[3] / count as f64) }
                };
                let centre = texels.get((h / 2) * w + w / 2).copied().unwrap_or([0.0; 4]);
                text += &format!("{name}: format {format}, {what}; png scale {scale:.4} ({}); mean all {}; mean his {}; centre {:?}\n", written.err().unwrap_or_else(|| "written".to_string()), mean(&|_| true), mean(&|k| his.get(k).copied().unwrap_or(false)), centre);
                if matches!(*format, crate::hybrid::fmt::R11G11B10_FLOAT | crate::hybrid::fmt::R16G16B16A16_FLOAT) {
                    let raw: Vec<u8> = texels.iter().flat_map(|t| t.iter().flat_map(|v| v.to_le_bytes())).collect();
                    let _ = std::fs::write(dir.join(format!("hybrid_dump_{n}_{name}.f32")), raw);
                }
            }
            let _ = std::fs::write(dir.join(format!("hybrid_dump_{n}.txt")), text);
        });
    }

    /// (IMPL_ER53) Executes the colour list of `slot` recorded by an early draw.
    unsafe fn run_pending(&mut self, slot: usize) -> windows::core::Result<()> {
        let list = self.slots[slot].list.cast::<ID3D12CommandList>()?;
        self.queue.ExecuteCommandLists(&[Some(list)]);
        self.fence_value += 1;
        self.queue.Signal(&self.fence, self.fence_value)?;
        self.slots[slot].busy_until = self.fence_value;
        Ok(())
    }

    /// HARDENING: a failed frame is never fatal. Nothing is drawn for a while (2 s, 10 s after
    /// three failures in a row), then it is tried again; a lost device drops our objects, which are
    /// made again when a working swap chain presents. Each kind of failure is logged once.
    fn outcome(&mut self, result: windows::core::Result<()>) {
        match result {
            Ok(()) => self.failures = 0,
            Err(e) => {
                self.failures += 1;
                // SAFETY: plain getter.
                let removed = unsafe { self.device.GetDeviceRemovedReason() };
                self.tell(format!("a frame failed ({e}); device removed reason: {removed:?}; drawing pauses and is tried again"));
                self.velo = None;
                self.retry_at = Some(Instant::now() + Duration::from_secs(if self.failures >= 3 { 10 } else { 2 }));
                if removed.is_err() {
                    self.lost = true;
                    rlog!("render: the device is lost: our objects are dropped and made again on the next working swap chain");
                }
            }
        }
    }
}

/// Waits for the GPU to finish our frames, then frees the objects.
fn drop_core(core: Option<Core>) {
    let Some(core) = core else { return };
    // SAFETY: plain getters.
    unsafe {
        let waited = Instant::now();
        while core.fence.GetCompletedValue() < core.fence_value && waited.elapsed() < Duration::from_millis(1000) && core.device.GetDeviceRemovedReason().is_ok() {
            std::thread::sleep(Duration::from_millis(1));
        }
        crate::outfit::release_all();
        for slot in &core.slots {
            slot.upload.Unmap(0, None);
        }
        if let Some(kit) = &core.portrait {
            kit.unmap();
        }
    }
    drop(core);
}

// ---------------------------------------------------------------------------- the game's constant buffers

/// Rows (vec4) copied per constant buffer; registers b4..b13 per table.
pub(crate) const DUMP_ROWS: u32 = 256;
const DUMP_REGISTERS: u32 = 10;
const DUMP_TABLES: u32 = 100;

/// What copies constant buffers of the game into a texture of ours: the game's own descriptor
/// heap is set on a list of ours and one of its tables (pixel stage, registers b4..b13: root
/// parameter 29 of the game's root signature) is bound to a root signature of ours with a table of
/// the same shape; ten small pixel shaders (one per register) write the buffer's rows as pixels.
/// Nothing of the game is read on the CPU and no address of its memory is needed.
struct DumpKit {
    texture: ID3D12Resource,
    readback: ID3D12Resource,
    footprint: D3D12_PLACED_SUBRESOURCE_FOOTPRINT,
    rtv: ID3D12DescriptorHeap,
    root: ID3D12RootSignature,
    pipelines: Vec<ID3D12PipelineState>,
    allocator: ID3D12CommandAllocator,
    list: ID3D12GraphicsCommandList,
}

/// The root signature for passes that read the GAME's descriptor tables: 0 = constants of ours
/// (b0, root descriptor), 1 = the game's pixel-stage constant buffers b4..b13 (a table of the
/// shape the game's own root signature has), 2 = the game's pixel-stage textures t20..t65.
pub(crate) unsafe fn game_root(device: &ID3D12Device) -> windows::core::Result<ID3D12RootSignature> {
    let buffers = [D3D12_DESCRIPTOR_RANGE { RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_CBV, NumDescriptors: 10, BaseShaderRegister: 4, RegisterSpace: 0, OffsetInDescriptorsFromTableStart: 0 }];
    let textures = [D3D12_DESCRIPTOR_RANGE { RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SRV, NumDescriptors: 46, BaseShaderRegister: 20, RegisterSpace: 0, OffsetInDescriptorsFromTableStart: 0 }];
    let table = |ranges: &[D3D12_DESCRIPTOR_RANGE]| D3D12_ROOT_PARAMETER { ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE, Anonymous: D3D12_ROOT_PARAMETER_0 { DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE { NumDescriptorRanges: ranges.len() as u32, pDescriptorRanges: ranges.as_ptr() } }, ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL };
    let parameters = [
        D3D12_ROOT_PARAMETER { ParameterType: D3D12_ROOT_PARAMETER_TYPE_CBV, Anonymous: D3D12_ROOT_PARAMETER_0 { Descriptor: D3D12_ROOT_DESCRIPTOR { ShaderRegister: 0, RegisterSpace: 0 } }, ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL },
        table(&buffers),
        table(&textures),
    ];
    let desc = D3D12_ROOT_SIGNATURE_DESC { NumParameters: parameters.len() as u32, pParameters: parameters.as_ptr(), NumStaticSamplers: 0, pStaticSamplers: std::ptr::null(), Flags: D3D12_ROOT_SIGNATURE_FLAG_NONE };
    let mut blob: Option<ID3DBlob> = None;
    D3D12SerializeRootSignature(&desc, D3D_ROOT_SIGNATURE_VERSION_1, &mut blob, None)?;
    let blob = blob.ok_or_else(|| windows::core::Error::from_hresult(windows::Win32::Foundation::E_FAIL))?;
    device.CreateRootSignature(0, std::slice::from_raw_parts(blob.GetBufferPointer() as *const u8, blob.GetBufferSize()))
}

/// A full-screen pipeline state of ours: the one-triangle vertex shader and `pixel`, one target, no depth.
pub(crate) unsafe fn screen_pipeline(device: &ID3D12Device, root: &ID3D12RootSignature, vertex: &[u8], pixel: &[u8], format: DXGI_FORMAT) -> windows::core::Result<ID3D12PipelineState> {
    let mut formats = [DXGI_FORMAT_UNKNOWN; 8];
    formats[0] = format;
    let mut blend = D3D12_BLEND_DESC::default();
    blend.RenderTarget[0].RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8;
    let pso = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
        pRootSignature: std::mem::transmute_copy(root),
        VS: D3D12_SHADER_BYTECODE { pShaderBytecode: vertex.as_ptr() as *const c_void, BytecodeLength: vertex.len() },
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
    device.CreateGraphicsPipelineState(&pso)
}

/// (IMPL_ER80) A full-screen pipeline state of ours blended by the pixel shader's alpha (over what is there): one target,
/// no depth, the target's alpha left alone.
pub(crate) unsafe fn screen_blend_pipeline(device: &ID3D12Device, root: &ID3D12RootSignature, vertex: &[u8], pixel: &[u8], format: DXGI_FORMAT) -> windows::core::Result<ID3D12PipelineState> {
    let mut formats = [DXGI_FORMAT_UNKNOWN; 8];
    formats[0] = format;
    let mut blend = D3D12_BLEND_DESC::default();
    blend.RenderTarget[0] = D3D12_RENDER_TARGET_BLEND_DESC {
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
    let pso = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
        pRootSignature: std::mem::transmute_copy(root),
        VS: D3D12_SHADER_BYTECODE { pShaderBytecode: vertex.as_ptr() as *const c_void, BytecodeLength: vertex.len() },
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
    device.CreateGraphicsPipelineState(&pso)
}

/// A full-screen pipeline that multiplies the target by the blend factor (the scene darkening).
pub(crate) unsafe fn dim_pipeline(device: &ID3D12Device, root: &ID3D12RootSignature, vertex: &[u8], pixel: &[u8], format: DXGI_FORMAT) -> windows::core::Result<ID3D12PipelineState> {
    let mut formats = [DXGI_FORMAT_UNKNOWN; 8];
    formats[0] = format;
    let mut blend = D3D12_BLEND_DESC::default();
    blend.RenderTarget[0] = D3D12_RENDER_TARGET_BLEND_DESC {
        BlendEnable: true.into(),
        LogicOpEnable: false.into(),
        SrcBlend: D3D12_BLEND_ZERO,
        DestBlend: D3D12_BLEND_BLEND_FACTOR,
        BlendOp: D3D12_BLEND_OP_ADD,
        SrcBlendAlpha: D3D12_BLEND_ZERO,
        DestBlendAlpha: D3D12_BLEND_ONE,
        BlendOpAlpha: D3D12_BLEND_OP_ADD,
        LogicOp: D3D12_LOGIC_OP_NOOP,
        RenderTargetWriteMask: D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8,
    };
    let pso = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
        pRootSignature: std::mem::transmute_copy(root),
        VS: D3D12_SHADER_BYTECODE { pShaderBytecode: vertex.as_ptr() as *const c_void, BytecodeLength: vertex.len() },
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
    device.CreateGraphicsPipelineState(&pso)
}

/// The views heap among the (at most two) heaps a list of the game had set.
pub(crate) unsafe fn views_heap(heaps: [usize; 2]) -> Option<ID3D12DescriptorHeap> {
    for pointer in heaps {
        if pointer == 0 || !hook::readable(pointer, 8) {
            continue;
        }
        let raw = pointer as *mut c_void;
        if let Some(heap) = ID3D12DescriptorHeap::from_raw_borrowed(&raw) {
            if heap.GetDesc().Type == D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV {
                return Some(heap.clone());
            }
        }
    }
    None
}

unsafe fn desc_w(r: &ID3D12Resource) -> u64 {
    r.GetDesc().Width
}
unsafe fn desc_h(r: &ID3D12Resource) -> u32 {
    r.GetDesc().Height
}

/// (IMPL_ER17) What the real-shadow pass needs: a one-row float texture a caster pass's cbSceneParam is copied into (the
/// game's own descriptor table bound to a root signature of the table shape its own has), a table of views of that
/// texture for his vertex shader, the depth-only pipeline, and lists of its own.
struct CastKit {
    texture: ID3D12Resource,
    rtv: ID3D12DescriptorHeap,
    heap: ID3D12DescriptorHeap,
    root: ID3D12RootSignature,
    copy: ID3D12PipelineState,
    depth: ID3D12PipelineState,
    depth_dbg: ID3D12PipelineState,
    lists: Vec<(ID3D12CommandAllocator, ID3D12GraphicsCommandList, u64)>,
}

const CAST_ROW: u32 = 128;

/// (RSF2 round 6) THE CASCADES FOR THE HYBRID'S SUN: a 128 x CASC_MAX float texture (row r = the cbSceneParam of the game's
/// caster pass of cascade r, copied by dump8.ps.dxil through the game's own descriptor table, as the cast kit does), its
/// render target view, the game-shaped root signature, the copy pipeline, lists of its own.
struct SunKit {
    texture: ID3D12Resource,
    rtv: ID3D12DescriptorHeap,
    root: ID3D12RootSignature,
    copy: ID3D12PipelineState,
    lists: Vec<(ID3D12CommandAllocator, ID3D12GraphicsCommandList, u64)>,
}

/// (RSF2 round 15) THE KH CASTER MAPS: one D16 slice per KH character (0 Sora, 1..3 the party in plan order), KH_MAP_SIZE square,
/// drawn with the light matrices of the finest cascade (the pass with the largest viewport), depth views per slice.
const KH_SLICES: usize = 4;
struct KhMaps {
    texture: ID3D12Resource,
    dsv: ID3D12DescriptorHeap,
}
/// (for the status line) frames whose capture read the maps / frames the maps were drawn
pub static KH_MAPS_USED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
pub static KH_MAPS_DRAWN: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static KH_MAPS_FAILED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

unsafe fn make_kh_maps(device: &ID3D12Device) -> windows::core::Result<KhMaps> {
    let size = crate::hybrid::KH_MAP_SIZE as u64;
    let mut desc = texture_desc(size as u32, size as u32, KH_SLICES as u16, 1, DXGI_FORMAT_R16_TYPELESS, D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL);
    desc.DepthOrArraySize = KH_SLICES as u16;
    let clear = D3D12_CLEAR_VALUE { Format: DXGI_FORMAT_D16_UNORM, Anonymous: D3D12_CLEAR_VALUE_0 { DepthStencil: D3D12_DEPTH_STENCIL_VALUE { Depth: 0.0, Stencil: 0 } } };
    let texture = committed(device, D3D12_HEAP_TYPE_DEFAULT, &desc, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE | D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE, Some(&clear))?;
    let dsv: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_DSV, NumDescriptors: KH_SLICES as u32, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE, NodeMask: 0 })?;
    let inc = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_DSV) as usize;
    for k in 0..KH_SLICES {
        let view = D3D12_DEPTH_STENCIL_VIEW_DESC { Format: DXGI_FORMAT_D16_UNORM, ViewDimension: D3D12_DSV_DIMENSION_TEXTURE2DARRAY, Flags: D3D12_DSV_FLAG_NONE, Anonymous: D3D12_DEPTH_STENCIL_VIEW_DESC_0 { Texture2DArray: D3D12_TEX2D_ARRAY_DSV { MipSlice: 0, FirstArraySlice: k as u32, ArraySize: 1 } } };
        device.CreateDepthStencilView(&texture, Some(&view), D3D12_CPU_DESCRIPTOR_HANDLE { ptr: dsv.GetCPUDescriptorHandleForHeapStart().ptr + k * inc });
    }
    Ok(KhMaps { texture, dsv })
}

/// (RSF2 round 6) What the cast split of this frame read: the atlas (pointer, size) and the viewports of the rows, finest first.
#[derive(Clone)]
struct SunNow {
    atlas: usize,
    width: u32,
    height: u32,
    viewports: Vec<[f32; 4]>,
    at: Instant,
}

impl Core {
    unsafe fn make_cast_kit(&mut self) -> Result<CastKit, String> {
        let text = |what: &str, e: windows::core::Error| format!("{what}: {e}");
        let pack = SHARED.lock().ok().and_then(|g| g.as_ref().map(|s| s.pack.clone())).ok_or("no pack")?;
        let dir = pack.join("materials").join("er");
        let read = |name: &str| std::fs::read(dir.join(name)).map_err(|e| format!("{name}: {e} (cargo run --release -p kh-shaderc -- --only shadow)"));
        let (screen, scene, vertex) = (read("blob.vs.dxil")?, read("dump8.ps.dxil")?, read("shadow.vs.dxil")?);
        let format = DXGI_FORMAT_R32G32B32A32_FLOAT;
        let desc = texture_desc(CAST_ROW, 1, 1, 1, format, D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET);
        let texture = committed(&self.device, D3D12_HEAP_TYPE_DEFAULT, &desc, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, None).map_err(|e| text("shadow data texture", e))?;
        let rtv: ID3D12DescriptorHeap = self.device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV, NumDescriptors: 1, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE, NodeMask: 0 }).map_err(|e| text("shadow data rtv heap", e))?;
        self.device.CreateRenderTargetView(&texture, None, rtv.GetCPUDescriptorHandleForHeapStart());
        // (the table of root parameter 3 as his vertex shader sees it: every entry is the data texture; it reads entry 16)
        let heap: ID3D12DescriptorHeap = self.device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, NumDescriptors: SLOT_TEXTURES as u32, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, NodeMask: 0 }).map_err(|e| text("shadow data view heap", e))?;
        let size = self.device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV) as usize;
        for k in 0..SLOT_TEXTURES {
            self.device.CreateShaderResourceView(&texture, None, D3D12_CPU_DESCRIPTOR_HANDLE { ptr: heap.GetCPUDescriptorHandleForHeapStart().ptr + k * size });
        }
        let root = game_root(&self.device).map_err(|e| text("game root signature", e))?;
        let copy = screen_pipeline(&self.device, &root, &screen, &scene, format).map_err(|e| text("shadow copy pipeline", e))?;
        let elements = layout();
        let pso = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            pRootSignature: std::mem::transmute_copy(&self.root),
            VS: D3D12_SHADER_BYTECODE { pShaderBytecode: vertex.as_ptr() as *const c_void, BytecodeLength: vertex.len() },
            BlendState: D3D12_BLEND_DESC::default(),
            SampleMask: u32::MAX,
            // (the light's near plane does not cut him: depth is clamped, as shadow casters are drawn)
            RasterizerState: D3D12_RASTERIZER_DESC { FillMode: D3D12_FILL_MODE_SOLID, CullMode: D3D12_CULL_MODE_NONE, DepthClipEnable: false.into(), ..Default::default() },
            // (MEASURED: the game clears the atlas to 0: reversed depth, the nearer to the light the larger)
            DepthStencilState: D3D12_DEPTH_STENCIL_DESC { DepthEnable: true.into(), DepthWriteMask: D3D12_DEPTH_WRITE_MASK_ALL, DepthFunc: D3D12_COMPARISON_FUNC_GREATER_EQUAL, StencilEnable: false.into(), ..Default::default() },
            InputLayout: D3D12_INPUT_LAYOUT_DESC { pInputElementDescs: elements.as_ptr(), NumElements: elements.len() as u32 },
            PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
            NumRenderTargets: 0,
            DSVFormat: DXGI_FORMAT_D16_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            ..Default::default()
        };
        let depth: ID3D12PipelineState = self.device.CreateGraphicsPipelineState(&pso).map_err(|e| text("shadow depth pipeline", e))?;
        // (testing aid, `r.shadow_dbg = 98`: his footprint pushed to the light's near plane, drawn whatever is there)
        let mut pso_dbg = pso;
        let dbg = CAST_PUSH_BIAS.load(Ordering::Relaxed) as i32;
        pso_dbg.RasterizerState.DepthBias = dbg;
        pso_dbg.DepthStencilState.DepthFunc = if dbg < 0 { D3D12_COMPARISON_FUNC_ALWAYS } else { D3D12_COMPARISON_FUNC_GREATER_EQUAL };
        pso_dbg.RasterizerState.DepthBias = dbg.abs() - 1;
        let depth_dbg: ID3D12PipelineState = self.device.CreateGraphicsPipelineState(&pso_dbg).map_err(|e| text("shadow depth pipeline (debug)", e))?;
        let mut lists = Vec::new();
        for _ in 0..FRAME_SLOTS + 1 {
            let allocator: ID3D12CommandAllocator = self.device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT).map_err(|e| text("shadow allocator", e))?;
            let list: ID3D12GraphicsCommandList = self.device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator, None).map_err(|e| text("shadow list", e))?;
            list.Close().map_err(|e| text("Close", e))?;
            lists.push((allocator, list, 0u64));
        }
        rlog!("render: HIS REAL SHADOW: his skinned mesh is drawn depth-only into the game's cascaded shadow atlas (r.shadow_real), each cascade with the game's own light matrices of that pass");
        Ok(CastKit { texture, rtv, heap, root, copy, depth, depth_dbg, lists })
    }

    unsafe fn make_fx_kit(&mut self) -> Result<FxKit, String> {
        let text = |what: &str, e: windows::core::Error| format!("{what}: {e}");
        let pack = SHARED.lock().ok().and_then(|g| g.as_ref().map(|s| s.pack.clone())).ok_or("no pack")?;
        let dir = pack.join("materials").join("er");
        let read = |name: &str| std::fs::read(dir.join(name)).map_err(|e| format!("{name}: {e}"));
        let (screen, merge_ps) = (read("blob.vs.dxil")?, read("hdmerge.ps.dxil")?);
        let fill_ps = read("hdfill.ps.dxil").ok();
        let make = |ps: &[u8]| {
            let desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
                pRootSignature: std::mem::transmute_copy(&self.root),
                VS: D3D12_SHADER_BYTECODE { pShaderBytecode: screen.as_ptr() as *const c_void, BytecodeLength: screen.len() },
                PS: D3D12_SHADER_BYTECODE { pShaderBytecode: ps.as_ptr() as *const c_void, BytecodeLength: ps.len() },
                BlendState: D3D12_BLEND_DESC::default(),
                SampleMask: u32::MAX,
                RasterizerState: D3D12_RASTERIZER_DESC { FillMode: D3D12_FILL_MODE_SOLID, CullMode: D3D12_CULL_MODE_NONE, DepthClipEnable: false.into(), ..Default::default() },
                DepthStencilState: D3D12_DEPTH_STENCIL_DESC { DepthEnable: true.into(), DepthWriteMask: D3D12_DEPTH_WRITE_MASK_ALL, DepthFunc: D3D12_COMPARISON_FUNC_GREATER, StencilEnable: false.into(), ..Default::default() },
                PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
                NumRenderTargets: 0,
                RTVFormats: [DXGI_FORMAT_UNKNOWN; 8],
                DSVFormat: DXGI_FORMAT_D32_FLOAT_S8X24_UINT,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                ..Default::default()
            };
            self.device.CreateGraphicsPipelineState::<ID3D12PipelineState>(&desc)
        };
        let merge = make(&merge_ps).map_err(|e| text("the effect depth's pipeline", e))?;
        let fill = fill_ps.and_then(|p| make(&p).ok());
        let srv: ID3D12DescriptorHeap = self.device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, NumDescriptors: (SLOT_TEXTURES * FRAME_SLOTS) as u32, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, NodeMask: 0 }).map_err(|e| text("the effect depth's view heap", e))?;
        let dsv: ID3D12DescriptorHeap = self.device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_DSV, NumDescriptors: FRAME_SLOTS as u32, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE, NodeMask: 0 }).map_err(|e| text("the effect depth's depth view heap", e))?;
        let mut lists = Vec::new();
        for _ in 0..FRAME_SLOTS {
            let allocator: ID3D12CommandAllocator = self.device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT).map_err(|e| text("allocator", e))?;
            let list: ID3D12GraphicsCommandList = self.device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator, None).map_err(|e| text("list", e))?;
            list.Close().map_err(|e| text("Close", e))?;
            lists.push((allocator, list, 0u64));
        }
        rlog!("render: (RSF2 round 14) THE GAME'S HALF-SIZE EFFECT DEPTH GETS THE KH CHARACTERS (hdmerge.ps.dxil, r.effects_depth): its fire, embers and glows behind them are hidden");
        Ok(FxKit { merge, fill, srv, dsv, lists })
    }

    /// (RSF2 round 14) The effect-depth pass on our own list (see `effects_depth_pass`).
    unsafe fn draw_effects_depth(&mut self, half: usize, half_planes: [i32; 2], scene_ptr: usize, scene_planes: [i32; 2]) -> windows::core::Result<()> {
        if self.fx_kit.is_none() {
            match self.make_fx_kit() {
                Ok(kit) => self.fx_kit = Some(kit),
                Err(e) => {
                    self.fx_failed = true;
                    rlog!("render: (RSF2 round 14) the effect depth pass is OFF ({e}): the game's half-size effects as before");
                    return Ok(());
                }
            }
        }
        let (raw_half, raw_scene) = (half as *mut c_void, scene_ptr as *mut c_void);
        let (Some(half_res), Some(scene_res)) = (ID3D12Resource::from_raw_borrowed(&raw_half), ID3D12Resource::from_raw_borrowed(&raw_scene)) else { return Ok(()) };
        let (hd, sd) = (half_res.GetDesc(), scene_res.GetDesc());
        // HARDENING: the measured shapes only (D32S8 at half and full size), states we can leave and come back to
        let ok_state = |s: i32| s != 0 && s & (D3D12_RESOURCE_STATE_RENDER_TARGET.0 | D3D12_RESOURCE_STATE_COPY_DEST.0 | D3D12_RESOURCE_STATE_COPY_SOURCE.0 | D3D12_RESOURCE_STATE_UNORDERED_ACCESS.0) == 0;
        if hd.Format != DXGI_FORMAT_R32G8X24_TYPELESS || sd.Format != DXGI_FORMAT_R32G8X24_TYPELESS || hd.Width * 2 != sd.Width || hd.Height * 2 != sd.Height || !half_planes.iter().all(|s| ok_state(*s)) || !scene_planes.iter().all(|s| ok_state(*s)) {
            return Ok(());
        }
        let Some(white) = self.assets.as_ref().map(|a| a.white.clone()) else { return Ok(()) };
        let done = self.fence.GetCompletedValue();
        let mode = EFFECTS_DEPTH.load(Ordering::Relaxed);
        let kit = self.fx_kit.as_mut().unwrap();
        let Some(k) = kit.lists.iter().position(|(_, _, busy)| *busy <= done) else { return Ok(()) };
        let size = self.device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV) as usize;
        let table = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: kit.srv.GetCPUDescriptorHandleForHeapStart().ptr + k * SLOT_TEXTURES * size };
        for slot in 0..8 {
            let at = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: table.ptr + slot * size };
            if slot == 5 {
                let desc = D3D12_SHADER_RESOURCE_VIEW_DESC { Format: DXGI_FORMAT_R32_FLOAT_X8X24_TYPELESS, ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D, Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING, Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 { Texture2D: D3D12_TEX2D_SRV { MostDetailedMip: 0, MipLevels: 1, PlaneSlice: 0, ResourceMinLODClamp: 0.0 } } };
                self.device.CreateShaderResourceView(scene_res, Some(&desc), at);
            } else {
                self.device.CreateShaderResourceView(&white, None, at);
            }
        }
        let dsv_inc = self.device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_DSV) as usize;
        let view = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: kit.dsv.GetCPUDescriptorHandleForHeapStart().ptr + k * dsv_inc };
        let vdesc = D3D12_DEPTH_STENCIL_VIEW_DESC { Format: DXGI_FORMAT_D32_FLOAT_S8X24_UINT, ViewDimension: D3D12_DSV_DIMENSION_TEXTURE2D, Flags: D3D12_DSV_FLAG_NONE, Anonymous: D3D12_DEPTH_STENCIL_VIEW_DESC_0 { Texture2D: D3D12_TEX2D_DSV { MipSlice: 0 } } };
        self.device.CreateDepthStencilView(half_res, Some(&vdesc), view);
        kit.lists[k].0.Reset()?;
        kit.lists[k].1.Reset(&kit.lists[k].0, None)?;
        let list = kit.lists[k].1.clone();
        let read = D3D12_RESOURCE_STATE_DEPTH_READ.0 | D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE.0 | D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE.0;
        let mut to: Vec<D3D12_RESOURCE_BARRIER> = Vec::new();
        let mut back: Vec<D3D12_RESOURCE_BARRIER> = Vec::new();
        for (p, s) in half_planes.iter().enumerate() {
            if *s != D3D12_RESOURCE_STATE_DEPTH_WRITE.0 {
                to.push(transition(half_res, p as u32, D3D12_RESOURCE_STATES(*s), D3D12_RESOURCE_STATE_DEPTH_WRITE));
                back.push(transition(half_res, p as u32, D3D12_RESOURCE_STATE_DEPTH_WRITE, D3D12_RESOURCE_STATES(*s)));
            }
        }
        for (p, s) in scene_planes.iter().enumerate() {
            if *s & D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE.0 == 0 {
                to.push(transition(scene_res, p as u32, D3D12_RESOURCE_STATES(*s), D3D12_RESOURCE_STATES(read)));
                back.push(transition(scene_res, p as u32, D3D12_RESOURCE_STATES(read), D3D12_RESOURCE_STATES(*s)));
            }
        }
        if !to.is_empty() {
            list.ResourceBarrier(&to);
        }
        list.OMSetRenderTargets(0, None, false, Some(&view));
        list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: hd.Width as f32, Height: hd.Height as f32, MinDepth: 0.0, MaxDepth: 1.0 }]);
        list.RSSetScissorRects(&[RECT { left: 0, top: 0, right: hd.Width as i32, bottom: hd.Height as i32 }]);
        list.SetGraphicsRootSignature(&self.root);
        list.SetDescriptorHeaps(&[Some(kit.srv.clone())]);
        list.SetGraphicsRootDescriptorTable(3, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: kit.srv.GetGPUDescriptorHandleForHeapStart().ptr + (k * SLOT_TEXTURES * size) as u64 });
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        list.SetPipelineState(if mode == 2 { kit.fill.as_ref().unwrap_or(&kit.merge) } else { &kit.merge });
        list.DrawInstanced(3, 1, 0, 0);
        if !back.is_empty() {
            list.ResourceBarrier(&back);
        }
        list.Close()?;
        self.queue.ExecuteCommandLists(&[Some(list.cast::<ID3D12CommandList>()?)]);
        self.fence_value += 1;
        self.queue.Signal(&self.fence, self.fence_value)?;
        kit.lists[k].2 = self.fence_value;
        FX_DRAWN.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    unsafe fn make_sun_kit(&mut self) -> Result<SunKit, String> {
        let text = |what: &str, e: windows::core::Error| format!("{what}: {e}");
        let pack = SHARED.lock().ok().and_then(|g| g.as_ref().map(|s| s.pack.clone())).ok_or("no pack")?;
        let dir = pack.join("materials").join("er");
        let read = |name: &str| std::fs::read(dir.join(name)).map_err(|e| format!("{name}: {e}"));
        let (screen, scene) = (read("blob.vs.dxil")?, read("dump8.ps.dxil")?);
        let format = DXGI_FORMAT_R32G32B32A32_FLOAT;
        let desc = texture_desc(CAST_ROW, crate::hybrid::CASC_MAX as u32, 1, 1, format, D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET);
        let texture = committed(&self.device, D3D12_HEAP_TYPE_DEFAULT, &desc, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, None).map_err(|e| text("cascade texture", e))?;
        let rtv: ID3D12DescriptorHeap = self.device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV, NumDescriptors: 1, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE, NodeMask: 0 }).map_err(|e| text("cascade rtv heap", e))?;
        self.device.CreateRenderTargetView(&texture, None, rtv.GetCPUDescriptorHandleForHeapStart());
        let root = game_root(&self.device).map_err(|e| text("game root signature", e))?;
        let copy = screen_pipeline(&self.device, &root, &screen, &scene, format).map_err(|e| text("cascade copy pipeline", e))?;
        let mut lists = Vec::new();
        for _ in 0..FRAME_SLOTS + 1 {
            let allocator: ID3D12CommandAllocator = self.device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT).map_err(|e| text("cascade allocator", e))?;
            let list: ID3D12GraphicsCommandList = self.device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator, None).map_err(|e| text("cascade list", e))?;
            list.Close().map_err(|e| text("Close", e))?;
            lists.push((allocator, list, 0u64));
        }
        rlog!("render: (RSF2 round 6) THE HYBRID'S SUN FROM THE GAME'S CASCADES: each cascade's constants are copied at the cast split (read only), the atlas is looked up at every KH pixel's own place (r.hybrid_sun)");
        Ok(SunKit { texture, rtv, root, copy, lists })
    }

    /// (RSF2 round 6) The constants of the game's caster passes (one row per cascade, finest first) into our texture; nothing of the
    /// game's is written. `sun_now` = what was read (none when the atlas or its state is not as measured).
    unsafe fn read_cascades(&mut self, sun: &survey::SunAtlas) -> windows::core::Result<()> {
        self.sun_now = None;
        if self.sun_kit.is_none() {
            match self.make_sun_kit() {
                Ok(kit) => self.sun_kit = Some(kit),
                Err(e) => {
                    self.sun_failed = true;
                    rlog!("render: (RSF2 round 6) the hybrid's sun from the cascades is OFF ({e}): the round-5 mean");
                    return Ok(());
                }
            }
        }
        let raw = sun.resource as *mut c_void;
        let Some(atlas) = ID3D12Resource::from_raw_borrowed(&raw) else { return Ok(()) };
        let desc = atlas.GetDesc();
        // HARDENING: a 16-bit depth atlas resting where a pixel shader may read it (the game's own state after its passes)
        if desc.Format != DXGI_FORMAT_R16_TYPELESS || desc.Dimension != D3D12_RESOURCE_DIMENSION_TEXTURE2D || desc.SampleDesc.Count != 1 || sun.state & D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE.0 == 0 || sun.state & (D3D12_RESOURCE_STATE_DEPTH_WRITE.0 | D3D12_RESOURCE_STATE_RENDER_TARGET.0) != 0 || sun.count == 0 {
            return Ok(());
        }
        let size = (desc.Width as u32, desc.Height);
        let passes: Vec<&survey::SunPass> = sun.passes.iter().take(sun.count).collect();
        let viewports: Vec<[f32; 4]> = passes.iter().map(|p| if p.table.handle == 0 { [0.0; 4] } else { p.viewport }).collect();
        let chosen = crate::hybrid::cascade_passes(size, &viewports);
        if chosen.is_empty() {
            return Ok(());
        }
        let done = self.fence.GetCompletedValue();
        let kit = self.sun_kit.as_mut().unwrap();
        let Some(k) = kit.lists.iter().position(|(_, _, busy)| *busy <= done) else { return Ok(()) };
        kit.lists[k].0.Reset()?;
        kit.lists[k].1.Reset(&kit.lists[k].0, None)?;
        let list = kit.lists[k].1.clone();
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        list.ResourceBarrier(&[transition(&kit.texture, 0, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, D3D12_RESOURCE_STATE_RENDER_TARGET)]);
        let view = kit.rtv.GetCPUDescriptorHandleForHeapStart();
        list.OMSetRenderTargets(1, Some(&view), false, None);
        let mut rows: Vec<[f32; 4]> = Vec::new();
        for pass_index in &chosen {
            let pass = passes[*pass_index];
            let Some(game_heap) = views_heap(pass.table.heaps) else { continue };
            let r = rows.len() as f32;
            list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: 0.0, TopLeftY: r, Width: CAST_ROW as f32, Height: 1.0, MinDepth: 0.0, MaxDepth: 1.0 }]);
            list.RSSetScissorRects(&[RECT { left: 0, top: r as i32, right: CAST_ROW as i32, bottom: r as i32 + 1 }]);
            list.SetGraphicsRootSignature(&kit.root);
            list.SetDescriptorHeaps(&[Some(game_heap)]);
            list.SetGraphicsRootDescriptorTable(1, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: pass.table.handle });
            list.SetPipelineState(&kit.copy);
            list.DrawInstanced(3, 1, 0, 0);
            rows.push(pass.viewport);
        }
        list.ResourceBarrier(&[transition(&kit.texture, 0, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE)]);
        list.Close()?;
        self.queue.ExecuteCommandLists(&[Some(list.cast::<ID3D12CommandList>()?)]);
        self.fence_value += 1;
        self.queue.Signal(&self.fence, self.fence_value)?;
        kit.lists[k].2 = self.fence_value;
        if !rows.is_empty() {
            SUN_READS.fetch_add(1, Ordering::Relaxed);
            if let Ok(mut sets) = SUN_SETS.lock() {
                let key = rows.iter().map(|v| format!("{}x{}@{},{}", v[2], v[3], v[0], v[1])).collect::<Vec<_>>().join(" ");
                match sets.iter().position(|s| s.0 == key) {
                    Some(at) => sets[at].1 += 1,
                    None if sets.len() < 8 => sets.push((key, 1)),
                    None => {}
                }
            }
            self.sun_now = Some(SunNow { atlas: sun.resource, width: size.0, height: size.1, viewports: rows, at: Instant::now() });
        }
        Ok(())
    }

    unsafe fn draw_cast(&mut self, frame: &VeloFrame, sun: &survey::SunAtlas) -> windows::core::Result<()> {
        if self.cast_kit.is_none() {
            match self.make_cast_kit() {
                Ok(kit) => self.cast_kit = Some(kit),
                Err(e) => {
                    self.cast_failed = true;
                    rlog!("render: his real shadow is OFF ({e}): the stand-in stays");
                    return Ok(());
                }
            }
        }
        let Some(assets) = self.assets.as_ref() else { return Ok(()) };
        let raw = sun.resource as *mut c_void;
        let Some(atlas) = ID3D12Resource::from_raw_borrowed(&raw) else { return Ok(()) };
        let desc = atlas.GetDesc();
        if desc.Format != DXGI_FORMAT_R16_TYPELESS || desc.Dimension != D3D12_RESOURCE_DIMENSION_TEXTURE2D || desc.SampleDesc.Count != 1 {
            return Ok(());
        }
        // (RSF2 round 15) the pass of the finest cascade (the largest viewport the cascades are read from: hybrid.rs cascade_passes)
        let all_vps: Vec<[f32; 4]> = sun.passes.iter().take(sun.count).map(|p| if p.table.handle == 0 { [0.0; 4] } else { p.viewport }).collect();
        let maps_pass = crate::hybrid::cascade_passes((desc.Width as u32, desc.Height), &all_vps).last().copied();
        if maps_pass.is_some() && self.kh_maps.is_none() && !KH_MAPS_FAILED.load(Ordering::Relaxed) {
            self.kh_maps = make_kh_maps(&self.device).map_err(|e| rlog!("render: (RSF2 round 15) the KH caster maps could not be made ({e}): the KH characters shade themselves through the atlas")).ok();
            KH_MAPS_FAILED.store(self.kh_maps.is_none(), Ordering::Relaxed);
            if self.kh_maps.is_some() {
                rlog!("render: (RSF2 round 15) THE KH CASTER MAPS: {KH_SLICES} x {0} x {0} D16, the finest cascade's light matrices: a KH character's own body in the sun atlas no longer shades him (r.shadow_real)", crate::hybrid::KH_MAP_SIZE);
            }
        }
        let done = self.fence.GetCompletedValue();
        let kit = self.cast_kit.as_mut().unwrap();
        let Some(k) = kit.lists.iter().position(|(_, _, busy)| *busy <= done) else { return Ok(()) };
        kit.lists[k].0.Reset()?;
        kit.lists[k].1.Reset(&kit.lists[k].0, None)?;
        let list = kit.lists[k].1.clone();
        let rest = D3D12_RESOURCE_STATES(sun.state);
        let data_view = kit.rtv.GetCPUDescriptorHandleForHeapStart();
        let depth_view = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: sun.view };
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        let debug = CAST_DEBUG.swap(0, Ordering::Relaxed) as usize;
        let mut drawn = 0u32;
        for (pass_index, pass) in sun.passes.iter().take(if debug > 0 { debug.min(sun.count) } else { sun.count }).enumerate() {
            let Some(game_heap) = views_heap(pass.table.heaps) else { continue };
            let v = pass.viewport;
            if pass.table.handle == 0 || v[2] < 1.0 || v[3] < 1.0 || v[0] < 0.0 || v[1] < 0.0 || v[0] + v[2] > desc.Width as f32 + 0.5 || v[1] + v[3] > desc.Height as f32 + 0.5 {
                continue;
            }
            // (1) the pass's cbSceneParam (the light's view-projection of this cascade) into the data texture
            list.ResourceBarrier(&[transition(&kit.texture, 0, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, D3D12_RESOURCE_STATE_RENDER_TARGET)]);
            list.OMSetRenderTargets(1, Some(&data_view), false, None);
            list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: CAST_ROW as f32, Height: 1.0, MinDepth: 0.0, MaxDepth: 1.0 }]);
            list.RSSetScissorRects(&[RECT { left: 0, top: 0, right: CAST_ROW as i32, bottom: 1 }]);
            list.SetGraphicsRootSignature(&kit.root);
            list.SetDescriptorHeaps(&[Some(game_heap)]);
            list.SetGraphicsRootDescriptorTable(1, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: pass.table.handle });
            list.SetPipelineState(&kit.copy);
            list.DrawInstanced(3, 1, 0, 0);
            list.ResourceBarrier(&[transition(&kit.texture, 0, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE)]);
            // (2) him, depth only, into this cascade's part of the atlas
            list.ResourceBarrier(&[transition(atlas, 0, rest, D3D12_RESOURCE_STATE_DEPTH_WRITE)]);
            list.OMSetRenderTargets(0, None, false, Some(&depth_view));
            list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: v[0], TopLeftY: v[1], Width: v[2], Height: v[3], MinDepth: 0.0, MaxDepth: 1.0 }]);
            let s = pass.scissor;
            let inside = s[2] > s[0] && s[3] > s[1] && s[0] >= 0 && s[1] >= 0 && s[2] as u64 <= desc.Width && s[3] as u32 <= desc.Height;
            list.RSSetScissorRects(&[if inside { RECT { left: s[0].max(v[0] as i32), top: s[1].max(v[1] as i32), right: s[2].min((v[0] + v[2]) as i32), bottom: s[3].min((v[1] + v[3]) as i32) } } else { RECT { left: v[0] as i32, top: v[1] as i32, right: (v[0] + v[2]) as i32, bottom: (v[1] + v[3]) as i32 } }]);
            // (testing aid, `r.shadow_dbg = 99`, held: the cascade's whole rectangle is cleared to "nearest": everything it covers is in shadow)
            if CAST_FLOOD.load(Ordering::Relaxed) {
                list.ClearDepthStencilView(depth_view, D3D12_CLEAR_FLAG_DEPTH, 1.0, 0, Some(&[RECT { left: v[0] as i32, top: v[1] as i32, right: (v[0] + v[2]) as i32, bottom: (v[1] + v[3]) as i32 }]));
            }
            list.SetGraphicsRootSignature(&self.root);
            list.SetDescriptorHeaps(&[Some(kit.heap.clone()), Some(self.sampler_heap.clone())]);
            list.SetGraphicsRootConstantBufferView(0, frame.frame_constants);
            list.SetGraphicsRootDescriptorTable(3, kit.heap.GetGPUDescriptorHandleForHeapStart());
            list.SetPipelineState(if CAST_PUSH.load(Ordering::Relaxed) { &kit.depth_dbg } else { &kit.depth });
            for (model_index, now, _) in &frame.skins {
                let Some(model) = assets.models.get(*model_index) else { continue };
                list.IASetVertexBuffers(0, Some(&[model.vertex_view]));
                list.IASetIndexBuffer(Some(&model.index_view));
                list.SetGraphicsRootConstantBufferView(2, *now);
                list.SetGraphicsRootConstantBufferView(6, *now);
                for draw in &model.draws {
                    if !assets.library.slots[draw.slot].translucent {
                        list.DrawIndexedInstanced(draw.index_count, 1, draw.first_index, draw.base_vertex, 0);
                    }
                }
            }
            // (ER party S4) the friends into the same cascade (their own skin shader)
            #[cfg(feature = "party")]
            if !frame.party.is_empty() {
                party::cast(&mut self.party, &frame.party, &self.device, &self.root, &list, &kit.heap, &self.sampler_heap);
            }
            list.ResourceBarrier(&[transition(atlas, 0, D3D12_RESOURCE_STATE_DEPTH_WRITE, rest)]);
            drawn += 1;
            // (RSF2 round 15) the KH caster maps from this pass (its matrices are in the data texture now): each character alone
            if Some(pass_index) == maps_pass {
                if let Some(maps) = self.kh_maps.as_ref() {
                    let dsv_inc = self.device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_DSV) as usize;
                    let slice_view = |k: usize| D3D12_CPU_DESCRIPTOR_HANDLE { ptr: maps.dsv.GetCPUDescriptorHandleForHeapStart().ptr + k * dsv_inc };
                    let size = crate::hybrid::KH_MAP_SIZE;
                    list.ResourceBarrier(&[transition(&maps.texture, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE | D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE, D3D12_RESOURCE_STATE_DEPTH_WRITE)]);
                    for k in 0..KH_SLICES {
                        list.ClearDepthStencilView(slice_view(k), D3D12_CLEAR_FLAG_DEPTH, 0.0, 0, None);
                    }
                    list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: size as f32, Height: size as f32, MinDepth: 0.0, MaxDepth: 1.0 }]);
                    list.RSSetScissorRects(&[RECT { left: 0, top: 0, right: size as i32, bottom: size as i32 }]);
                    list.OMSetRenderTargets(0, None, false, Some(&slice_view(0)));
                    list.SetGraphicsRootSignature(&self.root);
                    list.SetDescriptorHeaps(&[Some(kit.heap.clone()), Some(self.sampler_heap.clone())]);
                    list.SetGraphicsRootConstantBufferView(0, frame.frame_constants);
                    list.SetGraphicsRootDescriptorTable(3, kit.heap.GetGPUDescriptorHandleForHeapStart());
                    list.SetPipelineState(&kit.depth);
                    for (model_index, now, _) in &frame.skins {
                        let Some(model) = assets.models.get(*model_index) else { continue };
                        list.IASetVertexBuffers(0, Some(&[model.vertex_view]));
                        list.IASetIndexBuffer(Some(&model.index_view));
                        list.SetGraphicsRootConstantBufferView(2, *now);
                        list.SetGraphicsRootConstantBufferView(6, *now);
                        for draw in &model.draws {
                            if !assets.library.slots[draw.slot].translucent {
                                list.DrawIndexedInstanced(draw.index_count, 1, draw.first_index, draw.base_vertex, 0);
                            }
                        }
                    }
                    #[cfg(feature = "party")]
                    for (k, member) in frame.party.iter().take(KH_SLICES - 1).enumerate() {
                        list.OMSetRenderTargets(0, None, false, Some(&slice_view(k + 1)));
                        party::cast(&mut self.party, &vec![member.clone()], &self.device, &self.root, &list, &kit.heap, &self.sampler_heap);
                    }
                    list.ResourceBarrier(&[transition(&maps.texture, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_DEPTH_WRITE, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE | D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE)]);
                    self.kh_maps_vp = Some((pass.viewport, Instant::now()));
                    KH_MAPS_DRAWN.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
        list.Close()?;
        self.queue.ExecuteCommandLists(&[Some(list.cast::<ID3D12CommandList>()?)]);
        self.fence_value += 1;
        self.queue.Signal(&self.fence, self.fence_value)?;
        kit.lists[k].2 = self.fence_value;
        // (his constants are in that frame slot's buffer: it is not written again before the GPU has drawn this)
        if let Some(slot) = self.slots.get_mut(frame.slot) {
            slot.busy_until = slot.busy_until.max(self.fence_value);
        }
        if drawn > 0 {
            self.cast_drawn += drawn;
            self.cast_at = Some(Instant::now());
        }
        if debug > 0 {
            // (discovery: the data texture holds the row of the last pass drawn; read it back, waiting for the GPU)
            let kit = self.cast_kit.as_ref().unwrap();
            let desc = texture_desc(CAST_ROW, 1, 1, 1, DXGI_FORMAT_R32G32B32A32_FLOAT, D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET);
            let mut footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default();
            let mut total = 0u64;
            self.device.GetCopyableFootprints(&desc, 0, 1, 0, Some(&mut footprint), None, None, Some(&mut total));
            let readback = committed(&self.device, D3D12_HEAP_TYPE_READBACK, &buffer_desc(total), D3D12_RESOURCE_STATE_COPY_DEST, None)?;
            let wait = |core: &Core| {
                let t = Instant::now();
                while core.fence.GetCompletedValue() < core.fence_value && t.elapsed() < Duration::from_secs(3) {
                    std::thread::sleep(Duration::from_millis(1));
                }
            };
            wait(self);
            let (allocator, list, _) = &kit.lists[(k + 1) % kit.lists.len()];
            allocator.Reset()?;
            list.Reset(allocator, None)?;
            list.ResourceBarrier(&[transition(&kit.texture, 0, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, D3D12_RESOURCE_STATE_COPY_SOURCE)]);
            let to = D3D12_TEXTURE_COPY_LOCATION { pResource: std::mem::transmute_copy(&readback), Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT, Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { PlacedFootprint: footprint } };
            let from = D3D12_TEXTURE_COPY_LOCATION { pResource: std::mem::transmute_copy(&kit.texture), Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX, Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { SubresourceIndex: 0 } };
            list.CopyTextureRegion(&to, 0, 0, 0, &from, None);
            list.ResourceBarrier(&[transition(&kit.texture, 0, D3D12_RESOURCE_STATE_COPY_SOURCE, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE)]);
            list.Close()?;
            self.queue.ExecuteCommandLists(&[Some(list.cast::<ID3D12CommandList>()?)]);
            self.fence_value += 1;
            self.queue.Signal(&self.fence, self.fence_value)?;
            wait(self);
            let mut at: *mut c_void = std::ptr::null_mut();
            readback.Map(0, None, Some(&mut at))?;
            let words = std::slice::from_raw_parts((at as *const u8).add(footprint.Offset as usize) as *const f32, CAST_ROW as usize * 4).to_vec();
            readback.Unmap(0, Some(&D3D12_RANGE { Begin: 0, End: 0 }));
            let row = |k: usize| format!("{:?}", &words[k * 4..k * 4 + 4]);
            rlog!("shadow dbg: atlas {}x{} state {:#x} view {:#x}; {} pass(es): {}", desc_w(atlas), desc_h(atlas), sun.state, sun.view, sun.count, sun.passes.iter().take(sun.count).map(|p| format!("vp {:?} sc {:?} table {:#x}", p.viewport, p.scissor, p.table.handle)).collect::<Vec<_>>().join(" | "));
            rlog!("shadow dbg: pass {} cbSceneParam rows: 1 screen {} | 3 camera {} | 5-7 view {} {} {} | 16-19 proj {} {} {} {} | 24-27 viewproj {} {} {} {}", debug - 1, row(1), row(3), row(5), row(6), row(7), row(16), row(17), row(18), row(19), row(24), row(25), row(26), row(27));
        }
        Ok(())
    }

    /// (IMPL_ER80) THE LATE RE-COMPOSITE AT THE VELOCITY SPLIT: everything it needs, checked for this frame. `Some(state)` =
    /// draw it (the scene target's state at the split); then the late pass does not paste again. `None` with the re-composite
    /// wanted = no re-composite in this frame (counted, with the reason; never the post-TAA paste in its place).
    unsafe fn recomp_at_split(&mut self, sub: &survey::Submission) -> Option<i32> {
        if !recomp_split_wanted() {
            return None;
        }
        let late = self.late.as_mut()?;
        if !late.recomp {
            return None;
        }
        // (from here the frame wanted it: whatever happens, no paste after the TAA in this frame)
        late.recomp = false;
        let (target, width, height) = (late.target, late.width, late.height);
        let state = sub.watch_state;
        if state == 0 || state & (D3D12_RESOURCE_STATE_RENDER_TARGET.0 | D3D12_RESOURCE_STATE_DEPTH_WRITE.0 | D3D12_RESOURCE_STATE_COPY_DEST.0) != 0 {
            recomp_split_missed("the scene target's state at the velocity split is unknown or not a resting read state");
            return None;
        }
        if !sub.clean || !sub.depth_seen || sub.depth_states.iter().any(|s| *s & D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE.0 == 0 || *s & D3D12_RESOURCE_STATE_DEPTH_WRITE.0 != 0) {
            recomp_split_missed("the scene depth is not readable at the velocity split");
            return None;
        }
        if !survey::usable_now(target, width, height) {
            recomp_split_missed("the scene target was not transitioned by the game in this frame");
            return None;
        }
        let raw = target as *mut c_void;
        let format = ID3D12Resource::from_raw_borrowed(&raw)?.GetDesc().Format;
        let kit = self.late_kit.as_mut()?;
        if kit.target != target {
            recomp_split_missed("the saved colour is of another target");
            return None;
        }
        if kit.pipeline2.as_ref().map(|p| p.0) != Some(format.0) && !kit.pipeline2_failed {
            let pack = SHARED.lock().ok().and_then(|g| g.as_ref().map(|s| s.pack.clone()));
            let made = pack.and_then(|pack| {
                let dir = pack.join("materials").join("er");
                let (vertex, pixel) = (std::fs::read(dir.join("blob.vs.dxil")).ok()?, std::fs::read(dir.join("recomp2.ps.dxil")).ok()?);
                screen_blend_pipeline(&self.device, &self.root, &vertex, &pixel, format).ok()
            });
            match made {
                Some(p) => kit.pipeline2 = Some((format.0, p)),
                None => {
                    kit.pipeline2_failed = true;
                    rlog!("render: (IMPL_ER80) the late re-composite before the TAA cannot be made (materials/er/recomp2.ps.dxil missing?): none");
                }
            }
        }
        if kit.pipeline2.as_ref().map(|p| p.0) != Some(format.0) {
            recomp_split_missed("recomp2.ps.dxil is not in the pack (or its pipeline state failed)");
            return None;
        }
        Some(state)
    }

    /// The draws at the game's velocity split (one list, executed between the game's velocity list and the rest of its
    /// forward submission, i.e. before its TAA): (IMPL_ER80) the late re-composite when `recomp` (the scene target's state
    /// there), then his motion vectors when `velocity` (the frame, the game's velocity target and its state).
    unsafe fn draw_split(&mut self, velocity: Option<(&VeloFrame, usize, i32)>, recomp: Option<i32>, depth_states: [i32; 2]) -> windows::core::Result<()> {
        let Some(slot_index) = velocity.map(|v| v.0.slot).or_else(|| self.late.as_ref().map(|l| l.slot)).or_else(|| self.glow.hair_late.as_ref().map(|h| h.slot)) else { return Ok(()) };
        let slot = &self.slots[slot_index];
        slot.velo_allocator.Reset()?;
        slot.velo_list.Reset(&slot.velo_allocator, None)?;
        let list = &slot.velo_list;
        let rtv_size = self.device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV) as usize;
        let mut recorded = false;
        // (IMPL_ER80) the late re-composite, first: his colour saved right after his draw, where the depth is still his, blended
        // by the smooth far weight; the scene target from its resting state to RENDER_TARGET and back
        if let (Some(scene_state), Some(late), Some(kit)) = (recomp, self.late.as_ref(), self.late_kit.as_ref()) {
            let raw = late.target as *mut c_void;
            if let (Some(scene_target), Some((_, pipeline))) = (ID3D12Resource::from_raw_borrowed(&raw), kit.pipeline2.as_ref()) {
                let view = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: self.rtv_heap.GetCPUDescriptorHandleForHeapStart().ptr + (2 * FRAME_SLOTS + late.slot) * rtv_size };
                self.device.CreateRenderTargetView(scene_target, None, view);
                let barrier = scene_state != D3D12_RESOURCE_STATE_RENDER_TARGET.0;
                if barrier {
                    list.ResourceBarrier(&[transition(scene_target, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATES(scene_state), D3D12_RESOURCE_STATE_RENDER_TARGET)]);
                }
                list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: late.width as f32, Height: late.height as f32, MinDepth: 0.0, MaxDepth: 1.0 }]);
                list.RSSetScissorRects(&[RECT { left: 0, top: 0, right: late.width as i32, bottom: late.height as i32 }]);
                list.SetGraphicsRootSignature(&self.root);
                list.SetDescriptorHeaps(&[Some(kit.heap.clone()), Some(self.sampler_heap.clone())]);
                list.SetGraphicsRootConstantBufferView(0, late.frame_constants);
                list.SetGraphicsRootDescriptorTable(3, kit.heap.GetGPUDescriptorHandleForHeapStart());
                list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
                list.OMSetRenderTargets(1, Some(&view), false, None);
                list.SetPipelineState(pipeline);
                list.DrawInstanced(3, 1, 0, 0);
                if barrier {
                    list.ResourceBarrier(&[transition(scene_target, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATES(scene_state))]);
                }
                self.recomp_drawn += 1;
                RECOMP_SPLIT.fetch_add(1, Ordering::Relaxed);
                recorded = true;
            }
        }
        // (ER87) THE HAIR FRINGE (hair.rs): over the game's forward stage, before its TAA; depth GREATER against the scene depth
        // as it is now (his core is in it), nothing written but the colour
        if let Some(hair) = self.glow.hair_late.take() {
            // (ER91) with the game's velocity target: the 4x pass writes the strands' own motion vectors too
            recorded |= self.record_hair(&hair, slot_index, depth_states, velocity.map(|(_, t, s)| (t, s)));
        }
        if let Some((frame, target, target_state)) = velocity {
            recorded |= self.record_velocity(frame, target, target_state, depth_states)?;
        }
        let slot = &self.slots[slot_index];
        slot.velo_list.Close()?;
        if !recorded {
            // (nothing in the list: closed and not executed)
            return Ok(());
        }
        self.queue.ExecuteCommandLists(&[Some(slot.velo_list.cast::<ID3D12CommandList>()?)]);
        self.fence_value += 1;
        self.queue.Signal(&self.fence, self.fence_value)?;
        self.slots[slot_index].busy_until = self.fence_value;
        Ok(())
    }

    /// (ER87) The hair fringe into the scene target at the velocity split (draw_split's list): the scene target from its state
    /// there to RENDER_TARGET, the scene depth's planes to DEPTH_WRITE (tested, not written), and back. True when recorded.
    unsafe fn record_hair(&mut self, hair: &HairLate, slot_index: usize, depth_states: [i32; 2], velocity: Option<(usize, i32)>) -> bool {
        let state = self.glow.hair_split_state;
        let depth_ok = depth_states.iter().all(|s| *s != 0 && *s & D3D12_RESOURCE_STATE_RENDER_TARGET.0 == 0);
        if state == 0 || state & (D3D12_RESOURCE_STATE_DEPTH_WRITE.0 | D3D12_RESOURCE_STATE_COPY_DEST.0) != 0 || !depth_ok || slot_index != hair.slot {
            crate::hair::missed("the scene target's or the depth's state at the velocity split is unknown");
            return false;
        }
        let Some(scene) = survey::scene_depth(hair.width, hair.height).filter(|s| s.resource.as_raw() as usize == hair.depth && s.fresh) else {
            crate::hair::missed("the scene depth is not the colour draw's");
            return false;
        };
        if !survey::usable_now(hair.target, hair.width, hair.height) {
            crate::hair::missed("the scene target was not transitioned by the game in this frame");
            return false;
        }
        let (Some(assets), Some(set)) = (self.assets.as_ref(), self.glow.hair_sets.get(&(hair.key, false))) else { return false };
        if set.len() != assets.library.slots.len() {
            return false;
        }
        let raw = hair.target as *mut c_void;
        let Some(target) = ID3D12Resource::from_raw_borrowed(&raw) else { return false };
        let slot = &self.slots[slot_index];
        let list = &slot.velo_list;
        let rtv_size = self.device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV) as usize;
        // (the re-composite's view of this slot: the same scene target)
        let view = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: self.rtv_heap.GetCPUDescriptorHandleForHeapStart().ptr + (2 * FRAME_SLOTS + slot_index) * rtv_size };
        self.device.CreateRenderTargetView(target, None, view);
        let depth_view = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: self.dsv_heap.GetCPUDescriptorHandleForHeapStart().ptr + (1 + slot_index) * self.dsv_size };
        let mut before = Vec::new();
        let mut after = Vec::new();
        if state != D3D12_RESOURCE_STATE_RENDER_TARGET.0 {
            before.push(transition(target, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATES(state), D3D12_RESOURCE_STATE_RENDER_TARGET));
            after.push(transition(target, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATES(state)));
        }
        for (plane, s) in depth_states.iter().enumerate() {
            if *s != D3D12_RESOURCE_STATE_DEPTH_WRITE.0 {
                before.push(transition(&scene.resource, plane as u32, D3D12_RESOURCE_STATES(*s), D3D12_RESOURCE_STATE_DEPTH_WRITE));
                after.push(transition(&scene.resource, plane as u32, D3D12_RESOURCE_STATE_DEPTH_WRITE, D3D12_RESOURCE_STATES(*s)));
            }
        }
        if !before.is_empty() {
            list.ResourceBarrier(&before);
        }
        let srv_start = assets.srv_heap.GetGPUDescriptorHandleForHeapStart();
        // (ER91) THE 4x HAIR PASS (hair91.rs) in place of the fringe; any failure: the fringe as before
        let mut fringe_wanted = true;
        if let Some((box_, params, stencil, snapshot)) = hair.h91 {
            match (self.glow.hair_msaa.get(&hair.key), SHARED.lock().ok().and_then(|g| g.as_ref().map(|s| s.pack.join("materials").join("er")))) {
                (Some(msaa), Some(dir)) if msaa.len() == assets.library.slots.len() => {
                    let velocity_raw = velocity.map(|(t, s)| (t as *mut c_void, s));
                    let snapshot_raw = snapshot.map(|s| s as *mut c_void);
                    let snapshot = snapshot_raw.as_ref().and_then(|raw| ID3D12Resource::from_raw_borrowed(raw)).filter(|s| self.snapshot.as_ref().is_some_and(|(r, ..)| r.as_raw() == s.as_raw()));
                    let velocity_target = velocity_raw.as_ref().and_then(|(raw, s)| ID3D12Resource::from_raw_borrowed(raw).map(|r| (r, *s)));
                    let frame = crate::hair91::Frame {
                        device: &self.device,
                        root: &self.root,
                        list,
                        sampler_heap: &self.sampler_heap,
                        sampler_index: self.sampler_index.GetGPUVirtualAddress(),
                        srv_heap: &assets.srv_heap,
                        table_start: srv_start.ptr,
                        table_stride: (SLOT_TEXTURES * assets.srv_size) as u64,
                        scene: target,
                        scene_state: D3D12_RESOURCE_STATE_RENDER_TARGET.0,
                        scene_view: view,
                        depth: &scene.resource,
                        depth_states: [D3D12_RESOURCE_STATE_DEPTH_WRITE.0; 2],
                        depth_view,
                        dsv_format: DXGI_FORMAT(hair.key.depth),
                        width: hair.width,
                        height: hair.height,
                        box_,
                        frame_constants: hair.frame_constants,
                        params,
                        velocity: velocity_target,
                        stencil,
                        snapshot,
                        completed: self.fence.GetCompletedValue(),
                        fence_next: self.fence_value + 1,
                    };
                    match self.glow.h91.record(&frame, &dir, &hair.draws, msaa, &|m| assets.models.get(m).map(|model| (model.vertex_view, model.index_view))) {
                        Ok(()) => fringe_wanted = false,
                        Err(why) => crate::hair91::missed(why),
                    }
                }
                _ => crate::hair91::missed("the outfit's 4x hair states are not made yet"),
            }
        }
        list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: hair.width as f32, Height: hair.height as f32, MinDepth: 0.0, MaxDepth: 1.0 }]);
        list.RSSetScissorRects(&[RECT { left: 0, top: 0, right: hair.width as i32, bottom: hair.height as i32 }]);
        list.SetGraphicsRootSignature(&self.root);
        list.SetDescriptorHeaps(&[Some(assets.srv_heap.clone()), Some(self.sampler_heap.clone())]);
        list.SetGraphicsRootConstantBufferView(0, hair.frame_constants);
        list.SetGraphicsRootDescriptorTable(4, self.sampler_heap.GetGPUDescriptorHandleForHeapStart());
        list.SetGraphicsRootShaderResourceView(5, self.sampler_index.GetGPUVirtualAddress());
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        list.OMSetRenderTargets(1, Some(&view), false, Some(&depth_view));
        if fringe_wanted {
            crate::hair::draw_fringe(list, set, &hair.draws, &|m| assets.models.get(m).map(|model| (model.vertex_view, model.index_view)), srv_start.ptr, (SLOT_TEXTURES * assets.srv_size) as u64);
        }
        // (then the translucent covers held back for it, with their own colour states)
        if let (false, Some(pipelines)) = (hair.translucent.is_empty(), self.pso_sets.get(&hair.key)) {
            let states: Vec<Option<ID3D12PipelineState>> = pipelines.iter().take(assets.library.slots.len()).cloned().map(Some).collect();
            crate::hair::draw_fringe(list, &states, &hair.translucent, &|m| assets.models.get(m).map(|model| (model.vertex_view, model.index_view)), srv_start.ptr, (SLOT_TEXTURES * assets.srv_size) as u64);
        }
        if !after.is_empty() {
            list.ResourceBarrier(&after);
        }
        crate::hair::drawn_late();
        true
    }

    /// His motion vectors into the game's velocity target, recorded into the frame slot's split list (draw_split). True when
    /// recorded.
    unsafe fn record_velocity(&mut self, frame: &VeloFrame, target: usize, target_state: i32, depth_states: [i32; 2]) -> windows::core::Result<bool> {
        let Some(scene) = survey::scene_depth(frame.width, frame.height) else { return Ok(false) };
        if scene.resource.as_raw() as usize != frame.depth || !scene.fresh {
            return Ok(false);
        }
        let (Some(assets), Some(pipelines)) = (self.assets.as_ref(), self.pso_sets.get(&frame.key)) else { return Ok(false) };
        let Some(pipeline) = pipelines.get(assets.library.slots.len() + 1) else { return Ok(false) };
        // (ER88) the own-record velocity state when there is one (taa.rs; the frame constants say per frame which route)
        let own_state = crate::taa::velocity_state(key_bits(frame.key));
        let pipeline = own_state.as_ref().unwrap_or(pipeline);
        let raw = target as *mut c_void;
        let Some(target) = ID3D12Resource::from_raw_borrowed(&raw) else { return Ok(false) };
        let desc = target.GetDesc();
        if desc.Format != DXGI_FORMAT_R16G16_FLOAT || desc.Width != frame.width as u64 || desc.Height != frame.height {
            return Ok(false);
        }
        let slot = &self.slots[frame.slot];
        let list = &slot.velo_list;
        let rtv_size = self.device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV) as usize;
        let target_view = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: self.rtv_heap.GetCPUDescriptorHandleForHeapStart().ptr + (FRAME_SLOTS + frame.slot) * rtv_size };
        self.device.CreateRenderTargetView(target, None, target_view);
        // (the depth view of this frame slot was made by the colour draw)
        let depth_view = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: self.dsv_heap.GetCPUDescriptorHandleForHeapStart().ptr + (1 + frame.slot) * self.dsv_size };
        let rest = D3D12_RESOURCE_STATES(target_state);
        let mut before = vec![transition(target, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, rest, D3D12_RESOURCE_STATE_RENDER_TARGET)];
        let mut after = vec![transition(target, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_RENDER_TARGET, rest)];
        if target_state == D3D12_RESOURCE_STATE_RENDER_TARGET.0 {
            before.clear();
            after.clear();
        }
        for (plane, state) in depth_states.iter().enumerate() {
            if *state != D3D12_RESOURCE_STATE_DEPTH_WRITE.0 {
                before.push(transition(&scene.resource, plane as u32, D3D12_RESOURCE_STATES(*state), D3D12_RESOURCE_STATE_DEPTH_WRITE));
                after.push(transition(&scene.resource, plane as u32, D3D12_RESOURCE_STATE_DEPTH_WRITE, D3D12_RESOURCE_STATES(*state)));
            }
        }
        if !before.is_empty() {
            list.ResourceBarrier(&before);
        }
        list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: frame.width as f32, Height: frame.height as f32, MinDepth: 0.0, MaxDepth: 1.0 }]);
        list.RSSetScissorRects(&[RECT { left: 0, top: 0, right: frame.width as i32, bottom: frame.height as i32 }]);
        list.SetGraphicsRootSignature(&self.root);
        list.SetDescriptorHeaps(&[Some(assets.srv_heap.clone()), Some(self.sampler_heap.clone())]);
        list.SetGraphicsRootConstantBufferView(0, frame.frame_constants);
        // (any slot's table: its entry 16 is the game-data texture)
        list.SetGraphicsRootDescriptorTable(3, assets.srv_heap.GetGPUDescriptorHandleForHeapStart());
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        list.OMSetRenderTargets(1, Some(&target_view), false, Some(&depth_view));
        list.SetPipelineState(pipeline);
        for (model_index, now, earlier) in &frame.skins {
            let Some(model) = assets.models.get(*model_index) else { continue };
            list.IASetVertexBuffers(0, Some(&[model.vertex_view]));
            list.IASetIndexBuffer(Some(&model.index_view));
            list.SetGraphicsRootConstantBufferView(2, *now);
            list.SetGraphicsRootConstantBufferView(6, *earlier);
            for draw in &model.draws {
                if !assets.library.slots[draw.slot].translucent {
                    list.DrawIndexedInstanced(draw.index_count, 1, draw.first_index, draw.base_vertex, 0);
                }
            }
        }
        // (ER party S4) the friends' motion vectors
        #[cfg(feature = "party")]
        if !frame.party.is_empty() {
            party::velocity(&mut self.party, &frame.party, frame.key, list, &self.sampler_heap);
        }
        if !after.is_empty() {
            list.ResourceBarrier(&after);
        }
        self.velocity_drawn += 1;
        Ok(true)
    }

    unsafe fn draw_late(&mut self, late: &LateFx, target_state: i32, depth_states: [i32; 2], dim: f32) -> windows::core::Result<()> {
        let Some(scene) = survey::scene_depth(late.width, late.height) else { return Ok(()) };
        if scene.resource.as_raw() as usize != late.depth || !scene.fresh {
            return Ok(());
        }
        let raw = late.target as *mut c_void;
        let Some(target) = ID3D12Resource::from_raw_borrowed(&raw) else { return Ok(()) };
        let desc = target.GetDesc();
        if desc.Width != late.width as u64 || desc.Height != late.height || desc.SampleDesc.Count != 1 {
            return Ok(());
        }
        // The darkening pipeline: a full-screen triangle whose blend is destination x blend factor
        // (what its pixel shader outputs does not matter).
        if dim < 0.999 && self.dim.as_ref().map(|d| d.0) != Some(desc.Format.0) {
            self.dim = None;
            let pack = SHARED.lock().ok().and_then(|g| g.as_ref().map(|s| s.pack.clone()));
            let made = pack.and_then(|pack| {
                let dir = pack.join("materials").join("er");
                let (vertex, pixel) = (std::fs::read(dir.join("blob.vs.dxil")).ok()?, std::fs::read(dir.join("probe.ps.dxil")).ok()?);
                dim_pipeline(&self.device, &self.root, &vertex, &pixel, desc.Format).ok()
            });
            match made {
                Some(pipeline) => self.dim = Some((desc.Format.0, pipeline)),
                None => self.tell("the scene darkening pipeline could not be made: Thunder does not darken the scene".to_string()),
            }
        }
        // (IMPL_ER65) the late re-composite's pipeline (blob.vs + recomp.ps.dxil, the target's format), once
        let recomp = late.recomp && depth_states.iter().all(|s| *s & D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE.0 != 0);
        if let (true, Some(kit)) = (recomp, self.late_kit.as_mut()) {
            if kit.pipeline.as_ref().map(|p| p.0) != Some(desc.Format.0) && !kit.pipeline_failed {
                let pack = SHARED.lock().ok().and_then(|g| g.as_ref().map(|s| s.pack.clone()));
                let made = pack.and_then(|pack| {
                    let dir = pack.join("materials").join("er");
                    let (vertex, pixel) = (std::fs::read(dir.join("blob.vs.dxil")).ok()?, std::fs::read(dir.join("recomp.ps.dxil")).ok()?);
                    screen_pipeline(&self.device, &self.root, &vertex, &pixel, desc.Format).ok()
                });
                match made {
                    Some(p) => kit.pipeline = Some((desc.Format.0, p)),
                    None => {
                        kit.pipeline_failed = true;
                        rlog!("render: (IMPL_ER65) the late re-composite cannot be made (materials/er/recomp.ps.dxil missing?): none");
                    }
                }
            }
        }
        let Some(assets) = self.assets.as_ref() else { return Ok(()) };
        let slot = &self.slots[late.slot];
        slot.late_allocator.Reset()?;
        slot.late_list.Reset(&slot.late_allocator, None)?;
        let list = &slot.late_list;
        let rtv_size = self.device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV) as usize;
        let target_view = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: self.rtv_heap.GetCPUDescriptorHandleForHeapStart().ptr + (2 * FRAME_SLOTS + late.slot) * rtv_size };
        self.device.CreateRenderTargetView(target, None, target_view);
        // (the depth view of this frame slot was made by the colour draw)
        let depth_view = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: self.dsv_heap.GetCPUDescriptorHandleForHeapStart().ptr + (1 + late.slot) * self.dsv_size };
        let rest = D3D12_RESOURCE_STATES(target_state);
        let mut before = vec![transition(target, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, rest, D3D12_RESOURCE_STATE_RENDER_TARGET)];
        let mut after = vec![transition(target, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_RENDER_TARGET, rest)];
        for (plane, state) in depth_states.iter().enumerate() {
            before.push(transition(&scene.resource, plane as u32, D3D12_RESOURCE_STATES(*state), D3D12_RESOURCE_STATE_DEPTH_WRITE));
            after.push(transition(&scene.resource, plane as u32, D3D12_RESOURCE_STATE_DEPTH_WRITE, D3D12_RESOURCE_STATES(*state)));
        }
        list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: late.width as f32, Height: late.height as f32, MinDepth: 0.0, MaxDepth: 1.0 }]);
        list.RSSetScissorRects(&[RECT { left: 0, top: 0, right: late.width as i32, bottom: late.height as i32 }]);
        // (IMPL_ER65) THE LATE RE-COMPOSITE, first: the depth is still readable (its resting state), the target goes to
        // RENDER_TARGET; his saved colour where the depth is still his (recomp.ps.dxil)
        if let (true, Some(kit)) = (recomp, self.late_kit.as_ref()) {
            if let Some((_, pipeline)) = kit.pipeline.as_ref() {
                list.ResourceBarrier(&before[..1]);
                list.SetGraphicsRootSignature(&self.root);
                list.SetDescriptorHeaps(&[Some(kit.heap.clone()), Some(self.sampler_heap.clone())]);
                list.SetGraphicsRootConstantBufferView(0, late.frame_constants);
                list.SetGraphicsRootDescriptorTable(3, kit.heap.GetGPUDescriptorHandleForHeapStart());
                list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
                list.OMSetRenderTargets(1, Some(&target_view), false, None);
                list.SetPipelineState(pipeline);
                list.DrawInstanced(3, 1, 0, 0);
                list.ResourceBarrier(&after[..1]);
                self.recomp_drawn += 1;
            }
        }
        list.ResourceBarrier(&before);
        list.OMSetRenderTargets(1, Some(&target_view), false, Some(&depth_view));
        if let (true, Some((_, pipeline))) = (dim < 0.999, self.dim.as_ref()) {
            list.SetGraphicsRootSignature(&self.root);
            list.SetDescriptorHeaps(&[Some(assets.srv_heap.clone()), Some(self.sampler_heap.clone())]);
            list.SetGraphicsRootConstantBufferView(0, late.frame_constants);
            list.SetGraphicsRootDescriptorTable(3, assets.srv_heap.GetGPUDescriptorHandleForHeapStart());
            list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            list.OMSetRenderTargets(1, Some(&target_view), false, None);
            list.OMSetBlendFactor(Some(&[dim, dim, dim, 1.0]));
            list.SetPipelineState(pipeline);
            list.DrawInstanced(3, 1, 0, 0);
            list.OMSetRenderTargets(1, Some(&target_view), false, Some(&depth_view));
            crate::vfx::DIMMED_FRAMES.fetch_add(1, Ordering::Relaxed);
        }
        if late.effects {
            crate::vfx::draw(&crate::vfx::DrawCx { device: &self.device, list, slot: late.slot, target, target_view, depth: &scene.resource, depth_format: late.depth_format, depth_view, width: late.width, height: late.height, clip_from_game: late.clip, camera: late.camera, right: late.right, up: late.up, forward: late.forward, depth_a: late.depth_a, depth_b: late.depth_b, proj00: late.proj00, scale: late.scale, lit: late.lit, stage: crate::vfx::Stage::Late });
            // (ZFIN) ZETTAFLARE'S FINALE on an Elden Ring character (finale.rs), after the effects' late stage
            let view = crate::finale::View { clip: late.clip, camera: late.camera, right: late.right, up: late.up, forward: late.forward, depth_a: late.depth_a, depth_b: late.depth_b, proj00: late.proj00, width: late.width, height: late.height };
            let snap = late.snap.as_ref().filter(|_| self.snapshot.as_ref().is_some_and(|s| s.1 == late.depth));
            let fcx = crate::finale::FinaleCx { device: &self.device, list, root: &self.root, slot: late.slot, target_view, target_format: desc.Format, depth: &scene.resource, snapshot: snap, view, scale: late.scale };
            if crate::finale::draw(&fcx) {
                list.OMSetRenderTargets(1, Some(&target_view), false, Some(&depth_view));
            }
            // (ZFIN) the effects drawn over the finale (ZETTA's fragments, `r.finale_over`), held back by the late stage above
            if crate::finale::active() {
                crate::vfx::draw(&crate::vfx::DrawCx { device: &self.device, list, slot: late.slot, target, target_view, depth: &scene.resource, depth_format: late.depth_format, depth_view, width: late.width, height: late.height, clip_from_game: late.clip, camera: late.camera, right: late.right, up: late.up, forward: late.forward, depth_a: late.depth_a, depth_b: late.depth_b, proj00: late.proj00, scale: late.scale, lit: late.lit, stage: crate::vfx::Stage::OverFinale });
            }
        }
        list.ResourceBarrier(&after);
        list.Close()?;
        self.queue.ExecuteCommandLists(&[Some(list.cast::<ID3D12CommandList>()?)]);
        self.fence_value += 1;
        self.queue.Signal(&self.fence, self.fence_value)?;
        self.slots[late.slot].busy_until = self.fence_value;
        self.late_drawn += 1;
        Ok(())
    }

    /// The objects of the game-light pass (once).
    unsafe fn make_game_kit(&mut self) -> Result<GameKit, String> {
        let text = |what: &str, e: windows::core::Error| format!("{what}: {e}");
        let pack = SHARED.lock().ok().and_then(|g| g.as_ref().map(|s| s.pack.clone())).ok_or("no pack")?;
        let dir = pack.join("materials").join("er");
        let read = |name: &str| std::fs::read(dir.join(name)).map_err(|e| format!("{name}: {e} (cargo run --release -p kh-shaderc)"));
        // (IMPL_ER64) probe2.ps.dxil (the probes + the ring for the world's lights on him) when the pack has it
        let ring = crate::pointlight::settings().world && dir.join("probe2.ps.dxil").is_file();
        let (vertex, light, scene, probe) = (read("blob.vs.dxil")?, read("dump5.ps.dxil")?, read("dump8.ps.dxil")?, read(if ring { "probe2.ps.dxil" } else { "probe.ps.dxil" })?);
        // (IMPL_ER86) probe3.ps.dxil when the pack has it: probe2 (the screen grid, the ring) + the WORLD PROBE (taps on the
        // ground in fixed world directions around his feet, stable.rs); its pipeline failing falls back to the file above
        // (RSF2) probe4.ps.dxil first: probe3 + the taps that land on a KH character's own G-buffer (the hybrid's white albedo)
        // left out, so a character's light level is measured on the ground, not on his own or a friend's body
        let skip_kh = dir.join("probe4.ps.dxil").is_file();
        let world_probe = std::fs::read(dir.join(if skip_kh { "probe4.ps.dxil" } else { "probe3.ps.dxil" })).ok();
        let (probe, ring, world) = match world_probe {
            Some(bytes) => (bytes, true, true),
            None => (probe, ring, false),
        };
        rlog!("render: the light probes: {}", if world && skip_kh { "probe4.ps.dxil (RSF2: probe3 without the taps on a KH character's own G-buffer)" } else if world { "probe3.ps.dxil (the world probe of IMPL_ER86 + the ring of IMPL_ER64)" } else if ring { "probe2.ps.dxil (with the ring for the world's lights on Sora, IMPL_ER64)" } else { "probe.ps.dxil" });
        let format = DXGI_FORMAT_R32G32B32A32_FLOAT;
        let desc = texture_desc(crate::light::WIDTH as u32, GAME_ROWS as u32 /* (ER party I) */, 1, 1, format, D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET);
        let texture = committed(&self.device, D3D12_HEAP_TYPE_DEFAULT, &desc, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, None).map_err(|e| text("game-data texture", e))?;
        let mut footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default();
        let mut bytes = 0u64;
        self.device.GetCopyableFootprints(&desc, 0, 1, 0, Some(&mut footprint), None, None, Some(&mut bytes));
        let rtv: ID3D12DescriptorHeap = self.device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV, NumDescriptors: 1, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE, NodeMask: 0 }).map_err(|e| text("game-data rtv heap", e))?;
        self.device.CreateRenderTargetView(&texture, None, rtv.GetCPUDescriptorHandleForHeapStart());
        let root = game_root(&self.device).map_err(|e| text("game root signature", e))?;
        let copy_light = screen_pipeline(&self.device, &root, &vertex, &light, format).map_err(|e| text("copy pipeline", e))?;
        let copy_scene = screen_pipeline(&self.device, &root, &vertex, &scene, format).map_err(|e| text("copy pipeline", e))?;
        let (probe, ring, world) = match screen_pipeline(&self.device, &self.root, &vertex, &probe, format) {
            Ok(p) => (p, ring, world),
            Err(e) if world => {
                rlog!("render: (IMPL_ER86) probe3.ps.dxil cannot be used ({}): the earlier probes", text("probe pipeline", e));
                let ring = crate::pointlight::settings().world && dir.join("probe2.ps.dxil").is_file();
                let bytes = read(if ring { "probe2.ps.dxil" } else { "probe.ps.dxil" })?;
                (screen_pipeline(&self.device, &self.root, &vertex, &bytes, format).map_err(|e| text("probe pipeline", e))?, ring, false)
            }
            Err(e) => return Err(text("probe pipeline", e)),
        };
        rlog!("render: the game's light is read: its constant buffers (cbLight, cbSceneParam) through its own descriptor tables, its targets around his feet");
        Ok(GameKit { texture, rtv, footprint, bytes, root, copy_light, copy_scene, probe, probed: [0; 5], ring, world })
    }

    /// (discovery) Copies `DUMP_ROWS` rows of the constant buffers b4..b13 of each table and
    /// returns them as raw words (per table ten buffers one after the other). Waits for the GPU.
    unsafe fn copy_tables(&mut self, tables: &[crate::cbcap::Table]) -> Result<Vec<u32>, String> {
        let text = |what: &str, e: windows::core::Error| format!("{what}: {e}");
        if self.dump.is_none() {
            let pack = SHARED.lock().ok().and_then(|g| g.as_ref().map(|s| s.pack.clone())).ok_or("no pack")?;
            let dir = pack.join("materials").join("er");
            let vertex = std::fs::read(dir.join("blob.vs.dxil")).map_err(|e| format!("blob.vs.dxil: {e}"))?;
            let format = DXGI_FORMAT_R32G32B32A32_FLOAT;
            let desc = texture_desc(DUMP_ROWS, DUMP_TABLES * DUMP_REGISTERS, 1, 1, format, D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET);
            let texture = committed(&self.device, D3D12_HEAP_TYPE_DEFAULT, &desc, D3D12_RESOURCE_STATE_RENDER_TARGET, None).map_err(|e| text("dump texture", e))?;
            let mut footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default();
            let mut total = 0u64;
            self.device.GetCopyableFootprints(&desc, 0, 1, 0, Some(&mut footprint), None, None, Some(&mut total));
            let readback = committed(&self.device, D3D12_HEAP_TYPE_READBACK, &buffer_desc(total), D3D12_RESOURCE_STATE_COPY_DEST, None).map_err(|e| text("dump readback", e))?;
            let rtv: ID3D12DescriptorHeap = self.device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV, NumDescriptors: 1, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE, NodeMask: 0 }).map_err(|e| text("dump rtv heap", e))?;
            self.device.CreateRenderTargetView(&texture, None, rtv.GetCPUDescriptorHandleForHeapStart());
            let root = game_root(&self.device).map_err(|e| text("game root signature", e))?;
            let mut pipelines = Vec::new();
            for register in 4..4 + DUMP_REGISTERS {
                let pixel = std::fs::read(dir.join(format!("dump{register}.ps.dxil"))).map_err(|e| format!("dump{register}.ps.dxil: {e}"))?;
                pipelines.push(screen_pipeline(&self.device, &root, &vertex, &pixel, format).map_err(|e| text("dump pipeline", e))?);
            }
            let allocator: ID3D12CommandAllocator = self.device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT).map_err(|e| text("dump allocator", e))?;
            let list: ID3D12GraphicsCommandList = self.device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator, None).map_err(|e| text("dump list", e))?;
            list.Close().map_err(|e| text("Close", e))?;
            self.dump = Some(DumpKit { texture, readback, footprint, rtv, root, pipelines, allocator, list });
        }
        let kit = self.dump.as_ref().unwrap();
        let count = tables.len().min(DUMP_TABLES as usize);
        kit.allocator.Reset().map_err(|e| text("allocator Reset", e))?;
        kit.list.Reset(&kit.allocator, None).map_err(|e| text("list Reset", e))?;
        let list = &kit.list;
        let view = kit.rtv.GetCPUDescriptorHandleForHeapStart();
        list.OMSetRenderTargets(1, Some(&view), false, None);
        list.ClearRenderTargetView(view, &[0.0; 4], None);
        list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: DUMP_ROWS as f32, Height: (DUMP_TABLES * DUMP_REGISTERS) as f32, MinDepth: 0.0, MaxDepth: 1.0 }]);
        list.SetGraphicsRootSignature(&kit.root);
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        for (k, table) in tables.iter().take(count).enumerate() {
            let Some(heap) = views_heap(table.heaps) else { continue };
            list.SetDescriptorHeaps(&[Some(heap)]);
            list.SetGraphicsRootDescriptorTable(1, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: table.handle });
            for register in 0..DUMP_REGISTERS as usize {
                let row = (k * DUMP_REGISTERS as usize + register) as i32;
                list.RSSetScissorRects(&[RECT { left: 0, top: row, right: DUMP_ROWS as i32, bottom: row + 1 }]);
                list.SetPipelineState(&kit.pipelines[register]);
                list.DrawInstanced(3, 1, 0, 0);
            }
        }
        list.ResourceBarrier(&[transition(&kit.texture, 0, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_COPY_SOURCE)]);
        let to = D3D12_TEXTURE_COPY_LOCATION { pResource: std::mem::transmute_copy(&kit.readback), Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT, Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { PlacedFootprint: kit.footprint } };
        let from = D3D12_TEXTURE_COPY_LOCATION { pResource: std::mem::transmute_copy(&kit.texture), Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX, Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { SubresourceIndex: 0 } };
        list.CopyTextureRegion(&to, 0, 0, 0, &from, None);
        list.ResourceBarrier(&[transition(&kit.texture, 0, D3D12_RESOURCE_STATE_COPY_SOURCE, D3D12_RESOURCE_STATE_RENDER_TARGET)]);
        list.Close().map_err(|e| text("Close", e))?;
        self.queue.ExecuteCommandLists(&[Some(list.cast::<ID3D12CommandList>().map_err(|e| text("cast", e))?)]);
        self.fence_value += 1;
        self.queue.Signal(&self.fence, self.fence_value).map_err(|e| text("Signal", e))?;
        let waited = Instant::now();
        while self.fence.GetCompletedValue() < self.fence_value {
            if waited.elapsed() > Duration::from_secs(5) || self.device.GetDeviceRemovedReason().is_err() {
                return Err("the GPU did not finish the copy".to_string());
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        let mut at: *mut c_void = std::ptr::null_mut();
        kit.readback.Map(0, None, Some(&mut at)).map_err(|e| text("Map", e))?;
        let rows = count * DUMP_REGISTERS as usize;
        let mut words = Vec::with_capacity(rows * DUMP_ROWS as usize * 4);
        for row in 0..rows {
            let line = (at as *const u8).add(kit.footprint.Offset as usize + row * kit.footprint.Footprint.RowPitch as usize) as *const u32;
            words.extend_from_slice(std::slice::from_raw_parts(line, DUMP_ROWS as usize * 4));
        }
        kit.readback.Unmap(0, Some(&D3D12_RANGE { Begin: 0, End: 0 }));
        Ok(words)
    }
}

/// (discovery) One submission of a capture frame: its text and the tables to copy constant
/// buffers from, appended to `<dump dir>/cbcap_<serial>.txt`.
pub fn write_capture(queue: usize, mut lines: Vec<String>, tables: Vec<crate::cbcap::Table>) {
    let Some(dir) = crate::DUMP_DIR.lock().ok().and_then(|d| d.clone()) else { return };
    {
        let mut guard = CORE.lock().unwrap();
        match guard.as_mut() {
            Some(core) if core.queue.as_raw() as usize == queue && !core.disabled && !tables.is_empty() => {
                // SAFETY: our own objects on the game's device; the tables were bound by lists the
                // game submitted a moment ago on this queue: heap and descriptors are alive.
                match unsafe { core.copy_tables(&tables) } {
                    Ok(words) => {
                        let per = DUMP_ROWS as usize * 4;
                        for (k, table) in tables.iter().enumerate() {
                            for register in 0..DUMP_REGISTERS as usize {
                                let n = k * DUMP_REGISTERS as usize + register;
                                if let Some(rows) = words.get(n * per..(n + 1) * per) {
                                    if rows.iter().any(|w| *w != 0) {
                                        lines.extend(crate::cbcap::rows_text(&format!("table {:#x} b{}", table.handle, register + 4), rows));
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => lines.push(format!("(constant buffers not copied: {e})")),
                }
            }
            Some(_) if !tables.is_empty() => lines.push("(another queue: constant buffers not copied)".to_string()),
            _ => {}
        }
    }
    let path = dir.join(format!("cbcap_{}.txt", crate::cbcap::serial()));
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        use std::io::Write;
        let _ = file.write_all((lines.join("\n") + "\n").as_bytes());
    }
}

pub fn release() {
    crate::vfx::release(); // (vfx)
    crate::finale::release(); // (ZFIN)
    crate::zdraw::release(); // (ZDRAW)
    crate::pointlight::release(); // (IMPL_ER39)
    crate::hud::release(); // (hud)
    let core = CORE.lock().unwrap().take();
    drop_core(core);
    survey::release();
    crate::cbcap::release();
}

pub fn status() -> String {
    let mut guard = CORE.lock().unwrap();
    match guard.as_mut() {
        Some(core) => {
            let s = std::mem::take(&mut core.stats);
            let state = if core.disabled {
                "OFF (failed)"
            } else if core.assets_failed {
                "OFF (no assets)"
            } else if core.assets.is_none() {
                "loading the pack"
            } else {
                "on"
            };
            let round = |v: Vec3| v.to_array().map(|x| (x * 1.0e4).round() / 1.0e4);
            let light = match (core.light.as_ref(), core.rig_light.as_ref()) {
                (Some(l), Some(r)) => format!(
                    "\n  light: sun {} sky {:?} | at his feet: scene {:?} direct {:?} surface {:?} mask {:?} probed {} | scene {} exposure {:.3} pre {:.2} jitter ({:.3}, {:.3}) px | he gets (white surface): key {:?} ambient {:?} + up {:?} (rig x{:.1}; ground / formula this frame {:.3}) | top of the picture {:?} ({} taps): reflection environment x{:.2}",
                    l.sun.map_or("none".to_string(), |(d, c)| format!("dir ({:.3}, {:.3}, {:.3}) colour ({:.4}, {:.4}, {:.4})", d.x, d.y, d.z, c.x, c.y, c.z)),
                    round(l.sky.1), round(l.scene_px), round(l.direct_px), round(l.albedo_px), l.mask.to_array().map(|v| (v * 100.0).round() / 100.0), l.probed(), l.scene, l.exposure, l.pre_exposure, l.jitter.0, l.jitter.1,
                    round(r.key / std::f32::consts::PI * r.out_scale), round(r.ambient * r.out_scale), round(r.ambient_up * r.out_scale), 1.0 / r.out_scale, core.calibration, round(l.env_px), l.env_taps, core.env_now
                ) + &format!(" | fog ambient {:?} toward the light {:?} opposite {:?}", round(l.fog[0]), round(l.fog[1]), round(l.fog[2])),
                _ => String::new(),
            };
            let light = format!("{light}\n  velocity: {} frame(s) with his motion vectors written | late pass (after the game's forward stage): {} frame(s) with late effects / darkening drawn, the scene darkened in {} | real shadow: {} cascade pass(es) drawn into the game's atlas, {} us of CPU in all{} | (IMPL_ER65) late re-composite: {} frame(s) | (IMPL_ER53) early depth: {} frame(s) with his depth in the scene depth right after the G-buffer ({} draws in the latest; (IMPL_ER56) stencil {}, ground taps {:?})", std::mem::take(&mut core.velocity_drawn), std::mem::take(&mut core.late_drawn), crate::vfx::DIMMED_FRAMES.swap(0, Ordering::Relaxed), std::mem::take(&mut core.cast_drawn), std::mem::take(&mut core.cast_micros), if core.cast_failed { " (OFF: failed)" } else { "" }, std::mem::take(&mut core.recomp_drawn), std::mem::take(&mut core.early_drawn), EARLY_DRAWS.load(Ordering::Relaxed), match STENCIL_USED.load(Ordering::Relaxed) { 0 => "not written".to_string(), v => format!("{:#04x} written (ER88: TAA class {}{})", v - 1, taa_class((v - 1) as u8), if v - 1 == CHAR_STENCIL as u32 { ", the characters' value" } else { "" }) }, core.stencil_seen);
            // (IMPL_ER80) the glow mask: frames masked / skipped this second and why
            let glow_line = if !GLOW_MASK.load(Ordering::Relaxed) {
                "OFF (r.glow_mask 0: the glow of what lies behind him is blurred over him, as before)".to_string()
            } else {
                let (d, k) = (std::mem::take(&mut core.glow.drawn), std::mem::take(&mut core.glow.skipped));
                format!("{d} frame(s) with his pixels masked out of the game's glow source, {k} on today's path (latest reason: {})", if k == 0 { "none this second" } else { core.glow.reason })
            };
            let light = format!("{light}\n  (IMPL_ER80) glow mask: {glow_line}");
            let light = format!("{light}\n  (IMPL_ER80b) light shafts: his mask made in {} frame(s); {}", std::mem::take(&mut core.glow.shaft_frames), crate::shaft::status());
            let light = format!("{light}\n  {}", crate::hybrid::status());
            // (ER87) the hair path's line (only when the drawn outfit takes it)
            let light = match crate::hair::status() {
                Some(h) => format!("{light}\n  {h}"),
                None => light,
            };
            // (ER party S4) the party's line (only once a member was asked for)
            #[cfg(feature = "party")]
            let light = if crate::party::wanted().is_empty() { light } else { format!("{light}\n  (ER party S4) party: {}", party::status(&mut core.party)) };
            format!(
                "render {state}: {} presents, {} drawn ({} draws/frame, {} with the game's depth), {} waiting for pipelines, {} skipped busy, {} without a scene, {} skipped (unknown frame shape); our CPU time {:.3} ms/frame (max {:.3}), GPU time {:.3} ms/frame (max {:.3}); the host's frame was {:.1} ms old when drawn (least {:.1}, most {:.1}; same thread {} times){light}",
                s.presents,
                s.drawn,
                if s.drawn > 0 { s.draws / s.drawn } else { 0 },
                s.with_scene_depth,
                s.waiting,
                s.busy,
                s.no_scene,
                s.skipped,
                if s.drawn > 0 { s.micros as f64 / s.drawn as f64 / 1000.0 } else { 0.0 },
                s.micros_max as f64 / 1000.0,
                if s.gpu_frames > 0 { s.gpu as f64 / s.gpu_frames as f64 / 1000.0 } else { 0.0 },
                s.gpu_max as f64 / 1000.0,
                if s.drawn + s.waiting + s.busy > 0 { s.age as f64 / (s.drawn + s.waiting + s.busy) as f64 / 1000.0 } else { 0.0 },
                s.age_min as f64 / 1000.0,
                s.age_max as f64 / 1000.0,
                s.same_thread
            )
        }
        None => format!("render: no swap chain yet ({})", WAITING.lock().map(|w| w.clone()).unwrap_or_default()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (FP) the routed fragments' reserve: the later skins (now and a frame ago) and material blocks, 256-aligned, plus the
    /// margin; FINAL's worst frame (Sora + friends + 160 fragments, about 1.2 MB) still leaves the fragments room in 2 MB.
    #[test]
    fn fp_routed_fragments_leave_room_for_what_the_frame_still_needs() {
        assert_eq!(routed_reserve(0, 0), 64 * 1024);
        let skin = (16 * (1 + MAX_JOINTS_OUTFIT) * 4 + 255) & !255;
        let block = (kh_look::PARAM_ROWS * 16 + 255) & !255;
        assert_eq!(routed_reserve(3, 50), 2 * 3 * skin + 50 * block + 64 * 1024);
        assert!(routed_reserve(3, 120) < UPLOAD_BYTES / 2);
        // the test cap only lowers the buffer
        set_upload_test_cap(4 * UPLOAD_BYTES);
        assert_eq!(upload_cap(), UPLOAD_BYTES);
        set_upload_test_cap(512 * 1024);
        assert_eq!(upload_cap(), 512 * 1024);
        set_upload_test_cap(0);
        assert_eq!(upload_cap(), UPLOAD_BYTES);
    }

    /// (IMPL_ER53) the early-depth states sit after the slots', the contact shadow's and the velocity's, one per slot;
    /// a set without them (or with a part missing) has none.
    #[test]
    fn the_early_depth_states_are_found_only_in_a_full_set() {
        assert_eq!(early_base(2 * 12 + 2, 12), Some(14));
        assert_eq!(early_base(12 + 2, 12), None);
        assert_eq!(early_base(12 + 1, 12), None);
        assert_eq!(early_base(2, 0), None);
        // (IMPL_ER56) with the stencil-writing set after them
        assert_eq!((early_base(3 * 12 + 2, 12), early_stencil_base(3 * 12 + 2, 12), early_stencil_base(2 * 12 + 2, 12)), (Some(14), Some(26), None));
        // (IMPL_ER78) with the G-buffer writers after those: all three parts found; without them no G-buffer part
        assert_eq!((early_base(4 * 12 + 2, 12), early_stencil_base(4 * 12 + 2, 12), gbuffer_base(4 * 12 + 2, 12)), (Some(14), Some(26), Some(38)));
        assert_eq!((gbuffer_base(3 * 12 + 2, 12), gbuffer_base(2 * 12 + 2, 12), gbuffer_base(2, 0)), (None, None, None));
    }

    /// (IMPL_ER78) the slot table has room for the hybrid's two views after an outfit's extra textures
    #[test]
    fn the_hybrid_views_follow_the_outfit_textures() {
        assert!(texture_descriptor(kh_look::library::SLOT_TEXTURES_MAX - 1) < HY_LIGHT_TEXTURE);
        assert!(HY_LIGHT_TEXTURE != HY_MASK_TEXTURE && HY_MASK_TEXTURE < SLOT_TEXTURES);
        assert!(![TABLE_TEXTURE, CUBE_TEXTURE, DEPTH_TEXTURE].contains(&HY_LIGHT_TEXTURE) && ![TABLE_TEXTURE, CUBE_TEXTURE, DEPTH_TEXTURE].contains(&HY_MASK_TEXTURE));
    }

    /// (IMPL_ER65) the late re-composite is on by default, for what lies behind him farther than 100 m (or the sky)
    #[test]
    fn the_late_recomposite_defaults() {
        assert!(!LATE_SORA.load(Ordering::Relaxed), "(IMPL_ER74) off by default");
        assert_eq!(f32::from_bits(LATE_FAR.load(Ordering::Relaxed)), 100.0);
    }

    /// (IMPL_ER80) the glow mask is on by default, the re-composite (when switched on) goes before the TAA; the per-frame
    /// check lets only a float target other than the scene, seen this frame, resting in a read or target state through
    #[test]
    fn er88_his_stencil_is_the_characters_value() {
        // the game's characters: 0x50 = TAA class 1; the grounds the user's logs show: class 0 (0x12, 0x10), 3 (0xd2), 2 (0x90)
        assert_eq!((taa_class(CHAR_STENCIL), taa_class(0x12), taa_class(0xd2), taa_class(0x90), taa_class(0x10)), (1, 0, 3, 2, 0));
        // r.taa_char on: the characters' value whatever the ground is, from the first frame (no ground known yet)
        assert_eq!(his_stencil(-1, true, Some(0xd2)), Some(0x50));
        assert_eq!(his_stencil(-1, true, None), Some(0x50));
        // r.taa_char 0: the ground's (IMPL_ER56); r.stencil_ref wins over both
        assert_eq!(his_stencil(-1, false, Some(0x12)), Some(0x12));
        assert_eq!(his_stencil(-1, false, None), None);
        assert_eq!(his_stencil(0x12, true, Some(0xd2)), Some(0x12));
        assert!(TAA_CHAR.load(Ordering::Relaxed), "r.taa_char defaults on");
    }

    #[test]
    fn the_glow_mask_defaults_and_its_frame_check() {
        assert!(GLOW_MASK.load(Ordering::Relaxed));
        assert!(LATE_TAA.load(Ordering::Relaxed));
        assert!(!recomp_split_wanted(), "the re-composite stays off by default");
        let srv = (D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE | D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE).0;
        let r11 = DXGI_FORMAT_R11G11B10_FLOAT.0;
        assert_eq!(glow_check(0x2000, srv, r11, 0x1000, true), None);
        assert_eq!(glow_check(0x2000, D3D12_RESOURCE_STATE_RENDER_TARGET.0, DXGI_FORMAT_R16G16B16A16_FLOAT.0, 0x1000, true), None);
        assert!(glow_check(0x1000, srv, r11, 0x1000, true).is_some(), "the scene target itself");
        assert!(glow_check(0, srv, r11, 0x1000, true).is_some(), "none");
        assert!(glow_check(0x2000, srv, r11, 0x1000, false).is_some(), "not seen this frame");
        assert!(glow_check(0x2000, srv, DXGI_FORMAT_R8G8B8A8_UNORM.0, 0x1000, true).is_some(), "not a float HDR target");
        assert!(glow_check(0x2000, 0, r11, 0x1000, true).is_some(), "COMMON");
        assert!(glow_check(0x2000, D3D12_RESOURCE_STATE_COPY_DEST.0, r11, 0x1000, true).is_some(), "copy destination");
    }

    #[test]
    fn his_stencil_is_the_grounds_most_frequent_non_zero_value() {
        assert_eq!(ground_stencil(&[0, 0, 0, 0, 0]), None);
        assert_eq!(ground_stencil(&[0, 0x21, 0x21, 0x05, 0]), Some(0x21));
        assert_eq!(ground_stencil(&[0x05]), Some(0x05));
    }
}
