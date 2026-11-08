//! (GOOFY) THE PARTY'S STEP FOR EVERY GOOFY, pad or AI (party.rs, marked): the clocks, his level's abilities, GUARD's extras, Hop
//! and Slip against the host's telegraphs, the cues a hit left behind, the events to the seams. Ally Guard reads a friend's hit
//! here (R14: Goofy absorbs it; "never a hit moved").

use super::seams::{self, GoofyEvent, StaggerWhy};
use super::stagger::{StaggerBook, StaggerFacts};
use super::tune::K;
use super::{GoofyAbility, Telegraph};
use crate::host::PartyHost;
use crate::ids::{Chara, MemberId};
use crate::pad::goofy_breaker::{self as breaker, BrokenMark};
use crate::pad::goofy_shots::lockable;
use crate::party::Party;
use crate::pawn::{horizontal, NpcPawn};
use crate::view::SoraView;
use kh_core::host::{HitRecord, HitResult, StaggerAsk, TargetFlags, TargetId, TargetInfo};

/// The party-wide part (`Party::gof`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct PartyGoofy {
    /// What Goofy's rules did in the last step (for the seams' callers, the tests and the trace).
    pub events: Vec<(MemberId, GoofyEvent)>,
    /// (GF-BREAK) The Stagger book, one gauge per target.
    pub book: StaggerBook,
    /// (GF-BREAK) Stagger the debug word `x3g stagger` asked for.
    pub word: Vec<(TargetId, f32)>,
    /// (GF-AI) Each AI Goofy's memory.
    pub ai: Vec<super::ai::AiMemory>,
    /// (GF-AI) KH3's AI Goofy instead of the overhaul's (his KH3 actions as the trees ask): only for the tests of that
    /// machinery (kh-party tests/party.rs, p3_loops.rs); off in play.
    pub kh3_ai: bool,
    /// (GF-THREAT) Who drew each enemy's attention (threat.rs).
    pub threat: super::threat::ThreatBook,
    /// (GF-POLISH) The enemies' aims the host could read this step (`Party::set_enemy_aims`). Empty: it reads none, as the test level.
    pub aims: Vec<super::ai::EnemyAim>,
    /// (FINAL-FIX) AI Goofy's `goofy-ai:` lines of the last step (what he does about each BROKEN enemy, `ai::break_verdict`)
    /// and the debug word's: the host says them and empties the list (kh-partyhost runtime.rs).
    pub ai_notes: Vec<String>,
}

impl PartyGoofy {
    /// (FINAL-FIX) One `goofy-ai:` line for the host. A host that never empties the list (the tests, the test level) keeps the
    /// last `NOTES_KEPT` only.
    pub fn note(&mut self, line: String) {
        if self.ai_notes.len() >= NOTES_KEPT {
            self.ai_notes.remove(0);
        }
        self.ai_notes.push(line);
    }
}

/// (FINAL-FIX) [`PartyGoofy::note`]'s bound.
pub const NOTES_KEPT: usize = 64;

/// (GF-BREAK) The book's step: its clock against what the host says of each target now, the windows that ran out, and the BROKEN
/// targets copied to each Goofy (a pawn's step cannot reach the book).
fn stagger_step(party: &mut Party, dt: f32, host: &mut PartyHost, ids: &[MemberId]) {
    let alive = |t: &TargetInfo| t.flags.has(TargetFlags::ALIVE);
    let ended = party.gof.book.step(dt, |t| host.targets.info(t).filter(alive).map(|i| StaggerFacts::of(&*host.targets, &i)));
    for t in ended {
        host.targets.stagger_ask(t, StaggerAsk::Release { juggle: 0.0 });
    }
    let marks: Vec<BrokenMark> = party.gof.book.broken().filter_map(|(t, left, _)| host.targets.info(t).filter(lockable).map(|i| BrokenMark { target: t, position: i.position, radius: i.radius, left })).collect();
    for id in ids {
        if let Some(s) = party.member_mut(*id).and_then(|m| m.status.goofy.as_mut()) {
            s.run.breaker.marks.clone_from(&marks);
        }
    }
}

fn clocks(m: &mut NpcPawn, dt: f32) {
    let (iron, wind) = (m.status.gof_has(GoofyAbility::IronPosture), m.status.gof_has(GoofyAbility::SecondWind));
    let low = (m.status.hp as f32) < m.status.max_hp() as f32 * K.second_wind_below;
    let ko = m.status.hp < 1;
    let Some(s) = m.status.goofy.as_mut() else { return };
    s.core.tick(dt);
    let k = &mut s.run;
    k.invincible = (k.invincible - dt).max(0.0);
    k.nudge.0 = (k.nudge.0 - dt).max(0.0);
    if let Some(t) = k.taunt.as_mut() {
        t.left -= dt;
    }
    if k.taunt.is_some_and(|t| t.left <= 0.0) {
        k.taunt = None;
    }
    let fire = wind && low && !ko && s.core.second_wind_ready;
    s.core.second_wind_ready = if fire { false } else { s.core.second_wind_ready || !low };
    if let Some(g) = m.status.guard.as_mut() {
        if iron && !g.broken && g.regenerating() {
            g.value = (g.value + g.regen_rate * (K.guard_iron_mult - 1.0) * dt).min(g.max);
        }
        if fire {
            g.value = (g.value + g.max * K.second_wind_refill).min(g.max);
        }
    }
}

/// One step, after the members' own.
pub fn step(party: &mut Party, dt: f32, sora: &SoraView, host: &mut PartyHost) {
    party.gof.events.clear();
    let list: Vec<Telegraph> = party.telegraphs().to_vec();
    let ids: Vec<MemberId> = party.members().collect();
    let mut out = Vec::new();
    crate::pad::goofy_arts::note_friends(party, &ids); // (GF-ARTS) who Interpose can reach
    for id in ids.iter().copied() {
        let Some(m) = party.member_mut(id).filter(|m| m.chara == Chara::Goofy && m.status.goofy.is_some()) else { continue };
        m.status.gof_sync_level();
        clocks(m, dt);
        crate::pad::goofy_arts::clock(m, dt); // (GF-ARTS)
        crate::pad::goofy::evade_step(m, &list, dt, host);
        crate::pad::goofy_arts::step(m, dt, host); // (GF-ARTS) the shield's flight, in any state
        let Some(s) = m.status.goofy.as_mut() else { continue };
        let fx = std::mem::take(&mut s.run.fx);
        let sfx = std::mem::take(&mut s.run.sfx);
        let shield_fx = std::mem::take(&mut s.run.shield_fx);
        out.extend(std::mem::take(&mut s.core.events).into_iter().map(|e| (id, e)));
        for (name, at) in fx {
            crate::pad::goofy_shots::effect(m, host, name, at, 1.0);
        }
        for name in shield_fx {
            crate::pad::goofy_shots::effect_on_shield(m, host, name);
        }
        for (name, at) in sfx {
            crate::pad::goofy_shots::sound(m, host, name, at);
        }
    }
    for (id, e) in out {
        match e {
            GoofyEvent::Stagger { target, amount, why } => seams::on_stagger(party, id, target, amount, why, host),
            GoofyEvent::Threat { target, amount, why } => seams::on_threat(party, id, target, amount, why, host),
            GoofyEvent::BreakerStarted { target, hold } => party.gof.book.hold(target, hold),
            GoofyEvent::BreakerJoin { target } => breaker::join(party, id, target, host),
            GoofyEvent::BreakerLanded { target, rally, .. } => {
                party.gof.book.close(target);
                if rally {
                    breaker::rally(party, id, sora);
                }
            }
            // (THE USER 2026-11-02: "...bounce off of Goofy's shield...") a bounce the host did not take is told to nobody
            GoofyEvent::Bounce { target } if !seams::on_bounce(party, target, host) => continue,
            _ => {}
        }
        party.gof.events.push((id, e));
    }
    for (target, amount) in std::mem::take(&mut party.gof.word) {
        let Some((id, at, lock)) = ids.iter().find_map(|i| party.member(*i).filter(|m| m.chara == Chara::Goofy).map(|m| (*i, m.position(), m.status.goofy.as_ref().map_or(TargetId::NONE, |s| s.run.ai.lock)))) else { continue };
        let nearest = || crate::pad::goofy_shots::enemies(host).into_iter().min_by(|a, b| a.position.distance(at).total_cmp(&b.position.distance(at))).map_or(target, |t| t.id);
        let target = match target {
            TargetId::NONE => nearest(),
            WORD_OTHER => match other_than_his_lock(host, at, lock) {
                Some(t) => t,
                None => {
                    party.gof.note(format!("goofy-ai: x3g stagger other: no lockable enemy within {:.1} m of Goofy but his lock {} (nothing broken)", K.ai_breaker_chase / 100.0, lock.0));
                    continue;
                }
            },
            t => t,
        };
        if target != TargetId::NONE {
            party.gof.note(format!("goofy-ai: x3g stagger: {amount} on target {} (AI Goofy's lock {})", target.0, lock.0));
        }
        seams::on_stagger(party, id, target, amount, StaggerWhy::Word, host);
    }
    stagger_step(party, dt, host, &ids);
    threat_step(party, dt, host, &ids);
}

/// (FINAL-FIX) The debug word `x3g stagger other N` (pad/goofy.rs `goofy_word`): Stagger on the enemy nearest to Goofy that is
/// NOT his own lock, within his walk range (`ai.breaker_chase`): the in-game proof of "a broken enemy that is not his target".
pub const WORD_OTHER: TargetId = TargetId(u32::MAX - 1);

/// (FINAL-FIX) The nearest lockable enemy to `at` within `ai.breaker_chase` (edge) that is not `lock`.
fn other_than_his_lock(host: &PartyHost, at: glam::Vec3, lock: TargetId) -> Option<TargetId> {
    let edge = |t: &TargetInfo| (horizontal(t.position - at).length() - t.radius).max(0.0);
    crate::pad::goofy_shots::enemies(host).into_iter().filter(|t| t.id != lock && edge(t) <= K.ai_breaker_chase).min_by(|a, b| edge(a).total_cmp(&edge(b))).map(|t| t.id)
}

/// (GF-THREAT) Taunt's points on the enemies in its reach (what the lock-on accepts), the book's clock.
fn threat_step(party: &mut Party, dt: f32, host: &PartyHost, ids: &[MemberId]) {
    let taunts: Vec<_> = ids.iter().filter_map(|id| Some((*id, party.goofy_taunt(*id)?, seams::threat_mult(party.status(*id)?.goofy.as_ref()?)))).collect();
    if !taunts.is_empty() {
        let enemies = crate::pad::goofy_shots::enemies(host);
        for (id, taunt, mult) in taunts {
            for e in enemies.iter().filter(|e| super::threat::taunt_reaches(&taunt, e.position)) {
                party.gof.threat.add(e.id, id, K.threat_taunt * mult * dt);
            }
        }
    }
    party.gof.threat.step(dt);
}

/// Fills in who threw a hit the host could not name: the attack in the list of the same class, else the only one open.
pub fn name_attacker(list: &[Telegraph], hit: &mut HitRecord) {
    if hit.attack_info.attacker.is_some() {
        return;
    }
    let class = super::parry::class_of(hit);
    let open = || list.iter().filter(|t| t.active);
    hit.attack_info.attacker = open().find(|t| t.class == class).or_else(|| open().next()).map(|t| t.attacker);
}

/// R14, ALLY GUARD: Goofy within `ally_guard.range` of the friend `id`, off its cooldown, parries the blow for him. Some(result):
/// the hit is absorbed (the friend's side, as a Ward absorbs).
pub fn ally_guard(party: &mut Party, id: MemberId, hit: &HitRecord) -> Option<HitResult> {
    if !hit.takes_hp() || hit.damage < 1.0 {
        return None;
    }
    let at = party.member(id)?.position();
    let goofy = party.members().find(|g| {
        *g != id && party.member(*g).is_some_and(|m| m.status.gof_has(GoofyAbility::AllyGuard) && m.status.goofy.as_ref().is_some_and(|s| s.core.ally_cd <= 0.0) && horizontal(m.position() - at).length() <= K.ally_guard_range)
    })?;
    let m = party.member_mut(goofy)?;
    if !crate::pad::goofy::parry_for_friend(m, hit) {
        return None;
    }
    m.status.goofy.as_mut()?.core.ally_cd = K.ally_guard_cooldown;
    let hp = party.member(id)?.status.hp as f32;
    Some(HitResult { accepted: false, damage_dealt: 0.0, hp_after: hp, killed: false, guarded: true })
}
