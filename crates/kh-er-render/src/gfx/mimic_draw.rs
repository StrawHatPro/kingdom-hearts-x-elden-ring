//! (MIMIC SORA, THE USER 2026-11-02: "Make sure mimic Sora uses the shaders that mimic should use from Elden Ring.") THE MIMIC
//! TEAR'S G-BUFFER WRITERS IN THE RENDERER'S FRAME (crate::mimic has the rule and the host's side): one pipeline state per
//! material slot of Sora's library (`<variant>[_S]_gb3_mm.ps.dxil`, kh-shaderc `--only mimic`; None for a slot without one: a
//! translucent slot, an XS form weapon's), made on the worker beside his other sets once a mimic was submitted, and the
//! mimic's G-buffer draws of a frame (gfx.rs `Core::draw`, inside his G-buffer pass) into the game's FIVE targets.

use std::ffi::c_void;
use std::path::Path;

use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;

use super::{layout, transition, Assets, PsoKey, SLOT_TEXTURES};

/// The file of a slot's Mimic Tear G-buffer writer in a shader folder (`suffix`: an own set's, "" for the pack's).
pub(super) fn file_of(dir: &Path, variant: &str, suffix: &str, stem: &str) -> std::path::PathBuf {
    dir.join(format!("{variant}{suffix}{stem}.ps.dxil"))
}

/// The two writer sets: the whole mimic's (early depth before, depth EQUAL) and his FORMING one (THE USER 2026-11-03: his
/// arrival and dissolve; its own depth written, nothing above the cut: kh-shaderc `--only mimicform`).
pub(super) const WHOLE: &str = "_gb3_mm";
pub(super) const FORM: &str = "_gb3_mf";

/// The game's subsurface and emissive targets (roles 3 / 4) are of the formats the writers are made for.
pub(super) fn targets_known(ordered: &[crate::hybrid::GbTarget; crate::hybrid::ROLES]) -> bool {
    let typed = |k: usize| crate::hybrid::role_of(ordered[k].format).map_or(ordered[k].format, |(_, f)| f);
    typed(3) == crate::hybrid::fmt::R8G8_UNORM && typed(4) == crate::hybrid::fmt::R16G16B16A16_FLOAT
}

/// The writers' five targets in the game's role order (hybrid.rs `role_of`).
fn formats() -> [DXGI_FORMAT; 8] {
    let mut f = [DXGI_FORMAT_UNKNOWN; 8];
    for (k, typed) in [crate::hybrid::fmt::R10G10B10A2_UNORM, crate::hybrid::fmt::R8G8B8A8_UNORM, crate::hybrid::fmt::R8G8B8A8_UNORM_SRGB, crate::hybrid::fmt::R8G8_UNORM, crate::hybrid::fmt::R16G16B16A16_FLOAT].into_iter().enumerate() {
        f[k] = DXGI_FORMAT(typed);
    }
    f
}

/// THE MIMIC SET for `key`: per slot (variant, two sided, translucent, masked) its writer's state (the G-buffer writers'
/// description of gfx.rs `pipelines` with five targets: depth EQUAL to what his early depth left, no write, the slot's
/// rasteriser). Err when not one slot of the body has a file (the pack or the worn outfit has no mimic set).
pub(super) unsafe fn pipelines(device: &ID3D12Device, root: &ID3D12RootSignature, key: PsoKey, slots: &[(String, bool, bool, bool)], vertex: &[u8], dir: &Path, suffix: &str) -> Result<Vec<Option<ID3D12PipelineState>>, String> {
    set_of(device, root, key, slots, vertex, (dir, suffix, WHOLE))
}

/// THE FORMING SET for `key` (`<variant>[_S]_gb3_mf`): as the whole set but drawn WITHOUT his early depth, so it writes his
/// depth itself (GREATER_EQUAL, written; the cut's discard keeps the world above it). Err when the folder has none.
pub(super) unsafe fn form_pipelines(device: &ID3D12Device, root: &ID3D12RootSignature, key: PsoKey, slots: &[(String, bool, bool, bool)], vertex: &[u8], dir: &Path, suffix: &str) -> Result<Vec<Option<ID3D12PipelineState>>, String> {
    set_of(device, root, key, slots, vertex, (dir, suffix, FORM))
}

unsafe fn set_of(device: &ID3D12Device, root: &ID3D12RootSignature, key: PsoKey, slots: &[(String, bool, bool, bool)], vertex: &[u8], (dir, suffix, stem): (&Path, &str, &str)) -> Result<Vec<Option<ID3D12PipelineState>>, String> {
    let form = stem == FORM;
    let elements = layout();
    let mut blend = D3D12_BLEND_DESC { IndependentBlendEnable: false.into(), ..Default::default() };
    for t in blend.RenderTarget.iter_mut() {
        t.RenderTargetWriteMask = D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8;
    }
    let mut out = Vec::with_capacity(slots.len());
    for (variant, two_sided, translucent, _) in slots {
        let pixel = if *translucent { None } else { std::fs::read(file_of(dir, variant, suffix, stem)).ok() };
        let Some(pixel) = pixel else {
            out.push(None);
            continue;
        };
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
            DepthStencilState: depth_state(form),
            InputLayout: D3D12_INPUT_LAYOUT_DESC { pInputElementDescs: elements.as_ptr(), NumElements: elements.len() as u32 },
            PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
            NumRenderTargets: crate::hybrid::ROLES as u32,
            RTVFormats: formats(),
            DSVFormat: DXGI_FORMAT(key.depth),
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            ..Default::default()
        };
        let state: ID3D12PipelineState = crate::psocache::create(device, &desc).map_err(|e| format!("CreateGraphicsPipelineState(mimic G-buffer {variant}): {e}"))?;
        out.push(Some(state));
    }
    if out.iter().all(Option::is_none) {
        return Err(format!("no <variant>{suffix}{stem}.ps.dxil file in {} (cargo run --release -p kh-shaderc -- --only {})", dir.display(), if form { "mimicform" } else { "mimic" }));
    }
    Ok(out)
}

/// The writers' depth: the whole mimic's EQUAL to what his early depth left (no write); his forming's written by itself
/// (GREATER_EQUAL, reversed Z: nearer is larger).
fn depth_state(form: bool) -> D3D12_DEPTH_STENCIL_DESC {
    if form {
        D3D12_DEPTH_STENCIL_DESC { DepthEnable: true.into(), DepthWriteMask: D3D12_DEPTH_WRITE_MASK_ALL, DepthFunc: D3D12_COMPARISON_FUNC_GREATER_EQUAL, StencilEnable: false.into(), ..Default::default() }
    } else {
        D3D12_DEPTH_STENCIL_DESC { DepthEnable: true.into(), DepthWriteMask: D3D12_DEPTH_WRITE_MASK_ZERO, DepthFunc: D3D12_COMPARISON_FUNC_EQUAL, StencilEnable: false.into(), ..Default::default() }
    }
}

/// THE POOL'S STATE (er_char.wgsl ER_MIMIC_POOL: mmpool.vs / mmpool.ps of the MAIN pack, kh-shaderc `--only mimicpool`): no
/// vertex buffer (six corners from the vertex index), its own depth written (GREATER_EQUAL), both faces, the five targets.
pub(super) unsafe fn pool_pipeline(device: &ID3D12Device, root: &ID3D12RootSignature, key: PsoKey, main_dir: &Path) -> Result<ID3D12PipelineState, String> {
    let read = |name: &str| std::fs::read(main_dir.join(name)).map_err(|e| format!("{name}: {e} (kh-shaderc --only mimicpool)"));
    let (vertex, pixel) = (read("mmpool.vs.dxil")?, read("mmpool.ps.dxil")?);
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
        RasterizerState: D3D12_RASTERIZER_DESC { FillMode: D3D12_FILL_MODE_SOLID, CullMode: D3D12_CULL_MODE_NONE, FrontCounterClockwise: key.front_ccw.into(), DepthClipEnable: true.into(), ..Default::default() },
        DepthStencilState: depth_state(true),
        PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
        NumRenderTargets: crate::hybrid::ROLES as u32,
        RTVFormats: formats(),
        DSVFormat: DXGI_FORMAT(key.depth),
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        ..Default::default()
    };
    crate::psocache::create(device, &desc).map_err(|e| format!("CreateGraphicsPipelineState(mimic pool): {e}"))
}

/// What the mimic's G-buffer draws need of the frame's G-buffer pass.
pub(super) struct GbCx<'a> {
    pub device: &'a ID3D12Device,
    pub list: &'a ID3D12GraphicsCommandList,
    /// The pass's render target views (five a frame slot, the first three made by the pass), this slot's first.
    pub first: D3D12_CPU_DESCRIPTOR_HANDLE,
    pub rtv_size: usize,
    pub ordered: &'a [crate::hybrid::GbTarget; crate::hybrid::ROLES],
    pub resources: &'a [ID3D12Resource],
    pub depth_view: D3D12_CPU_DESCRIPTOR_HANDLE,
    pub assets: &'a Assets,
    pub srv_start: D3D12_GPU_DESCRIPTOR_HANDLE,
    /// The mimics' screen box (scene pixels).
    pub scissor: [i32; 4],
}

/// THE MIMIC'S G-BUFFER DRAWS (after Sora's and the friends', inside his G-buffer pass): the game's subsurface and emissive
/// targets to render targets too (from the states the game left them in, and back), all five bound, every recorded draw with
/// its slot's mimic writer. Returns the draws made.
/// (`passes`: (a writer set, its records): the whole mimics' with their set, the forming ones with theirs; `pools`: the pool's state
/// and each pool's block in the upload buffer.)
pub(super) unsafe fn gbuffer(cx: &GbCx, passes: &[(&[Option<ID3D12PipelineState>], &[crate::hair::HairDraw])], pools: Option<(&ID3D12PipelineState, &[u64])>) -> u32 {
    let la = cx.list;
    let mut to_rt = Vec::new();
    let mut back = Vec::new();
    for k in crate::hybrid::WRITTEN..crate::hybrid::ROLES {
        let (Some(t), Some(resource)) = (cx.ordered.get(k), cx.resources.get(k)) else { return 0 };
        let typed = crate::hybrid::role_of(t.format).map_or(t.format, |(_, f)| f);
        let desc = D3D12_RENDER_TARGET_VIEW_DESC { Format: DXGI_FORMAT(typed), ViewDimension: D3D12_RTV_DIMENSION_TEXTURE2D, Anonymous: D3D12_RENDER_TARGET_VIEW_DESC_0 { Texture2D: D3D12_TEX2D_RTV { MipSlice: 0, PlaneSlice: 0 } } };
        cx.device.CreateRenderTargetView(resource, Some(&desc), D3D12_CPU_DESCRIPTOR_HANDLE { ptr: cx.first.ptr + k * cx.rtv_size });
        if t.state != D3D12_RESOURCE_STATE_RENDER_TARGET.0 {
            to_rt.push(transition(resource, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATES(t.state), D3D12_RESOURCE_STATE_RENDER_TARGET));
            back.push(transition(resource, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_STATES(t.state)));
        }
    }
    if !to_rt.is_empty() {
        la.ResourceBarrier(&to_rt);
    }
    la.OMSetRenderTargets(crate::hybrid::ROLES as u32, Some(&cx.first), true, Some(&cx.depth_view));
    let s = cx.scissor;
    la.RSSetScissorRects(&[RECT { left: s[0], top: s[1], right: s[2], bottom: s[3] }]);
    let mut n = 0;
    for (set, draws) in passes {
        let mut bound_model = usize::MAX;
        for (model_index, slot_k, count, first_index, base_vertex, skin_now, skin_then, material) in draws.iter() {
            let (Some(model), Some(Some(state))) = (cx.assets.models.get(*model_index), set.get(*slot_k)) else { continue };
            if bound_model != *model_index {
                la.IASetVertexBuffers(0, Some(&[model.vertex_view]));
                la.IASetIndexBuffer(Some(&model.index_view));
                bound_model = *model_index;
            }
            la.SetGraphicsRootConstantBufferView(2, *skin_now);
            la.SetGraphicsRootConstantBufferView(6, *skin_then);
            la.SetPipelineState(state);
            la.SetGraphicsRootConstantBufferView(1, *material);
            la.SetGraphicsRootDescriptorTable(3, D3D12_GPU_DESCRIPTOR_HANDLE { ptr: cx.srv_start.ptr + (cx.assets.copies.table_of_record(*slot_k, *material) * SLOT_TEXTURES * cx.assets.srv_size) as u64 }); // (MIMIC-BOSS hotfix 3, MB3-FORMLEAK) his own table
            la.DrawIndexedInstanced(*count, 1, *first_index, *base_vertex, 0);
            n += 1;
        }
    }
    // (THE USER 2026-11-03: his arrival and dissolve) the liquid-silver pools at the mimics' feet
    if let Some((state, blocks)) = pools {
        la.SetPipelineState(state);
        la.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        for block in blocks {
            la.SetGraphicsRootConstantBufferView(2, *block);
            la.DrawInstanced(6, 1, 0, 0);
            n += 1;
        }
    }
    if !back.is_empty() {
        la.ResourceBarrier(&back);
    }
    // (the pass's own three targets again, as the code after it expects them)
    la.OMSetRenderTargets(crate::hybrid::WRITTEN as u32, Some(&cx.first), true, Some(&cx.depth_view));
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mimic_the_writers_are_found_by_the_pack_names_and_take_five_targets() {
        assert_eq!(file_of(Path::new("er"), "c0ms", "", WHOLE), Path::new("er").join("c0ms_gb3_mm.ps.dxil"));
        assert_eq!(file_of(Path::new("er"), "c0ms", "_cg", FORM), Path::new("er").join("c0ms_cg_gb3_mf.ps.dxil"));
        let f = formats();
        assert_eq!(f.iter().filter(|x| **x != DXGI_FORMAT_UNKNOWN).count(), crate::hybrid::ROLES);
        assert_eq!((f[3], f[4]), (DXGI_FORMAT_R8G8_UNORM, DXGI_FORMAT_R16G16B16A16_FLOAT));
        let t = |format| crate::hybrid::GbTarget { resource: 1, format, width: 4, height: 4, state: 0 };
        let known = [t(crate::hybrid::fmt::R10G10B10A2_UNORM), t(crate::hybrid::fmt::R8G8B8A8_UNORM), t(crate::hybrid::fmt::R8G8B8A8_UNORM_SRGB), t(crate::hybrid::fmt::R8G8_UNORM), t(crate::hybrid::fmt::R16G16B16A16_FLOAT)];
        assert!(targets_known(&known));
        let mut other = known;
        other[4] = t(crate::hybrid::fmt::R8G8B8A8_UNORM);
        assert!(!targets_known(&other), "an emissive target of another format: the mimic is not drawn");
        // the main pack's set, when it is built: every opaque / masked variant has its writer
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/materials/er");
        if let Ok(lib) = kh_look::library::Library::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack")) {
            if dir.join("shaders_mimic.json").is_file() {
                for s in lib.slots.iter().filter(|s| !s.translucent) {
                    assert!(file_of(&dir, &s.variant(), "", WHOLE).is_file() && file_of(&dir, &s.variant(), "", FORM).is_file(), "{} has no mimic writer", s.variant());
                }
            }
        }
    }

    /// (MIMIC SORA, the user 2026-11-03: "mimic Sora also needs to use the same outfit that Sora is currently using") EVERY
    /// OUTFIT the renderer can wear (PACK/outfits/<id>) and every own set of it (own_shaders*.json `suffix`) has a mimic writer for
    /// each opaque / masked slot, under the names gfx.rs's worker reads with that outfit's folder and suffix. Skipped without the pack.
    #[test]
    fn mimic_every_outfit_and_own_set_has_its_writers() {
        let outfits = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/outfits");
        let Ok(entries) = std::fs::read_dir(&outfits) else { return };
        let mut checked = 0;
        for dir in entries.flatten().map(|e| e.path()).filter(|p| p.join("model.glb").is_file()) {
            let er = dir.join("materials").join("er");
            if !er.join("shaders_mimic.json").is_file() {
                continue;
            }
            let Ok(lib) = kh_look::library::Library::load(&dir) else { continue };
            let mut suffixes = vec![String::new()];
            for own in std::fs::read_dir(dir.join("materials")).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("own_shaders"))) {
                let json: serde_json::Value = serde_json::from_str(std::fs::read_to_string(&own).unwrap_or_default().trim_start_matches('\u{feff}')).unwrap_or_default();
                if let Some(s) = json["suffix"].as_str() {
                    suffixes.push(format!("_{s}"));
                }
            }
            for suffix in &suffixes {
                for s in lib.slots.iter().filter(|s| !s.translucent) {
                    assert!(file_of(&er, &s.variant(), suffix, WHOLE).is_file() && file_of(&er, &s.variant(), suffix, FORM).is_file(), "{}: {}{suffix} has no mimic writer", dir.display(), s.variant());
                    checked += 1;
                }
            }
        }
        println!("{checked} outfit slot writer(s) found");
    }
}
