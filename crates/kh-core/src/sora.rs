//! The host-facing API and the order of one frame (docs/sora-movement-spec.md 1.2, read at
//! instruction level):
//!
//! ```text
//! C(N)  controller: the Sora input update (jump key, then the stick)
//! M(N)  movement component tick:
//!         a. consume the input vector the state wrote in frame N-1
//!         b. CheckJumpInput (sees the latch the launch notify set in M(N-1))
//!         c. acceleration, analog modifier
//!         d. POSE TICK: animation clock, root motion, all notifies -> their handlers
//!         f. PerformMovement: walking / falling; root-motion rotation
//! S(N)  pawn tick -> state queue MainProc: End / Begin / Tick of the state
//! ```
//!
//! So a state's writes act one frame later, and the state sees a landing in the same frame.
//!
//! REFERENCE FRAME RATE 60 fps. The game has no fixed step: C, M, the animation and S all get
//! the frame's dt (at most 0.05 s, `MaxDeltaTime`). The port is driven with a fixed dt of 1/60 s
//! per step whatever the display rate (the host interpolates what it draws), which is the game
//! as it runs at 60 fps; `kh-replay` drives this same core with the per-frame dt recorded from
//! the running game, for comparison against captures. A dt above 0.05 is clamped here.

use glam::{Vec2, Vec3};

use crate::anim::{AnimId, AnimPlayer, AnimRequest};
use crate::cmc::Cmc;
use crate::controller;
use crate::host::{EffectHandle, EffectSpawn, Event, HitRecord, Host, NullHost, SoundHandle, SoundPlay};
use crate::input::{PadButton, PadState};
use crate::params::SoraParams;
use crate::pawn::{Ctx, Pawn};
use crate::states::{self, StateData, StateFlags, StateId};
use crate::status::requests::{self, StatusRequest};
use crate::status::Status;
use crate::ui_model::{HudState, UiPopup, UiSnapshot};
use crate::world::World;

/// Where Sora is put when he is created or teleported.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    /// The ACTOR ORIGIN, cm: at foot level (the capsule's bottom is 2 cm above it, its centre 91).
    pub position: Vec3,
    /// Facing, degrees about Z from +X toward +Y.
    pub yaw: f32,
}

/// The player's intent for one fixed step: what the game's controller polls (spec section 2).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Input {
    /// Left stick, RAW axis values as the pad reports them, each -1..1: `x` = LeftX (right),
    /// `y` = LeftY (forward, away from the camera). The game's own dead zone (0.25 per axis,
    /// DefaultInput.ini) is applied by the core; the host must not apply another.
    pub move_stick: Vec2,
    /// True on the one step in which the jump button went down (`WasInputKeyJustPressed`). The
    /// host latches a press until a step consumes it.
    pub jump_pressed: bool,
    /// True while the jump button is down.
    pub jump_held: bool,
    /// The camera's yaw in degrees, same convention as Sora's (0 looks along +X, 90 along +Y).
    pub camera_yaw: f32,
    /// Every button of the pad (input.rs: held, just pressed, just released), for the states
    /// beyond stage 1. The jump is NOT read from here yet: fill `jump_pressed` / `jump_held` as
    /// well, or build the input with [`Input::with_pad`].
    pub pad: PadState,
    /// The view of the last drawn frame (camera position, axes, field of view, view rectangle):
    /// the shotlock reticle test projects targets with it (magic spec F A4.2). `None`: no
    /// camera, nothing can be locked. Added by the magic implementer.
    pub view: Option<crate::magic::View>,
}

impl Input {
    /// This input with the pad's buttons, and the jump taken from the jump button of the game's
    /// default layout (`PadButton::JUMP`: the right face button).
    pub fn with_pad(mut self, pad: PadState) -> Input {
        self.pad = pad;
        self.jump_pressed = pad.pressed.has(PadButton::JUMP);
        self.jump_held = pad.held.has(PadButton::JUMP);
        self
    }
}

/// (integration pass) The player states the field camera has a branch for (`Sora::camera_state`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum CameraState {
    #[default]
    None,
    /// Player state 8 with `WallRun+0xA8`: 0 up, 1 along toward his left, 2 the mirror.
    WallRun(u8),
    RailSlide,
    SlopeSlide,
}

/// A coarse, readable name for what he is doing, derived from the state and its sub-step
/// (for traces and debug text; the simulation itself runs on [`Output::state`] and
/// [`Output::sub_step`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MovementState {
    Idle,
    Walk,
    Run,
    Sprint,
    /// Run sub-step 1 / 2: `RunStartN` (from idle, or the turn-around).
    RunStart,
    /// Run sub-step 4: `RunEnd` / `WalkEnd`.
    RunStop,
    /// Jump sub-step 0: `JumpNormalStart`, still on the ground.
    JumpStart,
    /// Jump sub-steps 1 and 2.
    JumpRise,
    /// Jump sub-steps 3 and 4.
    Fall,
    /// Jump sub-steps 10 and 11.
    Land,
    /// A state outside this movement summary (everything beyond stage 1): see `Output::state`.
    Other,
}

impl MovementState {
    pub const fn name(self) -> &'static str {
        match self {
            MovementState::Idle => "Idle",
            MovementState::Walk => "Walk",
            MovementState::Run => "Run",
            MovementState::Sprint => "Sprint",
            MovementState::RunStart => "RunStart",
            MovementState::RunStop => "RunStop",
            MovementState::JumpStart => "JumpStart",
            MovementState::JumpRise => "JumpRise",
            MovementState::Fall => "Fall",
            MovementState::Land => "Land",
            MovementState::Other => "Other",
        }
    }
}

/// The result of one fixed step. Not `Copy` since round 4 (it carries the step's event list):
/// `clone()` it to keep two.
#[derive(Clone, Debug, PartialEq)]
pub struct Output {
    /// The actor origin, cm: where the model's origin goes (foot level).
    pub position: Vec3,
    /// Centre of the collision capsule, cm (`position` + 91 up).
    pub capsule_center: Vec3,
    /// Facing, degrees, in (-180, 180].
    pub yaw: f32,
    /// The movement component's velocity, cm/s. During `RunStartN` the displacement comes from
    /// the animation's root motion and is not in here.
    pub velocity: Vec3,
    /// `ETresStateID` of the current state: Idle 1, Run 2, Jump 3 ...
    pub state: StateId,
    /// The state's `m_nSubStep`.
    pub sub_step: u8,
    pub movement_state: MovementState,
    /// Movement mode Walking (the game's "on ground").
    pub on_floor: bool,
    /// The displacement of this step over its dt (`MyMovement + 0x9A0`): what the game's camera
    /// reads as the pawn's velocity. It includes root motion and is 0 against a wall.
    pub displacement_velocity: Vec3,
    /// The `FullBody` slot at the end of the frame: what this frame's pose evaluation shows. A
    /// play call made by this frame's state tick is already in it (weight 0: the slot shows the
    /// saved pose until the next frame's pose tick).
    pub anim: AnimRequest,
    /// The slot as this frame's pose tick left it, BEFORE the state tick: whose notifies fired
    /// in this frame and in which window. Equal to `anim` unless the state played something.
    pub anim_ticked: AnimRequest,
    /// Steps taken so far, this one included: the first step returns 1.
    pub step: u64,
    /// Everything this step told the host, in the order it happened: state changes, sounds,
    /// effects, hits, popups (host/events.rs). Empty in most steps.
    pub events: Vec<Event>,
}

impl Output {
    /// The transform to draw between two consecutive outputs: `alpha` 0 is `previous`, 1 is
    /// `self`. Yaw takes the short way round. Returns `(position, yaw)`.
    pub fn lerp(&self, previous: &Output, alpha: f32) -> (Vec3, f32) {
        let turn = wrap_degrees(self.yaw - previous.yaw);
        (previous.position.lerp(self.position, alpha), wrap_degrees(previous.yaw + turn * alpha))
    }
}

/// PORT OPTIONS: deliberate departures from the original, decided by the user. They are not
/// values read from the game (so they are not in the generated `SoraParams`). With every option
/// off (`Tuning::GAME_EXACT`) the core follows the game's code as read, bit for bit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tuning {
    /// USER DECISION 2026-10-11 ("when I spin the stick around I don't want Sora to stop running,
    /// he should do his lurch animation only when the stick isn't pushed anymore"; after
    /// playing it: "his animation for changing directions should still play when you flick the
    /// stick into the opposite direction, also his lurch animation isn't playing when you first
    /// move from standstill"). While the stick is rolled round, the run never stops to replay
    /// `RunStartN`:
    ///
    /// - the turn-around of the run loop (|target yaw - facing| >= 75 degrees: `StopMovement`,
    ///   `RunStartN`; 0x1413AF8E0) fires only for a FLICK to the opposite direction: when the
    ///   target yaw has also swung by `reversal_angle` or more within `reversal_window` ticks.
    ///   Otherwise he keeps running, the input keeps being written every tick and the facing
    ///   keeps easing with the same `Turn(target, 0.1 x dt x 60)` as below 75 degrees;
    /// - a run request after a walk request inside the Run state (`TryRun`: sub-step 1) goes
    ///   straight on in the loop, with the run animation, without `RunStartN` - EXCEPT as the
    ///   start from standstill: in a Run state entered from Idle that has had no run tick yet
    ///   and is younger than `start_lunge_window`, the game's path (`RunStartN`) is kept. (A
    ///   real stick takes a few frames from the centre to the rim: the state begins with walk
    ///   requests, for which the game plays no run start, and this path IS its start lunge.)
    ///
    /// Unchanged: Idle -> Run with a run request plays `RunStartN`; so does a run request during
    /// the stop clip (the stick was released and the stop had begun); a centred stick is "not
    /// pushed" and the original stop logic applies to it as it is. The original rules:
    /// docs/sora-movement-spec-turning.md.
    pub continuous_turning: bool,
    /// With `continuous_turning`: how far the stick's target yaw must have swung, degrees, within
    /// `reversal_window` ticks for the turn-around to play. With 135 in 5 ticks a stick rolled
    /// round the rim stays lunge-free up to 135 x 60 / 5 = 1620 degrees a second (4.5 turns a
    /// second). Port values chosen with the user.
    pub reversal_angle: f32,
    /// Ticks (frames of the reference 60 a second) the swing is measured over; at most 15.
    pub reversal_window: u32,
    /// With `continuous_turning`: seconds after a Run state began from Idle during which the
    /// first run request after its walk requests still plays the start `RunStartN`.
    pub start_lunge_window: f32,
    /// USER DECISION 2026-10-11 ("is there any way to make the transition from walk to run
    /// smooth?" - "do the speed ramp"). In the game the gait is binary: walk 140 cm/s below 0.8
    /// of stick, run 680, and the limit jumps when the gait switches. With this option, while
    /// he stays in the Run state's loop, the ground speed limit moves linearly between the two
    /// gaits' limits over `speed_ramp_time` (either direction; a switch back mid-ramp ramps back
    /// from where it is). Not ramped: the start from standstill, the stop, the turn-around,
    /// jumps and landings, and run -> sprint. Acceleration, the 0.2 s run grace, the sprint timer
    /// (reset by any walk tick) and the WalkLoopN <-> RunLoopN swap with its 0.2 s blend-in are
    /// the game's; the two cycles are not speed-matched, so the feet slide during the ramp.
    pub speed_ramp: bool,
    /// Seconds for the whole 140 <-> 680 ramp. A port value chosen with the user.
    pub speed_ramp_time: f32,
    /// USER DECISION 2026-10-11 ("I want speed matched animation"). In the game the loop is
    /// either WalkLoopN or RunLoopN at play rate 1.0, swapped with a 0.2 s blend-in. With this
    /// option, in the Run state's loop the slot plays ONE continuous locomotion loop: the walk
    /// cycle and the run blend space (with its lean and filter) on one shared phase, blended by
    /// his actual speed between the walk and the run speed, the phase advancing at speed /
    /// stride so that the feet match the ground (`AnimRequest::locomotion`). No play call when
    /// the gait changes. Notifies (reverse leg for the stop foot, and in the pose host the
    /// leg-IK windows and face plays) are the dominant side's. Everything outside the loop
    /// (run start, stop, turn-around, jump, landing, idle) is the game's playback.
    pub speed_matched_anim: bool,
    /// With `speed_matched_anim`: also scale the run cycle's rate above the run speed (sprint
    /// 980). Off: the sprint plays the run loop at rate 1.0 as in the game.
    pub speed_match_sprint: bool,
    /// USER DECISION 2026-10-11 ("Sora should stop gaining height in his jump as soon as I let go
    /// of the jump button", then after playing a hard cut: "make the jump a bit higher after you
    /// let go of B before he falls", then "make the minimum jump height just high enough so he
    /// can land on the orange block"). At the start of every movement tick in which he is in
    /// the air in the Jump state after the launch (sub-step 1 or 2) and the jump button is not
    /// held: `Velocity.Z = min(Velocity.Z, cap)` with
    /// `cap = max(release_rise_speed, speed still needed to reach min_jump_height)` (see
    /// `min_jump_height`). From there the game's own code runs:
    /// the rest of the rise under the normal rise gravity (-4116 cm/s^2), then the apex rule, the
    /// hang and the fall. With this option on, the game's own short-hop rule (GravityScale 3.0
    /// from the first rise-loop tick with the button up) is NOT applied: this option replaces it.
    ///
    /// The launch tick itself is not clamped (the test is made before `CheckJumpInput` launches),
    /// so a jump whose button was let go before the launch is one tick of full rise plus what
    /// `release_rise_speed` leaves: the smallest hop.
    ///
    /// The game's rule, in force when this is off: a release only counts from the first
    /// rise-loop state tick and switches `GravityScale` to 3.0 until the apex (movement spec 4.2).
    pub jump_cut_on_release: bool,
    /// With `jump_cut_on_release`: the upward speed left after the release, cm/s. 400 leaves
    /// 400^2 / (2 x 4116) = about 19 cm of rise over about 0.1 s; 0 is a hard cut. A port value
    /// chosen with the user, not read from the game.
    pub release_rise_speed: f32,
    /// With `jump_cut_on_release`: a jump always reaches at least this height above where it
    /// took off, cm, however early the button is let go; 0 disables. 110 = the test level's 1 m
    /// (orange) box plus 10 cm, so that his feet clear its top. The speed needed is
    /// `sqrt(2 x g x (min_jump_height - risen + g dt^2 / 8))`, `g` = the rise gravity the movement
    /// code applies (4116 cm/s^2), recomputed every tick while the button is up, so the cap
    /// shrinks as he rises and the apex converges on the minimum. The falling code integrates
    /// with the mean of the old and new velocity, which is exact for a constant gravity at the
    /// tick ends, but the true apex falls between two ticks: the highest SAMPLED height is up to
    /// `g dt^2 / 8` (0.14 cm at 60 Hz) under it, hence that term: the sampled apex is not below
    /// `min_jump_height` and at most 0.14 cm above it. A port value chosen with the user.
    pub min_jump_height: f32,
    /// NOT a user decision yet: a switch offered for one. By the game's code as read (actions
    /// spec II.2 "What actually ends a glide") NOTHING ends a glide but releasing the button:
    /// carried onto rising ground he keeps the Glide state and skims the floor at 1500 cm/s
    /// while the button is held. That literal reading is what runs with this `false` (the
    /// default, in `GAME_EXACT` and `USER` alike). `true`: the glide ends on ground contact
    /// (a port rule, not read from the game). To be confirmed against the running game.
    pub glide_ends_on_ground: bool,
    /// USER DECISION 2026-10-12 ("the keyblade doesn't appear immediately with your first attack
    /// which it should"). The game's Attack Begin (0x141333210) calls `Draw(1, 1, 1.0)`, the same
    /// call as the stance rules: the appear effect at once and the mesh 0.22 s later with a
    /// 0.06 s ramp (docs/sora-vfx-spec.md 2.4), so by the code as read the first swing starts
    /// with an empty hand. With this option the first attack of a chain asks the host for the
    /// mesh AT ONCE (`EventKind::WeaponVisible { instant: true }`), also when an appear
    /// sequence is still running; the appear effect plays as before. Off: the game's sequence.
    pub weapon_instant_on_attack: bool,
    /// USER DECISION 2026-10-12 ("you can walk up the slide when you should slide down it by
    /// trying to walk up it"). The game's start probe (0x14063D9C0) wants him to FACE DOWNHILL
    /// (within 50 degrees of the fall line; 80 on a forced floor) and, standing or walking, the
    /// latch of an earlier slide: a flagged slope can be walked up. With this option a flagged
    /// floor that is steep enough starts the slide from Idle, Run and a landing whatever he
    /// faces and whatever the latch says; he is turned to the fall line when the slide begins
    /// and the stick can steer at most `slide_steer_cone` degrees off it. Off: the game's rule.
    pub slide_any_direction: bool,
    /// With `slide_any_direction`: how far off the fall line the stick may steer, degrees. A
    /// port value (the game's start cone is 50).
    pub slide_steer_cone: f32,
}

impl Tuning {
    /// The game's code as read: every port option off. What comparisons with captures of the
    /// real game must use.
    pub const GAME_EXACT: Tuning = Tuning { continuous_turning: false, reversal_angle: 135.0, reversal_window: 5, start_lunge_window: 0.30, speed_ramp: false, speed_ramp_time: 0.25, speed_matched_anim: false, speed_match_sprint: false, jump_cut_on_release: false, release_rise_speed: 400.0, min_jump_height: 110.0, glide_ends_on_ground: false, weapon_instant_on_attack: false, slide_any_direction: false, slide_steer_cone: 50.0 };
}

impl Tuning {
    /// The user's choices: every option on. The default.
    pub const USER: Tuning = Tuning { continuous_turning: true, reversal_angle: 135.0, reversal_window: 5, start_lunge_window: 0.30, speed_ramp: true, speed_ramp_time: 0.25, speed_matched_anim: true, speed_match_sprint: false, jump_cut_on_release: true, release_rise_speed: 400.0, min_jump_height: 110.0, glide_ends_on_ground: false, weapon_instant_on_attack: true, slide_any_direction: true, slide_steer_cone: 50.0 };
    /// The game's rules with only the turning option on (for tests of that option alone).
    pub const GAME_EXACT_WITH_TURNING: Tuning = Tuning { continuous_turning: true, jump_cut_on_release: true, ..Tuning::GAME_EXACT };
}

impl Default for Tuning {
    /// The user's choices.
    fn default() -> Tuning {
        Tuning::USER
    }
}

/// Something to push through the host seam at the start of the next step, WITHOUT any gameplay
/// rule behind it: a hook for tests and for a host's debug keys (the test level's script verbs
/// `emit-sound`, `emit-effect`, `emit-hit`, `emit-popup`). The states do not use this; they call
/// the same seam functions directly.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DebugEmit {
    Sound(SoundPlay),
    StopSound(SoundHandle, f32),
    Effect(EffectSpawn),
    StopEffect(EffectHandle, bool),
    Hit(HitRecord),
    Popup(UiPopup),
}

/// Sora: the pawn (movement component, animation slot, state queue, status) and its running
/// state. Plain data: cloning it is a save state.
#[derive(Clone, Debug)]
pub struct Sora {
    pub(crate) pawn: Pawn,
    /// The running state (`TresStateQueueComponent`'s current state).
    pub(crate) state: StateData,
    /// Debug emissions waiting for the next step.
    debug: Vec<DebugEmit>,
    /// Status requests waiting for the next step (`Sora::status_request`: EXP awards, damage,
    /// heals, items ... from the host or other systems).
    requests: Vec<StatusRequest>,
    /// The HUD model's own state: the red lag gauges, the popups on screen (ui_model/).
    hud: HudState,
}

impl Sora {
    /// Creates him at `pose`, in the Idle state. Nothing is queried until the first `step`,
    /// which finds the floor (or starts him falling when there is none under him). His status is
    /// the explicitly empty placeholder loadout until `set_status` gives him one.
    pub fn new(params: SoraParams, pose: Pose) -> Sora {
        // The queue's default state begins (TSID_STATE_IDLE): AnimPlayer::new played the idle animation.
        let anim = AnimPlayer::new(AnimId::IdleN);
        Sora { pawn: Pawn::new(params, Cmc::new(&params, pose.position), wrap_degrees(pose.yaw), anim), state: StateData::DEFAULT, debug: Vec::new(), requests: Vec::new(), hud: HudState::default() }
    }

    /// One frame with a host that has only a world: no targets, no sound, no effects, no UI
    /// (`NullHost`). The step's events are still in `Output::events`. See [`Sora::step_hosted`].
    pub fn step(&mut self, dt: f32, input: &Input, world: &dyn World) -> Output {
        let mut null = NullHost::default();
        let mut host = null.host(world);
        self.step_hosted(dt, input, &mut host)
    }

    /// Advances one frame of `dt` seconds (the reference is a fixed 1/60; never more than 0.05,
    /// the game's `MaxDeltaTime`) and returns the new state. `input.camera_yaw` must be the yaw
    /// the camera had at the end of the PREVIOUS frame. During the step the core asks
    /// `host.world` and `host.targets` and tells `host.audio`, `host.effects`; at its end it
    /// publishes one snapshot to `host.ui`. Everything it told the host is in `Output::events`.
    pub fn step_hosted(&mut self, dt: f32, input: &Input, host: &mut Host<'_>) -> Output {
        let Sora { pawn, state, debug, requests, hud } = self;
        let p = *pawn.params();
        let dt = dt.min(p.frame_max_delta_time);
        // (magic implementer) The SQEX per-object-type time dilation (magic spec F A6): Sora's own
        // step runs at the PLAYER type's rate, the controller's timers at the world's (the
        // controller is type VEHICLE); projectiles, the HUD and the camera keep the real dt.
        // Both rates are exactly 1.0 outside the aim mode, the airstep and the menu's slow option.
        let real_dt = dt;
        // (combat implementer, 2026-10-12) The actor's own `CustomTimeDilation` on top of it: the bad
        // statuses STOP (0.1) and HONEY slow Sora himself (combat spec 9.9.1); 1.0 otherwise.
        let (dt, dt_controller) = (real_dt * pawn.time_dilation(crate::host::time::OBJ_PLAYER) * pawn.combat.bad.dilation, real_dt * pawn.time_dilation(crate::magic::OBJ_CONTROLLER));
        pawn.real_dt = real_dt;
        let world = host.world;
        pawn.step += 1;
        pawn.events.clear();
        pawn.popups.clear();
        pawn.input = *input;
        let mut cx = Ctx::new(pawn, host);
        // Debug emissions asked for since the last step (not gameplay: see `DebugEmit`).
        for emit in debug.drain(..) {
            match emit {
                DebugEmit::Sound(play) => {
                    cx.play_sound(play);
                }
                DebugEmit::StopSound(handle, fade_out) => cx.stop_sound(handle, fade_out),
                DebugEmit::Effect(spawn) => {
                    cx.spawn_effect(spawn);
                }
                DebugEmit::StopEffect(handle, immediate) => cx.stop_effect(handle, immediate),
                DebugEmit::Hit(record) => {
                    cx.deliver_hit(&record);
                }
                DebugEmit::Popup(popup) => cx.popup(popup),
            }
        }
        // Status requests asked for since the last step (status/requests.rs): they change the
        // status and raise the HUD popups through the seam.
        for request in requests.drain(..) {
            requests::apply(&mut cx, request);
        }
        // PORT ADDITION (IMPL_ER44): the host's hold on the Keyblade.
        crate::actions::host_hold(&mut cx);
        // C(N): the Sora input update (0x1413F1AB0), spec 2.
        controller::controller_update(state, &mut cx, input, dt_controller);
        // pawn+0xE60 bit 17, the latch of a command that armed the command lock in the UI phase:
        // cleared by every pawn tick, after it made this frame's requests refuse (strand A 1.3).
        cx.pawn.magic.command_latch = false;
        // Hits on him the host delivered since the last step (combat strand E: `TakeDamage`,
        // the reaction gate, the Damage / Die states). AFTER the controller's requests: the gate
        // clears the queue and pushes DAMAGE, and a request of the same frame must not undo it
        // (in the game the hit comes from another actor's tick, whose place in the frame is not
        // known: strand C open question 1).
        // (DON-KIT) the Aero aura's added invincible time runs down (none without the aura)
        if cx.pawn.aura_ghost > 0.0 {
            cx.pawn.aura_ghost = (cx.pawn.aura_ghost - dt).max(0.0);
        }
        for hit in std::mem::take(&mut cx.pawn.combat.incoming) {
            // ACTIONS (spec II.1 "Body-collision reaction types"): what the hit meets first: his
            // reaction type 11 drops it (Dodge Roll, Air Slide), his guard guards it
            // (`OnGuardedHit`). COMBAT IMPLEMENTER: once `take_hit` classifies the hit itself
            // (`combat::guard::classify_on_sora` / `guarded`), DELETE this call: it is the
            // stand-in until then (a hit it lets through is classified again harmlessly).
            // (2026-10-11: `take_hit` classifies the hit itself now - `combat::guard::
            // classify_on_sora`, the full table - and hands a guarded one to `OnGuardedHit`: the
            // actions side's stand-in `actions::resolve_incoming` is no longer called.)
            states::damage::take_hit(state, &mut cx, &hit);
        }
        // PORT OPTION `Tuning::jump_cut_on_release` (a user decision, not the game's code): letting
        // go of the button while rising from a jump caps the upward speed.
        if cx.pawn.tuning.jump_cut_on_release && !input.jump_held && !cx.pawn.cmc.on_ground() && states::jump::rising_from_a_jump(state) {
            let this = &mut *cx.pawn;
            let gravity = this.cmc.rise_gravity(&p);
            let risen = this.cmc.location.z - this.cmc.launch_z;
            let missing = (this.tuning.min_jump_height - risen).max(0.0);
            // Half a tick of bias, so that the highest sampled height is not under the minimum.
            let needed = if this.tuning.min_jump_height > 0.0 && missing > 0.0 { (2.0 * gravity * (missing + gravity * dt * dt / 8.0)).sqrt() } else { 0.0 };
            let cap = this.tuning.release_rise_speed.max(0.0).max(needed);
            this.cmc.velocity.z = this.cmc.velocity.z.min(cap);
        }
        // M(N): the movement component tick (0x140598190).
        cx.pawn.cmc.tick_head(world, &p, dt);
        // d. The pose tick (0x140595730), every frame, in every movement mode: clock, root
        //    motion, notifies -> handlers (the launch latch is seen by CheckJumpInput next frame).
        // (combat implementer, 2026-10-12) Frozen (the bad status FREEZE, pawn+0x868 bit 1): the mesh's
        // animation is paused (pawn vt+0xC78(1)): the pose clock stands still.
        // (DON-KIT) the Aero aura's recovery: his ground hit reaction / get-up clip runs faster (x 1 at no aura)
        let recovery = aura_recovery_rate(state, cx.pawn);
        let tick = cx.pawn.anim.pose_tick(if cx.pawn.combat.bad.frozen { 0.0 } else { dt * recovery });
        cx.pawn.anim_ticked = cx.pawn.anim.request();
        states::on_notifies(state, &mut cx, &tick.notifies);
        // The pawn's own handlers: `TresLandAnimPose`, `TresLastAnimPose` (actions.rs).
        crate::actions::on_notifies(&mut cx, &tick.notifies);
        // Hit windows open and close (combat/hit.rs; the state's pre-processor has just run).
        crate::combat::hit::on_notifies(&mut cx, &tick.notifies);
        // The generic handlers of the same notifies: sounds, voices, effects (notify.rs).
        crate::notify::dispatch(&mut cx, &tick.notifies);
        // The root motion of this update, from the actor's frame into the world (it is consumed
        // by this same frame's move; its rotation is applied after the physics).
        let this = &mut *cx.pawn;
        let (scale_xy, scale_z) = this.root_scale;
        let root_motion = tick.root_motion.map(|(x, y, z, yaw)| {
            let (sin, cos) = this.yaw.to_radians().sin_cos();
            // (The scale is 1 unless an attack set it: combat spec 6.3, root-scale channel.)
            let (x, y, z) = if scale_xy != 1.0 || scale_z != 1.0 { (x * scale_xy, y * scale_xy, z * scale_z) } else { (x, y, z) };
            (Vec3::new(x * cos - y * sin, x * sin + y * cos, z), yaw)
        });
        let mut yaw = this.yaw;
        this.cmc.perform_movement(world, &p, dt, root_motion, &mut yaw);
        this.yaw = wrap_degrees(yaw);
        // The pawn's `MoveBlockedBy` -> the running state's move-blocked handler (actions spec
        // II.2: Air Slide and Superslide try a wall kick).
        crate::actions::move_blocked(state, &mut cx);
        // The soft body push between push bodies (movement slot 414; combat spec 6.5,
        // combat/push.rs): zero unless a target's push body overlaps his.
        let push = crate::combat::push::compute(&cx);
        cx.pawn.combat.pushed = push;
        if push != Vec3::ZERO {
            cx.pawn.cmc.push_move(world, &p, push, dt);
        }
        // (combat implementer, fourth pass 2026-10-12: the attack-collision manager's tick MOVED from
        // here to after MainProc, see below.)
        // (magic implementer) The spell actors and shotlock projectiles (object type PROJECTILE:
        // real time), the Aero jump pads, and the queued item effects (on the pawn's clock).
        crate::magic::projectile::tick(&mut cx, real_dt);
        crate::magic::pads_tick(state, &mut cx);
        crate::magic::items::tick(&mut cx, dt);
        // The status part of the pawn tick (docs/sora-stats-spec.md 6.4, magic spec C 3): walk
        // regeneration and the Withstand Combo timer (0x1405AF5B0), then the MP charge
        // (0x1405A8080). Its place against MainProc inside the pawn tick was not read: before it.
        let speed_squared = cx.pawn.cmc.velocity.length_squared();
        cx.pawn.status.tick(dt, speed_squared);
        // (combat implementer, 2026-10-12) The bad status on him: its timer, the stun trigger, the
        // max-HP cap of HP_BREAK (pawn tick 0x1405BC470 -> 0x1405AFF90; combat spec 9.9.1).
        crate::combat::badstat::tick(state, &mut cx, dt, real_dt);
        // S(N): the pawn tick -> state queue MainProc (0x14150A7E0), last.
        let main_dt = dt * aura_recovery_rate(state, cx.pawn); // (DON-KIT) x 1 without the aura
        states::main_proc(state, &mut cx, main_dt);
        // (combat implementer, fourth pass 2026-10-12; combat spec 8.2.2) The attack-collision
        // manager's tick: the active hit windows sweep and hit. `ATresAtkCollManager` ticks in group
        // 8 = `TG_SQEX_PostUpdateTransform` of this engine fork (`ETickingGroup`: ... PrePhysics 3 ...
        // PostPhysics 7, SQEX_PostUpdateTransform 8; its constructor 0x14062EC80 writes the byte),
        // the pawn in the stock `TG_PrePhysics` (no constructor of the pawn chain writes its group):
        // a window opened by a notify of M(N) sweeps in frame N, AFTER the pawn's whole tick
        // (controller, movement, MainProc). The state sees a landed hit in its next Tick.
        crate::combat::hit::tick(&mut cx, dt);
        // U(N): the UI tick, after the state tick: the command menu's attack decide (strand A 1.1).
        controller::ui_attack_request(state, &mut cx, input, dt);
        // The HUD after this step.
        hud.step(real_dt, &cx.pawn.status, &cx.pawn.popups);
        let mut snapshot = hud.snapshot(cx.pawn.step, &cx.pawn.status, cx.pawn.popups.clone());
        // The command menu (command/mod.rs) and the time dilation in force after this step.
        snapshot.command = crate::command::snapshot(&mut cx);
        cx.pawn.command_shown = snapshot.command.clone();
        // The combat side's part of the HUD: the lock-on target, the situation commands, the form.
        snapshot.lock_on = if cx.pawn.combat.lockon.locked { cx.pawn.combat.lockon.current } else { crate::host::TargetId::NONE };
        snapshot.situation = crate::combat::situation::hud(cx.pawn);
        // (integration pass) The soft target, the push-button prompt, the shotlock aim.
        crate::ui_model::fill(&mut snapshot, cx.pawn, state);
        cx.host.ui.publish(&snapshot);
        self.output()
    }

    /// The current state without stepping (what the last `step` returned).
    pub fn output(&self) -> Output {
        let pawn = &self.pawn;
        let p = pawn.params();
        Output {
            position: pawn.cmc.location,
            capsule_center: pawn.cmc.capsule_centre(p),
            yaw: pawn.yaw,
            velocity: pawn.cmc.velocity,
            state: self.state.id(),
            sub_step: self.state.sub_step(),
            movement_state: self.state.movement_state(pawn),
            on_floor: pawn.cmc.on_ground(),
            displacement_velocity: pawn.cmc.displacement_velocity,
            anim: pawn.anim.request(),
            anim_ticked: pawn.anim_ticked,
            step: pawn.step,
            events: pawn.events.clone(),
        }
    }

    /// Puts him somewhere else, standing still in Idle (the next `step` finds the floor). As
    /// before round 4 this is a new Sora (the port options go back to their default); his status
    /// and the step and handle counters carry over.
    pub fn teleport(&mut self, pose: Pose) {
        let mut fresh = Sora::new(*self.pawn.params(), pose);
        fresh.pawn.status = std::mem::take(&mut self.pawn.status);
        fresh.pawn.carry_counters(&self.pawn);
        // (magic implementer) The pouch, the projectiles in flight and the menu carry over.
        fresh.pawn.magic = std::mem::take(&mut self.pawn.magic);
        fresh.pawn.command = std::mem::take(&mut self.pawn.command);
        fresh.requests = std::mem::take(&mut self.requests);
        fresh.hud = std::mem::take(&mut self.hud);
        *self = fresh;
    }

    /// The port options in force (the default is the user's choices).
    pub fn tuning(&self) -> Tuning {
        self.pawn.tuning
    }

    /// Sets the port options; `Tuning::GAME_EXACT` for the game's code as read.
    pub fn set_tuning(&mut self, tuning: Tuning) {
        self.pawn.tuning = tuning;
    }

    /// The same Sora with these port options.
    pub fn with_tuning(mut self, tuning: Tuning) -> Sora {
        self.pawn.tuning = tuning;
        self
    }

    pub fn params(&self) -> &SoraParams {
        self.pawn.params()
    }

    /// (party K1) AI SORA'S CATCH-UP (kh-partyhost, while a friend leads): his run and sprint speeds (MaxWalkSpeed
    /// 680 / MySprintSpeed 980) times the scale (1 = his own; clamped 1 .. 3); his run cycle stays speed-matched above
    /// the run speed while scaled (states/run.rs). Port design: KH3 has no AI Sora.
    pub fn set_speed_scale(&mut self, scale: f32) {
        let s = if scale.is_finite() { scale.clamp(1.0, 3.0) } else { 1.0 };
        let base = crate::params::SORA_PARAMS;
        let p = self.pawn.params_mut();
        if (p.speed_run - base.speed_run * s).abs() > 1e-3 || (p.speed_sprint - base.speed_sprint * s).abs() > 1e-3 {
            p.speed_run = base.speed_run * s;
            p.speed_sprint = base.speed_sprint * s;
        }
    }

    /// (DON-KIT) PORT ADDITION: DONALD'S AERO AURA ON SORA (documentation/DONALD_OVERHAUL.md 4.2 "Party evasion and
    /// recovery speed up"; the host passes the aura where he stands, every step): `evasion` lengthens the Dodge Roll's and
    /// the Air Slide / Air Dodge's invincible window x (1 + evasion) (the hits that come in the added time are dropped as
    /// the reaction type 11 drops them); `recovery` plays his hit reaction and getting up ON THE GROUND x (1 + recovery)
    /// (the state's clock and the pose's; the knockback's movement keeps the real time). 0 / 0 = KH3's behaviour.
    pub fn set_aura(&mut self, evasion: f32, recovery: f32) {
        let ok = |v: f32| if v.is_finite() { v.clamp(0.0, 2.0) } else { 0.0 };
        self.pawn.aura_evasion = ok(evasion);
        self.pawn.aura_recovery = ok(recovery);
    }

    /// (DON-KIT) The aura shares in force (evasion, recovery).
    pub fn aura(&self) -> (f32, f32) {
        (self.pawn.aura_evasion, self.pawn.aura_recovery)
    }

    /// (ER69, the user: KH1 Sora is shorter) THE BODY'S SIZE FOR AN OUTFIT OF OTHER PROPORTIONS: the collision capsule
    /// (radius, half height, centre height above the origin) is KH3 Sora's times `scale` (the outfit's hip height over
    /// his; 1 = his own). The capsule's foot stays at the origin's height (both heights scale alike).
    pub fn set_body_scale(&mut self, scale: f32) {
        let s = if scale.is_finite() { scale.clamp(0.5, 1.5) } else { 1.0 };
        let base = crate::params::SORA_PARAMS;
        let p = self.pawn.params_mut();
        (p.capsule_radius, p.capsule_half_height, p.capsule_center_z) = (base.capsule_radius * s, base.capsule_half_height * s, base.capsule_center_z * s);
    }

    /// Level, gauges, abilities, magic, items (data only until the stats rules exist).
    pub fn status(&self) -> &Status {
        &self.pawn.status
    }

    /// The status, to set it up (a loadout: `Status::from_loadout`) or for a host's debug keys.
    pub fn status_mut(&mut self) -> &mut Status {
        &mut self.pawn.status
    }

    /// The same Sora with this status.
    pub fn with_status(mut self, status: Status) -> Sora {
        self.pawn.status = status;
        self
    }

    /// The HUD model as the latest step published it.
    pub fn ui_snapshot(&self) -> UiSnapshot {
        let mut snapshot = self.hud.snapshot(self.pawn.step, &self.pawn.status, self.pawn.popups.clone());
        snapshot.lock_on = if self.pawn.combat.lockon.locked { self.pawn.combat.lockon.current } else { crate::host::TargetId::NONE };
        snapshot.situation = crate::combat::situation::hud(&self.pawn);
        snapshot.command = self.pawn.command_shown.clone();
        crate::ui_model::fill(&mut snapshot, &self.pawn, &self.state);
        snapshot
    }

    /// (integration pass) What the field camera's state branches test (docs/sora-camera-spec.md
    /// section 5 row 3; base rotation source 0x1406068F0 and the wall-run test 0x140608790,
    /// decompiled C): the wall run with its class (`WallRun+0xA8`: 0 up, 1 / 2 along), the rail
    /// slide (pawn vt+0x2B38), the slope slide (state 10).
    pub fn camera_state(&self) -> CameraState {
        match &self.state {
            StateData::WallRun(w) => CameraState::WallRun(states::wall_run::class_of(w)),
            StateData::RailSlide(_) => CameraState::RailSlide,
            StateData::SlopeSlide(_) => CameraState::SlopeSlide,
            _ => CameraState::None,
        }
    }

    /// The running weapon recoil, when the state is `Reflect`.
    pub fn recoil(&self) -> Option<states::ReflectView> {
        states::reflect::view(&self.state)
    }

    /// The running finish command, when the state is `StyleFinish`.
    pub fn finish(&self) -> Option<states::FinishView> {
        states::style_finish::view(&self.state)
    }

    /// The time dilation of object type `obj` (host/time.rs; 1.0 = normal).
    pub fn time_dilation(&self, obj: u32) -> f32 {
        self.pawn.time_dilation(obj)
    }

    /// DEBUG / FIXTURE (a host's `fill-style N`): adds raw points to the formchange gauge
    /// (`AddStylePoint(n, 3, 0)`: no rates), as the game's `DebugAddStylePoint` does.
    /// (XSI) DEBUG: the next ground finisher is the XS kind `kind` (0xE3 Spiral Rush, 0xE4 Zantetsuken, 0xE5 Event
    /// Horizon, 0xE6 Giant Stamp) whatever its condition, once, if its ability is equipped (captures of the moves).
    pub fn debug_xs_finisher(&mut self, kind: u8) {
        self.pawn.combat.xs.debug_finisher = Some(kind);
    }

    /// (XW-b) Chandelier's chandelier (KH3's StylePawn w_so088) this step, for the hosts to draw: where it stands, its turn
    /// and its clip time (abilities/xs/chandelier.rs `pawn`). None outside the move.
    pub fn xs_chandelier_pawn(&self) -> Option<crate::abilities::xs::chandelier::ChandelierPawn> {
        if self.attack().map(|a| a.kind) != Some(crate::abilities::xs::chandelier::CHANDELIER) {
            return None;
        }
        crate::abilities::xs::chandelier::pawn(self.pawn.combat.xs.chandelier.as_ref(), self.pawn.anim.time)
    }

    pub fn debug_add_style(&mut self, points: f32) {
        let style = &mut self.pawn.combat.style;
        style.pending += points.max(0.0);
        style.dec_start = crate::combat::tables::P.form_gauge_dec_start_time;
        style.dec_interval = 0.0;
    }

    /// (FX1) AI Sora can start a friend leader's unison / fusion now (combat/friend.rs `leader_unison_ready`): the host
    /// rolls `FriendState::roll_leader_unison` only when this is true (contract D.6 / FX1). `leader_uid` 11 Donald, 12 Goofy.
    pub fn can_start_leader_unison(&self, leader_uid: u8) -> bool {
        crate::combat::friend::leader_unison_ready(&self.state, &self.pawn, leader_uid)
    }

    /// The pawn's and controller's melee fields, mutable: for a host's fixtures and for tests
    /// (pausing the situation timers, the style fix ...).
    pub fn combat_mut(&mut self) -> &mut crate::combat::PawnCombat {
        &mut self.pawn.combat
    }

    /// Asks for a change of his status at the start of the next step: an EXP award, damage, a
    /// heal, MP use, an item, a prize (status/requests.rs). The rules of `status/` apply it and
    /// the HUD popups it causes go through the seam (`EventKind::Popup`, `UiSnapshot`). This is
    /// how a HOST reports what its world did to him (an enemy died: `give_exp`; an enemy hit
    /// him: `StatusRequest::Damage` with the amount of `status::compute_damage`).
    pub fn status_request(&mut self, request: StatusRequest) {
        self.requests.push(request);
    }

    /// (new game) GIVES HIM A REWARD NOW (status/reward.rs): an ability (owned, not equipped: as the
    /// game's victory bonuses), a magic tier (also put into a magic slot: 0x14159EE70), a max-HP / max-MP
    /// bonus, a slot, AP, a shotlock, an item (into the stock). Everything it changes is save data: a
    /// host's save captures it. The game's popup for it (the bonus panel, the item popup) is raised at
    /// the start of the next step. Returns what changed.
    pub fn grant(&mut self, reward: crate::status::Reward) -> crate::status::RewardOutcome {
        let outcome = self.grant_quiet(reward);
        if outcome.changed {
            self.requests.push(StatusRequest::RewardShown(outcome));
        }
        outcome
    }

    /// (boss rewards, IMPL_ER31) A BONUS PANEL of the host's own composition (one or two lines; the game's panel has
    /// two): raised at the start of the next step, as `grant`'s.
    pub fn show_bonus(&mut self, lines: [crate::status::BonusLine; 2]) {
        self.requests.push(StatusRequest::RewardShown(crate::status::RewardOutcome { changed: true, lines, ..Default::default() }));
    }

    /// (boss rewards, IMPL_ER31) [`Sora::grant`] WITHOUT ITS PANEL: the host shows several rewards together
    /// ([`Sora::show_bonus`]: two lines a panel, as the game's victory bonuses of one row).
    pub fn grant_quiet(&mut self, reward: crate::status::Reward) -> crate::status::RewardOutcome {
        let outcome = self.pawn.status.apply_reward(reward);
        if !outcome.changed {
            return outcome;
        }
        if let Some(command) = outcome.learned {
            let status = self.pawn.status.clone();
            self.pawn.command.fill(&status);
            self.pawn.command.learn(command);
        }
        if outcome.item_slot || outcome.item.is_some() {
            if !self.pawn.magic.pouch.filled {
                // (filled from the status as it was before a slot bonus: the slot is added below)
                let mut before = self.pawn.status.clone();
                before.item_slots -= u32::from(outcome.item_slot);
                self.pawn.magic.pouch = crate::magic::Pouch::from_status(&before);
            }
            if outcome.item_slot {
                self.pawn.magic.pouch.add_slot();
            }
            if let crate::status::Reward::Item { item, count } = reward {
                self.pawn.magic.pouch.stock_add(item, count);
            }
        }
        outcome
    }

    /// An enemy was defeated: `give_exp(enemy base exp, rate)` of docs/sora-stats-spec.md 4 at
    /// the start of the next step. `base_exp` = `EnemyBaseStatus[battle level].m_BaseExp`,
    /// `rate` = the enemy row's `m_ExpRate`; `position` = where the floating EXP number shows
    /// (the enemy), `None` for an award without one.
    pub fn give_exp(&mut self, base_exp: i32, rate: f32, position: Option<Vec3>) {
        self.requests.push(StatusRequest::GiveExp { base_exp, rate, position });
    }

    /// A hit on Sora (an enemy's attack reached him): applied at the start of the next step:
    /// HP by the status rules (Second Chance, Withstand Combo), then the reaction: the Damage
    /// state of the hit's damage kind, or the Die state. The host decides WHETHER he was hit (its
    /// own collision against his hurt shape: a capsule of radius 50 and half-length 90 on his
    /// `Center` socket, combat strand E 9.10) and with what amount.
    pub fn take_hit(&mut self, hit: states::HitOnPlayer) {
        self.pawn.combat.incoming.push(hit);
    }

    /// The running damage reaction, when the state is `Damage`.
    pub fn damage_reaction(&self) -> Option<states::DamageView> {
        states::damage::view(&self.state)
    }

    /// The running attack, when the state is `Attack` (kind, combo count, windows).
    pub fn attack(&self) -> Option<states::AttackView> {
        states::attack::view(&self.state)
    }

    /// The fields of the action states (actions.rs): the availability bits, the reaction type and
    /// guard parts, the battle flag, weapon out, the poses, the counter window.
    pub fn actions(&self) -> &crate::actions::PawnActions {
        &self.pawn.actions
    }

    /// The action fields, to set them up: the level's switches (`traversal.wall_run_unlocked`,
    /// `flow_unlocked`, `traversal.floating` ...). For hosts and tests.
    pub fn actions_mut(&mut self) -> &mut crate::actions::PawnActions {
        &mut self.pawn.actions
    }

    /// `BP_NotifySetFloatingMode(IsOn, IsSpecial)` (controller slot 705 0x1413EA6C0; actions spec
    /// II.2 "Floating"): the floating mode of a scripted set piece. While it is on he is never "on
    /// the ground": Idle and Jump hand over to the Floating state (slow sink, the stick flies him
    /// at 680 cm/s, or 1500 with `special`).
    pub fn set_floating(&mut self, on: bool, special: bool) {
        let t = &mut self.pawn.actions.traversal;
        (t.floating, t.floating_special) = (on, on && special);
    }

    /// An enemy's attack warns of a risk dodge (`TresAnimNotify_DamageCaution` with its
    /// `ETresRiskDodgeType` 1 UNDERSLIDE, 2 OVERSLIDE, 3 SLIDETURN, 4 WARP; actions spec II.1): the
    /// offer stays for `seconds` (the enemy's warning window). If a hit of that attacker then
    /// breaks Sora's guard into the weapon recoil (reflect level >= 3), the risk-dodge prompt
    /// shows for 0.6 s and the jump button dodges.
    pub fn offer_risk_dodge(&mut self, attacker: crate::host::TargetId, kind: u8, seconds: f32) {
        let offers = &mut self.pawn.actions.risk_offers;
        offers.retain(|o| o.attacker != attacker);
        offers.push(crate::actions::RiskOffer { kind, attacker, left: seconds });
    }

    /// The running risk dodge, when the state is `RiskDodge`.
    pub fn risk_dodge(&self) -> Option<states::RiskDodgeView> {
        states::risk_dodge::view(&self.state)
    }

    /// (IMPL_ER58) A HOST'S TREASURE BOX: Sora opens it as KH3 does (`TresPlayerState_OpenTreasureBox`,
    /// states/open_treasure_box.rs: `OpenTreasureS` / `OpenTreasureL` with their sounds and the Keyblade), from the
    /// next step, whatever he is doing. False (nothing asked) when he is dead.
    pub fn open_treasure_box(&mut self, large: bool) -> bool {
        if self.pawn.status.hp < 1 {
            return false;
        }
        states::open_treasure_box::request(&mut self.pawn, large);
        true
    }

    /// (IMPL_ER58) The running treasure-box state, if it runs.
    pub fn treasure_box(&self) -> Option<states::TreasureView> {
        states::open_treasure_box::view(&self.state)
    }

    /// The running jump's type (0 normal, 1 High Jump, 2 super jump, 3 Doubleflight), when the
    /// state is `Jump`.
    pub fn jump_kind(&self) -> Option<u8> {
        states::jump::kind_of(&self.state)
    }

    /// The forced battle flag (the game object's G+0x8D4 bit 4, setter 0x140EB8560): battle
    /// whatever the host's engaged-enemy count says.
    pub fn force_battle(&mut self, on: bool) {
        self.pawn.actions.battle_forced = on;
    }

    /// PORT ADDITION (IMPL_ER44, not the game's code): the host holds the Keyblade out (true: Sora draws it with the
    /// game's appear sequence at the next step and keeps it, idle and running included; false: put away again at the
    /// next step when he is not in battle). Off by default: the game's own stance rules alone.
    pub fn hold_weapon(&mut self, hold: bool) {
        self.pawn.actions.hold_weapon = hold;
    }

    /// The pawn's and controller's melee fields: targets, hit windows, combo bookkeeping.
    pub fn combat(&self) -> &crate::combat::PawnCombat {
        &self.pawn.combat
    }

    /// DEBUG (a host's `play-anim NAME`): plays an animation on the FullBody slot as a one-shot
    /// without root motion, whatever the state; the running state goes on and may replace it.
    pub fn debug_play_anim(&mut self, anim: AnimId, args: crate::anim::PlayArgs) {
        self.pawn.anim.play_with(anim, args);
    }

    /// MAGIC SIDE (magic/mod.rs): the projectiles, ice-rail nodes and Aero pads alive, the pouch,
    /// the aim mode's lists, the grand-magic tallies.
    pub fn magic(&self) -> &crate::magic::PawnMagic {
        &self.pawn.magic
    }

    /// (DON-MAGIC tip) The Keyblade's tip (socket ATK_01) in the world now, from the playing clip (None: the clip has no
    /// socket track): where his spells leave from.
    pub fn keyblade_tip(&self) -> Option<crate::Vec3> {
        crate::states::fire_magic::muzzle(&self.pawn)
    }

    /// The magic side, to set it up: the level's airstep points, the story switches
    /// (`shotlock_unlocked`, `airstep_unlocked`), the pouch and its stock.
    pub fn magic_mut(&mut self) -> &mut crate::magic::PawnMagic {
        &mut self.pawn.magic
    }

    /// The per-object-type time dilation after the latest step: the host applies `world` to its
    /// enemies and level objects (and `camera` to its camera); Sora's own rate is already applied.
    pub fn dilation(&self) -> crate::magic::TimeDilation {
        crate::magic::TimeDilation { player: self.pawn.time_dilation(1), world: self.pawn.time_dilation(2), camera: self.pawn.time_dilation(9) }
    }

    /// The shotlock aim mode (mode, kind, locks, airstep chain).
    pub fn aim(&self) -> crate::magic::AimView {
        self.pawn.magic.aim.view()
    }

    /// The running cast, when the state is `FireMagic`.
    pub fn cast(&self) -> Option<states::CastView> {
        states::fire_magic::view(&self.state)
    }

    /// (magic implementer) The running airstep, when the state is `AthleticFlow`.
    pub fn airstep(&self) -> Option<states::AirstepView> {
        states::athletic_flow::view(&self.state, &self.pawn)
    }

    /// The command system's state (menu cursors, shortcut pages, config, command timers).
    pub fn command(&self) -> &crate::command::CommandState {
        &self.pawn.command
    }

    pub fn command_mut(&mut self) -> &mut crate::command::CommandState {
        &mut self.pawn.command
    }

    /// Asks for a command directly (a script word, a debug key): executed in the next step's UI
    /// phase through the same `ExecuteCommand` as the menu, with the same gates.
    pub fn command_request(&mut self, request: crate::command::CommandRequest) {
        self.pawn.command.requests.push(request);
    }

    /// The game left a pause (the camp menu closed): used auto-reload pouch slots are refilled
    /// from the stock (magic spec J 7.5). Returns how many slots were refilled.
    pub fn on_unpause(&mut self) -> u32 {
        if !self.pawn.magic.pouch.filled {
            self.pawn.magic.pouch = crate::magic::Pouch::from_status(&self.pawn.status);
        }
        let n = self.pawn.magic.pouch.refill();
        let pouch = self.pawn.magic.pouch.clone();
        pouch.sync_status(&mut self.pawn.status);
        n
    }

    /// The running state's flag byte (`m_bEnableChange`).
    pub fn state_flags(&self) -> StateFlags {
        self.pawn.queue.flags
    }

    /// Queues a debug emission for the start of the next step (see [`DebugEmit`]).
    pub fn debug_emit(&mut self, emit: DebugEmit) {
        self.debug.push(emit);
    }

    /// Things asked for that the port does not implement yet (each said once), e.g. the roll landing.
    pub fn notes(&self) -> &[&'static str] {
        &self.pawn.notes
    }

    /// Height of the actor origin above a flat floor while standing: the capsule's bottom floats
    /// 1.9 to 2.4 cm (aimed at 2.15) and sits 2 cm above the origin.
    pub fn origin_height_on_floor(params: &SoraParams) -> f32 {
        params.floor_dist_target - (params.capsule_center_z - params.capsule_half_height)
    }
}

/// An angle in degrees brought into (-180, 180].
pub fn wrap_degrees(a: f32) -> f32 {
    let r = a.rem_euclid(360.0);
    if r > 180.0 {
        r - 360.0
    } else {
        r
    }
}

/// (DON-KIT) THE AERO AURA'S RECOVERY RATE of this step: 1 + the share while he is in the Damage state on the ground
/// (the flinch, the landing, lying and getting up; the flight keeps the physics' time), else exactly 1.
fn aura_recovery_rate(state: &states::StateData, pawn: &crate::pawn::Pawn) -> f32 {
    if pawn.aura_recovery > 0.0 && matches!(state, states::StateData::Damage(_)) && pawn.cmc.on_ground() {
        1.0 + pawn.aura_recovery
    } else {
        1.0
    }
}
