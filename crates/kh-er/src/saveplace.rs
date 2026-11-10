//! (CONT-LADDER) CONTINUE AND LOAD PUT THE LEADER AT THE SAVE POINT OF THE SAVE THEY LOAD. The user, 2026-11-08: "continue
//! is still placing you at the last ladder you interacted with instead of your last save or load", and the rule:
//! "regardless of what Elden Ring does, continue needs to only load the last save or load of a kingdom hearts save".
//!
//! THE CAUSE (eldenring.exe 2.7.2.0, read in the file; the SDK names the fields): the place a save holds is not where the
//! player stands but `PlayerIns::safe_block_pos` / `safe_block_id` (+0x6D4 / +0x6E4, "Last safe for saving block position
//! ... Used for save files"). Its one writer is 0x14065B630, called once a frame from 0x140B03060 only while the main
//! player's ChrIns +0x1C6 bit 0x40 is set, his HP is above 0 and two global gates are off; 0x140651140 (the player's
//! constructor) starts it empty. In the user's log (2026-11-08) every save made after a ladder leap loaded him at the
//! BOTTOM of that ladder (the place where the game's own ladder state ran, StartBottom, before LADDER2 let the body go),
//! and every save made before the first ladder of a session loaded him at its save point: once the game's ladder is cut
//! short, the gate stays shut and the game's "safe" place stays at the ladder until the next load. (Which code sets the
//! bit is in the exe's protected code: not read. The rest is read; the gate's link to the ladder is from the log.)
//!
//! WHAT IS DONE (both halves, every leader: the place is the game's body, which stands where the leader stands):
//! 1. AT EVERY KH SAVE (slots.rs `save_to_slot`: the save point's Save, KH3's Save screen, `dbg.saveto`) the body's own
//!    place (`block_position` / `current_block_id`: where the save point stood him) is copied into the game's "safe" place
//!    before the game is asked to save, so the game's own save holds the save point (only the game's own fields, with
//!    the values its own writer would have put there) - and OUR RECORD of that place is written beside Sora's file:
//!    `<slot key>.place` (block, block-local position, facing, the grace of the save point and its place, the game's
//!    play time). It begins with `slot<N>_`, so a delete moves it with the slot.
//! 2. AT EVERY KH LOAD (the save point's Load, the title's load and Continue, KH3's game-over Continue: every start of a
//!    character from its file, save.rs `on_start`) the loaded slot's record is armed; once the game's step runs again
//!    the body's place is compared with it ([`at_place`]). The game's place stands only when it IS the record's place;
//!    otherwise THE GAME'S OWN WARP to the record's grace is asked for (what EMEVD WarpPlayer writes, as testaid.rs
//!    `test.warp` and the Abductor's warp do), once, and the arrival after it is checked and logged.
//!    Elden Ring's stored place never decides where he is after a load: the record does.
//!
//! WHAT CANNOT HOLD (said in the log, never guessed): a slot with no record (saved before this build, or never saved at
//! a save point) - nothing is known but the game's place, so the game's place stands; a record without a grace (no
//! grace was within reach at the save) - the warp has no target.
//!
//! KEYS: `saveplace` (1; 0 = neither half), `saveplace.stamp` (1; 0 = the game's "safe" place is not written at a save),
//! `saveplace.warp` (1; 0 = a mismatch is only logged).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::erlog;

/// The leader stands at the record's place when the body is this near the recorded spot (m).
pub const MATCH_M: f32 = 3.0;
/// ... or this near the recorded save point's grace (m): the game's warp puts him at the grace's player point.
pub const GRACE_M: f32 = 6.0;
/// The save point he saved at: the nearest grace this near the body at the save (m).
pub const GRACE_REACH: f32 = 10.0;
/// After the load the game's step runs this long before the place is compared (s): the game's last moves are done.
pub const SETTLE_S: f32 = 0.5;
/// Without the step's hook: the player object this many frames old is compared.
pub const SETTLE_FRAMES: u32 = 120;
/// The arrival of the game's warp is awaited this long (s) before it is called missed.
pub const WARP_WAIT_S: f32 = 90.0;
/// A record is from an EARLIER character of the same name and slot when it is this far ahead of the loaded play time (ms).
pub const AHEAD_MS: u64 = 120_000;

/// OUR RECORD OF A KH SAVE'S PLACE.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    /// The block the body stood in (`BlockId` as i32).
    pub block: i32,
    /// Block-local position (m).
    pub pos: [f32; 3],
    pub yaw: f32,
    /// The save point's grace (bonfire entity id; 0 = none in reach).
    pub grace: u32,
    /// That grace's place, block-local in `block`'s frame.
    pub grace_pos: Option<[f32; 3]>,
    /// Elden Ring's play time at the save (ms).
    pub play_ms: u64,
}

impl Place {
    /// The record's text (pure).
    pub fn to_text(self) -> String {
        let [x, y, z] = self.pos;
        let mut t = format!("kh-er place of this slot's last KH save (CONT-LADDER: Continue and Load put the leader here)\nblock={}\npos={x} {y} {z}\nyaw={}\ngrace={}\n", self.block, self.yaw, self.grace);
        if let Some([gx, gy, gz]) = self.grace_pos {
            t.push_str(&format!("grace_pos={gx} {gy} {gz}\n"));
        }
        t.push_str(&format!("play_ms={}\n", self.play_ms));
        t
    }

    /// A record read back (pure; `None`: not a whole record).
    pub fn from_text(text: &str) -> Option<Place> {
        let field = |name: &str| text.lines().find_map(|l| l.strip_prefix(name).and_then(|r| r.strip_prefix('='))).map(str::trim);
        let vec3 = |s: &str| -> Option<[f32; 3]> {
            let v: Vec<f32> = s.split_whitespace().map(|p| p.parse().ok()).collect::<Option<Vec<f32>>>()?;
            (v.len() == 3 && v.iter().all(|c| c.is_finite())).then(|| [v[0], v[1], v[2]])
        };
        Some(Place {
            block: field("block")?.parse().ok()?,
            pos: vec3(field("pos")?)?,
            yaw: field("yaw").and_then(|s| s.parse().ok()).filter(|y: &f32| y.is_finite()).unwrap_or(0.0),
            grace: field("grace").and_then(|s| s.parse().ok()).unwrap_or(0),
            grace_pos: field("grace_pos").and_then(vec3),
            play_ms: field("play_ms").and_then(|s| s.parse().ok()).unwrap_or(0),
        })
    }
}

/// `BlockId` parts of an i32 (area, block, region, index).
pub fn parts(block: i32) -> (u8, u8, u8, u8) {
    let b = block as u32;
    ((b >> 24) as u8, (b >> 16) as u8, (b >> 8) as u8, b as u8)
}

/// The open world's areas (the SDK's `BlockId::is_overworld`): 256 m tiles that share one plane.
fn overworld(area: u8) -> bool {
    (50..89).contains(&area)
}

/// `pos` (block-local in `block`) in the frame of block `base`; `None`: the two blocks share no frame (other maps). Pure.
pub fn in_frame_of(base: i32, block: i32, pos: [f32; 3]) -> Option<[f32; 3]> {
    if base == block {
        return Some(pos);
    }
    let (a0, x0, z0, _) = parts(base);
    let (a1, x1, z1, _) = parts(block);
    (a0 == a1 && overworld(a0)).then(|| [pos[0] + 256.0 * (f32::from(x1) - f32::from(x0)), pos[1], pos[2] + 256.0 * (f32::from(z1) - f32::from(z0))])
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// THE BODY STANDS AT THE RECORD'S PLACE: in a block that shares the record's frame, within [`MATCH_M`] of the spot or
/// [`GRACE_M`] of its save point's grace. Pure.
pub fn at_place(rec: &Place, block: i32, pos: [f32; 3]) -> bool {
    let Some(p) = in_frame_of(rec.block, block, pos).filter(|p| p.iter().all(|c| c.is_finite())) else { return false };
    distance(p, rec.pos) <= MATCH_M || rec.grace_pos.is_some_and(|g| distance(p, g) <= GRACE_M)
}

/// The record applies to the loaded character (pure): not one from an earlier character of the name and slot (a record
/// whose play time is more than [`AHEAD_MS`] ahead of the loaded play time).
pub fn record_applies(rec: &Place, loaded_play_ms: u64) -> bool {
    rec.play_ms <= loaded_play_ms.saturating_add(AHEAD_MS)
}

/// The nearest grace within [`GRACE_REACH`] of `pos` (pure): `rows` = (entity, block-local place in the body's frame).
pub fn nearest_grace(pos: [f32; 3], rows: &[(u32, [f32; 3])]) -> Option<(u32, [f32; 3])> {
    rows.iter().filter(|(_, g)| distance(pos, *g) <= GRACE_REACH).min_by(|a, b| distance(pos, a.1).total_cmp(&distance(pos, b.1))).copied()
}

/// (testaid.rs `grace_player_point`'s rule) The player point of a grace: its entity id - 970 for ids ending 1950..1999.
pub fn grace_player_point(bonfire: u32) -> Option<u32> {
    (1950..2000).contains(&(bonfire % 10_000)).then(|| bonfire - 970)
}

// ------------------------------------------------------------------------------------------ the arrival (pure)

/// What the host sees once a frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Seen {
    /// The player object (its address; 0 = none).
    pub player: usize,
    /// Frames it has been complete for (game.rs `player_age`).
    pub age: u32,
    pub block: i32,
    pub pos: [f32; 3],
    /// The game's step runs (`None`: its hook is not there).
    pub step: Option<bool>,
    pub dt: f32,
}

/// What the arrival asks for this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Act {
    Wait,
    /// The game's place is the record's: nothing to do.
    Matched,
    /// The game's place is NOT the record's: the game's warp to the record's grace is to be asked for.
    Warp(u32),
    /// Not at the record's place and no grace to warp to.
    NoGrace,
    /// After the warp: at the record's place.
    Reached,
    /// After the warp: still not there (or the warp never came): left as it is, never warped twice.
    Missed,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    /// Waiting for the loaded player, its step running for `ran` s.
    Loading { ran: f32 },
    /// The warp is asked for: waiting for a new player object (not `from`), then its settling.
    Warped { from: usize, from_age: u32, waited: f32, ran: f32, new: bool },
    Done,
}

/// ONE KH LOAD'S ARRIVAL: armed with the loaded slot's record at the character's start from its file.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arrival {
    pub rec: Place,
    phase: Phase,
}

impl Arrival {
    pub fn new(rec: Place) -> Arrival {
        Arrival { rec, phase: Phase::Loading { ran: 0.0 } }
    }

    pub fn done(&self) -> bool {
        self.phase == Phase::Done
    }

    /// The player has settled: the step has run [`SETTLE_S`] (or, unhooked, the object is [`SETTLE_FRAMES`] old).
    fn settled(ran: f32, s: &Seen) -> bool {
        match s.step {
            Some(_) => ran >= SETTLE_S,
            None => s.age >= SETTLE_FRAMES,
        }
    }

    /// ONE FRAME. `warp_on`: a mismatch may be answered by the game's warp.
    pub fn frame(&mut self, s: Seen, warp_on: bool) -> Act {
        let dt = if s.dt.is_finite() { s.dt.clamp(0.0, 0.5) } else { 0.0 };
        let valid = s.player != 0 && s.pos.iter().all(|c| c.is_finite());
        let running = s.step == Some(true);
        match self.phase {
            Phase::Done => Act::Wait,
            Phase::Loading { ran } => {
                if !valid {
                    self.phase = Phase::Loading { ran: 0.0 };
                    return Act::Wait;
                }
                let ran = ran + if running { dt } else { 0.0 };
                self.phase = Phase::Loading { ran };
                if !Self::settled(ran, &s) {
                    return Act::Wait;
                }
                if at_place(&self.rec, s.block, s.pos) {
                    self.phase = Phase::Done;
                    return Act::Matched;
                }
                if !warp_on || grace_player_point(self.rec.grace).is_none() {
                    self.phase = Phase::Done;
                    return Act::NoGrace;
                }
                self.phase = Phase::Warped { from: s.player, from_age: s.age, waited: 0.0, ran: 0.0, new: false };
                Act::Warp(self.rec.grace)
            }
            Phase::Warped { from, from_age, waited, ran, new } => {
                let waited = waited + dt;
                // a new player object: another address, or the same one counted from 0 again
                let new = new || (valid && (s.player != from || s.age < from_age));
                let ran = if new && valid { ran + if running { dt } else { 0.0 } } else { 0.0 };
                self.phase = Phase::Warped { from, from_age: if new { from_age } else { s.age.max(from_age) }, waited, ran, new };
                if new && valid && Self::settled(ran, &s) {
                    self.phase = Phase::Done;
                    return if at_place(&self.rec, s.block, s.pos) { Act::Reached } else { Act::Missed };
                }
                if waited >= WARP_WAIT_S {
                    self.phase = Phase::Done;
                    return Act::Missed;
                }
                Act::Wait
            }
        }
    }
}

// ------------------------------------------------------------------------------------------ the game's side

/// A KH save's place captured when the save was asked for: (the target slot's record path, the place).
static PENDING: Mutex<Option<(PathBuf, Place)>> = Mutex::new(None);
/// The KH load being followed (armed at the character's start from its file) and its slot (the log).
static ARRIVAL: Mutex<Option<(Arrival, i32)>> = Mutex::new(None);
/// The switches of the latest frame (`saveplace`, `saveplace.stamp`, `saveplace.warp`).
static KEYS: Mutex<(bool, bool, bool)> = Mutex::new((true, true, true));
/// The frame clock of the arrival.
static LAST: Mutex<Option<std::time::Instant>> = Mutex::new(None);

/// The record of a character key (`slot<N>_...`) in `dir`.
pub fn record_path(dir: &Path, key: &str) -> PathBuf {
    dir.join(format!("{key}.place"))
}

/// (frame.rs, once a frame) The keys.
pub fn keys(control: &crate::control::Control) {
    let on_unless_off = |key: &str| control.flag(key) != Some(false);
    let on = on_unless_off("saveplace");
    if let Ok(mut k) = KEYS.lock() {
        *k = (on, on && on_unless_off("saveplace.stamp"), on && on_unless_off("saveplace.warp"));
    }
}

fn keys_now() -> (bool, bool, bool) {
    KEYS.lock().map(|k| *k).unwrap_or((true, true, true))
}

fn play_ms() -> u64 {
    use eldenring::cs::GameDataMan;
    use fromsoftware_shared::FromStatic;
    // SAFETY: game thread; the singleton lives as long as the game; a plain read.
    unsafe { GameDataMan::instance() }.map_or(0, |d| u64::from(d.play_time))
}

/// The graces near the body (entity, block-local place in its frame): every BonfireWarpParam row of its area whose block
/// is its own or (in the open world) a neighbouring tile (savepoint.rs `graces_within`'s reading of the game's table).
fn graces_near(player: &eldenring::cs::PlayerIns) -> Vec<(u32, [f32; 3])> {
    use eldenring::cs::{BonfireWarpParam, SoloParamRepository};
    use fromsoftware_shared::FromStatic;
    let mut out = Vec::new();
    // SAFETY: game thread; the singleton lives as long as the game.
    let Some(repo) = (unsafe { SoloParamRepository::instance() }).ok().filter(|r| crate::game::param_loaded::<BonfireWarpParam>(r)) else { return out };
    let block: i32 = player.current_block_id.into();
    let (area, gx, gz, _) = parts(block);
    for entry in repo.bonfire_warps.iter().take(8192) {
        let Some(row) = repo.get_by_bonfire_warp_param_by_entity_id(entry.bonfire_entity_id) else { continue };
        if row.area_no() != area {
            continue;
        }
        let (dx, dz) = (i32::from(row.grid_x_no()) - i32::from(gx), i32::from(row.grid_z_no()) - i32::from(gz));
        if (overworld(area) && (dx.abs() > 1 || dz.abs() > 1)) || (!overworld(area) && (dx != 0 || dz != 0)) {
            continue;
        }
        let local = [row.pos_x() + 256.0 * dx as f32, row.pos_y(), row.pos_z() + 256.0 * dz as f32];
        if local.iter().all(|c| c.is_finite()) {
            out.push((entry.bonfire_entity_id, local));
        }
    }
    out
}

/// (slots.rs `save_to_slot`, before the game is asked to save into `slot`) THE KH SAVE'S PLACE: the body's own place is
/// captured for the target slot's record and copied into the game's "safe" place (the field its save holds). False:
/// nothing captured (no world, `saveplace` off, Sora has no save data).
pub fn before_save(slot: i32) -> bool {
    let (on, stamp, _) = keys_now();
    let Some((dir, key)) = crate::save::slot_key(slot).filter(|_| on) else { return false };
    let Some(player) = crate::game::player_mut() else { return false };
    let block: i32 = player.current_block_id.into();
    let bp = player.block_position;
    let pos = [bp.x, bp.y, bp.z];
    if block == -1 || !pos.iter().all(|c| c.is_finite()) {
        erlog!("saveplace: the body's place is not known (block {block}): no record for this save");
        return false;
    }
    let grace = nearest_grace(pos, &graces_near(player));
    let place = Place { block, pos, yaw: if bp.yaw.is_finite() { bp.yaw } else { 0.0 }, grace: grace.map_or(0, |g| g.0), grace_pos: grace.map(|g| g.1), play_ms: play_ms() };
    let safe_block: i32 = player.safe_block_id.into();
    let safe = player.safe_block_pos;
    let moved = in_frame_of(block, safe_block, [safe.x, safe.y, safe.z]).map(|s| distance(s, pos));
    if stamp {
        // the game's own two fields, with the values its own writer (0x14065B630) puts there: the save holds them
        player.safe_block_pos = bp;
        player.safe_block_id = player.current_block_id;
    }
    erlog!(
        "saveplace: THE KH SAVE'S PLACE (slot {slot}): {} ({:.2}, {:.2}, {:.2}), grace {}; the game's own saved place was {} {}",
        eldenring::cs::BlockId::from(block),
        pos[0],
        pos[1],
        pos[2],
        if place.grace == 0 { "NONE within reach".to_string() } else { place.grace.to_string() },
        match moved {
            Some(d) if d <= MATCH_M => format!("the same ({d:.1} m)"),
            Some(d) => format!("{d:.1} m away"),
            None => format!("in another map ({})", eldenring::cs::BlockId::from(safe_block)),
        },
        if stamp { "-> set to the body's place for this save" } else { "(saveplace.stamp = 0: left)" }
    );
    if let Ok(mut p) = PENDING.lock() {
        *p = Some((record_path(&dir, &key), place));
    }
    true
}

/// (slots.rs) The game's save into the slot is written (`done`) or nothing came of it: the record is written or dropped.
pub fn after_save(done: bool) {
    let Some((path, place)) = PENDING.lock().ok().and_then(|mut p| p.take()) else { return };
    if !done {
        erlog!("saveplace: the save was not made: its place record is dropped");
        return;
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match std::fs::write(&path, place.to_text()) {
        Ok(()) => erlog!("saveplace: the place record of this save is written: Continue and Load of this slot put the leader there"),
        Err(e) => erlog!("saveplace: the place record could NOT be written ({e}): a load of this slot keeps the game's own place"),
    }
}

/// (save.rs `on_start`, a character started FROM ITS FILE: a KH load of slot `slot`) The slot's record is armed.
pub fn on_load(dir: &Path, key: &str, slot: i32, loaded_play_ms: u64) {
    let (on, _, _) = keys_now();
    let mut arrival = ARRIVAL.lock().unwrap_or_else(|e| e.into_inner());
    *arrival = None;
    if !on {
        return;
    }
    let rec = std::fs::read_to_string(record_path(dir, key)).ok().and_then(|t| Place::from_text(&t));
    match rec {
        None => erlog!("saveplace: (CANNOT HOLD) slot {slot} has NO PLACE RECORD (saved before this build, or never at a save point): the game's own place stands for this load"),
        Some(rec) if !record_applies(&rec, loaded_play_ms) => erlog!("saveplace: slot {slot}'s place record is of an EARLIER character ({:.1} min ahead of this one's play time): not used", (rec.play_ms - loaded_play_ms) as f64 / 60_000.0),
        Some(rec) => {
            erlog!("saveplace: A KH LOAD of slot {slot}: its save's place {} ({:.2}, {:.2}, {:.2}), grace {}: he is put there if the game's place is not it", eldenring::cs::BlockId::from(rec.block), rec.pos[0], rec.pos[1], rec.pos[2], rec.grace);
            *arrival = Some((Arrival::new(rec), slot));
        }
    }
}

/// THE GAME'S WARP to grace `bonfire` (what EMEVD WarpPlayer writes into GameMan; testaid.rs `warp_to_grace`).
fn warp_to(bonfire: u32) -> Result<String, &'static str> {
    use eldenring::cs::{BonfireWarpParam, SoloParamRepository};
    use fromsoftware_shared::FromStatic;
    // SAFETY: game thread; the singleton lives as long as the game.
    let repo = (unsafe { SoloParamRepository::instance() }).ok().filter(|r| crate::game::param_loaded::<BonfireWarpParam>(r)).ok_or("no param repository")?;
    let row = repo.get_by_bonfire_warp_param_by_entity_id(bonfire).ok_or("the grace has no BonfireWarpParam row")?;
    let point = grace_player_point(bonfire).ok_or("the grace's player point is not known")?;
    let target = eldenring::cs::BlockId::from_parts(row.area_no(), row.grid_x_no(), row.grid_z_no(), 0);
    // SAFETY: game thread; the singleton lives as long as the game; three plain fields of it (the SDK's layout).
    let gm = unsafe { eldenring::cs::GameMan::instance_mut() }.map_err(|_| "no GameMan")?;
    gm.initial_area_entity_id = point;
    gm.move_map_target = target;
    gm.warp_requested = true;
    Ok(format!("{target} point {point}"))
}

/// (slots.rs `frame`, FRAME_BEGIN) The armed arrival, followed to its end.
pub fn frame() {
    let dt = {
        let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
        let now = std::time::Instant::now();
        let dt = last.map_or(0.0, |t| now.duration_since(t).as_secs_f32());
        *last = Some(now);
        dt
    };
    let mut guard = ARRIVAL.lock().unwrap_or_else(|e| e.into_inner());
    let Some((arrival, slot)) = guard.as_mut() else { return };
    let (_, _, warp_on) = keys_now();
    let seen = match crate::game::player_mut() {
        Some(p) => Seen {
            player: p as *const eldenring::cs::PlayerIns as usize,
            age: crate::game::player_age(),
            block: p.current_block_id.into(),
            pos: [p.block_position.x, p.block_position.y, p.block_position.z],
            step: crate::pause::hooked().then(crate::pause::available),
            dt,
        },
        None => Seen { dt, ..Seen::default() },
    };
    let where_ = || format!("{} ({:.2}, {:.2}, {:.2})", eldenring::cs::BlockId::from(seen.block), seen.pos[0], seen.pos[1], seen.pos[2]);
    let rec = arrival.rec;
    match arrival.frame(seen, warp_on) {
        Act::Wait => {}
        Act::Matched => erlog!("saveplace: slot {slot}: the game put him AT HIS SAVE'S PLACE ({}): it stands", where_()),
        Act::Warp(grace) => match warp_to(grace) {
            Ok(target) => erlog!("saveplace: slot {slot}: THE GAME PUT HIM ELSEWHERE ({}; his save's place {} ({:.2}, {:.2}, {:.2})): THE GAME'S WARP to the save point's grace {grace} ({target}) is asked", where_(), eldenring::cs::BlockId::from(rec.block), rec.pos[0], rec.pos[1], rec.pos[2]),
            Err(why) => {
                erlog!("saveplace: slot {slot}: (CANNOT HOLD) the game put him elsewhere ({}) and the warp to grace {grace} could not be asked ({why}): left where the game put him", where_());
                *guard = None;
            }
        },
        Act::NoGrace => erlog!("saveplace: slot {slot}: (CANNOT HOLD) the game put him elsewhere ({}; his save's place {} ({:.2}, {:.2}, {:.2})) and {}: left where the game put him", where_(), eldenring::cs::BlockId::from(rec.block), rec.pos[0], rec.pos[1], rec.pos[2], if warp_on { "the record has no save point's grace to warp to" } else { "saveplace.warp = 0" }),
        Act::Reached => erlog!("saveplace: slot {slot}: after the game's warp he IS AT HIS SAVE POINT ({})", where_()),
        Act::Missed => erlog!("saveplace: slot {slot}: (CANNOT HOLD) after the game's warp he is still not at his save point ({}): left there, not warped again", where_()),
    }
    if guard.as_ref().is_some_and(|(a, _)| a.done()) {
        *guard = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;
    /// m10_00_00_00 (a legacy dungeon) and two open-world tiles m60_42_36_00 / m60_43_36_00.
    const CASTLE: i32 = 0x0A00_0000;
    const TILE: i32 = 0x3C2A_2400;
    const EAST: i32 = 0x3C2B_2400;

    fn rec() -> Place {
        Place { block: CASTLE, pos: [-6.13, 2.44, 24.82], yaw: 0.5, grace: 10_001_955, grace_pos: Some([-6.26, 2.44, 24.96]), play_ms: 1_000_000 }
    }

    fn seen(player: usize, age: u32, block: i32, pos: [f32; 3], step: Option<bool>) -> Seen {
        Seen { player, age, block, pos, step, dt: DT }
    }

    #[test]
    fn the_record_reads_back_what_was_written() {
        let r = rec();
        assert_eq!(Place::from_text(&r.to_text()), Some(r));
        let none = Place { grace: 0, grace_pos: None, ..r };
        assert_eq!(Place::from_text(&none.to_text()), Some(none));
        assert_eq!(Place::from_text("block=1\n"), None);
        assert_eq!(Place::from_text("block=1\npos=1 2\n"), None);
        assert_eq!(Place::from_text("block=x\npos=1 2 3\n"), None);
        assert_eq!(Place::from_text("block=1\npos=1 NaN 3\n"), None);
    }

    #[test]
    fn places_compare_only_in_a_shared_frame() {
        assert_eq!(parts(TILE), (60, 42, 36, 0));
        assert_eq!(in_frame_of(CASTLE, CASTLE, [1.0, 2.0, 3.0]), Some([1.0, 2.0, 3.0]));
        // the neighbouring open-world tile is 256 m east
        assert_eq!(in_frame_of(TILE, EAST, [1.0, 2.0, 3.0]), Some([257.0, 2.0, 3.0]));
        // another map shares no frame
        assert_eq!(in_frame_of(CASTLE, TILE, [1.0, 2.0, 3.0]), None);
        assert_eq!(in_frame_of(CASTLE, CASTLE + 1, [1.0, 2.0, 3.0]), None);
    }

    /// THE USER'S CASE (2026-11-08): saved at grace 10001955, loaded at the bottom of the ladder 19.5 m away in the same
    /// castle block: not his place. At the spot, or at the grace's player point, it is.
    #[test]
    fn the_ladder_bottom_is_not_the_save_point() {
        let r = rec();
        assert!(!at_place(&r, CASTLE, [-4.8, 3.23, 5.34]));
        assert!(at_place(&r, CASTLE, [-6.0, 2.44, 24.5]));
        assert!(at_place(&r, CASTLE, [-7.5, 2.44, 21.0]), "the grace's player point, 4 m from the spot");
        assert!(!at_place(&r, TILE, [-6.13, 2.44, 24.82]), "the same numbers in another map");
        assert!(!at_place(&r, CASTLE, [f32::NAN, 0.0, 0.0]));
        let open = Place { block: TILE, pos: [255.0, 10.0, 5.0], grace_pos: None, ..r };
        assert!(at_place(&open, EAST, [-0.5, 10.0, 5.0]), "across a tile edge");
    }

    #[test]
    fn a_record_ahead_of_the_loaded_play_time_is_another_characters() {
        let r = rec();
        assert!(record_applies(&r, 1_000_000));
        assert!(record_applies(&r, 1_005_000));
        assert!(record_applies(&r, 990_000), "the save's moment, a few seconds either way");
        assert!(!record_applies(&r, 1_000_000 - AHEAD_MS - 1));
    }

    #[test]
    fn the_save_point_is_the_nearest_grace_in_reach() {
        let rows = [(1, [0.0, 0.0, 30.0]), (2, [1.0, 0.0, 0.0]), (3, [0.0, 0.0, 3.0])];
        assert_eq!(nearest_grace([0.0, 0.0, 0.5], &rows), Some((2, [1.0, 0.0, 0.0])));
        assert_eq!(nearest_grace([0.0, 0.0, 19.0], &rows), None);
        assert_eq!(grace_player_point(10_001_955), Some(10_000_985));
        assert_eq!(grace_player_point(1_042_361_951), Some(1_042_360_981));
        assert_eq!(grace_player_point(12_345), None);
    }

    /// A load that puts him where he saved: no warp, whatever the loading screen did before.
    #[test]
    fn the_games_place_stands_only_when_it_is_the_records() {
        let mut a = Arrival::new(rec());
        for k in 0..300 {
            assert_eq!(a.frame(seen(0x1000, k, CASTLE, [50.0, 0.0, 0.0], Some(false)), true), Act::Wait, "under the loading screen nothing is decided");
        }
        let mut acts = (0..60).map(|k| a.frame(seen(0x1000, 300 + k, CASTLE, [-6.1, 2.44, 24.8], Some(true)), true)).filter(|x| *x != Act::Wait);
        assert_eq!(acts.next(), Some(Act::Matched));
        assert!(a.done());
    }

    /// THE LADDER CASE: the game puts him at the ladder: the game's warp to the save point's grace is asked once, the
    /// arrival after it is at the grace.
    #[test]
    fn a_wrong_place_asks_the_games_warp_once_and_checks_the_arrival() {
        let mut a = Arrival::new(rec());
        let ladder = [-4.8, 3.23, 5.34];
        let acts: Vec<Act> = (0..60).map(|k| a.frame(seen(0x1000, 100 + k, CASTLE, ladder, Some(true)), true)).filter(|x| *x != Act::Wait).collect();
        assert_eq!(acts, vec![Act::Warp(10_001_955)]);
        // the old player stays a while, still at the ladder: no second warp, no verdict
        assert!((0..200).all(|k| a.frame(seen(0x1000, 200 + k, CASTLE, ladder, Some(true)), true) == Act::Wait));
        // gone under the warp's loading screen, then a new player at the grace's player point
        assert!((0..100).all(|_| a.frame(Seen { dt: DT, ..Seen::default() }, true) == Act::Wait));
        let acts: Vec<Act> = (0..60).map(|k| a.frame(seen(0x2000, 30 + k, CASTLE, [-7.0, 2.44, 23.5], Some(true)), true)).filter(|x| *x != Act::Wait).collect();
        assert_eq!(acts, vec![Act::Reached]);
        assert!(a.done());
    }

    /// The new player object may come back at the same address: its age counted from 0 again tells it.
    #[test]
    fn a_new_player_at_the_same_address_is_known_by_its_age() {
        let mut a = Arrival::new(rec());
        let acts: Vec<Act> = (0..40).map(|k| a.frame(seen(0x1000, 500 + k, TILE, [0.0, 0.0, 0.0], Some(true)), true)).filter(|x| *x != Act::Wait).collect();
        assert_eq!(acts, vec![Act::Warp(10_001_955)]);
        let acts: Vec<Act> = (0..40).map(|k| a.frame(seen(0x1000, 30 + k, CASTLE, [0.0, 0.0, 0.0], Some(true)), true)).filter(|x| *x != Act::Wait).collect();
        assert_eq!(acts, vec![Act::Missed], "a new object, not at the save point: missed, never warped twice");
    }

    #[test]
    fn a_warp_that_never_comes_is_missed_and_no_grace_is_said() {
        let mut a = Arrival::new(rec());
        assert!((0..40).any(|k| matches!(a.frame(seen(0x1000, 100 + k, TILE, [0.0; 3], Some(true)), true), Act::Warp(_))));
        let n = (0..10_000).take_while(|k| a.frame(seen(0x1000, 200 + k, TILE, [0.0; 3], Some(true)), true) == Act::Wait).count();
        assert!((5390..=5401).contains(&n), "{n}");
        let mut a = Arrival::new(Place { grace: 0, grace_pos: None, ..rec() });
        assert!((0..40).any(|k| a.frame(seen(0x1000, 100 + k, TILE, [0.0; 3], Some(true)), true) == Act::NoGrace));
        let mut a = Arrival::new(rec());
        assert!((0..40).any(|k| a.frame(seen(0x1000, 100 + k, TILE, [0.0; 3], Some(true)), false) == Act::NoGrace), "saveplace.warp = 0");
    }

    /// Without the step's hook the object's age decides when the place is compared.
    #[test]
    fn without_the_steps_hook_the_age_decides() {
        let mut a = Arrival::new(rec());
        assert_eq!(a.frame(seen(0x1000, SETTLE_FRAMES - 1, CASTLE, [-6.1, 2.44, 24.8], None), true), Act::Wait);
        assert_eq!(a.frame(seen(0x1000, SETTLE_FRAMES, CASTLE, [-6.1, 2.44, 24.8], None), true), Act::Matched);
    }
}
