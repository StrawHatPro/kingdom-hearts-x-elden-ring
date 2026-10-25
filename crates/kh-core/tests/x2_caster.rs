//! (party X2) Sora's spells cast by another body (kh-core magic/caster.rs): the actors, the damage with the caster's
//! stats, the Cure's heals, the formchange gauge and the grand-magic roll, the handles out of Sora's range.

use kh_core::enums::ETresAbilityKind;
use kh_core::host::party::{handle_member, PartyMember};
use kh_core::host::targets::Ally;
use kh_core::host::{EffectHandle, EffectSpawn, Effects, HitRecord, HitResult, HurtShape, TargetDefense, TargetFlags, TargetId, TargetInfo, Targets};
use kh_core::magic::caster::{CasterStats, Shot, SpellCaster};
use kh_core::magic::spells::cmd;
use kh_core::status::AbilitySet;
use kh_core::test_world::TestWorld;
use kh_core::{Attach, Vec3};

const DT: f32 = 1.0 / 60.0;

struct Dummy(Vec3, Vec<HitRecord>);

impl Targets for Dummy {
    fn enumerate(&self, out: &mut Vec<TargetInfo>) {
        out.push(self.info(TargetId(1)).unwrap());
    }
    fn info(&self, id: TargetId) -> Option<TargetInfo> {
        (id == TargetId(1)).then(|| TargetInfo { id, position: self.0, center: self.0 + Vec3::Z * 90.0, velocity: Vec3::ZERO, yaw: 180.0, radius: 50.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 })
    }
    fn hurt_shapes(&self, id: TargetId, out: &mut Vec<HurtShape>) {
        if id == TargetId(1) {
            out.push(HurtShape { a: self.0 + Vec3::Z * 30.0, b: self.0 + Vec3::Z * 150.0, radius: 60.0, part: 0 });
        }
    }
    fn apply_hit(&mut self, hit: &HitRecord) -> HitResult {
        self.1.push(*hit);
        HitResult { accepted: true, damage_dealt: hit.damage.max(1.0), hp_after: 1.0e6, killed: false, guarded: false }
    }
    fn defense(&self, _id: TargetId) -> Option<TargetDefense> {
        Some(TargetDefense { defense: 10, max_hp: 1_000_000, hp_percent: 100.0, enemy_kind: true, damage_min: 0, damage_max: 0, resist: [0; 8], bio_type: 0 })
    }
}

#[derive(Default)]
struct Fx(Vec<(EffectHandle, EffectSpawn)>);

impl Effects for Fx {
    fn spawn(&mut self, h: EffectHandle, s: &EffectSpawn) {
        self.0.push((h, *s));
    }
    fn stop(&mut self, _h: EffectHandle, _i: bool) {}
}

fn caster(magic: i32, grand: &AbilitySet) -> SpellCaster {
    let mut c = SpellCaster::new(PartyMember::DONALD);
    c.set_stats(CasterStats { strength: 63, magic, ..CasterStats::default() }, &AbilitySet::EMPTY, grand);
    c
}

fn shot(c: u8) -> Shot {
    Shot { cmd: c, origin: Vec3::ZERO, yaw: 0.0, muzzle: Some(Vec3::new(40.0, 0.0, 100.0)), target: TargetId(1), finish: false, grand: false, full_mp: false, combo: [1, 0, 0, 0] }
}

fn run(c: &mut SpellCaster, world: &TestWorld, t: &mut Dummy, fx: &mut Fx, seconds: f32) -> Vec<u8> {
    let mut audio = kh_core::host::audio::NullAudio;
    let mut offered = Vec::new();
    for _ in 0..(seconds / DT) as usize {
        offered.extend(c.step(world, t, &mut audio, fx, &[], DT).offered);
    }
    offered
}

#[test]
fn firaga_hits_with_the_casters_magic_and_member_handles() {
    let world = TestWorld::floor();
    let mut audio = kh_core::host::audio::NullAudio;
    let (mut weak, mut strong) = (0.0, 0.0);
    for (magic, out) in [(10, &mut weak), (67, &mut strong)] {
        let mut t = Dummy(Vec3::new(600.0, 0.0, 0.0), Vec::new());
        let mut fx = Fx::default();
        let mut c = caster(magic, &AbilitySet::EMPTY);
        let ids = c.shoot(&world, &mut t, &mut audio, &mut fx, &shot(cmd::FIRAGA));
        assert_eq!(ids.len(), 1);
        assert_eq!(c.projectiles().next().map(|v| v.class), Some("TresProj_MagFigaNml_C"));
        run(&mut c, &world, &mut t, &mut fx, 2.0);
        let hit = t.1.iter().find(|h| h.attack_name.starts_with("MAG_FI")).expect("Firaga hits");
        assert_eq!(hit.flags & 0xFF, cmd::FIRAGA as u32);
        *out = t.1.iter().map(|h| h.damage).sum::<f32>();
        // every handle is Donald's (the upper half of his range), never Sora's
        assert!(fx.0.iter().all(|(h, _)| handle_member(h.0) == Some(PartyMember::DONALD)));
    }
    assert!(strong > weak, "the caster's Magic: {strong} > {weak}");
}

#[test]
fn the_water_barrier_and_the_cure_attach_to_the_member() {
    let world = TestWorld::floor();
    let mut audio = kh_core::host::audio::NullAudio;
    let mut t = Dummy(Vec3::new(120.0, 0.0, 0.0), Vec::new());
    let mut fx = Fx::default();
    let mut c = caster(67, &AbilitySet::EMPTY);
    c.shoot(&world, &mut t, &mut audio, &mut fx, &shot(cmd::WATAGA));
    assert!(fx.0.iter().any(|(_, s)| matches!(s.at, Attach::Party { member: PartyMember::DONALD, .. })), "the barrier body on Donald");
    assert!(!fx.0.iter().any(|(_, s)| matches!(s.at, Attach::Sora { .. })));
    // Curaga: the caster's heal and the ally's in range (Sora's DoCure numbers)
    let mut fx = Fx::default();
    let mut c = caster(67, &AbilitySet::EMPTY);
    c.shoot(&world, &mut t, &mut audio, &mut fx, &Shot { target: TargetId::NONE, ..shot(cmd::CURAGA) });
    let allies = [Ally { id: TargetId(77), position: Vec3::new(100.0, 0.0, 0.0), dead: false }, Ally { id: TargetId(78), position: Vec3::new(5000.0, 0.0, 0.0), dead: false }];
    let mut heals = Vec::new();
    for _ in 0..10 {
        heals.extend(c.step(&world, &mut t, &mut audio, &mut fx, &allies, DT).heals);
    }
    assert!(heals.iter().any(|h| h.ally.is_none() && h.amount > 67), "the caster: {heals:?}");
    assert!(heals.iter().any(|h| h.ally == Some(TargetId(77))) && !heals.iter().any(|h| h.ally == Some(TargetId(78))), "the ally in range only: {heals:?}");
    assert!(fx.0.iter().any(|(_, s)| s.name.starts_with("vm_cur_hed") && matches!(s.at, Attach::Party { .. })), "Sora's heal effect on Donald");
}

#[test]
fn his_magic_hits_fill_the_gauge_and_roll_the_grand_magic() {
    let world = TestWorld::floor();
    let mut audio = kh_core::host::audio::NullAudio;
    let mut t = Dummy(Vec3::new(500.0, 0.0, 0.0), Vec::new());
    let mut fx = Fx::default();
    // Sora's Grand Magic ability (x1.3) shared
    let mut grand = AbilitySet::EMPTY;
    grand.set(ETresAbilityKind::GrandMagic, 1, 1);
    let mut c = caster(67, &grand);
    let mut offered = Vec::new();
    let mut casts = 0;
    while offered.is_empty() && casts < 400 {
        c.shoot(&world, &mut t, &mut audio, &mut fx, &shot(cmd::FIRAGA));
        casts += 1;
        offered = run(&mut c, &world, &mut t, &mut fx, 0.8);
    }
    assert!(!offered.is_empty(), "an offer after {casts} casts (gauge {:?}, tallies {:?})", c.gauge(), c.element_points());
    assert!(offered.iter().all(|k| (110..=112).contains(k)), "Fire's -za (S_FIRA .. S_FIRAGAN): {offered:?}");
    let o = c.offers()[0];
    assert!(o.delay > 0.0 && o.total >= 20.0, "the queue's delay, 20 s life: {o:?}");
    assert!(c.ready_offer().is_none());
    run(&mut c, &world, &mut t, &mut fx, 1.0);
    let o = c.ready_offer().expect("ready after the delay");
    assert_eq!(c.take_offer(o.serial), Some(o.cmd));
    assert_eq!(c.gauge().0, 0.0, "the gauge is emptied by the grand magic");
    // the -za cast: Sora's `_S` rows
    let before = t.1.len();
    c.shoot(&world, &mut t, &mut audio, &mut fx, &Shot { grand: true, ..shot(o.cmd) });
    run(&mut c, &world, &mut t, &mut fx, 3.0);
    assert!(t.1[before..].iter().any(|h| h.attack_name.starts_with("MAG_FI")), "{:?}", t.1[before..].iter().map(|h| h.attack_name).collect::<Vec<_>>());
}
