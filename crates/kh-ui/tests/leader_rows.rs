//! (LEADER RESET) THE MAIN LIST'S SHAPE FOLLOWS THE MODEL BOTH WAYS (THE USER 2026-11-02: Donald's menu read "Attack /
//! Magicunement / (empty) / Items" after Goofy died and a Donald save was loaded): the display alone, with no host reset, given
//! Goofy's two-row list (Attack / Items) and then a four-row one (Donald's Attack / Magic / Attunement / Items, Sora's Attack /
//! Magic / Items / Link), puts every row back on its own line; and a model with no names at all (the title's empty snapshot)
//! never makes the two-row list. Pictures: build/sora_tmp/hud/t_leader_rows_*.png.

use std::path::Path;

use kh_ui::command::{CommandDisplay, CommandModel, CommandStrings, MenuRow, MenuState};
use kh_ui::def::write_png;
use kh_ui::hud_command::HudCommand;
use kh_ui::{Canvas, Fonts, Matrix, Movie, Pack, Renderer, UiSound};

const DT: f32 = 1.0 / 60.0;
const MAIN: &str = "cmdMenu.cmdMainTop.cmdMain";

fn pack() -> Option<Pack> {
    match Pack::open_default() {
        Ok(pack) => Some(pack),
        Err(_) => {
            eprintln!("SKIPPED: no build/sora_pack/ui");
            None
        }
    }
}

fn shot(name: &str, movie: &Movie, fonts: &Fonts) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/hud");
    std::fs::create_dir_all(&dir).ok();
    let mut canvas = Canvas::new(1920, 1080);
    Renderer::new(fonts.clone()).draw(&mut canvas, movie, &Matrix::IDENTITY);
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|_, y| [0.30 + 0.1 * (y as f32 / 1080.0), 0.38, 0.46], &mut rgba);
    let (x0, y0, w, h, k) = (0usize, 760usize, 500usize, 320usize, 2usize);
    let mut zoom = vec![0u8; w * k * h * k * 4];
    for y in 0..h * k {
        for x in 0..w * k {
            let s = ((y0 + y / k) * 1920 + x0 + x / k) * 4;
            let d = (y * w * k + x) * 4;
            zoom[d..d + 4].copy_from_slice(&rgba[s..s + 4]);
        }
    }
    write_png(&dir.join(format!("t_leader_rows_{name}.png")), (w * k) as u32, (h * k) as u32, &zoom).unwrap();
}

fn model(rows: [&str; 4], link_none: bool) -> CommandModel {
    let mut m = CommandModel { state: MenuState::MainSel, link_none, ..CommandModel::default() };
    for (slot, text) in rows.into_iter().enumerate() {
        m.main[slot] = MenuRow { text: text.to_string(), enabled: !text.is_empty(), ..MenuRow::default() };
    }
    m
}

struct Menu {
    hud: HudCommand,
    display: CommandDisplay,
    strings: CommandStrings,
    sounds: Vec<UiSound>,
}

impl Menu {
    fn run(&mut self, model: &CommandModel, n: usize) {
        for _ in 0..n {
            self.display.update(&mut self.hud, model, &self.strings, DT, &mut self.sounds);
            self.hud.tick();
        }
    }

    /// (row 1 ..= 4, text, y x10) of every row that draws.
    fn rows(&self) -> Vec<(usize, String, i32)> {
        let m = &self.hud.movie;
        (1..=4)
            .filter_map(|n| {
                let path = format!("{MAIN}.cmdMain{n}");
                let node = m.node(&path)?;
                let drawn = node.is_visible() && !matches!(m.current_label(&path), Some("off" | "out"));
                let text = m.text(&format!("{path}.cmdText1.txCmdName.txCmdNameCol.coCmdName.textField")).unwrap_or_default();
                drawn.then(|| (n, text, (node.matrix.ty * 10.0).round() as i32))
            })
            .collect()
    }
}

#[test]
fn the_rows_go_back_to_their_own_lines_after_a_two_row_list() {
    let Some(mut pack) = pack() else { return };
    let fonts = Fonts::load(&mut pack).unwrap();
    let def = pack.movie("HudCommand").unwrap();
    let strings = CommandStrings::load(&mut pack);
    let goofy = model(["Attack", "", "Items", ""], true);
    let donald = model(["Attack", "Magic", "Attunement", "Items"], false);
    let sora = model(["Attack", "Magic", "Items", "Link"], false);
    // each list on a menu of its own: the reference lines
    let fresh = |m: &CommandModel| {
        let mut menu = Menu { hud: HudCommand::new(def.clone(), fonts.clone()), display: CommandDisplay::new(), strings: strings.clone(), sounds: Vec::new() };
        menu.run(m, 120);
        menu.rows()
    };
    let (want_donald, want_sora, want_goofy) = (fresh(&donald), fresh(&sora), fresh(&goofy));
    assert_eq!(want_goofy.iter().map(|r| r.1.as_str()).collect::<Vec<_>>(), ["Attack", "Items"]);
    assert_eq!(want_goofy[1].2, want_donald[1].2, "Goofy's Items stands on the second line");
    // one menu, the lists one after the other, no reset in between
    let mut menu = Menu { hud: HudCommand::new(def.clone(), fonts.clone()), display: CommandDisplay::new(), strings: strings.clone(), sounds: Vec::new() };
    menu.run(&goofy, 120);
    assert_eq!(menu.rows(), want_goofy);
    menu.run(&donald, 120);
    shot("goofy_then_donald", &menu.hud.movie, &fonts);
    assert_eq!(menu.rows(), want_donald, "Goofy's list, then Donald's: Attunement on its own line, no empty line");
    menu.run(&goofy, 120);
    assert_eq!(menu.rows(), want_goofy);
    menu.run(&sora, 120);
    shot("goofy_then_sora", &menu.hud.movie, &fonts);
    assert_eq!(menu.rows(), want_sora, "Goofy's list, then Sora's");
    // the title's empty snapshot (no names at all) between two four-row lists: no two-row list
    menu.run(&model(["", "", "", ""], true), 60);
    menu.run(&donald, 120);
    shot("empty_then_donald", &menu.hud.movie, &fonts);
    assert_eq!(menu.rows(), want_donald, "an empty model, then Donald's");
}
