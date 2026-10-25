//! `TresPlayerState_WallRun` (id 8) and its request `TryWallRun`: docs/sora-actions-spec.md II.4
//! "Wall Run" (instruction level; addresses are the exe's). OWNER: the actions implementer.
//!
//! A wall whose component is flagged `SQEX_bEnableWallRun` (`World::surface_flags`), met while
//! falling in a Jump with the stick held or while running with the stick above 0.5 (the world-hit
//! event, `actions::move_blocked`): the stick within 35 degrees of "into the wall" runs UP it
//! (class 0), 50 - 130 degrees off runs ALONG it (class 1: toward his left when facing it, 2:
//! right). Flying mode; the displacement is the clips' root motion (the loops at scale 1.84:
//! 1251 cm/s) plus, in the up-run, a lateral drift of 980 cm/s along the stick. The stick must be
//! held (0.2 s grace); the stick turned 80 degrees or more for 0.1 s changes the direction; the
//! jump button is a super jump (SUPERJUMP), the guard button a wall kick that ends in a
//! Superslide (SUPERSLIDE + WALL_KICK); the run ends over the top (onto a top flagged
//! `SQEX_bEnableAutoRun`: the ledge climb-up), under a ceiling, or when the wall ends.
//!
//! SIMPLIFIED (each where it is): the actor has no pitch in this port, so a leaning wall is run
//! with an upright body; the corner look-ahead of the probe (`roll`: the `WallRunRoll*` clips)
//! and the wall camera are not made; the flying move's own use of the wall direction (probe W1)
//! is the plain flying move.

use glam::{Vec2, Vec3};

use super::common::on_ground;
use super::{dangle, jump, wall_kick, PlayerState, StateData, StateId};
use crate::actions::probes::{self, dir, pitch_of, yaw_of};
use crate::actions::{self, A};
use crate::anim::{AnimId, PlayArgs};
use crate::enums::ETresAbilityKind as Ability;
use crate::host::EventKind;
use crate::pawn::{Ctx, Pawn};
use crate::sora::{wrap_degrees, MovementState};
use crate::world::Hit;

/// The fields of II.4's table.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WallRun {
    /// 0 entry, 6 entry clip, 1 loop, 4 change clip, 7 leave clip.
    pub(super) sub: u8,
    /// +0x90, +0x9C.
    component: u32,
    normal: Vec3,
    /// +0x98 bit 1 stick this frame, bit 2 wall valid, bit 3 entered airborne, bit 6 guard latch,
    /// bit 7 ceiling found.
    stick: bool,
    valid: bool,
    air_entry: bool,
    pub(super) guard_latch: bool,
    ceiling: bool,
    /// +0xA8: 0 up, 1 along toward his left when facing the wall, 2 the mirror.
    class: u8,
    /// +0xAC, +0xB0, +0xB4, +0xB8.
    stick_yaw: f32,
    grace: f32,
    switch: f32,
    blocked: f32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Cmd {
    None,
    Left,
    Right,
    Back,
}

impl WallRun {
    /// (combat implementer) +0xA8 for the attack request (slot 45 0x1414377B1: next+0x1E4 = WallRun+0xA8).
    pub(super) fn class(&self) -> u8 {
        self.class
    }


    /// Reset (0x1413D0F80).
    pub const RESET: WallRun = WallRun { sub: 0, component: 0, normal: Vec3::ZERO, stick: false, valid: false, air_entry: false, guard_latch: false, ceiling: false, class: 0, stick_yaw: 0.0, grace: 0.0, switch: 0.0, blocked: 0.0 };

    /// `FACE(class, Rot(N))`: the actor's rotation on the wall (yaw only here).
    fn face(s: &mut Pawn, class: u8, normal: Vec3) {
        let yaw = yaw_of(normal);
        s.yaw = wrap_degrees(match class {
            0 => yaw + 180.0,
            1 => yaw + 90.0,
            _ => yaw - 90.0,
        });
    }

    /// `LOOP(blendIn)`: the loop of the class with its root scale (`MyWallRunSpeedScaleUp` /
    /// `...Side` 1.84: the second element of the pair scales the vertical root motion).
    fn play_loop(&self, s: &mut Pawn, blend_in: f32) {
        let (clip, scale) = match self.class {
            0 => (AnimId::WallRunLoopF, (1.0, A.wallrun_root_scale_up)),
            1 => (AnimId::WallRunLoopR, (A.wallrun_root_scale_side, 1.0)),
            _ => (AnimId::WallRunLoopL, (A.wallrun_root_scale_side, 1.0)),
        };
        s.root_scale = scale;
        s.anim.play_with(clip, PlayArgs::mode(3, blend_in));
    }

    /// `LEDGE()` (0x1413D95A9): the top he can run out onto -> `TryDangle` (the wall-run
    /// variant: straight to the climb-up).
    fn ledge(&mut self, state_cx: &mut Ctx) -> bool {
        let Some((component, point, yaw)) = probes::wall_top(state_cx.pawn, state_cx.world) else { return false };
        let s = &mut *state_cx.pawn;
        // The hang point under the top: his own place on the wall, 150 cm below the top.
        let ledge = probes::Ledge { component, position: Vec3::new(s.cmc.location.x, s.cmc.location.y, point.z - 150.0), yaw, contact: point };
        if dangle::push_from_wall_run(s, ledge) {
            s.queue.do_chain();
            return true;
        }
        false
    }

    /// `CHANGE(class, cmd)` (0x1413D9304). Returns false when it ended the state.
    fn change(&mut self, s: &mut Pawn, cmd: Cmd) -> bool {
        let half = (A.wallrun_change_reverse_scale, A.wallrun_change_reverse_scale);
        let (class, clip, scale) = match (self.class, cmd) {
            (0, Cmd::Left) => (1, AnimId::WallRunChangeFR, (1.0, 1.0)),
            (0, Cmd::Right) => (2, AnimId::WallRunChangeFL, (1.0, 1.0)),
            (1, Cmd::Back) => (2, AnimId::WallRunChangeL, half),
            (1, Cmd::Right) => (0, AnimId::WallRunChangeRF, (1.0, 1.0)),
            (2, Cmd::Back) => (1, AnimId::WallRunChangeR, half),
            (2, Cmd::Left) => (0, AnimId::WallRunChangeLF, (1.0, 1.0)),
            _ => {
                s.queue.do_chain();
                return false;
            }
        };
        self.class = class;
        s.root_scale = scale;
        s.anim.play_with(clip, PlayArgs::mode(2, -1.0));
        s.cmc.stop_movement();
        self.sub = 4;
        true
    }

    /// `PROBE()` (0x1413D965C). Returns false when the state was left.
    fn probe(&mut self, cx: &mut Ctx, class_at_start: u8) -> bool {
        self.valid = false;
        let Some(point) = probes::wall_point(cx.pawn, cx.world, self.normal, class_at_start) else { return true };
        let dp = wrap_degrees(pitch_of(self.normal) - pitch_of(point.normal));
        let dy = wrap_degrees(yaw_of(self.normal) - yaw_of(point.normal));
        if dp.abs() > A.wallrun_track_angle_max || dy.abs() > A.wallrun_track_angle_max {
            return true;
        }
        (self.component, self.normal, self.valid) = (point.component, point.normal, true);
        if self.guard_latch && wall_kick::try_wall_kick_latched(cx, self.normal) {
            return false;
        }
        WallRun::face(cx.pawn, self.class, point.normal);
        if point.end {
            cx.pawn.queue.do_chain();
            return false;
        }
        true
    }
}

/// The ceiling probe (0x140634980): a sphere of radius 5 from the origin straight up `height`: a
/// surface whose normal is 45 degrees or more off horizontal.
fn ceiling(s: &Pawn, w: &dyn crate::world::World, height: f32) -> bool {
    let l = s.cmc.location;
    for hit in w.sweep_capsule_all(l, l + Vec3::Z * height, 5.0, 5.0) {
        if hit.start_penetrating {
            return false;
        }
        if pitch_of(hit.impact_normal).abs() >= 45.0 {
            return true;
        }
    }
    false
}

/// The gate (slot 180 0x14142FF80) and `TryWallRun(hit, bProbe)` (slot 181 0x141442550) with
/// its push (slot 182): nothing queued, `WallValid`, with `bProbe` also `WallPoint`. The queue is
/// not cleared: the calling state finishes.
pub(crate) fn try_wall_run(cx: &mut Ctx, hit: &Hit, probe: bool) -> bool {
    let (w, s) = (cx.world, &mut *cx.pawn);
    if s.queue.queued_id().is_some() {
        return false;
    }
    let Some(class) = probes::wall_valid(s, w, hit.impact_normal, hit.component) else { return false };
    if probe && probes::wall_point(s, w, hit.impact_normal, class).is_none() {
        return false;
    }
    s.queue.push_state(WallRun { component: hit.component, normal: hit.impact_normal, valid: true, class, ..WallRun::RESET });
    true
}

/// `MoveInput` case 0x1414408ED: any active stick: flags |= 2, +0xAC = stick yaw.
pub(crate) fn move_input(state: &mut StateData, yaw: f32) {
    if let StateData::WallRun(r) = state {
        (r.stick, r.stick_yaw) = (true, yaw);
    }
}

/// The jump gate 0x14142A6F8 (the loop) and `JumpRequest` 0x14143DE2E: SUPERJUMP (18), else
/// refused; `ClearQueue`; `RequestJump(actor yaw, type 2, bAir)` with no launch direction.
pub(super) fn jump_request(r: &WallRun, s: &mut Pawn) {
    if r.sub == 1 && actions::has(s, Ability::Superjump) {
        let yaw = s.yaw;
        s.queue.clear();
        jump::push_super_jump(s, yaw, Vec3::ZERO);
    }
}

impl PlayerState for WallRun {
    const ID: StateId = StateId::Wallrun;

    fn sub_step(&self) -> u8 {
        self.sub
    }

    /// Begin 0x1413CCD10.
    fn begin(&mut self, cx: &mut Ctx) {
        actions::sheathe(cx);
        let s = &mut *cx.pawn;
        self.air_entry = !on_ground(s);
        // Availability bits 0, 3, 4; ResetMaxWalkSpeed; skip CalcVelocity (the flying move here
        // never calls it).
        (s.actions.second_jump, s.actions.flow_jump, s.actions.superslide_ok) = (true, true, true);
        s.cmc.max_walk_speed = s.params().speed_run;
        s.actions.traversal.begun[0] += 1;
        s.event(EventKind::Traversal { kind: "wallrun", id: self.component, on: true });
    }

    /// Tick 0x1413D87E0.
    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        let class_at_start = self.class;
        match self.sub {
            0 => {
                let s = &mut *cx.pawn;
                WallRun::face(s, self.class, self.normal);
                s.cmc.stop_movement();
                s.cmc.set_flying();
                s.cmc.gravity_off = true;
                s.root_scale = (1.0, 1.0);
                if self.air_entry {
                    // The loop clip once (mode 2) with m_bEnableChange set (0x1413D9CF5).
                    let clip = [AnimId::WallRunLoopF, AnimId::WallRunLoopR, AnimId::WallRunLoopL][self.class as usize];
                    s.anim.play_with(clip, PlayArgs::mode(2, -1.0));
                    s.queue.flags.enable_change = true;
                } else {
                    let clip = [AnimId::WallRunF, AnimId::WallRunR, AnimId::WallRunL][self.class as usize];
                    s.anim.play_with(clip, PlayArgs::mode(2, -1.0));
                }
                self.sub = 6;
                return;
            }
            6 => {
                let s = &mut *cx.pawn;
                let mut ledge = false;
                if s.queue.flags.enable_change {
                    if self.stick {
                        self.sub = 1;
                        self.switch = 0.0;
                        self.play_loop(s, 0.0);
                    } else if self.class == 0 {
                        // Let the clip finish (it drops him back to the foot of the wall).
                        self.sub = 7;
                    } else {
                        s.queue.do_chain();
                        return;
                    }
                } else {
                    if !self.valid {
                        s.queue.do_chain();
                        return;
                    }
                    ledge = self.class == 0;
                }
                if s.anim.ended {
                    s.queue.do_chain();
                    return;
                }
                if ledge && self.ledge(cx) {
                    return;
                }
            }
            1 => {
                let s = &mut *cx.pawn;
                if !self.valid || !(self.grace > 0.0) {
                    s.queue.do_chain();
                    return;
                }
                if self.stick {
                    // The blocked test: actual speed under 30 % of the root-motion speed for 0.2 s.
                    let actual = s.cmc.displacement_velocity;
                    let moved = if self.class == 0 { actual.z } else { actual.truncate().length() };
                    if A.wallrun_loop_speed * A.wallrun_blocked_ratio > moved {
                        self.blocked += dt;
                        if self.blocked > A.wallrun_blocked_time {
                            s.queue.do_chain();
                            return;
                        }
                    } else {
                        self.blocked = 0.0;
                    }
                    let d = wrap_degrees(self.stick_yaw - s.yaw);
                    let cmd = if d.abs() >= A.wallrun_down_switch_angle {
                        Cmd::Back
                    } else if d >= A.wallrun_side_switch_angle {
                        Cmd::Right
                    } else if d > -A.wallrun_side_switch_angle {
                        Cmd::None
                    } else {
                        Cmd::Left
                    };
                    let mut changed = false;
                    if cmd == Cmd::None {
                        self.switch = 0.0;
                    } else {
                        self.switch += dt;
                        if self.switch >= A.wallrun_switch_hold_time {
                            if !self.change(s, cmd) {
                                return;
                            }
                            changed = true;
                        }
                    }
                    if !changed && self.class == 0 {
                        // The lateral drift of the up-run: SetVelocityXY(v x stick direction).
                        let mut v = A.wallrun_side_move_speed;
                        if self.blocked > 0.0 {
                            v *= A.wallrun_side_move_blocked_scale;
                        }
                        let drift = dir(self.stick_yaw);
                        s.cmc.set_velocity_xy(Vec2::new(drift.x, drift.y) * v.max(0.1));
                        if let Some((low, high)) = s.anim.axis_range() {
                            s.anim.set_axis(d.clamp(low, high));
                        }
                    }
                    if class_at_start == 0 && self.ledge(cx) {
                        return;
                    }
                }
                if !self.probe(cx, class_at_start) {
                    return;
                }
            }
            4 => {
                let (w, s) = (cx.world, &mut *cx.pawn);
                s.cmc.stop_movement();
                if self.class == 0 && s.anim.time > 0.3 && !self.ceiling && ceiling(s, w, A.wallrun_ceiling_height) {
                    self.ceiling = true;
                    s.cmc.gravity_scale = s.params().gravity_scale;
                    s.cmc.set_velocity_z(0.0);
                    s.queue.do_chain();
                    return;
                }
                if s.anim.ended {
                    self.sub = 1;
                    (self.switch, self.blocked) = (0.0, 0.0);
                    self.play_loop(s, -1.0);
                    // (The first loop tick probes with the wall as it was: the valid bit stays.)
                    WallRun::face(s, self.class, self.normal);
                }
            }
            _ => {
                if cx.pawn.anim.ended {
                    cx.pawn.queue.do_chain();
                }
            }
        }
        // Tail (0x1413D9DAD).
        self.grace = if self.stick { A.wallrun_input_grace } else { (self.grace - dt).max(0.0) };
        self.stick = false;
        self.guard_latch = false;
    }

    /// End 0x1413CF500.
    fn end(&mut self, cx: &mut Ctx) {
        let next = cx.pawn.queue.queued_id();
        let s = &mut *cx.pawn;
        s.root_scale = (1.0, 1.0);
        s.cmc.direct_xy = None;
        s.cmc.direct_z = None;
        s.cmc.max_walk_speed = s.params().speed_run;
        if !on_ground(s) {
            // The falling move gets half the actual velocity of the last tick (cmc+0x9A0).
            let v = s.cmc.displacement_velocity;
            s.cmc.set_falling();
            s.cmc.velocity = v * A.wallrun_end_velocity_scale;
        }
        s.cmc.gravity_off = false;
        s.event(EventKind::Traversal { kind: "wallrun", id: self.component, on: false });
        // Next none or JUMP: the weapon only in battle; DANGLE / HOPPING: nothing. (`vt+0x1360()`
        // is false for the plain Keyblade: nothing is drawn either way.)
        let _ = next;
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}

/// (integration pass) +0xA8 for the host's camera (`Sora::camera_state`): 0 up, 1 / 2 along.
pub(crate) fn class_of(w: &WallRun) -> u8 {
    w.class
}
