//! (GF-THREAT) The threat book through the party's step: Goofy's events and multipliers, Taunt in and out of range and its end, a
//! friend's hits, Taunt Pass, the lock-on rule. The book's own rules are tested in threat.rs.

use super::seams::{self, GoofyEvent, ThreatWhy};
use super::threat::{self, ThreatBook};
use super::{GoofyAbilities, GoofyAbility as GA, K};
use crate::donald::cast::SORA_MEMBER;
use crate::host::PartyHost;
use crate::ids::{Chara, MemberId};
use crate::pad::goofy::Taunt;
use crate::party::Party;
use crate::view::{PartyEvent, PartyEventKind, PartyOutput, SoraView};
use crate::PartyConfig;
use kh_core::host::{HitRecord, HitResult, HurtShape, TargetFlags, TargetId, TargetInfo, Targets};
use kh_core::test_world::TestWorld;
use kh_core::Vec3;

const DT: f32 = 1.0 / 60.0;
const NEAR: TargetId = TargetId(1);
const FAR: TargetId = TargetId(2);
/// Alive and hittable, but the lock-on refuses it: a pot, a hidden body, a proxy.
const REFUSED: TargetId = TargetId(3);

struct Foes;

impl Targets for Foes {
    fn enumerate(&self, out: &mut Vec<TargetInfo>) {
        out.extend([NEAR, FAR, REFUSED].into_iter().filter_map(|t| self.info(t)));
    }
    fn info(&self, id: TargetId) -> Option<TargetInfo> {
        let (x, lock) = match id {
            NEAR => (900.0, true),
            FAR => (1100.0, true),
            REFUSED => (400.0, false),
            _ => return None,
        };
        let flags = TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::ENEMY | if lock { TargetFlags::LOCKABLE } else { 0 };
        let at = Vec3::new(x, 0.0, 0.0);
        Some(TargetInfo { id, position: at, center: at + Vec3::Z * 90.0, velocity: Vec3::ZERO, yaw: 180.0, radius: 50.0, half_height: 90.0, flags: TargetFlags(flags), hp: 1000.0, hp_max: 1000.0 })
    }
    fn hurt_shapes(&self, _id: TargetId, _out: &mut Vec<HurtShape>) {}
    fn apply_hit(&mut self, _hit: &HitRecord) -> HitResult {
        HitResult { accepted: true, ..Default::default() }
    }
    // (AUDIT-B) no bones here: a rider attached to a hit follows the target's origin and yaw
    fn hit_bone(&self, _id: kh_core::TargetId, _at: kh_core::Vec3) -> Option<kh_core::host::TargetBone> {
        None
    }
    fn bone_pose(&self, _id: kh_core::TargetId, _bone: kh_core::host::TargetBone) -> Option<kh_core::host::BonePose> {
        None
    }
}

struct Rig {
    world: TestWorld,
    foes: Foes,
    audio: kh_core::host::audio::NullAudio,
    effects: kh_core::host::effects::NullEffects,
    party: Party,
    sora: SoraView,
    goofy: MemberId,
}

impl Rig {
    fn new() -> Rig {
        let mut party = Party::new(PartyConfig::test_level99());
        let goofy = party.spawn(Chara::Goofy, Vec3::ZERO, 0.0);
        let mut rig = Rig { world: TestWorld::floor(), foes: Foes, audio: Default::default(), effects: Default::default(), party, sora: SoraView::default(), goofy };
        rig.sora.position = Vec3::new(-3000.0, 0.0, 0.0);
        rig.sora.hp_max = 1000;
        rig.wear(&[]);
        rig
    }

    fn wear(&mut self, list: &[GA]) {
        let mut a = GoofyAbilities::default();
        for k in GA::ALL {
            a.give(k);
        }
        for k in list {
            a.set_equipped(*k, true);
        }
        self.state().abilities = a;
    }

    fn state(&mut self) -> &mut super::GoofyState {
        self.party.status_mut(self.goofy).unwrap().goofy.as_mut().unwrap()
    }

    fn step(&mut self) -> PartyOutput {
        let mut host = PartyHost { world: &self.world, targets: &mut self.foes, audio: &mut self.audio, effects: &mut self.effects };
        self.party.step(DT, &self.sora, &mut host)
    }

    /// An event of his, as the kit raises them.
    fn raise(&mut self, target: TargetId, amount: f32, why: ThreatWhy) {
        self.state().core.events.push(GoofyEvent::Threat { target, amount, why });
        self.step();
    }

    fn taunt(&mut self, secs: f32) {
        self.state().run.taunt = Some(Taunt { centre: Vec3::ZERO, radius: K.art_taunt_radius, left: secs });
    }

    fn book(&self) -> &ThreatBook {
        &self.party.gof.threat
    }
}

/// A guard, a deflect and a basic hit earn their rows x his multiplier, and only his.
#[test]
fn goofys_events_fill_the_book_times_his_multiplier() {
    let mut rig = Rig::new();
    rig.raise(NEAR, K.threat_deflect, ThreatWhy::Deflect);
    let v = rig.book().value(NEAR, rig.goofy);
    assert!((v - K.threat_deflect * K.threat_base_mult).abs() < 0.01, "{v}");
    rig.raise(FAR, K.threat_guard, ThreatWhy::Guard);
    rig.raise(FAR, K.threat_basic, ThreatWhy::Basic);
    let v = rig.book().value(FAR, rig.goofy);
    assert!((v - (K.threat_guard + K.threat_basic) * K.threat_base_mult).abs() < 0.02, "{v}");
    assert_eq!(rig.book().top(FAR), Some(rig.goofy));
}

/// R15: x1.5 base, x3 at Resolve 2+, Loud Mouth +25 % on top.
#[test]
fn the_multipliers() {
    let mut rig = Rig::new();
    let m = |rig: &mut Rig| seams::threat_mult(rig.state());
    assert_eq!(m(&mut rig), 1.5);
    rig.state().core.resolve.thirds = 5;
    assert_eq!(m(&mut rig), 1.5, "Resolve 1 and two thirds");
    rig.state().core.resolve.thirds = 6;
    assert_eq!(m(&mut rig), 3.0, "Resolve 2");
    assert!(threat::strong(rig.state()));
    rig.wear(&[GA::LoudMouth]);
    assert_eq!(m(&mut rig), 3.0 * 1.25);
    rig.state().core.resolve.thirds = 0;
    assert_eq!(m(&mut rig), 1.5 * 1.25);
}

/// Taunt raises his threat on the enemies in reach and only those; what the lock-on refuses earns nothing however close.
#[test]
fn taunt_in_and_out_of_range() {
    let mut rig = Rig::new();
    rig.taunt(5.0);
    for _ in 0..60 {
        rig.step();
    }
    let on_near = rig.book().value(NEAR, rig.goofy);
    assert!(on_near > K.threat_taunt * 0.9, "a second of it at x1.5: {on_near}");
    assert_eq!(rig.book().value(FAR, rig.goofy), 0.0, "11 m: out of range");
    assert_eq!(rig.book().value(REFUSED, rig.goofy), 0.0, "the lock-on refuses it");
    assert_eq!(threat::taunter_of(&rig.party, Vec3::new(900.0, 0.0, 0.0)), Some(rig.goofy));
    assert_eq!(threat::taunter_of(&rig.party, Vec3::new(1100.0, 0.0, 0.0)), None);
}

/// It runs out on its clock, whoever drives him: the choice is free again, his points fade.
#[test]
fn taunt_expires() {
    let mut rig = Rig::new();
    rig.taunt(0.5);
    for _ in 0..29 {
        rig.step();
    }
    assert!(rig.party.goofy_taunt(rig.goofy).is_some());
    for _ in 0..3 {
        rig.step();
    }
    assert!(rig.party.goofy_taunt(rig.goofy).is_none(), "0.5 s");
    assert_eq!(threat::taunter_of(&rig.party, Vec3::new(900.0, 0.0, 0.0)), None);
    let peak = rig.book().value(NEAR, rig.goofy);
    for _ in 0..60 * K.threat_half_life as usize {
        rig.step();
    }
    assert!(rig.book().value(NEAR, rig.goofy) < peak * 0.55, "the book halves");
}

/// A taunt on an AI Goofy works as on a pad: the same state, the same clock; no pad is ever set in this rig.
#[test]
fn taunt_needs_no_pad() {
    let mut rig = Rig::new();
    assert!(rig.party.member(rig.goofy).unwrap().pad.is_none());
    rig.taunt(1.0);
    rig.step();
    assert!(rig.party.goofy_taunt(rig.goofy).is_some_and(|t| t.left < 1.0));
    assert!(rig.book().value(NEAR, rig.goofy) > 0.0);
}

/// The lock-on rule: a hit, guard or deflect on what the lock-on refuses earns no row and raises no stance.
#[test]
fn what_the_lock_on_refuses_earns_no_threat() {
    let mut rig = Rig::new();
    rig.raise(REFUSED, 10.0, ThreatWhy::Guard);
    assert!(rig.book().rows().is_empty());
    assert!(rig.party.gof.threat.take_stances().is_empty());
    rig.raise(TargetId(99), 10.0, ThreatWhy::Guard);
    assert!(rig.book().rows().is_empty(), "a target gone");
}

/// A guard and a deflect reach the director's ledger as touches, once; a basic hit does not (the host's own hit route has it).
#[test]
fn guards_and_deflects_reach_the_director_once() {
    let mut rig = Rig::new();
    rig.raise(NEAR, K.threat_basic, ThreatWhy::Basic);
    assert!(rig.party.gof.threat.take_stances().is_empty());
    rig.raise(NEAR, K.threat_guard, ThreatWhy::Guard);
    rig.raise(FAR, K.threat_deflect, ThreatWhy::Deflect);
    assert_eq!(rig.party.gof.threat.take_stances(), vec![(rig.goofy, NEAR), (rig.goofy, FAR)]);
    assert!(rig.party.gof.threat.take_stances().is_empty());
}

/// Synergy: a guard or a deflect raises Taunt Pass even when it is a moment's state.
#[test]
fn a_guard_raises_taunt_pass() {
    let mut rig = Rig::new();
    assert_eq!(rig.party.don_prog.taunt_now(), None);
    rig.raise(NEAR, K.threat_deflect, ThreatWhy::Deflect);
    assert_eq!(rig.party.don_prog.taunt_now(), Some(rig.goofy));
}

/// A friend's hit counts its plain points; his own hit event is not counted twice; a pot earns nothing.
#[test]
fn a_friends_hit_counts_plain_points() {
    let mut rig = Rig::new();
    let donald = rig.party.spawn(Chara::Donald, Vec3::new(100.0, 100.0, 0.0), 0.0);
    let hit = |member, target| PartyEvent { member, kind: PartyEventKind::Hit { target, damage: 5.0, killed: false, attack: "X" } };
    let out = PartyOutput { events: vec![hit(donald, NEAR), hit(rig.goofy, NEAR), hit(donald, REFUSED)], ..Default::default() };
    let mut host = PartyHost { world: &rig.world, targets: &mut rig.foes, audio: &mut rig.audio, effects: &mut rig.effects };
    threat::friends_hits(&mut rig.party, &out, &host);
    let _ = &mut host;
    assert_eq!(rig.book().value(NEAR, donald), K.threat_basic);
    assert_eq!(rig.book().value(NEAR, rig.goofy), 0.0, "his come as his own events");
    assert_eq!(rig.book().value(REFUSED, donald), 0.0);
}

/// Sora is no member: his points have their own id, his guard a touch and points.
#[test]
fn soras_side() {
    let mut rig = Rig::new();
    threat::sora_hit(&mut rig.party, NEAR);
    threat::sora_guard(&mut rig.party, NEAR);
    assert_eq!(rig.book().value(NEAR, SORA_MEMBER), K.threat_basic + K.threat_guard);
    assert_eq!(rig.party.gof.threat.take_stances(), vec![(SORA_MEMBER, NEAR)]);
}

/// Goofy's points outweigh a friend's at the same work, x3 at Resolve 2+.
#[test]
fn his_threat_counts_over_a_friends() {
    let mut rig = Rig::new();
    rig.raise(NEAR, K.threat_basic, ThreatWhy::Basic);
    threat::sora_hit(&mut rig.party, NEAR);
    let ratio = |rig: &Rig, t: TargetId| rig.book().value(t, rig.goofy) / rig.book().value(t, SORA_MEMBER);
    assert!((ratio(&rig, NEAR) - 1.5).abs() < 0.01, "1.5 against 1");
    rig.state().core.resolve.thirds = 6;
    rig.raise(FAR, K.threat_basic, ThreatWhy::Basic);
    threat::sora_hit(&mut rig.party, FAR);
    assert!((ratio(&rig, FAR) - 3.0).abs() < 0.01, "3 against 1");
    assert!(rig.book().share(FAR, rig.goofy) > rig.book().share(NEAR, rig.goofy));
}
