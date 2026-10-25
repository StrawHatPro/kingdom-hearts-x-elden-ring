//! `TresPlayerState_PoleTurn` (id 24) and its request `TryPoleTurn`: docs/sora-actions-spec.md
//! II.4 "Pole Turn" (instruction level except the grab-pose algebra of sub-step 0, which is
//! decompiled C there and SIMPLIFIED here). OWNER: the actions implementer.
//!
//! A pole of the host's registry (`World::poles`) is caught when the point 120 cm ahead of and
//! 90 cm above his origin passes within 100 + `m_CheckRadius` of its segment while he is in an
//! Air Slide or a Superslide (their ticks and their world hits; the airstep arrival calls
//! `try_pole_turn` too). A vertical pole (POLE_SPIN): `PoleTurn` 0.1667 s, then `PoleTurnLoop`
//! (360 degrees per 0.3 s) for 4 loops = 1.2 s, then a Superslide starts by itself. A horizontal
//! bar (POLE_SWING): `PoleSwing`, `PoleSwingLoop` (0.4 s) for 1.6 s, then a directed super jump
//! by itself. Jump button: a directed super jump at once (stick yaw if the stick is active, else
//! the grab yaw). Guard button: a Superslide along the stick or the facing. The pole just left is
//! not caught again by the Superslide it launched.
//!
//! He is attached to the pole: movement mode Custom here (the game: Flying with zero velocity,
//! attached to the component). SIMPLIFIED: the grab pose. Vertical: his origin hangs 100 cm
//! below the nearest point of the axis, `m_AttachRadius` to the side (the game offsets the MESH
//! by it and spins the actor on the axis; here the actor itself circles the axis, which draws
//! the same). Horizontal: his origin at the middle of the bar, facing across it.

use glam::Vec3;

use super::{jump, super_slide, PlayerState, StateData, StateId};
use crate::actions::probes::{self, dir, yaw_of};
use crate::actions::{self, A};
use crate::anim::{AnimId, PlayArgs};
use crate::enums::ETresAbilityKind as Ability;
use crate::host::EventKind;
use crate::pawn::{Ctx, Pawn};
use crate::sora::{wrap_degrees, MovementState};
use crate::world::Pole;

/// The fields of II.4's list.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PoleTurn {
    pub(super) sub: u8,
    /// +0x90 (the registry id), +0xB0 bit 0, +0xB4.
    pole: u32,
    horizontal: bool,
    attach_radius: f32,
    /// The point of the axis he is attached to and the bar's direction (horizontal).
    anchor: Vec3,
    /// +0xC4 loop count, +0xC8 loop length, +0xCC approach timer, +0xD0 remaining time.
    loops: f32,
    approach: f32,
    time_left: f32,
    /// +0xD4 / +0xD8 stick flag / yaw; +0xDC latches (bit 0 jump, bit 1 guard); +0xE0 Superslide
    /// yaw; +0xE8 release yaw; +0xF0 bit 0 rotate on release.
    stick: bool,
    stick_yaw: f32,
    pub(super) jump_latch: bool,
    guard_latch: bool,
    slide_yaw: f32,
    release_yaw: f32,
    rotate_on_release: bool,
}

impl PoleTurn {
    /// (combat implementer) +0xB0 bit 0 and the release yaw, for the attack request (slot 45
    /// 0x141438611: a bar swing forces the Somersault, kind 0x39, and turns him to that yaw).
    pub(super) fn swing(&self) -> Option<f32> {
        self.horizontal.then_some(self.release_yaw)
    }

    /// Reset (0x14150DFE0).
    pub const RESET: PoleTurn = PoleTurn { sub: 0, pole: 0, horizontal: false, attach_radius: 0.0, anchor: Vec3::ZERO, loops: 3.0, approach: 0.0, time_left: 0.0, stick: false, stick_yaw: 0.0, jump_latch: false, guard_latch: false, slide_yaw: 0.0, release_yaw: 0.0, rotate_on_release: false };

    /// Where his origin is on the pole for the facing `yaw`.
    fn grab(&self, yaw: f32) -> Vec3 {
        if self.horizontal {
            self.anchor
        } else {
            // 100 cm below the grab point, `m_AttachRadius` to his left of the axis.
            let a = yaw.to_radians();
            self.anchor + Vec3::Z * A.pole_spin_hang_offset - Vec3::new(-a.sin(), a.cos(), 0.0) * self.attach_radius
        }
    }

    /// The directed super jump of the jump latch / the bar's time-out (slot 171 0x1414459A0).
    fn launch_jump(&mut self, s: &mut Pawn) {
        self.rotate_on_release = true;
        if self.stick {
            self.release_yaw = self.stick_yaw;
        }
        jump::push_super_jump(s, self.release_yaw, dir(self.release_yaw));
    }
}

/// `TryPoleTurn(out, bKeep)` (slot 78 0x14143F0C0) behind its gate (slot 77 0x14142C640): the
/// ability by the pole's type (POLE_SWING 96 for a bar, POLE_SPIN 95 for a pole), nothing
/// queued, the FreeFlow bit, the state one of AIR_SLIDE, SUPER_SLIDE, ATHLETICFLOW;
/// `ClearQueue`; the push (slot 80 0x141445270). THE ENTRY CALL for the airstep arrival too.
pub(crate) fn try_pole_turn(state: &StateData, cx: &mut Ctx, pole: &Pole) -> bool {
    let s = &mut *cx.pawn;
    let ability = if pole.horizontal { Ability::PoleSwing } else { Ability::PoleSpin };
    if !actions::has(s, ability) || s.queue.queued_id().is_some() || !s.actions.flow_unlocked {
        return false;
    }
    if !matches!(state, StateData::AirSlide(_) | StateData::SuperSlide(_) | StateData::AthleticFlow(_)) {
        return false;
    }
    // `AttachOffset(L)`: the nearest point of the segment (`m_bEnableNearDetectOffset`), or the
    // component's own attach point (a bar: the middle of its segment).
    let anchor = if pole.horizontal { (pole.base1 + pole.base2) * 0.5 } else { probes::closest_on_segment(s.cmc.location - Vec3::Z * A.pole_spin_hang_offset, pole.base1, pole.base2) };
    let release_yaw = if pole.horizontal {
        // Across the bar, on the side he comes from.
        let across = yaw_of(pole.base2 - pole.base1) + 90.0;
        if dir(across).dot(dir(s.yaw)) >= 0.0 { across } else { across + 180.0 }
    } else {
        s.yaw
    };
    s.queue.clear();
    s.queue.push_state(PoleTurn { pole: pole.id, horizontal: pole.horizontal, attach_radius: pole.attach_radius, anchor, release_yaw: wrap_degrees(release_yaw), ..PoleTurn::RESET });
    true
}

/// The pole probe of the Air Slide / Superslide ticks and world hits (II.4 "The pole component
/// and its probe": point = L + rotation x (120, 0, 90), range 100) followed by `TryPoleTurn`.
pub(crate) fn probe_and_try(state: &StateData, cx: &mut Ctx) -> bool {
    let point = probes::pole_probe_point(cx.pawn);
    let Some(pole) = probes::pole_probe(cx.pawn, cx.world, point, A.pole_probe_range) else { return false };
    try_pole_turn(state, cx, &pole)
}

/// `MoveInput` case 0x141440D7B: +0xD8 = yaw, +0xD4 = 1.
pub(crate) fn move_input(state: &mut StateData, yaw: f32) {
    if let StateData::PoleTurn(p) = state {
        (p.stick_yaw, p.stick) = (yaw, true);
    }
}

/// `TryFlow` case 0x1414418F9 -> slot 114 (the guard latch): if not yet latched: +0xDC |= 2,
/// +0xE0 = the stick yaw when the stick is above 0.5, else the actor yaw.
pub(super) fn flow_latch(p: &mut PoleTurn, s: &Pawn) {
    if s.stick_magnitude > A.pole_flow_stick_min {
        (p.stick, p.stick_yaw) = (true, s.stick_yaw);
    }
    if !p.guard_latch {
        p.guard_latch = true;
        p.slide_yaw = if p.stick { p.stick_yaw } else { s.yaw };
    }
}

impl PlayerState for PoleTurn {
    const ID: StateId = StateId::PoleTurn;

    fn sub_step(&self) -> u8 {
        self.sub
    }

    /// Begin (parent 0x141509AA0, player 0x1413A41F0).
    fn begin(&mut self, cx: &mut Ctx) {
        actions::draw(cx, true);
        // (integration pass) PoleTurn Begin 0x1413A41F0: the attack camera off, then mode 5 (the
        // normal camera's distance scale 1.6; combat/camera.rs, combat spec 7.12.1).
        crate::combat::camera::off(cx);
        crate::combat::camera::request(cx, true, 5, crate::host::TargetId::NONE);
        let s = &mut *cx.pawn;
        (s.actions.second_jump, s.actions.flow_jump, s.actions.superslide_ok) = (true, true, true);
        // pawn+0x660 = the component: it cannot be caught again while it is remembered.
        s.actions.traversal.pole_exclude = Some((self.pole, f32::MAX));
        // +0xC4 = MyPoleTurnRotMax (4).
        self.loops = A.pole_rot_max;
        s.actions.traversal.begun[1] += 1;
        s.event(EventKind::Traversal { kind: "pole", id: self.pole, on: true });
    }

    /// Tick 0x1415102E0.
    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        let valid = cx.world.poles().iter().any(|p| p.id == self.pole && p.active);
        let s = &mut *cx.pawn;
        match self.sub {
            0 => {
                if self.horizontal {
                    s.yaw = self.release_yaw;
                }
                s.cmc.stop_movement();
                s.cmc.set_custom();
                s.cmc.gravity_off = true;
                s.anim.play_with(if self.horizontal { AnimId::PoleSwing } else { AnimId::PoleTurn }, PlayArgs::mode(2, -1.0));
                self.approach = A.pole_approach_time;
                self.sub = 1;
            }
            1 => {
                let target = self.grab(s.yaw);
                if s.anim.ended {
                    s.cmc.location = target;
                    s.cmc.stop_movement();
                    let clip = if self.horizontal { AnimId::PoleSwingLoop } else { AnimId::PoleTurnLoop };
                    s.anim.play_with(clip, PlayArgs::mode(3, -1.0));
                    self.time_left = self.loops * clip.data().length;
                    self.sub = 2;
                } else {
                    // The relative location approaches the target: weight 2 x timer, falling.
                    self.approach -= dt;
                    let a = (2.0 * self.approach).clamp(0.0, 1.0);
                    s.cmc.location = target + (s.cmc.location - target) * a;
                }
            }
            _ => {
                // He circles the axis with the loop's root yaw (40 degrees per frame).
                s.cmc.location = self.grab(s.yaw);
                // Slot 116: release = the time is up, the pole is gone (latches cleared), or a latch.
                if !valid {
                    (self.jump_latch, self.guard_latch) = (false, false);
                }
                let release = !(self.time_left > 0.0) || !valid || self.jump_latch || self.guard_latch;
                if !release {
                    self.time_left -= dt;
                } else {
                    if s.queue.nothing_queued() {
                        if self.time_left > 0.0 {
                            if self.jump_latch {
                                self.launch_jump(s);
                            } else if self.guard_latch {
                                // Push SuperSlide(kind = bar ? 1 : 2, yaw +0xE0).
                                super_slide::push(s, Some(self.slide_yaw));
                            }
                        } else {
                            self.rotate_on_release = true;
                            if self.horizontal {
                                self.launch_jump(s);
                            } else {
                                // A vertical pole: the Superslide starts by itself (kind 2, no yaw).
                                super_slide::push(s, None);
                            }
                        }
                    }
                    s.queue.do_finish();
                }
            }
        }
        self.stick = false;
    }

    /// End (player 0x1413AA820, parent 0x14150BF30).
    fn end(&mut self, cx: &mut Ctx) {
        // (integration pass) End 0x1413AA820: the attack camera off.
        crate::combat::camera::off(cx);
        let s = &mut *cx.pawn;
        // Unless the next state is SUPER_SLIDE: pawn+0x660 = 0. (After a Superslide the port
        // forgets the pole 1.5 s later: the game's clearing point after it was not read.)
        s.actions.traversal.pole_exclude = if s.queue.queued_id() == Some(StateId::SuperSlide) { Some((self.pole, 1.5)) } else { None };
        s.cmc.stop_movement();
        if self.rotate_on_release {
            s.yaw = wrap_degrees(self.release_yaw);
        }
        s.cmc.set_falling();
        s.cmc.gravity_off = false;
        s.event(EventKind::Traversal { kind: "pole", id: self.pole, on: false });
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}
