//! (party PF) THE ICON-BASED PARTY FORMATION SCREEN THROUGH THE HUD, OFFLINE: the save point's screen opened with
//! `Hud::open_party_formation` (the path of both hosts: the test level's savepoint.rs and kh-er's grace menu), driven by
//! the menu keys the pad gives (D-pad / stick left / right, A, Y, B), drawn at the four output sizes (1920x1080,
//! 2560x1440, 2560x1080, 1920x1200) by the stage root the hosts use. Pictures of every state under
//! build/sora_tmp/party/pf/ (to be looked at): the default party, a leader change mid-animation and done, a member
//! removed (mid and done), a refused edit, locked characters (the Elden Ring unlocks) and a refused locked pick.
//! Needs the UI pack (skipped without it).

use kh_core::{PadButtons, Sora, TargetId, Vec3};
use kh_host::formation::Formation;
use kh_host::hud::{Frame, Hud, HudTargets, TargetView};
use kh_host::hud_layers::stage_root;
use kh_host::party::PartyChara;

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
        hud.frame(&mut Frame { snapshot: &snapshot, sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 3725, targets: &NoTargets, to_view: same, view: None, words: &w, dt: 1.0 / 60.0, ticks: 1, pressed: PadButtons::NONE, nav: Vec::new() });
    }
}

const SIZES: [(usize, usize); 4] = [(1920, 1080), (2560, 1440), (2560, 1080), (1920, 1200)];

fn shoot(hud: &Hud, fonts: &kh_ui::Fonts, out: &std::path::Path, name: &str) {
    let movie = hud.movie("CampMenu").expect("the camp menu's movie");
    for (w, h) in SIZES {
        let (root, _) = stage_root(w as f32, h as f32);
        let mut canvas = kh_ui::Canvas::new(w, h);
        kh_ui::Renderer::new(fonts.clone()).draw(&mut canvas, movie, &root);
        let mut rgba = Vec::new();
        canvas.to_rgba8_over(|_, _| [0.0, 0.0, 0.0], &mut rgba);
        kh_ui::def::write_png(&out.join(format!("{name}_{w}x{h}.png")), w as u32, h as u32, &rgba).unwrap();
    }
}

fn names(hud: &Hud) -> Vec<String> {
    hud.movie("CampMenu").map(|m| m.root.extra.iter().filter_map(|n| n.name.as_deref().map(str::to_string)).filter(|n| n.starts_with("pf_")).collect()).unwrap_or_default()
}

fn sounds(hud: &mut Hud) -> Vec<String> {
    hud.take_sounds().into_iter().map(|(_, line)| line).collect()
}

fn sora() -> Sora {
    Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None)))
}

#[test]
fn the_icon_formation_screen_in_every_state_at_every_size() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root.join("build/sora_pack/ui");
    let Ok((mut hud, fonts)) = Hud::load(&dir) else {
        eprintln!("no UI pack at {}: skipped", dir.display());
        return;
    };
    let out = root.join("build/sora_tmp/party/pf");
    let _ = std::fs::create_dir_all(&out);
    let sora = sora();
    let f = Formation::default();
    assert!(hud.open_party_formation(&f));
    frames(&mut hud, &sora, 90, &[]);
    let shown = names(&hud);
    assert!(shown.iter().any(|n| n == "pf_card0") && shown.iter().any(|n| n == "pf_icon2_face") && shown.iter().any(|n| n == "pf_bust0"), "the icon screen is up: {shown:?}");
    assert!(shown.iter().any(|n| n == "pf_cursor"), "the KH3 list cursor is placed");
    // the camp's own list and panels (and the 3D pictures on them) are hidden
    let movie = hud.movie("CampMenu").unwrap();
    assert!(!movie.node("topMenu.statusAnm").unwrap().is_visible() && !movie.node("topMenu.charaList").unwrap().is_visible());
    let _ = sounds(&mut hud);
    shoot(&hud, &fonts, &out, "a_default");

    // cursor right (Donald), Y: Donald leads; mid-animation, then done
    frames(&mut hud, &sora, 6, &["menu right"]);
    let cursor_sound = sounds(&mut hud);
    assert!(cursor_sound.iter().any(|l| l.contains("se00000_001") || l.contains(" 0")), "the cursor sound: {cursor_sound:?}");
    frames(&mut hud, &sora, 4, &["menu triangle"]);
    let decide = sounds(&mut hud);
    assert!(!decide.is_empty(), "the decide sound");
    assert_eq!(hud.take_formation_change().map(|f| f.leader()), Some(PartyChara::Donald));
    shoot(&hud, &fonts, &out, "b_leader_mid");
    frames(&mut hud, &sora, 50, &[]);
    shoot(&hud, &fonts, &out, "c_leader_done");

    // cursor right (Goofy), A: Goofy leaves; mid, then done
    frames(&mut hud, &sora, 6, &["menu right"]);
    frames(&mut hud, &sora, 5, &["menu decide"]);
    let left = hud.take_formation_change().expect("Goofy left");
    assert_eq!(left.active(), &[PartyChara::Sora, PartyChara::Donald]);
    shoot(&hud, &fonts, &out, "d_remove_mid");
    frames(&mut hud, &sora, 50, &[]);
    shoot(&hud, &fonts, &out, "e_removed");

    // cursor left (Donald, the leader), A: refused (the leader cannot leave): the error sound and the reason
    let _ = sounds(&mut hud);
    frames(&mut hud, &sora, 6, &["menu left"]);
    frames(&mut hud, &sora, 10, &["menu decide"]);
    let refused = sounds(&mut hud);
    assert!(hud.take_formation_change().is_none(), "nothing changed");
    let help = hud.movie("CampMenu").and_then(|m| m.text("topMenu.helpText.txHelpText.textField")).unwrap_or_default();
    assert!(help.contains("leader cannot leave"), "the help line says why: {help}");
    eprintln!("refused sounds: {refused:?}");
    shoot(&hud, &fonts, &out, "f_refused");

    // B: back (the host goes back to its save menu)
    frames(&mut hud, &sora, 60, &["menu cancel"]);
    assert!(hud.take_formation_closed(), "B closes the screen");

    // ELDEN RING'S UNLOCKS: Donald and Goofy locked, Sora alone; a locked pick is refused
    let (mut hud, _) = Hud::load(&dir).unwrap();
    hud.set_formation_locked(&[PartyChara::Donald, PartyChara::Goofy]);
    let alone = Formation::new(&[PartyChara::Sora], PartyChara::Sora).unwrap();
    assert!(hud.open_party_formation(&alone));
    frames(&mut hud, &sora, 90, &[]);
    shoot(&hud, &fonts, &out, "g_locked");
    frames(&mut hud, &sora, 6, &["menu right"]);
    frames(&mut hud, &sora, 10, &["menu decide"]);
    assert!(hud.take_formation_change().is_none(), "a locked character cannot join");
    shoot(&hud, &fonts, &out, "h_locked_refused");
    eprintln!("pictures: {}", out.display());
}
