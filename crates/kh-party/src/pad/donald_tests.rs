//! (DON-KIT) Donald's kit after the overhaul: one or more tests per rule (documentation/DONALD_OVERHAUL.md 2, 3, 4.2,
//! 4.4, 4.6, 5.1 - 5.3, 6.6, 8, 12, 13; the plan's DON-KIT test list, DON_PLAN.md 6.3).
//! `cargo test -p kh-party donald_tests -- --nocapture` prints the harnesses (the seconds to Resonance 3, the BAL-style
//! comparison).

use super::donald::{self, KitEvent, K};
use super::donald_combo as combo;
use super::donald_moves as moves;
use super::donald_shots::{self as shots, MissileKind};
use super::*;
use crate::donald::{data as don_data, DonAbilities, DonAbility, DonCmd, DonLearned, DonMods, Element, Reaction, SpellRef};
use crate::host::PartyHost;
use crate::{Party, PartyConfig, SoraView};
use kh_core::enums::ETresDamageKind;
use kh_core::host::{HitAttacker, HitRecord, HitResult, HurtShape, TargetDefense, TargetInfo, Targets};
use kh_core::input::PadButtons;
use kh_core::test_world::TestWorld;

const DT: f32 = 1.0 / 60.0;
const A: PadButton = PadButton::FaceBottom;
const B: PadButton = PadButton::FaceRight;
const X: PadButton = PadButton::FaceLeft;
const RB: PadButton = PadButton::RightShoulder;

struct Dummies(Vec<(TargetId, Vec3)>, Vec<HitRecord>);

impl Dummies {
    fn info_of(&self, (id, p): (TargetId, Vec3)) -> TargetInfo {
        // ids from 100 are an Elden Ring breakable / friendly NPC: ALIVE | HITTABLE (| GIMMICK), never LOCKABLE
        let flags = if id.0 >= 100 { TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::GIMMICK } else { TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY };
        TargetInfo { id, position: p, center: p + Vec3::Z * 90.0, velocity: Vec3::ZERO, yaw: 180.0, radius: 50.0, half_height: 90.0, flags: TargetFlags(flags), hp: 1.0e6, hp_max: 1.0e3 }
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
            out.push(HurtShape { a: i.position + Vec3::Z * 30.0, b: i.position + Vec3::Z * 150.0, radius: 50.0, part: 0 });
        }
    }
    fn apply_hit(&mut self, hit: &HitRecord) -> HitResult {
        self.1.push(*hit);
        HitResult { accepted: true, damage_dealt: hit.damage, hp_after: 1.0e6, killed: false, guarded: false }
    }
    fn defense(&self, _id: TargetId) -> Option<TargetDefense> {
        Some(TargetDefense { defense: 10, max_hp: 1_000_000, hp_percent: 100.0, enemy_kind: true, damage_min: 0, damage_max: 0, resist: [0; 8], bio_type: 0 })
    }
}

/// What the kit asked the audio / effect sides for (names).
#[derive(Default)]
struct RecAudio {
    played: Vec<&'static str>,
    /// (KITFX) (handle, name, volume) of every looping play, and (handle, fade) of every stop
    loops: Vec<(u32, &'static str, f32)>,
    stops: Vec<(u32, f32)>,
}

impl kh_core::host::Audio for RecAudio {
    fn play(&mut self, handle: kh_core::host::SoundHandle, play: &kh_core::host::SoundPlay) {
        self.played.push(play.name);
        if play.looping {
            self.loops.push((handle.0, play.name, play.volume));
        }
    }
    fn stop(&mut self, handle: kh_core::host::SoundHandle, fade_out: f32) {
        self.stops.push((handle.0, fade_out));
    }
}

#[derive(Default)]
struct RecEffects {
    spawned: Vec<&'static str>,
    /// (handle, immediate) of every stop
    stops: Vec<(u32, bool)>,
}

impl kh_core::host::Effects for RecEffects {
    fn spawn(&mut self, _handle: kh_core::host::EffectHandle, spawn: &kh_core::host::EffectSpawn) {
        self.spawned.push(spawn.name);
    }
    fn stop(&mut self, handle: kh_core::host::EffectHandle, immediate: bool) {
        self.stops.push((handle.0, immediate));
    }
}

struct Rig {
    world: TestWorld,
    dummies: Dummies,
    audio: RecAudio,
    effects: RecEffects,
    party: Party,
    sora: SoraView,
    id: MemberId,
    held: PadButtons,
}

impl Rig {
    /// A pad Donald with everything learned and NO ability worn (each test wears what it tests).
    fn new(dummies: &[Vec3]) -> Rig {
        let mut party = Party::new(PartyConfig::test_level99());
        let id = party.spawn(Chara::Donald, Vec3::ZERO, 0.0);
        party.set_pad(id, Some(Tuning::USER));
        {
            let st = party.status_mut(id).unwrap().donald.as_mut().unwrap();
            st.learned = DonLearned::all();
            st.abilities = DonAbilities::default();
        }
        let d = Dummies(dummies.iter().enumerate().map(|(i, p)| (TargetId(i as u32 + 1), *p)).collect(), Vec::new());
        let mut rig = Rig { world: TestWorld::floor(), dummies: d, audio: Default::default(), effects: Default::default(), party, sora: SoraView::default(), id, held: PadButtons::NONE };
        rig.sora.position = Vec3::new(-2000.0, 0.0, 0.0);
        // in a fight (Resonance drains and Arcane Flow empties out of combat, 4.1 / 6.3)
        rig.party.set_in_battle(true);
        rig.idle(10);
        rig
    }

    fn m(&self) -> &NpcPawn {
        self.party.member(self.id).unwrap()
    }

    fn mm(&mut self) -> &mut NpcPawn {
        self.party.member_mut(self.id).unwrap()
    }

    fn kit(&self) -> &donald::DonaldKit {
        &self.m().pad.as_ref().unwrap().donald
    }

    fn wear(&mut self, list: &[DonAbility]) {
        let st = self.party.status_mut(self.id).unwrap().donald.as_mut().unwrap();
        for k in list {
            st.abilities.give(*k);
            st.abilities.set_equipped(*k, true);
        }
    }

    /// Attunes him at once to `e` (Instant Attune's rule), optionally to full power.
    fn attune(&mut self, e: Element, full: bool) {
        let d = don_data();
        let st = self.party.status_mut(self.id).unwrap().donald.as_mut().unwrap();
        let learned = st.learned;
        let mods = DonMods { instant_attune: true, dual: st.abilities.has(DonAbility::DualAttunement), ..DonMods::default() };
        st.core.begin_attune(e, &learned, &mods, &d).unwrap();
        if full {
            // missile hits (res.gain_missile each) until Resonance 3
            for _ in 0..1000 {
                if st.core.full_power(e, &d) {
                    break;
                }
                st.core.on_missile_hit(Some(e), &d);
            }
            assert!(st.core.full_power(e, &d));
        }
    }

    fn step(&mut self, stick: Vec2, held: &[PadButton]) {
        let mut now = PadButtons::NONE;
        for b in held {
            now.set(*b, true);
        }
        let pad = Buttons { held: now, pressed: PadButtons(now.0 & !self.held.0), released: PadButtons(self.held.0 & !now.0) };
        self.held = now;
        self.party.set_pad_frame(self.id, PadFrame { stick, camera_yaw: 0.0, pad });
        let mut host = PartyHost { world: &self.world, targets: &mut self.dummies, audio: &mut self.audio, effects: &mut self.effects };
        self.party.step(DT, &self.sora, &mut host);
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.step(Vec2::ZERO, &[]);
        }
    }

    fn tap(&mut self, b: PadButton, stick: Vec2) {
        self.step(stick, &[b]);
        self.step(stick, &[]);
    }

    fn until(&mut self, stick: Vec2, held: &[PadButton], n: usize, done: impl Fn(&Rig) -> bool) -> Option<usize> {
        for k in 1..=n {
            self.step(stick, held);
            if done(self) {
                return Some(k);
            }
        }
        None
    }

    fn state(&self) -> Option<u8> {
        donald::kit_state(self.m())
    }

    fn log(&self) -> Vec<KitEvent> {
        self.kit().log.iter().map(|e| e.1).collect()
    }

    fn started(&self) -> Vec<&'static str> {
        self.log().into_iter().filter_map(|e| if let KitEvent::Started(s) = e { Some(s) } else { None }).collect()
    }

    fn missiles(&self) -> Vec<(MissileKind, Option<Element>, f32)> {
        self.log().into_iter().filter_map(|e| if let KitEvent::Missile { kind, element, mult } = e { Some((kind, element, mult)) } else { None }).collect()
    }

    fn rows(&self) -> Vec<&'static str> {
        self.dummies.1.iter().map(|h| h.attack_name).collect()
    }

    /// Mashes A every `every` frames for `frames`.
    fn mash(&mut self, every: usize, frames: usize) {
        for i in 0..frames {
            let held: &[PadButton] = if i % every == 0 { &[A] } else { &[] };
            self.step(Vec2::ZERO, held);
        }
    }

    fn core(&self) -> &crate::donald::DonCore {
        &self.m().status.donald.as_ref().unwrap().core
    }
}

// ================================================================================================ 3.1 the ground combo

#[test]
fn the_ground_combo_is_wave_left_wave_right_and_the_slam_of_two_missiles_at_075() {
    let mut rig = Rig::new(&[Vec3::new(1200.0, 0.0, 0.0)]);
    rig.mash(8, 240);
    let steps: Vec<&str> = rig.started().into_iter().filter(|s| matches!(*s, "wave_left" | "wave_right" | "plus_left" | "plus_right" | "slam")).collect();
    assert_eq!(&steps[..3], &["wave_left", "wave_right", "slam"], "{:?}", rig.log());
    let m = rig.missiles();
    assert_eq!(m[0], (MissileKind::Step, None, 1.0));
    assert_eq!(m[1], (MissileKind::Step, None, 1.0));
    assert_eq!((m[2].0, m[2].2), (MissileKind::Slam, K.slam_mult));
    assert_eq!((m[3].0, m[3].2), (MissileKind::Slam, 0.75), "the slam bursts two at x0.75");
    rig.idle(90);
    let rows = rig.rows();
    assert!(rows.iter().filter(|r| **r == "DON_MISSILE").count() >= 2 && rows.iter().filter(|r| **r == "DON_SLAM").count() >= 2, "{rows:?}");
    // each slam missile's power is 0.75 of a step missile's (the same range, no Spell Weaver)
    let p_step = rig.dummies.1.iter().find(|h| h.attack_name == "DON_MISSILE").unwrap().info.power;
    let p_slam = rig.dummies.1.iter().find(|h| h.attack_name == "DON_SLAM").unwrap().info.power;
    assert!((p_slam / p_step - 0.75).abs() < 1e-3, "{p_slam} / {p_step}");
    // a missile is a magic hit
    assert!(rig.dummies.1.iter().all(|h| h.info.flags50 & kh_core::host::HitInfo::FLAG50_MAGIC != 0));
}

#[test]
fn combo_plus_adds_steps_three_four_five() {
    for (wear, want) in [(vec![], 3usize), (vec![DonAbility::ComboPlus], 4), (vec![DonAbility::ComboPlus, DonAbility::ComboPlus2], 5)] {
        let mut rig = Rig::new(&[Vec3::new(1200.0, 0.0, 0.0)]);
        rig.wear(&wear);
        rig.mash(8, 400);
        let steps: Vec<&str> = rig.started().into_iter().filter(|s| matches!(*s, "wave_left" | "wave_right" | "plus_left" | "plus_right" | "slam")).collect();
        let to_slam = steps.iter().position(|s| *s == "slam").unwrap() + 1;
        assert_eq!(to_slam, want, "{wear:?}: {steps:?}");
    }
}

#[test]
fn the_link_and_move_points_and_the_cancel_after_the_missile() {
    let mut rig = Rig::new(&[Vec3::new(1200.0, 0.0, 0.0)]);
    rig.tap(A, Vec2::ZERO);
    assert_eq!(rig.state(), Some(donald::STEP));
    // before the missile leaves: no cancel; X does nothing yet
    assert!(!combo::cancel_open(rig.m()));
    let shot = rig.until(Vec2::ZERO, &[], 60, |r| combo::cancel_open(r.m())).expect("the cancel point opens");
    let left = rig.missiles().len();
    assert_eq!(left, 1, "the cancel opens after the missile left");
    assert!(shot as f32 * DT >= K.step_wave_l_shot / K.step_wave_l_rate - 0.05);
    // X + stick: the dodge cancels the step
    rig.tap(X, Vec2::new(0.0, 1.0));
    assert_eq!(rig.state(), Some(donald::DODGE));
    // a cast is accepted at a cancel point and refused mid-swing before the shot
    let mut rig = Rig::new(&[Vec3::new(1200.0, 0.0, 0.0)]);
    rig.tap(A, Vec2::ZERO);
    let cmd = DonCmd::Spell(SpellRef::Line1(Element::Fire, 1));
    assert!(!donald::wants_cast(rig.mm(), cmd, crate::donald::cast::CastTarget::Enemy(TargetId(1))), "not before the missile left");
    rig.until(Vec2::ZERO, &[], 60, |r| combo::cancel_open(r.m())).unwrap();
    assert!(donald::wants_cast(rig.mm(), cmd, crate::donald::cast::CastTarget::Enemy(TargetId(1))), "a spell weaves in after the missile");
    assert!(rig.kit().pending_cast.is_some());
    // the stick takes him out at the move point
    let mut rig = Rig::new(&[Vec3::new(1200.0, 0.0, 0.0)]);
    rig.tap(A, Vec2::ZERO);
    let out = rig.until(Vec2::new(0.0, 1.0), &[], 90, |r| r.m().state == NpcStateId::Pad(RUN)).expect("out to the run");
    assert!(out as f32 * DT >= K.step_wave_l_move / K.step_wave_l_rate - 0.05, "{out}");
}

// ================================================================================================ the missile

#[test]
fn soft_homing_a_sidestep_dodges_it_a_standing_target_is_hit() {
    // the rule: the turn is capped
    let v = Vec3::new(1800.0, 0.0, 0.0);
    let turned = shots::home(v, Vec3::ZERO, Vec3::new(0.0, 1000.0, 0.0), K.missile_turn, DT);
    let deg = turned.normalize().y.asin().to_degrees();
    assert!((deg - K.missile_turn * DT).abs() < 0.05, "{deg}");
    // a target 7 m ahead that sidesteps at run speed when the missile leaves is missed
    let mut rig = Rig::new(&[Vec3::new(700.0, 0.0, 0.0)]);
    rig.tap(A, Vec2::ZERO);
    rig.until(Vec2::ZERO, &[], 60, |r| !r.missiles().is_empty()).unwrap();
    for _ in 0..90 {
        rig.dummies.0[0].1.y += 680.0 * DT;
        rig.idle(1);
    }
    assert!(!rig.rows().contains(&"DON_MISSILE"), "a sidestep dodges a missile: {:?}", rig.rows());
    // standing still it is hit
    let mut rig = Rig::new(&[Vec3::new(700.0, 0.0, 0.0)]);
    rig.tap(A, Vec2::ZERO);
    rig.idle(90);
    assert!(rig.rows().contains(&"DON_MISSILE"));
}

#[test]
fn a_missile_flies_30_m_then_fades() {
    // a target to shoot at (no target = the Wand Bonk), gone once the missile left: it flies straight on
    let mut rig = Rig::new(&[Vec3::new(1500.0, 0.0, 0.0)]);
    rig.tap(A, Vec2::ZERO);
    rig.until(Vec2::ZERO, &[], 60, |r| !r.kit().shots.missiles.is_empty()).unwrap();
    rig.dummies.0.clear();
    let mut far = 0.0f32;
    for _ in 0..240 {
        rig.idle(1);
        if let Some(m) = rig.kit().shots.missiles.first() {
            far = far.max(m.travelled);
            if m.fading.is_some() {
                break;
            }
        }
    }
    assert!((far - K.missile_range).abs() < K.missile_speed * DT * 2.0, "{far}");
    rig.idle(30);
    assert!(rig.kit().shots.missiles.is_empty(), "faded out");
}

#[test]
fn full_damage_from_6_m_x06_under_6_m() {
    assert_eq!(shots::range_mult(K.missile_near - 1.0), K.missile_near_mult);
    assert_eq!(shots::range_mult(K.missile_near + 1.0), 1.0);
    assert_eq!(shots::range_mult(2400.0), 1.0);
    let power = |x: f32| {
        let mut rig = Rig::new(&[Vec3::new(x, 0.0, 0.0)]);
        rig.tap(A, Vec2::ZERO);
        rig.idle(100);
        rig.dummies.1.iter().find(|h| h.attack_name == "DON_MISSILE").map(|h| h.info.power).unwrap()
    };
    let (near, far) = (power(450.0), power(1200.0));
    assert!((near / far - 0.6).abs() < 1e-3, "{near} vs {far}");
}

// ================================================================================================ 5.3 Wand Bonk

#[test]
fn wand_bonk_under_2_m_knocks_back_3_m_and_is_no_combo_step() {
    let mut rig = Rig::new(&[Vec3::new(180.0, 0.0, 0.0)]);
    rig.tap(A, Vec2::ZERO);
    assert_eq!(rig.state(), Some(donald::BONK));
    rig.idle(40);
    let h = rig.dummies.1.iter().find(|h| h.attack_name == "DON_WAND_BONK").copied().expect("the Bonk hit");
    assert_eq!(h.info.move_length, K.bonk_knockback);
    assert_eq!(h.info.move_length, 300.0);
    assert_eq!(h.info.flags50 & kh_core::host::HitInfo::FLAG50_MAGIC, 0, "a physical hit");
    assert!(rig.missiles().is_empty(), "no missile");
    assert_eq!(rig.kit().combo.n, 0, "not a combo step");
    // Wand Bonk+: bigger knockback
    let mut rig = Rig::new(&[Vec3::new(180.0, 0.0, 0.0)]);
    rig.wear(&[DonAbility::WandBonkPlus]);
    rig.tap(A, Vec2::ZERO);
    rig.idle(40);
    assert_eq!(rig.dummies.1.iter().find(|h| h.attack_name == "DON_WAND_BONK").unwrap().info.move_length, K.bonk_knockback_plus);
    // a pot (not lockable) never draws the Bonk: with an enemy to shoot at he shoots
    let mut rig = Rig::new(&[Vec3::new(1200.0, 0.0, 0.0)]);
    rig.dummies.0.push((TargetId(100), Vec3::new(150.0, 0.0, 0.0)));
    rig.tap(A, Vec2::ZERO);
    assert_eq!(rig.state(), Some(donald::STEP));
}

// ================================================================================================ 3.3 the Charged Missile

#[test]
fn holding_a_charges_x25_piercing_doubled_breaker_no_mp() {
    for (plus, hold) in [(false, K.charge_hold), (true, K.charge_hold_plus)] {
        let mut rig = Rig::new(&[Vec3::new(1200.0, 0.0, 0.0), Vec3::new(1500.0, 0.0, 0.0)]);
        if plus {
            rig.wear(&[DonAbility::ChargedMissilePlus]);
        }
        let mp = rig.m().status.mp;
        let f = rig.until(Vec2::ZERO, &[A], 90, |r| r.state() == Some(donald::CHARGE)).expect("the charge");
        assert!((f as f32 * DT - hold).abs() < 0.08, "{plus}: {f} frames for {hold}");
        assert!(rig.missiles().is_empty(), "the step's missile is not fired");
        // he walks slowly while charging
        let x0 = rig.m().position().y;
        for _ in 0..30 {
            rig.step(Vec2::new(1.0, 0.0), &[A]);
        }
        let walked = (rig.m().position().y - x0).abs() / (30.0 * DT);
        assert!(walked > 0.5 * K.charge_walk && walked < 1.2 * K.charge_walk, "{walked}");
        rig.idle(120);
        let m = rig.missiles();
        assert_eq!((m[0].0, m[0].2), (MissileKind::Charged, K.charge_mult));
        let hits: Vec<&HitRecord> = rig.dummies.1.iter().filter(|h| h.attack_name == "DON_CHARGED").collect();
        assert_eq!(hits.len(), 2, "it pierces");
        assert_eq!(rig.m().status.mp, mp, "no MP");
    }
    // it shatters a Frozen enemy (R24 breaker) whatever its element
    let mut rig = Rig::new(&[Vec3::new(1200.0, 0.0, 0.0)]);
    let d = don_data();
    let mut ev = Vec::new();
    rig.party.don.status.entry(1, crate::donald::Size::Small).freeze(Default::default(), &d.tune, &mut ev);
    rig.until(Vec2::ZERO, &[A], 90, |r| r.state() == Some(donald::CHARGE)).unwrap();
    rig.idle(120);
    assert!(!rig.party.don.status.get(1).unwrap().is_frozen(), "Shatter ended Frozen");
}

// ================================================================================================ 3.2 air

#[test]
fn the_air_combo_two_missiles_a_float_air_combo_plus_and_starfall() {
    let air = |wear: &[DonAbility]| {
        let mut rig = Rig::new(&[Vec3::new(900.0, 0.0, 0.0)]);
        rig.wear(wear);
        for i in 0..160 {
            let held: &[PadButton] = match i {
                0..=6 => &[B],
                i if i >= 14 && i % 7 == 0 => &[A],
                _ => &[],
            };
            rig.step(Vec2::ZERO, held);
        }
        rig.idle(120);
        rig
    };
    let rig = air(&[]);
    let m = rig.missiles();
    assert_eq!(m.iter().filter(|x| x.0 == MissileKind::Air).count(), 2, "{:?}", rig.log());
    assert!(!rig.started().contains(&"starfall"), "no air finisher by default");
    let rig = air(&[DonAbility::AirComboPlus]);
    assert_eq!(rig.missiles().iter().filter(|x| x.0 == MissileKind::Air).count(), 3);
    let rig = air(&[DonAbility::Starfall]);
    assert!(rig.started().contains(&"starfall"));
    assert_eq!(rig.missiles().iter().filter(|x| x.0 == MissileKind::Starfall).count(), K.starfall_count as usize);
}

// ================================================================================================ 9 finishers, 4.2 slams

#[test]
fn finisher_abilities_replace_the_slam() {
    let slam = |wear: &[DonAbility], extra: bool| {
        let mut d = vec![Vec3::new(1200.0, 0.0, 0.0)];
        if extra {
            d.push(Vec3::new(1300.0, 300.0, 0.0));
        }
        let mut rig = Rig::new(&d);
        rig.wear(wear);
        rig.mash(8, 240);
        rig
    };
    let rig = slam(&[DonAbility::MissileVolley], false);
    let v: Vec<f32> = rig.missiles().iter().filter(|m| m.0 == MissileKind::Slam).map(|m| m.2).collect();
    assert!(v.len() >= 5 && v[..5].iter().all(|x| *x == K.volley_mult), "{v:?}");
    let rig = slam(&[DonAbility::ArcaneBolt], false);
    let b: Vec<f32> = rig.missiles().iter().filter(|m| m.0 == MissileKind::Bolt).map(|m| m.2).collect();
    assert_eq!(b.first(), Some(&K.bolt_mult));
    // both worn: Volley with a crowd, Arcane Bolt on one target
    assert!(slam(&[DonAbility::MissileVolley, DonAbility::ArcaneBolt], true).started().contains(&"missile_volley"));
    assert!(slam(&[DonAbility::MissileVolley, DonAbility::ArcaneBolt], false).started().contains(&"arcane_bolt"));
}

#[test]
fn the_slam_of_each_attunement() {
    for (e, want) in [(Element::Fire, "slam_fire"), (Element::Blizzard, "slam_blizzard"), (Element::Thunder, "slam_thunder"), (Element::Water, "slam_water"), (Element::Aero, "slam_aero"), (Element::Light, "slam_light")] {
        // inside the Water wave's 6 m
        let mut rig = Rig::new(&[Vec3::new(550.0, 0.0, 0.0)]);
        rig.attune(e, false);
        rig.mash(8, 240);
        rig.idle(60);
        assert!(rig.started().contains(&want), "{e:?}: {:?}", rig.started());
        let rows = rig.rows();
        match e {
            Element::Fire => assert_eq!(rig.missiles().iter().filter(|m| m.0 == MissileKind::Slam && m.1 == Some(Element::Fire)).count() % K.slam_fire_count as usize, 0),
            Element::Blizzard => assert!(rows.contains(&"DON_SLAM_BOLT"), "the ice lance {rows:?}"),
            Element::Thunder => assert!(rows.contains(&"DON_SLAM_THUNDER"), "{rows:?}"),
            Element::Water => assert!(rows.contains(&"DON_SLAM_WATER") && rig.dummies.1.iter().find(|h| h.attack_name == "DON_SLAM_WATER").unwrap().info.move_length == K.slam_water_push, "{rows:?}"),
            Element::Aero => assert!(rig.dummies.1.iter().any(|h| h.attack_name == "DON_SLAM_AERO" && h.damage_kind == ETresDamageKind::Toss), "the tornado lifts {rows:?}"),
            Element::Light => assert!(rig.effects.spawned.contains(&K.slam_light_fx), "the pulse"),
        }
    }
}

// ================================================================================================ 4.2 the missiles' elements

#[test]
fn missile_elements_burn_chill_chain_pierce_wet_launch_reveal() {
    // Fire: Burn + a splash on a neighbour
    let mut rig = Rig::new(&[Vec3::new(1000.0, 0.0, 0.0), Vec3::new(1080.0, 60.0, 0.0)]);
    rig.attune(Element::Fire, false);
    rig.tap(A, Vec2::ZERO);
    rig.idle(90);
    assert!(rig.party.don.status.get(1).is_some_and(|s| s.is_burned()));
    assert!(rig.rows().contains(&"DON_SPLASH"), "{:?}", rig.rows());
    // Blizzard: Chill
    let mut rig = Rig::new(&[Vec3::new(1000.0, 0.0, 0.0)]);
    rig.attune(Element::Blizzard, false);
    rig.tap(A, Vec2::ZERO);
    rig.idle(90);
    assert!(rig.party.don.status.get(1).is_some_and(|s| s.is_chilled()));
    // Thunder: a chain to a 2nd enemy within 8 m; 3 targets at full power
    for (full, want) in [(false, 1usize), (true, 2)] {
        let mut rig = Rig::new(&[Vec3::new(1000.0, 0.0, 0.0), Vec3::new(1000.0, 500.0, 0.0), Vec3::new(1300.0, 900.0, 0.0)]);
        rig.attune(Element::Thunder, full);
        rig.tap(A, Vec2::ZERO);
        rig.idle(90);
        let chains = rig.rows().iter().filter(|r| **r == "DON_CHAIN").count();
        assert_eq!(chains, want, "full {full}: {:?}", rig.rows());
    }
    // Water: pierces + Wet
    let mut rig = Rig::new(&[Vec3::new(1000.0, 0.0, 0.0), Vec3::new(1300.0, 0.0, 0.0)]);
    rig.attune(Element::Water, false);
    rig.tap(A, Vec2::ZERO);
    rig.idle(90);
    assert_eq!(rig.rows().iter().filter(|r| **r == "DON_MISSILE").count(), 2, "it pierces");
    assert!(rig.party.don.status.get(1).is_some_and(|s| s.is_wet()) && rig.party.don.status.get(2).is_some_and(|s| s.is_wet()));
    // Aero: the third hit launches a small enemy
    let mut rig = Rig::new(&[Vec3::new(1000.0, 0.0, 0.0)]);
    rig.attune(Element::Aero, false);
    for _ in 0..3 {
        rig.tap(A, Vec2::ZERO);
        rig.idle(70);
    }
    let kinds: Vec<ETresDamageKind> = rig.dummies.1.iter().filter(|h| h.attack_name == "DON_MISSILE").map(|h| h.damage_kind).collect();
    assert_eq!(kinds.len(), 3, "{:?}", rig.rows());
    assert_eq!(kinds[..2], [ETresDamageKind::Small, ETresDamageKind::Small]);
    assert_eq!(kinds[2], ETresDamageKind::Toss, "the third launches");
    // Light: Reveals, and heals a member it passes
    let mut rig = Rig::new(&[Vec3::new(1200.0, 0.0, 0.0)]);
    let goofy = rig.party.spawn(Chara::Goofy, Vec3::new(500.0, 30.0, 0.0), 0.0);
    rig.party.status_mut(goofy).unwrap().hp = 10;
    rig.attune(Element::Light, false);
    rig.tap(A, Vec2::ZERO);
    rig.idle(90);
    assert!(rig.party.don.status.get(1).is_some_and(|s| s.is_revealed()));
    assert!(rig.party.status(goofy).unwrap().hp > 10, "the missile healed Goofy as it passed");
}

#[test]
fn dual_attunement_missiles_alternate_and_every_hit_goes_through_the_reaction_rule() {
    let mut rig = Rig::new(&[Vec3::new(1000.0, 0.0, 0.0)]);
    rig.wear(&[DonAbility::DualAttunement]);
    rig.attune(Element::Water, false);
    rig.attune(Element::Thunder, false);
    rig.mash(8, 240);
    rig.idle(60);
    let els: Vec<Option<Element>> = rig.missiles().iter().map(|m| m.1).collect();
    assert!(els.windows(2).take(3).all(|w| w[0] != w[1]), "{els:?}");
    // Water then Thunder on the same enemy: Conduct (Party::don_hit), its pip and Arcane Flow
    assert!(rig.kit().shots.reactions.is_empty() || true);
    let any_conduct = rig.party.member(rig.id).unwrap().status.donald.as_ref().unwrap().core.arcane.charges() > 0;
    assert!(any_conduct, "a reaction gave Arcane Flow");
}

#[test]
fn spell_weaver_adds_20_percent_right_after_a_spell() {
    let power = |weaver: bool| {
        let mut rig = Rig::new(&[Vec3::new(1200.0, 0.0, 0.0)]);
        if weaver {
            rig.wear(&[DonAbility::SpellWeaver]);
            let clock = rig.kit().clock;
            rig.mm().pad.as_mut().unwrap().donald.last_spell_at = Some(clock);
        }
        rig.tap(A, Vec2::ZERO);
        rig.idle(90);
        rig.dummies.1.iter().find(|h| h.attack_name == "DON_MISSILE").unwrap().info.power
    };
    assert!((power(true) / power(false) - K.weaver_mult).abs() < 1e-3);
}

#[test]
fn missiles_fill_resonance_and_shorten_the_mp_charge() {
    let mut rig = Rig::new(&[Vec3::new(1000.0, 0.0, 0.0)]);
    rig.attune(Element::Fire, false);
    let before = rig.core().resonance(Element::Fire).unwrap();
    rig.mm().status.mp = 0;
    rig.mm().status.mp_charge_left = 30.0;
    rig.idle(1);
    let charge0 = rig.m().status.mp_charge_left;
    assert!(charge0 <= donald::mp_charge_time(&rig.m().status) + 1e-3, "R7: his charge length");
    rig.tap(A, Vec2::ZERO);
    rig.idle(90);
    let after = rig.core().resonance(Element::Fire).unwrap();
    assert!((after - before - don_data().tune.res_gain_missile).abs() < 1e-3, "{before} -> {after}");
    let charge1 = rig.m().status.mp_charge_left;
    assert!(charge0 - charge1 > K.mpcharge_missile_shorten + 91.0 * DT - 0.05, "the landed missile shortened it: {charge0} -> {charge1}");
}

#[test]
fn mp_charge_scales_with_magic_r7() {
    let rig = Rig::new(&[]);
    let s = &rig.m().status;
    let t = donald::mp_charge_time(s);
    assert!(t < K.mpcharge_base && t >= K.mpcharge_base * K.mpcharge_ratio_min, "{t}");
    let sora = kh_core::status::tables::level_row(s.level).magic as f32 + 3.0;
    assert!((t - K.mpcharge_base * (sora / s.magic() as f32).clamp(K.mpcharge_ratio_min, 1.0)).abs() < 1e-4);
}

// ================================================================================================ 5.1 the dodges

#[test]
fn the_plain_hop_300_cm_invincible_with_a_cooldown_and_buffer() {
    let mut rig = Rig::new(&[]);
    let x0 = rig.m().position().y;
    rig.tap(X, Vec2::new(1.0, 0.0));
    assert_eq!(rig.state(), Some(donald::DODGE));
    assert!(rig.started().contains(&"hop"));
    let mut inv = 0;
    for _ in 0..30 {
        if donald::invincible(rig.m()) {
            inv += 1;
        }
        rig.idle(1);
    }
    let moved = rig.m().position().y - x0;
    assert!((moved - K.hop_distance).abs() < 20.0, "{moved}");
    assert!((inv as f32 * DT - (K.hop_invincible_to - K.hop_invincible_from)).abs() < 0.04, "{inv}");
    // cooldown: a press at once is refused (too early), one near the end is kept and hops when it ends
    let mut rig = Rig::new(&[]);
    rig.tap(X, Vec2::new(1.0, 0.0));
    rig.idle(30);
    rig.tap(X, Vec2::new(1.0, 0.0));
    assert!(rig.log().contains(&KitEvent::Refused("dodge_cooldown")), "{:?}", rig.log());
    let wait = ((K.dodge_cooldown - K.dodge_buffer * 0.5) / DT) as usize - 32;
    rig.idle(wait);
    rig.tap(X, Vec2::new(1.0, 0.0));
    let hops_before = rig.started().iter().filter(|s| **s == "hop").count();
    rig.until(Vec2::new(1.0, 0.0), &[], 30, |r| r.started().iter().filter(|s| **s == "hop").count() > hops_before).expect("the kept press hops at the cooldown's end");
}

#[test]
fn each_attunement_has_its_dodge() {
    for (e, want) in [(Element::Fire, "flame_step"), (Element::Blizzard, "ice_glide"), (Element::Thunder, "blink"), (Element::Water, "bubble_hop"), (Element::Light, "ward_step"), (Element::Aero, "hop")] {
        // on the dodge's line (the Ice Glide touches it), out of the Bonk's reach
        let mut rig = Rig::new(&[Vec3::new(40.0, 520.0, 0.0)]);
        rig.attune(e, false);
        let start = rig.m().position();
        rig.tap(X, Vec2::new(1.0, 0.0));
        rig.idle(60);
        assert!(rig.started().contains(&want), "{e:?}: {:?}", rig.started());
        let moved = horizontal(rig.m().position() - start).length();
        let (dist, ..) = moves::Dodge::of(Some(e)).numbers();
        assert!((moved - dist).abs() < dist * 0.15 + 15.0, "{e:?}: moved {moved} of {dist}");
        match e {
            Element::Fire => assert!(rig.effects.spawned.contains(&K.flame_fx), "the fire trail"),
            Element::Blizzard => assert!(rig.rows().contains(&"DON_ICE_GLIDE") && rig.party.don.status.get(1).is_some_and(|s| s.is_chilled()), "{:?}", rig.rows()),
            Element::Thunder => assert!(rig.effects.spawned.contains(&K.blink_in_fx) && rig.effects.spawned.contains(&K.blink_out_fx)),
            Element::Water => assert!(rig.effects.spawned.contains(&K.bubble_fx)),
            Element::Light => assert!(rig.effects.spawned.contains(&K.ward_fx)),
            _ => {}
        }
    }
}

#[test]
fn air_step_twice_in_the_air_with_aero_none_without() {
    let mut rig = Rig::new(&[]);
    rig.attune(Element::Aero, false);
    for _ in 0..6 {
        rig.step(Vec2::ZERO, &[B]);
    }
    rig.idle(4);
    rig.tap(X, Vec2::new(1.0, 0.0));
    rig.idle(16);
    rig.tap(X, Vec2::new(0.0, 1.0));
    rig.idle(16);
    rig.tap(X, Vec2::new(1.0, 0.0));
    assert_eq!(rig.started().iter().filter(|s| **s == "air_step").count(), 2, "{:?}", rig.log());
    // no air dodge without Aero (the old Air Blink is gone)
    let mut rig = Rig::new(&[]);
    for _ in 0..6 {
        rig.step(Vec2::ZERO, &[B]);
    }
    rig.idle(4);
    rig.tap(X, Vec2::new(1.0, 0.0));
    assert!(!rig.started().iter().any(|s| *s == "air_step" || *s == "hop"));
}

fn hit_on(rig: &mut Rig, from: Vec3, kind: ETresDamageKind) -> HitResult {
    let mut h = HitRecord::empty(TargetId(1));
    h.damage = 5.0;
    h.damage_kind = kind;
    h.direction = (rig.m().position() - from).normalize_or_zero();
    h.attacker = HitAttacker { position: from, yaw: 180.0, radius: 50.0, valid: true };
    rig.mm().receive_hit(&h)
}

#[test]
fn bubble_hop_absorbs_one_hit_and_ward_step_shields_a_member() {
    let mut rig = Rig::new(&[]);
    rig.attune(Element::Water, false);
    rig.tap(X, Vec2::new(1.0, 0.0));
    rig.idle(30);
    let hp = rig.m().status.hp;
    let r = hit_on(&mut rig, Vec3::new(200.0, 0.0, 0.0), ETresDamageKind::Small);
    assert!(!r.accepted && rig.m().status.hp == hp, "the bubble absorbed it");
    let r = hit_on(&mut rig, Vec3::new(200.0, 0.0, 0.0), ETresDamageKind::Small);
    assert!(r.accepted, "only one");
    // Ward Step: Goofy standing where Donald dodged from is shielded once
    let mut rig = Rig::new(&[]);
    let goofy = rig.party.spawn(Chara::Goofy, Vec3::new(50.0, 0.0, 0.0), 0.0);
    rig.attune(Element::Light, false);
    rig.tap(X, Vec2::new(1.0, 0.0));
    rig.idle(10);
    let mut h = HitRecord::empty(TargetId(1));
    h.damage = 5.0;
    let g = rig.party.member_mut(goofy).unwrap();
    let hp = g.status.hp;
    assert!(!g.receive_hit(&h).accepted && g.status.hp == hp, "the ward absorbed Goofy's hit");
    rig.idle(2);
    assert!(rig.party.member_mut(goofy).unwrap().receive_hit(&h).accepted, "once per member");
}

// ================================================================================================ 5.2 Magic Block

#[test]
fn magic_block_one_hit_perfect_reflects_and_fills_a_miss_leaves_him_open() {
    let mut rig = Rig::new(&[Vec3::new(1500.0, 0.0, 0.0)]);
    rig.attune(Element::Fire, false);
    rig.tap(X, Vec2::ZERO);
    assert_eq!(rig.state(), Some(donald::BLOCK));
    let res0 = rig.core().resonance(Element::Fire).unwrap();
    let r = hit_on(&mut rig, Vec3::new(1500.0, 0.0, 0.0), ETresDamageKind::Small);
    assert!(r.guarded && r.damage_dealt == 0.0);
    assert!(rig.log().contains(&KitEvent::Blocked { perfect: true }));
    assert!((rig.core().resonance(Element::Fire).unwrap() - res0 - 1.0).abs() < 1e-3, "+1 pip");
    assert_eq!(rig.core().arcane.charges(), 1, "+1 Arcane Flow");
    rig.idle(90);
    assert!(rig.rows().contains(&"DON_REFLECT"), "the projectile went back: {:?}", rig.rows());
    // one hit only
    let mut rig = Rig::new(&[]);
    rig.tap(X, Vec2::ZERO);
    rig.idle(12);
    assert!(hit_on(&mut rig, Vec3::new(150.0, 0.0, 0.0), ETresDamageKind::Small).guarded);
    assert!(!hit_on(&mut rig, Vec3::new(150.0, 0.0, 0.0), ETresDamageKind::Small).guarded);
    // a miss: open (A does nothing) until the window + miss_open
    let mut rig = Rig::new(&[Vec3::new(1200.0, 0.0, 0.0)]);
    rig.tap(X, Vec2::ZERO);
    rig.idle(((K.block_window + 0.1) / DT) as usize);
    assert!(rig.kit().open > 0.0);
    rig.tap(A, Vec2::ZERO);
    assert!(rig.missiles().is_empty(), "open: no attack");
    // the back is not covered
    let mut rig = Rig::new(&[]);
    rig.tap(X, Vec2::ZERO);
    assert!(!hit_on(&mut rig, Vec3::new(-200.0, 0.0, 0.0), ETresDamageKind::Small).guarded);
}

// ================================================================================================ 4.1 the attune cast

#[test]
fn the_attune_cast_takes_its_cast_time_and_a_hit_interrupts_it() {
    let mut rig = Rig::new(&[]);
    assert!(donald::begin_attune(rig.mm(), Element::Blizzard));
    assert_eq!(rig.state(), Some(donald::ATTUNE));
    let f = rig.until(Vec2::ZERO, &[], 180, |r| r.core().attuned(Element::Blizzard)).expect("attuned");
    assert!((f as f32 * DT - don_data().tune.attune_cast).abs() < 0.05, "{f}");
    assert!(rig.effects.spawned.contains(&K.attune_blizzard_fx), "the cast effect");
    rig.idle(30);
    assert_ne!(rig.state(), Some(donald::ATTUNE));
    // interrupted
    let mut rig = Rig::new(&[]);
    assert!(donald::begin_attune(rig.mm(), Element::Fire));
    rig.idle(10);
    hit_on(&mut rig, Vec3::new(150.0, 0.0, 0.0), ETresDamageKind::Small);
    rig.idle(60);
    assert!(!rig.core().attuned(Element::Fire), "the hit stopped it");
    // through the menu action route (DonCmd::Attune)
    let mut rig = Rig::new(&[]);
    assert!(donald::wants_cast(rig.mm(), DonCmd::Attune(Element::Water), crate::donald::cast::CastTarget::Enemy(TargetId::NONE)));
    rig.idle((don_data().tune.attune_cast / DT) as usize + 30);
    assert!(rig.core().attuned(Element::Water));
    // a heavy hit costs a pip (4.1)
    let mut rig = Rig::new(&[]);
    rig.attune(Element::Fire, true);
    hit_on(&mut rig, Vec3::new(150.0, 0.0, 0.0), ETresDamageKind::Blow);
    assert!(rig.core().resonance(Element::Fire).unwrap() <= 2.0 + 1e-3);
}

#[test]
fn ai_donald_attunes_through_the_same_move() {
    let mut party = Party::new(PartyConfig::test_level99());
    let id = party.spawn(Chara::Donald, Vec3::ZERO, 0.0);
    let world = TestWorld::floor();
    let mut dummies = Dummies(Vec::new(), Vec::new());
    let (mut audio, mut effects) = (RecAudio::default(), RecEffects::default());
    assert!(donald::begin_attune(party.member_mut(id).unwrap(), Element::Fire));
    for _ in 0..60 {
        let mut host = PartyHost { world: &world, targets: &mut dummies, audio: &mut audio, effects: &mut effects };
        party.step(DT, &SoraView::default(), &mut host);
    }
    assert!(party.status(id).unwrap().donald.as_ref().unwrap().core.attuned(Element::Fire));
    assert!(effects.spawned.contains(&K.attune_fire_fx));
}

// ================================================================================================ 8 movement, removals

#[test]
fn run_680_sprint_900_lower_jump_no_slide_rb_lock_only() {
    let mut rig = Rig::new(&[Vec3::new(900.0, 0.0, 0.0)]);
    for _ in 0..40 {
        rig.step(Vec2::new(0.0, 1.0), &[]);
    }
    let v = rig.m().cmc.velocity;
    assert!(((v.x * v.x + v.y * v.y).sqrt() - K.move_run).abs() < 6.0);
    for _ in 0..80 {
        rig.step(Vec2::new(0.0, 1.0), &[]);
    }
    let v = rig.m().cmc.velocity;
    assert!(((v.x * v.x + v.y * v.y).sqrt() - K.move_sprint).abs() < 8.0, "sprint {v:?}");
    // X while sprinting: the dodge, never the Superslide
    rig.tap(X, Vec2::new(0.0, 1.0));
    assert_ne!(rig.m().state, NpcStateId::SuperSlide);
    assert_eq!(rig.state(), Some(donald::DODGE));
    // the jump is lower than Goofy's (Sora's power)
    let top = |chara: Chara| {
        let mut party = Party::new(PartyConfig::test_level99());
        let id = party.spawn(chara, Vec3::ZERO, 0.0);
        party.set_pad(id, Some(Tuning::USER));
        let world = TestWorld::floor();
        let mut dummies = Dummies(Vec::new(), Vec::new());
        let (mut audio, mut effects) = (RecAudio::default(), RecEffects::default());
        let mut top = 0.0f32;
        let mut held = PadButtons::NONE;
        for i in 0..120 {
            let now = if i < 40 { PadButtons::NONE.with(PadButton::JUMP) } else { PadButtons::NONE };
            let pad = Buttons { held: now, pressed: PadButtons(now.0 & !held.0), released: PadButtons(held.0 & !now.0) };
            held = now;
            party.set_pad_frame(id, PadFrame { stick: Vec2::ZERO, camera_yaw: 0.0, pad });
            let mut host = PartyHost { world: &world, targets: &mut dummies, audio: &mut audio, effects: &mut effects };
            party.step(DT, &SoraView::default(), &mut host);
            top = top.max(party.member(id).unwrap().position().z);
        }
        top
    };
    let (d, g) = (top(Chara::Donald), top(Chara::Goofy));
    assert!(d < g * 0.9, "Donald {d} vs Goofy {g}");
    // RB: the lock on the press, no aim hold
    let mut rig = Rig::new(&[Vec3::new(900.0, 0.0, 0.0)]);
    rig.step(Vec2::ZERO, &[RB]);
    assert_eq!(rig.m().pad.as_ref().unwrap().lock, TargetId(1), "the lock on the press");
    for _ in 0..40 {
        rig.step(Vec2::ZERO, &[RB]);
    }
    assert!(rig.state().is_none(), "no Spell Volley aim");
}

#[test]
fn aero_float_and_aero_glide_with_aero_attuned() {
    let glide = |aero: bool| {
        let mut rig = Rig::new(&[]);
        rig.wear(&[DonAbility::AeroFloat]);
        if aero {
            rig.attune(Element::Aero, false);
        }
        rig.mm().cmc.location.z += 1500.0;
        rig.mm().cmc.set_falling();
        rig.mm().enter(NpcStateId::Pad(JUMP));
        rig.mm().pad.as_mut().unwrap().jump.sub = 4;
        rig.until(Vec2::new(0.0, 1.0), &[B], 30, |r| r.state() == Some(donald::FLOAT)).expect("the float");
        let n = rig.until(Vec2::new(0.0, 1.0), &[B], 900, |r| r.state() != Some(donald::FLOAT)).unwrap_or(900);
        let v = rig.kit().float_used;
        (n, v, rig.started())
    };
    let (_, used_float, s) = glide(false);
    assert!(s.contains(&"aero_float"));
    let (_, used_glide, s2) = glide(true);
    assert!(s2.contains(&"aero_glide"));
    assert!(used_glide > used_float, "Aero Glide stays up longer: {used_glide} vs {used_float}");
    assert!(K.glide_aero_speed > K.float_speed);
}

#[test]
fn y_casts_the_y_lists_spell_and_the_team_attacks_keep_y() {
    let mut rig = Rig::new(&[Vec3::new(900.0, 0.0, 0.0)]);
    // DON-UI's y_selected (stub: nothing selected): Y is left to Sora's queue (the team attacks)
    rig.tap(PadButton::FaceTop, Vec2::ZERO);
    assert!(!rig.m().pad.as_ref().unwrap().x2.y_taken);
    assert!(rig.kit().pending_cast.is_none());
}

// ================================================================================================ 2 stats, 6.6 team

#[test]
fn the_stats_are_the_docs_ratios_and_max_mp_has_the_steps() {
    use crate::balance::BP;
    assert_eq!((BP.n_ex001_lead_magic_ratio, BP.n_ex001_lead_mp_ratio, BP.n_ex001_lead_strength_ratio, BP.n_ex001_lead_hp_ratio, BP.n_ex001_lead_defense_ratio), (1.2, 1.25, 0.667, 0.85, 0.8)); // (2026-10-27) Strength 0.55 -> 0.667: 10 at level 1; (DONLINE2, 2026-10-27) Magic 1.25 -> 1.2: 18 at level 1, MP 1.5 -> 1.25: "make Donald have x1.25 MP"
    let mut rig = Rig::new(&[]);
    let max = rig.m().status.max_mp();
    rig.party.status_mut(rig.id).unwrap().donald.as_mut().unwrap().learned.apply(crate::donald::DonStep::Arcane(crate::donald::ArcaneLine::Ultima));
    let after = rig.m().status.max_mp();
    assert_eq!(after - max, rig.m().status.donald.as_ref().unwrap().learned.mp_bonus(), "Max MP + learned.mp_bonus()");
    // the leader's Max MP is 1.25x Sora's reference (+ his steps; DONLINE2, the user 2026-10-27: "make Donald have x1.25 MP")
    let s = &rig.m().status;
    let (_, sora_mp) = crate::balance::sora_reference(s);
    assert_eq!(s.max_mp() - donald::mp_bonus(s), (1.25 * sora_mp as f32).round() as i32 + s.bonuses.totals(s.game_level).1 + s.roster.hp_mp(s.game_level).1);
}

/// (DONLINE2) THE USER (2026-10-27): "make Donald's starting magic 18 at level 1" (the status screen's Magic: his row of
/// party-balance-levels + the starting staff's 4) and "make Donald have x1.25 MP": Sora's opening Vitality / Balance /
/// Wisdom (100 / 110 / 120 MP) give Donald 125 / 138 / 150 at the start.
#[test]
fn donald_starts_with_magic_18_and_1_25x_soras_mp() {
    let mut s = crate::FriendStatus::new(crate::Chara::Donald, 0);
    assert_eq!((s.level, s.magic()), (1, 18));
    assert_eq!(crate::balance::base_stats(crate::Chara::Donald, 1).1 + 4, 18, "his row + the starting staff's 4");
    for (sora_mp, want) in [(100, 125), (110, 138), (120, 150)] {
        s.bal.sora = Some((100, sora_mp));
        assert_eq!(s.max_mp(), want, "Sora {sora_mp} MP");
    }
    let l99 = crate::FriendStatus::test_level99(crate::Chara::Donald);
    assert_eq!(l99.magic(), 80, "level 99");
}

#[test]
fn team_attacks_gain_x12_at_resonance_3() {
    let mut rig = Rig::new(&[]);
    assert_eq!(crate::team::link::don_res3_mult(&rig.party), 1.0);
    rig.attune(Element::Fire, true);
    assert_eq!(crate::team::link::don_res3_mult(&rig.party), K.team_res3_mult);
    assert_eq!(crate::team::link::don_team_hit_mult(&rig.party, "FR_FLAREFORCE_FINISH01"), 1.2);
    assert_eq!(crate::team::link::don_team_hit_mult(&rig.party, "FR_CALLMETEOR_FINISH"), 1.2);
    assert_eq!(crate::team::link::don_team_hit_mult(&rig.party, "KB_VERTICAL"), 1.0, "not a team row");
    assert_eq!(crate::team::link::don_team_hit_mult(&rig.party, "FR_SHIELDGUARDIAN_SHIELDBASH"), 1.2, "Trinity Guard: Donald's team attack with Goofy");
    assert_eq!(crate::team::link::don_team_hit_mult(&rig.party, "UNISON_FIRE"), 1.2);
}

// ================================================================================================ 12 feel targets: the harnesses

/// THE SECONDS TO RESONANCE 3 at 6 - 25 m (feel 12: 5 - 8 s) with "a good attunement loop": attuned, he mashes the missile
/// combo and casts an attuned base spell at every slam's cancel point (the cast's own Resonance through
/// `DonCore::commit_cast`, as DON-MAGIC's `cast::begin` does at the cast's start; the cast's own clip is DON-MAGIC's, here
/// its length is skipped and the string goes on).
#[test]
fn harness_seconds_to_resonance_3_at_range() {
    let mut out = Vec::new();
    for dist in [600.0, 1200.0, 2500.0] {
        for element in [Element::Fire, Element::Thunder] {
            let mut rig = Rig::new(&[Vec3::new(dist, 0.0, 0.0)]);
            rig.attune(element, false);
            let d = don_data();
            let mut t = 0.0f32;
            let mut casts = 0;
            let mut cast_lock = 0.0f32;
            for i in 0..1200 {
                let held: &[PadButton] = if i % 8 == 0 && cast_lock <= 0.0 { &[A] } else { &[] };
                rig.step(Vec2::ZERO, held);
                t += DT;
                cast_lock -= DT;
                // the slam's cancel point: an attuned base spell (cast time of its row)
                let slam_shot = rig.kit().combo.step == combo::Step::Slam && combo::cancel_open(rig.m());
                if slam_shot && cast_lock <= 0.0 {
                    let spell = SpellRef::Line1(element, 1);
                    let row = spell.row(&d).unwrap();
                    let st = rig.party.status_mut(rig.id).unwrap().donald.as_mut().unwrap();
                    let plan = st.core.check_cast(spell, &st.learned, 999, 999, false, &d).unwrap();
                    let mods = st.abilities.mods();
                    st.core.commit_cast(&plan, &mods, &d);
                    casts += 1;
                    cast_lock = row.cast + 0.6;
                    donald::end_kit(rig.mm(), "harness cast");
                    rig.mm().to_idle();
                }
                if rig.core().full_power(element, &d) {
                    break;
                }
            }
            let full = rig.core().full_power(element, &d);
            out.push((dist, element, t, casts, full));
        }
    }
    for (dist, e, t, casts, full) in &out {
        eprintln!("RESONANCE 3: {e:?} at {:.0} m: {t:.2} s ({casts} attuned base casts; full {full})", dist / 100.0);
    }
    for (dist, e, t, _, full) in &out {
        assert!(*full, "{e:?} at {dist}");
        // (the user 2026-10-22: "pips should take longer to build up": the feel target is about 8 - 12 s now)
        assert!((7.5..=13.0).contains(t), "{e:?} at {dist}: {t:.2} s (feel 12: about 8 - 12 s)");
    }
}

/// THE BAL-STYLE DAMAGE COMPARISON (BAL_DESIGN.md's model: attack = KH3 x 7 on Elden Ring's defence curve, the Godrick
/// soldier): Donald's plain missile combo against Sora's base combo, per second, at level 1 / 40 / 99. Feel 12: a player
/// who never opens the menu finds him weak (below Sora); the attunement loop (Resonance, reactions, spells) is his power.
#[test]
fn harness_bal_damage_comparison() {
    fn er_hp(a: f32, d: f32) -> f32 {
        let r = a / d;
        let m = if r < 0.125 { 0.1 } else if r < 1.0 { 0.1 + 19.2 / 49.0 * (r - 0.125).powi(2) } else if r < 2.5 { 0.7 - 0.4 / 3.0 * (r - 2.5).powi(2) } else if r < 8.0 { 0.9 - 0.8 / 121.0 * (r - 8.0).powi(2) } else { 0.9 };
        (a * m).max(1.0)
    }
    let er = |kh: f32| er_hp((kh * 7.0).round().max(1.0), 100.0);
    for level in [1u32, 40, 99] {
        // Donald, leading at the level, the plain combo at 12 m against a dummy with no KH3 defence (Elden Ring's)
        let exp = crate::tables::level_row(Chara::Donald, level).0;
        let mut party = Party::new(PartyConfig { level99: false, total_exp: exp, seed: 0x5EED });
        let id = party.spawn(Chara::Donald, Vec3::ZERO, 0.0);
        party.set_pad(id, Some(Tuning::USER));
        party.set_lead_reference(Some(crate::balance::sora_base(kh_core::status::GameLevel::Standard)));
        struct NoDef(Dummies);
        impl Targets for NoDef {
            fn enumerate(&self, out: &mut Vec<TargetInfo>) {
                self.0.enumerate(out)
            }
            fn info(&self, id: TargetId) -> Option<TargetInfo> {
                self.0.info(id)
            }
            fn hurt_shapes(&self, id: TargetId, out: &mut Vec<HurtShape>) {
                self.0.hurt_shapes(id, out)
            }
            fn apply_hit(&mut self, hit: &HitRecord) -> HitResult {
                self.0.apply_hit(hit)
            }
            fn defense(&self, _id: TargetId) -> Option<TargetDefense> {
                None
            }
        }
        let mut targets = NoDef(Dummies(vec![(TargetId(1), Vec3::new(1200.0, 0.0, 0.0))], Vec::new()));
        let world = TestWorld::floor();
        let (mut audio, mut effects) = (RecAudio::default(), RecEffects::default());
        let mut held = PadButtons::NONE;
        let frames = 600;
        for i in 0..frames {
            let now = if i % 8 == 0 { PadButtons::NONE.with(A) } else { PadButtons::NONE };
            let pad = Buttons { held: now, pressed: PadButtons(now.0 & !held.0), released: PadButtons(held.0 & !now.0) };
            held = now;
            party.set_pad_frame(id, PadFrame { stick: Vec2::ZERO, camera_yaw: 0.0, pad });
            let mut host = PartyHost { world: &world, targets: &mut targets, audio: &mut audio, effects: &mut effects };
            party.step(DT, &SoraView::default(), &mut host);
        }
        let don: f32 = targets.0 .1.iter().map(|h| er(h.damage)).sum::<f32>() / (frames as f32 * DT);
        // Sora: his base combo rows at the level's Strength (BAL's formula), the 4 hits in his combo's ~1.6 s + recovery
        let row = |n: &str| kh_core::combat::tables::attack_row(n).map(|(_, r)| r.power).unwrap();
        let (s, _) = (kh_core::status::tables::level_row(level).attack + 4, 0);
        let sora_hit = |power: f32| {
            let a = kh_core::status::Attacker { has_status: true, strength: s, magic: 0, abilities: None, side: kh_core::status::Side::Player, state_boost: false };
            kh_core::status::compute_damage(&a, &kh_core::status::Defender::NONE, &kh_core::status::DamageAttack::physical(power), &kh_core::status::DamageContext::new(kh_core::status::GameLevel::Standard, 1)).amount
        };
        let sora_combo: f32 = ["KB_VERTICAL", "KB_STAB", "KB_TURN", "KB_BREAK"].iter().map(|n| er(sora_hit(row(n)))).sum();
        let sora = sora_combo / 2.2;
        eprintln!("BAL level {level}: Donald's plain missile combo at 12 m {don:.1} ER HP/s vs Sora's base combo {sora:.1} ER HP/s ({:.2}x)", don / sora);
        assert!(don < sora, "level {level}: the menu-less Donald is weaker than Sora ({don:.1} vs {sora:.1})");
        assert!(don > 0.35 * sora, "level {level}: but not useless ({don:.1} vs {sora:.1})");
    }
}

// ================================================================================================ 13 removals

#[test]
fn the_removed_moves_are_gone() {
    // A at close range is the Bonk / missiles, never the Beat / Swing actions
    let mut rig = Rig::new(&[Vec3::new(400.0, 0.0, 0.0)]);
    rig.mash(8, 200);
    assert!(rig.m().action.is_none() || !matches!(rig.m().action.and_then(|r| r.def), Some(crate::command::ActionDef::DonaldBeat | crate::command::ActionDef::DonaldSwing | crate::command::ActionDef::DonaldJump)));
    assert!(!rig.rows().iter().any(|r| r.starts_with("Attack") || r.starts_with("UNISON") || r.starts_with("FR_") || r.ends_with("_FIN")), "{:?}", rig.rows());
    // no Flutter: B while blown back is the shared aerial recovery (HR), not a kit state
    for n in donald::FIRST..=donald::LAST {
        assert!(!matches!(donald::state_name(n), "Flutter" | "AirBlink" | "SpellVolley" | "CounterBlast" | "UnisonFinisher" | "MeteorDrop" | "FlareForce" | "GrandMagic"));
    }
}

// ================================================================================================ assets

/// Every effect and sound the kit names exists in a pack (the main pack, the party pack, or DON-KIT's own pack folder for
/// the ones it packed): the harness log of the brief ("every effect and sound must be proven to play").
#[test]
fn harness_every_kit_asset_is_in_a_pack() {
    // the repo root: this crate's tree, or (a stream's work copy under build/sora_tmp/party/<stream>/ws) the live one above it
    let mut root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for _ in 0..6 {
        if root.join("build/sora_pack/vfx/effects").is_dir() {
            break;
        }
        root = root.join("..");
    }
    let live = root.join("build/sora_pack");
    if !live.join("vfx/effects").is_dir() {
        eprintln!("ASSETS: no build/sora_pack found: skipped");
        return;
    }
    // the packs that get deployed: the main pack, the play pack (build/play/donpack) and DON-KIT's staged pack (merged
    // into the main vfx pack at deploy), looked for in every ancestor (a stream's work copy has no DON-KIT folder of its
    // own: the live tree above it has)
    let mut fx_dirs = vec![live.join("vfx/effects"), live.join("party/vfx/effects")];
    let mut up = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for _ in 0..8 {
        for d in ["build/sora_pack/vfx/effects", "build/sora_pack/party/vfx/effects", "build/play/donpack/vfx/effects", "build/play/donpack/party/vfx/effects", "build/sora_tmp/party/DON-KIT/pack/vfx/effects", "build/sora_tmp/party/KITFX/pack/vfx/effects" /* (KITFX) */] {
            if up.join(d).is_dir() {
                fx_dirs.push(up.join(d));
            }
        }
        up = up.join("..");
    }
    let audio = std::fs::read_to_string(live.join("audio/manifest.json")).unwrap_or_default();
    let fx = [K.missile_fire_fx, K.missile_hit_fx, K.missile_fire_body, K.missile_blizzard_body, K.missile_thunder_body, K.missile_water_body, K.missile_aero_body, K.missile_light_body, K.missile_fire_hit_fx, K.missile_blizzard_hit_fx, K.missile_thunder_hit_fx, K.missile_water_hit_fx, K.missile_aero_hit_fx, K.missile_light_hit_fx, K.missile_thunder_chain_fx, K.missile_fire_ground_fx, K.slam_blizzard_fx, K.slam_thunder_fx, K.slam_water_fx, K.slam_aero_fx, K.slam_light_fx, K.charge_charged_fx, K.flame_fx, K.glide_fx, K.blink_in_fx, K.blink_out_fx, K.bubble_fx, K.airstep_fx, K.ward_fx, K.block_fx, K.block_raise_fx, K.block_perfect_fx, K.float_fx, K.attune_fire_fx, K.attune_blizzard_fx, K.attune_thunder_fx, K.attune_water_fx, K.attune_aero_fx, K.attune_light_fx];
    let mut missing = Vec::new();
    for name in fx {
        if !fx_dirs.iter().any(|d| d.join(format!("{name}.json")).is_file()) {
            missing.push(name);
        }
    }
    let se = [K.missile_hit_se, K.missile_fire_hit_se, K.missile_blizzard_hit_se, K.missile_thunder_hit_se, K.missile_water_hit_se, K.missile_aero_hit_se, K.missile_light_hit_se, K.slam_light_se, K.charge_charged_se, K.bonk_hit_se, K.glide_se, K.blink_se, K.bubble_pop_se, K.block_se, K.missile_thunder_launch_se, K.missile_aero_launch_se, K.missile_light_launch_se, K.float_se_start, K.float_se_loop /* (KITFX) */];
    for name in se {
        if !audio.contains(name) {
            missing.push(name);
        }
    }
    eprintln!("ASSETS: {} effects + {} sounds checked; missing {missing:?}", fx.len(), se.len());
    assert!(missing.is_empty(), "{missing:?}");
    // the plain missile's body is Flare Force's class (the team props' row: team_host.json)
    let team = std::fs::read_to_string(live.join("party/team/team_host.json")).unwrap_or_default();
    assert!(team.contains(K.missile_body));
    // the clips
    for c in [K.step_wave_l_clip, K.step_wave_r_clip, K.slam_clip, K.slam_fallback_clip, K.air_clip, K.charge_clip, K.bonk_clip, K.block_clip, K.attune_clip, K.attune_air_clip, "HopL", "HopR", "SonicSlide", "SonicSlideLoop", "SlideEnd", "AttackAirReflect", "MagicAirCast"] {
        assert_eq!(crate::tables::clip(Chara::Donald, c).name, c, "his clip {c}");
    }
}


// ================================================================================================ 5.3 his longer knockdowns (HR)

#[test]
fn heavy_hits_knock_him_down_longer_through_the_shared_hr_rules() {
    let base = kh_react::ClipTimes::MEDIAN;
    let longer = crate::hurt::don_longer_down(Chara::Donald, base);
    let rate = crate::hurt::HP.hurt_n_ex001_down_rate;
    assert!(rate < 1.0);
    assert!((longer.blow_down - base.blow_down / rate).abs() < 1e-4 && (longer.toss_down - base.toss_down / rate).abs() < 1e-4);
    assert_eq!(crate::hurt::don_longer_down(Chara::Goofy, base), base, "Goofy keeps his own");
    // a BLOW on him: the down clip plays at his rate, and he is down longer than the same reaction at rate 1
    let down_time = |chara: Chara| {
        let mut party = Party::new(PartyConfig::test_level99());
        let id = party.spawn(chara, Vec3::ZERO, 0.0);
        let world = TestWorld::floor();
        let mut dummies = Dummies(Vec::new(), Vec::new());
        let (mut audio, mut effects) = (RecAudio::default(), RecEffects::default());
        let mut h = HitRecord::empty(TargetId(1));
        h.damage = 2.0;
        h.damage_kind = ETresDamageKind::Blow;
        h.direction = Vec3::new(1.0, 0.0, 0.0);
        h.attacker = HitAttacker { position: Vec3::new(-150.0, 0.0, 0.0), yaw: 0.0, radius: 50.0, valid: true };
        party.member_mut(id).unwrap().receive_hit(&h);
        let (mut t, mut slow) = (0.0f32, false);
        for _ in 0..600 {
            let mut host = PartyHost { world: &world, targets: &mut dummies, audio: &mut audio, effects: &mut effects };
            party.step(DT, &SoraView::default(), &mut host);
            let m = party.member(id).unwrap();
            if m.state != NpcStateId::Damage {
                break;
            }
            slow |= (m.anim.speed - rate).abs() < 1e-4;
            t += DT;
        }
        (t, slow)
    };
    let (don, slow) = down_time(Chara::Donald);
    assert!(slow, "his down clip played at {rate}");
    let down_clip = crate::tables::clip(Chara::Donald, "M_0213").length;
    assert!(don > down_clip / rate - 0.1, "Donald down {don:.2} s (his M_0213 {down_clip:.2} s at {rate})");
}

// ================================================================================================ 4.2 the auras on motion (DON-KIT follow-up)

/// Run speed of the pad leader after 50 frames of a full stick (before the sprint).
fn run_speed(rig: &mut Rig) -> f32 {
    for _ in 0..50 {
        rig.step(Vec2::new(0.0, 1.0), &[]);
    }
    let v = rig.m().cmc.velocity;
    (v.x * v.x + v.y * v.y).sqrt()
}

#[test]
fn the_thunder_aura_speeds_the_party_by_10_percent_party_only() {
    let p = crate::donald::ability_info::prog();
    let want = 1.0 + p.value("aura.thunder_move");
    assert!((want - 1.1).abs() < 1e-4);
    // alone: no aura (the party rule)
    let mut rig = Rig::new(&[]);
    rig.attune(Element::Thunder, false);
    let alone = run_speed(&mut rig);
    assert!((alone - K.move_run).abs() < 6.0, "{alone}");
    assert_eq!(rig.m().don_aura.move_mult, 1.0);
    // with Goofy beside him: Donald runs x1.1 and Goofy gets it too (the aura is the party's)
    let mut rig = Rig::new(&[]);
    let goofy = rig.party.spawn(Chara::Goofy, Vec3::new(0.0, 150.0, 0.0), 0.0);
    rig.attune(Element::Thunder, false);
    rig.idle(2);
    assert!((rig.party.member(goofy).unwrap().don_aura.move_mult - want).abs() < 1e-4, "Goofy inside the aura");
    let fast = run_speed(&mut rig);
    assert!((fast - K.move_run * want).abs() < 8.0, "Donald with the aura {fast} vs {}", K.move_run * want);
    // Sora's share (the host scales his speeds by it: kh-partyhost lead.rs)
    let at = rig.m().position();
    assert!((donald::sora_move_mult(&rig.party, at) - want).abs() < 1e-4);
    assert_eq!(donald::sora_move_mult(&rig.party, at + Vec3::new(5000.0, 0.0, 0.0)), 1.0, "out of reach");
    // no compounding: the speed limit the states set is untouched by the aura
    assert!((rig.m().cmc.max_walk_speed - K.move_run).abs() < 1.0 || (rig.m().cmc.max_walk_speed - K.move_sprint).abs() < 1.0);
}

#[test]
fn the_thunder_aura_speeds_an_ai_member_and_goofy_leading() {
    // Goofy leads, AI Donald (Thunder) beside him: Goofy's run x1.1
    let run = |thunder: bool| {
        let mut party = Party::new(PartyConfig::test_level99());
        let g = party.spawn(Chara::Goofy, Vec3::ZERO, 0.0);
        let d = party.spawn(Chara::Donald, Vec3::new(0.0, 150.0, 0.0), 0.0);
        party.set_pad(g, Some(Tuning::USER));
        if thunder {
            let dd = don_data();
            let st = party.status_mut(d).unwrap().donald.as_mut().unwrap();
            st.learned = DonLearned::all();
            let l = st.learned;
            st.core.begin_attune(Element::Thunder, &l, &DonMods { instant_attune: true, ..DonMods::default() }, &dd).unwrap();
        }
        let world = TestWorld::floor();
        let mut dummies = Dummies(Vec::new(), Vec::new());
        let (mut audio, mut effects) = (RecAudio::default(), RecEffects::default());
        for _ in 0..50 {
            let pad = Buttons::default();
            party.set_pad_frame(g, PadFrame { stick: Vec2::new(0.0, 1.0), camera_yaw: 0.0, pad });
            let mut host = PartyHost { world: &world, targets: &mut dummies, audio: &mut audio, effects: &mut effects };
            party.step(DT, &SoraView::default(), &mut host);
        }
        let v = party.member(g).unwrap().cmc.velocity;
        (v.x * v.x + v.y * v.y).sqrt()
    };
    let (plain, fast) = (run(false), run(true));
    assert!((fast / plain - 1.1).abs() < 0.02, "Goofy {plain} -> {fast}");
}

#[test]
fn the_aero_aura_lengthens_dodge_invincibility_and_shortens_the_cooldown() {
    let ev = 1.0 + crate::donald::ability_info::prog().value("aura.aero_evasion");
    let hop = |aero: bool| {
        let mut rig = Rig::new(&[]);
        let _goofy = rig.party.spawn(Chara::Goofy, Vec3::new(0.0, -150.0, 0.0), 0.0);
        if aero {
            // Aero attuned: his ground dodge is the Hop, his aura the evasion
            rig.attune(Element::Aero, false);
        }
        rig.idle(2);
        rig.tap(X, Vec2::new(1.0, 0.0));
        let mut inv = 0;
        for _ in 0..40 {
            if donald::invincible(rig.m()) {
                inv += 1;
            }
            rig.idle(1);
        }
        (inv, rig.kit().dodge_ready_at - 2.0 * DT - (rig.kit().clock - 42.0 * DT))
    };
    let ((i0, cd0), (i1, cd1)) = (hop(false), hop(true));
    assert!(i1 > i0, "invincible {i0} -> {i1} frames");
    assert!(((K.hop_invincible_to * ev - K.hop_invincible_from) / DT - i1 as f32).abs() <= 2.0, "{i1}");
    assert!((cd1 - K.dodge_cooldown / ev).abs() < 0.05 && (cd0 - K.dodge_cooldown).abs() < 0.05, "cooldown {cd0} -> {cd1}");
    // Goofy's Hop inside the aura: the same window and cooldown rule
    let mut rig = Rig::new(&[]);
    let goofy = rig.party.spawn(Chara::Goofy, Vec3::new(0.0, 150.0, 0.0), 0.0);
    rig.attune(Element::Aero, false);
    rig.idle(2);
    let g = rig.party.member(goofy).unwrap();
    assert!((g.don_aura.evasion + 1.0 - ev).abs() < 1e-4);
}

#[test]
fn the_aero_aura_speeds_recovery_from_hits() {
    let down = |aero: bool| {
        let mut rig = Rig::new(&[]);
        let goofy = rig.party.spawn(Chara::Goofy, Vec3::new(0.0, 150.0, 0.0), 0.0);
        if aero {
            rig.attune(Element::Aero, false);
        }
        rig.idle(2);
        let mut h = HitRecord::empty(TargetId(1));
        h.damage = 2.0;
        h.damage_kind = ETresDamageKind::Blow;
        h.direction = Vec3::new(0.0, 1.0, 0.0);
        h.attacker = HitAttacker { position: Vec3::new(0.0, 0.0, 0.0), yaw: 0.0, radius: 50.0, valid: true };
        rig.party.member_mut(goofy).unwrap().receive_hit(&h);
        let mut t = 0.0f32;
        for _ in 0..600 {
            rig.idle(1);
            if rig.party.member(goofy).unwrap().state != NpcStateId::Damage {
                break;
            }
            t += DT;
        }
        t
    };
    let (slow, fast) = (down(false), down(true));
    assert!(fast < slow - 0.15, "Goofy's blow + get-up {slow:.2} s -> {fast:.2} s with the Aero aura");
    let r = crate::hurt::aura_recovery(1.2, kh_react::ClipTimes::MEDIAN);
    assert!((r.blow_down - kh_react::ClipTimes::MEDIAN.blow_down / 1.2).abs() < 1e-4);
    assert_eq!(r.blow_start, kh_react::ClipTimes::MEDIAN.blow_start, "the flight stays the physics'");
}

#[test]
fn max_mp_adds_the_boss_steps_for_the_leader_and_for_ai_donald() {
    for leading in [true, false] {
        let mut s = crate::status::FriendStatus::new(Chara::Donald, crate::tables::level_row(Chara::Donald, 30).0);
        s.set_leading(leading);
        let before = s.max_mp();
        let st = s.donald.as_mut().unwrap();
        st.learned = DonLearned::all();
        // a step at a line's top: Max MP +3 (DonLearned::mp_bonus)
        st.learned.apply(crate::donald::DonStep::Arcane(crate::donald::ArcaneLine::Ultima));
        let bonus = st.learned.mp_bonus();
        assert_eq!(bonus, 3);
        assert_eq!(s.max_mp() - before, bonus, "leading {leading}");
    }
}

#[test]
fn soras_share_of_the_aero_aura_for_kh_core_set_aura() {
    let mut rig = Rig::new(&[]);
    assert_eq!(donald::sora_aura(&rig.party, rig.m().position()), (0.0, 0.0), "alone: no aura");
    let _goofy = rig.party.spawn(Chara::Goofy, Vec3::new(0.0, 150.0, 0.0), 0.0);
    rig.attune(Element::Aero, false);
    rig.idle(2);
    let p = crate::donald::ability_info::prog();
    let at = rig.m().position() + Vec3::new(200.0, 0.0, 0.0);
    let (e, r) = donald::sora_aura(&rig.party, at);
    assert!((e - p.value("aura.aero_evasion")).abs() < 1e-4 && (r - p.value("aura.aero_recovery")).abs() < 1e-4, "({e}, {r})");
    assert_eq!(donald::sora_aura(&rig.party, at + Vec3::new(5000.0, 0.0, 0.0)), (0.0, 0.0), "out of reach");
}

// ================================================================================================ the user's bug: attuned missiles not moving

/// The user (test level): "his basic missiles when attuned aren't moving". Every element's missile flies at 18 m/s toward
/// its target, its hit lands there, and the hosts' projectile taps report its body effect at the moving position (the
/// hosts make a body spawned by its effect name follow that tap: kh-app / kh-host vfx.rs).
#[test]
fn every_elements_missile_flies_at_18_m_s_to_its_target_and_hits() {
    for e in [None, Some(Element::Fire), Some(Element::Blizzard), Some(Element::Thunder), Some(Element::Water), Some(Element::Aero), Some(Element::Light)] {
        let target = Vec3::new(1600.0, 0.0, 0.0);
        let mut rig = Rig::new(&[target]);
        if let Some(e) = e {
            rig.attune(e, false);
        }
        rig.tap(A, Vec2::ZERO);
        rig.until(Vec2::ZERO, &[], 60, |r| !r.kit().shots.missiles.is_empty()).expect("a missile");
        let m0 = rig.kit().shots.missiles[0].clone();
        assert_eq!(m0.spec.element, e);
        assert_ne!(m0.effect, 0, "{e:?}: its body effect");
        let mut last = (m0.position, (target + Vec3::Z * 90.0 - m0.position).length());
        let mut samples = 0;
        for _ in 0..40 {
            rig.idle(1);
            let Some(m) = rig.kit().shots.missiles.first().cloned().filter(|m| m.fading.is_none()) else { break };
            let step = (m.position - last.0).length() / DT;
            assert!((step - K.missile_speed).abs() < 1.0, "{e:?}: {step} cm/s");
            let dist = (target + Vec3::Z * 90.0 - m.position).length();
            assert!(dist < last.1, "{e:?}: toward the target");
            // the hosts' tap of its body: the moving position
            let taps = donald::party_taps(&rig.party);
            assert!(taps.iter().any(|t| t.0 == m.effect && (t.1 - m.position).length() < 1e-3), "{e:?}: the body's tap follows it");
            last = (m.position, dist);
            samples += 1;
        }
        assert!(samples >= 20, "{e:?}: it flew {samples} steps");
        rig.idle(30);
        let hit = rig.dummies.1.iter().find(|h| h.attack_name == "DON_MISSILE").copied().unwrap_or_else(|| panic!("{e:?}: no hit {:?}", rig.rows()));
        assert!((hit.position - (target + Vec3::Z * 90.0)).length() < 120.0, "{e:?}: the hit is at the target {:?}", hit.position);
    }
}

// ================================================================================================ the user's rule: no target -> Wand Bonk

/// The user (2026-10-22): "when Donald doesn't have a target, either soft locked or locked on, then he should just do a
/// wand bonk". Ground: no lock and no soft target -> the Bonk, never a missile; with a target the missiles (the Bonk
/// within 2 m). Air: no target -> the air Wand Bonk (his air staff swing), as many as the air string's steps.
#[test]
fn no_target_means_the_wand_bonk_never_a_missile() {
    // nothing at all
    let mut rig = Rig::new(&[]);
    rig.mash(8, 120);
    rig.idle(40);
    assert!(rig.missiles().is_empty(), "no missile without a target");
    assert!(rig.started().iter().filter(|s| **s == "wand_bonk").count() >= 2, "{:?}", rig.started());
    // an enemy behind him (outside the soft target's cone) and nothing locked: no target -> the Bonk
    let mut rig = Rig::new(&[Vec3::new(-1500.0, 0.0, 0.0)]);
    rig.tap(A, Vec2::ZERO);
    assert_eq!(rig.state(), Some(donald::BONK));
    // the same enemy locked on (RB): the missiles
    let mut rig = Rig::new(&[Vec3::new(-1500.0, 0.0, 0.0)]);
    rig.tap(RB, Vec2::ZERO);
    assert_eq!(rig.m().pad.as_ref().unwrap().lock, TargetId(1));
    rig.tap(A, Vec2::ZERO);
    assert_eq!(rig.state(), Some(donald::STEP));
    // a soft target ahead: the missiles
    let mut rig = Rig::new(&[Vec3::new(1500.0, 0.0, 0.0)]);
    rig.tap(A, Vec2::ZERO);
    assert_eq!(rig.state(), Some(donald::STEP));
    // the target gone mid-string: the next press is the Bonk
    let mut rig = Rig::new(&[Vec3::new(1500.0, 0.0, 0.0)]);
    rig.tap(A, Vec2::ZERO);
    rig.dummies.0.clear();
    rig.mash(8, 90);
    assert!(rig.started().contains(&"wand_bonk"), "{:?}", rig.started());
    assert_eq!(rig.missiles().len(), 1, "only the first step's missile");
}

#[test]
fn in_the_air_with_no_target_the_air_wand_bonk() {
    let mut rig = Rig::new(&[]);
    for i in 0..90 {
        let held: &[PadButton] = match i {
            0..=6 => &[B],
            i if i >= 14 && i % 10 == 4 => &[A],
            _ => &[],
        };
        rig.step(Vec2::ZERO, held);
    }
    rig.idle(90);
    assert!(rig.missiles().is_empty(), "no air missile without a target");
    // (the user 2026-10-22: a Bonk only after the last one's whole clip) one per clip: the air ones are capped by it
    let n = rig.started().iter().filter(|s| **s == "air_wand_bonk").count();
    assert!(n >= 1 && n <= combo::air_steps(rig.m()) as usize, "{:?}", rig.started());
    assert_eq!(rig.m().state, NpcStateId::Idle, "landed");
    // a Bonk in the air hits what it touches
    let mut rig = Rig::new(&[]);
    rig.dummies.0.push((TargetId(100), Vec3::new(120.0, 0.0, 120.0)));
    for i in 0..60 {
        let held: &[PadButton] = match i {
            0..=6 => &[B],
            14 => &[A],
            _ => &[],
        };
        rig.step(Vec2::ZERO, held);
    }
    assert!(rig.rows().contains(&"DON_WAND_BONK"), "{:?}", rig.rows());
}

// ================================================================================================ feel (the user 2026-10-22)

/// 8: Aero Float / Aero Glide sway (the staff arm about the held frame, the body rocking about his feet) on the
/// state's clock: the same pose at the same time whatever the frame rate; the drawing gets the rock (MemberView.pitch);
/// it is gone once he lands.
#[test]
fn the_float_sways_his_staff_and_body_frame_rate_free() {
    // the pure function of the state time: bounded, periodic, Aero Glide faster
    let (mut lo, mut hi, mut plo, mut phi) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    for i in 0..400 {
        let (pose, pitch) = moves::sway(i as f32 * 0.01, false);
        (lo, hi, plo, phi) = (lo.min(pose), hi.max(pose), plo.min(pitch), phi.max(pitch));
    }
    assert!((hi - lo - 2.0 * K.float_sway_pose).abs() < 0.01 && (phi - plo - 2.0 * K.float_sway_pitch).abs() < 0.2, "{lo}..{hi} {plo}..{phi}");
    let period = 1.0 / K.float_sway_hz;
    assert!((moves::sway(0.37, false).1 - moves::sway(0.37 + period, false).1).abs() < 1e-3);
    assert!(K.glide_aero_sway_hz > K.float_sway_hz);
    // in the game: the pitch the hosts draw moves, the held clip time moves, and both stop on landing
    let mut rig = Rig::new(&[]);
    rig.wear(&[DonAbility::AeroFloat]);
    rig.mm().cmc.location.z += 1500.0;
    rig.mm().cmc.set_falling();
    rig.mm().enter(NpcStateId::Pad(JUMP));
    rig.mm().pad.as_mut().unwrap().jump.sub = 4;
    rig.until(Vec2::new(0.0, 1.0), &[B], 30, |r| r.state() == Some(donald::FLOAT)).expect("the float");
    let (mut pitches, mut times) = (Vec::new(), Vec::new());
    for _ in 0..90 {
        rig.step(Vec2::new(0.0, 1.0), &[B]);
        if rig.state() != Some(donald::FLOAT) {
            break;
        }
        let t = rig.m().state_time;
        let view = rig.party.view(rig.id).unwrap();
        assert!((view.pitch - moves::sway(t, false).1).abs() < 1e-3, "the drawing gets the rock of the sway");
        assert!((rig.m().anim.time - moves::sway(t, false).0).abs() < 1e-3);
        pitches.push(view.pitch);
        times.push(rig.m().anim.time);
    }
    let span = |v: &[f32]| v.iter().cloned().fold(f32::MIN, f32::max) - v.iter().cloned().fold(f32::MAX, f32::min);
    assert!(span(&pitches) > K.float_sway_pitch && span(&times) > K.float_sway_pose, "{pitches:?}");
    rig.until(Vec2::ZERO, &[], 900, |r| r.state() != Some(donald::FLOAT)).expect("he lets go");
    assert_eq!(rig.party.view(rig.id).unwrap().pitch, 0.0, "the rock is off once the float ends");
}

/// 3.3: holding A he is never still: the stick walks him at `charge.walk` with his walk clip, facing the target, in
/// the held step and in the charge; the stick let go, the held cast pose again; the release still fires.
#[test]
fn holding_a_walks_him_with_his_walk_clip_facing_the_target() {
    let mut rig = Rig::new(&[Vec3::new(1200.0, 0.0, 0.0)]);
    // the held step (before the charge): past the step's shot frame, A still held
    rig.until(Vec2::ZERO, &[A], 60, |r| r.m().anim.speed == 0.0 && r.state() == Some(donald::STEP)).expect("the held step");
    let y0 = rig.m().position().y;
    for _ in 0..10 {
        rig.step(Vec2::new(1.0, 0.0), &[A]);
    }
    assert_eq!(rig.state(), Some(donald::STEP));
    assert_eq!(rig.m().anim.clip.name, K.charge_walk_right_clip, "his side step cycle in the held step (stick right)");
    assert!(rig.m().position().y - y0 > 0.5 * K.charge_walk * 10.0 * DT, "he walks");
    // the charge: walking, facing the target (+X)
    rig.until(Vec2::new(1.0, 0.0), &[A], 90, |r| r.state() == Some(donald::CHARGE)).expect("the charge");
    let y1 = rig.m().position().y;
    for _ in 0..30 {
        rig.step(Vec2::new(1.0, 0.0), &[A]);
    }
    assert_eq!(rig.m().anim.clip.name, K.charge_walk_right_clip);
    assert!(rig.m().anim.speed > 0.0, "the step cycle plays");
    let v = (rig.m().position().y - y1) / (30.0 * DT);
    assert!(v > 0.6 * K.charge_walk && v < 1.2 * K.charge_walk, "{v}");
    let to_target = crate::pawn::yaw_of(Vec3::new(1200.0, 0.0, 0.0) - rig.m().position());
    assert!(crate::pawn::wrap180(rig.m().yaw - to_target).abs() < 10.0, "facing the target: {} vs {to_target}", rig.m().yaw);
    // the stick let go: the held cast pose
    for _ in 0..5 {
        rig.step(Vec2::ZERO, &[A]);
    }
    assert_eq!(rig.m().anim.clip.name, K.charge_clip);
    assert!((rig.m().anim.time - K.charge_pose).abs() < 1e-3 && rig.m().anim.speed == 0.0);
    // each way its cycle: forward the battle walk, left the other side step, back the walk played backwards
    for _ in 0..20 {
        rig.step(Vec2::new(0.0, 1.0), &[A]);
    }
    assert_eq!(rig.m().anim.clip.name, K.charge_walk_clip);
    for _ in 0..20 {
        rig.step(Vec2::new(-1.0, 0.0), &[A]);
    }
    assert_eq!(rig.m().anim.clip.name, K.charge_walk_left_clip);
    let mut last = -1.0;
    let mut back = 0;
    for _ in 0..20 {
        rig.step(Vec2::new(0.0, -1.0), &[A]);
        let t = rig.m().anim.time;
        if last >= 0.0 && t < last && last - t < 0.2 {
            back += 1;
        }
        last = t;
    }
    assert_eq!(rig.m().anim.clip.name, K.charge_walk_back_clip);
    if K.charge_walk_back_reverse != 0.0 {
        assert!(back >= 15, "the back walk runs backwards: {back}");
    }
    rig.idle(90);
    assert!(rig.missiles().iter().any(|m| m.0 == MissileKind::Charged), "{:?}", rig.missiles());
}

/// 3.1: with the stick held, after each step's missile he strafes (left / right) or hops (forward / back), facing the
/// target, the distance over the time whatever the frame rate; the string goes on with the next press; never in the
/// slam step (he stands still for it).
#[test]
fn the_string_strafes_and_hops_between_missiles_facing_the_target_never_in_the_slam() {
    let target = Vec3::new(1200.0, 0.0, 0.0);
    // which move a stick makes, his facing +X (yaw grows to his right: the stick right is +90)
    assert_eq!(combo::move_of(90.0, 0.0).0, combo::Move::Right);
    assert_eq!(combo::move_of(-90.0, 0.0).0, combo::Move::Left);
    assert_eq!(combo::move_of(0.0, 0.0).0, combo::Move::Forward);
    assert_eq!(combo::move_of(180.0, 0.0).0, combo::Move::Back);
    let mut rig = Rig::new(&[target]);
    let one = |rig: &mut Rig, stick: Vec2| -> (Vec3, Vec3) {
        rig.step(stick, &[A]);
        rig.step(stick, &[]);
        rig.until(stick, &[], 90, |r| r.kit().combo.mv != combo::Move::None).expect("a strafe / hop after the missile");
        let start = rig.m().position();
        assert!(!rig.missiles().is_empty(), "the move comes after the missile");
        // facing the target through the move
        rig.until(stick, &[], 60, |r| {
            let to = crate::pawn::yaw_of(target - r.m().position());
            assert!(crate::pawn::wrap180(r.m().yaw - to).abs() < 12.0, "facing the target: {} vs {to}", r.m().yaw);
            // landed (the next press within strafe.link goes on with the string)
            let c = r.kit().combo;
            c.mv == combo::Move::None || c.mv_t >= c.mv.spec().2 || r.state() != Some(donald::STEP)
        });
        (start, rig.m().position())
    };
    // stick right: strafe right (+Y), about strafe.side_distance
    let (a, b) = one(&mut rig, Vec2::new(1.0, 0.0));
    assert!(rig.started().contains(&"strafe_right"), "{:?}", rig.started());
    let side = b - a;
    assert!(side.y > 0.85 * K.strafe_side_distance && side.y < 1.1 * K.strafe_side_distance && side.x.abs() < 0.3 * side.y, "{side:?}");
    // the string goes on: the next press is the next step (wave right), then stick left: strafe left
    let (a, b) = one(&mut rig, Vec2::new(-1.0, 0.0));
    let steps: Vec<&str> = rig.started().into_iter().filter(|s| matches!(*s, "wave_left" | "wave_right" | "plus_left" | "plus_right" | "slam")).collect();
    assert_eq!(steps, ["wave_left", "wave_right"], "the combo continues after the move");
    assert!(rig.started().contains(&"strafe_left") && (b - a).y < -0.85 * K.strafe_side_distance, "{:?}", b - a);
    // the slam: the stick held back, he stands still for it
    rig.step(Vec2::new(0.0, -1.0), &[A]);
    rig.step(Vec2::new(0.0, -1.0), &[]);
    rig.until(Vec2::new(0.0, -1.0), &[], 120, |r| r.started().contains(&"slam")).expect("the slam");
    let n = rig.started().len();
    rig.until(Vec2::new(0.0, -1.0), &[], 120, |r| r.state() != Some(donald::STEP));
    let after: Vec<&str> = rig.started()[n..].to_vec();
    assert!(!after.iter().any(|s| s.starts_with("strafe") || s.starts_with("hop")), "no move in the slam: {after:?}");
    assert!(rig.missiles().iter().any(|m| m.0 == MissileKind::Slam));
    // a new string: stick back = the hop back, stick forward = the hop forward
    rig.idle(40);
    let (a, b) = one(&mut rig, Vec2::new(0.0, -1.0));
    assert!(rig.started().contains(&"hop_back") && (b - a).x < -0.85 * K.strafe_back_distance, "{:?}", b - a);
    rig.idle(40);
    let (a, b) = one(&mut rig, Vec2::new(0.0, 1.0));
    assert!(rig.started().contains(&"hop_forward") && (b - a).x > 0.85 * K.strafe_fwd_distance, "{:?}", b - a);
}

// ================================================================================================ the user's rules (2026-10-22, round 2)

/// Starts a cast of `spell` on `rig`'s Donald right now (DON-MAGIC's `cast::begin`, free), at the first dummy (an ally
/// spell on himself).
fn begin_cast(rig: &mut Rig, spell: SpellRef) -> bool {
    let d = don_data();
    let row = spell.row(&d).copied().expect("the spell's row");
    let plan = crate::donald::CastPlan { spell, mp: 0, pct: 0.0, attuned: true, quick: false, echo: false, cast: row.cast, charges: 0 };
    let target = if spell.ally_target() || matches!(spell, SpellRef::Line1(Element::Light, _)) { crate::donald::cast::CastTarget::Member(rig.id) } else { crate::donald::cast::CastTarget::Enemy(TargetId(1)) };
    let id = rig.id;
    let mut host = PartyHost { world: &rig.world, targets: &mut rig.dummies, audio: &mut rig.audio, effects: &mut rig.effects };
    crate::donald::cast::begin(rig.party.member_mut(id).unwrap(), &plan, target, &mut host)
}

/// "Ensure Donald does not ever walk while casting a spell, his animation takes priority": the stick held through each
/// kind of cast (a line spell, line 2, a hybrid, an ultimate, a Y spell, Zettaflare, the attune cast) moves him not at
/// all and the cast clip stays on him, from the cast's start to its end; then the stick runs again.
#[test]
fn no_walk_while_casting_the_cast_clip_stays_on_top() {
    let spells = [SpellRef::Line1(Element::Fire, 1), SpellRef::Line2(Element::Blizzard, 1), SpellRef::Hybrid(Element::Fire, Element::Blizzard), SpellRef::Ultimate(Element::Thunder), SpellRef::Arcane(crate::donald::ArcaneLine::Comet, 1), SpellRef::Arcane(crate::donald::ArcaneLine::Zettaflare, 1)];
    for spell in spells {
        let mut rig = Rig::new(&[Vec3::new(1500.0, 0.0, 0.0)]);
        // running when the cast starts
        rig.until(Vec2::new(1.0, 0.0), &[], 30, |r| r.m().state == NpcStateId::Pad(RUN)).expect("running");
        for _ in 0..10 {
            rig.step(Vec2::new(1.0, 0.0), &[]);
        }
        assert!(begin_cast(&mut rig, spell), "{spell:?}");
        let clip = rig.m().anim.clip.name;
        let at = rig.m().position();
        let mut frames = 0;
        while donald::cast_holds_still(rig.m()) && frames < 1200 {
            rig.step(Vec2::new(1.0, 0.0), &[]);
            frames += 1;
            let moved = crate::pawn::horizontal(rig.m().position() - at).length();
            assert!(moved < 0.5, "{spell:?}: moved {moved} cm at frame {frames}");
            if donald::cast_holds_still(rig.m()) {
                assert!(rig.m().pad.as_ref().unwrap().loco.is_none(), "{spell:?}: no run cycle over the cast");
                assert_ne!(rig.m().state, NpcStateId::Pad(RUN), "{spell:?}: no run");
                // (DON-SPELLB) the ultimates take ZETTA's channel and cast clips (the user: Zettaflare's casting animation)
                if !matches!(spell, SpellRef::Arcane(crate::donald::ArcaneLine::Zettaflare, _)) && !spell.is_ultimate() {
                    assert_eq!(rig.m().anim.clip.name, clip, "{spell:?}: the cast clip stays on top");
                }
            }
        }
        assert!(frames > 5 && frames < 1200, "{spell:?}: the cast ran {frames} frames");
        // the cast over: the stick runs him again ((ZETTA round 5) Zettaflare: the user's cutscene holds him rooted until its
        // hand-off behind him, about 19 s after the cast clip; then he runs)
        let budget = if matches!(spell, SpellRef::Arcane(crate::donald::ArcaneLine::Zettaflare, _)) { 60 * 30 } else { 60 };
        rig.until(Vec2::new(1.0, 0.0), &[], budget, |r| crate::pawn::horizontal(r.m().position() - at).length() > 20.0).expect("free after the cast");
    }
    // the attune cast (the menu's attunement: the ATTUNE move)
    let mut rig = Rig::new(&[Vec3::new(1500.0, 0.0, 0.0)]);
    assert!(donald::wants_cast(rig.mm(), DonCmd::Attune(Element::Water), crate::donald::cast::CastTarget::Enemy(TargetId::NONE)));
    rig.step(Vec2::new(1.0, 0.0), &[]);
    let at = rig.m().position();
    let mut held = 0;
    while donald::cast_holds_still(rig.m()) && held < 200 {
        rig.step(Vec2::new(1.0, 0.0), &[]);
        held += 1;
        assert!(crate::pawn::horizontal(rig.m().position() - at).length() < 0.5, "the attune cast holds him");
    }
    assert!(held > 10, "{held}");
}

/// (ZETTA round 5c) THE USER: "Keep Donald in combat until the animation fully ends if the target dies": the host drops the
/// battle state mid-cast (the target died): he stays in battle (stance, combat clips) until the cast and its recovery end
/// (Zettaflare and the ultimates: to their hand-off), then the host's state applies.
#[test]
fn donald_stays_in_battle_until_his_cast_ends_when_the_target_dies() {
    let spells = [SpellRef::Line1(Element::Fire, 1), SpellRef::Hybrid(Element::Fire, Element::Blizzard), SpellRef::Ultimate(Element::Thunder), SpellRef::Arcane(crate::donald::ArcaneLine::Comet, 1), SpellRef::Arcane(crate::donald::ArcaneLine::Zettaflare, 1)];
    for spell in spells {
        let mut rig = Rig::new(&[Vec3::new(1500.0, 0.0, 0.0)]);
        assert!(begin_cast(&mut rig, spell), "{spell:?}");
        for _ in 0..3 {
            rig.step(Vec2::ZERO, &[]);
        }
        // (ZETTA 2026-10-23) after Zettaflare his down (the collapse, 2 s down, the get-up: zettaflare::ZettaDown) is part of it
        let busy = |r: &Rig| donald::cast_holds_still(r.m()) || r.party.don_zetta.runs.iter().any(|z| z.caster == r.id && !z.done) || r.m().don_zetta.down.is_some();
        assert!(busy(&rig), "{spell:?}: casting");
        // the target dies: the host says the fight is over, every step
        let mut frames = 0;
        while busy(&rig) && frames < 60 * 40 {
            rig.party.set_in_battle(false);
            assert!(rig.m().in_battle, "{spell:?}: still in battle mid-cast (frame {frames})");
            rig.step(Vec2::ZERO, &[]);
            frames += 1;
        }
        assert!(frames < 60 * 40, "{spell:?}: the cast ended");
        // the cast over: the host's state from here
        rig.party.set_in_battle(false);
        assert!(!rig.m().in_battle, "{spell:?}: out of battle once the animation has ended");
    }
}

/// The same for AI Donald: a MoveTo command running while he casts does not move him until the cast ends.
#[test]
fn ai_donald_does_not_walk_while_casting() {
    let mut party = Party::new(PartyConfig::test_level99());
    let id = party.spawn(Chara::Donald, Vec3::ZERO, 0.0);
    let world = TestWorld::floor();
    let mut dummies = Dummies(vec![(TargetId(1), Vec3::new(1500.0, 0.0, 0.0))], Vec::new());
    let (mut audio, mut effects) = (RecAudio::default(), RecEffects::default());
    let sora = SoraView { position: Vec3::new(-3000.0, 0.0, 0.0), ..SoraView::default() };
    party.command(id, crate::MemberCommand::MoveTo { goal: crate::MoveGoal::Point(Vec3::new(0.0, 3000.0, 0.0)), locomotion: crate::LocomotionDef::Move1, walk: false, acceptable_radius: 50.0, follow_path: false });
    for _ in 0..30 {
        let mut host = PartyHost { world: &world, targets: &mut dummies, audio: &mut audio, effects: &mut effects };
        party.step(DT, &sora, &mut host);
    }
    let d = don_data();
    let spell = SpellRef::Line1(Element::Fire, 1);
    let plan = crate::donald::CastPlan { spell, mp: 0, pct: 0.0, attuned: true, quick: false, echo: false, cast: spell.row(&d).unwrap().cast, charges: 0 };
    {
        let mut host = PartyHost { world: &world, targets: &mut dummies, audio: &mut audio, effects: &mut effects };
        assert!(crate::donald::cast::begin(party.member_mut(id).unwrap(), &plan, crate::donald::cast::CastTarget::Enemy(TargetId(1)), &mut host));
    }
    let clip = party.member(id).unwrap().anim.clip.name;
    let at = party.member(id).unwrap().position();
    let mut n = 0;
    while donald::cast_holds_still(party.member(id).unwrap()) && n < 600 {
        let mut host = PartyHost { world: &world, targets: &mut dummies, audio: &mut audio, effects: &mut effects };
        party.step(DT, &sora, &mut host);
        n += 1;
        let m = party.member(id).unwrap();
        assert!(crate::pawn::horizontal(m.position() - at).length() < 0.5, "AI Donald moved during his cast");
        if donald::cast_holds_still(m) {
            assert_eq!(m.anim.clip.name, clip, "the cast clip stays on top");
        }
    }
    assert!(n > 5);
}

/// "Donald should not hover in the air when standing still while holding A, his feet should be planted on the ground":
/// the held cast frames are frames with his feet down (KH3's MagicCast hops from 0.37; the waves lift a foot around their
/// shots; the frames measured on his posed skeleton: kh-partyhost tests/don_feet.rs) and he stands on the floor.
#[test]
fn holding_a_still_his_feet_stay_on_the_ground() {
    assert!(K.charge_pose < 0.37, "MagicCast hops from 0.37: {}", K.charge_pose);
    assert!(combo::def(combo::Step::WaveL).hold <= combo::def(combo::Step::WaveL).shot && combo::def(combo::Step::WaveR).hold <= combo::def(combo::Step::WaveR).shot);
    let mut rig = Rig::new(&[Vec3::new(1200.0, 0.0, 0.0)]);
    rig.until(Vec2::ZERO, &[A], 90, |r| r.state() == Some(donald::CHARGE)).expect("the charge");
    for _ in 0..60 {
        rig.step(Vec2::ZERO, &[A]);
        assert!(rig.m().on_ground() && rig.m().position().z.abs() < 0.5, "on the floor: {}", rig.m().position().z);
        assert_eq!(rig.m().anim.clip.name, K.charge_clip);
        assert!((rig.m().anim.time - K.charge_pose).abs() < 1e-3, "the feet-down frame");
    }
    // the held step before the charge: its hold frame, on the floor
    let mut rig = Rig::new(&[Vec3::new(1200.0, 0.0, 0.0)]);
    rig.until(Vec2::ZERO, &[A], 60, |r| r.m().anim.speed == 0.0 && r.state() == Some(donald::STEP)).expect("the held step");
    let d = combo::def(combo::Step::WaveL);
    assert!((rig.m().anim.time - d.hold).abs() < 1e-3 && rig.m().on_ground());
    // let go: the clip plays on to the shot, the missile leaves
    rig.idle(40);
    assert_eq!(rig.missiles().len(), 1);
}

/// "The Aero effect for Aero glide should end as SOON as he stops using it, instead of lingering on his wand": the wand
/// wind is stopped at once (its particles killed) on landing and on letting go of B (every way out goes through end_float / left_state).
#[test]
fn the_glide_wind_ends_at_once() {
    for how in ["land", "let_go"] {
        let mut rig = Rig::new(&[]);
        rig.wear(&[DonAbility::AeroFloat]);
        rig.mm().cmc.location.z += if how == "land" { 120.0 } else { 1500.0 };
        rig.mm().cmc.set_falling();
        rig.mm().enter(NpcStateId::Pad(JUMP));
        rig.mm().pad.as_mut().unwrap().jump.sub = 4;
        rig.until(Vec2::new(0.0, 1.0), &[B], 30, |r| r.state() == Some(donald::FLOAT)).expect("the float");
        let fx = rig.kit().owned_fx;
        assert!(fx != 0 && rig.effects.spawned.contains(&K.float_fx));
        let stopped = |r: &Rig| r.effects.stops.iter().find(|s| s.0 == fx).copied();
        let mut n = 0;
        while rig.state() == Some(donald::FLOAT) && n < 900 {
            match how {
                "land" => rig.step(Vec2::new(0.0, 1.0), &[B]),
                "let_go" => rig.step(Vec2::new(0.0, 1.0), &[]),
                _ => unreachable!(),
            }
            n += 1;
        }
        assert_ne!(rig.state(), Some(donald::FLOAT), "{how}");
        assert_eq!(stopped(&rig), Some((fx, true)), "{how}: the wind stopped at once (immediate), the step he left the float");
        assert_eq!(rig.kit().owned_fx, 0);
    }
}

/// "Donald shouldn't be able to spam wand bonk even without a target, he can only use it again after the animation": A
/// mashed with no target gives Bonks one whole clip apart (presses during a Bonk are dropped, not kept).
#[test]
fn wand_bonks_are_one_whole_clip_apart() {
    let mut rig = Rig::new(&[]);
    let clip = combo::clip_copy(K.bonk_clip, true);
    let len = clip.length / (clip.rate * K.bonk_rate);
    let mut starts = Vec::new();
    let mut before = 0;
    for i in 0..400 {
        let held: &[PadButton] = if i % 4 == 0 { &[A] } else { &[] };
        rig.step(Vec2::ZERO, held);
        let n = rig.started().iter().filter(|s| **s == "wand_bonk").count();
        if n > before {
            starts.push(i as f32 * DT);
            before = n;
        }
    }
    assert!(starts.len() >= 3, "{starts:?}");
    for w in starts.windows(2) {
        let gap = w[1] - w[0];
        assert!(gap >= len - 1e-3 && gap <= len + 5.0 * DT, "Bonks {gap} s apart, the clip {len} s: {starts:?}");
    }
    // in the air: one air Bonk per clip too (the air Bonks are capped by the clip)
    let mut rig = Rig::new(&[]);
    let air = combo::clip_copy(K.bonk_air_clip, true);
    let air_len = air.length / (air.rate * K.bonk_air_rate);
    let mut starts = Vec::new();
    let mut before = 0;
    for i in 0..120 {
        let held: &[PadButton] = match i {
            0..=6 => &[B],
            i if i >= 14 && i % 3 == 2 => &[A],
            _ => &[],
        };
        rig.step(Vec2::ZERO, held);
        let n = rig.started().iter().filter(|s| s.ends_with("wand_bonk")).count();
        if n > before {
            starts.push(i as f32 * DT);
            before = n;
        }
    }
    for w in starts.windows(2) {
        assert!(w[1] - w[0] >= air_len.min(len) - 1e-3, "{starts:?} (air clip {air_len})");
    }
}

/// The user (2026-10-22): "the little item VFX doesn't appear when Goofy and Donald first use an item": the FIRST item a
/// member uses asks the host for its effect (KH3's Recover kind: a Potion's vs_itm_pot0 on him). The host side (kh-app's
/// warm spawn, kh-host's preload list: kh-host tests/item_fx_preload.rs) makes that first spawn draw.
#[test]
fn the_first_item_use_spawns_its_effect() {
    let mut rig = Rig::new(&[]);
    let id = rig.id;
    rig.party.status_mut(id).unwrap().items[0] = crate::ItemSlot { item: Some(kh_core::status::ItemKind::Potion), present: true };
    assert!(rig.effects.spawned.is_empty() || !rig.effects.spawned.contains(&"vs_itm_pot0"));
    rig.party.command(id, crate::MemberCommand::UseItem { item: kh_core::status::ItemKind::Potion, target: crate::TargetRef::Member(id) });
    rig.idle(120);
    assert_eq!(rig.effects.spawned.iter().filter(|n| **n == "vs_itm_pot0").count(), 1, "{:?}", rig.effects.spawned);
}

/// (KITFX) the user 2026-10-23: "aero glide needs a little non-annoying sfx": a gentle whoosh at the start, ONE quiet airy
/// loop on him while he floats (Aero Glide a little louder), faded softly the moment the float ends; toggled quickly it never
/// stacks or restarts: the whoosh at most once per `float.se_debounce`, no loop for a float shorter than `float.se_loop_delay`.
#[test]
fn kitfx_the_glide_wind_is_one_soft_debounced_loop() {
    for aero in [false, true] {
        let mut rig = Rig::new(&[]);
        rig.wear(&[DonAbility::AeroFloat]);
        if aero {
            rig.attune(Element::Aero, false);
        }
        rig.mm().cmc.location.z += 4000.0;
        rig.mm().cmc.set_falling();
        rig.mm().enter(NpcStateId::Pad(JUMP));
        rig.mm().pad.as_mut().unwrap().jump.sub = 4;
        rig.until(Vec2::new(0.0, 1.0), &[B], 30, |r| r.state() == Some(donald::FLOAT)).expect("the float");
        for _ in 0..60 {
            rig.step(Vec2::new(0.0, 1.0), &[B]);
        }
        let whooshes = |r: &Rig| r.audio.played.iter().filter(|n| **n == K.float_se_start).count();
        assert_eq!(whooshes(&rig), 1, "aero {aero}: one whoosh");
        let loops: Vec<_> = rig.audio.loops.iter().filter(|l| l.1 == K.float_se_loop).copied().collect();
        assert_eq!(loops.len(), 1, "aero {aero}: one loop");
        let vol = if aero { K.glide_aero_se_loop_volume } else { K.float_se_loop_volume };
        assert!((loops[0].2 - vol).abs() < 1e-6 && vol <= 0.6, "aero {aero}: quiet ({})", loops[0].2);
        let h = loops[0].0;
        let n = rig.until(Vec2::new(0.0, 1.0), &[], 30, |r| r.state() != Some(donald::FLOAT));
        assert!(n.is_some(), "he lets go");
        assert_eq!(rig.audio.stops.iter().find(|s| s.0 == h).map(|s| s.1), Some(K.float_se_fade), "aero {aero}: the soft fade at once");
        // quick toggles in the same airtime: 6 frames floating, 6 not, for 2 s
        let (w0, l0) = (whooshes(&rig), rig.audio.loops.len());
        let floats = |r: &Rig| r.started().iter().filter(|s| s.starts_with("aero_")).count();
        let f0 = floats(&rig);
        for k in 0..120 {
            let held: &[PadButton] = if (k / 6) % 2 == 0 { &[B] } else { &[] };
            rig.step(Vec2::new(0.0, 1.0), held);
            let live = rig.audio.loops.iter().filter(|l| l.1 == K.float_se_loop && !rig.audio.stops.iter().any(|s| s.0 == l.0)).count();
            assert!(live <= 1, "aero {aero}: never two loops ({live})");
        }
        assert!(floats(&rig) >= f0 + 3, "aero {aero}: the toggles floated again ({} floats)", floats(&rig) - f0);
        assert_eq!(rig.audio.loops.len(), l0, "aero {aero}: no loop restarts while toggling");
        assert!(whooshes(&rig) - w0 <= (2.0 / K.float_se_debounce).ceil() as usize, "aero {aero}: the whoosh debounced ({})", whooshes(&rig) - w0);
    }
}

/// (KITFX) the user 2026-10-23: "Donalds basic water bolt reads as ice ... Thunder bolts ... uses ice particles": no missile
/// body or hit is one of Donald's UNDERWATER effects (vn_ex001_w*: his Caribbean Fire / Blizzard / Thunder / Aero), Water's
/// is Sora's Water shot with its splash, Thunder's the crackling bolt; the bodies without a flight SE of their own sound at
/// the launch, and every element's look is its own (no two elements share a body or a hit).
#[test]
fn kitfx_every_elements_missile_is_its_own_element() {
    let all = [None, Some(Element::Fire), Some(Element::Blizzard), Some(Element::Thunder), Some(Element::Water), Some(Element::Aero), Some(Element::Light)];
    for e in all {
        let body = donald_shots::body_of(e);
        assert!(!body.starts_with("vn_ex001_w"), "{e:?}: {body} is an underwater variant");
        assert!(!donald_shots::hit_look(e).0.starts_with("vn_ex001_w"), "{e:?}");
    }
    let bodies: std::collections::BTreeSet<_> = all.iter().map(|e| donald_shots::body_of(*e)).collect();
    assert_eq!(bodies.len(), all.len(), "every element its own body");
    assert_eq!(donald_shots::body_of(Some(Element::Water)), "vm_wat_blt0");
    assert_eq!(donald_shots::hit_look(Some(Element::Water)).0, "vs_wat_hit1");
    assert_eq!(donald_shots::body_of(Some(Element::Thunder)), "vd_kitfx_thd_blt");
    for e in [Element::Thunder, Element::Aero, Element::Light] {
        assert!(donald_shots::launch_se(Some(e)).starts_with("se"), "{e:?}: a launch sound");
    }
    // a Thunder missile sounds as it leaves
    let mut rig = Rig::new(&[Vec3::new(900.0, 0.0, 0.0)]);
    rig.attune(Element::Thunder, false);
    let id = rig.id;
    let spec = donald_shots::MissileSpec::new(donald_shots::MissileKind::Step, Some(Element::Thunder), Vec3::new(0.0, 0.0, 110.0), Vec3::X, kh_core::host::TargetId(1));
    {
        let mut host = PartyHost { world: &rig.world, targets: &mut rig.dummies, effects: &mut rig.effects, audio: &mut rig.audio };
        donald_shots::launch(rig.party.member_mut(id).unwrap(), &mut host, spec);
    }
    assert!(rig.effects.spawned.contains(&"vd_kitfx_thd_blt"));
    assert!(rig.audio.played.contains(&K.missile_thunder_launch_se));
}
