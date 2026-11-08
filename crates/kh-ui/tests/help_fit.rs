//! (THE USER 2026-11-03: "the textbox for item descriptions in our KH camp menu needs to accommodate text so that it is
//! never cut off") THE SHARED FIT RULE ON THE CAMP MENU'S PANES (`kh_ui::render::fit_box` through
//! `Movie::set_text_fit`): a description longer than KH3's three lines shrinks, and one longer still scrolls with the right
//! stick (THE USER 2026-11-03: "right stick scroll"; `Movie::scroll_fits`, KH3's `RstickUp` / `RstickDown` marks beside
//! it), every word shown and every line inside the field. The Items screen's item status pane and the screen's help band, with the
//! longest real texts of the pack (KH3's Shotlock help page, as it is and run on as one paragraph the way Elden Ring's
//! captions arrive: `kh_er::loot::flowing`). Pictures before (IMPL_ER42's cut) and after: build/sora_tmp/help_fit/*.png.
//! Skipped without the UI pack. (The user, 2026-11-03: "no need to test it that rigorously": a few focused cases.)

use kh_ui::camp::{CampKey, CampMenu, CampScreen, StatsIn, TopEntry};
use kh_ui::camp_equip::{InvEntry, CAT_KEY};
use kh_ui::camp_menu::CampMenuView;
use kh_ui::render::{fit_box, fit_text, markup_width, BoxFit, FIT_MIN_PX};

const PANE: &str = "backyard.backyardAnm.itemStatusA1.txHelp.textField";
const BAND: &str = "backyard.helpText.txHelpText.textField";

fn stats() -> StatsIn {
    StatsIn { name: "Sora".into(), level: 9, exp: 100, next_level_exp: 50, level_exp: 80, hp: 100, hp_max: 120, mp: 100, mp_max: 100, focus: 100, focus_max: 100, focus_unlocked: true, ap_max: 20, strength: 10, magic: 10, defense: 10, resists: [0; 6], munny: 0, play_seconds: 60 }
}

fn frames(view: &mut CampMenuView, menu: &mut CampMenu, n: usize) {
    for _ in 0..n {
        view.sync(menu);
        view.tick();
    }
}

fn out_dir() -> std::path::PathBuf {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/help_fit");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn shoot(view: &CampMenuView, fonts: &kh_ui::Fonts, name: &str) {
    let mut canvas = kh_ui::Canvas::new(1920, 1080);
    kh_ui::Renderer::new(fonts.clone()).draw(&mut canvas, &view.movie, &kh_ui::geom::Matrix::new(1.0, 0.0, 0.0, 1.0, 0.0, 0.0));
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|_, _| [0.12, 0.13, 0.16], &mut rgba);
    kh_ui::def::write_png(&out_dir().join(format!("{name}.png")), 1920, 1080, &rgba).unwrap();
}

/// The words of a marked-up text (colour spans and line breaks aside: a wrapped line opens its span again): what a
/// reader must see, in order.
fn words(text: &str) -> Vec<String> {
    let mut plain = String::new();
    let mut rest = text;
    while let Some(at) = rest.find("<color") {
        plain.push_str(&rest[..at]);
        rest = rest[at..].find('>').map_or("", |end| &rest[at + end + 1..]);
    }
    plain.push_str(rest);
    plain.split(['\n', ' ']).filter(|w| !w.is_empty()).map(str::to_string).collect()
}

/// The camp menu on its Items screen, one key item focused that carries `help`.
fn items_screen(help: &str) -> Option<(CampMenuView, CampMenu, kh_ui::Fonts, std::collections::HashMap<String, String>)> {
    let Ok(mut pack) = kh_ui::Pack::open_default() else {
        eprintln!("SKIPPED: no build/sora_pack/ui");
        return None;
    };
    let fonts = kh_ui::Fonts::load(&mut pack).unwrap();
    let strings = pack.string_table("kh3_gamehelp");
    let mut view = CampMenuView::load(&mut pack, fonts.clone()).unwrap();
    let mut sounds = Vec::new();
    let mut menu = CampMenu::open(false, false, stats(), &mut sounds);
    view.open(&menu);
    frames(&mut view, &mut menu, 50);
    while menu.focused_entry() != Some(TopEntry::Items) {
        menu.key(CampKey::Down, false, &mut sounds);
    }
    menu.key(CampKey::Decide, false, &mut sounds);
    assert_eq!(menu.screen, CampScreen::Items);
    menu.items.set(vec![InvEntry { id: "HOSTKEY_0".into(), name: "Rusty Key".into(), category: CAT_KEY, count: 1, help: help.to_string(), ..InvEntry::default() }]);
    frames(&mut view, &mut menu, 40);
    Some((view, menu, fonts, strings))
}

/// Every line of the fit inside the field's width, at no less than the readable minimum, every word of `text` in it.
fn assert_whole(view: &CampMenuView, fonts: &kh_ui::Fonts, path: &str, text: &str) -> BoxFit {
    let Some(kh_ui::player::Kind::Text(t)) = view.movie.node(path).map(|n| &n.kind) else { panic!("{path}: no field") };
    let font = fonts.get(t.def.font_class.as_deref()).unwrap();
    let fit = view.movie.text_fit(path).cloned().unwrap_or_else(|| fit_box(fonts, &t.def, text));
    let box_w = t.def.bounds.x1 - t.def.bounds.x0 - 4.0;
    for line in &fit.lines {
        assert!(markup_width(fonts, font, line, fit.size) <= box_w + 0.01, "{path}: a line wider than the field at {} px: {line:?}", fit.size);
    }
    assert!(fit.size >= FIT_MIN_PX - 0.01, "{path}: {} px is under the readable minimum", fit.size);
    assert_eq!(words(&fit.lines.join("\n")), words(text), "{path}: every word, in order");
    assert_eq!(view.movie.text_whole(path).as_deref(), Some(text), "{path}: the field holds the whole text");
    fit
}

/// The field's more-above / more-below marks (KH3's `RstickUp` / `RstickDown` icons beside it) that are up.
fn marks(view: &CampMenuView, path: &str) -> (bool, bool) {
    (view.movie.exists(&format!("{path}~fitMoreUp")), view.movie.exists(&format!("{path}~fitMoreDown")))
}

/// (THE USER 2026-11-03: "right stick scroll") A scrolling field: it starts at the top (more below, nothing above); one
/// right-stick step a line moves the window down through every line, never more lines than the field shows, and stops
/// at the end (more above, nothing below); the stick up brings it back to the top. Moving the window does not wait for
/// a frame of the view (the screen's own sync leaves it where it is).
fn assert_scrolls_through(view: &mut CampMenuView, menu: &mut CampMenu, path: &str, fit: &BoxFit) {
    assert_eq!((view.movie.fit_top(path), marks(view, path)), (0, (false, true)), "{path}: a new text starts at its top");
    let mut seen = vec![false; fit.lines.len()];
    for step in 0..=fit.scroll_max() + 2 {
        let shown = view.movie.text(path).unwrap_or_default();
        let lines: Vec<&str> = shown.split('\n').collect();
        assert!(lines.len() <= fit.rows, "{path}: {} lines in a field of {}", lines.len(), fit.rows);
        let top = view.movie.fit_top(path);
        assert_eq!(top, step.min(fit.scroll_max()), "{path}: one line a step, then the end");
        assert_eq!(lines[0], fit.lines[top]);
        for k in 0..lines.len() {
            seen[top + k] = true;
        }
        view.movie.scroll_fits(1);
        frames(view, menu, 1);
    }
    assert!(seen.iter().all(|s| *s), "{path}: lines never shown: {seen:?}");
    assert_eq!(marks(view, path), (true, false), "{path}: at the end only the more-above mark");
    assert!(!view.movie.scroll_fits(1), "{path}: no further than the last line");
    view.movie.scroll_fits(-1000);
    frames(view, menu, 1);
    assert_eq!((view.movie.fit_top(path), marks(view, path)), (0, (false, true)), "{path}: the stick up: back at the top");
}

#[test]
fn the_items_pane_shows_a_long_description_whole() {
    let Some((mut view, mut menu, fonts, strings)) = items_screen("Used to unlock doors.") else { return };
    // KH3's longest help page (796 characters, icons and colour spans), as it is and run on as Elden Ring's captions are
    let page = strings.get("kh3_gamehelp.system.detail_01100_00").expect("the Shotlock help page").replace("<br>", "\n");
    let flowing = page.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ");
    // a description of four lines at the pane's 23 px: KH3's Zero EXP warning run on, and a Customize text's last line
    let four = "Are you sure you want to install the <color(strong)>Zero EXP<color> ability? Sora will <color(strong)>remain at his current level<color>, making the game much more challenging. <color(strong)>Requires a full MP gauge and uses all MP.<color>";
    let Some(kh_ui::player::Kind::Text(t)) = view.movie.node(PANE).map(|n| &n.kind) else { panic!("no pane field") };
    let authored = t.def.clone();
    for (name, text) in [("four_lines", four.to_string()), ("page", page.clone()), ("flowing", flowing.clone())] {
        // BEFORE: IMPL_ER42 cut it to the pane's lines and "..."
        view.movie.set_text(PANE, &fit_text(&fonts, &authored, &text), &fonts);
        shoot(&view, &fonts, &format!("items_{name}_before"));
        // AFTER: the row's text through the view (the shared fit rule)
        menu.items.set(vec![InvEntry { id: "HOSTKEY_0".into(), name: "Rusty Key".into(), category: CAT_KEY, count: 1, help: text.clone(), ..InvEntry::default() }]);
        frames(&mut view, &mut menu, 20);
        let fit = assert_whole(&view, &fonts, PANE, &text);
        eprintln!("{name}: {} characters -> {} lines at {} px, {} shown at once", text.len(), fit.lines.len(), fit.size, fit.rows);
        shoot(&view, &fonts, &format!("items_{name}_after"));
        if name == "four_lines" {
            assert!(fit.whole() && fit.size < authored.size, "four lines fit by shrinking, no scroll: {fit:?}");
        } else {
            assert!(!fit.whole(), "{name}: longer than the pane at the smallest size: it scrolls");
            assert_scrolls_through(&mut view, &mut menu, PANE, &fit);
            // the window in the middle (both marks) and at the end of the text
            view.movie.scroll_fits(2);
            frames(&mut view, &mut menu, 1);
            assert_eq!(marks(&view, PANE), (true, true));
            shoot(&view, &fonts, &format!("items_{name}_after_middle"));
            view.movie.scroll_fits(1000);
            frames(&mut view, &mut menu, 1);
            shoot(&view, &fonts, &format!("items_{name}_after_end"));
        }
    }
    // a short text again: the field's own size and text
    menu.items.set(vec![InvEntry { id: "HOSTKEY_2".into(), name: "Rusty Key".into(), category: CAT_KEY, count: 1, help: "Used to unlock doors.".into(), ..InvEntry::default() }]);
    frames(&mut view, &mut menu, 10);
    let Some(kh_ui::player::Kind::Text(t)) = view.movie.node(PANE).map(|n| &n.kind) else { panic!("no pane field") };
    assert_eq!((t.def.size, t.shown()), (authored.size, "Used to unlock doors."), "a text that fits is drawn as authored");
    assert!(view.movie.text_fit(PANE).is_none());
}

#[test]
fn the_help_band_shows_a_long_help_whole() {
    let Some((mut view, _menu, fonts, strings)) = items_screen("Used to unlock doors.") else { return };
    let page = strings.get("kh3_gamehelp.system.detail_01100_00").expect("the Shotlock help page").replace("<br>", "\n");
    let Some(kh_ui::player::Kind::Text(t)) = view.movie.node(BAND).map(|n| &n.kind) else { panic!("no band field") };
    let authored = t.def.clone();
    view.movie.set_text(BAND, &page, &fonts);
    shoot(&view, &fonts, "band_page_before");
    // the page straight into the band, as a screen's help line is set (`CampMenuView::help_line`); the movie alone ticks
    // from here (the screen fills its own line again only when its rows change)
    view.movie.set_text_fit(BAND, &page, &fonts);
    let fit = assert_whole(&view, &fonts, BAND, &page);
    eprintln!("band: {} lines at {} px, {} shown at once (authored {} px)", fit.lines.len(), fit.size, fit.rows, authored.size);
    shoot(&view, &fonts, "band_page_after");
    assert!(!fit.whole());
    assert_eq!(marks(&view, BAND), (false, true), "more below");
    // the right stick down three lines
    for _ in 0..3 {
        view.movie.scroll_fits(1);
        view.movie.tick();
    }
    assert_eq!((view.movie.fit_top(BAND), marks(&view, BAND)), (3, (true, true)));
    assert_eq!(view.movie.text(BAND).unwrap_or_default(), fit.window(3));
    shoot(&view, &fonts, "band_page_after_scrolled");
    // another text: its top again
    view.movie.set_text_fit(BAND, &format!("{page}\nOne line more."), &fonts);
    assert_eq!((view.movie.fit_top(BAND), marks(&view, BAND)), (0, (false, true)), "a new text starts at its top");
}
