//! (DON-SPELLB) Part B's presentation and polish: Aero Barrier's look over the whole duration and its fade, the cast poses
//! and their timing, the ultimates through ZETTA's presentation, the Releases' gesture, Gravity's recovery.
#![allow(dead_code, unused_imports)]
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
    /// (status looks) its velocity (cm/s): a Wet one moving splashes
    vel: Vec3,
}

#[derive(Default)]
struct Dummies {
    list: Vec<Dummy>,
    hits: Vec<HitRecord>,
}

impl Dummies {
    fn info_of(&self, d: &Dummy) -> TargetInfo {
        let flags = TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY;
        TargetInfo { id: d.id, position: d.pos, center: d.pos + Vec3::Z * 90.0, velocity: d.vel, yaw: 180.0, radius: 50.0, half_height: 90.0, flags: TargetFlags(flags), hp: d.hp, hp_max: d.hp_max }
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
    /// (status looks) the stops that remove at once, and the per-axis rescales
    stopped_now: Vec<u32>,
    transformed: Vec<(u32, Vec3)>,
    tinted: Vec<(u32, kh_core::host::Tint)>,
    placed: Vec<(u32, Vec3)>,
}

impl kh_core::host::Effects for RecEffects {
    fn spawn(&mut self, handle: EffectHandle, spawn: &EffectSpawn) {
        self.spawned.push((handle.0, spawn.name, spawn.at, spawn.persistent));
    }
    fn stop(&mut self, handle: EffectHandle, immediate: bool) {
        self.stopped.push(handle.0);
        if immediate {
            self.stopped_now.push(handle.0);
        }
    }
    fn transform(&mut self, handle: EffectHandle, _at: Vec3, _rotation: Vec3, scale: Vec3) {
        self.transformed.push((handle.0, scale));
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
}

impl kh_core::host::Audio for RecAudio {
    fn play(&mut self, _handle: kh_core::host::SoundHandle, _play: &kh_core::host::SoundPlay) {}
    fn stop(&mut self, _handle: kh_core::host::SoundHandle, _fade_out: f32) {}
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
        let list = enemies.iter().enumerate().map(|(i, p)| Dummy { id: TargetId(i as u32 + 1), pos: *p, size: 1, hp: 1.0e6, hp_max: 1.0e6, resist: [0; 8], vel: Vec3::ZERO }).collect();
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

// ================================================================================================ DON-SPELLB

#[test]
fn aero_barrier_wears_the_ribbons_the_whole_duration_then_fades_them() {
    let mut rig = Rig::new(&[Vec3::new(3000.0, 0.0, 0.0)]);
    rig.cast(SpellRef::Line2(Element::Aero, 1), CastTarget::Member(rig.don), 0.1);
    let look: Vec<_> = rig.effects.spawned.iter().filter(|e| e.1 == fx::BARRIER_LOOK[0]).cloned().collect();
    assert_eq!(look.len(), 1, "one set of ribbons");
    assert!(look[0].3, "persistent: it stays until stopped");
    let h = look[0].0;
    assert!(!rig.effects.tinted.iter().any(|t| t.0 == h), "its own authored colours (no tint)");
    assert!(matches!(look[0].2, Attach::Party { .. }), "on the wearer");
    // the whole duration (20 s at Lv 1): still on at 19 s
    rig.secs(19.0);
    assert!(!rig.effects.stopped.contains(&h), "still on near the end");
    rig.secs(1.5);
    assert!(rig.effects.stopped.contains(&h), "stopped (to fade out) when it ends");
    assert!(super::allyfx::barrier_of(&rig.party, rig.don).is_none());
}

#[test]
fn aero_barrier_recast_renews_and_a_new_level_swaps_the_ribbons() {
    let mut rig = Rig::new(&[Vec3::new(3000.0, 0.0, 0.0)]);
    rig.cast(SpellRef::Line2(Element::Aero, 1), CastTarget::Member(rig.don), 0.1);
    rig.secs(1.0);
    rig.cast(SpellRef::Line2(Element::Aero, 1), CastTarget::Member(rig.don), 0.1);
    assert_eq!(rig.spawned(fx::BARRIER_LOOK[0]), 1, "the same level keeps its ribbons");
    rig.cast(SpellRef::Line2(Element::Aero, 3), CastTarget::Member(rig.don), 0.1);
    assert_eq!(rig.spawned(fx::BARRIER_LOOK[2]), 1, "Lv 3's denser ribbons");
    let first = rig.effects.spawned.iter().find(|e| e.1 == fx::BARRIER_LOOK[0]).unwrap().0;
    assert!(rig.effects.stopped.contains(&first), "the old ribbons fade");
    assert_eq!(rig.spawned(fx::BARRIER_GUST), 3, "the gust at each cast");
    assert_eq!(rig.spawned(fx::BARRIER_CAST), 0, "never his KH3 Aero tornado");
}

#[test]
fn aero_barrier_ribbons_fade_when_the_wearer_is_ko() {
    let mut rig = Rig::new(&[Vec3::new(3000.0, 0.0, 0.0)]);
    rig.cast(SpellRef::Line2(Element::Aero, 2), CastTarget::Member(rig.don), 0.1);
    let h = rig.effects.spawned.iter().find(|e| e.1 == fx::BARRIER_LOOK[1]).unwrap().0;
    rig.party.status_mut(rig.don).unwrap().hp = 0;
    rig.secs(0.2);
    assert!(rig.effects.stopped.contains(&h), "a KO ends it (fading)");
}

#[test]
fn aero_barrier_size_follows_the_wearer() {
    let m = magic_fx::magic();
    let rig = Rig::new(&[]);
    assert!((super::allyfx::barrier_scale(&rig.party, rig.don, false) - m.barrier_fx_scale_donald).abs() < 1e-6);
    assert!((super::allyfx::barrier_scale(&rig.party, SORA_MEMBER, false) - 1.0).abs() < 1e-6);
}

#[test]
fn one_worn_spell_per_member_each_replaces_the_other() {
    let mut rig = Rig::new(&[Vec3::new(3000.0, 0.0, 0.0)]);
    rig.cast(SpellRef::Line2(Element::Aero, 1), CastTarget::Member(rig.don), 0.1);
    let ribbons = rig.effects.spawned.iter().find(|e| e.1 == fx::BARRIER_LOOK[0]).unwrap().0;
    assert_eq!(super::allyfx::worn_of(&rig.party, rig.don), Some(SpellRef::Line2(Element::Aero, 1)));
    // Ball Lightning replaces the barrier: the ribbons fade, the barrier's cut is gone
    rig.cast(SpellRef::Line2(Element::Thunder, 1), CastTarget::Member(rig.don), 0.1);
    assert!(rig.effects.stopped.contains(&ribbons), "the ribbons fade out");
    assert!(super::allyfx::barrier_of(&rig.party, rig.don).is_none());
    assert!(super::allyfx::orbs_of(&rig.party, rig.don).is_some_and(|o| o.active));
    assert!((super::allyfx::damage_mult(&rig.party, rig.don) - 1.0).abs() < 1e-6, "no cut any more");
    let cores: Vec<u32> = rig.effects.spawned.iter().filter(|e| e.1 == fx::ORB_CORE).map(|e| e.0).collect();
    assert!(!cores.is_empty());
    // and the barrier replaces Ball Lightning: its balls dissolve
    rig.cast(SpellRef::Line2(Element::Aero, 2), CastTarget::Member(rig.don), 0.1);
    assert!(cores.iter().all(|c| rig.effects.stopped.contains(c)), "every ball dissolves");
    assert!(super::allyfx::orbs_of(&rig.party, rig.don).is_none());
    assert_eq!(super::allyfx::worn_of(&rig.party, rig.don), Some(SpellRef::Line2(Element::Aero, 2)));
    // another member is not touched
    rig.cast(SpellRef::Line2(Element::Thunder, 1), CastTarget::Member(SORA_MEMBER), 0.1);
    assert!(super::allyfx::barrier_of(&rig.party, rig.don).is_some(), "Donald keeps his barrier");
}

#[test]
fn ultimates_and_ultima_are_refused_in_the_air() {
    assert!(cast::needs_ground(SpellRef::Ultimate(Element::Fire)));
    assert!(cast::needs_ground(SpellRef::Arcane(ArcaneLine::Ultima, 1)));
    assert!(!cast::needs_ground(SpellRef::Line2(Element::Fire, 1)));
    assert!(!cast::needs_ground(SpellRef::Arcane(ArcaneLine::Gravity, 1)));
    let mut rig = Rig::new(&[Vec3::new(700.0, 0.0, 0.0)]);
    rig.attune(Element::Fire, 3.0);
    // in the air: refused, nothing spent
    {
        let p = rig.party.member_mut(rig.don).unwrap();
        p.cmc.location.z += 300.0;
        p.cmc.set_falling();
    }
    let before = rig.mp();
    assert!(!rig.begin(SpellRef::Ultimate(Element::Fire), e1()), "no ultimate in the air");
    assert_eq!(rig.mp(), before, "nothing spent");
    assert_eq!(cast::ground_refusal(rig.party.member(rig.don).unwrap(), SpellRef::Ultimate(Element::Fire)), Some(super::combat::Refusal::NeedsGround));
    // back on the ground: cast
    rig.secs(2.0);
    assert!(rig.party.member(rig.don).unwrap().on_ground());
    rig.attune(Element::Fire, 3.0); // (Resonance drains out of combat)
    assert!(rig.begin(SpellRef::Ultimate(Element::Fire), e1()), "on the ground it casts");
}

/// (DON-SPELLB) the user, 2026-10-22: "Donald's line 2 magic needs to wait for him to actually cast the magic with his
/// animation before happening": for every part-B spell cast through `begin` (line 2, the hybrids, the Y magic but the
/// ultimate-style ones), nothing but the wand's charge light appears before the release, the release comes at the clip's
/// own release frame, and the spell appears at the release.
#[test]
fn every_part_b_spell_happens_at_its_clips_release_and_not_before() {
    let d = data();
    for spell in super::spell_look::part_b() {
        if spell.is_ultimate() || matches!(spell, SpellRef::Release(_) | SpellRef::Arcane(ArcaneLine::Ultima | ArcaneLine::Zettaflare, _)) {
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
                rig.party.status_mut(rig.don).unwrap().mp = 1000;
                let st = rig.party.status_mut(rig.don).unwrap().donald.as_mut().unwrap();
                for _ in 0..4 {
                    st.core.arcane.gain(0.0, 100.0, 4);
                }
            }
            s => rig.attune(s.element().unwrap(), 3.0),
        }
        let target = if spell.ally_target() { CastTarget::Member(rig.don) } else { e1() };
        let look = super::spell_look::look_of(spell).unwrap();
        let charge: Vec<&str> = look.charge.iter().map(|p| p.name).collect();
        let before = rig.effects.spawned.len();
        assert!(rig.begin(spell, target), "{spell:?}");
        let shot_at = rig.party.member(rig.don).unwrap().don_cast.run.unwrap().shot_at;
        let row = spell.row(&d).copied().unwrap();
        assert!((shot_at - row.cast.max(0.05)).abs() < 1e-4, "{spell:?}: the release at the row's cast time");
        // up to just before the release: only the charge light
        let mut clip_at_release = None;
        while rig.party.member(rig.don).unwrap().don_cast.run.is_some_and(|r| !r.shot && r.t + DT < r.shot_at) {
            rig.step();
            let early: Vec<&str> = rig.effects.spawned[before..].iter().map(|e| e.1).filter(|n| !charge.contains(n)).collect();
            assert!(early.is_empty(), "{spell:?}: {early:?} before the release");
            let p = rig.party.member(rig.don).unwrap();
            clip_at_release = Some(p.anim.time);
        }
        rig.step();
        rig.step();
        let after: Vec<&str> = rig.effects.spawned[before..].iter().map(|e| e.1).filter(|n| !charge.contains(n)).collect();
        assert!(!after.is_empty(), "{spell:?}: something at the release");
        // the clip's pose at the release is its own release frame (within a step at its speed)
        let air = false;
        let clip_shot = if air { 0.0 } else { look.shot };
        let t = clip_at_release.unwrap_or(0.0);
        assert!((t - clip_shot).abs() < 0.08, "{spell:?}: clip time {t} at the release, its release frame {clip_shot}");
    }
}

/// (DON-SPELLB) every effect the presentation and the payloads spawn is in a pack (KH3 effects packed by DON-SPELLB, the
/// main / party packs) and its materials are in the party shading table of the pack given (KH_SPELLB_PACK, else donpack).
#[test]
fn spellb_effects_are_packed_and_drawable() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let pack = std::env::var("KH_SPELLB_PACK").map(std::path::PathBuf::from).unwrap_or_else(|_| root.join("build").join("play").join("donpack"));
    if !pack.join("party").join("vfx").join("effects").is_dir() {
        eprintln!("no pack at {}: skipped", pack.display());
        return;
    }
    let mut names = super::spell_look::all_names();
    names.extend(super::spell_payload::all_names());
    names.extend(fx::BARRIER_LOOK);
    names.sort_unstable();
    names.dedup();
    let shading: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(pack.join("party/vfx/shaders/shading.json")).unwrap_or_default()).unwrap_or(serde_json::Value::Null);
    let mut missing = Vec::new();
    for n in &names {
        let f = [pack.join("vfx/effects").join(format!("{n}.json")), pack.join("party/vfx/effects").join(format!("{n}.json"))].into_iter().find(|p| p.is_file());
        let Some(f) = f else {
            missing.push(format!("{n}: no effect file"));
            continue;
        };
        let e: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        for m in e["files"]["materials"].as_array().into_iter().flatten().filter_map(|m| m.as_str()) {
            let id = m.trim_start_matches("materials/").trim_end_matches(".json");
            if shading.get("materials").and_then(|t| t.get(id)).is_none() {
                missing.push(format!("{n}: material {id} has no shading row"));
            }
        }
    }
    assert!(missing.is_empty(), "{missing:#?}");
}

/// (the user, 2026-10-22: damaging holy magic must use his attack quotes) no part-B spell picks a member / heal line: every
/// voice they set or queue is one of his attack shouts 101-103.
#[test]
fn part_b_spells_use_his_attack_shouts_never_the_heal_lines() {
    for spell in super::spell_look::part_b() {
        if matches!(spell, SpellRef::Release(_)) {
            continue;
        }
        let mut rig = Rig::new(&[Vec3::new(600.0, 0.0, 0.0)]);
        rig.give(DonAbility::DualAttunement);
        match spell {
            SpellRef::Hybrid(a, b) => {
                rig.attune(a, 3.0);
                rig.attune(b, 3.0);
            }
            SpellRef::Arcane(..) => {
                rig.attune(Element::Fire, 3.0);
                rig.party.status_mut(rig.don).unwrap().mp = 1000;
                let st = rig.party.status_mut(rig.don).unwrap().donald.as_mut().unwrap();
                for _ in 0..4 {
                    st.core.arcane.gain(0.0, 100.0, 4);
                }
            }
            s => rig.attune(s.element().unwrap(), 3.0),
        }
        let target = if spell.ally_target() { CastTarget::Member(rig.don) } else { e1() };
        rig.audio.voice_numbers.clear();
        assert!(rig.begin(spell, target), "{spell:?}");
        let queued: Vec<i32> = rig.party.member(rig.don).unwrap().voices.iter().map(|v| v.1).collect();
        let lines: Vec<i32> = rig.audio.voice_numbers.iter().copied().chain(queued).collect();
        assert!(lines.iter().all(|n| (101..=103).contains(n)), "{spell:?}: {lines:?}");
        if !spell.is_ultimate() && !matches!(spell, SpellRef::Arcane(ArcaneLine::Ultima, _)) {
            assert!(!lines.is_empty(), "{spell:?}: a cast line");
        }
    }
}

// ================================================================================================ status looks (round 2)

fn status_handle(rig: &Rig, name: &str) -> Option<u32> {
    rig.effects.spawned.iter().rev().find(|e| e.1 == name).map(|e| e.0)
}

/// (the user, 2026-10-23: "make actual visual effects that show up on enemies for things like wet and all of that stuff")
#[test]
fn every_status_wears_its_own_look_on_the_enemy_while_it_lasts() {
    use super::status::TargetStatus;
    let cases: [(&str, fn(&mut TargetStatus)); 9] = [
        (fx::WET, |s| s.wet = 2.0),
        (fx::CHILL_STACKS[0], |s| (s.chill, s.chill_left) = (1, 2.0)),
        (fx::CHILL_STACKS[3], |s| (s.chill, s.chill_left) = (4, 2.0)),
        (fx::FROZEN, |s| s.frozen = 2.0),
        (fx::BURN, |s| s.burn = 2.0),
        (fx::REVEAL, |s| s.reveal = 2.0),
        (fx::EXPOSED, |s| (s.reveal, s.exposed) = (2.0, true)),
        (fx::BLIND, |s| s.blind = 2.0),
        (fx::SLOW, |s| s.big_slow = 2.0),
    ];
    for (name, set) in cases {
        let mut rig = Rig::new(&[Vec3::new(600.0, 0.0, 0.0)]);
        set(rig.party.don.status.entry(1, super::status::Size::Small));
        rig.idle(2);
        assert_eq!(rig.spawned(name), 1, "{name} on");
        let look = *rig.effects.spawned.iter().find(|e| e.1 == name).unwrap();
        assert!(matches!(look.2, Attach::Target { target: TargetId(1), .. }) && look.3, "{name}: persistent, on the enemy (it follows it)");
        // sized to its capsule: the rig's enemy is 180 cm tall and 50 cm in radius (the looks are made for 40): widened
        let scale = rig.effects.transformed.iter().find(|t| t.0 == look.0).map(|t| t.1);
        assert_eq!(scale, Some(Vec3::new(1.25, 1.25, 1.0)), "{name}: widened to the body");
        assert!(!rig.effects.stopped.contains(&look.0), "{name} still on");
        rig.secs(2.2);
        assert!(rig.effects.stopped.contains(&look.0), "{name}: stopped when it ends");
    }
}

#[test]
fn chill_stacks_and_exposed_swap_the_look_and_stack_with_the_others() {
    let mut rig = Rig::new(&[Vec3::new(600.0, 0.0, 0.0)]);
    {
        let s = rig.party.don.status.entry(1, super::status::Size::Small);
        (s.chill, s.chill_left, s.wet, s.burn, s.reveal) = (1, 5.0, 5.0, 5.0, 5.0);
    }
    rig.idle(2);
    for name in [fx::CHILL_STACKS[0], fx::WET, fx::BURN, fx::REVEAL] {
        assert_eq!(rig.spawned(name), 1, "{name}: several at once, one look each");
    }
    let one = status_handle(&rig, fx::CHILL_STACKS[0]).unwrap();
    rig.party.don.status.entry(1, super::status::Size::Small).chill = 3;
    rig.idle(2);
    assert_eq!(rig.spawned(fx::CHILL_STACKS[2]), 1, "a third stack: the 3-stack look");
    assert!(rig.effects.stopped.contains(&one), "the 1-stack look fades");
    let rev = status_handle(&rig, fx::REVEAL).unwrap();
    rig.party.don.status.entry(1, super::status::Size::Small).exposed = true;
    rig.idle(2);
    assert_eq!(rig.spawned(fx::EXPOSED), 1, "Exposed: its stronger look");
    assert!(rig.effects.stopped.contains(&rev));
    assert_eq!(rig.spawned(fx::WET) + rig.spawned(fx::BURN), 2, "the others stay as they were");
}

#[test]
fn a_thawing_frozen_breaks_its_ice_with_a_shatter_burst() {
    let mut rig = Rig::new(&[Vec3::new(600.0, 0.0, 0.0)]);
    rig.party.don.status.entry(1, super::status::Size::Small).frozen = 1.0;
    rig.idle(2);
    assert_eq!(rig.spawned(fx::FROZEN_IN), 1, "the freeze-in burst");
    assert_eq!(rig.spawned(fx::FROZEN_OUT), 0);
    let ice = status_handle(&rig, fx::FROZEN).unwrap();
    rig.secs(1.1);
    assert!(rig.effects.stopped_now.contains(&ice), "the ice goes at once");
    assert_eq!(rig.spawned(fx::FROZEN_OUT), 1, "and breaks");
    for n in [fx::FROZEN_IN, fx::FROZEN_OUT] {
        assert!(!n.starts_with("vs_abn_frz"), "KH3's freeze in / out draw Sora's body: never on an enemy");
    }
}

#[test]
fn a_wet_enemy_splashes_when_it_moves() {
    let mut rig = Rig::new(&[Vec3::new(600.0, 0.0, 0.0)]);
    rig.party.don.status.entry(1, super::status::Size::Small).wet = 5.0;
    rig.secs(1.0);
    assert_eq!(rig.spawned(fx::WET_SPLASH), 0, "standing still: no splash");
    rig.dummies.list[0].vel = Vec3::new(400.0, 0.0, 0.0);
    rig.secs(1.0);
    let n = rig.spawned(fx::WET_SPLASH);
    assert!((2..=4).contains(&n), "moving: a splash every 0.3 s ({n})");
}
