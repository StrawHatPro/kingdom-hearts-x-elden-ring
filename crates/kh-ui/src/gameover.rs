//! THE GAME-OVER MENU (docs/sora-gameover-spec.md section 3; `UTresUIP_GameOver`, the clip `gameOverMenu`
//! of the Pause movie): the title "MENU", up to five buttons and a hint text in a panel at the lower left
//! of the screen, over the 3D game-over scene (Sora in the dark: the host's).
//!
//! The open function `0x1417035E0(uip, menu_type)` [decompiled C]: per button slot 0..4 the id of table
//! `0x146B37470` (5 ids per menu type; 13 = no button: the slot is hidden), the button's text from the key
//! table at `0x1486F3D50` (16 bytes per id), the hint text into `hintText.coText` (a `TextFieldAlign` clip:
//! centred both ways), the clip's label, the focus on the first button. Both tables were read from the
//! executable's data (the values below; 16 menu types).
//!
//! The focus draws the LIGHT CURSOR of the movie `Cursor` (`UTresUIP_Cursor`, `0x14175AE50` [decompiled
//! C]): the clip `lightCursor` (a small orb that wanders in a loop of 360 frames) at the top right corner
//! of the focused button's `anchor` rectangle; a move slides it there in 0.3 s with `1 - cos(t pi / 2)`
//! (`0x1411A0E70` type 3). The buttons switch the hand cursor off (`enableArrowCursor = false`).
//!
//! LOCAL (not traced): the frame at which the choice takes effect after the decide animation (the cursor's
//! `decide` label runs 16 frames), the sounds (the menus' cursor / decide static SE), the wrap of the
//! cursor, `<br>` of a hint string as a line break (the native text formatter `0x141580780` was not read;
//! the reference capture breaks the line there).

use std::collections::HashMap;
use std::sync::Arc;

use crate::camp::CampKey;
use crate::menu::{SE_MENU_CURSOR, SE_MENU_DECIDE};
use crate::player::Kind;
use crate::{Fonts, Movie, MovieDef, Pack, PackError, UiSound};

/// A button by its id in the key table at `0x1486F3D50`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameOverButton {
    Continue = 0,
    Load = 1,
    Retry = 2,
    PrepareRetry = 3,
    GameHelp = 4,
    Exit = 5,
    TitleScreen = 10,
    GiveUp = 11,
    /// The second "Prepare & Retry" of the table (id 12, menu types 13 - 15: the same text).
    PrepareRetry2 = 12,
}

impl GameOverButton {
    pub fn from_id(id: i32) -> Option<GameOverButton> {
        use GameOverButton::*;
        [Continue, Load, Retry, PrepareRetry, GameHelp, Exit, TitleScreen, GiveUp, PrepareRetry2].into_iter().find(|b| *b as i32 == id)
    }

    /// The string key (`0x140161560` fills the table in this order: ids 0..12).
    pub fn text_key(self) -> &'static str {
        match self {
            GameOverButton::Continue => "kh3_menu.gameover.menu_0001",
            GameOverButton::Load => "kh3_menu.gameover.menu_0004",
            GameOverButton::Retry => "kh3_menu.gameover.menu_0002",
            GameOverButton::PrepareRetry | GameOverButton::PrepareRetry2 => "kh3_menu.gameover.menu_0003",
            GameOverButton::GameHelp => "kh3_menu.gameover.menu_0005",
            GameOverButton::Exit => "kh3_menu.gameover.menu_0006",
            GameOverButton::TitleScreen => "kh3_menu.savepoint.menu_0005",
            GameOverButton::GiveUp => "kh3_menu.gameover.menu_0007",
        }
    }

    /// What the button asks of the host.
    pub fn result(self) -> GameOverResult {
        match self {
            GameOverButton::Continue => GameOverResult::Continue,
            GameOverButton::Load => GameOverResult::Load,
            GameOverButton::Retry => GameOverResult::Retry,
            GameOverButton::PrepareRetry | GameOverButton::PrepareRetry2 => GameOverResult::PrepareAndRetry,
            GameOverButton::GameHelp => GameOverResult::GameHelp,
            GameOverButton::Exit => GameOverResult::Exit,
            GameOverButton::TitleScreen => GameOverResult::TitleScreen,
            GameOverButton::GiveUp => GameOverResult::GiveUp,
        }
    }
}

/// What the player chose (the host carries it out).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameOverResult {
    Retry,
    PrepareAndRetry,
    Continue,
    Load,
    TitleScreen,
    GameHelp,
    Exit,
    GiveUp,
}

/// Table `0x146B37470`: the five button ids of each menu type (13 = none; 6..9 are the Gummi Ship's).
pub const MENU_TYPES: [[i32; 5]; 16] = [
    [0, 1, 10, 13, 13],
    [0, 4, 1, 10, 13],
    [2, 3, 0, 1, 13],
    [2, 4, 3, 0, 1],
    [2, 1, 5, 13, 13],
    [2, 4, 1, 5, 13],
    [2, 0, 1, 13, 13],
    [2, 4, 0, 1, 13],
    [2, 1, 13, 13, 13],
    [7, 8, 9, 13, 13],
    [6, 8, 9, 13, 13],
    [2, 3, 11, 13, 13],
    [2, 4, 3, 11, 13],
    [2, 12, 1, 13, 13],
    [2, 12, 0, 1, 13],
    [2, 12, 4, 0, 1],
];

/// The menu types by what they show (the chooser is `0x1415A2830` -> `0x141710B00` / `0x1417119A0` /
/// `0x1417118E0` [decompiled C]; docs/sora-gameover-spec.md 3.2). The `...Help` twin of a type (its
/// number + 1, with a Game Help entry) is taken when the game has help pages for the fight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameOverVariant {
    /// Type 0: Continue / Load / Title Screen (a death outside a retry battle).
    Field,
    /// Type 2: Retry / Prepare & Retry / Continue / Load (a retry battle: the reference capture).
    Battle,
    /// Type 6: Retry / Continue / Load (a retry battle where preparing is not offered).
    BattleNoPrepare,
    /// Type 4: Retry / Load / Exit.
    RetryExit,
    /// Type 8: Retry / Load.
    RetryLoad,
    /// Type 11: Retry / Prepare & Retry / Give up.
    GiveUp,
    /// Any row of [`MENU_TYPES`] by its number.
    Type(usize),
}

impl GameOverVariant {
    pub fn menu_type(self) -> usize {
        match self {
            GameOverVariant::Field => 0,
            GameOverVariant::Battle => 2,
            GameOverVariant::BattleNoPrepare => 6,
            GameOverVariant::RetryExit => 4,
            GameOverVariant::RetryLoad => 8,
            GameOverVariant::GiveUp => 11,
            GameOverVariant::Type(n) => n.min(MENU_TYPES.len() - 1),
        }
    }

    /// The buttons of the variant, top to bottom.
    pub fn buttons(self) -> Vec<GameOverButton> {
        MENU_TYPES[self.menu_type()].iter().filter_map(|id| GameOverButton::from_id(*id)).collect()
    }
}

const ROOT: &str = "gameOverMenu";
/// Frames from the decide press to the choice taking effect (the cursor's `decide` label: frames 127..142).
const DECIDE_FRAMES: u32 = 16;
/// The hint's text clip and its field (`CoTxGOverHint` = `tres.controls.TextFieldAlign`).
const HINT_FIELD: &str = "gameOverMenu.hintText.coText.textField";
/// The light cursor: the clip of the movie `Cursor`, attached to this movie's root.
const LIGHT_CURSOR: &str = "lightCursor";
/// `UTresUIP_Cursor` `0x14175AE50`: seconds of the cursor's slide to another anchor (0x3E99999A).
pub const CURSOR_SLIDE_SECONDS: f32 = 0.3;
/// Where the game-over clip sits on the stage and where its button set and first button sit in it
/// (the movie's own placements), and a button's `anchor` rectangle in the button (30 x 30 shape under
/// the matrix [15.2, 0, 0, 1.66667, -227.95, -25]).
const ANCHOR_RIGHT_TOP: (f32, f32) = (960.0 - 632.0 - 227.95 + 456.0, 540.0 + 261.0 - 25.0);
const BUTTON_PITCH: f32 = 50.0;

/// The game-over menu (`GameOverMenu::open(variant, hint)`; the older name `GameOverScreen` is this type).
pub struct GameOverMenu {
    pub movie: Movie,
    fonts: Fonts,
    strings: HashMap<String, String>,
    /// The movie `Cursor` (`None`: the pack has none: the menu works without the orb).
    cursor_def: Option<Arc<MovieDef>>,
    open: bool,
    /// The buttons shown, top to bottom (slot n = clip `coBtn<n + 1>`).
    buttons: Vec<GameOverButton>,
    /// Buttons the host cannot carry out (shown disabled; decide on one is refused).
    disabled: Vec<GameOverButton>,
    cursor: usize,
    decided: Option<(GameOverButton, u32)>,
    /// The light cursor's place (stage pixels) and its slide: (from, to, frames done).
    light: (f32, f32),
    slide: Option<((f32, f32), (f32, f32), u32)>,
}

/// The older name of [`GameOverMenu`].
pub type GameOverScreen = GameOverMenu;

impl GameOverMenu {
    pub fn load(pack: &mut Pack, fonts: Fonts) -> Result<GameOverMenu, PackError> {
        let def: Arc<MovieDef> = pack.movie("Pause")?;
        let strings = pack.string_table("kh3_menu");
        let cursor_def = pack.movie("Cursor").ok();
        Ok(GameOverMenu { movie: Movie::new(def), fonts, strings, cursor_def, open: false, buttons: Vec::new(), disabled: Vec::new(), cursor: 0, decided: None, light: (0.0, 0.0), slide: None })
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn buttons(&self) -> &[GameOverButton] {
        &self.buttons
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// A button was decided and its animation runs (the result comes from [`GameOverMenu::tick`]).
    pub fn deciding(&self) -> bool {
        self.decided.is_some()
    }

    fn string(&self, key: &str) -> String {
        self.strings.get(key).cloned().unwrap_or_default()
    }

    fn button_path(slot: usize) -> String {
        format!("{ROOT}.btnSet.coBtn{}", slot + 1)
    }

    /// OPENS THE MENU with the buttons of `variant` and the hint text (`None` or empty: the panel stays,
    /// as in the game, without a text). `<br>` in the hint is a line break.
    pub fn open(&mut self, variant: GameOverVariant, hint: Option<&str>) {
        self.open_type(variant.menu_type(), &[], hint.unwrap_or(""));
    }

    /// `0x1417035E0(uip, menu_type)`. `disabled`: buttons this host cannot carry out. `hint`: the hint
    /// text (empty: none).
    pub fn open_type(&mut self, menu_type: usize, disabled: &[GameOverButton], hint: &str) {
        let ids = MENU_TYPES[menu_type.min(MENU_TYPES.len() - 1)];
        self.buttons = ids.iter().filter_map(|id| GameOverButton::from_id(*id)).collect();
        self.disabled = disabled.to_vec();
        self.cursor = self.buttons.iter().position(|b| !self.disabled.contains(b)).unwrap_or(0);
        self.decided = None;
        self.movie.goto_and_play("", "loop");
        self.movie.goto_and_play(ROOT, "in");
        let title = self.string("kh3_menu.gameover.title_0001");
        let fonts = self.fonts.clone();
        self.movie.set_text(&format!("{ROOT}.txTitle.textField"), &title, &fonts);
        self.set_hint(hint);
        for slot in 0..5 {
            let path = Self::button_path(slot);
            match self.buttons.get(slot).copied() {
                Some(button) => {
                    let text = self.string(button.text_key());
                    self.movie.set_visible(&path, true);
                    self.movie.set_text(&format!("{path}.btnName.coName.textField"), &text, &fonts);
                }
                None => {
                    self.movie.set_visible(&path, false);
                }
            }
        }
        self.open = true;
        // The light cursor (`UTresUIP_Cursor`): placed at once on the first focus.
        if let Some(def) = self.cursor_def.clone() {
            self.movie.attach_first_child("", &def);
        }
        self.light = self.anchor_corner(self.cursor);
        self.slide = None;
        self.place_light();
        self.show_cursor();
    }

    /// `TextFieldAlign.updateText` with `alignH = alignV = "Center"` (the component's settings in
    /// `McGOverHintText`): `textField.x = (width - textWidth) / 2`, and `textField.y = (height -
    /// textHeight) / 2` unless the text scrolls (`maxScrollV > 1`: then y = 0). The field wraps at its
    /// width; its text is left aligned, so a block of lines is centred as a whole.
    pub fn set_hint(&mut self, hint: &str) {
        let text = hint.replace("<br>", "\n");
        let fonts = self.fonts.clone();
        self.movie.set_text(HINT_FIELD, &text, &fonts);
        self.movie.set_visible(&format!("{ROOT}.hintText"), !text.is_empty());
        let Some(node) = self.movie.node_mut(HINT_FIELD) else { return };
        let Kind::Text(field) = &node.kind else { return };
        let def = field.def.clone();
        let Some(font) = fonts.get(def.font_class.as_deref()).cloned() else { return };
        let (width, height) = (def.bounds.x1 - def.bounds.x0, def.bounds.y1 - def.bounds.y0);
        let wrapped = if def.word_wrap { crate::render::wrap_text(&fonts, &font, &text, def.size, width - 4.0) } else { text.clone() };
        let lines: Vec<&str> = wrapped.split('\n').collect();
        let text_width = lines.iter().map(|line| crate::render::markup_width(&fonts, &font, line, def.size)).fold(0.0, f32::max);
        let line_height = (font.ascent + font.descent + font.leading) * def.size / font.units_per_em;
        let text_height = lines.len() as f32 * line_height + (lines.len().max(1) - 1) as f32 * def.leading;
        let x = (width - text_width) * 0.5;
        let y = if text_height > height - 4.0 { 0.0 } else { (height - text_height) * 0.5 };
        // The field's own x / y are those of its top left corner: the authored matrix puts it at (0, 0).
        node.matrix.tx = -def.bounds.x0 + x;
        node.matrix.ty = -def.bounds.y0 + y;
        node.script_matrix = true;
    }

    /// The top right corner of a button's anchor rectangle on the stage: where the light cursor goes.
    fn anchor_corner(&self, slot: usize) -> (f32, f32) {
        (ANCHOR_RIGHT_TOP.0, ANCHOR_RIGHT_TOP.1 + BUTTON_PITCH * slot as f32)
    }

    fn place_light(&mut self) {
        let (x, y) = self.light;
        if let Some(node) = self.movie.node_mut(LIGHT_CURSOR) {
            (node.matrix.tx, node.matrix.ty) = (x, y);
            node.script_matrix = true;
            node.script_visible = Some(true);
        }
    }

    /// Where the light cursor is now (stage pixels; the orb itself wanders around this point).
    pub fn light_cursor(&self) -> (f32, f32) {
        self.light
    }

    /// Every button's state clip by the cursor.
    fn show_cursor(&mut self) {
        for slot in 0..self.buttons.len() {
            let path = Self::button_path(slot);
            let disabled = self.disabled.contains(&self.buttons[slot]);
            let label = match (slot == self.cursor, disabled) {
                (true, false) => "select",
                (true, true) => "disableSelect",
                (false, false) => "normal",
                (false, true) => "disable",
            };
            if self.movie.current_label(&path) != Some(label) {
                self.movie.goto_and_play(&path, label);
            }
            let cursor = format!("{path}.cursor");
            self.movie.set_visible(&cursor, slot == self.cursor);
            if slot == self.cursor {
                if self.movie.current_label(&cursor) != Some("loop") {
                    self.movie.goto_and_play(&cursor, "loop");
                }
            } else {
                self.movie.goto_and_stop(&cursor, "off");
            }
        }
    }

    /// A menu key (up / down / decide; the others do nothing: there is no way out of this menu but a button).
    pub fn key(&mut self, key: CampKey, repeat: bool, sounds: &mut Vec<UiSound>) {
        if !self.open || self.decided.is_some() || self.buttons.is_empty() {
            return;
        }
        match key {
            CampKey::Up | CampKey::Down => {
                if let Some(next) = crate::camp::move_wrapping(self.cursor, self.buttons.len(), key == CampKey::Down, repeat) {
                    self.cursor = next;
                    sounds.push(UiSound::Play(SE_MENU_CURSOR));
                    self.show_cursor();
                    self.slide = Some((self.light, self.anchor_corner(next), 0));
                }
            }
            CampKey::Decide => {
                let button = self.buttons[self.cursor];
                if self.disabled.contains(&button) {
                    sounds.push(UiSound::Play(crate::menu::SE_MENU_REFUSED));
                    return;
                }
                sounds.push(UiSound::Play(SE_MENU_DECIDE));
                let path = Self::button_path(self.cursor);
                self.movie.goto_and_play(&path, "decide");
                self.movie.goto_and_play(&format!("{path}.cursor"), "decide");
                self.decided = Some((button, DECIDE_FRAMES));
            }
            _ => {}
        }
    }

    /// One stage frame. The button chosen, once, when its decide animation is over (the menu then leaves).
    pub fn tick(&mut self) -> Option<GameOverButton> {
        if !self.open {
            return None;
        }
        self.movie.tick();
        self.movie.take_events();
        if let Some((from, to, done)) = self.slide {
            // `0x1411A0E70` type 3: 1 - cos(t pi / 2) over the slide's time.
            let frames = (CURSOR_SLIDE_SECONDS * 60.0).round() as u32;
            let done = done + 1;
            let t = (done as f32 / frames as f32).min(1.0);
            let k = 1.0 - (t * std::f32::consts::FRAC_PI_2).cos();
            self.light = (from.0 + (to.0 - from.0) * k, from.1 + (to.1 - from.1) * k);
            self.slide = (done < frames).then_some((from, to, done));
            if self.slide.is_none() {
                self.light = to;
            }
            self.place_light();
        }
        if let Some((button, left)) = self.decided.as_mut() {
            if *left > 0 {
                *left -= 1;
                return None;
            }
            let button = *button;
            self.close();
            return Some(button);
        }
        None
    }

    /// Takes the menu off at once (the host goes on: a respawn, a load).
    pub fn close(&mut self) {
        self.open = false;
        self.decided = None;
        self.slide = None;
        self.movie.detach("");
        self.movie.goto_and_stop(ROOT, "off");
        self.movie.goto_and_play("", "off");
    }
}
