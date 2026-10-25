//! Tier 3, second part: the camp screens drawn from the movies' list widgets (Abilities, Config,
//! Equipment, Items, Customize, Game Help, dialogs; docs/sora-ui-spec.md 10.4 - 10.9, 11b tests
//! 12 - 15, 17). Pictures: build/sora_tmp/hud/t4_*.png.

use std::path::Path;

use kh_ui::camp::{AbilityIn, CampAction, CampKey, CampMenu, CampScreen, OptionRow, StatsIn};
use kh_ui::camp_custom::{CommandEntry, CustomizeIn, HelpCategory, HelpTopic, GROUP_ITEM, GROUP_MAGIC};
use kh_ui::camp_equip::{EquipIn, EquipSlot, InvEntry, Piece, StockEntry, CAT_ARMOUR, CAT_ITEM, KIND_ACCESSORY, KIND_ARMOUR, KIND_ITEM, KIND_WEAPON};
use kh_ui::camp_menu::CampMenuView;
use kh_ui::def::write_png;
use kh_ui::{Canvas, Fonts, Matrix, Pack, Renderer};

fn pack() -> Option<Pack> {
    Pack::open_default().map_err(|_| eprintln!("SKIPPED: no build/sora_pack/ui")).ok()
}

fn shot(name: &str, view: &CampMenuView, fonts: &Fonts) {
    let dir = match std::env::var_os("KH_UI_SHOTS") {
        Some(dir) => std::path::PathBuf::from(dir),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/hud"),
    };
    std::fs::create_dir_all(&dir).ok();
    let mut canvas = Canvas::new(1920, 1080);
    let mut renderer = Renderer::new(fonts.clone());
    renderer.draw(&mut canvas, &view.movie, &Matrix::IDENTITY);
    for movie in [view.page_movie(), view.dialog_movie()].into_iter().flatten() {
        renderer.draw(&mut canvas, movie, &Matrix::IDENTITY);
    }
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|_, y| [0.30 + 0.1 * (y as f32 / 1080.0), 0.38, 0.46], &mut rgba);
    write_png(&dir.join(format!("t4_{name}.png")), 1920, 1080, &rgba).unwrap();
}

fn stats_99() -> StatsIn {
    StatsIn { name: "Sora".into(), level: 99, exp: 1_527_640, next_level_exp: 0, level_exp: 1_527_640, hp: 100, hp_max: 265, mp: 169, mp_max: 169, focus: 100, focus_max: 100, focus_unlocked: true, ap_max: 180, strength: 66, magic: 67, defense: 59, resists: [0; 6], munny: 1234, play_seconds: 3725 }
}

struct Walk {
    view: CampMenuView,
    menu: CampMenu,
    fonts: Fonts,
    sounds: Vec<kh_ui::UiSound>,
}

impl Walk {
    fn new(pack: &mut Pack) -> Walk {
        let fonts = Fonts::load(pack).unwrap();
        let mut view = CampMenuView::load(pack, fonts.clone()).unwrap();
        let mut sounds = Vec::new();
        let menu = CampMenu::open(false, false, stats_99(), &mut sounds);
        view.open(&menu);
        let mut walk = Walk { view, menu, fonts, sounds };
        walk.frames(45);
        walk
    }

    fn frames(&mut self, n: usize) {
        for _ in 0..n {
            self.view.sync(&mut self.menu);
            self.view.tick();
        }
    }

    fn key(&mut self, key: CampKey) -> Vec<CampAction> {
        let actions = self.menu.key(key, false, &mut self.sounds);
        self.frames(2);
        actions
    }

    /// Top screen: to the entry with this index, decide (twice for a character list).
    fn enter(&mut self, entry: usize, character: bool) -> Vec<CampAction> {
        while self.menu.top_cursor != entry {
            self.key(CampKey::Down);
        }
        let mut actions = self.key(CampKey::Decide);
        if character {
            self.frames(20);
            actions = self.key(CampKey::Decide);
        }
        actions
    }

    fn shot(&mut self, name: &str) {
        self.frames(40);
        shot(name, &self.view, &self.fonts);
    }

    fn finish(&self, what: &str) {
        println!("{what}: paths not found: {:?}", self.view.movie.misses);
        assert!(self.view.movie.misses.is_empty(), "{what}: paths not found: {:?}", self.view.movie.misses);
    }
}

fn abilities() -> Vec<AbilityIn> {
    let mut list = Vec::new();
    let mut add = |id: i32, name: &str, category: u8, ap: i32, owned: u8, equipped: u8| list.push(AbilityIn { id, name: name.into(), category, ap, priority: id, owned, equipped: (1u8 << equipped) - 1, help: format!("Help line of {name}.") });
    add(1, "Aerial Recovery", 0, 2, 1, 1);
    add(2, "Payback Strike", 0, 5, 1, 1);
    add(3, "Block", 0, 2, 1, 1);
    add(4, "Counter Slash", 0, 4, 1, 1);
    add(5, "Counter Impact", 0, 4, 1, 0);
    add(6, "Counter Kick", 0, 4, 1, 0);
    add(23, "Air Slide", 1, 2, 3, 3);
    add(24, "Superslide", 1, 2, 3, 3);
    add(28, "Combo Plus", 2, 4, 2, 2);
    for n in 0..12 {
        add(100 + n, &format!("Support Ability {n}"), 2, 11, 1, 1);
    }
    add(200, "Filler", 2, 15, 1, 1);
    list
}

#[test]
fn abilities_from_the_list_widget() {
    let Some(mut pack) = pack() else { return };
    let mut w = Walk::new(&mut pack);
    assert_eq!(w.enter(1, true), vec![CampAction::Opened(CampScreen::Abilities)]);
    w.menu.set_abilities(&abilities(), 180);
    assert_eq!((w.menu.abilities.rows.len(), w.menu.abilities.used_ap), (27, 180));
    w.shot("abilities");
    let list = "ability.abilityAnm.abilityList.container";
    assert_eq!(w.view.movie.text(&format!("{list}.i0.abilityNameAnm.abilityName.textField")).as_deref(), Some("Aerial Recovery"));
    assert_eq!(w.view.movie.current_label(&format!("{list}.i0")), Some("select"));
    assert_eq!(w.view.movie.current_label(&format!("{list}.i0.abilityCategoryTitle")), Some("titleAction"));
    assert_eq!(w.view.movie.current_label(&format!("{list}.i1.abilityCategoryTitle")), Some("lineAction"));
    assert_eq!(w.view.movie.current_label(&format!("{list}.i6.abilityCategoryTitle")), Some("titleMove"));
    let pane = "ability.abilityAnm.selectCharaImage1.abilityParameterAnm.abilityParameter";
    assert_eq!(w.view.movie.text(&format!("{pane}.apNow.apText.textField")).as_deref(), Some("180"));
    assert_eq!(w.view.movie.current_frame(&format!("{pane}.apGgBarAnm")), Some(201));
    // Cursor on Counter Impact (not installed): 184, `lack`.
    for _ in 0..4 {
        w.key(CampKey::Down);
    }
    assert_eq!(w.view.movie.text(&format!("{pane}.apNow.apText.textField")).as_deref(), Some("184"));
    assert_eq!(w.view.movie.current_label(&format!("{pane}.apNow")), Some("lack"));
    assert_eq!(w.view.movie.text("ability.helpText.txHelpText.textField").as_deref(), Some("Help line of Counter Impact."));
    // The list scrolls: 12 more rows down, the window follows and the scroll bar moves.
    let thumb = |w: &Walk| w.view.movie.node("ability.abilityAnm.abilityList.scrollbar.thumb").map(|n| n.matrix.ty).unwrap();
    let before = thumb(&w);
    for _ in 0..12 {
        w.key(CampKey::Down);
    }
    assert_eq!((w.menu.abilities.cursor(), w.menu.abilities.list.scroll), (16, 7));
    assert!(thumb(&w) > before, "the thumb moved down");
    assert_eq!(w.view.movie.current_label(&format!("{list}.i9")), Some("select"));
    w.shot("abilities_scrolled");
    // R1: a page down.
    w.key(CampKey::R1);
    assert_eq!((w.menu.abilities.cursor(), w.menu.abilities.list.scroll), (26, 17));
    w.finish("abilities");
}

fn options() -> Vec<OptionRow> {
    let labels = ["Camera Work", "Vertical Camera Control", "Horizontal Camera Control", "Vertical Camera Sensitivity (Gamepad)", "Horizontal Camera Sensitivity (Gamepad)", "Vertical Reticle Control", "Horizontal Reticle Control", "Vertical Ride Control", "Combo Cam", "Keyblade Cinematics", "Link Cinematics", "Attraction Cinematics", "Command Menus", "Command Selection", "Cursor Position", "Situation Commands", "Control Type", "Subtitles", "Maps", "Map Orientation", "Vibration", "Master Volume", "Graphics", "Display Area", "Screen Brightness", "Language"];
    labels
        .iter()
        .enumerate()
        .map(|(i, label)| match i {
            3 | 4 => OptionRow { label: label.to_string(), choices: vec![], range: (1, 100), value: 50, stored: true },
            21 => OptionRow { label: label.to_string(), choices: vec![], range: (0, 10), value: 10, stored: true },
            0 => OptionRow { label: label.to_string(), choices: vec!["Auto".into(), "Manual".into()], range: (0, 1), value: 0, stored: true },
            22..=25 => OptionRow { label: label.to_string(), choices: vec!["Default Mode".into(), "Stable Mode".into()], range: (0, 1), value: 0, stored: false },
            _ => OptionRow { label: label.to_string(), choices: vec!["Normal".into(), "Flipped".into()], range: (0, 1), value: 0, stored: true },
        })
        .collect()
}

#[test]
fn config_from_the_config_library() {
    let Some(mut pack) = pack() else { return };
    let mut w = Walk::new(&mut pack);
    assert_eq!(w.enter(6, false), vec![CampAction::Opened(CampScreen::Config)]);
    w.menu.set_options(options(), true);
    w.shot("config");
    let row = |n: usize| format!("config.configMenu.configPack.setConfigValue{n}");
    assert_eq!(w.view.movie.text(&format!("{}.itemName", row(1))).as_deref(), Some("Camera Work"));
    assert_eq!(w.view.movie.text(&format!("{}.selectValue", row(1))).as_deref(), Some("Auto"));
    assert_eq!(w.view.movie.current_label(&row(1)), Some("select"));
    assert_eq!(w.view.movie.text(&format!("{}.slider.sliderBar.textField", row(4))).as_deref(), Some("50"));
    assert_eq!(w.view.movie.text("config.configMenu.helpText.txHelpText.textField").as_deref(), Some("The camera will automatically move into a good position."));
    // Right: Manual, and its help line.
    w.key(CampKey::Right);
    assert_eq!(w.view.movie.text(&format!("{}.selectValue", row(1))).as_deref(), Some("Manual"));
    assert_eq!(w.view.movie.text("config.configMenu.helpText.txHelpText.textField").as_deref(), Some("The camera will not move unless you take control of it."));
    // The window: R1 pages down by 10 rows, the pack moves up.
    let y = |w: &Walk| w.view.movie.node("config.configMenu.configPack").unwrap().matrix.ty;
    let top = y(&w);
    w.key(CampKey::R1);
    assert_eq!((w.menu.option_cursor, w.menu.option_scroll), (10, 10));
    assert_eq!(y(&w), top - 480.0);
    w.key(CampKey::R1);
    assert_eq!((w.menu.option_cursor, w.menu.option_scroll), (16, 16));
    w.key(CampKey::R1);
    assert_eq!((w.menu.option_cursor, w.menu.option_scroll), (25, 16), "the last page: to the last row");
    w.shot("config_end");
    w.key(CampKey::R1);
    assert_eq!((w.menu.option_cursor, w.menu.option_scroll), (0, 0), "a fresh press wraps");
    w.key(CampKey::Up);
    assert_eq!((w.menu.option_cursor, w.menu.option_scroll), (25, 16));
    // Triangle: the dialog; Yes restores.
    w.key(CampKey::Triangle);
    w.shot("config_dialog");
    assert!(w.view.dialog_movie().is_some());
    w.key(CampKey::Left);
    assert_eq!(w.key(CampKey::Decide), vec![CampAction::RestoreConfigDefaults]);
    // Leaving applies the change of row 1.
    let applied = w.key(CampKey::Cancel);
    assert!(matches!(applied.as_slice(), [CampAction::ApplyConfig(values)] if values[0] == 1));
    w.finish("config");
}

fn piece(id: &str, kind: u8) -> Piece {
    Piece { id: id.into(), name: id.into(), kind, help: format!("About the {id}."), ..Piece::default() }
}

fn loadout() -> EquipIn {
    let mut slots = Vec::new();
    for slot in 0..3 {
        slots.push(EquipSlot { kind: KIND_WEAPON, slot, piece: (slot == 0).then(|| Piece { abilities: vec!["Treasure Magnet".into()], ..piece("Kingdom Key", KIND_WEAPON) }), current: slot == 0, ..EquipSlot::default() });
    }
    for kind in [KIND_ARMOUR, KIND_ACCESSORY] {
        for slot in 0..3 {
            slots.push(EquipSlot { kind, slot, ..EquipSlot::default() });
        }
    }
    for (slot, item) in ["Potion", "Potion", "Hi-Potion", "Hi-Potion", "Ether", "Ether", "", ""].into_iter().enumerate() {
        slots.push(EquipSlot { kind: KIND_ITEM, slot, piece: (!item.is_empty()).then(|| piece(item, KIND_ITEM)), auto_reload: slot < 2, ..EquipSlot::default() });
    }
    let stock = vec![StockEntry { piece: Piece { defense: 2, resist: [20, 0, 0, 0, 0, 0], ..piece("Test Belt", KIND_ARMOUR) }, count: 1 }, StockEntry { piece: piece("Hero's Belt", KIND_ARMOUR), count: 2 }];
    EquipIn { slots, stock, ap_in_use: 180, stock_max: 99 }
}

#[test]
fn equipment_lists_and_preview() {
    let Some(mut pack) = pack() else { return };
    let mut w = Walk::new(&mut pack);
    assert_eq!(w.enter(0, true), vec![CampAction::Opened(CampScreen::Equipment)]);
    w.menu.equipment.set(loadout());
    w.shot("equip");
    let list = "equip.equipDetailAnm.equipDetail.equipListVisibleAnm.equipListVisible.equipList.container";
    let name = |w: &Walk, slot: usize| w.view.movie.text(&format!("{list}.i{slot}.equipItemNameColorAnm.equipItemNameColor.equipItemName"));
    assert_eq!(name(&w, 0).as_deref(), Some("Kingdom Key"));
    assert_eq!(name(&w, 1).as_deref(), Some("—"));
    assert_eq!(name(&w, 9).as_deref(), Some("Potion"));
    assert_eq!(w.view.movie.current_label(&format!("{list}.i0.equipListTitleItem")), Some("lineTitleWeapon"));
    assert_eq!(w.view.movie.current_label(&format!("{list}.i3.equipListTitleItem")), Some("lineTitleArmor"));
    assert_eq!(w.view.movie.current_label(&format!("{list}.i9.autoReloadMark")), Some("loop"));
    // Square on the Kingdom Key: the dialog.
    w.key(CampKey::Square);
    w.shot("equip_dialog");
    assert!(w.menu.dialog.is_some() && w.view.dialog_movie().is_some());
    w.key(CampKey::Decide);
    assert!(w.menu.dialog.is_none());
    // The armour slot's stock list with the preview.
    for _ in 0..3 {
        w.key(CampKey::Down);
    }
    w.key(CampKey::Decide);
    w.shot("equip_stock");
    let stock = "equip.equipReplaceAnm.equipReplace.replaceListVisibleAnm.replaceListVisible.replaceList.container";
    assert_eq!(w.view.movie.text(&format!("{stock}.i0.listItemName")).as_deref(), Some("Test Belt"));
    assert_eq!(w.view.movie.text(&format!("{stock}.i1.itemCnt")).as_deref(), Some("2"));
    let pane = "equip.equipReplaceAnm.equipReplace.itemStatusB";
    assert_eq!(w.view.movie.text(&format!("{pane}.num23.txText.textField")).as_deref(), Some("61"));
    assert_eq!(w.view.movie.current_label(&format!("{pane}.num23")), Some("up"));
    assert_eq!(w.view.movie.text(&format!("{pane}.num11.txText.textField")).as_deref(), Some("265"));
    assert_eq!(w.view.movie.text(&format!("{pane}.num31.txText.textField")).as_deref(), Some("20%"));
    assert_eq!(w.key(CampKey::Decide), vec![CampAction::Equip { kind: KIND_ARMOUR, slot: 0, id: Some("Test Belt".into()) }]);
    // Triangle on the Kingdom Key: its ability list.
    for _ in 0..3 {
        w.key(CampKey::Up);
    }
    w.key(CampKey::Triangle);
    w.shot("equip_abilities");
    w.key(CampKey::Cancel);
    w.key(CampKey::Cancel);
    // (DON-UI) B is one level (the user's rule for all menus): the character list first, then the top screen
    assert_eq!(w.menu.screen, CampScreen::Character(kh_ui::camp::TopEntry::Equipment));
    w.key(CampKey::Cancel);
    assert_eq!(w.menu.screen, CampScreen::Top);
    w.frames(40);
    w.finish("equipment");
}

#[test]
fn items_tabs_and_target() {
    let Some(mut pack) = pack() else { return };
    let mut w = Walk::new(&mut pack);
    assert_eq!(w.enter(2, false), vec![CampAction::Opened(CampScreen::Items)]);
    let potion = InvEntry { id: "Potion".into(), name: "Potion".into(), category: CAT_ITEM, count: 5, usable: true, benefit: true, help: "Restores a little HP.".into(), benefits: Vec::new() }; // (party P2) F's new field
    w.menu.items.set(vec![potion, InvEntry { id: "PRT_ITEM01".into(), name: "Hero's Belt".into(), category: CAT_ARMOUR, count: 2, ..InvEntry::default() }]);
    w.shot("items");
    let list = "backyard.backyardAnm.backyardItem.backyardList.container";
    assert_eq!(w.view.movie.text(&format!("{list}.i0.backyardItemName")).as_deref(), Some("Potion"));
    assert_eq!(w.view.movie.text(&format!("{list}.i0.itemCnt")).as_deref(), Some("5"));
    assert_eq!(w.view.movie.text("backyard.backyardAnm.backyardItem.listTabCamp.textField").as_deref(), Some("All"));
    w.key(CampKey::R2);
    assert_eq!(w.view.movie.text("backyard.backyardAnm.backyardItem.listTabCamp.textField").as_deref(), Some("Items"));
    assert_eq!(w.view.movie.current_label("backyard.backyardAnm.backyardItem.listTabCamp.t1"), Some("select"));
    w.key(CampKey::Decide);
    w.shot("items_target");
    assert_eq!(w.view.movie.text("backyard.backyardAnm.charaList.btnSelectChara1.parameterHp.hpNow.textField").as_deref(), Some("100"));
    assert_eq!(w.key(CampKey::Decide), vec![CampAction::UseItem { id: "Potion".into() }]);
    w.key(CampKey::Cancel);
    w.key(CampKey::Triangle);
    w.shot("items_discard");
    w.key(CampKey::Cancel);
    w.finish("items");
}

fn customize_99() -> CustomizeIn {
    let mut entries = Vec::new();
    for (base, names) in [(29, ["Fire", "Fira", "Firaga"]), (33, ["Blizzard", "Blizzara", "Blizzaga"]), (37, ["Thunder", "Thundara", "Thundaga"]), (41, ["Water", "Watera", "Waterga"]), (45, ["Aero", "Aerora", "Aeroga"]), (49, ["Cure", "Cura", "Curaga"])] {
        for (tier, name) in names.into_iter().enumerate() {
            entries.push(CommandEntry { kind: base + tier as u8, name: name.into(), group: GROUP_MAGIC, auto_reload: false, help: format!("{name}: a spell.") });
        }
    }
    entries.push(CommandEntry { kind: 58, name: "Potion".into(), group: GROUP_ITEM, auto_reload: true, help: String::new() });
    CustomizeIn { magic_slots: [31, 43, 51, 35, 39, 47], shortcuts: [[31, 51, 0, 58], [0; 4], [0; 4]], entries, names: Vec::new(), friends: Vec::new(), ..CustomizeIn::default() } // (party P2) F's new field: no friends; (party X2) the leader's fields
}

#[test]
fn customize_layers() {
    let Some(mut pack) = pack() else { return };
    let mut w = Walk::new(&mut pack);
    assert_eq!(w.enter(3, false), vec![CampAction::Opened(CampScreen::Customize)]);
    w.menu.customize.set(customize_99());
    w.shot("customize");
    assert_eq!(w.view.movie.text("customize.customizeMenu.btnCmdMenu1.cmdMenuListName.textField").as_deref(), Some("Magic"));
    w.key(CampKey::Decide);
    w.shot("customize_magic");
    let common = "customize.cmdMenu2ndLayer.cmdSetCommon";
    assert_eq!(w.view.movie.text(&format!("{common}.btnSetCmdCommon1.cmdMove.cmdReplace.setCmdName.setItemName")).as_deref(), Some("Firaga"));
    w.key(CampKey::Decide);
    w.shot("customize_tier");
    assert_eq!(w.view.movie.text("customize.cmdMenu3rdLayer.cmdChangeList.btnSelectCmd1.selectCmdName.textField").as_deref(), Some("Fire"));
    w.key(CampKey::Down);
    assert_eq!(w.key(CampKey::Decide), vec![CampAction::SetMagicSlot { slot: 0, kind: 29 }]);
    assert_eq!(w.view.movie.text(&format!("{common}.btnSetCmdCommon1.cmdMove.cmdReplace.setCmdName.setItemName")).as_deref(), Some("Fire"));
    w.key(CampKey::Cancel);
    w.key(CampKey::Down);
    w.key(CampKey::Decide);
    w.shot("customize_set");
    let set = "customize.shortcut2ndLayer.setShortcutCmd";
    assert_eq!(w.view.movie.text(&format!("{set}.btnSetShortcutCmd2.shortcutSetName.textField")).as_deref(), Some("Curaga"));
    assert_eq!(w.view.movie.current_label(&format!("{set}.btnSetShortcutCmd4.autoReloadMark")), Some("loop"));
    w.key(CampKey::Decide);
    w.shot("customize_list");
    let list = "customize.shortcut3rdLayer.shortcutChangeListAnm.shortcutChangeList.container";
    assert_eq!(w.view.movie.text(&format!("{list}.i0.selectCmdName.textField")).as_deref(), Some("Fire"));
    assert_eq!(w.menu.customize.list.count, 19);
    w.key(CampKey::R2);
    assert_eq!(w.view.movie.text("customize.shortcut3rdLayer.shortcutChangeListAnm.listTabCampB.textField").as_deref(), Some("Magic"));
    w.finish("customize");
}

#[test]
fn game_help_lists_and_page() {
    let Some(mut pack) = pack() else { return };
    let mut w = Walk::new(&mut pack);
    assert_eq!(w.enter(5, false), vec![CampAction::Opened(CampScreen::GameHelp)]);
    let text = "<color(heading)>Attacking<color><br>Press <icon(Decide)> while <color(strong)>Attack<color> is selected to attack with Sora's Keyblade. You can keep attacking to chain together a <color(strong)>combo<color>.";
    w.menu.help.set(vec![
        HelpCategory { name: "Mechanics".into(), topics: vec![HelpTopic { title: "Save Points".into(), pages: vec!["Save your game.".into()] }] },
        HelpCategory { name: "Combat".into(), topics: vec![HelpTopic { title: "Combat Basics".into(), pages: vec![text.into(), "Second page.".into()] }] },
    ]);
    w.shot("help");
    w.key(CampKey::Down);
    w.key(CampKey::Decide);
    w.key(CampKey::Decide);
    w.shot("help_page");
    let page = w.view.page_movie().expect("the page movie is up");
    assert_eq!(page.text("helpTitle.helpTitleAnm.helpTitle").as_deref(), Some("Combat Basics"));
    assert_eq!(page.text("helpTitle.pageNumber.textField").as_deref(), Some("1 / 2"));
    println!("help page paths not found: {:?}", page.misses);
    assert!(page.misses.is_empty());
    w.key(CampKey::Cancel);
    w.frames(60);
    assert!(w.view.page_movie().is_none(), "the page closed");
    w.finish("game help");
}
