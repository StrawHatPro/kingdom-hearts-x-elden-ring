//! (B4) The traversal and situation states of a friend: `UTresNpcState_SlopeSlide` (id 10),
//! `UTresNpcState_SuperSlide` (0x15), `UTresNpcState_RailSlide` (0x16, over the shared
//! `UTresCharState_RailSlide`), `UTresNpcState_Hopping` (9), `UTresNpcState_SituationAction` (0x93,
//! Goofy's `n_ex002` override) and the fall they leave into, transcribed from the decompiled C.
//! The sheet `party-states` lists the addresses; `party-params` rows `slide.*`, `superslide.*`,
//! `rail.*`, `hop.*`, `situation.*` hold the constants.
//!
//! Entry (UTresAICharState_Run tick 0x140442430, while a move runs): not on the ground -> the
//! jump request; state 0x16 present -> the rail probe (pawn vt+0xEA0, range 50) -> RailSlide;
//! (B5) state 7 present -> the climb probe FUN_140634E60 (NPC branch FUN_140636BF0) -> Climbing
//! (crate::climb); the hop probe 120 cm ahead (vt+0x2350, radius 100) ->
//! Hopping; the slope probe FUN_14063C870 (mode 0) -> SlopeSlide. SuperSlide and SituationAction
//! are requested from outside (`Party::request_super_slide`, `Party::set_situation`,
//! `Party::request_npc_motion`).
//!
//! The AI move feeds these states (pawn interface vt+0x118 = 0x141270D90, called by the path
//! following through 0x140549DC0 with the move direction and scale 1): SlopeSlide / SuperSlide /
//! RailSlide get +0x88 = 1, +0x8C = the yaw; Hopping +0xA8 yaw, +0xAC scale, +0xA5 = 1. The port
//! feeds the direction to the MoveTo goal (no nav path: the straight line).
//!
//! Positions: as kh-core's Sora ports, a rail node / hop point position is where the actor
//! location (`cmc.location`) goes. (B5) The body pitch the slides interpolate (`Traverse::pitch`) is
//! `MemberView::pitch`; kh-app draws the actor rotation (pitch, yaw, 0). The fall / code-driven jump is
//! `crate::jump` (NavLinkJumpCodeDriven), the rails include Sora's Blizzard ice rails (`StepCtx::ice_rails`).

use crate::command::{LocomotionDef, MemberCommand, MoveGoal};
use crate::host::PartyHost;
use crate::pawn::{forward, horizontal, wrap180, yaw_of, NpcPawn, StepCtx};
use crate::states::NpcStateId;
use crate::tables;
use crate::view::PartyEventKind;
use glam::{Vec2, Vec3};
use kh_core::actions::ACTION_PARAMS as A;
use kh_core::{surface, HopPoint, World};

/// The AI move feed of this tick (pawn interface vt+0x118 0x141270D90).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Feed {
    /// +0x88 (slides) / +0xA5 (Hopping): input this tick.
    pub on: bool,
    /// +0x8C / +0xA8: the yaw (kept after the tick, cleared only at the state's Init).
    pub yaw: f32,
    /// +0xAC (Hopping): the scale (1 from the path following).
    pub magnitude: f32,
}

/// `UTresCharState_RailSlide` fields the tick reads (as kh-core's port names them) and the NPC's own.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct RailRun {
    pub id: u32,
    pub attach: Vec3,
    pub yaw: f32,
    pub reversed: bool,
    pub waiting: bool,
    pub turn_armed: bool,
    pub narrow: bool,
    pub no_reverse: bool,
    pub no_jump: bool,
    pub must_release: bool,
    pub turn_lock: f32,
    pub paused: bool,
    pub ended: bool,
    pub speed: f32,
    /// +0x190: the pawn's +0x75E at the request (invincible on the rail, vt+0x2068).
    pub invincible: bool,
    /// +0x191 the warp point is found, +0x19C the delay, +0x198 the limit, +0x1A0 the point, +0x194 a query runs.
    pub warp_found: bool,
    pub warp_delay: f32,
    pub warp_limit: f32,
    pub warp_point: Vec3,
    pub warp_query: bool,
}

/// `UTresNpcState_Hopping` fields (Init 0x141247D60).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct HopRun {
    /// m_pHitComponent: the registry id and the point's facts.
    pub point: u32,
    pub target: Vec3,
    pub idle_motion: u8,
    pub end_point: bool,
    /// +0x98: the actor location at the hop start.
    pub start: Vec3,
    /// +0x84 bit 1 (which foot hops next; m_bReverseLeg of the request).
    pub reverse_leg: bool,
    /// +0xB0: moving toward the point.
    pub moving: bool,
    /// +0xB4: hops made (carried to the next point's state).
    pub counter: u32,
    /// +0xB8: the walk speed of the hop (Begin: vt+0x1588, the pawn's default MaxWalkSpeed).
    pub speed: f32,
}

/// `UTresNpcState_SituationAction` (Init 0x141247F40 -> 0x141247430): the inner machine (+0x88; modes 0
/// Wait, 1 TalkPointWait, 2 GimmickWait, 3 NpcMotionPlay; Goofy's 7 GCoop) and its fields.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct SituationRun {
    pub mode: u8,
    /// +0xF4 the mode's phase; +0xFC motions played; +0x104 the count (< 0: loop); +0x108 / +0x110 /
    /// +0x118 the start / loop / end motions; +0x120 the finish request.
    pub phase: i32,
    pub done: i32,
    pub count: i32,
    pub anims: [Option<&'static str>; 3],
    pub finish: bool,
    /// Goofy (n_ex002 SituationAction): +0x128 phase, +0x12C the situation command shown.
    pub coop_phase: i32,
    pub coop_shown: bool,
}

/// A friend's traversal / situation state data (one per pawn; reset at each state's Begin as the game's
/// Init slots do).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Traverse {
    pub feed: Feed,
    pub sub: u8,
    /// SlopeSlide +0x90 (the slope pitch of the last probe).
    pub slope_pitch: f32,
    /// The body pitch the rotation interpolation holds (vt+0xDB0 target pitch; B5: `MemberView::pitch`, drawn).
    pub pitch: f32,
    /// SuperSlide +0x90 / +0x94 (the air latch and its timer) and +0xA8 (a turn rate). B5: +0x90 and +0xA8 are
    /// zeroed by Init 0x141266B90 and written by nothing else (every function referencing the class read), so
    /// `sup_flag` / `sup_rate` stay false / 0 as in the game.
    pub sup_flag: bool,
    pub sup_hold: f32,
    pub sup_rate: f32,
    pub rail: RailRun,
    pub hop: HopRun,
    /// The pawn's m_LastHitHopComponent (FUN_141513A80) and the rail just left (the probe refuses it).
    pub last_hop: Option<u32>,
    pub rail_exclude: Option<u32>,
    /// Pawn +0xBD4 / +0xBD8: the situation (0 none, 3 / 5 gimmick action point, 6 talk point, 7 Goofy's
    /// coop, 8 gimmick wait, 9 NPC motion play) and its value.
    pub situation: u8,
    pub situation_value: f32,
    pub sit: SituationRun,
    /// Goofy's coop actor (pawn +0xBE8) location, set by the host.
    pub coop_target: Option<Vec3>,
    /// (B5) the friends' jump / fall (`NpcStateId::NavLinkJump`, crate::jump; it replaces B4's fall stand-in).
    pub jump: crate::jump::JumpRun,
    /// (B5) Climbing / Dangle (crate::climb) and the dangle cool-down pawn +0x6F8 (vt+0x2378 / vt+0x2380; only
    /// the player's TryDangle gate reads it).
    pub climb: crate::climb::ClimbRun,
    pub dangle: crate::climb::DangleRun,
    pub dangle_cooldown: f32,
}

pub const fn is_traverse(s: NpcStateId) -> bool {
    matches!(s, NpcStateId::SlopeSlide | NpcStateId::SuperSlide | NpcStateId::RailSlide | NpcStateId::Hopping | NpcStateId::SituationAction | NpcStateId::NavLinkJump | NpcStateId::Climbing | NpcStateId::Dangle)
}

/// (B5) The states that take a MoveTo as their feed and refuse other commands until their CanChange (vt+0x388:
/// NavLinkJump 0x1404325F0, Climbing 0x1412405A0) [inf: who asks vt+0x388 was not found; read as the interrupt
/// gate]; Dangle has no vt+0x388 override (the base answers false) and lets go only by its own input.
pub fn refuses_now(pw: &NpcPawn) -> bool {
    match pw.state {
        NpcStateId::NavLinkJump => !crate::jump::can_change(pw),
        NpcStateId::Climbing => !crate::climb::climb_can_change(pw),
        NpcStateId::Dangle => true,
        _ => false,
    }
}

/// (B5) The ice-rail id space: Sora's Blizzard rail nodes (`kh_core::magic::RailNode` ids) tagged with the high
/// bit, the level's rails keep the host's ids.
pub const ICE_RAIL: u32 = 0x8000_0000;

/// (B5) A rail exists to probe for (the pawn's RailSlide state is there; the friends' sets register it).
pub fn has_rails(w: &dyn World, cx: &StepCtx) -> bool {
    !w.rails().is_empty() || !cx.ice_rails.is_empty()
}

/// (B5) `back` for the jump / ledge states.
pub fn back_pub(pw: &mut NpcPawn) {
    back(pw);
}

fn p(id: &str) -> f32 {
    tables::param(id)
}

/// RInterp of pawn vt+0xDB0 (0x1405DDD90: quaternion slerp by clamp(alpha, 0, 1)) on the yaw.
fn interp_yaw(yaw: f32, target: f32, alpha: f32) -> f32 {
    wrap180(yaw + wrap180(target - yaw) * alpha.clamp(0.0, 1.0))
}

fn rot_dir(pitch: f32, yaw: f32) -> Vec3 {
    let (sp, cp) = pitch.to_radians().sin_cos();
    let (sy, cy) = yaw.to_radians().sin_cos();
    Vec3::new(cp * cy, cp * sy, sp)
}

// ============================================================================ probes

/// What FUN_14063C870 hands back: +0x20 the fall angle of the floor under him (deg, negative downhill).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct SlopeProbe {
    pub ok: bool,
    pub angle: f32,
    pub forced: bool,
}

/// The NPC slope probe FUN_14063C870(pawn, out, mode): a sphere of 50 swept from 50 above the location to
/// 50 below it (channels 0 / 1 / 0x12); the first component (not his own) flagged `SQEX_bEnableSlopeSlide`
/// (component vt+0x550) gives the fall line (the ImpactNormal's rotator, pitch - 90, X axis) and its
/// angle (+0x20). It passes when the floor is forced (vt+0x580) or the angle <= -25 (mode 0) / -20
/// (mode > 0), he faces down it (dot(facing, fall) >= cos 50 / cos 80), and the same sweep 50 cm further
/// along Rot(angle, facing yaw) meets a floor that is forced or descends as steeply.
pub fn slope_probe(w: &dyn World, location: Vec3, yaw: f32, mode: u8) -> SlopeProbe {
    let (limit, min_dot) = if mode > 0 { (p("slide.continue_angle"), p("slide.continue_dot")) } else { (p("slide.start_angle"), p("slide.start_dot")) };
    let r = p("slide.probe_radius");
    let a = location + Vec3::Z * 50.0;
    let b = a - Vec3::Z * 100.0;
    let fall_of = |n: Vec3| -> Vec3 {
        let pitch = n.z.atan2(n.truncate().length()).to_degrees();
        let ny = n.y.atan2(n.x).to_degrees();
        rot_dir(pitch - 90.0, ny)
    };
    let angle_of = |v: Vec3| v.z.atan2(v.truncate().length()).to_degrees();
    let Some(h1) = w.sweep_capsule_all(a, b, r, r).into_iter().find(|h| w.surface_flags(h.component) & surface::SLOPE_SLIDE != 0) else {
        return SlopeProbe::default();
    };
    let fall = fall_of(h1.impact_normal);
    let angle = angle_of(fall);
    let forced = w.surface_flags(h1.component) & surface::FORCE_SLOPE_SLIDE != 0;
    let mut out = SlopeProbe { ok: false, angle, forced };
    if !(forced || angle <= limit) {
        return out;
    }
    if forward(yaw).dot(fall) < min_dot {
        return out;
    }
    let step = rot_dir(angle, yaw) * p("slide.ahead");
    let Some(h2) = w.sweep_capsule_all(a + step, b + step, r, r).into_iter().next() else { return out };
    let forced2 = w.surface_flags(h2.component) & surface::FORCE_SLOPE_SLIDE != 0;
    out.ok = forced2 || angle_of(fall_of(h2.impact_normal)) <= limit;
    out
}

/// A level rail as a polyline (kh-core's `RailPath`, the level registry only).
#[derive(Clone, Debug, PartialEq)]
pub struct RailPath {
    pub id: u32,
    pub points: Vec<Vec3>,
    pub speeds: Vec<Option<f32>>,
    pub range_plus: f32,
    pub start_dir: u8,
    pub no_reverse: bool,
    pub no_jump: bool,
    pub narrow: bool,
}

impl RailPath {
    pub fn length(&self) -> f32 {
        self.points.windows(2).map(|q| (q[1] - q[0]).length()).sum()
    }

    pub fn at(&self, d: f32) -> (Vec3, Vec3) {
        let mut left = d.max(0.0);
        let last = self.points.len().saturating_sub(2);
        for (i, q) in self.points.windows(2).enumerate() {
            let seg = q[1] - q[0];
            let len = seg.length();
            if left <= len || i == last {
                let t = if len > 0.0 { (left / len).clamp(0.0, 1.0) } else { 0.0 };
                return (q[0] + seg * t, seg.normalize_or_zero());
            }
            left -= len;
        }
        (self.points.first().copied().unwrap_or(Vec3::ZERO), Vec3::X)
    }

    pub fn closest(&self, a: Vec3, b: Vec3) -> (f32, f32) {
        let mut best = (0.0, f32::MAX);
        let mut along = 0.0;
        for q in self.points.windows(2) {
            let len = (q[1] - q[0]).length();
            for x in [a, (a + b) * 0.5, b] {
                let ab = q[1] - q[0];
                let t = if ab.length_squared() > 0.0 { ((x - q[0]).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
                let near = q[0] + ab * t;
                let d2 = (x - near).length_squared();
                if d2 < best.1 {
                    best = (along + (near - q[0]).length(), d2);
                }
            }
            along += len;
        }
        best
    }

    pub fn speed_at(&self, d: f32, base: f32) -> f32 {
        let mut left = d.max(0.0);
        for (i, q) in self.points.windows(2).enumerate() {
            let len = (q[1] - q[0]).length();
            if left <= len || i + 2 == self.points.len() {
                let Some(v0) = self.speeds[i] else { return base };
                let v1 = self.speeds[i + 1].unwrap_or(base);
                return v0 + (v1 - v0) * if len > 0.0 { (left / len).clamp(0.0, 1.0) } else { 0.0 };
            }
            left -= len;
        }
        base
    }
}

/// Every rail there is: the level's (`World::rails`), then (B5) Sora's Blizzard ice rails (actor tag
/// `BlizzardRail`, which the shared probe 0x1405A7740 finds for any pawn: kh-core reads it for Sora,
/// `probes::rail_paths`): nodes with 0.1 s of life or more (0x1405A7A03); a chain starts at a living node
/// whose predecessor is gone and follows the links; its id is its LAST node's (the longest-lived), tagged
/// `ICE_RAIL`. The host hands the nodes in with `Party::set_ice_rails`.
pub fn rail_paths(w: &dyn World, ice: &[kh_core::magic::RailNode]) -> Vec<RailPath> {
    let mut out: Vec<RailPath> = w
        .rails()
        .iter()
        .filter(|r| r.nodes.len() >= 2)
        .map(|r| RailPath { id: r.id, points: r.nodes.iter().map(|n| n.position).collect(), speeds: r.nodes.iter().map(|n| n.speed).collect(), range_plus: r.nodes.iter().map(|n| n.detect_range_plus).fold(0.0, f32::max), start_dir: r.start_dir, no_reverse: r.no_reverse, no_jump: r.no_jump, narrow: r.narrow })
        .collect();
    let nodes: Vec<_> = ice.iter().filter(|n| n.life >= A.rail_blizzard_min_life).collect();
    for first in nodes.iter().filter(|n| n.previous == 0 || !nodes.iter().any(|m| m.id == n.previous)) {
        let mut points = vec![first.position];
        let mut id = first.id;
        while let Some(next) = nodes.iter().find(|m| m.previous == id) {
            points.push(next.position);
            id = next.id;
        }
        if points.len() >= 2 {
            let speeds = vec![None; points.len()];
            out.push(RailPath { id: ICE_RAIL | id, points, speeds, range_plus: 0.0, start_dir: 0, no_reverse: false, no_jump: false, narrow: false });
        }
    }
    out
}

/// (B5) The rail of `id` now: a level rail by its id; an ice rail by any node of its chain (the links walked to
/// the chain's last node, kh-core `probes::rail_path`).
pub fn rail_path(w: &dyn World, ice: &[kh_core::magic::RailNode], id: u32) -> Option<RailPath> {
    let paths = rail_paths(w, ice);
    if id & ICE_RAIL == 0 {
        return paths.into_iter().find(|r| r.id == id);
    }
    let mut last = id & !ICE_RAIL;
    while let Some(next) = ice.iter().find(|m| m.previous == last) {
        last = next.id;
    }
    paths.into_iter().find(|r| r.id == ICE_RAIL | last)
}

/// What the rail probe hands back.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RailHit {
    pub id: u32,
    pub distance: f32,
    pub reversed: bool,
    pub position: Vec3,
    pub yaw: f32,
}

/// The rail probe, pawn vt+0xEA0 = ATresCharPawnBase 0x1405A7740 (the friends' too; kh-core ports it for
/// Sora): a rail within `range + m_DetectRangePlus` of the segment [start, end]; the travel direction by
/// `m_StartDir` or the sign of dot(rail, want); not within 150 cm of the end in the travel direction; the
/// rail just left refused until he is 200 cm away. (B5) Sora's Blizzard ice rails count as rails (`ice`).
pub fn rail_probe(t: &mut Traverse, w: &dyn World, ice: &[kh_core::magic::RailNode], start: Vec3, end: Vec3, range: f32, want: Vec3) -> Option<RailHit> {
    let last = t.rail_exclude;
    let mut best: Option<(f32, RailHit)> = None;
    let mut near_last = false;
    for path in rail_paths(w, ice) {
        let (along, d2) = path.closest(start, end);
        let r = range + path.range_plus;
        if Some(path.id) == last {
            near_last |= d2 < A.rail_probe_near_limit_sq;
            continue;
        }
        if d2 > r * r || best.is_some_and(|(b, _)| d2 >= b) {
            continue;
        }
        let (position, fwd) = path.at(along);
        let reversed = match path.start_dir {
            1 => false,
            2 => true,
            _ => fwd.dot(want) < 0.0,
        };
        let ahead = if reversed { along } else { path.length() - along };
        if ahead <= A.rail_end_margin {
            continue;
        }
        let travel = if reversed { -fwd } else { fwd };
        best = Some((d2, RailHit { id: path.id, distance: along, reversed, position, yaw: yaw_of(travel) }));
    }
    if !near_last {
        t.rail_exclude = None;
    }
    best.map(|(_, h)| h)
}

/// The hop probe, pawn vt+0x2350 = ATresCharPawnBase 0x1405A6F40 (as kh-core reads it): the active
/// registered points but the last one (an end point only with a last one remembered), within
/// `radius + m_CheckRadius` of `pos`, the XY distance under radius^2 x 36, in front of `yaw` or inside its
/// own radius; the higher point wins, then the nearer.
pub fn hop_probe(t: &Traverse, w: &dyn World, pos: Vec3, yaw: f32, radius: f32) -> Option<HopPoint> {
    let last = t.last_hop;
    let fwd = forward(yaw);
    let mut limit = radius * radius * A.hopping_probe_limit_factor;
    let mut best: Option<HopPoint> = None;
    for c in w.hop_points() {
        if !c.active || Some(c.id) == last || (c.end_point && last.is_none()) {
            continue;
        }
        let reach = radius + c.check_radius;
        if (pos - c.position).length_squared() > reach * reach {
            continue;
        }
        if best.is_some_and(|b| c.position.z < b.position.z) {
            continue;
        }
        let flat = (c.position - pos).truncate();
        let d2 = flat.length_squared();
        if d2 > limit {
            continue;
        }
        let to = if d2 > 0.0 { flat.normalize().extend(0.0) } else { fwd };
        if d2 <= c.check_radius * c.check_radius || fwd.dot(to) >= 0.0 {
            best = Some(*c);
            limit = d2;
        }
    }
    best
}

// ============================================================================ the feed and the exits

/// The MoveTo the AI runs (goal, locomotion, acceptable radius).
fn move_cmd(pw: &NpcPawn) -> Option<(MoveGoal, LocomotionDef, bool, f32)> {
    match pw.command_now() {
        Some(MemberCommand::MoveTo { goal, locomotion, walk, acceptable_radius, .. }) => Some((goal, locomotion, walk, acceptable_radius)),
        _ => None,
    }
}

/// The locomotion definition's MaxWalkSpeed (Move1 140..680, Move_Slow 140, SlopeSlide 680..1600).
pub fn loco_speed(pw: &NpcPawn, loco: LocomotionDef, walk: bool) -> f32 {
    match (loco, walk) {
        (LocomotionDef::MoveSlow, _) | (_, true) => tables::cparam(pw.chara, "loco.move1.min_speed"),
        (LocomotionDef::SlopeSlide, _) => tables::cparam(pw.chara, "loco.slope_slide.max_speed"),
        _ => tables::cparam(pw.chara, "loco.move1.max_speed"),
    }
}

/// The path following's feed (0x140549DC0 -> iface vt+0x118 0x141270D90) for this tick; the move ends
/// (Succeeded) inside its acceptable radius.
fn feed(pw: &mut NpcPawn, cx: &StepCtx, host: &PartyHost) {
    let Some((goal, loco, walk, radius)) = move_cmd(pw) else { return };
    let Some(g) = pw.goal_point(goal, cx, host) else { return };
    let d = horizontal(g - pw.position());
    if d.length() <= radius.max(1.0) {
        pw.finish_command_pub(true);
        return;
    }
    // (B5) 0x141270D90's other cases: 0x2B -> vt+0x3A8(1) (crate::jump::feed), 7 -> +0xAE = 1, 6 -> +0xB8 / +0xB4
    // (magnitude 1 from the path following, above their 0.5).
    match pw.state {
        NpcStateId::NavLinkJump => return crate::jump::feed(pw),
        NpcStateId::Climbing => {
            pw.trav.climb.input = true;
            return;
        }
        NpcStateId::Dangle => return crate::climb::dangle_feed(pw, yaw_of(d), 1.0),
        _ => {}
    }
    if pw.state != NpcStateId::SuperSlide {
        pw.cmc.max_walk_speed = loco_speed(pw, loco, walk);
    }
    pw.trav.feed = Feed { on: true, yaw: yaw_of(d), magnitude: 1.0 };
}

/// A traversal state ended (vt+0x268 / vt+0x270 with nothing requested): the default state; the AI move,
/// still running, asks for Run again (0x140549DC0) -> Move, else Idle.
fn back(pw: &mut NpcPawn) {
    end(pw);
    if let Some((_, loco, walk, _)) = move_cmd(pw) {
        pw.enter(NpcStateId::Move);
        pw.cmc.max_walk_speed = loco_speed(pw, loco, walk);
        let set = if pw.in_battle { "RunLoopB" } else if walk { "WalkLoopN" } else { "RunLoopN" };
        pw.play(set, -1.0);
    } else {
        pw.to_idle();
    }
}

/// The End of the running traversal state (also on an interrupt: a hit, KO, a command).
pub fn end(pw: &mut NpcPawn) {
    let base_accel = pw.params.accel_default;
    match pw.state {
        // End 0x14125E310: movement vt+0x918(0), MaxAcceleration back (vt+0x1648, the CDO's), mesh vt+0xC28(0).
        NpcStateId::SlopeSlide => pw.cmc.max_acceleration = base_accel,
        // End 0x14125E460: vt+0x918(0), StopMovement + input cleared (vt+0x16E8), MaxWalkSpeed (vt+0x1578)
        // and MaxAcceleration (vt+0x1648) back.
        NpcStateId::SuperSlide => {
            pw.cmc.stop_movement();
            pw.cmc.control_input = Vec3::ZERO;
            pw.cmc.max_walk_speed = pw.params.speed_run;
            pw.cmc.max_acceleration = base_accel;
        }
        // End 0x141245430 -> 0x14150C1F0: the rail is kept as the last one; Flying left; vt+0x2068 when
        // +0x190; a pending warp query is aborted.
        NpcStateId::RailSlide => {
            pw.trav.rail_exclude = Some(pw.trav.rail.id);
            pw.trav.rail.warp_query = false;
            pw.cmc.gravity_off = false;
            pw.cmc.gravity_scale = pw.params.gravity_scale;
            if pw.cmc.is_custom() {
                pw.cmc.set_falling();
            }
        }
        // End 0x141245270: movement mode back (vt+0x17A0()), MaxWalkSpeed / MaxAcceleration back,
        // StopMovement; the last hop component cleared unless the next state is Hopping (FUN_141513A80).
        NpcStateId::Hopping => {
            pw.cmc.gravity_off = false;
            if pw.cmc.is_custom() {
                pw.cmc.set_falling();
            }
            pw.cmc.max_walk_speed = pw.params.speed_run;
            pw.cmc.max_acceleration = base_accel;
            pw.cmc.stop_movement();
            pw.cmc.direct_xy = None;
        }
        // End 0x1412454B0: unless the situation is 0: situation 8 -> GimmickWait's exit (0x141245540:
        // vt+0x2410(0), +0xB96 = 1); then SetSituation(0, 0) (vt+0x2888).
        NpcStateId::SituationAction => {
            if pw.trav.sit.coop_shown {
                pw.events.push(PartyEventKind::SituationCommand { command: 0xCE, on: false });
                pw.trav.sit.coop_shown = false;
            }
            pw.trav.situation = 0;
            pw.trav.situation_value = 0.0;
        }
        // (B5)
        NpcStateId::NavLinkJump => crate::jump::end(pw),
        NpcStateId::Climbing => crate::climb::climb_end(pw),
        NpcStateId::Dangle => crate::climb::dangle_end(pw),
        _ => {}
    }
    pw.trav.pitch = 0.0;
}

// ============================================================================ entries

/// The checks of UTresAICharState_Run's tick 0x140442430 while the move runs (`dir` the move direction):
/// the rail probe (range 50, want = the facing), the hop probe 120 cm ahead (radius 100, the move yaw),
/// the slope probe mode 0. True when a state was entered.
/// (B5) First, not on the ground (FUN_1405AD850) -> RequestJump (iface vt+0x510: the fall, crate::jump); after the
/// rail, the climb probe (Run +0xBC bit 0x40, on from Init 0x14043E670 | 0x78; state 7 present) FUN_140634E60 ->
/// FUN_140636BF0 -> the climb request vt+0x538 with Run +0x84 bit 1 (the leg; 0 here as for the hop request).
pub fn move_checks(pw: &mut NpcPawn, cx: &StepCtx, host: &PartyHost, dir: Vec3) -> bool {
    let w = host.world;
    let loc = pw.position();
    if !pw.on_ground() {
        crate::jump::request_fall(pw, host.world);
        return true;
    }
    if has_rails(w, cx) {
        let want = forward(pw.yaw);
        if let Some(hit) = rail_probe(&mut pw.trav, w, cx.ice_rails, loc, loc, p("slide.rail_range"), want) {
            begin_rail(pw, hit);
            return true;
        }
    }
    if let Some(hit) = crate::climb::climb_probe(pw, w) {
        crate::climb::request_climb(pw, hit, false, false);
        return true;
    }
    if !w.hop_points().is_empty() {
        let at = loc + forward(pw.yaw) * p("slide.hop_ahead");
        let yaw = if dir.length_squared() > 1e-6 { yaw_of(dir) } else { pw.yaw };
        if let Some(pt) = hop_probe(&pw.trav, w, at, yaw, p("slide.hop_radius")) {
            begin_hop(pw, pt, yaw, false, 0);
            return true;
        }
    }
    if pw.on_ground() && slope_probe(w, loc, pw.yaw, 0).ok {
        begin_slope(pw);
        return true;
    }
    false
}

/// SlopeSlide request (iface vt+0x520 0x141276B10) + Init 0x141266A80 (+0x88 = 0, +0x8C / +0x90 = 0) +
/// Begin 0x14125B8A0: `SlopeSlide` (blend 0.1, mode 1), mesh vt+0xC28(20) (not modelled).
pub fn begin_slope(pw: &mut NpcPawn) {
    pw.end_action();
    pw.enter(NpcStateId::SlopeSlide);
    pw.trav.sub = 0;
    pw.trav.feed = Feed::default();
    pw.trav.slope_pitch = 0.0;
    let c = tables::clip(pw.chara, "SlopeSlide");
    pw.anim.play_loop(c, 0.1);
    pw.hit_list.clear();
}

/// The SuperSlide gate (iface vt+0x238 0x141259FC0: the set has `SonicSlide`; the state is Idle / Run on
/// the ground, or SuperSlide) and request (vt+0x240 0x141271170 -> vt+0x248 0x141276B70: sub 1, +0x8C =
/// the actor yaw, +0x88 = 1); Init 0x141266B90; Begin 0x14125BA50 (vt+0x1340(1, 1, 1.0): not modelled).
pub fn request_super_slide(pw: &mut NpcPawn) -> bool {
    if tables::anim_set(pw.chara).get("SonicSlide").is_none() {
        return false;
    }
    let ok_state = matches!(pw.state, NpcStateId::Idle | NpcStateId::Move) && pw.on_ground();
    if pw.state == NpcStateId::SuperSlide {
        return true;
    }
    if !ok_state {
        return false;
    }
    begin_super_slide_at(pw, 1);
    true
}

/// (B5) The SuperSlide request vt+0x248 0x141276B70 itself (sub 1, +0x8C = the actor yaw, +0x88 = 1) with the
/// caller's m_nSubStep: the Climbing tick's SonicSlideClimbing end writes +0x78 = 4 (the loop at once).
/// Init 0x141266B90 zeroes +0x88..+0xAB, so +0x90 and +0xA8 start at 0: nothing else in the game writes them
/// (B5: the state's own functions and every function referencing the class were read).
pub fn begin_super_slide_at(pw: &mut NpcPawn, sub: u8) {
    pw.end_action();
    pw.enter(NpcStateId::SuperSlide);
    pw.trav.sub = sub;
    pw.trav.feed = Feed { on: true, yaw: pw.yaw, magnitude: 1.0 };
    pw.trav.sup_flag = false;
    pw.trav.sup_hold = 0.0;
    pw.trav.sup_rate = 0.0;
}

/// RailSlide request (iface vt+0x260 0x141276800: the probe's hit) + Init 0x141247F00 (base Reset
/// 0x14150E0C0, +0x190 / +0x194 cleared) + Begin 0x1412423C0 (base 0x141509CA0: StopMovement, Flying,
/// gravity off; the warp query delegate 0x141250A00; FUN_1412529F0 is n_bx202's only).
pub fn begin_rail(pw: &mut NpcPawn, hit: RailHit) {
    pw.end_action();
    pw.enter(NpcStateId::RailSlide);
    pw.trav.sub = 0;
    pw.trav.feed = Feed::default();
    pw.trav.rail = RailRun { id: hit.id, attach: hit.position, yaw: hit.yaw, reversed: hit.reversed, must_release: true, ..RailRun::default() };
    pw.cmc.stop_movement();
    pw.cmc.set_custom();
    pw.cmc.gravity_off = true;
}

/// Hopping request (iface vt+0x530 0x141276660: actor, component, reverse-leg flag) + Init 0x141247D60 +
/// Begin 0x141242050: vt+0x2360(component); +0xB8 = the default MaxWalkSpeed (vt+0x1588); MaxWalkSpeed =
/// +0xB8, MaxAcceleration = +0xB8 x 5.4.
pub fn begin_hop(pw: &mut NpcPawn, pt: HopPoint, yaw: f32, reverse_leg: bool, counter: u32) {
    pw.end_action();
    pw.enter(NpcStateId::Hopping);
    pw.trav.sub = 0;
    pw.trav.feed = Feed { on: false, yaw, magnitude: 0.0 };
    let speed = pw.params.speed_run;
    pw.trav.hop = HopRun { point: pt.id, target: pt.position, idle_motion: pt.idle_motion, end_point: pt.end_point, start: pw.position(), reverse_leg, moving: false, counter, speed };
    pw.cmc.max_walk_speed = speed;
    pw.cmc.max_acceleration = speed * p("hop.accel_factor");
}

/// SituationAction Begin 0x141242570 (Goofy 0x1411BD5D0: situation 7 -> his GCoop machine, +0x128 = 0):
/// situation 6 / 8 / 9 -> the inner mode 1 / 2 / 3; any other: the inner machine stays in Wait.
pub fn begin_situation(pw: &mut NpcPawn) {
    pw.end_action();
    pw.enter(NpcStateId::SituationAction);
    pw.trav.sit.phase = 0;
    pw.trav.sit.coop_phase = 0;
    pw.trav.sit.coop_shown = false;
    let mode = match pw.trav.situation {
        7 if pw.chara == crate::Chara::Goofy => 7,
        6 => 1,
        8 => 2,
        9 => 3,
        _ => 0,
    };
    sit_enter(pw, mode);
}

// (B5) B4's fall stand-in (`enter_fall` / `fall_tick`, NpcStateId::Jump) is replaced by crate::jump: RequestJump
// = `jump::request_fall`, JumpCodeDriven = `jump::request_code_driven`.

// ============================================================================ ticks

/// The state ticks (main_proc of the traversal states).
pub fn tick(pw: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) {
    if matches!(pw.state, NpcStateId::SlopeSlide | NpcStateId::SuperSlide | NpcStateId::RailSlide | NpcStateId::Hopping | NpcStateId::NavLinkJump | NpcStateId::Climbing | NpcStateId::Dangle) {
        feed(pw, cx, host);
    }
    match pw.state {
        // (B5)
        NpcStateId::NavLinkJump => return crate::jump::tick(pw, cx, host),
        NpcStateId::Climbing => return crate::climb::climb_tick(pw, cx, host),
        NpcStateId::Dangle => return crate::climb::dangle_tick(pw, cx, host.world),
        _ => {}
    }
    match pw.state {
        NpcStateId::SlopeSlide => slope_tick(pw, cx, host),
        NpcStateId::SuperSlide => super_tick(pw, cx, host),
        NpcStateId::RailSlide => rail_tick(pw, cx, host),
        NpcStateId::Hopping => hop_tick(pw, cx, host),
        NpcStateId::SituationAction => situation_tick(pw, cx),
        _ => {}
    }
}

/// SlopeSlide Tick 0x14126C950.
fn slope_tick(pw: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) {
    let w = host.world;
    let dt = cx.dt;
    // Not on the ground (FUN_1405AD850) and the jump request with +0x8C accepted -> vt+0x270.
    if !pw.on_ground() {
        // (B5) RequestJump -> NavLinkJump's fall (crate::jump)
        crate::jump::request_fall(pw, host.world);
        return;
    }
    let loc = pw.position();
    // State 0x16 present: the rail probe at the location (range 50; want = +0x8C with input, else the facing).
    if has_rails(w, cx) {
        let want = if pw.trav.feed.on { forward(pw.trav.feed.yaw) } else { forward(pw.yaw) };
        if let Some(hit) = rail_probe(&mut pw.trav, w, cx.ice_rails, loc, loc, p("slide.rail_range"), want) {
            end(pw);
            begin_rail(pw, hit);
            return;
        }
    }
    match pw.trav.sub {
        0 => {
            // (B5) state 7 present: the climb probe FUN_140634E60 -> FUN_140636BF0 -> vt+0x538 (the leg = +0x84
            // bit 1, 0 here) -> vt+0x268 (0x14126C950 lines with LAB_14126CC34).
            if let Some(hit) = crate::climb::climb_probe(pw, w) {
                crate::climb::request_climb(pw, hit, false, false);
                return;
            }
            // The hop probe at location + Rot x (120, 0, 0), Rot (0, +0x8C, 0), radius 100.
            if !w.hop_points().is_empty() {
                let at = loc + rot_dir(pw.trav.pitch, pw.yaw) * p("slide.hop_ahead");
                let yaw = pw.trav.feed.yaw;
                if let Some(pt) = hop_probe(&pw.trav, w, at, yaw, p("slide.hop_radius")) {
                    end(pw);
                    begin_hop(pw, pt, yaw, false, 0);
                    return;
                }
            }
            let probe = slope_probe(w, loc, pw.yaw, 1);
            if !probe.ok {
                if pw.trav.feed.on {
                    // input (the AI still moves): vt+0x268, back to the run
                    back(pw);
                    return;
                }
                // no input: StopMovement + input cleared (vt+0x16E8), `RunEndR` (blend 0.1, mode 2), sub 1,
                // the rotation set to its yaw only (FUN_143EAD160).
                pw.cmc.stop_movement();
                pw.cmc.control_input = Vec3::ZERO;
                pw.play("RunEndR", 0.1);
                pw.trav.sub = 1;
                pw.trav.pitch = 0.0;
            }
            pw.trav.slope_pitch = probe.angle;
        }
        _ => {
            // sub 1: the end clip's end -> vt+0x268
            if pw.anim.finished {
                back(pw);
            }
            return;
        }
    }
    // LAB_14126D080: vt+0xDB0 toward Rot(sub 0 ? +0x90 + 25 : 0, +0x8C, 0) with alpha dt x 6;
    // MaxAcceleration = MaxWalkSpeed x 60; AddMovementInput(Rot.Vector(), 1); +0x88 = 0.
    let a = dt * p("slide.turn_rate");
    let target_pitch = if pw.trav.sub == 0 { pw.trav.slope_pitch + p("slide.pitch_add") } else { 0.0 };
    pw.yaw = interp_yaw(pw.yaw, pw.trav.feed.yaw, a);
    pw.trav.pitch += (target_pitch - pw.trav.pitch) * a.clamp(0.0, 1.0);
    pw.cmc.max_acceleration = pw.cmc.max_walk_speed * p("slide.accel_factor");
    pw.cmc.control_input = rot_dir(pw.trav.pitch, pw.yaw);
    pw.trav.feed.on = false;
}

/// SuperSlide Tick 0x14126D3B0.
fn super_tick(pw: &mut NpcPawn, cx: &StepCtx, host: &PartyHost) {
    let dt = cx.dt;
    // MaxWalkSpeed = m_SuperSlideSpeed (vt+0x1AB0 -> +0xB30) every tick.
    pw.cmc.max_walk_speed = p("superslide.speed");
    let mut sub = pw.trav.sub;
    match sub {
        0 => {
            // (B5) +0x90 clear (always: only Init writes it) and state 7 present: the climb probe FUN_140634E60 ->
            // the request vt+0x538 (leg = +0x84 bit 1) with the Climbing's +0xB5 = 1 (SonicSlideClimbing) -> vt+0x270.
            if !pw.trav.sup_flag {
                if let Some(hit) = crate::climb::climb_probe(pw, host.world) {
                    crate::climb::request_climb(pw, hit, false, true);
                    return;
                }
            }
            // A turn of 75 deg or more stops the speed.
            if !pw.trav.sup_flag && wrap180(pw.trav.feed.yaw - pw.yaw).abs() >= p("superslide.stop_angle") {
                pw.cmc.stop_movement();
                pw.cmc.control_input = Vec3::ZERO;
            }
            if pw.anim.clip.name != "SonicSlideLoop" {
                let c = tables::clip(pw.chara, "SonicSlideLoop");
                pw.anim.play_loop(c, 0.2);
            }
        }
        1 => {
            pw.cmc.stop_movement();
            pw.cmc.control_input = Vec3::ZERO;
            pw.play("SonicSlide", 0.1);
            sub = 2;
        }
        2 => {
            if pw.anim.finished {
                let c = tables::clip(pw.chara, "SonicSlideLoop");
                pw.anim.play_loop(c, 0.2);
                sub = 0;
            }
        }
        3 => {
            if pw.anim.finished {
                pw.trav.sub = sub;
                back(pw);
                return;
            }
            pw.trav.sub = sub;
            return;
        }
        _ => {
            let c = tables::clip(pw.chara, "SonicSlideLoop");
            pw.anim.play_loop(c, 0.2);
            sub = 0;
        }
    }
    pw.trav.sub = sub;
    if sub == 0 {
        let rate = if pw.trav.sup_rate > 0.0 { pw.trav.sup_rate } else { p("superslide.turn_rate") };
        pw.yaw = interp_yaw(pw.yaw, pw.trav.feed.yaw, dt * rate);
    }
    if !pw.trav.feed.on {
        // no input this tick and no acceleration (|Acceleration.xy|^2 <= 1): StopMovement, `SlideEnd`
        // (blend 0.1, mode 2), +0x94 = 0, sub 3.
        if sub == 0 && pw.cmc.acceleration.truncate().length_squared() <= 1.0 {
            pw.cmc.stop_movement();
            pw.cmc.control_input = Vec3::ZERO;
            pw.play("SlideEnd", 0.1);
            pw.trav.sup_hold = 0.0;
            pw.trav.sub = 3;
        }
        return;
    }
    // movement vt+0x918(+0x94 <= 0 ? +0x90 : 0) not modelled; +0x94: 0.2 without +0x90, else counts down.
    if !pw.trav.sup_flag {
        pw.trav.sup_hold = p("superslide.input_hold");
    } else {
        pw.trav.sup_hold = (pw.trav.sup_hold - dt).max(0.0);
    }
    if sub == 0 {
        let d = forward(pw.trav.feed.yaw);
        let max = pw.cmc.max_walk_speed;
        if !pw.on_ground() {
            // the air: Velocity + AirControl x dir x 3888.0002 x dt, XY clamped to MaxWalkSpeed
            let mut v = pw.cmc.velocity + d * pw.params.air_control * p("superslide.air_accel") * dt;
            let xy = Vec2::new(v.x, v.y);
            if max >= 1.0e-4 && xy.length_squared() > max * max {
                let c = xy.normalize() * max;
                v.x = c.x;
                v.y = c.y;
            }
            pw.cmc.set_velocity_xy(Vec2::new(v.x, v.y));
        } else if pw.trav.sup_rate <= 0.0 {
            pw.cmc.set_velocity_xy(Vec2::new(d.x, d.y) * max);
        } else {
            let f = forward(pw.yaw);
            pw.cmc.set_velocity_xy(Vec2::new(f.x, f.y) * max);
        }
    }
    pw.trav.feed.on = false;
}

/// RailSlide Tick 0x14124E890 (+ the base Tick 0x141510EE0 as kh-core ports it for Sora; the leave launch is
/// the NPC's slot 145 0x141252210).
fn rail_tick(pw: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) {
    let dt = cx.dt;
    if !pw.trav.rail.invincible {
        // The off-rail warp: no query yet, the leader not in states 4 / 8 / 9 / 0x16 / 0x8A and farther than
        // FUN_14125FF90 (FLT_MAX without a friend component, 1000 in the battle-start window, else 3000) ->
        // the warp EQS (friend data +0x60); its callback 0x141250A00 keeps the point (+0x191, delay 1 s, limit 2 s).
        if !pw.trav.rail.warp_found {
            let leader_busy = matches!(cx.sora.state, 4 | 8 | 9 | 0x16 | 0x8A);
            let window = pw.in_battle && pw.battle_time < tables::param("warp.battle_window");
            let limit = tables::param(if window { "warp.threshold_battle" } else { "warp.threshold" });
            if !pw.trav.rail.warp_query && !leader_busy && pw.position().distance_squared(cx.sora.position) > limit * limit {
                // (B5) The query: FEnvQueryRequest(TresFriendData.WarpQuery +0x60 (FUN_141260020), querier = the pawn)
                // .Execute(SingleResult, the delegate +0x1B0) 0x143C5A420, +0x194 = its id. kh-ai runs the game's
                // query (EQSQuery_NpcWarp) for the body that asks (`AiBody::wants_warp_query`) and the answer comes
                // back through `warp_query_answer` (the callback 0x141250A00).
                pw.trav.rail.warp_query = true;
            }
        } else if pw.trav.rail.warp_delay > 0.0 {
            pw.trav.rail.warp_delay -= dt;
        } else {
            pw.trav.rail.warp_limit -= dt;
            if pw.trav.rail.warp_limit <= 0.0 {
                pw.trav.rail.warp_found = false;
            } else if warp_out_of_view(pw, cx, limit_of(pw)) {
                // vt+0x4D0: to the point, facing the leader; the rail probe there (segment +30, range 150,
                // the facing) -> RailSlide request; vt+0x270.
                let to = pw.trav.rail.warp_point;
                let d = horizontal(cx.sora.position - to);
                let from = pw.position();
                if d.length_squared() > 1e-4 {
                    pw.yaw = yaw_of(d);
                }
                end(pw);
                pw.cmc.location = to;
                pw.cmc.stop_movement();
                pw.events.push(PartyEventKind::Warped { from, to });
                let want = forward(pw.yaw);
                let hit = rail_probe(&mut pw.trav, host.world, cx.ice_rails, to, to + Vec3::Z * 30.0, p("rail.warp_probe_range"), want);
                match hit {
                    Some(h) => begin_rail(pw, h),
                    None => back(pw),
                }
                return;
            }
        }
    }
    rail_base_tick(pw, cx, host.world);
}

/// (B5) True while the RailSlide state waits for its warp query's answer (+0x194 != -1).
pub fn warp_query_wanted(pw: &NpcPawn) -> bool {
    pw.state == NpcStateId::RailSlide && pw.trav.rail.warp_query
}

/// (B5) The query's answer (the delegate 0x141250A00): items found -> +0x1A0 = the first item's location,
/// +0x191 = 1, +0x19C = 1 s, +0x198 = 2 s; in any case +0x194 = -1 (the tick may ask again).
pub fn warp_query_answer(pw: &mut NpcPawn, best: Option<Vec3>) {
    if !warp_query_wanted(pw) {
        return;
    }
    let r = &mut pw.trav.rail;
    r.warp_query = false;
    if let Some(at) = best {
        r.warp_found = true;
        r.warp_point = at;
        r.warp_delay = p("rail.warp_delay");
        r.warp_limit = p("rail.warp_limit");
    }
}

fn limit_of(pw: &NpcPawn) -> f32 {
    let window = pw.in_battle && pw.battle_time < tables::param("warp.battle_window");
    tables::param(if window { "warp.threshold_battle" } else { "warp.threshold" })
}

/// FUN_1412405C0: still farther than the limit from the leader, and neither the friend's bounds sphere nor
/// the point (+ Z x radius) is in the camera (FUN_140E5AC70).
fn warp_out_of_view(pw: &NpcPawn, cx: &StepCtx, limit: f32) -> bool {
    if pw.position().distance_squared(cx.sora.position) < limit * limit {
        return false;
    }
    let r = pw.params.capsule_radius.max(pw.params.capsule_half_height);
    match cx.sora.camera {
        Some(cam) => !cam.sees(pw.capsule_centre(), r) && !cam.sees(pw.trav.rail.warp_point + Vec3::Z * r, r),
        None => true,
    }
}

/// UTresCharState_RailSlide Tick 0x141510EE0 with PhysRailSlide 0x14058ECF0 (kh-core's port of the shared
/// base; (B5) the level rails and Sora's ice rails).
fn rail_base_tick(pw: &mut NpcPawn, cx: &StepCtx, w: &dyn World) {
    let dt = cx.dt;
    let Some(path) = rail_path(w, cx.ice_rails, pw.trav.rail.id) else {
        back(pw);
        return;
    };
    let r = &mut pw.trav.rail;
    (r.narrow, r.no_reverse, r.no_jump) = (path.narrow, path.no_reverse, path.no_jump);
    if pw.trav.sub == 0 {
        // sub 0: facing and location snap to the attach point; the base speed (pawn MyRailSlideSpeed 2000);
        // paused; `RailSlideStart` (slot 129 0x141514E90).
        pw.yaw = wrap180(r.yaw);
        pw.cmc.location = r.attach;
        r.speed = A.rail_speed;
        r.paused = true;
        r.waiting = true;
        pw.cmc.stop_movement();
        pw.play("RailSlideStart", -1.0);
        pw.trav.sub = 1;
        return;
    }
    if pw.trav.sub == 1 {
        if pw.trav.rail.waiting && pw.anim.finished {
            pw.trav.rail.waiting = false;
        }
        if pw.trav.rail.waiting {
            return;
        }
        // StartLoop (slot 130 0x141514AD0): StopMovement, the loop (mode 3), unpaused, the turn lock-out.
        pw.cmc.stop_movement();
        let name = if pw.trav.rail.narrow { "RailSlideNarrowLoop" } else { "RailSlideLoop" };
        let c = tables::clip(pw.chara, name);
        pw.anim.play_loop(c, -1.0);
        let r = &mut pw.trav.rail;
        r.paused = false;
        if !r.must_release {
            pw.trav.feed.on = false;
        }
        r.turn_lock = A.rail_turn_lockout;
        r.turn_armed = false;
        pw.trav.sub = 2;
    }
    let r = pw.trav.rail;
    let (mut along, _) = path.closest(pw.position(), pw.position());
    let length = path.length();
    let speed = path.speed_at(along, A.rail_speed);
    if !r.paused {
        along += if r.reversed { -1.0 } else { 1.0 } * speed * dt;
    }
    let ended = along >= length || along <= 0.0;
    along = along.clamp(0.0, length);
    let (position, fwd) = path.at(along);
    let travel = if r.reversed { -fwd } else { fwd };
    pw.cmc.location = position;
    // rot = Slerp(current, rail, 0.5) per movement tick. PORT: the 60 Hz tick's 0.5 as 1 - 0.5^(dt x 60).
    let alpha = 1.0 - (1.0 - A.rail_rotation_slerp).powf(dt * 60.0);
    pw.yaw = interp_yaw(pw.yaw, yaw_of(travel), alpha);
    pw.trav.rail.speed = speed;
    if ended {
        pw.trav.rail.ended = true;
        // The LEAVE branch (0x141511AE6): the automatic launch (slot 145) when -forward.z < 0.86.
        let launch = -travel.z < A.rail_end_launch_max_down;
        rail_leave(pw, travel, launch, w);
        return;
    }
    // The lean (slot 132): the signed angle to the direction 200 cm further along -> the blend X.
    let ahead = if r.reversed { along - A.rail_lean_lookahead } else { along + A.rail_lean_lookahead };
    let (_, fa) = path.at(ahead.clamp(0.0, length));
    let lean = wrap180(yaw_of(if r.reversed { -fa } else { fa }) - yaw_of(travel));
    pw.anim.axis = lean;
    // The turn-around (0x141511802 - 0x1415119FD): the AI direction more than 120 deg off the travel.
    let feed = pw.trav.feed;
    let r = &mut pw.trav.rail;
    if r.turn_armed && !r.narrow && !r.no_reverse {
        if feed.on && !r.must_release && forward(pw.yaw).dot(forward(feed.yaw)) < A.rail_turn_dot {
            r.reversed = !r.reversed;
            pw.yaw = wrap180(yaw_of(-travel));
            r.paused = true;
            pw.cmc.stop_movement();
            r.waiting = true;
            r.must_release = true;
            pw.play("RailSlideTurn", -1.0);
            pw.trav.sub = 1;
        }
    } else if r.turn_lock > 0.0 {
        r.turn_lock -= dt;
        if r.turn_lock <= 0.0 {
            r.turn_lock = 0.0;
            r.turn_armed = true;
        }
    }
    let r = &mut pw.trav.rail;
    if !feed.on {
        r.must_release = false;
    }
    pw.trav.feed.on = false;
}

/// NPC RailSlide slot 145 0x141252210: the forward direction (XY), the point 500 cm ahead projected by
/// FUN_140542040 (1000 down) -> JumpCodeDriven there (h_min 400, flags 0x101); without a launch the
/// default state (falling).
fn rail_leave(pw: &mut NpcPawn, travel: Vec3, launch: bool, w: &dyn World) {
    end(pw);
    if !launch {
        back(pw);
        return;
    }
    let f = horizontal(travel).normalize_or_zero();
    let mut target = pw.position() + f * p("rail.leave_distance");
    // FUN_140542040: the ground under the point (a line down 1000), else the point itself.
    if let Some(h) = w.line_trace(target, target - Vec3::Z * 1000.0) {
        target = h.position;
    }
    // (B5) JumpCodeDriven (iface vt+0x3A8 0x14054C6A0 -> vt+0x390): +0x9C = 400, +0xA8 / +0xA9 = 1 (0x101: the air
    // steering is always on) -> crate::jump.
    crate::jump::request_code_driven(pw, target, p("rail.leave_height_min"), true, w);
}

/// Hopping Tick 0x14124C010.
fn hop_tick(pw: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) {
    let dt = cx.dt;
    let w = host.world;
    let h = pw.trav.hop;
    if !w.hop_points().iter().any(|c| c.id == h.point && c.active) {
        back(pw);
        return;
    }
    match pw.trav.sub {
        0 => {
            // attach (FUN_143E8FC60), +0x98 = the location, StopMovement, Flying (vt+0x17A0(5): Custom here,
            // the state places him), HopL / HopR by +0x84 bit 1 (blend +0xC1 ? 0.2 : 0), sub 1, +0xB0 = 1.
            pw.trav.hop.start = pw.position();
            pw.cmc.stop_movement();
            pw.cmc.set_custom();
            pw.cmc.gravity_off = true;
            let was = pw.trav.hop.reverse_leg;
            pw.trav.hop.reverse_leg = !was;
            pw.play(if was { "HopR" } else { "HopL" }, 0.0);
            pw.trav.sub = 1;
            pw.trav.hop.moving = true;
        }
        1 => {
            let mut blend = 0.0;
            if pw.trav.hop.moving {
                // P = Lerp(start, target, clip time / length); the step = VInterpConstantTo at MaxWalkSpeed;
                // the nearer of the two; arrived (+0xB0 = 0, blend 0.1) within one step or past the target.
                let len = pw.anim.clip.length.max(1e-4);
                let t = (pw.anim.time / len).clamp(0.0, 1.0);
                let start = h.start - h.target;
                let current = pw.position() - h.target;
                let a = start * (1.0 - t);
                let step = pw.cmc.max_walk_speed * dt;
                let b = current - start.normalize_or_zero() * step;
                let mut q = if b.length_squared() <= a.length_squared() { b } else { a };
                if q.length_squared() < step * step || current.length_squared() < q.length_squared() {
                    q = Vec3::ZERO;
                    pw.trav.hop.moving = false;
                    blend = 0.1;
                }
                pw.cmc.location = h.target + q;
                // vt+0xDB0 toward the point's direction (XY), alpha dt x 6.
                let flat = -horizontal(q);
                if flat.length_squared() > 1e-4 {
                    pw.yaw = interp_yaw(pw.yaw, yaw_of(flat), dt * p("hop.turn_rate"));
                }
            }
            if !pw.anim.finished && pw.trav.hop.moving {
                return hop_tail(pw);
            }
            // on the point: snap, StopMovement, m_LastHitHopComponent = it; an end point: sub 4, vt+0x270.
            pw.cmc.location = h.target;
            pw.cmc.stop_movement();
            pw.trav.last_hop = Some(h.point);
            if h.end_point {
                pw.trav.sub = 4;
                back(pw);
                return;
            }
            let name = if h.idle_motion == 3 { "IdleN" } else if pw.trav.hop.reverse_leg { "BalanceL" } else { "BalanceR" };
            let c = tables::clip(pw.chara, name);
            pw.anim.play_loop(c, blend);
            pw.trav.hop.counter += 1;
            pw.trav.sub = 2;
            pw.trav.hop.moving = false;
        }
        2 => {
            let f = pw.trav.feed;
            if pw.anim.time >= p("hop.wait_min") && f.magnitude > p("hop.turn_min") {
                if f.magnitude > p("hop.next_min") {
                    // the next point: probes 150, 200 .. 350 along +0xA8, radius 100, the highest wins.
                    let mut best: Option<HopPoint> = None;
                    let mut d = p("hop.search_start");
                    while d < p("hop.search_limit") {
                        if let Some(c) = hop_probe(&pw.trav, w, pw.position() + forward(f.yaw) * d, f.yaw, p("hop.search_radius")) {
                            if best.is_none_or(|b| c.position.z > b.position.z) {
                                best = Some(c);
                            }
                        }
                        d += p("hop.search_step");
                    }
                    match best {
                        Some(next) => {
                            // RequestHopping (vt+0x530) with +0x84 bit 1; the new state's +0xB4 = ours; vt+0x270.
                            let (leg, counter) = (pw.trav.hop.reverse_leg, pw.trav.hop.counter);
                            let last = pw.trav.last_hop;
                            end(pw);
                            pw.trav.last_hop = last;
                            begin_hop(pw, next, f.yaw, leg, counter);
                            return;
                        }
                        None => {
                            // hop off (m_bIsClosing not in the seam: off): StopMovement, the other foot's clip
                            // (blend 0), sub 3.
                            pw.cmc.stop_movement();
                            let was = pw.trav.hop.reverse_leg;
                            pw.trav.hop.reverse_leg = !was;
                            pw.play(if was { "HopR" } else { "HopL" }, 0.0);
                            pw.trav.sub = 3;
                            return hop_tail(pw);
                        }
                    }
                }
                pw.yaw = interp_yaw(pw.yaw, f.yaw, dt * p("hop.turn_rate"));
            }
        }
        3 => {
            let f = pw.trav.feed;
            pw.yaw = interp_yaw(pw.yaw, f.yaw, dt * p("hop.turn_rate"));
            if f.on {
                // AddMovementInput(Rot(0, +0xA8, 0)): the hop-off moves along it at MaxWalkSpeed.
                let d = forward(f.yaw) * pw.cmc.max_walk_speed;
                pw.cmc.location += d * dt;
            }
            if pw.anim.finished {
                // RequestJump(+0xA8) -> vt+0x270 (B5: the NavLinkJump fall; its vt+0x3B0 yaw is a no-op, the fall
                // keeps the actor yaw)
                crate::jump::request_fall(pw, host.world);
                return;
            }
        }
        _ => {
            if pw.anim.finished {
                back(pw);
                return;
            }
        }
    }
    hop_tail(pw)
}

/// The tick's tail (LAB_14124CCAF): vt+0xDC8(dt x 6), +0xA5 = 0, +0xAC = 0.
fn hop_tail(pw: &mut NpcPawn) {
    pw.trav.feed.on = false;
    pw.trav.feed.magnitude = 0.0;
}

// ============================================================================ situations

/// The inner machine's enter functions (0x141247430's table).
fn sit_enter(pw: &mut NpcPawn, mode: u8) {
    let old = pw.trav.sit.mode;
    // exits: GimmickWait 0x141245540 (vt+0x2410(0), +0xB96 = 1); NpcMotionPlay 0x1412455A0 (vt+0x268 and,
    // still in 0x93, the Idle request).
    pw.trav.sit.mode = mode;
    if old == 3 && mode != 3 && pw.state == NpcStateId::SituationAction {
        end(pw);
        pw.to_idle();
        return;
    }
    match mode {
        // TalkPointWait 0x141242710: +0xF4 = 0.
        1 => pw.trav.sit.phase = 0,
        // GimmickWait 0x141242620: vt+0x2410(1), +0xB96 = 0, +0xF4 = 0.
        2 => pw.trav.sit.phase = 0,
        // NpcMotionPlay 0x1412426A0: +0xF4 = 0, +0xFC = 0, +0x120 = 0.
        3 => {
            pw.trav.sit.phase = 0;
            pw.trav.sit.done = 0;
            pw.trav.sit.finish = false;
        }
        _ => {}
    }
}

/// SituationAction Tick 0x14124F1E0 (the inner machine) / Goofy's 0x1411CB950 (situation 7 -> 0x1411CB9F0).
fn situation_tick(pw: &mut NpcPawn, cx: &StepCtx) {
    let s = pw.trav.sit;
    match s.mode {
        1 => {
            // TalkPointWait 0x14124F6B0: FUN_14150DAF0 (the player controller), +0xF4 = 1 once; it waits.
            if s.phase == 0 {
                pw.trav.sit.phase = 1;
            }
        }
        2 => {
            // GimmickWait 0x14124F280: once, the idle clip (vt+0x1C68 with the battle flag) mode 3.
            if s.phase == 0 {
                let name = if pw.in_battle { "IdleB" } else { "IdleN" };
                let c = tables::clip(pw.chara, name);
                pw.anim.play_loop(c, -1.0);
                pw.trav.sit.phase = 1;
            }
        }
        3 => motion_tick(pw),
        7 => coop_tick(pw, cx),
        _ => {}
    }
}

/// NpcMotionPlay 0x14124F360: phase 0 the start motion (+0x108, or the loop +0x110 when none, counting it),
/// 1 the loop motion played +0x104 times (< 0: for ever), 2 (or at once on the finish request) the end
/// motion +0x118, 3 its end; a missing motion ends the mode (-> Wait: the state ends). Mode 2 = once, 3 =
/// loop (count < 0).
fn motion_tick(pw: &mut NpcPawn) {
    let s = pw.trav.sit;
    let has = |pw: &NpcPawn, n: Option<&'static str>| n.filter(|n| tables::anim_set(pw.chara).get(n).is_some());
    // PlayAnim(name, -1, mode): 2 once, 3 loop.
    let play = |pw: &mut NpcPawn, n: &'static str, looping: bool| {
        let c = tables::clip(pw.chara, n);
        if looping {
            pw.anim.play_loop(c, -1.0);
        } else {
            pw.anim.play(c, -1.0);
        }
        pw.hit_list.clear();
    };
    let loop_bit = s.count < 0;
    match s.phase {
        0 => {
            // no start motion: the loop motion stands in (counted; the count reached -> phase 2, so the
            // phase step below makes it 3 and the end motion is skipped); else the start plays once.
            let mut looping = false;
            if s.anims[0].is_none() && s.anims[1].is_some() {
                pw.trav.sit.anims[0] = s.anims[1];
                looping = loop_bit;
                if !loop_bit {
                    pw.trav.sit.done += 1;
                    if pw.trav.sit.done >= s.count {
                        pw.trav.sit.phase = 2;
                    }
                }
            }
            match has(pw, pw.trav.sit.anims[0]) {
                Some(n) => {
                    play(pw, n, looping);
                    pw.trav.sit.phase += 1;
                }
                None => sit_enter(pw, 0),
            }
        }
        1 => {
            if !pw.anim.finished {
                return;
            }
            match has(pw, s.anims[1]) {
                Some(n) => {
                    play(pw, n, loop_bit);
                    if loop_bit {
                        return;
                    }
                    pw.trav.sit.done += 1;
                    if pw.trav.sit.done >= s.count {
                        pw.trav.sit.phase += 1;
                    }
                }
                None => sit_enter(pw, 0),
            }
        }
        2 => {
            if !s.finish && !pw.anim.finished {
                return;
            }
            match has(pw, s.anims[2]) {
                Some(n) => {
                    play(pw, n, false);
                    pw.trav.sit.phase = 3;
                }
                None => sit_enter(pw, 0),
            }
        }
        3 => {
            if pw.anim.finished {
                pw.trav.sit.phase = 4;
                sit_enter(pw, 0);
            }
        }
        _ => sit_enter(pw, 0),
    }
}

/// Goofy's situation 7 (n_ex002 SituationAction 0x1411CB9F0): `GCoopStart` (mode 2), turning to the leader
/// at 180 deg/s (vt+0xDE0) until it ends, then `GCoopLoop` (mode 3); in the loop the situation command 0xCE
/// is offered (vt+0x2618(0xCE, 0, 1)) while the coop actor (pawn +0xBE8) is within 500 cm (XY) of the
/// leader, and taken back (FUN_14125CE90) when it is not.
fn coop_tick(pw: &mut NpcPawn, cx: &StepCtx) {
    match pw.trav.sit.coop_phase {
        0 => {
            pw.play("GCoopStart", -1.0);
            pw.trav.sit.coop_phase = 1;
        }
        1 => {
            pw.turn_toward(cx.sora.position, cx.dt, p("situation.coop_turn"));
            if pw.anim.finished {
                let c = tables::clip(pw.chara, "GCoopLoop");
                pw.anim.play_loop(c, -1.0);
                pw.trav.sit.coop_phase = 2;
            }
        }
        _ => {
            let r = p("situation.coop_range");
            let near = pw.trav.coop_target.is_some_and(|t| horizontal(t - cx.sora.position).length_squared() <= r * r);
            if near && !pw.trav.sit.coop_shown {
                pw.events.push(PartyEventKind::SituationCommand { command: 0xCE, on: true });
                pw.trav.sit.coop_shown = true;
            } else if !near && pw.trav.sit.coop_shown {
                pw.events.push(PartyEventKind::SituationCommand { command: 0xCE, on: false });
                pw.trav.sit.coop_shown = false;
            }
        }
    }
}

/// `SetSituation(type, value)` (pawn vt+0x2888 = ATresNpcPawnBase 0x141277550): the same pair again does
/// nothing; 0 ends a running situation (vt+0x2890 -> the Idle request); 6 / 8 request SituationAction (when
/// the pawn has it); 3 / 5 set the BT key `GimmickActionLocation` (C's: not modelled here); the pair is kept
/// (+0xBD4 / +0xBD8). Goofy's 7 also runs his override when the state is requested.
pub fn set_situation(pw: &mut NpcPawn, kind: u8, value: f32) -> bool {
    if pw.trav.situation == kind && pw.trav.situation_value == value {
        return true;
    }
    let was = pw.trav.situation;
    pw.trav.situation = kind;
    pw.trav.situation_value = value;
    match kind {
        0 => {
            if was != 0 && pw.state == NpcStateId::SituationAction {
                end(pw);
                pw.to_idle();
            }
        }
        6 | 8 | 7 => {
            if pw.state == NpcStateId::Die {
                return true;
            }
            pw.abort_command_pub();
            begin_situation(pw);
        }
        _ => {}
    }
    true
}

/// `RequestNpcMotionPlay` (pawn 0x141270750): refused while vt+0xA50 (a blocking state) or with no motion;
/// SituationAction requested (or the running one reused), +0x104 = the count (< 0 loops the middle motion),
/// +0x108 / +0x110 / +0x118 = start / loop / end, situation 9; the controller's focus on the action point is
/// not modelled.
pub fn request_npc_motion(pw: &mut NpcPawn, count: i32, anims: [Option<&'static str>; 3]) -> bool {
    if anims.iter().all(|a| a.is_none()) || matches!(pw.state, NpcStateId::Die | NpcStateId::Fr(_)) {
        return false;
    }
    pw.trav.sit.count = count;
    pw.trav.sit.anims = anims;
    pw.trav.situation = 9;
    pw.abort_command_pub();
    begin_situation(pw);
    true
}

/// 0x141270970 (iface): in SituationAction -> FUN_141245900 (+0x120 = 1, +0xF4 = 2: the end motion next).
pub fn finish_npc_motion(pw: &mut NpcPawn) -> bool {
    if pw.state == NpcStateId::SituationAction && pw.trav.sit.mode == 3 {
        pw.trav.sit.finish = true;
        pw.trav.sit.phase = 2;
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use kh_core::test_world::TestWorld;

    #[test]
    fn slope_probe_needs_a_flagged_steep_floor_facing_down() {
        // a 30 deg plane descending toward +X, flagged on component 1 (the plane's id in TestWorld)
        let a = 30.0f32.to_radians();
        let n = Vec3::new(a.sin(), 0.0, a.cos());
        let w = TestWorld::default().with_plane_through(Vec3::new(0.0, 0.0, 0.0), n).with_flags(1, surface::SLOPE_SLIDE);
        let loc = Vec3::new(0.0, 0.0, 0.0);
        let down = slope_probe(&w, loc, 0.0, 0);
        assert!((down.angle + 30.0).abs() < 0.5, "fall angle {}", down.angle);
        assert!(down.ok, "facing downhill on 30 deg starts");
        assert!(!slope_probe(&w, loc, 180.0, 0).ok, "facing uphill does not");
        assert!(!slope_probe(&w, loc, 60.0, 0).ok, "60 deg off the fall line is outside cos 50");
        assert!(slope_probe(&w, loc, 60.0, 1).ok, "but inside the continuation's cos 80");
    }
}
