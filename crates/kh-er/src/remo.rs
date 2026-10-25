//! (IMPL_ER11) THE CUTSCENE'S ACTORS: finding (and hiding) what stands in for the player in a real-time
//! cutscene (build/sora_tmp/er/IMPL_ER11_STATE.md). Testing aids here; the rule that came of them is in
//! what follows.

use eldenring::cs::{ChrIns, ChrSet, ChrSetEntry, WorldChrMan};
use fromsoftware_shared::FromStatic;
use glam::Vec3;

use crate::erlog;
use crate::game::{self, plausible};
use crate::word;

/// The RTTI class name of the object at `p` (its first word must be a vtable inside the game's executable).
pub fn rtti_name(p: usize) -> Option<String> {
    use pelite::pe64::Pe;
    if !plausible(p) || crate::pause::readable(p, 8) < 8 {
        return None;
    }
    let base = crate::entry::exe_base();
    let size = crate::entry::exe_view().optional_header().SizeOfImage as usize;
    let inside = |a: usize, n: usize| a >= base + 0x1000 && a + n <= base + size && crate::pause::readable(a, n) >= n;
    // SAFETY: every address is checked to be inside the mapped executable (or readable) before it is read.
    unsafe {
        let vt = std::ptr::read_unaligned(p as *const usize);
        if !inside(vt.wrapping_sub(8), 16) || vt % 8 != 0 {
            return None;
        }
        let col = std::ptr::read_unaligned((vt - 8) as *const usize);
        if !inside(col, 24) || std::ptr::read_unaligned(col as *const u32) != 1 {
            return None;
        }
        let td = base + std::ptr::read_unaligned((col + 12) as *const u32) as usize;
        if !inside(td, 0x90) {
            return None;
        }
        let bytes = std::slice::from_raw_parts((td + 16) as *const u8, 0x70);
        let end = bytes.iter().position(|b| *b == 0)?;
        let name = std::str::from_utf8(&bytes[..end]).ok()?;
        name.starts_with(".?A").then(|| name.trim_start_matches(".?AV").trim_start_matches(".?AU").trim_end_matches("@@").to_string())
    }
}

/// A character found in the game's sets.
pub struct Found {
    pub set: &'static str,
    pub ptr: usize,
    pub npc_id: i32,
    pub chr_type: i32,
    pub pos: Vec3,
    pub render: bool,
    pub class: String,
}

unsafe fn each<T>(set: &ChrSet<T>, f: &mut dyn FnMut(usize))
where
    T: fromsoftware_shared::Subclass<ChrIns> + 'static,
{
    let entries = word!(set.entries);
    let capacity = set.capacity as usize;
    if !plausible(entries) || capacity == 0 || capacity > 8192 {
        return;
    }
    for k in 0..capacity {
        let entry = (entries as *const ChrSetEntry<T>).add(k);
        let p = std::ptr::read_unaligned(entry as *const usize);
        if plausible(p) {
            f(p);
        }
    }
}

/// Every complete character of every set of the game's (the main player included), with the set it is in.
pub fn all_chrs() -> Vec<Found> {
    let mut raw: Vec<(&'static str, usize)> = Vec::new();
    // SAFETY: game thread; every pointer is checked before it is followed.
    unsafe {
        let Ok(wcm) = WorldChrMan::instance_mut() else { return Vec::new() };
        each(&wcm.player_chr_set, &mut |p| raw.push(("player", p)));
        each(&wcm.ghost_chr_set, &mut |p| raw.push(("ghost", p)));
        each(&wcm.summon_buddy_chr_set, &mut |p| raw.push(("buddy", p)));
        each(&wcm.debug_chr_set, &mut |p| raw.push(("debug", p)));
        each(&wcm.open_field_chr_set.base, &mut |p| raw.push(("field", p)));
        for k in 0..wcm.chr_sets.len() {
            let w = word!(wcm.chr_sets[k]);
            if plausible(w) {
                each(&*(w as *const ChrSet<ChrIns>), &mut |p| raw.push(("block", p)));
            }
        }
    }
    let mut out: Vec<Found> = Vec::new();
    for (set, p) in raw {
        if out.iter().any(|f| f.ptr == p) || out.len() >= 2048 || crate::pause::readable(p, 0x200) < 0x200 {
            continue;
        }
        // SAFETY: a pointer of a character-set entry, readable; completeness is checked before its parts are read.
        let chr = unsafe { &mut *(p as *mut ChrIns) };
        if !game::chr_complete(chr) {
            continue;
        }
        out.push(Found { set, ptr: p, npc_id: chr.npc_id, chr_type: chr.chr_type as i32, pos: game::v3(&chr.modules.physics.position), render: chr.chr_flags1c5.enable_render(), class: rtti_name(p).unwrap_or_default() });
    }
    out
}

/// (`dbg.remo`) What is around the body: every character within 60 m, and the cutscene manager's objects.
pub fn dump(body: Option<Vec3>) {
    let list = all_chrs();
    erlog!("remo: {} character(s) in the game's sets; within 60 m of the body:", list.len());
    for f in list.iter().filter(|f| body.is_none_or(|b| (f.pos - b).length() < 60.0)).take(80) {
        erlog!("remo:   {} {:#x} {} c{:04} type {} at ({:.2}, {:.2}, {:.2}){} render {}", f.set, f.ptr, f.class, f.npc_id, f.chr_type, f.pos.x, f.pos.y, f.pos.z, body.map_or(String::new(), |b| format!(" = {:.1} m", (f.pos - b).length())), f.render);
    }
    dump_acts();
    // (A walk of the cutscene manager's object graph by pointers was tried here twice: IT CRASHED THE GAME ONCE AND HUNG
    // IT ONCE (IMPL_ER11_STATE.md). It is gone: the cutscene's own model has to be found by reading the executable.)
}

// ---------------------------------------------------------------- (IMPL_ER12) the cutscene's animated models

/// `CSRemoAnimAct`'s virtual that hands out its model accessor (vtable slot 21, RVA 0xA89140, `fn(act, out) -> out`):
/// the act keeps what it animates at +0x5D8 / +0x5E8 / +0x600 (the accessor's three first pointers: +0x5D8 is a
/// `CSModelIns`). Detoured only to COPY those three words on the game's own call (plain reads of the live act).
/// (A detour of the act's CONSTRUCTOR was tried and the game HUNG at the next scene's loading: not done again.)
pub(crate) const ANIM_ACT_ACCESSOR: crate::title::Known = crate::title::Known { name: "CSRemoAnimAct model accessor getter", rva: 0x00A8_9140, rva_2_7_2_0: 0x00A8_9600 /* (PORT) */, bytes: "48 89 54 24 10 53 48 83 ec 40 48 c7 44 24 38 fe ff ff ff 48 8b da c7 44 24 30 00 00 00 00 48 8d 81 80 06 00 00 48 89 44 24 28 48 8b 81 90 06 00 00 48 89 44 24 20 4c 8b 89 00 06 00 00" };
static ORIG_ACCESSOR: AtomicUsize = AtomicUsize::new(0);
const MAX_MODELS: usize = 32;

/// One model a cutscene act animates: the accessor's three pointers, when the game last asked, how often.
struct Seen {
    model: AtomicUsize,
    second: AtomicUsize,
    third: AtomicUsize,
    at: AtomicU64,
    calls: AtomicU32,
}

static SEEN: [Seen; MAX_MODELS] = [const { Seen { model: AtomicUsize::new(0), second: AtomicUsize::new(0), third: AtomicUsize::new(0), at: AtomicU64::new(0), calls: AtomicU32::new(0) } }; MAX_MODELS];

use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering::Relaxed};

unsafe extern "C" fn accessor_hook(act: usize, out: usize) -> usize {
    let original: unsafe extern "C" fn(usize, usize) -> usize = std::mem::transmute(ORIG_ACCESSOR.load(Relaxed));
    // (the act is the live object the game is calling a method of: three plain reads)
    let model = std::ptr::read_unaligned((act + 0x5D8) as *const usize);
    if model != 0 {
        let now = windows_sys::Win32::System::SystemInformation::GetTickCount64();
        for seen in SEEN.iter() {
            let have = seen.model.load(Relaxed);
            if have == model || (have == 0 && seen.model.compare_exchange(0, model, Relaxed, Relaxed).is_ok()) {
                seen.second.store(std::ptr::read_unaligned((act + 0x5E8) as *const usize), Relaxed);
                seen.third.store(std::ptr::read_unaligned((act + 0x600) as *const usize), Relaxed);
                seen.at.store(now, Relaxed);
                seen.calls.fetch_add(1, Relaxed);
                break;
            }
        }
    }
    original(act, out)
}

pub fn install() {
    let Some(f) = crate::title::at(&ANIM_ACT_ACCESSOR) else { return };
    // SAFETY: the function's start at its recorded address with its recorded bytes; our hook has its signature.
    match unsafe { kh_er_render::inline_hook(f, accessor_hook as *const () as usize, &ORIG_ACCESSOR, "remo anim act accessor") } {
        Ok(()) => erlog!("remo: the cutscene's animated models are watched (CSRemoAnimAct's model accessor getter is detoured)"),
        Err(e) => {
            ORIG_ACCESSOR.store(0, std::sync::atomic::Ordering::SeqCst);
            erlog!("remo: the anim act's accessor getter was NOT detoured ({e})");
        }
    }
}

pub fn uninstalled() {
    ORIG_ACCESSOR.store(0, std::sync::atomic::Ordering::SeqCst);
}

/// The scene is over: what was seen is forgotten (the models are the scene's).
pub fn forget() {
    if let Ok(mut actors) = ACTORS.lock() {
        actors.clear();
    }
    if let Ok(mut rejected) = REJECTED.lock() {
        rejected.clear();
    }
    for seen in SEEN.iter() {
        seen.model.store(0, Relaxed);
        seen.calls.store(0, Relaxed);
    }
}

/// The models the cutscene's acts animate (the accessor's first pointer), as seen since the last `forget`.
pub fn models() -> Vec<(usize, usize, usize, u32)> {
    SEEN.iter().filter(|s| s.model.load(Relaxed) != 0).map(|s| (s.model.load(Relaxed), s.second.load(Relaxed), s.third.load(Relaxed), s.calls.load(Relaxed))).collect()
}

fn word_at(p: usize) -> Option<usize> {
    // SAFETY: checked readable.
    (plausible(p) && crate::pause::readable(p, 8) >= 8).then(|| unsafe { std::ptr::read_unaligned(p as *const usize) })
}

/// (`dbg.remo`) The models of the cutscene's acts and what each is (class names; every read checked; capped).
pub fn dump_acts() {
    let player = game::player_mut().map(|p| p as *mut _ as usize).unwrap_or(0);
    let player_model = game::player_mut().map(|p| word!(p.chr_ins.chr_model_ins)).unwrap_or(0);
    let list = models();
    erlog!("remo: {} model(s) asked for by the cutscene's animation acts; the player object is {player:#x}, its model {player_model:#x}", list.len());
    for (model, second, third, calls) in list.into_iter().take(MAX_MODELS) {
        let name = |p: usize| rtti_name(p).unwrap_or_default();
        let mut line = format!("remo:   model {model:#x} {}{} | +5E8 {second:#x} {} | +600 {third:#x} {} | {calls} call(s)", name(model), if model == player_model && model != 0 { " = THE PLAYER'S MODEL" } else { "" }, name(second), name(third));
        // a CSModelIns: +0x10 model item, +0x18 display entity (its two flag words)
        if let (Some(item), Some(disp)) = (word_at(model + 0x10), word_at(model + 0x18)) {
            line += &format!(" | item {item:#x} {} | disp {disp:#x} {}", name(item), name(disp));
            if crate::pause::readable(disp + 0x20, 8) >= 8 {
                // SAFETY: checked readable.
                let (f1, f2) = unsafe { (std::ptr::read_unaligned((disp + 0x20) as *const u32), std::ptr::read_unaligned((disp + 0x24) as *const u32)) };
                line += &format!(" flags {f1:#x} {f2:#x}");
            }
        }
        erlog!("{line}");
    }
}
/// THE CUTSCENE'S OWN MODELS HIDDEN (`CSViewerModelIns`: the scene's stand-in for the player is one of them): bit 0 of
/// the model's display entity flags (`CSModelDispEntity::disp_flags1`, the same bit that hides a character's parts) is
/// cleared while asked and put back after. `which`: `None` = all seen, `Some(k)` = the k-th.
static HIDDEN: std::sync::Mutex<Vec<(usize, u32)>> = std::sync::Mutex::new(Vec::new());

fn disp_flags(model: usize) -> Option<*mut u32> {
    let vt = word_at(model)?;
    // (only a model whose class the game's RTTI still calls a model: the object may be gone)
    if !rtti_name(model).is_some_and(|n| n.contains("ModelIns")) || vt == 0 {
        return None;
    }
    let disp = word_at(model + 0x18)?;
    (rtti_name(disp).is_some_and(|n| n.starts_with("CSModelDispEntity")) && crate::pause::readable(disp + 0x20, 8) >= 8).then(|| (disp + 0x20) as *mut u32)
}

pub fn hide_models(which: Option<usize>, on: bool) -> usize {
    let Ok(mut hidden) = HIDDEN.lock() else { return 0 };
    let list = models();
    let mut n = 0;
    for (k, (model, _, _, _)) in list.iter().enumerate() {
        let want = on && which.is_none_or(|w| w == k);
        let Some(flags) = disp_flags(*model) else { continue };
        // SAFETY: checked readable; a plain flag word of a live model's display entity (game thread).
        unsafe {
            let now = std::ptr::read_unaligned(flags);
            let had = hidden.iter().position(|(m, _)| m == model);
            if want {
                if had.is_none() {
                    hidden.push((*model, now));
                }
                if now & 1 != 0 {
                    std::ptr::write_unaligned(flags, now & !1);
                }
                n += 1;
            } else if let Some(i) = had {
                let (_, before) = hidden.remove(i);
                if before & 1 != 0 && now & 1 == 0 {
                    std::ptr::write_unaligned(flags, now | 1);
                }
            }
        }
    }
    // (models that are gone need nothing put back)
    hidden.retain(|(m, _)| list.iter().any(|(model, _, _, _)| model == m));
    n
}
/// The name of a model (its `CSFD4ModelItem`'s string at +0x6C0: allocator, 16 bytes of inline text or a pointer,
/// length, capacity; UTF-16), e.g. `c0000`. Every read checked; at most 31 units.
pub fn model_name(model: usize) -> Option<String> {
    let item = word_at(model + 0x10)?;
    if crate::pause::readable(item + 0x6C0, 0x30) < 0x30 {
        return None;
    }
    // SAFETY: checked readable.
    let (len, cap) = unsafe { (std::ptr::read_unaligned((item + 0x6D8) as *const usize), std::ptr::read_unaligned((item + 0x6E0) as *const usize)) };
    if len == 0 || len > 31 || cap < len || cap > 0x1000 {
        return None;
    }
    let text = if cap >= 8 { word_at(item + 0x6C8)? } else { item + 0x6C8 };
    if crate::pause::readable(text, len * 2) < len * 2 {
        return None;
    }
    // SAFETY: checked readable.
    let units = unsafe { std::slice::from_raw_parts(text as *const u8, len * 2) }.chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]])).collect::<Vec<_>>();
    let name = String::from_utf16(&units).ok()?;
    name.chars().all(|c| c.is_ascii_graphic()).then_some(name)
}

/// The box of a model in the world (its item's location exporter at +0x658: min at +0x90, max at +0xA0).
pub fn model_box(model: usize) -> Option<(Vec3, Vec3)> {
    let item = word_at(model + 0x10)?;
    let exporter = word_at(item + 0x658)?;
    if crate::pause::readable(exporter + 0x90, 0x20) < 0x20 {
        return None;
    }
    // SAFETY: checked readable.
    let f = unsafe { std::ptr::read_unaligned((exporter + 0x90) as *const [f32; 8]) };
    let (min, max) = (Vec3::new(f[0], f[1], f[2]), Vec3::new(f[4], f[5], f[6]));
    (min.is_finite() && max.is_finite() && (max - min).min_element() >= 0.0 && (max - min).max_element() < 50.0).then_some((min, max))
}

/// (`dbg.remo`) more of each model: its name, its box, the first words of its bone matrix array.
pub fn dump_models_more() {
    for (model, _, _, _) in models() {
        let item = word_at(model + 0x10).unwrap_or(0);
        let mut line = format!("remo:   model {model:#x} name {:?} box {:?}", model_name(model), model_box(model).map(|(a, b)| format!("({:.2}, {:.2}, {:.2}) .. ({:.2}, {:.2}, {:.2})", a.x, a.y, a.z, b.x, b.y, b.z)));
        // the raw string area, for the layout
        if crate::pause::readable(item + 0x6C0, 0x30) >= 0x30 {
            // SAFETY: checked readable.
            let w = unsafe { std::ptr::read_unaligned((item + 0x6C0) as *const [usize; 6]) };
            line += &format!(" | +6C0: {:x} {:x} {:x} {:x} {:x} {:x}", w[0], w[1], w[2], w[3], w[4], w[5]);
        }
        if let Some(m) = word_at(item + 0x640) {
            line += &format!(" | bones {m:#x} {}", rtti_name(m).unwrap_or_default());
            if crate::pause::readable(m, 0x40) >= 0x40 {
                // SAFETY: checked readable.
                let w = unsafe { std::ptr::read_unaligned(m as *const [usize; 8]) };
                line += &format!(": {:x} {:x} {:x} {:x} {:x} {:x} {:x} {:x}", w[0], w[1], w[2], w[3], w[4], w[5], w[6], w[7]);
            }
        }
        erlog!("{line}");
    }
}

/// One model of the running scene, looked at once (the expensive, checked reads) and then only compared.
struct Actor {
    model: usize,
    model_vt: usize,
    disp: usize,
    disp_vt: usize,
    player: bool,
    /// The display flags before ours (`Some` while we hold the model hidden).
    before: Option<u32>,
}

static ACTORS: std::sync::Mutex<Vec<Actor>> = std::sync::Mutex::new(Vec::new());
/// Every model that was looked at since the scene began (taken or turned down): never looked at twice.
static REJECTED: std::sync::Mutex<Vec<usize>> = std::sync::Mutex::new(Vec::new());

/// ONCE A FRAME (FRAME_BEGIN): new models of the scene are looked at (class, name: checked reads), models that are gone
/// or changed are dropped (their memory is never written again).
pub fn refresh() {
    let Ok(mut actors) = ACTORS.lock() else { return };
    let list = models();
    if list.is_empty() && actors.is_empty() {
        return;
    }
    // (the checked reads cost ~30 ms each on this machine (MEASURED: 95 ms for one new model): twice a second, not every frame)
    static EVERY: AtomicU32 = AtomicU32::new(0);
    let check = EVERY.fetch_add(1, Relaxed) % 30 == 0;
    actors.retain(|a| list.iter().any(|(m, _, _, _)| *m == a.model) && (!check || word_at(a.model) == Some(a.model_vt) && word_at(a.model + 0x18) == Some(a.disp) && word_at(a.disp) == Some(a.disp_vt) && crate::pause::readable(a.disp + 0x20, 8) >= 8));
    for (model, _, _, _) in list {
        if actors.iter().any(|a| a.model == model) || actors.len() >= MAX_MODELS {
            continue;
        }
        // (A MODEL IS LOOKED AT ONCE: the checked reads cost tens of milliseconds; a model that was turned down used to be
        // looked at again every frame and the scene ran at 6 frames a second: MEASURED)
        let Ok(mut rejected) = REJECTED.lock() else { return };
        if rejected.contains(&model) {
            continue;
        }
        if rejected.len() < 64 {
            rejected.push(model);
        }
        drop(rejected);
        let started = std::time::Instant::now();
        let (Some(model_vt), Some(disp)) = (word_at(model), word_at(model + 0x18)) else { continue };
        let Some(disp_vt) = word_at(disp) else { continue };
        if !rtti_name(model).is_some_and(|n| n.starts_with("CSViewerModelIns")) || !rtti_name(disp).is_some_and(|n| n.starts_with("CSModelDispEntity")) || crate::pause::readable(disp + 0x20, 8) < 8 {
            continue;
        }
        let name = model_name(model);
        let player = name.as_deref().is_some_and(|n| n.starts_with("c0000"));
        erlog!("cutscene: a model of the scene: {model:#x} {:?}{} (looked at in {} us)", name, if player { " = THE SCENE'S OWN MODEL OF THE PLAYER" } else { "" }, started.elapsed().as_micros());
        actors.push(Actor { model, model_vt, disp, disp_vt, player, before: None });
    }
}

/// THE RULE (`cut.actor`, see frame.rs): while a real-time cutscene plays, the scene's own model of the player (a
/// `CSViewerModelIns` named `c0000...`) is held hidden (bit 0 of its display flags, as a character's parts are
/// hidden); Sora stands in its place. Called in every phase (plain compares and one flag word). Returns how many are held.
pub fn hide_player_models(on: bool) -> usize {
    let Ok(mut actors) = ACTORS.lock() else { return 0 };
    let mut n = 0;
    for a in actors.iter_mut() {
        // SAFETY: the model and its display entity were read (checked) at this frame's start; plain reads / one flag word.
        unsafe {
            if std::ptr::read_unaligned(a.model as *const usize) != a.model_vt || std::ptr::read_unaligned((a.model + 0x18) as *const usize) != a.disp || std::ptr::read_unaligned(a.disp as *const usize) != a.disp_vt {
                continue;
            }
            let flags = (a.disp + 0x20) as *mut u32;
            let now = std::ptr::read_unaligned(flags);
            if on && a.player {
                if a.before.is_none() {
                    a.before = Some(now);
                    erlog!("cutscene: THE SCENE'S OWN MODEL OF THE PLAYER ({:#x}) is hidden (display flags {now:#x}): Sora stands in for it", a.model);
                }
                if now & 1 != 0 {
                    std::ptr::write_unaligned(flags, now & !1);
                }
                n += 1;
            } else if let Some(before) = a.before.take() {
                if before & 1 != 0 && now & 1 == 0 {
                    std::ptr::write_unaligned(flags, now | 1);
                }
            }
        }
    }
    n
}
/// Where the scene's model of the player is (the middle of its box on the ground it lies / stands on).
pub fn player_model_box() -> Option<(Vec3, Vec3)> {
    models().into_iter().find(|(m, _, _, _)| model_name(*m).is_some_and(|n| n.starts_with("c0000"))).and_then(|(m, _, _, _)| model_box(m))
}