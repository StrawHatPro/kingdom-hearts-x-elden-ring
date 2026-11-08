//! (GOOFY) A RECHARGING SHORTCUT ENTRY (THE USER 2026-11-02: "Goofy's abilities (except taunt) should cost more MP, and
//! introduce a cooldown with a visible timer, they are way too spammable"): while one of his Arts waits, KH3's situation
//! command timer gauge drains on its entry's plate on the LB panel, under its lit name, and its seconds stand in the entry's
//! number field.
//! Pictures of the panel go to build/sora_tmp/hud/t_gof_recharge_*.png (to be looked at).

use std::path::Path;

use kh_ui::command::{recharge_text, CommandDisplay, CommandModel, CommandStrings, MenuRow, MenuState};
use kh_ui::def::write_png;
use kh_ui::frames::situation_gauge_frame;
use kh_ui::hud_command::HudCommand;
use kh_ui::{Canvas, Fonts, Matrix, Movie, Pack, Renderer, UiSound};

const DT: f32 = 1.0 / 60.0;
const SET: &str = "cmdMenu.cmdScutTop.cmdShort.cmdEx.cmdExTextSet";

fn pack() -> Option<Pack> {
    match Pack::open_default() {
        Ok(pack) => Some(pack),
        Err(_) => {
            eprintln!("SKIPPED: no build/sora_pack/ui");
            None
        }
    }
}

/// The whole screen, and the panel's corner three times as large (the entries read at a glance there).
fn shot(name: &str, movie: &Movie, fonts: &Fonts) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/hud");
    std::fs::create_dir_all(&dir).ok();
    let mut canvas = Canvas::new(1920, 1080);
    Renderer::new(fonts.clone()).draw(&mut canvas, movie, &Matrix::IDENTITY);
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|_, y| [0.30 + 0.1 * (y as f32 / 1080.0), 0.38, 0.46], &mut rgba);
    write_png(&dir.join(format!("t_{name}.png")), 1920, 1080, &rgba).unwrap();
    let (x0, y0, w, h, k) = (0usize, 860usize, 400usize, 200usize, 3usize);
    let mut zoom = vec![0u8; w * k * h * k * 4];
    for y in 0..h * k {
        for x in 0..w * k {
            let s = ((y0 + y / k) * 1920 + x0 + x / k) * 4;
            let d = (y * w * k + x) * 4;
            zoom[d..d + 4].copy_from_slice(&rgba[s..s + 4]);
        }
    }
    write_png(&dir.join(format!("t_{name}_zoom.png")), (w * k) as u32, (h * k) as u32, &zoom).unwrap();
}

fn row(text: &str, enabled: bool, quantity: u32, cooldown: (f32, f32)) -> MenuRow {
    MenuRow { text: text.to_string(), enabled, quantity, cooldown, ..MenuRow::default() }
}

/// Goofy's panel in the order B, Y, X, A: Goofy Charge (waiting `left` of 6 s), a Potion, Taunt, Goofy Bash (waiting `bash` of
/// 8 s), as kh-party's `panel_row` gives it: `enabled` leaves the wait out, MP Charge (`charging`) greys the MP commands.
fn panel(left: f32, bash: f32, charging: bool) -> [MenuRow; 4] {
    let cd = |l: f32, t: f32| if l > 0.0 { (l, t) } else { (0.0, 0.0) };
    [row("Goofy Charge", !charging, 0, cd(left, 6.0)), row("Potion", true, 3, (0.0, 0.0)), row("Taunt", !charging, 0, (0.0, 0.0)), row("Goofy Bash", !charging, 0, cd(bash, 8.0))]
}

struct Panel {
    hud: HudCommand,
    display: CommandDisplay,
    strings: CommandStrings,
    model: CommandModel,
    sounds: Vec<UiSound>,
}

impl Panel {
    fn frame(&mut self) {
        self.display.update(&mut self.hud, &self.model, &self.strings, DT, &mut self.sounds);
        self.hud.tick();
    }

    /// What the entry's number field shows ("": the field is off).
    fn number(&self, row: usize) -> String {
        let clip = format!("{SET}{}.cmdExText1.cmdNumber", row + 1);
        if self.hud.movie.current_label(&clip) == Some("off") {
            return String::new();
        }
        self.hud.movie.text(&format!("{clip}.cmdNumCol.txCmdNumber.cmdNumberTxt")).unwrap_or_default()
    }

    fn label(&self, row: usize) -> String {
        self.hud.movie.current_label(&format!("{SET}{}.cmdExText1", row + 1)).unwrap_or_default().to_string()
    }
}

#[test]
fn a_recharging_entry_drains_kh3s_timer_gauge_and_counts_its_seconds_down() {
    let Some(mut pack) = pack() else { return };
    let fonts = Fonts::load(&mut pack).unwrap();
    let hud = HudCommand::new(pack.movie("HudCommand").unwrap(), fonts.clone());
    let strings = CommandStrings::load(&mut pack);
    let mut model = CommandModel { state: MenuState::MainSel, battle_mode: 2, ..CommandModel::default() };
    for (slot, (text, enabled)) in [("Attack", true), ("", false), ("Items", true), ("", false)].into_iter().enumerate() {
        model.main[slot] = row(text, enabled, 0, (0.0, 0.0));
    }
    model.link_none = true;
    let mut p = Panel { hud, display: CommandDisplay::new(), strings, model, sounds: Vec::new() };
    for _ in 0..40 {
        p.frame();
    }
    // LB held: the panel opens with Goofy Charge just used (6 s) and Goofy Bash nearly back (0.6 of 8 s)
    p.model.state = MenuState::Shortcut;
    let (mut charge, mut bash) = (6.0f32, 0.6f32);
    for _ in 0..36 {
        p.model.shortcut = panel(charge, bash, false);
        p.frame();
        charge -= DT;
        bash -= DT;
    }
    // the gauges stand where the left time says, as the situation command's does
    let charge_frame = p.hud.shortcut_recharge(0).expect("Goofy Charge's gauge is on show");
    let bash_frame = p.hud.shortcut_recharge(3).expect("Goofy Bash's gauge is on show");
    let want = |l: f32, t: f32| situation_gauge_frame(l + DT, t) as u32;
    assert!(charge_frame.abs_diff(want(charge, 6.0)) <= 1, "{charge_frame} vs {}", want(charge, 6.0));
    assert!(bash_frame.abs_diff(want(bash, 8.0)) <= 1, "{bash_frame} vs {}", want(bash, 8.0));
    assert!(charge_frame < bash_frame, "more time left: less drained ({charge_frame}, {bash_frame})");
    assert_eq!((p.hud.shortcut_recharge(1), p.hud.shortcut_recharge(2)), (None, None), "the Potion and Taunt have none");
    // their whole seconds in the entries' number fields; the Potion keeps its count; the waiting entries stay lit over the gauge
    assert_eq!((p.number(0).as_str(), p.number(1).as_str(), p.number(2).as_str(), p.number(3).as_str()), ("6", "x3", "", "1"));
    assert_eq!((p.label(0).as_str(), p.label(1).as_str(), p.label(2).as_str(), p.label(3).as_str()), ("on", "on", "on", "on"));
    shot("gof_recharge_running", &p.hud.movie, &fonts);
    // Goofy Bash comes back: its gauge goes, its number empties
    for _ in 0..40 {
        p.model.shortcut = panel(charge, bash, false);
        p.frame();
        charge -= DT;
        bash -= DT;
    }
    assert!(p.hud.shortcut_recharge(0).is_some());
    assert_eq!(p.hud.shortcut_recharge(3), None, "back: no gauge");
    assert_eq!((p.number(0).as_str(), p.number(3).as_str(), p.label(3).as_str()), ("5", "", "on"));
    shot("gof_recharge_back", &p.hud.movie, &fonts);
    // MP Charge greys his MP commands; a waiting one still shows its gauge and count
    p.model.shortcut = panel(charge, 0.0, true);
    p.frame();
    assert_eq!((p.label(0).as_str(), p.label(2).as_str(), p.label(3).as_str()), ("disabled", "disabled", "disabled"));
    assert!(p.hud.shortcut_recharge(0).is_some() && p.hud.shortcut_recharge(3).is_none());
    assert_eq!(p.number(0), "5");
    // the panel shut and opened again after every wait is over: nothing of a gauge is left
    p.model.state = MenuState::MainSel;
    for _ in 0..30 {
        p.frame();
    }
    p.model.state = MenuState::Shortcut;
    p.model.shortcut = panel(0.0, 0.0, false);
    for _ in 0..30 {
        p.frame();
    }
    assert_eq!((0..4).filter_map(|r| p.hud.shortcut_recharge(r)).count(), 0);
    assert_eq!(p.number(0), "");
    assert!(p.hud.movie.misses.is_empty(), "paths not found: {:?}", p.hud.movie.misses);
}

#[test]
fn the_count_is_whole_seconds_counted_up() {
    assert_eq!(recharge_text(4.25), "5");
    assert_eq!(recharge_text(15.0), "15");
    assert_eq!(recharge_text(0.05), "1");
    assert_eq!(recharge_text(0.0), "0");
}
