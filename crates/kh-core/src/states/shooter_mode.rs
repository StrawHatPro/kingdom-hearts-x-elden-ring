//! (F-ARROWGUN) `TresPlayerState_ShooterMode` (id 40, TSID_STATE_SHOOTERMODE): the gun mode of the four gun formchanges
//! (Double Arrowguns 0x80 / Hunny Blasters 0x8E: "Auto-Shot", the DW_GATLING loop; Magic Launcher 0x83 / Hunny Launcher
//! 0x8F: "Charged Shot", BZ_BOMBSHOT2 / BZ_CRACKER). Held RT past the tap time requests it (combat/forms/arrowgun/aim.rs:
//! UpdateLockon step 6 -> request interface slot 191 0x141447640 -> the gate 0x14142D9D0 -> SetState(40)).
//!
//! Vtables read from the exe: main 0x14693BF48 (Begin 96 0x1413A6900, Tick 97 0x1413B0DC0, End 98 0x1413ABE40, slot 73
//! 0x1413A81A0 debug text only, 113 0x1413B2DC0 SetLookAt, 114 0x1413BED50 SetCamera, 115 0x1413BDA70 SetDamagePawn,
//! 116 0x1413C5170), anim interface 0x14693C2F0 (slot 69 +0x228 = 0x1413B51B0, the TresShot handler; every other slot is
//! the base's), interface 2 0x14693C738. Constructor 0x1413A2C30: +0xBC -1, +0xC4 120.0, +0xFC 5000, +0xEC 60.0, +0xF0 /
//! +0xF4 / +0xF8 (30, -65, 45).
//!
//! Not in the core (INT / other owners, build/sora_tmp/weapons/F-ARROWGUN/STATE.md): the camera actor
//! ATresCameraShooterMode (the hosts read `aim.view`), the HUD scope / charge gauge / target gauge, the weapon glow
//! (pawn vt+0x1468), the actor's pitch (the port's actor turns about Z only: the aim pitch is kept here and given to the
//! shots and the view), ctrl+0x878 bit 1 (StartStyle's permission) and pawn+0xE48 bit 6, the Tick's tail probes (water ->
//! SWIM, slope slide, rail slide: their requests live in the traversal states), SMALL hits swallowed by the reaction
//! gate 0x141439A50 case 40 (damage.rs), and the jump / attack / air-slide requests out of state 40 (their gates).

use super::common::turn;
use super::{PlayerState, StateData, StateId};
use crate::anim::{AnimId, NotifyClass, NotifyData, NotifyPhase, Notifies, PlayArgs};
use crate::combat::forms::arrowgun::{self as ag, Family, Homing};
use crate::combat::AttackCtx;
use crate::host::{EffectHandle, TargetFlags, TargetId};
use crate::pawn::{Ctx, Pawn};
use crate::sora::{wrap_degrees, MovementState};
use glam::{Vec2, Vec3};

/// Begin: pawn vt+0x1650(0.1) (the movement's GravityScale).
const GRAVITY_SCALE: f32 = 0.1;
/// +0xC4: the charge per second; the full charge.
const CHARGE_RATE: f32 = 120.0;
const CHARGE_FULL: f32 = 100.0;
/// +0xFC's start and the look distance without a target.
const LOOK_DISTANCE: f32 = 5000.0;
/// The eye above the actor (Rot * (0, 0, 150)) and the nearest target the aim turns to.
const EYE_UP: f32 = 150.0;
const AIM_MIN_DIST: f32 = 150.0;
/// The aim pitch's clamp (FUN_142D74AD0(x, -45, 45)).
const PITCH_LIMIT: f32 = 45.0;
/// +0xA4 >= 0.5: the manual aim; its rate (+0xA4 x 0.03), the auto aim's (0.2, 0.5 on a new target, 1.0 looking at one).
const MANUAL_MIN: f32 = 0.5;
const MANUAL_RATE: f32 = 0.03;
const AUTO_RATE: f32 = 0.2;
const NEW_TARGET_RATE: f32 = 0.5;
/// The manual aim's speed: the camera input x 60 x dt (degrees; the input is -1 .. 1 after the dead zones).
const AIM_SPEED: f32 = 60.0;
/// The shooter camera's stick play (m_AnalogStickPlayX / Y: TresCameraShooterMode is the native class, native default 0.1:
/// docs/sora-camera-spec.md) after the engine's per-axis dead zone 0.25 (DefaultInput.ini; controller.rs `dead_zone`).
const STICK_PLAY: f32 = 0.1;
const AXIS_DEAD_ZONE: f32 = 0.25;
/// Camera slot 258 0x1405CD3F0: the right stick's Y is negated (the left stick's, the fallback, is not). AS TRACED; the
/// sign of the aim's pitch under the right stick is a point to look at in game (config byte +0x2E8 bit 0x20 inverts it).
const RIGHT_Y_SIGN: f32 = -1.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ShooterMode {
    sub: u8,
    /// +0xA4: MoveInput's stick magnitude, also the peak of the right stick: the manual aim test (0x141440600 case 40
    /// writes +0xA8 yaw / +0xA4 / +0xA0 = 1; +0xA0 / +0xA8 feed only the rail probe and EndStyle's Fall: not ported).
    peak: f32,
    /// +0xB1: the end request (slot 715's tail, slot 840).
    pub(crate) end_request: bool,
    /// +0xB8 (and +0xB4): the launcher's charge mode (weapon kinds 5 / 0x11).
    launcher: bool,
    /// +0xBC: the weapon's charge effect level (-1 / 0 / 1); +0xC0 the charge; +0xC8 charged.
    level: i8,
    charge: f32,
    charged: bool,
    /// +0xCC / +0xD0: the aim yaw and pitch (+0xCC is the yaw TryAttack 0x14143E670 / JumpRequest 0x14143D8F0 case 40 use).
    pub(crate) yaw: f32,
    pitch: f32,
    /// +0xD8: the projectile name of the TresShot handler.
    shot: Option<&'static str>,
    /// +0xE0: looking at a target given by slot 113.
    look_at: bool,
    /// +0xFC: the look distance.
    distance: f32,
    /// +0x100: the shooter camera is on (slot 114).
    camera: bool,
    /// m_pTargetComponent (state+0x90).
    target: TargetId,
    /// (AUDIT-A ARROWGUN) The weapon glow's started groups (weapon +0x220 .. : 0x802 + i), `arrowgun::charge_glow`.
    glow: [Option<EffectHandle>; 3],
    /// (AUDIT-A ARROWGUN) +0xA0 / +0xA8: a MoveInput this frame and its stick yaw (kept on the pawn as `aim.stick_yaw`).
    stick_now: bool,
    stick_yaw: f32,
}

impl ShooterMode {
    pub const RESET: ShooterMode = ShooterMode {
        sub: 0,
        peak: 0.0,
        end_request: false,
        launcher: false,
        level: -1,
        charge: 0.0,
        charged: false,
        yaw: 0.0,
        pitch: 0.0,
        shot: None,
        look_at: false,
        distance: LOOK_DISTANCE,
        camera: false,
        target: TargetId::NONE,
        glow: [None; 3], // (AUDIT-A ARROWGUN)
        stick_now: false,
        stick_yaw: 0.0,
    };

    /// Play(name, 0, blendIn, mode, 0, 1.0, scale) of a logical name through the form's anim set; its length (0 = missing).
    fn play(cx: &mut Ctx, logical: &str, blend_in: f32, mode: u8) -> f32 {
        let clip = ag::clip(cx.pawn.combat.form.cmd, logical);
        match AnimId::from_name(clip) {
            Some(anim) => {
                cx.pawn.anim.play_with(anim, PlayArgs::mode(mode, blend_in));
                anim.data().length
            }
            None => {
                cx.pawn.note("a ShooterMode clip is not in the generated animation table (tools/gen_port.js FORM_ANIM_SETS)");
                0.0
            }
        }
    }

    /// The eye of the aim: the actor's centre (its root component) + Rot(pitch, yaw) * (0, 0, 150).
    fn eye(&self, cx: &Ctx) -> Vec3 {
        let (sp, cp) = self.pitch.to_radians().sin_cos();
        let (sy, cy) = self.yaw.to_radians().sin_cos();
        ag::centre(cx) + Vec3::new(-sp * cy, -sp * sy, cp) * EYE_UP
    }

    /// Slot 114 0x1413BED50 SetCamera(on).
    fn set_camera(&mut self, on: bool) {
        self.camera = on;
    }

    /// ShooterModeEnd (blend -1, mode 2), the camera off, sub-step 7.
    fn finish(&mut self, cx: &mut Ctx) {
        Self::play(cx, "ShooterModeEnd", -1.0, 2);
        self.set_camera(false);
        self.sub = 7;
    }

    /// The level of the weapon's charge effect (+0xBC; pawn vt+0x1468 when it changes: (AUDIT-A ARROWGUN) the weapon glow,
    /// `arrowgun::charge_glow`).
    fn set_level(&mut self, cx: &mut Ctx, level: i8) {
        if self.level != level {
            self.level = level;
            ag::charge_glow(cx, &mut self.glow, i32::from(level));
        }
    }

    /// The aim part of the Tick (sub-steps 2 .. 5, with the shooter camera on).
    fn aim(&mut self, cx: &mut Ctx, dt: f32) {
        let mut rate = AUTO_RATE;
        let mut wanted = LOOK_DISTANCE;
        // The right stick's magnitude raises the peak (0x1444333A0(ctrl, 1)).
        let right = massage(cx.pawn.combat.arrowgun.aim.look);
        let mag = right.length();
        if self.peak < mag {
            self.peak = mag;
        }
        if self.peak >= MANUAL_MIN {
            // Manual: camera vt+0x818 (slot 259 -> slot 258 mode 2): the right stick, else the left one.
            let input = if right == Vec2::ZERO { play(massage(cx.pawn.input.move_stick)) } else { let p = play(right); Vec2::new(p.x, p.y * RIGHT_Y_SIGN) };
            self.yaw = wrap_degrees(self.yaw + input.x * AIM_SPEED * dt);
            self.pitch = (self.pitch + input.y * AIM_SPEED * dt).clamp(-PITCH_LIMIT, PITCH_LIMIT);
            rate = self.peak * MANUAL_RATE;
            let pick = cx.pawn.combat.arrowgun.aim.pick;
            if valid(cx, pick) && pick != self.target {
                // slot 115(owner): the HUD's target gauge (the hosts read `aim.view.target`).
                self.target = pick;
            }
            self.look_at = false;
        } else {
            let old = self.target;
            if !self.look_at {
                self.target = cx.pawn.combat.arrowgun.aim.pick;
            } else {
                rate = 1.0;
                self.peak = 0.0;
            }
            if let Some(t) = cx.target(self.target).filter(|_| valid(cx, self.target)) {
                let d = t.center - self.eye(cx);
                let dist = d.length();
                wanted = dist;
                if dist >= AIM_MIN_DIST {
                    let flat = d.truncate().length();
                    self.pitch = d.z.atan2(flat).to_degrees().clamp(-PITCH_LIMIT, PITCH_LIMIT);
                    if flat > 1.0e-4 {
                        self.yaw = d.y.atan2(d.x).to_degrees();
                    }
                    if old != self.target {
                        rate = NEW_TARGET_RATE;
                    }
                }
            }
        }
        // SetActorRotation(pitch, yaw, 0) (0x143EAD160, Tick 0x1413B0DC0): (AUDIT-A ARROWGUN) the actor takes the yaw AND
        // the aim's pitch (+0xD0: the body tilts with the aim; `Output::pitch`, hosts tilt about the capsule centre).
        cx.pawn.yaw = self.yaw;
        cx.pawn.pitch = self.pitch;
        // +0xFC toward the target's distance; the camera's look-at point.
        let a = (rate * dt * 60.0).clamp(0.0, 1.0);
        self.distance += (wanted - self.distance) * a;
    }

    /// Publish what the camera / HUD read.
    fn publish(&self, cx: &mut Ctx) {
        let eye = self.eye(cx);
        let view = ag::aim::ShooterView {
            camera: self.camera,
            look_at: eye + dir(self.pitch, self.yaw) * self.distance,
            eye,
            yaw: self.yaw,
            pitch: self.pitch,
            distance: self.distance,
            charge: self.charge,
            charge_level: self.level,
            charged: self.charged,
            launcher: self.launcher,
            target: self.target,
        };
        let aim = &mut cx.pawn.combat.arrowgun.aim;
        aim.view = view;
        aim.look_at = self.look_at;
    }
}

/// Rotator(pitch, yaw).Vector().
fn dir(pitch: f32, yaw: f32) -> Vec3 {
    let (sp, cp) = pitch.to_radians().sin_cos();
    let (sy, cy) = yaw.to_radians().sin_cos();
    Vec3::new(cp * cy, cp * sy, sp)
}

/// The engine's per-axis dead zone (MassageAxisInput, DefaultInput.ini 0.25) of a raw stick.
fn massage(v: Vec2) -> Vec2 {
    let f = |x: f32| if !x.is_finite() { 0.0 } else if x > 0.0 { (x - AXIS_DEAD_ZONE).max(0.0) / (1.0 - AXIS_DEAD_ZONE) } else { -(-x - AXIS_DEAD_ZONE).max(0.0) / (1.0 - AXIS_DEAD_ZONE) };
    Vec2::new(f(v.x), f(v.y))
}

/// The camera's stick play (slot 258: per axis, linear, the sign kept).
fn play(v: Vec2) -> Vec2 {
    let f = |x: f32| (((x.abs() - STICK_PLAY) / (1.0 - STICK_PLAY)).max(0.0)).copysign(x);
    Vec2::new(f(v.x), f(v.y))
}

fn valid(cx: &Ctx, t: TargetId) -> bool {
    t != TargetId::NONE && cx.target(t).is_some_and(|t| t.flags.has(TargetFlags::ALIVE | TargetFlags::LOCKABLE))
}

/// The four forms' weapon kinds (pawn vt+0x12E0(-1)): 2 / 5 / 0x10 / 0x11 in 0x80 / 0x83 / 0x8E / 0x8F (the Tick's end test
/// and Begin's charge mode).
fn family_of(cx: &Ctx) -> Option<Family> {
    ag::family(cx.pawn.combat.form.cmd)
}

impl PlayerState for ShooterMode {
    const ID: StateId = StateId::Shootermode;

    fn sub_step(&self) -> u8 {
        self.sub
    }

    /// Begin 0x1413A6900.
    fn begin(&mut self, cx: &mut Ctx) {
        // vt+0x1780(1): the last pose BATTLE; vt+0x16E8 StopMovement; Draw(1, 1, 1.0); vt+0x1650(0.1).
        cx.pawn.actions.last_pose = 1;
        cx.pawn.cmc.stop_movement();
        crate::actions::draw(cx, true);
        cx.pawn.cmc.gravity_scale = GRAVITY_SCALE;
        // The aim from the actor's rotation; weapon kinds 5 / 0x11: the charge mode (+0xB4 = +0xB8 = 1).
        (self.pitch, self.yaw) = (0.0, cx.pawn.yaw);
        self.launcher = family_of(cx) == Some(Family::Bz);
        // The camera actor on the manager's stack: pitch 0, yaw = the camera's; Turn((0, yaw, 0), 60.0) (a full turn).
        self.yaw = wrap_degrees(cx.pawn.input.camera_yaw);
        turn(cx.pawn, self.yaw, 60.0);
        // (AUDIT-A ARROWGUN) an EndStyle Fall left from outside the state is not this one's
        cx.pawn.combat.arrowgun.aim.end_style_fall = false;
        self.publish(cx);
    }

    /// Tick 0x1413B0DC0.
    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        if self.stick_now {
            cx.pawn.combat.arrowgun.aim.stick_yaw = self.stick_yaw; // (AUDIT-A ARROWGUN) +0xA8 outlives the state
        }
        // (AUDIT-A ARROWGUN) EndStyle 0x14141CA20 found this state: iface slot 172 Fall(+0xA8) and DoChain (no ShooterModeEnd).
        if std::mem::take(&mut cx.pawn.combat.arrowgun.aim.end_style_fall) {
            let yaw = cx.pawn.combat.arrowgun.aim.stick_yaw;
            super::jump::fall(cx.pawn, yaw);
            self.publish(cx);
            return;
        }
        let active = (2..=5).contains(&self.sub);
        if active {
            // The end test: the shooter aim off (ctrl vt+0x16C8), ctrl+0xA08 bit 0, or a weapon kind outside 2 / 5 / 0x10 / 0x11.
            if !cx.pawn.combat.arrowgun.aim.shooter || family_of(cx).is_none() {
                self.finish(cx);
                self.publish(cx);
                return;
            }
            if self.camera {
                self.aim(cx, dt);
            }
        }
        match self.sub {
            0 => {
                Self::play(cx, "ShooterModeStart", -1.0, 2);
                self.sub = 1;
            }
            1 => {
                if cx.pawn.anim.ended {
                    if !self.launcher {
                        // The arrowguns: the camera, DW_GATLING, the loop M_2416 (blend 0, mode 3).
                        self.set_camera(true);
                        self.shot = Some("DW_GATLING");
                        Self::play(cx, "M_2416", 0.0, 3);
                        self.sub = 3;
                    } else {
                        Self::play(cx, "ShooterMode", 0.0, 3);
                        self.set_camera(true);
                        self.sub = 4;
                    }
                }
            }
            3 => {
                if self.end_request {
                    self.finish(cx);
                }
            }
            4 => self.charge_tick(cx, dt),
            5 => {
                if cx.pawn.queue.flags.enable_change {
                    self.set_level(cx, -1);
                    self.charge = 0.0;
                    // HUD charge-full off (0x14140BE90(ctrl, +0xB8, 0)).
                    self.charged = false;
                    if self.end_request {
                        self.finish(cx);
                    } else {
                        cx.pawn.queue.flags.enable_change = false;
                        self.sub = 4;
                    }
                }
            }
            _ => {
                if cx.pawn.anim.ended {
                    self.publish(cx);
                    cx.pawn.queue.do_chain();
                    return;
                }
            }
        }
        // (AUDIT-A ARROWGUN) THE TAIL PROBES (LAB_1413b2023, only when the Tick began in sub-steps 2 .. 5): in water
        // (pawn vt+0x1A80 = movement slot 180 0x140589410, deeper than the swim depth) a SWIM (102) request; with pawn+0x10E8
        // bit 0 a COVER (142) request; TrySlopeSlide(0) (iface slot 188); unless movement+0xBDC bit 0 (vt+0x19F8) the rail
        // probe (vt+0xEA0: MyRailSlideCheckRange along the actor's facing, the stick yaw +0xA8 while +0xA0) and
        // TryRailSlide(1, 0) (iface slot 75): any of them taken -> DoChain. The port has no water or cover (no SWIM / COVER
        // state), so the first two never ask; the rail gate (0x14142C800) refuses state 40 (only 0 .. 3 and COVER pass), so
        // the rail request is refused as in KH3; the slope gate (0x14142DE50) has no state test: a flagged slope under him
        // slides him out of ShooterMode.
        if active {
            let me = StateData::ShooterMode(*self);
            let yaw = if self.stick_now { cx.pawn.combat.arrowgun.aim.stick_yaw } else { cx.pawn.yaw };
            if super::slope_slide::try_slope_slide(&me, cx, false) || super::rail_slide::probe_and_try(&me, cx, crate::actions::A.rail_check_range, yaw) {
                self.publish(cx);
                cx.pawn.queue.do_chain();
                return;
            }
        }
        // +0xA0 = 0 (LAB_1413b22ca): the stick of this frame is used up
        self.stick_now = false;
        self.publish(cx);
    }

    /// End 0x1413ABE40.
    fn end(&mut self, cx: &mut Ctx) {
        self.set_camera(false);
        // slot 115(0): the HUD's target released; the charge effect off; the HUD's charge-full off.
        self.target = TargetId::NONE;
        self.set_level(cx, -1);
        self.charged = false;
        // ResetGravityScale (vt+0x1658); Turn((0, actor yaw, 0), 1.0); vt+0xDC8(1.0) (pitch and roll back).
        cx.pawn.cmc.gravity_scale = cx.pawn.params().gravity_scale;
        let yaw = cx.pawn.yaw;
        turn(cx.pawn, yaw, 1.0);
        self.pitch = 0.0;
        cx.pawn.pitch = 0.0; // (AUDIT-A ARROWGUN) Turn((0, yaw, 0), 1.0): the actor's pitch back
        let aim = &mut cx.pawn.combat.arrowgun.aim;
        aim.view = ag::aim::ShooterView::default();
        aim.look_at = false;
    }

    fn on_notifies(&mut self, cx: &mut Ctx, notifies: &Notifies) {
        for event in &notifies.events {
            if (event.class(), event.phase) != (NotifyClass::TresAnimNotifyShot, NotifyPhase::Instant) {
                continue;
            }
            let location = match event.def().data {
                NotifyData::Shot { location_index, .. } => location_index.unwrap_or(0),
                _ => 0,
            };
            self.on_shot(cx, location);
        }
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}

impl ShooterMode {
    /// Sub-step 4: the launcher's charge (+0xC0 += world dt x 120 to 100), then the shot: BZ_CRACKER / M_5416 at the full
    /// charge (fired at once, no release needed), BZ_BOMBSHOT2 / M_5415 on an earlier release (+0xB1).
    fn charge_tick(&mut self, cx: &mut Ctx, dt: f32) {
        if cx.pawn.anim.ended {
            Self::play(cx, "ShooterMode", 0.2, 3);
        }
        if !self.charged {
            self.charge += dt * CHARGE_RATE;
            if self.charge >= CHARGE_FULL {
                self.set_level(cx, 1);
                self.charge = CHARGE_FULL;
                // HUD charge-full on (0x14140BE90(ctrl, +0xB8, 1)).
                self.charged = true;
            } else {
                self.set_level(cx, 0);
            }
            if !self.charged && !self.end_request {
                return;
            }
        }
        let (name, clip) = if self.level == 1 { ("BZ_CRACKER", "M_5416") } else { ("BZ_BOMBSHOT2", "M_5415") };
        self.shot = Some(name);
        Self::play(cx, clip, 0.0, 2);
        cx.pawn.queue.flags.enable_change = false;
        self.sub = 5;
    }

    /// The anim interface's TresShot handler (slot 69 0x1413B51B0): the shot name +0xD8 from weapon[LocationIndex]'s muzzle
    /// (pawn vt+0x1410) along the actor's forward (the aim's pitch and yaw), the team's; homing (vt+0x1220) on
    /// m_pTargetComponent while the pick is on screen (ctrl vt+0x16D0 = +0x98C); vt+0x11F8(muzzle), vt+0x1270 (start).
    /// The charge effect then goes (+0xBC -1). No vt+0x1130 / 0x1140 / 0x1120 / 0x1180: the shot carries no attack data.
    fn on_shot(&mut self, cx: &mut Ctx, location: i32) {
        let Some(name) = self.shot else { return };
        if cx.pawn.actions.weapon_out {
            let at = ag::muzzle(cx, location);
            let homing = if cx.pawn.combat.arrowgun.aim.on_screen && valid(cx, self.target) { Homing::Target(self.target) } else { Homing::None };
            ag::fire(cx, name, at, dir(self.pitch, self.yaw), homing, AttackCtx::default());
        }
        self.set_level(cx, -1);
    }
}

/// MoveInput (slot 35 0x141440600) case 40: +0xA4 = magnitude, (AUDIT-A ARROWGUN) +0xA8 = the stick yaw, +0xA0 = 1 (the
/// Tick's rail probe direction, cleared at its end; EndStyle's Fall).
pub(crate) fn move_input(state: &mut StateData, yaw: f32, magnitude: f32) {
    if let StateData::ShooterMode(m) = state {
        m.peak = magnitude;
        m.stick_now = true;
        m.stick_yaw = yaw;
    }
}

/// The request interface's gate (slot 93 0x14142D9D0) for the states this port has: the form (pawn vt+0x948 vt+0x1A70)
/// one of 0x80 / 0x83 / 0x8E / 0x8F with at least 1 s of form time left (vt+0x1A98 = ctrl+0xB54); the current state's row
/// (JUMP: no flow jump, Jump+0xF8; the attack classes: nothing queued, Attack+0x143; FIREMAGIC: not a follow-up, +0xD8 &
/// 0x10; the refused ids of the switch). Not read here: pawn vt+0x2770 / 0x2540 / 0xA48 / 0x1C30 / 0x2748 / 0x1DF8, iface
/// vt+0x618, 0x1405AB6C0 and 0x14138FAC0 (the cutscene / ride / command restrictions: off in the forms' field play).
pub(crate) fn gate(state: &StateData, s: &Pawn) -> bool {
    let cmd = s.combat.form.cmd;
    if !matches!(cmd, ag::CHANGE_DW | ag::CHANGE_BZ | ag::CHANGE_DW2 | ag::CHANGE_BZ2) || s.combat.form.time_left < 1.0 {
        return false;
    }
    let id = state.id() as i32;
    const REFUSED: &[i32] = &[6, 7, 8, 9, 10, 11, 19, 21, 22, 23, 24, 25, 26, 27, 28, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 61, 62, 73, 74, 92, 95, 96, 97, 101, 103, 110, 111, 112, 114, 115, 117, 119, 120, 121, 122, 123, 124, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 142, 144, 145, 146, 150, 163, 164, 165, 166, 167, 168];
    if REFUSED.contains(&id) {
        return false;
    }
    // (AUDIT-A ARROWGUN) the inputs F-ARROWGUN took as off, traced: pawn vt+0x2748 = 0x14110E6E0 = pawn+0xE48 bit 5 (the
    // finisher's end, `combat.finish_end`) refuses the attack rows and FIREMAGIC; 0x14138FAC0(jump) = Jump sub-step 7 / 8
    // (the landing) refuses JUMP; pawn vt+0x1DF8 = 0x141109770 = pawn+0xE2C (the command in action,
    // `form.command_in_action`): 0x4B / 0xC0 refuse, the magic ids (0x20 0x24 0x28 0x2C 0x30, 0x6E .. 0x7D) refuse
    // FIREMAGIC while a follow-up is queued or no shot has left (+0xD0 < 1).
    let by_state = match state {
        StateData::Jump(j) => !j.flow && !matches!(j.sub, 7 | 8),
        StateData::FireMagic(m) => m.flags & super::fire_magic::flag::FOLLOW_UP == 0 && !s.combat.finish_end,
        _ => match state.attack() {
            Some(a) if matches!(id, 12..=14 | 75..=90 | 156 | 157 | 159..=161) => !a.next_queued && !s.combat.finish_end,
            _ => true,
        },
    };
    let in_action = s.combat.form.command_in_action;
    let by_command = match in_action {
        0x4B | 0xC0 => false,
        0x20 | 0x24 | 0x28 | 0x2C | 0x30 | 0x6E..=0x7D => !matches!(state, StateData::FireMagic(m) if !m.shot_left_unchained()),
        _ => true,
    };
    by_state && by_command
}

/// Slot 191 0x141447640: the gate, then SetState(40) (0x1405B9FE0: the queue cleared, the new state begins in this frame's
/// MainProc).
pub(crate) fn request(state: &StateData, cx: &mut Ctx) -> bool {
    if !gate(state, cx.pawn) {
        return false;
    }
    cx.pawn.queue.set_state(ShooterMode::RESET);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sticks_go_through_the_dead_zone_and_the_cameras_play() {
        assert_eq!(massage(Vec2::new(0.25, -0.2)), Vec2::ZERO);
        assert!((massage(Vec2::new(1.0, 0.0)).x - 1.0).abs() < 1e-6);
        assert_eq!(play(Vec2::new(0.1, -0.05)), Vec2::ZERO);
        assert!((play(Vec2::new(-1.0, 0.55)).x + 1.0).abs() < 1e-6 && (play(Vec2::new(0.0, 0.55)).y - 0.5).abs() < 1e-6);
        assert!((dir(0.0, 90.0) - Vec3::Y).length() < 1e-6);
    }
}
