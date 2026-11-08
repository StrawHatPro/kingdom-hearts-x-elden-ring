//! (PROMPT-UI, the user 2026-11-05: "make sure this attack prompt appears whenever it should in KH3 as well as those
//! arrows above the command menu (for all characters now and in the future that use the Y commands")
//! THE REAL HUD (`Hud`, the UI pack; skipped without it) driven by snapshots:
//! - the button notice ("ATTACK!") comes in with KH3's `in` (frames 9 .. 18), loops, and leaves with `select` when its
//!   button took it, `out` when it ran out; every KH3 case (airstep, reaction prompt, counter window, finish extra) and
//!   "DODGE!"; the notice's command names the Attack slot;
//! - the style-point gauge row (">>"): none below a quarter, 1 / 2 / 3 chevrons at a quarter / half / three quarters, gone
//!   at full; Sora's only while he leads: a friend's snapshot through the gate carries his own (Donald / Goofy: none).
//!
//! Pictures: build/sora_tmp/weapons/PROMPT-UI/shots/hud_*.png (+ the KH3 reference beside ours).

use kh_core::command::ShootFlowPanel;
use kh_core::ui_model::{ButtonPrompt, PromptKind, YPrompts};
use kh_core::{PadButton, PadButtons, Sora, StateId, TargetId, Vec3};
use kh_host::hud::{Frame, Hud, HudTargets, TargetView};
use kh_host::hud_layers::{draw_movie_in, stage_root, Draw, DrawArea, Drawn, Perf};

struct NoTargets;
impl HudTargets for NoTargets {
    fn target(&self, _id: TargetId) -> Option<TargetView> {
        None
    }
}

fn same(p: Vec3) -> Vec3 {
    p
}

fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn out_dir() -> std::path::PathBuf {
    let d = root().join("build/sora_tmp/weapons/PROMPT-UI/shots");
    std::fs::create_dir_all(&d).ok();
    d
}

/// The HUD's movies over a dusky floor (1920 x 1080 RGBA).
fn picture(fonts: &kh_ui::Fonts, movies: &[&kh_ui::Movie]) -> Vec<u8> {
    let mut out = vec![0u8; 1920 * 1080 * 4];
    for (i, o) in out.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let y = (i / 1920) as f32 / 1080.0;
        o.copy_from_slice(&[(70.0 + 50.0 * y) as u8, (48.0 + 20.0 * y) as u8, 40, 255]);
    }
    for movie in movies {
        let (stage, view) = stage_root(1920.0, 1080.0);
        let mut draw = Draw::default();
        let mut renderer = kh_ui::render::Renderer::new(fonts.clone());
        let Drawn::Picture(rect, _) = draw_movie_in(&mut draw, &mut renderer, movie, &stage, DrawArea { size: (1920.0, 1080.0), clip: Some(view) }, true, Perf::default()) else { continue };
        for y in 0..rect.3 {
            for x in 0..rect.2 {
                let (fx, fy) = (rect.0 + x as i32, rect.1 + y as i32);
                if (0..1920).contains(&fx) && (0..1080).contains(&fy) {
                    let d = (fy as usize * 1920 + fx as usize) * 4;
                    let s = (y * rect.2 + x) * 4;
                    let a = draw.rgba[s + 3] as f32 / 255.0;
                    for c in 0..3 {
                        out[d + c] = (draw.rgba[s + c] as f32 * a + out[d + c] as f32 * (1.0 - a)).round() as u8;
                    }
                }
            }
        }
    }
    out
}

fn save(name: &str, w: usize, h: usize, rgba: &[u8]) {
    kh_ui::def::write_png(&out_dir().join(name), w as u32, h as u32, rgba).unwrap();
}

/// A crop (x, y, w, h) of a picture `sw` wide with `ch` channels, scaled to `ow` x `oh` (nearest).
fn crop(src: &[u8], sw: usize, ch: usize, (x0, y0, w, h): (usize, usize, usize, usize), (ow, oh): (usize, usize)) -> Vec<u8> {
    let mut out = vec![0u8; ow * oh * 4];
    for y in 0..oh {
        for x in 0..ow {
            let s = ((y0 + y * h / oh) * sw + x0 + x * w / ow) * ch;
            let d = (y * ow + x) * 4;
            out[d..d + 3].copy_from_slice(&src[s..s + 3]);
            out[d + 3] = 255;
        }
    }
    out
}

fn above_below(a: &[u8], b: &[u8], w: usize, h: usize) -> Vec<u8> {
    let mut out = vec![255u8; w * (h * 2 + 8) * 4];
    out[..w * h * 4].copy_from_slice(a);
    out[w * (h + 8) * 4..].copy_from_slice(b);
    out
}

/// Pixels of the gauge's yellow chevrons in the box.
fn chevron_px(p: &[u8], (x0, y0, x1, y1): (usize, usize, usize, usize)) -> usize {
    let mut n = 0;
    for y in y0..y1 {
        for x in x0..x1 {
            let s = (y * 1920 + x) * 4;
            let (r, g, b) = (p[s] as i32, p[s + 1] as i32, p[s + 2] as i32);
            if r > 170 && g > 160 && b < 120 && r - b > 80 {
                n += 1;
            }
        }
    }
    n
}

struct Rig {
    hud: Hud,
    fonts: kh_ui::Fonts,
    sora: Sora,
    snap: kh_core::UiSnapshot,
}

impl Rig {
    fn new() -> Option<Rig> {
        let Ok((hud, fonts)) = Hud::load(&root().join("build/sora_pack/ui")) else {
            eprintln!("no UI pack: skipped");
            return None;
        };
        let sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None)));
        let mut snap = sora.ui_snapshot();
        for (row, (kind, name)) in snap.command.main.iter_mut().zip([(1u8, "Attack"), (28, "Magic"), (57, "Items"), (0, "Link")]) {
            (row.kind, row.name, row.enabled) = (kind, name, kind != 0);
        }
        let mut r = Rig { hud, fonts, sora, snap };
        r.frames(90, PadButtons::NONE, StateId::Idle);
        Some(r)
    }

    fn frames(&mut self, n: usize, pressed: PadButtons, state: StateId) {
        for k in 0..n {
            let p = if k == 0 { pressed } else { PadButtons::NONE };
            self.snap.step += 1;
            self.hud.frame(&mut Frame { snapshot: &self.snap, sora: &self.sora, events: &[], state, step_dt: 1.0 / 60.0, play_seconds: 0, targets: &NoTargets, to_view: same, view: None, words: &[], dt: 1.0 / 60.0, ticks: 1, pressed: p, nav: Vec::new() });
        }
    }

    fn notice(&self) -> (Option<i32>, Option<String>) {
        self.hud.notice_state()
    }

    fn shot(&self) -> Vec<u8> {
        let movies: Vec<&kh_ui::Movie> = ["HudCommand", "HudTarget"].iter().filter_map(|m| self.hud.movie(m)).collect();
        picture(&self.fonts, &movies)
    }

    fn attack_text(&self) -> Option<String> {
        self.hud.movie("HudCommand").and_then(|m| m.text("cmdMenu.cmdMainTop.cmdMain.cmdMain1.cmdText1.txCmdName.txCmdNameCol.coCmdName"))
    }
}

fn attack(kind: PromptKind) -> Option<ButtonPrompt> {
    Some(ButtonPrompt { kind, target: TargetId::NONE })
}

#[test]
fn prompt_ui_the_attack_notice_for_every_kh3_case() {
    let Some(mut r) = Rig::new() else { return };
    for (case, kind) in [("airstep", PromptKind::AirstepAttack), ("reaction", PromptKind::Reaction(2)), ("counter", PromptKind::Counter(3)), ("extra", PromptKind::FinishExtra(0xAF))] {
        // shown in the frame its source stands, with KH3's `in` (10 stage frames), then the loop
        r.snap.prompt = attack(kind);
        r.frames(1, PadButtons::NONE, StateId::Idle);
        assert_eq!(r.notice(), (Some(0xB), Some("in".into())), "{case}: in");
        r.frames(12, PadButtons::NONE, StateId::Idle);
        assert_eq!(r.notice(), (Some(0xB), Some("loop".into())), "{case}: loop");
        if case == "reaction" {
            let shown = r.shot();
            save("hud_notice_reaction.png", 1920, 1080, &shown);
        }
        // taken by its button: `select`; else `out`
        r.snap.prompt = None;
        r.frames(1, PadButtons::NONE.with(PadButton::ATTACK), if case == "airstep" { StateId::Attack } else { StateId::Idle });
        assert_eq!(r.notice(), (None, Some("select".into())), "{case}: used");
        r.frames(40, PadButtons::NONE, StateId::Idle);
        r.snap.prompt = attack(kind);
        r.frames(20, PadButtons::NONE, StateId::Idle);
        r.snap.prompt = None;
        r.frames(1, PadButtons::NONE, StateId::Idle);
        let left = r.notice();
        if case == "extra" {
            assert_eq!(left, (None, Some("select".into())), "{case}: slot 163 always takes it as used (mode 3)");
        } else {
            assert_eq!(left, (None, Some("out".into())), "{case}: ran out");
        }
        r.frames(40, PadButtons::NONE, StateId::Idle);
    }
    // the notice's command on the Attack slot (0x14162A440), Attack back with it gone (0x141627150)
    r.snap.prompt = attack(PromptKind::Reaction(2));
    r.frames(20, PadButtons::NONE, StateId::Idle);
    let named = r.attack_text();
    r.snap.prompt = None;
    r.frames(20, PadButtons::NONE, StateId::Idle);
    eprintln!("Attack slot: {named:?} -> {:?}", r.attack_text());
    assert!(named.is_some_and(|t| t != "Attack"), "the Blow Counter's name on Attack");
    // "DODGE!" (key 0xC, Cancel = B)
    r.snap.prompt = Some(ButtonPrompt { kind: PromptKind::RiskDodge, target: TargetId::NONE });
    r.frames(20, PadButtons::NONE, StateId::Idle);
    assert_eq!(r.notice(), (Some(0xC), Some("loop".into())));
    save("hud_notice_dodge.png", 1920, 1080, &r.shot());
    // a different notice while one stands: the first goes, the new one comes in
    r.snap.prompt = attack(PromptKind::Counter(3));
    r.frames(1, PadButtons::NONE, StateId::Idle);
    assert_eq!(r.notice(), (Some(0xB), Some("in".into())));
    // the same notice from another source: KH3 ignores the second show, it simply stays
    r.frames(20, PadButtons::NONE, StateId::Idle);
    r.snap.prompt = attack(PromptKind::Reaction(2));
    r.frames(1, PadButtons::NONE, StateId::Idle);
    assert_eq!(r.notice(), (Some(0xB), Some("loop".into())));
    r.snap.prompt = None;
    r.frames(60, PadButtons::NONE, StateId::Idle);
}

#[test]
fn prompt_ui_the_gauge_row_and_the_gate_per_leader() {
    let Some(mut r) = Rig::new() else { return };
    let gauge_px = |r: &Rig| chevron_px(&r.shot(), (0, 560, 520, 900));
    let base = gauge_px(&r);
    let mut seen = Vec::new();
    for (ratio, phase, shown) in [(0.1, 0, false), (0.3, 1, true), (0.6, 2, true), (0.8, 3, true), (1.0, 3, false)] {
        r.snap.situation.gauge_ratio = ratio;
        r.frames(60, PadButtons::NONE, StateId::Idle);
        assert_eq!(r.hud.gauge_row_state(), (shown, phase), "ratio {ratio}");
        let px = gauge_px(&r);
        eprintln!("ratio {ratio}: row {shown} phase {phase}, chevron pixels {px} (none: {base})");
        seen.push(px);
        let shot = r.shot();
        save(&format!("hud_gauge_{:02}.png", (ratio * 100.0) as u32), 1920, 1080, &shot);
    }
    assert!(seen[1] > base + 50 && seen[2] > seen[1] + 50 && seen[3] > seen[2] + 50, "1 / 2 / 3 chevrons: {seen:?}");
    assert!(seen[4] < seen[1], "gone at full: {seen:?}");
    // THE GATE: a friend leading (his own prompts: none) -> no Sora gauge, no Sora notice on his HUD
    r.snap.situation.gauge_ratio = 0.0;
    r.frames(10, PadButtons::NONE, StateId::Idle);
    r.snap.situation.gauge_ratio = 0.6;
    r.snap.prompt = attack(PromptKind::Counter(3));
    r.frames(30, PadButtons::NONE, StateId::Idle);
    assert_eq!((r.hud.gauge_row_state(), r.notice().0), ((true, 2), Some(0xB)), "Sora leads: his");
    let soras = r.snap.clone();
    YPrompts::default().apply(&mut r.snap);
    r.frames(30, PadButtons::NONE, StateId::Idle);
    assert_eq!((r.hud.gauge_row_state().0, r.notice().0), (false, None), "a friend leads: his own (none)");
    // back to Sora
    r.snap = soras;
    r.frames(30, PadButtons::NONE, StateId::Idle);
    assert_eq!((r.hud.gauge_row_state().0, r.notice().0), (true, Some(0xB)));
    // THE KH3 REFERENCE FRAME: the shotlock panel up, the gauge at two chevrons, "ATTACK!" (the airstep's enemy stop)
    r.snap.command.shoot_flow = Some(ShootFlowPanel { kind: 1, enabled: [true, true], locks: 3, max_locks: 10, aim_time: 2.0, aim_time_start: 3.0, is_max: false });
    r.snap.command.state = kh_core::command::MenuState::ShootFlow;
    r.snap.prompt = None;
    r.frames(2, PadButtons::NONE, StateId::Idle);
    r.snap.prompt = attack(PromptKind::AirstepAttack);
    r.frames(40, PadButtons::NONE, StateId::Idle);
    let ours = r.shot();
    save("hud_reference_frame.png", 1920, 1080, &ours);
    if let Ok((rw, rh, buf)) = kh_ui::def::read_png(&root().join("build/sora_tmp/weapons/PROMPT-UI/user_kh3_attack_prompt.png")) {
        let (rw, rh) = (rw as usize, rh as usize);
        let ch = buf.len() / (rw * rh).max(1);
        // the reference's game picture is x 180 .. 1820 of 2048 x 946 (16:9 inside it): KH3's 1920 x 1080 = 1640 x 922
        let k = 1640.0 / 1920.0;
        let at = |x: f32, y: f32, w: f32, h: f32| ((180.0 + x * k) as usize, (y * k + 12.0) as usize, (w * k) as usize, (h * k) as usize);
        let (w, h) = (1200, 330);
        let theirs = crop(&buf, rw, ch, at(0.0, 700.0, 1920.0, 380.0), (w * 2, h));
        let mine = crop(&ours, 1920, 4, (0, 700, 1920, 380), (w * 2, h));
        save("hud_vs_kh3_reference.png", w * 2, h * 2 + 8, &above_below(&theirs, &mine, w * 2, h));
    }
}
