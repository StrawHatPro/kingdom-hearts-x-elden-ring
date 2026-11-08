//! (THE IN-GAME TESTER 2026-11-03, run 2 at Stormveil: Goofy's Breaker on Margit, the Fell Omen asked attack 4224, the KH
//! side expected 2749 of his 3456 HP to go (707 left), the game took all 3456 and the boss died with its reward granted)
//! A KH HIT NEVER TAKES MORE THAN THE KH SIDE EXPECTS, AND NEVER KILLS A BODY THE KH SIDE EXPECTS TO SURVIVE.
//!
//! THE CAUSE: a hit goes out as the player's bullet asking KH's damage x `damage.out.scale` and the KH side counts on the
//! game taking that times the share this body was measured to take (targets.rs `Memory::ratio`, a running mean). On
//! Margit that mean was 0.65: his first sample was a 1-HP take of 352 asked (clamped to x0.02), then two x0.86. That 1 HP
//! was the aggro director's 1-point hate hit landing first (the shields agent's finding): `split_side` keeps such hits out.
//! Elden Ring's defence is no fixed share either: its curve passes more of a big attack value than of a small one (up to
//! 0.9 of it), and a weakness, a counter hit or a damage-taken effect raise it further (x1.54 measured on a c3500). So
//! the big Breaker took x0.82 or more where x0.65 was expected, and that was all he had left.
//!
//! THE RULES (pure, here; targets.rs applies them):
//! - THE WORST CASE: no bullet of ours takes more than `ceiling` HP per attack point asked (`CEILING`, or a margin over
//!   the most this body was ever seen to take, whichever is more).
//! - A BULLET THAT COULD KILL A BODY THE KH SIDE EXPECTS TO LIVE asks only what leaves it 1 HP at the worst (counting
//!   the bullets still in flight at it at their worst); the rest of the expected HP is written to its HP directly once
//!   the game has taken its share (the bullet still carries the hit's poise, stamina and damage level: the game's
//!   reaction is unchanged). Too little room for any bullet: the expected HP is written directly, no bullet.
//! - A HIT THE KH SIDE EXPECTS TO KILL is never capped: the game's own death, credit and rewards follow.
//! - WHAT THE GAME TOOK OVER THE EXPECTED HP (past `tolerance`) is given back to a body that lives.

/// The most Elden Ring HP one attack point of our bullet takes off a body at the worst. Elden Ring's defence curve passes
/// at most 0.9 of an attack value; a weakness (a negative absorption), a counter hit or a damage-taken effect on the body
/// raise that (the most measured: x1.54). Twice the attack value is beyond all of them.
pub const CEILING: f32 = 2.0;

/// The margin over the most a body has been seen to take per attack point, when that is above `CEILING` / this.
pub const SEEN_MARGIN: f32 = 1.25;

/// The worst case per attack point on a body that was seen to take at most `seen_max` per point (0: never measured).
pub fn ceiling(seen_max: f32) -> f32 {
    if seen_max.is_finite() { CEILING.max(seen_max * SEEN_MARGIN) } else { CEILING }
}

/// HP the game may take over the expected before any is given back: a twentieth of it, at least 2 (rounding, a stray tick).
pub fn tolerance(expected: i32) -> i32 {
    (expected / 20).max(2)
}

/// What one hit's bullet asks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plan {
    /// The attack value the bullet asks; 0: no bullet (the expected HP is written to the body's HP directly).
    pub ask: i32,
    /// The bullet asks less than the hit's attack value: what the game's defences leave of the expected HP is written.
    pub capped: bool,
}

/// The KH side expects the body to live after this hit: `hp` (the game's) less what the bullets in flight at it and this
/// hit are expected to take leaves it something.
pub fn survives(expected: i32, hp: i32, flight_expected: i32) -> bool {
    hp - flight_expected - expected > 0
}

/// One hit's bullet. `wanted`: the attack value of KH's damage; `expected`: the HP the KH side counts on it taking;
/// `hp`: the body's HP as the game has it now; `flight_expected` / `flight_worst`: what the bullets still in flight at it
/// are expected to take / could take at the worst; `ceiling`: the worst case per attack point (`ceiling`).
pub fn plan(wanted: i32, expected: i32, hp: i32, flight_expected: i32, flight_worst: i32, ceiling: f32) -> Plan {
    let whole = Plan { ask: wanted, capped: false };
    // (a kill the KH side counts on: the game's own death)
    if wanted <= 0 || !survives(expected, hp, flight_expected) {
        return whole;
    }
    // (the HP the worst case may take and still leave it 1)
    let room = hp - flight_worst - 1;
    if wanted as f32 * ceiling <= room as f32 {
        return whole;
    }
    let ask = (room as f32 / ceiling).floor() as i32;
    Plan { ask: if ask >= 1 { ask.min(wanted) } else { 0 }, capped: true }
}

/// What a bullet could take at the worst: its ask times the ceiling, or the HP that is to be written for it.
pub fn worst_of(ask: i32, full: i32, ceiling: f32) -> i32 {
    ((ask.max(0) as f32 * ceiling).ceil() as i32).max(full)
}

/// HP to give back after the game took `drop` for bullets the KH side expected to take `expected` (0: none expected, a
/// kill or a hit without a ceiling): everything over the expected HP, once it is past the tolerance.
pub fn give_back(drop: i32, expected: i32) -> i32 {
    if expected <= 0 || drop <= expected + tolerance(expected) { 0 } else { drop - expected }
}

/// (THE SHIELDS AGENT'S FINDING 2026-11-03, the coordinator: "the x0.02 first hit is our own aggro HATE HIT") The party's
/// 1-point bullets (the aggro director's hate hit, Donald's aggro, Goofy's Over Here!: `ErTargets::note_side_hit`) land on
/// a body a frame before or around a KH hit. Their take was counted as the KH bullet's (Margit's first sample: 1 HP for 352
/// asked, x0.02 into his ratio; the real hit a frame later was then never seen). THE RULE: of a drop, what the side hits in
/// flight at the body could take at the worst (`side_worst`) is theirs, and a drop no bigger than that is all theirs: the
/// KH bullets stay in flight, no share is learnt. A bigger drop: the side hits took at least a point each (`side_least`,
/// the game's floor), the rest is the KH bullets'. Returns (the side hits' take, the KH bullets' take).
pub fn split_side(drop: i32, side_least: i32, side_worst: i32) -> (i32, i32) {
    if side_worst <= 0 || drop <= 0 {
        (0, drop.max(0))
    } else if drop <= side_worst {
        (drop, 0)
    } else {
        let side = side_least.clamp(0, drop);
        (side, drop - side)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hate_hit_s_point_is_never_the_kh_bullet_s_measure() {
        // run 2 at 75.1 s: Margit took 1 HP with the hate hit (1 point) and the 352-point KH bullet in flight: the hate hit's
        let (side, kh) = split_side(1, 1, worst_of(1, 0, CEILING));
        assert_eq!((side, kh), (1, 0), "the KH bullet stays in flight, no x0.02");
        // both in the same frame: 1 + 186 -> 1 the hate hit's, 186 the KH bullet's (x0.53 of 352, not x0.53 of 353)
        assert_eq!(split_side(187, 1, 2), (1, 186));
        // no side hit in flight: all the KH bullet's
        assert_eq!(split_side(186, 0, 0), (0, 186));
        // Over Here! with 50 points: up to 100 is its own
        assert_eq!(split_side(60, 50, worst_of(50, 0, CEILING)), (60, 0));
        assert_eq!(split_side(300, 50, 100), (50, 250));
    }

    /// Elden Ring's damage for one physical attack value against a defence and an absorption (the community's formula of
    /// the defence curve), for proving the rule against the game's own non-linear numbers.
    fn er_damage(attack: f32, defence: f32, absorption: f32) -> f32 {
        let r = attack / defence.max(1.0);
        let share = if r > 8.0 {
            0.9
        } else if r > 2.5 {
            0.9 - 0.2 * ((8.0 - r) / 5.5).powi(2)
        } else if r > 1.0 {
            0.7 - 0.6 * ((2.5 - r) / 1.5).powi(2)
        } else if r > 0.125 {
            0.1 + 0.3 * ((r - 0.125) / 0.875).powi(2)
        } else {
            0.1
        };
        attack * share * (1.0 - absorption)
    }

    /// One hit through the whole rule: the bullet (its take by the game's formula times `bonus`: a counter hit, a
    /// weakness), the HP written for a capped one, what is given back. Returns (HP taken in all, dead).
    fn land(wanted: i32, expected: i32, hp: i32, defence: f32, absorption: f32, bonus: f32) -> (i32, bool) {
        let p = plan(wanted, expected, hp, 0, 0, ceiling(0.0));
        let took = if p.ask > 0 { ((er_damage(p.ask as f32, defence, absorption) * bonus).round() as i32).min(hp) } else { 0 };
        if took >= hp {
            return (hp, true);
        }
        let written = if p.capped { (expected - took).max(0) } else { 0 };
        let back = give_back(took + written, expected);
        let taken = took + written - back;
        (taken, taken >= hp)
    }

    #[test]
    fn the_breaker_on_margit_no_longer_kills_him() {
        // run 2: 528 KH x 8 = 4224 asked, x0.65 learnt -> 2749 expected of 3456; the game passed x0.86 (his defence ~ none)
        let (wanted, hp) = (4224, 3456);
        let expected = (wanted as f32 * 0.6508).round() as i32;
        assert_eq!(hp - expected, 707);
        let p = plan(wanted, expected, hp, 0, 0, ceiling(0.86));
        assert!(p.capped && p.ask > 0 && p.ask as f32 * ceiling(0.86) <= (hp - 1) as f32, "{p:?}");
        let (taken, dead) = land(wanted, expected, hp, 100.0, 0.045, 1.0);
        assert!(!dead && taken == expected, "taken {taken} of {hp}, expected {expected}");
        // the same with a counter hit and a weakness on top (x1.5 of the formula)
        let (taken, dead) = land(wanted, expected, hp, 100.0, -0.1, 1.5);
        assert!(!dead && taken <= expected + tolerance(expected), "taken {taken}");
    }

    #[test]
    fn a_kill_the_kh_side_counts_on_goes_out_whole() {
        // a Breaker's defeat (all its HP) or a hit that ends it: the game's own death and rewards
        assert_eq!(plan(4224, 3456, 3456, 0, 0, CEILING), Plan { ask: 4224, capped: false });
        assert_eq!(plan(424, 230, 218, 0, 0, CEILING), Plan { ask: 424, capped: false });
        // ... also when a bullet in flight is expected to leave it less than this one takes
        assert_eq!(plan(300, 150, 400, 300, 600, CEILING), Plan { ask: 300, capped: false });
    }

    #[test]
    fn a_small_hit_on_a_big_body_is_never_touched() {
        // 216 asked of a boss with 3642: at the worst 432, far from a kill
        assert_eq!(plan(216, 186, 3642, 0, 0, CEILING), Plan { ask: 216, capped: false });
        let (taken, dead) = land(216, 186, 3642, 30.0, 0.0, 1.0);
        assert!(!dead && taken <= 186 + tolerance(186), "{taken}");
    }

    #[test]
    fn a_soldier_that_should_live_lives_whatever_the_game_passes() {
        // a Godrick soldier (219 HP, x0.52 learnt): 416 asked, 216 expected, 3 left
        for (defence, absorption, bonus) in [(80.0, 0.1, 1.0), (40.0, -0.2, 1.3), (200.0, 0.3, 1.0), (1.0, -0.3, 1.2)] {
            let (taken, dead) = land(416, 216, 219, defence, absorption, bonus);
            assert!(!dead, "defence {defence} absorption {absorption} bonus {bonus}: took {taken}");
            assert!(taken <= 216 + tolerance(216), "took {taken}");
        }
    }

    #[test]
    fn the_bullets_in_flight_count_at_their_worst() {
        // 1000 HP, a capped bullet of 400 in flight (worst 800): a second hit of 300 expected 150 has 199 HP of room
        let p = plan(300, 150, 1000, 300, 800, CEILING);
        assert!(p.capped && p.ask == 99, "{p:?}");
        // no room left at all: no bullet, the expected written directly
        assert_eq!(plan(300, 150, 1000, 300, 999, CEILING), Plan { ask: 0, capped: true });
    }

    #[test]
    fn the_worst_case_follows_what_a_body_was_seen_to_take() {
        assert_eq!(ceiling(0.0), CEILING);
        assert_eq!(ceiling(0.86), CEILING);
        assert!((ceiling(1.8) - 2.25).abs() < 1e-6);
        assert_eq!(ceiling(f32::NAN), CEILING);
        assert_eq!(worst_of(100, 0, 2.0), 200);
        assert_eq!(worst_of(100, 500, 2.0), 500);
    }

    #[test]
    fn what_is_given_back() {
        assert_eq!(give_back(3456, 2749), 707);
        assert_eq!(give_back(2760, 2749), 0, "within the tolerance");
        assert_eq!(give_back(100, 0), 0, "no ceiling (a kill / friendly / a guard)");
        assert_eq!(give_back(4, 2), 0);
        assert_eq!(give_back(5, 2), 3);
        assert_eq!(tolerance(2749), 137);
    }
}
