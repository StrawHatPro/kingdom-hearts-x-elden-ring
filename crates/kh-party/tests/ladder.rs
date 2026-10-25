//! (LADDER2) The port's ladder jump of a party member (`MemberCommand::LadderJump`, crate::ladder; PORT DESIGN, KH3 has no
//! ladders): up onto a ledge on one arc, landing on the spot asked facing the heading asked, refusing the AI's commands
//! and hits meanwhile (the AI's Stop too), one take-off for the host's sound; the host's cancel and Warp end it.

use glam::Vec3;
use kh_core::test_world::TestWorld;
use kh_party::{Chara, CommandStatus, LadderJump, LocomotionDef, MemberCommand, MoveGoal, NpcStateId, NullPartyHost, Party, PartyConfig, SoraView};

const DT: f32 = 1.0 / 60.0;

/// A floor and a 3 m ledge from x 130 on; the ladder up its face at x 100, its top exit 60 cm onto the ledge.
fn world() -> TestWorld {
    TestWorld::floor().with_box(Vec3::new(130.0, -300.0, 0.0), Vec3::new(600.0, 300.0, 300.0))
}

const JUMP: LadderJump = LadderJump { to: Vec3::new(190.0, 0.0, 300.0), facing: 0.0, delay: 0.0 };

fn step(party: &mut Party, world: &TestWorld, n: usize) {
    let mut null = NullPartyHost::default();
    let sora = SoraView { position: Vec3::new(-400.0, 0.0, 0.0), ..SoraView::default() };
    for _ in 0..n {
        let mut host = null.host(world);
        party.step(DT, &sora, &mut host);
    }
}

#[test]
fn an_ally_jumps_up_onto_the_ledge_and_lands_facing_the_heading() {
    let world = world();
    let mut party = Party::new(PartyConfig::test_level99());
    let d = party.spawn(Chara::Donald, Vec3::new(40.0, 0.0, 0.0), 180.0);
    step(&mut party, &world, 5);
    let t = party.command(d, MemberCommand::LadderJump(LadderJump { delay: 0.2, ..JUMP }));
    step(&mut party, &world, 2);
    assert_eq!(party.view(d).unwrap().state, NpcStateId::Ladder);
    assert_eq!(party.view(d).unwrap().state_name, "TresNpcState_Ladder (port)");
    // the AI's follow commands are refused in the jump
    let mv = party.command(d, MemberCommand::MoveTo { goal: MoveGoal::Point(Vec3::new(-400.0, 0.0, 0.0)), locomotion: LocomotionDef::Move1, walk: false, acceptable_radius: 50.0, follow_path: false });
    assert_eq!(party.command_status(d, mv), CommandStatus::Failed);
    let (mut highest, mut takeoffs, mut done) = (0.0f32, 0, false);
    for _ in 0..(4.0 / DT) as usize {
        step(&mut party, &world, 1);
        takeoffs += party.take_ladder_takeoffs().len();
        highest = highest.max(party.view(d).unwrap().position.z);
        if party.command_status(d, t) == CommandStatus::Succeeded {
            done = true;
            break;
        }
    }
    assert!(done, "the jump finishes");
    assert_eq!(takeoffs, 1, "one take-off: one sound");
    assert!(highest > 300.0 + kh_party::ladder::CLEAR * 0.8, "over the lip ({highest})");
    step(&mut party, &world, 30);
    let v = party.view(d).unwrap();
    assert!(v.on_ground && (v.position.x - 190.0).abs() < 2.0 && v.position.y.abs() < 2.0 && v.position.z > 295.0, "on the landing: {:?} on_ground {}", v.position, v.on_ground);
    assert!(kh_party::ladder::wrap180(v.yaw - JUMP.facing).abs() < 1.0, "facing the heading: {}", v.yaw);
    assert_ne!(v.state, NpcStateId::Ladder);
}

#[test]
fn the_jump_refuses_hits_and_stop_the_host_or_warp_end_it() {
    let world = world();
    let mut party = Party::new(PartyConfig::test_level99());
    let g = party.spawn(Chara::Goofy, Vec3::new(60.0, 0.0, 0.0), 0.0);
    step(&mut party, &world, 10);
    let t = party.command(g, MemberCommand::LadderJump(JUMP));
    step(&mut party, &world, 20);
    assert_eq!(party.view(g).unwrap().state, NpcStateId::Ladder);
    let hp = party.view(g).unwrap().hp;
    let hit = party.receive_hit(g, &kh_core::HitRecord { damage: 10.0, ..kh_core::HitRecord::empty(kh_core::TargetId::NONE) });
    assert!(!hit.accepted && party.view(g).unwrap().hp == hp, "no hit takes him out of the air");
    // the AI's Stop (its tree stops a member who waits for his turn) does not end it
    let stop = party.command(g, MemberCommand::Stop);
    step(&mut party, &world, 2);
    assert_eq!(party.command_status(g, stop), CommandStatus::Failed, "Stop is refused in the jump");
    assert_eq!(party.view(g).unwrap().state, NpcStateId::Ladder);
    // the host ends it (a reseat)
    kh_party::ladder::cancel(party.member_mut(g).unwrap());
    step(&mut party, &world, 2);
    assert_ne!(party.command_status(g, t), CommandStatus::Running);
    assert_ne!(party.view(g).unwrap().state, NpcStateId::Ladder);
    step(&mut party, &world, 120);
    let _ = party.command(g, MemberCommand::LadderJump(JUMP));
    step(&mut party, &world, 10);
    let w = party.command(g, MemberCommand::Warp { goal: Vec3::new(-300.0, 0.0, 0.0), yaw: 0.0 });
    step(&mut party, &world, 1);
    assert_ne!(party.command_status(g, w), CommandStatus::Failed, "Warp is taken in the jump");
    assert_ne!(party.view(g).unwrap().state, NpcStateId::Ladder, "out of the jump: {:?}", party.view(g).unwrap().state);
}
