//! (THE USER 2026-11-02: "Re-add Grave Gloveworts and Ghost Gloveworts ... make them work completely with our KH3 systems.
//! Treat them as upgrade materials and have them appear alongside upgrade materials in the camp menu's item menus") THE
//! GLOVEWORTS THROUGH THE HUD, OFFLINE (the path of both hosts): a pickup through the one door (`kh_host::items::give`, as
//! the Elden Ring gate and the prize boxes hand items over) shows KH3's item popup with the composed picture and goes into
//! the camp menu's stock; the camp menu's Items screen lists them on the Materials tab right after KH3's ores, before the
//! crystals, whatever order they came in, with their own help text; the save keeps them. Needs the UI pack (skipped
//! without it). Pictures: build/sora_tmp/er/glovewort/*.png (to be looked at).

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

fn shoot(hud: &Hud, fonts: &kh_ui::Fonts, out: &std::path::Path, movie: &str, name: &str) {
    let movie = hud.movie(movie).expect("the movie");
    let (w, h) = (1920, 1080);
    let (root, _) = stage_root(w as f32, h as f32);
    let mut canvas = kh_ui::Canvas::new(w, h);
    kh_ui::Renderer::new(fonts.clone()).draw(&mut canvas, movie, &root);
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|_, y| [0.30 + 0.1 * (y as f32 / 1080.0), 0.38, 0.46], &mut rgba);
    kh_ui::def::write_png(&out.join(format!("{name}.png")), w as u32, h as u32, &rgba).unwrap();
}

fn sora() -> Sora {
    Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None)))
}

#[test]
fn gloveworts_are_upgrade_materials_on_the_items_screen() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let Ok((mut hud, fonts)) = Hud::load(&root.join("build/sora_pack/ui")) else {
        eprintln!("no UI pack: skipped");
        return;
    };
    let out = root.join("build/sora_tmp/er/glovewort");
    let _ = std::fs::create_dir_all(&out);
    let sora = sora();
    let id = |key: &str| kh_host::items::by_key(key).unwrap().item_id;
    // the pickups, in an order that is not the list's: a Ghost first, KH3 materials and ores in between
    for (key, n) in [("MAT_GLOVE_GHOST_03", 2), ("MAT_ITEM60", 1), ("MAT_GLOVE_GRAVE_01", 3), ("MAT_ITEM01", 4), ("MAT_ITEM53", 1), ("MAT_GLOVE_GRAVE_GREAT", 1), ("MAT_ITEM58", 2)] {
        assert!(kh_host::items::give(id(key), n), "{key}");
    }
    frames(&mut hud, &sora, 20, &[]);
    let obtained = hud.take_obtained();
    assert!(obtained.contains(&(id("MAT_GLOVE_GHOST_03"), 2)) && obtained.contains(&(id("MAT_GLOVE_GRAVE_GREAT"), 1)), "taken into the stock: {obtained:?}");
    let popup = hud.movie("HudLeft").and_then(|m| m.text("hudleft.getItemS.txItemNameS.coItemNameSTxt")).unwrap_or_default();
    assert_eq!(popup, "Ghost Glovewort [3]", "KH3's item popup names the first one (the others queue behind it)");
    shoot(&hud, &fonts, &out, "HudLeft", "items_01_obtained_popup");
    // the camp menu: Items, the Materials tab
    frames(&mut hud, &sora, 400, &[]);
    assert!(hud.open_camp_menu());
    frames(&mut hud, &sora, 60, &[]);
    let at = hud.camp_menu().map(|c| c.entries.iter().position(|e| *e == TopEntry::Items).unwrap()).unwrap();
    for _ in 0..at {
        frames(&mut hud, &sora, 4, &["menu down"]);
    }
    frames(&mut hud, &sora, 40, &["menu decide"]);
    assert_eq!(hud.camp_screen(), Some(CampScreen::Items));
    for _ in 0..5 {
        frames(&mut hud, &sora, 12, &["menu r2"]);
    }
    let menu = hud.camp_menu().unwrap();
    assert_eq!(kh_ui::camp_equip::ITEM_TABS[menu.items.tab].1, Some(kh_ui::camp_equip::CAT_MATERIAL), "the Materials tab");
    let rows: Vec<String> = menu.items.rows().iter().map(|e| e.name.clone()).collect();
    assert_eq!(rows, ["Blazing Shard", "Fluorite", "Electrum", "Grave Glovewort [1]", "Great Grave Glovewort", "Ghost Glovewort [3]", "Illusory Crystal"], "KH3's order, the gloveworts right after the ores");
    let glove = menu.items.rows().into_iter().find(|e| e.id == "MAT_GLOVE_GRAVE_01").unwrap();
    assert!(glove.help.contains("Tunes spirit ashes to +1.") && glove.help.contains('\n') && glove.count == 3, "{glove:?}");
    // the focus on Grave Glovewort [1]: its pane shows the composed picture and the text
    for _ in 0..3 {
        frames(&mut hud, &sora, 6, &["menu down"]);
    }
    frames(&mut hud, &sora, 40, &[]);
    shoot(&hud, &fonts, &out, "CampMenu", "items_02_materials_tab");
    // (THE USER 2026-11-03) each glovewort's text fits the pane whole, its tag included (no "..." cut), Elden Ring's icon above it
    for name in ["Grave Glovewort [1]", "Great Grave Glovewort", "Ghost Glovewort [3]"] {
        let pane = hud.movie("CampMenu").and_then(|m| m.text("backyard.backyardAnm.itemStatusA1.txHelp.textField")).unwrap_or_default();
        assert!(pane.contains("material)") && !pane.contains("..."), "{name}: the pane reads {pane:?}");
        frames(&mut hud, &sora, 20, &["menu down"]);
    }
    // the All tab (R2 twice: past Key Items, wrapping): the same materials in the same order
    frames(&mut hud, &sora, 12, &["menu r2"]);
    frames(&mut hud, &sora, 12, &["menu r2"]);
    let all: Vec<String> = hud.camp_menu().unwrap().items.rows().iter().map(|e| e.name.clone()).collect();
    assert_eq!(all, rows, "the All tab");
    shoot(&hud, &fonts, &out, "CampMenu", "items_03_all_tab");
    eprintln!("pictures: {}", out.display());
}
