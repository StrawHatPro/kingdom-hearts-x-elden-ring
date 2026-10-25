//! OWNER: party stream D (team attacks). Sora's side of the party's team attacks: the friend
//! gauge on the player controller, the team-command offers of the friends, the unison / fusion
//! roll of the combo, and what Sora's FR states share (`FrLink`). Contract:
//! `docs/party-contracts.md` section D; plan `docs/party-port-plan.md` 2.6, stream D.
//!
//! | game | here |
//! |---|---|
//! | ctrl+0xC0C gauge, +0xC10 pending points, +0xC14 out-of-battle timer | [`FriendGauge`] |
//! | ctrl slot 899 0x1413E8D70 (points of a damaging hit; called by `AddStylePoint`) | [`FriendState::gauge_add`] |
//! | ctrl slot 900 0x1413FCC10 (tick: pending -> gauge, cap `MyFriendPointMax`, timer) | [`tick`] |
//! | ctrl slot 896 0x141402C10 (gauge full) | [`FriendState::gauge_full`] |
//! | ctrl slot 901 0x141411090 (reset) | [`reset`] |
//! | a friend's `NotifySetActionCommand(kind, time, info = friend)` (NPC 0x141279010 -> ctrl slot 687; team branch of 0x14140DA40) | [`offer`] |
//! | pawn slot 1304 0x141430A60 (unison roll) / 1303 0x141431A80 (clear) | [`unison_roll`] |
//! | `PlayAttack` cases 0x3F..0x43 (0x1413378B0) and attack slots 137 / 138 (shots) | [`play_unison`], [`unison_shot`] |
//! | executor 0x14141B470 for kinds 193 / 195 / 207 / 208 -> the FR states | [`try_team_attack`] |
//!
//! Not ported: the anti form's exclusions (ctrl+0x888 == 0x8B: no anti form in the port).

use crate::anim::{AnimId, Notifies, NotifyClass, NotifyData, NotifyPhase, PlayArgs};
use crate::enums::ETresAbilityKind as Ab;
use crate::host::targets::{Friend, FriendLinkRequest, FriendLinkView};
use crate::host::{EventKind, TargetId};
use crate::input::PadButton;
use crate::pawn::Ctx;
use crate::sora::Input;
use crate::states::{StateData, StateId};
use glam::Vec3;

/// `MyFriendPointMax` (pawn prop +0xCF4, int) [A: `p_ex001_PawnBase` CDO, plan 2.6].
pub const FRIEND_POINT_MAX: i32 = 200;
/// `MyFriendPointTime` (pawn prop +0xCF8) [A].
pub const FRIEND_POINT_TIME: f32 = 30.0;
/// The unison roll's numbers (`ATresPlayerPawnSora` constructor 0x1413E7580 lines 243 - 249) [C]:
/// `MyFriendComboFinishRange` / `RangeHalf` / `RangeNear` (cm), `AddRate`, `AddRateRange` /
/// `RangeHalf` / `RangeNear`.
pub const COMBO_FINISH_RANGE: [f32; 3] = [800.0, 550.0, 350.0];
pub const COMBO_FINISH_ADD_RATE: i32 = 20;
pub const COMBO_FINISH_ADD_RATE_RANGE: [i32; 3] = [0, 20, 60];
/// Sora's actor location (KH3 `GetActorLocation`, the capsule centre) above his feet (kh-core's
/// position): lib.rs "the collision capsule has its centre 91 cm above it".
pub const ACTOR_CENTRE_Z: f32 = 91.0;

/// The friend gauge of the player controller.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct FriendGauge {
    /// ctrl+0xC0C: the gauge, 0 .. `MyFriendPointMax`.
    pub points: f32,
    /// ctrl+0xC10: points added since the last tick (slot 899), moved into the gauge by slot 900.
    pub pending: f32,
    /// ctrl+0xC14: reloaded with `MyFriendPointTime` by each gain; counts down out of battle; at 0
    /// the gauge resets (slot 901).
    pub timer: f32,
}

/// Sora's team-attack bookkeeping (pawn / controller fields; `PawnCombat::friend`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FriendState {
    pub gauge: FriendGauge,
    /// pawn+0x1464: 0 = none, else the unison command kind (22..26) the next attack plays.
    pub unison: u8,
    /// pawn+0x1468: the roll's chance in percent (100 at the constructor 0x141423210; halved by
    /// each success, back to 100 by each failed roll).
    pub unison_chance: i32,
    /// pawn+0x1470: which of the five unison kinds were played since the list was last full.
    pub unison_used: [bool; 5],
    /// The friends as the host answered `Targets::friends` this step (party-slot order).
    pub friends: [Option<Friend>; 2],
    /// The command kind of the running team attack (0 = none).
    pub running: i32,
    /// The input the controller passed for the running FR state (taken by its tick).
    pub input: (Vec3, bool, bool, bool),
    /// The per-pawn stream for the unison roll's `rand()` (plan N6).
    pub rng: u32,
    /// Offers taken / unisons played (tests, debug text).
    pub offers_taken: u32,
    pub unisons: u32,
    /// (D3, leader-agnostic team attacks) seconds left for a unison / fusion rolled on a
    /// player-controlled FRIEND leader's combo (FriendState::leader_unison): Sora's core starts
    /// it as his own attack (rule 7 without his combo) while > 0.
    pub leader_unison: f32,
    /// (D3) Sora is AI-controlled (a friend leads; stream P sets it each step): his own combos
    /// never roll a unison (the user's rule: team attacks are started only by the player's leader).
    pub sora_is_ai: bool,
}

impl Default for FriendState {
    fn default() -> FriendState {
        FriendState { gauge: FriendGauge::default(), unison: 0, unison_chance: 100, unison_used: [false; 5], friends: [None; 2], running: 0, input: (Vec3::ZERO, false, false, false), rng: 0x1234_5678, offers_taken: 0, unisons: 0, leader_unison: 0.0, sora_is_ai: false }
    }
}

impl FriendState {
    /// (D3, leader-agnostic team attacks; contract D.6) The unison / fusion roll for a combo of a
    /// player-controlled FRIEND leader (Donald `leader_uid` 11 / Goofy 12), Sora being his
    /// partner: the law of `unison_roll` (pawn slot 1304 0x141402C10's numbers: base = 20 x 10 x
    /// the five abilities' count, + 0 / 20 / 60 x 10 by the leader - Sora distance at 800 / 550 /
    /// 350, x chance / 100 against rand() % 1000; chance halved on success, 100 on a failure; the
    /// used list), with only the leader's own kinds (Donald 22..24, Goofy 25..26). The leader's
    /// stream calls it at its combo's step before the finisher (P: Beat -> Swing -> JumpAttack) and
    /// skips its own finisher when it returns true; Sora's core then plays the unison as his attack
    /// (rule 7) within 0.5 s and the friend enters FRDonaldSora / FRGoofySora on him [D: the
    /// game has no friend leader].
    pub fn roll_leader_unison(&mut self, abilities: &crate::status::AbilitySet, leader_uid: u8, leader_pos: Vec3, sora_pos: Vec3) -> bool {
        if self.unison != 0 || self.running != 0 || self.leader_unison > 0.0 || !(leader_uid == 11 || leader_uid == 12) {
            return false;
        }
        let c = [abilities.count(Ab::UnisonFire), abilities.count(Ab::UnisonBlizzard), abilities.count(Ab::UnisonThunder), abilities.count(Ab::FusionSpin), abilities.count(Ab::FusionRocket)].map(|n| n as i32);
        let base = c.iter().sum::<i32>() * COMBO_FINISH_ADD_RATE * 10;
        let (lo, hi) = if leader_uid == 11 { (0usize, 3usize) } else { (3, 5) };
        if base == 0 || c[lo..hi].iter().sum::<i32>() == 0 {
            return false;
        }
        let d = (leader_pos - sora_pos).length();
        let out = if leader_uid == 11 { d > COMBO_FINISH_RANGE[0] } else { d >= COMBO_FINISH_RANGE[0] };
        if out {
            return false;
        }
        let rate = if d <= COMBO_FINISH_RANGE[1] { if d <= COMBO_FINISH_RANGE[2] { COMBO_FINISH_ADD_RATE_RANGE[2] } else { COMBO_FINISH_ADD_RATE_RANGE[1] } } else { COMBO_FINISH_ADD_RATE_RANGE[0] };
        if self.rand() % 1000 < (base + rate * 10) * self.unison_chance / 100 {
            self.unison_chance /= 2;
            let list: Vec<u8> = (lo..hi).filter(|k| c[*k] != 0 && !self.unison_used[*k]).map(|k| 22 + k as u8).collect();
            if list.is_empty() {
                self.unison_used = [false; 5];
                self.unison = 0;
                return false;
            }
            let kind = list[(self.rand() as usize) % list.len()];
            self.unison = kind;
            self.unison_used[(kind - 22) as usize] = true;
            self.leader_unison = 0.5;
            true
        } else {
            self.unison_chance = 100;
            false
        }
    }

    /// Slot 899 (0x1413E8D70): `pending += amount`; the timer is reloaded with `MyFriendPointTime`.
    pub fn gauge_add(&mut self, amount: f32) {
        self.gauge.pending += amount;
        self.gauge.timer = FRIEND_POINT_TIME;
    }

    /// Slot 896 (0x141402C10): the gauge is full (`MyFriendPointMax <= gauge`).
    pub fn gauge_full(&self) -> bool {
        FRIEND_POINT_MAX as f32 <= self.gauge.points
    }

    /// DEBUG (`team-gauge full`): the gauge to its maximum and the timer reloaded.
    pub fn debug_fill_gauge(&mut self) {
        self.gauge.points = FRIEND_POINT_MAX as f32;
        self.gauge.timer = FRIEND_POINT_TIME;
    }

    /// The friend of unique id `uid`, if present this step.
    pub fn friend(&self, uid: u8) -> Option<&Friend> {
        self.friends.iter().flatten().find(|f| f.uid == uid)
    }

    fn rand(&mut self) -> i32 {
        // xorshift32; the game's CRT rand() returns 0..32767.
        let mut x = self.rng.max(1);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        ((x >> 8) & 0x7FFF) as i32
    }
}

/// Slot 901 (0x141411090): gauge and timer to 0.
pub(crate) fn reset(cx: &mut Ctx) {
    cx.pawn.combat.friend.gauge.points = 0.0;
    cx.pawn.combat.friend.gauge.timer = 0.0;
}

/// The UI-phase part of the step (called first by `controller::ui_attack_request`): the friends'
/// rows, slot 900, the gauge state to the friends' side, their offers. Returns true while an FR
/// state runs (the command menu and the situation queue wait: the HUD hides the menu while a
/// class-7 command executes, combat spec 11.5).
pub(crate) fn tick(state: &StateData, cx: &mut Ctx, dt: f32) -> bool {
    // The friends of this step.
    let mut rows = Vec::new();
    cx.host.targets.friends(&mut rows);
    let mut friends = [None; 2];
    for (slot, row) in friends.iter_mut().zip(rows.iter()) {
        *slot = Some(*row);
    }
    cx.pawn.combat.friend.friends = friends;
    // (D3) The link pawns' hits as Sora's shots every step, in or out of his FR state: their
    // projectiles (GoofyShoot_Proj, the missiles' blasts) are spawned through the player's
    // spawner and outlive his FR state (the link pawn lingers after releasing him).
    for f in friends.iter().flatten() {
        for shot in f.link.shots.iter().flatten() {
            super::hit::spawn_team_shot(cx, shot);
        }
    }
    // Slot 900 (0x1413FCC10): not while the form command 0x9A runs (ctrl vt+0x1A70; the ctrl
    // vt+0x1BF0 test is not identified and taken as false); the player controls the pawn.
    let in_battle = cx.pawn.actions.battle;
    if cx.pawn.combat.form.cmd != 0x9A {
        let g = &mut cx.pawn.combat.friend.gauge;
        g.points = (g.points + g.pending).min(FRIEND_POINT_MAX as f32);
        g.pending = 0.0;
        if !in_battle && g.timer > 0.0 {
            g.timer -= dt;
            if g.timer <= 0.0 {
                reset(cx);
            }
        }
    } else {
        cx.pawn.combat.friend.gauge.pending = 0.0;
        reset(cx);
    }
    // To the friends' side: slot 896 for their link points (NPC slot 1265 0x1412585B0).
    let full = cx.pawn.combat.friend.gauge_full();
    let (position, yaw) = (cx.pawn.cmc.location, cx.pawn.yaw);
    request(cx, FriendLinkRequest::Sora { gauge_full: full, in_battle, position, yaw });
    // Their offers (`NotifySetActionCommand(kind, time, friend)`).
    for f in friends.iter().flatten() {
        if let Some((kind, time)) = f.offer {
            if offer(cx, kind, time, f.combi_time) {
                cx.pawn.combat.friend.offers_taken += 1;
            }
        }
    }
    in_fr_state(state)
}

/// (FX1) AI SORA CAN START A FRIEND LEADER'S UNISON / FUSION NOW (`Sora::can_start_leader_unison`; contract D.6, FX1).
/// The host rolls `FriendState::roll_leader_unison` only when this holds, and answers the pad "no" otherwise, so the
/// friend plays his own finisher instead of waiting on a team move that cannot start. [D: the game has no friend
/// leader; the conditions are what makes the latch's `TryAttack` turn into rule 7 within its first step]:
/// - Sora is not KO'd (HP >= 1) and the leader's friend row (when the host reports one) is not KO'd;
/// - GROUNDED IDLE / RUN: the states where `TryAttack` (slot 45 0x141437330) starts a fresh attack by a plain
///   SetState with no forced kind. Mid-action (an attack, a cast, a dodge, a style change or finish), in a
///   traversal state (its forced kinds 0x36..0x39 replace rule 7's kind) or in the air (the XS forced attacks, Diving
///   Strike out of a fall, replace it) the team move would not start;
/// - nothing queued, no style change / finish command in action (pawn+0xE2C 0x4B / 0xC0, `form::can_style_change`),
///   no command cool-down (pawn+0xE34, `can_attack`), no sneeze latch (rule 6 runs before rule 7 can);
/// - no team attack running, no unison request or latch already armed.
///
/// The distance rule (the move's start distance from the leader, `COMBO_FINISH_RANGE[0]`) stays the roll's own.
pub(crate) fn leader_unison_ready(state: &StateData, pawn: &crate::pawn::Pawn, leader_uid: u8) -> bool {
    let c = &pawn.combat;
    let f = &c.friend;
    pawn.status.hp >= 1
        && matches!(state.id(), StateId::Idle | StateId::Run)
        && pawn.cmc.on_ground()
        && pawn.queue.nothing_queued()
        && !matches!(c.form.command_in_action, 0x4B | 0xC0)
        && c.command_cooldown <= 0.0
        && !c.sneeze_next
        && f.running == 0
        && f.unison == 0
        && f.leader_unison <= 0.0
        && f.friend(leader_uid).is_none_or(|r| !r.ko)
}

/// One of Sora's FR states runs.
pub(crate) fn in_fr_state(state: &StateData) -> bool {
    matches!(state.id(), StateId::FrShieldGuardian | StateId::FrGoofyshoot | StateId::FrCallmeteor | StateId::FrFlareforce)
}

/// A friend's `NotifySetActionCommand(kind, time, info = friend)`: the team branch of
/// 0x14140DA40 (combat spec 11.2): `if time != -1: t = time`; `t *= 1 + 0.5 x COMBI_TIME of the
/// friend` (info+0x40); `ctrl.slot901()` (the gauge resets); `t = min(t, 99.9)`; the HUD insert.
pub(crate) fn offer(cx: &mut Ctx, kind: i32, time: f32, combi_time: u8) -> bool {
    if !matches!(kind, 193..=210 | 237..=240 | 242) {
        return false;
    }
    // -1: the row's own lifetime (TresCmdData Param0: 30 s for 193 / 195 / 207 / 208).
    let t = if time != -1.0 { time } else { super::tables::command_row(kind).map_or(-1.0, |r| r.param0) };
    let t = if t > 0.0 { t * (1.0 + 0.5 * combi_time as f32) } else { t };
    reset(cx);
    super::situation::enqueue(cx, kind, t)
}

/// Sends a request to the friends' side (the host relays it to `kh_party::team`).
pub(crate) fn request(cx: &mut Ctx, req: FriendLinkRequest) {
    cx.host.targets.friend_link(req);
}

/// Pawn slot 1304 (0x141430A60) [C; the early exit I]: the unison / fusion roll, called by
/// `Decide` when `combo + 1 == max - 1`.
///
/// ```text
/// if !player-controlled || !ctrl || ctrl+0x920 (sneeze): return
/// base = (count 190 + 191 + 192 + 193 + 194) x AddRate (20) x 10;  if base == 0: return
/// per friend (11 needs one of 190..192, 12 one of 193..194; not hidden, HP >= 1):
///     d = |friend - Sora| (3D);  out of range when d > Range (800; Goofy: d >= 800)
///     rate = d <= Near (350) ? 60 : d <= Half (550) ? 20 : 0     (the larger of the two friends)
/// if a friend is in range:
///     if rand() % 1000 < (base + rate x 10) x chance / 100:
///         chance /= 2;  list = the kinds 22..24 (Donald) / 25..26 (Goofy) equipped and not yet used
///         if list: pawn+0x1464 = list[rand() % n]; used[it] = 1; ctrl+0x898 = it (the HUD's name)
///         else: used = all 0; pawn+0x1464 = 0
///     else: chance = 100
/// ```
pub(crate) fn unison_roll(cx: &mut Ctx) {
    if cx.pawn.combat.sneeze_next || cx.pawn.combat.friend.sora_is_ai {
        return;
    }
    let ab = &cx.pawn.status.abilities;
    let c = [ab.count(Ab::UnisonFire), ab.count(Ab::UnisonBlizzard), ab.count(Ab::UnisonThunder), ab.count(Ab::FusionSpin), ab.count(Ab::FusionRocket)].map(|n| n as i32);
    let base = c.iter().sum::<i32>() * COMBO_FINISH_ADD_RATE * 10;
    if base == 0 {
        return;
    }
    let me = cx.pawn.cmc.location;
    let fs = cx.pawn.combat.friend;
    let rate_of = |d: f32| if d <= COMBO_FINISH_RANGE[1] { if d <= COMBO_FINISH_RANGE[2] { COMBO_FINISH_ADD_RATE_RANGE[2] } else { COMBO_FINISH_ADD_RATE_RANGE[1] } } else { COMBO_FINISH_ADD_RATE_RANGE[0] };
    let mut rate = 0;
    // Donald: `Range <= d && d != Range` is out.
    let donald = fs.friend(11).filter(|f| !f.ko && c[0] + c[1] + c[2] >= 1).map(|f| (f.position - me).length()).filter(|d| !(*d > COMBO_FINISH_RANGE[0]));
    if let Some(d) = donald {
        rate = rate_of(d);
    }
    // Goofy: `Range <= d` is out.
    let goofy = fs.friend(12).filter(|f| !f.ko && c[3] + c[4] >= 1).map(|f| (f.position - me).length()).filter(|d| *d < COMBO_FINISH_RANGE[0]);
    if let Some(d) = goofy {
        rate = rate.max(rate_of(d));
    }
    if donald.is_none() && goofy.is_none() {
        return;
    }
    let f = &mut cx.pawn.combat.friend;
    if f.rand() % 1000 < (base + rate * 10) * f.unison_chance / 100 {
        f.unison_chance /= 2;
        let mut list = Vec::new();
        if donald.is_some() {
            for (k, kind) in [22u8, 23, 24].into_iter().enumerate() {
                if c[k] != 0 && !f.unison_used[k] {
                    list.push(kind);
                }
            }
        }
        if goofy.is_some() {
            for (k, kind) in [25u8, 26].into_iter().enumerate() {
                if c[3 + k] != 0 && !f.unison_used[3 + k] {
                    list.push(kind);
                }
            }
        }
        if list.is_empty() {
            f.unison_used = [false; 5];
            f.unison = 0;
        } else {
            let kind = list[(f.rand() as usize) % list.len()];
            f.unison = kind;
            f.unison_used[(kind - 22) as usize] = true;
        }
    } else {
        f.unison_chance = 100;
    }
}

/// The attack kind of a unison request: 22 -> 0x3F .. 26 -> 0x43.
pub fn unison_attack_kind(unison: u8) -> u8 {
    unison.saturating_sub(22) + 0x3F
}

/// The shot of the attack-state slots 137 (0x14134CCE0: UNISON_FIRE / _BLIZZARD / _THUNDER for
/// 0x3F / 0x40 / 0x41) and 138 (0x14134C710: FUSION_SPIN).
pub(crate) fn unison_shot(kind: u8) -> &'static str {
    match kind {
        0x3F => "UNISON_FIRE",
        0x40 => "UNISON_BLIZZARD",
        0x41 => "UNISON_THUNDER",
        _ => "FUSION_SPIN",
    }
}

/// `PlayAttack` cases 0x3F..0x43: the clip, the friend's FR state on Sora's "vfx_root"
/// (`TresNpcState_n_ex001_FRDonaldSora` / `n_ex002_FRGoofySora`), facing the target (+0x1EC bit 0
/// -> pawn vt+0x140).
pub(crate) fn play_unison(cx: &mut Ctx, kind: u8, target: TargetId) {
    let clip = match kind {
        0x3F => "DoUFire",
        0x40 => "DoUBlizzard",
        0x41 => "DoUThunder",
        0x42 => "GooFSpin",
        _ => "GooFRocket",
    };
    if let Some(t) = cx.target(target) {
        let d = t.center - cx.pawn.cmc.location;
        if d.truncate().length_squared() > 1.0 {
            cx.pawn.yaw = d.y.atan2(d.x).to_degrees();
        }
    }
    match AnimId::from_name(clip) {
        Some(anim) => cx.pawn.anim.play_with(anim, PlayArgs::mode(2, -1.0)),
        None => cx.pawn.note("a unison clip is missing from the generated animation table"),
    }
    let uid = if kind <= 0x41 { 11 } else { 12 };
    cx.pawn.combat.friend.unisons += 1;
    cx.pawn.event(EventKind::TeamAttack { kind: kind as i32, phase: 0 });
    request(cx, FriendLinkRequest::Unison { uid, kind, target });
}

/// The friends a team command needs (FriendLinkPawn spawn `FUN_14127A780` needs them; Goofy's
/// 0x1411D1020: 193 needs Donald AND Goofy).
fn needed(kind: i32) -> &'static [u8] {
    match kind {
        193 => &[11, 12],
        195 => &[12],
        _ => &[11],
    }
}

/// The executor 0x14141B470 for the team commands 193 / 195 / 207 / 208: Sora's FR state begins
/// (`SetState`); it spawns the link pawn through the friends' side. The conditions (D3, read
/// [C]): the kinds share the executor's case group 0xC1.. -> the pawn's command interface
/// (+0x450) slot 0x348 0x14143B450, gated by its slot 0x340 0x141429180:
/// - Sora: no restriction bit (vt+0x1F48 / 0x1EB0 / 0xA48 / 0x2770 / 0x2788 / 0x1C30 / 0x2540
///   and the interface's 0x618: flags without a counterpart here), the acting-state test
///   vt+0x29C0, NO QUEUED STATE (FUN_1405AB6C0), vt+0x29F0(kind) = 0x141425510 (the gate
///   CanStyleChange uses: `form::state_gate`, pawn+0xE2C not 0x4B / 0xC0);
/// - the friend manager FUN_141259230 (when ctrl+0x888 != kind): FUN_141268A20 counts the party
///   members of the listed uids that are not KO'd (vt+0x1978): 193 Donald + Goofy (then Goofy's
///   own interface slot 0x340 must accept [D: read as "Goofy not KO'd"]), 195 Goofy, 207 / 208
///   Donald;
/// - then SetState (FUN_1405B9FE0): 193 -> 30, 195 -> 101 (+ ctrl FUN_141413910(1, 0)),
///   207 -> 135, 208 -> 136.
pub(crate) fn try_team_attack(state: &mut StateData, cx: &mut Ctx, kind: i32) -> bool {
    if in_fr_state(state) || matches!(state.id(), StateId::Damage | StateId::Die) {
        return false;
    }
    if !cx.pawn.queue.nothing_queued() || matches!(cx.pawn.combat.form.command_in_action, 0x4B | 0xC0) || !super::form::state_gate(state, cx) {
        return false;
    }
    if !needed(kind).iter().all(|uid| cx.pawn.combat.friend.friend(*uid).is_some_and(|f| !f.ko)) {
        return false;
    }
    let link = FrLink::new(kind);
    match kind {
        193 => cx.pawn.queue.set_state(crate::states::fr_shield_guardian::FrShieldGuardian::new(link)),
        195 => cx.pawn.queue.set_state(crate::states::fr_goofy_shoot::FrGoofyShoot::new(link)),
        207 => cx.pawn.queue.set_state(crate::states::fr_call_meteor::FrCallMeteor::new(link)),
        208 => cx.pawn.queue.set_state(crate::states::fr_flare_force::FrFlareForce::new(link)),
        _ => return false,
    }
    true
}

/// The controller's part during an FR state: the stick (world direction x magnitude) and the
/// presses the state passes to the link pawn. True while an FR state runs (the rest of the input
/// update is skipped: no other request is made).
pub(crate) fn fr_input(state: &StateData, cx: &mut Ctx, input: &Input, stick: Vec3) -> bool {
    if !in_fr_state(state) {
        return false;
    }
    let pad = input.pad;
    cx.pawn.combat.friend.input = (stick, pad.pressed.has(PadButton::ATTACK), input.jump_pressed, pad.pressed.has(PadButton::SITUATION));
    true
}

/// What Sora's four FR states share: the game's `TresPlayerState_FR*` shell (Begin spawns the link
/// pawn and sends `SpecificAction(0)`; sub 1 waits while the link pawn holds Sora; then the state's
/// own landing / `M_1501` and `DoFinish`), with the link pawn on the friends' side.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FrLink {
    pub kind: i32,
    pub sub: u8,
    /// The link view was seen (the link pawn exists: `m_pObj`).
    pub linked: bool,
    /// Steps without a view (the link pawn is gone / never came).
    pub missing: u8,
    /// The row the link pawn last made Sora play.
    pub motion: Option<&'static str>,
    /// `m_bEnableChange`-like: the end clip was started.
    pub ending: bool,
}

impl FrLink {
    pub fn new(kind: i32) -> FrLink {
        FrLink { kind, sub: 1, linked: false, missing: 0, motion: None, ending: false }
    }

    /// The link view of this team attack from the friends' rows.
    fn view(&self, cx: &Ctx) -> Option<FriendLinkView> {
        cx.pawn.combat.friend.friends.iter().flatten().map(|f| f.link).find(|l| l.kind == self.kind)
    }

    /// Begin (0x141368F90 and its siblings): the command bookkeeping (vt+0x330), the link pawn
    /// (`FUN_14127A780(friend manager, kind)`), `SpecificAction(0, 0)` on Sora.
    pub fn begin(&mut self, cx: &mut Ctx, state: StateId) {
        cx.pawn.combat.friend.running = self.kind;
        cx.pawn.cmc.stop_movement();
        cx.pawn.event(EventKind::TeamAttack { kind: self.kind, phase: 0 });
        let target = cx.pawn.combat.lockon.current;
        request(cx, FriendLinkRequest::Start { kind: self.kind, target });
        cx.pawn.event(EventKind::PlayerSpecificAction { state, action: 0, sub: 0 });
        // ctrl+0x8F4 = 0.1 and "command in action" are the executor's (situation::execute).
    }

    /// Sub 1: the link pawn holds and animates Sora; its specific actions; the input to it.
    /// Returns true when the link pawn has finished (or was never there): the state's ending.
    pub fn hold(&mut self, cx: &mut Ctx, state: StateId) -> bool {
        let (stick, attack, jump, _) = cx.pawn.combat.friend.input;
        let target = cx.pawn.combat.lockon.current;
        request(cx, FriendLinkRequest::Input { kind: self.kind, stick, attack, jump, target });
        let Some(view) = self.view(cx) else {
            self.missing = self.missing.saturating_add(1);
            // One step of latency through the host (the request above reaches the friends after
            // this step): a link pawn that has not answered after two steps does not exist.
            return self.missing > 2;
        };
        self.linked = true;
        self.missing = 0;
        if let Some(action) = view.sora_action {
            cx.pawn.event(EventKind::PlayerSpecificAction { state, action, sub: 0 });
        }
        if let Some((at, yaw)) = view.sora {
            let s = &mut *cx.pawn;
            s.cmc.set_custom();
            s.cmc.stop_movement();
            s.cmc.location = at;
            s.yaw = yaw;
        }
        if view.sora_motion != self.motion {
            self.motion = view.sora_motion;
            if let Some(anim) = view.sora_motion.and_then(AnimId::from_name) {
                cx.pawn.anim.play_with(anim, PlayArgs::mode(2, -1.0));
            }
        }
        view.finished
    }

    /// Detach from the link pawn (vt+0x23B0(0)): gravity again.
    pub fn detach(&mut self, cx: &mut Ctx) {
        cx.pawn.cmc.set_falling();
        cx.pawn.cmc.gravity_off = false;
    }

    /// End (0x14136BB10 and siblings): the command bookkeeping (vt+0x370), the link pawn's end.
    pub fn end(&mut self, cx: &mut Ctx) {
        let interrupted = !self.ending;
        cx.pawn.combat.friend.running = 0;
        if cx.pawn.cmc.is_custom() {
            self.detach(cx);
        }
        request(cx, FriendLinkRequest::End { kind: self.kind, interrupted });
        super::situation::delete(cx, 214);
        cx.pawn.event(EventKind::TeamAttack { kind: self.kind, phase: 255 });
    }

    /// `TresAnimNotifyState_BpEvent` of Sora's clips -> the state-event camera blueprints
    /// (`ReceiveAnimNotifyStartBpEvent` / `...EndBpEvent`).
    pub fn on_notifies(&mut self, cx: &mut Ctx, notifies: &Notifies) {
        for e in notifies.events.iter().filter(|e| e.class() == NotifyClass::TresAnimNotifyStateBpEvent) {
            let param = match e.def().data {
                NotifyData::Param { param } => param.unwrap_or(0),
                _ => 0,
            };
            let begin = match e.phase {
                NotifyPhase::Begin => true,
                NotifyPhase::End => false,
                _ => continue,
            };
            cx.pawn.event(EventKind::AnimBpEvent { seq: e.sequence_name(), param, begin });
        }
    }

    /// The end clip (`M_1501` of CallMeteor / FlareForce / ShieldGuardian, played with blend 0 /
    /// -1) and its wait (sub 4: `vt+0xDC8(dt x 15)`, then `DoFinish` at its end).
    pub fn play_end(&mut self, cx: &mut Ctx, clip: &str, blend: f32) {
        self.ending = true;
        match AnimId::from_name(clip) {
            Some(anim) => cx.pawn.anim.play_with(anim, PlayArgs::mode(2, blend)),
            None => cx.pawn.note("an FR end clip is missing from the generated animation table"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gauge_fills_from_pending_and_is_full_at_200() {
        let mut f = FriendState::default();
        f.gauge_add(150.0);
        assert_eq!((f.gauge.pending, f.gauge.timer), (150.0, 30.0));
        assert!(!f.gauge_full());
        f.gauge.points = 200.0;
        assert!(f.gauge_full());
        f.debug_fill_gauge();
        assert!(f.gauge_full());
    }

    #[test]
    fn unison_kinds_and_shots() {
        assert_eq!([22, 23, 24, 25, 26].map(unison_attack_kind), [0x3F, 0x40, 0x41, 0x42, 0x43]);
        assert_eq!(unison_shot(0x40), "UNISON_BLIZZARD");
        assert_eq!(unison_shot(0x42), "FUSION_SPIN");
        assert_eq!(needed(193), &[11, 12]);
    }
}
