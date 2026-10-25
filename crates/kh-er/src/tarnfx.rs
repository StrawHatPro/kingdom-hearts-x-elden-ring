//! (LANT, the user 2026-10-22 with build/sora_tmp/er/fix13/ref/flicker_lantern_friends.jpg: "this lantern on the road
//! passed thru him") NOTHING OF THE HIDDEN TARNISHED IS SEEN WHILE A KINGDOM HEARTS CHARACTER IS SHOWN IN HIS PLACE
//! (Sora, or Donald / Goofy when a friend leads: the hidden body follows the leader, party/leader.rs, so the rule does not
//! depend on who leads). UNTESTED IN GAME.
//!
//! WHY THE LANTERN SHOWED. The body is hidden by `ChrInsFlags1c5.enable_render = 0` (passive.rs `Switches::passive`),
//! which hides the character's own model and its equipment part models. AN EFFECT (FXR) PLAYING ON THE CHARACTER IS NOT
//! PART OF THAT MODEL: it is a record of the game's effect pool (graceflame.rs, IMPL_ER17), drawn by the effect system,
//! and the render flag does not reach it. Elden Ring's Lantern is exactly such an effect. READ FROM THE GAME'S OWN DATA
//! (offline, read-only: the project's copy of regulation.bin and the install's sfx/sfxbnd_commoneffects.ffxbnd.dcx;
//! IMPL_LANT_STATE.md has the scripts' output):
//! - EquipParamGoods 2070 "Lantern": refId_default = SpEffect 3245 (refId_1 3246 has no visual). Using the goods toggles it.
//! - SpEffectParam 3245: effectEndurance -1 (stays until toggled), stateInfo 147, vfxId 5400 and vfxId1 5401.
//! - SpEffectVfxParam 5400: midst FXR 302420 at dummy poly 160 (the hip); 5401: midst FXR 302421 at dummy poly 160.
//! - FXR 302420 = THE LANTERN'S MODEL: its render action is 10015 (a model) and its resource list is the model s87060
//!   with its textures (s87060_a / _em / _m / _n: the emissive map is the lantern's glow).
//! - FXR 302421 = THE LANTERN'S LIGHT: its render action is 609 (a point light) and its resource list is EMPTY (nothing
//!   with a texture or a mesh: nothing visible but the light it casts).
//! - No other SpEffectVfx row uses either FXR, and only SpEffect 3245 uses those rows (NPCs that carry a Lantern in their
//!   CharaInitParam use the same effect).
//!
//! WHAT IS DONE (no game function is called; nothing of the inventory, the equipment, the save or regulation.bin is
//! touched: only the seven size / colour floats of live effect records, the write graceflame.rs / cutgrace.rs / stain.rs
//! make, MEASURED in IMPL_ER17: "All seven at 0: flame and ring gone; the game never writes them again"):
//! 1. THE LANTERN'S MODEL IS HIDDEN, ITS LIGHT IS KEPT. While SpEffect 3245 is on the body, the effect pool is searched
//!    (a worker thread; the search of cutgrace.rs, then only the memory regions where records were found, a few ms) for
//!    records of FXR 302420, whose seven floats are held at 0 every frame. FXR 302421, the light, is NEVER touched: it
//!    keeps lighting dark places from the body's hip, and the body stands where the leader stands (Sora, Donald or Goofy),
//!    so the light is at the leader's hip, as the Tarnished's was. An NPC that lights its own Lantern loses its lantern
//!    model too (the light stays): the pool record does not say whose it is (not measured).
//! 2. EVERY OTHER EFFECT OF THE BODY (an equipped torch's flame or a sacred seal's glow: EquipParamWeapon residentSfxId;
//!    the auras of the Branchsword / Ritual talismans: FXR 4500 / 4790; status ailments and buffs on the body; anything a
//!    future pass finds) IS HIDDEN ONLY WHEN IT IS PROVEN TO BE THE BODY'S: those FXR ids are shared with enemies (4500 is
//!    used by 34 effect rows), so they are never hidden by id. A worker walks from the body's own effect modules
//!    (`ChrInsModuleContainer` + 0xB0 sfx, + 0xB8 vfx) through at most `WALK_DEPTH` levels of their objects to effect
//!    records (a record word, or an `FXHGSfxCtrl` whose + 0x18 is a record: the chain MEASURED for the grace's slot). It
//!    never walks from a record (its + 0x68 links the whole pool) or a control, never into a manager / world / scene /
//!    system object, the body itself or the effect system, and a walk that reaches more than `WALK_CAP` records is thrown
//!    away (it would have left the body's own structures). NOT MEASURED: whether the modules lead to the records this way
//!    at all. The first walk logs what it saw (`tarnfx: walk`), so a log tells the next pass where the records are.
//! 3. Everything is put back (the floats a record had, while it is still the same record and still at zero) when the KH
//!    character is no longer shown in the Tarnished's place (`sora.hide = 0`, another `mode`), with `tarnished.fx_hide = 0`,
//!    and when the payload goes.
//!
//! IF THE LANTERN'S LIGHT EVER GOES WITH THE MODEL (it cannot by this code: its record is not written), the hero light
//! (kh-er-render pointlight.rs, `hero.light`, automatic in the dark) still lights dark places around the leader.
//!
//! | key | default | meaning |
//! |---|---|---|
//! | `tarnished.fx_hide` | 1 | 0 = the Tarnished's effects are drawn as the game draws them (the old behaviour) |
//! | `tarnished.fx_walk` | 1 | 0 = only the Lantern's model (and `tarnished.fx_ids`) is hidden; the walk is off |
//! | `tarnished.fx_ids` | - | more FXR ids hidden wherever they play (`id,id`) |
//! | `tarnished.fx_keep` | - | more FXR ids never touched (besides the Lantern's light 302421) |
//! | `tarnished.fx_log` | 0 | 1 = a line a second |

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use windows_sys::Win32::System::Memory::{VirtualQuery, MEMORY_BASIC_INFORMATION, MEM_COMMIT, MEM_PRIVATE, PAGE_READWRITE};

use crate::control::Control;
use crate::erlog;
use crate::probe::{fx_record, read};

/// EquipParamGoods "Lantern".
pub const LANTERN_GOODS: u32 = 2070;
/// Its special effect (the goods' refId_default), read from the params at run time; this is what they held offline.
pub const LANTERN_SP_EFFECT: i32 = 3245;
/// The Lantern's model effect (hidden) and its light effect (kept).
pub const LANTERN_MODEL_FXR: u32 = 302420;
pub const LANTERN_LIGHT_FXR: u32 = 302421;

const RECORD_FLOATS: usize = 0x0C;
const FLOATS: usize = 7;
const CTRL_CLASS: &str = "FXHGSfxCtrl@FX4HG";
const CTRL_RECORD: usize = 0x18;
/// `ChrInsModuleContainer`: the sfx and vfx modules (the SDK's field order; savepoint.rs reads the same words).
const SFX_MODULE: usize = 0xB0;
const VFX_MODULE: usize = 0xB8;

/// Seconds between searches of the known pool regions while the Lantern is lit and no record of its model is held.
pub const QUICK_EVERY: f32 = 0.5;
/// Seconds between searches of the whole memory (cutgrace.rs measured 1.8 - 2.0 s of a worker thread for one).
pub const FULL_EVERY: f32 = 10.0;
/// Seconds between walks of the body's effect modules.
pub const WALK_EVERY: f32 = 1.0;
pub const WALK_DEPTH: usize = 3;
pub const WALK_BUDGET: usize = 160;
pub const WALK_CAP: usize = 24;
/// Words of an object looked at by the walk.
pub const NODE_WORDS: usize = 32;

// ------------------------------------------------------------------------------------------------ the rules (pure)

/// The hiding is on: Sora's mode runs, the body is hidden (`sora.hide` not 0) and `tarnished.fx_hide` is not 0.
pub fn wanted_on(mode_sora: bool, body_hidden: bool, key: Option<bool>) -> bool {
    mode_sora && body_hidden && key == Some(true) // (coordinator 2026-10-22) OFF by default: the user wants the Lantern visible
}

/// (RSF round 18, the user: "lights rendered through Donald" while Donald led; "whatever fixes you made to prevent things from
/// rendering through Sora ... apply them to Donald and Goofy") A FRIEND LEADS: the hidden Tarnished follows him (party/leader.rs),
/// and his Lantern's MODEL (its glowing emissive, FXR 302420, at the Tarnished's hip height) sticks out of or shows through a
/// short Donald or a thin Goofy, where Sora's body covers it. With `tarnished.fx_hide_friend` (DEFAULT 1, the user's decision
/// 2026-10-23: "hide the model, keep the light" while Donald or Goofy leads) and no `tarnished.fx_hide` key it is hidden while a
/// friend leads (the model only, by id; the walk stays off; ITS LIGHT 302421 IS KEPT); Sora leading: nothing changes.
/// `tarnished.fx_hide_friend = 0`: the Lantern as the game draws it.
static FRIEND_LEADS: AtomicBool = AtomicBool::new(false);

pub fn set_friend_leads(on: bool) {
    FRIEND_LEADS.store(on, Relaxed);
}

/// (RSF round 18) The hiding is on: the key's rule, or no key, `tarnished.fx_hide_friend` on (`friend_key`) and a friend leads.
pub fn wanted_on_lead(mode_sora: bool, body_hidden: bool, key: Option<bool>, friend_key: bool, friend_leads: bool) -> bool {
    wanted_on(mode_sora, body_hidden, key) || (mode_sora && body_hidden && key.is_none() && friend_key && friend_leads)
}

/// The built-in ids and those of a key (`id,id`), without repeats.
pub fn id_list(defaults: &[u32], key: Option<&str>) -> Vec<u32> {
    let mut ids: Vec<u32> = defaults.iter().copied().filter(|i| *i > 0).collect();
    for id in key.unwrap_or("").split(',').filter_map(|t| t.trim().parse::<u32>().ok()).filter(|i| *i > 0) {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    ids
}

/// A live effect record of FXR `id` is held hidden: never a kept id (the Lantern's light); always an id hidden by id (the
/// Lantern's model); any other only when it was reached from the body's own modules (`owned`).
pub fn hide_record(id: u32, by_id: &[u32], keep: &[u32], owned: bool) -> bool {
    !keep.contains(&id) && (owned || by_id.contains(&id))
}

/// The FXR ids an effect row set shows: for each vfx row id (> 0) its (midst, init, finish) FXR ids from `vfx`.
pub fn fxr_of(vfx_rows: &[i32], vfx: impl Fn(i32) -> Option<[i32; 3]>) -> Vec<u32> {
    let mut out = Vec::new();
    for row in vfx_rows.iter().copied().filter(|v| *v > 0) {
        for id in vfx(row).unwrap_or([-1; 3]).iter().copied().filter(|i| *i > 0) {
            if !out.contains(&(id as u32)) {
                out.push(id as u32);
            }
        }
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scan {
    /// Only the regions where records were found before.
    Quick,
    /// The whole private read-write memory.
    Full,
}

/// What the search plan looks at.
#[derive(Clone, Copy, Debug)]
pub struct ScanInputs {
    pub busy: bool,
    /// Something is to be found by id (the Lantern is lit, or `tarnished.fx_ids` names ids).
    pub wanted: bool,
    /// A record of a wanted id is held (still that record).
    pub held: bool,
    pub have_regions: bool,
    /// The last quick search found none of the wanted records.
    pub quick_missed: bool,
    pub since_quick: f32,
    pub since_full: f32,
    /// Whole searches in a row that found none of the wanted records.
    pub full_misses: u32,
}

/// Seconds between whole searches after `misses` fruitless ones in a row: 10, 20, 40, 80, then 160 (a whole search is
/// some 2 s of a worker thread; a Lantern whose model never shows up as a record must not keep one busy).
pub fn full_wait(misses: u32) -> f32 {
    FULL_EVERY * (1u32 << misses.min(4)) as f32
}

/// WHICH SEARCH IS DUE (pure): none while one runs, nothing is wanted or it is held; a whole search when no region is known
/// or a quick one missed, at most every `full_wait` s; otherwise a quick one every `QUICK_EVERY` s.
pub fn scan_plan(s: &ScanInputs) -> Option<Scan> {
    if s.busy || !s.wanted || s.held {
        return None;
    }
    if (!s.have_regions || s.quick_missed) && s.since_full >= full_wait(s.full_misses) {
        return Some(Scan::Full);
    }
    (s.have_regions && s.since_quick >= QUICK_EVERY).then_some(Scan::Quick)
}

// ------------------------------------------------------------------------------------------------ the walk (pure over Mem)

/// What the walk reads (the live process, or a test's fake memory).
pub trait Mem {
    /// `n` words at `addr`.
    fn words(&self, addr: usize, n: usize) -> Option<Vec<usize>>;
    /// The RTTI class of the object at `addr` (its first word a vtable of the executable).
    fn class(&self, addr: usize) -> Option<String>;
    /// `addr` is a live effect record: its FXR id.
    fn record(&self, addr: usize) -> Option<u32>;
}

#[derive(Debug, Default, Clone)]
pub struct WalkOut {
    /// (record, FXR id) reached.
    pub records: Vec<(usize, u32)>,
    pub nodes: usize,
    /// The classes of the objects walked through (count), for the log.
    pub classes: BTreeMap<String, u32>,
    /// More than the cap: the walk left the body's own structures; the records are not to be used.
    pub overflow: bool,
}

/// No walk into these (substrings of the class name): singletons and shared owners of everything.
pub const DENIED: [&str; 9] = ["Man", "World", "Scene", "System", "ChrIns", "Player", "Repository", "Param", "Task"];

pub fn denied(class: &str) -> bool {
    DENIED.iter().any(|d| class.contains(d))
}

fn plausible(w: usize) -> bool {
    (0x1_0000..0x7FFF_FFFF_0000).contains(&w) && w % 8 == 0
}

/// FROM THE BODY'S EFFECT MODULES TO ITS EFFECT RECORDS (breadth first, `max_depth` levels, `budget` objects): a word
/// that is a record is taken (never walked from); an `FXHGSfxCtrl`'s + 0x18 record is taken (the control is not walked
/// from); objects of a denied class, the `stop` addresses (the body, its module bag, the effect system) and repeats are
/// skipped; anything else (a class or plain memory: arrays, list nodes) is walked into while the depth allows.
pub fn walk(mem: &impl Mem, roots: &[usize], stop: &[usize], max_depth: usize, budget: usize, cap: usize) -> WalkOut {
    let mut out = WalkOut::default();
    let mut seen: HashSet<usize> = stop.iter().copied().collect();
    let mut queue: VecDeque<(usize, usize)> = VecDeque::new();
    for r in roots.iter().copied().filter(|r| plausible(*r)) {
        if seen.insert(r) {
            queue.push_back((r, 0));
        }
    }
    let take = |out: &mut WalkOut, rec: usize, id: u32| {
        if !out.records.iter().any(|(r, _)| *r == rec) {
            out.records.push((rec, id));
        }
    };
    while let Some((node, depth)) = queue.pop_front() {
        if out.nodes >= budget {
            break;
        }
        out.nodes += 1;
        let Some(words) = mem.words(node, NODE_WORDS) else { continue };
        for w in words {
            if !plausible(w) || !seen.insert(w) {
                continue;
            }
            if let Some(id) = mem.record(w) {
                take(&mut out, w, id);
                continue;
            }
            match mem.class(w) {
                Some(c) if c == CTRL_CLASS => {
                    if let Some(rec) = mem.words(w + CTRL_RECORD, 1).and_then(|v| v.first().copied()).filter(|r| plausible(*r)) {
                        if let Some(id) = mem.record(rec) {
                            take(&mut out, rec, id);
                        }
                    }
                }
                Some(c) if denied(&c) => {}
                c => {
                    if let Some(c) = c {
                        *out.classes.entry(c).or_insert(0) += 1;
                    }
                    if depth < max_depth {
                        queue.push_back((w, depth + 1));
                    }
                }
            }
        }
    }
    out.overflow = out.records.len() > cap;
    out
}

// ------------------------------------------------------------------------------------------------ the live process

/// The process's memory for the walk (worker thread: every read copies or refuses).
struct Live {
    system: usize,
    exe: (usize, usize),
    classes: std::cell::RefCell<HashMap<usize, Option<String>>>,
}

impl Mem for Live {
    fn words(&self, addr: usize, n: usize) -> Option<Vec<usize>> {
        let mut b = vec![0u8; n * 8];
        read(addr, &mut b).then(|| b.chunks_exact(8).map(|c| usize::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]])).collect())
    }
    fn class(&self, addr: usize) -> Option<String> {
        let vt = crate::probe::word(addr)?;
        if vt < self.exe.0 || vt >= self.exe.1 {
            return None;
        }
        self.classes.borrow_mut().entry(vt).or_insert_with(|| crate::probe::vtable_class(vt, self.exe)).clone()
    }
    fn record(&self, addr: usize) -> Option<u32> {
        fx_record(addr, self.system)
    }
}

/// What a search of the pool found: (record, id) of the wanted ids, the regions holding any record, how many records.
#[derive(Default)]
struct ScanOut {
    hits: Vec<(usize, u32)>,
    regions: Vec<(usize, usize)>,
    records: usize,
    ms: u128,
    full: bool,
}

/// The effect records of the process (worker thread): the whole private read-write memory (`only` empty) or the regions
/// of `only` (their bases, each queried again).
fn pool_scan(system: usize, ids: &[u32], only: &[(usize, usize)]) -> ScanOut {
    let started = Instant::now();
    let mut out = ScanOut { full: only.is_empty(), ..ScanOut::default() };
    let mut buf = vec![0u8; 1 << 20];
    let mut look = |base: usize, next: usize, out: &mut ScanOut| {
        let mut found_here = false;
        let mut p = base;
        while p < next {
            let len = (next - p).min(buf.len());
            if read(p, &mut buf[..len]) {
                for (k, c) in buf[..len].chunks_exact(8).enumerate() {
                    if usize::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]) == system && p + k * 8 >= 0x38 {
                        let rec = p + k * 8 - 0x38;
                        if let Some(id) = fx_record(rec, system) {
                            out.records += 1;
                            found_here = true;
                            if ids.contains(&id) {
                                out.hits.push((rec, id));
                            }
                        }
                    }
                }
            }
            p += len;
        }
        if found_here {
            out.regions.push((base, next - base));
        }
    };
    let query = |at: usize| -> Option<(usize, usize, bool)> {
        // SAFETY: VirtualQuery on any address; plain structure.
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { VirtualQuery(at as *const _, &mut info, std::mem::size_of::<MEMORY_BASIC_INFORMATION>()) } == 0 {
            return None;
        }
        let base = info.BaseAddress as usize;
        let ok = info.State == MEM_COMMIT && info.Type == MEM_PRIVATE && info.Protect == PAGE_READWRITE && base >= 0x7000_0000_0000;
        Some((base, base.saturating_add(info.RegionSize), ok))
    };
    if only.is_empty() {
        let mut at = 0x1_0000usize;
        while at < 0x7FFF_FFFF_0000 && out.records < 20_000 {
            let Some((base, next, ok)) = query(at) else { break };
            if next <= at {
                break;
            }
            if ok {
                look(base, next, &mut out);
            }
            at = next;
        }
    } else {
        for &(base, _) in only {
            if let Some((b, next, true)) = query(base) {
                look(b, next, &mut out);
            }
        }
    }
    out.ms = started.elapsed().as_millis();
    out
}

/// A record we hold at zero.
struct Held {
    record: usize,
    id: u32,
    was: [f32; FLOATS],
    /// Reached by the walk from the body's modules (dropped and put back when a later walk no longer reaches it).
    owned: bool,
}

#[derive(Default)]
struct State {
    told: Option<bool>,
    chain_told: bool,
    lantern_sp: Option<i32>,
    /// The body's special effects last seen (sorted) and whether the Lantern's is one of them.
    effects: Vec<i32>,
    lantern_on: bool,
    visuals: HashMap<i32, Vec<u32>>,
    held: Vec<Held>,
    regions: Vec<(usize, usize)>,
    quick_missed: bool,
    full_misses: u32,
    last_quick: Option<Instant>,
    last_full: Option<Instant>,
    last_walk: Option<Instant>,
    scan_busy: Arc<AtomicBool>,
    scan_out: Arc<Mutex<Option<ScanOut>>>,
    walk_busy: Arc<AtomicBool>,
    walk_out: Arc<Mutex<Option<WalkOut>>>,
    walks: u32,
    walk_said: Option<(usize, bool)>,
    overflow_said: bool,
    zeroed: u32,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn floats(record: usize) -> Option<[f32; FLOATS]> {
    crate::probe::floats::<FLOATS>(record + RECORD_FLOATS)
}

fn write_floats(record: usize, v: &[f32; FLOATS]) {
    for (f, x) in v.iter().enumerate() {
        // SAFETY: the record was checked through the effect system an instant before (its system pointer and FXR id,
        // `fx_record`) and its floats read; the write graceflame.rs makes; game thread.
        unsafe { std::ptr::write_volatile((record + RECORD_FLOATS + 4 * f) as *mut f32, *x) };
    }
}

/// Holds `record` at zero (remembering what it had) unless it is held already.
fn hold(st: &mut State, record: usize, id: u32, owned: bool, system: usize) {
    if let Some(h) = st.held.iter_mut().find(|h| h.record == record && h.id == id) {
        h.owned |= owned;
        return;
    }
    if fx_record(record, system) != Some(id) || st.held.len() >= 64 {
        return;
    }
    let Some(now) = floats(record) else { return };
    // (a record already at zero is someone else's hold, graceflame.rs's: kept as it is, put back as zero)
    st.held.push(Held { record, id, was: now, owned });
    if now.iter().any(|f| *f != 0.0) {
        write_floats(record, &[0.0; FLOATS]);
        st.zeroed += 1;
    }
}

/// Every record we hold gets its floats back while it is still the same record and still at zero.
fn restore_all(st: &mut State, system: usize) {
    let n = st.held.len();
    for h in st.held.drain(..) {
        if fx_record(h.record, system) == Some(h.id) && floats(h.record).is_some_and(|f| f.iter().all(|x| *x == 0.0)) {
            write_floats(h.record, &h.was);
        }
    }
    if n > 0 {
        erlog!("tarnfx: {n} effect record(s) of the Tarnished given back to the game");
    }
}

/// The Lantern's chain in the params: its special effect and, per vfx row, the (midst, init, finish) FXR ids.
fn lantern_chain() -> Option<(i32, Vec<(i32, [i32; 3])>)> {
    use eldenring::cs::{EquipParamGoods, SoloParamRepository};
    use fromsoftware_shared::FromStatic;
    // SAFETY: game thread; the repository lives as long as the game; rows are read only.
    let repo = unsafe { SoloParamRepository::instance() }.ok()?;
    let sp = repo.get::<EquipParamGoods>(LANTERN_GOODS)?.ref_id_default();
    Some((sp, vfx_rows(sp)?))
}

/// An effect's vfx rows with their FXR ids (none: the row does not exist).
fn vfx_rows(sp: i32) -> Option<Vec<(i32, [i32; 3])>> {
    use eldenring::cs::{SoloParamRepository, SpEffectParam, SpEffectVfxParam};
    use fromsoftware_shared::FromStatic;
    if sp <= 0 {
        return None;
    }
    // SAFETY: as above.
    let repo = unsafe { SoloParamRepository::instance() }.ok()?;
    let row = repo.get::<SpEffectParam>(sp as u32)?;
    let ids = [row.vfx_id(), row.vfx_id1(), row.vfx_id2(), row.vfx_id3(), row.vfx_id4(), row.vfx_id5(), row.vfx_id6(), row.vfx_id7()];
    Some(ids.iter().copied().filter(|v| *v > 0).filter_map(|v| repo.get::<SpEffectVfxParam>(v as u32).map(|r| (v, [r.midst_sfx_id(), r.init_sfx_id(), r.finish_sfx_id()]))).collect())
}

/// ONCE A FRAME (PrePhysics). `mode_sora`: `mode = sora` (out of it everything is given back); `body_hidden`: `sora.hide`
/// is not 0.
pub fn frame(control: &Control, mode_sora: bool, body_hidden: bool, tick: bool) {
    let Ok(mut guard) = STATE.lock() else { return };
    if !mode_sora && guard.as_ref().is_none_or(|st| st.held.is_empty()) {
        return; // (nothing to give back: no work outside Sora's mode)
    }
    let st = guard.get_or_insert_with(State::default);
    let Some(system) = crate::probe::fx_system() else { return };
    let on = wanted_on_lead(mode_sora, body_hidden, control.flag("tarnished.fx_hide"), control.flag("tarnished.fx_hide_friend") != Some(false), FRIEND_LEADS.load(Relaxed)); // (RSF round 18, default on: the user's decision)
    if st.told != Some(on) {
        st.told = Some(on);
        erlog!("tarnfx: the hidden Tarnished's effects are {}", if on { "NOT DRAWN while a KH character is shown in his place: the Lantern's model (FXR 302420) is held at zero size and colour, ITS LIGHT (FXR 302421) IS KEPT at the leader's hip; any other effect only when it is reached from the body's own effect modules (tarnished.fx_walk); tarnished.fx_hide = 0 switches it off" } else { "the game's (tarnished.fx_hide = 0 or the body is shown)" });
    }
    if !on {
        restore_all(st, system);
        return;
    }
    let Some(player) = crate::game::player_mut() else { return };
    if !st.chain_told {
        if let Some((sp, rows)) = lantern_chain() {
            st.chain_told = true;
            st.lantern_sp = Some(sp);
            let fxr = fxr_of(&rows.iter().map(|r| r.0).collect::<Vec<_>>(), |v| rows.iter().find(|r| r.0 == v).map(|r| r.1));
            let as_read = sp == LANTERN_SP_EFFECT && fxr.contains(&LANTERN_MODEL_FXR) && fxr.contains(&LANTERN_LIGHT_FXR);
            erlog!("tarnfx: the game's params: goods {LANTERN_GOODS} (Lantern) -> special effect {sp} -> vfx rows {rows:?} -> FXR {fxr:?}: {}", if as_read { "as read offline (model 302420 hidden, light 302421 kept)" } else { "!!! NOT AS READ OFFLINE (3245 -> 302420 model / 302421 light): the Lantern's model may stay drawn; name its model FXR in tarnished.fx_ids" });
        }
    }
    // ---- the body's special effects (what is on him and shows something: for the log; the Lantern lit or not)
    let mut effects: Vec<i32> = player.chr_ins.special_effect.entries().take(128).map(|e| e.param_id).collect();
    effects.sort_unstable();
    effects.dedup();
    if effects != st.effects {
        let lantern = st.lantern_sp.unwrap_or(LANTERN_SP_EFFECT);
        let mut shown = Vec::new();
        for id in effects.iter().filter(|id| !st.effects.contains(id)) {
            let fxr = st.visuals.entry(*id).or_insert_with(|| vfx_rows(*id).map(|rows| fxr_of(&rows.iter().map(|r| r.0).collect::<Vec<_>>(), |v| rows.iter().find(|r| r.0 == v).map(|r| r.1))).unwrap_or_default()).clone();
            if !fxr.is_empty() {
                shown.push(format!("{id} -> FXR {fxr:?}"));
            }
        }
        let lit = effects.contains(&lantern);
        if lit != st.lantern_on || !shown.is_empty() {
            erlog!("tarnfx: the body's special effects with a visual effect, new: {} | the Lantern ({lantern}) is {}", if shown.is_empty() { "none".to_string() } else { shown.join(", ") }, if lit { "LIT: its model is looked for" } else { "not lit" });
        }
        st.lantern_on = lit;
        st.effects = effects;
        st.last_quick = None; // (a change: a quick search soon, and whole ones at the first pace again)
        st.full_misses = 0;
    }
    let by_id = id_list(&[LANTERN_MODEL_FXR], control.str("tarnished.fx_ids"));
    let keep = id_list(&[LANTERN_LIGHT_FXR], control.str("tarnished.fx_keep"));
    // ---- the records held: still the same record (else dropped), at zero (the game made it again: zeroed again)
    let mut k = 0;
    while k < st.held.len() {
        let h = &st.held[k];
        if fx_record(h.record, system) != Some(h.id) || keep.contains(&h.id) {
            let h = st.held.swap_remove(k);
            if keep.contains(&h.id) && fx_record(h.record, system) == Some(h.id) {
                write_floats(h.record, &h.was);
            }
            continue;
        }
        if floats(h.record).is_some_and(|f| f.iter().any(|x| *x != 0.0)) {
            write_floats(h.record, &[0.0; FLOATS]);
            st.zeroed += 1;
        }
        k += 1;
    }
    // ---- a search's result
    if let Some(out) = st.scan_out.try_lock().ok().and_then(|mut f| f.take()) {
        let wanted: Vec<(usize, u32)> = out.hits.iter().copied().filter(|(_, id)| hide_record(*id, &by_id, &keep, false)).collect();
        if out.full || !out.regions.is_empty() {
            st.regions = out.regions.clone();
        }
        st.quick_missed = !out.full && wanted.is_empty();
        if out.full {
            st.full_misses = if wanted.is_empty() { st.full_misses + 1 } else { 0 };
        }
        let before = st.zeroed;
        for (rec, id) in &wanted {
            hold(st, *rec, *id, false, system);
        }
        if out.full || !wanted.is_empty() {
            erlog!("tarnfx: {} search of the effect pool ({} ms): {} live record(s) in {} region(s); the ids hidden by id {by_id:?}: {} record(s), {} set to zero size and colour", if out.full { "a whole" } else { "a quick" }, out.ms, out.records, st.regions.len(), wanted.len(), st.zeroed - before);
        }
    }
    // ---- a walk's result
    if let Some(out) = st.walk_out.try_lock().ok().and_then(|mut f| f.take()) {
        st.walks += 1;
        if out.overflow {
            if !st.overflow_said {
                st.overflow_said = true;
                erlog!("!!! tarnfx: the walk from the body's effect modules reached {} records (more than {WALK_CAP}): it left the body's own structures; its records are NOT used (only the Lantern's model is hidden)", out.records.len());
            }
        } else {
            let mine: Vec<(usize, u32)> = out.records.iter().copied().filter(|(_, id)| hide_record(*id, &by_id, &keep, true)).collect();
            // records no longer reached: given back (the effect ended, or the memory serves another owner now)
            let mut k = 0;
            while k < st.held.len() {
                let (record, id) = (st.held[k].record, st.held[k].id);
                if !st.held[k].owned || out.records.iter().any(|(r, i)| *r == record && *i == id) {
                    k += 1;
                    continue;
                }
                if by_id.contains(&id) {
                    st.held[k].owned = false; // (still hidden by its id)
                    k += 1;
                    continue;
                }
                let h = st.held.swap_remove(k);
                if fx_record(h.record, system) == Some(h.id) && floats(h.record).is_some_and(|f| f.iter().all(|x| *x == 0.0)) {
                    write_floats(h.record, &h.was);
                }
            }
            for (rec, id) in &mine {
                hold(st, *rec, *id, true, system);
            }
            let said = (out.records.len(), out.records.is_empty());
            if st.walk_said != Some(said) || st.walks == 1 {
                st.walk_said = Some(said);
                let mut ids: Vec<u32> = out.records.iter().map(|r| r.1).collect();
                ids.sort_unstable();
                let top: Vec<String> = { let mut v: Vec<(&String, &u32)> = out.classes.iter().collect(); v.sort_by(|a, b| b.1.cmp(a.1)); v.iter().take(12).map(|(c, n)| format!("{c} x{n}")).collect() };
                erlog!("tarnfx: walk {} of the body's effect modules: {} object(s), {} effect record(s) reached (FXR {ids:?}), {} held at zero as the body's; classes walked through: {}", st.walks, out.nodes, out.records.len(), mine.len(), if top.is_empty() { "none".to_string() } else { top.join(", ") });
            }
        }
    }
    // ---- a search, when due
    let since = |t: Option<Instant>| t.map_or(f32::INFINITY, |t| t.elapsed().as_secs_f32());
    let extra = control.str("tarnished.fx_ids").is_some_and(|s| !s.trim().is_empty());
    let held = st.held.iter().any(|h| by_id.contains(&h.id));
    let plan = scan_plan(&ScanInputs { busy: st.scan_busy.load(Relaxed), wanted: st.lantern_on || extra, held, have_regions: !st.regions.is_empty(), quick_missed: st.quick_missed, since_quick: since(st.last_quick), since_full: since(st.last_full), full_misses: st.full_misses });
    if let Some(kind) = plan {
        let now = Some(Instant::now());
        st.last_quick = now;
        if kind == Scan::Full {
            st.last_full = now;
        }
        st.scan_busy.store(true, Relaxed);
        let (busy, found, ids) = (st.scan_busy.clone(), st.scan_out.clone(), by_id.clone());
        let only = if kind == Scan::Full { Vec::new() } else { st.regions.clone() };
        let spawned = std::thread::Builder::new().name("kh-tarnfx-scan".into()).spawn(move || {
            let out = pool_scan(system, &ids, &only);
            if let Ok(mut f) = found.lock() {
                *f = Some(out);
            }
            busy.store(false, Relaxed);
        });
        if spawned.is_err() {
            st.scan_busy.store(false, Relaxed);
        }
    }
    // ---- a walk, when due
    if control.flag("tarnished.fx_walk") == Some(true) && /* (coordinator) OFF by default until seen in game */ !st.walk_busy.load(Relaxed) && since(st.last_walk) >= WALK_EVERY {
        st.last_walk = Some(Instant::now());
        let chr = &player.chr_ins as *const _ as usize;
        let modules = crate::word!(player.chr_ins.modules);
        let roots: Vec<usize> = [SFX_MODULE, VFX_MODULE].iter().filter_map(|o| crate::probe::word(modules + o)).filter(|w| *w != 0).collect();
        let sfx = {
            use fromsoftware_shared::FromStatic;
            // SAFETY: game thread; the singleton's address only.
            unsafe { eldenring::cs::CSSfxImp::instance() }.ok().map_or(0, |s| s as *const _ as usize)
        };
        let stop = vec![chr, &*player as *const eldenring::cs::PlayerIns as usize, modules, system, sfx];
        st.walk_busy.store(true, Relaxed);
        let (busy, found) = (st.walk_busy.clone(), st.walk_out.clone());
        let exe = crate::probe::exe();
        let spawned = std::thread::Builder::new().name("kh-tarnfx-walk".into()).spawn(move || {
            let mem = Live { system, exe, classes: Default::default() };
            let out = walk(&mem, &roots, &stop, WALK_DEPTH, WALK_BUDGET, WALK_CAP);
            if let Ok(mut f) = found.lock() {
                *f = Some(out);
            }
            busy.store(false, Relaxed);
        });
        if spawned.is_err() {
            st.walk_busy.store(false, Relaxed);
        }
    }
    if tick {
        if control.on("tarnished.fx_log") {
            erlog!("tarnfx: {} record(s) held at zero ({} by id, {} the body's by the walk), {} zeroed this second | the Lantern {} | {} pool region(s) known", st.held.len(), st.held.iter().filter(|h| !h.owned).count(), st.held.iter().filter(|h| h.owned).count(), st.zeroed, if st.lantern_on { "lit" } else { "not lit" }, st.regions.len());
        }
        st.zeroed = 0;
    }
}

/// The payload goes: every record gets its floats back.
pub fn release() {
    if let Ok(mut guard) = STATE.lock() {
        if let (Some(st), Some(system)) = (guard.as_mut(), crate::probe::fx_system()) {
            restore_all(st, system);
        }
        *guard = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hiding_is_off_by_default_and_on_only_by_its_key() {
        assert!(!wanted_on(true, true, None), "(coordinator) off by default: the user wants the Lantern visible");
        assert!(wanted_on(true, true, Some(true)));
        assert!(!wanted_on(true, true, Some(false)), "tarnished.fx_hide = 0");
        assert!(!wanted_on(true, false, None), "sora.hide = 0: the Tarnished is shown, with his Lantern");
        assert!(!wanted_on(false, true, None), "not Sora's mode");
    }

    /// (RSF round 18) a friend leads: the Lantern's model hidden with no key (Sora's body covers it, a friend's does not);
    /// Sora leading: as before; the key decides when given
    #[test]
    fn rsf18_a_friend_leader_hides_the_lanterns_model() {
        assert!(!wanted_on_lead(true, true, None, false, true), "tarnished.fx_hide_friend = 0: the Lantern drawn");
        assert!(wanted_on_lead(true, true, None, true, true), "the default (tarnished.fx_hide_friend on) and a friend leads: hidden");
        assert!(!wanted_on_lead(true, true, None, true, false), "Sora leads");
        assert!(!wanted_on_lead(true, true, Some(false), true, true), "tarnished.fx_hide = 0 keeps it drawn");
        assert!(wanted_on_lead(true, true, Some(true), false, false));
        assert!(!wanted_on_lead(true, false, None, true, true) && !wanted_on_lead(false, true, None, true, true));
    }

    /// THE USER'S BUG: the Lantern's model is hidden wherever it plays; its light is never touched, even when the walk
    /// reaches it on the body; other effects only when proven the body's.
    #[test]
    fn the_lantern_model_goes_its_light_stays_and_shared_effects_need_the_owner() {
        let by_id = id_list(&[LANTERN_MODEL_FXR], None);
        let keep = id_list(&[LANTERN_LIGHT_FXR], None);
        assert!(hide_record(302420, &by_id, &keep, false), "the model, found by id in the pool");
        assert!(hide_record(302420, &by_id, &keep, true));
        assert!(!hide_record(302421, &by_id, &keep, false), "the light found by id: kept");
        assert!(!hide_record(302421, &by_id, &keep, true), "the light reached on the body: still kept");
        assert!(!hide_record(4500, &by_id, &keep, false), "a talisman aura shared with 34 effect rows (enemies' too): never by id");
        assert!(hide_record(4500, &by_id, &keep, true), "the same aura reached from the body's own modules: hidden");
        assert!(hide_record(402000, &by_id, &keep, true), "a torch's flame on the body");
        assert!(!hide_record(402000, &by_id, &keep, false), "an enemy soldier's torch");
        let keep_more = id_list(&[LANTERN_LIGHT_FXR], Some("402000"));
        assert!(!hide_record(402000, &by_id, &keep_more, true), "tarnished.fx_keep");
    }

    #[test]
    fn ids_are_the_built_in_ones_and_the_keys() {
        assert_eq!(id_list(&[302420], None), [302420]);
        assert_eq!(id_list(&[302420], Some(" 4500, 302420,x,0, 4790")), [302420, 4500, 4790]);
        assert_eq!(id_list(&[0], Some("")), Vec::<u32>::new());
    }

    /// The chain as the game's params held it offline (regulation.bin copy: SpEffect 3245's vfx rows 5400 / 5401).
    #[test]
    fn the_lantern_chain_gives_its_model_and_its_light() {
        let rows = |v: i32| match v {
            5400 => Some([302420, -1, -1]),
            5401 => Some([302421, -1, -1]),
            _ => None,
        };
        assert_eq!(fxr_of(&[5400, 5401, -1, -1, -1, -1, -1, -1], rows), [LANTERN_MODEL_FXR, LANTERN_LIGHT_FXR]);
        assert_eq!(fxr_of(&[-1; 8], rows), Vec::<u32>::new());
        assert_eq!(fxr_of(&[5400, 5400, 7777], rows), [302420], "repeats once, a missing row nothing");
    }

    fn plan(busy: bool, wanted: bool, held: bool, regions: bool, missed: bool, q: f32, f: f32) -> Option<Scan> {
        scan_plan(&ScanInputs { busy, wanted, held, have_regions: regions, quick_missed: missed, since_quick: q, since_full: f, full_misses: 0 })
    }

    #[test]
    fn fruitless_whole_searches_slow_down_to_one_in_160_s() {
        assert_eq!([0, 1, 2, 3, 4, 9].map(full_wait), [10.0, 20.0, 40.0, 80.0, 160.0, 160.0]);
        let s = |misses, since_full| scan_plan(&ScanInputs { busy: false, wanted: true, held: false, have_regions: false, quick_missed: false, since_quick: 0.0, since_full, full_misses: misses });
        assert_eq!(s(3, 50.0), None);
        assert_eq!(s(3, 81.0), Some(Scan::Full));
    }

    #[test]
    fn the_pool_is_searched_only_while_the_lantern_is_lit_and_its_model_not_held() {
        assert_eq!(plan(false, false, false, false, false, 99.0, 99.0), None, "no Lantern lit: never searched");
        assert_eq!(plan(false, true, true, true, false, 99.0, 99.0), None, "its record is held");
        assert_eq!(plan(true, true, false, false, false, 99.0, 99.0), None, "a search runs");
        assert_eq!(plan(false, true, false, false, false, 0.0, f32::INFINITY), Some(Scan::Full), "the first: the whole memory");
        assert_eq!(plan(false, true, false, false, false, 0.0, 3.0), None, "no region known, a whole search 3 s ago");
        assert_eq!(plan(false, true, false, true, false, 0.6, 3.0), Some(Scan::Quick), "regions known: a quick one");
        assert_eq!(plan(false, true, false, true, false, 0.2, 3.0), None);
        assert_eq!(plan(false, true, false, true, true, 0.6, 3.0), Some(Scan::Quick), "a quick miss: quick ones go on");
        assert_eq!(plan(false, true, false, true, true, 0.6, 10.5), Some(Scan::Full), "and a whole one every 10 s");
    }

    /// A fake memory: objects (address -> words), classes (address -> name), records (address -> FXR id).
    #[derive(Default)]
    struct Fake {
        objects: HashMap<usize, Vec<usize>>,
        classes: HashMap<usize, String>,
        records: HashMap<usize, u32>,
    }

    impl Mem for Fake {
        fn words(&self, addr: usize, n: usize) -> Option<Vec<usize>> {
            if let Some(o) = self.objects.get(&addr) {
                let mut v = o.clone();
                v.resize(n, 0);
                return Some(v);
            }
            // (a word inside an object: a control's + 0x18)
            let (base, words) = self.objects.iter().find(|(b, w)| addr > **b && addr < **b + w.len() * 8)?;
            Some(words[(addr - base) / 8..].iter().copied().chain(std::iter::repeat(0)).take(n).collect())
        }
        fn class(&self, addr: usize) -> Option<String> {
            self.classes.get(&addr).cloned()
        }
        fn record(&self, addr: usize) -> Option<u32> {
            self.records.get(&addr).copied()
        }
    }

    const CHR: usize = 0x7000_0000_1000;
    const SYSTEM: usize = 0x7000_0000_2000;

    /// sfx module -> list (plain memory) -> FXHGSfxCtrl -> record (the torch); vfx module -> record (an aura) directly;
    /// the records link to an enemy's record (+ 0x68, the pool's list); the module points back at the body and at a
    /// manager that holds an enemy's record.
    fn body() -> Fake {
        let mut m = Fake::default();
        let (sfx, vfx, list, ctrl, torch, aura, enemy, man) = (0x7100_0000_0000, 0x7100_0000_1000, 0x7100_0000_2000, 0x7100_0000_3000, 0x7100_0000_4000, 0x7100_0000_5000, 0x7100_0000_6000, 0x7100_0000_7000);
        m.objects.insert(sfx, vec![0x1234, CHR, list, man, SYSTEM]);
        m.objects.insert(vfx, vec![0x1234, aura, 7, CHR]);
        m.objects.insert(list, vec![ctrl, 0, 0]);
        m.objects.insert(ctrl, vec![0x1234, 0, 0, torch]);
        m.classes.insert(sfx, "CSChrSfxModule@CS".into());
        m.classes.insert(vfx, "CSChrVfxModule@CS".into());
        m.classes.insert(ctrl, CTRL_CLASS.into());
        m.classes.insert(man, "CSWorldSfxMan@CS".into());
        m.classes.insert(CHR, "PlayerIns@CS".into());
        m.objects.insert(man, vec![0x1234, enemy]);
        m.objects.insert(CHR, vec![0x1234, enemy]);
        m.objects.insert(torch, vec![402000, 0, 0, 0, 0, 0, 0, SYSTEM, 0, 0, 0, 0, 0, enemy]);
        m.objects.insert(aura, vec![4500, 0, 0, 0, 0, 0, 0, SYSTEM, 0, 0, 0, 0, 0, enemy]);
        m.records.insert(torch, 402000);
        m.records.insert(aura, 4500);
        m.records.insert(enemy, 4500);
        m
    }

    #[test]
    fn the_walk_finds_the_bodys_records_and_nothing_of_the_pool_or_the_world() {
        let m = body();
        let out = walk(&m, &[0x7100_0000_0000, 0x7100_0000_1000], &[CHR, SYSTEM], WALK_DEPTH, WALK_BUDGET, WALK_CAP);
        let mut got: Vec<(usize, u32)> = out.records.clone();
        got.sort();
        assert_eq!(got, [(0x7100_0000_4000, 402000), (0x7100_0000_5000, 4500)], "the torch through its control, the aura directly; the enemy's record (behind the pool link, the manager, the body) never");
        assert!(!out.overflow);
        assert_eq!(out.nodes, 3, "the two modules and the list; the control is not walked from");
        assert!(!out.classes.contains_key("CSWorldSfxMan@CS"), "a manager is not walked through");
    }

    #[test]
    fn the_walk_keeps_to_its_depth_and_a_walk_that_reaches_too_much_is_thrown_away() {
        let m = body();
        // depth 0: only the roots' own words (the aura is a word of the vfx module; the torch is two objects away)
        let out = walk(&m, &[0x7100_0000_0000, 0x7100_0000_1000], &[CHR, SYSTEM], 0, WALK_BUDGET, WALK_CAP);
        assert_eq!(out.records, [(0x7100_0000_5000, 4500)]);
        // a structure with more records than the cap: overflow
        let mut wide = Fake::default();
        let root = 0x7200_0000_0000;
        let recs: Vec<usize> = (0..30).map(|k| 0x7200_0001_0000 + k * 0x100).collect();
        wide.objects.insert(root, recs.clone());
        for r in &recs {
            wide.records.insert(*r, 1000);
        }
        let out = walk(&wide, &[root], &[], WALK_DEPTH, WALK_BUDGET, WALK_CAP);
        assert!(out.overflow, "30 records > {WALK_CAP}");
        // no roots (the modules not readable): nothing
        assert!(walk(&m, &[0, 5], &[], WALK_DEPTH, WALK_BUDGET, WALK_CAP).records.is_empty());
    }

    #[test]
    fn denied_classes() {
        assert!(denied("CSWorldSfxMan@CS"));
        assert!(denied("PlayerIns@CS"));
        assert!(denied("GXFfxSceneCtrl@GX"));
        assert!(!denied("CSChrSfxModule@CS"));
        assert!(!denied(CTRL_CLASS));
    }
}
