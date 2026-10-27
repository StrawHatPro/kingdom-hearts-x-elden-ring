//! (IMPL_ER17) THE GOLDEN FLAME OF A SITE OF GRACE, NOT DRAWN (the user: KH3's save points REPLACE the sites of grace; the
//! guidance ray stays).
//!
//! WHAT DRAWS IT (MEASURED, IMPL_ER17_STATE.md): not the grace's character (c1000) and not an event effect. Every grace has
//! a map object `AEG099_060` (in the open world it lives in the large far-view block, named `m60_XX_YY_00-AEG099_060_9000`)
//! whose own effect slot (`geom + 0x348` -> `GeomMultiSfxSlot`: `+0x18` the sfx id, `+0x20` the dummy poly 201, `+0x30` ->
//! `FXHGSfxCtrl`) plays FXR 1011 (discovered: the flame and the ring on the ground) or 1010 (not discovered yet). The slot's
//! control points (`+0x18`) at the effect's record in the effect system's pool: `+0x00` the FXR id, `+0x0C..+0x14` scale,
//! `+0x18..+0x24` colour and alpha multipliers (all 1.0), `+0x38` the effect system. With those seven floats at 0 nothing of
//! the effect is drawn (SEEN; a faint warm light on the ground stays); the game does not write them again.
//! THE GUIDANCE RAY is another effect (sfx 6400, made by the map's event script on the helper object `AEG099_090`): untouched.
//!
//! Every read goes through `probe::read`; a write is a plain write of a float that was read an instant before, into a record
//! whose owner chain (object by class and name -> slot by class -> control by class -> record by its system pointer and id)
//! was checked in the same call. The objects are looked for a few at a time (`BUDGET` a frame).
//!
//! (IMPL_ER52, the user 2026-10-13: "remove the grace bit on the ground for every grace as well as it's collision") THE
//! GRACE'S MOUND = the model of that same map object `AEG099_060`. UNTESTED IN GAME; NO GAME FUNCTION IS CALLED:
//! - NOT DRAWN (`grace.hide_mound`, default 1): gracehide.rs's (GRACENOSCAN, the user 2026-10-27: "what about the drops in
//!   frame when I was running around?"): it reads the model of every grace object found here ([`objects`]; MEASURED at
//!   object + 0x1D0) every frame and clears bit 0 of its display flags. The look made here gave up three frames after an
//!   object was found, before the game had made its model (`CSGeomModelInsDelayCreator`): moved there and made patient.
//!   The object itself stays: its effect slot, its event entity, the grace's character (c1000) and with them the game's
//!   prompt / talk of the grace are untouched. In cutscenes the scene's own copies are cutgrace.rs's.
//! - NO COLLISION FOR SORA (`grace.mound_collision`, default 0): his casts (world.rs `ErWorld::cast`) skip every contact on
//!   a body whose owner (`CSPhysIns.owner`, owner type 6) is one of these objects ([`mound_body`]). THE GAME'S OWN BODY
//!   STAYS IN THE HAVOK WORLD (no route to disable one body is known without reading the executable): the hidden
//!   Tarnished body, the game's camera and other characters still meet it. `grace.mound_collision = 1` = as before.

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering::Relaxed};
use std::sync::{Mutex, RwLock};

use crate::control::Control;
use crate::erlog;
use crate::probe::{class_of, read, u32_at, word};

/// The grace's map object (MSB part names contain it).
const ASSET: &str = "AEG099_060";
const GEOM_SLOT: usize = 0x348;
const GEOM_PART: usize = 0x48;
const SLOT_SFX: usize = 0x18;
const SLOT_CTRL: usize = 0x30;
const CTRL_RECORD: usize = 0x18;
const RECORD_FLOATS: usize = 0x0C;
const FLOATS: usize = 7;
/// Map objects looked at per frame while searching (three guarded reads each).
const BUDGET: usize = 60;
/// (IMPL_ER21) ... while a save point waits for its grace's object (a round of some 18 000 objects in a dozen frames).
const BURST: usize = 1500;
/// (IMPL_ER21) Search rounds after which every address is looked at by name again.
const FRESH_ROUNDS: u32 = 24;

/// The sfx ids of the grace's own light (sheet rows `grace.flame.sfx.undiscovered` / `.discovered`).
static SFX: [AtomicI32; 2] = [AtomicI32::new(1010), AtomicI32::new(1011)];

pub fn set_ids(ids: [i32; 2]) {
    SFX[0].store(ids[0], Relaxed);
    SFX[1].store(ids[1], Relaxed);
}

/// (IMPL_ER52) Every grace object known this frame (addresses; published for world.rs's casts) and whether Sora's casts
/// pass through their bodies now.
static MOUNDS: RwLock<Vec<usize>> = RwLock::new(Vec::new());
static PASSABLE: AtomicBool = AtomicBool::new(false);

/// (GRACENOSCAN) The grace objects known (addresses, checked by class and name this frame): gracehide.rs reads their models.
pub fn objects() -> Vec<usize> {
    MOUNDS.read().map(|m| m.clone()).unwrap_or_default()
}

/// (IMPL_ER52) The body's owner (`CastHit::owner_ptr` of owner type 6) is a grace's map object (its mound).
pub fn mound_body(owner: usize) -> bool {
    owner != 0 && MOUNDS.read().is_ok_and(|m| m.contains(&owner))
}

/// (IMPL_ER52) Sora's casts pass through the graces' mounds (`grace.mound_collision` is not 1 and the save points are on).
pub fn mounds_passable() -> bool {
    PASSABLE.load(Relaxed)
}

static VT_GEOM: [AtomicUsize; 2] = [AtomicUsize::new(0), AtomicUsize::new(0)];
static VT_SLOT: AtomicUsize = AtomicUsize::new(0);
static VT_CTRL: AtomicUsize = AtomicUsize::new(0);

fn is(p: usize, known: &AtomicUsize, class: &str) -> bool {
    let Some(vt) = word(p) else { return false };
    let have = known.load(Relaxed);
    if have != 0 {
        return vt == have;
    }
    if class_of(p).is_some_and(|c| c == class) {
        known.store(vt, Relaxed);
        return true;
    }
    false
}

fn is_geom(p: usize) -> bool {
    is(p, &VT_GEOM[0], "CSWorldGeomDynamicIns@CS") || is(p, &VT_GEOM[1], "CSWorldGeomStaticIns@CS")
}

/// The object's MSB part name (its first 40 characters).
pub(crate) fn part_name(geom: usize) -> Option<String> {
    let name = word(geom + GEOM_PART).and_then(word)?;
    let mut b = [0u8; 80];
    if !read(name, &mut b) {
        return None;
    }
    Some(b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|u| *u != 0).map(|u| if u < 128 { u as u8 as char } else { '?' }).collect())
}

/// The object's MSB part name contains the grace's asset.
fn is_grace(geom: usize) -> bool {
    part_name(geom).is_some_and(|text| text.contains(ASSET))
}

/// (IMPL_ER21) The object's effect slot plays one of the grace's own lights (1010 / 1011): THE GAME SHOWS THIS GRACE.
/// (A slot without a running effect record still counts: the slot is what the game makes when it enables the object.)
/// (IMPL_ER33) With what was read: (the slot pointer, its sfx id (0 = no slot of that class), the answer).
fn flame_reading(geom: usize) -> (usize, u32, bool) {
    let Some(slot) = word(geom + GEOM_SLOT).filter(|s| *s != 0) else { return (0, 0, false) };
    if !is(slot, &VT_SLOT, "GeomMultiSfxSlot@CS") {
        return (slot, 0, false);
    }
    let sfx = u32_at(slot + SLOT_SFX).unwrap_or(0);
    (slot, sfx, SFX.iter().any(|s| s.load(Relaxed) as u32 == sfx))
}

/// (IMPL_ER33) THE FLAME TEST MADE STEADY. MEASURED in the user's log of 2026-10-13 (er/logs/kh-er.log 469.6 - 476.8 s):
/// from 1.7 s into the game's OWN touch of a grace (the body's animation 68000: the moment the game itself registers
/// the grace and changes its object's light from 1010 to 1011) and for 7.1 s, the test of that one object read "flame"
/// and "no flame" on alternating frames; the save point was hidden and shown 60 times a second, and a hidden save point
/// offers no Save. The game shows a grace it has shown a moment ago: an object counts as showing its flame until the
/// test has failed for `HOLD` frames in a row; a grace that never showed one (a boss's before the boss is defeated) is
/// hidden from the first frame as before.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Steady {
    /// Frames in a row without the flame.
    missing: u16,
    shown: bool,
    last: bool,
    /// Changes of the raw reading since the count was last taken (for the log).
    flips: u16,
    /// The last two different raw readings (slot pointer, sfx id).
    seen: [(usize, u32); 2],
}

impl Steady {
    /// Frames the flame may be missing before the grace counts as hidden (1.5 s at 60 frames a second).
    pub const HOLD: u16 = 90;

    pub fn update(&mut self, raw: bool) -> bool {
        if raw != self.last {
            self.flips = self.flips.saturating_add(1);
            self.last = raw;
        }
        if raw {
            self.missing = 0;
            self.shown = true;
        } else if self.shown {
            self.missing = self.missing.saturating_add(1);
            if self.missing >= Self::HOLD {
                self.shown = false;
            }
        }
        self.shown
    }

    fn note(&mut self, reading: (usize, u32)) {
        if self.seen[0] != reading {
            self.seen[1] = self.seen[0];
            self.seen[0] = reading;
        }
    }
}

/// (IMPL_ER21) WHERE A GRACE BELONGS, as its ids say it: (map area, x, z). An open-world tile for the ten-digit entity
/// ids (10XXYYnnnn = m60_XX_YY, 20XXYYnnnn = m61_XX_YY), else the block of a dungeon (AABBnnnn = mAA_BB).
pub fn home_of_entity(entity: u32) -> (u32, u32, u32) {
    if entity >= 1_000_000_000 {
        (if entity / 100_000_000 == 20 { 61 } else { 60 }, entity / 1_000_000 % 100, entity / 10_000 % 100)
    } else {
        (entity / 1_000_000, entity / 10_000 % 100, 0)
    }
}

/// (IMPL_ER21) The same for a grace's map object: the tile its name is prefixed with (`m60_42_37_00-AEG099_060_9000`: an
/// object kept in a far-view block), else its own block (id word: area, x, z, level).
pub fn home_of_object(name: &str, block: u32) -> (u32, u32, u32) {
    let prefix: Vec<u32> = name.strip_prefix('m').and_then(|rest| rest.split_once('-')).map(|(tile, _)| tile.split('_').filter_map(|n| n.parse().ok()).collect()).unwrap_or_default();
    let (area, x, z) = if prefix.len() == 4 { (prefix[0], prefix[1], prefix[2]) } else { (block >> 24, block >> 16 & 255, block >> 8 & 255) };
    if (60..=61).contains(&area) { (area, x, z) } else { (area, x, 0) }
}

/// (IMPL_ER21) What the game shows of a site of grace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Presence {
    /// Its map object plays its flame: the grace is there for the player.
    Present,
    /// Its map object is known and plays nothing: the game hides this grace (a boss's, before the boss is defeated).
    Hidden,
    /// No map object of it is known (yet).
    Unknown,
}

/// One grace object as read this frame.
#[derive(Clone, Debug)]
struct Seen {
    home: (u32, u32, u32),
    /// Its own block's id and its place in that block (the MSB part's), when the name has no tile prefix.
    block: u32,
    local: Option<[f32; 3]>,
    flame: bool,
    /// The MSB part's entity id (the part's entity data, + 0x60 of the part as in the file), 0 = not read.
    entity: u32,
}

/// (IMPL_ER21) THE DECISION, pure: `objects` = the grace objects known; `entity` = the grace; `chr` = its character's
/// place and `anchor` = the player's (block id, block-local place, physics place), all of this frame; `alone` = no
/// other grace of the game's table shares its home. An object WITH THE GRACE'S ENTITY ID decides first; else an object AT the grace (within 3.5 m; possible when it lies in the
/// player's block or in an open-world tile of his area) decides; without one, the objects of its home decide when the
/// grace is alone there.
fn decide(objects: &[Seen], entity: u32, chr: [f32; 3], anchor: (u32, [f32; 3], [f32; 3]), alone: bool) -> Presence {
    let home = home_of_entity(entity);
    let (here, local, physics) = anchor;
    // (the grace's entity id IS its map object's: BonfireWarpParam names the object the script registers)
    let own: Vec<bool> = objects.iter().filter(|o| o.entity == entity && entity != 0).map(|o| o.flame).collect();
    if !own.is_empty() {
        return if own.iter().any(|f| *f) { Presence::Present } else { Presence::Hidden };
    }
    let (mut at, mut of_home): (Vec<bool>, Vec<bool>) = (Vec::new(), Vec::new());
    for o in objects.iter().filter(|o| o.home == home) {
        of_home.push(o.flame);
        let Some(l) = o.local else { continue };
        let (a, gx, gz) = ((o.block >> 24) as i32, (o.block >> 16 & 255) as i32, (o.block >> 8 & 255) as i32);
        let (ha, hx, hz) = ((here >> 24) as i32, (here >> 16 & 255) as i32, (here >> 8 & 255) as i32);
        let tiles = (60..=61).contains(&a) && a == ha && o.block & 255 == 0 && here & 255 == 0;
        if o.block != here && !tiles {
            continue;
        }
        let (dx, dz) = if tiles { ((gx - hx) as f32 * 256.0, (gz - hz) as f32 * 256.0) } else { (0.0, 0.0) };
        let p = [physics[0] + l[0] + dx - local[0], physics[1] + l[1] - local[1], physics[2] + l[2] + dz - local[2]];
        if ((p[0] - chr[0]).powi(2) + (p[2] - chr[2]).powi(2)).sqrt() <= 3.5 && (p[1] - chr[1]).abs() <= 6.0 {
            at.push(o.flame);
        }
    }
    let by = if !at.is_empty() { at } else if alone { of_home } else { Vec::new() };
    match (by.is_empty(), by.iter().any(|f| *f)) {
        (true, _) => Presence::Unknown,
        (false, true) => Presence::Present,
        (false, false) => Presence::Hidden,
    }
}

/// The record of the effect the object's slot plays now, when the whole chain is as measured: (record, sfx id).
fn record_of(geom: usize, system: usize) -> Option<(usize, u32)> {
    let slot = word(geom + GEOM_SLOT).filter(|s| *s != 0)?;
    if !is(slot, &VT_SLOT, "GeomMultiSfxSlot@CS") {
        return None;
    }
    let sfx = u32_at(slot + SLOT_SFX)?;
    if !SFX.iter().any(|s| s.load(Relaxed) as u32 == sfx) {
        return None;
    }
    let ctrl = word(slot + SLOT_CTRL).filter(|c| *c != 0)?;
    if !is(ctrl, &VT_CTRL, "FXHGSfxCtrl@FX4HG") {
        return None;
    }
    let record = word(ctrl + CTRL_RECORD).filter(|r| *r != 0)?;
    (crate::probe::fx_record(record, system)? == sfx).then_some((record, sfx))
}

fn floats(record: usize) -> Option<[f32; FLOATS]> {
    crate::probe::floats::<FLOATS>(record + RECORD_FLOATS)
}

#[derive(Default)]
struct State {
    /// The grace objects found (addresses; checked again at every use). (IMPL_ER21) EVERY one, with or without its flame.
    graces: Vec<usize>,
    /// (IMPL_ER21) What they are this frame (`presence` answers from it).
    seen: Vec<Seen>,
    /// (IMPL_ER33) The steady flame test of each known object (by address; dropped with the object).
    steady: std::collections::HashMap<usize, Steady>,
    /// (IMPL_ER21) Addresses already looked at by name and found to be something else (cleared every `FRESH_ROUNDS`
    /// rounds and when the save points ask: an address can be given to another object).
    other: std::collections::HashSet<usize>,
    fresh_at: u32,
    /// (IMPL_ER21) Frames the search runs with `BURST` objects a frame (a grace came into reach and is not known).
    burst: u32,
    /// The search: the blocks' object vectors (begin, end) of this round and where it stands.
    vectors: Vec<(usize, usize)>,
    at: (usize, usize),
    rounds: u32,
    /// The records we zeroed: (record, sfx id, what its floats were).
    hidden: Vec<(usize, u32, [f32; FLOATS])>,
    told: Option<bool>,
    /// (IMPL_ER52) The plan as said (the mounds themselves are gracehide.rs's: GRACENOSCAN).
    plan_told: Option<crate::savepoint_rules::MoundPlan>,
    writes: u32,
    spent: std::time::Duration,
    looked: u32,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

/// The object vectors of every loaded block (the SDK's map of blocks; the vectors' words are read guarded).
fn block_vectors() -> Vec<(usize, usize)> {
    use fromsoftware_shared::FromStatic;
    let mut out = Vec::new();
    // SAFETY: game thread; the singleton lives as long as the game; the tree is the game's own, walked read-only.
    let Ok(man) = (unsafe { eldenring::cs::CSWorldGeomMan::instance() }) else { return out };
    for pair in man.blocks.iter().take(512) {
        if !crate::game::plausible(crate::word!(pair.second)) {
            continue;
        }
        let at = &pair.second.geom_ins_vector as *const _ as usize;
        // (a DLVector: allocator, begin, end, capacity)
        let (Some(begin), Some(end)) = (word(at + 8), word(at + 16)) else { continue };
        if begin != 0 && end > begin && (end - begin) % 8 == 0 && end - begin <= 8 * 100_000 {
            out.push((begin, end));
        }
    }
    out
}

/// ONCE A FRAME in the world (`ChrIns_PrePhysics`). `savepoint.flame = 1` keeps the game's flame; without the save points
/// (`savepoint.kh = 0`) nothing is touched.
pub fn frame(control: &Control, tick: bool) {
    let Ok(mut guard) = STATE.lock() else { return };
    let state = guard.get_or_insert_with(State::default);
    let on = control.on("savepoint.kh");
    let hide = on && control.flag("savepoint.flame") != Some(true);
    if state.told != Some(hide) {
        state.told = Some(hide);
        erlog!("graceflame: the sites of grace's own flame is {}", if hide { "NOT DRAWN (savepoint.flame = 1 keeps it): the effect of the grace's map object is held at zero size and colour; the guidance ray is another effect and stays" } else { "left to the game" });
    }
    // (IMPL_ER52) the grace's mound: not drawn / no collision for Sora
    let plan = crate::savepoint_rules::mound_plan(on, control.flag("grace.hide_mound"), control.flag("grace.mound_collision"));
    if state.plan_told != Some(plan) {
        state.plan_told = Some(plan);
        erlog!("grace: the graces' mound (the model of their map object {ASSET}) is {} (grace.hide_mound); its collision is {} (grace.mound_collision)", if plan.hide { "NOT DRAWN" } else { "drawn by the game" }, if plan.collide { "the game's: Sora meets it" } else { "OFF FOR SORA: his casts pass through its body (the game's own body stays: its camera and the hidden Tarnished still meet it)" });
    }
    PASSABLE.store(!plan.collide, Relaxed);
    let started = std::time::Instant::now();
    let Some(system) = crate::probe::fx_system() else { return };
    if !hide {
        restore(state, system);
    }
    // (IMPL_ER21) the objects are looked for whenever the save points are on: whether a grace is PRESENT is read from them
    if !on {
        state.seen.clear();
        if let Ok(mut m) = MOUNDS.write() {
            m.clear();
        }
        return;
    }
    // ---- the search, a few objects a frame
    if state.at.0 >= state.vectors.len() {
        state.vectors = block_vectors();
        state.at = (0, 0);
        state.rounds += 1;
        if state.rounds.saturating_sub(state.fresh_at) >= FRESH_ROUNDS || state.other.len() > 400_000 {
            state.other.clear();
            state.fresh_at = state.rounds;
        }
    }
    let mut left = if state.burst > 0 { BURST } else { BUDGET };
    state.burst = state.burst.saturating_sub(1);
    while left > 0 && state.at.0 < state.vectors.len() {
        let (begin, end) = state.vectors[state.at.0];
        let n = (end - begin) / 8;
        let take = left.min(n.saturating_sub(state.at.1));
        if take == 0 {
            state.at = (state.at.0 + 1, 0);
            continue;
        }
        let mut raw = vec![0u8; take * 8];
        if read(begin + state.at.1 * 8, &mut raw) {
            for c in raw.chunks_exact(8) {
                let geom = usize::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]);
                // (IMPL_ER21) every object is looked at by name once (a hidden grace's object has no effect slot)
                if geom == 0 || state.other.contains(&geom) || state.graces.contains(&geom) {
                    continue;
                }
                if is_geom(geom) && is_grace(geom) && state.graces.len() < 256 {
                    state.graces.push(geom);
                    crate::gracehide::hurry(); // (SV) its model is looked for (GRACENOSCAN: gracehide.rs reads it from the object)
                } else {
                    state.other.insert(geom);
                }
            }
        }
        state.at.1 += take;
        left -= take;
    }
    // ---- every known grace: what it is now; its effect's floats held at 0
    let mut k = 0;
    state.seen.clear();
    while k < state.graces.len() {
        let geom = state.graces[k];
        let name = if is_geom(geom) { part_name(geom).filter(|n| n.contains(ASSET)) } else { None };
        let Some(name) = name else {
            state.graces.swap_remove(k);
            state.steady.remove(&geom);
            continue;
        };
        k += 1;
        let block = word(geom + 0x10).and_then(u32_at).unwrap_or(0);
        let local = if name.starts_with('m') { None } else { word(geom + GEOM_PART).and_then(|part| crate::probe::floats::<3>(part + 0x20)).filter(|f| f.iter().all(|v| v.is_finite())) };
        // (the entity data of the part: a pointer once the game has loaded the file; an offset from the part in the file)
        let entity = word(geom + GEOM_PART).and_then(|part| word(part + 0x60).map(|e| if e != 0 && e < 0x10_0000 { part + e } else { e })).filter(|e| *e != 0).and_then(u32_at).unwrap_or(0);
        // (IMPL_ER33) the steady test: an alternating reading no longer hides and shows the save point every frame
        let (slot, sfx, raw) = flame_reading(geom);
        let steady = state.steady.entry(geom).or_default();
        steady.note((slot, sfx));
        let flame = steady.update(raw);
        if tick && steady.flips >= 4 {
            erlog!("graceflame: the grace object {name} (entity {entity}): ITS FLAME TEST ALTERNATED {} time(s) this second: the readings (slot pointer, sfx id) {:x?} and {:x?}; held as {} (IMPL_ER33: {} frame(s) without the flame hide it)", steady.flips, steady.seen[0], steady.seen[1], if flame { "SHOWN" } else { "hidden" }, Steady::HOLD);
        }
        if tick {
            steady.flips = 0;
        }
        state.seen.push(Seen { home: home_of_object(&name, block), block, local, flame, entity });
        if !hide {
            continue;
        }
        let Some((record, sfx)) = record_of(geom, system) else { continue };
        let Some(now) = floats(record) else { continue };
        if now.iter().any(|f| *f != 0.0) {
            if !state.hidden.iter().any(|h| h.0 == record && h.1 == sfx) {
                if state.hidden.len() >= 256 {
                    state.hidden.remove(0);
                }
                state.hidden.push((record, sfx, now));
            }
            for f in 0..FLOATS {
                // SAFETY: the record was read through the system an instant ago (its system pointer and id checked in
                // `record_of`, its floats just read); game thread.
                unsafe { std::ptr::write_volatile((record + RECORD_FLOATS + 4 * f) as *mut f32, 0.0) };
            }
            state.writes += 1;
        }
    }
    if let Ok(mut m) = MOUNDS.write() {
        m.clone_from(&state.graces); // (IMPL_ER52) for world.rs's casts
    }
    crate::cutgrace::frame(control, hide, system, [SFX[0].load(Relaxed), SFX[1].load(Relaxed)], tick); // (IMPL_ER33) a cutscene's own grace
    state.spent += started.elapsed();
    state.looked += 1;
    if tick {
        erlog!("graceflame: {} grace object(s) known ({} with their flame = shown by the game, {} without = hidden), {} effect(s) zeroed this second ({} held so far) | {:.0} us a frame | search round {} ({} block(s))", state.graces.len(), state.seen.iter().filter(|o| o.flame).count(), state.seen.iter().filter(|o| !o.flame).count(), state.writes, state.hidden.len(), state.spent.as_secs_f64() * 1.0e6 / state.looked.max(1) as f64, state.rounds, state.vectors.len());
        state.writes = 0;
        state.spent = std::time::Duration::ZERO;
        state.looked = 0;
    }
}

/// (IMPL_ER21) WHAT THE GAME SHOWS OF A GRACE (savepoint.rs asks every frame; game thread): see `decide`. The second
/// value: the search rounds finished so far (an Unknown that outlives a whole round stays unknown).
pub fn presence(entity: u32, chr: [f32; 3], anchor: (u32, [f32; 3], [f32; 3]), alone: bool) -> (Presence, u32) {
    let Ok(guard) = STATE.lock() else { return (Presence::Unknown, 0) };
    let Some(state) = guard.as_ref() else { return (Presence::Unknown, 0) };
    (decide(&state.seen, entity, chr, anchor, alone), state.rounds)
}

/// (IMPL_ER21) A grace came into reach whose object is not known: the search hurries for a moment, from a clean slate.
pub fn hurry() {
    if let Ok(mut guard) = STATE.lock() {
        if let Some(state) = guard.as_mut() {
            if state.burst == 0 {
                state.other.clear();
                state.fresh_at = state.rounds;
            }
            state.burst = 40;
        }
    }
}

/// The effects we zeroed get their floats back, when they are still the same effects.
fn restore(state: &mut State, system: usize) {
    for (record, sfx, was) in state.hidden.drain(..) {
        if crate::probe::fx_record(record, system) == Some(sfx) && floats(record).is_some_and(|now| now.iter().all(|f| *f == 0.0)) {
            for (f, v) in was.iter().enumerate() {
                // SAFETY: as in `frame`: checked and read an instant ago.
                unsafe { std::ptr::write_volatile((record + RECORD_FLOATS + 4 * f) as *mut f32, *v) };
            }
        }
    }
}

/// The payload goes.
pub fn release() {
    if let Ok(mut guard) = STATE.lock() {
        if let (Some(state), Some(system)) = (guard.as_mut(), crate::probe::fx_system()) {
            restore(state, system);
        }
        *guard = None;
        if let Ok(mut m) = MOUNDS.write() {
            m.clear();
        }
        PASSABLE.store(false, Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (IMPL_ER33) THE USER'S LOG: the reading alternates every frame for seven seconds: the grace stays shown.
    #[test]
    fn an_alternating_flame_reading_does_not_hide_the_save_point() {
        let mut s = Steady::default();
        // a boss's grace before the boss is defeated: never a flame, hidden from the first frame
        assert!((0..600).all(|_| !s.update(false)));
        // the boss dies: shown at once
        assert!(s.update(true));
        // the reading alternates frame by frame (7.1 s at 60 frames a second), then settles on the flame
        assert!((0..426).all(|k| s.update(k % 2 == 0)));
        assert!(s.flips > 400);
        assert!(s.update(true));
        // short gaps (a slot made anew: a few frames) do not hide it either
        assert!((0..(Steady::HOLD - 1)).all(|_| s.update(false)));
        assert!(s.update(true));
        // the game really takes the grace away: hidden after HOLD frames, and it stays hidden
        let frames = (0..1000).take_while(|_| s.update(false)).count();
        assert_eq!(frames as u16, Steady::HOLD - 1);
        assert!((0..600).all(|_| !s.update(false)));
        // the two last readings are kept for the log line
        s.note((0x10, 1011));
        s.note((0x10, 1011));
        s.note((0, 0));
        assert_eq!(s.seen, [(0, 0), (0x10, 1011)]);
    }

    fn object(name: &str, block: u32, local: Option<[f32; 3]>, flame: bool) -> Seen {
        Seen { home: home_of_object(name, block), block, local, flame, entity: 0 }
    }

    #[test]
    fn homes() {
        assert_eq!(home_of_entity(1042371950), (60, 42, 37));
        assert_eq!(home_of_entity(2046401950), (61, 46, 40));
        assert_eq!(home_of_entity(10001951), (10, 0, 0));
        assert_eq!(home_of_entity(31151950), (31, 15, 0));
        // the far-view block's object carries its tile in its name; a dungeon's object has its own block
        assert_eq!(home_of_object("m60_42_37_00-AEG099_060_9000", 0x3C0A0902), (60, 42, 37));
        assert_eq!(home_of_object("AEG099_060_9001", 0x0A000000), (10, 0, 0));
        assert_eq!(home_of_object("AEG099_060_9000", 0x3C2A2500), (60, 42, 37));
    }

    /// STORMVEIL (m10_00_00_00), the user's session: Castleward Tunnel's grace 10001952 is shown, Margit's 10001951 is
    /// hidden until he is defeated; both objects lie in the player's block.
    #[test]
    fn a_boss_arena_grace_is_hidden_until_the_game_shows_it() {
        let block = 0x0A00_0000;
        let anchor = (block, [100.0, 50.0, 200.0], [-9.0, 10.0, -25.0]);
        // (block-local places; the characters stand at physics = local - (109, 40, 225))
        let chr = |l: [f32; 3]| [l[0] - 109.0, l[1] - 40.0, l[2] - 225.0];
        let (margit, tunnel) = ([84.0, 58.0, 220.0], [150.0, 48.0, 240.0]);
        let mut objects = vec![object("AEG099_060_9000", block, Some(tunnel), true), object("AEG099_060_9001", block, Some(margit), false)];
        assert_eq!(decide(&objects, 10001952, chr(tunnel), anchor, false), Presence::Present);
        assert_eq!(decide(&objects, 10001951, chr(margit), anchor, false), Presence::Hidden);
        // the boss is defeated: the game enables the object and gives it its flame
        objects[1].flame = true;
        assert_eq!(decide(&objects, 10001951, chr(margit), anchor, false), Presence::Present);
        // by its entity id alone (no place, another grace in the block)
        let by_id = vec![Seen { entity: 10001951, local: None, ..objects[1].clone() }, Seen { entity: 10001952, local: None, ..objects[0].clone() }];
        assert_eq!(decide(&by_id, 10001951, [0.0; 3], anchor, false), Presence::Present);
        assert_eq!(decide(&[Seen { flame: false, ..by_id[0].clone() }, by_id[1].clone()], 10001951, [0.0; 3], anchor, false), Presence::Hidden);
        // a grace whose object is not known: unknown, never hidden
        assert_eq!(decide(&objects[..1], 10001951, chr(margit), anchor, false), Presence::Unknown);
        assert_eq!(decide(&[], 10001952, chr(tunnel), anchor, true), Presence::Unknown);
    }

    /// THE OPEN WORLD: the grace's object lives in a far-view block under its tile's name, without a usable place.
    #[test]
    fn an_open_world_grace_is_matched_by_its_tile() {
        let anchor = (0x3C2A_2500, [10.0, 0.0, 10.0], [0.0, 0.0, 0.0]);
        let objects = vec![object("m60_42_37_00-AEG099_060_9000", 0x3C0A_0902, None, true), object("m60_42_38_00-AEG099_060_9000", 0x3C0A_0902, None, false)];
        assert_eq!(decide(&objects, 1042371950, [5.0, 0.0, 5.0], anchor, true), Presence::Present);
        assert_eq!(decide(&objects, 1042381950, [5.0, 0.0, 261.0], anchor, true), Presence::Hidden);
        // two graces in one tile and no place to tell them apart: unknown
        assert_eq!(decide(&objects, 1042371950, [5.0, 0.0, 5.0], anchor, false), Presence::Unknown);
        assert_eq!(decide(&objects, 1043371950, [5.0, 0.0, 5.0], anchor, true), Presence::Unknown);
    }
}
