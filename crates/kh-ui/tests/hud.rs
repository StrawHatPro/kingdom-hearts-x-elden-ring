//! The checks of docs/sora-ui-spec.md 11b (tier 1) against the ported movies and drivers.
//! Pictures of the states go to build/sora_tmp/hud/t_*.png (to be looked at).

use std::path::Path;

use kh_ui::command::{CommandDisplay, CommandModel, CommandStrings, MenuState};
use kh_ui::command_debug::DebugCommandSystem;
use kh_ui::def::write_png;
use kh_ui::hud_command::HudCommand;
use kh_ui::hud_status::{HudStatus, SlotValues, StatusDriver, StatusValues};
use kh_ui::{Canvas, Fonts, Matrix, Movie, Pack, Renderer, UiSound};

const DT: f32 = 1.0 / 60.0;

fn pack() -> Option<Pack> {
    match Pack::open_default() {
        Ok(pack) => Some(pack),
        Err(_) => {
            eprintln!("SKIPPED: no build/sora_pack/ui");
            None
        }
    }
}

fn shot(name: &str, movies: &[&Movie], fonts: &Fonts) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/hud");
    std::fs::create_dir_all(&dir).ok();
    let mut canvas = Canvas::new(1920, 1080);
    let mut renderer = Renderer::new(fonts.clone());
    for movie in movies {
        renderer.draw(&mut canvas, movie, &Matrix::IDENTITY);
    }
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|_, y| [0.30 + 0.1 * (y as f32 / 1080.0), 0.38, 0.46], &mut rgba);
    write_png(&dir.join(format!("t_{name}.png")), 1920, 1080, &rgba).unwrap();
}

fn values(hp: i32, hp_max: i32, mp: i32, mp_max: i32, focus: i32) -> StatusValues {
    let mut v = StatusValues { focus, focus_max: 100, mp_gauge_unlocked: true, focus_gauge_unlocked: true, ..StatusValues::default() };
    v.slots[0] = SlotValues { active: true, hp, hp_max, mp, mp_max, ..SlotValues::default() };
    v
}

struct Status {
    hud: HudStatus,
    driver: StatusDriver,
    sounds: Vec<UiSound>,
    time: f32,
}

impl Status {
    fn new(pack: &mut Pack) -> Status {
        let mut hud = HudStatus::new(pack.movie("HudStatus").unwrap());
        let mut driver = StatusDriver::default();
        driver.faces[0] = Some(pack.movie("FacePcExSora01").unwrap());
        hud.set_status(0, 0);
        Status { hud, driver, sounds: Vec::new(), time: 0.0 }
    }

    /// One frame: the native update, then the stage frame.
    fn frame(&mut self, v: &StatusValues) {
        self.driver.update(&mut self.hud, v, DT, &mut self.sounds);
        self.hud.tick(DT);
        self.time += DT;
    }

    fn run(&mut self, v: &StatusValues, seconds: f32) {
        for _ in 0..(seconds * 60.0).round() as usize {
            self.frame(v);
        }
    }
}

#[test]
fn status_gauge_frames_at_level_99() {
    let Some(mut pack) = pack() else { return };
    let fonts = Fonts::load(&mut pack).unwrap();
    let mut s = Status::new(&mut pack);
    let v = values(250, 250, 179, 179, 60);
    s.frame(&v);
    // setSolo, the slot's in, base 501, full 501 with the red gauge on it, MP base 323.
    let calls = s.hud.calls.join(" | ");
    for expected in ["setSolo(true)", "loadFace(0, Some(\"FacePcExSora01\"), true)", "setStatus(10, 0)", "setHPMax(0, 501)", "setHP(0, 501, 501)", "setFaceLook(0, 0)", "setMPMax(0, 323)", "setMP(0, 323)", "setFocusGauge(61, 61, false, false)", "setPcPartsVisible(1, true)", "setPcPartsVisible(2, true)"] {
        assert!(calls.contains(expected), "missing {expected} in: {calls}");
    }
    assert_eq!(s.driver.frames(), (501, 501, 323, 323, 61));
    assert_eq!((s.hud.hp_frame(0), s.hud.hp_red_frame(0)), (501, 501));
    s.run(&v, 1.5);
    shot("status_full", &[&s.hud.movie], &fonts);
    // Nothing more is sent while nothing changes.
    let sent = s.hud.calls.len();
    s.run(&v, 0.5);
    assert_eq!(s.hud.calls.len(), sent);
    // HP 125 -> 251, HP 1 -> 3 (never frame 1 while alive), HP 0 -> 1.
    for (hp, frame) in [(125, 251), (1, 3), (0, 1)] {
        s.frame(&values(hp, 250, 179, 179, 60));
        assert_eq!(s.hud.hp_frame(0), frame, "hp {hp}");
    }
    // Focus 100 / 100 -> 101 with isMax.
    s.frame(&values(250, 250, 179, 179, 100));
    assert!(s.hud.calls.last().unwrap().starts_with("setFocusGauge(101, 101, true"), "{:?}", s.hud.calls.last());
    assert_eq!(s.hud.movie.current_label("sttsPc.pcEx.pcFocusGg"), Some("max_in"));
    // Max HP 280 -> base 561; 139 -> 279; 400 -> 669.
    for (max, base) in [(280, 561), (139, 279), (400, 669)] {
        s.frame(&values(max, max, 179, 179, 60));
        assert_eq!(s.driver.frames().0, base);
        assert_eq!(s.hud.movie.current_frame("sttsPc.pcHp.pcHpPack.pcHpGgSet.pcHpDodai"), Some(base as u32));
    }
    assert!(s.hud.movie.misses.is_empty(), "paths not found: {:?}", s.hud.movie.misses);
}

#[test]
fn red_lag_follows_after_800_ms_in_ten_ticks() {
    let Some(mut pack) = pack() else { return };
    let fonts = Fonts::load(&mut pack).unwrap();
    let mut s = Status::new(&mut pack);
    s.run(&values(250, 250, 179, 179, 60), 0.5);
    // The hit: 250 -> 180 at t = 0.
    let hit = values(180, 250, 179, 179, 60);
    s.time = 0.0;
    s.frame(&hit);
    assert!(s.hud.calls.iter().any(|c| c == "setHP(0, 361, 501)"));
    assert_eq!((s.hud.hp_frame(0), s.hud.hp_red_frame(0)), (361, 501), "fill jumps at once, red stays");
    let (mut first_move, mut done, mut steps) = (None, None, Vec::new());
    let mut last = 501;
    while s.time < 2.0 {
        s.frame(&hit);
        let red = s.hud.hp_red_frame(0);
        if red != last {
            steps.push(last as i32 - red as i32);
            first_move.get_or_insert(s.time);
            last = red;
            if red == 361 {
                done.get_or_insert(s.time);
            }
        }
        if (s.time - 0.95).abs() < DT / 2.0 {
            shot("status_red_lag", &[&s.hud.movie], &fonts);
        }
    }
    let (first_move, done) = (first_move.unwrap(), done.unwrap());
    // 25 timer ticks of 33.3 ms pass the 800 ms delay (832.5 ms); 10 steps of 14 frames end at
    // 34 ticks = 1.132 s. The stage frame quantises both to 1/60 s.
    assert!(first_move > 0.80 && first_move < 0.87, "first move at {first_move}");
    assert!(done > 1.10 && done < 1.17, "done at {done}");
    assert_eq!(steps, vec![14; 10]);
    // A heal: the red gauge jumps with the fill.
    s.frame(&values(230, 250, 179, 179, 60));
    assert_eq!((s.hud.hp_frame(0), s.hud.hp_red_frame(0)), (461, 461));
    // A second hit during the follow: the red gauge continues from where it is.
    s.frame(&values(200, 250, 179, 179, 60));
    s.run(&values(200, 250, 179, 179, 60), 0.95);
    let mid = s.hud.hp_red_frame(0);
    assert!(mid < 461 && mid > 401, "following: {mid}");
    s.frame(&values(150, 250, 179, 179, 60));
    assert_eq!(s.hud.hp_red_frame(0), mid, "the red position is kept");
    s.run(&values(150, 250, 179, 179, 60), 1.0);
    assert_eq!(s.hud.hp_red_frame(0), 301);
}

#[test]
fn pinch_shows_the_dying_portrait_and_requests_the_alarm() {
    let Some(mut pack) = pack() else { return };
    let fonts = Fonts::load(&mut pack).unwrap();
    let mut s = Status::new(&mut pack);
    s.run(&values(250, 250, 179, 179, 60), 0.2);
    let pinch = values(62, 250, 179, 179, 60);
    s.time = 0.0;
    s.frame(&pinch);
    assert_eq!(s.hud.hp_frame(0), 125);
    assert!(s.hud.calls.iter().any(|c| c == "setFaceLook(0, 2)"));
    assert!(s.sounds.is_empty(), "the alarm waits 0.8 s and 15 ticks");
    s.run(&pinch, 0.7);
    assert!(s.sounds.is_empty());
    s.run(&pinch, 0.2);
    assert_eq!(s.sounds, vec![UiSound::LoopStart(9)]);
    s.run(&pinch, 1.0);
    shot("status_pinch", &[&s.hud.movie], &fonts);
    // HP 63: off.
    s.frame(&values(63, 250, 179, 179, 60));
    assert_eq!(s.sounds, vec![UiSound::LoopStart(9), UiSound::LoopStop(9)]);
    assert!(s.hud.calls.last().unwrap().starts_with("setFaceLook(0, 0)") || s.hud.calls.iter().rev().take(3).any(|c| c == "setFaceLook(0, 0)"));
    // Battle: look 1; pinch wins over battle.
    let mut battle = values(250, 250, 179, 179, 60);
    battle.game_in_battle = true;
    s.frame(&battle);
    assert!(s.hud.calls.iter().rev().take(4).any(|c| c == "setFaceLook(0, 1)"));
    // Max HP 139: pinch at HP <= 34.
    assert!(kh_ui::frames::is_pinch(34, 139) && !kh_ui::frames::is_pinch(35, 139));
}

#[test]
fn mp_charge_skin() {
    let Some(mut pack) = pack() else { return };
    let fonts = Fonts::load(&mut pack).unwrap();
    let mut s = Status::new(&mut pack);
    s.run(&values(250, 250, 179, 179, 60), 0.2);
    let mut charge = values(250, 250, 0, 179, 60);
    charge.slots[0].mp_charging = true;
    charge.slots[0].mp_charge_value = 60.0;
    s.frame(&charge);
    let calls = s.hud.calls.join(" | ");
    assert!(calls.contains("setMP(0, 1)") && calls.contains("setMPChargeMode(true, false)"), "{calls}");
    // F(60, 179, 322) + 1 = trunc(60 * 322 / 179 + 0.5) + 1 = 109.
    assert!(calls.contains("setMPCharge(109)"), "{calls}");
    assert_eq!(s.hud.movie.current_label("sttsPc.pcMp.pcMpPack"), Some("charge_in"));
    // charge_in runs 76 frames (1.27 s), then charge_loop.
    s.run(&charge, 1.5);
    assert_eq!(s.hud.movie.current_label("sttsPc.pcMp.pcMpPack"), Some("charge_loop"));
    shot("status_mp_charge", &[&s.hud.movie], &fonts);
    s.frame(&values(250, 250, 179, 179, 60));
    let calls = s.hud.calls.join(" | ");
    assert!(calls.ends_with("setMPChargeMode(false, false) | setMPCharge(0)") || calls.contains("setMPChargeMode(false, false) | setMPCharge(0)"), "{calls}");
    assert_eq!(s.hud.movie.current_label("sttsPc.pcMp.pcMpPack"), Some("charge_out"));
}

struct Menu {
    hud: HudCommand,
    display: CommandDisplay,
    system: DebugCommandSystem,
    strings: CommandStrings,
    sounds: Vec<UiSound>,
}

impl Menu {
    fn new(pack: &mut Pack) -> Menu {
        let fonts = Fonts::load(pack).unwrap();
        let hud = HudCommand::new(pack.movie("HudCommand").unwrap(), fonts);
        let strings = CommandStrings::load(pack);
        let mut system = DebugCommandSystem::new(Some(pack));
        system.set_status(179, 179, false);
        Menu { hud, display: CommandDisplay::new(), system, strings, sounds: Vec::new() }
    }

    /// One frame; `words` are debug verbs applied in it.
    fn frame(&mut self, words: &[&str]) {
        self.system.begin_frame();
        for w in words {
            self.system.command(w).unwrap();
        }
        self.system.step(DT);
        self.display.update(&mut self.hud, &self.system.model, &self.strings, DT, &mut self.sounds);
        self.hud.tick();
    }

    fn run(&mut self, seconds: f32) {
        for _ in 0..(seconds * 60.0).round() as usize {
            self.frame(&[]);
        }
    }

    fn calls_since(&self, mark: usize) -> String {
        self.hud.calls[mark..].join(" | ")
    }
}

#[test]
fn command_menu_main_slots() {
    let Some(mut pack) = pack() else { return };
    let mut m = Menu::new(&mut pack);
    assert_eq!(m.system.model.state, MenuState::Wait);
    m.frame(&["show"]);
    let calls = m.calls_since(0);
    assert!(calls.contains("initMainCommand(\"Attack\", \"Magic\", \"Items\", \"Link\")") && calls.contains("setState(0, true)") && calls.contains("setBattleMode(0, false)"), "{calls}");
    m.run(1.0);
    let fonts = m.hud.fonts.clone();
    shot("command_main", &[&m.hud.movie], &fonts);
    assert_eq!(m.hud.movie.text("cmdMenu.cmdMainTop.cmdMain.cmdMain2.cmdText1.txCmdName.txCmdNameCol.coCmdName").as_deref(), Some("Magic"));
    assert_eq!(m.hud.movie.current_label("cmdMenu.cmdMainTop.cmdMain"), Some("loop"));
    assert_eq!(m.hud.movie.current_label("cmdMenu.cmdMainTop.cmdMain.cmdMain1.cmdText1"), Some("on"));
    // The cursor wraps over 0 .. 3 and sounds static SE 0 when it moves.
    m.sounds.clear();
    let mut cursors = Vec::new();
    for _ in 0..5 {
        let mark = m.hud.calls.len();
        m.frame(&["down"]);
        cursors.push(m.calls_since(mark));
        m.run(0.2);
    }
    assert_eq!(cursors, ["selectMainCommand(1, -1, false)", "selectMainCommand(2, -1, false)", "selectMainCommand(3, -1, false)", "selectMainCommand(0, -1, false)", "selectMainCommand(1, -1, false)"]);
    assert_eq!(m.sounds, vec![UiSound::Play(0); 5]);
    m.frame(&["up"]);
    assert_eq!(m.hud.main_cursor(), 0);
    // Battle colours.
    for (mode, label) in [(2, "cmdRed"), (1, "cmdYellow"), (0, "cmdBlue")] {
        m.frame(&[&format!("battle {mode}")]);
        assert_eq!(m.hud.movie.current_label("cmdMenu.cmdMainTop"), Some(label));
        assert_eq!(m.hud.movie.current_label("cmdMenu.cmdTitleTop.cmdTitle.cmdTitleName"), Some(label));
        m.run(0.3);
        if mode == 2 {
            shot("command_battle", &[&m.hud.movie], &fonts);
        }
    }
    // Without Link: three slots.
    let mark = m.hud.calls.len();
    m.frame(&["link off"]);
    assert!(m.calls_since(mark).contains("SetLinkCommandNone(true)"));
    m.frame(&["up"]);
    assert_eq!(m.hud.main_cursor(), 2, "wraps over 0 .. 2 without Link");
    assert!(m.hud.movie.misses.is_empty(), "paths not found: {:?}", m.hud.movie.misses);
}

#[test]
fn command_menu_sub_list_shortcuts_and_numbers() {
    let Some(mut pack) = pack() else { return };
    let mut m = Menu::new(&mut pack);
    let fonts = m.hud.fonts.clone();
    m.system.set_list(1, &[("Fire", 10, 0, false), ("Blizzard", 12, 0, false), ("Thunder", 14, 0, false)]);
    m.frame(&["show"]);
    m.run(0.8);
    // Open Magic with 3 rows.
    m.sounds.clear();
    let mark = m.hud.calls.len();
    m.frame(&["down", "confirm"]);
    let calls = m.calls_since(mark);
    assert!(calls.contains("initSubCommand(1, 3)") && calls.contains("setSubCommandText([\"Fire\", \"Blizzard\", \"Thunder\"])") && calls.contains("setSubCommand(0, 0)"), "{calls}");
    assert_eq!(m.sounds, vec![UiSound::Play(6)]);
    // Bottom aligned: rows use cmdSub1_6 .. 8.
    let rows: Vec<String> = (0..3).map(|r| m.hud.sub_row_clip(1, r).rsplit('.').next().unwrap().to_string()).collect();
    assert_eq!(rows, ["cmdSub1_6", "cmdSub1_7", "cmdSub1_8"]);
    m.run(0.6);
    shot("command_sub_magic", &[&m.hud.movie], &fonts);
    assert_eq!(m.hud.movie.current_label("cmdMenu.cmdSubTop"), Some("mahou"));
    // Cursor down three times wraps to row 0, SE 0 each.
    m.sounds.clear();
    let mut seen = Vec::new();
    for _ in 0..3 {
        let mark = m.hud.calls.len();
        m.frame(&["down"]);
        seen.push(m.calls_since(mark));
        m.run(0.1);
    }
    assert_eq!(seen, ["selectSubCommand(1)", "selectSubCommand(2)", "selectSubCommand(0)"]);
    assert_eq!(m.sounds, vec![UiSound::Play(0); 3]);
    // MP Charge: every magic row and the Magic slot turn `disabled`.
    m.system.set_status(0, 179, true);
    m.frame(&[]);
    for row in 0..3 {
        assert_eq!(m.hud.movie.current_label(&format!("{}.cmdText1", m.hud.sub_row_clip(1, row))), Some("disabled"), "row {row}");
    }
    m.run(0.3);
    shot("command_sub_mp_charge", &[&m.hud.movie], &fonts);
    // Cancel: setSubCommand(2, 0), SE 3; the Magic slot is greyed.
    m.sounds.clear();
    let mark = m.hud.calls.len();
    m.frame(&["cancel"]);
    assert!(m.calls_since(mark).contains("setSubCommand(2, 0)"));
    assert_eq!(m.sounds, vec![UiSound::Play(3)]);
    m.run(0.5);
    assert_eq!(m.hud.movie.current_label("cmdMenu.cmdMainTop.cmdMain.cmdMain2.cmdText1"), Some("disabled"));
    assert_eq!(m.hud.main_cursor(), 0);
    // A refused confirm on the greyed slot: SE 4.
    m.sounds.clear();
    m.frame(&["down"]);
    m.frame(&["confirm"]);
    assert_eq!(m.sounds, vec![UiSound::Play(0), UiSound::Play(4)]);
    m.system.set_status(20, 179, false);
    m.frame(&["up"]);
    m.run(0.3);
    // Low MP does not disable, it recolours (col2).
    m.system.set_list(1, &[("Fire", 10, 0, false), ("Firaga", 40, 0, false)]);
    m.frame(&["open magic"]);
    m.run(0.5);
    assert_eq!(m.hud.movie.current_label(&format!("{}.cmdText1.txCmdName", m.hud.sub_row_clip(1, 1))), Some("col2"));
    assert_eq!(m.hud.movie.current_label(&format!("{}.cmdText1.txCmdName", m.hud.sub_row_clip(1, 0))), Some("col1"));
    m.frame(&["left"]);
    assert!(m.hud.calls.last().unwrap().starts_with("setMainCommand(5)") || m.hud.calls.iter().rev().take(3).any(|c| c == "setSubCommand(1, 0)"));
    m.run(0.5);
    assert_eq!(m.hud.main_cursor(), 1, "left keeps the main cursor");
    // Items: Potion x3 shows "x3", x1 nothing, x12 "x9".
    let mark = m.hud.calls.len();
    m.frame(&["open items"]);
    let calls = m.calls_since(mark);
    assert!(calls.contains("setSubCommandNumText(2, 4, false, [\"x3\", \"\", \"x9\", \"\"])"), "{calls}");
    m.run(0.6);
    shot("command_sub_items", &[&m.hud.movie], &fonts);
    m.frame(&["cancel"]);
    m.run(0.5);
    // Shortcut held: setModeShortcut(0, page); D-pad down: page + 1 with SE 6.
    m.sounds.clear();
    let mark = m.hud.calls.len();
    m.frame(&["shortcut on"]);
    let calls = m.calls_since(mark);
    assert!(calls.contains("setShortcutText(") && calls.contains("setModeShortcut(0, 0)") && calls.contains("setActionSelect(4, 0, 0)"), "{calls}");
    m.run(0.6);
    shot("command_shortcut", &[&m.hud.movie], &fonts);
    let mark = m.hud.calls.len();
    m.frame(&["page down"]);
    assert!(m.calls_since(mark).contains("setModeShortcut(3, 1)"));
    assert_eq!(m.sounds, vec![UiSound::Play(6)]);
    m.run(0.3);
    m.frame(&["page up"]);
    assert!(m.hud.calls.iter().rev().take(4).any(|c| c == "setModeShortcut(4, 0)"));
    m.run(0.3);
    let mark = m.hud.calls.len();
    m.frame(&["shortcut off"]);
    let calls = m.calls_since(mark);
    assert!(calls.contains("setModeShortcut(1, 0)") && calls.contains("setActionSelect(5, 0, 0)"), "{calls}");
    m.run(0.6);
    assert_eq!(m.hud.movie.current_label("cmdMenu.cmdMainTop.cmdMain"), Some("loop"));
    assert!(m.hud.movie.misses.is_empty(), "paths not found: {:?}", m.hud.movie.misses);
}

#[test]
fn situation_command_timer_display() {
    let Some(mut pack) = pack() else { return };
    let mut m = Menu::new(&mut pack);
    let fonts = m.hud.fonts.clone();
    m.frame(&["show"]);
    m.run(0.8);
    let mark = m.hud.calls.len();
    m.frame(&["situation add Second_Form 20 1"]);
    let calls = m.calls_since(mark);
    assert!(calls.contains("addActionSelectButton(0, \"Second Form\", 1, 0, true)") && calls.contains("setActionCommandMark(0, true, false, true)"), "{calls}");
    // Nothing counts during the first 0.5 s.
    m.run(0.4);
    assert_eq!(m.system.model.situations[0].remaining, 20.0);
    let counter = "cmdMenu.cmdActTop.cmdActAll.cmdActState.cmdActPos1.cmdAct.cmdAct1";
    assert_eq!(m.hud.movie.current_frame(&format!("{counter}.actGgAnm")), Some(5), "full = frame 5");
    // 12.34 s of 20 s: digits 12 and 3, counter `loop`, gauge frame 87.
    m.system.model.situations[0].remaining = 12.34 + DT;
    m.frame(&[]);
    let number = |m: &Menu, name: &str| m.hud.movie.sprite(&format!("{counter}.actCount.{name}")).unwrap().number.as_ref().unwrap().number;
    assert_eq!((number(&m, "actCountS"), number(&m, "actCountSs")), (12, 3));
    assert_eq!(m.hud.movie.current_label(&format!("{counter}.actCount")), Some("loop"));
    assert_eq!(m.hud.movie.current_frame(&format!("{counter}.actGgAnm")), Some(87));
    m.run(0.3);
    shot("command_situation", &[&m.hud.movie], &fonts);
    // At 9.99 s the counter turns red.
    m.system.model.situations[0].remaining = 9.99 + DT;
    m.frame(&[]);
    assert_eq!(m.hud.movie.current_label(&format!("{counter}.actCount")), Some("red"));
    // A second entry goes above; the left trigger switches with SE 0; used -> decide out.
    m.frame(&["situation add Blizzaza 15 3"]);
    m.run(0.3);
    m.sounds.clear();
    let mark = m.hud.calls.len();
    m.frame(&["situation switch"]);
    assert!(m.calls_since(mark).contains("setActionCommandMark(1, true, false, true)"));
    assert_eq!(m.sounds, vec![UiSound::Play(0)]);
    m.run(0.3);
    shot("command_situation2", &[&m.hud.movie], &fonts);
    let mark = m.hud.calls.len();
    m.frame(&["situation use"]);
    assert!(m.calls_since(mark).contains("deleteActionSelectButton(1, true, false)"), "{}", m.calls_since(mark));
    // Expiry: `out`.
    m.system.model.situations[0].remaining = 0.01;
    let mark = m.hud.calls.len();
    m.frame(&[]);
    assert!(m.calls_since(mark).contains("deleteActionSelectButton(0, false, false)"), "{}", m.calls_since(mark));
    // Shotlock panel.
    m.run(0.5);
    let mark = m.hud.calls.len();
    m.frame(&["shotlock on"]);
    let calls = m.calls_since(mark);
    assert!(calls.contains("setActionSelect(4, 0, 0)") && calls.contains("setShootFlow(0, 2, 0, false, [(\"Shotlocks\", \"<icon(Cross)>\"), (\"Airstep\", \"<icon(Square)>\")])"), "{calls}");
    m.run(0.6);
    shot("command_shotlock", &[&m.hud.movie], &fonts);
    assert_eq!(m.hud.movie.current_label("cmdMenu.cmdTitleTop.cmdTitle.cmdTitleName"), Some("cmdSlock"));
    m.frame(&["shotlock off"]);
    m.run(0.6);
    assert_eq!(m.hud.movie.current_label("cmdMenu.cmdMainTop.cmdMain"), Some("loop"));
    assert!(m.hud.movie.misses.is_empty(), "paths not found: {:?}", m.hud.movie.misses);
}

#[test]
fn sneeze_replaces_the_attack_text_and_gives_it_back() {
    // UI spec 5.3: setMainCommandTextBadStatus(0, text, on) (0x141618290).
    let Some(mut pack) = pack() else { return };
    let fonts = Fonts::load(&mut pack).unwrap();
    let mut hud = HudCommand::new(pack.movie("HudCommand").unwrap(), fonts);
    let mut display = CommandDisplay::new();
    let mut model = CommandModel { state: MenuState::MainSel, ..CommandModel::default() };
    for (slot, text) in ["Attack", "Magic", "Items", ""].into_iter().enumerate() {
        model.main[slot] = kh_ui::command::MenuRow::new(text);
    }
    let mut sounds = Vec::new();
    display.update(&mut hud, &model, &CommandStrings::default(), DT, &mut sounds);
    assert!(!hud.calls.iter().any(|c| c.starts_with("setMainCommandTextBadStatus")), "{:?}", hud.calls);
    model.attack_bad_status = Some("Sneeze".to_string());
    display.update(&mut hud, &model, &CommandStrings::default(), DT, &mut sounds);
    display.update(&mut hud, &model, &CommandStrings::default(), DT, &mut sounds);
    let on: Vec<_> = hud.calls.iter().filter(|c| c.starts_with("setMainCommandTextBadStatus")).collect();
    assert_eq!(on, ["setMainCommandTextBadStatus(0, \"Sneeze\", 1)"]);
    model.attack_bad_status = None;
    display.update(&mut hud, &model, &CommandStrings::default(), DT, &mut sounds);
    assert_eq!(hud.calls.last().map(String::as_str), Some("setMainCommandTextBadStatus(0, \"Attack\", 0)"), "{:?}", hud.calls);
}

#[test]
fn the_model_alone_drives_the_display() {
    // A command system publishes a model; no debug system is needed.
    let Some(mut pack) = pack() else { return };
    let fonts = Fonts::load(&mut pack).unwrap();
    let mut hud = HudCommand::new(pack.movie("HudCommand").unwrap(), fonts);
    let mut display = CommandDisplay::new();
    let mut model = CommandModel { state: MenuState::MainSel, ..CommandModel::default() };
    for (slot, text) in ["Attack", "Magic", "Items", ""].into_iter().enumerate() {
        model.main[slot] = kh_ui::command::MenuRow::new(text);
    }
    let mut sounds = Vec::new();
    display.update(&mut hud, &model, &CommandStrings::default(), DT, &mut sounds);
    // A slot whose list is not available shows "? ? ?".
    assert!(hud.calls.iter().any(|c| c == "initMainCommand(\"Attack\", \"Magic\", \"Items\", \"? ? ?\")"), "{:?}", hud.calls);
    assert!(sounds.is_empty());
}
