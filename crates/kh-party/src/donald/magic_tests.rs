//! (DON-MAGIC) Donald's casts, one or more tests per spell row and rule (documentation/DONALD_OVERHAUL.md 4.3, 4.5, 5.3,
//! 6.1 - 6.5; build/sora_tmp/party/DON_PLAN.md 6.3 DON-MAGIC), and the pack harness (every effect a spell spawns is in a
//! pack).

use super::abilities::DonAbility;
use super::attune::Slot;
use super::cast::{self, CastTarget, SORA_MEMBER};
use super::combat::{CastPlan, Refusal};
use super::element::Element;
use super::learn::DonLearned;
use super::magic_fx::{self, fx, FxLog, DON_HIT_FLAG};
use super::react::Reaction;
use super::spells::{hybrid_pairs, ArcaneLine, SpellRef};
use super::status::{Apply, Size};
use super::tune::data;
use crate::host::PartyHost;
use crate::ids::{Chara, MemberId};
use crate::{Party, PartyConfig, SoraView};
use glam::Vec3;
use kh_core::enums::ETresDamageKind as Kind;
use kh_core::host::{Attach, EffectHandle, EffectSpawn, HitRecord, HitResult, HurtShape, TargetDefense, TargetFlags, TargetId, TargetInfo, Targets};
use kh_core::test_world::TestWorld;

const DT: f32 = 1.0 / 60.0;

#[derive(Clone, Copy, Debug)]
struct Dummy {
    id: TargetId,
    pos: Vec3,
    size: u8,
    hp: f32,
    hp_max: f32,
    resist: [i32; 8],
}

#[derive(Default)]
struct Dummies {
    list: Vec<Dummy>,
    hits: Vec<HitRecord>,
}

impl Dummies {
    fn info_of(&self, d: &Dummy) -> TargetInfo {
        let flags = TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY;
        TargetInfo { id: d.id, position: d.pos, center: d.pos + Vec3::Z * 90.0, velocity: Vec3::ZERO, yaw: 180.0, radius: 50.0, half_height: 90.0, flags: TargetFlags(flags), hp: d.hp, hp_max: d.hp_max }
    }

    fn hits_on(&self, id: u32) -> Vec<HitRecord> {
        self.hits.iter().filter(|h| h.target.0 == id).copied().collect()
    }

    fn damage_on(&self, id: u32) -> f32 {
        self.hits_on(id).iter().map(|h| h.damage).sum()
    }
}

impl Targets for Dummies {
    fn enumerate(&self, out: &mut Vec<TargetInfo>) {
        for d in &self.list {
            out.push(self.info_of(d));
        }
    }
    fn info(&self, id: TargetId) -> Option<TargetInfo> {
        self.list.iter().find(|d| d.id == id).map(|d| self.info_of(d))
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
    fn defense(&self, id: TargetId) -> Option<TargetDefense> {
        let r = self.list.iter().find(|d| d.id == id).map_or([0; 8], |d| d.resist);
        Some(TargetDefense { defense: 10, max_hp: 1_000_000, hp_percent: 100.0, enemy_kind: true, damage_min: 0, damage_max: 0, resist: r, bio_type: 0 })
    }
    fn size_class(&self, id: TargetId) -> u8 {
        self.list.iter().find(|d| d.id == id).map_or(0, |d| d.size)
    }
}

#[derive(Default)]
struct RecEffects {
    spawned: Vec<(u32, &'static str, Attach, bool)>,
    stopped: Vec<u32>,
    tinted: Vec<(u32, kh_core::host::Tint)>,
    placed: Vec<(u32, Vec3)>,
}

impl kh_core::host::Effects for RecEffects {
    fn spawn(&mut self, handle: EffectHandle, spawn: &EffectSpawn) {
        self.spawned.push((handle.0, spawn.name, spawn.at, spawn.persistent));
    }
    fn stop(&mut self, handle: EffectHandle, _immediate: bool) {
        self.stopped.push(handle.0);
    }
    fn tint(&mut self, handle: EffectHandle, tint: kh_core::host::Tint) {
        self.tinted.push((handle.0, tint));
    }
    fn place(&mut self, handle: EffectHandle, at: Vec3) {
        self.placed.push((handle.0, at));
    }
}

#[derive(Default)]
struct RecAudio {
    voice_numbers: Vec<i32>,
    /// (KITFX) (handle, name, looping) of every play, (handle, fade) of every stop
    plays: Vec<(u32, &'static str, bool)>,
    stops: Vec<(u32, f32)>,
}

impl kh_core::host::Audio for RecAudio {
    fn play(&mut self, handle: kh_core::host::SoundHandle, play: &kh_core::host::SoundPlay) {
        self.plays.push((handle.0, play.name, play.looping));
    }
    fn stop(&mut self, handle: kh_core::host::SoundHandle, fade_out: f32) {
        self.stops.push((handle.0, fade_out));
    }
    fn party_voice_group_no(&mut self, _member: kh_core::host::PartyMember, number: i32) {
        self.voice_numbers.push(number);
    }
}

struct Rig {
    world: TestWorld,
    dummies: Dummies,
    audio: RecAudio,
    effects: RecEffects,
    party: Party,
    sora: SoraView,
    don: MemberId,
}

impl Rig {
    fn new(enemies: &[Vec3]) -> Rig {
        Rig::with_world(TestWorld::floor(), enemies)
    }

    fn with_world(world: TestWorld, enemies: &[Vec3]) -> Rig {
        let mut party = Party::new(PartyConfig::test_level99());
        let don = party.spawn(Chara::Donald, Vec3::ZERO, 0.0);
        // the pad drives him (no input): AI Donald's own choices (DON-PROG) stay out of these tests
        party.set_pad(don, Some(kh_core::Tuning::USER));
        {
            let s = party.status_mut(don).unwrap();
            let st = s.donald.as_mut().unwrap();
            st.learned = DonLearned::all();
            // no ability unless a test gives it (the level-99 loadout would bring Instant Attune, the Boosts ...)
            st.abilities = super::abilities::DonAbilities::default();
            s.mp = s.max_mp();
        }
        let list = enemies.iter().enumerate().map(|(i, p)| Dummy { id: TargetId(i as u32 + 1), pos: *p, size: 1, hp: 1.0e6, hp_max: 1.0e6, resist: [0; 8] }).collect();
        let mut rig = Rig { world, dummies: Dummies { list, hits: Vec::new() }, audio: Default::default(), effects: Default::default(), party, sora: SoraView::default(), don };
        rig.sora.position = Vec3::new(-3000.0, 0.0, 0.0);
        rig.sora.hp_max = 100;
        rig.sora.hp = 100;
        rig.idle(2);
        rig
    }

    fn step(&mut self) {
        let mut host = PartyHost { world: &self.world, targets: &mut self.dummies, audio: &mut self.audio, effects: &mut self.effects };
        self.party.step(DT, &self.sora, &mut host);
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.step();
        }
    }

    fn secs(&mut self, s: f32) {
        self.idle((s / DT).round() as usize);
    }

    fn attune(&mut self, e: Element, res: f32) {
        let st = self.party.status_mut(self.don).unwrap().donald.as_mut().unwrap();
        let period = st.core.attune.slots[0].map_or(1, |s| s.period + 1);
        st.core.attune.slots[1] = st.core.attune.slots[0];
        st.core.attune.slots[0] = Some(Slot { element: e, res, period, ult_used: false, full_at: Some(st.core.clock) });
    }

    fn give(&mut self, a: DonAbility) {
        let st = self.party.status_mut(self.don).unwrap().donald.as_mut().unwrap();
        st.abilities.give(a);
        st.abilities.set_equipped(a, true);
    }

    fn plan(&self, spell: SpellRef) -> Result<CastPlan, Refusal> {
        let s = self.party.status(self.don).unwrap();
        let st = s.donald.as_ref().unwrap();
        st.core.check_cast(spell, &st.learned, s.mp, s.max_mp(), st.abilities.mods().dual, &data())
    }

    fn begin(&mut self, spell: SpellRef, target: CastTarget) -> bool {
        let plan = self.plan(spell).unwrap_or_else(|r| panic!("{spell:?} refused: {r:?}"));
        let mut host = PartyHost { world: &self.world, targets: &mut self.dummies, audio: &mut self.audio, effects: &mut self.effects };
        let p = self.party.member_mut(self.don).unwrap();
        cast::begin(p, &plan, target, &mut host)
    }

    /// Casts and steps until the shot plus `after` seconds.
    fn cast(&mut self, spell: SpellRef, target: CastTarget, after: f32) {
        assert!(self.begin(spell, target), "{spell:?} did not begin");
        // (DON-SPELLB) the cast's own release time (the ultimates: ZETTA's channel)
        let shot = self.party.member(self.don).and_then(|p| p.don_cast.run).map_or(0.0, |r| r.shot_at);
        let shot = if shot > 60.0 { spell.row(&data()).map_or(0.0, |r| r.cast) } else { shot }; // (Zettaflare: its own sequence)
        self.secs(shot + after);
    }

    fn mp(&self) -> i32 {
        self.party.status(self.don).unwrap().mp
    }

    fn spawned(&self, name: &str) -> usize {
        self.effects.spawned.iter().filter(|e| e.1 == name).count()
    }

    fn status(&self, id: u32) -> Option<super::status::TargetStatus> {
        self.party.don.status.get(id).copied()
    }

    fn log(&self) -> &[FxLog] {
        &self.party.don_fx.log
    }

    fn reacted(&self, r: Reaction) -> bool {
        self.log().iter().any(|e| matches!(e, FxLog::Hit { reactions, .. } if reactions.contains(&r)))
    }
}

fn e1() -> CastTarget {
    CastTarget::Enemy(TargetId(1))
}

// ================================================================================================ the cast itself

#[test]
fn the_cast_clip_by_spell_and_air() {
    assert_eq!(cast::clip_of(SpellRef::Line1(Element::Fire, 2), false), "MagicFire");
    assert_eq!(cast::clip_of(SpellRef::Line1(Element::Blizzard, 1), true), "MagicAirFire");
    assert_eq!(cast::clip_of(SpellRef::Line1(Element::Fire, 4), false), "MagicCast");
    assert_eq!(cast::clip_of(SpellRef::Line1(Element::Thunder, 1), false), "MagicCast");
    assert_eq!(cast::clip_of(SpellRef::Line1(Element::Water, 2), true), "MagicAirCast");
    assert_eq!(cast::clip_of(SpellRef::Line2(Element::Water, 1), false), "1311");
    assert_eq!(cast::clip_of(SpellRef::Arcane(ArcaneLine::Meteor, 1), false), "MagicCast");
}

#[test]
fn begin_spends_the_mp_at_once_commits_and_shoots_at_the_rows_cast_time() {
    let mut rig = Rig::new(&[Vec3::new(800.0, 0.0, 0.0)]);
    let spell = SpellRef::Line2(Element::Fire, 1);
    let plan = rig.plan(spell).unwrap();
    let before = rig.mp();
    assert!(rig.begin(spell, e1()));
    assert_eq!(rig.mp(), before - plan.mp, "the MP is spent at the start");
    assert!(cast::before_shot(rig.party.member(rig.don).unwrap()));
    let row = spell.row(&data()).copied().unwrap();
    assert!((row.cast - 0.8).abs() < 1e-4, "Flare Lv 1: 0.8 s cast");
    // before the cast time: no star; at it: the star flies
    rig.secs(row.cast - 0.1);
    assert_eq!(rig.spawned(fx::FLARE_STAR), 0);
    rig.secs(0.15);
    assert_eq!(rig.spawned(fx::FLARE_STAR), 1, "the star leaves at the cast time");
    // a second cast before the shot is refused; after it the cast frees him after cast.recover
    rig.secs(magic_fx::magic().cast_recover + 0.05);
    assert!(!cast::casting(rig.party.member(rig.don).unwrap()));
}

#[test]
fn a_hit_while_casting_interrupts_and_the_mp_is_lost() {
    let mut rig = Rig::new(&[Vec3::new(800.0, 0.0, 0.0)]);
    let spell = SpellRef::Line2(Element::Fire, 1);
    let before = rig.mp();
    assert!(rig.begin(spell, e1()));
    rig.secs(0.2);
    let mut hit = HitRecord::empty(TargetId::NONE);
    hit.damage = 5.0;
    hit.damage_kind = Kind::Small;
    let r = rig.party.receive_hit(rig.don, &hit);
    assert!(r.accepted && r.damage_dealt > 0.0);
    let p = rig.party.member(rig.don).unwrap();
    assert!(!cast::casting(p), "interrupted");
    assert_eq!(p.don_cast.interrupted, 1);
    assert!(rig.mp() < before, "the MP stays spent");
    rig.secs(1.5);
    assert_eq!(rig.spawned(fx::FLARE_STAR), 0, "no star after the interruption");
}

#[test]
fn cast_guard_shrugs_off_about_thirty_percent_of_the_hits() {
    let mut guarded = 0;
    let n = 200;
    for seed in 0..n {
        let mut rig = Rig::new(&[Vec3::new(800.0, 0.0, 0.0)]);
        rig.give(DonAbility::CastGuard);
        rig.party.member_mut(rig.don).unwrap().rng = crate::rng::Rng::new(seed as u64 * 7919 + 1);
        assert!(rig.begin(SpellRef::Line1(Element::Fire, 1), e1()));
        let p = rig.party.member_mut(rig.don).unwrap();
        if !cast::interrupt(p, true) {
            guarded += 1;
        }
    }
    let share = guarded as f32 / n as f32;
    assert!((0.2..=0.4).contains(&share), "Cast Guard share {share}");
    // without it every hit interrupts
    let mut rig = Rig::new(&[Vec3::new(800.0, 0.0, 0.0)]);
    assert!(rig.begin(SpellRef::Line1(Element::Fire, 1), e1()));
    assert!(cast::interrupt(rig.party.member_mut(rig.don).unwrap(), true));
}

#[test]
fn the_attune_cast_is_interrupted_by_the_same_rule() {
    let mut rig = Rig::new(&[]);
    let st = rig.party.status_mut(rig.don).unwrap().donald.as_mut().unwrap();
    let (learned, mods) = (st.learned, st.abilities.mods());
    assert!(st.core.begin_attune(Element::Fire, &learned, &mods, &data()).unwrap() > 0.0);
    assert!(cast::interrupt(rig.party.member_mut(rig.don).unwrap(), true));
    assert!(rig.party.status(rig.don).unwrap().donald.as_ref().unwrap().core.attune.cast.is_none());
}

#[test]
fn the_voices_big_spells_take_101_to_103() {
    let mut rig = Rig::new(&[Vec3::new(800.0, 0.0, 0.0)]);
    rig.attune(Element::Fire, 3.0);
    rig.cast(SpellRef::Line1(Element::Fire, 4), e1(), 0.5);
    assert!(rig.audio.voice_numbers.iter().any(|n| (101..=103).contains(n)), "Firaza: a big callout {:?}", rig.audio.voice_numbers);
}

// ================================================================================================ line 1

#[test]
fn line1_uses_soras_actors_and_the_rows_statuses() {
    let mut rig = Rig::new(&[Vec3::new(600.0, 0.0, 0.0)]);
    rig.cast(SpellRef::Line1(Element::Fire, 1), e1(), 1.5);
    assert!(rig.log().iter().any(|e| matches!(e, FxLog::Sora { cmd: 29, .. })), "Fire = Sora's command 29");
    let hits = rig.dummies.hits_on(1);
    assert!(!hits.is_empty(), "Sora's Fire actor hit the dummy");
    assert!(hits.iter().all(|h| h.flags & DON_HIT_FLAG != 0), "through the reaction rule");
    assert!(rig.status(1).is_some_and(|s| s.is_burned()), "Fire Burns (row applies)");
    // the -za tier is Sora's Grand Magic -gan actor
    assert_eq!(magic_fx::sora_cmd(Element::Fire, 4), 112);
    assert_eq!(magic_fx::sora_cmd(Element::Light, 4), 125);
    assert_eq!(magic_fx::sora_cmd(Element::Water, 3), 43);
}

#[test]
fn line1_power_scale_is_the_rows_power_over_soras_row() {
    let d = data();
    for e in [Element::Fire, Element::Blizzard, Element::Thunder, Element::Water, Element::Aero] {
        for t in 1..=4u8 {
            let s = magic_fx::l1_scale(e, t, &d);
            assert!(s > 0.0 && s.is_finite(), "{e:?} {t}: {s}");
            assert!(magic_fx::sora_row(e, t).is_some(), "Sora's row of {e:?} {t}");
        }
    }
    assert_eq!(magic_fx::tier_of_attack("MAG_FIRA_FIN"), Some((Element::Fire, 2)));
    assert_eq!(magic_fx::tier_of_attack("MAG_B_BLIZZAGAN"), Some((Element::Blizzard, 4)));
    assert_eq!(magic_fx::tier_of_attack("MAG_WATER_BARRIER"), Some((Element::Water, 1)));
    assert_eq!(magic_fx::tier_of_attack("FR_SOMETHING"), None);
}

#[test]
fn the_boosts_add_twenty_percent() {
    let mut a = super::abilities::DonAbilities::default();
    assert_eq!(magic_fx::boost_of(SpellRef::Line2(Element::Fire, 1), &a), 1.0);
    a.give(DonAbility::FireBoost);
    a.set_equipped(DonAbility::FireBoost, true);
    assert!((magic_fx::boost_of(SpellRef::Line2(Element::Fire, 1), &a) - 1.2).abs() < 1e-5);
    assert!((magic_fx::boost_of(SpellRef::Hybrid(Element::Fire, Element::Water), &a) - 1.2).abs() < 1e-5, "a hybrid: either element");
    assert_eq!(magic_fx::boost_of(SpellRef::Line2(Element::Water, 1), &a), 1.0);
    a.give(DonAbility::ArcaneBoost);
    a.set_equipped(DonAbility::ArcaneBoost, true);
    assert!((magic_fx::boost_of(SpellRef::Arcane(ArcaneLine::Meteor, 1), &a) - 1.2).abs() < 1e-5);
    assert_eq!(magic_fx::boost_of(SpellRef::Arcane(ArcaneLine::Zettaflare, 1), &a), 1.0, "Arcane Boost: Comet, Gravity, Meteor, Ultima");
}

#[test]
fn cure_heals_by_kh3s_rule_and_curaza_needs_full_power() {
    let mut rig = Rig::new(&[]);
    {
        let s = rig.party.status_mut(rig.don).unwrap();
        s.hp = s.max_hp() / 3;
    }
    let before = rig.party.status(rig.don).unwrap().hp;
    rig.cast(SpellRef::Line1(Element::Light, 3), CastTarget::Member(rig.don), 1.5);
    assert!(rig.party.status(rig.don).unwrap().hp > before, "Curaga healed him");
    assert_eq!(rig.plan(SpellRef::Line1(Element::Light, 4)), Err(Refusal::NeedsFullPower), "Curaza: full power only (R4)");
    rig.attune(Element::Light, 3.0);
    assert!(rig.plan(SpellRef::Line1(Element::Light, 4)).is_ok());
}

// ================================================================================================ line 2

#[test]
fn flare_shrinks_then_detonates_with_its_splash_and_eats_the_burn() {
    // Lv 1: 2 m splash; the neighbour 2.5 m away is spared
    let mut rig = Rig::new(&[Vec3::new(700.0, 0.0, 0.0), Vec3::new(700.0, 280.0, 0.0)]);
    assert!(rig.begin(SpellRef::Line2(Element::Fire, 1), e1()));
    rig.secs(0.8 + 0.35);
    // flying ~1.5 m/s * ... the star arrives, then shrinks 0.5 s before it detonates
    let first_hit = (0..200).find(|_| {
        rig.step();
        !rig.dummies.hits_on(1).is_empty()
    });
    assert!(first_hit.is_some(), "the flare detonated");
    assert!(rig.spawned(fx::FLARE_BOOM) == 1);
    assert!(rig.dummies.hits_on(2).is_empty(), "Lv 1: 2 m splash");
    assert_eq!(rig.dummies.hits_on(1)[0].damage_kind, Kind::Blow, "a big stagger");
    let plain = rig.dummies.damage_on(1);
    // on a Burned target: +50 %, the Burn consumed
    let mut rig = Rig::new(&[Vec3::new(700.0, 0.0, 0.0)]);
    rig.party.don.status.entry(1, Size::Small).burn = 6.0;
    rig.cast(SpellRef::Line2(Element::Fire, 1), e1(), 1.5);
    let burned = rig.dummies.hits_on(1).iter().filter(|h| h.attack_name.starts_with("Flare")).map(|h| h.damage).sum::<f32>();
    assert!((burned / plain - 1.5).abs() < 0.02, "Burn consumed for +50 %: {burned} vs {plain}");
    assert!(!rig.status(1).is_some_and(|s| s.is_burned()), "the Burn is used up");
    // Lv 3: 0.4 s cast, an after-flare 1 s later at half damage
    let mut rig = Rig::new(&[Vec3::new(700.0, 0.0, 0.0)]);
    rig.cast(SpellRef::Line2(Element::Fire, 3), e1(), 3.0);
    assert_eq!(rig.spawned(fx::FLARE_AFTER), 1, "the after-flare");
    let hits = rig.dummies.hits_on(1);
    assert_eq!(hits.len(), 2);
    assert!((hits[1].damage / hits[0].damage - 0.5).abs() < 0.02, "after-flare at half damage");
}

#[test]
fn freeze_warns_one_second_then_freezes_and_bosses_slow_instead() {
    let mut rig = Rig::new(&[Vec3::new(700.0, 0.0, 0.0), Vec3::new(700.0, 200.0, 0.0)]);
    assert!(rig.begin(SpellRef::Line2(Element::Blizzard, 1), e1()));
    rig.secs(0.4 + 0.9);
    assert_eq!(rig.spawned(fx::FREEZE_FROST), 1, "the frost warning");
    assert!(!rig.status(1).is_some_and(|s| s.is_frozen()), "not yet: the 1 s warning");
    rig.secs(0.2);
    let s = rig.status(1).unwrap();
    assert!(s.is_frozen() && s.frozen > 2.5 && s.frozen <= 3.0, "Frozen 3 s: {}", s.frozen);
    assert!(rig.dummies.hits_on(1).iter().any(|h| h.info.bad_status == "FREEZE"), "kh-react holds the body (FREEZE)");
    assert!(rig.status(2).is_none_or(|s| !s.is_frozen()), "Lv 1: the target only");
    // Lv 2: every small and medium within 3 m, 4 s
    let mut rig = Rig::new(&[Vec3::new(700.0, 0.0, 0.0), Vec3::new(700.0, 200.0, 0.0)]);
    rig.cast(SpellRef::Line2(Element::Blizzard, 2), e1(), 1.2);
    assert!(rig.status(2).is_some_and(|s| s.is_frozen() && s.frozen > 3.5));
    // large: 3 Chill and -30 % for 4 s
    let mut rig = Rig::new(&[Vec3::new(700.0, 0.0, 0.0)]);
    rig.dummies.list[0].size = 4;
    rig.cast(SpellRef::Line2(Element::Blizzard, 1), e1(), 1.2);
    let s = rig.status(1).unwrap();
    assert!(!s.is_frozen() && s.chill == 3 && s.big_slow > 3.5, "boss: {s:?}");
    // Lv 3: shards Chill the neighbours (a large neighbour too)
    let mut rig = Rig::new(&[Vec3::new(700.0, 0.0, 0.0), Vec3::new(700.0, 250.0, 0.0), Vec3::new(900.0, 400.0, 0.0)]);
    rig.dummies.list[2].size = 3;
    rig.cast(SpellRef::Line2(Element::Blizzard, 3), e1(), 1.2);
    assert_eq!(rig.spawned(fx::FREEZE_SHARDS), 2, "each freezer throws shards");
    assert!(rig.status(3).is_some_and(|s| s.chill >= 1 || s.big_slow > 0.0), "the shards Chilled the large one");
}

/// Ball Lightning's ring (party-don-magic orbs.*) and a ball's contact sphere (the row radius): an enemy whose body (radius
/// 50 in the rig) stands `gap` cm beyond the reach of the balls' spheres, on the +X axis from Donald.
fn beyond_balls(level: u8, gap: f32) -> Vec3 {
    let m = magic_fx::magic();
    let r = SpellRef::Line2(Element::Thunder, level).row(&data()).copied().unwrap().radius;
    Vec3::new(m.orbs_orbit_radius + r + 50.0 + gap, 0.0, 0.0)
}

#[test]
fn ball_lightning_balls_orbit_the_wearer_and_hit_only_on_contact() {
    // (the user's change of 2026-10-22) an enemy 1 m beyond the balls' reach: no contact, no damage, for the whole 5 s
    let mut rig = Rig::new(&[beyond_balls(1, 100.0)]);
    rig.cast(SpellRef::Line2(Element::Thunder, 1), CastTarget::Member(rig.don), 0.1);
    let v = super::allyfx::orbs_of(&rig.party, rig.don).unwrap();
    assert_eq!((v.orbs, v.max, v.active), (3, 3, true));
    assert!((v.total - 5.0).abs() < 1e-4, "Lv 1: 5 s");
    assert_eq!(rig.spawned(fx::ORB_CORE), 3, "three balls shown");
    assert!(rig.effects.tinted.iter().any(|t| t.1 == fx::ELECTRIC), "the cores are electric white-blue");
    // they orbit: each core is moved round the wearer at the ring's radius and height
    let m = magic_fx::magic();
    let core = rig.effects.spawned.iter().find(|e| e.1 == fx::ORB_CORE).unwrap().0;
    let path: Vec<Vec3> = rig.effects.placed.iter().filter(|p| p.0 == core).map(|p| p.1).collect();
    rig.secs(1.0);
    let path2: Vec<Vec3> = rig.effects.placed.iter().filter(|p| p.0 == core).map(|p| p.1).collect();
    let (a, b) = (path.last().copied().unwrap(), path2.last().copied().unwrap());
    assert!((a.truncate().length() - m.orbs_orbit_radius).abs() < 1.0 && (a.z - m.orbs_height).abs() < 1.0, "on the ring: {a:?}");
    assert!((a - b).length() > 50.0, "it moved round: {a:?} -> {b:?}");
    assert!(rig.spawned(fx::ORB_CRACKLE[0]) + rig.spawned(fx::ORB_CRACKLE[1]) >= 6, "they crackle");
    rig.secs(4.5);
    assert!(rig.dummies.hits.is_empty(), "no contact, no damage: {:?}", rig.dummies.hits.len());
    assert!(super::allyfx::orbs_of(&rig.party, rig.don).is_none(), "the time ran out");
    assert!(rig.spawned(fx::HIT_THUNDER) <= rig.spawned(fx::ORB_CRACKLE[0]), "no leap, no area hit");

    // the balls move with the wearer: Donald somewhere else, the ring round him
    let mut rig = Rig::new(&[beyond_balls(1, 3000.0)]);
    rig.party.member_mut(rig.don).unwrap().cmc.location = Vec3::new(-800.0, 400.0, 0.0);
    rig.cast(SpellRef::Line2(Element::Thunder, 1), CastTarget::Member(rig.don), 0.2);
    let at = rig.party.member(rig.don).unwrap().position();
    let core = rig.effects.spawned.iter().find(|e| e.1 == fx::ORB_CORE).unwrap().0;
    let p = rig.effects.placed.iter().rev().find(|p| p.0 == core).unwrap().1;
    assert!(((p - at).truncate().length() - m.orbs_orbit_radius).abs() < 5.0, "round the wearer: {p:?} vs {at:?}");
}

#[test]
fn ball_lightning_a_touch_uses_up_that_ball_and_all_can_be_spent_early() {
    // an enemy standing in the ring's path: every ball that comes round touches it
    let mut rig = Rig::new(&[Vec3::new(150.0, 0.0, 0.0)]);
    assert!(rig.begin(SpellRef::Line2(Element::Thunder, 1), CastTarget::Member(rig.don)));
    let mut first = None;
    for _ in 0..(5.0 / DT) as usize {
        rig.step();
        let hits = rig.dummies.hits_on(1).len();
        if first.is_none() && hits == 1 {
            // exactly the ball that touched is gone: one core stopped, two still orbiting
            let o = super::allyfx::orbs_of(&rig.party, rig.don).unwrap();
            assert_eq!((o.orbs, o.active), (2, true));
            let cores: Vec<u32> = rig.effects.spawned.iter().filter(|e| e.1 == fx::ORB_CORE).map(|e| e.0).collect();
            let gone: Vec<u32> = cores.iter().copied().filter(|h| rig.effects.stopped.contains(h)).collect();
            assert_eq!(gone.len(), 1, "one ball used up");
            let set = rig.party.don_fx.worn.orbs(rig.don).unwrap();
            assert_eq!(set.balls.iter().filter(|b| !b.live).count(), 1);
            let h = &rig.dummies.hits_on(1)[0];
            assert_eq!(h.attribute, kh_core::enums::ETresDamageAttribute::Thunder);
            assert!(h.info.move_length > 0.0 && h.damage_kind == Kind::Blow, "knocked back");
            first = Some(rig.party.don_fx.worn.orbs(rig.don).unwrap().left);
        }
        if rig.dummies.hits_on(1).len() >= 3 {
            break;
        }
    }
    assert!(first.is_some(), "a ball touched");
    assert_eq!(rig.dummies.hits_on(1).len(), 3, "each ball hit once");
    assert!(super::allyfx::orbs_of(&rig.party, rig.don).is_none(), "all three spent before the 5 s: the set is gone");
    rig.secs(1.0);
    assert_eq!(rig.dummies.hits_on(1).len(), 3, "used up for good");

    // on a Wet enemy a ball Conducts
    let mut rig = Rig::new(&[Vec3::new(150.0, 0.0, 0.0)]);
    rig.party.don.status.entry(1, Size::Small).wet = 8.0;
    rig.cast(SpellRef::Line2(Element::Thunder, 1), CastTarget::Member(rig.don), 1.0);
    assert!(rig.reacted(Reaction::Conduct), "a ball on a Wet enemy Conducts");

    // a recast with balls left hurls them at the lock-on target as homing shots, each carrying its glow core
    let mut rig = Rig::new(&[beyond_balls(1, 1000.0)]);
    rig.cast(SpellRef::Line2(Element::Thunder, 1), CastTarget::Member(rig.don), 0.1);
    rig.party.member_mut(rig.don).unwrap().focus = crate::command::TargetRef::Enemy(TargetId(1));
    let cores_before = rig.spawned(fx::ORB_CORE);
    rig.cast(SpellRef::Line2(Element::Thunder, 1), CastTarget::Member(rig.don), 1.5);
    assert!(super::allyfx::orbs_of(&rig.party, rig.don).is_none(), "the set was hurled");
    assert_eq!(rig.dummies.hits_on(1).len(), 3, "every hurled ball hit");
    assert_eq!(rig.spawned(fx::ORB_CORE) - cores_before, 3, "each hurled ball carries a core");
}

#[test]
fn ball_lightning_lv2_recharges_lv3_chains_and_one_set_per_member() {
    // enemy 1 in the ring's path, enemy 2 out of it but within the Lv 3 arc's range
    let mut rig = Rig::new(&[Vec3::new(150.0, 0.0, 0.0), beyond_balls(3, 200.0)]);
    let goofy = rig.party.spawn(Chara::Goofy, Vec3::new(-500.0, 0.0, 0.0), 0.0);
    rig.cast(SpellRef::Line2(Element::Thunder, 3), CastTarget::Member(rig.don), 0.05);
    let v = super::allyfx::orbs_of(&rig.party, rig.don).unwrap();
    assert_eq!((v.max, v.level), (5, 3));
    assert!((v.total - 20.0).abs() < 1e-4);
    rig.idle(2);
    assert!(!rig.dummies.hits_on(2).is_empty(), "Lv 3: a touched ball arcs to a second enemy");
    // Lv 2 recharge
    rig.secs(3.5);
    let o = super::allyfx::orbs_of(&rig.party, rig.don).unwrap();
    assert!(o.orbs < o.max || rig.dummies.hits_on(1).len() > 1);
    // another member wears his own set
    rig.secs(0.5);
    rig.cast(SpellRef::Line2(Element::Thunder, 2), CastTarget::Member(goofy), 0.05);
    assert!(super::allyfx::orbs_of(&rig.party, goofy).is_some());
    assert!(super::allyfx::orbs_of(&rig.party, rig.don).is_some(), "one set per member");
    assert_eq!(rig.party.don_fx.worn.orbs.iter().filter(|o| o.member == goofy).count(), 1);
}

#[test]
fn flood_arc_push_wet_wall_stun_and_puddle() {
    // Lv 1: a 90 deg arc, 8 m, push 5 m; the one behind / beyond 8 m is spared
    let mut rig = Rig::new(&[Vec3::new(500.0, 0.0, 0.0), Vec3::new(-400.0, 0.0, 0.0), Vec3::new(1000.0, 0.0, 0.0)]);
    rig.cast(SpellRef::Line2(Element::Water, 1), e1(), 0.6);
    let h = rig.dummies.hits_on(1);
    assert!(!h.is_empty());
    assert!((h[0].info.move_length - 500.0).abs() < 1.0 && h[0].damage_kind == Kind::Blow, "push 5 m");
    assert!(rig.status(1).is_some_and(|s| s.is_wet()));
    assert!(rig.dummies.hits_on(2).is_empty(), "behind him");
    assert!(rig.dummies.hits_on(3).is_empty(), "beyond 8 m");
    // the puddle keeps what stands in it Wet, no damage
    rig.party.don.status.forget(1);
    rig.secs(1.0);
    assert!(rig.status(1).is_some_and(|s| s.is_wet()), "re-wet by the puddle (it stands in it)");
    let puddle = rig.dummies.hits_on(1).last().copied().unwrap();
    assert_eq!(puddle.damage, 0.0);
    // a wall behind the enemy: extra damage and a 1 s stun
    let world = TestWorld::floor().with_box(Vec3::new(700.0, -500.0, 0.0), Vec3::new(800.0, 500.0, 400.0));
    let mut rig = Rig::with_world(world, &[Vec3::new(500.0, 0.0, 0.0)]);
    rig.cast(SpellRef::Line2(Element::Water, 1), e1(), 0.6);
    let h = rig.dummies.hits_on(1)[0];
    assert_eq!(h.info.bad_status, "STUN");
    assert!((h.info.bad_time - 1.0).abs() < 1e-4);
    // Lv 3: a full ring, 15 m, small enemies knocked down
    let mut rig = Rig::new(&[Vec3::new(-1200.0, 0.0, 0.0)]);
    rig.cast(SpellRef::Line2(Element::Water, 3), e1(), 1.2);
    let h = rig.dummies.hits_on(1);
    assert!(!h.is_empty(), "the ring reaches behind him");
    assert_eq!(h[0].damage_kind, Kind::Parabola, "Lv 3 knocks small enemies down");
}

#[test]
fn aero_barrier_cuts_lasts_renews_one_per_member_and_lv3_hurts() {
    let mut rig = Rig::new(&[Vec3::new(3000.0, 0.0, 0.0)]);
    rig.cast(SpellRef::Line2(Element::Aero, 1), CastTarget::Member(rig.don), 0.1);
    let b = super::allyfx::barrier_of(&rig.party, rig.don).unwrap();
    assert!((b.cut - 0.25).abs() < 1e-4 && (b.total - 20.0).abs() < 1e-4, "R28: -25 % for 20 s");
    assert_eq!(rig.spawned(fx::BARRIER_LOOK[0]), 1, "the wind ribbons on the wearer"); // (DON-SPELLB)
    let mut hit = HitRecord::empty(TargetId::NONE);
    hit.damage = 20.0;
    hit.info.victim_result = kh_core::host::ReactionResult::NoReaction;
    let r = rig.party.receive_hit(rig.don, &hit);
    assert!((r.damage_dealt - 15.0).abs() < 1.01, "20 x 0.75: {}", r.damage_dealt);
    // a recast renews (no second barrier)
    rig.secs(10.0);
    rig.cast(SpellRef::Line2(Element::Aero, 2), CastTarget::Member(rig.don), 0.1);
    assert_eq!(rig.party.don_fx.worn.barriers.len(), 1);
    let b = super::allyfx::barrier_of(&rig.party, rig.don).unwrap();
    assert!(b.left > 29.0 && (b.cut - 0.4).abs() < 1e-4, "renewed at Lv 2");
    rig.secs(31.0);
    assert!(super::allyfx::barrier_of(&rig.party, rig.don).is_none(), "it runs out");
    // Lv 3: enemies touching it are hurt
    rig.dummies.list[0].pos = Vec3::new(120.0, 0.0, 0.0);
    rig.cast(SpellRef::Line2(Element::Aero, 3), CastTarget::Member(rig.don), 0.2);
    assert!(!rig.dummies.hits_on(1).is_empty(), "Lv 3 contact hit");
    // on Sora: the hosts read the cut for SORA_MEMBER
    rig.secs(1.0);
    rig.cast(SpellRef::Line2(Element::Aero, 1), CastTarget::Member(SORA_MEMBER), 0.1);
    assert!((super::allyfx::damage_mult(&rig.party, SORA_MEMBER) - 0.75).abs() < 1e-4);
}

#[test]
fn banish_reveals_and_banish_iii_fires_more_rays_at_nearby_enemies() {
    let mut rig = Rig::new(&[Vec3::new(700.0, 0.0, 0.0), Vec3::new(800.0, 300.0, 0.0), Vec3::new(600.0, -300.0, 0.0)]);
    rig.cast(SpellRef::Line2(Element::Light, 1), e1(), 0.3);
    assert!(rig.status(1).is_some_and(|s| s.is_revealed()));
    assert_eq!(rig.spawned(fx::BANISH_RAY), 1);
    assert!(rig.dummies.hits_on(2).is_empty());
    let mut rig = Rig::new(&[Vec3::new(700.0, 0.0, 0.0), Vec3::new(800.0, 300.0, 0.0), Vec3::new(600.0, -300.0, 0.0)]);
    rig.cast(SpellRef::Line2(Element::Light, 3), e1(), 1.0);
    assert_eq!(rig.spawned(fx::BANISH_RAY), 5, "Banish III: 5 rays");
    assert!(rig.status(2).is_some_and(|s| s.is_revealed()) && rig.status(3).is_some_and(|s| s.is_revealed()));
}

// ================================================================================================ ultimates

#[test]
fn ultimates_once_per_period_and_each_works() {
    let mut rig = Rig::new(&[Vec3::new(700.0, 0.0, 0.0), Vec3::new(900.0, 200.0, 0.0)]);
    rig.dummies.list[1].size = 4;
    rig.attune(Element::Fire, 3.0);
    assert!(rig.begin(SpellRef::Ultimate(Element::Fire), e1()));
    assert_eq!(rig.plan(SpellRef::Ultimate(Element::Fire)), Err(Refusal::UltimateSpent), "once per period");
    let shot = rig.party.member(rig.don).and_then(|p| p.don_cast.run).map_or(1.0, |r| r.shot_at); // (DON-SPELLB) ZETTA's channel
    rig.secs(shot + 2.5);
    assert!(rig.dummies.hits_on(1).len() >= 2, "Inferno burns what stands in it");
    assert!(rig.status(1).is_some_and(|s| s.is_burned()));
    // Diamond Dust freezes all (a boss: the slow)
    rig.attune(Element::Blizzard, 3.0);
    rig.party.don.status.forget(1);
    rig.cast(SpellRef::Ultimate(Element::Blizzard), e1(), 1.0);
    assert!(rig.status(1).is_some_and(|s| s.is_frozen()));
    assert!(rig.status(2).is_some_and(|s| !s.is_frozen() && s.big_slow > 0.0));
    // Judgment Bolt: 12 bolts over the enemies around
    let mut rig = Rig::new(&[Vec3::new(700.0, 0.0, 0.0), Vec3::new(-700.0, 0.0, 0.0)]);
    rig.attune(Element::Thunder, 3.0);
    rig.cast(SpellRef::Ultimate(Element::Thunder), e1(), 3.0);
    assert_eq!(rig.dummies.hits_on(1).len() + rig.dummies.hits_on(2).len(), 12);
    // Tsunami: a ring that pushes 10 m and wets
    let mut rig = Rig::new(&[Vec3::new(-900.0, 0.0, 0.0)]);
    rig.attune(Element::Water, 3.0);
    rig.cast(SpellRef::Ultimate(Element::Water), e1(), 1.5);
    let h = rig.dummies.hits_on(1);
    assert!(!h.is_empty() && (h[0].info.move_length - 1000.0).abs() < 1.0 && rig.status(1).is_some_and(|s| s.is_wet()));
    // Aerial Blast: the cannon launches a small one
    let mut rig = Rig::new(&[Vec3::new(1200.0, 0.0, 0.0), Vec3::new(1200.0, 900.0, 0.0)]);
    rig.attune(Element::Aero, 3.0);
    rig.cast(SpellRef::Ultimate(Element::Aero), e1(), 0.2);
    assert_eq!(rig.dummies.hits_on(1)[0].damage_kind, Kind::Parabola, "launched");
    assert!(rig.dummies.hits_on(2).is_empty(), "outside the cannon");
    // Holy: big light damage on one target, Revealed
    let mut rig = Rig::new(&[Vec3::new(700.0, 0.0, 0.0)]);
    rig.attune(Element::Light, 3.0);
    rig.cast(SpellRef::Ultimate(Element::Light), e1(), 0.6);
    assert!(rig.dummies.damage_on(1) > 0.0 && rig.status(1).is_some_and(|s| s.is_revealed()));
    assert_eq!(rig.spawned(fx::HOLY[2]), 1);
}

// ================================================================================================ hybrids

fn dual_rig(enemies: &[Vec3], a: Element, b: Element) -> Rig {
    let mut rig = Rig::new(enemies);
    rig.give(DonAbility::DualAttunement);
    rig.attune(a, 1.0);
    rig.attune(b, 1.0);
    rig
}

#[test]
fn every_hybrid_casts_hits_and_light_hybrids_never_heal() {
    for (a, b) in hybrid_pairs() {
        let mut rig = dual_rig(&[Vec3::new(600.0, 0.0, 0.0), Vec3::new(650.0, 120.0, 0.0)], a, b);
        {
            let s = rig.party.status_mut(rig.don).unwrap();
            s.hp = s.max_hp() / 2;
        }
        let hp = rig.party.status(rig.don).unwrap().hp;
        let spell = SpellRef::hybrid(a, b).unwrap();
        rig.cast(spell, e1(), 2.0);
        let hits = rig.dummies.hits_on(1).len() + rig.dummies.hits_on(2).len();
        assert!(hits > 0, "{spell:?} hit nothing");
        assert!(!rig.log().iter().any(|e| matches!(e, FxLog::Heal { .. })), "{spell:?} healed");
        // (HP may still rise from Light attunement's own Regen aura, DON-PROG: the hybrid itself never heals)
        let _ = hp;
        if a == Element::Light || b == Element::Light {
            assert!(rig.status(1).is_some_and(|s| s.is_revealed()) || rig.status(2).is_some_and(|s| s.is_revealed()), "{spell:?}: Light's part Reveals");
        }
    }
}

#[test]
fn hybrids_count_as_both_elements() {
    // Plasma (Fire + Thunder) on a Wet enemy Conducts instead of Fizzling
    let mut rig = dual_rig(&[Vec3::new(600.0, 0.0, 0.0), Vec3::new(700.0, 600.0, 0.0)], Element::Fire, Element::Thunder);
    rig.party.don.status.entry(1, Size::Small).wet = 8.0;
    rig.party.don.status.entry(2, Size::Small).wet = 8.0;
    rig.cast(SpellRef::Hybrid(Element::Fire, Element::Thunder), e1(), 2.5);
    assert!(rig.reacted(Reaction::Conduct));
    assert!(!rig.reacted(Reaction::Fizzle));
    assert!(!rig.dummies.hits_on(2).is_empty(), "the Conduct chained to the other Wet enemy");
    // Scald (Fire + Water): never a Fizzle, Burn + Blind
    let mut rig = dual_rig(&[Vec3::new(600.0, 0.0, 0.0)], Element::Fire, Element::Water);
    rig.party.don.status.entry(1, Size::Small).wet = 8.0;
    rig.cast(SpellRef::Hybrid(Element::Fire, Element::Water), e1(), 1.5);
    assert!(!rig.reacted(Reaction::Fizzle));
    assert!(rig.status(1).is_some_and(|s| s.is_burned() && s.is_blind()));
}

#[test]
fn hybrid_special_rules() {
    // Plasma: the Burn arcs to one more enemy each second
    let mut rig = dual_rig(&[Vec3::new(600.0, 0.0, 0.0), Vec3::new(600.0, 550.0, 0.0)], Element::Fire, Element::Thunder);
    rig.cast(SpellRef::Hybrid(Element::Fire, Element::Thunder), e1(), 1.2);
    rig.secs(2.0);
    assert!(rig.status(2).is_some_and(|s| s.is_burned()), "the Burn arced");
    // Glacier: a wall that blocks projectiles for 5 s, enemies caught Frozen
    let mut rig = dual_rig(&[Vec3::new(600.0, 0.0, 0.0)], Element::Blizzard, Element::Water);
    rig.cast(SpellRef::Hybrid(Element::Blizzard, Element::Water), e1(), 0.1);
    assert_eq!(rig.party.don_walls().len(), 1);
    assert!(magic_fx::blocked_by_walls(rig.party.don_walls(), Vec3::new(600.0, 0.0, 100.0)));
    assert!(rig.status(1).is_some_and(|s| s.is_frozen()));
    rig.secs(5.2);
    assert!(rig.party.don_walls().is_empty(), "it melts after 5 s");
    // Maelstrom: drags to its centre and wets
    let mut rig = dual_rig(&[Vec3::new(600.0, 0.0, 0.0), Vec3::new(600.0, 400.0, 0.0)], Element::Water, Element::Aero);
    rig.cast(SpellRef::Hybrid(Element::Water, Element::Aero), e1(), 0.6);
    let h = rig.dummies.hits_on(2);
    assert!(!h.is_empty() && h[0].direction.y < -0.5, "pulled toward the centre");
    assert!(rig.status(2).is_some_and(|s| s.is_wet()));
    // Thunderstorm strikes the Wet ones
    let mut rig = dual_rig(&[Vec3::new(600.0, 0.0, 0.0), Vec3::new(600.0, 300.0, 0.0)], Element::Thunder, Element::Water);
    rig.cast(SpellRef::Hybrid(Element::Thunder, Element::Water), e1(), 2.5);
    assert!(rig.dummies.hits_on(1).iter().any(|h| h.attribute == kh_core::enums::ETresDamageAttribute::Thunder && h.damage > 0.0));
    // Divine Spark jumps once more per Revealed enemy
    let mut rig = dual_rig(&[Vec3::new(600.0, 0.0, 0.0), Vec3::new(900.0, 0.0, 0.0), Vec3::new(1200.0, 0.0, 0.0), Vec3::new(1500.0, 0.0, 0.0), Vec3::new(1800.0, 0.0, 0.0), Vec3::new(2100.0, 0.0, 0.0)], Element::Thunder, Element::Light);
    rig.party.don.status.entry(2, Size::Small).reveal = 8.0;
    rig.cast(SpellRef::Hybrid(Element::Thunder, Element::Light), e1(), 2.0);
    let n = (1..=6).filter(|i| !rig.dummies.hits_on(*i).is_empty()).count();
    assert_eq!(n, 5, "4 hops + 1 for the Revealed one");
}

// ================================================================================================ Y Arcane

#[test]
fn gravity_crushes_a_share_of_current_hp_by_magic_bosses_resist_pulls_and_holds() {
    // (the user, 2026-10-22) share = tier base + Magic x factor, of the CURRENT HP; a boss resists 90 %
    let mut rig = Rig::new(&[Vec3::new(700.0, 0.0, 0.0), Vec3::new(700.0, 300.0, 0.0), Vec3::new(700.0, -300.0, 0.0)]);
    rig.dummies.list[0].hp = 1000.0;
    rig.dummies.list[1].hp = 1000.0;
    rig.dummies.list[2] = Dummy { size: 4, hp: 1.0e5, hp_max: 1.0e5, ..rig.dummies.list[2] };
    rig.party.status_mut(rig.don).unwrap().donald.as_mut().unwrap().core.arcane.gain(0.0, 100.0, 4);
    rig.party.status_mut(rig.don).unwrap().donald.as_mut().unwrap().core.arcane.gain(0.0, 100.0, 4);
    let mag = rig.party.status(rig.don).unwrap().magic();
    let m = magic_fx::magic();
    let share = (m.gravity_share_base_1 + mag as f32 * m.gravity_share_per_magic).min(m.gravity_share_cap);
    rig.cast(SpellRef::Arcane(ArcaneLine::Gravity, 1), e1(), m.gravity_press_at + 0.2); // the crush lands as the ball presses
    let h = rig.dummies.hits_on(1)[0];
    assert!((h.damage - 1000.0 * share).abs() < 1.0, "{share} of 1000 (Magic {mag}): {}", h.damage);
    let n = rig.dummies.hits_on(2)[0];
    assert!(n.direction.y < -0.9, "pulled toward the target");
    assert_eq!(n.info.bad_status, "STUN");
    assert!((n.info.bad_time - 2.0).abs() < 1e-4, "held 2 s");
    let boss = rig.dummies.hits_on(3)[0];
    assert!((boss.damage - 1.0e5 * share * (1.0 - m.gravity_boss_resist)).abs() < 1.0, "boss: a tenth of the share, no other cap: {}", boss.damage);
    assert_eq!(boss.info.bad_status, "STUN", "a boss is still held");
}

#[test]
fn gravity_share_grows_with_tier_and_magic_is_capped_and_shrinks_with_the_bar() {
    let m = magic_fx::magic();
    // the tiers: a bigger share each
    let s = |lv: u8, mag: i32| magic_fx::gravity_share(magic_fx::gravity_base(lv), mag, false);
    assert!(s(1, 50) < s(2, 50) && s(2, 50) < s(3, 50));
    // Magic adds its factor per point
    assert!((s(1, 80) - s(1, 0) - 80.0 * m.gravity_share_per_magic).abs() < 1e-6);
    // capped
    assert!((s(3, 100_000) - m.gravity_share_cap).abs() < 1e-6);
    // a boss loses a tenth of it
    assert!((magic_fx::gravity_share(magic_fx::gravity_base(3), 50, true) - s(3, 50) * (1.0 - m.gravity_boss_resist)).abs() < 1e-6);
    // the CURRENT HP: a healthy big one loses the most, a repeat cast less as its bar shrinks
    let first = magic_fx::gravity_damage(10_000.0, magic_fx::gravity_base(2), 50, false);
    let second = magic_fx::gravity_damage(10_000.0 - first, magic_fx::gravity_base(2), 50, false);
    assert!(second < first && (second / first - (1.0 - s(2, 50))).abs() < 1e-4, "{first} then {second}");
    assert!(magic_fx::gravity_damage(0.0, magic_fx::gravity_base(1), 50, false) >= m.gravity_min_damage);
}

#[test]
fn gravity_ball_presses_down_and_flattens_in_sync_restores_after_the_hold_never_a_boss() {
    // (the user, 2026-10-22) "the gravity ball should squish down while also moving towards the floor, which is what
    // squishes the enemies": the enemy's height follows the ball's descending top; flat for the hold; back 0.5 s after it
    let m = magic_fx::magic();
    // the ball: its top comes down to a thin disc on the floor over the press
    let top0 = magic_fx::gravity_ball_top(1, m.gravity_press_at);
    let top_mid = magic_fx::gravity_ball_top(1, m.gravity_press_at + m.gravity_press_time * 0.6);
    let top_end = magic_fx::gravity_ball_top(1, m.gravity_press_at + m.gravity_press_time);
    assert!(top0 > top_mid && top_mid > top_end, "{top0} {top_mid} {top_end}");
    // (YFX) the whole ball sinks until its equator (the band) is on the floor: the top half of the pressed dome is left
    assert!((top_end - m.gravity_fx_diameter_1 * 100.0 * m.gravity_fx_floor_thin / 2.0).abs() < 1.0, "the pressed dome's top above the floor: {top_end}");

    let mut rig = Rig::new(&[Vec3::new(700.0, 0.0, 0.0), Vec3::new(700.0, 300.0, 0.0)]);
    rig.dummies.list[1].size = 4;
    rig.party.status_mut(rig.don).unwrap().donald.as_mut().unwrap().core.arcane.gain(0.0, 100.0, 4);
    rig.party.status_mut(rig.don).unwrap().donald.as_mut().unwrap().core.arcane.gain(0.0, 100.0, 4);
    // (DON-SPELLB gif) pressed until the effect's end (the GIF's length), not the row's hold
    let hold = m.gravity_fx_total - m.gravity_press_at;
    assert!(rig.begin(SpellRef::Arcane(ArcaneLine::Gravity, 1), e1()));
    let mut steps = 0;
    while rig.dummies.hits_on(1).is_empty() {
        rig.step();
        steps += 1;
        assert!(steps < 600, "Gravity never landed");
    }
    assert!(!rig.dummies.hits_on(2).is_empty(), "the boss is hit (damage and hold)");
    assert!(rig.party.don_squash(2).is_none(), "a boss is never flattened");
    // (YFX, the user 2026-10-23: "The enemys also need to be squeezed down from it in time with the ball squeezing") the
    // crush lands as the ball starts to press: still its own height
    let (_, h) = rig.party.don_squash(1).expect("pressed by the ball");
    assert!(h > 0.99, "the press has only begun: {h}");
    // in sync with the ball: the enemy's flattening is the ball's squish curve on the ball's clock (the ball's height share
    // 1 -> fx_floor_thin while the enemy's goes 1 -> flat_height, by the same press curve), whatever the enemy's height
    let mut seen_mid = false;
    for _ in 0..((m.gravity_press_time / DT) as usize) {
        rig.step();
        let (w, h) = rig.party.don_squash(1).unwrap();
        let q = rig.party.don_fx.squash.iter().find(|q| q.target == 1).unwrap();
        let t = m.gravity_press_at + q.age;
        let ball = 1.0 - (1.0 - m.gravity_fx_floor_thin) * magic_fx::gravity_ball_press(t);
        let ball_pressed = (1.0 - ball) / (1.0 - m.gravity_fx_floor_thin);
        // (YFX) its own flat share (flat_height, or the sunk ball's top for a body the ball was grown for)
        let flat = m.gravity_flat_height.min(100.0 * m.gravity_fx_diameter_1 * crate::donald::yfx::gravity_scale(q.lv, q.height) * m.gravity_fx_floor_thin / 2.0 / q.height);
        let enemy_pressed = (1.0 - h) / (1.0 - flat);
        assert!((ball_pressed - enemy_pressed).abs() < 1e-3, "in sync with the ball's squish: ball {ball_pressed} enemy {enemy_pressed}");
        if h < 0.9 && h > m.gravity_flat_height + 0.05 {
            seen_mid = true;
            assert!(w > 1.0, "wider as it flattens: {w}");
        }
    }
    assert!(seen_mid, "it flattened gradually with the ball");
    let pressed = { let q = rig.party.don_fx.squash.iter().find(|q| q.target == 1).unwrap(); m.gravity_flat_height.min(100.0 * m.gravity_fx_diameter_1 * crate::donald::yfx::gravity_scale(q.lv, q.height) * m.gravity_fx_floor_thin / 2.0 / q.height) }; // (YFX) its own flat share
    // any size: a small and a giant body follow the very same curve (generic, from no per-enemy table)
    // (YFX) the ball sinks to its equator: a body the ball was grown for is pressed to the sunk ball's top instead of
    // flat_height when that is lower (never left poking out), on the same curve
    for height in [90.0, 180.0, 600.0] {
        let q = magic_fx::Squash { target: 9, age: m.gravity_press_time * 0.5, hold: 2.0, height, lv: 2 };
        let d = 100.0 * m.gravity_fx_diameter_2 * crate::donald::yfx::gravity_scale(2, height);
        let flat = m.gravity_flat_height.min(d * m.gravity_fx_floor_thin / 2.0 / height);
        if height <= 180.0 {
            assert_eq!(flat, m.gravity_flat_height, "an ordinary body: flat_height");
        }
        let want = 1.0 - (1.0 - flat) * magic_fx::gravity_ball_press(m.gravity_press_at + m.gravity_press_time * 0.5);
        assert!((q.scale().1 - want).abs() < 1e-5, "{height} cm: {} vs {want}", q.scale().1);
        let end = magic_fx::Squash { age: m.gravity_press_time + 0.1, ..q };
        assert!(end.scale().1 * height <= magic_fx::gravity_ball_top(2, m.gravity_press_at + m.gravity_press_time) * crate::donald::yfx::gravity_scale(2, height) + 1.0, "{height} cm: under the sunk ball's top");
    }
    // flat for the rest of the hold and until just before the restore delay (DON-SPELLB: the sheet's, 0.2 s)
    rig.secs(hold - m.gravity_press_time + (m.gravity_restore_delay - 0.1).max(0.0));
    let (w, h) = rig.party.don_squash(1).unwrap();
    assert!((h - pressed).abs() < 1e-3 && w >= 1.0, "pressed as the ball left it (DON-SPELLB gif): {w} {h}");
    // springing back
    rig.secs(0.1 + m.gravity_restore_time * 0.5);
    let (_, h) = rig.party.don_squash(1).unwrap();
    assert!(h > pressed - 1e-3 && h <= 1.0, "springing back: {h}");
    // back to normal and dropped
    rig.secs(m.gravity_restore_time * 0.5 + 0.05);
    assert!(rig.party.don_squash(1).is_none(), "restored");
    assert!(rig.party.don_squashes().is_empty());
}

#[test]
fn meteor_counts_and_ultima_ignores_resistances() {
    for (lv, n) in [(1u8, 3usize), (2, 6), (3, 10)] {
        let mut rig = Rig::new(&[Vec3::new(700.0, 0.0, 0.0)]);
        for _ in 0..3 {
            rig.party.status_mut(rig.don).unwrap().donald.as_mut().unwrap().core.arcane.gain(0.0, 100.0, 4);
        }
        rig.cast(SpellRef::Arcane(ArcaneLine::Meteor, lv), e1(), 3.5);
        assert_eq!(rig.spawned(fx::METEOR[1]), n, "Meteor Lv {lv}");
        if lv == 3 {
            assert!(rig.dummies.hits_on(1).iter().all(|h| h.damage_kind == Kind::Blow), "Lv 3 knocks down");
        }
    }
    // Ultima on a target resisting everything 50 %: full damage
    let dmg = |resist: [i32; 8]| {
        let mut rig = Rig::new(&[Vec3::new(700.0, 0.0, 0.0)]);
        rig.dummies.list[0].resist = resist;
        rig.attune(Element::Fire, 3.0);
        for _ in 0..4 {
            rig.party.status_mut(rig.don).unwrap().donald.as_mut().unwrap().core.arcane.gain(0.0, 100.0, 4);
        }
        rig.cast(SpellRef::Arcane(ArcaneLine::Ultima, 1), e1(), 1.0);
        rig.dummies.damage_on(1)
    };
    let plain = dmg([0; 8]);
    assert!(plain > 0.0);
    assert!((dmg([50; 8]) - plain).abs() < 0.01, "Ultima ignores resistances");
}

#[test]
fn zettaflare_costs_its_row_and_empties_resonance_and_arcane_flow() {
    let mut rig = Rig::new(&[Vec3::new(700.0, 0.0, 0.0)]);
    {
        let st = rig.party.status_mut(rig.don).unwrap().donald.as_mut().unwrap();
        st.learned = DonLearned { tier: [1, 0, 0, 0, 0, 0], line2: [0; 6], arcane: [0, 1, 0, 0, 1], mp_steps: 0 };
        for _ in 0..4 {
            st.core.arcane.gain(0.0, 100.0, 4);
        }
        st.core.fight_full.insert(Element::Fire);
    }
    rig.attune(Element::Fire, 3.0);
    // (the coordinator, 2026-10-22) Zettaflare is priced from its row (200 x the base 1.5), no longer the whole bar
    rig.party.status_mut(rig.don).unwrap().mp = 1000;
    let need = rig.plan(SpellRef::Arcane(ArcaneLine::Zettaflare, 1)).unwrap().mp;
    assert!(need > 0);
    rig.cast(SpellRef::Arcane(ArcaneLine::Zettaflare, 1), e1(), 1.6);
    // (ZETTA) its presentation first: the burst at its end
    for _ in 0..(20.0 / DT) as usize {
        if rig.dummies.damage_on(1) > 0.0 {
            break;
        }
        rig.step();
    }
    assert!(rig.mp() <= 1000 - need, "its row's MP is spent: {} left of 1000, price {need}", rig.mp());
    let st = rig.party.status(rig.don).unwrap().donald.as_ref().unwrap();
    assert_eq!(st.core.arcane.charges(), 0);
    assert_eq!(st.core.resonance(Element::Fire), Some(0.0));
    assert!(rig.dummies.damage_on(1) > 0.0);
}

// ================================================================================================ Releases

#[test]
fn every_release_bursts_at_the_lock_target_for_free() {
    for e in Element::ALL {
        let mut rig = Rig::new(&[Vec3::new(700.0, 0.0, 0.0), Vec3::new(750.0, 200.0, 0.0)]);
        rig.party.member_mut(rig.don).unwrap().focus = crate::command::TargetRef::Enemy(TargetId(1));
        rig.attune(e, 3.0);
        let mp = rig.mp();
        {
            let st = rig.party.status_mut(rig.don).unwrap().donald.as_mut().unwrap();
            let (l, m) = (st.learned, st.abilities.mods());
            let other = if e == Element::Fire { Element::Water } else { Element::Fire };
            st.core.begin_attune(other, &l, &m, &data()).unwrap();
        }
        rig.secs(data().tune.attune_cast + 0.4); // (DON-KIT) the attune cast from the sheet (1.0 s since 2026-10-22)
        assert_eq!(rig.mp(), mp, "{e:?}: free");
        match e {
            Element::Light => assert!(rig.log().iter().any(|l| matches!(l, FxLog::Heal { .. })), "Sanctuary heals"),
            _ => assert!(!rig.dummies.hits_on(1).is_empty() && !rig.dummies.hits_on(2).is_empty(), "{e:?}: the burst round the target"),
        }
    }
    // Shatter breaks Frozen; Deluge wets; Cyclone gathers
    let mut rig = Rig::new(&[Vec3::new(700.0, 0.0, 0.0)]);
    rig.party.don.status.entry(1, Size::Small).frozen = 3.0;
    let mut fxw = std::mem::take(&mut rig.party.don_fx);
    let _ = &mut fxw;
    rig.party.don_fx = fxw;
    rig.party.member_mut(rig.don).unwrap().focus = crate::command::TargetRef::Enemy(TargetId(1));
    {
        let mut host = PartyHost { world: &rig.world, targets: &mut rig.dummies, audio: &mut rig.audio, effects: &mut rig.effects };
        cast::release(&mut rig.party, rig.don, Element::Blizzard, &mut host);
    }
    rig.idle(2);
    assert!(rig.reacted(Reaction::Shatter), "Release Blizzard: Shatter breaks Frozen");
}

#[test]
fn sanctuary_heals_the_party_and_makes_it_invincible_briefly() {
    let mut rig = Rig::new(&[]);
    {
        let s = rig.party.status_mut(rig.don).unwrap();
        s.hp = s.max_hp() / 2;
    }
    let hp = rig.party.status(rig.don).unwrap().hp;
    {
        let mut host = PartyHost { world: &rig.world, targets: &mut rig.dummies, audio: &mut rig.audio, effects: &mut rig.effects };
        cast::release(&mut rig.party, rig.don, Element::Light, &mut host);
    }
    rig.idle(1);
    assert!(rig.party.status(rig.don).unwrap().hp > hp);
    let mut hit = HitRecord::empty(TargetId::NONE);
    hit.damage = 20.0;
    assert!(!rig.party.receive_hit(rig.don, &hit).accepted, "invincible");
    rig.secs(2.0);
    assert!(rig.party.receive_hit(rig.don, &hit).accepted, "1.5 s only");
}

// ================================================================================================ the damage side

#[test]
fn conduct_chains_to_every_wet_enemy_in_range() {
    let mut rig = Rig::new(&[Vec3::new(600.0, 0.0, 0.0), Vec3::new(600.0, 800.0, 0.0), Vec3::new(600.0, -1200.0, 0.0), Vec3::new(600.0, 3000.0, 0.0)]);
    for id in 1..=4 {
        rig.party.don.status.entry(id, Size::Small).wet = 8.0;
    }
    // Banish? no: a Thunder hit; use Ball Lightning's hurl-free path: a Thunder hybrid would be two elements. Thunder
    // line 1 is Sora's actor: its Conduct chains through after_reactions too.
    rig.cast(SpellRef::Line1(Element::Thunder, 1), e1(), 1.5);
    assert!(rig.reacted(Reaction::Conduct));
    assert!(!rig.dummies.hits_on(2).is_empty() && !rig.dummies.hits_on(3).is_empty(), "chained within 15 m");
    assert!(rig.dummies.hits_on(4).is_empty(), "not beyond 15 m");
}

#[test]
fn wildfire_spreads_the_burn_and_burn_ticks_deal_damage() {
    let mut rig = Rig::new(&[Vec3::new(600.0, 0.0, 0.0), Vec3::new(600.0, 400.0, 0.0)]);
    rig.party.don.status.entry(1, Size::Small).burn = 6.0;
    rig.cast(SpellRef::Line2(Element::Aero, 1), CastTarget::Member(rig.don), 0.1);
    // an Aero hit on the Burned one: Aerial path through deliver; use the Release Aero burst (Cyclone)
    rig.party.member_mut(rig.don).unwrap().focus = crate::command::TargetRef::Enemy(TargetId(1));
    {
        let mut host = PartyHost { world: &rig.world, targets: &mut rig.dummies, audio: &mut rig.audio, effects: &mut rig.effects };
        cast::release(&mut rig.party, rig.don, Element::Aero, &mut host);
    }
    rig.idle(2);
    assert!(rig.reacted(Reaction::Wildfire));
    assert!(rig.status(2).is_some_and(|s| s.is_burned()), "the Burn spread");
    // the ticks: one a second, magic damage without a reaction
    rig.dummies.hits.clear();
    rig.secs(2.05);
    let ticks: Vec<_> = rig.dummies.hits_on(2).into_iter().filter(|h| h.attack_name == "Burn").collect();
    assert_eq!(ticks.len(), 2);
    assert!(ticks.iter().all(|t| t.damage >= 1.0 && t.info.victim_result == kh_core::host::ReactionResult::NoReaction));
}

#[test]
fn exposed_counts_for_everyones_hits() {
    let mut rig = Rig::new(&[Vec3::new(600.0, 0.0, 0.0)]);
    {
        let s = rig.party.don.status.entry(1, Size::Small);
        s.reveal = 8.0;
        s.exposed = true;
    }
    let exposed = rig.party.don_exposed();
    assert_eq!(exposed, vec![(1, 1.25)]);
    let mut wrapped = magic_fx::PartyFx { inner: &mut rig.dummies, mults: &exposed, seen: Vec::new(), team: 1.0 };
    let mut hit = HitRecord::empty(TargetId(1));
    hit.damage = 10.0;
    hit.damage_kind = Kind::Small;
    wrapped.apply_hit(&hit);
    let mut own = hit;
    own.flags |= DON_HIT_FLAG;
    wrapped.apply_hit(&own);
    assert_eq!(wrapped.seen.len(), 1);
    assert!((rig.dummies.hits[0].damage - 12.5).abs() < 1e-4, "a friend's hit x1.25");
    assert!((rig.dummies.hits[1].damage - 10.0).abs() < 1e-4, "Donald's own hit already carries it");
    // a friend's hit on a Revealed enemy turns it into Exposed (R8)
    let mut rig = Rig::new(&[Vec3::new(600.0, 0.0, 0.0)]);
    rig.party.don.status.entry(1, Size::Small).reveal = 8.0;
    let m = rig.party.don_other_hit(1, Size::Small, kh_core::enums::ETresDamageAttribute::Physical);
    assert!((m - 1.25).abs() < 1e-4 && rig.status(1).unwrap().exposed);
}

#[test]
fn the_statuses_show_their_effects_while_they_last() {
    let mut rig = Rig::new(&[Vec3::new(600.0, 0.0, 0.0)]);
    rig.party.don.status.entry(1, Size::Small).wet = 1.0;
    rig.idle(2);
    assert_eq!(rig.spawned(fx::WET), 1);
    let h = rig.effects.spawned.iter().find(|e| e.1 == fx::WET).unwrap().0;
    rig.secs(1.2);
    assert!(rig.effects.stopped.contains(&h), "stopped when the Wet ends");
}

#[test]
fn every_spell_row_casts_and_puts_something_out() {
    let d = data();
    for row in d.spells.iter() {
        let Some(spell) = SpellRef::from_id(row.id) else { panic!("{}", row.id) };
        if matches!(spell, SpellRef::Release(_)) {
            continue;
        }
        let mut rig = Rig::new(&[Vec3::new(600.0, 0.0, 0.0), Vec3::new(650.0, 150.0, 0.0)]);
        rig.give(DonAbility::DualAttunement);
        match spell {
            SpellRef::Hybrid(a, b) => {
                rig.attune(a, 3.0);
                rig.attune(b, 3.0);
            }
            SpellRef::Arcane(..) => {
                rig.attune(Element::Fire, 3.0);
                rig.party.status_mut(rig.don).unwrap().mp = 1000; // (Zettaflare's row price is above his bar)
                let st = rig.party.status_mut(rig.don).unwrap().donald.as_mut().unwrap();
                for _ in 0..4 {
                    st.core.arcane.gain(0.0, 100.0, 4);
                }
                for e in Element::ALL {
                    st.core.fight_full.insert(e);
                }
            }
            s => rig.attune(s.element().unwrap(), 3.0),
        }
        let target = if spell.ally_target() { CastTarget::Member(rig.don) } else { e1() };
        let spawned_before = rig.effects.spawned.len();
        rig.cast(spell, target, 4.0);
        // (ZETTA) Zettaflare plays the KH3 cutscene's presentation first (donald/zettaflare.rs): its hit comes at its end
        if matches!(spell, SpellRef::Arcane(ArcaneLine::Zettaflare, _)) {
            for _ in 0..(20.0 / DT) as usize {
                if !rig.dummies.hits.is_empty() {
                    break;
                }
                rig.step();
            }
        }
        assert!(rig.effects.spawned.len() > spawned_before + 1, "{}: no effect", row.id);
        if !spell.ally_target() && !matches!(spell, SpellRef::Line1(Element::Light, _)) {
            assert!(!rig.dummies.hits.is_empty(), "{}: no hit", row.id);
        }
    }
}

// ================================================================================================ the pack harness

/// EVERY EFFECT A SPELL SPAWNS IS IN A PACK (the main pack, the party pack, or this stream's new pack folder), and
/// every effect with an effect-attached sound in KH3 has it in an audio pack (or is listed for the --party-extra alias).
/// The report: build/sora_tmp/party/DON-MAGIC/effects_harness.txt.
#[test]
fn effects_are_packed() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let live = root.join("build").join("sora_pack");
    if !live.join("vfx").join("effects").is_dir() {
        eprintln!("no pack at {}: harness skipped", live.display());
        return;
    }
    // this stream's folder: in the live tree build/sora_tmp/party/DON-MAGIC, in its work copy the copy's parent
    let stream = root.join("build").join("sora_tmp").join("party").join("DON-MAGIC");
    let stream = if stream.is_dir() { stream } else { root.join("..") };
    let mine = stream.join("pack").join("vfx").join("effects");
    // (DON-SPELLB pack) part B's own effects (the authored ribbons ...): its folder in the live tree, else its ws copy's parent
    let spellb = root.join("build").join("sora_tmp").join("party").join("DON-SPELLB");
    let spellb = if spellb.is_dir() { spellb } else { root.join("..") };
    // (AUD) stream AUD's authored / strobe-free effects (build/sora_tmp/party/AUD/pack, staged for the ER pack by its MERGE_NOTE)
    let aud = root.join("build").join("sora_tmp").join("party").join("AUD").join("pack").join("vfx").join("effects");
    let dirs = [live.join("vfx").join("effects"), live.join("party").join("vfx").join("effects"), mine, spellb.join("pack").join("vfx").join("effects"), aud];
    let mut have = std::collections::BTreeMap::new();
    for d in &dirs {
        if let Ok(rd) = std::fs::read_dir(d) {
            for f in rd.flatten() {
                let n = f.file_name().to_string_lossy().to_lowercase();
                if let Some(stem) = n.strip_suffix(".json") {
                    have.entry(stem.to_string()).or_insert_with(|| d.display().to_string());
                }
            }
        }
    }
    let mut report = String::new();
    let mut missing = Vec::new();
    for name in fx::all() {
        match have.get(&name.to_lowercase()) {
            Some(d) => report.push_str(&format!("ok      {name:<40} {d}\n")),
            None => {
                report.push_str(&format!("MISSING {name}\n"));
                missing.push(name);
            }
        }
    }
    // THE SOUNDS: the audio packs' effect triggers ("Effect <name> starts" with a sound) of the main pack, the party pack
    // and this stream's party-extra pack
    let mut sounding = std::collections::BTreeSet::new();
    for m in [live.join("audio").join("manifest.json"), live.join("party").join("audio").join("manifest.json"), stream.join("pack").join("party").join("audio").join("manifest.json")] {
        let Ok(text) = std::fs::read_to_string(&m) else { continue };
        // (pretty-printed or not: "trigger": "Effect <name> starts", ... "sounds": [ { ...)
        for chunk in text.split("\"trigger\":").skip(1) {
            let Some(rest) = chunk.trim_start().strip_prefix("\"Effect ") else { continue };
            let name = rest.split([' ', '"']).next().unwrap_or("").to_lowercase();
            let plays = chunk.split("\"sounds\":").nth(1).is_some_and(|r| r.trim_start().strip_prefix('[').is_some_and(|r| r.trim_start().starts_with('{')));
            if plays {
                sounding.insert(name);
            }
        }
    }
    report.push_str("\n-- sounds --\n");
    let mut silent = Vec::new();
    for name in fx::all() {
        let own = sounding.contains(&name.to_lowercase());
        let extra = fx::extra_se(name).and_then(|s| s.strip_prefix("effect:")).filter(|s| sounding.contains(&s.to_lowercase()));
        let line = match (own, extra) {
            (true, _) => "own SE (pack trigger)".to_string(),
            (false, Some(s)) => format!("played as effect:{s} (magic_fx extra_se)"),
            _ if fx::silent_by_design(name) => "silent in KH3 too (silent_by_design)".to_string(),
            // (YFX) Gravity's ball: its own layered KH3 sounds on its clock (yfx.rs gravity), played by path
            _ if crate::donald::yfx::own_sounds(name).is_some() => "its own layered sounds (yfx.rs)".to_string(),
            _ => {
                silent.push(name);
                "NO SOUND".to_string()
            }
        };
        report.push_str(&format!("{name:<40} {line}\n"));
    }
    let out = stream.join("effects_harness.txt");
    let _ = std::fs::write(&out, &report);
    assert!(missing.is_empty(), "effects not in any pack: {missing:?}\n{report}");
    assert!(silent.is_empty(), "effects without a sound: {silent:?}\n{report}");
}

#[test]
fn the_sheet_keys_all_set() {
    let mut t = magic_fx::DON_MAGIC_PARAMS;
    for k in magic_fx::magic_keys() {
        assert!(magic_fx::set_magic(&mut t, k, 1.0), "{k}");
    }
    assert!(!magic_fx::set_magic(&mut t, "nope.nope", 1.0));
    let _ = Apply::Wet;
}

#[test]
fn a_blind_enemy_attacks_the_spot_it_last_saw_its_target() {
    // the user, 2026-10-22: "blind should make enemies attack the position they last saw you before they were blinded"
    let mut rig = Rig::new(&[Vec3::new(600.0, 0.0, 0.0)]);
    let at_blinding = Vec3::new(100.0, 0.0, 0.0);
    rig.party.don_blind_track(1, at_blinding);
    assert_eq!(rig.party.don_blind_point(1), None, "it sees: no spot");
    rig.party.don.status.entry(1, Size::Small).blind = 5.0;
    rig.party.don_blind_track(1, at_blinding);
    // the target walks off: the Blind enemy keeps the spot of the moment it was blinded
    rig.party.don_blind_track(1, Vec3::new(900.0, 400.0, 0.0));
    assert_eq!(rig.party.don_blind_point(1), Some(at_blinding));
    assert!(magic_fx::blind_swing_hits(at_blinding, Vec3::new(150.0, 30.0, 0.0)), "whoever stands in that spot is hit");
    assert!(!magic_fx::blind_swing_hits(at_blinding, Vec3::new(900.0, 400.0, 0.0)), "the real target, gone from it, is not");
    // Steam Burst blinds through the reaction rule; the record ends with the Blind
    rig.secs(5.5);
    assert_eq!(rig.party.don_blind_point(1), None);
    assert!(rig.party.don_fx.blind_at.is_empty(), "the record ended with the Blind");
}

#[test]
fn enemies_run_slower_when_frozen_chilled_or_in_the_blizzard_aura() {
    let mut rig = Rig::new(&[Vec3::new(600.0, 0.0, 0.0)]);
    assert_eq!(rig.party.don_enemy_speed(1, Vec3::new(600.0, 0.0, 0.0)), 1.0);
    {
        let s = rig.party.don.status.entry(1, Size::Small);
        s.chill = 2;
        s.chill_left = 4.0;
    }
    assert!((rig.party.don_enemy_speed(1, Vec3::new(600.0, 0.0, 0.0)) - 0.88).abs() < 1e-4, "2 Chill stacks");
    rig.party.don.status.entry(1, Size::Small).frozen = 2.0;
    assert_eq!(rig.party.don_enemy_speed(1, Vec3::new(600.0, 0.0, 0.0)), 0.0, "Frozen: it cannot act");
}

#[test]
fn the_r_recolours_are_asked_of_the_effect_side() {
    // (HYB) the hybrids' looks are authored in their own colours (donald/hybrid_look.rs, HYB make_hyb_fx.js): Plasma's ball
    // is its violet plasma orb, Prism Frost's fan carries the whole spectrum in its shards; the KH3 strike layered on
    // Plasma's detonation is recoloured violet by the look itself
    let mut rig = dual_rig(&[Vec3::new(600.0, 0.0, 0.0)], Element::Fire, Element::Thunder);
    rig.cast(SpellRef::Hybrid(Element::Fire, Element::Thunder), e1(), 0.1);
    assert_eq!(rig.spawned("vd_hyb_pl_orb"), 1);
    let mut rig = dual_rig(&[Vec3::new(600.0, 0.0, 0.0)], Element::Blizzard, Element::Light);
    rig.cast(SpellRef::Hybrid(Element::Blizzard, Element::Light), e1(), 0.1);
    assert_eq!(rig.spawned("vd_hyb_pf_cone"), 1);
    // the Reveal mark in gold; DON-KIT's tags
    assert_eq!(fx::tint_of(None, "vp_ath_tgt0"), Some(fx::GOLD)); // (DON-SPELLB) the Reveal look is authored gold
    assert_eq!(fx::kit_tint("DON_WARD"), Some(fx::GOLD));
    assert_eq!(fx::kit_tint("glide"), Some(fx::ICE_BLUE));
    assert_eq!(fx::kit_tint("blink"), Some(fx::YELLOW));
    assert_eq!(fx::kit_tint("DON_SLAM_AERO"), Some(fx::GREEN));
    assert_eq!(fx::tint_of(Some(SpellRef::Line1(Element::Fire, 3)), "vm_fir_blt3"), None, "Firaga keeps its own colours");
}

#[test]
fn spells_cost_their_fixed_mp_and_y_spells_a_share() {
    let rig = Rig::new(&[]);
    let d = data();
    let scale = d.tune.cost_scale;
    let max = rig.party.status(rig.don).unwrap().max_mp();
    let p = rig.plan(SpellRef::Line2(Element::Fire, 1)).unwrap();
    let row = SpellRef::Line2(Element::Fire, 1).row(&d).copied().unwrap();
    assert_eq!(p.mp, ((row.cost_free * scale) - 1e-4).ceil().max(1.0) as i32, "fixed MP like Sora's (unattuned)");
    let y = rig.plan(SpellRef::Arcane(ArcaneLine::Gravity, 1));
    // Gravity needs 2 charges: refused for charges, not priced from a share here; the Y share is the core's rule
    assert!(matches!(y, Err(Refusal::NotEnoughCharges { .. })));
    assert!(max > 0);
}

// (DON-MAGIC part A) the magic Donald shares with Sora (its rig is this module's)
#[path = "sora_magic_tests.rs"]
mod sora_magic_tests;
#[path = "hyb_tests.rs"]
mod hyb_tests; // (HYB) the hybrids' look

/// (KITFX) the user 2026-10-23: "Flare currently has no sound effects, fix that": the launch (Firaga's projectile-spawn SE),
/// the flight hum (KH3's flare-shot loop, looping) stopped with a short fade at the detonation, and a detonation SE that is a
/// one-shot (its old stand-in named a LOOPING asset, refused by the audio engine as a one-shot).
#[test]
fn kitfx_flare_launches_hums_and_detonates_audibly() {
    let mut rig = Rig::new(&[Vec3::new(700.0, 0.0, 0.0)]);
    rig.cast(SpellRef::Line2(Element::Fire, 1), e1(), 3.0);
    assert!(rig.audio.plays.iter().any(|p| p.1 == fx::FLARE_LAUNCH_SE && !p.2), "the launch");
    let hum = rig.audio.plays.iter().find(|p| p.1 == fx::FLARE_HUM_SE).copied().expect("the flight hum");
    assert!(hum.2, "the hum loops");
    assert_eq!(rig.audio.stops.iter().filter(|s| s.0 == hum.0).count(), 1, "the hum stops once, at the detonation");
    assert!(rig.party.don_fx.kitfx_hums.is_empty());
    assert_eq!(fx::extra_se(fx::FLARE_BOOM), Some("effect:vm_fir_han_070"), "the detonation: a one-shot Fire burst");
    assert_eq!(fx::extra_se(fx::FLARE_AFTER), Some("effect:vm_fir_han_040"));
}

/// (KITFX) THE ROOT CAUSE OF THE SILENT FLARE, CHECKED FOR EVERY SPELL EFFECT: a stand-in sound `extra_se` plays as a one-shot,
/// and the audio engine refuses a one-shot of a LOOPING asset ("a looping asset from an instant notify is refused"), so an
/// `effect:<KH3 effect>` whose SeAttach is a loop is silent. Every spell effect's extra sound must be a one-shot. Read from the
/// test level's pack (build/play/donpack), else the live pack; skipped, saying so, without either.
#[test]
fn kitfx_no_extra_sound_is_a_looping_asset() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("build");
    let pack = [root.join("play").join("donpack"), root.join("sora_pack")].into_iter().find(|p| p.join("audio").join("manifest.json").is_file());
    let Some(pack) = pack else {
        eprintln!("no audio pack: skipped");
        return;
    };
    let mut first: std::collections::BTreeMap<String, String> = Default::default();
    let mut looping: std::collections::BTreeMap<String, bool> = Default::default();
    for m in [pack.join("audio").join("manifest.json"), pack.join("party").join("audio").join("manifest.json")] {
        let Ok(text) = std::fs::read_to_string(&m) else { continue };
        let j: serde_json::Value = serde_json::from_str(&text).expect("an audio manifest");
        for t in j["triggers"].as_array().into_iter().flatten() {
            let Some(name) = t["trigger"].as_str().and_then(|s| s.strip_prefix("Effect ")).and_then(|s| s.strip_suffix(" starts")) else { continue };
            if let Some(a) = t["sounds"][0]["asset"].as_str() {
                first.entry(name.to_lowercase()).or_insert_with(|| a.to_string());
            }
        }
        for (k, a) in j["sound_assets"].as_object().into_iter().flatten() {
            looping.entry(k.clone()).or_insert(a["looping"].as_bool().unwrap_or(false));
        }
    }
    let mut bad = Vec::new();
    for name in fx::all() {
        let Some(se) = fx::extra_se(name) else { continue };
        let asset = match se.strip_prefix("effect:") {
            Some(e) => first.get(&e.to_lowercase()).cloned(),
            None => looping.keys().find(|k| k.ends_with(&format!("/{se}"))).cloned(),
        };
        match asset {
            Some(a) if looping.get(&a).copied().unwrap_or(false) => bad.push(format!("{name}: {se} -> {a} LOOPS")),
            Some(_) => {}
            None => bad.push(format!("{name}: {se} resolves to no packed sound")),
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}
