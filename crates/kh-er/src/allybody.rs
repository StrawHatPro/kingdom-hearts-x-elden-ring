//! (ER party S3) THE AI ALLIES' INVISIBLE ELDEN RING BODIES (plan ER_PARTY_PLAN.md stream S3, research
//! ER_PARTY_RESEARCH.md part 1, state note build/sora_tmp/er/party/IMPL_S3_STATE.md). Compiled with the cargo feature
//! `party` only; at run time `party.er = 0` or `party.bodies = 0` leaves the game untouched.
//!
//! THE USER'S RULE (U1, 2026-10-15): "They need to be targeted and attacked and need to be able to be anywhere at all
//! times, they should never be forced to despawn or break". So:
//! - every AI ally (Donald, Goofy, AI Sora while a friend leads) gets one invisible, passive Elden Ring character made
//!   by the game's own DEBUG CHARACTER CREATOR (`WorldChrMan::spawn_debug_character`), from a Spirit Ash's NpcParam
//!   (BuddyParam, read at run time: the game's own spirit-summon team, so enemies target it by their own AI);
//! - it is written to the friend's feet every frame (both physics phases, as the Tarnished is written to Sora's);
//! - its HP losses become hits on the KH3 friend by Sora's own damage-in rule (raised maximum, `last_hit_by`, the
//!   status classes): `take_hits` -> `deliver` -> `PartyRuntime::receive_hit` (AI Sora: `Sora::take_hit`);
//! - THE FRIEND NEVER DESPAWNS. When the game drops or kills a body (a load, a block change, a death), the loss is
//!   seen the next frame (the body is validated by the debug set and its handle before any field is touched) and a new
//!   body is asked for at once (one spawn a frame); until it is there, that friend simply cannot be hit;
//! - A HIT ON THE LEADER ALWAYS STAYS THE LEADER'S: nothing is ever redirected. (The F1 hit-redirection fallback of
//!   research 1.6 was REMOVED BY THE USER, 2026-10-15: "please delete that backup ... that is the worst shit ever".)
//!   When no body can be made at all, that is logged loudly, spawning is retried every `body.retry` s, and the
//!   go/no-go report says so.
//!
//! Routes, best first (sheet `party-er-bodies`, row `body.order`): `buddy` (a Spirit Ash's row), `c0000` (the player's
//! always-loaded model with a ChrInit appearance), `copy` (a copy of a nearby enemy, team written). A route that times
//! out is skipped until the retry.
//!
//! THE HOST'S SEAM (sora.rs calls it at the end of pre_physics and in post_physics, marked "(ER party S3)"):
//! - `pre_physics(&mut Ctx { .., wants: S1's core.feed.wants })`, then `deliver(core, ..)`: the hits made final with each
//!   friend's own maximum and queued on S1's core (`PartyCore::queue_hit`, applied at the next party step);
//! - `handle(chara)`, `status()`, `is_body(ptr)` (targets.rs leaves bodies out of Sora's targets),
//!   `fire_hit(..)` (targets.rs: stage-2 bullets owned by the body).
//!
//! THE GO / NO-GO TEST the user runs: `party.body_test = go` (any new value) in the control file: `party/bodytest.rs`.
//!
//! EVERYTHING GAME-SIDE IS UNTESTED IN GAME (agents never launch Elden Ring). Every game call is guarded: pointers are
//! checked before use, param access is behind `catch_unwind`, a body is only touched after it was found in the game's
//! own character sets this frame with its own handle.

#[path = "party/bodytest.rs"]
pub mod bodytest;

/// The pure rules: the pool's bookkeeping, HP top-up arithmetic, the hit a body loss becomes, the sheet, the auto
/// pick. No game types: unit-tested.
pub mod rules {
    use glam::Vec3;
    use kh_core::enums::ETresDamageKind;
    use kh_core::host::{HitAttacker, ReactionResult};
    use kh_core::{HitRecord, TargetId};
    use kh_host::party::PartyChara;

    use crate::ermap::Params;
    use crate::status_rules::{self as sr, Loss};

    /// Where a pool slot is.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum SlotState {
        /// Asked of the debug creator; not seen yet.
        Spawning,
        /// Placed at its friend, hittable (or invincible while he is).
        Live,
        /// Unhittable, neutral team, at the leader: a KO'd friend, a cutscene, or free for any friend.
        Parked,
        /// The game dropped it (or it died): never touched again.
        Lost,
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct SlotView {
        pub chara: Option<PartyChara>,
        pub state: SlotState,
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct WantView {
        pub chara: PartyChara,
        pub hittable: bool,
    }

    /// What the pool does this frame.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Action {
        /// Forget a lost slot.
        Drop(usize),
        /// Its friend left the wanted list: parked and free for any friend.
        Release(usize),
        /// A free slot is given to this friend.
        Reuse(usize, PartyChara),
        /// A parked slot of a friend is placed at him again (revived, after a cutscene).
        Activate(usize),
        /// A live slot is parked but stays his (KO).
        Park(usize),
        /// Ask the creator for a body for this friend (at most one a frame).
        Spawn(PartyChara),
    }

    /// THE POOL RULE: every wanted friend keeps his slot; a KO'd one keeps it parked; a friend without one takes a
    /// free (released) slot, else a new body is asked for (one a frame, `max` at most, only when `spawn_ok`).
    pub fn plan(slots: &[SlotView], wants: &[WantView], spawn_ok: bool, max: usize) -> Vec<Action> {
        let mut out = Vec::new();
        let alive = |s: &SlotView| s.state != SlotState::Lost;
        for (i, s) in slots.iter().enumerate() {
            if !alive(s) {
                out.push(Action::Drop(i));
            }
        }
        let wanted = |c: PartyChara| wants.iter().any(|w| w.chara == c);
        let mut free: Vec<usize> = Vec::new();
        for (i, s) in slots.iter().enumerate() {
            if !alive(s) {
                continue;
            }
            match s.chara {
                Some(c) if !wanted(c) => {
                    out.push(Action::Release(i));
                    free.push(i);
                }
                None => free.push(i),
                _ => {}
            }
        }
        let count = slots.iter().filter(|s| alive(s)).count();
        let mut spawned = false;
        for w in wants {
            match slots.iter().position(|s| alive(s) && s.chara == Some(w.chara)) {
                Some(i) => match slots[i].state {
                    SlotState::Parked if w.hittable => out.push(Action::Activate(i)),
                    SlotState::Live if !w.hittable => out.push(Action::Park(i)),
                    _ => {}
                },
                None => {
                    if let Some(k) = free.iter().position(|i| slots[*i].state != SlotState::Spawning) {
                        let i = free.remove(k);
                        out.push(Action::Reuse(i, w.chara));
                        if w.hittable {
                            out.push(Action::Activate(i));
                        }
                    } else if spawn_ok && !spawned && count < max {
                        out.push(Action::Spawn(w.chara));
                        spawned = true;
                    }
                }
            }
        }
        out
    }

    /// What a body's HP says since we last wrote it.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum HpRead {
        Nothing,
        /// HP at the game's maximum but below what we wrote: the game clamped it to a maximum it computed afresh,
        /// NEVER a hit (Sora's rule): the new value is what we hold.
        Clamp(i32),
        /// So many HP were taken.
        Lost(i32),
    }

    /// Sora's damage-in reading applied to a body (sora.rs `damage_in`).
    pub fn read_hp(written: Option<i32>, hp: i32, max_now: i32) -> HpRead {
        let Some(written) = written else { return HpRead::Nothing };
        if hp >= max_now && hp < written {
            return HpRead::Clamp(hp);
        }
        if hp >= written || hp < 0 {
            return HpRead::Nothing;
        }
        HpRead::Lost(written - hp)
    }

    /// THE ONE-SHOT RULE: the base HP the body is held at (k times its own; never below its own).
    pub fn raised_base(own_base: i32, k: f32) -> i32 {
        ((own_base as f32 * k.max(1.0)).round() as i32).clamp(own_base.max(1), 9_999_999)
    }

    /// (ER party I, S5's request) THE GROUND UNDER A BODY for the friend's footsteps (fx.rs `member_surface`): the game's
    /// `hit_material` of the body's physics module (its override first, as fx.rs reads the player's). The bodies are
    /// written to the friends' feet with map collision off, so the game may never update that reading: it is TRUSTED
    /// only once it has been seen to CHANGE while the body was out (a frozen reading never speaks: the leader's ground
    /// stays, as before). Parked / not live = nothing.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct GroundRead {
        first: Option<i32>,
        trusted: bool,
        now: Option<i32>,
    }

    impl GroundRead {
        /// One frame's reading (`None` = the body is parked / not live: the reading is not his ground).
        pub fn feed(&mut self, material: Option<i32>) -> Option<i32> {
            self.now = None;
            let m = material.filter(|m| *m >= 0)?;
            match self.first {
                None => self.first = Some(m),
                Some(f) if f != m => self.trusted = true,
                _ => {}
            }
            self.now = self.trusted.then_some(m);
            self.now
        }
        /// His ground for the footsteps (`None`: the leader's).
        pub fn ground(&self) -> Option<i32> {
            self.now
        }
        pub fn trusted(&self) -> bool {
            self.trusted
        }
    }

    /// ONE HIT ON AN AI ALLY, as the pool read it (contract ER.5 "AllyHit"); the amount is made at delivery from the
    /// friend's own maximum (the share rule, U8).
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct AllyHit {
        pub chara: PartyChara,
        /// HP the Elden Ring body lost.
        pub lost: i32,
        /// The reference maximum the share is taken of (the body's own maximum, or `body.ref_hp`).
        pub ref_max: i32,
        pub class: Loss,
        pub causer: TargetId,
        /// The attacker (KH3 space) and its facing / radius; `known` = named by the game's `last_hit_by`.
        pub from: Vec3,
        pub causer_yaw: f32,
        pub causer_radius: f32,
        pub known: bool,
        /// Unit, horizontal, from the attacker to the friend.
        pub direction: Vec3,
        /// `status.burn_flinch` (a loss below the minimum share still flinches; Sora's rule).
        pub burn_flinch: bool,
    }

    impl AllyHit {
        /// (a status tick, the loss taken at the reference maximum, a proc)
        fn parts(&self) -> (bool, i32, bool) {
            match self.class {
                Loss::Tick { own_loss, .. } => (true, own_loss, false),
                Loss::Proc { pct, flat, .. } => (false, sr::proc_own_loss(pct, flat, self.ref_max), true),
                Loss::Hit => (false, self.lost, false),
            }
        }

        /// What the friend loses (the same share of HIS maximum; a proc's percent of his maximum).
        pub fn amount(&self, p: &Params, friend_max: i32) -> f32 {
            match self.class {
                Loss::Proc { pct, flat, .. } => sr::proc_amount(p, pct, flat, self.ref_max, friend_max),
                _ => p.damage_in(self.parts().1, self.ref_max, friend_max),
            }
        }

        /// The reaction kind (`None`: a status tick or damage over time, no reaction).
        pub fn kind(&self, p: &Params) -> Option<ETresDamageKind> {
            let (tick, taken, proc) = self.parts();
            if tick {
                return None;
            }
            p.kind_in(taken, self.ref_max).or_else(|| (proc || self.burn_flinch).then_some(ETresDamageKind::Small))
        }

        /// The friend's hit record (`Party::receive_hit`: the damage is final). `at` = his position.
        pub fn record(&self, p: &Params, friend_max: i32, at: Vec3) -> HitRecord {
            let kind = self.kind(p);
            let mut record = HitRecord {
                position: at + Vec3::Z * 60.0,
                direction: self.direction,
                damage: self.amount(p, friend_max),
                damage_kind: kind.unwrap_or(ETresDamageKind::Small),
                attack_name: "er.body",
                attacker: if self.known || self.causer != TargetId::NONE { HitAttacker { position: self.from, yaw: self.causer_yaw, radius: self.causer_radius, valid: true } } else { HitAttacker::NONE },
                ..HitRecord::empty(self.causer)
            };
            if kind.is_none() {
                // (damage over time: the HP without a reaction)
                record.info.victim_result = ReactionResult::NoReaction;
            }
            record
        }

        /// AI Sora's hit (`Sora::take_hit`), exactly as sora.rs `damage_in` builds his own.
        pub fn sora_hit(&self, p: &Params, sora_max: i32) -> kh_core::states::HitOnPlayer {
            let kind = self.kind(p);
            kh_core::states::HitOnPlayer { direction: self.direction, causer_yaw: self.causer_yaw, causer_radius: self.causer_radius, reaction_type: if kind.is_some() { 2 } else { 1 }, ..kh_core::states::HitOnPlayer::new(kind.unwrap_or(ETresDamageKind::Small), self.amount(p, sora_max), self.causer, self.from) }
        }
    }

    /// Which body route.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum BodyRoute {
        Buddy,
        C0000,
        Copy,
    }

    impl BodyRoute {
        pub fn name(self) -> &'static str {
            match self {
                BodyRoute::Buddy => "buddy",
                BodyRoute::C0000 => "c0000",
                BodyRoute::Copy => "copy",
            }
        }
        pub fn parse(s: &str) -> Option<BodyRoute> {
            match s.trim() {
                "buddy" => Some(BodyRoute::Buddy),
                "c0000" => Some(BodyRoute::C0000),
                "copy" => Some(BodyRoute::Copy),
                _ => None,
            }
        }
        pub fn index(self) -> usize {
            self as usize
        }
    }

    /// A Spirit Ash row as the game has it (BuddyParam + its NpcParam).
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct BuddyRow {
        pub id: u32,
        pub npc_param: i32,
        pub think: i32,
        /// `npc_player_init_param_id` (> 0: a c0000-based spirit).
        pub init: i32,
        pub team: u8,
        /// NpcParam hit height / radius (m).
        pub height: f32,
        pub radius: f32,
        pub hp: u32,
        /// The highest of its ailment resistances.
        pub resist: u16,
        /// (AGGRO) All seven (poison, rot, blood, curse, frost, sleep, madness).
        pub resists: [u16; 7],
    }

    /// (AGGRO) How many of a row's ailments are at or above `immune` (Elden Ring writes 999 for "immune").
    pub fn immune_count(row: &BuddyRow, immune: u16) -> usize {
        row.resists.iter().filter(|v| **v >= immune).count()
    }

    /// The character id the creator is asked for: c0000 for a spirit built on the player's model, else the NpcParam
    /// id's chr digits (Elden Ring's NpcParam ids are chr id x 10000 + a variant: 41109010 = c4110; (AGGRO) the Spirit
    /// Ash rows have a leading 1: 121000000 = c2100, 144400000 = c4440: the four digits above the variant, checked
    /// against the game's archive names offline, IMPL_AGGRO_STATE.md).
    pub fn chr_of(row: &BuddyRow) -> i32 {
        if row.init > 0 {
            0
        } else {
            (row.npc_param / 10_000) % 10_000
        }
    }

    /// The Spirit Ash team: the commonest team type of the rows' NpcParams (never hard-coded; research 1.1).
    pub fn spirit_team(rows: &[BuddyRow]) -> Option<u8> {
        let mut count = std::collections::BTreeMap::<u8, u32>::new();
        for r in rows.iter().filter(|r| r.team != 0) {
            *count.entry(r.team).or_insert(0) += 1;
        }
        count.into_iter().max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0))).map(|(t, _)| t)
    }

    /// THE AUTO PICK: of the rows with the spirit team and a real model (chr id > 0 or a c0000 appearance), the one whose
    /// hit height is nearest to `height`, then (AGGRO) the FEWEST IMMUNE AILMENTS, then the lower id. (AGGRO) The rule
    /// before asked for no immune ailment at all, and no Spirit Ash of the game has that: all 154 rows of the 2.7.1.0 /
    /// 2.7.2.0 regulation are immune to blood loss and frost (read offline: 4 to 7 immunities each), so the buddy and
    /// c0000 routes could never make a body (the user's log: "no Spirit Ash row of the spirit team with normal
    /// resistances").
    pub fn auto_pick(rows: &[BuddyRow], height: f32, team: u8, immune: u16) -> Option<BuddyRow> {
        rows.iter()
            .filter(|r| r.team == team && r.npc_param > 0 && r.height > 0.1 && (chr_of(r) > 0 || r.init > 0))
            .min_by(|a, b| (a.height - height).abs().total_cmp(&(b.height - height).abs()).then(immune_count(a, immune).cmp(&immune_count(b, immune))).then(a.id.cmp(&b.id)))
            .copied()
    }

    /// `body.<who>.buddy`.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum Pick {
        Auto,
        Row(u32),
    }

    /// The sheet `party-er-bodies` (defaults = its rows as authored).
    #[derive(Clone, Debug, PartialEq)]
    pub struct Sheet {
        pub max: usize,
        pub order: Vec<BodyRoute>,
        /// By `PartyChara as usize` (Sora, Donald, Goofy).
        pub buddy: [Pick; 3],
        pub height: [f32; 3],
        pub c0000_npc: Option<i32>,
        pub c0000_init: Option<i32>,
        pub spawn_timeout: f32,
        pub respawn_delay: f32,
        pub retry: f32,
        pub hp_raise: f32,
        pub ref_hp: i32,
        pub team: Option<u8>,
        pub park_team: u8,
        pub target_check: f32,
        pub target_fails: u32,
        pub immune: u16,
        /// A parked body (KO, cutscene, free) is held this far below the leader (m; 0 = at his feet).
        pub park_depth: f32,
        pub owner_match: f32,
        pub test_offset: f32,
        pub test_report: Vec<f32>,
        /// Go/no-go item 13: an attack aimed at a knocked-out friend later than this after the KO fails it (s).
        pub ko_grace: f32,
    }

    impl Default for Sheet {
        fn default() -> Self {
            Sheet {
                max: 3,
                order: vec![BodyRoute::Buddy, BodyRoute::C0000, BodyRoute::Copy],
                buddy: [Pick::Auto; 3],
                height: [1.8, 1.0, 1.8],
                c0000_npc: None,
                c0000_init: None,
                spawn_timeout: 5.0,
                respawn_delay: 0.25,
                retry: 30.0,
                hp_raise: 20.0,
                ref_hp: 0,
                team: None,
                park_team: 0,
                target_check: 2.0,
                target_fails: 3,
                immune: 900,
                park_depth: 30.0,
                owner_match: 80.0,
                test_offset: 3.0,
                test_report: vec![10.0, 30.0, 60.0],
                ko_grace: 3.0,
            }
        }
    }

    impl Sheet {
        /// The sheet's JSON (rows `[id, value, ...]`): the sheet and the rows not understood.
        pub fn from_text(text: &str) -> Result<(Sheet, Vec<String>), String> {
            let v: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
            let rows = v.get("rows").and_then(|r| r.as_array()).ok_or("no rows")?;
            let mut s = Sheet::default();
            let mut bad = Vec::new();
            for row in rows {
                let (Some(id), Some(value)) = (row.get(0).and_then(|v| v.as_str()), row.get(1)) else {
                    bad.push(format!("{row}"));
                    continue;
                };
                let f = || value.as_f64().map(|v| v as f32);
                let opt_i = || if value.is_null() { Some(None) } else { value.as_i64().map(|v| Some(v as i32)) };
                let who = |c: PartyChara| c as usize;
                let pick = || match value.as_str() {
                    Some("auto") => Some(Pick::Auto),
                    _ => value.as_u64().map(|v| Pick::Row(v as u32)),
                };
                let ok = match id {
                    "body.max" => value.as_u64().map(|v| s.max = v as usize).is_some(),
                    "body.order" => value.as_array().map(|a| s.order = a.iter().filter_map(|r| r.as_str().and_then(BodyRoute::parse)).collect()).is_some(),
                    "body.donald.buddy" => pick().map(|p| s.buddy[who(PartyChara::Donald)] = p).is_some(),
                    "body.goofy.buddy" => pick().map(|p| s.buddy[who(PartyChara::Goofy)] = p).is_some(),
                    "body.sora.buddy" => pick().map(|p| s.buddy[who(PartyChara::Sora)] = p).is_some(),
                    "body.donald.height" => f().map(|v| s.height[who(PartyChara::Donald)] = v).is_some(),
                    "body.goofy.height" => f().map(|v| s.height[who(PartyChara::Goofy)] = v).is_some(),
                    "body.sora.height" => f().map(|v| s.height[who(PartyChara::Sora)] = v).is_some(),
                    "body.c0000.npc_param" => opt_i().map(|v| s.c0000_npc = v).is_some(),
                    "body.c0000.chara_init" => opt_i().map(|v| s.c0000_init = v).is_some(),
                    "body.spawn_timeout" => f().map(|v| s.spawn_timeout = v).is_some(),
                    "body.respawn_delay" => f().map(|v| s.respawn_delay = v).is_some(),
                    "body.retry" => f().map(|v| s.retry = v).is_some(),
                    "body.hp_raise" => f().map(|v| s.hp_raise = v).is_some(),
                    "body.ref_hp" => value.as_i64().map(|v| s.ref_hp = v as i32).is_some(),
                    "body.team" => (if value.is_null() { Some(None) } else { value.as_u64().map(|v| Some(v as u8)) }).map(|v| s.team = v).is_some(),
                    "body.park_team" => value.as_u64().map(|v| s.park_team = v as u8).is_some(),
                    "body.target_check" => f().map(|v| s.target_check = v).is_some(),
                    "body.target_fails" => value.as_u64().map(|v| s.target_fails = v as u32).is_some(),
                    "body.immune" => value.as_u64().map(|v| s.immune = v as u16).is_some(),
                    "body.park_depth" => f().map(|v| s.park_depth = v).is_some(),
                    "test.ko_grace" => f().map(|v| s.ko_grace = v).is_some(),
                    "owner.match" => f().map(|v| s.owner_match = v).is_some(),
                    "test.offset" => f().map(|v| s.test_offset = v).is_some(),
                    "test.report" => value.as_array().map(|a| s.test_report = a.iter().filter_map(|t| t.as_f64().map(|t| t as f32)).collect()).is_some(),
                    _ => false,
                };
                if !ok {
                    bad.push(id.to_string());
                }
            }
            Ok((s, bad))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use SlotState::*;

        fn slot(chara: Option<PartyChara>, state: SlotState) -> SlotView {
            SlotView { chara, state }
        }
        fn want(chara: PartyChara, hittable: bool) -> WantView {
            WantView { chara, hittable }
        }

        /// (ER party I) a frozen reading never speaks; one that changes is his ground; parked = nothing.
        #[test]
        fn erpi_a_bodys_ground_speaks_only_once_it_has_changed() {
            let mut g = GroundRead::default();
            assert_eq!((g.feed(Some(3)), g.feed(Some(3)), g.feed(Some(-1)), g.feed(None)), (None, None, None, None));
            assert!(!g.trusted() && g.ground().is_none());
            assert_eq!(g.feed(Some(104)), Some(104));
            assert_eq!(g.feed(Some(3)), Some(3));
            assert_eq!((g.feed(None), g.ground()), (None, None));
            assert_eq!(g.feed(Some(3)), Some(3));
        }

        #[test]
        fn one_spawn_a_frame_up_to_the_maximum() {
            let wants = [want(PartyChara::Donald, true), want(PartyChara::Goofy, true)];
            assert_eq!(plan(&[], &wants, true, 3), vec![Action::Spawn(PartyChara::Donald)]);
            // Donald's spawn in flight: Goofy next
            assert_eq!(plan(&[slot(Some(PartyChara::Donald), Spawning)], &wants, true, 3), vec![Action::Spawn(PartyChara::Goofy)]);
            // not while spawning is not allowed (a spawn in flight, the world not ready)
            assert_eq!(plan(&[slot(Some(PartyChara::Donald), Live)], &wants, false, 3), vec![]);
            // the maximum
            assert_eq!(plan(&[slot(Some(PartyChara::Donald), Live)], &wants, true, 1), vec![]);
        }

        #[test]
        fn a_lost_body_is_dropped_and_remade_the_friend_stays() {
            let wants = [want(PartyChara::Donald, true)];
            assert_eq!(plan(&[slot(Some(PartyChara::Donald), Lost)], &wants, true, 3), vec![Action::Drop(0), Action::Spawn(PartyChara::Donald)]);
        }

        #[test]
        fn ko_parks_revive_activates_and_a_free_slot_is_reused() {
            assert_eq!(plan(&[slot(Some(PartyChara::Goofy), Live)], &[want(PartyChara::Goofy, false)], true, 3), vec![Action::Park(0)]);
            assert_eq!(plan(&[slot(Some(PartyChara::Goofy), Parked)], &[want(PartyChara::Goofy, true)], true, 3), vec![Action::Activate(0)]);
            // Goofy left the party, AI Sora came in: Goofy's body is released and given to Sora (no spawn)
            assert_eq!(plan(&[slot(Some(PartyChara::Goofy), Live)], &[want(PartyChara::Sora, true)], true, 3), vec![Action::Release(0), Action::Reuse(0, PartyChara::Sora), Action::Activate(0)]);
            // a free slot still spawning is not given away; a spawn is asked instead
            assert_eq!(plan(&[slot(None, Spawning)], &[want(PartyChara::Sora, true)], true, 3), vec![Action::Spawn(PartyChara::Sora)]);
            // nobody wanted: released, never despawned
            assert_eq!(plan(&[slot(Some(PartyChara::Donald), Live)], &[], true, 3), vec![Action::Release(0)]);
        }

        #[test]
        fn hp_top_up_arithmetic_and_the_clamp_is_no_hit() {
            assert_eq!(read_hp(None, 100, 100), HpRead::Nothing);
            assert_eq!(read_hp(Some(1000), 940, 1000), HpRead::Lost(60));
            assert_eq!(read_hp(Some(1000), 1000, 1000), HpRead::Nothing);
            // the game made its maximum 800 and clamped the HP to it: NOT a hit of 200
            assert_eq!(read_hp(Some(1000), 800, 800), HpRead::Clamp(800));
            assert_eq!(read_hp(Some(1000), -5, 1000), HpRead::Nothing);
            assert_eq!(raised_base(500, 20.0), 10_000);
            assert_eq!(raised_base(500, 0.5), 500);
            assert_eq!(raised_base(1_000_000, 20.0), 9_999_999);
        }

        #[test]
        fn a_body_loss_becomes_the_same_share_of_the_friends_maximum() {
            let p = Params::default();
            let hit = AllyHit { chara: PartyChara::Donald, lost: 100, ref_max: 1000, class: Loss::Hit, causer: TargetId(4), from: Vec3::new(-100.0, 0.0, 0.0), causer_yaw: 0.0, causer_radius: 50.0, known: true, direction: Vec3::X, burn_flinch: true };
            let amount = hit.amount(&p, 200);
            assert!((amount - p.damage_in(100, 1000, 200)).abs() < 1e-4);
            let r = hit.record(&p, 200, Vec3::ZERO);
            assert_eq!(r.damage, amount);
            assert!(r.attacker.valid && r.takes_hp());
            assert_eq!(r.damage_kind, p.kind_in(100, 1000).unwrap_or(ETresDamageKind::Small));
            // a status tick: HP without a reaction (NoReaction still takes HP)
            let tick = AllyHit { class: Loss::Tick { ailment: 0, own_loss: 10, dot: None }, ..hit };
            assert_eq!(tick.kind(&p), None);
            let r = tick.record(&p, 200, Vec3::ZERO);
            assert_eq!(r.info.victim_result, ReactionResult::NoReaction);
            assert!(r.takes_hp());
            assert_eq!(tick.sora_hit(&p, 300).reaction_type, 1);
            assert_eq!(hit.sora_hit(&p, 300).reaction_type, 2);
        }

        #[test]
        fn the_auto_pick_and_the_spirit_team() {
            let row = |id, npc, team, height, resist, init| BuddyRow { id, npc_param: npc, think: 0, init, team, height, radius: 0.4, hp: 500, resist, resists: [resist; 7] };
            let rows = [row(1, 40700010, 47, 1.0, 100, 0), row(2, 43110010, 47, 1.9, 100, 0), row(3, 46000010, 47, 1.1, 999, 0), row(4, 52300010, 47, 1.75, 100, 1000), row(5, 41000010, 9, 1.0, 100, 0)];
            assert_eq!(spirit_team(&rows), Some(47));
            assert_eq!(auto_pick(&rows, 1.0, 47, 900).map(|r| r.id), Some(1));
            assert_eq!(auto_pick(&rows, 1.8, 47, 900).map(|r| r.id), Some(4));
            assert_eq!(chr_of(&rows[3]), 0);
            assert_eq!(chr_of(&rows[1]), 4311);
            assert_eq!(auto_pick(&rows, 1.0, 12, 900), None);
        }

        /// (AGGRO) The game's own Spirit Ash rows as the 2.7.2.0 regulation has them (read offline): every row is immune
        /// to 4 or more ailments and their NpcParam hit heights are all 2.2 - 2.4 m. The pick must still give a row
        /// (the fewest immunities), and the 9-digit Spirit Ash NpcParam ids give the 4-digit model the archives hold.
        #[test]
        fn aggro_the_auto_pick_finds_a_spirit_ash_in_the_games_own_rows() {
            let row = |id, npc, init, resists: [u16; 7]| BuddyRow { id, npc_param: npc, think: npc, init, team: 47, height: 2.2, radius: 0.5, hp: 900, resist: *resists.iter().max().unwrap(), resists };
            let rows = [
                row(20000000, 121000000, -1, [999; 7]),
                row(20400000, 131700000, -1, [999, 999, 999, 999, 999, 112, 999]),
                row(20700001, 100000010, 26050, [999, 999, 999, 112, 999, 112, 112]),
                row(24400000, 144400000, -1, [999, 999, 999, 999, 999, 112, 112]),
            ];
            for h in [1.0, 1.8] {
                let p = auto_pick(&rows, h, 47, 900).expect("a body row");
                assert_eq!(p.id, 20700001, "the fewest immunities (4)");
                assert_eq!(immune_count(&p, 900), 4);
            }
            assert_eq!(chr_of(&rows[0]), 2100);
            assert_eq!(chr_of(&rows[3]), 4440);
            assert_eq!(chr_of(&rows[2]), 0, "a c0000 spirit");
            assert_eq!(chr_of(&BuddyRow { npc_param: 43113010, init: -1, ..rows[0] }), 4311, "an 8-digit enemy row as before");
        }

        #[test]
        fn the_sheet_rows_are_all_understood_and_are_the_defaults() {
            let text = include_str!("../../../sheets/port/party-er-bodies.json");
            let (sheet, bad) = Sheet::from_text(text).unwrap();
            assert!(bad.is_empty(), "rows not understood: {bad:?}");
            assert_eq!(sheet, Sheet::default());
        }
    }
}

// The game half (pointers, the debug creator, the per-frame writes): its own file.
#[cfg(windows)]
#[path = "party/allybody_live.rs"]
mod live;
#[cfg(windows)]
pub use live::*;
