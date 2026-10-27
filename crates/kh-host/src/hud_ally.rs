//! (ALLYJOIN) A NEW ALLY'S QUESTION: "join the party?" (kh_ui::question: KH3's own Dialog window, Yes / No).
//!
//! The user (2026-10-27): "when gaining a new ally, have a popup ask if the player wants them to join the party? If yes,
//! open up the party creation menu usually accessed from save points. If no, they are not added but can still be added at
//! a save point like normal."
//!
//! - WHAT FIRES IT: [`Hud::ask_ally_join`] with the character and the host's text (any [`PartyChara`]: nothing here knows
//!   who the allies are). False: the pack has no Dialog movie (the host keeps its old behaviour: he joins).
//! - WHEN IT SHOWS: as the OBTAINED popup, when the world is the player's, and AFTER every OBTAINED popup that is up or
//!   waits (the new member's own "obtained" comes first, then the question). Several in a row: one after the other.
//! - WHILE IT IS UP: [`Hud::paused`] is true (the host holds its world as for the menus), the cockpit stands still, Start
//!   opens nothing. The pad: D-pad / stick between Yes and No, A decides, B = No (kh_ui::question `step`). The keyboard's
//!   words `menu up / down / decide / cancel` the same.
//! - THE ANSWER, once: [`Hud::take_ally_answer`] (the host adds or leaves him out). YES ALSO OPENS THE PARTY FORMATION
//!   SCREEN (the save point's own: [`Hud::open_party_formation`] with the HUD's formation, the new ally unlocked on it); the
//!   world stays held from the answer until that screen is up (at most [`HOLD_FRAMES`] frames), and when it closes the
//!   host is NOT told "back to the save-point menu" (`take_formation_closed` stays false: no save-point menu was open).
//! - It is drawn on the "Dialog" layer (the camp menu's dialog window: the camp menu is never up with it): no new layer.

use std::collections::VecDeque;

use super::{CampKey, Frame, Hud};
use crate::party::PartyChara;

/// Frames the world stays held between a Yes and the Party Formation screen being up.
pub const HOLD_FRAMES: u32 = 30;

/// (ALLYJOIN) The question's state in the HUD (a field of [`Hud`]).
#[derive(Default)]
pub struct AllyAsk {
    pub(super) window: Option<kh_ui::question::Question>,
    /// Waiting for their turn: (who, the text).
    pub(super) waiting: VecDeque<(PartyChara, String)>,
    /// Up now: who.
    pub(super) asking: Option<PartyChara>,
    /// The answer not taken yet.
    pub(super) answer: Option<(PartyChara, bool)>,
    /// A Yes holds the world until the Party Formation screen is up (frames left).
    pub(super) hold: u32,
    /// The Party Formation screen that is (or is about to be) up was opened by a Yes here.
    pub(super) formation: bool,
}

impl AllyAsk {
    pub(super) fn load(pack: &mut kh_ui::Pack, fonts: &kh_ui::Fonts) -> AllyAsk {
        let window = match kh_ui::question::Question::load(pack, fonts.clone()) {
            Ok(q) => Some(q),
            Err(e) => {
                println!("hud: (ALLYJOIN) the ally question is NOT available ({e})");
                None
            }
        };
        AllyAsk { window, ..AllyAsk::default() }
    }
}

impl Hud {
    /// (ALLYJOIN) The question can be asked (the pack has the Dialog movie).
    pub fn ally_ask_available(&self) -> bool {
        self.ally.window.is_some()
    }

    /// (ALLYJOIN) ASKS WHETHER `chara` JOINS THE PARTY (see the module): shown at the first free frame after the OBTAINED
    /// popups. False: no Dialog movie (the caller keeps its old behaviour).
    pub fn ask_ally_join(&mut self, chara: PartyChara, text: &str) -> bool {
        if self.ally.window.is_none() {
            return false;
        }
        self.ally.waiting.retain(|(c, _)| *c != chara);
        self.ally.waiting.push_back((chara, text.to_string()));
        (self.log)(&format!("hud: (ALLYJOIN) the question \"{}\" for {} waits for its turn", text.replace("<br>", " / ").replace('\n', " / "), chara.name()));
        true
    }

    /// (ALLYJOIN) The question is up (or waits for its turn).
    pub fn ally_asking(&self) -> bool {
        self.ally.asking.is_some()
    }

    /// (ALLYJOIN) Who waits to be asked or is asked now.
    pub fn ally_pending(&self) -> Vec<PartyChara> {
        self.ally.asking.into_iter().chain(self.ally.waiting.iter().map(|w| w.0)).collect()
    }

    /// (ALLYJOIN) The player's answer, once: (who, true = Yes: he joins and the Party Formation screen opens).
    pub fn take_ally_answer(&mut self) -> Option<(PartyChara, bool)> {
        self.ally.answer.take()
    }

    /// (ALLYJOIN) The world is held for the question (up, or a Yes waiting for the Party Formation screen).
    pub(super) fn ally_holds(&self) -> bool {
        self.ally.asking.is_some() || self.ally.hold > 0
    }

    /// (ALLYJOIN) The window's movie (drawn on the "Dialog" layer when the camp menu has no dialog up).
    pub(super) fn ally_movie(&self) -> Option<&kh_ui::Movie> {
        self.ally.window.as_ref().and_then(|w| w.movie())
    }

    /// (ALLYJOIN) The Party Formation screen closed: true when a Yes here had opened it (the host is not sent back to a
    /// save-point menu that was never up).
    pub(super) fn ally_formation_closed(&mut self) -> bool {
        std::mem::take(&mut self.ally.formation)
    }

    /// (ALLYJOIN) One frame of the question. True: it is up and has the frame (nothing else of the HUD takes input).
    pub(super) fn ally_frame(&mut self, f: &Frame, nav: &[(u8, bool)], words: &[String]) -> bool {
        use kh_core::PadButton as B;
        // a Yes's hold runs out once the screen is up (or after HOLD_FRAMES)
        if self.ally.hold > 0 {
            if self.formation_open {
                self.ally.hold = 0;
            } else {
                self.ally.hold = self.ally.hold.saturating_sub(f.ticks.max(1));
                if self.ally.hold == 0 {
                    // the screen never came: a later save-point Party Formation must not be taken for ours
                    self.ally.formation = false;
                    (self.log)("hud: (ALLYJOIN) the Party Formation screen did not open after the Yes: the world is let go");
                }
            }
        }
        if self.ally.window.is_none() {
            return false;
        }
        if self.ally.asking.is_none() {
            // the window leaving
            if let Some(w) = self.ally.window.as_mut() {
                for _ in 0..f.ticks {
                    w.tick();
                }
            }
            // its turn: the world free and no OBTAINED popup up or waiting (his own "obtained" first)
            if self.ally.waiting.is_empty() || self.ally.hold > 0 || self.obtained_open() || self.obtained_waiting() > 0 || !self.obtained_free() || self.shop.is_open() {
                return false;
            }
            let (chara, text) = self.ally.waiting.pop_front().unwrap();
            if let Some(w) = self.ally.window.as_mut() {
                w.ask(&text, 0);
            }
            self.ally.asking = Some(chara);
            (self.log)(&format!("hud: (ALLYJOIN) the question for {} is up (the world is held; Yes / No, the cursor on Yes)", chara.name()));
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
                "menu decide" => keys.push(CampKey::Decide),
                "menu cancel" => keys.push(CampKey::Cancel),
                _ => {}
            }
        }
        let mut answered = None;
        if let Some(w) = self.ally.window.as_mut() {
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
        if let (Some(yes), Some(chara)) = (answered, self.ally.asking) {
            self.ally.asking = None;
            self.ally.answer = Some((chara, yes));
            if yes {
                // the save point's own Party Formation screen, with the HUD's formation (he is unlocked on it)
                let formation = self.formation.clone();
                let _ = self.formation_change.take();
                let opened = self.open_party_formation(&formation);
                self.ally.formation = opened;
                self.ally.hold = if opened { HOLD_FRAMES } else { 0 };
                (self.log)(&format!("hud: (ALLYJOIN) {} joins: YES; the Party Formation screen {}", chara.name(), if opened { "opens" } else { "could NOT open (another menu is up)" }));
            } else {
                (self.log)(&format!("hud: (ALLYJOIN) {} joins: NO (he can be added at a save point's Party Formation)", chara.name()));
            }
        }
        true
    }

    /// (ALLYJOIN) A load / the host going away: the question and its waiting ones go at once (no answer is given).
    pub fn ally_reset(&mut self) {
        self.ally.waiting.clear();
        self.ally.asking = None;
        self.ally.answer = None;
        self.ally.hold = 0;
        self.ally.formation = false;
        if let Some(w) = self.ally.window.as_mut() {
            w.force_close();
        }
    }
}
