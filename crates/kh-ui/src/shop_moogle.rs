//! (SHOP) KH3'S OWN MOOGLE SHOP SCREEN (movie `41_shop/Shop`, the user 2026-10-23: "Port the real Moogle shop"): the
//! same logic as the camp-menu screen ([`crate::shop::ShopMenu`], in KH3's own way: the quantity changed on the row,
//! A straight to the confirmation) drawn by the game's own movie. State note: build/sora_tmp/party/IMPL_SHOP_STATE.md.
//! Needs the movie in the UI pack (build/sora_tmp/party/SHOP/pack + MERGE_NOTE.md); without it the host keeps the
//! camp-menu screen (`crate::shop::ShopView`).
//!
//! WHAT IS KH3'S [A] (the movie, its AS3 classes, read in `as3/Shop.as3.txt` of the pack):
//! - the document class `Shop`: `stageBg` (the shop's warm stage, `in`; `sellIn` / `sellOut` = the sell screen's own
//!   tint), `stageFrame` (the frame: MUNNY counter, the area line, the button guide plate), `menuList` (`McMenuTrade`:
//!   `inNext` / `outBack`, its `stageCategory` strip with KH3's shop icons), `mesWin` (`McMesWin1`: the purchase
//!   confirmation: the item row, the quantity, the price, MUNNY before -> after with the MAX mark, the help line, the
//!   Yes / No buttons `mesBtn1` / `mesBtn2`), the root's `in` / `out` (`ANIM_END_OUT` at its end);
//! - the trade list `McTradeList`: `CoTradeList` (10 rows, scroll loop, paging), the column heads (Name / Qty. / Price /
//!   Stock), `CoTradeListItem` rows (icon + name, the quantity input `numInput`, `numPrice`, `x` + `numShojisuu`,
//!   states normal / select / disable / disableSelect), `listLines` (`empty` with its text), the tab bar
//!   `ListTabShop` (LT / RT), the item status pane `ItemStatusA1`, the help line;
//! - KH3's texts (`kh3_menu.shop.*`) and the menu sounds.
//! COMPOSED [L]: the entry goes STRAIGHT to the BUY or the SELL list the merchant's talk asked for (the user: no hub;
//! KH3's own top menu `menuTop` and its `chara` stay off); the tab bar holds BUY / SELL (KH3's holds the item
//! categories); the Moogle himself is a live 3D render in KH3 (`CoImgChara` is fed by the game's capture, no picture of
//! it is in the files): not shown: the merchant is Elden Ring's own character in the world.

use std::collections::HashMap;
use std::sync::Arc;

use crate::list::{RowState, TabBar};
use crate::shop::{category_icon, grouped, stamp_of, Layer, ShopMenu, ShopRow, ShopTab, ShopTexts};
use crate::{Fonts, Movie, MovieDef, Pack, PackError};

/// The libraries the movie places symbols of.
pub const LIBRARIES: [&str; 5] = ["MainLibrary", "ItemListLibrary", "ItemStatusLibrary", "SystemLibrary", "MarkLibrary"];
pub const MOVIE: &str = "Shop";

pub const LIST: &str = "menuList.listMenuObj.itemList.coList";
pub const LIST_ITEM: &str = "CoTradeListItem";
const ITEM_LIST: &str = "menuList.listMenuObj.itemList";
const TAB: &str = "menuList.listMenuObj.itemList.coTab";
const PANE: &str = "menuList.listMenuObj.itemStatus";
const HELP: &str = "menuList.listMenuObj.helpText.txHelpText.textField";
const CATEGORY: &str = "menuList.stageCategory";
const FRAME: &str = "stageFrame.stageObjR";
const GUIDE: &str = "stageFrame.stageObjR.btnInfo.btnInfoText.txText.textField";
const MES: &str = "mesWin.mesWinAnm.mesWinObj";
const TAB_ICONS: [i32; 2] = [1, 0];

pub struct MoogleShopView {
    pub movie: Movie,
    pub fonts: Fonts,
    strings: HashMap<String, String>,
    pub art: crate::item_art::ItemArt,
    open: bool,
    closing: bool,
    age: u32,
    tabs: TabBar,
    stamp: u64,
    guide: String,
    pane_item: Option<Option<String>>,
    /// The stage's tint as played (false: the buy look).
    shown_sell: bool,
    mes_open: bool,
    mes_stamp: u64,
}

impl MoogleShopView {
    pub fn load(pack: &mut Pack, fonts: Fonts) -> Result<MoogleShopView, PackError> {
        let def: Arc<MovieDef> = pack.movie_linked(MOVIE, &LIBRARIES)?;
        let mut strings = HashMap::new();
        for table in ["kh3_menu", "kh3_system", "kh3_item", "kh3_camp"] {
            strings.extend(pack.string_table(table));
        }
        let art = crate::item_art::ItemArt::load(pack);
        let movie = Movie::new(def);

        Ok(MoogleShopView { movie, fonts, strings, art, open: false, closing: false, age: 0, tabs: TabBar::new(TAB), stamp: 0, guide: String::new(), pane_item: None, shown_sell: false, mes_open: false, mes_stamp: 0 })
    }

    pub fn string(&self, key: &str, fallback: &str) -> String {
        self.strings.get(key).map(|s| s.replace("<br>", "\n")).filter(|s| !s.is_empty()).unwrap_or_else(|| fallback.to_string())
    }

    pub fn texts(&self) -> ShopTexts {
        let d = ShopTexts::default();
        ShopTexts { no_munny: self.string("kh3_menu.shop.dialog_0020", &d.no_munny), full: self.string("kh3_menu.shop.dialog_0010", &d.full), none_held: d.none_held, traded: self.string("kh3_menu.shop.dialog_0310", &d.traded) }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    fn text(&mut self, path: &str, text: &str) {
        self.movie.set_text(path, text, &self.fonts);
    }

    /// THE SHOP OPENS STRAIGHT ON THE LIST the talk asked for (BUY or SELL): the stage, the frame and the trade list come
    /// in together (root `in`, `stageBg` `in`, `stageFrame` `in`, `menuList` `inNext`).
    pub fn open(&mut self, menu: &mut ShopMenu) {
        menu.texts = self.texts();
        menu.inline_qty = true;
        menu.list = crate::list::ListView::new(LIST, LIST_ITEM);
        let n = menu.rows().len();
        menu.list.set_count(n);
        // (the root places its parts on its first frames: their own `in` is played after that, [`Self::start_parts`])
        self.movie.goto_and_play("", "in");
        for clip in ["menuTop", "chara", "mesWin", "mesWin2", "menuMixTop", "menuSynthesis", "menuKeyb", "menuPhoto", "menuCollection", "menuMaterial", "screenEffBlack"] {
            self.movie.goto_and_play(clip, "off");
        }
        self.open = true;
        self.closing = false;
        self.age = 0;
        self.stamp = 0;
        self.guide.clear();
        self.pane_item = None;
        self.shown_sell = false;
        self.mes_open = false;
        self.mes_stamp = 0;
    }

    /// The stage, the frame and the trade list come in (once the root has placed them).
    fn start_parts(&mut self) {
        for clip in ["stageBg", "stageFrame"] {
            self.movie.goto_and_play(clip, "in");
        }
        self.movie.goto_and_play("menuList", "inNext");
        self.stamp = 0;
    }

    /// The shop leaves (the list `outBack`, the frame `out`, the root `out`); [`MoogleShopView::tick`] says when it is gone.
    pub fn close(&mut self) {
        if self.open && !self.closing {
            if self.mes_open {
                self.movie.goto_and_play("mesWin", "out");
                self.mes_open = false;
            }
            self.movie.goto_and_play("menuList", "outBack");
            self.movie.goto_and_play("stageFrame", "out");
            self.movie.goto_and_play("", "out");
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

    /// The item status pane (`ItemStatusA1`): the picture and the text (as the camp's screens, IMPL_ER37).
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

    fn help_of(&self, row: &ShopRow) -> String {
        if !row.help.is_empty() {
            return row.help.clone();
        }
        self.art.help_id(&row.key).and_then(|key| self.strings.get(key)).map(|t| t.replace("<br>", "\n")).unwrap_or_default()
    }

    /// Reflects the logic: the frame, the strip, the tab, the rows, the pane, the help line, the guide, the window.
    pub fn sync(&mut self, menu: &mut ShopMenu) {
        if !self.open || self.closing {
            return;
        }
        let sell = menu.tab == ShopTab::Sell;
        if self.movie.exists(&format!("{FRAME}.munny.munnyNum.textField")) {
            let munny = menu.data.munny.max(0).min(999_999).to_string();
            self.text(&format!("{FRAME}.munny.munnyNum.textField"), &munny);
            let word = self.string("kh3_menu.shop.munny_0010", "MUNNY");
            self.text(&format!("{FRAME}.munny.txMunny.coText.textField"), &word);
            let title = if menu.data.title.is_empty() { self.string("kh3_menu.shop.shop_0010", "Moogle Shop") } else { menu.data.title.clone() };
            self.text(&format!("{FRAME}.areaInfo.areaName.txName.textField"), &title);
            self.movie.goto_and_stop(&format!("{FRAME}.areaInfo.worldTitle"), "off");
            self.movie.goto_and_stop(&format!("{FRAME}.playTime"), "off");
        }
        // the sell screen's own tint (once the stage is in)
        if self.age > 18 && self.shown_sell != sell {
            self.movie.goto_and_play("stageBg", if sell { "sellIn" } else { "sellOut" });
            self.shown_sell = sell;
        }
        if !self.movie.exists(&format!("{LIST}.container")) {
            self.sync_mes(menu);
            return;
        }
        let stamp = stamp_of(&(&menu.data, menu.tab, &menu.list, menu.layer, menu.qty, menu.confirm, &menu.notice)) ^ self.movie.exists(&menu.list.item_path(0)) as u64;
        if stamp != self.stamp {
            let fresh = self.stamp == 0;
            self.stamp = stamp;
            if fresh {
                for (field, key, fallback) in [("txItem", "kh3_menu.shop.item_0010", "Name"), ("txKosuu", "kh3_menu.shop.item_0020", "Qty."), ("txMunny", "kh3_menu.shop.item_0050", "Price"), ("txShojisuu", "kh3_menu.shop.item_0040", "Stock")] {
                    let t = self.string(key, fallback);
                    self.text(&format!("{ITEM_LIST}.{field}.textField"), &t);
                }
                self.tabs.select = if sell { 1 } else { 0 };
                self.tabs.build(&mut self.movie, 2, &TAB_ICONS);
                self.tabs.set_switch_icons(&mut self.movie, "<icon(L2)>", "<icon(R2)>", &self.fonts);
            }
            // the strip: KH3's word and icon for the screen
            let (word, icon) = if sell { (self.string("kh3_menu.shop.tab_1030", "SELL"), "shop2") } else { (self.string("kh3_menu.shop.tab_1020", "BUY"), "shop1") };
            self.text(&format!("{CATEGORY}.category.categoryName.txName.textField"), &word);
            self.movie.goto_and_stop(&format!("{CATEGORY}.category.categoryIcon"), icon);
            self.movie.goto_and_stop(&format!("{CATEGORY}.stageObjL.bgCategoryIcon"), icon);
            let fonts = self.fonts.clone();
            self.tabs.select(&mut self.movie, if sell { 1 } else { 0 }, &word, menu.tab_flash.take(), &fonts);
            let help = menu.notice.clone().unwrap_or_else(|| if sell { self.string("kh3_menu.shop.detail_0120", "") } else { self.string("kh3_menu.shop.detail_0110", "") });
            self.text(HELP, &help);
            let rows: Vec<ShopRow> = menu.rows().to_vec();
            let lines = format!("{ITEM_LIST}.listLines");
            let want = if rows.is_empty() { "empty" } else { "loop" };
            if self.movie.current_label(&lines) != Some(want) {
                self.movie.goto_and_stop(&lines, want);
            }
            if rows.is_empty() {
                let empty = self.string("kh3_system.general.system_0011", "");
                self.text(&format!("{lines}.txDataEmpty"), &empty);
            }
            let focused = menu.focused().cloned();
            let pane_text = focused.as_ref().map(|r| self.help_of(r)).unwrap_or_default();
            self.pane(focused.as_ref().map(|r| r.key.as_str()), &pane_text);
            let enabled: Vec<bool> = rows.iter().map(|r| menu.refusal(r).is_none()).collect();
            let (focus, qty) = (menu.list.focus, menu.qty);
            menu.list.has_focus = true;
            menu.list.refresh(&mut self.movie, |movie, path, index| {
                let row = &rows[index];
                movie.set_text(&format!("{path}.itemLabel.txIcon"), category_icon(row.category), &fonts);
                movie.set_text(&format!("{path}.itemLabel.txName.textField"), &row.name, &fonts);
                // the quantity on the focused row only (KH3: the input of the row under the cursor)
                let input = if focus == index as i32 && enabled[index] { qty.to_string() } else { String::new() };
                movie.set_text(&format!("{path}.numInput.textField"), &input, &fonts);
                movie.set_text(&format!("{path}.numPrice.textField"), &row.price.to_string(), &fonts);
                movie.set_text(&format!("{path}.numShojisuu.textField"), &row.owned.to_string(), &fonts);
                movie.goto_and_stop(&format!("{path}.newMark"), "off");
                RowState { enabled: enabled[index] }
            });
            let verb = if sell { self.string("kh3_menu.shop.menu_0020", "Sell") } else { self.string("kh3_menu.shop.menu_0010", "Buy") };
            let change = self.string("kh3_menu.shop.button_0010", "<icon(KeyLeftRight)> Change Quantity");
            let guide = match menu.layer {
                Layer::Confirm => "<icon(Decide)> OK  <icon(Cancel)> Back".to_string(),
                _ => format!("{change}  <icon(Decide)> {verb}  <icon(L2)><icon(R2)> {} / {}  <icon(Cancel)> Back", self.string("kh3_menu.shop.menu_0010", "Buy"), self.string("kh3_menu.shop.menu_0020", "Sell")),
            };
            if self.age > 20 {
                self.set_guide(&guide);
            } else {
                self.stamp = 1;
            }
        }
        self.sync_mes(menu);
    }

    /// The purchase / sale confirmation (`mesWin`, KH3's own window): the row, MUNNY before -> after, Yes / No.
    fn sync_mes(&mut self, menu: &ShopMenu) {
        let want = menu.layer == Layer::Confirm && menu.focused().is_some();
        if !want {
            if self.mes_open {
                self.movie.goto_and_play("mesWin", "out");
                self.mes_open = false;
                self.mes_stamp = 0;
            }
            return;
        }
        let Some(row) = menu.focused().cloned() else { return };
        let sell = menu.tab == ShopTab::Sell;
        let total = row.price as i64 * menu.qty as i64;
        let after = if sell { menu.data.munny + total } else { menu.data.munny - total };
        let stamp = stamp_of(&(&row, menu.qty, menu.confirm, menu.data.munny, sell));
        if !self.mes_open {
            self.movie.goto_and_play("mesWin", "in");
            self.mes_open = true;
        }
        if stamp == self.mes_stamp {
            return;
        }
        self.mes_stamp = stamp;
        for (field, key, fallback) in [("txItem", "kh3_menu.shop.item_0010", "Name"), ("txKosuu", "kh3_menu.shop.item_0030", "Qty."), ("txMunny", "kh3_menu.shop.item_0050", "Price")] {
            let t = self.string(key, fallback);
            self.text(&format!("{MES}.{field}.textField"), &t);
        }
        self.text(&format!("{MES}.itemLabel.txIcon"), category_icon(row.category));
        self.text(&format!("{MES}.itemLabel.txName.textField"), &row.name);
        self.text(&format!("{MES}.numKosuu.textField"), &menu.qty.to_string());
        self.text(&format!("{MES}.numPrice.textField"), &total.to_string());
        let word = self.string("kh3_menu.shop.dialog_0110", "MUNNY");
        self.text(&format!("{MES}.txMesMunny"), &word);
        self.text(&format!("{MES}.numMunny1.textField"), &menu.data.munny.max(0).to_string());
        self.text(&format!("{MES}.numMunny2.textField"), &after.max(0).to_string());
        // MAX: the stock reaches its maximum with this purchase
        let max = !sell && row.owned + menu.qty >= row.max;
        self.movie.goto_and_stop(&format!("{MES}.txMesMax"), if max { "loop" } else { "off" });
        self.movie.set_visible(&format!("{MES}.txMesMax"), max);
        let ask = if sell { self.string("kh3_menu.shop.dialog_0220", "") } else { self.string("kh3_menu.shop.dialog_0210", "") };
        self.text(&format!("{MES}.txMesHelp.textField"), &ask);
        for (n, (button, key, fallback)) in [("mesBtn1", "kh3_system.general.choice_0001", "Yes"), ("mesBtn2", "kh3_system.general.choice_0002", "No")].into_iter().enumerate() {
            let label = self.string(key, fallback);
            self.text(&format!("{MES}.{button}.mesBtnText.txMesBtnText.textField"), &label);
            let state = if menu.confirm == n { "select" } else { "normal" };
            if self.movie.current_label(&format!("{MES}.{button}")) != Some(state) {
                self.movie.goto_and_play(&format!("{MES}.{button}"), state);
            }
        }
        let _ = grouped(total); // (the window shows KH3's plain digits)
    }

    /// One stage frame. True when the close animation ended.
    pub fn tick(&mut self) -> bool {
        if !self.open {
            return false;
        }
        self.age = self.age.saturating_add(1);
        self.movie.tick();
        if self.age == 2 && !self.closing {
            self.start_parts();
        }
        let mut ended = false;
        for event in self.movie.take_events() {
            if event.name == "ANIM_END_OUT" && self.closing {
                ended = true;
            }
        }
        if self.closing && self.age > 4 && self.movie.current_label("") == Some("off") {
            ended = true;
        }
        if ended {
            self.open = false;
            self.closing = false;
        }
        ended
    }
}
