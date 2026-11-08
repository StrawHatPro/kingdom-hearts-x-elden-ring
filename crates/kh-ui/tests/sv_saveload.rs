//! (SV, the user 2026-10-17: "Have a menu to load save data at save points, and have the ability to delete save data in
//! the save menu (without saving)") KH3's Delete on the slot lists and the load question of the save point's Load.
//! Pictures: build/sora_tmp/party/sv/shots/ui_*.png (KH_UI_SHOTS names another folder).

use std::path::Path;

use kh_ui::camp::CampKey;
use kh_ui::def::write_png;
use kh_ui::menu::{SE_MENU_CLOSE, SE_MENU_CURSOR, SE_MENU_DECIDE, SE_MENU_REFUSED};
use kh_ui::saveload::{Phase, Portrait, Progress, SaveLoadResult, SaveLoadScreen, SlotMode, SlotState, SlotSummary, DELETE_FAILED, DELETE_FOR_NEW_GAME, DELETE_IN_USE, LOAD_QUESTION, MESSAGE_MIN_FRAMES, SE_SAVE_DONE};
use kh_ui::savepoint::{SavePointCommand, SavePointMenu, SavePointResult, LOAD_TEXT, MAX_COMMANDS};
use kh_ui::{Canvas, Fonts, Matrix, Pack, Renderer, UiSound};

fn screen() -> Option<(SaveLoadScreen, Fonts, Pack)> {
    let mut pack = Pack::open_default().map_err(|_| eprintln!("SKIPPED: no build/sora_pack/ui")).ok()?;
    let fonts = Fonts::load(&mut pack).unwrap();
    let screen = SaveLoadScreen::load(&mut pack, fonts.clone()).unwrap();
    Some((screen, fonts, pack))
}

fn shots_dir() -> std::path::PathBuf {
    match std::env::var_os("KH_UI_SHOTS") {
        Some(dir) => std::path::PathBuf::from(dir),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/party/sv/shots"),
    }
}

fn picture(name: &str, movies: &[&kh_ui::Movie], fonts: &Fonts) {
    let dir = shots_dir();
    std::fs::create_dir_all(&dir).ok();
    let mut canvas = Canvas::new(1920, 1080);
    let mut renderer = Renderer::new(fonts.clone());
    for movie in movies {
        renderer.draw(&mut canvas, movie, &Matrix::IDENTITY);
    }
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|x, y| [0.25 + 0.25 * (x as f32 / 1920.0), 0.45, 0.30 + 0.3 * (y as f32 / 1080.0)], &mut rgba);
    write_png(&dir.join(format!("ui_{name}.png")), 1920, 1080, &rgba).unwrap();
}

fn shot(name: &str, screen: &SaveLoadScreen, fonts: &Fonts) {
    let mut movies = vec![&screen.movie];
    if let Some(dialog) = screen.dialog_movie() {
        movies.push(dialog);
    }
    picture(name, &movies, fonts);
}

/// Ten slots: 0, 1 (the running game's) and 6 used.
fn slots() -> Vec<SlotSummary> {
    (0..10)
        .map(|index| match index {
            0 => SlotSummary { index, state: SlotState::Used, name: "Sora".into(), level: 12, munny: 1234, play_time_seconds: 3 * 3600 + 7 * 60 + 9, location: "Stranded Graveyard".into(), world: None, mode: SlotMode::None, clear: false, is_current: false, portrait: Portrait::Sora },
            1 => SlotSummary { index, state: SlotState::Used, name: "Sora".into(), level: 20, munny: 5000, play_time_seconds: 10 * 3600, location: "Church of Elleh".into(), world: None, mode: SlotMode::None, clear: false, is_current: true, portrait: Portrait::Goofy },
            6 => SlotSummary { index, state: SlotState::Used, name: "Sora".into(), level: 3, munny: 40, play_time_seconds: 900, location: "The First Step".into(), world: None, mode: SlotMode::None, clear: false, is_current: false, portrait: Portrait::Donald },
            _ => SlotSummary::empty(index),
        })
        .collect()
}

fn settle(screen: &mut SaveLoadScreen, sounds: &mut Vec<UiSound>, frames: usize) -> Option<SaveLoadResult> {
    (0..frames).find_map(|_| screen.tick(sounds))
}

#[test]
fn delete_asks_first_with_the_cursor_on_no_and_never_deletes_without_yes() {
    let Some((mut screen, fonts, _)) = screen() else { return };
    screen.delete_enabled = true;
    let mut sounds = Vec::new();
    screen.open_save(&slots(), true, false, &mut sounds);
    settle(&mut screen, &mut sounds, 60);
    assert_eq!(screen.phase(), Phase::List);
    // the guide under the help line
    assert_eq!(screen.movie.text("menuSaveData.saveDataObj.helpText.txHelpText.textField").as_deref(), Some("Select a slot to save to.\n<icon(Triangle)> Delete"));
    shot("save_list_delete_guide", &screen, &fonts);
    // up to slot 1 (index 0)
    screen.key(CampKey::Up, false, &mut sounds);
    assert_eq!(screen.cursor(), 0);
    sounds.clear();
    // Y / Triangle: the question, the cursor on No
    assert_eq!(screen.key(CampKey::Triangle, false, &mut sounds), None);
    assert_eq!(screen.phase(), Phase::ConfirmDelete(1));
    assert_eq!(sounds, [UiSound::Play(SE_MENU_DECIDE)]);
    assert_eq!(screen.message().as_deref(), Some("Delete the save data in this slot?"));
    settle(&mut screen, &mut sounds, 30);
    assert_eq!(screen.dialog.text("dialogButton.dialogBtnText.textField").as_deref(), Some("Yes"));
    assert_eq!(screen.dialog.text("dialogButton2.dialogBtnText.textField").as_deref(), Some("No"));
    shot("delete_question", &screen, &fonts);
    // A on No: nothing told, the list again
    sounds.clear();
    assert_eq!(screen.key(CampKey::Decide, false, &mut sounds), None);
    assert_eq!((screen.phase(), sounds.as_slice()), (Phase::List, [UiSound::Play(SE_MENU_CLOSE)].as_slice()));
    // again, B: nothing told
    screen.key(CampKey::Triangle, false, &mut sounds);
    assert_eq!(screen.key(CampKey::Cancel, false, &mut sounds), None);
    assert_eq!(screen.phase(), Phase::List);
    // again, up to Yes (cursor sound), A: Delete(0) and "Deleting..."
    screen.key(CampKey::Triangle, false, &mut sounds);
    sounds.clear();
    screen.key(CampKey::Up, false, &mut sounds);
    assert_eq!((screen.phase(), sounds.as_slice()), (Phase::ConfirmDelete(0), [UiSound::Play(SE_MENU_CURSOR)].as_slice()));
    assert_eq!(screen.key(CampKey::Decide, false, &mut sounds), Some(SaveLoadResult::Delete(0)));
    assert_eq!(screen.phase(), Phase::Busy);
    assert_eq!(screen.message().as_deref(), Some("Deleting...\nDon't turn off your PC at this time."));
    settle(&mut screen, &mut sounds, 20);
    shot("delete_busy", &screen, &fonts);
    // the host is done: the new rows first, then Done: the access-done sound, the row shows NO DATA, the cursor stays
    let mut new = slots();
    new[0] = SlotSummary::empty(0);
    screen.set_slots(&new);
    screen.set_progress(Progress::Done);
    sounds.clear();
    assert_eq!(settle(&mut screen, &mut sounds, MESSAGE_MIN_FRAMES as usize + 5), None);
    assert_eq!((screen.phase(), screen.cursor()), (Phase::List, 0));
    assert_eq!(sounds, [UiSound::Play(SE_SAVE_DONE)]);
    settle(&mut screen, &mut sounds, 30);
    let list = "menuSaveData.saveDataObj.coList.container";
    assert_eq!(screen.movie.current_label(&format!("{list}.i0.dataLabel")), Some("empty"));
    shot("delete_done", &screen, &fonts);
    // Y on an empty row: refused, nothing asked
    sounds.clear();
    assert_eq!(screen.key(CampKey::Triangle, false, &mut sounds), None);
    assert_eq!((screen.phase(), sounds.as_slice()), (Phase::List, [UiSound::Play(SE_MENU_REFUSED)].as_slice()));
}

#[test]
fn the_running_games_slot_is_refused_with_a_message_and_a_failure_is_said() {
    let Some((mut screen, fonts, _)) = screen() else { return };
    screen.delete_enabled = true;
    let mut sounds = Vec::new();
    screen.open_save(&slots(), true, false, &mut sounds);
    settle(&mut screen, &mut sounds, 60);
    // the cursor starts on the current slot (index 1)
    assert_eq!(screen.cursor(), 1);
    sounds.clear();
    assert_eq!(screen.key(CampKey::Triangle, false, &mut sounds), None);
    assert_eq!((screen.phase(), sounds.as_slice()), (Phase::Notice, [UiSound::Play(SE_MENU_REFUSED)].as_slice()));
    assert_eq!(screen.message().as_deref(), Some(DELETE_IN_USE));
    settle(&mut screen, &mut sounds, 30);
    shot("delete_in_use", &screen, &fonts);
    assert_eq!(screen.key(CampKey::Decide, false, &mut sounds), None);
    assert_eq!(screen.phase(), Phase::List);
    // a host that lets its own slot go (the test level): asked as any other
    screen.delete_current = true;
    screen.key(CampKey::Triangle, false, &mut sounds);
    assert_eq!(screen.phase(), Phase::ConfirmDelete(1));
    screen.key(CampKey::Up, false, &mut sounds);
    assert_eq!(screen.key(CampKey::Decide, false, &mut sounds), Some(SaveLoadResult::Delete(1)));
    // the host fails: the message with OK, then the list
    screen.set_progress(Progress::Failed);
    settle(&mut screen, &mut sounds, MESSAGE_MIN_FRAMES as usize + 20);
    assert_eq!((screen.phase(), screen.message().as_deref()), (Phase::Failed, Some(DELETE_FAILED)));
    screen.key(CampKey::Decide, false, &mut sounds);
    assert_eq!(screen.phase(), Phase::List);
    // switched off (KH3's port before SV): Y does nothing
    screen.delete_enabled = false;
    sounds.clear();
    assert_eq!(screen.key(CampKey::Triangle, false, &mut sounds), None);
    assert!(sounds.is_empty() && screen.phase() == Phase::List);
}

#[test]
fn the_save_points_load_asks_first_and_deletes_on_the_load_list_too() {
    let Some((mut screen, fonts, _)) = screen() else { return };
    (screen.delete_enabled, screen.confirm_load) = (true, true);
    let mut sounds = Vec::new();
    screen.open_load(&slots(), false, &mut sounds);
    settle(&mut screen, &mut sounds, 60);
    assert_eq!(screen.phase(), Phase::List);
    assert_eq!(screen.movie.text("menuLoadClearData.loadClearDataObj.helpText.txHelpText.textField").as_deref(), Some("Select a slot to load from.\n<icon(Triangle)> Delete"));
    shot("load_list", &screen, &fonts);
    // the cursor on the current slot (index 1); up to index 0: A asks, the cursor on No
    screen.key(CampKey::Up, false, &mut sounds);
    sounds.clear();
    assert_eq!(screen.key(CampKey::Decide, false, &mut sounds), None);
    assert_eq!((screen.phase(), sounds.as_slice()), (Phase::ConfirmLoad(1), [UiSound::Play(SE_MENU_DECIDE)].as_slice()));
    assert_eq!(screen.message().as_deref(), Some(LOAD_QUESTION));
    settle(&mut screen, &mut sounds, 30);
    shot("load_question", &screen, &fonts);
    // No: nothing
    assert_eq!(screen.key(CampKey::Decide, false, &mut sounds), None);
    assert_eq!(screen.phase(), Phase::List);
    assert_eq!(settle(&mut screen, &mut sounds, 30), None);
    // Yes: the row's decide, then Load(0) and "Loading..."
    screen.key(CampKey::Decide, false, &mut sounds);
    screen.key(CampKey::Up, false, &mut sounds);
    assert_eq!(screen.key(CampKey::Decide, false, &mut sounds), None);
    assert_eq!(screen.phase(), Phase::Deciding);
    assert_eq!(settle(&mut screen, &mut sounds, 30), Some(SaveLoadResult::Load(0)));
    assert_eq!(screen.message().as_deref(), Some("Loading...\nDon't turn off your PC at this time."));
    settle(&mut screen, &mut sounds, 20);
    shot("load_busy", &screen, &fonts);
    screen.set_progress(Progress::Done);
    assert_eq!(settle(&mut screen, &mut sounds, MESSAGE_MIN_FRAMES as usize + 40), None);
    assert!(!screen.is_open());
    // Delete on the load list: asked the same way
    screen.open_load(&slots(), false, &mut sounds);
    settle(&mut screen, &mut sounds, 60);
    for _ in 0..5 {
        screen.key(CampKey::Down, false, &mut sounds);
    }
    assert_eq!(screen.cursor(), 6);
    screen.key(CampKey::Triangle, false, &mut sounds);
    assert_eq!(screen.phase(), Phase::ConfirmDelete(1));
    screen.key(CampKey::Down, false, &mut sounds);
    assert_eq!(screen.key(CampKey::Decide, false, &mut sounds), Some(SaveLoadResult::Delete(6)));
}

/// (SAVE-DELETE, the user 2026-11-07: "allow deleting save games from the load game screen from the title, same as save
/// point menu") THE TITLE'S LOAD SCREEN as kh-er opens it (`delete_enabled` from the save point's own switch, no load
/// question, no world so no row is the running game's, the stage background): the save point's Delete unchanged (the
/// guide line, Y asks with the cursor on No, nothing without Yes, then the rows anew and the list keeps the pad), and A on a
/// used row still loads at once (KH3's title has no load question).
#[test]
fn the_titles_load_screen_deletes_as_the_save_point_does_and_still_loads_on_a() {
    let Some((mut screen, fonts, _)) = screen() else { return };
    (screen.delete_enabled, screen.delete_current, screen.confirm_load, screen.delete_only) = (true, false, false, false);
    let title_slots: Vec<SlotSummary> = slots().into_iter().map(|s| SlotSummary { is_current: false, ..s }).collect();
    let mut sounds = Vec::new();
    screen.open_load(&title_slots, true, &mut sounds);
    settle(&mut screen, &mut sounds, 60);
    assert_eq!(screen.phase(), Phase::List);
    // the button prompt in the help bar: the save point's guide, "<icon(Triangle)> Delete" (Y on an Xbox pad; the icon
    // follows the remap tables and the Button Icons setting through kh-ui glyphs.rs)
    assert_eq!(screen.movie.text("menuLoadClearData.loadClearDataObj.helpText.txHelpText.textField").as_deref(), Some("Select a slot to load from.\n<icon(Triangle)> Delete"));
    shot("title_load_delete_guide", &screen, &fonts);
    // no running game: the first used row (index 1, the save point's "current" one) is asked about like any other
    assert_eq!(screen.cursor(), 0);
    screen.key(CampKey::Down, false, &mut sounds);
    assert_eq!(screen.cursor(), 1);
    sounds.clear();
    assert_eq!(screen.key(CampKey::Triangle, false, &mut sounds), None);
    assert_eq!((screen.phase(), sounds.as_slice()), (Phase::ConfirmDelete(1), [UiSound::Play(SE_MENU_DECIDE)].as_slice()));
    assert_eq!(screen.message().as_deref(), Some("Delete the save data in this slot?"));
    // No (the cursor's place): nothing told; B: nothing told
    assert_eq!(screen.key(CampKey::Decide, false, &mut sounds), None);
    assert_eq!(screen.phase(), Phase::List);
    screen.key(CampKey::Triangle, false, &mut sounds);
    assert_eq!(screen.key(CampKey::Cancel, false, &mut sounds), None);
    assert_eq!(screen.phase(), Phase::List);
    // Yes: Delete(1), "Deleting..."; the host's rows anew + Done: NO DATA, the list again, the screen stays up
    screen.key(CampKey::Triangle, false, &mut sounds);
    screen.key(CampKey::Up, false, &mut sounds);
    assert_eq!(screen.key(CampKey::Decide, false, &mut sounds), Some(SaveLoadResult::Delete(1)));
    assert_eq!(screen.phase(), Phase::Busy);
    let mut new = title_slots.clone();
    new[1] = SlotSummary::empty(1);
    screen.set_slots(&new);
    screen.set_progress(Progress::Done);
    assert_eq!(settle(&mut screen, &mut sounds, MESSAGE_MIN_FRAMES as usize + 30), None);
    assert!(screen.is_open());
    assert_eq!((screen.phase(), screen.cursor()), (Phase::List, 1));
    assert_eq!(screen.movie.current_label("menuLoadClearData.loadClearDataObj.coList.container.i1.dataLabel"), Some("empty"));
    shot("title_load_delete_done", &screen, &fonts);
    // the deleted row: A refuses, Y refuses (nothing asked)
    sounds.clear();
    assert_eq!(screen.key(CampKey::Decide, false, &mut sounds), None);
    assert_eq!(screen.key(CampKey::Triangle, false, &mut sounds), None);
    assert_eq!((screen.phase(), sounds.as_slice()), (Phase::List, [UiSound::Play(SE_MENU_REFUSED), UiSound::Play(SE_MENU_REFUSED)].as_slice()));
    // A on another used row: loads at once, no question
    screen.key(CampKey::Up, false, &mut sounds);
    assert_eq!(screen.cursor(), 0);
    assert_eq!(screen.key(CampKey::Decide, false, &mut sounds), None);
    assert_eq!(screen.phase(), Phase::Deciding);
    assert_eq!(settle(&mut screen, &mut sounds, 30), Some(SaveLoadResult::Load(0)));
}

/// (NG delete) A new game with every slot in use: the load list only deletes; A asks the Delete question as Y does and
/// nothing is ever loaded.
#[test]
fn the_new_games_delete_list_deletes_on_a_and_never_loads() {
    let Some((mut screen, fonts, _)) = screen() else { return };
    (screen.delete_enabled, screen.delete_only) = (true, true);
    let mut sounds = Vec::new();
    screen.open_load(&slots(), true, &mut sounds);
    settle(&mut screen, &mut sounds, 60);
    assert_eq!(screen.movie.text("menuLoadClearData.loadClearDataObj.helpText.txHelpText.textField").as_deref(), Some(format!("{DELETE_FOR_NEW_GAME}\n<icon(Triangle)> Delete").as_str()));
    shot("newgame_delete_list", &screen, &fonts);
    // an empty row: refused
    screen.key(CampKey::Down, false, &mut sounds);
    assert_eq!(screen.cursor(), 2);
    sounds.clear();
    assert_eq!(screen.key(CampKey::Decide, false, &mut sounds), None);
    assert_eq!((screen.phase(), sounds.as_slice()), (Phase::List, [UiSound::Play(SE_MENU_REFUSED)].as_slice()));
    // a used row: the Delete question (the cursor on No), No gives the list back, nothing told
    screen.key(CampKey::Up, false, &mut sounds);
    screen.key(CampKey::Up, false, &mut sounds);
    assert_eq!(screen.cursor(), 0);
    assert_eq!(screen.key(CampKey::Decide, false, &mut sounds), None);
    assert_eq!((screen.phase(), screen.message().as_deref()), (Phase::ConfirmDelete(1), Some("Delete the save data in this slot?")));
    assert_eq!(screen.key(CampKey::Decide, false, &mut sounds), None);
    assert_eq!(screen.phase(), Phase::List);
    // Yes: Delete(0), never Load
    screen.key(CampKey::Decide, false, &mut sounds);
    screen.key(CampKey::Up, false, &mut sounds);
    assert_eq!(screen.key(CampKey::Decide, false, &mut sounds), Some(SaveLoadResult::Delete(0)));
    assert_eq!(screen.phase(), Phase::Busy);
}

#[test]
fn the_save_point_menu_has_load_with_its_own_text_and_room_for_eight() {
    let Some((_, fonts, mut pack)) = screen() else { return };
    let mut menu = SavePointMenu::load(&mut pack, fonts.clone()).unwrap();
    use SavePointCommand as C;
    // (the widest menu: eight entries)
    let commands = [C::Save, C::Load, C::WorldMap, C::Bistro, C::Tutorials, C::SelectArea, C::PartyFormation, C::TitleScreen];
    assert_eq!(commands.len(), MAX_COMMANDS);
    menu.open(&commands, &[]);
    assert_eq!(menu.commands(), &commands);
    for _ in 0..30 {
        menu.tick();
    }
    assert_eq!(menu.movie.text("menuSavePoint.savePointObj.cmd2.mainCmdNameAnm.mainCmdName").as_deref(), Some(LOAD_TEXT.0));
    assert_eq!(menu.movie.text("menuSavePoint.savePointObj.cmd2.helpMessageAnm.helpMessage").as_deref(), Some(LOAD_TEXT.1));
    assert!(menu.movie.exists("menuSavePoint.savePointObj.cmd8"));
    let y = |m: &kh_ui::Movie, k: usize| m.node(&format!("menuSavePoint.savePointObj.cmd{k}")).map(|n| n.matrix.ty).unwrap();
    // the seventh and eighth slots continue the pitch of the authored six
    let pitch = y(&menu.movie, 6) - y(&menu.movie, 5);
    assert!((y(&menu.movie, 7) - y(&menu.movie, 6) - pitch).abs() < 0.01 && (y(&menu.movie, 8) - y(&menu.movie, 7) - pitch).abs() < 0.01);
    // down to Load, A: chosen
    let mut sounds = Vec::new();
    menu.key(CampKey::Down, false, &mut sounds);
    for _ in 0..10 {
        menu.tick();
    }
    picture("savepoint_menu_load", &[&menu.movie], &fonts);
    menu.key(CampKey::Decide, false, &mut sounds);
    let chosen = (0..40).find_map(|_| menu.tick());
    assert_eq!(chosen, Some(SavePointResult::Chosen(C::Load)));
    // the Load screen's cancel brings the menu back with the cursor on Load
    menu.reopen_back_at(C::Load);
    assert!(menu.is_open() && menu.cursor() == 1);
    // the eighth entry can be reached and chosen
    for _ in 0..6 {
        menu.key(CampKey::Down, false, &mut sounds);
    }
    assert_eq!(menu.cursor(), 7);
    for _ in 0..10 {
        menu.tick();
    }
    picture("savepoint_menu_eight", &[&menu.movie], &fonts);
}
