//! (IMPL_ER80b) THE LIGHT SHAFTS NEVER ADD OVER SORA.
//!
//! Elden Ring's light shafts (GXPostEffect: LightShaft_DetectSky* -> LightShaft_SkyMaskBlur* -> LightShaft_CompositePS) are a
//! radial blur of the SKY toward the light, added over the picture by one full-screen draw whose pixel shader is
//! LightShaft_CompositePS (DXIL HASH part be1fbb36801fe6046cb7bede87af3d90; it samples t0 with s1 and outputs rgb, 1; it
//! reads no depth). His depth and stencil keep his own pixels out of the sky mask (IMPL_ER53/56), but the blur still carries
//! the sky BESIDE him (the Erdtree's crown, the sun) across his pixels: the streaks over his face in 52.jpg.
//!
//! How it is masked, at that draw only:
//! - The game's pipeline states are watched as they are made (`ID3D12Device::CreateGraphicsPipelineState`,
//!   `ID3D12Device2::CreatePipelineState`, `ID3D12PipelineLibrary::LoadGraphicsPipeline / LoadPipeline`): one whose pixel
//!   shader carries that hash gets a TWIN made from the same description (same root signature, shaders, blend, targets),
//!   with ONLY a stencil test added: a depth-stencil format, stencil EQUAL 0, no depth test, nothing written.
//! - gfx.rs writes HIS MASK each frame (`set_mask`): a copy of the scene depth right after his colour draw with its stencil
//!   cleared to 0 and 1 written where his opaque / masked surfaces are the visible ones (his early-depth pipeline states,
//!   depth GREATER_EQUAL: what is in front of him keeps 0).
//! - At the game's draw with that pipeline (the command-list hooks of cbcap.rs / survey.rs call in here) the twin is set, the
//!   game's own render targets are bound again with OUR mask as a read-only depth-stencil view, the stencil reference 0,
//!   the draw is made, and the game's pipeline state, render targets (with its own depth view) and stencil reference are put
//!   back. Nothing else of the game's state is touched (root signature, root arguments, heaps, viewports, topology stay).
//! - PER DRAW CHECKS (else the game's draw unchanged, counted with the reason): `r.shaft_mask` on; a twin for the mask's
//!   format; a mask made in this frame or the one before; the list's render targets, stencil reference and viewport KNOWN
//!   (seen since its Reset); the viewport covering exactly the mask's size from (0, 0); when the render target view was seen
//!   being made, its size equal to the mask's.

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;

use windows::core::{Interface, GUID, HRESULT, PCWSTR};
use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;

use crate::hook::{as_ours, entry, is_ours, patch, LIST_VTABLE};
use crate::{rlog, DEAD};

/// LightShaft_CompositePS (gxposteffect.shaderbnd, 2.7.1.0): the digest of its DXIL `HASH` part, and the container's own digest.
pub const SHAFT_HASH: [u8; 16] = [0xbe, 0x1f, 0xbb, 0x36, 0x80, 0x1f, 0xe6, 0x04, 0x6c, 0xb7, 0xbe, 0xde, 0x87, 0xaf, 0x3d, 0x90];
pub const SHAFT_CONTAINER: [u8; 16] = [0x62, 0x1c, 0x83, 0x0c, 0x29, 0x4c, 0xd1, 0x48, 0x20, 0xc5, 0x3c, 0xb3, 0x1c, 0x12, 0xa1, 0x2b];

/// `r.shaft_mask` (default 1).
static WANTED: AtomicBool = AtomicBool::new(true);
/// The frame counter (survey.rs end_frame).
pub static FRAME: AtomicU64 = AtomicU64::new(0);
/// Twins made (fast test on the hot path: nothing is followed while it is 0).
static TWIN_COUNT: AtomicUsize = AtomicUsize::new(0);

static O_CREATE_GRAPHICS: AtomicUsize = AtomicUsize::new(0);
static O_CREATE_STREAM: AtomicUsize = AtomicUsize::new(0);
static O_CREATE_RTV: AtomicUsize = AtomicUsize::new(0);
static O_LOAD_GRAPHICS: AtomicUsize = AtomicUsize::new(0);
static O_LOAD_STREAM: AtomicUsize = AtomicUsize::new(0);
static O_STENCIL_REF: AtomicUsize = AtomicUsize::new(0);

/// The game's light-shaft pipeline states and their twins, by depth-stencil format of the twin.
struct Twin {
    /// the game's pipeline state, kept alive (its address cannot be taken by another object while we know it)
    _game: ID3D12PipelineState,
    by_format: Vec<(i32, ID3D12PipelineState)>,
}
static TWINS: Mutex<Option<HashMap<usize, Twin>>> = Mutex::new(None);

/// HIS MASK for the frame: the read-only depth-stencil view (a CPU descriptor of ours), its size, its format, the frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct Mask {
    pub view: usize,
    pub width: u32,
    pub height: u32,
    pub format: i32,
    pub frame: u64,
}
/// His masks of the frame: the full size, and the reduced sizes the composite was seen drawing at (WANT_SIZES).
static MASKS: Mutex<Vec<Mask>> = Mutex::new(Vec::new());
/// Viewport sizes (from (0, 0)) the composite was drawn at for which no mask existed: gfx.rs makes them from the next frame.
static WANT_SIZES: Mutex<Vec<(u32, u32)>> = Mutex::new(Vec::new());
/// Most reduced sizes made.
pub const MAX_SIZES: usize = 3;

/// gfx.rs: the reduced sizes wanted (besides the full one).
pub fn wanted_sizes() -> Vec<(u32, u32)> {
    WANT_SIZES.lock().map(|w| w.clone()).unwrap_or_default()
}

/// What a command list has bound since its Reset (only followed while a twin exists).
#[derive(Clone, Default)]
struct ListState {
    pso: usize,
    /// the game's OMSetRenderTargets: count, its handles (one with `single`), single, its depth view (0 = none)
    targets: Option<(u32, [usize; 8], bool, usize)>,
    stencil: Option<u32>,
    viewport: Option<[f32; 4]>,
}
static LISTS: Mutex<Option<HashMap<usize, ListState>>> = Mutex::new(None);
/// Render target views the game made: CPU handle -> (width, height).
static RTVS: Mutex<Option<HashMap<usize, (u32, u32)>>> = Mutex::new(None);

/// For the log: pipelines seen, draws seen / masked / left alone and why.
static SEEN_PSOS: AtomicU32 = AtomicU32::new(0);
static DRAWS: AtomicU32 = AtomicU32::new(0);
static MASKED: AtomicU32 = AtomicU32::new(0);
static LEFT: AtomicU32 = AtomicU32::new(0);
static WHY: Mutex<&'static str> = Mutex::new("none");

pub fn set_wanted(on: bool) {
    WANTED.store(on, Ordering::Relaxed);
}

/// gfx.rs: a twin exists and the key is on (his mask is worth making this frame).
pub fn wanted() -> bool {
    WANTED.load(Ordering::Relaxed) && TWIN_COUNT.load(Ordering::Relaxed) > 0 && !DEAD.load(Ordering::Relaxed)
}

/// gfx.rs: his mask of this frame is recorded (it is complete when that list has run, before the game's post chain).
pub fn set_masks(list: &[(usize, u32, u32, i32)]) {
    let frame = FRAME.load(Ordering::Relaxed);
    if let Ok(mut m) = MASKS.lock() {
        *m = list.iter().map(|(view, width, height, format)| Mask { view: *view, width: *width, height: *height, format: *format, frame }).collect();
    }
}

#[inline]
fn following() -> bool {
    TWIN_COUNT.load(Ordering::Relaxed) > 0 && WANTED.load(Ordering::Relaxed) && !DEAD.load(Ordering::Relaxed)
}

fn with_list(list: usize, f: impl FnOnce(&mut ListState)) {
    if let Ok(mut guard) = LISTS.lock() {
        let map = guard.get_or_insert_with(HashMap::new);
        if map.len() > 4096 {
            map.clear();
        }
        f(map.entry(list).or_default());
    }
}

fn left(why: &'static str) {
    LEFT.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut w) = WHY.lock() {
        *w = why;
    }
}

// ---- what the command lists bind (called from the hooks of survey.rs and cbcap.rs; never for our own calls)

/// A list was reset with this initial pipeline state: everything else at its defaults (stencil reference 0).
pub fn on_reset(list: usize, pipeline: usize) {
    if following() {
        with_list(list, |s| *s = ListState { pso: pipeline, targets: None, stencil: Some(0), viewport: None });
    } else if let Ok(mut guard) = LISTS.lock() {
        if let Some(map) = guard.as_mut() {
            map.remove(&list);
        }
    }
}

pub fn on_pso(list: usize, pso: usize) {
    if following() {
        with_list(list, |s| s.pso = pso);
    }
}

/// # Safety
/// The arguments of an `OMSetRenderTargets` call.
pub unsafe fn on_targets(list: usize, count: u32, targets: *const D3D12_CPU_DESCRIPTOR_HANDLE, single: i32, depth: *const D3D12_CPU_DESCRIPTOR_HANDLE) {
    if !following() {
        return;
    }
    let mut handles = [0usize; 8];
    let known = count <= 8 && (count == 0 || !targets.is_null());
    if known {
        let n = if single != 0 { count.min(1) } else { count };
        for (k, h) in handles.iter_mut().enumerate().take(n as usize) {
            *h = (*targets.add(k)).ptr;
        }
    }
    let depth = if depth.is_null() { 0 } else { (*depth).ptr };
    with_list(list, |s| s.targets = known.then_some((count, handles, single != 0, depth)));
}

pub fn on_viewport(list: usize, v: Option<[f32; 4]>) {
    if following() {
        with_list(list, |s| s.viewport = v);
    }
}

// ---- the draw

/// The game's 3-vertex draw on `list`: when its pipeline state is a light-shaft composite with a twin, the masked draw is
/// made here and true returned (the caller does not draw again).
///
/// # Safety
/// `this` is the command list of a `DrawInstanced` call being recorded; `original` is the real `DrawInstanced`.
pub unsafe fn draw(this: *mut c_void, count: u32, instances: u32, first: u32, first_instance: u32, original: unsafe extern "system" fn(*mut c_void, u32, u32, u32, u32)) -> bool {
    if !following() || is_ours() {
        return false;
    }
    let list = this as usize;
    let state = LISTS.lock().ok().and_then(|g| g.as_ref().and_then(|m| m.get(&list).cloned()));
    let Some(state) = state else { return false };
    let twins = TWINS.lock().ok().and_then(|g| g.as_ref().and_then(|m| m.get(&state.pso).map(|t| t.by_format.clone())));
    let Some(twins) = twins else { return false };
    DRAWS.fetch_add(1, Ordering::Relaxed);
    let (Some((n, handles, single, depth)), Some(stencil), Some(viewport)) = (state.targets, state.stencil, state.viewport) else {
        left("the list's render targets / stencil reference / viewport were not all seen since its Reset");
        return false;
    };
    if n == 0 || viewport[0] != 0.0 || viewport[1] != 0.0 || viewport[2] < 1.0 || viewport[3] < 1.0 {
        left("the composite's viewport does not start at (0, 0)");
        return false;
    }
    let (vw, vh) = (viewport[2] as u32, viewport[3] as u32);
    let masks = MASKS.lock().map(|m| m.clone()).unwrap_or_default();
    let now = FRAME.load(Ordering::Relaxed);
    let Some(full) = masks.first().copied() else {
        left("no mask of his made yet");
        return false;
    };
    if full.frame + 1 < now {
        left("his mask is not of this frame (he was not drawn)");
        return false;
    }
    let Some(mask) = masks.iter().find(|m| m.width == vw && m.height == vh).copied() else {
        // (a reduced size: made from the next frame, when it is no larger than the full one)
        if vw <= full.width && vh <= full.height && vw >= 16 && vh >= 16 {
            if let Ok(mut want) = WANT_SIZES.lock() {
                if !want.contains(&(vw, vh)) && want.len() < MAX_SIZES {
                    want.push((vw, vh));
                    rlog!("render: (IMPL_ER80b) the game's light-shaft composite draws at {vw}x{vh}: his mask of that size is made from the next frame");
                }
            }
            left("no mask of the composite's size yet");
        } else {
            left("the composite's viewport is larger than his mask");
        }
        return false;
    };
    let Some(twin) = twins.iter().find(|(f, _)| *f == mask.format).map(|(_, t)| t.clone()) else {
        left("no twin for the mask's depth format");
        return false;
    };
    let sizes = RTVS.lock().ok().and_then(|g| g.as_ref().and_then(|m| m.get(&handles[0]).copied()));
    if sizes.is_some_and(|(w, h)| w != mask.width || h != mask.height) {
        left("the composite's render target is not of the mask's size");
        return false;
    }
    let raw = this;
    let Some(cl) = ID3D12GraphicsCommandList::from_raw_borrowed(&raw) else { return false };
    let rt: Vec<D3D12_CPU_DESCRIPTOR_HANDLE> = handles.iter().take(if single { 1 } else { n as usize }).map(|h| D3D12_CPU_DESCRIPTOR_HANDLE { ptr: *h }).collect();
    let ours = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: mask.view };
    let theirs = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: depth };
    let game_pso = state.pso as *mut c_void;
    let Some(game_pso) = ID3D12PipelineState::from_raw_borrowed(&game_pso) else { return false };
    as_ours(|| {
        cl.OMSetRenderTargets(n, Some(rt.as_ptr()), single, Some(&ours));
        cl.SetPipelineState(&twin);
        cl.OMSetStencilRef(0);
        original(this, count, instances, first, first_instance);
        // the game's state back
        cl.SetPipelineState(game_pso);
        cl.OMSetRenderTargets(n, Some(rt.as_ptr()), single, if depth == 0 { None } else { Some(&theirs) });
        cl.OMSetStencilRef(stencil);
    });
    MASKED.fetch_add(1, Ordering::Relaxed);
    true
}

// ---- the pipeline states as the game makes them

/// The digest of the `HASH` part of a DXIL container, and the container's own digest.
pub fn dxil_hashes(code: &[u8]) -> Option<([u8; 16], [u8; 16])> {
    if code.len() < 32 || &code[0..4] != b"DXBC" {
        return None;
    }
    let word = |at: usize| code.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize);
    let mut container = [0u8; 16];
    container.copy_from_slice(&code[4..20]);
    let parts = word(28)?;
    let mut hash = [0u8; 16];
    for k in 0..parts.min(64) {
        let at = word(32 + 4 * k)?;
        if code.get(at..at + 4) == Some(b"HASH") && word(at + 4)? >= 20 {
            hash.copy_from_slice(code.get(at + 12..at + 28)?);
            return Some((hash, container));
        }
    }
    Some(([0u8; 16], container))
}

pub fn is_shaft(code: &[u8]) -> bool {
    dxil_hashes(code).is_some_and(|(h, c)| h == SHAFT_HASH || c == SHAFT_CONTAINER)
}

unsafe fn bytecode(b: &D3D12_SHADER_BYTECODE) -> &[u8] {
    if b.pShaderBytecode.is_null() || b.BytecodeLength == 0 {
        &[]
    } else {
        std::slice::from_raw_parts(b.pShaderBytecode as *const u8, b.BytecodeLength)
    }
}

/// The twin of a light-shaft description: the same with a depth-stencil format and the stencil test EQUAL (reference 0
/// at the draw), no depth test, nothing written; no cached blob.
pub fn twin_desc(desc: &D3D12_GRAPHICS_PIPELINE_STATE_DESC, dsv: DXGI_FORMAT) -> D3D12_GRAPHICS_PIPELINE_STATE_DESC {
    let mut d = desc.clone();
    let keep = D3D12_DEPTH_STENCILOP_DESC { StencilFailOp: D3D12_STENCIL_OP_KEEP, StencilDepthFailOp: D3D12_STENCIL_OP_KEEP, StencilPassOp: D3D12_STENCIL_OP_KEEP, StencilFunc: D3D12_COMPARISON_FUNC_EQUAL };
    d.DepthStencilState = D3D12_DEPTH_STENCIL_DESC { DepthEnable: false.into(), DepthWriteMask: D3D12_DEPTH_WRITE_MASK_ZERO, DepthFunc: D3D12_COMPARISON_FUNC_ALWAYS, StencilEnable: true.into(), StencilReadMask: 0xFF, StencilWriteMask: 0, FrontFace: keep, BackFace: keep };
    d.DSVFormat = dsv;
    d.CachedPSO = D3D12_CACHED_PIPELINE_STATE::default();
    d
}

type CreateGraphicsFn = unsafe extern "system" fn(*mut c_void, *const D3D12_GRAPHICS_PIPELINE_STATE_DESC, *const GUID, *mut *mut c_void) -> HRESULT;
type CreateStreamFn = unsafe extern "system" fn(*mut c_void, *const D3D12_PIPELINE_STATE_STREAM_DESC, *const GUID, *mut *mut c_void) -> HRESULT;
type CreateRtvFn = unsafe extern "system" fn(*mut c_void, *mut c_void, *const D3D12_RENDER_TARGET_VIEW_DESC, D3D12_CPU_DESCRIPTOR_HANDLE);
type LoadGraphicsFn = unsafe extern "system" fn(*mut c_void, PCWSTR, *const D3D12_GRAPHICS_PIPELINE_STATE_DESC, *const GUID, *mut *mut c_void) -> HRESULT;
type LoadStreamFn = unsafe extern "system" fn(*mut c_void, PCWSTR, *const D3D12_PIPELINE_STATE_STREAM_DESC, *const GUID, *mut *mut c_void) -> HRESULT;
type StencilRefFn = unsafe extern "system" fn(*mut c_void, u32);

/// The game made a pipeline state `made` from `desc` (a graphics description) on `device`: a light-shaft composite gets its
/// twins (made through the ORIGINAL entry, not through our hook).
unsafe fn consider(device: *mut c_void, desc: &D3D12_GRAPHICS_PIPELINE_STATE_DESC, made: *mut c_void, how: &str) {
    if made.is_null() || !is_shaft(bytecode(&desc.PS)) {
        return;
    }
    SEEN_PSOS.fetch_add(1, Ordering::Relaxed);
    if desc.SampleDesc.Count > 1 {
        rlog!("render: (IMPL_ER80b) the game's light-shaft composite {made:p} ({how}) is multisampled: no twin (its draws stay as they are)");
        return;
    }
    let raw = made;
    let Some(game) = ID3D12PipelineState::from_raw_borrowed(&raw).cloned() else { return };
    let original: CreateGraphicsFn = std::mem::transmute(O_CREATE_GRAPHICS.load(Ordering::Relaxed));
    let mut by_format = Vec::new();
    for dsv in [DXGI_FORMAT_D32_FLOAT_S8X24_UINT, DXGI_FORMAT_D24_UNORM_S8_UINT] {
        let d = twin_desc(desc, dsv);
        let mut out: *mut c_void = std::ptr::null_mut();
        let hr = as_ours(|| original(device, &d, &ID3D12PipelineState::IID, &mut out));
        if hr.is_ok() && !out.is_null() {
            by_format.push((dsv.0, ID3D12PipelineState::from_raw(out)));
        }
    }
    let n = by_format.len();
    if n == 0 {
        rlog!("render: (IMPL_ER80b) the game's light-shaft composite {made:p} ({how}): its twin could not be made: its draws stay as they are");
        return;
    }
    if let Ok(mut guard) = TWINS.lock() {
        guard.get_or_insert_with(HashMap::new).insert(made as usize, Twin { _game: game, by_format });
        TWIN_COUNT.store(guard.as_ref().map_or(0, |m| m.len()), Ordering::Relaxed);
    }
    rlog!("render: (IMPL_ER80b) the game's light-shaft composite pipeline {made:p} was made ({how}): {n} stencil-masked twin(s) ready (r.shaft_mask)");
}

unsafe extern "system" fn create_graphics_hook(this: *mut c_void, desc: *const D3D12_GRAPHICS_PIPELINE_STATE_DESC, riid: *const GUID, out: *mut *mut c_void) -> HRESULT {
    let original: CreateGraphicsFn = std::mem::transmute(O_CREATE_GRAPHICS.load(Ordering::Relaxed));
    let hr = original(this, desc, riid, out);
    if hr.is_ok() && !desc.is_null() && !out.is_null() && !is_ours() && !DEAD.load(Ordering::Relaxed) && !riid.is_null() && *riid == ID3D12PipelineState::IID {
        let _ = std::panic::catch_unwind(|| consider(this, &*desc, *out, "CreateGraphicsPipelineState"));
    }
    hr
}

unsafe extern "system" fn load_graphics_hook(this: *mut c_void, name: PCWSTR, desc: *const D3D12_GRAPHICS_PIPELINE_STATE_DESC, riid: *const GUID, out: *mut *mut c_void) -> HRESULT {
    let original: LoadGraphicsFn = std::mem::transmute(O_LOAD_GRAPHICS.load(Ordering::Relaxed));
    let hr = original(this, name, desc, riid, out);
    if hr.is_ok() && !desc.is_null() && !out.is_null() && !is_ours() && !DEAD.load(Ordering::Relaxed) && !riid.is_null() && *riid == ID3D12PipelineState::IID {
        let _ = std::panic::catch_unwind(|| {
            // (the twin is made on the device the library belongs to)
            let raw = this;
            if let Some(library) = ID3D12PipelineLibrary::from_raw_borrowed(&raw) {
                let mut device: Option<ID3D12Device> = None;
                if let (Ok(()), Some(device)) = (library.GetDevice(&mut device), device.as_ref()) {
                    consider(device.as_raw(), &*desc, *out, "ID3D12PipelineLibrary::LoadGraphicsPipeline");
                }
            }
        });
    }
    hr
}

/// A pipeline-state stream as a graphics description, when every sub-object is one a graphics description has (`None`
/// otherwise: mesh shaders, view instancing, newer sub-objects). Absent sub-objects take the D3D12 defaults.
pub unsafe fn stream_as_graphics(stream: &D3D12_PIPELINE_STATE_STREAM_DESC) -> Option<D3D12_GRAPHICS_PIPELINE_STATE_DESC> {
    let base = stream.pPipelineStateSubobjectStream as *const u8;
    let size = stream.SizeInBytes;
    if base.is_null() || size == 0 || size > 1 << 20 {
        return None;
    }
    let mut d = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
        BlendState: D3D12_BLEND_DESC::default(),
        SampleMask: u32::MAX,
        RasterizerState: D3D12_RASTERIZER_DESC { FillMode: D3D12_FILL_MODE_SOLID, CullMode: D3D12_CULL_MODE_BACK, DepthClipEnable: true.into(), ..Default::default() },
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        ..Default::default()
    };
    for t in d.BlendState.RenderTarget.iter_mut() {
        t.RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8;
        (t.SrcBlend, t.DestBlend, t.BlendOp, t.SrcBlendAlpha, t.DestBlendAlpha, t.BlendOpAlpha, t.LogicOp) = (D3D12_BLEND_ONE, D3D12_BLEND_ZERO, D3D12_BLEND_OP_ADD, D3D12_BLEND_ONE, D3D12_BLEND_ZERO, D3D12_BLEND_OP_ADD, D3D12_LOGIC_OP_NOOP);
    }
    let mut at = 0usize;
    let align = |x: usize, a: usize| (x + a - 1) & !(a - 1);
    while at + 4 <= size {
        let kind = std::ptr::read_unaligned(base.add(at) as *const u32);
        // (inner size, inner alignment)
        let (inner, ia): (usize, usize) = match kind {
            0 => (8, 8),
            1..=6 => (16, 8),
            7 => (32, 8),
            8 => (std::mem::size_of::<D3D12_BLEND_DESC>(), 4),
            9 | 13 | 14 | 16 | 18 | 20 => (4, 4),
            10 => (std::mem::size_of::<D3D12_RASTERIZER_DESC>(), 4),
            11 => (std::mem::size_of::<D3D12_DEPTH_STENCIL_DESC>(), 4),
            12 => (16, 8),
            15 => (std::mem::size_of::<D3D12_RT_FORMAT_ARRAY>(), 4),
            17 => (8, 4),
            19 => (16, 8),
            21 => (std::mem::size_of::<D3D12_DEPTH_STENCIL_DESC1>(), 4),
            _ => return None,
        };
        let p = at + align(4, ia);
        if p + inner > size {
            return None;
        }
        let ptr = base.add(p);
        match kind {
            0 => d.pRootSignature = std::mem::transmute_copy(&std::ptr::read_unaligned(ptr as *const *mut c_void)),
            1 => d.VS = std::ptr::read_unaligned(ptr as *const D3D12_SHADER_BYTECODE),
            2 => d.PS = std::ptr::read_unaligned(ptr as *const D3D12_SHADER_BYTECODE),
            3 => d.DS = std::ptr::read_unaligned(ptr as *const D3D12_SHADER_BYTECODE),
            4 => d.HS = std::ptr::read_unaligned(ptr as *const D3D12_SHADER_BYTECODE),
            5 => d.GS = std::ptr::read_unaligned(ptr as *const D3D12_SHADER_BYTECODE),
            6 => return None,
            7 => d.StreamOutput = std::ptr::read_unaligned(ptr as *const D3D12_STREAM_OUTPUT_DESC),
            8 => d.BlendState = std::ptr::read_unaligned(ptr as *const D3D12_BLEND_DESC),
            9 => d.SampleMask = std::ptr::read_unaligned(ptr as *const u32),
            10 => d.RasterizerState = std::ptr::read_unaligned(ptr as *const D3D12_RASTERIZER_DESC),
            11 => d.DepthStencilState = std::ptr::read_unaligned(ptr as *const D3D12_DEPTH_STENCIL_DESC),
            12 => d.InputLayout = std::ptr::read_unaligned(ptr as *const D3D12_INPUT_LAYOUT_DESC),
            13 => d.IBStripCutValue = D3D12_INDEX_BUFFER_STRIP_CUT_VALUE(std::ptr::read_unaligned(ptr as *const i32)),
            14 => d.PrimitiveTopologyType = D3D12_PRIMITIVE_TOPOLOGY_TYPE(std::ptr::read_unaligned(ptr as *const i32)),
            15 => {
                let a = std::ptr::read_unaligned(ptr as *const D3D12_RT_FORMAT_ARRAY);
                d.RTVFormats = a.RTFormats;
                d.NumRenderTargets = a.NumRenderTargets;
            }
            16 => d.DSVFormat = DXGI_FORMAT(std::ptr::read_unaligned(ptr as *const i32)),
            17 => d.SampleDesc = std::ptr::read_unaligned(ptr as *const DXGI_SAMPLE_DESC),
            18 => d.NodeMask = std::ptr::read_unaligned(ptr as *const u32),
            19 => {}
            20 => d.Flags = D3D12_PIPELINE_STATE_FLAGS(std::ptr::read_unaligned(ptr as *const i32)),
            21 => {
                let s = std::ptr::read_unaligned(ptr as *const D3D12_DEPTH_STENCIL_DESC1);
                d.DepthStencilState = D3D12_DEPTH_STENCIL_DESC { DepthEnable: s.DepthEnable, DepthWriteMask: s.DepthWriteMask, DepthFunc: s.DepthFunc, StencilEnable: s.StencilEnable, StencilReadMask: s.StencilReadMask, StencilWriteMask: s.StencilWriteMask, FrontFace: s.FrontFace, BackFace: s.BackFace };
            }
            _ => return None,
        }
        at = align(p + inner, 8);
    }
    (!d.PS.pShaderBytecode.is_null()).then_some(d)
}

unsafe fn consider_stream(device: *mut c_void, stream: *const D3D12_PIPELINE_STATE_STREAM_DESC, made: *mut c_void, how: &str) {
    if stream.is_null() || made.is_null() {
        return;
    }
    match stream_as_graphics(&*stream) {
        Some(d) => {
            consider(device, &d, made, how);
            // (the description borrowed the stream's root signature pointer without a reference of its own)
            std::mem::forget(d);
        }
        None => {
            // (a stream we cannot read: if its pixel shader is the shafts', say so)
            let base = (*stream).pPipelineStateSubobjectStream as *const u8;
            if !base.is_null() {
                let bytes = std::slice::from_raw_parts(base, (*stream).SizeInBytes.min(1 << 20));
                if bytes.windows(16).any(|w| w == SHAFT_HASH) {
                    rlog!("render: (IMPL_ER80b) a light-shaft pipeline {made:p} came from a stream ({how}) we cannot read: no twin");
                }
            }
        }
    }
}

unsafe extern "system" fn create_stream_hook(this: *mut c_void, desc: *const D3D12_PIPELINE_STATE_STREAM_DESC, riid: *const GUID, out: *mut *mut c_void) -> HRESULT {
    let original: CreateStreamFn = std::mem::transmute(O_CREATE_STREAM.load(Ordering::Relaxed));
    let hr = original(this, desc, riid, out);
    if hr.is_ok() && !out.is_null() && !is_ours() && !DEAD.load(Ordering::Relaxed) && !riid.is_null() && *riid == ID3D12PipelineState::IID {
        let _ = std::panic::catch_unwind(|| consider_stream(this, desc, *out, "ID3D12Device2::CreatePipelineState"));
    }
    hr
}

unsafe extern "system" fn load_stream_hook(this: *mut c_void, name: PCWSTR, desc: *const D3D12_PIPELINE_STATE_STREAM_DESC, riid: *const GUID, out: *mut *mut c_void) -> HRESULT {
    let original: LoadStreamFn = std::mem::transmute(O_LOAD_STREAM.load(Ordering::Relaxed));
    let hr = original(this, name, desc, riid, out);
    if hr.is_ok() && !out.is_null() && !is_ours() && !DEAD.load(Ordering::Relaxed) && !riid.is_null() && *riid == ID3D12PipelineState::IID {
        let _ = std::panic::catch_unwind(|| {
            let raw = this;
            if let Some(library) = ID3D12PipelineLibrary::from_raw_borrowed(&raw) {
                let mut device: Option<ID3D12Device> = None;
                if let (Ok(()), Some(device)) = (library.GetDevice(&mut device), device.as_ref()) {
                    consider_stream(device.as_raw(), desc, *out, "ID3D12PipelineLibrary1::LoadPipeline");
                }
            }
        });
    }
    hr
}

unsafe extern "system" fn create_rtv_hook(this: *mut c_void, resource: *mut c_void, desc: *const D3D12_RENDER_TARGET_VIEW_DESC, handle: D3D12_CPU_DESCRIPTOR_HANDLE) {
    if !resource.is_null() && WANTED.load(Ordering::Relaxed) && !is_ours() && !DEAD.load(Ordering::Relaxed) {
        let _ = std::panic::catch_unwind(|| {
            let raw = resource;
            if let Some(r) = ID3D12Resource::from_raw_borrowed(&raw) {
                let d = r.GetDesc();
                let mip = if !desc.is_null() && (*desc).ViewDimension == D3D12_RTV_DIMENSION_TEXTURE2D { (*desc).Anonymous.Texture2D.MipSlice } else { 0 };
                let size = (((d.Width >> mip).max(1)) as u32, (d.Height >> mip).max(1));
                if let Ok(mut guard) = RTVS.lock() {
                    let map = guard.get_or_insert_with(HashMap::new);
                    if map.len() > 65536 {
                        map.clear();
                    }
                    map.insert(handle.ptr, size);
                }
            }
        });
    }
    let original: CreateRtvFn = std::mem::transmute(O_CREATE_RTV.load(Ordering::Relaxed));
    original(this, resource, desc, handle)
}

unsafe extern "system" fn stencil_ref_hook(this: *mut c_void, value: u32) {
    if !is_ours() && following() {
        with_list(this as usize, |s| s.stencil = Some(value));
    }
    let original: StencilRefFn = std::mem::transmute(O_STENCIL_REF.load(Ordering::Relaxed));
    original(this, value)
}

/// The hooks (after hook::install: the command-list vtable is known). Not fatal: without them the shafts stay as they are.
pub fn install() -> Result<(), String> {
    let list_vtable = LIST_VTABLE.load(Ordering::SeqCst);
    if list_vtable == 0 {
        return Err("no command-list vtable".to_string());
    }
    // SAFETY: throw-away objects of our own on the default adapter; the entries patched have the hooks' signatures.
    unsafe {
        let mut device: Option<ID3D12Device> = None;
        D3D12CreateDevice(None, D3D_FEATURE_LEVEL_11_0, &mut device).map_err(|e| format!("D3D12CreateDevice: {e}"))?;
        let device = device.ok_or("no device")?;
        let vtable = *(device.as_raw() as *const usize);
        patch(list_vtable, entry!(ID3D12GraphicsCommandList_Vtbl, OMSetStencilRef), stencil_ref_hook as *const () as usize, &O_STENCIL_REF, "ID3D12GraphicsCommandList::OMSetStencilRef")?;
        patch(vtable, entry!(ID3D12Device_Vtbl, CreateGraphicsPipelineState), create_graphics_hook as *const () as usize, &O_CREATE_GRAPHICS, "ID3D12Device::CreateGraphicsPipelineState")?;
        patch(vtable, entry!(ID3D12Device_Vtbl, CreateRenderTargetView), create_rtv_hook as *const () as usize, &O_CREATE_RTV, "ID3D12Device::CreateRenderTargetView")?;
        let mut extra = Vec::new();
        if device.cast::<ID3D12Device2>().is_ok() {
            patch(vtable, entry!(ID3D12Device2_Vtbl, CreatePipelineState), create_stream_hook as *const () as usize, &O_CREATE_STREAM, "ID3D12Device2::CreatePipelineState")?;
            extra.push("CreatePipelineState");
        }
        if let Ok(d1) = device.cast::<ID3D12Device1>() {
            if let Ok(library) = d1.CreatePipelineLibrary::<ID3D12PipelineLibrary>(&[]) {
                let lv = *(library.as_raw() as *const usize);
                patch(lv, entry!(ID3D12PipelineLibrary_Vtbl, LoadGraphicsPipeline), load_graphics_hook as *const () as usize, &O_LOAD_GRAPHICS, "ID3D12PipelineLibrary::LoadGraphicsPipeline")?;
                extra.push("LoadGraphicsPipeline");
                if library.cast::<ID3D12PipelineLibrary1>().is_ok() {
                    patch(lv, entry!(ID3D12PipelineLibrary1_Vtbl, LoadPipeline), load_stream_hook as *const () as usize, &O_LOAD_STREAM, "ID3D12PipelineLibrary1::LoadPipeline")?;
                    extra.push("LoadPipeline");
                }
            }
        }
        rlog!("render: (IMPL_ER80b) the light-shaft watch is installed (CreateGraphicsPipelineState, CreateRenderTargetView, OMSetStencilRef, {})", extra.join(", "));
    }
    Ok(())
}

/// The status line part.
pub fn status() -> String {
    let (d, m, l) = (DRAWS.swap(0, Ordering::Relaxed), MASKED.swap(0, Ordering::Relaxed), LEFT.swap(0, Ordering::Relaxed));
    let why = WHY.lock().map(|w| *w).unwrap_or("?");
    if !WANTED.load(Ordering::Relaxed) {
        return "OFF (r.shaft_mask 0: the game's light shafts are added over him as before)".to_string();
    }
    format!(
        "{} game pipeline(s) seen being made, {} with twins; {d} composite draw(s) this second: {m} masked off his pixels, {l} left as they are (latest reason: {})",
        SEEN_PSOS.load(Ordering::Relaxed),
        TWIN_COUNT.load(Ordering::Relaxed),
        if l == 0 { "none this second" } else { why }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (IMPL_ER80b) the game's own LightShaft_CompositePS is recognised from its bytes (build/sora_tmp/er/er80/game/post,
    /// extracted read-only from gxposteffect.shaderbnd), another shader is not
    #[test]
    fn the_light_shaft_composite_is_recognised_by_its_hash() {
        let dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../build/sora_tmp/er/er80/game/post"));
        let (Ok(shaft), Ok(other)) = (std::fs::read(dir.join("LightShaft_CompositePS.ppo")), std::fs::read(dir.join("LightShaft_DetectSkyPS.ppo"))) else {
            eprintln!("(the extracted game shaders are not here: skipped)");
            return;
        };
        assert!(is_shaft(&shaft));
        assert!(!is_shaft(&other));
        assert!(!is_shaft(b"not a shader at all, not at all"));
    }

    #[test]
    fn the_twin_only_adds_the_stencil_test() {
        let mut d = D3D12_GRAPHICS_PIPELINE_STATE_DESC { NumRenderTargets: 1, SampleMask: u32::MAX, ..Default::default() };
        d.RTVFormats[0] = DXGI_FORMAT_R11G11B10_FLOAT;
        d.BlendState.RenderTarget[0].BlendEnable = true.into();
        let t = twin_desc(&d, DXGI_FORMAT_D32_FLOAT_S8X24_UINT);
        assert_eq!(t.DSVFormat, DXGI_FORMAT_D32_FLOAT_S8X24_UINT);
        assert!(t.DepthStencilState.StencilEnable.as_bool() && !t.DepthStencilState.DepthEnable.as_bool());
        assert_eq!(t.DepthStencilState.StencilWriteMask, 0);
        assert_eq!(t.DepthStencilState.FrontFace.StencilFunc, D3D12_COMPARISON_FUNC_EQUAL);
        assert_eq!(t.RTVFormats[0], DXGI_FORMAT_R11G11B10_FLOAT);
        assert!(t.BlendState.RenderTarget[0].BlendEnable.as_bool());
        std::mem::forget(t);
        std::mem::forget(d);
    }
}
