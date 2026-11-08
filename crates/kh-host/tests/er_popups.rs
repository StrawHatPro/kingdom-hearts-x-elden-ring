//! (ER-POPUPS) ELDEN RING'S POP-UP BOXES AS KH3'S DIALOG WINDOW, THROUGH THE REAL HUD. The user (2026-11-07): "evergaol pop
//! up boxes, as well as anything else still using Elden Ring's pop up boxes needs a Kingdom Hearts equivalent, same as what
//! we've already replaced". Per box TYPE: a question (Yes / No, the cursor on Yes) and a message (one button, KH3's "OK");
//! the window holds the world, takes the pad (D-pad / A / B) and the keyboard's menu words, answers once per id; it waits
//! for the OBTAINED popup and the ally question. Offline pictures of the KH versions (every layer drawn by the kh_ui
//! software renderer): build/sora_tmp/er_popups/shots/. Needs the UI pack.

use kh_core::{PadButton, PadButtons, Sora, TargetId, Vec3};
use kh_host::hud::{ErButtons, Frame, Hud, HudTargets, ObtainedKind, TargetView};
use kh_host::hud_layers::{stage_root, LAYERS};
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

const W: usize = 1920;
const H: usize = 1080;

fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn load() -> Option<(Hud, Sora)> {
    let Ok((mut hud, _)) = Hud::load(&root().join("build/sora_pack/ui")) else {
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

fn press(b: PadButton) -> PadButtons {
    PadButtons::NONE.with(b)
}

fn word(w: &str) -> Vec<String> {
    vec![w.to_string()]
}

/// The whole screen (every HUD layer over a dusk-coloured backdrop) and the window's middle, full size.
fn shot(hud: &Hud, name: &str) {
    let out = root().join("build/sora_tmp/er_popups/shots");
    std::fs::create_dir_all(&out).unwrap();
    let fonts = kh_ui::Fonts::load(&mut kh_ui::Pack::open(root().join("build/sora_pack/ui")).unwrap()).unwrap();
    let (stage, _) = stage_root(W as f32, H as f32);
    let mut canvas = kh_ui::Canvas::new(W, H);
    let mut renderer = kh_ui::Renderer::new(fonts);
    for layer in LAYERS {
        if let Some(movie) = hud.movie(layer) {
            renderer.draw(&mut canvas, movie, &stage);
        }
    }
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|x, y| [0.18 + 0.10 * (y as f32 / H as f32), 0.22 + 0.05 * (x as f32 / W as f32), 0.20], &mut rgba);
    kh_ui::def::write_png(&out.join(format!("{name}.png")), W as u32, H as u32, &rgba).unwrap();
}

/// THE EVERGAOL: "Enter evergaol?" as KH3's Dialog window, Yes / No, the cursor on Yes; the world held; A takes Yes.
#[test]
fn er_popups_the_evergaol_question_through_the_hud() {
    let Some((mut hud, sora)) = load() else { return };
    assert!(hud.er_dialog_available(), "the pack has the Dialog movie");
    idle(&mut hud, &sora, 5);
    assert!(hud.ask_er_dialog(1, "Enter evergaol?", ErButtons::YesNo));
    assert!(hud.ask_er_dialog(1, "Enter evergaol?", ErButtons::YesNo), "asked twice: kept once");
    assert_eq!(hud.er_dialog_pending(), vec![1]);
    idle(&mut hud, &sora, 40);
    assert!(hud.er_dialog_open() && hud.paused(), "up, the world held");
    assert!(hud.movie("Dialog").is_some_and(|m| m.def.name == "Dialog"), "KH3's Dialog window on the Dialog layer");
    shot(&hud, "01_evergaol_yes_no");
    // Start / Y / X / LB do nothing; the window keeps the pad
    for b in [PadButton::SpecialRight, PadButton::FaceTop, PadButton::FaceLeft, PadButton::LeftShoulder] {
        frame(&mut hud, &sora, press(b), &[]);
        assert!(hud.er_dialog_open() && hud.movie("CampMenu").is_none(), "{b:?}");
    }
    assert!(hud.take_er_dialog_answer(1).is_none());
    let yes = frame(&mut hud, &sora, press(PadButton::FaceBottom), &[]);
    assert!(yes.iter().any(|l| l.starts_with("static SE 1 ")), "Yes: the decide sound: {yes:?}");
    assert_eq!(hud.take_er_dialog_answer(1), Some(true));
    assert_eq!(hud.take_er_dialog_answer(1), None, "once");
    idle(&mut hud, &sora, 40);
    assert!(!hud.paused() && !hud.er_dialog_open() && hud.movie("Dialog").is_none(), "the window gone, the world runs");
}

/// No: the D-pad / keyboard down moves to No (the cursor sound), A takes it; B answers No from Yes.
#[test]
fn er_popups_no_and_b() {
    let Some((mut hud, sora)) = load() else { return };
    hud.ask_er_dialog(7, "Use Stonesword Key?", ErButtons::YesNo);
    idle(&mut hud, &sora, 40);
    let moved = frame(&mut hud, &sora, PadButtons::NONE, &word("menu down"));
    assert!(moved.iter().any(|l| l.starts_with("static SE 0 ")), "the cursor sound: {moved:?}");
    idle(&mut hud, &sora, 20);
    shot(&hud, "02_stonesword_cursor_on_no");
    let no = frame(&mut hud, &sora, press(PadButton::FaceBottom), &[]);
    assert!(no.iter().any(|l| l.starts_with("static SE 3 ")), "No: the close sound: {no:?}");
    assert_eq!(hud.take_er_dialog_answer(7), Some(false));
    idle(&mut hud, &sora, 40);
    hud.ask_er_dialog(8, "Travel to another location?", ErButtons::YesNo);
    idle(&mut hud, &sora, 40);
    frame(&mut hud, &sora, press(PadButton::FaceRight), &[]);
    assert_eq!(hud.take_er_dialog_answer(8), Some(false), "B = No (Elden Ring's B gives its right answer too)");
    idle(&mut hud, &sora, 40);
    // the keyboard alone
    hud.ask_er_dialog(9, "Return to entrance?", ErButtons::YesNo);
    idle(&mut hud, &sora, 40);
    frame(&mut hud, &sora, PadButtons::NONE, &word("menu decide"));
    assert_eq!(hud.take_er_dialog_answer(9), Some(true), "the keyboard's decide on Yes");
}

/// A MESSAGE: one button, KH3's "OK"; the directions do nothing; A = OK (true), B closes it (false).
#[test]
fn er_popups_a_message_with_ok() {
    let Some((mut hud, sora)) = load() else { return };
    hud.ask_er_dialog(3, "Cannot open from this side", ErButtons::Ok);
    idle(&mut hud, &sora, 40);
    assert!(hud.er_dialog_open() && hud.paused());
    shot(&hud, "03_message_ok");
    let moved = frame(&mut hud, &sora, PadButtons::NONE, &word("menu down"));
    assert!(!moved.iter().any(|l| l.starts_with("static SE 0 ")), "no second button to move to: {moved:?}");
    frame(&mut hud, &sora, press(PadButton::FaceBottom), &[]);
    assert_eq!(hud.take_er_dialog_answer(3), Some(true));
    idle(&mut hud, &sora, 40);
    hud.ask_er_dialog(4, "Used Stonesword Key", ErButtons::Ok);
    idle(&mut hud, &sora, 40);
    frame(&mut hud, &sora, press(PadButton::FaceRight), &[]);
    assert_eq!(hud.take_er_dialog_answer(4), Some(false), "B closes it");
    idle(&mut hud, &sora, 40);
    // a question after a message: two buttons again (the window's second button comes back)
    hud.ask_er_dialog(5, "Enter evergaol?", ErButtons::YesNo);
    idle(&mut hud, &sora, 40);
    frame(&mut hud, &sora, PadButtons::NONE, &word("menu down"));
    frame(&mut hud, &sora, press(PadButton::FaceBottom), &[]);
    assert_eq!(hud.take_er_dialog_answer(5), Some(false), "No is there again");
}

/// The other pictures: a long lore box (a war plaque), a two-line notice, an NPC invasion question, a DLC question.
#[test]
fn er_popups_pictures_of_long_texts() {
    let Some((mut hud, sora)) = load() else { return };
    let texts: [(&str, &str, ErButtons); 7] = [
        ("04_war_plaque", "The siege of Castle Morne\nA lone hero fights for his vengeance\nOnly to fall at the hand of an ally\nAnd become a fiend himself", ErButtons::Ok),
        ("05_two_lines", "Impenetrable thorns refuse all.\nNone may enter the Erdtree.", ErButtons::Ok),
        ("06_invasion", "Invade Vargram the Raging Wolf and Errant Sorcerer Wilhelm's world?", ErButtons::YesNo),
        ("07_dlc_summon", "Summon Pureblood Knight Ansbach?\nThis cooperator will not arrive until the time is right.", ErButtons::YesNo),
        // the inventory's shortest message (30040) and longest texts (61011 the longest plaque; 80613 the widest line)
        ("08_shortest_message", "Cannot be used now", ErButtons::Ok),
        ("09_longest_plaque", "Lord Godfrey, at last at the end of his campaign\nHis golden armies unvanquished and unbowed\nYet finds grace lost, tattered and faded", ErButtons::Ok),
        ("10_widest_question", "Use Festering Bloody Finger and invade Magnus the Beast Claw's world?", ErButtons::YesNo),
    ];
    for (n, (name, text, buttons)) in texts.into_iter().enumerate() {
        let id = 100 + n as u64;
        hud.ask_er_dialog(id, text, buttons);
        idle(&mut hud, &sora, 40);
        assert!(hud.er_dialog_open(), "{name}");
        shot(&hud, name);
        frame(&mut hud, &sora, press(PadButton::FaceBottom), &[]);
        assert_eq!(hud.take_er_dialog_answer(id), Some(true), "{name}");
        idle(&mut hud, &sora, 40);
    }
}

/// The order: an OBTAINED popup and an ally question first, then the boxes one after the other in the order asked; a
/// box the host cancels leaves the queue / the window; a reset takes everything.
#[test]
fn er_popups_queue_cancel_and_reset() {
    let Some((mut hud, sora)) = load() else { return };
    idle(&mut hud, &sora, 5);
    assert!(hud.show_obtained(ObtainedKind::Character { chara: PartyChara::Goofy, caption: "Captain.".into() }));
    assert!(hud.ask_ally_join(PartyChara::Goofy, "Have Goofy join the party?"));
    hud.ask_er_dialog(1, "Enter evergaol?", ErButtons::YesNo);
    hud.ask_er_dialog(2, "Used Stonesword Key", ErButtons::Ok);
    idle(&mut hud, &sora, 130);
    assert!(hud.obtained_open() && !hud.er_dialog_open(), "the OBTAINED popup first");
    frame(&mut hud, &sora, press(PadButton::FaceBottom), &[]);
    idle(&mut hud, &sora, 40);
    assert!(hud.ally_asking() && !hud.er_dialog_open(), "then the ally question");
    frame(&mut hud, &sora, press(PadButton::FaceRight), &[]);
    idle(&mut hud, &sora, 40);
    assert!(hud.er_dialog_open(), "then the first box");
    assert_eq!(hud.er_dialog_pending(), vec![1, 2]);
    frame(&mut hud, &sora, press(PadButton::FaceBottom), &[]);
    idle(&mut hud, &sora, 40);
    assert_eq!(hud.take_er_dialog_answer(1), Some(true));
    assert_eq!(hud.er_dialog_pending(), vec![2], "the second one up now");
    hud.cancel_er_dialog(2);
    assert!(!hud.er_dialog_open() && hud.er_dialog_pending().is_empty() && hud.take_er_dialog_answer(2).is_none(), "cancelled: no answer");
    idle(&mut hud, &sora, 10);
    assert!(!hud.paused());
    hud.ask_er_dialog(3, "Enter evergaol?", ErButtons::YesNo);
    hud.ask_er_dialog(4, "Enter evergaol?", ErButtons::YesNo);
    idle(&mut hud, &sora, 10);
    hud.reset_popups();
    assert!(!hud.er_dialog_open() && hud.er_dialog_pending().is_empty() && !hud.paused(), "a load takes every box");
}
