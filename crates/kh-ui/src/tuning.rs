//! (THE USER 2026-11-02: "Re-add Grave Gloveworts and Ghost Gloveworts ... make them work completely with our KH3 systems";
//! DECIDED BY THE USER the same day: "Roderika's spirit tuning opens a KH3-STYLE SCREEN, not Elden Ring's menu") THE SPIRIT
//! TUNING SCREEN that stands in for a host's spirit-ash upgrade (the Elden Ring mod: Roderika's "Spirit Tuning", her talk
//! script's spirit-tuning window). The rows, costs and effects are the host's (kh-er spirittune.rs reads them from the game's
//! own params at run time); this file is the screen: [`TuneMenu`] the logic, [`TuneView`] the display.
//!
//! WHAT IS KH3'S [A]: the camp menu's movie (`15_campmenu/CampMenu`) on its Items screen ("backyard": the list widget of
//! `ItemListLibrary`, the item status pane `ItemStatusA1` with a picture and a text, the frame with the munny counter, the
//! title strip with a category icon (`shop3`, the Workshop's), the help line, the button guide plate), the `Dialog` movie for
//! the Yes / No, and KH3'S OWN KEYBLADE FORGE TEXTS (`kh3_menu.synthesis.*`: item_0010 / item_0040 / item_0030, the question
//! dialog_0220, the refusal dialog_0210, the help detail_0210; `kh3_menu.shop.dialog_0020` the munny refusal;
//! `kh3_system.general.choice_0001 / 0002` Yes / No; ALL READ FROM THE PLAYER'S PACK by id, never compiled in: KH3TEXT
//! 2026-11-08, an id the pack lacks shows as the id), the menu sounds (static SE 0 cursor, 1 decide, 2 open, 3 close, 4
//! refused, 33 the item-get cue on an upgrade).
//! WHAT IS COMPOSED HERE [L]: the title "SPIRIT TUNING", the level in the row's count column ("+3", "MAX"), the column's
//! "Level", the pane's lines (the next level, the materials needed and held, the munny, what the level gives), the highest
//! level's refusal (KH3's materials_0010 line for a Keyblade, read from the pack, with "spirit ash" in its place).
//!
//! THE PAD (Xbox layout of the menus; the keyboard reaches the same keys through the host): D-pad / stick up and down the
//! list (LB / RB a page), A power up (KH3's question, Yes / No), B or Menu back: from the question to the list, from the list
//! the screen closes (the host's talk goes on).

use std::collections::HashMap;
use std::sync::Arc;

use crate::camp::CampKey;
use crate::list::{ListView, RowState};
use crate::menu::{SE_MENU_CLOSE, SE_MENU_CURSOR, SE_MENU_DECIDE, SE_MENU_REFUSED};
use crate::shop::{grouped, stamp_of, SE_TRADE};
use crate::{Fonts, Movie, MovieDef, Pack, PackError, UiSound};

/// What one level costs: a KH item (the glovewort the game asks for, as the host maps it) and how many; what he holds.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TuneCost {
    pub key: String,
    pub name: String,
    pub count: u32,
    pub owned: u32,
}

/// One spirit ash he holds.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TuneRow {
    /// The host's handle of the ash (Elden Ring: its goods id at the current level).
    pub id: u32,
    /// The picture's id for the item art (Elden Ring: `HOSTKEY_<n>.icon<iconId>`).
    pub icon: String,
    /// Its name, without the level.
    pub name: String,
    pub level: u8,
    pub max_level: u8,
    /// The next level's materials (`None`: it is at its highest).
    pub cost: Option<TuneCost>,
    /// The next level's munny.
    pub munny: u32,
    /// What the next level gives, one line each ("HP  x1.81 -> x2.15").
    pub gives: Vec<String>,
}

impl TuneRow {
    pub fn maxed(&self) -> bool {
        self.cost.is_none() || self.level >= self.max_level
    }
}

/// What the host shows.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TuneData {
    /// The tuner's name (the frame's area line).
    pub title: String,
    pub munny: i64,
    pub rows: Vec<TuneRow>,
}

/// What the screen asks of the host.
#[derive(Clone, Debug, PartialEq)]
pub enum TuneEvent {
    /// Yes on the question: power the ash up one level (the host checks again and does it).
    Upgrade { id: u32, to_level: u8 },
    /// B / Menu on the list: the screen closes (its `out` follows).
    Closed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TuneLayer {
    List,
    Confirm,
}

/// Why a row cannot be powered up now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TuneRefused {
    Max,
    Materials,
    Munny,
}

/// KH3's texts the logic and the view use, set by the view from the player's pack. KH3's text is never compiled in
/// (KH3TEXT 2026-11-08): the defaults are the texts' ids ([`TuneTexts::MAX`] ..), shown only if the pack lacks them.
#[derive(Clone, Debug, PartialEq)]
pub struct TuneTexts {
    pub max: String,
    pub materials: String,
    pub munny: String,
    /// "Power up the {0}?"
    pub question: String,
    /// "Power up the {0}." (the help line)
    pub help: String,
}

impl TuneTexts {
    /// KH3's Keyblade Forge line for a Keyblade at its highest level ("Keyblade" -> "spirit ash" by the view).
    pub const MAX: &'static str = "kh3_menu.synthesis.materials_0010";
    /// KH3's refusal for missing materials.
    pub const MATERIALS: &'static str = "kh3_menu.synthesis.dialog_0210";
    /// KH3's shop refusal for too little munny.
    pub const MUNNY: &'static str = "kh3_menu.shop.dialog_0020";
    /// KH3's Keyblade Forge question (with {0}).
    pub const QUESTION: &'static str = "kh3_menu.synthesis.dialog_0220";
    /// KH3's Keyblade Forge help line (with {0}).
    pub const HELP: &'static str = "kh3_menu.synthesis.detail_0210";
}

impl Default for TuneTexts {
    /// The ids themselves (what shows when the pack has none of them).
    fn default() -> TuneTexts {
        TuneTexts { max: TuneTexts::MAX.into(), materials: TuneTexts::MATERIALS.into(), munny: TuneTexts::MUNNY.into(), question: TuneTexts::QUESTION.into(), help: TuneTexts::HELP.into() }
    }
}

pub struct TuneMenu {
    pub data: TuneData,
    pub list: ListView,
    pub layer: TuneLayer,
    /// The question's cursor: 0 Yes, 1 No.
    pub confirm: usize,
    /// The help line's message after a refusal or an upgrade (cleared by the next move).
    pub notice: Option<String>,
    pub texts: TuneTexts,
    events: Vec<TuneEvent>,
}

/// The shown name with its level ("Lone Wolf Ashes +3"; +0 shows the plain name, as Elden Ring does).
pub fn leveled(name: &str, level: u8) -> String {
    if level == 0 { name.to_string() } else { format!("{name} +{level}") }
}

impl TuneMenu {
    pub fn new(data: TuneData) -> TuneMenu {
        let mut menu = TuneMenu { data, list: ListView::new(crate::shop::LIST, crate::shop::LIST_ITEM), layer: TuneLayer::List, confirm: 0, notice: None, texts: TuneTexts::default(), events: Vec::new() };
        let n = menu.data.rows.len();
        menu.list.set_count(n);
        menu
    }

    pub fn focused(&self) -> Option<&TuneRow> {
        usize::try_from(self.list.focus).ok().and_then(|i| self.data.rows.get(i))
    }

    /// The host's data again (every frame; after an upgrade the ash's id changes): the focused row is kept by its place.
    pub fn set_data(&mut self, data: TuneData) {
        if data == self.data {
            return;
        }
        let focus = self.list.focus;
        self.data = data;
        let n = self.data.rows.len();
        self.list.set_count(n);
        if focus >= 0 && n > 0 {
            self.list.set_focus((focus as usize).min(n - 1));
        }
        self.list.invalidate();
        if self.layer == TuneLayer::Confirm && self.focused().is_none_or(|r| self.refusal(r).is_some()) {
            self.layer = TuneLayer::List;
        }
    }

    /// Why the row cannot be powered up (None: it can).
    pub fn refusal(&self, row: &TuneRow) -> Option<TuneRefused> {
        match &row.cost {
            _ if row.maxed() => Some(TuneRefused::Max),
            Some(c) if c.owned < c.count => Some(TuneRefused::Materials),
            _ if self.data.munny < row.munny as i64 => Some(TuneRefused::Munny),
            _ => None,
        }
    }

    fn refusal_text(&self, r: TuneRefused) -> String {
        match r {
            TuneRefused::Max => self.texts.max.clone(),
            TuneRefused::Materials => self.texts.materials.clone(),
            TuneRefused::Munny => self.texts.munny.clone(),
        }
    }

    /// A MENU KEY.
    pub fn key(&mut self, key: CampKey, repeat: bool, sounds: &mut Vec<UiSound>) {
        match self.layer {
            TuneLayer::List => match key {
                CampKey::Up | CampKey::Down => {
                    if self.list.move_anchor(key == CampKey::Down, repeat) {
                        sounds.push(UiSound::Play(SE_MENU_CURSOR));
                        self.notice = None;
                    }
                }
                CampKey::L1 | CampKey::R1 => {
                    let moved = if key == CampKey::R1 { self.list.next_page(repeat) } else { self.list.prev_page(repeat) };
                    if moved {
                        sounds.push(UiSound::Play(SE_MENU_CURSOR));
                        self.notice = None;
                    }
                }
                CampKey::Decide if !repeat => {
                    let Some(row) = self.focused().cloned() else {
                        sounds.push(UiSound::Play(SE_MENU_REFUSED));
                        return;
                    };
                    match self.refusal(&row) {
                        Some(r) => {
                            sounds.push(UiSound::Play(SE_MENU_REFUSED));
                            self.notice = Some(self.refusal_text(r));
                        }
                        None => {
                            sounds.push(UiSound::Play(SE_MENU_DECIDE));
                            self.notice = None;
                            self.confirm = 0;
                            self.layer = TuneLayer::Confirm;
                        }
                    }
                }
                CampKey::Cancel | CampKey::Menu if !repeat => {
                    sounds.push(UiSound::Play(SE_MENU_CLOSE));
                    self.events.push(TuneEvent::Closed);
                }
                _ => {}
            },
            TuneLayer::Confirm => match key {
                CampKey::Up | CampKey::Down | CampKey::Left | CampKey::Right => {
                    self.confirm = 1 - self.confirm.min(1);
                    sounds.push(UiSound::Play(SE_MENU_CURSOR));
                }
                CampKey::Decide if !repeat && self.confirm == 0 => self.upgrade(sounds),
                // No / B / Menu: back to the list
                CampKey::Decide | CampKey::Cancel | CampKey::Menu if !repeat => {
                    sounds.push(UiSound::Play(SE_MENU_CLOSE));
                    self.layer = TuneLayer::List;
                }
                _ => {}
            },
        }
    }

    /// YES: the upgrade is asked of the host and shown at once (the host's next data is the truth: the level, the stock,
    /// the munny).
    fn upgrade(&mut self, sounds: &mut Vec<UiSound>) {
        self.layer = TuneLayer::List;
        let Some(row) = self.focused().cloned() else { return };
        if self.refusal(&row).is_some() {
            sounds.push(UiSound::Play(SE_MENU_REFUSED));
            return;
        }
        let to_level = row.level + 1;
        self.events.push(TuneEvent::Upgrade { id: row.id, to_level });
        sounds.push(UiSound::Play(SE_TRADE));
        self.data.munny -= row.munny as i64;
        let k = self.list.focus as usize;
        if let Some(r) = self.data.rows.get_mut(k) {
            r.level = to_level;
            if let Some(c) = r.cost.as_mut() {
                c.owned = c.owned.saturating_sub(c.count);
            }
        }
        self.notice = Some(format!("<color(strong)>{}<color>", leveled(&row.name, to_level)));
        self.list.invalidate();
    }

    pub fn take_events(&mut self) -> Vec<TuneEvent> {
        std::mem::take(&mut self.events)
    }
}

// ================================================================================================= the display

const BACKYARD: &str = "backyard.backyardAnm";
const ITEM_ROOT: &str = "backyard.backyardAnm.backyardItem";
const FRAME: &str = "stageFrame.stageObjR";
const GUIDE: &str = "stageFrame.stageObjR.btnInfo.btnInfoText.txText.textField";
const PANE: &str = "backyard.backyardAnm.itemStatusA1";
/// The category icon: KH3's Workshop (where its Keyblades are powered up).
const CATEGORY_ICON: &str = "shop3";

/// The pane's text of a row (the pane's field holds three lines): the materials needed, what he holds of them and the
/// munny, then what the next level gives (the host's lines joined); at the highest level its name, MAX and what it gives.
pub fn pane_text(row: &TuneRow, stock_word: &str, needed_word: &str) -> String {
    let gives = row.gives.join("  ");
    let mut lines = Vec::new();
    match row.cost.as_ref().filter(|_| !row.maxed()) {
        Some(c) => {
            lines.push(format!("<color(strong)>{needed_word}<color>  {} x {}", c.name, c.count));
            let munny = if row.munny > 0 { format!("   {} MUNNY", grouped(row.munny as i64)) } else { String::new() };
            lines.push(format!("<color(strong)>{stock_word}<color> {}{munny}   <color(strong)>+{} -> +{}<color>", c.owned, row.level, row.level + 1));
        }
        None => lines.push(format!("<color(strong)>{}<color>  MAX", leveled(&row.name, row.level))),
    }
    if !gives.is_empty() {
        lines.push(gives);
    }
    lines.join("\n")
}

pub struct TuneView {
    pub movie: Movie,
    pub dialog: Movie,
    pub fonts: Fonts,
    strings: HashMap<String, String>,
    pub art: crate::item_art::ItemArt,
    open: bool,
    closing: bool,
    age: u32,
    stamp: u64,
    guide: String,
    pane_item: Option<Option<String>>,
    dialog_open: bool,
    dialog_closing: bool,
    dialog_stamp: u64,
}

impl TuneView {
    pub fn load(pack: &mut Pack, fonts: Fonts) -> Result<TuneView, PackError> {
        let def: Arc<MovieDef> = pack.movie_linked("CampMenu", &crate::camp_menu::LIBRARIES)?;
        let dialog = pack.movie_linked("Dialog", &crate::camp_menu::LIBRARIES)?;
        let mut strings = HashMap::new();
        for table in ["kh3_menu", "kh3_system", "kh3_camp"] {
            strings.extend(pack.string_table(table));
        }
        let art = crate::item_art::ItemArt::load(pack);
        Ok(TuneView { movie: Movie::new(def), dialog: Movie::new(dialog), fonts, strings, art, open: false, closing: false, age: 0, stamp: 0, guide: String::new(), pane_item: None, dialog_open: false, dialog_closing: false, dialog_stamp: 0 })
    }

    /// A string of the pack, `<br>` as a line break; `fallback` when the pack has none.
    pub fn string(&self, key: &str, fallback: &str) -> String {
        self.strings.get(key).map(|s| s.replace("<br>", "\n")).filter(|s| !s.is_empty()).unwrap_or_else(|| fallback.to_string())
    }

    /// A string of the pack by its id (`<br>` as a line break); the id itself when the pack has none (KH3's text is never
    /// compiled in).
    pub fn string_id(&self, key: &str) -> String {
        self.string(key, key)
    }

    /// KH3's texts for the logic (the Keyblade Forge's, the shop's munny line), from the pack.
    pub fn texts(&self) -> TuneTexts {
        let max = self.strings.get(TuneTexts::MAX).filter(|s| !s.is_empty()).map_or_else(|| TuneTexts::MAX.to_string(), |s| s.replace("Keyblade", "spirit ash"));
        TuneTexts { max, materials: self.string_id(TuneTexts::MATERIALS), munny: self.string_id(TuneTexts::MUNNY), question: self.string_id(TuneTexts::QUESTION), help: self.string_id(TuneTexts::HELP) }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn dialog_movie(&self) -> Option<&Movie> {
        self.dialog_open.then_some(&self.dialog)
    }

    fn text(&mut self, path: &str, text: &str) {
        self.movie.set_text(path, text, &self.fonts);
    }

    /// THE SCREEN OPENS: the camp menu's movie comes in (root `in`) straight onto its Items screen (the top screen stays off).
    pub fn open(&mut self, menu: &mut TuneMenu) {
        menu.texts = self.texts();
        self.movie.goto_and_play("", "in");
        self.movie.goto_and_play("topMenu", "off");
        self.movie.set_visible("topMenu", false);
        self.movie.goto_and_play("backyard", "in");
        self.movie.goto_and_play(BACKYARD, "backyard");
        self.movie.set_visible(&format!("{BACKYARD}.charaList"), false);
        self.open = true;
        self.closing = false;
        self.age = 0;
        self.stamp = 0;
        self.guide.clear();
        self.pane_item = None;
        menu.list.invalidate();
    }

    /// The screen leaves (root `out`); [`TuneView::tick`] says when it is gone.
    pub fn close(&mut self) {
        if self.open && !self.closing {
            self.movie.goto_and_play("backyard", "out");
            self.movie.goto_and_play("", "out");
            self.sync_dialog(None);
            self.closing = true;
        }
    }

    fn set_guide(&mut self, text: &str) {
        if self.guide == text {
            return;
        }
        let plate = format!("{FRAME}.btnInfo");
        let shown = matches!(self.movie.current_label(&plate), Some("in" | "loop" | "in2"));
        if text.is_empty() {
            if shown {
                self.movie.goto_and_play(&plate, "out");
            }
        } else {
            if !shown {
                self.movie.goto_and_play(&plate, "in");
            }
            self.text(GUIDE, text);
        }
        self.guide = text.to_string();
    }

    /// The item status pane (`ItemStatusA1`): the ash's picture and the row's text.
    fn pane(&mut self, item: Option<&str>, help: &str) {
        let texture = item.and_then(|id| self.art.icon(id));
        let label = if help.is_empty() && texture.is_none() { "off" } else { "loop" };
        if self.movie.current_label(PANE) != Some(label) {
            self.movie.goto_and_play(PANE, label);
        }
        let spinner = format!("{PANE}.itemLoad");
        if self.movie.exists(&spinner) {
            self.movie.set_visible(&spinner, false);
        }
        // (THE USER 2026-11-03: "...never cut off") the whole text: shrunk, then scrolled (`Movie::set_text_fit`)
        self.movie.set_text_fit(&format!("{PANE}.txHelp.textField"), help, &self.fonts);
        let shown = texture.as_ref().and(item).map(str::to_string);
        if self.pane_item.as_ref() != Some(&shown) {
            let image = format!("{PANE}.itemImage");
            self.movie.set_visible(&image, shown.is_some());
            self.movie.goto_and_play(&image, if shown.is_some() { "in" } else { "off" });
            if let Some(sprite) = self.movie.sprite_mut(&format!("{image}.imageAnm.coImage")) {
                crate::controls::icon_load(sprite, texture);
            }
            self.pane_item = Some(shown);
        }
    }

    /// Reflects the logic: the frame, the rows, the pane, the help line, the guide, the question.
    pub fn sync(&mut self, menu: &mut TuneMenu) {
        if !self.open || self.closing {
            return;
        }
        if self.movie.exists(&format!("{FRAME}.munny.munnyNum.textField")) {
            let munny = menu.data.munny.clamp(0, 99_999_999).to_string();
            self.text(&format!("{FRAME}.munny.munnyNum.textField"), &munny);
            let word = self.string_id("kh3_menu.shop.munny_0010");
            self.text(&format!("{FRAME}.munny.txMunny.coText"), &word);
            let title = menu.data.title.clone();
            self.text(&format!("{FRAME}.areaInfo.areaName.txName"), &title);
            self.movie.set_visible(&format!("{FRAME}.areaInfo.worldTitle"), false);
            self.movie.set_visible(&format!("{FRAME}.playTime"), false);
        }
        if !self.movie.exists(&format!("{ITEM_ROOT}.backyardList.container")) {
            self.sync_dialog(Some(menu));
            return;
        }
        let stamp = stamp_of(&(&menu.data, &menu.list, menu.layer, menu.confirm, &menu.notice)) ^ self.movie.exists(&menu.list.item_path(0)) as u64;
        if stamp != self.stamp {
            let fresh = self.stamp == 0;
            self.stamp = stamp;
            if fresh {
                let name = self.string_id("kh3_menu.synthesis.item_0010");
                self.text("backyard.stageCategory.category.categoryName.txName", "SPIRIT TUNING");
                self.text(&format!("{ITEM_ROOT}.backyardListTitle.textField"), &name);
                self.text(&format!("{ITEM_ROOT}.backyardListTitle.backyardListTitleCnt"), "Level");
                // one list, no tabs: the tab bar stays off
                self.movie.set_visible(&format!("{ITEM_ROOT}.listTabCamp"), false);
                self.movie.goto_and_stop("backyard.stageCategory.category.categoryIcon", CATEGORY_ICON);
                self.movie.goto_and_stop("backyard.stageCategory.stageObjL.bgCategoryIcon", CATEGORY_ICON);
            }
            let focused = menu.focused().cloned();
            let help = menu.notice.clone().unwrap_or_else(|| focused.as_ref().map_or_else(String::new, |r| menu.texts.help.replace("{0}", &r.name)));
            self.movie.set_text_fit("backyard.helpText.txHelpText.textField", &help, &self.fonts); // (THE USER 2026-11-03) never cut
            let rows = menu.data.rows.clone();
            let empty = if rows.is_empty() { "You have no spirit ashes.".to_string() } else { String::new() };
            self.text(&format!("{ITEM_ROOT}.textField"), &empty);
            let (stock, needed) = (self.string_id("kh3_menu.synthesis.item_0030"), self.string_id("kh3_menu.synthesis.item_0040"));
            let pane = focused.as_ref().map(|r| pane_text(r, &stock, &needed)).unwrap_or_default();
            self.pane(focused.as_ref().map(|r| r.icon.as_str()), &pane);
            let enabled: Vec<bool> = rows.iter().map(|r| menu.refusal(r).is_none()).collect();
            let fonts = self.fonts.clone();
            menu.list.has_focus = true;
            menu.list.refresh(&mut self.movie, |movie, path, index| {
                let row = &rows[index];
                movie.set_text(&format!("{path}.textIcon"), "<icon(KeyItem)>", &fonts);
                movie.set_text(&format!("{path}.backyardItemName"), &row.name, &fonts);
                movie.set_text(&format!("{path}.itemCnt"), &if row.maxed() { "MAX".to_string() } else { format!("+{}", row.level) }, &fonts);
                movie.set_visible(&format!("{path}.coTextBack"), false);
                RowState { enabled: enabled[index] }
            });
            let guide = match menu.layer {
                TuneLayer::List => "<icon(Decide)> Power Up  <icon(Cancel)> Back".to_string(),
                TuneLayer::Confirm => "<icon(Decide)> OK  <icon(Cancel)> Back".to_string(),
            };
            if self.age > 20 {
                self.set_guide(&guide);
            } else {
                self.stamp = 1;
            }
        }
        self.sync_dialog(Some(menu));
    }

    /// KH3's question (the `Dialog` movie: its window, text field and the Yes / No buttons).
    fn sync_dialog(&mut self, menu: Option<&TuneMenu>) {
        let content = menu.filter(|m| m.layer == TuneLayer::Confirm).and_then(|m| {
            let row = m.focused()?;
            // (two lines: the window keeps its authored size and its buttons sit under the second; the munny is in the pane)
            let what = row.cost.as_ref().map(|c| format!("{} x {}", c.name, c.count)).unwrap_or_default();
            Some((format!("{}\n{what}", m.texts.question.replace("{0}", &leveled(&row.name, row.level))), m.confirm))
        });
        let Some((text, cursor)) = content else {
            if self.dialog_open && !self.dialog_closing {
                self.dialog.goto_and_play("dialog", "out");
                self.dialog.set_visible("dialogText", false);
                for name in ["dialogButton", "dialogButton2"] {
                    self.dialog.set_visible(name, false);
                }
                self.dialog_closing = true;
            }
            return;
        };
        let buttons = [self.string_id("kh3_system.general.choice_0001"), self.string_id("kh3_system.general.choice_0002")];
        let stamp = stamp_of(&(&text, &buttons, cursor));
        if self.dialog_open && !self.dialog_closing && stamp == self.dialog_stamp {
            self.place_dialog_parts();
            return;
        }
        if !self.dialog_open || self.dialog_closing {
            self.dialog.goto_and_play("dialog", "in");
            if !self.dialog.exists("dialogButton2") {
                self.dialog.add_instance("", "DialogButton", "dialogButton2");
            }
        }
        self.dialog_open = true;
        self.dialog_closing = false;
        self.dialog_stamp = stamp;
        self.dialog.set_visible("dialogText", true);
        self.dialog.set_text("dialogText.textField", &text, &self.fonts);
        for (n, name) in ["dialogButton", "dialogButton2"].into_iter().enumerate() {
            self.dialog.set_visible(name, true);
            self.dialog.set_text(&format!("{name}.dialogBtnText.textField"), &buttons[n], &self.fonts);
            self.dialog.goto_and_play(name, if cursor == n { "select" } else { "normal" });
        }
        self.place_dialog_parts();
    }

    /// The text and the buttons at the window's position holders (as the camp menu's dialogs).
    fn place_dialog_parts(&mut self) {
        let holder = "dialog.dialogAnm.dialogPoAll.dialogPoAll2";
        let stage = |movie: &Movie, path: &str| -> Option<(f32, f32)> {
            let mut m = crate::geom::Matrix::IDENTITY;
            let mut at = String::new();
            for part in path.split('.') {
                at = if at.is_empty() { part.to_string() } else { format!("{at}.{part}") };
                m = m.mul(&movie.node(&at)?.matrix);
            }
            Some((m.tx, m.ty))
        };
        if let Some((x, y)) = stage(&self.dialog, &format!("{holder}.poDialogText")) {
            self.dialog.set_position("dialogText", x, y, 0.0);
        }
        if let Some((x, y)) = stage(&self.dialog, &format!("{holder}.poDialogButton")) {
            self.dialog.set_position("dialogButton", x, y, 0.0);
            self.dialog.set_position("dialogButton2", x, y + 35.0, 0.0);
        }
    }

    /// One stage frame. True when the close animation ended (the screen is gone).
    pub fn tick(&mut self) -> bool {
        if !self.open {
            return false;
        }
        self.age = self.age.saturating_add(1);
        self.movie.tick();
        if self.dialog_open {
            self.dialog.tick();
            if self.dialog_closing && self.dialog.current_label("dialog") == Some("off") {
                self.dialog_open = false;
                self.dialog_closing = false;
            }
        }
        let mut ended = false;
        for event in self.movie.take_events() {
            if event.name == "ON_END_TOP_OUT" && self.closing {
                ended = true;
            }
        }
        self.dialog.take_events();
        if self.closing && self.movie.current_label("") == Some("off") {
            ended = true;
        }
        if ended {
            self.open = false;
            self.closing = false;
            self.dialog_open = false;
            self.dialog_closing = false;
        }
        ended
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: u32, name: &str, level: u8, cost: Option<(&str, u32, u32)>, munny: u32) -> TuneRow {
        TuneRow { id, icon: format!("HOSTKEY_0.icon{id}"), name: name.into(), level, max_level: 10, cost: cost.map(|(key, count, owned)| TuneCost { key: key.into(), name: key.into(), count, owned }), munny, gives: vec!["HP  x1.28 -> x1.81".into()] }
    }

    fn menu() -> TuneMenu {
        TuneMenu::new(TuneData {
            title: "Roderika".into(),
            munny: 5000,
            rows: vec![
                row(200003, "Lone Wolf Ashes", 3, Some(("MAT_GLOVE_GHOST_04", 1, 2)), 5600),
                row(214001, "Black Knife Tiche", 1, Some(("MAT_GLOVE_GRAVE_02", 1, 0)), 1520),
                row(201002, "Jellyfish Ashes", 2, Some(("MAT_GLOVE_GHOST_03", 1, 1)), 3410),
                row(202010, "Wandering Noble Ashes", 10, None, 0),
            ],
        })
    }

    /// (THE USER 2026-11-02) THE LIST: A on a row he can afford -> KH3's question; Yes asks the host for the next level and
    /// shows it at once (level, stock, munny); No / B back to the list; refusals in KH3's words (not enough munny, the
    /// materials, the highest level) with the refused sound; B on the list closes the screen; Menu does the same.
    #[test]
    fn tuning_question_upgrade_refusals_and_close() {
        let mut m = menu();
        let mut s = Vec::new();
        // Lone Wolf +3: Ghost Glovewort [4] held, 5600 munny of 5000: refused (munny)
        m.key(CampKey::Decide, false, &mut s);
        assert_eq!((m.layer, m.notice.as_deref()), (TuneLayer::List, Some(TuneTexts::MUNNY)));
        assert_eq!(s.pop(), Some(UiSound::Play(SE_MENU_REFUSED)));
        // Tiche: no Grave Glovewort [2]
        m.key(CampKey::Down, false, &mut s);
        assert!(m.notice.is_none(), "a move clears the notice");
        m.key(CampKey::Decide, false, &mut s);
        assert_eq!(m.notice.as_deref(), Some(TuneTexts::MATERIALS));
        // the Jellyfish: the question, No, then Yes
        m.key(CampKey::Down, false, &mut s);
        m.key(CampKey::Decide, false, &mut s);
        assert_eq!((m.layer, m.confirm), (TuneLayer::Confirm, 0));
        m.key(CampKey::Down, false, &mut s);
        m.key(CampKey::Decide, false, &mut s);
        assert_eq!(m.layer, TuneLayer::List, "No: back to the list");
        assert!(m.take_events().is_empty());
        m.key(CampKey::Decide, false, &mut s);
        m.key(CampKey::Cancel, false, &mut s);
        assert_eq!(m.layer, TuneLayer::List, "B on the question: back");
        m.key(CampKey::Decide, false, &mut s);
        m.key(CampKey::Decide, false, &mut s);
        assert_eq!(m.take_events(), vec![TuneEvent::Upgrade { id: 201002, to_level: 3 }]);
        assert_eq!((m.data.munny, m.data.rows[2].level, m.data.rows[2].cost.as_ref().map(|c| c.owned)), (5000 - 3410, 3, Some(0)));
        assert!(s.contains(&UiSound::Play(SE_TRADE)));
        assert_eq!(m.notice.as_deref(), Some("<color(strong)>Jellyfish Ashes +3<color>"));
        // the noble at +10: the highest level
        m.key(CampKey::Down, false, &mut s);
        m.key(CampKey::Decide, false, &mut s);
        assert_eq!(m.notice.as_deref(), Some(TuneTexts::MAX), "(no pack in the logic test: the id)");
        // Menu / B close
        m.key(CampKey::Menu, false, &mut s);
        assert_eq!(m.take_events(), vec![TuneEvent::Closed]);
        m.key(CampKey::Cancel, false, &mut s);
        assert_eq!(m.take_events(), vec![TuneEvent::Closed]);
    }

    /// The host's next data: the focus stays on its place; a question whose row can no longer be paid goes back to the list.
    #[test]
    fn tuning_host_data_keeps_the_focus() {
        let mut m = menu();
        let mut s = Vec::new();
        m.data.munny = 99_999;
        m.key(CampKey::Decide, false, &mut s);
        assert_eq!(m.layer, TuneLayer::Confirm);
        let mut d = m.data.clone();
        d.munny = 0;
        m.set_data(d);
        assert_eq!((m.layer, m.focused().map(|r| r.id)), (TuneLayer::List, Some(200003)));
    }

    #[test]
    fn tuning_pane_lines() {
        let r = row(200003, "Lone Wolf Ashes", 3, Some(("Ghost Glovewort [4]", 1, 2)), 5600);
        let text = pane_text(&r, "Stock", "Materials Needed");
        assert_eq!(text, "<color(strong)>Materials Needed<color>  Ghost Glovewort [4] x 1\n<color(strong)>Stock<color> 2   5,600 MUNNY   <color(strong)>+3 -> +4<color>\nHP  x1.28 -> x1.81");
        assert!(pane_text(&row(1, "A", 10, None, 0), "Stock", "Needed").starts_with("<color(strong)>A +10<color>  MAX"));
        assert_eq!((leveled("A", 0), leveled("A", 4)), ("A".to_string(), "A +4".to_string()));
    }
}
