//! (F-HUD) EVERY FORM LOGO DRAWS CLEANLY: each `CHANGE_*` row of MainCommandData (the 30 forms, Rage Form among them) is
//! put on the command menu's title plate (style in, the logo's Loader, 2 s of its loop) and drawn with the software
//! renderer; the plate's area is measured for the logo's ink (it must be there) and for a dark box behind it (it must not:
//! the in-game round saw one behind Agile Claws / Twin Yo-Yos). Crops: build/sora_tmp/weapons/F-HUD/shots/logos/.

use std::path::Path;

use kh_ui::command::{CommandDisplay, CommandModel, CommandStrings, MenuRow, MenuState};
use kh_ui::def::write_png;
use kh_ui::forms_hud_style::FormStyles;
use kh_ui::hud_command::HudCommand;
use kh_ui::{Canvas, Fonts, Matrix, Pack, Renderer, UiSound};

const DT: f32 = 1.0 / 60.0;
/// The title plate's area on the 1920 x 1080 stage, where the logo stands.
const PLATE: (usize, usize, usize, usize) = (0, 740, 480, 960);

fn background(y: usize) -> [f32; 3] {
    [0.30 + 0.1 * (y as f32 / 1080.0), 0.38, 0.46]
}

/// (ink, dark): pixels of the plate area that differ from the background, and the longest run of near-black pixels on
/// one row (a box behind the logo spans the plate; the logos' own dark art (Highwind's flag, the launchers' banners) does
/// not).
fn measure(rgba: &[u8]) -> (usize, usize) {
    let (mut ink, mut dark) = (0, 0);
    for y in PLATE.1..PLATE.3 {
        let mut run = 0;
        for x in PLATE.0..PLATE.2 {
            let i = (y * 1920 + x) * 4;
            let bg = background(y).map(|c| (c * 255.0) as i32);
            let px = [rgba[i] as i32, rgba[i + 1] as i32, rgba[i + 2] as i32];
            if (0..3).map(|c| (px[c] - bg[c]).abs()).max().unwrap_or(0) > 40 {
                ink += 1;
            }
            run = if px.iter().all(|c| *c < 24) { run + 1 } else { 0 };
            dark = dark.max(run);
        }
    }
    (ink, dark)
}

/// The logo of `kind` after 2 s on the plate: the whole screen's RGBA.
fn render(pack: &mut Pack, fonts: &Fonts, styles: &FormStyles, kind: &str) -> Option<Vec<u8>> {
    let mut hud = HudCommand::new(pack.movie("HudCommand").ok()?, fonts.clone());
    let mut display = CommandDisplay::new();
    let strings = CommandStrings::load(pack);
    let mut sounds: Vec<UiSound> = Vec::new();
    let mut model = CommandModel { state: MenuState::MainSel, battle_mode: 2, link_none: true, ..CommandModel::default() };
    for (slot, text) in ["Attack", "Magic", "Items", ""].into_iter().enumerate() {
        model.main[slot] = MenuRow { text: text.to_string(), enabled: !text.is_empty(), ..MenuRow::default() };
    }
    for frame in 0..160 {
        if frame == 40 {
            model.form_style = styles.style_of(kind, 30.0);
            model.form_gauge = Some((30.0, 30.0));
        }
        display.update(&mut hud, &model, &strings, DT, &mut sounds);
        hud.tick();
    }
    let mut canvas = Canvas::new(1920, 1080);
    Renderer::new(fonts.clone()).draw(&mut canvas, &hud.movie, &Matrix::IDENTITY);
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|_, y| background(y), &mut rgba);
    Some(rgba)
}

fn crop(rgba: &[u8]) -> Vec<u8> {
    let (w, h) = (PLATE.2 - PLATE.0, PLATE.3 - PLATE.1);
    let mut out = vec![0u8; w * h * 4];
    for y in 0..h {
        let s = ((PLATE.1 + y) * 1920 + PLATE.0) * 4;
        out[y * w * 4..(y + 1) * w * 4].copy_from_slice(&rgba[s..s + w * 4]);
    }
    out
}

#[test]
fn every_form_logo_draws_without_a_dark_box() {
    let Ok(mut pack) = Pack::open_default() else {
        eprintln!("SKIPPED: no build/sora_pack/ui");
        return;
    };
    let fonts = Fonts::load(&mut pack).unwrap();
    let styles = FormStyles::load(&mut pack);
    assert!(styles.missing.is_empty(), "logo movies missing: {:?}", styles.missing);
    let data = pack.data("main_command_styles").unwrap();
    let kinds: Vec<(String, String)> = data["rows"].as_array().unwrap().iter().filter_map(|r| Some((r["id"].as_str()?.to_string(), r["command_kind"].as_str()?.to_string()))).filter(|(_, k)| k.starts_with("TRES_CMD_KIND_CHANGE_")).collect();
    assert_eq!(kinds.len(), 30);
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/weapons/F-HUD/shots/logos");
    std::fs::create_dir_all(&dir).ok();
    let plain = render(&mut pack, &fonts, &styles, "TRES_CMD_KIND_NONE").unwrap();
    let (plain_ink, plain_dark) = measure(&plain);
    let mut faults = Vec::new();
    for (row, kind) in &kinds {
        let rgba = render(&mut pack, &fonts, &styles, kind).unwrap();
        let (ink, dark) = measure(&rgba);
        let logo = styles.style_of(kind, 30.0).and_then(|s| s.logo).map(|l| l.name).unwrap_or_default();
        eprintln!("{row:12} {logo:12} ink {ink:6} (plain {plain_ink}) dark {dark:6} (plain {plain_dark})");
        let (w, h) = (PLATE.2 - PLATE.0, PLATE.3 - PLATE.1);
        write_png(&dir.join(format!("{row}.png")), w as u32, h as u32, &crop(&rgba)).unwrap();
        if ink < plain_ink + 4000 {
            faults.push(format!("{row} ({logo}): no logo (ink {ink})"));
        }
        if dark > plain_dark + 300 {
            faults.push(format!("{row} ({logo}): a dark box (a near-black run of {dark} px)"));
        }
    }
    assert!(faults.is_empty(), "{faults:#?}");
}

