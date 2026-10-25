//! (CV2) THE MOCK: real Varre lines and his real choice, as the adapter makes them (steps.rs, rules.rs), run through
//! CV1's own runner (kh_host::conv) onto CV1's own bubble (kh_ui::bubble) on the real UI pack, pictures in
//! build/sora_tmp/er/conv/shots/cv2_*.png. The texts are the game's (TalkMsg / EventTextForTalk of the user's copy,
//! printed by conv/tools/talkwalk.js for t301006000: block 30101100 rows 0..1, the choice of group x50). Skipped
//! without build/sora_pack/ui.

use kh_host::conv::{ConvEvent, ConvInput, ConvView, Conversation};
use kh_ui::bubble::{BubbleEvent, Face, SpeechBubble};
use kh_ui::camp::CampKey;
use kh_ui::{Fonts, Pack, UiSound};

use super::rules::{cmd, env, AskKind, Decision, Talks, Texts, Val};
use super::steps;

struct View {
    b: SpeechBubble,
    sounds: Vec<UiSound>,
}

impl ConvView for View {
    fn conv_line(&mut self, name: Option<&str>, text: &str, reveal_cps: Option<f32>) {
        self.b.say(name, text, reveal_cps, &mut self.sounds);
    }
    fn conv_choice(&mut self, name: Option<&str>, prompt: &str, options: &[String], start: usize, cancel: Option<usize>) {
        self.b.choose(name, prompt, options, start, cancel, &mut self.sounds);
    }
    fn conv_cue(&mut self, cue: Option<bool>) {
        self.b.set_cue(cue);
    }
    fn conv_close(&mut self) {
        self.b.close(&mut self.sounds);
    }
    fn conv_events(&mut self) -> Vec<BubbleEvent> {
        self.b.take_events()
    }
    fn conv_line_shown(&self) -> bool {
        self.b.finished()
    }
    fn conv_passive(&mut self, passive: bool) {
        self.b.set_passive(passive);
    }
}

struct VarreTexts;
impl Texts for VarreTexts {
    fn flag(&self, _: u32) -> Option<bool> {
        None
    }
    fn option_text(&self, msg: i32) -> Option<String> {
        match msg {
            23_011_000 => Some("I'll fly straight and true!".into()),
            23_011_001 => Some("I don't need a seat".into()),
            _ => None,
        }
    }
    fn dialog_text(&self, _: i32) -> Option<String> {
        None
    }
}

fn shot(fonts: &Fonts, name: &str, b: &SpeechBubble, faces: &[Face]) {
    let (w, h) = (1920u32, 1080u32);
    let mut canvas = kh_ui::Canvas::new(w as usize, h as usize);
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
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/er/conv/shots");
    std::fs::create_dir_all(&dir).ok();
    kh_ui::def::write_png(&dir.join(format!("{name}.png")), w, h, &rgba).unwrap();
}

#[test]
fn varres_lines_and_choice_in_cv1s_bubble() {
    let Ok(mut pack) = Pack::open_default() else {
        eprintln!("SKIPPED: no build/sora_pack/ui");
        return;
    };
    let Ok(fonts) = Fonts::load(&mut pack) else { return };
    let Ok(bubble) = SpeechBubble::load(&mut pack, fonts.clone()) else { return };
    let mut view = View { b: bubble, sounds: Vec::new() };
    let varre = Face { x: 1180.0, y: 560.0, r: 46.0 };
    let sora = Face { x: 760.0, y: 600.0, r: 42.0 };
    let mut conv = Conversation::new(steps::actors("", "White Mask Varré").to_vec(), Vec::new());
    conv.open_ended = true;
    let dt = 1.0 / 60.0;
    let mut events = Vec::new();
    let run = |conv: &mut Conversation, view: &mut View, frames: u32, voice: Option<bool>, speaker: Face, other: Face, events: &mut Vec<ConvEvent>| {
        for _ in 0..frames {
            view.b.set_speaker(Some(speaker));
            view.b.set_faces(&[other]);
            conv.step(view, &ConvInput { dt, combat: false, voice_playing: voice, apart: None });
            view.b.step(dt);
            events.extend(conv.take_events());
        }
    };

    // ONE SUBTITLE LINE = ONE BUBBLE: Varre's first two lines of block 30101100, each its own Line step
    conv.push(steps::line("Oh, yes... Tarnished, are we?", 30_101_100, 301_011_000, 2.6));
    run(&mut conv, &mut view, 90, Some(true), varre, sora, &mut events);
    assert_eq!(view.b.mode(), kh_ui::bubble::BubbleMode::Line);
    shot(&fonts, "cv2_varre_line_1", &view.b, &[varre, sora]);
    // A while the game's voice plays (the text is all shown): the runner's StopVoice = the adapter's game skip
    view.b.key(CampKey::Decide, false, &mut view.sounds);
    run(&mut conv, &mut view, 2, Some(true), varre, sora, &mut events);
    assert!(events.iter().any(|e| matches!(e, ConvEvent::StopVoice { .. })), "A mid-voice: StopVoice ({events:?})");
    assert!(events.iter().any(|e| matches!(e, ConvEvent::LineDone { index: 0 })));
    // the game went on to its next row: the next Line, its own bubble; it goes on by itself when the voice ends
    events.clear();
    conv.push(steps::line("Come to the Lands Between for the Elden Ring, hmm?", 30_101_101, 301_011_010, 3.0));
    run(&mut conv, &mut view, 90, Some(true), varre, sora, &mut events);
    shot(&fonts, "cv2_varre_line_2", &view.b, &[varre, sora]);
    assert!(!events.iter().any(|e| matches!(e, ConvEvent::LineDone { .. })), "waits while the voice plays");
    run(&mut conv, &mut view, 3, Some(false), varre, sora, &mut events);
    assert!(events.iter().any(|e| matches!(e, ConvEvent::LineDone { index: 1 })), "the voice is over: the line goes on (auto_after 0)");

    // HIS CHOICE (t301006000 x50): the script's own commands through the rules, the external Choice, B none, Down + A
    let mut talks = Talks::new();
    talks.ready = true;
    let i = Val::Int;
    talks.on_event(301_006_000, 7, 8, cmd::CLEAR_LIST, &[], &VarreTexts);
    talks.on_event(301_006_000, 7, 8, cmd::ADD_LIST, &[i(1), i(23_011_000), i(-1)], &VarreTexts);
    talks.on_event(301_006_000, 7, 8, cmd::ADD_LIST, &[i(2), i(23_011_001), i(-1)], &VarreTexts);
    assert_eq!(talks.on_event(301_006_000, 7, 8, cmd::CHOICES, &[i(0)], &VarreTexts), Decision::Swallow);
    let ask = talks.ask.clone().unwrap();
    assert_eq!(ask.kind, AskKind::Choices);
    events.clear();
    conv.push(steps::choice(&ask));
    run(&mut conv, &mut view, 40, None, sora, varre, &mut events);
    assert_eq!(view.b.mode(), kh_ui::bubble::BubbleMode::Choice);
    view.b.key(CampKey::Down, false, &mut view.sounds);
    run(&mut conv, &mut view, 30, None, sora, varre, &mut events);
    shot(&fonts, "cv2_varre_choice", &view.b, &[sora, varre]);
    view.b.key(CampKey::Decide, false, &mut view.sounds);
    run(&mut conv, &mut view, 2, None, sora, varre, &mut events);
    let option = events.iter().find_map(|e| if let ConvEvent::Chosen { option, .. } = e { Some(*option) } else { None }).expect("chosen");
    assert_eq!(option, 1);
    // back to the script exactly as the game's menu: 59 reads closed, 23 reads the option's own index 2
    assert_eq!(talks.answer(ask.serial, option), Some(2));
    assert_eq!(talks.on_env(301_006_000, env::MENU_OPEN, &[i(12), i(0)]), None);
    assert_eq!(talks.on_env(301_006_000, env::LIST_RESULT, &[]), Some(2), "x50 state 4: reg0 == 2 -> state 6 -> block 30102300");
}

/// (CV2-b) A HOSTILE VARRE'S TAUNT MID-FIGHT as an AMBIENT bubble over him (t301006000 x39 block 30180400, TalkToPlayer
/// flag 1): the ambient runner (nobody held, no A), the passive bubble; it goes on by itself when the game's voice ends,
/// a fight does not end it, A does nothing. Picture conv/shots/cv2_ambient_taunt.png.
#[test]
fn a_hostile_taunt_mid_fight_is_an_ambient_bubble_over_him() {
    let Ok(mut pack) = Pack::open_default() else {
        eprintln!("SKIPPED: no build/sora_pack/ui");
        return;
    };
    let Ok(fonts) = Fonts::load(&mut pack) else { return };
    let Ok(bubble) = SpeechBubble::load(&mut pack, fonts.clone()) else { return };
    let mut view = View { b: bubble, sounds: Vec::new() };
    let varre = Face { x: 1240.0, y: 470.0, r: 46.0 };
    let sora = Face { x: 820.0, y: 640.0, r: 42.0 };
    let mut conv = Conversation::new(vec![steps::actors("", "White Mask Varré")[1].clone()], Vec::new());
    conv.open_ended = true;
    conv.ambient = true;
    conv.push(steps::ambient_line("You will die nameless, without ceremony.", 30_180_400, 301_804_010, 2.4));
    let dt = 1.0 / 60.0;
    let mut events = Vec::new();
    for f in 0..120 {
        view.b.set_speaker(Some(varre));
        view.b.set_faces(&[sora]);
        if f == 100 {
            view.b.key(CampKey::Decide, false, &mut view.sounds); // A: nothing (the pad is the player's)
        }
        conv.step(&mut view, &ConvInput { dt, combat: false, voice_playing: Some(true), apart: None });
        view.b.step(dt);
        events.extend(conv.take_events());
    }
    shot(&fonts, "cv2_ambient_taunt", &view.b, &[varre, sora]);
    assert!(!conv.holds(steps::NPC), "nobody is held");
    assert!(!events.iter().any(|e| matches!(e, ConvEvent::StopVoice { .. } | ConvEvent::LineDone { .. })), "A does nothing; the line waits for the voice: {events:?}");
    for _ in 0..5 {
        conv.step(&mut view, &ConvInput { dt, combat: false, voice_playing: Some(false), apart: None });
        view.b.step(dt);
        events.extend(conv.take_events());
    }
    assert!(events.iter().any(|e| matches!(e, ConvEvent::LineDone { index: 0 })), "the voice is over: the line goes on by itself");
}
