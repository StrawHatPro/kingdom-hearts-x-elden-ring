//! (SHOP) THE KH SHOP IN THE HUD (kh_ui::shop: the screen; crate::shop: the rules). A host opens it with
//! [`Hud::open_shop`] (the Elden Ring mod: when a merchant's talk script asks for its buy or sell window); while it is up
//! [`Hud::paused`] is true (the host holds its world as for the menus), it has the pad (D-pad / stick, A, B, LB / RB,
//! LT / RT; the keyboard's keys arrive as those buttons; `menu ...` words too) and it is drawn on the camp menu's layers
//! ("CampMenu" for the screen, "Dialog" for its quantity / confirmation window: the camp menu is never up with it).
//!
//! THE STOCK IS THE HUD'S: the rows are rebuilt every frame from what Sora holds (battle items: the pouch's stock; the
//! rest: the camp menu's stock), so the shop always shows the truth. A purchase or a sale is carried out in
//! [`Hud::apply_actions`] (the one place the HUD writes to the simulation): munny on Sora's status, the item in or out of
//! the stock, with the checks again (enough munny, room under the stock's maximum, enough held). The save keeps both
//! (kh-host save.rs: munny, the pouch's stock, the camp menu's stock). Worn pieces are never in the sell list.

use kh_ui::camp::CampKey;
use kh_ui::camp_equip::{CAT_ACCESSORY, CAT_ARMOUR, CAT_ITEM, CAT_MATERIAL, CAT_WEAPON};
use kh_ui::shop::{ShopData, ShopEvent, ShopMenu, ShopRow, ShopTab, ShopView};
use kh_ui::shop_moogle::MoogleShopView;

use super::{Frame, Hud, STOCK_MAX};
use crate::items::Class;

/// What a host asks for when it opens the shop.
#[derive(Clone, Debug, PartialEq)]
pub struct ShopOpen {
    /// The merchant's name ("" = KH3's "Moogle Shop").
    pub title: String,
    /// The shop tier (crate::shop: the area's).
    pub tier: u8,
    /// The tab it opens on (Elden Ring: "Purchase" -> BUY, "Sell" -> SELL).
    pub tab: ShopTab,
    /// (THE USER 2026-11-02: "Re-add Grave Gloveworts and Ghost Gloveworts ... make them appear where they usually would")
    /// The host's own lines on top of the tier's stock, at the host's price (Elden Ring: the merchant's own glovewort lines,
    /// the Twin Maiden Husks' after the bell bearings, read from the game's ShopLineupParam at run time). Listed after the
    /// tier's stock; a key the stock already sells is not listed twice.
    pub extra: Vec<crate::shop::Offer>,
    /// (SHOP-TIERS, THE USER 2026-11-07: "start them off with basic, limited selection") How far the merchant's own lineup
    /// has grown (crate::shop_growth): the buy list (the tier's stock and `extra`) is cut to it. `None`: the whole stock
    /// (a lineup that never grows, a host without merchants).
    pub growth: Option<crate::shop_growth::Growth>,
}

/// A trade the screen asked for, carried out in `apply_actions`.
#[derive(Clone, Debug, PartialEq)]
struct Trade {
    buy: bool,
    key: String,
    qty: u32,
    price: u32,
}

/// The shop's state in the HUD.
#[derive(Default)]
pub struct ShopState {
    view: Option<ShopScreen>,
    menu: Option<ShopMenu>,
    open: Option<ShopOpen>,
    trades: Vec<Trade>,
    /// The shop closed (its `out` ended) since the host last asked.
    closed: bool,
    /// What it did (for the host's log): trades, refusals of the HUD's own checks.
    pub log: Vec<String>,
}

impl ShopState {
    pub fn load(pack: &mut kh_ui::Pack, fonts: &kh_ui::Fonts) -> ShopState {
        // KH3's own Moogle shop movie when the pack has it (41_shop/Shop: the shop stream's pack merge), else the camp
        // menu's Items screen look
        let view = match MoogleShopView::load(pack, fonts.clone()) {
            Ok(v) => Some(ShopScreen::Moogle(Box::new(v))),
            Err(e) => {
                println!("hud: KH3's Moogle shop movie is not in the UI pack ({e}): the shop uses the camp menu's screen");
                ShopView::load(pack, fonts.clone()).map_err(|e| println!("hud: the KH shop is NOT available ({e})")).ok().map(|v| ShopScreen::Camp(Box::new(v)))
            }
        };
        ShopState { view, ..ShopState::default() }
    }

    pub fn is_open(&self) -> bool {
        self.view.as_ref().is_some_and(|v| v.is_open())
    }

    /// (THE USER 2026-11-03: "right stick scroll") the right stick's lines on the open screen's descriptions.
    pub(super) fn scroll_help(&mut self, lines: i32) {
        let movie = match self.view.as_mut().filter(|v| v.is_open()) {
            Some(ShopScreen::Moogle(v)) => &mut v.movie,
            Some(ShopScreen::Camp(v)) => &mut v.movie,
            None => return,
        };
        movie.scroll_fits(lines);
    }
}

/// (SHOP) The shop's screen: KH3's own Moogle shop, or the camp menu's Items screen look (no Shop movie in the pack).
pub enum ShopScreen {
    Moogle(Box<MoogleShopView>),
    Camp(Box<ShopView>),
}

impl ShopScreen {
    pub fn is_open(&self) -> bool {
        match self {
            ShopScreen::Moogle(v) => v.is_open(),
            ShopScreen::Camp(v) => v.is_open(),
        }
    }
    fn open(&mut self, menu: &mut ShopMenu) {
        match self {
            ShopScreen::Moogle(v) => v.open(menu),
            ShopScreen::Camp(v) => v.open(menu),
        }
    }
    fn close(&mut self) {
        match self {
            ShopScreen::Moogle(v) => v.close(),
            ShopScreen::Camp(v) => v.close(),
        }
    }
    fn sync(&mut self, menu: &mut ShopMenu) {
        match self {
            ShopScreen::Moogle(v) => v.sync(menu),
            ShopScreen::Camp(v) => v.sync(menu),
        }
    }
    fn tick(&mut self) -> bool {
        match self {
            ShopScreen::Moogle(v) => v.tick(),
            ShopScreen::Camp(v) => v.tick(),
        }
    }
    /// The screen (the camp menu's layer).
    pub fn movie(&self) -> &kh_ui::Movie {
        match self {
            ShopScreen::Moogle(v) => &v.movie,
            ShopScreen::Camp(v) => &v.movie,
        }
    }
    /// The camp-look screen's window (the Moogle shop's confirmation is part of its own movie).
    pub fn dialog_movie(&self) -> Option<&kh_ui::Movie> {
        match self {
            ShopScreen::Moogle(_) => None,
            ShopScreen::Camp(v) => v.dialog_movie(),
        }
    }
    /// KH3's own Moogle shop is up (not the fallback).
    pub fn is_moogle(&self) -> bool {
        matches!(self, ShopScreen::Moogle(_))
    }
}

/// The camp menu's category of an item (its row icon).
/// (P9b) The camp menu's category of a stock row by its key: a weapon row (`crate::weapons`: Donald's staves, Goofy's
/// shields; no item row) is KH3's Weapons category (`<icon(Weapon)>`), anything else its item class's.
fn category_of(key: &str) -> u8 {
    if crate::weapons::by_key(key).is_some() {
        return CAT_WEAPON;
    }
    category(crate::items::by_key(key).map_or(Class::Battle, |i| i.class))
}

fn category(class: Class) -> u8 {
    match class {
        Class::Battle | Class::Camp => CAT_ITEM,
        Class::Armour => CAT_ARMOUR,
        Class::Accessory => CAT_ACCESSORY,
        Class::Material => CAT_MATERIAL,
    }
}

impl Hud {
    /// OPENS THE KH SHOP. False: it cannot open now (no shop movie in the pack, another menu of the HUD is up).
    pub fn open_shop(&mut self, open: ShopOpen, sora: &kh_core::Sora) -> bool {
        if self.shop.view.is_none() || self.shop.is_open() || self.tuning.is_open() /* (THE USER 2026-11-02) */ || self.workshop.is_open() /* (weapons P6b) */ || self.flow.paused() || self.gameover.is_open() || self.savepoint.is_open() || self.saveload_open() || self.newgame_choice_open() || self.cutscene_pause.is_open() {
            return false;
        }
        let tab = open.tab;
        self.shop.open = Some(open);
        let data = self.shop_data(sora);
        let mut menu = ShopMenu::new(data, tab);
        if let Some(view) = self.shop.view.as_mut() {
            view.open(&mut menu);
        }
        self.sounds.push(kh_ui::UiSound::Play(kh_ui::menu::SE_MENU_OPEN));
        self.shop.menu = Some(menu);
        self.shop.closed = false;
        let o = self.shop.open.as_ref().expect("just set");
        let screen = if self.shop.view.as_ref().is_some_and(|v| v.is_moogle()) { "KH3's Moogle shop" } else { "the camp-menu look (no Shop movie in the pack)" };
        let grown = o.growth.as_ref().map_or_else(|| "the whole stock".to_string(), |g| format!("its lineup grown to {:.0} %", g.share * 100.0));
        (self.log)(&format!("shop: OPENS ({}, tier {}, straight on {:?}; {screen}; {grown}): {} munny", if o.title.is_empty() { "Moogle Shop" } else { &o.title }, o.tier, o.tab, sora.status().munny));
        true
    }

    /// The shop is up (opening, open or closing).
    pub fn shop_open(&self) -> bool {
        self.shop.is_open()
    }

    /// True once after the shop closed (its `out` ended): the host's talk goes on.
    pub fn take_shop_closed(&mut self) -> bool {
        std::mem::take(&mut self.shop.closed)
    }

    /// The host closes it from outside (a cutscene, a load, the merchant's talk was cut off).
    pub fn close_shop(&mut self) {
        if let Some(view) = self.shop.view.as_mut() {
            view.close();
        }
    }

    /// (tests) The screen's logic while the shop is up.
    pub fn shop_menu(&self) -> Option<&ShopMenu> {
        self.shop.menu.as_ref().filter(|_| self.shop.is_open())
    }

    /// The shop's movies (the screen, its window) for the camp menu's layers.
    pub(super) fn shop_movie(&self, name: &str) -> Option<Option<&kh_ui::Movie>> {
        let view = self.shop.view.as_ref().filter(|v| v.is_open())?;
        Some(match name {
            "CampMenu" => Some(view.movie()),
            "Dialog" => view.dialog_movie(),
            _ => None,
        })
    }

    /// THE ROWS FROM WHAT SORA HOLDS: the tier's stock with how many he has; everything sellable in his stock.
    fn shop_data(&self, sora: &kh_core::Sora) -> ShopData {
        let rules = crate::shop::rules();
        let tier = self.shop.open.as_ref().map_or(1, |o| o.tier);
        let pouch = Self::pouch(sora);
        let held = |key: &str, item_id: u32| -> u32 {
            match crate::items::battle_kind(item_id) {
                Some(kind) => pouch.stock_of(kind) as u32,
                None => self.inventory.equipment.iter().filter(|e| e.0 == key).map(|e| e.1).sum(),
            }
        };
        let extra = self.shop.open.as_ref().map(|o| o.extra.clone()).unwrap_or_default();
        let mut offers = rules.stock_for(tier);
        for o in extra {
            if !offers.iter().any(|s| s.key == o.key) {
                offers.push(o);
            }
        }
        // (SHOP-TIERS) only what the merchant's own lineup has grown to (the cheapest first; materials and the host's own lines always)
        if let Some(growth) = self.shop.open.as_ref().and_then(|o| o.growth.as_ref()) {
            offers = crate::shop_growth::rules().grown_stock(offers, growth, rules);
        }
        let mut buy: Vec<ShopRow> = offers
            .into_iter()
            .map(|o| ShopRow { owned: held(&o.key, o.item_id), max: o.max.min(STOCK_MAX), help: crate::items::own_help(&o.key) /* (THE USER 2026-11-02) a composed row's own text */, category: category_of(&o.key), key: o.key, name: o.name, price: o.price })
            .collect();
        // (P9b) KH3's shop order (TresShopData: items, weapons, armour, accessories, materials), each category in the order it came
        buy.sort_by_key(|r| r.category);
        // the sell list: the battle items of the pouch's stock, then the camp menu's stock, in the item table's order
        let mut sell = Vec::new();
        for item in crate::items::table() {
            let owned = held(&item.key, item.item_id);
            if owned == 0 {
                continue;
            }
            let Some(price) = rules.sell_price(&item.key, tier) else { continue };
            sell.push(ShopRow { key: item.key.clone(), name: item.name.clone(), price, owned, max: item.max.min(STOCK_MAX), category: category(item.class), help: crate::items::own_help(&item.key) });
        }
        let title = self.shop.open.as_ref().map(|o| o.title.clone()).unwrap_or_default();
        ShopData { title, munny: sora.status().munny as i64, buy, sell, qty_max: rules.qty_max }
    }

    /// (SHOP) The shop's part of a frame: the pad drives it, its movies run. True while it has the frame.
    pub(super) fn shop_frame(&mut self, f: &Frame, nav: &[(u8, bool)], words: &[String]) -> bool {
        use kh_core::PadButton as B;
        if !self.shop.is_open() {
            return false;
        }
        // the truth of this frame (after the last frame's trades were applied)
        let data = self.shop_data(f.sora);
        let mut keys: Vec<(CampKey, bool)> = nav.iter().map(|(dir, repeat)| ([CampKey::Up, CampKey::Down, CampKey::Left, CampKey::Right][*dir as usize & 3], *repeat)).collect();
        for (button, key) in [(B::FaceBottom, CampKey::Decide), (B::FaceRight, CampKey::Cancel), (B::LeftShoulder, CampKey::L1), (B::RightShoulder, CampKey::R1), (B::LeftTrigger, CampKey::L2), (B::RightTrigger, CampKey::R2)] {
            if f.pressed.has(button) {
                keys.push((key, false));
            }
        }
        for word in words {
            let key = match word.as_str() {
                "menu up" => CampKey::Up,
                "menu down" => CampKey::Down,
                "menu left" => CampKey::Left,
                "menu right" => CampKey::Right,
                "menu decide" => CampKey::Decide,
                "menu cancel" => CampKey::Cancel,
                "menu l1" => CampKey::L1,
                "menu r1" => CampKey::R1,
                "menu l2" => CampKey::L2,
                "menu r2" => CampKey::R2,
                _ => continue,
            };
            keys.push((key, false));
        }
        let (Some(menu), Some(view)) = (self.shop.menu.as_mut(), self.shop.view.as_mut()) else { return false };
        if self.shop.trades.is_empty() {
            menu.set_data(data);
        }
        for (key, repeat) in keys {
            menu.key(key, repeat, &mut self.sounds);
        }
        let mut close = false;
        for event in menu.take_events() {
            match event {
                ShopEvent::Buy { key, qty, price } => self.shop.trades.push(Trade { buy: true, key, qty, price }),
                ShopEvent::Sell { key, qty, price } => self.shop.trades.push(Trade { buy: false, key, qty, price }),
                ShopEvent::Closed => close = true,
            }
        }
        if close {
            view.close();
            (self.log)("shop: B on the list: the shop closes");
        }
        for _ in 0..f.ticks.max(1) {
            view.sync(menu);
            if view.tick() {
                self.shop.closed = true;
                self.shop.menu = None;
                self.shop.open = None;
                self.status_actions.push(super::StatusAction::Unpause); // (the pouch's refill, as after the camp menu)
                (self.log)("shop: closed");
                break;
            }
        }
        true
    }

    /// (SHOP) THE TRADES of the last frame, on Sora's munny and the stock (called first by `apply_actions`).
    pub(super) fn shop_apply(&mut self, sora: &mut kh_core::Sora) {
        // (SHOP-3) a host's take-backs (`crate::items::take`): out of the stock as far as he holds them
        for (item_id, count) in crate::items::take_requests() {
            let Some(item) = crate::items::by_id(item_id) else { continue };
            let taken = match crate::items::battle_kind(item_id) {
                Some(kind) => {
                    kh_core::command::customize::ensure(sora);
                    kh_core::command::customize::discard(sora, kind, count.min(99) as u8) as u32
                }
                None => {
                    let held = self.inventory.equipment.iter().find(|e| e.0 == item.key).map_or(0, |e| e.1);
                    if let Some(e) = self.inventory.equipment.iter_mut().find(|e| e.0 == item.key) {
                        e.1 = e.1.saturating_sub(count);
                    }
                    self.inventory.equipment.retain(|e| e.1 > 0);
                    held.min(count)
                }
            };
            (self.log)(&format!("item: {} x{taken} taken back out of the stock (a host's repair; asked {count})", item.name));
        }
        if self.shop.trades.is_empty() {
            return;
        }
        kh_core::command::customize::ensure(sora);
        for t in std::mem::take(&mut self.shop.trades) {
            // (P9b) a friend's weapon of KH3's Moogle stock is a weapon row, not an item row: bought straight into the stock (no popup)
            let weapon = crate::weapons::by_key(&t.key).filter(|w| !w.is_keyblade());
            let Some((item_id, name, item_max)) = crate::items::by_key(&t.key).map(|i| (i.item_id, i.name.clone(), i.max)).or_else(|| weapon.map(|w| (w.item_id, w.name.clone(), w.max))) else { continue };
            let max = item_max.min(STOCK_MAX);
            let kind = crate::items::battle_kind(item_id);
            let owned = match kind {
                Some(k) => sora.magic().pouch.stock_of(k) as u32,
                None => self.inventory.equipment.iter().filter(|e| e.0 == t.key).map(|e| e.1).sum(),
            };
            let munny = sora.status().munny as i64;
            let done = if t.buy { crate::shop::buy(munny, owned, max, t.price, t.qty) } else { crate::shop::sell(munny, owned, t.price, t.qty) };
            let line = match done {
                Ok((munny_after, owned_after)) => {
                    sora.status_mut().munny = munny_after.clamp(0, i32::MAX as i64) as i32;
                    match (kind, t.buy) {
                        (Some(k), true) => sora.magic_mut().pouch.stock_add(k, t.qty.min(99) as u8),
                        (Some(k), false) => drop(kh_core::command::customize::discard(sora, k, t.qty.min(99) as u8)),
                        (None, true) => self.inventory.add(&t.key, t.qty),
                        (None, false) => {
                            if let Some(e) = self.inventory.equipment.iter_mut().find(|e| e.0 == t.key) {
                                e.1 = e.1.saturating_sub(t.qty);
                            }
                            self.inventory.equipment.retain(|e| e.1 > 0);
                        }
                    }
                    format!("shop: {} {} x{} for {} munny: munny {munny} -> {munny_after}, held {owned} -> {owned_after}", if t.buy { "BOUGHT" } else { "SOLD" }, name, t.qty, t.price as i64 * t.qty as i64)
                }
                Err(why) => format!("shop: {} {} x{} REFUSED ({why:?}: munny {munny}, held {owned} of {max})", if t.buy { "buying" } else { "selling" }, name, t.qty),
            };
            (self.log)(&line);
            self.shop.log.push(line);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (SHOP) The HUD's shop on a real Sora and the real UI pack: it opens with the tier's stock and his munny, a
    /// purchase moves munny and stock (into the pouch's stock for a battle item), a sale back; the stock's maximum
    /// and the munny are checked again when the trade is carried out; B closes it. Skipped without the UI pack.
    /// (SHOP) THE MOOGLE SHOP IN THE HUD (the shop stream's pack with KH3's Shop movie; skipped without it): the talk's
    /// "Sell" opens it STRAIGHT on the sell list (no hub), in KH3's way (the quantity on the row); B on the list closes
    /// it (the merchant's talk goes on).
    #[test]
    fn shop_the_moogle_shop_opens_straight_on_the_tab_asked() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/party/SHOP/pack/ui");
        if !dir.join("movies/Shop.json").exists() {
            eprintln!("SKIPPED: no Shop movie in the shop pack");
            return;
        }
        let Ok((mut hud, _)) = Hud::load(&dir) else { panic!("the shop pack loads") };
        assert!(hud.shop.view.as_ref().is_some_and(|v| v.is_moogle()), "KH3's own Moogle shop");
        let mut sora = crate::savepoint::tests::sora99();
        sora.status_mut().munny = 500;
        assert!(hud.open_shop(ShopOpen { title: "Twin Maiden Husks".into(), tier: 2, tab: ShopTab::Sell, extra: Vec::new(), growth: None }, &sora));
        let menu = hud.shop.menu.as_ref().unwrap();
        assert_eq!((menu.tab, menu.inline_qty), (ShopTab::Sell, true), "straight on SELL, KH3's way");
        assert!(hud.movie("CampMenu").is_some_and(|m| m.exists("menuList")), "the Shop movie on the screen");
        if let Some(menu) = hud.shop.menu.as_mut() {
            menu.key(CampKey::Cancel, false, &mut Vec::new());
        }
        let mut closed = false;
        for _ in 0..200 {
            let snapshot = kh_core::UiSnapshot::default();
            let mut f = Frame { snapshot: &snapshot, sora: &sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 0, targets: &super::super::NoTargets, to_view: |v| v, view: None, words: &[], dt: 1.0 / 60.0, ticks: 1, pressed: Default::default(), nav: Vec::new() };
            hud.frame(&mut f);
            if hud.take_shop_closed() {
                closed = true;
                break;
            }
        }
        assert!(closed && !hud.paused(), "B on the list: the shop is gone");
    }

    /// (THE USER 2026-11-02: "Re-add Grave Gloveworts and Ghost Gloveworts ... make them appear where they usually would")
    /// THE HOST'S OWN LINES: listed after the tier's stock at the host's price, with the composed row's own text; a key the
    /// stock already sells is not listed twice; a purchase of one goes into the camp menu's stock like any other.
    #[test]
    fn shop_sells_the_host_s_own_lines_after_the_stock() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/ui");
        let Ok((mut hud, _)) = Hud::load(&dir) else {
            eprintln!("SKIPPED: no UI pack");
            return;
        };
        let mut sora = crate::savepoint::tests::sora99();
        kh_core::command::customize::ensure(&mut sora);
        sora.status_mut().munny = 5000;
        let offer = |key: &str, price: u32| {
            let item = crate::items::by_key(key).unwrap();
            crate::shop::Offer { key: key.into(), item_id: item.item_id, name: item.name.clone(), price, max: item.max }
        };
        let stocked = crate::shop::rules().stock_for(2);
        let extra = vec![offer("MAT_GLOVE_GRAVE_01", 800), offer("MAT_GLOVE_GHOST_02", 1800), offer(&stocked[0].key, 1)];
        assert!(hud.open_shop(ShopOpen { title: "Twin Maiden Husks".into(), tier: 2, tab: ShopTab::Buy, extra, growth: None }, &sora));
        let data = hud.shop_data(&sora);
        let keys: Vec<&str> = data.buy.iter().map(|r| r.key.as_str()).collect();
        assert_eq!(&keys[..stocked.len()], stocked.iter().map(|o| o.key.as_str()).collect::<Vec<_>>().as_slice(), "the tier's stock first, its own price");
        assert_eq!(&keys[stocked.len()..], ["MAT_GLOVE_GRAVE_01", "MAT_GLOVE_GHOST_02"], "then the host's lines (not a stocked key twice)");
        let row = data.buy.iter().find(|r| r.key == "MAT_GLOVE_GRAVE_01").unwrap();
        assert_eq!((row.price, row.category, row.help.contains("Upgrade material")), (800, CAT_MATERIAL, true));
        hud.shop.trades.push(Trade { buy: true, key: "MAT_GLOVE_GRAVE_01".into(), qty: 2, price: 800 });
        hud.apply_actions(&mut sora);
        assert_eq!((sora.status().munny, hud.inventory.equipment.iter().find(|e| e.0 == "MAT_GLOVE_GRAVE_01").map(|e| e.1)), (3400, Some(2)));
    }

    /// (P9b) KH3'S MOOGLE STOCK AT A MERCHANT, 1:1 WITH KH3'S SHOP: Donald's staves and Goofy's shields are KH3's Weapons
    /// category (`<icon(Weapon)>`), the list in KH3's order (items, weapons, armour, accessories, materials; each in the order
    /// it came); buying a staff puts it straight into the stock, no OBTAINED popup.
    #[test]
    fn p9b_weapons_are_kh3_s_weapons_category_and_bought_without_a_popup() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/ui");
        let Ok((mut hud, _)) = Hud::load(&dir) else {
            eprintln!("SKIPPED: no UI pack");
            return;
        };
        hud.obtained_on = true;
        let mut sora = crate::savepoint::tests::sora99();
        kh_core::command::customize::ensure(&mut sora);
        sora.status_mut().munny = 10_000;
        let ws = crate::weapons::Workshop { moogle_level: 6, ..crate::weapons::Workshop::default() };
        let rules = crate::shop::rules();
        let mut extra = vec![crate::shop::Offer { key: "MAT_GLOVE_GRAVE_01".into(), item_id: crate::items::by_key("MAT_GLOVE_GRAVE_01").unwrap().item_id, name: "Grave Glovewort".into(), price: 800, max: 99 }];
        extra.extend(crate::shop::kh3_stock(rules, crate::shop::weapon_shops(), 600001110, &ws, &|_| 0));
        assert!(hud.open_shop(ShopOpen { title: "Twin Maiden Husks".into(), tier: 3, tab: ShopTab::Buy, extra, growth: None }, &sora));
        let data = hud.shop_data(&sora);
        let cats: Vec<u8> = data.buy.iter().map(|r| r.category).collect();
        assert!(cats.windows(2).all(|w| w[0] <= w[1]), "KH3's order by category: {cats:?}");
        let weapons: Vec<&str> = data.buy.iter().filter(|r| r.category == CAT_WEAPON).map(|r| r.key.as_str()).collect();
        assert_eq!(weapons, ["WEP_DONALD_02", "WEP_DONALD_04", "WEP_DONALD_06", "WEP_DONALD_08", "WEP_GOOFY_02", "WEP_GOOFY_04", "WEP_GOOFY_06"]);
        assert_eq!(data.buy.iter().find(|r| r.key == "MAT_GLOVE_GRAVE_01").map(|r| r.category), Some(CAT_MATERIAL));
        // a purchase: munny, the stock, no popup
        let price = data.buy.iter().find(|r| r.key == "WEP_DONALD_02").unwrap().price;
        hud.shop.trades.push(Trade { buy: true, key: "WEP_DONALD_02".into(), qty: 1, price });
        hud.apply_actions(&mut sora);
        assert_eq!((sora.status().munny, hud.inventory.equipment.iter().find(|e| e.0 == "WEP_DONALD_02").map(|e| e.1)), (10_000 - price as i32, Some(1)));
        assert_eq!(hud.obtained_waiting(), 0, "a purchase shows no OBTAINED popup");
    }

    #[test]
    fn shop_trades_through_the_hud() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/ui");
        let Ok((mut hud, _)) = Hud::load(&dir) else {
            eprintln!("SKIPPED: no UI pack");
            return;
        };
        let mut sora = crate::savepoint::tests::sora99();
        kh_core::command::customize::ensure(&mut sora);
        sora.status_mut().munny = 2000;
        hud.inventory.add("PRT_ITEM01", 1);
        hud.inventory.add("WEP_DONALD_03", 1);
        assert!(hud.open_shop(ShopOpen { title: "Merchant".into(), tier: 1, tab: ShopTab::Buy, extra: Vec::new(), growth: None }, &sora));
        assert!(hud.paused() && hud.shop_open());
        let data = hud.shop_data(&sora);
        assert_eq!(data.buy.iter().map(|r| r.key.as_str()).collect::<Vec<_>>(), crate::shop::rules().stock_for(1).iter().map(|o| o.key.as_str()).collect::<Vec<_>>());
        assert!(data.sell.iter().any(|r| r.key == "PRT_ITEM01"), "armour sells");
        assert!(!data.sell.iter().any(|r| r.key.starts_with("WEP_")), "a friend's weapon never sells");
        // a purchase of 3 Potions
        let price = data.buy.iter().find(|r| r.key == "ITEM_POTION").unwrap().price;
        hud.shop.trades.push(Trade { buy: true, key: "ITEM_POTION".into(), qty: 3, price });
        hud.apply_actions(&mut sora);
        assert_eq!((sora.status().munny, sora.magic().pouch.stock_of(kh_core::status::ItemKind::Potion)), (2000 - 3 * price as i32, 3));
        // more than the munny reaches: refused, nothing moves
        hud.shop.trades.push(Trade { buy: true, key: "ITEM_TENT".into(), qty: 50, price: 1200 });
        hud.apply_actions(&mut sora);
        assert_eq!(sora.magic().pouch.stock_of(kh_core::status::ItemKind::Tent), 0);
        assert!(hud.shop.log.last().unwrap().contains("REFUSED"));
        // a sale of the armour
        let before = sora.status().munny;
        hud.shop.trades.push(Trade { buy: false, key: "PRT_ITEM01".into(), qty: 1, price: 500 });
        hud.apply_actions(&mut sora);
        assert_eq!(sora.status().munny, before + 500);
        assert!(!hud.inventory.equipment.iter().any(|e| e.0 == "PRT_ITEM01"));
        // the shop's rows follow
        let data = hud.shop_data(&sora);
        assert_eq!(data.buy.iter().find(|r| r.key == "ITEM_POTION").unwrap().owned, 3);
        assert!(data.sell.iter().any(|r| r.key == "ITEM_POTION" && r.owned == 3));
        // B closes it
        if let Some(menu) = hud.shop.menu.as_mut() {
            menu.key(CampKey::Cancel, false, &mut Vec::new());
        }
        let mut closed = false;
        for _ in 0..200 {
            let snapshot = kh_core::UiSnapshot::default();
            let mut f = Frame { snapshot: &snapshot, sora: &sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 0, targets: &super::super::NoTargets, to_view: |v| v, view: None, words: &[], dt: 1.0 / 60.0, ticks: 1, pressed: Default::default(), nav: Vec::new() };
            hud.frame(&mut f);
            if hud.take_shop_closed() {
                closed = true;
                break;
            }
        }
        assert!(closed && !hud.shop_open() && !hud.paused());
    }

    /// One frame of the HUD with nothing pressed.
    fn idle_frame(hud: &mut Hud, sora: &kh_core::Sora) {
        let snapshot = kh_core::UiSnapshot::default();
        let mut f = Frame { snapshot: &snapshot, sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 0, targets: &super::super::NoTargets, to_view: |v| v, view: None, words: &[], dt: 1.0 / 60.0, ticks: 1, pressed: Default::default(), nav: Vec::new() };
        hud.frame(&mut f);
    }

    /// (SHOP-TIERS, THE USER 2026-11-07: "start them off with basic, limited selection") REAL MERCHANTS, THE STOCK BY THEIR
    /// OWN LINEUP'S GROWTH (the sheet's generated lineups, the flags as a save would hold them; the same functions kh-er runs
    /// on the game's live rows): the Twin Maiden Husks with no / one / several / every bell bearing, Corhyn before and after
    /// his prayerbooks, Kalé (a lineup that never grows: the whole stock). Each step only adds; materials always stay.
    /// Pictures (KH3's Moogle shop movie, skipped without the shop pack): build/sora_tmp/shop_tiers/shots/.
    #[test]
    fn shop_tiers_real_merchants_grow_with_their_lineups() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/party/SHOP/pack/ui");
        if !dir.join("movies/Shop.json").exists() {
            eprintln!("SKIPPED: no Shop movie in the shop pack");
            return;
        }
        let Ok((mut hud, _)) = Hud::load(&dir) else { panic!("the shop pack loads") };
        let fonts = kh_ui::Pack::open(&dir).ok().and_then(|mut p| kh_ui::Fonts::load(&mut p).ok()).expect("fonts");
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/shop_tiers/shots");
        std::fs::create_dir_all(&out).ok();
        let mut sora = crate::savepoint::tests::sora99();
        kh_core::command::customize::ensure(&mut sora);
        sora.status_mut().munny = 30_000;
        let tiers = crate::shop_growth::rules();
        let shop = crate::shop::rules();
        let ws = crate::weapons::Workshop { moogle_level: 4, ..crate::weapons::Workshop::default() };
        // opens the shop for a merchant (the host's way: the tier's stock + KH3's Moogle stock, cut by the growth), lets it
        // settle, takes its buy list and a picture (None: no picture), closes it
        let open = |hud: &mut Hud, title: &str, talk: i32, growth: Option<crate::shop_growth::Growth>, shot: Option<&str>| -> Vec<String> {
            let extra = crate::shop::kh3_stock(shop, crate::shop::weapon_shops(), talk, &ws, &|_| 0);
            assert!(hud.open_shop(ShopOpen { title: title.into(), tier: 3, tab: ShopTab::Buy, extra, growth }, &sora));
            for _ in 0..90 {
                idle_frame(hud, &sora);
            }
            let keys: Vec<String> = hud.shop_data(&sora).buy.iter().map(|r| r.key.clone()).collect();
            if let Some(shot) = shot {
                let mut canvas = kh_ui::Canvas::new(1920, 1080);
                if let Some(m) = hud.movie("CampMenu") {
                    kh_ui::Renderer::new(fonts.clone()).draw(&mut canvas, m, &kh_ui::geom::Matrix::new(1.0, 0.0, 0.0, 1.0, 0.0, 0.0));
                }
                let mut rgba = Vec::new();
                canvas.to_rgba8_over(|_, _| [0.12, 0.13, 0.16], &mut rgba);
                kh_ui::def::write_png(&out.join(format!("{shot}.png")), 1920, 1080, &rgba).unwrap();
                println!("{shot}: {} offers: {}", keys.len(), keys.join(", "));
            }
            hud.close_shop();
            for _ in 0..200 {
                idle_frame(hud, &sora);
                if !hud.shop_open() {
                    break;
                }
            }
            assert!(!hud.shop_open(), "{title}: closed");
            hud.take_shop_closed();
            keys
        };
        let grows = |a: &[String], b: &[String]| a.iter().all(|k| b.contains(k)) && b.len() > a.len();
        let materials = |keys: &[String]| keys.iter().filter(|k| crate::items::by_key(k).is_some_and(|i| i.class == crate::items::Class::Material)).count();
        // the Twin Maiden Husks: their own range; the bell bearings' flags as the sheet lists them
        const HUSKS: (i32, i32) = (101_800, 101_897);
        let bells: Vec<u32> = tiers.lineups[&HUSKS].flags.iter().map(|f| f.0).collect();
        let husks = |on: &[u32]| tiers.sheet_share(HUSKS, &|f| on.contains(&f));
        let none = open(&mut hud, "Twin Maiden Husks", 600_001_110, husks(&[]), Some("01_husks_no_bell_bearing"));
        let one = open(&mut hud, "Twin Maiden Husks", 600_001_110, husks(&[11_109_752]), Some("02_husks_one_bell_bearing"));
        let several = open(&mut hud, "Twin Maiden Husks", 600_001_110, husks(&bells[..8]), Some("03_husks_eight_bell_bearings"));
        let all = open(&mut hud, "Twin Maiden Husks", 600_001_110, husks(&bells), Some("04_husks_every_bell_bearing"));
        let whole = open(&mut hud, "Twin Maiden Husks", 600_001_110, None, None);
        assert!(grows(&none, &one) && grows(&one, &several) && grows(&several, &all), "every step adds: {} -> {} -> {} -> {}", none.len(), one.len(), several.len(), all.len());
        assert_eq!(all, whole, "every bell bearing: the whole stock");
        assert_eq!(materials(&none), materials(&whole), "the materials are always sold");
        assert!(none.len() < whole.len() / 2, "a basic, limited start: {} of {}", none.len(), whole.len());
        // a quest vendor: Corhyn before and after his prayerbooks (his talk script sets 11109874 .. 11109884)
        const CORHYN: (i32, i32) = (100_350, 100_399);
        let books: Vec<u32> = tiers.lineups[&CORHYN].flags.iter().map(|f| f.0).filter(|f| (11_109_874..=11_109_884).contains(f)).collect();
        let before = open(&mut hud, "Brother Corhyn", 351_001_110, tiers.sheet_share(CORHYN, &|_| false), Some("05_corhyn_before_prayerbooks"));
        let after = open(&mut hud, "Brother Corhyn", 351_001_110, tiers.sheet_share(CORHYN, &|f| books.contains(&f)), Some("06_corhyn_after_prayerbooks"));
        assert!(grows(&before, &after), "Corhyn: {} -> {}", before.len(), after.len());
        // a lineup that never grows: Kalé sells the whole stock from the start
        const KALE: (i32, i32) = (100_500, 100_524);
        assert_eq!(tiers.sheet_share(KALE, &|_| false), None);
        let kale = open(&mut hud, "Merchant Kalé", 800_006_000, tiers.sheet_share(KALE, &|_| false), Some("07_kale_never_grows"));
        let kale_whole = open(&mut hud, "Merchant Kalé", 800_006_000, None, None);
        assert_eq!(kale, kale_whole, "Kalé: the whole stock");
    }
}
