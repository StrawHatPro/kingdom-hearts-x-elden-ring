//! The hooks, all our own (no hudhook, no hook library), all undone at shutdown.
//!
//! Everything we need is reached through virtual calls on three classes whose objects all share
//! one vtable each (the DXGI swap chain, the D3D12 command queue, the D3D12 graphics command
//! list). The vtables are found from throw-away objects of our own (`discover`), so no address of
//! the game is involved. Two kinds of hook:
//! - INLINE (detour.rs) for `Present`, `Present1` and `ExecuteCommandLists`: the functions the
//!   vtables name get a jump at their start. A vtable patch on `Present` does NOT work beside the
//!   Steam overlay (crash 2 of IMPL_ER2_STATE.md: endless recursion between their detour and ours).
//! - VTABLE ENTRIES for the command-list functions of the survey (survey.rs): hot, never hooked by
//!   an overlay, and undone by writing the old pointer back.
//!
//! Hooked: `IDXGISwapChain::Present`, `IDXGISwapChain1::Present1` (where we draw),
//! `ID3D12CommandQueue::ExecuteCommandLists` (to learn the game's queue and, for the survey, the
//! order of its submissions) and, in survey.rs, four entries of the command list.

use std::cell::Cell;
use std::ffi::c_void;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;

use windows::core::{w, Interface, HRESULT};
use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::System::Memory::{VirtualProtect, VirtualQuery, MEMORY_BASIC_INFORMATION, MEM_COMMIT, PAGE_GUARD, PAGE_NOACCESS, PAGE_PROTECTION_FLAGS, PAGE_READWRITE};
use windows::Win32::UI::WindowsAndMessaging::{CreateWindowExW, DestroyWindow, WINDOW_EX_STYLE, WS_OVERLAPPED};

use crate::{rlog, DEAD};

struct Patch {
    slot: usize,
    original: usize,
    ours: usize,
    what: &'static str,
}

static PATCHES: Mutex<Vec<Patch>> = Mutex::new(Vec::new());

static ORIGINAL_PRESENT: AtomicUsize = AtomicUsize::new(0);
static ORIGINAL_PRESENT1: AtomicUsize = AtomicUsize::new(0);
static ORIGINAL_EXECUTE: AtomicUsize = AtomicUsize::new(0);

/// The vtable of every graphics command list (found by `discover`), for survey.rs.
pub static LIST_VTABLE: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    /// Set while this thread is inside our own drawing: our own calls of hooked functions pass through.
    static OURS: Cell<u32> = const { Cell::new(0) };
}

/// Runs `f` with this thread marked as ours (its hooked calls are not looked at).
pub fn as_ours<T>(f: impl FnOnce() -> T) -> T {
    OURS.with(|c| c.set(c.get() + 1));
    let out = f();
    OURS.with(|c| c.set(c.get() - 1));
    out
}

pub fn is_ours() -> bool {
    OURS.with(|c| c.get() > 0)
}

/// The command queues seen executing: (pointer, lists executed). Pointers are not owned; the
/// game's queues live as long as its device.
static QUEUES: Mutex<Vec<(usize, u64)>> = Mutex::new(Vec::new());

/// Replaces entry `index` of a vtable; the entry that was there is stored in `original` BEFORE
/// ours goes in (other threads call through the entry all the time: our function must never find
/// its original missing).
///
/// # Safety
/// `vtable` must be a COM vtable with more than `index` entries and `ours` a function of the
/// entry's exact signature that stays loaded for the life of the process.
pub unsafe fn patch(vtable: usize, index: usize, ours: usize, original: &AtomicUsize, what: &'static str) -> Result<(), String> {
    let slot = (vtable + index * 8) as *mut usize;
    let mut old = PAGE_PROTECTION_FLAGS(0);
    VirtualProtect(slot as *const c_void, 8, PAGE_READWRITE, &mut old).map_err(|e| format!("VirtualProtect for {what}: {e}"))?;
    let entry = &*(slot as *const AtomicUsize);
    let mut was = entry.load(Ordering::SeqCst);
    loop {
        original.store(was, Ordering::SeqCst);
        match entry.compare_exchange(was, ours, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => break,
            Err(now) => was = now,
        }
    }
    let _ = VirtualProtect(slot as *const c_void, 8, old, &mut old);
    PATCHES.lock().unwrap().push(Patch { slot: slot as usize, original: was, ours, what });
    Ok(())
}

/// Puts every patched entry back (an entry somebody else replaced after us is left alone).
pub fn remove() {
    for what in crate::detour::remove() {
        rlog!("render: {what} was hooked again by someone else after us: our jump stays (it passes straight through now)");
    }
    let mut patches = PATCHES.lock().unwrap();
    for p in patches.drain(..).rev() {
        // SAFETY: the slot was writable through VirtualProtect before; the module it lies in
        // (dxgi / d3d12core) is never unloaded while the game runs.
        unsafe {
            let slot = p.slot as *mut usize;
            let mut old = PAGE_PROTECTION_FLAGS(0);
            if VirtualProtect(slot as *const c_void, 8, PAGE_READWRITE, &mut old).is_ok() {
                if (*(slot as *const AtomicUsize)).compare_exchange(p.ours, p.original, Ordering::SeqCst, Ordering::SeqCst).is_err() {
                    rlog!("render: {} was re-hooked by someone else after us: left as it is", p.what);
                }
                let _ = VirtualProtect(slot as *const c_void, 8, old, &mut old);
            }
        }
    }
}

/// Index of a vtable entry from the offset of its field in the bindings' vtable struct.
macro_rules! entry {
    ($vtbl:ty, $field:ident) => {
        std::mem::offset_of!($vtbl, $field) / 8
    };
}
pub(crate) use entry;

/// The vtables of the three classes, read from objects of our own that are released again.
unsafe fn discover() -> Result<(usize, usize, usize), String> {
    let text = |what: &str, e: windows::core::Error| format!("{what}: {e}");
    let factory: IDXGIFactory2 = CreateDXGIFactory1().map_err(|e| text("CreateDXGIFactory1", e))?;
    let mut device: Option<ID3D12Device> = None;
    D3D12CreateDevice(None, D3D_FEATURE_LEVEL_11_0, &mut device).map_err(|e| text("D3D12CreateDevice", e))?;
    let device = device.ok_or("D3D12CreateDevice gave no device")?;
    let queue: ID3D12CommandQueue = device.CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC { Type: D3D12_COMMAND_LIST_TYPE_DIRECT, ..Default::default() }).map_err(|e| text("CreateCommandQueue", e))?;
    let allocator: ID3D12CommandAllocator = device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT).map_err(|e| text("CreateCommandAllocator", e))?;
    let list: ID3D12GraphicsCommandList = device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator, None).map_err(|e| text("CreateCommandList", e))?;
    let _ = list.Close();
    let window = CreateWindowExW(WINDOW_EX_STYLE(0), w!("STATIC"), w!("kh-er vtable probe"), WS_OVERLAPPED, 0, 0, 64, 64, None, None, None, None).map_err(|e| text("CreateWindowExW", e))?;
    let desc = DXGI_SWAP_CHAIN_DESC1 {
        Width: 64,
        Height: 64,
        Format: DXGI_FORMAT_R8G8B8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: 2,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
        ..Default::default()
    };
    let swap = factory.CreateSwapChainForHwnd(&queue, window, &desc, None, None);
    let result = match &swap {
        Ok(swap) => {
            let vtable_of = |raw: *mut c_void| *(raw as *const usize);
            Ok((vtable_of(swap.as_raw()), vtable_of(queue.as_raw()), vtable_of(list.as_raw())))
        }
        Err(e) => Err(format!("CreateSwapChainForHwnd: {e}")),
    };
    drop(swap);
    let _ = DestroyWindow(window);
    result
}

/// Finds the vtables and installs the three hooks. Called once, from the worker thread.
pub fn install() -> Result<(), String> {
    // SAFETY: plain object creation; the hook functions below have the entries' signatures.
    unsafe {
        let (swap, queue, list) = discover()?;
        LIST_VTABLE.store(list, Ordering::SeqCst);
        // IDXGISwapChain: IUnknown 0..2, IDXGIObject 3..6, IDXGIDeviceSubObject 7, Present 8.
        let present_index = entry!(IDXGISwapChain_Vtbl, Present);
        let present1_index = entry!(IDXGISwapChain1_Vtbl, Present1);
        let execute_index = entry!(ID3D12CommandQueue_Vtbl, ExecuteCommandLists);
        if present_index != 8 || present1_index != 22 || execute_index != 10 {
            return Err(format!("the bindings' vtable layout is not the expected one (Present {present_index}, Present1 {present1_index}, ExecuteCommandLists {execute_index})"));
        }
        // These three are hooked INLINE at the functions the vtables name (detour.rs says why:
        // the Steam overlay re-hooks a patched Present entry into an endless recursion).
        let function = |vtable: usize, index: usize| *((vtable + index * 8) as *const usize);
        let bytes = |at: usize| if readable(at, 8) { std::slice::from_raw_parts(at as *const u8, 8).iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ") } else { "?".to_string() };
        let (execute, present1, present) = (function(queue, execute_index), function(swap, present1_index), function(swap, present_index));
        rlog!("render: Present at {present:#x} starts {}; Present1 at {present1:#x} starts {}; ExecuteCommandLists at {execute:#x} starts {}", bytes(present), bytes(present1), bytes(execute));
        crate::detour::install(execute, execute_hook as *const () as usize, &ORIGINAL_EXECUTE, "ID3D12CommandQueue::ExecuteCommandLists")?;
        crate::detour::install(present1, present1_hook as *const () as usize, &ORIGINAL_PRESENT1, "IDXGISwapChain1::Present1")?;
        crate::detour::install(present, present_hook as *const () as usize, &ORIGINAL_PRESENT, "IDXGISwapChain::Present")?;
        rlog!("render: hooks installed (inline: Present, Present1, ExecuteCommandLists; vtables: swap chain {swap:#x}, command queue {queue:#x}, command list {list:#x})");
    }
    Ok(())
}

type PresentFn = unsafe extern "system" fn(*mut c_void, u32, u32) -> HRESULT;
type Present1Fn = unsafe extern "system" fn(*mut c_void, u32, u32, *const DXGI_PRESENT_PARAMETERS) -> HRESULT;
type ExecuteFn = unsafe extern "system" fn(*mut c_void, u32, *const *mut c_void);

thread_local! {
    static IN_PRESENT: Cell<bool> = const { Cell::new(false) };
}

/// Our work in front of a Present of the game (never unwinds).
/// (TITLEPERF) Our own time in the hooks since the last [`take_perf`]: Present's work (ns, calls), the submission
/// survey before the game's ExecuteCommandLists (ns, calls).
static PERF: [AtomicU64; 4] = [AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0)];

/// (TITLEPERF) ((Present ns, Present calls), (survey ns, submissions)) since the last call; the counters start again.
pub fn take_perf() -> ((u64, u64), (u64, u64)) {
    let t = |k: usize| PERF[k].swap(0, Ordering::Relaxed);
    ((t(0), t(1)), (t(2), t(3)))
}

/// (TITLEPERF) The game's adapter's local video memory: (in use, budget) in bytes, read once a second at Present.
static VRAM: [AtomicU64; 3] = [AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0)];

pub fn video_memory() -> Option<(u64, u64)> {
    let budget = VRAM[1].load(Ordering::Relaxed);
    (budget > 0).then(|| (VRAM[0].load(Ordering::Relaxed), budget))
}

/// Reads the adapter's memory figures (at most once a second; any failure leaves the last reading).
fn read_video_memory(swap: *mut c_void) {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    let now = START.get_or_init(std::time::Instant::now).elapsed().as_millis() as u64 + 1000;
    if now.saturating_sub(VRAM[2].load(Ordering::Relaxed)) < 1000 {
        return;
    }
    VRAM[2].store(now, Ordering::Relaxed);
    // SAFETY: the swap chain the game is presenting (borrowed, not released); COM calls on objects we own.
    let read = || -> windows::core::Result<DXGI_QUERY_VIDEO_MEMORY_INFO> {
        let swap = unsafe { IDXGISwapChain::from_raw_borrowed(&swap) }.ok_or_else(windows::core::Error::empty)?;
        let device: ID3D12Device = unsafe { swap.GetDevice() }?;
        let factory: IDXGIFactory4 = unsafe { CreateDXGIFactory1() }?;
        let adapter: IDXGIAdapter3 = unsafe { factory.EnumAdapterByLuid(device.GetAdapterLuid()) }?;
        let mut info = DXGI_QUERY_VIDEO_MEMORY_INFO::default();
        unsafe { adapter.QueryVideoMemoryInfo(0, DXGI_MEMORY_SEGMENT_GROUP_LOCAL, &mut info) }?;
        Ok(info)
    };
    if let Ok(info) = read() {
        VRAM[0].store(info.CurrentUsage, Ordering::Relaxed);
        VRAM[1].store(info.Budget, Ordering::Relaxed);
    }
}

fn before_present(swap: *mut c_void, flags: u32) {
    if DEAD.load(Ordering::Relaxed) || flags & DXGI_PRESENT_TEST.0 != 0 || is_ours() || IN_PRESENT.with(|c| c.get()) {
        return;
    }
    IN_PRESENT.with(|c| c.set(true));
    let started = std::time::Instant::now();
    let result = std::panic::catch_unwind(|| as_ours(|| crate::gfx::present(swap)));
    PERF[0].fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
    PERF[1].fetch_add(1, Ordering::Relaxed);
    let _ = std::panic::catch_unwind(|| as_ours(|| read_video_memory(swap)));
    IN_PRESENT.with(|c| c.set(false));
    if result.is_err() {
        DEAD.store(true, Ordering::SeqCst);
        rlog!("render: PANIC while drawing; the renderer is inert from now on");
    }
}

unsafe extern "system" fn present_hook(this: *mut c_void, sync: u32, flags: u32) -> HRESULT {
    before_present(this, flags);
    let original: PresentFn = std::mem::transmute(ORIGINAL_PRESENT.load(Ordering::Relaxed));
    // (Present may call Present1 inside, or the reverse: that inner call is not a new frame.)
    let outer = !IN_PRESENT.with(|c| c.replace(true));
    let result = original(this, sync, flags);
    if outer {
        IN_PRESENT.with(|c| c.set(false));
    }
    result
}

unsafe extern "system" fn present1_hook(this: *mut c_void, sync: u32, flags: u32, parameters: *const DXGI_PRESENT_PARAMETERS) -> HRESULT {
    before_present(this, flags);
    let original: Present1Fn = std::mem::transmute(ORIGINAL_PRESENT1.load(Ordering::Relaxed));
    let outer = !IN_PRESENT.with(|c| c.replace(true));
    let result = original(this, sync, flags, parameters);
    if outer {
        IN_PRESENT.with(|c| c.set(false));
    }
    result
}

unsafe extern "system" fn execute_hook(this: *mut c_void, count: u32, lists: *const *mut c_void) {
    let mut submission = crate::survey::Submission::default();
    let started = std::time::Instant::now();
    if !DEAD.load(Ordering::Relaxed) && !is_ours() {
        submission = std::panic::catch_unwind(|| {
            if let Ok(mut queues) = QUEUES.lock() {
                match queues.iter().position(|(q, _)| *q == this as usize) {
                    Some(k) => queues[k].1 += count as u64,
                    None if queues.len() < 16 => queues.push((this as usize, count as u64)),
                    None => {}
                }
            }
            crate::survey::on_execute(this as usize, count, lists)
        })
        .unwrap_or_default();
    }
    // (discovery) A capture frame: the text of this submission and the constant buffers to copy.
    let capture = if crate::cbcap::writing() && !DEAD.load(Ordering::Relaxed) && !is_ours() && !lists.is_null() {
        std::panic::catch_unwind(|| {
            let all: Vec<usize> = (0..count as usize).map(|k| *lists.add(k) as usize).collect();
            crate::cbcap::describe(crate::cbcap::next_submission(), &all, &crate::survey::describe_resource)
        })
        .ok()
    } else {
        None
    };
    PERF[2].fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
    PERF[3].fetch_add(1, Ordering::Relaxed);
    let original: ExecuteFn = std::mem::transmute(ORIGINAL_EXECUTE.load(Ordering::Relaxed));
    // The game's velocity pass is in this submission: its lists up to that pass, then our own
    // velocity draw (his motion vectors, for the game's temporal anti-aliasing), then the rest.
    let split = submission.split as u32;
    // (IMPL_ER17) The sun's cascade atlas is filled in this submission: the lists up to its last caster pass, then HIS REAL
    // SHADOW (his skinned mesh, depth only, into the same cascades), then the rest (the G-buffer, the shadow mask).
    let cast = submission.sun.map_or(0, |s| s.split as u32);
    let cast = if cast > 0 && cast <= count && submission.velocity.is_none() && !DEAD.load(Ordering::Relaxed) && !is_ours() && crate::gfx::cast_wanted() { cast } else { 0 };
    // (IMPL_ER53) THE EARLY DEPTH: in the deferred stage, after its last G-buffer list (and after his real shadow when
    // that comes first), his depth goes into the scene depth before the rest of the stage makes its depth copies.
    let early = submission.early_split as u32;
    let early = if submission.scene.is_some() && early > cast && early < count && submission.velocity.is_none() && !DEAD.load(Ordering::Relaxed) && !is_ours() && crate::gfx::early_wanted() { early } else { 0 };
    if cast > 0 || early > 0 {
        let mut done = 0u32;
        if cast > 0 {
            original(this, cast, lists);
            done = cast;
            if std::panic::catch_unwind(|| as_ours(|| crate::gfx::cast_pass(this as usize, submission))).is_err() {
                DEAD.store(true, Ordering::SeqCst);
                rlog!("render: PANIC while drawing; the renderer is inert from now on");
            }
        }
        if early > 0 {
            original(this, early - done, lists.add(done as usize));
            done = early;
            if !DEAD.load(Ordering::Relaxed) && std::panic::catch_unwind(|| as_ours(|| crate::gfx::early_pass(this as usize, submission))).is_err() {
                DEAD.store(true, Ordering::SeqCst);
                rlog!("render: PANIC while drawing; the renderer is inert from now on");
            }
        }
        if count > done {
            original(this, count - done, lists.add(done as usize));
        }
    } else if submission.velocity.is_some() && split > 0 && split <= count && !DEAD.load(Ordering::Relaxed) && (crate::gfx::velocity_wanted() || crate::gfx::recomp_split_wanted()) {
        // (RSF2 round 14) the KH characters' depth into the game's half-size effect depth, right after the game rebuilt it
        let fx = submission.effects_split as u32;
        if fx > 0 && fx < split && submission.effects_half.is_some() && crate::gfx::effects_depth_wanted() {
            original(this, fx, lists);
            if std::panic::catch_unwind(|| as_ours(|| crate::gfx::effects_depth_pass(this as usize, submission))).is_err() {
                DEAD.store(true, Ordering::SeqCst);
                rlog!("render: PANIC while drawing; the renderer is inert from now on");
            }
            original(this, split - fx, lists.add(fx as usize));
        } else {
            original(this, split, lists);
        }
        if std::panic::catch_unwind(|| as_ours(|| crate::gfx::velocity_pass(this as usize, submission))).is_err() {
            DEAD.store(true, Ordering::SeqCst);
            rlog!("render: PANIC while drawing; the renderer is inert from now on");
        }
        if count > split {
            original(this, count - split, lists.add(split as usize));
        }
    } else {
        original(this, count, lists);
    }
    // The game's forward stage (the submission with its velocity pass) is complete: Sora's late
    // effects and the scene darkening go here, before the game's temporal anti-aliasing and tone map.
    if submission.velocity.is_some() && !DEAD.load(Ordering::Relaxed) && !is_ours() {
        if std::panic::catch_unwind(|| as_ours(|| crate::gfx::late_pass(this as usize, submission))).is_err() {
            DEAD.store(true, Ordering::SeqCst);
            rlog!("render: PANIC while drawing; the renderer is inert from now on");
        }
    }
    if let Some((lines, tables)) = capture {
        let _ = std::panic::catch_unwind(|| as_ours(|| crate::gfx::write_capture(this as usize, lines, tables)));
    }
    // The game's picture is complete in the back buffer now: our draw goes on top, on the same
    // queue (gfx::after_composite). NOT for a submission made inside Present: that is the Steam
    // overlay drawing its own things into the buffer on its way out (seen: survey, step 3).
    // And the deferred stage of the scene just went in: with `r.at = scene` (the default) our draw
    // goes HERE, into the HDR scene target, before the game's forward stage, effects, TAA, tone
    // mapping and HUD (gfx::after_submission decides which of the two it acts on).
    if submission.scene.is_some() || submission.finished.is_some_and(|_| !IN_PRESENT.with(|c| c.get())) {
        if !DEAD.load(Ordering::Relaxed) && std::panic::catch_unwind(|| as_ours(|| crate::gfx::after_submission(this as usize, submission))).is_err() {
            DEAD.store(true, Ordering::SeqCst);
            rlog!("render: PANIC while drawing; the renderer is inert from now on");
        }
    }
}

/// The queues seen executing so far.
pub fn queues_seen() -> Vec<(usize, u64)> {
    QUEUES.lock().map(|q| q.clone()).unwrap_or_default()
}

/// True when `count` bytes at `address` can be read.
pub fn readable(address: usize, count: usize) -> bool {
    let mut info = MEMORY_BASIC_INFORMATION::default();
    // SAFETY: VirtualQuery only inspects the address.
    let got = unsafe { VirtualQuery(Some(address as *const c_void), &mut info, std::mem::size_of::<MEMORY_BASIC_INFORMATION>()) };
    got != 0 && info.State == MEM_COMMIT && (info.Protect & (PAGE_GUARD | PAGE_NOACCESS)).0 == 0 && info.Protect.0 != 0 && address + count <= info.BaseAddress as usize + info.RegionSize
}

/// The direct command queue a swap chain presents with. D3D12 gives no call for it, so: of the
/// direct queues of `device` that were seen executing, the one whose pointer is stored inside the
/// swap chain object (it keeps the queue it was created with); failing that, the busiest.
pub fn queue_of(swap: *mut c_void, device: &ID3D12Device) -> Option<ID3D12CommandQueue> {
    let mut candidates: Vec<(ID3D12CommandQueue, usize, u64)> = Vec::new();
    for (pointer, count) in queues_seen() {
        let raw = pointer as *mut c_void;
        // SAFETY: the pointer was `this` of an ExecuteCommandLists call: a live command queue.
        let Some(queue) = (unsafe { ID3D12CommandQueue::from_raw_borrowed(&raw) }) else { continue };
        // SAFETY: plain getters.
        let direct = unsafe { queue.GetDesc() }.Type == D3D12_COMMAND_LIST_TYPE_DIRECT;
        let mut owner: Option<ID3D12Device> = None;
        let same = unsafe { queue.GetDevice(&mut owner) }.is_ok() && owner.as_ref().is_some_and(|d| d.as_raw() == device.as_raw());
        if direct && same {
            candidates.push((queue.clone(), pointer, count));
        }
    }
    if candidates.is_empty() {
        return None;
    }
    let mut inside = None;
    for offset in (0..0x400usize).step_by(8) {
        let at = swap as usize + offset;
        if !readable(at, 8) {
            break;
        }
        // SAFETY: checked readable.
        let word = unsafe { *(at as *const usize) };
        if let Some(k) = candidates.iter().position(|(_, p, _)| *p == word) {
            inside = Some((k, offset));
            break;
        }
    }
    let chosen = match inside {
        Some((k, offset)) => {
            rlog!("render: the swap chain holds command queue {:#x} at +{offset:#x} ({} direct queue(s) seen)", candidates[k].1, candidates.len());
            k
        }
        None => {
            let k = (0..candidates.len()).max_by_key(|k| candidates[*k].2).unwrap_or(0);
            rlog!("render: no queue pointer found inside the swap chain; taking the busiest of {} direct queue(s): {:#x}", candidates.len(), candidates[k].1);
            k
        }
    };
    Some(candidates.swap_remove(chosen).0)
}

