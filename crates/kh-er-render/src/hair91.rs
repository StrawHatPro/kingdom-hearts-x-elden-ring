//! (ER91, the user: "the tips of Sora's hair is also still broken, it needs to look like this image, anything else is
//! unacceptable") THE HAIR'S OWN 4x PASS at the game's velocity split (after its forward stage, before its TAA).
//!
//! Why: ER91's shaders give the strands their true, continuous, supersampled alpha (hair.rs, er91/tools/hair91.js), but
//! the hair CARDS' own edges are triangle edges: drawn into the game's single-sample target they step, and only the game's
//! TAA would smooth them. Here every hair draw is drawn once more into our own 4x targets in a box around his head:
//! 1. the 4x depth from the scene depth (his body, the world, the early hair core; pushed back a little so that the core's
//!    own fragments pass) and the same into a 1x init texture (`h91init` / `h91init1`);
//! 2. the solid half of every strand (coverage >= 1/2, `<set>_h91m`, depth written) and the thin half over it (`_h91n`),
//!    premultiplied, with his motion vector and depth x coverage in a second target;
//! 3. the four samples resolved and laid over the scene (`h91comp`): where the early core took the pixel (its colour hid
//!    what lay behind), the background is the nearest hair-free scene around it;
//! 4. where the hair covers at least half of a pixel the core did not take: his motion vector, depth and TAA class
//!    (`h91velo`, the game's velocity target and the scene depth / stencil), so the TAA keeps the strands with him.
//! The result does not depend on the game's TAA: the same picture every frame (the jitter only moves it as the game's).
//! `r.hair_msaa = 0`: off (ER91's fringe at the split as ER87's); `r.hair_msaa_far` (m, default 8): only nearer than this.
//! Any failure in a frame: that frame's fringe as before (counted, with the latest reason, in the status line).

use std::collections::HashMap;
use std::ffi::c_void;
use std::mem::ManuallyDrop;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use windows::core::Interface;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;

static ON: AtomicBool = AtomicBool::new(true);
static FAR: AtomicU32 = AtomicU32::new(0x4100_0000); // 8.0
static DRAWN: AtomicU32 = AtomicU32::new(0);
static MISSED: AtomicU32 = AtomicU32::new(0);
static VELO: AtomicU32 = AtomicU32::new(0);
static SNAP: AtomicU32 = AtomicU32::new(0);
static TILES: AtomicU32 = AtomicU32::new(0);
static WHY: std::sync::Mutex<&'static str> = std::sync::Mutex::new("");

/// The host's key (kh-er `r.hair_msaa`, default 1).
pub fn set_on(on: bool) {
    ON.store(on, Ordering::Relaxed);
}

pub fn on() -> bool {
    ON.load(Ordering::Relaxed)
}

/// The host's key (kh-er `r.hair_msaa_far`, metres, default 8).
pub fn set_far(metres: f32) {
    FAR.store(metres.max(0.0).to_bits(), Ordering::Relaxed);
}

pub fn far() -> f32 {
    f32::from_bits(FAR.load(Ordering::Relaxed))
}

pub(crate) fn missed(why: &'static str) {
    MISSED.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut w) = WHY.lock() {
        *w = why;
    }
}

/// The status line's part (hair.rs `status`).
pub(crate) fn status() -> String {
    let (d, m, v, s, t) = (DRAWN.swap(0, Ordering::Relaxed), MISSED.swap(0, Ordering::Relaxed), VELO.swap(0, Ordering::Relaxed), SNAP.swap(0, Ordering::Relaxed), TILES.swap(0, Ordering::Relaxed));
    let why = WHY.lock().map(|w| *w).unwrap_or("");
    format!("; (ER91) the 4x hair pass (r.hair_msaa {}, within {:.0} m): {d} frame(s) ({t} tile(s); {s} with every hair layer: the snapshot; {v} with his motion vectors), {m} not{}", on() as u8, far(), if m > 0 { format!(" (latest: {why})") } else { String::new() })
}

pub(crate) const COLOR: DXGI_FORMAT = DXGI_FORMAT_R16G16B16A16_FLOAT;
pub(crate) const DEPTH: DXGI_FORMAT = DXGI_FORMAT_D32_FLOAT;
const DEPTH_TYPELESS: DXGI_FORMAT = DXGI_FORMAT_R32_TYPELESS;
const INIT: DXGI_FORMAT = DXGI_FORMAT_R32_FLOAT;
pub(crate) const SAMPLES: u32 = 4;
/// The kit's targets are one TILE large at most; a larger box is drawn tile by tile (a 4K close-up: up to 3 x 3). At most
/// about 1280 x 1024 x (2 x 32 + 16 + 4 + 8) bytes = 120 MB of targets, made only when such a box comes (grown, never
/// shrunk), and the full-size mark (8 bytes a pixel).
pub(crate) const TILE_W: u32 = 1280;
pub(crate) const TILE_H: u32 = 1024;
pub(crate) const MAX_TILES: usize = 9;
/// One tile's parameter block (er_char.wgsl `H91Params`, a constant buffer: 256 bytes).
pub(crate) const PARAMS_BYTES: usize = 256;

/// The box cut into tiles of at most TILE_W x TILE_H (row by row).
pub(crate) fn tiles(b: [u32; 4]) -> Vec<[u32; 4]> {
    let [x, y, w, h] = b;
    let mut out = Vec::new();
    let (nx, ny) = (w.div_ceil(TILE_W), h.div_ceil(TILE_H));
    for j in 0..ny {
        for i in 0..nx {
            let (tx, ty) = (x + i * w / nx, y + j * h / ny);
            let (tx1, ty1) = (x + (i + 1) * w / nx, y + (j + 1) * h / ny);
            out.push([tx, ty, tx1 - tx, ty1 - ty]);
        }
    }
    out
}
/// The depth tolerance (relative): without the snapshot, how far the 4x depth is pushed back from the scene depth (the
/// early core's own fragments pass it); with it, how much nearer than the hair's front his own surface must be to hide it
/// (3 mm at a metre), and how close the scene depth must be to the hair's to be its early core.
pub(crate) const PUSH: f32 = 0.003;
/// From this coverage a pixel the early core did not take carries his motion vector, depth and TAA class.
pub(crate) const VELO_FROM: f32 = 0.5;
/// The background search radius (pixels) for a pixel the early core took.
pub(crate) const RADIUS: f32 = 2.0;
/// His head's reach for the box: a ball of this radius (cm) around a point this far above the head joint. Measured on the
/// packs' hair in the bind pose (er91/tools/hairradius.js): the farthest hair vertex from `atama` is 25 cm (KH3, KH1, San
/// Fransokyo, Quadratum), 31 cm (KH2), 15-17 cm (Caribbean, Monstropolis); from the ball's centre less.
pub(crate) const REACH_CM: f32 = 38.0;
pub(crate) const ABOVE_CM: f32 = 8.0;

/// The box (x, y, width, height in scene pixels) that holds the ball of `REACH_CM` around `centre` (game space, cm) seen
/// through `clip` (clip from game space), padded, inside the target; the whole target when the ball reaches behind the
/// camera; None when it is off the target.
pub(crate) fn head_box(clip: &glam::Mat4, centre: glam::Vec3, width: u32, height: u32) -> Option<[u32; 4]> {
    let (mut lo, mut hi) = (glam::Vec2::splat(f32::MAX), glam::Vec2::splat(f32::MIN));
    // (26 points on the ball, a little outside it: the box of their pictures holds the ball's)
    for k in 0..27 {
        let d = glam::Vec3::new((k % 3) as f32 - 1.0, ((k / 3) % 3) as f32 - 1.0, (k / 9) as f32 - 1.0);
        if d == glam::Vec3::ZERO {
            continue;
        }
        let corner = centre + d.normalize() * REACH_CM * 1.06;
        let c = *clip * corner.extend(1.0);
        if c.w <= 1.0 {
            return Some([0, 0, width, height]);
        }
        let p = glam::Vec2::new((c.x / c.w * 0.5 + 0.5) * width as f32, (0.5 - c.y / c.w * 0.5) * height as f32);
        lo = lo.min(p);
        hi = hi.max(p);
    }
    let pad = 8.0;
    let x0 = (lo.x - pad).floor().max(0.0);
    let y0 = (lo.y - pad).floor().max(0.0);
    let x1 = (hi.x + pad).ceil().min(width as f32);
    let y1 = (hi.y + pad).ceil().min(height as f32);
    if !(x1 > x0 && y1 > y0) {
        return None;
    }
    Some([x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32])
}

/// The parameter block of the screen passes (er_char.wgsl `H91Params`), 256 bytes. `snapshot`: the 4x depth starts from
/// the scene depth without him (the early depth's snapshot).
/// One block per tile of the box (`tiles`), each PARAMS_BYTES long.
pub(crate) fn params(b: [u32; 4], snapshot: bool) -> Vec<f32> {
    let per = PARAMS_BYTES / 4;
    let tiles = tiles(b);
    let mut p = vec![0.0f32; per * tiles.len().max(1)];
    for (k, t) in tiles.iter().enumerate() {
        p[k * per..k * per + 4].copy_from_slice(&[t[0] as f32, t[1] as f32, t[2] as f32, t[3] as f32]);
        p[k * per + 4..k * per + 8].copy_from_slice(&[PUSH, VELO_FROM, RADIUS, snapshot as u8 as f32]);
    }
    p
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

unsafe fn texture(device: &ID3D12Device, w: u32, h: u32, format: DXGI_FORMAT, samples: u32, flags: D3D12_RESOURCE_FLAGS, state: D3D12_RESOURCE_STATES, clear: Option<D3D12_CLEAR_VALUE>) -> windows::core::Result<ID3D12Resource> {
    let desc = D3D12_RESOURCE_DESC { Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D, Width: w as u64, Height: h, DepthOrArraySize: 1, MipLevels: 1, Format: format, SampleDesc: DXGI_SAMPLE_DESC { Count: samples, Quality: 0 }, Flags: flags, ..Default::default() };
    let props = D3D12_HEAP_PROPERTIES { Type: D3D12_HEAP_TYPE_DEFAULT, ..Default::default() };
    let mut out: Option<ID3D12Resource> = None;
    device.CreateCommittedResource(&props, D3D12_HEAP_FLAG_NONE, &desc, state, clear.as_ref().map(|c| c as *const _), &mut out)?;
    out.ok_or_else(|| windows::core::Error::from_hresult(windows::Win32::Foundation::E_FAIL))
}

/// The screen passes' shaders (PACK/materials/er: h91.vs, h91init, h91init1, h91comp, h91velo).
pub(crate) struct Screens {
    vs: Vec<u8>,
    init: Vec<u8>,
    init1: Vec<u8>,
    comp: Vec<u8>,
    velo: Vec<u8>,
}

impl Screens {
    pub(crate) fn read(dir: &Path) -> Option<Screens> {
        let r = |n: &str| std::fs::read(dir.join(n)).ok();
        Some(Screens { vs: r("h91.vs.dxil")?, init: r("h91init.ps.dxil")?, init1: r("h91init1.ps.dxil")?, comp: r("h91comp.ps.dxil")?, velo: r("h91velo.ps.dxil")? })
    }
}

/// The resting states of the kit's resources between frames: the 4x pair and the init texture RENDER_TARGET, the 4x depth
/// DEPTH_WRITE, the scene copy COPY_DEST.
struct Kit {
    w: u32,
    h: u32,
    col: ID3D12Resource,
    aux: ID3D12Resource,
    dep: ID3D12Resource,
    init: ID3D12Resource,
    copy: ID3D12Resource,
    copy_format: i32,
    /// (full size, 1x) where the early hair core is the scene's surface; RENDER_TARGET at rest
    mark: ID3D12Resource,
    mark_size: (u32, u32),
    srv: ID3D12DescriptorHeap,
    rtv: ID3D12DescriptorHeap,
    dsv: ID3D12DescriptorHeap,
    /// the depths whose views are at t0 (the snapshot, or the scene depth) and t6 (the scene depth) (pointers)
    depth_for: (usize, usize),
}

struct Pipes {
    scene_format: i32,
    dsv_format: i32,
    init: ID3D12PipelineState,
    init1: ID3D12PipelineState,
    comp: ID3D12PipelineState,
    velo: Option<ID3D12PipelineState>,
}

/// The 4x pass's objects (gfx.rs `GlowKit`); the per-slot hair states are kept beside it by key (gfx.rs).
#[derive(Default)]
pub(crate) struct State {
    kit: Option<Kit>,
    pipes: Option<Pipes>,
    pipes_failed: Option<(i32, i32)>,
    screens: Option<Screens>,
    screens_read: bool,
    /// kits replaced (a larger box, another scene format), dropped once the GPU passed the fence value
    old: Vec<(u64, Kit)>,
}

/// A hair slot's states of the 4x pass: the solid and thin halves into the 4x pair, and the MARK of where its early core is
/// the scene's surface (the slot's own core shader, 1x, depth EQUAL against the scene depth: its own colour, exactly what
/// the early core drew there).
pub(crate) struct HairStates {
    pub solid: ID3D12PipelineState,
    pub thin: ID3D12PipelineState,
    pub mark: Option<ID3D12PipelineState>,
}

/// The mark's format (a full-size 1x target: the early core's own colour where it is the surface at this pixel's centre,
/// alpha 1; the composite keeps what the game's forward stage added over it: scene - mark).
pub(crate) const MARK: DXGI_FORMAT = DXGI_FORMAT_R16G16B16A16_FLOAT;

/// The 4x hair states, one set per slot (None: not a hair slot with 4x shaders): solid (depth GREATER_EQUAL, written) and
/// thin (depth GREATER, not written), both premultiplied over (ONE, INV_SRC_ALPHA) into the 4x pair; and the mark (`core`:
/// the slot's colour core shader; `dsv`: the scene depth's format).
///
/// # Safety
/// Direct3D object creation with a live device and root signature.
#[allow(clippy::too_many_arguments)]
pub(crate) unsafe fn msaa_pipelines(device: &ID3D12Device, root: &ID3D12RootSignature, front_ccw: bool, cull: bool, dsv: DXGI_FORMAT, slots: &[(String, bool, bool, bool)], vertex: &[u8], msaa: &HashMap<String, (Vec<u8>, Vec<u8>)>, core: &HashMap<String, Vec<u8>>, elements: &[D3D12_INPUT_ELEMENT_DESC]) -> Result<Vec<Option<HairStates>>, String> {
    let mut out = Vec::with_capacity(slots.len());
    for (variant, two_sided, translucent, masked) in slots {
        let Some((solid, thin)) = msaa.get(variant).filter(|_| *masked && !*translucent) else {
            out.push(None);
            continue;
        };
        let rasterizer = D3D12_RASTERIZER_DESC {
            FillMode: D3D12_FILL_MODE_SOLID,
            CullMode: if *two_sided || !cull { D3D12_CULL_MODE_NONE } else { D3D12_CULL_MODE_BACK },
            FrontCounterClockwise: front_ccw.into(),
            DepthClipEnable: true.into(),
            ..Default::default()
        };
        // the mark: the core shader's colour where it keeps the pixel and its depth is the scene's
        let mark = core.get(variant).and_then(|pixel| {
            let mut blend = D3D12_BLEND_DESC::default();
            blend.RenderTarget[0].RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8;
            let mut formats = [DXGI_FORMAT_UNKNOWN; 8];
            formats[0] = MARK;
            let desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
                pRootSignature: std::mem::transmute_copy(root),
                VS: D3D12_SHADER_BYTECODE { pShaderBytecode: vertex.as_ptr() as *const c_void, BytecodeLength: vertex.len() },
                PS: D3D12_SHADER_BYTECODE { pShaderBytecode: pixel.as_ptr() as *const c_void, BytecodeLength: pixel.len() },
                BlendState: blend,
                SampleMask: u32::MAX,
                RasterizerState: rasterizer,
                DepthStencilState: D3D12_DEPTH_STENCIL_DESC { DepthEnable: true.into(), DepthWriteMask: D3D12_DEPTH_WRITE_MASK_ZERO, DepthFunc: D3D12_COMPARISON_FUNC_EQUAL, StencilEnable: false.into(), ..Default::default() },
                InputLayout: D3D12_INPUT_LAYOUT_DESC { pInputElementDescs: elements.as_ptr(), NumElements: elements.len() as u32 },
                PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
                NumRenderTargets: 1,
                RTVFormats: formats,
                DSVFormat: dsv,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                ..Default::default()
            };
            device.CreateGraphicsPipelineState(&desc).ok()
        });
        let make = |pixel: &[u8], write: bool| -> Result<ID3D12PipelineState, String> {
            let mut blend = D3D12_BLEND_DESC { IndependentBlendEnable: false.into(), ..Default::default() };
            let over = D3D12_RENDER_TARGET_BLEND_DESC {
                BlendEnable: true.into(),
                LogicOpEnable: false.into(),
                SrcBlend: D3D12_BLEND_ONE,
                DestBlend: D3D12_BLEND_INV_SRC_ALPHA,
                BlendOp: D3D12_BLEND_OP_ADD,
                SrcBlendAlpha: D3D12_BLEND_ONE,
                DestBlendAlpha: D3D12_BLEND_INV_SRC_ALPHA,
                BlendOpAlpha: D3D12_BLEND_OP_ADD,
                LogicOp: D3D12_LOGIC_OP_NOOP,
                RenderTargetWriteMask: D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8,
            };
            blend.RenderTarget[0] = over;
            blend.RenderTarget[1] = over;
            let mut formats = [DXGI_FORMAT_UNKNOWN; 8];
            formats[0] = COLOR;
            formats[1] = COLOR;
            let desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
                // SAFETY: a borrowed copy of the interface pointer for the duration of the call.
                pRootSignature: std::mem::transmute_copy(root),
                VS: D3D12_SHADER_BYTECODE { pShaderBytecode: vertex.as_ptr() as *const c_void, BytecodeLength: vertex.len() },
                PS: D3D12_SHADER_BYTECODE { pShaderBytecode: pixel.as_ptr() as *const c_void, BytecodeLength: pixel.len() },
                BlendState: blend,
                SampleMask: u32::MAX,
                RasterizerState: D3D12_RASTERIZER_DESC {
                    FillMode: D3D12_FILL_MODE_SOLID,
                    CullMode: if *two_sided || !cull { D3D12_CULL_MODE_NONE } else { D3D12_CULL_MODE_BACK },
                    FrontCounterClockwise: front_ccw.into(),
                    DepthClipEnable: true.into(),
                    MultisampleEnable: true.into(),
                    ..Default::default()
                },
                DepthStencilState: D3D12_DEPTH_STENCIL_DESC {
                    DepthEnable: true.into(),
                    DepthWriteMask: if write { D3D12_DEPTH_WRITE_MASK_ALL } else { D3D12_DEPTH_WRITE_MASK_ZERO },
                    DepthFunc: if write { D3D12_COMPARISON_FUNC_GREATER_EQUAL } else { D3D12_COMPARISON_FUNC_GREATER },
                    StencilEnable: false.into(),
                    ..Default::default()
                },
                InputLayout: D3D12_INPUT_LAYOUT_DESC { pInputElementDescs: elements.as_ptr(), NumElements: elements.len() as u32 },
                PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
                NumRenderTargets: 2,
                RTVFormats: formats,
                DSVFormat: DEPTH,
                SampleDesc: DXGI_SAMPLE_DESC { Count: SAMPLES, Quality: 0 },
                ..Default::default()
            };
            device.CreateGraphicsPipelineState(&desc).map_err(|e| format!("CreateGraphicsPipelineState(hair 4x {variant}{}): {e}", if write { "" } else { ", thin" }))
        };
        out.push(Some(HairStates { solid: make(solid, true)?, thin: make(thin, false)?, mark }));
    }
    Ok(out)
}

unsafe fn screen(device: &ID3D12Device, root: &ID3D12RootSignature, vs: &[u8], ps: &[u8], rt: Option<DXGI_FORMAT>, dsv: Option<(DXGI_FORMAT, bool)>, samples: u32) -> windows::core::Result<ID3D12PipelineState> {
    let mut formats = [DXGI_FORMAT_UNKNOWN; 8];
    if let Some(f) = rt {
        formats[0] = f;
    }
    let mut blend = D3D12_BLEND_DESC::default();
    blend.RenderTarget[0].RenderTargetWriteMask = if rt == Some(COLOR) || rt == Some(INIT) || rt == Some(DXGI_FORMAT_R16G16_FLOAT) { D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8 } else { (D3D12_COLOR_WRITE_ENABLE_RED.0 | D3D12_COLOR_WRITE_ENABLE_GREEN.0 | D3D12_COLOR_WRITE_ENABLE_BLUE.0) as u8 };
    let stencil_op = D3D12_DEPTH_STENCILOP_DESC { StencilFailOp: D3D12_STENCIL_OP_KEEP, StencilDepthFailOp: D3D12_STENCIL_OP_KEEP, StencilPassOp: D3D12_STENCIL_OP_REPLACE, StencilFunc: D3D12_COMPARISON_FUNC_ALWAYS };
    let depth = match dsv {
        // the 4x depth's init: written everywhere
        Some((f, _)) if f == DEPTH => D3D12_DEPTH_STENCIL_DESC { DepthEnable: true.into(), DepthWriteMask: D3D12_DEPTH_WRITE_MASK_ALL, DepthFunc: D3D12_COMPARISON_FUNC_ALWAYS, StencilEnable: false.into(), ..Default::default() },
        // his depth (in front of what is there) and stencil into the scene's
        Some((_, stencil)) => D3D12_DEPTH_STENCIL_DESC { DepthEnable: true.into(), DepthWriteMask: D3D12_DEPTH_WRITE_MASK_ALL, DepthFunc: D3D12_COMPARISON_FUNC_GREATER_EQUAL, StencilEnable: stencil.into(), StencilReadMask: 0xFF, StencilWriteMask: 0xFF, FrontFace: stencil_op, BackFace: stencil_op },
        None => D3D12_DEPTH_STENCIL_DESC { DepthEnable: false.into(), StencilEnable: false.into(), ..Default::default() },
    };
    let desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
        pRootSignature: std::mem::transmute_copy(root),
        VS: D3D12_SHADER_BYTECODE { pShaderBytecode: vs.as_ptr() as *const c_void, BytecodeLength: vs.len() },
        PS: D3D12_SHADER_BYTECODE { pShaderBytecode: ps.as_ptr() as *const c_void, BytecodeLength: ps.len() },
        BlendState: blend,
        SampleMask: u32::MAX,
        RasterizerState: D3D12_RASTERIZER_DESC { FillMode: D3D12_FILL_MODE_SOLID, CullMode: D3D12_CULL_MODE_NONE, DepthClipEnable: false.into(), ..Default::default() },
        DepthStencilState: depth,
        PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
        NumRenderTargets: rt.is_some() as u32,
        RTVFormats: formats,
        DSVFormat: dsv.map_or(DXGI_FORMAT_UNKNOWN, |d| d.0),
        SampleDesc: DXGI_SAMPLE_DESC { Count: samples, Quality: 0 },
        ..Default::default()
    };
    device.CreateGraphicsPipelineState(&desc)
}

/// The scene depth's view format for reading its depth plane.
pub(crate) fn depth_read_format(resource_format: DXGI_FORMAT) -> Option<DXGI_FORMAT> {
    match resource_format {
        DXGI_FORMAT_R32G8X24_TYPELESS | DXGI_FORMAT_D32_FLOAT_S8X24_UINT => Some(DXGI_FORMAT_R32_FLOAT_X8X24_TYPELESS),
        DXGI_FORMAT_R24G8_TYPELESS | DXGI_FORMAT_D24_UNORM_S8_UINT => Some(DXGI_FORMAT_R24_UNORM_X8_TYPELESS),
        DXGI_FORMAT_R32_TYPELESS | DXGI_FORMAT_D32_FLOAT => Some(DXGI_FORMAT_R32_FLOAT),
        _ => None,
    }
}

/// The kit's size for a box: grown in steps of 256 (never past the target's size), at least the current kit.
pub(crate) fn kit_size(box_w: u32, box_h: u32, now: Option<(u32, u32)>, target: (u32, u32)) -> (u32, u32) {
    let up = |v: u32, most: u32| (v.div_ceil(256).max(1) * 256).min(most.max(v));
    let (w, h) = (up(box_w, target.0), up(box_h, target.1));
    match now {
        Some((a, b)) => (w.max(a), h.max(b)),
        None => (w, h),
    }
}

/// What a frame of the 4x pass needs (gfx.rs `record_hair`).
pub(crate) struct Frame<'a> {
    pub device: &'a ID3D12Device,
    pub root: &'a ID3D12RootSignature,
    pub list: &'a ID3D12GraphicsCommandList,
    pub sampler_heap: &'a ID3D12DescriptorHeap,
    pub sampler_index: u64,
    pub srv_heap: &'a ID3D12DescriptorHeap,
    pub table_start: u64,
    pub table_stride: u64,
    /// the scene target, its state at the split and its render target view (made by the caller)
    pub scene: &'a ID3D12Resource,
    pub scene_state: i32,
    pub scene_view: D3D12_CPU_DESCRIPTOR_HANDLE,
    /// the scene depth, its two planes' states at the split, its writable view, its depth-stencil format
    pub depth: &'a ID3D12Resource,
    pub depth_states: [i32; 2],
    pub depth_view: D3D12_CPU_DESCRIPTOR_HANDLE,
    pub dsv_format: DXGI_FORMAT,
    pub width: u32,
    pub height: u32,
    pub box_: [u32; 4],
    pub frame_constants: u64,
    pub params: u64,
    /// the game's velocity target and its state at the split (none: no motion vectors this frame)
    pub velocity: Option<(&'a ID3D12Resource, i32)>,
    /// his stencil (gfx.rs `his_stencil`), when written
    pub stencil: Option<u8>,
    /// the early depth's snapshot of this frame (the scene depth without him; PIXEL_SHADER_RESOURCE), when there
    pub snapshot: Option<&'a ID3D12Resource>,
    /// the GPU has passed this fence value (old kits are dropped)
    pub completed: u64,
    /// the fence value this frame's lists will signal
    pub fence_next: u64,
}

impl State {
    /// The screens and the kit for this frame (made or grown as needed). Err: why not.
    unsafe fn ready(&mut self, f: &Frame, dir: &Path) -> Result<(), &'static str> {
        self.old.retain(|(fence, _)| *fence > f.completed);
        if !self.screens_read {
            self.screens_read = true;
            self.screens = Screens::read(dir);
            if self.screens.is_none() {
                crate::rlog!("render: (ER91) the 4x hair pass's screen shaders (materials/er/h91*.dxil) are not in the pack: the fringe as before");
            }
        }
        let Some(screens) = self.screens.as_ref() else { return Err("its screen shaders (h91*.dxil) are not in the pack") };
        // (the kit holds the largest tile of the box)
        let (bw, bh) = tiles(f.box_).iter().fold((0, 0), |(w, h), t| (w.max(t[2]), h.max(t[3])));
        let scene_format = f.scene.GetDesc().Format;
        let depth_format = f.depth.GetDesc().Format;
        let Some(read_format) = depth_read_format(depth_format) else { return Err("the scene depth's format cannot be read") };
        let need = self.kit.as_ref().map(|k| (k.w, k.h, k.copy_format));
        let mark_size = self.kit.as_ref().map(|k| k.mark_size);
        if need.is_none_or(|(w, h, cf)| w < bw || h < bh || cf != scene_format.0) || mark_size != Some((f.width, f.height)) {
            let (w, h) = kit_size(bw, bh, need.filter(|n| n.2 == scene_format.0).map(|n| (n.0, n.1)), (f.width, f.height));
            let made = (|| -> windows::core::Result<Kit> {
                let zero = |format: DXGI_FORMAT| D3D12_CLEAR_VALUE { Format: format, Anonymous: D3D12_CLEAR_VALUE_0 { Color: [0.0; 4] } };
                let col = texture(f.device, w, h, COLOR, SAMPLES, D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET, D3D12_RESOURCE_STATE_RENDER_TARGET, Some(zero(COLOR)))?;
                let aux = texture(f.device, w, h, COLOR, SAMPLES, D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET, D3D12_RESOURCE_STATE_RENDER_TARGET, Some(zero(COLOR)))?;
                let dclear = D3D12_CLEAR_VALUE { Format: DEPTH, Anonymous: D3D12_CLEAR_VALUE_0 { DepthStencil: D3D12_DEPTH_STENCIL_VALUE { Depth: 0.0, Stencil: 0 } } };
                let dep = texture(f.device, w, h, DEPTH_TYPELESS, SAMPLES, D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL, D3D12_RESOURCE_STATE_DEPTH_WRITE, Some(dclear))?;
                let init = texture(f.device, w, h, INIT, 1, D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET, D3D12_RESOURCE_STATE_RENDER_TARGET, Some(zero(INIT)))?;
                let copy = texture(f.device, w, h, scene_format, 1, D3D12_RESOURCE_FLAG_NONE, D3D12_RESOURCE_STATE_COPY_DEST, None)?;
                let mark = texture(f.device, f.width, f.height, MARK, 1, D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET, D3D12_RESOURCE_STATE_RENDER_TARGET, Some(zero(MARK)))?;
                let slots = 23u32;
                let srv: ID3D12DescriptorHeap = f.device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, NumDescriptors: slots, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, NodeMask: 0 })?;
                let rtv: ID3D12DescriptorHeap = f.device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV, NumDescriptors: 5, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE, NodeMask: 0 })?;
                let dsv: ID3D12DescriptorHeap = f.device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_DSV, NumDescriptors: 2, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE, NodeMask: 0 })?;
                let ss = f.device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV) as usize;
                let rs = f.device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV) as usize;
                let s0 = srv.GetCPUDescriptorHandleForHeapStart();
                let at = |k: usize| D3D12_CPU_DESCRIPTOR_HANDLE { ptr: s0.ptr + k * ss };
                // (every entry a valid view: the init texture; then ours)
                for k in 0..slots as usize {
                    f.device.CreateShaderResourceView(&init, None, at(k));
                }
                let ms = |format: DXGI_FORMAT| D3D12_SHADER_RESOURCE_VIEW_DESC { Format: format, ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2DMS, Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING, ..Default::default() };
                f.device.CreateShaderResourceView(&col, Some(&ms(COLOR)), at(1));
                f.device.CreateShaderResourceView(&aux, Some(&ms(COLOR)), at(2));
                f.device.CreateShaderResourceView(&dep, Some(&ms(DXGI_FORMAT_R32_FLOAT)), at(3));
                f.device.CreateShaderResourceView(&copy, None, at(4));
                f.device.CreateShaderResourceView(&init, None, at(5));
                f.device.CreateShaderResourceView(&mark, None, at(7));
                let r0 = rtv.GetCPUDescriptorHandleForHeapStart();
                f.device.CreateRenderTargetView(&col, None, r0);
                f.device.CreateRenderTargetView(&aux, None, D3D12_CPU_DESCRIPTOR_HANDLE { ptr: r0.ptr + rs });
                f.device.CreateRenderTargetView(&init, None, D3D12_CPU_DESCRIPTOR_HANDLE { ptr: r0.ptr + 2 * rs });
                f.device.CreateRenderTargetView(&mark, None, D3D12_CPU_DESCRIPTOR_HANDLE { ptr: r0.ptr + 4 * rs });
                let dview = D3D12_DEPTH_STENCIL_VIEW_DESC { Format: DEPTH, ViewDimension: D3D12_DSV_DIMENSION_TEXTURE2DMS, Flags: D3D12_DSV_FLAG_NONE, ..Default::default() };
                f.device.CreateDepthStencilView(&dep, Some(&dview), dsv.GetCPUDescriptorHandleForHeapStart());
                Ok(Kit { w, h, col, aux, dep, init, copy, copy_format: scene_format.0, mark, mark_size: (f.width, f.height), srv, rtv, dsv, depth_for: (0, 0) })
            })();
            match made {
                Ok(kit) => {
                    crate::rlog!("render: (ER91) the 4x hair pass's targets: {w} x {h} (4x colour + motion, 4x depth, the init depth, a copy of the scene in format {}); the early core's mark {} x {}", scene_format.0, f.width, f.height);
                    if let Some(old) = self.kit.replace(kit) {
                        self.old.push((f.fence_next, old));
                    }
                }
                Err(e) => {
                    crate::rlog!("render: (ER91) the 4x hair pass's targets cannot be made ({e})");
                    return Err("its targets cannot be made");
                }
            }
        }
        let kit = self.kit.as_mut().expect("made above");
        // t0: the snapshot (the scene depth without him) or the scene depth; t6: the scene depth (their depth planes)
        let first = f.snapshot.unwrap_or(f.depth);
        let wanted = (first.as_raw() as usize, f.depth.as_raw() as usize);
        if kit.depth_for != wanted {
            let ss = f.device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV) as usize;
            let s0 = kit.srv.GetCPUDescriptorHandleForHeapStart();
            for (k, resource) in [(0usize, first), (6, f.depth)] {
                let Some(format) = depth_read_format(resource.GetDesc().Format) else { return Err("the scene depth's format cannot be read") };
                let view = D3D12_SHADER_RESOURCE_VIEW_DESC { Format: format, ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D, Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING, Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 { Texture2D: D3D12_TEX2D_SRV { MostDetailedMip: 0, MipLevels: 1, PlaneSlice: 0, ResourceMinLODClamp: 0.0 } } };
                f.device.CreateShaderResourceView(resource, Some(&view), D3D12_CPU_DESCRIPTOR_HANDLE { ptr: s0.ptr + k * ss });
            }
            // (the scene depth's read-only view, for the mark's EQUAL test)
            let ro = D3D12_DEPTH_STENCIL_VIEW_DESC { Format: f.dsv_format, ViewDimension: D3D12_DSV_DIMENSION_TEXTURE2D, Flags: D3D12_DSV_FLAG_READ_ONLY_DEPTH | D3D12_DSV_FLAG_READ_ONLY_STENCIL, ..Default::default() };
            let ds = f.device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_DSV) as usize;
            f.device.CreateDepthStencilView(f.depth, Some(&ro), D3D12_CPU_DESCRIPTOR_HANDLE { ptr: kit.dsv.GetCPUDescriptorHandleForHeapStart().ptr + ds });
            kit.depth_for = wanted;
        }
        let _ = read_format;
        let formats = (scene_format.0, f.dsv_format.0);
        if self.pipes.as_ref().map(|p| (p.scene_format, p.dsv_format)) != Some(formats) {
            if self.pipes_failed == Some(formats) {
                return Err("its screen pipeline states failed");
            }
            let made = (|| -> windows::core::Result<Pipes> {
                Ok(Pipes {
                    scene_format: formats.0,
                    dsv_format: formats.1,
                    init: screen(f.device, f.root, &screens.vs, &screens.init, None, Some((DEPTH, false)), SAMPLES)?,
                    init1: screen(f.device, f.root, &screens.vs, &screens.init1, Some(INIT), None, 1)?,
                    comp: screen(f.device, f.root, &screens.vs, &screens.comp, Some(scene_format), None, 1)?,
                    velo: screen(f.device, f.root, &screens.vs, &screens.velo, Some(DXGI_FORMAT_R16G16_FLOAT), Some((f.dsv_format, true)), 1).ok(),
                })
            })();
            match made {
                Ok(p) => {
                    crate::rlog!("render: (ER91) the 4x hair pass's screen states for scene format {} / depth {}{}", formats.0, formats.1, if p.velo.is_none() { " (no motion-vector state: the strands' ends carry the background's)" } else { "" });
                    self.pipes = Some(p);
                }
                Err(e) => {
                    crate::rlog!("render: (ER91) the 4x hair pass's screen states cannot be made ({e})");
                    self.pipes_failed = Some(formats);
                    return Err("its screen pipeline states failed");
                }
            }
        }
        Ok(())
    }

    /// The 4x pass of this frame into `f.list` (see the module text). `draws` / `set`: the hair draws (hair.rs `HairDraw`)
    /// and the slots' 4x states; `model`: a model's buffers. Ok: drawn (the caller then draws no fringe). Err: nothing was
    /// recorded, why.
    ///
    /// # Safety
    /// `f.list` is open; every resource in `f` is alive for the list's execution.
    pub(crate) unsafe fn record(&mut self, f: &Frame, dir: &Path, draws: &[crate::hair::HairDraw], set: &[Option<HairStates>], model: &dyn Fn(usize) -> Option<(D3D12_VERTEX_BUFFER_VIEW, D3D12_INDEX_BUFFER_VIEW)>) -> Result<(), &'static str> {
        if draws.iter().all(|d| set.get(d.1).is_none_or(|s| s.is_none())) {
            return Err("no hair draw has its 4x states");
        }
        let tiles = tiles(f.box_);
        if tiles.is_empty() || tiles.len() > MAX_TILES {
            return Err("the box around his head needs more tiles than the pass has (a picture larger than 4K)");
        }
        self.ready(f, dir)?;
        let (Some(kit), Some(pipes)) = (self.kit.as_ref(), self.pipes.as_ref()) else { return Err("not ready") };
        let list = f.list;
        let [bx, by, bw, bh] = f.box_;
        let rs = f.device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV) as usize;
        let r0 = kit.rtv.GetCPUDescriptorHandleForHeapStart();
        let (col_rtv, aux_rtv, init_rtv) = (r0, D3D12_CPU_DESCRIPTOR_HANDLE { ptr: r0.ptr + rs }, D3D12_CPU_DESCRIPTOR_HANDLE { ptr: r0.ptr + 2 * rs });
        let kit_dsv = kit.dsv.GetCPUDescriptorHandleForHeapStart();
        let scene_state = D3D12_RESOURCE_STATES(f.scene_state);
        let read = D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE | D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE;
        let depth_read = D3D12_RESOURCE_STATE_DEPTH_READ | D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE;
        let full_view = D3D12_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: f.width as f32, Height: f.height as f32, MinDepth: 0.0, MaxDepth: 1.0 };
        let rect = |x: u32, y: u32, w: u32, h: u32| windows::Win32::Foundation::RECT { left: x as i32, top: y as i32, right: (x + w) as i32, bottom: (y + h) as i32 };
        // the hair draws with one state per slot (`pick`), into whatever targets are bound
        let draw_all = |pick: &dyn Fn(&HairStates) -> Option<&ID3D12PipelineState>| {
            let mut bound = usize::MAX;
            for (model_index, slot, count, first_index, base_vertex, skin_now, skin_then, material) in draws {
                let (Some(Some(states)), Some((vertex_view, index_view))) = (set.get(*slot), model(*model_index)) else { continue };
                let Some(state) = pick(states) else { continue };
                if bound != *model_index {
                    list.IASetVertexBuffers(0, Some(&[vertex_view]));
                    list.IASetIndexBuffer(Some(&index_view));
                    bound = *model_index;
                }
                list.SetGraphicsRootConstantBufferView(2, *skin_now);
                list.SetGraphicsRootConstantBufferView(6, *skin_then);
                list.SetPipelineState(state);
                list.SetGraphicsRootConstantBufferView(1, *material);
                list.SetGraphicsRootDescriptorTable(3, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: f.table_start + *slot as u64 * f.table_stride });
                list.DrawIndexedInstanced(*count, 1, *first_index, *base_vertex, 0);
            }
        };
        let slot_tables = || {
            list.SetDescriptorHeaps(&[Some(f.srv_heap.clone()), Some(f.sampler_heap.clone())]);
            list.SetGraphicsRootDescriptorTable(4, f.sampler_heap.GetGPUDescriptorHandleForHeapStart());
            list.SetGraphicsRootShaderResourceView(5, f.sampler_index);
        };
        let kit_tables = |params: u64| {
            list.SetDescriptorHeaps(&[Some(kit.srv.clone()), Some(f.sampler_heap.clone())]);
            list.SetGraphicsRootConstantBufferView(0, f.frame_constants);
            list.SetGraphicsRootConstantBufferView(1, params);
            list.SetGraphicsRootDescriptorTable(3, kit.srv.GetGPUDescriptorHandleForHeapStart());
            list.SetGraphicsRootDescriptorTable(4, f.sampler_heap.GetGPUDescriptorHandleForHeapStart());
            list.SetGraphicsRootShaderResourceView(5, f.sampler_index);
        };
        // ---- 0. the scene depth readable (and, for the copies, the scene target a copy source)
        let mut first = vec![transition(f.scene, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, scene_state, D3D12_RESOURCE_STATE_COPY_SOURCE)];
        for (plane, s) in f.depth_states.iter().enumerate() {
            if *s != depth_read.0 {
                first.push(transition(f.depth, plane as u32, D3D12_RESOURCE_STATES(*s), depth_read));
            }
        }
        list.ResourceBarrier(&first);
        list.SetGraphicsRootSignature(f.root);
        list.SetGraphicsRootConstantBufferView(0, f.frame_constants);
        list.IASetPrimitiveTopology(windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        // ---- 1. THE MARK over the whole box: every hair draw once more with its own core shader, 1x, depth EQUAL against the
        // scene depth (read only): its own colour exactly where the early core is the scene's surface
        let mark_rtv = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: r0.ptr + 4 * rs };
        let ds = f.device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_DSV) as usize;
        let depth_ro = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: kit_dsv.ptr + ds };
        list.ClearRenderTargetView(mark_rtv, &[0.0; 4], Some(&[rect(bx, by, bw, bh)]));
        list.RSSetViewports(&[full_view]);
        list.RSSetScissorRects(&[rect(bx, by, bw, bh)]);
        slot_tables();
        list.OMSetRenderTargets(1, Some(&mark_rtv), false, Some(&depth_ro));
        draw_all(&|s| s.mark.as_ref());
        list.ResourceBarrier(&[transition(&kit.mark, 0, D3D12_RESOURCE_STATE_RENDER_TARGET, read)]);
        // ---- per tile of the box (the kit's targets are one tile large)
        let mut velo_done = false;
        for (k, &[tx, ty, tw, th]) in tiles.iter().enumerate() {
            let params = f.params + (k * PARAMS_BYTES) as u64;
            // 2. the scene's tile copied; clears, the 4x depth and the init texture from the scene depth
            let from = D3D12_TEXTURE_COPY_LOCATION { pResource: std::mem::transmute_copy(f.scene), Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX, Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { SubresourceIndex: 0 } };
            let to = D3D12_TEXTURE_COPY_LOCATION { pResource: std::mem::transmute_copy(&kit.copy), Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX, Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { SubresourceIndex: 0 } };
            let region = D3D12_BOX { left: tx, top: ty, front: 0, right: tx + tw, bottom: ty + th, back: 1 };
            list.CopyTextureRegion(&to, 0, 0, 0, &from, Some(&region));
            list.ClearRenderTargetView(col_rtv, &[0.0; 4], None);
            list.ClearRenderTargetView(aux_rtv, &[0.0; 4], None);
            list.ClearDepthStencilView(kit_dsv, D3D12_CLEAR_FLAG_DEPTH, 0.0, 0, None);
            let tile_rect = rect(0, 0, tw, th);
            list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: tw as f32, Height: th as f32, MinDepth: 0.0, MaxDepth: 1.0 }]);
            list.RSSetScissorRects(&[tile_rect]);
            kit_tables(params);
            list.OMSetRenderTargets(0, None, false, Some(&kit_dsv));
            list.SetPipelineState(&pipes.init);
            list.DrawInstanced(3, 1, 0, 0);
            list.OMSetRenderTargets(1, Some(&init_rtv), false, None);
            list.SetPipelineState(&pipes.init1);
            list.DrawInstanced(3, 1, 0, 0);
            // 3. the hair: the scene's projection moved so that the tile's corner is the targets' origin
            list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: -(tx as f32), TopLeftY: -(ty as f32), Width: f.width as f32, Height: f.height as f32, MinDepth: 0.0, MaxDepth: 1.0 }]);
            list.RSSetScissorRects(&[tile_rect]);
            slot_tables();
            list.SetGraphicsRootConstantBufferView(0, f.frame_constants);
            let pair = [col_rtv, aux_rtv];
            list.OMSetRenderTargets(2, Some(pair.as_ptr()), false, Some(&kit_dsv));
            draw_all(&|s| Some(&s.solid));
            draw_all(&|s| Some(&s.thin));
            // 4. resolved over the scene
            let mut mid = vec![
                transition(&kit.col, 0, D3D12_RESOURCE_STATE_RENDER_TARGET, read),
                transition(&kit.aux, 0, D3D12_RESOURCE_STATE_RENDER_TARGET, read),
                transition(&kit.dep, 0, D3D12_RESOURCE_STATE_DEPTH_WRITE, read),
                transition(&kit.init, 0, D3D12_RESOURCE_STATE_RENDER_TARGET, read),
                transition(&kit.copy, 0, D3D12_RESOURCE_STATE_COPY_DEST, read),
                transition(f.scene, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_COPY_SOURCE, D3D12_RESOURCE_STATE_RENDER_TARGET),
            ];
            list.ResourceBarrier(&mid);
            list.RSSetViewports(&[full_view]);
            list.RSSetScissorRects(&[rect(tx, ty, tw, th)]);
            kit_tables(params);
            list.OMSetRenderTargets(1, Some(&f.scene_view), false, None);
            list.SetPipelineState(&pipes.comp);
            list.DrawInstanced(3, 1, 0, 0);
            // 5. his motion vectors, depth and stencil where the strands' own pixels cover enough (only with the snapshot at
            // t0: the pass writes the scene depth)
            if let (Some((target, target_state)), Some(velo), true, Some(stencil)) = (f.velocity, pipes.velo.as_ref(), f.snapshot.is_some(), f.stencil) {
                let desc = target.GetDesc();
                if desc.Format == DXGI_FORMAT_R16G16_FLOAT && desc.Width == f.width as u64 && desc.Height == f.height {
                    let rtv = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: r0.ptr + 3 * rs };
                    f.device.CreateRenderTargetView(target, None, rtv);
                    let mut b = Vec::new();
                    if target_state != D3D12_RESOURCE_STATE_RENDER_TARGET.0 {
                        b.push(transition(target, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATES(target_state), D3D12_RESOURCE_STATE_RENDER_TARGET));
                    }
                    for plane in 0..f.depth_states.len() {
                        b.push(transition(f.depth, plane as u32, depth_read, D3D12_RESOURCE_STATE_DEPTH_WRITE));
                    }
                    list.ResourceBarrier(&b);
                    list.OMSetRenderTargets(1, Some(&rtv), false, Some(&f.depth_view));
                    list.OMSetStencilRef(stencil as u32);
                    list.SetPipelineState(velo);
                    list.DrawInstanced(3, 1, 0, 0);
                    velo_done = true;
                    let mut a = Vec::new();
                    if target_state != D3D12_RESOURCE_STATE_RENDER_TARGET.0 {
                        a.push(transition(target, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATES(target_state)));
                    }
                    for plane in 0..f.depth_states.len() {
                        a.push(transition(f.depth, plane as u32, D3D12_RESOURCE_STATE_DEPTH_WRITE, depth_read));
                    }
                    list.ResourceBarrier(&a);
                }
            }
            // back to the resting states (the scene target a copy source again for the next tile)
            for b in mid.iter_mut() {
                let t = &mut *b.Anonymous.Transition;
                std::mem::swap(&mut t.StateBefore, &mut t.StateAfter);
            }
            list.ResourceBarrier(&mid);
        }
        // ---- the mark to its resting state; the scene and its depth to their states at the split
        let mut last = vec![transition(&kit.mark, 0, read, D3D12_RESOURCE_STATE_RENDER_TARGET)];
        if f.scene_state != D3D12_RESOURCE_STATE_COPY_SOURCE.0 {
            last.push(transition(f.scene, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_COPY_SOURCE, scene_state));
        }
        for (plane, s) in f.depth_states.iter().enumerate() {
            if *s != depth_read.0 {
                last.push(transition(f.depth, plane as u32, depth_read, D3D12_RESOURCE_STATES(*s)));
            }
        }
        list.ResourceBarrier(&last);
        DRAWN.fetch_add(1, Ordering::Relaxed);
        TILES.fetch_add(tiles.len() as u32, Ordering::Relaxed);
        if f.snapshot.is_some() {
            SNAP.fetch_add(1, Ordering::Relaxed);
        }
        if velo_done {
            VELO.fetch_add(1, Ordering::Relaxed);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_box_holds_the_head_and_stays_on_the_target() {
        // a camera 2 m in front of the head, looking at it (game space: X forward, Z up): a box in the middle
        let eye = glam::Vec3::new(-200.0, 0.0, 150.0);
        let view = glam::Mat4::look_at_rh(eye, glam::Vec3::new(0.0, 0.0, 150.0), glam::Vec3::Z);
        let proj = glam::Mat4::perspective_rh(50f32.to_radians(), 16.0 / 9.0, 10.0, 10000.0);
        let clip = proj * view;
        let b = head_box(&clip, glam::Vec3::new(0.0, 0.0, 150.0), 2560, 1440).unwrap();
        assert!(b[0] > 0 && b[1] > 0 && b[0] + b[2] < 2560 && b[1] + b[3] < 1440, "{b:?}");
        assert!(b[0] < 1280 && b[0] + b[2] > 1280 && b[1] < 720 && b[1] + b[3] > 720, "{b:?}");
        // the ball reaches behind the camera: the whole target
        assert_eq!(head_box(&clip, eye + glam::Vec3::X * 20.0, 2560, 1440), Some([0, 0, 2560, 1440]));
        // off to the side: none
        assert_eq!(head_box(&clip, glam::Vec3::new(0.0, 5000.0, 150.0), 2560, 1440), None);
    }

    #[test]
    fn the_kit_grows_in_steps_and_never_shrinks() {
        assert_eq!(kit_size(300, 200, None, (2560, 1440)), (512, 256));
        assert_eq!(kit_size(300, 200, Some((1024, 768)), (2560, 1440)), (1024, 768));
        assert_eq!(kit_size(1100, 300, Some((1024, 768)), (2560, 1440)), (1280, 768));
        assert_eq!(kit_size(2560, 1410, None, (2560, 1440)), (2560, 1440), "never past the target");
        let p = params([10, 20, 300, 400], true);
        assert_eq!(&p[..8], &[10.0, 20.0, 300.0, 400.0, PUSH, VELO_FROM, RADIUS, 1.0]);
        assert_eq!(params([0; 4], false)[7], 0.0);
        // a 4K close-up: 3 x 3 tiles, covering the box exactly once, none larger than a tile
        let b = [100, 50, 3600, 2100];
        let ts = tiles(b);
        assert_eq!(ts.len(), 9);
        assert!(ts.iter().all(|t| t[2] <= TILE_W && t[3] <= TILE_H));
        assert_eq!(ts.iter().map(|t| t[2] as u64 * t[3] as u64).sum::<u64>(), 3600 * 2100);
        assert_eq!((ts[0][0], ts[0][1], ts[8][0] + ts[8][2], ts[8][1] + ts[8][3]), (100, 50, 3700, 2150));
        assert_eq!(tiles([5, 6, 300, 200]), vec![[5, 6, 300, 200]]);
        let p = params(b, true);
        assert_eq!(p.len(), 9 * PARAMS_BYTES / 4);
        assert_eq!(&p[PARAMS_BYTES / 4..PARAMS_BYTES / 4 + 4], &[ts[1][0] as f32, ts[1][1] as f32, ts[1][2] as f32, ts[1][3] as f32]);
    }
}
