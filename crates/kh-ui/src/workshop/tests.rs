//! (weapons P6b) The Workshop's pad flows on plain data (no pack): every screen, KH3's rules of each key.

use super::*;

fn need(key: &str, need: u32, stock: u32) -> MatNeed {
    MatNeed { key: key.into(), name: key.into(), need, stock }
}

fn recipe(id: &str, tab: u8, max: u32, new: bool) -> RecipeRow {
    RecipeRow { id: id.into(), key: format!("ITEM_{id}"), name: id.into(), category: 1, tab, held: 0, max, made: 0, new, materials: vec![need("MAT_ITEM01", 2, 10)], help: String::new() }
}

fn blade(key: &str, level: u8, ready: bool) -> ForgeRow {
    let next = (level < 10).then(|| ForgeNext { level: level + 1, strength: 5, magic: 4, ability: None, materials: vec![need("MAT_ITEM40", 1, u32::from(ready))], ready });
    ForgeRow { key: key.into(), name: key.into(), level, max_level: 10, strength: 4, magic: 3, next }
}

fn data() -> WorkshopData {
    WorkshopData {
        title: "Roundtable Hold".into(),
        munny: 100,
        commands: Screen::MOOGLE.iter().map(|s| (*s, *s == Screen::Synthesis)).collect(),
        log: LogRow::default(),
        recipes: vec![recipe("IS_0", 1, 5, true), recipe("IS_12", 2, 1, false), recipe("IS_20", 3, 0, true), recipe("IS_30", 5, 3, false)],
        keyblades: vec![blade("KB_A", 0, true), blade("KB_B", 3, false), blade("KB_C", 10, false)],
        photos: vec![PhotoRow { id: "IM_1".into(), text: "kupo".into(), help: "help".into(), done: false, new: true, reward_key: "ACC_1".into(), reward_name: "Ring".into() }, PhotoRow { id: "IM_2".into(), new: true, ..PhotoRow::default() }],
        goals: (0..12).map(|k| GoalRow { id: format!("IC_{k}"), text: format!("goal {k}"), ..GoalRow::default() }).collect(),
        materials: (0..60).map(|k| MaterialRow { key: format!("MAT_ITEM{k:02}"), name: "? ? ?".into(), ..MaterialRow::default() }).collect(),
    }
}

fn press(menu: &mut WorkshopMenu, keys: &[CampKey]) -> Vec<UiSound> {
    let mut s = Vec::new();
    for k in keys {
        menu.key(*k, false, &mut s);
    }
    s
}

use CampKey::*;

/// The Moogle: the top first; the entry dialogs before anything (A each; dialog_0030's SE 10); up / down wrap on a fresh
/// press and stop held; A enters, B from a screen comes back to its command, B on the top closes.
#[test]
fn top_dialogs_cursor_enter_back_close() {
    let mut m = WorkshopMenu::new(data(), Entry::Workshop);
    assert_eq!(m.screen, Screen::Top);
    let mut s = Vec::new();
    m.push_messages([Message { text: "left here".into(), sound: None }, Message { text: "hands over".into(), sound: Some(SE_ITEM_GET) }], &mut s);
    assert!(s.is_empty());
    let s = press(&mut m, &[Down]);
    assert_eq!((m.top, s.len()), (0, 0), "the dialog has the pad");
    let s = press(&mut m, &[Decide]);
    assert_eq!(s, vec![UiSound::Play(SE_MENU_DECIDE), UiSound::Play(SE_ITEM_GET)], "the next dialog's own sound");
    press(&mut m, &[Decide]);
    assert!(m.message().is_none());
    press(&mut m, &[Up]);
    assert_eq!(m.top, 3, "a fresh press wraps (the Moogle's four commands)");
    let mut s = Vec::new();
    m.key(Down, true, &mut s);
    assert_eq!(m.top, 3, "held it stops");
    press(&mut m, &[Down, Down, Decide]);
    assert_eq!((m.screen, m.leaving), (Screen::Photo, Some(Screen::Top)));
    press(&mut m, &[Cancel]);
    assert_eq!((m.screen, m.top, m.leaving), (Screen::Top, 1, Some(Screen::Photo)), "back on Journey to the Erdtree's command");
    assert_eq!(m.take_events(), vec![WorkshopEvent::SeenPhoto("IM_1".into())], "the first mission seen");
    let s = press(&mut m, &[Cancel]);
    assert_eq!((m.take_events(), s), (vec![WorkshopEvent::Closed], vec![UiSound::Play(SE_MENU_CLOSE)]));
}

/// THE MOOGLE DOES NOT FORGE (the user, 2026-11-07: "that is what the blacksmiths are for"): even when the host sends KH3's
/// five commands, the Moogle's list holds four and never reaches the Forge; a smith's menu still opens on the Forge.
#[test]
fn the_moogle_has_no_forge() {
    let mut five = data();
    five.commands = Screen::COMMANDS.iter().map(|s| (*s, false)).collect();
    let mut m = WorkshopMenu::new(five.clone(), Entry::Workshop);
    assert_eq!(m.data.commands.iter().map(|c| c.0).collect::<Vec<_>>(), Screen::MOOGLE);
    m.set_data(five.clone());
    assert!(m.data.commands.iter().all(|c| c.0 != Screen::Forge), "the host's data again: still no Forge");
    for _ in 0..8 {
        press(&mut m, &[Down]);
        assert_ne!(m.data.commands[m.top].0, Screen::Forge);
    }
    for k in 0..4 {
        let mut m = WorkshopMenu::new(five.clone(), Entry::Workshop);
        press(&mut m, &vec![Down; k]);
        press(&mut m, &[Decide]);
        assert_ne!(m.screen, Screen::Forge, "command {k}");
    }
    let smith = WorkshopMenu::new(five, Entry::Forge);
    assert_eq!(smith.screen, Screen::Forge, "a smith still forges");
}

/// Synthesize Items: the NEW mark goes when a row gets the cursor; L2 / R2 the six tabs (wrapping, the Tab sound); left /
/// right the quantity (only when more than one can be made; a fresh press wraps, held it stops); A on a row that cannot be
/// made: Beep; else mesWin2 with the cursor on No; B there does nothing; Yes asks for exactly the count shown.
#[test]
fn synthesis_tabs_quantity_confirm() {
    let mut m = WorkshopMenu::new(data(), Entry::Workshop);
    press(&mut m, &[Decide]);
    assert_eq!(m.screen, Screen::Synthesis);
    assert_eq!(m.take_events(), vec![WorkshopEvent::SeenRecipe("IS_0".into())], "the first row is seen on entering");
    assert!(!m.data.commands[0].1 || m.data.recipes.iter().any(|r| r.new));
    // tabs
    let s = press(&mut m, &[R2]);
    assert_eq!((m.tab, m.recipe_rows().len(), s), (1, 1, vec![UiSound::Play(SE_TAB)]));
    press(&mut m, &[L2, L2]);
    assert_eq!((m.tab, m.recipe_rows()[0].id.as_str()), (5, "IS_30"), "Materials, wrapping left");
    press(&mut m, &[R2]);
    assert_eq!(m.tab, 0);
    // the quantity on the row
    let s = press(&mut m, &[Right, Right]);
    assert_eq!((m.qty_of(m.recipe().unwrap()), s.len()), (3, 2));
    press(&mut m, &[Left, Left, Left]);
    assert_eq!(m.qty_of(m.recipe().unwrap()), 5, "1 -> the most on a fresh press");
    let mut s = Vec::new();
    m.key(Right, true, &mut s);
    assert_eq!((m.qty_of(m.recipe().unwrap()), s.len()), (5, 0), "held it stops");
    press(&mut m, &[Down]);
    assert_eq!(m.recipe().unwrap().id, "IS_12");
    let s = press(&mut m, &[Right]);
    assert!(s.is_empty() && m.qty_of(m.recipe().unwrap()) == 1, "one at most: the quantity stays");
    press(&mut m, &[Down]);
    assert_eq!(m.take_events(), vec![WorkshopEvent::SeenRecipe("IS_20".into())]);
    let s = press(&mut m, &[Decide]);
    assert_eq!((m.layer, s), (Layer::Main, vec![UiSound::Play(SE_MENU_REFUSED)]), "nothing can be made: Beep");
    // back to the first row: confirm with the cursor on No
    press(&mut m, &[Up, Up]);
    press(&mut m, &[Decide]);
    assert_eq!((m.layer, m.confirm), (Layer::Confirm, 1));
    press(&mut m, &[Cancel]);
    assert_eq!(m.layer, Layer::Confirm, "KH3: B does nothing on mesWin2");
    press(&mut m, &[Decide]);
    assert_eq!((m.layer, m.take_events()), (Layer::Main, vec![]), "No: back to the list");
    press(&mut m, &[Decide, Left, Decide]);
    assert_eq!(m.take_events(), vec![WorkshopEvent::Synthesize { id: "IS_0".into(), count: 5 }], "exactly the count shown");
    assert_eq!((m.recipe().unwrap().made, m.recipe().unwrap().held, m.qty_of(m.recipe().unwrap())), (5, 5, 1));
    // L1 / R1 page (the list fits: nothing moves); B back to the top
    press(&mut m, &[Cancel]);
    assert_eq!((m.screen, m.top), (Screen::Top, 0));
}

/// The Keyblade Forge (from a smith): straight on the Forge; left / right through the Keyblades (circular, the Click);
/// A at the highest level: Beep; lacking materials: dialog_0210 (A closes it); else Yes / No (B = No); Yes: the power-up
/// with SE 32, its effects hold the pad until they end; B closes the menu (back to the smith's talk).
#[test]
fn forge_picker_refusals_power_up() {
    let mut m = WorkshopMenu::new(data(), Entry::Forge);
    assert_eq!(m.screen, Screen::Forge);
    let s = press(&mut m, &[Left]);
    assert_eq!((m.keyblade, m.picker_move, s), (2, Some(false), vec![UiSound::Play(SE_MENU_CURSOR)]), "circular");
    let s = press(&mut m, &[Decide]);
    assert_eq!((m.layer, s), (Layer::Main, vec![UiSound::Play(SE_MENU_REFUSED)]), "at +10");
    press(&mut m, &[Right, Right]);
    assert_eq!(m.forge_row().unwrap().key, "KB_B");
    press(&mut m, &[Decide]);
    assert_eq!(m.layer, Layer::Notice, "You lack the needed materials.");
    press(&mut m, &[Down]);
    assert_eq!((m.layer, m.keyblade), (Layer::Notice, 1), "the notice has the pad");
    press(&mut m, &[Decide]);
    assert_eq!(m.layer, Layer::Main);
    press(&mut m, &[Left]);
    press(&mut m, &[Decide]);
    assert_eq!((m.layer, m.confirm), (Layer::Confirm, 0));
    press(&mut m, &[Cancel]);
    assert_eq!(m.layer, Layer::Main, "B on the system Yes / No: No");
    press(&mut m, &[Decide]);
    let s = press(&mut m, &[Decide]);
    assert_eq!(s, vec![UiSound::Play(SE_FORGE)]);
    assert_eq!((m.take_events(), m.enhance, m.layer), (vec![WorkshopEvent::Forge { key: "KB_A".into() }], Some(1), Layer::Effect));
    assert_eq!((m.forge_row().unwrap().level, m.forge_row().unwrap().strength), (1, 5));
    press(&mut m, &[Right, Cancel, Decide]);
    assert_eq!((m.keyblade, m.take_events()), (0, vec![]), "no input while the effects play");
    m.effect_done();
    press(&mut m, &[Cancel]);
    assert_eq!(m.take_events(), vec![WorkshopEvent::Closed], "a smith's Forge closes on B");
}

/// Photo Missions / Collector's Goals: lists (A does nothing), a mission's NEW goes on the cursor; the Material List has
/// no cursor: up / down a line, LB / RB a page, a fresh press wraps.
#[test]
fn photo_goals_materials() {
    let mut m = WorkshopMenu::new(data(), Entry::Workshop);
    press(&mut m, &[Down, Decide]);
    assert_eq!(m.screen, Screen::Photo);
    assert_eq!(m.take_events(), vec![WorkshopEvent::SeenPhoto("IM_1".into())]);
    press(&mut m, &[Decide]);
    assert_eq!(m.layer, Layer::Main);
    press(&mut m, &[Down]);
    assert_eq!(m.take_events(), vec![WorkshopEvent::SeenPhoto("IM_2".into())]);
    assert!(!m.data.commands[1].1, "no NEW mission left: the command's mark goes");
    press(&mut m, &[Cancel, Down, Decide]);
    assert_eq!(m.screen, Screen::Goals);
    press(&mut m, &[R1]);
    assert_eq!((m.goal_row().unwrap().id.as_str(), m.goals.scroll), ("IC_2", 2), "a page (KH3's List: the cursor keeps its place in the window)");
    press(&mut m, &[Cancel, Down, Decide]);
    assert_eq!((m.screen, m.material_line, m.material_line_max()), (Screen::Materials, 0, 20));
    press(&mut m, &[Up]);
    assert_eq!(m.material_line, 20, "a fresh press wraps");
    press(&mut m, &[Down]);
    assert_eq!(m.material_line, 0);
    press(&mut m, &[R1, R1]);
    assert_eq!(m.material_line, 20);
    let mut s = Vec::new();
    m.key(R1, true, &mut s);
    assert_eq!(m.material_line, 20, "held it stops");
    press(&mut m, &[Cancel]);
    assert_eq!((m.screen, m.top), (Screen::Top, 3));
}

/// New data keeps the cursors on their rows (after a synthesis or a power-up the host's rows come back).
#[test]
fn new_data_keeps_the_cursors() {
    let mut m = WorkshopMenu::new(data(), Entry::Forge);
    press(&mut m, &[Right]);
    let mut d = data();
    d.keyblades.insert(0, blade("KB_0", 0, false));
    m.set_data(d);
    assert_eq!(m.forge_row().unwrap().key, "KB_B");
    assert_eq!(picker_offsets(1), vec![0]);
    assert_eq!(picker_offsets(2), vec![0, 1]);
    assert_eq!(picker_offsets(17), vec![-3, -2, -1, 0, 1, 2, 3]);
    assert_eq!((picker_label(0), picker_label(-1), picker_label(3)), ("p0", "p2", "p5"));
    assert_eq!((tab_of("weapon"), tab_of("camp"), tab_of("material")), (2, 1, 5));
}
