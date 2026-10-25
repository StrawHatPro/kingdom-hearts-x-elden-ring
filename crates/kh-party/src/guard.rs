//! (GOOFY-GG) THE GUARD GAUGE: Goofy's own meter for his manual block (the user, 2026-10-16 23:40,
//! build/sora_tmp/party/ER_PHASE_NOTES.md: "Instead of using Focus, he should have a new bar above MP called GUARD that
//! is silver and matches the rest of the HUD design. Use that for his guard mechanics. Also make it possible to increase
//! via levelling up"). KH3 has no such gauge: it is THE PORT'S. Every number is a row of `sheets/port/party-guard.json`
//! (feel) or `sheets/port/party-guard-levels.json` (the max per level).
//!
//! Where it lives: `FriendStatus::guard` (`Some` for Goofy only; Donald has none, Sora keeps his Focus). The pad's block
//! (stream GK, pad/goofy.rs) spends / drains / breaks it; `FriendStatus::tick` runs [`Guard::tick`] every step; the
//! level-up path (`FriendStatus::set_total_exp`) runs [`Guard::set_level`]; the HUDs read [`Guard::view`].
//!
//! THE RULES:
//! - `value` in points, `0..=max`; `max` = [`Guard::max_for_level`] (plus `bonus`, 0 today: room for a GUARD-up reward).
//! - USE (anything that spends, drains, absorbs or holds the guard up) restarts the regen delay: no refill for
//!   `guard.refill_delay` after the last use; then it refills at `max / guard.refill_time` per second.
//! - EMPTY = BROKEN (reaching 0 by any route, or [`Guard::break_now`]). While broken nothing can be spent; refilling
//!   starts `guard.broken_delay` after the break at `max / guard.broken_refill_time` per second, and the break clears
//!   once the value is back to `guard.rearm_fraction` of the max.
//! - Frame-rate independent: everything is per second over the caller's fixed step `dt`.

include!(concat!(env!("KH_PARTY_GEN_DIR"), "/party_guard.rs"));
include!(concat!(env!("KH_PARTY_GEN_DIR"), "/party_guard_levels.rs"));

/// The gauge's numbers (sheet `party-guard`).
pub const GP: PartyGuardParams = PARTY_GUARD_PARAMS;

/// The GUARD meter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Guard {
    /// Current points.
    pub value: f32,
    /// Max points (`max_for_level(level) + bonus`).
    pub max: f32,
    /// Refill speed out of the break, points per second (`max / guard.refill_time`).
    pub regen_rate: f32,
    /// Seconds after the last use before the refill starts (`guard.refill_delay`).
    pub regen_delay: f32,
    /// Seconds since the last use.
    pub since_use: f32,
    /// Empty: no guarding until it rearms.
    pub broken: bool,
    /// Seconds since the break (counts while broken).
    pub broken_time: f32,
    /// The level the max was taken from.
    pub level: u32,
    /// Extra max points beyond the level table (0 today; a future reward row).
    pub bonus: f32,
}

/// What a blocked hit did to the gauge ([`Guard::absorb`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Absorbed {
    /// Points the gauge took.
    pub taken: f32,
    /// Points it could not take (the gauge ran out): the caller decides what the rest does (chip damage, a stagger).
    pub overflow: f32,
    /// This hit broke the guard.
    pub broke: bool,
}

/// What the HUDs draw.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GuardView {
    pub value: f32,
    pub max: f32,
    pub broken: bool,
    /// Refilling right now (the regen look).
    pub regenerating: bool,
    /// While broken: the share of the max it must reach to rearm.
    pub rearm_at: f32,
}

impl Default for Guard {
    fn default() -> Guard {
        Guard::for_level(1)
    }
}

impl Guard {
    /// GUARD max at `level` (1..=99, clamped): the sheet `party-guard-levels`.
    pub fn max_for_level(level: u32) -> f32 {
        let lv = level.clamp(1, 99);
        PARTY_GUARD_LEVELS.iter().filter(|r| r.level <= lv).max_by_key(|r| r.level).map(|r| r.guard_max).unwrap_or(100.0)
    }

    /// A full gauge for `level`.
    pub fn for_level(level: u32) -> Guard {
        let mut g = Guard { value: 0.0, max: 0.0, regen_rate: 0.0, regen_delay: GP.guard_refill_delay, since_use: 99.0, broken: false, broken_time: 0.0, level: level.clamp(1, 99), bonus: 0.0 };
        g.recompute_max();
        g.value = g.max;
        g
    }

    fn recompute_max(&mut self) {
        self.max = (Guard::max_for_level(self.level) + self.bonus).max(1.0);
        self.regen_rate = self.max / GP.guard_refill_time.max(0.01);
        self.value = self.value.min(self.max);
    }

    /// A new level: the max follows the table; with `guard.level_up_adds` a rise is added to the value (full stays full).
    pub fn set_level(&mut self, level: u32) {
        let old = self.max;
        self.level = level.clamp(1, 99);
        self.recompute_max();
        if GP.guard_level_up_adds && self.max > old {
            self.value = (self.value + (self.max - old)).min(self.max);
        }
    }

    /// Extra max points (rewards); keeps the value's points.
    pub fn set_bonus(&mut self, bonus: f32) {
        self.bonus = bonus.max(0.0);
        self.recompute_max();
    }

    /// The share 0..=1.
    pub fn fraction(&self) -> f32 {
        (self.value / self.max.max(1e-4)).clamp(0.0, 1.0)
    }

    /// Whether he can guard now (not broken, something left).
    pub fn can_guard(&self) -> bool {
        !self.broken && self.value > 0.0
    }

    /// Marks a use without spending (the shield held up): restarts the regen delay.
    pub fn hold(&mut self) {
        self.since_use = 0.0;
    }

    /// Spends `amount` all at once (a move's cost). False, nothing spent, when broken or short of it. Reaching 0 breaks.
    pub fn spend(&mut self, amount: f32) -> bool {
        if self.broken || self.value + 1e-4 < amount {
            return false;
        }
        self.since_use = 0.0;
        self.value = (self.value - amount.max(0.0)).max(0.0);
        if self.value <= 1e-4 {
            self.break_now();
        }
        true
    }

    /// A blocked hit worth `amount` points: takes what it can; an emptied gauge breaks.
    pub fn absorb(&mut self, amount: f32) -> Absorbed {
        let amount = amount.max(0.0);
        if self.broken {
            return Absorbed { taken: 0.0, overflow: amount, broke: false };
        }
        self.since_use = 0.0;
        let taken = amount.min(self.value);
        self.value -= taken;
        let broke = self.value <= 1e-4;
        if broke {
            self.break_now();
        }
        Absorbed { taken, overflow: amount - taken, broke }
    }

    /// A drain of `per_sec` points per second over `dt` (e.g. holding a move that burns GUARD). False when it ran out
    /// (it is broken now) or was already broken.
    pub fn drain_per_sec(&mut self, per_sec: f32, dt: f32) -> bool {
        if self.broken {
            return false;
        }
        self.since_use = 0.0;
        self.value = (self.value - per_sec.max(0.0) * dt.max(0.0)).max(0.0);
        if self.value <= 1e-4 {
            self.break_now();
            return false;
        }
        true
    }

    /// GUARD BREAK now (a guard-breaking hit, or an emptied gauge).
    pub fn break_now(&mut self) {
        self.value = 0.0;
        self.broken = true;
        self.broken_time = 0.0;
        self.since_use = 0.0;
    }

    /// Full and unbroken (a save point / a site of grace, a load).
    pub fn refill(&mut self) {
        self.value = self.max;
        self.broken = false;
        self.broken_time = 0.0;
        self.since_use = 99.0;
    }

    /// Whether it is refilling at this moment.
    pub fn regenerating(&self) -> bool {
        self.value < self.max && if self.broken { self.broken_time >= GP.guard_broken_delay } else { self.since_use >= self.regen_delay }
    }

    /// One step: the delays, the refill, the rearm.
    pub fn tick(&mut self, dt: f32) {
        let dt = dt.max(0.0);
        self.since_use += dt;
        if self.broken {
            self.broken_time += dt;
            if self.broken_time >= GP.guard_broken_delay {
                self.value = (self.value + self.max / GP.guard_broken_refill_time.max(0.01) * dt).min(self.max);
            }
            if self.value >= GP.guard_rearm_fraction * self.max - 1e-4 {
                self.broken = false;
            }
        } else if self.since_use >= self.regen_delay {
            self.value = (self.value + self.regen_rate * dt).min(self.max);
        }
    }

    pub fn view(&self) -> GuardView {
        GuardView { value: self.value, max: self.max, broken: self.broken, regenerating: self.regenerating(), rearm_at: GP.guard_rearm_fraction }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn run(g: &mut Guard, secs: f32) {
        for _ in 0..(secs / DT).round() as i32 {
            g.tick(DT);
        }
    }

    #[test]
    fn max_grows_with_level() {
        assert_eq!(Guard::max_for_level(1), 100.0);
        assert_eq!(Guard::max_for_level(4), 100.0);
        assert_eq!(Guard::max_for_level(5), 105.0);
        assert_eq!(Guard::max_for_level(99), 195.0);
        assert_eq!(Guard::max_for_level(500), 195.0);
        assert!((1..99).all(|l| Guard::max_for_level(l + 1) >= Guard::max_for_level(l)));
        let mut g = Guard::for_level(4);
        assert_eq!((g.value, g.max), (100.0, 100.0));
        g.set_level(5);
        assert_eq!((g.value, g.max), (105.0, 105.0), "a full bar stays full");
        g.value = 50.0;
        g.set_level(10);
        assert_eq!(g.value, 55.0, "the rise is added");
        assert!((g.regen_rate - 110.0 / GP.guard_refill_time).abs() < 1e-4);
    }

    #[test]
    fn spend_delay_and_regen() {
        let mut g = Guard::for_level(1);
        assert!(g.spend(40.0));
        assert_eq!(g.value, 60.0);
        assert!(!g.spend(61.0), "short of it: nothing spent");
        assert_eq!(g.value, 60.0);
        run(&mut g, GP.guard_refill_delay - 0.1);
        assert_eq!(g.value, 60.0, "no refill inside the delay");
        assert!(!g.regenerating());
        run(&mut g, 0.2);
        assert!(g.regenerating());
        run(&mut g, GP.guard_refill_time);
        assert_eq!(g.value, 100.0);
        // holding restarts the delay
        g.spend(10.0);
        run(&mut g, 1.5);
        g.hold();
        run(&mut g, 1.5);
        assert_eq!(g.value, 90.0);
    }

    #[test]
    fn drain_and_break_and_rearm() {
        let mut g = Guard::for_level(1);
        let mut t = 0.0;
        while g.drain_per_sec(50.0, DT) {
            t += DT;
        }
        assert!((t - 2.0).abs() < 0.05, "100 pt at 50/s lasts 2 s ({t})");
        assert!(g.broken && g.value == 0.0 && !g.can_guard());
        assert!(!g.spend(1.0));
        run(&mut g, GP.guard_broken_delay - 0.05);
        assert_eq!(g.value, 0.0);
        run(&mut g, 0.1 + GP.guard_rearm_fraction * GP.guard_broken_refill_time);
        assert!(!g.broken, "rearmed at {} of max ({})", GP.guard_rearm_fraction, g.value);
        assert!(g.can_guard());
    }

    #[test]
    fn absorb_overflow() {
        let mut g = Guard::for_level(1);
        let a = g.absorb(30.0);
        assert_eq!(a, Absorbed { taken: 30.0, overflow: 0.0, broke: false });
        let a = g.absorb(90.0);
        assert_eq!(a, Absorbed { taken: 70.0, overflow: 20.0, broke: true });
        assert_eq!(g.absorb(5.0).taken, 0.0);
        g.refill();
        assert_eq!((g.value, g.broken), (100.0, false));
        assert_eq!(g.view().max, 100.0);
    }

    #[test]
    fn goofy_status_carries_it_and_levels_it() {
        use crate::{Chara, FriendStatus};
        assert!(FriendStatus::new(Chara::Donald, 0).guard.is_none(), "Donald has no GUARD");
        let mut g = FriendStatus::new(Chara::Goofy, 0);
        assert_eq!(g.guard_view().map(|v| (v.value, v.max)), Some((100.0, 100.0)));
        // a level-up through the shared EXP: Goofy's row 20 needs 28,841 -> GUARD 120
        assert_eq!(g.set_total_exp(28_841), Some(20));
        assert_eq!(g.guard.unwrap().max, 120.0);
        // a route that sets the level directly (join_level, a load): tick syncs it
        g.level = 50;
        g.tick(DT);
        assert_eq!(g.guard.unwrap().max, 150.0);
        g.guard.as_mut().unwrap().break_now();
        g.refill_guard();
        assert_eq!(g.guard_view().map(|v| (v.value, v.broken)), Some((150.0, false)));
        assert_eq!(FriendStatus::test_level99(Chara::Goofy).guard.unwrap().max, 195.0);
    }

    #[test]
    fn frame_rate_independent() {
        let mut a = Guard::for_level(1);
        let mut b = a;
        a.spend(70.0);
        b.spend(70.0);
        for _ in 0..(3 * 30) {
            a.tick(1.0 / 30.0);
        }
        for _ in 0..(3 * 144) {
            b.tick(1.0 / 144.0);
        }
        assert!((a.value - b.value).abs() < 1.0, "{} vs {}", a.value, b.value);
    }
}
