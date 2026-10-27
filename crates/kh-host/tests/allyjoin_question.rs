//! (ALLYJOIN) A NEW ALLY'S "JOIN THE PARTY?" THROUGH THE REAL HUD. The user (2026-10-27): "when gaining a new ally, have a
//! popup ask if the player wants them to join the party? If yes, open up the party creation menu usually accessed from
//! save points. If no, they are not added but can still be added at a save point like normal." The question comes after
//! his OBTAINED popup, holds the world, takes the pad (D-pad / A / B) and the keyboard's words; Yes opens the Party
//! Formation screen, whose closing does NOT send the host back to a save-point menu. Needs the UI pack.

use kh_core::{PadButton, PadButtons, Sora, TargetId, Vec3};
use kh_host::hud::{Frame, Hud, HudTargets, ObtainedKind, TargetView};
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

fn load() -> Option<(Hud, Sora)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let Ok((mut hud, _)) = Hud::load(&root.join("build/sora_pack/ui")) else {
        eprintln!("SKIPPED: no UI pack");
        return None;
    };
    hud.obtained_on = true;
    let sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None)));
    Some((hud, sora))
}

fn frame(hud: &mut Hud, sora: &Sora, pressed: PadButtons, words: &[String]) -> Vec<String> {
    let snapshot = sora.ui_snapshot();
    hud.frame(&mut Frame { snapshot: &snapshot, sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 0, targets: &NoTargets, to_view: same, view: None, words, dt: 1.0 / 60.0, ticks: 1, pressed, nav: Vec::new() });
    hud.apply_actions(&mut sora.clone());
    hud.take_sounds().into_iter().map(|(_, line)| line).collect()
}

fn idle(hud: &mut Hud, sora: &Sora, n: usize) -> Vec<String> {
    (0..n).flat_map(|_| frame(hud, sora, PadButtons::NONE, &[])).collect()
}

fn a() -> PadButtons {
    PadButtons::NONE.with(PadButton::FaceBottom)
}

#[test]
fn allyjoin_the_question_follows_the_obtained_popup_and_no_leaves_him_out() {
    let Some((mut hud, sora)) = load() else { return };
    assert!(hud.ally_ask_available(), "the pack has the Dialog movie");
    idle(&mut hud, &sora, 5);
    assert!(hud.show_obtained(ObtainedKind::Character { chara: PartyChara::Goofy, caption: "Captain.".into() }));
    assert!(hud.ask_ally_join(PartyChara::Goofy, "Have Goofy join the party?"));
    idle(&mut hud, &sora, 3);
    assert!(hud.obtained_open() && !hud.ally_asking(), "his OBTAINED popup first");
    assert_eq!(hud.ally_pending(), vec![PartyChara::Goofy]);
    idle(&mut hud, &sora, 130);
    frame(&mut hud, &sora, a(), &[]);
    idle(&mut hud, &sora, 40);
    assert!(!hud.obtained_open(), "the popup closed by A");
    assert!(hud.ally_asking() && hud.paused(), "then the question, the world held");
    assert!(hud.movie("Dialog").is_some_and(|m| m.def.name == "Dialog"), "KH3's Dialog window on the Dialog layer");
    assert!(hud.take_ally_answer().is_none());
    // Start / Y / X do nothing; down moves to No (the menus' cursor sound), A takes it (the close sound)
    for b in [PadButton::SpecialRight, PadButton::FaceTop, PadButton::FaceLeft] {
        frame(&mut hud, &sora, PadButtons::NONE.with(b), &[]);
        assert!(hud.ally_asking() && hud.movie("CampMenu").is_none(), "{b:?}");
    }
    let moved = frame(&mut hud, &sora, PadButtons::NONE, &["menu down".to_string()]);
    assert!(moved.iter().any(|l| l.starts_with("static SE 0 ")), "the cursor sound: {moved:?}");
    let no = frame(&mut hud, &sora, a(), &[]);
    assert!(no.iter().any(|l| l.starts_with("static SE 3 ")), "No: the close sound: {no:?}");
    assert_eq!(hud.take_ally_answer(), Some((PartyChara::Goofy, false)));
    idle(&mut hud, &sora, 30);
    assert!(!hud.paused() && !hud.ally_asking() && hud.ally_pending().is_empty(), "the world runs again");
    assert!(hud.movie("CampMenu").is_none(), "No opens no screen");
    assert!(hud.movie("Dialog").is_none(), "the window is gone");
}

#[test]
fn allyjoin_b_is_no_and_yes_opens_the_party_formation_screen() {
    let Some((mut hud, sora)) = load() else { return };
    idle(&mut hud, &sora, 5);
    // B on the question: No
    assert!(hud.ask_ally_join(PartyChara::Donald, "Have Donald join the party?"));
    idle(&mut hud, &sora, 3);
    assert!(hud.ally_asking());
    frame(&mut hud, &sora, PadButtons::NONE.with(PadButton::FaceRight), &[]);
    assert_eq!(hud.take_ally_answer(), Some((PartyChara::Donald, false)), "B answers No");
    idle(&mut hud, &sora, 30);
    // A on the first button: Yes, and the save point's Party Formation screen opens; the world held all the way
    assert!(hud.ask_ally_join(PartyChara::Sora, "Have Sora join the party?"));
    idle(&mut hud, &sora, 3);
    let yes = frame(&mut hud, &sora, a(), &[]);
    assert!(yes.iter().any(|l| l.starts_with("static SE 1 ")), "Yes: the decide sound: {yes:?}");
    assert_eq!(hud.take_ally_answer(), Some((PartyChara::Sora, true)));
    let mut up = false;
    for _ in 0..60 {
        frame(&mut hud, &sora, PadButtons::NONE, &[]);
        assert!(hud.paused(), "the world stays held from the answer to the screen");
        if hud.movie("CampMenu").is_some() {
            up = true;
            break;
        }
    }
    assert!(up, "the Party Formation screen is up");
    // B until it is gone: the host is not sent back to a save-point menu
    for _ in 0..40 {
        if !hud.paused() {
            break;
        }
        frame(&mut hud, &sora, PadButtons::NONE.with(PadButton::FaceRight), &[]);
        idle(&mut hud, &sora, 20);
    }
    assert!(!hud.paused() && hud.movie("CampMenu").is_none(), "closed");
    assert!(!hud.take_formation_closed(), "no save-point menu to go back to");
}
