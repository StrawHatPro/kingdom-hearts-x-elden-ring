//! (OUTFIT-UNLOCK, the user 2026-11-05: "once you have the sewing kit, you can access the costume menu that lets you change your
//! clothes, but it costs 50,000 munny. the selection you have depends on what needle you have given Boc"; "using Boc makes
//! costume change free") THROUGH THE HUD, OFFLINE (needs the UI pack; skipped without it):
//! - no sewing kit (the host's `outfits_entry` false): the camp menu has no Outfits entry;
//! - the kit: "Outfits" before Config; A on another outfit asks KH3's Yes / No (the outfit, 50,000 munny), Yes changes it and
//!   `apply_actions` takes 50,000 munny; the one worn is free; short of munny KH3's refusal and nothing changes;
//! - Boc's "Alter garments" (`open_outfits`): the same list, no question, nothing charged.

use kh_core::{PadButtons, Sora, TargetId, Vec3};
use kh_host::hud::{Frame, Hud, HudTargets, TargetView};
use kh_ui::camp::{CampScreen, DialogKind, TopEntry};

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

fn sora(munny: i32) -> Sora {
    let mut s = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None)));
    s.status_mut().munny = munny;
    s
}

fn load() -> Option<Hud> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/ui");
    match Hud::load(&dir) {
        Ok((hud, _)) => Some(hud),
        Err(_) => {
            eprintln!("no UI pack at {}: skipped", dir.display());
            None
        }
    }
}

/// The needles' selection the host gives (KH3's + KH1 / KH2: the Sewing Needle given).
fn needle_list() -> Vec<(String, String)> {
    vec![("kh3".to_string(), "KINGDOM HEARTS III".to_string()), ("kh2".to_string(), "KINGDOM HEARTS II".to_string()), ("kh1".to_string(), "KINGDOM HEARTS".to_string())]
}

/// The camp menu's top: the Outfits screen through its entry.
fn open_outfits_entry(hud: &mut Hud, sora: &Sora) {
    assert!(hud.open_camp_menu());
    frames(hud, sora, 60, &[]);
    let entries = hud.camp_menu().map(|c| c.entries.clone()).unwrap();
    let at = entries.iter().position(|e| *e == TopEntry::Outfits).expect("the Outfits entry");
    assert_eq!(entries.get(at + 1), Some(&TopEntry::Config), "before Config: {entries:?}");
    for _ in 0..at {
        frames(hud, sora, 4, &["menu down"]);
    }
    frames(hud, sora, 40, &["menu decide"]);
    assert_eq!(hud.camp_screen(), Some(CampScreen::Outfits));
}

#[test]
fn the_sewing_kit_opens_a_paid_outfits_entry_and_boc_s_is_free() {
    let Some(mut hud) = load() else { return };
    hud.outfit_menu = Some((needle_list(), "kh3".to_string()));
    hud.outfit_price = 50_000;
    let mut s = sora(60_000);
    // NO KIT: no entry
    assert!(hud.open_camp_menu());
    frames(&mut hud, &s, 60, &[]);
    assert!(!hud.camp_menu().unwrap().entries.contains(&TopEntry::Outfits), "no Tailoring Tools: no Outfits entry");
    frames(&mut hud, &s, 60, &["menu cancel"]);
    assert!(hud.camp_screen().is_none());
    // THE KIT: the entry, the needles' selection, the price
    hud.outfits_entry = true;
    open_outfits_entry(&mut hud, &s);
    let camp = hud.camp_menu().unwrap();
    assert_eq!((camp.outfits.outfits.clone(), camp.outfits.price, camp.outfits_only), (needle_list(), 50_000, false));
    // the one worn: free, nothing asked
    frames(&mut hud, &s, 20, &["menu decide"]);
    assert!(hud.camp_menu().unwrap().dialog.is_none() && hud.take_outfit_choice().is_none());
    // KH2: KH3's Yes / No with the price, No first; Yes changes it
    frames(&mut hud, &s, 6, &["menu down"]);
    frames(&mut hud, &s, 20, &["menu decide"]);
    let d = hud.camp_menu().unwrap().dialog.clone().expect("the price question");
    assert_eq!((d.kind, d.cursor, d.argument.as_str()), (DialogKind::YesNo, 1, "Change into KINGDOM HEARTS II?\n50,000 munny"));
    frames(&mut hud, &s, 6, &["menu left"]);
    frames(&mut hud, &s, 20, &["menu decide"]);
    assert_eq!(hud.take_outfit_choice().as_deref(), Some("kh2"));
    hud.apply_actions(&mut s);
    assert_eq!(s.status().munny, 10_000, "50,000 munny charged");
    // too little munny now: KH3's refusal, nothing changes, nothing charged
    frames(&mut hud, &s, 6, &["menu down"]);
    frames(&mut hud, &s, 20, &["menu decide"]);
    let d = hud.camp_menu().unwrap().dialog.clone().expect("the refusal");
    assert_eq!((d.text_key, d.kind), ("kh3_menu.shop.dialog_0020", DialogKind::Ok));
    frames(&mut hud, &s, 20, &["menu decide"]);
    assert!(hud.take_outfit_choice().is_none());
    hud.apply_actions(&mut s);
    assert_eq!(s.status().munny, 10_000);
    frames(&mut hud, &s, 40, &["menu menu"]);
    frames(&mut hud, &s, 60, &[]);
    assert!(hud.camp_screen().is_none());
    // BOC: the same selection, free (no question, nothing charged), though the price is set and the munny short
    let _ = hud.take_outfit_choice();
    assert!(hud.open_outfits());
    frames(&mut hud, &s, 90, &[]);
    assert_eq!(hud.camp_screen(), Some(CampScreen::Outfits));
    let camp = hud.camp_menu().unwrap();
    assert_eq!((camp.outfits.outfits.clone(), camp.outfits.price, camp.outfits_only), (needle_list(), 0, true));
    frames(&mut hud, &s, 6, &["menu down"]);
    frames(&mut hud, &s, 20, &["menu decide"]);
    assert!(hud.camp_menu().unwrap().dialog.is_none(), "no price question at Boc's");
    assert!(hud.take_outfit_choice().is_some(), "changed at once");
    hud.apply_actions(&mut s);
    assert_eq!(s.status().munny, 10_000, "nothing charged at Boc's");
}

/// The camp top as the hosts draw it, into a PNG.
fn shoot(hud: &Hud, fonts: &kh_ui::Fonts, out: &std::path::Path, name: &str) {
    let movie = hud.movie("CampMenu").expect("the camp menu's movie");
    let (w, h) = (1920, 1080);
    let (root, _) = kh_host::hud_layers::stage_root(w as f32, h as f32);
    let mut canvas = kh_ui::Canvas::new(w, h);
    kh_ui::Renderer::new(fonts.clone()).draw(&mut canvas, movie, &root);
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|_, y| [0.30 + 0.1 * (y as f32 / 1080.0), 0.38, 0.46], &mut rgba);
    kh_ui::def::write_png(&out.join(format!("{name}.png")), w as u32, h as u32, &rgba).unwrap();
}

/// The places of the top list's slots (`topMenu.cmdBtnTopMenu.topMenuN`: their matrices' y).
fn slot_ys(hud: &Hud) -> Vec<f32> {
    let movie = hud.movie("CampMenu").expect("the camp menu's movie");
    (1..=9).map(|n| movie.node(&format!("topMenu.cmdBtnTopMenu.topMenu{n}")).map_or(f32::NAN, |node| node.matrix.ty)).collect()
}

/// (OUTFIT-UNLOCK, the coordinator 2026-11-05: "the camp top with 9 rows, where Title Screen draws below the panel line") with
/// Outfits AND Title Screen the top has nine entries: the slots close up (80 apart) so the ninth stands above where KH3's eighth
/// does, above the frame's bottom band; with eight or fewer the slots keep KH3's own places (96 apart, from the movie). The
/// movie's own places are checked against the constants. Pictures: build/sora_tmp/weapons/OUTFIT-UNLOCK/shots/camp_top_{8,9}.png.
#[test]
fn nine_top_entries_close_up_inside_the_panel_and_eight_keep_kh3_s_places() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let Ok((mut hud, fonts)) = Hud::load(&root.join("build/sora_pack/ui")) else {
        eprintln!("no UI pack: skipped");
        return;
    };
    let out = root.join("build/sora_tmp/weapons/OUTFIT-UNLOCK/shots");
    let _ = std::fs::create_dir_all(&out);
    // KH3's own places in the movie (frame 1 of McCmdBtnTopMenu_11), the constants' source
    let text = std::fs::read_to_string(root.join("build/sora_pack/ui/movies/CampMenu.json")).unwrap();
    for n in 1..=9 {
        let at = text.find(&format!("\"name\":\"topMenu{n}\",\"class\":\"tres.rsl.TopMenuCmd\"")).expect("the slot's placement");
        let m = &text[text[..at].rfind("\"m\":[").unwrap() + 5..];
        let v: Vec<f32> = m[..m.find(']').unwrap()].split(',').map(|x| x.trim().parse().unwrap()).collect();
        assert_eq!((v[4], v[5]), kh_ui::camp_menu::top_slot_at(n - 1, 8), "topMenu{n}: KH3's own place");
    }
    let s = sora(60_000);
    hud.outfit_menu = Some((needle_list(), "kh3".to_string()));
    // eight entries (the Outfits entry, no Title Screen): KH3's places
    hud.outfits_entry = true;
    assert!(hud.open_camp_menu());
    frames(&mut hud, &s, 60, &[]);
    assert_eq!(hud.camp_menu().unwrap().entries.len(), 8);
    let eight = slot_ys(&hud);
    assert_eq!(eight, (0..9).map(|k| -408.0 + 96.0 * k as f32).collect::<Vec<_>>());
    shoot(&hud, &fonts, &out, "camp_top_8");
    frames(&mut hud, &s, 60, &["menu cancel"]);
    // nine (+ Title Screen): closed up, the ninth at KH3's eighth place, above the bottom band (stage y 870 = 330 here)
    hud.title_entry = true;
    assert!(hud.open_camp_menu());
    frames(&mut hud, &s, 60, &[]);
    let entries = hud.camp_menu().unwrap().entries.clone();
    assert_eq!((entries.len(), entries.last()), (9, Some(&TopEntry::TitleScreen)));
    let nine = slot_ys(&hud);
    assert_eq!(nine, (0..9).map(|k| -408.0 + 80.0 * k as f32).collect::<Vec<_>>());
    assert!(nine[8] <= eight[7] && nine[8] + 540.0 < 870.0 - 60.0, "the ninth row and its help line above the band: {}", nine[8] + 540.0);
    // the cursor reaches the ninth (Title Screen) as before
    for _ in 0..8 {
        frames(&mut hud, &s, 4, &["menu down"]);
    }
    assert_eq!(hud.camp_menu().map(|c| c.focused_entry()), Some(Some(TopEntry::TitleScreen)));
    frames(&mut hud, &s, 30, &[]);
    shoot(&hud, &fonts, &out, "camp_top_9");
    eprintln!("pictures: {}", out.display());
}
