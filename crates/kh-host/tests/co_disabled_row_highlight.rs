//! (CAMP-OPTIONS, BUTTON-ICONS's report 2026-11-05: "the disabled Adjust Image Quality row keeps its highlight after the cursor
//! passes over it") A DISABLED CONFIG ROW'S HIGHLIGHT CLEARS WHEN THE CURSOR LEAVES IT, as KH3's Config rows do: the row and its
//! background clip back on their disabled frames, and stay there. Needs the UI pack (skipped without it).

use kh_core::{PadButtons, Sora, TargetId, Vec3};
use kh_host::er_options::{Controller, MockGame};
use kh_host::hud::{Frame, Hud, HudTargets, TargetView};

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

const PACK: &str = "config.configMenu.configPack";

/// The row slot (setConfigValueN or an extra slider) whose name text is `label`.
fn slot_of(hud: &Hud, label: &str) -> Option<String> {
    let movie = hud.movie("CampMenu")?;
    (1..=26).map(|n| format!("{PACK}.setConfigValue{n}")).find(|slot| movie.text(&format!("{slot}.itemName")).is_some_and(|t| t.contains(label)) && movie.node(slot).is_some_and(|n| n.is_visible()))
}

/// What the row shows: its own frame label and its background clip's.
fn state(hud: &Hud, slot: &str) -> (Option<String>, Option<String>) {
    let movie = hud.movie("CampMenu").expect("the settings screen");
    (movie.current_label(slot).map(str::to_string), movie.current_label(&format!("{slot}.listItemBgConf")).map(str::to_string))
}

#[test]
fn co_a_disabled_rows_highlight_clears_when_the_cursor_leaves() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/ui");
    let Ok((mut hud, _)) = Hud::load(&root) else {
        println!("no UI pack: skipped");
        return;
    };
    // HDR off: Display > Adjust Image Quality is a disabled row
    hud.options_host = Some(Box::new(Controller::new(MockGame::default())));
    let sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 });
    hud.settings_only = true;
    frames(&mut hud, &sora, 2, &[]);
    assert!(hud.open_options());
    frames(&mut hud, &sora, 40, &[]);
    // Game Options -> Camera Options -> Display
    frames(&mut hud, &sora, 20, &["menu r1"]);
    frames(&mut hud, &sora, 20, &["menu r1"]);
    let slot = slot_of(&hud, "Adjust Image Quality").expect("the Display page's Adjust Image Quality row");
    let at_rest = state(&hud, &slot);
    // down to the row (Subtitles, HUD, Show Tutorials, HDR, Adjust Brightness, Adjust Image Quality)
    for _ in 0..5 {
        frames(&mut hud, &sora, 12, &["menu down"]);
    }
    let on = state(&hud, &slot);
    assert_eq!(on.0.as_deref(), Some("select"), "the cursor is on the disabled row");
    // and off it: the highlight leaves and stays gone
    frames(&mut hud, &sora, 12, &["menu down"]);
    frames(&mut hud, &sora, 120, &[]);
    let off = state(&hud, &slot);
    assert_eq!(off.0.as_deref(), Some("disable"), "the row is back on its disabled frames");
    assert_ne!(off.1.as_deref(), Some("select"), "its background is not the highlight 120 frames later ({off:?}, at rest {at_rest:?})");
    assert_eq!(off.1, at_rest.1, "its background is as it was before the cursor came");
}
