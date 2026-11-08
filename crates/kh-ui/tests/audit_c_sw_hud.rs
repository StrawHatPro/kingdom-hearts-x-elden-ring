//! (AUDIT-C SW) Ultimate Form's HUD: the command menu style of CHANGE_SW with its act logo ActLogoK20 (UIDataAssetWeaponSW),
//! the logo's `finishIn` when FINISH_SW is used from slot B, the style out, and the situation labels. Pictures go to
//! build/sora_tmp/weapons/AUDIT-C/SW/shots/hud/.

use std::path::Path;

use kh_ui::command::{CommandDisplay, CommandModel, CommandStrings, EventButton, MenuEvent, MenuRow, MenuState};
use kh_ui::def::write_png;
use kh_ui::forms_hud_style::FormStyles;
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

/// The whole screen and the menu's corner twice as large.
fn shot(name: &str, movie: &Movie, fonts: &Fonts) -> Vec<u8> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/weapons/AUDIT-C/SW/shots/hud");
    std::fs::create_dir_all(&dir).ok();
    let mut canvas = Canvas::new(1920, 1080);
    Renderer::new(fonts.clone()).draw(&mut canvas, movie, &Matrix::IDENTITY);
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|_, y| [0.30 + 0.1 * (y as f32 / 1080.0), 0.38, 0.46], &mut rgba);
    write_png(&dir.join(format!("{name}.png")), 1920, 1080, &rgba).unwrap();
    let (x0, y0, w, h, k) = (0usize, 700usize, 560usize, 380usize, 2usize);
    let mut zoom = vec![0u8; w * k * h * k * 4];
    for y in 0..h * k {
        for x in 0..w * k {
            let s = ((y0 + y / k) * 1920 + x0 + x / k) * 4;
            let d = (y * w * k + x) * 4;
            zoom[d..d + 4].copy_from_slice(&rgba[s..s + 4]);
        }
    }
    write_png(&dir.join(format!("{name}_zoom.png")), (w * k) as u32, (h * k) as u32, &zoom).unwrap();
    rgba
}

fn menu() -> CommandModel {
    let mut model = CommandModel { state: MenuState::MainSel, battle_mode: 2, link_none: true, ..CommandModel::default() };
    for (slot, text) in ["Attack", "Magic", "Items", ""].into_iter().enumerate() {
        model.main[slot] = MenuRow { text: text.to_string(), enabled: !text.is_empty(), ..MenuRow::default() };
    }
    model
}

struct Rig {
    hud: HudCommand,
    display: CommandDisplay,
    strings: CommandStrings,
    sounds: Vec<UiSound>,
}

impl Rig {
    fn new(pack: &mut Pack, fonts: &Fonts) -> Rig {
        Rig { hud: HudCommand::new(pack.movie("HudCommand").unwrap(), fonts.clone()), display: CommandDisplay::new(), strings: CommandStrings::load(pack), sounds: Vec::new() }
    }

    fn run(&mut self, model: &mut CommandModel, frames: usize) {
        for _ in 0..frames {
            self.display.update(&mut self.hud, model, &self.strings, DT, &mut self.sounds);
            self.hud.tick();
            model.events.clear();
            if let Some((left, total)) = model.form_gauge.as_mut() {
                *left = (*left - DT).max(0.0);
                let _ = total;
            }
        }
    }

    fn label(&self, path: &str) -> Option<String> {
        self.hud.movie.current_label(path).map(str::to_string)
    }
}

/// The non-background pixels of the title plate's area (stage 100 .. 470 x 820 .. 960), where KH3 draws the form logo.
fn plate_ink(rgba: &[u8]) -> usize {
    let mut n = 0;
    for y in 820..960 {
        for x in 100..470 {
            let i = (y * 1920 + x) * 4;
            let bg = [((0.30 + 0.1 * (y as f32 / 1080.0)) * 255.0) as i32, (0.38 * 255.0) as i32, (0.46 * 255.0) as i32];
            let d = (0..3).map(|c| (rgba[i + c] as i32 - bg[c]).abs()).max().unwrap_or(0);
            if d > 40 {
                n += 1;
            }
        }
    }
    n
}

#[test]
fn ultimate_form_style_logo_k20_finish_in_and_labels() {
    let Some(mut pack) = pack() else { return };
    // the situation labels (TresCmdData via the UI pack's commands table): CHANGE_SW 135 "Ultimate Form", FINISH_SW 161 "Finish"
    let commands = pack.data("commands").unwrap();
    let name = |id: i64| commands["rows"].as_array().unwrap().iter().find(|r| r["id"].as_i64() == Some(id)).and_then(|r| r["name_en"].as_str()).map(str::to_string);
    assert_eq!(name(135).as_deref(), Some("Ultimate Form"));
    assert_eq!(name(161).as_deref(), Some("Finish"));
    let fonts = Fonts::load(&mut pack).unwrap();
    let styles = FormStyles::load(&mut pack);
    let style = styles.style_of("TRES_CMD_KIND_CHANGE_SW", 30.0).expect("CHANGE_SW's style row");
    assert_eq!(style.logo.as_ref().map(|l| l.name.as_str()), Some("ActLogoK20"), "UIDataAssetWeaponSW's act logo");
    let mut rig = Rig::new(&mut pack, &fonts);
    let mut model = menu();
    rig.run(&mut model, 40);
    let normal = shot("sw_normal", &rig.hud.movie, &fonts);
    model.form_style = Some(style.clone());
    model.form_gauge = Some((30.0, 30.0));
    rig.run(&mut model, 2);
    let clip = rig.hud.act_logo_clip().expect("the logo loaded").to_string();
    assert!(clip.ends_with("actLogoK20"), "{clip}");
    rig.run(&mut model, 120);
    assert_eq!(rig.label(&clip).as_deref(), Some("loop"));
    let in_style = shot("sw_style", &rig.hud.movie, &fonts);
    assert!(plate_ink(&in_style) > plate_ink(&normal) + 2000, "the K20 logo draws on the title plate");
    // FINISH_SW used from slot B: the logo's finishIn
    model.event_button = Some(EventButton { name: "Finish".into(), has_gauge: false, slot_b: true });
    rig.run(&mut model, 30);
    shot("sw_finish_offer", &rig.hud.movie, &fonts);
    model.event_button = None;
    model.events.push(MenuEvent::EventButtonUsed { form_change: true });
    rig.run(&mut model, 1);
    assert_eq!(rig.label(&clip).as_deref(), Some("finishIn"));
    rig.run(&mut model, 14);
    shot("sw_finish_in", &rig.hud.movie, &fonts);
    model.form_style = None;
    model.form_gauge = None;
    rig.run(&mut model, 40);
    assert!(rig.hud.act_logo_clip().is_none(), "the logo unloads after the style out");
    assert!(rig.hud.movie.misses.is_empty(), "paths not found: {:?}", rig.hud.movie.misses);
}
