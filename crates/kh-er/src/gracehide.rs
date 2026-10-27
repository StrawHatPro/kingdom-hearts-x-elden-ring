//! (SV, the user 2026-10-17 01:05: "please delete the site of grace object so the thing on the ground never appears")
//! THE GRACE'S MOUND IS NEVER DRAWN, found from the grace OBJECT side, with no memory search.
//!
//! (GRACENOSCAN, the user 2026-10-27: "what about the drops in frame when I was running around?") WHY THE SEARCH WENT: the
//! models used to be found by a worker thread reading the process's whole private read-write memory (~14 GB, 2.4 - 3.7 s a
//! search). MEASURED in the user's log of 2026-10-27: every search made one frame of 110 - 345 ms (338 / 345 / 228 ms
//! inside searches 5, 8 and 10): its VirtualQuery of the game's huge heap region costs ~100 ms a call and holds the address
//! space lock the game's own allocations wait on; a lower priority and pauses did not help. NO VirtualQuery IS MADE HERE.
//!
//! THE DIRECT ROUTE (MEASURED in the same log, 28.808 s: "m60_42_37_00-AEG099_060_9000 ... ITS MOUND IS NOT DRAWN:
//! CSGeomModelIns at object + 0x1d0 (model AEG099_060, display flags 0xf)", the same for all five graces near the player,
//! and the memory search of 29.643 s found exactly those five "5 grace model(s) known ... among 12483 CSGeomModelIns"):
//! the grace's map object (`CSWorldGeomIns`, found by graceflame.rs in the blocks' object vectors of the SDK's
//! `CSWorldGeomMan`: every grace of the base game and the DLC, ones in blocks streamed in later within one search round of
//! graceflame.rs) holds its mound's model (`CSGeomModelIns`, a `CSModelIns`: + 0x10 its `CSFD4ModelItem` whose name is at
//! + 0x6C0, + 0x18 its `CSModelDispEntity`) at + 0x1D0 (inside the SDK's `unk1a8` of `CSWorldGeomIns`). The model is made
//! LATE (the game makes map models through `CSGeomModelInsDelayCreator`s when the object comes within its draw distance)
//! and freed again far away: the old IMPL_ER52 look gave up after three frames, before the model existed. So:
//! - EVERY FRAME (PRE_PHYSICS, right after graceflame.rs) each grace object's model word is read (+ 0x1D0 and any offset a
//!   full look learnt; one read for an object without a model): a model of class `CSGeomModelIns` with a
//!   `CSModelDispEntity...` and a name containing `AEG099_060` is taken THE FRAME IT APPEARS.
//! - A FULL LOOK (the object's first 0x800 bytes, as IMPL_ER52 did) is made for one object a frame at most: once for each
//!   new object, once more when its model has appeared, and every `LOOK_EVERY` s for an object without one (a model held
//!   elsewhere teaches its offset to the per-frame read).
//! - Bit 0 of each found model's display flags (`CSModelDispEntity` + 0x20: the bit the cutscene figure is hidden with,
//!   MEASURED in figure.rs) is cleared whenever it is set. A write is made only after the model's identity is read back
//!   the same frame (the object still holds it, its vtable, its display entity and that entity's vtable as found): a model
//!   the game freed and whose memory was reused is dropped, never written.
//! - `grace.hide_mound = 0` (graceflame.rs's key), `grace.hide = 0` or `savepoint.kh = 0`: the bits are set back where we
//!   cleared them.
//!
//! COST: a few dozen guarded reads a frame (objects x 1 + models x 5) and one full look of ~2 KB and its pointers every few
//! frames at most; far below a millisecond, a few KB a second. No game function is called; the Elden Ring install and
//! regulation.bin are untouched. The golden flame and its ring on the ground are graceflame.rs's (the effect held at zero
//! size and colour); the collision is world.rs's (Sora's casts pass through the mound's body).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

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
/// (GRACENOSCAN) MEASURED: where the grace object holds its mound's model (five of five graces, the user's log 2026-10-27).
pub const MODEL_AT: usize = 0x1D0;
/// (GRACENOSCAN) A full look: the object's first bytes (the SDK sizes `CSWorldGeomIns` 0x430; its subclasses are larger);
/// a model past `BASE_END` is taken only by its name.
const LOOK: usize = 0x800;
const BASE_END: usize = 0x430;
/// (GRACENOSCAN) Seconds between two full looks of one object without a model.
const LOOK_EVERY: f32 = 20.0;
/// (GRACENOSCAN) Frames before a pointer that was not the mound is looked at again (a model being made: no name yet).
const RECHECK: u32 = 10;
/// (GRACENOSCAN) Model lines told in a session (later ones only in the per-second counts).
const TELL: u32 = 64;

/// (pure) Is this model name the grace's (its model or a copy of it)?
pub fn is_grace_model(name: &str) -> bool {
    name.to_ascii_uppercase().contains(ASSET)
}

/// (pure) Bit 0 of the display flags: drawn.
pub fn hidden_flags(flags: u32) -> u32 {
    flags & !1
}

/// (IMPL_ER52) One model-like pointer of an object: (its offset in the object, of class `CSGeomModelIns`, its model name).
pub type ModelCandidate = (usize, bool, Option<String>);

/// (IMPL_ER52, moved here from graceflame.rs by GRACENOSCAN) WHICH OF THE OBJECT'S MODELS IS THE MOUND (pure): every one
/// whose name contains the grace's asset; without a readable name, the one `CSGeomModelIns` inside the base object's
/// extent when it is the only model there. Indices.
pub fn pick_models(cands: &[ModelCandidate]) -> Vec<usize> {
    let named: Vec<usize> = cands.iter().enumerate().filter(|(_, c)| c.1 && c.2.as_ref().is_some_and(|n| is_grace_model(n))).map(|(k, _)| k).collect();
    if !named.is_empty() {
        return named;
    }
    let inside: Vec<usize> = cands.iter().enumerate().filter(|(_, c)| c.0 < BASE_END).map(|(k, _)| k).collect();
    match inside.as_slice() {
        [k] if cands[*k].1 && cands[*k].2.is_none() => vec![*k],
        _ => Vec::new(),
    }
}

/// (GRACENOSCAN, pure) Is a full look of this object due? Once for a new object (never looked), once more when its model
/// has appeared, and every `every` s while it has none.
pub fn look_due(has_model: bool, looked_with_model: bool, since_look: Option<f32>, every: f32) -> bool {
    match since_look {
        None => true,
        Some(_) if has_model => !looked_with_model,
        Some(s) => s >= every,
    }
}

/// (GRACENOSCAN, pure) Round robin: the first index from `cursor` on (wrapping) whose look is due.
pub fn next_due(cursor: usize, n: usize, due: impl Fn(usize) -> bool) -> Option<usize> {
    (0..n).map(|k| (cursor + k) % n).find(|k| due(*k))
}

#[derive(Clone, Copy, Debug)]
struct Model {
    /// Where the object holds it.
    off: usize,
    model: usize,
    vt: usize,
    disp: usize,
    disp_vt: usize,
    /// We cleared its bit at least once (it is ours to set back).
    ours: bool,
}

/// (GRACENOSCAN) One grace object (graceflame.rs's) and its mound.
#[derive(Clone, Debug)]
struct Obj {
    geom: usize,
    models: Vec<Model>,
    looked: Option<Instant>,
    looked_with_model: bool,
    /// Pointers seen in its model words that were not the mound: (pointer, frame).
    rejected: Vec<(usize, u32)>,
}

impl Obj {
    fn new(geom: usize) -> Self {
        Obj { geom, models: Vec::new(), looked: None, looked_with_model: false, rejected: Vec::new() }
    }
}

#[derive(Default)]
struct State {
    objs: Vec<Obj>,
    /// Offsets of the object the per-frame read looks at (`MODEL_AT` first; full looks add more).
    offsets: Vec<usize>,
    /// vtable -> RTTI class (None = not a class).
    classes: HashMap<usize, Option<String>>,
    cursor: usize,
    frame: u32,
    hurry: bool,
    born: u32,
    told_on: Option<bool>,
    told_models: u32,
    second: Option<Instant>,
    cleared: u32,
    again: u32,
    dropped: u32,
    found: u32,
    looks: u32,
    spent: std::time::Duration,
    frames: u32,
    told_counts: (usize, usize),
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

/// (graceflame.rs / savepoint.rs) A grace object came into reach, or the world was entered: every object without a model
/// gets its full look again soon (the per-frame read needs no hurry).
pub fn hurry() {
    if let Ok(mut s) = STATE.lock() {
        if let Some(s) = s.as_mut() {
            s.hurry = true;
        }
    }
}

impl State {
    fn class(&mut self, vt: usize) -> Option<String> {
        if self.classes.len() > 4096 {
            self.classes.clear();
        }
        let exe = crate::probe::exe();
        self.classes.entry(vt).or_insert_with(|| vtable_class(vt, exe)).clone()
    }

    /// A pointer that is a model (class `...ModelIns`, a `CSModelDispEntity...` at + 0x18): (geom model class, its name,
    /// its vtable, its display entity, the entity's vtable).
    fn model_at(&mut self, p: usize) -> Option<(bool, Option<String>, usize, usize, usize)> {
        let vt = word(p)?;
        let class = self.class(vt)?;
        if !class.ends_with("ModelIns@CS") {
            return None;
        }
        let disp = word(p + MODEL_DISP).filter(|d| crate::game::plausible(*d))?;
        let disp_vt = word(disp)?;
        if !self.class(disp_vt).is_some_and(|c| c.starts_with("CSModelDispEntity")) {
            return None;
        }
        let name = word(p + MODEL_ITEM).filter(|i| crate::game::plausible(*i)).and_then(|item| dl_string(item + ITEM_NAME));
        Some((class == CLASS, name, vt, disp, disp_vt))
    }
}

/// The model is still the one that was found, still held by its object (read back now).
fn same(geom: usize, m: &Model) -> bool {
    word(geom + m.off) == Some(m.model) && word(m.model) == Some(m.vt) && word(m.model + MODEL_DISP) == Some(m.disp) && word(m.disp) == Some(m.disp_vt)
}

/// (GRACENOSCAN) The per-frame read of one object's model words: a named grace model there is taken.
fn quick(s: &mut State, k: usize) -> Vec<Model> {
    let (geom, frame) = (s.objs[k].geom, s.frame);
    let mut out = Vec::new();
    for off in s.offsets.clone() {
        let Some(p) = word(geom + off).filter(|p| crate::game::plausible(*p) && *p != geom) else { continue };
        if s.objs[k].models.iter().any(|m| m.model == p) || s.objs[k].rejected.iter().any(|(r, at)| *r == p && frame.wrapping_sub(*at) < RECHECK) {
            continue;
        }
        match s.model_at(p) {
            Some((true, Some(name), vt, disp, disp_vt)) if is_grace_model(&name) => out.push(Model { off, model: p, vt, disp, disp_vt, ours: false }),
            _ => {
                let r = &mut s.objs[k].rejected;
                r.retain(|(q, _)| *q != p);
                if r.len() >= 8 {
                    r.remove(0);
                }
                r.push((p, frame));
            }
        }
    }
    out
}

/// (GRACENOSCAN) A full look of one object (IMPL_ER52's): its first `LOOK` bytes, the mound picked as `pick_models` says.
fn full(s: &mut State, k: usize) -> Vec<Model> {
    let geom = s.objs[k].geom;
    let mut raw = vec![0u8; LOOK - 0x18];
    if !read(geom + 0x18, &mut raw) {
        return Vec::new();
    }
    let mut cands = Vec::new();
    let mut facts = Vec::new();
    for (j, c) in raw.chunks_exact(8).enumerate() {
        let p = usize::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]);
        if !crate::game::plausible(p) || p == geom {
            continue;
        }
        if let Some((g, name, vt, disp, disp_vt)) = s.model_at(p) {
            cands.push((0x18 + j * 8, g, name));
            facts.push((p, vt, disp, disp_vt));
        }
    }
    pick_models(&cands).into_iter().map(|j| Model { off: cands[j].0, model: facts[j].0, vt: facts[j].1, disp: facts[j].2, disp_vt: facts[j].3, ours: false }).collect()
}

/// Once a frame in the world (PRE_PHYSICS, frame.rs; after graceflame.rs).
pub fn frame(control: &Control) {
    let Ok(mut guard) = STATE.lock() else { return };
    let s = guard.get_or_insert_with(State::default);
    let on = control.on("savepoint.kh") && control.flag("grace.hide_mound") != Some(false) && control.flag("grace.hide") != Some(false);
    if s.told_on != Some(on) {
        s.told_on = Some(on);
        erlog!("gracehide: (SV) the graces' mound {}", if on { "IS NEVER DRAWN: (GRACENOSCAN) each grace object's model (object + 0x1D0, CSGeomModelIns named AEG099_060) is read every frame, no memory search; its display bit is held off every frame" } else { "is the game's (grace.hide_mound = 0 / grace.hide = 0 / savepoint.kh = 0)" });
        if !on {
            release_locked(s);
        }
    }
    if !on {
        return;
    }
    let started = Instant::now();
    s.frame = s.frame.wrapping_add(1);
    if s.offsets.is_empty() {
        s.offsets.push(MODEL_AT);
    }
    // (a new body / a load: the world is new)
    let age = crate::game::player_age();
    if age < s.born {
        s.hurry = true;
    }
    s.born = age;
    if std::mem::take(&mut s.hurry) {
        for o in s.objs.iter_mut().filter(|o| o.models.is_empty()) {
            o.looked = None;
        }
    }
    // ---- (GRACENOSCAN) the grace objects graceflame.rs knows this frame (gone ones: their models dropped, never written)
    let now = crate::graceflame::objects();
    let before = s.objs.len();
    let mut lost = 0;
    s.objs.retain(|o| {
        let keep = now.contains(&o.geom);
        if !keep {
            lost += o.models.len() as u32;
        }
        keep
    });
    s.dropped += lost;
    if s.objs.len() != before {
        s.cursor = 0;
    }
    for g in now {
        if !s.objs.iter().any(|o| o.geom == g) {
            s.objs.push(Obj::new(g));
        }
    }
    // ---- every object: its models read back, new ones taken
    for k in 0..s.objs.len() {
        let geom = s.objs[k].geom;
        let before = s.objs[k].models.len();
        s.objs[k].models.retain(|m| same(geom, m));
        let gone = before - s.objs[k].models.len();
        if gone > 0 {
            s.dropped += gone as u32;
            if s.objs[k].models.is_empty() {
                s.objs[k].looked_with_model = false;
            }
        }
        let got = quick(s, k);
        take(s, k, got, "the per-frame read");
    }
    // ---- one full look a frame at most
    let n = s.objs.len();
    let objs = &s.objs;
    if let Some(k) = next_due(s.cursor, n, |k| look_due(!objs[k].models.is_empty(), objs[k].looked_with_model, objs[k].looked.map(|t| t.elapsed().as_secs_f32()), LOOK_EVERY)) {
        s.cursor = (k + 1) % n.max(1);
        s.looks += 1;
        let got = full(s, k);
        let o = &mut s.objs[k];
        o.looked = Some(Instant::now());
        o.looked_with_model = !o.models.is_empty() || !got.is_empty();
        for m in &got {
            if !s.offsets.contains(&m.off) && s.offsets.len() < 8 {
                s.offsets.push(m.off);
                erlog!("gracehide: (GRACENOSCAN) a grace's mound model found at object + {:#x} by a full look: that offset is read every frame from now ({:#x?})", m.off, s.offsets);
            }
        }
        take(s, k, got, "a full look");
    }
    // ---- the bits, every frame
    for o in s.objs.iter_mut() {
        for m in o.models.iter_mut() {
            if let Some(flags) = u32_at(m.disp + DISP_FLAGS) {
                if flags & 1 != 0 && poke(m.disp + DISP_FLAGS, hidden_flags(flags), u32::MAX) {
                    if m.ours {
                        s.again += 1;
                    } else {
                        s.cleared += 1;
                    }
                    m.ours = true;
                }
            }
        }
    }
    s.spent += started.elapsed();
    s.frames += 1;
    if s.second.is_none_or(|t| t.elapsed().as_secs_f32() >= 1.0) {
        let models = s.objs.iter().map(|o| o.models.len()).sum::<usize>();
        let counts = (s.objs.len(), models);
        if s.cleared + s.again + s.dropped > 0 || counts != s.told_counts {
            erlog!("gracehide: {models} grace model(s) held not drawn ({} grace object(s) watched) | this second: {} model(s) found, {} hidden for the first time, {} drawn again by the game and hidden again, {} gone (freed / reused: dropped, not written), {} full look(s) | {:.0} us a frame", s.objs.len(), s.found, s.cleared, s.again, s.dropped, s.looks, s.spent.as_secs_f64() * 1.0e6 / s.frames.max(1) as f64);
            s.told_counts = counts;
        }
        (s.cleared, s.again, s.dropped, s.found, s.looks, s.frames) = (0, 0, 0, 0, 0, 0);
        s.spent = std::time::Duration::ZERO;
        s.second = Some(Instant::now());
    }
}

/// (GRACENOSCAN) Models found for object `k` are taken (each once).
fn take(s: &mut State, k: usize, got: Vec<Model>, how: &str) {
    for m in got {
        if s.objs[k].models.iter().any(|h| h.model == m.model) {
            continue;
        }
        s.found += 1;
        if s.told_models < TELL {
            s.told_models += 1;
            let name = crate::graceflame::part_name(s.objs[k].geom).unwrap_or_else(|| "?".into());
            erlog!("gracehide: {name}: its mound model found by {how} at object + {:#x} ({:#x}, display flags {:#x}): held not drawn", m.off, m.model, u32_at(m.disp + DISP_FLAGS).unwrap_or(0));
        }
        s.objs[k].models.push(m);
    }
}

fn release_locked(s: &mut State) {
    let mut n = 0;
    for o in &s.objs {
        for m in o.models.iter().filter(|m| m.ours) {
            if same(o.geom, m) && poke(m.disp + DISP_FLAGS, 1, 1) {
                n += 1;
            }
        }
    }
    s.objs.clear();
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
        // MEASURED offset (the user's log 2026-10-27): inside the full look, inside the base object
        assert!(MODEL_AT >= 0x18 && MODEL_AT < BASE_END && MODEL_AT % 8 == 0);
    }

    /// (IMPL_ER52) The mound's model is the one named after the grace's asset; an unnamed one only when it is alone.
    #[test]
    fn the_mound_is_the_model_named_after_the_grace() {
        let named = |off: usize, n: &str| (off, true, Some(n.to_string()));
        // by name, wherever it lies (LOD models of the same asset too); another model is never taken
        assert_eq!(pick_models(&[named(0x1d0, "AEG099_060"), (0x1c8, false, Some("AEG099_090".into())), named(0x600, "aeg099_060_l1")]), vec![0, 2]);
        assert_eq!(pick_models(&[named(0x1d0, "AEG007_310")]), Vec::<usize>::new());
        // (GRACENOSCAN) a model of another class with the grace's name is not the mound
        assert_eq!(pick_models(&[(0x1d0, false, Some("AEG099_060".into()))]), Vec::<usize>::new());
        // no readable name: the one CSGeomModelIns inside the base object, when it is the only model there
        assert_eq!(pick_models(&[(0x1d0, true, None), (0x500, true, None)]), vec![0]);
        assert_eq!(pick_models(&[(0x1b0, true, None), (0x1d0, true, None)]), Vec::<usize>::new());
        assert_eq!(pick_models(&[(0x1d0, false, None)]), Vec::<usize>::new());
        assert_eq!(pick_models(&[(0x500, true, None)]), Vec::<usize>::new());
        assert_eq!(pick_models(&[(0x1b0, true, None), (0x1c0, true, Some("AEG001_000".into()))]), Vec::<usize>::new());
        assert_eq!(pick_models(&[]), Vec::<usize>::new());
    }

    /// (GRACENOSCAN) A full look: once for a new object, once more when its model appears, then every `every` s without.
    #[test]
    fn full_looks_are_few() {
        assert!(look_due(false, false, None, 20.0));
        assert!(look_due(true, false, None, 20.0));
        assert!(!look_due(false, false, Some(5.0), 20.0));
        assert!(look_due(false, false, Some(20.0), 20.0));
        // its model appeared after the look: one more look (other models of the asset it may hold), then none
        assert!(look_due(true, false, Some(1.0), 20.0));
        assert!(!look_due(true, true, Some(1.0), 20.0));
        assert!(!look_due(true, true, Some(1000.0), 20.0));
    }

    #[test]
    fn the_round_robin_wraps_and_finds_nothing_when_nothing_is_due() {
        assert_eq!(next_due(0, 0, |_| true), None);
        assert_eq!(next_due(2, 4, |k| k == 1), Some(1));
        assert_eq!(next_due(2, 4, |k| k >= 2), Some(2));
        assert_eq!(next_due(3, 4, |k| k != 3), Some(0));
        assert_eq!(next_due(1, 4, |_| false), None);
    }
}
