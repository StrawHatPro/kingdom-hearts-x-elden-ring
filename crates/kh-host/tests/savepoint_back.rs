//! (THE TESTER 2026-11-03: "after Party Formation from the save-point menu and back, the menu's cursor returns to Save
//! instead of Party Formation") KH3'S WAY BACK: a save-point menu that opens again as the way back from one of its screens
//! plays `inBack` with the cursor on the entry that screen came from (KH3's Save -> Save, Load -> Load; here Party
//! Formation -> Party Formation); a later open of the menu starts on Save as before. The path of the Elden Ring host
//! (kh-er `reopen_after_formation`: `take_formation_closed`, then `open_savepoint_grace`). Needs the UI pack (skipped
//! without it).

use kh_core::{PadButtons, Sora, TargetId, Vec3};
use kh_host::hud::{Frame, Hud, HudTargets, TargetView};
use kh_host::savepoint_menu::GraceMenu;
use kh_ui::savepoint::SavePointCommand;

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
        hud.frame(&mut Frame { snapshot: &snapshot, sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 100, targets: &NoTargets, to_view: same, view: None, words: &w, dt: 1.0 / 60.0, ticks: 1, pressed: PadButtons::NONE, nav: Vec::new() });
    }
}

/// The host's reopen: tried every frame until the HUD takes it (the camp menu's `out` plays first).
fn reopen(hud: &mut Hud, sora: &Sora) -> bool {
    for _ in 0..240 {
        if hud.open_savepoint_grace(GraceMenu::DEFAULT) {
            return true;
        }
        frames(hud, sora, 1, &[]);
    }
    false
}

#[test]
fn back_from_party_formation_the_cursor_is_on_party_formation() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/ui");
    let Ok((mut hud, _fonts)) = Hud::load(&dir) else {
        eprintln!("no UI pack: skipped");
        return;
    };
    let sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 });
    // a fresh open: on Save
    assert!(hud.open_savepoint_grace(GraceMenu::DEFAULT));
    frames(&mut hud, &sora, 30, &[]);
    assert_eq!(hud.savepoint_command(), Some(SavePointCommand::Save));
    frames(&mut hud, &sora, 60, &["menu cancel"]);
    let _ = hud.take_savepoint_result();
    assert!(!hud.savepoint_open());
    // Party Formation chosen (the host opens its screen), B there: the host is told to go back
    assert!(hud.open_party_formation(&kh_host::formation::Formation::default()));
    frames(&mut hud, &sora, 90, &[]);
    frames(&mut hud, &sora, 60, &["menu cancel"]);
    assert!(hud.take_formation_closed(), "B closes the screen");
    assert!(reopen(&mut hud, &sora), "the save-point menu opens again");
    assert_eq!(hud.savepoint_command(), Some(SavePointCommand::PartyFormation), "back on the entry the screen came from");
    // closed, and a later open: Save again
    frames(&mut hud, &sora, 60, &["menu cancel"]);
    let _ = hud.take_savepoint_result();
    assert!(hud.open_savepoint_grace(GraceMenu::DEFAULT));
    assert_eq!(hud.savepoint_command(), Some(SavePointCommand::Save), "an open of its own starts on Save");
}
