//! (THE USER 2026-11-02) THE CAMP MENU'S BUTTON GUIDE AFTER THE PARTY SELECT, AND BOC'S OUTFITS SCREEN, THROUGH THE HUD,
//! OFFLINE (the path of both hosts), drawn by the stage root the hosts use. Needs the UI pack (skipped without it).
//! - "The camp menu also has the 'Add / Remove' guide above there after using the party select": the save point's Party
//!   Formation closed with its guide up, the camp menu opened after it shows no guide on its top level (KH3's), and
//!   the same for a sub screen closed with Menu (Items' "Discard").
//! - "Move the Outfits selection to Boc, replacing his armor alteration menu with it completely (this means it is only
//!   accessible by speaking with Boc)": `Hud::open_outfits` (kh-er: Boc's "Alter garments") opens the Outfits screen
//!   alone, A wears, B closes it (the host is told once); the camp menu has no Outfits entry.
//!
//! Pictures: build/sora_tmp/er/boc_outfits/*.png (to be looked at).

use kh_core::{PadButtons, Sora, TargetId, Vec3};
use kh_host::hud::{Frame, Hud, HudTargets, TargetView};
use kh_host::hud_layers::stage_root;
use kh_ui::camp::{CampScreen, TopEntry};

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

fn shoot(hud: &Hud, fonts: &kh_ui::Fonts, out: &std::path::Path, name: &str) {
    let movie = hud.movie("CampMenu").expect("the camp menu's movie");
    let (w, h) = (1920, 1080);
    let (root, _) = stage_root(w as f32, h as f32);
    let mut canvas = kh_ui::Canvas::new(w, h);
    kh_ui::Renderer::new(fonts.clone()).draw(&mut canvas, movie, &root);
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|_, y| [0.30 + 0.1 * (y as f32 / 1080.0), 0.38, 0.46], &mut rgba);
    kh_ui::def::write_png(&out.join(format!("{name}.png")), w as u32, h as u32, &rgba).unwrap();
}

const PLATE: &str = "stageFrame.stageObjR.btnInfo";
const GUIDE: &str = "stageFrame.stageObjR.btnInfo.btnInfoText.txText.textField";

/// The guide plate is up, and its text.
fn guide(hud: &Hud) -> (bool, String) {
    let movie = hud.movie("CampMenu").expect("the camp menu's movie");
    (matches!(movie.current_label(PLATE), Some("in" | "loop" | "in2")), movie.text(GUIDE).unwrap_or_default())
}

fn sora() -> Sora {
    Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None)))
}

fn load() -> Option<(Hud, kh_ui::Fonts, std::path::PathBuf)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root.join("build/sora_pack/ui");
    let Ok((hud, fonts)) = Hud::load(&dir) else {
        eprintln!("no UI pack at {}: skipped", dir.display());
        return None;
    };
    let out = root.join("build/sora_tmp/er/boc_outfits");
    let _ = std::fs::create_dir_all(&out);
    Some((hud, fonts, out))
}

/// (THE USER 2026-11-02: "The camp menu also has the 'Add / Remove' guide above there after using the party select")
#[test]
fn the_top_level_has_its_own_guide_after_the_party_select() {
    let Some((mut hud, fonts, out)) = load() else { return };
    let sora = sora();
    // the save point's Party Formation: its guide up
    assert!(hud.open_party_formation(&kh_host::formation::Formation::default()));
    frames(&mut hud, &sora, 90, &[]);
    let (up, text) = guide(&hud);
    assert!(up && text.contains("Add / Remove"), "Party Formation's guide: {up} {text:?}");
    shoot(&hud, &fonts, &out, "a_party_formation_guide");
    // B closes it (the host goes back to its save-point menu)
    frames(&mut hud, &sora, 60, &["menu cancel"]);
    assert!(hud.take_formation_closed() && hud.camp_screen().is_none());
    // THE USER'S CASE: the camp menu opened afterwards: its top level, no guide (KH3's top shows none in the port)
    assert!(hud.open_camp_menu());
    frames(&mut hud, &sora, 60, &[]);
    assert_eq!(hud.camp_screen(), Some(CampScreen::Top));
    let (up, text) = guide(&hud);
    assert!(!up, "the top level after the party select shows no guide, the plate showed {text:?}");
    shoot(&hud, &fonts, &out, "b_top_after_party_select");
    // the same leak through any screen: Items ("Discard") left with Menu, the menu opened again
    let at = hud.camp_menu().map(|c| c.entries.iter().position(|e| *e == TopEntry::Items).unwrap()).unwrap();
    for _ in 0..at {
        frames(&mut hud, &sora, 4, &["menu down"]);
    }
    frames(&mut hud, &sora, 40, &["menu decide"]);
    assert_eq!(hud.camp_screen(), Some(CampScreen::Items));
    let (up, text) = guide(&hud);
    assert!(up && !text.is_empty(), "Items has its own guide: {text:?}");
    shoot(&hud, &fonts, &out, "c_items_guide");
    // B back to the top: no guide there either
    frames(&mut hud, &sora, 40, &["menu cancel"]);
    assert_eq!(hud.camp_screen(), Some(CampScreen::Top));
    assert!(!guide(&hud).0, "the top level after Items");
    frames(&mut hud, &sora, 40, &["menu decide"]);
    frames(&mut hud, &sora, 60, &["menu menu"]);
    assert!(hud.camp_screen().is_none(), "Menu closed the whole menu from Items");
    assert!(hud.open_camp_menu());
    frames(&mut hud, &sora, 60, &[]);
    assert!(!guide(&hud).0, "the top level after Items was left with Menu");
    shoot(&hud, &fonts, &out, "d_top_after_items");
    eprintln!("pictures: {}", out.display());
}

/// (THE USER 2026-11-02: "Move the Outfits selection to Boc ... only accessible by speaking with Boc")
#[test]
fn boc_s_outfits_screen_opens_alone_and_b_closes_it_and_the_camp_has_no_entry() {
    let Some((mut hud, fonts, out)) = load() else { return };
    let sora = sora();
    assert!(!hud.open_outfits(), "no outfits offered: nothing opens");
    let list = vec![("kh3".to_string(), "KINGDOM HEARTS III".to_string()), ("kh1".to_string(), "KINGDOM HEARTS".to_string()), ("kh2".to_string(), "KINGDOM HEARTS II".to_string())];
    hud.outfit_menu = Some((list, "kh3".to_string()));
    // THE CAMP MENU: no Outfits entry (the outfits are offered all the same)
    assert!(hud.open_camp_menu());
    frames(&mut hud, &sora, 60, &[]);
    let entries = hud.camp_menu().map(|c| c.entries.clone()).unwrap();
    assert!(!entries.contains(&TopEntry::Outfits), "the camp menu has no Outfits entry: {entries:?}");
    shoot(&hud, &fonts, &out, "e_camp_top_no_outfits");
    frames(&mut hud, &sora, 60, &["menu cancel"]);
    assert!(hud.camp_screen().is_none());
    let _ = hud.take_sounds();
    // BOC'S "ALTER GARMENTS": the Outfits screen alone, the cursor on the one worn
    assert!(hud.open_outfits() && hud.outfits_up());
    frames(&mut hud, &sora, 90, &[]);
    assert!(hud.outfits_shown());
    assert_eq!(hud.camp_screen(), Some(CampScreen::Outfits));
    assert_eq!(hud.camp_menu().map(|c| (c.entries.clone(), c.outfits.cursor())), Some((vec![TopEntry::Outfits], Some(0))));
    assert!(!hud.take_sounds().is_empty(), "the camp's open sound");
    shoot(&hud, &fonts, &out, "f_boc_outfits");
    // pad: down, A wears KH1 (the choice once), the screen stays
    frames(&mut hud, &sora, 6, &["menu down"]);
    frames(&mut hud, &sora, 20, &["menu decide"]);
    assert_eq!(hud.take_outfit_choice().as_deref(), Some("kh1"));
    assert!(!hud.take_sounds().is_empty(), "the cursor and decide sounds");
    assert_eq!(hud.camp_screen(), Some(CampScreen::Outfits));
    shoot(&hud, &fonts, &out, "g_boc_outfits_kh1_worn");
    // B: the menu closes, the host is told once (kh-er: Boc's script reads his window closed, his talk list comes back)
    assert!(!hud.take_outfits_closed());
    frames(&mut hud, &sora, 60, &["menu cancel"]);
    assert!(hud.camp_screen().is_none(), "B closed Boc's menu, not to a top level");
    assert!(hud.take_outfits_closed(), "closed: told once");
    assert!(!hud.take_outfits_closed() && !hud.outfits_up() && !hud.outfits_shown());
    // the next camp menu is the ordinary one (no Outfits, no leftover screen)
    assert!(hud.open_camp_menu());
    frames(&mut hud, &sora, 60, &[]);
    assert_eq!(hud.camp_screen(), Some(CampScreen::Top));
    assert!(!hud.camp_menu().unwrap().entries.contains(&TopEntry::Outfits) && !hud.camp_menu().unwrap().outfits_only);
    assert!(!guide(&hud).0, "no guide on the top level");
    shoot(&hud, &fonts, &out, "h_camp_top_after_boc");
    eprintln!("pictures: {}", out.display());
}
