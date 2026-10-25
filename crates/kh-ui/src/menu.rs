//! From the Menu button to a paused game (docs/sora-ui-spec.md 10.1): the request, the pause
//! gate, the screen selector with the camp menu's refuse rules, and the in-battle Pause screen
//! (`13_pause/Pause`, `UTresUIP_Pause`).
//!
//! The camp menu's own logic is `camp.rs`. The host owns the engine pause: while
//! [`MenuFlow::paused`] is true it must not step the simulation (the game's `pc.SetPause(true)`:
//! world time stops, actors stop ticking; the UI keeps ticking on the undilated frame time) and
//! must freeze the cockpit movies (layers 0 / 1: the HUD timelines).

use std::sync::Arc;

use crate::def::MovieDef;
use crate::player::Movie;
use crate::render::Fonts;
use crate::UiSound;

/// Menu sounds (static SE; spec 10.2): cursor 0, decide 1, open a screen / tab 2, cancel /
/// close 3, refused 4.
pub const SE_MENU_CURSOR: u32 = 0;
pub const SE_MENU_DECIDE: u32 = 1;
pub const SE_MENU_OPEN: u32 = 2;
pub const SE_MENU_CLOSE: u32 = 3;
pub const SE_MENU_REFUSED: u32 = 4;

/// What the gate reads from the game (10.1 steps 3 - 5). State ids are `ETresStateID` values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GateInput {
    /// `TresSetDisableGamePause` (GI+0x338 bit 0): nothing happens.
    pub pause_disabled: bool,
    /// The unlock flag `campmenu`.
    pub camp_unlocked: bool,
    /// `TresSetDisableCampMenu` (GI+0x8D4 bit 1).
    pub camp_disabled: bool,
    /// `(GI+0x8D4 & 0x14) != 0`: enemies engaged (kept 1.0 s after the last one) or the
    /// special battle mode.
    pub in_battle: bool,
    /// An event is running (GI+0x880 bit 0).
    pub in_event: bool,
    /// The current and the pending player state.
    pub state: i32,
    pub pending_state: i32,
    /// The jump state's field +0x78 is 7 or 8 (which phases those are is not settled).
    pub jump_phase_blocked: bool,
    /// The state is an Attack state whose pawn test (vt+0x2748) is true, or the attack kind is
    /// 40 - 43, 53 - 57 or 63 - 67.
    pub attack_blocked: bool,
    /// The controller is aiming a shotlock, its style command is 0x8B, or the command lock is set.
    pub command_blocked: bool,
}

impl Default for GateInput {
    fn default() -> GateInput {
        GateInput { pause_disabled: false, camp_unlocked: true, camp_disabled: false, in_battle: false, in_event: false, state: 1, pending_state: 0, jump_phase_blocked: false, attack_blocked: false, command_blocked: false }
    }
}

const STATE_JUMP: i32 = 3;
const STATE_DAMAGE: i32 = 19;
const STATE_WEAPONCHANGE: i32 = 32;
const STATE_DIE: i32 = 59;
const STATE_GAMEOVER: i32 = 118;
const STATE_ONE_ACTION: i32 = 150;

/// The current states in which the camp menu is refused (slot 29 `0x14142BA80`, second switch
/// table) and the states of the interface test vt+0x618.
fn state_refuses_camp(state: i32) -> bool {
    matches!(state,
        5 | 15 | 26..=28 | 30 | 73 | 92 | 95..=97 | 101 | 103 | 110 | 112 | 114 | 115 | 117 | 119..=121 | 126..=137 | 163..=167
        | 31 | 32 | 33..=39 | 40 | 41 | 42 | 74 | 104 | 108 | 142 | 144 | 145
        // vt+0x618: SUPER_SLIDE, RAIL_SLIDE, WALL_KICK, POLE_TURN, ENEMY_FLOW, RAILSLIDE_MI
        | 21..=25 | 138)
}

/// `MayPause` `0x140EA0680` as far as the port has the inputs: no pause while the pause is
/// disabled by script, or while the state or the pending state is DIE or GAMEOVER.
pub fn may_pause(g: &GateInput) -> bool {
    !g.pause_disabled && !matches!(g.state, STATE_DIE | STATE_GAMEOVER) && !matches!(g.pending_state, STATE_DIE | STATE_GAMEOVER)
}

/// The pawn's request interface slot 29, "may open the camp menu" (10.1 step 5).
pub fn may_open_camp(g: &GateInput) -> bool {
    if g.in_battle || g.in_event {
        return false;
    }
    if matches!(g.pending_state, STATE_DAMAGE | STATE_WEAPONCHANGE | STATE_DIE) {
        return false;
    }
    if state_refuses_camp(g.state) || (g.state == STATE_JUMP && g.jump_phase_blocked) || g.state == STATE_ONE_ACTION {
        return false;
    }
    !(g.attack_blocked || g.command_blocked)
}

/// Which screen a pause shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Camp,
    /// The Pause movie with a button set (0 = the plain "PAUSE" of a battle).
    Pause(u8),
}

/// The screen selector `0x1415A2E10` for normal play: the camp menu when the unlock flag is
/// set, the script has not disabled it and slot 29 agrees; else the Pause screen.
pub fn select_screen(g: &GateInput) -> Screen {
    if g.camp_unlocked && !g.camp_disabled && may_open_camp(g) {
        Screen::Camp
    } else {
        Screen::Pause(0)
    }
}

/// The request and the pause itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Playing,
    /// The Menu key was pressed: the pause begins with the next frame.
    Requested,
    /// Paused with a screen up.
    Open(ScreenKind),
    /// The screen plays its way out; the game resumes when it reports the end.
    Closing(ScreenKind),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenKind {
    Camp,
    Pause,
}

/// What the flow asks of the host in a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowEvent {
    /// `SetPause(true)`: the engine pause begins; open this screen.
    Opened(Screen),
    /// `SetPause(false)`: the game resumes.
    Resumed,
}

/// The Menu button's flow (10.1 steps 1 - 3, 7).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct MenuFlow {
    pub phase: Phase,
}

impl MenuFlow {
    /// The host must not step the simulation while this is true.
    pub fn paused(&self) -> bool {
        matches!(self.phase, Phase::Open(_) | Phase::Closing(_))
    }

    /// One frame while playing. `menu_pressed` = UI pad key 22 just pressed.
    pub fn update(&mut self, menu_pressed: bool, gate: &GateInput) -> Option<FlowEvent> {
        match self.phase {
            Phase::Playing => {
                if menu_pressed {
                    self.phase = Phase::Requested;
                }
                None
            }
            // One frame after the press, at the start of the frame.
            Phase::Requested => {
                if !may_pause(gate) {
                    self.phase = Phase::Playing;
                    return None;
                }
                let screen = select_screen(gate);
                self.phase = Phase::Open(if screen == Screen::Camp { ScreenKind::Camp } else { ScreenKind::Pause });
                Some(FlowEvent::Opened(screen))
            }
            _ => None,
        }
    }

    /// The open screen starts to close.
    pub fn begin_close(&mut self) {
        if let Phase::Open(kind) = self.phase {
            self.phase = Phase::Closing(kind);
        }
    }

    /// The closing screen reported its end: unpause.
    pub fn closed(&mut self) -> Option<FlowEvent> {
        if matches!(self.phase, Phase::Closing(_) | Phase::Open(_)) {
            self.phase = Phase::Playing;
            return Some(FlowEvent::Resumed);
        }
        None
    }
}

/// The Pause screen (`UTresUIP_Pause`, `0x141712640`; input slot 89 `0x141708B30`). Only type
/// 0, the plain "PAUSE" without buttons, is driven: the other button sets (Continue / Skip
/// Scene / Retry / Exit) belong to scenes and game over, which the port does not have.
pub struct PauseScreen {
    pub movie: Movie,
    pub fonts: Fonts,
    open: bool,
    closing: bool,
}

const P_TITLE: &str = "pauseMenu.menuObj.title";
const P_BUTTONS: &str = "pauseMenu.menuObj.btnSet";
const P_BG: &str = "stageBg";

impl PauseScreen {
    pub fn new(def: Arc<MovieDef>, fonts: Fonts) -> PauseScreen {
        PauseScreen { movie: Movie::new(def), fonts, open: false, closing: false }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// `0x141712640(pause, 0)`: static SE 2, root `loop`, title and background `in`; no button.
    pub fn open(&mut self, sounds: &mut Vec<UiSound>) {
        sounds.push(UiSound::Play(SE_MENU_OPEN));
        self.movie.goto_and_play("", "loop");
        self.movie.goto_and_stop("pauseMenu.menuObj", "normal");
        for n in 1..=6 {
            self.movie.set_visible(&format!("{P_BUTTONS}.btnSetObj.coBtn{n}"), false);
        }
        self.movie.goto_and_play(P_TITLE, "in");
        self.movie.goto_and_play(P_BG, "in");
        self.movie.goto_and_play(P_BUTTONS, "in");
        self.open = true;
        self.closing = false;
    }

    /// The Menu key again (event 13): static SE 3; title, background and buttons `out`.
    pub fn close(&mut self, sounds: &mut Vec<UiSound>) {
        if !self.open || self.closing {
            return;
        }
        sounds.push(UiSound::Play(SE_MENU_CLOSE));
        self.movie.goto_and_play(P_TITLE, "out");
        self.movie.goto_and_play(P_BG, "out");
        self.movie.goto_and_play(P_BUTTONS, "out");
        self.closing = true;
    }

    /// One frame. True when the close animation reported its end
    /// (`Callback(END_PAUSE_ANIMATION_CLOSE)`): the Continue delegate unpauses.
    pub fn tick(&mut self) -> bool {
        self.movie.tick();
        let mut ended = false;
        for event in self.movie.take_events() {
            if event.name == "END_PAUSE_ANIMATION_CLOSE" && self.closing {
                ended = true;
            }
        }
        if ended {
            self.open = false;
            self.closing = false;
            self.movie.goto_and_play("", "off");
        }
        ended
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_11b_test_10_open_and_refuse() {
        // Idle on the ground, no enemy engaged: the camp menu, one frame after the press.
        let idle = GateInput::default();
        let mut flow = MenuFlow::default();
        assert_eq!(flow.update(true, &idle), None);
        assert!(!flow.paused(), "the pause begins with the next frame");
        assert_eq!(flow.update(false, &idle), Some(FlowEvent::Opened(Screen::Camp)));
        assert!(flow.paused());
        flow.begin_close();
        assert!(flow.paused());
        assert_eq!(flow.closed(), Some(FlowEvent::Resumed));
        assert!(!flow.paused());
        // An enemy engaged (or within 1.0 s after the last one died): the Pause movie.
        assert_eq!(select_screen(&GateInput { in_battle: true, ..idle }), Screen::Pause(0));
        // Guarding (15), aiming a shotlock, talking (104): Pause, even with no enemy.
        assert_eq!(select_screen(&GateInput { state: 15, ..idle }), Screen::Pause(0));
        assert_eq!(select_screen(&GateInput { command_blocked: true, ..idle }), Screen::Pause(0));
        assert_eq!(select_screen(&GateInput { state: 104, ..idle }), Screen::Pause(0));
        // An ordinary jump: the camp menu; run and idle too.
        for state in [1, 2, 3] {
            assert_eq!(select_screen(&GateInput { state, ..idle }), Screen::Camp);
        }
        assert_eq!(select_screen(&GateInput { state: 3, jump_phase_blocked: true, ..idle }), Screen::Pause(0));
        // A pending damage state, the flag missing, the script's switch.
        assert_eq!(select_screen(&GateInput { pending_state: 19, ..idle }), Screen::Pause(0));
        assert_eq!(select_screen(&GateInput { camp_unlocked: false, ..idle }), Screen::Pause(0));
        assert_eq!(select_screen(&GateInput { camp_disabled: true, ..idle }), Screen::Pause(0));
        // TresSetDisableGamePause(true): nothing happens. Dying: no pause.
        let mut flow = MenuFlow::default();
        let disabled = GateInput { pause_disabled: true, ..idle };
        flow.update(true, &disabled);
        assert_eq!(flow.update(false, &disabled), None);
        assert!(!flow.paused() && flow.phase == Phase::Playing);
        assert!(!may_pause(&GateInput { state: 59, ..idle }) && !may_pause(&GateInput { pending_state: 118, ..idle }));
    }
}
