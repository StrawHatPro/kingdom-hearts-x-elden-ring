//! (IMPL_ER34, the user: "leaving the equipment menu is glitchy") The Equipment screen's transitions play the movie's
//! animations the right way round: going DEEPER the screen left behind shrinks, going BACK the screen left grows away
//! and the one underneath grows back from 0.75. Skipped without the UI pack.

use kh_ui::camp::{CampKey, CampMenu, CampScreen, StatsIn};
use kh_ui::camp_equip::{EquipIn, EquipLayer, EquipSlot, Piece, StockEntry, KIND_WEAPON};
use kh_ui::camp_menu::{leave_label, CampMenuView, DETAIL_TO_STOCK};

const DETAIL: &str = "equip.equipDetailAnm";
const REPLACE: &str = "equip.equipReplaceAnm";

fn stats() -> StatsIn {
    StatsIn { name: "Sora".into(), level: 9, exp: 100, next_level_exp: 50, level_exp: 80, hp: 100, hp_max: 120, mp: 100, mp_max: 100, focus: 100, focus_max: 100, focus_unlocked: true, ap_max: 20, strength: 10, magic: 10, defense: 10, resists: [0; 6], munny: 0, play_seconds: 60 }
}

fn piece(id: &str) -> Piece {
    Piece { id: id.into(), name: id.into(), kind: KIND_WEAPON, ..Piece::default() }
}

/// The scale and opacity of a clip's child.
fn child(view: &CampMenuView, clip: &str, name: &str) -> (f32, f32, bool) {
    let node = view.movie.sprite(clip).and_then(|s| s.child(name)).unwrap_or_else(|| panic!("{clip}.{name}"));
    (node.matrix.a, node.cx.mul[3], node.is_visible())
}

fn frames(view: &mut CampMenuView, menu: &mut CampMenu, n: usize) {
    for _ in 0..n {
        view.sync(menu);
        view.tick();
    }
}

/// The menu on the Equipment screen's slot list, settled.
fn at_equipment() -> Option<(CampMenuView, CampMenu)> {
    let Ok(mut pack) = kh_ui::Pack::open_default() else {
        eprintln!("SKIPPED: no build/sora_pack/ui");
        return None;
    };
    let fonts = kh_ui::Fonts::load(&mut pack).unwrap();
    let mut view = CampMenuView::load(&mut pack, fonts).unwrap();
    let mut sounds = Vec::new();
    let mut menu = CampMenu::open(false, false, stats(), &mut sounds);
    view.open(&menu);
    frames(&mut view, &mut menu, 50);
    menu.key(CampKey::Decide, false, &mut sounds);
    frames(&mut view, &mut menu, 30);
    menu.key(CampKey::Decide, false, &mut sounds);
    assert_eq!(menu.screen, CampScreen::Equipment);
    menu.equipment.set(EquipIn { slots: vec![EquipSlot { kind: KIND_WEAPON, slot: 0, piece: Some(piece("Kingdom Key")), current: true, ..EquipSlot::default() }], stock: vec![StockEntry { piece: piece("Hero's Origin"), count: 1 }], ap_in_use: 0, stock_max: 99 });
    frames(&mut view, &mut menu, 40);
    assert_eq!(view.movie.current_label(DETAIL), Some("loop"));
    Some((view, menu))
}

#[test]
fn the_labels_are_the_movies_directions() {
    assert_eq!((leave_label(CampScreen::Equipment), DETAIL_TO_STOCK), ("out2", "out"));
    for screen in [CampScreen::Abilities, CampScreen::Items, CampScreen::Customize, CampScreen::Stats, CampScreen::GameHelp, CampScreen::Config] {
        assert_eq!(leave_label(screen), "out", "{screen:?}");
    }
}

/// B on the slot list: the slot list GROWS away (never below its own size) while the top screen grows back from
/// 0.75; one press goes one level (the menu stays open, on the top screen); nothing of the Equipment is left after it.
#[test]
fn leaving_equipment_the_slot_list_grows_away_and_the_top_screen_grows_back() {
    let Some((mut view, mut menu)) = at_equipment() else { return };
    let mut sounds = Vec::new();
    let actions = menu.key(CampKey::Cancel, false, &mut sounds);
    // (DON-UI) one level back (the user's rule for all menus): Equipment was reached through the character list
    assert!(actions.is_empty() && menu.screen == CampScreen::Character(kh_ui::camp::TopEntry::Equipment), "one level back, nothing else: {actions:?}");
    assert_eq!(sounds.len(), 1, "one sound for one press: {sounds:?}");
    let (mut detail, mut top) = (Vec::new(), Vec::new());
    for _ in 0..16 {
        frames(&mut view, &mut menu, 1);
        let (d, t) = (child(&view, DETAIL, "equipDetail"), child(&view, "topMenu", "statusAnm"));
        if d.2 {
            detail.push((d.0, d.1));
        }
        if t.2 {
            top.push((t.0, t.1));
        }
    }
    assert!(detail.len() >= 8 && detail.len() <= 12, "the slot list goes in about ten frames: {detail:?}");
    assert!(detail.iter().all(|(s, _)| *s >= 0.999), "the slot list never shrinks on the way out: {detail:?}");
    assert!(detail.windows(2).all(|w| w[1].0 >= w[0].0 && w[1].1 <= w[0].1), "it grows and fades steadily: {detail:?}");
    assert!(detail.last().unwrap().0 > 1.3 && detail.last().unwrap().1 < 0.3, "{detail:?}");
    let _ = top; // (DON-UI) the character list comes back (KH3's `backChara`), not the top screen
    // settled: the Equipment's clips are off, the character list loops, the menu is still open
    frames(&mut view, &mut menu, 10);
    assert_eq!((view.movie.current_label(DETAIL), view.movie.current_label(REPLACE), view.movie.current_label("topMenu")), (Some("off"), Some("off"), Some("loopChara")));
    assert!(view.is_open());
}

/// A on a slot: the slot list SHRINKS away under the stock list coming in from 1.5; B there: the stock list grows
/// away and the slot list grows back; B again: the top screen (three presses, three levels).
#[test]
fn the_stock_list_opens_deeper_and_closes_back() {
    let Some((mut view, mut menu)) = at_equipment() else { return };
    let mut sounds = Vec::new();
    menu.key(CampKey::Decide, false, &mut sounds);
    assert_eq!(menu.equipment.layer, EquipLayer::Stock);
    frames(&mut view, &mut menu, 5);
    let (detail, replace) = (child(&view, DETAIL, "equipDetail"), child(&view, REPLACE, "equipReplace"));
    assert!(detail.0 < 1.0 && detail.0 > 0.75 && detail.1 < 1.0, "the slot list shrinks away: {detail:?}");
    assert!(replace.0 > 1.0 && replace.1 < 1.0, "the stock list comes in from 1.5: {replace:?}");
    frames(&mut view, &mut menu, 30);
    assert_eq!((view.movie.current_label(DETAIL), view.movie.current_label(REPLACE)), (Some("off"), Some("loop")));
    // back to the slot list
    assert!(menu.key(CampKey::Cancel, false, &mut sounds).is_empty());
    assert_eq!((menu.screen, menu.equipment.layer), (CampScreen::Equipment, EquipLayer::Slots));
    frames(&mut view, &mut menu, 5);
    let (detail, replace) = (child(&view, DETAIL, "equipDetail"), child(&view, REPLACE, "equipReplace"));
    assert!(detail.0 < 1.0 && detail.0 >= 0.75, "the slot list grows back from 0.75: {detail:?}");
    assert!(replace.0 > 1.0, "the stock list grows away: {replace:?}");
    frames(&mut view, &mut menu, 30);
    assert_eq!((view.movie.current_label(DETAIL), view.movie.current_label(REPLACE)), (Some("loop"), Some("off")));
    // and out: (DON-UI) one level, the character list, then the top screen
    assert!(menu.key(CampKey::Cancel, false, &mut sounds).is_empty());
    assert_eq!(menu.screen, CampScreen::Character(kh_ui::camp::TopEntry::Equipment));
    assert!(menu.key(CampKey::Cancel, false, &mut sounds).is_empty());
    assert_eq!(menu.screen, CampScreen::Top);
}

/// (DON-UI) THE USER'S RULE FOR ALL MENUS (2026-10-22): "when you press B it should bring you back to the previous menu,
/// not to the origin menu". B on the Equipment's slot list goes back to the character list (the movie's own `backChara`,
/// the list stays with its cursor), B there to the top screen (`outChara2`). Pictures: build/sora_tmp/party/DON-UI/shots/.
#[test]
fn b_from_equipment_goes_back_to_the_character_list_then_the_top() {
    let Some((mut view, mut menu)) = at_equipment() else { return };
    let fonts = kh_ui::Fonts::load(&mut kh_ui::Pack::open_default().unwrap()).unwrap();
    let shot = |view: &CampMenuView, name: &str| {
        let mut canvas = kh_ui::Canvas::new(1920, 1080);
        let (root, _) = (kh_ui::geom::Matrix::new(1.0, 0.0, 0.0, 1.0, 0.0, 0.0), ());
        kh_ui::Renderer::new(fonts.clone()).draw(&mut canvas, &view.movie, &root);
        let mut rgba = Vec::new();
        canvas.to_rgba8_over(|_, _| [0.1, 0.1, 0.12], &mut rgba);
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/party/DON-UI/shots");
        std::fs::create_dir_all(&dir).ok();
        kh_ui::def::write_png(&dir.join(format!("{name}.png")), 1920, 1080, &rgba).unwrap();
    };
    shot(&view, "20_camp_equipment");
    let mut sounds = Vec::new();
    assert!(menu.key(CampKey::Cancel, false, &mut sounds).is_empty());
    assert_eq!(menu.screen, CampScreen::Character(kh_ui::camp::TopEntry::Equipment), "one level back: the character list");
    frames(&mut view, &mut menu, 40);
    println!("after B: topMenu {:?}", view.movie.current_label("topMenu"));
    shot(&view, "21_camp_b_character_list");
    assert!(menu.key(CampKey::Cancel, false, &mut sounds).is_empty());
    assert_eq!(menu.screen, CampScreen::Top, "one more: the top screen");
    frames(&mut view, &mut menu, 40);
    println!("after B B: topMenu {:?}", view.movie.current_label("topMenu"));
    shot(&view, "22_camp_b_b_top");
    assert_eq!(view.movie.current_label("topMenu"), Some("loop"));
    assert!(view.is_open());
}
