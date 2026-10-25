//! The combo decision: `TresPlayerState_Attack::Decide` 0x14134ACA0 (docs/sora-combat-spec.md
//! 4.2; the ordered rules of sheet `sora-combat-rules`, each branch below names its rule).
//! A pure function of its inputs, so that the expected kind sequences of spec section 15 are
//! tested without a world.

use super::tables::P;
use crate::enums::ETresAbilityKind as A;
use crate::status::AbilitySet;

/// The abilities the decision asks for (`has(id)` = pawn slot 668, `AbilityCount(id)` = slot 672).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ComboAbilities {
    pub tripple_slash: bool,
    pub charge_thrust: bool,
    pub magicflush: bool,
    pub final_heaven: bool,
    pub air_down: bool,
    pub quick_shaft: bool,
    pub flash_step: bool,
    pub radial_cut: bool,
    pub aerial_sweep: bool,
    pub aerial_dive: bool,
    pub slash_upper: i32,
    pub air_roll_beat: i32,
    pub combo_plus: i32,
    pub air_combo_plus: i32,
}

impl ComboAbilities {
    /// No ability: the four-step base combo.
    pub const NONE: ComboAbilities = ComboAbilities { tripple_slash: false, charge_thrust: false, magicflush: false, final_heaven: false, air_down: false, quick_shaft: false, flash_step: false, radial_cut: false, aerial_sweep: false, aerial_dive: false, slash_upper: 0, air_roll_beat: 0, combo_plus: 0, air_combo_plus: 0 };
    /// Every combo ability, no Combo Plus (the right column of spec section 15).
    pub const ALL: ComboAbilities = ComboAbilities { tripple_slash: true, charge_thrust: true, magicflush: true, final_heaven: true, air_down: true, quick_shaft: true, flash_step: true, radial_cut: true, aerial_sweep: true, aerial_dive: true, slash_upper: 1, air_roll_beat: 1, combo_plus: 0, air_combo_plus: 0 };

    /// From the status container's equipped abilities.
    pub fn of(set: &AbilitySet) -> ComboAbilities {
        ComboAbilities {
            tripple_slash: set.has(A::TrippleSlash),
            charge_thrust: set.has(A::ChargeThrust),
            magicflush: set.has(A::Magicflush),
            final_heaven: set.has(A::FinalHeaven),
            air_down: set.has(A::AirDown),
            quick_shaft: set.has(A::QuickShaft),
            flash_step: set.has(A::FlashStep),
            radial_cut: set.has(A::RadialCut),
            aerial_sweep: set.has(A::AerialSweep),
            aerial_dive: set.has(A::AerialDive),
            slash_upper: set.count(A::SlashUpper) as i32,
            air_roll_beat: set.count(A::AirRollBeat) as i32,
            combo_plus: set.count(A::ComboPlus) as i32,
            air_combo_plus: set.count(A::AircomboPlus) as i32,
        }
    }

    /// `MaxCombo()` (pawn slot 928 0x141433570), base Sora: 4 + Combo Plus on the ground, + Air
    /// Combo Plus off it (spec 4.1).
    pub fn max_combo(&self, on_ground: bool) -> i32 {
        P.combo_base_max + if on_ground { self.combo_plus } else { self.air_combo_plus }
    }

    /// `Finishers()` (pawn slot 1253 0x141433A60), base Sora (spec 4.1).
    pub fn finishers(&self) -> i32 {
        P.combo_finishers_base + (self.tripple_slash || self.charge_thrust) as i32 + (self.magicflush || self.final_heaven) as i32
    }
}

/// What `PreDecide` / `TargetMetrics` measured and what the state and the pawn carry (spec 4.1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DecideIn {
    /// state+0x128.
    pub combo: i32,
    /// state+0x154 (0 on a fresh state, copied along a chain).
    pub reach: u8,
    /// pawn+0xE10.
    pub last_kind: u8,
    /// state+0x1CC: horizontal distance to the lock-on point - 50, floored at 0 (0 without a target).
    pub dist: f32,
    /// state+0x1D0: lock-on point Z - own Z.
    pub dz: f32,
    /// state+0x1E0: dz > 160.
    pub target_high: bool,
    pub target_on_ground: bool,
    /// state+0x142 after PreDecide.
    pub air: bool,
    /// state+0x1C4: lock-on targets within 400 cm.
    pub num_enemies: i32,
    /// state+0x1EC bit 2.
    pub launch: bool,
    /// state+0x234 bit 21 (Charge Berserk: endless combo).
    pub endless: bool,
    pub max_combo: i32,
    pub finishers: i32,
    /// pawn+0xE14.
    pub finish_count: i32,
    /// pawn+0xE50 / +0xE54 (the launch stocks).
    pub stocks: (i32, i32),
}

/// What `Decide` leaves behind.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decision {
    /// state+0x1B0.
    pub kind: u8,
    pub reach: u8,
    pub air: bool,
    pub combo: i32,
    /// state+0x140.
    pub finish: bool,
    /// state+0x141 and pawn+0xE48 bit 5.
    pub finish_end: bool,
    /// state+0x1DD with its DirZ.
    pub fixed_dir_z: Option<f32>,
    pub finish_count: i32,
    pub stocks: (i32, i32),
    /// pawn+0xE10 was zeroed (combo 0).
    pub last_kind_reset: bool,
}

/// rules 25 - 28: the ability upgrades.
pub fn upgrade(kind: u8, a: &ComboAbilities) -> u8 {
    match kind {
        1 if a.quick_shaft => 9,
        2 if a.flash_step => 0x0A,
        3 if a.radial_cut => 0x0B,
        0x10 if a.quick_shaft => 0x18,
        0x11 if a.flash_step => 0x19,
        0x12 if a.radial_cut => 0x1A,
        8 if a.aerial_sweep => 0x1F,
        0x0F if a.aerial_dive => 0x20,
        other => other,
    }
}

/// `Decide` (0x14134ACA0), spec 4.2. Out of scope and never requested here: the wall-run kinds
/// (rule 4), sneeze (rule 6), friend unison (rule 7), forced kinds (rule 29).
pub fn decide(i: &DecideIn, a: &ComboAbilities) -> Decision {
    decide_class(i, a, false)
}

/// `TresPlayerState_ComboLimit::Decide` (0x141362AB0; spec 10.5): Second Form's decision, a diff
/// of the base one: the normal-step condition is `combo < max - fin` alone (6 - 3), the rising
/// attack does not exclude kind 0x1F and does not need dist <= 400, and the finisher step picks
/// 7A / 7B / 7C by the finish count (+3 airborne; the air-down finisher 0x0E as in the base).
/// `i.max_combo` / `i.finishers` are the form's (6 / 3).
pub fn decide_limit(i: &DecideIn, a: &ComboAbilities) -> Decision {
    decide_class(i, a, true)
}

fn decide_class(i: &DecideIn, a: &ComboAbilities, limit: bool) -> Decision {
    let mut d = Decision { kind: 0, reach: i.reach, air: i.air, combo: i.combo, finish: false, finish_end: false, fixed_dir_z: None, finish_count: i.finish_count, stocks: i.stocks, last_kind_reset: false };
    let mut last = i.last_kind;
    let mut first = false;
    // rule 2: reach at combo start (and after a launch attack).
    let reach_rule = |reach: u8| if i.dist > P.decide_far_dist { 1 } else if i.num_enemies >= P.decide_crowd_count { 2 } else { reach };
    if i.combo == 0 {
        d.reach = reach_rule(d.reach);
        first = true;
        // rule 3
        last = 0;
        d.last_kind_reset = true;
        d.stocks = (a.slash_upper, a.air_roll_beat);
    } else if matches!(last, 0x1C | 0x1D) {
        d.reach = reach_rule(d.reach);
        first = true;
    }
    let reach = d.reach;
    let (max, fin) = (i.max_combo, i.finishers);
    let m3 = i.combo.rem_euclid(3);
    // The finisher slots (rules 21 - 23).
    let a_special = a.tripple_slash || a.charge_thrust;
    let mut slot_b: u8 = if a_special { 4 } else { 0 };
    let mut slot_c: u8 = 0;
    if a.magicflush {
        let f = if a.final_heaven && i.num_enemies < 2 { 0x0C } else { 7 };
        if slot_b == 0 {
            slot_b = f;
        } else {
            slot_c = f;
        }
    } else if a.final_heaven {
        if slot_b == 0 {
            slot_b = 0x0C;
        } else {
            slot_c = 0x0C;
        }
    }
    // rule 5: the launch request.
    if i.launch {
        let kind = if d.stocks.0 != 1 || (d.stocks.1 == 1 && d.air && i.dz < P.decide_launch_air_dz) { 0x1D } else { 0x1C };
        if !d.air && kind == 0x1C {
            d.fixed_dir_z = Some(1000.0);
        }
        // "used stock = 2"
        if kind == 0x1C {
            d.stocks.0 = 2;
        } else {
            d.stocks.1 = 2;
        }
        d.combo = 0;
        d.kind = kind;
        return d;
    }
    let air_down = |last: u8| a.air_down && i.dz <= P.decide_air_down_dz && i.target_on_ground && !matches!(last, 0x0D | 0x0E) && i.dist <= P.decide_far_dist;
    let mut kind: u8;
    if i.combo < max - fin || (i.endless && !limit) {
        // NORMAL STEP
        let rise = if limit { i.target_high && !matches!(last, 8 | 0x17) } else { i.target_high && !matches!(last, 8 | 0x17 | 0x1F) && i.dist <= P.decide_far_dist };
        if rise {
            // rule 8
            kind = 8;
            d.fixed_dir_z = Some(P.decide_rise_speed);
        } else if d.air && i.dz < P.decide_air_below_dz {
            // rule 9
            kind = if air_down(last) { 0x0D } else { 0x0F };
        } else {
            kind = match last {
                1 | 9 | 0x18 => if reach == 1 { 3 } else { 2 },          // rule 10
                2 | 0x0A | 0x11 | 0x19 => if reach == 1 { 1 } else { 3 }, // rule 11
                3 | 0x0B | 0x12 | 0x1A => if reach == 1 { 1 } else { 2 }, // rule 12
                8 | 0x1F => 1,                                            // rule 13
                0x0F | 0x10 | 0x20 => 8,                                  // rule 14
                0x17 => 3,                                                // rule 15
                _ if first => match reach {                               // rule 16
                    1 => 2,
                    2 => 3,
                    _ => 1,
                },
                _ => match m3 {                                           // rule 17
                    0 => reach + 1,
                    1 => if reach != 0 { 1 } else { 2 },
                    _ => if reach == 2 { 2 } else { 3 },
                },
            };
            // rule 18
            if d.air || (i.combo > 0 && i.target_high) {
                kind += 0x0F;
            }
        }
        d.finish_count = 0;
    } else {
        // FINISHER STEP (rule 19)
        let n = i.finish_count;
        d.finish = true;
        if n >= fin - 1 || i.combo >= max - 1 {
            d.finish_end = true;
        }
        if d.air && i.dz < P.decide_air_below_dz && air_down(last) {
            // rule 20
            d.finish_count = n + 1;
            d.kind = 0x0E;
            return d;
        }
        if limit {
            // 0x141362FD5: 7A (Explosion), 7B (Stride Break), 7C (Ripple Drive); +3 in the air
            // (Hurricane Period, Aerial Finish, Magnet Splash): `bAir` alone lifts a finisher. No upgrade.
            d.kind = match n {
                1 => 0x7B,
                2 => 0x7C,
                _ => 0x7A,
            } + if d.air { 3 } else { 0 };
            d.finish_count = n + 1;
            return d;
        }
        kind = match n {
            // rule 21
            0 => {
                if !a_special {
                    4
                } else if !a.charge_thrust {
                    5
                } else if !a.tripple_slash || i.dist > P.decide_far_dist || i.num_enemies == 1 {
                    6
                } else {
                    5
                }
            }
            1 => slot_b, // rule 22
            2 => slot_c, // rule 23
            _ => 1,
        };
        // rule 24
        if d.air || i.target_high {
            kind += 0x0F;
        }
        d.finish_count = n + 1;
    }
    d.kind = upgrade(kind, a);
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::tables::COMBO_RULES;

    /// A chain of decisions against a fixed situation: each step's kind becomes the next one's
    /// previous kind, as the pawn carries it (pawn+0xE10).
    fn chain(a: &ComboAbilities, steps: usize, situation: impl Fn(usize, u8, &Decision) -> (f32, f32, bool, bool, i32)) -> Vec<u8> {
        let mut out = Vec::new();
        let (mut last, mut reach, mut count, mut stocks) = (0u8, 0u8, 0, (0, 0));
        let mut previous = Decision { kind: 0, reach: 0, air: false, combo: 0, finish: false, finish_end: false, fixed_dir_z: None, finish_count: 0, stocks: (0, 0), last_kind_reset: false };
        for combo in 0..steps {
            let (dist, dz, air, target_on_ground, num_enemies) = situation(combo, last, &previous);
            let input = DecideIn { combo: combo as i32, reach, last_kind: last, dist, dz, target_high: dz > P.decide_rise_dz, target_on_ground, air, num_enemies, launch: false, endless: false, max_combo: a.max_combo(!air), finishers: a.finishers(), finish_count: count, stocks };
            let d = decide(&input, a);
            out.push(d.kind);
            (last, reach, count, stocks) = (d.kind, d.reach, d.finish_count, d.stocks);
            previous = d;
            if d.finish_end {
                break;
            }
        }
        out
    }

    // docs/sora-combat-spec.md section 15, the table of expected attack kinds. "near" = 200 cm
    // centre distance (dist 150), "far" = 600 (dist 550); the dummy's lock-on point is 90 above
    // its feet.
    #[test]
    fn spec_15_ground_one_dummy_near() {
        let near = |_: usize, _: u8, _: &Decision| (150.0, 90.0, false, true, 1);
        assert_eq!(chain(&ComboAbilities::NONE, 8, near), [1, 2, 3, 4]);
        assert_eq!(chain(&ComboAbilities::ALL, 8, near), [9, 6, 4, 0x0C]);
    }

    #[test]
    fn spec_15_ground_one_dummy_far() {
        // dist > 400 only matters at the first press: reach is carried through the chain.
        let far = |combo: usize, _: u8, _: &Decision| (if combo == 0 { 550.0 } else { 150.0 }, 90.0, false, true, 1);
        assert_eq!(chain(&ComboAbilities::NONE, 8, far), [2, 1, 3, 4]);
        assert_eq!(chain(&ComboAbilities::ALL, 8, far), [0x0A, 6, 4, 0x0C]);
    }

    #[test]
    fn spec_15_ground_three_dummies_within_400() {
        let crowd = |_: usize, _: u8, _: &Decision| (150.0, 90.0, false, true, 3);
        assert_eq!(chain(&ComboAbilities::NONE, 8, crowd), [3, 2, 3, 4]);
        assert_eq!(chain(&ComboAbilities::ALL, 8, crowd), [0x0B, 5, 4, 7]);
    }

    #[test]
    fn spec_15_dummy_above() {
        // Sora on the ground, the dummy 250 above: the rise attack takes him into the air; from
        // then on he is airborne and level with it (dz within +-100).
        let above = |combo: usize, _: u8, _: &Decision| if combo == 0 { (150.0, 250.0, false, false, 1) } else { (150.0, 20.0, true, false, 1) };
        assert_eq!(chain(&ComboAbilities::NONE, 8, above), [8, 0x10, 0x17, 0x13]);
        assert_eq!(chain(&ComboAbilities::ALL, 8, above), [0x1F, 0x15, 0x13, 0x1B]);
    }

    #[test]
    fn spec_15_airborne_dummy_at_his_height() {
        let level = |_: usize, _: u8, _: &Decision| (150.0, 0.0, true, false, 1);
        assert_eq!(chain(&ComboAbilities::NONE, 8, level), [0x10, 0x17, 0x12, 0x13]);
        assert_eq!(chain(&ComboAbilities::ALL, 8, level), [0x18, 0x15, 0x13, 0x1B]);
    }

    #[test]
    fn spec_15_airborne_grounded_dummy_below() {
        // 100 - 250 below: the falling attack on every normal step while dz < -100, finisher 0x13.
        let below = |_: usize, _: u8, _: &Decision| (150.0, -200.0, true, true, 1);
        assert_eq!(chain(&ComboAbilities::NONE, 8, below), [0x0F, 0x0F, 0x0F, 0x13]);
        // All abilities: aerial dive once (it takes him down to the dummy: from then on he is
        // airborne and level with it), then the three finishers.
        let dived = |depth: f32| move |combo: usize, _: u8, _: &Decision| if combo == 0 { (150.0, depth, true, true, 1) } else { (150.0, 20.0, true, true, 1) };
        assert_eq!(chain(&ComboAbilities::ALL, 8, dived(-200.0)), [0x20, 0x15, 0x13, 0x1B]);
        // 250 or more below with AIR_DOWN: 0x0D first; 0x0E is refused right after 0x0D.
        let deep = |_: usize, _: u8, _: &Decision| (150.0, -300.0, true, true, 1);
        assert_eq!(chain(&ComboAbilities::NONE, 8, deep), [0x0F, 0x0F, 0x0F, 0x13]);
        assert_eq!(chain(&ComboAbilities::ALL, 8, dived(-300.0)), [0x0D, 0x15, 0x13, 0x1B]);
        // (Kept 300 above a grounded dummy the whole time, the literal procedure gives the finisher
        // air-down 0x0E at the second finisher, where the previous kind is 0x15: rule 20.)
        assert_eq!(chain(&ComboAbilities::ALL, 8, deep), [0x0D, 0x15, 0x0E, 0x1B]);
    }

    #[test]
    fn spec_15_combo_plus_adds_normal_steps() {
        let near = |_: usize, _: u8, _: &Decision| (150.0, 90.0, false, true, 1);
        let plus = |n: i32, base: ComboAbilities| ComboAbilities { combo_plus: n, ..base };
        assert_eq!(chain(&plus(1, ComboAbilities::NONE), 9, near), [1, 2, 3, 2, 4]);
        assert_eq!(chain(&plus(2, ComboAbilities::NONE), 9, near), [1, 2, 3, 2, 3, 4]);
        assert_eq!(chain(&plus(2, ComboAbilities::ALL), 9, near), [9, 0x0A, 0x0B, 6, 4, 0x0C]);
        assert_eq!((ComboAbilities::NONE.max_combo(true), ComboAbilities::NONE.finishers()), (4, 1));
        assert_eq!((plus(2, ComboAbilities::ALL).max_combo(true), plus(2, ComboAbilities::ALL).max_combo(false), ComboAbilities::ALL.finishers()), (6, 4, 3));
    }

    #[test]
    fn without_a_target_the_decision_takes_the_near_single_branches() {
        // spec 4.1: dist = dz = 0, targetHigh false, reach 0.
        let none = |_: usize, _: u8, _: &Decision| (0.0, 0.0, false, true, 0);
        assert_eq!(chain(&ComboAbilities::NONE, 8, none), [1, 2, 3, 4]);
    }

    // Spec 10.5 "Consequences" and 10.8: the Second Form chains.
    #[test]
    fn second_form_chains_of_spec_10_5() {
        let limit = |a: &ComboAbilities, air: bool, dz: f32| {
            let mut out = Vec::new();
            let (mut last, mut reach, mut count, mut stocks) = (0u8, 0u8, 0, (0, 0));
            for combo in 0..8 {
                let input = DecideIn { combo, reach, last_kind: last, dist: 150.0, dz, target_high: dz > P.decide_rise_dz, target_on_ground: !air, air, num_enemies: 1, launch: false, endless: true, max_combo: P.form_max_combo, finishers: P.form_finishers, finish_count: count, stocks };
                let d = decide_limit(&input, a);
                out.push((d.kind, d.finish, d.finish_end));
                (last, reach, count, stocks) = (d.kind, d.reach, d.finish_count, d.stocks);
                if d.finish_end {
                    break;
                }
            }
            out
        };
        let kinds = |v: Vec<(u8, bool, bool)>| v.iter().map(|x| x.0).collect::<Vec<_>>();
        // Ground, one near target, no abilities: 1, 2, 3, then 7A, 7B, 7C; finishEnd on the third.
        let ground = limit(&ComboAbilities::NONE, false, 90.0);
        assert_eq!(kinds(ground.clone()), [1, 2, 3, 0x7A, 0x7B, 0x7C]);
        assert_eq!(ground.iter().map(|x| (x.1, x.2)).collect::<Vec<_>>(), [(false, false), (false, false), (false, false), (true, false), (true, false), (true, true)]);
        // In the air, the dummy level: 10, 17, 12, then 7D, 7E, 7F.
        assert_eq!(kinds(limit(&ComboAbilities::NONE, true, 0.0)), [0x10, 0x17, 0x12, 0x7D, 0x7E, 0x7F]);
        // The upgrade abilities change the first three steps only; the endless-combo flag is not tested.
        assert_eq!(kinds(limit(&ComboAbilities::ALL, false, 90.0)), [9, 0x0A, 0x0B, 0x7A, 0x7B, 0x7C]);
        assert_eq!(kinds(limit(&ComboAbilities::ALL, true, 0.0)), [0x18, 0x19, 0x1A, 0x7D, 0x7E, 0x7F]);
    }

    #[test]
    fn the_rule_sheet_is_the_one_this_code_was_written_from() {
        // 29 ordered rules; the branches above cite 2, 3, 5, 8 - 28 (4, 6, 7, 29 are out of scope).
        assert_eq!(COMBO_RULES.len(), 29);
        assert!(COMBO_RULES.iter().enumerate().all(|(k, rule)| rule.order == k as i32 + 1));
        assert_eq!(COMBO_RULES[7].kind, "8");
        assert_eq!(COMBO_RULES[19].kind, "0x0E");
    }
}
