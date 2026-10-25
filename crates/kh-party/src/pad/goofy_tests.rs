//! (party X3G) Goofy's playable kit, one test per move (and the feel numbers against Sora's).

use super::super::{PadFrame, AIR_ATTACK, JUMP, RUN};
use super::*;
use crate::{Party, PartyConfig, SoraView};
use kh_core::host::{HitAttacker, HitRecord, HitResult, HurtShape, TargetDefense, TargetFlags, TargetInfo, Targets};
use kh_core::input::{PadButtons, PadState as Buttons};
use kh_core::test_world::TestWorld;
use kh_core::Tuning;

const DT: f32 = 1.0 / 60.0;

/// (GOOFY-GK) the third list: ids that are a breakable pot / friendly NPC (HITTABLE, never LOCKABLE).
struct Dummies(Vec<(TargetId, Vec3)>, Vec<HitRecord>, Vec<TargetId>);

impl Dummies {
    fn info_of(&self, (id, p): (TargetId, Vec3)) -> TargetInfo {
        TargetInfo { id, position: p, center: p + Vec3::Z * 90.0, velocity: Vec3::ZERO, yaw: 180.0, radius: 50.0, half_height: 90.0, flags: if self.2.contains(&id) { TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE) } else { TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY) }, hp: 1.0e6, hp_max: 1.0e6 }
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
    fn defense(&self, _id: TargetId) -> Option<TargetDefense> {
        Some(TargetDefense { defense: 10, max_hp: 1_000_000, hp_percent: 100.0, enemy_kind: true, damage_min: 0, damage_max: 0, resist: [0; 8], bio_type: 0 })
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
    camera: f32,
}

const X: PadButton = PadButton::FaceLeft;
const A: PadButton = PadButton::FaceBottom;
const B: PadButton = PadButton::FaceRight;
const Y: PadButton = PadButton::FaceTop;
const RB: PadButton = PadButton::RightShoulder;
const FWD: Vec2 = Vec2::new(0.0, 1.0);
const RIGHT: Vec2 = Vec2::new(1.0, 0.0);
const NONE: Vec2 = Vec2::ZERO;

impl Rig {
    fn new(dummies: &[Vec3]) -> Rig {
        let mut party = Party::new(PartyConfig::test_level99());
        let id = party.spawn(Chara::Goofy, Vec3::ZERO, 0.0);
        party.set_pad(id, Some(Tuning::USER));
        let d = Dummies(dummies.iter().enumerate().map(|(i, p)| (TargetId(i as u32 + 1), *p)).collect(), Vec::new(), Vec::new());
        let mut rig = Rig { world: TestWorld::floor(), dummies: d, audio: Default::default(), effects: Default::default(), party, sora: SoraView::default(), id, held: PadButtons::NONE, camera: 0.0 };
        rig.sora.position = Vec3::new(-3000.0, 0.0, 0.0);
        rig.idle(10);
        rig
    }

    fn m(&self) -> &NpcPawn {
        self.party.member(self.id).unwrap()
    }

    fn mm(&mut self) -> &mut NpcPawn {
        self.party.member_mut(self.id).unwrap()
    }

    fn k(&self) -> &GoofyPad {
        &self.m().pad.as_ref().unwrap().goofy
    }

    /// (GOOFY-GK) GG's GUARD meter on his status.
    fn guard(&self) -> crate::guard::Guard {
        self.m().status.guard.expect("Goofy has GUARD")
    }

    /// (GOOFY-GK) The kit's log entries that are combo steps, in order.
    fn steps(&self) -> Vec<&'static str> {
        self.k().log.iter().copied().filter(|w| w.starts_with("combo_") || w.starts_with("fin_") || w.starts_with("air_") || *w == "fusion_spin" || *w == "fusion_rocket").collect()
    }

    /// (GOOFY-GK) Wears only these of his combo / finisher abilities.
    fn wear_only(&mut self, on: &[GA]) {
        for k in [GA::ComboPlus, GA::ComboPlus2, GA::AirComboPlus, GA::ShieldQuake, GA::ShieldUpper, GA::FusionSpin, GA::FusionRocket] {
            self.mm().status.goofy_kit.set_equipped(k, on.contains(&k));
        }
    }

    /// (GOOFY-GK) Mashes A (a press every other step) until the predicate holds.
    fn mash(&mut self, n: usize, stick: Vec2, f: impl Fn(&Rig) -> bool) {
        for _ in 0..n {
            if f(self) {
                return;
            }
            self.step(stick, &[A]);
            self.step(stick, &[]);
        }
        panic!("mash: not reached; state {:?} clip {} log {:?}", self.state(), self.m().anim.clip.name, self.k().log);
    }

    fn step(&mut self, stick: Vec2, held: &[PadButton]) {
        let mut now = PadButtons::NONE;
        for b in held {
            now.set(*b, true);
        }
        let pad = Buttons { held: now, pressed: PadButtons(now.0 & !self.held.0), released: PadButtons(self.held.0 & !now.0) };
        self.held = now;
        if let Some(pad) = self.party.member_mut(self.id).and_then(|m| m.pad.as_mut()).filter(|p| p.finisher_pending) {
            pad.finisher_pending = false;
            pad.finisher_answer = Some(false);
        }
        self.party.set_pad_frame(self.id, PadFrame { stick, camera_yaw: self.camera, pad });
        let mut host = PartyHost { world: &self.world, targets: &mut self.dummies, audio: &mut self.audio, effects: &mut self.effects };
        self.party.step(DT, &self.sora, &mut host);
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

    /// An enemy hit on him from `from` (the attacker's position), `damage` HP.
    fn hit_from(&mut self, from: Vec3, damage: f32, kind: Kind) -> HitResult {
        let me = self.m().position();
        let mut h = HitRecord::empty(TargetId::NONE);
        h.damage = damage;
        h.damage_kind = kind;
        h.direction = horizontal(me - from).normalize_or_zero();
        h.attacker = HitAttacker { position: from, yaw: 0.0, radius: 50.0, valid: true };
        h.position = me + Vec3::Z * 90.0;
        self.party.receive_hit(self.id, &h)
    }

    fn state(&self) -> NpcStateId {
        self.m().state
    }

    fn hits(&self, row: &str) -> Vec<HitRecord> {
        self.dummies.1.iter().filter(|h| h.attack_name == row).copied().collect()
    }

    fn logged(&self, w: &str) -> bool {
        self.k().log.contains(&w)
    }

    /// Runs A presses (tap, then release) until the predicate holds, at most `n` steps.
    fn until(&mut self, n: usize, stick: Vec2, held: &[PadButton], f: impl Fn(&Rig) -> bool) -> usize {
        for i in 0..n {
            if f(self) {
                return i;
            }
            self.step(stick, held);
        }
        panic!("condition not met in {n} steps: state {:?} clip {} log {:?}", self.state(), self.m().anim.clip.name, self.k().log);
    }

    fn tap(&mut self, b: PadButton, stick: Vec2) {
        self.step(stick, &[b]);
        self.step(stick, &[]);
    }
}

fn front() -> Vec3 {
    Vec3::new(200.0, 0.0, 0.0)
}

// ------------------------------------------------------------------------------------------------ the guard

#[test]
fn x_raises_the_shield_and_he_walks_shield_up_at_180() {
    let mut rig = Rig::new(&[]);
    rig.step(NONE, &[X]);
    assert_eq!(rig.state(), NpcStateId::Pad(GUARD), "X pressed: the guard on the press step (latency 0)");
    assert_eq!(rig.m().anim.clip.name, "SGuardIdle");
    rig.run(60, FWD, &[X]);
    assert_eq!(rig.m().anim.clip.name, "SGuardWalk");
    let v = horizontal(rig.m().cmc.velocity).length();
    assert!((v - K.guard_walk_speed).abs() < 3.0, "shield-up walk {v}");
    rig.step(NONE, &[]);
    rig.idle(2);
    assert!(!matches!(rig.state(), NpcStateId::Pad(GUARD)), "let go: the shield goes down");
}

#[test]
fn perfect_guard_is_free_and_opens_the_counter_window() {
    let mut rig = Rig::new(&[]);
    rig.run(6, NONE, &[X]); // 0.1 s after raising it
    let before = (rig.m().status.hp, rig.guard().value);
    let r = rig.hit_from(front(), 40.0, Kind::Small);
    assert!(r.guarded && r.accepted && r.damage_dealt == 0.0, "{r:?}");
    assert_eq!(rig.m().status.hp, before.0, "a Perfect Guard costs nothing");
    assert!(rig.guard().value >= before.1, "and gives GUARD back (guard.perfect_refund)");
    assert!(rig.logged("perfect_guard"));
    assert!((rig.k().counter - K.perfect_counter_window).abs() < 1e-4);
    assert_eq!(rig.state(), NpcStateId::Pad(GUARD_HIT));
    assert_eq!(rig.m().anim.clip.name, "SGuardHit");
    // the counter window: A bashes with X let go
    rig.idle(3);
    rig.tap(A, NONE);
    assert_eq!(rig.state(), NpcStateId::Pad(BASH), "A in the counter window: Shield Bash");
}

#[test]
fn a_late_block_spends_guard_and_an_empty_guard_breaks_the_guard() {
    let mut rig = Rig::new(&[]);
    rig.run(30, NONE, &[X]); // 0.5 s: past the Perfect Guard window (Iron Wall 0.40)
    let max = rig.m().status.max_hp() as f32;
    let before = rig.guard().value;
    let r = rig.hit_from(front(), max * 0.1, Kind::Small);
    assert!(r.guarded);
    let drained = before - rig.guard().value;
    assert!((drained - K.guard_drain_share * 0.1).abs() < 0.5, "a block is paid from GUARD: {drained}");
    assert!(rig.logged("block"));
    let mut n = 0;
    while !rig.guard().broken {
        rig.run(30, NONE, &[X]);
        rig.hit_from(front(), max * 0.1, Kind::Small);
        n += 1;
        assert!(n < 20);
    }
    assert_eq!(rig.state(), NpcStateId::Pad(GUARD_BREAK));
    assert_eq!(rig.m().anim.clip.name, "SGuardDamage");
    assert!(rig.logged("guard_break"));
    // open to hits now
    let hp = rig.m().status.hp;
    let r = rig.hit_from(front(), 10.0, Kind::Small);
    assert!(!r.guarded && rig.m().status.hp < hp, "a broken guard lets hits through");
    // no shield until GUARD rearms (GG's rule)
    rig.run(60, NONE, &[]);
    rig.step(NONE, &[X]);
    assert_ne!(rig.state(), NpcStateId::Pad(GUARD));
}

#[test]
fn holding_the_shield_drains_guard_slowly_and_holds_the_refill() {
    let mut rig = Rig::new(&[]);
    let g0 = rig.guard().value;
    rig.run(120, NONE, &[X]);
    let g1 = rig.guard().value;
    assert!((g0 - g1 - 2.0 * K.guard_hold_drain).abs() < 0.5, "2 s held: {g0} -> {g1}");
    rig.idle(1);
    let g2 = rig.guard().value;
    let delay = crate::guard::GP.guard_refill_delay;
    rig.idle(((delay - 0.1) * 60.0) as usize);
    assert!((rig.guard().value - g2).abs() < 1e-3, "no refill inside GUARD's delay");
    rig.idle(30);
    assert!(rig.guard().value > g2, "then it refills");
}
#[test]
fn the_guard_is_frontal_and_grabs_go_through() {
    let mut rig = Rig::new(&[]);
    rig.run(30, NONE, &[X]);
    let r = rig.hit_from(Vec3::new(-200.0, 0.0, 0.0), 10.0, Kind::Small);
    assert!(!r.guarded, "from behind");
    rig.mm().status.hp = rig.m().status.max_hp();
    rig.idle(60);
    rig.run(30, NONE, &[X]);
    let r = rig.hit_from(front(), 10.0, Kind::Catch);
    assert!(!r.guarded, "a grab");
    rig.idle(60);
    rig.run(30, NONE, &[X]);
    let r = rig.hit_from(Vec3::new(200.0, 100.0, 0.0), 10.0, Kind::Small);
    assert!(r.guarded, "26 deg off his facing: inside the 120 deg front");
}

#[test]
fn iron_wall_widens_the_perfect_guard_window() {
    for iron in [false, true] {
        let mut rig = Rig::new(&[]);
        rig.mm().status.goofy_kit.set_equipped(GA::IronWall, iron);
        rig.run(21, NONE, &[X]); // 0.35 s
        rig.hit_from(front(), 30.0, Kind::Small);
        assert_eq!(rig.logged("perfect_guard"), iron, "iron wall {iron}");
    }
}

#[test]
fn rally_gives_mp_on_a_perfect_guard() {
    let mut rig = Rig::new(&[]);
    rig.mm().status.mp = 10;
    rig.run(3, NONE, &[X]);
    rig.hit_from(front(), 30.0, Kind::Small);
    assert_eq!(rig.m().status.mp, 10 + K.rally_mp as i32);
}

// ------------------------------------------------------------------------------------------------ the counter chain

#[test]
fn the_counter_chain_bash_bash2_then_the_held_heavy_bash() {
    let mut rig = Rig::new(&[Vec3::new(180.0, 0.0, 0.0)]);
    rig.mm().status.goofy_kit.set_equipped(GA::PaybackBash, false);
    rig.run(3, NONE, &[X]);
    rig.hit_from(front(), 30.0, Kind::Small);
    assert!(rig.logged("perfect_guard") && rig.k().brace == 1);
    rig.idle(2);
    rig.tap(A, NONE);
    rig.until(60, NONE, &[], |r| !r.hits("FR_SHIELDGUARDIAN_SHIELDBASH").is_empty());
    let b1 = rig.hits("FR_SHIELDGUARDIAN_SHIELDBASH")[0];
    let want = 2.0 * K.bash_mult * K.counter_mult * (1.0 + K.brace_per);
    assert!((b1.info.power - want).abs() < 1e-3, "the Counter Bash (x{}) with one Brace: {} vs {want}", K.counter_mult, b1.info.power);
    assert_eq!(b1.damage_kind, Kind::Blow);
    assert_eq!(rig.k().brace, 0, "the Brace is spent");
    rig.tap(A, NONE);
    rig.until(90, NONE, &[], |r| !r.hits("FR_SHIELDGUARDIAN_SHIELDBASH2").is_empty());
    let b2 = rig.hits("FR_SHIELDGUARDIAN_SHIELDBASH2")[0];
    assert!((b2.info.power - 3.0 * K.bash2_mult).abs() < 1e-3, "Shield Bash 2 = 1.1: {}", b2.info.power);
    assert_eq!(b2.damage_kind, Kind::Blow);
    // third: hold A for the Heavy Bash; it spends GUARD
    let g = rig.guard().value;
    rig.until(60, NONE, &[A], |r| r.logged("heavy"));
    rig.until(90, NONE, &[A], |r| !r.hits("FR_SHIELDGUARDIAN_HEAVYBASH").is_empty());
    let h = rig.hits("FR_SHIELDGUARDIAN_HEAVYBASH")[0];
    assert!((h.info.power - 8.0 * K.heavy_mult).abs() < 1e-3, "Heavy Bash = 2.0: {}", h.info.power);
    assert_eq!(h.damage_kind, Kind::Parabola);
    assert!(g - rig.guard().value >= K.guard_heavy_cost - 0.5, "GUARD spent: {g} -> {}", rig.guard().value);
}
#[test]
fn a_short_third_press_is_a_shield_bash_not_the_heavy() {
    let mut rig = Rig::new(&[Vec3::new(180.0, 0.0, 0.0)]);
    rig.run(3, NONE, &[X]);
    rig.hit_from(front(), 30.0, Kind::Small);
    rig.idle(2);
    rig.tap(A, NONE);
    rig.until(60, NONE, &[], |r| r.k().bash_hit.is_some());
    rig.tap(A, NONE);
    rig.until(60, NONE, &[], |r| r.k().bash == BashKind::Bash2 && r.k().bash_hit.is_some());
    rig.tap(A, NONE);
    rig.idle(3);
    assert_eq!(rig.k().bash, BashKind::Bash1, "let go before 0.6 s: the chain starts over");
}

#[test]
fn holding_a_in_guard_turns_the_bash_into_the_heavy_bash() {
    let mut rig = Rig::new(&[Vec3::new(180.0, 0.0, 0.0)]);
    rig.run(10, NONE, &[X]);
    rig.step(NONE, &[X, A]);
    assert_eq!(rig.k().bash, BashKind::Bash1);
    rig.until(60, NONE, &[X, A], |r| r.k().bash == BashKind::Heavy);
    rig.until(60, NONE, &[X, A], |r| !r.hits("FR_SHIELDGUARDIAN_HEAVYBASH").is_empty());
}

#[test]
fn payback_bash_is_x1_5_and_blow_after_a_block_and_the_brace_adds() {
    let mut rig = Rig::new(&[Vec3::new(180.0, 0.0, 0.0)]);
    rig.run(30, NONE, &[X]);
    rig.hit_from(front(), 30.0, Kind::Small);
    assert_eq!(rig.k().brace, 1, "a block is stored as Brace");
    rig.idle(1);
    rig.run(25, NONE, &[X]);
    rig.step(NONE, &[X, A]);
    rig.step(NONE, &[X]);
    rig.until(60, NONE, &[X], |r| !r.hits("FR_SHIELDGUARDIAN_SHIELDBASH").is_empty());
    let b = rig.hits("FR_SHIELDGUARDIAN_SHIELDBASH")[0];
    let want = 2.0 * K.bash_mult * K.payback_mult * (1.0 + K.brace_per);
    assert!((b.info.power - want).abs() < 1e-3, "{} vs {want}", b.info.power);
    assert_eq!(b.damage_kind, Kind::Blow);
}

#[test]
fn a_full_brace_makes_the_next_bash_a_blow() {
    let mut rig = Rig::new(&[Vec3::new(180.0, 0.0, 0.0)]);
    rig.mm().status.goofy_kit.set_equipped(GA::PaybackBash, false);
    for _ in 0..3 {
        rig.run(30, NONE, &[X]);
        rig.hit_from(front(), 10.0, Kind::Small);
    }
    assert_eq!(rig.k().brace, K.brace_max as u8);
    rig.run(30, NONE, &[X]);
    rig.step(NONE, &[X, A]);
    rig.step(NONE, &[X]);
    rig.until(60, NONE, &[X], |r| !r.hits("FR_SHIELDGUARDIAN_SHIELDBASH").is_empty());
    let b = rig.hits("FR_SHIELDGUARDIAN_SHIELDBASH")[0];
    assert!((b.info.power - 2.0 * K.bash_mult * (1.0 + K.brace_per * K.brace_max)).abs() < 1e-3, "{}", b.info.power);
    assert_eq!(b.damage_kind, Kind::Blow);
    // unused, the Brace fades
    let mut rig = Rig::new(&[]);
    rig.run(30, NONE, &[X]);
    rig.hit_from(front(), 10.0, Kind::Small);
    rig.idle(((K.brace_keep + 0.2) * 60.0) as usize);
    assert_eq!(rig.k().brace, 0);
}

#[test]
fn a_guard_bash_lunges_at_its_target() {
    let mut rig = Rig::new(&[Vec3::new(400.0, 0.0, 0.0)]);
    rig.mm().pad.as_mut().unwrap().lock = TargetId(1);
    rig.run(30, NONE, &[X]);
    let x0 = rig.m().position().x;
    rig.step(NONE, &[X, A]);
    rig.step(NONE, &[X]);
    rig.until(60, NONE, &[X], |r| !r.hits("FR_SHIELDGUARDIAN_SHIELDBASH").is_empty());
    let d = rig.m().position().x - x0;
    assert!(d > 120.0 && d <= K.bash_lunge_max + 5.0, "the user: 'he doesn't move at all when he does a shield bash': {d}");
}
#[test]
fn over_here_taunts_on_a_bash_hit() {
    let mut rig = Rig::new(&[Vec3::new(180.0, 0.0, 0.0)]);
    rig.run(3, NONE, &[X]);
    rig.hit_from(front(), 30.0, Kind::Small);
    rig.idle(2);
    rig.tap(A, NONE);
    rig.until(60, NONE, &[], |r| r.k().taunt.is_some());
    let t = rig.party.goofy_taunt(rig.id).unwrap();
    assert_eq!((t.radius, t.left.ceil()), (K.over_here_radius, K.over_here_time));
}

#[test]
fn shield_charge_dashes_with_the_shield_up() {
    let mut rig = Rig::new(&[Vec3::new(300.0, 0.0, 0.0)]);
    rig.run(10, NONE, &[X]);
    let x0 = rig.m().position().x;
    let g0 = rig.guard().value;
    rig.step(FWD, &[X, A]);
    assert_eq!(rig.state(), NpcStateId::Pad(CHARGE));
    assert!(g0 - rig.guard().value >= K.guard_charge_cost - 0.5, "it spends GUARD");
    rig.run(24, FWD, &[X]);
    let d = rig.m().position().x - x0;
    assert!((d - K.charge_speed * K.charge_time).abs() < 60.0, "charge distance {d}");
    assert!(!rig.hits("FR_SHIELDGUARDIAN_SHIELDBASH").is_empty(), "contact on the way");
    // a hit from the front during the charge is blocked
    rig.step(FWD, &[X, A]);
    let r = rig.hit_from(rig.m().position() + Vec3::X * 200.0, 10.0, Kind::Small);
    assert!(r.guarded);
}
// ------------------------------------------------------------------------------------------------ X moves

#[test]
fn hop_dodge_300_cm_invincible_0_03_to_0_30() {
    let mut rig = Rig::new(&[]);
    let p0 = rig.m().position();
    rig.step(RIGHT, &[X]);
    assert_eq!(rig.state(), NpcStateId::Pad(HOP), "X + stick: the hop on the press step");
    assert_eq!(rig.m().anim.clip.name, "HopR");
    // state time 0 now: not yet invincible
    rig.step(NONE, &[]);
    rig.step(NONE, &[]);
    rig.step(NONE, &[]);
    assert!(invincible(rig.m()), "invincible at {}", rig.m().state_time);
    let r = rig.hit_from(rig.m().position() + Vec3::X * 200.0, 10.0, Kind::Small);
    assert!(!r.accepted);
    rig.run(16, NONE, &[]);
    let d = horizontal(rig.m().position() - p0);
    assert!((d.length() - K.hop_distance).abs() < 25.0 && d.y > 250.0, "hop {d}");
    rig.run(3, NONE, &[]);
    assert!(!invincible(rig.m()));
    // a left hop plays HopL
    let mut rig = Rig::new(&[]);
    rig.step(-RIGHT, &[X]);
    assert_eq!(rig.m().anim.clip.name, "HopL");
}

#[test]
fn no_air_dodge_x_in_the_air_is_only_the_air_guard() {
    let mut rig = Rig::new(&[]);
    rig.step(NONE, &[B]);
    rig.run(20, NONE, &[B]);
    assert!(!rig.m().on_ground());
    rig.step(FWD, &[X]);
    assert!(!rig.logged("surf"), "Shield Surf is removed (the user: 'Remove Goofy's air dodge')");
    assert!(!invincible(rig.m()), "no air dodge invincibility");
    assert!(matches!(rig.state(), NpcStateId::Pad(AIR_GUARD) | NpcStateId::Pad(JUMP)), "{:?}", rig.state());
    rig.step(NONE, &[]);
    rig.step(NONE, &[X]);
    if !rig.m().on_ground() {
        assert_eq!(rig.state(), NpcStateId::Pad(AIR_GUARD), "X held in the air: the air guard (a block, not a dodge)");
    }
}
#[test]
fn air_guard_blocks_and_a_is_the_air_bash() {
    let mut rig = Rig::new(&[Vec3::new(180.0, 0.0, 120.0)]);
    rig.step(NONE, &[B]);
    rig.run(10, NONE, &[B]);
    rig.step(NONE, &[X]);
    assert_eq!(rig.state(), NpcStateId::Pad(AIR_GUARD));
    rig.run(4, NONE, &[X]);
    let r = rig.hit_from(rig.m().position() + Vec3::X * 200.0, 10.0, Kind::Small);
    assert!(r.guarded);
    rig.idle(1);
    rig.step(NONE, &[X]);
    rig.step(NONE, &[X, A]);
    rig.until(30, NONE, &[X], |r| r.k().bash == BashKind::Air && r.state() == NpcStateId::Pad(BASH));
}

#[test]
fn guard_jump_keeps_the_shield_up() {
    let mut rig = Rig::new(&[]);
    rig.run(10, NONE, &[X]);
    rig.step(NONE, &[X, B]);
    assert_eq!(rig.state(), NpcStateId::Pad(AIR_GUARD));
    rig.run(10, NONE, &[X]);
    assert!(!rig.m().on_ground());
    rig.until(120, NONE, &[X], |r| r.state() == NpcStateId::Pad(GUARD) && r.m().on_ground());
}

// ------------------------------------------------------------------------------------------------ finishers

#[test]
fn ground_combo_push_sweep_big_bash_lunges_and_hit_stop() {
    let mut rig = Rig::new(&[Vec3::new(260.0, 0.0, 0.0)]);
    rig.wear_only(&[]);
    rig.mm().pad.as_mut().unwrap().lock = TargetId(1);
    let x0 = rig.m().position().x;
    rig.tap(A, NONE);
    assert_eq!(rig.state(), NpcStateId::Pad(super::super::goofy_combo::COMBO), "A: his own combo (not stream P's ActionDef)");
    assert_eq!(rig.m().anim.clip.name, "M_1301");
    assert!(rig.m().action.is_none());
    // the hit-stop at the first landed hit: his clip stands still
    rig.until(60, NONE, &[], |r| r.k().hitstop > 0.0);
    let t = rig.m().anim.time;
    rig.step(NONE, &[]);
    assert!((rig.m().anim.time - t).abs() < 1e-5 && rig.m().anim.speed == 0.0, "frozen");
    rig.mash(200, NONE, |r| r.logged("fin_big_bash"));
    assert_eq!(rig.steps()[..3], ["combo_push", "combo_sweep", "fin_big_bash"], "{:?}", rig.steps());
    rig.until(120, NONE, &[], |r| !r.hits("FR_SHIELDGUARDIAN_SHIELDBASH2").is_empty());
    let (p1, p2, f) = (rig.hits("Attack1_Push")[0], rig.hits("Attack2_Sweep")[0], rig.hits("FR_SHIELDGUARDIAN_SHIELDBASH2")[0]);
    assert!((p1.info.power - K.step_push_power).abs() < 1e-4 && (p2.info.power - K.step_sweep_power).abs() < 1e-4);
    assert!((f.info.power - K.step_big_power).abs() < 1e-4 && f.damage_kind == Kind::Blow, "Big Bash {} {:?}", f.info.power, f.damage_kind);
    assert!(rig.m().position().x - x0 > 40.0, "the steps lunged at the target");
    // Sora's scale: the whole base string stays near Sora's KK combo (0.6 + 0.8 + 0.8 + 1.2)
    let total = K.step_push_power + K.step_sweep_power + K.step_big_power;
    assert!(total < 3.4 && total > 2.0, "{total}");
}

#[test]
fn combo_plus_adds_steps_up_to_four_plus_the_finisher() {
    let mut rig = Rig::new(&[Vec3::new(240.0, 0.0, 0.0)]);
    rig.wear_only(&[GA::ComboPlus, GA::ComboPlus2]);
    rig.mm().pad.as_mut().unwrap().lock = TargetId(1);
    rig.mash(400, NONE, |r| r.logged("fin_big_bash"));
    assert_eq!(rig.steps()[..5], ["combo_push", "combo_sweep", "combo_bash", "combo_shove", "fin_big_bash"], "{:?}", rig.steps());
    let mut rig = Rig::new(&[Vec3::new(240.0, 0.0, 0.0)]);
    rig.wear_only(&[GA::ComboPlus]);
    rig.mm().pad.as_mut().unwrap().lock = TargetId(1);
    rig.mash(400, NONE, |r| r.logged("fin_big_bash"));
    assert_eq!(rig.steps()[..4], ["combo_push", "combo_sweep", "combo_bash", "fin_big_bash"], "{:?}", rig.steps());
}

#[test]
fn a_far_target_opens_with_the_shield_dash() {
    let mut rig = Rig::new(&[Vec3::new(650.0, 0.0, 0.0)]);
    rig.wear_only(&[]);
    rig.mm().pad.as_mut().unwrap().lock = TargetId(1);
    let x0 = rig.m().position().x;
    rig.tap(A, NONE);
    assert!(rig.logged("combo_dash"));
    rig.until(60, NONE, &[], |r| !r.hits("FR_SHIELDGUARDIAN_SHIELDBASH").is_empty());
    let d = rig.m().position().x - x0;
    assert!(d > 400.0, "the dash closes the gap: {d}");
}

#[test]
fn the_finisher_choice_rule() {
    use super::super::goofy_combo::{choose, Fin, Step};
    let mut rig = Rig::new(&[Vec3::new(700.0, 0.0, 0.0)]);
    let pick = |rig: &mut Rig| {
        let host = PartyHost { world: &rig.world, targets: &mut rig.dummies, audio: &mut rig.audio, effects: &mut rig.effects };
        match choose(rig.party.member(rig.id).unwrap(), &host) {
            Fin::Step(s) => format!("{s:?}"),
            Fin::Fusion(f) => format!("{f:?}"),
        }
    };
    rig.mm().pad.as_mut().unwrap().lock = TargetId(1);
    assert_eq!(pick(&mut rig), "Big", "7 m, stick neutral: no automatic rocket");
    {
        let pad = rig.mm().pad.as_mut().unwrap();
        (pad.stick_yaw, pad.stick_magnitude) = (0.0, 1.0);
    }
    assert_eq!(pick(&mut rig), "Rocket", "7 m, the stick toward it: Fusion Rocket");
    {
        let pad = rig.mm().pad.as_mut().unwrap();
        (pad.stick_yaw, pad.stick_magnitude) = (180.0, 1.0);
    }
    assert_eq!(pick(&mut rig), "Upper", "the stick held away: Shield Upper");
    rig.mm().pad.as_mut().unwrap().stick_magnitude = 0.0;
    rig.dummies.0 = vec![(TargetId(1), Vec3::new(150.0, 0.0, 0.0)), (TargetId(2), Vec3::new(100.0, 0.0, 0.0)), (TargetId(3), Vec3::new(-100.0, 50.0, 0.0))];
    assert_eq!(pick(&mut rig), "Spin", "crowded");
    rig.wear_only(&[]);
    assert_eq!(pick(&mut rig), format!("{:?}", Step::Big), "nothing worn: Big Bash");
}

#[test]
fn shield_quake_by_holding_a_on_the_finisher() {
    let mut rig = Rig::new(&[Vec3::new(240.0, 0.0, 0.0)]);
    rig.wear_only(&[GA::ShieldQuake]);
    rig.mm().pad.as_mut().unwrap().lock = TargetId(1);
    rig.mash(200, NONE, |r| r.logged("fin_big_bash"));
    rig.until(60, NONE, &[A], |r| r.logged("fin_shield_quake"));
    assert_eq!(rig.m().anim.clip.name, "SGuardHeavyBash");
    rig.until(90, NONE, &[], |r| !r.hits("FR_SHIELDGUARDIAN_HEAVYBASH").is_empty());
    let h = rig.hits("FR_SHIELDGUARDIAN_HEAVYBASH")[0];
    assert!((h.info.power - K.step_quake_power).abs() < 1e-4 && h.damage_kind == Kind::Parabola);
}

#[test]
fn shield_upper_launches_with_toss_and_lands() {
    let mut rig = Rig::new(&[Vec3::new(240.0, 0.0, 0.0)]);
    rig.wear_only(&[GA::ShieldUpper]);
    rig.mm().pad.as_mut().unwrap().lock = TargetId(1);
    rig.mash(200, NONE, |r| r.steps().len() >= 2);
    // the finisher press with the stick held away from the target (camera yaw 0: stick back = yaw 180)
    rig.mash(100, -FWD, |r| r.logged("fin_shield_upper"));
    assert!(wrap180(rig.m().yaw).abs() < 30.0, "he keeps facing the target, not the pulled stick: {}", rig.m().yaw);
    let mut top = 0.0f32;
    for _ in 0..120 {
        rig.step(NONE, &[]);
        top = top.max(rig.m().position().z);
        if !rig.hits("Attack3_JumpAscent").is_empty() && rig.m().on_ground() && rig.k().combo.phase >= 2 {
            break;
        }
    }
    let h = rig.hits("Attack3_JumpAscent")[0];
    assert_eq!(h.damage_kind, Kind::Toss, "a launch");
    assert!((h.info.power - K.step_upper_power).abs() < 1e-4);
    assert!(top > K.combo_upper_hop * 0.6 && top < K.combo_upper_hop * 1.25, "he hops about combo.upper_hop: {top}");
    assert!(rig.m().state_time < 1.2, "and lands soon after (no float): {}", rig.m().state_time);
}

#[test]
fn link_points_and_cancels_of_a_step() {
    use super::super::goofy_combo as gc;
    let mut rig = Rig::new(&[Vec3::new(240.0, 0.0, 0.0)]);
    rig.wear_only(&[]);
    rig.mm().pad.as_mut().unwrap().lock = TargetId(1);
    rig.tap(A, NONE);
    // the stick before the move point does nothing (the committal recovery)
    rig.run(5, FWD, &[]);
    assert_eq!(rig.state(), NpcStateId::Pad(gc::COMBO));
    // B before the link point: no jump
    rig.step(NONE, &[B]);
    rig.step(NONE, &[]);
    assert_eq!(rig.state(), NpcStateId::Pad(gc::COMBO));
    // the hit confirm: X + stick hops out after the first landed hit
    rig.until(60, NONE, &[], |r| r.k().combo.landed.is_some() && r.k().hitstop <= 0.0);
    rig.until(20, NONE, &[], |r| gc::confirm_open(r.m()));
    assert!(!gc::link_open(rig.m()) || rig.m().anim.time >= K.step_push_link);
    rig.step(RIGHT, &[X]);
    assert_eq!(rig.state(), NpcStateId::Pad(HOP), "hit confirm -> Hop");
    // the link point: an ability starts from it (link_for_ability), B jumps
    let mut rig = Rig::new(&[Vec3::new(240.0, 0.0, 0.0)]);
    rig.wear_only(&[]);
    rig.mm().pad.as_mut().unwrap().lock = TargetId(1);
    rig.tap(A, NONE);
    rig.until(90, NONE, &[], |r| gc::link_open(r.m()));
    assert!(link_for_ability(rig.m()));
    rig.step(NONE, &[B]);
    assert_eq!(rig.state(), NpcStateId::Pad(JUMP), "B at the link point jumps");
    // the move point: the stick runs out
    let mut rig = Rig::new(&[Vec3::new(240.0, 0.0, 0.0)]);
    rig.wear_only(&[]);
    rig.tap(A, NONE);
    rig.until(120, NONE, &[], |r| gc::move_open(r.m()));
    rig.step(FWD, &[]);
    assert_eq!(rig.state(), NpcStateId::Pad(RUN));
}

#[test]
fn a_combo_step_links_into_his_abilities() {
    let mut rig = Rig::new(&[Vec3::new(240.0, 0.0, 0.0)]);
    rig.wear_only(&[]);
    rig.mm().pad.as_mut().unwrap().lock = TargetId(1);
    rig.tap(A, NONE);
    rig.until(90, NONE, &[], |r| super::super::goofy_combo::link_open(r.m()));
    // LB + B: the first shortcut (Goofy Tornado) from the link point
    rig.step(NONE, &[PadButton::LeftShoulder]);
    rig.step(NONE, &[PadButton::LeftShoulder, B]);
    rig.step(NONE, &[]);
    assert_eq!(rig.m().action.and_then(|a| a.def), Some(ActionDef::GoofyTornado), "state {:?} log {:?}", rig.state(), rig.k().log);
    assert!(rig.m().anim.speed > 1.2, "the leader's faster Tornado start ({})", rig.m().anim.speed);
}

#[test]
fn air_combo_is_short_and_air_combo_plus_adds_the_air_shield_bash() {
    for plus in [false, true] {
        let mut rig = Rig::new(&[Vec3::new(160.0, 0.0, 150.0)]);
        rig.wear_only(if plus { &[GA::AirComboPlus] } else { &[] });
        rig.step(NONE, &[B]);
        rig.run(14, NONE, &[B]);
        for _ in 0..80 {
            rig.step(NONE, &[A]);
            rig.step(NONE, &[]);
            if rig.m().on_ground() && rig.m().state_time > 0.2 {
                break;
            }
        }
        let air: Vec<&str> = rig.steps().into_iter().filter(|w| w.starts_with("air_")).collect();
        let want: &[&str] = if plus { &["air_strike", "air_smash", "air_shield_bash"] } else { &["air_strike", "air_smash"] };
        assert_eq!(&air[..], want, "plus {plus}");
        assert!(!rig.logged("bombardier"), "no air finisher: Bombardier is Y");
    }
}
#[test]
fn fusion_spin_when_crowded_ends_the_string() {
    let mut rig = Rig::new(&[Vec3::new(150.0, 0.0, 0.0), Vec3::new(120.0, 120.0, 0.0)]);
    rig.wear_only(&[GA::FusionSpin]);
    rig.mash(300, NONE, |r| r.state() == NpcStateId::Pad(FINISH));
    assert_eq!(rig.k().fin, Finisher::Spin);
    assert!(invincible(rig.m()), "until the release");
    rig.until(60, NONE, &[], |r| r.k().released);
    rig.until(60, NONE, &[], |r| r.hits("FUSION_SPIN").len() >= 2);
    let h = rig.hits("FUSION_SPIN")[0];
    assert!((h.info.power - 1.8 * K.fin_mult).abs() < 1e-3);
}
#[test]
fn the_string_works_without_a_target() {
    let mut rig = Rig::new(&[]);
    rig.wear_only(&[]);
    rig.mash(300, NONE, |r| r.logged("fin_big_bash"));
    assert_eq!(rig.m().anim.clip.name, "SGuardShieldBash2");
}
#[test]
fn y_is_bombardier_with_a_cooldown_shown_on_the_y_row() {
    let mut rig = Rig::new(&[Vec3::new(500.0, 0.0, 0.0)]);
    let id = rig.id;
    // the Y row: Bombardier ready (no timer); never Shield Guardian (the user, 2026-10-17 00:40)
    rig.mm().pad.as_mut().unwrap().goofy.mode_gauge = K.mode_gauge;
    let row = rig.party.pad_menu(id).unwrap().situations[0];
    assert_eq!((row.name, row.total, row.enabled), ("Bombardier", 0.0, true));
    assert!(rig.party.pad_menu(id).unwrap().situations.iter().all(|s| s.name != "Shield Guardian" && s.name != "Finish"));
    rig.mm().pad.as_mut().unwrap().lock = TargetId(1);
    rig.tap(Y, NONE);
    assert_eq!(rig.state(), NpcStateId::Pad(DIVE), "Y on the ground: Bombardier");
    assert!(rig.m().pad.as_ref().unwrap().x2.y_taken || rig.logged("bombardier"));
    assert!(invincible(rig.m()));
    assert_eq!(rig.m().anim.clip.name, "GShootRise");
    let z0 = rig.m().position().z;
    rig.until(60, NONE, &[], |r| r.k().dive_sub == 1);
    assert!(rig.m().position().z - z0 > K.bomb_ground_rise * 0.7, "it rises from the ground: {} (mode {:?})", rig.m().position().z - z0, rig.m().cmc.mode);
    assert_eq!(rig.m().anim.clip.name, "GShootEnd");
    rig.until(120, NONE, &[], |r| r.logged("bombardier_exp"));
    rig.idle(30);
    let e = rig.hits("FR_GOOFYSHOOT_EXP");
    assert!(!e.is_empty());
    assert!((e[0].info.power - 8.0 * K.dive_exp_mult).abs() < 1e-4, "2.0 on Sora's scale: {}", e[0].info.power);
    // the cooling row counts down
    let r1 = rig.party.pad_menu(id).unwrap().situations[0];
    assert!(!r1.enabled && r1.total == K.bomb_cooldown && r1.remaining > 0.0 && r1.remaining < K.bomb_cooldown, "{r1:?}");
    rig.idle(30);
    let r2 = rig.party.pad_menu(id).unwrap().situations[0];
    assert!(r2.remaining < r1.remaining - 0.4, "it goes down: {} -> {}", r1.remaining, r2.remaining);
    // Y while cooling: refused
    rig.tap(Y, NONE);
    assert_ne!(rig.state(), NpcStateId::Pad(DIVE));
    rig.idle(((K.bomb_cooldown + 0.2) * 60.0) as usize);
    let r3 = rig.party.pad_menu(id).unwrap().situations[0];
    assert!(r3.enabled && r3.total == 0.0, "ready again");
    rig.tap(Y, NONE);
    assert_eq!(rig.state(), NpcStateId::Pad(DIVE));
}
// ------------------------------------------------------------------------------------------------ feel

#[test]
fn the_hop_has_a_cooldown_and_a_late_press_is_kept() {
    let mut rig = Rig::new(&[]);
    rig.step(RIGHT, &[X]);
    assert_eq!(rig.state(), NpcStateId::Pad(HOP));
    rig.run(40, NONE, &[]);
    // 0.68 s after the hop started: still cooling (0.9 s), early: refused
    rig.step(RIGHT, &[X]);
    assert_ne!(rig.state(), NpcStateId::Pad(HOP));
    assert!(rig.logged("hop_refused"));
    rig.idle(1);
    // inside the buffer before its end: kept, it hops when the cooldown ends
    rig.until(60, NONE, &[], |r| r.k().hop_cd <= K.hop_buffer - 0.05);
    rig.step(RIGHT, &[X]);
    assert_ne!(rig.state(), NpcStateId::Pad(HOP));
    rig.until(20, NONE, &[], |r| r.state() == NpcStateId::Pad(HOP));
}
#[test]
fn feel_numbers_against_sora() {
    let sa = kh_core::actions::ACTION_PARAMS;
    let d = super::super::goofy_combo::def(super::super::goofy_combo::Step::Push);
    println!("FEEL | Shield Push: hit {:.3} s after the press, link {:.3} s, move {:.3} s (clip at {}x)", d.hit / d.rate, d.link / d.rate, d.r#move / d.rate, d.rate);
    println!("FEEL | hit-stop {:.3} s per step, {:.3} s on the finisher (KH3 MyHitStopTimeSmall/Big 0.02/0.05, unused by the game)", K.step_push_hitstop, K.step_big_hitstop);
    assert!(d.hit / d.rate < 0.4, "a Push lands within 0.4 s (was 0.46 at 1x)");
    // latencies: the press step
    let mut rig = Rig::new(&[]);
    rig.step(NONE, &[X]);
    let guard_latency = (rig.state() == NpcStateId::Pad(GUARD)) as i32;
    let mut rig = Rig::new(&[]);
    rig.step(RIGHT, &[X]);
    let hop_latency = (rig.state() == NpcStateId::Pad(HOP)) as i32;
    let mut rig = Rig::new(&[]);
    rig.step(NONE, &[A]);
    let a_latency = (rig.state() == NpcStateId::Pad(super::super::goofy_combo::COMBO)) as i32;
    println!("FEEL | latency: guard {} step, hop {} step, attack {} step (0 frames after the press)", 1 - guard_latency, 1 - hop_latency, 1 - a_latency);
    assert_eq!((guard_latency, hop_latency, a_latency), (1, 1, 1));
    println!("FEEL | Perfect Guard window {:.2} s (Iron Wall {:.2}) vs Sora's just-guard {:.3} s; held guard vs Sora's tap guard {:.2} s; counter window {:.2} s = Sora's {:.2} s", K.perfect_window, K.perfect_window_iron, sa.guard_just_guard_time, sa.guard_window_end, K.perfect_counter_window, sa.guard_counter_window);
    println!("FEEL | dodge: Hop invincible {:.2}-{:.2} s ({:.2} s), cooldown {:.2} s, vs Sora's roll 0-{:.3} s", K.hop_inv_from, K.hop_inv_to, K.hop_inv_to - K.hop_inv_from, K.hop_cooldown, sa.dodge_invincible_until);
    assert!(K.hop_inv_to - K.hop_inv_from < sa.dodge_invincible_until, "the Hop's invincibility stays shorter than Sora's roll");
    assert!((K.perfect_counter_window - sa.guard_counter_window).abs() < 1e-5);
}

#[test]
fn goofy_runs_slower_than_sora_and_donald() {
    let speed = |chara: Chara, frames: usize| {
        let mut party = Party::new(PartyConfig::test_level99());
        let id = party.spawn(chara, Vec3::ZERO, 0.0);
        party.set_pad(id, Some(Tuning::USER));
        let world = TestWorld::floor();
        let mut d = Dummies(Vec::new(), Vec::new(), Vec::new());
        let (mut au, mut ef) = (kh_core::host::audio::NullAudio::default(), kh_core::host::effects::NullEffects::default());
        let mut sora = SoraView::default();
        sora.position = Vec3::new(-3000.0, 0.0, 0.0);
        let mut held = PadButtons::NONE;
        for _ in 0..frames {
            let pad = Buttons { held: PadButtons::NONE, pressed: PadButtons::NONE, released: held };
            held = PadButtons::NONE;
            party.set_pad_frame(id, PadFrame { stick: FWD, camera_yaw: 0.0, pad });
            let mut host = PartyHost { world: &world, targets: &mut d, audio: &mut au, effects: &mut ef };
            party.step(DT, &sora, &mut host);
        }
        horizontal(party.member(id).unwrap().cmc.velocity).length()
    };
    let (g_run, d_run) = (speed(Chara::Goofy, 45), speed(Chara::Donald, 45));
    let (g_sprint, d_sprint) = (speed(Chara::Goofy, 120), speed(Chara::Donald, 120));
    println!("RUN | Goofy {g_run:.0} / sprint {g_sprint:.0} cm/s; Donald {d_run:.0} / {d_sprint:.0}; Sora {} / {}", kh_core::SORA_PARAMS.speed_run, kh_core::SORA_PARAMS.speed_sprint);
    assert!((g_run - K.move_run).abs() < 15.0, "{g_run}");
    assert!((g_sprint - K.move_sprint).abs() < 15.0, "{g_sprint}");
    assert!(g_run < d_run - 80.0 && g_sprint < d_sprint - 80.0); // (DON-KIT) Donald sprints 900 now (doc 8): Goofy's 800 is still clearly slower
    assert!(g_run < kh_core::SORA_PARAMS.speed_run - 80.0 && g_sprint < kh_core::SORA_PARAMS.speed_sprint - 120.0);
}
// ------------------------------------------------------------------------------------------------ specials' tiers

fn special(rig: &mut Rig, def: ActionDef, tier: u8, target: TargetRef) {
    rig.mm().pad.as_mut().unwrap().x2.tier = tier;
    let id = rig.id;
    rig.party.member_mut(id).unwrap().command(MemberCommand::Action { def, target });
    rig.step(NONE, &[]);
    assert_eq!(rig.m().action.and_then(|a| a.def), Some(def));
    assert_eq!(rig.k().tier, tier.clamp(1, 3));
}

#[test]
fn tornado_tier_2_steers_and_pulls_tier_3_launches() {
    let mut rig = Rig::new(&[Vec3::new(400.0, 0.0, 0.0)]);
    special(&mut rig, ActionDef::GoofyTornado, 2, TargetRef::Enemy(TargetId(1)));
    rig.until(90, NONE, &[], |r| r.m().action.is_some_and(|a| a.sub == 1));
    let t0 = rig.m().action.unwrap().timer;
    rig.step(RIGHT, &[]);
    assert!(rig.m().action.unwrap().timer > t0, "one more loop");
    rig.run(30, RIGHT, &[]);
    assert!(rig.m().position().y > 100.0, "the stick steers the spin: {:?}", rig.m().position());
    // the pull: the spin's hits knock toward him
    rig.dummies.0[0].1 = rig.m().position() + Vec3::new(80.0, 0.0, 0.0);
    rig.until(120, NONE, &[], |r| !r.hits("Attack6_Tornado").is_empty());
    let h = rig.hits("Attack6_Tornado")[0];
    assert!(h.direction.x < -0.5, "inward: {}", h.direction);
    // tier 3: the launcher at the end
    let mut rig = Rig::new(&[Vec3::new(150.0, 0.0, 0.0)]);
    special(&mut rig, ActionDef::GoofyTornado, 3, TargetRef::Enemy(TargetId(1)));
    rig.until(900, NONE, &[], |r| r.logged("tornado_launch"));
    rig.idle(2);
    assert!(rig.dummies.1.iter().any(|h| h.damage_kind == Kind::SwirlUp), "a SWIRL_UP launcher");
}

#[test]
fn turbo_tier_2_hops_tier_3_lasts_longer_and_ends_with_a_blow() {
    let mut rig = Rig::new(&[]);
    special(&mut rig, ActionDef::GoofyTurbo, 2, TargetRef::None);
    rig.until(90, NONE, &[], |r| r.m().action.is_some_and(|a| a.sub == 1));
    rig.idle(3);
    rig.step(NONE, &[B]);
    rig.run(5, NONE, &[]);
    assert!(rig.logged("turbo_hop"));
    assert!(!rig.m().on_ground(), "hopping mid-ride");
    assert_eq!(rig.m().action.and_then(|a| a.def), Some(ActionDef::GoofyTurbo), "the ride goes on");
    let mut rig = Rig::new(&[]);
    special(&mut rig, ActionDef::GoofyTurbo, 3, TargetRef::None);
    rig.until(90, NONE, &[], |r| r.m().action.is_some_and(|a| a.sub == 1));
    rig.idle(1);
    let t = rig.m().action.unwrap().timer;
    assert!((t - K.turbo_ride * K.turbo_t3_time).abs() < 0.1, "the leader's ride x1.5 at tier 3 ({t})");
    rig.until(1200, NONE, &[], |r| r.logged("turbo_blow"));
}

#[test]
fn a_during_turbo_s_ride_links_into_big_bash() {
    let mut rig = Rig::new(&[]);
    special(&mut rig, ActionDef::GoofyTurbo, 1, TargetRef::None);
    rig.until(90, NONE, &[], |r| r.m().action.is_some_and(|a| a.sub == 1));
    rig.idle(10);
    rig.tap(A, NONE);
    assert!(rig.logged("turbo_bash"), "{:?}", rig.k().log);
    assert!(rig.logged("fin_big_bash"));
    assert_eq!(rig.m().anim.clip.name, "SGuardShieldBash2");
}
#[test]
fn turbo_tier_3_speeds_up_with_each_hit() {
    let mut rig = Rig::new(&[Vec3::new(600.0, 0.0, 0.0), Vec3::new(1300.0, 0.0, 0.0)]);
    special(&mut rig, ActionDef::GoofyTurbo, 3, TargetRef::None);
    rig.until(90, NONE, &[], |r| r.m().action.is_some_and(|a| a.sub == 1));
    rig.idle(2);
    let s0 = rig.m().action.unwrap().speed;
    rig.until(240, NONE, &[], |r| r.m().hits_this_action >= 1);
    rig.idle(2);
    let s1 = rig.m().action.unwrap().speed;
    assert!(s1 > s0 * 1.05, "{s0} -> {s1}");
}

#[test]
fn discus_tier_2_ricochets_at_half_power_tier_3_shockwave() {
    let ds = [Vec3::new(600.0, 0.0, 0.0), Vec3::new(700.0, 500.0, 0.0), Vec3::new(800.0, -500.0, 0.0), Vec3::new(1300.0, 0.0, 0.0)];
    let mut rig = Rig::new(&ds);
    rig.mm().status.goofy_kit.set_equipped(GA::Ricochet, false);
    special(&mut rig, ActionDef::GoofyDiscus, 2, TargetRef::Enemy(TargetId(1)));
    rig.until(400, NONE, &[], |r| r.hits("Attack8_Discus").len() >= 3);
    let hits = rig.hits("Attack8_Discus");
    let targets: Vec<TargetId> = hits.iter().map(|h| h.target).collect();
    assert_eq!(targets[0], TargetId(1));
    assert_ne!(targets[1], targets[0]);
    assert!(hits[1].damage < hits[0].damage * 0.75, "a ricochet at x0.5: {} vs {}", hits[1].damage, hits[0].damage);
    rig.until(600, NONE, &[], |r| r.state() != NpcStateId::Action);
    assert!(rig.logged("bounce"));
    // tier 3 + Ricochet: 5 bounces; the catch's shockwave
    let mut rig = Rig::new(&[Vec3::new(600.0, 0.0, 0.0)]);
    special(&mut rig, ActionDef::GoofyDiscus, 3, TargetRef::Enemy(TargetId(1)));
    assert_eq!(rig.k().bounces, K.discus_t3_bounces as u8 + K.ricochet_bounces as u8);
    rig.until(900, NONE, &[], |r| r.logged("discus_wave"));
}

#[test]
fn discus_barrage_locks_up_to_8_and_bounces_lock_to_lock_for_no_mp() {
    let ds = [Vec3::new(800.0, -150.0, 0.0), Vec3::new(900.0, 150.0, 0.0), Vec3::new(1100.0, 0.0, 0.0)];
    let mut rig = Rig::new(&ds);
    let mp = rig.m().status.mp;
    rig.run(25, NONE, &[RB]);
    assert_eq!(rig.state(), NpcStateId::Pad(AIM), "RB held 0.27 s: the aim");
    assert_eq!(rig.m().pad.as_ref().unwrap().lock, TargetId::NONE, "the RB press did not lock (a hold is no tap)");
    rig.run(60, NONE, &[RB]);
    let locks = rig.party.goofy_locks(rig.id).unwrap().to_vec();
    assert_eq!(locks.len(), K.barrage_max_locks as usize, "{locks:?}");
    assert!(locks.contains(&TargetId(1)) && locks.contains(&TargetId(2)) && locks.contains(&TargetId(3)));
    assert!(rig.k().focus < K.barrage_focus_max);
    rig.step(NONE, &[]);
    rig.idle(1);
    assert_eq!(rig.m().action.and_then(|a| a.def), Some(ActionDef::GoofyDiscus));
    assert_eq!(rig.m().status.mp, mp, "the barrage costs no MP");
    rig.until(900, NONE, &[], |r| r.hits("Attack8_Discus").len() >= 4);
    let h = rig.hits("Attack8_Discus");
    assert!(h[1].damage <= h[0].damage + 1e-3, "bounces at x{}", K.barrage_mult);
    // a short RB tap still toggles the lock
    let mut rig = Rig::new(&ds);
    rig.step(NONE, &[RB]);
    rig.step(NONE, &[]);
    assert_ne!(rig.m().pad.as_ref().unwrap().lock, TargetId::NONE, "tap: lock on");
}

// ------------------------------------------------------------------------------------------------ mode, copter, abilities

#[test]
fn tornado_copter_with_sora_s_glide() {
    for unlocked in [false, true] {
        let mut rig = Rig::new(&[]);
        if !unlocked {
            rig.mm().status.goofy_kit.take(GA::TornadoCopter);
        }
        rig.step(NONE, &[B]);
        for _ in 0..90 {
            rig.step(FWD, &[B]);
            if rig.state() == NpcStateId::Pad(COPTER) {
                break;
            }
        }
        assert_eq!(rig.state() == NpcStateId::Pad(COPTER), unlocked, "copter unlocked {unlocked}");
        if unlocked {
            assert!(rig.m().anim.clip.notifies.iter().all(|n| !matches!(n.data, crate::anim::ClipNotifyData::Attack { .. })), "contact off");
            rig.run(10, FWD, &[B]);
            let v = rig.m().cmc.velocity;
            assert!((horizontal(v).length() - K.copter_speed).abs() < 20.0 && (v.z + K.copter_sink).abs() < 10.0, "copter {v}");
            rig.step(FWD, &[]);
            rig.idle(1);
            assert_ne!(rig.state(), NpcStateId::Pad(COPTER), "let go of B");
        }
    }
}

#[test]
fn stalwart_keeps_him_swinging_through_small_hits() {
    for stalwart in [false, true] {
        let mut rig = Rig::new(&[Vec3::new(400.0, 0.0, 0.0)]);
        rig.mm().status.goofy_kit.set_equipped(GA::Stalwart, stalwart);
        rig.tap(A, NONE);
        rig.idle(5);
        let hp = rig.m().status.hp;
        rig.hit_from(Vec3::new(-200.0, 0.0, 0.0), 10.0, Kind::Small);
        assert!(rig.m().status.hp < hp, "the damage is taken");
        assert_eq!(rig.state() == NpcStateId::Pad(super::super::goofy_combo::COMBO), stalwart, "stalwart {stalwart}");
    }
}

#[test]
fn the_kit_abilities_ap_levels_and_sora_s_side() {
    let mut s = crate::status::FriendStatus::new(Chara::Goofy, 0);
    assert_eq!(s.goofy_kit.ap_in_use(), 0);
    s.level = 20;
    let got = s.kit_goofy_sync_level();
    assert_eq!(got, vec![GA::ComboPlus, GA::IronWall, GA::AirComboPlus, GA::OverHere, GA::ShieldQuake, GA::PaybackBash], "learned in level order");
    // (GOOFY-GK) every ability has an in-game name and description
    for k in GA::ALL {
        assert!(!k.label().is_empty() && k.description().ends_with('.'), "{k:?}");
    }
    assert!(got.iter().all(|k| s.goofy_kit.has(*k) || s.ap_in_use() + k.ap() > s.max_ap()));
    let mut sora = kh_core::status::AbilitySet::EMPTY;
    sora.set(kh_core::enums::ETresAbilityKind::from_game_name("GLIDE").unwrap(), 1, 1);
    sora.set(kh_core::enums::ETresAbilityKind::from_game_name("FUSION_SPIN").unwrap(), 1, 0);
    let got = s.kit_goofy_sync_sora(&sora);
    assert_eq!(got, vec![GA::FusionSpin, GA::TornadoCopter]);
    assert!(s.goofy_kit.owned(GA::TornadoCopter) && !s.goofy_kit.owned(GA::FusionRocket));
    assert_eq!(GA::IronWall.ap(), 3);
    assert_eq!((GA::OverHere.ap(), GA::ShieldCharge.ap(), GA::Stalwart.ap(), GA::TornadoCopter.ap()), (2, 4, 5, 0));
    // the save record round trip
    let rec = s.goofy_kit.record();
    let mut k = crate::pad::goofy_kit::GoofyKit::default();
    for (n, o, e) in &rec {
        assert!(k.apply_record(n, *o, *e));
    }
    assert_eq!(k, s.goofy_kit);
    assert_eq!(GA::from_name("kit:TORNADO_COPTER"), Some(GA::TornadoCopter));
}

#[test]
fn donald_is_untouched_by_the_kit() {
    let mut party = Party::new(PartyConfig::test_level99());
    let id = party.spawn(Chara::Donald, Vec3::ZERO, 0.0);
    party.set_pad(id, Some(Tuning::USER));
    let m = party.member(id).unwrap();
    assert_eq!(m.status.goofy_kit.ap_in_use(), 0);
    assert!(!invincible(m));
    assert!(party.goofy_taunt(id).is_none() && party.pad_mesh_pitch(id).is_none());
    let _ = (AIR_ATTACK, JUMP, RUN);
}

// ------------------------------------------------------------------------------------------------ X3G-b

#[test]
fn x_while_sprinting_is_the_hop_not_the_superslide() {
    let mut rig = Rig::new(&[]);
    rig.run(90, FWD, &[]);
    assert!(rig.m().pad.as_ref().unwrap().x2.sprinting, "sprinting");
    rig.step(FWD, &[X]);
    assert_ne!(rig.state(), NpcStateId::SuperSlide, "the user: 'remove his shield surf when you press X while running'");
    assert_eq!(rig.state(), NpcStateId::Pad(HOP));
    assert!(!rig.logged("superslide"));
}
#[test]
fn a_out_of_a_slide_starts_the_combo() {
    let mut rig = Rig::new(&[]);
    rig.run(30, FWD, &[]);
    crate::traverse::begin_super_slide_at(rig.mm(), 1);
    rig.run(20, FWD, &[]);
    rig.step(FWD, &[A]);
    rig.step(FWD, &[]);
    assert!(rig.logged("slide_attack"));
    assert_eq!(rig.state(), NpcStateId::Pad(super::super::goofy_combo::COMBO));
}
#[test]
fn aerial_recovery_out_of_a_blow_with_his_own_clip_invincible_then_the_air_counter() {
    let mut rig = Rig::new(&[Vec3::new(200.0, 0.0, 0.0)]);
    // knocked away while in the air (a jump, then a BLOW)
    rig.step(NONE, &[B]);
    rig.run(14, NONE, &[B]);
    let r = rig.hit_from(rig.m().position() + Vec3::X * 200.0, 10.0, Kind::Blow);
    assert!(r.accepted);
    rig.step(NONE, &[]);
    assert_eq!(rig.state(), NpcStateId::Damage, "clip {}", rig.m().anim.clip.name);
    rig.until(30, NONE, &[], |r| !r.m().on_ground() && r.m().state_time >= K.recover_delay);
    rig.step(NONE, &[B]);
    assert_eq!(rig.state(), NpcStateId::Pad(RECOVER), "B while blown back");
    assert_eq!(rig.m().anim.clip.name, K.recover_clip, "his own clip");
    assert!(invincible(rig.m()));
    let z = rig.m().position().z;
    rig.run(10, NONE, &[]);
    assert!((rig.m().position().z - z).abs() < 2.0, "held in the air");
    let r = rig.hit_from(rig.m().position() + Vec3::X * 200.0, 10.0, Kind::Small);
    assert!(!r.accepted, "invincible through the recovery");
    rig.until(40, NONE, &[], |r| r.state() == NpcStateId::Pad(JUMP));
    // A in the counter window: the air counter
    rig.step(NONE, &[A]);
    rig.step(NONE, &[]);
    assert!(rig.logged("air_bash"), "{:?}", rig.k().log);
    // without Sora's AIR_RECOVERY: no recovery
    let mut rig = Rig::new(&[]);
    rig.mm().pad.as_mut().unwrap().goofy.air_recovery = false;
    rig.step(NONE, &[B]);
    rig.run(14, NONE, &[B]);
    rig.hit_from(rig.m().position() + Vec3::X * 200.0, 10.0, Kind::Blow);
    rig.run(15, NONE, &[]);
    rig.step(NONE, &[B]);
    assert_ne!(rig.state(), NpcStateId::Pad(RECOVER));
}

#[test]
fn y_never_starts_shield_guardian_any_more() {
    let mut rig = Rig::new(&[]);
    rig.mm().pad.as_mut().unwrap().goofy.mode_gauge = K.mode_gauge;
    rig.tap(Y, NONE);
    assert!(rig.k().mode_left <= 0.0 && rig.party.goofy_mode_actor(rig.id).is_none());
    assert!(rig.logged("bombardier"));
}


#[test]
fn y_inside_a_running_team_attack_stays_the_team_attack_s() {
    // (GOOFY-GK) Trinity Guard's Finish is Y (Sora's FR state): Goofy's Bombardier must not take it nor buzz
    let mut rig = Rig::new(&[]);
    rig.mm().enter(NpcStateId::Fr(4));
    rig.step(NONE, &[Y]);
    let pad = rig.m().pad.as_ref().unwrap();
    assert!(!pad.x2.y_taken && !pad.goofy.bomb_request && !pad.menu.events.contains(&kh_core::command::MenuEvent::Refused));
    assert!(!rig.logged("bombardier"));
}

#[test]
fn pots_and_npcs_are_never_chosen_but_still_break_on_contact() {
    // (GOOFY-GK, AUD's rule) target 1 an enemy, 2 a pot right beside it (nearer than enemy 3), 3 a second enemy
    let mut rig = Rig::new(&[Vec3::new(600.0, 0.0, 0.0), Vec3::new(700.0, 150.0, 0.0), Vec3::new(1100.0, -500.0, 0.0)]);
    rig.dummies.2 = vec![TargetId(2)];
    rig.mm().status.goofy_kit.set_equipped(GA::Ricochet, false);
    special(&mut rig, ActionDef::GoofyDiscus, 2, TargetRef::Enemy(TargetId(1)));
    rig.until(500, NONE, &[], |r| r.hits("Attack8_Discus").len() >= 2);
    let order: Vec<TargetId> = rig.hits("Attack8_Discus").iter().map(|h| h.target).collect();
    assert_eq!(order[1], TargetId(3), "the ricochet skips the pot for the enemy: {order:?}");
    // the crowd: an enemy and two pots around him is no crowd (no Fusion Spin)
    let mut rig = Rig::new(&[Vec3::new(150.0, 0.0, 0.0), Vec3::new(120.0, 120.0, 0.0), Vec3::new(-120.0, 80.0, 0.0)]);
    rig.dummies.2 = vec![TargetId(2), TargetId(3)];
    rig.wear_only(&[GA::FusionSpin]);
    let host = PartyHost { world: &rig.world, targets: &mut rig.dummies, audio: &mut rig.audio, effects: &mut rig.effects };
    assert!(matches!(super::super::goofy_combo::choose(rig.party.member(rig.id).unwrap(), &host), super::super::goofy_combo::Fin::Step(_)), "pots do not make a crowd");
    // a swing still breaks a pot it touches
    let mut rig = Rig::new(&[Vec3::new(200.0, 0.0, 0.0)]);
    rig.dummies.2 = vec![TargetId(1)];
    rig.tap(A, NONE);
    rig.until(90, NONE, &[], |r| !r.hits("Attack1_Push").is_empty());
}
