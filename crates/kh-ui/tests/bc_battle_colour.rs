//! (BC) THE COMMAND MENU'S BATTLE COLOUR (THE USER 2026-11-02: "make the HUD change to this colour during combat"): KH3's
//! `setBattleMode(mode, keepTitle)` recolours the menu (`cmdMainTop` and the title plate take `cmdBlue` / `cmdYellow` /
//! `cmdRed`); the native driver picks the mode from the game's battle flags every frame (0x141631D90, docs/sora-ui-spec.md
//! 5.3). Pictures of the menu in and out of combat go to build/sora_tmp/hud/t_bc_*.png (to be looked at).

use std::path::Path;

use kh_ui::command::{CommandDisplay, CommandModel, CommandStrings, MenuRow, MenuState};
use kh_ui::def::write_png;
use kh_ui::hud_command::HudCommand;
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

/// The whole screen, and the menu's corner twice as large; returns the screen's pixels.
fn shot(name: &str, movie: &Movie, fonts: &Fonts) -> Vec<u8> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/hud");
    std::fs::create_dir_all(&dir).ok();
    let mut canvas = Canvas::new(1920, 1080);
    Renderer::new(fonts.clone()).draw(&mut canvas, movie, &Matrix::IDENTITY);
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|_, y| [0.30 + 0.1 * (y as f32 / 1080.0), 0.38, 0.46], &mut rgba);
    write_png(&dir.join(format!("t_{name}.png")), 1920, 1080, &rgba).unwrap();
    let (x0, y0, w, h, k) = (0usize, 700usize, 520usize, 380usize, 2usize);
    let mut zoom = vec![0u8; w * k * h * k * 4];
    for y in 0..h * k {
        for x in 0..w * k {
            let s = ((y0 + y / k) * 1920 + x0 + x / k) * 4;
            let d = (y * w * k + x) * 4;
            zoom[d..d + 4].copy_from_slice(&rgba[s..s + 4]);
        }
    }
    write_png(&dir.join(format!("t_{name}_zoom.png")), (w * k) as u32, (h * k) as u32, &zoom).unwrap();
    rgba
}

fn row(text: &str) -> MenuRow {
    MenuRow { text: text.to_string(), enabled: true, ..MenuRow::default() }
}

/// The mean colour of the selected Attack bar's left end (stage pixels 10 .. 60, 916 .. 936).
fn attack_bar(rgba: &[u8]) -> [f32; 3] {
    let mut sum = [0f32; 3];
    let mut n = 0f32;
    for y in 916..936 {
        for x in 10..60 {
            let i = (y * 1920 + x) * 4;
            for c in 0..3 {
                sum[c] += rgba[i + c] as f32;
            }
            n += 1.0;
        }
    }
    sum.map(|s| s / n)
}

#[test]
fn the_three_battle_modes_recolour_the_menu_blue_orange_gold_and_red() {
    let Some(mut pack) = pack() else { return };
    let fonts = Fonts::load(&mut pack).unwrap();
    let names = ["field", "battle", "special"];
    for mode in 0..3 {
        let mut hud = HudCommand::new(pack.movie("HudCommand").unwrap(), fonts.clone());
        let strings = CommandStrings::load(&mut pack);
        let mut model = CommandModel { state: MenuState::MainSel, battle_mode: mode, ..CommandModel::default() };
        for (slot, text) in ["Attack", "Magic", "Items", ""].into_iter().enumerate() {
            model.main[slot] = row(text);
        }
        model.main[3].enabled = false;
        model.link_none = true;
        let mut display = CommandDisplay::new();
        let mut sounds: Vec<UiSound> = Vec::new();
        for _ in 0..60 {
            display.update(&mut hud, &model, &strings, DT, &mut sounds);
            hud.tick();
        }
        let rgba = shot(&format!("bc_menu_{mode}_{}", names[mode as usize]), &hud.movie, &fonts);
        // the colour clips took the mode's label: the bases, the selected bar and the title (`cmdTitleName`)
        let want = ["cmdBlue", "cmdYellow", "cmdRed"][mode as usize];
        assert_eq!(hud.movie.current_label("cmdMenu.cmdMainTop"), Some(want));
        assert_eq!(hud.movie.current_label("cmdMenu.cmdTitleTop.cmdTitle.cmdTitleName"), Some(want));
        let [r, g, b] = attack_bar(&rgba);
        match mode {
            0 => assert!(b > r + 60.0 && b > g + 40.0, "field: KH3's blue bar {:?}", [r, g, b]),
            1 => assert!(r > g + 40.0 && g > b + 40.0 && r > 150.0, "battle: KH3's orange-gold bar {:?}", [r, g, b]),
            _ => assert!(r > g + 80.0 && r > b + 80.0 && g < 120.0, "special battle: KH3's red bar {:?}", [r, g, b]),
        }
        assert!(hud.movie.misses.is_empty(), "paths not found: {:?}", hud.movie.misses);
    }
}
