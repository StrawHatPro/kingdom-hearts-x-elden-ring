//! (cutscene) THE CUTSCENE PAUSE MENU: what the Menu button shows while an event / cutscene plays: the
//! Pause movie (`13_pause/Pause`, `UTresUIP_Pause`) with its button set of MENU TYPE 1, "Continue" and
//! "Skip Scene", under the "PAUSE" title, over the dimmed scene.
//!
//! EVIDENCE ([A] read from the movie / the strings / the executable's data, [C] decompiled C only,
//! [D] inferred, [L] LOCAL choice; nothing was checked in the running game):
//! - The screen selector `0x1415A2E10` reads the game instance's pause kind (`+0x60` of the object
//!   `0x140EA8620` returns): kinds 1 and 2 call `0x1417103C0`, which is `0x141712640(pause, 1, no hint)` [C].
//! - Menu type 1 of the table at `0x146B37820` (6 ids per type, 30 = no button) is `0, 1, 30, 30, 30, 30`
//!   [A]; the key table at `0x1486F40C0` (filled by `0x1401618A0`) gives id 0 = `kh3_menu.pause.menu_0001`
//!   "Continue", id 1 = `kh3_menu.pause.menu_0003` "Skip Scene" [C for the order, A for the texts].
//! - Open `0x141712640` [C]: static SE 2; root `loop`; title and background `in`; the hint clip off (no
//!   hint texts); the button set `in`; per slot `0x141717190`: visible, text, user id, enabled; the menu
//!   object (`pauseMenu.menuObj`, labels normal / hint / retry) `normal` and MOVED UP by 12 + 25 x buttons
//!   pixels (two buttons: -62, so title and buttons stay centred on the screen); the focus on the first
//!   button, whose cursor plays `in`. The label NAMES behind the code's string globals are [D] (the
//!   clips' own labels).
//! - The clips [A]: `pauseMenu.menuObj.title` (the five letters of "PAUSE", centred at y 576),
//!   `btnSet.btnSetObj.coBtn1..6` (class `CoBtnPause`: normal / disable / select / disableSelect / decide;
//!   `cursor` off / loop / in / decide; text `btnName.coName.textField`, size 23, centred; x 960, y 627 +
//!   50 n), `stageBg`. Anchors: coBtn1 up -> coBtn6 with `repeatStopUp`, coBtn6 down -> coBtn1 with
//!   `repeatStopDown`, invisible buttons are skipped: the cursor wraps on a fresh press and stops at
//!   the ends while a direction is held.
//! - Input slot 89 `0x141708B30` [C]: the keys count only after the open animation reported its end
//!   (`Callback(END_PAUSE_ANIMATION_OPEN)` sets the flag at +0xA2; frame 13 of the button set [A]);
//!   decide on Continue / Skip Scene (ids 0 / 1): title, background and buttons `out`, and at
//!   `Callback(END_PAUSE_ANIMATION_CLOSE)` (`0x141705860`; frame 43 of the button set) the button's
//!   delegate runs: +0x1D0 Continue, +0x210 Skip Scene (only when the byte at +0x6DC is set). THERE IS NO
//!   CONFIRM STEP: no dialog is opened for Skip Scene anywhere in these two functions. The Menu button
//!   again (event 13) = static SE 3 and Continue. Decide on a disabled button = static SE 4.
//!   The cancel key is NOT handled by the part for these buttons (event 8 only for ids 13 - 17).
//! - [D]: cursor move = static SE 0, decide = static SE 1 (the menus' common sounds, docs/sora-ui-spec.md
//!   10.2); the decided button plays its `decide` label while the screen leaves.
//! - The cancel key (B) does nothing, as in the game (the port's owner, 2026-10-12: "match the game with B").
//! - [L]: `skip_enabled = false` shows Skip Scene with the `disable` labels and refuses it (the game's
//!   handling of a scene that cannot be skipped, +0x6DC, was not traced further).

use std::collections::HashMap;
use std::sync::Arc;

use crate::camp::CampKey;
use crate::menu::{SE_MENU_CLOSE, SE_MENU_CURSOR, SE_MENU_DECIDE, SE_MENU_OPEN, SE_MENU_REFUSED};
use crate::{Fonts, Movie, MovieDef, Pack, PackError, UiSound};

/// What the menu told the host, once, when its way out has ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CutscenePauseResult {
    /// "Continue" (or the Menu button): the cutscene goes on.
    Resume,
    /// "Skip Scene": the host ends the cutscene.
    SkipScene,
}

/// The pause menu type of an event (table `0x146B37820`), and its row: ids of the key table `0x1486F40C0`.
pub const MENU_TYPE: usize = 1;
pub const BUTTON_IDS: [i32; 6] = [0, 1, 30, 30, 30, 30];
/// (id, string key) of the two buttons.
pub const BUTTONS: [(i32, &str, CutscenePauseResult); 2] = [(0, "kh3_menu.pause.menu_0001", CutscenePauseResult::Resume), (1, "kh3_menu.pause.menu_0003", CutscenePauseResult::SkipScene)];
/// `0x141712640`: the menu object's y with n buttons: -12 - 25 n (0 without buttons).
pub fn menu_object_y(buttons: usize) -> f32 {
    if buttons == 0 { 0.0 } else { -12.0 - 25.0 * buttons as f32 }
}

const P_OBJ: &str = "pauseMenu.menuObj";
const P_TITLE: &str = "pauseMenu.menuObj.title";
const P_HINT: &str = "pauseMenu.menuObj.hint";
const P_BUTTONS: &str = "pauseMenu.menuObj.btnSet";
const P_BG: &str = "stageBg";

pub struct CutscenePauseMenu {
    pub movie: Movie,
    fonts: Fonts,
    strings: HashMap<String, String>,
    open: bool,
    /// The open animation reported its end: the keys count.
    input_on: bool,
    /// The way out runs; this is told at its end.
    leaving: Option<CutscenePauseResult>,
    cursor: usize,
    skip_enabled: bool,
}

impl CutscenePauseMenu {
    pub fn load(pack: &mut Pack, fonts: Fonts) -> Result<CutscenePauseMenu, PackError> {
        let def: Arc<MovieDef> = pack.movie("Pause")?;
        let strings = pack.string_table("kh3_menu");
        Ok(CutscenePauseMenu { movie: Movie::new(def), fonts, strings, open: false, input_on: false, leaving: None, cursor: 0, skip_enabled: true })
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// 0 = Continue, 1 = Skip Scene.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// The keys count (the open animation has ended and no button was decided).
    pub fn accepts_input(&self) -> bool {
        self.open && self.input_on && self.leaving.is_none()
    }

    /// The text of button `slot` as the game's string table has it.
    pub fn button_text(&self, slot: usize) -> String {
        BUTTONS.get(slot).and_then(|b| self.strings.get(b.1)).cloned().unwrap_or_default()
    }

    fn button_path(slot: usize) -> String {
        format!("{P_BUTTONS}.btnSetObj.coBtn{}", slot + 1)
    }

    /// `0x141712640(pause, 1, no hint)`. `skip_enabled = false`: the scene cannot be skipped (the button
    /// is shown disabled and refuses).
    pub fn open(&mut self, skip_enabled: bool, sounds: &mut Vec<UiSound>) {
        sounds.push(UiSound::Play(SE_MENU_OPEN));
        self.skip_enabled = skip_enabled;
        self.cursor = 0;
        self.leaving = None;
        self.input_on = false;
        self.movie.goto_and_play("", "loop");
        self.movie.set_visible("gameOverMenu", false);
        self.movie.goto_and_play(P_TITLE, "in");
        self.movie.goto_and_play(P_BG, "in");
        self.movie.goto_and_stop(P_HINT, "off");
        self.movie.goto_and_play(P_BUTTONS, "in");
        let fonts = self.fonts.clone();
        let mut shown = 0;
        for (slot, id) in BUTTON_IDS.iter().enumerate() {
            let path = Self::button_path(slot);
            let button = BUTTONS.iter().find(|b| b.0 == *id);
            self.movie.set_visible(&path, button.is_some());
            if button.is_some() {
                let text = self.button_text(slot);
                self.movie.set_text(&format!("{path}.btnName.coName.textField"), &text, &fonts);
                shown += 1;
            }
        }
        self.movie.goto_and_stop(P_OBJ, "normal");
        self.movie.set_position(P_OBJ, 0.0, menu_object_y(shown), 0.0);
        self.open = true;
        self.show_cursor();
        // The focused button's cursor plays `in` (its end goes on into `loop`).
        self.movie.goto_and_play(&format!("{}.cursor", Self::button_path(0)), "in");
    }

    fn enabled(&self, slot: usize) -> bool {
        slot != 1 || self.skip_enabled
    }

    /// Every button's state label by the cursor (the label's own frame script drives the `cursor` clip).
    fn show_cursor(&mut self) {
        for slot in 0..BUTTONS.len() {
            let path = Self::button_path(slot);
            let label = match (slot == self.cursor, self.enabled(slot)) {
                (true, true) => "select",
                (true, false) => "disableSelect",
                (false, true) => "normal",
                (false, false) => "disable",
            };
            if self.movie.current_label(&path) != Some(label) {
                self.movie.goto_and_play(&path, label);
            }
        }
    }

    fn leave(&mut self, result: CutscenePauseResult) {
        self.movie.goto_and_play(P_TITLE, "out");
        self.movie.goto_and_play(P_BG, "out");
        self.movie.goto_and_play(P_BUTTONS, "out");
        self.leaving = Some(result);
    }

    /// A menu key: up / down move the cursor, decide chooses, the Menu button resumes; cancel is ignored.
    pub fn key(&mut self, key: CampKey, repeat: bool, sounds: &mut Vec<UiSound>) {
        if !self.accepts_input() {
            return;
        }
        match key {
            CampKey::Up | CampKey::Down => {
                if let Some(next) = crate::camp::move_wrapping(self.cursor, BUTTONS.len(), key == CampKey::Down, repeat) {
                    self.cursor = next;
                    sounds.push(UiSound::Play(SE_MENU_CURSOR));
                    self.show_cursor();
                }
            }
            CampKey::Decide if !repeat => {
                if !self.enabled(self.cursor) {
                    sounds.push(UiSound::Play(SE_MENU_REFUSED));
                    return;
                }
                sounds.push(UiSound::Play(SE_MENU_DECIDE));
                self.movie.goto_and_play(&Self::button_path(self.cursor), "decide");
                self.leave(BUTTONS[self.cursor].2);
            }
            CampKey::Menu if !repeat => {
                sounds.push(UiSound::Play(SE_MENU_CLOSE));
                self.leave(CutscenePauseResult::Resume);
            }
            _ => {}
        }
    }

    /// One stage frame. The result, once, when the way out reported its end (the menu is then gone).
    pub fn tick(&mut self) -> Option<CutscenePauseResult> {
        if !self.open {
            return None;
        }
        self.movie.tick();
        let mut ended = false;
        for event in self.movie.take_events() {
            match event.name.as_str() {
                "END_PAUSE_ANIMATION_OPEN" => self.input_on = true,
                "END_PAUSE_ANIMATION_CLOSE" if self.leaving.is_some() => ended = true,
                _ => {}
            }
        }
        if !ended {
            return None;
        }
        let result = self.leaving;
        self.close();
        result
    }

    /// Takes the menu off at once, without a result (the cutscene ended under it).
    pub fn close(&mut self) {
        self.open = false;
        self.input_on = false;
        self.leaving = None;
        self.movie.goto_and_play("", "off");
    }
}
