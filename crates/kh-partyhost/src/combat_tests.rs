//! (HUD-REVIVE-COMBAT, THE USER 2026-11-07: "if an enemy is actively fighting the party leader or any party member, the whole
//! party should still be in combat. Combat only ends when no enemy is targetting the party.") THE PARTY RUNTIME'S ONE STATE:
//! `PartyRuntime::party_combat` is the value `step` gives `Party::set_in_battle` (every member's battle state, the HUD's slots),
//! Sora's own flag while he leads, and while a friend leads AI Sora's own flag counts what the party's does (`SoraTargets::
//! engaged_count` adds the party aggro of the step). The host's engaged count is the whole party's (kh-er targets.rs
//! `engages_the_party`; kh-er combat_tests.rs proves it on Elden Ring's targets); here the host counts nothing and only the
//! runtime's own party aggro (an enemy within 8 m of a member) puts the party in battle. Skipped (passes) without the AI pack.

use kh_core::host::{HitRecord, HitResult, HurtShape, TargetId, TargetInfo};
use kh_core::test_world::TestWorld;
use kh_core::{Input, Sora, Vec3};

use crate::*;

const DT: f32 = 1.0 / 60.0;

/// One hostile actor (the trees' list and the party aggro); the host's own engaged count is always 0.
struct OneFoe {
    at: Vec3,
    alive: bool,
}

impl kh_core::Targets for OneFoe {
    fn enumerate(&self, _out: &mut Vec<TargetInfo>) {}
    fn info(&self, _id: TargetId) -> Option<TargetInfo> {
        None
    }
    fn hurt_shapes(&self, _id: TargetId, _out: &mut Vec<HurtShape>) {}
    fn apply_hit(&mut self, _hit: &HitRecord) -> HitResult {
        HitResult::default()
    }
    fn hit_bone(&self, _id: kh_core::TargetId, _at: kh_core::Vec3) -> Option<kh_core::host::TargetBone> {
        None
    }
    fn bone_pose(&self, _id: kh_core::TargetId, _bone: kh_core::host::TargetBone) -> Option<kh_core::host::BonePose> {
        None
    }
}

impl PartyTargets for OneFoe {
    fn ai_actors(&self, out: &mut Vec<kh_ai::ActorView>) {
        out.push(kh_ai::ActorView { actor: kh_ai::ActorRef::Enemy(1), position: self.at, center: self.at + Vec3::Z * 90.0, radius: 40.0, half_height: 90.0, alive: self.alive, hostile: true, lockable: true, hp: 100.0, hp_max: 100.0, ..kh_ai::ActorView::default() });
    }
    fn attacked_by_enemies(&self) -> bool {
        false
    }
}

fn project(p: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(p)
}

fn rows(path: &std::path::Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()).and_then(|j| j["rows"].as_array().cloned()).unwrap_or_default()
}

struct Bench {
    rt: PartyRuntime,
    sora: Sora,
    out: kh_core::Output,
    world: TestWorld,
    foe: OneFoe,
    t: f64,
}

impl Bench {
    fn new() -> Option<Bench> {
        let pack = kh_ai::AiPack::load_dir(&project("build/sora_pack/party/ai")).ok()?;
        let sheets = project("sheets/port");
        let rt = PartyRuntime::new(RuntimeConfig { wanted: true, test: PartyTest::from_rows(&rows(&sheets.join("party-test.json"))), weapons: PartyWeapon::from_rows(&rows(&sheets.join("party-weapons.json"))), ai_pack: Some(std::sync::Arc::new(pack)), ai_pack_source: None });
        let sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&kh_core::SORA_PARAMS)), yaw: 0.0 });
        let out = sora.output();
        Some(Bench { rt, sora, out, world: TestWorld::floor(), foe: OneFoe { at: Vec3::new(0.0, 5000.0, 0.0), alive: true }, t: 0.0 })
    }

    /// One fixed step in the hosts' order: route -> Sora's step with `SoraTargets` -> step -> after_step.
    fn step(&mut self) {
        let lp = self.rt.leader_view_of(&self.out, self.sora.status()).position;
        let camera = kh_camera::CameraView { position: lp - Vec3::new(450.0, 0.0, -220.0), rotation: kh_camera::Rot { pitch: -15.0, yaw: 0.0, roll: 0.0 }, rotation_target: kh_camera::Rot { pitch: -15.0, yaw: 0.0, roll: 0.0 }, look_point: lp, distance: 450.0, auto_power: 0.0, stick_wait: 0.0, still_wait: 0.0, reset_wait: 0.0, slope_pitch: 0.0, stick_active: false, followed: false, collided: false };
        let input = self.rt.route(RouteIn { sora: &mut self.sora, input: &Input::default(), dt: DT, targets: &self.foe, camera: &camera, asked_formation: None, asked_ai: Vec::new(), nav: None, now: self.t });
        let mut audio = kh_core::host::audio::NullAudio;
        let mut effects = kh_core::host::effects::NullEffects;
        let mut ui = kh_core::host::NullUi;
        if self.rt.lead.sora_active {
            let mut st = SoraTargets { targets: &mut self.foe, runtime: &mut self.rt };
            let mut host = kh_core::Host { world: &self.world, targets: &mut st, audio: &mut audio, effects: &mut effects, ui: &mut ui };
            self.out = self.sora.step_hosted(DT, &input, &mut host);
        }
        let _ = self.rt.step(StepIn { dt: DT, sora: &mut self.sora, out: &self.out, lock_on: TargetId::NONE, camera: &camera, world: &self.world, targets: &mut self.foe, audio: &mut audio, effects: &mut effects, nav: None, now: self.t });
        let _ = self.rt.after_step(&self.foe, DT, self.t);
        self.t += DT as f64;
    }

    fn steps(&mut self, n: usize) {
        for _ in 0..n {
            self.step();
        }
    }

    /// (party_combat, every member's battle state, every HUD slot's, Sora's own flag)
    fn readers(&self) -> (bool, bool, bool, bool) {
        let members = self.rt.members.iter().all(|(_, id)| self.rt.party.view(*id).is_some_and(|v| v.in_battle));
        let hud = self.rt.hud_slots().iter().all(|s| s.in_battle);
        (self.rt.party_combat(), members, hud, self.sora.actions().battle)
    }
}

#[test]
fn combat_party_combat_is_the_members_state_whoever_leads_and_ai_sora_follows_it() {
    let Some(mut b) = Bench::new() else {
        eprintln!("(skipped: no AI pack under build/sora_pack/party/ai)");
        return;
    };
    b.steps(30);
    // Sora leads: the party's state is his own flag (the host's count, nothing here)
    assert!(b.rt.leader.is_none());
    assert_eq!(b.readers(), (false, false, false, false));
    b.rt.lead_word(&["leader", "Donald"]);
    b.steps(60);
    assert!(b.rt.leader.is_some() && b.rt.lead.sora_active, "Donald leads, AI Sora in the party");
    assert_eq!(b.readers(), (false, false, false, false), "nobody near: out of combat");
    // a hostile 5 m from the leader: the party aggro (the host counts nothing): the whole party AND AI Sora's own flag
    let lead = b.rt.leader_view_of(&b.out, b.sora.status()).position;
    b.foe.at = lead + Vec3::new(500.0, 0.0, 0.0);
    b.steps(3);
    assert_eq!(b.readers(), (true, true, true, true), "in combat everywhere (AI Sora's flag counts the party's aggro)");
    // it dies: KH3's 1.0 s linger, then out everywhere
    b.foe.alive = false;
    b.steps(45);
    assert_eq!(b.readers(), (true, true, true, true), "0.75 s after: the linger holds");
    b.steps(30);
    assert_eq!(b.readers(), (false, false, false, false), "1.25 s after: out of combat everywhere");
}
