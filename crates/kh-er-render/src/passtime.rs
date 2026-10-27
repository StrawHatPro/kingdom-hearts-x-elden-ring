//! (HAIRPERF, the user: "hair still slows the framerate down when zoomed in") THE GPU TIME OF OUR MAIN LIST BY SECTION: six
//! timestamps a frame (the list's start, four marks, its end), read back when the frame slot comes round again (it is finished
//! then), averaged over the second for the status line. Measuring only: nothing drawn depends on it; without a query heap
//! nothing is measured.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use windows::core::Interface;
use windows::Win32::Graphics::Direct3D12::*;

/// Stamps a frame: 0 the list's start, 1 before the hybrid's capture, 2 before the contact shadow, 3 before his colour draws,
/// 4 after them, 5 the list's end.
pub const STAMPS: usize = 6;
const SLOTS: usize = 8;
const NAMES: [&str; STAMPS - 1] = ["the game's light read", "the hybrid's capture", "contact shadow and setup", "his colour draws", "the rest (friends, glow, hair fringe, effects)"];

struct Kit {
    device: usize,
    heap: ID3D12QueryHeap,
    results: ID3D12Resource,
    frequency: u64,
    /// per slot: the stamps written this frame, and a resolve is waiting to be read
    marked: [[bool; STAMPS]; SLOTS],
    pending: [bool; SLOTS],
}

static KIT: Mutex<Option<Kit>> = Mutex::new(None);
static SUM: [AtomicU64; STAMPS - 1] = [const { AtomicU64::new(0) }; STAMPS - 1];
static FRAMES: AtomicU64 = AtomicU64::new(0);

unsafe fn kit<'a>(guard: &'a mut Option<Kit>, device: &ID3D12Device, queue: Option<&ID3D12CommandQueue>) -> Option<&'a mut Kit> {
    if guard.as_ref().is_some_and(|k| k.device != device.as_raw() as usize) {
        *guard = None;
    }
    if guard.is_none() {
        let queue = queue?;
        let mut heap: Option<ID3D12QueryHeap> = None;
        device.CreateQueryHeap(&D3D12_QUERY_HEAP_DESC { Type: D3D12_QUERY_HEAP_TYPE_TIMESTAMP, Count: (STAMPS * SLOTS) as u32, NodeMask: 0 }, &mut heap).ok()?;
        let results = crate::gfx::readback_buffer(device, (STAMPS * SLOTS * 8) as u64).ok()?;
        let frequency = queue.GetTimestampFrequency().ok()?.max(1);
        *guard = Some(Kit { device: device.as_raw() as usize, heap: heap?, results, frequency, marked: [[false; STAMPS]; SLOTS], pending: [false; SLOTS] });
    }
    guard.as_mut()
}

/// Stamp `k` of this frame into `list` (slot `slot`).
pub unsafe fn stamp(device: &ID3D12Device, queue: &ID3D12CommandQueue, list: &ID3D12GraphicsCommandList, slot: usize, k: usize) {
    if slot >= SLOTS || k >= STAMPS {
        return;
    }
    let Ok(mut guard) = KIT.lock() else { return };
    let Some(kit) = kit(&mut guard, device, Some(queue)) else { return };
    if k == 0 {
        kit.marked[slot] = [false; STAMPS];
    }
    list.EndQuery(&kit.heap, D3D12_QUERY_TYPE_TIMESTAMP, (slot * STAMPS + k) as u32);
    kit.marked[slot][k] = true;
}

/// The frame's last stamp written: its stamps are resolved for the read when the slot comes round again.
pub unsafe fn finish(device: &ID3D12Device, list: &ID3D12GraphicsCommandList, slot: usize) {
    if slot >= SLOTS {
        return;
    }
    let Ok(mut guard) = KIT.lock() else { return };
    let Some(kit) = kit(&mut guard, device, None) else { return };
    if !kit.marked[slot][0] || !kit.marked[slot][STAMPS - 1] {
        return;
    }
    // (a stamp of a section that did not run this frame is resolved as whatever it held: `collect` gives it the time of the
    // one before)
    list.ResolveQueryData(&kit.heap, D3D12_QUERY_TYPE_TIMESTAMP, (slot * STAMPS) as u32, STAMPS as u32, &kit.results, (slot * STAMPS * 8) as u64);
    kit.pending[slot] = true;
}

/// The slot is free again (its frame is done on the GPU): its stamps go into the second's sums.
pub unsafe fn collect(slot: usize) {
    if slot >= SLOTS {
        return;
    }
    let Ok(mut guard) = KIT.lock() else { return };
    let Some(kit) = guard.as_mut() else { return };
    if !kit.pending[slot] {
        return;
    }
    kit.pending[slot] = false;
    let range = D3D12_RANGE { Begin: slot * STAMPS * 8, End: (slot + 1) * STAMPS * 8 };
    let mut at: *mut std::ffi::c_void = std::ptr::null_mut();
    if kit.results.Map(0, Some(&range), Some(&mut at)).is_err() {
        return;
    }
    let raw = std::slice::from_raw_parts((at as *const u8).add(slot * STAMPS * 8) as *const u64, STAMPS).to_vec();
    kit.results.Unmap(0, Some(&D3D12_RANGE { Begin: 0, End: 0 }));
    // (an unwritten stamp takes the time of the one before: its section counts 0)
    let mut t = raw.clone();
    for k in 1..STAMPS {
        if !kit.marked[slot][k] {
            t[k] = t[k - 1];
        }
    }
    let total = t[STAMPS - 1].saturating_sub(t[0]).saturating_mul(1_000_000) / kit.frequency;
    if total >= 1_000_000 {
        return; // (a bad read)
    }
    for k in 0..STAMPS - 1 {
        SUM[k].fetch_add(t[k + 1].saturating_sub(t[k]).saturating_mul(1_000_000) / kit.frequency, Ordering::Relaxed);
    }
    FRAMES.fetch_add(1, Ordering::Relaxed);
}

/// The status line's part: the GPU ms a frame of each section since the last call.
pub fn status() -> String {
    let frames = FRAMES.swap(0, Ordering::Relaxed).max(1);
    let parts: Vec<String> = NAMES.iter().enumerate().map(|(k, n)| format!("{n} {:.2}", SUM[k].swap(0, Ordering::Relaxed) as f64 / 1000.0 / frames as f64)).collect();
    format!("(HAIRPERF) GPU ms a frame of our main list by section: {}", parts.join(", "))
}
