//! OWNER: the UI implementer (docs/sora-ui-spec.md). The plain-data model of the HUD and menus
//! that the core publishes once per step through [`crate::host::UiSink`].
//!
//! STATE OF THIS MODULE (2026-10-11): THE STATUS HUD'S NUMBERS ARE IMPLEMENTED by the stats
//! implementer: the gauges with their timeline FRAME numbers (`gauges.rs`, UI spec section 4:
//! the frame helper, HP / MP / focus frames, the red damage-lag gauge as a deterministic
//! fixed-step model, pinch, MP charge), level / EXP, and the popups (`popups.rs`, UI spec
//! section 8: EXP points, level up, Next LV; munny and item as stubs with the spec's timing).
//! The command menu is still empty and hidden, nothing sets the lock-on: those rules come with
//! the UI / combat implementers (put the menu's own state machine in further files of this
//! folder, `ui_model/command_menu.rs` ...). Keep the structs plain data, so that a host needs
//! nothing but them to draw. The real Scaleform-art HUD is drawn from this model by the host.

pub mod gauges;
pub mod popups;
pub mod prompts;
pub mod situation;

pub use prompts::{AimHud, AimMark, ButtonPrompt, PromptKind};
pub use gauges::{gauge_frame, Gauges, RedLag};
pub use situation::{FormHud, SituationEntry, SituationHud};
pub use popups::{ExpPoint, PopupBoard, PopupDetail, TimedPopup, UiPopup};

use crate::enums::ETresCommandKind;
use crate::host::TargetId;
use crate::status::Status;

/// One line of the command menu.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CommandEntry {
    pub kind: ETresCommandKind,
    /// False: listed but greyed out.
    pub enabled: bool,
}

/// The command menu as it is shown.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct CommandMenu {
    pub visible: bool,
    /// Index into `entries` of the highlighted line.
    pub cursor: u32,
    pub entries: Vec<CommandEntry>,
}

/// Everything the HUD shows after one step.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct UiSnapshot {
    /// `Output::step` of the step that published it.
    pub step: u64,
    /// The status HUD: values and gauge frames.
    pub gauges: Gauges,
    pub command_menu: CommandMenu,
    /// The locked-on target, `TargetId::NONE` without one.
    pub lock_on: TargetId,
    /// THE COMMAND MENU as the command system (command/mod.rs, magic implementer) shows it:
    /// state, cursors, rows with their greyed / MP-short flags, shortcut page, the shotlock
    /// panel, this step's one-shot menu events. Mirrors `kh_ui::CommandModel`.
    pub command: crate::command::CommandSnapshot,
    /// The popups RAISED in this step, in order (the same as the step's `Popup` events).
    pub popups: Vec<UiPopup>,
    /// The popups ON SCREEN after this step, with their ages (the UI spec's lifetimes).
    pub board: PopupBoard,
    /// The situation commands, the formchange gauge and the form's time (combat implementer:
    /// `ui_model/situation.rs`; UI spec 5.6 - 5.8). Filled by `Sora::step_hosted`.
    pub situation: SituationHud,
    /// (integration pass) THE SOFT TARGET: the target search's current target while no manual
    /// lock is held (the AUTO cursor, type 0 of UI spec 7.1); `TargetId::NONE` without one.
    pub soft_target: TargetId,
    /// (integration pass) The push-button prompt on screen (risk dodge "DODGE!" / the airstep's
    /// "ATTACK!"): `ui_model/prompts.rs`.
    pub prompt: Option<ButtonPrompt>,
    /// (integration pass) The shotlock aim: scope, lock counter, shotlock marks, airstep marks.
    pub aim: AimHud,
    /// (cleanup pass) `PawnCombat::sneeze_next` (controller+0x920): the HUD shows the Attack
    /// command as "Sneeze" while it is set (combat spec 9.9.5, 0x141583FF0; UI spec 5.3).
    pub sneeze: bool,
}

/// (integration pass) The combat / actions / magic sides' part of a snapshot that is plain pawn
/// state: called by `Sora` wherever it builds one.
pub(crate) fn fill(snapshot: &mut UiSnapshot, pawn: &crate::pawn::Pawn, state: &crate::states::StateData) {
    let lock = &pawn.combat.lockon;
    snapshot.soft_target = if lock.locked { TargetId::NONE } else { lock.current };
    snapshot.sneeze = pawn.combat.sneeze_next;
    snapshot.prompt = if pawn.actions.risk_prompt {
        Some(ButtonPrompt { kind: PromptKind::RiskDodge, target: pawn.actions.risk_chosen.map_or(TargetId::NONE, |c| c.0) })
    } else {
        crate::states::athletic_flow::view(state, pawn).filter(|a| a.flags & 0x01 != 0).map(|a| ButtonPrompt { kind: PromptKind::AirstepAttack, target: a.target })
    };
    let aim = &pawn.magic.aim;
    let point = |id: u32| pawn.magic.airstep_points.iter().find(|p| p.id == id).map_or(glam::Vec3::ZERO, |p| p.position);
    snapshot.aim = AimHud {
        mode: aim.mode,
        aim_time: aim.aim_time,
        total: aim.total,
        max: aim.max_locks,
        locks: aim.locked.iter().map(|e| AimMark { target: e.target, point: 0, count: e.lock_count, position: glam::Vec3::ZERO }).collect(),
        airstep: aim.airstep_targets.iter().map(|e| AimMark { target: e.target, point: e.point, count: 1, position: point(e.point) }).collect(),
    };
}

/// The HUD's own state between steps: the red lag gauges and the popups on screen. Part of
/// `Sora`'s plain data (deterministic: it advances by the step's dt, not by a wall clock as the
/// game's Flash timers do).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct HudState {
    pub hp_red: RedLag,
    pub focus_red: RedLag,
    pub board: PopupBoard,
}

impl HudState {
    /// Advances the model by one step and takes the step's raised popups onto the board.
    pub fn step(&mut self, dt: f32, status: &Status, raised: &[UiPopup]) {
        let g = Gauges::of(status, 0.0, 0.0);
        if status.placeholder {
            (self.hp_red, self.focus_red) = (RedLag::default(), RedLag::default());
        } else {
            self.hp_red.update(dt, g.hp_frame, g.hp_base_frame);
            self.focus_red.update(dt, g.focus_frame, gauges::FOCUS_FRAMES + 1);
        }
        self.board.step(dt);
        for popup in raised {
            self.board.raise(popup);
        }
    }

    /// The snapshot of a step.
    pub fn snapshot(&self, step: u64, status: &Status, raised: Vec<UiPopup>) -> UiSnapshot {
        UiSnapshot { step, gauges: Gauges::of(status, self.hp_red.frame(), self.focus_red.frame()), command_menu: CommandMenu::default(), lock_on: TargetId::NONE, command: Default::default(), popups: raised, board: self.board.clone(), situation: SituationHud::default(), soft_target: TargetId::NONE, prompt: None, aim: AimHud::default(), sneeze: false }
    }
}
