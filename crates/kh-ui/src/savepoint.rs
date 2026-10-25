//! (savepoint) THE SAVE-POINT MENU (docs/sora-savepoint-spec.md section 5): the clip `menuSavePoint`
//! of the movie `17_saveload/SaveLoad` (`UTresUIP_SaveLoad`): the title plate "SAVE" and up to six
//! commands (`savePointObj.cmd1` .. `cmd6`, each a `tres.rsl.TopMenuCmd`, the camp menu's top
//! button), with their help lines.
//!
//! READ FROM THE ASSETS: the clip, its labels, the six command clips and their positions, the
//! strings `kh3_menu.savepoint.*` (title, six command names, six help lines).
//! LOCAL (the open function of `UTresUIP_SaveLoad` was not read): the ORDER of the six commands
//! (taken as the order of their help strings `detail_0001` .. `detail_0006`: Save, Select an Area,
//! World Map, Visit the Bistro, Tutorials, Title Screen), which clip label plays at open and close,
//! the 16 frames between the decide press and the choice (the cursor's `decide` label, as in the
//! game-over menu), the wrap of the cursor, the sounds (the menus' cursor / decide / close static SE).
//! In the game the entries Select an Area / World Map / Bistro are gated per save point by game
//! progress (`/Game/UI/Data/SaveMenuRestrictionData`); a host passes what it cannot carry out as
//! `disabled`.

use std::collections::HashMap;
use std::sync::Arc;

use crate::camp::CampKey;
use crate::menu::{SE_MENU_CLOSE, SE_MENU_CURSOR, SE_MENU_DECIDE, SE_MENU_REFUSED};
use crate::{Fonts, Movie, MovieDef, Pack, PackError, UiSound};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SavePointCommand {
    Save,
    SelectArea,
    WorldMap,
    Bistro,
    Tutorials,
    TitleScreen,
    /// (party P) NOT A COMMAND OF KH3'S MENU (the user's request 2026-10-15): "Party Formation" (who is in the active
    /// party, who leads). Its text is the port's ([`PARTY_FORMATION_TEXT`]), its icon the camp menu's Customize cell.
    PartyFormation,
    /// (SV) NOT A COMMAND OF KH3'S MENU (the user's request 2026-10-17: "Have a menu to load save data at save points"):
    /// "Load" leads to the Load screen (the slot list of the same movie). Its name and help line are KH3's own words of
    /// the title menu's LOAD entry ([`LOAD_TEXT`]), its icon cell [`LOAD_ICON`].
    Load,
}

/// (SV) The name and help line of "Load": the title menu's entry (`kh3_menu.title.menu_0003` "LOAD", written as the
/// save-point menu writes its entries) and KH3's help line of that entry ("Resume from the save data of your choice.").
pub const LOAD_TEXT: (&str, &str) = ("Load", "kh3_menu.title.detail_0003"); // (release) the help line: the game's text, by key (kh_gendata::text_or)
/// (SV) LOCAL: the category icon cell of "Load": `shopGm2` (an arrow going down into a vessel: data taken back in); KH3's
/// save-point menu has no Load entry, so no cell is drawn for one.
pub const LOAD_ICON: &str = "shopGm2";

/// (party P) The name and help line of "Party Formation" (LOCAL: KH3 has no such entry or string).
pub const PARTY_FORMATION_TEXT: (&str, &str) = ("Party Formation", "Choose who is in the party (up to three)\nand who leads.");

/// (party P) The clip has six command slots (`cmd1` .. `cmd6`); a seventh is placed as another `tres.rsl.TopMenuCmd`
/// one slot pitch below the sixth (LOCAL: KH3's menu never shows seven).
/// (SV) ... and an eighth one more pitch below (Load joined the menu: Save, Load, Select an Area, Pass Time, Respawn
/// Enemies, Talk to Melina, Party Formation, Title Screen).
pub const MAX_COMMANDS: usize = 8;

impl SavePointCommand {
    pub const ALL: [SavePointCommand; 6] = [SavePointCommand::Save, SavePointCommand::SelectArea, SavePointCommand::WorldMap, SavePointCommand::Bistro, SavePointCommand::Tutorials, SavePointCommand::TitleScreen];

    /// (IMPL_ER21) The label of the entry's icon in the category icon clip.
    pub fn icon(self) -> &'static str {
        match self {
            SavePointCommand::Save => "save1",
            SavePointCommand::SelectArea => "save2",
            SavePointCommand::WorldMap => "save3",
            SavePointCommand::Bistro => "save4",
            SavePointCommand::Tutorials => "save5",
            SavePointCommand::TitleScreen => "save6",
            // (party P) the camp menu's Customize cell (a figure with a cogwheel)
            SavePointCommand::PartyFormation => "camp4",
            SavePointCommand::Load => LOAD_ICON, // (SV)
        }
    }

    /// (name, help line) keys of table `kh3_menu`.
    pub fn text_keys(self) -> (&'static str, &'static str) {
        match self {
            SavePointCommand::Save => ("kh3_menu.savepoint.menu_0001", "kh3_menu.savepoint.detail_0001"),
            SavePointCommand::SelectArea => ("kh3_menu.savepoint.menu_0002", "kh3_menu.savepoint.detail_0002"),
            SavePointCommand::WorldMap => ("kh3_menu.savepoint.menu_0003", "kh3_menu.savepoint.detail_0003"),
            SavePointCommand::Bistro => ("kh3_menu.savepoint.menu_0004", "kh3_menu.savepoint.detail_0004"),
            SavePointCommand::Tutorials => ("kh3_menu.savepoint.menu_0006", "kh3_menu.savepoint.detail_0005"),
            SavePointCommand::TitleScreen => ("kh3_menu.savepoint.menu_0005", "kh3_menu.savepoint.detail_0006"),
            // (party P) no key: PARTY_FORMATION_TEXT
            SavePointCommand::PartyFormation => ("", ""),
            // (SV) no key: LOAD_TEXT
            SavePointCommand::Load => ("", ""),
        }
    }
}

/// What the menu told the host in a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SavePointResult {
    Chosen(SavePointCommand),
    /// Cancel: the menu is gone.
    Closed,
}

const ROOT: &str = "menuSavePoint";
const OTHER_MENUS: [&str; 5] = ["menuLoadClearData", "menuLoadData", "menuSaveData", "menuTutorial", "menuAreaSelect"];
const DECIDE_FRAMES: u32 = 16;

pub struct SavePointMenu {
    pub movie: Movie,
    fonts: Fonts,
    strings: HashMap<String, String>,
    open: bool,
    commands: Vec<SavePointCommand>,
    disabled: Vec<SavePointCommand>,
    cursor: usize,
    decided: Option<(SavePointCommand, u32)>,
    /// (saveload) The host's own texts of the last open (for [`SavePointMenu::reopen_back`]).
    texts: Vec<(SavePointCommand, String, String)>,
    /// (er-host, IMPL_ER21) "Title Screen" asks KH3's question first (`kh3_menu.savepoint.dialog_0004`: "Return to the Title
    /// Screen? All unsaved game progress will be lost.", Yes / No with the cursor on No) in the Dialog window, a layer
    /// of its own above the menu. Off by default (a host switches it on).
    pub confirm_title: bool,
    dialog: Movie,
    /// The question is up: the cursor (0 = Yes, 1 = No).
    asking: Option<usize>,
    dialog_closing: bool,
    system: HashMap<String, String>,
    /// (er-host, IMPL_ER26) A host's own icon label (of the category icon clip) for some commands; kept across opens.
    pub host_icons: Vec<(SavePointCommand, String)>,
    /// (er-host, IMPL_ER26) Commands that ask a CHOICE before they are carried out (the window of the Title Screen
    /// question, with the host's text and up to three buttons, cursor on the first; cancel = back to the menu). The
    /// button chosen: [`SavePointMenu::take_choice`]. Kept across opens.
    pub host_choices: Vec<HostChoice>,
    /// The choice window that is up (index into `host_choices`); `None` while the Title Screen question is.
    ask_choice: Option<usize>,
    choice: Option<usize>,
}

/// (er-host, IMPL_ER26) A command's choice window: its text and its buttons (at most [`CHOICE_BUTTONS`]).
#[derive(Clone, Debug, PartialEq)]
pub struct HostChoice {
    pub command: SavePointCommand,
    pub text: String,
    pub options: Vec<String>,
}

/// The Dialog window's buttons a choice can have (the first is the movie's own, the others are added instances).
pub const CHOICE_BUTTONS: usize = 3;
const BUTTON_NAMES: [&str; CHOICE_BUTTONS] = ["dialogButton", "dialogButton2", "dialogButton3"];
/// Stage pixels between two buttons of the window (as the two-button questions of the other screens place theirs).
const BUTTON_PITCH: f32 = 35.0;
/// Stage pixels from the text holder down to the first of three buttons (LOCAL: by eye, the block centred in the window).
const THREE_TOP: f32 = 12.0;

impl SavePointMenu {
    pub fn load(pack: &mut Pack, fonts: Fonts) -> Result<SavePointMenu, PackError> {
        let def: Arc<MovieDef> = pack.movie_linked("SaveLoad", &crate::camp_menu::LIBRARIES)?;
        let strings = pack.string_table("kh3_menu");
        let dialog = pack.movie_linked("Dialog", &crate::camp_menu::LIBRARIES)?;
        let system = pack.string_table("kh3_system");
        Ok(SavePointMenu { movie: Movie::new(def), fonts, strings, open: false, commands: Vec::new(), disabled: Vec::new(), cursor: 0, decided: None, texts: Vec::new(), confirm_title: false, dialog: Movie::new(dialog), asking: None, dialog_closing: false, system, host_icons: Vec::new(), host_choices: Vec::new(), ask_choice: None, choice: None })
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn commands(&self) -> &[SavePointCommand] {
        &self.commands
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    fn string(&self, key: &str) -> String {
        self.strings.get(key).cloned().unwrap_or_default()
    }

    /// (er-host, IMPL_ER21) The Dialog movie while the Title Screen question is up (or leaving).
    pub fn dialog_movie(&self) -> Option<&Movie> {
        (self.asking.is_some() || self.dialog_closing).then_some(&self.dialog)
    }

    /// The question's window: text, Yes / No, the cursor.
    fn show_question(&mut self, cursor: usize) {
        let fonts = self.fonts.clone();
        // (IMPL_ER26) a host's choice window uses the same window with its own text and buttons
        let (text, labels): (String, Vec<String>) = match self.ask_choice.and_then(|k| self.host_choices.get(k)) {
            Some(choice) => (choice.text.clone(), choice.options.iter().take(CHOICE_BUTTONS).cloned().collect()),
            None => (self.string("kh3_menu.savepoint.dialog_0004").replace("<br>", "\n"), vec![self.system.get("kh3_system.general.choice_0001").cloned().unwrap_or_else(|| "Yes".into()), self.system.get("kh3_system.general.choice_0002").cloned().unwrap_or_else(|| "No".into())]),
        };
        if self.asking.is_none() {
            self.dialog.goto_and_play("dialog", "in");
            for name in &BUTTON_NAMES[1..] {
                self.dialog.add_instance("", "DialogButton", name);
            }
            self.dialog.set_visible("dialogText", labels.len() <= 2);
            self.dialog.set_text("dialogText.textField", &text, &fonts);
        }
        self.dialog_closing = false;
        self.asking = Some(cursor);
        for (n, name) in BUTTON_NAMES.into_iter().enumerate() {
            self.dialog.set_visible(name, n < labels.len());
            if let Some(label) = labels.get(n) {
                self.dialog.set_text(&format!("{name}.dialogBtnText.textField"), label, &fonts);
                self.dialog.goto_and_play(name, if cursor == n { "select" } else { "normal" });
            }
        }
        self.place_question();
    }

    /// (IMPL_ER26) How many buttons the window that is up has.
    fn question_buttons(&self) -> usize {
        self.ask_choice.and_then(|k| self.host_choices.get(k)).map_or(2, |c| c.options.len().clamp(1, CHOICE_BUTTONS))
    }

    /// (er-host, IMPL_ER26) The button chosen in a host's choice window, once (it goes with the `Chosen` result of its command).
    pub fn take_choice(&mut self) -> Option<usize> {
        self.choice.take()
    }

    /// (er-host, IMPL_ER26) The choice window that is up: (command, cursor).
    pub fn asking_choice(&self) -> Option<(SavePointCommand, usize)> {
        Some((self.host_choices.get(self.ask_choice?)?.command, self.asking?))
    }

    /// The text and the buttons sit at the window's position holders (as the camp menu's dialogs do).
    fn place_question(&mut self) {
        let holder = "dialog.dialogAnm.dialogPoAll.dialogPoAll2";
        let stage = |movie: &Movie, path: &str| -> Option<(f32, f32)> {
            let mut m = crate::Matrix::IDENTITY;
            let mut at = String::new();
            for part in path.split('.') {
                at = if at.is_empty() { part.to_string() } else { format!("{at}.{part}") };
                m = m.mul(&movie.node(&at)?.matrix);
            }
            Some((m.tx, m.ty))
        };
        let mut text_y = None;
        if let Some((x, y)) = stage(&self.dialog, &format!("{holder}.poDialogText")) {
            self.dialog.set_position("dialogText", x, y, 0.0);
            text_y = Some(y);
        }
        if let Some((x, y)) = stage(&self.dialog, &format!("{holder}.poDialogButton")) {
            // (IMPL_ER26) the window is as tall as a line of text and two buttons: a choice of three has no text line (its
            // options say it) and its buttons start where the text would stand
            let top = if self.question_buttons() > 2 { text_y.map_or(y - BUTTON_PITCH, |t| t + THREE_TOP) } else { y };
            for (n, name) in BUTTON_NAMES.into_iter().enumerate() {
                self.dialog.set_position(name, x, top + BUTTON_PITCH * n as f32, 0.0);
            }
        }
    }

    fn hide_question(&mut self) {
        if self.asking.take().is_some() {
            self.ask_choice = None;
            self.dialog.goto_and_play("dialog", "out");
            self.dialog.set_visible("dialogText", false);
            for name in BUTTON_NAMES {
                self.dialog.set_visible(name, false);
            }
            self.dialog_closing = true;
        }
    }

    /// The question is up (the menu's keys go to it).
    pub fn asking(&self) -> bool {
        self.asking.is_some()
    }

    fn command_path(slot: usize) -> String {
        format!("{ROOT}.savePointObj.cmd{}", slot + 1)
    }

    /// Opens the menu with `commands` from the top; `disabled` ones are shown greyed and refuse.
    pub fn open(&mut self, commands: &[SavePointCommand], disabled: &[SavePointCommand]) {
        self.texts.clear();
        self.commands = commands.iter().copied().take(MAX_COMMANDS).collect();
        // (party P) a seventh entry: its slot clip, made once, one slot pitch below the sixth
        // (SV) ... and an eighth below that, the same way
        for slot in 6..MAX_COMMANDS {
            if self.commands.len() <= slot || self.movie.exists(&Self::command_path(slot)) {
                continue;
            }
            let obj = format!("{ROOT}.savePointObj");
            let (name, from) = (format!("cmd{}", slot + 1), "cmd6"); // (the authored sixth is the model of every added one)
            if self.movie.add_instance(&obj, "tres.rsl.TopMenuCmd", &name) || self.movie.clone_child(&obj, from, &name) {
                let pos = |m: &Movie, p: &str| m.node(p).map(|n| (n.matrix.tx, n.matrix.ty));
                if let (Some(a), Some(b)) = (pos(&self.movie, &Self::command_path(4)), pos(&self.movie, &Self::command_path(5))) {
                    let k = (slot - 5) as f32;
                    self.movie.set_position(&Self::command_path(slot), b.0 + (b.0 - a.0) * k, b.1 + (b.1 - a.1) * k, 0.0);
                }
            } else {
                self.commands.truncate(slot);
                break;
            }
        }
        self.disabled = disabled.to_vec();
        self.cursor = self.commands.iter().position(|c| !self.disabled.contains(c)).unwrap_or(0);
        self.decided = None;
        (self.ask_choice, self.choice) = (None, None);
        self.movie.goto_and_play("", "in");
        for other in OTHER_MENUS {
            self.movie.set_visible(other, false);
        }
        self.movie.set_visible(ROOT, true);
        self.movie.goto_and_play(ROOT, "in");
        self.movie.set_visible(&format!("{ROOT}.savePointObj"), true);
        let fonts = self.fonts.clone();
        self.movie.goto_and_play(&format!("{ROOT}.stageCategory"), "in");
        let title = self.string("kh3_menu.savepoint.title_0001");
        self.movie.set_text(&format!("{ROOT}.stageCategory.category.categoryName.txName"), &title, &fonts);
        for slot in 0..MAX_COMMANDS {
            let path = Self::command_path(slot);
            if slot >= 6 && !self.movie.exists(&path) {
                continue;
            }
            match self.commands.get(slot).copied() {
                Some(command) => {
                    let (name, detail) = command.text_keys();
                    // (IMPL_ER21) the help lines' line breaks; the entry's own icon (the category icon's `save1` ..
                    // `save6`) [LOCAL: taken in the order of the commands; save6 is the return arrow of Title Screen]
                    let (name, detail) = (self.string(name), self.string(detail).replace("<br>", "\n"));
                    // (party P) Party Formation's own text
                    let (name, detail) = if command == SavePointCommand::PartyFormation { (PARTY_FORMATION_TEXT.0.to_string(), PARTY_FORMATION_TEXT.1.to_string()) } else { (name, detail) };
                    // (SV) Load's own text
                    let (name, detail) = if command == SavePointCommand::Load { (LOAD_TEXT.0.to_string(), LOAD_TEXT.1.to_string()) } else { (name, detail) };
                    self.movie.set_visible(&path, true);
                    // (IMPL_ER26) ... or the host's icon for it
                    let icon = self.host_icons.iter().find(|i| i.0 == command).map_or(command.icon(), |i| i.1.as_str()).to_string();
                    self.movie.goto_and_stop(&format!("{path}.categoryIcon"), &icon);
                    self.movie.set_text(&format!("{path}.mainCmdNameAnm.mainCmdName"), &name, &fonts);
                    self.movie.set_text(&format!("{path}.helpMessageAnm.helpMessage"), &detail, &fonts);
                }
                None => {
                    self.movie.set_visible(&path, false);
                }
            }
        }
        self.open = true;
        self.show_cursor(None);
    }

    /// (er9) As [`SavePointMenu::open`], for a host whose commands mean something of its own: `texts` gives
    /// other (name, help line) texts for some of the commands; the rest keep the game's strings.
    pub fn open_with_texts(&mut self, commands: &[SavePointCommand], disabled: &[SavePointCommand], texts: &[(SavePointCommand, &str, &str)]) {
        self.open(commands, disabled);
        self.texts = texts.iter().map(|(c, name, detail)| (*c, name.to_string(), detail.to_string())).collect();
        let fonts = self.fonts.clone();
        for slot in 0..self.commands.len() {
            let command = self.commands[slot];
            if let Some((_, name, detail)) = texts.iter().find(|t| t.0 == command) {
                let path = Self::command_path(slot);
                self.movie.set_text(&format!("{path}.mainCmdNameAnm.mainCmdName"), name, &fonts);
                self.movie.set_text(&format!("{path}.helpMessageAnm.helpMessage"), detail, &fonts);
            }
        }
    }

    /// (saveload) THE MENU COMES BACK from the Save list (0x1416F7F00: the clip's `inBack`): the commands,
    /// disabled ones and texts of the last open, the cursor on Save.
    pub fn reopen_back(&mut self) {
        self.reopen_back_at(SavePointCommand::Save);
    }

    /// (SV) As [`SavePointMenu::reopen_back`], the cursor on `command` (the Load screen's cancel: on Load).
    pub fn reopen_back_at(&mut self, command: SavePointCommand) {
        let (commands, disabled, texts) = (self.commands.clone(), self.disabled.clone(), self.texts.clone());
        let texts: Vec<(SavePointCommand, &str, &str)> = texts.iter().map(|(c, name, detail)| (*c, name.as_str(), detail.as_str())).collect();
        self.open_with_texts(&commands, &disabled, &texts);
        self.movie.goto_and_play(ROOT, "inBack");
        if let Some(at) = self.commands.iter().position(|c| *c == command) {
            let left = self.cursor;
            self.cursor = at;
            self.show_cursor(Some(left));
        }
    }

    /// Every command's state clip by the cursor (`tres.rsl.TopMenuCmd`: normal / select / selectOut /
    /// disable / disableSelect / disableSelectOut).
    fn show_cursor(&mut self, left: Option<usize>) {
        for slot in 0..self.commands.len() {
            let path = Self::command_path(slot);
            let disabled = self.disabled.contains(&self.commands[slot]);
            let label = match (slot == self.cursor, Some(slot) == left, disabled) {
                (true, _, false) => "select",
                (true, _, true) => "disableSelect",
                (false, true, false) => "selectOut",
                (false, true, true) => "disableSelectOut",
                (false, false, false) => "normal",
                (false, false, true) => "disable",
            };
            let settled = matches!((label, self.movie.current_label(&path)), ("normal", Some("selectOut")) | ("disable", Some("disableSelectOut")));
            if self.movie.current_label(&path) != Some(label) && !settled {
                self.movie.goto_and_play(&path, label);
            }
        }
    }

    pub fn key(&mut self, key: CampKey, repeat: bool, sounds: &mut Vec<UiSound>) -> Option<SavePointResult> {
        if !self.open || self.decided.is_some() || self.commands.is_empty() {
            return None;
        }
        // (er-host, IMPL_ER21) the Title Screen question: any direction moves between Yes and No; Yes goes on to the
        // command's decide, No / cancel takes the window away
        // (IMPL_ER26) a host's choice window: up / down move through its buttons (wrapping), A takes the one under the
        // cursor and goes on to the command's decide, B takes the window away (back in the menu)
        if let (Some(cursor), Some(_)) = (self.asking, self.ask_choice) {
            let n = self.question_buttons();
            match key {
                CampKey::Up | CampKey::Left => {
                    sounds.push(UiSound::Play(SE_MENU_CURSOR));
                    self.show_question((cursor + n - 1) % n);
                }
                CampKey::Down | CampKey::Right => {
                    sounds.push(UiSound::Play(SE_MENU_CURSOR));
                    self.show_question((cursor + 1) % n);
                }
                CampKey::Decide => {
                    sounds.push(UiSound::Play(SE_MENU_DECIDE));
                    self.hide_question();
                    self.choice = Some(cursor);
                    self.movie.goto_and_play(&Self::command_path(self.cursor), "decide");
                    self.decided = Some((self.commands[self.cursor], DECIDE_FRAMES));
                }
                CampKey::Cancel | CampKey::Menu => {
                    sounds.push(UiSound::Play(SE_MENU_CLOSE));
                    self.hide_question();
                }
                _ => {}
            }
            return None;
        }
        if let Some(cursor) = self.asking {
            match key {
                CampKey::Up | CampKey::Down | CampKey::Left | CampKey::Right => {
                    sounds.push(UiSound::Play(SE_MENU_CURSOR));
                    self.show_question(cursor ^ 1);
                }
                CampKey::Decide if cursor == 0 => {
                    sounds.push(UiSound::Play(SE_MENU_DECIDE));
                    self.hide_question();
                    self.movie.goto_and_play(&Self::command_path(self.cursor), "decide");
                    self.decided = Some((self.commands[self.cursor], DECIDE_FRAMES));
                }
                CampKey::Decide | CampKey::Cancel | CampKey::Menu => {
                    sounds.push(UiSound::Play(SE_MENU_CLOSE));
                    self.hide_question();
                }
                _ => {}
            }
            return None;
        }
        match key {
            CampKey::Up | CampKey::Down => {
                if let Some(next) = crate::camp::move_wrapping(self.cursor, self.commands.len(), key == CampKey::Down, repeat) {
                    let left = self.cursor;
                    self.cursor = next;
                    sounds.push(UiSound::Play(SE_MENU_CURSOR));
                    self.show_cursor(Some(left));
                }
            }
            CampKey::Decide => {
                let command = self.commands[self.cursor];
                if self.disabled.contains(&command) {
                    sounds.push(UiSound::Play(SE_MENU_REFUSED));
                    return None;
                }
                sounds.push(UiSound::Play(SE_MENU_DECIDE));
                if command == SavePointCommand::TitleScreen && self.confirm_title {
                    self.show_question(1);
                    return None;
                }
                // (IMPL_ER26) a command with a choice of the host's
                if let Some(k) = self.host_choices.iter().position(|c| c.command == command && !c.options.is_empty()) {
                    self.ask_choice = Some(k);
                    self.show_question(0);
                    return None;
                }
                self.movie.goto_and_play(&Self::command_path(self.cursor), "decide");
                self.decided = Some((command, DECIDE_FRAMES));
            }
            CampKey::Cancel | CampKey::Menu => {
                sounds.push(UiSound::Play(SE_MENU_CLOSE));
                self.close();
                return Some(SavePointResult::Closed);
            }
            _ => {}
        }
        None
    }

    /// One stage frame. The command chosen, once, when its decide animation is over (the menu then leaves).
    pub fn tick(&mut self) -> Option<SavePointResult> {
        if !self.open {
            return None;
        }
        self.movie.tick();
        self.movie.take_events();
        if self.asking.is_some() || self.dialog_closing {
            self.dialog.tick();
            self.dialog.take_events();
            if self.asking.is_some() {
                self.place_question();
            } else if self.dialog.current_label("dialog") == Some("off") {
                self.dialog_closing = false;
            }
        }
        if let Some((command, left)) = self.decided.as_mut() {
            if *left > 0 {
                *left -= 1;
                return None;
            }
            let command = *command;
            self.close();
            return Some(SavePointResult::Chosen(command));
        }
        None
    }

    /// Takes the menu off at once.
    pub fn close(&mut self) {
        self.open = false;
        self.decided = None;
        (self.asking, self.dialog_closing, self.ask_choice) = (None, false, None);
        self.dialog.goto_and_stop("dialog", "off");
        self.movie.goto_and_stop(ROOT, "off");
        self.movie.goto_and_play("", "off");
    }
}
