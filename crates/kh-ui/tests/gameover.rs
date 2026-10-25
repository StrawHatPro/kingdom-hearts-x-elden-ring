//! (game over) The game-over menu (kh-ui `gameover`): the variants of the menu-type table, the hint panel,
//! the light cursor, the results.

use kh_ui::camp::CampKey;
use kh_ui::gameover::{GameOverButton as B, GameOverMenu, GameOverResult as R, GameOverVariant as V, MENU_TYPES};
use kh_ui::{Fonts, Pack};

fn menu() -> Option<GameOverMenu> {
    let mut pack = Pack::open_default().map_err(|_| eprintln!("SKIPPED: no build/sora_pack/ui")).ok()?;
    let fonts = Fonts::load(&mut pack).unwrap();
    Some(GameOverMenu::load(&mut pack, fonts).unwrap())
}

#[test]
fn the_variants_are_the_rows_of_the_games_table() {
    assert_eq!(MENU_TYPES.len(), 16);
    assert_eq!(V::Field.buttons(), [B::Continue, B::Load, B::TitleScreen]);
    assert_eq!(V::Battle.buttons(), [B::Retry, B::PrepareRetry, B::Continue, B::Load]);
    assert_eq!(V::BattleNoPrepare.buttons(), [B::Retry, B::Continue, B::Load]);
    assert_eq!(V::RetryExit.buttons(), [B::Retry, B::Load, B::Exit]);
    assert_eq!(V::RetryLoad.buttons(), [B::Retry, B::Load]);
    assert_eq!(V::GiveUp.buttons(), [B::Retry, B::PrepareRetry, B::GiveUp]);
    // The twin with Game Help, and the table's second Prepare & Retry id.
    assert_eq!(V::Type(3).buttons(), [B::Retry, B::GameHelp, B::PrepareRetry, B::Continue, B::Load]);
    assert_eq!(V::Type(14).buttons(), [B::Retry, B::PrepareRetry2, B::Continue, B::Load]);
    assert_eq!((B::PrepareRetry.result(), B::PrepareRetry2.result(), B::TitleScreen.result()), (R::PrepareAndRetry, R::PrepareAndRetry, R::TitleScreen));
    assert_eq!(B::PrepareRetry2.text_key(), B::PrepareRetry.text_key());
}

#[test]
fn the_battle_menu_shows_four_entries_the_hint_and_the_light_cursor() {
    let Some(mut menu) = menu() else { return };
    let mut sounds = Vec::new();
    menu.open(V::Battle, Some("Airstep to the surrounding rubble to quickly close in on the<br>Tornado Titan."));
    let m = &menu.movie;
    let name = |n: usize| m.text(&format!("gameOverMenu.btnSet.coBtn{n}.btnName.coName.textField")).unwrap_or_default();
    assert_eq!([name(1), name(2), name(3), name(4)], ["Retry", "Prepare & Retry", "Continue", "Load"]);
    assert!(!m.node("gameOverMenu.btnSet.coBtn5").unwrap().is_visible());
    assert_eq!(m.text("gameOverMenu.txTitle.textField").as_deref(), Some("MENU"));
    // `<br>` is a line break; the block of two lines is centred in the 940 x 129 field (TextFieldAlign):
    // x = (940 - 730.4) / 2, y = (129 - (2 x 23 + 11)) / 2 = 36 from the field's corner.
    assert_eq!(m.text("gameOverMenu.hintText.coText.textField").as_deref(), Some("Airstep to the surrounding rubble to quickly close in on the\nTornado Titan."));
    let field = m.node("gameOverMenu.hintText.coText.textField").unwrap();
    assert!((field.matrix.ty - (2.0 + 36.0)).abs() < 0.01, "{}", field.matrix.ty);
    assert!((field.matrix.tx - 161.2 - 104.8).abs() < 1.5, "{}", field.matrix.tx);
    // The default cursor is the first button; the light cursor sits on its anchor's top right corner.
    assert_eq!((menu.cursor(), m.current_label("gameOverMenu.btnSet.coBtn1")), (0, Some("select")));
    assert_eq!(menu.light_cursor(), (556.05, 776.0));
    assert!(m.node("lightCursor").is_some_and(|n| n.is_visible() && n.matrix.a == m.node("lightCursor").unwrap().matrix.d && n.matrix.a < 1.0));
    // Down: the cursor slides to the second button in 0.3 s (18 frames), slowly at first.
    menu.key(CampKey::Down, false, &mut sounds);
    menu.tick();
    let after_one = menu.light_cursor().1 - 776.0;
    assert!(after_one > 0.0 && after_one < 50.0 / 18.0, "{after_one}");
    for _ in 0..17 {
        menu.tick();
    }
    assert_eq!((menu.cursor(), menu.light_cursor()), (1, (556.05, 826.0)));
    // Decide: Prepare & Retry, once, after the decide animation; the menu is then off.
    menu.key(CampKey::Decide, false, &mut sounds);
    assert!(menu.deciding());
    let chosen = (0..40).find_map(|_| menu.tick());
    assert_eq!((chosen.map(B::result), menu.is_open()), (Some(R::PrepareAndRetry), false));
}

#[test]
fn no_hint_keeps_the_panel_and_a_disabled_button_refuses() {
    let Some(mut menu) = menu() else { return };
    let mut sounds = Vec::new();
    menu.open(V::Field, None);
    assert!(!menu.movie.node("gameOverMenu.hintText").unwrap().is_visible());
    // (the panel `base` with the heart emblem is part of the clip: it stays, as in the game)
    assert!(menu.movie.node("gameOverMenu.base").is_some());
    menu.close();
    menu.open_type(0, &[B::Continue], "");
    // The cursor starts on the first button the host can carry out.
    assert_eq!((menu.cursor(), menu.buttons()[menu.cursor()]), (1, B::Load));
    menu.key(CampKey::Up, false, &mut sounds);
    sounds.clear();
    menu.key(CampKey::Decide, false, &mut sounds);
    assert_eq!((menu.deciding(), sounds.as_slice()), (false, [kh_ui::UiSound::Play(kh_ui::menu::SE_MENU_REFUSED)].as_slice()));
}
