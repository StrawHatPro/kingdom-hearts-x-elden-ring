//! (VFX-RING) THE THUNDER ULTIMATE'S TRACE: Judgment Bolt, cast through `cast::begin` at full-power attunement, spawns its
//! storm: one softened Thundaga strike (`fx::JUDGMENT_SOFT`, DON-SPELLB make_jb.js) per strike of its sheet row, every one on
//! an enemy inside the field, round robin, `judgment_gap` apart, each with its Thunder hit. Its payload arm in
//! spell_payload.rs is empty ON PURPOSE: its look is the storm (magic_fx `FieldRule::Storm`), and the user's rule of
//! 2026-10-22 ("Judgement Bolt should be less seizure inducing ... don't change anything else except how violently it
//! flashes") keeps KH3's Thundagan finisher (vm_thc_rng3, with its scene-darkening `kuro_filter` and flash `fls0`) out of
//! it, where the other ultimates open with their vm_*c_rng3. The test pins both: the storm plays, vm_thc_rng3 does not.
use super::cast::{self, CastTarget};
use super::combat::{CastPlan, Refusal};
use super::element::Element;
use super::learn::DonLearned;
use super::magic_fx::{self, fx};
use super::spells::SpellRef;
use super::tune::data;
use super::attune::Slot;
use crate::host::PartyHost;
use crate::ids::{Chara, MemberId};
use crate::{Party, PartyConfig, SoraView};
use glam::Vec3;
use kh_core::host::{Attach, EffectHandle, EffectSpawn, HitRecord, HitResult, HurtShape, TargetDefense, TargetFlags, TargetId, TargetInfo, Targets};
use kh_core::test_world::TestWorld;

const DT: f32 = 1.0 / 60.0;

struct Dummies {
    list: Vec<(TargetId, Vec3)>,
    hits: Vec<HitRecord>,
}

impl Dummies {
    fn info_of(id: TargetId, pos: Vec3) -> TargetInfo {
        let flags = TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY;
        TargetInfo { id, position: pos, center: pos + Vec3::Z * 90.0, velocity: Vec3::ZERO, yaw: 180.0, radius: 50.0, half_height: 90.0, flags: TargetFlags(flags), hp: 1.0e6, hp_max: 1.0e6 }
    }
}

impl Targets for Dummies {
    fn enumerate(&self, out: &mut Vec<TargetInfo>) {
        out.extend(self.list.iter().map(|(id, p)| Dummies::info_of(*id, *p)));
    }
    fn info(&self, id: TargetId) -> Option<TargetInfo> {
        self.list.iter().find(|(i, _)| *i == id).map(|(i, p)| Dummies::info_of(*i, *p))
    }
    fn hurt_shapes(&self, id: TargetId, out: &mut Vec<HurtShape>) {
        if let Some(i) = self.info(id) {
            out.push(HurtShape { a: i.position + Vec3::Z * 30.0, b: i.position + Vec3::Z * 150.0, radius: 60.0, part: 0 });
        }
    }
    fn apply_hit(&mut self, hit: &HitRecord) -> HitResult {
        self.hits.push(*hit);
        HitResult { accepted: true, damage_dealt: hit.damage, hp_after: 1.0e6, killed: false, guarded: false }
    }
    fn defense(&self, _id: TargetId) -> Option<TargetDefense> {
        Some(TargetDefense { defense: 10, max_hp: 1_000_000, hp_percent: 100.0, enemy_kind: true, damage_min: 0, damage_max: 0, resist: [0; 8], bio_type: 0 })
    }
    fn size_class(&self, _id: TargetId) -> u8 {
        1
    }
    fn hit_bone(&self, _id: TargetId, _at: Vec3) -> Option<kh_core::host::TargetBone> {
        None
    }
    fn bone_pose(&self, _id: TargetId, _bone: kh_core::host::TargetBone) -> Option<kh_core::host::BonePose> {
        None
    }
}

/// Every spawn: (step, name, where).
#[derive(Default)]
struct Rec(Vec<(usize, &'static str, Attach)>, usize);

impl kh_core::host::Effects for Rec {
    fn spawn(&mut self, _handle: EffectHandle, spawn: &EffectSpawn) {
        self.0.push((self.1, spawn.name, spawn.at));
    }
    fn stop(&mut self, _handle: EffectHandle, _immediate: bool) {}
}

struct Rig {
    world: TestWorld,
    dummies: Dummies,
    audio: kh_core::host::audio::NullAudio,
    effects: Rec,
    party: Party,
    sora: SoraView,
    don: MemberId,
}

impl Rig {
    fn new(enemies: &[Vec3]) -> Rig {
        let mut party = Party::new(PartyConfig::test_level99());
        let don = party.spawn(Chara::Donald, Vec3::ZERO, 0.0);
        party.set_pad(don, Some(kh_core::Tuning::USER));
        {
            let s = party.status_mut(don).expect("Donald");
            let st = s.donald.as_mut().expect("his state");
            st.learned = DonLearned::all();
            st.abilities = super::abilities::DonAbilities::default();
            s.mp = s.max_mp();
        }
        let list = enemies.iter().enumerate().map(|(i, p)| (TargetId(i as u32 + 1), *p)).collect();
        let sora = SoraView { position: Vec3::new(-3000.0, 0.0, 0.0), hp_max: 100, hp: 100, ..SoraView::default() };
        let mut rig = Rig { world: TestWorld::floor(), dummies: Dummies { list, hits: Vec::new() }, audio: kh_core::host::audio::NullAudio, effects: Rec::default(), party, sora, don };
        rig.secs(2.0 * DT);
        rig
    }

    fn secs(&mut self, s: f32) {
        for _ in 0..(s / DT).round() as usize {
            self.effects.1 += 1;
            let mut host = PartyHost { world: &self.world, targets: &mut self.dummies, audio: &mut self.audio, effects: &mut self.effects };
            self.party.step(DT, &self.sora, &mut host);
        }
    }

    /// Full-power attunement: attuned to `e` with Resonance 3, a fresh period (its ultimate unused).
    fn attune_full(&mut self, e: Element) {
        let st = self.party.status_mut(self.don).and_then(|s| s.donald.as_mut()).expect("his state");
        let period = st.core.attune.slots[0].map_or(1, |s| s.period + 1);
        st.core.attune.slots[1] = st.core.attune.slots[0];
        st.core.attune.slots[0] = Some(Slot { element: e, res: 3.0, period, ult_used: false, full_at: Some(st.core.clock) });
    }

    fn plan(&self, spell: SpellRef) -> Result<CastPlan, Refusal> {
        let s = self.party.status(self.don).expect("Donald");
        let st = s.donald.as_ref().expect("his state");
        st.core.check_cast(spell, &st.learned, s.mp, s.max_mp(), st.abilities.mods().dual, &data())
    }

    fn begin(&mut self, spell: SpellRef, target: CastTarget) -> bool {
        let plan = self.plan(spell).unwrap_or_else(|r| panic!("{spell:?} refused: {r:?}"));
        let mut host = PartyHost { world: &self.world, targets: &mut self.dummies, audio: &mut self.audio, effects: &mut self.effects };
        let p = self.party.member_mut(self.don).expect("Donald");
        cast::begin(p, &plan, target, &mut host)
    }
}

#[test]
fn judgment_bolt_spawns_its_storm_of_strikes_on_the_enemies() {
    let spell = SpellRef::Ultimate(Element::Thunder);
    let d = data();
    let row = spell.row(&d).expect("Judgment Bolt's row");
    let gap = magic_fx::magic().judgment_gap;
    assert!(row.count > 0 && gap > 0.0, "count {} gap {gap}", row.count);
    // three enemies well inside the field round Donald
    let foes = [Vec3::new(500.0, 0.0, 0.0), Vec3::new(400.0, 300.0, 0.0), Vec3::new(350.0, -350.0, 0.0)];
    assert!(foes.iter().all(|f| f.length() < row.radius * 0.8), "radius {}", row.radius);
    let mut rig = Rig::new(&foes);
    rig.attune_full(Element::Thunder);
    assert!(rig.begin(spell, CastTarget::Enemy(TargetId(1))), "Judgment Bolt did not begin");
    let shot = rig.party.member(rig.don).and_then(|p| p.don_cast.run).map_or(row.cast, |r| if r.shot_at > 60.0 { row.cast } else { r.shot_at });
    let cast_step = rig.effects.1;
    rig.secs(shot + row.count as f32 * gap + 1.5);

    let strikes: Vec<_> = rig.effects.0.iter().filter(|e| e.0 > cast_step && fx::JUDGMENT_SOFT.contains(&e.1)).collect();
    assert_eq!(strikes.len(), row.count as usize, "one strike per count: {:?}", rig.effects.0.iter().map(|e| e.1).collect::<Vec<_>>());
    // every strike on an enemy (round robin over the three), and the flash pattern: the mild-dim strike on every 3rd
    for (k, s) in strikes.iter().enumerate() {
        let Attach::World(at) = s.2 else { panic!("strike {k} not in the world: {:?}", s.2) };
        assert!(foes.iter().any(|f| f.distance(at) < 1.0), "strike {k} at {at:?} is on no enemy");
        assert_eq!(s.1, fx::JUDGMENT_SOFT[usize::from((k + 1) % 3 != 1)], "strike {k}");
    }
    let hit: std::collections::BTreeSet<u32> = rig.dummies.hits.iter().map(|h| h.target.0).collect();
    assert_eq!(hit.len(), 3, "each enemy struck: {hit:?}");
    // the strikes are spaced by the storm's tick
    let steps: Vec<usize> = strikes.iter().map(|s| s.0).collect();
    for w in steps.windows(2) {
        assert!(((w[1] - w[0]) as f32 * DT - gap).abs() < 2.5 * DT, "strike spacing {:?} vs gap {gap}", steps);
    }
    // the user's rule: no Thundagan finisher (scene darkening + flash) added to it
    assert!(!rig.effects.0.iter().any(|e| e.1 == "vm_thc_rng3"), "vm_thc_rng3 must stay out of Judgment Bolt");
}

#[test]
fn the_other_ultimates_open_with_their_finisher_and_judgment_bolt_does_not() {
    use super::magic_fx::MagicFx;
    for (e, finisher) in [(Element::Fire, Some("vm_fic_rng3")), (Element::Blizzard, Some("vm_blc_rng3")), (Element::Water, Some("vm_wac_rng3")), (Element::Aero, Some("vm_aic_rng3")), (Element::Thunder, None)] {
        let mut fxw = MagicFx::default();
        super::spell_payload::launch(&mut fxw, MemberId(0), SpellRef::Ultimate(e), Vec3::new(600.0, 0.0, 0.0), Vec3::ZERO, 0.0, 1200.0);
        let names: Vec<&str> = fxw.timed.iter().map(|t| t.name).collect();
        match finisher {
            Some(f) => assert!(names.contains(&f), "{e:?}: {names:?}"),
            None => assert!(names.is_empty(), "{e:?}: its look is the storm only: {names:?}"),
        }
    }
}
