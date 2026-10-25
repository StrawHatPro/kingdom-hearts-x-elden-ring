//! (ER87, the user: "fix Sora's hair on ALL of his costumes so it looks like this close up, it looks buggy as it is now")
//! THE HAIR PATH.
//!
//! What made the hair jagged, holey and broken up (IMPL_ER87_STATE.md):
//! 1. KH3's hair material (SQEX_KHHairBaseMat, every outfit's hair owner) masks its strands with a DitherTemporalAA screen
//!    door: phase = (x + 2 y) mod 5 of the pixel, which KH3's TAA averages into soft strand ends. The translator dropped the
//!    `udiv` that takes that remainder (tools/sora_material_pack.js, fixed), so the phase was a huge number and every
//!    partly covered texel of the strand masks became fully opaque: hard, ragged card edges, stray slivers of the
//!    hair-line textures, no tapered ends.
//! 2. His masked draws wrote the depth (and the early depth, the stencil) wherever ANY coverage was left, so a thin strand
//!    end hid the hair behind it.
//!
//! The fix, data-driven per material slot (kh-shaderc `--hair`): beside the shader the renderer picks for a hair slot,
//! `<name>_hair.ps.dxil` (er_char.wgsl KH_HAIR_CORE) and `<name>_hairf.ps.dxil` (KH_HAIR_FRINGE), built from the hair
//! translation with the remainder restored and two hair-path changes (er87/tools/hair_fade.js): the strands' view-angle
//! fade kept at 1 (taken 1:1 it made whole camera-facing cards half transparent) and the dither phase handed in, so
//! char_common.wgsl KH_HAIR takes the coverage over 16 even phases (no screen pattern, no 5-step staircase).
//! - CORE: coverage >= 1/2, opaque; writes the depth, the early depth, the stencil, the glow mask and the motion vectors.
//! - FRINGE: coverage below that (the strand ends), alpha rising to 1 at the cut; drawn after his own draws with the depth
//!   test GREATER and no depth write: at the game's velocity split (after its forward stage, whose depth-tested sky and
//!   far field would otherwise cover it; before its TAA) while he is nearer than `r.hair_late_far` m, else right after him.
//!   Also out of the game's glow source x (1 - alpha).
//! Nothing changes from frame to frame, so Elden Ring's TAA (our draws carry its jitter) resolves a stable edge.
//! `r.hair = 0` (kh-er): the files are not used (exactly the draws as before); a change reloads the drawn outfit.
//! `r.hair_late = 0`: the fringe right after him always. The fur of Monstropolis (a mod-6 dither without the phase input):
//! KH_HAIR6, six screen phases.
//!
//! (ER91, the user: "the tips of Sora's hair is also still broken, it needs to look like this image") ER91's FILES
//! (`<name>_h91` core, `_h91f` fringe, `_h91m` / `_h91n` the 4x pass's halves, `_gb2_h91` the hybrid's G-buffer; built by
//! kh-shaderc `--hair91` from er91/tools/hair91.js's translation), taken when the outfit has them and `r.hair91` is on:
//! - THE CAUSE OF THE BLUNT, CARD-WIDE ENDS: the strand-end mask (hairlineendmask2) is CLAMP across U and WRAP across V; the
//!   translator gave it one clamp sampler, so every card outside v x HairMaskUVRange in [0, 1] (the hair's second UV set runs
//!   -1.3 .. 2.3) read the mask's first or last row: one strand profile for the whole card instead of 16 tapered strands per
//!   repeat. ER91 samples it clamp-U / wrap-V;
//! - the coverage is continuous (the d sweep's crossing found by bisection, no 16-step staircase) and taken at four sub-pixel
//!   points (char_common.wgsl `kh91_cover`); KH3's view-angle fade is kept 1:1 (ER87 held it at 1: the half transparent
//!   camera-facing cards it saw came from the clamped mask);
//! - near the camera (`r.hair_msaa_far`) the hair is drawn once more in its own 4x pass at the split (hair91.rs).

use std::collections::HashMap;
use std::ffi::c_void;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};

use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;

static ON: AtomicBool = AtomicBool::new(true);
/// The shaders as last read: 0 = no hair files for the drawn outfit, 1 = read with the hair path (ER87's files), 2 = there,
/// not used, 3 = read with ER91's files.
static READ: AtomicU8 = AtomicU8::new(0);
/// (ER91) which hair files the drawn outfit has: bit 0 = ER87's (`_hair` / `_hairf`), bit 1 = ER91's (`_h91` / `_h91f`).
static FILES: AtomicU8 = AtomicU8::new(0);
/// (ER91, `r.hair91`, default 1) ER91's hair files when the outfit has them (the strand mask's sampler fixed, the continuous
/// supersampled coverage); 0 = ER87's.
static V91: AtomicBool = AtomicBool::new(true);

/// The host's key (kh-er `r.hair91`, default 1). A change reloads the drawn outfit.
pub fn set_v91(on: bool) {
    V91.store(on, Ordering::Relaxed);
}

pub fn v91() -> bool {
    V91.load(Ordering::Relaxed)
}

/// (ER91) the mode a read takes with the files `files` (FILES bits) and the keys as they are now (READ's values).
fn wanted_mode(files: u8) -> u8 {
    match (files, on(), v91()) {
        (0, ..) => 0,
        (_, false, _) => 2,
        (f, true, true) if f & 2 != 0 => 3,
        (f, true, _) if f & 1 != 0 => 1,
        _ => 2,
    }
}
/// Fringe shaders of the last read (pipeline states can be made).
static FRINGE: AtomicBool = AtomicBool::new(false);
/// Fringe draws since the last status line.
static DRAWS: AtomicU32 = AtomicU32::new(0);
/// (`r.hair_late`, default 1) the fringe drawn at the game's velocity split (after its forward stage, before its TAA).
static LATE: AtomicBool = AtomicBool::new(true);
/// Frames with the fringe drawn at the split / missed there since the last status line, and the latest reason.
static LATE_DRAWN: AtomicU32 = AtomicU32::new(0);
static LATE_MISSED: AtomicU32 = AtomicU32::new(0);
static LATE_WHY: std::sync::Mutex<&'static str> = std::sync::Mutex::new("");

/// The host's key (kh-er `r.hair_late`, default 1): 0 = the fringe drawn right after his own draws (it is then covered by
/// whatever the game's forward stage draws later behind him: its sky, far field and translucents).
pub fn set_late(on: bool) {
    LATE.store(on, Ordering::Relaxed);
}

pub fn late() -> bool {
    LATE.load(Ordering::Relaxed)
}

/// (`r.hair_late_far`, metres, default 4) the late fringe only while he is nearer the camera than this.
static LATE_FAR: AtomicU32 = AtomicU32::new(0x4080_0000);

pub fn set_late_far(metres: f32) {
    LATE_FAR.store(metres.max(0.0).to_bits(), Ordering::Relaxed);
}

pub fn late_far() -> f32 {
    f32::from_bits(LATE_FAR.load(Ordering::Relaxed))
}

pub(crate) fn drawn_late() {
    LATE_DRAWN.fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn missed(why: &'static str) {
    LATE_MISSED.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut w) = LATE_WHY.lock() {
        *w = why;
    }
}

/// The host's key (kh-er `r.hair`, default 1).
pub fn set_on(on: bool) {
    ON.store(on, Ordering::Relaxed);
}

pub fn on() -> bool {
    ON.load(Ordering::Relaxed)
}

/// The drawn outfit has hair files and the key no longer matches how its shaders were read: it is loaded again.
pub fn reload_wanted() -> bool {
    let files = FILES.load(Ordering::Relaxed);
    files != 0 && wanted_mode(files) != READ.load(Ordering::Relaxed)
}

/// A translucent slot drawn after the late fringe (at the velocity split) instead of in his colour list: a cover that hair
/// can lie behind (San Fransokyo's visor: owner SQEX_VisorGrass_DisplayMode). The eyes' lenses and lashes (owners with
/// `_E_`, `Eye`, `Matsuge`) stay in the colour list: hair lies in front of them, never behind.
pub(crate) fn defer_translucent(owner: &str) -> bool {
    !(owner.contains("_E_") || owner.contains("Eye") || owner.contains("Matsuge"))
}

/// The fringe's pipeline states may be made (the last read took the hair path and has fringe shaders).
pub(crate) fn fringe_ready() -> bool {
    on() && FRINGE.load(Ordering::Relaxed)
}

/// The hair files beside `stem` (the name the renderer picked for a slot, without `.ps.dxil`): (core, fringe) file names,
/// when the slot is masked and BOTH are in `dir`.
pub(crate) fn pick(dir: &Path, stem: &str, masked: bool) -> Option<(String, String)> {
    let core = format!("{stem}_hair.ps.dxil");
    let edge = format!("{stem}_hairf.ps.dxil");
    (masked && dir.join(&core).is_file() && dir.join(&edge).is_file()).then_some((core, edge))
}

/// (ER91) ER91's hair files beside `stem`: (core, fringe) and, when both are there too, the 4x pass's (solid, thin) ones.
pub(crate) fn pick91(dir: &Path, stem: &str, masked: bool) -> Option<((String, String), Option<(String, String)>)> {
    let core = format!("{stem}_h91.ps.dxil");
    let edge = format!("{stem}_h91f.ps.dxil");
    if !(masked && dir.join(&core).is_file() && dir.join(&edge).is_file()) {
        return None;
    }
    let (solid, thin) = (format!("{stem}_h91m.ps.dxil"), format!("{stem}_h91n.ps.dxil"));
    let msaa = (dir.join(&solid).is_file() && dir.join(&thin).is_file()).then_some((solid, thin));
    Some(((core, edge), msaa))
}

/// (ER91) the mode a read with these files (bit 0 ER87's, bit 1 ER91's) takes now: 0 none, 1 ER87's, 2 off, 3 ER91's.
pub(crate) fn mode_for(files: u8) -> u8 {
    wanted_mode(files)
}

/// gfx.rs `read_shaders`: what this read did (`files`: bit 0 = ER87's hair files, bit 1 = ER91's).
pub(crate) fn note_read<T>(files: u8, fringe: &HashMap<String, T>) {
    let mode = wanted_mode(files);
    FILES.store(files, Ordering::Relaxed);
    READ.store(mode, Ordering::Relaxed);
    FRINGE.store((mode == 1 || mode == 3) && !fringe.is_empty(), Ordering::Relaxed);
    crate::rlog!(
        "render: (ER87/ER91) the hair path: {}",
        match mode {
            0 => "no hair files for this outfit (the draws as before)".to_string(),
            1 => format!("{} hair variant(s) with ER87's core and fringe shaders (r.hair = 0 switches it off){}", fringe.len(), if files & 2 != 0 { "; ER91's are there, r.hair91 = 0" } else { "; no ER91 files" }),
            3 => format!("{} hair variant(s) with ER91's core and fringe shaders (the strand mask's sampler fixed, the continuous coverage; r.hair91 = 0: ER87's, r.hair = 0: off)", fringe.len()),
            _ => "switched off (r.hair = 0): the draws as before".to_string(),
        }
    );
}

/// The target a fringe set is made for (gfx.rs `PsoKey`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Target {
    pub format: i32,
    pub depth: i32,
    pub front_ccw: bool,
    pub cull: bool,
    pub alpha: bool,
}

/// The fringe's states, one per slot (None: a slot without a fringe shader): the slot's vertex shader input, its fringe
/// pixel shader and rasteriser; depth tested GREATER (reversed Z: strictly in front of what his opaque / masked draws left),
/// not written. Colour: alpha blended over what is there (the target's alpha as the colour draws treat it). `glow`: the
/// game's glow source, kept x (1 - coverage) (nothing of the shader's colour reaches it).
///
/// # Safety
/// Direct3D object creation with a live device and root signature.
#[allow(clippy::too_many_arguments)]
pub(crate) unsafe fn fringe_pipelines(device: &ID3D12Device, root: &ID3D12RootSignature, key: Target, glow: bool, slots: &[(String, bool, bool, bool)], vertex: &[u8], fringe: &HashMap<String, Vec<u8>>, elements: &[D3D12_INPUT_ELEMENT_DESC]) -> Result<Vec<Option<ID3D12PipelineState>>, String> {
    let mut out = Vec::with_capacity(slots.len());
    for (variant, two_sided, translucent, masked) in slots {
        let Some(pixel) = fringe.get(variant).filter(|_| *masked && !*translucent) else {
            out.push(None);
            continue;
        };
        let mut blend = D3D12_BLEND_DESC::default();
        blend.RenderTarget[0] = if glow {
            D3D12_RENDER_TARGET_BLEND_DESC {
                BlendEnable: true.into(),
                LogicOpEnable: false.into(),
                SrcBlend: D3D12_BLEND_ZERO,
                DestBlend: D3D12_BLEND_INV_SRC_ALPHA,
                BlendOp: D3D12_BLEND_OP_ADD,
                SrcBlendAlpha: D3D12_BLEND_ZERO,
                DestBlendAlpha: D3D12_BLEND_INV_SRC_ALPHA,
                BlendOpAlpha: D3D12_BLEND_OP_ADD,
                LogicOp: D3D12_LOGIC_OP_NOOP,
                RenderTargetWriteMask: D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8,
            }
        } else {
            D3D12_RENDER_TARGET_BLEND_DESC {
                BlendEnable: true.into(),
                LogicOpEnable: false.into(),
                SrcBlend: D3D12_BLEND_SRC_ALPHA,
                DestBlend: D3D12_BLEND_INV_SRC_ALPHA,
                BlendOp: D3D12_BLEND_OP_ADD,
                SrcBlendAlpha: if key.alpha { D3D12_BLEND_ONE } else { D3D12_BLEND_ZERO },
                DestBlendAlpha: if key.alpha { D3D12_BLEND_INV_SRC_ALPHA } else { D3D12_BLEND_ONE },
                BlendOpAlpha: D3D12_BLEND_OP_ADD,
                LogicOp: D3D12_LOGIC_OP_NOOP,
                RenderTargetWriteMask: if key.alpha { D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8 } else { (D3D12_COLOR_WRITE_ENABLE_RED.0 | D3D12_COLOR_WRITE_ENABLE_GREEN.0 | D3D12_COLOR_WRITE_ENABLE_BLUE.0) as u8 },
            }
        };
        let mut formats = [DXGI_FORMAT_UNKNOWN; 8];
        formats[0] = DXGI_FORMAT(key.format);
        let desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            // SAFETY: a borrowed copy of the interface pointer for the duration of the call.
            pRootSignature: std::mem::transmute_copy(root),
            VS: D3D12_SHADER_BYTECODE { pShaderBytecode: vertex.as_ptr() as *const c_void, BytecodeLength: vertex.len() },
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
            DepthStencilState: D3D12_DEPTH_STENCIL_DESC { DepthEnable: true.into(), DepthWriteMask: D3D12_DEPTH_WRITE_MASK_ZERO, DepthFunc: D3D12_COMPARISON_FUNC_GREATER, StencilEnable: false.into(), ..Default::default() },
            InputLayout: D3D12_INPUT_LAYOUT_DESC { pInputElementDescs: elements.as_ptr(), NumElements: elements.len() as u32 },
            PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
            NumRenderTargets: 1,
            RTVFormats: formats,
            DSVFormat: DXGI_FORMAT(key.depth),
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            ..Default::default()
        };
        let state: ID3D12PipelineState = device.CreateGraphicsPipelineState(&desc).map_err(|e| format!("CreateGraphicsPipelineState(hair fringe {variant}{}): {e}", if glow { ", glow mask" } else { "" }))?;
        out.push(Some(state));
    }
    Ok(out)
}

/// One draw of a hair slot as gfx.rs recorded it: (model, slot, index count, first index, base vertex, skin, skin a frame
/// ago, material constants).
pub(crate) type HairDraw = (usize, usize, u32, u32, i32, u64, u64, u64);

/// The fringe draws on `list` (its root signature, heaps, frame constants, targets and viewport are bound): per draw the
/// model's buffers, the skins, the slot's fringe state, its material constants and texture table.
///
/// # Safety
/// `list` is open and recording with the renderer's root signature and heaps bound.
pub(crate) unsafe fn draw_fringe(list: &ID3D12GraphicsCommandList, set: &[Option<ID3D12PipelineState>], draws: &[HairDraw], model: &dyn Fn(usize) -> Option<(D3D12_VERTEX_BUFFER_VIEW, D3D12_INDEX_BUFFER_VIEW)>, table_start: u64, table_stride: u64) {
    let mut bound = usize::MAX;
    let mut n = 0u32;
    for (model_index, slot, count, first_index, base_vertex, skin_now, skin_then, material) in draws {
        let (Some(Some(pso)), Some((vertex_view, index_view))) = (set.get(*slot), model(*model_index)) else { continue };
        if bound != *model_index {
            list.IASetVertexBuffers(0, Some(&[vertex_view]));
            list.IASetIndexBuffer(Some(&index_view));
            bound = *model_index;
        }
        list.SetGraphicsRootConstantBufferView(2, *skin_now);
        list.SetGraphicsRootConstantBufferView(6, *skin_then);
        list.SetPipelineState(pso);
        list.SetGraphicsRootConstantBufferView(1, *material);
        list.SetGraphicsRootDescriptorTable(3, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: table_start + *slot as u64 * table_stride });
        list.DrawIndexedInstanced(*count, 1, *first_index, *base_vertex, 0);
        n += 1;
    }
    DRAWS.fetch_add(n, Ordering::Relaxed);
}

/// The status line's part (gfx.rs, once a second): fringe draws since the last line.
pub(crate) fn status() -> Option<String> {
    let n = DRAWS.swap(0, Ordering::Relaxed);
    match READ.load(Ordering::Relaxed) {
        r @ (1 | 3) => {
            let (d, m) = (LATE_DRAWN.swap(0, Ordering::Relaxed), LATE_MISSED.swap(0, Ordering::Relaxed));
            let why = LATE_WHY.lock().map(|w| *w).unwrap_or("");
            Some(format!("({}) hair path on: {n} fringe draw(s) this second{}; at the velocity split (r.hair_late {}): {d} frame(s), {m} missed{}{}", if r == 3 { "ER91" } else { "ER87" }, if FRINGE.load(Ordering::Relaxed) { "" } else { " (no fringe shaders)" }, late() as u8, if m > 0 { format!(" (latest: {why})") } else { String::new() }, if r == 3 { crate::hair91::status() } else { String::new() }))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The staged hair files (er87/pack): every outfit's hair variant has its core and fringe for every set the renderer may
    /// pick. Skipped without the folder.
    #[test]
    fn the_staged_hair_sets_are_complete() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/er/er87/pack");
        if !root.is_dir() {
            return;
        }
        for (dir, variants) in [("materials/er", &["c0ms"][..]), ("outfits/kh1/materials/er", &["c8ms"]), ("outfits/kh2/materials/er", &["c6ms"]), ("outfits/caribbean/materials/er", &["c0ms", "c0ms_real"]), ("outfits/sanfransokyo/materials/er", &["c12ms", "c12ms_cg"]), ("outfits/verumrex/materials/er", &["c9ms", "c10ms", "c9ms_cg", "c10ms_cg", "c9ms_cg2", "c10ms_cg2"]), ("outfits/monstropolis/materials/er", &["c3ms", "c3ms_cg"])] {
            for v in variants {
                for set in ["", "_pl", "_bl", "_bli", "_hy2"] {
                    assert!(pick(&root.join(dir), &format!("{v}{set}"), true).is_some(), "{dir}/{v}{set}");
                }
            }
        }
    }

    /// (ER91) The staged ER91 hair files (er91/pack): every outfit's hair variant has its core, fringe and 4x pair for every
    /// set the renderer may pick, the hybrid's G-buffer writer, and the 4x pass's screen shaders are there. Skipped without
    /// the folder.
    #[test]
    fn er91_the_staged_hair_sets_are_complete() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/er/er91/pack");
        if !root.is_dir() {
            return;
        }
        for name in ["h91.vs", "h91init.ps", "h91init1.ps", "h91comp.ps", "h91velo.ps"] {
            assert!(root.join("materials/er").join(format!("{name}.dxil")).is_file(), "{name}");
        }
        for (dir, variants) in [("materials/er", &["c0ms"][..]), ("outfits/kh1/materials/er", &["c8ms"]), ("outfits/kh2/materials/er", &["c6ms"]), ("outfits/caribbean/materials/er", &["c0ms", "c0ms_real"]), ("outfits/sanfransokyo/materials/er", &["c12ms", "c12ms_cg"]), ("outfits/verumrex/materials/er", &["c9ms", "c10ms", "c9ms_cg", "c10ms_cg", "c9ms_cg2", "c10ms_cg2"]), ("outfits/monstropolis/materials/er", &["c3ms", "c3ms_cg"])] {
            for v in variants {
                for set in ["", "_pl", "_bl", "_bli", "_hy2"] {
                    let picked = pick91(&root.join(dir), &format!("{v}{set}"), true);
                    assert!(picked.as_ref().is_some_and(|p| p.1.is_some()), "{dir}/{v}{set}");
                }
                let (base, own) = v.split_once('_').map_or((*v, String::new()), |(b, s)| (b, format!("_{s}")));
                assert!(root.join(dir).join(format!("{base}{own}_gb2_h91.ps.dxil")).is_file(), "{dir}/{v} gb2_h91");
            }
        }
    }

    #[test]
    fn the_key_and_the_files_decide() {
        let dir = std::env::temp_dir().join(format!("kh_er87_hair_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("c0ms_bl_hair.ps.dxil"), b"x").unwrap();
        assert!(pick(&dir, "c0ms_bl", true).is_none(), "the core alone is not enough");
        std::fs::write(dir.join("c0ms_bl_hairf.ps.dxil"), b"x").unwrap();
        assert_eq!(pick(&dir, "c0ms_bl", true), Some(("c0ms_bl_hair.ps.dxil".to_string(), "c0ms_bl_hairf.ps.dxil".to_string())));
        assert!(pick(&dir, "c0ms_bl", false).is_none(), "only masked slots");
        assert!(pick(&dir, "c0ms_pl", true).is_none(), "per picked set");
        let _ = std::fs::remove_dir_all(&dir);
        // (the visor goes after the late fringe; lenses and lashes of every outfit stay)
        assert!(defer_translucent("SQEX_VisorGrass_DisplayMode"));
        for eye in ["p_ex001_E_mat1_Inst", "p_ex001_E_mat3_Inst", "SQEX_DLC_MatsugeBaseMat"] {
            assert!(!defer_translucent(eye), "{eye}");
        }
        let none: HashMap<String, ()> = HashMap::new();
        let one: HashMap<String, ()> = HashMap::from([("c0ms".to_string(), ())]);
        let _guard = KEYS.lock().unwrap_or_else(|e| e.into_inner());
        set_on(true);
        set_v91(true);
        note_read(1, &one);
        assert!(!reload_wanted() && fringe_ready());
        set_on(false);
        assert!(reload_wanted() && !fringe_ready());
        note_read(1, &none);
        assert!(!reload_wanted());
        set_on(true);
        assert!(reload_wanted());
        note_read(0, &none);
        assert!(!reload_wanted());
        set_on(true);
    }

    static KEYS: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// (ER91) ER91's files win while `r.hair91` is on; switching it (or `r.hair`) asks for the outfit again; without ER91's
    /// files the key changes nothing.
    #[test]
    fn er91_files_and_keys_decide_the_mode() {
        let _guard = KEYS.lock().unwrap_or_else(|e| e.into_inner());
        let one: HashMap<String, ()> = HashMap::from([("c0ms".to_string(), ())]);
        set_on(true);
        set_v91(true);
        assert_eq!((mode_for(0), mode_for(1), mode_for(2), mode_for(3)), (0, 1, 3, 3));
        note_read(3, &one);
        assert!(!reload_wanted() && fringe_ready());
        set_v91(false);
        assert_eq!(mode_for(3), 1);
        assert!(reload_wanted(), "r.hair91 0: ER87's files");
        note_read(3, &one);
        assert!(!reload_wanted());
        set_on(false);
        assert_eq!(mode_for(3), 2);
        assert!(reload_wanted());
        set_on(true);
        set_v91(true);
        note_read(1, &one);
        set_v91(false);
        assert!(!reload_wanted(), "no ER91 files: the key changes nothing");
        set_v91(true);
        // the 4x pass's files are picked with the core / fringe only
        let dir = std::env::temp_dir().join(format!("kh_er91_hair_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for f in ["c0ms_hy2_h91.ps.dxil", "c0ms_hy2_h91f.ps.dxil", "c0ms_hy2_h91m.ps.dxil"] {
            std::fs::write(dir.join(f), b"x").unwrap();
        }
        assert_eq!(pick91(&dir, "c0ms_hy2", true), Some((("c0ms_hy2_h91.ps.dxil".to_string(), "c0ms_hy2_h91f.ps.dxil".to_string()), None)));
        std::fs::write(dir.join("c0ms_hy2_h91n.ps.dxil"), b"x").unwrap();
        assert_eq!(pick91(&dir, "c0ms_hy2", true).and_then(|p| p.1), Some(("c0ms_hy2_h91m.ps.dxil".to_string(), "c0ms_hy2_h91n.ps.dxil".to_string())));
        assert!(pick91(&dir, "c0ms_hy2", false).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
