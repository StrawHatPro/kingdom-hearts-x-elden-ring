//! The friends' real trees (build/sora_pack/party/ai) on a kinematic host.

mod common;
use common::*;
use glam::Vec3;
use kh_ai::bt::{AuxKind, Library, NodeKind};
use kh_ai::*;

/// Reachability: every node instance in every tree reachable from n_ex001_BT / n_ex002_BT (with
/// their dynamic injections) maps to an implementation. Prints the census.
#[test]
fn reachability_zero_unknown() {
    let pack = pack();
    let lib = Library::compile(&pack);
    let mut todo: Vec<String> = Vec::new();
    for pawn in ["n_ex001", "n_ex002"] {
        let r = &pack.roots[pawn];
        todo.push(r.tree.clone());
        todo.extend(r.dynamic.iter().map(|(_, t)| t.clone()));
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut missing = std::collections::BTreeMap::<String, u32>::new();
    let mut instances = 0u32;
    while let Some(t) = todo.pop() {
        if !seen.insert(t.clone()) {
            continue;
        }
        let Some(tree) = lib.trees.get(&t) else { panic!("tree {t} not packed") };
        for n in &tree.nodes {
            instances += 1;
            if !n.implemented {
                *missing.entry(n.class.clone()).or_default() += 1;
            }
            if let NodeKind::RunBehavior { asset, tag } = &n.kind {
                if let Some(a) = asset {
                    todo.push(a.clone());
                }
                if tag.is_some() {
                    // injected trees are already roots; the default asset is listed above
                }
            }
        }
        for a in &tree.aux {
            instances += 1;
            if !a.implemented {
                *missing.entry(a.class.clone()).or_default() += 1;
            }
            let _ = matches!(a.kind, AuxKind::Service(_));
        }
    }
    println!("reachable trees: {} ({:?})", seen.len(), seen);
    println!("node instances: {instances}");
    assert!(missing.is_empty(), "unimplemented node classes: {missing:?}");
}

#[test]
fn field_follow_home_position() {
    let mut p = Party::new();
    p.world.set_player(Vec3::new(0.0, 0.0, 0.0), 0.0);
    p.run(3.0);
    for i in 0..2 {
        println!("member {i} at {:?}: {}", p.pawns[i].view.position, p.path(i));
        for l in p.ais[i].trace().log.iter().take(12) {
            println!("   {l}");
        }
    }
    // Sora runs away 1200 cm: both follow
    p.world.set_player(Vec3::new(1200.0, 0.0, 0.0), 0.0);
    p.world.player.actor.velocity = Vec3::new(680.0, 0.0, 0.0);
    p.run(4.0);
    p.world.player.actor.velocity = Vec3::ZERO;
    p.run(2.0);
    for i in 0..2 {
        let d = (p.pawns[i].view.position - p.world.player.actor.position).length();
        println!("after run: member {i} at {:?} (dist {d:.0}): {}", p.pawns[i].view.position, p.path(i));
        assert!(d < 700.0, "member {i} did not follow ({d})");
    }
}

/// Relay F 2026-10-15: no enemies and not in battle -> the root's `CanBattle() != true` branch
/// (BT_n_Common_Field > n_Common_FollowPlayer), never BT_n_Common_Battle; and back to the field
/// tree when the battle ends. Both with the body answering `CanBattle` and with kh-ai's fallback.
#[test]
fn no_enemies_runs_field_tree() {
    let has = |p: &Party, i: usize, t: &str| p.ais[i].trace().path.iter().any(|l| l.starts_with(&format!("{t}:")));
    let mut p = Party::new();
    p.run(1.0);
    p.world.set_player(Vec3::new(800.0, 0.0, 0.0), 0.0);
    p.world.player.actor.velocity = Vec3::new(680.0, 0.0, 0.0);
    p.run(3.0);
    for i in 0..2 {
        assert!(!has(&p, i, "BT_n_Common_Battle"), "member {i} in the battle tree: {}", p.path(i));
        assert!(has(&p, i, "BT_n_Common_Field") && has(&p, i, "n_Common_FollowPlayer"), "member {i}: {}", p.path(i));
    }
    // the player in battle but the pawn's own CanBattle false (B answers it): still the field tree
    p.world.player.in_battle = true;
    p.run(1.0);
    for i in 0..2 {
        assert!(!has(&p, i, "BT_n_Common_Battle"), "member {i} followed the player's flag: {}", p.path(i));
    }
    // the pawns' battle on: battle tree; off again: field tree
    p.set_battle(true);
    p.run(1.0);
    for i in 0..2 {
        assert!(has(&p, i, "BT_n_Common_Battle"), "member {i}: {}", p.path(i));
    }
    p.set_battle(false);
    p.run(1.0);
    for i in 0..2 {
        assert!(!has(&p, i, "BT_n_Common_Battle") && has(&p, i, "BT_n_Common_Field"), "member {i} stayed in battle: {}", p.path(i));
    }
}

#[test]
fn field_warp_when_far() {
    let mut p = Party::new();
    p.run(0.5);
    // Sora 6000 cm away, the camera behind him looking his way: the friends are behind the
    // camera (out of the view frustum) and the path is >= 3000 cm: the next 2 s check warps
    p.world.set_player(Vec3::new(6000.0, 0.0, 0.0), 0.0);
    p.world.camera.position = Vec3::new(5600.0, 0.0, 200.0);
    p.run(1.4);
    assert!(!p.pawns.iter().any(|pw| pw.log.iter().any(|c| matches!(c, AiCommand::Warp { .. }))), "warped before the 2 s check");
    p.run(0.2);
    for i in 0..2 {
        let d = (p.pawns[i].view.position - p.world.player.actor.position).length();
        println!("member {i} at {:?} dist {d:.0}", p.pawns[i].view.position);
        assert!(p.pawns[i].log.iter().any(|c| matches!(c, AiCommand::Warp { .. })), "member {i} did not warp");
        assert!(d < 1000.0);
    }
}

/// 0x141258790: no warp while the friend is in the camera's view, nor when the path is short.
#[test]
fn no_warp_in_view_or_near() {
    let mut p = Party::new();
    p.run(0.2);
    // far away but in front of the camera (the camera looks at them from beyond Sora)
    p.world.set_player(Vec3::new(6000.0, 0.0, 0.0), 180.0);
    p.world.camera.position = Vec3::new(6400.0, 0.0, 200.0);
    p.world.camera.yaw = 180.0;
    p.run(2.5);
    assert!(!p.pawns.iter().any(|pw| pw.log.iter().any(|c| matches!(c, AiCommand::Warp { .. }))), "warped while seen");
    assert!(kh_ai::controller::in_view(&p.world.camera, Vec3::new(0.0, 0.0, 70.0), 70.0));
    assert!(!kh_ai::controller::in_view(&p.world.camera, Vec3::new(7000.0, 0.0, 70.0), 70.0));
    // behind the camera but only 2000 cm of path: no warp either
    let mut q = Party::new();
    q.world.set_player(Vec3::new(2000.0, 0.0, 0.0), 0.0);
    q.world.camera.position = Vec3::new(1600.0, 0.0, 200.0);
    q.pawns[0].view.position = Vec3::new(0.0, 0.0, 0.0);
    q.run(0.1);
    assert!(!q.pawns[0].log.iter().any(|c| matches!(c, AiCommand::Warp { .. })));
}

#[test]
fn battle_engage_dummies() {
    let mut p = Party::new();
    p.world.add_dummy(7, Vec3::new(600.0, 0.0, 0.0));
    p.set_battle(true);
    p.run(8.0);
    for i in 0..2 {
        println!("member {i} at {:?}: {}", p.pawns[i].view.position, p.path(i));
        let actions: Vec<String> = p.pawns[i].log.iter().filter_map(|c| if let AiCommand::Action { def, target, .. } = c { Some(format!("{def}->{target:?}")) } else { None }).collect();
        println!("  actions: {actions:?}");
        for l in p.ais[i].trace().log.iter().take(if i == 1 { 32 } else { 4 }) {
            println!("   {l}");
        }
        assert!(!actions.is_empty(), "member {i} never attacked");
    }
}
