//! Tier 3: the Pause screen and the camp menu's display (docs/sora-ui-spec.md 10, 11b tests
//! 10, 11, 16). Pictures: build/sora_tmp/hud/t3_*.png.

use std::path::Path;

use kh_ui::camp::{CampAction, CampKey, CampMenu, CampScreen, StatsIn};
use kh_ui::camp_menu::CampMenuView;
use kh_ui::def::write_png;
use kh_ui::menu::PauseScreen;
use kh_ui::{Canvas, Fonts, Matrix, Movie, Pack, Renderer, UiSound};

fn pack() -> Option<Pack> {
    Pack::open_default().map_err(|_| eprintln!("SKIPPED: no build/sora_pack/ui")).ok()
}

fn shot(name: &str, movie: &Movie, fonts: &Fonts) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/hud");
    std::fs::create_dir_all(&dir).ok();
    let mut canvas = Canvas::new(1920, 1080);
    Renderer::new(fonts.clone()).draw(&mut canvas, movie, &Matrix::IDENTITY);
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|_, y| [0.30 + 0.1 * (y as f32 / 1080.0), 0.38, 0.46], &mut rgba);
    write_png(&dir.join(format!("t3_{name}.png")), 1920, 1080, &rgba).unwrap();
}

fn stats_99() -> StatsIn {
    StatsIn { name: "Sora".into(), level: 99, exp: 1_527_640, next_level_exp: 0, level_exp: 1_527_640, hp: 265, hp_max: 265, mp: 169, mp_max: 169, focus: 100, focus_max: 100, focus_unlocked: true, ap_max: 180, strength: 66, magic: 67, defense: 59, resists: [0; 6], munny: 1234, play_seconds: 3725 }
}

#[test]
fn pause_screen_opens_and_closes() {
    let Some(mut pack) = pack() else { return };
    let fonts = Fonts::load(&mut pack).unwrap();
    let mut pause = PauseScreen::new(pack.movie("Pause").unwrap(), fonts.clone());
    assert!(pause.movie.def.unknown_statements.is_empty(), "{:?}", pause.movie.def.unknown_statements);
    let mut sounds = Vec::new();
    pause.open(&mut sounds);
    assert_eq!(sounds, vec![UiSound::Play(2)]);
    for _ in 0..40 {
        assert!(!pause.tick());
    }
    shot("pause", &pause.movie, &fonts);
    assert_eq!(pause.movie.current_label("pauseMenu.menuObj.title"), Some("loop"));
    // Menu again: static SE 3, then the close animation reports its end.
    pause.close(&mut sounds);
    assert_eq!(sounds, vec![UiSound::Play(2), UiSound::Play(3)]);
    let mut frames = 0;
    while !pause.tick() {
        frames += 1;
        assert!(frames < 120, "the close animation never ended");
    }
    assert!(!pause.is_open());
    assert!(pause.movie.misses.is_empty(), "paths not found: {:?}", pause.movie.misses);
}

#[test]
fn camp_menu_top_and_stats_screens() {
    let Some(mut pack) = pack() else { return };
    let fonts = Fonts::load(&mut pack).unwrap();
    let mut view = CampMenuView::load(&mut pack, fonts.clone()).unwrap();
    let mut sounds = Vec::new();
    let mut menu = CampMenu::open(false, false, stats_99(), &mut sounds);
    view.open(&menu);
    for _ in 0..60 {
        view.tick();
        view.sync(&mut menu);
    }
    shot("camp_top", &view.movie, &fonts);
    let button = |n: usize| format!("topMenu.cmdBtnTopMenu.topMenu{n}");
    assert_eq!(view.movie.text(&format!("{}.mainCmdNameAnm.mainCmdName", button(1))).as_deref(), Some("Equipment"));
    assert_eq!(view.movie.text(&format!("{}.mainCmdNameAnm.mainCmdName", button(7))).as_deref(), Some("Config"));
    assert!(view.movie.node(&button(7)).unwrap().is_visible() && !view.movie.node(&button(8)).unwrap().is_visible(), "7 rows");
    assert_eq!(view.movie.current_label(&button(1)), Some("select"));
    assert_eq!(view.movie.text("topMenu.statusAnm.status.charaWindow01.lv").as_deref(), Some("99"));
    assert_eq!(view.movie.current_frame("topMenu.statusAnm.status.charaWindow01.expGg"), Some(1));
    // Down four times: Stats; decide twice (character list, then the screen).
    for _ in 0..4 {
        menu.key(CampKey::Down, false, &mut sounds);
        view.sync(&mut menu);
        view.tick();
    }
    assert_eq!(view.movie.current_label(&button(5)), Some("select"));
    menu.key(CampKey::Decide, false, &mut sounds);
    assert_eq!(menu.key(CampKey::Decide, false, &mut sounds), vec![CampAction::Opened(CampScreen::Stats)]);
    for _ in 0..60 {
        view.sync(&mut menu);
        view.tick();
    }
    shot("camp_stats", &view.movie, &fonts);
    assert_eq!(view.movie.text("status.statusAnm.statusList1.expNum").as_deref(), Some("1527640"));
    assert_eq!(view.movie.current_frame("status.statusAnm.statusList1.statusExpGg"), Some(101));
    assert_eq!(view.movie.text("status.statusAnm.statusList3.fireNum").as_deref(), Some("0%"));
    // Cancel back (DON-UI: one level each, the character list, then the top screen), then cancel: close; the animation
    // reports its end.
    menu.key(CampKey::Cancel, false, &mut sounds);
    for _ in 0..40 {
        view.sync(&mut menu);
        view.tick();
    }
    menu.key(CampKey::Cancel, false, &mut sounds);
    for _ in 0..40 {
        view.sync(&mut menu);
        view.tick();
    }
    assert_eq!(menu.key(CampKey::Cancel, false, &mut sounds), vec![CampAction::Close]);
    view.close();
    let mut frames = 0;
    while !view.tick() {
        frames += 1;
        assert!(frames < 200, "the close animation never ended");
    }
    assert!(!view.is_open());
    println!("camp menu paths not found: {:?}", view.movie.misses);
}
