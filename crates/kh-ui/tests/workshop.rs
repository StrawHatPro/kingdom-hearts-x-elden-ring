//! (weapons P6b) KH3's Workshop screens on the UI pack (movie 41_shop/Shop). Skipped without the pack.
//! Pictures: build/sora_tmp/weapons/P6b/shots/.

use kh_ui::camp::CampKey;
use kh_ui::workshop::view::WorkshopView;
use kh_ui::workshop::*;

fn need(key: &str, name: &str, need: u32, stock: u32) -> MatNeed {
    MatNeed { key: key.into(), name: name.into(), need, stock }
}

/// What a Sora a few bosses in shows at the Moogle (KH3's names).
fn data() -> WorkshopData {
    let recipe = |id: &str, key: &str, name: &str, category: u8, tab: u8, held: u32, max: u32, made: u32, new: bool, materials: Vec<MatNeed>| RecipeRow { id: id.into(), key: key.into(), name: name.into(), category, tab, held, max, made, new, materials, help: format!("{name}: a description long enough to need the whole box, so the pane shows that nothing is cut off at its edge; the right stick scrolls what does not fit.") };
    let blazing = |n, s| need("MAT_ITEM01", "Blazing Shard", n, s);
    WorkshopData {
        title: "Roundtable Hold".into(),
        munny: 12_345,
        commands: Screen::MOOGLE.iter().map(|s| (*s, matches!(s, Screen::Synthesis | Screen::Photo))).collect(),
        log: LogRow { created: 12, photos: 3, goals: 20, complete: [false, false, false] },
        recipes: vec![
            recipe("IS_0", "ITEM_ETHER", "Ether", 1, 1, 4, 9, 2, false, vec![blazing(2, 18), need("MAT_ITEM05", "Pulsing Shard", 1, 9)]),
            recipe("IS_12", "WEP_DONALD_03", "Warhammer+", 3, 2, 0, 1, 0, true, vec![need("MAT_ITEM53", "Mythril Stone", 2, 2), need("MAT_ITEM49", "Fluorite", 1, 1), need("MAT_ITEM15", "Pulsing Gem", 1, 1), need("MAT_ITEM14", "Hungry Shard", 1, 4)]),
            recipe("IS_19", "WEP_GOOFY_07", "Aegis Shield+", 3, 2, 0, 0, 0, true, vec![need("MAT_ITEM54", "Mythril Gem", 2, 0), need("MAT_ITEM51", "Adamantite", 1, 0)]),
            recipe("IS_40", "PRT_ITEM05", "Fencer's Earring", 4, 3, 1, 3, 1, false, vec![blazing(3, 18)]),
            recipe("IS_58", "WEP_KEYBLADE_SO_14", "Ultima Weapon", 3, 2, 0, 0, 0, false, vec![need("MAT_ITEM57", "Orichalcum+", 7, 1), need("MAT_ITEM40", "Wellspring Crystal", 2, 0)]),
        ],
        keyblades: vec![
            ForgeRow { key: "WEP_KEYBLADE_SO_00".into(), name: "Kingdom Key".into(), level: 2, max_level: 10, strength: 4, magic: 3, next: Some(ForgeNext { level: 3, strength: 5, magic: 3, ability: Some("Critical Converter".into()), materials: vec![blazing(3, 18), need("MAT_ITEM49", "Fluorite", 1, 1)], ready: true }) },
            ForgeRow { key: "WEP_KEYBLADE_SO_01".into(), name: "Hero's Origin".into(), level: 0, max_level: 10, strength: 5, magic: 2, next: Some(ForgeNext { level: 1, strength: 6, magic: 2, ability: None, materials: vec![need("MAT_ITEM02", "Frost Shard", 2, 0)], ready: false }) },
            ForgeRow { key: "WEP_KEYBLADE_SO_02".into(), name: "Shooting Star".into(), level: 10, max_level: 10, strength: 8, magic: 10, next: None },
            ForgeRow { key: "WEP_KEYBLADE_SO_05".into(), name: "Happy Gear".into(), level: 2, max_level: 10, strength: 6, magic: 3, next: None },
        ],
        photos: vec![
            PhotoRow { id: "IM_1".into(), text: "That fiend with the golden rune scared my pom-pom, kupo!".into(), help: "The moogle must want word that <color(strong)>Margit, the Fell Omen<color> is beaten.".into(), done: true, new: false, reward_key: "ACC_ITEM05".into(), reward_name: "Fire Bangle".into() },
            PhotoRow { id: "IM_2".into(), text: "A grafted giant sits on the throne, kupo!".into(), help: "The moogle must want word that <color(strong)>Godrick the Grafted<color> is beaten.".into(), done: false, new: true, reward_key: String::new(), reward_name: String::new() },
        ],
        goals: vec![
            GoalRow { id: "IC_1".into(), text: "Obtain 5 different synthesis materials.".into(), done: true, reward: RewardKind::Item, items: vec![(1, "Hi-Potion".into())] },
            GoalRow { id: "IC_8".into(), text: "Obtain 15 different synthesis materials.".into(), done: false, reward: RewardKind::Synthesis, items: vec![(3, "Warhammer+".into()), (3, "Clockwork Shield+".into())] },
            GoalRow { id: "IC_30".into(), text: "Obtain at least 30 <color(strong)>Blazing Shards<color>.".into(), done: false, reward: RewardKind::Buy, items: vec![(6, "Blazing Shard".into())] },
        ],
        materials: (1..=60).map(|k| MaterialRow { key: format!("MAT_ITEM{k:02}"), name: if k % 7 == 3 { "? ? ?".into() } else { format!("Material {k:02}") }, known: k % 7 != 3, stock: if k % 7 == 3 { 0 } else { k * 3 }, rank: 3u8.saturating_sub((k / 15) as u8) }).collect(),
    }
}

fn load() -> Option<(WorkshopView, kh_ui::Fonts)> {
    let Ok(mut pack) = kh_ui::Pack::open_default() else {
        eprintln!("SKIPPED: no build/sora_pack/ui");
        return None;
    };
    let fonts = kh_ui::Fonts::load(&mut pack).unwrap();
    match WorkshopView::load(&mut pack, fonts.clone()) {
        Ok(v) => Some((v, fonts)),
        Err(e) => {
            eprintln!("SKIPPED: no Shop movie ({e})");
            None
        }
    }
}

fn run(view: &mut WorkshopView, menu: &mut WorkshopMenu, n: usize) {
    for _ in 0..n {
        view.sync(menu);
        view.tick(Some(menu));
    }
}

fn shot(view: &WorkshopView, fonts: &kh_ui::Fonts, name: &str) {
    let mut canvas = kh_ui::Canvas::new(1920, 1080);
    let root = kh_ui::geom::Matrix::new(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
    let mut renderer = kh_ui::Renderer::new(fonts.clone());
    renderer.draw(&mut canvas, &view.movie, &root);
    if let Some(dialog) = view.dialog_movie() {
        renderer.draw(&mut canvas, dialog, &root);
    }
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|_, _| [0.12, 0.13, 0.16], &mut rgba);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/weapons/P6b/shots");
    std::fs::create_dir_all(&dir).ok();
    kh_ui::def::write_png(&dir.join(format!("{name}.png")), 1920, 1080, &rgba).unwrap();
}

fn t(view: &WorkshopView, path: &str) -> String {
    view.movie.text(path).unwrap_or_default()
}

/// THE MOOGLE'S WORKSHOP, every screen: the top with the SYNTH LOG and the entry dialogs; Synthesize Items (tabs, the
/// quantity on the row, the materials pane, the confirmation); Photo Missions; Collector's Goals (the three reward kinds);
/// the Material List; B back to the top and out.
#[test]
fn workshop_every_screen_on_the_pack() {
    let Some((mut view, fonts)) = load() else { return };
    let mut menu = WorkshopMenu::new(data(), Entry::Workshop);
    let mut s = Vec::new();
    view.open(&mut menu);
    let deposit = view.string("kh3_menu.synthesis.dialog_0010");
    menu.push_messages([Message { text: deposit, sound: None }], &mut s);
    run(&mut view, &mut menu, 40);
    assert!(view.dialog_movie().is_some(), "the entry dialog in the system window");
    assert!(view.dialog.text("dialogText.textField").unwrap_or_default().contains("left here at the Workshop"));
    shot(&view, &fonts, "01_top_entry_dialog");
    menu.key(CampKey::Decide, false, &mut s);
    run(&mut view, &mut menu, 30);
    assert!(view.dialog_movie().is_none());
    assert_eq!(t(&view, "menuMixTop.menuObj.cmd1.mainCmdNameAnm.mainCmdName.textField"), "Synthesize Items");
    assert_eq!(t(&view, "menuMixTop.menuObj.cmd2.mainCmdNameAnm.mainCmdName.textField"), "Journey to the Erdtree");
    assert_eq!(t(&view, "menuMixTop.menuObj.cmd4.mainCmdNameAnm.mainCmdName.textField"), "Material List", "the Moogle has no Keyblade Forge");
    assert_eq!(t(&view, "menuMixTop.menuObj.createLog.num3"), "20");
    assert_eq!(t(&view, "menuMixTop.stageCategory.category.categoryName.txName.textField"), "WORKSHOP");
    shot(&view, &fonts, "02_top");
    // Synthesize Items
    menu.key(CampKey::Decide, false, &mut s);
    run(&mut view, &mut menu, 30);
    let row0 = "menuSynthesis.menuObj.synthesisList.coList.container.i0";
    assert_eq!(t(&view, &format!("{row0}.itemLabel.txName.textField")), "Ether");
    assert_eq!(t(&view, &format!("{row0}.numShojisuu.textField")), "4");
    assert_eq!(t(&view, "menuSynthesis.menuObj.synthesisList.coTab.textField"), "All");
    menu.key(CampKey::Right, false, &mut s);
    menu.key(CampKey::Right, false, &mut s);
    run(&mut view, &mut menu, 10);
    assert_eq!(t(&view, &format!("{row0}.numInput.textField")), "3");
    assert_eq!(t(&view, "menuSynthesis.menuObj.infoMaterials.label1.numReq"), "6", "need x quantity");
    assert_eq!(t(&view, "menuSynthesis.menuObj.infoMaterials.label1.txName"), "Blazing Shard");
    shot(&view, &fonts, "03_synthesis_list");
    menu.key(CampKey::Decide, false, &mut s);
    run(&mut view, &mut menu, 30);
    assert_eq!(t(&view, "mesWin2.mesWinAnm.mesWinObj.txMesHelp.textField"), "Proceed with synthesis?");
    assert_eq!(t(&view, "mesWin2.mesWinAnm.mesWinObj.numKosuu.textField"), "x 3");
    shot(&view, &fonts, "04_synthesis_confirm");
    menu.key(CampKey::Left, false, &mut s);
    menu.key(CampKey::Decide, false, &mut s);
    assert_eq!(menu.take_events(), vec![WorkshopEvent::Synthesize { id: "IS_0".into(), count: 3 }]);
    run(&mut view, &mut menu, 30);
    menu.key(CampKey::R2, false, &mut s);
    run(&mut view, &mut menu, 20);
    assert_eq!(t(&view, "menuSynthesis.menuObj.synthesisList.coTab.textField"), "Items");
    menu.key(CampKey::R2, false, &mut s);
    menu.key(CampKey::Down, false, &mut s);
    run(&mut view, &mut menu, 20);
    assert_eq!(t(&view, "menuSynthesis.menuObj.synthesisList.coTab.textField"), "Weapons");
    shot(&view, &fonts, "05_synthesis_weapons_tab_lack");
    menu.key(CampKey::Cancel, false, &mut s);
    run(&mut view, &mut menu, 30);
    // Journey to the Erdtree (KH3's Photo Missions: the milestones)
    menu.key(CampKey::Down, false, &mut s);
    menu.key(CampKey::Decide, false, &mut s);
    run(&mut view, &mut menu, 40);
    assert!(t(&view, "menuPhoto.menuObj.photoList.coList.container.i0.txText.textField").contains("kupo"));
    assert_eq!(t(&view, "menuPhoto.stageCategory.category.categoryName.txName.textField"), "Journey to the Erdtree", "the screen title");
    assert_eq!(t(&view, "menuPhoto.menuObj.infoItem.body.txName"), "Fire Bangle");
    shot(&view, &fonts, "06_photo_done");
    menu.key(CampKey::Down, false, &mut s);
    run(&mut view, &mut menu, 20);
    assert_eq!(t(&view, "menuPhoto.menuObj.infoItem.body.txName"), "? ? ?");
    shot(&view, &fonts, "07_photo_unknown");
    menu.key(CampKey::Cancel, false, &mut s);
    run(&mut view, &mut menu, 30);
    // Collector's Goals: the three reward kinds
    menu.key(CampKey::Down, false, &mut s);
    menu.key(CampKey::Decide, false, &mut s);
    run(&mut view, &mut menu, 40);
    assert!(t(&view, "menuCollection.menuObj.infoBonus.txText").contains("reward"));
    assert_eq!(t(&view, "menuCollection.menuObj.infoBonus.label1.txName"), "Hi-Potion");
    shot(&view, &fonts, "08_goals_item");
    menu.key(CampKey::Down, false, &mut s);
    run(&mut view, &mut menu, 10);
    assert!(t(&view, "menuCollection.menuObj.infoBonus.txText").contains("synthesize"));
    shot(&view, &fonts, "09_goals_synthesis");
    menu.key(CampKey::Down, false, &mut s);
    run(&mut view, &mut menu, 10);
    assert!(t(&view, "menuCollection.menuObj.infoBonus.txText").contains("merchants"));
    shot(&view, &fonts, "10_goals_buy_merchants");
    menu.key(CampKey::Cancel, false, &mut s);
    run(&mut view, &mut menu, 30);
    // the Material List
    menu.key(CampKey::Down, false, &mut s);
    menu.key(CampKey::Decide, false, &mut s);
    run(&mut view, &mut menu, 40);
    assert_eq!(t(&view, "menuMaterial.menuObj.materialList.coList.container.i1.itemLabel.txName.textField"), "Material 02");
    assert_eq!(t(&view, "menuMaterial.menuObj.materialList.coList.container.i2.itemLabel.txName.textField"), "? ? ?");
    shot(&view, &fonts, "11_materials");
    menu.key(CampKey::R1, false, &mut s);
    run(&mut view, &mut menu, 10);
    assert_eq!(t(&view, "menuMaterial.menuObj.materialList.coList.container.i0.itemLabel.txName.textField"), "Material 21");
    shot(&view, &fonts, "12_materials_page2");
    menu.key(CampKey::Cancel, false, &mut s);
    run(&mut view, &mut menu, 30);
    menu.key(CampKey::Cancel, false, &mut s);
    assert_eq!(menu.take_events(), vec![WorkshopEvent::SeenRecipe("IS_12".into()), WorkshopEvent::SeenRecipe("IS_19".into()), WorkshopEvent::SeenPhoto("IM_2".into()), WorkshopEvent::Closed], "the rows seen, then B on the top");
    view.close();
    let mut gone = false;
    for _ in 0..120 {
        if view.tick(Some(&mut menu)) {
            gone = true;
            break;
        }
    }
    assert!(gone && !view.is_open(), "the menu's out ended");
}

/// A SMITH'S KEYBLADE FORGE: straight on the Forge; the picker; the status pane before -> after, the level icons, the
/// ability, the materials; at +10 the disabled materials; lacking materials: dialog_0210; Yes / No; the power-up's
/// effects; B closes it.
#[test]
fn workshop_forge_on_the_pack() {
    let Some((mut view, fonts)) = load() else { return };
    let mut menu = WorkshopMenu::new(data(), Entry::Forge);
    let mut s = Vec::new();
    view.open(&mut menu);
    run(&mut view, &mut menu, 50);
    let enh = "menuKeyb.menuObj.infoEnhance";
    assert_eq!(t(&view, "menuKeyb.menuObj.infoKeyb.txName"), "<icon(Weapon)>Kingdom Key"); // (FORGE-PREVIEW) KH3's Keyblade mark before the name
    assert_eq!((t(&view, &format!("{enh}.label1.num1.textField")), t(&view, &format!("{enh}.label1.num2.textField"))), ("4".into(), "5".into()));
    assert_eq!(t(&view, &format!("{enh}.label3.txName")), "Critical Converter");
    assert_eq!(view.movie.current_label(&format!("{enh}.icon2")), Some("unlock"));
    assert_eq!(view.movie.current_label(&format!("{enh}.icon3")), Some("lock"));
    assert_eq!(view.movie.current_label(&format!("{enh}.icon10")), Some("lockF"));
    assert!(t(&view, "menuKeyb.menuObj.helpText.txHelpText.textField").contains("Kingdom Key"));
    shot(&view, &fonts, "13_forge");
    menu.key(CampKey::Right, false, &mut s);
    for _ in 0..40 {
        if view.ready() {
            break;
        }
        run(&mut view, &mut menu, 1);
    }
    run(&mut view, &mut menu, 4);
    assert_eq!(t(&view, "menuKeyb.menuObj.infoKeyb.txName"), "<icon(Weapon)>Hero's Origin"); // (FORGE-PREVIEW)
    menu.key(CampKey::Decide, false, &mut s);
    run(&mut view, &mut menu, 30);
    assert!(view.dialog.text("dialogText.textField").unwrap_or_default().contains("lack the needed materials"));
    shot(&view, &fonts, "14_forge_lack");
    menu.key(CampKey::Decide, false, &mut s);
    run(&mut view, &mut menu, 20);
    menu.key(CampKey::Right, false, &mut s);
    run(&mut view, &mut menu, 30);
    assert_eq!(view.movie.current_label("menuKeyb.menuObj.infoMaterials"), Some("disable"), "+10: nothing more");
    shot(&view, &fonts, "15_forge_max");
    menu.key(CampKey::Right, false, &mut s);
    run(&mut view, &mut menu, 30);
    menu.key(CampKey::Right, false, &mut s);
    run(&mut view, &mut menu, 30);
    assert_eq!(menu.forge_row().unwrap().name, "Kingdom Key");
    menu.key(CampKey::Decide, false, &mut s);
    run(&mut view, &mut menu, 30);
    assert!(view.dialog.text("dialogText.textField").unwrap_or_default().contains("Power up the"));
    shot(&view, &fonts, "16_forge_confirm");
    menu.key(CampKey::Decide, false, &mut s);
    assert_eq!(menu.take_events(), vec![WorkshopEvent::Forge { key: "WEP_KEYBLADE_SO_00".into() }]);
    run(&mut view, &mut menu, 20);
    shot(&view, &fonts, "17_forge_power_up_effect");
    for _ in 0..300 {
        if menu.layer == Layer::Main {
            break;
        }
        run(&mut view, &mut menu, 1);
    }
    assert_eq!(menu.layer, Layer::Main, "the effects end, the pad comes back");
    run(&mut view, &mut menu, 10);
    assert_eq!(view.movie.current_label(&format!("{enh}.icon3")), Some("unlock"));
    shot(&view, &fonts, "18_forge_after");
    menu.key(CampKey::Cancel, false, &mut s);
    assert_eq!(menu.take_events(), vec![WorkshopEvent::Closed]);
}

/// (P6b, development aid) the Shop movie's Workshop parts in, their visible tree and a picture.
/// `SHOP_PLAY="menuMixTop=inNext;stageBg=mixIn"` `SHOP_NAME=top`.
#[test]
#[ignore]
fn workshop_debug_movie() {
    let Ok(mut pack) = kh_ui::Pack::open_default() else { return };
    let fonts = kh_ui::Fonts::load(&mut pack).unwrap();
    let def = pack.movie_linked("Shop", &kh_ui::shop_moogle::LIBRARIES).unwrap();
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
    let depth_max: usize = std::env::var("SHOP_DEPTH").ok().and_then(|d| d.parse().ok()).unwrap_or(7);
    fn walk(s: &kh_ui::player::Sprite, path: &str, shown: bool, depth: usize, max: usize) {
        for node in s.children.iter().chain(&s.extra) {
            let p = if path.is_empty() { node.label() } else { format!("{path}.{}", node.label()) };
            let vis = shown && node.is_visible() && node.cx.mul[3] > 0.01;
            match &node.kind {
                kh_ui::player::Kind::Sprite(child) => {
                    if vis && depth < max {
                        println!("S {p} [{:?}] {:?}", child.def.class, child.def.labels.iter().map(|l| l.0.clone()).collect::<Vec<_>>().join(","));
                    }
                    walk(child, &p, vis, depth + 1, max)
                }
                kh_ui::player::Kind::Text(t) if vis => println!("T {p} = {:?}", t.text.clone().unwrap_or_else(|| format!("[authored] {}", t.def.initial))),
                _ => {}
            }
        }
    }
    walk(&movie.root, "", true, 0, depth_max);
    let mut canvas = kh_ui::Canvas::new(1920, 1080);
    kh_ui::Renderer::new(fonts).draw(&mut canvas, &movie, &kh_ui::geom::Matrix::new(1.0, 0.0, 0.0, 1.0, 0.0, 0.0));
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|_, _| [0.12, 0.13, 0.16], &mut rgba);
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/weapons/P6b/shots");
    std::fs::create_dir_all(&out).ok();
    kh_ui::def::write_png(&out.join(format!("explore_{}.png", std::env::var("SHOP_NAME").unwrap_or_else(|_| "x".into()))), 1920, 1080, &rgba).unwrap();
}
