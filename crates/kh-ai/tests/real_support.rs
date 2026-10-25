//! Recovery and combat styles on the real trees.

mod common;
use common::*;
use glam::Vec3;
use kh_ai::*;

#[test]
fn donald_cures_sora_below_threshold() {
    let mut p = Party::new();
    p.world.add_dummy(7, Vec3::new(700.0, 0.0, 0.0));
    p.set_battle(true);
    p.run(2.0);
    // Sora at 20 % (Use As Needed = PINCH)
    p.world.player.hp = 20;
    p.run(6.0);
    let cures: Vec<_> = p.pawns[0].log.iter().filter(|c| matches!(c, AiCommand::Cure { .. })).cloned().collect();
    println!("donald: {}\n cures {cures:?}\n record {:?}", p.path(0), p.shared.cure_info);
    for l in p.ais[0].trace().log.iter().take(40) {
        println!("   {l}");
    }
    assert!(cures.iter().any(|c| *c == AiCommand::Cure { target: ActorRef::Player }), "Donald did not cast Cure on Sora");
}

#[test]
fn no_cure_above_threshold() {
    let mut p = Party::new();
    p.world.add_dummy(7, Vec3::new(700.0, 0.0, 0.0));
    p.set_battle(true);
    p.world.player.hp = 60;
    p.run(6.0);
    assert!(!p.pawns[0].log.iter().any(|c| matches!(c, AiCommand::Cure { .. })));
}

#[test]
fn goofy_drinks_potion_when_low() {
    let mut p = Party::new();
    // Donald does not heal anyone in this test (his Recovery targets are all off)
    let mut s = p.ais[0].settings();
    s.cure_targets = 0;
    p.ais[0].set_settings(s);
    p.world.add_dummy(7, Vec3::new(700.0, 0.0, 0.0));
    p.set_battle(true);
    p.run(1.0);
    p.pawns[1].view.hp = 30;
    p.run(8.0);
    let items: Vec<_> = p.pawns[1].log.iter().filter(|c| matches!(c, AiCommand::UseItem { .. })).cloned().collect();
    println!("goofy: {}\n items {items:?}\n record {:?}", p.path(1), p.shared.cure_info);
    for l in p.ais[1].trace().log.iter().take(20) {
        println!("   {l}");
    }
    assert!(items.iter().any(|c| *c == AiCommand::UseItem { item: "POTION", target: ActorRef::Member(1) }), "Goofy did not drink a Potion");
}

#[test]
fn style_hands_off_runs_support_subtree() {
    let mut p = Party::new();
    for ai in &mut p.ais {
        let mut s = ai.settings();
        s.battle = BattleStyle::Support;
        ai.set_settings(s);
    }
    p.world.add_dummy(7, Vec3::new(900.0, 0.0, 0.0));
    p.set_battle(true);
    p.run(4.0);
    for i in 0..2 {
        let path = p.path(i);
        println!("member {i} at {:?}: {path}", p.pawns[i].view.position);
        assert!(path.contains("BT_n_Common_BattleStyle_Support"), "member {i} not in the Support style tree");
    }
    // and the default style runs Together
    let mut q = Party::new();
    q.world.add_dummy(7, Vec3::new(900.0, 0.0, 0.0));
    q.set_battle(true);
    q.run(1.0);
    assert!(q.path(0).contains("BT_n_Common_BattleStyle_Together"));
    assert!(q.path(1).contains("BT_n_Common_BattleStyle_Together"));
}

/// The attack-flow wait phase (battle controller tick 0x14123CDA0): with Sora locked on a dummy
/// near the friends the phase runs (CanAttackFlowWait false); it ends when Sora's target is more
/// than 1500 cm from him, and after 10 s.
#[test]
fn attack_flow_phase() {
    let mut p = Party::new();
    // the BattleFlow trees switch the wait on only against e_ex761 (IsEnemy EX761 ->
    // EnableAttackFlowWait m_bEnable=true; every other target -> off)
    let uid = p.pack.enum_value("ETresEnemyUniqueID", "ETresEnemyUniqueID::TRES_ENEMY_UID_EX761");
    assert!(uid >= 0, "EX761 uid");
    p.world.add_dummy(7, Vec3::new(500.0, 0.0, 0.0));
    p.world.dummies[0].uid = uid as i32;
    p.world.player.lock_on = ActorRef::Enemy(7);
    p.set_battle(true);
    let mut seen = false;
    for _ in 0..240 {
        p.step(1.0 / 60.0);
        let s = p.ais[0].state();
        if s.attack_flow_wait && s.attack_flow_phase {
            seen = true;
            break;
        }
    }
    assert!(seen, "the attack phase never started (wait on: {})", p.ais[0].state().attack_flow_wait);
    // Sora walks 2000 cm away from his target, his friends stay: the phase ends at once
    p.world.set_player(Vec3::new(-1700.0, 0.0, 0.0), 0.0);
    p.step(1.0 / 60.0);
    assert!(!p.ais[0].state().attack_flow_phase || !p.ais[0].state().attack_flow_wait);
}

#[test]
fn stop_ai_parks_the_tree() {
    let mut p = Party::new();
    p.ais[0].set_stop_ai(true);
    p.run(1.0);
    assert!(p.path(0).contains("BTTask_Wait"), "{}", p.path(0));
    p.ais[0].set_stop_ai(false);
    p.run(1.0);
    assert!(!p.path(0).contains("BTComposite_Selector_1 > BTTask_Wait"), "{}", p.path(0));
}
