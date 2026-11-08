//! (GF-BREAK) GOOFY BREAKER, HIS FINISHING BLOW (documentation/GOOFY_OVERHAUL.md 6.3; PLAN.md R6, R7, R13): Y on a BROKEN enemy
//! in reach, ahead of the team rows. Wind-up, spin, slam: `M_1321`, `M_1322` with its Attack window stripped, `SGuardHeavyBash`,
//! the hit a clip time of the last (they carry no Attack window to use). 8.0x (`breaker.power`), with Resolve's In the Zone and
//! Deep Breaker on top. (THE USER 2026-11-02: "Goofy's big damage should solely come from his guard break attacks from 6.3")
//! it is his only big damage now: the counters are gone, so it went up from 5.0x.
//! He cannot be hit while it runs, and it holds the enemy's BROKEN until the slam (`StaggerAsk::Hold`).
//!
//! THE DEFEAT (R7): a non-boss under 40 % HP dies to it, by one lethal hit through the host's own damage path
//! (`HIT_FLAG_BREAKER`), when the host agrees (`StaggerAsk::MayDefeat`: no no-death state, no event).
//!
//! THE VARIANTS (his abilities, doc 6.3, 11.3): Deep (+30 %), Party (the friends near the enemy join in, each by his own Strength),
//! Launch (a Toss instead of a Blow, with the host's poise window opened for the juggle; bosses and large bodies are not thrown,
//! and the slam is then an ordinary one), Rally (the party heals 10 % of each one's own max HP).
//!
//! The book lives on the party, a pawn's step cannot reach it: the party's step copies the BROKEN targets here (`BreakerState::marks`)
//! and takes what the move did from `GoofyCore::events`.

use super::goofy::{self, g, gp, has, st, stm, BRACE, BREAKER, GUARD, GUARD_HIT, HOP, PARRY};
use super::goofy_combo::COMBO;
use super::goofy_shots::{self as shots, SoloRow};
use crate::command::{MemberCommand, TargetRef};
use crate::goofy::stagger::{StaggerFacts, StaggerView};
use crate::goofy::{seams, GoofyAbility as GA, GoofyEvent, K};
use crate::host::PartyHost;
use crate::ids::MemberId;
use crate::party::Party;
use crate::pawn::{forward, horizontal, yaw_of, NpcPawn, Request, StepCtx};
use crate::states::NpcStateId;
use crate::view::SoraView;
use glam::{Vec2, Vec3};
use kh_core::enums::ETresDamageKind as Kind;
use kh_core::host::{StaggerAsk, TargetFlags, TargetId, TargetInfo, HIT_FLAG_BREAKER};

/// One BROKEN target as the party's step left it for him.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrokenMark {
    pub target: TargetId,
    pub position: Vec3,
    pub radius: f32,
    pub left: f32,
}

/// The running Breaker.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BreakerRun {
    pub target: TargetId,
    pub stage: u8,
    /// The wind-up's step toward it, cm/s, world.
    pub step: Vec2,
    /// The state time of the slam's hit.
    pub slammed: Option<f32>,
}

/// `GoofyRun::breaker`.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct BreakerState {
    pub marks: Vec<BrokenMark>,
    pub run: Option<BreakerRun>,
}

const WINDUP: u8 = 0;
const SPIN: u8 = 1;
const SLAM: u8 = 2;

fn stage_clip(stage: u8) -> (&'static str, f32) {
    match stage {
        WINDUP => (K.breaker_clip_windup, K.breaker_rate_windup),
        SPIN => (K.breaker_clip_spin, K.breaker_rate_spin),
        _ => (K.breaker_clip_slam, K.breaker_rate_slam),
    }
}

/// Seconds from the press to the slam's hit, and what the enemy's BROKEN must last to see it.
fn to_slam() -> f32 {
    let time = |stage| {
        let (name, rate) = stage_clip(stage);
        shots::clip(name).length / rate.max(0.01)
    };
    time(WINDUP) + time(SPIN) + K.breaker_hit / K.breaker_rate_slam.max(0.01)
}

fn hold_time() -> f32 {
    to_slam() + K.breaker_hitstop + K.breaker_hold_margin
}

fn play(p: &mut NpcPawn, stage: u8) {
    let (name, rate) = stage_clip(stage);
    p.anim.play(shots::clip_copy(name, true, false, true), 0.05);
    p.anim.speed = rate;
}

/// From what he may start it: the free ground, a raised shield, a shove, and whatever a cancel opens. Not in the bash's end lag
/// (THE USER 2026-11-02: "He needs some end lag at the end of his combo so you can't spam it, like Sora.").
fn can_start(p: &NpcPawn) -> bool {
    if !p.on_ground() || p.state == NpcStateId::Die || p.hurt.dying {
        return false;
    }
    // (GF-POLISH) the shield is out: he can only guard (8)
    if super::goofy_arts::bare_handed(p) {
        return false;
    }
    let n = goofy::state_n(p);
    super::free(p)
        || (p.pad.is_none() && matches!(p.state, NpcStateId::Move | NpcStateId::Turn))
        || matches!(n, Some(GUARD | BRACE | PARRY | GUARD_HIT))
        || (n == Some(COMBO) && !super::goofy_combo::end_lag_holds(p))
        || (n == Some(HOP) && p.anim.time >= K.hop_cancel)
}

/// The BROKEN target Y would take: his lock when it is in reach, else the nearest.
fn pick(p: &NpcPawn) -> Option<BrokenMark> {
    let me = p.position();
    let edge = |m: &BrokenMark| (horizontal(m.position - me).length() - m.radius).max(0.0);
    let marks = || g(p).breaker.marks.iter().filter(|m| m.left > 0.0 && edge(m) <= K.breaker_reach);
    let lock = goofy::target_id(p);
    marks().find(|m| Some(m.target) == lock).or_else(|| marks().min_by(|a, b| edge(a).total_cmp(&edge(b)))).copied()
}

/// Y is the Breaker's this step (a BROKEN enemy in reach): checked in `decide`, ahead of the team rows (R13).
pub fn breaker_offered(p: &NpcPawn) -> bool {
    can_start(p) && pick(p).is_some()
}

/// Y on a BROKEN enemy: Goofy Breaker. True = he started it.
pub fn try_breaker(p: &mut NpcPawn, _cx: &StepCtx, host: &mut PartyHost) -> bool {
    if !can_start(p) {
        return false;
    }
    let Some(mark) = pick(p) else { return false };
    // the rule of every choice of his: what the lock-on accepts
    let Some(info) = host.targets.info(mark.target).filter(shots::lockable) else { return false };
    goofy::cancel_action(p);
    p.cmc.speed_limit_override = None;
    p.cmc.walk_mode = false;
    p.enter(NpcStateId::Pad(BREAKER));
    p.cmc.stop_movement();
    let to = horizontal(info.position - p.position());
    if to.length() > 1.0 {
        p.yaw = yaw_of(to);
    }
    let (first, rate) = stage_clip(WINDUP);
    let gap = (to.length() - info.radius - p.params.capsule_radius - K.breaker_gap).max(0.0);
    let f = forward(p.yaw) * (gap / (shots::clip(first).length / rate.max(0.01)).max(0.05));
    play(p, WINDUP);
    let hold = hold_time();
    host.targets.stagger_ask(mark.target, StaggerAsk::Hold { seconds: hold });
    let k = gp(p);
    k.hitstop = 0.0;
    k.hold_invincible = true;
    k.breaker.run = Some(BreakerRun { target: mark.target, stage: WINDUP, step: Vec2::new(f.x, f.y), slammed: None });
    stm(p).core.events.push(GoofyEvent::BreakerStarted { target: mark.target, hold });
    goofy::log(p, "breaker");
    true
}

/// The Breaker's end, whatever ended it.
fn end(p: &mut NpcPawn) {
    p.anim.speed = 1.0;
    let k = gp(p);
    k.hitstop = 0.0;
    k.hold_invincible = false;
    k.breaker.run = None;
}

fn alive(t: &TargetInfo) -> bool {
    t.flags.has(TargetFlags::ALIVE)
}

/// THE SLAM.
fn slam(p: &mut NpcPawn, host: &mut PartyHost, target: TargetId) {
    let Some(info) = host.targets.info(target).filter(alive) else { return };
    let facts = StaggerFacts::of(&*host.targets, &info);
    let deep = if has(p, GA::DeepBreaker) { K.breaker_deep_mult } else { 1.0 };
    let power = K.breaker_power * seams::breaker_mult(st(p)) * deep;
    let defeat = facts.defeats() && host.targets.stagger_ask(target, StaggerAsk::MayDefeat);
    // (a body that dies is not thrown; a boss or a large body simply takes the slam)
    let launched = has(p, GA::LaunchBreaker) && facts.launches() && !defeat;
    let kind = Kind::from_game_name(if launched { K.breaker_launch_kind } else { K.breaker_kind }).unwrap_or(Kind::Blow);
    let mut row = SoloRow { power, invalid_guard: true, ..SoloRow::plain("GOOFY_BREAKER", kind) };
    if defeat {
        row.flat = Some(info.hp.max(1.0) + 1.0);
        row.flags |= HIT_FLAG_BREAKER;
    }
    // before the hit: the kneel must end for the slam's own reaction to play on it
    host.targets.stagger_ask(target, StaggerAsk::Release { juggle: if launched { K.breaker_launch_window } else { 0.0 } });
    let mut shapes = Vec::new();
    host.targets.hurt_shapes(target, &mut shapes);
    let (part, at) = shapes.first().map_or((0, info.center), |s| (s.part, (s.a + s.b) * 0.5));
    let dealt = shots::deliver(p, &row, target, part, at, horizontal(info.position - p.position()), host);
    shots::effect(p, host, K.breaker_effect, at, 1.0);
    shots::sound(p, host, K.breaker_se, at);
    gp(p).hitstop = K.breaker_hitstop;
    let rally = has(p, GA::RallyBreaker);
    stm(p).core.events.push(GoofyEvent::BreakerLanded { target, defeat, launched, rally, dealt: dealt.unwrap_or(0.0) });
    goofy::log(p, if defeat { "breaker_defeat" } else { "breaker_slam" });
}

/// MainProc of `goofy::BREAKER`.
pub fn tick(p: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) {
    let Some(mut r) = g(p).breaker.run else {
        goofy::leave_to_ground(p);
        return;
    };
    // (something else ended the enemy before the slam: nothing left to slam)
    if r.slammed.is_none() && host.targets.info(r.target).filter(alive).is_none() {
        end(p);
        goofy::leave_to_ground(p);
        return;
    }
    if g(p).hitstop > 0.0 {
        gp(p).hitstop -= cx.dt;
        p.anim.speed = 0.0;
        p.cmc.direct_xy = Some(Vec2::ZERO);
        if g(p).hitstop > 0.0 {
            return;
        }
        gp(p).hitstop = 0.0;
    }
    p.anim.speed = stage_clip(r.stage).1;
    p.cmc.direct_xy = Some(if r.stage == WINDUP && p.on_ground() { r.step } else { Vec2::ZERO });
    if r.stage == SLAM {
        if r.slammed.is_none() && p.anim.time >= K.breaker_hit {
            r.slammed = Some(p.state_time);
            gp(p).breaker.run = Some(r);
            slam(p, host, r.target);
            return;
        }
        if p.anim.finished || r.slammed.is_some_and(|at| p.state_time - at >= K.breaker_recovery + K.breaker_hitstop) {
            end(p);
            goofy::leave_to_ground(p);
            return;
        }
    } else if p.anim.finished {
        r.stage += 1;
        play(p, r.stage);
        if r.stage == SPIN && has(p, GA::PartyBreaker) {
            stm(p).core.events.push(GoofyEvent::BreakerJoin { target: r.target });
        }
    }
    gp(p).breaker.run = Some(r);
}

// ---------------------------------------------------------------------------------------------- the party's side

/// PARTY BREAKER: the friends near the enemy join in, each hit by his own Strength. A free AI friend turns and swings; one busy
/// with a move of his own, or led by a pad, only lands the hit.
pub fn join(party: &mut Party, from: MemberId, target: TargetId, host: &mut PartyHost) {
    let Some(info) = host.targets.info(target).filter(alive) else { return };
    let kind = Kind::from_game_name(K.breaker_party_kind).unwrap_or(Kind::Small);
    let row = SoloRow { power: K.breaker_party_power, invalid_guard: true, ..SoloRow::plain("GOOFY_BREAKER_PARTY", kind) };
    let ids: Vec<MemberId> = party.members().filter(|id| *id != from).collect();
    for id in ids {
        let Some(m) = party.member_mut(id) else { continue };
        let to = horizontal(info.position - m.position());
        if m.status.hp < 1 || m.state == NpcStateId::Die || m.hurt.dying || to.length() > K.breaker_party_range {
            continue;
        }
        if m.pad.is_none() && matches!(m.state, NpcStateId::Idle | NpcStateId::Move) {
            if to.length() > 1.0 {
                m.yaw = yaw_of(to);
            }
            let _ = m.command(MemberCommand::PlayMotion { name: K.breaker_party_clip, looping: false });
        }
        let _ = shots::deliver(m, &row, target, 0, info.center, to, host);
    }
}

/// RALLY BREAKER: 10 % of each one's own max HP; Sora, who is no member, through the host's heal.
pub fn rally(party: &mut Party, from: MemberId, sora: &SoraView) {
    let share = K.breaker_rally_heal;
    let ids: Vec<MemberId> = party.members().collect();
    let friend_leads = ids.iter().any(|id| party.member(*id).is_some_and(|m| m.pad.is_some()));
    for id in ids {
        let Some(s) = party.status(id).filter(|s| s.hp > 0) else { continue };
        let amount = ((s.max_hp() as f32 * share).round() as i32).max(1);
        party.heal(id, amount, 1);
        if let Some(m) = party.member_mut(from) {
            m.requests.push(Request::HealEffect { member: id, kind: 1 });
        }
    }
    let amount = ((sora.hp_max.max(1) as f32 * share).round() as i32).max(1);
    let target = match sora.ai_sora {
        Some((id, _)) => Some(TargetRef::Member(id)),
        // (a heal on "the player" goes to a leading friend: he has had his)
        None if !friend_leads && !sora.dead => Some(TargetRef::Sora),
        None => None,
    };
    if let (Some(target), Some(m)) = (target, party.member_mut(from)) {
        m.requests.push(Request::Heal { target, amount, effect: 1 });
    }
}

impl Party {
    /// THE HUD'S VIEW OF A TARGET'S STAGGER (GF-UI, the `TargetView` route): the gauge under its HP gauge, BROKEN and the window
    /// left. None: nothing on it yet (draw no gauge).
    pub fn stagger_view(&self, target: TargetId) -> Option<StaggerView> {
        self.gof.book.view(target)
    }

    /// Y would be Goofy Breaker for member `id` now (the Y row, the prompt).
    pub fn goofy_breaker_offered(&self, id: MemberId) -> bool {
        self.member(id).is_some_and(|m| m.status.goofy.is_some() && breaker_offered(m))
    }
}

#[cfg(test)]
#[path = "goofy_breaker_tests.rs"]
mod tests;
