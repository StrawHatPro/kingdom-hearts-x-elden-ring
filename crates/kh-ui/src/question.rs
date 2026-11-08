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
//!
//! (ER-POPUPS, the user 2026-11-07: "evergaol pop up boxes, as well as anything else still using Elden Ring's pop up boxes
//! needs a Kingdom Hearts equivalent") THE ONE-BUTTON FORM, [`Question::tell`]: the same window with ONE `DialogButton`
//! whose word is KH3's own "OK" (`kh3_system.general.choice_0003`), for a message that is only acknowledged. A takes it
//! (true), B closes it too (false); the directions do nothing ([`step_ok`]).

use std::collections::HashMap;
use std::sync::Arc;

use crate::camp::CampKey;
use crate::menu::{SE_MENU_CLOSE, SE_MENU_CURSOR, SE_MENU_DECIDE};
use crate::{Fonts, Movie, MovieDef, Pack, PackError, UiSound};

const BUTTONS: [&str; 2] = ["dialogButton", "dialogButton2"];
/// Stage pixels between the two buttons (as the save-point menu's and the camp menu's two-button questions).
const BUTTON_PITCH: f32 = 35.0;
const HOLDER: &str = "dialog.dialogAnm.dialogPoAll.dialogPoAll2";
/// (ER-POPUPS) The window's authored frame: the window's matrix, its size, the content holder's place.
type Frame = (crate::Matrix, (f32, f32), (f32, f32));

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

/// (ER-POPUPS) THE RULE OF THE ONE-BUTTON (OK) WINDOW'S KEYS: A takes OK (true), B and Start close it (false), nothing
/// else does anything (there is no second button to move to).
pub fn step_ok(key: CampKey) -> Step {
    match key {
        CampKey::Decide => Step::Answered(true),
        CampKey::Cancel | CampKey::Menu => Step::Answered(false),
        _ => Step::None,
    }
}

/// (ER-POPUPS; the coordinator 2026-11-07 for the user: "for the KH style pop ups replacing elden ring, the box should be
/// dynamically sized", and the same layout for the save point's, Save / Load's and the shop's questions) KH3'S DIALOG LAYOUT
/// FOR ANY COPY OF THE `Dialog` MOVIE: [`DialogFit::lay`] fits the window to the text and the buttons; [`DialogFit::canvas`]
/// is the offset of the button canvas the caller adds where it places its buttons; [`DialogFit::unlay`] puts the authored
/// window back (a window laid out by another rule).
#[derive(Clone, Copy, Debug, Default)]
pub struct DialogFit {
    /// The window's authored frame (the window's matrix, its size, the content holder's place), taken once.
    frame: Option<Frame>,
    /// KH3's layout of the text in force (the window fitted to it; the buttons under its last line).
    layout: Option<crate::newgame::DialogLayout>,
}

impl DialogFit {
    /// The button canvas' offset from the authored button holder (0, 0 without a layout).
    pub fn canvas(&self) -> (f32, f32) {
        self.layout.map_or((0.0, 0.0), |l| l.canvas)
    }

    /// The window's size of the layout in force (None: the authored window).
    pub fn window(&self) -> Option<(f32, f32)> {
        self.layout.map(|l| l.window)
    }

    /// The authored window again.
    pub fn unlay(&mut self, dialog: &mut Movie) {
        if self.layout.take().is_none() {
            return;
        }
        let (base, _, holder) = self.frame(dialog);
        if let Some(node) = dialog.node_mut("dialog.dialogAnm.dialogWin.dialogWin2") {
            node.matrix = base;
        }
        dialog.set_position(HOLDER, holder.0, holder.1, 0.0);
        if let Some(crate::player::Node { kind: crate::player::Kind::Text(t), .. }) = dialog.node_mut("dialogText.textField") {
            t.adjust = None;
        }
    }

    /// The window's authored frame (taken once, before any layout changed it).
    fn frame(&mut self, dialog: &Movie) -> Frame {
        if self.frame.is_none() {
            let base = dialog.node("dialog.dialogAnm.dialogWin.dialogWin2").map_or(crate::Matrix::IDENTITY, |n| n.matrix);
            let size = dialog.node("dialog.dialogAnm.dialogPoAll").map_or((510.0, 186.0), |n| (-2.0 * n.matrix.tx, -2.0 * n.matrix.ty));
            let holder = dialog.node(HOLDER).map_or((0.0, 0.0), |n| (n.matrix.tx, n.matrix.ty));
            self.frame = Some((base, size, holder));
        }
        self.frame.unwrap_or((crate::Matrix::IDENTITY, (510.0, 186.0), (0.0, 0.0)))
    }

    /// (ER-POPUPS; the coordinator 2026-11-07, the user: "for the KH style pop ups replacing elden ring, the box should be
    /// dynamically sized") KH3's `Dialog.setText(text)` (method 192: crate::newgame::dialog_text_layout, ASH-ROWS's port, the
    /// steps of camp_menu.rs `lay_dialog`): the window (its 9-slice frame scaled) as wide as the widest line within KH3's
    /// WINDOW_SIZE_MIN .. MAX (wrapping past the max), as tall as the lines plus the button rows, its content holder centred
    /// around it, the buttons under the last line.
    pub fn lay(&mut self, dialog: &mut Movie, fonts: &Fonts, text: &str, buttons: &[String]) {
        const FIELD: &str = "dialogText.textField";
        let (base, default_window, _) = self.frame(dialog);
        let text_def = match dialog.node(FIELD) {
            Some(crate::player::Node { kind: crate::player::Kind::Text(t), .. }) => Some(t.def.clone()),
            _ => None,
        };
        let button_def = match dialog.node("dialogButton.dialogBtnText.textField") {
            Some(crate::player::Node { kind: crate::player::Kind::Text(t), .. }) => Some(t.def.clone()),
            _ => None,
        };
        let Some((def, font)) = text_def.and_then(|d| fonts.get(d.font_class.as_deref()).cloned().filter(|_| d.size > 0.0).map(|f| (d, f))) else {
            dialog.set_text(FIELD, text, fonts);
            self.layout = None;
            return;
        };
        let text_width = text.lines().map(|line| crate::render::markup_width(fonts, &font, line, def.size)).fold(0.0, f32::max);
        let buttons_width = button_def.and_then(|b| fonts.get(b.font_class.as_deref()).cloned().map(|f| (f, b.size))).map_or(0.0, |(f, size)| buttons.iter().map(|b| crate::render::markup_width(fonts, &f, b, size)).fold(0.0, f32::max));
        let height = |field_w: f32| crate::newgame::text_height(&font, def.size, def.leading, crate::render::wrap_text(fonts, &font, text, def.size, field_w - 4.0).lines().count());
        let holder = |movie: &Movie, name: &str| movie.node(&format!("{HOLDER}.{name}")).map_or((0.0, 0.0), |n| (n.matrix.tx, n.matrix.ty));
        let (text_pos, button_pos) = (holder(dialog, "poDialogText"), holder(dialog, "poDialogButton"));
        let default_text = dialog.sprite(&format!("{HOLDER}.poDialogText")).map_or((0.0, 0.0), |s| {
            let mut r = crate::geom::Rect::EMPTY;
            for n in &s.children {
                if let crate::player::Kind::Shape(sh) = &n.kind {
                    r = r.union(&sh.bounds.rect().transform(&n.matrix));
                }
            }
            if r.is_empty() {
                (0.0, 0.0)
            } else {
                (r.x1 - r.x0, r.y1 - r.y0)
            }
        });
        let d = crate::newgame::dialog_text_layout(text_width, height, (buttons.len(), buttons_width), default_window, [text_pos, button_pos], default_text);
        let (sx, sy) = (d.layout.window.0 / default_window.0.max(1.0), d.layout.window.1 / default_window.1.max(1.0));
        if let Some(node) = dialog.node_mut("dialog.dialogAnm.dialogWin.dialogWin2") {
            node.matrix = crate::Matrix::new(base.a * sx, base.b * sx, base.c * sy, base.d * sy, base.tx, base.ty);
            node.script_matrix = true;
        }
        dialog.set_position(HOLDER, d.layout.holder.0, d.layout.holder.1, 0.0);
        dialog.set_text(FIELD, text, fonts);
        if let Some(node) = dialog.node_mut(FIELD) {
            let tx = node.matrix.tx;
            if let crate::player::Kind::Text(t) = &mut node.kind {
                t.adjust = Some((1.0, tx + t.def.bounds.x0, d.field.0));
            }
        }
        self.layout = Some(d.layout);
    }

}

/// (ALLYJOIN) The Dialog window as a Yes / No question of the host's (ER-POPUPS: or an OK message, [`Question::tell`]).
pub struct Question {
    dialog: Movie,
    fonts: Fonts,
    yes_no: [String; 2],
    /// (ER-POPUPS) KH3's "OK" (the one-button form's word).
    ok: String,
    /// (ER-POPUPS) The window shows one button (OK) instead of two (Yes / No).
    single: bool,
    /// Up: the cursor (0 = Yes, 1 = No).
    cursor: Option<usize>,
    closing: bool,
    /// (ER-POPUPS) KH3's layout of the window (fitted to its text).
    fit: DialogFit,
}

impl Question {
    pub fn load(pack: &mut Pack, fonts: Fonts) -> Result<Question, PackError> {
        let def: Arc<MovieDef> = pack.movie_linked("Dialog", &crate::camp_menu::LIBRARIES)?;
        let system: HashMap<String, String> = pack.string_table("kh3_system");
        let word = |k: &str, or: &str| system.get(k).cloned().filter(|s| !s.is_empty()).unwrap_or_else(|| or.to_string());
        let yes_no = [word("kh3_system.general.choice_0001", "Yes"), word("kh3_system.general.choice_0002", "No")];
        let ok = word("kh3_system.general.choice_0003", "OK");
        let mut dialog = Movie::new(def);
        dialog.goto_and_stop("dialog", "off");
        Ok(Question { dialog, fonts, yes_no, ok, single: false, cursor: None, closing: false, fit: DialogFit::default() })
    }

    /// (ER-POPUPS) The window shows one button (OK): the open window is a [`Question::tell`].
    pub fn is_single(&self) -> bool {
        self.single
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
        self.open(text, false);
        self.show_cursor(cursor.min(1));
        self.place();
    }

    /// (ER-POPUPS) Opens the window as a MESSAGE with `text` and one button, KH3's "OK" (selected): A takes it, B closes it
    /// ([`step_ok`]); the answer comes from [`Question::key`] as for a question (true = A).
    pub fn tell(&mut self, text: &str) {
        self.open(text, true);
        self.show_cursor(0);
        self.place();
    }

    /// The window in (when it was not up), its text and its button(s) with their words.
    fn open(&mut self, text: &str, single: bool) {
        let fonts = self.fonts.clone();
        if self.cursor.is_none() {
            self.dialog.goto_and_play("dialog", "in");
            self.dialog.add_instance("", "DialogButton", BUTTONS[1]);
        }
        self.closing = false;
        self.single = single;
        self.dialog.set_visible("dialogText", true);
        let words: Vec<String> = if single { vec![self.ok.clone()] } else { self.yes_no.to_vec() };
        self.fit.lay(&mut self.dialog, &fonts, &text.replace("<br>", "\n"), &words);
        for (n, name) in BUTTONS.into_iter().enumerate() {
            let shown = !single || n == 0;
            self.dialog.set_visible(name, shown);
            let word = if single { &self.ok } else { &self.yes_no[n] };
            self.dialog.set_text(&format!("{name}.dialogBtnText.textField"), word, &fonts);
        }
    }

    fn show_cursor(&mut self, cursor: usize) {
        self.cursor = Some(cursor);
        for (n, name) in BUTTONS.into_iter().enumerate() {
            self.dialog.goto_and_play(name, if cursor == n { "select" } else { "normal" });
        }
        if self.single {
            self.dialog.set_visible(BUTTONS[1], false);
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
            // (ER-POPUPS) the button canvas of KH3's layout (under the text's last line)
            let (cx, cy) = self.fit.canvas();
            let (x, y) = (x + cx, y + cy);
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
        let got = if self.single { step_ok(key) } else { step(cursor, key) };
        match got {
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

    /// (ER-POPUPS) The one-button (OK) window: A takes OK, B / Start close it, the directions and the rest do nothing.
    #[test]
    fn er_popups_the_ok_window_keys() {
        assert_eq!(step_ok(CampKey::Decide), Step::Answered(true));
        assert_eq!(step_ok(CampKey::Cancel), Step::Answered(false));
        assert_eq!(step_ok(CampKey::Menu), Step::Answered(false));
        for k in [CampKey::Up, CampKey::Down, CampKey::Left, CampKey::Right, CampKey::L1] {
            assert_eq!(step_ok(k), Step::None, "{k:?}");
        }
    }
}
