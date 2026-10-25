//! (CV3) THE FREE CONVERSATION on the real UI pack (the user, 2026-10-17: "remove the cinematic camera from talking and
//! allow the player to move the camera and their character, though A is reserved for moving dialogue along while in
//! conversation unless in combat"): the bubble out of a fight (A advances: the Keyblade cue), the same line hands-free in
//! a fight (passive: no cue, A is not taken), a choice on the D-pad, and the tail following the speaker while the player
//! walks and turns his camera (the faces move on the screen every frame). Pictures: build/sora_tmp/party/cv3/shots/.

use kh_ui::bubble::{BubbleEvent, Face, SpeechBubble};
use kh_ui::camp::CampKey;
use kh_ui::{Fonts, Pack};

fn pack() -> Option<(Pack, Fonts)> {
    let mut pack = Pack::open_default().map_err(|_| eprintln!("SKIPPED: no build/sora_pack/ui")).ok()?;
    let fonts = Fonts::load(&mut pack).ok()?;
    Some((pack, fonts))
}

fn shots_dir() -> std::path::PathBuf {
    let d = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/party/cv3/shots");
    std::fs::create_dir_all(&d).ok();
    d
}

/// The bubble over a backdrop with the faces as discs (the speaker's orange, the others' blue), 1920 x 1080.
fn picture(fonts: &Fonts, name: &str, b: &SpeechBubble, faces: &[Face]) {
    let (w, h) = (1920usize, 1080usize);
    let mut canvas = kh_ui::Canvas::new(w, h);
    let mut renderer = kh_ui::Renderer::new(fonts.clone());
    renderer.draw(&mut canvas, &b.movie, &kh_ui::Matrix::IDENTITY);
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(
        |x, y| {
            let (fx, fy) = (x as f32, y as f32);
            for (k, f) in faces.iter().enumerate() {
                if (fx - f.x).hypot(fy - f.y) < f.r {
                    return if k == 0 { [0.85, 0.62, 0.45] } else { [0.45, 0.6, 0.85] };
                }
            }
            [0.30 + 0.25 * (fx / w as f32), 0.38, 0.33 + 0.2 * (fy / h as f32)]
        },
        &mut rgba,
    );
    kh_ui::def::write_png(&shots_dir().join(format!("{name}.png")), w as u32, h as u32, &rgba).unwrap();
}

fn settle(b: &mut SpeechBubble, frames: u32) {
    for _ in 0..frames {
        b.tick();
    }
}

#[test]
fn cv3_out_of_a_fight_a_advances_in_a_fight_the_bubble_is_passive() {
    let Some((mut pack, fonts)) = pack() else { return };
    let mut b = SpeechBubble::load(&mut pack, fonts.clone()).expect("bubble");
    let mut sounds = Vec::new();
    let donald = Face { x: 1040.0, y: 640.0, r: 30.0 };
    let goofy = Face { x: 760.0, y: 600.0, r: 44.0 };
    b.set_speaker(Some(donald));
    b.set_faces(&[goofy]);
    b.say(Some("Donald"), "Aw, phooey! That old crone's a fake!", None, &mut sounds);
    settle(&mut b, 400);
    assert!(b.cue_shown(), "out of a fight: the Keyblade waits for A");
    picture(&fonts, "cv3_line_a_advances", &b, &[donald, goofy]);
    // a fight: the runner makes it passive (hands-free): no cue, no key taken (A attacks)
    b.set_passive(true);
    b.set_cue(Some(false));
    settle(&mut b, 2);
    assert!(!b.cue_shown(), "in a fight: no Keyblade cue");
    b.key(CampKey::Decide, false, &mut sounds);
    assert!(!b.take_events().contains(&BubbleEvent::Advanced), "A is the attack in a fight, not the bubble's");
    picture(&fonts, "cv3_line_hands_free_in_a_fight", &b, &[donald, goofy]);
    // the fight is over: A again
    b.set_passive(false);
    b.set_cue(None);
    settle(&mut b, 2);
    assert!(b.cue_shown());
    b.key(CampKey::Decide, false, &mut sounds);
    assert!(b.take_events().contains(&BubbleEvent::Advanced));
}

#[test]
fn cv3_a_choice_on_the_dpad_and_the_tail_follows_the_walking_player() {
    let Some((mut pack, fonts)) = pack() else { return };
    let mut b = SpeechBubble::load(&mut pack, fonts.clone()).expect("bubble");
    let mut sounds = Vec::new();
    let mut crone = Face { x: 1180.0, y: 560.0, r: 34.0 };
    b.set_speaker(Some(crone));
    b.choose(Some("Finger Reader Crone"), "", &["Show hands".to_string(), "Leave".to_string()], 0, Some(1), &mut sounds);
    settle(&mut b, 60);
    b.key(CampKey::Down, false, &mut sounds); // (the D-pad: the stick walks the player)
    settle(&mut b, 120);
    assert_eq!(b.cursor(), 1);
    picture(&fonts, "cv3_choice_dpad", &b, &[crone]);
    b.key(CampKey::Cancel, false, &mut sounds);
    assert!(b.take_events().contains(&BubbleEvent::Chosen(1)), "B = Leave");
    // a line while the player walks past her and swings his camera: her face slides across the screen; the tail follows
    b.say(Some("Finger Reader Crone"), "Within the fog, a gaping hole. Beyond the fog, one cursed by blood.", None, &mut sounds);
    settle(&mut b, 300);
    let mut tips = Vec::new();
    for k in 0..=60 {
        crone = Face { x: 1180.0 - k as f32 * 9.0, y: 560.0 + (k as f32 * 0.1).sin() * 20.0, r: 34.0 - k as f32 * 0.1 };
        b.set_speaker(Some(crone));
        b.tick();
        tips.push(b.drawn.expect("drawn").1.tip.0);
        if k % 30 == 0 {
            picture(&fonts, &format!("cv3_walking_tail_{k:02}"), &b, &[crone]);
        }
    }
    assert!(tips.windows(2).all(|w| w[1] <= w[0] + 0.01), "the tail follows her across: {tips:?}");
    assert!(tips[0] - tips[60] > 300.0, "{tips:?}");
}
