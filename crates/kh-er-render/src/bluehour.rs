//! (AUDIT-C TF) DOUBLE FORM'S ULTIMATE FINISH: THE BLUE HOUR over Elden Ring's frame.
//!
//! KH3 (the state-event camera BP_TresPlayerStateEventCamera_Oath_keeper, decoded by AUDIT-C CAM, @495 .. @1750, while M_N425
//! plays): SetActorVisibleOff / StopOn on the map, the background, the gimmicks, the NPCs and the prizes; the scene lights off
//! (DisableLight); r.ReflectionEnvironment 0; the post-process volume postprocess_BlueHour (IndirectLightingIntensity 0, the
//! sky light's cubemap x3, its angle 160 - the camera's yaw). The enemies and the players stay. What is left of the world is
//! the sky dome vw_so220_fin_fsg0 (unlit, opaque: the Blue Hour sky SKY_bluehour_018_8K_EXR).
//!
//! HERE (the one route Elden Ring's frame allows: its map and its characters are told apart by the class every pixel carries
//! in the G-buffer's STENCIL, never by colour): while the Blue Hour is on (`set`, kh-er cam.rs from kh-camera form_bp's
//! `blue_hour`), before Sora's colour draws (gfx.rs), one full-screen triangle with the stencil test `(stencil & 0xF0) !=
//! 0x50` (Elden Ring's characters: 0x50 measured, gfx.rs CHAR_STENCIL; the KH characters' early depth writes 0x50 too) paints
//! every other pixel black at the farthest depth (er_char.wgsl ER_BLUEHOUR, reversed Z: 0). The sky dome (an effect drawn
//! after it, depth-tested) is then the background; the characters keep their pixels and their depth. The light half
//! (scene lights off, IndirectLightingIntensity 0) is the KH characters' own light: kh-er's light rig (see `lights_off`).
//! Nothing is drawn without the pack's `materials/er/bluehour.ps.dxil` (or `KH_BLUEHOUR_SHADERS=DIR`, the harness's staged
//! files), a depth without stencil, or `blue_hour` off.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use windows::core::Interface;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;

/// The stencil class of Elden Ring's characters (gfx.rs CHAR_STENCIL: 0x50, the upper four bits compared).
pub const CHARACTER_CLASS: u8 = 0x50;
/// The bits of the stencil the class is read from.
pub const CLASS_MASK: u8 = 0xF0;

/// The sky light that lights the characters in the Blue Hour (postprocess_BlueHour's SkyLight: SLS_SpecifiedCubemap
/// SKY_bluehour_HDR, Intensity 3, Movable; the post-process volume: IndirectLightingIntensity 0, priority 500): the cubemap's
/// mean radiance (its L0 band; BC6H 128 x 6 decoded by tools/lib/bcdec.js: (0.05146, 0.0988, 0.21167)) x 3, as the rig's
/// ambient (KH3 units, the stand-in rig's). The scene's lights are off (DisableLight): no key light.
pub const SKY_LIGHT: glam::Vec3 = glam::Vec3::new(0.15438, 0.2964, 0.63501);

/// (AUDIT-B W-FIN) FINISH_SK's CEILING LOOK (BP_TresPlayerStateEventCamera_fz_key, ATARI: a ceiling over him at 7419;
/// Fz_FInish01_End @9451 .. @6975, undone @10151 .. @7243): KH3's same environment pattern as the Blue Hour, so the same pass:
/// - `SetActorVisibleOff` / `SetActorStopOn(self, CH_EVENT, bPlayer false, bFriend false, bEnemy false, bMap, bBackground,
///   bGimmick, bProjectile, bNpc, bPrize)` (TresGameBlueprintLibrary 0x14222B890 -> FUN_140E65FE0 with the nine kinds;
///   ETresObjTypeProcChannel 2 = CH_EVENT): the map hidden, the enemies and the players kept -> the stencil pass below;
/// - `DisableLight(true, CH_PLAYER)` (ATresSceneManager 0x142A23E20 -> FUN_1411572E0: every ULightComponentBase of the world's
///   levels off) and `r.ReflectionEnvironment 0`: no key light, no shadow;
/// - `postprocess_Rapunzel_tower_C` spawned AFTER it (so its own SkyLight stays): PostProcessComponent (unbound, priority 500)
///   bOverride_IndirectLightingIntensity, IndirectLightingIntensity 0 (the hybrid capture off), SkyLightComponent
///   SLS_SpecifiedCubemap htx_dw03_hdr (Maps/Common/Textures, Intensity not overridden: 1) -> [`TOWER_SKY_LIGHT`] = its mean
///   radiance (BC6H 128 x 6, the same face mean as `SKY_LIGHT`: build/sora_tmp/weapons/AUDIT-B/W-FIN/tools/sky_light_l0.js) x 1;
/// - what is left of the world: the particle vw_so073_fin_fsg0 (kh-core states/bp_event.rs), an opaque unlit two-sided
///   sphere 300 m across, drawn after this pass as the Blue Hour's dome is.
///
/// kh-er cam.rs / the harness call [`set_tower`] on kh-camera `FormBpOutput::tower`.
/// (STYLEFINISH's dtor @18641 .. @9621 in KH3 makes the actors visible and fades the particle but leaves the lights off and the
/// post-process actor until the area reloads; the port has no such reload, so the dtor ends the whole look.)
pub const TOWER_SKY_LIGHT: glam::Vec3 = glam::Vec3::new(0.09761, 0.09703, 0.14428);

static ON: AtomicBool = AtomicBool::new(false);
static TOWER: AtomicBool = AtomicBool::new(false);

/// The Blue Hour on or off (kh-er cam.rs: kh-camera `FormBpOutput::blue_hour`, N425's start / end).
pub fn set(on: bool) {
    ON.store(on, Ordering::Relaxed);
}

/// (AUDIT-B W-FIN) FINISH_SK's ceiling look on or off (kh-er cam.rs: kh-camera `FormBpOutput::tower`).
pub fn set_tower(on: bool) {
    TOWER.store(on, Ordering::Relaxed);
}

/// (AUDIT-B W-FIN) Is FINISH_SK's ceiling look on?
pub fn tower() -> bool {
    TOWER.load(Ordering::Relaxed)
}

/// Is one of the two looks on (the map hidden, the scene lights off, the hybrid capture off)?
pub fn active() -> bool {
    ON.load(Ordering::Relaxed) || tower() // (AUDIT-B W-FIN) + the ceiling look
}

/// (AUDIT-B W-FIN) The sky light that lights the characters while a look is on: the Blue Hour's, else the tower's.
pub fn sky_light() -> glam::Vec3 {
    if ON.load(Ordering::Relaxed) {
        SKY_LIGHT
    } else {
        TOWER_SKY_LIGHT
    }
}

/// Is a pixel of this stencil value kept (a character's) or emptied (the map, the sky, props)?
pub fn kept(stencil: u8) -> bool {
    stencil & CLASS_MASK == CHARACTER_CLASS
}

/// What gfx.rs hands over: its list with the scene target and the scene depth bound (DEPTH_WRITE), its root signature (the
/// shader declares the frame's cbuffers but reads none of them).
pub(crate) struct BlueHourCx<'a> {
    pub device: &'a ID3D12Device,
    pub list: &'a ID3D12GraphicsCommandList,
    pub root: &'a ID3D12RootSignature,
    pub target_format: DXGI_FORMAT,
    pub depth_format: DXGI_FORMAT,
}

struct Kit {
    device: usize,
    target: i32,
    depth: i32,
    pipeline: ID3D12PipelineState,
}

// SAFETY: the pipeline is used on the render thread only (the Mutex serialises the rest).
unsafe impl Send for Kit {}

static KIT: Mutex<Option<Kit>> = Mutex::new(None);
static FAILED: AtomicBool = AtomicBool::new(false);

fn shader(name: &str) -> Option<Vec<u8>> {
    if let Some(bytes) = std::env::var_os("KH_BLUEHOUR_SHADERS").and_then(|d| std::fs::read(std::path::Path::new(&d).join(name)).ok()) {
        return Some(bytes);
    }
    let pack = crate::SHARED.lock().ok().and_then(|g| g.as_ref().map(|s| s.pack.clone()))?;
    std::fs::read(pack.join("materials").join("er").join(name)).ok()
}

/// The DSV format of a depth resource format (the stencil test needs a format with stencil).
fn dsv_format(format: DXGI_FORMAT) -> Option<DXGI_FORMAT> {
    match format {
        DXGI_FORMAT_R32G8X24_TYPELESS | DXGI_FORMAT_D32_FLOAT_S8X24_UINT => Some(DXGI_FORMAT_D32_FLOAT_S8X24_UINT),
        DXGI_FORMAT_R24G8_TYPELESS | DXGI_FORMAT_D24_UNORM_S8_UINT => Some(DXGI_FORMAT_D24_UNORM_S8_UINT),
        _ => None,
    }
}

unsafe fn make(device: &ID3D12Device, root: &ID3D12RootSignature, target: DXGI_FORMAT, depth: DXGI_FORMAT) -> Result<ID3D12PipelineState, String> {
    let (Some(vertex), Some(pixel)) = (shader("blob.vs.dxil"), shader("bluehour.ps.dxil")) else { return Err("materials/er/bluehour.ps.dxil (or blob.vs.dxil) missing".to_string()) };
    let mut formats = [DXGI_FORMAT_UNKNOWN; 8];
    formats[0] = target;
    let mut blend = D3D12_BLEND_DESC::default();
    blend.RenderTarget[0].RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8;
    let keep = D3D12_DEPTH_STENCILOP_DESC { StencilFailOp: D3D12_STENCIL_OP_KEEP, StencilDepthFailOp: D3D12_STENCIL_OP_KEEP, StencilPassOp: D3D12_STENCIL_OP_KEEP, StencilFunc: D3D12_COMPARISON_FUNC_NOT_EQUAL };
    let pso = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
        pRootSignature: std::mem::transmute_copy(root),
        VS: D3D12_SHADER_BYTECODE { pShaderBytecode: vertex.as_ptr() as *const c_void, BytecodeLength: vertex.len() },
        PS: D3D12_SHADER_BYTECODE { pShaderBytecode: pixel.as_ptr() as *const c_void, BytecodeLength: pixel.len() },
        BlendState: blend,
        SampleMask: u32::MAX,
        RasterizerState: D3D12_RASTERIZER_DESC { FillMode: D3D12_FILL_MODE_SOLID, CullMode: D3D12_CULL_MODE_NONE, DepthClipEnable: false.into(), ..Default::default() },
        // every non-character pixel: depth written (SV_Depth 0) whatever was there, the stencil read only
        DepthStencilState: D3D12_DEPTH_STENCIL_DESC { DepthEnable: true.into(), DepthWriteMask: D3D12_DEPTH_WRITE_MASK_ALL, DepthFunc: D3D12_COMPARISON_FUNC_ALWAYS, StencilEnable: true.into(), StencilReadMask: CLASS_MASK, StencilWriteMask: 0, FrontFace: keep, BackFace: keep },
        PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
        NumRenderTargets: 1,
        RTVFormats: formats,
        DSVFormat: depth,
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        ..Default::default()
    };
    device.CreateGraphicsPipelineState(&pso).map_err(|e| format!("the Blue Hour's pipeline: {e}"))
}

/// Records the Blue Hour's pass into the bound target and depth (nothing when it is off or cannot run). True: recorded (the
/// caller's pipeline state and stencil reference must be set again).
pub(crate) unsafe fn draw(cx: &BlueHourCx) -> bool {
    if !active() || FAILED.load(Ordering::Relaxed) {
        return false;
    }
    let Some(depth) = dsv_format(cx.depth_format) else { return false };
    let Ok(mut guard) = KIT.try_lock() else { return false };
    let fresh = guard.as_ref().is_none_or(|k| k.device != cx.device.as_raw() as usize || k.target != cx.target_format.0 || k.depth != depth.0);
    if fresh {
        match make(cx.device, cx.root, cx.target_format, depth) {
            Ok(pipeline) => *guard = Some(Kit { device: cx.device.as_raw() as usize, target: cx.target_format.0, depth: depth.0, pipeline }),
            Err(e) => {
                crate::rlog!("bluehour: (AUDIT-C) {e}: the Blue Hour's map hide is not drawn");
                FAILED.store(true, Ordering::Relaxed);
                return false;
            }
        }
    }
    let Some(kit) = guard.as_ref() else { return false };
    cx.list.SetPipelineState(&kit.pipeline);
    cx.list.OMSetStencilRef(u32::from(CHARACTER_CLASS));
    cx.list.IASetPrimitiveTopology(windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
    cx.list.DrawInstanced(3, 1, 0, 0);
    cx.list.OMSetStencilRef(0);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Elden Ring's measured classes (gfx.rs CHAR_STENCIL notes): characters 0x50 (and the KH characters' early depth) are
    /// kept; terrain 0x12 / 0xd2 / 0x90, map objects 0x10 and the sky 0 are emptied.
    #[test]
    fn the_characters_class_is_kept_the_map_is_not() {
        for s in [0x50u8, 0x51, 0x5F] {
            assert!(kept(s), "{s:#x}");
        }
        for s in [0x00u8, 0x10, 0x12, 0x90, 0xD2, 0x03] {
            assert!(!kept(s), "{s:#x}");
        }
        set(true);
        assert!(active());
        set(false);
        assert!(!active());
        // (AUDIT-B W-FIN) the ceiling look: the same hide, the tower's sky light
        set_tower(true);
        assert!(active() && tower());
        assert_eq!(sky_light(), TOWER_SKY_LIGHT);
        set(true);
        assert_eq!(sky_light(), SKY_LIGHT);
        set(false);
        set_tower(false);
        assert!(!active() && !tower());
    }
}
