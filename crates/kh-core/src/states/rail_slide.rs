//! `TresPlayerState_RailSlide` (id 22) and its request `TryRailSlide`: docs/sora-actions-spec.md
//! II.5 "Rail Slide" R1 - R8 (dispatch at instruction level, several bodies decompiled C there).
//! OWNER: the actions implementer.
//!
//! A rail is a registry entry, not collision: the host's level rails (`World::rails`) and the
//! Blizzard spell's ice rails (`PawnMagic::rails`, the magic side's nodes). Idle, Run (range 50)
//! and Jump (range 150) probe for one every tick (`actions::probes::rail_probe`); on a hit he
//! snaps onto it, `RailSlideStart` plays for 0.2333 s with the slide paused, then he moves along
//! it at 2000 cm/s (or the nodes' speeds), ignoring the level, the facing following the rail.
//! Stick against the travel direction (more than 120 degrees off): `RailSlideTurn` (0.3 s) and
//! he slides back; jump button: a super jump along the rail (175 cm/s of inertia); guard button:
//! a Superslide; the end of the rail launches the same super jump by itself.
//!
//! The game moves him in the movement tick (`PhysRailSlide`, component slot 408, while the
//! state only labels the mode Flying); here the state moves him in its own tick, one phase
//! later in the frame, in movement mode Custom. Not here: the on-rail jump of `RELATIVE` rails,
//! the transfer rails (state 138), slope acceleration, the rail camera, the HUD prompts, the
//! obstacle sweep of ice rails, and pitch / roll of the actor (he keeps the rail's yaw only).

use glam::Vec3;

use super::common::turn;
use super::{jump, super_slide, PlayerState, StateData, StateId};
use crate::actions::probes::{self, dir, yaw_of, RailHit, RailSource};
use crate::actions::{self, A};
use crate::anim::{AnimId, PlayArgs};
use crate::enums::ETresAbilityKind as Ability;
use crate::host::EventKind;
use crate::pawn::{Ctx, Pawn};
use crate::sora::{wrap_degrees, MovementState};

/// The fields of II.5 R3 (state) and of `FTresRailSlideWork` (movement side) this port keeps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RailSlide {
    /// 0 attach, 1 a one-shot clip (start or turn), 2 the slide.
    pub(super) sub: u8,
    source: RailSource,
    id: u32,
    /// +0xA0 attach point, +0xAC its yaw, +0xC8 bit 0 reversed.
    attach: Vec3,
    yaw: f32,
    reversed: bool,
    /// +0x88 / +0x8C: stick this frame, stick yaw.
    stick: bool,
    stick_yaw: f32,
    /// Flags: 4 waiting for a one-shot clip, 8 turn-around armed, 0x20 narrow, 0x40 no reverse,
    /// 0x100 no jump, 0x4000 the stick must be released before the next turn (set by Reset).
    waiting: bool,
    turn_armed: bool,
    pub(super) narrow: bool,
    no_reverse: bool,
    pub(super) no_jump: bool,
    must_release: bool,
    /// +0xCC: the turn-around lock-out timer.
    turn_lock: f32,
    /// work+0x1C bit 0 paused; work+0x24 bit 2 end reached / detached; work+0x64 current speed.
    paused: bool,
    ended: bool,
    speed: f32,
}

impl RailSlide {
    /// Reset (0x1413AD0B0 -> 0x14150E0C0).
    pub const RESET: RailSlide = RailSlide { sub: 0, source: RailSource::Level, id: 0, attach: Vec3::ZERO, yaw: 0.0, reversed: false, stick: false, stick_yaw: 0.0, waiting: false, turn_armed: false, narrow: false, no_reverse: false, no_jump: false, must_release: true, turn_lock: 0.0, paused: false, ended: false, speed: 0.0 };

    /// `StartLoop(name, blendIn)` (slot 130 0x141514AD0).
    fn start_loop(&mut self, s: &mut Pawn, blend_in: f32) {
        s.cmc.stop_movement();
        let clip = if self.narrow { AnimId::RailSlideNarrowLoop } else { AnimId::RailSlideLoop };
        s.anim.play_with(clip, PlayArgs::mode(3, blend_in));
        self.paused = false;
        if !self.must_release {
            self.stick = false;
        }
        self.turn_lock = A.rail_turn_lockout;
        self.turn_armed = false;
    }
}

/// The gate (slot 74 0x14142C800) and `TryRailSlide(out, bKeep = 1, bSkipStart = 0)` (slot 75
/// 0x14143F4E0) with its push (slot 76): the flowmotion unlock bit; the state one of IDLE, RUN,
/// JUMP. The queue is kept: the caller chains.
pub(crate) fn try_rail_slide(state: &StateData, s: &mut Pawn, hit: &RailHit) -> bool {
    if !s.actions.flow_unlocked || !matches!(state, StateData::Idle(_) | StateData::Run(_) | StateData::Jump(_)) {
        return false;
    }
    s.queue.push_state(RailSlide { source: hit.source, id: hit.id, attach: hit.position, yaw: hit.yaw, reversed: hit.reversed, ..RailSlide::RESET });
    true
}

/// The probe of the Idle / Run / Jump ticks followed by the request: `range` 50 on the ground,
/// 150 in the air; `want` = the stick's direction when it is active, else the facing.
pub(crate) fn probe_and_try(state: &StateData, cx: &mut Ctx, range: f32, want_yaw: f32) -> bool {
    // (Nothing to find: no probe arithmetic at all, so that worlds without rails are untouched.)
    if cx.world.rails().is_empty() && cx.pawn.magic.rails.is_empty() {
        return false;
    }
    let Some(hit) = probes::rail_probe(cx, range, dir(want_yaw)) else { return false };
    try_rail_slide(state, cx.pawn, &hit)
}

/// `MoveInput` (slot 35), RAIL_SLIDE: +0x88 = 1, +0x8C = stick yaw.
pub(crate) fn move_input(state: &mut StateData, yaw: f32) {
    if let StateData::RailSlide(r) = state {
        (r.stick, r.stick_yaw) = (true, yaw);
    }
}

/// The jump gate (state slot 113 0x141507BC0: sub 2, not narrow, jump allowed) and `JumpRequest`:
/// `ClearQueue`; `RequestJump(yaw, type 2, bAir)` with Jump+0x100 = the forward direction in XY,
/// +0x10C = 175, +0x110 = 0.92 (II.5 R6).
pub(super) fn jump_request(r: &RailSlide, s: &mut Pawn) {
    if r.sub == 2 && !r.narrow && !r.no_jump {
        let yaw = s.yaw;
        s.queue.clear();
        jump::push_super_jump(s, yaw, dir(yaw));
    }
}

/// `TryFlow` for state 22 (gate: SUPERSLIDE, state slot 116: sub 2 or 3, not narrow):
/// `ClearQueue`; push a plain Superslide.
pub(super) fn flow(r: &RailSlide, s: &mut Pawn) -> bool {
    if !matches!(r.sub, 2 | 3) || r.narrow || !actions::has(s, Ability::Superslide) {
        return false;
    }
    s.queue.clear();
    s.queue.push_state(super_slide::SuperSlide::RESET);
    true
}

impl PlayerState for RailSlide {
    const ID: StateId = StateId::RailSlide;

    fn sub_step(&self) -> u8 {
        self.sub
    }

    /// Begin 0x1413A4650 -> 0x141509CA0.
    fn begin(&mut self, cx: &mut Ctx) {
        // (State slot 146: the weapon is put away for the slide unless a form keeps it: a no-op
        // for the plain Keyblade's Sheathe.)
        actions::sheathe(cx);
        let s = &mut *cx.pawn;
        (s.actions.second_jump, s.actions.flow_jump, s.actions.superslide_ok) = (true, true, true);
        s.cmc.stop_movement();
        s.cmc.set_custom();
        s.cmc.gravity_off = true;
        s.actions.traversal.begun[2] += 1;
        s.event(EventKind::Traversal { kind: "rail", id: self.id, on: true });
    }

    /// Tick 0x141510EE0 with the movement side's `PhysRailSlide` (0x14058ECF0).
    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        let path = probes::rail_path(cx, self.source, self.id);
        let s = &mut *cx.pawn;
        let Some(path) = path else {
            // The rail is gone (an ice rail melted): detached.
            if self.sub == 0 {
                s.queue.do_chain();
            } else {
                leave(s, false);
            }
            return;
        };
        (self.narrow, self.no_reverse, self.no_jump) = (path.narrow, path.no_reverse, path.no_jump);
        if self.sub == 0 {
            // The facing snaps to the rail; attach: the base speed; pause; the start clip.
            s.yaw = wrap_degrees(self.yaw);
            s.cmc.location = self.attach;
            self.speed = A.rail_speed;
            self.paused = true;
            s.cmc.stop_movement();
            s.anim.play_with(AnimId::RailSlideStart, PlayArgs::mode(0, -1.0));
            self.waiting = true;
            self.sub = 1;
            return;
        }
        if self.sub == 1 {
            if self.waiting && s.anim.ended {
                self.waiting = false;
            }
            if self.waiting {
                return;
            }
            self.start_loop(s, -1.0);
            self.sub = 2;
        }
        // ---- the slide: where he is on the rail (the ice rail's first nodes melt behind him:
        // re-projected every tick), the step along it.
        let (mut along, _) = path.closest(s.cmc.location, s.cmc.location);
        let length = path.length();
        self.speed = path.speed_at(along, A.rail_speed);
        if !self.paused {
            along += if self.reversed { -1.0 } else { 1.0 } * self.speed * dt;
        }
        if along >= length || along <= 0.0 {
            self.ended = true;
            along = along.clamp(0.0, length);
        }
        let (position, forward, _) = path.at(along);
        let travel = if self.reversed { -forward } else { forward };
        s.cmc.location = position;
        // rot = Slerp(current rotation, rail rotation, 0.5) per tick.
        turn(s, yaw_of(travel), A.rail_rotation_slerp);
        if self.ended {
            leave(s, true);
            return;
        }
        // The lean (slot 132): the signed angle to the direction 200 cm further along.
        let ahead = if self.reversed { along - A.rail_lean_lookahead } else { along + A.rail_lean_lookahead };
        let (_, forward_ahead, _) = path.at(ahead.clamp(0.0, length));
        let lean = wrap_degrees(yaw_of(if self.reversed { -forward_ahead } else { forward_ahead }) - yaw_of(travel));
        if let Some((low, high)) = s.anim.axis_range() {
            s.anim.set_axis(lean.clamp(low, high));
        }
        // The turn-around (0x141511802 - 0x1415119FD).
        if self.turn_armed && !self.narrow && !self.no_reverse {
            if self.stick && !self.must_release && dir(s.yaw).dot(dir(self.stick_yaw)) < A.rail_turn_dot {
                self.reversed = !self.reversed;
                s.yaw = wrap_degrees(yaw_of(-travel));
                // Slot 131: pause; StopMovement; Play("RailSlideTurn", -1, mode 2).
                self.paused = true;
                s.cmc.stop_movement();
                s.anim.play_with(AnimId::RailSlideTurn, PlayArgs::mode(2, -1.0));
                (self.waiting, self.must_release) = (true, true);
                self.sub = 1;
            }
        } else if self.turn_lock > 0.0 {
            self.turn_lock -= dt;
            if self.turn_lock <= 0.0 {
                self.turn_lock = 0.0;
                self.turn_armed = true;
            }
        }
        if !self.stick {
            self.must_release = false;
        }
        self.stick = false;
    }

    /// End 0x1413AAF40 -> 0x14150C1F0: detach (the last rail is kept: the probe refuses it until
    /// he is 200 cm away); leave Flying.
    fn end(&mut self, cx: &mut Ctx) {
        let s = &mut *cx.pawn;
        s.actions.traversal.rail_exclude = Some((self.id, f32::MAX));
        s.cmc.gravity_scale = s.params().gravity_scale;
        s.cmc.gravity_off = false;
        if s.cmc.is_custom() {
            s.cmc.set_falling();
        }
        s.event(EventKind::Traversal { kind: "rail", id: self.id, on: false });
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}

/// The LEAVE branch (0x141511AE6): with nothing queued, the automatic launch (slot 145
/// 0x141513460): `RequestJump(actor yaw, type 2, bAir)` with the forward direction, 175 and
/// 0.92; then `DoFinish`.
fn leave(s: &mut Pawn, launch: bool) {
    if launch && s.queue.nothing_queued() {
        let yaw = s.yaw;
        jump::push_super_jump(s, yaw, dir(yaw));
    }
    s.queue.do_finish();
}
