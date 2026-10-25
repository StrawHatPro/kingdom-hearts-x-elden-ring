//! (party I1) The warp check's state switch (FUN_141258790, the switch at 0x141258882): a friend in
//! NAVLINK_JUMP (the friends' jump / fall state 0x2B) may warp unless the jump is the rail leave's (state byte
//! +0xA9, set only by 0x141252210); CLIMBING and DANGLE (the switch default) never warp.

mod common;
use common::*;
use glam::Vec3;
use kh_ai::*;

/// A body pinned in one state (the BT's commands are logged, the state stays), answering
/// `IsRailLeaveJump` as asked.
struct Pinned<'a> {
    inner: Body<'a>,
    state: &'static str,
    rail_leave: bool,
}

impl AiBody for Pinned<'_> {
    fn view(&self) -> BodyView {
        let mut v = self.inner.view();
        v.state = self.state;
        v
    }
    fn function(&self, name: &str) -> Option<f32> {
        match name {
            "IsRailLeaveJump" => Some(self.rail_leave as i32 as f32),
            _ => self.inner.function(name),
        }
    }
    fn command(&mut self, cmd: AiCommand) -> u32 {
        self.inner.command(cmd)
    }
    fn command_status(&self, ticket: u32) -> AiCommandStatus {
        self.inner.command_status(ticket)
    }
}

/// Sora 6000 cm away with the camera behind him (the friend is out of the view, the path >= 3000): the
/// field warp case of `real_trees::field_warp_when_far`, with the friend pinned in `state`.
fn warps_in(state: &'static str, rail_leave: bool) -> bool {
    let mut p = Party::new();
    p.world.set_player(Vec3::new(6000.0, 0.0, 0.0), 0.0);
    p.world.camera.position = Vec3::new(5600.0, 0.0, 200.0);
    p.sync_allies();
    for _ in 0..(2.2f32 * 60.0) as i32 {
        let (ai, pawn) = (&mut p.ais[0], &mut p.pawns[0]);
        ai.tick(1.0 / 60.0, &mut Pinned { inner: Body(pawn), state, rail_leave }, &p.world, &mut p.shared);
        if pawn.log.iter().any(|c| matches!(c, AiCommand::Warp { .. })) {
            return true;
        }
    }
    false
}

#[test]
fn a_falling_friend_warps_but_not_in_a_rail_leave_climb_or_dangle() {
    assert!(warps_in("TSID_STATE_NAVLINK_JUMP", false), "a plain NAVLINK_JUMP (fall) passes the switch");
    assert!(!warps_in("TSID_STATE_NAVLINK_JUMP", true), "the rail leave's jump (+0xA9) does not");
    assert!(!warps_in("TSID_STATE_CLIMBING", false), "CLIMBING is the switch default");
    assert!(!warps_in("TSID_STATE_DANGLE", false), "DANGLE is the switch default");
    assert!(warps_in("TSID_STATE_IDLE", false));
}
