//! (SHOP) The KH shop screen on the UI pack: it opens onto the Items screen look with KH3's shop texts, the BUY / SELL
//! tabs, the rows with their prices, the munny, the item pane; the quantity window and the confirmation; B closes it.
//! Pictures: build/sora_tmp/party/SHOP/shots/. Skipped without the UI pack.

use kh_ui::camp::CampKey;
use kh_ui::camp_equip::{CAT_ARMOUR, CAT_ITEM};
use kh_ui::shop::{Layer, ShopData, ShopEvent, ShopMenu, ShopRow, ShopTab, ShopView};

fn load() -> Option<(ShopView, kh_ui::Fonts)> {
    let Ok(mut pack) = kh_ui::Pack::open_default() else {
        eprintln!("SKIPPED: no build/sora_pack/ui");
        return None;
    };
    let fonts = kh_ui::Fonts::load(&mut pack).unwrap();
    Some((ShopView::load(&mut pack, fonts.clone()).unwrap(), fonts))
}

fn row(key: &str, name: &str, price: u32, owned: u32, category: u8) -> ShopRow {
    ShopRow { key: key.into(), name: name.into(), price, owned, max: 99, category, help: String::new() }
}

/// What a Limgrave merchant (shop tier 1 of the shipped sheet) shows to a Sora with 2,350 munny.
fn data() -> ShopData {
    ShopData {
        title: "Merchant Kalé".into(),
        munny: 2350,
        buy: vec![row("ITEM_POTION", "Potion", 400, 3, CAT_ITEM), row("ITEM_ETHER", "Ether", 800, 0, CAT_ITEM), row("ITEM_ALLCURE", "Panacea", 250, 1, CAT_ITEM), row("ITEM_TENT", "Tent", 1200, 0, CAT_ITEM)],
        sell: vec![row("ITEM_POTION", "Potion", 100, 3, CAT_ITEM), row("ITEM_ALLCURE", "Panacea", 60, 1, CAT_ITEM), row("PRT_ITEM01", "Hero's Belt", 750, 1, CAT_ARMOUR)],
        qty_max: 99,
    }
}

fn t(view: &ShopView, path: &str) -> String {
    view.movie.text(path).unwrap_or_default()
}

fn frames(view: &mut ShopView, menu: &mut ShopMenu, n: usize) {
    for _ in 0..n {
        view.sync(menu);
        view.tick();
    }
}

fn shot(view: &ShopView, fonts: &kh_ui::Fonts, name: &str) {
    let mut canvas = kh_ui::Canvas::new(1920, 1080);
    let root = kh_ui::geom::Matrix::new(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
    let mut renderer = kh_ui::Renderer::new(fonts.clone());
    renderer.draw(&mut canvas, &view.movie, &root);
    if let Some(dialog) = view.dialog_movie() {
        renderer.draw(&mut canvas, dialog, &root);
    }
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|_, _| [0.12, 0.13, 0.16], &mut rgba);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/party/SHOP/shots");
    std::fs::create_dir_all(&dir).ok();
    kh_ui::def::write_png(&dir.join(format!("{name}.png")), 1920, 1080, &rgba).unwrap();
}

#[test]
fn shop_screen_buy_quantity_confirm_sell_and_close() {
    let Some((mut view, fonts)) = load() else { return };
    let mut menu = ShopMenu::new(data(), ShopTab::Buy);
    view.open(&mut menu);
    frames(&mut view, &mut menu, 60);
    assert!(view.is_open());
    // KH3's own texts and the rows

    assert_eq!(t(&view, "backyard.stageCategory.category.categoryName.txName"), "SHOP");
    assert_eq!(t(&view, "backyard.backyardAnm.backyardItem.backyardListTitle.backyardListTitleCnt"), "Price");
    assert_eq!(t(&view, "backyard.backyardAnm.backyardItem.backyardList.container.i0.backyardItemName"), "Potion");
    assert_eq!(t(&view, "backyard.backyardAnm.backyardItem.backyardList.container.i0.itemCnt"), "400");
    assert_eq!(t(&view, "stageFrame.stageObjR.munny.munnyNum.textField"), "2350");
    assert!(t(&view, "backyard.backyardAnm.itemStatusA1.txHelp.textField").contains("Owned"), "the pane says how many he holds");
    shot(&view, &fonts, "01_buy_list");
    // the quantity window, then the confirmation
    let mut s = Vec::new();
    menu.key(CampKey::Decide, false, &mut s);
    menu.key(CampKey::Right, false, &mut s);
    assert_eq!((menu.layer, menu.qty), (Layer::Quantity, 2));
    frames(&mut view, &mut menu, 30);
    assert!(view.dialog_movie().is_some(), "the quantity window is up");
    assert!(view.dialog.text("dialogText.textField").unwrap_or_default().contains("x 2"));
    shot(&view, &fonts, "02_quantity");
    menu.key(CampKey::Decide, false, &mut s);
    frames(&mut view, &mut menu, 30);
    assert!(view.dialog.text("dialogText.textField").unwrap_or_default().contains("Complete this purchase?"));
    shot(&view, &fonts, "03_confirm");
    menu.key(CampKey::Decide, false, &mut s);
    assert_eq!(menu.take_events(), vec![ShopEvent::Buy { key: "ITEM_POTION".into(), qty: 2, price: 400 }]);
    frames(&mut view, &mut menu, 30);
    assert!(view.dialog_movie().is_none(), "the window went");
    assert_eq!(t(&view, "stageFrame.stageObjR.munny.munnyNum.textField"), "1550");
    shot(&view, &fonts, "04_bought");
    // the sell tab
    menu.key(CampKey::R2, false, &mut s);
    frames(&mut view, &mut menu, 30);
    assert_eq!(t(&view, "backyard.backyardAnm.backyardItem.backyardList.container.i2.backyardItemName"), "Hero's Belt");
    shot(&view, &fonts, "05_sell_list");
    // B closes the shop
    menu.key(CampKey::Cancel, false, &mut s);
    assert_eq!(menu.take_events(), vec![ShopEvent::Closed]);
    view.close();
    let mut gone = false;
    for _ in 0..120 {
        if view.tick() {
            gone = true;
            break;
        }
    }
    assert!(gone && !view.is_open(), "the shop's out ended");
}

/// (SHOP, development aid) the visible text fields that still show their authored placeholder.
#[test]
#[ignore]
fn shop_debug_placeholders() {
    let Some((mut view, _)) = load() else { return };
    let mut menu = ShopMenu::new(data(), ShopTab::Buy);
    view.open(&mut menu);
    frames(&mut view, &mut menu, 60);
    fn walk(s: &kh_ui::player::Sprite, path: &str, shown: bool) {
        for node in s.children.iter().chain(&s.extra) {
            let p = if path.is_empty() { node.label() } else { format!("{path}.{}", node.label()) };
            let vis = shown && node.is_visible() && node.cx.mul[3] > 0.01;
            match &node.kind {
                kh_ui::player::Kind::Sprite(child) => walk(child, &p, vis),
                kh_ui::player::Kind::Text(t) if vis => println!("{p} = {:?}", t.text.clone().unwrap_or_else(|| format!("[authored] {}", t.def.initial))),
                _ => {}
            }
        }
    }
    walk(&view.movie.root, "", true);
}

/// (SHOP, development aid) KH3's Moogle shop movie from the shop stream's pack copy: its parts in, a picture, the tree.
#[test]
#[ignore]
fn shop_debug_moogle_movie() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/party/SHOP/pack/ui");
    let Ok(mut pack) = kh_ui::Pack::open(&dir) else { return };
    let fonts = kh_ui::Fonts::load(&mut pack).unwrap();
    let def = pack.movie_linked("Shop", &["MainLibrary", "ItemListLibrary", "ItemStatusLibrary", "SystemLibrary", "MarkLibrary"]).unwrap();
    let mut movie = kh_ui::Movie::new(def);
    let args: Vec<String> = std::env::var("SHOP_PLAY").unwrap_or_default().split(';').filter(|s| !s.is_empty()).map(str::to_string).collect();
    movie.goto_and_play("", "in");
    for _ in 0..10 {
        movie.tick();
    }
    for a in &args {
        if let Some((p, l)) = a.split_once('=') {
            movie.goto_and_play(p, l);
        }
    }
    for _ in 0..60 {
        movie.tick();
    }
    fn walk(s: &kh_ui::player::Sprite, path: &str, shown: bool, depth: usize) {
        for node in s.children.iter().chain(&s.extra) {
            let p = if path.is_empty() { node.label() } else { format!("{path}.{}", node.label()) };
            let vis = shown && node.is_visible() && node.cx.mul[3] > 0.01;
            match &node.kind {
                kh_ui::player::Kind::Sprite(child) => {
                    if vis && depth < 7 {
                        println!("S {p} [{:?}] {:?}", child.def.class, child.def.labels.iter().map(|l| l.0.clone()).collect::<Vec<_>>().join(","));
                    }
                    walk(child, &p, vis, depth + 1)
                }
                kh_ui::player::Kind::Text(t) if vis => println!("T {p} = {:?}", t.text.clone().unwrap_or_else(|| format!("[authored] {}", t.def.initial))),
                _ => {}
            }
        }
    }
    walk(&movie.root, "", true, 0);
    let mut canvas = kh_ui::Canvas::new(1920, 1080);
    kh_ui::Renderer::new(fonts).draw(&mut canvas, &movie, &kh_ui::geom::Matrix::new(1.0, 0.0, 0.0, 1.0, 0.0, 0.0));
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|_, _| [0.12, 0.13, 0.16], &mut rgba);
    let out = dir.join("../../shots");
    std::fs::create_dir_all(&out).ok();
    kh_ui::def::write_png(&out.join(format!("moogle_{}.png", std::env::var("SHOP_NAME").unwrap_or_else(|_| "explore".into()))), 1920, 1080, &rgba).unwrap();
}

/// (SHOP) KH3'S OWN MOOGLE SHOP (movie 41_shop/Shop from the shop stream's pack, build/sora_tmp/party/SHOP/pack/ui;
/// skipped without it): it opens STRAIGHT on the list the talk asked for, the quantity is changed on the row (left /
/// right), A brings KH3's confirmation (MUNNY before -> after, Yes / No), Yes trades, B goes back one level at a time,
/// LT / RT switch to the sell list (the stage's sell tint), B on the list closes the shop.
#[test]
fn shop_moogle_screen_buy_on_the_row_confirm_sell_and_close() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/party/SHOP/pack/ui");
    let Ok(mut pack) = kh_ui::Pack::open(&dir) else { return };
    if !dir.join("movies/Shop.json").exists() {
        eprintln!("SKIPPED: no Shop movie in the shop pack");
        return;
    }
    let fonts = kh_ui::Fonts::load(&mut pack).unwrap();
    let mut view = kh_ui::shop_moogle::MoogleShopView::load(&mut pack, fonts.clone()).unwrap();
    let shot = |view: &kh_ui::shop_moogle::MoogleShopView, name: &str| {
        let mut canvas = kh_ui::Canvas::new(1920, 1080);
        kh_ui::Renderer::new(fonts.clone()).draw(&mut canvas, &view.movie, &kh_ui::geom::Matrix::new(1.0, 0.0, 0.0, 1.0, 0.0, 0.0));
        let mut rgba = Vec::new();
        canvas.to_rgba8_over(|_, _| [0.12, 0.13, 0.16], &mut rgba);
        let out = dir.join("../../shots");
        std::fs::create_dir_all(&out).ok();
        kh_ui::def::write_png(&out.join(format!("{name}.png")), 1920, 1080, &rgba).unwrap();
    };
    let run = |view: &mut kh_ui::shop_moogle::MoogleShopView, menu: &mut ShopMenu, n: usize| {
        for _ in 0..n {
            view.sync(menu);
            view.tick();
        }
    };
    let text = |view: &kh_ui::shop_moogle::MoogleShopView, path: &str| view.movie.text(path).unwrap_or_default();
    // the talk asked for the BUY window
    let mut menu = ShopMenu::new(data(), ShopTab::Buy);
    view.open(&mut menu);
    assert!(menu.inline_qty, "KH3's own way: the quantity on the row");
    run(&mut view, &mut menu, 60);
    let row0 = "menuList.listMenuObj.itemList.coList.container.i0";
    assert_eq!(text(&view, &format!("{row0}.itemLabel.txName.textField")), "Potion");
    assert_eq!(text(&view, &format!("{row0}.numPrice.textField")), "400");
    assert_eq!(text(&view, &format!("{row0}.numShojisuu.textField")), "3");
    assert_eq!(text(&view, "stageFrame.stageObjR.munny.munnyNum.textField"), "2350");
    assert_eq!(text(&view, "menuList.stageCategory.category.categoryName.txName.textField"), "BUY");
    shot(&view, "11_moogle_buy");
    let mut s = Vec::new();
    menu.key(CampKey::Right, false, &mut s);
    menu.key(CampKey::Right, false, &mut s);
    assert_eq!((menu.layer, menu.qty), (Layer::List, 3), "the quantity on the row");
    run(&mut view, &mut menu, 10);
    assert_eq!(text(&view, &format!("{row0}.numInput.textField")), "3");
    menu.key(CampKey::Decide, false, &mut s);
    assert_eq!(menu.layer, Layer::Confirm, "A: KH3's confirmation at once");
    run(&mut view, &mut menu, 40);
    let mes = "mesWin.mesWinAnm.mesWinObj";
    assert_eq!((text(&view, &format!("{mes}.numMunny1.textField")), text(&view, &format!("{mes}.numMunny2.textField")), text(&view, &format!("{mes}.numPrice.textField"))), ("2350".to_string(), "1150".to_string(), "1200".to_string()));
    assert_eq!(text(&view, &format!("{mes}.txMesHelp.textField")), "Complete this purchase?");
    shot(&view, "12_moogle_confirm");
    menu.key(CampKey::Cancel, false, &mut s);
    assert_eq!((menu.layer, menu.qty), (Layer::List, 3), "B: back to the list, the quantity kept");
    menu.key(CampKey::Decide, false, &mut s);
    menu.key(CampKey::Decide, false, &mut s);
    assert_eq!(menu.take_events(), vec![ShopEvent::Buy { key: "ITEM_POTION".into(), qty: 3, price: 400 }]);
    run(&mut view, &mut menu, 40);
    assert_eq!(text(&view, "stageFrame.stageObjR.munny.munnyNum.textField"), "1150");
    shot(&view, "13_moogle_bought");
    menu.key(CampKey::R2, false, &mut s);
    assert_eq!(menu.tab, ShopTab::Sell);
    run(&mut view, &mut menu, 40);
    assert_eq!(text(&view, "menuList.stageCategory.category.categoryName.txName.textField"), "SELL");
    shot(&view, "14_moogle_sell");
    menu.key(CampKey::Cancel, false, &mut s);
    assert_eq!(menu.take_events(), vec![ShopEvent::Closed], "B on the list: back to the merchant's talk");
    view.close();
    let mut gone = false;
    for _ in 0..120 {
        if view.tick() {
            gone = true;
            break;
        }
    }
    assert!(gone && !view.is_open());
    // the talk asked for the SELL window: it opens there
    let mut menu = ShopMenu::new(data(), ShopTab::Sell);
    view.open(&mut menu);
    run(&mut view, &mut menu, 60);
    assert_eq!(text(&view, "menuList.stageCategory.category.categoryName.txName.textField"), "SELL");
    shot(&view, "15_moogle_opens_on_sell");
}
