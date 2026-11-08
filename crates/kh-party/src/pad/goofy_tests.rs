//! (GOOFY) Goofy's kit, one test per rule of documentation/GOOFY_OVERHAUL.md 3 - 7, 10, 11 and PLAN.md R1 - R3, R13, R14.

use super::super::{PadFrame, JUMP};
use super::*;
use crate::goofy::{seams, GoofyAbilities, RankBank, Telegraph, RESOLVE_FULL};
use crate::growth::{Growth, RankLine};
use crate::{Party, PartyConfig, SoraView};
use kh_core::enums::ETresDamageKind as Kind;
use kh_core::host::{AttackInfo, BodyReaction, HitAttacker, HurtShape, StaggerAsk, TargetBody, TargetDefense, TargetFlags, Targets};
use kh_core::input::PadButtons;
use kh_core::test_world::TestWorld;
use kh_core::Tuning;

const DT: f32 = 1.0 / 60.0;

/// The enemies, the hits, every question the kit asked, and whether their bodies are too heavy to fling.
struct Dummies(Vec<(TargetId, Vec3)>, Vec<HitRecord>, Vec<StaggerAsk>, bool);

impl Dummies {
    fn info_of(&self, (id, p): (TargetId, Vec3)) -> TargetInfo {
        TargetInfo { id, position: p, center: p + Vec3::Z * 90.0, velocity: Vec3::ZERO, yaw: 180.0, radius: 50.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 }
    }
}

impl Targets for Dummies {
    fn enumerate(&self, out: &mut Vec<TargetInfo>) {
        for &t in &self.0 {
            out.push(self.info_of(t));
        }
    }
    fn info(&self, id: TargetId) -> Option<TargetInfo> {
        self.0.iter().find(|t| t.0 == id).map(|&t| self.info_of(t))
    }
    fn hurt_shapes(&self, id: TargetId, out: &mut Vec<HurtShape>) {
        if let Some(i) = self.info(id) {
            out.push(HurtShape { a: i.position + Vec3::Z * 30.0, b: i.position + Vec3::Z * 150.0, radius: 60.0, part: 0 });
        }
    }
    fn apply_hit(&mut self, hit: &HitRecord) -> HitResult {
        self.1.push(*hit);
        HitResult { accepted: true, damage_dealt: hit.damage, hp_after: 1.0e6, killed: false, guarded: false }
    }
    fn stagger_ask(&mut self, _id: TargetId, ask: StaggerAsk) -> bool {
        self.2.push(ask);
        true
    }
    fn body(&self, _id: TargetId, _part: u32) -> TargetBody {
        TargetBody { reaction: if self.3 { BodyReaction::Strong } else { BodyReaction::Default }, ..TargetBody::DEFAULT }
    }
    fn defense(&self, _id: TargetId) -> Option<TargetDefense> {
        Some(TargetDefense { defense: 10, max_hp: 1_000_000, hp_percent: 100.0, enemy_kind: true, damage_min: 0, damage_max: 0, resist: [0; 8], bio_type: 0 })
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
    dummies: Dummies,
    audio: kh_core::host::audio::NullAudio,
    effects: kh_core::host::effects::NullEffects,
    party: Party,
    sora: SoraView,
    id: crate::MemberId,
    held: PadButtons,
    /// The Stagger seen so far (the party keeps one step's events).
    events: Vec<GoofyEvent>,
}

const X: PadButton = PadButton::FaceLeft;
const A: PadButton = PadButton::FaceBottom;
const B: PadButton = PadButton::FaceRight;
const FWD: Vec2 = Vec2::new(0.0, 1.0);
const BACK: Vec2 = Vec2::new(0.0, -1.0);
const RIGHT: Vec2 = Vec2::new(1.0, 0.0);
const NONE: Vec2 = Vec2::ZERO;

/// The one enemy of the rigs stands here, ahead of him.
const ENEMY: Vec3 = Vec3::new(200.0, 0.0, 0.0);
const T: TargetId = TargetId(1);

impl Rig {
    fn new(dummies: &[Vec3]) -> Rig {
        Rig::at(dummies, true)
    }

    fn at(dummies: &[Vec3], level99: bool) -> Rig {
        let mut party = Party::new(if level99 { PartyConfig::test_level99() } else { PartyConfig::default() });
        let id = party.spawn(Chara::Goofy, Vec3::ZERO, 0.0);
        party.set_pad(id, Some(Tuning::USER));
        let d = Dummies(dummies.iter().enumerate().map(|(i, p)| (TargetId(i as u32 + 1), *p)).collect(), Vec::new(), Vec::new(), false);
        let mut rig = Rig { world: TestWorld::floor(), dummies: d, audio: Default::default(), effects: Default::default(), party, sora: SoraView::default(), id, held: PadButtons::NONE, events: Vec::new() };
        rig.sora.position = Vec3::new(-3000.0, 0.0, 0.0);
        rig.idle(10);
        rig
    }

    /// A rig with one enemy ahead and no ability worn: the plain rules.
    fn plain() -> Rig {
        let mut rig = Rig::new(&[ENEMY]);
        rig.bare();
        rig
    }

    /// Every ability owned (the level does not give them back) and none worn.
    fn bare(&mut self) {
        let mut a = GoofyAbilities::default();
        for k in GA::ALL {
            a.give(k);
        }
        self.s().abilities = a;
    }

    fn m(&self) -> &NpcPawn {
        self.party.member(self.id).unwrap()
    }

    fn mm(&mut self) -> &mut NpcPawn {
        self.party.member_mut(self.id).unwrap()
    }

    fn s(&mut self) -> &mut crate::goofy::GoofyState {
        self.mm().status.goofy.as_mut().unwrap()
    }

    fn k(&self) -> &crate::goofy::GoofyCore {
        &self.m().status.goofy.as_ref().unwrap().core
    }

    fn run_state(&self) -> &GoofyRun {
        &self.m().status.goofy.as_ref().unwrap().run
    }

    fn guard(&self) -> crate::guard::Guard {
        self.m().status.guard.expect("Goofy has GUARD")
    }

    fn wear(&mut self, a: GA) {
        let s = self.s();
        s.abilities.give(a);
        s.abilities.set_equipped(a, true);
    }

    fn level(&mut self, level: u32) {
        self.mm().status.level = level;
    }

    /// The boss ranks banked on a line (0 .. 2).
    fn bank(&mut self, line: RankLine, n: u64) {
        let i = RankLine::ALL.iter().position(|l| *l == line).unwrap();
        let mut parts: Vec<u64> = self.m().status.growth.ranks.parts().iter().map(|v| *v as u64).collect();
        parts[i] = n;
        self.mm().status.growth.ranks = RankBank::from_parts(&parts);
    }

    fn step(&mut self, stick: Vec2, held: &[PadButton]) {
        let mut now = PadButtons::NONE;
        for b in held {
            now.set(*b, true);
        }
        let pad = kh_core::input::PadState { held: now, pressed: PadButtons(now.0 & !self.held.0), released: PadButtons(self.held.0 & !now.0) };
        self.held = now;
        if let Some(pad) = self.party.member_mut(self.id).and_then(|m| m.pad.as_mut()).filter(|p| p.finisher_pending) {
            pad.finisher_pending = false;
            pad.finisher_answer = Some(false);
        }
        self.party.set_pad_frame(self.id, PadFrame { stick, camera_yaw: 0.0, pad });
        let mut host = PartyHost { world: &self.world, targets: &mut self.dummies, audio: &mut self.audio, effects: &mut self.effects };
        self.party.step(DT, &self.sora, &mut host);
        self.events.extend(self.party.gof.events.iter().map(|e| e.1));
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.step(NONE, &[]);
        }
    }

    fn run(&mut self, n: usize, stick: Vec2, held: &[PadButton]) {
        for _ in 0..n {
            self.step(stick, held);
        }
    }

    fn tap(&mut self, b: PadButton, stick: Vec2) {
        self.step(stick, &[b]);
        self.step(stick, &[]);
    }

    /// A hit of `class` from the rig's enemy, `damage` HP.
    fn hit(&mut self, class: AttackClass, damage: f32) -> HitResult {
        let me = self.m().position();
        let mut h = HitRecord::empty(TargetId::NONE);
        h.damage = damage;
        h.damage_kind = Kind::Small;
        h.direction = horizontal(me - ENEMY).normalize_or_zero();
        h.attacker = HitAttacker { position: ENEMY, yaw: 180.0, radius: 50.0, valid: true };
        h.position = me + Vec3::Z * 90.0;
        h.attack_info = AttackInfo { class, perilous: matches!(class, AttackClass::Thrust | AttackClass::Sweep | AttackClass::Grab | AttackClass::Heavy), attacker: Some(T) };
        self.party.receive_hit(self.id, &h)
    }

    fn state(&self) -> NpcStateId {
        self.m().state
    }

    fn logged(&self, w: &str) -> bool {
        self.run_state().log.contains(&w)
    }

    fn until(&mut self, n: usize, stick: Vec2, held: &[PadButton], f: impl Fn(&Rig) -> bool) -> usize {
        for i in 0..n {
            if f(self) {
                return i;
            }
            self.step(stick, held);
        }
        panic!("condition not met in {n} steps: state {:?} clip {} log {:?}", self.state(), self.m().anim.clip.name, self.run_state().log);
    }

    fn stagger_total(&self) -> f32 {
        self.events.iter().map(|e| if let GoofyEvent::Stagger { amount, .. } = e { *amount } else { 0.0 }).sum()
    }

    /// The telegraph list for the next steps.
    fn telegraph(&mut self, class: AttackClass, active: bool) {
        self.party.set_telegraphs(&[Telegraph { attacker: T, class, perilous: matches!(class, AttackClass::Sweep | AttackClass::Grab | AttackClass::Thrust | AttackClass::Heavy), eta: 0.0, active }]);
    }

    fn no_telegraphs(&mut self) {
        self.party.set_telegraphs(&[]);
    }
}

// ------------------------------------------------------------------------------------------------ the shoves (3)

/// The three shoves at the doc's powers. Each carries him forward (THE USER 2026-11-02: "His combo should move him a little
/// bit"), but the travel stops at the enemy's body: he closes in up to it and never into or through it.
#[test]
fn three_shoves_at_the_docs_powers_whose_travel_stops_at_the_body() {
    assert_eq!((K.step_shove1_power, K.step_shove2_power, K.step_shove3_power), (0.6, 0.6, 0.9));
    let enemy = Vec3::new(170.0, 0.0, 0.0);
    let mut rig = Rig::new(&[enemy]);
    let start = rig.m().position();
    rig.until(240, NONE, &[], |r| r.state() == NpcStateId::Idle);
    let stop = rig.m().params.capsule_radius + 50.0 + K.combo_travel_gap; // the dummies' radius is 50
    let held_a = |i: usize| if i.is_multiple_of(8) { vec![A] } else { vec![] };
    let mut closest = f32::MAX;
    for i in 0..150 {
        rig.step(NONE, &held_a(i));
        closest = closest.min(horizontal(enemy - rig.m().position()).length());
    }
    let log = &rig.run_state().log;
    let steps: Vec<&&str> = log.iter().filter(|w| w.starts_with("shove")).collect();
    assert_eq!(steps[..3], [&"shove1", &"shove2", &"shove3"], "{log:?}");
    let moved = horizontal(rig.m().position() - start).length();
    assert!(moved > enemy.x - stop - 3.0, "the steps carried him up to the body: {moved} cm");
    assert!(closest >= stop - 1.0, "never into the body: {closest} cm from its centre, the body starts at {stop}");
}

/// On open ground each step travels its row (KH3's root tracks of M_1301 / M_1302: 81 and 117 cm; the bash's 60 is ours), so
/// the whole string covers their sum.
#[test]
fn each_ground_step_travels_its_rows_distance_on_open_ground() {
    let mut rig = Rig::new(&[]);
    let start = rig.m().position();
    rig.tap(A, NONE);
    rig.until(120, NONE, &[], |r| r.state() != NpcStateId::Pad(COMBO));
    let one = horizontal(rig.m().position() - start).length();
    assert!((one - K.step_shove1_travel).abs() < 2.0, "shove 1 carried him {one} cm, its row says {}", K.step_shove1_travel);
    rig.idle(10);
    let from = rig.m().position();
    for i in 0..240 {
        if rig.logged("shove3") {
            break;
        }
        rig.step(NONE, if i % 8 == 0 { &[A] } else { &[] });
    }
    assert!(rig.logged("shove3"), "{:?}", rig.run_state().log);
    rig.until(120, NONE, &[], |r| r.state() != NpcStateId::Pad(COMBO));
    let all = horizontal(rig.m().position() - from).length();
    let rows = K.step_shove1_travel + K.step_shove2_travel + K.step_shove3_travel;
    assert!((all - rows).abs() < 3.0, "the string carried him {all} cm, its rows say {rows}");
}

/// A step's travel stops at a drop deeper than his step height: mashing A at a platform's edge never takes him over it.
#[test]
fn a_steps_travel_never_carries_him_off_a_ledge() {
    let mut rig = Rig::new(&[]);
    rig.world = TestWorld::default().with_box(Vec3::new(-2000.0, -2000.0, -400.0), Vec3::new(60.0, 2000.0, 0.0));
    rig.idle(5);
    assert!(rig.m().on_ground());
    let edge = 60.0 - rig.m().params.capsule_radius;
    for i in 0..200 {
        rig.step(NONE, if i % 8 == 0 { &[A] } else { &[] });
        assert!(rig.m().on_ground(), "step {i}: off the edge at {:?}", rig.m().position());
    }
    assert!(rig.logged("shove2"), "{:?}", rig.run_state().log);
    let x = rig.m().position().x;
    assert!(x > 0.0 && x <= edge + 0.5, "he stepped up to the edge and stopped there: x {x}, edge {edge}");
}

/// The wind-up plays at combo.windup x (THE USER 2026-11-02: "Goofy's combat also feels EXTREMELY stilted"): a press's hit
/// comes out in a quarter second.
#[test]
fn a_press_lands_after_the_quick_wind_up() {
    let mut rig = Rig::new(&[]);
    rig.step(NONE, &[A]);
    let n = rig.until(60, NONE, &[], |r| r.run_state().combo.hit_out);
    let want = K.step_shove1_hit / (K.step_shove1_rate * K.combo_windup) / DT;
    assert!((n as f32 - want).abs() <= 2.0, "the hit came out after {n} steps, the rows say {want}");
    assert!(want * DT < 0.26, "{} s", want * DT);
}

/// The clip's KH3 TurnToTarget window keeps him turning to his target while the step runs (M_1301: 0.26 - 0.51 clip s), at
/// the friends' attack turn rate, not a snap.
#[test]
fn a_step_keeps_turning_to_its_target_in_the_clips_turn_window() {
    let mut rig = Rig::new(&[Vec3::new(300.0, 0.0, 0.0)]);
    rig.tap(PadButton::RightShoulder, NONE);
    rig.step(NONE, &[A]);
    assert_eq!(rig.state(), NpcStateId::Pad(COMBO));
    let yaw0 = rig.m().yaw;
    rig.dummies.0[0].1 = Vec3::new(150.0, 260.0, 0.0); // it steps 60 degrees to his side
    rig.until(60, NONE, &[], |r| r.m().anim.time >= 0.25);
    let before = crate::pawn::wrap180(rig.m().yaw - yaw0).abs();
    assert!(before < 1.0, "no turn before the window: {before}");
    rig.until(60, NONE, &[], |r| r.m().anim.time >= 0.52);
    let turned = crate::pawn::wrap180(rig.m().yaw - yaw0).abs();
    assert!(turned > 20.0 && turned <= 60.0 + 1e-3, "turned {turned} degrees toward it");
}

/// THE USER 2026-11-02: "His aerial hit's hitbox also extends way too far away from him". The bonk's sphere is KH3's (50 cm on
/// the shield's ATK_00 socket track, never more than a metre from his centre line): an enemy close ahead is hit, one 2.5 m
/// ahead is not (the old 160 cm sphere 130 cm ahead reached it).
#[test]
fn the_air_bonks_hit_rides_the_shield_and_reaches_only_what_is_close() {
    let bonk = |enemy: Vec3| {
        let mut rig = Rig::new(&[enemy]);
        rig.bare();
        rig.step(NONE, &[B]);
        rig.run(8, NONE, &[B]);
        assert!(!rig.m().on_ground());
        rig.tap(A, NONE);
        assert!(rig.logged("bonk"), "{:?}", rig.run_state().log);
        let mut reach = 0.0f32;
        for _ in 0..90 {
            if let Some(a) = rig.run_state().shots.iter().find(|a| a.tag == "combo") {
                assert_eq!(a.radius(), K.step_bonk_radius);
                reach = reach.max(horizontal(a.centre - rig.m().position()).length());
            }
            rig.step(NONE, &[]);
        }
        (rig.dummies.1.len(), reach)
    };
    let (near, reach) = bonk(Vec3::new(110.0, 0.0, 0.0));
    assert!(near >= 1, "an enemy close ahead is bonked");
    assert!(reach > 0.0 && reach <= 100.0, "the sphere stays on the shield: {reach} cm from him");
    let (far, _) = bonk(Vec3::new(250.0, 0.0, 0.0));
    assert_eq!(far, 0, "an enemy 2.5 m ahead is out of the bonk's reach");
}

/// (COMBAT-FIX) THE USER 2026-11-06: "Goofy's ground combo hit box also extends out way too far"; the coordinator 2026-11-07:
/// KH3 1:1. Each ground step's shape is KH3's: the push and the sweep the 50 cm SPHERE of w_gu_WepBase's group "None" on
/// M_1301's / M_1302's ATK_00 tracks with its (0, 0, -10) offset turned by the socket, the bash the Trinity Guard pawn's FR_SHIELDGUARDIAN_SHIELDBASH SPHERE 100, 80 ahead
/// and 120 up. The far edge stays within KH3's reach (163 / 179 / 180 cm from his feet; the old 170 - 190 cm spheres 130 cm
/// ahead reached 300 - 320). The dummy's body is 60 cm: an enemy kept 2 m ahead of his feet is hit by all three steps, one
/// kept 2.3 m ahead only by the bash (the shield reaches it up to about 2.23 m, the bash up to 2.4 m), one kept 2.5 m ahead
/// by none (the old spheres reached 3.6 m).
#[test]
fn the_ground_steps_hits_ride_the_shield_and_reach_only_kh3s_distance() {
    let string = |ahead: f32| {
        let mut rig = Rig::new(&[Vec3::new(ahead, 0.0, 0.0)]);
        rig.bare();
        let mut reach = [0.0f32; 3];
        for i in 0..200 {
            let pressing = !rig.logged("shove3") && i % 8 == 0;
            rig.step(NONE, if pressing { &[A] } else { &[] });
            // the enemy stays `ahead` in front of his feet: his travel never changes the gap
            let (me, yaw) = (rig.m().position(), rig.m().yaw);
            rig.dummies.0[0].1 = me + forward(yaw) * ahead;
            for a in rig.run_state().shots.iter().filter(|a| a.tag == "combo" || a.tag == "combo_fin") {
                let k = match a.row.name {
                    "Attack1_Push" => 0,
                    "Attack2_Sweep" => 1,
                    _ => 2,
                };
                let kh3 = if k == 2 { 100.0 } else { 50.0 };
                assert_eq!(a.radius(), kh3, "step {}: KH3's sphere", k + 1);
                reach[k] = reach[k].max(horizontal(a.centre - me).length() + a.radius());
            }
        }
        assert!(rig.logged("shove3"), "{:?}", rig.run_state().log);
        let names: Vec<&str> = rig.dummies.1.iter().map(|h| h.attack_name).collect();
        (names, reach)
    };
    let (near, reach) = string(200.0);
    for (k, kh3) in [163.0, 179.0, 180.0].into_iter().enumerate() {
        assert!(reach[k] > kh3 - 20.0 && reach[k] <= kh3 + 2.0, "step {}: the far edge {} cm from his feet, KH3's {kh3}", k + 1, reach[k]);
    }
    for row in ["Attack1_Push", "Attack2_Sweep", "FR_SHIELDGUARDIAN_SHIELDBASH"] {
        assert!(near.contains(&row), "{row} reaches an enemy 2 m ahead: {near:?}");
    }
    let (mid, _) = string(230.0);
    assert_eq!(mid, ["FR_SHIELDGUARDIAN_SHIELDBASH"], "at 2.3 m only the bash's 100 cm sphere reaches");
    let (far, _) = string(250.0);
    assert!(far.is_empty(), "an enemy 2.5 m ahead is out of every ground step's reach: {far:?}");
}

#[test]
fn a_shove_cancels_into_the_guard_at_any_time_with_no_recovery() {
    let mut rig = Rig::plain();
    rig.tap(A, NONE);
    rig.idle(2);
    assert_eq!(rig.state(), NpcStateId::Pad(COMBO), "the shove's wind-up");
    assert!(rig.m().anim.time < K.step_shove1_hit, "before its hit");
    rig.step(NONE, &[X]);
    assert_eq!(rig.state(), NpcStateId::Pad(GUARD), "X cancels it on the press step");
}

// ------------------------------------------------------------------------------------------------ the bash's end lag
// (THE USER 2026-11-02: "He needs some end lag at the end of his combo so you can't spam it, like Sora.")

/// Mashes A up to the bash and steps to its hit: its end lag has just started.
fn to_the_bash(rig: &mut Rig) {
    for i in 0..240 {
        if rig.logged("shove3") {
            break;
        }
        rig.step(NONE, if i % 8 == 0 { &[A] } else { &[] });
    }
    assert!(rig.logged("shove3"), "{:?}", rig.run_state().log);
    rig.until(60, NONE, &[], |r| r.run_state().combo.end_lag > 0.0);
    assert_eq!(rig.run_state().combo.step, combo::Step::Shove3);
}

fn count(rig: &Rig, w: &str) -> usize {
    rig.run_state().log.iter().filter(|x| **x == w).count()
}

/// The rows follow Sora's rule on KH3's own clip of the bash: the lag ends just inside SGuardShieldBash (which then hands
/// back), and it and its cancel point sit where Sora's ground finisher (kind 4, clip 1304) puts them, at Goofy's rate.
#[test]
fn the_end_lag_rows_follow_soras_finisher_on_the_bashs_own_clip() {
    let bash = shots::clip(K.step_shove3_clip);
    let after_hit = (bash.length - K.step_shove3_hit) / K.step_shove3_rate;
    // a frame inside the clip at least (the hit may come in a frame late), and not much less than it
    assert!(K.combo_end_lag <= after_hit - DT * K.combo_windup &&K.combo_end_lag > after_hit - 0.06, "the lag {} ends just inside the clip's {after_hit} s", K.combo_end_lag);
    let sora = kh_core::combat::tables::attack_kind(4).expect("Sora's ground finisher");
    let hit = kh_core::combat::tables::HIT_WINDOWS.iter().find(|w| w.anim == sora.anim).expect("its hit window");
    let free = sora.length - hit.start;
    let close = sora.length - hit.end;
    assert!(K.combo_end_lag < free && K.combo_end_lag - K.step_shove3_active / K.step_shove3_rate > close, "Sora: {free} s after his hit, {close} s after its window");
    let cancel = (sora.enable_change.expect("its TresEnableChange") - hit.start) / K.step_shove3_rate;
    assert!((K.combo_end_lag_cancel - cancel).abs() < 0.01, "the cancel point {} against Sora's {cancel}", K.combo_end_lag_cancel);
}

/// After the bash's hit an A press cannot start a new string until the lag is over, however often it is pressed; the lag
/// lasts its row, he stays in the bash all through it, and the press made at its end starts the new string at once.
#[test]
fn after_the_bash_an_a_press_cannot_restart_the_string_until_the_end_lag_is_over() {
    let mut rig = Rig::new(&[]);
    to_the_bash(&mut rig);
    let shove1s = count(&rig, "shove1");
    let mut n = 0;
    while rig.run_state().combo.end_lag > 0.0 {
        assert_eq!(rig.state(), NpcStateId::Pad(COMBO), "step {n}: the bash holds him through its lag");
        rig.step(NONE, if n % 2 == 0 { &[A] } else { &[] });
        assert_eq!(count(&rig, "shove1"), shove1s, "step {n}: an A press restarted the string in the end lag");
        n += 1;
        assert!(n < 200, "the lag never ended");
    }
    let want = K.combo_end_lag / DT;
    assert!((n as f32 - want).abs() <= 1.0, "the lag lasted {n} steps, the row says {want}");
    assert_eq!(rig.state(), NpcStateId::Pad(COMBO), "it ends inside the clip");
    rig.until(4, NONE, &[], |r| count(r, "shove1") > shove1s);
}

/// The bash's hit-stop does not count toward the lag (the clip stands still with it).
#[test]
fn the_end_lag_stands_still_through_the_bashs_hit_stop() {
    let mut rig = Rig::new(&[Vec3::new(170.0, 0.0, 0.0)]);
    to_the_bash(&mut rig);
    let mut n = 0;
    while rig.run_state().combo.end_lag > 0.0 {
        rig.step(NONE, &[]);
        n += 1;
        assert!(n < 200, "the lag never ended");
    }
    assert!(rig.run_state().combo.landed.is_some(), "the bash landed");
    let want = (K.combo_end_lag + K.step_shove3_hitstop) / DT;
    assert!((n as f32 - want).abs() <= 2.0, "the lag lasted {n} steps with the hit-stop, the rows say {want}");
}

/// Only the end of the string has the lag: each shove before the bash chains at its link point and has none.
#[test]
fn mid_string_steps_still_chain_at_their_link_points_with_no_end_lag() {
    let mut rig = Rig::new(&[]);
    rig.tap(A, NONE);
    for (now, link, next) in [("shove1", K.step_shove1_link, "shove2"), ("shove2", K.step_shove2_link, "shove3")] {
        assert!(rig.logged(now), "{:?}", rig.run_state().log);
        let mut at = None;
        for i in 0..90 {
            let t = rig.m().anim.time;
            assert_eq!(rig.run_state().combo.end_lag, 0.0, "{now} has no end lag");
            rig.step(NONE, if i % 2 == 0 { &[A] } else { &[] });
            if rig.logged(next) {
                at = Some(t);
                break;
            }
        }
        let t = at.unwrap_or_else(|| panic!("{now} never chained: {:?}", rig.run_state().log));
        assert!(t <= link + 1.5 * DT * 2.0, "{now} chained at clip {t}, its link is {link}");
    }
}

/// Before `combo.end_lag_cancel` nothing cancels the lag: not X (the hop, the guard), not B, not the stick.
#[test]
fn nothing_cancels_the_end_lag_before_its_cancel_point() {
    let mut rig = Rig::plain();
    to_the_bash(&mut rig);
    rig.tap(X, NONE);
    assert_eq!(rig.state(), NpcStateId::Pad(COMBO), "no guard out of the lag's start");
    rig.tap(X, RIGHT);
    assert_eq!(rig.state(), NpcStateId::Pad(COMBO), "no hop either");
    rig.tap(B, NONE);
    assert!(rig.m().on_ground() && rig.state() == NpcStateId::Pad(COMBO), "no jump: {:?}", rig.state());
    while rig.run_state().combo.end_lag > 0.0 {
        rig.step(FWD, &[]);
        if rig.run_state().combo.end_lag > 0.0 {
            assert_eq!(rig.state(), NpcStateId::Pad(COMBO), "the stick does not take him out of the lag");
        }
    }
}

/// Sora's rule: from the cancel point the guard and the hop may cancel the lag (his finisher's TresEnableChange lets the guard
/// and the dodge in), but its clock runs on: no new string, ground or air, out of them until it is over.
#[test]
fn the_guard_and_the_hop_cancel_the_end_lag_from_its_cancel_point_but_no_string_restarts() {
    let guard = |stick: Vec2, want: NpcStateId| {
        let mut rig = Rig::plain();
        to_the_bash(&mut rig);
        rig.until(60, NONE, &[], |r| K.combo_end_lag - r.run_state().combo.end_lag >= K.combo_end_lag_cancel);
        rig.tap(X, stick);
        assert_eq!(rig.state(), want, "X cancels the lag from its cancel point");
        assert!(rig.run_state().combo.end_lag > 0.0, "the lag's clock runs on");
        let shove1s = count(&rig, "shove1");
        while rig.run_state().combo.end_lag > 0.0 {
            rig.tap(A, NONE);
            if rig.run_state().combo.end_lag > 0.0 {
                assert_eq!(count(&rig, "shove1"), shove1s, "no string out of the cancel while the lag runs");
            }
        }
        rig
    };
    let mut rig = guard(NONE, NpcStateId::Pad(GUARD));
    rig.tap(A, NONE);
    assert!(rig.logged("shove1") && count(&rig, "shove1") > 1, "after the lag A starts the string again: {:?}", rig.run_state().log);
    guard(RIGHT, NpcStateId::Pad(HOP));
    // out of the guard B still jumps, but the air bonk waits for the lag too
    let mut rig = Rig::plain();
    to_the_bash(&mut rig);
    rig.until(60, NONE, &[], |r| K.combo_end_lag - r.run_state().combo.end_lag >= K.combo_end_lag_cancel);
    rig.tap(X, NONE);
    rig.step(NONE, &[B]);
    rig.run(4, NONE, &[B]);
    assert!(!rig.m().on_ground(), "B jumps out of the guard");
    let bonks = count(&rig, "bonk");
    rig.tap(A, NONE);
    assert!(rig.run_state().combo.end_lag > 0.0);
    assert_eq!(count(&rig, "bonk"), bonks, "no bonk in the lag");
}

#[test]
fn the_air_bonk_is_one_per_airtime_and_there_is_no_air_guard() {
    let mut rig = Rig::plain();
    rig.step(NONE, &[B]);
    rig.run(14, NONE, &[B]);
    assert!(!rig.m().on_ground());
    rig.step(NONE, &[B, X]);
    assert!(!matches!(rig.state(), NpcStateId::Pad(GUARD)), "no air guard: {:?}", rig.state());
    rig.step(NONE, &[B]);
    rig.tap(A, NONE);
    assert!(rig.logged("bonk"));
    let bonks = rig.run_state().log.iter().filter(|w| **w == "bonk").count();
    rig.tap(A, NONE);
    rig.tap(A, NONE);
    assert_eq!(rig.run_state().log.iter().filter(|w| **w == "bonk").count(), bonks, "one bonk an airtime");
}

/// (THE USER 2026-11-02: "Goofy's big damage should solely come from his guard break attacks from 6.3") mashing A is weak: his
/// string's power is a small share of the Breaker's.
#[test]
fn mashing_a_does_poor_damage_against_the_breaker() {
    let mut mash = Rig::new(&[Vec3::new(170.0, 0.0, 0.0)]);
    mash.bare();
    for i in 0..240 {
        mash.step(NONE, if i % 8 == 0 { &[A] } else { &[] });
    }
    let shoves: Vec<f32> = mash.dummies.1.iter().map(|h| h.info.power).collect();
    assert!(shoves.len() >= 3, "{shoves:?}");
    let mean = shoves.iter().sum::<f32>() / shoves.len() as f32;
    assert!(K.breaker_power >= 8.0 * mean, "the Breaker {} against a mean shove {mean}: the break is where the damage lives", K.breaker_power);
}

/// The parry the last hit was answered with (its event waits for the party's step, or has gone through it).
impl Rig {
    fn answer(&self) -> Option<(Tech, bool)> {
        self.events.iter().chain(self.k().events.iter()).rev().find_map(|e| match e {
            GoofyEvent::Parried { tech, right, .. } => Some((*tech, *right)),
            _ => None,
        })
    }

    /// The Stagger the last parry asked for.
    fn parry_stagger(&self) -> Option<f32> {
        self.events.iter().chain(self.k().events.iter()).rev().find_map(|e| match e {
            GoofyEvent::Stagger { amount, why: seams::StaggerWhy::Deflect | seams::StaggerWhy::Technique, .. } => Some(*amount),
            _ => None,
        })
    }

    /// (THE USER 2026-11-02: "Remove countering with 'A' after a parry technique") A right after the parry: no counter of any
    /// kind; his plain string at most.
    fn a_brings_no_counter(&mut self) {
        let hp_hits = self.dummies.1.len();
        self.run(2, NONE, &[]);
        self.tap(A, NONE);
        for _ in 0..90 {
            assert_ne!(self.state(), NpcStateId::Pad(43), "the old counter state");
            self.step(NONE, &[]);
        }
        let names: Vec<&str> = self.dummies.1[hp_hits..].iter().map(|h| h.attack_name).collect();
        assert!(names.iter().all(|n| *n != "GOOFY_COUNTER"), "no counter landed: {names:?}");
        for gone in ["Counter Bash", "Pin Down", "Goofy Stomp", "Pratfall Toss", "Return Fire", "Ward Burst", "Shield Shove", "Crescendo", "counter_follow", "free_art"] {
            assert!(!self.logged(gone), "{gone}: {:?}", self.run_state().log);
        }
    }
}

// ------------------------------------------------------------------------------------------------ guard and GUARD (4)

#[test]
fn x_raises_the_shield_on_the_press_step_and_he_walks_shield_up() {
    let mut rig = Rig::new(&[]);
    rig.step(NONE, &[X]);
    assert_eq!(rig.state(), NpcStateId::Pad(GUARD));
    assert_eq!(rig.m().anim.clip.name, "SGuardIdle");
    rig.run(60, NONE, &[X]);
    rig.run(60, FWD, &[X]);
    assert_eq!(rig.m().anim.clip.name, "SGuardWalk");
    let v = horizontal(rig.m().cmc.velocity).length();
    assert!((v - K.guard_walk_speed).abs() < 3.0, "shield-up walk {v}");
}

#[test]
fn a_held_guard_blocks_for_a_high_guard_cost_and_no_stagger() {
    let mut rig = Rig::plain();
    rig.run(30, NONE, &[X]); // past every window
    let max = rig.m().status.max_hp() as f32;
    let (hp, before) = (rig.m().status.hp, rig.guard().value);
    let r = rig.hit(AttackClass::Normal, max * 0.1);
    assert!(r.guarded && r.damage_dealt == 0.0, "{r:?}");
    assert_eq!(rig.m().status.hp, hp);
    let drained = before - rig.guard().value;
    assert!((drained - K.guard_drain_share * 0.1).abs() < 0.5, "paid by the hit's power: {drained}");
    assert_eq!(rig.answer(), None, "a block is no parry");
    assert_eq!(rig.state(), NpcStateId::Pad(GUARD_HIT));
    assert_eq!(rig.k().resolve.thirds, 0, "nothing for guarding");
}

#[test]
fn steady_guard_cuts_the_block_by_a_fifth() {
    let mut rig = Rig::plain();
    rig.wear(GA::SteadyGuard);
    rig.run(30, NONE, &[X]);
    let max = rig.m().status.max_hp() as f32;
    let before = rig.guard().value;
    rig.hit(AttackClass::Normal, max * 0.1);
    let drained = before - rig.guard().value;
    assert!((drained - K.guard_drain_share * 0.1 * K.guard_steady_mult).abs() < 0.5, "{drained}");
}

#[test]
fn an_empty_guard_breaks_for_one_and_a_half_seconds_and_he_takes_full_damage() {
    let mut rig = Rig::plain();
    rig.run(30, NONE, &[X]);
    if let Some(g) = rig.mm().status.guard.as_mut() {
        g.value = 1.0;
    }
    let hp = rig.m().status.hp;
    let r = rig.hit(AttackClass::Normal, 30.0);
    assert!(!r.guarded && r.damage_dealt == 30.0, "full damage: {r:?}");
    assert_eq!(rig.m().status.hp, hp - 30);
    assert_eq!(rig.state(), NpcStateId::Pad(GUARD_BREAK));
    assert_eq!(rig.m().anim.clip.name, K.guard_break_clip);
    assert!(rig.events.is_empty() || !rig.events.contains(&GoofyEvent::GuardBreak));
    let n = rig.until(200, NONE, &[], |r| r.state() != NpcStateId::Pad(GUARD_BREAK));
    assert!((n as f32 * DT - K.guard_break_time).abs() < 0.05, "{} s", n as f32 * DT);
    assert!(rig.events.contains(&GoofyEvent::GuardBreak));
}

#[test]
fn x_is_refused_while_guard_is_broken() {
    let mut rig = Rig::plain();
    rig.mm().status.guard.as_mut().unwrap().break_now();
    rig.step(NONE, &[X]);
    assert_ne!(rig.state(), NpcStateId::Pad(GUARD));
}

#[test]
fn a_hit_from_behind_goes_past_the_shield() {
    let mut rig = Rig::plain();
    rig.mm().yaw = 180.0;
    rig.run(30, NONE, &[X]);
    rig.mm().yaw = 180.0;
    let r = rig.hit(AttackClass::Normal, 10.0);
    assert!(!r.guarded, "{r:?}");
}

// ------------------------------------------------------------------------------------------------ Deflect (4.1)

#[test]
fn a_tap_on_the_hit_deflects_for_no_damage_refills_guard_and_staggers() {
    let mut rig = Rig::plain();
    if let Some(g) = rig.mm().status.guard.as_mut() {
        g.value = g.max - 20.0;
    }
    rig.step(NONE, &[X]);
    rig.run(3, NONE, &[]);
    let (hp, before) = (rig.m().status.hp, rig.guard().value);
    let r = rig.hit(AttackClass::Normal, 40.0);
    assert!(r.guarded && r.damage_dealt == 0.0, "{r:?}");
    assert_eq!(rig.m().status.hp, hp);
    assert!(rig.guard().value > before, "it refills a bit on success");
    assert!(rig.logged("deflect"));
    assert_eq!(rig.answer(), Some((Tech::Deflect, true)));
    assert_eq!(rig.parry_stagger(), Some(K.stagger_deflect * parry::rank_mult(rank(rig.m(), Tech::Deflect))));
    assert_eq!(rig.state(), NpcStateId::Pad(PARRY));
    assert_eq!(rig.m().anim.clip.name, K.deflect_clip);
    assert_eq!(rig.run_state().hitstop, K.deflect_hitstop, "the hit-stop sells the hit");
    assert!(rig.run_state().fx.iter().any(|f| f.0 == K.deflect_spark), "sparks");
    assert!(!rig.run_state().sfx.iter().any(|s| s.0 == K.guard_block_se), "AttackReflect's own clang: not doubled");
    assert!(rig.party.goofy_nudge(rig.id).is_some(), "the camera nudge");
}

#[test]
fn the_deflect_window_closes_at_point_two_seconds() {
    assert_eq!(K.deflect_window, 0.2);
    for (steps, deflects) in [(1, true), (11, true), (14, false)] {
        let mut rig = Rig::plain();
        rig.bank(RankLine::Deflect, 0);
        rig.step(NONE, &[X]);
        rig.run(steps - 1, NONE, &[X]);
        let r = rig.hit(AttackClass::Normal, 20.0);
        assert!(r.guarded);
        assert_eq!(rig.logged("deflect"), deflects, "{steps} steps: {:?}", rig.run_state().log);
        assert_eq!(rig.logged("block"), !deflects);
    }
}

#[test]
fn a_tap_commits_him_to_a_short_stance_with_x_let_go() {
    let mut rig = Rig::plain();
    rig.tap(X, NONE);
    assert_eq!(rig.state(), NpcStateId::Pad(GUARD), "the stance holds with X up");
    let n = rig.until(40, NONE, &[], |r| r.state() != NpcStateId::Pad(GUARD));
    assert!((n as f32 * DT + 2.0 * DT - K.deflect_stance).abs() < 0.04, "{n}");
}

#[test]
fn each_mashed_tap_shrinks_the_window_by_thirty_percent() {
    let mut rig = Rig::plain();
    for _ in 0..3 {
        rig.step(NONE, &[X]);
        rig.step(NONE, &[]);
        rig.run(2, NONE, &[]);
    }
    assert_eq!(rig.k().mash, 2, "two taps within 0.25 s of the last");
    let tap = rig.k().tap();
    let m = mods(rig.m());
    let w = parry::window(Tech::Deflect, &tap, false, &m);
    let bare = crate::goofy::parry::window(Tech::Deflect, &crate::goofy::parry::TapFacts::default(), false, &m);
    assert!((w - bare * 0.49).abs() < 1e-4, "{w} against {bare}");
}

#[test]
fn mashing_x_fails_where_one_timed_tap_works() {
    let mut rig = Rig::plain();
    for _ in 0..6 {
        rig.tap(X, NONE);
    }
    rig.run(6, NONE, &[]);
    let r = rig.hit(AttackClass::Normal, 20.0);
    assert!(!rig.logged("deflect"), "a 0.1 s old tap is past the mashed window: {r:?}");
}

#[test]
fn a_pause_clears_the_mash() {
    let mut rig = Rig::plain();
    rig.tap(X, NONE);
    rig.tap(X, NONE);
    assert_eq!(rig.k().mash, 1);
    rig.run(30, NONE, &[]);
    rig.step(NONE, &[X]);
    assert_eq!(rig.k().mash, 0);
}

/// (THE USER 2026-11-02: "Remove countering with 'A' after a parry technique") A after a deflect is his plain string: no counter
/// window, no counter state, no counter hit.
#[test]
fn a_after_a_deflect_is_no_counter_but_his_plain_string() {
    let mut rig = Rig::plain();
    rig.step(NONE, &[X]);
    rig.step(NONE, &[]);
    rig.hit(AttackClass::Normal, 20.0);
    assert!(rig.logged("deflect"));
    rig.a_brings_no_counter();
    assert!(rig.logged("shove1"), "A was his string: {:?}", rig.run_state().log);
}

/// (THE USER 2026-11-02: "MP back on a perfect parry is good") THE NEW MP SOURCE: a perfect parry gives `parry.mp` back; a plain
/// deflect of an attack with its own technique and a late tap Second Deflect forgave give none; during MP Charge it shortens it.
#[test]
fn a_perfect_parry_gives_mp_back_and_a_plain_or_forgiven_one_none() {
    assert_eq!((K.parry_mp, K.parry_mp_charge), (4.0, 0.5));
    let mp_after = |class: AttackClass, stick: Vec2, setup: &dyn Fn(&mut Rig)| {
        let mut rig = Rig::plain();
        setup(&mut rig);
        rig.mm().status.mp = 10;
        rig.step(stick, &[X]);
        rig.run(3, stick, &[]);
        rig.hit(class, 20.0);
        (rig.m().status.mp, rig.answer())
    };
    assert_eq!(mp_after(AttackClass::Normal, NONE, &|_| {}), (10 + K.parry_mp as i32, Some((Tech::Deflect, true))), "a perfect deflect");
    assert_eq!(mp_after(AttackClass::Thrust, FWD, &|_| {}), (10 + K.parry_mp as i32, Some((Tech::PinningGuard, true))), "the right technique");
    assert_eq!(mp_after(AttackClass::Thrust, NONE, &|_| {}), (10, Some((Tech::Deflect, false))), "a plain deflect of a thrust: none");
    // a late tap Second Deflect forgave: Stagger, but no MP
    let mut rig = Rig::plain();
    rig.wear(GA::SecondDeflect);
    rig.bank(RankLine::Deflect, 0);
    rig.mm().status.mp = 10;
    rig.step(NONE, &[X]);
    rig.run(14, NONE, &[X]); // past 0.20, inside the grace
    rig.hit(AttackClass::Normal, 20.0);
    assert!(rig.logged("deflect"), "{:?}", rig.run_state().log);
    assert_eq!(rig.m().status.mp, 10, "forgiven is not perfect");
    // MP Charge: the charge is cut, the bar stays as it is
    let mut rig = Rig::plain();
    rig.mm().status.mp = 0;
    rig.mm().status.mp_charge_left = 10.0;
    rig.step(NONE, &[X]);
    rig.run(3, NONE, &[]);
    rig.hit(AttackClass::Normal, 20.0);
    assert!((rig.m().status.mp_charge_left - (10.0 - K.parry_mp_charge - 4.0 * DT)).abs() < 0.05, "{}", rig.m().status.mp_charge_left);
    // never past the bar
    let mut rig = Rig::plain();
    let max = rig.m().status.max_mp();
    rig.mm().status.mp = max - 1;
    rig.step(NONE, &[X]);
    rig.run(3, NONE, &[]);
    rig.hit(AttackClass::Normal, 20.0);
    assert_eq!(rig.m().status.mp, max);
}

// the right technique fills fastest, a plain deflect least; a normal enemy (100) breaks in four deflects
const _: () = assert!(K.stagger_brace > K.stagger_pin && K.stagger_pin > K.stagger_deflect && K.stagger_deflect * K.stagger_plain_mult < K.stagger_deflect);
const _: () = assert!(K.stagger_deflect * 4.0 >= K.stagger_gauge && K.stagger_deflect * 3.0 < K.stagger_gauge);

/// (THE USER 2026-11-02: "The parries are there to make the enemy's stagger go up fast while taking 0 damage yourself") EACH
/// TECHNIQUE'S STAGGER, through the real pad and judgement: 0 HP each, the technique's own row, a plain deflect half of Deflect's.
#[test]
fn each_technique_fills_its_own_stagger_for_no_damage() {
    assert_eq!((K.stagger_deflect, K.stagger_reflect, K.stagger_ward, K.stagger_hop, K.stagger_pin, K.stagger_slip, K.stagger_brace), (25.0, 30.0, 34.0, 36.0, 40.0, 40.0, 45.0));
    for (class, stick, tech, want) in [(AttackClass::Normal, NONE, Tech::Deflect, K.stagger_deflect), (AttackClass::Thrust, FWD, Tech::PinningGuard, K.stagger_pin), (AttackClass::Projectile, FWD, Tech::ShieldReflect, K.stagger_reflect), (AttackClass::Thrust, NONE, Tech::Deflect, K.stagger_deflect * K.stagger_plain_mult)] {
        let mut rig = Rig::plain();
        for line in [RankLine::Deflect, RankLine::PinningGuard, RankLine::ShieldReflect] {
            rig.bank(line, 0); // rank 1: the rows' own numbers
        }
        let hp = rig.m().status.hp;
        rig.step(stick, &[X]);
        rig.run(3, stick, &[]);
        let r = rig.hit(class, 20.0);
        assert!(r.guarded && r.damage_dealt == 0.0 && rig.m().status.hp == hp, "{tech:?}: 0 damage");
        assert_eq!(rig.answer().map(|a| a.0), Some(tech), "{class:?}");
        assert_eq!(rig.parry_stagger(), Some(want), "{tech:?} against {class:?}");
    }
    // the rule itself, for the ones a tap does not reach here (their own tests run them through the pad)
    let m = Mods { level: 99, ..Mods::default() };
    for t in [Tech::HopCounter, Tech::SlipCounter, Tech::SpellWard, Tech::BullBrace] {
        let p = Parry { tech: t, right: true, aggressive: false, forgiven: false, revenge: false };
        assert_eq!(parry::parry_stagger(&p, 1, &m), t.stagger(), "{t:?}");
    }
    // ranks, Resolve's first pip, Stagger Boost
    let p = Parry { tech: Tech::PinningGuard, right: true, aggressive: false, forgiven: false, revenge: false };
    let pin = |ranks: &[(RankLine, u64)], pips: u8, boost: bool| {
        let mut parts = [0u64; RankLine::ALL.len()];
        for (l, n) in ranks {
            parts[RankLine::ALL.iter().position(|x| x == l).unwrap()] = *n;
        }
        parry::parry_stagger(&p, 1, &Mods { level: 99, ranks: RankBank::from_parts(&parts), pips, stagger_boost: boost, ..Mods::default() })
    };
    assert_eq!(pin(&[], 0, false), 40.0);
    assert_eq!(pin(&[(RankLine::PinningGuard, 1)], 0, false), 40.0 * K.stagger_rank2_mult);
    assert_eq!(pin(&[(RankLine::PinningGuard, 2)], 0, false), 40.0 * K.stagger_rank3_mult);
    assert!((pin(&[], 1, true) - 40.0 * K.resolve_pip1_stagger * K.stagger_boost_mult).abs() < 1e-4);
    // a normal enemy (100) breaks in four deflects (the const above), three of a perilous attack's right technique
    for t in [Tech::PinningGuard, Tech::SlipCounter, Tech::BullBrace, Tech::HopCounter, Tech::SpellWard] {
        assert!(t.stagger() * 3.0 >= K.stagger_gauge, "{t:?}");
    }
}

// ------------------------------------------------------------------------------------------------ the techniques (4.3, 6.1)

/// Answers an attack from the plain rig with a tap of `stick` `lead` steps before impact.
fn tap_answer(class: AttackClass, stick: Vec2, lead: usize, setup: impl Fn(&mut Rig)) -> Rig {
    let mut rig = Rig::plain();
    setup(&mut rig);
    rig.step(stick, &[X]);
    rig.run(lead, stick, &[]);
    rig.hit(class, 20.0);
    rig
}

#[test]
fn pinning_guard_right_answer_fills_its_stagger_and_a_full_pip() {
    let mut rig = tap_answer(AttackClass::Thrust, FWD, 3, |_| {});
    assert!(rig.logged("pinning_guard"));
    assert_eq!(rig.answer(), Some((Tech::PinningGuard, true)));
    assert_eq!(rig.k().resolve.thirds, 3);
    assert!(rig.stagger_total() == 0.0, "the event drains at the next step");
    assert_eq!(rig.parry_stagger(), Some(K.stagger_pin * parry::rank_mult(rank(rig.m(), Tech::PinningGuard))));
    rig.a_brings_no_counter();
}

#[test]
fn pinning_guard_wrong_answer_is_a_plain_deflect_at_half_stagger() {
    let mut rig = tap_answer(AttackClass::Thrust, NONE, 3, |_| {});
    assert!(rig.logged("plain_deflect"));
    assert_eq!(rig.answer(), Some((Tech::Deflect, false)), "a plain deflect");
    assert_eq!(rig.k().resolve.thirds, 1, "a third of a pip");
    rig.idle(1);
    let want = K.stagger_deflect * K.stagger_plain_mult * parry::rank_mult(rank(rig.m(), Tech::Deflect));
    assert!((rig.stagger_total() - want).abs() < 1e-3, "{}", rig.stagger_total());
}

#[test]
fn pinning_guard_not_learned_is_a_plain_aggressive_deflect() {
    let mut rig = tap_answer(AttackClass::Thrust, FWD, 3, |r| r.level(RankLine::PinningGuard.learn_level() - 1));
    assert!(rig.logged("plain_deflect"));
    assert_eq!(rig.answer(), Some((Tech::Deflect, false)));
    rig.idle(1);
    let plain = K.stagger_deflect * K.stagger_plain_mult * parry::rank_mult(rank(rig.m(), Tech::Deflect)) * K.deflect_aggressive_stagger;
    assert!((rig.stagger_total() - plain).abs() < 1e-3, "a plain aggressive deflect's Stagger, not Pinning Guard's: {}", rig.stagger_total());
    assert!(rig.stagger_total() < K.stagger_pin * parry::rank_mult(rank(rig.m(), Tech::PinningGuard).max(1)));
}

#[test]
fn pinning_guard_past_its_window_is_a_block() {
    let mut rig = Rig::plain();
    rig.step(FWD, &[X]);
    rig.run(18, FWD, &[X]);
    rig.hit(AttackClass::Thrust, 20.0);
    assert!(!rig.logged("pinning_guard"));
    assert!(rig.logged("block"));
}

/// (THE USER 2026-11-02: the counters' rank 3 follow-ups went with the counters) rank 3 fills more Stagger instead: x1.5 in all,
/// rank 2 x1.25.
#[test]
fn rank_three_fills_half_again_the_stagger_and_rank_two_a_quarter() {
    let at = |n: u64| {
        let mut rig = Rig::plain();
        rig.bank(RankLine::PinningGuard, n);
        rig.step(FWD, &[X]);
        rig.run(3, FWD, &[]);
        rig.hit(AttackClass::Thrust, 20.0);
        assert_eq!(rank(rig.m(), Tech::PinningGuard) as u64, n + 1);
        rig.parry_stagger().unwrap()
    };
    assert_eq!((at(0), at(1), at(2)), (K.stagger_pin, K.stagger_pin * K.stagger_rank2_mult, K.stagger_pin * K.stagger_rank3_mult));
    assert_eq!((K.stagger_rank2_mult, K.stagger_rank3_mult), (1.25, 1.5));
}

/// Shield Reflect stops the shot on his shield for 0 damage and staggers the shooter; nothing is sent back, and A is no Return
/// Fire (THE USER 2026-11-02).
#[test]
fn shield_reflect_right_answer_stops_the_shot_and_staggers_the_shooter() {
    let mut rig = tap_answer(AttackClass::Projectile, FWD, 3, |_| {});
    assert!(rig.logged("shield_reflect"));
    assert_eq!(rig.answer(), Some((Tech::ShieldReflect, true)));
    assert_eq!(rig.k().resolve.thirds, 1, "not perilous: a third");
    assert_eq!(rig.parry_stagger(), Some(K.stagger_reflect * parry::rank_mult(rank(rig.m(), Tech::ShieldReflect))));
    rig.a_brings_no_counter();
    assert!(rig.party.projectiles.iter().all(|p| p.batted.is_none()), "no shot flies back");
}

#[test]
fn shield_reflect_with_a_neutral_stick_is_plain_and_not_learned_is_plain() {
    let rig = tap_answer(AttackClass::Projectile, NONE, 3, |_| {});
    assert!(rig.logged("plain_deflect") && !rig.logged("shield_reflect"));
    let rig = tap_answer(AttackClass::Projectile, FWD, 3, |r| r.level(4));
    assert!(rig.logged("plain_deflect") && !rig.logged("shield_reflect"));
}

#[test]
fn spell_ward_is_a_hold_through_the_cast_then_a_re_press_on_impact() {
    let mut rig = Rig::plain();
    rig.run(24, NONE, &[X]); // 0.4 s through the cast
    rig.run(2, NONE, &[]); // released
    rig.step(NONE, &[X]); // the re-press
    rig.run(2, NONE, &[X]);
    assert_eq!(rig.k().mash, 0, "exempt from the mash penalty");
    rig.hit(AttackClass::Magic, 20.0);
    assert!(rig.logged("spell_ward"), "{:?}", rig.run_state().log);
    assert_eq!(rig.answer(), Some((Tech::SpellWard, true)));
    assert_eq!(rig.parry_stagger(), Some(K.stagger_ward * parry::rank_mult(rank(rig.m(), Tech::SpellWard))));
    rig.a_brings_no_counter();
}

#[test]
fn spell_ward_wrong_answer_a_plain_tap_is_plain_and_too_short_a_hold_is_plain() {
    let rig = tap_answer(AttackClass::Magic, NONE, 3, |_| {});
    assert!(rig.logged("plain_deflect") && !rig.logged("spell_ward"));
    let mut rig = Rig::plain();
    rig.run(8, NONE, &[X]); // under ward.hold_min
    rig.run(2, NONE, &[]);
    rig.step(NONE, &[X]);
    rig.run(1, NONE, &[]);
    rig.hit(AttackClass::Magic, 20.0);
    assert!(!rig.logged("spell_ward"));
}

#[test]
fn spell_ward_not_learned_cannot_be_used() {
    let mut rig = Rig::plain();
    rig.level(RankLine::SpellWard.learn_level() - 1);
    rig.run(24, NONE, &[X]);
    rig.run(2, NONE, &[]);
    rig.step(NONE, &[X]);
    rig.run(1, NONE, &[]);
    rig.hit(AttackClass::Magic, 20.0);
    assert!(!rig.logged("spell_ward"));
}

#[test]
fn bull_brace_held_away_before_impact_answers_a_heavy() {
    let mut rig = Rig::plain();
    rig.run(4, BACK, &[X]); // the first step reads the stick away from the enemy
    assert_eq!(rig.state(), NpcStateId::Pad(BRACE), "{:?}", rig.run_state().log);
    rig.run(14, BACK, &[X]);
    let r = rig.hit(AttackClass::Heavy, 40.0);
    assert!(r.guarded, "{r:?}");
    assert!(rig.logged("bull_brace"));
    assert_eq!(rig.answer(), Some((Tech::BullBrace, true)));
    assert_eq!(rig.k().resolve.thirds, 3);
    assert_eq!(rig.parry_stagger(), Some(K.stagger_brace * parry::rank_mult(rank(rig.m(), Tech::BullBrace))), "the most of all");
    assert_eq!(r.damage_dealt, 0.0, "0 damage");
    rig.a_brings_no_counter();
}

#[test]
fn bull_brace_set_too_late_lets_the_heavy_through_and_rank_two_is_wider() {
    let late = |rank: u64| {
        let mut rig = Rig::plain();
        rig.bank(RankLine::BullBrace, rank);
        rig.run(6, BACK, &[X]); // 0.1 s set
        rig.hit(AttackClass::Heavy, 40.0)
    };
    assert!(!late(0).guarded, "0.1 s is under rank 1's set time");
    assert!(late(1).guarded, "rank 2's window is wider");
}

#[test]
fn a_heavy_goes_through_guard_and_a_plain_tap_and_a_failed_tap_drains_guard() {
    let mut rig = Rig::plain();
    rig.run(30, NONE, &[X]);
    assert!(!rig.hit(AttackClass::Heavy, 30.0).guarded, "unblockable");
    let mut rig = Rig::plain();
    rig.step(NONE, &[X]);
    rig.step(NONE, &[]);
    let before = rig.guard().value;
    let r = rig.hit(AttackClass::Heavy, 30.0);
    assert!(!r.guarded && r.damage_dealt == 30.0, "{r:?}");
    assert!(rig.logged("parry_failed"));
    assert!((before - rig.guard().value - K.guard_fail_drain).abs() < 1.0);
}

#[test]
fn bull_brace_not_learned_walks_back_and_the_heavy_lands() {
    let mut rig = Rig::plain();
    rig.level(RankLine::BullBrace.learn_level() - 1);
    rig.run(20, BACK, &[X]);
    assert_eq!(rig.state(), NpcStateId::Pad(GUARD));
    assert!(!rig.hit(AttackClass::Heavy, 30.0).guarded);
}

#[test]
fn a_grab_goes_through_guard_and_slip_answers_it_from_a_hop_on_its_closing() {
    let mut rig = Rig::plain();
    rig.run(30, NONE, &[X]);
    assert!(!rig.hit(AttackClass::Grab, 20.0).guarded, "a guard does not stop a grab");
    let mut rig = Rig::plain();
    rig.step(RIGHT, &[X]);
    assert_eq!(rig.state(), NpcStateId::Pad(HOP));
    rig.run(4, RIGHT, &[]);
    let r = rig.hit(AttackClass::Grab, 20.0);
    assert!(!r.accepted, "the hop's invincibility: {r:?}");
    assert!(rig.logged("slip_counter"));
    assert_eq!(rig.answer(), Some((Tech::SlipCounter, true)));
    assert_eq!(rig.parry_stagger(), Some(K.stagger_slip * parry::rank_mult(rank(rig.m(), Tech::SlipCounter))));
    assert_eq!(rig.k().resolve.thirds, 3);
    rig.a_brings_no_counter();
}

#[test]
fn slip_counter_is_judged_from_a_telegraph_that_closes_with_no_hit() {
    let mut rig = Rig::plain();
    rig.telegraph(AttackClass::Grab, true);
    rig.run(4, NONE, &[]);
    rig.step(RIGHT, &[X]);
    rig.run(8, RIGHT, &[]);
    rig.no_telegraphs();
    rig.idle(1);
    assert!(rig.logged("slip_counter"));
    assert_eq!(rig.answer(), Some((Tech::SlipCounter, true)));
}

#[test]
fn slip_counter_wrong_answer_jumping_a_grab_gives_nothing() {
    let mut rig = Rig::plain();
    rig.telegraph(AttackClass::Grab, true);
    rig.run(2, NONE, &[B]);
    rig.run(12, NONE, &[B]);
    rig.no_telegraphs();
    rig.idle(1);
    assert!(!rig.logged("slip_counter") && rig.answer().is_none());
}

#[test]
fn slip_counter_not_learned_is_a_plain_dodge() {
    let mut rig = Rig::plain();
    rig.level(RankLine::SlipCounter.learn_level() - 1);
    rig.step(RIGHT, &[X]);
    rig.run(4, RIGHT, &[]);
    assert!(!rig.hit(AttackClass::Grab, 20.0).accepted, "the hop still dodges");
    assert!(!rig.logged("slip_counter") && rig.answer().is_none(), "no parry");
}

/// Hop Counter: the sweep jumped is 0 damage and its Stagger; A in the air is his plain air bonk, no Goofy Stomp (THE USER
/// 2026-11-02).
#[test]
fn hop_counter_jumps_a_sweep_for_its_stagger_and_a_in_the_air_is_the_plain_bonk() {
    let mut rig = Rig::plain();
    rig.telegraph(AttackClass::Sweep, true);
    rig.run(16, NONE, &[B]);
    assert!(!rig.m().on_ground());
    rig.no_telegraphs();
    rig.step(NONE, &[]);
    assert!(rig.logged("hop_counter"), "{:?}", rig.run_state().log);
    assert_eq!(rig.answer(), Some((Tech::HopCounter, true)));
    assert_eq!(rig.parry_stagger(), Some(K.stagger_hop * parry::rank_mult(rank(rig.m(), Tech::HopCounter))));
    assert_eq!(rig.k().resolve.thirds, 3);
    rig.tap(A, NONE);
    assert!(!rig.logged("Goofy Stomp") && rig.state() != NpcStateId::Pad(43), "{:?}", rig.run_state().log);
}

#[test]
fn a_real_sweep_that_reaches_him_in_the_air_is_jumped_not_taken() {
    // (GF-INGAME, Margit) a game's swing box does not know his jump: the host reports the hit all the same
    let mut rig = Rig::plain();
    rig.telegraph(AttackClass::Sweep, true);
    rig.run(16, NONE, &[B]);
    assert!(!rig.m().on_ground());
    assert!(!rig.hit(AttackClass::Sweep, 20.0).accepted, "over it");
    rig.no_telegraphs();
    rig.step(NONE, &[]);
    assert!(rig.logged("hop_counter"), "{:?}", rig.run_state().log);
    // a jump just begun has not cleared it
    let mut rig = Rig::plain();
    rig.telegraph(AttackClass::Sweep, true);
    rig.run(2, NONE, &[B]);
    assert!(rig.hit(AttackClass::Sweep, 20.0).accepted);
}

#[test]
fn a_grab_closing_on_his_hop_tells_the_host_to_refuse_the_body() {
    // (GF-INGAME, Margit) a grab is the game's throw and never asks the kit
    let mut rig = Rig::plain();
    rig.telegraph(AttackClass::Grab, true);
    rig.step(NONE, &[]);
    assert!(!rig.k().slip_open, "no hop");
    rig.step(RIGHT, &[X]);
    rig.run(4, RIGHT, &[]);
    assert!(rig.k().slip_open);
    rig.no_telegraphs();
    rig.step(RIGHT, &[]);
    assert!(!rig.k().slip_open);
}

#[test]
fn hop_counter_fails_when_the_sweep_landed_or_he_stayed_down_or_it_was_far() {
    // a hit landed from it (he was not off the ground long enough: over it, the hit is no hit)
    let mut rig = Rig::plain();
    rig.telegraph(AttackClass::Sweep, true);
    rig.run(2, NONE, &[B]);
    rig.hit(AttackClass::Sweep, 20.0);
    rig.no_telegraphs();
    rig.idle(1);
    assert!(!rig.logged("hop_counter"));
    // on the ground
    let mut rig = Rig::plain();
    rig.telegraph(AttackClass::Sweep, true);
    rig.run(16, NONE, &[]);
    rig.no_telegraphs();
    rig.idle(1);
    assert!(!rig.logged("hop_counter"));
    // an attacker too far away to mean him
    let mut rig = Rig::new(&[Vec3::new(4000.0, 0.0, 0.0)]);
    rig.bare();
    rig.telegraph(AttackClass::Sweep, true);
    rig.run(16, NONE, &[B]);
    rig.no_telegraphs();
    rig.idle(1);
    assert!(!rig.logged("hop_counter"));
}

#[test]
fn hop_counter_not_learned_gives_nothing_and_a_tap_on_a_sweep_is_plain() {
    let mut rig = Rig::plain();
    rig.level(RankLine::HopCounter.learn_level() - 1);
    rig.telegraph(AttackClass::Sweep, true);
    rig.run(16, NONE, &[B]);
    rig.no_telegraphs();
    rig.idle(1);
    assert!(!rig.logged("hop_counter"));
    let rig = tap_answer(AttackClass::Sweep, NONE, 3, |_| {});
    assert!(rig.logged("plain_deflect"));
}

/// The Stagger of each deflect of a string, Resolve held at 0 so its pip does not scale it.
fn string_staggers(rig: &mut Rig, n: usize) -> Vec<f32> {
    let mut out = Vec::new();
    for _ in 0..n {
        rig.run(20, NONE, &[]);
        rig.s().core.resolve = Default::default();
        rig.step(NONE, &[X]);
        rig.step(NONE, &[]);
        rig.hit(AttackClass::Normal, 20.0);
        out.push(rig.parry_stagger().unwrap());
    }
    out
}

/// (THE USER 2026-11-02: Crescendo's payoff moved into the string) RHYTHM DEFLECT: each deflect in time of a string fills more
/// Stagger than the last, from the second on (28, +6 each, at most 46 at rank 1); no counter at the end.
#[test]
fn rhythm_deflect_fills_more_stagger_with_each_deflect_in_time() {
    let mut rig = Rig::plain();
    rig.bank(RankLine::RhythmDeflect, 0);
    rig.bank(RankLine::Deflect, 0);
    let s = string_staggers(&mut rig, 6);
    assert_eq!(rig.k().string, 6);
    assert_eq!(s, vec![K.stagger_deflect, 28.0, 34.0, 40.0, 46.0, 46.0], "a plain Deflect, then the string's own, capped");
    assert_eq!((K.rhythm_stagger, K.rhythm_stagger_per, K.rhythm_stagger_max), (28.0, 6.0, 46.0));
    assert!(s.iter().take(5).sum::<f32>() >= K.stagger_gauge_large, "a five-hit string breaks a large enemy");
    rig.a_brings_no_counter();
    // rank 3 of Rhythm Deflect scales the string's numbers
    let mut rig = Rig::plain();
    rig.bank(RankLine::RhythmDeflect, 2);
    rig.bank(RankLine::Deflect, 0);
    assert_eq!(string_staggers(&mut rig, 2)[1], 28.0 * K.stagger_rank3_mult);
}

#[test]
fn rhythm_deflect_not_learned_keeps_plain_deflects_and_a_long_gap_ends_the_string() {
    let mut rig = Rig::plain();
    rig.level(RankLine::RhythmDeflect.learn_level() - 1);
    rig.bank(RankLine::Deflect, 0);
    let s = string_staggers(&mut rig, 3);
    assert_eq!(s, vec![K.stagger_deflect; 3], "every one a plain Deflect");
    let mut rig = Rig::plain();
    rig.step(NONE, &[X]);
    rig.step(NONE, &[]);
    rig.hit(AttackClass::Normal, 20.0);
    rig.run(100, NONE, &[]);
    assert_eq!(rig.k().string, 0);
}

// ------------------------------------------------------------------------------------------------ no counters (6.2)

/// (THE USER 2026-11-02: "Remove countering with 'A' after a parry technique") NO TECHNIQUE OPENS A COUNTER: after each of the
/// eight, A is no counter of any kind; no Art becomes free; the counters' rows and state are gone.
#[test]
fn no_technique_opens_a_counter_on_the_pad() {
    // Deflect, Pinning Guard, Shield Reflect, a plain deflect: a tap
    for (class, stick) in [(AttackClass::Normal, NONE), (AttackClass::Thrust, FWD), (AttackClass::Projectile, FWD), (AttackClass::Thrust, NONE)] {
        let mut rig = tap_answer(class, stick, 3, |_| {});
        assert!(rig.answer().is_some(), "{class:?}");
        rig.a_brings_no_counter();
    }
    // Spell Ward
    let mut rig = Rig::plain();
    rig.run(24, NONE, &[X]);
    rig.run(2, NONE, &[]);
    rig.step(NONE, &[X]);
    rig.run(2, NONE, &[X]);
    rig.hit(AttackClass::Magic, 20.0);
    assert_eq!(rig.answer().map(|a| a.0), Some(Tech::SpellWard));
    rig.a_brings_no_counter();
    // Bull Brace
    let mut rig = Rig::plain();
    rig.run(18, BACK, &[X]);
    rig.hit(AttackClass::Heavy, 40.0);
    assert_eq!(rig.answer().map(|a| a.0), Some(Tech::BullBrace));
    rig.a_brings_no_counter();
    // Slip Counter
    let mut rig = Rig::plain();
    rig.step(RIGHT, &[X]);
    rig.run(4, RIGHT, &[]);
    rig.hit(AttackClass::Grab, 20.0);
    assert_eq!(rig.answer().map(|a| a.0), Some(Tech::SlipCounter));
    rig.a_brings_no_counter();
    // Hop Counter
    let mut rig = Rig::plain();
    rig.telegraph(AttackClass::Sweep, true);
    rig.run(16, NONE, &[B]);
    rig.no_telegraphs();
    rig.step(NONE, &[]);
    assert_eq!(rig.answer().map(|a| a.0), Some(Tech::HopCounter));
    rig.a_brings_no_counter();
    // Rhythm Deflect's string
    let mut rig = Rig::plain();
    string_staggers(&mut rig, 4);
    rig.a_brings_no_counter();
    // the HUD has no counter to show; the Arts are never free
    let hud = rig.party.goofy_hud(rig.id).unwrap();
    assert_eq!(hud, crate::goofy::GoofyHud { thirds: hud.thirds, in_zone: hud.in_zone, hop_cd: hud.hop_cd });
}

// ------------------------------------------------------------------------------------------------ Resolve (7)

#[test]
fn resolve_fills_from_clean_play_and_empties_when_he_is_hit() {
    let mut rig = Rig::plain();
    rig.s().core.resolve.thirds = 7;
    rig.step(NONE, &[X]);
    rig.step(NONE, &[]);
    rig.hit(AttackClass::Normal, 20.0);
    assert_eq!(rig.k().resolve.thirds, 8, "a third for a deflect");
    rig.run(40, NONE, &[]);
    rig.hit(AttackClass::Normal, 20.0);
    assert_eq!(rig.k().resolve.thirds, 0, "it empties if he gets hit");
}

#[test]
fn a_guarded_hit_neither_fills_nor_empties_resolve() {
    let mut rig = Rig::plain();
    rig.s().core.resolve.thirds = 4;
    rig.run(30, NONE, &[X]);
    rig.hit(AttackClass::Normal, 20.0);
    assert_eq!(rig.k().resolve.thirds, 4);
}

#[test]
fn resolve_keeper_takes_one_pip_only() {
    let mut rig = Rig::plain();
    rig.wear(GA::ResolveKeeper);
    rig.s().core.resolve.thirds = 8;
    rig.hit(AttackClass::Normal, 20.0);
    assert_eq!(rig.k().resolve.thirds, 5);
}

#[test]
fn zone_extender_lasts_through_one_hit() {
    let mut rig = Rig::plain();
    rig.wear(GA::ZoneExtender);
    rig.s().core.resolve.thirds = RESOLVE_FULL;
    rig.hit(AttackClass::Normal, 10.0);
    assert!(rig.k().resolve.in_zone(), "through one hit");
    rig.run(120, NONE, &[]);
    rig.hit(AttackClass::Normal, 10.0);
    assert_eq!(rig.k().resolve.thirds, 0);
}

#[test]
fn one_pip_widens_the_window_and_raises_the_parrys_stagger() {
    let mut rig = Rig::plain();
    rig.bank(RankLine::Deflect, 0);
    let bare = parry::window(Tech::Deflect, &parry::TapFacts::default(), false, &mods(rig.m()));
    rig.s().core.resolve.thirds = 3;
    let m = mods(rig.m());
    assert!((parry::window(Tech::Deflect, &parry::TapFacts::default(), false, &m) - bare - K.resolve_pip1_window).abs() < 1e-5);
    let p = Parry { tech: Tech::Deflect, right: true, aggressive: false, forgiven: false, revenge: false };
    assert!((parry::parry_stagger(&p, 1, &m) - K.stagger_deflect * K.resolve_pip1_stagger).abs() < 1e-4, "+10 % Stagger (it was +10 % counter damage)");
}

#[test]
fn two_pips_refill_more_guard() {
    let refund = |thirds: u8| {
        let mut rig = Rig::plain();
        rig.s().core.resolve.thirds = thirds;
        if let Some(g) = rig.mm().status.guard.as_mut() {
            g.value = 50.0;
        }
        rig.step(NONE, &[X]);
        rig.step(NONE, &[]);
        let before = rig.guard().value;
        rig.hit(AttackClass::Normal, 20.0);
        rig.guard().value - before
    };
    assert!((refund(6) - refund(0) - K.resolve_pip2_refund).abs() < 0.01);
}

#[test]
fn in_the_zone_every_deflect_is_the_right_technique_with_its_stagger() {
    for (class, tech) in [(AttackClass::Thrust, Tech::PinningGuard), (AttackClass::Grab, Tech::SlipCounter), (AttackClass::Heavy, Tech::BullBrace), (AttackClass::Sweep, Tech::HopCounter), (AttackClass::Magic, Tech::SpellWard), (AttackClass::Projectile, Tech::ShieldReflect)] {
        let mut rig = tap_answer(class, NONE, 3, |r| {
            r.s().core.resolve.thirds = RESOLVE_FULL;
            r.level(1);
        });
        assert_eq!(rig.answer(), Some((tech, true)), "{class:?}");
        assert_eq!(rig.parry_stagger(), Some(tech.stagger() * K.resolve_pip1_stagger), "{tech:?}: its own Stagger (rank 1 at level 1), Resolve's pip");
        rig.a_brings_no_counter();
    }
}

#[test]
fn the_zone_raises_breakers_by_half() {
    let mut rig = Rig::plain();
    assert_eq!(seams::breaker_mult(rig.s()), 1.0);
    rig.s().core.resolve.thirds = RESOLVE_FULL;
    assert_eq!(seams::breaker_mult(rig.s()), K.resolve_zone_breaker);
}

#[test]
fn an_aggressive_deflect_gains_stagger_and_tightens_the_window() {
    let mut rig = tap_answer(AttackClass::Normal, FWD, 3, |_| {});
    assert!(rig.logged("deflect"));
    rig.idle(1);
    let mut plain = tap_answer(AttackClass::Normal, NONE, 3, |_| {});
    plain.idle(1);
    assert!((rig.stagger_total() / plain.stagger_total() - K.deflect_aggressive_stagger).abs() < 1e-3);
    let m = mods(rig.m());
    let t = parry::TapFacts::default();
    assert!(parry::window(Tech::Deflect, &t, true, &m) < parry::window(Tech::Deflect, &t, false, &m));
    let plus = Mods { forward_plus: true, ..m };
    assert!(parry::window(Tech::Deflect, &t, true, &plus) > parry::window(Tech::Deflect, &t, true, &m), "Forward Deflect+ loses less");
}

// ------------------------------------------------------------------------------------------------ abilities (11.3)

#[test]
fn second_deflect_forgives_one_late_tap_per_string() {
    let mut rig = Rig::plain();
    rig.wear(GA::SecondDeflect);
    rig.bank(RankLine::Deflect, 0);
    rig.step(NONE, &[X]);
    rig.run(14, NONE, &[X]); // past 0.20, inside the grace
    rig.hit(AttackClass::Normal, 20.0);
    assert!(rig.logged("deflect"));
    assert!(rig.k().second_deflect_used);
    rig.run(20, NONE, &[]);
    rig.step(NONE, &[X]);
    rig.run(14, NONE, &[X]);
    rig.hit(AttackClass::Normal, 20.0);
    assert!(rig.logged("block"), "one per string");
}

#[test]
fn revenge_bash_turns_the_next_block_after_a_guard_break_into_a_deflect() {
    let mut rig = Rig::plain();
    rig.wear(GA::RevengeBash);
    if let Some(g) = rig.mm().status.guard.as_mut() {
        g.value = 1.0;
    }
    rig.run(30, NONE, &[X]);
    rig.s().core.tap_age = 9.0;
    if let Some(g) = rig.mm().status.guard.as_mut() {
        g.value = 1.0;
    }
    rig.hit(AttackClass::Normal, 30.0);
    assert!(rig.k().revenge);
    rig.mm().status.guard.as_mut().unwrap().refill();
    rig.until(200, NONE, &[], |r| r.state() != NpcStateId::Pad(GUARD_BREAK));
    rig.run(30, NONE, &[X]);
    rig.hit(AttackClass::Normal, 20.0);
    assert!(rig.logged("deflect") && !rig.k().revenge);
}

#[test]
fn iron_posture_refills_guard_faster() {
    let refill = |worn: bool| {
        let mut rig = Rig::plain();
        if worn {
            rig.wear(GA::IronPosture);
        }
        if let Some(g) = rig.mm().status.guard.as_mut() {
            g.value = 20.0;
            g.since_use = 99.0;
        }
        rig.run(30, NONE, &[]);
        rig.guard().value
    };
    assert!(refill(true) > refill(false));
}

#[test]
fn second_wind_restores_guard_once_under_a_quarter_hp() {
    let mut rig = Rig::plain();
    rig.wear(GA::SecondWind);
    if let Some(g) = rig.mm().status.guard.as_mut() {
        g.value = 10.0;
    }
    let max = rig.m().status.max_hp();
    rig.mm().status.hp = max / 5;
    rig.idle(1);
    let after = rig.guard().value;
    assert!(after > 10.0 + 0.4 * rig.guard().max, "{after}");
    if let Some(g) = rig.mm().status.guard.as_mut() {
        g.value = 10.0;
    }
    rig.idle(1);
    assert!(rig.guard().value < 20.0, "once, until his HP has risen again");
}

#[test]
fn danger_sense_and_loud_mouth_answer_their_seams() {
    let mut rig = Rig::plain();
    assert_eq!(seams::danger_lead(rig.s()), 0.0);
    assert!(!seams::danger_names_kind(rig.s()), "the plain red \"!\" never says which");
    assert_eq!(seams::threat_mult(rig.s()), K.threat_base_mult);
    rig.wear(GA::DangerSense);
    rig.wear(GA::LoudMouth);
    assert_eq!(seams::danger_lead(rig.s()), K.danger_lead);
    assert!(seams::danger_names_kind(rig.s()), "Danger Sense names the kind");
    assert_eq!(seams::threat_mult(rig.s()), K.threat_base_mult * K.threat_loud_mouth);
}

#[test]
fn ally_guard_absorbs_a_blow_on_a_near_friend_once_per_ten_seconds() {
    let mut rig = Rig::plain();
    rig.wear(GA::AllyGuard);
    let don = rig.party.spawn(Chara::Donald, Vec3::new(0.0, 120.0, 0.0), 0.0);
    rig.idle(1);
    let hp = rig.party.member(don).unwrap().status.hp;
    let mut h = HitRecord::empty(TargetId::NONE);
    h.damage = 20.0;
    h.attacker = HitAttacker { position: ENEMY, yaw: 180.0, radius: 50.0, valid: true };
    h.attack_info.attacker = Some(T);
    let r = rig.party.receive_hit(don, &h);
    assert!(r.guarded && !r.accepted, "{r:?}");
    assert_eq!(rig.party.member(don).unwrap().status.hp, hp, "the friend takes nothing");
    assert!(rig.logged("ally_guard"));
    assert_eq!(rig.state(), NpcStateId::Pad(PARRY), "Goofy parries for him");
    assert_eq!(rig.k().ally_cd, K.ally_guard_cooldown);
    let again = rig.party.receive_hit(don, &h);
    assert!(!again.guarded, "once per 10 s");
    assert!(rig.party.member(don).unwrap().status.hp < hp);
}

/// (THE USER 2026-11-02: "Goofy's big damage should solely come from his guard break attacks from 6.3. The parries are there to
/// make the enemy's stagger go up fast while taking 0 damage yourself") THE LOOP ON THE PAD: deflects for 0 damage fill the
/// gauge, the enemy breaks, Y is Goofy Breaker and it lands at its 8.0x: his big hit.
#[test]
fn deflects_break_the_enemy_and_y_lands_goofy_breaker() {
    assert_eq!(K.breaker_power, 8.0);
    let mut rig = Rig::plain();
    rig.bank(RankLine::Deflect, 0);
    let hp = rig.m().status.hp;
    let mut deflects = 0;
    while !rig.events.iter().any(|e| matches!(e, GoofyEvent::Broken { target: T })) {
        assert!(deflects < 6, "four deflects at most: {:?}", rig.party.gof.book.summary());
        rig.run(40, NONE, &[]);
        rig.step(NONE, &[X]);
        rig.step(NONE, &[]);
        assert!(rig.hit(AttackClass::Normal, 20.0).guarded);
        rig.idle(1);
        deflects += 1;
    }
    assert!(deflects <= 4, "{deflects}");
    assert_eq!(rig.m().status.hp, hp, "0 damage all the way");
    assert!(rig.dummies.1.is_empty(), "no damage dealt before the Breaker: no counters");
    rig.until(60, NONE, &[], |r| super::super::goofy_breaker::breaker_offered(r.m()));
    rig.tap(PadButton::FaceTop, NONE);
    rig.until(240, NONE, &[], |r| r.dummies.1.iter().any(|h| h.attack_name == "GOOFY_BREAKER"));
    let slam = rig.dummies.1.iter().find(|h| h.attack_name == "GOOFY_BREAKER").copied().unwrap();
    assert!(slam.info.power >= K.breaker_power, "{}", slam.info.power);
}

/// (THE USER 2026-11-02: "Remove countering with 'A' after a parry technique") Counter Chain+ is Stagger Boost: the same slot,
/// AP and level, the list keeps its 17; an old save's entry is read as it, owned and worn as it was.
#[test]
fn counter_chain_plus_became_stagger_boost_and_old_saves_keep_it() {
    assert_eq!(GA::ALL.len(), 17);
    assert_eq!((GA::StaggerBoost.ap(), GA::StaggerBoost.level(), GA::StaggerBoost.name().as_str()), (4, 30, "GOF_STAGGER_BOOST"));
    assert_eq!(GA::from_name("GOF_COUNTER_CHAIN_PLUS"), Some(GA::StaggerBoost));
    let mut a = GoofyAbilities::default();
    assert!(a.apply_record("GOF_COUNTER_CHAIN_PLUS", 1, 1));
    assert!(a.has(GA::StaggerBoost), "owned and worn as the old save had it");
    assert!(GA::StaggerBoost.description().contains("Stagger") && !GA::StaggerBoost.description().to_lowercase().contains("counter"));
    for k in GA::ALL {
        assert!(!k.description().to_lowercase().contains("counter"), "{k:?}: {}", k.description());
    }
}

#[test]
fn every_ability_has_an_ap_cost_a_level_and_a_unique_save_name() {
    let mut names = std::collections::HashSet::new();
    assert_eq!(GA::ALL.len(), 17);
    for k in GA::ALL {
        // (THE USER 2026-11-05: "Danger Sense needs to be 8 AP") the dearest ability is 8 AP
        assert!((1..=8).contains(&k.ap()), "{k:?}");
        assert!((1..=99).contains(&k.level()), "{k:?}");
        assert!(names.insert(k.name()), "{k:?}");
        assert!(k.name().starts_with("GOF_"));
        assert_eq!(GA::from_name(&k.name()), Some(k));
    }
}

#[test]
fn the_level_gives_abilities_worn_while_the_ap_allows() {
    let mut s = crate::FriendStatus::new(Chara::Goofy, 0);
    assert!(s.goofy.as_ref().unwrap().abilities.list().is_empty(), "level 1: none");
    s.level = 40;
    let new = s.gof_sync_level();
    assert!(new.contains(&GA::SteadyGuard));
    assert!(s.ap_in_use() <= s.max_ap());
    assert!(s.gof_sync_level().is_empty(), "once");
}

#[test]
fn item_boost_comes_by_level_and_the_old_seeds_are_gone() {
    use kh_core::enums::ETresAbilityKind as Ab;
    let mut s = crate::FriendStatus::new(Chara::Goofy, 0);
    assert_eq!(s.abilities.owned(Ab::ItemUp), 0);
    assert_eq!(s.abilities.owned(Ab::from_game_name("GOOFY_TORNADE").unwrap()), 0, "R11");
    s.level = K.ability_item_boost_level as u32;
    s.gof_sync_level();
    assert_eq!(s.abilities.owned(Ab::ItemUp), 1);
}

#[test]
fn an_old_saves_seeds_drop_and_free_their_ap() {
    use kh_core::enums::ETresAbilityKind as Ab;
    let mut s = crate::FriendStatus::new(Chara::Goofy, 0);
    let free = s.ap_in_use();
    let k = Ab::from_game_name("GOOFY_TURBO").unwrap();
    s.abilities.set(k, 1, 1);
    assert!(s.ap_in_use() > free);
    assert_eq!(s.gof_drop_kh3(), ["GOOFY_TURBO"]);
    assert_eq!(s.ap_in_use(), free);
    assert!(s.gof_drop_kh3().is_empty(), "once");
}

#[test]
fn second_chance_and_withstand_combo_work_on_goofy() {
    use kh_core::enums::ETresAbilityKind as Ab;
    let mut s = crate::FriendStatus::new(Chara::Goofy, 0);
    assert_eq!(s.gof_survive(50, -5, false), -5);
    s.abilities.set(Ab::LastLeave, 1, 1);
    assert_eq!(s.gof_survive(50, -5, false), 1);
    assert_eq!(s.gof_survive(1, -5, false), -5, "not from 1 HP");
    s.abilities.set(Ab::ComboLeave, 1, 1);
    assert_eq!(s.gof_survive(1, -5, true), 1, "in a running reaction");
}

#[test]
fn the_ai_tree_keeps_its_arts_through_the_rank_gate() {
    let mut s = crate::FriendStatus::new(Chara::Goofy, 0);
    assert_eq!(s.gof_tree_gate("GOOFY_TORNADE"), Some(false));
    assert!(s.mp_cost("GOOFY_TORNADE").is_none(), "not learned");
    s.level = 8;
    assert_eq!(s.gof_tree_gate("GOOFY_TORNADE"), Some(true));
    assert!(s.mp_cost("GOOFY_TORNADE").is_some());
    assert_eq!(s.gof_tree_gate("PRIZE_DRAW"), None);
}

// ------------------------------------------------------------------------------------------------ ranks (11)

#[test]
fn a_rank_waits_for_its_level_and_applies_at_once() {
    let mut g = Growth::default();
    for line in [RankLine::PinningGuard, RankLine::PinningGuard] {
        g.apply(Chara::Goofy, crate::growth::GrowthStep::Rank(line));
    }
    assert_eq!(learn_rank(&g, RankLine::PinningGuard, 1), 0, "a saved rank does not bring the move forward");
    assert_eq!(learn_rank(&g, RankLine::PinningGuard, RankLine::PinningGuard.learn_level() - 1), 0);
    assert_eq!(learn_rank(&g, RankLine::PinningGuard, RankLine::PinningGuard.learn_level()), 3, "both bosses beaten: rank 3 at once");
}

fn learn_rank(g: &Growth, line: RankLine, level: u32) -> u8 {
    crate::goofy::learn::rank(&g.ranks, line, level)
}

#[test]
fn ranks_come_out_the_same_in_every_boss_order() {
    let steps = [RankLine::Deflect, RankLine::PinningGuard, RankLine::Deflect, RankLine::Deflect, RankLine::PinningGuard];
    let mut order = steps;
    let mut seen = Vec::new();
    for rotate in 0..steps.len() {
        order.rotate_left(rotate.min(1));
        let mut g = Growth::default();
        for l in order {
            g.apply(Chara::Goofy, crate::growth::GrowthStep::Rank(l));
        }
        seen.push((learn_rank(&g, RankLine::Deflect, 99), learn_rank(&g, RankLine::PinningGuard, 99), g.hp));
    }
    assert!(seen.iter().all(|s| *s == seen[0]), "{seen:?}");
    assert_eq!((seen[0].0, seen[0].1), (3, 3));
    assert!(seen[0].2 > 0, "Deflect's third boss: past rank 3");
}

#[test]
fn level_one_knows_deflect_and_nothing_else() {
    let s = crate::FriendStatus::new(Chara::Goofy, 0);
    assert_eq!(s.gof_rank(RankLine::Deflect), 1);
    for t in [RankLine::ShieldReflect, RankLine::HopCounter, RankLine::PinningGuard, RankLine::SlipCounter, RankLine::BullBrace, RankLine::SpellWard, RankLine::RhythmDeflect] {
        assert_eq!(s.gof_rank(t), 0, "{t:?}");
    }
}

// ------------------------------------------------------------------------------------------------ the dodge and the stick (R1)

#[test]
fn x_sideways_hops_with_a_one_second_cooldown() {
    assert_eq!(K.hop_cooldown, 1.0);
    let mut rig = Rig::plain();
    rig.step(RIGHT, &[X]);
    assert_eq!(rig.state(), NpcStateId::Pad(HOP));
    assert!(matches!(rig.m().anim.clip.name, "HopL" | "HopR"));
    assert!((rig.k().hop_cd - K.hop_cooldown).abs() < 0.02);
    rig.step(RIGHT, &[]);
    rig.until(120, RIGHT, &[], |r| r.state() != NpcStateId::Pad(HOP));
    rig.run(2, NONE, &[]);
    rig.step(RIGHT, &[X]);
    assert_ne!(rig.state(), NpcStateId::Pad(HOP), "the cooldown holds it");
}

#[test]
fn any_stick_hops_with_no_enemy_near_and_a_back_stick_hops_by_its_side() {
    let mut rig = Rig::new(&[]);
    rig.step(BACK, &[X]);
    assert_eq!(rig.state(), NpcStateId::Pad(HOP));
    assert!(matches!(rig.m().anim.clip.name, "HopL" | "HopR"));
}

#[test]
fn the_hop_is_invincible_for_its_short_window_only() {
    let mut rig = Rig::plain();
    rig.step(RIGHT, &[X]);
    rig.run(4, RIGHT, &[]);
    assert!(!rig.hit(AttackClass::Normal, 10.0).accepted);
    rig.run(30, NONE, &[]);
    assert!(rig.hit(AttackClass::Normal, 10.0).accepted);
}

#[test]
fn toward_and_away_read_against_the_lock_then_the_telegraph_then_the_nearest() {
    use crate::goofy::parry::stick_rel;
    assert_eq!(stick_rel(0.0, true, Some(0.0)), StickRel::Toward);
    assert_eq!(stick_rel(180.0, true, Some(0.0)), StickRel::Away);
    assert_eq!(stick_rel(90.0, true, Some(0.0)), StickRel::Side);
    assert_eq!(stick_rel(90.0, true, None), StickRel::Side, "no enemy: any stick is the hop");
    assert_eq!(stick_rel(90.0, false, Some(0.0)), StickRel::Neutral);
    // the attacker of a live telegraph beats a nearer enemy: X + stick at the far one is toward it, not sideways
    let mut rig = Rig::new(&[Vec3::new(0.0, 300.0, 0.0), Vec3::new(600.0, 0.0, 0.0)]);
    rig.bare();
    rig.party.set_telegraphs(&[Telegraph { attacker: TargetId(2), class: AttackClass::Thrust, perilous: true, eta: 0.2, active: false }]);
    rig.step(FWD, &[X]);
    assert_eq!(rig.state(), NpcStateId::Pad(GUARD), "toward the telegraph's attacker: a tap, no hop");
}

#[test]
fn b_is_always_jump_out_of_the_guard() {
    let mut rig = Rig::plain();
    rig.run(10, NONE, &[X]);
    rig.step(NONE, &[X, B]);
    assert_eq!(rig.state(), NpcStateId::Pad(JUMP));
}

#[test]
fn rb_is_a_plain_lock_on() {
    let mut rig = Rig::new(&[ENEMY]);
    rig.tap(PadButton::RightShoulder, NONE);
    assert_eq!(rig.m().pad.as_ref().unwrap().lock, T);
}

// ------------------------------------------------------------------------------------------------ AI Goofy has the same rules (R12)

/// AI Goofy's string is the same steps: his bash has the same end lag (his A presses cannot restart it, his X cannot cancel
/// it before the cancel point), and he strings again once it is over (THE USER 2026-11-02: "...end lag... like Sora.").
#[test]
fn ai_goofys_bash_has_the_same_end_lag() {
    let mut rig = Rig::new(&[]);
    rig.party.set_pad(rig.id, None);
    rig.party.gof.kh3_ai = true; // the drive is the test's, not the brain's
    let mut last = PadButtons::NONE;
    let mut ai = |rig: &mut Rig, held: &[PadButton]| {
        let mut now = PadButtons::NONE;
        for b in held {
            now.set(*b, true);
        }
        let pad = kh_core::input::PadState { held: now, pressed: PadButtons(now.0 & !last.0), released: PadButtons(last.0 & !now.0) };
        last = now;
        ai_drive(rig.mm(), AiDrive { pad, ..AiDrive::default() }); // what GF-AI's brain hands him each step
        let mut host = PartyHost { world: &rig.world, targets: &mut rig.dummies, audio: &mut rig.audio, effects: &mut rig.effects };
        rig.party.step(DT, &rig.sora, &mut host);
    };
    for i in 0..240 {
        if rig.run_state().combo.end_lag > 0.0 {
            break;
        }
        ai(&mut rig, if i % 8 == 0 { &[A] } else { &[] });
    }
    assert!(rig.logged("shove3") && rig.run_state().combo.end_lag > 0.0, "{:?}", rig.run_state().log);
    assert!(rig.m().pad.is_none());
    let shove1s = count(&rig, "shove1");
    ai(&mut rig, &[X]);
    ai(&mut rig, &[]);
    assert_eq!(rig.state(), NpcStateId::Pad(COMBO), "no guard out of the lag's start");
    let mut n = 2;
    while rig.run_state().combo.end_lag > 0.0 {
        ai(&mut rig, if n % 2 == 0 { &[A] } else { &[] });
        assert_eq!(count(&rig, "shove1"), shove1s, "step {n}: his A press restarted the string in the end lag");
        n += 1;
        assert!(n < 200, "the lag never ended");
    }
    assert!((n as f32 - K.combo_end_lag / DT).abs() <= 1.0, "his lag lasted {n} steps");
    for i in 0..60 {
        if count(&rig, "shove1") > shove1s {
            break;
        }
        ai(&mut rig, if i % 2 == 0 { &[A] } else { &[] });
    }
    assert!(count(&rig, "shove1") > shove1s, "after the lag he strings again: {:?}", rig.run_state().log);
}

#[test]
fn ai_goofy_carries_the_state_and_a_hit_empties_his_resolve() {
    let mut rig = Rig::plain();
    rig.party.set_pad(rig.id, None);
    rig.s().core.resolve.thirds = 6;
    rig.hit(AttackClass::Normal, 10.0);
    assert_eq!(rig.k().resolve.thirds, 0);
    assert!(rig.m().pad.is_none());
}

#[test]
fn ai_goofy_in_a_raised_stance_parries_by_the_same_judgement() {
    let mut rig = Rig::plain();
    rig.party.set_pad(rig.id, None);
    rig.mm().enter(NpcStateId::Pad(GUARD));
    rig.s().core.press(0.0, false, StickRel::Neutral);
    rig.hit(AttackClass::Normal, 10.0);
    assert!(rig.logged("deflect"), "{:?}", rig.run_state().log);
}

// ------------------------------------------------------------------------------------------------ events, seams, assets

#[test]
fn deflects_and_shoves_emit_stagger_and_threat_on_their_target() {
    let mut rig = Rig::plain();
    rig.step(NONE, &[X]);
    rig.step(NONE, &[]);
    rig.hit(AttackClass::Normal, 20.0);
    rig.idle(1);
    assert!(rig.events.iter().any(|e| matches!(e, GoofyEvent::Stagger { target, why: StaggerWhy::Deflect, .. } if *target == T)));
    assert!(rig.events.iter().any(|e| matches!(e, GoofyEvent::Threat { target, why: ThreatWhy::Deflect, .. } if *target == T)));
    assert!(rig.events.iter().any(|e| matches!(e, GoofyEvent::Parried { tech: Tech::Deflect, right: true, .. })));
    let mut rig = Rig::new(&[Vec3::new(170.0, 0.0, 0.0)]);
    rig.tap(A, NONE);
    rig.until(120, NONE, &[], |r| r.events.iter().any(|e| matches!(e, GoofyEvent::Stagger { why: StaggerWhy::Basic, .. })));
    assert!(rig.events.iter().any(|e| matches!(e, GoofyEvent::Threat { why: ThreatWhy::Basic, .. })), "Taunt Hits");
}

#[test]
fn a_hit_with_no_named_attacker_takes_it_from_the_open_telegraph() {
    let mut rig = Rig::plain();
    rig.telegraph(AttackClass::Normal, true);
    rig.idle(1);
    let mut h = HitRecord::empty(TargetId::NONE);
    h.damage = 10.0;
    crate::goofy::hooks::name_attacker(rig.party.telegraphs(), &mut h);
    assert_eq!(h.attack_info.attacker, Some(T));
}

#[test]
fn every_clip_of_the_kit_is_in_his_set() {
    use super::super::goofy_combo::{def as step_def, Step};
    let mut names = vec![K.deflect_clip, K.brace_clip, K.brace_hit_clip, K.guard_break_clip, K.guard_break_air_clip, K.guard_break_loop_clip, K.guard_break_air_loop_clip, K.ward_clip, "SGuardIdle", "SGuardWalk", "SGuardHit", "HopL", "HopR"];
    for s in [Step::Shove1, Step::Shove2, Step::Shove3, Step::Bonk] {
        names.push(step_def(s).clip);
    }
    for c in names {
        assert_eq!(crate::tables::clip(Chara::Goofy, c).name, c, "a misspelled clip silently becomes a 0.5 s empty one");
    }
}

#[test]
fn every_kind_row_of_the_kit_parses() {
    assert!(Kind::from_game_name(K.batted_kind).is_some(), "Goofy Tornado's batted bullet");
    assert!(Kind::from_game_name(K.breaker_kind).is_some() && Kind::from_game_name(K.breaker_launch_kind).is_some());
}
#[test]
fn every_sound_and_effect_of_the_kit_is_packed() {
    let live = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut up = live.clone();
    let (mut fx_dirs, mut audio) = (Vec::new(), String::new());
    for _ in 0..10 {
        for d in ["build/sora_pack/vfx/effects", "build/sora_pack/party/vfx/effects"] {
            if up.join(d).is_dir() {
                fx_dirs.push(up.join(d));
            }
        }
        if audio.is_empty() {
            audio = std::fs::read_to_string(up.join("build/sora_pack/audio/manifest.json")).unwrap_or_default();
        }
        up = up.join("..");
    }
    if fx_dirs.is_empty() || audio.is_empty() {
        println!("no pack here: skipped");
        return;
    }
    let (fx, se) = shots::cue_names();
    let mut missing: Vec<&str> = fx.iter().copied().filter(|n| !fx_dirs.iter().any(|d| d.join(format!("{n}.json")).is_file())).collect();
    missing.extend(se.iter().copied().filter(|n| !audio.contains(n)));
    assert!(missing.is_empty(), "{missing:?}");
}

// ------------------------------------------------------------------------------------------------ the batted shot (Tornado rank 3)

// (THE USER 2026-11-02) Return Fire went with the counters; Goofy Tornado's rank 3 still bats a bullet back (goofy_arts_tests)
const _: () = assert!(K.batted_life > 0.0 && K.batted_speed > 0.0);

#[test]
fn the_shots_sheet_row_names_a_kind_with_an_actor_effect() {
    let kind = crate::projectile::ProjectileKind::of_key(K.batted_shot).expect("a proj.* key");
    assert!(kind.class().is_some(), "something to see");
}
/// (THE USER 2026-11-02: "this effect is not aligned properly") the block's guard barrier VN_bsh_inl0 goes ON his shield
/// (the weapon's vfx_call, where KH3's own row attaches it), following it in any pose; no longer a point ahead of his capsule.
#[test]
fn the_blocks_barrier_is_spawned_on_his_shield() {
    let mut rig = Rig::plain();
    rig.run(30, NONE, &[X]);
    let max = rig.m().status.max_hp() as f32;
    assert!(rig.hit(AttackClass::Normal, max * 0.1).guarded);
    assert_eq!(rig.run_state().shield_fx, vec![K.guard_block_effect]);
    assert!(!rig.run_state().fx.iter().any(|f| f.0 == K.guard_block_effect), "not at a fixed point too");
    let s = super::super::goofy_shots::shield_spawn(rig.m(), K.guard_block_effect);
    assert_eq!(s.at, kh_core::Attach::Party { member: kh_core::host::PartyMember(rig.m().id.0), bone: Some("wep:vfx_call"), offset: Vec3::ZERO });
    assert!(!s.persistent && s.name == "VN_bsh_inl0");
    // the party's step plays it and empties the queue
    rig.idle(1);
    assert!(rig.run_state().shield_fx.is_empty());
}

/// (COMBAT-FIX) KH3 puts the shield's sphere at ATK_00 + (0, 0, -10) in the socket's own frame (w_gu_WepBase TresAtkColl0
/// group "None" m_RelativeLocation1, turned by the socket: 0x140647950). The pack now carries the socket's rotation
/// (`attack_sockets.rotations`, SocketKey::q); ATK_00 is roll 180 on wep and ATK_01 is 19 cm up wep's z, so the point is 10/19
/// of the way from ATK_00 to ATK_01 on every frame: checked against the pack's own ATK_01 track, which the rotation never saw.
#[test]
fn the_shield_sphere_sits_10_cm_off_atk_00_in_the_sockets_frame() {
    let path = format!("{}/../../build/sora_pack/party/n_ex002/manifest.json", env!("CARGO_MANIFEST_DIR"));
    let Ok(text) = std::fs::read_to_string(&path) else { return };
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    for (clip, seq) in [("M_1301", "1301"), ("M_1302", "1302"), ("M_1311", "1311")] {
        let keys = shots::clip(clip).socket;
        let frames = json["attack_sockets"]["clips"][seq]["sockets"].as_array().unwrap();
        assert_eq!(keys.len(), frames.len(), "{clip}");
        assert!(keys.iter().any(|k| k.q != [0.0, 0.0, 0.0, 1.0]), "{clip}: the pack's rotations are in");
        for (k, f) in keys.iter().zip(frames) {
            let v: Vec<f32> = f.as_array().unwrap().iter().map(|x| x.as_f64().unwrap() as f32).collect();
            let (a0, a1) = (Vec3::new(v[0], v[1], v[2]), Vec3::new(v[3], v[4], v[5]));
            let at = crate::actions::socket_point_at(keys, k.time, crate::actions::GOOFY_SHIELD_OFFSET);
            assert!(at.distance(a0 + (a1 - a0) * (10.0 / 19.0)) < 0.05, "{clip} at {}: {at} against ATK_01's line", k.time);
        }
    }
}
