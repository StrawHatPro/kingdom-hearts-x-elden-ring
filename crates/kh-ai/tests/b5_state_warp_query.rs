//! (party B5) A pawn state's own warp query (the NPC RailSlide's off-rail warp 0x14124E890 over
//! `TresFriendData.WarpQuery`): the controller runs the game's query for a body that asks and hands the best
//! item back through `AiBody::warp_query_result` (defaulted trait methods, kh-ai API unchanged otherwise).

mod common;
use common::*;
use glam::Vec3;
use kh_ai::*;

struct AskingBody<'a> {
    inner: Body<'a>,
    answer: &'a mut Option<Option<Vec3>>,
}

impl AiBody for AskingBody<'_> {
    fn view(&self) -> BodyView {
        self.inner.view()
    }
    fn function(&self, name: &str) -> Option<f32> {
        self.inner.function(name)
    }
    fn command(&mut self, cmd: AiCommand) -> u32 {
        self.inner.command(cmd)
    }
    fn command_status(&self, ticket: u32) -> AiCommandStatus {
        self.inner.command_status(ticket)
    }
    fn wants_warp_query(&self) -> bool {
        self.answer.is_none()
    }
    fn warp_query_result(&mut self, best: Option<Vec3>) {
        *self.answer = Some(best);
    }
}

#[test]
fn a_state_s_warp_query_is_run_and_answered() {
    let mut p = Party::new();
    p.world.set_player(Vec3::ZERO, 0.0);
    p.sync_allies();
    let mut answer = None;
    {
        let (ai, pawn) = (&mut p.ais[0], &mut p.pawns[0]);
        let mut body = AskingBody { inner: Body(pawn), answer: &mut answer };
        ai.tick(1.0 / 60.0, &mut body, &p.world, &mut p.shared);
    }
    let best = answer.expect("the query ran in the tick");
    let at = best.expect("EQSQuery_NpcWarp finds a point near the player on the flat floor");
    assert!(at.truncate().length() < 1500.0, "near the player: {at:?}");
    // a body that does not ask is never answered (the default)
    let mut quiet = None;
    {
        let (ai, pawn) = (&mut p.ais[1], &mut p.pawns[1]);
        struct Quiet<'a>(Body<'a>, &'a mut Option<Option<Vec3>>);
        impl AiBody for Quiet<'_> {
            fn view(&self) -> BodyView {
                self.0.view()
            }
            fn function(&self, name: &str) -> Option<f32> {
                self.0.function(name)
            }
            fn command(&mut self, cmd: AiCommand) -> u32 {
                self.0.command(cmd)
            }
            fn command_status(&self, ticket: u32) -> AiCommandStatus {
                self.0.command_status(ticket)
            }
            fn warp_query_result(&mut self, best: Option<Vec3>) {
                *self.1 = Some(best);
            }
        }
        ai.tick(1.0 / 60.0, &mut Quiet(Body(pawn), &mut quiet), &p.world, &mut p.shared);
    }
    assert!(quiet.is_none());
}
