//! `TresCharState_Dangle` (the ledge hang, id 6) and its request `TryDangle`:
//! docs/sora-actions-spec.md II.6 "Dangle" (instruction level; the JumpRequest branch is
//! decompiled C there). OWNER: the actions implementer.
//!
//! While he rises slowly or falls in a Jump, a ledge ahead (the walkable top of a component
//! flagged `SQEX_bEnableDangle`, 100 - 250 cm above his feet: `actions::probes::ledge_probe`) is
//! grabbed: his feet are put 150 cm below the top and 60 cm back from the edge, facing the wall,
//! in movement mode Custom (no movement at all); `GripStart`, then `GripLoop`. From 0.199 s the
//! stick decides: toward the wall (less than 45 degrees off: Sora is pad-driven; 90 for an AI-input
//! pawn) climbs up (`GripRise`: 100 cm forward and 200 up by root motion in Flying mode), away from
//! it (135 degrees or more; the AI rule: 90) lets go, in between he stays; the jump button
//! drops him. After letting go the ledge can be grabbed again 0.6 s later.
//!
//! (party I1) A movable ledge (the base attachment, +0xA4, and its support test FUN_14064A090) is ported; not
//! here: the wall-run variant's pitch levelling.

use glam::Vec3;

use super::{jump, PlayerState, StateData, StateId};
use crate::actions::probes::{self, rotate, Ledge};
use crate::actions::{self, A};
use crate::anim::{AnimId, PlayArgs};
use crate::host::EventKind;
use crate::pawn::{Ctx, Pawn};
use crate::sora::{wrap_degrees, MovementState};

/// The fields of II.6's table.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Dangle {
    /// 0 `GripStart`, 1 `GripLoop`, 2 the climb-up, 3 failed (chains out at once).
    pub(super) sub: u8,
    /// +0x90, +0x98, +0xB0.
    component: u32,
    position: Vec3,
    yaw: f32,
    /// +0xB4 / +0xB8: stick this frame (magnitude > 0.5), stick yaw.
    stick: bool,
    stick_yaw: f32,
    /// +0xBC: 1 = pushed from WALLRUN (straight to the climb-up).
    from_wall_run: bool,
    /// (party I1) +0xA4 with +0xBD bit 0: the hang point in the component's space (push 0x141444810), and the
    /// based movement on a Movable ledge (vt+0xA60 in Begin): his location in the base's space, the base's yaw.
    local: Option<Vec3>,
    based: Option<(Vec3, f32)>,
}

impl Dangle {
    /// Reset (0x14150DEA0).
    pub const RESET: Dangle = Dangle { sub: 0, component: 0, position: Vec3::ZERO, yaw: 0.0, stick: false, stick_yaw: 0.0, from_wall_run: false, local: None, based: None };
}

/// The gate (iface slot 63 0x141427830): the dangle cool-down pawn+0x6F8, nothing queued, the
/// state IDLE, WALLRUN, or JUMP with `Velocity.Z` <= 500 in sub-steps 1 / 2, <= 0 otherwise.
fn gate(state: &StateData, s: &Pawn) -> bool {
    if s.actions.traversal.dangle_cooldown > 0.0 || s.queue.queued_id().is_some() {
        return false;
    }
    match state {
        StateData::Idle(_) | StateData::WallRun(_) => true,
        StateData::Jump(j) => s.cmc.velocity.z <= if matches!(j.sub, 1 | 2) { A.dangle_gate_velz_rising } else { 0.0 },
        _ => false,
    }
}

/// The Jump tick's tail (0x141397FE6 - 0x1413980C6): the gate, the ledge probe with (up 250,
/// down 50), the push (slot 65). On success the caller chains.
pub(crate) fn try_from_jump(state: &StateData, cx: &mut Ctx) -> bool {
    if !gate(state, cx.pawn) {
        return false;
    }
    let Some(ledge) = probes::ledge_probe(cx.pawn, cx.world, A.dangle_probe_up, A.dangle_probe_down) else { return false };
    // (party I1) +0xA4: the hang point in the component's space while it lives (+0xBD bit 0)
    let local = cx.world.component_motion(ledge.component).map(|m| m.to_local(ledge.position));
    cx.pawn.queue.push_state(Dangle { component: ledge.component, position: ledge.position, yaw: ledge.yaw, from_wall_run: false, local, ..Dangle::RESET });
    true
}

/// `TryDangle` (slot 64) from the wall run's top edge: variant 1 (the state is WALLRUN: the gate
/// passes unless the cool-down runs or something is queued).
pub(super) fn push_from_wall_run(s: &mut Pawn, ledge: Ledge) -> bool {
    if s.actions.traversal.dangle_cooldown > 0.0 || s.queue.queued_id().is_some() {
        return false;
    }
    push(s, ledge, true);
    true
}

/// THE ENTRY CALL for other states (the airstep arrival's END branch: probe + slot 65 with
/// variant 1): pushes DANGLE for a ledge found with `actions::probes::ledge_probe`; the caller
/// chains or finishes as its own code says.
#[allow(dead_code)]
pub(crate) fn push(s: &mut Pawn, ledge: Ledge, from_wall_run: bool) {
    s.queue.push_state(Dangle { component: ledge.component, position: ledge.position, yaw: ledge.yaw, from_wall_run, ..Dangle::RESET });
}

/// (party I1) The moving-ledge support test FUN_14064A090 (FUN_1406385E0 in Begin, FUN_140638690 in the STAY tick, both
/// with 10): r = 5; the point T x (r / 2 - offset.x, -offset.y, -offset.z) (2.5 cm past the contact, on the ledge
/// top); a sphere of 10 from the point + (r + 10) Z down to it: a blocking walkable hit = supported.
fn ledge_supports(s: &Pawn, w: &dyn crate::world::World, feet: Vec3, yaw: f32) -> bool {
    let off = Vec3::new(A.dangle_offset_back, 0.0, A.dangle_offset_down);
    let r = 5.0f32;
    let point = feet + rotate(0.0, yaw, Vec3::new(r * 0.5 - off.x, -off.y, -off.z));
    let walkable_z = s.params().floor_walkable_z;
    w.sweep_capsule_all(point + Vec3::Z * (r + 10.0), point, 10.0, 10.0).into_iter().any(|h| h.impact_normal.z >= walkable_z)
}

/// `MoveInput` DANGLE branch (0x1414406D0): only above magnitude 0.5: +0xB8 = stick yaw, +0xB4 = 1.
pub(crate) fn move_input(state: &mut StateData, yaw: f32, magnitude: f32) {
    if let StateData::Dangle(d) = state {
        if magnitude > A.dangle_input_min_stick {
            (d.stick_yaw, d.stick) = (yaw, true);
        }
    }
}

/// The jump gate's DANGLE branch ((sub 0 with m_bEnableChange) or sub 1) and `JumpRequest`:
/// `RequestJump(actor yaw, type 0, bFall = 1, bAir = 1)`: he simply drops.
pub(super) fn jump_request(d: &Dangle, s: &mut Pawn) {
    if (d.sub == 0 && s.queue.flags.enable_change) || d.sub == 1 {
        let yaw = s.yaw;
        s.queue.clear();
        jump::push_fall(s, yaw);
    }
}

impl PlayerState for Dangle {
    const ID: StateId = StateId::Dangle;

    fn sub_step(&self) -> u8 {
        self.sub
    }

    /// Begin (player wrapper 0x1413685A0 -> 0x141508DE0).
    fn begin(&mut self, cx: &mut Ctx) {
        actions::sheathe(cx);
        let (w, s) = (cx.world, &mut *cx.pawn);
        let p = *s.params();
        let mut ok = true;
        // (party I1) +0xBD bit 0: +0x98 = ComponentToWorld x (+0xA4 x scale)
        let motion = w.component_motion(self.component);
        if let (Some(m), Some(local)) = (motion, self.local) {
            self.position = m.to_world(local);
        }
        // The body at the hang point: swept there from 10 cm behind; a start-penetrating hit
        // fails (variant 0), otherwise he stops short by what is left.
        let back = rotate(0.0, self.yaw, Vec3::new(-A.dangle_begin_backoff, 0.0, 0.0));
        let centre = self.position + Vec3::Z * p.capsule_center_z;
        if let Some(hit) = w.sweep_capsule(centre + back, centre, p.capsule_radius, p.capsule_half_height) {
            if hit.start_penetrating {
                ok = self.from_wall_run;
            } else {
                self.position += back * (1.0 - hit.time).clamp(0.0, 1.0);
            }
        }
        // (party I1) variant 0 on a Movable component (+0x348 == 2): the support test FUN_1406385E0(.., 10.0)
        if ok && !self.from_wall_run && motion.is_some_and(|m| m.movable) {
            ok = ledge_supports(s, w, self.position, self.yaw);
        }
        s.queue.flags.enable_change = false;
        if !ok {
            self.sub = 3;
            return;
        }
        s.cmc.location = self.position;
        s.yaw = wrap_degrees(self.yaw);
        // (party I1) the base = the ledge (vt+0xA60(component, 0, 1)): a Movable one carries him
        self.based = motion.filter(|m| m.movable).map(|m| (m.to_local(self.position), (m.rotation * Vec3::X).y.atan2((m.rotation * Vec3::X).x).to_degrees()));
        s.cmc.stop_movement();
        s.root_scale = (1.0, 1.0);
        if self.from_wall_run {
            s.cmc.set_flying();
            s.cmc.gravity_off = true;
            s.anim.play_with(AnimId::GripRise, PlayArgs::mode(2, -1.0));
            self.sub = 2;
        } else {
            // Movement mode Custom (6): its Phys function is an empty stub.
            s.cmc.set_custom();
            s.anim.play_with(AnimId::GripStart, PlayArgs::mode(2, -1.0));
            self.sub = 0;
        }
        s.actions.traversal.begun[3] += 1;
        s.event(EventKind::Traversal { kind: "dangle", id: self.component, on: true });
    }

    /// Tick 0x14150F220.
    fn tick(&mut self, cx: &mut Ctx, _dt: f32) {
        let w = cx.world;
        let s = &mut *cx.pawn;
        // (party I1) the based movement on a Movable ledge (the movement tick runs before the state's)
        if let Some((local, base_yaw)) = self.based {
            match w.component_motion(self.component) {
                Some(m) => {
                    s.cmc.location = m.to_world(local);
                    let f = m.rotation * Vec3::X;
                    let now = f.y.atan2(f.x).to_degrees();
                    s.yaw = wrap_degrees(s.yaw + wrap_degrees(now - base_yaw));
                    self.based = Some((local, now));
                }
                None => self.based = None,
            }
        }
        match self.sub {
            0 | 1 => {
                if self.sub == 0 {
                    if s.anim.ended {
                        s.anim.play_with(AnimId::GripLoop, PlayArgs::mode(3, -1.0));
                        self.sub = 1;
                    }
                    if !s.queue.flags.enable_change && self.sub == 0 {
                        self.stick = false;
                        return;
                    }
                }
                // The input block.
                let t = &mut s.actions.traversal;
                // The rule by pawn vt+0x1A20 (0x1405ADAF0: (vt+0x1A18 0x1405AA1E0 - 1) < 2): a pawn
                // with a PlayerController answers input type 1 (PAD), so Sora takes the pad branch:
                // climb under 45 (0x14150F4D8), let go from 135 (0x14150F4E1), STAY between. An
                // AI-input pawn (TRES_CHAR_INPUT_AI, the AI controller's +0x4E0) climbs under 90,
                // else lets go (0x14150F50B). AI Sora (a friend leads, combat.friend.sora_is_ai:
                // the port's design) is not pad-driven. (party I1)
                let pad = !s.combat.friend.sora_is_ai;
                let a = if self.stick { wrap_degrees(self.stick_yaw - s.yaw).abs() } else { 0.0 };
                let (climb, release) = if !self.stick {
                    (false, false)
                } else if pad {
                    (a < A.dangle_climb_angle_pad, a >= A.dangle_release_angle_pad)
                } else {
                    (a < A.dangle_climb_angle, a >= A.dangle_climb_angle)
                };
                if self.stick && (climb || release) {
                    if release {
                        // RELEASE: the cool-down; Fall(actorYaw); DoChain.
                        t.dangle_cooldown = t.dangle_cooldown.max(A.dangle_cooldown);
                        let yaw = s.yaw;
                        jump::fall(s, yaw);
                    } else {
                        // CLIMB: the cool-down is zeroed; Flying; `GripRise` (mode 2).
                        t.dangle_cooldown = 0.0;
                        s.cmc.stop_movement();
                        s.cmc.set_flying();
                        s.cmc.gravity_off = true;
                        s.anim.play_with(AnimId::GripRise, PlayArgs::mode(2, -1.0));
                        self.sub = 2;
                    }
                } else {
                    // STAY: pawn+0x6F8 = max(pawn+0x6F8, 0.6) on every hanging tick.
                    t.dangle_cooldown = t.dangle_cooldown.max(A.dangle_cooldown);
                    // (party I1) a Movable component (0x14150F5BB): FUN_140638690(pawn, 10.0) false -> Fall(actorYaw);
                    // DoChain (the base moved away from under the contact)
                    let (feet, yaw) = (s.cmc.location, s.yaw);
                    if w.component_motion(self.component).is_some_and(|m| m.movable) && !ledge_supports(s, w, feet, yaw) {
                        jump::fall(s, yaw);
                    }
                }
            }
            2 => {
                if s.anim.ended {
                    s.queue.do_chain();
                }
            }
            _ => s.queue.do_chain(),
        }
        self.stick = false;
    }

    /// End (0x14136B710 -> 0x14150BB80): SetMovementMode(3 Falling, 0).
    fn end(&mut self, cx: &mut Ctx) {
        let s = &mut *cx.pawn;
        if self.sub != 3 {
            s.cmc.set_falling();
            s.cmc.gravity_off = false;
            if matches!(self.sub, 0 | 1) {
                // Let go by another state (damage, the jump button): the cool-down stands.
                let t = &mut s.actions.traversal;
                t.dangle_cooldown = t.dangle_cooldown.max(A.dangle_cooldown);
            }
            s.event(EventKind::Traversal { kind: "dangle", id: self.component, on: false });
        }
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}
