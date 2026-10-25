//! (VFXR) THE EFFECT LAYER'S POST PASSES in the Elden Ring renderer: KH3's glow (UE4's bloom) on OUR effects, and the
//! screen-wide tricks KH3's effects ask for (kh_vfx::screen: radial blurs, light streaks, gauss blur, screen blur,
//! colour filters, colour fringes). A child of vfx.rs (its GPU objects, root signature and heaps).
//!
//! THE ROUTE AND WHY (build/sora_tmp/party/IMPL_VFXR_STATE.md):
//! - Elden Ring's own glow is made from its GLOW SOURCE (the deferred composite's and every forward material's second
//!   target, IMPL_ER80), not from the scene colour: our effects write only the scene colour, so the game never blooms
//!   them. Writing them into the glow source would make the game's glow scale (set per area) decide KH3's look and
//!   depends on that target still being open at our late stage. So the effect layer gets ITS OWN BLOOM, UE4's (the
//!   one KH3 runs, at KH3's settings: kh_vfx::screen::UE4_*), made from what OUR effects added to the scene only: the
//!   scene after them minus a copy made before them. The world's own pixels add nothing, so its look is unchanged;
//!   only light our effects added glows (also over the world around them, as in KH3).
//! - The decals are drawn in an earlier stage of the frame (Stage::Decals): what they add is measured there (a copy
//!   before, the difference after) into the bloom source, and the late stage adds its own to it in the same frame.
//! - The screen passes follow KH3's own shaders (disassembled from the game's global shader cache: kh_vfx::screen) and
//!   run on the scene target after the effects, before the game's TAA / exposure / tone mapping; a filter is given in
//!   KH3's displayed units and converted with KH3's reference white (`white`).
//! - Everything here can be switched off (`set_post`: kh-er `vfx.bloom`, `vfx.screen_fx`); nothing fails a frame.
//!
//! UNTESTED IN GAME: proven offline only (examples/effects.rs, a real D3D12 device, Elden Ring's R11G11B10 scene format).

use super::*;
use kh_vfx::screen::{ScreenFx, ScreenKind, UE4_BLOOM_INTENSITY, UE4_BLOOM_LEVELS, UE4_BLOOM_SIZE_SCALE, UE4_BLOOM_THRESHOLD};

/// Tables of 12 descriptors per frame slot for the post passes (behind the frame slots' own two).
pub(super) const POST_TABLES: usize = 48;
/// The decal stage's tables (the late stage of the same frame uses those below: both lists run after both are recorded).
pub(super) const DECAL_TABLES: usize = 44;
pub(super) const POST_DESCRIPTORS: usize = FRAME_SLOTS * POST_TABLES * MATERIAL_TEXTURES;
const HALF: DXGI_FORMAT = DXGI_FORMAT_R16G16B16A16_FLOAT;
const LEVELS: usize = 6;
/// The sampler heap entry the post passes sample with (clamp, clamp, linear).
const CLAMP: u32 = 4;

/// `vfx.bloom` (a factor on KH3's bloom of the effects; 0 = none) and `vfx.screen_fx` (the screen-wide passes).
static BLOOM: AtomicU32 = AtomicU32::new(0x3F80_0000);
static SCREEN_FX: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
/// (VFXR) `vfx.eye_adapt` (default on): the eye-adaptation materials at KH3's place (see `eye_boost`).
// (VFXR eye evidence) default OFF since 2026-10-23 (the test app's evidence: KH3 captures (build/sora_tmp/ref water_real_*, thunder_real_*) against the test level: with the 3.27 boost Watera's burst read 20 % near-white and half as saturated as KH3's (KH3 1-2 %, saturation 0.35-0.58; no boost: 4 %, 0.42), Thunder's flash lifted the sky KH3 leaves alone (build/sora_tmp/vfxr/shots/bright)); `vfx.eye_adapt = 1`: on
static EYE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// (VFXR) The host's switch for the eye-adaptation materials (`vfx.eye_adapt`).
pub fn set_eye_adapt(on: bool) {
    EYE.store(on, Ordering::Relaxed);
}

/// (VFXR) What the eye-adaptation materials' colour is multiplied by: KH3 shows their emissive at the tone mapper's input
/// (the compiled shaders divide it by the view's eye adaptation, v161.y), where the effect rule (vfx.rs effect_scale) puts
/// KH3's REFERENCE WHITE (3.27, a lit white surface): their emissive 1.0 belongs at 3.27 of KH3's scene-linear units.
/// (The prize orbs' own factor prize.glow 3.27 was this fix for the orbs alone: kh-er divides it by this.)
pub fn eye_boost() -> f32 {
    if EYE.load(Ordering::Relaxed) {
        crate::light::reference_white()
    } else {
        1.0
    }
}

/// For the log: frames with the bloom, frames with screen passes, passes run.
static BLOOM_FRAMES: AtomicU32 = AtomicU32::new(0);
static SCREEN_FRAMES: AtomicU32 = AtomicU32::new(0);
static PASSES: AtomicU64 = AtomicU64::new(0);

/// (VFXR) The host's switches: `bloom` multiplies KH3's bloom of the effects (1 = KH3's, 0 = off), `screen` runs the
/// screen-wide passes the effects ask for.
pub fn set_post(bloom: f32, screen: bool) {
    BLOOM.store(bloom.max(0.0).to_bits(), Ordering::Relaxed);
    SCREEN_FX.store(screen, Ordering::Relaxed);
}

pub(super) fn bloom_factor() -> f32 {
    f32::from_bits(BLOOM.load(Ordering::Relaxed))
}

pub(super) fn screen_on() -> bool {
    SCREEN_FX.load(Ordering::Relaxed)
}

/// For `vfx::status`.
pub(super) fn status() -> String {
    format!("post: bloom x{:.2} on {} frame(s), screen passes on {} frame(s), {} pass(es)", bloom_factor(), BLOOM_FRAMES.load(Ordering::Relaxed), SCREEN_FRAMES.load(Ordering::Relaxed), PASSES.load(Ordering::Relaxed))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Mode {
    Write,
    Add,
    Alpha,
}

struct Tex {
    res: ID3D12Resource,
    w: u32,
    h: u32,
    rtv: D3D12_CPU_DESCRIPTOR_HANDLE,
}

struct Targets {
    w: u32,
    h: u32,
    format: i32,
    /// The scene after the effects (and the source of every screen pass), and the scene before the decals.
    copy: ID3D12Resource,
    pre: ID3D12Resource,
    /// The bloom chain (1/2 .. 1/64 of the scene) and its blur temporaries.
    levels: Vec<Tex>,
    temps: Vec<Tex>,
    /// Half-size targets of the radial / gauss passes.
    ra: Tex,
    rb: Tex,
}

pub(super) struct PostKit {
    /// The device and formats it was made for (another: made again).
    pub(super) device: usize,
    pub(super) formats: (i32, i32),
    states: HashMap<(&'static str, Mode, i32), ID3D12PipelineState>,
    rtv: ID3D12DescriptorHeap,
    rtv_size: usize,
    targets: Option<Targets>,
    /// The prepared frame (its Arc's address) whose decals' added light is already in levels[0].
    decal_frame: usize,
}

// SAFETY: as Gpu: free-threaded D3D12 objects, used under the GPU mutex.
unsafe impl Send for PostKit {}

const PASS_LIST: [(&str, Mode, bool); 13] = [
    ("fxp_bsrc", Mode::Write, false),
    ("fxp_bsrc", Mode::Add, false),
    ("fxp_down", Mode::Write, false),
    ("fxp_blur", Mode::Write, false),
    ("fxp_radial", Mode::Write, false),
    ("fxp_bloom", Mode::Add, true),
    ("fxp_rcomp", Mode::Alpha, true),
    ("fxp_rcomp", Mode::Add, true),
    ("fxp_sblur", Mode::Alpha, true),
    ("fxp_filter", Mode::Add, true),
    ("fxp_filter", Mode::Alpha, true),
    ("fxp_fringe", Mode::Write, true),
    ("fxp_down", Mode::Write, true),
];

impl PostKit {
    /// The pipeline states (on the worker thread). Err: a shader file is missing (kh-shaderc --vfx --only fxp).
    pub(super) unsafe fn new(gpu: &mut Gpu, dir: &Path) -> Result<PostKit, String> {
        let vertex = gpu.shader(dir, "screen.vs.dxil").ok_or("screen.vs.dxil is not in PACK/vfx/er")?;
        let mut states = HashMap::new();
        for (name, mode, scene) in PASS_LIST {
            let pixel = gpu.shader(dir, &format!("{name}.ps.dxil")).ok_or(format!("{name}.ps.dxil is not in PACK/vfx/er (kh-shaderc --vfx --only fxp)"))?;
            let format = if scene { DXGI_FORMAT(gpu.formats.0) } else { HALF };
            let rgb = (D3D12_COLOR_WRITE_ENABLE_RED.0 | D3D12_COLOR_WRITE_ENABLE_GREEN.0 | D3D12_COLOR_WRITE_ENABLE_BLUE.0) as u8;
            let (enable, source, dest) = match mode {
                Mode::Write => (false, D3D12_BLEND_ONE, D3D12_BLEND_ZERO),
                Mode::Add => (true, D3D12_BLEND_ONE, D3D12_BLEND_ONE),
                Mode::Alpha => (true, D3D12_BLEND_SRC_ALPHA, D3D12_BLEND_INV_SRC_ALPHA),
            };
            let mut blend = D3D12_BLEND_DESC::default();
            blend.RenderTarget[0] = D3D12_RENDER_TARGET_BLEND_DESC { BlendEnable: enable.into(), LogicOpEnable: false.into(), SrcBlend: source, DestBlend: dest, BlendOp: D3D12_BLEND_OP_ADD, SrcBlendAlpha: D3D12_BLEND_ZERO, DestBlendAlpha: D3D12_BLEND_ONE, BlendOpAlpha: D3D12_BLEND_OP_ADD, LogicOp: D3D12_LOGIC_OP_NOOP, RenderTargetWriteMask: if scene { rgb } else { D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8 } };
            let mut formats = [DXGI_FORMAT_UNKNOWN; 8];
            formats[0] = format;
            let desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
                pRootSignature: std::mem::transmute_copy(&gpu.root),
                VS: D3D12_SHADER_BYTECODE { pShaderBytecode: vertex.as_ptr() as *const c_void, BytecodeLength: vertex.len() },
                PS: D3D12_SHADER_BYTECODE { pShaderBytecode: pixel.as_ptr() as *const c_void, BytecodeLength: pixel.len() },
                BlendState: blend,
                SampleMask: u32::MAX,
                RasterizerState: D3D12_RASTERIZER_DESC { FillMode: D3D12_FILL_MODE_SOLID, CullMode: D3D12_CULL_MODE_NONE, DepthClipEnable: true.into(), ..Default::default() },
                DepthStencilState: D3D12_DEPTH_STENCIL_DESC { DepthEnable: false.into(), StencilEnable: false.into(), ..Default::default() },
                PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
                NumRenderTargets: 1,
                RTVFormats: formats,
                DSVFormat: DXGI_FORMAT_UNKNOWN,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                ..Default::default()
            };
            let state: ID3D12PipelineState = gpu.device.CreateGraphicsPipelineState(&desc).map_err(|e| format!("{name} ({mode:?}): {e}"))?;
            states.insert((name, mode, format.0), state);
        }
        let rtv: ID3D12DescriptorHeap = gpu.device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV, NumDescriptors: 32, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE, NodeMask: 0 }).map_err(|e| format!("rtv heap: {e}"))?;
        let rtv_size = gpu.device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV) as usize;
        Ok(PostKit { device: gpu.device.as_raw() as usize, formats: gpu.formats, states, rtv, rtv_size, targets: None, decal_frame: 0 })
    }

    /// Our targets for a scene of this size and format (made again when it changes; the old ones kept two seconds).
    unsafe fn ensure(&mut self, device: &ID3D12Device, desc: &D3D12_RESOURCE_DESC) -> bool {
        let (w, h, format) = (desc.Width as u32, desc.Height, desc.Format.0);
        if self.targets.as_ref().is_some_and(|t| (t.w, t.h, t.format) == (w, h, format)) {
            return true;
        }
        if let Some(old) = self.targets.take() {
            let mut retired = RETIRED.lock().unwrap();
            for r in [old.copy, old.pre].into_iter().chain(old.levels.into_iter().map(|t| t.res)).chain(old.temps.into_iter().map(|t| t.res)).chain([old.ra.res, old.rb.res]) {
                retired.push((Instant::now(), r));
            }
        }
        let copy_desc = D3D12_RESOURCE_DESC { Flags: D3D12_RESOURCE_FLAG_NONE, Alignment: 0, ..*desc };
        let (Ok(copy), Ok(pre)) = (committed(device, D3D12_HEAP_TYPE_DEFAULT, &copy_desc, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE), committed(device, D3D12_HEAP_TYPE_DEFAULT, &copy_desc, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE)) else { return false };
        let start = self.rtv.GetCPUDescriptorHandleForHeapStart();
        let mut next = 0usize;
        let mut make = |w: u32, h: u32| -> Option<Tex> {
            let (w, h) = (w.max(1), h.max(1));
            let d = D3D12_RESOURCE_DESC { Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D, Width: w as u64, Height: h, DepthOrArraySize: 1, MipLevels: 1, Format: HALF, SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 }, Flags: D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET, ..Default::default() };
            let res = committed(device, D3D12_HEAP_TYPE_DEFAULT, &d, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE).ok()?;
            let rtv = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: start.ptr + next * self.rtv_size };
            next += 1;
            device.CreateRenderTargetView(&res, None, rtv);
            Some(Tex { res, w, h, rtv })
        };
        let mut levels = Vec::new();
        let mut temps = Vec::new();
        for k in 0..LEVELS {
            let f = 2u32 << k;
            let (Some(l), Some(t)) = (make(w / f, h / f), make(w / f, h / f)) else { return false };
            levels.push(l);
            temps.push(t);
        }
        let (Some(ra), Some(rb)) = (make(w / 2, h / 2), make(w / 2, h / 2)) else { return false };
        self.targets = Some(Targets { w, h, format, copy, pre, levels, temps, ra, rb });
        self.decal_frame = 0;
        true
    }
}

/// Where a world point (KH3 space) lands on the screen (uv, origin top left); a point behind the camera keeps the
/// mirrored place the projection gives it, as the game's would.
pub(super) fn project(clip_from_game: &Mat4, p: Vec3) -> [f32; 2] {
    let c = *clip_from_game * p.extend(1.0);
    let w = if c.w.abs() < 1.0e-4 { 1.0e-4_f32.copysign(c.w) } else { c.w };
    [((c.x / w) * 0.5 + 0.5).clamp(-2.0, 3.0), (0.5 - (c.y / w) * 0.5).clamp(-2.0, 3.0)]
}

/// The gaussian of bloom level `k` for a scene `width` pixels wide: (sigma, taps to each side, the step in texels) at
/// that level's size (UE4: the kernel's radius is BloomNSize x BloomSizeScale percent of the view's width) [A].
pub(super) fn level_kernel(k: usize, width: u32) -> (f32, i32, f32) {
    let radius_full = kh_vfx::screen::ue4_bloom_radius_px(k, width as f32); // (VFXR bloom fix) the size is a diameter
    let radius = radius_full / (2u32 << k) as f32;
    let step = (radius / 64.0).max(1.0);
    let taps = (radius / step).ceil().max(1.0);
    (radius / 3.0 / step, taps as i32, step)
}

/// What one pass reads and writes.
enum Out<'a> {
    Ours(&'a Tex, Mode),
    Scene(Mode),
}

/// The recording context of one call of `vfx::draw`.
pub(super) struct Rec<'a, 'b> {
    pub cx: &'a DrawCx<'b>,
    pub gpu: &'a Gpu,
    pub put: &'a mut dyn FnMut(&[u8], usize) -> Option<u64>,
    pub tables: usize,
    pub samplers: Option<u64>,
}

impl Rec<'_, '_> {
    fn bytes<T>(v: &[T]) -> &[u8] {
        // SAFETY: plain floats / words.
        unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, std::mem::size_of_val(v)) }
    }

    /// One full-screen pass. `inputs` go to t0..; `rows` to KhParams rows 2..; `input_size` = t0's size.
    unsafe fn pass(&mut self, kit: &PostKit, name: &'static str, out: Out, inputs: &[&ID3D12Resource], input_size: (u32, u32), rows: &[[f32; 4]]) -> bool {
        let cx = self.cx;
        let gpu = self.gpu;
        if self.tables >= if self.tables >= DECAL_TABLES { POST_TABLES } else { DECAL_TABLES } {
            return false;
        }
        let (mode, format, rtv, size, ours) = match &out {
            Out::Ours(t, m) => (*m, HALF.0, t.rtv, (t.w, t.h), Some(*t)),
            Out::Scene(m) => (*m, gpu.formats.0, cx.target_view, (cx.width, cx.height), None),
        };
        let Some(state) = kit.states.get(&(name, mode, format)) else { return false };
        if self.samplers.is_none() {
            self.samplers = (self.put)(Self::bytes(&[CLAMP; 16]), 256);
        }
        let Some(samplers) = self.samplers else { return false };
        let mut params = [0.0f32; PARAM_ROWS * 4];
        params[0..4].copy_from_slice(&[input_size.0 as f32, input_size.1 as f32, 1.0 / input_size.0.max(1) as f32, 1.0 / input_size.1.max(1) as f32]);
        params[4..8].copy_from_slice(&[size.0 as f32, size.1 as f32, 1.0 / size.0.max(1) as f32, 1.0 / size.1.max(1) as f32]);
        for (k, r) in rows.iter().enumerate().take(62) {
            params[(k + 2) * 4..(k + 3) * 4].copy_from_slice(r);
        }
        let Some(constants) = (self.put)(Self::bytes(&params), 256) else { return false };
        // the pass's table: its inputs, white behind them
        let base = MAX_MATERIALS * MATERIAL_TEXTURES + FRAME_SLOTS * 2 + (cx.slot * POST_TABLES + self.tables) * MATERIAL_TEXTURES;
        self.tables += 1;
        let start = gpu.srv_heap.GetCPUDescriptorHandleForHeapStart();
        for k in 0..MATERIAL_TEXTURES {
            let r = inputs.get(k).copied().unwrap_or(&gpu.white);
            cx.device.CreateShaderResourceView(r, None, D3D12_CPU_DESCRIPTOR_HANDLE { ptr: start.ptr + (base + k) * gpu.srv_size });
        }
        let list = cx.list;
        if let Some(t) = ours {
            list.ResourceBarrier(&[transition(&t.res, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, D3D12_RESOURCE_STATE_RENDER_TARGET)]);
        }
        list.OMSetRenderTargets(1, Some(&rtv), false, None);
        list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: size.0 as f32, Height: size.1 as f32, MinDepth: 0.0, MaxDepth: 1.0 }]);
        list.RSSetScissorRects(&[windows::Win32::Foundation::RECT { left: 0, top: 0, right: size.0 as i32, bottom: size.1 as i32 }]);
        list.SetPipelineState(state);
        list.SetGraphicsRootConstantBufferView(1, constants);
        list.SetGraphicsRootDescriptorTable(2, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: gpu.srv_heap.GetGPUDescriptorHandleForHeapStart().ptr + (base * gpu.srv_size) as u64 });
        list.SetGraphicsRootShaderResourceView(5, samplers);
        list.DrawInstanced(3, 1, 0, 0);
        if let Some(t) = ours {
            list.ResourceBarrier(&[transition(&t.res, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE)]);
        }
        PASSES.fetch_add(1, Ordering::Relaxed);
        true
    }

    /// The scene target copied into `to` (the target is in RENDER_TARGET and goes back to it).
    unsafe fn copy_scene(&mut self, to: &ID3D12Resource) {
        let list = self.cx.list;
        list.ResourceBarrier(&[transition(self.cx.target, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_COPY_SOURCE), transition(to, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, D3D12_RESOURCE_STATE_COPY_DEST)]);
        list.CopyResource(to, self.cx.target);
        list.ResourceBarrier(&[transition(self.cx.target, D3D12_RESOURCE_STATE_COPY_SOURCE, D3D12_RESOURCE_STATE_RENDER_TARGET), transition(to, D3D12_RESOURCE_STATE_COPY_DEST, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE)]);
    }
}

/// The value of KH3's exposed white in the target's units: KH3's reference white x what KH3's 1.0 becomes here.
pub(super) fn white(cx: &DrawCx) -> f32 {
    crate::light::reference_white() * cx.scale * options().hdr
}

/// Whether the frame's target is a float HDR scene (the post passes run on those only).
pub(super) fn hdr_target(desc: &D3D12_RESOURCE_DESC) -> bool {
    matches!(desc.Format, DXGI_FORMAT_R11G11B10_FLOAT | DXGI_FORMAT_R16G16B16A16_FLOAT | DXGI_FORMAT_R32G32B32A32_FLOAT) && desc.Dimension == D3D12_RESOURCE_DIMENSION_TEXTURE2D && desc.SampleDesc.Count == 1 && desc.DepthOrArraySize == 1
}

/// THE DECALS' SHARE of the bloom (Stage::Decals): `before` copies the scene before the decals; `after` puts what they
/// added into the bloom source (levels[0]) for the late stage of the same frame.
pub(super) unsafe fn decals_before(rec: &mut Rec, kit: &mut PostKit, desc: &D3D12_RESOURCE_DESC) -> bool {
    if bloom_factor() <= 0.0 || !kit.ensure(rec.cx.device, desc) {
        return false;
    }
    let Some(t) = kit.targets.as_ref() else { return false };
    let pre = t.pre.clone();
    rec.copy_scene(&pre);
    true
}

pub(super) unsafe fn decals_after(rec: &mut Rec, kit: &mut PostKit, frame: usize) {
    let Some(t) = kit.targets.as_ref() else { return };
    let (copy, pre) = (t.copy.clone(), t.pre.clone());
    rec.copy_scene(&copy);
    let w = white(rec.cx);
    let made = {
        let t = kit.targets.as_ref().unwrap();
        rec.pass(kit, "fxp_bsrc", Out::Ours(&t.levels[0], Mode::Write), &[&copy, &pre], (t.w, t.h), &[[w, cine_threshold() /* (VFXR cinematic) */, 0.0, 0.0]])
    };
    if made {
        kit.decal_frame = frame;
    }
}

/// THE LATE STAGE'S POST PASSES (after the translucent / additive effects): `pre` = the scene before them (vfx.rs's
/// copy), `bloom` = the frame's effects added light (else only the screen passes run), `screen` = the passes asked for.
pub(super) unsafe fn after_effects(rec: &mut Rec, kit: &mut PostKit, desc: &D3D12_RESOURCE_DESC, pre: Option<&ID3D12Resource>, frame: usize, screen: &[ScreenFx]) {
    let bloom = bloom_factor();
    let do_bloom = bloom > 0.0 && (pre.is_some() || kit.decal_frame == frame);
    let screen: Vec<ScreenFx> = if screen_on() { screen.to_vec() } else { Vec::new() };
    if (!do_bloom && screen.is_empty()) || !kit.ensure(rec.cx.device, desc) {
        return;
    }
    let cx = rec.cx;
    let w = white(cx);
    let mut have_source = kit.decal_frame == frame;
    kit.decal_frame = 0;
    let kit: &PostKit = kit;
    let t = kit.targets.as_ref().unwrap();
    let copy = t.copy.clone();
    rec.copy_scene(&copy);
    // ---- 1. the bloom source: what the effects of this stage added (+ the decals' share made earlier this frame)
    if let Some(pre) = pre.filter(|_| do_bloom) {
        let mode = if have_source { Mode::Add } else { Mode::Write };
        have_source |= rec.pass(kit, "fxp_bsrc", Out::Ours(&t.levels[0], mode), &[&copy, pre], (t.w, t.h), &[[w, cine_threshold() /* (VFXR cinematic) */, 0.0, 0.0]]);
    }
    // ---- 2. the screen passes (KH3's order: blurs on the picture, the glow added after, filters last)
    if !screen.is_empty() {
        SCREEN_FRAMES.fetch_add(1, Ordering::Relaxed);
    }
    let aspect = cx.width as f32 / cx.height.max(1) as f32;
    for fx in screen.iter().filter(|f| matches!(f.kind, ScreenKind::RadialBlur | ScreenKind::BloomRadialBlur | ScreenKind::GaussBlur | ScreenKind::ScreenBlur)) {
        let center = fx.world.map_or(fx.screen, |p| project(&cx.clip_from_game, p));
        match fx.kind {
            ScreenKind::RadialBlur | ScreenKind::BloomRadialBlur => {
                let bloomy = fx.kind == ScreenKind::BloomRadialBlur;
                // FPostProcessSqexBloomRadialBlurSetupPS: threshold / exposure: KH3's exposed units -> ours
                let threshold = if bloomy { fx.threshold * w } else { 0.0 };
                let a = fx.angle.to_radians();
                // (LINEAR: the direction in uv, the vertical part scaled so that it is a screen direction)
                let dir = [a.cos(), -a.sin() * aspect];
                rec.pass(kit, "fxp_down", Out::Ours(&t.ra, Mode::Write), &[&copy], (t.w, t.h), &[[threshold, 0.0, 0.0, 0.0]]);
                rec.pass(kit, "fxp_radial", Out::Ours(&t.rb, Mode::Write), &[&t.ra.res], (t.ra.w, t.ra.h), &[[center[0], center[1], fx.power, if fx.linear { 1.0 } else { 0.0 }], [dir[0], dir[1], 0.0, 0.0]]);
                let mode = if bloomy { Mode::Add } else { Mode::Alpha };
                rec.pass(kit, "fxp_rcomp", Out::Scene(mode), &[&t.rb.res], (t.rb.w, t.rb.h), &[[center[0], center[1], 0.0, 0.0], [fx.alpha, if bloomy { 1.0 } else { 0.0 }, 0.0, 0.0], fx.ring]);
            }
            ScreenKind::GaussBlur => {
                // radius: Size percent of the screen's width [A], at half size
                let radius = fx.size / 100.0 * cx.width as f32 * 0.5;
                let step = (radius / 64.0).max(1.0);
                let taps = (radius / step).ceil().max(1.0);
                let sigma = radius / 3.0 / step;
                rec.pass(kit, "fxp_down", Out::Ours(&t.ra, Mode::Write), &[&copy], (t.w, t.h), &[[0.0; 4]]);
                rec.pass(kit, "fxp_blur", Out::Ours(&t.rb, Mode::Write), &[&t.ra.res], (t.ra.w, t.ra.h), &[[step / t.ra.w as f32, 0.0, sigma, taps]]);
                rec.pass(kit, "fxp_blur", Out::Ours(&t.ra, Mode::Write), &[&t.rb.res], (t.rb.w, t.rb.h), &[[0.0, step / t.rb.h as f32, sigma, taps]]);
                rec.pass(kit, "fxp_rcomp", Out::Scene(Mode::Alpha), &[&t.ra.res], (t.ra.w, t.ra.h), &[[0.5, 0.5, 0.0, 0.0], [fx.alpha, 0.0, 0.0, 0.0], [0.0, 0.0, 4.0, 0.0]]);
            }
            _ => {
                // TPostProcessSqexScreenBlurPS<0>: the copy displaced
                rec.pass(kit, "fxp_sblur", Out::Scene(Mode::Alpha), &[&copy], (t.w, t.h), &[[center[0], center[1], 0.0, 0.0], [fx.alpha, fx.angle.to_radians(), fx.offset[0], fx.offset[1]], fx.ring, [w, fx.mode as f32, 0.0, 0.0]]);
            }
        }
    }
    // ---- 3. the bloom: the chain of half sizes, each blurred, added with UE4's tints x BloomIntensity
    if do_bloom && have_source {
        BLOOM_FRAMES.fetch_add(1, Ordering::Relaxed);
        for k in 1..LEVELS {
            let (from, to) = (&t.levels[k - 1], &t.levels[k]);
            rec.pass(kit, "fxp_down", Out::Ours(to, Mode::Write), &[&from.res], (from.w, from.h), &[[0.0; 4]]);
        }
        for k in 0..LEVELS {
            let (l, tmp) = (&t.levels[k], &t.temps[k]);
            let (sigma, taps, step) = level_kernel(k, t.w);
            rec.pass(kit, "fxp_blur", Out::Ours(tmp, Mode::Write), &[&l.res], (l.w, l.h), &[[step / l.w as f32, 0.0, sigma, taps as f32]]);
            rec.pass(kit, "fxp_blur", Out::Ours(l, Mode::Write), &[&tmp.res], (tmp.w, tmp.h), &[[0.0, step / tmp.h as f32, sigma, taps as f32]]);
        }
        let k = UE4_BLOOM_INTENSITY * bloom * kh_vfx::screen::UE4_BLOOM_TINT_SCALE; // (VFXR bloom fix) UE 4's 1 / NumBloomStages
        let tint = |i: usize| UE4_BLOOM_LEVELS[i].1 * k;
        let inputs: Vec<&ID3D12Resource> = t.levels.iter().map(|l| &l.res).collect();
        rec.pass(kit, "fxp_bloom", Out::Scene(Mode::Add), &inputs, (t.levels[0].w, t.levels[0].h), &[[tint(0), tint(1), tint(2), tint(3)], [tint(4), tint(5), 0.0, 0.0]]);
    }
    // ---- 4. filters and fringes, on the finished picture
    for fx in screen.iter().filter(|f| f.kind == ScreenKind::Filter) {
        match fx.mode {
            0 => drop(rec.pass(kit, "fxp_filter", Out::Scene(Mode::Add), &[], (t.w, t.h), &[[fx.color[0], fx.color[1], fx.color[2], fx.alpha], [0.0, w, 0.0, 0.0]])),
            1 => drop(rec.pass(kit, "fxp_filter", Out::Scene(Mode::Alpha), &[], (t.w, t.h), &[[fx.color[0], fx.color[1], fx.color[2], fx.alpha], [1.0, w, 0.0, 0.0]])),
            _ => {
                rec.copy_scene(&copy);
                rec.pass(kit, "fxp_filter", Out::Scene(Mode::Alpha), &[&copy], (t.w, t.h), &[[fx.color[0], fx.color[1], fx.color[2], fx.alpha], [2.0, w, 0.0, 0.0]]);
            }
        }
    }
    for fx in screen.iter().filter(|f| f.kind == ScreenKind::Fringe) {
        rec.copy_scene(&copy);
        rec.pass(kit, "fxp_fringe", Out::Scene(Mode::Write), &[&copy], (t.w, t.h), &[[fx.alpha, 0.0, 0.0, 0.0]]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bloom_levels_follow_ue4s_sizes() {
        // (VFXR bloom fix) level 0 at 1280: a 1.2 % DIAMETER = 7.7 px radius, at half size 3.84 px: sigma 1.28, 4 taps, step 1
        let (sigma, taps, step) = level_kernel(0, 1280);
        assert!((sigma - 1.28).abs() < 0.01 && taps == 4 && step == 1.0);
        // the widest level (256 % of the width at 1/64): its radius 25.6 px, under 64 taps
        let (_, taps, step) = level_kernel(5, 1280);
        assert!(taps <= 64 && step == 1.0);
        // at 8K the widest needs a longer step, never more than 64 taps
        let (_, taps, step) = level_kernel(5, 7680);
        assert!(taps <= 64 && step > 1.0);
    }

    #[test]
    fn a_point_in_front_projects_into_the_screen() {
        let proj = Mat4::perspective_rh(1.0, 16.0 / 9.0, 0.1, 1000.0) * Mat4::look_at_rh(Vec3::ZERO, Vec3::new(0.0, 0.0, -1.0), Vec3::Y);
        let c = project(&proj, Vec3::new(0.0, 0.0, -10.0));
        assert!((c[0] - 0.5).abs() < 1.0e-4 && (c[1] - 0.5).abs() < 1.0e-4);
        let r = project(&proj, Vec3::new(1.0, 1.0, -10.0));
        assert!(r[0] > 0.5 && r[1] < 0.5);
    }
}

/// (VFXR cinematic) UE 4.17's bloom threshold of the sequence's look (KH3's default -1; Zettaflare's beam 10 then 5).
fn cine_threshold() -> f32 {
    super::cine_look().2
}
