//! (ASH-ROWS) THE WORKSHOP'S SYSTEM WINDOW LAID OUT AS KH3'S `Dialog.setText` DOES (the FINAL round: "A 3-line message
//! ('...: Hungry Shard' wrapped) overlaps the dialog's OK row; the box does not grow with the text"). The window is as wide
//! as the text's widest line, the text wraps only past the largest window, and the window grows down with every further
//! line, the OK row under the text's last line. Skipped without the pack. Pictures: build/sora_tmp/weapons/ASH-ROWS/shots/.

use kh_ui::camp::CampKey;
use kh_ui::workshop::view::WorkshopView;
use kh_ui::workshop::*;
use kh_ui::player::{Kind, Node};
use kh_ui::{Fonts, Matrix, Movie};

fn load() -> Option<(WorkshopView, Fonts)> {
    let Ok(mut pack) = kh_ui::Pack::open_default() else {
        eprintln!("SKIPPED: no build/sora_pack/ui");
        return None;
    };
    let fonts = Fonts::load(&mut pack).unwrap();
    WorkshopView::load(&mut pack, fonts.clone()).ok().map(|v| (v, fonts))
}

fn run(view: &mut WorkshopView, menu: &mut WorkshopMenu, n: usize) {
    for _ in 0..n {
        view.sync(menu);
        view.tick(Some(menu));
    }
}

fn shot(view: &WorkshopView, fonts: &Fonts, name: &str) {
    let mut canvas = kh_ui::Canvas::new(1920, 1080);
    let mut renderer = kh_ui::Renderer::new(fonts.clone());
    renderer.draw(&mut canvas, &view.movie, &Matrix::IDENTITY);
    if let Some(dialog) = view.dialog_movie() {
        renderer.draw(&mut canvas, dialog, &Matrix::IDENTITY);
    }
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|_, _| [0.16, 0.3, 0.2], &mut rgba);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/weapons/ASH-ROWS/shots");
    std::fs::create_dir_all(&dir).ok();
    kh_ui::def::write_png(&dir.join(format!("{name}.png")), 1920, 1080, &rgba).unwrap();
}

/// The stage matrix of a path (the product of its nodes' matrices).
fn stage(movie: &Movie, path: &str) -> Matrix {
    let mut m = Matrix::IDENTITY;
    let mut at = String::new();
    for part in path.split('.') {
        at = if at.is_empty() { part.to_string() } else { format!("{at}.{part}") };
        m = m.mul(&movie.node(&at).unwrap().matrix);
    }
    m
}

/// What the window shows: (the text's lines as drawn, its top and bottom on the stage, the OK row's top and bottom, the
/// window's top / bottom / left / right).
struct Shown {
    lines: usize,
    text: (f32, f32),
    ok: (f32, f32),
    window: (f32, f32, f32, f32),
}

fn shown(view: &WorkshopView, fonts: &Fonts) -> Shown {
    let d = &view.dialog;
    let field = "dialogText.textField";
    let Some(Node { kind: Kind::Text(t), .. }) = d.node(field) else { panic!("no dialog text") };
    let font = fonts.get(t.def.font_class.as_deref()).unwrap();
    let width = t.adjust.map_or(t.def.bounds.x1 - t.def.bounds.x0, |a| a.2);
    let lines = kh_ui::render::wrap_text(fonts, font, t.shown(), t.def.size, width - 4.0).lines().count();
    let m = stage(d, field);
    let top = m.apply(0.0, t.def.bounds.y0 + 2.0).1;
    let bottom = top + kh_ui::newgame::text_height(font, t.def.size, t.def.leading, lines);
    // the OK row: its `anchor` (the button's 24 px high box, 6 below its origin)
    let ok = stage(d, "dialogButton").apply(0.0, 6.0).1;
    // the window: the frame's shape (-127 .. 127 x -63 .. 63) under its scaling
    let w = stage(d, "dialog.dialogAnm.dialogWin.dialogWin2");
    let (x0, y0) = w.apply(-127.0, -63.0);
    let (x1, y1) = w.apply(127.0, 63.0);
    Shown { lines, text: (top, bottom), ok: (ok, ok + 24.0), window: (y0, y1, x0, x1) }
}

#[test]
fn ash_rows_the_system_window_grows_with_its_text_and_the_ok_row_stays_under_it() {
    let Some((mut view, fonts)) = load() else { return };
    let mut menu = WorkshopMenu::new(WorkshopData { title: "Roundtable Hold".into(), ..WorkshopData::default() }, Entry::Workshop);
    let mut s = Vec::new();
    view.open(&mut menu);
    let lines = |id: &str| view.string(id);
    let deposit = lines("kh3_menu.synthesis.dialog_0010");
    let synth = lines("kh3_menu.synthesis.dialog_0020");
    let handed = kh_ui::workshop::texts::lines(&view.raw("kh3_menu.synthesis.dialog_0030").replace("{0}", "Hungry Shard"));
    // past the largest window (1024): the field wraps, the window grows down
    let long = format!("{}<br>{}", "Collector's Goal complete!", ["The moogle hands over an item: <color(strong)>Hungry Shard<color>"; 3].join(" and then "));
    let long = kh_ui::workshop::texts::lines(&long);
    menu.push_messages([deposit, synth, handed, long].into_iter().map(|text| Message { text, sound: None }), &mut s);
    let mut seen = Vec::new();
    for (k, name) in ["1_deposit_two_lines", "2_unlocked_items", "3_handed_hungry_shard", "4_wraps_past_the_largest_window"].into_iter().enumerate() {
        run(&mut view, &mut menu, 40);
        assert!(view.dialog_movie().is_some(), "{name}: the dialog is up");
        let at = shown(&view, &fonts);
        println!("{name}: lines {} text {:?} ok {:?} window {:?}", at.lines, at.text, at.ok, at.window);
        shot(&view, &fonts, &format!("dialog_{name}"));
        // the OK row under the text's last line, inside the window; the text inside the window
        assert!(at.ok.0 >= at.text.1, "{name}: the OK row ({:?}) under the text ({:?})", at.ok, at.text);
        assert!(at.ok.1 <= at.window.1 && at.text.0 >= at.window.0, "{name}: text {:?} / OK {:?} in the window {:?}", at.text, at.ok, at.window);
        seen.push(at);
        if k < 3 {
            menu.key(CampKey::Decide, false, &mut s);
            run(&mut view, &mut menu, 2);
        }
    }
    // KH3's own texts fit their window: two lines each (the reported dialog: no third line any more)
    assert!(seen[..3].iter().all(|a| a.lines == 2), "{:?}", seen.iter().map(|a| a.lines).collect::<Vec<_>>());
    // the authored two-line window: the OK row where the movie puts it (`poDialogButton`, 115 under the holder)
    let authored = seen[0].ok.0 - seen[0].text.0;
    assert!((authored - (115.0 + 6.0 - 40.0 - 2.0)).abs() < 6.0, "the OK row of a two-line text near the authored place: {authored}");
    // a wider text, a wider window, centred on the same middle
    let width = |a: &Shown| a.window.3 - a.window.2;
    assert!(width(&seen[2]) > width(&seen[0]) - 1.0, "{} vs {}", width(&seen[2]), width(&seen[0]));
    assert!(((seen[2].window.2 + seen[2].window.3) - (seen[0].window.2 + seen[0].window.3)).abs() < 2.0, "centred");
    // past the largest window: more lines, the window and the OK row move down by the lines it grew
    let wrapped = &seen[3];
    assert!(wrapped.lines >= 3, "it wraps: {}", wrapped.lines);
    assert!(width(wrapped) <= 1024.0 + 2.0 + 1.0, "the largest window: {}", width(wrapped));
    let grown = (wrapped.ok.0 - wrapped.text.0) - authored;
    let line = (wrapped.text.1 - wrapped.text.0 - (seen[0].text.1 - seen[0].text.0)) / (wrapped.lines - 2) as f32;
    assert!((grown - line * (wrapped.lines - 2) as f32).abs() < 1.0, "the OK row moved down by the extra lines: {grown} vs {line} x {}", wrapped.lines - 2);
    assert!((wrapped.window.1 - wrapped.window.0) - (seen[0].window.1 - seen[0].window.0) > grown - 1.0, "the window grew with it");
}
