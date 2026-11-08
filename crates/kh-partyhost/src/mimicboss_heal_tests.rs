//! (MIMIC-BOSS hotfix 3, THE USER 2026-11-07: "anything the mimic boss or summon does that would heal it should heal it for
//! the same % of HP that Sora would get healed by by using it") A copy's heal is the SAME SHARE of its own pool as the source
//! gives the copied leader of his: the boss's 30 x pool, the boss's summoned Mimic (the boss's pool), the player's Mimic Sora
//! (his own KH3 HP, the leader's: the share is his core's by itself).

use crate::mimicboss::{boss_hp, heal_share, healed_er_hp, mirrored_hp, BOSS_HP_X_LEADER};

/// KH3's heals as the core gives them to a level-99 Sora (kh-core tests magic.rs `cure`: Cure +147, Cura +187, Curaga +227)
/// and amounts at level 50 (KH3 max 115): every source is measured the same way (the core's own HP gain in its step), so the
/// table only spans sizes, from a 1 HP drain to a Curaga.
const SOURCES: [(&str, i32, i32); 7] = [
    ("Cure +147 on a 446 maximum", 147, 446),
    ("Cura +187 on a 446 maximum", 187, 446),
    ("Curaga +227 on a 446 maximum", 227, 446),
    ("an item heal of 46 at level 50", 46, 115),
    ("an item heal of 69 at level 50", 69, 115),
    ("a self heal of 50 at level 50 (a form, a finisher, an ability)", 50, 115),
    ("a 1 HP drain", 1, 115),
];

/// The pools: the boss (30 x the leader's KH3 maximum), its summoned Mimic (the boss's), an ER body of a later journey (x 1.2).
fn pools(kh_max: i32) -> [i32; 3] {
    let boss = boss_hp(kh_max, BOSS_HP_X_LEADER);
    [boss, boss, (boss as f32 * 1.2) as i32]
}

#[test]
fn heal_every_source_gives_a_copy_the_leaders_share_of_its_own_pool() {
    for (name, heal, kh_max) in SOURCES {
        for er_max in pools(kh_max) {
            // the copy at 20 % of its body's HP: its core mirrors it, heals itself by the source's amount in its step
            let er_hp = er_max / 5;
            let before = mirrored_hp(er_hp, er_max, kh_max);
            let after = (before + heal).min(kh_max);
            let share = heal_share(before, after, kh_max);
            let healed = healed_er_hp(share, er_hp, er_max) - er_hp;
            let kh_share = (after - before) as f32 / kh_max as f32;
            let er_share = healed as f32 / er_max as f32;
            assert!((er_share - kh_share).abs() <= 0.5 / er_max as f32 + 1e-6, "{name} on a {er_max} HP pool: {er_share:.4} of it, the leader's {kh_share:.4}");
            assert!(healed >= 1, "{name}: a heal is never nothing");
            // never tiny in absolute terms: 30 x the KH3 heal (the pool is 30 x the leader's)
            assert!(healed + 1 >= (after - before) * BOSS_HP_X_LEADER as i32, "{name}: {healed} on a {er_max} pool");
        }
    }
}

#[test]
fn heal_is_capped_at_the_maximum_and_never_raises_the_dead() {
    let (kh_max, er_max) = (115, 3450);
    // at 95 %: a Curaga-sized share fills it, never past
    assert_eq!(healed_er_hp(0.5, er_max * 95 / 100, er_max), er_max);
    assert_eq!(healed_er_hp(0.5, 0, er_max), 0, "a dead body is not healed");
    // no gain, a loss, or a dead core: no heal
    assert_eq!(heal_share(60, 60, kh_max), 0.0);
    assert_eq!(heal_share(60, 40, kh_max), 0.0);
    assert_eq!(heal_share(0, 50, kh_max), 0.0);
    assert_eq!(healed_er_hp(0.0, 1000, er_max), 1000);
}

/// The player's Mimic Sora uses his own KH3 HP (the leader's maximum): his core's heal IS the leader's share (no ER pool
/// in between); the boss copies' pools are 30 x as big, the same share.
#[test]
fn heal_the_players_mimic_and_the_boss_get_the_same_share() {
    let kh_max = 115;
    let heal = 46; // an item heal at level 50
    let mimic_share = heal as f32 / kh_max as f32;
    let er_max = boss_hp(kh_max, BOSS_HP_X_LEADER);
    let boss_gain = healed_er_hp(heal_share(20, 20 + heal, kh_max), 1000, er_max) - 1000;
    assert!((boss_gain as f32 / er_max as f32 - mimic_share).abs() < 1e-3, "boss {boss_gain} of {er_max} vs the Mimic's {mimic_share:.3}");
}
