//! (FREEZE-FX, the user 2026-11-07: "make sure frozen effects are applied to anything that gets frozen by blizzard") THE ICE
//! SHELL OF A FROZEN ELDEN RING BODY. KH3 encases its frozen characters in ice through their own materials (the "Ice" drive of
//! `vs_abn_frz_in` on every KH3 character material); an Elden Ring body's materials are the game's, so the encasing is drawn
//! by us: its OWN POSED MODEL (the triangles kh-er skins from its FLVER and the game's bones, as Zettaflare's draw does:
//! zdraw.rs) over it, with KH3's ice response (shaders/er_iceshell.wgsl: KH3's ice tile of the LibraryTiling16a atlas, its
//! ice colour, its fresnel share), by the amount the host ramps as KH3 ramps "Ice" (0 -> 1 over 0.3 s, back over 0.1 s at
//! the break). Generic: any model the reader can read; a body that cannot be read keeps KH3's particle look alone (the host
//! logs it).
//!
//! Drawn once a frame right after our effects (gfx.rs, after `vfx::draw`), into the scene target the effects are drawn into,
//! tested against the game's depth (reversed z) and lifted toward the eye, blended over the body. Its files: materials/er/
//! iceshell.vs.dxil + iceshell.ps.dxil (kh-shaderc `--only iceshell`); without them nothing is drawn (logged once). The ice
//! tile's texels are read once from the pack's own textures (materials/textures/LibraryTiling16a_Mask.png / _normal.png) on a
//! thread of their own.

use std::ffi::c_void;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use windows::core::{Interface, PCSTR};
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;

/// The look the host hands over (sheet kh3-freeze-look `shell.*`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShellLook {
    /// The ice tile of the 16 x 16 atlas: its uv offset and size (0.78125, 0.15625, 0.0625).
    pub tile_uv: [f32; 3],
    pub tile_cm: f32,
    pub facing_share: f32,
    pub emissive: f32,
    pub light_wrap: f32,
    pub lift_cm: f32,
    pub colour_slope: [f32; 3],
    pub colour_base: [f32; 3],
}

/// One frozen body this frame: its id, the ice amount (0..1), its triangles (game cm, xyz + w unused).
pub type ShellBody = (u32, f32, Vec<[f32; 4]>);

struct Frame {
    at: Instant,
    bodies: Vec<ShellBody>,
    look: ShellLook,
}

static FRAME: Mutex<Option<Frame>> = Mutex::new(None);
/// A frame's bodies older than this are not drawn (the host stopped handing them over).
const STALE: Duration = Duration::from_millis(500);

/// The host's bodies of this frame (an empty list: nothing frozen).
pub fn set_frame(bodies: Vec<ShellBody>, look: ShellLook) {
    if let Ok(mut f) = FRAME.lock() {
        *f = Some(Frame { at: Instant::now(), bodies, look });
    }
}

/// How many shells were drawn (frames), for the hosts' logs.
pub static DRAWN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

// ------------------------------------------------------------------------------------------------ the ice tile (KH3's texels)

/// The ice tile's side (texels): the 2048 atlas / 16.
const TILE: u32 = 128;

/// The tile: (normal x, normal y, mask alpha, 0) per texel, or the reason it cannot be had.
static TILE_DATA: Mutex<Option<Result<Vec<[f32; 4]>, String>>> = Mutex::new(None);
static TILE_ASKED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Cuts the tile at `uv` (offset, size) out of the two atlases (RGBA8, `side` x `side`): normal xy from `normal`, alpha of `mask`.
pub fn cut_tile(mask: &[u8], normal: &[u8], side: u32, uv: [f32; 3]) -> Vec<[f32; 4]> {
    let (x0, y0) = ((uv[0] * side as f32).round() as u32, (uv[1] * side as f32).round() as u32);
    let n = ((uv[2] * side as f32).round() as u32).max(1);
    let mut out = Vec::with_capacity((n * n) as usize);
    for y in 0..n {
        for x in 0..n {
            let at = (((y0 + y).min(side - 1) * side + (x0 + x).min(side - 1)) * 4) as usize;
            let (m, nr) = (mask.get(at..at + 4).unwrap_or(&[0; 4]), normal.get(at..at + 4).unwrap_or(&[128, 128, 255, 255]));
            out.push([nr[0] as f32 / 255.0, nr[1] as f32 / 255.0, m[3] as f32 / 255.0, 0.0]);
        }
    }
    out
}

fn ask_tile(uv: [f32; 3]) {
    if TILE_ASKED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    let Some(pack) = crate::SHARED.lock().ok().and_then(|g| g.as_ref().map(|s| s.pack.clone())) else {
        TILE_ASKED.store(false, std::sync::atomic::Ordering::Relaxed);
        return;
    };
    let spawned = std::thread::Builder::new().name("kh-iceshell-tile".into()).spawn(move || {
        let dir = pack.join("materials").join("textures");
        let read = |name: &str| crate::texture::Pixels::load_png(&dir.join(name), false).map(|p| (p.width, p.data));
        let result = match (read("LibraryTiling16a_Mask.png"), read("LibraryTiling16a_normal.png")) {
            (Ok((w, m)), Ok((w2, n))) if w == w2 && w > 0 => Ok(cut_tile(&m, &n, w, uv)),
            (Ok(_), Ok(_)) => Err("the two atlases differ in size".to_string()),
            (Err(e), _) | (_, Err(e)) => Err(e),
        };
        match &result {
            Ok(t) => crate::rlog!("render: (FREEZE-FX) the ice shell's tile: KH3's LibraryTiling16a ice tile, {} texels", t.len()),
            Err(e) => crate::rlog!("render: (FREEZE-FX) the ice shell's tile cannot be read ({e}): no ice shells"),
        }
        if let Ok(mut t) = TILE_DATA.lock() {
            *t = Some(result);
        }
    });
    if spawned.is_err() {
        TILE_ASKED.store(false, std::sync::atomic::Ordering::Relaxed);
    }
}

// ------------------------------------------------------------------------------------------------ the GPU side

struct Ring {
    buffer: ID3D12Resource,
    mapped: *mut u8,
    bytes: usize,
}

struct Gpu {
    device: usize,
    root: ID3D12RootSignature,
    vs: Vec<u8>,
    ps: Vec<u8>,
    /// (target format, depth format) -> its pipeline
    states: Vec<((i32, i32), ID3D12PipelineState)>,
    tile: Option<ID3D12Resource>,
    rings: Vec<Option<Ring>>,
}

// SAFETY: used under its lock on the submission thread only; `mapped` points into the ring's own buffer.
unsafe impl Send for Gpu {}

static GPU: Mutex<Option<Gpu>> = Mutex::new(None);
static FAILED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
const SLOTS: usize = 4;
const BLOCK: usize = 256;

fn shader(name: &str) -> Option<Vec<u8>> {
    if let Some(bytes) = std::env::var_os("KH_ICESHELL_SHADERS").and_then(|d| std::fs::read(std::path::Path::new(&d).join(name)).ok()) {
        return Some(bytes);
    }
    let pack = crate::SHARED.lock().ok().and_then(|g| g.as_ref().map(|s| s.pack.clone()))?;
    std::fs::read(pack.join("materials").join("er").join(name)).ok()
}

unsafe fn upload(device: &ID3D12Device, bytes: usize) -> windows::core::Result<Ring> {
    let bytes = bytes.next_power_of_two().max(1 << 16);
    let desc = D3D12_RESOURCE_DESC { Dimension: D3D12_RESOURCE_DIMENSION_BUFFER, Width: bytes as u64, Height: 1, DepthOrArraySize: 1, MipLevels: 1, SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 }, Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR, ..Default::default() };
    let mut out: Option<ID3D12Resource> = None;
    device.CreateCommittedResource(&D3D12_HEAP_PROPERTIES { Type: D3D12_HEAP_TYPE_UPLOAD, ..Default::default() }, D3D12_HEAP_FLAG_NONE, &desc, D3D12_RESOURCE_STATE_GENERIC_READ, None, &mut out)?;
    let buffer = out.ok_or_else(|| windows::core::Error::from_hresult(windows::Win32::Foundation::E_FAIL))?;
    let mut at: *mut c_void = std::ptr::null_mut();
    buffer.Map(0, None, Some(&mut at))?;
    Ok(Ring { buffer, mapped: at as *mut u8, bytes })
}

unsafe fn make(device: &ID3D12Device) -> Result<Gpu, String> {
    let (Some(vs), Some(ps)) = (shader("iceshell.vs.dxil"), shader("iceshell.ps.dxil")) else { return Err("materials/er/iceshell.vs.dxil / iceshell.ps.dxil missing".to_string()) };
    let params = [
        D3D12_ROOT_PARAMETER { ParameterType: D3D12_ROOT_PARAMETER_TYPE_CBV, Anonymous: D3D12_ROOT_PARAMETER_0 { Descriptor: D3D12_ROOT_DESCRIPTOR { ShaderRegister: 0, RegisterSpace: 0 } }, ShaderVisibility: D3D12_SHADER_VISIBILITY_ALL },
        D3D12_ROOT_PARAMETER { ParameterType: D3D12_ROOT_PARAMETER_TYPE_SRV, Anonymous: D3D12_ROOT_PARAMETER_0 { Descriptor: D3D12_ROOT_DESCRIPTOR { ShaderRegister: 0, RegisterSpace: 0 } }, ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL },
    ];
    let desc = D3D12_ROOT_SIGNATURE_DESC { NumParameters: params.len() as u32, pParameters: params.as_ptr(), NumStaticSamplers: 0, pStaticSamplers: std::ptr::null(), Flags: D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT };
    let mut blob: Option<ID3DBlob> = None;
    D3D12SerializeRootSignature(&desc, D3D_ROOT_SIGNATURE_VERSION_1, &mut blob, None).map_err(|e| format!("root signature: {e}"))?;
    let blob = blob.ok_or("no root signature")?;
    let root: ID3D12RootSignature = device.CreateRootSignature(0, std::slice::from_raw_parts(blob.GetBufferPointer() as *const u8, blob.GetBufferSize())).map_err(|e| format!("root signature: {e}"))?;
    Ok(Gpu { device: device.as_raw() as usize, root, vs, ps, states: Vec::new(), tile: None, rings: (0..SLOTS).map(|_| None).collect() })
}

unsafe fn state(g: &mut Gpu, device: &ID3D12Device, target: DXGI_FORMAT, depth: DXGI_FORMAT) -> Option<ID3D12PipelineState> {
    if let Some((_, s)) = g.states.iter().find(|(k, _)| *k == (target.0, depth.0)) {
        return Some(s.clone());
    }
    let element = [D3D12_INPUT_ELEMENT_DESC { SemanticName: PCSTR(c"LOC".as_ptr().cast()), SemanticIndex: 0, Format: DXGI_FORMAT_R32G32B32A32_FLOAT, InputSlot: 0, AlignedByteOffset: 0, InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA, InstanceDataStepRate: 0 }];
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
    let mut formats = [DXGI_FORMAT_UNKNOWN; 8];
    formats[0] = target;
    let pso = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
        pRootSignature: std::mem::transmute_copy(&g.root),
        VS: D3D12_SHADER_BYTECODE { pShaderBytecode: g.vs.as_ptr() as *const c_void, BytecodeLength: g.vs.len() },
        PS: D3D12_SHADER_BYTECODE { pShaderBytecode: g.ps.as_ptr() as *const c_void, BytecodeLength: g.ps.len() },
        BlendState: blend,
        SampleMask: u32::MAX,
        RasterizerState: D3D12_RASTERIZER_DESC { FillMode: D3D12_FILL_MODE_SOLID, CullMode: D3D12_CULL_MODE_NONE, DepthClipEnable: true.into(), ..Default::default() },
        // the game's depth, reversed z (as our effects test it: vfx.rs), never written
        DepthStencilState: D3D12_DEPTH_STENCIL_DESC { DepthEnable: true.into(), DepthWriteMask: D3D12_DEPTH_WRITE_MASK_ZERO, DepthFunc: D3D12_COMPARISON_FUNC_GREATER_EQUAL, StencilEnable: false.into(), ..Default::default() },
        InputLayout: D3D12_INPUT_LAYOUT_DESC { pInputElementDescs: element.as_ptr(), NumElements: 1 },
        PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
        NumRenderTargets: 1,
        RTVFormats: formats,
        DSVFormat: depth,
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        ..Default::default()
    };
    match device.CreateGraphicsPipelineState::<ID3D12PipelineState>(&pso) {
        Ok(s) => {
            g.states.push(((target.0, depth.0), s.clone()));
            Some(s)
        }
        Err(e) => {
            crate::rlog!("render: (FREEZE-FX) the ice shell's pipeline cannot be made for formats {} / {} ({e})", target.0, depth.0);
            None
        }
    }
}

/// (gfx.rs, right after `vfx::draw`: the scene target and the game's depth bound by the caller as the effects had them) The
/// frozen bodies' ice shells. Returns true when something was drawn: the caller binds its own root signature, heaps,
/// pipeline and targets again.
pub(crate) unsafe fn draw(cx: &crate::vfx::DrawCx) -> bool {
    if cx.slot >= SLOTS || FAILED.load(std::sync::atomic::Ordering::Relaxed) {
        return false;
    }
    let (bodies, look) = {
        let Ok(guard) = FRAME.lock() else { return false };
        let Some(f) = guard.as_ref().filter(|f| f.at.elapsed() < STALE) else { return false };
        let bodies: Vec<ShellBody> = f.bodies.iter().filter(|b| b.1 > 0.0 && !b.2.is_empty()).cloned().collect();
        (bodies, f.look)
    };
    if bodies.is_empty() {
        return false;
    }
    ask_tile(look.tile_uv);
    let tile = match TILE_DATA.lock().ok().and_then(|t| t.clone()) {
        Some(Ok(t)) => t,
        _ => return false,
    };
    let Ok(mut guard) = GPU.lock() else { return false };
    if guard.as_ref().is_some_and(|g| g.device != cx.device.as_raw() as usize) {
        *guard = None;
    }
    if guard.is_none() {
        match make(cx.device) {
            Ok(g) => {
                crate::rlog!("render: (FREEZE-FX) the ice shell pass is made: frozen Elden Ring bodies drawn from their own models with KH3's ice");
                *guard = Some(g);
            }
            Err(e) => {
                crate::rlog!("render: (FREEZE-FX) the ice shell pass cannot be made ({e}): frozen bodies keep KH3's particle look only");
                FAILED.store(true, std::sync::atomic::Ordering::Relaxed);
                return false;
            }
        }
    }
    let Some(g) = guard.as_mut() else { return false };
    if g.tile.is_none() {
        let bytes: Vec<u8> = tile.iter().flat_map(|t| t.iter().flat_map(|v| v.to_le_bytes())).collect();
        match upload(cx.device, bytes.len()) {
            Ok(r) => {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), r.mapped, bytes.len());
                g.tile = Some(r.buffer);
            }
            Err(_) => return false,
        }
    }
    let target_format = cx.target.GetDesc().Format;
    let Some(pso) = state(g, cx.device, target_format, cx.depth_format) else { return false };
    let verts: usize = bodies.iter().map(|b| b.2.len()).sum();
    let need = bodies.len() * BLOCK + verts * 16;
    if g.rings[cx.slot].as_ref().is_none_or(|r| r.bytes < need) {
        g.rings[cx.slot] = upload(cx.device, need).ok();
    }
    let (Some(r), Some(tile_buffer)) = (g.rings[cx.slot].as_ref(), g.tile.as_ref()) else { return false };
    let base = r.buffer.GetGPUVirtualAddress();
    let list = cx.list;
    list.SetGraphicsRootSignature(&g.root);
    list.SetPipelineState(&pso);
    list.SetGraphicsRootShaderResourceView(1, tile_buffer.GetGPUVirtualAddress());
    list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
    list.OMSetRenderTargets(1, Some(&cx.target_view), false, Some(&cx.depth_view));
    let light = (cx.up * 0.8 - cx.forward * 0.5 + cx.right * 0.3).normalize_or_zero();
    let mut at = bodies.len() * BLOCK;
    for (k, (_, ice, soup)) in bodies.iter().enumerate() {
        let mut block: Vec<f32> = cx.clip_from_game.to_cols_array().to_vec();
        block.extend_from_slice(&[cx.camera.x, cx.camera.y, cx.camera.z, ice.clamp(0.0, 1.0)]);
        block.extend_from_slice(&[light.x, light.y, light.z, look.light_wrap]);
        block.extend_from_slice(&[TILE as f32, look.tile_cm, look.facing_share, look.emissive]);
        block.extend_from_slice(&[look.colour_slope[0], look.colour_slope[1], look.colour_slope[2], cx.scale.max(1e-4)]);
        block.extend_from_slice(&[look.colour_base[0], look.colour_base[1], look.colour_base[2], look.lift_cm]);
        std::ptr::copy_nonoverlapping(block.as_ptr() as *const u8, r.mapped.add(k * BLOCK), block.len() * 4);
        std::ptr::copy_nonoverlapping(soup.as_ptr() as *const u8, r.mapped.add(at), soup.len() * 16);
        list.SetGraphicsRootConstantBufferView(0, base + (k * BLOCK) as u64);
        list.IASetVertexBuffers(0, Some(&[D3D12_VERTEX_BUFFER_VIEW { BufferLocation: base + at as u64, SizeInBytes: (soup.len() * 16) as u32, StrideInBytes: 16 }]));
        list.DrawInstanced(soup.len() as u32, 1, 0, 0);
        at += soup.len() * 16;
    }
    DRAWN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tile is cut out of the atlases at KH3's offset: normal xy from the normal atlas, the mask's alpha.
    #[test]
    fn iceshell_cuts_kh3s_ice_tile() {
        let side = 32u32;
        let mut mask = vec![0u8; (side * side * 4) as usize];
        let mut normal = vec![0u8; (side * side * 4) as usize];
        // KH3's offset (0.78125, 0.15625) = (12.5, 2.5) tiles of the 16 x 16 atlas: on a side of 32, texels x 25..27, y 5..7
        for y in 5..7 {
            for x in 25..27 {
                let at = ((y * side + x) * 4) as usize;
                mask[at + 3] = 255;
                normal[at] = 255;
                normal[at + 1] = 51;
            }
        }
        let t = cut_tile(&mask, &normal, side, [0.78125, 0.15625, 0.0625]);
        assert_eq!(t.len(), 4);
        assert!(t.iter().all(|v| v[2] == 1.0 && v[0] == 1.0 && (v[1] - 0.2).abs() < 1e-6));
    }
}
