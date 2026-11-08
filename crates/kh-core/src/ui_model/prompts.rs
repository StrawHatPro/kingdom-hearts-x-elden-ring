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
//!   Modes of the HUD function: 0 show (clip label by the flag), 1 hide, 2 used, 3 a third leave
//!   label. WHERE the clip is put on screen was not read (list item of the integration note).

use crate::host::TargetId;
use crate::input::PadButton;

/// Which prompt (the label string and the button).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptKind {
    /// "DODGE!" on the jump button: the Reflect state's 0.6 s risk-dodge window.
    RiskDodge,
    /// "ATTACK!" on the attack button: the airstep's enemy stop while the lock flag is up.
    AirstepAttack,
    /// (PROMPT-UI) "ATTACK!": a reaction command of the controller's prompt `0x141414350(ctrl, kind, time, _)` ->
    /// `0x14158C180(hud, 0, kind, 0)` (Aerial Recovery's Blow Counter, Risk Dodge's counter, Final Blow's Revenge Slash II,
    /// Rage Raid): the port's ex-kind entry in slot A / B (`combat::situation::set_reaction`). The command kind.
    Reaction(i32),
    /// (PROMPT-UI) "ATTACK!": the guard counter window `0x1414145C0` -> `0x14158C180(hud, 0, ctrl+0x8A4, 0)`. The kind.
    Counter(i32),
    /// (PROMPT-UI) "ATTACK!": a finish's extra command, StyleFinish slot 162 `0x1413E3690` -> `0x14158C180(hud, 0, cmd,
    /// 0)` (slot 163 `0x1413CFF40` takes it away with mode 3, "used"). The kind (0xAA, 0xAF .. 0xB2, 0xF3).
    FinishExtra(i32),
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
            _ => PadButton::ATTACK,
        }
    }

    /// The string table key of its label.
    pub fn label_key(&self) -> &'static str {
        match self.kind {
            PromptKind::RiskDodge => "kh3_system.cockpit.cockpit_0006",
            _ => "kh3_system.cockpit.cockpit_0003",
        }
    }

    /// (PROMPT-UI) The key and label the controller hands the HUD's notice function 0x1416793E0: "DODGE!" key 0xC label
    /// 3 (0x14158B260), every "ATTACK!" key 0xB label 1 (0x14158B120 / 0x14158C180).
    pub fn notice_key_label(&self) -> (i32, i32) {
        match self.kind {
            PromptKind::RiskDodge => (0xC, 3),
            _ => (0xB, 1),
        }
    }

    /// (PROMPT-UI) The command 0x14158C180 also puts on the Attack slot (`0x14162A440(HudCommand, kind, 0)` =
    /// `setMainCommandText(0, name)`; restored by 0x141627150 when the notice goes), if any.
    pub fn command(&self) -> Option<i32> {
        match self.kind {
            PromptKind::Reaction(k) | PromptKind::Counter(k) | PromptKind::FinishExtra(k) if k != 0 => Some(k),
            _ => None,
        }
    }
}

/// (PROMPT-UI) THE Y-COMMAND OWNER'S PROMPTS: the button notice and the style-point gauge row (">>" above the command
/// menu's situation panel), as KH3's controller sends them to its own HUD. ONE GATE FOR EVERY CHARACTER: whoever's
/// command menu the HUD shows (Sora leading, a friend leading, any later character) puts his own here (`apply`), and the
/// HUD draws only what the snapshot carries. In KH3 every writer of both is a player controller or a player state
/// (`TresPlayerControllerBase` slots 829 / 836 -> 0x14158C410 for the gauge; 0x14158B120 / 0x14158C180 / 0x14158C260 /
/// 0x14158B260 for the notice); no friend pawn (Donald, Goofy) calls them.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct YPrompts {
    pub notice: Option<ButtonPrompt>,
    /// The gauge's ratio 0..1 (KH3: controller+0xB40 / MyStylePointMax, 0x14141C4F0 / 0x14141DD00); `None` = no gauge.
    pub gauge: Option<f32>,
}

impl YPrompts {
    /// What a snapshot carries (Sora's own: `ui_model::fill` and `Sora::step_hosted`'s situation).
    pub fn of(s: &super::UiSnapshot) -> YPrompts {
        YPrompts { notice: s.prompt, gauge: (s.situation.gauge_ratio > 0.0).then_some(s.situation.gauge_ratio) }
    }

    /// THE GATE: the HUD's snapshot shows these prompts and no one else's (`prompt`, the gauge's ratio and phase).
    pub fn apply(&self, s: &mut super::UiSnapshot) {
        s.prompt = self.notice;
        let ratio = self.gauge.unwrap_or(0.0);
        s.situation.gauge_ratio = ratio;
        s.situation.gauge_phase = ((ratio * 4.0) as u8).min(3); // as combat/situation.rs `hud` fills it
    }
}

/// (PROMPT-UI) The other "ATTACK!" notices of a Sora core (the player's, AI Sora's, a Mimic's: the same pawn state), after
/// the risk dodge and the airstep, in this order: the guard counter window (`actions.counter_window` with its command,
/// 0x1414145C0), a reaction command of the controller's prompt (an ex-kind entry in slot A, else slot B: 0x141414350's
/// callers as the port enters them, `combat::situation::set_reaction`), a finish's extra command (`situation.extra`).
/// KH3 has one notice clip for all of them (the first shown stands until it is taken away).
pub(crate) fn attack_notice(pawn: &crate::pawn::Pawn) -> Option<ButtonPrompt> {
    let a = &pawn.actions;
    if a.counter_window > 0.0 && a.counter_command != 0 {
        return Some(ButtonPrompt { kind: PromptKind::Counter(a.counter_command), target: a.counter_attacker });
    }
    let q = &pawn.combat.situation;
    if let Some(e) = q.slot_a.iter().chain(q.slot_b.iter()).find(|e| e.ex && e.kind != 0) {
        return Some(ButtonPrompt { kind: PromptKind::Reaction(e.kind), target: TargetId::NONE });
    }
    q.extra.filter(|k| *k != 0).map(|k| ButtonPrompt { kind: PromptKind::FinishExtra(k), target: TargetId::NONE })
}

/// (PROMPT-UI) The gauge's phase (HudCommand 0x14162DC40): 0 below 0.25, 1 below 0.5, 2 below 0.75, 3 below 1.0; at 1.0
/// and above the entry is taken away (4 here).
pub fn gauge_phase(ratio: f32) -> u8 {
    if ratio < 0.25 {
        0
    } else if ratio < 0.5 {
        1
    } else if ratio < 0.75 {
        2
    } else if ratio < 1.0 {
        3
    } else {
        4
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

    /// (PROMPT-UI) KH3's "ATTACK!" cases of a Sora core, each shown while its source stands and gone with it: the counter
    /// window (0x1414145C0), a reaction command of the controller's prompt (0x141414350, the port's ex-kind entry), a
    /// finish's extra (StyleFinish slots 162 / 163); the counter comes first; every one is key 0xB, label 1 (ATTACK!),
    /// and names its command for the Attack slot.
    #[test]
    fn prompt_ui_every_attack_notice_of_a_sora_core() {
        use crate::anim::{AnimId, AnimPlayer};
        use crate::cmc::Cmc;
        use crate::combat::situation;
        use crate::host::NullHost;
        use crate::params::SORA_PARAMS;
        use crate::pawn::{Ctx, Pawn};
        use crate::test_world::TestWorld;
        let mut p = Pawn::new(SORA_PARAMS, Cmc::new(&SORA_PARAMS, glam::Vec3::ZERO), 0.0, AnimPlayer::new(AnimId::IdleN));
        p.status.hp = 100;
        let world = TestWorld::floor();
        let mut null = NullHost::default();
        let mut host = null.host(&world);
        let mut cx = Ctx::new(&mut p, &mut host);
        assert_eq!(attack_notice(cx.pawn), None, "nothing up");
        // a reaction command (Aerial Recovery's Blow Counter, kind 2)
        situation::set_reaction(&mut cx, 2, 2.0);
        let n = attack_notice(cx.pawn).expect("the reaction prompt");
        assert_eq!((n.kind, n.notice_key_label(), n.command(), n.label_key()), (PromptKind::Reaction(2), (0xB, 1), Some(2), "kh3_system.cockpit.cockpit_0003"));
        // the counter window comes first while both stand
        (cx.pawn.actions.counter_window, cx.pawn.actions.counter_command, cx.pawn.actions.counter_attacker) = (1.0, 3, TargetId(7));
        assert_eq!(attack_notice(cx.pawn), Some(ButtonPrompt { kind: PromptKind::Counter(3), target: TargetId(7) }));
        crate::actions::close_counter_window(cx.pawn);
        assert_eq!(attack_notice(cx.pawn).map(|n| n.kind), Some(PromptKind::Reaction(2)));
        situation::clear_reaction(&mut cx, 2);
        assert_eq!(attack_notice(cx.pawn), None, "the reaction prompt went");
        // a finish's extra (Sonic Rave's 0xAF)
        situation::set_extra(&mut cx, 0xAF);
        assert_eq!(attack_notice(cx.pawn).map(|n| (n.kind, n.command(), n.button())), Some((PromptKind::FinishExtra(0xAF), Some(0xAF), PadButton::ATTACK)));
        situation::clear_extra(&mut cx);
        assert_eq!(attack_notice(cx.pawn), None);
        // "DODGE!" is key 0xC, label 3, on Jump (B)
        let dodge = ButtonPrompt { kind: PromptKind::RiskDodge, target: TargetId::NONE };
        assert_eq!((dodge.notice_key_label(), dodge.button(), dodge.command()), ((0xC, 3), PadButton::JUMP, None));
    }

    /// (PROMPT-UI) The gate: a snapshot shows the Y owner's prompts and no one else's.
    #[test]
    fn prompt_ui_the_gate_replaces_the_prompts_of_the_snapshot() {
        let mut s = super::super::UiSnapshot { prompt: Some(ButtonPrompt { kind: PromptKind::Counter(3), target: TargetId(1) }), ..Default::default() };
        s.situation.gauge_ratio = 0.6;
        s.situation.gauge_phase = 2;
        let soras = YPrompts::of(&s);
        assert_eq!(soras.gauge, Some(0.6));
        YPrompts::default().apply(&mut s);
        assert_eq!((s.prompt, s.situation.gauge_ratio, s.situation.gauge_phase), (None, 0.0, 0), "a friend with none: Sora's go");
        soras.apply(&mut s);
        assert_eq!((YPrompts::of(&s), s.situation.gauge_phase), (soras, 2));
        assert_eq!([0.1, 0.25, 0.5, 0.74, 0.75, 0.99, 1.0].map(gauge_phase), [0, 1, 2, 2, 3, 3, 4]);
    }
}
