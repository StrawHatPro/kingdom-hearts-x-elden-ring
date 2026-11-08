//! (SHOT-A) Snakebite (Happy Gear, `ETresShootFlowKind` 12 MI_SHOT): KH3 runs it with the Kingdom Key's King of Hearts
//! update 0x1413B8C90 (Begin 0x1413A5CD0 case 0xC: delegate vt+0x408, FName "MI_SHOT", effect group 0x7A3), so the
//! sequence is King of Hearts' (M_1473 -> volley set-up -> M_1474 -> ShootFlowEnd, lifted 500 cm, 7 per tick) with
//! Happy Gear's projectile (w_so060_ProjSet MI_SHOT = TresProjMI_Shot_C, row SF_MISHOT).
//! Trace: lock-on -> fire -> finish -> back to normal.

mod shotlocks_common;

use kh_core::{AnimId, StateId, TargetId};
use shotlocks_common::{dummy, level99_with, seconds, Bench};

#[test]
fn snakebite_lock_on_fire_finish_back_to_normal() {
    let mut b = Bench::new(level99_with("WEP_KEYBLADE_SO_05"), &[dummy(1, 800.0, 0.0), dummy(2, 800.0, 150.0)]);
    // Lock-on: Happy Gear's base kind is 12 (SelectKind: weapon type 6).
    let kind = b.aim_until(8, 300);
    assert_eq!(kind, 12, "Snakebite while aiming");
    assert_eq!(b.sora.aim().max_locks, 18, "TresShootFlowData row 12 Param0");
    let n = b.sora.aim().total as usize;
    // Fire.
    b.fire();
    let trace = b.trace_shoot_flow(600);
    // The King of Hearts timeline: the charge clip, the volley, the end clip; about 4.0 s; lifted.
    assert!(trace.iter().any(|s| s.clip == AnimId::M1473));
    assert!(trace.iter().any(|s| s.clip == AnimId::M1474));
    assert!(trace.iter().any(|s| s.clip == AnimId::ShootFlowEnd));
    let first = trace.iter().position(|s| s.projectiles > 0).expect("the volley");
    assert!((seconds(first + 1) - 2.5).abs() < 0.1, "set-up at {}", seconds(first + 1));
    assert!((seconds(trace.len()) - 4.0).abs() < 0.15, "ended at {}", seconds(trace.len()));
    assert!(trace.iter().map(|s| s.position.z).fold(0.0, f32::max) > 450.0, "lifted 500 cm");
    // Happy Gear's projectile class, one per lock.
    let classes: Vec<&str> = b.sora.magic().projectile_views().map(|p| p.class).collect();
    assert!(classes.iter().all(|c| *c == "TresProjMI_Shot_C"), "{classes:?}");
    b.run(300);
    let hits = b.hits();
    assert_eq!(hits.len(), n, "one hit per projectile");
    assert!(hits.iter().all(|h| h.attack_name == "SF_MISHOT" && h.flags == 0xFD | 12 << 8), "{:?}", hits.iter().map(|h| (h.attack_name, h.flags)).collect::<Vec<_>>());
    assert!(hits.iter().any(|h| h.target == TargetId(1)) && hits.iter().any(|h| h.target == TargetId(2)));
    // Back to normal.
    b.assert_back_to_normal();
    assert_ne!(b.step(&[]).state, StateId::Shootflow);
}
