//! (CV1) the speech bubble (bubble.rs) on the real UI pack: the colour kept from the S8 talk window, sizes, the tail,
//! the Keyblade icon, choices, pictures at several resolutions.

use kh_ui::talk::TalkWindow;
use kh_ui::{Fonts, Pack};

fn pack() -> Option<(Pack, Fonts)> {
    let mut pack = Pack::open_default().map_err(|_| eprintln!("SKIPPED: no build/sora_pack/ui")).ok()?;
    let fonts = Fonts::load(&mut pack).ok()?;
    Some((pack, fonts))
}

/// THE COLOUR OF TODAY'S WINDOW, measured: the S8 talk window drawn over black and over white; per row of its middle
/// column the straight colour and the alpha (a = 1 - (white - black) / 255, c = black / a).
#[test]
fn measure_the_old_window() {
    let Some((mut pack, fonts)) = pack() else { return };
    let mut w = TalkWindow::load(&mut pack, fonts.clone()).expect("talk window");
    let mut sounds = Vec::new();
    w.say(Some("Donald"), "I let her read my fingers.", (960.0, 600.0), &mut sounds);
    for _ in 0..60 {
        w.tick();
    }
    let draw = |bg: f32| {
        let mut canvas = kh_ui::Canvas::new(1920, 1080);
        let mut renderer = kh_ui::Renderer::new(fonts.clone());
        renderer.draw(&mut canvas, &w.dialog, &kh_ui::Matrix::IDENTITY);
        let mut rgba = Vec::new();
        canvas.to_rgba8_over(|_, _| [bg, bg, bg], &mut rgba);
        rgba
    };
    let (black, white) = (draw(0.0), draw(1.0));
    let x = 960usize;
    for y in (380..620).step_by(3) {
        let k = (y * 1920 + x) * 4;
        let a = 1.0 - (white[k] as f32 - black[k] as f32) / 255.0;
        if a < 0.02 {
            continue;
        }
        println!("y {y}: a {a:.3} rgb ({:.1}, {:.1}, {:.1})", black[k] as f32 / a, black[k + 1] as f32 / a, black[k + 2] as f32 / a);
    }
    // the edge column (bevel)
    for x in (780..880).step_by(1) {
        let k = (500 * 1920 + x) * 4;
        let a = 1.0 - (white[k] as f32 - black[k] as f32) / 255.0;
        if a < 0.02 {
            continue;
        }
        println!("x {x}: a {a:.3} rgb ({:.1}, {:.1}, {:.1})", black[k] as f32 / a, black[k + 1] as f32 / a, black[k + 2] as f32 / a);
    }
}

use kh_ui::bubble::{BubbleEvent, Face, SpeechBubble, TailSide};
use kh_ui::camp::CampKey;

fn shots_dir() -> std::path::PathBuf {
    let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/er/conv/shots");
    std::fs::create_dir_all(&d).ok();
    d
}

/// A backdrop (the S8 pictures' gradient) with the faces drawn as discs (the speaker's orange, the others' blue),
/// the movies through KH3's ExactFit 16:9 viewport (kh_host::hud_layers::stage_root's rule) at `size`.
fn picture(fonts: &Fonts, name: &str, movies: &[&kh_ui::Movie], faces: &[Face], size: (u32, u32)) {
    let (w, h) = size;
    let (l, t, vw, vh) = kh_ui::frames::stage_viewport(w as f32, h as f32, (1.0, 1.0));
    let root = kh_ui::Matrix::new(vw / 1920.0, 0.0, 0.0, vh / 1080.0, l, t);
    let mut canvas = kh_ui::Canvas::new(w as usize, h as usize);
    let mut renderer = kh_ui::Renderer::new(fonts.clone());
    for m in movies {
        renderer.draw(&mut canvas, m, &root);
    }
    let disc: Vec<(f32, f32, f32)> = faces.iter().map(|f| (l + f.x * vw / 1920.0, t + f.y * vh / 1080.0, f.r * vh / 1080.0)).collect();
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(
        |x, y| {
            let (fx, fy) = (x as f32, y as f32);
            for (k, (cx, cy, r)) in disc.iter().enumerate() {
                if (fx - cx).hypot(fy - cy) < *r {
                    return if k == 0 { [0.85, 0.62, 0.45] } else { [0.45, 0.6, 0.85] };
                }
            }
            if fx < l || fx >= l + vw {
                return [0.12, 0.12, 0.14];
            }
            [0.30 + 0.25 * (fx / w as f32), 0.38, 0.33 + 0.2 * (fy / h as f32)]
        },
        &mut rgba,
    );
    kh_ui::def::write_png(&shots_dir().join(format!("{name}.png")), w, h, &rgba).unwrap();
}

fn settle(b: &mut SpeechBubble, frames: u32) {
    for _ in 0..frames {
        b.tick();
    }
}

/// THE SIZES (the user: shrink or expand to the text): a 1-word line, a medium line, a long line, a choice; widths
/// between MIN_W and MAX_W, a long line grows taller at MAX_W; the Keyblade at the bottom middle at every size.
#[test]
fn sizes_follow_the_content() {
    let Some((mut pack, fonts)) = pack() else { return };
    let mut b = SpeechBubble::load(&mut pack, fonts.clone()).expect("bubble");
    let mut sounds = Vec::new();
    let speaker = Face { x: 980.0, y: 700.0, r: 40.0 };
    let listener = Face { x: 640.0, y: 690.0, r: 40.0 };
    b.set_speaker(Some(speaker));
    b.set_faces(&[listener]);
    let lines: [(&str, Option<&str>, &str); 3] = [
        ("size_1_word", Some("Donald"), "Phooey!"),
        ("size_2_medium", Some("Donald"), "Aw, phooey! That old crone's a fake!"),
        ("size_3_long", Some("White Mask Varre"), "Ahh, you're maidenless, aren't you? Then you're in luck, for I know of a maiden who could use a Tarnished such as yourself. But she'll not show herself to just anyone. You must prove yourself worthy of her grace first."),
    ];
    let mut sizes = Vec::new();
    for (name, who, text) in lines {
        b.say(who, text, None, &mut sounds);
        settle(&mut b, 600);
        let (rect, tail) = b.drawn.expect("drawn");
        sizes.push(b.size());
        assert!(b.cue_shown(), "{name}: the Keyblade waits for A");
        let key = b.cue_rect().unwrap();
        assert!(((key.x0 + key.x1) * 0.5 - (rect.x0 + rect.x1) * 0.5).abs() <= 1.0, "{name}: the Keyblade is in the middle");
        assert!(key.y1 <= rect.y1 && key.y0 > rect.y1 - kh_ui::bubble::PAD_ICON - 6.0, "{name}: at the bottom, inside {key:?} {rect:?}");
        assert_eq!(tail.side, TailSide::Bottom, "{name}");
        picture(&fonts, name, &[&b.movie], &[speaker, listener], (1920, 1080));
    }
    assert!(sizes[0].0 < sizes[1].0 && sizes[1].0 < sizes[2].0, "{sizes:?}");
    assert!(sizes[0].0 >= kh_ui::bubble::MIN_W && sizes[2].0 <= kh_ui::bubble::MAX_W, "{sizes:?}");
    assert!(sizes[2].1 > sizes[1].1, "a long line grows taller: {sizes:?}");
    // a choice counts toward the size
    b.choose(Some("White Mask Varre"), "Will you take this?", &["Accept".to_string(), "Refuse".to_string(), "Talk about the maiden".to_string()], 0, Some(1), &mut sounds);
    settle(&mut b, 30);
    b.key(CampKey::Down, false, &mut sounds);
    settle(&mut b, 600);
    assert_eq!(b.cursor(), 1);
    assert!(!b.cue_shown(), "a choice shows the glove, not the Keyblade");
    picture(&fonts, "size_4_choice", &[&b.movie], &[speaker, listener], (1920, 1080));
    b.key(CampKey::Cancel, false, &mut sounds);
    assert!(b.take_events().contains(&BubbleEvent::Chosen(1)));
}

/// THE SIZE IS EASED between lines (no jump): the frames after a short line follows a long one shrink step by step.
#[test]
fn the_size_eases_between_lines() {
    let Some((mut pack, fonts)) = pack() else { return };
    let mut b = SpeechBubble::load(&mut pack, fonts.clone()).expect("bubble");
    let mut sounds = Vec::new();
    b.set_speaker(Some(Face { x: 960.0, y: 700.0, r: 40.0 }));
    b.say(Some("Donald"), "Then she tells me I'll face \"wrath, then madness, and finally, chaos\"!", None, &mut sounds);
    settle(&mut b, 300);
    let big = b.size();
    b.say(Some("Donald"), "Bogus!", None, &mut sounds);
    let mut widths = vec![b.size().0];
    for _ in 0..30 {
        b.tick();
        widths.push(b.size().0);
    }
    assert!(widths.windows(2).all(|w| w[1] <= w[0] + 0.01), "{widths:?}");
    assert!(widths[1] < big.0 && widths[1] > b.target_size().0, "eased, not jumped: {widths:?}");
    assert_eq!(*widths.last().unwrap(), b.target_size().0, "settled within half a second");
}

/// THE TAIL TRACKS THE SPEAKER every frame: he walks across, the tail's tip follows; he goes above the bubble, the
/// tail flips to the top; frames at 1080p, 1440p and 21:9.
#[test]
fn the_tail_tracks_the_speaker() {
    let Some((mut pack, fonts)) = pack() else { return };
    let mut b = SpeechBubble::load(&mut pack, fonts.clone()).expect("bubble");
    let mut sounds = Vec::new();
    let mut s = Face { x: 760.0, y: 640.0, r: 36.0 };
    b.set_speaker(Some(s));
    b.say(Some("Donald"), "I let her read my fingers, and she says, \"A dreadful sign! Truly dreadful!\"", None, &mut sounds);
    settle(&mut b, 400);
    picture(&fonts, "tail_a_1080p", &[&b.movie], &[s], (1920, 1080));
    picture(&fonts, "tail_a_1440p", &[&b.movie], &[s], (2560, 1440));
    picture(&fonts, "tail_a_ultrawide", &[&b.movie], &[s], (3440, 1440));
    let mut tips = Vec::new();
    for k in 0..40 {
        s.x = 760.0 + k as f32 * 10.0;
        b.set_speaker(Some(s));
        b.tick();
        let (_, tail) = b.drawn.unwrap();
        tips.push(tail.tip.0);
    }
    assert!(tips.windows(2).all(|w| w[1] >= w[0] - 0.01), "{tips:?}");
    picture(&fonts, "tail_b_moved", &[&b.movie], &[s], (1920, 1080));
    // the speaker climbs above the bubble: the tail flips to the top (the bubble moves under him)
    s = Face { x: 1200.0, y: 120.0, r: 36.0 };
    b.set_speaker(Some(s));
    settle(&mut b, 120);
    let (rect, tail) = b.drawn.unwrap();
    assert_eq!(tail.side, TailSide::Top, "{rect:?} {tail:?}");
    picture(&fonts, "tail_c_flipped", &[&b.movie], &[s], (1920, 1080));
    // off screen: no tail, the lower middle
    b.set_speaker(None);
    settle(&mut b, 120);
    assert_eq!(b.drawn.unwrap().1.side, TailSide::None);
    picture(&fonts, "tail_d_offscreen", &[&b.movie], &[], (1920, 1080));
}

/// The pop-in and the typewriter, frame by frame (a strip of frames for the state note).
#[test]
fn the_pop_and_the_typewriter() {
    let Some((mut pack, fonts)) = pack() else { return };
    let mut b = SpeechBubble::load(&mut pack, fonts.clone()).expect("bubble");
    let mut sounds = Vec::new();
    let s = Face { x: 960.0, y: 640.0, r: 36.0 };
    b.set_speaker(Some(s));
    b.say(Some("Donald"), "Hmph! I'm not stayin' here another minute. Sora, I'm comin' with ya!", None, &mut sounds);
    assert_eq!(sounds, vec![kh_ui::UiSound::Play(kh_ui::menu::SE_MENU_OPEN)]);
    let mut frame = 0u32;
    for f in [2u32, 6, 12, 40, 90] {
        while frame < f {
            b.tick();
            frame += 1;
        }
        picture(&fonts, &format!("pop_{f:03}"), &[&b.movie], &[s], (1920, 1080));
    }
    // A shows the rest; A again goes on
    b.key(CampKey::Decide, false, &mut sounds);
    assert!(b.finished());
    b.key(CampKey::Decide, false, &mut sounds);
    assert!(b.take_events().contains(&BubbleEvent::Advanced));
    b.close(&mut sounds);
    settle(&mut b, 20);
    assert!(!b.is_open());
}

/// A close look at the bubble's bottom (the Keyblade) and its tail: 4x crops.
#[test]
fn zoom_on_the_keyblade_and_the_tail() {
    let Some((mut pack, fonts)) = pack() else { return };
    let mut b = SpeechBubble::load(&mut pack, fonts.clone()).expect("bubble");
    let mut sounds = Vec::new();
    let s = Face { x: 1100.0, y: 700.0, r: 36.0 };
    b.set_speaker(Some(s));
    b.say(Some("Donald"), "Aw, phooey! That old crone is a fake!", None, &mut sounds);
    settle(&mut b, 300);
    let (rect, tail) = b.drawn.unwrap();
    let mut canvas = kh_ui::Canvas::new(1920, 1080);
    let mut renderer = kh_ui::Renderer::new(fonts.clone());
    renderer.draw(&mut canvas, &b.movie, &kh_ui::Matrix::IDENTITY);
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|x, y| [0.30 + 0.25 * (x as f32 / 1920.0), 0.38, 0.33 + 0.2 * (y as f32 / 1080.0)], &mut rgba);
    let crop = |name: &str, x0: i32, y0: i32, w: i32, h: i32, k: i32| {
        let mut out = vec![0u8; (w * k * h * k * 4) as usize];
        for y in 0..h * k {
            for x in 0..w * k {
                let (sx, sy) = ((x0 + x / k).clamp(0, 1919), (y0 + y / k).clamp(0, 1079));
                let si = ((sy * 1920 + sx) * 4) as usize;
                let di = ((y * w * k + x) * 4) as usize;
                out[di..di + 4].copy_from_slice(&rgba[si..si + 4]);
            }
        }
        kh_ui::def::write_png(&shots_dir().join(format!("{name}.png")), (w * k) as u32, (h * k) as u32, &out).unwrap();
    };
    let mid = ((rect.x0 + rect.x1) * 0.5) as i32;
    crop("zoom_keyblade", mid - 60, rect.y1 as i32 - 70, 120, 90, 5);
    crop("zoom_tail", tail.base_x as i32 - 70, rect.y1 as i32 - 40, 140, 110, 4);
}
