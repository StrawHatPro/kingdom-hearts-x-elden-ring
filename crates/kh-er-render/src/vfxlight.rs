//! (PARTY-NATIVE-LIGHT) ELDEN RING'S OWN LIGHT ON THE KH LIT TRANSLUCENT EFFECTS (the user 2026-11-05: "literally the lighting
//! code needs to work for ALL KH objects"; "no estimating, it MUST be Elden Ring's lighting"). The native mode only
//! (`crate::hybrid::native_now()`); the Kingdom Hearts 3 style (`r.hybrid_native 0`) never builds or draws any of it.
//!
//! The forward-lit KH effects (the smoke implit / sublit owners: tools/sora_vfx_shaders.js `fwd`, KH3's UE 4.17 forward
//! lighting) were lit by fwdlight.rs: the game's sun CONSTANTS with the visibility at Sora's feet, our own KH light list and a
//! sky cube. Here they take what Elden Ring's OWN lit particles take (build/sora_tmp/weapons/PARTY-NATIVE/ffx/FFX_FINDINGS.md 4;
//! `GS_VFX_StandardParticleBumpLight` / `PS_VFX_SingleTextureAllLight`), at each effect pixel:
//! - the L1 SH of the particle pass's two irradiance volumes (or, in a frame without a particle draw, the deferred irradiance
//!   regions' volumes) + the tile-clustered point / spot lights with ER's own attenuation;
//! - the sun: its colour as read (FC_DirLight Col_0) x its shadow from the game's own cascades at the pixel x N.L.
//!
//! This kit copies the inputs each frame at the start of the effects' late stage (as vfx.rs `FogKit` copies the fog), with a
//! root signature shaped like the GX one at the pixel stage and the game's views heap (shaders/er_fxlit.hlsl, `kh-shaderc --only
//! fxlit`): the constants into a rows target, the volumes slice by slice into 3D textures of ours, the four light buffers into
//! one list texture. The effects' lit variants `l<pixel file>` (er_vfx.wgsl KH_FXLIT, `kh-shaderc --vfx --only fxlitvars`) read
//! them at t18..t29 of the frame table.

use std::ffi::c_void;
use std::path::Path;
use std::sync::Mutex;

use windows::core::Interface;
use windows::Win32::Graphics::Direct3D::{ID3DBlob, D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST};
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;

use super::{buffer_desc, committed, transition, FRAME_SLOTS};
use crate::ffx::{LitBinds, LitTables};
use crate::rlog;

// ---- the rows target (texels; shaders/er_fxlit.hlsl and er_vfx.wgsl KH_FXLIT use the same numbers: the tests read them)
/// cbSceneParam of the deferred tile lighting (126 rows: FC_MatrixInvViewProj 8..11, FC_MatrixInvView 20..23, FC_FrustumNear /
/// SliceScale / SliceBias 65, FC_LightTile_Width / NumCellX 66, SC_PreExposure 72.z, SC_EnvColorConvert 89..91).
pub(super) const ROW_SCENE: u32 = 0;
/// cbLightAccParam of the same pass (24 rows: g_InvProjMatrix 0..3).
pub(super) const ROW_LACC: u32 = 126;
/// The element counts of the light buffers t59..t62, clamped (`clamp_counts`).
pub(super) const ROW_COUNTS: u32 = 150;
/// gcbSceneParam of the particle pass (58 rows: IV A 29..31, IV B 33..35, (interp, -, scale A, scale B) 37, (enable A, enable B)
/// 38, sampling A / B 39 / 40, safe voxel A / B 41 / 42, the colour convert 52..54).
pub(super) const ROW_PASS: u32 = 160;
/// The sizes of the pass's eight volumes (A: X Y Z W, B: X Y Z W).
pub(super) const ROW_PASS_DIMS: u32 = 218;
/// A region's CB_ENVMAP_REGION (36 rows) and its four volumes' sizes, per region (two at most).
pub(super) const ROW_REGION: u32 = 230;
pub(super) const REGION_STRIDE: u32 = 42;
pub(super) const ROWS_TEXELS: u32 = 320;
/// The list texture's width.
pub(super) const LISTS_W: u32 = 1024;
const LISTS_MAX_H: u32 = 2048;
const MAX_COUNTS: [u32; 4] = [4_194_304, 262_144, 8192, 8192];
/// The volumes' largest side taken (a guard against a misread size).
const IV_MAX: u32 = 256;
pub(super) const VOLUMES: usize = 8;
/// The frame constants' lit rows (er_vfx.wgsl `ErFrame.fxl`, 13 rows), after the fog row.
pub(super) const FXL_FLOATS: usize = 13 * 4;
// (the rows fit, checked when the crate is built: the scene's 126 + 24 rows + the counts, the pass's 58 + its 8 sizes, two regions of
// 36 + 4)
const _: () = assert!(ROW_LACC == ROW_SCENE + 126 && ROW_COUNTS == ROW_LACC + 24 && ROW_PASS > ROW_COUNTS && ROW_PASS_DIMS == ROW_PASS + 58 && ROW_REGION >= ROW_PASS_DIMS + 8 && REGION_STRIDE >= 40 && ROW_REGION + 2 * REGION_STRIDE <= ROWS_TEXELS);

/// (PARTY-NATIVE-LIGHT) The sun for the lit effects (gfx.rs, once a frame): the game's cascade atlas (raw pointer, 0 = none this
/// frame), our copy of the cascades' constants (gfx.rs `SunKit`), their rows (hybrid.rs `sun_rows`), the game -> host matrix
/// (math.rs host_from_game, columns), the sun's colour as read (FC_DirLight Col_0; None = no directional light).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct FxSun {
    pub atlas: usize,
    pub cascades: usize,
    pub rows: [f32; 24],
    pub host_from_game: [f32; 16],
    pub colour: Option<[f32; 3]>,
}

static SUN: Mutex<Option<FxSun>> = Mutex::new(None);

/// gfx.rs, once a frame: this frame's sun for the lit effects.
pub(crate) fn set_sun(sun: Option<FxSun>) {
    if let Ok(mut s) = SUN.lock() {
        *s = sun;
    }
}

pub(super) fn sun() -> Option<FxSun> {
    SUN.lock().ok().and_then(|s| *s)
}

/// (round 2) The late frames of the native mode since the last status: all, with the copy drawn, by the volumes' source (none,
/// the pass, the regions), with the sun's colour, with its cascades, with a lit material among the late batches.
static FRAMES: [std::sync::atomic::AtomicU32; 8] = [const { std::sync::atomic::AtomicU32::new(0) }; 8];

/// (round 2) The counters a late frame adds to (pure, tested): [all, copied, source none, pass, regions, sun colour, cascades, lit
/// material].
pub(super) fn frame_marks(copied: bool, plan: &LitPlan, sun: Option<&FxSun>, lit_material: bool) -> [bool; 8] {
    let colour = sun.is_some_and(|s| s.colour.is_some());
    let cascades = sun.is_some_and(|s| s.atlas != 0 && s.cascades != 0 && s.rows[0] >= 1.0);
    [true, copied, copied && plan.source == 0, copied && plan.source == 1, copied && plan.source == 2, colour, cascades, lit_material]
}

/// vfx.rs, once per late stage in the native mode: what this frame had.
pub(super) fn note_frame(copied: bool, plan: &LitPlan, sun: Option<&FxSun>, lit_material: bool) {
    for (k, on) in frame_marks(copied, plan, sun, lit_material).iter().enumerate() {
        if *on {
            FRAMES[k].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

/// For the log: which inputs the late frames had since the last call.
pub(super) fn frames_status() -> String {
    let n = FRAMES.each_ref().map(|a| a.swap(0, std::sync::atomic::Ordering::Relaxed));
    format!("late frames this second {}: with the tile lights copied {} (volumes: none {}, the particle pass {}, the regions {}), the sun's colour {}, its cascades {}, a lit material on screen {}", n[0], n[1], n[2], n[3], n[4], n[5], n[6], n[7])
}

/// The light buffers' counts as the copy takes them (pure, tested; er_fxlit.hlsl `light_counts`).
pub(super) fn clamp_counts(c: [u32; 4]) -> [u32; 4] {
    [c[0].min(MAX_COUNTS[0]), c[1].min(MAX_COUNTS[1]), c[2].min(MAX_COUNTS[2]), c[3].min(MAX_COUNTS[3])]
}

/// Where each light buffer lies in the list texture (texels): the indices (four a texel) from 0, the cell lookup, the point
/// lights (13 texels each), the spot lights (12 each).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ListsLayout {
    pub lookup: u32,
    pub points: u32,
    pub spots: u32,
    pub total: u32,
    pub height: u32,
}

/// The layout for these counts (pure, tested; er_fxlit.hlsl `ps_lists`, er_vfx.wgsl `kh_lit_walk`): None without a cell lookup
/// or larger than the texture may be.
pub(super) fn lists_layout(counts: [u32; 4]) -> Option<ListsLayout> {
    let c = clamp_counts(counts);
    if c[1] == 0 {
        return None;
    }
    let lookup = c[0].div_ceil(4);
    let points = lookup + c[1];
    let spots = points + 13 * c[2];
    let total = spots + 12 * c[3];
    let height = total.div_ceil(LISTS_W).max(1);
    (height <= LISTS_MAX_H).then_some(ListsLayout { lookup, points, spots, total, height })
}

/// A volume size read back is taken when every side is between 1 and `IV_MAX` (pure, tested).
pub(super) fn iv_size_ok(d: [u32; 3]) -> bool {
    d.iter().all(|s| (1..=IV_MAX).contains(s))
}

/// Where this frame's irradiance volumes come from (pure, tested): the particle pass when the game drew one (its exact
/// constants), else the regions (the deferred route's), else none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum IvSource {
    None,
    Pass(LitTables),
    Regions([Option<LitTables>; 2]),
}

impl IvSource {
    pub(super) fn of(binds: &LitBinds) -> IvSource {
        match (binds.pass, binds.regions) {
            (Some(p), _) => IvSource::Pass(p),
            (None, r) if r[0].is_some() => IvSource::Regions(r),
            _ => IvSource::None,
        }
    }

    /// er_vfx.wgsl `fxl[0].y`: 0 none, 1 the pass, 2 the regions.
    fn code(&self) -> u32 {
        match self {
            IvSource::None => 0,
            IvSource::Pass(_) => 1,
            IvSource::Regions(_) => 2,
        }
    }

    fn regions(&self) -> u32 {
        match self {
            // (round 3) a two-volume region takes all eight slots: it is the only one
            IvSource::Regions(r) if self.lerp() => u32::from(r[0].is_some()),
            IvSource::Regions(r) => r.iter().filter(|x| x.is_some()).count() as u32,
            _ => 0,
        }
    }

    /// (round 3) the first region blends two volumes (`GXEnvMapRegion[...][Lerp]`): its second takes the slots 4..7.
    pub(super) fn lerp(&self) -> bool {
        matches!(self, IvSource::Regions([Some(r), _]) if r.variant == 1)
    }

    /// The volume slots this source fills (pure, tested): the pass all eight (A, B), a region four each.
    fn slots(&self) -> u32 {
        match self {
            IvSource::None => 0,
            IvSource::Pass(_) => 0xff,
            IvSource::Regions(_) if self.lerp() => 0xff,
            IvSource::Regions(_) => match self.regions() {
                0 => 0,
                1 => 0x0f,
                _ => 0xff,
            },
        }
    }
}

/// What this frame's copy draws (`LitKit::plan`), and so what the lit variants may read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct LitPlan {
    /// The tile lighting's constants (the matrices, the tile parameters, the pre-exposure): without them nothing is drawn.
    pub scene: bool,
    pub source: u32,
    pub regions: u32,
    /// The volumes copied (bit k = slot k).
    pub volumes: u32,
    /// The light buffers copied.
    pub lists: bool,
    /// (round 3) the region source is one two-volume region
    pub lerp: bool,
}

/// The lit rows of the frame constants (pure, tested; er_vfx.wgsl `ErFrame.fxl`): [0] (ready, the volumes' source, the volume
/// mask, the lists copied), [1] the sun's colour as read (rgb, w 1 = a sun), [2] (the cascades bound, the regions, the lists'
/// width, (round 3) 1 = a two-volume region), [3..9) the cascades' rows (hybrid.rs `sun_rows`), [9..13) the game -> host matrix (columns).
pub(super) fn fxl_row(plan: &LitPlan, sun: Option<&FxSun>) -> [f32; FXL_FLOATS] {
    let mut out = [0.0f32; FXL_FLOATS];
    if !plan.scene {
        return out;
    }
    out[0..4].copy_from_slice(&[1.0, plan.source as f32, plan.volumes as f32, if plan.lists { 1.0 } else { 0.0 }]);
    out[8..12].copy_from_slice(&[0.0, plan.regions as f32, LISTS_W as f32, if plan.lerp { 1.0 } else { 0.0 }]);
    if let Some(s) = sun {
        if let Some(c) = s.colour.filter(|c| c.iter().all(|v| v.is_finite() && *v >= 0.0)) {
            out[4..8].copy_from_slice(&[c[0], c[1], c[2], 1.0]);
        }
        let cascades = s.atlas != 0 && s.cascades != 0 && s.rows[0] >= 1.0;
        out[8] = if cascades { 1.0 } else { 0.0 };
        out[12..36].copy_from_slice(&s.rows);
        out[36..52].copy_from_slice(&s.host_from_game);
    }
    out
}

/// (PARTY-NATIVE-LIGHT) THE COPY OF THE GAME'S LIGHT INPUTS for the lit translucent effects (see the module).
pub(super) struct LitKit {
    root: ID3D12RootSignature,
    scene_pso: ID3D12PipelineState,
    pass_pso: ID3D12PipelineState,
    /// (round 3) a tessellated lit particle's DSCommonBuffer into the pass rows
    dspass_pso: ID3D12PipelineState,
    region_pso: ID3D12PipelineState,
    vol_pso: ID3D12PipelineState,
    lists_pso: ID3D12PipelineState,
    /// the rows target (ROWS_TEXELS x 1 RGBA32_UINT) and its RTV
    pub(super) rows: ID3D12Resource,
    rows_rtv: ID3D12DescriptorHeap,
    /// per frame slot: the rows read back (the sizes for the next use of the slot), its footprint
    readback: Vec<ID3D12Resource>,
    footprint: D3D12_PLACED_SUBRESOURCE_FOOTPRINT,
    /// the volume copies (RGBA16F, the game's sizes), one RTV per slice
    pub(super) volumes: [Option<(ID3D12Resource, ID3D12DescriptorHeap, [u32; 3])>; VOLUMES],
    /// the list texture (LISTS_W x height RGBA32_UINT) and its RTV
    pub(super) lists: Option<(ID3D12Resource, ID3D12DescriptorHeap, u32)>,
    rtv_size: usize,
    /// read back most recently: the light buffers' counts, the pass's volume sizes, the regions' (two x four)
    counts: [u32; 4],
    pass_dims: [[u32; 3]; VOLUMES],
    region_dims: [[u32; 3]; VOLUMES],
    /// whether a readback has given the counts, the pass's sizes, the regions' sizes at all (for the log)
    seen: [bool; 3],
}

impl LitKit {
    /// The kit for this device (its shaders: `<main root>/er/fxlit.vs.dxil`, `fxlit_scene / _pass / _region / _vol / _lists.ps.dxil`).
    pub(super) unsafe fn new(device: &ID3D12Device, dir: &Path) -> Result<LitKit, String> {
        let text = |what: &str, e: windows::core::Error| format!("{what}: {e}");
        let read = |f: &str| std::fs::read(dir.join("er").join(f)).map_err(|e| format!("{f}: {e} (kh-shaderc --only fxlit)"));
        let vs = read("fxlit.vs.dxil")?;
        let (scene_ps, pass_ps, region_ps, vol_ps, lists_ps) = (read("fxlit_scene.ps.dxil")?, read("fxlit_pass.ps.dxil")?, read("fxlit_region.ps.dxil")?, read("fxlit_vol.ps.dxil")?, read("fxlit_lists.ps.dxil")?);
        let dspass_ps = read("fxlit_dspass.ps.dxil")?; // (round 3)
        // the root signature: the game's pixel-stage tables b0..b3, b4..b13, t4..t19, t20..t65 (its GX root signature's 28, 29, 31, 32)
        // and four root constants at b14
        let range = |kind: D3D12_DESCRIPTOR_RANGE_TYPE, n: u32, base: u32| [D3D12_DESCRIPTOR_RANGE { RangeType: kind, NumDescriptors: n, BaseShaderRegister: base, RegisterSpace: 0, OffsetInDescriptorsFromTableStart: 0 }];
        let ranges = [range(D3D12_DESCRIPTOR_RANGE_TYPE_CBV, 4, 0), range(D3D12_DESCRIPTOR_RANGE_TYPE_CBV, 10, 4), range(D3D12_DESCRIPTOR_RANGE_TYPE_SRV, 16, 4), range(D3D12_DESCRIPTOR_RANGE_TYPE_SRV, 46, 20)];
        let table = |r: &[D3D12_DESCRIPTOR_RANGE; 1]| D3D12_ROOT_PARAMETER { ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE, Anonymous: D3D12_ROOT_PARAMETER_0 { DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE { NumDescriptorRanges: 1, pDescriptorRanges: r.as_ptr() } }, ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL };
        let parameters = [
            table(&ranges[0]),
            table(&ranges[1]),
            table(&ranges[2]),
            table(&ranges[3]),
            D3D12_ROOT_PARAMETER { ParameterType: D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS, Anonymous: D3D12_ROOT_PARAMETER_0 { Constants: D3D12_ROOT_CONSTANTS { ShaderRegister: 14, RegisterSpace: 0, Num32BitValues: 4 } }, ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL },
        ];
        let desc = D3D12_ROOT_SIGNATURE_DESC { NumParameters: parameters.len() as u32, pParameters: parameters.as_ptr(), NumStaticSamplers: 0, pStaticSamplers: std::ptr::null(), Flags: D3D12_ROOT_SIGNATURE_FLAG_NONE };
        let mut blob: Option<ID3DBlob> = None;
        D3D12SerializeRootSignature(&desc, D3D_ROOT_SIGNATURE_VERSION_1, &mut blob, None).map_err(|e| text("lit root signature", e))?;
        let blob = blob.ok_or("lit root signature: no blob")?;
        let root: ID3D12RootSignature = device.CreateRootSignature(0, std::slice::from_raw_parts(blob.GetBufferPointer() as *const u8, blob.GetBufferSize())).map_err(|e| text("CreateRootSignature (lit)", e))?;
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
            device.CreateGraphicsPipelineState(&d).map_err(|e| text("CreateGraphicsPipelineState (lit)", e))
        };
        let scene_pso = pso(&scene_ps, DXGI_FORMAT_R32G32B32A32_UINT)?;
        let pass_pso = pso(&pass_ps, DXGI_FORMAT_R32G32B32A32_UINT)?;
        let dspass_pso = pso(&dspass_ps, DXGI_FORMAT_R32G32B32A32_UINT)?;
        let region_pso = pso(&region_ps, DXGI_FORMAT_R32G32B32A32_UINT)?;
        let vol_pso = pso(&vol_ps, DXGI_FORMAT_R16G16B16A16_FLOAT)?;
        let lists_pso = pso(&lists_ps, DXGI_FORMAT_R32G32B32A32_UINT)?;
        let rows_desc = D3D12_RESOURCE_DESC { Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D, Width: ROWS_TEXELS as u64, Height: 1, DepthOrArraySize: 1, MipLevels: 1, Format: DXGI_FORMAT_R32G32B32A32_UINT, SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 }, Flags: D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET, ..Default::default() };
        let rows = committed(device, D3D12_HEAP_TYPE_DEFAULT, &rows_desc, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE).map_err(|e| text("lit rows", e))?;
        let rows_rtv: ID3D12DescriptorHeap = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV, NumDescriptors: 1, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE, NodeMask: 0 }).map_err(|e| text("lit rows rtv", e))?;
        device.CreateRenderTargetView(&rows, None, rows_rtv.GetCPUDescriptorHandleForHeapStart());
        let mut footprint = D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default();
        let mut total = 0u64;
        device.GetCopyableFootprints(&rows_desc, 0, 1, 0, Some(&mut footprint), None, None, Some(&mut total));
        let mut readback = Vec::new();
        for _ in 0..FRAME_SLOTS {
            readback.push(committed(device, D3D12_HEAP_TYPE_READBACK, &buffer_desc(total), D3D12_RESOURCE_STATE_COPY_DEST).map_err(|e| text("lit readback", e))?);
        }
        let rtv_size = device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV) as usize;
        Ok(LitKit { root, scene_pso, pass_pso, dspass_pso, region_pso, vol_pso, lists_pso, rows, rows_rtv, readback, footprint, volumes: Default::default(), lists: None, rtv_size, counts: [0; 4], pass_dims: [[0; 3]; VOLUMES], region_dims: [[0; 3]; VOLUMES], seen: [false; 3] })
    }

    /// The sizes this slot's readback holds (written by its previous use: the GPU is past it), and the list texture and the
    /// volume copies this frame's source needs made again when they differ.
    pub(super) unsafe fn prepare(&mut self, device: &ID3D12Device, slot: usize, binds: &LitBinds) {
        if let Some(readback) = self.readback.get(slot) {
            let mut at: *mut c_void = std::ptr::null_mut();
            let end = self.footprint.Offset as usize + ROWS_TEXELS as usize * 16;
            if readback.Map(0, Some(&D3D12_RANGE { Begin: 0, End: end }), Some(&mut at)).is_ok() && !at.is_null() {
                let texel = |k: u32| {
                    let p = (at as *const u8).add(self.footprint.Offset as usize + k as usize * 16) as *const u32;
                    [*p, *p.add(1), *p.add(2), *p.add(3)]
                };
                let c = texel(ROW_COUNTS);
                if c != [0; 4] {
                    self.counts = clamp_counts(c);
                    self.seen[0] = true;
                }
                for k in 0..VOLUMES as u32 {
                    let d = texel(ROW_PASS_DIMS + k);
                    if d[3] == 1 {
                        self.pass_dims[k as usize] = [d[0], d[1], d[2]];
                        self.seen[1] = true;
                    }
                    let d = texel(ROW_REGION + REGION_STRIDE * (k / 4) + 36 + k % 4);
                    if d[3] == 1 {
                        self.region_dims[k as usize] = [d[0], d[1], d[2]];
                        self.seen[2] = true;
                    }
                }
                readback.Unmap(0, Some(&D3D12_RANGE { Begin: 0, End: 0 }));
            }
        }
        // the list texture at the counts' height
        if let Some(layout) = lists_layout(self.counts) {
            if self.lists.as_ref().is_none_or(|l| l.2 != layout.height) {
                let desc = D3D12_RESOURCE_DESC { Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D, Width: LISTS_W as u64, Height: layout.height, DepthOrArraySize: 1, MipLevels: 1, Format: DXGI_FORMAT_R32G32B32A32_UINT, SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 }, Flags: D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET, ..Default::default() };
                if let (Ok(texture), Ok(heap)) = (committed(device, D3D12_HEAP_TYPE_DEFAULT, &desc, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE), device.CreateDescriptorHeap::<ID3D12DescriptorHeap>(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV, NumDescriptors: 1, Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE, NodeMask: 0 })) {
                    device.CreateRenderTargetView(&texture, None, heap.GetCPUDescriptorHandleForHeapStart());
                    // (the one before may be named by a frame in flight: kept until the GPU is past it, GPUSAFE)
                    if let Some((old, old_heap, _)) = self.lists.replace((texture, heap, layout.height)) {
                        crate::outfit::retire(Box::new(old));
                        crate::outfit::retire(Box::new(old_heap));
                    }
                    rlog!("vfx: (PARTY-NATIVE-LIGHT) the game's light buffers: {} light index(es), {} tile cell(s), {} point light(s), {} spot light(s): their copy is {}x{}", self.counts[0], self.counts[1], self.counts[2], self.counts[3], LISTS_W, layout.height);
                }
            }
        }
        // the volumes of this frame's source at their sizes
        let source = IvSource::of(binds);
        for k in 0..VOLUMES {
            let Some(size) = self.wanted_size(&source, k) else { continue };
            if self.volumes[k].as_ref().is_some_and(|v| v.2 == size) {
                continue;
            }
            let desc = D3D12_RESOURCE_DESC { Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE3D, Width: size[0] as u64, Height: size[1], DepthOrArraySize: size[2] as u16, MipLevels: 1, Format: DXGI_FORMAT_R16G16B16A16_FLOAT, SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 }, Flags: D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET, ..Default::default() };
            let (Ok(volume), Ok(heap)) = (committed(device, D3D12_HEAP_TYPE_DEFAULT, &desc, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE), device.CreateDescriptorHeap::<ID3D12DescriptorHeap>(&D3D12_DESCRIPTOR_HEAP_DESC { Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV, NumDescriptors: size[2], Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE, NodeMask: 0 })) else { continue };
            let start = heap.GetCPUDescriptorHandleForHeapStart();
            for z in 0..size[2] {
                let rtv = D3D12_RENDER_TARGET_VIEW_DESC { Format: DXGI_FORMAT_R16G16B16A16_FLOAT, ViewDimension: D3D12_RTV_DIMENSION_TEXTURE3D, Anonymous: D3D12_RENDER_TARGET_VIEW_DESC_0 { Texture3D: D3D12_TEX3D_RTV { MipSlice: 0, FirstWSlice: z, WSize: 1 } } };
                device.CreateRenderTargetView(&volume, Some(&rtv), D3D12_CPU_DESCRIPTOR_HANDLE { ptr: start.ptr + z as usize * self.rtv_size });
            }
            if let Some((old, old_heap, _)) = self.volumes[k].replace((volume, heap, size)) {
                crate::outfit::retire(Box::new(old));
                crate::outfit::retire(Box::new(old_heap));
            }
            rlog!("vfx: (PARTY-NATIVE-LIGHT) the game's irradiance volume {k} ({}) is {}x{}x{}: its copy is made", if matches!(source, IvSource::Pass(_)) { "the particle pass" } else { "a region" }, size[0], size[1], size[2]);
        }
    }

    /// Volume slot `k`'s size for this source, when it fills that slot and its size is known.
    fn wanted_size(&self, source: &IvSource, k: usize) -> Option<[u32; 3]> {
        if source.slots() & (1 << k) == 0 {
            return None;
        }
        let size = match source {
            IvSource::Pass(_) => self.pass_dims[k],
            _ => self.region_dims[k],
        };
        iv_size_ok(size).then_some(size)
    }

    /// What this frame's copy draws (the lit rows of the frame constants say so to the shaders).
    pub(super) fn plan(&self, binds: &LitBinds) -> LitPlan {
        if binds.tile.is_none() {
            return LitPlan::default();
        }
        let source = IvSource::of(binds);
        let mut volumes = 0u32;
        for k in 0..VOLUMES {
            if let (Some(size), Some(v)) = (self.wanted_size(&source, k), self.volumes[k].as_ref()) {
                if v.2 == size {
                    volumes |= 1 << k;
                }
            }
        }
        let lists = lists_layout(self.counts).is_some_and(|l| self.lists.as_ref().is_some_and(|x| x.2 == l.height));
        LitPlan { scene: true, source: source.code(), regions: source.regions(), volumes, lists, lerp: source.lerp() }
    }

    /// The copy of this frame's light inputs (`binds`, as `plan` said), recorded into `list`; true when drawn. Leaves the lit root
    /// signature and the game's heap bound: the caller binds its own again.
    pub(super) unsafe fn copy(&self, list: &ID3D12GraphicsCommandList, slot: usize, binds: &LitBinds, plan: &LitPlan) -> bool {
        let Some(tile) = binds.tile.filter(|_| plan.scene) else { return false };
        let Some(tile_heap) = crate::gfx::views_heap(tile.heaps) else { return false };
        let handle = |ptr: u64| D3D12_GPU_DESCRIPTOR_HANDLE { ptr };
        let span = |a: u32, b: u32| list.RSSetScissorRects(&[windows::Win32::Foundation::RECT { left: a as i32, top: 0, right: b as i32, bottom: 1 }]);
        list.SetGraphicsRootSignature(&self.root);
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        // ---- the rows (and the sizes, then into this slot's readback)
        list.ResourceBarrier(&[transition(&self.rows, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, D3D12_RESOURCE_STATE_RENDER_TARGET)]);
        let rtv = self.rows_rtv.GetCPUDescriptorHandleForHeapStart();
        list.OMSetRenderTargets(1, Some(&rtv), false, None);
        list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: ROWS_TEXELS as f32, Height: 1.0, MinDepth: 0.0, MaxDepth: 1.0 }]);
        list.SetDescriptorHeaps(&[Some(tile_heap.clone())]);
        list.SetGraphicsRootDescriptorTable(0, handle(tile.low_cbv));
        list.SetGraphicsRootDescriptorTable(1, handle(tile.high_cbv));
        list.SetGraphicsRootDescriptorTable(3, handle(tile.high_srv));
        span(ROW_SCENE, ROW_COUNTS + 1);
        list.SetPipelineState(&self.scene_pso);
        list.DrawInstanced(3, 1, 0, 0);
        let source = IvSource::of(binds);
        match source {
            IvSource::Pass(pass) => {
                if let Some(heap) = crate::gfx::views_heap(pass.heaps) {
                    list.SetDescriptorHeaps(&[Some(heap)]);
                    // (round 3) a tessellated particle's: its DSCommonBuffer at b0 (DS table 14), remapped
                    if pass.variant == 1 {
                        list.SetGraphicsRootDescriptorTable(0, handle(pass.low_cbv));
                    } else {
                        list.SetGraphicsRootDescriptorTable(1, handle(pass.high_cbv));
                    }
                    list.SetGraphicsRootDescriptorTable(2, handle(pass.mid_srv));
                    span(ROW_PASS, ROW_PASS_DIMS + VOLUMES as u32);
                    list.SetPipelineState(if pass.variant == 1 { &self.dspass_pso } else { &self.pass_pso });
                    list.DrawInstanced(3, 1, 0, 0);
                }
            }
            IvSource::Regions(regions) => {
                list.SetPipelineState(&self.region_pso);
                let lerp = source.lerp();
                for (r, region) in regions.iter().enumerate() {
                    let Some(region) = region.filter(|_| !lerp || r == 0) else { continue };
                    let Some(heap) = crate::gfx::views_heap(region.heaps) else { continue };
                    list.SetDescriptorHeaps(&[Some(heap)]);
                    list.SetGraphicsRootDescriptorTable(0, handle(region.low_cbv));
                    list.SetGraphicsRootDescriptorTable(3, handle(region.high_srv));
                    list.SetGraphicsRoot32BitConstants(4, 4, [0u32, 0, u32::from(lerp), r as u32].as_ptr() as *const c_void, 0);
                    let base = ROW_REGION + REGION_STRIDE * r as u32;
                    // (round 3) a two-volume region: also its second volume's sizes, where a second region's would go
                    span(base, if lerp { base + REGION_STRIDE + 40 } else { base + 40 });
                    list.DrawInstanced(3, 1, 0, 0);
                }
            }
            IvSource::None => {}
        }
        list.ResourceBarrier(&[transition(&self.rows, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_COPY_SOURCE)]);
        if let Some(readback) = self.readback.get(slot) {
            let to = D3D12_TEXTURE_COPY_LOCATION { pResource: std::mem::transmute_copy(readback), Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT, Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { PlacedFootprint: self.footprint } };
            let from = D3D12_TEXTURE_COPY_LOCATION { pResource: std::mem::transmute_copy(&self.rows), Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX, Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 { SubresourceIndex: 0 } };
            list.CopyTextureRegion(&to, 0, 0, 0, &from, None);
        }
        list.ResourceBarrier(&[transition(&self.rows, D3D12_RESOURCE_STATE_COPY_SOURCE, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE)]);
        // ---- the volumes, slice by slice (the planned ones)
        list.SetPipelineState(&self.vol_pso);
        for k in 0..VOLUMES {
            let Some((volume, heap, size)) = self.volumes[k].as_ref().filter(|_| plan.volumes & (1 << k) != 0) else { continue };
            let (tables, from) = match source {
                IvSource::Pass(pass) => (pass, 0u32),
                // (round 3) a two-volume region: its second volume (t31..t34) in the slots 4..7
                IvSource::Regions(regions) if source.lerp() => match regions[0] {
                    Some(region) => (region, if k < 4 { 1u32 } else { 2u32 }),
                    None => continue,
                },
                IvSource::Regions(regions) => match regions[k / 4] {
                    Some(region) => (region, 1u32),
                    None => continue,
                },
                IvSource::None => continue,
            };
            let Some(game_heap) = crate::gfx::views_heap(tables.heaps) else { continue };
            list.SetDescriptorHeaps(&[Some(game_heap)]);
            if from == 0 {
                list.SetGraphicsRootDescriptorTable(2, handle(tables.mid_srv));
            } else {
                list.SetGraphicsRootDescriptorTable(3, handle(tables.high_srv));
            }
            list.ResourceBarrier(&[transition(volume, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, D3D12_RESOURCE_STATE_RENDER_TARGET)]);
            list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: size[0] as f32, Height: size[1] as f32, MinDepth: 0.0, MaxDepth: 1.0 }]);
            list.RSSetScissorRects(&[windows::Win32::Foundation::RECT { left: 0, top: 0, right: size[0] as i32, bottom: size[1] as i32 }]);
            let start = heap.GetCPUDescriptorHandleForHeapStart();
            for z in 0..size[2] {
                let rtv = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: start.ptr + z as usize * self.rtv_size };
                list.OMSetRenderTargets(1, Some(&rtv), false, None);
                list.SetGraphicsRoot32BitConstants(4, 4, [z, k as u32, from, 0].as_ptr() as *const c_void, 0);
                list.DrawInstanced(3, 1, 0, 0);
            }
            list.ResourceBarrier(&[transition(volume, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE)]);
        }
        // ---- the light buffers into the list texture
        if let Some((texture, heap, height)) = self.lists.as_ref().filter(|_| plan.lists) {
            list.SetDescriptorHeaps(&[Some(tile_heap)]);
            list.SetGraphicsRootDescriptorTable(3, handle(tile.high_srv));
            list.ResourceBarrier(&[transition(texture, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, D3D12_RESOURCE_STATE_RENDER_TARGET)]);
            let rtv = heap.GetCPUDescriptorHandleForHeapStart();
            list.OMSetRenderTargets(1, Some(&rtv), false, None);
            list.RSSetViewports(&[D3D12_VIEWPORT { TopLeftX: 0.0, TopLeftY: 0.0, Width: LISTS_W as f32, Height: *height as f32, MinDepth: 0.0, MaxDepth: 1.0 }]);
            list.RSSetScissorRects(&[windows::Win32::Foundation::RECT { left: 0, top: 0, right: LISTS_W as i32, bottom: *height as i32 }]);
            list.SetPipelineState(&self.lists_pso);
            list.DrawInstanced(3, 1, 0, 0);
            list.ResourceBarrier(&[transition(texture, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE)]);
        }
        true
    }

    /// For the log.
    pub(super) fn status(&self) -> String {
        let made = self.volumes.iter().filter(|v| v.is_some()).count();
        format!("light buffers {:?}{}, {made} volume copy(ies), sizes read back (counts, pass, regions) {:?}", self.counts, if self.lists.is_some() { "" } else { " (no list copy yet)" }, self.seen)
    }
}

/// The lit views of a frame slot's table (t18..t29, er_vfx.wgsl KH_FXLIT) from `at` on: the rows, the eight volumes, the lists,
/// the sun's atlas and cascades; a null view of the shader's kind where there is none this frame.
pub(super) unsafe fn write_views(device: &ID3D12Device, kit: Option<&LitKit>, plan: &LitPlan, sun: Option<&FxSun>, at: impl Fn(usize) -> D3D12_CPU_DESCRIPTOR_HANDLE) {
    let null = |k: usize, dim: D3D12_SRV_DIMENSION, format: DXGI_FORMAT| {
        let anonymous = if dim == D3D12_SRV_DIMENSION_TEXTURE3D { D3D12_SHADER_RESOURCE_VIEW_DESC_0 { Texture3D: D3D12_TEX3D_SRV { MostDetailedMip: 0, MipLevels: 1, ResourceMinLODClamp: 0.0 } } } else { D3D12_SHADER_RESOURCE_VIEW_DESC_0 { Texture2D: D3D12_TEX2D_SRV { MostDetailedMip: 0, MipLevels: 1, PlaneSlice: 0, ResourceMinLODClamp: 0.0 } } };
        let desc = D3D12_SHADER_RESOURCE_VIEW_DESC { Format: format, ViewDimension: dim, Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING, Anonymous: anonymous };
        device.CreateShaderResourceView(None, Some(&desc), at(k));
    };
    match kit.filter(|_| plan.scene) {
        Some(k) => device.CreateShaderResourceView(&k.rows, None, at(0)),
        None => null(0, D3D12_SRV_DIMENSION_TEXTURE2D, DXGI_FORMAT_R32G32B32A32_UINT),
    }
    for v in 0..VOLUMES {
        match kit.and_then(|k| k.volumes[v].as_ref()).filter(|_| plan.volumes & (1 << v) != 0) {
            Some((volume, ..)) => device.CreateShaderResourceView(volume, None, at(1 + v)),
            None => null(1 + v, D3D12_SRV_DIMENSION_TEXTURE3D, DXGI_FORMAT_R16G16B16A16_FLOAT),
        }
    }
    match kit.and_then(|k| k.lists.as_ref()).filter(|_| plan.lists) {
        Some((texture, ..)) => device.CreateShaderResourceView(texture, None, at(9)),
        None => null(9, D3D12_SRV_DIMENSION_TEXTURE2D, DXGI_FORMAT_R32G32B32A32_UINT),
    }
    let sun = sun.filter(|s| s.atlas != 0 && s.cascades != 0);
    let atlas_raw = sun.map_or(0, |s| s.atlas) as *mut c_void;
    match ID3D12Resource::from_raw_borrowed(&atlas_raw).filter(|_| !atlas_raw.is_null()) {
        Some(atlas) => {
            let desc = D3D12_SHADER_RESOURCE_VIEW_DESC { Format: DXGI_FORMAT_R16_UNORM, ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D, Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING, Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 { Texture2D: D3D12_TEX2D_SRV { MostDetailedMip: 0, MipLevels: 1, PlaneSlice: 0, ResourceMinLODClamp: 0.0 } } };
            device.CreateShaderResourceView(atlas, Some(&desc), at(10));
        }
        None => null(10, D3D12_SRV_DIMENSION_TEXTURE2D, DXGI_FORMAT_R16_UNORM),
    }
    let casc_raw = sun.map_or(0, |s| s.cascades) as *mut c_void;
    match ID3D12Resource::from_raw_borrowed(&casc_raw).filter(|_| !casc_raw.is_null()) {
        Some(cascades) => device.CreateShaderResourceView(cascades, None, at(11)),
        None => null(11, D3D12_SRV_DIMENSION_TEXTURE2D, DXGI_FORMAT_R32G32B32A32_FLOAT),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (PARTY-NATIVE-LIGHT) the copy's numbers are the same in the kit, the copy shader and the effects' harness; the list layout,
    /// the volume sizes taken, the source of the volumes and the frame rows
    #[test]
    fn party_native_light_the_copy_and_the_harness_share_one_layout() {
        let hlsl = include_str!("../shaders/er_fxlit.hlsl");
        for (name, value) in [("ROW_SCENE", ROW_SCENE), ("ROW_LACC", ROW_LACC), ("ROW_COUNTS", ROW_COUNTS), ("ROW_PASS", ROW_PASS), ("ROW_PASS_DIMS", ROW_PASS_DIMS), ("ROW_REGION", ROW_REGION), ("REGION_STRIDE", REGION_STRIDE), ("LISTS_W", LISTS_W), ("MAX_INDICES", MAX_COUNTS[0]), ("MAX_LOOKUP", MAX_COUNTS[1]), ("MAX_POINTS", MAX_COUNTS[2]), ("MAX_SPOTS", MAX_COUNTS[3])] {
            assert!(hlsl.contains(&format!("#define {name} {value}\n")), "er_fxlit.hlsl {name}");
        }
        let wgsl = include_str!("../shaders/er_vfx.wgsl");
        for (name, value) in [("LIT_ROW_SCENE", ROW_SCENE), ("LIT_ROW_LACC", ROW_LACC), ("LIT_ROW_COUNTS", ROW_COUNTS), ("LIT_ROW_PASS", ROW_PASS), ("LIT_ROW_REGION", ROW_REGION), ("LIT_REGION_STRIDE", REGION_STRIDE)] {
            assert!(wgsl.contains(&format!("const {name}: i32 = {value};")), "er_vfx.wgsl {name}");
        }
        assert!(wgsl.contains(&format!("const LIT_LISTS_W: u32 = {LISTS_W}u;")));
        assert!(wgsl.contains("fxl: array<vec4<f32>, 13>,") && FXL_FLOATS == 13 * 4);
        // the lists: indices four a texel, then the cells, 13 texels a point light, 12 a spot light
        assert_eq!(lists_layout([10, 5, 2, 1]), Some(ListsLayout { lookup: 3, points: 8, spots: 34, total: 46, height: 1 }));
        assert_eq!(lists_layout([0, 0, 0, 0]), None, "no cell lookup");
        assert_eq!(lists_layout([4_000_000, 14_400, 1024, 512]).map(|l| l.height), Some((1_000_000 + 14_400 + 13 * 1024 + 12 * 512u32).div_ceil(1024)));
        assert_eq!(clamp_counts([u32::MAX; 4]), MAX_COUNTS);
        assert_eq!(lists_layout([u32::MAX; 4]).map(|l| l.height), Some(1480), "the clamped counts always fit (24 MB at most)");
        assert!(iv_size_ok([32, 32, 16]) && !iv_size_ok([0, 32, 16]) && !iv_size_ok([32, 32, 512]));
        // the volumes' source: the pass first, then the regions
        let t = |n: u64| LitTables { low_cbv: n, high_cbv: n, mid_srv: n, high_srv: n, heaps: [1, 0], variant: 0 };
        let pass = LitBinds { tile: Some(t(1)), pass: Some(t(2)), regions: [Some(t(3)), None] };
        assert_eq!(IvSource::of(&pass), IvSource::Pass(t(2)));
        assert_eq!((IvSource::of(&pass).slots(), IvSource::of(&pass).code()), (0xff, 1));
        let one = LitBinds { pass: None, ..pass };
        assert_eq!((IvSource::of(&one).slots(), IvSource::of(&one).code(), IvSource::of(&one).regions()), (0x0f, 2, 1));
        let two = LitBinds { regions: [Some(t(3)), Some(t(4))], ..one };
        assert_eq!(IvSource::of(&two).slots(), 0xff);
        assert_eq!(IvSource::of(&LitBinds::default()).slots(), 0);
        // (round 3) a two-volume region first: it alone, all eight slots
        let lerp = LitBinds { regions: [Some(LitTables { variant: 1, ..t(5) }), Some(t(6))], ..one };
        let src = IvSource::of(&lerp);
        assert_eq!((src.lerp(), src.slots(), src.regions(), src.code()), (true, 0xff, 1, 2));
        assert!(!IvSource::of(&two).lerp() && !IvSource::of(&pass).lerp());
        // the frame rows: nothing without the scene's rows; else the plan, the sun's colour, its cascades and the matrix
        assert_eq!(fxl_row(&LitPlan::default(), None), [0.0; FXL_FLOATS]);
        let plan = LitPlan { scene: true, source: 1, regions: 0, volumes: 0xff, lists: true, lerp: false };
        let mut rows = [0.0; 24];
        rows[0] = 3.0;
        let sun = FxSun { atlas: 1, cascades: 2, rows, host_from_game: [7.0; 16], colour: Some([0.5, 0.4, 0.3]) };
        let r = fxl_row(&plan, Some(&sun));
        assert_eq!(&r[0..4], &[1.0, 1.0, 255.0, 1.0]);
        assert_eq!(&r[4..12], &[0.5, 0.4, 0.3, 1.0, 1.0, 0.0, LISTS_W as f32, 0.0]);
        assert_eq!((r[12], r[36], r[51]), (3.0, 7.0, 7.0));
        let r = fxl_row(&plan, Some(&FxSun { atlas: 0, colour: None, ..sun }));
        assert_eq!((r[7], r[8]), (0.0, 0.0), "no sun colour, no cascades");
        // (round 2) a frame's marks: the copy with no volume source still counts as copied (the tile lights + the sun)
        let none = LitPlan { source: 0, volumes: 0, ..plan };
        assert_eq!(frame_marks(true, &none, Some(&sun), false), [true, true, true, false, false, true, true, false]);
        assert_eq!(frame_marks(false, &plan, None, true), [true, false, false, false, false, false, false, true]);
        assert_eq!(fxl_row(&none, Some(&sun))[0..4], [1.0, 0.0, 0.0, 1.0], "ready without volumes");
        assert_eq!(fxl_row(&LitPlan { lerp: true, ..plan }, Some(&sun))[11], 1.0, "(round 3) a two-volume region");
    }
}
