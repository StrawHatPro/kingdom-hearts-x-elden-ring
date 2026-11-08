//! (weapons P6b, documentation/KH3_WEAPONS_PLAN.md 1.5 / 1.6 / 6a) KH3'S WORKSHOP SCREENS: the Workshop part of the Moogle
//! menu (movie `41_shop/Shop`), 1:1 with KH3. The rules are kh-host's `workshop.rs` (P6a); this module is the screen:
//! [`WorkshopMenu`] the logic (plain data in, [`WorkshopEvent`]s out), [`view::WorkshopView`] the display.
//!
//! THE USER (2026-11-03): "The moogle in the round table is only for synthesis": no Buy, no Sell, no M.O.G. Card. The menu
//! opens STRAIGHT ON KH3's Workshop list ([`Entry::Workshop`]); a blacksmith opens it STRAIGHT ON THE KEYBLADE FORGE
//! ([`Entry::Forge`], B there closes it and the smith's talk goes on). THE USER (2026-11-07): "remove the ability to upgrade
//! keyblades from moogle, that is what the blacksmiths are for": the Moogle's list has NO Keyblade Forge (the menu drops it
//! from the Moogle's commands whatever the host sends: [`WorkshopMenu::new`] / [`WorkshopMenu::set_data`]); KH3's Photo
//! Missions read "Journey to the Erdtree" ([`texts::MOD`]).
//!
//! WHAT IS KH3'S [A] (native drivers traced in the decomp, addresses below; the movie's AS3 in `as3/Shop.as3.txt`):
//! - the Workshop top (`UTresUIP_ShopSynthesisTop`, `0x14173F670` / `0x14173C890` / `0x141738450`): five commands
//!   (`kh3_menu.synthesis.menu_0010..0050`, help `detail_0010..0050`, category icons `shop11..shop15`), the SYNTH LOG
//!   (Items Created / Photo Missions Completed / Collector's Goals Completed, the COMPLETE marks), up / down (a fresh press
//!   wraps), A enters, B leaves; the NEW marks of Synthesize Items and Photo Missions;
//! - the entry dialogs (`0x14172EFF0`, shown one by one in the system window, `0x14173F1D0`): dialog_0010 (materials left
//!   at the Workshop), per new Collector's Goal dialog_0020 (once) / dialog_0030 (an item handed over, static SE 10) /
//!   dialog_0040 (an item for sale), then dialog_0050 once;
//! - Synthesize Items (`UTresUIP_ShopItemSynthesis`, `0x141731730` / `0x1417373A0`): the six tabs (All / Items / Weapons /
//!   Armor / Accessories / Materials, L2 / R2, the Tab sound), rows with the quantity on the row (left / right only when
//!   more than one can be made: a fresh press wraps 1 <-> the most, held it stops; static SE 0), Stock, the clear mark,
//!   NEW (cleared when the row gets the cursor), the materials pane (Qty. = need x quantity, Stock, "lack"), the item pane;
//!   A on a row that cannot be made: static SE 4; else the confirmation (mesWin2: the row, "x n", dialog_0110, Yes / No
//!   with the cursor on No; B there does nothing); Yes: the synthesis and KH3's large item-get popup (the host's);
//! - the Keyblade Forge (`UTresUIP_ShopKeyPowerUp`, `0x141731B50` / `0x141737770` / `0x1417406D0`): the Keyblade picker
//!   (left / right, only with two or more; circular), the level icons, Strength / Magic now -> after, the ability the level
//!   adds, the materials; A at the highest level: static SE 4; lacking materials: dialog_0210 in the system window; else
//!   the system Yes / No dialog_0220 ("Power up the {0}?"); Yes: the power-up (static SE 32) and its effects
//!   (`effEnhance` lv1..10, `anmKeyb`, `anmLock` / `inF`, `anmKeybF`, `anmWhiteout`); NO MUNNY (WD 6d);
//! - Photo Missions (`UTresUIP_ShopPhotoMission`, `0x141732880` / `0x141738290`): the list with the COMPLETE marks and
//!   NEW, the speech balloon and the reward pane (the item when done, "? ? ?" before); A does nothing;
//! - Collector's Goals (`UTresUIP_ShopCollection`, `0x1417313F0`): the goal texts, the clear marks, the reward pane
//!   (bonus_0010 recipes / bonus_0020 an item / bonus_0030 a purchase, up to four names);
//! - the Material List (`UTresUIP_ShopMaterialListMenu`, `0x141732340` / `0x141738050`): two columns, no cursor: up / down
//!   scroll a line, LB / RB a page (a fresh press wraps), static SE 0; "? ? ?" for a material never obtained;
//! - the sounds: the system Decide / Cancel / Tab (static 1 / 3 / 2), Click (0), Beep (4), 10, 32.
//!
//! WHAT IS COMPOSED HERE [L]:
//! - the Photo Missions are the user's 20 Elden Ring milestones (P6a's Moogle-voiced texts, no camera);
//! - the "for sale" texts (bonus_0030, dialog_0040) say the MERCHANTS sell the item (the user: the converted Elden Ring
//!   merchants sell what a goal unlocks; [`texts::MERCHANT_BONUS`], [`texts::MERCHANT_DIALOG`]);
//! - B on the Workshop top closes the menu (KH3 goes back to its Buy / Sell top, which the Moogle no longer has);
//! - the button guide plate's lines (KH3 sets none natively here; the mod's screens all carry one: controller first).
//!
//! THE PAD (Xbox layout, KH3's): D-pad / left stick move, A decide, B back one level, LB / RB a page, LT / RT the tabs,
//! the right stick scrolls a description that does not fit (the host's `scroll_help`). The keyboard reaches the same keys
//! through the host.

pub mod forge_art; // (FORGE-PREVIEW) KH3's Forge pictures: keychains, whole Keyblades
pub mod texts;
pub mod view;

use std::collections::{BTreeMap, VecDeque};

use crate::camp::CampKey;
use crate::list::ListView;
use crate::menu::{SE_MENU_CLOSE, SE_MENU_CURSOR, SE_MENU_DECIDE, SE_MENU_OPEN, SE_MENU_REFUSED};
use crate::UiSound;

/// Static SE 10 (se00000_011): the entry dialog of an item handed over (dialog_0030); KH3's large item-get popup plays it too.
pub const SE_ITEM_GET: u32 = 10;
/// Static SE 32 (se00000_066): the Keyblade Forge's power-up.
pub const SE_FORGE: u32 = 32;
/// The Tab sound (system SE 4 -> static 2).
pub const SE_TAB: u32 = SE_MENU_OPEN;

/// Where the menu opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Entry {
    /// The Moogle: the Workshop's list.
    #[default]
    Workshop,
    /// A blacksmith: straight on the Keyblade Forge (B closes the menu).
    Forge,
}

/// The screens of the Workshop.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, PartialOrd, Ord)]
pub enum Screen {
    #[default]
    Top,
    Synthesis,
    Forge,
    Photo,
    Goals,
    Materials,
}

impl Screen {
    /// The Workshop's commands in KH3's order.
    pub const COMMANDS: [Screen; 5] = [Screen::Synthesis, Screen::Forge, Screen::Photo, Screen::Goals, Screen::Materials];
    /// The Moogle's commands: KH3's without the Keyblade Forge (the user, 2026-11-07: the blacksmiths forge).
    pub const MOOGLE: [Screen; 4] = [Screen::Synthesis, Screen::Photo, Screen::Goals, Screen::Materials];

    /// `kh3_menu.synthesis.menu_00n0` / `detail_00n0` number (1..5); the category icon is `shop1n`.
    pub fn number(self) -> u8 {
        match self {
            Screen::Top => 0,
            Screen::Synthesis => 1,
            Screen::Forge => 2,
            Screen::Photo => 3,
            Screen::Goals => 4,
            Screen::Materials => 5,
        }
    }

    /// The movie clip of the screen.
    pub fn clip(self) -> &'static str {
        match self {
            Screen::Top => "menuMixTop",
            Screen::Synthesis => "menuSynthesis",
            Screen::Forge => "menuKeyb",
            Screen::Photo => "menuPhoto",
            Screen::Goals => "menuCollection",
            Screen::Materials => "menuMaterial",
        }
    }

    /// The category icon (`McCategoryIcon*` labels): the Workshop `shop3`, its entries `shop11..shop15`.
    pub fn icon(self) -> &'static str {
        match self {
            Screen::Top => "shop3",
            Screen::Synthesis => "shop11",
            Screen::Forge => "shop12",
            Screen::Photo => "shop13",
            Screen::Goals => "shop14",
            Screen::Materials => "shop15",
        }
    }
}

/// A material a recipe or a Forge level needs.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct MatNeed {
    pub key: String,
    pub name: String,
    /// For one item (one level).
    pub need: u32,
    /// In the Workshop's stock.
    pub stock: u32,
}

/// One listed recipe.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct RecipeRow {
    pub id: String,
    /// The item made.
    pub key: String,
    pub name: String,
    /// `camp_equip::CAT_*` (the row's icon).
    pub category: u8,
    /// The tab filter (KH3's `FUN_1415b2640`): 1 Items, 2 Weapons, 3 Armor, 4 Accessories, 5 Materials.
    pub tab: u8,
    /// Copies held (the Stock column).
    pub held: u32,
    /// How many can be made now (0: greyed, A refused).
    pub max: u32,
    /// Made before (the clear mark).
    pub made: u32,
    pub new: bool,
    pub materials: Vec<MatNeed>,
    /// The item's description.
    pub help: String,
}

/// The level the Forge offers a Keyblade.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ForgeNext {
    pub level: u8,
    pub strength: i32,
    pub magic: i32,
    /// The ability the level adds (its name), if any.
    pub ability: Option<String>,
    pub materials: Vec<MatNeed>,
    /// Its materials are in the stock.
    pub ready: bool,
}

/// One owned Keyblade.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ForgeRow {
    pub key: String,
    pub name: String,
    pub level: u8,
    pub max_level: u8,
    pub strength: i32,
    pub magic: i32,
    /// `None`: at its highest level.
    pub next: Option<ForgeNext>,
}

/// One listed milestone mission.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct PhotoRow {
    pub id: String,
    /// The Moogle's line (the row).
    pub text: String,
    /// The detail (the help line).
    pub help: String,
    pub done: bool,
    pub new: bool,
    /// What it unlocks (shown once done; "? ? ?" before).
    pub reward_key: String,
    pub reward_name: String,
}

/// KH3's three reward kinds of a Collector's Goal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RewardKind {
    /// TICR_GET_ITEM: bonus_0020 / dialog_0030.
    #[default]
    Item,
    /// TICR_ENABLE_SYNTHESIS: bonus_0010 / dialog_0020.
    Synthesis,
    /// TICR_ENABLE_BUY: bonus_0030 / dialog_0040 (the merchants sell it).
    Buy,
}

/// One Collector's Goal.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct GoalRow {
    pub id: String,
    /// The goal's text (collection_00n0 with its numbers).
    pub text: String,
    pub done: bool,
    pub reward: RewardKind,
    /// The reward's items (an item, the recipes' items, the purchases): (category, name), up to four shown.
    pub items: Vec<(u8, String)>,
}

/// One row of the Material List.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct MaterialRow {
    pub key: String,
    /// The name, or "? ? ?".
    pub name: String,
    pub known: bool,
    pub stock: u32,
    /// The rarity (the row's mark): 0 RANK00 (crystals, ores) .. 3 RANK03 (shards).
    pub rank: u8,
}

/// The SYNTH LOG.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct LogRow {
    pub created: u32,
    pub photos: u32,
    pub goals: u32,
    /// The COMPLETE marks.
    pub complete: [bool; 3],
}

/// What the host shows.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct WorkshopData {
    /// The frame's area line (the place).
    pub title: String,
    pub munny: i64,
    /// The Workshop's commands shown (KH3 hides a locked one) with their NEW marks.
    pub commands: Vec<(Screen, bool)>,
    pub log: LogRow,
    pub recipes: Vec<RecipeRow>,
    pub keyblades: Vec<ForgeRow>,
    pub photos: Vec<PhotoRow>,
    pub goals: Vec<GoalRow>,
    pub materials: Vec<MaterialRow>,
}

/// A system message of the entry (one A each).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Message {
    pub text: String,
    /// A sound with it (dialog_0030: static SE 10).
    pub sound: Option<u32>,
}

/// What the screen asks of the host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkshopEvent {
    /// Yes on the synthesis confirmation.
    Synthesize { id: String, count: u32 },
    /// Yes on the Forge's question.
    Forge { key: String },
    /// A recipe row got the cursor (its NEW mark goes).
    SeenRecipe(String),
    /// A mission row got the cursor.
    SeenPhoto(String),
    /// The menu closes (its `out` follows): B on the top, or on the Forge opened by a smith.
    Closed,
}

/// The layer that has the pad.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Layer {
    #[default]
    Main,
    /// The synthesis confirmation (mesWin2) or the Forge's Yes / No (the system window).
    Confirm,
    /// The Forge's "You lack the needed materials." (the system window, one A).
    Notice,
    /// The power-up's effects play (no input until they end).
    Effect,
}

/// The synthesis list's six tabs (`DAT_149a98640`): (label id, tab icon cell, filter; 0 = all).
pub const TABS: [(&str, i32, u8); 6] = [("kh3_menu.shop.tab_0010", 0, 0), ("kh3_menu.shop.tab_0020", 1, 1), ("kh3_menu.shop.tab_0030", 2, 2), ("kh3_menu.shop.tab_0040", 3, 3), ("kh3_menu.shop.tab_0050", 4, 4), ("kh3_menu.shop.tab_0060", 6, 5)];

/// The list widgets of the screens.
pub const SYNTH_LIST: &str = "menuSynthesis.menuObj.synthesisList.coList";
pub const PHOTO_LIST: &str = "menuPhoto.menuObj.photoList.coList";
pub const GOAL_LIST: &str = "menuCollection.menuObj.collectionList.coList";
pub const MATERIAL_LIST: &str = "menuMaterial.menuObj.materialList.coList";
/// The Material List's grid: 2 columns x 10 lines.
pub const MATERIAL_COLUMNS: usize = 2;
pub const MATERIAL_LINES: usize = 10;

pub struct WorkshopMenu {
    pub data: WorkshopData,
    pub entry: Entry,
    pub screen: Screen,
    pub layer: Layer,
    /// The top's cursor (an index into `data.commands`).
    pub top: usize,
    /// The synthesis tab (0..6).
    pub tab: usize,
    /// The tab switch arrow to flash.
    pub tab_flash: Option<bool>,
    pub synth: ListView,
    /// The synthesis quantity per recipe (KH3: per entry, from 1).
    pub qty: BTreeMap<String, u32>,
    /// The Forge's Keyblade (an index into `data.keyblades`).
    pub keyblade: usize,
    /// The picker's last move (true: right) until the view played it.
    pub picker_move: Option<bool>,
    pub photo: ListView,
    pub goals: ListView,
    /// The Material List's first line.
    pub material_line: usize,
    /// The Yes / No cursor: 0 Yes, 1 No.
    pub confirm: usize,
    /// Entry messages waiting (the first is up).
    pub messages: VecDeque<Message>,
    /// The Forge's power-up to play (the level reached) until the view started it.
    pub enhance: Option<u8>,
    /// The screen to leave (its `outBack` / `outNext`) until the view played it.
    pub leaving: Option<Screen>,
    events: Vec<WorkshopEvent>,
}

/// The tab filter of a row's kind (KH3's category map `FUN_1415b2640`).
pub fn tab_of(kind: &str) -> u8 {
    match kind {
        "weapon" => 2,
        "armour" => 3,
        "accessory" => 4,
        "material" => 5,
        _ => 1,
    }
}

/// The Forge's picker offsets shown for `n` Keyblades: -3..=3 at most, each Keyblade once.
pub fn picker_offsets(n: usize) -> Vec<i32> {
    if n == 0 {
        return Vec::new();
    }
    let right = (n as i32 / 2).min(3);
    let left = ((n as i32 - 1) / 2).min(3);
    (-left..=right).collect()
}

/// The picker label of an offset (p0 the centre, p1 / p2 one right / left, p3 / p4, p5 / p6).
pub fn picker_label(offset: i32) -> &'static str {
    match offset {
        0 => "p0",
        1 => "p1",
        -1 => "p2",
        2 => "p3",
        -2 => "p4",
        3 => "p5",
        -3 => "p6",
        o if o > 0 => "p5",
        _ => "p6",
    }
}

/// The commands a menu opened this way shows: the Moogle's never hold the Keyblade Forge (the user, 2026-11-07).
fn commands_for(entry: Entry, data: &mut WorkshopData) {
    if entry == Entry::Workshop {
        data.commands.retain(|c| c.0 != Screen::Forge);
    }
}

impl WorkshopMenu {
    pub fn new(mut data: WorkshopData, entry: Entry) -> WorkshopMenu {
        let screen = if entry == Entry::Forge { Screen::Forge } else { Screen::Top };
        commands_for(entry, &mut data);
        let mut menu = WorkshopMenu {
            data,
            entry,
            screen,
            layer: Layer::Main,
            top: 0,
            tab: 0,
            tab_flash: None,
            synth: ListView::new(SYNTH_LIST, "CoSynthesisListItem"),
            qty: BTreeMap::new(),
            keyblade: 0,
            picker_move: None,
            photo: ListView::new(PHOTO_LIST, "CoMissionListItem"),
            goals: ListView::new(GOAL_LIST, "CoMissionListItem"),
            material_line: 0,
            confirm: 0,
            messages: VecDeque::new(),
            enhance: None,
            leaving: None,
            events: Vec::new(),
        };
        menu.recount();
        menu
    }

    /// The entry dialogs (KH3 shows them over the top, one A each).
    pub fn push_messages(&mut self, messages: impl IntoIterator<Item = Message>, sounds: &mut Vec<UiSound>) {
        let was_empty = self.messages.is_empty();
        self.messages.extend(messages);
        if was_empty {
            if let Some(sound) = self.messages.front().and_then(|m| m.sound) {
                sounds.push(UiSound::Play(sound));
            }
        }
    }

    /// The message up now.
    pub fn message(&self) -> Option<&Message> {
        self.messages.front()
    }

    /// The recipes of the current tab.
    pub fn recipe_rows(&self) -> Vec<&RecipeRow> {
        let filter = TABS[self.tab.min(TABS.len() - 1)].2;
        self.data.recipes.iter().filter(|r| filter == 0 || r.tab == filter).collect()
    }

    pub fn recipe(&self) -> Option<&RecipeRow> {
        usize::try_from(self.synth.focus).ok().and_then(|i| self.recipe_rows().get(i).copied())
    }

    /// The quantity shown on a recipe's row (1 .. the most that can be made).
    pub fn qty_of(&self, row: &RecipeRow) -> u32 {
        self.qty.get(&row.id).copied().unwrap_or(1).clamp(1, row.max.max(1))
    }

    pub fn forge_row(&self) -> Option<&ForgeRow> {
        self.data.keyblades.get(self.keyblade)
    }

    pub fn photo_row(&self) -> Option<&PhotoRow> {
        usize::try_from(self.photo.focus).ok().and_then(|i| self.data.photos.get(i))
    }

    pub fn goal_row(&self) -> Option<&GoalRow> {
        usize::try_from(self.goals.focus).ok().and_then(|i| self.data.goals.get(i))
    }

    /// The Material List's last first line.
    pub fn material_line_max(&self) -> usize {
        self.data.materials.len().div_ceil(MATERIAL_COLUMNS).saturating_sub(MATERIAL_LINES)
    }

    /// The lists' counts after new data (the focused rows kept by their ids).
    fn recount(&mut self) {
        let n = self.recipe_rows().len();
        self.synth.set_count(n);
        self.photo.set_count(self.data.photos.len());
        self.goals.set_count(self.data.goals.len());
        self.keyblade = self.keyblade.min(self.data.keyblades.len().saturating_sub(1));
        self.top = self.top.min(self.data.commands.len().saturating_sub(1));
        self.material_line = self.material_line.min(self.material_line_max());
    }

    /// The host's data again (every frame): the cursors are kept on the same rows.
    pub fn set_data(&mut self, mut data: WorkshopData) {
        commands_for(self.entry, &mut data);
        if data == self.data {
            return;
        }
        let recipe = self.recipe().map(|r| r.id.clone());
        let photo = self.photo_row().map(|r| r.id.clone());
        let goal = self.goal_row().map(|r| r.id.clone());
        let keyblade = self.forge_row().map(|r| r.key.clone());
        let top = self.data.commands.get(self.top).map(|c| c.0);
        self.data = data;
        self.recount();
        if let Some(k) = recipe.and_then(|id| self.recipe_rows().iter().position(|r| r.id == id)) {
            self.synth.set_focus(k);
        }
        if let Some(k) = photo.and_then(|id| self.data.photos.iter().position(|r| r.id == id)) {
            self.photo.set_focus(k);
        }
        if let Some(k) = goal.and_then(|id| self.data.goals.iter().position(|r| r.id == id)) {
            self.goals.set_focus(k);
        }
        if let Some(k) = keyblade.and_then(|key| self.data.keyblades.iter().position(|r| r.key == key)) {
            self.keyblade = k;
        }
        if let Some(k) = top.and_then(|s| self.data.commands.iter().position(|c| c.0 == s)) {
            self.top = k;
        }
        for list in [&mut self.synth, &mut self.photo, &mut self.goals] {
            list.invalidate();
        }
    }

    /// The row under the cursor of a list screen was seen (its NEW mark goes).
    fn seen(&mut self) {
        match self.screen {
            Screen::Synthesis => {
                if let Some(r) = self.recipe().filter(|r| r.new).map(|r| r.id.clone()) {
                    for row in self.data.recipes.iter_mut().filter(|row| row.id == r) {
                        row.new = false;
                    }
                    if let Some(c) = self.data.commands.iter_mut().find(|c| c.0 == Screen::Synthesis) {
                        c.1 = self.data.recipes.iter().any(|row| row.new);
                    }
                    self.synth.invalidate();
                    self.events.push(WorkshopEvent::SeenRecipe(r));
                }
            }
            Screen::Photo => {
                if let Some(r) = self.photo_row().filter(|r| r.new).map(|r| r.id.clone()) {
                    for row in self.data.photos.iter_mut().filter(|row| row.id == r) {
                        row.new = false;
                    }
                    if let Some(c) = self.data.commands.iter_mut().find(|c| c.0 == Screen::Photo) {
                        c.1 = self.data.photos.iter().any(|row| row.new);
                    }
                    self.photo.invalidate();
                    self.events.push(WorkshopEvent::SeenPhoto(r));
                }
            }
            _ => {}
        }
    }

    /// Enters a screen from the top.
    fn enter(&mut self, screen: Screen) {
        self.leaving = Some(Screen::Top);
        self.screen = screen;
        self.layer = Layer::Main;
        match screen {
            Screen::Synthesis => {
                self.tab = 0;
                let n = self.recipe_rows().len();
                self.synth.focus = 0;
                self.synth.scroll = 0;
                self.synth.set_count(n);
                self.synth.invalidate();
            }
            Screen::Photo => {
                self.photo.focus = 0;
                self.photo.scroll = 0;
                self.photo.set_count(self.data.photos.len());
                self.photo.invalidate();
            }
            Screen::Goals => {
                self.goals.focus = 0;
                self.goals.scroll = 0;
                self.goals.set_count(self.data.goals.len());
                self.goals.invalidate();
            }
            Screen::Materials => self.material_line = 0,
            Screen::Forge | Screen::Top => {}
        }
        self.seen();
    }

    /// Back one level: a sub-screen to the top (focused on it), the top or a smith's Forge closes the menu.
    fn back(&mut self, sounds: &mut Vec<UiSound>) {
        sounds.push(UiSound::Play(SE_MENU_CLOSE));
        if self.screen == Screen::Top || self.entry == Entry::Forge {
            self.events.push(WorkshopEvent::Closed);
            return;
        }
        self.leaving = Some(self.screen);
        if let Some(k) = self.data.commands.iter().position(|c| c.0 == self.screen) {
            self.top = k;
        }
        self.screen = Screen::Top;
        self.layer = Layer::Main;
    }

    /// A MENU KEY.
    pub fn key(&mut self, key: CampKey, repeat: bool, sounds: &mut Vec<UiSound>) {
        // the entry dialogs first: A (or B) takes each away
        if !self.messages.is_empty() {
            if matches!(key, CampKey::Decide | CampKey::Cancel) && !repeat {
                sounds.push(UiSound::Play(SE_MENU_DECIDE));
                self.messages.pop_front();
                if let Some(sound) = self.messages.front().and_then(|m| m.sound) {
                    sounds.push(UiSound::Play(sound));
                }
            }
            return;
        }
        match self.layer {
            Layer::Effect => {}
            Layer::Notice => {
                if matches!(key, CampKey::Decide | CampKey::Cancel) && !repeat {
                    sounds.push(UiSound::Play(SE_MENU_DECIDE));
                    self.layer = Layer::Main;
                }
            }
            Layer::Confirm => self.key_confirm(key, repeat, sounds),
            Layer::Main => match self.screen {
                Screen::Top => self.key_top(key, repeat, sounds),
                Screen::Synthesis => self.key_synthesis(key, repeat, sounds),
                Screen::Forge => self.key_forge(key, repeat, sounds),
                Screen::Photo | Screen::Goals => self.key_list(key, repeat, sounds),
                Screen::Materials => self.key_materials(key, repeat, sounds),
            },
        }
    }

    fn key_top(&mut self, key: CampKey, repeat: bool, sounds: &mut Vec<UiSound>) {
        let n = self.data.commands.len();
        match key {
            CampKey::Up | CampKey::Down if n > 0 => {
                let down = key == CampKey::Down;
                let to = match (down, self.top) {
                    (true, t) if t + 1 < n => Some(t + 1),
                    (true, _) => (!repeat).then_some(0),
                    (false, 0) => (!repeat).then_some(n - 1),
                    (false, t) => Some(t - 1),
                };
                if let Some(to) = to.filter(|t| *t != self.top) {
                    self.top = to;
                    sounds.push(UiSound::Play(SE_MENU_CURSOR));
                }
            }
            CampKey::Decide if !repeat => {
                if let Some((screen, _)) = self.data.commands.get(self.top).copied() {
                    sounds.push(UiSound::Play(SE_MENU_DECIDE));
                    self.enter(screen);
                }
            }
            CampKey::Cancel if !repeat => self.back(sounds),
            _ => {}
        }
    }

    fn switch_tab(&mut self, right: bool, sounds: &mut Vec<UiSound>) {
        self.tab = if right { (self.tab + 1) % TABS.len() } else { (self.tab + TABS.len() - 1) % TABS.len() };
        self.tab_flash = Some(right);
        self.synth.focus = 0;
        self.synth.scroll = 0;
        let n = self.recipe_rows().len();
        self.synth.set_count(n);
        self.synth.invalidate();
        sounds.push(UiSound::Play(SE_TAB));
        self.seen();
    }

    fn key_synthesis(&mut self, key: CampKey, repeat: bool, sounds: &mut Vec<UiSound>) {
        match key {
            CampKey::Up | CampKey::Down => {
                if self.synth.move_anchor(key == CampKey::Down, repeat) {
                    sounds.push(UiSound::Play(SE_MENU_CURSOR));
                    self.seen();
                }
            }
            CampKey::L1 | CampKey::R1 => {
                let moved = if key == CampKey::R1 { self.synth.next_page(repeat) } else { self.synth.prev_page(repeat) };
                if moved {
                    sounds.push(UiSound::Play(SE_MENU_CURSOR));
                    self.seen();
                }
            }
            CampKey::L2 | CampKey::R2 if !repeat => self.switch_tab(key == CampKey::R2, sounds),
            CampKey::Left | CampKey::Right => {
                let Some(row) = self.recipe().cloned() else { return };
                if row.max <= 1 {
                    return;
                }
                let was = self.qty_of(&row);
                let next = if key == CampKey::Right {
                    if was < row.max { was + 1 } else if repeat { was } else { 1 }
                } else if was > 1 {
                    was - 1
                } else if repeat {
                    was
                } else {
                    row.max
                };
                if next != was {
                    self.qty.insert(row.id.clone(), next);
                    self.synth.invalidate();
                    sounds.push(UiSound::Play(SE_MENU_CURSOR));
                }
            }
            CampKey::Decide if !repeat => {
                let Some(row) = self.recipe().cloned() else { return };
                if row.max == 0 {
                    sounds.push(UiSound::Play(SE_MENU_REFUSED));
                    return;
                }
                sounds.push(UiSound::Play(SE_MENU_DECIDE));
                self.confirm = 1;
                self.layer = Layer::Confirm;
            }
            CampKey::Cancel if !repeat => self.back(sounds),
            _ => {}
        }
    }

    fn key_forge(&mut self, key: CampKey, repeat: bool, sounds: &mut Vec<UiSound>) {
        let n = self.data.keyblades.len();
        match key {
            CampKey::Left | CampKey::Right if n >= 2 => {
                let right = key == CampKey::Right;
                self.keyblade = if right { (self.keyblade + 1) % n } else { (self.keyblade + n - 1) % n };
                self.picker_move = Some(right);
                sounds.push(UiSound::Play(SE_MENU_CURSOR));
            }
            CampKey::Decide if !repeat => {
                let Some(row) = self.forge_row() else { return };
                match &row.next {
                    None => sounds.push(UiSound::Play(SE_MENU_REFUSED)),
                    Some(next) if !next.ready => {
                        sounds.push(UiSound::Play(SE_MENU_DECIDE));
                        self.layer = Layer::Notice;
                    }
                    Some(_) => {
                        sounds.push(UiSound::Play(SE_MENU_DECIDE));
                        self.confirm = 0;
                        self.layer = Layer::Confirm;
                    }
                }
            }
            CampKey::Cancel if !repeat => self.back(sounds),
            _ => {}
        }
    }

    fn key_list(&mut self, key: CampKey, repeat: bool, sounds: &mut Vec<UiSound>) {
        let list = if self.screen == Screen::Photo { &mut self.photo } else { &mut self.goals };
        let moved = match key {
            CampKey::Up | CampKey::Down => list.move_anchor(key == CampKey::Down, repeat),
            CampKey::R1 => list.next_page(repeat),
            CampKey::L1 => list.prev_page(repeat),
            CampKey::Cancel if !repeat => {
                self.back(sounds);
                return;
            }
            _ => false,
        };
        if moved {
            sounds.push(UiSound::Play(SE_MENU_CURSOR));
            self.seen();
        }
    }

    fn key_materials(&mut self, key: CampKey, repeat: bool, sounds: &mut Vec<UiSound>) {
        let max = self.material_line_max();
        let line = self.material_line;
        let to = match key {
            CampKey::Down if line < max => line + 1,
            CampKey::Down if !repeat => 0,
            CampKey::Up if line > 0 => line - 1,
            CampKey::Up if !repeat => max,
            CampKey::R1 if line < max => (line + MATERIAL_LINES).min(max),
            CampKey::R1 if !repeat => 0,
            CampKey::L1 if line > 0 => line.saturating_sub(MATERIAL_LINES),
            CampKey::L1 if !repeat => max,
            CampKey::Cancel if !repeat => {
                self.back(sounds);
                return;
            }
            _ => line,
        };
        if to != line {
            self.material_line = to;
            sounds.push(UiSound::Play(SE_MENU_CURSOR));
        }
    }

    fn key_confirm(&mut self, key: CampKey, repeat: bool, sounds: &mut Vec<UiSound>) {
        match key {
            CampKey::Left | CampKey::Right | CampKey::Up | CampKey::Down if !repeat => {
                self.confirm = 1 - self.confirm.min(1);
                sounds.push(UiSound::Play(SE_MENU_CURSOR));
            }
            CampKey::Decide if !repeat && self.confirm == 0 => match self.screen {
                Screen::Synthesis => self.synthesize(sounds),
                Screen::Forge => self.power_up(sounds),
                _ => self.layer = Layer::Main,
            },
            CampKey::Decide if !repeat => {
                sounds.push(UiSound::Play(SE_MENU_CLOSE));
                self.layer = Layer::Main;
            }
            // KH3: B does nothing on the synthesis confirmation (mesWin2); the Forge's system Yes / No takes it as No
            CampKey::Cancel if !repeat && self.screen == Screen::Forge => {
                sounds.push(UiSound::Play(SE_MENU_CLOSE));
                self.layer = Layer::Main;
            }
            _ => {}
        }
    }

    /// YES on the synthesis: asked of the host (its next data is the truth; KH3's large item-get popup follows).
    fn synthesize(&mut self, sounds: &mut Vec<UiSound>) {
        self.layer = Layer::Main;
        let Some(row) = self.recipe().cloned() else { return };
        let count = self.qty_of(&row).min(row.max);
        if count == 0 {
            sounds.push(UiSound::Play(SE_MENU_REFUSED));
            return;
        }
        sounds.push(UiSound::Play(SE_MENU_DECIDE));
        self.events.push(WorkshopEvent::Synthesize { id: row.id.clone(), count });
        // shown at once: the clear mark, the stock, the quantity back to 1
        for r in self.data.recipes.iter_mut().filter(|r| r.id == row.id) {
            r.made += count;
            r.held += count;
        }
        self.qty.remove(&row.id);
        self.synth.invalidate();
    }

    /// YES on the Forge's question: the power-up is asked of the host and its effects play.
    fn power_up(&mut self, sounds: &mut Vec<UiSound>) {
        let Some(row) = self.forge_row().cloned() else {
            self.layer = Layer::Main;
            return;
        };
        let Some(next) = row.next.as_ref().filter(|n| n.ready) else {
            self.layer = Layer::Main;
            sounds.push(UiSound::Play(SE_MENU_REFUSED));
            return;
        };
        sounds.push(UiSound::Play(SE_FORGE));
        self.events.push(WorkshopEvent::Forge { key: row.key.clone() });
        self.enhance = Some(next.level);
        self.layer = Layer::Effect;
        // shown at once: the level and the numbers (the host's next data is the truth)
        if let Some(r) = self.data.keyblades.get_mut(self.keyblade) {
            r.level = next.level;
            r.strength = next.strength;
            r.magic = next.magic;
            if next.level >= r.max_level {
                r.next = None;
            } else if let Some(n) = r.next.as_mut() {
                // (the host's next data names the level after this one)
                n.ready = false;
            }
        }
    }

    /// The power-up's effects ended (the view's `ANIM_END_KEY_ENHANCE_EFFECT`): the pad comes back.
    pub fn effect_done(&mut self) {
        if self.layer == Layer::Effect {
            self.layer = Layer::Main;
        }
    }

    pub fn take_events(&mut self) -> Vec<WorkshopEvent> {
        std::mem::take(&mut self.events)
    }
}

#[cfg(test)]
mod tests;
