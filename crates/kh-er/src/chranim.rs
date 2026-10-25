//! ANIMATIONS OF ELDEN RING CHARACTERS (IMPL_ER6_STATE.md): what a character plays now, how to
//! ask its behaviour graph for a state, how to hold an animation.
//!
//! - READING: `CSChrTimeActModule::anim_queue` (ten slots written round robin; the slot with the
//!   largest index before `write_idx` is the newest: `now`).
//! - ASKING: the game's own "fire a Havok behaviour event by name" on the character's behaviour
//!   module, the function its own code uses for `W_RideOn`, `W_Event<id>` and the like (read from
//!   the exe with iced-x86: `get(behaviour module, &mut slot)` gives the holder of the Havok
//!   character, `fire(holder, L"W_...")` looks the name up in the game's global event-name table
//!   and queues the event; an unknown name does nothing). Both are found by pattern and must sit
//!   at their 2.7.1.0 addresses. This is what `hkbFireEvent` of the game's behaviour scripts does.
//! - HOLDING: `CSChrBehaviorModule::animation_speed` (0 = the pose stands still).
//! - THE NAME TABLE (`event_names`): the global map event name -> id the lookup reads, walked
//!   with every pointer checked (discovery aid: which `W_...` events exist at all).

use std::sync::OnceLock;

use eldenring::cs::ChrIns;

use crate::erlog;
use crate::game::plausible;
use crate::havok::{locate, Located};
use crate::word;

pub(crate) const GET: Located = Located { name: "behaviour: Havok character holder", pattern: "40 53 48 83 ec 20 48 8b 49 10 48 8b da 48 85 c9 75 10 48 c7 02 00 00 00 00 48 8b c3 48 83 c4 20 5b c3 48 83 3d", capture: 0, rva_2_7_1_0: 0x0041_b420, rva_2_7_2_0: 0x0041_b4b0 /* (PORT) */ };
pub(crate) const FIRE: Located = Located { name: "behaviour: fire event by name", pattern: "40 53 48 83 ec 40 48 83 39 00 48 8b d9 74 3b 48 85 d2 74 36 48 8d 4c 24 50 e8 ? ? ? ? 48 8b 0b 8b 00 48 85 c9 74 22 85 c0 78 1e 48 8b 89 d0 00 00 00", capture: 0, rva_2_7_1_0: 0x00c1_5ab0, rva_2_7_2_0: 0x00c1_5f70 /* (PORT) */ };
/// The global the name lookup starts from (`mov rax, [rip + ...]` at the head of the narrow-string lookup).
pub(crate) const NAMES: Located = Located { name: "behaviour: event name table", pattern: "48 89 5c 24 10 57 48 83 ec 20 48 8b 05 $ { ' } 48 8b fa 48 8b d9 48 85 c0 75 2e", capture: 1, rva_2_7_1_0: 0x03d7_9338, rva_2_7_2_0: 0x03d7_c358 /* (PORT) */ };

#[derive(Clone, Copy)]
struct Fns {
    get: usize,
    fire: usize,
    names: Option<usize>,
}

static FNS: OnceLock<Option<Fns>> = OnceLock::new();

fn fns() -> Option<Fns> {
    *FNS.get_or_init(|| {
        let (get, fire) = (locate(&GET)?, locate(&FIRE)?);
        let names = locate(&NAMES);
        erlog!("chranim: the behaviour event functions were located by pattern at the recorded RVAs (PORT) (name table: {})", if names.is_some() { "found" } else { "NOT found" });
        Some(Fns { get, fire, names })
    })
}

/// The event functions were found (asked once; logs the result).
pub fn available() -> bool {
    fns().is_some()
}

/// What a character plays.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AnimNow {
    pub id: i32,
    pub time: f32,
    pub length: f32,
}

/// The newest animation of the character's TimeAct queue.
pub fn now(chr: &ChrIns) -> AnimNow {
    if !plausible(word!(chr.modules.time_act)) {
        return AnimNow::default();
    }
    let ta = &chr.modules.time_act;
    let slot = &ta.anim_queue[((ta.write_idx as usize) + 9) % 10];
    AnimNow { id: slot.anim_id, time: slot.play_time, length: slot.anim_length }
}

/// The whole queue as text (discovery).
pub fn queue(chr: &ChrIns) -> String {
    if !plausible(word!(chr.modules.time_act)) {
        return "no time-act module".into();
    }
    let ta = &chr.modules.time_act;
    let slots: Vec<String> = ta.anim_queue.iter().map(|a| format!("{}@{:.2}/{:.2}", a.anim_id, a.play_time, a.anim_length)).collect();
    format!("w{} r{} [{}]", ta.write_idx, ta.read_idx, slots.join(" "))
}

/// Asks the character's behaviour graph for the event `name` (e.g. `W_DamageLarge`). False when
/// the functions were not found or the character has no behaviour; the game ignores names it
/// does not know.
pub fn fire(chr: &mut ChrIns, name: &str) -> bool {
    let Some(f) = fns() else { return false };
    let behavior = word!(chr.modules.behavior);
    if !plausible(behavior) || name.is_empty() || name.len() > 96 {
        return false;
    }
    // SAFETY: the game's own two calls, made the way its own code makes them (holder slot on the
    // stack, a NUL-terminated wide string); game thread, a live complete character.
    unsafe {
        if !plausible(std::ptr::read_unaligned((behavior + 0x10) as *const usize)) {
            return false;
        }
        let get: unsafe extern "C" fn(usize, *mut usize) -> *mut usize = std::mem::transmute(f.get);
        let fire: unsafe extern "C" fn(*mut usize, *const u16) = std::mem::transmute(f.fire);
        let mut slot = 0usize;
        let holder = get(behavior, &mut slot);
        if holder.is_null() || !plausible(slot) {
            return false;
        }
        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        fire(holder, wide.as_ptr());
    }
    true
}

/// The speed the character's animations play at (1 = normal, 0 = the pose stands still).
pub fn speed(chr: &ChrIns) -> f32 {
    if plausible(word!(chr.modules.behavior)) {
        chr.modules.behavior.animation_speed
    } else {
        1.0
    }
}

/// (ZETTA 2026-10-23) As `set_speed`, up to `zpose::FORWARD_SPEED_MAX` (Zettaflare's fast-forward of its target's hurt
/// to its middle, a few frames, unseen).
pub fn set_speed_fast(chr: &mut ChrIns, speed: f32) {
    if plausible(word!(chr.modules.behavior)) && speed.is_finite() {
        chr.modules.behavior.as_mut().animation_speed = speed.clamp(0.0, crate::zpose::FORWARD_SPEED_MAX);
    }
}

pub fn set_speed(chr: &mut ChrIns, speed: f32) {
    if plausible(word!(chr.modules.behavior)) && speed.is_finite() {
        chr.modules.behavior.as_mut().animation_speed = speed.clamp(0.0, 8.0);
    }
}

/// Checked reads of game memory: a region the system reported committed and readable is
/// remembered, so that walking a table costs a handful of queries (asking for every word made the
/// game stand still for minutes: IMPL_ER6_STATE.md).
#[derive(Default)]
struct Reader {
    regions: Vec<(usize, usize)>,
    queries: u32,
}

impl Reader {
    fn readable(&mut self, at: usize, len: usize) -> bool {
        use windows_sys::Win32::System::Memory::{VirtualQuery, MEMORY_BASIC_INFORMATION, MEM_COMMIT, PAGE_GUARD, PAGE_NOACCESS};
        if !(0x10000..0x7FFF_FFFF_0000).contains(&at) {
            return false;
        }
        if self.regions.iter().any(|(a, b)| at >= *a && at + len <= *b) {
            return true;
        }
        if self.queries > 20_000 {
            return false;
        }
        self.queries += 1;
        // SAFETY: VirtualQuery only describes the address.
        unsafe {
            let mut info: MEMORY_BASIC_INFORMATION = std::mem::zeroed();
            if VirtualQuery(at as *const _, &mut info, std::mem::size_of::<MEMORY_BASIC_INFORMATION>()) == 0 {
                return false;
            }
            let (base, end) = (info.BaseAddress as usize, info.BaseAddress as usize + info.RegionSize);
            let ok = info.State == MEM_COMMIT && info.Protect & (PAGE_GUARD | PAGE_NOACCESS) == 0 && info.Protect != 0;
            if ok {
                if self.regions.len() > 64 {
                    self.regions.remove(0);
                }
                self.regions.push((base, end));
            }
            ok && at + len <= end
        }
    }

    fn peek(&mut self, at: usize) -> Option<usize> {
        // SAFETY: checked readable.
        self.readable(at, 8).then(|| unsafe { std::ptr::read_unaligned(at as *const usize) })
    }
}

/// The names of the game's global behaviour-event table that contain `filter` (discovery aid).
pub fn event_names(filter: &str) -> Vec<(String, i32)> {
    let mut out = Vec::new();
    let Some(global) = fns().and_then(|f| f.names) else { return out };
    let started = std::time::Instant::now();
    let mut r = Reader::default();
    let mut total = 0u32;
    // (the chain the game's own lookup walks: [[[[G] + 8] + 0x80] + 0x18] + 0x28 = the map;
    //  [map] = 16-byte entries (name pointer, id), [map + 0x10] = 8-byte buckets (.., entry index), [map + 0x18] = mask)
    let mut walk = || -> Option<()> {
        let g = r.peek(global)?;
        let a = r.peek(g + 8)?;
        let b = r.peek(a + 0x80)?;
        let c = r.peek(b + 0x18)?;
        let map = c + 0x28;
        let (entries, buckets, mask) = (r.peek(map)?, r.peek(map + 0x10)?, r.peek(map + 0x18)? as u32);
        if mask == 0 || mask > 0xF_FFFF {
            erlog!("chranim: the event name table's mask is {mask:#x}: not walked");
            return None;
        }
        for k in 0..=mask as usize {
            if k % 4096 == 0 && started.elapsed().as_millis() > 150 {
                erlog!("chranim: the event name table walk was cut short at bucket {k} of {mask}");
                break;
            }
            let Some(bucket) = r.peek(buckets + k * 8) else { continue };
            let index = (bucket >> 32) as i32;
            if index < 0 || index as u32 > mask {
                continue;
            }
            let entry = entries + index as usize * 16;
            let (Some(name), Some(id)) = (r.peek(entry), r.peek(entry + 8)) else { continue };
            if !r.readable(name, 64) {
                continue;
            }
            total += 1;
            // SAFETY: 64 readable bytes; the string is cut at its NUL.
            let bytes = unsafe { std::slice::from_raw_parts(name as *const u8, 64) };
            let len = bytes.iter().position(|b| *b == 0).unwrap_or(64);
            if let Ok(text) = std::str::from_utf8(&bytes[..len]) {
                if text.contains(filter) && out.len() < 4000 {
                    out.push((text.to_string(), id as u32 as i32));
                }
            }
        }
        Some(())
    };
    if walk().is_none() {
        erlog!("chranim: the event name table could not be walked");
    }
    erlog!("chranim: event name table: {total} name(s) read in {:.1} ms with {} region queries", started.elapsed().as_secs_f32() * 1000.0, r.queries);
    out.sort();
    out
}
