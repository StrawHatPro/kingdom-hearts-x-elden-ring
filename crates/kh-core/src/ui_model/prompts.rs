//! The world-layer part of the HUD model that is not a gauge or a popup (integration pass,
//! 2026-10-12; docs/sora-ui-spec.md 7.1, 7.3): the soft target, the push-button prompt and the
//! shotlock aim, so that a host needs nothing but the snapshot to draw them.
//!
//! THE PUSH-BUTTON PROMPT [decompiled C]: the controller's two prompt calls end in the same HUD
//! function `0x1416793E0(targetHud, mode, key, label, flag)` (the `pushBtnMark` clip of
//! `HudTarget`, AS3 `setupPushButtonMark`):
//! - the risk-dodge prompt `FUN_141414170(ctl, mode)` -> `0x14158B260(hud, mode)` ->
//!   `0x14158C260(hud, mode, key 0xC, label 3)`: label 3 = string `kh3_system.cockpit.cockpit_0006`
//!   ("DODGE!"), key 0xC = icon 0x25;
//! - the airstep's attack prompt `FUN_14140DEF0(ctl, n)` -> `0x14158B120(hud, n)` -> the same
//!   function with key 0xB (icon 0x24) and label 1 = `cockpit_0003` ("ATTACK!").
//! Modes of the HUD function: 0 show (clip label by the flag), 1 hide, 2 used, 3 a third leave
//! label. WHERE the clip is put on screen was not read (list item of the integration note).

use crate::host::TargetId;
use crate::input::PadButton;

/// Which prompt (the label string and the button).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptKind {
    /// "DODGE!" on the jump button: the Reflect state's 0.6 s risk-dodge window.
    RiskDodge,
    /// "ATTACK!" on the attack button: the airstep's enemy stop while the lock flag is up.
    AirstepAttack,
}

/// A push-button prompt on screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ButtonPrompt {
    pub kind: PromptKind,
    /// The enemy it is about (the attacker / the airstep's target); `TargetId::NONE` if unknown.
    pub target: TargetId,
}

impl ButtonPrompt {
    /// The pad button that answers it (the game's default layout).
    pub fn button(&self) -> PadButton {
        match self.kind {
            PromptKind::RiskDodge => PadButton::JUMP,
            PromptKind::AirstepAttack => PadButton::ATTACK,
        }
    }

    /// The string table key of its label.
    pub fn label_key(&self) -> &'static str {
        match self.kind {
            PromptKind::RiskDodge => "kh3_system.cockpit.cockpit_0006",
            PromptKind::AirstepAttack => "kh3_system.cockpit.cockpit_0003",
        }
    }
}

/// One mark of the shotlock aim.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AimMark {
    /// The enemy, or `TargetId::NONE` for a level point (`point` is then its id).
    pub target: TargetId,
    pub point: u32,
    /// Locks on it (shotlock marks); 1 for an airstep mark.
    pub count: i32,
    /// Where the mark sits in the world (a level point's position; an enemy's is the host's).
    pub position: glam::Vec3,
}

/// The shotlock aim as the HUD shows it (UI spec 7.3).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct AimHud {
    /// Controller +0xA10: 0 off, 1 aiming, 2 fired, 3 airstep requested, 4 focus-syphon aiming.
    pub mode: u8,
    /// The aim time of the entry, seconds (focus x convert rate); 0 = the empty scope.
    pub aim_time: f32,
    /// Total locks and the maximum (the lock counter).
    pub total: i32,
    pub max: i32,
    /// The shotlock marks (type 2; type 3 while `mode` is 4), in lock order.
    pub locks: Vec<AimMark>,
    /// The airstep chain (type 4 marks), the chosen target last.
    pub airstep: Vec<AimMark>,
}

impl AimHud {
    pub fn open(&self) -> bool {
        matches!(self.mode, 1 | 4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompts_name_the_games_strings_and_buttons() {
        let risk = ButtonPrompt { kind: PromptKind::RiskDodge, target: TargetId(3) };
        assert_eq!((risk.button(), risk.label_key()), (PadButton::JUMP, "kh3_system.cockpit.cockpit_0006"));
        let attack = ButtonPrompt { kind: PromptKind::AirstepAttack, target: TargetId(1) };
        assert_eq!((attack.button(), attack.label_key()), (PadButton::ATTACK, "kh3_system.cockpit.cockpit_0003"));
        assert!(!AimHud::default().open() && AimHud { mode: 4, ..AimHud::default() }.open() && !AimHud { mode: 2, ..AimHud::default() }.open());
    }
}
