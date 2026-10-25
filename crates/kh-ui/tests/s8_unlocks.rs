//! (ER party S8) the new game's starter question (newgame.rs) and the talk window (talk.rs) on the real UI pack.

use kh_ui::camp::CampKey;
use kh_ui::newgame::{NewGameChoice, Step, StarterQuestion, STARTER};
use kh_ui::talk::{TalkEvent, TalkKind, TalkWindow};
use kh_ui::{Fonts, Pack};

fn pack() -> Option<(Pack, Fonts)> {
    let mut pack = Pack::open_default().map_err(|_| eprintln!("SKIPPED: no build/sora_pack/ui")).ok()?;
    let fonts = Fonts::load(&mut pack).ok()?;
    Some((pack, fonts))
}

fn question() -> StarterQuestion {
    StarterQuestion { question: "Whose journey will you begin?".into(), options: ["Sora".into(), "Donald".into(), "Goofy".into()], describe: ["a".into(), "b".into(), "c".into()], confirm: "Is this who you will be?".into() }
}

/// Ticks until the window takes keys (or `max` frames).
fn until_input(menu: &mut NewGameChoice, max: u32) {
    for _ in 0..max {
        if menu.accepts_input() {
            return;
        }
        menu.tick();
    }
    panic!("the window never took input (step {:?})", menu.step());
}

fn decide(menu: &mut NewGameChoice, sounds: &mut Vec<kh_ui::UiSound>) {
    until_input(menu, 600);
    menu.key(CampKey::Decide, false, sounds);
    for _ in 0..60 {
        if menu.accepts_input() || !menu.is_open() {
            break;
        }
        menu.tick();
    }
}

#[test]
fn the_starter_question_comes_first_and_is_taken_with_the_pair() {
    let Some((mut pack, fonts)) = pack() else { return };
    let mut menu = NewGameChoice::load(&mut pack, fonts).expect("menu");
    menu.set_starter_question(Some(question()));
    let mut sounds = Vec::new();
    menu.open(&mut sounds);
    assert_eq!(menu.step(), Step::List(STARTER));
    until_input(&mut menu, 600);
    assert_eq!(menu.entries(), &["Sora".to_string(), "Donald".to_string(), "Goofy".to_string()]);
    // cancel on the first list is refused
    menu.key(CampKey::Cancel, false, &mut sounds);
    assert_eq!(menu.step(), Step::List(STARTER));
    // Goofy: down twice, decide
    menu.key(CampKey::Down, false, &mut sounds);
    menu.key(CampKey::Down, false, &mut sounds);
    assert_eq!(menu.cursor(), 2);
    decide(&mut menu, &mut sounds);
    // the description, then the confirmation: Yes (cursor 1)
    for _ in 0..600 {
        if menu.step() == Step::Confirm(STARTER) && menu.accepts_input() {
            break;
        }
        menu.tick();
    }
    assert_eq!(menu.step(), Step::Confirm(STARTER));
    menu.key(CampKey::Down, false, &mut sounds);
    decide(&mut menu, &mut sounds);
    for _ in 0..120 {
        if menu.step() == Step::List(0) {
            break;
        }
        menu.tick();
    }
    assert_eq!(menu.step(), Step::List(0), "after the starter: the game's first question");
    // cancel on the game's first question goes back to the starter question
    until_input(&mut menu, 600);
    menu.key(CampKey::Cancel, false, &mut sounds);
    for _ in 0..120 {
        if menu.step() == Step::List(STARTER) {
            break;
        }
        menu.tick();
    }
    assert_eq!(menu.step(), Step::List(STARTER));
    assert_eq!(menu.cursor(), 2, "the starter pick is kept");
    // through everything with Yes: the starter comes with the pair
    let mut done = None;
    for _ in 0..40 {
        if !menu.is_open() {
            break;
        }
        until_input(&mut menu, 600);
        if matches!(menu.step(), Step::Confirm(_) | Step::Final) {
            menu.key(CampKey::Down, false, &mut sounds);
        }
        menu.key(CampKey::Decide, false, &mut sounds);
        for _ in 0..600 {
            if let Some(p) = menu.tick() {
                done = Some(p);
            }
            if menu.accepts_input() || !menu.is_open() {
                break;
            }
        }
    }
    assert!(done.is_some(), "the menu ended with a pair");
    assert_eq!(menu.take_starter(), Some(2));
    assert_eq!(menu.take_starter(), None, "once");
}

#[test]
fn without_the_starter_question_the_menu_is_the_games() {
    let Some((mut pack, fonts)) = pack() else { return };
    let mut menu = NewGameChoice::load(&mut pack, fonts).expect("menu");
    let mut sounds = Vec::new();
    menu.open(&mut sounds);
    assert_eq!(menu.step(), Step::List(0));
    until_input(&mut menu, 600);
    menu.key(CampKey::Cancel, false, &mut sounds);
    assert_eq!(menu.step(), Step::List(0));
    assert_eq!(menu.take_starter(), None);
}

#[test]
fn a_talk_line_reveals_advances_and_closes() {
    let Some((mut pack, fonts)) = pack() else { return };
    let mut w = TalkWindow::load(&mut pack, fonts).expect("talk window");
    let mut sounds = Vec::new();
    w.say(Some("Donald"), "Aw, phooey! That old crone's a fake!", (900.0, 600.0), &mut sounds);
    assert!(w.is_open());
    assert_eq!(w.kind(), TalkKind::Line);
    let mut opened = false;
    for _ in 0..120 {
        w.tick();
        if w.take_events().contains(&TalkEvent::Opened) {
            opened = true;
            break;
        }
    }
    assert!(opened, "the window's in animation ends");
    // the first decide shows the whole line, the second goes on
    w.tick();
    assert!(!w.finished());
    w.key(CampKey::Decide, false, &mut sounds);
    assert!(w.finished());
    assert!(w.shown_text.contains("Donald") && w.shown_text.contains("fake!"), "{}", w.shown_text);
    w.key(CampKey::Decide, false, &mut sounds);
    assert_eq!(w.take_events(), vec![TalkEvent::Advanced]);
    w.close(&mut sounds);
    let mut closed = false;
    for _ in 0..120 {
        w.tick();
        if w.take_events().contains(&TalkEvent::Closed) {
            closed = true;
            break;
        }
    }
    assert!(closed && !w.is_open());
}

#[test]
fn a_banner_leaves_by_itself() {
    let Some((mut pack, fonts)) = pack() else { return };
    let mut w = TalkWindow::load(&mut pack, fonts).expect("talk window");
    let mut sounds = Vec::new();
    w.banner("Goofy joined the party!", 1.0, &mut sounds);
    let mut closed = false;
    for _ in 0..400 {
        w.tick();
        if w.take_events().contains(&TalkEvent::Closed) {
            closed = true;
            break;
        }
    }
    assert!(closed, "the banner closes after its time");
}

/// Pictures for the state note (build/sora_tmp/er/party/s8/shots/*.png): the character select, a talk line, the
/// prompt, the banner, over a plain backdrop.
#[test]
fn pictures() {
    let Some((mut pack, fonts)) = pack() else { return };
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/er/party/s8/shots");
    std::fs::create_dir_all(&dir).ok();
    let shot = |name: &str, movies: &[&kh_ui::Movie]| {
        let mut canvas = kh_ui::Canvas::new(1920, 1080);
        let mut renderer = kh_ui::Renderer::new(fonts.clone());
        for m in movies {
            renderer.draw(&mut canvas, m, &kh_ui::Matrix::IDENTITY);
        }
        let mut rgba = Vec::new();
        canvas.to_rgba8_over(|x, y| [0.18 + 0.2 * (x as f32 / 1920.0), 0.22, 0.20 + 0.2 * (y as f32 / 1080.0)], &mut rgba);
        kh_ui::def::write_png(&dir.join(format!("{name}.png")), 1920, 1080, &rgba).unwrap();
    };
    // the character select's list, then its description
    let mut menu = NewGameChoice::load(&mut pack, fonts.clone()).expect("menu");
    menu.set_starter_question(Some(StarterQuestion { question: "Whose journey will you begin?".into(), options: ["Sora".into(), "Donald".into(), "Goofy".into()], describe: ["A boy chosen by the Keyblade.<br>Strikes, spells and speed in equal measure.".into(), "The royal court magician.<br>Masters every spell, but keep him out of the thick of it.".into(), "Captain of the royal knights.<br>His shield guards him and his friends alike.".into()], confirm: "Is this who you will be?".into() }));
    let mut sounds = Vec::new();
    menu.open(&mut sounds);
    until_input(&mut menu, 600);
    menu.key(CampKey::Down, false, &mut sounds);
    for _ in 0..30 {
        menu.tick();
    }
    shot("select_list", &[&menu.text, &menu.dialog]);
    // the talk window: a line (all shown), the prompt, the banner
    let mut w = TalkWindow::load(&mut pack, fonts.clone()).expect("talk window");
    w.say(Some("Donald"), "I let her read my fingers, and she says, \"A dreadful sign! Truly dreadful!\"", (1100.0, 520.0), &mut sounds);
    for _ in 0..40 {
        w.tick();
    }
    w.key(CampKey::Decide, false, &mut sounds);
    for _ in 0..5 {
        w.tick();
    }
    shot("talk_line", &[&w.dialog]);
    let mut p = TalkWindow::load(&mut pack, fonts.clone()).expect("talk window");
    p.prompt(&kh_ui::talk::prompt_text("Talk"), (900.0, 560.0), &mut sounds);
    for _ in 0..40 {
        p.tick();
    }
    shot("talk_prompt", &[&p.dialog]);
    let mut b = TalkWindow::load(&mut pack, fonts.clone()).expect("talk window");
    b.banner("Donald joined the party!", 3.0, &mut sounds);
    for _ in 0..40 {
        b.tick();
    }
    shot("unlock_banner", &[&b.dialog]);
}
