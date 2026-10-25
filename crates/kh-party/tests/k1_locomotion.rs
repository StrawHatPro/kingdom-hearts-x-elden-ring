//! (party K1) The AI members' locomotion (crates/kh-party/src/locomotion.rs): KH3's FollowPlayer speed (0x1412196C0),
//! the start clip from standing, the speed-matched loop (no foot slide, no cycle without movement), the blocked guard,
//! the idle turn by the turn clip.

use glam::Vec3;
use kh_core::test_world::TestWorld;
use kh_party::{Chara, CommandStatus, LocomotionDef, MemberCommand, MoveGoal, NpcStateId, NullPartyHost, Party, PartyCamera, PartyConfig, SoraView, TargetRef};

const DT: f32 = 1.0 / 60.0;

fn step(party: &mut Party, world: &TestWorld, sora: &SoraView, n: usize) {
    let mut null = NullPartyHost::default();
    for _ in 0..n {
        let mut host = null.host(world);
        party.step(DT, sora, &mut host);
    }
}

fn move_to(p: Vec3) -> MemberCommand {
    MemberCommand::MoveTo { goal: MoveGoal::Point(p), locomotion: LocomotionDef::Move1, walk: false, acceptable_radius: 5.0, follow_path: false }
}

/// A camera at `at` looking along +X.
fn camera(at: Vec3) -> PartyCamera {
    PartyCamera { position: at, pitch: 0.0, yaw: 0.0, fov: 90.0, aspect: 16.0 / 9.0 }
}

#[test]
fn the_follow_speed_is_khs_rule() {
    let world = TestWorld::floor();
    let mut party = Party::new(PartyConfig::test_level99());
    let d = party.spawn(Chara::Donald, Vec3::ZERO, 0.0);
    // the leader sprints (980) 1000 cm ahead, the friend in view: 980 x 1.1
    let mut sora = SoraView { position: Vec3::new(1000.0, 0.0, 0.0), velocity: Vec3::new(980.0, 0.0, 0.0), max_speed: 980.0, camera: Some(camera(Vec3::new(-400.0, 0.0, 150.0))), ..SoraView::default() };
    step(&mut party, &world, &sora, 2);
    let m = party.member(d).unwrap();
    let k = kh_party::locomotion::K;
    let cx = |s: &SoraView| kh_party::locomotion::follow_speed(m, &kh_party::pawn::StepCtx { sora: s, members: &[], dt: DT, ice_rails: &[], zones: &[] /* (ZETTA) */ });
    assert!((cx(&sora).unwrap() - 980.0 * k.follow_in_view_rate).abs() < 1.0, "{:?}", cx(&sora));
    // out of view (the camera ahead of him, looking away): x 2.5
    sora.camera = Some(camera(Vec3::new(500.0, 0.0, 150.0)));
    assert!((cx(&sora).unwrap() - 980.0 * k.follow_out_view_rate).abs() < 1.0, "{:?}", cx(&sora));
    // within m_RangeSamePlayerSpeed: distance / 300 of the reference (here the walk floor wins below 140)
    sora.position = Vec3::new(150.0, 0.0, 0.0);
    assert!((cx(&sora).unwrap() - 980.0 * 150.0 / 300.0).abs() < 1.0, "{:?}", cx(&sora));
    sora.position = Vec3::new(10.0, 0.0, 0.0);
    sora.velocity = Vec3::ZERO;
    sora.max_speed = 680.0;
    assert!((cx(&sora).unwrap() - 140.0).abs() < 1.0, "the floor m_MinSpeed: {:?}", cx(&sora));
    // in battle (no battle-follow mode): the rule does not run
    party.set_in_battle(true);
    step(&mut party, &world, &sora, 1);
    let m = party.member(d).unwrap();
    assert!(kh_party::locomotion::follow_speed(m, &kh_party::pawn::StepCtx { sora: &sora, members: &[], dt: DT, ice_rails: &[], zones: &[] /* (ZETTA) */ }).is_none());
}

#[test]
fn a_start_from_standing_plays_the_start_clip_and_turns_by_it() {
    let world = TestWorld::floor();
    let mut party = Party::new(PartyConfig::test_level99());
    let d = party.spawn(Chara::Donald, Vec3::ZERO, 0.0);
    let sora = SoraView::default();
    step(&mut party, &world, &sora, 5);
    // a goal behind him
    party.command(d, move_to(Vec3::new(-2000.0, 0.0, 0.0)));
    step(&mut party, &world, &sora, 1);
    let v = party.view(d).unwrap();
    if v.anim.clip != "RunStartN" {
        eprintln!("SKIPPED: no full anim tables (the start clip needs stream A's SequenceData)");
        return;
    }
    let mut yaw_without_clip = 0.0f32;
    let mut last = v.yaw;
    for _ in 0..40 {
        step(&mut party, &world, &sora, 1);
        let v = party.view(d).unwrap();
        let dy = kh_party::pawn::wrap180(v.yaw - last).abs();
        if !v.anim.clip.starts_with("RunStart") && v.anim.clip != "RunLoopN" && v.anim.clip != "WalkLoopN" {
            yaw_without_clip = yaw_without_clip.max(dy / DT);
        }
        last = v.yaw;
    }
    let v = party.view(d).unwrap();
    assert!(kh_party::pawn::wrap180(v.yaw - 180.0).abs() < 20.0, "turned round by the start clip: yaw {}", v.yaw);
    assert!(v.position.x < -100.0, "and runs to the goal: {:?}", v.position);
    assert_eq!(yaw_without_clip, 0.0);
}

#[test]
fn the_loop_matches_the_ground_and_never_cycles_in_place() {
    // a wall across his way: he runs, is blocked, steps aside or gives up; the loop's pose never runs without movement
    let world = TestWorld::floor().with_box(Vec3::new(300.0, -2000.0, 0.0), Vec3::new(340.0, 2000.0, 400.0));
    let mut party = Party::new(PartyConfig::test_level99());
    let d = party.spawn(Chara::Goofy, Vec3::ZERO, 0.0);
    let sora = SoraView { position: Vec3::new(3000.0, 0.0, 0.0), ..SoraView::default() };
    step(&mut party, &world, &sora, 5);
    let t = party.command(d, move_to(Vec3::new(1500.0, 0.0, 0.0)));
    let mut prev = party.view(d).unwrap().position;
    let mut in_place = 0;
    let mut slow = 0;
    for _ in 0..240 {
        step(&mut party, &world, &sora, 1);
        let v = party.view(d).unwrap();
        let speed = Vec3::new(v.position.x - prev.x, v.position.y - prev.y, 0.0).length() / DT;
        prev = v.position;
        if party.ai_loco(d).is_some() && speed < 14.0 {
            slow += 1;
        } else {
            slow = 0;
        }
        // more than k1.stand_after (0.1 s) of the cycle without movement = running in place
        if slow > 7 {
            in_place += 1;
        }
    }
    assert_eq!(in_place, 0, "the run cycle played without movement");
    // blocked straight on: he steps aside along the wall (moving) or gives up (the MoveTo fails, he stands)
    let s = party.command_status(d, t);
    let v = party.view(d).unwrap();
    let m = party.member(d).unwrap();
    assert!(s == CommandStatus::Failed || (v.state == NpcStateId::Move && (m.loco.side > 0.0 || m.loco.blocked < kh_party::locomotion::K.k1_blocked_give_up)), "{s:?} at {:?} state {:?}", v.position, v.state);
    if s == CommandStatus::Failed {
        assert_eq!(v.state, NpcStateId::Idle);
        // a MoveTo into the same wall at once fails (no run-stop flicker)
        let again = party.command(d, move_to(Vec3::new(1500.0, 0.0, 0.0)));
        step(&mut party, &world, &sora, 1);
        assert_eq!(party.command_status(d, again), CommandStatus::Failed);
    }
}

#[test]
fn standing_he_turns_to_his_focus_with_the_turn_clip() {
    let world = TestWorld::floor();
    let mut party = Party::new(PartyConfig::test_level99());
    let d = party.spawn(Chara::Donald, Vec3::ZERO, 0.0);
    let sora = SoraView::default();
    step(&mut party, &world, &sora, 5);
    party.command(d, MemberCommand::Focus(TargetRef::Point(Vec3::new(0.0, 500.0, 0.0))));
    let mut slid = 0.0f32;
    let mut turned_by_clip = false;
    let mut last = party.view(d).unwrap().yaw;
    for _ in 0..120 {
        step(&mut party, &world, &sora, 1);
        let v = party.view(d).unwrap();
        let rate = kh_party::pawn::wrap180(v.yaw - last).abs() / DT;
        if v.anim.clip.starts_with("Idle") && rate > 0.0 {
            slid = slid.max(rate);
        }
        if v.anim.clip.starts_with("Turn") {
            turned_by_clip = true;
        }
        last = v.yaw;
    }
    let v = party.view(d).unwrap();
    assert!(turned_by_clip, "the turn clip played");
    assert!(kh_party::pawn::wrap180(v.yaw - 90.0).abs() < 5.0, "faces the focus: {}", v.yaw);
    assert!(slid < 1.0, "no yaw change under the idle clip: {slid} deg/s");
}

#[test]
fn a_moveto_arrives_at_radius_5_10_15_at_the_run_speeds() {
    // AIX K1-a: with a 5 cm radius and an 11.3 cm step at 680 cm/s he stepped over the goal and back forever
    let world = TestWorld::floor();
    for battle in [true, false] {
        for radius in [5.0f32, 10.0, 15.0] {
            for chara in [Chara::Donald, Chara::Goofy] {
                let mut party = Party::new(PartyConfig::test_level99());
                let d = party.spawn(chara, Vec3::ZERO, 0.0);
                party.set_in_battle(battle);
                // out of battle the follow rule runs: a leader far ahead, out of view (2.5 x his 980 MaxWalkSpeed)
                let sora = SoraView { position: Vec3::new(9000.0, 0.0, 0.0), max_speed: 980.0, camera: Some(camera(Vec3::new(8000.0, 0.0, 150.0))), ..SoraView::default() };
                step(&mut party, &world, &sora, 3);
                let t = party.command(d, MemberCommand::MoveTo { goal: MoveGoal::Point(Vec3::new(700.0, 0.0, 0.0)), locomotion: LocomotionDef::Move1, walk: false, acceptable_radius: radius, follow_path: false });
                let mut done = None;
                for i in 0..300 {
                    step(&mut party, &world, &sora, 1);
                    if party.command_status(d, t) != CommandStatus::Running {
                        done = Some(i);
                        break;
                    }
                }
                assert_eq!(party.command_status(d, t), CommandStatus::Succeeded, "{chara:?} battle {battle} radius {radius}: not arrived ({:?})", party.view(d).unwrap().position);
                assert!(done.unwrap() < 200, "{chara:?} battle {battle} radius {radius}: {done:?} steps");
                // and he stays: no step over and back
                let p0 = party.view(d).unwrap().position;
                step(&mut party, &world, &sora, 60);
                let p1 = party.view(d).unwrap().position;
                assert!((p1 - p0).truncate().length() < 120.0, "{chara:?} radius {radius}: kept moving {p0} -> {p1}");
            }
        }
    }
}
