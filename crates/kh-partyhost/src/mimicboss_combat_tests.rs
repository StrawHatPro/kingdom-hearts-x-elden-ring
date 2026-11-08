//! (MIMIC-BOSS hotfix 3, MB3-COMBAT; THE USER 2026-11-06: "make sure mimic Donald get's properly hit stunned by Donald's
//! missiles from his standard combo. ... it does almost no damage to you when it does it's finisher move on you and goofy does
//! almost no damage to it when he does his finisher on it. Fix this. It also needs to play Goofy's animation for getting knocked
//! down when it's stagger breaks.") The copies' combat through the real party runtime, the real kh-core classification and
//! damage function, and KH3's own tables (party-levels, EnemyBaseStatus).

use kh_core::enums::{ETresDamageAttribute as Attr, ETresDamageKind as Kind};
use kh_core::host::{BodyReaction, HitInfo, HitRecord, HurtShape, ReactionResult, TargetBody, TargetId, TargetInfo};
use kh_core::status::{compute_damage, DamageAttack, DamageContext, Defender, LoadoutValue};
use kh_core::test_world::TestWorld;
use kh_core::{Input, Loadout, Pose, Sora, Status, Vec3, SORA_PARAMS};
use kh_host::formation::Formation;
use kh_host::party::PartyChara;
use kh_party::NpcStateId;

use crate::mimicboss::*;
use crate::*;

const DT: f32 = 1.0 / 60.0;

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

struct NoEnemies;

impl kh_core::Targets for NoEnemies {
    fn enumerate(&self, _out: &mut Vec<TargetInfo>) {}
    fn info(&self, _id: TargetId) -> Option<TargetInfo> {
        None
    }
    fn hurt_shapes(&self, _id: TargetId, _out: &mut Vec<HurtShape>) {}
    fn apply_hit(&mut self, _hit: &HitRecord) -> kh_core::HitResult {
        kh_core::HitResult::default()
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

fn camera() -> kh_camera::CameraView {
    kh_camera::CameraView { position: Vec3::new(-600.0, 0.0, 200.0), rotation: Default::default(), rotation_target: Default::default(), look_point: Vec3::ZERO, distance: 600.0, auto_power: 0.0, stick_wait: 0.0, still_wait: 0.0, reset_wait: 0.0, slope_pitch: 0.0, stick_active: false, followed: false, collided: false }
}

fn sora_at(p: Vec3) -> Sora {
    let mut s = Sora::new(SORA_PARAMS, Pose { position: p, yaw: 0.0 }).with_status(level99());
    s.command_mut().fill(&level99());
    s
}

fn party_step(rt: &mut PartyRuntime, sora: &mut Sora, world: &TestWorld, asked: Option<Formation>) {
    let cam = camera();
    let mut targets = NoEnemies;
    let input = Input::default();
    let _ = rt.route(RouteIn { sora, input: &input, dt: DT, targets: &targets, camera: &cam, asked_formation: asked, asked_ai: Vec::new(), nav: None, now: 0.0 });
    let out = sora.output();
    let mut audio = kh_core::host::audio::NullAudio;
    let mut effects = kh_core::host::effects::NullEffects;
    rt.step(StepIn { dt: DT, sora, out: &out, lock_on: TargetId::NONE, camera: &cam, world, targets: &mut targets, audio: &mut audio, effects: &mut effects, nav: None, now: 0.0 });
    let _ = rt.after_step(&NoEnemies, DT, 0.0);
}

/// The copy of friend `chara` from a real party of three led by him (no AI pack needed: no step of his tree here).
fn copy_of(chara: PartyChara) -> (FriendBoss, PartyRuntime) {
    let mut rt = PartyRuntime::new(RuntimeConfig { wanted: true, test: PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack: None, ai_pack_source: None });
    let mut sora = sora_at(Vec3::ZERO);
    let world = TestWorld::floor();
    party_step(&mut rt, &mut sora, &world, None);
    party_step(&mut rt, &mut sora, &world, Some(Formation::new(&[PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy], chara).unwrap()));
    for _ in 0..3 {
        party_step(&mut rt, &mut sora, &world, None);
    }
    let boss = FriendBoss::new(&rt, &sora, chara, Vec3::new(-700.0, 0.0, 0.0), 0.0).expect("the copy");
    (boss, rt)
}

fn state_of(boss: &FriendBoss) -> NpcStateId {
    boss.rt.party.member(boss.id().unwrap()).unwrap().state
}

fn reactions_of(boss: &FriendBoss) -> u32 {
    boss.rt.party.member(boss.id().unwrap()).unwrap().damage_reactions
}

/// Donald's combo missile as the Elden Ring host hands it on (kh-party donald_shots `deliver_seen`): kind Small, Fire, magic,
/// reaction power 1, CLASSIFIED ON THE BOSS'S ELDEN RING BODY: a STRONG body (`ErTargets::body` of an armoured boss, the user's
/// log: target 41 at 2940 HP, over `target.hp_armoured` 1500) with endurance left.
fn missile_on_the_boss_body(from: Vec3, to: Vec3) -> HitRecord {
    let mut h = HitRecord { position: to, direction: (to - from).normalize_or_zero(), damage: 8.0, damage_kind: Kind::Small, attribute: Attr::Fire, attack_name: "DON_MISSILE", ..HitRecord::empty(TargetId(41)) };
    h.attacker = kh_core::host::HitAttacker { position: from, yaw: 0.0, radius: 40.0, valid: true };
    h.info.reaction_power = 1;
    h.info.flags50 |= 1;
    let strong = TargetBody { reaction: BodyReaction::Strong, ..TargetBody::DEFAULT };
    let mut info = h.info;
    let c = kh_core::combat::classify::classify(&strong, kh_core::combat::classify::MapResponse::Other, &kh_core::combat::classify::Shape { no_clash: true, ..Default::default() }, kh_core::combat::classify::DamageClass { kind: h.damage_kind, attribute: h.attribute }, &mut info, &mut |_, _| 0, &mut |_| 3);
    h.info = info;
    assert_eq!(c.victim, ReactionResult::StrongBodyDamage1, "the boss's armoured body: a Small hit with endurance left");
    h
}

/// ITEM 8. The cause: the hit came with the Elden Ring body's STRONG verdict and kh-react refused the copy's reaction; the fix
/// judges it on the copy's own (DEFAULT) body: Donald's missile stuns a Donald copy, and so it does every copy.
#[test]
fn combat_donalds_combo_missile_stuns_a_donald_copy_and_every_copy() {
    let (mut old, _rt) = copy_of(PartyChara::Donald);
    let at = old.view().unwrap().position;
    let from = at + Vec3::new(600.0, 0.0, 0.0);
    let hit = missile_on_the_boss_body(from, at);
    // the old route (the hit as it came, no HP): no reaction
    let mut zero = hit;
    zero.damage = 0.0;
    let _ = old.rt.receive_hit(PartyChara::Donald, &zero);
    assert_eq!(reactions_of(&old), 0, "THE BUG: the boss body's STRONG verdict refused the copy's flinch ({:?})", state_of(&old));
    // the fix
    assert_eq!(copy_reaction_hit(&hit).info.victim_result, ReactionResult::Damage, "judged on the copy's own body");
    assert_eq!(copy_reaction_hit(&hit).damage, 0.0, "no HP: the copy's HP mirrors the game's");
    for chara in [PartyChara::Donald, PartyChara::Goofy] {
        let (mut boss, _rt) = copy_of(chara);
        let at = boss.view().unwrap().position;
        let hit = missile_on_the_boss_body(at + Vec3::new(600.0, 0.0, 0.0), at);
        let _ = boss.take_reaction(&hit);
        assert_eq!(state_of(&boss), NpcStateId::Damage, "{chara:?} copy: hit-stunned by the missile");
        assert_eq!(reactions_of(&boss), 1);
    }
    // a Sora copy: Sora's own damage state from the same hit (reaction_of)
    let world = TestWorld::floor();
    let mut core = copy_sora(&sora_at(Vec3::ZERO), Pose { position: Vec3::ZERO, yaw: 0.0 });
    for _ in 0..20 {
        core.step(DT, &Input::default(), &world);
    }
    core.take_hit(reaction_of(&missile_on_the_boss_body(Vec3::new(600.0, 0.0, 0.0), Vec3::ZERO)));
    let hurt = (0..10).any(|_| core.step(DT, &Input::default(), &world).state == kh_core::StateId::Damage);
    assert!(hurt, "a Sora copy flinches too");
}

/// The rows of EnemyBaseStatus.json (KH3's own table) the mirror Defense is.
#[test]
fn combat_the_mirror_defense_is_kh3s_enemy_base_status() {
    let path = format!("{}/../../sheets/entities/Load/Common/EnemyBaseStatus.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(std::fs::read_to_string(&path).unwrap().trim_start_matches('\u{feff}')).unwrap();
    let rows = json["rows"].as_array().expect("rows");
    let mut seen = 0;
    for r in rows {
        let level: u32 = r["id"].as_str().and_then(|s| s.parse().ok()).expect("id");
        assert_eq!(mirror_defense(level), r["m_BaseDefensePower"].as_i64().unwrap() as i32, "battle level {level}");
        seen += 1;
    }
    assert_eq!(seen, 99);
    assert_eq!((mirror_defense(0), mirror_defense(50), mirror_defense(200)), (8, 32, 57), "clamped to 1 .. 99");
}

/// A friend of level `level` by his own table (the shared EXP that gives it), with no gear (the user's test save: level 50).
fn friend_at(c: kh_party::Chara, level: u32) -> kh_party::FriendStatus {
    let (mut lo, mut hi) = (0, 50_000_000);
    while lo < hi {
        let mid = (lo + hi) / 2;
        if kh_party::tables::level_for_exp(c, mid) >= level {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    let st = kh_party::FriendStatus::new(c, lo);
    assert_eq!(st.level, level);
    st
}

fn target_defense(d: &Defender) -> kh_core::host::TargetDefense {
    kh_core::host::TargetDefense { defense: d.defense, max_hp: d.max_hp, hp_percent: d.hp_percent, enemy_kind: false, damage_min: d.damage_min, damage_max: d.damage_max, resist: d.resist, bio_type: d.bio_type }
}

fn breaker_hit(damage: f32) -> HitRecord {
    HitRecord { damage, damage_kind: Kind::Blow, attack_name: "GOOFY_BREAKER", ..HitRecord::empty(victim_id(PartyChara::Goofy)) }
}

/// ITEM 11 (b). Goofy's finisher on his copy: the cause (the copy's party Defense on top of Elden Ring's body defence: base 1,
/// the user's `GOOFY_BREAKER ... 8.0 KH`) and the fix (no second Defense, as on every Elden Ring body; its own resists kept).
#[test]
fn combat_goofys_breaker_on_his_copy_lands_its_kh3_damage() {
    let goofy = friend_at(kh_party::Chara::Goofy, 50);
    assert_eq!((goofy.strength(), goofy.defense()), (28, 48), "level 50 Goofy (party-balance-levels): a tank");
    let breaker = DamageAttack::physical(kh_party::goofy::K.breaker_power);
    let cx = DamageContext::new(goofy.game_level, 1);
    let own = target_defense(&goofy.as_defender());
    let old = compute_damage(&goofy.as_attacker(), &Defender::of_target(Some(&own)), &breaker, &cx).amount;
    assert_eq!(old, kh_party::goofy::K.breaker_power, "THE BUG: base 1, the Breaker its power x 1 (the user's 8.0 KH of 4680)");
    let foe = copy_as_foe(own);
    assert_eq!((foe.defense, foe.enemy_kind, foe.damage_max, foe.resist, foe.max_hp), (0, true, 255, own.resist, own.max_hp), "its own resists kept");
    let new = compute_damage(&goofy.as_attacker(), &Defender::of_target(Some(&foe)), &breaker, &cx).amount;
    assert_eq!(new, (goofy.strength() as f32 * kh_party::goofy::K.breaker_power).ceil(), "as on every Elden Ring boss: Strength x 8.0");
    let push = compute_damage(&goofy.as_attacker(), &Defender::of_target(Some(&foe)), &DamageAttack::physical(1.0), &cx).amount;
    assert_eq!(push, goofy.strength() as f32, "the other hits by the same rule");
    // a resistance still cuts (the user's hotfix-2 request)
    let mut fire_proof = own;
    fire_proof.resist[Attr::Fire as usize] = 50;
    let fire = DamageAttack { attribute: Attr::Fire, magic: true, ..DamageAttack::physical(2.0) };
    let donald = friend_at(kh_party::Chara::Donald, 50);
    let plain = compute_damage(&donald.as_attacker(), &Defender::of_target(Some(&copy_as_foe(own))), &fire, &cx).amount;
    let cut = compute_damage(&donald.as_attacker(), &Defender::of_target(Some(&copy_as_foe(fire_proof))), &fire, &cx).amount;
    assert!(cut < plain, "fire resist 50 %: {cut} < {plain}");
    eprintln!("Goofy L50 Breaker on his copy: {old} -> {new}; push {push}; Donald's Fire on it {plain} (50 % resist {cut})");
}

/// ITEM 11 (a). The copy's finisher on the leader: the cause (its party Strength against his party Defense through
/// `BossTargets::defense`: base 1, the user's `damage: 9.6` -> `Damaged { amount: 9 }` of 156) and the fix (KH3's base against
/// a foe of the copied level: EnemyBaseStatus's Defense and floor), for every copy.
#[test]
fn combat_the_copys_finisher_on_the_leader_lands_its_kh3_damage() {
    let goofy = friend_at(kh_party::Chara::Goofy, 50);
    let breaker = DamageAttack::physical(kh_party::goofy::K.breaker_power);
    let cx = DamageContext::new(goofy.game_level, 1);
    let t = BossTargets { victims: vec![crate::mimicboss::Victim { chara: PartyChara::Goofy, feet: Vec3::ZERO, yaw: 0.0, velocity: Vec3::ZERO, radius: 40.0, half_height: 90.0, hp: goofy.hp, hp_max: goofy.max_hp(), defense: goofy.defense(), airborne: false, down: false, tell: Default::default(), stance: Default::default() }], hits: Vec::new(), attack_rate: 1.2, orders: Vec::new() };
    let made = compute_damage(&goofy.as_attacker(), &Defender::of_target(kh_core::Targets::defense(&t, victim_id(PartyChara::Goofy)).as_ref()), &breaker, &cx).amount * t.attack_rate;
    assert!((made - 9.6).abs() < 1e-4, "THE BUG, the user's number: {made}");
    let mut hit = breaker_hit(made);
    assert_eq!(mirror_defense(50), 32);
    assert_eq!(foe_floor(50), 6, "1 + ceil(0.1 x 50)");
    assert_eq!(mirror_hit_on_leader(&mut hit, (goofy.strength(), goofy.magic()), 50, goofy.defense()), Some((made, 58.0)), "base max(28 - 32, 6) = 6: 8 x 6 x 1.2");
    assert!(hit.damage < goofy.max_hp() as f32, "a finisher, not a kill from full HP: {} of {}", hit.damage, goofy.max_hp());
    // his push (power 1): 1.2 -> 7.2 -> 8
    let mut push = HitRecord { damage: 1.2, ..breaker_hit(1.2) };
    assert_eq!(mirror_hit_on_leader(&mut push, (28, 0), 50, 48).map(|x| x.1), Some(8.0));
    // a Sora copy on a level-50 Sora (Strength 40, Defense 34): base 6 -> 8, a mild change
    let mut stab = HitRecord { damage: 7.2, ..breaker_hit(7.2) };
    assert_eq!(mirror_hit_on_leader(&mut stab, (40, 41), 50, 34).map(|x| x.1), Some(10.0));
    // a magic row takes Magic: a Donald copy on Donald (Magic 52, Defense 25): base 27 -> 20
    let donald = friend_at(kh_party::Chara::Donald, 50);
    let mut fire = HitRecord { damage: 54.0, ..breaker_hit(54.0) };
    fire.info.flags50 |= 1;
    assert_eq!(mirror_hit_on_leader(&mut fire, (donald.strength(), donald.magic()), 50, donald.defense()).map(|x| x.1), Some(40.0));
    // Zettaflare (forced) and a guarded hit are left alone
    let mut forced = breaker_hit(500.0);
    forced.flags |= kh_core::host::HIT_FLAG_FORCE;
    assert_eq!(mirror_hit_on_leader(&mut forced, (28, 0), 50, 48), None);
    let mut guarded = breaker_hit(9.6);
    guarded.info.victim_result = ReactionResult::Guard;
    assert_eq!(mirror_hit_on_leader(&mut guarded, (28, 0), 50, 48), None);
}

/// ITEM 12. A broken stance knocks each friend copy down with HIS OWN KH3 blow (Goofy's clips for Goofy, Donald's for Donald),
/// he gets up by his own get-up, and a Sora copy takes Sora's own blow.
#[test]
fn combat_a_broken_stance_knocks_each_copy_down_with_its_own_knockdown() {
    let world = TestWorld::floor();
    let cam = camera();
    for chara in [PartyChara::Goofy, PartyChara::Donald] {
        let (mut boss, _rt) = copy_of(chara);
        let at = boss.view().unwrap().position;
        assert!(boss.knock_down(at + Vec3::new(300.0, 0.0, 0.0)), "{chara:?}: the knockdown starts");
        assert_eq!(state_of(&boss), NpcStateId::Damage);
        let m = boss.rt.party.member(boss.id().unwrap()).unwrap();
        let first = m.anim.clip.name;
        assert!(kh_party::tables::anim_set(m.chara).get(first).is_some(), "{chara:?}: his own set's clip {first}");
        let mut clips = vec![first];
        let mut up = None;
        for k in 0..600 {
            let mut t = BossTargets { victims: Vec::new(), hits: Vec::new(), attack_rate: 1.0, orders: Vec::new() };
            let (mut audio, mut effects) = (kh_core::host::audio::NullAudio, kh_core::host::effects::NullEffects);
            boss.step(FriendBossStep { dt: DT, world: &world, targets: &mut t, audio: &mut audio, effects: &mut effects, camera: &cam, nav: None, now: k as f64 * DT as f64 });
            let m = boss.rt.party.member(boss.id().unwrap()).unwrap();
            if clips.last() != Some(&m.anim.clip.name) {
                clips.push(m.anim.clip.name);
            }
            if m.state != NpcStateId::Damage {
                up = Some(k);
                break;
            }
        }
        eprintln!("{chara:?} copy knocked down: {clips:?}, up after {up:?} steps");
        assert!(clips.len() >= 2, "{chara:?}: blow, then down / get-up: {clips:?}");
        assert!(up.is_some_and(|k| k > 30), "{chara:?}: down for a while, then up by his own get-up: {up:?}");
    }
    let hit = knockdown_hit(Vec3::ZERO, Vec3::new(300.0, 0.0, 0.0));
    assert_eq!((hit.damage_kind, hit.damage, hit.info.victim_result), (Kind::Blow, 0.0, ReactionResult::Damage));
    assert!(hit.info.flags50 & HitInfo::FLAG50_IGNORE_GUARD != 0, "the break opened him: not guardable");
    let mut core = copy_sora(&sora_at(Vec3::ZERO), Pose { position: Vec3::ZERO, yaw: 0.0 });
    for _ in 0..20 {
        core.step(DT, &Input::default(), &world);
    }
    let hp = core.status().hp;
    core.take_hit(knockdown_of_sora(Vec3::ZERO, Vec3::new(300.0, 0.0, 0.0)));
    let down = (0..30).any(|_| core.step(DT, &Input::default(), &world).state == kh_core::StateId::Damage);
    assert!(down, "a Sora copy: Sora's own blow");
    assert_eq!(core.status().hp, hp, "no HP");
}

