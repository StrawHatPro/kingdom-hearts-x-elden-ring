//! (IMPL_ER21) BOSSES FIGHT BACK: what keeps an armoured body (KH3's STRONG hurt shape: Heavy / Giant of the class
//! table, i.e. large bodies and bosses) from being staggered again and again. Pure bookkeeping, no game access; the
//! Elden Ring side is puppet.rs (one `Guard` per armoured body) and targets.rs (the poise damage of a hit).
//!
//! WHAT WAS WRONG (the user's session, Margit c2130, stance 80): every basic hit carried 10 poise damage and every
//! finisher 60, so one ground combo (3 basic hits + the finisher = 90) broke his stance; the game's own kneel
//! (animation 8700, 5 s) protects the stance only while it plays, and the next combo broke it again: 15 stance breaks
//! in 178 s, 2.9 - 6 s apart at the worst. KH3's revenge value crossed its limit of 7 again and again, but a body
//! that is never moved had nothing to be released from: the revenge did nothing.
//!
//! THE RULES (every number a row of sheets/port/sora-er-host.json, `react.poise.*`, `react.revenge.*`,
//! `react.endurance.*`, `react.chain.*`; the key `react.guard = 0` switches all of them off):
//! 1. POISE DAMAGE (Elden Ring's side; the game's own stance logic does the rest, nothing is written to its poise):
//!    a hit with KH3 reaction power 0 (basic combo hits: they never move a STRONG body) deals `poise.basic` (0) of its
//!    row's poise damage; no hit deals more than `poise.max_share` of the body's maximum; for `poise.lockout`
//!    seconds after a stance break of the game's no poise damage is dealt at all.
//! 2. REVENGE (KH3's side): the armoured classes have a revenge limit (`revenge.giant`, `revenge.heavy`; KH3's rows:
//!    6.5 - 8). When the value crosses it the body is UNFLINCHABLE for `revenge.armour` seconds: no reaction starts,
//!    its endurance holds, and its hits carry neither a damage level nor poise damage, so that Elden Ring cannot
//!    interrupt the attack it answers with. Damage itself is dealt as always.
//! 3. THE ENDURANCE (KH3's side): emptied by Elden Ring's stance break or by reaction power, it is open for KH3's own
//!    10 s and refills; for `endurance.lockout` seconds after a refill nothing takes anything off it.
//! 4. THE CHAIN CAP: after `chain.max` reactions within `chain.window` seconds further reactions are refused for
//!    `chain.cooldown` seconds (hits within `CHAIN_GAP` of a counted reaction are the same stagger).

/// Hits this close (seconds) after a counted reaction belong to it (a multi-hit volley is one stagger).
pub const CHAIN_GAP: f32 = 0.5;

/// The rows `react.poise.*`, `react.revenge.*`, `react.endurance.*`, `react.chain.*`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GuardRules {
    /// `react.poise.basic`: the share of its row's poise damage a hit WITHOUT reaction power deals to an armoured body.
    pub poise_basic: f32,
    /// `react.poise.max_share`: no hit deals more poise damage than this share of the body's poise maximum.
    pub poise_max_share: f32,
    /// `react.poise.lockout`: seconds after a stance break without any poise damage of ours.
    pub poise_lockout: f32,
    /// `react.revenge.giant` / `.heavy`: KH3's `m_RevengeLimit` of the two armoured classes (0 = no revenge).
    pub revenge_giant: f32,
    pub revenge_heavy: f32,
    /// `react.revenge.armour`: seconds a body is unflinchable after its revenge (also KH3's revenge cool-down:
    /// no revenge points are earned meanwhile).
    pub revenge_armour: f32,
    /// `react.endurance.lockout`: seconds after a refill in which the endurance cannot be taken from.
    pub endurance_lockout: f32,
    /// `react.chain.max` reactions within `react.chain.window` seconds, then none for `react.chain.cooldown`.
    pub chain_max: u32,
    pub chain_window: f32,
    pub chain_cooldown: f32,
}

impl GuardRules {
    /// The sheet's rows.
    pub const DEFAULT: GuardRules = GuardRules { poise_basic: 0.0, poise_max_share: 0.2, poise_lockout: 20.0, revenge_giant: 7.0, revenge_heavy: 8.0, revenge_armour: 4.0, endurance_lockout: 15.0, chain_max: 3, chain_window: 10.0, chain_cooldown: 8.0 };
    /// `react.guard = 0`: as before this pass (a Giant's revenge limit of 7 stays kh-react's own, without effect).
    pub const OFF: GuardRules = GuardRules { poise_basic: 1.0, poise_max_share: f32::INFINITY, poise_lockout: 0.0, revenge_giant: 7.0, revenge_heavy: 0.0, revenge_armour: 0.0, endurance_lockout: 0.0, chain_max: 0, chain_window: 0.0, chain_cooldown: 0.0 };
}

/// Why a reaction is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Allow,
    /// The body is in its revenge (unflinchable).
    Armoured,
    /// Too many reactions in a row: the cool-down runs.
    Chain,
}

/// One armoured body's state.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Guard {
    clock: f32,
    armour_until: f32,
    poise_lock_until: f32,
    endurance_lock_until: f32,
    chain_until: f32,
    /// The times of the counted reactions (0 = unused; the clock starts at 1).
    reactions: [f32; 8],
    last_counted: f32,
    pub stance_breaks: u32,
    pub revenges: u32,
    pub refused: u32,
}

impl Guard {
    pub fn new() -> Guard {
        Guard { clock: 1.0, last_counted: -100.0, ..Guard::default() }
    }

    pub fn tick(&mut self, dt: f32) {
        self.clock += dt;
    }

    /// The body is unflinchable (its revenge runs).
    pub fn armoured(&self) -> bool {
        self.clock < self.armour_until
    }

    /// Seconds of the revenge armour / the poise lock / the endurance lock / the chain cool-down left.
    pub fn left(&self) -> [f32; 4] {
        [self.armour_until, self.poise_lock_until, self.endurance_lock_until, self.chain_until].map(|until| (until - self.clock).max(0.0))
    }

    /// (XSI, 2026-10-16) Rage Raid's END hit: the revenge armour ends now.
    pub fn end_revenge(&mut self) {
        self.armour_until = self.clock;
    }

    /// KH3's revenge value crossed the limit.
    pub fn revenge(&mut self, rules: &GuardRules) {
        self.revenges += 1;
        self.armour_until = self.clock + rules.revenge_armour;
    }

    /// Elden Ring's own stance break was seen. True: KH3's endurance is emptied with it (not while it is locked
    /// after a refill, not during a revenge).
    pub fn stance_broken(&mut self, rules: &GuardRules) -> bool {
        self.stance_breaks += 1;
        self.poise_lock_until = self.clock + rules.poise_lockout;
        !self.holds_endurance()
    }

    /// KH3's endurance refilled (its 10 s window closed).
    pub fn endurance_refilled(&mut self, rules: &GuardRules) {
        self.endurance_lock_until = self.clock + rules.endurance_lockout;
    }

    /// Nothing is taken off the endurance now.
    pub fn holds_endurance(&self) -> bool {
        self.armoured() || self.clock < self.endurance_lock_until
    }

    /// THE POISE DAMAGE of a hit on this body: `row` = its kind's poise number (already scaled), `reaction_power` =
    /// KH3's `m_ReactionPowerValue` of the attack, `poise_max` = the body's poise maximum.
    pub fn poise(&self, rules: &GuardRules, row: f32, reaction_power: i32, poise_max: f32) -> f32 {
        if self.armoured() || self.clock < self.poise_lock_until {
            return 0.0;
        }
        let asked = if reaction_power <= 0 { row * rules.poise_basic } else { row };
        let cap = if poise_max > 0.0 { poise_max * rules.poise_max_share } else { f32::INFINITY };
        asked.min(cap).max(0.0)
    }

    /// A reaction would start on this body: may it? An allowed one is counted.
    pub fn reaction(&mut self, rules: &GuardRules) -> Verdict {
        if self.armoured() {
            self.refused += 1;
            return Verdict::Armoured;
        }
        if self.clock < self.chain_until {
            self.refused += 1;
            return Verdict::Chain;
        }
        if rules.chain_max == 0 || self.clock - self.last_counted < CHAIN_GAP {
            return Verdict::Allow;
        }
        let recent = self.reactions.iter().filter(|t| **t > 0.0 && self.clock - **t <= rules.chain_window).count() as u32;
        if recent >= rules.chain_max {
            self.chain_until = self.clock + rules.chain_cooldown;
            self.reactions = [0.0; 8];
            self.refused += 1;
            return Verdict::Chain;
        }
        // (the oldest slot is reused)
        let slot = (0..self.reactions.len()).min_by(|a, b| self.reactions[*a].total_cmp(&self.reactions[*b])).unwrap_or(0);
        self.reactions[slot] = self.clock;
        self.last_counted = self.clock;
        Verdict::Allow
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: GuardRules = GuardRules::DEFAULT;

    fn run(g: &mut Guard, seconds: f32) {
        for _ in 0..(seconds * 60.0).round() as u32 {
            g.tick(1.0 / 60.0);
        }
    }

    /// THE USER'S FIGHT: Margit, stance 80; the ground combo = three basic hits (row poise 10, reaction power 0) and
    /// the finisher (row 60, power 1).
    #[test]
    fn a_ground_combo_no_longer_breaks_a_boss() {
        let g = Guard::new();
        let combo: f32 = [(10.0, 0), (10.0, 0), (10.0, 0), (60.0, 1)].iter().map(|(row, power)| g.poise(&R, *row, *power, 80.0)).sum();
        assert_eq!(combo, 16.0);
        // five finishers are needed (the game's own stance regeneration not counted)
        assert_eq!((80.0 / combo).ceil(), 5.0);
        // before this pass: 90 a combo
        let before: f32 = [(10.0, 0), (10.0, 0), (10.0, 0), (60.0, 1)].iter().map(|(row, power)| g.poise(&GuardRules::OFF, *row, *power, 80.0)).sum();
        assert_eq!(before, 90.0);
        // a body without a poise maximum gets the row's number
        assert_eq!(g.poise(&R, 60.0, 1, 0.0), 60.0);
    }

    #[test]
    fn no_poise_damage_after_a_stance_break_and_the_endurance_locks_after_its_refill() {
        let mut g = Guard::new();
        assert!(g.stance_broken(&R), "the first break empties KH3's endurance");
        assert_eq!(g.poise(&R, 60.0, 1, 80.0), 0.0);
        run(&mut g, 19.0);
        assert_eq!(g.poise(&R, 60.0, 1, 80.0), 0.0);
        run(&mut g, 1.5);
        assert_eq!(g.poise(&R, 60.0, 1, 80.0), 16.0);
        // the refill: 15 s in which neither reaction power nor another stance break takes it
        g.endurance_refilled(&R);
        assert!(g.holds_endurance());
        assert!(!g.stance_broken(&R));
        run(&mut g, 15.5);
        assert!(!g.holds_endurance());
        assert!(g.stance_broken(&R));
        assert_eq!(g.stance_breaks, 3);
    }

    #[test]
    fn revenge_makes_it_unflinchable_for_a_while() {
        let mut g = Guard::new();
        assert_eq!(g.reaction(&R), Verdict::Allow);
        g.revenge(&R);
        assert!(g.armoured() && g.holds_endurance());
        assert_eq!(g.reaction(&R), Verdict::Armoured);
        assert_eq!(g.poise(&R, 60.0, 5, 80.0), 0.0);
        run(&mut g, 3.9);
        assert_eq!(g.reaction(&R), Verdict::Armoured);
        run(&mut g, 0.2);
        assert!(!g.armoured());
        assert_eq!(g.reaction(&R), Verdict::Allow);
        assert_eq!((g.revenges, g.refused), (1, 2));
    }

    #[test]
    fn three_reactions_in_ten_seconds_then_a_cool_down() {
        let mut g = Guard::new();
        // a volley of twenty hits in a third of a second is one stagger
        for _ in 0..20 {
            assert_eq!(g.reaction(&R), Verdict::Allow);
            run(&mut g, 1.0 / 60.0);
        }
        run(&mut g, 2.0);
        assert_eq!(g.reaction(&R), Verdict::Allow);
        run(&mut g, 2.0);
        assert_eq!(g.reaction(&R), Verdict::Allow);
        run(&mut g, 2.0);
        // the fourth within the window: refused, and so is everything for 8 s
        assert_eq!(g.reaction(&R), Verdict::Chain);
        run(&mut g, 7.5);
        assert_eq!(g.reaction(&R), Verdict::Chain);
        run(&mut g, 1.0);
        assert_eq!(g.reaction(&R), Verdict::Allow);
        // spaced out (one every 6 s) it never trips
        let mut g = Guard::new();
        for _ in 0..10 {
            assert_eq!(g.reaction(&R), Verdict::Allow);
            run(&mut g, 6.0);
        }
        // switched off: never
        let mut g = Guard::new();
        for _ in 0..30 {
            assert_eq!(g.reaction(&GuardRules::OFF), Verdict::Allow);
            run(&mut g, 0.6);
        }
    }
}
