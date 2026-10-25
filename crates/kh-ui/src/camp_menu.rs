//! The camp menu's DISPLAY: the `15_campmenu/CampMenu` movie (a "bound" movie: the game's native
//! parts set text and frames on its clips directly; docs/sora-ui-spec.md 10.3 - 10.8) driven
//! from the logic of `camp.rs`, `camp_equip.rs` and `camp_custom.rs`.
//!
//! DRAWN FROM THE MOVIES: the open / close animations, the top screen with the character list,
//! Stats, Abilities (list widget with scroll bar, AP numbers and gauges, help line), Config
//! (the 26 rows of `ConfigLibrary` with the native window scroll, sliders, scroll bar, help
//! line), Equipment (equipped list, stock list, preview pane, ability list), Items (tabs, list,
//! target buttons), Customize (the three layers of buttons and the shortcut list with tabs),
//! Game Help (two lists, and the `GameHelp` movie for a topic's pages), the confirmation
//! dialogs (`Dialog` movie) and the button guide of the frame.
//!
//! WHERE THE DISPLAY IS NOT THE GAME'S (each marked LOCAL below): clip labels of rows and
//! panes were chosen from the label names (the native list fillers were not read: spec 13,
//! item 10 h); the item status panes `ItemStatusA1` / `A2` show the help text and the item picture (IMPL_ER37, `item_art`); the dialog window
//! keeps its authored size (the AS3 `Dialog.setText` layout is not ported);
//! character images and models, world title and area name are not drawn.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::camp::{CampMenu, CampScreen, Dialog, DialogKind, TopEntry, CONFIG_VISIBLE};
use crate::camp_custom::{CustomLayer, HelpLayer, GROUP_ITEM, TAB_KEYS};
use crate::camp_equip::{EquipLayer, CAT_ACCESSORY, CAT_ARMOUR, CAT_ITEM, CAT_KEY, CAT_MATERIAL, CAT_WEAPON, ITEM_TABS, KIND_ACCESSORY, KIND_ARMOUR, KIND_ITEM, KIND_WEAPON};
use crate::def::{MovieDef, Pack, PackError};
use crate::geom::Matrix;
use crate::list::{set_y, RowState, ScrollBar, TabBar};
use crate::player::Movie;
use crate::render::Fonts;

/// The library movies the camp menu places symbols of (a library may use the ones before it).
pub const LIBRARIES: [&str; 5] = ["MainLibrary", "ItemListLibrary", "ItemStatusLibrary", "ConfigLibrary", "SystemLibrary"];

const TOP: &str = "topMenu";

/// (party) The top screen's panel of party slot `n` (1 .. 5).
fn panel_path(n: usize) -> String {
    format!("topMenu.statusAnm.status.charaWindow0{}", n.clamp(1, 5))
}
const FRAME: &str = "stageFrame.stageObjR";
const STATS: &str = "status.statusAnm";
const GUIDE: &str = "stageFrame.stageObjR.btnInfo.btnInfoText.txText.textField";
const ABILITY: &str = "ability.abilityAnm";
const AP_PANE: &str = "ability.abilityAnm.selectCharaImage1.abilityParameterAnm.abilityParameter";
const CONFIG_PACK: &str = "config.configMenu.configPack";
const EQ_DETAIL: &str = "equip.equipDetailAnm";
const EQ_REPLACE: &str = "equip.equipReplaceAnm";
const BACKYARD: &str = "backyard.backyardAnm";
const CUSTOM: &str = "customize";

/// The category icon label of an entry: `camp1` .. `camp9` follow the numbering of the text keys
/// `kh3_camp.top.menu_0001` .. (Items, Equipment, Abilities, Customize, Stats, Gummiphone, Game
/// Help, Config, then Cuisine) [read off the icons' art; the native table was not read].
fn icon(entry: TopEntry) -> &'static str {
    match entry {
        TopEntry::Items => "camp1",
        TopEntry::Equipment => "camp2",
        TopEntry::Abilities => "camp3",
        TopEntry::Customize => "camp4",
        TopEntry::Stats => "camp5",
        TopEntry::Gummiphone => "camp6",
        TopEntry::GameHelp => "camp7",
        TopEntry::Config => "camp8",
        TopEntry::Cuisine => "camp9",
        // (er-host, IMPL_ER21) the save-point menu's sixth icon [LOCAL: KH3's camp menu has no such entry]
        TopEntry::TitleScreen => "save6",
        // (er-host, IMPL_ER59) the outfits: Customize's icon [LOCAL: KH3's camp menu has no such entry]
        TopEntry::Outfits => "camp4",
        // (party P) the save point's Party Formation: Customize's icon [LOCAL]
        TopEntry::PartyFormation => "camp4",
    }
}

/// Text keys of an entry: `kh3_camp.top.menu_* / detail_*` (numbered as the icons).
fn entry_keys(entry: TopEntry) -> (&'static str, &'static str) {
    match entry {
        TopEntry::Items => ("kh3_camp.top.menu_0001", "kh3_camp.top.detail_0001"),
        TopEntry::Equipment => ("kh3_camp.top.menu_0002", "kh3_camp.top.detail_0002"),
        TopEntry::Abilities => ("kh3_camp.top.menu_0003", "kh3_camp.top.detail_0003"),
        TopEntry::Customize => ("kh3_camp.top.menu_0004", "kh3_camp.top.detail_0004"),
        TopEntry::Stats => ("kh3_camp.top.menu_0005", "kh3_camp.top.detail_0005"),
        TopEntry::Gummiphone => ("kh3_camp.top.menu_0006", "kh3_camp.top.detail_0006"),
        TopEntry::GameHelp => ("kh3_camp.top.menu_0007", "kh3_camp.top.detail_0007"),
        TopEntry::Config => ("kh3_camp.top.menu_0008", "kh3_camp.top.detail_0008"),
        TopEntry::Cuisine => ("kh3_camp.top.menu_0020", "kh3_camp.top.detail_0020"),
        // (er-host, IMPL_ER21) the save-point menu's own entry and help line
        TopEntry::TitleScreen => ("kh3_menu.savepoint.menu_0005", "kh3_menu.savepoint.detail_0006"),
        // (er-host, IMPL_ER59) LOCAL text, set by `open` (no key of the game's)
        TopEntry::Outfits => ("", ""),
        // (party P) LOCAL text (camp::FORMATION_TITLE)
        TopEntry::PartyFormation => ("", ""),
    }
}

/// The sub screen's clip, its title key and its icon label.
fn sub_clip(screen: CampScreen) -> Option<(&'static str, &'static str, &'static str, &'static str)> {
    // (clip that plays in / out, clip that holds stageCategory + helpText, title key, icon)
    Some(match screen {
        CampScreen::Equipment => (EQ_DETAIL, EQ_DETAIL, "kh3_camp.equip.title_0010", "camp2"),
        CampScreen::Abilities => ("ability", "ability", "kh3_camp.ability.title_0010", "camp3"),
        CampScreen::Items => ("backyard", "backyard", "kh3_camp.item.title_0001", "camp1"),
        CampScreen::Customize => (CUSTOM, CUSTOM, "kh3_camp.customize.title_0001", "camp4"),
        CampScreen::Stats => ("status", "status", "kh3_camp.status.title_0010", "camp5"),
        CampScreen::GameHelp => ("gameHelp", "gameHelp", "kh3_camp.gamehelp.title_0010", "camp7"),
        CampScreen::Config => ("config", "config", "kh3_camp.config.title_0001", "camp8"),
        // (er-host, IMPL_ER59) the outfits: the Game Help screen (its category list), title LOCAL (set by change_screen)
        CampScreen::Outfits => ("gameHelp", "gameHelp", "", "camp4"),
        _ => return None,
    })
}

/// (IMPL_ER34, the user: "leaving the equipment menu is glitchy") THE LABEL A SUB SCREEN'S CLIP PLAYS WHEN THE PLAYER
/// GOES BACK TO THE TOP SCREEN. The movie's convention, read from its timelines (kh-ui example `anim_dump`): a screen
/// the player leaves by going DEEPER shrinks away (scale 1 -> 0.78, alpha 1 -> 0.1) and the deeper one comes in from
/// 1.5; going BACK the deeper screen grows away (1 -> 1.45) and the one underneath grows back from 0.75 (`back`).
/// Every sub screen's only `out` is the growing one, except the Equipment's slot list, which has something deeper
/// (the stock list): its `out` is the SHRINKING one (to the stock list) and its `out2` the growing one (back to the
/// top). The port played `out` when leaving Equipment and `out2` when opening the stock list: both the wrong way
/// round, so on the way out the slot list shrank into the very place the top screen grows back from.
pub fn leave_label(screen: CampScreen) -> &'static str {
    match screen {
        CampScreen::Equipment => "out2",
        _ => "out",
    }
}

/// (IMPL_ER37, the user: "fix the menu when leaving the equipment menu without selecting a character") THE LABEL THE
/// TOP SCREEN'S CLIP (`topMenu`) PLAYS BETWEEN TWO SCREENS, `None` = it is not animated. Its timeline (kh-ui example
/// `anim_dump topMenu ?`): `inChara` 71 (entries shrink to 0.75, the character list comes from 1.5), `loopChara` 81,
/// `outChara` 91 (the character list shrinks to 0.75: on to the sub screen), `backChara` 103 (the character list grows
/// back from 0.75 and STAYS: back from the sub screen to the character list), `outChara2` 115 (the character list grows
/// away to 1.45 and the entries come back from 0.75, then `loop`: back from the character list to the top).
/// KH3 0x1415BDA30 (verdict same) leaves the character step with `outChara2` when backing out and `outChara` when going
/// on (strings 0x146A6DBB8 / static 0x140108C40, 0x140108C90). The port played `backChara` for B on the character list:
/// the list came back and stayed, the entries stayed invisible, the status panel (and Sora's portrait in it) blinked off.
pub fn top_label(was: CampScreen, now: CampScreen) -> Option<&'static str> {
    let top_like = |s: CampScreen| matches!(s, CampScreen::Top | CampScreen::Character(_));
    match (was, now) {
        (CampScreen::Top, CampScreen::Character(_)) => Some("inChara"),
        (CampScreen::Character(_), CampScreen::Top) => Some("outChara2"),
        (CampScreen::Character(_), n) if !top_like(n) => Some("outChara"),
        (w, CampScreen::Character(_)) if !top_like(w) => Some("backChara"),
        (CampScreen::Top, n) if !top_like(n) => Some("out"),
        (w, CampScreen::Top) if !top_like(w) => Some("back"),
        _ => None,
    }
}

/// The Equipment's slot list gives way to the stock list (see [`leave_label`]).
pub const DETAIL_TO_STOCK: &str = "out";

/// The numbers of the Config rows' text keys (`kh3_camp.config.config_NNNN`, choices and details
/// count up from the same number).
const CONFIG_KEYS: [u32; 26] = [10, 20, 30, 40, 50, 60, 70, 75, 80, 85, 90, 100, 110, 120, 130, 140, 150, 160, 170, 180, 190, 200, 205, 210, 220, 230];

/// The icon text of an item category (`UI/IconText` rows) [LOCAL: chosen by the icons' names].
fn category_icon(category: u8) -> &'static str {
    match category {
        CAT_ITEM => "<icon(BattleItem)>",
        CAT_WEAPON => "<icon(Weapon)>",
        CAT_ARMOUR => "<icon(Armor)>",
        CAT_ACCESSORY => "<icon(Accessory)>",
        CAT_MATERIAL => "<icon(MixItem)>",
        CAT_KEY => "<icon(KeyItem)>",
        _ => "",
    }
}

fn kind_icon(kind: u8) -> &'static str {
    match kind {
        KIND_WEAPON => "<icon(Weapon)>",
        KIND_ARMOUR => "<icon(Armor)>",
        KIND_ACCESSORY => "<icon(Accessory)>",
        _ => "<icon(BattleItem)>",
    }
}

fn stamp_of(value: &impl std::fmt::Debug) -> u64 {
    let mut h = DefaultHasher::new();
    format!("{value:?}").hash(&mut h);
    h.finish()
}

pub struct CampMenuView {
    pub movie: Movie,
    pub fonts: Fonts,
    strings: HashMap<String, String>,
    open: bool,
    closing: bool,
    shown_cursor: Option<usize>,
    /// (party) The character list's focused button as shown.
    shown_chara: Option<usize>,
    /// (party P) the Party Formation texts shown (their stamp)
    formation_stamp: Option<u32>,
    shown_screen: CampScreen,
    /// The frame's clips exist only once their timelines placed them.
    frame_filled: bool,
    /// What the sub screen last showed.
    stamp: u64,
    guide: String,
    item_tabs: TabBar,
    custom_tabs: TabBar,
    config_bar: ScrollBar,
    config_pack_y: Option<f32>,
    /// `Slider.m_thumbOffset` per slider row.
    config_thumb: HashMap<usize, f32>,
    /// (IMPL_ER21) `m_originalGaugeWidth` of each slider, as the gauge's authored horizontal scale (taken once).
    config_gauge: HashMap<usize, f32>,
    equip_layer: EquipLayer,
    custom_layer: CustomLayer,
    items_target: bool,
    /// The `GameHelp` movie (a topic's pages) and the `Dialog` movie, each its own layer.
    page: Movie,
    page_open: bool,
    page_closing: bool,
    dialog: Movie,
    dialog_open: bool,
    dialog_closing: bool,
    dialog_stamp: u64,
    /// (IMPL_ER37) The item pictures and help-text ids of the pack, and the item each status pane shows (by pane path).
    pub art: crate::item_art::ItemArt,
    pane_item: HashMap<String, Option<String>>,
    /// (party PF) the icon-based Party Formation screen: its pictures, and the screen while it is up
    formation_art: crate::party_formation::FormationArt,
    formation_scene: Option<crate::party_formation::FormationScene>,
}

/// (party PF) The clips of the camp's top screen the Party Formation screen hides (the character list, the status panels
/// and the 3D pictures placed on them, the entries, the goal line): its own icons stand there.
const FORMATION_HIDES: [&str; 4] = ["topMenu.statusAnm", "topMenu.charaList", "topMenu.cmdBtnTopMenu", "topMenu.nextGoal"];

/// (party PF) The help line's clip (its colour flashes on a refused edit).
const FORMATION_HELP: &str = "topMenu.helpText.txHelpText";

/// (party PF) The Party Formation screen's button guide (the pad's glyphs, Xbox layout by default: A, Y, B) [L].
pub const FORMATION_GUIDE: &str = "<icon(Decide)> Add / Remove  <icon(Triangle)> Make Leader  <icon(Cancel)> Back";

impl CampMenuView {
    /// The pack's movie name.
    pub const MOVIE: &'static str = "CampMenu";

    pub fn load(pack: &mut Pack, fonts: Fonts) -> Result<CampMenuView, PackError> {
        let def: Arc<MovieDef> = pack.movie_linked(Self::MOVIE, &LIBRARIES)?;
        let page = pack.movie_linked("GameHelp", &LIBRARIES)?;
        let dialog = pack.movie_linked("Dialog", &LIBRARIES)?;
        let mut strings = HashMap::new();
        for table in ["kh3_camp", "kh3_system", "kh3_gamehelp", "kh3_item", "kh3_menu"] {
            strings.extend(pack.string_table(table));
        }
        let art = crate::item_art::ItemArt::load(pack);
        let formation_art = crate::party_formation::FormationArt::load(pack);
        Ok(CampMenuView {
            movie: Movie::new(def),
            fonts,
            strings,
            open: false,
            closing: false,
            shown_cursor: None,
            shown_chara: None,
            formation_stamp: None,
            shown_screen: CampScreen::Top,
            frame_filled: false,
            stamp: 0,
            guide: String::new(),
            item_tabs: TabBar::new(format!("{BACKYARD}.backyardItem.listTabCamp")),
            custom_tabs: TabBar::new(format!("{CUSTOM}.shortcut3rdLayer.shortcutChangeListAnm.listTabCampB")),
            config_bar: ScrollBar::new("config.configMenu.scrollbar"),
            config_pack_y: None,
            config_thumb: HashMap::new(),
            config_gauge: HashMap::new(),
            equip_layer: EquipLayer::Slots,
            custom_layer: CustomLayer::Top,
            items_target: false,
            page: Movie::new(page),
            page_open: false,
            page_closing: false,
            dialog: Movie::new(dialog),
            dialog_open: false,
            dialog_closing: false,
            dialog_stamp: 0,
            art,
            pane_item: HashMap::new(),
            formation_art,
            formation_scene: None,
        })
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// The Game Help page movie while a page is up.
    pub fn page_movie(&self) -> Option<&Movie> {
        self.page_open.then_some(&self.page)
    }

    /// The dialog movie while a dialog is up.
    pub fn dialog_movie(&self) -> Option<&Movie> {
        self.dialog_open.then_some(&self.dialog)
    }

    /// An English string of the pack with `<br>` as line breaks ("" when the pack has none).
    pub fn string(&self, key: &str) -> String {
        self.strings.get(key).map(|s| s.replace("<br>", "\n")).unwrap_or_default()
    }

    /// (IMPL_ER23B) THE HELP TEXT OF AN ITEM OR A PIECE OF EQUIPMENT, by its shown name: the game's item tables name a
    /// text id (`kh3_item.<table>.<id>`) and the status pane shows `kh3_item.<table>.detail_<id>`; the port's rows
    /// carry the English name only, so the id is found by the name ("" when the pack has no such text).
    pub fn item_help(&self, name: &str) -> String {
        if name.is_empty() {
            return String::new();
        }
        let mut found: Vec<&String> = self.strings.iter().filter(|(k, v)| v.as_str() == name && k.starts_with("kh3_item.") && !k.contains(".detail_")).map(|(k, _)| k).collect();
        found.sort();
        found.iter().find_map(|key| {
            let (table, id) = key.rsplit_once('.')?;
            self.strings.get(&format!("{table}.detail_{id}")).map(|s| s.replace("<br>", "\n"))
        }).unwrap_or_default()
    }

    /// (IMPL_ER37) THE HELP TEXT OF A ROW: the host's own text, else the game's by the item key (`TresItemData.m_HelpTextID`,
    /// pack `data/item_icons.json`), else by the shown name ([`CampMenuView::item_help`]). "" = none (the pane goes off).
    pub fn piece_help(&self, id: &str, name: &str, own: &str) -> String {
        if !own.is_empty() {
            return own.to_string();
        }
        if let Some(text) = self.art.help_id(id).and_then(|key| self.strings.get(key)) {
            return text.replace("<br>", "
");
        }
        self.item_help(name)
    }

    /// (IMPL_ER37) AN ITEM STATUS PANE (`tres.rsl.ItemStatusA1` / `A2`; native proxy `UTresASProxyRSLItemStatusA` 0x141539E60
    /// binds `itemImage.imageAnm.coImage` and `txHelp`): the help text, and the item's picture loaded into `coImage` (a
    /// `tres.controls.Icon`) with `itemImage` playing `in` (fades in, stops in `loop`) when the item changes. LOCAL: the
    /// loading spinner `itemLoad` is never shown (the picture is there at once); an item without a picture (Elden Ring's own
    /// key items) shows its text only; the pane is `off` when there is neither. `item`: the host id, `None` = empty row.
    fn item_pane(&mut self, pane: &str, item: Option<&str>, help: &str) {
        let texture = item.and_then(|id| self.art.icon(id));
        let label = if help.is_empty() && texture.is_none() { "off" } else { "loop" };
        if self.movie.current_label(pane) != Some(label) {
            self.movie.goto_and_play(pane, label);
        }
        let spinner = format!("{pane}.itemLoad"); // (A2 has none)
        if self.movie.exists(&spinner) {
            self.movie.set_visible(&spinner, false);
        }
        // (IMPL_ER42) a long text (Elden Ring's descriptions) cut to what the field holds: see `crate::render::fit_text`
        let field = format!("{pane}.txHelp.textField");
        let help = match self.movie.node(&field).map(|n| &n.kind) {
            Some(crate::player::Kind::Text(t)) => crate::render::fit_text(&self.fonts, &t.def, help),
            _ => help.to_string(),
        };
        self.text(&field, &help);
        let shown = texture.as_ref().and(item).map(str::to_string);
        if self.pane_item.get(pane) != Some(&shown) {
            let image = format!("{pane}.itemImage");
            self.movie.set_visible(&image, shown.is_some());
            self.movie.goto_and_play(&image, if shown.is_some() { "in" } else { "off" });
            if let Some(sprite) = self.movie.sprite_mut(&format!("{image}.imageAnm.coImage")) {
                crate::controls::icon_load(sprite, texture);
            }
            self.pane_item.insert(pane.to_string(), shown);
        }
    }

    /// The Game Help topics of the pack's strings: categories by `kh3_camp.gamehelp.group_*`,
    /// topics `kh3_gamehelp.<table>.title_N` with their pages `detail_N_00 ..`.
    /// LOCAL: which topics a save has unlocked is game progress (`TresUI_SetPauseGameHelp` and
    /// the tutorials register them); the port lists every topic of the four tables whose
    /// category is evident from the group names. Classic Kingdom, Re Mind and the Gummi help
    /// are left out.
    pub fn help_categories(&self) -> Vec<crate::camp_custom::HelpCategory> {
        let mut out = Vec::new();
        for (group, table) in [("kh3_camp.gamehelp.group_0010", "system"), ("kh3_camp.gamehelp.group_0020", "player"), ("kh3_camp.gamehelp.group_0030", "battle"), ("kh3_camp.gamehelp.group_0040", "world")] {
            let prefix = format!("kh3_gamehelp.{table}.title_");
            let mut numbers: Vec<&str> = self.strings.keys().filter_map(|k| k.strip_prefix(&prefix)).collect();
            numbers.sort();
            let topics = numbers
                .into_iter()
                .map(|n| {
                    let pages = (0..20).map_while(|page| self.strings.get(&format!("kh3_gamehelp.{table}.detail_{n}_{page:02}")).cloned()).collect();
                    crate::camp_custom::HelpTopic { title: self.string(&format!("{prefix}{n}")), pages }
                })
                .collect();
            out.push(crate::camp_custom::HelpCategory { name: self.string(group), topics });
        }
        out
    }

    fn text(&mut self, path: &str, text: &str) {
        self.movie.set_text(path, text, &self.fonts);
    }

    fn key_text(&mut self, path: &str, key: &str) {
        let text = self.string(key);
        self.text(path, &text);
    }

    fn set_guide(&mut self, text: &str) {
        if self.guide != text {
            // `tres.rsl.BtnInfo`: the guide plate comes in with a text and goes with none.
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
    }

    /// Slot 74 `0x1415C8DD0`: the menu is shown (root `in`), the top screen is filled.
    pub fn open(&mut self, menu: &CampMenu) {
        self.movie.goto_and_play("", "in");
        self.pane_item.clear(); // (IMPL_ER37) the panes load their picture again
        self.open = true;
        self.closing = false;
        self.shown_cursor = None;
        self.shown_screen = CampScreen::Top;
        self.frame_filled = false;
        self.stamp = 0;
        self.guide.clear();
        // Entries: the shown ones fill the clips from the top; unused clips are invisible.
        for i in 0..9 {
            let button = format!("{TOP}.cmdBtnTopMenu.topMenu{}", i + 1);
            match menu.entries.get(i) {
                Some(entry) => {
                    let (name, detail) = entry_keys(*entry);
                    self.movie.set_visible(&button, true);
                    self.key_text(&format!("{button}.mainCmdNameAnm.mainCmdName"), name);
                    self.key_text(&format!("{button}.helpMessageAnm.helpMessage"), detail);
                    if *entry == TopEntry::Outfits {
                        self.text(&format!("{button}.mainCmdNameAnm.mainCmdName"), crate::camp::OUTFITS_TITLE);
                        self.text(&format!("{button}.helpMessageAnm.helpMessage"), crate::camp::OUTFITS_DETAIL);
                    }
                    self.movie.goto_and_stop(&format!("{button}.categoryIcon"), icon(*entry));
                    self.movie.goto_and_play(&button, "normal");
                }
                None => drop(self.movie.set_visible(&button, false)),
            }
        }
        // The frame: title "MENU", play time, munny.
        self.key_text(&format!("{TOP}.stageCategory.category.categoryName.txName"), "kh3_camp.top.title_0001");
        self.movie.goto_and_stop(&format!("{TOP}.stageCategory.category.categoryIcon"), "top");
        self.movie.goto_and_stop(&format!("{TOP}.stageCategory.stageObjL.bgCategoryIcon"), "top");
        self.movie.goto_and_play("topMenu.statusAnm", "loop");
        // (party) the panel layout by party size (`0x1486E0F50`: `one .. five`), each panel's mask likewise
        let layout = ["one", "two", "three", "four", "five"][menu.party.len().clamp(1, 5) - 1];
        self.movie.goto_and_stop("topMenu.statusAnm.status", layout);
        for n in 1..=5 {
            self.movie.goto_and_stop(&panel_path(n), &format!("{layout}Msk"));
        }
        self.text("topMenu.nextGoal.nextGoalTitle", "");
        self.text("topMenu.nextGoal.nextGoalInfo", "");
        self.fill_panel(menu);
        self.sync_top(menu);
        // (party PF) the save point's Party Formation: the icon screen from the first frame
        if let Some(f) = menu.formation.as_ref() {
            for path in FORMATION_HIDES {
                self.movie.set_visible(path, false);
            }
            self.text(&format!("{TOP}.stageCategory.category.categoryName.txName"), &crate::camp::FORMATION_TITLE.to_uppercase()); // (party PF) the strip in capitals, as KH3's
            self.movie.goto_and_stop(&format!("{TOP}.stageCategory.category.categoryIcon"), "camp4");
            self.movie.goto_and_stop(&format!("{TOP}.stageCategory.stageObjL.bgCategoryIcon"), "camp4");
            let help = f.help.clone();
            self.help_line(TOP, &help);
            self.formation_scene = Some(crate::party_formation::FormationScene::new(&self.formation_art, f, menu.chara_cursor));
        } else {
            self.leave_formation();
        }
    }

    /// (party PF) Back to the camp menu of its own: the hidden clips are the timeline's again, the icons go.
    fn leave_formation(&mut self) {
        for path in FORMATION_HIDES {
            if let Some(node) = self.movie.node_mut(path) {
                node.script_visible = None;
            }
        }
        crate::party_formation::clear(&mut self.movie);
        if self.formation_scene.take().is_some() {
            if let Some(node) = self.movie.node_mut(FORMATION_HELP) {
                node.cx = crate::geom::Cx::IDENTITY;
            }
        }
    }

    /// (party PF) The Party Formation screen while it is up (tests, pictures).
    pub fn formation_scene(&self) -> Option<&crate::party_formation::FormationScene> {
        self.formation_scene.as_ref()
    }

    /// The party panels (`0x1415D3530` -> `0x1415D0020`, one per `m_PartyParams` slot): name, LV, HP, MP, the EXP
    /// gauge; the unused panels are hidden.
    fn fill_panel(&mut self, menu: &CampMenu) {
        let sora = [menu.stats.clone()];
        let party: &[crate::camp::StatsIn] = if menu.party.is_empty() { &sora } else { &menu.party };
        for n in 1..=5usize {
            let panel = panel_path(n);
            let Some(s) = party.get(n - 1) else {
                self.movie.set_visible(&panel, false);
                continue;
            };
            self.movie.set_visible(&panel, true);
            for (field, value) in [("charaName", s.name.clone()), ("lv", s.level.to_string()), ("hpNow", s.hp.to_string()), ("hpMax", s.hp_max.to_string()), ("mpNow", s.mp.to_string()), ("mpMax", s.mp_max.to_string()), ("articleLv", "LV".into()), ("articleHp", "HP".into()), ("articleMp", "MP".into()), ("exp", "EXP".into()), ("statusArrowHp", String::new()), ("statusArrowMp", String::new())] {
                self.text(&format!("{panel}.{field}"), &value);
            }
            self.movie.goto_frame(&format!("{panel}.expGg"), s.panel_exp_frame().max(1) as u32);
        }
        if self.frame_filled {
            self.text(&format!("{FRAME}.munny.munnyNum.textField"), &party[0].munny.to_string());
        }
    }

    fn sync_top(&mut self, menu: &CampMenu) {
        if !self.frame_filled && self.movie.exists(&format!("{FRAME}.playTime.hour.textField")) {
            self.frame_filled = true;
            let time = menu.stats.play_time();
            let (hours, minutes) = time.split_once(':').unwrap_or(("0", "00"));
            self.text(&format!("{FRAME}.playTime.hour.textField"), hours);
            self.text(&format!("{FRAME}.playTime.min.textField"), minutes);
            self.text(&format!("{FRAME}.playTime.txPlayTime.coText"), "Time");
            self.text(&format!("{FRAME}.munny.munnyNum.textField"), &menu.stats.munny.to_string());
            self.text(&format!("{FRAME}.munny.txMunny.coText"), "Munny");
            self.text(&format!("{FRAME}.areaInfo.areaName.txName"), "");
            // (IMPL_ER23B) the world title's picture (`CoImgWorldTitle`: an image the game loads for the world; the pack
            // holds none): its authoring placeholder, a faint red rectangle top right, is not shown.
            self.movie.set_visible(&format!("{FRAME}.areaInfo.worldTitle"), false);
        }
        if self.shown_cursor != Some(menu.top_cursor) {
            if let Some(old) = self.shown_cursor {
                self.movie.goto_and_play(&format!("{TOP}.cmdBtnTopMenu.topMenu{}", old + 1), "selectOut");
            }
            self.movie.goto_and_play(&format!("{TOP}.cmdBtnTopMenu.topMenu{}", menu.top_cursor + 1), "select");
            self.shown_cursor = Some(menu.top_cursor);
        }
    }

    /// Reflects the logic's state: the focus, the screen that is up, its rows and panes.
    pub fn sync(&mut self, menu: &mut CampMenu) {
        if !self.open || self.closing {
            return;
        }
        self.sync_top(menu);
        if self.shown_screen != menu.screen {
            self.change_screen(menu);
        }
        // (party) the character list's cursor
        if let CampScreen::Character(_) = menu.screen {
            if self.shown_chara != Some(menu.chara_cursor) {
                if let Some(old) = self.shown_chara {
                    self.movie.goto_and_play(&format!("{TOP}.charaList.btnChara{}", old + 1), "selectOut");
                }
                self.movie.goto_and_play(&format!("{TOP}.charaList.btnChara{}", menu.chara_cursor + 1), "select");
                self.shown_chara = Some(menu.chara_cursor);
            }
        }
        // (party P) Party Formation's texts changed (an edit): the rows, the help line and the panels again
        if let (CampScreen::Character(TopEntry::PartyFormation), Some(f)) = (menu.screen, menu.formation.as_ref()) {
            if self.formation_stamp != Some(f.stamp) && self.shown_screen == menu.screen {
                self.formation_stamp = Some(f.stamp);
                for (i, row) in f.rows.iter().enumerate().take(5) {
                    self.text(&format!("{TOP}.charaList.btnChara{}.charaName", i + 1), row);
                }
                let help = f.help.clone();
                self.help_line(TOP, &help);
                self.fill_panel(menu);
            }
        }
        // (party PF) the icon screen follows the feed and the cursor; the guide once the frame has its plate
        if let (Some(scene), Some(f)) = (self.formation_scene.as_mut(), menu.formation.as_ref()) {
            scene.set(f, menu.chara_cursor);
            if self.frame_filled && matches!(menu.screen, CampScreen::Character(TopEntry::PartyFormation)) {
                self.set_guide(FORMATION_GUIDE);
            }
        }
        // (party) a member switch on Equipment / Abilities / Stats: the screen shows the new member
        if menu.member_flash.take().is_some() {
            self.stamp = 0;
            self.pane_item.clear();
            if menu.screen == CampScreen::Stats {
                self.fill_stats(menu);
            }
        }
        match menu.screen {
            CampScreen::Abilities => self.sync_abilities(menu),
            CampScreen::Config => self.sync_config(menu),
            CampScreen::Equipment => self.sync_equipment(menu),
            CampScreen::Items => self.sync_items(menu),
            CampScreen::Customize => self.sync_customize(menu),
            CampScreen::GameHelp => self.sync_help(menu),
            CampScreen::Outfits => self.sync_outfits(menu),
            _ => {}
        }
        self.sync_dialog(menu.dialog.as_ref());
    }

    /// The transition between two screens: the entries play `out` and the sub menu `in`, and
    /// back; the character list between them.
    fn change_screen(&mut self, menu: &CampMenu) {
        let (was, now) = (self.shown_screen, menu.screen);
        self.shown_screen = now;
        self.pane_item.clear(); // (IMPL_ER37) the screen's panes load their picture again
        self.stamp = 0;
        let top_like = |s: CampScreen| matches!(s, CampScreen::Top | CampScreen::Character(_));
        match (was, now) {
            (CampScreen::Top, CampScreen::Character(_)) => {
                self.movie.goto_and_play(TOP, top_label(was, now).unwrap_or("inChara"));
                // (party) one button per party slot, the cursor's `select`, the others `normal`
                let names: Vec<String> = if menu.party.is_empty() { vec![menu.stats.name.clone()] } else { menu.party.iter().map(|s| s.name.clone()).collect() };
                // (party P) Party Formation: the roster rows, not the party
                let names: Vec<String> = menu.formation.as_ref().map_or(names, |f| f.rows.clone());
                for (i, name) in names.iter().enumerate().take(5) {
                    let button = format!("{TOP}.charaList.btnChara{}", i + 1);
                    self.movie.set_visible(&button, true);
                    self.text(&format!("{button}.charaName"), name);
                    self.movie.goto_and_play(&button, if i == menu.chara_cursor { "select" } else { "normal" });
                }
                self.shown_chara = Some(menu.chara_cursor);
                // (cleanup pass) The step's own header and help line [C 0x1415CB3E0 Equipment,
                // 0x1415C9D80 Abilities, 0x1415CB8D0 Stats]: the help text object gets the
                // step's text and `m_StageCategory2` (`topMenu.stageCategory2`, 0x1415C0470)
                // the icon index and the title of the sub menu (0x1415476B0(category, n, text)).
                let step = match now {
                    CampScreen::Character(TopEntry::Equipment) => Some(("kh3_camp.equip.char_0001", "kh3_camp.equip.title_0010", "camp2")),
                    CampScreen::Character(TopEntry::Abilities) => Some(("kh3_camp.ability.detail_0010", "kh3_camp.ability.title_0010", "camp3")),
                    CampScreen::Character(TopEntry::Stats) => Some(("kh3_camp.status.detail_0010", "kh3_camp.status.title_0010", "camp5")),
                    _ => None,
                };
                // (party P) Party Formation: its own title and help line (LOCAL texts), Customize's icon
                if let (CampScreen::Character(TopEntry::PartyFormation), Some(f)) = (now, menu.formation.as_ref()) {
                    let help = f.help.clone();
                    self.help_line(TOP, &help);
                    self.text(&format!("{TOP}.stageCategory2.category.categoryName.txName"), &crate::camp::FORMATION_TITLE.to_uppercase()); // (party PF) capitals
                    self.movie.goto_and_stop(&format!("{TOP}.stageCategory2.category.categoryIcon"), "camp4");
                    self.movie.goto_and_stop(&format!("{TOP}.stageCategory2.stageObjL.bgCategoryIcon"), "camp4");
                    self.formation_stamp = Some(f.stamp);
                }
                if let Some((help, title, icon)) = step {
                    let help = self.string(help);
                    self.help_line(TOP, &help);
                    self.key_text(&format!("{TOP}.stageCategory2.category.categoryName.txName"), title);
                    self.movie.goto_and_stop(&format!("{TOP}.stageCategory2.category.categoryIcon"), icon);
                    self.movie.goto_and_stop(&format!("{TOP}.stageCategory2.stageObjL.bgCategoryIcon"), icon);
                }
                for n in names.len().max(1) + 1..=5 {
                    self.movie.set_visible(&format!("{TOP}.charaList.btnChara{n}"), false);
                }
            }
            // (IMPL_ER37) back out of the character step: `outChara2`, not `backChara` (see [`top_label`])
            (CampScreen::Character(_), CampScreen::Top) => drop(self.movie.goto_and_play(TOP, top_label(was, now).unwrap_or("outChara2"))),
            _ => {}
        }
        if top_like(was) && !top_like(now) {
            self.movie.goto_and_play(TOP, top_label(was, now).unwrap_or("out"));
            if let Some((clip, holder, title, icon)) = sub_clip(now) {
                self.movie.goto_and_play(clip, "in");
                self.key_text(&format!("{holder}.stageCategory.category.categoryName.txName"), title);
                if now == CampScreen::Outfits {
                    self.text(&format!("{holder}.stageCategory.category.categoryName.txName"), crate::camp::OUTFITS_TITLE);
                }
                self.movie.goto_and_stop(&format!("{holder}.stageCategory.category.categoryIcon"), icon);
                self.movie.goto_and_stop(&format!("{holder}.stageCategory.stageObjL.bgCategoryIcon"), icon);
            }
            self.equip_layer = EquipLayer::Slots;
            self.custom_layer = CustomLayer::Top;
            self.items_target = false;
            if now == CampScreen::Stats {
                self.fill_stats(menu);
            }
        } else if !top_like(was) && top_like(now) {
            self.leave_sub(was);
            self.movie.goto_and_play(TOP, top_label(was, now).unwrap_or("back"));
            self.fill_panel(menu);
            self.set_guide("");
        }
    }

    fn leave_sub(&mut self, was: CampScreen) {
        if let Some((clip, ..)) = sub_clip(was) {
            self.movie.goto_and_play(clip, leave_label(was));
        }
        match was {
            CampScreen::Equipment => {
                // Whatever layer was up goes with the screen.
                self.movie.goto_and_play(EQ_REPLACE, "off");
                self.movie.goto_and_play("equip.abilityListVisibleAnm", "off");
                self.movie.goto_and_play("equip.selectEquipAnm", "off");
            }
            CampScreen::GameHelp => self.close_page(),
            _ => {}
        }
    }

    fn help_line(&mut self, holder: &str, text: &str) {
        self.text(&format!("{holder}.helpText.txHelpText.textField"), text);
    }

    // ------------------------------------------------------------------------- Stats (10.7)

    /// The Stats screen's filler `0x1415E8E00`.
    fn fill_stats(&mut self, menu: &CampMenu) {
        let s = menu.stats.clone();
        let cap = |v: i32| v.min(999).to_string();
        let help = self.string("kh3_camp.status.detail_0020").replace("{0}", &s.name);
        self.help_line("status", &help);
        let list1 = format!("{STATS}.statusList1");
        for (field, value) in [("articleLv", self.string("kh3_camp.status.item_0010")), ("articleAp", "AP".into()), ("articleExp", self.string("kh3_camp.status.item_0020")), ("articleNextLv", self.string("kh3_camp.status.item_0030")), ("lvNum", s.level.to_string()), ("apNum", cap(s.ap_max)), ("expNum", s.exp.to_string()), ("nextLvNum", s.next_level().to_string())] {
            self.text(&format!("{list1}.{field}"), &value);
        }
        self.movie.goto_frame(&format!("{list1}.statusExpGg"), s.stats_exp_frame().max(1) as u32);
        let list2 = format!("{STATS}.statusList2");
        let focus = if s.focus_unlocked { (crate::camp::third_gauge_label(&s.name).to_string() /* (GOOFY-GG) */, cap(s.focus), cap(s.focus_max), "normal") } else { (self.string("kh3_camp.status.item_0040"), "—".to_string(), "—".to_string(), "focus") };
        for (n, name, now, max) in [(1, "HP".to_string(), cap(s.hp), cap(s.hp_max)), (2, "MP".to_string(), cap(s.mp), cap(s.mp_max)), (3, focus.0, focus.1, focus.2)] {
            let row = format!("{list2}.statusValue{n}");
            self.text(&format!("{row}.statusItemName.textField"), &name);
            self.text(&format!("{row}.statusValueNum1.textField"), &now);
            self.text(&format!("{row}.statusValueNum2.textField"), &max);
            if n != 3 {
                self.text(&format!("{row}.statusArrow.textField"), "");
            }
            self.movie.goto_and_stop(&row, if n == 3 { focus.3 } else { "normal" });
        }
        for (n, name, icon, value) in [(4, "kh3_system.general.window_0110", "<icon(Power)>", cap(s.strength)), (5, "kh3_system.general.window_0120", "<icon(MagicPower)>", cap(s.magic)), (6, "kh3_system.general.window_0130", "<icon(Deffence)>", cap(s.defense))] {
            let row = format!("{list2}.statusValue{n}");
            self.key_text(&format!("{row}.title"), name);
            self.text(&format!("{row}.icon"), icon);
            self.text(&format!("{row}.statusValueNum1.textField"), &value);
            self.text(&format!("{row}.statusArrow.textField"), "");
            self.movie.goto_and_stop(&row, "normal");
        }
        let list3 = format!("{STATS}.statusList3");
        for (((clip, key), icon), value) in [("Fire", "kh3_system.general.window_0140"), ("Brizzard", "kh3_system.general.window_0150"), ("Thunder", "kh3_system.general.window_0160"), ("Water", "kh3_system.general.window_0170"), ("Aero", "kh3_system.general.window_0180"), ("Dark", "kh3_system.general.window_0190")].into_iter().zip(["Fire", "Blizzard", "Thunder", "Water", "Aero", "Dark"]).zip(s.resists) {
            self.key_text(&format!("{list3}.article{clip}.textField"), key);
            self.text(&format!("{list3}.icon{clip}"), &format!("<icon({icon})>"));
            let number = format!("{}{}Num", clip[..1].to_lowercase(), &clip[1..]);
            self.text(&format!("{list3}.{number}"), &format!("{}%", value.min(100)));
        }
        self.text(&format!("{STATS}.selectCharaImage2.charaName"), &s.name);
    }

    // --------------------------------------------------------------------- Abilities (10.4)

    fn sync_abilities(&mut self, menu: &mut CampMenu) {
        let a = &mut menu.abilities;
        let stamp = stamp_of(&(&a.list, a.used_ap, a.max_ap, a.rows.iter().map(|r| r.equipped).collect::<Vec<_>>())) ^ self.movie.exists(&a.list.item_path(0)) as u64;
        if stamp == self.stamp {
            return;
        }
        let fresh = self.stamp == 0;
        self.stamp = stamp;
        if fresh {
            self.movie.goto_and_play(&format!("{ABILITY}.abilityListTitleAnm"), "loop");
            self.key_text(&format!("{ABILITY}.abilityListTitleAnm.abilityListTitle.titleAbilityName"), "kh3_camp.ability.menu_0020");
            self.text(&format!("{ABILITY}.abilityListTitleAnm.abilityListTitle.titleAp"), "AP");
            self.movie.goto_and_play(&format!("{ABILITY}.selectEquipItemAnm"), "off");
            self.movie.goto_and_play(&format!("{ABILITY}.selectCharaImage1.abilityLimitAnm"), "off");
            self.movie.goto_and_play(&format!("{ABILITY}.selectCharaImage1.abilityParameterAnm"), "normal");
            self.text(&format!("{ABILITY}.selectCharaImage1.charaName"), &menu.stats.name.clone());
            self.text(&format!("{AP_PANE}.ap"), "AP");
            // The guide without "Show Next List" (those pages are not in the port).
            let guide = self.string("kh3_camp.ability.button_0012");
            let guide = guide.split("  <icon(Triangle)>").next().unwrap_or("").to_string();
            self.set_guide(&guide);
        }
        let empty = if a.rows.is_empty() { self.string("kh3_system.general.system_0013") } else { String::new() };
        self.text(&format!("{ABILITY}.textField"), &empty);
        // AP numbers and gauges: `m_NowUseApNum`, `m_MaxApNum`, `m_ApGauge` and the preview.
        let preview = a.preview(a.cursor());
        self.text(&format!("{AP_PANE}.apNow.apText.textField"), &preview.number.to_string());
        self.text(&format!("{AP_PANE}.apMax"), &a.max_ap.to_string());
        if self.movie.current_label(&format!("{AP_PANE}.apNow")) != Some(preview.label) {
            self.movie.goto_and_play(&format!("{AP_PANE}.apNow"), preview.label);
        }
        self.movie.goto_frame(&format!("{AP_PANE}.apGgBarAnm"), a.gauge_frame().max(1) as u32);
        let (plus, lack) = match (preview.preview_frame, preview.label) {
            (Some(frame), "lack") => (1, frame),
            (Some(frame), _) => (frame, 1),
            _ => (1, 1),
        };
        for (bar, blink, frame) in [("plusGgBarAnm", "plusGgBarBlink", plus), ("lackGgBarAnm", "lackGgBarBlink", lack)] {
            self.movie.goto_frame(&format!("{AP_PANE}.{bar}"), frame.max(1) as u32);
            let label = if frame > 1 { "loop" } else { "off" };
            if self.movie.current_label(&format!("{AP_PANE}.{bar}.{blink}")) != Some(label) {
                self.movie.goto_and_play(&format!("{AP_PANE}.{bar}.{blink}"), label);
            }
        }
        let help = a.rows.get(a.cursor()).map(|r| r.help.clone()).unwrap_or_default();
        self.help_line("ability", &help);
        // Rows.
        let groups = [self.string("kh3_camp.ability.group_0010"), self.string("kh3_camp.ability.group_0020"), self.string("kh3_camp.ability.group_0030")];
        let may: Vec<bool> = (0..a.rows.len()).map(|i| a.may_equip(i)).collect();
        let (rows, fonts) = (&a.rows, &self.fonts);
        a.list.refresh(&mut self.movie, |movie, path, index| {
            let row = &rows[index];
            let category = row.category.min(2) as usize;
            let label = match (row.first_of_category, category) {
                (true, 0) => "titleAction",
                (false, 0) => "lineAction",
                (true, 1) => "titleMove",
                (false, 1) => "lineMove",
                (true, _) => "titleSupport",
                (false, _) => "lineSupport",
            };
            movie.goto_and_stop(&format!("{path}.abilityCategoryTitle"), label);
            if row.first_of_category {
                movie.set_text(&format!("{path}.abilityCategoryTitle.abilityCategoryTitleColor.abilityCategoryName.textField"), &groups[category], fonts);
            }
            // LOCAL: the row icon by the icon names (`SetAbility` = installed).
            movie.set_text(&format!("{path}.textIcon.textField"), if row.equipped { "<icon(SetAbility)>" } else { "<icon(Ability)>" }, fonts);
            movie.goto_and_stop(&format!("{path}.abilityNameAnm"), if row.equipped || may[index] { "normal" } else { "lack" });
            movie.set_text(&format!("{path}.abilityNameAnm.abilityName.textField"), &row.name, fonts);
            movie.goto_and_stop(&format!("{path}.abilityPointAnm"), "loop");
            movie.set_text(&format!("{path}.abilityPointAnm.abilityPoint.textField"), &row.ap.to_string(), fonts);
            RowState::default()
        });
    }

    // ------------------------------------------------------------------------ Config (10.8)

    fn sync_config(&mut self, menu: &mut CampMenu) {
        if !self.movie.exists(&format!("{CONFIG_PACK}.setConfigValue1")) {
            return;
        }
        let stamp = stamp_of(&(&menu.options, menu.option_cursor, menu.option_scroll));
        if stamp == self.stamp {
            return;
        }
        let fresh = self.stamp == 0;
        self.stamp = stamp;
        if fresh {
            let guide = self.string("kh3_camp.config.button_0001");
            self.set_guide(&guide);
        }
        if self.config_pack_y.is_none() || fresh {
            // The pack's authored place; rows are 48 apart (the native table of row offsets).
            if let Some(node) = self.movie.node(CONFIG_PACK).filter(|n| !n.script_matrix) {
                self.config_pack_y = Some(node.matrix.ty);
            }
        }
        for n in 1..=26usize {
            let item = format!("{CONFIG_PACK}.setConfigValue{n}");
            let Some(row) = menu.options.get(n - 1).cloned() else {
                self.movie.set_visible(&item, false);
                continue;
            };
            self.movie.set_visible(&item, true);
            self.text(&format!("{item}.itemName"), &row.label);
            let selected = menu.option_cursor == n - 1;
            if row.choices.is_empty() {
                // `tres.controls.Slider`: thumb.x = bar.x + thumbOffset + bar.width * ratio; the
                // gauge is scaled by the ratio.
                self.text(&format!("{item}.slider.sliderBar.textField"), &row.value.to_string());
                let ratio = if row.range.1 > row.range.0 { (row.value.saturating_sub(row.range.0)) as f32 / (row.range.1 - row.range.0) as f32 } else { 0.0 };
                let offsets = &mut self.config_thumb;
                let gauges = &mut self.config_gauge;
                if let Some(bar) = self.movie.sprite_mut(&format!("{item}.slider.sliderBar")) {
                    let bar_node = bar.child("bar").map(|b| (b.matrix.tx, b.sprite().map(crate::list::local_bounds).map_or(0.0, |r| (r.x1 - r.x0) * b.matrix.a)));
                    if let Some((bar_x, bar_width)) = bar_node {
                        if let Some(gauge) = bar.child_mut("gauge") {
                            // (IMPL_ER21) `gauge.width = m_originalGaugeWidth * ratio` (tres.controls.Slider, read in the
                            // pack's ActionScript): the AUTHORED width times the ratio. It was written as a scale of
                            // `ratio` alone, which left a tick of the gauge whatever the value.
                            let authored = *gauges.entry(n).or_insert(gauge.matrix.a);
                            gauge.matrix.a = (authored * ratio).max(0.001);
                            gauge.script_matrix = true;
                        }
                        if let Some(thumb) = bar.child_mut("thumb") {
                            // m_thumbOffset = thumb.x - bar.x, taken once.
                            let offset = *offsets.entry(n).or_insert(thumb.matrix.tx - bar_x);
                            thumb.matrix.tx = bar_x + offset + bar_width * ratio;
                            thumb.script_matrix = true;
                        }
                    }
                }
            } else {
                self.text(&format!("{item}.selectValue"), &row.value_text());
                for (arrow, shown) in [("changeArrowAnmLeft", selected && row.stored && row.value > 0), ("changeArrowAnmRight", selected && row.stored && (row.value as usize) + 1 < row.choices.len())] {
                    let label = if shown { "loop" } else { "off" };
                    if self.movie.current_label(&format!("{item}.{arrow}")) != Some(label) {
                        self.movie.goto_and_play(&format!("{item}.{arrow}"), label);
                    }
                }
            }
            let label = match (selected, row.stored) {
                (true, _) => "select",
                (false, true) => "normal",
                (false, false) => "disable",
            };
            if self.movie.current_label(&item) != Some(label) {
                self.movie.goto_and_play(&item, label);
            }
        }
        // The window (`0x14154E060`): the pack moves up by the first shown row.
        if let Some(y) = self.config_pack_y {
            set_y(&mut self.movie, CONFIG_PACK, y - 48.0 * menu.option_scroll as f32);
        }
        let max = menu.options.len().saturating_sub(CONFIG_VISIBLE) as u32;
        self.config_bar.set(&mut self.movie, max, menu.option_scroll as u32, CONFIG_VISIBLE as f32 / menu.options.len().max(1) as f32, max > 0);
        // The help line of the focused row's value: `kh3_camp.config.detail_NNNN`.
        let help = menu
            .options
            .get(menu.option_cursor)
            .map(|row| {
                let base = CONFIG_KEYS.get(menu.option_cursor).copied().unwrap_or(0);
                let shown = row.value_text();
                let by_choice = (0..3).find(|k| !row.choices.is_empty() && self.strings.get(&format!("kh3_camp.config.choice_{:04}", base + k)) == Some(&shown));
                self.string(&format!("kh3_camp.config.detail_{:04}", base + by_choice.unwrap_or(0))).replace("{0}", &shown)
            })
            .unwrap_or_default();
        self.text("config.configMenu.helpText.txHelpText.textField", &help);
    }

    // --------------------------------------------------------------------- Equipment (10.5)

    fn sync_equipment(&mut self, menu: &mut CampMenu) {
        let stats = menu.stats.clone();
        let e = &mut menu.equipment;
        let stamp = stamp_of(&(&e.data, e.layer, &e.list, &e.stock, &e.abilities, &stats)) ^ self.movie.exists(&e.list.item_path(0)) as u64;
        if stamp == self.stamp {
            return;
        }
        let fresh = self.stamp == 0;
        self.stamp = stamp;
        let detail = format!("{EQ_DETAIL}.equipDetail");
        let replace = format!("{EQ_REPLACE}.equipReplace");
        if fresh {
            self.movie.goto_and_play(&format!("{detail}.equipListVisibleAnm"), "loop");
            self.text(&format!("{detail}.equipSelectCharaName.textField"), &stats.name);
        }
        // Layers. (IMPL_ER34) The detail clip's labels BY WHAT THEY ANIMATE (see `leave_label`): `out` = it SHRINKS
        // away (the player went deeper, to the stock list, whose `in` comes from 1.5), `back` = it grows back from
        // 0.75 (while the stock list's `out` grows away), `out2` = it GROWS away (back to the top screen).
        if e.layer != self.equip_layer {
            match (self.equip_layer, e.layer) {
                (EquipLayer::Slots, EquipLayer::Stock) => {
                    self.movie.goto_and_play(EQ_DETAIL, DETAIL_TO_STOCK);
                    self.movie.goto_and_play(EQ_REPLACE, "in");
                    self.key_text(&format!("{EQ_REPLACE}.stageCategory.category.categoryName.txName"), "kh3_camp.equip.title_0010");
                    self.movie.goto_and_stop(&format!("{EQ_REPLACE}.stageCategory.category.categoryIcon"), "camp2");
                    self.movie.goto_and_stop(&format!("{EQ_REPLACE}.stageCategory.stageObjL.bgCategoryIcon"), "camp2");
                }
                (EquipLayer::Stock, EquipLayer::Slots) => {
                    self.movie.goto_and_play(EQ_REPLACE, "out");
                    self.movie.goto_and_play(EQ_DETAIL, "back");
                }
                (EquipLayer::Slots, EquipLayer::Abilities) => {
                    self.movie.goto_and_play(&format!("{detail}.equipListVisibleAnm"), "off");
                    self.movie.goto_and_play("equip.abilityListVisibleAnm", "loop");
                    self.movie.goto_and_play("equip.selectEquipAnm", "loop");
                }
                (EquipLayer::Abilities, _) => {
                    self.movie.goto_and_play("equip.abilityListVisibleAnm", "off");
                    self.movie.goto_and_play("equip.selectEquipAnm", "off");
                    self.movie.goto_and_play(&format!("{detail}.equipListVisibleAnm"), "loop");
                }
                _ => {}
            }
            self.equip_layer = e.layer;
        }
        let guide = self.string(e.guide_key());
        self.set_guide(&guide);
        let help = self.string(e.help_key());
        self.help_line(EQ_DETAIL, &help);
        self.help_line(EQ_REPLACE, &help);
        let none = self.string("kh3_item.weapon.kb_none");
        let groups = [self.string("kh3_camp.equip.group_0001"), self.string("kh3_camp.equip.group_0002"), self.string("kh3_camp.equip.group_0003"), self.string("kh3_camp.equip.group_0004")];
        // The status pane: the highlighted piece's help text (see the module comment).
        // (IMPL_ER37) with the piece's picture; the stock list fills its own pane (itemStatusA2, below)
        let highlighted = e.highlighted().cloned();
        let piece_help = highlighted.as_ref().map(|p| self.piece_help(&p.id, &p.name, &p.help)).unwrap_or_default();
        let (pane, stock_pane) = (format!("{detail}.itemStatusA1"), format!("{replace}.itemStatusA2"));
        let target = if e.layer == EquipLayer::Stock { &stock_pane } else { &pane };
        self.item_pane(target, highlighted.as_ref().map(|p| p.id.as_str()), &piece_help);
        for p in [&pane, &stock_pane] {
            self.movie.goto_and_play(&format!("{p}.parameters"), "off");
        }
        let e = &mut menu.equipment;
        let fonts = self.fonts.clone();
        // The equipped list: one row per open slot, the first of a kind carries the group title.
        let slots = e.data.slots.clone();
        e.list.has_focus = e.layer == EquipLayer::Slots;
        e.list.refresh(&mut self.movie, |movie, path, index| {
            let slot = &slots[index];
            let first = index == 0 || slots[index - 1].kind != slot.kind;
            let label = match (slot.kind, first) {
                (KIND_WEAPON, true) => "lineTitleWeapon",
                (KIND_WEAPON, false) => "lineWeapon",
                (KIND_ARMOUR, true) => "lineTitleArmor",
                (KIND_ARMOUR, false) => "lineArmor",
                (KIND_ACCESSORY, true) => "lineTitleAccessory",
                (KIND_ACCESSORY, false) => "lineAccessory",
                (_, true) => "lineTitleItem",
                (_, false) => "lineItem",
            };
            movie.goto_and_stop(&format!("{path}.equipListTitleItem"), label);
            if first {
                movie.set_text(&format!("{path}.equipListTitleItem.equipCategoryTitleColor.equipCategoryTitleAnm.equipCategoryTitle"), &groups[slot.kind.min(3) as usize], &fonts);
            }
            movie.goto_and_stop(&format!("{path}.abilityCategoryTitleColor"), "off");
            movie.set_text(&format!("{path}.textIcon.textField"), if slot.piece.is_some() { kind_icon(slot.kind) } else { "" }, &fonts);
            let auto = slot.kind == KIND_ITEM && slot.auto_reload && slot.piece.is_some();
            movie.goto_and_stop(&format!("{path}.autoReloadMark"), if auto { "loop" } else { "off" });
            // LOCAL: the `auto` colour (red) is taken as "used, waiting for a refill".
            movie.goto_and_stop(&format!("{path}.equipItemNameColorAnm"), if slot.used { "auto" } else { "normal" });
            movie.set_text(&format!("{path}.equipItemNameColorAnm.equipItemNameColor.equipItemName"), slot.piece.as_ref().map_or(none.as_str(), |p| p.name.as_str()), &fonts);
            RowState::default()
        });
        match e.layer {
            EquipLayer::Stock => {
                self.text(&format!("{replace}.equipSelectCharaName.textField"), &stats.name);
                let visible = format!("{replace}.replaceListVisibleAnm");
                if self.movie.current_label(&visible) != Some("loop") {
                    self.movie.goto_and_play(&visible, "loop");
                }
                self.key_text(&format!("{visible}.equipListTitle.listTitleItem"), "kh3_camp.equip.list_0002");
                self.key_text(&format!("{visible}.equipListTitle.listTitleCnt"), "kh3_camp.equip.list_0001");
                let empty = if e.stock_rows.is_empty() { self.string("kh3_camp.equip.list_0004") } else { String::new() };
                self.text(&format!("{replace}.textField"), &empty);
                let rows = e.stock_rows.clone();
                e.stock.refresh(&mut self.movie, |movie, path, index| {
                    let row = &rows[index];
                    movie.set_text(&format!("{path}.textIcon.textField"), kind_icon(row.piece.kind), &fonts);
                    movie.goto_and_stop(&format!("{path}.equipMark"), if row.count.is_none() { "loop" } else { "off" });
                    movie.set_text(&format!("{path}.listItemName"), &row.piece.name, &fonts);
                    movie.set_text(&format!("{path}.itemCnt"), &row.count.map_or(String::new(), |c| c.to_string()), &fonts);
                    movie.set_visible(&format!("{path}.coTextReplace"), row.count.is_some());
                    RowState::default()
                });
                // The preview pane `ItemStatusB` (`0x1415AA460`): the value with the piece, `up` /
                // `down` against the current one.
                let pane = format!("{replace}.itemStatusB");
                if self.movie.current_label(&pane) != Some("loop") {
                    self.movie.goto_and_play(&pane, "loop");
                }
                let preview = e.preview(&stats);
                let names = [("11", "HP".to_string(), ""), ("12", "MP".to_string(), ""), ("13", "AP".to_string(), ""), ("21", self.string("kh3_system.general.window_0110"), "<icon(Power)>"), ("22", self.string("kh3_system.general.window_0120"), "<icon(MagicPower)>"), ("23", self.string("kh3_system.general.window_0130"), "<icon(Deffence)>"), ("31", self.string("kh3_system.general.window_0140_s"), "<icon(Fire)>"), ("32", self.string("kh3_system.general.window_0150_s"), "<icon(Blizzard)>"), ("33", self.string("kh3_system.general.window_0160_s"), "<icon(Thunder)>"), ("34", self.string("kh3_system.general.window_0170_s"), "<icon(Water)>"), ("35", self.string("kh3_system.general.window_0180_s"), "<icon(Aero)>"), ("36", self.string("kh3_system.general.window_0190_s"), "<icon(Dark)>")];
                for (k, (id, name, icon)) in names.into_iter().enumerate() {
                    if k < 3 {
                        self.text(&format!("{pane}.name{id}.textField"), &name);
                    } else {
                        self.text(&format!("{pane}.name{id}.txName"), &name);
                        self.text(&format!("{pane}.name{id}.icon.textField"), icon);
                    }
                    let (now, new) = (preview.now[k], preview.new[k]);
                    let value = if k >= 6 { format!("{new}%") } else { new.to_string() };
                    self.text(&format!("{pane}.num{id}.txText.textField"), &value);
                    self.movie.goto_and_stop(&format!("{pane}.num{id}"), if new > now { "up" } else if new < now { "down" } else { "normal" });
                }
            }
            EquipLayer::Abilities => {
                // The piece whose abilities are listed, above the list.
                let slot_piece = e.slot().and_then(|s| s.piece.clone());
                let kind = e.slot().map_or(KIND_ITEM, |s| s.kind);
                self.text("equip.selectEquipAnm.selectEquip.textIcon.textField", if slot_piece.is_some() { kind_icon(kind) } else { "" });
                self.text("equip.selectEquipAnm.selectEquip.equipItemName", slot_piece.as_ref().map_or(none.as_str(), |p| p.name.as_str()));
                let abilities = e.highlighted().map(|p| p.abilities.clone()).unwrap_or_default();
                let title = self.string("kh3_camp.customize.group_0010");
                e.abilities.refresh(&mut self.movie, |movie, path, index| {
                    movie.goto_and_stop(&format!("{path}.equipListTitleItem"), if index == 0 { "lineTitleAbility" } else { "lineAbility" });
                    if index == 0 {
                        movie.set_text(&format!("{path}.equipListTitleItem.equipCategoryTitleColor.equipCategoryTitleAnm.equipCategoryTitle"), &title, &fonts);
                    }
                    movie.goto_and_stop(&format!("{path}.abilityCategoryTitleColor"), "off");
                    movie.set_text(&format!("{path}.textIcon.textField"), "<icon(WeponAbility)>", &fonts);
                    movie.goto_and_stop(&format!("{path}.autoReloadMark"), "off");
                    movie.set_text(&format!("{path}.equipItemNameColorAnm.equipItemNameColor.equipItemName"), &abilities[index], &fonts);
                    RowState::default()
                });
            }
            EquipLayer::Slots => {}
        }
    }

    // ------------------------------------------------------------------------- Items (10.5)

    fn sync_items(&mut self, menu: &mut CampMenu) {
        let stats = menu.stats.clone();
        // (party) the target buttons show every party slot (`m_TargetButtons`)
        let party: Vec<crate::camp::StatsIn> = if menu.party.is_empty() { vec![stats.clone()] } else { menu.party.clone() };
        let it = &mut menu.items;
        let item_root = format!("{BACKYARD}.backyardItem");
        if !self.movie.exists(&format!("{item_root}.backyardList.container")) {
            return;
        }
        let stamp = stamp_of(&(&it.entries, it.tab, &it.list, it.target, it.target_cursor, &party)) ^ self.movie.exists(&it.list.item_path(0)) as u64;
        if stamp == self.stamp {
            return;
        }
        let fresh = self.stamp == 0;
        self.stamp = stamp;
        if fresh {
            self.key_text(&format!("{item_root}.backyardListTitle.textField"), "kh3_camp.item.list_0002");
            self.key_text(&format!("{item_root}.backyardListTitle.backyardListTitleCnt"), "kh3_camp.item.list_0001");
            self.item_tabs.select = it.tab;
            self.item_tabs.build(&mut self.movie, ITEM_TABS.len(), &crate::camp_equip::ITEM_TAB_ICONS);
            self.item_tabs.set_switch_icons(&mut self.movie, "<icon(L2)>", "<icon(R2)>", &self.fonts);
            let guide = self.string("kh3_camp.item.button_0001");
            self.set_guide(&guide);
        }
        let tab_name = self.string(ITEM_TABS[it.tab.min(ITEM_TABS.len() - 1)].0);
        self.item_tabs.select(&mut self.movie, it.tab, &tab_name, it.tab_flash.take(), &self.fonts);
        if it.target != self.items_target {
            self.items_target = it.target;
            self.movie.goto_and_play(BACKYARD, if it.target { "charaSelect" } else { "backyard" });
            self.movie.set_visible(&format!("{BACKYARD}.charaList"), it.target);
            self.movie.set_visible(&item_root, !it.target);
        }
        let help = self.string(if it.target { "kh3_camp.item.detail_0004" } else { "kh3_camp.item.detail_0003" });
        self.help_line("backyard", &help);
        let rows: Vec<crate::camp_equip::InvEntry> = it.rows().into_iter().cloned().collect();
        let focused = it.focused().cloned();
        let empty = if rows.is_empty() { self.string("kh3_system.general.system_0011") } else { String::new() };
        self.text(&format!("{item_root}.textField"), &empty);
        let pane = format!("{BACKYARD}.itemStatusA1");
        // (IMPL_ER37) the picture and the game's help text by the item key
        let piece_help = focused.as_ref().map(|f| self.piece_help(&f.id, &f.name, &f.help)).unwrap_or_default();
        self.item_pane(&pane, focused.as_ref().map(|f| f.id.as_str()), &piece_help);
        let fonts = self.fonts.clone();
        if it.target {
            // The item above the party buttons, and Sora's button (`m_TargetButtons[0]`).
            let chara = format!("{BACKYARD}.charaList");
            self.key_text(&format!("{chara}.backyardListTitle.textField"), "kh3_camp.item.list_0002");
            self.key_text(&format!("{chara}.backyardListTitle.backyardListTitleCnt"), "kh3_camp.item.list_0001");
            if let Some(entry) = &focused {
                let select = format!("{chara}.selectBackyardItem");
                self.text(&format!("{select}.textIcon"), category_icon(entry.category));
                self.text(&format!("{select}.selectBackyardTxtColor.selectBackyardTxt.backyardItemName"), &entry.name);
                self.text(&format!("{select}.selectBackyardTxtColor.selectBackyardTxt.itemCnt"), &entry.count.to_string());
                self.movie.goto_and_stop(&format!("{select}.selectBackyardTxtColor"), if entry.count > 0 { "normal" } else { "lack" });
            }
            let cap = |v: i32| v.min(999).to_string();
            for n in 1..=5usize {
                let button = format!("{chara}.btnSelectChara{n}");
                let Some(stats) = party.get(n - 1) else {
                    self.movie.set_visible(&button, false);
                    continue;
                };
                self.movie.set_visible(&button, true);
                let want = if it.target_cursor == n - 1 { "select" } else { "normal" };
                if self.movie.current_label(&button) != Some(want) {
                    self.movie.goto_and_play(&button, want);
                }
                self.text(&format!("{button}.charaListNameAnm.charaListName"), &stats.name);
                for (clip, field, value) in [("parameterHp", "hpNow.textField", cap(stats.hp)), ("parameterHp", "hpMax", cap(stats.hp_max)), ("parameterHp", "articleHp", "HP".into()), ("parameterMp", "mpNow.textField", cap(stats.mp)), ("parameterMp", "mpMax", cap(stats.mp_max)), ("parameterMp", "articleMp", "MP".into()), ("parameterAtk", "atkNow.textField", cap(stats.strength)), ("parameterAtk", "textIconAtk", "<icon(Power)>".into()), ("parameterAtk", "atkArrow", String::new()), ("parameterDef", "defNow.textField", cap(stats.defense)), ("parameterDef", "textIconDef", "<icon(Deffence)>".into()), ("parameterDef", "defArrow", String::new()), ("parameterMgc", "mgcNow.textField", cap(stats.magic)), ("parameterMgc", "textIconMgc", "<icon(MagicPower)>".into()), ("parameterMgc", "mgcArrow", String::new()), ("parameterAp", "apNow.textField", cap(stats.ap_max)), ("parameterAp", "articleAp", "AP".into()), ("parameterAp", "apArrow", String::new())] {
                    self.text(&format!("{button}.{clip}.{field}"), &value);
                }
                let focus = format!("{button}.parameterFcsVisible");
                self.movie.goto_and_play(&focus, if stats.focus_unlocked { "loop" } else { "off" });
                for (field, value) in [("fcsNow.textField", cap(stats.focus)), ("fcsMax", cap(stats.focus_max)), ("articleFcs", crate::camp::third_gauge_label(&stats.name).to_string() /* (GOOFY-GG) */)] {
                    self.text(&format!("{focus}.parameterFcs.{field}"), &value);
                }
            }
            return;
        }
        it.list.refresh(&mut self.movie, |movie, path, index| {
            let row = &rows[index];
            movie.set_text(&format!("{path}.textIcon"), category_icon(row.category), &fonts);
            movie.set_text(&format!("{path}.backyardItemName"), &row.name, &fonts);
            movie.set_text(&format!("{path}.itemCnt"), &row.count.to_string(), &fonts);
            RowState::default()
        });
    }

    // --------------------------------------------------------------------- Customize (10.6)

    fn button_state(&mut self, path: &str, selected: bool, enabled: bool) {
        let labels: &[&str] = match (enabled, selected) {
            (true, true) => &["select"],
            (true, false) => &["normal"],
            (false, true) => &["disableSelect", "disable"],
            (false, false) => &["disable"],
        };
        if self.movie.current_label(path).is_some_and(|l| labels.contains(&l)) {
            return;
        }
        for label in labels {
            if self.movie.goto_and_play(path, label) {
                break;
            }
        }
    }

    fn sync_customize(&mut self, menu: &mut CampMenu) {
        let c = &mut menu.customize;
        let menu_root = format!("{CUSTOM}.customizeMenu");
        if !self.movie.exists(&format!("{menu_root}.btnCmdMenu1")) {
            return;
        }
        let stamp = stamp_of(&*c);
        if stamp == self.stamp {
            return;
        }
        let fresh = self.stamp == 0;
        self.stamp = stamp;
        if fresh {
            for (n, key) in [(1, "kh3_camp.customize.group_0001"), (2, "kh3_camp.customize.group_0002"), (3, "kh3_camp.customize.group_0003")] {
                self.key_text(&format!("{menu_root}.sectionTitle{n}.sectionTitleText{n}.textField"), key);
            }
            // (party) the Friends group: its title is greyed only when no friend is in the party
            let friends = c.data.friends.len().min(4);
            self.movie.goto_and_stop(&format!("{menu_root}.sectionTitle3"), if friends == 0 { "disable" } else { "loop" });
            for (button, key) in [("btnCmdMenu1", "kh3_camp.customize.custom_0001"), ("btnCmdMenu2", "kh3_camp.customize.custom_0002"), ("btnShortcut1", "kh3_camp.customize.custom_0003"), ("btnShortcut2", "kh3_camp.customize.custom_0004"), ("btnShortcut3", "kh3_camp.customize.custom_0005")] {
                // (party X2) a friend leader's own label for his Magic entry (Goofy: "Abilities")
                let key = if button == "btnCmdMenu1" { c.data.magic_label.clone().unwrap_or_else(|| key.to_string()) } else { key.to_string() };
                self.key_text(&format!("{menu_root}.{button}.cmdMenuListName.textField"), &key);
            }
            // Entries without content are hidden: Links (no link learned), the friends not in the party.
            // (XSI) KH3's Links button carries the Shotlock choice (XS 2.8) when there is one.
            let shotlock = c.data.shotlock.as_ref().filter(|s| !s.choices.is_empty()).map(|s| s.text());
            self.movie.set_visible(&format!("{menu_root}.btnCmdMenu2"), shotlock.is_some());
            if let Some(text) = shotlock {
                self.text(&format!("{menu_root}.btnCmdMenu2.cmdMenuListName.textField"), &text);
            }
            for n in 1..=4 {
                let shown = n <= friends;
                self.movie.set_visible(&format!("{menu_root}.btnBattleStyle{n}"), shown);
                if shown {
                    // `custom_0006 ..` = "{0}": the friend's name
                    let name = c.data.friends[n - 1].name.clone();
                    self.text(&format!("{menu_root}.btnBattleStyle{n}.cmdMenuListName.textField"), &name);
                }
            }
        }
        let mut buttons: Vec<String> = ["btnCmdMenu1", "btnShortcut1", "btnShortcut2", "btnShortcut3"].iter().map(|b| b.to_string()).collect();
        // (XSI) the Shotlock entry: KH3's Links button, under Magic; its text follows the choice
        if let Some(s) = c.data.shotlock.as_ref().filter(|s| !s.choices.is_empty()) {
            buttons.insert(1, "btnCmdMenu2".to_string());
            let text = s.text();
            self.text(&format!("{menu_root}.btnCmdMenu2.cmdMenuListName.textField"), &text);
        }
        buttons.extend((1..=c.data.friends.len().min(4)).map(|n| format!("btnBattleStyle{n}")));
        for (index, button) in buttons.iter().enumerate() {
            self.button_state(&format!("{menu_root}.{button}"), index == c.top_cursor, true);
        }
        // Layers on / off.
        let layer = c.layer;
        if layer != self.custom_layer || fresh {
            self.custom_layer = layer;
            let on = |want: bool| if want { "loop" } else { "off" };
            for (clip, shown) in [("cmdMenu2ndLayer", matches!(layer, CustomLayer::Magic | CustomLayer::Tier)), ("cmdMenu3rdLayer", layer == CustomLayer::Tier), ("shortcut2ndLayer", matches!(layer, CustomLayer::Set | CustomLayer::List)), ("shortcut3rdLayer", layer == CustomLayer::List), ("battleStyle2ndLayer", matches!(layer, CustomLayer::Friend | CustomLayer::FriendList)), ("battleStyle3rdLayer", layer == CustomLayer::FriendList)] {
                let path = format!("{CUSTOM}.{clip}");
                if self.movie.current_label(&path) != Some(on(shown)) {
                    self.movie.goto_and_play(&path, on(shown));
                }
            }
            if layer == CustomLayer::List {
                self.custom_tabs.select = c.tab;
                let icons: Vec<i32> = c.tabs().iter().map(|t| crate::camp_custom::TAB_ICONS[*t]).collect();
                self.custom_tabs.build(&mut self.movie, c.tabs().len(), &icons);
                self.custom_tabs.set_switch_icons(&mut self.movie, "<icon(L2)>", "<icon(R2)>", &self.fonts);
            }
        }
        let (key, own) = c.help();
        let help = key.map_or(own, |k| self.string(k));
        self.help_line(CUSTOM, &help);
        let guide = self.string(c.guide_key());
        self.set_guide(&guide);
        let group_icon = |group: u8| if group == GROUP_ITEM { "<icon(BattleItem)>" } else { "<icon(Magic)>" };
        // (party) A FRIEND'S SETTINGS (0x1415E7E80 fills them: the three style names by value, the three targets
        // ON / OFF) and the choice list of the focused style.
        if let (true, Some(f)) = (matches!(layer, CustomLayer::Friend | CustomLayer::FriendList), c.friend_in().cloned()) {
            use crate::camp_custom::{STYLE_CHOICES, STYLE_LABELS};
            let second = format!("{CUSTOM}.battleStyle2ndLayer.battleStyle2ndLayerAnm");
            self.movie.goto_and_stop(&format!("{second}.cmdMenuWindow"), "friend");
            for (n, key) in [(4, "kh3_camp.customize.bstyle_0201"), (5, "kh3_camp.customize.bstyle_0202"), (6, "kh3_camp.customize.bstyle_0203")] {
                self.key_text(&format!("{second}.sectionTitle{n}.sectionTitleText{n}.textField"), key);
            }
            for (row, button) in ["btnSetBattleStyle", "btnSetAbilityStyle", "btnSetRecoveryStyle"].into_iter().enumerate() {
                let path = format!("{second}.{button}");
                self.text(&format!("{path}.textIcon.textField"), "<icon(BattleStyle)>");
                let label = STYLE_LABELS[row].get(f.styles[row] as usize).copied().unwrap_or("");
                self.key_text(&format!("{path}.setStyleName.textField"), label);
                self.button_state(&path, layer == CustomLayer::Friend && c.friend_row == row, true);
            }
            for t in 0..3usize {
                let path = format!("{second}.btnSetRecoveryKyara{}", t + 1);
                self.key_text(&format!("{path}.targetName.textField"), ["kh3_camp.customize.bstyle_0004", "kh3_camp.customize.bstyle_0005", "kh3_camp.customize.bstyle_0006"][t]);
                let on = f.targets & (1 << t) != 0;
                self.key_text(&format!("{path}.targetState.textField"), if on { "kh3_camp.customize.bstyle_0007" } else { "kh3_camp.customize.bstyle_0008" });
                let focused = layer == CustomLayer::Friend && c.friend_row == 3 + t;
                self.button_state(&path, focused, true);
                for arrow in ["changeArrowLeftAnm", "changeArrowRightAnm"] {
                    let a = format!("{path}.{arrow}");
                    let want = if focused { "loop" } else { "off" };
                    if self.movie.current_label(&a) != Some(want) {
                        self.movie.goto_and_play(&a, want);
                    }
                }
            }
            if layer == CustomLayer::FriendList {
                let third = format!("{CUSTOM}.battleStyle3rdLayer.battleStyle3rdLayerAnm");
                let row = c.friend_row.min(2);
                self.key_text(&format!("{third}.sectionTitle11.textField"), ["kh3_camp.customize.bstyle_0201", "kh3_camp.customize.bstyle_0202", "kh3_camp.customize.bstyle_0203"][row]);
                for n in 1..=4usize {
                    let path = format!("{third}.btnSelectBattleStyle{n}");
                    let shown = n <= STYLE_CHOICES[row];
                    self.movie.set_visible(&path, shown);
                    if shown {
                        self.text(&format!("{path}.textIcon.textField"), "<icon(BattleStyle)>");
                        self.key_text(&format!("{path}.setBattleStyleName.textField"), STYLE_LABELS[row][n - 1]);
                        self.button_state(&path, c.choice_cursor == n - 1, true);
                    }
                }
            }
        }
        if matches!(layer, CustomLayer::Magic | CustomLayer::Tier) {
            let common = format!("{CUSTOM}.cmdMenu2ndLayer.cmdSetCommon");
            let count = c.magic_count();
            self.movie.goto_and_stop(&format!("{common}.cmdMenuWindow"), ["off", "one", "two", "three", "four", "five", "six"][count.min(6)]);
            let title = c.data.magic_label.clone().unwrap_or_else(|| "kh3_camp.customize.group_0004".to_string()); // (party X2)
            self.key_text(&format!("{common}.sectionTitle7.sectionTitleText7.textField"), &title);
            for n in 1..=6usize {
                let button = format!("{common}.btnSetCmdCommon{n}");
                self.movie.set_visible(&button, n <= count);
                if n > count {
                    continue;
                }
                let kind = c.data.magic_slots[n - 1];
                self.text(&format!("{button}.cmdMove.cmdReplace.textIcon.textField"), "<icon(Magic)>");
                let name = c.data.name(kind);
                self.text(&format!("{button}.cmdMove.cmdReplace.setCmdName.setItemName"), &name);
                self.text(&format!("{button}.cmdSetNumber.cmdSetNumber"), &n.to_string());
                self.movie.goto_and_stop(&format!("{button}.cmdMove"), if c.picked == Some(n - 1) { "select" } else { "normal" });
                self.button_state(&button, c.magic_cursor == n - 1, true);
            }
        }
        if layer == CustomLayer::Tier {
            let tiers = c.tiers();
            for n in 1..=3usize {
                let button = format!("{CUSTOM}.cmdMenu3rdLayer.cmdChangeList.btnSelectCmd{n}");
                let Some((kind, learned)) = tiers.get(n - 1).copied() else {
                    self.movie.set_visible(&button, false);
                    continue;
                };
                self.movie.set_visible(&button, true);
                self.text(&format!("{button}.textIcon.textField"), "<icon(Magic)>");
                let name = if learned { c.data.name(kind) } else { "? ? ?".to_string() };
                self.text(&format!("{button}.selectCmdName.textField"), &name);
                self.button_state(&button, c.tier_cursor == n - 1, learned);
            }
        }
        if matches!(layer, CustomLayer::Set | CustomLayer::List) {
            let set = format!("{CUSTOM}.shortcut2ndLayer.setShortcutCmd");
            self.movie.goto_and_stop(&format!("{set}.cmdMenuWindow"), "four");
            self.key_text(&format!("{set}.sectionTitle8.sectionTitleText8.textField"), ["kh3_camp.customize.group_0006", "kh3_camp.customize.group_0007", "kh3_camp.customize.group_0008"][c.page.min(2)]);
            // The face buttons right, top, left, bottom, as in the HUD panel (5.5).
            for (n, pad) in ["<icon(Circle)>", "<icon(Triangle)>", "<icon(Square)>", "<icon(Cross)>"].into_iter().enumerate() {
                let button = format!("{set}.btnSetShortcutCmd{}", n + 1);
                let kind = c.data.shortcuts[c.page.min(2)][n];
                let entry = c.data.entry(kind).cloned();
                self.text(&format!("{button}.padBtn.txPadBtn"), pad);
                self.text(&format!("{button}.textIcon.textField"), entry.as_ref().map_or("", |e| group_icon(e.group)));
                let name = c.data.name(kind);
                self.text(&format!("{button}.shortcutSetName.textField"), &name);
                self.movie.goto_and_stop(&format!("{button}.autoReloadMark"), if entry.is_some_and(|e| e.auto_reload) { "loop" } else { "off" });
                self.button_state(&button, c.set_cursor == n, true);
            }
        }
        if layer == CustomLayer::List {
            let list_root = format!("{CUSTOM}.shortcut3rdLayer.shortcutChangeListAnm");
            self.key_text(&format!("{list_root}.sectionTitle9.textField"), "kh3_camp.customize.group_0012");
            let tabs = c.tabs();
            let tab = tabs.get(c.tab).copied().unwrap_or(0);
            // (party X2) the Magic tab of a friend leader's own list (Goofy: "Abilities")
            let tab_key = match (&c.data.magic_label, TAB_KEYS[tab].1) {
                (Some(label), Some(crate::camp_custom::GROUP_MAGIC)) => label.clone(),
                _ => TAB_KEYS[tab].0.to_string(),
            };
            let tab_name = self.string(&tab_key);
            self.custom_tabs.select(&mut self.movie, c.tab, &tab_name, c.tab_flash.take(), &self.fonts);
            let rows: Vec<crate::camp_custom::CommandEntry> = c.list_rows().into_iter().cloned().collect();
            let fonts = self.fonts.clone();
            c.list.refresh(&mut self.movie, |movie, path, index| {
                let row = &rows[index];
                movie.set_text(&format!("{path}.textIcon.textField"), group_icon(row.group), &fonts);
                movie.set_text(&format!("{path}.selectCmdName.textField"), &row.name, &fonts);
                movie.goto_and_stop(&format!("{path}.autoReloadMark"), if row.auto_reload { "loop" } else { "off" });
                RowState::default()
            });
            // The stamp must not change because the flash was taken.
            self.stamp = stamp_of(&*c);
        }
    }

    // --------------------------------------------------------------------------- Game Help

    fn sync_help(&mut self, menu: &mut CampMenu) {
        let h = &mut menu.help;
        if !self.movie.exists(&format!("{}.container", h.cats.path)) {
            return;
        }
        let stamp = stamp_of(&(&h.cats, &h.topics, h.layer, h.categories.len())) ^ self.movie.exists(&h.cats.item_path(0)) as u64;
        if stamp == self.stamp {
            return;
        }
        self.stamp = stamp;
        let help = self.string("kh3_camp.gamehelp.detail_0010");
        self.help_line("gameHelp", &help);
        let fonts = self.fonts.clone();
        let names: Vec<String> = h.categories.iter().map(|c| c.name.clone()).collect();
        h.cats.has_focus = h.layer == HelpLayer::Categories;
        h.cats.refresh(&mut self.movie, |movie, path, index| {
            movie.set_text(&format!("{path}.gameHelpCategoryName.textField"), &names[index], &fonts);
            RowState::default()
        });
        let titles: Vec<String> = h.category().map_or(Vec::new(), |c| c.topics.iter().map(|t| t.title.clone()).collect());
        h.topics.has_focus = h.layer != HelpLayer::Categories;
        h.topics.refresh(&mut self.movie, |movie, path, index| {
            movie.set_text(&format!("{path}.gameHelpCategoryName.textField"), &titles[index], &fonts);
            RowState::default()
        });
        // A topic's page: the `GameHelp` movie.
        match (h.layer, h.topic().cloned()) {
            (HelpLayer::Page(page), Some(topic)) => {
                if !self.page_open || self.page_closing {
                    self.page.goto_and_play("", "in");
                    self.page_open = true;
                    self.page_closing = false;
                }
                let pages = topic.pages.len();
                self.page.set_text("helpTitle.helpTitleAnm.helpTitle", &topic.title, &self.fonts);
                self.page.set_text("helpTitle.pageNumber.textField", &if pages > 1 { format!("{} / {}", page + 1, pages) } else { String::new() }, &self.fonts);
                self.page.set_text("helpImageAnm.helpImage.helpInfo.textField", &topic.pages[page.min(pages.saturating_sub(1))].replace("<br>", "\n"), &self.fonts);
                let guide = self.string(if pages <= 1 { "kh3_gamehelp.general.button_001" } else if page + 1 < pages { "kh3_gamehelp.general.button_002" } else { "kh3_gamehelp.general.button_003" });
                self.page.set_text("helpButtonAnm.helpButton.textField", &guide, &self.fonts);
            }
            _ => self.close_page(),
        }
    }

    // ---------------------------------------------------------------- (er-host, IMPL_ER59) Outfits

    /// Sora's outfits in the Game Help screen's category list (the topic list stays empty); the help line names the
    /// one worn.
    fn sync_outfits(&mut self, menu: &mut CampMenu) {
        let o = &mut menu.outfits;
        if !self.movie.exists(&format!("{}.container", o.list.path)) {
            return;
        }
        let stamp = stamp_of(&(&o.list, o.worn, o.outfits.len())) ^ self.movie.exists(&o.list.item_path(0)) as u64 ^ 0x0F17;
        if stamp == self.stamp {
            return;
        }
        self.stamp = stamp;
        let worn = o.outfits.get(o.worn).map(|(_, n)| n.clone()).unwrap_or_default();
        self.help_line("gameHelp", &format!("{} Wearing: {worn}", crate::camp::OUTFITS_DETAIL));
        let fonts = self.fonts.clone();
        let names: Vec<String> = o.outfits.iter().map(|(_, n)| n.clone()).collect();
        o.list.has_focus = true;
        o.list.refresh(&mut self.movie, |movie, path, index| {
            movie.set_text(&format!("{path}.gameHelpCategoryName.textField"), &names[index], &fonts);
            RowState::default()
        });
        // the Game Help screen's topic list: nothing in it
        let mut topics = crate::list::ListView::new("gameHelp.gameHelpAnm.gameHelpItemList", "CoGameHelpCategoryListItem");
        topics.set_count(0);
        topics.has_focus = false;
        topics.refresh(&mut self.movie, |_, _, _| RowState::default());
    }

    fn close_page(&mut self) {
        if self.page_open && !self.page_closing {
            self.page.goto_and_play("", "out");
            self.page_closing = true;
        }
    }

    // ----------------------------------------------------------------------------- Dialogs

    /// `0x1415A1C70(mgr, kind, text, ...)`: the dialog window with its text and buttons.
    fn sync_dialog(&mut self, dialog: Option<&Dialog>) {
        let Some(dialog) = dialog else {
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
        let stamp = stamp_of(dialog);
        if self.dialog_open && !self.dialog_closing && stamp == self.dialog_stamp {
            self.place_dialog_parts();
            return;
        }
        if !self.dialog_open || self.dialog_closing {
            self.dialog.goto_and_play("dialog", "in");
            self.dialog.add_instance("", "DialogButton", "dialogButton2");
        }
        self.dialog_open = true;
        self.dialog_closing = false;
        self.dialog_stamp = stamp;
        let text = self.string(dialog.text_key).replace("{0}", &dialog.argument);
        self.dialog.set_visible("dialogText", true);
        self.dialog.set_text("dialogText.textField", &text, &self.fonts);
        let buttons: Vec<String> = match dialog.kind {
            DialogKind::Ok => vec![self.string("kh3_system.general.choice_0003")],
            DialogKind::YesNo => vec![self.string("kh3_system.general.choice_0001"), self.string("kh3_system.general.choice_0002")],
        };
        for (n, name) in ["dialogButton", "dialogButton2"].into_iter().enumerate() {
            self.dialog.set_visible(name, n < buttons.len());
            if let Some(label) = buttons.get(n) {
                self.dialog.set_text(&format!("{name}.dialogBtnText.textField"), label, &self.fonts);
                let selected = dialog.kind == DialogKind::Ok || dialog.cursor == n;
                self.dialog.goto_and_play(name, if selected { "select" } else { "normal" });
            }
        }
        self.place_dialog_parts();
    }

    /// The text and the buttons sit at the window's position holders (`poDialogText`,
    /// `poDialogButton`; buttons 35 apart: `BUTTON_HEIGHT`).
    fn place_dialog_parts(&mut self) {
        let holder = "dialog.dialogAnm.dialogPoAll.dialogPoAll2";
        let stage = |movie: &Movie, path: &str| -> Option<(f32, f32)> {
            let mut m = Matrix::IDENTITY;
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

    /// Slot 75 `0x1415BD5F0`: root `out`.
    pub fn close(&mut self) {
        if self.open && !self.closing {
            let was = self.shown_screen;
            self.leave_sub(was);
            self.sync_dialog(None);
            self.movie.goto_and_play("", "out");
            self.closing = true;
            // (party PF) the icons fade with it
            if let Some(scene) = self.formation_scene.as_mut() {
                scene.close();
            }
        }
    }

    /// One frame. True when the close animation ended (`ON_END_TOP_OUT`): the close delegate unpauses.
    pub fn tick(&mut self) -> bool {
        self.movie.tick();
        // (party PF) the icon screen's frame
        if let Some(scene) = self.formation_scene.as_mut() {
            scene.step();
            scene.apply(&mut self.movie);
            // a refused edit: the help line that says why flashes red, then fades back to white
            let flash = scene.refusal().map_or(0.0, |(_, k)| k);
            if let Some(node) = self.movie.node_mut(FORMATION_HELP) {
                node.cx = crate::geom::Cx { mul: [1.0, 1.0 - 0.55 * flash, 1.0 - 0.6 * flash, 1.0], add: [0.0; 4] };
            }
        }
        if self.page_open {
            self.page.tick();
            if self.page_closing && self.page.current_label("") == Some("off") {
                self.page_open = false;
                self.page_closing = false;
            }
            // The keyboard picture of the controls topics is not driven.
            if self.page.current_label("helpKeyboardAnm").is_some_and(|l| l != "off") {
                self.page.goto_and_play("helpKeyboardAnm", "off");
            }
        }
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
        self.page.take_events();
        self.dialog.take_events();
        if ended {
            self.open = false;
            self.closing = false;
            self.page_open = false;
            self.dialog_open = false;
        }
        ended
    }
}
