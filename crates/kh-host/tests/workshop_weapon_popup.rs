//! (weapons P6b, plan Q4 + P9b's hold) A SYNTHESIZED WEAPON GIVES EXACTLY ONE OBTAINED POPUP, AFTER KH3'S OWN RESULT, end
//! to end through the HUD (its Workshop as INT wired it): the Moogle's Workshop opens on `hud_workshop::open`, the pad
//! synthesizes Warhammer+ (IS_12), KH3's large item-get popup shows the result while the weapon's OBTAINED popup is held
//! (`bossrewards::hold_weapon_popups`), A closes the result (the hold goes), B closes the Workshop, and the HUD shows the
//! OBTAINED popup once. Its own test binary (its own process): the hold and the Workshop's request are process-wide and
//! no other test of the crate may flip them meanwhile. Skipped without the UI pack.

use kh_core::{PadButtons, Sora, Vec3};
use kh_host::hud::{Frame, Hud, NoTargets};
use kh_host::hud_layers::{layer_index, LayerEvent, Layers, LAYERS};
use kh_host::hud_workshop::{self, Entry, Outcome};

fn same(p: Vec3) -> Vec3 {
    p
}

fn step(hud: &mut Hud, sora: &Sora, words: &[&str]) {
    let snapshot = kh_core::UiSnapshot::default();
    let words: Vec<String> = words.iter().map(|w| w.to_string()).collect();
    hud.frame(&mut Frame { snapshot: &snapshot, sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 0, targets: &NoTargets, to_view: same, view: None, words: &words, dt: 1.0 / 60.0, ticks: 1, pressed: PadButtons::NONE, nav: Vec::new() });
}

fn wait(hud: &mut Hud, sora: &Sora, n: usize) {
    for _ in 0..n {
        step(hud, sora, &[]);
    }
}

#[test]
fn a_synthesized_weapon_gives_one_obtained_popup_after_the_result() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/ui");
    let Ok((mut hud, fonts)) = Hud::load(&dir) else {
        eprintln!("SKIPPED: no UI pack");
        return;
    };
    hud.obtained_on = true;
    if !hud.obtained_available() {
        eprintln!("SKIPPED: no KeyItem movie");
        return;
    }
    let sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None)));
    // the record: IC_8 achieved (Warhammer+ and Clockwork Shield+ listed), the stock for one Warhammer+
    let recipe = kh_host::workshop::tables().recipe("IS_12").expect("IS_12").clone();
    let weapon = kh_host::weapons::by_key(&recipe.reward).expect("IS_12 makes a weapon").clone();
    kh_host::weapons::with(|s| {
        s.workshop = kh_host::weapons::Workshop::default();
        s.workshop.goals.push("IC_8".into());
        for (k, n) in &recipe.materials {
            s.workshop.stock.insert(k.clone(), *n);
        }
    });
    kh_host::items::take_given();
    assert!(hud_workshop::open(Entry::Workshop, "Roundtable Hold", &|_| false));
    wait(&mut hud, &sora, 40);
    if !hud_workshop::is_up() {
        eprintln!("SKIPPED: no Shop movie (the Workshop could not open)");
        return;
    }
    assert!(hud.paused(), "the Workshop holds the world");
    // Synthesize Items, the Weapons tab (All -> Items -> Weapons), Warhammer+ (its first row), A, Yes
    for word in ["menu decide", "menu r2", "menu r2"] {
        step(&mut hud, &sora, &[word]);
        wait(&mut hud, &sora, 25);
    }
    step(&mut hud, &sora, &["menu decide"]);
    wait(&mut hud, &sora, 10);
    step(&mut hud, &sora, &["menu left"]);
    step(&mut hud, &sora, &["menu decide"]);
    wait(&mut hud, &sora, 3);
    assert_eq!(kh_host::weapons::with(|s| s.workshop.created.get("IS_12").copied()), Some(1), "Warhammer+ made");
    assert!(kh_host::bossrewards::weapon_popups_held(), "KH3's own result is up: the OBTAINED popup waits");
    // (in-game bug, combined round D3 / D4) KH3's result popup is ROUTED ABOVE the Workshop's screen: the HUD's layers are
    // drawn in `LAYERS` order; the KeyItem movie must be the topmost layer shown, above "CampMenu", and the compositor
    // (the same `Layers` the Elden Ring overlay uses) must show a picture on that layer
    let shown: Vec<(usize, &str)> = LAYERS.iter().enumerate().filter_map(|(k, name)| hud.movie(name).map(|m| (k, m.def.name.as_str()))).collect();
    let popup_at = shown.iter().find(|s| s.1 == kh_ui::key_item::MOVIE).map(|s| s.0).unwrap_or_else(|| panic!("the result popup is on no layer: {shown:?}"));
    assert!(popup_at > layer_index("CampMenu") && shown.iter().all(|s| s.0 <= popup_at), "the result popup must be the topmost layer: {shown:?}");
    let mut layers = Layers::new(fonts.clone(), true);
    let mut pictures = Vec::new();
    layers.update(&hud, &kh_ui::geom::Matrix::new(1.0, 0.0, 0.0, 1.0, 0.0, 0.0), (1920.0, 1080.0), true, &mut |k, e| {
        if let LayerEvent::Show { rect, .. } = e {
            pictures.push((k, rect));
        }
    });
    assert!(pictures.iter().any(|p| p.0 == popup_at && p.1 .2 > 0 && p.1 .3 > 0), "the compositor draws the result popup's layer: {pictures:?}");
    assert!(!hud.obtained_open() && hud.obtained_waiting() == 0, "nothing of the OBTAINED popup yet");
    // A closes KH3's result: the hold goes
    for _ in 0..300 {
        if !kh_host::bossrewards::weapon_popups_held() {
            break;
        }
        step(&mut hud, &sora, &["menu decide"]);
    }
    assert!(!kh_host::bossrewards::weapon_popups_held(), "the result closed: the hold is off");
    assert!(!hud.obtained_open(), "the Workshop still has the screen");
    // B, B: the Workshop closes; the host is told once
    let mut told = None;
    for _ in 0..4 {
        step(&mut hud, &sora, &["menu cancel"]);
        for _ in 0..60 {
            step(&mut hud, &sora, &[]);
            if let Some(r) = hud_workshop::take_result() {
                told = Some(r);
            }
        }
        if told.is_some() {
            break;
        }
    }
    assert_eq!(told, Some(Outcome::Closed(Entry::Workshop)));
    // exactly one OBTAINED popup, for the weapon made
    wait(&mut hud, &sora, 3);
    assert!(hud.obtained_open() && hud.obtained_waiting() == 0, "one OBTAINED popup, after the result");
    for _ in 0..300 {
        if !hud.obtained_open() {
            break;
        }
        step(&mut hud, &sora, &["menu decide"]);
    }
    wait(&mut hud, &sora, 10);
    assert!(!hud.obtained_open() && hud.obtained_waiting() == 0, "exactly one: {}", weapon.name);
}
