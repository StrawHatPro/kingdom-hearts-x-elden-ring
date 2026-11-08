//! (ER-POPUPS) The event-script boxes' rules, without the game: the instructions' layout as the game's own scripts write
//! them (the evergaol's 90005881, the imp statue's 90005620, the "Cannot open from this side" door 90005515, the great
//! rune's 4852), the route per TYPE, the waiter's answer -> the flag the game's waiter sets, the text, the window's life.

use super::*;

/// A KH window that records what it is asked and answers what the test says.
#[derive(Default)]
struct MockHud {
    asked: Vec<(u64, String, Buttons)>,
    up: Vec<u64>,
    answers: Vec<(u64, bool)>,
    cancelled: Vec<u64>,
    refuse: bool,
}

impl DialogHud for MockHud {
    fn ask(&mut self, id: u64, text: &str, buttons: Buttons) -> bool {
        if self.refuse {
            return false;
        }
        self.asked.push((id, text.to_string(), buttons));
        self.up.push(id);
        true
    }
    fn take(&mut self, id: u64) -> Option<bool> {
        let at = self.answers.iter().position(|a| a.0 == id)?;
        self.up.retain(|u| *u != id);
        Some(self.answers.remove(at).1)
    }
    fn pending(&self, id: u64) -> bool {
        self.up.contains(&id)
    }
    fn cancel(&mut self, id: u64) {
        self.up.retain(|u| *u != id);
        self.cancelled.push(id);
    }
}

fn bytes(words: &[i32]) -> Vec<u8> {
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}

/// 2007[10] as common_func 90005881 (the evergaols) writes it once its parameters are in: msg 20300 "Enter evergaol?",
/// prompt 0, buttons 2, the evergaol's entity, 3.0 m, left X8, right X12, cancel X12 (Stormhill: 1042362800 / 1042362801).
fn evergaol() -> Request {
    let mut a = bytes(&[20300]);
    a.extend_from_slice(&0i16.to_le_bytes());
    a.extend_from_slice(&2i16.to_le_bytes());
    a.extend(bytes(&[1_042_361_700]));
    a.extend_from_slice(&3.0f32.to_le_bytes());
    a.extend(bytes(&[1_042_362_800, 1_042_362_801, 1_042_362_801]));
    Request::parse(INSTR_QUESTION, &a).expect("28 bytes")
}

/// 2007[1] as common_func 90005515 writes it: msg 4010 "Cannot open from this side", prompt 1, buttons 1, entity, 3.0 m.
fn door_message() -> Request {
    let mut a = bytes(&[4010]);
    a.extend_from_slice(&1i16.to_le_bytes());
    a.extend_from_slice(&1i16.to_le_bytes());
    a.extend(bytes(&[10_001_000]));
    a.extend_from_slice(&3.0f32.to_le_bytes());
    Request::parse(INSTR_MESSAGE, &a).expect("16 bytes")
}

#[test]
fn er_popups_the_instructions_parse_as_the_game_writes_them() {
    let q = evergaol();
    assert_eq!((q.msg, q.prompt, q.buttons, q.flags), (20300, 0, 2, [1_042_362_800, 1_042_362_801, 1_042_362_801]));
    assert!(q.blocking());
    assert_eq!(q.window_buttons(), Buttons::YesNo, "the game's two-button box: Yes / No");
    let m = door_message();
    assert_eq!((m.msg, m.prompt, m.buttons, m.flags), (4010, 1, 1, [0; 3]));
    assert!(!m.blocking());
    assert_eq!(m.window_buttons(), Buttons::Ok);
    // 2007[10] with one button (common 4852 "Somewhere, a great rune has broken...": prompt 1, buttons 1) and 2007[1] with
    // buttons 6 (the "Used <item>" messages, the war plaques): OK
    let mut great_rune = bytes(&[1_030_040]);
    great_rune.extend_from_slice(&1i16.to_le_bytes());
    great_rune.extend_from_slice(&1i16.to_le_bytes());
    great_rune.extend(bytes(&[0, 0x42c8_0000, 2_048_452_713, 2_048_452_712, 2_048_452_712]));
    assert_eq!(Request::parse(INSTR_QUESTION, &great_rune).map(|r| r.window_buttons()), Some(Buttons::Ok));
    let mut used = bytes(&[208_000]);
    used.extend_from_slice(&1i16.to_le_bytes());
    used.extend_from_slice(&6i16.to_le_bytes());
    used.extend(bytes(&[0, 0x4040_0000]));
    assert_eq!(Request::parse(INSTR_MESSAGE, &used).map(|r| r.window_buttons()), Some(Buttons::Ok));
    // other instructions of the bank and short arguments are not boxes of this module
    for instr in [2, 4, 9, 12, 13, 14, 15, 16] {
        assert_eq!(Request::parse(instr, &[0; 32]), None, "2007[{instr}]");
    }
    assert_eq!(Request::parse(INSTR_QUESTION, &[0; 16]), None);
    assert_eq!(Request::parse(INSTR_MESSAGE, &[0; 12]), None);
}

/// The waiter's flags: Yes / OK = left (+0x60), No or B = right (+0x64), taken away = cancel (+0x68); 0 = none; a message
/// sets none.
#[test]
fn er_popups_each_answer_sets_the_flag_the_games_waiter_sets() {
    let q = evergaol();
    assert_eq!(q.flag_for(Answer::Left), Some(1_042_362_800));
    assert_eq!(q.flag_for(Answer::Right), Some(1_042_362_801));
    assert_eq!(q.flag_for(Answer::Cancel), Some(1_042_362_801));
    let mut no_flags = q;
    no_flags.flags = [0; 3];
    assert_eq!(no_flags.flag_for(Answer::Left), None, "m60_33_47's OK boxes pass no flags");
    assert_eq!(door_message().flag_for(Answer::Left), None);
    assert_eq!([Answer::Left.slot(), Answer::Right.slot(), Answer::Cancel.slot()], [0, 1, 2]);
}

#[test]
fn er_popups_the_text_is_the_games_with_the_goods_names_in() {
    let goods = |id: i32| (id == 8000).then(|| "Stonesword Key".to_string());
    assert_eq!(dialog_text("Use <?gdsparam@8000?>?", &goods), "Use Stonesword Key?");
    assert_eq!(dialog_text("No <?gdsparam@8000?> in inventory", &goods), "No Stonesword Key in inventory");
    assert_eq!(dialog_text("Used <?gdsparam@9999?>", &goods), "Used ...", "an unknown item: the placeholder's dots");
    assert_eq!(dialog_text("Commencing combat.\nTime limit: <?matchDuel_TotalTime?>s.", &goods), "Commencing combat.\nTime limit: ...s.");
    assert_eq!(dialog_text("Impenetrable thorns refuse all.\r\nNone may enter the Erdtree.  ", &goods), "Impenetrable thorns refuse all.\nNone may enter the Erdtree.", "line breaks kept");
    assert_eq!(dialog_text("a <b> c", &goods), "a (b) c", "angle brackets are KH markup");
    assert_eq!(dialog_text("Enter evergaol?", &goods), "Enter evergaol?");
}

/// THE EVERGAOL, end to end: the game's case runs with its box suppressed, KH3's window asks Yes / No, the waiter holds the
/// event until the answer and then sets the flag of that answer.
#[test]
fn er_popups_the_evergaol_question_yes_no_and_b() {
    for (answer, flag) in [(true, 1_042_362_800u32), (false, 1_042_362_801)] {
        let (mut d, mut hud) = (Dialogs::default(), MockHud::default());
        assert_eq!(d.instruction(evergaol(), Some("Enter evergaol?".into()), true), Route::Suppress, "the game's case runs (its waiter) without its box");
        let w = 0x7ff6_1000_0000usize;
        assert_eq!(d.waiter(w, 20300, evergaol().flags), Wait::Hold, "the waiter is bound and holds the event");
        d.step(&mut hud, true);
        assert_eq!(hud.asked, vec![(1, "Enter evergaol?".to_string(), Buttons::YesNo)]);
        for _ in 0..30 {
            assert_eq!(d.waiter(w, 20300, evergaol().flags), Wait::Hold);
            d.step(&mut hud, false);
        }
        assert!(hud.cancelled.is_empty());
        hud.answers.push((1, answer)); // A on Yes / A on No (or B: the window answers false too)
        d.step(&mut hud, false);
        assert_eq!(d.waiter(w, 20300, evergaol().flags), Wait::Done(Some(flag)), "answer {answer}");
        assert!(d.is_empty());
        assert_eq!(d.waiter(w, 20300, evergaol().flags), Wait::Game, "a new waiter at the same place is the game's");
    }
}

/// The HUD lets the window go without an answer (a load): the waiter gets the cancel answer (the cancel flag), as the
/// game's box taken away.
#[test]
fn er_popups_a_window_taken_away_is_the_cancel_answer() {
    let (mut d, mut hud) = (Dialogs::default(), MockHud::default());
    d.instruction(evergaol(), Some("Enter evergaol?".into()), true);
    d.waiter(1, 20300, evergaol().flags);
    d.step(&mut hud, false);
    hud.up.clear();
    d.step(&mut hud, false);
    assert_eq!(d.waiter(1, 20300, evergaol().flags), Wait::Done(Some(1_042_362_801)));
    // a window that cannot be shown at all: the cancel answer too
    let (mut d, mut hud) = (Dialogs::default(), MockHud { refuse: true, ..MockHud::default() });
    d.instruction(evergaol(), Some("Enter evergaol?".into()), true);
    d.step(&mut hud, true);
    assert_eq!(d.waiter(2, 20300, evergaol().flags), Wait::Done(Some(1_042_362_801)));
}

/// A MESSAGE (2007[1]): handled at once (the event goes on), KH3's window with OK, gone when acknowledged.
#[test]
fn er_popups_a_message_is_handled_at_once_and_shown_with_ok() {
    let (mut d, mut hud) = (Dialogs::default(), MockHud::default());
    assert_eq!(d.instruction(door_message(), Some("Cannot open from this side".into()), true), Route::Handled);
    d.step(&mut hud, true);
    assert_eq!(hud.asked, vec![(1, "Cannot open from this side".to_string(), Buttons::Ok)]);
    assert_eq!(d.len(), 1);
    // a message has no waiter: it is never abandoned for silence
    for _ in 0..(ABANDON_FRAMES + 10) {
        d.step(&mut hud, true);
    }
    assert!(hud.cancelled.is_empty() && d.len() == 1);
    hud.answers.push((1, true));
    d.step(&mut hud, true);
    assert!(d.is_empty(), "acknowledged");
    assert_eq!(d.waiter(5, 4010, [0; 3]), Wait::Game, "no waiter is ever taken for a message");
}

/// No text, or KH3's window not available: the game's own box (its case runs unchanged).
#[test]
fn er_popups_no_text_or_no_window_gives_the_game_its_box() {
    let mut d = Dialogs::default();
    assert_eq!(d.instruction(evergaol(), None, true), Route::Game);
    assert_eq!(d.instruction(evergaol(), Some("  ".into()), true), Route::Game);
    assert_eq!(d.instruction(door_message(), Some("Cannot open from this side".into()), false), Route::Game);
    assert!(d.is_empty());
    assert_eq!(d.waiter(9, 20300, evergaol().flags), Wait::Game, "the game's own waiter: the game's update");
}

/// A question whose waiter stops asking while the world runs (its event ended: a load, the map unloaded) is taken off KH3's
/// window; while a KH window holds the world nothing is counted.
#[test]
fn er_popups_a_question_whose_event_ended_is_taken_off() {
    let (mut d, mut hud) = (Dialogs::default(), MockHud::default());
    d.instruction(evergaol(), Some("Enter evergaol?".into()), true);
    d.waiter(7, 20300, evergaol().flags);
    for _ in 0..(ABANDON_FRAMES * 3) {
        d.step(&mut hud, false);
    }
    assert!(hud.cancelled.is_empty() && d.len() == 1, "the world held: never abandoned");
    for _ in 0..=ABANDON_FRAMES {
        d.step(&mut hud, true);
    }
    assert_eq!(hud.cancelled, vec![1]);
    assert!(d.is_empty());
}

/// Two questions with the same text and flags (two evergaols cannot, but the rule is general): the waiters are bound in
/// the order the questions were asked, and each takes its own answer.
#[test]
fn er_popups_waiters_bind_in_order() {
    let (mut d, mut hud) = (Dialogs::default(), MockHud::default());
    d.instruction(evergaol(), Some("Enter evergaol?".into()), true);
    d.instruction(evergaol(), Some("Enter evergaol?".into()), true);
    assert_eq!(d.waiter(0x100, 20300, evergaol().flags), Wait::Hold);
    assert_eq!(d.waiter(0x200, 20300, evergaol().flags), Wait::Hold);
    assert_eq!(d.waiter(0x300, 20300, evergaol().flags), Wait::Game, "a third waiter is the game's");
    d.step(&mut hud, false);
    hud.answers.push((2, true));
    hud.answers.push((1, false));
    d.step(&mut hud, false);
    assert_eq!(d.waiter(0x200, 20300, evergaol().flags), Wait::Done(Some(1_042_362_800)));
    assert_eq!(d.waiter(0x100, 20300, evergaol().flags), Wait::Done(Some(1_042_362_801)));
    // a waiter of another question (other msg / flags) is never bound to ours
    d.instruction(evergaol(), Some("Enter evergaol?".into()), true);
    assert_eq!(d.waiter(0x400, 4300, evergaol().flags), Wait::Game);
    assert_eq!(d.waiter(0x400, 20300, [1, 2, 3]), Wait::Game);
}

/// (THE INVENTORY) Every event-script box of the game's scripts (build/sora_tmp/er_popups/inventory.csv, written by
/// tools/inventory.js from every EMEVD): each 2007[1] / 2007[10] row with a text routes to KH3's window by its TYPE, with
/// the buttons of its type, and each question's three answers set the row's own flags. Skipped without the file.
#[test]
fn er_popups_every_box_of_the_inventory_routes_by_type() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/er_popups/inventory.csv");
    let Ok(csv) = std::fs::read_to_string(&path) else {
        println!("SKIPPED: no {}", path.display());
        return;
    };
    let mut header: Vec<String> = Vec::new();
    let (mut questions, mut messages) = (0, 0);
    for line in csv.lines() {
        let cells: Vec<String> = split_csv(line);
        if header.is_empty() {
            header = cells;
            continue;
        }
        let get = |k: &str| header.iter().position(|h| h == k).and_then(|i| cells.get(i)).cloned().unwrap_or_default();
        let instr = match get("source").as_str() {
            "EMEVD 2007[1]" => INSTR_MESSAGE,
            "EMEVD 2007[10]" => INSTR_QUESTION,
            _ => continue,
        };
        let num = |k: &str| get(k).parse::<i64>().unwrap_or(0);
        let text = get("text_en");
        let mut a = bytes(&[num("text_id") as i32]);
        a.extend_from_slice(&(num("prompt") as i16).to_le_bytes());
        a.extend_from_slice(&(num("buttons") as i16).to_le_bytes());
        a.extend(bytes(&[0, 0x4040_0000, num("flag_left") as u32 as i32, num("flag_right") as u32 as i32, num("flag_cancel") as u32 as i32]));
        let req = Request::parse(instr, &a).expect("parse");
        let (mut d, mut hud) = (Dialogs::default(), MockHud::default());
        let route = d.instruction(req, Some(text.clone()).filter(|t| !t.is_empty()), true);
        if text.is_empty() {
            assert_eq!(route, Route::Game, "{line}");
            continue;
        }
        d.step(&mut hud, true);
        let want = if req.buttons == 2 { Buttons::YesNo } else { Buttons::Ok };
        assert_eq!(hud.asked.first().map(|a| a.2), Some(want), "{line}");
        assert_eq!(get("kh_buttons"), if want == Buttons::YesNo { "Yes/No" } else { "OK" }, "{line}");
        if instr == INSTR_MESSAGE {
            assert_eq!(route, Route::Handled, "{line}");
            messages += 1;
            continue;
        }
        assert_eq!(route, Route::Suppress, "{line}");
        hud.answers.push((1, true));
        d.step(&mut hud, true);
        assert_eq!(d.waiter(1, req.msg, req.flags), Wait::Done(req.flag_for(Answer::Left)), "{line}");
        questions += 1;
    }
    println!("inventory: {questions} questions, {messages} messages routed");
    assert!(questions >= 20 && messages >= 50, "{questions} / {messages}");
}

/// A CSV line's cells (quoted cells may hold commas and doubled quotes).
fn split_csv(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                cell.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => out.push(std::mem::take(&mut cell)),
            _ => cell.push(c),
        }
    }
    out.push(cell);
    out
}
