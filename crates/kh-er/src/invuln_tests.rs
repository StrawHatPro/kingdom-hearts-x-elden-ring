//! (COMBAT-FIX, THE USER 2026-11-06: "Rennala can currently be hit in her shield bubble during phase 1, this is not correct she
//! should only be targetable when her shield has broken") Elden Ring's own "cannot be damaged" state on the host's target list
//! (invuln.rs, targets.rs): a body the game holds invulnerable takes nothing from the party's hits of any route and is no
//! target; cleared, it is a target again. Engine-free: rows with no game object behind them.

use crate::coords::Frame;
use crate::ermap::{Params, Weight};
use crate::invuln::{State, Why};
use crate::targets::{Enemy, ErTargets};
use glam::Vec3;
use kh_core::enums::ETresDamageKind as Kind;
use kh_core::host::{StaggerAsk, TargetFlags, HIT_FLAG_BREAKER, HIT_FLAG_FORCE};
use kh_core::{HitRecord, TargetId, Targets};

/// Rennala's phase-1 body in her barrier (SpEffect 14361 on her: every damage cut rate 0).
fn rennala(state: State) -> ErTargets {
    let mut t = ErTargets::new(Frame::at(Vec3::ZERO), Params::default());
    let mut e = Enemy::stub(7, Vec3::new(400.0, 0.0, 0.0), 60.0, 140.0, 3195, Weight::Large);
    e.invuln = state;
    t.list.push(e);
    t
}

fn hit(flags: u32) -> HitRecord {
    HitRecord { damage: 80.0, damage_kind: Kind::Toss, direction: Vec3::X, position: Vec3::new(400.0, 0.0, 90.0), flags, ..HitRecord::empty(TargetId(7)) }
}

fn flags_of(t: &ErTargets) -> TargetFlags {
    let mut out = Vec::new();
    t.enumerate(&mut out);
    out.iter().find(|i| i.id == TargetId(7)).map(|i| i.flags).expect("still listed: an enemy in the fight")
}

#[test]
fn a_body_er_holds_invulnerable_takes_nothing_from_any_route_and_is_no_target() {
    let barrier = State::Shut(Why::SpEffect(14361));
    let mut t = rennala(barrier);
    // melee / magic, a forced hit (Zettaflare, the shotlocks' finish), a Breaker's defeat: none goes through
    for flags in [0, HIT_FLAG_FORCE, HIT_FLAG_BREAKER, HIT_FLAG_FORCE | HIT_FLAG_BREAKER] {
        let r = t.apply_hit(&hit(flags));
        assert!(!r.accepted && r.damage_dealt == 0.0 && !r.killed, "flags {flags:#x}: {r:?}");
        assert_eq!(r.hp_after, 3195.0);
    }
    assert_eq!(t.get(TargetId(7)).map(|e| e.hp), Some(3195), "no HP of ours");
    assert!(t.events.is_empty(), "no hit event, no KH3 reaction asked");
    // still an enemy in the fight, but neither hittable nor lockable (lock-on, soft target, AI picks, Zettaflare, the shotlock)
    let f = flags_of(&t);
    assert!(f.has(TargetFlags::ENEMY | TargetFlags::ALIVE));
    assert!(!f.has(TargetFlags::HITTABLE) && !f.has(TargetFlags::LOCKABLE), "{f:?}");
    assert!(!t.stagger_ask(TargetId(7), StaggerAsk::MayDefeat));
    // the same for its flags
    for why in [Why::InvincibleFlag, Why::NoHitFlag] {
        let mut t = rennala(State::Shut(why));
        assert!(!t.apply_hit(&hit(0)).accepted);
        assert!(!flags_of(&t).has(TargetFlags::LOCKABLE));
    }
}

#[test]
fn its_invincibility_frames_pass_hits_through_and_keep_the_lock() {
    let mut t = rennala(State::Frames);
    assert!(!t.apply_hit(&hit(0)).accepted);
    let f = flags_of(&t);
    assert!(f.has(TargetFlags::LOCKABLE) && !f.has(TargetFlags::HITTABLE), "{f:?}");
}

#[test]
fn the_barrier_broken_she_is_a_target_and_hittable_again() {
    let mut t = rennala(State::Shut(Why::SpEffect(14361)));
    assert!(!flags_of(&t).has(TargetFlags::HITTABLE));
    // the next frame's read: ER lets her be damaged (the ring's SpEffect gone)
    t.list[0].invuln = State::Open;
    let f = flags_of(&t);
    assert!(f.has(TargetFlags::ENEMY | TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE), "{f:?}");
    assert!(!t.list[0].invuln.refuses_damage(), "the hit gate and the HP fallback are open again");
}

/// (COMBAT-FIX follow-up) HER STATE FROM HER LIVE SPEFFECTS, as the game's data has them (invuln.rs `rennala`): each barrier
/// ring and the shatter-and-fall (14578, until a000_003010's 14579 replaces it at 5.667 s) hold her shut, as they hold off the
/// Tarnished's hits; from then on she is hittable and lockable.
#[test]
fn her_live_speffects_decide_barrier_shatter_and_window() {
    use crate::invuln::{rennala as r, state_of, Facts};
    let state = |effects| state_of(&Facts { effects, ..Facts::default() });
    for (name, effects) in [("ring 1", r::barrier(r::RING_1)), ("ring 2", r::barrier(r::RING_2)), ("ring 3", r::barrier(r::RING_3)), ("shatter and fall", r::falling())] {
        let t = rennala(state(effects));
        let f = flags_of(&t);
        assert!(!f.has(TargetFlags::HITTABLE) && !f.has(TargetFlags::LOCKABLE), "{name}: {f:?}");
    }
    let mut t = rennala(state(r::open()));
    let f = flags_of(&t);
    assert!(f.has(TargetFlags::ENEMY | TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE), "after the lift: {f:?}");
    assert!(!t.stagger_ask(TargetId(7), StaggerAsk::Break { window: 2.5 }), "(no game object behind the row: refused for that reason only)");
    assert_eq!(t.get(TargetId(7)).map(|e| e.invuln), Some(State::Open));
}

/// (MIMIC-BOSS, THE USER 2026-11-07: "also mimic tear boss should not be targetable or attackable until it has transformed into
/// Sora") The Mimic Tear's body while its copy forms rides the same gate: no hit of any route, no lock; open once formed.
#[test]
fn a_mimic_tear_still_forming_is_no_target_and_takes_nothing_then_opens() {
    let mut t = rennala(State::Shut(Why::Forming));
    for flags in [0, HIT_FLAG_FORCE, HIT_FLAG_BREAKER, HIT_FLAG_FORCE | HIT_FLAG_BREAKER] {
        let r = t.apply_hit(&hit(flags));
        assert!(!r.accepted && r.damage_dealt == 0.0 && !r.killed, "flags {flags:#x}: {r:?}");
    }
    let f = flags_of(&t);
    assert!(!f.has(TargetFlags::HITTABLE) && !f.has(TargetFlags::LOCKABLE), "{f:?}");
    // the copy has taken over: the next frame's word is open
    t.list[0].invuln = State::Open;
    let f = flags_of(&t);
    assert!(f.has(TargetFlags::HITTABLE | TargetFlags::LOCKABLE), "{f:?}");
    assert!(!t.list[0].invuln.refuses_damage(), "the hit gate is open again (a stub body has no game HP to take: not dealt here)");
}
