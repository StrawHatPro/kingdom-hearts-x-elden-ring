//! (saveload) The Save and Load screens (kh-ui `saveload`): the slot lists of the movie SaveLoad.
//! Pictures: build/sora_tmp/er/saveload/t_*.png (KH_UI_SHOTS names another folder).

use std::path::Path;

use kh_ui::camp::CampKey;
use kh_ui::def::write_png;
use kh_ui::menu::{SE_MENU_CURSOR, SE_MENU_DECIDE, SE_MENU_REFUSED};
use kh_ui::saveload::{play_time_text, Phase, Portrait, Progress, SaveLoadResult, SaveLoadScreen, SlotMode, SlotState, SlotSummary, MESSAGE_MIN_FRAMES, SE_SAVE_DONE};
use kh_ui::{Canvas, Fonts, Matrix, Pack, Renderer, UiSound};

fn screen() -> Option<(SaveLoadScreen, Fonts)> {
    let mut pack = Pack::open_default().map_err(|_| eprintln!("SKIPPED: no build/sora_pack/ui")).ok()?;
    let fonts = Fonts::load(&mut pack).unwrap();
    Some((SaveLoadScreen::load(&mut pack, fonts.clone()).unwrap(), fonts))
}

fn shot(name: &str, screen: &SaveLoadScreen, fonts: &Fonts) {
    let dir = match std::env::var_os("KH_UI_SHOTS") {
        Some(dir) => std::path::PathBuf::from(dir),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/er/saveload"),
    };
    std::fs::create_dir_all(&dir).ok();
    let mut canvas = Canvas::new(1920, 1080);
    let mut renderer = Renderer::new(fonts.clone());
    renderer.draw(&mut canvas, &screen.movie, &Matrix::IDENTITY);
    if let Some(dialog) = screen.dialog_movie() {
        renderer.draw(&mut canvas, dialog, &Matrix::IDENTITY);
    }
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|x, y| [0.25 + 0.25 * (x as f32 / 1920.0), 0.45, 0.30 + 0.3 * (y as f32 / 1080.0)], &mut rgba);
    write_png(&dir.join(format!("t_{name}.png")), 1920, 1080, &rgba).unwrap();
}

/// Ten slots: 0, 1 and 6 used.
fn slots() -> Vec<SlotSummary> {
    (0..10)
        .map(|index| match index {
            0 => SlotSummary { index, state: SlotState::Used, name: "Sora".into(), level: 12, munny: 1234, play_time_seconds: 3 * 3600 + 7 * 60 + 9, location: "Stranded Graveyard".into(), world: None, mode: SlotMode::Difficulty(1), clear: false, is_current: false, portrait: Portrait::Sora },
            1 => SlotSummary { index, state: SlotState::Used, name: "Sora".into(), level: 99, munny: 999999, play_time_seconds: 123 * 3600 + 59 * 60, location: "The Neighborhood".into(), world: Some("tt".into()), mode: SlotMode::Difficulty(3), clear: true, is_current: true, portrait: Portrait::SoraSecond },
            6 => SlotSummary { index, state: SlotState::Used, name: "Sora".into(), level: 1, munny: 0, play_time_seconds: 59, location: "Church of Elleh".into(), world: None, mode: SlotMode::Text("Journey 2".into()), clear: false, is_current: false, portrait: Portrait::Sora },
            _ => SlotSummary::empty(index),
        })
        .collect()
}

fn settle(screen: &mut SaveLoadScreen, sounds: &mut Vec<UiSound>, frames: usize) -> Option<SaveLoadResult> {
    (0..frames).find_map(|_| screen.tick(sounds))
}

#[test]
fn the_play_time_is_hours_and_two_digit_minutes() {
    assert_eq!(play_time_text(0), "0:00");
    assert_eq!(play_time_text(3 * 3600 + 7 * 60 + 59), "3:07");
    assert_eq!(play_time_text(123 * 3600 + 59 * 60), "123:59");
}

#[test]
fn the_save_list_shows_the_rows_and_saves_an_empty_slot_at_once() {
    let Some((mut screen, fonts)) = screen() else { return };
    let mut sounds = Vec::new();
    screen.open_save(&slots(), false, false, &mut sounds);
    assert_eq!(screen.phase(), Phase::Opening);
    // The cursor starts on the current slot; keys count after the way in.
    assert_eq!(screen.cursor(), 1);
    assert_eq!(screen.key(CampKey::Down, false, &mut sounds), None);
    assert_eq!(screen.cursor(), 1);
    assert_eq!(settle(&mut screen, &mut sounds, 60), None);
    assert_eq!(screen.phase(), Phase::List);
    let m = &screen.movie;
    let list = "menuSaveData.saveDataObj.coList.container";
    assert_eq!(m.text("menuSaveData.stageCategory.category.categoryName.txName").as_deref(), Some("SAVE"));
    assert_eq!(m.text("menuSaveData.saveDataObj.helpText.txHelpText.textField").as_deref(), Some("Select a slot to save to."));
    assert_eq!(m.text(&format!("{list}.i0.txNum.textField")).as_deref(), Some("01"));
    assert_eq!(m.text(&format!("{list}.i0.dataLabel.dataInfo.txNumLv")).as_deref(), Some("12"));
    assert_eq!(m.text(&format!("{list}.i0.dataLabel.dataInfo.txNumMunny")).as_deref(), Some("1234"));
    assert_eq!(m.text(&format!("{list}.i0.dataLabel.dataInfo.txPlayTime")).as_deref(), Some("3:07"));
    assert_eq!(m.text(&format!("{list}.i0.dataLabel.dataInfo.txAreaName")).as_deref(), Some("Stranded Graveyard"));
    assert_eq!(m.text(&format!("{list}.i0.dataLabel.dataInfo.txDifficulty")).as_deref(), Some("Standard"));
    assert_eq!(m.text(&format!("{list}.i1.dataLabel.dataInfo.txDifficulty")).as_deref(), Some("Critical"));
    assert_eq!(m.current_label(&format!("{list}.i1")), Some("select"));
    assert_eq!(m.current_label(&format!("{list}.i0.dataLabel")), Some("info"));
    assert_eq!(m.current_label(&format!("{list}.i2.dataLabel")), Some("empty"));
    assert_eq!(m.text(&format!("{list}.i2.dataLabel.txDataEmpty")).as_deref(), Some("NO DATA"));
    assert!(!m.node("menuSavePoint").unwrap().is_visible());
    shot("save_list", &screen, &fonts);
    // Down to the empty slot 3: decide saves at once (no question), after the row's decide animation.
    sounds.clear();
    screen.key(CampKey::Down, false, &mut sounds);
    assert_eq!((screen.cursor(), sounds.as_slice()), (2, [UiSound::Play(SE_MENU_CURSOR)].as_slice()));
    assert_eq!(screen.key(CampKey::Decide, false, &mut sounds), None);
    assert_eq!(screen.phase(), Phase::Deciding);
    assert_eq!(settle(&mut screen, &mut sounds, 30), Some(SaveLoadResult::Save(2)));
    assert_eq!(screen.phase(), Phase::Busy);
    assert_eq!(screen.message().as_deref(), Some("Saving...
Don't turn off your PC at this time."));
    settle(&mut screen, &mut sounds, 20);
    shot("save_busy", &screen, &fonts);
    // The host is done at once: the message still stays its minimum, then the sound and the list again.
    let mut new = slots();
    new[2] = SlotSummary { index: 2, state: SlotState::Used, level: 7, munny: 55, play_time_seconds: 600, location: "Gatefront".into(), is_current: true, ..SlotSummary::default() };
    new[1].is_current = false;
    screen.set_slots(&new);
    screen.set_progress(Progress::Done);
    sounds.clear();
    assert_eq!(settle(&mut screen, &mut sounds, MESSAGE_MIN_FRAMES as usize + 5), None);
    assert_eq!((screen.phase(), screen.cursor()), (Phase::List, 2));
    assert_eq!(sounds, [UiSound::Play(SE_SAVE_DONE)]);
    settle(&mut screen, &mut sounds, 30);
    assert_eq!(screen.movie.text(&format!("{list}.i2.dataLabel.dataInfo.txAreaName")).as_deref(), Some("Gatefront"));
    assert_eq!(screen.dialog_movie().map(|_| ()), None);
    shot("save_done", &screen, &fonts);
    // Cancel: the way out, then the result.
    assert_eq!(screen.key(CampKey::Cancel, false, &mut sounds), None);
    assert_eq!(settle(&mut screen, &mut sounds, 30), Some(SaveLoadResult::Cancel));
    assert!(!screen.is_open());
}

#[test]
fn a_used_slot_asks_before_it_is_overwritten() {
    let Some((mut screen, fonts)) = screen() else { return };
    let mut sounds = Vec::new();
    screen.open_save(&slots(), true, false, &mut sounds);
    settle(&mut screen, &mut sounds, 60);
    assert_eq!(screen.phase(), Phase::List);
    // The cursor is on the used slot 2: the question, the cursor on No.
    assert_eq!(screen.key(CampKey::Decide, false, &mut sounds), None);
    assert_eq!(screen.phase(), Phase::Confirm(1));
    assert_eq!(screen.message().as_deref(), Some("Overwrite the save data in this slot?"));
    settle(&mut screen, &mut sounds, 30);
    assert_eq!(screen.dialog.text("dialogButton.dialogBtnText.textField").as_deref(), Some("Yes"));
    assert_eq!(screen.dialog.text("dialogButton2.dialogBtnText.textField").as_deref(), Some("No"));
    shot("save_overwrite", &screen, &fonts);
    // No: back on the list, nothing told.
    assert_eq!(screen.key(CampKey::Decide, false, &mut sounds), None);
    assert_eq!(screen.phase(), Phase::List);
    settle(&mut screen, &mut sounds, 30);
    // Again, up to Yes, decide: the save of slot index 1.
    screen.key(CampKey::Decide, false, &mut sounds);
    screen.key(CampKey::Up, false, &mut sounds);
    assert_eq!(screen.phase(), Phase::Confirm(0));
    assert_eq!(screen.key(CampKey::Decide, false, &mut sounds), Some(SaveLoadResult::Save(1)));
    assert_eq!(screen.phase(), Phase::Busy);
    // The host fails: the game's failure message with OK, then the list.
    screen.set_progress(Progress::Failed);
    settle(&mut screen, &mut sounds, MESSAGE_MIN_FRAMES as usize + 20);
    assert_eq!(screen.phase(), Phase::Failed);
    assert_eq!(screen.message().as_deref(), Some("Failed to save game data."));
    shot("save_failed", &screen, &fonts);
    screen.key(CampKey::Decide, false, &mut sounds);
    assert_eq!(screen.phase(), Phase::List);
}

#[test]
fn the_load_list_refuses_an_empty_slot_and_loads_a_used_one_without_a_question() {
    let Some((mut screen, fonts)) = screen() else { return };
    let mut sounds = Vec::new();
    screen.open_load(&slots(), true, &mut sounds);
    settle(&mut screen, &mut sounds, 60);
    assert_eq!(screen.phase(), Phase::List);
    let m = &screen.movie;
    assert_eq!(m.text("menuLoadClearData.stageCategory.category.categoryName.txName").as_deref(), Some("LOAD"));
    assert_eq!(m.text("menuLoadClearData.loadClearDataObj.helpText.txHelpText.textField").as_deref(), Some("Select a slot to load from."));
    shot("load_list", &screen, &fonts);
    // R1: the next page (rows 6 - 10).
    sounds.clear();
    screen.key(CampKey::R1, false, &mut sounds);
    assert_eq!(screen.cursor(), 6);
    settle(&mut screen, &mut sounds, 20);
    shot("load_page2", &screen, &fonts);
    // Down to an empty slot: refused.
    screen.key(CampKey::Down, false, &mut sounds);
    sounds.clear();
    assert_eq!(screen.key(CampKey::Decide, false, &mut sounds), None);
    assert_eq!((screen.phase(), sounds.as_slice()), (Phase::List, [UiSound::Play(SE_MENU_REFUSED)].as_slice()));
    // The list does not wrap (no scroll loop): down at the last row stays.
    for _ in 0..5 {
        screen.key(CampKey::Down, false, &mut sounds);
    }
    assert_eq!(screen.cursor(), 9);
    // Back up to slot 7 (used): decide loads it, no question.
    for _ in 0..3 {
        screen.key(CampKey::Up, false, &mut sounds);
    }
    sounds.clear();
    assert_eq!(screen.key(CampKey::Decide, false, &mut sounds), None);
    assert_eq!(sounds, [UiSound::Play(SE_MENU_DECIDE)]);
    assert_eq!(settle(&mut screen, &mut sounds, 30), Some(SaveLoadResult::Load(6)));
    assert_eq!(screen.message().as_deref(), Some("Loading...
Don't turn off your PC at this time."));
    settle(&mut screen, &mut sounds, 20);
    shot("load_busy", &screen, &fonts);
    screen.set_progress(Progress::Done);
    assert_eq!(settle(&mut screen, &mut sounds, MESSAGE_MIN_FRAMES as usize + 40), None);
    assert!(!screen.is_open());
}
