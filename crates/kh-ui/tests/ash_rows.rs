//! (ASH-ROWS, THE USER 2026-11-05: "For the ash summons, make the bubble dynamically extend to show the whole name (only for
//! spirit ashes)") THE SUMMON LIST'S SPIRIT-ASH ROWS GROW WITH THEIR NAMES: the whole name, a space, the count; every ash
//! row of the list as wide as the widest (one column of plates and counts, as KH3's lists); the plates' rounded ends
//! unstretched; the cursor's highlight as wide; every other list (and a numbered row that is not an ash) as before.
//! Skipped without the pack. Pictures: build/sora_tmp/weapons/ASH-ROWS/shots/rows_*.png (the bottom-left corner, 2x).

use std::path::Path;

use kh_ui::command::{CommandDisplay, CommandModel, CommandStrings, MenuRow, MenuState};
use kh_ui::def::write_png;
use kh_ui::hud_command::HudCommand;
use kh_ui::player::Kind;
use kh_ui::{Canvas, Fonts, Matrix, Movie, Pack, Renderer, UiSound};

const DT: f32 = 1.0 / 60.0;
/// The row clips' authored number field place (`cmdText1.cmdNumber` x) and plate end (the dark pill's 246).
const NUMBER_X: f32 = 180.0;
const PLATE_END: f32 = 246.0;

fn pack() -> Option<Pack> {
    match Pack::open_default() {
        Ok(pack) => Some(pack),
        Err(_) => {
            eprintln!("SKIPPED: no build/sora_pack/ui");
            None
        }
    }
}

fn render(movie: &Movie, fonts: &Fonts) -> Canvas {
    let mut canvas = Canvas::new(1920, 1080);
    Renderer::new(fonts.clone()).draw(&mut canvas, movie, &Matrix::IDENTITY);
    canvas
}

fn shot(name: &str, canvas: &Canvas) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/weapons/ASH-ROWS/shots");
    std::fs::create_dir_all(&dir).ok();
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|_, y| [0.22 + 0.1 * (y as f32 / 1080.0), 0.2, 0.18], &mut rgba);
    write_png(&dir.join(format!("rows_{name}_full.png")), 1920, 1080, &rgba).unwrap();
    let (x0, y0, w, h, k) = (0usize, 600usize, 960usize, 480usize, 2usize);
    let mut zoom = vec![0u8; w * k * h * k * 4];
    for y in 0..h * k {
        for x in 0..w * k {
            let s = ((y0 + y / k) * 1920 + x0 + x / k) * 4;
            let d = (y * w * k + x) * 4;
            zoom[d..d + 4].copy_from_slice(&rgba[s..s + 4]);
        }
    }
    write_png(&dir.join(format!("rows_{name}.png")), (w * k) as u32, (h * k) as u32, &zoom).unwrap();
}

/// A row of a list: an ash (its number set, the flag), an item (a quantity), a spell.
fn ash(name: &str, number: &str) -> MenuRow {
    MenuRow { text: name.to_string(), enabled: true, number: Some(number.to_string()), spirit_ash: true, ..MenuRow::default() }
}

fn item(name: &str, quantity: u32) -> MenuRow {
    MenuRow { text: name.to_string(), enabled: true, quantity, ..MenuRow::default() }
}

fn model(state: MenuState, list: usize, rows: Vec<MenuRow>, cursor: usize) -> CommandModel {
    let mut m = CommandModel { state, main_cursor: list, list, row_cursor: cursor, ..CommandModel::default() };
    for (slot, text) in ["Attack", "Magic", "Items", "Summon"].into_iter().enumerate() {
        m.main[slot] = MenuRow::new(text);
    }
    m.lists[list] = rows;
    m
}

struct Menu {
    hud: HudCommand,
    display: CommandDisplay,
    strings: CommandStrings,
    sounds: Vec<UiSound>,
}

impl Menu {
    fn new(pack: &mut Pack, fonts: &Fonts) -> Menu {
        Menu { hud: HudCommand::new(pack.movie("HudCommand").unwrap(), fonts.clone()), display: CommandDisplay::new(), strings: CommandStrings::load(pack), sounds: Vec::new() }
    }

    fn run(&mut self, model: &CommandModel, n: usize) {
        for _ in 0..n {
            self.display.update(&mut self.hud, model, &self.strings, DT, &mut self.sounds);
            self.hud.tick();
        }
    }

    /// Opens `list` with `rows` from the main list (the menu shown first), the cursor on `cursor`.
    fn open(&mut self, list: usize, rows: Vec<MenuRow>, cursor: usize) {
        self.run(&model(MenuState::MainSel, list, Vec::new(), 0), 60);
        self.run(&model(MenuState::SubSel, list, rows, cursor), 60);
    }

    fn close(&mut self, list: usize) {
        self.run(&model(MenuState::MainSel, list, Vec::new(), 0), 60);
    }
}

/// What a shown row says: (the name shown, its x scale, the number, how much wider, the number field's x).
#[derive(Debug)]
struct Row {
    name: String,
    squeeze: f32,
    number: String,
    by: f32,
    /// How far the number moved (the name's growth).
    number_by: f32,
    number_x: f32,
    /// The name's right end and the number text's left edge, in the row's units.
    name_end: f32,
    number_left: f32,
}

fn row(menu: &Menu, fonts: &Fonts, list: usize, r: usize) -> Row {
    let m = &menu.hud.movie;
    let clip = menu.hud.sub_row_clip(list as i32, r);
    let name_path = format!("{clip}.cmdText1.txCmdName.txCmdNameCol.coCmdName.textField");
    let Some(Kind::Text(t)) = m.node(&name_path).map(|n| &n.kind) else { panic!("no name field") };
    let squeeze = t.adjust.map_or(1.0, |a| a.0);
    let font = fonts.get(t.def.font_class.as_deref()).unwrap();
    // cmdText1 (18) + coCmdName's field at 2, its 2 px gutter
    let name_end = 18.0 + 2.0 + t.def.bounds.x0 + 2.0 + font.text_width(t.shown(), t.def.size) * squeeze;
    let number_x = m.node(&format!("{clip}.cmdText1.cmdNumber")).unwrap().matrix.tx;
    // cmdText1 (18) + cmdNumber + the text field's 28.5 and its left edge -28.5 + 2 px gutter
    let number_left = 18.0 + number_x + 2.0;
    Row { name: t.shown().to_string(), squeeze, number: m.text(&format!("{clip}.cmdText1.cmdNumber.cmdNumCol.txCmdNumber.cmdNumberTxt.textField")).unwrap_or_default(), by: menu.hud.sub_row_width(list as i32, r), number_by: menu.hud.sub_row_number_shift(list as i32, r), number_x, name_end, number_left }
}

/// The stage place of a point of a row's plate (`cmdSubBase1`: the cursor's row slides it right).
fn stage(menu: &Menu, list: usize, r: usize, x: f32, y: f32) -> (f32, f32) {
    let path = format!("{}.cmdSubBase1", menu.hud.sub_row_clip(list as i32, r));
    let mut m = Matrix::IDENTITY;
    let mut at = String::new();
    for part in path.split('.') {
        at = if at.is_empty() { part.to_string() } else { format!("{at}.{part}") };
        m = m.mul(&menu.hud.movie.node(&at).unwrap().matrix);
    }
    m.apply(x, y)
}

fn alpha(canvas: &Canvas, (x, y): (f32, f32)) -> f32 {
    canvas.px[(y.round() as usize * canvas.w + x.round() as usize) * 4 + 3]
}

#[test]
fn ash_rows_widen_to_show_the_whole_name_the_list_takes_the_widest() {
    let Some(mut pack) = pack() else { return };
    let fonts = Fonts::load(&mut pack).unwrap();
    let mut menu = Menu::new(&mut pack, &fonts);
    // Elden Ring's longest spirit-ash names at +10, the Mimic Tear's "All"
    let names = [("Black Knife Tiche +10", "66"), ("Mimic Tear Ashes +10", "All"), ("Lhutel the Headless +10", "40"), ("Nightmaiden & Swordstress Puppets +10", "112")];
    menu.open(3, names.iter().map(|(n, c)| ash(n, c)).collect(), 3);
    let rows: Vec<Row> = (0..4).map(|r| row(&menu, &fonts, 3, r)).collect();
    for (r, (name, count)) in rows.iter().zip(names) {
        assert_eq!((r.name.as_str(), r.number.as_str()), (name, count), "the whole name (its \" +10\" too) and the count: {r:?}");
        assert_eq!(r.squeeze, 1.0, "no TextFieldAdjust squeeze: the name fits its wider field: {r:?}");
        assert!(r.name_end <= r.number_left - 3.0, "the name ends a space before the count: {r:?}");
        assert!((r.number_x - (NUMBER_X + r.number_by)).abs() < 1e-3 && r.number_by > 0.0, "the count moves with the name: {r:?}");
    }
    let widest = rows.iter().map(|r| r.by).fold(0.0, f32::max);
    assert!(widest > 100.0 && rows.iter().all(|r| r.by == widest && r.number_by == rows[0].number_by), "one width for the list (the widest name's): {rows:?}");
    // the three-digit cost: the plate reaches past the count as far as past an "x3" (the field's right end + its gutter)
    assert!(widest > rows[0].number_by, "the plate grows by the count's overrun too: {rows:?}");
    // the longest name: its count a space after it (the plate no wider than it needs)
    let long = &rows[3];
    assert!(long.number_left - long.name_end < 12.0, "{long:?}");
    // the plates as drawn: the row's end grows by `by`, the rounded end cap as authored; the cursor's highlight too
    let canvas = render(&menu.hud.movie, &fonts);
    shot("1_longest_er_names_cursor_on_the_longest", &canvas);
    for r in 0..4 {
        let inside = stage(&menu, 3, r, PLATE_END + widest - 8.0, 16.0);
        let outside = stage(&menu, 3, r, PLATE_END + widest + 8.0, 16.0);
        assert!(alpha(&canvas, inside) > 0.5 && alpha(&canvas, outside) < 0.05, "row {r}: the plate reaches its new end ({} / {})", alpha(&canvas, inside), alpha(&canvas, outside));
    }
    let clip = menu.hud.sub_row_clip(3, 3);
    assert_eq!(menu.hud.movie.current_label(&format!("{clip}.cmdSubBaseSel")), Some("over"), "the cursor's highlight on the row");
    assert_eq!(menu.hud.movie.sprite(&format!("{clip}.cmdSubBaseSel")).and_then(|s| s.stretch).map(|s| s.by), Some(widest), "the highlight as wide as the plate");
    // the cursor moved: the highlight's slide on the wider plates
    menu.run(&model(MenuState::SubSel, 3, names.iter().map(|(n, c)| ash(n, c)).collect(), 0), 4);
    shot("2_cursor_moving_mid_slide", &render(&menu.hud.movie, &fonts));
    menu.run(&model(MenuState::SubSel, 3, names.iter().map(|(n, c)| ash(n, c)).collect(), 0), 40);
    shot("3_cursor_on_the_first", &render(&menu.hud.movie, &fonts));
    assert_eq!(rows.iter().map(|r| r.by).collect::<Vec<_>>(), (0..4).map(|r| menu.hud.sub_row_width(3, r)).collect::<Vec<_>>(), "the widths stay");
}

#[test]
fn ash_rows_short_names_stay_authored_and_other_lists_keep_their_rows() {
    let Some(mut pack) = pack() else { return };
    let fonts = Fonts::load(&mut pack).unwrap();
    let mut menu = Menu::new(&mut pack, &fonts);
    // short ash names fit the authored plate: nothing changes
    menu.open(3, vec![ash("Jellyfish Ashes", "6"), ash("Mimic Tear", "All")], 0);
    for r in 0..2 {
        let row = row(&menu, &fonts, 3, r);
        assert_eq!((row.by, row.number_by, row.number_x), (0.0, 0.0, NUMBER_X), "{row:?}");
    }
    assert_eq!(row(&menu, &fonts, 3, 0).name, "Jellyfish Ashes");
    shot("4_short_names_authored", &render(&menu.hud.movie, &fonts));
    menu.close(3);
    // a wide Summon list, then Items on the same row clips: Items' rows as authored, "x3" in its place
    menu.open(3, vec![ash("Nightmaiden & Swordstress Puppets +10", "112"), ash("Lone Wolf Ashes +3", "9")], 0);
    assert!(menu.hud.sub_row_width(3, 0) > 100.0);
    menu.close(3);
    menu.open(2, vec![item("Potion", 3), item("Hi-Potion", 1), item("Ether", 12)], 0);
    for r in 0..3 {
        let row = row(&menu, &fonts, 2, r);
        assert_eq!((row.by, row.number_by, row.number_x), (0.0, 0.0, NUMBER_X), "Items row {r}: {row:?}");
    }
    for clip in 1..=9 {
        let path = format!("cmdMenu.cmdSubTop.cmdSub1Set.cmdSub1_{clip}");
        for part in ["@1", "cmdSubBase1", "cmdSubBaseSel"] {
            assert!(menu.hud.movie.sprite(&format!("{path}.{part}")).is_some_and(|s| s.stretch.is_none()), "{path}.{part} back to its authored width");
        }
    }
    assert_eq!((row(&menu, &fonts, 2, 0).name.as_str(), row(&menu, &fonts, 2, 0).number.as_str()), ("Potion", "x3"));
    shot("5_items_after_summon_authored", &render(&menu.hud.movie, &fonts));
    menu.close(2);
    // a numbered row that is not a spirit ash keeps the cut before its number (today's rule)
    let other = MenuRow { spirit_ash: false, ..ash("Black Knife Tiche +5", "66") };
    menu.open(3, vec![other], 0);
    let r = row(&menu, &fonts, 3, 0);
    assert_eq!((r.name.as_str(), r.by), ("Black Knife... +5", 0.0), "{r:?}");
    menu.close(3);
    // Magic: the authored rows
    menu.open(1, vec![item("Fire", 0), item("Blizzard", 0), item("Thunder", 0)], 1);
    for r in 0..3 {
        assert_eq!(row(&menu, &fonts, 1, r).by, 0.0);
    }
    shot("6_magic_authored", &render(&menu.hud.movie, &fonts));
}

#[test]
fn ash_rows_a_name_past_the_largest_plate_is_cut_before_its_count() {
    let Some(mut pack) = pack() else { return };
    let fonts = Fonts::load(&mut pack).unwrap();
    let mut menu = Menu::new(&mut pack, &fonts);
    let huge = "Nightmaiden & Swordstress Puppets and Their Many Many Many Friends +10";
    menu.open(3, vec![ash(huge, "112"), ash("Black Knife Tiche +10", "66")], 0);
    let r = row(&menu, &fonts, 3, 0);
    assert_eq!(r.number_by, kh_ui::command::ASH_ROW_MAX_EXTRA, "{r:?}");
    assert!(r.name.ends_with("... +10") && r.name_end <= r.number_left - 3.0, "cut, its level whole, before the count: {r:?}");
    assert_eq!(row(&menu, &fonts, 3, 1).name, "Black Knife Tiche +10");
    shot("7_past_the_largest_plate_cut", &render(&menu.hud.movie, &fonts));
}
