//! (MIMIC-BOSS) the Mimic Tear bosses' engine-free rules through the real party runtime, the real Sora core and the real
//! party AI (the AI pack at build/sora_pack/party/ai; the AI traces are skipped without it, said on stderr).

use kh_core::host::{HitRecord, HitResult, HurtShape, TargetId, TargetInfo};
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Loadout, Pose, Sora, Status, Vec3, SORA_PARAMS};
use kh_host::formation::Formation;
use kh_host::party::PartyChara;

use crate::mimicboss::*;


use crate::*;

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

fn camera() -> kh_camera::CameraView {
    kh_camera::CameraView { position: Vec3::new(-600.0, 0.0, 200.0), rotation: Default::default(), rotation_target: Default::default(), look_point: Vec3::ZERO, distance: 600.0, auto_power: 0.0, stick_wait: 0.0, still_wait: 0.0, reset_wait: 0.0, slope_pitch: 0.0, stick_active: false, followed: false, collided: false }
}

fn runtime(ai: bool) -> PartyRuntime {
    let pack = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/party/ai");
    let ai_pack = if ai { kh_ai::AiPack::load_dir(&pack).ok().map(std::sync::Arc::new) } else { None };
    PartyRuntime::new(RuntimeConfig { wanted: true, test: PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack, ai_pack_source: None })
}

fn sora_at(p: Vec3) -> Sora {
    let mut s = Sora::new(SORA_PARAMS, Pose { position: p, yaw: 0.0 }).with_status(level99());
    s.command_mut().fill(&level99());
    s
}

/// One host step of the party (route, step, after-step) with no enemies.
fn party_step(rt: &mut PartyRuntime, sora: &mut Sora, world: &TestWorld, asked: Option<Formation>) {
    let cam = camera();
    let mut targets = NoEnemies;
    let input = Input::default();
    let _ = rt.route(RouteIn { sora, input: &input, dt: 1.0 / 60.0, targets: &targets, camera: &cam, asked_formation: asked, asked_ai: Vec::new(), nav: None, now: 0.0 });
    let out = sora.output();
    let mut audio = kh_core::host::audio::NullAudio;
    let mut effects = kh_core::host::effects::NullEffects;
    rt.step(StepIn { dt: 1.0 / 60.0, sora, out: &out, lock_on: TargetId::NONE, camera: &cam, world, targets: &mut targets, audio: &mut audio, effects: &mut effects, nav: None, now: 0.0 });
    let _ = rt.after_step(&NoEnemies, 1.0 / 60.0, 0.0);
}

/// A party of three (Sora, Donald, Goofy) led by `leader`, spawned and stepped.
fn party(leader: PartyChara) -> (PartyRuntime, Sora, TestWorld) {
    let mut rt = runtime(false);
    let mut sora = sora_at(Vec3::ZERO);
    let world = TestWorld::floor();
    party_step(&mut rt, &mut sora, &world, None);
    party_step(&mut rt, &mut sora, &world, Some(Formation::new(&[PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy], leader).unwrap()));
    for _ in 0..3 {
        party_step(&mut rt, &mut sora, &world, None);
    }
    assert_eq!(rt.leader_chara(), leader);
    assert!(rt.id(PartyChara::Donald).is_some() && rt.id(PartyChara::Goofy).is_some());
    (rt, sora, world)
}

// ---------------------------------------------------------------------------------------------- the gate

#[test]
fn mimicboss_the_gate_is_the_two_mimic_tears_only() {
    let nokron = boss_of_npc(526_100_965).expect("the Mimic Tear (Nokron)");
    let stray = boss_of_npc(526_100_052).expect("the Stray Mimic Tear (Haligtree)");
    assert_eq!((nokron.key, nokron.entity, nokron.defeat_flag, nokron.fog_flag), ("mimic_tear", 12_020_850, 12_020_850, 12_022_855));
    assert_eq!((stray.key, stray.entity, stray.defeat_flag, stray.fog_flag), ("stray_mimic_tear", 30_200_800, 30_200_800, 30_202_805));
    assert_eq!(boss_of_entity(12_020_850), Some(nokron));
    assert_eq!(boss_of_entity(30_200_800), Some(stray));
    // the spirit ash's own Mimic Tear (BuddyParam 20700001's NpcParam), Godrick's soldier, Margit, Malenia, a Silver Tear
    // pool, Radahn: none of them
    for other in [100_000_010, 43_000_010, 21_300_014, 21_200_014, 33_201_065, 47_701_165, 0, -1] {
        assert!(boss_of_npc(other).is_none(), "{other} is no Mimic Tear boss");
    }
    for other in [12_020_800, 12_020_860, 30_200_801, 10_000_850] {
        assert!(boss_of_entity(other).is_none(), "{other} is no Mimic Tear boss");
    }
    // the arena: the pool and its room in, the fog's outside and the grace far away out
    assert!(nokron.in_arena(nokron.pool_at) && nokron.in_arena([1020.755, -618.381, 1129.591]), "Nokron's first-encounter region's centre");
    assert!(!nokron.in_arena([1184.54, -619.219, 1909.859]), "Nokron, Eternal City's grace");
    assert!(stray.in_arena(stray.pool_at) && stray.in_arena([-88.65, 1276.79, -299.61]), "the Stray's own place");
    assert!(!stray.in_arena([-105.101, 1338.619, -122.303]), "the Stray's escape point (60 m up the path)");
}

#[test]
fn mimicboss_the_fight_runs_only_past_the_fog_in_the_arena_alive_unbeaten() {
    let on = FightFacts { defeated: false, fog_passed: true, in_arena: true, leader_alive: true };
    assert!(fight_on(&on));
    for (f, end) in [(FightFacts { defeated: true, ..on }, SoloEnd::Win), (FightFacts { leader_alive: false, ..on }, SoloEnd::Death), (FightFacts { in_arena: false, ..on }, SoloEnd::Away), (FightFacts { fog_passed: false, ..on }, SoloEnd::Away)] {
        assert!(!fight_on(&f));
        assert_eq!(SoloEnd::of(&f), end, "{f:?}");
    }
    // a win and a death at once: the win (the game set the flag)
    assert_eq!(SoloEnd::of(&FightFacts { defeated: true, leader_alive: false, ..on }), SoloEnd::Win);
}

// ---------------------------------------------------------------------------------------------- the solo fight

#[test]
fn mimicboss_fog_gate_leaves_sora_alone_and_the_win_brings_the_friends_back_as_they_were() {
    let (mut rt, mut sora, world) = party(PartyChara::Sora);
    let before = rt.lead.formation.clone();
    let (don, gof) = (rt.id(PartyChara::Donald).unwrap(), rt.id(PartyChara::Goofy).unwrap());
    rt.party.status_mut(don).unwrap().hp = 7;
    rt.party.status_mut(gof).unwrap().mp = 3;
    let don_status = rt.party.status(don).unwrap().clone();
    let gof_status = rt.party.status(gof).unwrap().clone();
    assert!(rt.solo_begin(&mut sora, "Mimic Tear", 1.0));
    assert!(!rt.solo_begin(&mut sora, "Mimic Tear", 1.0), "once");
    assert!(rt.members.is_empty() && rt.hud_slots().is_empty(), "only the leader: no friend on the field or the HUD");
    assert_eq!(rt.lead.formation, Formation::new(&[PartyChara::Sora], PartyChara::Sora).unwrap());
    // no friend switched in: the menu's formation, a word, a load's set_formation path
    rt.set_formation(before.clone());
    rt.lead_word(&["formation", "in", "donald"]);
    rt.lead_word(&["leader", "goofy"]);
    for _ in 0..3 {
        party_step(&mut rt, &mut sora, &world, Some(before.clone()));
    }
    assert!(rt.members.is_empty() && rt.leader_chara() == PartyChara::Sora, "every switch refused inside");
    assert_eq!(rt.to_save().formation, before, "a save in the fight keeps the party from before the fog");
    assert!(rt.solo_end(&mut sora, SoloEnd::Win, 2.0));
    assert!(!rt.solo_end(&mut sora, SoloEnd::Win, 2.0), "once");
    assert_eq!(rt.lead.formation, before);
    let (don, gof) = (rt.id(PartyChara::Donald).expect("Donald back"), rt.id(PartyChara::Goofy).expect("Goofy back"));
    assert_eq!(rt.party.status(don).unwrap(), &don_status, "Donald exactly as he was (HP 7)");
    assert_eq!(rt.party.status(gof).unwrap(), &gof_status, "Goofy exactly as he was (MP 3)");
    let lead = sora.output().position;
    for id in [don, gof] {
        assert!(rt.party.view(id).unwrap().position.distance(lead) < 600.0, "beside the leader");
    }
    party_step(&mut rt, &mut sora, &world, None);
    assert_eq!(rt.hud_slots().len(), 2, "the HUD's friends back");
}

#[test]
fn mimicboss_death_and_away_bring_the_friends_back_and_a_load_drops_the_hold() {
    for why in [SoloEnd::Death, SoloEnd::Away] {
        let (mut rt, mut sora, _world) = party(PartyChara::Sora);
        let before = rt.lead.formation.clone();
        assert!(rt.solo_begin(&mut sora, "Stray Mimic Tear", 1.0));
        assert!(rt.members.is_empty());
        assert!(rt.solo_end(&mut sora, why, 2.0));
        assert_eq!(rt.lead.formation, before, "{why:?}");
        assert!(rt.id(PartyChara::Donald).is_some() && rt.id(PartyChara::Goofy).is_some(), "{why:?}");
    }
    // a load in the fight (quit and continue, a save loaded): the save's formation is the party, the hold is gone
    let (mut rt, mut sora, world) = party(PartyChara::Sora);
    let save = rt.to_save();
    assert!(rt.solo_begin(&mut sora, "Mimic Tear", 1.0));
    let mid = rt.to_save();
    assert_eq!(mid.formation, save.formation, "the save written in the fight");
    rt.from_save(&mid);
    assert!(rt.solo().is_none(), "the load dropped the hold");
    for _ in 0..3 {
        party_step(&mut rt, &mut sora, &world, None);
    }
    assert!(rt.id(PartyChara::Donald).is_some() && rt.id(PartyChara::Goofy).is_some(), "the loaded party has its friends");
}

#[test]
fn mimicboss_a_friend_leader_fights_alone_too_ai_sora_and_the_other_friend_leave() {
    for leader in [PartyChara::Donald, PartyChara::Goofy] {
        let (mut rt, mut sora, world) = party(leader);
        assert!(rt.lead.sora_active, "AI Sora in the party");
        sora.status_mut().hp = 11;
        assert!(rt.solo_begin(&mut sora, "Mimic Tear", 1.0));
        let other = if leader == PartyChara::Donald { PartyChara::Goofy } else { PartyChara::Donald };
        assert!(rt.id(other).is_none(), "{} out", other.name());
        assert!(rt.id(leader).is_some() && rt.leader_chara() == leader, "{} leads alone", leader.name());
        assert!(!rt.lead.sora_active, "AI Sora out (hidden, not stepped)");
        assert_eq!(rt.solo().unwrap().taken_out(), vec![other, PartyChara::Sora]);
        party_step(&mut rt, &mut sora, &world, None);
        assert_eq!(rt.hud_slots().len(), 1, "the HUD: the leader alone");
        rt.lead_word(&["leader", "sora"]);
        party_step(&mut rt, &mut sora, &world, None);
        assert_eq!(rt.leader_chara(), leader, "no swap inside");
        assert!(rt.solo_end(&mut sora, SoloEnd::Death, 2.0));
        assert!(rt.id(other).is_some() && rt.lead.sora_active, "both back");
        assert_eq!(sora.status().hp, 11, "AI Sora as he was");
        assert_eq!(rt.leader_chara(), leader);
    }
}

// ---------------------------------------------------------------------------------------------- the copy

#[test]
fn mimicboss_the_copy_is_the_leader_sora_whole_with_his_form_and_no_summon() {
    let mut lead = sora_at(Vec3::new(500.0, 0.0, 0.0));
    lead.status_mut().hp = 9;
    lead.status_mut().mp = 1;
    lead.status_mut().munny = 4242;
    lead.command_mut().spirit_ashes = [207_000, 200_000, 0, 0];
    lead.command_mut().summon.allowed = true;
    {
        let f = &mut lead.combat_mut().form;
        f.cmd = 0x94;
        f.level = 1;
        f.time_left = 12.5;
        f.time_full = 30.0;
        f.wear_form = 5;
    }
    let copy = copy_sora(&lead, Pose { position: Vec3::new(-300.0, 0.0, 0.0), yaw: 180.0 });
    let (a, b) = (lead.status(), copy.status());
    assert_eq!((b.weapon.as_str(), b.weapon_upgrade, b.level, &b.armour, &b.accessories, &b.magic), (a.weapon.as_str(), a.weapon_upgrade, a.level, &a.armour, &a.accessories, &a.magic), "his kit");
    assert_eq!(copy.command().magic_slots, lead.command().magic_slots);
    assert_eq!((b.hp, b.mp, b.focus, b.munny), (b.max_hp(), b.max_mp(), b.focus_max, 0), "whole, nothing of the moment");
    let f = &copy.combat().form;
    assert_eq!((f.cmd, f.level, f.wear_form), (0x94, 1, 5), "the form he was in, and its wear form");
    assert!((f.time_left - 12.5).abs() < 1e-3, "its clock as it stood");
    assert!(copy.output().position.distance(Vec3::new(-300.0, 0.0, 0.0)) < 1.0);
    assert_eq!(copy.command().spirit_ashes, [0; 4], "no ashes of its own (the boss's summon is the host's)");
    assert!(!copy.command().summon.allowed);
    // never a live link
    lead.status_mut().weapon = "WEP_KEYBLADE_SO_01".into();
    assert_ne!(copy.status().weapon, lead.status().weapon);
    assert!(describe_sora(&copy).contains("form cmd 148"));
}

#[test]
fn mimicboss_hp_mirror_the_summon_point_and_the_victim_ids() {
    assert_eq!(mirrored_hp(1000, 1000, 120), 120);
    assert_eq!(mirrored_hp(500, 1000, 120), 60);
    assert_eq!(mirrored_hp(1, 100_000, 120), 1, "never 0 while the game's body lives");
    assert_eq!(mirrored_hp(0, 1000, 120), 0, "the game's death is its KO");
    assert!(!summon_due(0.61, false) && summon_due(0.6, false) && summon_due(0.2, false));
    assert!(!summon_due(0.5, true), "once a fight");
    assert!(!summon_due(0.0, false), "not dead");
    for c in [PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy] {
        assert_eq!(victim_chara(victim_id(c)), Some(c));
    }
    assert_eq!(victim_chara(TargetId(7)), None, "an Elden Ring target is never a victim");
}

#[test]
fn mimicboss_the_copys_targets_are_the_party_and_its_hits_are_kept_for_the_host() {
    use kh_core::Targets;
    let v = crate::mimicboss::Victim { chara: PartyChara::Sora, feet: Vec3::new(100.0, 0.0, 0.0), yaw: 0.0, velocity: Vec3::ZERO, radius: 34.0, half_height: 90.0, hp: 50, hp_max: 100, defense: 20, airborne: false, down: false, tell: Default::default(), stance: Default::default() };
    let mut t = BossTargets { victims: vec![v], hits: Vec::new(), attack_rate: 1.2, orders: Vec::new() };
    let mut list = Vec::new();
    t.enumerate(&mut list);
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, victim_id(PartyChara::Sora));
    assert!(list[0].flags.has(kh_core::host::TargetFlags::LOCKABLE | kh_core::host::TargetFlags::HITTABLE));
    assert_eq!(t.defense(victim_id(PartyChara::Sora)).unwrap().defense, 20, "his own Defense in the copy's formula");
    let mut actors = Vec::new();
    t.ai_actors(&mut actors);
    assert!(actors[0].hostile && actors[0].lockable, "the copy's AI sees the leader as its enemy");
    let mut hit = HitRecord::empty(victim_id(PartyChara::Sora));
    hit.damage = 10.0;
    assert!(t.apply_hit(&hit).accepted);
    assert!((t.hits[0].damage - 12.0).abs() < 1e-4, "x the boss's attack rate");
    // a downed victim is no target; an Elden Ring id never one
    t.victims[0].down = true;
    assert!(t.info(victim_id(PartyChara::Sora)).is_none());
    assert!(!t.apply_hit(&HitRecord { target: TargetId(3), ..hit }).accepted);
}

// ---------------------------------------------------------------------------------------------- the AI traces (the real trees)

fn victim_of(c: PartyChara, s: &Sora) -> crate::mimicboss::Victim {
    let o = s.output();
    let st = s.status();
    crate::mimicboss::Victim { chara: c, feet: o.position, yaw: o.yaw, velocity: o.velocity, radius: 34.0, half_height: 90.0, hp: st.hp, hp_max: st.max_hp(), defense: st.defense(), airborne: !o.on_floor, down: st.hp < 1, tell: Default::default(), stance: Default::default() }
}

fn victim_of_member(v: &kh_party::MemberView, defense: i32) -> crate::mimicboss::Victim {
    let c = crate::runtime::party_chara(v.chara);
    crate::mimicboss::Victim { chara: c, feet: v.position, yaw: v.yaw, velocity: v.velocity, radius: v.capsule.0.max(30.0), half_height: v.capsule.1.max(60.0), hp: v.hp, hp_max: v.hp_max, defense, airborne: !v.on_ground, down: v.ko, tell: Default::default(), stance: Default::default() }
}

/// The boss copy (Sora) against the leader Sora for `steps` fixed steps: (the copy's hits that reached him, the copy's core).
fn sora_duel(rt: &PartyRuntime, lead: &mut Sora, steps: usize) -> (usize, Sora) {
    let world = TestWorld::floor();
    let cam = camera();
    let mut boss = copy_sora(lead, Pose { position: Vec3::ZERO, yaw: 0.0 });
    let mut ai = BossAi::new(rt);
    assert!(ai.has_tree());
    let mut landed = 0;
    let dt = 1.0 / 60.0;
    for k in 0..steps {
        let mut t = BossTargets { victims: vec![victim_of(PartyChara::Sora, lead)], hits: Vec::new(), attack_rate: 1.0, orders: Vec::new() };
        let input = ai.input(rt, &mut boss, BossScene { targets: &t, camera: &cam, camera_yaw: 0.0, nav: None, dt, now: k as f64 * dt as f64 });
        assert!(!boss.command().summon.allowed && boss.command().spirit_ashes == [0; 4], "the copy never summons by itself");
        let out = {
            let (mut audio, mut effects, mut ui) = (kh_core::host::audio::NullAudio, kh_core::host::effects::NullEffects, kh_core::host::NullUi);
            let mut host = kh_core::Host { world: &world, targets: &mut t, audio: &mut audio, effects: &mut effects, ui: &mut ui };
            boss.step_hosted(dt, &input, &mut host)
        };
        for h in t.hits.drain(..) {
            assert_eq!(h.target, victim_id(PartyChara::Sora));
            lead.take_hit(player_hit_of(&h, out.position, out.yaw));
            landed += 1;
        }
        lead.step(dt, &Input::default(), &world);
    }
    (landed, boss)
}

#[test]
fn mimicboss_a_sora_copy_hunts_the_leader_with_ai_soras_brain_and_its_hits_land_on_him() {
    let rt = runtime(true);
    if rt.ai_pack.is_none() {
        eprintln!("no AI pack at build/sora_pack/party/ai: skipped");
        return;
    }
    let mut lead = sora_at(Vec3::new(700.0, 0.0, 0.0));
    let hp0 = lead.status().hp;
    let (landed, boss) = sora_duel(&rt, &mut lead, 900);
    assert!(landed > 0, "the copy reached the leader and hit him (15 s)");
    assert!(lead.status().hp < hp0, "through his own damage path: {} -> {}", hp0, lead.status().hp);
    assert!(boss.output().position.distance(lead.output().position) < 400.0, "it closed in on him");
}

#[test]
fn mimicboss_the_leaders_hits_make_the_copy_react_kh3_style_and_the_games_death_is_its_ko() {
    let world = TestWorld::floor();
    let lead = sora_at(Vec3::new(150.0, 0.0, 0.0));
    let mut boss = copy_sora(&lead, Pose { position: Vec3::ZERO, yaw: 0.0 });
    let dt = 1.0 / 60.0;
    for _ in 0..30 {
        boss.step(dt, &Input::default(), &world);
    }
    let hp = boss.status().hp;
    // a launcher (TRES_DMG_KIND_LAUNCH): up into the air, KH3's own reaction of the copy; its HP is the game's (mirrored)
    let mut launch = HitRecord::empty(TargetId(1));
    launch.damage_kind = kh_core::enums::ETresDamageKind::Launch;
    launch.direction = Vec3::new(-1.0, 0.0, 0.0);
    launch.damage = 40.0;
    launch.attacker = kh_core::host::HitAttacker { position: lead.output().position, yaw: 180.0, radius: 34.0, valid: true };
    boss.take_hit(reaction_of(&launch));
    let mut top: f32 = 0.0;
    let mut hurt = false;
    for _ in 0..40 {
        let o = boss.step(dt, &Input::default(), &world);
        top = top.max(o.position.z);
        hurt |= o.state == kh_core::StateId::Damage;
    }
    assert!(hurt, "its damage state");
    assert!(top > 30.0, "launched: {top:.0} cm up");
    assert_eq!(boss.status().hp, hp, "the reaction takes no HP (the game's body has the HP)");
    boss.status_mut().hp = mirrored_hp(400, 1000, boss.status().max_hp());
    assert!(boss.status().hp < hp);
    // the game's body died: the lethal blow, its KO
    // (the host gives it again every 10 steps until the KO: a hit in its invincible frames is not taken)
    let mut died = false;
    for k in 0..240 {
        if k % 10 == 0 {
            knock_out(&mut boss, Vec3::new(100.0, 0.0, 0.0));
        }
        died |= boss.step(dt, &Input::default(), &world).state == kh_core::StateId::Die;
    }
    assert!(died && boss.status().hp <= 0, "KO: died {died}, hp {}, state {:?}", boss.status().hp, boss.output().state);
}

#[test]
fn mimicboss_a_boss_copy_has_no_timer_it_fights_on_past_twice_the_ash_mimics_time() {
    let rt = runtime(true);
    if rt.ai_pack.is_none() {
        eprintln!("no AI pack: skipped");
        return;
    }
    // the spirit ash Mimic Tear's longest time (+10, kh-core summon::duration of either tier)
    let ash = kh_core::command::summon::duration(true, 10).max(kh_core::command::summon::duration(false, 10));
    let steps = (2.0 * ash * 60.0) as usize;
    let mut lead = sora_at(Vec3::new(700.0, 0.0, 0.0));
    lead.status_mut().hp_limit_one = true; // (the leader stands the whole run)
    let (landed, boss) = sora_duel(&rt, &mut lead, steps);
    assert!(landed > 0, "still fighting");
    assert!(boss.status().hp > 0, "nothing ended it: {:.0} s = 2 x the ash's {ash:.0} s", steps as f32 / 60.0);
}

/// A friend leader (`leader`) of a real party and his boss copy: (the copy's hits that reached him).
fn friend_duel(leader: PartyChara, steps: usize) -> Option<usize> {
    let mut rt = runtime(true);
    rt.ai_pack.as_ref()?;
    let mut sora = sora_at(Vec3::ZERO);
    let world = TestWorld::floor();
    party_step(&mut rt, &mut sora, &world, None);
    party_step(&mut rt, &mut sora, &world, Some(Formation::new(&[PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy], leader).unwrap()));
    for _ in 0..3 {
        party_step(&mut rt, &mut sora, &world, None);
    }
    assert!(rt.solo_begin(&mut sora, "Mimic Tear", 0.0));
    let lead_id = rt.id(leader).unwrap();
    let mut boss = FriendBoss::new(&rt, &sora, leader, Vec3::new(-700.0, 0.0, 0.0), 0.0).expect("the copy");
    assert_eq!(boss.rt.party.status(boss.id().unwrap()).unwrap().weapon, rt.party.status(lead_id).unwrap().weapon, "his weapon");
    assert_eq!(boss.rt.party.status(boss.id().unwrap()).unwrap().level, rt.party.status(lead_id).unwrap().level, "his level");
    let cam = camera();
    let mut landed = 0;
    let dt = 1.0 / 60.0;
    for k in 0..steps {
        let lv = rt.current.iter().find(|v| v.id == lead_id).copied().unwrap();
        let mut t = BossTargets { victims: vec![victim_of_member(&lv, rt.party.status(lead_id).unwrap().defense())], hits: Vec::new(), attack_rate: 1.0, orders: Vec::new() };
        let (mut audio, mut effects) = (kh_core::host::audio::NullAudio, kh_core::host::effects::NullEffects);
        boss.step(FriendBossStep { dt, world: &world, targets: &mut t, audio: &mut audio, effects: &mut effects, camera: &cam, nav: None, now: k as f64 * dt as f64 });
        for h in t.hits.drain(..) {
            assert_eq!(h.target, victim_id(leader));
            let _ = rt.receive_hit(leader, &h);
            landed += 1;
        }
        party_step(&mut rt, &mut sora, &world, None);
    }
    Some(landed)
}

#[test]
fn mimicboss_a_donald_copy_and_a_goofy_copy_fight_their_leader_with_their_own_moves() {
    for leader in [PartyChara::Donald, PartyChara::Goofy] {
        match friend_duel(leader, 1200) {
            None => {
                eprintln!("no AI pack: skipped");
                return;
            }
            Some(landed) => assert!(landed > 0, "{}'s copy hit him (20 s)", leader.name()),
        }
    }
}

// ---------------------------------------------------------------------------------------------- the boss's summon (the gate)

fn ash(goods: u32, level: u8, mimic: bool) -> kh_core::command::summon::AshSlot {
    kh_core::command::summon::AshSlot { goods, level, name: format!("ash {goods}"), mp: 10, renowned: mimic, mimic, usable: true, icon: None }
}

#[test]
fn mimicboss_only_a_sora_copy_summons_its_leaders_equipped_ash_at_its_level() {
    let mut sora = sora_at(Vec3::ZERO);
    // the player's Sora with his Summon command open, the Mimic Tear Ashes +4 in slot 1, the Lone Wolf +2 in slot 2
    sora.command_mut().summon.allowed = true;
    sora.command_mut().summon.slots = [Some(ash(207_004, 4, true)), Some(ash(200_002, 2, false)), None, None];
    let a = boss_summon_of(PartyChara::Sora, &sora).expect("a Sora copy summons");
    assert_eq!((a.goods, a.level), (207_004, 4), "the first equipped slot, at the player's own level");
    assert_eq!(boss_summon_kind(&a), BossSummon::MimicTear { level: 4, renowned: true, goods: 207_004 }, "its own Mimic Tear");
    // another ash first: Elden Ring's spirit of it
    sora.command_mut().summon.slots = [None, Some(ash(200_002, 2, false)), None, None];
    let b = boss_summon_of(PartyChara::Sora, &sora).unwrap();
    assert_eq!(boss_summon_kind(&b), BossSummon::Spirit { goods: 200_002, level: 2 });
    // nothing equipped: nothing
    sora.command_mut().summon.slots = [None, None, None, None];
    assert!(boss_summon_of(PartyChara::Sora, &sora).is_none());
    // Donald and Goofy have no Summon command: their copies never summon, whatever the player's Sora has equipped
    sora.command_mut().summon.slots = [Some(ash(207_000, 0, true)), None, None, None];
    for c in [PartyChara::Donald, PartyChara::Goofy] {
        assert!(boss_summon_of(c, &sora).is_none(), "{} copy", c.name());
    }
    // the command closed (a friend led: kh-er opens Summon only for the player's own Sora): none either
    sora.command_mut().summon.allowed = false;
    assert!(boss_summon_of(PartyChara::Sora, &sora).is_none());
}

#[test]
fn mimicboss_neither_copy_nor_the_bosss_mimic_ever_opens_summon_itself() {
    // the boss's copy: its own Summon rules shut at the copy and every AI step (BossAi keeps them shut)
    let mut lead = sora_at(Vec3::ZERO);
    lead.command_mut().summon.allowed = true;
    lead.command_mut().spirit_ashes = [207_000, 0, 0, 0];
    let copy = copy_sora(&lead, Pose { position: Vec3::ZERO, yaw: 0.0 });
    assert!(!copy.command().summon.allowed && copy.command().spirit_ashes == [0; 4]);
    assert!(boss_summon_of(PartyChara::Sora, &copy).is_none(), "a copy's own menu: no Summon (no recursion: copy -> Mimic -> copy)");
    // the boss's summoned Mimic Tear is the spirit route's copy (mimic.rs copy_kit): no Summon of its own
    let kit = crate::mimic::copy_kit(&lead, 3);
    let mimic = crate::mimic::make_core(&kit, Pose { position: Vec3::ZERO, yaw: 0.0 });
    assert!(!mimic.command().summon.allowed && mimic.command().spirit_ashes == [0; 4]);
    assert!(boss_summon_of(PartyChara::Sora, &mimic).is_none());
}

// ---------------------------------------------------------------------------------------------- the live arena (the user's run)

/// (MIMIC-BOSS, the user's run 2026-11-06 at Nokron: "SORA DID NOT APPEAR", "GOOFY AND DONALD STAYED IN MY PARTY AFTER
/// ENTERING FOG GATE") the log's own numbers: the leader at map-local (10.0, -1.5, -14.0), the boss's body at (17.33, -1.50,
/// -17.01), its MSB nest (1029.65, -641.31, 1174.02), fog flag 1: the fight MUST start. The MSB box said "out" (the bug).
#[test]
fn mimicboss_the_users_nokron_numbers_start_the_fight_on_the_live_boss_body() {
    let nokron = boss_of_npc(526_100_965).unwrap();
    let (leader, body, nest) = ([10.0, -1.5, -14.0], [17.33, -1.50, -17.01], [1029.65, -641.31, 1174.02]);
    assert!(!nokron.in_arena(leader), "the old MSB-frame box: out (why the fight never ran)");
    assert!(nokron.boss_at.iter().zip(nest).all(|(a, b)| (a - b).abs() < 0.01), "the log printed the MSB nest to 2 places");
    assert!(nokron.near_boss(body, leader), "the live rule: 7.9 m from the boss's own body");
    let facts = FightFacts { defeated: false, fog_passed: true, in_arena: nokron.near_boss(body, leader), leader_alive: true };
    assert!(fight_on(&facts), "fog 1, not defeated, alive, by the boss: THE FIGHT STARTS");
    // the later places of the same log (the leader moving round the boss): still in
    for at in [[10.0, -1.5, -14.1], [11.8, -1.5, -17.7], [24.2, -1.5, -18.5], [24.8, -1.5, -17.9]] {
        assert!(nokron.near_boss(body, at), "{at:?}");
        assert!(!nokron.away_from_boss(body, at), "{at:?}");
    }
    // away: a respawn at the grace hundreds of metres off, or a floor far below
    assert!(nokron.away_from_boss(body, [300.0, -1.5, -17.0]));
    assert!(nokron.away_from_boss(body, [17.0, -60.0, -17.0]));
    // the margin: just past the radius is not yet "away" (a fight running never ends at the edge)
    let edge = [body[0] + nokron.arena_radius + 5.0, body[1], body[2]];
    assert!(!nokron.near_boss(body, edge) && !nokron.away_from_boss(body, edge));
}

#[test]
fn mimicboss_the_stray_mimic_tear_is_measured_the_same_way() {
    let stray = boss_of_npc(526_100_052).unwrap();
    // any frame: the leader 6 m from the live body is in, whatever the MSB's places say
    let body = [3.0, 0.5, -20.0];
    assert!(stray.near_boss(body, [7.0, 0.5, -24.0]));
    assert!(fight_on(&FightFacts { defeated: false, fog_passed: true, in_arena: stray.near_boss(body, [7.0, 0.5, -24.0]), leader_alive: true }));
    assert!(stray.away_from_boss(body, [3.0, 0.5, 200.0]), "the grace's side, far up the path");
    assert_eq!((stray.first_flag, stray.fog_flag), (None, 30_202_805), "no first encounter without a fog");
    let nokron = boss_of_npc(526_100_965).unwrap();
    assert_eq!(nokron.first_flag, Some(12_020_851), "Nokron's first visit: its event's own flag");
}

// ---------------------------------------------------------------------------------------------- hotfix 2 (HP, resistances, map, abilities)

/// The least shared EXP that puts a friend at `level` on his own table.
fn friend_exp_for(chara: kh_party::Chara, level: u32) -> i32 {
    let (mut lo, mut hi) = (0, 50_000_000);
    while lo < hi {
        let mid = (lo + hi) / 2;
        if kh_party::tables::level_for_exp(chara, mid) >= level {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    lo
}

/// (THE USER 2026-11-06: "x30 the health then") the boss's HP for a level-50 Sora, Donald and Goofy: 30 x each one's own KH3
/// maximum, its base / 3 (the resident SpEffect 19397), the sheet's default 30.
#[test]
fn mimicboss_the_boss_hp_is_thirty_times_the_copied_leaders_own_maximum() {
    assert_eq!(BOSS_HP_X_LEADER, 30.0);
    assert_eq!(boss_hp(115, 30.0), 3450, "the user's own example: a level-50 Sora at 115 HP");
    assert_eq!(boss_base_hp(3450), 1150, "x the resident maxHpRate 3 = 3450 (a later journey's rate on top)");
    let sora = level99();
    let _ = sora;
    let mut seen = Vec::new();
    for c in [kh_party::Chara::Donald, kh_party::Chara::Goofy] {
        let exp = friend_exp_for(c, 50);
        let st = kh_party::FriendStatus::new(c, exp);
        assert_eq!(st.level, 50, "{c:?} at level 50 by his own table ({exp} EXP)");
        let want = boss_hp(st.max_hp(), BOSS_HP_X_LEADER);
        assert_eq!(want, st.max_hp() * 30);
        seen.push((c, st.max_hp(), want));
    }
    eprintln!("level 50: Sora 115 -> 3450; {}", seen.iter().map(|(c, m, w)| format!("{c:?} {m} -> {w}")).collect::<Vec<_>>().join("; "));
    // the boss-summoned Mimic Tear's body: the boss's maximum over its own SpEffects' rates
    assert_eq!(base_for(3450, 1037, 1037), 3450, "no rate of its own: the base is the maximum");
    assert_eq!(base_for(3450, 1000, 1500), 2300, "x1.5 of its own: the base makes 3450");
}

/// (THE USER: "Mimic tear needs to copy your abilities, both boss and summon") Second Chance / Withstand Combo on the boss's
/// game HP: the body's no-death switch while KH3's own rule would leave 1 HP.
#[test]
fn mimicboss_a_copys_second_chance_and_withstand_combo_hold_the_games_hp_at_one() {
    use kh_core::enums::ETresAbilityKind as A;
    let mut set = kh_core::status::AbilitySet::EMPTY;
    assert!(!holds_no_dead(&set, true, 500), "no ability: the game's death");
    set.set(A::LastLeave, 1, 1);
    assert!(holds_no_dead(&set, false, 500), "Second Chance above 1 HP");
    assert!(!holds_no_dead(&set, false, 1), "at 1 HP it no longer saves (KH3: HP > 1)");
    set.set(A::ComboLeave, 1, 1);
    assert!(holds_no_dead(&set, true, 1), "Withstand Combo in its window, even at 1 HP");
    assert!(!holds_no_dead(&set, false, 1));
    assert!(!holds_no_dead(&set, true, 0), "dead is dead");
}

/// The abilities are copied whole, both a boss copy (a clone) and a summoned Mimic (the spirit route's kit), and they act on
/// the copy's own state: the copy's Second Chance keeps it at 1 HP once from more; its Combo Plus lengthens its combo.
#[test]
fn mimicboss_copies_take_the_leaders_equipped_abilities_and_use_them() {
    use kh_core::enums::ETresAbilityKind as A;
    let lead = sora_at(Vec3::ZERO);
    let boss = copy_sora(&lead, Pose { position: Vec3::X * 300.0, yaw: 0.0 });
    let mimic = crate::mimic::make_core(&crate::mimic::copy_kit(&lead, 0), Pose { position: Vec3::X * 300.0, yaw: 0.0 });
    for k in [A::LastLeave, A::ComboLeave, A::ComboPlus, A::AircomboPlus, A::MpHaste] {
        assert_eq!(boss.status().abilities.count(k), lead.status().abilities.count(k), "boss copy {k:?}");
        assert_eq!(mimic.status().abilities.count(k), lead.status().abilities.count(k), "summoned Mimic {k:?}");
    }
    assert_eq!(boss.status().max_combo(false), lead.status().max_combo(false), "Combo Plus: the copy's own combo length");
    // Second Chance on a copy's own KH3 HP (the player's Mimic: its HP is the leader's kit): a lethal blow from 50 HP leaves 1
    let mut m = mimic;
    if m.status().abilities.count(A::LastLeave) == 0 {
        m.status_mut().abilities.set(A::LastLeave, 1, 1);
    }
    m.status_mut().hp = 50;
    let world = TestWorld::floor();
    for _ in 0..10 {
        m.step(1.0 / 60.0, &Input::default(), &world);
    }
    m.take_hit(kh_core::states::HitOnPlayer::new(kh_core::enums::ETresDamageKind::Small, 9999.0, TargetId::NONE, Vec3::new(100.0, 0.0, 0.0)));
    for _ in 0..5 {
        m.step(1.0 / 60.0, &Input::default(), &world);
    }
    assert_eq!(m.status().hp, 1, "Second Chance: 1 HP left");
}

/// (THE USER: "make sure the proper resistences are applied to the mimic boss") the copy's own KH3 defence in the party's
/// formula: a fire-resistant leader's copy takes less fire damage than a plain one.
#[test]
fn mimicboss_a_fire_resistant_leaders_copy_takes_reduced_fire_damage() {
    use kh_core::enums::ETresDamageAttribute as E;
    use kh_core::status::{compute_damage, Attacker, DamageAttack, DamageContext, Defender, Side};
    let plain = sora_at(Vec3::ZERO);
    let base = copy_defense(plain.status());
    let mut resistant = base;
    let fire = E::Fire as usize;
    resistant.resist[fire] = 50;
    let attacker = Attacker { has_status: true, strength: 60, magic: 60, abilities: None, side: Side::Player, state_boost: false };
    let fire_hit = DamageAttack { attribute: E::Fire, magic: true, ..DamageAttack::physical(2.0) };
    let cx = DamageContext::of(plain.status());
    let a = compute_damage(&attacker, &Defender::of_target(Some(&base)), &fire_hit, &cx).amount;
    let b = compute_damage(&attacker, &Defender::of_target(Some(&resistant)), &fire_hit, &cx).amount;
    assert!(b < a, "fire resistance 50 %: {b} < {a}");
    assert_eq!(base.defense, plain.status().defense(), "the leader's own Defense");
}

#[test]
fn mimicboss_the_fog_gate_is_known_by_the_map_block_before_the_body_is_in_the_world() {
    assert_eq!(boss_of_block(0x0C02_0000).map(|b| b.key), Some("mimic_tear"));
    assert_eq!(boss_of_block(0x1E14_0000).map(|b| b.key), Some("stray_mimic_tear"));
    assert_eq!(boss_of_block(0x0C07_0000), None, "Nokron, Eternal City's grace map (m12_07): not the arena's");
    assert_eq!(boss_of_block(0), None);
    assert!(INERT_ON_A_COPY.len() >= 5);
}

/// (HUD-REVIVE consistency) A copy KO'd before its AI's first step takes a party member's Die: the lying KO (`M_0230`), never
/// the leader's Die (it waits for a game over) nor AI Sora's game-over hover (`M_0270`), even cloned from AI Sora with his
/// hover flag on; Mimic Sora's own core (make_core) the same.
#[test]
fn a_fresh_copy_knocked_out_at_once_lies_down_as_a_party_member() {
    use kh_core::AnimId;
    let world = kh_core::test_world::TestWorld::floor();
    let mut leader = sora_at(Vec3::ZERO);
    // the leader as AI Sora while a friend leads: his hover flag on
    leader.combat_mut().friend.sora_is_ai = true;
    leader.combat_mut().friend.ko_game_over_pose = true;
    let boss = copy_sora(&leader, Pose { position: Vec3::new(300.0, 0.0, 0.0), yaw: 0.0 });
    let kit = crate::mimic::copy_kit(&leader, 0);
    let mimic = crate::mimic::make_core(&kit, Pose { position: Vec3::new(-300.0, 0.0, 0.0), yaw: 0.0 });
    let down = AnimId::from_name("M_0230") /* kh-core die.rs member::DOWN_CLIP */.expect("M_0230 in the table");
    for (name, mut core) in [("the boss's copy", boss), ("Mimic Sora", mimic)] {
        assert!(core.combat().friend.sora_is_ai && !core.combat().friend.ko_game_over_pose, "{name}: a member, no hover");
        assert!(core.status().hp > 0, "{name}: alive before its KO");
        let at = core.output().position;
        knock_out(&mut core, at + Vec3::X * 100.0);
        let mut lay = false;
        for _ in 0..(4 * 60) {
            let out = core.step(1.0 / 60.0, &Input::default(), &world);
            lay |= out.anim.name == down;
            assert_ne!(AnimId::from_name("M_0270") /* member::GAME_OVER_CLIP */, Some(out.anim.name), "{name}: never the hover");
        }
        assert!(lay, "{name}: the member's lying KO (M_0230)");
    }
}

// ------------------------------------------------------------------------------------------------ (REVENGE) KH3's player rules

/// One host step of a Sora copy with `held` buttons (pressed on the step they start).
fn copy_step(boss: &mut Sora, world: &TestWorld, held: &[kh_core::PadButton], before: &mut kh_core::PadButtons) -> kh_core::Output {
    let now = held.iter().fold(kh_core::PadButtons::NONE, |all, b| all.with(*b));
    let input = Input::default().with_pad(kh_core::PadState::from_held(now, *before));
    *before = now;
    boss.step(1.0 / 60.0, &input, world)
}

/// A hit of the leader's on the copy from in front of it (it faces +X, the leader stands at +X).
fn leader_hit(lead: &Sora, kind: kh_core::enums::ETresDamageKind) -> HitRecord {
    let mut h = HitRecord::empty(TargetId(1));
    h.damage_kind = kind;
    h.direction = Vec3::new(-1.0, 0.0, 0.0);
    h.damage = 20.0;
    h.attacker = kh_core::host::HitAttacker { position: lead.output().position, yaw: 180.0, radius: 34.0, valid: true };
    h
}

/// (REVENGE, THE USER 2026-11-07: "make the mimic tear boss stagger whenever Sora would stagger") A Sora copy in the middle of its
/// own combo is knocked back by a hit that knocks Sora back (KH3's player rules, `reaction_of`): no boss armour on a copy.
#[test]
fn revenge_a_sora_copy_hit_mid_combo_staggers_as_sora_would() {
    let world = TestWorld::floor();
    let lead = sora_at(Vec3::new(150.0, 0.0, 0.0));
    let mut boss = copy_sora(&lead, Pose { position: Vec3::ZERO, yaw: 0.0 });
    let mut held = kh_core::PadButtons::NONE;
    for _ in 0..20 {
        copy_step(&mut boss, &world, &[], &mut held);
    }
    // its combo: Attack pressed every other step until a swing runs
    let mut swinging = false;
    for k in 0..40 {
        let press: &[kh_core::PadButton] = if k % 2 == 0 { &[kh_core::PadButton::ATTACK] } else { &[] };
        copy_step(&mut boss, &world, press, &mut held);
        if boss.attack().is_some() && k > 6 {
            swinging = true;
            break;
        }
    }
    assert!(swinging, "the copy is in its combo");
    boss.take_hit(reaction_of(&leader_hit(&lead, kh_core::enums::ETresDamageKind::Blow)));
    let hurt = (0..40).any(|_| copy_step(&mut boss, &world, &[], &mut held).state == kh_core::StateId::Damage);
    assert!(hurt, "a blow that knocks Sora back knocks the copy back mid-combo");
}

/// (REVENGE) A hit Sora would shrug off leaves the copy alone as well: its guard (KH3's player guard) takes a frontal hit.
#[test]
fn revenge_a_sora_copy_guarding_shrugs_off_what_sora_shrugs_off() {
    let world = TestWorld::floor();
    let lead = sora_at(Vec3::new(150.0, 0.0, 0.0));
    let mut boss = copy_sora(&lead, Pose { position: Vec3::ZERO, yaw: 0.0 });
    let mut held = kh_core::PadButtons::NONE;
    for _ in 0..20 {
        copy_step(&mut boss, &world, &[], &mut held);
    }
    let mut guarding = false;
    for _ in 0..10 {
        guarding |= copy_step(&mut boss, &world, &[kh_core::PadButton::GUARD], &mut held).state == kh_core::StateId::Guard;
    }
    assert!(guarding, "the copy guards");
    boss.take_hit(reaction_of(&leader_hit(&lead, kh_core::enums::ETresDamageKind::Small)));
    let hurt = (0..30).any(|_| copy_step(&mut boss, &world, &[kh_core::PadButton::GUARD], &mut held).state == kh_core::StateId::Damage);
    assert!(!hurt, "guarded: no hit reaction, as Sora");
    // the control: the same hit on a copy that does not guard is a hit reaction
    let mut open = copy_sora(&lead, Pose { position: Vec3::ZERO, yaw: 0.0 });
    let mut held = kh_core::PadButtons::NONE;
    for _ in 0..20 {
        copy_step(&mut open, &world, &[], &mut held);
    }
    open.take_hit(reaction_of(&leader_hit(&lead, kh_core::enums::ETresDamageKind::Small)));
    assert!((0..30).any(|_| copy_step(&mut open, &world, &[], &mut held).state == kh_core::StateId::Damage), "unguarded: it flinches");
}
