//! (party ER S1) The Elden Ring host's additions (port design): the waypoint a MoveTo heads for while the straight line
//! is blocked (`NpcPawn::steer`, `Party::set_steer` / `move_goal`), and the host's pending LadderJump that the AI's
//! commands of the same step do not replace.

use glam::Vec3;
use kh_core::test_world::TestWorld;
use kh_party::{Chara, CommandStatus, LadderJump, LocomotionDef, MemberCommand, MoveGoal, NpcStateId, NullPartyHost, Party, PartyConfig, SoraView};

const DT: f32 = 1.0 / 60.0;

fn step(party: &mut Party, world: &TestWorld, n: usize) {
    let mut null = NullPartyHost::default();
    let sora = SoraView { position: Vec3::new(-400.0, 0.0, 0.0), ..SoraView::default() };
    for _ in 0..n {
        let mut host = null.host(world);
        party.step(DT, &sora, &mut host);
    }
}

fn move_to(p: Vec3) -> MemberCommand {
    MemberCommand::MoveTo { goal: MoveGoal::Point(p), locomotion: LocomotionDef::Move1, walk: false, acceptable_radius: 50.0, follow_path: false }
}

#[test]
fn the_waypoint_leads_the_moveto_and_only_for_its_goal() {
    kh_party::locomotion::FOL2_ON.store(true, std::sync::atomic::Ordering::Relaxed); // (FOL2) the natural follow on
    let world = TestWorld::floor();
    let mut party = Party::new(PartyConfig::test_level99());
    let d = party.spawn(Chara::Donald, Vec3::ZERO, 0.0);
    step(&mut party, &world, 5);
    let goal = Vec3::new(1000.0, 0.0, 0.0);
    party.command(d, move_to(goal));
    assert_eq!(party.move_goal(d), Some(MoveGoal::Point(goal)));
    // the waypoint is off to the side (+Y): he heads there first
    party.set_steer(d, Some((goal, Vec3::new(0.0, 800.0, 0.0))));
    // (FOL2) the leader stands 4 m off: KH3's follow rule walks him (AnalogInputModifier 0), so longer than at a run
    step(&mut party, &world, 120);
    let v = party.view(d).unwrap();
    assert!(v.position.y > 100.0 && v.position.y > v.position.x.abs(), "toward the waypoint: {:?}", v.position);
    // a waypoint made for another goal is not followed
    let before = v.position;
    party.set_steer(d, Some((Vec3::new(-3000.0, 0.0, 0.0), Vec3::new(0.0, -3000.0, 0.0))));
    step(&mut party, &world, 120); // (FOL2) walking: longer
    let v = party.view(d).unwrap();
    assert!(v.position.x > before.x + 50.0, "straight toward the goal again: {:?}", v.position);
    party.set_steer(d, None);
    assert!(party.member(d).unwrap().steer.is_none());
}

#[test]
fn a_pending_ladder_jump_wins_over_the_ais_commands_of_the_step() {
    let world = TestWorld::floor().with_box(Vec3::new(130.0, -300.0, 0.0), Vec3::new(600.0, 300.0, 300.0));
    let ladder = LadderJump { to: Vec3::new(190.0, 0.0, 300.0), facing: 0.0, delay: 0.0 };
    let mut party = Party::new(PartyConfig::test_level99());
    let d = party.spawn(Chara::Donald, Vec3::new(40.0, 0.0, 0.0), 0.0);
    step(&mut party, &world, 5);
    let climb = party.command(d, MemberCommand::LadderJump(ladder));
    // the AI's MoveTo issued after it in the same step is refused
    let mv = party.command(d, move_to(Vec3::new(-400.0, 0.0, 0.0)));
    assert_eq!(party.command_status(d, mv), CommandStatus::Failed);
    assert_eq!(party.command_status(d, climb), CommandStatus::Running);
    step(&mut party, &world, 2);
    assert_eq!(party.view(d).unwrap().state, NpcStateId::Ladder);
    // a Warp (the host's fallback) still replaces a pending climb
    let mut party = Party::new(PartyConfig::test_level99());
    let g = party.spawn(Chara::Goofy, Vec3::new(40.0, 0.0, 0.0), 0.0);
    step(&mut party, &world, 5);
    let climb = party.command(g, MemberCommand::LadderJump(ladder));
    party.command(g, MemberCommand::Warp { goal: Vec3::new(-300.0, 0.0, 0.0), yaw: 0.0 });
    assert_eq!(party.command_status(g, climb), CommandStatus::Aborted);
}
