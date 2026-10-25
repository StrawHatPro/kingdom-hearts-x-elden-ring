//! (DON-PROG) AI DONALD IN THE OFFLINE AI HARNESS (the style of tests/aix.rs: the shared party runtime in the host's step
//! order with the real AI pack, the real sheets and a level-99 Sora, against scripted enemies). What is measured
//! (documentation/DONALD_OVERHAUL.md 11; the decisions come from kh-party `donald::ai::party_pass`):
//! - his KH3 spells are never cast (the trees' DONALD_FIRE .. CURE requests are the overhaul's choices now);
//! - range: an enemy closing in makes him retreat; the attunement picks (the target's weakness, Water when Sora uses
//!   Thunder, Light when the party is low); the Cure line by KH3's MetaAI; Aero Barrier / Ball Lightning targets;
//! - never a Release; the test level's enemy choice with his aggro abilities (Low Profile, Distraction, Taunt Pass).
//! The casts themselves are DON-MAGIC's (`cast::begin`): until it lands they are refused, so a choice is measured as
//! CHOSEN (`DonProgState::ai[..].chosen`, `last`), the attunement as DONE (its core runs here).
//! `cargo test -p kh-partyhost --test don_aix -- --nocapture` prints each scenario's numbers.

use kh_core::host::{HitResult, HurtShape, TargetDefense, TargetFlags, TargetInfo};
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{HitRecord, Input, Loadout, Output, Pose, Sora, Status, TargetId, Vec3, SORA_PARAMS};
use kh_host::party::PartyChara;
use kh_party::donald::ai::AiChoice;
use kh_party::donald::{DonAbility, DonEvent, Element, SpellRef};
use kh_party::{ActionDef, PartyEventKind};
use kh_partyhost::*;

const STEP: f32 = 1.0 / 60.0;

fn project(p: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(p)
}

fn rows(path: &std::path::Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(t.trim_start_matches('\u{feff}')).ok()).and_then(|j| j["rows"].as_array().cloned()).unwrap_or_default()
}

fn level99() -> Option<Status> {
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(project("sheets/port/sora-loadout-99.json")).ok()?).ok()?;
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array()? {
        let v = match &row[1] {
            serde_json::Value::Number(n) => LoadoutValue::Number(n.as_f64()?),
            serde_json::Value::String(s) => LoadoutValue::Text(s.clone()),
            serde_json::Value::Array(a) => LoadoutValue::List(a.iter().filter_map(|i| i.as_str().map(String::from)).collect()),
            serde_json::Value::Object(m) => LoadoutValue::Map(m.iter().filter_map(|(k, v)| v.as_f64().map(|f| (k.clone(), f))).collect()),
            _ => continue,
        };
        loadout.set(row[0].as_str()?, &v).ok()?;
    }
    loadout.resolve().ok()?;
    Some(Status::from_loadout(&loadout))
}

#[derive(Clone, Debug)]
struct Foe {
    id: u32,
    pos: Vec3,
    hp: f32,
    /// Walks toward Donald at this speed (cm/s) while farther than 150 cm.
    speed: f32,
    /// resist by ETresDamageAttribute (negative = weak).
    resist: [i32; 8],
    last_hit_step: u64,
}

impl Foe {
    fn at(id: u32, x: f32, y: f32) -> Foe {
        Foe { id, pos: Vec3::new(x, y, 0.0), hp: 1.0e9, speed: 0.0, resist: [0; 8], last_hit_step: 0 }
    }
    fn info(&self) -> TargetInfo {
        TargetInfo { id: TargetId(self.id), position: self.pos, center: self.pos + Vec3::Z * 90.0, velocity: Vec3::ZERO, yaw: 180.0, radius: 50.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: self.hp, hp_max: 1.0e9 }
    }
}

#[derive(Default)]
struct Foes {
    list: Vec<Foe>,
    step: u64,
}

impl kh_core::Targets for Foes {
    fn enumerate(&self, out: &mut Vec<TargetInfo>) {
        out.extend(self.list.iter().map(Foe::info));
    }
    fn info(&self, id: TargetId) -> Option<TargetInfo> {
        self.list.iter().find(|f| f.id == id.0).map(Foe::info)
    }
    fn hurt_shapes(&self, id: TargetId, out: &mut Vec<HurtShape>) {
        if let Some(f) = self.list.iter().find(|f| f.id == id.0) {
            out.push(HurtShape { a: f.pos + Vec3::Z * 50.0, b: f.pos + Vec3::Z * 130.0, radius: 50.0, part: 0 });
        }
    }
    fn apply_hit(&mut self, hit: &HitRecord) -> HitResult {
        let step = self.step;
        let Some(f) = self.list.iter_mut().find(|f| f.id == hit.target.0) else { return HitResult::default() };
        f.last_hit_step = step;
        HitResult { accepted: true, damage_dealt: hit.damage, hp_after: f.hp, killed: false, guarded: false }
    }
    fn defense(&self, id: TargetId) -> Option<TargetDefense> {
        let f = self.list.iter().find(|f| f.id == id.0)?;
        Some(TargetDefense { defense: 0, max_hp: 100_000, hp_percent: 100.0, enemy_kind: true, damage_min: 0, damage_max: 0, resist: f.resist, bio_type: 0 })
    }
    fn engaged_count(&self) -> u32 {
        self.list.len() as u32
    }
}

impl PartyTargets for Foes {
    fn ai_actors(&self, out: &mut Vec<kh_ai::ActorView>) {
        out.extend(self.list.iter().map(|f| kh_ai::ActorView { actor: kh_ai::ActorRef::Enemy(f.id), uid: -1, position: f.pos, center: f.pos + Vec3::Z * 90.0, yaw: 180.0, radius: 50.0, half_height: 90.0, alive: true, hostile: true, lockable: true, hp: f.hp, hp_max: 1.0e9, ..kh_ai::ActorView::default() }));
    }
    fn attacked_by_enemies(&self) -> bool {
        true
    }
    fn fighting_party(&self, _id: u32) -> bool {
        true
    }
}

struct Sim {
    rt: PartyRuntime,
    sora: Sora,
    out: Output,
    world: TestWorld,
    foes: Foes,
    t: f64,
    camera: kh_camera::CameraView,
    /// Every Donald event of the overhaul and every KH3 spell Donald cast.
    don_events: Vec<DonEvent>,
    kh3_spells: u32,
}

impl Sim {
    fn new(foes: Vec<Foe>) -> Option<Sim> {
        let pack = match kh_ai::AiPack::load_dir(&project("build/sora_pack/party/ai")) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("NO AI PACK ({e}): the scenario is skipped");
                return None;
            }
        };
        let sheets = project("sheets/port");
        let config = RuntimeConfig { wanted: true, test: PartyTest::from_rows(&rows(&sheets.join("party-test.json"))), weapons: PartyWeapon::from_rows(&rows(&sheets.join("party-weapons.json"))), ai_pack: Some(std::sync::Arc::new(pack)), ai_pack_source: None };
        let rt = PartyRuntime::new(config);
        let sora = Sora::new(SORA_PARAMS, Pose { position: Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS)), yaw: 0.0 }).with_status(level99()?);
        let out = sora.output();
        let camera = kh_camera::CameraView { position: Vec3::new(-450.0, 0.0, 220.0), rotation: kh_camera::Rot { pitch: -15.0, yaw: 0.0, roll: 0.0 }, rotation_target: Default::default(), look_point: Vec3::ZERO, distance: 450.0, auto_power: 0.0, stick_wait: 0.0, still_wait: 0.0, reset_wait: 0.0, slope_pitch: 0.0, stick_active: false, followed: false, collided: false };
        Some(Sim { rt, sora, out, world: TestWorld::floor(), foes: Foes { list: foes, step: 0 }, t: 0.0, camera, don_events: Vec::new(), kh3_spells: 0 })
    }

    fn donald(&self) -> kh_party::MemberId {
        self.rt.id(PartyChara::Donald).expect("Donald in the party")
    }

    fn step(&mut self, each: &mut dyn FnMut(&mut PartyRuntime)) {
        self.foes.step += 1;
        let input = Input::default();
        let routed = self.rt.route(RouteIn { sora: &mut self.sora, input: &input, dt: STEP, targets: &self.foes, camera: &self.camera, asked_formation: None, asked_ai: Vec::new(), nav: None, now: self.t });
        // the chasers walk toward Donald
        if let Some(d) = self.rt.id(PartyChara::Donald).and_then(|id| self.rt.party.view(id)) {
            for f in self.foes.list.iter_mut().filter(|f| f.speed > 0.0) {
                let to = Vec3::new(d.position.x - f.pos.x, d.position.y - f.pos.y, 0.0);
                if to.length() > 150.0 {
                    f.pos += to.normalize_or_zero() * f.speed * STEP;
                }
            }
        }
        {
            let mut null = kh_core::NullHost::default();
            let mut targets = SoraTargets { targets: &mut self.foes, runtime: &mut self.rt };
            let mut host = kh_core::Host { world: &self.world, targets: &mut targets, audio: &mut null.audio, effects: &mut null.effects, ui: &mut null.ui };
            self.out = self.sora.step_hosted(STEP, &routed, &mut host);
        }
        each(&mut self.rt);
        let mut audio = kh_core::host::audio::NullAudio;
        let mut effects = kh_core::host::effects::NullEffects;
        let out = self.out.clone();
        let so = self.rt.step(StepIn { dt: STEP, sora: &mut self.sora, out: &out, lock_on: TargetId::NONE, camera: &self.camera, world: &self.world, targets: &mut self.foes, audio: &mut audio, effects: &mut effects, nav: None, now: self.t });
        let _ = self.rt.after_step(&self.foes, STEP, self.t);
        for e in &so.events {
            if let PartyEventKind::SpellCast { def } = e.kind {
                if matches!(def, ActionDef::DonaldFire | ActionDef::DonaldBlizzard | ActionDef::DonaldThunder | ActionDef::DonaldAero | ActionDef::DonaldCure) {
                    self.kh3_spells += 1;
                }
            }
        }
        self.don_events.extend(self.rt.party.don.events.iter().map(|e| e.1));
        self.t += STEP as f64;
    }

    fn run(&mut self, seconds: f32, each: &mut dyn FnMut(&mut PartyRuntime)) {
        for _ in 0..(seconds / STEP).round() as usize {
            self.step(each);
        }
    }

    fn memory(&self) -> Option<kh_party::donald::ai::AiMemory> {
        let id = self.rt.id(PartyChara::Donald)?;
        self.rt.party.don_prog.ai.iter().find(|a| a.member == id).copied()
    }

    fn attuned(&self) -> Option<Element> {
        self.rt.party.status(self.donald()).and_then(|s| s.donald.as_ref()).and_then(|d| d.core.attune.primary())
    }

    fn no_release(&self) -> bool {
        !self.don_events.iter().any(|e| matches!(e, DonEvent::Released { .. }))
    }
}

/// Records every choice AI Donald makes (the memory's last choice when the count grows).
struct Choices(Vec<(AiChoice, bool)>, u32);

impl Choices {
    fn see(&mut self, rt: &PartyRuntime) {
        let Some(id) = rt.id(PartyChara::Donald) else { return };
        if let Some(m) = rt.party.don_prog.ai.iter().find(|a| a.member == id) {
            if m.chosen > self.1 {
                self.1 = m.chosen;
                if let Some(c) = m.last {
                    self.0.push(c);
                }
            }
        }
    }
    fn casts(&self) -> Vec<(SpellRef, kh_party::donald::cast::CastTarget)> {
        self.0.iter().filter_map(|(c, _)| if let AiChoice::Cast { plan, target } = c { Some((plan.spell, *target)) } else { None }).collect()
    }
}

#[test]
fn don_aix_his_kh3_spells_are_gone_he_attunes_to_the_weakness_and_never_releases() {
    // a dummy weak to Blizzard 12 m ahead
    let mut weak = Foe::at(1, 1200.0, 0.0);
    weak.resist[kh_core::enums::ETresDamageAttribute::Blizzard as usize] = -50;
    let Some(mut sim) = Sim::new(vec![weak]) else { return };
    let mut seen = Choices(Vec::new(), 0);
    let mut attuned = Vec::new();
    for _ in 0..(20.0 / STEP) as usize {
        sim.step(&mut |_| {});
        seen.see(&sim.rt);
        if let Some(e) = sim.attuned() {
            if attuned.last() != Some(&e) {
                attuned.push(e);
            }
        }
    }
    let m = sim.memory().expect("AI Donald decided");
    println!("don_aix weak-to-Blizzard: {} choices ({} started), attuned {:?}, casts {:?}, KH3 spells {}", m.chosen, m.started, attuned, seen.casts().iter().take(6).collect::<Vec<_>>(), sim.kh3_spells);
    assert_eq!(sim.kh3_spells, 0, "his five KH3 spells are replaced by the overhaul");
    assert!(m.chosen > 0, "the trees asked and the overhaul chose");
    assert_eq!(attuned.first(), Some(&Element::Blizzard), "the target's weakness (he has Blizzard in the test level)");
    assert!(sim.no_release(), "never a deliberate Release");
}

#[test]
fn don_aix_water_when_sora_uses_thunder_and_light_when_the_party_is_low() {
    let Some(mut sim) = Sim::new(vec![Foe::at(1, 1200.0, 300.0)]) else { return };
    // Sora's Thunder lands all the time
    sim.run(10.0, &mut |rt| kh_party::donald::ai::note_sora_thunder(&mut rt.party));
    println!("don_aix thunder: attuned {:?}", sim.attuned());
    assert_eq!(sim.attuned(), Some(Element::Water), "Water for Conduct");
    // the party low: Light (he switches only without a Release: Water is far from Resonance 3 here, no casts land)
    let Some(mut low) = Sim::new(vec![Foe::at(1, 1200.0, 300.0)]) else { return };
    for _ in 0..(10.0 / STEP) as usize {
        let max = low.sora.status().max_hp();
        low.sora.status_mut().set_hp(max / 5);
        low.step(&mut |rt| {
            for (_, id) in rt.members.clone() {
                if let Some(s) = rt.party.status_mut(id) {
                    s.hp = (s.max_hp() / 5).max(1);
                }
            }
        });
    }
    println!("don_aix low party: attuned {:?}", low.attuned());
    assert_eq!(low.attuned(), Some(Element::Light));
    assert!(sim.no_release() && low.no_release());
}

#[test]
fn don_aix_heals_by_kh3s_cure_rules_with_the_cure_line_and_shields_the_attacked() {
    let Some(mut sim) = Sim::new(vec![Foe::at(1, 900.0, 300.0)]) else { return };
    let mut seen = Choices(Vec::new(), 0);
    // (DON-SPELLB one worn) 40 s: without the barrier / orb ping-pong (now forbidden) his first heal comes after his opening
    // attacks (he keeps a Cura's MP in reserve while Goofy is low, ai.rs)
    for _ in 0..(40.0 / STEP) as usize {
        sim.step(&mut |rt| {
            // Goofy is hurt over and over (KH3's MetaAI: Cure him)
            if let Some(g) = rt.id(PartyChara::Goofy) {
                if let Some(s) = rt.party.status_mut(g) {
                    s.hp = (s.max_hp() / 4).max(1);
                }
            }
        });
        seen.see(&sim.rt);
    }
    let casts = seen.casts();
    let goofy = sim.rt.id(PartyChara::Goofy).unwrap();
    let cures: Vec<_> = casts.iter().filter(|c| matches!(c.0, SpellRef::Line1(Element::Light, _))).collect();
    let barrier: Vec<_> = casts.iter().filter(|c| c.0 == SpellRef::Line2(Element::Aero, 3)).collect();
    println!("don_aix heal: {} choices, cures {:?}, barriers {:?}", seen.0.len(), cures.iter().take(3).collect::<Vec<_>>(), barrier.iter().take(3).collect::<Vec<_>>());
    assert!(cures.iter().any(|c| c.1 == kh_party::donald::cast::CastTarget::Member(goofy)), "the Cure line on Goofy: {casts:?}");
    // 75 % missing: the first (full MP) is Cura or more; later ones may be a cheaper tier once his MP runs low (the
    // highest tier he can pay: the MP costs are real since DON-MAGIC's casts spend them)
    assert!(matches!(cures.first().map(|c| c.0), Some(SpellRef::Line1(Element::Light, 2..=4))), "75 % missing: Cura or more first: {cures:?}");
    assert!(barrier.iter().any(|c| c.1 == kh_party::donald::cast::CastTarget::Member(goofy)), "Aero Barrier on the low friend: {casts:?}");
    assert_eq!(sim.kh3_spells, 0);
}

#[test]
fn don_aix_retreats_from_a_chaser() {
    let mut chaser = Foe::at(1, 1500.0, 0.0);
    chaser.speed = 250.0;
    let Some(mut sim) = Sim::new(vec![chaser]) else { return };
    let mut seen = Choices(Vec::new(), 0);
    for _ in 0..(20.0 / STEP) as usize {
        sim.step(&mut |_| {});
        seen.see(&sim.rt);
    }
    let moves = seen.0.iter().filter(|c| matches!(c.0, AiChoice::Move { retreat: true, .. })).count();
    let cornered: Vec<_> = seen.casts().into_iter().filter(|c| c.1 == kh_party::donald::cast::CastTarget::Member(sim.donald())).collect();
    println!("don_aix chaser: {moves} retreat moves, self-casts {:?}", cornered.iter().take(4).collect::<Vec<_>>());
    assert!(moves > 0, "he steps back from an enemy within 6 m");
    assert!(cornered.iter().any(|c| c.0 == SpellRef::Line2(Element::Aero, 3)), "Aero Barrier on himself when it closes in");
}

#[test]
fn don_aix_the_test_levels_enemies_choose_him_less() {
    let Some(mut sim) = Sim::new(vec![Foe::at(1, 1200.0, 0.0)]) else { return };
    sim.run(1.0, &mut |_| {});
    let d = sim.donald();
    let dpos = sim.rt.party.view(d).unwrap().position;
    // an enemy right by Donald (Sora and Goofy farther): count its choices over 2-s windows
    let from = dpos + Vec3::new(100.0, 0.0, 0.0);
    let sample = |sim: &mut Sim, n: usize| -> f32 {
        let mut on = 0;
        for k in 0..n {
            sim.rt.party.don_prog.clock = k as f32 * 2.0;
            if sim.rt.victim(k as u32 % 97, from, sim.out.position) == runtime_victim_donald() {
                on += 1;
            }
        }
        on as f32 / n as f32
    };
    // the test level's loadout: Low Profile, Hide Behind, Distraction ... equipped
    let share = sample(&mut sim, 2000);
    println!("don_aix aggro: an enemy by Donald picks him {:.0} % of the time", share * 100.0);
    assert!(share < 0.75, "Low Profile / Hide Behind / Distraction: {share}");
    // without his aggro abilities: always him (the nearest)
    for k in [DonAbility::LowProfile, DonAbility::HideBehind, DonAbility::Distraction, DonAbility::OutOfSight] {
        sim.rt.party.status_mut(d).unwrap().donald.as_mut().unwrap().abilities.set_equipped(k, false);
    }
    assert_eq!(sample(&mut sim, 200), 1.0);
    // Taunt Pass: Goofy guards, the enemy near Donald turns to Goofy
    sim.rt.party.status_mut(d).unwrap().donald.as_mut().unwrap().abilities.set_equipped(DonAbility::TauntPass, true);
    let g = sim.rt.id(PartyChara::Goofy).unwrap();
    kh_party::donald::ai::note_guard(&mut sim.rt.party, g);
    assert_eq!(sim.rt.victim(5, from, sim.out.position), Victim::Friend(PartyChara::Goofy));
}

fn runtime_victim_donald() -> Victim {
    Victim::Friend(PartyChara::Donald)
}
