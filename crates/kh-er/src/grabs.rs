//! (GRABS) ELDEN RING'S GRABS ON KH CHARACTERS. The user, 2026-11-06: "the enemy in Raya Lucaria that takes you to Volcano
//! Manor needs to actually take you to Volcano Manor, it kills the tarnished in elden ring so find a way for JUST THAT ONE
//! SPECIFIC enemy in Raya Lucaria to take you to Volcano Manor. Fix the animation of the party leader getting grabbed, and
//! the camera gets stuck locked on to it when you get grabbed by it, even when it dies so fix that for all of those kinds of
//! enemies. Make sure ALL grab moves work and animate properly for every party leader". State note:
//! build/sora_tmp/weapons/GRABS/STATE.md.
//!
//! The pure rules (no game memory here; sora.rs, cam.rs, party/leader.rs, partydraw.rs and party/allybody_live.rs carry
//! them out):
//! - THE CAUGHT SEQUENCE ([`Catch`]): KH3's own caught state, `TresPlayerState_Catch` (state 61; Begin 0x141508A60 plays
//!   the state's clip 0, Update 0x14150F070: clip 0 to its end, then clip 1 looped while the pawn is still held, then the
//!   state ends). Its rows are `CatchStart` (0227), `CatchLoop` (0228) and `CatchEnd` (0213: down and get-up), and Sora
//!   (p_ex001), Donald (n_ex001) and Goofy (n_ex002) all carry the three rows under these names (their AnimSets). One
//!   sequence for every character, so every leader and every AI friend goes through the same gate.
//! - THE THROW CENSUS ([`ThrowRow`], [`family`]): every ThrowParam row in which an Elden Ring character throws the player
//!   model resolves to that sequence (`census.json`, made by GRABS/tools/census.js from the 2.7.2.0 regulation).
//! - THE ABDUCTOR ([`abduction`]): the ONE Abductor Virgin of Raya Lucaria whose grab takes the player to Volcano Manor.
//! - THE LOCK-ON CAMERA ([`lock_stale`]): the KH camera's lock never outlives the lock it was made for.

use glam::Vec3;
use kh_host::party::PartyChara;

// ---------------------------------------------------------------------------------------------------- the caught sequence

/// The rows of KH3's caught state, the same names in every party character's AnimSet (p_ex001 / n_ex001 / n_ex002).
pub const CATCH_START: &str = "CatchStart";
pub const CATCH_LOOP: &str = "CatchLoop";
pub const CATCH_END: &str = "CatchEnd";

/// Where a caught character is in KH3's caught sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// `CatchStart`, once (the state's clip 0).
    Start,
    /// `CatchLoop`, looped while the throw still holds him (the state's clip 1, substep 1: `pawn+0x968` still true).
    Hold,
    /// `CatchEnd` (0213: down and get-up), once, from the moment the throw lets go.
    End,
    /// Over: his own pose again.
    Done,
}

/// The lengths of one character's three clips (s): read from his own pack (Sora: `AnimId` data; a friend: his animator).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lengths {
    pub start: f32,
    pub hold: f32,
    pub end: f32,
}

impl Lengths {
    /// KH3 Sora's (build/gen/rust/sora_anim.rs: CatchStart 0.1 s, CatchLoop 0.4667 s, CatchEnd 0.8333 s): the fallback when
    /// a character's own clip cannot be read.
    pub const SORA: Lengths = Lengths { start: 0.1, hold: 0.466_666_7, end: 0.833_333_3 };

    fn safe(self) -> Lengths {
        let ok = |v: f32, d: f32| if v.is_finite() && v > 0.01 { v } else { d };
        Lengths { start: ok(self.start, Lengths::SORA.start), hold: ok(self.hold, Lengths::SORA.hold), end: ok(self.end, Lengths::SORA.end) }
    }
}

/// The clip a caught character shows now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shown {
    pub row: &'static str,
    pub time: f32,
    pub looping: bool,
    /// Counts the phases played (a new play restarts the clip and its blend).
    pub play: u32,
}

/// KH3'S CAUGHT SEQUENCE for one character, stepped once a frame with "the throw still holds his body".
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Catch {
    pub phase: Phase,
    pub since: f32,
    pub plays: u32,
    /// Seconds the throw held him in all (the log's measure).
    pub held_for: f32,
}

impl Catch {
    /// The throw caught him this frame.
    pub fn begin() -> Catch {
        Catch { phase: Phase::Start, since: 0.0, plays: 1, held_for: 0.0 }
    }

    fn to(&mut self, phase: Phase) {
        self.phase = phase;
        self.since = 0.0;
        self.plays += 1;
    }

    /// One frame. `held`: the throw node still holds his body (Elden Ring's throw state); its release starts `CatchEnd`
    /// at once, also in the middle of `CatchStart` (the state ends the moment the pawn is no longer held).
    pub fn step(&mut self, held: bool, dt: f32, len: Lengths) -> Option<Shown> {
        let len = len.safe();
        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        self.since += dt;
        if held {
            self.held_for += dt;
        }
        match self.phase {
            Phase::Start if !held => self.to(Phase::End),
            Phase::Start if self.since >= len.start => {
                let over = self.since - len.start;
                self.to(Phase::Hold);
                self.since = over;
            }
            Phase::Hold if !held => self.to(Phase::End),
            Phase::End if self.since >= len.end => self.to(Phase::Done),
            _ => {}
        }
        self.shown(len)
    }

    /// The clip of this moment (`None`: over).
    pub fn shown(&self, len: Lengths) -> Option<Shown> {
        let len = len.safe();
        let play = self.plays;
        match self.phase {
            Phase::Start => Some(Shown { row: CATCH_START, time: self.since.min(len.start), looping: false, play }),
            Phase::Hold => Some(Shown { row: CATCH_LOOP, time: self.since.rem_euclid(len.hold), looping: true, play }),
            Phase::End => Some(Shown { row: CATCH_END, time: self.since.min(len.end), looping: false, play }),
            Phase::Done => None,
        }
    }

    pub fn done(&self) -> bool {
        self.phase == Phase::Done
    }

    /// Still held or getting up: his own input and movement wait (KH3's state holds the pawn: Begin's `0x16D8(1)` / the
    /// movement stops).
    pub fn holds(&self) -> bool {
        self.phase != Phase::Done
    }
}

// ---------------------------------------------------------------------------------------------------- the throw census

/// One ThrowParam row in which an Elden Ring character throws the player model (`census.json`). The census is a check of
/// the data (the test below): at run time every grab takes the same sequence, whatever its row.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThrowRow {
    pub id: i32,
    /// The thrower's character model (c####).
    pub atk: i32,
    /// `throwType` (0, 20, 22 in 2.7.2.0).
    pub throw_type: u8,
    /// `escHp`: > 0 = the player can struggle free (the pad mash; `PadType`).
    pub esc_hp: u16,
    /// `throwFollowingType`: 1 = the victim is carried along the thrower's dummy poly (a lift, a carry).
    pub following: u8,
    /// The victim's animation (the player's paired clip).
    pub def_anim: i32,
}

/// What kind of grab a row is, by its own fields (descriptive: every family takes KH3's one caught sequence).
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// The victim is carried along the thrower's dummy poly (`throwFollowingType` 1): lifts, carries, lift-and-slam.
    Carried,
    /// Held in place and hit (escHp > 0: a grab the player can struggle out of).
    Held,
    /// A fixed paired throw (neither): one grab and its blow.
    Paired,
}

#[cfg(test)]
pub fn family(row: &ThrowRow) -> Family {
    if row.following == 1 {
        Family::Carried
    } else if row.esc_hp > 0 {
        Family::Held
    } else {
        Family::Paired
    }
}

/// The KH3 rows a family plays on a character (every family, every character: KH3's caught state; KH3 has no other
/// caught reaction for its party: `TresPlayerState_Bind` is the attraction rides' DRILL / YO / RALPH binds).
#[cfg(test)]
pub fn sequence(_family: Family, _chara: PartyChara) -> [&'static str; 3] {
    [CATCH_START, CATCH_LOOP, CATCH_END]
}

// ---------------------------------------------------------------------------------------------------- the Abductor

/// THE RAYA LUCARIA ABDUCTOR VIRGIN'S WARP, as Elden Ring's own data has it (2.7.2.0; read-only extraction, GRABS/STATE.md):
/// - the thrower: MSB m14_00_00_00 part `c4470_9001`, entity 14000294 (NpcParam 44701024, think 44701000), standing inside
///   the region 14002080 ("game progress: abducted by the abductor weapon"); its throw is ThrowParam 34447000 (c4470,
///   attack anim 4100, the player's paired anim 70290);
/// - m14 EVENT 14002080: the player inside region 14002080 with SpEffect 14307 (the throw's marker, 0.1 s) ->
///   SetPlayerRespawnPoint(16002080), SaveRequest, SetEventFlag(16000540, on); EVENT 14000720 (same condition) ->
///   SetEventFlag(14009300, on); the throw then kills the player and the death respawns him at 16002080;
/// - m16_00_00_00 MSB region 16002080 ("warp to the volcano prison by the abductor weapon"); m16 EVENT 16002650 on arrival:
///   flag 16000540 on -> SetEventFlag(9080, on), 16000540 off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Abduction {
    pub map: &'static str,
    pub entity: u32,
    /// The destination block (mAA_BB_CC_DD) and its player point (WarpPlayer's arguments).
    pub area: u8,
    pub block: u8,
    pub region: u8,
    pub index: u8,
    pub point: u32,
    /// The flags the two m14 events set when the abduction is recognised.
    pub flags: [u32; 2],
}

pub const RAYA_LUCARIA_ABDUCTOR: Abduction = Abduction { map: "m14_00_00_00", entity: 14_000_294, area: 16, block: 0, region: 0, index: 0, point: 16_002_080, flags: [16_000_540, 14_009_300] };

/// The abduction a grab by `entity` (its MSB event entity id) in `map` is, if any: ONLY the one Abductor Virgin of Raya
/// Lucaria (the user's "JUST THAT ONE SPECIFIC enemy"); every other grabber, the other Abductor Virgin of the Academy
/// (14000293) and Volcano Manor's own, is an ordinary grab.
pub fn abduction(map: &str, entity: u32) -> Option<Abduction> {
    let a = RAYA_LUCARIA_ABDUCTOR;
    (map == a.map && entity == a.entity).then_some(a)
}

/// The steps of the abduction once its grab is seen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AbductStep {
    /// Held: the leader is caught (KH3's sequence), the throw's blows cost nothing (the game's death is what carries him).
    Held,
    /// The throw let go (or was given up): the game's warp is asked now, the flags set.
    Warp,
}

/// The abduction's rule per frame: the warp is asked once, when the throw lets go of the body.
pub fn abduct_step(held: bool) -> AbductStep {
    if held {
        AbductStep::Held
    } else {
        AbductStep::Warp
    }
}

// ---------------------------------------------------------------------------------------------------- the lock-on camera

/// THE LOCK-ON CAMERA IS STALE: it holds `rig` while the leader's lock asks for `wanted` (`None`: no lock) or the target is
/// gone or dead (`alive` = the target's word, `None` = not in the list). CAUSE (the user's log 2026-11-06, 9950.9 s): a
/// grab seats Sora on the body with `Sora::teleport`, which makes his core afresh: his lock state is reset WITHOUT the
/// unlock event (`CameraLock { NONE }`) that alone ends the camera's lock-on, so the camera stayed "lock-on" on the
/// grabber through the grab, its death and after ("lock soft (current 0)", camera "mode lock-on").
pub fn lock_stale(rig: Option<u32>, wanted: Option<u32>, alive: Option<bool>) -> bool {
    match rig {
        None => false,
        Some(id) => wanted != Some(id) || alive != Some(true),
    }
}

/// The lock the leader's core asks the camera for: his hard lock, else an attack camera's lock (modes 3 / 6 lock the
/// camera on their target, combat/camera.rs `request`).
pub fn core_lock(locked: bool, current: u32, attack_mode: u8, attack_target: u32) -> Option<u32> {
    if locked && current != 0 {
        Some(current)
    } else if matches!(attack_mode, 3 | 6) && attack_target != 0 {
        Some(attack_target)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------------------------------- AI friends

/// (AI friends) An AI friend's body held by a throw this frame: his body's feet (KH3 space, cm) and yaw, as the game moves it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AllyHeld {
    pub chara: PartyChara,
    pub feet: Vec3,
    pub yaw: f32,
    pub held: bool,
}

static ALLY: std::sync::Mutex<Vec<AllyHeld>> = std::sync::Mutex::new(Vec::new());

/// A FRIEND LEADING is held by a throw now (sora.rs `grab_kh`: the leader body's throw node; partydraw.rs draws his
/// caught sequence from it).
static LEADER_HELD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_leader_held(held: bool) {
    LEADER_HELD.store(held, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(feature = "party")]
pub fn leader_held() -> bool {
    LEADER_HELD.load(std::sync::atomic::Ordering::Relaxed)
}

/// party/allybody_live.rs, once a frame per body: a friend's body held by a throw now (`held`), or not (his entry goes).
#[cfg(feature = "party")]
pub fn note_ally(a: AllyHeld) {
    if let Ok(mut g) = ALLY.lock() {
        g.retain(|x| x.chara != a.chara);
        if a.held {
            g.push(a);
        }
    }
}

pub fn allies() -> Vec<AllyHeld> {
    ALLY.lock().map(|g| g.clone()).unwrap_or_default()
}

pub fn ally_held(chara: PartyChara) -> bool {
    allies().iter().any(|a| a.chara == chara && a.held)
}

/// The caught sequences of the party's characters other than the leader Sora (friend leaders and AI friends), stepped by
/// whoever draws them (partydraw.rs) from [`allies`] and the leader's grab (party/leader.rs).
#[cfg(feature = "party")]
static CAUGHT: std::sync::Mutex<Vec<(PartyChara, Catch)>> = std::sync::Mutex::new(Vec::new());

/// One frame of a character's caught sequence: `held` = his body is held now. Returns the clip to show (None: his own).
#[cfg(feature = "party")]
pub fn friend_step(chara: PartyChara, held: bool, dt: f32, len: Lengths) -> Option<Shown> {
    let Ok(mut g) = CAUGHT.lock() else { return None };
    let at = g.iter().position(|(c, _)| *c == chara);
    match (at, held) {
        (None, false) => None,
        (None, true) => {
            let mut c = Catch::begin();
            let shown = c.step(true, 0.0, len);
            g.push((chara, c));
            shown
        }
        (Some(i), _) => {
            let shown = g[i].1.step(held, dt, len);
            if g[i].1.done() {
                g.remove(i);
            }
            shown
        }
    }
}

/// A friend is still caught or getting up (his pawn is held where he is).
#[cfg(feature = "party")]
pub fn friend_caught(chara: PartyChara) -> bool {
    CAUGHT.lock().map(|g| g.iter().any(|(c, k)| *c == chara && k.holds())).unwrap_or(false)
}

/// KH3 Sora's caught clips' lengths (his AnimSet rows CatchStart / CatchLoop / CatchEnd in the pack): Sora and his copies.
pub fn sora_lengths() -> Lengths {
    let len = |name: &str| kh_core::AnimId::from_name(name).map_or(0.0, |a| a.data().length);
    Lengths { start: len(CATCH_START), hold: len(CATCH_LOOP), end: len(CATCH_END) }
}

/// A caught clip as a Sora core's body request (the copies' route: their pose is made from their `Output`'s request).
pub fn catch_request(s: Shown) -> Option<kh_core::AnimRequest> {
    let anim = kh_core::AnimId::from_name(s.row)?;
    let data = anim.data();
    Some(kh_core::AnimRequest { name: anim, play_id: 0x4D00_0000 + s.play, time: s.time, active: true, weight: 1.0, looping: s.looping, blend_in: 0.1, axis: 0.0, axis_y: 0.0, root_motion: data.root_motion_enabled, notify_window: (0.0, 0.0), locomotion: None, length: data.length })
}

// ---------------------------------------------------------------------------------------------------- Mimic Sora

/// (MIMIC SORA) His body held by a throw now (party/allybody_live.rs, his slot), and his caught sequence.
#[cfg(feature = "party")]
static MIMIC: std::sync::Mutex<(Option<AllyHeld>, Option<Catch>)> = std::sync::Mutex::new((None, None));

/// party/allybody_live.rs, once a frame for Mimic Sora's body.
#[cfg(feature = "party")]
pub fn note_mimic(a: AllyHeld) {
    if let Ok(mut g) = MIMIC.lock() {
        g.0 = a.held.then_some(a);
    }
}

/// (MIMIC SORA) ONE STEP OF MIMIC SORA (spirit_mimic.rs, before his AI): his caught sequence stepped, his core seated on his
/// body while the throw holds it. True = his body is held now (his AI's word: kh-partyhost `MimicAi::set_caught`).
#[cfg(feature = "party")]
pub fn mimic_step(core: &mut kh_core::Sora, dt: f32) -> bool {
    let Ok(mut g) = MIMIC.lock() else { return false };
    let held = g.0;
    if held.is_some() && g.1.is_none() {
        g.1 = Some(Catch::begin());
        crate::erlog!("grab: (GRABS) MIMIC SORA is CAUGHT: KH3's caught sequence on his core, his AI stopped until he is up");
    }
    if let Some(c) = g.1.as_mut() {
        c.step(held.is_some(), dt, sora_lengths());
        if c.done() {
            g.1 = None;
        }
    }
    if let Some(a) = held {
        if core.status().hp > 0 && (core.output().position - a.feet).length() > 30.0 {
            core.teleport(kh_core::Pose { position: a.feet, yaw: a.yaw });
        }
    }
    held.is_some()
}

/// (MIMIC SORA) His step's output for his pose (spirit_mimic.rs `pose_step`): KH3's caught clip in his body slot while he is
/// caught or getting up, his own otherwise.
#[cfg(feature = "party")]
pub fn mimic_out(out: &kh_core::Output) -> std::borrow::Cow<'_, kh_core::Output> {
    let shown = MIMIC.lock().ok().and_then(|g| g.1.and_then(|c| c.shown(sora_lengths())));
    match shown.and_then(catch_request) {
        Some(r) => {
            let mut o = out.clone();
            (o.anim, o.anim_ticked) = (r, r);
            std::borrow::Cow::Owned(o)
        }
        None => std::borrow::Cow::Borrowed(out),
    }
}

/// (GRABS, wired by MIMIC-BOSS hotfix 3) A boss-side copy's Elden Ring body is held by a throw now (its throw node in the
/// holding / thrown states 4 / 6): the copy's caught gate (kh-partyhost `BossAi::set_caught`, the friend copy's runtime gate).
/// Inert while nobody grabs it (the copies' bodies are on the Mimic Tear's team: no Elden Ring enemy throws them).
#[cfg(all(windows, feature = "party"))]
pub fn body_held(handle: &eldenring::cs::FieldInsHandle) -> bool {
    use eldenring::cs::{CSChrThrowModule, CSThrowNode};
    let Some(chr) = crate::targets::find_chr(handle) else { return false };
    let module = crate::word!(chr.modules.throw);
    if !crate::game::plausible(module) {
        return false;
    }
    let Some(node) = crate::probe::word(module + std::mem::offset_of!(CSChrThrowModule, throw_node)).filter(|n| crate::game::plausible(*n)) else { return false };
    matches!(crate::probe::u32_at(node + std::mem::offset_of!(CSThrowNode, throw_state)), Some(4 | 6))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn run(c: &mut Catch, held: bool, seconds: f32) -> Vec<Shown> {
        (0..(seconds / DT).round() as usize).filter_map(|_| c.step(held, DT, Lengths::SORA)).collect()
    }

    #[test]
    fn the_caught_sequence_follows_the_throw_start_hold_then_the_get_up_at_the_release() {
        let mut c = Catch::begin();
        let held = run(&mut c, true, 3.0);
        assert_eq!(held.first().map(|s| s.row), Some(CATCH_START), "CatchStart first");
        let start_frames = held.iter().filter(|s| s.row == CATCH_START).count();
        assert!((5..=7).contains(&start_frames), "CatchStart plays its 0.1 s once ({start_frames} frames)");
        assert!(held.iter().skip(start_frames).all(|s| s.row == CATCH_LOOP && s.looping), "then CatchLoop, looped, while held");
        assert!(held.iter().all(|s| s.time <= Lengths::SORA.hold + 1e-4));
        assert!((c.held_for - 3.0).abs() < 0.05);
        // the release: CatchEnd at once from its start, then over after its 0.8333 s
        let end = c.step(false, DT, Lengths::SORA).expect("CatchEnd");
        assert_eq!((end.row, end.time), (CATCH_END, 0.0));
        assert!(c.holds());
        let rest = run(&mut c, false, 0.9);
        assert!(rest.iter().all(|s| s.row == CATCH_END && !s.looping));
        assert!(c.done() && !c.holds() && c.step(false, DT, Lengths::SORA).is_none());
    }

    #[test]
    fn a_release_inside_catchstart_goes_straight_to_the_get_up() {
        let mut c = Catch::begin();
        assert_eq!(c.step(true, 0.03, Lengths::SORA).map(|s| s.row), Some(CATCH_START));
        assert_eq!(c.step(false, 0.03, Lengths::SORA).map(|s| s.row), Some(CATCH_END));
    }

    #[test]
    fn unreadable_lengths_fall_back_to_kh3_soras() {
        let mut c = Catch::begin();
        let s = c.step(true, 0.05, Lengths { start: f32::NAN, hold: 0.0, end: -1.0 }).expect("shown");
        assert_eq!(s.row, CATCH_START);
        assert!(s.time <= Lengths::SORA.start);
    }

    #[cfg(feature = "party")]
    #[test]
    fn friends_have_their_own_sequence_and_are_held_until_they_are_up() {
        let len = Lengths::SORA;
        assert!(friend_step(PartyChara::Goofy, false, DT, len).is_none(), "not caught: his own pose");
        assert_eq!(friend_step(PartyChara::Goofy, true, DT, len).map(|s| s.row), Some(CATCH_START));
        assert!(friend_caught(PartyChara::Goofy) && !friend_caught(PartyChara::Donald));
        for _ in 0..30 {
            friend_step(PartyChara::Goofy, true, DT, len);
        }
        assert_eq!(friend_step(PartyChara::Goofy, true, DT, len).map(|s| s.row), Some(CATCH_LOOP));
        assert_eq!(friend_step(PartyChara::Goofy, false, DT, len).map(|s| s.row), Some(CATCH_END));
        for _ in 0..60 {
            friend_step(PartyChara::Goofy, false, DT, len);
        }
        assert!(!friend_caught(PartyChara::Goofy), "up again after CatchEnd");
        assert!(friend_step(PartyChara::Goofy, false, DT, len).is_none());
    }

    /// THE THROW CENSUS: every ThrowParam row in which an Elden Ring character throws the player (2.7.2.0) resolves to KH3's
    /// caught sequence for every leader, and every row of that sequence is in each leader's own AnimSet as the pack has it.
    #[test]
    fn every_enemy_throw_of_the_census_resolves_to_kh3s_caught_sequence_for_every_leader() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let text = std::fs::read_to_string(root.join("build/sora_tmp/weapons/GRABS/census.json")).expect("GRABS/census.json (tools/census.js)");
        let json: serde_json::Value = serde_json::from_str(&text).expect("census json");
        let rows: Vec<ThrowRow> = json["rows"]
            .as_array()
            .expect("rows")
            .iter()
            .map(|r| ThrowRow { id: r["id"].as_i64().unwrap_or(0) as i32, atk: r["atk"].as_i64().unwrap_or(0) as i32, throw_type: r["throwType"].as_u64().unwrap_or(0) as u8, esc_hp: r["escHp"].as_u64().unwrap_or(0) as u16, following: r["following"].as_u64().unwrap_or(0) as u8, def_anim: r["defAnim"].as_i64().unwrap_or(0) as i32 })
            .collect();
        assert_eq!(rows.len(), 189, "2.7.2.0: 189 enemy throws on the player");
        assert!(rows.iter().all(|r| r.atk > 0));
        let mut families = [0usize; 3];
        for r in &rows {
            let f = family(r);
            families[f as usize] += 1;
            for chara in [PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy] {
                assert_eq!(sequence(f, chara), [CATCH_START, CATCH_LOOP, CATCH_END], "row {} ({:?}) for {:?}", r.id, f, chara);
            }
        }
        assert!(families.iter().all(|n| *n > 0), "every family is used: {families:?}");
        let abductor = rows.iter().find(|r| r.atk == 4470).expect("the Abductor Virgin's throw");
        assert_eq!((abductor.id, abductor.def_anim), (34_447_000, 70_290), "the user's log: body animation 70290 in the grab");
        // the rows in each character's own pack
        for name in [CATCH_START, CATCH_LOOP, CATCH_END] {
            assert!(kh_core::AnimId::from_name(name).is_some(), "Sora's AnimId {name}");
        }
        let party = std::fs::read_to_string(root.join("sheets/port/party-assets.json")).expect("party-assets sheet");
        for npc in ["n_ex001", "n_ex002"] {
            for (file, name) in [("0227", CATCH_START), ("0228", CATCH_LOOP), ("0213", CATCH_END)] {
                assert!(party.lines().any(|l| l.contains(&format!("\"{npc}/anims/{file}.glb\"")) && l.contains(&format!("\"{name}\""))), "{npc} {name} = {file}");
            }
        }
    }

    #[test]
    fn only_the_one_raya_lucaria_abductor_takes_the_party_to_volcano_manor() {
        let a = abduction("m14_00_00_00", 14_000_294).expect("the Raya Lucaria Abductor Virgin");
        assert_eq!((a.area, a.block, a.region, a.index, a.point), (16, 0, 0, 0, 16_002_080));
        assert_eq!(a.flags, [16_000_540, 14_009_300]);
        assert!(abduction("m14_00_00_00", 14_000_293).is_none(), "the Academy's other Abductor Virgin");
        for e in [16_000_460, 16_000_461, 16_000_462, 16_000_860, 16_000_861] {
            assert!(abduction("m16_00_00_00", e).is_none(), "Volcano Manor's Abductor Virgins (and the boss pair) {e}");
        }
        assert!(abduction("m16_00_00_00", 14_000_294).is_none(), "the entity id only in its own map");
        assert!(abduction("m14_00_00_00", 0).is_none());
        assert_eq!(abduct_step(true), AbductStep::Held);
        assert_eq!(abduct_step(false), AbductStep::Warp);
    }

    #[test]
    fn the_lock_on_camera_lets_go_of_a_grabber_the_lock_no_longer_holds_or_that_died() {
        // the user's case: the camera locked on the grabber (171), the core's lock reset by the seat (no lock)
        assert!(lock_stale(Some(171), core_lock(false, 171, 0, 0), Some(true)), "a soft target is no lock");
        // it died while the lock still named it
        assert!(lock_stale(Some(171), Some(171), Some(false)));
        assert!(lock_stale(Some(171), Some(171), None), "gone from the list");
        // a live lock the leader holds stays
        assert!(!lock_stale(Some(171), core_lock(true, 171, 0, 0), Some(true)));
        // an attack camera's lock (modes 3 / 6) stays while its attack runs
        assert!(!lock_stale(Some(9), core_lock(false, 0, 6, 9), Some(true)));
        assert!(lock_stale(Some(9), core_lock(false, 0, 4, 9), Some(true)));
        assert!(!lock_stale(None, None, None));
    }
}
