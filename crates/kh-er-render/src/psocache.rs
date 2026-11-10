//! (KEYBLADE-SWAP) THE PIPELINE-STATE CACHE OF SORA'S SETS. A reload of his assets (an equip change, an outfit, a switched
//! setting) makes every set again for the new material slots (gfx.rs `pipelines`, `glow_pipelines`, the hair fringe's, the
//! 4x hair pass's, the Mimic Tear's writers). Most of those states are the same as the drawn ones: the same shaders, the same
//! description. [`create`] keys each description by its CONTENT (the shaders' bytes, the input layout, every fixed field,
//! the root signature and the device) and hands back the state made before for the same key, so a keyblade-only reload makes
//! only the new `kb:` slots' states and takes Sora's own over (shared: one object, held by both sets until the old set is
//! retired to the GPU fence).
//!
//! The cache only adds references: a state stays alive while any set or the cache holds it, never shorter than before. Each
//! reload is a GENERATION ([`next_generation`]); after its hand-over the states no set of the latest two generations asked
//! for are let go of by the cache ([`prune`]). [`clear`] lets go of all (the device is gone).

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Mutex;

use windows::core::Interface;
use windows::Win32::Graphics::Direct3D12::*;

/// The states by content key, with the generation that asked for each last.
type States = HashMap<u64, (ID3D12PipelineState, u64)>;

struct Cache {
    generation: u64,
    states: States,
    made: u64,
    taken: u64,
}

static CACHE: Mutex<Option<Cache>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut Cache) -> R) -> R {
    let mut g = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(|| Cache { generation: 1, states: HashMap::new(), made: 0, taken: 0 }))
}

/// The bytes of a shader of the description (empty: none).
///
/// # Safety
/// The bytecode's pointer and length describe readable memory (or the pointer is null).
unsafe fn bytes(code: &D3D12_SHADER_BYTECODE) -> &[u8] {
    if code.pShaderBytecode.is_null() || code.BytecodeLength == 0 {
        &[]
    } else {
        std::slice::from_raw_parts(code.pShaderBytecode as *const u8, code.BytecodeLength)
    }
}

/// The content key of a description; None for one the cache does not take (stream output, a cached blob).
///
/// # Safety
/// `desc` is a valid description: its shader and input-layout pointers point at what their counts say.
unsafe fn key(device: &ID3D12Device, desc: &D3D12_GRAPHICS_PIPELINE_STATE_DESC) -> Option<u64> {
    if desc.StreamOutput.NumEntries != 0 || desc.CachedPSO.CachedBlobSizeInBytes != 0 {
        return None;
    }
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (device.as_raw() as usize).hash(&mut h);
    desc.pRootSignature.as_ref().map_or(0, |r| r.as_raw() as usize).hash(&mut h);
    for code in [&desc.VS, &desc.PS, &desc.DS, &desc.HS, &desc.GS] {
        let b = bytes(code);
        b.len().hash(&mut h);
        b.hash(&mut h);
    }
    let layout = &desc.InputLayout;
    let elements: &[D3D12_INPUT_ELEMENT_DESC] = if layout.pInputElementDescs.is_null() || layout.NumElements == 0 { &[] } else { std::slice::from_raw_parts(layout.pInputElementDescs, layout.NumElements as usize) };
    elements.len().hash(&mut h);
    for e in elements {
        if !e.SemanticName.is_null() {
            e.SemanticName.as_bytes().hash(&mut h);
        }
        (e.SemanticIndex, e.Format.0, e.InputSlot, e.AlignedByteOffset, e.InputSlotClass.0, e.InstanceDataStepRate).hash(&mut h);
    }
    // every fixed field, by its (field-wise) debug text: no padding bytes are read
    format!("{:?}|{:?}|{:?}|{:?}|{:?}", desc.BlendState, desc.RasterizerState, desc.DepthStencilState, desc.RTVFormats, desc.SampleDesc).hash(&mut h);
    (desc.SampleMask, desc.IBStripCutValue.0, desc.PrimitiveTopologyType.0, desc.NumRenderTargets, desc.DSVFormat.0, desc.NodeMask, desc.Flags.0).hash(&mut h);
    Some(h.finish())
}

/// `device.CreateGraphicsPipelineState(desc)`, or the state made before for a description of the same content.
///
/// # Safety
/// As `ID3D12Device::CreateGraphicsPipelineState`.
pub(crate) unsafe fn create(device: &ID3D12Device, desc: &D3D12_GRAPHICS_PIPELINE_STATE_DESC) -> windows::core::Result<ID3D12PipelineState> {
    let Some(k) = key(device, desc) else { return device.CreateGraphicsPipelineState(desc) };
    if let Some(state) = with(|c| {
        let generation = c.generation;
        let found = c.states.get_mut(&k).map(|(s, g)| {
            *g = generation;
            s.clone()
        });
        if found.is_some() {
            c.taken += 1;
        }
        found
    }) {
        return Ok(state);
    }
    let state: ID3D12PipelineState = device.CreateGraphicsPipelineState(desc)?;
    with(|c| {
        c.made += 1;
        let generation = c.generation;
        c.states.insert(k, (state.clone(), generation));
    });
    Ok(state)
}

/// A reload begins: the states it asks for belong to a new generation. Returns that generation.
pub(crate) fn next_generation() -> u64 {
    with(|c| {
        c.generation += 1;
        c.generation
    })
}

/// What [`prune`] tells: states made and taken over since the last call, states still held, the states let go of (the caller
/// hands them to the worker's release: no release under a lock of the game's frame).
pub(crate) struct Pruned {
    pub(crate) made: u64,
    pub(crate) taken: u64,
    pub(crate) held: usize,
    pub(crate) let_go: Vec<ID3D12PipelineState>,
}

/// After a reload's hand-over: the cache lets go of the states not asked for since generation `keep_from - 1`.
pub(crate) fn prune(keep_from: u64) -> Pruned {
    with(|c| {
        let (keep, old): (States, States) = std::mem::take(&mut c.states).into_iter().partition(|(_, (_, g))| *g + 1 >= keep_from);
        c.states = keep;
        let out = Pruned { made: c.made, taken: c.taken, held: c.states.len(), let_go: old.into_values().map(|(s, _)| s).collect() };
        (c.made, c.taken) = (0, 0);
        out
    })
}

/// The device is gone (or our objects are being dropped): the cache lets go of every state.
pub(crate) fn clear() {
    with(|c| c.states.clear());
}

/// Our objects are left alive (the GPU did not finish our frames): the cache's states are left alive too, never freed under
/// work in flight.
pub(crate) fn forget_all() {
    let states = with(|c| std::mem::take(&mut c.states));
    std::mem::forget(states);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// the generations: a state asked for in the latest two generations stays, an older one is let go of
    #[test]
    fn prune_keeps_the_latest_two_generations() {
        let g = next_generation();
        let pruned = prune(g);
        // nothing of a device is needed to see the arithmetic: an empty cache stays empty
        assert_eq!(pruned.held, with(|c| c.states.len()));
        assert!(pruned.let_go.is_empty());
        assert!(next_generation() > g);
    }
}
