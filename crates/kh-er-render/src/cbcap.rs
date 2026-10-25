//! What the game binds at its draws: pipeline state, root signature, root constant buffers,
//! descriptor tables (more command-list vtable entries, as survey.rs).
//!
//! Two uses:
//! - DISCOVERY (`r.cbcap = N`, a change of N captures two frames, the second is written): every
//!   list's stream is replayed at `ExecuteCommandLists`; a text of the passes and their bindings
//!   goes to `<dump dir>/cbcap_<n>.txt`, and the constant buffers bound at full-screen draws,
//!   dispatches and on slots that keep one address through a pass are copied ON THE GPU (bound as a
//!   root constant buffer of a shader of ours that writes them into a float texture) and read
//!   back. No address of the game's memory is read on the CPU.
//! - WATCHES (always on, cheap): the last constant buffer address bound on a root slot at a draw
//!   that matches a watch (see `Watch`), for the renderer to bind in its own shaders.
//!
//! A hook costs one atomic load when nothing is asked for.

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::Mutex;

use windows::Win32::Graphics::Direct3D12::*;

use crate::hook::{entry, is_ours, patch, LIST_VTABLE};
use crate::DEAD;

#[derive(Clone, Copy, Debug)]
pub enum Ev {
    Pso(usize),
    GRoot(usize),
    CRoot(usize),
    GCbv(u32, u64),
    CCbv(u32, u64),
    GTable(u32, u64),
    CTable(u32, u64),
    Draw { indexed: bool, count: u32, instances: u32 },
    Dispatch(u32, u32, u32),
    /// The descriptor heaps set on the list (at most two: views, samplers).
    Heaps(usize, usize),
    /// A resource went to this state (targets are known by their RENDER_TARGET transitions).
    Barrier { resource: usize, subresource: u32, after: i32 },
}

/// 0 = off; n > 0 = frames still to record (the last one is written).
pub static CAPTURE: AtomicU32 = AtomicU32::new(0);
/// 1 = record the streams for the watches (the renderer wants them).
pub static WATCHING: AtomicU32 = AtomicU32::new(0);
static SERIAL: AtomicU32 = AtomicU32::new(0);
static SUBMISSION: AtomicU32 = AtomicU32::new(0);

/// The number of the submission being executed in this frame (1 = first).
pub fn next_submission() -> u32 {
    SUBMISSION.fetch_add(1, Ordering::SeqCst) + 1
}

static LISTS: Mutex<Option<HashMap<usize, Vec<Ev>>>> = Mutex::new(None);

static O_DRAW: AtomicUsize = AtomicUsize::new(0);
static O_DRAW_INDEXED: AtomicUsize = AtomicUsize::new(0);
static O_DISPATCH: AtomicUsize = AtomicUsize::new(0);
static O_PSO: AtomicUsize = AtomicUsize::new(0);
static O_GROOT: AtomicUsize = AtomicUsize::new(0);
static O_CROOT: AtomicUsize = AtomicUsize::new(0);
static O_GCBV: AtomicUsize = AtomicUsize::new(0);
static O_CCBV: AtomicUsize = AtomicUsize::new(0);
static O_GTABLE: AtomicUsize = AtomicUsize::new(0);
static O_CTABLE: AtomicUsize = AtomicUsize::new(0);
static O_HEAPS: AtomicUsize = AtomicUsize::new(0);

#[inline]
fn active() -> bool {
    CAPTURE.load(Ordering::Relaxed) != 0 && !DEAD.load(Ordering::Relaxed)
}

thread_local! {
    /// THE WATCH (always on once the renderer wants it, lock-free): the list this thread is
    /// recording, the heaps set on it and its pixel-stage constant buffer table (root parameter
    /// 29). A full-screen draw (3 vertices) hands them to survey.rs, which knows the pass.
    static CURRENT: std::cell::Cell<(usize, [usize; 2], u64)> = const { std::cell::Cell::new((0, [0; 2], 0)) };
}

/// (IMPL_ER17) 1 = the renderer wants the game's shadow-caster passes (HIS REAL SHADOW): the vertex-stage constant
/// buffer table (root parameter 1: b4..b13, cbSceneParam of the pass at b8), the viewport and the scissor rectangle of
/// the list this thread records are followed, and handed to survey.rs when a depth target leaves DEPTH_WRITE.
pub static CASTING: AtomicU32 = AtomicU32::new(0);

thread_local! {
    /// The list this thread is recording, its vertex-stage table, its viewport (x, y, width, height) and scissor.
    static CAST: std::cell::Cell<(usize, u64, [f32; 4], [i32; 4])> = const { std::cell::Cell::new((0, 0, [0.0; 4], [0; 4])) };
}

#[inline]
pub fn casting() -> bool {
    CASTING.load(Ordering::Relaxed) != 0 && WATCHING.load(Ordering::Relaxed) != 0 && !DEAD.load(Ordering::Relaxed)
}

#[inline]
fn cast(list: usize) -> (usize, u64, [f32; 4], [i32; 4]) {
    let now = CAST.with(|c| c.get());
    if now.0 == list {
        now
    } else {
        (list, 0, [0.0; 4], [0; 4])
    }
}

/// What the list binds for its casters right now: (the vertex-stage table with the heaps, viewport, scissor).
pub fn cast_now(list: usize) -> Option<(Table, [f32; 4], [i32; 4])> {
    let (c, heaps) = (cast(list), current(list).1);
    (c.1 != 0 && heaps[0] != 0 && c.2[2] >= 1.0 && c.2[3] >= 1.0).then_some((Table { heaps, handle: c.1 }, c.2, c.3))
}

type ViewportsFn = unsafe extern "system" fn(*mut c_void, u32, *const D3D12_VIEWPORT);
type ScissorsFn = unsafe extern "system" fn(*mut c_void, u32, *const windows::Win32::Foundation::RECT);
static O_VIEWPORTS: AtomicUsize = AtomicUsize::new(0);
static O_SCISSORS: AtomicUsize = AtomicUsize::new(0);

unsafe extern "system" fn viewports_hook(this: *mut c_void, count: u32, viewports: *const D3D12_VIEWPORT) {
    // (IMPL_ER80b) the light-shaft mask checks the composite's viewport
    if !is_ours() {
        crate::shaft::on_viewport(this as usize, (count > 0 && !viewports.is_null()).then(|| {
            let v = &*viewports;
            [v.TopLeftX, v.TopLeftY, v.Width, v.Height]
        }));
    }
    if count > 0 && !viewports.is_null() && casting() && !is_ours() {
        let v = &*viewports;
        let now = cast(this as usize);
        CAST.with(|c| c.set((now.0, now.1, [v.TopLeftX, v.TopLeftY, v.Width, v.Height], now.3)));
    }
    let original: ViewportsFn = std::mem::transmute(O_VIEWPORTS.load(Ordering::Relaxed));
    original(this, count, viewports)
}

unsafe extern "system" fn scissors_hook(this: *mut c_void, count: u32, rects: *const windows::Win32::Foundation::RECT) {
    if count > 0 && !rects.is_null() && casting() && !is_ours() {
        let r = &*rects;
        let now = cast(this as usize);
        CAST.with(|c| c.set((now.0, now.1, now.2, [r.left, r.top, r.right, r.bottom])));
    }
    let original: ScissorsFn = std::mem::transmute(O_SCISSORS.load(Ordering::Relaxed));
    original(this, count, rects)
}

#[inline]
fn watching() -> bool {
    WATCHING.load(Ordering::Relaxed) != 0 && !DEAD.load(Ordering::Relaxed)
}

#[inline]
fn current(list: usize) -> (usize, [usize; 2], u64) {
    let now = CURRENT.with(|c| c.get());
    if now.0 == list {
        now
    } else {
        (list, [0; 2], 0)
    }
}

fn record(list: usize, event: Ev) {
    if is_ours() {
        return;
    }
    if let Ok(mut guard) = LISTS.lock() {
        let events = guard.get_or_insert_with(HashMap::new).entry(list).or_default();
        if events.len() < 400_000 {
            events.push(event);
        }
    }
}

/// From survey.rs's hooks (they exist already): a list was reset / a transition was recorded.
pub fn on_reset(list: usize) {
    if active() {
        if let Ok(mut guard) = LISTS.lock() {
            if let Some(events) = guard.as_mut().and_then(|m| m.get_mut(&list)) {
                events.clear();
            }
        }
    }
}

pub fn on_barrier(list: usize, resource: usize, subresource: u32, after: i32) {
    if active() {
        record(list, Ev::Barrier { resource, subresource, after });
    }
}

type DrawFn = unsafe extern "system" fn(*mut c_void, u32, u32, u32, u32);
type DrawIndexedFn = unsafe extern "system" fn(*mut c_void, u32, u32, u32, i32, u32);
type DispatchFn = unsafe extern "system" fn(*mut c_void, u32, u32, u32);
type PointerFn = unsafe extern "system" fn(*mut c_void, *mut c_void);
type SlotFn = unsafe extern "system" fn(*mut c_void, u32, u64);

unsafe extern "system" fn draw_hook(this: *mut c_void, count: u32, instances: u32, a: u32, b: u32) {
    if count == 3 && instances == 1 && watching() && !is_ours() {
        let now = current(this as usize);
        if now.2 != 0 && now.1[0] != 0 {
            crate::survey::fullscreen(this as usize, Table { heaps: now.1, handle: now.2 });
        }
    }
    if active() {
        record(this as usize, Ev::Draw { indexed: false, count, instances });
    }
    let original: DrawFn = std::mem::transmute(O_DRAW.load(Ordering::Relaxed));
    // (IMPL_ER80b) the game's light-shaft composite: drawn masked off his pixels (shaft.rs), or as it is
    if count == 3 && instances == 1 && !is_ours() && crate::shaft::draw(this, count, instances, a, b, original) {
        return;
    }
    original(this, count, instances, a, b)
}

unsafe extern "system" fn draw_indexed_hook(this: *mut c_void, count: u32, instances: u32, a: u32, b: i32, c: u32) {
    if active() {
        record(this as usize, Ev::Draw { indexed: true, count, instances });
    }
    let original: DrawIndexedFn = std::mem::transmute(O_DRAW_INDEXED.load(Ordering::Relaxed));
    original(this, count, instances, a, b, c)
}

unsafe extern "system" fn dispatch_hook(this: *mut c_void, x: u32, y: u32, z: u32) {
    if active() {
        record(this as usize, Ev::Dispatch(x, y, z));
    }
    let original: DispatchFn = std::mem::transmute(O_DISPATCH.load(Ordering::Relaxed));
    original(this, x, y, z)
}

type HeapsFn = unsafe extern "system" fn(*mut c_void, u32, *const *mut c_void);

unsafe extern "system" fn heaps_hook(this: *mut c_void, count: u32, heaps: *const *mut c_void) {
    if watching() && !heaps.is_null() && !is_ours() {
        let at = |k: u32| if k < count { *heaps.add(k as usize) as usize } else { 0 };
        let now = current(this as usize);
        CURRENT.with(|c| c.set((now.0, [at(0), at(1)], now.2)));
    }
    if active() && !heaps.is_null() {
        let at = |k: u32| if k < count { *heaps.add(k as usize) as usize } else { 0 };
        record(this as usize, Ev::Heaps(at(0), at(1)));
    }
    let original: HeapsFn = std::mem::transmute(O_HEAPS.load(Ordering::Relaxed));
    original(this, count, heaps)
}

macro_rules! pointer_hook {
    ($name:ident, $original:ident, $event:path) => {
        unsafe extern "system" fn $name(this: *mut c_void, object: *mut c_void) {
            if active() {
                record(this as usize, $event(object as usize));
            }
            let original: PointerFn = std::mem::transmute($original.load(Ordering::Relaxed));
            original(this, object)
        }
    };
}
macro_rules! slot_hook {
    ($name:ident, $original:ident, $event:path) => {
        unsafe extern "system" fn $name(this: *mut c_void, slot: u32, value: u64) {
            if active() {
                record(this as usize, $event(slot, value));
            }
            let original: SlotFn = std::mem::transmute($original.load(Ordering::Relaxed));
            original(this, slot, value)
        }
    };
}
unsafe extern "system" fn pso_hook(this: *mut c_void, object: *mut c_void) {
    // (IMPL_ER80b) the light-shaft mask follows the pipeline state of each list
    if !is_ours() {
        crate::shaft::on_pso(this as usize, object as usize);
    }
    if active() {
        record(this as usize, Ev::Pso(object as usize));
    }
    let original: PointerFn = std::mem::transmute(O_PSO.load(Ordering::Relaxed));
    original(this, object)
}
pointer_hook!(groot_hook, O_GROOT, Ev::GRoot);
pointer_hook!(croot_hook, O_CROOT, Ev::CRoot);
slot_hook!(gcbv_hook, O_GCBV, Ev::GCbv);
slot_hook!(ccbv_hook, O_CCBV, Ev::CCbv);
unsafe extern "system" fn gtable_hook(this: *mut c_void, slot: u32, value: u64) {
    if slot as usize == PS_CBV_HIGH && watching() && !is_ours() {
        let now = current(this as usize);
        CURRENT.with(|c| c.set((now.0, now.1, value)));
    }
    if slot as usize == VS_CBV_HIGH && casting() && !is_ours() {
        let now = cast(this as usize);
        CAST.with(|c| c.set((now.0, value, now.2, now.3)));
    }
    if active() {
        record(this as usize, Ev::GTable(slot, value));
    }
    let original: SlotFn = std::mem::transmute(O_GTABLE.load(Ordering::Relaxed));
    original(this, slot, value)
}
slot_hook!(ctable_hook, O_CTABLE, Ev::CTable);

/// Hooks the entries (after hook::install; undone by hook::remove with the others).
pub fn install() -> Result<(), String> {
    let vtable = LIST_VTABLE.load(Ordering::SeqCst);
    if vtable == 0 {
        return Err("no command-list vtable".to_string());
    }
    // SAFETY: the hook functions have the entries' signatures (a descriptor handle is one u64).
    unsafe {
        patch(vtable, entry!(ID3D12GraphicsCommandList_Vtbl, DrawInstanced), draw_hook as *const () as usize, &O_DRAW, "DrawInstanced")?;
        patch(vtable, entry!(ID3D12GraphicsCommandList_Vtbl, DrawIndexedInstanced), draw_indexed_hook as *const () as usize, &O_DRAW_INDEXED, "DrawIndexedInstanced")?;
        patch(vtable, entry!(ID3D12GraphicsCommandList_Vtbl, Dispatch), dispatch_hook as *const () as usize, &O_DISPATCH, "Dispatch")?;
        patch(vtable, entry!(ID3D12GraphicsCommandList_Vtbl, SetPipelineState), pso_hook as *const () as usize, &O_PSO, "SetPipelineState")?;
        patch(vtable, entry!(ID3D12GraphicsCommandList_Vtbl, SetGraphicsRootSignature), groot_hook as *const () as usize, &O_GROOT, "SetGraphicsRootSignature")?;
        patch(vtable, entry!(ID3D12GraphicsCommandList_Vtbl, SetComputeRootSignature), croot_hook as *const () as usize, &O_CROOT, "SetComputeRootSignature")?;
        patch(vtable, entry!(ID3D12GraphicsCommandList_Vtbl, SetGraphicsRootConstantBufferView), gcbv_hook as *const () as usize, &O_GCBV, "SetGraphicsRootConstantBufferView")?;
        patch(vtable, entry!(ID3D12GraphicsCommandList_Vtbl, SetComputeRootConstantBufferView), ccbv_hook as *const () as usize, &O_CCBV, "SetComputeRootConstantBufferView")?;
        patch(vtable, entry!(ID3D12GraphicsCommandList_Vtbl, SetGraphicsRootDescriptorTable), gtable_hook as *const () as usize, &O_GTABLE, "SetGraphicsRootDescriptorTable")?;
        patch(vtable, entry!(ID3D12GraphicsCommandList_Vtbl, SetComputeRootDescriptorTable), ctable_hook as *const () as usize, &O_CTABLE, "SetComputeRootDescriptorTable")?;
        patch(vtable, entry!(ID3D12GraphicsCommandList_Vtbl, SetDescriptorHeaps), heaps_hook as *const () as usize, &O_HEAPS, "SetDescriptorHeaps")?;
        patch(vtable, entry!(ID3D12GraphicsCommandList_Vtbl, RSSetViewports), viewports_hook as *const () as usize, &O_VIEWPORTS, "RSSetViewports")?;
        patch(vtable, entry!(ID3D12GraphicsCommandList_Vtbl, RSSetScissorRects), scissors_hook as *const () as usize, &O_SCISSORS, "RSSetScissorRects")?;
    }
    Ok(())
}

pub fn release() {
    if let Ok(mut guard) = LISTS.lock() {
        *guard = None;
    }
}

/// Asks for a capture (two frames; the second is written).
pub fn request() {
    CAPTURE.store(2, Ordering::SeqCst);
}

/// At Present: a frame ends.
pub fn end_frame() {
    SUBMISSION.store(0, Ordering::SeqCst);
    let left = CAPTURE.load(Ordering::Relaxed);
    if left > 0 {
        CAPTURE.store(left - 1, Ordering::SeqCst);
        if left == 1 {
            SERIAL.fetch_add(1, Ordering::SeqCst);
        }
    }
}

/// True while the frame being recorded is the one to write.
pub fn writing() -> bool {
    CAPTURE.load(Ordering::Relaxed) == 1
}

pub fn serial() -> u32 {
    SERIAL.load(Ordering::Relaxed)
}

/// The recorded stream of a list (as of its last Reset), for a submission being executed.
pub fn stream(list: usize) -> Vec<Ev> {
    LISTS.lock().ok().and_then(|g| g.as_ref().and_then(|m| m.get(&list).cloned())).unwrap_or_default()
}

/// Root parameters followed (the game's graphics root signature has 36).
pub const SLOTS: usize = 40;
/// The game's ONE graphics root signature (read from the RTS0 chunk of its shaders, 2.7.1.0):
/// per stage seven tables (CBV b0-3, CBV b4-13, SRV t0-3, SRV t4-19, SRV t20-65, samplers s0-3,
/// samplers s4-15), stages in the order VS 0.., HS 7.., DS 14.., GS 21.., PS 28..; 35 = UAVs.
pub const PS_CBV_LOW: usize = 28;
pub const PS_CBV_HIGH: usize = 29;
pub const PS_SRV_HIGH: usize = 32;
pub const VS_CBV_HIGH: usize = 1;

/// The binding state while a stream is replayed.
#[derive(Clone)]
pub struct Bound {
    pub pso: usize,
    pub groot: usize,
    pub croot: usize,
    pub gcbv: [u64; SLOTS],
    pub ccbv: [u64; SLOTS],
    pub gtable: [u64; SLOTS],
    pub ctable: [u64; SLOTS],
    pub heaps: [usize; 2],
    /// Resources in the RENDER_TARGET state, in the order they went there.
    pub targets: Vec<usize>,
}

impl Default for Bound {
    fn default() -> Bound {
        Bound { pso: 0, groot: 0, croot: 0, gcbv: [0; SLOTS], ccbv: [0; SLOTS], gtable: [0; SLOTS], ctable: [0; SLOTS], heaps: [0; 2], targets: Vec::new() }
    }
}

impl Bound {
    /// Applies a binding event; returns true for a draw / dispatch (the caller looks at the state then).
    pub fn apply(&mut self, ev: &Ev) -> bool {
        match *ev {
            Ev::Pso(p) => self.pso = p,
            Ev::GRoot(r) => {
                self.groot = r;
                self.gcbv = [0; SLOTS];
                self.gtable = [0; SLOTS];
            }
            Ev::CRoot(r) => {
                self.croot = r;
                self.ccbv = [0; SLOTS];
                self.ctable = [0; SLOTS];
            }
            Ev::Heaps(a, b) => self.heaps = [a, b],
            Ev::GCbv(s, v) => {
                if let Some(x) = self.gcbv.get_mut(s as usize) {
                    *x = v;
                }
            }
            Ev::CCbv(s, v) => {
                if let Some(x) = self.ccbv.get_mut(s as usize) {
                    *x = v;
                }
            }
            Ev::GTable(s, v) => {
                if let Some(x) = self.gtable.get_mut(s as usize) {
                    *x = v;
                }
            }
            Ev::CTable(s, v) => {
                if let Some(x) = self.ctable.get_mut(s as usize) {
                    *x = v;
                }
            }
            Ev::Barrier { resource, subresource, after } => {
                if subresource == 0 || subresource == D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES {
                    if after & D3D12_RESOURCE_STATE_RENDER_TARGET.0 != 0 {
                        if !self.targets.contains(&resource) {
                            self.targets.push(resource);
                        }
                    } else {
                        self.targets.retain(|t| *t != resource);
                    }
                }
            }
            Ev::Draw { .. } | Ev::Dispatch(..) => return true,
        }
        false
    }
}

/// A descriptor table of the game to copy constant buffers from: the heaps set at the draw and
/// the table's handle (root parameter `PS_CBV_HIGH`: registers b4..b13 of the pixel stage).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Table {
    pub heaps: [usize; 2],
    pub handle: u64,
}

/// The text of one submission for the capture file, and the tables to copy constant buffers from.
pub fn describe(submission: u32, lists: &[usize], resource: &dyn Fn(usize) -> String) -> (Vec<String>, Vec<Table>) {
    let mut lines = vec![format!("==== submission {submission}: {} list(s)", lists.len())];
    let mut wanted: Vec<Table> = Vec::new();
    let want = |b: &Bound, wanted: &mut Vec<Table>| {
        let table = Table { heaps: b.heaps, handle: b.gtable[PS_CBV_HIGH] };
        if table.handle != 0 && table.heaps[0] != 0 && !wanted.contains(&table) && wanted.len() < 100 {
            wanted.push(table);
        }
    };
    let slots = |values: &[u64; SLOTS]| values.iter().enumerate().filter(|(_, v)| **v != 0).map(|(k, v)| format!("{k}:{v:#x}")).collect::<Vec<_>>().join(" ");
    for &list in lists {
        let events = stream(list);
        lines.push(format!("  list {list:#x}: {} events", events.len()));
        let mut b = Bound::default();
        let mut pass: Vec<usize> = vec![usize::MAX];
        // Per pass: big draws, and per root slot the distinct addresses seen at them.
        let mut big = 0u32;
        let mut per_slot: Vec<Vec<u64>> = vec![Vec::new(); SLOTS];
        let mut depth_state: HashMap<usize, i32> = HashMap::new();
        let flush = |lines: &mut Vec<String>, big: &mut u32, per_slot: &mut Vec<Vec<u64>>| {
            if *big > 0 {
                let mut text = format!("      {} larger draws; tables per root parameter:", *big);
                for (k, values) in per_slot.iter().enumerate() {
                    if values.is_empty() {
                        continue;
                    }
                    if values.len() <= 3 {
                        text.push_str(&format!(" {k}:[{}]", values.iter().map(|v| format!("{v:#x}")).collect::<Vec<_>>().join(",")));
                    } else {
                        text.push_str(&format!(" {k}:({} distinct)", values.len()));
                    }
                }
                lines.push(text);
            }
            *big = 0;
            per_slot.iter_mut().for_each(Vec::clear);
        };
        for ev in &events {
            if let Ev::Barrier { resource, after, subresource } = ev {
                if *subresource == 1 {
                    // (the stencil plane of a depth target)
                } else if after & (D3D12_RESOURCE_STATE_DEPTH_WRITE.0 | D3D12_RESOURCE_STATE_DEPTH_READ.0) != 0 {
                    depth_state.insert(*resource, *after);
                } else {
                    depth_state.remove(resource);
                }
            }
            if !b.apply(ev) {
                continue;
            }
            if b.targets != pass {
                flush(&mut lines, &mut big, &mut per_slot);
                pass = b.targets.clone();
                lines.push(format!(
                    "    pass: targets [{}] depth [{}]",
                    pass.iter().map(|t| resource(*t)).collect::<Vec<_>>().join(" | "),
                    depth_state.iter().map(|(r, s)| format!("{} {}", resource(*r), crate::survey::state_text(*s))).collect::<Vec<_>>().join(" | ")
                ));
            }
            match *ev {
                Ev::Draw { indexed, count, instances } => {
                    if count <= 6 {
                        lines.push(format!("      draw {}{count} x{instances} pso {:#x} root {:#x} heaps {:#x} {:#x} tables {{{}}}", if indexed { "indexed " } else { "" }, b.pso, b.groot, b.heaps[0], b.heaps[1], slots(&b.gtable)));
                        want(&b, &mut wanted);
                    } else {
                        big += 1;
                        if big == 1 {
                            lines.push(format!("      first larger draw: {}{count} x{instances} pso {:#x} tables {{{}}}", if indexed { "indexed " } else { "" }, b.pso, slots(&b.gtable)));
                            want(&b, &mut wanted);
                        }
                        for (k, v) in b.gtable.iter().enumerate() {
                            if *v != 0 && !per_slot[k].contains(v) && per_slot[k].len() < 8 {
                                per_slot[k].push(*v);
                            }
                        }
                    }
                }
                Ev::Dispatch(x, y, z) => {
                    lines.push(format!("      dispatch {x}x{y}x{z} pso {:#x} root {:#x} tables {{{}}}", b.pso, b.croot, slots(&b.ctable)));
                }
                _ => {}
            }
        }
        flush(&mut lines, &mut big, &mut per_slot);
    }
    (lines, wanted)
}

/// The rows of a copied constant buffer as text: four floats and the raw words per row; rows of zeros are skipped.
pub fn rows_text(title: &str, words: &[u32]) -> Vec<String> {
    let mut lines = vec![format!("---- constant buffer {title}")];
    let mut zeros = 0u32;
    for (k, row) in words.chunks_exact(4).enumerate() {
        if row.iter().all(|w| *w == 0) {
            zeros += 1;
            continue;
        }
        let f: Vec<String> = row.iter().map(|w| format!("{:>14.6}", f32::from_bits(*w))).collect();
        let x: Vec<String> = row.iter().map(|w| format!("{w:08x}")).collect();
        lines.push(format!("  [{k:3}] {} | {}", f.join(" "), x.join(" ")));
    }
    lines.push(format!("  ({zeros} zero rows of {})", words.len() / 4));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bound_state_follows_a_stream() {
        let mut b = Bound::default();
        let stream = [Ev::GRoot(7), Ev::GCbv(2, 0x1000), Ev::Barrier { resource: 5, subresource: 0xFFFF_FFFF, after: 4 }, Ev::Pso(9), Ev::Draw { indexed: false, count: 3, instances: 1 }];
        let draws: Vec<bool> = stream.iter().map(|e| b.apply(e)).collect();
        assert_eq!(draws, [false, false, false, false, true]);
        assert_eq!((b.pso, b.groot, b.gcbv[2], b.targets.clone()), (9, 7, 0x1000, vec![5]));
        // A new root signature forgets the slots; a target that leaves RENDER_TARGET is no target.
        b.apply(&Ev::GRoot(8));
        b.apply(&Ev::Barrier { resource: 5, subresource: 0xFFFF_FFFF, after: 0x80 });
        assert!(b.gcbv[2] == 0 && b.targets.is_empty());
        let text = rows_text("x", &[0x3f800000, 0, 0, 1, 0, 0, 0, 0]);
        assert!(text[1].contains("1.000000") && text[1].contains("00000001") && text[2].contains("1 zero rows of 2"));
    }
}
