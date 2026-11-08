//! (HUD-REVIVE SORAKO, THE USER 2026-11-07: "Make sure Sora as a party member is using a proper knocked down animation and
//! isn't just floating in air using his usual defeat animation.") A PARTY MEMBER'S KO on an AI core
//! (`combat.friend.sora_is_ai`, kh-core states/die.rs): thrown as KH3's Die, on the ground the down pose (`M_0230` held), on
//! HP > 0 the blow family's get-up (`0213` / `0216`, invincible) and Idle. The leader's Die (the flag false) is KH3's own and
//! unchanged: the flight loop held, waiting for the host's game over.

use kh_core::enums::ETresDamageKind as Kind;
use kh_core::states::member_ko as member;
use kh_core::states::HitOnPlayer;
use kh_core::test_world::TestWorld;
use kh_core::{AnimId, Input, Output, Pose, Sora, StateId, Status, StatusRequest, TargetId, Vec3, SORA_PARAMS};

const DT: f32 = 1.0 / 60.0;

struct Bench {
    sora: Sora,
    world: TestWorld,
    floor_z: f32,
}

impl Bench {
    /// Sora on the test floor (or `up` cm above it), an AI core when `ai`.
    fn new(ai: bool, up: f32) -> Bench {
        let floor_z = Sora::origin_height_on_floor(&SORA_PARAMS);
        let mut status = Status::default();
        status.placeholder = false; // a real status (the placeholder takes no damage, no recover)
        status.hp_base = 100;
        status.hp_max = 100;
        status.hp = 100;
        let mut sora = Sora::new(SORA_PARAMS, Pose { position: Vec3::new(0.0, 0.0, floor_z + up), yaw: 0.0 }).with_status(status);
        sora.combat_mut().friend.sora_is_ai = ai;
        Bench { sora, world: TestWorld::floor(), floor_z }
    }

    fn step(&mut self) -> Output {
        self.sora.step(DT, &Input::default(), &self.world)
    }

    /// A lethal hit from 150 cm in front of him (he faces +X), then the steps until the Die state.
    fn knock_out(&mut self) -> Output {
        self.sora.status_mut().hp = 1;
        let at = self.sora.output().position;
        self.sora.take_hit(HitOnPlayer { direction: Vec3::new(-1.0, 0.0, 0.0), causer_yaw: 180.0, ..HitOnPlayer::new(Kind::Small, 60.0, TargetId(1), at + Vec3::X * 150.0) });
        for _ in 0..10 {
            let out = self.step();
            if out.state == StateId::Die {
                return out;
            }
        }
        panic!("no Die state after a lethal hit: {:?}", self.sora.output().state);
    }
}

fn clip(name: &str) -> AnimId {
    AnimId::from_name(name).unwrap_or_else(|| panic!("clip {name} in the generated table"))
}

/// AI SORA KO'D IN MID-AIR (a friend leads): he falls as KH3's Die throws him, lands ON THE FLOOR and holds the down pose there
/// (not the floating flight loop); HP above 0 -> the get-up clip, invincible, then Idle, no stale Die.
#[test]
fn an_ai_member_ko_d_in_the_air_lies_on_the_floor_and_gets_up_at_hp() {
    let mut b = Bench::new(true, 300.0);
    for _ in 0..5 {
        b.step();
    }
    assert!(!b.sora.output().on_floor, "in the air at the hit");
    b.knock_out();
    assert!(b.sora.status().hp < 1);
    let flight = [clip("0212"), clip("0215")];
    let down = clip(member::DOWN_CLIP);
    // the fall: the flight pose in the air, the down pose from the landing on
    let mut landed_at = None;
    for k in 0..240 {
        let out = b.step();
        assert_eq!(out.state, StateId::Die);
        if out.on_floor && landed_at.is_none() {
            landed_at = Some(k);
        }
        if !out.on_floor {
            assert_ne!(out.anim.name, down, "no down pose before the landing (step {k})");
        }
    }
    assert!(landed_at.is_some(), "he lands");
    let out = b.step();
    assert!(out.on_floor && (out.position.z - b.floor_z).abs() < 1.0, "ON THE FLOOR, not floating: z {} (floor {})", out.position.z, b.floor_z);
    assert_eq!(out.sub_step, 4, "the member's down");
    assert_eq!(out.anim.name, down, "the down pose, not the flight loop");
    assert!(!flight.contains(&out.anim.name));
    assert!((out.anim.time - member::DOWN_POSE_TIME).abs() < 1.0e-4, "held at its flattest frame: {}", out.anim.time);
    // it holds for as long as he is down (no timer: the revive policy is the host's)
    for _ in 0..600 {
        let o = b.step();
        assert_eq!((o.state, o.sub_step, o.anim.name), (StateId::Die, 4, down));
        assert!((o.anim.time - member::DOWN_POSE_TIME).abs() < 1.0e-4);
        assert!(o.velocity.length() < 1.0, "still on the floor");
    }
    // HP above 0: the get-up from the down pose, invincible, then Idle
    b.sora.status_mut().hp = 40;
    let o = b.step();
    assert_eq!((o.state, o.sub_step), (StateId::Die, 5), "the get-up");
    assert!(o.anim.name == clip("0213") || o.anim.name == clip("0216"), "the blow family's get-up: {:?}", o.anim.name);
    assert!(o.anim.time >= member::GET_UP_FROM);
    assert!(b.sora.status().damage_invalid, "invincible while he gets up");
    let at = b.sora.output().position;
    b.sora.take_hit(HitOnPlayer::new(Kind::Small, 30.0, TargetId(1), at + Vec3::X * 150.0));
    b.step();
    assert_eq!(b.sora.status().hp, 40, "no hit lands during the get-up");
    let mut idle = None;
    for k in 0..120 {
        if b.step().state != StateId::Die {
            idle = Some(k);
            break;
        }
    }
    assert!(idle.is_some_and(|k| k < 60), "up within the clip's 0.63 s: {idle:?}");
    let out = b.step();
    assert_eq!(out.state, StateId::Idle, "Idle after the get-up");
    assert!(!b.sora.status().damage_invalid, "the invincibility goes with the state");
    assert_eq!(b.sora.status().hp, 40);
}

/// A SAVE POINT'S RECOVER (a `Recover` request, as kh-partyhost progression.rs `savepoint_recover` sends) on a down AI Sora:
/// the same get-up; `FullRecover` too.
#[test]
fn a_recover_request_gets_a_down_member_up() {
    for full in [false, true] {
        let mut b = Bench::new(true, 0.0);
        b.step();
        b.knock_out();
        for _ in 0..120 {
            b.step();
        }
        assert_eq!(b.sora.output().sub_step, 4, "down on the floor");
        b.sora.status_request(if full { StatusRequest::FullRecover } else { StatusRequest::Recover { hp: 25, mp: 25, focus: 25, kind: 1 } });
        assert_eq!(b.step().sub_step, 5, "the get-up at the step that applied it");
        let mut up = false;
        for _ in 0..90 {
            up |= b.step().state == StateId::Idle;
        }
        assert!(up && b.sora.status().hp > 0, "standing ({full})");
    }
}

/// A KILL PLANE'S KO (`combat.kill_plane_held`) stays down through any HP gain (the host stands him up:
/// kh-partyhost `PartyRuntime::revive_ai_sora`).
#[test]
fn a_kill_plane_ko_stays_down_through_a_heal() {
    let mut b = Bench::new(true, 0.0);
    b.step();
    b.sora.kill_unconditional(Vec3::new(150.0, 0.0, 0.0));
    for _ in 0..120 {
        b.step();
    }
    assert_eq!((b.sora.output().state, b.sora.output().sub_step), (StateId::Die, 4));
    b.sora.status_request(StatusRequest::Recover { hp: 25, mp: 0, focus: 0, kind: 0 });
    for _ in 0..60 {
        let o = b.step();
        assert_eq!((o.state, o.sub_step), (StateId::Die, 4));
    }
    assert_eq!(b.sora.status().hp, 0);
}

/// THE LEADER'S DIE IS KH3'S, UNCHANGED: thrown, the flight loop held to the end (sub-steps 0 - 3), never the down pose or a
/// get-up: HP given back does not stand him up (the game over is the host's: kh-er death.rs).
#[test]
fn the_leader_s_die_is_unchanged() {
    let mut b = Bench::new(false, 0.0);
    b.step();
    b.knock_out();
    let flight = [clip("0212"), clip("0215")];
    let down = clip(member::DOWN_CLIP);
    let mut subs = Vec::new();
    for _ in 0..240 {
        let o = b.step();
        assert_eq!(o.state, StateId::Die);
        assert_ne!(o.anim.name, down);
        if subs.last() != Some(&o.sub_step) {
            subs.push(o.sub_step);
        }
    }
    assert_eq!(subs, vec![1, 2, 3], "KH3's sub-steps");
    assert!(flight.contains(&b.sora.output().anim.name), "the flight loop held: {:?}", b.sora.output().anim.name);
    b.sora.status_mut().hp = 40;
    for _ in 0..180 {
        let o = b.step();
        assert_eq!((o.state, o.sub_step), (StateId::Die, 3), "no get-up for the leader");
        assert!(flight.contains(&o.anim.name));
    }
}
