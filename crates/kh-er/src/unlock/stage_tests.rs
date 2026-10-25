//! (ER party S8) THE STAGE OFFLINE: Goofy's stage runtime (S1's party core with a stand-in Sora who leads, never stepped,
//! "in battle", at the anchor) over synthetic geometry (kh-core's TestWorld floor) with the real AI pack and sheets, and
//! the boss as the stage's only target (`StageTargets`). What it shows: Goofy goes for the boss and swings at it, every
//! hit of his is swallowed (accepted, no damage anywhere), he stays at the arena (no warp to anyone), and the boss's blow
//! by the no-body rule (a 0-damage record) starts his reaction without taking HP. No game, no window.

use glam::Vec3;
use kh_core::host::TargetInfo;
use kh_core::test_world::TestWorld;
use kh_core::{HitRecord, Input, NullHost, Pose, Sora, TargetFlags, TargetId, SORA_PARAMS};
use kh_host::formation::Formation;
use kh_host::party::PartyChara;

use super::host::{StageTargets, STAGE_BOSS};
use crate::party::glue::{FrameIn, HostRules, PartyCore};

const STEP: f32 = 1.0 / 60.0;

fn project(p: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(p)
}

fn rows(path: &std::path::Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()).and_then(|j| j["rows"].as_array().cloned()).unwrap_or_default()
}

fn boss_at(p: Vec3) -> TargetInfo {
    TargetInfo { id: STAGE_BOSS, position: p, center: p + Vec3::Z * 150.0, velocity: Vec3::ZERO, yaw: 180.0, radius: 120.0, half_height: 150.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY | TargetFlags::PRIORITY_HIGH), hp: 5000.0, hp_max: 5000.0 }
}

#[test]
fn goofy_fights_the_staged_boss_and_nothing_takes_damage() {
    let Ok(pack) = kh_ai::AiPack::load_dir(&project("build/sora_pack/party/ai")) else {
        eprintln!("SKIPPED: no AI pack at build/sora_pack/party/ai");
        return;
    };
    let sheets = project("sheets/port");
    let config = kh_partyhost::RuntimeConfig { wanted: true, test: kh_partyhost::PartyTest::from_rows(&rows(&sheets.join("party-test.json"))), weapons: kh_partyhost::PartyWeapon::from_rows(&rows(&sheets.join("party-weapons.json"))), ai_pack: Some(std::sync::Arc::new(pack)), ai_pack_source: None };
    let mut core = PartyCore::new(config, HostRules::DEFAULT);
    core.sink = Some(|t| if std::env::var_os("KH_PARTY_LOG").is_some() { eprintln!("{t}") });
    core.asked_formation = Formation::new(&[PartyChara::Sora, PartyChara::Goofy], PartyChara::Sora).ok();
    let world = TestWorld::floor();
    let mut null = NullHost::default();
    let ground = Sora::origin_height_on_floor(&SORA_PARAMS);
    // the boss at x 1000; the anchor 250 cm toward the "fog" (-x), Goofy's spot 500 cm toward it
    let boss = Vec3::new(1000.0, 0.0, 0.0);
    let anchor = Vec3::new(750.0, 0.0, ground);
    let mut stand_in = Sora::new(SORA_PARAMS, Pose { position: anchor, yaw: 0.0 });
    stand_in.actions_mut().battle = true;
    let mut targets = StageTargets { boss: Some(boss_at(boss)), swallowed: 0, who: None };
    let mut fresh = true;
    let mut closest = f32::MAX;
    let mut hp_before = None;
    let mut reacted = false;
    for k in 0..1800 {
        core.frame(FrameIn { on: true, fresh: std::mem::take(&mut fresh), relocated: false, hidden: None, body_action: false, body: None, pose: false, trace: false, dt: STEP, now: k as f64 * STEP as f64 });
        let out = stand_in.output();
        let _ = core.route(&mut stand_in, &Input::default(), STEP, &world, &targets, k as f64 * STEP as f64);
        core.step(&mut stand_in, &out, STEP, TargetId::NONE, &world, &kh_anim::NoIkWorld, &mut targets, &mut null.audio, &mut null.effects, None, k as f64 * STEP as f64);
        if k == 1 {
            // his stage spot (the host's teleport after the first step)
            let id = core.rt.id(PartyChara::Goofy).expect("Goofy is spawned in his stage runtime");
            let m = core.rt.party.member_mut(id).unwrap();
            m.cmc.location = Vec3::new(500.0, 0.0, m.cmc.location.z);
            m.cmc.stop_movement();
        }
        if let Some(v) = core.rt.view(PartyChara::Goofy) {
            closest = closest.min(Vec3::new(v.position.x - boss.x, v.position.y - boss.y, 0.0).length());
            // the boss's blow by the no-body rule at 20 s: a 0-damage record
            if k == 1200 {
                hp_before = core.rt.party.status(core.rt.id(PartyChara::Goofy).unwrap()).map(|s| s.hp);
                let record = HitRecord { position: v.position + Vec3::Z * 60.0, direction: Vec3::new(-1.0, 0.0, 0.0), damage: 0.0, damage_kind: kh_core::enums::ETresDamageKind::Small, attack_name: "er.stage", attacker: kh_core::host::HitAttacker { position: boss, yaw: 180.0, radius: 120.0, valid: true }, ..HitRecord::empty(STAGE_BOSS) };
                core.rt.receive_hit(PartyChara::Goofy, &record);
            }
            if (1201..1260).contains(&k) && format!("{:?}", v.state).contains("Damage") {
                reacted = true;
            }
        }
    }
    let goofy = core.rt.view(PartyChara::Goofy).expect("Goofy");
    let hp_after = core.rt.party.status(core.rt.id(PartyChara::Goofy).unwrap()).map(|s| s.hp);
    eprintln!("stage: closest {closest:.0} cm, swallowed {}, Goofy at ({:.0}, {:.0}), hp {:?} -> {:?}, reacted {reacted}", targets.swallowed, goofy.position.x, goofy.position.y, hp_before, hp_after);
    assert!(closest < 400.0, "Goofy closes in on the boss (closest {closest:.0} cm)");
    assert!(targets.swallowed > 0, "Goofy's hits land on the boss and are swallowed");
    assert!(Vec3::new(goofy.position.x - boss.x, goofy.position.y, 0.0).length() < 1500.0, "he stays at the arena");
    assert_eq!(hp_before, hp_after, "the boss's blow takes no HP");
    assert!(reacted, "the boss's blow starts his reaction");
}
