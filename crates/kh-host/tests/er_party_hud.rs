//! (ER party S6) THE PARTY HUD THROUGH THE ELDEN RING HOST'S LAYER PATH, OFFLINE: the status HUD with Donald and Goofy
//! (and the leader switched to Donald: KH-style shrink and grow) drawn by `kh_host::hud_layers` at the stage root
//! `stage_root(w, h)` the Elden Ring host uses (kh-er hud.rs), at the four output sizes stream P checked in the test
//! app (1920x1080, 2560x1440, 2560x1080, 1920x1200). THE ICONS SCALE AS KH3: the picture is the 16:9 stage's (ExactFit
//! 16:9 viewport 0x1438F8E90), scaled by the stage's height and placed inside the pillar / letter box, never
//! stretched and never counted twice (the test app's letterbox bug, P2 item 3). Pictures under
//! build/sora_tmp/er/party/s6/hud/ (needs the UI pack: skipped without it).

use kh_core::{PadButtons, Sora, TargetId, Vec3};
use kh_host::hud::{Frame, Hud, HudTargets, TargetView};
use kh_host::hud_layers::{draw_movie_in, stage_root, Draw, Drawn, Perf};
use kh_host::party::{PartyChara, PartyHudSlot};

struct NoTargets;
impl HudTargets for NoTargets {
    fn target(&self, _id: TargetId) -> Option<TargetView> {
        None
    }
}

fn same(p: Vec3) -> Vec3 {
    p
}

fn frames(hud: &mut Hud, sora: &Sora, n: u32) {
    let snapshot = sora.ui_snapshot();
    for _ in 0..n {
        hud.frame(&mut Frame { snapshot: &snapshot, sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 0, targets: &NoTargets, to_view: same, view: None, words: &[], dt: 1.0 / 60.0, ticks: 1, pressed: PadButtons::NONE, nav: Vec::new() });
    }
}

#[test]
fn the_party_hud_scales_with_the_stage_at_every_output_size() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root.join("build/sora_pack/ui");
    let Ok((mut hud, fonts)) = Hud::load(&dir) else {
        eprintln!("no UI pack at {}: skipped", dir.display());
        return;
    };
    let out = root.join("build/sora_tmp/er/party/s6/hud");
    let _ = std::fs::create_dir_all(&out);
    let sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None)));
    let slots = [
        PartyHudSlot { chara: PartyChara::Donald, hp: 120, hp_max: 200, mp: 60, mp_max: 100, mp_charging: false, mp_charge_value: 0.0, in_battle: true, focus: None },
        PartyHudSlot { chara: PartyChara::Goofy, hp: 220, hp_max: 220, mp: 100, mp_max: 100, mp_charging: false, mp_charge_value: 0.0, in_battle: true, focus: None },
    ];
    let sizes = [(1920.0, 1080.0), (2560.0, 1440.0), (2560.0, 1080.0), (1920.0, 1200.0)];
    let mut found = Vec::new();
    for (phase, leader) in [("sora", PartyChara::Sora), ("donald", PartyChara::Donald)] {
        hud.set_party(&slots);
        hud.set_leader(leader, true);
        // the leader's swap: the slots shrink, the faces change, they grow back (well over a second)
        frames(&mut hud, &sora, 150);
        let mut rects = Vec::new();
        for (w, h) in sizes {
            let Some(movie) = hud.movie("HudStatus") else {
                eprintln!("no HudStatus movie: skipped");
                return;
            };
            let (stage, view) = stage_root(w, h);
            let mut draw = Draw::default();
            let mut renderer = kh_ui::render::Renderer::new(fonts.clone());
            eprintln!("{phase} {w}x{h}: bounds {:?}", renderer.bounds(movie, &stage));
            // the Elden Ring host clips the cockpit to the stage (Layers::clip): KH3's ExactFit 16:9 viewport
            let rect = match draw_movie_in(&mut draw, &mut renderer, movie, &stage, (w, h), true, Perf::default(), Some(view)) {
                Drawn::Picture(rect, _) => rect,
                _ => panic!("{phase} {w}x{h}: the status HUD drew nothing"),
            };
            let _ = kh_ui::def::write_png(&out.join(format!("status_{phase}_{w}x{h}.png")), rect.2 as u32, rect.3 as u32, &draw.rgba);
            eprintln!("{phase} {w}x{h}: stage ({:.0}, {:.0}) {:.0} x {:.0}, the status HUD's picture at ({}, {}) {} x {}", view.0, view.1, view.2, view.3, rect.0, rect.1, rect.2, rect.3);
            rects.push((rect, view));
        }
        // the reference: 1920x1080 is the stage itself
        let ((x, y, rw, rh), _) = rects[0];
        let close = |a: f32, b: f32, tol: f32| (a - b).abs() <= tol;
        for (k, ((px, py, pw, ph), (left, top, sw, sh))) in rects.iter().enumerate().skip(1) {
            let s = sh / 1080.0;
            assert!(close(*sw / *sh, 16.0 / 9.0, 0.01), "{phase} {:?}: the stage is 16:9", sizes[k]);
            // the same picture, scaled by the stage's height, placed from the stage's corner (2 px of edge rounding)
            assert!(close(*pw as f32, rw as f32 * s, 3.0 * s) && close(*ph as f32, rh as f32 * s, 3.0 * s), "{phase} {:?}: {pw} x {ph} is not {rw} x {rh} scaled by {s}", sizes[k]);
            assert!(close(*px as f32, left + x as f32 * s, 3.0 * s) && close(*py as f32, top + y as f32 * s, 3.0 * s), "{phase} {:?}: at ({px}, {py}), not at the stage's ({left}, {top}) + ({x}, {y}) x {s}", sizes[k]);
            assert!(*px as f32 >= *left - 1.0 && (*px as f32 + *pw as f32) <= left + sw + 1.0 && (*py as f32 + *ph as f32) <= top + sh + 1.0, "{phase} {:?}: nothing outside the stage (the pillar / letter box)", sizes[k]);
        }
        found.push((phase, rects[0].0));
    }
    eprintln!("pictures: {}", out.display());
    assert_eq!(found.len(), 2);
}
