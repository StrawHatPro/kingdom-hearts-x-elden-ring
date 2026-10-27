//! (ALLYJOIN) A HOST'S YES / NO QUESTION IN KH3'S OWN DIALOG WINDOW, standing alone in the world (no menu under it).
//!
//! The user (2026-10-27): "when gaining a new ally, have a popup ask if the player wants them to join the party? If yes,
//! open up the party creation menu usually accessed from save points. If no, they are not added but can still be added at
//! a save point like normal." The window is the one of the save-point menu's Title Screen question and of the Save / Load
//! screens' slot questions (savepoint.rs `show_question`, saveload.rs `ask`): KH3's `Dialog` movie, its text field, its two
//! `DialogButton`s placed at the window's position holders, the buttons' words KH3's own (`kh3_system.general.choice_0001`
//! "Yes" / `choice_0002` "No"), the menus' sounds (cursor static SE 0, decide 1, close 3).
//!
//! WHAT IS THE GAME'S [A]: the movie, its labels (`dialog` in / out / off, the buttons' normal / select), the field, the
//! words Yes / No, the sounds. WHAT IS COMPOSED [L]: the text (the host's), where the cursor starts (the host's), B = No
//! (KH3's questions take B as the No of the window: the save-point Title Screen question, the slot questions).
//!
//! KEYS (the host turns the pad into [`CampKey`]s as for every menu): up / down / left / right move between Yes and No (one
//! press, one move, no wrap needed for two), A decides, B answers No. The answer comes once from [`Question::key`].

use std::collections::HashMap;
use std::sync::Arc;

use crate::camp::CampKey;
use crate::menu::{SE_MENU_CLOSE, SE_MENU_CURSOR, SE_MENU_DECIDE};
use crate::{Fonts, Movie, MovieDef, Pack, PackError, UiSound};

const BUTTONS: [&str; 2] = ["dialogButton", "dialogButton2"];
/// Stage pixels between the two buttons (as the save-point menu's and the camp menu's two-button questions).
const BUTTON_PITCH: f32 = 35.0;
const HOLDER: &str = "dialog.dialogAnm.dialogPoAll.dialogPoAll2";

/// (ALLYJOIN) What a key did to the question (the pure half: unit-tested without a pack).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Nothing (a key the window does not take).
    None,
    /// The cursor moved to this button (0 = Yes, 1 = No).
    Moved(usize),
    /// Answered: true = Yes.
    Answered(bool),
}

/// (ALLYJOIN) THE RULE OF THE WINDOW'S KEYS: any direction flips between Yes and No; A takes the button under the
/// cursor; B (and Start) answer No.
pub fn step(cursor: usize, key: CampKey) -> Step {
    match key {
        CampKey::Up | CampKey::Down | CampKey::Left | CampKey::Right => Step::Moved(cursor.min(1) ^ 1),
        CampKey::Decide => Step::Answered(cursor == 0),
        CampKey::Cancel | CampKey::Menu => Step::Answered(false),
        _ => Step::None,
    }
}

/// (ALLYJOIN) The Dialog window as a Yes / No question of the host's.
pub struct Question {
    dialog: Movie,
    fonts: Fonts,
    yes_no: [String; 2],
    /// Up: the cursor (0 = Yes, 1 = No).
    cursor: Option<usize>,
    closing: bool,
}

impl Question {
    pub fn load(pack: &mut Pack, fonts: Fonts) -> Result<Question, PackError> {
        let def: Arc<MovieDef> = pack.movie_linked("Dialog", &crate::camp_menu::LIBRARIES)?;
        let system: HashMap<String, String> = pack.string_table("kh3_system");
        let word = |k: &str, or: &str| system.get(k).cloned().filter(|s| !s.is_empty()).unwrap_or_else(|| or.to_string());
        let yes_no = [word("kh3_system.general.choice_0001", "Yes"), word("kh3_system.general.choice_0002", "No")];
        let mut dialog = Movie::new(def);
        dialog.goto_and_stop("dialog", "off");
        Ok(Question { dialog, fonts, yes_no, cursor: None, closing: false })
    }

    /// The question is up (it takes the keys).
    pub fn is_open(&self) -> bool {
        self.cursor.is_some()
    }

    /// The movie to draw: while up and while its window leaves.
    pub fn movie(&self) -> Option<&Movie> {
        (self.cursor.is_some() || self.closing).then_some(&self.dialog)
    }

    /// Opens the window with `text` (`<br>` or `\n` break lines), the cursor on `cursor` (0 = Yes, 1 = No).
    pub fn ask(&mut self, text: &str, cursor: usize) {
        let fonts = self.fonts.clone();
        if self.cursor.is_none() {
            self.dialog.goto_and_play("dialog", "in");
            self.dialog.add_instance("", "DialogButton", BUTTONS[1]);
        }
        self.closing = false;
        self.dialog.set_visible("dialogText", true);
        self.dialog.set_text("dialogText.textField", &text.replace("<br>", "\n"), &fonts);
        for (n, name) in BUTTONS.into_iter().enumerate() {
            self.dialog.set_visible(name, true);
            self.dialog.set_text(&format!("{name}.dialogBtnText.textField"), &self.yes_no[n], &fonts);
        }
        self.show_cursor(cursor.min(1));
        self.place();
    }

    fn show_cursor(&mut self, cursor: usize) {
        self.cursor = Some(cursor);
        for (n, name) in BUTTONS.into_iter().enumerate() {
            self.dialog.goto_and_play(name, if cursor == n { "select" } else { "normal" });
        }
    }

    /// The text and the buttons at the window's position holders (as savepoint.rs / camp_menu.rs place theirs).
    fn place(&mut self) {
        let stage = |movie: &Movie, path: &str| -> Option<(f32, f32)> {
            let mut m = crate::Matrix::IDENTITY;
            let mut at = String::new();
            for part in path.split('.') {
                at = if at.is_empty() { part.to_string() } else { format!("{at}.{part}") };
                m = m.mul(&movie.node(&at)?.matrix);
            }
            Some((m.tx, m.ty))
        };
        if let Some((x, y)) = stage(&self.dialog, &format!("{HOLDER}.poDialogText")) {
            self.dialog.set_position("dialogText", x, y, 0.0);
        }
        if let Some((x, y)) = stage(&self.dialog, &format!("{HOLDER}.poDialogButton")) {
            for (n, name) in BUTTONS.into_iter().enumerate() {
                self.dialog.set_position(name, x, y + BUTTON_PITCH * n as f32, 0.0);
            }
        }
    }

    fn hide(&mut self) {
        if self.cursor.take().is_some() {
            self.dialog.goto_and_play("dialog", "out");
            self.dialog.set_visible("dialogText", false);
            for name in BUTTONS {
                self.dialog.set_visible(name, false);
            }
            self.closing = true;
        }
    }

    /// A menu key while up. The answer (true = Yes), once; the window then leaves.
    pub fn key(&mut self, key: CampKey, sounds: &mut Vec<UiSound>) -> Option<bool> {
        let cursor = self.cursor?;
        match step(cursor, key) {
            Step::None => None,
            Step::Moved(next) => {
                sounds.push(UiSound::Play(SE_MENU_CURSOR));
                self.show_cursor(next);
                None
            }
            Step::Answered(yes) => {
                sounds.push(UiSound::Play(if yes { SE_MENU_DECIDE } else { SE_MENU_CLOSE }));
                self.hide();
                Some(yes)
            }
        }
    }

    /// One stage frame (while up or leaving).
    pub fn tick(&mut self) {
        if self.cursor.is_none() && !self.closing {
            return;
        }
        self.dialog.tick();
        self.dialog.take_events();
        if self.cursor.is_some() {
            self.place();
        } else if self.dialog.current_label("dialog") == Some("off") {
            self.closing = false;
        }
    }

    /// Takes the window off at once (a load, the host going away).
    pub fn force_close(&mut self) {
        self.cursor = None;
        self.closing = false;
        self.dialog.goto_and_stop("dialog", "off");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (ALLYJOIN) The pad on the question: a direction flips Yes / No, A takes the cursor's button, B is No.
    #[test]
    fn allyjoin_the_question_keys() {
        assert_eq!(step(0, CampKey::Down), Step::Moved(1));
        assert_eq!(step(1, CampKey::Up), Step::Moved(0));
        assert_eq!(step(0, CampKey::Right), Step::Moved(1));
        assert_eq!(step(1, CampKey::Left), Step::Moved(0));
        assert_eq!(step(0, CampKey::Decide), Step::Answered(true));
        assert_eq!(step(1, CampKey::Decide), Step::Answered(false));
        assert_eq!(step(0, CampKey::Cancel), Step::Answered(false), "B answers No even on Yes");
        assert_eq!(step(0, CampKey::Menu), Step::Answered(false));
        assert_eq!(step(0, CampKey::L1), Step::None);
    }
}
