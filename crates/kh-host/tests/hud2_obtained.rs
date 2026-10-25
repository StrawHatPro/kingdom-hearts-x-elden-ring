//! (HUD2) KH3'S "OBTAINED" POPUP THROUGH THE REAL HUD: which content and sound each kind gets, the pause, the close.
//! The user: key items (Elden Ring and KH), Keyblades and new party members get KH3's major item popup instead of the
//! standard notification; it pauses the game; only A closes it, with KH3's confirm sound. Needs the UI pack.

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
    hud.obtained_on = true; // (off by default: the host's key `hud.obtained`)
    let sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None)));
    Some((hud, sora))
}

/// One frame with these buttons pressed (and words); the static SE indices it played.
fn frame(hud: &mut Hud, sora: &Sora, pressed: PadButtons, words: &[String]) -> Vec<String> {
    let snapshot = sora.ui_snapshot();
    hud.frame(&mut Frame { snapshot: &snapshot, sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 0, targets: &NoTargets, to_view: same, view: None, words, dt: 1.0 / 60.0, ticks: 1, pressed, nav: Vec::new() });
    hud.take_sounds().into_iter().map(|(_, line)| line).collect()
}

fn idle(hud: &mut Hud, sora: &Sora, n: usize) -> Vec<String> {
    (0..n).flat_map(|_| frame(hud, sora, PadButtons::NONE, &[])).collect()
}

#[test]
fn hud2_each_kind_gets_its_picture_name_and_caption() {
    let Some((hud, _)) = load() else { return };
    assert!(hud.obtained_available(), "the pack has the KeyItem movie");
    // an Elden Ring key item: its own icon (the pack's er_icons by iconId), the host's name and caption
    let er = hud.obtained_content(&ObtainedKind::ErKeyItem { name: "Stonesword Key".into(), caption: "Used to break imp statue seals.".into(), icon_id: Some(3080) });
    assert_eq!((er.name.as_str(), er.caption.as_str()), ("Stonesword Key", "Used to break imp statue seals."));
    assert!(er.picture.as_ref().is_some_and(|t| t.file.contains("03080")), "Elden Ring's own picture: {:?}", er.picture.map(|t| t.file.clone()));
    // a KH3 key item: TresItemData's icon and KH3's texts
    let kh = hud.obtained_content(&ObtainedKind::KhKeyItem { key: "KEY_ITEM02".into() });
    assert_eq!(kh.name, "Gummiphone");
    assert!(kh.caption.starts_with("Chip and Dale"), "{}", kh.caption);
    assert!(kh.picture.as_ref().is_some_and(|t| t.file.contains("item302")));
    // a Keyblade (the hook)
    let kb = hud.obtained_content(&ObtainedKind::Keyblade { key: "WEP_KEYBLADE_SO_01".into() });
    assert_eq!(kb.name, "Hero's Origin");
    assert!(kb.picture.is_some() && kb.caption.contains("Strength"));
    // a new party member: KH3's name, the host's description, his menu portrait (when the roster's faces are in the pack)
    let donald = hud.obtained_content(&ObtainedKind::Character { chara: PartyChara::Donald, caption: "The royal magician.".into() });
    assert_eq!((donald.name.as_str(), donald.caption.as_str()), ("Donald Duck", "The royal magician."));
}

#[test]
fn hud2_the_popup_pauses_plays_kh3_sounds_and_only_a_closes_it() {
    let Some((mut hud, sora)) = load() else { return };
    idle(&mut hud, &sora, 5);
    assert!(!hud.paused());
    assert!(hud.show_obtained(ObtainedKind::KhKeyItem { key: "KEY_ITEM02".into() }));
    // opens at the next frame: static SE 10, the world held, on the "Pause" layer
    let opened = frame(&mut hud, &sora, PadButtons::NONE, &[]);
    assert!(opened.iter().any(|l| l.starts_with("static SE 10 ")), "KH3's open sound: {opened:?}");
    assert!(hud.paused() && hud.obtained_open(), "the popup pauses the game");
    assert!(hud.movie("Pause").is_some_and(|m| m.def.name == "KeyItem"), "drawn on the Pause layer");
    // A during the `in` does nothing (KH3's ready byte), nor do B, Start, Y
    let early = frame(&mut hud, &sora, PadButtons::NONE.with(PadButton::FaceBottom), &[]);
    assert!(hud.obtained_open() && !early.iter().any(|l| l.starts_with("static SE 1 ")));
    idle(&mut hud, &sora, 130);
    for b in [PadButton::FaceRight, PadButton::SpecialRight, PadButton::FaceTop, PadButton::FaceLeft] {
        let s = frame(&mut hud, &sora, PadButtons::NONE.with(b), &[]);
        assert!(hud.obtained_open() && hud.paused(), "{b:?} does not close it");
        assert!(s.is_empty(), "{b:?}: no sound: {s:?}");
        assert!(hud.movie("CampMenu").is_none(), "Start opens no menu over it");
    }
    // A: KH3's Decide sound (static SE 1), the `out`, then the world runs again
    let close = frame(&mut hud, &sora, PadButtons::NONE.with(PadButton::FaceBottom), &[]);
    assert!(close.iter().any(|l| l.starts_with("static SE 1 ")), "KH3's confirm sound: {close:?}");
    assert!(hud.paused(), "held while the out plays");
    idle(&mut hud, &sora, 30);
    assert!(!hud.paused() && !hud.obtained_open(), "closed: the game runs");
    assert!(hud.movie("Pause").is_none());
}

#[test]
fn hud2_popups_queue_and_wait_for_a_free_world() {
    let Some((mut hud, sora)) = load() else { return };
    idle(&mut hud, &sora, 5);
    // the camp menu is up: the popup waits for it to close
    assert!(hud.open_camp_menu());
    hud.show_obtained(ObtainedKind::ErKeyItem { name: "Stonesword Key".into(), caption: String::new(), icon_id: Some(3080) });
    hud.show_obtained(ObtainedKind::Character { chara: PartyChara::Goofy, caption: "Captain.".into() });
    idle(&mut hud, &sora, 3);
    assert!(!hud.obtained_open() && hud.obtained_waiting() == 2, "waits under a menu");
    // B until the menu is gone
    for _ in 0..40 {
        if !hud.paused() && !hud.obtained_open() {
            break;
        }
        frame(&mut hud, &sora, PadButtons::NONE.with(PadButton::FaceRight), &[]);
        idle(&mut hud, &sora, 20);
    }
    idle(&mut hud, &sora, 60);
    assert!(hud.obtained_open(), "opens when the world is free");
    // the first, then the second, each closed by A
    for n in 0..2 {
        idle(&mut hud, &sora, 130);
        frame(&mut hud, &sora, PadButtons::NONE.with(PadButton::FaceBottom), &[]);
        idle(&mut hud, &sora, 30);
        assert_eq!(hud.obtained_open(), n == 0, "popup {n}");
    }
    // the word `menu decide` (the keyboard's confirm path of the test hosts) closes it too
    hud.show_obtained(ObtainedKind::Keyblade { key: "WEP_KEYBLADE_SO_00".into() });
    idle(&mut hud, &sora, 130);
    frame(&mut hud, &sora, PadButtons::NONE, &["menu decide".to_string()]);
    idle(&mut hud, &sora, 30);
    assert!(!hud.obtained_open());
}
