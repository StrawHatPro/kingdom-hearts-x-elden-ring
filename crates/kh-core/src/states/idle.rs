//! `TresPlayerState_Idle` (docs/sora-movement-spec.md revision 2, section 3.1; change D15) with
//! the stance system of docs/sora-actions-spec.md II.3 ("Idle::Begin" 0x141386050, "Idle::Tick"
//! 0x1413944D0; the actions implementer). With the battle flag off, the weapon away and the
//! pose NORMAL the behaviour is the stage-1 one, pinned bit for bit by tests/golden.rs.
//!
//! Stance for the plain Keyblade (weapon kind 1): the tick compares `battleAnim` (the LAST POSE,
//! written by `TresLastAnimPose` notifies) with the battle flag. Flag on, pose not BATTLE: the
//! Keyblade is drawn and `BattleStart` plays. Pose BATTLE, flag off: `BattleEnd` plays when the
//! idle clip that is playing ends (the 3.0 s countdown is for formchange kinds only); its notify
//! at 0.1 s ends the battle pose, the Keyblade stays in hand (`IdleN`), and is put away by
//! `Draw(0)` at the end of the first idle break (3 x `IdleN` + `IdleBreakW`). `IdleB` /
//! `IdleLockon` need the weapon out AND the battle pose. Not here: water flow, floating, hop
//! points, rails, slope slide, the weapon-restricted zone, the formchange momentum.

use super::common::on_ground;
use super::{floating, hopping, jump, rail_slide, slope_slide, PlayerState, StateData, StateId};
use crate::actions::{self, A};
use crate::anim::{AnimId, PlayArgs};
use crate::pawn::{Ctx, Pawn};
use crate::sora::MovementState;

/// Idle +0x88 `battleAnim` at Begin, +0x91 lock-on, +0x92 break played, +0x93 land-pose clip
/// playing, +0x98 wrap counter, +0xA8 last clip time. (+0x9C is zeroed by Reset and never
/// written: the gates' tests of it are constant.)
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Idle {
    /// 0 the idle loop, 1 a one-shot (`BattleStart`, `BattleEnd`, a land-pose clip).
    sub: u8,
    /// How often the idle animation has ended since the last fidget (+0x98).
    ends: u32,
    /// Idle+0x91: the controller's manual lock (slot 720) as the state last saw it (combat
    /// strand D 7.13).
    locked: bool,
    battle_anim: bool,
    break_played: bool,
    land_clip: bool,
    last_time: f32,
}

impl Idle {
    /// The state object after `Reset`.
    pub const RESET: Idle = Idle { sub: 0, ends: 0, locked: false, battle_anim: false, break_played: false, land_clip: false, last_time: 0.0 };

    /// `Play(IdleName(weaponOut, battleAnim, lock), -1, mode 2)`: `IdleN` with its stage-1 call.
    fn play_idle(s: &mut Pawn, battle_anim: bool, locked: bool) {
        match actions::idle_name(s.actions.weapon_out, battle_anim, locked) {
            AnimId::IdleN => s.anim.play(AnimId::IdleN),
            name => s.anim.play_with(name, PlayArgs::mode(2, -1.0)),
        }
    }
}

impl PlayerState for Idle {
    const ID: StateId = StateId::Idle;

    fn sub_step(&self) -> u8 {
        self.sub
    }

    /// Idle begin 0x141386050: returns without playing if not on ground (change D15).
    fn begin(&mut self, cx: &mut Ctx) {
        // `PushState(5)` when floating (0x14138610B; actions spec II.2 "Floating").
        if floating::try_enter(&StateData::Idle(*self), cx.pawn) {
            cx.pawn.queue.do_chain();
            return;
        }
        if !on_ground(cx.pawn) {
            return;
        }
        // `TryHopping(actor yaw, 0, 0)` (0x1413861E1) -> DoChain: a hop point in reach (II.5 H2).
        let yaw = cx.pawn.yaw;
        if hopping::try_hopping(&StateData::Idle(*self), cx, yaw, false) {
            cx.pawn.queue.do_chain();
            return;
        }
        self.locked = cx.pawn.combat.lockon.locked;
        // SetSecondJump(1) (0x141386337).
        cx.pawn.actions.second_jump = true;
        let battle_anim = actions::battle_anim(cx.pawn);
        if battle_anim {
            // elif bA: Draw(1, 1, 1.0); +0x8C = 3.0 (0x141386579).
            actions::draw(cx, true);
        }
        let s = &mut *cx.pawn;
        self.battle_anim = battle_anim;
        // A stored land pose: its landing clip first (II.3 "Landing variants and the land pose").
        if let Some(name) = actions::land_name(actions::land_pose_of(s), s.actions.weapon_out) {
            self.land_clip = true;
            s.anim.play_with(name, PlayArgs::mode(2, -1.0));
            self.sub = 1;
            return;
        }
        // The idle name; not replayed when it is the current clip or an idle break is playing.
        let name = actions::idle_name(s.actions.weapon_out, battle_anim, self.locked);
        let current = s.anim.name;
        if name != AnimId::IdleN {
            if name != current {
                s.anim.play_with(name, PlayArgs::mode(2, -1.0));
            }
        } else {
            // (Stage 1 replays `IdleN` at every Begin: kept, the golden runs pin it.)
            s.anim.play(AnimId::IdleN);
        }
        self.sub = 0;
        self.last_time = 0.0;
    }

    /// `TresPlayerState_Idle::Tick` 0x1413944D0 (3.1; actions spec II.3).
    fn tick(&mut self, cx: &mut Ctx, _dt: f32) {
        // (0x141394598.)
        if floating::try_enter(&StateData::Idle(*self), cx.pawn) {
            cx.pawn.queue.do_chain();
            return;
        }
        if !on_ground(cx.pawn) {
            let yaw = cx.pawn.yaw;
            jump::fall(cx.pawn, yaw);
            return;
        }
        // The rail probe (0x141394825: range 50 along the facing), then `TrySlopeSlide(0)`
        // (0x141394861): no effect in a world without rails and slide floors.
        {
            let (me, yaw) = (StateData::Idle(*self), cx.pawn.yaw);
            if rail_slide::probe_and_try(&me, cx, A.rail_check_range, yaw) || slope_slide::try_slope_slide(&me, cx, false) {
                cx.pawn.queue.do_chain();
                return;
            }
        }
        let battle_anim = actions::battle_anim(cx.pawn);
        let battle = cx.pawn.actions.battle;
        if self.sub == 0 {
            // vt+0x1E68(1); vt+0x1E78(1); vt+0x1E88(1); vt+0x1E98(1): bits 1 - 4, not bit 0.
            let a = &mut cx.pawn.actions;
            (a.air_slide_ok, a.air_guard_ok, a.flow_jump, a.superslide_ok) = (true, true, true, true);
        }
        if battle_anim != battle {
            if battle {
                // Battle began, the pose is not BATTLE (0x141394E59).
                actions::draw(cx, true);
                cx.pawn.anim.play_with(AnimId::BattleStart, PlayArgs::mode(2, -1.0));
                self.sub = 1;
            } else if cx.pawn.anim.ended {
                // Pose BATTLE, battle over: the plain Keyblade waits for the clip's end
                // (0x141394EF2), then Sheathe (a no-op) and "BattleEnd".
                actions::sheathe(cx);
                cx.pawn.anim.play_with(AnimId::BattleEnd, PlayArgs::mode(2, -1.0));
                self.sub = 1;
            }
        }
        let s = &mut *cx.pawn;
        if self.sub == 1 && s.anim.ended {
            Idle::play_idle(s, battle_anim, self.locked);
            (self.break_played, self.land_clip, self.ends, self.sub, self.last_time) = (false, false, 0, 0, 0.0);
        }
        if self.land_clip {
            return;
        }
        if battle_anim {
            // The battle pose: `IdleB` <-> `IdleLockon` with the lock flag; replayed at its end.
            let locked = s.combat.lockon.locked;
            if locked != self.locked {
                self.locked = locked;
                Idle::play_idle(s, true, locked);
                (self.break_played, self.ends) = (false, 0);
            } else if s.anim.ended {
                Idle::play_idle(s, true, locked);
                (self.break_played, self.ends) = (false, 0);
            }
        } else if self.sub == 0 {
            self.locked = s.combat.lockon.locked;
            // The clip wrapped (a looping instance) or ended.
            let wrapped = s.anim.looping() && s.anim.time < self.last_time;
            self.last_time = s.anim.time;
            if !s.anim.ended && !wrapped {
                return;
            }
            self.ends += 1;
            if self.ends >= A.stance_idle_break_count as u32 {
                // The third end: the idle break, by weapon out (cmp eax,3; jl @0x14139527A).
                if s.actions.weapon_out {
                    s.anim.play_with(AnimId::IdleBreakW, PlayArgs::mode(2, -1.0));
                } else {
                    s.anim.play(AnimId::IdleBreakN);
                }
                self.break_played = true;
                self.ends = 0;
            } else {
                if self.break_played {
                    // The clip that just ended was the break: the Keyblade is put away here
                    // (0x14139536A).
                    // (PORT ADDITION IMPL_ER44: not while the host holds it out)
                    if cx.pawn.actions.weapon_out && !cx.pawn.actions.hold_weapon {
                        actions::draw(cx, false);
                    }
                    self.break_played = false;
                    self.ends = 0;
                }
                Idle::play_idle(cx.pawn, false, self.locked);
            }
        }
    }

    fn end(&mut self, _s: &mut Ctx) {}

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Idle
    }
}
