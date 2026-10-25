//! `TresPlayerState_AthleticFlow` (id 42): the airstep travel / arrival state. OWNER: the magic
//! implementer; the reading that governs for Begin / Tick / End and the travel is
//! docs/sora-actions-spec.md II.5 ("Athletic Flow"), with docs/sora-magic-spec.md part H for the
//! arrival procedures; the target choice in aim mode is `magic/shotlock.rs`.
//!
//! The state flies the pawn in Flying mode straight at the (re-read every tick) target point,
//! invincible, gravity off: one leg per entry of the controller's target list; the leg speed
//! `v0 = max(2 d / T, min(60 d, 6000))` is fixed at the leg's start, 3000 inside the last 600 cm,
//! arrival at `d < m_CloseRange + 100`. Intermediate legs fly with world collision OFF (pawn
//! vt+0xFF0(0): `Cmc::world_collision_off`), the last leg with it on (H 4.4 `Select`).
//!
//! Arrivals:
//! - an enemy (kinds 1 - 3): the hit effect, one tick later `AthleticSlideEnd` (a 150 cm hop
//!   back), the 0.25 hit pause between `TresEnableInput` and `TresEnableChange` when the enemy
//!   can be locked; in the tail the attack press starts the FLOW ATTACK (`attack::flow_attack`,
//!   forced kind 0x36, clip 1181);
//! - a level point: the snap, then POLE / END / KICK / NEXT. KICK is the state's own use of
//!   `WallKickF` on the first intermediate stop (started at clip time 0.2); `TresPlayerState_
//!   WallKick` is never entered from here (II.5: nothing in this state calls `TryWallKick`).
//! - abort (jump press, target lost, stuck, the last leg's time-out): `SlideEnd` in place; cut
//!   on an intermediate leg, the landing fix puts him at a free place first.
//!
//! DEAD IN THE SHIPPED CODE, kept as the code has them: the Superslide of the tail (its latch
//! +0x135 is written only by `TryFlow`, whose gate refuses state 42: II.5 "Follow-ups") and the
//! kick projectile `ATHLETIC_KICK` of the Shot notify (needs the attack latch, which only an
//! enemy stop can set, and a `TresShot`, which only `WallKickF` has - at 0.199 s, before the 0.2 s
//! the kick starts it at).
//!
//! HAND-OFFS (integration pass, 2026-10-12; magic spec H 4.6, actions II.5 "Arrival branches"):
//! POLE = `pole_check` (a kind-6 point whose pole the probe finds: `TryPoleTurn` on the last
//! entry -> POLE_TURN 24; else the in-state pole stop, mode 2, SIMPLIFIED: see `pole_stop`);
//! END = `end_check` (the ledge probe -> push DANGLE 6 with the climb-up variant).
//! STUB: the enemy-turn swing of ETURN enemies (kind 2 arrives like a plain enemy).
//!
//! Port notes: the "requested velocity" of the flying move is written as the velocity itself
//! (`MaxAcceleration = 60 v0` reaches it within one 60 Hz step); body collision (pawn vt+0x1010)
//! has no switch here: he is invincible for the whole state; the airstep camera
//! (`ATresCameraAthleticFlow`) is the host's.

use glam::Vec3;

use super::common::turn;
use super::{attack, dangle, pole_turn, super_slide, PlayerState, StateData, StateId};
use crate::actions::probes;
use crate::anim::{AnimId, NotifyClass, NotifyPhase, PlayArgs};
use crate::command::MenuState;
use crate::host::{Attach, EffectId, EffectSpawn, TargetFlags, TargetId};
use crate::input::PadButton;
use crate::magic::projectile::{self, SpawnArgs, StageDef};
use crate::magic::shotlock::{self, AirEntry};
use crate::magic::tables::Sheet;
use crate::pawn::{Ctx, Pawn};
use crate::sora::MovementState;

/// The delegate (state+0xD0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Travel,
    Kick,
    /// Mode 2: the in-state pole stop (slot 118 0x141347750).
    Pole,
    Enemy,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AthleticFlow {
    sub: u8,
    mode: Mode,
    /// +0x90 leg time left; +0x98 pose variant; +0x9C / +0xA0 upward / downward legs.
    leg_time: f32,
    variant: u8,
    ups: u32,
    downs: u32,
    /// +0xB1: the jump-press latch (one frame).
    abort: bool,
    /// +0xD4, +0xD8.
    index: i32,
    count: i32,
    /// The current entry.
    entry: Option<AirEntry>,
    /// +0xEC the actor location at Begin (the landing fall-back +0x104 starts as it); +0xF8 the
    /// reference point at Begin; +0x140 the target point; +0x168 the stop direction.
    start_location: Vec3,
    start_ref: Vec3,
    target_point: Vec3,
    hit_dir: Vec3,
    /// +0x128 the attack latch; +0x135 the guard latch (no reachable writer).
    input_attack: bool,
    input_flow: bool,
    /// +0x12C.
    slow_rate: f32,
    /// +0x138: 0x01 lock prompt shown, 0x02 enemy target, 0x10 lock taken (attack accepted from
    /// now), 0x20 first stop, 0x40 last entry, 0x80 hit pause.
    flags: u32,
    /// +0x13C.
    stuck: f32,
    /// The leg speed (MaxFlySpeed).
    speed: f32,
    enable_input: bool,
    /// +0x174: the reference point at the arrival, before the snap onto the point.
    arrive_ref: Vec3,
    /// The in-state pole stop: the grab location, the bar flag, the approach timer (+0x1C4) and
    /// then the loop time left.
    pole_grab: Vec3,
    pole_bar: bool,
    pole_timer: f32,
}

const F_PROMPT: u32 = 0x01;
const F_ENEMY: u32 = 0x02;
const F_LOCK: u32 = 0x10;
const F_FIRST: u32 = 0x20;
const F_LAST: u32 = 0x40;
const F_PAUSE: u32 = 0x80;

/// For hosts and tests (`Sora::airstep`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AirstepView {
    /// 0 travel, 1 kick stop, 4 enemy stop.
    pub mode: u8,
    pub sub: u8,
    pub leg: i32,
    pub legs: i32,
    pub flags: u32,
    pub target_point: Vec3,
    pub attack_latched: bool,
    /// The intermediate hop's switch.
    pub world_collision_off: bool,
    /// (integration pass) The enemy of the current entry (`TargetId::NONE` for a level point):
    /// whom the attack prompt (flag 0x01) is about.
    pub target: TargetId,
}

pub(crate) fn view(state: &StateData, pawn: &Pawn) -> Option<AirstepView> {
    match state {
        StateData::AthleticFlow(a) => Some(AirstepView {
            mode: match a.mode {
                Mode::Travel => 0,
                Mode::Kick => 1,
                Mode::Pole => 2,
                Mode::Enemy => 4,
            },
            sub: a.sub,
            leg: a.index,
            legs: a.count,
            flags: a.flags,
            target_point: a.target_point,
            attack_latched: a.input_attack,
            world_collision_off: pawn.cmc.world_collision_off,
            target: a.entry.map_or(TargetId::NONE, |e| e.target),
        }),
        _ => None,
    }
}

impl AthleticFlow {
    /// Reset (0x14133CFB0): slow rate 0.5 / 0.1, flag 0x20 set.
    pub const RESET: AthleticFlow = AthleticFlow { sub: 0, mode: Mode::Travel, leg_time: 0.0, variant: 1, ups: 0, downs: 0, abort: false, index: -1, count: 0, entry: None, start_location: Vec3::ZERO, start_ref: Vec3::ZERO, target_point: Vec3::ZERO, hit_dir: Vec3::X, input_attack: false, input_flow: false, slow_rate: 0.5, flags: F_FIRST, stuck: 0.0, speed: 0.0, enable_input: false, arrive_ref: Vec3::ZERO, pole_grab: Vec3::ZERO, pole_bar: false, pole_timer: 0.0 };

    fn reference(cx: &Ctx) -> Vec3 {
        cx.pawn.cmc.location + Vec3::Z * Sheet::Shot.num("airstep2.select.eye_height")
    }

    /// The component's location, while it and its owner are alive.
    fn component_point(cx: &Ctx, e: &AirEntry) -> Option<Vec3> {
        if e.target != TargetId::NONE {
            cx.target(e.target).filter(|t| t.flags.has(TargetFlags::ALIVE)).map(|t| t.center)
        } else {
            cx.pawn.magic.airstep_points.iter().find(|p| p.id == e.point).map(|p| p.position)
        }
    }

    /// Slot 123 `TargetPoint` (0x141336520): the component's location; for an entry that is not
    /// the last, the warp sweep (pawn vt+0x2750 0x141431630: radius 60) from the midpoint to the
    /// next point toward this one: a hit gives the hit location, 60 lower, then 1 up when that is
    /// below the sweep's start and 1 down when above. So an intermediate point is approached on
    /// the side that faces the next one.
    fn point_of(&self, cx: &Ctx) -> Option<Vec3> {
        let entry = self.entry?;
        let p = Self::component_point(cx, &entry)?;
        if self.flags & F_LAST != 0 {
            return Some(p);
        }
        let next = cx.pawn.magic.aim.airstep_targets.get(self.index as usize + 1).and_then(|e| Self::component_point(cx, e));
        let Some(n) = next else { return Some(p) };
        let r = Sheet::Shot.num("airstep2.landing.sweep_radius");
        let from = (p + n) * 0.5;
        match cx.world.sweep_capsule(from, p, r, r) {
            Some(hit) => {
                let mut out = from + (p - from) * hit.time;
                out.z -= r;
                if out.z < from.z {
                    out.z += 1.0;
                } else if out.z > from.z {
                    out.z -= 1.0;
                }
                Some(out)
            }
            None => Some(p),
        }
    }

    /// Slot 124 `FindLanding(from, to, .., out, 60)` (0x141331780): the first hit of a sphere
    /// sweep gives the hit location with `z - 60 + 1` (false); no hit gives `to` (true).
    fn find_landing(cx: &Ctx, from: Vec3, to: Vec3) -> (Vec3, bool) {
        let s = Sheet::Shot;
        let r = s.num("airstep2.landing.sweep_radius");
        match cx.world.sweep_capsule(from, to, r, r) {
            Some(hit) => {
                let mut out = from + (to - from) * hit.time;
                out.z = out.z - r + s.num("airstep2.landing.z_lift");
                (out, false)
            }
            None => (to, true),
        }
    }

    /// `LandingFix()` (H 4.5): only when the flight is cut while world collision is off (an
    /// intermediate hop): from the target back to him, else from the start; `SetActorLocation`
    /// without a sweep. With the target gone the landing is the place the airstep began (+0x104).
    fn landing_fix(&mut self, cx: &mut Ctx) {
        let reference = Self::reference(cx);
        let mut landing = self.start_location;
        if let Some(point) = self.entry.and_then(|e| Self::component_point(cx, &e)) {
            let (out, ok) = Self::find_landing(cx, point, reference);
            landing = out;
            if !ok {
                landing = Self::find_landing(cx, self.start_ref, reference).0;
            }
        }
        cx.pawn.cmc.location = landing;
    }

    /// The END / list-used-up exit: `SlideEnd` in place (mode 0: its root motion is not applied).
    fn slide_end(&mut self, cx: &mut Ctx) {
        cx.pawn.cmc.world_collision_off = false;
        cx.pawn.cmc.stop_movement();
        cx.pawn.anim.play_with(AnimId::SlideEnd, PlayArgs::mode(0, 0.0));
        self.mode = Mode::Travel;
        self.sub = 2;
    }

    /// ABORT (0x141349847): collision on, `SlideEnd`; cut on an intermediate leg: the landing fix.
    fn abort_flight(&mut self, cx: &mut Ctx) {
        self.slide_end(cx);
        if self.flags & F_LAST == 0 {
            self.landing_fix(cx);
        }
    }

    /// Slot 115 `SetMode(0)` (0x1413311D0): the next travel leg. False when the list is used up.
    fn next_leg(&mut self, cx: &mut Ctx) -> bool {
        let list = &cx.pawn.magic.aim.airstep_targets;
        self.index += 1;
        let Some(entry) = list.get(self.index as usize).copied() else {
            // (Reached past the end only by a path the game leaves undefined: ported as an abort.)
            self.abort_flight(cx);
            return false;
        };
        self.entry = Some(entry);
        self.count = list.len() as i32;
        if self.index + 1 >= self.count {
            // The last entry: body and world collision on (and the airstep camera, mode 0).
            self.flags |= F_LAST;
            cx.pawn.cmc.world_collision_off = false;
        } else {
            // An intermediate hop: pawn vt+0x1010(0, 0); vt+0xFF0(0).
            cx.pawn.cmc.world_collision_off = true;
        }
        self.mode = Mode::Travel;
        self.sub = 0;
        self.travel(cx, 0.0);
        true
    }

    /// Slot 116: the travel (mode 0).
    fn travel(&mut self, cx: &mut Ctx, dt: f32) {
        let s = Sheet::Shot;
        let Some(entry) = self.entry else {
            cx.pawn.queue.do_chain();
            return;
        };
        match self.sub {
            0 => {
                // The player type leaves the 0.1 slow motion the controller set.
                cx.set_time_dilation(crate::magic::MASK_PLAYER, 1.0);
                self.input_attack = false;
                self.leg_time = s.num("airstep2.travel.slide_time");
                let reference = Self::reference(cx);
                self.target_point = self.point_of(cx).unwrap_or(cx.pawn.cmc.location);
                let d = self.target_point - reference;
                let pitch = if d.length_squared() > 1.0e-8 { (d.z / d.length()).clamp(-1.0, 1.0).asin().to_degrees() } else { 0.0 };
                self.variant = 1;
                if pitch > s.num("airstep2.travel.pitch_up") {
                    self.variant = 3;
                    self.ups += 1;
                } else if pitch < s.num("airstep2.travel.pitch_down") {
                    self.variant = 2;
                    self.downs += 1;
                }
                // The clip: AthleticSlide / AthleticSlide2 once on odd counts, else the loop of
                // the variant (SuperFlightLoop is not in the anim set: nothing new is played).
                let anim = &mut cx.pawn.anim;
                if self.variant == 3 && self.ups % 2 == 1 {
                    anim.play_with(AnimId::AthleticSlide, PlayArgs::mode(0, 0.0));
                } else if self.variant == 2 && self.downs % 2 == 1 {
                    anim.play_with(AnimId::AthleticSlide2, PlayArgs::mode(0, 0.0));
                } else if self.variant == 2 {
                    anim.play_with(AnimId::SonicSlideLoop, PlayArgs::mode(1, 0.0));
                } else if self.variant == 1 {
                    anim.play_with(AnimId::SuperSlideLoop, PlayArgs::mode(1, 0.0));
                }
                self.sub = 3;
            }
            3 => {
                let reference = Self::reference(cx);
                self.target_point = self.point_of(cx).unwrap_or(self.target_point);
                let v = self.target_point - reference;
                let d = v.length();
                // v0 = max(2 d / T, min(60 d, MyAthleticSlideSpeed)).
                self.speed = (2.0 * d / self.leg_time).max((s.num("airstep2.travel.speed_dist_gain") * d).min(s.num("airstep2.travel.slide_speed")));
                turn(cx.pawn, v.y.atan2(v.x).to_degrees(), dt * s.num("airstep2.travel.launch_turn_rate"));
                self.stuck = 0.0;
                self.sub = 1;
            }
            1 => {
                if self.leg_time <= 0.0 {
                    cx.pawn.cmc.stop_movement();
                    if self.flags & F_LAST == 0 {
                        // The hop timed out: on to the next entry.
                        self.next_leg(cx);
                        return;
                    }
                    self.abort = true;
                }
                let point = self.point_of(cx);
                // The stuck test (slot 122): 0.2 s below half the commanded speed.
                let pawn = &mut *cx.pawn;
                if s.num("airstep2.stuck.speed_ratio") * pawn.cmc.velocity.length() > pawn.cmc.displacement_velocity.length() {
                    self.stuck += dt;
                } else {
                    self.stuck = 0.0;
                }
                if self.abort || point.is_none() || self.stuck > s.num("airstep2.stuck.time") {
                    self.abort_flight(cx);
                    return;
                }
                self.target_point = point.unwrap_or(self.target_point);
                self.leg_time -= dt;
                if pawn.anim.ended && pawn.anim.name == AnimId::AthleticSlide {
                    pawn.anim.play_with(AnimId::SuperSlideLoop, PlayArgs::mode(1, -1.0));
                }
                let reference = Self::reference(cx);
                let v = self.target_point - reference;
                let d = v.length();
                if d < entry.close_range + s.num("airstep2.travel.arrive_margin") {
                    if self.flags & F_ENEMY != 0 {
                        // "ENEMY": OnReachEnemy(dir = normalize(startRef - target), owner, component):
                        // the hit effect (slot 120) at the reference point, then mode 4.
                        self.hit_dir = (self.start_ref - self.target_point).normalize_or_zero();
                        cx.spawn_effect(EffectSpawn { id: EffectId(u32::MAX), name: "vp_ffa_arv0", at: Attach::World(reference), rotation: Vec3::ZERO, scale: 1.0, persistent: false });
                        self.mode = Mode::Enemy;
                        self.sub = 0;
                        self.enemy(cx);
                    } else {
                        // A level point: SetActorLocation(target, sweep) - with the root's world
                        // responses still as the leg had them - then collision on; POLE / END / KICK / NEXT.
                        let (w, pawn) = (cx.world, &mut *cx.pawn);
                        let from = pawn.cmc.location;
                        let to = self.target_point;
                        let p = *pawn.params();
                        let centre = Vec3::Z * p.capsule_center_z;
                        let hit = if pawn.cmc.world_collision_off { None } else { w.sweep_capsule(from + centre, to + centre, p.capsule_radius, p.capsule_half_height) };
                        pawn.cmc.location = match hit {
                            Some(hit) => from + (to - from) * hit.time,
                            None => to,
                        };
                        pawn.cmc.world_collision_off = false;
                        // +0x174 = the reference point before the snap.
                        self.arrive_ref = reference;
                        if self.pole_check(cx, &entry) {
                            return;
                        }
                        if self.flags & F_LAST != 0 {
                            // "END": the ledge probe -> DANGLE, else SlideEnd.
                            if self.end_check(cx) {
                                cx.pawn.queue.do_chain();
                                return;
                            }
                            self.slide_end(cx);
                        } else if self.flags & F_FIRST != 0 {
                            // "KICK" (slot 117 0x141347510): the private use of WallKickF, started
                            // at clip time 0.2 (anim vt+0xF0 = SetAnimTime), facing the point.
                            self.flags &= !F_FIRST;
                            self.hit_dir = (to - reference).normalize_or_zero();
                            let pawn = &mut *cx.pawn;
                            pawn.cmc.stop_movement();
                            pawn.anim.play_with(AnimId::WallKickF, PlayArgs::mode(2, 0.0));
                            pawn.anim.set_time(s.num("airstep2.kick.anim_start"));
                            if self.hit_dir.truncate().length_squared() > 1.0e-8 {
                                pawn.yaw = self.hit_dir.y.atan2(self.hit_dir.x).to_degrees();
                            }
                            self.mode = Mode::Kick;
                            self.sub = 2;
                        } else {
                            // "NEXT".
                            cx.pawn.cmc.stop_movement();
                            self.next_leg(cx);
                        }
                    }
                    return;
                }
                // MOVE: v = max(6 d, MaxFlySpeed) clamped by MaxFlySpeed; 3000 inside 600 cm.
                let dir = v / d.max(1.0e-6);
                let mut speed = (s.num("airstep2.travel.chase_gain") * d).max(self.speed).min(self.speed);
                if d < s.num("airstep2.travel.near_dist") {
                    speed = s.num("airstep2.travel.near_speed").min(self.speed);
                }
                if speed * dt * s.num("airstep2.travel.overshoot_factor") > d {
                    return;
                }
                turn(cx.pawn, v.y.atan2(v.x).to_degrees(), dt * s.num("airstep2.travel.turn_rate"));
                cx.pawn.cmc.velocity = dir * speed;
            }
            // Sub 2 (and any other): the clip's end finishes the state.
            _ => {
                if cx.pawn.anim.ended {
                    cx.pawn.queue.do_chain();
                }
            }
        }
    }

    /// Slot 121 `SetEnemyLock(b)` (0x1413342C0): only for an enemy target. On: when the enemy can
    /// be locked on, the lock prompt and flags 0x11 (the attack is accepted from now; the hit
    /// pause is armed). Off: the prompt goes.
    fn set_enemy_lock(&mut self, cx: &mut Ctx, on: bool) {
        if self.flags & F_ENEMY == 0 {
            return;
        }
        if on && self.flags & F_PROMPT == 0 {
            let lockable = self.entry.and_then(|e| cx.target(e.target)).is_some_and(|t| t.flags.has(TargetFlags::ALIVE | TargetFlags::LOCKABLE));
            if lockable {
                self.flags |= F_PROMPT | F_LOCK;
            }
        } else if !on {
            self.flags &= !F_PROMPT;
        }
    }

    /// Slot 119 (0x141346600): the enemy stop (mode 4), a plain enemy (no turn answer).
    fn enemy(&mut self, cx: &mut Ctx) {
        match self.sub {
            0 => {
                cx.pawn.cmc.stop_movement();
                self.sub = 6;
                self.input_attack = false;
                self.set_enemy_lock(cx, true);
            }
            6 => {
                // m_pEnemyTurn invalid: Play("AthleticSlideEnd", blend 0, mode 2).
                self.enable_input = false;
                cx.pawn.queue.flags.enable_change = false;
                cx.pawn.anim.play_with(AnimId::AthleticSlideEnd, PlayArgs::mode(2, 0.0));
                self.sub = 3;
            }
            3 | 4 => {
                if self.sub == 3 && self.enable_input {
                    if self.flags & F_LOCK != 0 {
                        // SetObjTypeDilation(player, 0.25, 0.05).
                        cx.set_time_dilation(crate::magic::MASK_PLAYER, self.slow_rate);
                        self.flags |= F_PAUSE;
                    }
                    self.sub = 4;
                }
                if self.sub == 4 && cx.pawn.queue.flags.enable_change {
                    if self.flags & F_PAUSE != 0 {
                        cx.set_time_dilation(crate::magic::MASK_PLAYER, 1.0);
                        self.flags &= !F_PAUSE;
                    }
                    if self.flags & F_PROMPT != 0 {
                        self.set_enemy_lock(cx, false);
                    }
                    self.sub = 7;
                }
            }
            _ => {
                // Sub 7, the shared tail.
                let entry = self.entry;
                if cx.pawn.anim.ended {
                    cx.pawn.queue.do_chain();
                } else if self.input_flow {
                    // Request interface slot 73 (1, null): push SUPER_SLIDE kind 1, when nothing is queued.
                    if cx.pawn.queue.nothing_queued() {
                        super_slide::push(cx.pawn, None);
                    }
                    cx.pawn.queue.do_chain();
                } else if self.input_attack {
                    // SetState(ATTACK) with the target component; forced kind 0x38 after an enemy
                    // turn (kind 2), 0x36 for a plain enemy (kind 1) (0x141347455 / 0x141347465).
                    let (target, kind) = entry.map_or((TargetId::NONE, 0), |e| (e.target, e.kind));
                    let forced = match kind {
                        2 => Sheet::Shot.int("airstep2.enemy.attack_kind_turn") as u8,
                        1 => Sheet::Shot.int("airstep2.enemy.attack_kind") as u8,
                        _ => 0,
                    };
                    attack::flow_attack(cx.pawn, target, forced);
                    cx.pawn.queue.do_chain();
                } else if entry.is_some_and(|e| e.kind == 1) && cx.pawn.cmc.displacement_velocity.z == 0.0 {
                    cx.pawn.queue.do_chain();
                }
            }
        }
    }

    /// The attack request on this state (gate slot 44 0x141426D0E: nothing queued and flag 0x10;
    /// request slot 45 0x1414387A1 -> state slot 113 0x1413435F0): flags &= ~0x18, the latch, the
    /// prompt removed. WHO makes the request while the airstep runs was not read (the HUD's
    /// decide on the Attack row, or the aim update, which still owns the controller): the port
    /// takes the attack button while the shotlock panel is up (the aim mode is 3 for the whole
    /// state) or the main list's cursor is on Attack.
    fn attack_input(&mut self, cx: &mut Ctx) {
        let pad = cx.pawn.input.pad;
        let on_attack_row = cx.pawn.command.state == MenuState::ShootFlow || (cx.pawn.command.state == MenuState::MainSel && cx.pawn.command.main_cursor == 0);
        if pad.pressed.has(PadButton::ATTACK) && !pad.held.has(PadButton::LeftShoulder) && on_attack_row && self.flags & F_LOCK != 0 && cx.pawn.queue.nothing_queued() {
            self.flags &= !(F_LOCK | 0x08 | F_PROMPT);
            self.input_attack = true;
        }
    }
}

/// `CanAthleticFlow` + `TryAthleticFlow` (iface slots 96 / 97; actions II.5): `SetState(42)`.
pub(crate) fn try_athletic_flow(state: &mut StateData, cx: &mut Ctx) -> bool {
    if cx.pawn.status.hp < 1 || cx.pawn.magic.command_latch || matches!(state, StateData::Die(_)) || cx.pawn.magic.aim.airstep_targets.is_empty() {
        return false;
    }
    cx.pawn.queue.set_state(AthleticFlow::RESET);
    true
}

/// `JumpRequest` case 42 (0x14143E323): the jump press aborts the flight; no jump starts.
pub(crate) fn jump_request(state: &mut StateData) -> bool {
    if let StateData::AthleticFlow(a) = state {
        a.abort = true;
        return true;
    }
    false
}

impl PlayerState for AthleticFlow {
    const ID: StateId = StateId::Athleticflow;

    fn sub_step(&self) -> u8 {
        self.sub
    }

    /// `Begin` (0x141332F90).
    fn begin(&mut self, cx: &mut Ctx) {
        cx.set_weapon_visible(true);
        let pawn = &mut *cx.pawn;
        pawn.cmc.stop_movement();
        pawn.cmc.set_flying();
        pawn.cmc.gravity_off = true;
        pawn.status.damage_invalid = true;
        self.start_location = pawn.cmc.location;
        self.start_ref = Self::reference(cx);
        self.count = cx.pawn.magic.aim.airstep_targets.len() as i32;
        self.index = -1;
        if self.next_leg(cx) {
            if self.entry.is_some_and(|e| (1..=3).contains(&e.kind)) {
                self.flags |= F_ENEMY;
            }
            self.slow_rate = Sheet::Shot.num("airstep.attack_slow_rate");
        }
    }

    /// `Tick` (0x141341270).
    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        self.attack_input(cx);
        if cx.pawn.queue.flags.enable_change || self.enable_input {
            self.enable_input = true;
        }
        match self.mode {
            Mode::Travel => self.travel(cx, dt),
            Mode::Kick => {
                if cx.pawn.anim.ended {
                    self.next_leg(cx);
                }
            }
            Mode::Pole => self.pole_stop(cx, dt),
            Mode::Enemy => self.enemy(cx),
        }
        self.abort = false;
    }

    /// `End` (0x141334B60).
    fn end(&mut self, cx: &mut Ctx) {
        let pawn = &mut *cx.pawn;
        let p = *pawn.params();
        if pawn.cmc.is_flying() || pawn.cmc.is_custom() {
            pawn.cmc.set_falling();
        }
        pawn.cmc.gravity_off = false;
        pawn.cmc.gravity_scale = p.gravity_scale;
        pawn.cmc.world_collision_off = false;
        pawn.status.damage_invalid = false;
        // Controller vt+0x1948: the aim clean-up follows (all types back to 1.0).
        pawn.magic.aim.airstep_targets.clear();
        shotlock::release_done(cx);
        if self.flags & F_PAUSE != 0 {
            cx.set_time_dilation(crate::magic::MASK_PLAYER, 1.0);
        }
    }

    fn on_notifies(&mut self, cx: &mut Ctx, notifies: &crate::anim::Notifies) {
        if notifies.fired(NotifyClass::TresAnimNotifyEnableInput) {
            self.enable_input = true;
        }
        // `TresShot` -> slot +0x228 0x1413442A0: only with the attack latch: the kick notify of
        // the controller (no focus), and with the weapon out the projectile `ATHLETIC_KICK` at the
        // reference point with the actor's rotation; then the player type back to 1.0.
        let shot = notifies.events.iter().any(|e| e.class() == NotifyClass::TresAnimNotifyShot && e.phase == NotifyPhase::Instant);
        if shot && self.input_attack {
            let position = Self::reference(cx);
            let a = cx.pawn.yaw.to_radians();
            let direction = Vec3::new(a.cos(), a.sin(), 0.0);
            projectile::spawn(cx, &SpawnArgs { def: StageDef::athletic_kick(), position, direction, cmd: crate::magic::spells::cmd::SHOOTFLOW, shoot_flow: 37, finish: false, situation: false, full_mp: false, combo: [0; 4], target: TargetId::NONE, attached: None, fire: true });
            cx.set_time_dilation(crate::magic::MASK_PLAYER, 1.0);
        }
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}

// ---------------------------------------------------------------- hand-offs (integration pass)
// The arrival branches POLE and END with the states the actions side built (pole_turn.rs,
// dangle.rs, actions/probes.rs).

impl AthleticFlow {
    /// `PoleCheck` (slot 126 0x1413321B0; magic spec H 4.6): only for a kind-6 point;
    /// `range = max(m_CloseRange + 100 - 75, 125)`; the pole probe (pawn vt+0x2330) at the point
    /// must return the point's own pole. Then: the last entry and `TryPoleTurn` succeeds ->
    /// finish into POLE_TURN (24); else, while the first-arrival flag 0x20 is set, the in-state
    /// pole stop (mode 2); else `StopMovement` and the next leg.
    /// SEAM NOTE: a level point carries no owner actor, so "the point's own pole" is the pole the
    /// probe finds at the point (a host lays a POLE point on its pole).
    fn pole_check(&mut self, cx: &mut Ctx, entry: &AirEntry) -> bool {
        let s = Sheet::Shot;
        if entry.kind as i32 != s.int("airstep2.pole.kind") {
            return false;
        }
        let Some(p) = Self::component_point(cx, entry) else { return false };
        let range = (entry.close_range + s.num("airstep2.pole.range_margin") - s.num("airstep2.pole.range_sub")).max(s.num("airstep2.pole.range_min"));
        let Some(pole) = probes::pole_probe(cx.pawn, cx.world, p, range) else { return false };
        self.hit_dir = (p - self.arrive_ref).normalize_or_zero();
        if self.flags & F_LAST != 0 && pole_turn::try_pole_turn(&StateData::AthleticFlow(*self), cx, &pole) {
            cx.pawn.queue.do_finish();
            return true;
        }
        if self.flags & F_FIRST != 0 {
            // `Select(2)`: sub 0 of the pole stop runs at once.
            self.flags &= !F_FIRST;
            self.pole_bar = pole.horizontal;
            // The grab: the pole's attach point for his location, 100 lower for a turn.
            let l = cx.pawn.cmc.location;
            self.pole_grab = if pole.horizontal { (pole.base1 + pole.base2) * 0.5 } else { probes::closest_on_segment(l, pole.base1, pole.base2) + Vec3::Z * s.num("airstep2.pole.turn_offset_z") };
            let pawn = &mut *cx.pawn;
            pawn.cmc.stop_movement();
            pawn.cmc.set_custom();
            pawn.anim.play_with(if pole.horizontal { AnimId::PoleSwing } else { AnimId::PoleTurn }, PlayArgs::mode(2, -1.0));
            self.pole_timer = s.num("airstep2.pole.approach_time");
            self.mode = Mode::Pole;
            self.sub = 5;
            return true;
        }
        cx.pawn.cmc.stop_movement();
        self.next_leg(cx);
        true
    }

    /// `PoleProc` (slot 118 0x141347750; H 4.6), mode 2. SIMPLIFIED (the spec has the orientation
    /// algebra as "decompiled C, not transcribed"): no mesh offset, no attachment, the actor keeps
    /// its yaw; the movement mode is Custom while he is on the pole (as in this port's POLE_TURN
    /// state). Sub 5: the grab clip, the location easing to the grab point with
    /// `a = clamp(2 timer, 0, 1)`; sub 1: one loop of `PoleTurnLoop` / `PoleSwingLoop`, then the
    /// next leg; on the last entry (`TryPoleTurn` refused) `SlideEnd` in place instead of the
    /// swept landing with the (-0.2, 1.0) root scale.
    fn pole_stop(&mut self, cx: &mut Ctx, dt: f32) {
        let pawn = &mut *cx.pawn;
        if self.sub == 5 {
            if pawn.anim.ended {
                pawn.cmc.location = self.pole_grab;
                let clip = if self.pole_bar { AnimId::PoleSwingLoop } else { AnimId::PoleTurnLoop };
                pawn.anim.play_with(clip, PlayArgs::mode(3, -1.0));
                self.pole_timer = clip.data().length * Sheet::Shot.int("airstep2.pole.loop_count") as f32;
                self.sub = 1;
            } else {
                self.pole_timer -= dt;
                let a = (2.0 * self.pole_timer).clamp(0.0, 1.0);
                pawn.cmc.location = self.pole_grab + (pawn.cmc.location - self.pole_grab) * a;
            }
            return;
        }
        pawn.cmc.location = self.pole_grab;
        self.pole_timer -= dt;
        if self.pole_timer <= 0.0 {
            pawn.cmc.stop_movement();
            pawn.cmc.set_flying();
            if self.flags & F_LAST == 0 {
                self.next_leg(cx);
            } else {
                self.slide_end(cx);
            }
        }
    }

    /// `EndCheck` (slot 127 0x141331C40; H 4.6), for the last entry: the ledge probe
    /// 0x140638740(pawn, out, dz + 300, 300, horizontal distance - 45, 90) with (dz, distance)
    /// from the arrival's reference point to the target point; on success `StopMovement` and
    /// request slot 65: push DANGLE (6) with the ledge data, variant 1 (straight to the climb-up).
    /// The probe of this port takes (up, down): its forward range and radius-90 attempt are not
    /// arguments (actions/probes.rs `ledge_probe`, SIMPLIFIED there).
    fn end_check(&mut self, cx: &mut Ctx) -> bool {
        let s = Sheet::Shot;
        let dz = self.target_point.z - self.arrive_ref.z;
        let Some(ledge) = probes::ledge_probe(cx.pawn, cx.world, dz + s.num("airstep2.ledge.up"), s.num("airstep2.ledge.up")) else { return false };
        cx.pawn.cmc.stop_movement();
        dangle::push(cx.pawn, ledge, true);
        true
    }
}
