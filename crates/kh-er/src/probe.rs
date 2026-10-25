//! (IMPL_ER16) MEMORY PROBES for finding what a real-time cutscene draws in the player's place. Testing aids only,
//! each a one-shot key of the control file; nothing here runs unless its key CHANGES.
//!
//! Every read goes through `ReadProcessMemory` on our own process: a bad or freed address gives "not read", never a
//! fault (the pointer walks of IMPL_ER11 crashed and hung the game: these cannot). The census runs on its own thread.
//!
//! - `dbg.census = Text[,Text..]`: every object in the process's private memory whose RTTI class name contains one of
//!   the texts (e.g. `ModelIns,Remo`) -> `<logs>\census_<n>.txt` (class, address; for models: name, display flags, box).
//! - `dbg.peek = 0xADDR[,words]`: the words at an address with what each points at (class names, text, floats).
//! - `dbg.poke = 0xADDR,0xVALUE[,0xMASK]`: one masked u32 write (checked writable; `WriteProcessMemory`).
//! - `dbg.hold = 0xADDR:0xVALUE:0xMASK[|..] (the control file's lines end at ; in the test tools: | separates)`: the same, written again in every phase while the key holds it.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
use std::sync::Mutex;

use windows_sys::Win32::System::Diagnostics::Debug::{ReadProcessMemory, WriteProcessMemory};
use windows_sys::Win32::System::Memory::{VirtualQuery, MEMORY_BASIC_INFORMATION, MEM_COMMIT, MEM_PRIVATE, PAGE_GUARD, PAGE_READWRITE};
use windows_sys::Win32::System::Threading::GetCurrentProcess;

use crate::control::{Control, OneShot};
use crate::erlog;

/// `len` bytes at `addr` into `out`; false when any of it could not be read.
pub fn read(addr: usize, out: &mut [u8]) -> bool {
    if addr < 0x1_0000 || addr > 0x7FFF_FFFF_0000 || out.is_empty() {
        return false;
    }
    let mut got = 0usize;
    // SAFETY: the system copies or refuses; the buffer is ours.
    unsafe { ReadProcessMemory(GetCurrentProcess(), addr as *const _, out.as_mut_ptr() as *mut _, out.len(), &mut got) != 0 && got == out.len() }
}

pub fn word(addr: usize) -> Option<usize> {
    let mut b = [0u8; 8];
    read(addr, &mut b).then(|| usize::from_le_bytes(b))
}

pub fn u32_at(addr: usize) -> Option<u32> {
    let mut b = [0u8; 4];
    read(addr, &mut b).then(|| u32::from_le_bytes(b))
}

pub fn floats<const N: usize>(addr: usize) -> Option<[f32; N]> {
    let mut b = vec![0u8; N * 4];
    if !read(addr, &mut b) {
        return None;
    }
    let mut out = [0f32; N];
    for (k, c) in b.chunks_exact(4).enumerate() {
        out[k] = f32::from_le_bytes([c[0], c[1], c[2], c[3]]);
    }
    Some(out)
}

pub fn exe() -> (usize, usize) {
    exe_range()
}

fn exe_range() -> (usize, usize) {
    use pelite::pe64::Pe;
    let base = crate::entry::exe_base();
    (base, base + crate::entry::exe_view().optional_header().SizeOfImage as usize)
}

/// The RTTI class name behind a vtable address (inside the executable), read safely.
pub fn vtable_class(vt: usize, exe: (usize, usize)) -> Option<String> {
    if vt % 8 != 0 || vt < exe.0 + 0x1000 || vt >= exe.1 {
        return None;
    }
    let col = word(vt - 8)?;
    if col < exe.0 || col >= exe.1 || u32_at(col)? != 1 {
        return None;
    }
    let td = exe.0 + u32_at(col + 12)? as usize;
    if td < exe.0 || td + 0x90 >= exe.1 {
        return None;
    }
    let mut b = [0u8; 0x70];
    if !read(td + 16, &mut b) {
        return None;
    }
    let end = b.iter().position(|c| *c == 0)?;
    let name = std::str::from_utf8(&b[..end]).ok()?;
    name.starts_with(".?A").then(|| name[4..].trim_end_matches("@@").to_string())
}

/// The class of the object at `p` (its first word a vtable of the executable's).
pub fn class_of(p: usize) -> Option<String> {
    vtable_class(word(p)?, exe_range())
}

/// A `DLString` (UTF-16) at `at`: allocator, 16 bytes of inline text or a pointer, length, capacity.
pub fn dl_string(at: usize) -> Option<String> {
    let (len, cap) = (word(at + 0x18)?, word(at + 0x20)?);
    if len == 0 || len > 63 || cap < len || cap > 0x1000 {
        return None;
    }
    let text = if cap >= 8 { word(at + 8)? } else { at + 8 };
    let mut b = vec![0u8; len * 2];
    if !read(text, &mut b) {
        return None;
    }
    let units: Vec<u16> = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    let s = String::from_utf16(&units).ok()?;
    s.chars().all(|c| c.is_ascii_graphic() || c == ' ').then_some(s)
}

/// What is known of a model instance (`CSModelIns` and its subclasses): name, display flags, box.
pub fn model_facts(model: usize) -> String {
    let mut s = String::new();
    let (Some(item), Some(disp)) = (word(model + 0x10), word(model + 0x18)) else { return s };
    if let Some(name) = dl_string(item + 0x6C0) {
        let _ = write!(s, " name {name}");
    }
    if let (Some(f1), Some(f2)) = (u32_at(disp + 0x20), u32_at(disp + 0x24)) {
        let _ = write!(s, " item {item:#x} disp {disp:#x} flags {f1:#x} {f2:#x}");
    }
    if let Some(exporter) = word(item + 0x658) {
        if let Some(f) = floats::<8>(exporter + 0x90) {
            if f.iter().all(|v| v.is_finite() && v.abs() < 1.0e6) {
                let _ = write!(s, " box ({:.2}, {:.2}, {:.2}) .. ({:.2}, {:.2}, {:.2})", f[0], f[1], f[2], f[4], f[5], f[6]);
            }
        }
    }
    s
}

/// (IMPL_ER17) The player at the moment a census is asked for: (block id, block-local place, physics place), so a map
/// object's block-local place can be printed in the physics space of that moment.
pub type Anchor = Option<(u32, [f32; 3], [f32; 3])>;

pub fn anchor() -> Anchor {
    let p = crate::game::player_mut()?;
    let b: i32 = p.current_block_id.into();
    let q = crate::game::v3(&p.chr_ins.modules.physics.position);
    Some((b as u32, [p.block_position.x, p.block_position.y, p.block_position.z], [q.x, q.y, q.z]))
}

/// (IMPL_ER17) What is known of a map object (`CSWorldGeomIns` and subclasses; SDK layout: + 0x10 its block data
/// (first word = block id), + 0x48 its MSB part: the name pointer first, place / rotation at + 0x20 / + 0x2C as in the file).
pub fn geom_facts(geom: usize, anchor: Anchor) -> String {
    let mut s = String::new();
    let (Some(block_data), Some(part)) = (word(geom + 0x10), word(geom + 0x48)) else { return s };
    let block = u32_at(block_data).unwrap_or(0);
    let name = word(part).map(|n| {
        let mut b = [0u8; 80];
        if !read(n, &mut b) { return String::new(); }
        let units: Vec<u16> = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|u| *u != 0).collect();
        String::from_utf16_lossy(&units)
    }).unwrap_or_default();
    let _ = write!(s, " name {name} block m{:02}_{:02}_{:02}_{:02}", block >> 24, block >> 16 & 255, block >> 8 & 255, block & 255);
    // (the object's own effect slot: MEASURED geom + 0x348 -> GeomMultiSfxSlot { + 0x18 sfx id, + 0x20 dummy poly })
    if let Some(slot) = word(geom + 0x348).filter(|p| *p != 0) {
        let _ = write!(s, " SLOT {slot:#x} {} sfx {} dummy {}", class_of(slot).unwrap_or_default(), u32_at(slot + 0x18).unwrap_or(0), u32_at(slot + 0x20).unwrap_or(0) & 0xFFFF);
    }
    if let Some(f) = floats::<6>(part + 0x20) {
        let _ = write!(s, " local ({:.2}, {:.2}, {:.2}) rot ({:.0}, {:.0}, {:.0})", f[0], f[1], f[2], f[3], f[4], f[5]);
        if let Some((here, local, physics)) = anchor {
            let (a, gx, gz) = ((block >> 24) as i32, (block >> 16 & 255) as i32, (block >> 8 & 255) as i32);
            let (ha, hx, hz) = ((here >> 24) as i32, (here >> 16 & 255) as i32, (here >> 8 & 255) as i32);
            let tiles = (50..89).contains(&a) && a == ha;
            if block == here || tiles {
                let (dx, dz) = if tiles { ((gx - hx) as f32 * 256.0, (gz - hz) as f32 * 256.0) } else { (0.0, 0.0) };
                let p = [physics[0] + f[0] + dx - local[0], physics[1] + f[1] - local[1], physics[2] + f[2] + dz - local[2]];
                let d = ((p[0] - physics[0]).powi(2) + (p[1] - physics[1]).powi(2) + (p[2] - physics[2]).powi(2)).sqrt();
                let _ = write!(s, " ER ({:.2}, {:.2}, {:.2}) dist {:.1}", p[0], p[1], p[2], d);
            }
        }
    }
    s
}

static CENSUS_BUSY: AtomicBool = AtomicBool::new(false);
static CENSUS_N: AtomicU32 = AtomicU32::new(0);

/// Every object of the classes asked for, found by scanning the private read-write memory for their vtables.
pub fn census(texts: Vec<String>, peek_words: usize, anchor: Anchor) {
    if CENSUS_BUSY.swap(true, Relaxed) {
        erlog!("census: one is running");
        return;
    }
    let n = CENSUS_N.fetch_add(1, Relaxed);
    let out = crate::log::dir().join(format!("census_{n}.txt"));
    erlog!("census: {texts:?} -> {}", out.display());
    std::thread::spawn(move || {
        let started = std::time::Instant::now();
        let exe = exe_range();
        let mut cache: HashMap<usize, Option<String>> = HashMap::new();
        let mut hits: Vec<(usize, usize)> = Vec::new();
        let mut buf = vec![0u8; 1 << 20];
        let (mut at, mut scanned) = (0x1_0000usize, 0usize);
        while at < 0x7FFF_FFFF_0000 && hits.len() < 200_000 {
            // SAFETY: VirtualQuery on any address; plain structure.
            let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
            if unsafe { VirtualQuery(at as *const _, &mut info, std::mem::size_of::<MEMORY_BASIC_INFORMATION>()) } == 0 {
                break;
            }
            let (base, size) = (info.BaseAddress as usize, info.RegionSize);
            let next = base.saturating_add(size);
            if next <= at {
                break;
            }
            if info.State == MEM_COMMIT && info.Type == MEM_PRIVATE && info.Protect == PAGE_READWRITE && info.Protect & PAGE_GUARD == 0 {
                let mut p = base;
                while p < next {
                    let len = (next - p).min(buf.len());
                    if read(p, &mut buf[..len]) {
                        scanned += len;
                        for (k, c) in buf[..len].chunks_exact(8).enumerate() {
                            let w = usize::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]);
                            if w >= exe.0 && w < exe.1 && w % 8 == 0 {
                                let known = cache.entry(w).or_insert_with(|| vtable_class(w, exe).filter(|name| texts.iter().any(|t| name.contains(t.as_str()))));
                                // (IMPL_ER17: the game's objects live in its own heaps high up; our own lists of earlier finds are not objects)
                                if known.is_some() && p >= 0x7000_0000_0000 {
                                    hits.push((p + k * 8, w));
                                }
                            }
                        }
                    }
                    p += len;
                }
            }
            at = next;
        }
        let mut text = String::new();
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for (_, vt) in &hits {
            *counts.entry(cache[vt].as_deref().unwrap_or("")).or_default() += 1;
        }
        let mut by: Vec<_> = counts.into_iter().collect();
        by.sort();
        let _ = writeln!(text, "census of {texts:?}: {} object(s), {} MB scanned in {} ms", hits.len(), scanned >> 20, started.elapsed().as_millis());
        for (name, n) in &by {
            let _ = writeln!(text, "  {n:6} {name}");
        }
        for (addr, vt) in hits.iter().take(20_000) {
            let name = cache[vt].as_deref().unwrap_or("");
            let _ = write!(text, "{addr:#x} {name}");
            if name.contains("ModelIns") {
                text += &model_facts(*addr);
            }
            if name.contains("Geom") && name.contains("Ins") {
                text += &geom_facts(*addr, anchor);
            }
            text.push('\n');
            if peek_words > 0 && !name.contains("ModelIns") {
                text += &peek_text(*addr, peek_words, exe);
            }
        }
        let _ = std::fs::write(&out, text);
        erlog!("census: {} object(s) of {} class(es) in {} ms ({} MB): {}", hits.len(), by.len(), started.elapsed().as_millis(), scanned >> 20, out.display());
        CENSUS_BUSY.store(false, Relaxed);
    });
}

/// (IMPL_ER17, `dbg.findf = x,y,z[,tol[,tol_y]]`) Every place in the private read-write memory where three consecutive
/// floats are that point (within the tolerances): what sits at a place in the world (map objects, characters, effect
/// instances). Each hit is printed with the nearest class whose vtable lies in the 0x600 bytes before it. Worker thread.
pub fn find_floats(want: [f32; 3], tol: f32, tol_y: f32) {
    if CENSUS_BUSY.swap(true, Relaxed) {
        erlog!("findf: a scan is running");
        return;
    }
    let n = CENSUS_N.fetch_add(1, Relaxed);
    let out = crate::log::dir().join(format!("findf_{n}.txt"));
    erlog!("findf: {want:?} +- {tol} / {tol_y} -> {}", out.display());
    std::thread::spawn(move || {
        let started = std::time::Instant::now();
        let exe = exe_range();
        let mut hits: Vec<(usize, [f32; 3])> = Vec::new();
        let mut buf = vec![0u8; 1 << 20];
        let mut at = 0x1_0000usize;
        while at < 0x7FFF_FFFF_0000 && hits.len() < 20_000 {
            // SAFETY: VirtualQuery on any address; plain structure.
            let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
            if unsafe { VirtualQuery(at as *const _, &mut info, std::mem::size_of::<MEMORY_BASIC_INFORMATION>()) } == 0 {
                break;
            }
            let (base, next) = (info.BaseAddress as usize, (info.BaseAddress as usize).saturating_add(info.RegionSize));
            if next <= at {
                break;
            }
            if info.State == MEM_COMMIT && info.Type == MEM_PRIVATE && info.Protect == PAGE_READWRITE {
                let mut p = base;
                while p < next {
                    let len = (next - p).min(buf.len());
                    if read(p, &mut buf[..len]) && len >= 12 {
                        let f = |k: usize| f32::from_le_bytes([buf[k], buf[k + 1], buf[k + 2], buf[k + 3]]);
                        let mut k = 0;
                        while k + 12 <= len {
                            let x = f(k);
                            if (x - want[0]).abs() <= tol {
                                let (y, z) = (f(k + 4), f(k + 8));
                                if (y - want[1]).abs() <= tol_y && (z - want[2]).abs() <= tol {
                                    hits.push((p + k, [x, y, z]));
                                }
                            }
                            k += 4;
                        }
                    }
                    p += len;
                }
            }
            at = next;
        }
        let mut text = String::new();
        let _ = writeln!(text, "findf {want:?} +- {tol} / {tol_y}: {} hit(s) in {} ms", hits.len(), started.elapsed().as_millis());
        let mut classes: HashMap<String, usize> = HashMap::new();
        for (addr, v) in &hits {
            // the object it belongs to: the nearest vtable word before it
            let mut owner = String::new();
            let start = (addr & !7).saturating_sub(0x600);
            let mut b = vec![0u8; (addr & !7) - start + 8];
            if read(start, &mut b) {
                for (k, c) in b.chunks_exact(8).enumerate().rev() {
                    let w = usize::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]);
                    if let Some(name) = vtable_class(w, exe) {
                        owner = format!("{name} at {:#x} (+{:#x})", start + k * 8, addr - (start + k * 8));
                        *classes.entry(name).or_default() += 1;
                        break;
                    }
                }
            }
            let _ = writeln!(text, "{addr:#x} ({:.3}, {:.3}, {:.3}) {owner}", v[0], v[1], v[2]);
        }
        let mut by: Vec<_> = classes.into_iter().collect();
        by.sort();
        for (name, n) in by {
            let _ = writeln!(text, "  {n:5} {name}");
        }
        let _ = std::fs::write(&out, text);
        erlog!("findf: {} hit(s) in {} ms: {}", hits.len(), started.elapsed().as_millis(), out.display());
        CENSUS_BUSY.store(false, Relaxed);
    });
}

/// (IMPL_ER17) A live effect instance (`CSFD4SingleSfxIns` / `CSFD4MultiSfxIns`; MEASURED: + 0x14 flags (bit 6 = shown:
/// the game's own setter is virtual 6), + 0x18 the model display entity it follows, + 0x70 its sfx id).
pub const SFX_CLASSES: [&str; 2] = ["CSFD4SingleSfxIns@CS", "CSFD4MultiSfxIns@CS"];
pub fn sfx_ins(addr: usize) -> Option<(u32, u32, usize)> {
    let class = class_of(addr)?;
    if !SFX_CLASSES.contains(&class.as_str()) {
        return None;
    }
    Some((u32_at(addr + 0x70)?, u32_at(addr + 0x14)?, word(addr + 0x18)?))
}

/// (`dbg.sfxlist = 1`, `dbg.sfxbit = ID[,ID..]:0|1`) Every live effect instance (a scan on a worker thread: seconds); with ids, bit 6 of
/// their flags is written afterwards ON THE GAME THREAD, each instance checked again (class and id) right before its write.
static SFX_FOUND: Mutex<Option<(Vec<usize>, Vec<u32>, bool)>> = Mutex::new(None);
pub fn sfx_scan(ids: Vec<u32>, show: bool, list: bool) {
    if CENSUS_BUSY.swap(true, Relaxed) {
        erlog!("sfx: a scan is running");
        return;
    }
    std::thread::spawn(move || {
        let started = std::time::Instant::now();
        let mut found: Vec<usize> = SFX_CLASSES.iter().flat_map(|c| objects_of(c)).filter(|a| *a >= 0x7000_0000_0000).collect();
        found.sort();
        if list {
            let mut by: std::collections::BTreeMap<u32, Vec<String>> = std::collections::BTreeMap::new();
            for a in &found {
                if let Some((id, flags, disp)) = sfx_ins(*a) {
                    by.entry(id).or_default().push(format!("{a:#x} flags {flags:#x} disp {disp:#x}"));
                }
            }
            let mut text = String::new();
            for (id, v) in &by {
                let _ = writeln!(text, "  sfx {id}: {} instance(s): {}", v.len(), v.join(" | "));
            }
            erlog!("sfx: {} live instance(s) of {} id(s) in {} ms:
{text}", found.len(), by.len(), started.elapsed().as_millis());
        }
        if !ids.is_empty() {
            if let Ok(mut f) = SFX_FOUND.lock() {
                *f = Some((found, ids, show));
            }
        }
        CENSUS_BUSY.store(false, Relaxed);
    });
}

/// (IMPL_ER17) THE GAME'S LIVE EFFECTS, low level (MEASURED): the effect system object = `[[[CSSfx + 0x60] + 0x18] + 0x08]`
/// (scene control -> GXFfxManager -> system); every live effect is a 0x90-byte record { + 0x00 u32 FXR id, + 0x04 .. + 0x28
/// floats (1.0), + 0x38 the system object, + 0x68 the record before it, + 0x88 its owner's handle block }.
pub fn fx_system() -> Option<usize> {
    use fromsoftware_shared::FromStatic;
    // SAFETY: the game's singleton; pointer reads through the system.
    let sfx = unsafe { eldenring::cs::CSSfxImp::instance() }.ok()? as *const _ as usize;
    word(word(word(sfx + 0x60)? + 0x18)? + 0x08).filter(|p| *p > 0x1_0000)
}
pub fn fx_record(addr: usize, system: usize) -> Option<u32> {
    let mut b = [0u8; 0x40];
    if !read(addr, &mut b) || usize::from_le_bytes(b[0x38..0x40].try_into().unwrap()) != system {
        return None;
    }
    let id = u32::from_le_bytes(b[0..4].try_into().unwrap());
    (f32::from_le_bytes(b[4..8].try_into().unwrap()).is_finite() && id > 0 && id < 2_000_000_000).then_some(id)
}
static FX_FOUND: Mutex<Option<(Vec<usize>, Vec<u32>, f32, usize)>> = Mutex::new(None);
/// (`dbg.fxlist = 1` / `dbg.fxset = ID[,ID..]:value[:first float[:count]]`) A scan for the records (worker thread); with ids, floats of theirs are written
/// afterwards on the game thread, each record checked again right before.
pub fn fx_scan(ids: Vec<u32>, value: f32, which: usize, list: bool) {
    let Some(system) = fx_system() else {
        erlog!("fx: no effect system");
        return;
    };
    if CENSUS_BUSY.swap(true, Relaxed) {
        erlog!("fx: a scan is running");
        return;
    }
    std::thread::spawn(move || {
        let started = std::time::Instant::now();
        let mut found = Vec::new();
        let mut buf = vec![0u8; 1 << 20];
        let mut at = 0x1_0000usize;
        while at < 0x7FFF_FFFF_0000 && found.len() < 20_000 {
            // SAFETY: VirtualQuery on any address; plain structure.
            let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
            if unsafe { VirtualQuery(at as *const _, &mut info, std::mem::size_of::<MEMORY_BASIC_INFORMATION>()) } == 0 {
                break;
            }
            let (base, next) = (info.BaseAddress as usize, (info.BaseAddress as usize).saturating_add(info.RegionSize));
            if next <= at {
                break;
            }
            if info.State == MEM_COMMIT && info.Type == MEM_PRIVATE && info.Protect == PAGE_READWRITE && base >= 0x7000_0000_0000 {
                let mut p = base;
                while p < next {
                    let len = (next - p).min(buf.len());
                    if read(p, &mut buf[..len]) {
                        for (k, c) in buf[..len].chunks_exact(8).enumerate() {
                            if usize::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]) == system && p + k * 8 >= 0x38 {
                                let rec = p + k * 8 - 0x38;
                                if fx_record(rec, system).is_some() {
                                    found.push(rec);
                                }
                            }
                        }
                    }
                    p += len;
                }
            }
            at = next;
        }
        if list {
            let mut by: std::collections::BTreeMap<u32, Vec<usize>> = std::collections::BTreeMap::new();
            for a in &found {
                if let Some(id) = fx_record(*a, system) {
                    by.entry(id).or_default().push(*a);
                }
            }
            let mut text = String::new();
            for (id, v) in &by {
                let _ = writeln!(text, "  fxr {id}: {} record(s): {}", v.len(), v.iter().take(12).map(|a| format!("{a:#x}")).collect::<Vec<_>>().join(" "));
            }
            erlog!("fx: {} live effect record(s) of {} FXR id(s) in {} ms (system {system:#x}):
{text}", found.len(), by.len(), started.elapsed().as_millis());
        }
        if !ids.is_empty() {
            if let Ok(mut f) = FX_FOUND.lock() {
                *f = Some((found, ids, value, which));
            }
        }
        CENSUS_BUSY.store(false, Relaxed);
    });
}

/// The words at `addr`, each with what it is: a class (vtable), an object of a class, text, floats.
pub fn peek_text(addr: usize, words: usize, exe: (usize, usize)) -> String {
    let mut s = String::new();
    for k in 0..words.min(512) {
        let a = addr + k * 8;
        let Some(w) = word(a) else {
            let _ = writeln!(s, "    +{:03x} (not readable)", k * 8);
            break;
        };
        let mut note = String::new();
        if let Some(c) = vtable_class(w, exe) {
            let _ = write!(note, " VT {c}");
        } else if w > 0x1_0000 && w < 0x7FFF_FFFF_0000 && w % 4 == 0 {
            if let Some(c) = word(w).and_then(|vt| vtable_class(vt, exe)) {
                let _ = write!(note, " -> {c}");
                if c.contains("ModelIns") {
                    note += &model_facts(w);
                }
            } else if w >= exe.0 && w < exe.1 {
                let _ = write!(note, " exe+{:#x}", w - exe.0);
            } else if word(w).is_some() {
                note += " ->";
                let mut b = [0u8; 24];
                if read(w, &mut b) {
                    if b.iter().take_while(|c| c.is_ascii_graphic()).count() >= 4 {
                        let _ = write!(note, " \"{}\"", String::from_utf8_lossy(&b[..b.iter().position(|c| !c.is_ascii_graphic()).unwrap_or(24)]));
                    } else if b.chunks_exact(2).take_while(|c| c[1] == 0 && c[0].is_ascii_graphic()).count() >= 3 {
                        let _ = write!(note, " L\"{}\"", b.chunks_exact(2).take_while(|c| c[1] == 0 && c[0].is_ascii_graphic()).map(|c| c[0] as char).collect::<String>());
                    }
                }
            }
        }
        let (lo, hi) = (f32::from_bits(w as u32), f32::from_bits((w >> 32) as u32));
        let fl = |f: f32| if f != 0.0 && f.is_finite() && f.abs() > 1.0e-4 && f.abs() < 1.0e5 { format!("{f:.4}") } else { "-".to_string() };
        let _ = writeln!(s, "    +{:03x} {w:016x} [{} {}]{note}", k * 8, fl(lo), fl(hi));
    }
    s
}

/// Every address in the private read-write memory whose word is the vtable of exactly this class (worker threads only: seconds).
pub fn objects_of(class: &str) -> Vec<usize> {
    let exe = exe_range();
    let mut cache: HashMap<usize, bool> = HashMap::new();
    let mut hits = Vec::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut at = 0x1_0000usize;
    while at < 0x7FFF_FFFF_0000 && hits.len() < 4096 {
        // SAFETY: VirtualQuery on any address; plain structure.
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { VirtualQuery(at as *const _, &mut info, std::mem::size_of::<MEMORY_BASIC_INFORMATION>()) } == 0 {
            break;
        }
        let (base, next) = (info.BaseAddress as usize, (info.BaseAddress as usize).saturating_add(info.RegionSize));
        if next <= at {
            break;
        }
        if info.State == MEM_COMMIT && info.Type == MEM_PRIVATE && info.Protect == PAGE_READWRITE {
            let mut p = base;
            while p < next {
                let len = (next - p).min(buf.len());
                if read(p, &mut buf[..len]) {
                    for (k, c) in buf[..len].chunks_exact(8).enumerate() {
                        let w = usize::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]);
                        if w >= exe.0 && w < exe.1 && w % 8 == 0 && *cache.entry(w).or_insert_with(|| vtable_class(w, exe).is_some_and(|n| n == class)) {
                            hits.push(p + k * 8);
                        }
                    }
                }
                p += len;
            }
        }
        at = next;
    }
    hits
}

/// The scene's own model of the player and its assembled parts, by a scan (testing aid): (viewer model, parts assembly).
pub fn scan_scene_player() -> (Option<usize>, Option<usize>) {
    let viewer = objects_of("CSViewerModelIns@CS").into_iter().find(|m| word(m + 0x10).and_then(|item| dl_string(item + 0x6C0)).is_some_and(|n| n == "c0000") && word(m + 0x18).and_then(|d| class_of(d)).is_some());
    let asm = viewer.and_then(|v| objects_of("CSChrAsmModelIns@CS").into_iter().find(|a| word(a + 8) == Some(v)));
    (viewer, asm)
}

/// (`dbg.path = ROOT,TARGET[,depth[,span]]`) How `target` is reached from `root` by pointers: breadth first over the
/// readable words of each object (`span` bytes of each), every read checked. Worker thread.
pub fn path(root: usize, targets: Vec<usize>, depth: usize, span: usize) {
    std::thread::spawn(move || {
        let started = std::time::Instant::now();
        let exe = exe_range();
        let mut seen: HashMap<usize, (usize, usize)> = HashMap::new(); // node -> (parent, offset in the parent)
        let mut level = vec![root];
        seen.insert(root, (0, 0));
        let mut buf = vec![0u8; span];
        let mut found = 0;
        'all: for d in 0..depth {
            let mut next = Vec::new();
            for node in &level {
                let mut len = span;
                while len >= 8 && !read(*node, &mut buf[..len]) {
                    len /= 2;
                }
                if len < 8 {
                    continue;
                }
                for (k, c) in buf[..len].chunks_exact(8).enumerate() {
                    let w = usize::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]);
                    if w < 0x1_0000_0000 || w > 0x7FFF_FFFF_0000 || w % 8 != 0 || (w >= exe.0 && w < exe.1) || seen.contains_key(&w) {
                        continue;
                    }
                    seen.insert(w, (*node, k * 8));
                    if targets.contains(&w) {
                        let mut chain = Vec::new();
                        let mut at = w;
                        while at != root {
                            let (parent, off) = seen[&at];
                            chain.push(format!("+{off:#x} -> {at:#x} {}", class_of(at).unwrap_or_default()));
                            at = parent;
                        }
                        chain.reverse();
                        erlog!("path: {root:#x} {} {} (depth {})", class_of(root).unwrap_or_default(), chain.join(" "), d + 1);
                        found += 1;
                        if found >= targets.len() {
                            break 'all;
                        }
                        continue;
                    }
                    if seen.len() < 3_000_000 {
                        next.push(w);
                    }
                }
            }
            level = next;
        }
        erlog!("path: {found} of {} target(s) reached from {root:#x}; {} object(s) looked at in {} ms", targets.len(), seen.len(), started.elapsed().as_millis());
    });
}

/// An address of the control file: hex, or `remo` (the cutscene manager), `player`, `viewer` / `asm` (by a scan).
fn named(text: &str) -> Option<usize> {
    use fromsoftware_shared::FromStatic;
    // (ase>off>off: the pointer at base + off, then the pointer at that + off ...; a last +off adds without reading)
    if let Some((base, rest)) = text.split_once('>') {
        let mut at = named(base)?;
        for off in rest.split('>') {
            if let Some(plus) = off.trim().strip_prefix('+') {
                at += hex(plus)?;
            } else {
                at = word(at + hex(off)?)?;
            }
        }
        return Some(at);
    }
    match text.trim() {
        // SAFETY: the game's singletons; plain pointer reads.
        "remo" => unsafe { eldenring::cs::CSRemo::instance() }.ok().and_then(|r| word(r as *const _ as usize + 8)),
        "player" => crate::game::player_mut().map(|p| p as *mut _ as usize),
        // (IMPL_ER17) the singletons the bloodstain / effect searches start from
        "eventman" => unsafe { eldenring::cs::CSEventManImp::instance() }.ok().map(|r| r as *const _ as usize),
        "gamedata" => unsafe { eldenring::cs::GameDataMan::instance() }.ok().map(|r| r as *const _ as usize),
        "gameman" => unsafe { eldenring::cs::GameMan::instance() }.ok().map(|r| r as *const _ as usize),
        "sfx" => unsafe { eldenring::cs::CSSfxImp::instance() }.ok().map(|r| r as *const _ as usize),
        "wsfx" => unsafe { eldenring::cs::WorldSfxMan::instance() }.ok().map(|r| r as *const _ as usize),
        "geomman" => unsafe { eldenring::cs::CSWorldGeomMan::instance() }.ok().map(|r| r as *const _ as usize),
        "pgd" => crate::game::player_mut().map(|p| crate::word!(p.player_game_data)),
        "playermodel" => crate::game::player_mut().map(|p| crate::word!(p.chr_ins.chr_model_ins)),
        "viewer" => scan_scene_player().0,
        "asm" => scan_scene_player().1,
        "fig" => crate::figure::env().and_then(|e| crate::figure::find(&crate::figure::player_acts(e))).map(|f| f.asm),
        "figmodel" => crate::figure::env().and_then(|e| crate::figure::find(&crate::figure::player_acts(e))).map(|f| f.model),
        "figfg" => crate::figure::env().and_then(|e| crate::figure::find(&crate::figure::player_acts(e))).map(|f| f.fg),
        "figitem" => crate::figure::env().and_then(|e| crate::figure::find(&crate::figure::player_acts(e))).and_then(|f| word(f.model + 0x10)),
        t => hex(t),
    }
}
fn hex(text: &str) -> Option<usize> {
    let t = text.trim();
    usize::from_str_radix(t.trim_start_matches("0x"), 16).ok()
}

/// One masked u32 write; false when the address is not writable.
pub fn poke(addr: usize, value: u32, mask: u32) -> bool {
    let Some(now) = u32_at(addr) else { return false };
    let new = (now & !mask) | (value & mask);
    if new == now {
        return true;
    }
    let mut done = 0usize;
    // SAFETY: the system copies or refuses.
    unsafe { WriteProcessMemory(GetCurrentProcess(), addr as *const _, new.to_le_bytes().as_ptr() as *const _, 4, &mut done) != 0 && done == 4 }
}

struct Shots {
    fxlist: OneShot,
    fxset: OneShot,
    sfxlist: OneShot,
    sfxbit: OneShot,
    fxr: OneShot,
    findf: OneShot,
    snap: OneShot,
    snapdiff: OneShot,
    census: OneShot,
    peek: OneShot,
    path: OneShot,
    fig: OneShot,
    skel: OneShot,
    poke: OneShot,
}

static SHOTS: Mutex<Option<Shots>> = Mutex::new(None);
static SNAPS: Mutex<Vec<(String, usize, Vec<u8>)>> = Mutex::new(Vec::new());
static HOLD: Mutex<(String, Vec<(usize, u32, u32)>)> = Mutex::new((String::new(), Vec::new()));

/// FRAME_BEGIN: the one-shot keys.
pub fn frame(control: &Control) {
    let Ok(mut guard) = SHOTS.lock() else { return };
    let shots = guard.get_or_insert_with(|| Shots { fxlist: OneShot::new("dbg.fxlist", control), fxset: OneShot::new("dbg.fxset", control), sfxlist: OneShot::new("dbg.sfxlist", control), sfxbit: OneShot::new("dbg.sfxbit", control), fxr: OneShot::new("dbg.fxr", control), findf: OneShot::new("dbg.findf", control), snap: OneShot::new("dbg.snap", control), snapdiff: OneShot::new("dbg.snapdiff", control), census: OneShot::new("dbg.census", control), peek: OneShot::new("dbg.peek", control), path: OneShot::new("dbg.path", control), fig: OneShot::new("dbg.fig", control), skel: OneShot::new("dbg.figskel", control), poke: OneShot::new("dbg.poke", control) });
    if let Some(v) = shots.census.fired(control) {
        // (`Text,Text,..[,+N]`: +N = also N words of every object that is not a model)
        let mut words = 0;
        let texts: Vec<String> = v.split(',').map(str::trim).filter(|t| !t.is_empty()).filter(|t| t.strip_prefix('+').and_then(|n| n.parse::<usize>().ok()).map(|n| words = n).is_none()).filter(|t| !t.starts_with('#')).map(str::to_string).collect();
        if !texts.is_empty() {
            census(texts, words, anchor());
        }
    }
    // (IMPL_ER17) `dbg.where = n`: the player's block, block-local place and physics place
    {
        static SEEN: Mutex<Option<String>> = Mutex::new(None);
        let now = control.str("dbg.where").map(str::to_string);
        if let Ok(mut seen) = SEEN.lock() {
            if *seen != now {
                if seen.is_some() || now.is_some() {
                    if let Some((b, l, q)) = anchor() {
                        erlog!("where: block m{:02}_{:02}_{:02}_{:02} local ({:.3}, {:.3}, {:.3}) physics ({:.3}, {:.3}, {:.3})", b >> 24, b >> 16 & 255, b >> 8 & 255, b & 255, l[0], l[1], l[2], q[0], q[1], q[2]);
                    }
                }
                *seen = now;
            }
        }
    }
    // (IMPL_ER17, testing aid) `dbg.erpress = n[,delay ms[,second press ms]]`: Elden Ring's action button (Y / E) is pressed for the game after
    // the delay (and once more later: an item window closed), so that a capture can be running when the action starts
    {
        static PLAN: Mutex<(Option<String>, Vec<std::time::Instant>)> = Mutex::new((None, Vec::new()));
        if let Ok(mut plan) = PLAN.lock() {
            let now = control.str("dbg.erpress").map(str::to_string);
            if plan.0 != now {
                if plan.0.is_some() || now.is_some() {
                    if let Some(v) = &now {
                        let ms: Vec<u64> = v.split(',').skip(1).filter_map(|t| t.trim().parse().ok()).collect();
                        let t0 = std::time::Instant::now();
                        plan.1 = if ms.is_empty() { vec![t0] } else { ms.iter().map(|m| t0 + std::time::Duration::from_millis(*m)).collect() };
                    }
                }
                plan.0 = now;
            }
            if plan.1.first().is_some_and(|t| *t <= std::time::Instant::now()) {
                plan.1.remove(0);
                let how = crate::inputs::press_for_er_try(crate::inputs::XI_Y, crate::inputs::SCAN_E, 0);
                erlog!("erpress: Y / E pressed for the game ({})", how.unwrap_or("NO WAY: neither a pad it reads nor its window in front"));
            }
        }
    }
    if shots.fxlist.fired(control).is_some() {
        fx_scan(Vec::new(), 0.0, 0, true);
    }
    if let Some(v) = shots.fxset.fired(control) {
        // (ids:value[:mask of the floats 0..9 at + 0x04 + 4 k, hex])
        let p: Vec<&str> = v.split(':').collect();
        let ids = p[0].split(',').filter_map(|t| t.trim().parse().ok()).collect();
        fx_scan(ids, p.get(1).and_then(|t| t.trim().parse().ok()).unwrap_or(0.0), p.get(2).and_then(|t| hex(t)).unwrap_or(0x3FF), false);
    }
    if let Ok(mut f) = FX_FOUND.try_lock() {
        if let Some((found, ids, value, which)) = f.take() {
            let mut n = 0;
            if let Some(system) = fx_system() {
                for a in found {
                    if fx_record(a, system).is_some_and(|id| ids.contains(&id)) {
                        for k in 0..10usize {
                            // (+ 0x08 is not a float: skipped)
                            if which >> k & 1 != 0 && k != 1 {
                                // SAFETY: the record was read through the system an instant ago (its system pointer and id checked); game thread.
                                unsafe { std::ptr::write_volatile((a + 4 + 4 * k) as *mut f32, value) };
                            }
                        }
                        n += 1;
                    }
                }
            }
            erlog!("fx: floats (mask {which:#x}) of {n} record(s) of {ids:?} set to {value}");
        }
    }
    if shots.sfxlist.fired(control).is_some() {
        sfx_scan(Vec::new(), true, true);
    }
    if let Some(v) = shots.sfxbit.fired(control) {
        let (ids, bit) = v.split_once(':').unwrap_or((v.as_str(), "0"));
        sfx_scan(ids.split(',').filter_map(|t| t.trim().parse().ok()).collect(), bit.trim() == "1", false);
    }
    if let Ok(mut f) = SFX_FOUND.try_lock() {
        if let Some((found, ids, show)) = f.take() {
            let mut n = 0;
            for a in found {
                if let Some((id, flags, _)) = sfx_ins(a) {
                    if ids.contains(&id) && (flags & 0x40 != 0) != show {
                        // SAFETY: the instance's class and id were read through the system an instant ago; game thread.
                        unsafe { std::ptr::write_volatile((a + 0x14) as *mut u32, if show { flags | 0x40 } else { flags & !0x40 }) };
                        n += 1;
                    }
                }
            }
            erlog!("sfx: bit 6 {} on {n} instance(s) of {ids:?}", if show { "SET" } else { "CLEARED" });
        }
    }
    // (IMPL_ER17) `dbg.fxr = ID[,ID..]`: the effect definitions (FXR) the game has loaded: how many, and where the nodes of these ids are.
    if let Some(v) = shots.fxr.fired(control) {
        use fromsoftware_shared::FromStatic;
        let want: Vec<u32> = v.split(',').filter_map(|t| t.trim().parse().ok()).collect();
        // SAFETY: game thread; the singleton and its resource container live as long as the game.
        if let Ok(sfx) = unsafe { eldenring::cs::CSSfxImp::instance() } {
            let container = unsafe { sfx.scene_ctrl.graphics_resource_manager.as_ref() }.resource_container.as_ref();
            let (mut n, mut lo, mut hi) = (0usize, u32::MAX, 0u32);
            for node in container.fxr_definitions.iter().take(200_000) {
                n += 1;
                lo = lo.min(node.id);
                hi = hi.max(node.id);
                if want.contains(&node.id) {
                    let wrapper = crate::word!(node.fxr_wrapper);
                    erlog!("fxr: id {} node {:#x} wrapper {wrapper:#x} fxr {:#x}", node.id, node as *const _ as usize, word(wrapper).unwrap_or(0));
                }
            }
            erlog!("fxr: {n} definition(s) loaded, ids {lo} .. {hi}");
        }
    }
    if let Some(v) = shots.findf.fired(control) {
        let f: Vec<f32> = v.split(',').filter_map(|t| t.trim().parse().ok()).collect();
        if f.len() >= 3 {
            let tol = f.get(3).copied().unwrap_or(0.05);
            find_floats([f[0], f[1], f[2]], tol, f.get(4).copied().unwrap_or(tol));
        }
    }
    // (IMPL_ER17) `dbg.snap = label,ADDR,bytes[|..]` keeps a copy; `dbg.snapdiff = label[|..]` lists the 4-byte words that differ now.
    if let Some(v) = shots.snap.fired(control) {
        for one in v.split('|') {
            let p: Vec<&str> = one.split(',').collect();
            if let (Some(label), Some(addr), Some(len)) = (p.first(), p.get(1).and_then(|t| named(t)), p.get(2).and_then(|t| hex(t))) {
                let mut b = vec![0u8; len.min(1 << 20)];
                let ok = read(addr, &mut b);
                erlog!("snap {label}: {} byte(s) at {addr:#x} {}", b.len(), if ok { "kept" } else { "NOT READABLE" });
                if ok {
                    if let Ok(mut s) = SNAPS.lock() {
                        s.retain(|e| e.0 != label.trim());
                        s.push((label.trim().to_string(), addr, b));
                    }
                }
            }
        }
    }
    if let Some(v) = shots.snapdiff.fired(control) {
        if let Ok(s) = SNAPS.lock() {
            for one in v.split('|') {
                let Some((label, addr, old)) = s.iter().find(|e| e.0 == one.trim()) else { continue };
                let mut now = vec![0u8; old.len()];
                if !read(*addr, &mut now) {
                    erlog!("snapdiff {label}: no longer readable");
                    continue;
                }
                let mut text = String::new();
                let mut n = 0;
                for (k, (a, b)) in old.chunks_exact(4).zip(now.chunks_exact(4)).enumerate() {
                    if a != b {
                        n += 1;
                        if n <= 200 {
                            let (x, y) = (u32::from_le_bytes([a[0], a[1], a[2], a[3]]), u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
                            let _ = writeln!(text, "    +{:04x} {x:08x} -> {y:08x}  ({} -> {}; {:.3} -> {:.3})", k * 4, x as i32, y as i32, f32::from_bits(x), f32::from_bits(y));
                        }
                    }
                }
                erlog!("snapdiff {label} at {addr:#x}: {n} word(s) differ:\n{text}");
            }
        }
    }
    if let Some(v) = shots.peek.fired(control) {
        for one in v.split('|') {
            let mut parts = one.split(',');
            if let Some(addr) = parts.next().and_then(named) {
                let words = parts.next().and_then(|n| n.trim().parse().ok()).unwrap_or(32);
                erlog!("peek {addr:#x} {}{}:\n{}", class_of(addr).unwrap_or_default(), model_facts(addr), peek_text(addr, words, exe_range()));
            }
        }
    }
    if let Some(v) = shots.skel.fired(control) {
        // (ADDR[,label]: a CSFD4ModelItem)
        let mut p = v.split(',');
        if let Some(item) = p.next().and_then(named) {
            crate::figure::dump_skeleton(item, &crate::log::dir().join(format!("figskel_{}.txt", p.next().unwrap_or("x").trim())));
        }
    }
    if shots.fig.fired(control).is_some() {
        crate::figure::dump();
    }
    FIG_HIDE.store(control.str("dbg.fighide").and_then(|v| v.split_once(':')).and_then(|(a, b)| Some(((hex(a)? as u64) << 32) | hex(b)? as u64)).unwrap_or(0), Relaxed);
    if let Some(v) = shots.path.fired(control) {
        // (`ROOT,TARGET[+TARGET..][,depth[,span]]`; the scans behind `viewer` / `asm` take seconds: on a thread)
        let v = v.clone();
        std::thread::spawn(move || {
            let p: Vec<&str> = v.split(',').collect();
            let (Some(root), Some(targets)) = (p.first().and_then(|t| named(t)), p.get(1).map(|t| t.split('+').filter_map(named).collect::<Vec<_>>())) else { return };
            erlog!("path: from {root:#x} to {targets:x?}");
            path(root, targets, p.get(2).and_then(|d| d.trim().parse().ok()).unwrap_or(6), p.get(3).and_then(|s| hex(s)).unwrap_or(0x700));
        });
    }
    if let Some(v) = shots.poke.fired(control) {
        for one in v.split('|') {
            let parts: Vec<usize> = one.split(',').filter_map(hex).collect();
            if parts.len() >= 2 {
                let before = u32_at(parts[0]);
                let ok = poke(parts[0], parts[1] as u32, parts.get(2).map_or(u32::MAX, |m| *m as u32));
                erlog!("poke {:#x}: {before:x?} -> {:x?} ({})", parts[0], u32_at(parts[0]), if ok { "written" } else { "NOT WRITTEN" });
            }
        }
    }
    let key = control.str("dbg.holdw").or(control.str("dbg.hold").filter(|v| v.contains(':'))).unwrap_or("");
    if let Ok(mut hold) = HOLD.lock() {
        if hold.0 != key {
            hold.0 = key.to_string();
            hold.1 = key.split('|').filter_map(|one| {
                let p: Vec<usize> = one.split(':').filter_map(hex).collect();
                (p.len() >= 2).then(|| (p[0], p[1] as u32, p.get(2).map_or(u32::MAX, |m| *m as u32)))
            }).collect();
            erlog!("hold: {} word(s) held: {:x?}", hold.1.len(), hold.1);
        }
    }
}

static FIG_HIDE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Every phase: the held words are written again.
pub fn phase() {
    let fh = FIG_HIDE.load(Relaxed);
    if fh != 0 {
        if let Some(f) = crate::figure::env().and_then(|e| crate::figure::find(&crate::figure::player_acts(e))) {
            for w in crate::figure::flag_words(&f) {
                poke(w, (fh >> 32) as u32, fh as u32);
            }
        }
    }
    if let Ok(hold) = HOLD.try_lock() {
        for (addr, value, mask) in hold.1.iter() {
            poke(*addr, *value, *mask);
        }
    }
}
