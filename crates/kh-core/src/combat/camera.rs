//! The attack camera (docs/sora-combat-spec.md 7.12.1): the controller function 0x1414122A0
//! (thunk 0x14140DF20) `(ctrl, bOn, mode, targetComponent, pPosition)`, read in the decompiled C
//! (verdict same-partial; the constants are immediates of the C, the easing dispatch of the
//! interpolator was read in the disassembly). The mode is controller+0xAB0, the target pawn
//! controller+0xAD8 (`m_pAttackCameraTarget`).
//!
//! ```text
//! off (bOn = 0): nothing when the mode is 0. Else, on the normal camera:
//!     look-point rate / lag limit back to the class defaults (0x1405D4730: at once; mode 6: over
//!     1.0 s; mode 7: over 0.1 s), the lock-on yaw limit back to the default (0x140600BD0), distance
//!     scale 1.0 (1.35 when controller+0xAE0: not Sora with the Kingdom Key);
//!     the cameras of modes 1 / 2 / 8 (+0xAB8 / +0xAD0) end with {0.5 s, ease-out, 2.0};
//!     mode 1, 2, 3 or 6: manual lock active (ctrl.vt+0x1680) and its target alive: re-lock the
//!         camera on it (ctrl.vt+0x1788; not when controller+0x880 == 0xDC), else Camera.SetLockon(null);
//!     mode = 0
//! on, mode 0: nothing.          on, mode 3: Camera.SetLockon(target); target pawn stored
//! on, mode 4: rate 2.0, lag limit 2000 (0x1405C63A0), lock yaw limit 0 (0x14060A7D0), scale 1.0
//! on, mode 5: distance scale 1.6          on, mode 6: 3 then 4          on, mode 7: 0.2 / 3000 as 4
//! on, mode 1 / 2 / 8: a camera actor of its own (other keyblades' finishers, shooter mode): not ported
//! ```
//!
//! Callers for Sora with the Kingdom Key: `PlayAttack` kind 0x38 Buzz Saw (off, on 5) and 0x28
//! Hook Upper (off, on 3 with the target); `Attack::End` for kinds 0x28 - 0x2B, 0x36 - 0x39 (off);
//! Sonic Rave (off, on 3 with the target at the first dash; off at LMEX_FINISH and at the end);
//! the StyleFinish end event (off); BadStatBurn (off, on 4; off at its sub-step 3 and End);
//! PoleTurn Begin (off, on 5) / End (off); RiskDodge Begin (off, on 3 with the attacker) / End (off).
//!
//! The host: `EventKind::AttackCamera` for the normal camera's part, the existing
//! `EventKind::CameraLock` for the lock (`kh_camera::CameraRig::{set_attack_mode, set_lockon}`).

use crate::host::{EventKind, TargetFlags, TargetId};
use crate::pawn::Ctx;

/// controller+0xAB0 and +0xAD8.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttackCam {
    pub mode: u8,
    pub target: TargetId,
    /// Requests switched on so far (for tests and the debug text).
    pub requests: u32,
}

impl Default for AttackCam {
    fn default() -> AttackCam {
        AttackCam { mode: 0, target: TargetId::NONE, requests: 0 }
    }
}

/// `0x14140DF20(ctrl, 0, 0, null, 0)`.
pub(crate) fn off(cx: &mut Ctx) {
    request(cx, false, 0, TargetId::NONE);
}

/// `0x14140DF20(ctrl, bOn, mode, target, 0)`.
pub(crate) fn request(cx: &mut Ctx, on: bool, mode: u8, target: TargetId) {
    if !on {
        let was = cx.pawn.combat.attack_cam.mode;
        if was == 0 {
            return;
        }
        let restore = match was {
            6 => 1.0,
            7 => 0.1,
            _ => -1.0,
        };
        cx.pawn.event(EventKind::AttackCamera { mode: 0, restore });
        if matches!(was, 1..=3 | 6) {
            let lock = &cx.pawn.combat.lockon;
            let manual = if lock.locked { lock.current } else { TargetId::NONE };
            let alive = manual != TargetId::NONE && cx.target(manual).is_some_and(|t| t.flags.has(TargetFlags::ALIVE));
            cx.pawn.event(EventKind::CameraLock { target: if alive { manual } else { TargetId::NONE } });
        }
        (cx.pawn.combat.attack_cam.mode, cx.pawn.combat.attack_cam.target) = (0, TargetId::NONE);
        return;
    }
    if mode == 0 {
        return;
    }
    match mode {
        3 | 6 => {
            // `if manager: Camera.SetLockon(manager, target)`: a null target unlocks the camera.
            cx.pawn.event(EventKind::CameraLock { target });
            cx.pawn.combat.attack_cam.target = target;
            if mode == 6 {
                cx.pawn.event(EventKind::AttackCamera { mode, restore: -1.0 });
            }
        }
        4 | 5 | 7 => cx.pawn.event(EventKind::AttackCamera { mode, restore: -1.0 }),
        // 1, 2, 8: cameras of their own; the mode is still stored (the off call then runs).
        _ => {}
    }
    cx.pawn.combat.attack_cam.mode = mode;
    cx.pawn.combat.attack_cam.requests += 1;
}
