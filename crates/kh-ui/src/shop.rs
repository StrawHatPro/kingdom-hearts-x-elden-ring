//! (SHOP) THE KH SHOP SCREEN that stands in for a host's merchants (the Elden Ring mod: every merchant's buy / sell
//! window; the user, 2026-10-23: "shops need to be replaced with our own Kingdom Heart's styled shops", "by shop I mean
//! shop menus"). State note: build/sora_tmp/party/IMPL_SHOP_STATE.md. The rules (stock, prices, the sell rule) are the
//! host's (kh-host shop.rs); this file is the screen: [`ShopMenu`] the logic, [`ShopView`] the display.
//!
//! WHAT IS KH3'S [A]: the camp menu's movie (`15_campmenu/CampMenu`) on its Items screen ("backyard": the list widget of
//! `ItemListLibrary` with its rows, scroll bar and tab bar `listTabCamp`, the item status pane `ItemStatusA1` with the
//! item's picture and help text, the frame with the munny counter, the title strip with the category icon, the help
//! line, the button guide plate), the `Dialog` movie for the quantity and the confirmation, KH3'S OWN SHOP TEXTS
//! (`kh3_menu.shop.*`: "Moogle Shop" / "SHOP", the BUY / SELL tabs, "Name", "Price", "<KeyLeftRight> Change Quantity",
//! "Complete this purchase?" / "Complete this sale?", "You don't have enough munny.", "You can't carry any more of
//! that.", "Traded {0} for {1}."), KH3's shop icons of the category clip (`shop1` buy, `shop2` sell), the menu sounds
//! (static SE 0 cursor, 1 decide, 2 open, 3 close, 4 refused, 33 the item-get cue on a trade).
//! WHAT IS COMPOSED HERE [L] (KH3's own `41_shop/Shop` movie is not in the UI pack and may not be added by this stream):
//! the Items screen standing for the Moogle shop's list, the price in the row's count column, "Owned" in the pane, the
//! quantity window's text, the merchant's name in the frame's area line.
//!
//! THE PAD (Xbox layout of the menus; the keyboard reaches the same keys through the host): D-pad / stick up and down the
//! list (LB / RB a page), left / right or LT / RT the BUY / SELL tab, A choose, B back ONE level (the confirmation ->
//! the quantity -> the list -> the shop closes). In the quantity window left / right -1 / +1, up / down +10 / -10.

use std::collections::HashMap;
use std::sync::Arc;

use crate::camp::CampKey;
use crate::list::{ListView, RowState, TabBar};
use crate::menu::{SE_MENU_CLOSE, SE_MENU_CURSOR, SE_MENU_DECIDE, SE_MENU_OPEN, SE_MENU_REFUSED};
use crate::{Fonts, Movie, MovieDef, Pack, PackError, UiSound};

/// The item-get cue (static SE 33, `hud_side::SE_ITEM`): a trade went through.
pub const SE_TRADE: u32 = 33;

/// One row of a tab: the item, its price on this tab (buy: what it costs; sell: what it brings), how many he holds.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ShopRow {
    pub key: String,
    pub name: String,
    pub price: u32,
    pub owned: u32,
    pub max: u32,
    /// `camp_equip::CAT_*` (the row's icon).
    pub category: u8,
    /// The host's own help text ("" = the game's by the item key).
    pub help: String,
}

/// What the host shows: the merchant, the munny, the two tabs.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ShopData {
    /// The merchant's name (the frame's area line; "" = KH3's "Moogle Shop").
    pub title: String,
    pub munny: i64,
    pub buy: Vec<ShopRow>,
    pub sell: Vec<ShopRow>,
    /// The most of one item a single trade may move.
    pub qty_max: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ShopTab {
    #[default]
    Buy,
    Sell,
}

/// What the screen asks of the host.
#[derive(Clone, Debug, PartialEq)]
pub enum ShopEvent {
    Buy { key: String, qty: u32, price: u32 },
    Sell { key: String, qty: u32, price: u32 },
    /// B on the list: the shop closes (its `out` follows).
    Closed,
}

/// The layer that has the pad.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    List,
    Quantity,
    Confirm,
}

/// Why a row cannot be traded now (KH3's words).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    Munny,
    Full,
    NoneHeld,
}

pub struct ShopMenu {
    pub data: ShopData,
    pub tab: ShopTab,
    pub list: ListView,
    pub layer: Layer,
    pub qty: u32,
    /// The confirmation's cursor: 0 Yes, 1 No.
    pub confirm: usize,
    /// The help line's message after a refusal or a trade (cleared by the next move).
    pub notice: Option<String>,
    /// The tab switch arrow to flash (`TabBar::select`).
    pub tab_flash: Option<bool>,
    events: Vec<ShopEvent>,
    /// KH3's texts the logic writes into the notice (set by the view; English defaults).
    pub texts: ShopTexts,
    /// KH3's Moogle shop way (shop_moogle.rs): the quantity is changed ON THE ROW (left / right, up / down +-10 is
    /// not KH3's there: up / down move the cursor), A goes straight to the confirmation, B there back to the list; the
    /// BUY / SELL switch is LT / RT only. False: the camp-menu screen's quantity window.
    pub inline_qty: bool,
}

/// The texts of the notices (`kh3_menu.shop.dialog_*`).
#[derive(Clone, Debug, PartialEq)]
pub struct ShopTexts {
    pub no_munny: String,
    pub full: String,
    pub none_held: String,
    /// "Traded {0} for {1}."
    pub traded: String,
}

impl Default for ShopTexts {
    fn default() -> ShopTexts {
        ShopTexts { no_munny: "".into(), full: "".into(), none_held: "".into(), traded: "".into() }
    }
}

/// The list widget of the Items screen.
pub const LIST: &str = "backyard.backyardAnm.backyardItem.backyardList";
pub const LIST_ITEM: &str = "CoBackyardListItem";

/// `n` with thousands separators (KH3's counters show plain digits; the dialog's totals read better so) [L].
pub fn grouped(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (k, c) in digits.chars().enumerate() {
        if k > 0 && (digits.len() - k) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 {
        out.insert(0, '-');
    }
    out
}

impl ShopMenu {
    pub fn new(data: ShopData, tab: ShopTab) -> ShopMenu {
        let mut menu = ShopMenu { data, tab, list: ListView::new(LIST, LIST_ITEM), layer: Layer::List, qty: 1, confirm: 0, notice: None, tab_flash: None, events: Vec::new(), texts: ShopTexts::default(), inline_qty: false };
        let n = menu.rows().len();
        menu.list.set_count(n);
        menu
    }

    pub fn rows(&self) -> &[ShopRow] {
        match self.tab {
            ShopTab::Buy => &self.data.buy,
            ShopTab::Sell => &self.data.sell,
        }
    }

    pub fn focused(&self) -> Option<&ShopRow> {
        (self.list.focus >= 0).then(|| self.rows().get(self.list.focus as usize)).flatten()
    }

    /// The host's data again (after a trade, every frame): the tab and the focused item are kept.
    pub fn set_data(&mut self, data: ShopData) {
        if data == self.data {
            return;
        }
        let key = self.focused().map(|r| r.key.clone());
        self.data = data;
        let n = self.rows().len();
        self.list.set_count(n);
        if let Some(k) = key.and_then(|key| self.rows().iter().position(|r| r.key == key)) {
            self.list.set_focus(k);
        }
        self.list.invalidate();
        // a layer whose row went away goes back to the list
        if self.layer != Layer::List && self.focused().is_none_or(|r| self.max_of(r) == 0) {
            self.layer = Layer::List;
        }
        if self.layer != Layer::List {
            self.qty = self.qty.clamp(1, self.focused().map_or(1, |r| self.max_of(r)).max(1));
        }
    }

    /// How many of this row can be traded now.
    pub fn max_of(&self, row: &ShopRow) -> u32 {
        let qty_max = self.data.qty_max.max(1);
        match self.tab {
            ShopTab::Buy => {
                let room = row.max.saturating_sub(row.owned);
                let afford = if row.price == 0 { u32::MAX } else { (self.data.munny.max(0) / row.price as i64).min(u32::MAX as i64) as u32 };
                room.min(afford).min(qty_max)
            }
            ShopTab::Sell => row.owned.min(qty_max),
        }
    }

    /// Why the row cannot be traded (None: it can).
    pub fn refusal(&self, row: &ShopRow) -> Option<Refused> {
        if self.max_of(row) > 0 {
            return None;
        }
        Some(match self.tab {
            ShopTab::Buy if row.owned >= row.max => Refused::Full,
            ShopTab::Buy => Refused::Munny,
            ShopTab::Sell => Refused::NoneHeld,
        })
    }

    fn refusal_text(&self, r: Refused) -> String {
        match r {
            Refused::Munny => self.texts.no_munny.clone(),
            Refused::Full => self.texts.full.clone(),
            Refused::NoneHeld => self.texts.none_held.clone(),
        }
    }

    /// The trade's total at the quantity shown.
    pub fn total(&self) -> i64 {
        self.focused().map_or(0, |r| r.price as i64 * self.qty as i64)
    }

    fn switch_tab(&mut self, right: bool, sounds: &mut Vec<UiSound>) {
        self.tab = if self.tab == ShopTab::Buy { ShopTab::Sell } else { ShopTab::Buy };
        self.tab_flash = Some(right);
        self.list.focus = 0;
        self.list.scroll = 0;
        let n = self.rows().len();
        self.list.set_count(n);
        self.list.invalidate();
        self.notice = None;
        sounds.push(UiSound::Play(SE_MENU_OPEN));
    }

    /// The quantity one step on (left / right +-1, up / down +-10): a fresh press past an end wraps (1 <-> the most),
    /// held it stops.
    fn step_qty(&mut self, key: CampKey, repeat: bool, sounds: &mut Vec<UiSound>) {
        let max = self.focused().map_or(1, |r| self.max_of(r)).max(1);
        let step: i64 = match key {
            CampKey::Right => 1,
            CampKey::Left => -1,
            CampKey::Up => 10,
            _ => -10,
        };
        let was = self.qty;
        let next = self.qty as i64 + step;
        self.qty = if next > max as i64 {
            if repeat || was < max { max } else { 1 }
        } else if next < 1 {
            if repeat || was > 1 { 1 } else { max }
        } else {
            next as u32
        };
        if self.qty != was {
            sounds.push(UiSound::Play(SE_MENU_CURSOR));
        }
    }

    /// A MENU KEY.
    pub fn key(&mut self, key: CampKey, repeat: bool, sounds: &mut Vec<UiSound>) {
        match self.layer {
            // (the Moogle shop: the quantity on the row)
            Layer::List if self.inline_qty && matches!(key, CampKey::Left | CampKey::Right) => {
                if self.focused().is_some_and(|r| self.max_of(r) > 0) {
                    self.step_qty(key, repeat, sounds);
                }
            }
            Layer::List => match key {
                CampKey::Up | CampKey::Down => {
                    if self.list.move_anchor(key == CampKey::Down, repeat) {
                        sounds.push(UiSound::Play(SE_MENU_CURSOR));
                        self.notice = None;
                        self.qty = 1;
                    }
                }
                CampKey::L1 | CampKey::R1 => {
                    let moved = if key == CampKey::R1 { self.list.next_page(repeat) } else { self.list.prev_page(repeat) };
                    if moved {
                        sounds.push(UiSound::Play(SE_MENU_CURSOR));
                        self.notice = None;
                        self.qty = 1;
                    }
                }
                CampKey::Left | CampKey::Right | CampKey::L2 | CampKey::R2 if !repeat => {
                    self.switch_tab(matches!(key, CampKey::Right | CampKey::R2), sounds);
                    self.qty = 1;
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
                            if self.inline_qty {
                                self.qty = self.qty.clamp(1, self.max_of(&row).max(1));
                                self.confirm = 0;
                                self.layer = Layer::Confirm;
                            } else {
                                self.qty = 1;
                                self.layer = Layer::Quantity;
                            }
                        }
                    }
                }
                CampKey::Cancel if !repeat => {
                    sounds.push(UiSound::Play(SE_MENU_CLOSE));
                    self.events.push(ShopEvent::Closed);
                }
                _ => {}
            },
            Layer::Quantity => match key {
                CampKey::Left | CampKey::Right | CampKey::Up | CampKey::Down => self.step_qty(key, repeat, sounds),
                CampKey::Decide if !repeat => {
                    sounds.push(UiSound::Play(SE_MENU_DECIDE));
                    self.confirm = 0;
                    self.layer = Layer::Confirm;
                }
                CampKey::Cancel if !repeat => {
                    sounds.push(UiSound::Play(SE_MENU_CLOSE));
                    self.layer = Layer::List;
                }
                _ => {}
            },
            Layer::Confirm => match key {
                CampKey::Up | CampKey::Down | CampKey::Left | CampKey::Right => {
                    self.confirm = 1 - self.confirm.min(1);
                    sounds.push(UiSound::Play(SE_MENU_CURSOR));
                }
                CampKey::Decide if !repeat && self.confirm == 0 => self.trade(sounds),
                // No / B: one level back (the quantity window; the Moogle shop: its list, the quantity kept)
                CampKey::Decide | CampKey::Cancel if !repeat => {
                    sounds.push(UiSound::Play(SE_MENU_CLOSE));
                    self.layer = if self.inline_qty { Layer::List } else { Layer::Quantity };
                }
                _ => {}
            },
        }
    }

    /// YES: the trade is asked of the host, and shown at once (the host's next data is the truth).
    fn trade(&mut self, sounds: &mut Vec<UiSound>) {
        let Some(row) = self.focused().cloned() else { return };
        let qty = self.qty.min(self.max_of(&row));
        if qty == 0 {
            sounds.push(UiSound::Play(SE_MENU_REFUSED));
            self.layer = Layer::List;
            return;
        }
        let total = row.price as i64 * qty as i64;
        let (event, munny) = match self.tab {
            ShopTab::Buy => (ShopEvent::Buy { key: row.key.clone(), qty, price: row.price }, self.data.munny - total),
            ShopTab::Sell => (ShopEvent::Sell { key: row.key.clone(), qty, price: row.price }, self.data.munny + total),
        };
        self.events.push(event);
        sounds.push(UiSound::Play(SE_TRADE));
        let gained = if self.tab == ShopTab::Buy { qty as i64 } else { -(qty as i64) };
        self.data.munny = munny;
        for list in [&mut self.data.buy, &mut self.data.sell] {
            for r in list.iter_mut().filter(|r| r.key == row.key) {
                r.owned = (r.owned as i64 + gained).max(0) as u32;
            }
        }
        // what was sold to the last one leaves the sell list
        let focus = self.list.focus;
        self.data.sell.retain(|r| r.owned > 0);
        let n = self.rows().len();
        self.list.set_count(n);
        if focus >= 0 && n > 0 {
            self.list.set_focus((focus as usize).min(n - 1));
        }
        self.list.invalidate();
        let (what, price) = match self.tab {
            ShopTab::Buy => (format!("{} munny", grouped(total)), format!("{} x {qty}", row.name)),
            ShopTab::Sell => (format!("{} x {qty}", row.name), format!("{} munny", grouped(total))),
        };
        self.notice = Some(self.texts.traded.replace("{0}", &what).replace("{1}", &price));
        self.layer = Layer::List;
        self.qty = 1;
    }

    pub fn take_events(&mut self) -> Vec<ShopEvent> {
        std::mem::take(&mut self.events)
    }
}

// ================================================================================================= the display

const BACKYARD: &str = "backyard.backyardAnm";
const ITEM_ROOT: &str = "backyard.backyardAnm.backyardItem";
const FRAME: &str = "stageFrame.stageObjR";
const GUIDE: &str = "stageFrame.stageObjR.btnInfo.btnInfoText.txText.textField";
const PANE: &str = "backyard.backyardAnm.itemStatusA1";
/// The tab icons (cells of `ItemListLibrary_bmpIcon`): BUY the Items cell, SELL the All cell.
const TAB_ICONS: [i32; 2] = [1, 0];

/// The category icon text of a row (as the camp's Items screen).
pub(crate) fn category_icon(category: u8) -> &'static str {
    use crate::camp_equip::{CAT_ACCESSORY, CAT_ARMOUR, CAT_ITEM, CAT_KEY, CAT_MATERIAL, CAT_WEAPON};
    match category {
        CAT_ITEM => "<icon(BattleItem)>",
        CAT_WEAPON => "<icon(Weapon)>",
        CAT_ARMOUR => "<icon(Armor)>",
        CAT_ACCESSORY => "<icon(Accessory)>",
        CAT_MATERIAL => "<icon(MixItem)>",
        CAT_KEY => "<icon(KeyItem)>",
        _ => "<icon(CampItem)>",
    }
}

pub struct ShopView {
    pub movie: Movie,
    pub dialog: Movie,
    pub fonts: Fonts,
    strings: HashMap<String, String>,
    pub art: crate::item_art::ItemArt,
    open: bool,
    closing: bool,
    /// Frames since the open: the frame's clips exist once their timelines placed them.
    age: u32,
    tabs: TabBar,
    stamp: u64,
    shown_tab: Option<ShopTab>,
    guide: String,
    pane_item: Option<Option<String>>,
    dialog_open: bool,
    dialog_closing: bool,
    dialog_stamp: u64,
}

pub(crate) fn stamp_of(value: &impl std::fmt::Debug) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    format!("{value:?}").hash(&mut h);
    h.finish()
}

impl ShopView {
    pub fn load(pack: &mut Pack, fonts: Fonts) -> Result<ShopView, PackError> {
        let def: Arc<MovieDef> = pack.movie_linked("CampMenu", &crate::camp_menu::LIBRARIES)?;
        let dialog = pack.movie_linked("Dialog", &crate::camp_menu::LIBRARIES)?;
        let mut strings = HashMap::new();
        for table in ["kh3_menu", "kh3_system", "kh3_item", "kh3_camp"] {
            strings.extend(pack.string_table(table));
        }
        let art = crate::item_art::ItemArt::load(pack);
        Ok(ShopView { movie: Movie::new(def), dialog: Movie::new(dialog), fonts, strings, art, open: false, closing: false, age: 0, tabs: TabBar::new(format!("{ITEM_ROOT}.listTabCamp")), stamp: 0, shown_tab: None, guide: String::new(), pane_item: None, dialog_open: false, dialog_closing: false, dialog_stamp: 0 })
    }

    /// A string of the pack, `<br>` as a line break; `fallback` when the pack has none.
    pub fn string(&self, key: &str, fallback: &str) -> String {
        self.strings.get(key).map(|s| s.replace("<br>", "\n")).filter(|s| !s.is_empty()).unwrap_or_else(|| fallback.to_string())
    }

    /// KH3's notice texts for the logic.
    pub fn texts(&self) -> ShopTexts {
        let d = ShopTexts::default();
        ShopTexts { no_munny: self.string("kh3_menu.shop.dialog_0020", &d.no_munny), full: self.string("kh3_menu.shop.dialog_0010", &d.full), none_held: d.none_held, traded: self.string("kh3_menu.shop.dialog_0310", &d.traded) }
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

    /// THE SHOP OPENS: the camp menu's movie comes in (root `in`) straight onto its Items screen (the top screen stays off).
    pub fn open(&mut self, menu: &mut ShopMenu) {
        menu.texts = self.texts();
        self.movie.goto_and_play("", "in");
        // the top screen (its entries, the objective line) never shows in the shop: the root's `in` would bring it
        self.movie.goto_and_play("topMenu", "off");
        self.movie.set_visible("topMenu", false);
        self.movie.goto_and_play("backyard", "in");
        self.movie.goto_and_play(BACKYARD, "backyard");
        self.movie.set_visible(&format!("{BACKYARD}.charaList"), false);
        self.open = true;
        self.closing = false;
        self.age = 0;
        self.stamp = 0;
        self.shown_tab = None;
        self.guide.clear();
        self.pane_item = None;
        menu.list.invalidate();
    }

    /// The shop leaves (root `out`); [`ShopView::tick`] says when it is gone.
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

    /// The item status pane (`ItemStatusA1`): the picture and the text (as the camp's Items screen, IMPL_ER37).
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
        let field = format!("{PANE}.txHelp.textField");
        let help = match self.movie.node(&field).map(|n| &n.kind) {
            Some(crate::player::Kind::Text(t)) => crate::render::fit_text(&self.fonts, &t.def, help),
            _ => help.to_string(),
        };
        self.text(&field, &help);
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

    /// The help text of an item: the host's own, else the game's by the item key, else none.
    pub fn help_of(&self, row: &ShopRow) -> String {
        if !row.help.is_empty() {
            return row.help.clone();
        }
        self.art.help_id(&row.key).and_then(|key| self.strings.get(key)).map(|t| t.replace("<br>", "\n")).unwrap_or_default()
    }

    /// Reflects the logic: the frame, the tab, the rows, the pane, the help line, the guide, the dialog.
    pub fn sync(&mut self, menu: &mut ShopMenu) {
        if !self.open || self.closing {
            return;
        }
        // the frame (munny, the merchant's name) once its clips are there
        if self.movie.exists(&format!("{FRAME}.munny.munnyNum.textField")) {
            let munny = menu.data.munny.max(0).min(99_999_999).to_string();
            self.text(&format!("{FRAME}.munny.munnyNum.textField"), &munny);
            let word = self.string("kh3_menu.shop.munny_0010", "MUNNY");
            self.text(&format!("{FRAME}.munny.txMunny.coText"), &word);
            let title = if menu.data.title.is_empty() { self.string("kh3_menu.shop.shop_0010", "Moogle Shop") } else { menu.data.title.clone() };
            self.text(&format!("{FRAME}.areaInfo.areaName.txName"), &title);
            self.movie.set_visible(&format!("{FRAME}.areaInfo.worldTitle"), false);
            self.movie.set_visible(&format!("{FRAME}.playTime"), false);
        }
        if !self.movie.exists(&format!("{ITEM_ROOT}.backyardList.container")) {
            self.sync_dialog(Some(menu));
            return;
        }
        let stamp = stamp_of(&(&menu.data, menu.tab, &menu.list, menu.layer, menu.qty, menu.confirm, &menu.notice)) ^ self.movie.exists(&menu.list.item_path(0)) as u64;
        if stamp != self.stamp {
            let fresh = self.stamp == 0;
            self.stamp = stamp;
            if fresh {
                let (title, name, price) = (self.string("kh3_menu.shop.tab_1010", "SHOP"), self.string("kh3_menu.shop.item_0010", "Name"), self.string("kh3_menu.shop.item_0050", "Price"));
                self.text("backyard.stageCategory.category.categoryName.txName", &title);
                self.text(&format!("{ITEM_ROOT}.backyardListTitle.textField"), &name);
                self.text(&format!("{ITEM_ROOT}.backyardListTitle.backyardListTitleCnt"), &price);
                self.tabs.select = if menu.tab == ShopTab::Buy { 0 } else { 1 };
                self.tabs.build(&mut self.movie, 2, &TAB_ICONS);
                self.tabs.set_switch_icons(&mut self.movie, "<icon(L2)>", "<icon(R2)>", &self.fonts);
            }
            if self.shown_tab != Some(menu.tab) {
                let icon = if menu.tab == ShopTab::Buy { "shop1" } else { "shop2" };
                self.movie.goto_and_stop("backyard.stageCategory.category.categoryIcon", icon);
                self.movie.goto_and_stop("backyard.stageCategory.stageObjL.bgCategoryIcon", icon);
                self.shown_tab = Some(menu.tab);
            }
            let tab_name = if menu.tab == ShopTab::Buy { self.string("kh3_menu.shop.tab_1020", "BUY") } else { self.string("kh3_menu.shop.tab_1030", "SELL") };
            let index = if menu.tab == ShopTab::Buy { 0 } else { 1 };
            let fonts = self.fonts.clone();
            self.tabs.select(&mut self.movie, index, &tab_name, menu.tab_flash.take(), &fonts);
            // the help line: the notice, else KH3's line for the tab
            let help = menu.notice.clone().unwrap_or_else(|| if menu.tab == ShopTab::Buy { self.string("kh3_menu.shop.detail_0110", "") } else { self.string("kh3_menu.shop.detail_0120", "") });
            self.text("backyard.helpText.txHelpText.textField", &help);
            let rows: Vec<ShopRow> = menu.rows().to_vec();
            let empty = if rows.is_empty() { self.string("kh3_system.general.system_0011", "") } else { String::new() };
            self.text(&format!("{ITEM_ROOT}.textField"), &empty);
            // the pane: picture, "Owned", the help text
            let focused = menu.focused().cloned();
            let pane_text = focused.as_ref().map(|r| {
                let owned = format!("<color(strong)>Owned<color>  {} / {}", r.owned, r.max);
                let help = self.help_of(r);
                if help.is_empty() { owned } else { format!("{owned}\n{help}") }
            }).unwrap_or_default();
            self.pane(focused.as_ref().map(|r| r.key.as_str()), &pane_text);
            // the rows: icon, name, price ("disable" when it cannot be traded now)
            let refusals: Vec<bool> = rows.iter().map(|r| menu.refusal(r).is_none()).collect();
            menu.list.has_focus = true;
            menu.list.refresh(&mut self.movie, |movie, path, index| {
                let row = &rows[index];
                movie.set_text(&format!("{path}.textIcon"), category_icon(row.category), &fonts);
                movie.set_text(&format!("{path}.backyardItemName"), &row.name, &fonts);
                movie.set_text(&format!("{path}.itemCnt"), &row.price.to_string(), &fonts);
                // (the count column's "x" goes: the column is a price here)
                movie.set_visible(&format!("{path}.coTextBack"), false);
                RowState { enabled: refusals[index] }
            });
            let change = self.string("kh3_menu.shop.button_0010", "<icon(KeyLeftRight)> Change Quantity");
            let guide = match menu.layer {
                Layer::List => format!("<icon(Decide)> {}  <icon(L2)><icon(R2)> {} / {}  <icon(Cancel)> Back", if menu.tab == ShopTab::Buy { self.string("kh3_menu.shop.menu_0010", "Buy") } else { self.string("kh3_menu.shop.menu_0020", "Sell") }, self.string("kh3_menu.shop.menu_0010", "Buy"), self.string("kh3_menu.shop.menu_0020", "Sell")),
                Layer::Quantity => format!("{change}  <icon(Decide)> OK  <icon(Cancel)> Back"),
                Layer::Confirm => "<icon(Decide)> OK  <icon(Cancel)> Back".to_string(),
            };
            if self.age > 20 {
                self.set_guide(&guide);
            } else {
                self.stamp = 1; // (the plate comes in with the frame: the guide is written again next frame)
            }
        }
        self.sync_dialog(Some(menu));
    }

    /// The quantity window and the confirmation (the `Dialog` movie: its window, text field and buttons).
    fn sync_dialog(&mut self, menu: Option<&ShopMenu>) {
        let content = menu.filter(|m| m.layer != Layer::List).and_then(|m| {
            let row = m.focused()?;
            let total = row.price as i64 * m.qty as i64;
            let max = m.max_of(row);
            let word = self.string("kh3_menu.shop.dialog_0110", "MUNNY");
            Some(match m.layer {
                Layer::Quantity => (
                    // (two lines: the window keeps its authored size and its button sits under the second)
                    format!("<color(strong)>{}<color>   <icon(KeyLeftRight)> x {}{}\n{}  {word}", row.name, m.qty, if max > 1 { String::new() } else { " (1)".to_string() }, grouped(total)),
                    vec![if m.tab == ShopTab::Buy { self.string("kh3_menu.shop.menu_0010", "Buy") } else { self.string("kh3_menu.shop.menu_0020", "Sell") }],
                    0,
                ),
                _ => (
                    format!("{}\n<color(strong)>{}<color> x {}   {}  {word}", if m.tab == ShopTab::Buy { self.string("kh3_menu.shop.dialog_0210", "") } else { self.string("kh3_menu.shop.dialog_0220", "") }, row.name, m.qty, grouped(total)),
                    vec![self.string("kh3_system.general.choice_0001", "Yes"), self.string("kh3_system.general.choice_0002", "No")],
                    m.confirm,
                ),
            })
        });
        let Some((text, buttons, cursor)) = content else {
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
            self.dialog.set_visible(name, n < buttons.len());
            if let Some(label) = buttons.get(n) {
                self.dialog.set_text(&format!("{name}.dialogBtnText.textField"), label, &self.fonts);
                self.dialog.goto_and_play(name, if cursor == n { "select" } else { "normal" });
            }
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

    /// One stage frame. True when the close animation ended (the shop is gone).
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
        // (a movie whose close event never comes: gone after its out has had time to play)
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

    fn row(key: &str, price: u32, owned: u32) -> ShopRow {
        ShopRow { key: key.into(), name: key.into(), price, owned, max: 99, category: crate::camp_equip::CAT_ITEM, help: String::new() }
    }

    fn menu() -> ShopMenu {
        ShopMenu::new(ShopData { title: "Merchant".into(), munny: 1000, buy: vec![row("ITEM_POTION", 300, 2), row("ITEM_ELIXIR", 5000, 0)], sell: vec![row("ITEM_POTION", 75, 2), row("PRT_ITEM01", 400, 1)], qty_max: 99 }, ShopTab::Buy)
    }

    /// (SHOP) BUY: A on a row -> the quantity (left / right, wraps, stops at what the munny reaches), A -> the
    /// confirmation, Yes -> the trade is asked and shown; B steps back one level at a time; B on the list closes.
    #[test]
    fn shop_buy_quantity_confirm_and_back_one_level() {
        let mut m = menu();
        let mut s = Vec::new();
        m.key(CampKey::Decide, false, &mut s);
        assert_eq!((m.layer, m.qty), (Layer::Quantity, 1));
        m.key(CampKey::Right, false, &mut s);
        m.key(CampKey::Right, false, &mut s);
        assert_eq!(m.qty, 3, "1000 munny reach three Potions at 300");
        m.key(CampKey::Right, false, &mut s);
        assert_eq!(m.qty, 1, "a fresh press past the most wraps");
        m.key(CampKey::Left, false, &mut s);
        assert_eq!(m.qty, 3);
        m.key(CampKey::Up, true, &mut s);
        assert_eq!(m.qty, 3, "held: stops at the most");
        m.key(CampKey::Decide, false, &mut s);
        assert_eq!(m.layer, Layer::Confirm);
        m.key(CampKey::Cancel, false, &mut s);
        assert_eq!(m.layer, Layer::Quantity, "B: one level back");
        m.key(CampKey::Decide, false, &mut s);
        m.key(CampKey::Down, false, &mut s);
        assert_eq!(m.confirm, 1);
        m.key(CampKey::Decide, false, &mut s);
        assert_eq!(m.layer, Layer::Quantity, "No: back to the quantity");
        m.key(CampKey::Decide, false, &mut s);
        m.key(CampKey::Decide, false, &mut s);
        assert_eq!(m.take_events(), vec![ShopEvent::Buy { key: "ITEM_POTION".into(), qty: 3, price: 300 }]);
        assert_eq!((m.layer, m.data.munny, m.data.buy[0].owned, m.data.sell[0].owned), (Layer::List, 100, 5, 5));
        assert!(m.notice.as_deref().unwrap_or("").contains("900 munny"));
        assert!(s.contains(&UiSound::Play(SE_TRADE)));
        // the Elixir: not enough munny: refused with KH3's words, no layer
        m.key(CampKey::Down, false, &mut s);
        m.key(CampKey::Decide, false, &mut s);
        assert_eq!((m.layer, m.notice.as_deref()), (Layer::List, Some("You don't have enough munny.")));
        assert!(m.take_events().is_empty());
        m.key(CampKey::Cancel, false, &mut s);
        assert_eq!(m.take_events(), vec![ShopEvent::Closed]);
    }

    /// (SHOP) SELL: the tab switch (left / right / LT / RT), at most what he holds, the row leaves when all is sold.
    #[test]
    fn shop_sell_tab_and_the_last_one() {
        let mut m = menu();
        let mut s = Vec::new();
        m.key(CampKey::R2, false, &mut s);
        assert_eq!((m.tab, m.rows().len()), (ShopTab::Sell, 2));
        m.key(CampKey::Down, false, &mut s);
        m.key(CampKey::Decide, false, &mut s);
        m.key(CampKey::Right, false, &mut s);
        assert_eq!(m.qty, 1, "one held: the quantity cannot rise");
        m.key(CampKey::Decide, false, &mut s);
        m.key(CampKey::Decide, false, &mut s);
        assert_eq!(m.take_events(), vec![ShopEvent::Sell { key: "PRT_ITEM01".into(), qty: 1, price: 400 }]);
        assert_eq!((m.data.munny, m.rows().len(), m.focused().map(|r| r.key.as_str())), (1400, 1, Some("ITEM_POTION")));
        m.key(CampKey::Left, false, &mut s);
        assert_eq!(m.tab, ShopTab::Buy);
        // the host's data: kept tab and focus, the layer goes back when its row is gone
        m.key(CampKey::Down, false, &mut s);
        let mut d = m.data.clone();
        d.buy.reverse();
        m.set_data(d);
        assert_eq!(m.focused().map(|r| r.key.as_str()), Some("ITEM_ELIXIR"), "the focus follows the item");
    }

    #[test]
    fn shop_grouped_numbers() {
        assert_eq!((grouped(0), grouped(999), grouped(1000), grouped(1_234_567), grouped(-4500)), ("0".into(), "999".into(), "1,000".into(), "1,234,567".into(), "-4,500".into()));
    }
}
