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
    /// (F-STAFF) flag 0x10: the back slide of a weapon form (TryAirSlide without the stick).
    back: bool,
    /// (F-ARROWGUN) flag 4: the Sliding Shot of the arrowgun forms (weapon forms 0x02 / 0x10 in battle); +0xA0: the shots so far
    /// (the switch-list index of the next one).
    slide_shot: bool,
    shots: i32,
    /// (AUDIT-C) flag 0x100 (Begin 0x141332930, form 0x8B): Rage Form's back slide away from a guarded attacker (flag 0x10
    /// with a target pawn, OnGuardedHit 0x14143CAF0): it faces him and arms the reaction command 0xD (Rage Raid, kind 0xC6).
    rage_raid: bool,
}

impl AirSlide {
    /// The state object after `Reset` (0x14133CF60).
    pub const RESET: AirSlide = AirSlide { sub: 0, stick: false, yaw: 0.0, magnitude: 0.0, target: None, invincible: false, dodge: false, recovery: false, armed: false, abort: false, kept: false, back: false, slide_shot: false, shots: 0, rage_raid: false };

    /// Flag 0x80 (slot 114): the next tick chains.
    pub(super) fn aborted(&self) -> bool {
        self.abort
    }

    /// An Aerial Recovery (flag 0x20) facing the attacker at `target` (`JumpRequest` 0x14143E123).
    pub(super) fn recovery(target: Option<Vec3>) -> AirSlide {
        AirSlide { recovery: true, target, ..AirSlide::RESET }
    }

    /// (AUDIT-C) Rage Form's guard (OnGuardedHit 0x14143CAF0 case 0x8B): the back slide (flag 0x10) away from the attacker
    /// (+0x98 = it, +0x8C = his root yaw - 180).
    pub(crate) fn rage_back(yaw: f32, attacker: Option<Vec3>) -> AirSlide {
        AirSlide { back: true, yaw: wrap_degrees(yaw - 180.0), target: attacker, ..AirSlide::RESET }
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
    if !matches!(s.queue.queued_id(), None | Some(StateId::Attack) | Some(StateId::ComboWa) /* (F-STAFF) */) {
        return false;
    }
    let mut ground = on_ground(s);
    let pass = match state {
        StateData::Idle(_) | StateData::Run(_) => true,
        // (F-ARROWGUN) 40 SHOOTERMODE passes the gate (slot 68); TryAirSlide takes it with bKeep = 0.
        StateData::ShooterMode(_) => true,
        StateData::Jump(j) => !j.flow,
        // GLIDE, FLOATING (case 4 / 5, 0x141425926): bGround = false; pass.
        StateData::Glide(_) | StateData::Floating(_) => {
            ground = false;
            true
        }
        // Attack states: bit 1 && Attack+0x234 bit 7 (0x141425D3E).
        StateData::Attack(a) => s.actions.air_slide_ok && a.cancel,
        // (F-HAMMER) COMBO_HM / COMBO_DR: the Attack class's row.
        StateData::ComboHm(c) => s.actions.air_slide_ok && c.0.a.cancel,
        StateData::ComboDr(c) => s.actions.air_slide_ok && c.0.a.cancel,
        // (F-STAFF) COMBO_WA (83) is one of the attack ids of the row.
        StateData::ComboWa(c) => s.actions.air_slide_ok && c.a.cancel,
        // (AUDIT-C) COMBO_ANTI (86) is one of the attack ids of the row too.
        StateData::ComboAnti(c) => s.actions.air_slide_ok && c.a.cancel,
        StateData::Guard(g) => s.actions.air_slide_ok && g.hit_entry && g.enable_input,
        StateData::DodgeRoll(_) => s.queue.flags.enable_change,
        StateData::Damage(d) => matches!(d.kind.value(), 1 | 2 | 9) && s.queue.flags.enable_change,
        // (magic implementer) FIREMAGIC 18: bit 1 && m_bEnableChange && !(+0xD8 & 0x50) && the
        // combo is not over (II.2 gate table).
        StateData::FireMagic(m) => s.actions.air_slide_ok && m.can_dodge(s),
        // AIR_SLIDE: only out of a recovery, at m_bEnableChange (0x141425A38).
        StateData::AirSlide(a) => a.recovery && s.queue.flags.enable_change,
        _ => false,
    };
    if !pass {
        return false;
    }
    // (F-STAFF) The weapon form's rows (equipment vt+0x440, 0x141425F00 ..): Mirage Staff (form 10) passes without the stick;
    // in battle always (no ability, no ground test), else with AIRSLIDE.
    if crate::combat::forms::staff::is_staff(s.combat.form.cmd) {
        return magnitude < A.airslide_stick_min || s.actions.battle || actions::has(s, Ability::Airslide);
    }
    // (F-ARROWGUN) The gun forms' rows (0x141425780 / 0x141425F00; kind = pawn vt+0x12E0 = the weapon form 0x02 / 0x05 /
    // 0x10 / 0x11, sub-kind vt+0x12C8 = the Keyblade's weapon type): without the stick only Shooting Star's (type 3) pass
    // (the Back Slide); in battle the arrowguns always (no ability, no ground test), the launchers with AIRSLIDE or
    // AIRDODGE; out of battle the plain rule below.
    if let Some(family) = crate::combat::forms::arrowgun::family(s.combat.form.cmd) {
        if magnitude < A.airslide_stick_min {
            return !crate::combat::forms::arrowgun::is_hunny(s.combat.form.cmd);
        }
        if s.actions.battle {
            return family == crate::combat::forms::arrowgun::Family::Dw || actions::has(s, Ability::Airslide) || actions::has(s, Ability::Airdodge);
        }
    }
    // Weapon kind 1 with the stick below 0.1 FAILS (0x141425F00).
    if magnitude < A.airslide_stick_min {
        return false;
    }
    // (AUDIT-C) 0x141425F76: with the stick, Rage Form (ctrl vt+0x1A70 == 0x8B) passes: no ability, no ground test.
    if s.combat.form.cmd == crate::combat::forms::rage::CHANGE_AN {
        return true;
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
    let mut dodge = s.actions.battle && actions::has(s, Ability::Airdodge);
    // (F-STAFF) Weapon form 10 (Mirage Staff): without the stick the back slide (flag 0x10, +0x8C = yaw - 180); in battle
    // the dodge (flag 2, no ability needed) and a mirror image (controller slot 868(0) = ctrl vt+0x1B20(0)).
    let staff = crate::combat::forms::staff::is_staff(s.combat.form.cmd);
    let back = staff && magnitude < A.airslide_stick_min;
    let image = staff && s.actions.battle;
    if staff {
        dodge = s.actions.battle;
    }
    // (AUDIT-C) 0x141436E1A: in Rage Form (ctrl vt+0x948 -> vt+0x1A70 == 0x8B) the weapon-form switch is skipped: always the
    // Aerial Dodge (flag 2), no back slide, no Sliding Shot.
    let rage = s.combat.form.cmd == crate::combat::forms::rage::CHANGE_AN;
    if rage {
        dodge = true;
    }
    // (F-ARROWGUN) TryAirSlide 0x141436D20 in the gun forms: without the stick the Back Slide (forms 0x02 / 0x05 of Shooting
    // Star); in battle the arrowguns (0x02 / 0x10) shoot (flag 4: Sliding Shot), the launchers (0x05 / 0x11) dodge in the
    // air (dodge = !0x1405AD850, the movement's "on the ground").
    let gun = crate::combat::forms::arrowgun::family(s.combat.form.cmd);
    let back = back || (gun.is_some() && magnitude < A.airslide_stick_min);
    let slide_shot = gun == Some(crate::combat::forms::arrowgun::Family::Dw) && s.actions.battle;
    if gun == Some(crate::combat::forms::arrowgun::Family::Bz) && s.actions.battle {
        dodge = !on_ground(s);
    }
    let stick = magnitude >= A.airslide_stick_min;
    let yaw = if stick { yaw } else if back { wrap_degrees(s.yaw - 180.0) } else { yaw };
    s.queue.clear();
    // mag >= 0.1: +0x8C = yaw; +0x88 = 1 (the request never writes +0x90).
    s.queue.push_state(AirSlide { dodge, stick, yaw, back, slide_shot, ..AirSlide::RESET });
    if image {
        crate::combat::forms::staff::avatar_command(cx, 0);
    }
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
        if self.dodge || self.back || self.slide_shot {
            // The flagged variants draw the weapon; a plain slide in battle does nothing, out of
            // battle it calls Sheathe (a no-op for the plain Keyblade).
            actions::draw(cx, true);
        } else if !cx.pawn.actions.battle {
            actions::sheathe(cx);
        }
        // (F-ARROWGUN) the Sliding Shot's facing target (Begin 0x141332930: the switch list's entry 0 while valid).
        let face = if self.slide_shot && !self.back { cx.pawn.combat.arrowgun.aim.switch_list.first().and_then(|t| cx.target(*t)).filter(|t| t.flags.has(crate::host::TargetFlags::ALIVE)).map(|t| t.position) } else { None };
        // (AUDIT-C) Begin 0x141332930 in Rage Form (ctrl vt+0x1A70 == 0x8B): flag 0x100 with a target pawn and flag 0x10 (the
        // guard's back slide); the aura (group 100) and the eyes (13) end for the slide (pawn+0x440 vt+0x1A8(100 / 0xD, 1) =
        // 0x1405A85F0 -> FUN_141765520: ends the entries OF that group; End 0x1413347F0 starts them again).
        if cx.pawn.combat.form.cmd == crate::combat::forms::rage::CHANGE_AN {
            self.rage_raid = self.back && self.target.is_some();
            crate::combat::forms::rage::slide_hide(cx);
        }
        let s = &mut *cx.pawn;
        if self.rage_raid {
            // (+0xA8 & 0x120) != 0: faces the attacker (as the recovery); !vt+0x1EE8: +0xAC = 0xD, +0xB0 = 0xC6 and the
            // air-slide bit off (vt+0x1E68(0)) (0x141332D0B).
            if let Some(target) = self.target {
                let d = target - s.cmc.location;
                if d.truncate().length_squared() > 1.0e-8 {
                    self.yaw = d.y.atan2(d.x).to_degrees();
                    s.yaw = wrap_degrees(self.yaw);
                }
            }
            s.actions.air_slide_ok = false;
            self.armed = true;
        } else if !self.recovery {
            // pawn bit 1 = 0 (vt+0x1E68(0)): back on the next pawn tick with the timer at 0.
            s.actions.air_slide_ok = false;
            // (F-ARROWGUN) (flags & 0x14) == 4, the Sliding Shot: a full turn to the switch list's first target (ctrl+0x9E8 [0],
            // the yaw of (it - his centre).xy); no stick snap then.
            if let Some(target) = face {
                let d = target - s.cmc.location;
                if d.truncate().length_squared() > 1.0e-8 {
                    s.yaw = wrap_degrees(d.y.atan2(d.x).to_degrees());
                }
            } else if self.stick && !(self.slide_shot && !self.back) {
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
            // (AUDIT-C) slot 113(1) 0x14134C5E0: FUN_141414350(ctrl, +0xAC = 0xD, 2.0 s): Rage Raid's reaction command.
            if self.rage_raid && self.armed {
                s.actions.reaction_command = Some((crate::combat::forms::rage::RAGE_RAID_COMMAND, 2.0));
            }
            if self.recovery {
                // Slot 113(1): the reaction command (BLOW_COUNTER) for 2.0 s or until End.
                if self.armed {
                    s.actions.reaction_command = Some((2, A.airrecovery_counter_window));
                }
                s.anim.play_with(AnimId::AirRecovery, PlayArgs::mode(2, 0.0));
            } else if self.slide_shot && !self.back {
                // (F-ARROWGUN) flag 4 (Tick 0x141340A10): the clip by the stick relative to his facing (d = +0x8C - yaw):
                // |d| >= 135 SlideShotB, d >= 45 SlideShotR, d > -45 SlideShotF, else SlideShotL; the arrowguns' set.
                let d = wrap_degrees(self.yaw - s.yaw);
                let name = if d.abs() >= 135.0 { "SlideShotB" } else if d >= 45.0 { "SlideShotR" } else if d > -45.0 { "SlideShotF" } else { "SlideShotL" };
                match AnimId::from_name(crate::combat::forms::arrowgun::clip(s.combat.form.cmd, name)) {
                    Some(clip) => s.anim.play_with(clip, PlayArgs::mode(2, -1.0)),
                    None => s.note("a Sliding Shot clip is not in the generated animation table"),
                }
            } else if self.back || self.dodge {
                // (F-STAFF) flag 0x10 plays BackSlide; both through the form's replacement set (Mirage Staff: A058 / A054).
                let name = if self.back { "BackSlide" } else { "AirDodge" };
                // (F-ARROWGUN) the gun forms by their own command (AnimSet2 2062 / AnimSet5 5058 share the logical name).
                let cmd = s.combat.form.cmd;
                let logical = if crate::combat::forms::arrowgun::family(cmd).is_some() { crate::combat::forms::arrowgun::clip(cmd, name) } else { crate::combat::forms::clip(cmd, name) };
                let clip = AnimId::from_name(logical).unwrap_or(AnimId::AirDodge);
                s.anim.play_with(clip, PlayArgs::mode(2, -1.0));
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
        // (AUDIT-C) End 0x1413347F0 in Rage Form: the aura (group 100) again, and the eyes (13) with a Riskcharge used.
        if s.combat.form.cmd == crate::combat::forms::rage::CHANGE_AN {
            crate::combat::forms::rage::slide_restore(cx);
        }
        let s = &mut *cx.pawn;
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
        // (F-ARROWGUN) the anim interface's TresShot handler (slot 69 0x141343E10): with flag 4 the arrowguns' DW_NORMAL pair.
        if self.slide_shot {
            for event in &notifies.events {
                if (event.class(), event.phase) == (NotifyClass::TresAnimNotifyShot, crate::anim::NotifyPhase::Instant) {
                    let location = match event.def().data {
                        crate::anim::NotifyData::Shot { location_index, .. } => location_index.unwrap_or(0),
                        _ => 0,
                    };
                    self.shots = crate::combat::forms::arrowgun::slide_shot(cx, location, self.shots);
                }
            }
        }
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
