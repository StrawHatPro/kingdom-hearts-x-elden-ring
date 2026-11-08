//! (THE USER 2026-11-02: Roderika's spirit tuning opens a KH3-STYLE SCREEN) The Spirit Tuning screen on the UI pack: the
//! camp menu's Items look with KH3's Keyblade Forge texts, the ashes with their levels, the pane with the ash's picture
//! (Elden Ring's, by its icon number), the materials needed and held, the munny and what the level gives; KH3's question
//! ("Power up the ...?") with Yes / No; B closes it. Pictures: build/sora_tmp/er/glovewort/. Skipped without the UI pack.

use kh_ui::camp::CampKey;
use kh_ui::tuning::{TuneCost, TuneData, TuneEvent, TuneLayer, TuneMenu, TuneRow, TuneView};

fn load() -> Option<(TuneView, kh_ui::Fonts)> {
    let Ok(mut pack) = kh_ui::Pack::open_default() else {
        eprintln!("SKIPPED: no build/sora_pack/ui");
        return None;
    };
    let fonts = kh_ui::Fonts::load(&mut pack).unwrap();
    Some((TuneView::load(&mut pack, fonts.clone()).unwrap(), fonts))
}

fn row(id: u32, icon: u32, name: &str, level: u8, cost: Option<(&str, &str, u32, u32)>, munny: u32, gives: &[&str]) -> TuneRow {
    TuneRow { id, icon: kh_ui::item_art::er_icon_key(icon) /* (THE USER 2026-11-03) the shared Elden Ring icon route */, name: name.into(), level, max_level: 10, cost: cost.map(|(key, n, count, owned)| TuneCost { key: key.into(), name: n.into(), count, owned }), munny, gives: gives.iter().map(|s| s.to_string()).collect() }
}

/// What a Roundtable visit shows (Elden Ring's own numbers of these rows, regulation 2.7.2.0: the costs, the munny, the
/// doping SpEffects 290003 -> 290004 and 290001 -> 290002).
fn data() -> TuneData {
    TuneData {
        title: "Roderika".into(),
        munny: 12_000,
        rows: vec![
            row(200003, 113, "Black Knife Tiche", 3, Some(("MAT_GLOVE_GHOST_04", "Ghost Glovewort [4]", 1, 2)), 5600, &["HP x2.43", "Attack x1.83", "Defense x1.09"]),
            row(214001, 122, "Oracle Envoy Ashes", 1, Some(("MAT_GLOVE_GRAVE_02", "Grave Glovewort [2]", 1, 0)), 1520, &["HP x1.81", "Attack x1.50", "Defense x1.05"]),
            row(201002, 471, "Banished Knight Oleg", 2, Some(("MAT_GLOVE_GHOST_03", "Ghost Glovewort [3]", 1, 1)), 3410, &["HP x2.12", "Attack x1.66", "Defense x1.08"]),
            row(202010, 472, "Banished Knight Engvall", 10, None, 0, &["HP x7.42", "Attack x3.80", "Defense x1.20"]),
        ],
    }
}

fn t(view: &TuneView, path: &str) -> String {
    view.movie.text(path).unwrap_or_default()
}

fn frames(view: &mut TuneView, menu: &mut TuneMenu, n: usize) {
    for _ in 0..n {
        view.sync(menu);
        view.tick();
    }
}

fn shot(view: &TuneView, fonts: &kh_ui::Fonts, name: &str) {
    let mut canvas = kh_ui::Canvas::new(1920, 1080);
    let root = kh_ui::geom::Matrix::new(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
    let mut renderer = kh_ui::Renderer::new(fonts.clone());
    renderer.draw(&mut canvas, &view.movie, &root);
    if let Some(dialog) = view.dialog_movie() {
        renderer.draw(&mut canvas, dialog, &root);
    }
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|_, _| [0.12, 0.13, 0.16], &mut rgba);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/er/glovewort");
    std::fs::create_dir_all(&dir).ok();
    kh_ui::def::write_png(&dir.join(format!("{name}.png")), 1920, 1080, &rgba).unwrap();
}

/// (THE USER 2026-11-02) The screen, its question, an upgrade, a refusal, and B.
#[test]
fn spirit_tuning_screen_lists_the_ashes_asks_and_powers_up() {
    let Some((mut view, fonts)) = load() else { return };
    let mut menu = TuneMenu::new(data());
    view.open(&mut menu);
    frames(&mut view, &mut menu, 60);
    assert!(view.is_open());
    assert_eq!(t(&view, "backyard.stageCategory.category.categoryName.txName"), "SPIRIT TUNING");
    assert_eq!(t(&view, "backyard.backyardAnm.backyardItem.backyardListTitle.backyardListTitleCnt"), "Level");
    assert_eq!(t(&view, "backyard.backyardAnm.backyardItem.backyardList.container.i0.backyardItemName"), "Black Knife Tiche");
    assert_eq!(t(&view, "backyard.backyardAnm.backyardItem.backyardList.container.i0.itemCnt"), "+3");
    assert_eq!(t(&view, "backyard.backyardAnm.backyardItem.backyardList.container.i3.itemCnt"), "MAX");
    assert_eq!(t(&view, "stageFrame.stageObjR.munny.munnyNum.textField"), "12000");
    let pane = t(&view, "backyard.backyardAnm.itemStatusA1.txHelp.textField");
    assert!(pane.contains("Ghost Glovewort [4]") && pane.contains("Materials Needed") && pane.contains("Attack x1.83") && !pane.contains("..."), "the pane: {pane:?}");
    assert!(t(&view, "backyard.helpText.txHelpText.textField").contains("Power up the"), "KH3's Keyblade Forge line");
    // (KH3TEXT) KH3's texts come from the pack by their ids (nothing compiled in): every one is found there
    let texts = view.texts();
    for (id, text) in [(kh_ui::tuning::TuneTexts::MAX, &texts.max), (kh_ui::tuning::TuneTexts::MATERIALS, &texts.materials), (kh_ui::tuning::TuneTexts::MUNNY, &texts.munny), (kh_ui::tuning::TuneTexts::QUESTION, &texts.question), (kh_ui::tuning::TuneTexts::HELP, &texts.help)] {
        assert!(!text.is_empty() && text != id && !text.contains("kh3_"), "{id} is not in the pack: {text:?}");
    }
    assert!(texts.max.contains("spirit ash") && !texts.max.contains("Keyblade"), "{}", texts.max);
    assert!(texts.munny.to_lowercase().contains("munny"), "{}", texts.munny);
    shot(&view, &fonts, "tuning_01_list");
    // A: KH3's question with Yes / No
    let mut s = Vec::new();
    menu.key(CampKey::Decide, false, &mut s);
    assert_eq!(menu.layer, TuneLayer::Confirm);
    frames(&mut view, &mut menu, 30);
    let question = view.dialog_movie().and_then(|d| d.text("dialogText.textField")).unwrap_or_default();
    assert!(question.contains("Power up the") && question.contains("Black Knife Tiche +3"), "the question: {question:?}");
    shot(&view, &fonts, "tuning_02_question");
    menu.key(CampKey::Decide, false, &mut s);
    assert_eq!(menu.take_events(), vec![TuneEvent::Upgrade { id: 200003, to_level: 4 }]);
    frames(&mut view, &mut menu, 30);
    assert_eq!(t(&view, "backyard.backyardAnm.backyardItem.backyardList.container.i0.itemCnt"), "+4");
    shot(&view, &fonts, "tuning_03_after_upgrade");
    // Oracle Envoy: no Grave Glovewort [2]: KH3's refusal line
    menu.key(CampKey::Down, false, &mut s);
    menu.key(CampKey::Decide, false, &mut s);
    frames(&mut view, &mut menu, 10);
    assert_eq!(t(&view, "backyard.helpText.txHelpText.textField"), "You lack the needed materials.");
    shot(&view, &fonts, "tuning_04_refused");
    // B: the screen closes
    menu.key(CampKey::Cancel, false, &mut s);
    assert_eq!(menu.take_events(), vec![TuneEvent::Closed]);
    view.close();
    let mut gone = false;
    for _ in 0..120 {
        if view.tick() {
            gone = true;
            break;
        }
    }
    assert!(gone && !view.is_open());
}
