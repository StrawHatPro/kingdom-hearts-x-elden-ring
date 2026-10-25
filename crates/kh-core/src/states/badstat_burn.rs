//! `TresPlayerState_BadStatBurn` (id 124, vtable 0x146908D50; base `TresCharState_BadStatBurn`):
//! the bad status BURN (17) as a state. docs/sora-combat-spec.md 9.9.4, from the decompiled C of
//! Begin 0x14134E6E0 -> 0x141508620 (verdict `same`), Tick 0x1413530B0 -> 0x14150E960 (branch
//! order checked in the disassembly), End 0x141350220 -> 0x14150B720, the launch handler
//! 0x141512610 and the parameter getters 0x14150CA80 / 0x14150C9F0 / 0x14150CA20 / 0x14150CA50
//! (`TresDamageCommonParam` +0xB8 .. +0xC4 through the game mode's common-param class; the
//! asset's values: move speed 300, gravity scale 2, jump height max 200 / min 200).
//!
//! He hops about on fire: `BadStatBurnStart` (0.167 s, `TresJumpStart` at 0.133 s) launches a hop
//! of 200 cm; in the air (`BadStatBurnLoop`) he is driven forward with full input at max speed
//! 300 under gravity scale 2; on landing he turns to a random direction and hops again. Every
//! hop after the first costs Ceil(5 % of max HP) through the normal HP rule (Second Chance
//! applies; it can kill). While it lasts hits take HP without a flinch (`m_bIsNoDamageResponse`)
//! and the weapon is hidden. The status's time counts down only while this state runs; when it
//! is gone (or cured) he stops on the next landing and recovers (`BadStatBurnEnd` 1.2 s,
//! `TresEnableChange` at 0.9 s).

use glam::Vec3;

use super::badstat_stun::play_clip;
use super::common::on_ground;
use super::{PlayerState, StateId};
use crate::anim::{AnimId, Notifies};
use crate::combat::badstat::{self, kind};
use crate::host::{EventKind, TargetId};
use crate::pawn::{Ctx, Pawn};
use crate::sora::{wrap_degrees, MovementState};
use crate::status::HitOnSora;

/// `TresDamageCommonParam`: `m_BadStatBurnMoveSpeed`, `m_BadStatBurnGravityScale`,
/// `m_BadStatBurnJumpHeightMax` / `Min` (native 480 / 5 / 350 / 50; the game's assets
/// `EnemyCommonParam` and `PlayerDamageCommonParam` both give 300 / 2 / 200 / 200).
pub mod param {
    pub const MOVE_SPEED: f32 = 300.0;
    pub const GRAVITY_SCALE: f32 = 2.0;
    pub const JUMP_HEIGHT_MAX: f32 = 200.0;
    pub const JUMP_HEIGHT_MIN: f32 = 200.0;
    /// pawn vt+0x1870(0.05, 1 = of max HP, ...) at each hop after the first ([I] 0x1415126DB).
    pub const HOP_DAMAGE_RATE: f32 = 0.05;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BadstatBurn {
    sub: u8,
    /// state+0x8C bit 0 and +0x94: a direction given from outside for the next hop (its writer
    /// was not found: never set here; the code that reads it is kept).
    steer: Option<f32>,
    /// state+0x98: hops launched.
    pub hops: i32,
    /// state+0xA0 bit 0 (player): the controller's burn rumble is on.
    rumble: bool,
}

impl BadstatBurn {
    /// The base class's `Tick` (0x14150E960).
    fn base_tick(&mut self, cx: &mut Ctx, _dt: f32) {
        let s = &mut *cx.pawn;

        match self.sub {
            // 0 / 1: the start clip (1 = the hop was launched); at its end the loop (mode 3).
            0 | 1 => {
                if s.anim.ended {
                    play_clip(s, "BadStatBurnLoop", 3);
                    self.sub = 2;
                }
            }
            2 => {
                if !on_ground(s) {
                    // In the air: full input along the facing (a direction given from outside
                    // is taken first).
                    if let Some(yaw) = self.steer.take() {
                        s.yaw = wrap_degrees(yaw);
                    }
                    let a = s.yaw.to_radians();
                    s.cmc.control_input = Vec3::new(a.cos(), a.sin(), 0.0);
                    return;
                }
                if !s.combat.bad.is(kind::BURN) {
                    // The status is gone: he stops and recovers.
                    s.cmc.stop_movement();
                    s.cmc.stop_jumping();
                    play_clip(s, "BadStatBurnEnd", 2);
                    s.queue.flags.enable_change = true;
                    s.combat.bad.no_reaction = false;
                    self.sub = 3;
                    self.steer = None;
                    self.rumble = false;
                    return;
                }
                if s.cmc.velocity.z <= 0.0 {
                    // Landed, still burning: a new direction, the start clip again.
                    s.cmc.stop_movement();
                    s.cmc.stop_jumping();
                    if Some(s.anim.name) != AnimId::from_name("BadStatBurnStart") {
                        if self.steer.is_none() {
                            // [I] 0x14150EB7C: rand24 x (360 / 2^24) - 180.
                            s.yaw = wrap_degrees(random(s) * 360.0 - 180.0);
                        }
                        play_clip(s, "BadStatBurnStart", 2);
                        self.steer = None;
                        self.sub = 0;
                        return;
                    }
                }
                self.steer = None;
            }
            // 3: the end clip.
            3 => {
                if s.anim.ended {
                    s.queue.do_finish();
                }
            }
            _ => s.queue.do_finish(),
        }
    }

    pub fn new() -> BadstatBurn {
        BadstatBurn { sub: 0, steer: None, hops: 0, rumble: false }
    }
}

/// A uniform [0, 1) from the pawn's stream (the game draws from the engine's global xorshift128).
fn random(s: &mut Pawn) -> f32 {
    let rng = &mut s.combat.rng;
    if *rng == 0 {
        *rng = 0x9E37_79B9;
    }
    *rng ^= *rng << 13;
    *rng ^= *rng >> 17;
    *rng ^= *rng << 5;
    (*rng >> 8) as f32 / 16_777_216.0
}

impl PlayerState for BadstatBurn {
    const ID: StateId = StateId::BadstatBurn;

    fn sub_step(&self) -> u8 {
        self.sub
    }

    /// `Begin`: the actor levelled, `StopMovement`, MaxWalkSpeed = 300, GravityScale = 2, the
    /// weapon hidden (pawn vt+0x1340(0, 0, 1.0)), no damage response (pawn vt+0x2078(1)),
    /// `Play(BadStatBurnStart, mode 2)`, the burn effect (group 2, row 0xC08); the player part:
    /// the HUD lock and the rumble.
    fn begin(&mut self, cx: &mut Ctx) {
        let s = &mut *cx.pawn;
        s.cmc.stop_movement();
        s.cmc.max_walk_speed = param::MOVE_SPEED;
        s.cmc.gravity_scale = param::GRAVITY_SCALE;
        s.cmc.gravity_off = false;
        s.combat.bad.no_reaction = true;
        play_clip(s, "BadStatBurnStart", 2);
        badstat::set_effect(cx, Some(badstat::effect::BURN));
        cx.set_weapon_visible(false);
        badstat::hud_lock(cx, true);
        // The player part (0x14134E6E0): 0x14140DF20(ctrl, 0, 0, 0, 0), then (ctrl, 1, 4, 0, 0): the
        // attack camera's mode 4 (the look point lags behind him: combat/camera.rs).
        crate::combat::camera::off(cx);
        crate::combat::camera::request(cx, true, 4, crate::host::TargetId::NONE);
        self.rumble = true;
    }

    /// `Tick` (0x14150E960), then the player part (0x1413530B0: the rumble stops and the attack
    /// camera is switched off once the end clip plays, sub-step 3).
    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        self.base_tick(cx, dt);
        if self.sub == 3 {
            crate::combat::camera::off(cx);
        }
    }

    /// `End`: the jump latch cleared, MaxWalkSpeed and GravityScale back to the class defaults,
    /// the weapon shown again, the damage response back; the player part: HUD lock and rumble off.
    fn end(&mut self, cx: &mut Ctx) {
        let s = &mut *cx.pawn;
        let p = *s.params();
        s.cmc.stop_jumping();
        s.cmc.max_walk_speed = p.speed_run;
        s.cmc.gravity_scale = p.gravity_scale;
        s.combat.bad.no_reaction = false;
        let out = s.actions.weapon_out;
        cx.set_weapon_visible(out);
        badstat::hud_lock(cx, false);
        // (0x141350220) the attack camera off.
        crate::combat::camera::off(cx);
        self.rumble = false;
    }

    /// `TresAnimNotify_JumpStart` -> 0x141512610, only in sub-step 0: the jump latch (pawn slot
    /// 530), JumpZVelocity = the movement's height-to-speed (vt+0xC50 0x140578010:
    /// sqrt(2 x |gravity x JumpUpGravityScale| x h)) of a height drawn between the minimum and
    /// the maximum, sub-step 1; from the second hop on Ceil(5 % of max HP) damage.
    fn on_notifies(&mut self, cx: &mut Ctx, notifies: &Notifies) {
        if !notifies.jump_start || self.sub != 0 {
            return;
        }
        let s = &mut *cx.pawn;
        let p = *s.params();
        let height = param::JUMP_HEIGHT_MIN + random(s) * (param::JUMP_HEIGHT_MAX - param::JUMP_HEIGHT_MIN);
        let power = (2.0 * s.cmc.rise_gravity(&p) * height).sqrt();
        s.cmc.press_jump(power);
        self.sub = 1;
        if self.hops > 0 {
            hop_damage(cx);
        }
        self.hops += 1;
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}

/// `PercentDamage(0.05, of max HP)` (pawn slot 782 0x1405A3970): nothing while he is invincible
/// or takes no damage; Ceil(rate x MaxHP) through `ApplyHP` (no force: Second Chance and
/// Withstand Combo apply); at 0 HP the die path.
fn hop_damage(cx: &mut Ctx) {
    let s = &mut *cx.pawn;
    if s.status.damage_invalid || s.combat.no_damage_timer > 0.0 || s.status.hp < 1 {
        return;
    }
    let amount = (param::HOP_DAMAGE_RATE * s.status.hp_max as f32).ceil();
    if amount <= 0.0 {
        return;
    }
    let outcome = s.status.apply_damage_to_sora(amount, &HitOnSora { percent: true, ..HitOnSora::default() });
    if outcome.dead || s.status.hp < 1 {
        let a = s.yaw.to_radians();
        let knock = -Vec3::new(a.cos(), a.sin(), 0.0);
        s.queue.clear();
        s.queue.push_state(super::die::Die::new(knock, false));
        s.event(EventKind::PlayerDied { causer: TargetId::NONE });
    }
}
