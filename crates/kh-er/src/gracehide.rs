//! (SV, the user 2026-10-17 01:05: "please delete the site of grace object so the thing on the ground never appears")
//! THE GRACE'S MOUND IS NEVER DRAWN, found from the MODEL side.
//!
//! WHY graceflame.rs's mound hiding (IMPL_ER52) did nothing: it looked for the model among the grace OBJECT's own first
//! 0x800 bytes, and the user's session log (build/sora_tmp/er/fix4/ref/kh-er_final3_session.log) says, for every one of
//! the 40 grace objects it knew: "NO MODEL OF ITS OWN FOUND among its first 0x800 bytes (3 looks; model-like pointers:
//! none): ITS MOUND STAYS DRAWN", and each second "mounds: 0 not drawn, 40 without a model found". The object does not
//! hold its model there (the game makes map models through `CSGeomModelInsDelayCreator`s, apart from the object).
//!
//! WHAT IS DONE HERE (no game function is called; the Elden Ring install and regulation.bin are untouched):
//! - THE MODELS THEMSELVES ARE FOUND: a worker thread reads the process's private read-write memory for objects whose
//!   class is `CSGeomModelIns` (the vtable's RTTI name) and whose model name (`CSFD4ModelItem` + 0x6C0, the census of
//!   IMPL_ER16 / er16/census_torrent_*.txt MEASURED "CSGeomModelIns@CS name AEG099_060 ... flags 0xf 0x1" beside a grace)
//!   contains `AEG099_060`: every grace of the base game and the DLC uses that asset (and its far-view copies). A search
//!   takes ~2.5 s of a worker (14 GB, the census's figure); it runs when the world is entered (a load, a warp), when a
//!   grace object comes into reach (graceflame.rs `hurry`), and every `grace.hide.rescan` s (default 20) while in the
//!   world: newly streamed blocks and models made late are taken within that time.
//! - EVERY FRAME (PRE_PHYSICS) bit 0 of each found model's display flags (`CSModelDispEntity` + 0x20: the bit the cutscene
//!   figure is hidden with, MEASURED in figure.rs) is cleared whenever it is set. A write is made only after the model's
//!   identity is read back the same frame (its vtable, its display entity and that entity's vtable as found): a model the
//!   game freed and whose memory was reused is dropped, never written.
//! - `grace.hide_mound = 0` (graceflame.rs's key) or `savepoint.kh = 0`: the bits are set back where we cleared them.
//! The golden flame and its ring on the ground are graceflame.rs's (the effect held at zero size and colour); the
//! collision is world.rs's (Sora's casts pass through the mound's body).

use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use windows_sys::Win32::System::Memory::{VirtualQuery, MEMORY_BASIC_INFORMATION, MEM_COMMIT, MEM_PRIVATE, PAGE_GUARD, PAGE_READWRITE};

use crate::control::Control;
use crate::erlog;
use crate::probe::{dl_string, poke, read, u32_at, vtable_class, word};

/// The grace's map asset (the name of its model).
pub const ASSET: &str = "AEG099_060";
const CLASS: &str = "CSGeomModelIns@CS";
const MODEL_ITEM: usize = 0x10;
const MODEL_DISP: usize = 0x18;
const DISP_FLAGS: usize = 0x20;
const ITEM_NAME: usize = 0x6C0;

/// (pure) Is this model name the grace's (its model or a copy of it)?
pub fn is_grace_model(name: &str) -> bool {
    name.to_ascii_uppercase().contains(ASSET)
}

/// (pure) Bit 0 of the display flags: drawn.
pub fn hidden_flags(flags: u32) -> u32 {
    flags & !1
}

/// One model found: (model, its vtable, its display entity, the entity's vtable).
type Found = (usize, usize, usize, usize);

#[derive(Clone, Copy, Debug)]
struct Model {
    model: usize,
    vt: usize,
    disp: usize,
    disp_vt: usize,
    /// We cleared its bit at least once (it is ours to set back).
    ours: bool,
}

#[derive(Default)]
struct State {
    models: Vec<Model>,
    busy: Arc<AtomicBool>,
    found: Arc<Mutex<Option<(Vec<Found>, u128, usize)>>>,
    last_scan: Option<Instant>,
    hurry: bool,
    born: u32,
    told_on: Option<bool>,
    second: Option<Instant>,
    cleared: u32,
    again: u32,
    dropped: u32,
    scans: u32,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

/// (graceflame.rs / savepoint.rs) A grace object came into reach, or the world was entered: search soon.
pub fn hurry() {
    if let Ok(mut s) = STATE.lock() {
        if let Some(s) = s.as_mut() {
            s.hurry = true;
        }
    }
}

/// The search (worker thread): every `CSGeomModelIns` whose model name is the grace's.
fn scan() -> (Vec<Found>, usize) {
    let exe = crate::probe::exe();
    let mut cache: std::collections::HashMap<usize, bool> = std::collections::HashMap::new();
    let mut hits = Vec::new();
    let mut models = 0usize;
    let mut buf = vec![0u8; 1 << 20];
    let mut at = 0x1_0000usize;
    while at < 0x7FFF_FFFF_0000 {
        // SAFETY: VirtualQuery on any address; plain structure.
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { VirtualQuery(at as *const _, &mut info, std::mem::size_of::<MEMORY_BASIC_INFORMATION>()) } == 0 {
            break;
        }
        let (base, next) = (info.BaseAddress as usize, (info.BaseAddress as usize).saturating_add(info.RegionSize));
        if next <= at {
            break;
        }
        if info.State == MEM_COMMIT && info.Type == MEM_PRIVATE && info.Protect == PAGE_READWRITE && info.Protect & PAGE_GUARD == 0 && base >= 0x7000_0000_0000 {
            let mut p = base;
            while p < next {
                let len = (next - p).min(buf.len());
                if read(p, &mut buf[..len]) {
                    for (k, c) in buf[..len].chunks_exact(8).enumerate() {
                        let w = usize::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]);
                        if w < exe.0 || w >= exe.1 || w % 8 != 0 {
                            continue;
                        }
                        if !*cache.entry(w).or_insert_with(|| vtable_class(w, exe).is_some_and(|n| n == CLASS)) {
                            continue;
                        }
                        models += 1;
                        let model = p + k * 8;
                        let Some(disp) = word(model + MODEL_DISP).filter(|d| crate::game::plausible(*d)) else { continue };
                        let Some(name) = word(model + MODEL_ITEM).filter(|i| crate::game::plausible(*i)).and_then(|item| dl_string(item + ITEM_NAME)) else { continue };
                        if !is_grace_model(&name) {
                            continue;
                        }
                        if let Some(disp_vt) = word(disp) {
                            hits.push((model, w, disp, disp_vt));
                        }
                    }
                }
                p += len;
            }
        }
        at = next;
    }
    (hits, models)
}

/// The model is still the one that was found (read back now).
fn same(m: &Model) -> bool {
    word(m.model) == Some(m.vt) && word(m.model + MODEL_DISP) == Some(m.disp) && word(m.disp) == Some(m.disp_vt)
}

/// Once a frame in the world (PRE_PHYSICS, frame.rs).
pub fn frame(control: &Control) {
    let Ok(mut guard) = STATE.lock() else { return };
    let s = guard.get_or_insert_with(State::default);
    let on = control.on("savepoint.kh") && control.flag("grace.hide_mound") != Some(false) && control.flag("grace.hide") != Some(false);
    if s.told_on != Some(on) {
        s.told_on = Some(on);
        erlog!("gracehide: (SV) the graces' mound {}", if on { "IS NEVER DRAWN: every CSGeomModelIns named AEG099_060 is found by a memory search (worker thread) and its display bit is held off every frame" } else { "is the game's (grace.hide_mound = 0 / grace.hide = 0 / savepoint.kh = 0)" });
        if !on {
            release_locked(s);
        }
    }
    if !on {
        return;
    }
    // (a new body / a load: the world is new)
    let age = crate::game::player_age();
    if age < s.born {
        s.hurry = true;
    }
    s.born = age;
    // ---- what the worker found
    if let Some((hits, ms, models)) = s.found.lock().ok().and_then(|mut f| f.take()) {
        let before = s.models.len();
        for (model, vt, disp, disp_vt) in hits {
            if !s.models.iter().any(|m| m.model == model) {
                s.models.push(Model { model, vt, disp, disp_vt, ours: false });
            }
        }
        s.scans += 1;
        if s.models.len() != before || s.scans <= 2 {
            erlog!("gracehide: search {}: {} grace model(s) known ({} new) among {models} CSGeomModelIns, {ms} ms", s.scans, s.models.len(), s.models.len() - before);
        }
    }
    // ---- a search when due
    let every = control.f32("grace.hide.rescan").filter(|v| v.is_finite() && *v >= 2.0).unwrap_or(20.0);
    let since = s.last_scan.map_or(f32::INFINITY, |t| t.elapsed().as_secs_f32());
    if !s.busy.load(Relaxed) && (since >= every || (s.hurry && since >= 4.0)) {
        s.hurry = false;
        s.last_scan = Some(Instant::now());
        s.busy.store(true, Relaxed);
        let (busy, found) = (s.busy.clone(), s.found.clone());
        let _ = std::thread::Builder::new().name("kh-gracehide".into()).spawn(move || {
            let started = Instant::now();
            let (hits, models) = scan();
            if let Ok(mut f) = found.lock() {
                *f = Some((hits, started.elapsed().as_millis(), models));
            }
            busy.store(false, Relaxed);
        });
    }
    // ---- the bits, every frame
    let mut k = 0;
    while k < s.models.len() {
        let m = s.models[k];
        if !same(&m) {
            s.models.swap_remove(k);
            s.dropped += 1;
            continue;
        }
        if let Some(flags) = u32_at(m.disp + DISP_FLAGS) {
            if flags & 1 != 0 && poke(m.disp + DISP_FLAGS, hidden_flags(flags), u32::MAX) {
                if m.ours {
                    s.again += 1;
                } else {
                    s.cleared += 1;
                }
                s.models[k].ours = true;
            }
        }
        k += 1;
    }
    if s.second.is_none_or(|t| t.elapsed().as_secs_f32() >= 1.0) {
        if s.cleared + s.again + s.dropped > 0 {
            erlog!("gracehide: {} grace model(s) held not drawn | this second: {} hidden for the first time, {} drawn again by the game and hidden again, {} gone (freed / reused: dropped, not written) | {} search(es)", s.models.len(), s.cleared, s.again, s.dropped, s.scans);
        }
        (s.cleared, s.again, s.dropped) = (0, 0, 0);
        s.second = Some(Instant::now());
    }
}

fn release_locked(s: &mut State) {
    let mut n = 0;
    for m in s.models.iter().filter(|m| m.ours) {
        if same(m) && poke(m.disp + DISP_FLAGS, 1, 1) {
            n += 1;
        }
    }
    s.models.clear();
    if n > 0 {
        erlog!("gracehide: {n} grace model(s) drawn again (given back to the game)");
    }
}

/// The payload stops: every bit we cleared is set again.
pub fn release() {
    if let Ok(mut guard) = STATE.lock() {
        if let Some(s) = guard.as_mut() {
            release_locked(s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_graces_models_are_known_by_name_and_only_bit_0_goes() {
        assert!(is_grace_model("AEG099_060"));
        assert!(is_grace_model("aeg099_060"));
        assert!(!is_grace_model("AEG099_090"));
        assert!(!is_grace_model("AEG099_06"));
        assert_eq!(hidden_flags(0xf), 0xe);
        assert_eq!(hidden_flags(0xe), 0xe);
    }
}
