//! The UI seam: once per step, at its end, the core publishes one plain-data
//! [`UiSnapshot`](crate::ui_model::UiSnapshot) (gauges, command menu, lock-on, the step's popups).
//! The host draws it however it likes; nothing is returned. Menu INPUT goes the other way through
//! `Input::pad` like every other button.

use crate::ui_model::UiSnapshot;

pub trait UiSink {
    /// The state of the HUD after this step. Called exactly once per `Sora::step_hosted`.
    fn publish(&mut self, snapshot: &UiSnapshot);
}

/// No UI.
#[derive(Clone, Copy, Debug, Default)]
pub struct NullUi;

impl UiSink for NullUi {
    fn publish(&mut self, _snapshot: &UiSnapshot) {}
}
