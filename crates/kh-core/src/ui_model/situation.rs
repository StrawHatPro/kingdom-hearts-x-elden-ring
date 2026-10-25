//! The situation commands, the formchange gauge and the form's time as the HUD shows them
//! (docs/sora-ui-spec.md 5.6 - 5.8, the display contract; the rules are the combat
//! implementer's: `combat/situation.rs`, `combat/style.rs`, `combat/form.rs`; combat spec 10, 11).
//! Plain data, filled once per step into `UiSnapshot::situation`.
//!
//! How a HUD maps it onto the movie (UI spec 5.6): one `addActionSelectButton(position, name,
//! iconType, inEffect, hasTimer)` per `list` entry (`hasTimer` = `total > 0`; icon by `class`:
//! 3 formchange -> `wep`, 1 grand magic -> `magic`), `m_pGFxActionCmdTime` = `remaining` (capped
//! 99.9; `red` below 10.0 s), the gauge frame from `remaining / total`; the event button from
//! `slot_a` (effect 2), else `slot_b` (effect 1; `extra` replaces its text while a finish extra
//! is offered); `setActionCommandMark(selected, ...)` with -1 = the event button; the style-point
//! gauge entry (icon type 100) from `gauge_ratio` with `gauge_phase` (5.7: thresholds 0.25 / 0.5 /
//! 0.75 / 1.0), hidden while the list is full; the form logo's time gauge (5.8) from `form`
//! (`left / total`, `alert` below 10 s).

use crate::enums::ETresCommandKind;

/// One entry of the list or of a single slot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SituationEntry {
    pub kind: ETresCommandKind,
    /// The HUD class (combat spec 11.1): 1 grand magic, 2 reaction, 3 formchange, 4 finish ...
    pub class: u8,
    /// A reaction command of the controller's prompt (an "ex kind" entry).
    pub ex: bool,
    /// Total and remaining time, seconds; `total` -1 = no countdown.
    pub total: f32,
    pub remaining: f32,
    /// Still inside its 0.2 s appear time (not selectable yet).
    pub appearing: bool,
    /// The countdown has started (its 0.5 s start delay is over).
    pub counting: bool,
    pub serial: u32,
}

/// The form on the HUD (the logo plate and its time gauge).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FormHud {
    /// The form's command (`CHANGE_L1` = Second Form).
    pub kind: ETresCommandKind,
    /// controller+0xB58 / +0xB54: the form's full and remaining time.
    pub total: f32,
    pub left: f32,
    /// 0..3 and the finish command on offer (0 = none).
    pub finish_level: i32,
    /// The wear form shown (5 = Second Form's textures).
    pub wear_form: u8,
    /// The world is slowed (the transformation / a finish is playing).
    pub world_slow: bool,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct SituationHud {
    /// The counted list (at most 6), oldest first.
    pub list: Vec<SituationEntry>,
    pub slot_a: Option<SituationEntry>,
    pub slot_b: Option<SituationEntry>,
    /// A finish extra (175 - 178) on show in slot B.
    pub extra: Option<ETresCommandKind>,
    /// The selected list index; -1 = the event button; -2 = nothing.
    pub selected: i32,
    /// The formchange gauge 0..1 and its phase 0..3.
    pub gauge_ratio: f32,
    pub gauge_phase: u8,
    pub form: Option<FormHud>,
}
