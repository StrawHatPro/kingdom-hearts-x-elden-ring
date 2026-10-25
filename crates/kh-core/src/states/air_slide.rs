//! `TresPlayerState_AirSlide` (id 20): Air Slide, Aerial Dodge and Aerial Recovery, and the
//! request `TryAirSlide`: docs/sora-actions-spec.md II.2 (instruction level; addresses are the
//! exe's). Plain Keyblade (weapon kind 1), no water: the slide-shot / back-slide forms and the
//! water variant are not here. The pole probe of the tick and the move-blocked handler (pole
//! turn, wall kick) are tier 2: without poles the probe only clears the stick flag.
//!
//! The displacement is the clip's root motion alone (gravity off, velocity zero), along the
//! facing: Air Slide 1130 cm x the level scale 0.725 / 1.075 / 1.425 in 0.7333 s, Aerial Dodge
//! 700 cm in 0.7 s, Recovery none (0.5 s). Reaction type 11 for the whole state.

use glam::Vec3;

use super::common::on_ground;
use super::{jump, PlayerState, StateData, StateId};
use crate::actions::{self, reaction, A};
use crate::anim::{AnimId, Notifies, NotifyClass, PlayArgs};
use crate::enums::ETresAbilityKind as Ability;
use crate::pawn::{Ctx, Pawn};
use crate::sora::{wrap_degrees, MovementState};

/// State +0x88 stick flag, +0x8C yaw, +0x90 magnitude, +0x98 target pawn, +0xA8 flags (1
/// invincible, 2 Aerial Dodge, 0x20 Aerial Recovery, 0x40 reaction command armed, 0x80 abort).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AirSlide {
    sub: u8,
    stick: bool,
    pub(super) yaw: f32,
    magnitude: f32,
    /// The recovery's attacker (where it is).
    target: Option<Vec3>,
    invincible: bool,
    pub(super) dodge: bool,
    pub(super) recovery: bool,
    armed: bool,
    abort: bool,
    /// The stick exit was taken: the clip plays on in the Jump state (Jump flag 0x40).
    kept: bool,
}

impl AirSlide {
    /// The state object after `Reset` (0x14133CF60).
    pub const RESET: AirSlide = AirSlide { sub: 0, stick: false, yaw: 0.0, magnitude: 0.0, target: None, invincible: false, dodge: false, recovery: false, armed: false, abort: false, kept: false };

    /// Flag 0x80 (slot 114): the next tick chains.
    pub(super) fn aborted(&self) -> bool {
        self.abort
    }

    /// An Aerial Recovery (flag 0x20) facing the attacker at `target` (`JumpRequest` 0x14143E123).
    pub(super) fn recovery(target: Option<Vec3>) -> AirSlide {
        AirSlide { recovery: true, target, ..AirSlide::RESET }
    }

    /// Slot 113 (0x14134C5E0), disarm: the reaction command's window ends.
    fn disarm(&mut self, s: &mut Pawn) {
        if self.armed {
            s.actions.reaction_command = None;
            self.armed = false;
        }
    }
}

/// The gate (slot 68 0x141425780), the rows of the states this port has. `magnitude` = the
/// controller's stick magnitude.
fn can_air_slide(state: &StateData, s: &Pawn, magnitude: f32) -> bool {
    // AirSlideBlocked (vt+0x1E20): pawn+0xE38 > 0 || !(pawn+0xE48 bit 1).
    if s.combat.air_slide_cooldown > 0.0 || !s.actions.air_slide_ok {
        return false;
    }
    // Next queued: nothing or an attack (table 0x141426060).
    if !matches!(s.queue.queued_id(), None | Some(StateId::Attack)) {
        return false;
    }
    let mut ground = on_ground(s);
    let pass = match state {
        StateData::Idle(_) | StateData::Run(_) => true,
        StateData::Jump(j) => !j.flow,
        // GLIDE, FLOATING (case 4 / 5, 0x141425926): bGround = false; pass.
        StateData::Glide(_) | StateData::Floating(_) => {
            ground = false;
            true
        }
        // Attack states: bit 1 && Attack+0x234 bit 7 (0x141425D3E).
        StateData::Attack(a) => s.actions.air_slide_ok && a.cancel,
        StateData::Guard(g) => s.actions.air_slide_ok && g.hit_entry && g.enable_input,
        StateData::DodgeRoll(_) => s.queue.flags.enable_change,
        StateData::Damage(d) => matches!(d.kind.value(), 1 | 2 | 9) && s.queue.flags.enable_change,
        // (magic implementer) FIREMAGIC 18: bit 1 && m_bEnableChange && !(+0xD8 & 0x50) && the
        // combo is not over (II.2 gate table).
        StateData::FireMagic(m) => s.actions.air_slide_ok && m.can_dodge(s),
        // AIR_SLIDE: only out of a recovery, at m_bEnableChange (0x141425A38).
        StateData::AirSlide(a) => a.recovery && s.queue.flags.enable_change,
        #[allow(unreachable_patterns)]
        _ => false,
    };
    if !pass {
        return false;
    }
    // Weapon kind 1 with the stick below 0.1 FAILS (0x141425F00).
    if magnitude < A.airslide_stick_min {
        return false;
    }
    let (slide, dodge) = (actions::has(s, Ability::Airslide), actions::has(s, Ability::Airdodge));
    if s.actions.battle {
        (slide || dodge) && !ground
    } else {
        slide && !ground
    }
}

/// (XSI) The gate as Sonic Dash asks it (abilities/xs/sonic.rs: the same states, cool-down and abilities).
pub(crate) fn gate(state: &StateData, s: &Pawn, magnitude: f32) -> bool {
    can_air_slide(state, s, magnitude)
}

/// `TryAirSlide(yaw, mag, bKeep = 0)` (slot 69 0x141436D20).
pub(crate) fn try_air_slide(state: &mut StateData, cx: &mut Ctx, yaw: f32, magnitude: f32) -> bool {
    let s = &mut *cx.pawn;
    if !can_air_slide(state, s, magnitude) {
        return false;
    }
    // dodge = battle && HasAbility(AIRDODGE) (0x14143705B): Aerial Dodge whether or not AIRSLIDE.
    let dodge = s.actions.battle && actions::has(s, Ability::Airdodge);
    s.queue.clear();
    // mag >= 0.1: +0x8C = yaw; +0x88 = 1 (the request never writes +0x90).
    s.queue.push_state(AirSlide { dodge, stick: true, yaw, ..AirSlide::RESET });
    true
}

/// `MoveInput(yaw, mag)` (0x1414406A7), case AIR_SLIDE: +0x8C = yaw; +0x88 = 1; +0x90 = mag.
pub(crate) fn move_input(state: &mut StateData, yaw: f32, magnitude: f32) {
    if let StateData::AirSlide(a) = state {
        (a.yaw, a.stick, a.magnitude) = (yaw, true, magnitude);
    }
}

impl PlayerState for AirSlide {
    const ID: StateId = StateId::AirSlide;

    fn sub_step(&self) -> u8 {
        self.sub
    }

    /// Begin 0x141332930.
    fn begin(&mut self, cx: &mut Ctx) {
        if self.dodge {
            // The flagged variants draw the weapon; a plain slide in battle does nothing, out of
            // battle it calls Sheathe (a no-op for the plain Keyblade).
            actions::draw(cx, true);
        } else if !cx.pawn.actions.battle {
            actions::sheathe(cx);
        }
        let s = &mut *cx.pawn;
        if !self.recovery {
            // pawn bit 1 = 0 (vt+0x1E68(0)): back on the next pawn tick with the timer at 0.
            s.actions.air_slide_ok = false;
            if self.stick {
                // Turn((0, +0x8C, 0), 1.0): a full snap to the stick yaw (0x141332F20).
                s.yaw = wrap_degrees(self.yaw);
            }
        } else {
            // Faces the attacker (or keeps the facing), does not consume the air-slide bit.
            let yaw = match self.target {
                Some(target) => {
                    let d = target - s.cmc.location;
                    if d.truncate().length_squared() > 1.0e-8 { d.y.atan2(d.x).to_degrees() } else { s.yaw }
                }
                None => s.yaw,
            };
            self.yaw = yaw;
            s.yaw = wrap_degrees(yaw);
            // HasAbility(BLOW_COUNTER 2): +0xAC = 2, +0xB0 = 0x27 (0x141332CDE).
            self.armed = actions::has(s, Ability::BlowCounter);
        }
        actions::set_reaction(s, reaction::GHOST_INVINCIBLE);
        self.invincible = true;
    }

    /// Tick 0x141340A10.
    fn tick(&mut self, cx: &mut Ctx, _dt: f32) {
        // The pole probe of the tick (II.4; Part I 6.3): a pole in reach -> `TryPoleTurn`.
        if self.sub != 0 && !self.recovery && super::pole_turn::probe_and_try(&StateData::AirSlide(*self), cx) {
            return;
        }
        let s = &mut *cx.pawn;
        if self.sub == 0 {
            s.cmc.stop_movement();
            s.cmc.gravity_off = true;
            if self.recovery {
                // Slot 113(1): the reaction command (BLOW_COUNTER) for 2.0 s or until End.
                if self.armed {
                    s.actions.reaction_command = Some((2, A.airrecovery_counter_window));
                }
                s.anim.play_with(AnimId::AirRecovery, PlayArgs::mode(2, 0.0));
            } else if self.dodge {
                s.anim.play_with(AnimId::AirDodge, PlayArgs::mode(2, -1.0));
            } else {
                // The AIRSLIDE level scale on the horizontal root motion: scale (s, 1.0).
                s.root_scale = (actions::air_slide_scale(s), 1.0);
                s.anim.play_with(if s.actions.weapon_out { AnimId::AirSlideB } else { AnimId::AirSlide }, PlayArgs::mode(2, -1.0));
            }
            self.sub = 1;
        } else {
            if self.abort {
                s.queue.do_chain();
                return;
            }
            if s.anim.ended {
                if !on_ground(s) {
                    let yaw = s.yaw;
                    jump::fall(s, yaw);
                } else {
                    s.queue.do_chain();
                }
                return;
            }
            if s.queue.flags.enable_change {
                if s.queue.queued_id().is_some() {
                    if on_ground(s) && s.actions.battle {
                        s.actions.last_pose = 1;
                    }
                    s.queue.do_chain();
                    return;
                }
                // The stick exit: comiss 0.5,[+0x90]; jae skip (0x141340BC1).
                if !self.recovery && !on_ground(s) && self.stick && self.magnitude > A.airslide_fall_exit_stick_min {
                    jump::fall_keeping_clip(s, self.yaw);
                    self.kept = true;
                    return;
                }
            }
        }
        // The pole probe (tier 2): nothing found: the stick flag is cleared.
        self.stick = false;
    }

    /// End 0x1413347F0.
    fn end(&mut self, cx: &mut Ctx) {
        let s = &mut *cx.pawn;
        if !self.recovery && !on_ground(s) {
            // pawn+0xE38 = MyAirSlideCoolDownTime 0.6 (0x1413348ED): only when it ends airborne.
            s.combat.air_slide_cooldown = A.airslide_cooldown;
        }
        // SetMovementMode(3, 0); gravity on. The velocity is not touched (still the zero of sub 0).
        s.cmc.set_falling();
        s.cmc.gravity_off = false;
        actions::restore_reaction(s);
        // (DON-KIT) Donald's Aero aura: the Air Slide / Air Dodge window x (1 + evasion) (not the Aerial Recovery)
        if self.invincible && !self.recovery && s.aura_evasion > 0.0 {
            s.aura_ghost = s.anim.time.max(0.0) * s.aura_evasion;
        }
        self.invincible = false;
        let next = s.queue.queued_id();
        if next != Some(StateId::Jump) {
            // pawn bit 0 (second jump) = 0 (0x141334A3F).
            s.actions.second_jump = false;
        }
        // The clip keeps its root scale while it plays on in a Jump that keeps it (flag 0x40):
        // the Jump state puts the scale back.
        if !self.kept {
            s.root_scale = (1.0, 1.0);
        }
        self.disarm(s);
    }

    /// `TresTiming` 0x141345B60 (any parameter): the reaction type is restored. None of the
    /// three clips carries the notify.
    fn on_notifies(&mut self, cx: &mut Ctx, notifies: &Notifies) {
        if self.invincible && notifies.fired(NotifyClass::TresAnimNotifyTiming) {
            actions::restore_reaction(cx.pawn);
            if !self.recovery && cx.pawn.aura_evasion > 0.0 {
                cx.pawn.aura_ghost = cx.pawn.anim.time.max(0.0) * cx.pawn.aura_evasion; // (DON-KIT)
            }
            self.invincible = false;
        }
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}
