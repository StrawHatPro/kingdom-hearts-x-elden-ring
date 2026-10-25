//! (hud) THE 2D OVERLAY: Sora's HUD and menus over the game's finished picture.
//!
//! The host rasterises the HUD's movies itself (kh-ui through `kh_host::hud_layers`: one RGBA picture per
//! LAYER, at the size it has on screen, straight alpha, sRGB bytes) and hands every changed rectangle to
//! [`show_layer`] from its own thread. At Present, on the presenting thread, the changed rectangles are
//! copied into one texture per layer and the visible layers are drawn in order as textured rectangles over the
//! back buffer, texel for pixel, with premultiplied-alpha blending (the shader multiplies; blend ONE,
//! INV_SRC_ALPHA). The stage's scaling and letterboxing are the rasteriser's (the host asks [`output_size`]).
//!
//! OWN OBJECTS ONLY: a root signature, one pipeline state per target format, three frame slots (allocator,
//! list, a mapped upload buffer), a fence, the layer textures. Nothing of the 3D path (gfx.rs) is used but the
//! game's device and queue, so the two cannot disturb each other; the one call from gfx.rs is in `present`.
//!
//! COLOUR: the pictures are display-encoded. Into a 10-bit or (with `gamma`) an 8-bit UNORM back buffer they
//! are blended as they are (the game's picture in there is display-encoded too: blending in display space,
//! what Scaleform does). Into an 8-bit UNORM back buffer by default: through an sRGB view with linear values
//! (blending in linear light, exactly the Bevy test app's result). A float back buffer (scRGB) gets linear
//! values times `linear_scale`. Any other format: nothing is drawn (said once). An HDR10 (PQ) swap chain has
//! the 10-bit format too and CANNOT be told apart here: the HUD would be too dark there (not handled).
//!
//! SAFETY RULES of the renderer kept: nothing here panics into the game (a panic turns the overlay off for
//! good), any failed call skips the frame and is tried again after 2 s (10 s after three in a row), a lost or
//! changed device drops our objects and makes them again, the back buffer is put back into the PRESENT state
//! it was found in, no state of the game's lists is touched (our draws are on a list of our own). No per-frame
//! allocation: textures and upload buffers only grow.

use std::collections::HashMap;
use std::ffi::c_void;
use std::mem::ManuallyDrop;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use windows::core::Interface;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;

use crate::rlog;

/// Layers a host may use (drawn in index order).
pub const MAX_LAYERS: usize = 16;
const SLOTS: usize = 3;
/// (ER party S4) the friends' pictures' descriptors (after Sora's picture's): one per picture and frame slot.
#[cfg(feature = "party")]
const PARTY_SLOTS: usize = SLOTS * PARTY_PICTURES;
#[cfg(not(feature = "party"))]
const PARTY_SLOTS: usize = 0;
/// An upload buffer never grows beyond this; what does not fit waits for the next frame.
const UPLOAD_MAX: usize = 64 << 20;
const UPLOAD_START: usize = 4 << 20;

static VS: &[u8] = include_bytes!("../shaders/hud/hud.vs.dxil");
static PS: &[u8] = include_bytes!("../shaders/hud/hud.ps.dxil");

/// x, y, width, height in pixels of the back buffer.
pub type PxRect = (i32, i32, usize, usize);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HudOptions {
    /// Draw the overlay at all.
    pub on: bool,
    /// Blend display-encoded values even into an 8-bit back buffer (see the head of this file).
    pub gamma: bool,
    /// What linear 1.0 becomes in a float (scRGB) back buffer.
    pub linear_scale: f32,
    /// Opacity of the whole overlay, 0..1 (IMPL_ER8: faded out while a full-screen menu of the game is up).
    pub opacity: f32,
}

impl Default for HudOptions {
    fn default() -> HudOptions {
        HudOptions { on: false, gamma: false, linear_scale: 1.0, opacity: 1.0 }
    }
}

struct LayerCpu {
    visible: bool,
    rect: PxRect,
    /// The whole picture of `rect` (straight alpha, sRGB bytes).
    pixels: Vec<u8>,
    /// What the texture does not hold yet: x0, y0, x1, y1 inside the picture.
    dirty: Option<(usize, usize, usize, usize)>,
}

struct LayerGpu {
    texture: Option<(ID3D12Resource, u32, u32)>,
    /// The rectangle the texture's content is the picture of (`None`: nothing uploaded yet).
    shown: Option<PxRect>,
}

struct Slot {
    allocator: ID3D12CommandAllocator,
    list: ID3D12GraphicsCommandList,
    upload: ID3D12Resource,
    mapped: *mut u8,
    capacity: usize,
    busy_until: u64,
}

struct Gpu {
    device: ID3D12Device,
    queue: usize,
    root: ID3D12RootSignature,
    pipelines: HashMap<i32, ID3D12PipelineState>,
    srv_heap: ID3D12DescriptorHeap,
    srv_size: usize,
    rtv_heap: ID3D12DescriptorHeap,
    rtv_size: usize,
    fence: ID3D12Fence,
    fence_value: u64,
    slots: Vec<Slot>,
    layers: Vec<LayerGpu>,
    retired: Vec<(u64, ID3D12Resource)>,
}

#[derive(Default)]
struct Stats {
    presents: u32,
    drawn: u32,
    busy: u32,
    deferred: u32,
    quads: u32,
    upload_bytes: u64,
    micros: u64,
    micros_max: u64,
    host_micros: u64,
    host_calls: u32,
}

struct State {
    options: HudOptions,
    layers: Vec<LayerCpu>,
    gpu: Option<Gpu>,
    output: Option<(u32, u32)>,
    stats: Stats,
    told: Vec<String>,
    failures: u32,
    retry_at: Option<Instant>,
    dead: bool,
    texture_bytes: u64,
    /// (IMPL_ER15, game over) A plain plate over the whole back buffer, drawn right before the layer of this index: the
    /// colour (sRGB, straight alpha) and the moment it lapses unless the host renews it.
    plate: Option<(usize, [f32; 4], Instant)>,
    /// (IMPL_ER21) A picture of the renderer's own (the camp menu's portrait), drawn right before a layer; lapses.
    picture: Option<(Picture, Instant)>,
    /// (ER party S4) the friends' camp-menu pictures (gfx/party.rs), each as `picture`; up to `PARTY_PICTURES`.
    #[cfg(feature = "party")]
    party_pictures: Vec<(Picture, Instant)>,
}

/// (ER party S4) The friends' pictures the overlay holds at most (Donald, Goofy).
#[cfg(feature = "party")]
pub const PARTY_PICTURES: usize = 2;

/// (ER party S4) The friends' camp-menu pictures of this Present (gfx/party.rs); an empty list takes them off at once.
#[cfg(feature = "party")]
pub(crate) fn set_party_pictures(pictures: Vec<Picture>) {
    with(|s| s.party_pictures = pictures.into_iter().take(PARTY_PICTURES).map(|p| (p, Instant::now() + Duration::from_millis(PICTURE_HOLD_MS))).collect());
}

/// (IMPL_ER21) A PICTURE THE 3D PATH RENDERED ITSELF (gfx/portrait.rs: Sora for the camp menu's character panel): a
/// texture of the back buffer's pixel scale (R8G8B8A8, display-encoded, straight alpha, resting in the pixel shader
/// resource state), drawn texel for pixel at `rect` right before layer `before`, with the overlay's opacity times
/// `alpha`. It lapses half a second after the latest hand-over.
#[derive(Clone)]
pub struct Picture {
    pub texture: ID3D12Resource,
    pub before: usize,
    /// (IMPL_ER23B) The layer the picture belongs to (the camp menu's): it is only drawn while that layer is drawn.
    pub with_layer: usize,
    pub rect: PxRect,
    pub alpha: f32,
}

pub(crate) fn set_picture(picture: Option<Picture>) {
    with(|s| s.picture = picture.map(|p| (p, Instant::now() + Duration::from_millis(PICTURE_HOLD_MS))));
}

/// (IMPL_ER23B) How long a picture may be drawn after its latest hand-over (it is handed over at every Present while
/// the host has a portrait: this only bridges a Present in which the portrait could not be drawn). Was 500 ms: the
/// user saw the picture of Sora stay over the world after the camp menu had closed.
pub const PICTURE_HOLD_MS: u64 = 120;

/// (IMPL_ER23B) THE PICTURE'S LIFETIME, decided at every Present: the opacity it is drawn with, or `None` = it is not
/// drawn at all. It lives and dies WITH THE PANEL: the host must still hold a portrait (`host_has_portrait`: the camp
/// menu's movie is up and its panel shows; `None` from the host takes it off at once), its layer (the camp menu's)
/// must be drawn in this very Present with a picture of its own, the hand-over must be fresh, and its opacity is the
/// panel clip's (the menu's open / close animation, from the host) times the overlay's own (its fade under the
/// game's menus).
pub fn picture_opacity(panel_alpha: f32, age_ms: u64, host_has_portrait: bool, layer_drawn: bool, overlay_on: bool, overlay_opacity: f32) -> Option<f32> {
    if !host_has_portrait || !layer_drawn || !overlay_on || age_ms >= PICTURE_HOLD_MS {
        return None;
    }
    let alpha = panel_alpha.clamp(0.0, 1.0) * overlay_opacity.clamp(0.0, 1.0);
    (alpha > 0.002).then_some(alpha)
}

// SAFETY: D3D12 objects are free-threaded; the mapped pointers are only used under the mutex.
unsafe impl Send for State {}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with<T>(f: impl FnOnce(&mut State) -> T) -> Option<T> {
    // (a poisoned lock = a panic of ours while drawing: the overlay stays off)
    let mut guard = STATE.lock().ok()?;
    let state = guard.get_or_insert_with(|| State {
        options: HudOptions::default(),
        layers: (0..MAX_LAYERS).map(|_| LayerCpu { visible: false, rect: (0, 0, 0, 0), pixels: Vec::new(), dirty: None }).collect(),
        gpu: None,
        output: None,
        stats: Stats::default(),
        told: Vec::new(),
        failures: 0,
        retry_at: None,
        dead: false,
        texture_bytes: 0,
        plate: None,
        picture: None,
        #[cfg(feature = "party")]
        party_pictures: Vec::new(),
    });
    Some(f(state))
}

impl State {
    fn tell(&mut self, text: String) {
        if !self.told.contains(&text) {
            rlog!("hud: {text}");
            self.told.push(text);
        }
    }
}

// ---------------------------------------------------------------------------- the host's side

pub fn set_options(options: HudOptions) {
    with(|s| {
        if s.options != options {
            if (s.options.on, s.options.gamma, s.options.linear_scale) != (options.on, options.gamma, options.linear_scale) {
                rlog!("hud: overlay options {options:?}");
            }
            s.options = options;
        }
    });
}

/// The back buffer's size as of the latest Present (`None`: nothing was presented yet). The host rasterises
/// the stage for this size.
pub fn output_size() -> Option<(u32, u32)> {
    with(|s| s.output).flatten()
}

/// Layer `index` shows `rgba` (the whole picture of `rect`: straight alpha, sRGB bytes, `rect.2 * rect.3 * 4`
/// of them); `dirty` = only that rectangle of the back buffer differs from the picture given last for the same
/// `rect`. Any thread; copies what changed.
pub fn show_layer(index: usize, rect: PxRect, rgba: &[u8], dirty: Option<PxRect>) {
    let started = Instant::now();
    with(|s| {
        let Some(layer) = s.layers.get_mut(index) else { return };
        let (w, h) = (rect.2, rect.3);
        if w == 0 || h == 0 || rgba.len() < w * h * 4 {
            return;
        }
        let part = dirty.filter(|_| layer.rect == rect && layer.pixels.len() == w * h * 4).and_then(|d| {
            let (x0, y0) = ((d.0 - rect.0).max(0) as usize, (d.1 - rect.1).max(0) as usize);
            let (x1, y1) = ((x0 + d.2).min(w), (y0 + d.3).min(h));
            (x1 > x0 && y1 > y0).then_some((x0, y0, x1, y1))
        });
        match part {
            Some((x0, y0, x1, y1)) => {
                for y in y0..y1 {
                    let (from, to) = ((y * w + x0) * 4, (y * w + x1) * 4);
                    layer.pixels[from..to].copy_from_slice(&rgba[from..to]);
                }
                layer.dirty = Some(match layer.dirty {
                    Some(d) => (d.0.min(x0), d.1.min(y0), d.2.max(x1), d.3.max(y1)),
                    None => (x0, y0, x1, y1),
                });
            }
            None => {
                // (the buffer only grows: no allocation once the layer has been at its largest)
                layer.pixels.clear();
                layer.pixels.extend_from_slice(&rgba[..w * h * 4]);
                layer.rect = rect;
                layer.dirty = Some((0, 0, w, h));
            }
        }
        layer.visible = true;
        s.stats.host_micros += started.elapsed().as_micros() as u64;
        s.stats.host_calls += 1;
    });
}

/// (IMPL_ER15, game over) A PLATE of one colour over the WHOLE back buffer (sRGB 0..1, straight alpha), drawn right before
/// layer `before` (so under that layer and every later one, over the earlier ones and the game). It is independent of
/// the overlay`s `on` / `opacity` and of the layers being shown, and lapses by itself `hold_ms` after the latest call
/// (a host that stops calling can never leave the screen covered). `None`: off at once.
pub fn set_plate(plate: Option<(usize, [f32; 4], u64)>) {
    with(|s| s.plate = plate.map(|(before, colour, hold_ms)| (before.min(MAX_LAYERS), colour, Instant::now() + Duration::from_millis(hold_ms))));
}

pub fn hide_layer(index: usize) {
    with(|s| {
        if let Some(layer) = s.layers.get_mut(index) {
            layer.visible = false;
        }
    });
}

/// Nothing is shown until the host shows layers again (the player is gone, the HUD was switched off).
pub fn clear() {
    with(|s| {
        for layer in &mut s.layers {
            layer.visible = false;
        }
    });
}

/// One line for the host's log; resets the counters it reports.
pub fn status() -> String {
    with(|s| {
        let stats = std::mem::take(&mut s.stats);
        if !s.options.on {
            return "hud overlay: off".to_string();
        }
        let n = stats.drawn.max(1) as u64;
        format!(
            "hud overlay: {} | {} drawn of {} presents ({} no free slot) | {:.1} quads, {:.1} kB uploaded per frame ({} upload(s) put off) | present thread {} us avg, {} max | host copies {} us total in {} call(s) | textures {:.1} MB{}",
            if s.dead { "DEAD (panic)" } else if s.retry_at.is_some_and(|at| Instant::now() < at) { "PAUSED after a failure" } else { "on" },
            stats.drawn,
            stats.presents,
            stats.busy,
            stats.quads as f32 / n as f32,
            stats.upload_bytes as f32 / 1024.0 / n as f32,
            stats.deferred,
            stats.micros / n,
            stats.micros_max,
            stats.host_micros,
            stats.host_calls,
            s.texture_bytes as f32 / (1 << 20) as f32,
            s.output.map_or(String::new(), |(w, h)| format!(" | back buffer {w}x{h}")),
        )
    })
    .unwrap_or_else(|| "hud overlay: DEAD (panic)".to_string())
}

// ---------------------------------------------------------------------------- the presenting thread

fn heap(kind: D3D12_HEAP_TYPE) -> D3D12_HEAP_PROPERTIES {
    D3D12_HEAP_PROPERTIES { Type: kind, ..Default::default() }
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

fn fail() -> windows::core::Error {
    windows::core::Error::from_hresult(windows::Win32::Foundation::E_FAIL)
}

unsafe fn upload_buffer(device: &ID3D12Device, bytes: usize) -> windows::core::Result<(ID3D12Resource, *mut u8)> {
    let desc = D3D12_RESOURCE_DESC { Dimension: D3D12_RESOURCE_DIMENSION_BUFFER, Width: bytes as u64, Height: 1, DepthOrArraySize: 1, MipLevels: 1, SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 }, Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR, ..Default::default() };
    let mut out: Option<ID3D12Resource> = None;
    device.CreateCommittedResource(&heap(D3D12_HEAP_TYPE_UPLOAD), D3D12_HEAP_FLAG_NONE, &desc, D3D12_RESOURCE_STATE_GENERIC_READ, None, &mut out)?;
    let buffer = out.ok_or_else(fail)?;
    let mut at: *mut c_void = std::ptr::null_mut();
    buffer.Map(0, None, Some(&mut at))?;
    Ok((buffer, at as *mut u8))
}

/// How the pictures go into a back buffer of `format`: the view's format (`UNKNOWN` = the resource's own),
/// the shader's mode (0 display-encoded, 1 linear) and its scale. `None`: not a format we draw into.
fn output_mode(format: DXGI_FORMAT, options: &HudOptions) -> Option<(DXGI_FORMAT, f32, f32)> {
    Some(match format {
        DXGI_FORMAT_R10G10B10A2_UNORM => (format, 0.0, 1.0),
        DXGI_FORMAT_R8G8B8A8_UNORM | DXGI_FORMAT_B8G8R8A8_UNORM if options.gamma => (format, 0.0, 1.0),
        DXGI_FORMAT_R8G8B8A8_UNORM => (DXGI_FORMAT_R8G8B8A8_UNORM_SRGB, 1.0, 1.0),
        DXGI_FORMAT_B8G8R8A8_UNORM => (DXGI_FORMAT_B8G8R8A8_UNORM_SRGB, 1.0, 1.0),
        DXGI_FORMAT_R8G8B8A8_UNORM_SRGB | DXGI_FORMAT_B8G8R8A8_UNORM_SRGB => (format, 1.0, 1.0),
        DXGI_FORMAT_R16G16B16A16_FLOAT => (format, 1.0, options.linear_scale),
        _ => return None,
    })
}

impl Gpu {
    unsafe fn new(device: ID3D12Device, queue: usize) -> windows::core::Result<Gpu> {
        let ranges = [D3D12_DESCRIPTOR_RANGE { RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SRV, NumDescriptors: 1, BaseShaderRegister: 0, RegisterSpace: 0, OffsetInDescriptorsFromTableStart: 0 }];
        let parameters = [
            D3D12_ROOT_PARAMETER { ParameterType: D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS, Anonymous: D3D12_ROOT_PARAMETER_0 { Constants: D3D12_ROOT_CONSTANTS { ShaderRegister: 0, RegisterSpace: 0, Num32BitValues: 16 } }, ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL },
            D3D12_ROOT_PARAMETER { ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE, Anonymous: D3D12_ROOT_PARAMETER_0 { DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE { NumDescriptorRanges: 1, pDescriptorRanges: ranges.as_ptr() } }, ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL },
        ];
        let desc = D3D12_ROOT_SIGNATURE_DESC { NumParameters: parameters.len() as u32, pParameters: parameters.as_ptr(), NumStaticSamplers: 0, pStaticSamplers: std::ptr::null(), Flags: D3D12_ROOT_SIGNATURE_FLAG_NONE };
        let mut blob: Option<ID3DBlob> = None;
        D3D12SerializeRootSignature(&desc, D3D_ROOT_SIGNATURE_VERSION_1, &mut blob, None)?;
        let blob = blob.ok_or_else(fail)?;
        let root: ID3D12RootSignature = device.CreateRootSignature(0, std::slice::from_raw_parts(blob.GetBufferPointer() as *const u8, blob.GetBufferSize()))?;
        let srv_heap: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV, NumDescriptors: (SLOTS * MAX_LAYERS + 1 + SLOTS + PARTY_SLOTS) as u32, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE, NodeMask: 0 })?;
        let rtv_heap: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV, NumDescriptors: SLOTS as u32, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE, NodeMask: 0 })?;
        let fence: ID3D12Fence = device.CreateFence(0, D3D12_FENCE_FLAG_NONE)?;
        let mut slots = Vec::new();
        for _ in 0..SLOTS {
            let allocator: ID3D12CommandAllocator = device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)?;
            let list: ID3D12GraphicsCommandList = device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator, None)?;
            list.Close()?;
            let (upload, mapped) = upload_buffer(&device, UPLOAD_START)?;
            slots.push(Slot { allocator, list, upload, mapped, capacity: UPLOAD_START, busy_until: 0 });
        }
        let (srv_size, rtv_size) = (device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV) as usize, device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV) as usize);
        // (the plate draws no picture: a null view in the heap`s last place is bound for it)
        let null_desc = D3D12_SHADER_RESOURCE_VIEW_DESC { Format: DXGI_FORMAT_R8G8B8A8_UNORM, ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D, Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING, Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 { Texture2D: D3D12_TEX2D_SRV { MostDetailedMip: 0, MipLevels: 1, PlaneSlice: 0, ResourceMinLODClamp: 0.0 } } };
        device.CreateShaderResourceView(None, Some(&null_desc), D3D12_CPU_DESCRIPTOR_HANDLE { ptr: srv_heap.GetCPUDescriptorHandleForHeapStart().ptr + SLOTS * MAX_LAYERS * srv_size });
        Ok(Gpu { device, queue, root, pipelines: HashMap::new(), srv_heap, srv_size, rtv_heap, rtv_size, fence, fence_value: 0, slots, layers: (0..MAX_LAYERS).map(|_| LayerGpu { texture: None, shown: None }).collect(), retired: Vec::new() })
    }

    unsafe fn pipeline(&mut self, view_format: DXGI_FORMAT) -> windows::core::Result<ID3D12PipelineState> {
        if let Some(found) = self.pipelines.get(&view_format.0) {
            return Ok(found.clone());
        }
        let mut blend = D3D12_BLEND_DESC::default();
        blend.RenderTarget[0] = D3D12_RENDER_TARGET_BLEND_DESC {
            BlendEnable: true.into(),
            LogicOpEnable: false.into(),
            SrcBlend: D3D12_BLEND_ONE,
            DestBlend: D3D12_BLEND_INV_SRC_ALPHA,
            BlendOp: D3D12_BLEND_OP_ADD,
            SrcBlendAlpha: D3D12_BLEND_ONE,
            DestBlendAlpha: D3D12_BLEND_INV_SRC_ALPHA,
            BlendOpAlpha: D3D12_BLEND_OP_ADD,
            LogicOp: D3D12_LOGIC_OP_NOOP,
            // (the back buffer's alpha is the game's: left alone)
            RenderTargetWriteMask: (D3D12_COLOR_WRITE_ENABLE_RED.0 | D3D12_COLOR_WRITE_ENABLE_GREEN.0 | D3D12_COLOR_WRITE_ENABLE_BLUE.0) as u8,
        };
        let mut desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            pRootSignature: std::mem::transmute_copy(&self.root),
            VS: D3D12_SHADER_BYTECODE { pShaderBytecode: VS.as_ptr() as *const c_void, BytecodeLength: VS.len() },
            PS: D3D12_SHADER_BYTECODE { pShaderBytecode: PS.as_ptr() as *const c_void, BytecodeLength: PS.len() },
            BlendState: blend,
            SampleMask: u32::MAX,
            RasterizerState: D3D12_RASTERIZER_DESC { FillMode: D3D12_FILL_MODE_SOLID, CullMode: D3D12_CULL_MODE_NONE, DepthClipEnable: false.into(), ..Default::default() },
            DepthStencilState: D3D12_DEPTH_STENCIL_DESC::default(),
            PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
            NumRenderTargets: 1,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            ..Default::default()
        };
        desc.RTVFormats[0] = view_format;
        let made: windows::core::Result<ID3D12PipelineState> = self.device.CreateGraphicsPipelineState(&desc);
        // (the description holds a borrowed root signature: it must not be released through it)
        std::mem::forget(std::mem::replace(&mut desc.pRootSignature, ManuallyDrop::new(None)));
        let made = made?;
        self.pipelines.insert(view_format.0, made.clone());
        Ok(made)
    }
}

/// Waits (at most a second) for the GPU to be done with our lists, then frees the objects.
fn drop_gpu(gpu: Option<Gpu>, wait: bool) {
    let Some(gpu) = gpu else { return };
    // SAFETY: plain getters on our own objects.
    unsafe {
        let waited = Instant::now();
        while wait && gpu.fence.GetCompletedValue() < gpu.fence_value && waited.elapsed() < Duration::from_millis(1000) && gpu.device.GetDeviceRemovedReason().is_ok() {
            std::thread::sleep(Duration::from_millis(1));
        }
        for slot in &gpu.slots {
            slot.upload.Unmap(0, None);
        }
    }
    drop(gpu);
}

/// gfx.rs `present`, after the 3D path's own work: the game is about to present `swap`; `queue` is the
/// game's queue the swap chain presents on.
pub(crate) fn at_present(device: &ID3D12Device, queue: &ID3D12CommandQueue, swap: &IDXGISwapChain3) {
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let Ok(mut guard) = STATE.lock() else { return };
        let Some(state) = guard.as_mut() else { return };
        if state.dead {
            return;
        }
        // SAFETY: the game's live device, queue and swap chain for the duration of its Present call; our own objects.
        let result = unsafe { present(state, device, queue, swap) };
        match result {
            Ok(()) => state.failures = 0,
            Err(e) => {
                state.failures += 1;
                // SAFETY: plain getter.
                let removed = unsafe { device.GetDeviceRemovedReason() };
                state.tell(format!("a frame of the overlay failed ({e}); device removed reason: {removed:?}; it pauses and is tried again"));
                state.retry_at = Some(Instant::now() + Duration::from_secs(if state.failures >= 3 { 10 } else { 2 }));
                if removed.is_err() {
                    drop_gpu(state.gpu.take(), false);
                    forget_uploads(state);
                }
            }
        }
    }));
    if outcome.is_err() {
        rlog!("hud: PANIC while drawing the overlay: it is off for the rest of this run");
        // (the lock is poisoned: every later call of this module returns at once)
    }
}

/// The textures are gone: every visible picture is uploaded whole again.
fn forget_uploads(state: &mut State) {
    for layer in &mut state.layers {
        if !layer.pixels.is_empty() {
            layer.dirty = Some((0, 0, layer.rect.2, layer.rect.3));
        }
    }
    state.texture_bytes = 0;
}

unsafe fn present(state: &mut State, device: &ID3D12Device, queue: &ID3D12CommandQueue, swap: &IDXGISwapChain3) -> windows::core::Result<()> {
    let started = Instant::now();
    let desc = swap.GetDesc1()?;
    state.output = Some((desc.Width, desc.Height));
    state.stats.presents += 1;
    if state.plate.is_some_and(|(_, colour, until)| Instant::now() >= until || colour[3] <= 0.0) {
        state.plate = None;
    }
    let plate = state.plate.map(|(before, colour, _)| (before, colour));
    // (IMPL_ER23B) the portrait's picture goes WITH its panel: see `picture_opacity`
    let host_has_portrait = crate::portrait_now().is_some();
    if state.picture.as_ref().is_some_and(|(p, until)| Instant::now() >= *until || p.alpha <= 0.002 || !host_has_portrait) {
        state.picture = None;
    }
    let picture = state.picture.as_ref().map(|(p, until)| (p.clone(), PICTURE_HOLD_MS.saturating_sub(until.saturating_duration_since(Instant::now()).as_millis() as u64)));
    // (ER party S4) the friends' pictures go with their panels the same way
    #[cfg(feature = "party")]
    let host_has_party = crate::party::portraits_now().is_some();
    #[cfg(feature = "party")]
    state.party_pictures.retain(|(p, until)| Instant::now() < *until && p.alpha > 0.002 && host_has_party);
    #[cfg(feature = "party")]
    let party_pictures: Vec<(Picture, u64)> = state.party_pictures.iter().map(|(p, until)| (p.clone(), PICTURE_HOLD_MS.saturating_sub(until.saturating_duration_since(Instant::now()).as_millis() as u64))).collect();
    let layers_on = state.options.on && state.options.opacity > 0.002 && state.layers.iter().any(|l| l.visible);
    if !layers_on && plate.is_none() {
        return Ok(());
    }
    if state.retry_at.is_some_and(|at| Instant::now() < at) {
        return Ok(());
    }
    device.GetDeviceRemovedReason()?;
    let back: ID3D12Resource = swap.GetBuffer(swap.GetCurrentBackBufferIndex())?;
    let back_desc = back.GetDesc();
    let opacity = state.options.opacity.clamp(0.0, 1.0);
    let Some((view_format, mode, scale)) = output_mode(back_desc.Format, &state.options) else {
        state.tell(format!("the back buffer's format {} is not one the overlay draws into: nothing is drawn", back_desc.Format.0));
        return Ok(());
    };
    if back_desc.Dimension != D3D12_RESOURCE_DIMENSION_TEXTURE2D || back_desc.SampleDesc.Count != 1 || back_desc.Width == 0 || back_desc.Height == 0 {
        state.tell("the back buffer is not a plain 2D target: nothing is drawn".to_string());
        return Ok(());
    }
    // Our objects, on the device and queue that present (made again when either changed).
    if state.gpu.as_ref().is_some_and(|g| g.device.as_raw() != device.as_raw() || g.queue != queue.as_raw() as usize) {
        rlog!("hud: another device or queue presents: the overlay's objects are made again");
        drop_gpu(state.gpu.take(), true);
        forget_uploads(state);
    }
    if state.gpu.is_none() {
        state.gpu = Some(Gpu::new(device.clone(), queue.as_raw() as usize)?);
        forget_uploads(state);
        rlog!("hud: the overlay's Direct3D 12 objects are made (back buffer {}x{} format {}, {})", back_desc.Width, back_desc.Height, back_desc.Format.0, if mode == 0.0 { "display-encoded blending" } else { "linear blending" });
    }
    let State { gpu, layers, stats, texture_bytes, .. } = state;
    let gpu = gpu.as_mut().unwrap();
    let pipeline = gpu.pipeline(view_format)?;
    let completed = gpu.fence.GetCompletedValue();
    gpu.retired.retain(|(value, _)| *value > completed);
    let Some(slot_index) = (0..gpu.slots.len()).find(|k| gpu.slots[*k].busy_until <= completed) else {
        stats.busy += 1;
        return Ok(());
    };

    // ---- what is uploaded this frame: (layer, x, y, width, height, offset in the upload buffer, row pitch)
    let mut copies: [(usize, usize, usize, usize, usize, usize, usize); MAX_LAYERS] = [(0, 0, 0, 0, 0, 0, 0); MAX_LAYERS];
    let mut copy_count = 0;
    let mut offset = 0usize;
    for (index, layer) in layers.iter_mut().enumerate() {
        let Some((x0, y0, x1, y1)) = layer.dirty else { continue };
        if !layer.visible {
            continue;
        }
        let (w, h) = (layer.rect.2, layer.rect.3);
        // The texture: at least the picture's size, in steps of 64 (it only grows).
        let fits = gpu.layers[index].texture.as_ref().is_some_and(|(_, tw, th)| *tw as usize >= w && *th as usize >= h);
        let (x0, y0, x1, y1) = if fits { (x0, y0, x1, y1) } else { (0, 0, w, h) };
        let pitch = ((x1 - x0) * 4).next_multiple_of(D3D12_TEXTURE_DATA_PITCH_ALIGNMENT as usize);
        let bytes = pitch * (y1 - y0);
        let at = offset.next_multiple_of(D3D12_TEXTURE_DATA_PLACEMENT_ALIGNMENT as usize);
        let slot = &mut gpu.slots[slot_index];
        if at + bytes > slot.capacity {
            if copy_count == 0 && bytes <= UPLOAD_MAX {
                // The slot's buffer grows to hold it (the old one may still be read by the GPU).
                let (upload, mapped) = upload_buffer(&gpu.device, bytes.next_multiple_of(1 << 20))?;
                let old = std::mem::replace(&mut slot.upload, upload);
                old.Unmap(0, None);
                gpu.retired.push((gpu.fence_value, old));
                slot.mapped = mapped;
                slot.capacity = bytes.next_multiple_of(1 << 20);
            } else {
                // Next frame (the texture keeps what it shows meanwhile).
                stats.deferred += 1;
                continue;
            }
        }
        if !fits {
            let (tw, th) = ((w.next_multiple_of(64)) as u32, (h.next_multiple_of(64)) as u32);
            let desc = D3D12_RESOURCE_DESC { Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D, Width: tw as u64, Height: th, DepthOrArraySize: 1, MipLevels: 1, Format: DXGI_FORMAT_R8G8B8A8_UNORM, SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 }, ..Default::default() };
            let mut out: Option<ID3D12Resource> = None;
            gpu.device.CreateCommittedResource(&heap(D3D12_HEAP_TYPE_DEFAULT), D3D12_HEAP_FLAG_NONE, &desc, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, None, &mut out)?;
            *texture_bytes += tw as u64 * th as u64 * 4;
            if let Some((old, ow, oh)) = gpu.layers[index].texture.replace((out.ok_or_else(fail)?, tw, th)) {
                *texture_bytes -= (ow as u64 * oh as u64 * 4).min(*texture_bytes);
                gpu.retired.push((gpu.fence_value, old));
            }
            gpu.layers[index].shown = None;
        }
        let slot = &gpu.slots[slot_index];
        for row in 0..(y1 - y0) {
            let from = ((y0 + row) * w + x0) * 4;
            std::ptr::copy_nonoverlapping(layer.pixels.as_ptr().add(from), slot.mapped.add(at + row * pitch), (x1 - x0) * 4);
        }
        copies[copy_count] = (index, x0, y0, x1 - x0, y1 - y0, at, pitch);
        copy_count += 1;
        offset = at + bytes;
        stats.upload_bytes += bytes as u64;
        layer.dirty = None;
        gpu.layers[index].shown = Some(layer.rect);
    }

    // ---- our list
    let slot = &mut gpu.slots[slot_index];
    slot.allocator.Reset()?;
    slot.list.Reset(&slot.allocator, None)?;
    let list = &slot.list;
    for (index, x, y, w, h, at, pitch) in &copies[..copy_count] {
        let Some((texture, ..)) = &gpu.layers[*index].texture else { continue };
        list.ResourceBarrier(&[transition(texture, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, D3D12_RESOURCE_STATE_COPY_DEST)]);
        let from = D3D12_TEXTURE_COPY_LOCATION {
            pResource: std::mem::transmute_copy(&slot.upload),
            Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
            Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { PlacedFootprint: D3D12_PLACED_SUBRESOURCE_FOOTPRINT { Offset: *at as u64, Footprint: D3D12_SUBRESOURCE_FOOTPRINT { Format: DXGI_FORMAT_R8G8B8A8_UNORM, Width: *w as u32, Height: *h as u32, Depth: 1, RowPitch: *pitch as u32 } } },
        };
        let to = D3D12_TEXTURE_COPY_LOCATION { pResource: std::mem::transmute_copy(texture), Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX, Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { SubresourceIndex: 0 } };
        list.CopyTextureRegion(&to, *x as u32, *y as u32, 0, &from, None);
        list.ResourceBarrier(&[transition(texture, D3D12_RESOURCE_STATE_COPY_DEST, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE)]);
    }
    let (width, height) = (back_desc.Width as u32, back_desc.Height);
    let target_view = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: gpu.rtv_heap.GetCPUDescriptorHandleForHeapStart().ptr + slot_index * gpu.rtv_size };
    let view_desc = D3D12_RENDER_TARGET_VIEW_DESC { Format: view_format, ViewDimension: D3D12_RTV_DIMENSION_TEXTURE2D, ..Default::default() };
    gpu.device.CreateRenderTargetView(&back, Some(&view_desc), target_view);
    list.ResourceBarrier(&[transition(&back, D3D12_RESOURCE_STATE_PRESENT, D3D12_RESOURCE_STATE_RENDER_TARGET)]);
    list.OMSetRenderTargets(1, Some(&target_view), false, None);
    list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: width as f32, Height: height as f32, MinDepth: 0.0, MaxDepth: 1.0 }]);
    list.RSSetScissorRects(&[windows::Win32::Foundation::RECT { left: 0, top: 0, right: width as i32, bottom: height as i32 }]);
    list.SetGraphicsRootSignature(&gpu.root);
    list.SetDescriptorHeaps(&[Some(gpu.srv_heap.clone())]);
    list.SetPipelineState(&pipeline);
    list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP);
    let (cpu_start, gpu_start) = (gpu.srv_heap.GetCPUDescriptorHandleForHeapStart(), gpu.srv_heap.GetGPUDescriptorHandleForHeapStart());
    let mut quads = 0;
    let draw_plate = |colour: [f32; 4]| {
        let constants: [f32; 16] = [0.0, 0.0, width as f32, height as f32, width as f32, height as f32, mode, scale, 1.0, 1.0, 0.0, 0.0, colour[0], colour[1], colour[2], colour[3].clamp(0.0, 1.0)];
        list.SetGraphicsRoot32BitConstants(0, 16, constants.as_ptr() as *const c_void, 0);
        list.SetGraphicsRootDescriptorTable(1, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: gpu_start.ptr + (SLOTS * MAX_LAYERS * gpu.srv_size) as u64 });
        list.DrawInstanced(4, 1, 0, 0);
    };
    for (index, layer) in layers.iter().enumerate() {
        if let Some((_, colour)) = plate.filter(|(before, _)| *before == index) {
            draw_plate(colour);
            quads += 1;
        }
        if !layers_on {
            continue;
        }
        // (IMPL_ER21) the renderer's own picture (the camp menu's portrait) under this layer and over the earlier ones
        let picture_now = picture.as_ref().filter(|(p, _)| p.before == index && p.rect.2 > 0 && p.rect.3 > 0).and_then(|(p, age)| {
            let layer_drawn = layers.get(p.with_layer).is_some_and(|l| l.visible) && gpu.layers.get(p.with_layer).is_some_and(|l| l.texture.is_some() && l.shown.is_some());
            picture_opacity(p.alpha, *age, host_has_portrait, layer_drawn, layers_on, opacity).map(|alpha| (p, alpha))
        });
        if let Some((p, picture_alpha)) = picture_now {
            let k = SLOTS * MAX_LAYERS + 1 + slot_index;
            gpu.device.CreateShaderResourceView(&p.texture, None, D3D12_CPU_DESCRIPTOR_HANDLE { ptr: cpu_start.ptr + k * gpu.srv_size });
            let constants: [f32; 16] = [p.rect.0 as f32, p.rect.1 as f32, (p.rect.0 + p.rect.2 as i32) as f32, (p.rect.1 + p.rect.3 as i32) as f32, width as f32, height as f32, mode, scale, picture_alpha, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
            list.SetGraphicsRoot32BitConstants(0, 16, constants.as_ptr() as *const c_void, 0);
            list.SetGraphicsRootDescriptorTable(1, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: gpu_start.ptr + (k * gpu.srv_size) as u64 });
            list.DrawInstanced(4, 1, 0, 0);
            quads += 1;
        }
        // (ER party S4) the friends' pictures (their camp panels), each with a descriptor of its own per frame slot
        #[cfg(feature = "party")]
        for (n, (p, age)) in party_pictures.iter().enumerate().filter(|(_, (p, _))| p.before == index && p.rect.2 > 0 && p.rect.3 > 0) {
            let layer_drawn = layers.get(p.with_layer).is_some_and(|l| l.visible) && gpu.layers.get(p.with_layer).is_some_and(|l| l.texture.is_some() && l.shown.is_some());
            let Some(picture_alpha) = picture_opacity(p.alpha, *age, host_has_party, layer_drawn, layers_on, opacity) else { continue };
            let k = SLOTS * MAX_LAYERS + 1 + SLOTS + slot_index * PARTY_PICTURES + n;
            gpu.device.CreateShaderResourceView(&p.texture, None, D3D12_CPU_DESCRIPTOR_HANDLE { ptr: cpu_start.ptr + k * gpu.srv_size });
            let constants: [f32; 16] = [p.rect.0 as f32, p.rect.1 as f32, (p.rect.0 + p.rect.2 as i32) as f32, (p.rect.1 + p.rect.3 as i32) as f32, width as f32, height as f32, mode, scale, picture_alpha, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
            list.SetGraphicsRoot32BitConstants(0, 16, constants.as_ptr() as *const c_void, 0);
            list.SetGraphicsRootDescriptorTable(1, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: gpu_start.ptr + (k * gpu.srv_size) as u64 });
            list.DrawInstanced(4, 1, 0, 0);
            quads += 1;
        }
        let (true, Some((texture, ..)), Some(rect)) = (layer.visible, &gpu.layers[index].texture, gpu.layers[index].shown) else { continue };
        // (the slot's own descriptors: written every frame, never while a list that reads them is in flight)
        let k = slot_index * MAX_LAYERS + index;
        gpu.device.CreateShaderResourceView(texture, None, D3D12_CPU_DESCRIPTOR_HANDLE { ptr: cpu_start.ptr + k * gpu.srv_size });
        let constants: [f32; 16] = [rect.0 as f32, rect.1 as f32, (rect.0 + rect.2 as i32) as f32, (rect.1 + rect.3 as i32) as f32, width as f32, height as f32, mode, scale, opacity, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        list.SetGraphicsRoot32BitConstants(0, 16, constants.as_ptr() as *const c_void, 0);
        list.SetGraphicsRootDescriptorTable(1, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: gpu_start.ptr + (k * gpu.srv_size) as u64 });
        list.DrawInstanced(4, 1, 0, 0);
        quads += 1;
    }
    if let Some((_, colour)) = plate.filter(|(before, _)| *before >= MAX_LAYERS) {
        draw_plate(colour);
        quads += 1;
    }
    list.ResourceBarrier(&[transition(&back, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_PRESENT)]);
    list.Close()?;
    queue.ExecuteCommandLists(&[Some(list.cast::<ID3D12CommandList>()?)]);
    gpu.fence_value += 1;
    queue.Signal(&gpu.fence, gpu.fence_value)?;
    slot.busy_until = gpu.fence_value;
    let micros = started.elapsed().as_micros() as u64;
    stats.drawn += 1;
    stats.quads += quads;
    stats.micros += micros;
    stats.micros_max = stats.micros_max.max(micros);
    Ok(())
}

/// gfx.rs `release`: frees the overlay's objects (the host goes away).
pub(crate) fn release() {
    if let Ok(mut guard) = STATE.lock() {
        if let Some(state) = guard.as_mut() {
            drop_gpu(state.gpu.take(), true);
        }
        *guard = None;
    }
}

#[cfg(test)]
mod picture_tests {
    use super::{picture_opacity, PICTURE_HOLD_MS};

    /// (IMPL_ER23B) The camp menu's portrait appears and disappears WITH its panel.
    #[test]
    fn the_portrait_lives_and_dies_with_the_camp_menus_panel() {
        // OPEN: the host hands a portrait over every frame, the camp menu's layer is drawn: visible, with the panel's alpha
        assert_eq!(picture_opacity(1.0, 0, true, true, true, 1.0), Some(1.0));
        // the opening animation: the panel's alpha, frame by frame
        for alpha in [0.09f32, 0.4, 0.75, 0.98] {
            assert_eq!(picture_opacity(alpha, 0, true, true, true, 1.0), Some(alpha));
        }
        // CLOSING: the alpha follows the panel's down to nothing
        let mut last = 1.0;
        for alpha in [0.92f32, 0.67, 0.38, 0.14] {
            let drawn = picture_opacity(alpha, 0, true, true, true, 1.0).unwrap();
            assert!(drawn == alpha && drawn < last);
            last = drawn;
        }
        assert_eq!(picture_opacity(0.001, 0, true, true, true, 1.0), None);
        // CLOSED: the host took the portrait off (the menu's movie is gone): NO DRAW, at once, whatever the picture held
        assert_eq!(picture_opacity(1.0, 0, false, true, true, 1.0), None);
        assert_eq!(picture_opacity(0.38, 16, false, false, true, 1.0), None);
        // the camp menu's layer is not drawn (hidden by B / Start / the Title Screen entry / a load / Sora's frames stopping,
        // or its first picture has not arrived): no draw even if the host still hands a portrait over
        assert_eq!(picture_opacity(1.0, 0, true, false, true, 1.0), None);
        // the overlay is off, or faded out under one of the game's own menus: the picture fades with it
        assert_eq!(picture_opacity(1.0, 0, true, true, false, 1.0), None);
        assert_eq!(picture_opacity(1.0, 0, true, true, true, 0.5), Some(0.5));
        assert_eq!(picture_opacity(1.0, 0, true, true, true, 0.0), None);
        // a hand-over that is not renewed lapses within a few frames (it was half a second)
        assert!(picture_opacity(1.0, PICTURE_HOLD_MS - 1, true, true, true, 1.0).is_some());
        assert_eq!(picture_opacity(1.0, PICTURE_HOLD_MS, true, true, true, 1.0), None);
        assert!(PICTURE_HOLD_MS <= 150);
    }
}
