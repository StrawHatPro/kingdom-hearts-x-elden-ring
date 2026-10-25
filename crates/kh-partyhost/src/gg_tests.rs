//! (GOOFY-GG) Goofy's GUARD gauge in the party host: the HUD bar's feed, its level growth through the shared EXP, the
//! level-up line, the save field (additive in v5), the save point / join refill, the Stats page row.

use crate::*;
use kh_core::host::TargetId;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Sora, Vec3};
use kh_host::party::PartyChara;

struct NoEnemies;
impl kh_core::Targets for NoEnemies {
    fn enumerate(&self, _out: &mut Vec<kh_core::host::TargetInfo>) {}
    fn info(&self, _id: TargetId) -> Option<kh_core::host::TargetInfo> {
        None
    }
    fn hurt_shapes(&self, _id: TargetId, _out: &mut Vec<kh_core::host::HurtShape>) {}
    fn apply_hit(&mut self, _hit: &kh_core::HitRecord) -> kh_core::host::HitResult {
        kh_core::host::HitResult::default()
    }
}
impl PartyTargets for NoEnemies {
    fn ai_actors(&self, _out: &mut Vec<kh_ai::ActorView>) {}
    fn attacked_by_enemies(&self) -> bool {
        false
    }
}

fn runtime() -> PartyRuntime {
    let mut rt = PartyRuntime::new(RuntimeConfig { wanted: true, test: PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack: None, ai_pack_source: None });
    rt.set_progression(true);
    rt
}

fn new_game_sora() -> Sora {
    let mut sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 });
    *sora.status_mut() = kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None));
    sora
}

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

fn goofy_guard(rt: &PartyRuntime) -> kh_party::Guard {
    rt.party.status(rt.id(PartyChara::Goofy).unwrap()).unwrap().guard.expect("Goofy's GUARD")
}

#[test]
fn the_hud_bar_is_goofys_while_he_leads_and_grows_with_his_level() {
    let world = TestWorld::floor();
    let mut sora = new_game_sora();
    let mut rt = runtime();
    step(&mut rt, &mut sora, &world);
    step(&mut rt, &mut sora, &world);
    let d = rt.id(PartyChara::Donald).unwrap();
    assert!(rt.party.status(d).unwrap().guard.is_none(), "Donald has no GUARD");
    assert!(rt.leader_guard_hud().is_none(), "Sora leads: no GUARD bar");
    rt.lead_word(&["leader", "goofy"]);
    step(&mut rt, &mut sora, &world);
    let (v, look) = rt.leader_guard_hud().expect("Goofy leads: his GUARD bar");
    assert_eq!((v.value, v.max, v.broken), (100.0, 100.0, false));
    assert_eq!(look.fill, kh_party::guard::GP.hud_color_fill);
    // the Stats page's third row: his GUARD
    let m = rt.menu_members().into_iter().find(|m| m.chara == PartyChara::Goofy).unwrap();
    assert_eq!(m.guard, Some((100, 100)));
    assert_eq!(rt.menu_members().into_iter().find(|m| m.chara == PartyChara::Donald).unwrap().guard, None);
    // a level-up through the shared EXP to his level 5: GUARD 105, a GUARD line for that window
    let exp5 = kh_party::tables::level_row(kh_party::Chara::Goofy, 5).0;
    sora.status_mut().exp = exp5;
    step(&mut rt, &mut sora, &world);
    let g = goofy_guard(&rt);
    assert_eq!((g.level, g.max, g.value), (5, 105.0, 105.0), "the rise is added: a full bar stays full");
    assert!(rt.level_ups.iter().any(|u| u.0 == PartyChara::Goofy && u.1 == 5));
    assert!(rt.take_guard_up(PartyChara::Goofy, 5));
    assert!(!rt.take_guard_up(PartyChara::Goofy, 5), "taken once");
    // Goofy's AI-run slot shows no GUARD bar (KH3's friend slots show HP and MP only)
    rt.lead_word(&["leader", "sora"]);
    step(&mut rt, &mut sora, &world);
    assert!(rt.leader_guard_hud().is_none());
}

#[test]
fn the_save_keeps_guard_additively_and_an_old_v5_record_reads_full() {
    let world = TestWorld::floor();
    let mut sora = new_game_sora();
    let mut rt = runtime();
    step(&mut rt, &mut sora, &world);
    let gid = rt.id(PartyChara::Goofy).unwrap();
    {
        let g = rt.party.status_mut(gid).unwrap().guard.as_mut().unwrap();
        g.absorb(60.0);
    }
    let record = rt.party_record();
    let friends = record["friends"].as_array().unwrap();
    let goofy = friends.iter().find(|f| f["chara"] == "Goofy").unwrap();
    assert_eq!(goofy["guard"], serde_json::json!([40.0, false]));
    assert!(friends.iter().find(|f| f["chara"] == "Donald").unwrap().get("guard").is_none(), "Donald writes no key");
    assert_eq!(record["friends_version"], 5, "no version change");
    // round trip
    let back = FriendRecord::from_json(goofy).unwrap();
    assert_eq!(back.guard, Some((40.0, false)));
    let mut s = kh_party::FriendStatus::new(kh_party::Chara::Goofy, 0);
    back.apply(&mut s);
    assert_eq!(s.guard.map(|g| (g.value, g.broken)), Some((40.0, false)));
    // broken survives
    let mut broken = back.clone();
    broken.guard = Some((10.0, true));
    let mut s = kh_party::FriendStatus::new(kh_party::Chara::Goofy, 0);
    FriendRecord::from_json(&broken.to_json()).unwrap().apply(&mut s);
    assert_eq!(s.guard.map(|g| (g.value, g.broken)), Some((10.0, true)));
    // a v5 record written before GUARD (no key): full GUARD, everything else as before
    let mut old = goofy.clone();
    old.as_object_mut().unwrap().remove("guard");
    let r = FriendRecord::from_json(&old).unwrap();
    assert_eq!(r.guard, None);
    let mut s = kh_party::FriendStatus::new(kh_party::Chara::Goofy, 0);
    s.guard.as_mut().unwrap().spend(50.0);
    r.apply(&mut s);
    assert_eq!(s.guard.map(|g| (g.value, g.max)), Some((100.0, 100.0)));
    let _ = world;
}

#[test]
fn a_save_point_and_a_join_refill_guard() {
    let world = TestWorld::floor();
    let mut sora = new_game_sora();
    let mut rt = runtime();
    step(&mut rt, &mut sora, &world);
    let gid = rt.id(PartyChara::Goofy).unwrap();
    rt.party.status_mut(gid).unwrap().guard.as_mut().unwrap().break_now();
    rt.savepoint_recover(&mut sora, 1);
    assert!(kh_party::guard::GP.guard_save_point_refills);
    assert_eq!(goofy_guard(&rt).value, 100.0);
    assert!(!goofy_guard(&rt).broken);
    rt.party.status_mut(gid).unwrap().guard.as_mut().unwrap().spend(70.0);
    let exp20 = kh_party::tables::level_row(kh_party::Chara::Goofy, 20).0;
    assert_eq!(rt.join_level(PartyChara::Goofy, exp20), Some((1, 20)));
    let g = goofy_guard(&rt);
    assert_eq!((g.max, g.value), (120.0, 120.0));
}
