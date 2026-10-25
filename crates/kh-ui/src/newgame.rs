//! (new game) THE NEW-GAME CHOICE MENU: the two questions of Kingdom Hearts III's opening ("What do you
//! desire?" Wisdom / Vitality / Balance, "What power do you seek?" Guardian / Warrior / Mystic), each
//! with its description and its confirmation, and the last "Is this who you are?", as a menu a host can
//! show before Sora exists in its world.
//!
//! WHAT THE GAME DOES ([A] = read from the blueprints `Levels/ew/ew_01/umap/ew_01_gameflow` and
//! `ew_01_camera_ms010`, the string table `kh3_level.ew`, the movies `92_message/CutsceneText` and
//! `92_message/Dialog`; nothing was seen in the running game):
//! - The questions are asked IN THE WORLD: Sora walks to one of three mirrors (left, centre, right). The
//!   words are drawn by `TresUI_SetCutsceneText` (movie CutsceneText: white 38-point text in the field
//!   x 98 .. 1824, y 72 .., revealed line by line by a 960-pixel gradient wipe, `ShowMethod` LineFade),
//!   the Yes / No by `TresUI_OpenChoice` (movie Dialog with no text, colour DH, at 0.5 / 0.8 of the screen).
//! - Question 1: `text_0020` "What do you desire?" (centred at the top; shown over 2 s, kept 2 s, faded
//!   over 1 s; the mirrors work after 5 s). A mirror: its text (`text_0120` Wisdom left, `text_0110`
//!   Vitality centre, `text_0130` Balance right; left-aligned at the top, shown over 3 s), after 4 s
//!   `text_0040` "Is this what you desire?" in its place (it begins with two line breaks) and 2 s later
//!   the choice [`choice_0080` "No", `choice_0070` "Yes"] with the cursor on the first entry. Yes sets
//!   `1stChoice` (centre 0, right 1, left 2); No gives the mirrors back.
//! - Question 2: `text_0030` "What power do you seek?", mirrors `text_0230` the guardian left, `text_0210`
//!   the warrior centre, `text_0220` the mystic right, confirmation `text_0035` "Is this the power you
//!   seek?"; Yes sets `2ndChoice` (centre 0, right 1, left 2).
//! - Then `text_0240` "Is this who you are?" (centred) and 2 s later the same choice: Yes applies the pair
//!   (`BP_TresSetPlayerGrowthType` and the choice's victory bonus), No starts the mirrors again.
//!
//! WHAT IS THE GAME'S HERE: every text (the strings above, and the three-entry lists' "Choose Wisdom" ..
//! `choice_0010` .. `0060`, which the table holds for exactly these six options); the movie CutsceneText
//! with its field, font and sizes; the movie Dialog: its window art in the DH colour, its button clips
//! with their cursor, its open / close animation, its own layout rule (`Dialog.setText`, ported in
//! [`dialog_layout`]: 35 pixels per button, the window grown around them), the order and the default
//! cursor of the Yes / No, the positions 0.5 / 0.8, the times above, the menus' sounds.
//! WHAT IS COMPOSED HERE ([L], because a host has no mirrors to walk to): the three options of a question
//! are a Dialog list in the mirrors' order left, centre, right at the centre of the screen (0.5 / 0.5, as
//! the opening's own three-entry choice "Do you seek further guidance?" is placed), under the question,
//! which stays on screen while the list is up; a press of decide ends a wait at once (the game's script
//! waits cannot be skipped); cancel goes back one step (to the list from a confirmation, to the first
//! question's list from the second's and from the last confirmation; in the first list it is refused);
//! the gradient wipe is drawn with a hard edge at its middle (the player has no soft masks).
//! The background is the host's (a dark screen).

use std::collections::HashMap;
use std::sync::Arc;

use crate::camp::CampKey;
use crate::def::{Align, TextDef};
use crate::menu::{SE_MENU_CLOSE, SE_MENU_CURSOR, SE_MENU_DECIDE, SE_MENU_OPEN, SE_MENU_REFUSED};
use crate::player::Kind;
use crate::{Fonts, Matrix, Movie, MovieDef, Pack, PackError, UiSound};

/// The mirrors of a question, left to right: (the option's name in the list, its description).
pub const OPTIONS: [[(&str, &str); 3]; 2] = [
    [("kh3_level.ew.choice_0020", "kh3_level.ew.text_0120"), ("kh3_level.ew.choice_0010", "kh3_level.ew.text_0110"), ("kh3_level.ew.choice_0030", "kh3_level.ew.text_0130")],
    [("kh3_level.ew.choice_0060", "kh3_level.ew.text_0230"), ("kh3_level.ew.choice_0040", "kh3_level.ew.text_0210"), ("kh3_level.ew.choice_0050", "kh3_level.ew.text_0220")],
];
/// (the question, its confirmation) per question, and the last confirmation.
pub const QUESTIONS: [(&str, &str); 2] = [("kh3_level.ew.text_0020", "kh3_level.ew.text_0040"), ("kh3_level.ew.text_0030", "kh3_level.ew.text_0035")];
pub const FINAL_QUESTION: &str = "kh3_level.ew.text_0240";
/// The choice window's entries in its order: No, Yes (the blueprint's array; default index 0).
pub const NO_YES: [&str; 2] = ["kh3_level.ew.choice_0080", "kh3_level.ew.choice_0070"];
/// The value the game stores for the mirror at `index` 0 left, 1 centre, 2 right (`1stChoice` /
/// `2ndChoice`): centre 0, right 1, left 2.
pub const MIRROR_VALUE: [u8; 3] = [2, 0, 1];

/// Stage frames (60 a second) of the script's waits: the question's show time, the description's delay,
/// the wait before a Yes / No.
pub const QUESTION_SHOW: u32 = 120;
pub const DESCRIPTION_SHOW: u32 = 180;
pub const DESCRIPTION_WAIT: u32 = 240;
pub const CONFIRM_SHOW: u32 = 120;
pub const CONFIRM_WAIT: u32 = 120;
/// `CutsceneText.GRADATION_WIDTH`.
pub const GRADATION_WIDTH: f32 = 960.0;
/// `Dialog.BUTTON_HEIGHT`, `WINDOW_SIZE_MIN`, `WINDOW_SIZE_MAX`.
pub const BUTTON_HEIGHT: f32 = 35.0;
pub const WINDOW_SIZE_MIN: (f32, f32) = (256.0, 128.0);
pub const WINDOW_SIZE_MAX: (f32, f32) = (1024.0, 640.0);
/// `m_colorDictionary["dh"]`: text colour 0xAABCF2 (the glow 0x243159 is a filter: not drawn).
pub const DH_TEXT_COLOR: [f32; 4] = [170.0, 188.0, 242.0, 255.0];

const TEXT_FIELD: &str = "coCutsceneTextNe.textField";
const WIN: &str = "dialog.dialogAnm.dialogWin.dialogWin2";
const PO_ALL: &str = "dialog.dialogAnm.dialogPoAll.dialogPoAll2";
const MAX_BUTTONS: usize = 3;

/// The result: the mirror picked for each question, 0 left, 1 centre, 2 right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NewGamePick {
    pub mirrors: [usize; 2],
}

impl NewGamePick {
    /// What the game stores: (`1stChoice`, `2ndChoice`), each 0 / 1 / 2.
    pub fn values(self) -> (u8, u8) {
        (MIRROR_VALUE[self.mirrors[0].min(2)], MIRROR_VALUE[self.mirrors[1].min(2)])
    }
}

/// (ER party S8) THE STARTER QUESTION the Elden Ring host asks before the opening's two (the user, 2026-10-15: "At the
/// beginning of the game you can choose who you want to start playing as: Sora, Donald or Goofy"). Its words are the
/// host's (OUR TEXT, sheet party-er-unlocks); it runs as question [`STARTER`] with the same window, waits, sounds and keys
/// as the game's two: the list, the description, the No / Yes confirmation. Without it the menu is exactly the game's.
#[derive(Clone, Debug, PartialEq)]
pub struct StarterQuestion {
    pub question: String,
    /// Sora, Donald, Goofy (the order of the result's index).
    pub options: [String; 3],
    pub describe: [String; 3],
    pub confirm: String,
}

/// (ER party S8) The question index of the starter question in [`Step`].
pub const STARTER: usize = 2;

/// Where the menu is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// The question and its three options.
    List(usize),
    /// The picked option's description.
    Describe(usize),
    /// "Is this what you desire?" / "Is this the power you seek?" with No / Yes.
    Confirm(usize),
    /// "Is this who you are?" with No / Yes.
    Final,
}

/// `Dialog.setText("", colour)` with `count` buttons whose widest text is `text_width` (the text-less
/// form the opening uses): the window's size, the offset of the content holder (`m_mesPoAll`) and the
/// button canvas' position in the button holder (`m_poDialogButton`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DialogLayout {
    pub window: (f32, f32),
    pub holder: (f32, f32),
    pub canvas: (f32, f32),
}

/// `default_window` = the window's authored size, `text_pos` / `button_pos` = the two position holders
/// inside the content holder.
pub fn dialog_layout(count: usize, text_width: f32, default_window: (f32, f32), text_pos: (f32, f32), button_pos: (f32, f32)) -> DialogLayout {
    let margin_x = button_pos.0 - text_pos.0;
    // No text: the field is WINDOW_SIZE_MIN wide and the buttons move up to where the text would be.
    let field_width = WINDOW_SIZE_MIN.0;
    let canvas_y = text_pos.1 - button_pos.1;
    let mut height = text_pos.1 + BUTTON_HEIGHT * (count.max(1) - 1) as f32 + (default_window.1 - button_pos.1);
    let buttons_width = text_width + 5.0 + margin_x;
    let mut width = WINDOW_SIZE_MIN.0;
    if buttons_width > field_width {
        width = (text_pos.0 * 2.0 + buttons_width - 5.0).min(WINDOW_SIZE_MAX.0);
    }
    height = height.clamp(WINDOW_SIZE_MIN.1, WINDOW_SIZE_MAX.1);
    DialogLayout {
        window: (width + 2.0, height + 2.0),
        holder: ((default_window.0 - width) * 0.5, (default_window.1 - height) * 0.5),
        canvas: (((width - buttons_width) - margin_x) * 0.5 - text_pos.0, canvas_y),
    }
}

/// How the words on screen are shown.
#[derive(Clone, Debug, PartialEq)]
struct Words {
    /// The lines.
    lines: Vec<String>,
    centre: bool,
    /// Stage frames since it was set, and its show time (LineFade).
    age: u32,
    show: u32,
    /// Fading out: frames left of `hide`.
    hiding: Option<(u32, u32)>,
}

pub struct NewGameChoice {
    /// The words (movie CutsceneText) and the choice window (movie Dialog): two layers.
    pub text: Movie,
    pub dialog: Movie,
    fonts: Fonts,
    strings: HashMap<String, String>,
    open: bool,
    step: Step,
    /// Frames in this step.
    step_age: u32,
    cursor: usize,
    picked: [usize; 2],
    words: Option<Words>,
    /// The window: its entries, open (from `in` on), the keys count, leaving (what follows).
    entries: Vec<String>,
    window_open: bool,
    input_on: bool,
    leaving: Option<Leave>,
    default_window: (f32, f32),
    win_matrix: Matrix,
    /// The text field's authored x inside its clip.
    text_base_x: f32,
    layout: DialogLayout,
    result: Option<NewGamePick>,
    /// (ER party S8) the starter question (None = the game's menu), its cursor, the confirmed pick.
    starter: Option<StarterQuestion>,
    starter_pick: usize,
    starter_result: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Leave {
    To(Step),
    Done,
}

impl NewGameChoice {
    pub fn load(pack: &mut Pack, fonts: Fonts) -> Result<NewGameChoice, PackError> {
        let text: Arc<MovieDef> = pack.movie("CutsceneText")?;
        let dialog: Arc<MovieDef> = pack.movie_linked("Dialog", &crate::camp_menu::LIBRARIES)?;
        let strings = pack.string_table("kh3_level");
        let mut menu = NewGameChoice {
            text: Movie::new(text),
            dialog: Movie::new(dialog),
            fonts,
            strings,
            open: false,
            step: Step::List(0),
            step_age: 0,
            cursor: 0,
            picked: [0, 0],
            words: None,
            entries: Vec::new(),
            window_open: false,
            input_on: false,
            leaving: None,
            default_window: (510.0, 186.0),
            win_matrix: Matrix::IDENTITY,
            text_base_x: 0.0,
            layout: DialogLayout { window: (0.0, 0.0), holder: (0.0, 0.0), canvas: (0.0, 0.0) },
            result: None,
            starter: None, // (ER party S8)
            starter_pick: 0,
            starter_result: None,
        };
        // `Dialog.<init>`: the authored window size and matrix; the template text and button leave the stage.
        if let Some(node) = menu.dialog.node(WIN) {
            menu.win_matrix = node.matrix;
        }
        if let Some(all) = menu.dialog.node("dialog.dialogAnm.dialogPoAll") {
            // The window is centred on the clip's origin and the content holder sits at its top left corner.
            menu.default_window = (-2.0 * all.matrix.tx, -2.0 * all.matrix.ty);
        }
        if let Some(field) = menu.text.node(TEXT_FIELD) {
            menu.text_base_x = field.matrix.tx;
        }
        menu.dialog.set_visible("dialogText", false);
        menu.dialog.set_visible("dialogButton", false);
        menu.dialog.goto_and_stop(&format!("{WIN}.dialogWinWin"), "dh");
        for n in 0..MAX_BUTTONS {
            let name = format!("ngButton{n}");
            menu.dialog.add_instance("", "DialogButton", &name);
            menu.dialog.set_visible(&name, false);
            recolor(&mut menu.dialog, &format!("{name}.dialogBtnText.textField"), Some(DH_TEXT_COLOR), None);
        }
        menu.text.set_visible("coCutsceneTextJa", false);
        menu.text.set_visible("coCutsceneTextNe", false);
        Ok(menu)
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn step(&self) -> Step {
        self.step
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// The window is up and its keys count.
    pub fn accepts_input(&self) -> bool {
        self.open && self.window_open && self.input_on && self.leaving.is_none()
    }

    /// The words on screen now (the lines as set; "" between them for empty lines).
    pub fn shown_text(&self) -> String {
        self.words.as_ref().map(|w| w.lines.join("\n")).unwrap_or_default()
    }

    /// The window's entries (empty while it is not up).
    pub fn entries(&self) -> &[String] {
        if self.window_open { &self.entries } else { &[] }
    }

    fn string(&self, key: &str) -> String {
        self.strings.get(key).cloned().unwrap_or_default()
    }

    /// Opens at the first question.
    pub fn open(&mut self, sounds: &mut Vec<UiSound>) {
        sounds.push(UiSound::Play(SE_MENU_OPEN));
        self.open = true;
        self.result = None;
        self.picked = [0, 0];
        self.starter_result = None; // (ER party S8)
        self.enter(Step::List(self.first()));
    }

    /// Takes the menu off at once, without a result.
    pub fn close(&mut self) {
        self.open = false;
        self.words = None;
        self.hide_window();
        self.text.set_visible("coCutsceneTextNe", false);
    }

    fn hide_window(&mut self) {
        self.window_open = false;
        self.input_on = false;
        self.leaving = None;
        self.dialog.goto_and_play("dialog", "off");
        for n in 0..MAX_BUTTONS {
            self.dialog.set_visible(&format!("ngButton{n}"), false);
        }
    }

    /// (ER party S8) Asks the starter question first (None: the game's menu as it was).
    pub fn set_starter_question(&mut self, question: Option<StarterQuestion>) {
        self.starter = question;
    }

    /// (ER party S8) The starter the player confirmed (0 Sora, 1 Donald, 2 Goofy), once, with the menu's result.
    pub fn take_starter(&mut self) -> Option<usize> {
        self.starter_result.take()
    }

    /// (ER party S8) The first question: the starter's when it is asked.
    fn first(&self) -> usize {
        if self.starter.is_some() { STARTER } else { 0 }
    }

    fn enter(&mut self, step: Step) {
        // (ER party S8) the starter question's three steps take the host's words
        if let Some(s) = self.starter.clone() {
            match step {
                Step::List(STARTER) | Step::Describe(STARTER) | Step::Confirm(STARTER) => {
                    self.step = step;
                    self.step_age = 0;
                    self.hide_window();
                    match step {
                        Step::List(_) => {
                            self.set_words_text(&s.question, true, QUESTION_SHOW);
                            self.cursor = self.starter_pick;
                        }
                        Step::Describe(_) => self.set_words_text(&s.describe[self.starter_pick.min(2)], false, DESCRIPTION_SHOW),
                        _ => {
                            self.set_words_text(&s.confirm, false, CONFIRM_SHOW);
                            self.cursor = 0;
                        }
                    }
                    return;
                }
                _ => {}
            }
        }
        self.step = step;
        self.step_age = 0;
        self.hide_window();
        match step {
            Step::List(q) => {
                self.set_words(QUESTIONS[q].0, true, QUESTION_SHOW);
                self.cursor = self.picked[q];
            }
            Step::Describe(q) => self.set_words(OPTIONS[q][self.picked[q]].1, false, DESCRIPTION_SHOW),
            Step::Confirm(q) => {
                self.set_words(QUESTIONS[q].1, false, CONFIRM_SHOW);
                self.cursor = 0;
            }
            Step::Final => {
                self.set_words(FINAL_QUESTION, true, CONFIRM_SHOW);
                self.cursor = 0;
            }
        }
    }

    /// `CutsceneText.setText(text, false, alignH, top, .., LineFade, show, ..)`.
    fn set_words(&mut self, key: &str, centre: bool, show: u32) {
        let text = self.string(key);
        self.set_words_text(&text, centre, show);
    }

    /// (ER party S8) `set_words` with the words themselves (the starter question's are the host's).
    fn set_words_text(&mut self, text: &str, centre: bool, show: u32) {
        let lines: Vec<String> = text.split("<br>").map(str::to_string).collect();
        self.words = Some(Words { lines, centre, age: 0, show, hiding: None });
        self.text.set_visible("coCutsceneTextNe", true);
        self.draw_words();
    }

    /// The text of the field for the wipe's place now, and the field's place.
    fn draw_words(&mut self) {
        let Some(words) = self.words.clone() else { return };
        let Some((size, box_w)) = self.text.node(TEXT_FIELD).and_then(|n| match &n.kind {
            Kind::Text(t) => Some((t.def.size, t.def.bounds.x1 - t.def.bounds.x0 - 4.0)),
            _ => None,
        }) else {
            return;
        };
        let Some(font) = self.fonts.get(Some("$jimakuNE")).cloned() else { return };
        let widths: Vec<f32> = words.lines.iter().map(|l| font.text_width(l, size)).collect();
        // `m_graAddDelta` = the sum of (line width + GRADATION_WIDTH) over the show time; the lines are
        // revealed one after the other. Here: the characters left of the gradient's middle are shown.
        let total: f32 = widths.iter().map(|w| w + GRADATION_WIDTH).sum();
        let mut travelled = if words.show == 0 { total } else { total * (words.age as f32 / words.show as f32).min(1.0) };
        let mut shown = Vec::new();
        for (line, width) in words.lines.iter().zip(&widths) {
            let edge = travelled - GRADATION_WIDTH * 0.5;
            let mut x = 0.0;
            let mut cut = String::new();
            for c in line.chars() {
                let advance = font.text_width(c.encode_utf8(&mut [0; 4]), size);
                if x + advance * 0.5 > edge && travelled < width + GRADATION_WIDTH {
                    break;
                }
                cut.push(c);
                x += advance;
            }
            shown.push(cut);
            travelled = (travelled - (width + GRADATION_WIDTH)).max(0.0);
        }
        let fonts = self.fonts.clone();
        self.text.set_text(TEXT_FIELD, &shown.join("\n"), &fonts);
        // Centred: the block is centred in the field (`TextFieldAlign.ALIGN_H_CENTER`); the field keeps its
        // left alignment so that the shown part of a line does not move while it is revealed.
        let widest = widths.iter().copied().fold(0.0, f32::max);
        let shift = if words.centre { ((box_w - widest) * 0.5).max(0.0) } else { 0.0 };
        let alpha = words.hiding.map_or(1.0, |(left, of)| left as f32 / of.max(1) as f32);
        if let Some(node) = self.text.node_mut(TEXT_FIELD) {
            node.matrix.tx = self.text_base_x + shift;
            node.script_matrix = true;
            node.cx.mul[3] = alpha;
        }
    }

    /// The window with `entries`, the cursor on `self.cursor`, at `at` of the screen.
    fn open_window(&mut self, entries: Vec<String>, at: (f32, f32)) {
        let fonts = self.fonts.clone();
        let size = self.dialog.node("ngButton0.dialogBtnText.textField").and_then(|n| match &n.kind {
            Kind::Text(t) => Some(t.def.size),
            _ => None,
        });
        let font = self.fonts.get(Some("$jimakuNE")).cloned();
        let widest = match (&font, size) {
            (Some(font), Some(size)) => entries.iter().map(|e| font.text_width(e, size)).fold(0.0, f32::max),
            _ => 200.0,
        };
        let (text_pos, button_pos) = (self.holder_pos("poDialogText"), self.holder_pos("poDialogButton"));
        self.layout = dialog_layout(entries.len(), widest, self.default_window, text_pos, button_pos);
        // `m_mesWin.width / height`, `m_mesPoAll.x / y`, `Dialog.setPosition`.
        let (sx, sy) = (self.layout.window.0 / self.default_window.0, self.layout.window.1 / self.default_window.1);
        let base = self.win_matrix;
        if let Some(node) = self.dialog.node_mut(WIN) {
            node.matrix = Matrix::new(base.a * sx, base.b * sx, base.c * sy, base.d * sy, base.tx, base.ty);
            node.script_matrix = true;
        }
        self.dialog.set_position(PO_ALL, self.layout.holder.0, self.layout.holder.1, 0.0);
        self.dialog.set_position("dialog", 1920.0 * at.0, 1080.0 * at.1, 0.0);
        for n in 0..MAX_BUTTONS {
            let name = format!("ngButton{n}");
            self.dialog.set_visible(&name, n < entries.len());
            if let Some(entry) = entries.get(n) {
                self.dialog.set_text(&format!("{name}.dialogBtnText.textField"), entry, &fonts);
            }
        }
        self.entries = entries;
        self.window_open = true;
        self.input_on = false;
        self.leaving = None;
        self.dialog.goto_and_play("dialog", "in");
        self.show_cursor();
        self.place_buttons();
    }

    fn holder_pos(&self, name: &str) -> (f32, f32) {
        self.dialog.node(&format!("{PO_ALL}.{name}")).map_or((0.0, 0.0), |n| (n.matrix.tx, n.matrix.ty))
    }

    fn show_cursor(&mut self) {
        for n in 0..self.entries.len() {
            let name = format!("ngButton{n}");
            let label = if n == self.cursor { "select" } else { "normal" };
            if self.dialog.current_label(&name) != Some(label) {
                self.dialog.goto_and_play(&name, label);
            }
        }
    }

    /// The buttons are children of the button holder in the game: here they follow its matrix every frame
    /// (the window's open and close animation moves it).
    fn place_buttons(&mut self) {
        let mut m = Matrix::IDENTITY;
        let mut at = String::new();
        let mut alpha = 1.0;
        let mut shown = true;
        for part in format!("{PO_ALL}.poDialogButton").split('.') {
            at = if at.is_empty() { part.to_string() } else { format!("{at}.{part}") };
            let Some(node) = self.dialog.node(&at) else { return };
            m = m.mul(&node.matrix);
            alpha *= node.cx.mul[3];
            shown &= node.is_visible();
        }
        for n in 0..self.entries.len() {
            let name = format!("ngButton{n}");
            let local = Matrix::new(1.0, 0.0, 0.0, 1.0, self.layout.canvas.0, self.layout.canvas.1 + BUTTON_HEIGHT * n as f32);
            if let Some(node) = self.dialog.node_mut(&name) {
                node.matrix = m.mul(&local);
                node.script_matrix = true;
                node.cx.mul[3] = alpha;
                node.script_visible = Some(shown && self.window_open);
            }
        }
    }

    fn leave_window(&mut self, to: Leave) {
        self.dialog.goto_and_play("dialog", "out");
        self.leaving = Some(to);
    }

    /// A menu key. Up / down move the cursor (a fresh press wraps, a held direction stops at the ends:
    /// the Dialog's `repeatStop`), decide chooses, cancel goes back a step.
    pub fn key(&mut self, key: CampKey, repeat: bool, sounds: &mut Vec<UiSound>) {
        if !self.open || self.leaving.is_some() {
            return;
        }
        if !self.window_open {
            // A wait of the script: decide ends it at once [L].
            if key == CampKey::Decide && !repeat {
                self.skip_wait();
            } else if key == CampKey::Cancel && !repeat {
                self.back(sounds);
            }
            return;
        }
        if !self.input_on {
            return;
        }
        match key {
            CampKey::Up | CampKey::Down => {
                if let Some(next) = crate::camp::move_wrapping(self.cursor, self.entries.len(), key == CampKey::Down, repeat) {
                    self.cursor = next;
                    sounds.push(UiSound::Play(SE_MENU_CURSOR));
                    self.show_cursor();
                }
            }
            CampKey::Decide if !repeat => {
                sounds.push(UiSound::Play(SE_MENU_DECIDE));
                self.dialog.goto_and_play(&format!("ngButton{}", self.cursor), "decide");
                let yes = self.cursor == 1;
                let to = match self.step {
                    Step::List(STARTER) => {
                        // (ER party S8)
                        self.starter_pick = self.cursor;
                        Leave::To(Step::Describe(STARTER))
                    }
                    Step::List(q) => {
                        self.picked[q] = self.cursor;
                        Leave::To(Step::Describe(q))
                    }
                    Step::Confirm(q) if !yes => Leave::To(Step::List(q)),
                    Step::Confirm(STARTER) => Leave::To(Step::List(0)), // (ER party S8)
                    Step::Confirm(0) => Leave::To(Step::List(1)),
                    Step::Confirm(_) => Leave::To(Step::Final),
                    Step::Final if yes => Leave::Done,
                    Step::Final => Leave::To(Step::List(self.first())), // (ER party S8: the starter question first)
                    Step::Describe(q) => Leave::To(Step::Confirm(q)),
                };
                if matches!(self.step, Step::Confirm(_) | Step::Final) {
                    // `TresUI_CloseCutsceneText`: the words fade (0.33 s).
                    if let Some(words) = self.words.as_mut() {
                        words.hiding = Some((20, 20));
                    }
                }
                self.leave_window(to);
            }
            CampKey::Cancel if !repeat => self.back(sounds),
            _ => {}
        }
    }

    fn skip_wait(&mut self) {
        if let Some(words) = self.words.as_mut() {
            words.age = words.age.max(words.show);
        }
        self.step_age = u32::MAX / 2;
    }

    /// Cancel [L]: one step back.
    fn back(&mut self, sounds: &mut Vec<UiSound>) {
        let to = match self.step {
            // (ER party S8) from the opening's first question back to the starter question
            Step::List(0) if self.starter.is_some() => Step::List(STARTER),
            Step::List(STARTER) if self.starter.is_some() => {
                sounds.push(UiSound::Play(SE_MENU_REFUSED));
                return;
            }
            Step::List(0) => {
                sounds.push(UiSound::Play(SE_MENU_REFUSED));
                return;
            }
            Step::List(_) | Step::Final => Step::List(0),
            Step::Describe(q) | Step::Confirm(q) => Step::List(q),
        };
        sounds.push(UiSound::Play(SE_MENU_CLOSE));
        if self.window_open {
            self.leave_window(Leave::To(to));
        } else {
            self.enter(to);
        }
    }

    /// One stage frame. The pick, once, when the last Yes has left the screen (the menu is then closed).
    pub fn tick(&mut self) -> Option<NewGamePick> {
        if !self.open {
            return None;
        }
        self.step_age = self.step_age.saturating_add(1);
        if let Some(words) = self.words.as_mut() {
            words.age = words.age.saturating_add(1);
            if let Some((left, _)) = words.hiding.as_mut() {
                *left = left.saturating_sub(1);
            }
        }
        self.draw_words();
        self.text.tick();
        self.text.take_events();
        self.dialog.tick();
        let mut ended = false;
        for event in self.dialog.take_events() {
            match event.name.as_str() {
                "ANIM_END_MES_WIN_IN" => self.input_on = true,
                "ANIM_END_MES_WIN_OUT" => ended = true,
                _ => {}
            }
        }
        self.place_buttons();
        if ended {
            if let Some(leave) = self.leaving.take() {
                match leave {
                    Leave::To(step) => self.enter(step),
                    Leave::Done => {
                        let pick = NewGamePick { mirrors: self.picked };
                        self.starter_result = self.starter.is_some().then_some(self.starter_pick); // (ER party S8)
                        self.close();
                        self.result = Some(pick);
                        return Some(pick);
                    }
                }
            }
            return None;
        }
        // The script's waits: then the window opens, or the description gives way to its confirmation.
        if !self.window_open && self.leaving.is_none() {
            match self.step {
                // (ER party S8) the starter question's entries are the host's
                Step::List(STARTER) if self.step_age >= QUESTION_SHOW && self.starter.is_some() => {
                    let entries = self.starter.as_ref().map(|s| s.options.to_vec()).unwrap_or_default();
                    self.open_window(entries, (0.5, 0.5));
                }
                Step::List(q) if self.step_age >= QUESTION_SHOW => {
                    let entries = OPTIONS[q].iter().map(|o| self.string(o.0)).collect();
                    self.open_window(entries, (0.5, 0.5));
                }
                Step::Describe(q) if self.step_age >= DESCRIPTION_WAIT => self.enter(Step::Confirm(q)),
                Step::Confirm(_) | Step::Final if self.step_age >= CONFIRM_WAIT => {
                    let entries = NO_YES.iter().map(|k| self.string(k)).collect();
                    self.open_window(entries, (0.5, 0.8));
                }
                _ => {}
            }
        }
        None
    }
}

/// A text field with another colour and / or alignment (the definition is shared: it is cloned).
fn recolor(movie: &mut Movie, path: &str, color: Option<[f32; 4]>, align: Option<Align>) {
    if let Some(node) = movie.node_mut(path) {
        if let Kind::Text(t) = &mut node.kind {
            let mut def: TextDef = (*t.def).clone();
            if let Some(color) = color {
                def.color = color;
            }
            if let Some(align) = align {
                def.align = align;
            }
            t.def = Arc::new(def);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dialog_layout_without_text() {
        // The authored holders of the Dialog movie: text (34, 40), buttons (74, 115); window 510 x 186.
        let two = dialog_layout(2, 40.0, (510.0, 186.0), (34.0, 40.0), (74.0, 115.0));
        // Height: 40 + 35 + (186 - 115) = 146; width: the minimum.
        assert_eq!(two.window, (258.0, 148.0));
        assert_eq!(two.holder, (127.0, 20.0));
        assert_eq!(two.canvas.1, -75.0);
        let three = dialog_layout(3, 300.0, (510.0, 186.0), (34.0, 40.0), (74.0, 115.0));
        // Buttons 300 + 5 + 40 = 345 wide: the window grows to 2 x 34 + 345 - 5 = 408; height 181.
        assert_eq!(three.window, (410.0, 183.0));
        assert_eq!(three.canvas.0, ((408.0 - 345.0) - 40.0) * 0.5 - 34.0);
    }

    #[test]
    fn the_mirror_values() {
        // Left Wisdom 2, centre Vitality 0, right Balance 1; left guardian 2, centre warrior 0, right mystic 1.
        assert_eq!(NewGamePick { mirrors: [0, 2] }.values(), (2, 1));
        assert_eq!(NewGamePick { mirrors: [1, 1] }.values(), (0, 0));
    }
}
