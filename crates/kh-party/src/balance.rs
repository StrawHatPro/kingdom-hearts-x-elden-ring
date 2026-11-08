//! (GF-STATS) THE FRIENDS' OWN STATS (documentation/GOOFY_OVERHAUL.md 2, DONALD_OVERHAUL.md 2, build/sora_tmp/party/GOOFY/
//! PLAN.md R8 / R9). KH3 has no playable Donald / Goofy: these are the port's rules. Nothing here follows Sora (2026-11-01, the
//! user: "Donald and Goofy need to have independent stats from Sora, those multipliers are just for reference from level 1"),
//! and the numbers are the same leading or not (ONESTAT, 2026-10-27).
//!
//! EVERY NUMBER A SHEET ROW:
//! - Strength / Magic / Defense = his row of `sheets/port/party-balance-levels.json` at his level (built from his own anchors
//!   at levels 1 / 50 / 99 in `party-balance.json` by tools/party_balance_sheet.js), then his weapon and armour.
//! - Max HP / MP = his fixed start (`<chara>.start.hp` / `.mp`) + what the bosses gave him (crate::growth) + the roster's steps.
//! - DAMAGE TAKEN IN ELDEN RING: a hit takes a share of the Tarnished's max HP. A friend, leader or AI, loses that share of a
//!   fixed reference ([`incoming_reference`]): the sheet's curve at his level, times the reference Defense over his own. The
//!   reference never follows his own bar, so more HP and more Defense both make him last longer. Sora's rule is unchanged.

use crate::ids::Chara;
use crate::status::{critical_base, FriendStatus};

include!(concat!(env!("KH_PARTY_GEN_DIR"), "/party_balance.rs"));
include!(concat!(env!("KH_PARTY_GEN_DIR"), "/party_balance_levels.rs"));

/// The sheet's numbers.
pub const BP: PartyBalanceParams = PARTY_BALANCE_PARAMS;

fn row(chara: &str, level: u32) -> &'static BalanceLevelRow {
    let lv = level.clamp(1, 99);
    PARTY_BALANCE_LEVELS.iter().find(|r| r.chara == chara && r.level == lv).expect("party-balance-levels row")
}

/// The table row of a character at a level (1..=99, clamped).
pub fn level_row(chara: Chara, level: u32) -> &'static BalanceLevelRow {
    row(chara.model(), level)
}

/// His (Strength, Magic, Defense) before weapon and armour.
pub fn base_stats(chara: Chara, level: u32) -> (i32, i32, i32) {
    let r = level_row(chara, level);
    (r.strength, r.magic, r.defense)
}

/// His max HP / MP before any reward.
pub fn start_pools(chara: Chara) -> (i32, i32) {
    match chara {
        Chara::Donald => (BP.n_ex001_start_hp as i32, BP.n_ex001_start_mp as i32),
        Chara::Goofy => (BP.n_ex002_start_hp as i32, BP.n_ex002_start_mp as i32),
    }
}

/// The damage-taken curve at a level: (reference HP, reference Defense). One curve for every friend.
pub fn reference(level: u32) -> (i32, i32) {
    let r = row("reference", level);
    (r.hp, r.defense)
}

/// The Defense factor of an Elden Ring hit on him: (the reference Defense at his level / his status-screen Defense) ^
/// exponent, clamped. Armour and the roster's boosts count: what the status screen shows is what protects him.
pub fn defense_cut(s: &FriendStatus) -> f32 {
    if BP.incoming_defense_exponent == 0.0 {
        return 1.0;
    }
    let lo = BP.incoming_defense_cut_min.clamp(0.01, 1.0);
    (reference(s.level).1.max(1) as f32 / s.defense().max(1) as f32).powf(BP.incoming_defense_exponent).clamp(lo, 1.0 / lo)
}

/// THE HP AN ELDEN RING HIT'S SHARE IS TAKEN OF for this friend, leader or AI (kh-er sora.rs `damage_in`, party/
/// allybody_live.rs take `share x this`). Critical halves it as it halves his start, so the balance holds on every difficulty.
/// With `incoming.on` false: his own max HP.
pub fn incoming_reference(s: &FriendStatus) -> f32 {
    if !BP.incoming_on {
        return s.max_hp() as f32;
    }
    critical_base(reference(s.level).0, s.game_level) as f32 * defense_cut(s)
}

/// How many hits of Elden Ring's share `share` (0..1 of the Tarnished's max) knock him out (reports and tests).
pub fn hits_to_ko(s: &FriendStatus, share: f32) -> f32 {
    s.max_hp() as f32 / (share * incoming_reference(s)).max(1e-6)
}

impl crate::party::Party {
    /// The HP an Elden Ring hit's share is taken of for member `id` ([`incoming_reference`]).
    pub fn incoming_reference(&self, id: crate::MemberId) -> Option<f32> {
        self.status(id).map(incoming_reference)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::growth::{Growth, GrowthStep, RankLine, SlotKind};

    fn at(chara: Chara, level: u32) -> FriendStatus {
        FriendStatus::new(chara, crate::tables::level_row(chara, level).0)
    }

    /// The status-screen values of the docs, with the starting weapon.
    #[test]
    fn the_curves_at_levels_1_50_and_99() {
        for (chara, want) in [(Chara::Donald, [(5, 18, 6), (12, 52, 25), (19, 86, 44)]), (Chara::Goofy, [(11, 0, 10), (28, 0, 48), (44, 0, 86)])] {
            for (level, (st, mag, def)) in [1, 50, 99].into_iter().zip(want) {
                let s = at(chara, level);
                assert_eq!(s.level, level);
                assert_eq!((s.strength(), s.magic(), s.defense()), (st, mag, def), "{chara:?} level {level}");
            }
        }
    }

    #[test]
    fn each_stat_grows_steadily() {
        for chara in [Chara::Donald, Chara::Goofy] {
            for lv in 2..=99 {
                let (a, b) = (base_stats(chara, lv - 1), base_stats(chara, lv));
                for (x, y) in [(a.0, b.0), (a.1, b.1), (a.2, b.2)] {
                    assert!(y >= x && y - x <= 2, "{chara:?} level {lv}: {x} -> {y}");
                }
            }
        }
    }

    #[test]
    fn the_starts_and_the_slots_are_fixed() {
        let (d, g) = (FriendStatus::new(Chara::Donald, 0), FriendStatus::new(Chara::Goofy, 0));
        assert_eq!((d.max_hp(), d.max_mp(), d.hp, d.mp), (89, 138, 89, 138));
        assert_eq!((g.max_hp(), g.max_mp(), g.hp, g.mp), (142, 88, 142, 88));
        assert_eq!((d.item_slots, d.armour_slots, d.accessory_slots), (2, 1, 2));
        assert_eq!((g.item_slots, g.armour_slots, g.accessory_slots), (3, 2, 1));
    }

    /// (ONESTAT) One set of stats, leading or not: the pad changes nothing.
    #[test]
    fn a_friend_has_the_same_status_leading_or_not() {
        for chara in [Chara::Donald, Chara::Goofy] {
            let mut party = crate::Party::new(crate::PartyConfig { level99: false, total_exp: crate::tables::level_row(chara, 40).0, seed: 1 });
            let id = party.spawn(chara, kh_core::Vec3::ZERO, 0.0);
            let ai = party.status(id).unwrap().clone();
            party.set_pad(id, Some(kh_core::Tuning::USER));
            let led = party.status(id).unwrap();
            assert_eq!((ai.strength(), ai.magic(), ai.defense(), ai.max_hp(), ai.max_mp()), (led.strength(), led.magic(), led.defense(), led.max_hp(), led.max_mp()), "{chara:?}");
            assert_eq!(party.incoming_reference(id), Some(incoming_reference(&ai)), "{chara:?}: the same damage rule");
        }
    }

    /// R8: Goofy takes clearly less than Donald at the same level, with their own HP, and the reference is the same curve.
    #[test]
    fn goofy_takes_clearly_less_than_donald_at_every_level() {
        for lv in [1, 10, 20, 40, 50, 60, 80, 99] {
            let (g, d) = (at(Chara::Goofy, lv), at(Chara::Donald, lv));
            assert!(incoming_reference(&g) < incoming_reference(&d) * 0.7, "level {lv}: a hit on Goofy {} vs Donald {}", incoming_reference(&g), incoming_reference(&d));
            let (gh, dh) = (hits_to_ko(&g, 0.1), hits_to_ko(&d, 0.1));
            assert!(gh > dh * 2.0, "level {lv}: Goofy {gh:.2} hits, Donald {dh:.2}");
            // with every reward as well
            let (mut gf, mut df) = (g.clone(), d.clone());
            gf.set_growth(Growth::full(Chara::Goofy));
            df.set_growth(Growth::full(Chara::Donald));
            assert!(hits_to_ko(&gf, 0.1) > hits_to_ko(&df, 0.1) * 2.0, "level {lv} with every reward");
        }
    }

    /// C7: more HP and more Defense really make him tougher, and neither moves the other's reference.
    #[test]
    fn hp_rewards_and_defense_make_him_tougher() {
        let g = at(Chara::Goofy, 30);
        let base = hits_to_ko(&g, 0.1);
        let mut more_hp = g.clone();
        let mut record = Growth::default();
        for _ in 0..5 {
            record.apply(Chara::Goofy, GrowthStep::MaxHp(14));
        }
        more_hp.set_growth(record);
        assert_eq!(incoming_reference(&more_hp), incoming_reference(&g), "the reference does not follow his bar");
        let expected = base * (more_hp.max_hp() as f32 / g.max_hp() as f32);
        assert!((hits_to_ko(&more_hp, 0.1) - expected).abs() < 1e-3);
        assert!(hits_to_ko(&more_hp, 0.1) > base * 1.3, "{} of {base}", hits_to_ko(&more_hp, 0.1));
        // the status screen's Defense counts: roster boosts (and armour) cut the hit
        let mut harder = g.clone();
        harder.roster.defense = 10;
        assert!(incoming_reference(&harder) < incoming_reference(&g));
    }

    #[test]
    fn the_defense_factor_is_kept_in_its_bounds() {
        let mut g = at(Chara::Goofy, 1);
        g.roster.defense = 200;
        assert!((defense_cut(&g) - BP.incoming_defense_cut_min).abs() < 1e-6, "{}", defense_cut(&g));
    }

    /// The reference is the sheet's own curve: no status of anyone else is read.
    #[test]
    fn the_reference_curve_is_the_sheets() {
        assert_eq!([reference(1), reference(50), reference(99)], [(105, 8), (185, 34), (265, 59)]);
        for lv in 2..=99 {
            assert!(reference(lv).0 > reference(lv - 1).0, "level {lv}");
        }
    }

    #[test]
    fn reward_steps_of_each_kind_reach_the_status() {
        let mut g = at(Chara::Goofy, 1);
        let mut record = Growth::default();
        record.apply(Chara::Goofy, GrowthStep::MaxHp(14));
        record.apply(Chara::Goofy, GrowthStep::MaxMp(4));
        record.apply(Chara::Goofy, GrowthStep::Slot(SlotKind::Accessory));
        record.apply(Chara::Goofy, GrowthStep::Rank(RankLine::Taunt));
        g.set_growth(record);
        assert_eq!((g.max_hp(), g.max_mp(), g.accessory_slots, g.growth.ranks.rank(RankLine::Taunt)), (156, 92, 2, 2));
        assert_eq!((g.hp, g.mp), (156, 92), "a full bar stays full");
    }
}
