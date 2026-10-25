//! (ER S0) kh-partyhost's own tests: the D5 `pending_ai` rule, the leader hand-over, the body seams, the v5 record,
//! the poser on the real packs (skipped without them). The ladder jump's tests are kh-party's (tests/ladder.rs).

use kh_core::host::{HitRecord, HitResult, HurtShape, TargetId, TargetInfo};
use kh_core::test_world::TestWorld;
use kh_core::{Input, Sora, Vec3};
use kh_host::party::{FriendAiSettings, PartyChara};

use crate::*;

/// No enemies.
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
}

impl PartyTargets for NoEnemies {
    fn ai_actors(&self, _out: &mut Vec<kh_ai::ActorView>) {}
    fn attacked_by_enemies(&self) -> bool {
        false
    }
}

fn runtime(wanted: bool) -> PartyRuntime {
    PartyRuntime::new(RuntimeConfig { wanted, test: PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack: None, ai_pack_source: None })
}

fn sora() -> Sora {
    Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 })
}

/// One host step without Sora's own step (the route, the party step, the after-step).
fn step(rt: &mut PartyRuntime, sora: &mut Sora, world: &TestWorld) {
    let camera = kh_camera::CameraView { position: Vec3::new(-600.0, 0.0, 200.0), rotation: Default::default(), rotation_target: Default::default(), look_point: Vec3::ZERO, distance: 600.0, auto_power: 0.0, stick_wait: 0.0, still_wait: 0.0, reset_wait: 0.0, slope_pitch: 0.0, stick_active: false, followed: false, collided: false };
    let mut targets = NoEnemies;
    let input = Input::default();
    let _ = rt.route(RouteIn { sora, input: &input, dt: 1.0 / 60.0, targets: &targets, camera: &camera, asked_formation: None, asked_ai: Vec::new(), nav: None, now: 0.0 });
    let out = sora.output();
    let mut audio = kh_core::host::audio::NullAudio;
    let mut effects = kh_core::host::effects::NullEffects;
    rt.step(StepIn { dt: 1.0 / 60.0, sora, out: &out, lock_on: TargetId::NONE, camera: &camera, world, targets: &mut targets, audio: &mut audio, effects: &mut effects, nav: None, now: 0.0 });
    let _ = rt.after_step(&NoEnemies, 1.0 / 60.0, 0.0);
}

#[test]
fn d5_a_friend_out_of_the_party_keeps_his_ai_settings() {
    let mut rt = runtime(false);
    rt.spawn_one(PartyChara::Goofy, Vec3::ZERO, 0.0);
    let mine = FriendAiSettings { battle: 2, ability: 1, cure: 0, cure_targets: 0b101 };
    rt.set_ai_settings(PartyChara::Goofy, mine);
    assert_eq!(rt.ai_settings(PartyChara::Goofy), mine);
    rt.remove_one(PartyChara::Goofy);
    assert!(rt.id(PartyChara::Goofy).is_none());
    assert_eq!(rt.ai_settings(PartyChara::Goofy), mine, "what the save's party record writes for him");
    rt.spawn_one(PartyChara::Goofy, Vec3::ZERO, 0.0);
    assert_eq!(rt.ai_settings(PartyChara::Goofy), mine, "back with him");
    // a save loaded while he is out: its settings wait for him
    rt.remove_one(PartyChara::Goofy);
    let loaded = FriendAiSettings { battle: 3, ability: 2, cure: 2, cure_targets: 0b010 };
    rt.set_ai_settings(PartyChara::Goofy, loaded);
    assert_eq!(rt.ai_settings(PartyChara::Goofy), loaded);
    rt.spawn_one(PartyChara::Goofy, Vec3::ZERO, 0.0);
    assert_eq!(rt.ai_settings(PartyChara::Goofy), loaded);
}

#[test]
fn the_leader_is_handed_over_and_back() {
    let world = TestWorld::floor();
    let mut sora = sora();
    let mut rt = runtime(true);
    step(&mut rt, &mut sora, &world);
    assert_eq!(rt.members.len(), 2, "both friends spawned at the first step");
    assert!(!rt.friend_leads());
    let donald = rt.id(PartyChara::Donald).unwrap();
    let goofy = rt.id(PartyChara::Goofy).unwrap();
    // Donald leads: the pad on him, AI Sora, the HUD's main slot and the bodies follow
    rt.lead_word(&["leader", "donald"]);
    step(&mut rt, &mut sora, &world);
    assert_eq!(rt.leader, Some(donald));
    assert!(rt.friend_leads());
    assert!(rt.party.pad_state(donald).is_some());
    assert!(rt.party.pad_state(goofy).is_none());
    assert!(sora.combat().friend.sora_is_ai);
    assert_eq!(rt.leader_chara(), PartyChara::Donald);
    assert_eq!(rt.lead.leader_state, (PartyChara::Donald, false));
    assert!(rt.leader_capsule().is_some());
    let lv = rt.leader_view_of(&sora.output(), sora.status());
    assert_eq!(lv.chara, PartyChara::Donald);
    assert_eq!(lv.member, Some(donald));
    let wants = rt.ally_wants(&sora.output(), sora.status());
    assert_eq!(wants.iter().map(|w| w.chara).collect::<Vec<_>>(), vec![PartyChara::Sora, PartyChara::Goofy], "AI Sora and Goofy need bodies, the leader does not");
    assert_eq!(rt.ai_members().iter().map(|m| m.0).collect::<Vec<_>>(), vec![PartyChara::Sora, PartyChara::Goofy]);
    // Goofy leads: Donald's pad goes
    rt.lead_word(&["leader", "goofy"]);
    step(&mut rt, &mut sora, &world);
    assert_eq!(rt.leader, Some(goofy));
    assert!(rt.party.pad_state(donald).is_none());
    assert!(rt.party.pad_state(goofy).is_some());
    // Donald out while Goofy leads: his settings are kept; in again
    let before = rt.ai_settings(PartyChara::Donald);
    rt.lead_word(&["formation", "out", "donald"]);
    step(&mut rt, &mut sora, &world);
    assert!(rt.id(PartyChara::Donald).is_none());
    assert_eq!(rt.ai_settings(PartyChara::Donald), before);
    assert_eq!(rt.lead.current_formation().active(), vec![PartyChara::Sora, PartyChara::Goofy]);
    rt.lead_word(&["formation", "in", "donald"]);
    step(&mut rt, &mut sora, &world);
    assert!(rt.id(PartyChara::Donald).is_some());
    // Sora leads again: no pad on a friend, Sora is no longer AI
    rt.lead_word(&["leader", "sora"]);
    step(&mut rt, &mut sora, &world);
    assert_eq!(rt.leader, None);
    assert!(!rt.friend_leads());
    assert!(rt.party.pad_state(goofy).is_none());
    assert!(!sora.combat().friend.sora_is_ai);
    assert_eq!(rt.lead.leader_state, (PartyChara::Sora, false));
    assert_eq!(rt.leader_view_of(&sora.output(), sora.status()).chara, PartyChara::Sora);
    assert_eq!(rt.ally_wants(&sora.output(), sora.status()).len(), 2);
}

#[test]
fn a_hit_through_the_leader_sink_reaches_the_leader() {
    let world = TestWorld::floor();
    let mut sora = sora();
    let mut rt = runtime(true);
    step(&mut rt, &mut sora, &world);
    rt.lead_word(&["leader", "goofy"]);
    step(&mut rt, &mut sora, &world);
    let goofy = rt.id(PartyChara::Goofy).unwrap();
    let hp = rt.party.view(goofy).unwrap().hp;
    let record = HitRecord { damage: 10.0, attack_name: "test.leader", ..HitRecord::empty(TargetId::NONE) };
    let mut sink = RuntimeLeaderSink { runtime: &mut rt, sora: &mut sora };
    sink.take_hit(&LeaderHit { sora: None, record });
    sink.follow_body(Vec3::new(500.0, 0.0, 0.0), 90.0);
    assert!(rt.party.view(goofy).unwrap().hp < hp, "Goofy took the blow");
    assert_eq!(rt.party.member(goofy).unwrap().position().x, 500.0);
}

#[test]
fn the_friends_record_survives_a_save_and_a_load() {
    let world = TestWorld::floor();
    let mut sora = sora();
    let mut rt = runtime(true);
    step(&mut rt, &mut sora, &world);
    let donald = rt.id(PartyChara::Donald).unwrap();
    {
        let s = rt.party.status_mut(donald).unwrap();
        s.bonuses.0[kh_core::status::BonusCounts::HP_UP5] += 2;
        s.items[1] = kh_party::ItemSlot { item: Some(kh_core::status::ItemKind::Elixir), present: true };
    }
    let saved = rt.to_save();
    assert_eq!(saved.friends.len(), 2);
    // through the v5 JSON shape
    let json: Vec<_> = saved.friends.iter().map(|f| f.to_json()).collect();
    let back: Vec<FriendRecord> = json.iter().filter_map(FriendRecord::from_json).collect();
    assert_eq!(back, saved.friends);
    // a new run: loaded before the friends spawn, applied at their spawn
    let mut rt2 = runtime(true);
    rt2.from_save(&PartySave { friends: back, ..saved.clone() });
    let mut sora2 = self::sora();
    step(&mut rt2, &mut sora2, &world);
    let d2 = rt2.id(PartyChara::Donald).unwrap();
    let s2 = rt2.party.status(d2).unwrap();
    assert_eq!(s2.bonuses.0[kh_core::status::BonusCounts::HP_UP5], rt.party.status(donald).unwrap().bonuses.0[kh_core::status::BonusCounts::HP_UP5]);
    assert_eq!(s2.items[1].item, Some(kh_core::status::ItemKind::Elixir));
    assert_eq!(FriendRecord::of(PartyChara::Donald, s2), saved.friends[0]);
}

#[test]
fn the_poser_poses_the_real_packs() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/party");
    for (id, npc) in [(poser::DONALD, "n_ex001"), (poser::GOOFY, "n_ex002")] {
        let dir = root.join(npc);
        if !dir.join("manifest.json").is_file() {
            println!("no {}: skipped", dir.display());
            return;
        }
        let read = |p: &std::path::Path| -> serde_json::Value { serde_json::from_str(std::fs::read_to_string(p).unwrap().trim_start_matches('\u{feff}')).unwrap() };
        let manifest = read(&dir.join("manifest.json"));
        let index = read(&root.join("party.json"));
        let entry = index["members"].as_array().into_iter().flatten().find(|m| m["member"].as_u64() == Some(id as u64)).cloned().unwrap_or(serde_json::Value::Null);
        let animator = kh_anim::Animator::new(kh_anim::Pack::load(&dir).unwrap(), kh_anim::Settings { leg_ik: false, ..kh_anim::Settings::default() }).unwrap();
        let mut m = PosedMember::new(id, animator, &manifest, &entry).unwrap();
        let mut poser = PartyPoser::default();
        for row in ["IdleN", "RunLoopN", "WalkRunN"] {
            let Ok(d) = m.debug_pose(row, 0.0) else { continue };
            m.debug = Some(d);
            let taps = m.step_debug(3, 1.0 / 60.0, &kh_anim::NoIkWorld);
            assert_eq!(taps.len(), 3);
            assert!(taps[2].bones.contains_key("atama"), "{npc} {row}: the head in the tap");
            assert!(m.current.component.iter().all(|t| t.translation.is_finite()));
        }
        poser.members.insert(id, m);
        let posed = poser.pose(id, 1.0).unwrap();
        assert_eq!(posed.local.len(), posed.component.len());
        assert!(poser.bone_world(id, "atama").is_some());
    }
}
