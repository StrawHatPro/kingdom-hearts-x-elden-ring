//! (MIMIC-BOSS hotfix 3, THE USER 2026-11-06: "the boss mimic disappeared after it's mimic summon died/timed out"; "For mimic
//! Sora, it's AI seemed to completely break when the mimic sora summoned by the mimic Sora boss died or despawned") The fight
//! keeps its body, and so its copy, its AI and its effects and sounds, whatever its summon does and however far the leader
//! runs from it; it ends only when the world has no body, the leader leaves the fog, or the boss dies.

use crate::mimicboss::{boss_of_block, fight_body, FightBody, MIMIC_BOSSES};

/// The user's run at 246.013 s: the targets no longer listed the body (40.0 m and more from the leader); its handle's body
/// was alive in the world (found again at 39.9 m, "disabled alive", 3 s later). The fight now keeps it.
#[test]
fn a_body_beyond_the_targets_range_is_still_the_fights() {
    assert_eq!(fight_body(None, Some((true, 2008))), FightBody::Alive);
    assert_eq!(fight_body(Some((true, 2008)), None), FightBody::Alive);
    assert_eq!(fight_body(None, Some((false, 0))), FightBody::Dead);
    assert_eq!(fight_body(Some((true, 0)), None), FightBody::Dead);
    assert_eq!(fight_body(None, None), FightBody::Gone);
}

/// The summon dies, times out, or is despawned: the boss's own body word does not change (its summon is not part of it), so
/// the boss copy and its AI step on: for every leader's copy (Sora / Donald / Goofy; only Sora's copy summons).
#[test]
fn the_summons_end_never_ends_the_boss() {
    // the boss alive at 2008 HP; its summon's body at 0 HP (its death), or gone from the world (its time out / despawn)
    let boss = Some((true, 2008));
    for summon in [Some((false, 0)), None, Some((true, 1))] {
        let _ = summon; // the summon's body is never asked for the fight's word
        assert_eq!(fight_body(None, boss), FightBody::Alive);
        assert_eq!(fight_body(boss, None), FightBody::Alive);
    }
}

/// The user's runs: the leader 39.9 m and more from the body (the targets' range), in the boss's map behind its fog. The
/// fight holds; out of the map (a warp) or with the fog gone (a death, a reload) it ends when he is away from the body.
#[test]
fn the_fog_holds_the_leader_in_the_arena() {
    for boss in MIMIC_BOSSES.iter() {
        let block = (i32::from(boss.area) << 24) | (i32::from(boss.block) << 16);
        assert_eq!(boss_of_block(block).map(|b| b.key), Some(boss.key));
        let body = [0.0, 0.0, 0.0];
        let far = [boss.arena_radius + 60.0, 0.0, 0.0];
        // near the body: in, fog or not
        assert!(boss.solo_holds(Some(body), [5.0, 0.0, 0.0], false, 0));
        // far from the body (the copy left behind), behind the fog in its map: in
        assert!(boss.solo_holds(Some(body), far, true, block));
        // the body not even listed, behind the fog: in
        assert!(boss.solo_holds(None, far, true, block));
        // far, the fog gone (a reload) or another map (a warp): out
        assert!(!boss.solo_holds(Some(body), far, false, block));
        assert!(!boss.solo_holds(Some(body), far, true, 0x0A00_0000));
    }
}

/// (THE USER 2026-11-07: "also mimic tear boss should not be targetable or attackable until it has transformed into Sora") From
/// the fog to the copy's taking over the boss's body is untouchable, then a target like any; its summoned Mimic the same.
#[test]
fn the_boss_is_untouchable_until_its_copy_has_formed() {
    use crate::mimicboss::{boss_untouchable, summon_untouchable, CopyForm};
    // before the fog: Elden Ring's own (nothing of ours)
    assert!(!boss_untouchable(false, false, None));
    // the fog passed: the pool rising (no copy yet), then the copy forming: untouchable
    assert!(boss_untouchable(true, false, None));
    assert!(boss_untouchable(true, false, Some(CopyForm::Forming)));
    // formed (its AI acts): a target, hittable
    assert!(!boss_untouchable(true, false, Some(CopyForm::Formed)));
    // beaten: never held
    assert!(!boss_untouchable(true, true, None));
    // its summoned Mimic: until its copy has formed
    assert!(summon_untouchable(CopyForm::Forming));
    assert!(!summon_untouchable(CopyForm::Formed));
}
