//! The movement component: `TresCharMovementComponent` as docs/sora-movement-spec.md revision 2
//! describes it (a fork of UE 4.17 `UCharacterMovementComponent`, read at instruction level).
//! Each function names the spec section and the exe address it stands for; the items of
//! docs/sora-movement-spec-changes.md are cited as "change B5" etc.
//!
//! `location` is the ACTOR ORIGIN (foot level). The body is one upright capsule, radius 60, half
//! height 89, whose centre is 91 cm above the origin (spec 6); every move and the floor sweep use
//! it unshrunk. A hit's `location` here is the actor origin at the hit (change B9).
//!
//! Left out, as spec 7 lists: swimming, flying, bases, RVO, force volumes, external velocities.
//! Not possible with the host interface, and said where it matters:
//! - the encroach push-out before the physics (spec 3.2; needs an overlap report);
//! - `ResolvePenetration`'s overlap test: a move that starts overlapping is moved out along the
//!   hit normal by the reported depth (0.125 when none) plus 0.125 and tried once more;
//! - the `Time == 0` "unstick" of the falling code (change C4) is two plain moves.
//!
//! ASSUMPTION `move.penetration_pullback`: the run-time value of `p.PenetrationPullbackDistance`
//! was not read (runtime item R9); stock 0.125 is used.

use glam::{Vec2, Vec3};

use crate::params::SoraParams;
use crate::world::{Hit, World};

const MIN_TICK_TIME: f32 = 1.0e-6;
const KINDA_SMALL_NUMBER: f32 = 1.0e-4;
/// `VERTICAL_SLOPE_NORMAL_Z`
const VERTICAL_SLOPE_NORMAL_Z: f32 = 0.001;
/// `p.PenetrationPullbackDistance` (ASSUMED stock).
const PENETRATION_PULLBACK: f32 = 0.125;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MovementMode {
    Walking,
    Falling,
    /// `MOVE_Flying` (5): set by the air-mode attacks (combat spec 4.3 tail, 6.1). The fork's
    /// flying physics is not specified (combat spec open question 3): here the body moves by its
    /// velocity plus the root motion and slides along what it meets; no gravity, no friction.
    Flying,
    /// (actions implementer) `MOVE_Custom` (6): its Phys function is an empty stub (actions spec
    /// II.6 table 0x14059703C): no movement at all. Used for every state that places the actor
    /// itself: the ledge hang, and here also the rail (`PhysRailSlide` moves him without a
    /// sweep), the pole and the hop point (he is attached to a component).
    Custom,
}

/// `FFindFloorResult` (spec 5; change B8).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FloorResult {
    /// A valid blocking hit (not start-penetrating).
    pub blocking_hit: bool,
    pub walkable: bool,
    pub line_trace: bool,
    pub floor_dist: f32,
    pub line_dist: f32,
    pub hit: Option<Hit>,
    /// The actor origin at the hit (`Hit.Location`).
    pub hit_location: Vec3,
}

impl FloorResult {
    const NONE: FloorResult = FloorResult { blocking_hit: false, walkable: false, line_trace: false, floor_dist: 0.0, line_dist: 0.0, hit: None, hit_location: Vec3::ZERO };

    fn start_penetrating(&self) -> bool {
        self.hit.is_some_and(|h| h.start_penetrating)
    }
}

/// The result of one sweep move (`FHitResult` as the movement code uses it).
#[derive(Clone, Copy, Debug)]
pub struct Move {
    blocking: bool,
    /// Fraction of the move made (after the pull-back); 1 when nothing was hit.
    time: f32,
    hit: Option<Hit>,
    /// The actor origin where the move ended (`Hit.Location`, change B9).
    location: Vec3,
}

impl Move {
    fn start_penetrating(&self) -> bool {
        self.hit.is_some_and(|h| h.start_penetrating)
    }

    /// `IsValidBlockingHit`
    fn valid_blocking(&self) -> bool {
        self.blocking && !self.start_penetrating()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cmc {
    /// Actor origin, cm.
    pub location: Vec3,
    pub velocity: Vec3,
    pub acceleration: Vec3,
    pub mode: MovementMode,
    pub floor: FloorResult,
    floor_known: bool,
    pub max_walk_speed: f32,
    pub max_acceleration: f32,
    pub gravity_scale: f32,
    pub jump_z_velocity: f32,
    /// Walk-mode bit (component +0xA3C bit 0): `AnalogInputModifier` becomes `WalkSpeedRate`.
    pub walk_mode: bool,
    /// PORT OPTION `Tuning::speed_ramp` only (not a field of the game's component): when set, the
    /// ground speed limit of the next movement tick, in place of MaxWalkSpeed x AnalogInputModifier.
    pub speed_limit_override: Option<f32>,
    analog_input_modifier: f32,
    /// `APawn::ControlInputVector` (pawn +0x428): assigned by the states, consumed next frame.
    pub control_input: Vec3,
    /// `bPressedJump` (pawn +0x5BC), the second latch (pawn +0x5C0 bit 0) and
    /// `JumpKeyHoldTime` (pawn +0x5C4): spec 4.1.
    pub pressed_jump: bool,
    pub jump_latch: bool,
    pub jump_key_hold_time: f32,
    /// Actor Z at the latest `DoJump`. Not a field of the game's component: bookkeeping for the
    /// port option `Tuning::min_jump_height`.
    pub launch_z: f32,
    /// Jump-duration flag (+0xBFC bit 0) and timer (+0xC00): spec 4.3.
    jump_duration_finished: bool,
    jump_duration_timer: f32,
    /// `m_bDisableMoveOnSpawnFrame`: the first physics call after spawn is skipped (change B11).
    skip_spawn_frame: bool,
    /// Root-motion velocity of this tick (+0xA24), world space.
    root_motion_velocity: Vec3,
    /// The displacement of the last tick over its dt (`MyMovement + 0x9A0`, 0x140591690): what
    /// the camera reads as the pawn's velocity.
    pub displacement_velocity: Vec3,
    /// The "direct XY" record (movement slot 319 0x140592E10, component +0xC0C / +0xC18 bit 0;
    /// combat spec 6.1): a state wrote `Velocity.xy` directly. ASSUMPTION (combat spec open
    /// question 3: how the walking / falling code treats the flag was not read): the next
    /// movement tick moves with exactly this horizontal velocity - no acceleration, friction or
    /// braking - and consumes the record.
    pub direct_xy: Option<Vec2>,
    /// The same for Z (movement slot 321 0x140592E40).
    pub direct_z: Option<f32>,
    /// The gravity-disable bit (component +0xA3C bit 1; pawn slot 741(false)).
    pub gravity_off: bool,
    /// The first blocking hit of this tick's moves with the horizontal velocity he had when he
    /// met it (the engine's blocking-hit event: the Damage state's wall hit, event interface slot
    /// 9 0x141356930, reads it; combat spec 9.4). Cleared at the start of every movement tick.
    pub last_block: Option<(Hit, Vec3)>,
    /// (magic implementer) pawn vt+0xFF0(0) (0x1405C9B00): the root's responses to the world
    /// channels are "ignore": every move is free (the airstep's intermediate hops, magic spec H 4.4).
    pub world_collision_off: bool,
}

impl Cmc {
    pub fn new(p: &SoraParams, location: Vec3) -> Cmc {
        Cmc {
            location,
            velocity: Vec3::ZERO,
            acceleration: Vec3::ZERO,
            mode: MovementMode::Walking,
            floor: FloorResult::NONE,
            floor_known: false,
            max_walk_speed: p.speed_run,
            // TresAICharMovementComponent ctor, 0x14051240A
            max_acceleration: p.accel_default,
            gravity_scale: p.gravity_scale,
            jump_z_velocity: 0.0,
            walk_mode: false,
            speed_limit_override: None,
            analog_input_modifier: 1.0,
            control_input: Vec3::ZERO,
            pressed_jump: false,
            jump_latch: false,
            jump_key_hold_time: 0.0,
            launch_z: 0.0,
            jump_duration_finished: true,
            jump_duration_timer: 0.0,
            skip_spawn_frame: p.move_spawn_frame_skip,
            root_motion_velocity: Vec3::ZERO,
            displacement_velocity: Vec3::ZERO,
            direct_xy: None,
            direct_z: None,
            gravity_off: false,
            last_block: None,
            world_collision_off: false,
        }
    }

    /// The body push of this tick (combat spec 6.5; combat/push.rs): one extra sweep after the
    /// physics, blocked by the world like every move; it counts in the measured velocity.
    pub fn push_move(&mut self, w: &dyn World, p: &SoraParams, push: Vec3, dt: f32) {
        let before = self.location;
        let keep = self.last_block;
        self.safe_move(w, p, push);
        self.last_block = keep;
        self.displacement_velocity += (self.location - before) / dt;
    }

    /// `IsMovingOnGround` (slot 167 0x1405893A0): the game's "on ground" (0x1405AD850).
    pub fn on_ground(&self) -> bool {
        self.mode == MovementMode::Walking
    }

    /// The floor test at the current location (component vt+0x9D8, slot 315 0x1405894A0;
    /// actions spec II.3): `FindFloor`, true when the result is blocking and walkable. Added for
    /// the Jump state's fall Begin (the land-pose landing).
    pub fn floor_under(&self, w: &dyn World, p: &SoraParams) -> bool {
        let floor = self.find_floor(w, p, None);
        floor.blocking_hit && floor.walkable
    }

    /// `IsJumpDurationFinish` (slot 297 0x140589380).
    pub fn jump_duration_finished(&self, p: &SoraParams) -> bool {
        self.jump_duration_finished || self.jump_duration_timer >= p.gravity_jump_duration_time
    }

    pub fn capsule_centre(&self, p: &SoraParams) -> Vec3 {
        self.location + Vec3::Z * p.capsule_center_z
    }

    /// `StopMovement` (pawn slot 733 0x140530DD0): Velocity = 0, all axes; nothing else.
    pub fn stop_movement(&mut self) {
        self.velocity = Vec3::ZERO;
    }

    /// `SetVelocityXY` (pawn slot 728 -> movement slot 319): writes the velocity and the direct record.
    pub fn set_velocity_xy(&mut self, v: Vec2) {
        self.velocity.x = v.x;
        self.velocity.y = v.y;
        self.direct_xy = Some(v);
    }

    /// `SetVelocityZ` (pawn slot 730 -> movement slot 321).
    pub fn set_velocity_z(&mut self, z: f32) {
        self.velocity.z = z;
        self.direct_z = Some(z);
    }

    /// `SetMovementMode(Flying)` (pawn slot 756 with 5).
    pub fn set_flying(&mut self) {
        self.set_mode_raw(MovementMode::Flying);
    }

    /// `SetMovementMode(Falling)` (3) from Flying or Walking.
    pub fn set_falling(&mut self) {
        self.set_mode_raw(MovementMode::Falling);
    }

    /// (actions implementer) `SetMovementMode(Custom 6)`: see `MovementMode::Custom`.
    pub fn set_custom(&mut self) {
        self.set_mode_raw(MovementMode::Custom);
    }

    pub fn is_custom(&self) -> bool {
        self.mode == MovementMode::Custom
    }

    pub fn is_flying(&self) -> bool {
        self.mode == MovementMode::Flying
    }

    /// The jump launch handler's pawn part (pawn slot 530 0x1405ADF50), spec 4.1.
    pub fn press_jump(&mut self, power: f32) {
        self.pressed_jump = true;
        self.jump_latch = true;
        self.jump_key_hold_time = 0.0;
        self.jump_z_velocity = power;
    }

    /// `Jump::End`: clear the pawn jump latch.
    pub fn stop_jumping(&mut self) {
        self.pressed_jump = false;
        self.jump_latch = false;
        self.jump_key_hold_time = 0.0;
    }

    /// `TickComponent` (slot 97 0x140598190) steps a to c (spec 1.2, 3.2): what runs before the
    /// pose tick.
    pub fn tick_head(&mut self, w: &dyn World, p: &SoraParams, dt: f32) {
        if !self.floor_known {
            // The mode is set to Walking at spawn: OnMovementModeChanged finds the floor.
            self.floor_known = true;
            self.floor = self.find_floor(w, p, None);
            self.adjust_floor_height(w, p);
        }
        // a. ConsumeInputVector: what the state wrote in the previous frame (change A2).
        let input = std::mem::take(&mut self.control_input);
        // b. CheckJumpInput (0x1405A5B20), spec 4.1: sees the latch the notify set last frame.
        if self.pressed_jump || self.jump_latch {
            let was_held = self.jump_key_hold_time > 0.0;
            self.jump_key_hold_time += dt;
            // The interface test (iface vt+0x108) is constant true; DoJump always succeeds.
            self.do_jump();
            if !was_held {
                self.jump_latch = false;
            }
        }
        // c. Acceleration = MaxAcceleration x (|v|^2 > 1 ? normalize(v) : v), Z removed (slots 271, 272).
        let v = Vec3::new(input.x, input.y, 0.0);
        self.acceleration = self.max_acceleration * if v.length_squared() > 1.0 { v.normalize() } else { v };
        // AnalogInputModifier (slot 177 0x14057D050): 1.0 (the pawn's modifier is always 1), or
        // WalkSpeedRate in walk mode.
        self.analog_input_modifier = if self.walk_mode { p.speed_walk_rate } else { 1.0 };
    }

    /// `PerformMovement` (slot 276 0x14058CC60), spec 3.2: root motion to velocity, clear the
    /// jump input, physics, then the root-motion rotation on the actor. `root_motion` is this
    /// frame's root translation in world space and its yaw change.
    pub fn perform_movement(&mut self, w: &dyn World, p: &SoraParams, dt: f32, root_motion: Option<(Vec3, f32)>, yaw: &mut f32) {
        let before = self.location;
        self.last_block = None;
        // Slot 282 0x1405969D0: RM = world root translation of this tick / dt (the division by
        // the blend weight undoes the weighting the animation side applied: spec 3.6).
        self.root_motion_velocity = root_motion.map_or(Vec3::ZERO, |(translation, _)| translation / dt);
        // ClearJumpInput (0x1405A5D80, called at 0x14058CE88 before the physics): the max hold time
        // is 0, so bPressedJump is cleared at once; the latch stays.
        self.pressed_jump = false;
        // (Encroach check, slot 404: not possible here; see the file header.)
        if std::mem::take(&mut self.skip_spawn_frame) {
            // m_bDisableMoveOnSpawnFrame.
        } else {
            let mut iterations = 0;
            self.start_new_physics(w, p, dt, &mut iterations);
        }
        // Root-motion rotation is applied to the actor after the physics (change B11).
        if let Some((_, yaw_delta)) = root_motion {
            *yaw += yaw_delta;
        }
        self.root_motion_velocity = Vec3::ZERO;
        // The post step 0x14058C770 -> 0x140591690.
        self.displacement_velocity = (self.location - before) / dt;
    }

    /// `DoJump` (slot 195 0x14057ECB0, `same`): spec 4.1. (`pub`: kh-party's NPC jump launch calls it
    /// directly, movement vt+0x618 in 0x14122B940.)
    pub fn do_jump(&mut self) {
        // (For the port option `Tuning::min_jump_height` only: where the jump took off.)
        self.launch_z = self.location.z;
        self.velocity.z = self.jump_z_velocity; // plain assignment
        self.set_mode_raw(MovementMode::Falling);
        // SetJumpDurationFinish(false) (slot 296 0x140594910): opens the three-phase curve.
        self.jump_duration_finished = false;
        self.jump_duration_timer = 0.0;
    }

    /// The part of `SetMovementMode` (0x140594A00) that needs no world query.
    fn set_mode_raw(&mut self, mode: MovementMode) {
        if self.mode == mode {
            return;
        }
        self.mode = mode;
        // Every real mode change calls SetJumpDurationFinish(true) (0x140594A9C).
        self.jump_duration_finished = true;
        if mode == MovementMode::Falling {
            self.floor = FloorResult::NONE;
        }
    }

    /// `SetMovementMode(Walking)` with `OnMovementModeChanged`: `Velocity.Z = 0`, the floor found
    /// with the walking distances, the height adjusted (spec 4.6).
    fn set_walking(&mut self, w: &dyn World, p: &SoraParams) {
        let changed = self.mode != MovementMode::Walking;
        self.set_mode_raw(MovementMode::Walking);
        if changed {
            self.velocity.z = 0.0;
            self.floor = self.find_floor(w, p, None);
            self.adjust_floor_height(w, p);
        }
    }

    /// `StartNewPhysics` (slot 194 0x140596E10).
    fn start_new_physics(&mut self, w: &dyn World, p: &SoraParams, dt: f32, iterations: &mut u32) {
        if dt < MIN_TICK_TIME || *iterations >= p.sim_max_iterations as u32 {
            return;
        }
        match self.mode {
            MovementMode::Walking => self.phys_walking(w, p, dt, iterations),
            MovementMode::Falling => self.phys_falling(w, p, dt, iterations),
            MovementMode::Flying => self.phys_flying(w, p, dt),
            MovementMode::Custom => {}
        }
    }

    /// The sub-step rule of spec 3.4: one step of the whole dt for dt <= 0.1.
    fn simulation_time_step(&self, p: &SoraParams, remaining: f32, iterations: u32) -> f32 {
        let mut step = remaining;
        if step > p.sim_max_time_step && iterations < p.sim_max_iterations as u32 {
            step = p.sim_max_time_step.min(step * 0.5);
        }
        step.max(MIN_TICK_TIME)
    }

    // ------------------------------------------------------------------ velocity

    /// `CalcVelocity` (slot 211 0x1405795F0), spec 3.3. Works on a velocity and an acceleration
    /// given by the caller, because the falling code calls it with the vertical part removed.
    fn calc_velocity(&self, p: &SoraParams, velocity: Vec3, acceleration: Vec3, dt: f32, friction: f32, braking_deceleration: f32) -> Vec3 {
        if dt < MIN_TICK_TIME {
            return velocity;
        }
        let friction = friction.max(0.0);
        // GetMaxSpeed (slot 120 0x140583BF0): MaxWalkSpeed for Walking AND Falling.
        let max_speed = self.speed_limit_override.unwrap_or(self.max_walk_speed * self.analog_input_modifier).max(0.0);
        let over = |v: Vec3| v.length_squared() > max_speed * max_speed * p.move_over_max_speed_factor;
        let zero_acceleration = acceleration == Vec3::ZERO;
        let over_max = over(velocity);
        let mut v = velocity;
        if zero_acceleration || over_max {
            let old = v;
            v = apply_velocity_braking(p, v, dt, friction, braking_deceleration);
            if over_max && v.length_squared() < max_speed * max_speed && acceleration.dot(old) > 0.0 {
                v = old.normalize_or_zero() * max_speed;
            }
        } else {
            v -= (v - acceleration.normalize_or_zero() * v.length()) * (dt * friction).min(1.0);
        }
        let new_max = if over(v) { v.length() } else { max_speed };
        v += acceleration * dt;
        // Exactly 0 when NewMax < 1e-4 (change B13).
        if new_max < KINDA_SMALL_NUMBER {
            Vec3::ZERO
        } else {
            v.clamp_length_max(new_max)
        }
    }

    // ------------------------------------------------------------------ walking

    /// Flying (see `MovementMode::Flying`): one move by the velocity plus the root motion.
    fn phys_flying(&mut self, w: &dyn World, p: &SoraParams, dt: f32) {
        if let Some(v) = self.direct_xy.take() {
            self.velocity.x = v.x;
            self.velocity.y = v.y;
        }
        if let Some(z) = self.direct_z.take() {
            self.velocity.z = z;
        }
        let delta = (self.velocity + self.root_motion_velocity) * dt;
        let mut hit = self.safe_move(w, p, delta);
        if hit.blocking {
            if let Some(h) = hit.hit {
                let time = 1.0 - hit.time;
                self.slide_along_surface(w, p, delta, time, h.normal, &mut hit);
            }
        }
    }

    /// `PhysWalking` (slot 246 0x1405900B0), spec 3.4.
    fn phys_walking(&mut self, w: &dyn World, p: &SoraParams, dt: f32, iterations: &mut u32) {
        if dt < MIN_TICK_TIME {
            return;
        }
        // MaintainHorizontalGroundVelocity.
        self.velocity.z = 0.0;
        self.acceleration.z = 0.0;
        // ONCE with the whole dt (0x1405902DF).
        self.direct_z = None;
        self.velocity = match self.direct_xy.take() {
            // A state wrote the velocity directly (see `direct_xy`).
            Some(v) => Vec3::new(v.x, v.y, 0.0),
            None => self.calc_velocity(p, self.velocity, self.acceleration, dt, p.ground_friction, p.ground_braking_deceleration),
        };
        // MoveVel = (V.x + RM.x, V.y + RM.y, RM.z) (change B12).
        let rm = self.root_motion_velocity;
        let move_velocity = Vec3::new(self.velocity.x + rm.x, self.velocity.y + rm.y, rm.z);

        let mut remaining = dt;
        let mut checked_fall = false;
        while remaining >= MIN_TICK_TIME && *iterations < p.sim_max_iterations as u32 {
            *iterations += 1;
            let step = self.simulation_time_step(p, remaining, *iterations);
            remaining -= step;
            let old_location = self.location;
            let old_floor_walkable = self.floor.walkable;
            let delta = move_velocity * step;
            let zero_delta = delta.abs().max_element() <= KINDA_SMALL_NUMBER;
            let mut step_down_floor = None;
            if zero_delta {
                remaining = 0.0;
            } else {
                self.move_along_floor(w, p, move_velocity, step, &mut step_down_floor);
            }
            self.floor = match step_down_floor {
                Some(floor) => floor,
                None => self.find_floor(w, p, None),
            };
            if self.floor.walkable {
                self.adjust_floor_height(w, p);
            } else if self.floor.start_penetrating() && remaining <= 0.0 {
                // Resolve the penetration upward.
                if let Some(hit) = self.floor.hit {
                    self.location += penetration_adjustment(&hit);
                }
            }
            if !self.floor.walkable && !self.floor.start_penetrating() {
                // Edge float (slot 409 0x14057BD10), spec 5.4, before CheckFall (change B4).
                if old_floor_walkable && self.edge_float(w, p) {
                    return;
                }
                let must_jump = zero_delta;
                if must_jump || !checked_fall {
                    // CheckFall (0x14057BC80) -> StartFalling (0x140596D10): the walking move was
                    // made in full; Falling gets only the unused time fraction (0 when the move was
                    // unobstructed), so gravity starts in the next tick.
                    let desired = delta.length();
                    let actual = (self.location - old_location).truncate().length();
                    remaining = if desired < KINDA_SMALL_NUMBER { 0.0 } else { remaining + step * (1.0 - (actual / desired).min(1.0)) };
                    self.set_mode_raw(MovementMode::Falling);
                    self.start_new_physics(w, p, remaining, iterations);
                    return;
                }
                checked_fall = true;
            }
            // NO "Velocity = (location - OldLoc) / t".
            if self.location == old_location {
                break;
            }
        }
        if self.on_ground() {
            self.velocity.z = 0.0;
        }
    }

    /// `MoveAlongFloor` (0x14058A240), spec 3.4.
    fn move_along_floor(&mut self, w: &dyn World, p: &SoraParams, velocity: Vec3, dt: f32, step_down_floor: &mut Option<FloorResult>) {
        if !self.floor.walkable {
            return;
        }
        let delta = Vec3::new(velocity.x, velocity.y, 0.0) * dt;
        let ramp = self.ground_movement_delta(p, delta, self.floor.hit, self.floor.line_trace);
        let mut hit = self.safe_move(w, p, ramp);
        if hit.start_penetrating() {
            let normal = hit.hit.map_or(Vec3::Z, |h| h.normal);
            self.slide_along_surface(w, p, delta, 1.0, normal, &mut hit);
            return;
        }
        if hit.blocking {
            let mut applied = hit.time;
            let h = hit.hit.unwrap();
            if hit.time > 0.0 && h.normal.z > KINDA_SMALL_NUMBER && is_walkable(p, &h) {
                // Hit a walkable ramp.
                let left = 1.0 - applied;
                let ramp = self.ground_movement_delta(p, delta * left, Some(h), false);
                hit = self.safe_move(w, p, ramp);
                applied = (applied + hit.time * left).clamp(0.0, 1.0);
            }
            if hit.valid_blocking() {
                let h = hit.hit.unwrap();
                // CanStepUp (slot 215 0x14057A3E0): true for level geometry.
                if !self.step_up(w, p, delta * (1.0 - applied), &hit, step_down_floor) {
                    self.slide_along_surface(w, p, delta, 1.0 - applied, h.normal, &mut hit);
                }
            }
        }
    }

    /// `ComputeGroundMovementDelta` (0x14057DC90, stock): the horizontal speed is kept on slopes.
    fn ground_movement_delta(&self, p: &SoraParams, delta: Vec3, ramp: Option<Hit>, from_line_trace: bool) -> Vec3 {
        let Some(ramp) = ramp else { return delta };
        let (floor_normal, contact_normal) = (ramp.impact_normal, ramp.normal);
        if floor_normal.z < p.walk_ramp_flat_limit && floor_normal.z > KINDA_SMALL_NUMBER && contact_normal.z > KINDA_SMALL_NUMBER && !from_line_trace && is_walkable(p, &ramp) {
            return Vec3::new(delta.x, delta.y, -floor_normal.dot(delta) / floor_normal.z);
        }
        delta
    }

    /// The fork's `StepUp` (slot 216 0x140597570), spec 3.4, change B5: no capsule size, no
    /// top-of-capsule test, no edge-tolerance test on the down sweep.
    fn step_up(&mut self, w: &dyn World, p: &SoraParams, delta: Vec3, in_hit: &Move, step_down_floor: &mut Option<FloorResult>) -> bool {
        let Some(in_h) = in_hit.hit else { return false };
        let origin = self.location;
        if !(in_h.position.z > origin.z) {
            return false;
        }
        let max_step = p.floor_max_step_height;
        let fd = if self.on_ground() && self.floor.walkable { (self.floor.floor_dist - p.step_up_floor_dist_offset).max(0.0) } else { 0.0 };
        let up = (max_step - fd).max(0.0);
        let down = max_step + p.step_up_down_extra;
        let floor_point_z = origin.z - fd;
        let step_side_z = in_h.impact_normal.z;
        let revert = |me: &mut Cmc| {
            me.location = origin;
            false
        };
        // Up.
        let moved = self.move_updated(w, p, Vec3::Z * up);
        if moved.start_penetrating() {
            return revert(self);
        }
        // Forward by the unapplied delta.
        let mut forward = self.move_updated(w, p, delta);
        if forward.blocking {
            if forward.start_penetrating() {
                return revert(self);
            }
            let time = forward.time;
            let normal = forward.hit.unwrap().normal;
            let slide = self.slide_along_surface(w, p, delta, 1.0 - time, normal, &mut forward);
            if time == 0.0 && slide == 0.0 {
                return revert(self);
            }
        }
        // Down 51.8.
        let landed = self.move_updated(w, p, Vec3::NEG_Z * down);
        if landed.start_penetrating() {
            return revert(self);
        }
        if landed.blocking {
            let h = landed.hit.unwrap();
            let dz = h.position.z - floor_point_z;
            if dz > max_step {
                return revert(self);
            }
            if !is_walkable(p, &h) {
                if delta.dot(h.impact_normal) < 0.0 {
                    return revert(self);
                }
                if landed.location.z > origin.z {
                    return revert(self);
                }
            }
            // (dz > 0 && !CanStepUp(hit): CanStepUp is true for level geometry.)
            let floor = self.find_floor(w, p, Some(&landed));
            if landed.location.z > origin.z && !floor.blocking_hit && step_side_z < p.step_up_side_z_max {
                return revert(self);
            }
            *step_down_floor = Some(floor);
        }
        true
    }

    // ------------------------------------------------------------------ floor

    /// `FindFloor` (slot 259 0x14057FA80), spec 5.1: the main query, then the perch stage.
    /// `downward` is a vertical down-sweep that has just been made (`StepUp`, landing tests).
    fn find_floor(&self, w: &dyn World, p: &SoraParams, downward: Option<&Move>) -> FloorResult {
        let walking = self.on_ground();
        let adj = if walking { p.floor_dist_max + KINDA_SMALL_NUMBER } else { -p.floor_dist_max };
        let distance = p.floor_dist_max.max(p.floor_max_step_height + adj); // 49.4001 walking, 44.6 falling
        let mut out = self.compute_floor_dist(w, p, self.location, distance, distance, 0.0, downward);
        let Some(hit) = out.hit else { return out };
        // ShouldComputePerchResult (0x140596260): the contact is more than 1.0 cm (2D) from the axis.
        if out.blocking_hit && !out.line_trace && (hit.position - out.hit_location).truncate().length() > p.floor_perch_radius {
            // 59.4001 walking / 54.6 falling.
            let max_perch = p.floor_dist_max.max(adj + p.floor_max_step_height + p.floor_falling_perch_additional_height) + if walking { p.floor_perch_additional_height.max(0.0) } else { 0.0 };
            match self.compute_perch_result(w, p, &out, max_perch) {
                Some(perch) => {
                    let target = (p.floor_dist_min + p.floor_dist_max) * 0.5;
                    if (target - out.floor_dist) + perch.floor_dist >= max_perch {
                        out.floor_dist = target;
                    }
                    if !out.walkable {
                        // The perch hit as a line-trace floor.
                        if let Some(perch_hit) = perch.hit {
                            out.line_trace = true;
                            out.line_dist = perch.floor_dist.min(if perch.line_trace { perch.line_dist } else { perch.floor_dist });
                            out.hit = Some(Hit { time: hit.time, position: hit.position, ..perch_hit });
                            out.blocking_hit = true;
                            out.walkable = true;
                        }
                    }
                }
                None if out.walkable => {
                    // EdgeFallCheck (slot 260 0x1405887B0), spec 5.4.
                    if self.edge_fall_check(w, p, &out, max_perch, walking) {
                        out.walkable = false;
                    }
                }
                None => out.walkable = false,
            }
        }
        out
    }

    /// `ComputePerchResult` (0x14057E0C0), spec 5.2: a thin probe under the floor hit's origin.
    fn compute_perch_result(&self, w: &dyn World, p: &SoraParams, floor: &FloorResult, max_perch: f32) -> Option<FloorResult> {
        let hit = floor.hit?;
        let above_base = (hit.position.z - floor.hit_location.z).max(0.0);
        let line = (max_perch - above_base).max(0.0);
        let perch = self.compute_floor_dist(w, p, floor.hit_location, line, max_perch.max(0.0), p.floor_perch_radius, None);
        (perch.walkable && above_base + perch.floor_dist <= max_perch).then_some(perch)
    }

    /// `ComputeFloorDist` (slot 264 0x14057D100), spec 5.2. `sweep_radius` 0: the main path (the
    /// real body, unshrunk, no pull-back; the line from the actor origin). `sweep_radius` > 0:
    /// the perch probe (change B2).
    #[allow(clippy::too_many_arguments)]
    fn compute_floor_dist(&self, w: &dyn World, p: &SoraParams, location: Vec3, line_distance: f32, sweep_distance: f32, sweep_radius: f32, downward: Option<&Move>) -> FloorResult {
        let mut out = FloorResult::NONE;
        // A vertical downward sweep that was just made: use it.
        if let Some(down) = downward {
            if let (true, Some(hit)) = (down.valid_blocking(), down.hit) {
                let dist = location.z - down.location.z;
                out = FloorResult { blocking_hit: true, walkable: false, line_trace: false, floor_dist: dist, line_dist: 0.0, hit: Some(hit), hit_location: down.location };
                if is_walkable(p, &hit) {
                    out.walkable = true;
                    return out;
                }
            }
        }
        // (centre of the swept shape above `location`, radius, half height, shrink, line start height)
        let perch = sweep_radius > 0.0;
        let probe_half = p.floor_perch_probe_min_half_height; // 15
        let (centre_up, mut radius, mut half_height, mut shrink, line_up) =
            if perch { (probe_half, sweep_radius, probe_half - (probe_half - sweep_radius) * 0.1, (probe_half - sweep_radius) * 0.1, probe_half) } else { (p.capsule_center_z, p.capsule_radius, p.capsule_half_height, 0.0, 0.0) };
        let centre = location + Vec3::Z * centre_up;
        let mut swept: Option<Hit> = None;
        let mut trace = sweep_distance + shrink;
        if sweep_distance > 0.0 {
            swept = w.sweep_capsule(centre, centre - Vec3::Z * trace, radius, half_height);
            if perch {
                // Start-penetrating, or the impact is on the rim of the probe: a smaller capsule.
                if let Some(hit) = swept {
                    if hit.start_penetrating || !within_edge_tolerance(p, location, hit.position, sweep_radius) {
                        radius = (sweep_radius - p.floor_sweep_edge_reject - KINDA_SMALL_NUMBER).max(0.0); // 0.8499
                        shrink = (probe_half - sweep_radius) * 0.9; // 12.6
                        half_height = (probe_half - shrink).max(radius); // 2.4
                        trace = sweep_distance + shrink;
                        swept = w.sweep_capsule(centre, centre - Vec3::Z * trace, radius, half_height);
                    }
                }
            }
        }
        if let Some(hit) = swept {
            let dist = if perch { (trace * hit.time - shrink).max(-p.floor_dist_max) } else { sweep_distance * hit.time };
            out = FloorResult { blocking_hit: !hit.start_penetrating, walkable: false, line_trace: false, floor_dist: dist, line_dist: 0.0, hit: Some(hit), hit_location: location - Vec3::Z * dist };
            if !hit.start_penetrating && is_walkable(p, &hit) && dist <= sweep_distance {
                out.walkable = true;
                return out;
            }
        }
        // The line trace is not run when the sweep missed everything.
        if !out.blocking_hit && !out.start_penetrating() {
            out.floor_dist = sweep_distance;
            return out;
        }
        if line_distance > 0.0 {
            let start = location + Vec3::Z * line_up;
            let length = line_distance + line_up;
            if let Some(line) = w.line_trace(start, start - Vec3::Z * length) {
                if line.time > 0.0 {
                    let d = line.time * length - line_up;
                    out.blocking_hit = true;
                    if d <= line_distance && is_walkable(p, &line) {
                        // SetFromLineTrace (0x143FFD880): the sweep's Time, Location and
                        // ImpactPoint are kept; normals and component come from the line.
                        let kept = out.hit.unwrap_or(line);
                        out.hit = Some(Hit { time: kept.time, position: kept.position, ..line });
                        out.line_trace = true;
                        out.line_dist = d;
                        out.walkable = true;
                        return out;
                    }
                }
            }
        }
        out.walkable = false;
        out.floor_dist = sweep_distance;
        out
    }

    /// `EdgeFallCheck` (slot 260 0x1405887B0), spec 5.4: true = drop the floor.
    fn edge_fall_check(&self, w: &dyn World, p: &SoraParams, floor: &FloorResult, max_perch: f32, walking: bool) -> bool {
        let Some(hit) = floor.hit else { return true };
        if !p.edge_fall_check_enabled {
            return false;
        }
        let location = self.location;
        let fd = floor.floor_dist.max(0.0);
        let target = (p.floor_dist_min + p.floor_dist_max) * 0.5;
        // Part 1 (m_bNeedFallTravelCheck = 1): something on the pawn's side of the contact?
        let offset = location.truncate() - hit.position.truncate();
        let reach = offset.length();
        if reach <= p.edge_sphere_radius {
            let direction = offset.normalize_or_zero();
            let point = location.truncate() + direction * (p.edge_sphere_radius - 1.0 - reach).max(0.0);
            let z0 = location.z - fd.min(target);
            let (from, to) = (Vec3::new(point.x, point.y, z0 + 37.5), Vec3::new(point.x, point.y, z0 - 5.0));
            let r = p.edge_sphere_radius;
            for other in w.sweep_capsule_all(from, to, r, r) {
                // The pawn is not in the host's world; the floor component is ignored.
                if other.start_penetrating || (hit.component != 0 && other.component == hit.component) {
                    continue;
                }
                if direction.dot((other.position.truncate() - point).normalize_or_zero()) >= 0.0 {
                    return false;
                }
            }
        }
        // Part 2: too far out, or no walkable ground under the origin.
        let limit = p.edge_contact_limit_walking - if walking { 0.0 } else { 10.0 };
        if reach > limit {
            return true;
        }
        let mut left = max_perch + fd.min(target) - (hit.position.z - location.z).max(0.0);
        let mut position = location;
        let mut direction = Vec3::NEG_Z;
        for _ in 0..p.edge_line_max_traces as usize {
            if left <= 0.0 || direction == Vec3::ZERO {
                break;
            }
            let Some(line) = w.line_trace(position, position + direction * left) else { break };
            if is_walkable(p, &line) {
                return false;
            }
            // Step back 0.5 and slide along the face.
            let travelled = left * line.time;
            position += direction * (travelled - 0.5).max(0.0);
            left -= travelled;
            direction = plane_project(direction, line.impact_normal).normalize_or_zero();
        }
        true
    }

    /// `EdgeFloat` (slot 409 0x14057BD10; `m_bCheckEdgeFloat` = 1), spec 5.4: when the floor was
    /// walkable before the sub-step and is not now, slide (0, 0, -3.225) along the floor hit
    /// normal; if that ends on a valid landing spot with a walkable floor: stay Walking.
    fn edge_float(&mut self, w: &dyn World, p: &SoraParams) -> bool {
        if !p.edge_float_enabled {
            return false;
        }
        let Some(floor_hit) = self.floor.hit else { return false };
        let before = (self.location, self.floor);
        let slide = plane_project(Vec3::NEG_Z * p.edge_float_slide, floor_hit.normal);
        let moved = self.safe_move(w, p, slide);
        if self.is_valid_landing_spot(w, p, &moved) {
            let floor = self.find_floor(w, p, None);
            if floor.walkable {
                self.floor = floor;
                self.adjust_floor_height(w, p);
                return true;
            }
        }
        (self.location, self.floor) = before;
        false
    }

    /// `AdjustFloorHeight` (0x1405744E0, stock): keeps the floor distance within 1.9 .. 2.4 cm
    /// with one sweep move of `2.15 - dist`.
    fn adjust_floor_height(&mut self, w: &dyn World, p: &SoraParams) {
        if !self.floor.walkable {
            return;
        }
        let mut old_floor_dist = self.floor.floor_dist;
        if self.floor.line_trace {
            if old_floor_dist < p.floor_dist_min && self.floor.line_dist >= p.floor_dist_min {
                // This would cause us to scale unwalkable walls.
                return;
            }
            old_floor_dist = self.floor.line_dist;
        }
        if old_floor_dist < p.floor_dist_min || old_floor_dist > p.floor_dist_max {
            let initial_z = self.location.z;
            let move_dist = p.floor_dist_target - old_floor_dist;
            let adjust = self.safe_move(w, p, Vec3::Z * move_dist);
            if !adjust.valid_blocking() {
                self.floor.floor_dist += move_dist;
            } else if move_dist > 0.0 {
                self.floor.floor_dist += self.location.z - initial_z;
            } else {
                let hit = adjust.hit.unwrap();
                self.floor.floor_dist = self.location.z - adjust.location.z;
                if is_walkable(p, &hit) {
                    self.floor.hit = Some(hit);
                    self.floor.hit_location = adjust.location;
                    self.floor.blocking_hit = true;
                    self.floor.walkable = true;
                    self.floor.line_trace = false;
                }
            }
        }
    }

    /// The landing snap of the Jump state's sub-step 10 (spec 4.6): a swept `SetActorLocation`
    /// to height `z`.
    pub fn sweep_to_height(&mut self, w: &dyn World, p: &SoraParams, z: f32) {
        self.move_updated(w, p, Vec3::Z * (z - self.location.z));
    }

    // ------------------------------------------------------------------ falling

    /// Magnitude of the gravity of the rising phase of a jump as the falling code applies it:
    /// |GetGravityZ| x m_JumpUpGravityScale (4116 cm/s^2 with the default GravityScale).
    pub fn rise_gravity(&self, p: &SoraParams) -> f32 {
        (self.gravity_z(p) * p.gravity_jump_up_scale).abs()
    }

    /// `GetGravityZ` (0x140582270, `same`). (`pub` for kh-party's NPC jump, movement vt+0x3B8.)
    pub fn gravity_z(&self, p: &SoraParams) -> f32 {
        p.gravity_world_z * self.gravity_scale * p.gravity_rate
    }

    /// The phase hook (slot 405 0x140577F30), spec 4.3: sees `Velocity.Z` from before the step.
    fn jump_phase_gravity(&mut self, p: &SoraParams, gravity: f32, step: f32) -> f32 {
        if !self.jump_duration_finished {
            if self.velocity.z > 0.0 {
                return gravity * p.gravity_jump_up_scale;
            }
            if self.jump_duration_timer < p.gravity_jump_duration_time {
                self.jump_duration_timer += step;
                if self.jump_duration_timer < p.gravity_jump_duration_time {
                    return gravity * p.gravity_jump_duration_scale;
                }
                self.jump_duration_finished = true;
            }
        }
        gravity
    }

    /// `NewFallVelocity` (0x14058BCA0): terminal speed 4000 (ASSUMED: the physics volume default).
    fn new_fall_velocity(&self, p: &SoraParams, velocity: Vec3, gravity_z: f32, dt: f32) -> Vec3 {
        let mut result = velocity + Vec3::Z * gravity_z * dt;
        let limit = p.gravity_terminal_speed.abs();
        if gravity_z != 0.0 {
            let direction = if gravity_z < 0.0 { Vec3::NEG_Z } else { Vec3::Z };
            let along = result.dot(direction);
            if along > limit {
                result += direction * (limit - along);
            }
        }
        result
    }

    /// `PhysFalling` (slot 222 0x14058D0C0), spec 4.4. The velocity is only what `CalcVelocity`
    /// and gravity make it: no deflection ever writes it (change C1).
    fn phys_falling(&mut self, w: &dyn World, p: &SoraParams, dt: f32, iterations: &mut u32) {
        if dt < MIN_TICK_TIME {
            return;
        }
        let mut fall_acceleration = Vec3::new(self.acceleration.x, self.acceleration.y, 0.0) * p.air_control;
        fall_acceleration = fall_acceleration.clamp_length_max(self.max_acceleration);
        let has_air_control = fall_acceleration.length_squared() > 0.0;
        let rm = self.root_motion_velocity;
        // Direct velocities written by a state (see `direct_xy`): taken as they are.
        let direct = self.direct_xy.take();
        if let Some(v) = direct {
            self.velocity.x = v.x;
            self.velocity.y = v.y;
        }
        if let Some(z) = self.direct_z.take() {
            self.velocity.z = z;
        }

        let mut remaining = dt;
        while remaining >= MIN_TICK_TIME && *iterations < p.sim_max_iterations as u32 {
            *iterations += 1;
            let step = self.simulation_time_step(p, remaining, *iterations);
            remaining -= step;
            let old_velocity = self.velocity;
            let flat = Vec3::new(self.velocity.x, self.velocity.y, 0.0);
            let mut no_air_control = self.velocity;
            if has_air_control {
                let v = self.calc_velocity(p, flat, Vec3::ZERO, step, p.air_lateral_friction, p.air_braking_deceleration);
                no_air_control = Vec3::new(v.x, v.y, old_velocity.z);
            }
            let v = if direct.is_some() { flat } else { self.calc_velocity(p, flat, fall_acceleration, step, p.air_lateral_friction, p.air_braking_deceleration) };
            self.velocity = Vec3::new(v.x, v.y, old_velocity.z);
            if !has_air_control {
                no_air_control = self.velocity;
            }
            let gravity = if self.gravity_off { 0.0 } else { self.gravity_z(p) };
            let gravity = self.jump_phase_gravity(p, gravity, step);
            self.velocity = self.new_fall_velocity(p, self.velocity, gravity, step);
            no_air_control = self.new_fall_velocity(p, no_air_control, gravity, step);
            let air_control_accel = (self.velocity - no_air_control) / step;

            // Midpoint, plus the root-motion velocity (0 unless a root-motion clip plays).
            let mut adjusted = ((old_velocity + self.velocity) * 0.5 + rm) * step;
            let mut hit = self.safe_move(w, p, adjusted);
            let mut sub = step * (1.0 - hit.time);
            if hit.blocking {
                if self.is_valid_landing_spot(w, p, &hit) {
                    self.process_landed(w, p, remaining + sub, iterations);
                    return;
                }
                adjusted = self.velocity * step;
                if has_air_control {
                    adjusted = (no_air_control + limit_air_control(air_control_accel, &hit) * step) * step;
                }
                let first = hit.hit.unwrap();
                let (old_normal, old_impact_normal) = (first.normal, first.impact_normal);
                let mut delta = self.compute_slide_vector(adjusted, 1.0 - hit.time, old_normal);
                if sub > KINDA_SMALL_NUMBER && delta.dot(adjusted) > 0.0 {
                    hit = self.safe_move(w, p, delta);
                    if hit.blocking {
                        let last = sub;
                        sub *= 1.0 - hit.time;
                        if self.is_valid_landing_spot(w, p, &hit) {
                            self.process_landed(w, p, remaining + sub, iterations);
                            return;
                        }
                        let second = hit.hit.unwrap();
                        if has_air_control && second.normal.z > VERTICAL_SLOPE_NORMAL_Z {
                            delta = self.compute_slide_vector(no_air_control * last, 1.0, old_normal);
                        }
                        let pre = delta;
                        delta = self.two_wall_adjust(p, delta, &hit, old_normal);
                        if has_air_control {
                            let a = limit_air_control(air_control_accel, &hit) * sub;
                            if a.dot(old_normal) > 0.0 {
                                delta += a * sub;
                            }
                        }
                        // The fork's block for two vertical walls (change C3, 0x14058DC80).
                        if second.normal.z.abs() < VERTICAL_SLOPE_NORMAL_Z && old_normal.z.abs() < VERTICAL_SLOPE_NORMAL_Z && adjusted.truncate().length_squared() > 0.0 && delta.truncate().dot(adjusted.truncate()) < 0.0 {
                            delta = Vec3::new(0.0, 0.0, pre.z);
                            self.velocity.x = 0.0;
                            self.velocity.y = 0.0;
                            delta = self.compute_slide_vector(delta, 1.0 - hit.time, second.normal) + second.normal * p.fall_two_wall_nudge;
                        }
                        let ditch = old_impact_normal.z > 0.0 && second.impact_normal.z > 0.0 && delta.z.abs() <= KINDA_SMALL_NUMBER && second.impact_normal.dot(old_impact_normal) < 0.0;
                        hit = self.safe_move(w, p, delta);
                        if hit.time == 0.0 {
                            if self.is_valid_landing_spot(w, p, &hit) {
                                self.process_landed(w, p, 0.0, iterations);
                                return;
                            }
                            // Side step.
                            let impact = hit.hit.map_or(Vec3::ZERO, |h| h.impact_normal);
                            let mut side = Vec2::new(old_normal.x + impact.x, old_normal.y + impact.y).normalize_or_zero();
                            if side == Vec2::ZERO {
                                side = Vec2::new(old_normal.y, -old_normal.x).normalize_or_zero();
                            }
                            hit = self.safe_move(w, p, Vec3::new(side.x, side.y, 0.0));
                        }
                        if ditch || self.is_valid_landing_spot(w, p, &hit) {
                            self.process_landed(w, p, 0.0, iterations);
                            return;
                        }
                        if hit.time == 0.0 {
                            // Unstick (change C4): the delta, then its 2D part; land only on a valid spot.
                            for attempt in [delta, Vec3::new(delta.x, delta.y, 0.0)] {
                                let moved = self.safe_move(w, p, attempt);
                                if self.is_valid_landing_spot(w, p, &moved) {
                                    self.process_landed(w, p, 0.0, iterations);
                                    return;
                                }
                            }
                            return;
                        }
                    }
                }
            }
            if self.velocity.x * self.velocity.x + self.velocity.y * self.velocity.y <= p.fall_xy_zero_threshold {
                self.velocity.x = 0.0;
                self.velocity.y = 0.0;
            }
        }
    }

    /// `IsValidLandingSpot` (0x140589960), spec 4.6: no lower-hemisphere and no edge-tolerance
    /// test; `FindFloor` with the falling distances (and its perch stage) must be walkable. The
    /// flag component+0x308 bit 0 is 0 (change C6).
    fn is_valid_landing_spot(&self, w: &dyn World, p: &SoraParams, hit: &Move) -> bool {
        let Some(h) = hit.hit else { return false };
        if !hit.blocking {
            return false;
        }
        if !h.start_penetrating {
            if !is_walkable(p, &h) {
                return false;
            }
        } else if h.normal.z < KINDA_SMALL_NUMBER {
            return false;
        }
        self.find_floor(w, p, Some(hit)).walkable
    }

    /// `ProcessLanded` (0x140591DC0) -> `SetPostLandedPhysics` (0x140594DA0): Walking, XY kept,
    /// the rest of the tick walked with the same iteration counter.
    fn process_landed(&mut self, w: &dyn World, p: &SoraParams, remaining: f32, iterations: &mut u32) {
        self.set_walking(w, p);
        self.start_new_physics(w, p, remaining, iterations);
    }

    // ------------------------------------------------------------------ moves and slides

    /// One sweep move of the body by `delta` (`TresRootComponent::MoveComponentImpl` 0x1405D27A0):
    /// stock pull-back on a blocking hit (`Time -= clamp(0.1/Dist ...) + 0.001`).
    fn move_updated(&mut self, w: &dyn World, p: &SoraParams, delta: Vec3) -> Move {
        let origin = self.location;
        // MIN_MOVEMENT_DIST_SIZE_SQ = (4 x KINDA_SMALL_NUMBER)^2
        if delta.length_squared() <= (4.0 * KINDA_SMALL_NUMBER) * (4.0 * KINDA_SMALL_NUMBER) {
            return Move { blocking: false, time: 1.0, hit: None, location: origin };
        }
        if self.world_collision_off {
            self.location += delta;
            return Move { blocking: false, time: 1.0, hit: None, location: self.location };
        }
        let centre = self.capsule_centre(p);
        match w.sweep_capsule(centre, centre + delta, p.capsule_radius, p.capsule_half_height) {
            None => {
                self.location += delta;
                Move { blocking: false, time: 1.0, hit: None, location: self.location }
            }
            Some(hit) if hit.start_penetrating => Move { blocking: true, time: 0.0, hit: Some(hit), location: origin },
            Some(hit) => {
                if self.last_block.is_none() {
                    self.last_block = Some((hit, self.velocity));
                }
                let distance = delta.length();
                let back = 0.1f32.clamp(0.1 / distance, 1.0 / distance) + 0.001;
                let time = (hit.time - back).clamp(0.0, 1.0);
                self.location += delta * time;
                Move { blocking: true, time, hit: Some(hit), location: self.location }
            }
        }
    }

    /// `SafeMoveUpdatedComponent` (0x14401E730): the move; when it starts in penetration, move
    /// out and move again (change B10).
    fn safe_move(&mut self, w: &dyn World, p: &SoraParams, delta: Vec3) -> Move {
        let result = self.move_updated(w, p, delta);
        if let (true, Some(hit)) = (result.start_penetrating(), result.hit) {
            self.location += penetration_adjustment(&hit);
            return self.move_updated(w, p, delta);
        }
        result
    }

    /// `SlideAlongSurface` (0x140596360 and the stock base). On ground only: a non-walkable
    /// surface that faces up is flattened to 2D (change B6). Returns the fraction applied.
    fn slide_along_surface(&mut self, w: &dyn World, p: &SoraParams, delta: Vec3, time: f32, in_normal: Vec3, hit: &mut Move) -> f32 {
        if !hit.blocking {
            return 0.0;
        }
        let mut normal = in_normal;
        if self.on_ground() && normal.z > 0.0 && !hit.hit.is_some_and(|h| is_walkable(p, &h)) {
            normal = Vec3::new(normal.x, normal.y, 0.0).normalize_or_zero();
        }
        let old_normal = normal;
        let mut slide = self.compute_slide_vector(delta, time, normal);
        if slide.dot(delta) > 0.0 {
            *hit = self.safe_move(w, p, slide);
            let first = hit.time;
            let mut applied = first;
            if hit.valid_blocking() {
                slide = self.two_wall_adjust(p, slide, hit, old_normal);
                if slide.abs().max_element() > 1.0e-3 && slide.dot(delta) > 0.0 {
                    *hit = self.safe_move(w, p, slide);
                    applied += hit.time * (1.0 - first);
                }
            }
            return applied.clamp(0.0, 1.0);
        }
        0.0
    }

    /// `ComputeSlideVector`: the plane projection, and no boosting up slopes while falling.
    fn compute_slide_vector(&self, delta: Vec3, time: f32, normal: Vec3) -> Vec3 {
        let result = plane_project(delta, normal) * time;
        if self.mode == MovementMode::Falling {
            return handle_slope_boosting(result, delta, time, normal);
        }
        result
    }

    /// `TwoWallAdjust` (0x140598B60): the stock base, then the fork's ground rule (change B7).
    fn two_wall_adjust(&self, p: &SoraParams, in_delta: Vec3, hit: &Move, old_normal: Vec3) -> Vec3 {
        let Some(h) = hit.hit else { return in_delta };
        let hit_normal = h.normal;
        let mut delta = in_delta;
        if old_normal.dot(hit_normal) <= 0.0 {
            // 90 or less corner: along the crease.
            let desired = delta;
            let direction = hit_normal.cross(old_normal).normalize_or_zero();
            delta = delta.dot(direction) * (1.0 - hit.time) * direction;
            if desired.dot(delta) < 0.0 {
                delta = -delta;
            }
        } else {
            let desired = delta;
            delta = self.compute_slide_vector(delta, 1.0 - hit.time, hit_normal);
            if delta.dot(desired) <= 0.0 {
                delta = Vec3::ZERO;
            } else if (hit_normal.dot(old_normal) - 1.0).abs() < KINDA_SMALL_NUMBER {
                delta += hit_normal * p.fall_two_wall_nudge;
            }
        }
        if self.on_ground() {
            if delta.z > 0.0 {
                if is_walkable(p, &h) && h.impact_normal.z > KINDA_SMALL_NUMBER {
                    if delta.z > p.floor_max_step_height {
                        delta *= p.floor_max_step_height / delta.z;
                    }
                } else {
                    delta.z = 0.0;
                }
            } else if delta.z < 0.0 && self.floor.floor_dist < p.floor_dist_min && self.floor.blocking_hit {
                delta.z = 0.0;
            }
        }
        delta
    }
}

/// `GetPenetrationAdjustment` as the fork has it (0x144016CA0, change B10).
fn penetration_adjustment(hit: &Hit) -> Vec3 {
    hit.normal * ((if hit.penetration_depth > 0.0 { hit.penetration_depth } else { 0.125 }) + PENETRATION_PULLBACK)
}

/// `ApplyVelocityBraking` (0x140577890, `same`): spec 3.3.
fn apply_velocity_braking(p: &SoraParams, velocity: Vec3, dt: f32, friction: f32, braking_deceleration: f32) -> Vec3 {
    let friction = (friction * p.ground_braking_friction_factor.max(0.0)).max(0.0);
    let braking_deceleration = braking_deceleration.max(0.0);
    if friction == 0.0 && braking_deceleration == 0.0 {
        return velocity;
    }
    let old = velocity;
    let mut v = velocity;
    let reverse = if braking_deceleration == 0.0 { Vec3::ZERO } else { -braking_deceleration * v.normalize_or_zero() };
    let mut remaining = dt;
    let max_step = p.move_braking_sub_step; // 1/33
    while remaining >= MIN_TICK_TIME {
        let step = if remaining > max_step && friction != 0.0 { max_step.min(remaining * 0.5) } else { remaining };
        remaining -= step;
        v += (-friction * v + reverse) * step;
        if v.dot(old) <= 0.0 {
            return Vec3::ZERO;
        }
    }
    let speed_sq = v.length_squared();
    if speed_sq <= KINDA_SMALL_NUMBER || (braking_deceleration != 0.0 && speed_sq <= 100.0) {
        return Vec3::ZERO;
    }
    v
}

/// `IsWalkable` (0x140589A60, stock): only `ImpactNormal` is read; 49 degrees.
pub(crate) fn is_walkable(p: &SoraParams, hit: &Hit) -> bool {
    !hit.start_penetrating && hit.impact_normal.z >= KINDA_SMALL_NUMBER && hit.impact_normal.z >= p.floor_walkable_z
}

/// `IsWithinEdgeTolerance` (0x140589BA0): lower bound `max(R - 0.15, 1e-4)` (change B14).
fn within_edge_tolerance(p: &SoraParams, capsule_location: Vec3, impact: Vec3, radius: f32) -> bool {
    let distance_sq = (impact - capsule_location).truncate().length_squared();
    let reduced = (radius - p.floor_sweep_edge_reject).max(KINDA_SMALL_NUMBER);
    distance_sq < reduced * reduced
}

/// `LimitAirControl` (both callers pass "do not re-check the landing spot").
fn limit_air_control(fall_acceleration: Vec3, hit: &Move) -> Vec3 {
    let Some(h) = hit.hit else { return fall_acceleration };
    if hit.valid_blocking() && h.normal.z > VERTICAL_SLOPE_NORMAL_Z {
        if fall_acceleration.dot(h.normal) < 0.0 {
            let flat = Vec3::new(h.normal.x, h.normal.y, 0.0).normalize_or_zero();
            return plane_project(fall_acceleration, flat);
        }
    } else if h.start_penetrating {
        return if fall_acceleration.dot(h.normal) > 0.0 { fall_acceleration } else { Vec3::ZERO };
    }
    fall_acceleration
}

/// `HandleSlopeBoosting` (0x1405877E0): in the "was heading down, would deflect up" case the
/// fork returns (0, 0, ZLimit) (change C5).
fn handle_slope_boosting(slide: Vec3, delta: Vec3, time: f32, normal: Vec3) -> Vec3 {
    let mut result = slide;
    if result.z > 0.0 {
        let z_limit = delta.z * time;
        if result.z - z_limit > KINDA_SMALL_NUMBER {
            if z_limit > 0.0 {
                result *= z_limit / result.z;
            } else {
                result = Vec3::new(0.0, 0.0, z_limit);
            }
            let remainder = (slide - result) * Vec3::new(1.0, 1.0, 0.0);
            let flat = Vec3::new(normal.x, normal.y, 0.0).normalize_or_zero();
            result += plane_project(remainder, flat);
        }
    }
    result
}

/// `FVector::VectorPlaneProject`
fn plane_project(v: Vec3, normal: Vec3) -> Vec3 {
    v - normal * v.dot(normal)
}
