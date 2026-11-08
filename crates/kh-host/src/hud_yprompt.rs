//! (PROMPT-UI) THE Y-COMMAND OWNER'S PROMPTS ON THE HUD (the user, 2026-11-05: "make sure this attack prompt appears
//! whenever it should in KH3 as well as those arrows above the command menu (for all characters now and in the future
//! that use the Y commands"). A child module of `hud.rs` (it reads the Hud's fields). Both pieces draw ONLY what the
//! snapshot carries (`UiSnapshot::prompt`, `situation.gauge_ratio`), and the snapshot carries the prompts of whoever's
//! command menu the HUD shows: `kh_core::ui_model::YPrompts::apply`, the one gate (kh-partyhost `hud_snapshot` and kh-er
//! `partyui` for a leading friend; a Sora core fills his own, the player's, AI Sora's and a Mimic's alike). A later
//! character with Y commands publishes his `YPrompts` and gets both with no HUD code.
//!
//! - THE BUTTON NOTICE ("ATTACK!" with the Decide icon and the two lines; "DODGE!" on Cancel): HudTarget's
//!   `buttonNoticeSet.buttonNotice` through `TargetDriver::button_notice` (KH3's 0x1416793E0). KH3's cases, traced from
//!   the callers of 0x1416793E0's three wrappers (build/sora_tmp/weapons/PROMPT-UI/STATE.md): the airstep's enemy stop
//!   (0x14140DEF0), the controller's reaction prompt 0x141414350 (Blow Counter, Risk Dodge's counter, Revenge Slash II,
//!   Rage Raid), the guard counter window 0x1414145C0, a finish's extra command (StyleFinish slots 162 / 163), the risk
//!   dodge's "DODGE!" (0x14158B260). The used ones leave with `select` (the button flash), the others with `out`.
//! - THE ATTACK SLOT'S TEXT: 0x14158C180 also renames Attack with the notice's command (0x14162A440); the counter
//!   window's is F-HUD's `hud_forms::attack_text`, the others' here.
//! - THE STYLE-POINT GAUGE ROW (">>" above the situation panel): HudCommand's class-3 entry (icon type 100, kept last in
//!   the list) by 0x14162DC40's rule: added with phase 1 at a quarter of the gauge, `setStylePointPhase` 1 / 2 / 3 at
//!   0.25 / 0.5 / 0.75, taken away below 0.25 and at a full gauge (the formchange is offered); a new counted situation
//!   command takes it away too (0x1416158F0 calls 0x14162DC40(hud, 0)) until the controller sends a new ratio.

use std::sync::Mutex;

use kh_core::ui_model::{ButtonPrompt, PromptKind};
use kh_ui::command::{CommandModel, Situation};
use kh_ui::hud_command::ACTION_COMMAND_TYPE_STYLE_POINT_GAUGE;
use kh_ui::hud_target::{notice_label_key, NOTICE_HIDE, NOTICE_SHOW, NOTICE_USED};

use super::{Frame, Hud};

/// The serial of the gauge row in the command model (far from the core's and the host reactions').
pub const GAUGE_ROW_SERIAL: u32 = 0x7E00_0064;

/// The HUD's side of the notice between frames.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NoticeState {
    /// The counter window's seconds left in the last frame (its notice is "used" when it closed before its time).
    counter_left: f32,
}

/// The gauge row's state: HudCommand+0x9D4 (the phase; +0x9D0, the ratio it holds, drives only the phase), the entry,
/// the controller's last sent ratio and the counted rows seen.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GaugeRow {
    phase: u8,
    shown: bool,
    sent: Option<f32>,
    counted: Vec<u32>,
}

impl GaugeRow {
    /// One HUD frame: `ratio` = the owner's gauge (None: none), `counted` = the counted situation rows now.
    pub fn step(&mut self, ratio: Option<f32>, counted: &[u32]) -> bool {
        // 0x1416158F0: a new counted entry while the gauge entry stands -> 0x14162DC40(hud, 0.0).
        let added = counted.iter().any(|s| !self.counted.contains(s));
        self.counted = counted.to_vec();
        if added && self.shown {
            self.set(0.0);
        }
        // 0x14158C410: the controller sends its ratio when it changes (AddStylePoint, the decay, the reset).
        let r = ratio.unwrap_or(0.0);
        if self.sent != Some(r) {
            self.sent = Some(r);
            self.set(r);
        }
        self.shown
    }

    /// `0x14162DC40(hud, ratio)` [decompiled C].
    fn set(&mut self, ratio: f32) {
        let phase = kh_core::ui_model::prompts::gauge_phase(ratio);
        if phase >= 4 {
            // 1.0 and above: the entry is taken away (0x14161BB00), the phase is kept.
            self.shown = false;
            return;
        }
        if phase == self.phase {
            return;
        }
        self.phase = phase;
        // phase 0 -> 0x14161BB00 (removed); else 0x141616AA0 (added unless there) and setStylePointPhase(phase)
        self.shown = phase != 0;
    }

    pub fn shown(&self) -> bool {
        self.shown
    }

    pub fn phase(&self) -> u8 {
        self.phase
    }
}

/// Was the notice taken by its button when it went (KH3: mode 3 "used" -> `select`; else mode 1 -> `out`).
fn used(before: &ButtonPrompt, f: &Frame, counter_left: f32) -> bool {
    let attack = f.pressed.has(kh_core::PadButton::ATTACK);
    match before.kind {
        PromptKind::RiskDodge => f.sora.risk_dodge().is_some(),
        PromptKind::AirstepAttack => f.sora.airstep().is_some_and(|a| a.attack_latched) || f.state == kh_core::StateId::Attack,
        // 0x141414350(ctrl, kind, 0.0): CounterUsed (mode 3); a time out or a withdrawal (kind 0) is mode 1
        PromptKind::Reaction(_) => attack,
        // 0x1414145C0(ctrl, 0.0, ...): closed by the function (used / cut short) is mode 3; the countdown (0x1413F8130) mode 1
        PromptKind::Counter(_) => attack || counter_left > f.dt + f.step_dt,
        // StyleFinish slot 163 0x1413CFF40: 0x14158C180(hud, 3, kind) always
        PromptKind::FinishExtra(_) => true,
    }
}

impl Hud {
    /// (PROMPT-UI) THE BUTTON NOTICE of `UiSnapshot::prompt` (in place of the push-button mark the integration pass
    /// used: that clip, `pushBtnMark`, is `setupPushButtonMark`'s, not 0x1416793E0's).
    pub(super) fn button_notice(&mut self, f: &Frame) {
        let now = f.snapshot.prompt;
        let before = self.prompt;
        let label = |p: &ButtonPrompt| p.notice_key_label();
        match (before, now) {
            (None, None) => {}
            // KH3: a show while the notice stands is ignored (+0x2E0); the same notice simply stays.
            (Some(b), Some(n)) if label(&b) == label(&n) => self.prompt = Some(n),
            _ => {
                if let Some(b) = before {
                    let mode = if used(&b, f, self.notice.counter_left) { NOTICE_USED } else { NOTICE_HIDE };
                    self.target_driver.button_notice(&mut self.target, mode, b.notice_key_label().0, "", false);
                }
                if let Some(n) = now {
                    let (key, label) = n.notice_key_label();
                    let text = notice_label_key(label).map(|k| self.camp_view.string(k)).unwrap_or_default();
                    self.target_driver.button_notice(&mut self.target, NOTICE_SHOW, key, &text, false);
                }
                self.prompt = now;
            }
        }
        self.notice.counter_left = f.sora.actions().counter_window;
    }

    /// (PROMPT-UI) The Attack slot named by the notice's command (0x14158C180 -> 0x14162A440), unless the counter window
    /// already named it (F-HUD's `attack_text`).
    pub(super) fn notice_attack_text(&self, f: &Frame, model: &mut CommandModel, name_of: &dyn Fn(i64, &str) -> String) {
        let Some(p) = f.snapshot.prompt else { return };
        let Some(kind) = p.command().filter(|_| !matches!(p.kind, PromptKind::Counter(_))) else { return };
        let text = name_of(i64::from(kind), "");
        if !text.is_empty() && model.main[0].text != text {
            model.main[0].text = text;
            model.main[0].flash = true;
        }
    }

    /// (PROMPT-UI) THE GAUGE ROW: appended last to the situation rows while 0x14162DC40's rule keeps it (`counted` = the
    /// owner's counted rows, before the host's own reactions are laid on).
    pub(super) fn gauge_row(&self, model: &mut CommandModel, ratio: f32, counted: &[u32]) {
        let shown = self.gauge.lock().map(|mut g| g.step((ratio > 0.0).then_some(ratio), counted)).unwrap_or(false);
        if shown && !model.situations.iter().any(|s| s.serial == GAUGE_ROW_SERIAL) {
            model.situations.push(Situation { serial: GAUGE_ROW_SERIAL, icon_type: ACTION_COMMAND_TYPE_STYLE_POINT_GAUGE, enabled: true, ..Situation::default() });
        }
    }

    /// (PROMPT-UI) The gauge row as the HUD holds it (tests, pictures): shown and its phase.
    pub fn gauge_row_state(&self) -> (bool, u8) {
        self.gauge.lock().map(|g| (g.shown(), g.phase())).unwrap_or((false, 0))
    }

    /// (PROMPT-UI) The notice as the HUD shows it (tests, pictures): its key while up, the clip's label.
    pub fn notice_state(&self) -> (Option<i32>, Option<String>) {
        (self.target_driver.notice_shown(), self.target.button_notice_label())
    }
}

/// The gauge row's state lives behind a lock: `live_model` is `&self`.
pub type GaugeLock = Mutex<GaugeRow>;

#[cfg(test)]
mod tests {
    use super::*;

    /// 0x14162DC40's rule: a quarter adds it, below a quarter or at full it goes; a new counted row takes it away until
    /// the controller sends a new ratio.
    #[test]
    fn prompt_ui_gauge_row_follows_kh3s_phase_rule() {
        let mut g = GaugeRow::default();
        assert!(!g.step(Some(0.1), &[]), "below a quarter: none");
        assert!(g.step(Some(0.3), &[]) && g.phase() == 1, "a quarter: phase 1");
        assert!(g.step(Some(0.6), &[]) && g.phase() == 2);
        assert!(g.step(Some(0.8), &[]) && g.phase() == 3);
        assert!(!g.step(Some(1.0), &[]) && g.phase() == 3, "full: taken away, the phase kept");
        assert!(!g.step(Some(0.9), &[]), "back below full with the same phase: not added again (0x9D4 unchanged)");
        assert!(g.step(Some(0.6), &[]) && g.phase() == 2);
        assert!(!g.step(Some(0.0), &[]) && g.phase() == 0, "reset: gone");
        assert!(g.step(Some(0.55), &[1]) && g.phase() == 2);
        assert!(!g.step(Some(0.55), &[1, 2]), "a new counted row takes it away (0x14162DC40(hud, 0))");
        assert!(!g.step(Some(0.55), &[1, 2]), "until the controller sends a new ratio");
        assert!(g.step(Some(0.57), &[1, 2]) && g.phase() == 2, "the next point brings it back");
        assert!(g.step(Some(0.57), &[2]), "a row leaving changes nothing");
        assert!(!g.step(None, &[2]), "no gauge (a friend leading): none");
    }
}
