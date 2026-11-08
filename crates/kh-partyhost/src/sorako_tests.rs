//! (HUD-REVIVE SORAKO, THE USER 2026-11-07: "Make sure Sora as a party member is using a proper knocked down animation and
//! isn't just floating in air using his usual defeat animation.") AI SORA KNOCKED OUT WHILE A FRIEND LEADS, through the
//! party runtime: he lands on the floor and holds his down pose (kh-core states/die.rs, a party member's KO), his AI presses
//! nothing and never warps the KO'd body (a warp was a fresh core in Idle at 0 HP), `ai_sora_down` says so; the revive
//! (`revive_ai_sora`, the HUD-REVIVE policy's mechanism) plays the get-up, Idle follows, his AI acts again.

use kh_core::host::{HitRecord, HitResult, HurtShape, TargetId, TargetInfo};
use kh_core::states::{member_ko, HitOnPlayer};
use kh_core::test_world::TestWorld;
use kh_core::{Input, Output, Pose, Sora, StateId, Status, Vec3};
use kh_host::party::PartyChara;

use crate::*;

const DT: f32 = 1.0 / 60.0;

/// No enemy to hit, but (HUD-REVIVE) a fight going on: the host counts one engaged enemy, so the party is in combat and a downed
/// AI Sora stays down (out of combat he revives at once: progression.rs `ai_sora_revive_step`, the user's rule).
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
    fn hit_bone(&self, _id: kh_core::TargetId, _at: kh_core::Vec3) -> Option<kh_core::host::TargetBone> {
        None
    }
    fn bone_pose(&self, _id: kh_core::TargetId, _bone: kh_core::host::TargetBone) -> Option<kh_core::host::BonePose> {
        None
    }
    fn engaged_count(&self) -> u32 {
        1
    }
}

impl PartyTargets for NoEnemies {
    fn ai_actors(&self, _out: &mut Vec<kh_ai::ActorView>) {}
    fn attacked_by_enemies(&self) -> bool {
        false
    }
}

struct Bench {
    rt: PartyRuntime,
    sora: Sora,
    world: TestWorld,
    floor_z: f32,
}

impl Bench {
    /// The party with Donald leading; Sora (a real status: the placeholder takes no damage) beside him.
    fn donald_leads() -> Bench {
        let floor_z = Sora::origin_height_on_floor(&kh_core::SORA_PARAMS);
        let mut status = Status::default();
        status.placeholder = false;
        status.hp_base = 100;
        status.hp_max = 100;
        status.hp = 100;
        let sora = Sora::new(kh_core::SORA_PARAMS, Pose { position: Vec3::new(0.0, 0.0, floor_z), yaw: 0.0 }).with_status(status);
        let rt = PartyRuntime::new(RuntimeConfig { wanted: true, test: PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack: None, ai_pack_source: None });
        let mut b = Bench { rt, sora, world: TestWorld::floor(), floor_z };
        b.step();
        b.rt.lead_word(&["leader", "donald"]);
        b.step();
        assert_eq!(b.rt.leader_chara(), PartyChara::Donald);
        assert!(b.sora.combat().friend.sora_is_ai);
        b
    }

    /// One whole host step: the route (AI Sora's input), Sora's own step, the party's step and after-step. The camera at
    /// (-6 m, 0, 2 m) looks along +X: Sora far behind it is out of its view.
    fn step(&mut self) -> Output {
        let camera = kh_camera::CameraView { position: Vec3::new(-600.0, 0.0, 200.0), rotation: Default::default(), rotation_target: Default::default(), look_point: Vec3::ZERO, distance: 600.0, auto_power: 0.0, stick_wait: 0.0, still_wait: 0.0, reset_wait: 0.0, slope_pitch: 0.0, stick_active: false, followed: false, collided: false };
        let mut targets = NoEnemies;
        let input = self.rt.route(RouteIn { sora: &mut self.sora, input: &Input::default(), dt: DT, targets: &targets, camera: &camera, asked_formation: None, asked_ai: Vec::new(), nav: None, now: 0.0 });
        let out = self.sora.step(DT, &input, &self.world);
        let mut audio = kh_core::host::audio::NullAudio;
        let mut effects = kh_core::host::effects::NullEffects;
        self.rt.step(StepIn { dt: DT, sora: &mut self.sora, out: &out, lock_on: TargetId::NONE, camera: &camera, world: &self.world, targets: &mut targets, audio: &mut audio, effects: &mut effects, nav: None, now: 0.0 });
        let _ = self.rt.after_step(&NoEnemies, DT, 0.0);
        out
    }

    fn leader_at(&self) -> Vec3 {
        self.rt.leader.and_then(|l| self.rt.party.view(l)).map(|v| v.position).expect("Donald leads")
    }

    /// Sora put 40 m behind the leader (out of the camera's view: the friends' warp distance is 25 m), `up` cm over the floor.
    fn put_far(&mut self, up: f32) {
        let at = self.leader_at();
        self.sora.teleport(Pose { position: Vec3::new(at.x - 4000.0, at.y, self.floor_z + up), yaw: 0.0 });
    }
}

/// THE CONTROL: AI Sora alive, far behind the leader and unseen, is warped next to him (the friends' rule, `SoraAi::drive`).
#[test]
fn alive_and_far_ai_sora_warps_to_the_leader() {
    let mut b = Bench::donald_leads();
    b.put_far(0.0);
    let mut near = false;
    for _ in 0..120 {
        let out = b.step();
        near |= out.position.distance(b.leader_at()) < 600.0;
    }
    assert!(near, "warped behind the leader");
}

/// (HUD-REVIVE, THE USER 2026-11-07: "for Sora's downed animation as a party member, have him actually use his game over
/// animation, hovering in place") AI SORA KO'D IN THE AIR WHILE DONALD LEADS: KH3's game-over pose (`M_0270`, looped) hovering
/// where he went down (no throw, no fall), never warped, `ai_sora_down`; the revive in the air: gravity back, Idle, he lands;
/// `ai_sora_down` false, and his AI acts again (the far warp of the friends' rule brings him back).
#[test]
fn ai_sora_ko_d_in_the_air_hovers_in_his_game_over_pose_unwarped_and_stands_at_the_revive() {
    let mut b = Bench::donald_leads();
    b.put_far(300.0);
    for _ in 0..3 {
        b.step();
    }
    assert!(!b.sora.output().on_floor, "in the air");
    assert!(!b.rt.ai_sora_down(&b.sora));
    b.sora.status_mut().hp = 1;
    let at = b.sora.output().position;
    b.sora.take_hit(HitOnPlayer { direction: Vec3::new(-1.0, 0.0, 0.0), causer_yaw: 180.0, ..HitOnPlayer::new(kh_core::enums::ETresDamageKind::Small, 60.0, TargetId(1), at + Vec3::X * 150.0) });
    // 6 s down (longer than one 5 s round of the clip): far behind the leader and out of view (an alive Sora warps within
    // ~1 s), hovering where he went down
    let game_over = kh_core::AnimId::from_name(member_ko::GAME_OVER_CLIP);
    let mut went_down: Option<Vec3> = None;
    for k in 0..360 {
        let out = b.step();
        assert_ne!(out.state, StateId::Idle, "never stood up at 0 HP (step {k})");
        if out.state == StateId::Die {
            let here = *went_down.get_or_insert(out.position);
            assert!(out.position.distance(here) < 1.0, "hovering in place, never warped or thrown (step {k}: {:?} from {here:?})", out.position);
            assert_eq!(Some(out.anim.name), game_over, "the game-over clip (step {k})");
            assert_ne!(Some(out.anim.name), kh_core::AnimId::from_name(member_ko::DOWN_CLIP));
        }
    }
    let out = b.sora.output();
    assert_eq!((out.state, out.sub_step), (StateId::Die, 6), "his game-over hover");
    assert!(!out.on_floor && out.position.z > b.floor_z + 200.0, "in place in the air, not fallen: z {} (floor {})", out.position.z, b.floor_z);
    assert!(b.rt.ai_sora_down(&b.sora));
    // the revive: refused when not down, taken now
    assert!(b.rt.revive_ai_sora(&mut b.sora, 50));
    assert!(!b.rt.ai_sora_down(&b.sora));
    assert!(!b.rt.revive_ai_sora(&mut b.sora, 50), "not down any more");
    assert_eq!(b.sora.status().hp, 50);
    // revived in the air: gravity back and Idle at once (no get-up in the air), then he lands
    let left = (0..3).position(|_| b.step().state != StateId::Die);
    assert!(left.is_some(), "out of his KO at the revive (the state queue's finish takes a step)");
    let mut landed = false;
    for _ in 0..120 {
        landed |= b.step().on_floor;
    }
    assert!(landed, "he falls and lands");
    assert!(!b.sora.status().damage_invalid);
    // his AI acts again: the far warp brings him behind the leader
    let mut near = false;
    for _ in 0..150 {
        let out = b.step();
        near |= out.position.distance(b.leader_at()) < 600.0;
    }
    assert!(near, "AI Sora acts again after the get-up");
    assert_eq!(b.sora.status().hp, 50);
}

/// (HUD-REVIVE) AI SORA KO'D ON THE GROUND: the game-over pose looped in place on his spot (the clip's own hover, his root on
/// the floor), and at the revive the get-up clip and Idle.
#[test]
fn ai_sora_ko_d_on_the_ground_hovers_on_his_spot_and_gets_up() {
    let mut b = Bench::donald_leads();
    b.put_far(0.0);
    for _ in 0..30 {
        b.step();
    }
    b.sora.status_mut().hp = 1;
    let at = b.sora.output().position;
    b.sora.take_hit(HitOnPlayer { direction: Vec3::new(-1.0, 0.0, 0.0), causer_yaw: 180.0, ..HitOnPlayer::new(kh_core::enums::ETresDamageKind::Small, 60.0, TargetId(1), at + Vec3::X * 150.0) });
    let mut first = None;
    for _ in 0..400 {
        let out = b.step();
        if out.state == StateId::Die {
            let here = *first.get_or_insert(out.position);
            assert!(out.position.distance(here) < 1.0, "in place: {:?} from {here:?}", out.position);
        }
    }
    let out = b.sora.output();
    assert_eq!((out.state, out.sub_step), (StateId::Die, 6));
    assert_eq!(Some(out.anim.name), kh_core::AnimId::from_name(member_ko::GAME_OVER_CLIP));
    assert!((out.position.z - b.floor_z).abs() < 1.0, "his root stays on the floor (the hover is the clip's): z {} floor {}", out.position.z, b.floor_z);
    assert!(out.anim.time < 5.0, "looped: {}", out.anim.time);
    assert!(b.rt.revive_ai_sora(&mut b.sora, 60));
    let o = b.step();
    assert_eq!((o.state, o.sub_step), (StateId::Die, 5), "the get-up clip on the ground");
    let mut stood = false;
    for _ in 0..90 {
        stood |= b.step().state != StateId::Die;
    }
    assert!(stood && !b.sora.status().damage_invalid, "Idle after the get-up");
}

/// (HUD-REVIVE) THE ELDEN RING SIDE OF THE HOVER: the host draws AI Sora from his core (kh-er sora.rs `component_to_world(out)`)
/// and writes his invisible ally body TO the core's feet (`ally_wants` -> allybody_live.rs `write`, KH3 -> ER only, both
/// physics phases; nothing reads the body back into his core while he is down). While he hovers mid-air the want is his hover
/// spot, KO (the pool parks the body: unhittable, below the leader) and never hittable; at the revive it is his again.
#[test]
fn the_er_body_route_follows_the_hover_and_never_pins_him() {
    let mut b = Bench::donald_leads();
    b.put_far(300.0);
    for _ in 0..3 {
        b.step();
    }
    b.sora.status_mut().hp = 1;
    let at = b.sora.output().position;
    b.sora.take_hit(HitOnPlayer { direction: Vec3::new(-1.0, 0.0, 0.0), causer_yaw: 180.0, ..HitOnPlayer::new(kh_core::enums::ETresDamageKind::Small, 60.0, TargetId(1), at + Vec3::X * 150.0) });
    for _ in 0..240 {
        let out = b.step();
        let want = b.rt.ally_wants(&out, b.sora.status()).into_iter().find(|w| w.chara == PartyChara::Sora).expect("AI Sora's body want");
        if out.state == StateId::Die {
            assert!(want.feet.distance(out.position) < 1.0e-3, "the body is written to the core's hover spot");
            assert!(want.ko && !want.hittable, "KO: the pool parks the body, unhittable");
        }
    }
    let out = b.sora.output();
    assert_eq!((out.state, out.sub_step), (StateId::Die, 6));
    assert!(out.position.z > b.floor_z + 200.0, "still hovering in the air: nothing pinned him to the floor");
    assert!(b.rt.revive_ai_sora(&mut b.sora, 40));
    let out = b.step();
    let want = b.rt.ally_wants(&out, b.sora.status()).into_iter().find(|w| w.chara == PartyChara::Sora).unwrap();
    assert!(!want.ko && want.hittable, "up: the body is his again, hittable");
}

/// A KILL PLANE'S KO: the revive lifts the hold and puts him behind the leader standing (his body lay in the barrier).
#[test]
fn a_kill_plane_ko_is_revived_behind_the_leader() {
    let mut b = Bench::donald_leads();
    b.put_far(0.0);
    b.step();
    b.sora.kill_unconditional(b.sora.output().position + Vec3::X * 150.0);
    for _ in 0..120 {
        b.step();
    }
    assert!(b.rt.ai_sora_down(&b.sora) && b.sora.combat().kill_plane_held);
    assert!(b.rt.revive_ai_sora(&mut b.sora, 30));
    assert!(!b.sora.combat().kill_plane_held);
    let out = b.step();
    assert_eq!(b.sora.status().hp, 30, "his HP held no more");
    assert!(out.state != StateId::Die && out.position.distance(b.leader_at()) < 600.0, "standing behind the leader: {:?} at {:?}", out.state, out.position);
}

/// SORA LEADING: his death is the game over (kh-er death.rs): no AI Sora to revive.
#[test]
fn no_ai_sora_revive_while_sora_leads() {
    let mut b = Bench::donald_leads();
    b.rt.lead_word(&["leader", "sora"]);
    b.step();
    assert!(!b.rt.friend_leads());
    b.sora.status_mut().hp = 0;
    assert!(!b.rt.ai_sora_down(&b.sora));
    assert!(!b.rt.revive_ai_sora(&mut b.sora, 30));
    assert_eq!(b.sora.status().hp, 0);
}
