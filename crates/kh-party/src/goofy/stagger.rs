//! (GF-BREAK) THE STAGGER BOOK, THE ENEMY'S POSTURE (documentation/GOOFY_OVERHAUL.md 5; PLAN.md R6): one gauge per target, on
//! the party, so the test level and Elden Ring run one set of rules. Each parry technique's own Stagger, the Shield Arts and basic
//! hits fill it (`seams::on_stagger`); it drains slowly once he stops pressuring, slower while the enemy's HP is low (the low-HP
//! rule). (THE USER 2026-11-02: "The enemy's stagger gauge should not begin to deplete so fast") it waits `stagger.drain_delay`
//! (5 s, was 2) and drains `stagger.drain_rate` (4 a second, was 7): a full normal gauge left alone lasts 30 s, a boss's 65 s.
//! Full: the enemy is BROKEN for 2.5 s, a boss for 1.5 s, and a boss's gauge then resets to half.
//!
//! WHAT THE HOST ANSWERS: the boss and size facts are the plain `Targets::size_class` ([`StaggerFacts::of`]); the break itself
//! is `Targets::stagger_ask` (hooks.rs), and a host may refuse it (its chain cap, its armour, an event): then nothing happened
//! to the enemy and the gauge falls to `stagger.refused_share`. The book holds no enemy data: everything is asked each step.

use super::tune::K;
use crate::donald::status::Size;
use kh_core::host::{TargetId, TargetInfo, Targets};

/// What the host says about a target now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StaggerFacts {
    pub boss: bool,
    pub large: bool,
    /// Its HP share, 0 ..= 1.
    pub hp_share: f32,
}

impl StaggerFacts {
    /// The plain facts of `t`: its size by the host's word (`Targets::size_class`, else its bounds), its HP.
    pub fn of(targets: &dyn Targets, t: &TargetInfo) -> StaggerFacts {
        let size = crate::donald::magic_fx::size_of(targets, t);
        StaggerFacts { boss: size == Size::Boss, large: size == Size::Large, hp_share: if t.hp_max > 0.0 { (t.hp / t.hp_max).clamp(0.0, 1.0) } else { 1.0 } }
    }

    /// Its gauge.
    pub fn gauge(&self) -> f32 {
        if self.boss {
            K.stagger_gauge_boss
        } else if self.large {
            K.stagger_gauge_large
        } else {
            K.stagger_gauge
        }
    }

    /// Its BROKEN window, seconds.
    pub fn window(&self) -> f32 {
        if self.boss {
            K.stagger_window_boss
        } else {
            K.stagger_window
        }
    }

    /// What is left of the gauge after a break, as a share.
    pub fn reset_share(&self) -> f32 {
        if self.boss {
            K.stagger_reset_share_boss
        } else {
            K.stagger_reset_share
        }
    }

    /// R7: a Breaker defeats it outright. Never a boss; the host still has its word (`StaggerAsk::MayDefeat`).
    pub fn defeats(&self) -> bool {
        !self.boss && self.hp_share < K.breaker_defeat_below
    }

    /// Launch Breaker may throw it (bosses and large bodies stay on the ground).
    pub fn launches(&self) -> bool {
        !self.boss && !self.large
    }
}

/// The gauge's drain per second at this HP share (the low-HP rule: the less HP, the slower).
pub fn drain_rate(hp_share: f32) -> f32 {
    let floor = K.stagger_drain_low_share;
    K.stagger_drain_rate * (floor + (1.0 - floor) * hp_share.clamp(0.0, 1.0))
}

/// One target's BROKEN.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Broken {
    left: f32,
    total: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Entry {
    target: TargetId,
    value: f32,
    /// Seconds since something filled it.
    idle: f32,
    facts: StaggerFacts,
    broken: Option<Broken>,
}

/// One target's gauge as a HUD draws it (GF-UI: `Party::stagger_view`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StaggerView {
    pub value: f32,
    pub max: f32,
    pub boss: bool,
    /// BROKEN now: seconds left of the window, and its length.
    pub broken: Option<(f32, f32)>,
}

impl StaggerView {
    /// The gauge's fill, 0 ..= 1.
    pub fn share(&self) -> f32 {
        if self.max > 0.0 {
            (self.value / self.max).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

/// THE BOOK (`PartyGoofy::book`). Pure bookkeeping: the seams and the host's answers are hooks.rs'.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct StaggerBook {
    list: Vec<Entry>,
}

impl StaggerBook {
    fn entry(&mut self, target: TargetId) -> Option<&mut Entry> {
        self.list.iter_mut().find(|e| e.target == target)
    }

    /// `amount` on `target`. True: the gauge is full and the target is not BROKEN yet: the host is to be asked.
    pub fn add(&mut self, target: TargetId, amount: f32, facts: StaggerFacts) -> bool {
        if self.entry(target).is_none() {
            self.list.push(Entry { target, value: 0.0, idle: 0.0, facts, broken: None });
        }
        let Some(e) = self.entry(target) else { return false };
        e.facts = facts;
        // (a BROKEN gauge stands full: nothing more goes in)
        if e.broken.is_some() {
            return false;
        }
        e.value = (e.value + amount.max(0.0)).min(facts.gauge());
        e.idle = 0.0;
        e.value >= facts.gauge()
    }

    /// The host said yes: BROKEN for the window.
    pub fn open(&mut self, target: TargetId) {
        if let Some(e) = self.entry(target) {
            let total = e.facts.window();
            e.broken = Some(Broken { left: total, total });
        }
    }

    /// The host said no: nothing happened to the target, the gauge falls back.
    pub fn refused(&mut self, target: TargetId) {
        if let Some(e) = self.entry(target) {
            e.value = e.facts.gauge() * K.stagger_refused_share;
            e.idle = 0.0;
        }
    }

    /// A Breaker runs: the window lasts at least `seconds` more.
    pub fn hold(&mut self, target: TargetId, seconds: f32) {
        if let Some(b) = self.entry(target).and_then(|e| e.broken.as_mut()) {
            b.left = b.left.max(seconds);
        }
    }

    /// BROKEN ends now (the slam): the gauge resets. True: it was BROKEN.
    pub fn close(&mut self, target: TargetId) -> bool {
        let Some(e) = self.entry(target).filter(|e| e.broken.is_some()) else { return false };
        Self::reset(e);
        true
    }

    fn reset(e: &mut Entry) {
        e.value = e.facts.gauge() * e.facts.reset_share();
        e.idle = 0.0;
        e.broken = None;
    }

    /// One step. `facts`: the host's word on a target now; None: it is gone or dead, and its gauge goes with it. The targets
    /// whose window ran out are returned (the host is told).
    pub fn step(&mut self, dt: f32, mut facts: impl FnMut(TargetId) -> Option<StaggerFacts>) -> Vec<TargetId> {
        let mut ended = Vec::new();
        self.list.retain_mut(|e| {
            let Some(now) = facts(e.target) else { return false };
            e.facts = now;
            e.value = e.value.min(now.gauge());
            if let Some(b) = e.broken.as_mut() {
                b.left -= dt;
                if b.left <= 0.0 {
                    Self::reset(e);
                    ended.push(e.target);
                }
                return true;
            }
            e.idle += dt;
            if e.idle > K.stagger_drain_delay {
                e.value = (e.value - drain_rate(now.hp_share) * dt).max(0.0);
            }
            e.value > 0.0
        });
        ended
    }

    /// The BROKEN targets, with the seconds left.
    pub fn broken(&self) -> impl Iterator<Item = (TargetId, f32, StaggerFacts)> + '_ {
        self.list.iter().filter_map(|e| e.broken.map(|b| (e.target, b.left, e.facts)))
    }

    /// The HUD's view of a target (None: no Stagger on it).
    pub fn view(&self, target: TargetId) -> Option<StaggerView> {
        self.list.iter().find(|e| e.target == target).map(|e| {
            let max = e.facts.gauge();
            StaggerView { value: if e.broken.is_some() { max } else { e.value }, max, boss: e.facts.boss, broken: e.broken.map(|b| (b.left, b.total)) }
        })
    }

    /// Every gauge, for the debug word.
    pub fn summary(&self) -> String {
        self.list.iter().map(|e| format!("{}: {:.0}/{:.0}{}", e.target.0, e.value, e.facts.gauge(), e.broken.map_or(String::new(), |b| format!(" BROKEN {:.2}/{:.2} s", b.left, b.total)))).collect::<Vec<_>>().join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;
    const T: TargetId = TargetId(7);
    const FOE: StaggerFacts = StaggerFacts { boss: false, large: false, hp_share: 1.0 };
    const BOSS: StaggerFacts = StaggerFacts { boss: true, large: false, hp_share: 1.0 };
    const BIG: StaggerFacts = StaggerFacts { boss: false, large: true, hp_share: 1.0 };

    fn run(b: &mut StaggerBook, seconds: f32, f: StaggerFacts) -> Vec<TargetId> {
        let mut out = Vec::new();
        for _ in 0..(seconds * 60.0).round() as u32 {
            out.extend(b.step(DT, |_| Some(f)));
        }
        out
    }

    fn value(b: &StaggerBook) -> f32 {
        b.view(T).map_or(0.0, |v| v.value)
    }

    #[test]
    fn deflects_fill_a_lot_basic_hits_a_little_and_it_breaks_when_full() {
        let mut b = StaggerBook::default();
        assert!(!b.add(T, K.stagger_basic, FOE));
        assert_eq!(value(&b), K.stagger_basic);
        let (basic, deflect) = (K.stagger_basic, K.stagger_deflect);
        assert!(deflect > basic * 4.0);
        assert!(!b.add(T, K.stagger_deflect, FOE));
        assert!(b.add(T, K.stagger_gauge, FOE), "full");
        assert_eq!(value(&b), K.stagger_gauge, "never past the gauge");
        b.open(T);
        assert_eq!(b.broken().count(), 1);
        assert!(!b.add(T, 50.0, FOE), "a BROKEN gauge takes nothing");
    }

    #[test]
    fn a_boss_has_the_larger_gauge_a_large_body_between() {
        assert!(BOSS.gauge() > BIG.gauge() && BIG.gauge() > FOE.gauge());
        let mut b = StaggerBook::default();
        assert!(!b.add(T, FOE.gauge(), BOSS), "a normal gauge's worth does not break a boss");
        assert!(b.add(T, BOSS.gauge(), BOSS));
    }

    #[test]
    fn it_waits_then_drains_and_drains_slower_at_low_hp() {
        let mut b = StaggerBook::default();
        b.add(T, 50.0, FOE);
        run(&mut b, K.stagger_drain_delay - 0.1, FOE);
        assert_eq!(value(&b), 50.0, "no drain while he presses");
        run(&mut b, 1.1, FOE);
        assert!((value(&b) - (50.0 - K.stagger_drain_rate)).abs() < 0.3, "{}", value(&b));
        // the low-HP rule: the less HP, the slower; full HP drains at the row's rate
        assert_eq!(drain_rate(1.0), K.stagger_drain_rate);
        assert!(drain_rate(0.2) < drain_rate(0.6) && drain_rate(0.6) < drain_rate(1.0));
        assert!((drain_rate(0.0) - K.stagger_drain_rate * K.stagger_drain_low_share).abs() < 1e-4);
        // two gauges side by side, one at 15 % HP
        let low = StaggerFacts { hp_share: 0.15, ..FOE };
        let (mut a, mut c) = (StaggerBook::default(), StaggerBook::default());
        a.add(T, 50.0, FOE);
        c.add(T, 50.0, low);
        run(&mut a, K.stagger_drain_delay + 4.0, FOE);
        run(&mut c, K.stagger_drain_delay + 4.0, low);
        assert!(value(&c) > value(&a) + 5.0, "{} {}", value(&c), value(&a));
        // a new fill restarts the wait
        let before = value(&a);
        a.add(T, 1.0, FOE);
        run(&mut a, 1.0, FOE);
        assert_eq!(value(&a), before + 1.0);
        // an emptied gauge leaves the book
        run(&mut a, 30.0, FOE);
        assert_eq!(a.view(T), None);
    }

    /// (THE USER 2026-11-02: "The enemy's stagger gauge should not begin to deplete so fast") 5 s before it drains (was 2), 4 a
    /// second (was 7): a full normal gauge left alone lasts 30 s (was 16.3), a boss's 65 s (was 36.3); low HP still slower.
    #[test]
    fn the_gauge_waits_five_seconds_and_a_full_one_lasts_thirty() {
        assert_eq!((K.stagger_drain_delay, K.stagger_drain_rate, K.stagger_drain_low_share), (5.0, 4.0, 0.25));
        let lasts = |f: StaggerFacts| f.gauge() / drain_rate(f.hp_share) + K.stagger_drain_delay;
        assert!((lasts(FOE) - 30.0).abs() < 1e-3 && (lasts(BIG) - 40.0).abs() < 1e-3 && (lasts(BOSS) - 65.0).abs() < 1e-3);
        let mut b = StaggerBook::default();
        b.add(T, FOE.gauge() - 1.0, FOE);
        run(&mut b, 4.9, FOE);
        assert_eq!(value(&b), FOE.gauge() - 1.0, "nothing drains for 5 s");
        run(&mut b, 20.0, FOE);
        assert!(value(&b) > 0.0, "still on the gauge 25 s later: {}", value(&b));
        run(&mut b, 6.0, FOE);
        assert_eq!(b.view(T), None, "gone by 31 s");
    }

    #[test]
    fn broken_lasts_its_window_and_a_boss_resets_to_half() {
        assert_eq!((FOE.window(), BOSS.window()), (2.5, 1.5));
        let mut b = StaggerBook::default();
        b.add(T, 500.0, FOE);
        b.open(T);
        let v = b.view(T).unwrap();
        assert_eq!((v.share(), v.broken), (1.0, Some((2.5, 2.5))));
        assert_eq!(run(&mut b, 2.4, FOE), vec![]);
        assert_eq!(run(&mut b, 0.2, FOE), vec![T], "over at 2.5 s");
        assert_eq!(b.broken().count(), 0);
        assert_eq!(value(&b), 0.0, "a normal enemy starts over");
        let mut b = StaggerBook::default();
        b.add(T, 500.0, BOSS);
        b.open(T);
        assert_eq!(run(&mut b, 1.4, BOSS), vec![]);
        assert_eq!(run(&mut b, 0.2, BOSS), vec![T], "over at 1.5 s");
        assert_eq!(value(&b), BOSS.gauge() * 0.5, "half");
    }

    #[test]
    fn a_refused_break_is_no_break() {
        let mut b = StaggerBook::default();
        assert!(b.add(T, 500.0, FOE));
        b.refused(T);
        assert_eq!((b.broken().count(), value(&b)), (0, FOE.gauge() * K.stagger_refused_share));
        assert!(!b.add(T, 1.0, FOE), "the next fill starts from there");
    }

    #[test]
    fn a_breaker_holds_the_window_and_the_slam_closes_it() {
        let mut b = StaggerBook::default();
        b.add(T, 500.0, BOSS);
        b.open(T);
        run(&mut b, 1.0, BOSS);
        b.hold(T, 3.0);
        assert_eq!(run(&mut b, 2.5, BOSS), vec![], "the running Breaker is not cut by the clock");
        assert!(b.close(T));
        assert_eq!((b.broken().count(), value(&b)), (0, BOSS.gauge() * 0.5));
        assert!(!b.close(T), "once");
    }

    #[test]
    fn a_dead_target_leaves_the_book() {
        let mut b = StaggerBook::default();
        b.add(T, 500.0, FOE);
        b.open(T);
        assert_eq!(b.step(DT, |_| None), vec![]);
        assert_eq!(b.view(T), None);
    }

    #[test]
    fn the_defeat_rule_sits_at_forty_percent_and_never_on_a_boss() {
        assert!(StaggerFacts { hp_share: 0.39, ..FOE }.defeats());
        assert!(!StaggerFacts { hp_share: 0.40, ..FOE }.defeats());
        assert!(!StaggerFacts { hp_share: 0.41, ..FOE }.defeats());
        assert!(StaggerFacts { hp_share: 0.1, ..BIG }.defeats(), "a large body is no boss");
        assert!(!StaggerFacts { hp_share: 0.01, ..BOSS }.defeats());
        assert!(FOE.launches() && !BIG.launches() && !BOSS.launches());
    }
}
