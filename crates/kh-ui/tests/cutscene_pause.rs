//! (cutscene) The cutscene pause menu (kh-ui `cutscene_pause`): the Pause movie's button set of menu type 1.
//! Pictures: build/sora_tmp/er/cutscene/t_*.png (KH_UI_SHOTS names another folder).

use std::path::Path;

use kh_ui::camp::CampKey;
use kh_ui::cutscene_pause::{menu_object_y, CutscenePauseMenu, CutscenePauseResult};
use kh_ui::def::write_png;
use kh_ui::menu::{SE_MENU_CLOSE, SE_MENU_CURSOR, SE_MENU_DECIDE, SE_MENU_OPEN, SE_MENU_REFUSED};
use kh_ui::{Canvas, Fonts, Matrix, Pack, Renderer, UiSound};

fn menu() -> Option<(CutscenePauseMenu, Fonts)> {
    let mut pack = Pack::open_default().map_err(|_| eprintln!("SKIPPED: no build/sora_pack/ui")).ok()?;
    let fonts = Fonts::load(&mut pack).unwrap();
    Some((CutscenePauseMenu::load(&mut pack, fonts.clone()).unwrap(), fonts))
}

fn shot(name: &str, menu: &CutscenePauseMenu, fonts: &Fonts) {
    let dir = match std::env::var_os("KH_UI_SHOTS") {
        Some(dir) => std::path::PathBuf::from(dir),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/er/cutscene"),
    };
    std::fs::create_dir_all(&dir).ok();
    let mut canvas = Canvas::new(1920, 1080);
    Renderer::new(fonts.clone()).draw(&mut canvas, &menu.movie, &Matrix::IDENTITY);
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|x, y| [0.25 + 0.25 * (x as f32 / 1920.0), 0.45, 0.30 + 0.3 * (y as f32 / 1080.0)], &mut rgba);
    write_png(&dir.join(format!("t_{name}.png")), 1920, 1080, &rgba).unwrap();
}

/// Ticks until a result comes (at most `limit` frames).
fn run_out(menu: &mut CutscenePauseMenu, limit: usize) -> Option<(CutscenePauseResult, usize)> {
    (1..=limit).find_map(|n| menu.tick().map(|r| (r, n)))
}

#[test]
fn opens_as_the_game_does_and_the_keys_count_after_the_open_animation() {
    let Some((mut menu, fonts)) = menu() else { return };
    let mut sounds = Vec::new();
    menu.open(true, &mut sounds);
    assert_eq!(sounds, [UiSound::Play(SE_MENU_OPEN)]);
    assert!(menu.is_open() && !menu.accepts_input());
    // A key before the open animation's end does nothing.
    menu.key(CampKey::Down, false, &mut sounds);
    assert_eq!((menu.cursor(), sounds.len()), (0, 1));
    let mut frames = 0;
    while !menu.accepts_input() && frames < 60 {
        assert_eq!(menu.tick(), None);
        frames += 1;
    }
    // (the button set's `in` label is frame 5, its END_PAUSE_ANIMATION_OPEN script frame 13)
    assert!((6..=12).contains(&frames), "input after {frames} frames");
    for _ in 0..30 {
        menu.tick();
    }
    // The game's texts, two buttons, the rest hidden; the menu object moved up by 12 + 25 x 2.
    assert_eq!((menu.button_text(0).as_str(), menu.button_text(1).as_str()), ("Continue", "Skip Scene"));
    let m = &menu.movie;
    assert_eq!(m.text("pauseMenu.menuObj.btnSet.btnSetObj.coBtn1.btnName.coName.textField").as_deref(), Some("Continue"));
    assert_eq!(m.text("pauseMenu.menuObj.btnSet.btnSetObj.coBtn2.btnName.coName.textField").as_deref(), Some("Skip Scene"));
    for n in 1..=6 {
        assert_eq!(m.node(&format!("pauseMenu.menuObj.btnSet.btnSetObj.coBtn{n}")).unwrap().is_visible(), n <= 2, "coBtn{n}");
    }
    assert_eq!(menu_object_y(2), -62.0);
    assert_eq!(m.node("pauseMenu.menuObj").unwrap().matrix.ty, -62.0);
    assert_eq!(m.current_label("pauseMenu.menuObj.btnSet.btnSetObj.coBtn1"), Some("select"));
    assert_eq!(m.current_label("pauseMenu.menuObj.btnSet.btnSetObj.coBtn2"), Some("normal"));
    assert_eq!(m.current_label("pauseMenu.menuObj.btnSet.btnSetObj.coBtn1.cursor"), Some("loop"));
    assert!(!m.node("gameOverMenu").unwrap().is_visible());
    shot("open_continue", &menu, &fonts);
}

#[test]
fn cursor_wraps_on_a_fresh_press_only_and_skip_scene_is_told_after_the_way_out() {
    let Some((mut menu, fonts)) = menu() else { return };
    let mut sounds = Vec::new();
    menu.open(true, &mut sounds);
    for _ in 0..40 {
        menu.tick();
    }
    sounds.clear();
    menu.key(CampKey::Down, false, &mut sounds);
    assert_eq!((menu.cursor(), sounds.as_slice()), (1, &[UiSound::Play(SE_MENU_CURSOR)][..]));
    // Held: stops at the end. Fresh: wraps.
    menu.key(CampKey::Down, true, &mut sounds);
    assert_eq!((menu.cursor(), sounds.len()), (1, 1));
    menu.key(CampKey::Down, false, &mut sounds);
    assert_eq!(menu.cursor(), 0);
    menu.key(CampKey::Up, true, &mut sounds);
    assert_eq!(menu.cursor(), 0);
    menu.key(CampKey::Up, false, &mut sounds);
    assert_eq!(menu.cursor(), 1);
    for _ in 0..20 {
        menu.tick();
    }
    assert_eq!(menu.movie.current_label("pauseMenu.menuObj.btnSet.btnSetObj.coBtn2"), Some("select"));
    assert_eq!(menu.movie.current_label("pauseMenu.menuObj.btnSet.btnSetObj.coBtn1"), Some("normal"));
    shot("cursor_skip", &menu, &fonts);
    sounds.clear();
    menu.key(CampKey::Decide, false, &mut sounds);
    assert_eq!(sounds, [UiSound::Play(SE_MENU_DECIDE)]);
    assert!(menu.is_open() && !menu.accepts_input(), "the way out runs; no more keys");
    menu.key(CampKey::Up, false, &mut sounds);
    assert_eq!(menu.cursor(), 1);
    menu.tick();
    menu.tick();
    menu.tick();
    shot("leaving", &menu, &fonts);
    // (the button set's `out` label is frame 33, END_PAUSE_ANIMATION_CLOSE frame 43)
    let (result, frames) = run_out(&mut menu, 60).expect("a result");
    assert_eq!(result, CutscenePauseResult::SkipScene);
    assert!((5..=12).contains(&(frames + 3)), "told after {} frames", frames + 3);
    assert!(!menu.is_open());
    assert_eq!(menu.tick(), None, "told once");
}

#[test]
fn continue_and_the_menu_button_resume_cancel_is_ignored_and_a_scene_that_cannot_be_skipped_refuses() {
    let Some((mut menu, fonts)) = menu() else { return };
    for (key, sound) in [(CampKey::Decide, SE_MENU_DECIDE), (CampKey::Menu, SE_MENU_CLOSE)] {
        let mut sounds = Vec::new();
        menu.open(true, &mut sounds);
        for _ in 0..40 {
            menu.tick();
        }
        sounds.clear();
        menu.key(key, false, &mut sounds);
        assert_eq!(sounds, [UiSound::Play(sound)], "{key:?}");
        assert_eq!(run_out(&mut menu, 60).map(|r| r.0), Some(CutscenePauseResult::Resume), "{key:?}");
        assert!(!menu.is_open());
    }
    // The cancel key does nothing, as in the game: no sound, the menu stays.
    let mut sounds = Vec::new();
    menu.open(true, &mut sounds);
    for _ in 0..40 {
        menu.tick();
    }
    sounds.clear();
    menu.key(CampKey::Cancel, false, &mut sounds);
    assert!(sounds.is_empty());
    assert_eq!(run_out(&mut menu, 30).map(|r| r.0), None);
    assert!(menu.is_open() && menu.accepts_input());
    menu.close();
    // Skip Scene disabled: shown with the disable labels, decide on it is refused; the menu stays.
    let mut sounds = Vec::new();
    menu.open(false, &mut sounds);
    for _ in 0..40 {
        menu.tick();
    }
    assert_eq!(menu.movie.current_label("pauseMenu.menuObj.btnSet.btnSetObj.coBtn2"), Some("disable"));
    menu.key(CampKey::Down, false, &mut sounds);
    sounds.clear();
    menu.key(CampKey::Decide, false, &mut sounds);
    assert_eq!(sounds, [UiSound::Play(SE_MENU_REFUSED)]);
    assert!(menu.accepts_input());
    for _ in 0..20 {
        menu.tick();
    }
    assert_eq!(menu.movie.current_label("pauseMenu.menuObj.btnSet.btnSetObj.coBtn2"), Some("disableSelect"));
    shot("skip_disabled", &menu, &fonts);
    // `close` takes it off at once, without a result.
    menu.close();
    assert!(!menu.is_open());
    assert_eq!(menu.tick(), None);
}
