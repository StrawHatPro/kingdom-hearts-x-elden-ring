//! (F-ARROWGUN) The controller's aim state the gun forms read and write: the switch list, the shooter aim, the multi-lock.
//! KH3 keeps these on the controller (ATresPlayerControllerBase +0x93C .. +0x9F0); here on `PawnCombat.arrowgun.aim`.
//! ShooterMode (state 40, states/shooter_mode.rs), the target search's list part (0x141404E50, combat/targeting.rs) and the
//! multi-lock (slot 790 0x141419C00) fill it; the gun shot handlers (states/combo_dw.rs) and Sliding Shot
//! (states/air_slide.rs) read it.
//!
//! WHAT KH3 DOES (decompiled C in re/decomp; build/sora_tmp/combat/P2_D_section.md 7.5 for the target search's order):
//! - THE RT HOLD (slot 713 0x1413F0D00 / template 0x1413F29A0): while RT (FKey Gamepad_RightTrigger 0x149C707C0) is down
//!   its time +0x940 grows; past 0.26666668 s with the latch +0x93D clear: the hold flag +0x93C = 1 and the latch = 1;
//!   released: hold = latch = 0 (the tap +0x93E is targeting.rs's `rt_tap`). Slot 727 0x141404210 reads +0x93C.
//! - THE REQUEST (UpdateLockon 0x141404E50 step 6, at 0x1414066C0): the hold flag and the request interface slot 191
//!   0x141447640 (gate 0x14142D9D0, then SetState(40)) -> +0x968 = 1, +0x98C = 0; not locked: the current marker goes, the
//!   camera's lock ends, the multi-lock ends (vt+0x18B8 = slot 791), +0x944 / +0x94C / +0x994 / +0x99C cleared; locked:
//!   Unlock ([`controller_post`]; the gate: states/shooter_mode.rs `gate`).
//! - THE AIM PICK (step 5, slot 715 0x141406F80, every frame while +0x968 is set): [`shooter_pick`]; its tail: ShooterMode
//!   running -> the end request (+0xB1) when the RT hold flag is off; ShooterMode gone -> slot 840 0x1413FB4A0 (leave the
//!   aim: [`shooter_end`]) and in 0x80 / 0x8E the multi-lock (slot 790).
//! - THE MULTI-LOCK (slot 790 0x141419C00: +0x9C4 = 1, +0x968 = 0, +0x9CC = 6, both lists emptied with their markers, the
//!   targets cleared, ATresCameraMultiLock spawned (INT)): started by StartStyle 0x14141A9D0 in 0x80 / 0x8E, by UpdateLockon
//!   when a lock is lost in those forms (7.7, 0x141406EB4) and by slot 715 when ShooterMode ends; ended by slot 791
//!   0x1413FA990 (StartStyle's and EndStyle's common part, the RB lock out of it). While on (step 9, 0x141406894): the
//!   list sorted nearest first, cut to 6, a type-0 marker on each entry (old entries' markers removed); RB tap: slot
//!   791(1), lock list[0]. The DW shot handler 0x1413582D0 then spreads its bullets over the list ([`Aim::gun_target`]).

use crate::combat::forms::arrowgun::{CHANGE_DW, CHANGE_DW2};
use crate::combat::targeting::{CursorKind, TAP_TIME};
use crate::host::{EventKind, TargetFlags, TargetId, TargetInfo};
use crate::input::PadButton;
use crate::pawn::Ctx;
use crate::states::StateData;
use glam::{Vec2, Vec3};

/// slot 715 0x141406F80: the pick's range (2.5e7 = 5000 squared, from pawn slot 1058), the crowd radius (160000 = 400
/// squared) and the screen radius (14400 = 120 px squared, around the view rectangle's centre).
pub const PICK_RANGE: f32 = 5000.0;
pub const PICK_CROWD: f32 = 400.0;
pub const PICK_RADIUS_PX: f32 = 120.0;
/// Pawn slot 1058 0x141448550: the search origin, root + (0, 0, 100) (targeting.rs `ORIGIN_UP`).
const ORIGIN_UP: f32 = crate::combat::targeting::ORIGIN_UP;

/// What a host's camera needs while ShooterMode runs (ATresCameraShooterMode, ShooterMode slot 114 0x1413BED50 and the Tick's
/// look-at write camera+0x9E0): written by the state every tick, `camera` false otherwise.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct ShooterView {
    /// The shooter camera is on (slot 114(1) at sub-step 1's end; off at ShooterModeEnd / End).
    pub camera: bool,
    /// The camera's look-at point (camera+0x9E0 = eye + Rotator(pitch, yaw).Vector() * distance).
    pub look_at: Vec3,
    /// The eye the aim is measured from (actor centre + Rot(pitch, yaw) * (0, 0, 150)).
    pub eye: Vec3,
    /// The aim (state +0xCC yaw, +0xD0 pitch, degrees; the actor's rotation in KH3: the port's actor has no pitch).
    pub yaw: f32,
    pub pitch: f32,
    /// +0xFC: the look distance (5000 without a target).
    pub distance: f32,
    /// The launchers' charge (+0xC0, 0 .. 100), the weapon glow level (+0xBC: -1 off, 0 charging, 1 full: pawn vt+0x1468 ->
    /// equipment+0x200 vt+0x1E0) and the HUD's charge-full flag (+0xC8, 0x14140BE90).
    pub charge: f32,
    pub charge_level: i8,
    pub charged: bool,
    /// +0xB8: the launcher's charge mode (weapon kinds 5 / 0x11) -> the HUD scope type (0x14140D670).
    pub launcher: bool,
    /// m_pTargetComponent (state+0x90): the aim's target (its HUD gauge: slot 115 0x14158A860).
    pub target: TargetId,
}

/// The camera values ShooterMode gives its camera (slot 114 0x1413BED50: 0x1405D6F40(cam, +0xEC 60.0, 0),
/// 0x140626FC0(cam, +0xF0 (30, -65, 45), 0); off: the offset back to zero, blend (0, 3, 2.0, 2) through camera vt+0x798).
pub const CAMERA_VALUE: f32 = 60.0;
pub const CAMERA_OFFSET: Vec3 = Vec3::new(30.0, -65.0, 45.0);
pub const CAMERA_BLEND_OUT: f32 = 2.0;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Aim {
    /// ctrl+0x9E8 / +0x9F0: the switch list (targets within 2400 cm, +0x9C8), rebuilt every frame by the target search
    /// 0x141404E50 while locked on, in multi-lock or in shooter aim, in the search's order (sorted and cut to 6, +0x9CC, by
    /// the RT switch and the multi-lock step); empty otherwise.
    pub switch_list: Vec<TargetId>,
    /// ctrl+0x968: the shooter aim is on (ShooterMode's request; slot 729).
    pub shooter: bool,
    /// ctrl+0x994 while the shooter aim is on / +0x98C: the screen-centre pick and "a pick was found this frame" (slot 730).
    pub pick: TargetId,
    pub on_screen: bool,
    /// ctrl+0x9C4: the multi-lock mode is on (slot 790 0x141419C00).
    pub multilock: bool,
    /// ctrl+0x960 / +0x994 as the latest controller update left them (the DW shot handler reads them: [`Aim::gun_target`]).
    pub locked: bool,
    pub current: TargetId,
    /// ctrl+0x93C / +0x93D / +0x940: the RT hold flag, its latch, the hold time.
    pub rt_hold: bool,
    rt_latch: bool,
    rt_time: f32,
    /// PORT ADDITION: the right stick (raw axes, x right, y up), which the core's `Input` does not carry: the host passes it
    /// with `Sora::set_shooter_look` every step (ShooterMode's manual aim reads it through the shooter camera's input, camera
    /// slot 259 0x1405CD290 -> slot 258 0x1405CD3F0 mode 2: the right stick, the left stick when the right one is idle).
    pub look: Vec2,
    /// ShooterMode's +0xE0 "look at" flag (slot 113 0x1413B2DC0): the pick keeps its target while it is set.
    pub look_at: bool,
    /// The multi-lock's type-0 markers on the HUD (the hosts get `LockCursor` events).
    markers: Vec<TargetId>,
    /// What the shooter camera needs ([`ShooterView`]).
    pub view: ShooterView,
    /// (AUDIT-A ARROWGUN) ShooterMode's +0xA8: the stick yaw of its latest MoveInput (0x141440600 case 40). The state object
    /// is the pawn's own and nothing resets the field (Reset slot 95 = UTresStateBase's, Begin 0x1413A6900 does not write
    /// it), so it lives here, outside the state: EndStyle's Fall reads it.
    pub stick_yaw: f32,
    /// (AUDIT-A ARROWGUN) EndStyle 0x14141CA20's last block found SHOOTERMODE running: iface slot 172 Fall(+0xA8)
    /// and the state's DoChain. The core's EndStyle has no state in its hands: ShooterMode's next Tick carries it out first.
    pub end_style_fall: bool,
}

impl Aim {
    /// The homing target of a gun shot, the DW shot handler 0x1413582D0's choice (0x1413586F0): with the controller's
    /// vt+0x1680 (slot 720 0x1414030E0 = ctrl+0x960, LOCKED ON) vt+0x1690 (+0x994, the locked target); else with a switch
    /// list the entry `shots % count` (state+0x130; none when that entry is gone); else `fallback`, the state's homing target
    /// (+0x118 / param_1+0xD8).
    pub fn gun_target(&self, shots: i32, fallback: TargetId) -> TargetId {
        if self.locked {
            return self.current;
        }
        if self.switch_list.is_empty() {
            return fallback;
        }
        self.switch_list[shots.rem_euclid(self.switch_list.len() as i32) as usize]
    }
}

fn targetable(t: &TargetInfo) -> bool {
    t.flags.has(TargetFlags::ALIVE | TargetFlags::LOCKABLE)
}

/// One frame of slot 713's RT part (P2_D 7.2): the hold flag past the tap time.
fn rt_tick(aim: &mut Aim, held: bool, dt: f32) {
    if held {
        aim.rt_time += dt;
        if aim.rt_time > TAP_TIME && !aim.rt_latch {
            aim.rt_hold = true;
            aim.rt_latch = true;
        }
    } else {
        aim.rt_time = 0.0;
        aim.rt_hold = false;
        aim.rt_latch = false;
    }
}

/// UpdateLockon step 5, the pick of slot 715 0x141406F80 (called by targeting.rs while the shooter aim is on): keep the
/// current pick while ShooterMode looks at it (+0xE0) and it is targetable within 5000 cm; else the registry's targetable
/// candidates (every enumerated target is of another team) within 5000 cm of pawn slot 1058 (root + 100 up), projected with
/// the last drawn frame's view (0x140ECF670), in front of the camera, nearest to the view rectangle's centre within 120 px.
/// Returns the pick (+0x994), whether one was found (+0x98C) and the crowd count (+0x95C, within 400 cm).
pub(crate) fn shooter_pick(cx: &Ctx, targets: &[TargetInfo]) -> (TargetId, bool, i32) {
    let origin = cx.pawn.cmc.location + Vec3::Z * ORIGIN_UP;
    let aim = &cx.pawn.combat.arrowgun.aim;
    let mut crowd = 0;
    if aim.look_at {
        if let Some(t) = targets.iter().find(|t| t.id == aim.pick && targetable(t)) {
            if (t.center - origin).length_squared() < PICK_RANGE * PICK_RANGE {
                return (t.id, false, crowd);
            }
        }
    }
    let Some(view) = cx.pawn.input.view else {
        // No drawn frame: nothing can be projected (as the shotlock's reticle test).
        return (TargetId::NONE, false, crowd);
    };
    let [x0, y0, x1, y1] = view.rect;
    // (x0 + (x1 - x0) / 2, y0 + (y1 - y0) / 2) in integers, as the code computes the centre.
    let (cxp, cyp) = ((x0 + (x1 - x0) / 2) as f32, (y0 + (y1 - y0) / 2) as f32);
    let (mut best, mut best_d) = (TargetId::NONE, PICK_RADIUS_PX * PICK_RADIUS_PX);
    for t in targets.iter().filter(|t| targetable(t)) {
        let d2 = (t.center - origin).length_squared();
        if d2 >= PICK_RANGE * PICK_RANGE {
            continue;
        }
        if d2 < PICK_CROWD * PICK_CROWD {
            crowd += 1;
        }
        let s = view.project(t.center);
        if s.z <= 0.0 {
            continue;
        }
        let d = (s.x - cxp).powi(2) + (s.y - cyp).powi(2);
        if d < best_d {
            (best, best_d) = (t.id, d);
        }
    }
    (best, best != TargetId::NONE, crowd)
}

/// Slot 840 0x1413FB4A0 (leaving the shooter aim): +0x968 = +0x98C = 0, the hold flag +0x93C = 0 (the latch stays: no new
/// request until RT is released), the lock and the targets cleared (targeting.rs `unlock`).
pub(crate) fn shooter_end(cx: &mut Ctx) {
    let aim = &mut cx.pawn.combat.arrowgun.aim;
    if !aim.shooter && !aim.on_screen {
        return;
    }
    (aim.shooter, aim.on_screen, aim.rt_hold, aim.pick, aim.look_at) = (false, false, false, TargetId::NONE, false);
    crate::combat::targeting::unlock(cx);
}

/// Slot 790 0x141419C00: the multi-lock mode starts (the lock and both lists cleared; the multi-lock camera is the host's).
pub(crate) fn multilock_start(cx: &mut Ctx) {
    crate::combat::targeting::unlock(cx);
    clear_markers(cx);
    let aim = &mut cx.pawn.combat.arrowgun.aim;
    aim.shooter = false;
    aim.multilock = true;
    aim.switch_list.clear();
}

/// Slot 791 0x1413FA990 `(1)`: only in multi-lock mode: the markers and lists cleared, +0x9C4 = 0 (the camera's end: INT).
pub(crate) fn multilock_end(cx: &mut Ctx) {
    if !cx.pawn.combat.arrowgun.aim.multilock {
        return;
    }
    clear_markers(cx);
    let aim = &mut cx.pawn.combat.arrowgun.aim;
    aim.multilock = false;
    aim.switch_list.clear();
}

fn clear_markers(cx: &mut Ctx) {
    for t in std::mem::take(&mut cx.pawn.combat.arrowgun.aim.markers) {
        cx.pawn.event(EventKind::LockCursor { target: t, kind: None });
    }
}

/// UpdateLockon step 9 (0x141406894 .. 0x141406B4E): a type-0 marker on every entry of the (sorted, cut) list; the markers
/// of entries no longer listed are removed (the copy of last frame's list, 0x14042D9A0 / 0x14157A7A0).
pub(crate) fn show_markers(cx: &mut Ctx) {
    let list = cx.pawn.combat.arrowgun.aim.switch_list.clone();
    let old = std::mem::take(&mut cx.pawn.combat.arrowgun.aim.markers);
    for t in old.iter().filter(|t| !list.contains(t)) {
        cx.pawn.event(EventKind::LockCursor { target: *t, kind: None });
    }
    for t in list.iter().filter(|t| !old.contains(t)) {
        cx.pawn.event(EventKind::LockCursor { target: *t, kind: Some(CursorKind::Auto) });
    }
    cx.pawn.combat.arrowgun.aim.markers = list;
}

/// A lock was lost while locked on (7.7, 0x141406EB4): in 0x80 / 0x8E (the other gates, vt+0x1B78 / 0x1BD0 / 0x1BF0 /
/// 0x1C30, are the shotlock / flow / ride modes: false in the forms' field play) the multi-lock starts again.
pub(crate) fn on_unlock(cx: &mut Ctx) {
    if matches!(cx.pawn.combat.form.cmd, CHANGE_DW | CHANGE_DW2) {
        multilock_start(cx);
    }
}

/// The controller's part after UpdateLockon (controller.rs, right after `targeting::update`): the RT hold flag (slot 713),
/// then step 6's request or step 5's tail (slot 715's end: 0x141407898 .. 0x14140791B). The mirrors for the shot handlers.
pub(crate) fn controller_post(state: &mut StateData, cx: &mut Ctx, dt: f32) {
    let held = cx.pawn.input.pad.held.has(PadButton::RightTrigger);
    rt_tick(&mut cx.pawn.combat.arrowgun.aim, held, dt);
    if cx.pawn.combat.arrowgun.aim.shooter {
        if let StateData::ShooterMode(m) = state {
            // ShooterMode running: !RT hold (slot 727) -> the end request +0xB1 (the input lock +0x878 bit 6 is the UI's).
            if !cx.pawn.combat.arrowgun.aim.rt_hold {
                m.end_request = true;
            }
        } else {
            // It has gone: slot 840, then in 0x80 / 0x8E the multi-lock (slot 790).
            shooter_end(cx);
            if matches!(cx.pawn.combat.form.cmd, CHANGE_DW | CHANGE_DW2) {
                multilock_start(cx);
            }
        }
    } else if cx.pawn.combat.arrowgun.aim.rt_hold && crate::states::shooter_mode::request(state, cx) {
        // Step 6 (0x1414066C0): +0x968 = 1, +0x98C = 0; the lock / the soft target and the multi-lock go.
        multilock_end(cx);
        crate::combat::targeting::unlock(cx);
        let aim = &mut cx.pawn.combat.arrowgun.aim;
        (aim.shooter, aim.on_screen, aim.pick, aim.look_at) = (true, false, TargetId::NONE, false);
    }
    let lock = cx.pawn.combat.lockon;
    let aim = &mut cx.pawn.combat.arrowgun.aim;
    (aim.locked, aim.current) = (lock.locked, lock.current);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rt_hold_flag_needs_a_long_press_and_a_release_to_come_back() {
        let mut a = Aim::default();
        for _ in 0..16 {
            rt_tick(&mut a, true, 1.0 / 60.0);
        }
        assert!(!a.rt_hold, "16 frames = 0.2667 s is still a tap");
        rt_tick(&mut a, true, 1.0 / 60.0);
        assert!(a.rt_hold);
        // Slot 840 clears the flag, the latch stays: no new hold while RT stays down.
        a.rt_hold = false;
        rt_tick(&mut a, true, 1.0 / 60.0);
        assert!(!a.rt_hold);
        rt_tick(&mut a, false, 1.0 / 60.0);
        assert!(!a.rt_latch);
    }

    #[test]
    fn gun_shots_follow_the_lock_then_the_list() {
        let mut a = Aim { switch_list: vec![TargetId(1), TargetId(2)], ..Aim::default() };
        assert_eq!((a.gun_target(0, TargetId(9)), a.gun_target(3, TargetId(9))), (TargetId(1), TargetId(2)));
        a.locked = true;
        a.current = TargetId(2);
        assert_eq!(a.gun_target(0, TargetId(9)), TargetId(2));
        let a = Aim::default();
        assert_eq!(a.gun_target(0, TargetId(9)), TargetId(9));
    }
}
