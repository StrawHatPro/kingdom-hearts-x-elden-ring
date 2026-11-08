//! (SHOT-A drain) Drain Shock (Hero's Origin, kind 3 HE_DRAIN) and Shimmering Drops (Ever After, kind 10 RA_DRAIN): KH3's
//! update 0x1413B73D0 (magic/shotlocks/drain.rs). StyleChange11 (2.5 s, lifted 100 cm); its TresTiming (0.2 s) spawns ONE
//! blow at Sora (an 800 cm sphere, SF_DRAIN_BLOW / SF_RADRAIN_BLOW); its TresAtk begin (1.2333 s) one drain per lock at
//! every locked target (SF_DRAIN / SF_RADRAIN); each drained target gives an orb (vp_dra_blt02) that spirals to him for
//! 0.8 s, then HP + 6 x locks; M_1472 (0.7667 s); finish.
//! Trace per kind: lock-on -> fire -> finish -> back to normal.

mod shotlocks_common;

use kh_core::host::{Call, HitRecord};
use kh_core::{AnimId, PadButton, StateId, TargetId};
use shotlocks_common::{dummy, level99_with, seconds, Bench, TraceStep, DT};

struct Run {
    ground: f32,
    trace: Vec<TraceStep>,
    hits: Vec<HitRecord>,
    locks: i32,
    hp_before: i32,
    hp_curve: Vec<i32>,
    orb_spawns: Vec<u32>,
    orb_stops: Vec<u32>,
}

/// Lock `locks` on two dummies 400 cm ahead (inside the blow's 800 cm), fire, trace the state and the HP, back to normal.
fn run(key: &str, kind: u8, max_locks: i32, locks: i32) -> Run {
    let mut status = level99_with(key);
    let hp_before = status.max_hp() - 150;
    status.set_hp(hp_before);
    let mut b = Bench::new(status, &[dummy(1, 400.0, 0.0), dummy(2, 400.0, 150.0)]);
    assert_eq!(b.aim_until(locks, 400), kind, "the base kind while aiming");
    assert_eq!(b.sora.aim().max_locks, max_locks, "TresShootFlowData Param0");
    let locks = b.sora.aim().total;
    assert!(locks < max_locks, "not MAX (the MAX kind is another family)");
    let hp_before = b.sora.status().hp;
    let ground = b.step(&[PadButton::RightShoulder]).position.z;
    b.fire();
    let mut trace = Vec::new();
    let mut hp_curve = Vec::new();
    for _ in 0..600 {
        let t = b.trace_step();
        hp_curve.push(b.sora.status().hp);
        let done = t.state != StateId::Shootflow;
        trace.push(t);
        if done {
            break;
        }
    }
    assert_ne!(trace.last().unwrap().state, StateId::Shootflow, "the state ended");
    let orb_spawns = b.host.calls.iter().filter_map(|c| match c { Call::Spawn(h, s) if s.name == "vp_dra_blt02" => Some(h.0), _ => None }).collect();
    let orb_stops = b.host.calls.iter().filter_map(|c| match c { Call::StopEffect(h, _) => Some(h.0), _ => None }).collect();
    let hits = b.hits();
    b.assert_back_to_normal();
    Run { ground, trace, hits, locks, hp_before, hp_curve, orb_spawns, orb_stops }
}

trait TraceStepExt {
    fn trace_step(&mut self) -> TraceStep;
}

impl TraceStepExt for Bench {
    fn trace_step(&mut self) -> TraceStep {
        let o = self.step(&[]);
        TraceStep { t: self.time, state: o.state, sub: o.sub_step, clip: o.anim.name, position: o.position, projectiles: self.sora.magic().projectile_views().count(), hits: self.host.hits().len() }
    }
}

fn check(r: &Run, kind: u8, blow_row: &str, drain_row: &str) {
    let flags = 0xFD | (kind as u32) << 8;
    let t = |i: usize| seconds(i + 1);
    // StyleChange11 from the first tick, lifted 100 cm (LiftCheck(100)).
    assert_eq!(r.trace[0].clip, AnimId::StyleChange11);
    let top = r.trace.iter().map(|s| s.position.z).fold(f32::MIN, f32::max);
    assert!((top - r.ground - 100.0).abs() < 15.0, "lifted {}", top - r.ground);
    // The blow at 0.2 s (TresTiming): sub 4 -> 5.
    let blow = r.trace.iter().position(|s| s.sub == 5).expect("sub 5");
    assert!((t(blow) - 0.2).abs() < 2.5 * DT, "blow at {}", t(blow));
    // The drains at 1.2333 s (TresAtk begin): sub 5 -> 6.
    let drains = r.trace.iter().position(|s| s.sub == 6).expect("sub 6");
    assert!((t(drains) - 1.2333).abs() < 2.5 * DT, "drains at {}", t(drains));
    // The blow hits both dummies once; one drain hit per lock.
    let blows: Vec<&HitRecord> = r.hits.iter().filter(|h| h.attack_name == blow_row).collect();
    let drain_hits: Vec<&HitRecord> = r.hits.iter().filter(|h| h.attack_name == drain_row).collect();
    assert_eq!(blows.len(), 2, "{:?}", r.hits.iter().map(|h| (h.attack_name, h.target)).collect::<Vec<_>>());
    assert_eq!(drain_hits.len() as i32, r.locks, "one drain per lock");
    assert!(r.hits.iter().all(|h| h.flags == flags), "flags 0xFD | kind << 8");
    assert!([TargetId(1), TargetId(2)].iter().all(|t| drain_hits.iter().any(|h| h.target == *t)), "both targets drained");
    // One orb per drained target, all off again by the end.
    assert_eq!(r.orb_spawns.len(), 2, "one orb per target");
    assert!(r.orb_spawns.iter().all(|h| r.orb_stops.contains(h)), "the orbs are stopped");
    // HP + 6 x locks once the 0.8 s run out (1.2333 + 0.8), not before.
    let healed = r.hp_curve.iter().position(|hp| *hp > r.hp_before).expect("healed");
    assert!((t(healed) - 2.0333).abs() < 3.5 * DT, "healed at {}", t(healed));
    assert_eq!(*r.hp_curve.last().unwrap() - r.hp_before, 6 * r.locks, "6 HP per lock");
    // M_1472 at StyleChange11's end (2.5 s), finished at its end (+0.7667 s).
    let end = r.trace.iter().position(|s| s.clip == AnimId::M1472).expect("M_1472");
    assert!((t(end) - 2.5).abs() < 2.5 * DT, "M_1472 at {}", t(end));
    assert!((seconds(r.trace.len()) - 3.2667).abs() < 0.1, "ended at {}", seconds(r.trace.len()));
}

#[test]
fn drain_shock_lock_on_fire_finish_back_to_normal() {
    let r = run("WEP_KEYBLADE_SO_01", 3, 16, 6);
    check(&r, 3, "SF_DRAIN_BLOW", "SF_DRAIN");
}

/// (SHOT-FIX) The DrainEx has `m_bAttachToHitActor`: its hit Explodes it and Explode's slot 537 (0x141473E30) attaches it
/// to its victim (KeepWorldTransform), where it stays for its `m_ExplodeLife` (0: until the next tick's +0x758 countdown
/// destroys it). The generic attach of magic/projectile.rs (`stick`, as combat/hit.rs `ShotStick` of the form shots).
#[test]
fn the_drain_attaches_to_its_victim() {
    let mut b = Bench::new(level99_with("WEP_KEYBLADE_SO_01"), &[dummy(1, 400.0, 0.0), dummy(2, 400.0, 150.0)]);
    assert_eq!(b.aim_until(4, 400), 3);
    b.fire();
    let mut checked = 0;
    for _ in 0..300 {
        b.step(&[]);
        let hits: Vec<TargetId> = b.hits().iter().filter(|h| h.attack_name == "SF_DRAIN").map(|h| h.target).collect();
        let stuck: Vec<_> = b.sora.magic().projectiles.iter().filter(|p| !p.dead && p.def.class_name == "TresProjHE_DrainEx_C" && p.stick.is_some_and(|s| s.stuck())).map(|p| (p.stick.unwrap().target, p.position)).collect();
        for (target, at) in &stuck {
            let t = b.host.targets.iter().find(|t| t.id == *target).unwrap();
            assert!(hits.contains(target), "attached to a victim it hit");
            assert!((*at - t.center).length() < 1.0, "on its victim ({at:?})");
            checked += 1;
        }
        if !stuck.is_empty() {
            // m_ExplodeLife 0: gone the next tick
            b.step(&[]);
            assert!(!b.sora.magic().projectiles.iter().any(|p| p.def.class_name == "TresProjHE_DrainEx_C" && p.stick.is_some()), "destroyed after ExplodeLife 0");
            break;
        }
    }
    assert!(checked >= 2, "every drain that hit was attached ({checked})");
}

#[test]
fn shimmering_drops_lock_on_fire_finish_back_to_normal() {
    let r = run("WEP_KEYBLADE_SO_04", 10, 28, 8);
    check(&r, 10, "SF_RADRAIN_BLOW", "SF_RADRAIN");
}
