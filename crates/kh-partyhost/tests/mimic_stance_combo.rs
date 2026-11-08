//! (MIMIC-STANCE, THE USER 2026-11-05: "Goofy's parry should just work") A Goofy copy against the leader Sora's NON-STOP combo
//! (Attack held down the whole time, as a player mashes): the copy parries the hits of the string wherever the real Goofy's own
//! rules let him (his AI's read, his timing roll, his shield up), hit after hit. The AI pack at build/sora_pack/party/ai (the
//! trace is skipped without it, said on stderr).

use kh_core::host::{HitRecord, HitResult, HurtShape, TargetDefense, TargetFlags, TargetId, TargetInfo};
use kh_core::input::{PadButton, PadButtons, PadState};
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Loadout, Pose, Sora, Status, Vec3, SORA_PARAMS};
use kh_host::formation::Formation;
use kh_host::party::PartyChara;
use kh_partyhost::mimicboss::{self as mb, BossTargets, CopyDefence, FriendBoss, FriendBossStep};
use kh_partyhost::{PartyRuntime, PartyTargets, PartyTest, RouteIn, RuntimeConfig, StepIn};

const DT: f32 = 1.0 / 60.0;
const BOSS: TargetId = TargetId(1);

fn level99() -> Status {
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(std::fs::read_to_string(&path).unwrap().trim_start_matches('\u{feff}')).unwrap();
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let v = match &row[1] {
            serde_json::Value::Number(n) => LoadoutValue::Number(n.as_f64().unwrap()),
            serde_json::Value::String(s) => LoadoutValue::Text(s.clone()),
            serde_json::Value::Array(a) => LoadoutValue::List(a.iter().map(|i| i.as_str().unwrap().to_string()).collect()),
            serde_json::Value::Object(m) => LoadoutValue::Map(m.iter().map(|(k, v)| (k.clone(), v.as_f64().unwrap())).collect()),
            _ => continue,
        };
        loadout.set(row[0].as_str().unwrap(), &v).unwrap();
    }
    loadout.resolve().unwrap();
    Status::from_loadout(&loadout)
}

fn camera() -> kh_camera::CameraView {
    kh_camera::CameraView { position: Vec3::new(-600.0, 0.0, 200.0), rotation: Default::default(), rotation_target: Default::default(), look_point: Vec3::ZERO, distance: 600.0, auto_power: 0.0, stick_wait: 0.0, still_wait: 0.0, reset_wait: 0.0, slope_pitch: 0.0, stick_active: false, followed: false, collided: false }
}

struct NoEnemies;

impl kh_core::Targets for NoEnemies {
    fn enumerate(&self, _out: &mut Vec<TargetInfo>) {}
    fn info(&self, _id: TargetId) -> Option<TargetInfo> {
        None
    }
    fn hurt_shapes(&self, _id: TargetId, _out: &mut Vec<HurtShape>) {}
    fn apply_hit(&mut self, _hit: &HitRecord) -> HitResult {
        HitResult::default()
    }
    fn hit_bone(&self, _id: TargetId, _at: Vec3) -> Option<kh_core::host::TargetBone> {
        None
    }
    fn bone_pose(&self, _id: TargetId, _bone: kh_core::host::TargetBone) -> Option<kh_core::host::BonePose> {
        None
    }
}

impl PartyTargets for NoEnemies {
    fn ai_actors(&self, _out: &mut Vec<kh_ai::ActorView>) {}
    fn attacked_by_enemies(&self) -> bool {
        false
    }
}

/// The real party (Sora leads, Goofy at level 99 with his whole kit) and the boss's Goofy copy 1.6 m in front of Sora.
fn fight() -> Option<(Sora, FriendBoss)> {
    let pack = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/party/ai");
    let Some(ai_pack) = kh_ai::AiPack::load_dir(&pack).ok().map(std::sync::Arc::new) else {
        eprintln!("no AI pack at build/sora_pack/party/ai: skipped");
        return None;
    };
    let mut rt = PartyRuntime::new(RuntimeConfig { wanted: true, test: PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack: Some(ai_pack), ai_pack_source: None });
    let mut sora = Sora::new(SORA_PARAMS, Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(level99());
    sora.command_mut().fill(&level99());
    let world = TestWorld::floor();
    let cam = camera();
    for asked in [None, Some(Formation::new(&[PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy], PartyChara::Sora).unwrap()), None, None] {
        let mut t = NoEnemies;
        let _ = rt.route(RouteIn { sora: &mut sora, input: &Input::default(), dt: DT, targets: &t, camera: &cam, asked_formation: asked, asked_ai: Vec::new(), nav: None, now: 0.0 });
        let out = sora.output();
        let (mut audio, mut effects) = (kh_core::host::audio::NullAudio, kh_core::host::effects::NullEffects);
        rt.step(StepIn { dt: DT, sora: &mut sora, out: &out, lock_on: TargetId::NONE, camera: &cam, world: &world, targets: &mut t, audio: &mut audio, effects: &mut effects, nav: None, now: 0.0 });
        let _ = rt.after_step(&t, DT, 0.0);
    }
    let id = rt.id(PartyChara::Goofy)?;
    *rt.party.status_mut(id)? = kh_party::status::FriendStatus::test_level99(kh_party::Chara::Goofy);
    let boss = FriendBoss::new(&rt, &sora, PartyChara::Goofy, Vec3::new(160.0, 0.0, 0.0), 180.0)?;
    Some((sora, boss))
}

/// The boss's body: each party hit asked of the copy first, a negated one takes nothing, one that lands its reaction.
struct Body<'a> {
    boss: &'a mut FriendBoss,
    log: &'a mut Vec<Answer>,
    k: usize,
}

impl Body<'_> {
    fn info_of(&self) -> Option<TargetInfo> {
        let v = self.boss.view()?;
        Some(TargetInfo { id: BOSS, position: v.position, center: v.position + Vec3::Z * 90.0, velocity: v.velocity, yaw: v.yaw, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 })
    }
}

impl kh_core::Targets for Body<'_> {
    fn enumerate(&self, out: &mut Vec<TargetInfo>) {
        out.extend(self.info_of());
    }
    fn info(&self, id: TargetId) -> Option<TargetInfo> {
        self.info_of().filter(|_| id == BOSS)
    }
    fn hurt_shapes(&self, id: TargetId, out: &mut Vec<HurtShape>) {
        if let Some(i) = self.info(id) {
            out.push(HurtShape { a: i.center - Vec3::Z * 50.0, b: i.center + Vec3::Z * 50.0, radius: i.radius, part: 0 });
        }
    }
    fn apply_hit(&mut self, hit: &HitRecord) -> HitResult {
        let id = self.boss.id();
        let state = id.and_then(|id| self.boss.rt.party.member(id)).map_or(kh_party::NpcStateId::Idle, |m| m.state);
        // what his AI made of this swing: its timing roll (`good`) and its answer
        let roll = self.boss.rt.party.gof.ai.iter().find(|a| Some(a.member) == id).and_then(|a| a.seen.iter().find(|s| s.attacker == mb::victim_id(PartyChara::Sora)).map(|s| (s.good, s.answer)));
        let class = mb::party_hit_class(hit, mb::victim_id(PartyChara::Sora), self.boss.rt.party.telegraphs());
        let d = self.boss.defend(PartyChara::Sora, hit);
        self.log.push(Answer { k: self.k, defence: d, state, roll, class });
        if d.negates() {
            return d.result(1.0e6);
        }
        let _ = self.boss.take_reaction(hit);
        HitResult { accepted: true, damage_dealt: hit.damage, hp_after: 1.0e6, killed: false, guarded: false }
    }
    fn defense(&self, _id: TargetId) -> Option<TargetDefense> {
        Some(TargetDefense { defense: 10, max_hp: 1_000_000, hp_percent: 100.0, enemy_kind: true, damage_min: 0, damage_max: 0, resist: [0; 8], bio_type: 0 })
    }
    fn hit_bone(&self, _id: TargetId, _at: Vec3) -> Option<kh_core::host::TargetBone> {
        None
    }
    fn bone_pose(&self, _id: TargetId, _bone: kh_core::host::TargetBone) -> Option<kh_core::host::BonePose> {
        None
    }
}

/// Sora with Attack held (pressed again every other step: a mash) for `steps`; every answer of the copy to his hits.
/// One answer of the copy to a hit of Sora's, with what his own rules had at that moment.
#[derive(Clone, Copy, Debug)]
struct Answer {
    k: usize,
    defence: CopyDefence,
    /// His state when the hit came (Damage: a KH3 flinch holds him).
    state: kh_party::NpcStateId,
    /// His AI's timing roll of this swing and its answer (None: not read yet).
    roll: Option<(bool, kh_party::goofy::ai::Answer)>,
    class: kh_core::host::AttackClass,
}

fn mash(steps: usize) -> Option<Vec<Answer>> {
    let (mut sora, mut boss) = fight()?;
    let world = TestWorld::floor();
    let cam = camera();
    let mut log = Vec::new();
    let mut before = PadButtons::NONE;
    for k in 0..steps {
        let copy_at = boss.view().map_or(Vec3::ZERO, |v| v.position);
        let mut t = BossTargets { victims: vec![mb::victim_of_sora(&sora, copy_at)], attack_rate: 1.0, ..BossTargets::default() };
        let (mut audio, mut effects) = (kh_core::host::audio::NullAudio, kh_core::host::effects::NullEffects);
        boss.step(FriendBossStep { dt: DT, world: &world, targets: &mut t, audio: &mut audio, effects: &mut effects, camera: &cam, nav: None, now: k as f64 * DT as f64 });
        let (at, yaw) = boss.view().map_or((copy_at, 0.0), |v| (v.position, v.yaw));
        mb::stance_to_sora(&mut t, &mut sora, at);
        for h in t.hits.drain(..) {
            sora.take_hit(mb::player_hit_of(&h, at, yaw));
        }
        let now = if k % 2 == 0 { PadButtons::NONE.with(PadButton::ATTACK) } else { PadButtons::NONE };
        let input = Input::default().with_pad(PadState { held: now, pressed: PadButtons(now.0 & !before.0), released: PadButtons(before.0 & !now.0) });
        before = now;
        let mut body = Body { boss: &mut boss, log: &mut log, k };
        let (mut audio, mut effects, mut ui) = (kh_core::host::audio::NullAudio, kh_core::host::effects::NullEffects, kh_core::host::NullUi);
        let mut host = kh_core::Host { world: &world, targets: &mut body, audio: &mut audio, effects: &mut effects, ui: &mut ui };
        let _ = sora.step_hosted(DT, &input, &mut host);
    }
    Some(log)
}

/// THE REAL GOOFY'S RULES DECIDE EVERY HIT OF A MASHED COMBO: each hit the copy did not parry is one his own rules let through:
/// a KH3 flinch held him (a landed hit's reaction, Goofy's own clips: longer than the gaps of a mashed combo), his AI's
/// timing roll failed for that swing (`ai.parry_rate_*`, 0.85 at level 99), the swing was not read yet (`ai.read_delay_*`),
/// or the row is one Goofy's class rule reads as an unblockable heavy (KH3's ignore-guard flag: Bull Brace or a dodge only).
/// None landed on a free, raised-for-it Goofy with a good roll on a blockable swing. And he parried.
#[test]
fn stance_the_copy_parries_a_non_stop_sora_combo_where_the_real_goofys_rules_let_him() {
    let Some(log) = mash(60 * 20) else { return };
    let parried = log.iter().filter(|a| matches!(a.defence, CopyDefence::Parried { .. })).count();
    eprintln!("{} hits of a 20 s mash: {parried} parried", log.len());
    for a in &log {
        eprintln!("  step {}: {:?} ({:?}, roll {:?}, {:?})", a.k, a.defence, a.state, a.roll, a.class);
    }
    assert!(parried >= 1, "the opening swing is parried");
    let flinching = |a: &Answer| matches!(a.state, kh_party::NpcStateId::Damage);
    let bad_roll = |a: &Answer| a.roll.is_none_or(|(good, _)| !good);
    let unblockable = |a: &Answer| matches!(a.class, kh_core::host::AttackClass::Heavy | kh_core::host::AttackClass::Grab);
    for a in log.iter().filter(|a| a.defence == CopyDefence::Through) {
        assert!(flinching(a) || bad_roll(a) || unblockable(a), "a hit landed that the real Goofy's rules would have answered: {a:?}");
    }
}