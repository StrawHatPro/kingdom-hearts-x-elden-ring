//! (F-HUD) THE COMMAND MENU'S FORM STYLE: KH3's style in (`setStyle(styleIn, styleOut, 1, iconType, hasTimer, logoPath)`,
//! 0x14162EF30) puts the form's logo movie (`act_logo/ActLogoKNN`, loaded by the movie's own Loader) on the title plate
//! with the logo gauge; a used finish plays the logo's `finishIn`; the style out (`setStyle(0, styleOut, mode)`,
//! 0x14161C3F0) plays the logo out, after which the Loader is unloaded. Pictures go to
//! build/sora_tmp/weapons/F-HUD/shots/ (to be looked at).

use std::path::Path;

use kh_ui::command::{CommandDisplay, CommandModel, CommandStrings, EventButton, MenuEvent, MenuRow, MenuState};
use kh_ui::def::write_png;
use kh_ui::forms_hud_style::{self, FormStyles};
use kh_ui::hud_command::{HudCommand, ACT_LOGO_HOST};
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
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/weapons/F-HUD/shots");
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
fn the_table_gives_every_formchange_its_row_style_id_and_logo() {
    let Some(mut pack) = pack() else { return };
    let styles = FormStyles::load(&mut pack);
    assert!(styles.missing.is_empty(), "logo movies missing from the pack: {:?}", styles.missing);
    assert_eq!(styles.len(), 30, "the CHANGE_* rows of MainCommandData");
    // the row index is the style id (0x14155F700 writes it into row +0x50): NORMAL 0 ... CHANGE_DW 12
    let dw = styles.style_of("TRES_CMD_KIND_CHANGE_DW", 30.0).unwrap();
    assert_eq!((dw.row.as_str(), dw.style, dw.icon_type), ("CHANGE_DW", 12, 1));
    assert_eq!(dw.logo.as_ref().map(|l| l.name.as_str()), Some("ActLogoK01"));
    let l1 = styles.style_of("TRES_CMD_KIND_CHANGE_L1", 30.0).unwrap();
    assert_eq!(l1.logo_path.as_deref(), Some("/Game/UI/Scaleform/01_hud/act_logo/ActLogoK19.ActLogoK19"));
    assert_eq!(l1.logo.as_ref().map(|l| l.name.as_str()), Some("ActLogoK19"));
    // the four Second Form variants share K22; Spellweaver (CHANGE_A1) is S01
    assert_eq!(styles.style_of("TRES_CMD_KIND_CHANGE_L3", 30.0).and_then(|s| s.logo).map(|l| l.name), Some("ActLogoK22".to_string()));
    assert_eq!(styles.style_of("TRES_CMD_KIND_CHANGE_A1", 20.0).and_then(|s| s.logo).map(|l| l.name), Some("ActLogoS01".to_string()));
    // attractions and other styles are not served (the user excluded attractions)
    assert!(styles.style_of("TRES_CMD_KIND_AF_TEACUP", 30.0).is_none());
    assert!(styles.style_of("TRES_CMD_KIND_NONE", 0.0).is_none());
    // the one-off reader agrees
    assert_eq!(forms_hud_style::style_of(&mut pack, "TRES_CMD_KIND_CHANGE_L1", 30.0).map(|s| (s.style, s.logo.map(|l| l.name))), Some((l1.style, Some("ActLogoK19".to_string()))));
}

#[test]
fn a_form_style_goes_in_with_its_logo_finishes_and_goes_out_unloading_the_logo() {
    let Some(mut pack) = pack() else { return };
    let fonts = Fonts::load(&mut pack).unwrap();
    let styles = FormStyles::load(&mut pack);
    let mut rig = Rig::new(&mut pack, &fonts);
    let mut model = menu();
    rig.run(&mut model, 40);
    let normal = shot("style_normal", &rig.hud.movie, &fonts);
    rig.hud.calls.clear();

    // STYLE IN: Second Form, 30 s
    let style = styles.style_of("TRES_CMD_KIND_CHANGE_L1", 30.0).unwrap();
    model.form_style = Some(style.clone());
    model.form_gauge = Some((30.0, 30.0));
    rig.display.update(&mut rig.hud, &model, &rig.strings, DT, &mut rig.sounds);
    let calls = rig.hud.calls.clone();
    let set_style = format!("setStyle({}, 0, 1, 1, true, Some({:?}), 0)", style.style, style.logo_path.as_deref().unwrap());
    let at = |c: &str| calls.iter().position(|x| x.starts_with(c)).unwrap_or_else(|| panic!("no {c} in {calls:#?}"));
    assert!(calls.contains(&set_style), "{calls:#?}");
    // KH3's order (0x14162EF30): setStyle (whose Loader load follows its title off), then setActionSelect(0), (10), (2)
    assert!(at("setStyle(") < at("actLogoLoader.load(") && at("actLogoLoader.load(") < at("setActionSelect(0") && at("setActionSelect(0") < at("setActionSelect(10") && at("setActionSelect(10") < calls.iter().rposition(|c| c.starts_with("setActionSelect(2")).unwrap(), "{calls:#?}");
    assert!(rig.hud.act_logo_clip().is_none(), "the Loader completes on the next frame");
    rig.hud.tick();
    assert_eq!(rig.hud.act_logo_clip(), Some(format!("{ACT_LOGO_HOST}.actLogoK19").as_str()));
    assert!(rig.hud.calls.iter().any(|c| c == "onActLogoLoadComplete(actLogoK19)"));
    let clip = rig.hud.act_logo_clip().unwrap().to_string();
    assert_eq!(rig.label(&clip).as_deref(), Some("in"));
    assert_eq!(rig.label("cmdMenu.cmdTitleTop.cmdTitle").as_deref(), Some("off"), "the logo replaces COMMANDS");
    assert_eq!(rig.label("cmdMenu.cmdActLogo.cmdActLogoShort").as_deref(), Some("in"));
    assert_eq!(rig.label("cmdMenu.cmdActLogoGg.cmdActLogoGgShort").as_deref(), Some("in"));

    // THE GAUGE: the form's time drains, the logo loops
    rig.run(&mut model, 150);
    assert_eq!(rig.label(&clip).as_deref(), Some("loop"));
    assert_eq!(rig.label("cmdMenu.cmdActLogoGg.cmdActLogoGgShort").as_deref(), Some("loop"));
    let gauge = rig.hud.movie.current_frame("cmdMenu.cmdActLogoGg.cmdActLogoGgShort.cmdActLogoGgAnm").unwrap();
    assert!(gauge > 1 && gauge < 30, "about 2.5 s of 30 gone: frame {gauge}");
    let in_style = shot("style_l1", &rig.hud.movie, &fonts);
    assert!(plate_ink(&in_style) > plate_ink(&normal) + 2000, "the logo draws on the title plate: ink {} vs {}", plate_ink(&in_style), plate_ink(&normal));

    // FINISH: the finish command (the event button, slot B) is used: deleteActionSelectEventButton(true, true, ..) -> finishIn
    model.event_button = Some(EventButton { name: "Finish".into(), has_gauge: false, slot_b: true });
    rig.run(&mut model, 30);
    model.event_button = None;
    model.events.push(MenuEvent::EventButtonUsed { form_change: true });
    rig.run(&mut model, 1);
    assert_eq!(rig.label(&clip).as_deref(), Some("finishIn"));
    rig.run(&mut model, 14);
    shot("style_l1_finish", &rig.hud.movie, &fonts);

    // STYLE OUT
    rig.hud.calls.clear();
    model.form_style = None;
    model.form_gauge = None;
    rig.run(&mut model, 1);
    assert!(rig.hud.calls.contains(&format!("setStyle(0, {}, 1, 0, false, None, 0)", style.style)), "{:#?}", rig.hud.calls);
    assert!(!rig.hud.calls.iter().any(|c| c.starts_with("hideLogoGauge")), "the style's end took the gauge out: {:#?}", rig.hud.calls);
    assert_eq!(rig.label("cmdMenu.cmdActLogo.cmdActLogoShort").as_deref(), Some("out"));
    assert_eq!(rig.label("cmdMenu.cmdTitleTop.cmdTitle").as_deref(), Some("logoIn"), "COMMANDS comes back (mode 1 = MainSel)");
    assert!(rig.hud.act_logo_clip().is_some(), "the logo plays out with its plate");
    rig.run(&mut model, 7);
    shot("style_out_mid", &rig.hud.movie, &fonts);
    rig.run(&mut model, 30);
    // the plate's out animation reported ANIM_MCACTLOGOANM_OUT: logo "off", Loader unloaded
    assert!(rig.hud.calls.iter().any(|c| c == "actLogoLoader.unloadAndStop()"), "{:#?}", rig.hud.calls);
    assert!(rig.hud.act_logo_clip().is_none());
    assert!(rig.hud.movie.sprite(ACT_LOGO_HOST).is_some_and(|s| s.child("actLogoK19").is_none()));
    let out = shot("style_out", &rig.hud.movie, &fonts);
    assert!(plate_ink(&out) < plate_ink(&in_style), "the logo is gone");
    assert!(rig.hud.movie.misses.is_empty(), "paths not found: {:?}", rig.hud.movie.misses);
}

#[test]
fn form_to_form_and_other_logos_draw() {
    let Some(mut pack) = pack() else { return };
    let fonts = Fonts::load(&mut pack).unwrap();
    let styles = FormStyles::load(&mut pack);
    for (kind, name) in [("TRES_CMD_KIND_CHANGE_DW", "style_dw"), ("TRES_CMD_KIND_CHANGE_A1", "style_a1"), ("TRES_CMD_KIND_CHANGE_SH", "style_sh")] {
        let mut rig = Rig::new(&mut pack, &fonts);
        let mut model = menu();
        rig.run(&mut model, 40);
        let style = styles.style_of(kind, 30.0).unwrap();
        model.form_style = Some(style.clone());
        model.form_gauge = Some((30.0, 30.0));
        rig.run(&mut model, 160);
        let clip = rig.hud.act_logo_clip().unwrap_or_else(|| panic!("{kind}: no logo")).to_string();
        assert_eq!(rig.label(&clip).as_deref(), Some("loop"), "{kind}");
        shot(name, &rig.hud.movie, &fonts);
        assert!(rig.hud.movie.misses.is_empty(), "{kind}: paths not found: {:?}", rig.hud.movie.misses);
    }
    // a form replaced by another: setStyle(new, old, ...), the old logo unloaded, the new one loaded
    let mut rig = Rig::new(&mut pack, &fonts);
    let mut model = menu();
    rig.run(&mut model, 40);
    let (a, b) = (styles.style_of("TRES_CMD_KIND_CHANGE_SH", 30.0).unwrap(), styles.style_of("TRES_CMD_KIND_CHANGE_SH2", 30.0).unwrap());
    model.form_style = Some(a.clone());
    model.form_gauge = Some((30.0, 30.0));
    rig.run(&mut model, 60);
    rig.hud.calls.clear();
    model.form_style = Some(b.clone());
    rig.run(&mut model, 2);
    assert!(rig.hud.calls.iter().any(|c| c.starts_with(&format!("setStyle({}, {}, 1, 1, true", b.style, a.style))), "{:#?}", rig.hud.calls);
    // setStyle's own push down / push up (3, 2) only: the native's extra push up (2) is a first style's
    let tail = rig.hud.calls.iter().skip_while(|c| !c.starts_with("setActionSelect(10")).collect::<Vec<_>>();
    assert!(!tail.iter().any(|c| c.starts_with("setActionSelect(2")), "only a first style pushes the situation commands up again: {:#?}", rig.hud.calls);
    assert_eq!(rig.hud.calls.iter().filter(|c| c.starts_with("setActionSelect(2")).count(), 1, "{:#?}", rig.hud.calls);
    let want = format!("{ACT_LOGO_HOST}.act{}", &b.logo.as_ref().unwrap().name[3..]);
    assert_eq!(rig.hud.act_logo_clip(), Some(want.as_str()));
}
