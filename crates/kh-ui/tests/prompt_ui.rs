//! (PROMPT-UI) KH3's button notice ("ATTACK!" over the command menu's centre line, the clip `buttonNoticeSet.buttonNotice`
//! of HudTarget) through the HUD function 0x1416793E0's port (`TargetDriver::button_notice`): labels per mode, the clip's
//! frames, the drawn notice. Pictures: build/sora_tmp/weapons/PROMPT-UI/shots/ui_*.png.

use std::path::Path;

use kh_ui::def::write_png;
use kh_ui::hud_target::{notice_icon, notice_label_key, HudTarget, TargetDriver, NOTICE_HIDE, NOTICE_KEEP, NOTICE_KEY_CANCEL, NOTICE_KEY_DECIDE, NOTICE_LABEL_ATTACK, NOTICE_SHOW, NOTICE_SHOW2, NOTICE_USED, NOTICE_USED3};
use kh_ui::{Canvas, Fonts, Matrix, Movie, Pack, Renderer};

fn pack() -> Option<Pack> {
    Pack::open_default().map_err(|_| eprintln!("SKIPPED: no build/sora_pack/ui")).ok()
}

fn shot(name: &str, movie: &Movie, fonts: &Fonts) -> Vec<u8> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/weapons/PROMPT-UI/shots");
    std::fs::create_dir_all(&dir).ok();
    let mut canvas = Canvas::new(1920, 1080);
    let mut renderer = Renderer::new(fonts.clone());
    renderer.draw(&mut canvas, movie, &Matrix::IDENTITY);
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|_, y| [0.30 + 0.1 * (y as f32 / 1080.0), 0.38, 0.46], &mut rgba);
    write_png(&dir.join(format!("ui_{name}.png")), 1920, 1080, &rgba).unwrap();
    rgba
}

/// Pixels in the band of the notice (y 740 .. 830) that differ from the plain background.
fn ink(rgba: &[u8]) -> usize {
    let mut n = 0;
    for y in 740..830 {
        let bg = [((0.30 + 0.1 * (y as f32 / 1080.0)) * 255.0) as i32, (0.38 * 255.0) as i32, (0.46 * 255.0) as i32];
        for x in 500..1420 {
            let p = (y * 1920 + x) * 4;
            let d = (0..3).map(|c| (rgba[p + c] as i32 - bg[c]).abs()).max().unwrap_or(0);
            if d > 24 {
                n += 1;
            }
        }
    }
    n
}

#[test]
fn prompt_ui_notice_labels_follow_kh3s_modes() {
    let Some(mut pack) = pack() else { return };
    let fonts = Fonts::load(&mut pack).unwrap();
    let mut hud = HudTarget::new(pack.movie("HudTarget").unwrap(), fonts.clone());
    hud.native_root("loop");
    let mut d = TargetDriver::default();
    let text = pack.string(notice_label_key(NOTICE_LABEL_ATTACK).unwrap()).unwrap();
    assert_eq!(text, "ATTACK!");
    assert_eq!((notice_icon(NOTICE_KEY_DECIDE), notice_icon(NOTICE_KEY_CANCEL)), ("<icon(Cross2)>", "<icon(Circle2)>"));
    // Nothing up: hide / used / keep do nothing.
    assert!(!d.button_notice(&mut hud, NOTICE_HIDE, NOTICE_KEY_DECIDE, "", false));
    assert!(!d.button_notice(&mut hud, NOTICE_KEEP, NOTICE_KEY_DECIDE, "", false));
    // Show: in, then the loop (frame 19 on); a second show while it is up is ignored (+0x2E0).
    assert!(d.button_notice(&mut hud, NOTICE_SHOW, NOTICE_KEY_DECIDE, &text, false));
    assert_eq!(hud.button_notice_label().as_deref(), Some("in"));
    assert!(!d.button_notice(&mut hud, NOTICE_SHOW, NOTICE_KEY_DECIDE, &text, false));
    assert_eq!(hud.movie.text("buttonNoticeSet.buttonNotice.attack.txButtonNotice.coButtonNoticeTx").as_deref(), Some("ATTACK!"));
    let mut frames = Vec::new();
    for _ in 0..40 {
        hud.tick();
        frames.push(hud.movie.current_frame("buttonNoticeSet.buttonNotice").unwrap());
    }
    assert_eq!((frames[0], frames[9], frames[10]), (9, 18, 19), "in = frames 9 .. 18, 10 stage frames: {frames:?}");
    assert!(frames.iter().skip(10).all(|f| (19..=37).contains(f)), "then the loop 19 .. 37: {frames:?}");
    let up = shot("notice_attack_loop", &hud.movie, &fonts);
    assert!(ink(&up) > 3000, "the notice is drawn: {}", ink(&up));
    // Used: select (the button flash), then off.
    assert!(d.button_notice(&mut hud, NOTICE_USED, NOTICE_KEY_DECIDE, "", false));
    assert_eq!(hud.button_notice_label().as_deref(), Some("select"));
    for _ in 0..6 {
        hud.tick();
    }
    shot("notice_attack_select", &hud.movie, &fonts);
    for _ in 0..40 {
        hud.tick();
    }
    assert_eq!(hud.movie.current_frame("buttonNoticeSet.buttonNotice"), Some(3), "back to off (stopped on frame 3)");
    let gone = shot("notice_attack_off", &hud.movie, &fonts);
    assert!(ink(&gone) < 200, "nothing left: {}", ink(&gone));
    // Hide: out; show2: in2; keep: select2; used3: select3.
    assert!(d.button_notice(&mut hud, NOTICE_SHOW, NOTICE_KEY_DECIDE, &text, false));
    assert!(d.button_notice(&mut hud, NOTICE_HIDE, NOTICE_KEY_DECIDE, "", false));
    assert_eq!(hud.button_notice_label().as_deref(), Some("out"));
    assert!(d.button_notice(&mut hud, NOTICE_SHOW2, NOTICE_KEY_DECIDE, &text, false));
    assert_eq!(hud.button_notice_label().as_deref(), Some("in2"));
    assert!(d.button_notice(&mut hud, NOTICE_KEEP, NOTICE_KEY_DECIDE, "", false));
    assert_eq!((hud.button_notice_label().as_deref(), d.notice_shown()), (Some("select2"), Some(NOTICE_KEY_DECIDE)));
    assert!(d.button_notice(&mut hud, NOTICE_USED3, NOTICE_KEY_DECIDE, "", false));
    assert_eq!((hud.button_notice_label().as_deref(), d.notice_shown()), (Some("select3"), None));
    // King: inKing / outKing.
    assert!(d.button_notice(&mut hud, NOTICE_SHOW, NOTICE_KEY_DECIDE, &text, true));
    assert_eq!(hud.button_notice_label().as_deref(), Some("inKing"));
    assert!(d.button_notice(&mut hud, NOTICE_HIDE, NOTICE_KEY_DECIDE, "", false));
    assert_eq!(hud.button_notice_label().as_deref(), Some("outKing"));
}
