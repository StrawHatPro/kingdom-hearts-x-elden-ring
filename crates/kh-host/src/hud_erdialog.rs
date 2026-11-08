//! (ER-POPUPS) ELDEN RING'S OWN POP-UP BOXES AS KH3'S DIALOG WINDOW (kh_ui::question: the window of the save-point menu's
//! questions and of the ally question, hud_ally.rs).
//!
//! The user (2026-11-07): "evergaol pop up boxes, as well as anything else still using Elden Ring's pop up boxes needs a
//! Kingdom Hearts equivalent, same as what we've already replaced".
//!
//! - WHAT FIRES IT: [`Hud::ask_er_dialog`] with the host's id, the game's own text (its language, after the host's text
//!   swaps) and the box's TYPE ([`ErButtons`]): a question (Yes / No, the cursor on Yes as Elden Ring's) or a message (one
//!   button, KH3's "OK"). Nothing here knows which box of the game it is: kh-er erdialog.rs decides per box TYPE.
//! - WHEN IT SHOWS: as the ally question, when the world is the player's, after every OBTAINED popup that is up or waits
//!   and after an ally question. Several in a row: one after the other, in the order asked.
//! - WHILE IT IS UP: [`Hud::paused`] is true (the world is held as for every KH3 window: no menu, no attack, Start
//!   opens nothing). The pad: D-pad / stick between Yes and No, A decides, B = No (for a message A = OK, B closes it):
//!   the pad as the input layer remapped it (CAMP-OPTIONS); the keyboard's `menu up / down / decide / cancel` the same.
//! - THE ANSWER, once per id: [`Hud::take_er_dialog_answer`]: true = A on Yes / OK (Elden Ring's "left" answer), false =
//!   No or B (its "right" answer, what B gives in its own box too).
//! - [`Hud::cancel_er_dialog`]: the host's event went away (its box is no longer wanted): the window leaves, no answer.
//! - It is drawn on the "Dialog" layer (after the camp menu's dialog and the ally question, which are never up with it).

use std::collections::VecDeque;

use super::{CampKey, Frame, Hud};

/// (ER-POPUPS) The buttons of a box: what TYPE of Elden Ring box it stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErButtons {
    /// A question: Yes / No (the cursor starts on Yes).
    YesNo,
    /// A message: one button, KH3's "OK".
    Ok,
}

/// (ER-POPUPS) The boxes' state in the HUD (a field of [`Hud`]).
#[derive(Default)]
pub struct ErAsk {
    pub(super) window: Option<kh_ui::question::Question>,
    /// Waiting for their turn: (the host's id, the buttons, the text).
    pub(super) waiting: VecDeque<(u64, ErButtons, String)>,
    /// Up now: the host's id.
    pub(super) asking: Option<u64>,
    /// Answers not taken yet: (the host's id, true = A on Yes / OK).
    pub(super) answers: Vec<(u64, bool)>,
}

impl ErAsk {
    pub(super) fn load(pack: &mut kh_ui::Pack, fonts: &kh_ui::Fonts) -> ErAsk {
        let window = match kh_ui::question::Question::load(pack, fonts.clone()) {
            Ok(q) => Some(q),
            Err(e) => {
                println!("hud: (ER-POPUPS) Elden Ring's boxes as KH3's Dialog window are NOT available ({e})");
                None
            }
        };
        ErAsk { window, ..ErAsk::default() }
    }
}

/// One line of a box's text for the log.
fn one_line(text: &str) -> String {
    text.replace("<br>", " / ").replace('\n', " / ")
}

impl Hud {
    /// (ER-POPUPS) Elden Ring's boxes can be shown as KH3's window (the pack has the Dialog movie).
    pub fn er_dialog_available(&self) -> bool {
        self.er_dialog.window.is_some()
    }

    /// (ER-POPUPS) SHOWS ONE OF ELDEN RING'S BOXES as KH3's Dialog window (see the module): at the first free frame. An id
    /// already waiting or up is not asked twice. False: no Dialog movie (the caller leaves the box to the game).
    pub fn ask_er_dialog(&mut self, id: u64, text: &str, buttons: ErButtons) -> bool {
        if self.er_dialog.window.is_none() {
            return false;
        }
        if self.er_dialog.asking == Some(id) || self.er_dialog.waiting.iter().any(|w| w.0 == id) {
            return true;
        }
        self.er_dialog.waiting.push_back((id, buttons, text.to_string()));
        (self.log)(&format!("hud: (ER-POPUPS) Elden Ring's box #{id} ({buttons:?}) \"{}\" waits for its turn", one_line(text)));
        true
    }

    /// (ER-POPUPS) The player's answer to box `id`, once: true = A on Yes / OK, false = No or B.
    pub fn take_er_dialog_answer(&mut self, id: u64) -> Option<bool> {
        let at = self.er_dialog.answers.iter().position(|a| a.0 == id)?;
        Some(self.er_dialog.answers.remove(at).1)
    }

    /// (ER-POPUPS) The ids up or waiting, the one up first.
    pub fn er_dialog_pending(&self) -> Vec<u64> {
        self.er_dialog.asking.into_iter().chain(self.er_dialog.waiting.iter().map(|w| w.0)).collect()
    }

    /// (ER-POPUPS) A box is up now (it holds the world and the pad).
    pub fn er_dialog_open(&self) -> bool {
        self.er_dialog.asking.is_some()
    }

    /// (ER-POPUPS) The host no longer wants box `id` (its event went away): it leaves the queue / the window leaves, and any
    /// answer not taken is dropped.
    pub fn cancel_er_dialog(&mut self, id: u64) {
        self.er_dialog.waiting.retain(|w| w.0 != id);
        self.er_dialog.answers.retain(|a| a.0 != id);
        if self.er_dialog.asking == Some(id) {
            self.er_dialog.asking = None;
            if let Some(w) = self.er_dialog.window.as_mut() {
                w.force_close();
            }
            (self.log)(&format!("hud: (ER-POPUPS) box #{id} taken off (its event went away)"));
        }
    }

    /// (ER-POPUPS) The world is held for a box.
    pub(super) fn er_dialog_holds(&self) -> bool {
        self.er_dialog.asking.is_some()
    }

    /// (ER-POPUPS) The window's movie (drawn on the "Dialog" layer).
    pub(super) fn er_dialog_movie(&self) -> Option<&kh_ui::Movie> {
        self.er_dialog.window.as_ref().and_then(|w| w.movie())
    }

    /// (ER-POPUPS) One frame of the boxes. True: one is up and has the frame (nothing else of the HUD takes input).
    pub(super) fn er_dialog_frame(&mut self, f: &Frame, nav: &[(u8, bool)], words: &[String]) -> bool {
        use kh_core::PadButton as B;
        if self.er_dialog.window.is_none() {
            return false;
        }
        if self.er_dialog.asking.is_none() {
            // the window leaving
            if let Some(w) = self.er_dialog.window.as_mut() {
                for _ in 0..f.ticks {
                    w.tick();
                }
            }
            // its turn: the world free, no OBTAINED popup up or waiting, no ally question up
            if self.er_dialog.waiting.is_empty() || self.ally_holds() || self.obtained_open() || self.obtained_waiting() > 0 || !self.obtained_free() || self.shop.is_open() {
                return false;
            }
            let Some((id, buttons, text)) = self.er_dialog.waiting.pop_front() else { return false };
            if let Some(w) = self.er_dialog.window.as_mut() {
                match buttons {
                    ErButtons::YesNo => w.ask(&text, 0),
                    ErButtons::Ok => w.tell(&text),
                }
            }
            self.er_dialog.asking = Some(id);
            (self.log)(&format!("hud: (ER-POPUPS) box #{id} is up ({buttons:?}; the world is held): \"{}\"", one_line(&text)));
            return true;
        }
        let mut keys: Vec<CampKey> = nav.iter().map(|(dir, _)| [CampKey::Up, CampKey::Down, CampKey::Left, CampKey::Right][*dir as usize & 3]).collect();
        if f.pressed.has(B::FaceBottom) {
            keys.push(CampKey::Decide);
        }
        if f.pressed.has(B::FaceRight) {
            keys.push(CampKey::Cancel);
        }
        for word in words {
            match word.as_str() {
                "menu up" => keys.push(CampKey::Up),
                "menu down" => keys.push(CampKey::Down),
                "menu left" => keys.push(CampKey::Left),
                "menu right" => keys.push(CampKey::Right),
                "menu decide" => keys.push(CampKey::Decide),
                "menu cancel" => keys.push(CampKey::Cancel),
                _ => {}
            }
        }
        let mut answered = None;
        if let Some(w) = self.er_dialog.window.as_mut() {
            for key in keys {
                if let Some(yes) = w.key(key, &mut self.sounds) {
                    answered = Some(yes);
                    break;
                }
            }
            for _ in 0..f.ticks {
                w.tick();
            }
        }
        if let (Some(yes), Some(id)) = (answered, self.er_dialog.asking) {
            self.er_dialog.asking = None;
            self.er_dialog.answers.push((id, yes));
            (self.log)(&format!("hud: (ER-POPUPS) box #{id} answered: {}", if yes { "A on Yes / OK (Elden Ring's left answer)" } else { "No / B (Elden Ring's right answer)" }));
        }
        true
    }

    /// (ER-POPUPS) A load / the host going away: every box goes at once (no answer is given).
    pub fn er_dialog_reset(&mut self) {
        self.er_dialog.waiting.clear();
        self.er_dialog.asking = None;
        self.er_dialog.answers.clear();
        if let Some(w) = self.er_dialog.window.as_mut() {
            w.force_close();
        }
    }
}
