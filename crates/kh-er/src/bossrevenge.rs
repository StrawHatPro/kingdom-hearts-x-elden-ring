//! (REVENGE, 2026-11-06) KH3'S REVENGE FOR ELDEN RING'S BOSSES: pure bookkeeping, no game access. The Elden Ring side is
//! puppet.rs (which body is a boss, the meter's limit, the stun gate, the revenge) and aireplan.rs (the boss's AI asked to
//! plan its next action now). build/sora_tmp/revenge/STATE.md has the evidence.
//!
//! THE USER (2026-11-06): "do not make the shockwave for revenge value but the boss should try to start their own combo.
//! Different bosses should have different limits, from a span of 6 - 10" ("change the minimum to 6.5 to 10"), and
//! "allow bosses to get hitstunned while they're not currently attacking or in a combo, like how KH3 bosses work. Respect
//! revenge as they can still break out of hitstun too".
//!
//! THE RULES (every number a row of sheets/port/sora-er-host.json, `react.revenge.boss*` and `react.boss_stun*`):
//! 1. WHICH BODIES: any enemy on Elden Ring's boss bar, decided every frame (no list of bosses). With
//!    `react.revenge.bosses_only` = 1 no other body has a revenge (its limit is 0); with the default 0 a non-boss
//!    armoured body keeps its class row (bossguard.rs: Giant 7, Heavy 8) and its revenge armour exactly as before. The
//!    AI replan (rule 3) and the stun (rule 4) are for bosses only either way.
//! 2. THE LIMIT of a boss is mapped from its own Elden Ring data onto `boss_min` .. `boss_max`: its stance (the larger of
//!    its super-armour and toughness maxima) on `boss_stance_lo` .. `boss_stance_hi` (linear) and its maximum HP on
//!    `boss_hp_lo` .. `boss_hp_hi` (logarithmic), mixed by `boss_stance_weight`, each clamped to 0 .. 1. With
//!    `boss_tougher_lower` the tougher boss gets the LOWER limit (it breaks out sooner). The meter itself is KH3's
//!    (kh-react: every hit adds its row's revengepoint, held 3 s, then 5 a second off; after 20 s the next hit fills it).
//! 3. ON FULL: the reaction holding it ends, it is unflinchable for `react.revenge.armour` seconds (bossguard.rs), the
//!    meter is reset and earns nothing meanwhile, and (`boss_replan`) its AI is asked to plan its next action at once:
//!    its own battle goal chooses from its own moveset (aireplan.rs). No shockwave, no knock-back, no attack id forced.
//! 4. THE STUN (`react.boss_stun`): a STRONG (armoured) boss that is NOT attacking (no attack animation, no TAE super
//!    armour, and none for `boss_stun.combo_grace` seconds: the gap between the parts of its combo) takes KH3's hit-stun
//!    (a flinch, never a launch) from every hit, basic combo hits included. While it attacks it keeps KH3's super armour:
//!    no reaction, its attack is not interrupted (the hit still adds revenge points). An emptied endurance (its stance
//!    broken) is KH3's open window as before. The stuns count against the chain cap with `boss_stun.chain_max`
//!    (0 = `react.chain.max`); the revenge armour refuses every stun.

/// The rows `react.revenge.boss*` and `react.boss_stun*`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BossRules {
    /// `react.revenge.boss`: bosses have KH3's revenge (rules 1 - 3).
    pub on: bool,
    /// `react.revenge.boss_min` / `_max`: the span of a boss's revenge limit (KH3 revenge points).
    pub min: f32,
    pub max: f32,
    /// `react.revenge.boss_tougher_lower`: the tougher boss gets the lower limit.
    pub tougher_lower: bool,
    /// `react.revenge.boss_stance_lo` / `_hi`: the stance mapped onto 0 .. 1 (linear).
    pub stance_lo: f32,
    pub stance_hi: f32,
    /// `react.revenge.boss_hp_lo` / `_hi`: the maximum HP mapped onto 0 .. 1 (logarithmic).
    pub hp_lo: f32,
    pub hp_hi: f32,
    /// `react.revenge.boss_stance_weight`: the stance's share of the toughness (the HP has the rest).
    pub stance_weight: f32,
    /// `react.revenge.bosses_only`: no other body has a revenge.
    pub bosses_only: bool,
    /// `react.revenge.boss_replan`: on full, the boss's AI is asked to plan its next action now.
    pub replan: bool,
    /// `react.boss_stun`: an armoured boss that is not attacking takes KH3's hit-stun (rule 4).
    pub stun: bool,
    /// `react.boss_stun.combo_grace`: seconds after its last attack frame a boss still counts as in its combo.
    pub combo_grace: f32,
    /// `react.boss_stun.chain_max`: the chain cap of the stuns (0 = `react.chain.max`).
    pub stun_chain_max: u32,
    /// `react.revenge.mimic_limit`: the Mimic Tear boss copies' FIXED revenge limit (the user 2026-11-07: "make it 6.5"); 0 = the
    /// formula of rule 2 as for the other bosses.
    pub mimic_limit: f32,
    /// `react.revenge.retarget`: on its revenge a boss goes for whoever added the most revenge points in the last
    /// `retarget_window` seconds (ties: the leader), the AI replan asked `retarget_wait` seconds later (after its hate hit).
    pub retarget: bool,
    pub retarget_window: f32,
    pub retarget_wait: f32,
}

impl BossRules {
    /// The sheet's rows.
    pub const DEFAULT: BossRules = BossRules { on: true, min: 6.5, max: 8.0, tougher_lower: true, stance_lo: 60.0, stance_hi: 200.0, hp_lo: 2000.0, hp_hi: 20000.0, stance_weight: 0.5, bosses_only: false, replan: true, stun: true, combo_grace: 0.6, stun_chain_max: 6, mimic_limit: 6.5, retarget: true, retarget_window: 5.0, retarget_wait: 0.2 };
    /// `react.guard = 0`: nothing of this pass (the class limits of bossguard.rs as before).
    pub const OFF: BossRules = BossRules { on: false, bosses_only: false, replan: false, stun: false, ..BossRules::DEFAULT };

    /// One row of the sheet; false: not one of these rows, or out of range.
    pub fn apply_row(&mut self, key: &str, v: f32) -> bool {
        match key {
            "react.revenge.boss" => self.on = v != 0.0,
            "react.revenge.boss_min" if (0.5..=100.0).contains(&v) => self.min = v,
            "react.revenge.boss_max" if (0.5..=100.0).contains(&v) => self.max = v,
            "react.revenge.boss_tougher_lower" => self.tougher_lower = v != 0.0,
            "react.revenge.boss_stance_lo" if (0.0..=10_000.0).contains(&v) => self.stance_lo = v,
            "react.revenge.boss_stance_hi" if (0.0..=10_000.0).contains(&v) => self.stance_hi = v,
            "react.revenge.boss_hp_lo" if (1.0..=10_000_000.0).contains(&v) => self.hp_lo = v,
            "react.revenge.boss_hp_hi" if (1.0..=10_000_000.0).contains(&v) => self.hp_hi = v,
            "react.revenge.boss_stance_weight" if (0.0..=1.0).contains(&v) => self.stance_weight = v,
            "react.revenge.bosses_only" => self.bosses_only = v != 0.0,
            "react.revenge.boss_replan" => self.replan = v != 0.0,
            "react.boss_stun" => self.stun = v != 0.0,
            "react.boss_stun.combo_grace" if (0.0..=10.0).contains(&v) => self.combo_grace = v,
            "react.boss_stun.chain_max" if (0.0..=8.0).contains(&v) && v.fract() == 0.0 => self.stun_chain_max = v as u32,
            "react.revenge.mimic_limit" if (0.0..=100.0).contains(&v) => self.mimic_limit = v,
            "react.revenge.retarget" => self.retarget = v != 0.0,
            "react.revenge.retarget_window" if (0.0..=60.0).contains(&v) => self.retarget_window = v,
            "react.revenge.retarget_wait" if (0.0..=2.0).contains(&v) => self.retarget_wait = v,
            _ => return false,
        }
        true
    }

    /// HOW TOUGH a boss is by its own data, 0 .. 1: `stance` = its stance maximum (0 = none), `hp_max` = its maximum HP.
    pub fn toughness(&self, stance: f32, hp_max: f32) -> f32 {
        let share = |t: f32| if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.0 };
        let s = if self.stance_hi > self.stance_lo { share((stance - self.stance_lo) / (self.stance_hi - self.stance_lo)) } else { 0.0 };
        let h = if self.hp_hi > self.hp_lo && hp_max > 0.0 { share((hp_max / self.hp_lo).ln() / (self.hp_hi / self.hp_lo).ln()) } else { 0.0 };
        let w = self.stance_weight.clamp(0.0, 1.0);
        w * s + (1.0 - w) * h
    }

    /// THE REVENGE LIMIT of a boss (rule 2), within `min` .. `max`.
    pub fn limit(&self, stance: f32, hp_max: f32) -> f32 {
        let (lo, hi) = (self.min.min(self.max), self.min.max(self.max));
        let t = self.toughness(stance, hp_max);
        let t = if self.tougher_lower { 1.0 - t } else { t };
        (lo + t * (hi - lo)).clamp(lo, hi)
    }

    /// The revenge limit a body gets: a boss its own (rule 2); another body `class_limit` (bossguard.rs's class row),
    /// or none with `bosses_only`.
    pub fn limit_for(&self, boss: bool, stance: f32, hp_max: f32, class_limit: f32) -> f32 {
        match (self.on, boss) {
            (true, true) => self.limit(stance, hp_max),
            (true, false) if self.bosses_only => 0.0,
            _ => class_limit,
        }
    }
}

/// What a hit may do to an armoured boss (rule 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StunGate {
    /// Not a case of rule 4 (not a boss, the rule off, an unarmoured body, a forced or lethal hit): as before.
    Default,
    /// Not attacking: KH3's hit-stun (a flinch) from any hit.
    Stun,
    /// Attacking or in its combo: KH3's super armour, no reaction.
    Armour,
    /// Its endurance is empty (its stance broken): KH3's open window, as before.
    Open,
}

/// THE GATE of rule 4: `attacking` = an attack animation or the game's super armour now; `since_attack` = seconds since
/// the last such frame; `endurance_open` = its KH3 endurance is empty.
pub fn stun_gate(rules: &BossRules, boss: bool, strong: bool, attacking: bool, since_attack: f32, endurance_open: bool) -> StunGate {
    if !(rules.on && rules.stun && boss && strong) {
        return StunGate::Default;
    }
    if endurance_open {
        return StunGate::Open;
    }
    if attacking || since_attack < rules.combo_grace {
        return StunGate::Armour;
    }
    StunGate::Stun
}

/// THE STUN as KH3's reaction is asked for it: a flinch (SMALL; a boss is never launched by it) past the STRONG
/// body's gate (kh-core marks a hit on an armoured body StrongBodyDamage1 - 3, which turns SMALL hits away); a guard
/// or a reflection stays what it is.
pub fn stun_request(request: &mut kh_react::ReactionRequest) {
    use kh_core::host::ReactionResult as V;
    if matches!(request.victim_result, V::StrongBodyDamage1 | V::StrongBodyDamage2 | V::StrongBodyDamage3) {
        request.victim_result = V::Damage;
    }
    request.kind = kh_react::ETresDamageKind::Small;
}

/// (REVENGE) One party hit's revenge points on a boss: when (the puppets' clock), by whom (aggro.rs `Who::code`: 1 Sora,
/// 2 Donald, 3 Goofy, 4 Mimic Sora; 0 = not known, the leader's), how many.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contrib {
    pub at: f32,
    pub who: u8,
    pub points: f32,
}

/// (REVENGE) WHO TRIGGERED THE REVENGE: the member who added the most revenge points within `window` seconds before `now`;
/// a tie (or an unknown hitter) goes to the `leader`. None: no points at all.
pub fn top_contributor(book: &[Contrib], now: f32, window: f32, leader: u8) -> Option<u8> {
    let mut sums: Vec<(u8, f32)> = Vec::new();
    for c in book.iter().filter(|c| now - c.at <= window && c.points > 0.0) {
        let who = if c.who == 0 { leader } else { c.who };
        match sums.iter_mut().find(|s| s.0 == who) {
            Some(s) => s.1 += c.points,
            None => sums.push((who, c.points)),
        }
    }
    let best = sums.iter().map(|s| s.1).fold(f32::NEG_INFINITY, f32::max);
    if !best.is_finite() {
        return None;
    }
    let top: Vec<u8> = sums.iter().filter(|s| (s.1 - best).abs() < 1.0e-4).map(|s| s.0).collect();
    Some(if top.len() == 1 { top[0] } else { leader })
}

/// The chain cap a stun counts against.
pub fn stun_chain_max(rules: &BossRules, chain_max: u32) -> u32 {
    if rules.stun_chain_max > 0 { rules.stun_chain_max } else { chain_max }
}

/// The last frame a boss was seen attacking (the puppets' clock; it starts long ago).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttackWatch {
    last: f32,
}

impl Default for AttackWatch {
    fn default() -> Self {
        AttackWatch { last: -1000.0 }
    }
}

impl AttackWatch {
    /// One frame's look at the body: `attacking` = an attack animation or the game's super armour.
    pub fn seen(&mut self, clock: f32, attacking: bool) {
        if attacking {
            self.last = clock;
        }
    }

    /// Seconds since it was last seen attacking.
    pub fn since(&self, clock: f32) -> f32 {
        (clock - self.last).max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bossguard::{Guard, GuardRules, Verdict};
    use kh_react::{Body, BodyClass, EnemyReaction, HitOutcome, ReactionRequest};

    const R: BossRules = BossRules::DEFAULT;

    /// Regulation 2.7.2.0 (build/sora_tmp/revenge/STATE.md): NpcParam hp x its area-scaling SpEffect maxHpRate, and
    /// superArmorDurability (the stance).
    const MARGIT: (f32, f32) = (80.0, 2521.0 * 1.656);
    const GODRICK: (f32, f32) = (105.0, 3354.0 * 1.813);
    const RENNALA_2: (f32, f32) = (80.0, 1703.0 * 2.406);
    const RADAHN: (f32, f32) = (200.0, 2585.0 * 3.703);
    const MALENIA: (f32, f32) = (80.0, 2489.0 * 7.422);

    #[test]
    fn the_limit_maps_a_bosss_own_data_onto_the_span_and_clamps() {
        let lim = |b: (f32, f32)| R.limit(b.0, b.1);
        // the weakest possible and beyond: the top of the span; the toughest and beyond: its bottom
        assert_eq!(R.limit(0.0, 1.0), 8.0);
        assert_eq!(R.limit(-5.0, 0.0), 8.0);
        assert_eq!(R.limit(200.0, 20000.0), 6.5);
        assert_eq!(R.limit(5000.0, 9.0e6), 6.5);
        assert!(R.limit(f32::NAN, f32::INFINITY) >= 6.5);
        // the known bosses: within the span, the tougher lower
        for b in [MARGIT, GODRICK, RENNALA_2, RADAHN, MALENIA] {
            assert!((6.5..=8.0).contains(&lim(b)), "{b:?} -> {}", lim(b));
        }
        assert!(lim(RADAHN) < lim(MALENIA) && lim(MALENIA) < lim(GODRICK) && lim(GODRICK) < lim(MARGIT));
        // the examples of build/sora_tmp/revenge/STATE.md 3 (and the row notes)
        for (b, want) in [(MARGIT, 7.65), (RENNALA_2, 7.66), (GODRICK, 7.40), (MALENIA, 7.17), (RADAHN, 6.74)] {
            assert!((lim(b) - want).abs() < 0.006, "{b:?}: {} (documented {want})", lim(b));
        }
        // the Mimic Tear (stance 100): the copied player's HP x 3 (SpEffect 19397), so it follows the player
        for (player_hp, want) in [(1000.0, 7.65), (1900.0, 7.44)] {
            assert!((R.limit(100.0, player_hp * 3.0) - want).abs() < 0.006, "{player_hp}: {}", R.limit(100.0, player_hp * 3.0));
        }
        // the direction row turned round: the tougher higher
        let up = BossRules { tougher_lower: false, ..R };
        assert!(up.limit(RADAHN.0, RADAHN.1) > up.limit(MARGIT.0, MARGIT.1));
        assert_eq!(up.limit(0.0, 1.0), 6.5);
        // a span typed the wrong way round is still a span
        let swapped = BossRules { min: 8.0, max: 6.5, ..R };
        assert_eq!(swapped.limit(MARGIT.0, MARGIT.1), R.limit(MARGIT.0, MARGIT.1));
    }

    #[test]
    fn the_revenge_goes_for_whoever_added_the_most_points_ties_to_the_leader() {
        let c = |at, who, points| Contrib { at, who, points };
        // Goofy 3 points, the leader Sora 2, Donald 1.5: Goofy
        let book = [c(1.0, 3, 1.0), c(1.5, 1, 1.0), c(2.0, 3, 1.0), c(2.5, 2, 1.5), c(3.0, 1, 1.0), c(3.5, 3, 1.0)];
        assert_eq!(top_contributor(&book, 4.0, 5.0, 1), Some(3));
        // only the last 1.6 s: Donald 1.5, Sora 1, Goofy 1 -> Donald
        assert_eq!(top_contributor(&book, 4.0, 1.6, 1), Some(2));
        // a tie: the leader
        assert_eq!(top_contributor(&[c(1.0, 2, 1.0), c(1.2, 3, 1.0)], 2.0, 5.0, 1), Some(1));
        // an unknown hitter counts for the leader
        assert_eq!(top_contributor(&[c(1.0, 0, 2.0), c(1.2, 3, 1.0)], 2.0, 5.0, 1), Some(1));
        // nothing within the window
        assert_eq!(top_contributor(&book, 30.0, 5.0, 1), None);
    }

    #[test]
    fn bosses_get_their_own_limit_others_keep_their_class_limit() {
        // (the coordinator 2026-11-07) a non-boss armoured body keeps its class limit (Giant 7 / Heavy 8) as before
        assert_eq!(R.limit_for(false, 80.0, 4000.0, 7.0), 7.0);
        assert_eq!(R.limit_for(false, 80.0, 4000.0, 8.0), 8.0);
        assert!(R.limit_for(true, 80.0, 4000.0, 7.0) > 6.5);
        // the row turned on: no revenge for them
        let only = BossRules { bosses_only: true, ..R };
        assert_eq!(only.limit_for(false, 80.0, 4000.0, 7.0), 0.0);
        assert_eq!(BossRules::OFF.limit_for(true, 80.0, 4000.0, 7.0), 7.0);
        assert_eq!(BossRules::OFF.limit_for(false, 80.0, 4000.0, 8.0), 8.0);
    }

    #[test]
    fn the_rows_are_read_and_out_of_range_values_refused() {
        let mut r = BossRules::OFF;
        for (k, v) in [("react.revenge.boss", 1.0), ("react.revenge.boss_min", 6.5), ("react.revenge.boss_max", 8.0), ("react.revenge.boss_tougher_lower", 1.0), ("react.revenge.bosses_only", 0.0), ("react.revenge.boss_replan", 1.0), ("react.boss_stun", 1.0)] {
            assert!(r.apply_row(k, v), "{k}");
        }
        assert_eq!(r, R);
        assert!(!r.apply_row("react.boss_stun.chain_max", 2.5));
        assert!(!r.apply_row("react.revenge.boss_stance_weight", 2.0));
        assert!(!r.apply_row("react.revenge.giant", 7.0));
        assert_eq!(r, R);
    }

    #[test]
    fn an_idle_boss_is_stunned_an_attacking_one_is_not() {
        let gate = |attacking, since, open| stun_gate(&R, true, true, attacking, since, open);
        assert_eq!(gate(false, 5.0, false), StunGate::Stun);
        assert_eq!(gate(true, 0.0, false), StunGate::Armour);
        // between the parts of its combo
        assert_eq!(gate(false, 0.3, false), StunGate::Armour);
        assert_eq!(gate(false, 0.7, false), StunGate::Stun);
        // its stance broken: the open window as before
        assert_eq!(gate(true, 0.0, true), StunGate::Open);
        // not a boss, not armoured, the rule off
        assert_eq!(stun_gate(&R, false, true, false, 5.0, false), StunGate::Default);
        assert_eq!(stun_gate(&R, true, false, false, 5.0, false), StunGate::Default);
        assert_eq!(stun_gate(&BossRules { stun: false, ..R }, true, true, false, 5.0, false), StunGate::Default);
        // the watch: a frame of attack, then the grace runs out
        let mut w = AttackWatch::default();
        assert!(w.since(0.0) > R.combo_grace);
        w.seen(10.0, true);
        w.seen(10.5, false);
        assert_eq!(w.since(10.5), 0.5);
        assert_eq!(stun_gate(&R, true, true, false, w.since(10.5), false), StunGate::Armour);
        assert_eq!(stun_gate(&R, true, true, false, w.since(11.0), false), StunGate::Stun);
    }

    /// One basic combo hit on an armoured body: kh-core marks it StrongBodyDamage1 (KH3: a SMALL hit never moves a
    /// STRONG body).
    fn basic_hit(point: f32) -> ReactionRequest {
        let mut q = ReactionRequest::new(kh_core::enums::ETresDamageKind::Small, glam::Vec3::X);
        q.victim_result = kh_core::host::ReactionResult::StrongBodyDamage1;
        q.revenge_point = point;
        q
    }

    /// The hit as puppet.rs asks it of a stunned boss ([`stun_request`]).
    fn stun_hit(r: &mut EnemyReaction, point: f32) -> HitOutcome {
        let mut q = basic_hit(point);
        stun_request(&mut q);
        r.hit(&q, kh_react::Pose { position: glam::Vec3::ZERO, yaw: 0.0, grounded: true })
    }

    #[test]
    fn a_basic_hit_moves_a_boss_only_as_a_stun() {
        let mut body = Body::of(BodyClass::Giant);
        body.revenge_limit = 9.0;
        let mut r = EnemyReaction::new(body);
        let pose = kh_react::Pose { position: glam::Vec3::ZERO, yaw: 0.0, grounded: true };
        // KH3's own STRONG gate: nothing (the boss attacking: rule 4's armour keeps exactly this)
        assert_eq!(r.hit(&basic_hit(1.0), pose), HitOutcome::NoReaction);
        assert_eq!(r.revenge_value(), 1.0, "the refused hit still adds its revenge points");
        // the stun: a flinch, never a launch
        let mut launch = basic_hit(1.0);
        launch.kind = kh_core::enums::ETresDamageKind::Launch;
        launch.victim_result = kh_core::host::ReactionResult::StrongBodyDamage2;
        stun_request(&mut launch);
        assert!(matches!(r.hit(&launch, pose), HitOutcome::Started { kind: kh_core::enums::ETresDamageKind::Small, .. }));
        // a guard or a reflection is left as it is
        let mut guard = basic_hit(0.0);
        guard.victim_result = kh_core::host::ReactionResult::Guard;
        stun_request(&mut guard);
        assert_eq!(guard.victim_result, kh_core::host::ReactionResult::Guard);
    }

    fn step(r: &mut EnemyReaction, g: &mut Guard, seconds: f32) {
        for _ in 0..(seconds * 60.0).round() as u32 {
            r.step_dt(&kh_react::FlatWorld { floor_z: 0.0, walls: Vec::new() }, 1.0 / 60.0);
            g.tick(1.0 / 60.0);
        }
    }

    /// THE METER: KH3's own (kh-react), at a boss's limit: held 3 s after a hit, then 5 a second off.
    #[test]
    fn the_meter_holds_then_decays() {
        let mut body = Body::of(BodyClass::Giant);
        body.revenge_limit = R.limit(MARGIT.0, MARGIT.1);
        body.revenge_cooldown = GuardRules::DEFAULT.revenge_armour;
        let (mut r, mut g) = (EnemyReaction::new(body), Guard::new());
        for _ in 0..4 {
            stun_hit(&mut r, 1.0);
            step(&mut r, &mut g, 0.4);
        }
        assert_eq!(r.revenge_value(), 4.0);
        step(&mut r, &mut g, 2.4);
        assert_eq!(r.revenge_value(), 4.0, "held 3 s after the last hit");
        step(&mut r, &mut g, 0.5);
        assert!(r.revenge_value() < 4.0 && r.revenge_value() > 2.0, "{}", r.revenge_value());
        step(&mut r, &mut g, 1.0);
        assert_eq!(r.revenge_value(), 0.0);
        assert!(!r.take_revenge());
    }

    /// A CHAIN OF STUNS ON AN IDLE BOSS ends in its revenge: the armour refuses every stun, the meter earns nothing
    /// meanwhile, then stuns are allowed again.
    #[test]
    fn revenge_breaks_a_stun_chain_and_arms_the_boss() {
        let guard = GuardRules::DEFAULT;
        let chain = GuardRules { chain_max: stun_chain_max(&R, guard.chain_max), ..guard };
        let mut body = Body::of(BodyClass::Giant);
        body.revenge_limit = R.limit(MARGIT.0, MARGIT.1); // 7.65
        body.revenge_cooldown = guard.revenge_armour;
        let (mut r, mut g) = (EnemyReaction::new(body), Guard::new());
        let mut stuns = 0;
        let mut broke_out = None;
        // a basic hit (revengepoint 1) every 0.6 s, the boss idle all the time
        for n in 0..14 {
            assert_eq!(stun_gate(&R, true, true, false, 5.0, r.endurance() <= 0), StunGate::Stun);
            if matches!(stun_hit(&mut r, 1.0), HitOutcome::Started { .. }) && g.reaction(&chain) == Verdict::Allow {
                stuns += 1;
            }
            step(&mut r, &mut g, 0.6);
            if r.take_revenge() {
                r.clear_revenge();
                g.revenge(&guard);
                broke_out = Some(n);
                break;
            }
        }
        assert_eq!(broke_out, Some(7), "the 8th hit fills a limit of 7.65");
        assert!(stuns >= 6, "it was stunned until then ({stuns})");
        // the armour: no stun, no points
        assert!(g.armoured());
        stun_hit(&mut r, 1.0);
        assert_eq!(g.reaction(&chain), Verdict::Armoured);
        assert_eq!(r.revenge_value(), 0.0, "no revenge points during the armour");
        step(&mut r, &mut g, guard.revenge_armour + 0.1);
        assert!(!g.armoured());
        assert!(matches!(stun_hit(&mut r, 1.0), HitOutcome::Started { .. }));
        assert_eq!(r.revenge_value(), 1.0);
    }

    /// THE STUN CHAIN CAP: without revenge points (a row with revengepoint 0) the cap ends a stun chain.
    #[test]
    fn the_chain_cap_still_ends_a_chain_without_revenge_points() {
        let guard = GuardRules::DEFAULT;
        let chain = GuardRules { chain_max: stun_chain_max(&R, guard.chain_max), ..guard };
        assert_eq!(chain.chain_max, 6);
        assert_eq!(stun_chain_max(&BossRules { stun_chain_max: 0, ..R }, 3), 3);
        let mut g = Guard::new();
        let mut allowed = 0;
        for _ in 0..12 {
            if g.reaction(&chain) == Verdict::Allow {
                allowed += 1;
            }
            for _ in 0..36 {
                g.tick(1.0 / 60.0);
            }
        }
        assert_eq!(allowed, 6, "six within its window, then the cool-down");
        // the cool-down (react.chain.cooldown) over: stuns again
        for _ in 0..(guard.chain_cooldown * 60.0) as u32 {
            g.tick(1.0 / 60.0);
        }
        assert_eq!(g.reaction(&chain), Verdict::Allow);
    }
}
