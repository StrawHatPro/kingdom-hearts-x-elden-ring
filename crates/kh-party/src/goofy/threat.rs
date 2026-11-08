//! (GOOFY, GF-THREAT) THE THREAT BOOK (documentation/GOOFY_OVERHAUL.md 9; build/sora_tmp/party/GOOFY/PLAN.md R15): how much each
//! party member has drawn each enemy's attention, in the party so the test level has it too.
//!
//! - WHAT FEEDS IT: Goofy's basic hits, guards, deflects and Taunt (`seams::on_threat`, x [`seams::threat_mult`]: x1.5, x3 at
//!   Resolve 2+, Loud Mouth x1.25 on top); a friend's or Sora's hit and guard count their plain points, so his counts more.
//! - WHAT READS IT: whom an enemy chooses, never where a hit lands ("never a hit moved", R14). The doc's "the member with the most
//!   threat" gave way to R15: the director keeps its weighted roll and its holds, and a member's share of an enemy's threat
//!   adds [`weight`] to his chance. The test level's route is [`pick`] (kh-partyhost `gof_victim`), Elden Ring's the director
//!   (kh-er party/aggro.rs), which reads the rows through [`ThreatBook::rows`].
//! - TAUNT is a strong refreshed bias, not a force (no forcing route exists): while it runs, an enemy in its reach chooses him
//!   ([`taunt_reaches`]) and his points on it rise each step.
//! - THE LOCK-ON RULE: only what the game's lock-on accepts earns a row of Goofy's or a friend's: every route asks
//!   `pad::goofy_shots::lockable`, so a hidden body, a proxy or a duplicate never becomes anyone's threat. (Sora's rows come from
//!   his own events, which carry no target facts; a row only counts where a host's director has that enemy, and it has only
//!   enemies the lock-on lists.)
//! - Frame-rate independent: the book halves every `threat.half_life` seconds of `dt`.

use super::tune::K;
use crate::ids::MemberId;
use crate::host::PartyHost;
use crate::party::Party;
use crate::pawn::NpcPawn;
use crate::view::{PartyEventKind, PartyOutput};
use crate::NpcStateId;
use kh_core::host::TargetId;
use kh_core::Vec3;

/// A member's threat on one enemy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThreatRow {
    pub target: TargetId,
    /// The member ([`crate::donald::cast::SORA_MEMBER`] = Sora, who is no party member).
    pub member: MemberId,
    pub value: f32,
}

/// See the module's text. One book for the party (`Party::gof`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ThreatBook {
    rows: Vec<ThreatRow>,
    /// Guards and deflects not yet seen by a host's director (who took the blow, whose blow): zero-damage hits on the enemy's
    /// side of the ledger. Kept apart from the rows: a director also wants them as "who touched it last".
    stances: Vec<(MemberId, TargetId)>,
}

impl ThreatBook {
    /// Adds `amount` for `member` on `target` (a host's points, already scaled by whoever earned them).
    pub fn add(&mut self, target: TargetId, member: MemberId, amount: f32) {
        if amount <= 0.0 || target == TargetId::NONE {
            return;
        }
        match self.rows.iter_mut().find(|r| r.target == target && r.member == member) {
            Some(r) => r.value += amount,
            None => self.rows.push(ThreatRow { target, member, value: amount }),
        }
    }

    /// A guard or a deflect of `member` against `target`: for the next [`ThreatBook::take_stances`].
    pub fn note_stance(&mut self, member: MemberId, target: TargetId) {
        if self.stances.len() < 64 {
            self.stances.push((member, target));
        }
    }

    /// The guards and deflects since the last call (a host's director).
    pub fn take_stances(&mut self) -> Vec<(MemberId, TargetId)> {
        std::mem::take(&mut self.stances)
    }

    /// One step: everything fades, rows that faded away go.
    pub fn step(&mut self, dt: f32) {
        let keep = 0.5f32.powf(dt.max(0.0) / K.threat_half_life.max(0.1));
        for r in &mut self.rows {
            r.value *= keep;
        }
        self.rows.retain(|r| r.value >= K.threat_floor);
    }

    /// `member`'s points on `target`.
    pub fn value(&self, target: TargetId, member: MemberId) -> f32 {
        self.rows.iter().find(|r| r.target == target && r.member == member).map_or(0.0, |r| r.value)
    }

    /// `member`'s share of everything on `target` ([`share_of`]).
    pub fn share(&self, target: TargetId, member: MemberId) -> f32 {
        share_of(self.rows.iter().filter(|r| r.target == target).map(|r| (r.member == member, r.value)))
    }

    /// Who holds the most on `target`.
    pub fn top(&self, target: TargetId) -> Option<MemberId> {
        self.rows.iter().filter(|r| r.target == target).max_by(|a, b| a.value.total_cmp(&b.value)).map(|r| r.member)
    }

    /// Every row (a host's director reads them).
    pub fn rows(&self) -> &[ThreatRow] {
        &self.rows
    }

    /// The enemies with a row.
    pub fn targets(&self) -> impl Iterator<Item = TargetId> + '_ {
        let mut seen: Vec<TargetId> = Vec::new();
        self.rows.iter().filter_map(move |r| {
            if seen.contains(&r.target) {
                None
            } else {
                seen.push(r.target);
                Some(r.target)
            }
        })
    }

    /// The book as one line (the debug word `x3g threat`).
    pub fn summary(&self) -> String {
        if self.rows.is_empty() {
            return "the threat book is empty".to_string();
        }
        self.rows.iter().map(|r| format!("t{} m{}: {:.1}", r.target.0, r.member.0, r.value)).collect::<Vec<_>>().join(", ")
    }
}

/// `mine` of `(mine, value)` pairs as a share of them all plus `threat.prior`, 0..1: a point or two left on an enemy is not all of
/// its attention, a real lead comes near 1.
pub fn share_of(rows: impl Iterator<Item = (bool, f32)>) -> f32 {
    let (mine, all) = rows.fold((0.0, 0.0), |(m, a), (is_mine, v)| (if is_mine { m + v } else { m }, a + v));
    mine / (all + K.threat_prior.max(0.001))
}

/// What a member's share of an enemy's threat adds to his score in its weighted roll.
pub fn weight(share: f32) -> f32 {
    K.threat_weight * share.clamp(0.0, 1.0)
}

/// THE ROLL among `weights` (`roll` uniform in 0..1): the index chosen; the last one when the weights sum to nothing at all.
pub fn pick(weights: &[f32], roll: f32) -> Option<usize> {
    let sum: f32 = weights.iter().map(|w| w.max(0.0)).sum();
    let mut r = roll * sum;
    for (k, w) in weights.iter().enumerate() {
        let w = w.max(0.0);
        if r < w {
            return Some(k);
        }
        r -= w;
    }
    weights.len().checked_sub(1)
}

/// A taunt holds an enemy standing at `at`: within its radius on the ground, while it lasts.
pub fn taunt_reaches(t: &crate::pad::goofy::Taunt, at: Vec3) -> bool {
    t.left > 0.0 && (at - t.centre).truncate().length() <= t.radius
}

/// The taunting member whose taunt holds an enemy at `at`, if any (the first in the party's order).
pub fn taunter_of(party: &Party, at: Vec3) -> Option<MemberId> {
    party.members().find(|id| party.goofy_taunt(*id).is_some_and(|t| taunt_reaches(&t, at)))
}

/// His shield is up (guarding, bracing or parrying): Donald's Taunt Pass reads this (donald/ai.rs).
pub fn stance_up(m: &NpcPawn) -> bool {
    use crate::pad::goofy::{BRACE, GUARD, GUARD_HIT, PARRY};
    m.status.goofy.is_some() && matches!(m.state, NpcStateId::Pad(GUARD | GUARD_HIT | BRACE | PARRY))
}

/// R15's upper rung: his Resolve is at `threat.resolve_pips` or more.
pub fn strong(s: &super::GoofyState) -> bool {
    s.core.resolve.pips() as f32 >= K.threat_resolve_pips
}

/// A friend's hits (Goofy's own come through `seams::on_threat` with his multiplier): their plain points, on what the lock-on
/// accepts. After the party's step.
pub fn friends_hits(party: &mut Party, out: &PartyOutput, host: &PartyHost) {
    for e in &out.events {
        let PartyEventKind::Hit { target, .. } = e.kind else { continue };
        if party.member(e.member).is_some_and(|m| m.status.goofy.is_none()) && host.targets.info(target).as_ref().is_some_and(crate::pad::goofy_shots::lockable) {
            party.gof.threat.add(target, e.member, K.threat_basic);
        }
    }
}

/// Sora's side (he is no member; the hosts pass his events): a hit of his.
pub fn sora_hit(party: &mut Party, target: TargetId) {
    party.gof.threat.add(target, crate::donald::cast::SORA_MEMBER, K.threat_basic);
}

/// Sora's side: he guarded a blow of `from`.
pub fn sora_guard(party: &mut Party, from: TargetId) {
    party.gof.threat.add(from, crate::donald::cast::SORA_MEMBER, K.threat_guard);
    party.gof.threat.note_stance(crate::donald::cast::SORA_MEMBER, from);
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: TargetId = TargetId(7);
    const A: MemberId = MemberId(1);
    const B: MemberId = MemberId(2);

    #[test]
    fn the_book_fills_per_enemy_and_member() {
        let mut b = ThreatBook::default();
        b.add(T, A, 3.0);
        b.add(T, A, 2.0);
        b.add(T, B, 5.0);
        b.add(TargetId(8), A, 1.0);
        assert_eq!(b.value(T, A), 5.0);
        assert_eq!(b.value(TargetId(8), A), 1.0);
        assert_eq!(b.value(TargetId(9), A), 0.0);
        assert_eq!(b.share(T, A), 5.0 / (10.0 + K.threat_prior));
        assert_eq!(b.share(TargetId(9), A), 0.0);
        assert!(share_of([(true, 0.01)].into_iter()) < 0.02, "a stale crumb is not all its attention");
        assert!(share_of([(true, 40.0)].into_iter()) > 0.95, "a real lead nears 1");
        assert_eq!(b.rows().len(), 3);
        assert_eq!(b.targets().collect::<Vec<_>>(), vec![T, TargetId(8)]);
        b.add(T, A, 1.0);
        assert_eq!(b.top(T), Some(A));
        b.add(TargetId::NONE, A, 1.0);
        b.add(T, B, 0.0);
        assert_eq!(b.rows().len(), 3, "no target, no amount: no row");
    }

    #[test]
    fn the_book_halves_on_its_clock_whatever_the_frame_rate_and_forgets() {
        let run = |fps: f32| {
            let mut b = ThreatBook::default();
            b.add(T, A, 8.0);
            for _ in 0..(K.threat_half_life * fps).round() as u32 {
                b.step(1.0 / fps);
            }
            b.value(T, A)
        };
        for fps in [30.0, 60.0, 144.0] {
            assert!((run(fps) - 4.0).abs() < 0.02, "{fps} fps: {}", run(fps));
        }
        let mut b = ThreatBook::default();
        b.add(T, A, 8.0);
        for _ in 0..600 {
            b.step(1.0);
        }
        assert!(b.rows().is_empty(), "forgotten");
    }

    #[test]
    fn guards_and_deflects_wait_for_the_director_once() {
        let mut b = ThreatBook::default();
        b.note_stance(A, T);
        assert_eq!(b.take_stances(), vec![(A, T)]);
        assert!(b.take_stances().is_empty());
        for _ in 0..500 {
            b.note_stance(A, T);
        }
        assert!(b.take_stances().len() <= 64, "a host that never reads them holds a bounded list");
    }

    /// The roll's odds follow the weights: a member with all the threat is chosen far more often.
    #[test]
    fn the_weights_change_the_rolls_odds() {
        let odds = |w: &[f32]| (0..10_000u64).filter(|k| pick(w, crate::donald::aggro::roll(*k, 3, 5)) == Some(0)).count() as f32 / 10_000.0;
        let even = odds(&[1.0, 1.0]);
        assert!((even - 0.5).abs() < 0.03, "{even}");
        let drawn = odds(&[1.0 + weight(1.0), 1.0 + weight(0.0)]);
        let want = (1.0 + K.threat_weight) / (2.0 + K.threat_weight);
        assert!((drawn - want).abs() < 0.03, "{drawn} against {want}");
        assert_eq!(weight(5.0), K.threat_weight, "a share is at most 1");
    }

    #[test]
    fn the_roll_is_safe_at_its_edges() {
        assert_eq!(pick(&[], 0.5), None);
        assert_eq!(pick(&[0.0, 0.0], 0.5), Some(1));
        assert_eq!(pick(&[0.0, 2.0], 0.0), Some(1));
        assert_eq!(pick(&[2.0, 0.0], 0.99), Some(0));
    }

    #[test]
    fn a_taunt_reaches_within_its_radius_on_the_ground_while_it_lasts() {
        let t = crate::pad::goofy::Taunt { centre: Vec3::ZERO, radius: 1000.0, left: 5.0 };
        assert!(taunt_reaches(&t, Vec3::new(999.0, 0.0, 0.0)));
        assert!(taunt_reaches(&t, Vec3::new(0.0, 999.0, 800.0)), "height does not count");
        assert!(!taunt_reaches(&t, Vec3::new(1001.0, 0.0, 0.0)), "out of range");
        assert!(!taunt_reaches(&crate::pad::goofy::Taunt { left: 0.0, ..t }, Vec3::X), "it ran out");
    }
}
