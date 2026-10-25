//! What the game does with its render targets and depth targets, learnt from its own Direct3D
//! calls (survey level H2): four entries of the command-list vtable are hooked
//! (`ResourceBarrier`, `OMSetRenderTargets`, `ClearDepthStencilView`, `Reset`), each list's calls
//! are kept until the list is executed, and `ExecuteCommandLists` (hook.rs) replays them in
//! submission order. From that:
//!
//! - ALWAYS: the current state of every resource that is ever a depth target (so that we can put
//!   a barrier of our own on it and put it back), and which of them is the main scene depth.
//! - ON REQUEST (`r.survey = N`): a log of N frames: every submission, per list the targets set,
//!   depth clears, and the transitions of render targets and depth targets with size and format.
//!
//! A resource is identified by pointer; its description is read while the game itself is using
//! it in a call (it is alive then). The one resource we keep for drawing is AddRef'd.

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::Mutex;

use windows::core::{Interface, HRESULT};
use windows::Win32::Graphics::Direct3D::WKPDID_D3DDebugObjectNameW;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;

use crate::hook::{entry, is_ours, patch, LIST_VTABLE};
use crate::{rlog, DEAD};

#[derive(Clone, Copy, Debug)]
enum Event {
    Barrier { resource: usize, subresource: u32, before: i32, after: i32, flags: i32 },
    Targets { count: u32, first: usize, depth: usize },
    ClearDepth { view: usize, flags: i32, depth: f32 },
    /// A full-screen draw (3 vertices) with this pixel-stage constant buffer table (cbcap.rs, the watch).
    Fullscreen(crate::cbcap::Table),
    /// (IMPL_ER17) A depth target left DEPTH_WRITE: the caster pass that drew into it ended; what the list bound then.
    Cast { resource: usize, after: i32, pass: SunPass },
}

/// (IMPL_ER17) One caster pass of the game into a shadow atlas: the vertex-stage constant buffer table of its draws
/// (cbSceneParam of the pass at b8: the light's view-projection), its viewport (x, y, width, height) and scissor.
#[derive(Clone, Copy, Debug, Default)]
pub struct SunPass {
    pub table: crate::cbcap::Table,
    pub viewport: [f32; 4],
    pub scissor: [i32; 4],
}

pub const SUN_PASSES: usize = 12;

/// (IMPL_ER17) THE SUN'S CASCADE ATLAS as the game filled it in a submission (MEASURED: a 16-bit depth atlas, every
/// cascade pass a viewport of it under ONE depth view, each pass wrapped SRV -> DEPTH_WRITE -> SRV; the G-buffer follows).
#[derive(Clone, Copy, Debug, Default)]
pub struct SunAtlas {
    pub resource: usize,
    /// Its state after the last of those passes (a shader-resource state).
    pub state: i32,
    /// The depth view the game set for the passes (a CPU descriptor handle of its own heap).
    pub view: usize,
    /// How many lists of the submission run up to and including the last pass.
    pub split: usize,
    pub passes: [SunPass; SUN_PASSES],
    pub count: usize,
}

/// What is known about a resource the game transitioned.
#[derive(Clone, Debug)]
pub struct Info {
    pub width: u64,
    pub height: u32,
    pub format: DXGI_FORMAT,
    pub flags: D3D12_RESOURCE_FLAGS,
    pub mips: u16,
    pub layers: u16,
    pub samples: u32,
    pub name: String,
    /// State per subresource as of the last executed list (as far as the game's barriers told).
    pub states: Vec<i32>,
    /// The frame it was last transitioned in.
    pub seen: u64,
    /// Times it was transitioned to DEPTH_WRITE.
    pub depth_writes: u64,
    /// The frame in which a barrier of the game last named a BEFORE state that is not the state we
    /// had tracked (u64::MAX = never): our record of it was wrong then, so nothing of ours may put
    /// a barrier on it in that frame (hardening, IMPL_ER4_STATE.md step 0).
    pub mismatch: u64,
    /// The frame its description was last read from the live object (a resource MADE AGAIN AT THE
    /// SAME ADDRESS, after a resolution change, must not keep the dead one's size and format: measured,
    /// IMPL_ER4_STATE.md run 3: nothing was drawn any more after a change of the render resolution).
    pub checked: u64,
}

impl Info {
    pub fn is_depth(&self) -> bool {
        (self.flags & D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL).0 != 0
    }
}

#[derive(Default)]
struct State {
    /// list pointer -> the calls recorded on it since its last Reset.
    lists: HashMap<usize, Vec<Event>>,
    resources: HashMap<usize, Info>,
    frame: u64,
    /// Lines of the survey of the current frame (only while a survey runs).
    lines: Vec<String>,
    submissions: u32,
    /// The scene depth we hold a reference to: (pointer, the owned interface).
    held: Option<(usize, ID3D12Resource)>,
    /// The swap chain's buffers (pointers, for comparing only).
    back_buffers: Vec<usize>,
}

// SAFETY: the COM pointer inside is only used under the mutex; D3D12 objects are free-threaded.
unsafe impl Send for State {}

static STATE: Mutex<Option<State>> = Mutex::new(None);
/// Frames still to log.
static SURVEY: AtomicU32 = AtomicU32::new(0);
/// Resources found made again at a known address (for the log).
pub static RECREATED: AtomicU32 = AtomicU32::new(0);

static ORIGINAL_BARRIER: AtomicUsize = AtomicUsize::new(0);
static ORIGINAL_TARGETS: AtomicUsize = AtomicUsize::new(0);
static ORIGINAL_CLEAR_DEPTH: AtomicUsize = AtomicUsize::new(0);
static ORIGINAL_RESET: AtomicUsize = AtomicUsize::new(0);

type BarrierFn = unsafe extern "system" fn(*mut c_void, u32, *const D3D12_RESOURCE_BARRIER);
type TargetsFn = unsafe extern "system" fn(*mut c_void, u32, *const D3D12_CPU_DESCRIPTOR_HANDLE, i32, *const D3D12_CPU_DESCRIPTOR_HANDLE);
type ClearDepthFn = unsafe extern "system" fn(*mut c_void, D3D12_CPU_DESCRIPTOR_HANDLE, i32, f32, u8, u32, *const c_void);
type ResetFn = unsafe extern "system" fn(*mut c_void, *mut c_void, *mut c_void) -> HRESULT;

/// Hooks the command-list entries. After hook::install.
pub fn install() -> Result<(), String> {
    let vtable = LIST_VTABLE.load(Ordering::SeqCst);
    if vtable == 0 {
        return Err("no command-list vtable".to_string());
    }
    *STATE.lock().unwrap() = Some(State::default());
    // SAFETY: the hook functions have the entries' signatures.
    unsafe {
        patch(vtable, entry!(ID3D12GraphicsCommandList_Vtbl, Reset), reset_hook as *const () as usize, &ORIGINAL_RESET, "ID3D12GraphicsCommandList::Reset")?;
        patch(vtable, entry!(ID3D12GraphicsCommandList_Vtbl, ResourceBarrier), barrier_hook as *const () as usize, &ORIGINAL_BARRIER, "ID3D12GraphicsCommandList::ResourceBarrier")?;
        patch(vtable, entry!(ID3D12GraphicsCommandList_Vtbl, OMSetRenderTargets), targets_hook as *const () as usize, &ORIGINAL_TARGETS, "ID3D12GraphicsCommandList::OMSetRenderTargets")?;
        patch(vtable, entry!(ID3D12GraphicsCommandList_Vtbl, ClearDepthStencilView), clear_depth_hook as *const () as usize, &ORIGINAL_CLEAR_DEPTH, "ID3D12GraphicsCommandList::ClearDepthStencilView")?;
    }
    Ok(())
}

/// Drops what is held (at shutdown, after the hooks are gone).
pub fn release() {
    if let Ok(mut guard) = STATE.lock() {
        *guard = None;
    }
}

/// The swap chain's buffers as of the latest Present.
pub fn set_back_buffers(buffers: Vec<usize>) {
    if let Ok(mut guard) = STATE.lock() {
        if let Some(state) = guard.as_mut() {
            state.back_buffers = buffers;
        }
    }
}

/// Asks for a log of the next `frames` frames.
pub fn request(frames: u32) {
    SURVEY.store(frames, Ordering::SeqCst);
}

fn record(list: usize, event: Event) {
    if let Ok(mut guard) = STATE.lock() {
        if let Some(state) = guard.as_mut() {
            let events = state.lists.entry(list).or_default();
            // (a list that is never executed must not grow without bound)
            if events.len() < 4096 {
                events.push(event);
            }
        }
    }
}

/// From cbcap.rs's draw hook: a full-screen draw on `list`.
pub fn fullscreen(list: usize, table: crate::cbcap::Table) {
    record(list, Event::Fullscreen(table));
}

fn name_of(resource: &ID3D12Resource) -> String {
    let mut buffer = [0u16; 128];
    let mut size = (buffer.len() * 2) as u32;
    // SAFETY: the buffer is `size` bytes.
    let got = unsafe { resource.GetPrivateData(&WKPDID_D3DDebugObjectNameW, &mut size, Some(buffer.as_mut_ptr() as *mut c_void)) };
    if got.is_err() {
        return String::new();
    }
    String::from_utf16_lossy(&buffer[..(size as usize / 2).min(buffer.len())]).trim_end_matches('\0').to_string()
}

unsafe extern "system" fn barrier_hook(this: *mut c_void, count: u32, barriers: *const D3D12_RESOURCE_BARRIER) {
    if !DEAD.load(Ordering::Relaxed) && !is_ours() && !barriers.is_null() {
        let _ = std::panic::catch_unwind(|| {
            for k in 0..count as usize {
                let b = &*barriers.add(k);
                if b.Type != D3D12_RESOURCE_BARRIER_TYPE_TRANSITION {
                    continue;
                }
                let t = &b.Anonymous.Transition;
                let pointer: usize = std::mem::transmute_copy(&t.pResource);
                if pointer == 0 {
                    continue;
                }
                if b.Flags.0 & D3D12_RESOURCE_BARRIER_FLAG_BEGIN_ONLY.0 == 0 {
                    crate::cbcap::on_barrier(this as usize, pointer, t.Subresource, t.StateAfter.0);
                }
                let interesting = D3D12_RESOURCE_STATE_DEPTH_WRITE.0 | D3D12_RESOURCE_STATE_DEPTH_READ.0 | D3D12_RESOURCE_STATE_RENDER_TARGET.0;
                // (known, and whether its description is due for a look: every 30 frames, and at once
                // when the game's barrier contradicts what we have on record)
                let (known, recheck) = STATE.lock().ok().and_then(|g| g.as_ref().map(|s| match s.resources.get(&pointer) {
                    Some(i) => (true, s.frame.saturating_sub(i.checked) >= 30 || i.states.get(if t.Subresource == D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES { 0 } else { t.Subresource as usize }).is_some_and(|st| *st != t.StateBefore.0)),
                    None => (false, false),
                })).unwrap_or((false, false));
                if !known && (t.StateBefore.0 | t.StateAfter.0) & interesting == 0 {
                    continue;
                }
                if known && recheck {
                    // The game is using the resource in this very call: it is alive.
                    let raw = pointer as *mut c_void;
                    if let Some(resource) = ID3D12Resource::from_raw_borrowed(&raw) {
                        let d = resource.GetDesc();
                        if let Ok(mut guard) = STATE.lock() {
                            if let Some(state) = guard.as_mut() {
                                let frame = state.frame;
                                let same = state.resources.get(&pointer).is_some_and(|i| i.width == d.Width && i.height == d.Height && i.format == d.Format && i.flags == d.Flags && i.mips == d.MipLevels && i.layers == d.DepthOrArraySize);
                                if same {
                                    if let Some(i) = state.resources.get_mut(&pointer) {
                                        i.checked = frame;
                                    }
                                } else {
                                    // Another resource lives at this address now: forget the old one (it is
                                    // read afresh just below, as a resource never seen).
                                    state.resources.remove(&pointer);
                                    if state.held.as_ref().is_some_and(|(p, _)| *p == pointer) {
                                        state.held = None;
                                    }
                                    RECREATED.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                        }
                    }
                }
                let known = known && STATE.lock().ok().and_then(|g| g.as_ref().map(|s| s.resources.contains_key(&pointer))).unwrap_or(false);
                if !known {
                    // The game is using the resource in this very call: it is alive.
                    let raw = pointer as *mut c_void;
                    if let Some(resource) = ID3D12Resource::from_raw_borrowed(&raw) {
                        let d = resource.GetDesc();
                        if d.Dimension == D3D12_RESOURCE_DIMENSION_TEXTURE2D {
                            let planes = if matches!(d.Format, DXGI_FORMAT_R32G8X24_TYPELESS | DXGI_FORMAT_D32_FLOAT_S8X24_UINT | DXGI_FORMAT_R24G8_TYPELESS | DXGI_FORMAT_D24_UNORM_S8_UINT) { 2 } else { 1 };
                            let subresources = (d.MipLevels as usize * d.DepthOrArraySize as usize * planes).clamp(1, 64);
                            let info = Info { width: d.Width, height: d.Height, format: d.Format, flags: d.Flags, mips: d.MipLevels, layers: d.DepthOrArraySize, samples: d.SampleDesc.Count, name: name_of(resource), states: vec![t.StateBefore.0; subresources], seen: 0, depth_writes: 0, mismatch: u64::MAX, checked: 0 };
                            if let Ok(mut guard) = STATE.lock() {
                                if let Some(state) = guard.as_mut() {
                                    let mut info = info;
                                    info.checked = state.frame;
                                    state.resources.insert(pointer, info);
                                }
                            }
                        }
                    }
                }
                record(this as usize, Event::Barrier { resource: pointer, subresource: t.Subresource, before: t.StateBefore.0, after: t.StateAfter.0, flags: b.Flags.0 });
                // (IMPL_ER17) a caster pass ended: what the list bound for it
                if t.StateBefore.0 == D3D12_RESOURCE_STATE_DEPTH_WRITE.0 && t.StateAfter.0 & D3D12_RESOURCE_STATE_DEPTH_WRITE.0 == 0 && b.Flags.0 & D3D12_RESOURCE_BARRIER_FLAG_BEGIN_ONLY.0 == 0 && crate::cbcap::casting() {
                    if let Some((table, viewport, scissor)) = crate::cbcap::cast_now(this as usize) {
                        record(this as usize, Event::Cast { resource: pointer, after: t.StateAfter.0, pass: SunPass { table, viewport, scissor } });
                    }
                }
            }
        });
    }
    let original: BarrierFn = std::mem::transmute(ORIGINAL_BARRIER.load(Ordering::Relaxed));
    original(this, count, barriers)
}

unsafe extern "system" fn targets_hook(this: *mut c_void, count: u32, targets: *const D3D12_CPU_DESCRIPTOR_HANDLE, single: i32, depth: *const D3D12_CPU_DESCRIPTOR_HANDLE) {
    // (IMPL_ER80b) the light-shaft mask puts the game's targets back after its masked draw: it follows them
    if !DEAD.load(Ordering::Relaxed) && !is_ours() {
        let _ = std::panic::catch_unwind(|| crate::shaft::on_targets(this as usize, count, targets, single, depth));
    }
    // (IMPL_ER17: depth-only target sets are also kept while the real shadow wants the caster passes' depth view)
    if !DEAD.load(Ordering::Relaxed) && !is_ours() && (SURVEY.load(Ordering::Relaxed) > 0 || (count == 0 && !depth.is_null() && crate::cbcap::casting())) {
        let first = if count > 0 && !targets.is_null() { (*targets).ptr } else { 0 };
        let depth = if depth.is_null() { 0 } else { (*depth).ptr };
        record(this as usize, Event::Targets { count, first, depth });
    }
    let original: TargetsFn = std::mem::transmute(ORIGINAL_TARGETS.load(Ordering::Relaxed));
    original(this, count, targets, single, depth)
}

unsafe extern "system" fn clear_depth_hook(this: *mut c_void, view: D3D12_CPU_DESCRIPTOR_HANDLE, flags: i32, depth: f32, stencil: u8, count: u32, rects: *const c_void) {
    if !DEAD.load(Ordering::Relaxed) && !is_ours() && SURVEY.load(Ordering::Relaxed) > 0 {
        record(this as usize, Event::ClearDepth { view: view.ptr, flags, depth });
    }
    let original: ClearDepthFn = std::mem::transmute(ORIGINAL_CLEAR_DEPTH.load(Ordering::Relaxed));
    original(this, view, flags, depth, stencil, count, rects)
}

unsafe extern "system" fn reset_hook(this: *mut c_void, allocator: *mut c_void, pipeline: *mut c_void) -> HRESULT {
    if !DEAD.load(Ordering::Relaxed) && !is_ours() {
        crate::cbcap::on_reset(this as usize);
        crate::shaft::on_reset(this as usize, pipeline as usize);
        if let Ok(mut guard) = STATE.lock() {
            if let Some(state) = guard.as_mut() {
                if let Some(events) = state.lists.get_mut(&(this as usize)) {
                    events.clear();
                }
            }
        }
    }
    let original: ResetFn = std::mem::transmute(ORIGINAL_RESET.load(Ordering::Relaxed));
    original(this, allocator, pipeline)
}

pub fn state_text(state: i32) -> String {
    if state == 0 {
        return "COMMON".to_string();
    }
    let names = [
        (0x1, "VB/CB"), (0x2, "IB"), (0x4, "RENDER_TARGET"), (0x8, "UAV"), (0x10, "DEPTH_WRITE"), (0x20, "DEPTH_READ"), (0x40, "NON_PIXEL_SRV"), (0x80, "PIXEL_SRV"),
        (0x100, "STREAM_OUT"), (0x200, "INDIRECT"), (0x400, "COPY_DEST"), (0x800, "COPY_SOURCE"), (0x1000, "RESOLVE_DEST"), (0x2000, "RESOLVE_SOURCE"),
    ];
    let mut parts: Vec<String> = names.iter().filter(|(bit, _)| state & bit != 0).map(|(_, n)| n.to_string()).collect();
    let rest = state & !0x3FFF;
    if rest != 0 {
        parts.push(format!("{rest:#x}"));
    }
    parts.join("|")
}

fn describe(pointer: usize, info: Option<&Info>) -> String {
    match info {
        Some(i) => format!("{pointer:#x} {}x{} fmt {} mips {} layers {} samples {}{}{}", i.width, i.height, i.format.0, i.mips, i.layers, i.samples, if i.is_depth() { " DEPTH" } else { "" }, if i.name.is_empty() { String::new() } else { format!(" \"{}\"", i.name) }),
        None => format!("{pointer:#x}"),
    }
}

/// A resource as text (size, format, name) for the capture file.
pub fn describe_resource(pointer: usize) -> String {
    match STATE.lock().ok().and_then(|g| g.as_ref().map(|s| describe(pointer, s.resources.get(&pointer)))) {
        Some(text) => text,
        None => format!("{pointer:#x}"),
    }
}

/// Called by the queue hook: the game submits `count` lists.
///
/// # Safety
/// `lists` must be the argument of an `ExecuteCommandLists` call.
///
/// What a submission of the game was, as far as our drawing cares.
#[derive(Clone, Copy, Debug, Default)]
pub struct Submission {
    /// The back buffer it finished (took from RENDER_TARGET to PRESENT): the game's picture is
    /// complete in it once the submission has run.
    pub finished: Option<usize>,
    /// It was the deferred stage of the scene (it drew a G-buffer of four or more targets with
    /// the scene depth); this is the HDR scene target its last passes drew into with that depth
    /// (the deferred composite), and that target's state now.
    pub scene: Option<(usize, i32)>,
    /// The diffuse light accumulation target of that stage (the first of the two HDR targets its
    /// light passes drew into with the depth read-only), when it rests readable by pixel shaders.
    /// MEASURED (IMPL_ER3_STATE.md): the first such pair is the environment / GI accumulation.
    pub light: Option<usize>,
    /// The first target of the LAST such pair: the diffuse light accumulation (which, by the time
    /// our list runs, holds all diffuse light times the surface colour: light.rs).
    pub light2: Option<usize>,
    /// The screen-space shadow mask (a full-size R8G8B8A8 target drawn full screen in this submission).
    pub mask: Option<usize>,
    /// The G-buffer target that holds the surface colour (the one R8G8B8A8 typeless target of the five).
    pub albedo: Option<usize>,
    /// The pixel-stage constant buffer tables of the full-screen draws of those light passes, in
    /// order: one of them is the directional light's (`cbLight` at b5), all carry `cbSceneParam` (b8).
    pub tables: [crate::cbcap::Table; 6],
    pub table_count: usize,
    /// THE VELOCITY PASS is in this submission: the game's velocity target (full-size R16G16
    /// float, first filled by a full-screen draw) and its state after the list that draws it;
    /// `split` = how many lists of the submission run up to and including that list (our own
    /// velocity draw goes in between there); the scene depth's two planes' states at that point.
    pub velocity: Option<(usize, i32)>,
    pub split: usize,
    pub depth_states: [i32; 2],
    /// HARDENING: the scene depth was transitioned by the game in THIS submission (its state is
    /// observed, not remembered), and none of the resources our lists put barriers on (scene
    /// target, scene depth, velocity target) had a barrier whose BEFORE state contradicted our
    /// record in this frame. Without both, nothing of ours is drawn this frame.
    pub depth_seen: bool,
    pub clean: bool,
    /// (IMPL_ER17) The sun's cascade atlas was filled in this submission.
    pub sun: Option<SunAtlas>,
    /// (IMPL_ER53) THE EARLY DEPTH: how many lists of this submission run up to and including the LAST list that drew
    /// into the G-buffer (four or more full-size targets with the scene depth written); 0 = none. His depth goes into
    /// the scene depth there, before the depth copies, downsamples and screen-space passes made later in the
    /// submission (and read by the game's forward stage) are made. `early_states`: the scene depth's two planes'
    /// states at that point (the end of that list).
    pub early_split: usize,
    /// (RSF2 round 14) THE EFFECTS' HALF-SIZE DEPTH: the first list of this submission that wrote a half-size depth of the scene
    /// depth's format (the game's forward stage rebuilds it there, before its half-size effects: fire, embers, glows); the index
    /// after it, the resource, its two planes' states and the scene depth's planes at that point. 0 = none.
    pub effects_split: usize,
    pub effects_half: Option<(usize, [i32; 2], usize, [i32; 2])>,
    pub early_states: [i32; 2],
    /// (IMPL_ER78) THE G-BUFFER of that last G-buffer list: its targets (the ones in RENDER_TARGET when the scene depth went
    /// to DEPTH_WRITE with four or more full-size targets), each with its format, size and state at the end of that list;
    /// `gbuffer_count` = how many there were (the hybrid takes exactly five: hybrid.rs `order`).
    pub gbuffer: [crate::hybrid::GbTarget; crate::hybrid::ROLES],
    pub gbuffer_count: usize,
    /// (IMPL_ER80) THE GLOW SOURCE: the second HDR target of the deferred composite (GXLightComposite's SV_Target1 =
    /// emissive x its alpha; every forward pass after it draws [scene | this] too; the post chain's EmissiveToGlowPS takes
    /// rgb x a, blurs it and adds it over the picture). Sora draws only the scene target, so at his pixels this target keeps
    /// what was BEHIND him (the Erdtree's emissive): its glow went over him. With its state at the end of the submission.
    pub glow: Option<(usize, i32)>,
    /// (IMPL_ER80) the state of the resource named in `WATCH` (the scene target of his colour draw) at the velocity split
    /// (the end of the list with the velocity pass, before the game's TAA); 0 = unknown / contradicted this frame.
    pub watch_state: i32,
}

/// (IMPL_ER80) the scene target his colour draw wrote (gfx.rs sets it): its state at the velocity split goes into
/// `Submission::watch_state` (the late re-composite is drawn there, before the game's TAA).
pub static WATCH: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// (IMPL_ER65, offline harness only) a resource taken as the game's velocity target when it becomes a render target (0 =
/// none): the harness has no game draw hook that would recognise its velocity pass.
pub static TEST_VELOCITY: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
/// (IMPL_ER78, offline harness only) a resource taken as the game's diffuse light target (0 = none): the harness has no
/// full-screen draw hook that would recognise its light passes.
pub static TEST_LIGHT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
/// (HY, offline harness only) a resource taken as the game's screen-space shadow mask (0 = none).
pub static TEST_MASK: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub fn on_execute(queue: usize, count: u32, lists: *const *mut c_void) -> Submission {
    let mut out = Submission::default();
    let Ok(mut guard) = STATE.lock() else { return out };
    let Some(state) = guard.as_mut() else { return out };
    let mut finished = None;
    // The scene depth we hold, and its size (what a full-resolution target has).
    let depth = state.held.as_ref().map(|(p, _)| *p).and_then(|p| state.resources.get(&p).map(|i| (p, i.width, i.height)));
    let (mut g_buffer, mut hdr): (bool, Option<usize>) = (false, None);
    let mut pairs: Vec<usize> = Vec::new();
    let mut velocity: Option<usize> = None;
    let mut mask: Option<usize> = None;
    let mut albedo: Option<usize> = None;
    let logging = SURVEY.load(Ordering::Relaxed) > 0;
    let mut depth_seen = false;
    // (IMPL_ER17) the caster passes of this submission: (atlas, its state after, depth view, pass, list index)
    let mut casts: Vec<(usize, i32, usize, SunPass, usize)> = Vec::new();
    // (IMPL_ER78b) resources with a split barrier begun (BEGIN_ONLY) and not ended yet in this submission
    let mut open_splits: Vec<usize> = Vec::new();
    // (IMPL_ER80) the latest pair of full-size HDR targets drawn with the scene depth written (the composite: scene + glow)
    let mut hdr_pair: Option<(usize, usize)> = None;
    state.submissions += 1;
    if logging {
        state.lines.push(format!("submission {} on queue {queue:#x}: {count} list(s)", state.submissions));
    }
    for k in 0..count as usize {
        // SAFETY: the game's own argument array.
        let list = unsafe { *lists.add(k) } as usize;
        let events = state.lists.get(&list).cloned().unwrap_or_default();
        let (mut sets, mut clears) = (0u32, 0u32);
        let mut lines: Vec<String> = Vec::new();
        // The resources this list holds as render targets right now.
        let mut targets: Vec<usize> = Vec::new();
        // The scene depth's state inside this list (its depth plane).
        let mut depth_now = depth.and_then(|(p, ..)| state.resources.get(&p)).and_then(|i| i.states.first().copied()).unwrap_or(0);
        let mut depth_only_view = 0usize;
        // (IMPL_ER53) this list drew into the G-buffer
        let mut g_buffer_here = false;
        // (RSF2 round 14) a half-size depth of the scene depth's format went to DEPTH_WRITE in this list
        let mut half_here: Option<usize> = None;
        // (IMPL_ER78) the G-buffer's targets in this list
        let mut g_buffer_set: Vec<usize> = Vec::new();
        for event in events {
            match event {
                Event::Barrier { resource, subresource, before, after, flags } => {
                    // (a split barrier takes effect at its end)
                    if flags & D3D12_RESOURCE_BARRIER_FLAG_BEGIN_ONLY.0 != 0 {
                        // (IMPL_ER78b) a split barrier begun and not yet ended: the resource is between two states
                        open_splits.push(resource);
                        continue;
                    }
                    if flags & D3D12_RESOURCE_BARRIER_FLAG_END_ONLY.0 != 0 {
                        open_splits.retain(|r| *r != resource);
                    }
                    let frame = state.frame;
                    // The pass structure: targets go to RENDER_TARGET, then the depth to
                    // DEPTH_WRITE; afterwards all go back.
                    if after & D3D12_RESOURCE_STATE_RENDER_TARGET.0 != 0 {
                        if !targets.contains(&resource) {
                            targets.push(resource);
                        }
                    } else {
                        targets.retain(|t| *t != resource);
                    }
                    if let Some((depth, width, height)) = depth {
                        if resource == depth && subresource != 1 {
                            depth_now = after;
                            depth_seen = true;
                        }
                        // (RSF2 round 14) the half-size depth (the scene depth's format, half its size, one sample)
                        if resource != depth && subresource != 1 && after == D3D12_RESOURCE_STATE_DEPTH_WRITE.0 && half_here.is_none() {
                            let scene_format = state.resources.get(&depth).map(|i| i.format);
                            if state.resources.get(&resource).is_some_and(|i| i.is_depth() && i.samples == 1 && i.width * 2 == width && i.height * 2 == height && Some(i.format) == scene_format) {
                                half_here = Some(resource);
                            }
                        }

                        if resource == depth && subresource != 1 && after == D3D12_RESOURCE_STATE_DEPTH_WRITE.0 {
                            let full = |t: &usize| state.resources.get(t).is_some_and(|i| i.width == width && i.height == height);
                            if targets.len() >= 4 && targets.iter().all(full) {
                                g_buffer = true;
                                g_buffer_here = true;
                                g_buffer_set.clone_from(&targets);
                                albedo = targets.iter().copied().find(|t| state.resources.get(t).is_some_and(|i| i.format == DXGI_FORMAT_R8G8B8A8_TYPELESS)).or(albedo);
                            } else if (1..=2).contains(&targets.len()) && targets.iter().all(|t| full(t) && state.resources.get(t).is_some_and(|i| i.format == DXGI_FORMAT_R11G11B10_FLOAT || i.format == DXGI_FORMAT_R16G16B16A16_FLOAT)) {
                                hdr = Some(targets[0]);
                                if targets.len() == 2 {
                                    hdr_pair = Some((targets[0], targets[1]));
                                }
                            }
                        }
                    }
                    // (IMPL_ER65, offline harness only) a target named as the velocity target
                    if velocity.is_none() && resource != 0 && resource == TEST_VELOCITY.load(Ordering::Relaxed) && after & D3D12_RESOURCE_STATE_RENDER_TARGET.0 != 0 {
                        velocity = Some(resource);
                    }
                    if after == D3D12_RESOURCE_STATE_PRESENT.0 && before & D3D12_RESOURCE_STATE_RENDER_TARGET.0 != 0 && state.back_buffers.contains(&resource) {
                        finished = Some(resource);
                    }
                    if let Some(info) = state.resources.get_mut(&resource) {
                        // (hardening) The game says what state the resource is in: when that is not
                        // what we had on record, the record was wrong for this frame.
                        let tracked_ok = if subresource == D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES { info.states.iter().all(|s| *s == before) } else { info.states.get(subresource as usize).is_none_or(|s| *s == before) };
                        if !tracked_ok {
                            info.mismatch = frame;
                        }
                        if subresource == D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES {
                            info.states.iter_mut().for_each(|s| *s = after);
                        } else if let Some(s) = info.states.get_mut(subresource as usize) {
                            *s = after;
                        }
                        if after & D3D12_RESOURCE_STATE_DEPTH_WRITE.0 != 0 {
                            info.depth_writes += 1;
                        }
                        info.seen = frame;
                    }
                    if logging {
                        let sub = if subresource == D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES { "all".to_string() } else { subresource.to_string() };
                        lines.push(format!("      barrier {} [{sub}] {} -> {}", describe(resource, state.resources.get(&resource)), state_text(before), state_text(after)));
                    }
                }
                Event::Fullscreen(table) => {
                    if let (Some((_, width, height)), None) = (depth, velocity) {
                        if targets.len() == 1 && state.resources.get(&targets[0]).is_some_and(|i| i.width == width && i.height == height && i.format == DXGI_FORMAT_R16G16_FLOAT) {
                            velocity = Some(targets[0]);
                        }
                    }
                    // (the light accumulation passes test against the depth without writing it)
                    if let Some((_, width, height)) = depth {
                        let hdr_full = |t: &usize| state.resources.get(t).is_some_and(|i| i.width == width && i.height == height && i.format == DXGI_FORMAT_R11G11B10_FLOAT);
                        let depth_read = depth_now & D3D12_RESOURCE_STATE_DEPTH_READ.0 != 0 && depth_now & D3D12_RESOURCE_STATE_DEPTH_WRITE.0 == 0;
                        // The screen-space shadow mask: the one full-size R8G8B8A8 target drawn alone,
                        // full screen, against the read-only depth (GXScreenSpaceCascadedShadow_*).
                        if targets.len() == 1 && depth_read && mask.is_none() && state.resources.get(&targets[0]).is_some_and(|i| i.width == width && i.height == height && i.format == DXGI_FORMAT_R8G8B8A8_UNORM) {
                            mask = Some(targets[0]);
                        }
                        if targets.len() == 2 && targets.iter().all(hdr_full) && depth_now & D3D12_RESOURCE_STATE_DEPTH_READ.0 != 0 && depth_now & D3D12_RESOURCE_STATE_DEPTH_WRITE.0 == 0 {
                            if !pairs.contains(&targets[0]) {
                                pairs.push(targets[0]);
                            }
                            if out.table_count < out.tables.len() && !out.tables[..out.table_count].contains(&table) {
                                out.tables[out.table_count] = table;
                                out.table_count += 1;
                            }
                        }
                    }
                }
                Event::Cast { resource, after, pass } => {
                    // (a 16-bit depth atlas, not the scene depth; the view the list set for its depth-only passes)
                    if depth_only_view != 0 && casts.len() < 64 && state.resources.get(&resource).is_some_and(|i| i.is_depth() && i.format == DXGI_FORMAT_R16_TYPELESS && i.width >= 2048) {
                        casts.push((resource, after, depth_only_view, pass, k));
                    }
                    if logging {
                        lines.push(format!("      caster pass ended: viewport {:?} scissor {:?} vertex table {:#x}", pass.viewport, pass.scissor, pass.table.handle));
                    }
                }
                Event::Targets { count, first, depth } => {
                    if count == 0 {
                        depth_only_view = depth;
                    }
                    sets += 1;
                    if logging {
                        lines.push(format!("      targets {count} (first view {first:#x}) depth view {depth:#x}"));
                    }
                }
                Event::ClearDepth { view, flags, depth } => {
                    clears += 1;
                    if logging {
                        lines.push(format!("      clear depth view {view:#x} flags {flags} to {depth}"));
                    }
                }
            }
        }
        // (RSF2 round 14) the first list of the submission that rebuilt the half-size depth: the effects' split
        if let (Some(h), 0, Some((scene, ..))) = (half_here, out.effects_split, depth) {
            let planes = |r: usize| state.resources.get(&r).map(|i| [i.states.first().copied().unwrap_or(0), i.states.get(1).copied().unwrap_or(0)]).unwrap_or([0; 2]);
            if state.resources.get(&h).is_some_and(|i| i.mismatch != state.frame) && !open_splits.contains(&h) {
                out.effects_split = k + 1;
                out.effects_half = Some((h, planes(h), scene, planes(scene)));
            }
        }
        // (IMPL_ER53) the last G-buffer list so far: the early depth's place, and the scene depth's planes there
        if g_buffer_here {
            if let Some((depth, ..)) = depth {
                let planes = state.resources.get(&depth).map(|i| [i.states.first().copied().unwrap_or(0), i.states.get(1).copied().unwrap_or(0)]).unwrap_or([0; 2]);
                out.early_split = k + 1;
                out.early_states = planes;
                // (IMPL_ER78) the G-buffer's targets and their states at this point (a target whose record was contradicted
                // in this frame stays unknown: the hybrid then falls back)
                out.gbuffer = [crate::hybrid::GbTarget::default(); crate::hybrid::ROLES];
                out.gbuffer_count = g_buffer_set.len();
                for (slot, t) in out.gbuffer.iter_mut().zip(g_buffer_set.iter()) {
                    if let Some(i) = state.resources.get(t).filter(|i| i.mismatch != state.frame && !open_splits.contains(t)) {
                        *slot = crate::hybrid::GbTarget { resource: *t, format: i.format.0, width: i.width, height: i.height, state: i.states.first().copied().unwrap_or(0) };
                    }
                }
            }
        }
        // The velocity pass ended with this list: what our own velocity draw needs to know.
        if let (Some(target), None) = (velocity, out.velocity) {
            if let (Some(info), Some((depth, ..))) = (state.resources.get(&target), depth) {
                let planes = state.resources.get(&depth).map(|i| [i.states.first().copied().unwrap_or(0), i.states.get(1).copied().unwrap_or(0)]).unwrap_or([0; 2]);
                out.velocity = Some((target, info.states.first().copied().unwrap_or(0)));
                out.split = k + 1;
                // (IMPL_ER80) the scene target's state at this point (for the late re-composite before the TAA)
                let watched = WATCH.load(Ordering::Relaxed);
                out.watch_state = state.resources.get(&watched).filter(|i| watched != 0 && i.mismatch != state.frame).and_then(|i| i.states.first().copied()).unwrap_or(0);
                out.depth_states = planes;
            }
        }
        if logging {
            state.lines.push(format!("    list {list:#x}: {sets} target set(s), {clears} depth clear(s)"));
            state.lines.append(&mut lines);
        }
    }
    if logging && finished.is_some() {
        state.lines.push("    (this submission finishes the back buffer)".to_string());
    }
    out.finished = finished;
    out.depth_seen = depth_seen;
    // (IMPL_ER17) THE SUN'S ATLAS = the 16-bit depth atlas with the most caster passes in this submission (MEASURED by day:
    // 8192 x 4096 with nine passes; the two others, 6144 x 4096 and 4096 x 4096, have two and one), all under one view.
    if !casts.is_empty() {
        let mut best: Option<(usize, usize)> = None;
        for (resource, ..) in &casts {
            let n = casts.iter().filter(|c| c.0 == *resource).count();
            if best.is_none_or(|(_, m)| n > m) {
                best = Some((*resource, n));
            }
        }
        if let Some((resource, n)) = best.filter(|(_, n)| *n >= 3) {
            let mine: Vec<&(usize, i32, usize, SunPass, usize)> = casts.iter().filter(|c| c.0 == resource).collect();
            let view = mine[0].2;
            let frame = state.frame;
            let clean = state.resources.get(&resource).is_some_and(|i| i.mismatch != frame);
            if clean && mine.iter().all(|c| c.2 == view) && n <= SUN_PASSES {
                let last = mine[mine.len() - 1];
                let mut sun = SunAtlas { resource, state: last.1, view, split: last.4 + 1, passes: [SunPass::default(); SUN_PASSES], count: n };
                for (slot, c) in sun.passes.iter_mut().zip(mine.iter()) {
                    *slot = c.3;
                }
                out.sun = Some(sun);
            }
        }
    }
    {
        let frame = state.frame;
        let ok = |p: Option<usize>| p.and_then(|p| state.resources.get(&p)).is_none_or(|i| i.mismatch != frame);
        out.clean = ok(depth.map(|d| d.0)) && ok(hdr) && ok(out.velocity.map(|v| v.0)) && ok(finished);
    }
    if let (true, Some(target)) = (g_buffer, hdr) {
        out.scene = state.resources.get(&target).and_then(|i| i.states.first().copied()).map(|s| (target, s));
        let readable = |l: &usize| *l != target && state.resources.get(l).is_some_and(|i| i.states.iter().all(|s| *s & D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE.0 != 0));
        out.light = pairs.first().copied().filter(readable);
        // (the pass the directional light draws in is the LAST of them: where there is no
        // environment pass in front of it, it is the only one)
        out.light2 = pairs.last().copied().filter(readable).or_else(|| Some(TEST_LIGHT.load(Ordering::Relaxed)).filter(|t| *t != 0).filter(readable));
        out.mask = mask.filter(readable).or_else(|| Some(TEST_MASK.load(Ordering::Relaxed)).filter(|t| *t != 0).filter(readable));
        // (IMPL_ER80) the glow source: the other member of the latest pair that holds the scene target; full size, a float
        // HDR format, not contradicted this frame, no split barrier open on it
        let (frame, depth_size) = (state.frame, depth.map(|(_, w, h)| (w, h)));
        out.glow = hdr_pair
            .and_then(|(a, b)| if a == target { Some(b) } else if b == target { Some(a) } else { None })
            .filter(|g| *g != target && !open_splits.contains(g))
            .and_then(|g| state.resources.get(&g).filter(|i| i.mismatch != frame && depth_size.is_some_and(|(w, h)| i.width == w && i.height == h) && (i.format == DXGI_FORMAT_R11G11B10_FLOAT || i.format == DXGI_FORMAT_R16G16B16A16_FLOAT) && i.samples == 1).map(|i| (g, i.states.first().copied().unwrap_or(0))));
        out.albedo = albedo.filter(readable);
        if logging {
            state.lines.push(format!("    (this submission is the deferred stage: G-buffer, lights, composite into {}: our draw goes right after it)", describe(target, state.resources.get(&target))));
            if let Some((g, st)) = out.glow {
                state.lines.push(format!("    (IMPL_ER80) its glow source (the composite's second target): {} resting {}", describe(g, state.resources.get(&g)), state_text(st)));
            }
        }
    }
    out
}

/// The game's main scene depth: the depth target of the back buffer's size that the game writes
/// every frame (the newest one if there are several), with its state as of the latest executed list.
pub struct SceneDepth {
    pub resource: ID3D12Resource,
    pub info: Info,
    /// The game transitioned it in the frame in progress (its state is observed this frame).
    pub fresh: bool,
}

/// Called at Present: ends the frame of the survey.
pub fn end_frame(width: u32, height: u32) {
    let Ok(mut guard) = STATE.lock() else { return };
    let Some(state) = guard.as_mut() else { return };
    let frame = state.frame;
    crate::shaft::FRAME.fetch_add(1, Ordering::Relaxed);
    if SURVEY.load(Ordering::Relaxed) > 0 {
        let left = SURVEY.fetch_sub(1, Ordering::SeqCst) - 1;
        rlog!("survey: ---- frame {frame}: {} submission(s) before Present (back buffer {width}x{height})", state.submissions);
        for line in state.lines.drain(..) {
            rlog!("survey: {line}");
        }
        if left == 0 {
            rlog!("survey: ---- resources seen in transitions:");
            let mut all: Vec<(&usize, &Info)> = state.resources.iter().collect();
            all.sort_by_key(|(p, _)| **p);
            for (pointer, info) in all {
                rlog!("survey:   {} | states now {} | last frame {} | depth-write frames {}", describe(*pointer, Some(info)), info.states.iter().take(4).map(|s| state_text(*s)).collect::<Vec<_>>().join(" / "), info.seen, info.depth_writes);
            }
        }
    }
    state.lines.clear();
    state.submissions = 0;
    state.frame += 1;
    // Forget what the game has not touched for a long time (the pointer may be reused).
    state.resources.retain(|_, info| frame.saturating_sub(info.seen) < 600);
}

/// The scene depth for a target of this size as of the latest executed list, if one is known.
pub fn scene_depth(width: u32, height: u32) -> Option<SceneDepth> {
    let mut guard = STATE.lock().ok()?;
    let state = guard.as_mut()?;
    let frame = state.frame;
    let best = state
        .resources
        .iter()
        .filter(|(_, i)| i.is_depth() && i.width == width as u64 && i.height == height && i.samples == 1 && frame.saturating_sub(i.seen) < 3 && i.depth_writes > 0)
        .max_by_key(|(_, i)| (i.seen, i.depth_writes))
        .map(|(p, i)| (*p, i.clone()));
    let Some((pointer, info)) = best else {
        state.held = None;
        return None;
    };
    if state.held.as_ref().map(|(p, _)| *p) != Some(pointer) {
        // The game transitioned it within the last frames: it is alive. Keep it alive for us.
        let raw = pointer as *mut c_void;
        // SAFETY: see above.
        let resource = unsafe { ID3D12Resource::from_raw_borrowed(&raw) }?.clone();
        rlog!("render: scene depth found: {} (states now {})", describe(pointer, Some(&info)), info.states.iter().take(2).map(|s| state_text(*s)).collect::<Vec<_>>().join(" / "));
        state.held = Some((pointer, resource));
    }
    let fresh = info.seen == frame && info.mismatch != frame;
    state.held.as_ref().map(|(_, r)| SceneDepth { resource: r.clone(), info, fresh })
}

/// HARDENING: `pointer` is a texture of this size that the game transitioned in the frame in
/// progress (so it is alive, and what we know of it is of this frame) and nothing contradicted.
pub fn usable_now(pointer: usize, width: u32, height: u32) -> bool {
    if pointer == 0 {
        return false;
    }
    let Ok(guard) = STATE.lock() else { return false };
    let Some(state) = guard.as_ref() else { return false };
    state.resources.get(&pointer).is_some_and(|i| i.seen == state.frame && i.mismatch != state.frame && i.width == width as u64 && i.height == height && i.samples == 1)
}

/// (IMPL_ER78) The resource format, mip count and array size of a resource of the game as last read from it.
pub fn format_of(pointer: usize) -> Option<(i32, u16, u16)> {
    let guard = STATE.lock().ok()?;
    let state = guard.as_ref()?;
    state.resources.get(&pointer).map(|i| (i.format.0, i.mips, i.layers))
}

/// The states (first two subresources) of a resource of the game as of the latest executed list,
/// when the game transitioned it in THIS frame and nothing contradicted the record.
pub fn states_now(pointer: usize) -> Option<[i32; 2]> {
    let guard = STATE.lock().ok()?;
    let state = guard.as_ref()?;
    let info = state.resources.get(&pointer).filter(|i| i.seen == state.frame && i.mismatch != state.frame)?;
    let first = *info.states.first()?;
    Some([first, info.states.get(1).copied().unwrap_or(first)])
}

/// HARDENING: everything learnt about the game's resources is dropped (the device was lost).
pub fn forget() {
    if let Ok(mut guard) = STATE.lock() {
        if let Some(state) = guard.as_mut() {
            state.resources.clear();
            state.lists.clear();
            state.held = None;
            state.back_buffers.clear();
        }
    }
}

/// HOLDS THE MAIN SCENE DEPTH whatever its size (called at Present): the largest depth-STENCIL
/// target (the scene depth's formats; the shadow atlases are plain 16-bit depth) the game wrote in
/// the last frames. The render resolution need not be the window's (borderless with a smaller
/// resolution: measured, run 3). Returns its size.
pub fn hold_main_depth() -> Option<(u32, u32)> {
    let (width, height) = {
        let guard = STATE.lock().ok()?;
        let state = guard.as_ref()?;
        let frame = state.frame;
        state
            .resources
            .values()
            .filter(|i| i.is_depth() && i.samples == 1 && frame.saturating_sub(i.seen) < 3 && i.depth_writes > 0)
            .filter(|i| matches!(i.format, DXGI_FORMAT_R32G8X24_TYPELESS | DXGI_FORMAT_D32_FLOAT_S8X24_UINT | DXGI_FORMAT_R24G8_TYPELESS | DXGI_FORMAT_D24_UNORM_S8_UINT))
            .max_by_key(|i| i.width * i.height as u64)
            .map(|i| (i.width as u32, i.height))?
    };
    scene_depth(width, height).map(|_| (width, height))
}
