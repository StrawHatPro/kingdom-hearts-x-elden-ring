//! (F-HUD) The operation guide `HudOperation` (kh_ui::forms_hud_operation): ShooterMode's "<RT hold> Auto-Shot" row, the
//! open -> rows -> close state machine and its calls, pictures in build/sora_tmp/weapons/F-HUD/shots/.

use std::path::Path;

use kh_ui::def::write_png;
use kh_ui::forms_hud_operation::{guide_path, OperationGuide, OperationKind, TUPBT_R2HOLD};
use kh_ui::{Canvas, Fonts, Matrix, Movie, Pack, Renderer};

fn pack() -> Option<Pack> {
    Pack::open_default().map_err(|_| eprintln!("SKIPPED: no build/sora_pack/ui")).ok()
}

fn shot(name: &str, movie: &Movie, fonts: &Fonts) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/weapons/F-HUD/shots");
    std::fs::create_dir_all(&dir).ok();
    let mut canvas = Canvas::new(1920, 1080);
    let mut renderer = Renderer::new(fonts.clone());
    renderer.draw(&mut canvas, movie, &Matrix::IDENTITY);
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|_, y| [0.30 + 0.1 * (y as f32 / 1080.0), 0.38, 0.46], &mut rgba);
    write_png(&dir.join(format!("{name}.png")), 1920, 1080, &rgba).unwrap();
}

fn ticks(guide: &mut OperationGuide, n: usize) {
    for _ in 0..n {
        guide.tick();
    }
}

#[test]
fn shootermode_guide_opens_sets_its_row_and_closes() {
    let Some(mut pack) = pack() else { return };
    let fonts = Fonts::load(&mut pack).unwrap();
    let mut guide = OperationGuide::load(&mut pack, &fonts).expect("HudOperation in the pack");
    assert!(guide.hud.movie.def.unknown_statements.is_empty(), "{:?}", guide.hud.movie.def.unknown_statements);
    let row = guide.table.row("SHOOTERMODE").unwrap().clone();
    assert_eq!((row.texts.as_slice(), row.buttons.as_slice()), (&["Auto-Shot".to_string()][..], &[TUPBT_R2HOLD][..]));
    assert!(!guide.is_open() && guide.kind() == 0);
    // Nothing asked: nothing happens.
    guide.update(None, true);
    assert!(guide.hud.calls.is_empty());

    // ShooterMode's camera on: Open, SetOperationGuide(4); one row = the bottom one (operationGuide1).
    guide.update(Some(OperationKind::ShooterMode), true);
    assert!(guide.is_open());
    assert_eq!(guide.kind(), 4);
    let rows = guide.rows();
    assert_eq!(rows[5], (0, TUPBT_R2HOLD, 0));
    assert!(rows[..5].iter().all(|r| *r == (-1, 0, 0)), "{rows:?}");
    let g1 = guide_path(5);
    let calls = guide.hud.calls.join(" | ");
    for want in [
        "Open".to_string(),
        "operation.gotoAndPlay(\"in\")".to_string(),
        ".gotoAndPlay(\"loop\")".to_string(),
        "SetOperationGuide(4)".to_string(),
        format!("{}.gotoAndPlay(\"off\")", guide_path(0)),
        format!("{g1}.txOperation.coOperationTxt.textField.text = \"Auto-Shot\""),
        format!("{g1}.gotoAndPlay(\"loop\")"),
        format!("{g1}.padBtn.txPadBtn.text = \"<icon(R2Hold)>\""),
        format!("{g1}.padBtn2.txPadBtn.text = \"\""),
        format!("{g1}.padBtn.pushMark.gotoAndPlay(\"loop\")"),
        format!("{g1}.padBtn2.pushMark.gotoAndPlay(\"off\")"),
    ] {
        assert!(calls.contains(&want), "missing {want} in {calls}");
    }
    assert_eq!(guide.hud.movie.text(&format!("{g1}.txOperation.coOperationTxt.textField")).as_deref(), Some("Auto-Shot"));
    assert_eq!(guide.hud.movie.text(&format!("{g1}.padBtn.txPadBtn")).as_deref(), Some("<icon(R2Hold)>"));
    // Asking again changes nothing.
    let n = guide.hud.calls.len();
    guide.update(Some(OperationKind::ShooterMode), true);
    assert_eq!(guide.hud.calls.len(), n);

    // The `in` (frames 9 - 19 of McOperation_1) ends on its stop at `loop`.
    ticks(&mut guide, 24);
    assert_eq!(guide.hud.movie.current_label("operation"), Some("loop"));
    assert_eq!(guide.hud.movie.current_label(&g1), Some("loop"));
    assert_eq!(guide.hud.movie.current_label(&guide_path(1)), Some("off"));
    assert_eq!(guide.hud.movie.text(&format!("{g1}.txOperation.coOperationTxt.textField")).as_deref(), Some("Auto-Shot"), "the timeline kept the text");
    shot("operation_shootermode", guide.movie(), &fonts);

    // The pad stops being the input device: the rows again, the push mark off.
    guide.update(Some(OperationKind::ShooterMode), false);
    assert!(guide.hud.calls.last().unwrap().ends_with("padBtn2.pushMark.gotoAndPlay(\"off\")"));
    assert!(guide.hud.calls.iter().rev().take(8).any(|c| *c == format!("{g1}.padBtn.pushMark.gotoAndPlay(\"off\")")));
    guide.update(Some(OperationKind::ShooterMode), true);

    // ShooterMode over: Close (`out`, frames 27 - 37), then `off`.
    guide.update(None, true);
    assert!(!guide.is_open() && guide.kind() == 0);
    assert!(guide.hud.calls.iter().rev().take(3).any(|c| c == "operation.gotoAndPlay(\"out\")"));
    ticks(&mut guide, 4);
    assert_eq!(guide.hud.movie.current_label("operation"), Some("out"));
    shot("operation_shootermode_out", guide.movie(), &fonts);
    ticks(&mut guide, 20);
    assert_eq!(guide.hud.movie.current_label("operation"), Some("off"));
    // Closing a closed guide does nothing.
    let n = guide.hud.calls.len();
    guide.close();
    assert_eq!(guide.hud.calls.len(), n);
    assert!(guide.hud.movie.misses.is_empty(), "paths not found: {:?}", guide.hud.movie.misses);
}

#[test]
fn three_row_guide_fills_from_the_bottom_up() {
    let Some(mut pack) = pack() else { return };
    let fonts = Fonts::load(&mut pack).unwrap();
    let mut guide = OperationGuide::load(&mut pack, &fonts).unwrap();
    // A guide is only set while open (vt+0x260).
    guide.set_operation_guide(OperationKind::Underwater);
    assert!(guide.hud.calls.is_empty());
    guide.update(Some(OperationKind::Underwater), true);
    let rows = guide.rows();
    assert_eq!(&rows[3..], &[(0, 7, 0), (0, 8, 0), (0, 12, 0)]);
    let text = |guide: &OperationGuide, i: usize, field: &str| guide.hud.movie.text(&format!("{}.{field}", guide_path(i))).unwrap_or_default();
    assert_eq!(["Ascend", "Descend", "Swim Faster"].map(String::from), [3, 4, 5].map(|i| text(&guide, i, "txOperation.coOperationTxt.textField")));
    assert_eq!(["<icon(R2)>", "<icon(L2)>", "<icon(Cancel)>"].map(String::from), [3, 4, 5].map(|i| text(&guide, i, "padBtn.txPadBtn")));
    ticks(&mut guide, 24);
    for i in 3..6 {
        assert_eq!(guide.hud.movie.current_label(&guide_path(i)), Some("loop"));
    }
    shot("operation_underwater", guide.movie(), &fonts);
    // Another kind while open: the rows are set again (Open is a no-op).
    guide.update(Some(OperationKind::ShooterMode), true);
    assert_eq!(guide.kind(), 4);
    assert_eq!(guide.rows()[3].0, -1);
    assert_eq!(guide.hud.calls.iter().filter(|c| *c == "Open").count(), 1);
    assert!(guide.hud.movie.misses.is_empty(), "paths not found: {:?}", guide.hud.movie.misses);
}
