//! (MIMIC-STANCE, THE USER 2026-11-05: "make sure Goofy's mimic tear boss is able to parry you properly, can stagger you by
//! breaking your stance and can use his Breaker on you") A Goofy copy (`FriendBoss`, the bosses' and a boss-summoned Mimic's)
//! against the leader through the real cores: the leader Sora's own combo (his `step_hosted`, his hits on the boss's body) and a
//! leading Goofy's own shoves (the real party's step); the copy's Goofy AI reads their telegraphs ([`victim_tell_of_sora`] /
//! [`victim_tell_of_friend`]), parries by Goofy's judgement ([`FriendBoss::defend`]), breaks the leader's stance (KH3's STUN
//! state for the window) and slams him with Goofy Breaker; the leader recovers. The AI pack at build/sora_pack/party/ai (the
//! traces are skipped without it, said on stderr).

use kh_core::host::{HitRecord, HitResult, HurtShape, TargetDefense, TargetFlags, TargetId, TargetInfo};
use kh_core::input::{PadButton, PadButtons, PadState};
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Loadout, Pose, Sora, StateId, Status, Vec3, SORA_PARAMS};
use kh_host::formation::Formation;
use kh_host::party::PartyChara;
use kh_party::goofy::K;

use crate::mimicboss::*;
use crate::mimicboss::Victim;
use crate::*;

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

fn runtime() -> Option<PartyRuntime> {
    let pack = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/party/ai");
    let ai_pack = kh_ai::AiPack::load_dir(&pack).ok().map(std::sync::Arc::new);
    if ai_pack.is_none() {
        eprintln!("no AI pack at build/sora_pack/party/ai: skipped");
        return None;
    }
    Some(PartyRuntime::new(RuntimeConfig { wanted: true, test: PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack, ai_pack_source: None }))
}

fn sora_at(p: Vec3) -> Sora {
    let mut s = Sora::new(SORA_PARAMS, Pose { position: p, yaw: 0.0 }).with_status(level99());
    s.command_mut().fill(&level99());
    s
}

fn pad(held: &[PadButton], before: PadButtons) -> PadState {
    let now = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
    PadState { held: now, pressed: PadButtons(now.0 & !before.0), released: PadButtons(before.0 & !now.0) }
}

/// THE BOSS'S BODY as the party's attacks meet it (the Elden Ring host's part in this run): the copy where it stands, each hit
/// asked of the copy first ([`FriendBoss::defend`]); a negated one takes nothing, one that lands takes its HP and gives the copy
/// its reaction ([`FriendBoss::take_reaction`]), as the host must.
struct BossBody<'a> {
    boss: &'a mut FriendBoss,
    from: PartyChara,
    log: &'a mut Vec<CopyDefence>,
    hp: &'a mut f32,
}

impl BossBody<'_> {
    fn info_of(&self) -> Option<TargetInfo> {
        let v = self.boss.view()?;
        Some(TargetInfo { id: BOSS, position: v.position, center: v.position + Vec3::Z * 90.0, velocity: v.velocity, yaw: v.yaw, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: *self.hp, hp_max: 1.0e6 })
    }
}

impl kh_core::Targets for BossBody<'_> {
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
        let d = self.boss.defend(self.from, hit);
        self.log.push(d);
        if d.negates() {
            return d.result(*self.hp);
        }
        let _ = self.boss.take_reaction(hit);
        *self.hp -= hit.damage;
        HitResult { accepted: true, damage_dealt: hit.damage, hp_after: *self.hp, killed: false, guarded: false }
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

impl PartyTargets for BossBody<'_> {
    fn ai_actors(&self, out: &mut Vec<kh_ai::ActorView>) {
        if let Some(i) = self.info_of() {
            out.push(kh_ai::ActorView { actor: kh_ai::ActorRef::Enemy(BOSS.0), position: i.position, center: i.center, yaw: i.yaw, radius: i.radius, half_height: i.half_height, alive: true, hostile: true, lockable: true, hp: i.hp, hp_max: i.hp_max, ..kh_ai::ActorView::default() });
        }
    }
    fn attacked_by_enemies(&self) -> bool {
        false
    }
}

/// What a run saw.
#[derive(Debug, Default)]
struct Run {
    defences: Vec<CopyDefence>,
    orders: Vec<StanceOrder>,
    /// The copy's hits that reached the leader: (step, attack row).
    hits: Vec<(usize, &'static str)>,
    /// Steps the leader stood in KH3's STUN state, the first and the last.
    stunned: usize,
    stun_from: Option<usize>,
    stun_to: Option<usize>,
    /// The step the leader was up again (free) after the break.
    recovered: Option<usize>,
    /// The fullest his gauge was in the copy's book.
    stagger_max: f32,
}

impl Run {
    fn parried(&self) -> usize {
        self.defences.iter().filter(|d| matches!(d, CopyDefence::Parried { .. })).count()
    }
    fn broke(&self) -> Option<f32> {
        self.orders.iter().find_map(|o| match o {
            StanceOrder::Break { seconds, .. } => Some(*seconds),
            _ => None,
        })
    }
    fn breaker(&self) -> Option<usize> {
        self.hits.iter().find(|h| h.1 == "GOOFY_BREAKER").map(|h| h.0)
    }
}

/// The real party with Goofy at level 99 and his whole kit; `leader` leads. The Goofy copy at `at` (a boss-summoned Mimic
/// copying Goofy while Sora leads; the boss's copy of a leading Goofy).
fn fight(leader: PartyChara, at: Vec3) -> Option<(PartyRuntime, Sora, FriendBoss)> {
    let mut rt = runtime()?;
    let mut sora = sora_at(Vec3::ZERO);
    let world = TestWorld::floor();
    let cam = camera();
    let asked = [None, Some(Formation::new(&[PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy], leader).unwrap()), None, None, None];
    for a in asked {
        friend_step(&mut rt, &mut sora, &world, &cam, a, None);
    }
    let id = rt.id(PartyChara::Goofy).unwrap();
    *rt.party.status_mut(id).unwrap() = kh_party::status::FriendStatus::test_level99(kh_party::Chara::Goofy);
    if leader == PartyChara::Goofy {
        if let Some(m) = rt.party.member_mut(id) {
            m.cmc.location = Vec3::new(0.0, 0.0, m.cmc.location.z);
            m.yaw = 0.0;
        }
    }
    let boss = FriendBoss::new(&rt, &sora, PartyChara::Goofy, at, 180.0).expect("the copy");
    // the fog gate: the leader alone (his friends and AI Sora out of the field)
    assert!(rt.solo_begin(&mut sora, "Mimic Tear", 0.0));
    Some((rt, sora, boss))
}

/// One step of the real party with no enemy (`body`: the boss's body as its target, `input`: the leader's pad).
fn friend_step(rt: &mut PartyRuntime, sora: &mut Sora, world: &TestWorld, cam: &kh_camera::CameraView, asked: Option<Formation>, body: Option<(&mut BossBody<'_>, Input)>) {
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
    let mut none = NoEnemies;
    let (targets, input): (&mut dyn PartyTargets, Input) = match body {
        Some((b, i)) => (b, i),
        None => (&mut none, Input::default()),
    };
    let _ = rt.route(RouteIn { sora, input: &input, dt: DT, targets, camera: cam, asked_formation: asked, asked_ai: Vec::new(), nav: None, now: 0.0 });
    let out = sora.output();
    let (mut audio, mut effects) = (kh_core::host::audio::NullAudio, kh_core::host::effects::NullEffects);
    rt.step(StepIn { dt: DT, sora, out: &out, lock_on: TargetId::NONE, camera: cam, world, targets, audio: &mut audio, effects: &mut effects, nav: None, now: 0.0 });
    let _ = rt.after_step(&*targets, DT, 0.0);
}

fn sora_victim(sora: &Sora, copy_at: Vec3) -> Victim {
    let o = sora.output();
    let st = sora.status();
    Victim { chara: PartyChara::Sora, feet: o.position, yaw: o.yaw, velocity: o.velocity, radius: 34.0, half_height: 90.0, hp: st.hp, hp_max: st.max_hp(), defense: st.defense(), airborne: !o.on_floor, down: st.hp < 1, tell: victim_tell_of_sora(sora, copy_at), stance: victim_stance_of_sora(sora) }
}

fn friend_victim(rt: &PartyRuntime, c: PartyChara, copy_at: Vec3) -> Victim {
    let id = rt.id(c).unwrap();
    let v = rt.party.view(id).unwrap();
    let defense = rt.party.status(id).unwrap().defense();
    Victim { chara: c, feet: v.position, yaw: v.yaw, velocity: v.velocity, radius: v.capsule.0.max(30.0), half_height: v.capsule.1.max(60.0), hp: v.hp, hp_max: v.hp_max, defense, airborne: !v.on_ground, down: v.ko, tell: victim_tell_of_friend(rt, c, copy_at), stance: victim_stance_of_friend(rt, c) }
}

/// The copy's step against `victim`: its targets after it (the orders and hits for the host: the orders FIRST, then the hits),
/// and where it stands.
fn copy_step(boss: &mut FriendBoss, victim: Victim, k: usize) -> (BossTargets, Vec3, f32) {
    let world = TestWorld::floor();
    let cam = camera();
    let (copy_at, copy_yaw) = boss.view().map_or((Vec3::ZERO, 0.0), |v| (v.position, v.yaw));
    let mut t = BossTargets { victims: vec![victim], attack_rate: 1.0, ..BossTargets::default() };
    let (mut audio, mut effects) = (kh_core::host::audio::NullAudio, kh_core::host::effects::NullEffects);
    boss.step(FriendBossStep { dt: DT, world: &world, targets: &mut t, audio: &mut audio, effects: &mut effects, camera: &cam, nav: None, now: k as f64 * DT as f64 });
    (t, copy_at, copy_yaw)
}

/// THE LEADER SORA vs a Goofy copy 1.6 m in front of him for up to `steps`: Sora presses Attack when he is free and `every`
/// steps have passed since his last press (his own swings, his hits on the boss's body asked of the copy first). Stops `after`
/// steps past the first Breaker hit.
fn sora_vs_goofy(steps: usize, every: usize, after: usize) -> Option<(Run, Sora, FriendBoss)> {
    let (_rt, mut sora, mut boss) = fight(PartyChara::Sora, Vec3::new(160.0, 0.0, 0.0))?;
    let world = TestWorld::floor();
    let mut run = Run::default();
    let mut hp = 1.0e6;
    let mut before = PadButtons::NONE;
    let mut next = 0;
    for k in 0..steps {
        let copy_at = boss.view().map_or(Vec3::ZERO, |v| v.position);
        let (mut t, at, yaw) = copy_step(&mut boss, sora_victim(&sora, copy_at), k);
        run.stagger_max = run.stagger_max.max(boss.victim_stagger(PartyChara::Sora).map_or(0.0, |v| v.value));
        run.orders.extend(stance_to_sora(&mut t, &mut sora, at));
        assert!(t.orders.is_empty(), "every order is the host's to carry out");
        for h in t.hits.drain(..) {
            run.hits.push((k, h.attack_name));
            sora.take_hit(player_hit_of(&h, at, yaw));
        }
        let free = matches!(sora.output().state, StateId::Idle | StateId::Run | StateId::Attack);
        if free && k >= next {
            next = k + every;
        }
        let held: &[PadButton] = if free && k + every < next + 2 { &[PadButton::ATTACK] } else { &[] };
        let input = Input::default().with_pad(pad(held, before));
        before = input.pad.held;
        let out = {
            let mut body = BossBody { boss: &mut boss, from: PartyChara::Sora, log: &mut run.defences, hp: &mut hp };
            let (mut audio, mut effects, mut ui) = (kh_core::host::audio::NullAudio, kh_core::host::effects::NullEffects, kh_core::host::NullUi);
            let mut host = kh_core::Host { world: &world, targets: &mut body, audio: &mut audio, effects: &mut effects, ui: &mut ui };
            sora.step_hosted(DT, &input, &mut host)
        };
        if out.state == StateId::BadstatStun {
            run.stunned += 1;
            run.stun_from.get_or_insert(k);
            run.stun_to = Some(k);
        } else if run.stun_to.is_some() && run.recovered.is_none() && !sora.stance().broken() && matches!(out.state, StateId::Idle | StateId::Run | StateId::Attack) {
            run.recovered = Some(k);
        }
        if run.breaker().is_some_and(|b| k >= b + after) {
            break;
        }
    }
    Some((run, sora, boss))
}

#[test]
fn stance_the_goofy_copy_parries_the_leader_soras_combo_in_its_window_for_no_damage() {
    let Some((run, _sora, _boss)) = sora_vs_goofy(60 * 20, 90, 0) else { return };
    eprintln!("the copy's answers to Sora's hits: {:?}", run.defences);
    let parried = run.defences.iter().find_map(|d| match d {
        CopyDefence::Parried { tech, right } => Some((*tech, *right)),
        _ => None,
    });
    let (tech, _) = parried.expect("the copy parried a hit of Sora's combo");
    assert_eq!(tech, kh_party::goofy::Tech::Deflect, "a keyblade swing is a Normal blow: Deflect is its answer");
    assert!(CopyDefence::Parried { tech, right: true }.negates(), "a parried hit never reaches the boss's Elden Ring body");
    // the parry filled the leader's stance in the copy's own book, by the enemy side's rows (a deflect's 25 at least)
    assert!(run.stagger_max >= K.stagger_deflect || run.broke().is_some(), "the parry's Stagger is on Sora: {}", run.stagger_max);
}

#[test]
fn stance_the_goofy_copy_breaks_the_leader_soras_stance_he_kneels_for_the_window_takes_breaker_and_recovers() {
    let Some((run, sora, _boss)) = sora_vs_goofy(60 * 90, 90, 60 * 6) else { return };
    eprintln!("parries {}, orders {:?}, the copy's hits {:?}, stunned {} steps ({:?} - {:?}), recovered {:?}", run.parried(), run.orders, run.hits.iter().filter(|h| h.1 == "GOOFY_BREAKER").collect::<Vec<_>>(), run.stunned, run.stun_from, run.stun_to, run.recovered);
    assert!(run.parried() >= 2, "his parries are what break Sora: {}", run.parried());
    let window = run.broke().expect("the copy's book filled Sora's gauge and broke his stance");
    assert_eq!(window, K.stagger_window, "the enemy side's window of a normal-sized body (stagger.window)");
    let from = run.stun_from.expect("Sora stood in KH3's stun");
    assert!(run.stunned >= 30, "held in place, no action: {} steps", run.stunned);
    let slam = run.breaker().expect("Goofy Breaker landed on the broken Sora");
    assert!(slam >= from, "the slam came on the break");
    let pos = |o: &StanceOrder| run.orders.iter().position(|x| x == o);
    let release = pos(&StanceOrder::Release { chara: PartyChara::Sora }).expect("the break was released");
    assert!(run.orders.iter().position(|o| matches!(o, StanceOrder::Hold { .. })).is_some_and(|h| h < release), "the Breaker held the break, then released it before the slam: {:?}", run.orders);
    assert!(run.recovered.is_some_and(|r| r > slam), "Sora is up again after the slam: {:?}", run.recovered);
    assert!(sora.status().hp > 0 && !sora.stance().broken());
}

/// A LEADING GOOFY vs the copy of himself: the leader's own shoves (A on his pad, the real party's step); the copy's step with
/// [`friend_victim`]; its orders on his pawn ([`stance_to_party`]) and its hits through his `receive_hit`.
fn goofy_vs_goofy(steps: usize, every: usize, after: usize) -> Option<(Run, PartyRuntime)> {
    let (mut rt, mut sora, mut boss) = fight(PartyChara::Goofy, Vec3::new(160.0, 0.0, 0.0))?;
    let world = TestWorld::floor();
    let cam = camera();
    let lead = rt.id(PartyChara::Goofy).unwrap();
    let mut run = Run::default();
    let mut hp = 1.0e6;
    let (mut before, mut next) = (PadButtons::NONE, 0);
    for k in 0..steps {
        let copy_at = boss.view().map_or(Vec3::ZERO, |v| v.position);
        let (mut t, at, _) = copy_step(&mut boss, friend_victim(&rt, PartyChara::Goofy, copy_at), k);
        run.stagger_max = run.stagger_max.max(boss.victim_stagger(PartyChara::Goofy).map_or(0.0, |v| v.value));
        run.orders.extend(stance_to_party(&mut t, &mut rt, at));
        assert!(t.orders.is_empty(), "every order is the host's to carry out");
        for h in t.hits.drain(..) {
            run.hits.push((k, h.attack_name));
            let _ = rt.receive_hit(PartyChara::Goofy, &h);
        }
        // the leader's pad: A when he is free, once every `every` steps at most, toward the copy (its stick on it)
        let to = boss.view().map_or(Vec3::X, |v| v.position) - rt.party.view(lead).map_or(Vec3::ZERO, |v| v.position);
        let state = rt.party.member(lead).unwrap().state;
        let free = matches!(state, kh_party::NpcStateId::Idle | kh_party::NpcStateId::Move | kh_party::NpcStateId::Pad(kh_party::pad::RUN));
        if free && k >= next {
            next = k + every;
        }
        // (MIMIC-BOSS hotfix 3) up again after the slam he stops shoving: two parried shoves fill the gauge (the Stagger book's
        // rows, its reset share 0; measured 6 -> 51 -> 100), so shoving on into the parrying copy breaks him again (KH3-true
        // reach since 2026-11-07); the run then proves the stance clears after the slam
        let shoving = run.recovered.is_none();
        let held: &[PadButton] = if shoving && free && k + every < next + 2 { &[PadButton::FaceBottom] } else { &[] };
        let mut input = Input::default().with_pad(pad(held, before));
        before = input.pad.held;
        if to.length() > 120.0 {
            // (MIMIC-BOSS hotfix 3) the stick is (right, forward) of the camera (kh-party pad `stick_of`; camera yaw 0 looks
            // along +X): toward the copy is forward = to.x, right = to.y (the old (to.x, to.y) ran him sideways, away from it)
            input.move_stick = kh_core::Vec2::new(to.y, to.x).normalize_or_zero();
        }
        {
            let mut body = BossBody { boss: &mut boss, from: PartyChara::Goofy, log: &mut run.defences, hp: &mut hp };
            friend_step(&mut rt, &mut sora, &world, &cam, None, Some((&mut body, input)));
        }
        let state = rt.party.member(lead).unwrap().state;
        if state == kh_party::NpcStateId::BadStatStun {
            run.stunned += 1;
            run.stun_from.get_or_insert(k);
            run.stun_to = Some(k);
        } else if run.stun_to.is_some() && run.recovered.is_none() && !rt.party.member(lead).unwrap().stance.broken() && !matches!(state, kh_party::NpcStateId::Damage | kh_party::NpcStateId::Die) {
            run.recovered = Some(k);
        }
        if run.breaker().is_some_and(|b| k >= b + after) {
            break;
        }
    }
    Some((run, rt))
}

#[test]
fn stance_a_leading_goofy_is_parried_broken_slammed_and_recovers_against_his_copy() {
    let Some((run, rt)) = goofy_vs_goofy(60 * 90, 90, 60 * 6) else { return };
    eprintln!("parries {}, defences {}, orders {:?}, Breaker hits {:?}, stunned {} ({:?} - {:?}), recovered {:?}", run.parried(), run.defences.len(), run.orders, run.hits.iter().filter(|h| h.1 == "GOOFY_BREAKER").collect::<Vec<_>>(), run.stunned, run.stun_from, run.stun_to, run.recovered);
    assert!(run.parried() >= 1, "the copy parried its leader's shoves: {:?}", run.defences);
    assert!(run.broke().is_some(), "his stance broke");
    assert!(run.stunned >= 30, "held in KH3's stun: {} steps", run.stunned);
    let slam = run.breaker().expect("Goofy Breaker landed on the broken leader");
    assert!(run.recovered.is_some_and(|r| r > slam), "up again after the slam");
    let id = rt.id(PartyChara::Goofy).unwrap();
    assert!(!rt.party.member(id).unwrap().stance.broken());
}

/// The copy's Stagger book is answered from the real stance: no break on a member who is down or broken already, a Breaker never
/// defeats him outright, Pin Down holds nothing of his; a hold, a release and a rebound are orders for the host.
#[test]
fn stance_the_copys_book_asks_the_real_stance_and_the_host_carries_it_out() {
    use kh_core::host::StaggerAsk;
    use kh_core::Targets;
    let mut v = Victim { chara: PartyChara::Sora, feet: Vec3::ZERO, yaw: 0.0, velocity: Vec3::ZERO, radius: 34.0, half_height: 90.0, hp: 100, hp_max: 100, defense: 10, airborne: false, down: false, tell: VictimTell::default(), stance: VictimStance::default() };
    let id = victim_id(PartyChara::Sora);
    let mut t = BossTargets { victims: vec![v], attack_rate: 1.0, ..BossTargets::default() };
    assert!(!t.stagger_ask(id, StaggerAsk::Break { window: 2.5 }), "no stance said: no break (never the old 'always yes')");
    v.stance = VictimStance { breakable: true, broken: 0.0 };
    t.victims = vec![v];
    assert_eq!(t.size_class(id), 1, "a Small body: the normal gauge and window");
    assert!(t.stagger_ask(id, StaggerAsk::Break { window: 2.5 }));
    assert!(!t.stagger_ask(id, StaggerAsk::Break { window: 2.5 }), "one break at a time");
    assert!(t.stagger_ask(id, StaggerAsk::Hold { seconds: 3.0 }) && t.stagger_ask(id, StaggerAsk::Release { juggle: 0.0 }) && t.stagger_ask(id, StaggerAsk::Rebound { wait: 0.2 }));
    assert!(!t.stagger_ask(id, StaggerAsk::MayDefeat), "a Breaker never kills the player outright");
    assert!(!t.stagger_ask(id, StaggerAsk::Delay { seconds: 1.0, lost: 1.0 }));
    assert!(!t.stagger_ask(TargetId(7), StaggerAsk::Break { window: 2.5 }), "an Elden Ring id is none of its business");
    let mut sora = sora_at(Vec3::ZERO);
    let world = TestWorld::floor();
    for _ in 0..5 {
        sora.step(DT, &Input::default(), &world);
    }
    let done = stance_to_sora(&mut t, &mut sora, Vec3::new(160.0, 0.0, 0.0));
    assert_eq!(done, vec![StanceOrder::Break { chara: PartyChara::Sora, seconds: 2.5 }, StanceOrder::Hold { chara: PartyChara::Sora, seconds: 3.0 }, StanceOrder::Release { chara: PartyChara::Sora }, StanceOrder::Rebound { chara: PartyChara::Sora }]);
    assert!(t.orders.is_empty());
    assert!(!sora.stance().broken(), "broken, held, released");
    // the rebound is his weapon recoil (TresPlayerState_Reflect)
    let recoiled = (0..10).any(|_| sora.step(DT, &Input::default(), &world).state == StateId::Reflect);
    assert!(recoiled, "Sora bounces off the copy's shield");
    // broken already, or down: no break
    t.victims[0].stance = VictimStance { breakable: true, broken: 1.0 };
    assert!(!t.stagger_ask(id, StaggerAsk::Break { window: 2.5 }));
    t.victims[0].stance = VictimStance { breakable: false, broken: 0.0 };
    assert!(!t.stagger_ask(id, StaggerAsk::Break { window: 2.5 }));
}

/// The leader Sora's telegraph: the next keyblade window of his attack clip, then the open one (the enemy side's sensor rule).
#[test]
fn stance_soras_swing_is_seen_coming_then_open() {
    let mut sora = sora_at(Vec3::ZERO);
    let world = TestWorld::floor();
    for _ in 0..5 {
        sora.step(DT, &Input::default(), &world);
    }
    assert!(!victim_tell_of_sora(&sora, Vec3::new(160.0, 0.0, 0.0)).on, "standing: nothing coming");
    let mut before = PadButtons::NONE;
    let (mut coming, mut open) = (None, false);
    for k in 0..40 {
        let held: &[PadButton] = if k < 2 { &[PadButton::ATTACK] } else { &[] };
        let input = Input::default().with_pad(pad(held, before));
        before = input.pad.held;
        sora.step(DT, &input, &world);
        let tell = victim_tell_of_sora(&sora, Vec3::new(160.0, 0.0, 0.0));
        if tell.on && !tell.active && coming.is_none() {
            coming = Some(tell.eta);
        }
        open |= tell.active;
        if tell.on {
            let t = tell.telegraph(PartyChara::Sora);
            assert_eq!(t.attacker, victim_id(PartyChara::Sora));
        }
    }
    assert!(coming.is_some_and(|eta| eta > 0.0), "his swing was seen before its window: {coming:?}");
    assert!(open, "then its window open");
}
