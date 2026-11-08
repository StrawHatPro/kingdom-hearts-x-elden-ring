//! (F-HUD) The formchange pieces of the target layer `HudTarget`: the ShooterMode SCOPE and its charge states, and the
//! multi-lock MARKERS. The AS3 half is `HudTarget::set_shooter_mode_scope` (method 764) and the target cursors (type 0
//! `ALockonMark`); this is the native half, traced from the KH3 decomp:
//!
//! - ShooterMode's SetCamera (slot 114 0x1413BED50): ON -> `0x14140D670(ctrl, +0xB8)` -> `0x141589750(hud, launcher)` ->
//!   `setShooterModeScope(SCOPE_IN, launcher)` on HudTarget (hud+0x68, `0x14167B6D0(movie, 0, launcher)`) and
//!   `0x141625900(HudCommand, 1)` (the command menu's `setActionSelect(4)`: the situation commands darken); OFF (a camera
//!   was on) -> `0x14140BF50(ctrl, +0xB8)` -> `0x141577E00(hud, launcher, 0)` -> `setShooterModeScope(SCOPE_OUT, launcher)`
//!   and `0x141625900(HudCommand, 0)` (`setActionSelect(5)`); ShooterMode's End (0x1413ABE40) does the OFF part again.
//! - The launcher's charge (Tick 0x1413B0DC0 sub-steps 4 / 5, End): `0x14140BE90(ctrl, +0xB8, full)` ->
//!   `0x141576A80(hud, launcher, full)` -> `setShooterModeScope(full ? SCOPE_FULL_CHARGE : SCOPE_CHARGING, launcher)`
//!   (`(full ^ 1) + 3`); the AS3 ignores both for the arrowguns' scope (`a2 == 0`: `shootScpW` has no charge states).
//! - `launcher` = ShooterMode +0xB8, the charge mode of weapon kinds 5 / 0x11 (Magic Launcher, Hunny Launcher): the
//!   launchers' scope is `shootScpB`, the arrowguns' `shootScpW`.
//! - The multi-lock (UpdateLockon step 9, 0x141406894): a type-0 cursor on every entry of the switch list while the
//!   multi-lock mode is on (`kh_core` `aim::show_markers`, which also sends `LockCursor` events).
//! - The aim's target gauge (ShooterMode slot 115 0x1413BDA70 -> `0x14158A860(hud, target, 0)` -> hud+0x88+0x200
//!   `0x14165A050`): the enemy gauge points at the aim's target (the host's `EnemyGauge::point_at`).

use crate::hud_target::{HudTarget, SCOPE_CHARGING, SCOPE_FULL_CHARGE, SCOPE_IN, SCOPE_OUT};

/// What the scope driver reads each frame (from kh-core's `ShooterView`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ScopeIn {
    /// The shooter camera is on (`view.camera`).
    pub camera: bool,
    /// The launcher's charge mode (`view.launcher`).
    pub launcher: bool,
    /// The charge is full (`view.charged`, ShooterMode +0xC8).
    pub charged: bool,
}

/// What the scope's native calls ask of the command menu: `0x141625900(HudCommand, on)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandDim {
    /// `setActionSelect(4)`: the situation commands darken (ShooterMode's camera came on).
    Dark,
    /// `setActionSelect(5)`: back (the camera went).
    Back,
}

/// The native side of the scope (the cockpit's calls above), per frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ScopeDriver {
    shown: ScopeIn,
}

impl ScopeDriver {
    pub fn new() -> ScopeDriver {
        ScopeDriver::default()
    }

    /// One frame: the calls of the frame's changes, in KH3's order. Returns the command menu's dim request, if any.
    pub fn update(&mut self, hud: &mut HudTarget, now: ScopeIn) -> Option<CommandDim> {
        let was = self.shown;
        let launcher = |s: ScopeIn| i32::from(s.launcher);
        let mut dim = None;
        if now.camera && !was.camera {
            hud.set_shooter_mode_scope(SCOPE_IN, launcher(now));
            dim = Some(CommandDim::Dark);
        }
        if was.camera && now.charged != was.charged && now.camera {
            hud.set_shooter_mode_scope(if now.charged { SCOPE_FULL_CHARGE } else { SCOPE_CHARGING }, launcher(now));
        }
        if was.camera && !now.camera {
            // SetCamera(0) / End: the charge-full flag goes first (End's 0x14140BE90(.., 0)), then the scope's out.
            if was.charged {
                hud.set_shooter_mode_scope(SCOPE_CHARGING, launcher(was));
            }
            hud.set_shooter_mode_scope(SCOPE_OUT, launcher(was));
            dim = Some(CommandDim::Back);
        }
        self.shown = now;
        dim
    }

    pub fn open(&self) -> bool {
        self.shown.camera
    }
}

/// The multi-lock's type-0 markers to show this frame: the switch list while the multi-lock mode is on (step 9), as the
/// HUD's cursor keys (the core's `TargetId.0`).
pub fn multilock_markers(multilock: bool, list: &[u32]) -> Vec<u64> {
    if multilock {
        list.iter().map(|t| u64::from(*t)).collect()
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_charge_states_follow_the_camera() {
        let mut pack = crate::Pack::open_default().expect("ui pack");
        let fonts = crate::Fonts::load(&mut pack).expect("fonts");
        let mut hud = HudTarget::new(pack.movie("HudTarget").expect("HudTarget"), fonts);
        let mut d = ScopeDriver::new();
        let on = ScopeIn { camera: true, launcher: true, charged: false };
        assert_eq!(d.update(&mut hud, on), Some(CommandDim::Dark));
        assert_eq!(d.update(&mut hud, on), None);
        d.update(&mut hud, ScopeIn { charged: true, ..on });
        d.update(&mut hud, on);
        assert_eq!(d.update(&mut hud, ScopeIn::default()), Some(CommandDim::Back));
        assert_eq!(hud.calls, ["setShooterModeScope(0, 1)", "setShooterModeScope(3, 1)", "setShooterModeScope(4, 1)", "setShooterModeScope(1, 1)"]);
        assert_eq!(hud.movie.current_label("shootScpB"), Some("out"));
        // The arrowguns: the white scope, no charge.
        hud.calls.clear();
        let dw = ScopeIn { camera: true, launcher: false, charged: false };
        d.update(&mut hud, dw);
        assert_eq!(hud.movie.current_label("shootScpW"), Some("in"));
        d.update(&mut hud, ScopeIn::default());
        assert_eq!(hud.calls, ["setShooterModeScope(0, 0)", "setShooterModeScope(1, 0)"]);
        assert_eq!(multilock_markers(true, &[3, 4]), vec![3, 4]);
        assert!(multilock_markers(false, &[3]).is_empty());
    }
}
