//! (CAMP-OPTIONS, THE USER 2026-11-05: "settings from main menu should go straight into settings menu"; "and that is JUST for
//! the main menu yes?") THE TITLE'S SETTINGS: from the first frame after the open request only the settings screen is drawn
//! (no camp top, no command menu or gauges); the in-game route is the camp menu's own (its top, then its Config entry). Needs
//! the UI pack (skipped without it).

use kh_core::{PadButtons, Sora, TargetId, Vec3};
use kh_host::er_options::{Controller, MockGame};
use kh_host::hud::{Frame, Hud, HudTargets, TargetView};
use kh_ui::camp::CampScreen;

struct NoTargets;
impl HudTargets for NoTargets {
    fn target(&self, _id: TargetId) -> Option<TargetView> {
        None
    }
}

fn same(p: Vec3) -> Vec3 {
    p
}

fn frames(hud: &mut Hud, sora: &Sora, n: u32, words: &[&str]) {
    let snapshot = sora.ui_snapshot();
    for k in 0..n {
        let w: Vec<String> = if k == 0 { words.iter().map(|s| s.to_string()).collect() } else { Vec::new() };
        hud.frame(&mut Frame { snapshot: &snapshot, sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 0, targets: &NoTargets, to_view: same, view: None, words: &w, dt: 1.0 / 60.0, ticks: 1, pressed: PadButtons::NONE, nav: Vec::new() });
    }
}

fn hud() -> Option<(Hud, Sora)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/ui");
    let (mut hud, _) = Hud::load(&root).ok()?;
    hud.options_host = Some(Box::new(Controller::new(MockGame::default())));
    let sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 });
    Some((hud, sora))
}

const HUD_MOVIES: [&str; 8] = ["HudStatus", "HudCommand", "HudTarget", "HudLeft", "HudRight", "HudOperation", "Pause", "GameHelp"];

#[test]
fn co_the_titles_settings_draw_only_the_settings_screen_from_the_first_frame() {
    let Some((mut hud, sora)) = hud() else {
        println!("no UI pack: skipped");
        return;
    };
    hud.settings_only = true; // (the title's frames, kh-er hud.rs title_options_frame)
    frames(&mut hud, &sora, 2, &[]);
    assert!(hud.open_options());
    // the frames up to the open: nothing of the in-game HUD
    for _ in 0..3 {
        frames(&mut hud, &sora, 1, &[]);
        for m in HUD_MOVIES {
            assert!(hud.movie(m).is_none(), "{m} is drawn on the title");
        }
        if hud.camp_screen().is_some() {
            break;
        }
    }
    // the first frame the menu is there: the Config page, the camp's top hidden
    assert_eq!(hud.camp_screen(), Some(CampScreen::Config), "straight to the settings, no top");
    let movie = hud.movie("CampMenu").expect("the settings screen");
    assert!(!movie.node("topMenu").is_some_and(|n| n.is_visible()), "the camp's top is not shown");
    // B leaves the whole screen (back to the title)
    frames(&mut hud, &sora, 30, &["menu cancel"]);
    frames(&mut hud, &sora, 60, &[]);
    assert!(hud.camp_screen().is_none() && hud.take_options_closed());
}

#[test]
fn co_the_in_game_system_entry_opens_the_camp_menu_then_its_config() {
    let Some((mut hud, sora)) = hud() else {
        println!("no UI pack: skipped");
        return;
    };
    frames(&mut hud, &sora, 2, &[]);
    assert!(hud.open_camp_settings());
    frames(&mut hud, &sora, 3, &[]);
    assert_eq!(hud.camp_screen(), Some(CampScreen::Top), "the camp menu's top first, as usual");
    assert!(hud.movie("CampMenu").and_then(|m| m.node("topMenu")).is_some_and(|n| n.is_visible()));
    frames(&mut hud, &sora, 120, &[]);
    assert_eq!(hud.camp_screen(), Some(CampScreen::Config), "then its Config entry");
    // B: back to the camp's top
    frames(&mut hud, &sora, 20, &["menu cancel"]);
    assert_eq!(hud.camp_screen(), Some(CampScreen::Top));
}
