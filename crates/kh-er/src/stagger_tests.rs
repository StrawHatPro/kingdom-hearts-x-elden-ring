//! (GF-BREAK) Goofy's Stagger on Elden Ring's bodies, the engine-free part: the one explicit event into the poise rules (PLAN.md
//! R6) and the host's answers. The game's side (the kneel, `W_Idle`, a real no-death state) needs the game: stance.rs, and the
//! checks listed in the lane's report.

use crate::ermap::{Params, Weight};
use crate::puppet::Puppets;
use crate::targets::{Enemy, ErTargets};
use crate::coords::Frame;
use glam::Vec3;
use kh_core::enums::ETresDamageKind as Kind;
use kh_core::host::StaggerAsk;
use kh_core::{HitRecord, TargetId, Targets};
use kh_react::HitOutcome;

fn knight() -> Enemy {
    let mut e = Enemy::stub(2, Vec3::new(300.0, 0.0, 0.0), 45.0, 95.0, 2500, Weight::Medium);
    e.poise_max = 65.0;
    e.poise = 65.0;
    e
}

fn toss(e: &Enemy) -> HitRecord {
    let mut h = HitRecord { damage: 30.0, damage_kind: Kind::Toss, direction: Vec3::X, ..HitRecord::empty(e.id) };
    h.position = e.pos + Vec3::Z * 50.0;
    h
}

#[test]
fn a_launch_a_knight_refuses_goes_through_in_the_window_a_full_stagger_opens() {
    let params = Params::default();
    let e = knight();
    let mut p = Puppets::default();
    let unbroken = p.hit(&params, &e, &toss(&e), true, false);
    assert!(!matches!(unbroken, HitOutcome::Started { kind, .. } if kind == Kind::Toss), "{unbroken:?}");
    assert_eq!(p.stagger_break(&params, &e, 2.5), Ok(()));
    let o = p.hit(&params, &e, &toss(&e), true, false);
    assert!(matches!(o, HitOutcome::Started { .. }), "{o:?}");
}

#[test]
fn a_knockdown_goes_through_the_meter_of_a_body_we_throw_about() {
    let params = Params::default();
    let e = knight();
    let mut p = Puppets::default();
    let mut down = toss(&e);
    down.damage_kind = Kind::Blow;
    down.flags |= kh_core::host::HIT_FLAG_KNOCKDOWN;
    let o = p.hit(&params, &e, &down, true, false);
    assert!(matches!(o, HitOutcome::Started { kind: Kind::Blow, .. }), "{o:?}");
}

#[test]
fn a_knockdown_flings_no_armoured_body() {
    let params = Params::default();
    // too tough for the class table: `puppet_ok` is false, an armoured body
    let mut e = Enemy::stub(3, Vec3::new(300.0, 0.0, 0.0), 45.0, 95.0, 2500, Weight::Large);
    e.puppet_ok = false;
    let mut p = Puppets::default();
    let mut down = toss(&e);
    down.damage_kind = Kind::Blow;
    down.flags |= kh_core::host::HIT_FLAG_KNOCKDOWN;
    let mut hard = HitRecord { ..down };
    // what `classify` makes of a STRONG body with no reaction power
    hard.info.victim_result = kh_core::host::ReactionResult::StrongBodyDamage1;
    let o = p.hit(&params, &e, &hard, true, false);
    assert!(!matches!(o, HitOutcome::Started { .. }), "{o:?}");
}

#[test]
fn a_released_window_opens_nothing_more() {
    let params = Params::default();
    let e = knight();
    let mut p = Puppets::default();
    p.stagger_hold(e.id, 5.0);
    p.stagger_release(&params, e.id, 3.0);
    let o = p.hit(&params, &e, &toss(&e), true, false);
    assert!(!matches!(o, HitOutcome::Started { kind, .. } if kind == Kind::Toss), "no break, no launch: {o:?}");
}

#[test]
fn a_target_with_no_character_never_breaks_and_is_never_defeated() {
    let mut t = ErTargets::new(Frame::at(Vec3::ZERO), Params::default());
    t.list.push(knight());
    let id = TargetId(2);
    assert!(!t.stagger_ask(id, StaggerAsk::Break { window: 2.5 }));
    assert!(!t.stagger_ask(id, StaggerAsk::MayDefeat));
    assert!(!t.stagger_ask(TargetId(99), StaggerAsk::Break { window: 2.5 }));
    // (GF-POLISH) no character, or a body barred: no delay, nothing written
    assert!(!t.stagger_ask(id, StaggerAsk::Delay { seconds: 0.8, lost: 2.0 }));
    assert!(!t.stagger_ask(TargetId(99), StaggerAsk::Delay { seconds: 0.8, lost: 2.0 }));
    // the end of a break nobody began does nothing
    assert!(t.stagger_ask(id, StaggerAsk::Hold { seconds: 2.0 }));
    assert!(t.stagger_ask(id, StaggerAsk::Release { juggle: 0.0 }));
}
