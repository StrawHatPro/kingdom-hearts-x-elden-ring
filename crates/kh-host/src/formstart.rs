//! (FORMWEP-FIX) THE KEYBLADE UNTIL THE FORM CHANGE BEGINS: the draw side's gate for the frames between a form's start and its
//! transformation.
//!
//! kh-core's StartStyle (combat/form.rs `start_style`) sets the form (`Form::cmd`, `Form::begun` + 1) and queues StyleChange;
//! the state begins at the next MainProc, and only its Begin sets `Form::equip` to `Keyblade` (KH3: StartStyle put the form's
//! Keyblade in his hand, nothing of the form is equipped until StyleChange's Tick). In between, one or two steps, `equip` still
//! says `Weapons` (the last form's End, or the default), so the hosts drew the NEW form's weapon in his hand for a frame or two
//! before the cinematic (a Hyper Hammer pop ahead of the Keyblade's transformation). kh-core is left as it is (its golden
//! traces); the hosts ask this gate instead and draw the hand Keyblade while it is on:
//! - on from the step where `Form::begun` changes (a form started: first stage, second stage, Rage Form, Second Form ...),
//! - off as soon as the transformation state runs (`StateId::Stylechange`: from then on `equip` is right), the form is gone,
//!   or after [`WINDOW_STEPS`] steps (a StyleChange that never began, a damage reaction that replaced it: the form's own
//!   rule then draws as before, never the wrong weapon for long).
//!
//! One gate per drawn Sora (the player in kh-er sora.rs and its keychain step, each Mimic copy, the effects harness).

use kh_core::combat::form::Form;
use kh_core::StateId;

/// The longest the gate stays on without the transformation state (fixed steps: 0.1 s at 60 Hz). StyleChange begins at the
/// next MainProc, one step after StartStyle.
pub const WINDOW_STEPS: u32 = 6;

/// See the module notes.
#[derive(Clone, Copy, Debug, Default)]
pub struct FormStartGate {
    /// the `Form::begun` seen last (None: none seen yet, a load or a new character: no start to hold)
    begun: Option<u32>,
    /// steps on since the start
    steps: u32,
    on: bool,
}

impl FormStartGate {
    /// ONCE A FIXED STEP, after the core's step: whether the draw keeps the hand Keyblade (not the form's units) this step.
    pub fn step(&mut self, form: &Form, state: StateId) -> bool {
        match self.begun {
            Some(b) if b != form.begun => {
                self.on = true;
                self.steps = 0;
            }
            _ => {}
        }
        self.begun = Some(form.begun);
        if self.on {
            self.steps += 1;
            if state == StateId::Stylechange || form.cmd == 0 || self.steps > WINDOW_STEPS {
                self.on = false;
            }
        }
        self.on
    }

    /// Whether the gate was on at the latest step.
    pub fn on(&self) -> bool {
        self.on
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A held form (`cmd`, 0 = none) after `begun` starts.
    fn form(cmd: i32, begun: u32) -> Form {
        let mut f = Form::default();
        held(&mut f, cmd, begun);
        f
    }

    fn held(f: &mut Form, cmd: i32, begun: u32) {
        f.cmd = cmd;
        f.begun = begun;
    }

    #[test]
    fn the_keyblade_holds_from_the_form_start_to_the_transformation_and_no_longer() {
        let mut g = FormStartGate::default();
        // a running game: no form, then StartStyle (begun 1) while the state is still Idle
        assert!(!g.step(&form(0, 0), StateId::Idle));
        assert!(g.step(&form(0x81, 1), StateId::Idle), "the step StartStyle ran: the Keyblade, not the hammer");
        assert!(!g.step(&form(0x81, 1), StateId::Stylechange), "the transformation runs: its own units from now on");
        assert!(!g.step(&form(0x81, 1), StateId::Idle), "after it, the form's weapons");
        // the second stage starts the same way
        assert!(g.step(&form(0x84, 2), StateId::Attack));
        assert!(!g.step(&form(0x84, 2), StateId::Stylechange));
    }

    #[test]
    fn an_interrupted_transformation_lets_go_within_the_window() {
        let mut g = FormStartGate::default();
        g.step(&form(0, 3), StateId::Idle);
        let on: Vec<bool> = (0..WINDOW_STEPS + 3).map(|_| g.step(&form(0x81, 4), StateId::Damage)).collect();
        assert!(on[..WINDOW_STEPS as usize].iter().all(|o| *o), "held for the window");
        assert!(on[WINDOW_STEPS as usize..].iter().all(|o| !*o), "then the form's own rule: {on:?}");
        // a form that ends during the window lets go at once
        let mut g = FormStartGate::default();
        g.step(&form(0, 0), StateId::Idle);
        assert!(g.step(&form(0x81, 1), StateId::Idle));
        assert!(!g.step(&form(0, 1), StateId::Idle));
    }

    #[test]
    fn a_load_holds_nothing() {
        let mut g = FormStartGate::default();
        assert!(!g.step(&form(0x81, 7), StateId::Idle), "the first step seen: a form already held");
        assert!(!g.step(&form(0x81, 7), StateId::Idle));
    }
}
