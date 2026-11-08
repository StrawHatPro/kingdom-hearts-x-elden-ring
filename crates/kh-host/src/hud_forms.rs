//! (F-HUD) The formchange HUD of the host: everything KH3 shows for a form that the F-packages' cores publish, wired to
//! the kh-ui pieces. A sibling of `hud.rs` (like `hud_workshop`): `Hud` keeps one [`FormsHud`] and calls it from
//! `Hud::load` / `Hud::frame` / `live_model` (the INT lines in build/sora_tmp/weapons/F-HUD/STATE.md).
//!
//! - THE FORM STYLE of the command menu (any formchange): the logo on the title plate and the logo gauge
//!   (`kh_ui::forms_hud_style`, KH3 0x14162EF30 / 0x14161C3F0): [`FormsHud::fill_model`].
//! - SHOOTERMODE (Double Arrowguns, Magic Launcher, Hunny Blasters, Hunny Launcher): the scope (`shootScpW` / `shootScpB`)
//!   with the launchers' charge states, the command menu darkened while it is up (`kh_ui::forms_hud`), the operation guide
//!   "<RT hold> Auto-Shot" (`kh_ui::forms_hud_operation`), the enemy gauge on the aim's target: [`FormsHud::frame`].
//! - THE MULTI-LOCK markers (type-0 cursors on the switch list): [`FormsFrame::markers`].
//! - COUNTER SHIELD'S RIDE: the logo gauge runs on the ride's timer (StyleFinishSH Tick sub 4: controller vt+0x1B38 =
//!   0x141415D20 writes ctrl+0xB6C, which the form gauge 0x1416318A0 reads through vt+0x1B30 for an entry whose kind is not
//!   the controller's current one) [I: that the form's kind is not current during the finish is not traced]:
//!   [`FormsHud::fill_model`].
//! - THE COUNTER WINDOW'S COMMAND ON ATTACK: `0x1414145C0` (the counter window) -> `0x14158C180(hud, 0, id, 0)` ->
//!   `0x14162A440(HudCommand, id, 0)` -> `0x141618040` = `setMainCommandText(0, name(id), flash)`: KH3 renames the Attack
//!   slot with the window's command for as long as the window lasts (Counter Shield: TresCmdData 0x10 "Wrathful Fist" /
//!   0x11 "Wrathful Flurry" with a grown shield, Frying Pan 0x12 "Flame Torrent" / 0x13 "Flame Barrage", 2 s:
//!   kh-core `actions::form_counter`). VERIFIED against `data/commands.json` (TresCmdData: cate 5, loc keys
//!   `kh3_system.command.shieldcounter` .. `shieldcounter4`). F-SHIELD's `shield::attack_command` (a label by grow level)
//!   is NOT what KH3 does: the label belongs to the counter window, not to the grow level. [`attack_text`].

use std::collections::HashMap;

use kh_core::combat::forms::arrowgun::aim::ShooterView;
use kh_core::combat::forms::shield;
use kh_core::host::TargetId;
use kh_core::sora::Sora;
use kh_ui::command::CommandModel;
use kh_ui::forms_hud::{multilock_markers, CommandDim, ScopeDriver, ScopeIn};
use kh_ui::forms_hud_operation::{OperationGuide, OperationKind};
use kh_ui::forms_hud_style::FormStyles;
use kh_ui::hud_target::HudTarget;
use kh_ui::player::Movie;
use kh_ui::{Fonts, Pack};

/// Pegasus's `m_Param_MaxTime` (ctor 0x1413E8300; the blueprint sets none): the ride's seconds.
pub const RIDE_MAX_TIME: f32 = 10.0;

/// What one frame of the form HUD asks of the rest of `Hud::frame`.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct FormsFrame {
    /// The multi-lock's type-0 (AUTO) cursors: append them to `wanted` as `LOCKON_TYPE_AUTO` before the reconcile.
    pub markers: Vec<u64>,
    /// The aim's target changed to this one: point the enemy gauge at it (`EnemyGauge::point_at(key, false, None)`).
    pub gauge_target: Option<u64>,
    /// The command menu's situation rows: `Dark` = `setActionSelect(4)`, `Back` = `setActionSelect(5)`.
    pub dim: Option<CommandDim>,
}

/// The host's state of the formchange HUD.
pub struct FormsHud {
    scope: ScopeDriver,
    styles: FormStyles,
    operation: Option<OperationGuide>,
    gauge_target: TargetId,
}

/// ShooterMode's scope input from the core's view.
pub fn scope_in(view: &ShooterView) -> ScopeIn {
    ScopeIn { camera: view.camera, launcher: view.launcher, charged: view.charged }
}

/// The Attack slot's text while a counter window is open (its command's name; None = the slot's own text).
pub fn attack_text(sora: &Sora, names: &HashMap<i64, String>) -> Option<String> {
    let a = sora.actions();
    if a.counter_window <= 0.0 || a.counter_command == 0 {
        return None;
    }
    names.get(&i64::from(a.counter_command)).cloned()
}

/// The logo gauge during Counter Shield's ride: (the ride's seconds left, the form's total).
pub fn ride_gauge(sora: &Sora, form_total: f32) -> Option<(f32, f32)> {
    shield::ride_gauge(sora.finish(), sora.combat().form.cmd).map(|r| (r * RIDE_MAX_TIME, form_total))
}

impl FormsHud {
    /// At `Hud::load`: the form style table with its logo movies and the operation guide's movie.
    pub fn load(pack: &mut Pack, fonts: &Fonts) -> FormsHud {
        let styles = FormStyles::load(pack);
        let operation = OperationGuide::load(pack, fonts);
        FormsHud { scope: ScopeDriver::new(), styles, operation, gauge_target: TargetId::NONE }
    }

    /// Logo movies the pack lacks (for the host's log).
    pub fn missing(&self) -> &[String] {
        &self.styles.missing
    }

    /// One HUD frame, before the cursors' reconcile (`friend_leads`: the form HUD is Sora's: nothing while a friend leads).
    pub fn frame(&mut self, sora: &Sora, target: &mut HudTarget, friend_leads: bool) -> FormsFrame {
        let aim = &sora.combat().arrowgun.aim;
        let view = if friend_leads { ShooterView::default() } else { aim.view };
        let dim = self.scope.update(target, scope_in(&view));
        if let Some(g) = &mut self.operation {
            // The guide is controller first: the pad flag (uiManager+0x6BB1) is on.
            g.update(view.camera.then_some(OperationKind::ShooterMode), true);
        }
        let gauge_target = (view.camera && view.target != TargetId::NONE && view.target != self.gauge_target).then_some(u64::from(view.target.0));
        self.gauge_target = if view.camera { view.target } else { TargetId::NONE };
        let list: Vec<u32> = aim.switch_list.iter().map(|t| t.0).collect();
        let markers = if friend_leads { Vec::new() } else { multilock_markers(aim.multilock, &list) };
        FormsFrame { markers, gauge_target, dim }
    }

    /// In `live_model`, after the form gauge: the form style, the ride's gauge, the counter window's Attack text.
    pub fn fill_model(&self, sora: &Sora, form: Option<&kh_core::ui_model::FormHud>, names: &HashMap<i64, String>, model: &mut CommandModel) {
        model.form_style = form.and_then(|f| self.styles.style_of(f.kind.game_name(), f.total));
        if let Some(f) = form {
            if let Some(g) = ride_gauge(sora, f.total) {
                model.form_gauge = Some(g);
            }
        }
        if let Some(text) = attack_text(sora, names) {
            if model.main[0].text != text {
                model.main[0].text = text;
                model.main[0].flash = true;
            }
        }
    }

    /// One stage frame (inside `for _ in 0..f.ticks`).
    pub fn tick(&mut self) {
        if let Some(g) = &mut self.operation {
            g.tick();
        }
    }

    /// The operation guide's movie (`Hud::movie("HudOperation")`).
    pub fn operation_movie(&self) -> Option<&Movie> {
        self.operation.as_ref().map(|g| g.movie())
    }
}

#[cfg(test)]
mod tests;
