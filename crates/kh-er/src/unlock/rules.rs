//! (ER party S8) THE UNLOCK RULES, game-free: who is unlocked and how that is saved, the staged fight's phases, the
//! talk scene's phases, Sora's stage pilot, the celebration's clock. The game half is
//! `unlock/host.rs`; every rule here is unit-tested (the module's tests).
//!
//! THE USER'S RULE (2026-10-15, U2, verbatim in build/sora_tmp/party/ER_PHASE_NOTES.md) with the coordinator's defaults:
//! whoever you start as is unlocked and not waiting at his own spot; Goofy is unlocked by beating Leonine Misbegotten,
//! Donald by talking to him at the Limgrave Finger Reader, Sora (when not the starter) by beating Margit; an unlocked
//! character joins the party (all three fit: up to 3).

use glam::{Vec2, Vec3};
use kh_host::party::PartyChara;
use serde_json::{json, Value};

/// The three in their fixed order.
pub const ALL: [PartyChara; 3] = [PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy];

// ------------------------------------------------------------------------------------------------------ who is unlocked

/// WHO IS UNLOCKED, saved in the save's party record (`"unlocked": ["sora", ...]`, `"starter": "sora"`: the keys of
/// kh_host::formation, the same field S6's record writes).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unlocks {
    pub starter: PartyChara,
    unlocked: [bool; 3],
}

impl Default for Unlocks {
    fn default() -> Unlocks {
        Unlocks::legacy()
    }
}

impl Unlocks {
    /// A new game: the chosen starter alone.
    pub fn new_game(starter: PartyChara) -> Unlocks {
        let mut u = Unlocks { starter, unlocked: [false; 3] };
        u.unlocked[starter as usize] = true;
        u
    }

    /// A character whose save has no unlock record (made before the rule): Sora was the only one played.
    pub fn legacy() -> Unlocks {
        Unlocks::new_game(PartyChara::Sora)
    }

    /// From a save's party record (`None` when the record has no list: [`Unlocks::legacy`] is the caller's choice).
    pub fn from_record(record: &Value) -> Option<Unlocks> {
        let list = record.get("unlocked")?.as_array()?;
        let starter = record.get("starter").and_then(Value::as_str).and_then(kh_host::formation::chara_of);
        let mut u = Unlocks { starter: starter.unwrap_or(PartyChara::Sora), unlocked: [false; 3] };
        for c in list.iter().filter_map(Value::as_str).filter_map(kh_host::formation::chara_of) {
            u.unlocked[c as usize] = true;
        }
        if starter.is_none() {
            // (a list without a starter: the first unlocked one, Sora first)
            u.starter = ALL.into_iter().find(|c| u.unlocked[*c as usize]).unwrap_or(PartyChara::Sora);
        }
        // the starter is always unlocked (a record cannot lock him out)
        u.unlocked[u.starter as usize] = true;
        Some(u)
    }

    /// Writes the list and the starter into a save's party record (an object; anything else is made one).
    pub fn stamp(&self, record: &mut Value) {
        if !record.is_object() {
            *record = json!({});
        }
        record["unlocked"] = json!(self.list().iter().map(|c| kh_host::formation::key_of(*c)).collect::<Vec<_>>());
        record["starter"] = json!(kh_host::formation::key_of(self.starter));
    }

    pub fn is_unlocked(&self, c: PartyChara) -> bool {
        self.unlocked[c as usize]
    }

    /// Unlocks him; true when he was locked.
    pub fn unlock(&mut self, c: PartyChara) -> bool {
        !std::mem::replace(&mut self.unlocked[c as usize], true)
    }

    /// (testing aid `unlock.reset`) Locks him again (never the starter); true when he was unlocked.
    pub fn lock(&mut self, c: PartyChara) -> bool {
        c != self.starter && std::mem::replace(&mut self.unlocked[c as usize], false)
    }

    /// The unlocked ones, Sora / Donald / Goofy order.
    pub fn list(&self) -> Vec<PartyChara> {
        ALL.into_iter().filter(|c| self.unlocked[*c as usize]).collect()
    }

    /// He waits at his unlock spot (a staged fight or the crone): he is not unlocked yet.
    pub fn waits(&self, c: PartyChara) -> bool {
        !self.is_unlocked(c)
    }

    /// Margit's intro is skipped while Sora waits for him (the user: "This skips Margit's intro cutscene").
    pub fn skip_margit_intro(&self) -> bool {
        self.waits(PartyChara::Sora)
    }

    /// One line for the log.
    pub fn describe(&self) -> String {
        format!("starter {}, unlocked [{}]", self.starter.name(), self.list().iter().map(|c| c.name()).collect::<Vec<_>>().join(", "))
    }
}

/// (CV3, ERT4 fault 7) The save's party record says the starter is still to be asked (`starter_pending`).
pub fn starter_pending(record: &Value) -> bool {
    record.get("starter_pending").and_then(Value::as_bool) == Some(true)
}

/// The starter of a new game's pending file / the menu's index (0 Sora, 1 Donald, 2 Goofy).
pub fn starter_of_index(i: usize) -> PartyChara {
    ALL[i.min(2)]
}

// ------------------------------------------------------------------------------------------------------ the staged fight

/// Where a fight scene is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FightPhase {
    /// Not looked at yet (the first evaluation decides).
    Idle,
    /// The friend waits for the player to come near (nothing is simulated).
    Away,
    /// The player is near, outside the fog: the friend and the boss fight, nobody takes damage.
    Staged,
    /// The player went in: a normal fight, the friend in the party as an ally.
    Fight,
    /// The boss is dead: the unlock is being given.
    Won,
    /// He is unlocked: the scene is over for good.
    Done,
}

/// What the game says this frame (None = could not be read).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FightFacts {
    pub unlocked: bool,
    pub defeated: Option<bool>,
    pub fog_passed: Option<bool>,
    /// The game shows this boss's bar (its fight event ran: the player is in, or the boss was provoked).
    pub boss_bar: bool,
    /// The boss character was found by its entity id (loaded), and is alive.
    pub boss_found: bool,
    pub boss_alive: bool,
    /// The player is within `stage.range` of the boss (or of its map place when it is not loaded).
    pub near: bool,
    /// The leader is down (game over), a load or a death's reload is under way.
    pub leader_down: bool,
    /// A cutscene / the game's own menu: nothing starts (a running stage holds).
    pub hidden: bool,
}

/// What the host does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FightAct {
    /// Put the friend at the arena and let the boss and him fight (no damage either way).
    Stage,
    /// Take the staged friend away (why).
    Unstage(&'static str),
    /// The player is in: the friend joins the party where he stands (a normal fight).
    Join,
    /// The friend leaves the party again (why): the next approach stages him anew.
    Leave(&'static str),
    /// The boss is dead: unlock him (the celebration follows).
    Unlock,
}

#[derive(Clone, Debug)]
pub struct FightDirector {
    pub phase: FightPhase,
    /// Seconds the running fight has had neither its fog flag nor the boss bar.
    lost_for: f32,
    pub lost_after: f32,
}

impl FightDirector {
    pub fn new(lost_after: f32) -> FightDirector {
        FightDirector { phase: FightPhase::Idle, lost_for: 0.0, lost_after }
    }

    /// (testing aid) Back to the start: the next frame decides afresh.
    pub fn reset(&mut self) {
        self.phase = FightPhase::Idle;
        self.lost_for = 0.0;
    }

    /// One frame: the phase change and what the host does about it.
    pub fn step(&mut self, f: &FightFacts, dt: f32) -> Vec<FightAct> {
        use FightPhase as P;
        let mut acts = Vec::new();
        let was = self.phase;
        let inside = f.fog_passed == Some(true) || f.boss_bar;
        if f.unlocked {
            if was == P::Staged {
                acts.push(FightAct::Unstage("he is unlocked"));
            }
            self.phase = P::Done;
            return acts;
        }
        if f.defeated == Some(true) {
            match was {
                P::Staged => acts.push(FightAct::Unstage("the boss is dead")),
                P::Won | P::Done => return acts,
                _ => {}
            }
            acts.push(FightAct::Unlock);
            self.phase = P::Won;
            return acts;
        }
        match was {
            P::Idle | P::Away | P::Won | P::Done => {
                if f.near && f.boss_found && f.boss_alive && !f.hidden && !f.leader_down && f.defeated == Some(false) {
                    if inside {
                        // (the stage begins with the player already in: the fight goes on with him in the party)
                        acts.push(FightAct::Join);
                        self.phase = P::Fight;
                        self.lost_for = 0.0;
                    } else {
                        acts.push(FightAct::Stage);
                        self.phase = P::Staged;
                    }
                } else if was == P::Idle {
                    self.phase = P::Away;
                }
            }
            P::Staged => {
                if !f.near || !f.boss_found || !f.boss_alive {
                    acts.push(FightAct::Unstage(if !f.near { "the player went away" } else { "the boss is gone (unloaded)" }));
                    self.phase = P::Away;
                } else if f.leader_down {
                    acts.push(FightAct::Unstage("the leader is down"));
                    self.phase = P::Away;
                } else if inside {
                    acts.push(FightAct::Join);
                    self.phase = P::Fight;
                    self.lost_for = 0.0;
                }
            }
            P::Fight => {
                self.lost_for = if inside { 0.0 } else { self.lost_for + dt };
                if f.leader_down {
                    acts.push(FightAct::Leave("the leader is down: the fight restarts after the reload"));
                    self.phase = P::Away;
                } else if !f.boss_found && !f.near {
                    acts.push(FightAct::Leave("the boss is gone (a load, a warp)"));
                    self.phase = P::Away;
                } else if self.lost_for >= self.lost_after {
                    acts.push(FightAct::Leave("neither the fog flag nor the boss bar for a while (a death, a reload)"));
                    self.phase = P::Away;
                }
            }
        }
        acts
    }
}

// ------------------------------------------------------------------------------------------------------ the talk scene

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TalkPhase {
    Idle,
    /// The player is far: Donald is not there.
    Away,
    /// Donald stands at the crone, angry.
    Angry,
    /// The leader is close: the prompt is up (Y talks).
    Prompt,
    /// Line n of the talk is up.
    Talking(usize),
    /// The talk is over: the unlock.
    Unlocking,
    Done,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TalkFacts {
    pub unlocked: bool,
    /// The player is within `show_range` of the crone's place (her tile loaded).
    pub near: bool,
    /// The leader's distance to Donald (m) and whether Donald is in front of him (the prompt's rule).
    pub leader_dist: f32,
    pub facing: bool,
    /// Y went down this frame (the prompt's button).
    pub talk_pressed: bool,
    /// The talk window says the line was advanced (A after it was all shown).
    pub advanced: bool,
    /// A cutscene / a menu / the leader down / a load: nothing starts; a running talk ends.
    pub hidden: bool,
    /// An enemy is fighting the leader (no talking in battle, as KH3's Talk command).
    pub in_battle: bool,
    /// (CV3) Another held conversation runs (an Elden Ring NPC's talk): his prompt waits until it is over.
    pub busy: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TalkAct {
    Show,
    Hide,
    PromptOn,
    PromptOff,
    /// Line n (0-based) is to be shown.
    Line(usize),
    /// The window goes (the talk ended or was broken off).
    Close,
    Unlock,
}

#[derive(Clone, Debug)]
pub struct TalkDirector {
    pub phase: TalkPhase,
    pub lines: usize,
    pub talk_range: f32,
}

impl TalkDirector {
    pub fn new(lines: usize, talk_range: f32) -> TalkDirector {
        TalkDirector { phase: TalkPhase::Idle, lines: lines.max(1), talk_range }
    }

    pub fn reset(&mut self) {
        self.phase = TalkPhase::Idle;
    }

    pub fn step(&mut self, f: &TalkFacts) -> Vec<TalkAct> {
        use TalkPhase as P;
        let mut acts = Vec::new();
        let was = self.phase;
        if f.unlocked {
            match was {
                P::Prompt => acts.extend([TalkAct::PromptOff, TalkAct::Hide]),
                P::Talking(_) => acts.extend([TalkAct::Close, TalkAct::Hide]),
                P::Angry => acts.push(TalkAct::Hide),
                _ => {}
            }
            self.phase = P::Done;
            return acts;
        }
        let close_enough = f.leader_dist <= self.talk_range && f.facing;
        match was {
            P::Idle | P::Away | P::Done => {
                if f.near {
                    acts.push(TalkAct::Show);
                    self.phase = P::Angry;
                } else {
                    self.phase = P::Away;
                }
            }
            P::Angry => {
                if !f.near {
                    acts.push(TalkAct::Hide);
                    self.phase = P::Away;
                } else if close_enough && !f.hidden && !f.in_battle && !f.busy {
                    acts.push(TalkAct::PromptOn);
                    self.phase = P::Prompt;
                }
            }
            P::Prompt => {
                if !f.near || f.hidden || f.in_battle || f.busy || f.leader_dist > self.talk_range + 0.5 || !f.facing {
                    acts.push(TalkAct::PromptOff);
                    if !f.near {
                        acts.push(TalkAct::Hide);
                    }
                    self.phase = if f.near { P::Angry } else { P::Away };
                } else if f.talk_pressed {
                    acts.extend([TalkAct::PromptOff, TalkAct::Line(0)]);
                    self.phase = P::Talking(0);
                }
            }
            P::Talking(n) => {
                if f.hidden || !f.near || f.leader_dist > 8.0 {
                    // broken off: he starts over at the next talk
                    acts.push(TalkAct::Close);
                    self.phase = if f.near { P::Angry } else { P::Away };
                    if !f.near {
                        acts.push(TalkAct::Hide);
                    }
                } else if f.advanced {
                    if n + 1 < self.lines {
                        acts.push(TalkAct::Line(n + 1));
                        self.phase = P::Talking(n + 1);
                    } else {
                        acts.extend([TalkAct::Close, TalkAct::Unlock]);
                        self.phase = P::Unlocking;
                    }
                }
            }
            P::Unlocking => {}
        }
        acts
    }
}

// ------------------------------------------------------------------------------------------------------ Sora's stage pilot

/// AI SORA AT MARGIT BEFORE THE PLAYER IS IN (our design: KH3's AI Sora follows a leader, and there is none at the
/// arena): Sora's own pad driven by a small fight rule. Close in on the boss; A in rhythm when within reach (his
/// combos); a dodge (X with the stick sideways) when the boss swings close; now and then a jump and an air combo.
#[derive(Clone, Debug)]
pub struct Pilot {
    t: f32,
    press: f32,
    dodge_cd: f32,
    jump_in: f32,
    air: f32,
    rng: u32,
}

/// What the pilot sees of the boss.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PilotBoss {
    pub position: Vec3,
    pub radius: f32,
    pub attacking: bool,
}

/// The pad for one step: (stick, the yaw it is relative to, A, B, X).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PilotPad {
    pub stick: Vec2,
    pub camera_yaw: f32,
    pub a: bool,
    pub b: bool,
    pub x: bool,
}

impl Default for Pilot {
    fn default() -> Pilot {
        Pilot { t: 0.0, press: 0.0, dodge_cd: 0.0, jump_in: 6.0, air: 0.0, rng: 0x5EED_0F50 }
    }
}

impl Pilot {
    fn random(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng >> 8) as f32 / (1u32 << 24) as f32
    }

    /// `me` = Sora's position (KH3 cm), `on_floor`; `boss` in KH3 cm.
    pub fn pad(&mut self, dt: f32, me: Vec3, on_floor: bool, boss: Option<PilotBoss>) -> PilotPad {
        self.t += dt;
        self.press -= dt;
        self.dodge_cd -= dt;
        self.jump_in -= dt;
        let Some(b) = boss else { return PilotPad::default() };
        let d = Vec2::new(b.position.x - me.x, b.position.y - me.y);
        let heading = d.y.atan2(d.x).to_degrees();
        let gap = d.length() - b.radius;
        let mut pad = PilotPad { camera_yaw: heading, ..PilotPad::default() };
        // an air combo under way: A in rhythm until he lands
        if self.air > 0.0 {
            self.air -= dt;
            if self.press <= 0.0 {
                pad.a = true;
                self.press = 0.22;
            }
            return pad;
        }
        if b.attacking && gap < 380.0 && self.dodge_cd <= 0.0 && on_floor {
            // a dodge to one side
            let side = if self.random() < 0.5 { -1.0 } else { 1.0 };
            pad.stick = Vec2::new(side, -0.3).normalize();
            pad.x = true;
            self.dodge_cd = 2.0;
            return pad;
        }
        if gap > 170.0 {
            pad.stick = Vec2::new(0.0, 1.0);
            return pad;
        }
        if self.jump_in <= 0.0 && on_floor {
            pad.b = true;
            self.jump_in = 5.0 + 4.0 * self.random();
            self.air = 1.2;
            self.press = 0.15;
            return pad;
        }
        if self.press <= 0.0 {
            pad.a = true;
            self.press = 0.24;
        }
        pad
    }
}

// ------------------------------------------------------------------------------------------------------ the celebration

/// THE UNLOCK MOMENT'S CLOCK: the effect, sound, voice and banner at its start; the rows one after the other; his AI
/// held until `total`.
#[derive(Clone, Debug)]
pub struct Celebration {
    pub chara: PartyChara,
    pub t: f32,
    pub total: f32,
    pub banner: f32,
    pub rows: Vec<String>,
    pub row: usize,
    started: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CelebAct {
    /// The effect, the fanfare, his voice, the banner.
    Start,
    /// Play this row of his (the previous one ended or this is the first).
    Row(String),
    /// Over: his AI runs again.
    End,
}

impl Celebration {
    pub fn new(chara: PartyChara, rows: Vec<String>, banner: f32, total: f32) -> Celebration {
        Celebration { chara, t: 0.0, total: total.max(0.5), banner, rows, row: 0, started: false }
    }

    /// `row_done` = the row he plays ended (or could not be played).
    pub fn step(&mut self, dt: f32, row_done: bool) -> Vec<CelebAct> {
        let mut acts = Vec::new();
        if !self.started {
            self.started = true;
            acts.push(CelebAct::Start);
            if let Some(r) = self.rows.first() {
                acts.push(CelebAct::Row(r.clone()));
            }
            return acts;
        }
        self.t += dt;
        if row_done && self.row + 1 < self.rows.len() {
            self.row += 1;
            acts.push(CelebAct::Row(self.rows[self.row].clone()));
        }
        if self.t >= self.total {
            acts.push(CelebAct::End);
        }
        acts
    }
}

// ------------------------------------------------------------------------------------------------------ placing

/// The place `offset` m from `from` toward `to` (horizontal), ER axes (y up). `to == from`: `from`.
pub fn toward(from: Vec3, to: Vec3, offset: f32) -> Vec3 {
    let d = Vec3::new(to.x - from.x, 0.0, to.z - from.z);
    if d.length() < 1e-3 {
        return from;
    }
    from + d.normalize() * offset
}

/// An ER map-local place in this frame's physics space: `local` of map (area, gx, gz) seen from a player at
/// `player_local` of his block (area, gx, gz) whose physics position is `player_physics`. Overworld tiles are 256 m;
/// elsewhere only the same block counts. None: another area or too far.
pub fn local_to_physics(local: Vec3, map: (u8, i32, i32), player_block: (u8, i32, i32), overworld: bool, player_local: Vec3, player_physics: Vec3, reach: f32) -> Option<Vec3> {
    if map.0 != player_block.0 {
        return None;
    }
    let (dx, dz) = (map.1 - player_block.1, map.2 - player_block.2);
    if !overworld && (dx != 0 || dz != 0) {
        return None;
    }
    let shifted = Vec3::new(local.x + 256.0 * dx as f32, local.y, local.z + 256.0 * dz as f32);
    let p = player_physics + (shifted - player_local);
    (p.is_finite() && (p - player_physics).length() <= reach).then_some(p)
}

/// THE FACING OF AN MSB ROTATION Y (degrees) in Elden Ring axes: the model's -Z turned by +rot about Y. MEASURED on the
/// data: Margit (rot -80.08) then faces his entry fog (+x of him), Leonine (rot 179.69) faces his fog (+z of him).
pub fn msb_facing(rot_y: f32) -> Vec3 {
    let a = rot_y.to_radians();
    Vec3::new(-a.sin(), 0.0, -a.cos())
}

/// (CV3) Her facing as a KH3 yaw (degrees): her live one when her character is loaded, else her MSB rotation's.
pub fn her_yaw_kh(live: Option<f32>, msb_rot_y: f32) -> f32 {
    live.unwrap_or_else(|| crate::coords::yaw_kh_of_er_dir(msb_facing(msb_rot_y)))
}

/// (CV3) DONALD'S RECRUIT SPOT RELATIVE TO THE CRONE (Elden Ring physics space, metres): `dist` m from her in the
/// direction `angle` degrees off her facing (KH3 yaw sense), and his KH3 yaw FACING HER (CV3 round 4, the user's
/// original decision: "stand right in front of the Finger Reader looking at her"). The same rule for her live facing
/// and her MSB one. Pure.
pub fn recruit_spot(crone_er: Vec3, her_yaw: f32, dist: f32, angle: f32) -> (Vec3, f32) {
    let dir = crate::coords::facing_er(her_yaw + angle);
    let yaw = (her_yaw + angle + 180.0 + 540.0).rem_euclid(360.0) - 180.0;
    (crone_er + dir * dist, yaw)
}

/// (CV3 round 4) HOW DONALD STANDS at the crone, one rule for every step (applied before AND after his stage's step, so
/// neither a clip's root motion nor the idle turn toward his focus can carry him off his spot or turn him away): his
/// spot always; before / after the talk he faces HER and looks at her face; during the talk he faces the leader he
/// speaks to (when not right on top of him) and looks at his face. Returns (yaw, the point he looks at). Pure.
pub fn donald_pose(spot: Vec3, her_yaw_from_spot: f32, talking: bool, leader: Vec3, leader_head: Vec3, her_head: Option<Vec3>) -> (f32, Option<Vec3>) {
    if talking {
        let d = leader - spot;
        let yaw = if Vec2::new(d.x, d.y).length() > 30.0 { d.y.atan2(d.x).to_degrees() } else { her_yaw_from_spot };
        (yaw, Some(leader_head))
    } else {
        (her_yaw_from_spot, her_head)
    }
}

/// (CV3 round 4) His talk motion between the lines' gestures: KH3's NPC talk loop (n_ex001 0802).
pub const DONALD_TALK_LOOP: &str = "TalkLoopN";

/// (CV3) The leader to Donald for the talk prompt, WHOEVER LEADS (KH3 cm, degrees): the distance in metres (flat) and
/// whether Donald is in front of him (within 70 degrees of his yaw). Pure.
pub fn talk_geometry(leader_kh: Vec3, leader_yaw: f32, donald_kh: Option<Vec3>) -> (f32, bool) {
    match donald_kh {
        Some(dk) => {
            let d = dk - leader_kh;
            let off = (d.y.atan2(d.x).to_degrees() - leader_yaw + 540.0).rem_euclid(360.0) - 180.0;
            (Vec2::new(d.x, d.y).length() / 100.0, off.abs() <= 70.0)
        }
        None => (f32::INFINITY, false),
    }
}

/// `mAA_BB_CC_DD` -> (area AA, BB, CC) (the open world: grid x, grid z).
pub fn parse_map(name: &str) -> Option<(u8, i32, i32)> {
    let parts: Vec<&str> = name.trim_start_matches('m').split('_').collect();
    if parts.len() < 3 {
        return None;
    }
    Some((parts[0].parse().ok()?, parts[1].parse().ok()?, parts[2].parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_game_unlocks_the_starter_only_and_the_record_round_trips() {
        let mut u = Unlocks::new_game(PartyChara::Goofy);
        assert_eq!(u.list(), vec![PartyChara::Goofy]);
        assert!(u.waits(PartyChara::Sora) && u.waits(PartyChara::Donald) && !u.waits(PartyChara::Goofy));
        assert!(u.skip_margit_intro(), "Sora waits at Margit: the intro is skipped");
        assert!(u.unlock(PartyChara::Donald));
        assert!(!u.unlock(PartyChara::Donald), "once");
        let mut record = json!({ "format": 2, "leader": "goofy" });
        u.stamp(&mut record);
        assert_eq!(record["unlocked"], json!(["donald", "goofy"]));
        assert_eq!(record["starter"], json!("goofy"));
        assert_eq!(record["leader"], json!("goofy"), "the rest of the record stays");
        assert_eq!(Unlocks::from_record(&record), Some(u.clone()));
        // the starter cannot be locked, nor left out of a record
        assert!(!u.lock(PartyChara::Goofy));
        let odd = json!({ "unlocked": ["sora"], "starter": "donald" });
        assert!(Unlocks::from_record(&odd).unwrap().is_unlocked(PartyChara::Donald));
    }

    #[test]
    fn an_old_save_is_sora_alone() {
        assert_eq!(Unlocks::from_record(&json!({ "leader": "sora" })), None);
        let u = Unlocks::legacy();
        assert_eq!(u.list(), vec![PartyChara::Sora]);
        assert!(!u.skip_margit_intro(), "Sora is the starter: Margit's intro plays");
        // a list without a starter: the first unlocked one
        let u = Unlocks::from_record(&json!({ "unlocked": ["goofy", "donald"] })).unwrap();
        assert_eq!(u.starter, PartyChara::Donald);
    }

    fn near() -> FightFacts {
        FightFacts { unlocked: false, defeated: Some(false), fog_passed: Some(false), boss_bar: false, boss_found: true, boss_alive: true, near: true, leader_down: false, hidden: false }
    }

    #[test]
    fn the_fight_stages_joins_and_unlocks() {
        let mut d = FightDirector::new(4.0);
        assert_eq!(d.step(&FightFacts { near: false, ..near() }, 0.016), vec![]);
        assert_eq!(d.phase, FightPhase::Away);
        assert_eq!(d.step(&near(), 0.016), vec![FightAct::Stage]);
        assert_eq!(d.step(&near(), 0.016), vec![], "staged: nothing new");
        // the player is through the fog: he joins
        assert_eq!(d.step(&FightFacts { fog_passed: Some(true), ..near() }, 0.016), vec![FightAct::Join]);
        // the boss bar alone keeps the fight
        for _ in 0..600 {
            assert_eq!(d.step(&FightFacts { boss_bar: true, ..near() }, 0.016), vec![]);
        }
        assert_eq!(d.phase, FightPhase::Fight);
        // the boss dies
        assert_eq!(d.step(&FightFacts { defeated: Some(true), boss_alive: false, ..near() }, 0.016), vec![FightAct::Unlock]);
        assert_eq!(d.phase, FightPhase::Won);
        assert_eq!(d.step(&FightFacts { defeated: Some(true), ..near() }, 0.016), vec![], "once");
        assert_eq!(d.step(&FightFacts { unlocked: true, ..near() }, 0.016), vec![]);
        assert_eq!(d.phase, FightPhase::Done);
    }

    #[test]
    fn a_death_in_the_fight_restages_him() {
        let mut d = FightDirector::new(4.0);
        d.step(&near(), 0.016);
        d.step(&FightFacts { fog_passed: Some(true), ..near() }, 0.016);
        assert_eq!(d.phase, FightPhase::Fight);
        let acts = d.step(&FightFacts { fog_passed: Some(true), leader_down: true, ..near() }, 0.016);
        assert!(matches!(acts[..], [FightAct::Leave(_)]));
        // the reload: the flags read off again, he is staged anew
        assert_eq!(d.step(&near(), 0.016), vec![FightAct::Stage]);
        // a fight whose flag and bar both go away for 4 s ends too
        d.step(&FightFacts { boss_bar: true, ..near() }, 0.016);
        let mut left = false;
        for _ in 0..300 {
            if matches!(d.step(&near(), 0.016)[..], [FightAct::Leave(_)]) {
                left = true;
                break;
            }
        }
        assert!(left && d.phase == FightPhase::Away);
    }

    #[test]
    fn a_boss_already_dead_unlocks_at_once_and_an_unlocked_friend_is_never_staged() {
        let mut d = FightDirector::new(4.0);
        assert_eq!(d.step(&FightFacts { defeated: Some(true), ..near() }, 0.016), vec![FightAct::Unlock]);
        let mut d = FightDirector::new(4.0);
        assert_eq!(d.step(&FightFacts { unlocked: true, ..near() }, 0.016), vec![]);
        assert_eq!(d.phase, FightPhase::Done);
        // unknown flags (not readable) stage nothing
        let mut d = FightDirector::new(4.0);
        assert_eq!(d.step(&FightFacts { defeated: None, ..near() }, 0.016), vec![]);
        // staged and the player leaves
        let mut d = FightDirector::new(4.0);
        d.step(&near(), 0.016);
        assert!(matches!(d.step(&FightFacts { near: false, ..near() }, 0.016)[..], [FightAct::Unstage(_)]));
    }

    fn talk() -> TalkFacts {
        TalkFacts { unlocked: false, near: true, leader_dist: 10.0, facing: true, talk_pressed: false, advanced: false, hidden: false, in_battle: false, busy: false }
    }

    #[test]
    fn the_talk_runs_line_by_line_and_unlocks() {
        let mut d = TalkDirector::new(3, 2.5);
        assert_eq!(d.step(&talk()), vec![TalkAct::Show]);
        assert_eq!(d.step(&talk()), vec![]);
        assert_eq!(d.step(&TalkFacts { leader_dist: 2.0, ..talk() }), vec![TalkAct::PromptOn]);
        assert_eq!(d.step(&TalkFacts { leader_dist: 2.0, talk_pressed: true, ..talk() }), vec![TalkAct::PromptOff, TalkAct::Line(0)]);
        assert_eq!(d.step(&TalkFacts { leader_dist: 2.0, ..talk() }), vec![]);
        assert_eq!(d.step(&TalkFacts { leader_dist: 2.0, advanced: true, ..talk() }), vec![TalkAct::Line(1)]);
        assert_eq!(d.step(&TalkFacts { leader_dist: 2.0, advanced: true, ..talk() }), vec![TalkAct::Line(2)]);
        assert_eq!(d.step(&TalkFacts { leader_dist: 2.0, advanced: true, ..talk() }), vec![TalkAct::Close, TalkAct::Unlock]);
        assert_eq!(d.phase, TalkPhase::Unlocking);
        assert_eq!(d.step(&TalkFacts { unlocked: true, ..talk() }), vec![]);
        assert_eq!(d.phase, TalkPhase::Done);
    }

    #[test]
    fn no_talk_in_battle_and_a_broken_talk_starts_over() {
        let mut d = TalkDirector::new(3, 2.5);
        d.step(&talk());
        assert_eq!(d.step(&TalkFacts { leader_dist: 2.0, in_battle: true, ..talk() }), vec![]);
        assert_eq!(d.step(&TalkFacts { leader_dist: 2.0, facing: false, ..talk() }), vec![], "not facing him");
        d.step(&TalkFacts { leader_dist: 2.0, ..talk() });
        d.step(&TalkFacts { leader_dist: 2.0, talk_pressed: true, ..talk() });
        assert_eq!(d.step(&TalkFacts { leader_dist: 9.0, ..talk() }), vec![TalkAct::Close]);
        assert_eq!(d.phase, TalkPhase::Angry);
        // he walks off while the prompt is up
        d.step(&TalkFacts { leader_dist: 2.0, ..talk() });
        assert_eq!(d.step(&TalkFacts { near: false, ..talk() }), vec![TalkAct::PromptOff, TalkAct::Hide]);
    }

    /// (CV3 round 4) Donald's pose: before / after the talk he faces her and looks at her face; during it he faces the
    /// leader and looks at his face; a leader right on top of him keeps his facing (no spin).
    #[test]
    fn cv3_donald_looks_at_her_then_at_the_leader_he_talks_to() {
        let spot = Vec3::new(0.0, 0.0, 0.0);
        let her_head = Vec3::new(-300.0, 0.0, 150.0);
        let leader = Vec3::new(0.0, 400.0, 0.0);
        let leader_head = leader + Vec3::new(0.0, 0.0, 140.0);
        assert_eq!(donald_pose(spot, 180.0, false, leader, leader_head, Some(her_head)), (180.0, Some(her_head)), "before: at her");
        let (yaw, look) = donald_pose(spot, 180.0, true, leader, leader_head, Some(her_head));
        assert!((yaw - 90.0).abs() < 0.01, "talking: toward the leader ({yaw})");
        assert_eq!(look, Some(leader_head));
        assert_eq!(donald_pose(spot, 180.0, true, Vec3::new(10.0, 0.0, 0.0), leader_head, Some(her_head)).0, 180.0, "a leader on top of him: no spin");
        assert_eq!(DONALD_TALK_LOOP, "TalkLoopN");
    }

    /// (CV3) DONALD'S RECRUIT SPOT: ~3 m from her, 35 degrees off her facing toward the approach, facing HER (round 4);
    /// the same for her live facing (-141, the user's log) and her MSB rotation (39.271); outside her own talk reach.
    #[test]
    fn cv3_the_recruit_spot_holds_for_both_her_facings_and_is_outside_her_reach() {
        let crone = Vec3::new(10.0, 5.0, -3.0);
        let (live, msb) = (her_yaw_kh(Some(-141.0), 39.271), her_yaw_kh(None, 39.271));
        assert!((live - msb).abs() < 1.0, "her MSB rotation gives her live facing: {live} / {msb}");
        let sheet = super::super::sheet::UnlockSheet::default().donald;
        let (a, yaw_a) = recruit_spot(crone, live, sheet.spot_from_her(), sheet.spot_angle);
        let (b, yaw_b) = recruit_spot(crone, msb, sheet.spot_from_her(), sheet.spot_angle);
        assert!((a - b).length() < 0.05 && (yaw_a - yaw_b).abs() < 1.0, "both placements agree");
        let flat = |p: Vec3| Vec2::new(p.x - crone.x, p.z - crone.z).length();
        assert!((flat(a) - 3.0).abs() < 1e-3, "3 m from her: {}", flat(a));
        assert!(flat(a) > sheet.crone_reach, "outside her talk reach ({} m)", sheet.crone_reach);
        // where CV1 had him (1.25 m in front of her) is far from the new spot; the new spot's bearing from her is her
        // facing - 35 (KH3 yaw), the user's standing place in the log (2.95 m, bearing -173)
        let old = crone + crate::coords::facing_er(live) * 1.25;
        assert!((a - old).length() > 1.7, "{}", (a - old).length());
        let bearing = crate::coords::yaw_kh_of_er_dir(a - crone);
        assert!((bearing - (-176.0)).abs() < 1.0, "{bearing}");
        // (round 4) he faces HER: his yaw points from his spot to her
        let to_her = crate::coords::yaw_kh_of_er_dir(crone - a);
        assert!((yaw_a - to_her).abs() < 0.5, "he faces her: {yaw_a} vs {to_her}");
        // a spot asked inside her reach is pushed out of it
        let near = super::super::sheet::TalkScene { spot_dist: 1.0, ..sheet };
        assert!(near.spot_from_her() > near.crone_reach);
    }

    /// (CV3) RECRUITING WORKS WHOEVER LEADS: Sora, Goofy (Sora out of the party with the battle flag his staged Margit
    /// fight left on: THE USER'S SESSION), a later character. The prompt comes up within his talk range, Y talks, six
    /// advances unlock him. The old rule (Sora's flag counted whoever leads) never offered the prompt with Goofy.
    #[test]
    fn cv3_recruiting_works_whoever_leads() {
        let donald = Vec3::new(1000.0, 200.0, 0.0);
        // (leader, Sora in the active party, Sora's battle flag)
        for (who, sora_in, sora_flag) in [("Sora", true, false), ("Goofy after Sora's staged fight", false, true), ("a later leader", false, false)] {
            let mut d = TalkDirector::new(6, 2.5);
            let mut prompt_at = None;
            for k in 0..=20 {
                let at = Vec3::new(k as f32 * 45.0, 200.0, 0.0);
                let (dist, facing) = talk_geometry(at, 0.0, Some(donald));
                let in_battle = crate::conv::leader_combat(sora_in, sora_flag, false);
                let acts = d.step(&TalkFacts { leader_dist: dist, facing, in_battle, ..talk() });
                if acts.contains(&TalkAct::PromptOn) {
                    prompt_at = Some(dist);
                }
            }
            let dist = prompt_at.unwrap_or_else(|| panic!("{who}: the prompt never came up"));
            assert!(dist <= 2.5, "{who}: within his talk range ({dist} m)");
            let (dist, facing) = talk_geometry(Vec3::new(900.0, 200.0, 0.0), 0.0, Some(donald));
            let f = TalkFacts { leader_dist: dist, facing, in_battle: crate::conv::leader_combat(sora_in, sora_flag, false), ..talk() };
            assert_eq!(d.step(&TalkFacts { talk_pressed: true, ..f }), vec![TalkAct::PromptOff, TalkAct::Line(0)], "{who}");
            for n in 1..6 {
                assert_eq!(d.step(&TalkFacts { advanced: true, ..f }), vec![TalkAct::Line(n)], "{who}");
            }
            assert_eq!(d.step(&TalkFacts { advanced: true, ..f }), vec![TalkAct::Close, TalkAct::Unlock], "{who}: unlocked");
        }
        // the bug itself: Sora's frozen flag counted for Goofy = never a prompt
        let mut d = TalkDirector::new(6, 2.5);
        d.step(&talk());
        assert_eq!(d.step(&TalkFacts { leader_dist: 1.0, in_battle: true, ..talk() }), vec![], "(the user's session: no prompt)");
        // not facing him: no prompt; another held conversation (the crone's talk) runs: his prompt waits, and goes off
        assert_eq!(d.step(&TalkFacts { leader_dist: 1.0, facing: false, ..talk() }), vec![]);
        assert_eq!(d.step(&TalkFacts { leader_dist: 1.0, busy: true, ..talk() }), vec![]);
        assert_eq!(d.step(&TalkFacts { leader_dist: 1.0, ..talk() }), vec![TalkAct::PromptOn]);
        assert_eq!(d.step(&TalkFacts { leader_dist: 1.0, busy: true, ..talk() }), vec![TalkAct::PromptOff]);
    }

    /// (CV3) THE PLACEMENT PICTURE (top-down, KH3 x right / y up, 100 px a metre, the crone in the middle): her (white),
    /// her facing, her own talk reach (2.35 m, red ring), CV1's spot 1.25 m in front of her (grey), the new spot (yellow)
    /// with his facing and his talk range (2.5 m, green ring), and the leader's places in the user's session (blue: from
    /// the camera lines of kh-er_final3_session.log, about +-0.5 m; the two still points at 2.95 m are the big ones).
    /// build/sora_tmp/party/cv3/shots/cv3_donald_spot_map.png.
    #[test]
    fn cv3_the_placement_picture() {
        let (w, h, s) = (1100usize, 1100usize, 100.0f32);
        let crone = Vec3::new(84305.0, -34698.0, 0.0); // KH3 cm (from the log: Donald's old spot + 1.25 m toward her)
        let her = -141.0f32;
        let sheet = super::super::sheet::UnlockSheet::default().donald;
        let dir = |yaw: f32| Vec3::new(yaw.to_radians().cos(), yaw.to_radians().sin(), 0.0);
        let new_spot = crone + dir(her + sheet.spot_angle) * sheet.spot_from_her() * 100.0;
        let new_yaw = her + sheet.spot_angle + 180.0; // (round 4) facing her
        let old_spot = crone + dir(her) * 125.0;
        let path = [(82846.0, -33852.0), (83589.0, -34483.0), (84030.0, -34706.0), (84088.0, -34770.0), (84028.0, -34737.0), (83962.0, -34704.0), (84085.0, -34781.0), (84123.0, -34820.0), (84115.0, -34776.0), (84055.0, -34834.0), (84194.0, -34891.0), (84227.0, -34995.0), (84105.0, -34951.0), (84043.0, -35157.0), (84201.0, -34883.0), (84225.0, -34771.0), (84306.0, -34795.0), (84227.0, -34694.0), (84451.0, -34875.0)];
        let still = [(84086.0, -34897.0), (84012.0, -34735.0)];
        let px = |p: Vec3| ((p.x - crone.x) / 100.0 * s + w as f32 / 2.0, h as f32 / 2.0 - (p.y - crone.y) / 100.0 * s);
        let mut rgba = vec![0u8; w * h * 4];
        for (k, c) in rgba.chunks_mut(4).enumerate() {
            let (x, y) = ((k % w) as f32, (k / w) as f32);
            let grid = (x - w as f32 / 2.0).rem_euclid(s) < 1.0 || (y - h as f32 / 2.0).rem_euclid(s) < 1.0;
            let v = if grid { 52 } else { 34 };
            c.copy_from_slice(&[v, v, v + 4, 255]);
        }
        let mut put = |x: f32, y: f32, c: [u8; 3]| {
            if x >= 0.0 && y >= 0.0 && (x as usize) < w && (y as usize) < h {
                let k = (y as usize * w + x as usize) * 4;
                rgba[k..k + 3].copy_from_slice(&c);
            }
        };
        let disc = |p: Vec3, r: f32, c: [u8; 3], put: &mut dyn FnMut(f32, f32, [u8; 3])| {
            let (cx, cy) = px(p);
            for dy in -(r as i32)..=(r as i32) {
                for dx in -(r as i32)..=(r as i32) {
                    if ((dx * dx + dy * dy) as f32) <= r * r {
                        put(cx + dx as f32, cy + dy as f32, c);
                    }
                }
            }
        };
        let ring = |p: Vec3, r_m: f32, c: [u8; 3], put: &mut dyn FnMut(f32, f32, [u8; 3])| {
            let (cx, cy) = px(p);
            for k in 0..2000 {
                let a = k as f32 / 2000.0 * std::f32::consts::TAU;
                for t in 0..3 {
                    let r = r_m * s + t as f32 - 1.0;
                    put(cx + a.cos() * r, cy - a.sin() * r, c);
                }
            }
        };
        let arrow = |p: Vec3, yaw: f32, len_m: f32, c: [u8; 3], put: &mut dyn FnMut(f32, f32, [u8; 3])| {
            for k in 0..(len_m * s) as i32 {
                let q = p + dir(yaw) * (k as f32);
                let (x, y) = px(q);
                for d in -1..=1 {
                    put(x + d as f32, y, c);
                    put(x, y + d as f32, c);
                }
            }
        };
        ring(crone, sheet.crone_reach, [220, 70, 60], &mut put);
        ring(new_spot, sheet.talk_range, [80, 200, 90], &mut put);
        for (x, y) in path {
            disc(Vec3::new(x, y, 0.0), 5.0, [90, 140, 230], &mut put);
        }
        for (x, y) in still {
            disc(Vec3::new(x, y, 0.0), 10.0, [120, 170, 255], &mut put);
        }
        arrow(crone, her, 1.0, [235, 235, 235], &mut put);
        disc(crone, 14.0, [240, 240, 240], &mut put);
        disc(old_spot, 10.0, [130, 130, 130], &mut put);
        arrow(new_spot, new_yaw, 0.8, [250, 210, 60], &mut put);
        disc(new_spot, 12.0, [250, 210, 60], &mut put);
        let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/party/cv3/shots");
        std::fs::create_dir_all(&d).unwrap();
        kh_ui::def::write_png(&d.join("cv3_donald_spot_map.png"), w as u32, h as u32, &rgba).unwrap();
        // the still place at 2.95 m, bearing -173 (the leader's in the user's picture, most likely): the new spot is
        // within 0.5 m of it
        let pic = Vec3::new(84012.0, -34735.0, 0.0);
        assert!((new_spot - pic).truncate().length() < 60.0, "{}", (new_spot - pic).truncate().length());
    }

    /// (CV3) THE TALK PRIORITY: his prompt (any party member's) wins Y over Elden Ring's own action prompt.
    #[test]
    fn cv3_a_party_talk_wins_y_over_an_npc_prompt() {
        use crate::conv::er_prompt_wins;
        assert!(er_prompt_wins(true, false, false), "the crone alone: Y talks to her");
        assert!(!er_prompt_wins(true, true, false), "Donald's prompt up as well: Y is Donald's");
        assert!(!er_prompt_wins(true, false, true), "Donald's conversation runs: Y does not start hers under it");
        assert!(!er_prompt_wins(false, true, false) && !er_prompt_wins(false, false, false));
        // whoever leads: the geometry of the prompt is the leader's own place and yaw
        assert_eq!(talk_geometry(Vec3::ZERO, 0.0, Some(Vec3::new(150.0, 0.0, 0.0))), (1.5, true));
        assert_eq!(talk_geometry(Vec3::ZERO, 180.0, Some(Vec3::new(150.0, 0.0, 0.0))).1, false, "his back to Donald");
        assert_eq!(talk_geometry(Vec3::ZERO, 0.0, None).0, f32::INFINITY);
    }

    #[test]
    fn the_pilot_closes_in_attacks_and_dodges() {
        let mut p = Pilot::default();
        let boss = PilotBoss { position: Vec3::new(1000.0, 0.0, 0.0), radius: 100.0, attacking: false };
        let pad = p.pad(0.016, Vec3::ZERO, true, Some(boss));
        assert_eq!(pad.stick, Vec2::new(0.0, 1.0));
        assert!((pad.camera_yaw - 0.0).abs() < 1e-3, "toward the boss (+X)");
        let close = Vec3::new(800.0, 0.0, 0.0);
        let presses = (0..60).filter(|_| p.pad(1.0 / 60.0, close, true, Some(boss)).a).count();
        assert!((3..=6).contains(&presses), "A in rhythm: {presses}");
        let pad = p.pad(0.016, close, true, Some(PilotBoss { attacking: true, ..boss }));
        assert!(pad.x && pad.stick.length() > 0.9, "a dodge when he swings close");
        assert_eq!(p.pad(0.016, close, true, None), PilotPad::default());
    }

    #[test]
    fn the_celebration_runs_its_rows_and_ends() {
        let mut c = Celebration::new(PartyChara::Goofy, vec!["BattleEnd".into(), "ODanceIdle".into()], 3.0, 5.0);
        assert_eq!(c.step(0.016, false), vec![CelebAct::Start, CelebAct::Row("BattleEnd".into())]);
        assert_eq!(c.step(0.5, true), vec![CelebAct::Row("ODanceIdle".into())]);
        assert_eq!(c.step(0.5, true), vec![], "no more rows");
        let mut ended = false;
        for _ in 0..10 {
            if c.step(0.5, false).contains(&CelebAct::End) {
                ended = true;
            }
        }
        assert!(ended);
    }

    #[test]
    fn msb_rotations_face_the_fog() {
        // Margit (m10: rot -80.08 at x -18.6) faces his entry fog at x +19.4; Leonine (rot 179.69 at z -7.4) faces his at z +32.4
        assert!(msb_facing(-80.082).x > 0.95);
        assert!(msb_facing(179.691).z > 0.99);
    }

    #[test]
    fn places() {
        let p = toward(Vec3::new(0.0, 1.0, 0.0), Vec3::new(10.0, 5.0, 0.0), 5.0);
        assert!((p - Vec3::new(5.0, 1.0, 0.0)).length() < 1e-4);
        assert_eq!(parse_map("m60_43_30_00"), Some((60, 43, 30)));
        assert_eq!(parse_map("m10_00_00_00"), Some((10, 0, 0)));
        // the neighbour tile to the east (+x): 256 m on
        let p = local_to_physics(Vec3::new(10.0, 0.0, 0.0), (60, 44, 30), (60, 43, 30), true, Vec3::new(0.0, 0.0, 0.0), Vec3::new(100.0, 0.0, 100.0), 500.0).unwrap();
        assert!((p - Vec3::new(366.0, 0.0, 100.0)).length() < 1e-3);
        assert_eq!(local_to_physics(Vec3::ZERO, (10, 0, 0), (60, 43, 30), true, Vec3::ZERO, Vec3::ZERO, 500.0), None);
        assert_eq!(local_to_physics(Vec3::ZERO, (10, 1, 0), (10, 0, 0), false, Vec3::ZERO, Vec3::ZERO, 500.0), None);
    }

    /// (CV3, ERT4 fault 7) A starter still to be asked survives the save's record (a reload / respawn before the menu after
    /// the intro); an answered one (or an old record) is not asked again.
    #[test]
    fn cv3_a_pending_starter_survives_the_record() {
        assert!(starter_pending(&json!({ "unlocked": ["sora"], "starter": "sora", "starter_pending": true })));
        assert!(!starter_pending(&json!({ "unlocked": ["sora"], "starter": "sora" })));
        assert!(!starter_pending(&json!({ "starter_pending": false })));
        assert!(!starter_pending(&Value::Null));
    }
}
