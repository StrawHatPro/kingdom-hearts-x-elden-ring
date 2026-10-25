//! (IMPL_ER37, the user: "the items in the item screen and in the equipment screen aren't showing their pictures or
//! descriptions, same with the keyblade in the equip menu") The item key -> picture / help text lookup, and the status
//! panes of Items, Equipment (slot list) and the Equipment stock list showing both. Skipped without the UI pack.

use kh_ui::camp::{CampKey, CampMenu, CampScreen, StatsIn, TopEntry};
use kh_ui::camp_equip::{EquipIn, EquipLayer, EquipSlot, InvEntry, Piece, StockEntry, CAT_ITEM, CAT_KEY, KIND_WEAPON};
use kh_ui::camp_menu::CampMenuView;

fn stats() -> StatsIn {
    StatsIn { name: "Sora".into(), level: 9, exp: 100, next_level_exp: 50, level_exp: 80, hp: 100, hp_max: 120, mp: 100, mp_max: 100, focus: 100, focus_max: 100, focus_unlocked: true, ap_max: 20, strength: 10, magic: 10, defense: 10, resists: [0; 6], munny: 0, play_seconds: 60 }
}

fn frames(view: &mut CampMenuView, menu: &mut CampMenu, n: usize) {
    for _ in 0..n {
        view.sync(menu);
        view.tick();
    }
}

fn load() -> Option<CampMenuView> {
    let Ok(mut pack) = kh_ui::Pack::open_default() else {
        eprintln!("SKIPPED: no build/sora_pack/ui");
        return None;
    };
    let fonts = kh_ui::Fonts::load(&mut pack).unwrap();
    Some(CampMenuView::load(&mut pack, fonts).unwrap())
}

/// The pane's picture is drawn (the Icon holds a bitmap fill) and visible; its help text is `help`.
fn assert_pane(view: &CampMenuView, pane: &str, help_start: &str) {
    assert_eq!(view.movie.current_label(pane), Some("loop"), "{pane}");
    let node_visible = view.movie.sprite(pane).and_then(|s| s.child("itemImage")).is_some_and(|n| n.is_visible());
    assert!(node_visible, "{pane}.itemImage hidden");
    let icon = view.movie.sprite(&format!("{pane}.itemImage.imageAnm.coImage")).unwrap();
    assert_eq!(icon.gfx.len(), 1, "{pane}: no picture");
    let text = view.movie.text(&format!("{pane}.txHelp.textField")).unwrap_or_default();
    assert!(text.starts_with(help_start), "{pane}: help {text:?}");
}

#[test]
fn item_keys_find_their_pictures_and_help_texts() {
    let Some(view) = load() else { return };
    for id in ["ITEM_POTION", "ITEM_MEGAELIXIR", "ITEM_TENT", "WEP_KEYBLADE_SO_00", "WEP_KEYBLADE_SO_00.upgrade.2", "PRT_ITEM01", "ACC_ITEM01", "MAT_ITEM01", "KEY_ITEM02"] {
        assert!(view.art.icon(id).is_some(), "{id}: no picture");
        assert!(!view.piece_help(id, "", "").is_empty(), "{id}: no help text");
    }
    assert!(view.piece_help("WEP_KEYBLADE_SO_00", "Kingdom Key", "").starts_with("Your very first Keyblade."));
    assert_eq!(view.piece_help("ITEM_POTION", "Potion", "own text"), "own text", "the host's own text first");
    // Elden Ring's own key items: no KH3 picture, no KH3 text (the pane shows nothing for them).
    assert!(view.art.icon("HOSTKEY_0").is_none() && view.piece_help("HOSTKEY_0", "Rusty Key", "").is_empty());
    // (IMPL_ER43) with an icon number: Elden Ring's picture when the pack has tools/er_icons.js's index
    if !view.art.host_icons.is_empty() {
        let icon = view.art.icon("HOSTKEY_0.icon264").expect("Elden Ring's icon 264");
        assert_eq!((icon.width, icon.height), (160, 160));
    }
}

#[test]
fn the_items_pane_shows_the_picture_and_the_text() {
    let Some(mut view) = load() else { return };
    let mut sounds = Vec::new();
    let mut menu = CampMenu::open(false, false, stats(), &mut sounds);
    view.open(&menu);
    frames(&mut view, &mut menu, 50);
    while menu.focused_entry() != Some(TopEntry::Items) {
        menu.key(CampKey::Down, false, &mut sounds);
    }
    menu.key(CampKey::Decide, false, &mut sounds);
    assert_eq!(menu.screen, CampScreen::Items);
    menu.items.set(vec![
        InvEntry { id: "ITEM_POTION".into(), name: "Potion".into(), category: CAT_ITEM, count: 3, usable: true, benefit: true, help: String::new(), benefits: Vec::new() }, // (party P2) F's new field: empty = `benefit` for Sora alone
        InvEntry { id: "HOSTKEY_0".into(), name: "Rusty Key".into(), category: CAT_KEY, count: 1, ..InvEntry::default() },
    ]);
    frames(&mut view, &mut menu, 40);
    assert_pane(&view, "backyard.backyardAnm.itemStatusA1", "Restores");
}

#[test]
fn the_equipment_panes_show_the_keyblade() {
    let Some(mut view) = load() else { return };
    let mut sounds = Vec::new();
    let mut menu = CampMenu::open(false, false, stats(), &mut sounds);
    view.open(&menu);
    frames(&mut view, &mut menu, 50);
    menu.key(CampKey::Decide, false, &mut sounds);
    frames(&mut view, &mut menu, 30);
    menu.key(CampKey::Decide, false, &mut sounds);
    assert_eq!(menu.screen, CampScreen::Equipment);
    let piece = |id: &str, name: &str| Piece { id: id.into(), name: name.into(), kind: KIND_WEAPON, ..Piece::default() };
    menu.equipment.set(EquipIn { slots: vec![EquipSlot { kind: KIND_WEAPON, slot: 0, piece: Some(piece("WEP_KEYBLADE_SO_00", "Kingdom Key")), current: true, ..EquipSlot::default() }], stock: vec![StockEntry { piece: piece("WEP_KEYBLADE_SO_01", "Hero's Origin"), count: 1 }], ap_in_use: 0, stock_max: 99 });
    frames(&mut view, &mut menu, 40);
    assert_pane(&view, "equip.equipDetailAnm.equipDetail.itemStatusA1", "Your very first Keyblade.");
    // The stock list: its own pane, on the highlighted row (row 0 = the piece in the slot).
    menu.key(CampKey::Decide, false, &mut sounds);
    assert_eq!(menu.equipment.layer, EquipLayer::Stock);
    frames(&mut view, &mut menu, 40);
    assert_pane(&view, "equip.equipReplaceAnm.equipReplace.itemStatusA2", "Your very first Keyblade.");
    menu.key(CampKey::Down, false, &mut sounds);
    frames(&mut view, &mut menu, 20);
    let text = view.movie.text("equip.equipReplaceAnm.equipReplace.itemStatusA2.txHelp.textField").unwrap_or_default();
    assert!(!text.is_empty() && !text.starts_with("Your very first"), "the next Keyblade's text: {text:?}");
}
