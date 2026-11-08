//! (GOOFY, THE USER 2026-11-02: "Can we have a way to visually show in the camp menu what parry techniques Goofy currently
//! has, when to use them and how to perform them? (preferably using symbols from Kingdom Hearts' controller icons)")
//! THE PARRY TECHNIQUES PAGE end to end: kh-party's rows (his level, the sheet's learn levels, the saved ranks) through
//! kh-partyhost's feed into the camp menu (Menu > Abilities > Goofy > Y "Show Next List"), the pad's icons resolved against
//! the pack, and pictures of the page drawn from the CampMenu movie. Skipped without the UI pack.
//! Pictures: build/sora_tmp/party/GOOFY/shots/ui_parry_*.png (KH_UI_SHOTS names another folder).

use std::path::Path;

use kh_party::goofy::{GoofyMove, RankBank};
use kh_partyhost::parry_page;
use kh_ui::camp::{AbilityIn, CampAction, CampKey, CampMenu, CampScreen, StatsIn};
use kh_ui::camp_menu::CampMenuView;
use kh_ui::camp_parry::{ParryPage, HEAD_HOW, HEAD_RANK, HEAD_STAGGER, HEAD_TECHNIQUE, HEAD_WHEN};
use kh_ui::def::write_png;
use kh_ui::menu::{SE_MENU_CLOSE, SE_MENU_OPEN};
use kh_ui::{Canvas, Fonts, Matrix, Pack, Renderer, UiSound};

fn pack() -> Option<Pack> {
    Pack::open_default().map_err(|_| eprintln!("SKIPPED: no build/sora_pack/ui")).ok()
}

fn shots_dir() -> std::path::PathBuf {
    match std::env::var_os("KH_UI_SHOTS") {
        Some(dir) => std::path::PathBuf::from(dir),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/party/GOOFY/shots"),
    }
}

fn bank(steps: &[(GoofyMove, u64)]) -> RankBank {
    let mut parts = [0u64; GoofyMove::ALL.len()];
    for (m, n) in steps {
        if let Some(i) = GoofyMove::ALL.iter().position(|x| x == m) {
            parts[i] = *n;
        }
    }
    RankBank::from_parts(&parts)
}

fn stats(name: &str, level: i32) -> StatsIn {
    StatsIn { name: name.into(), level, hp: 200, hp_max: 200, mp: 100, mp_max: 100, ap_max: 30, munny: 1234, play_seconds: 3725, ..StatsIn::default() }
}

fn goofy_abilities() -> Vec<AbilityIn> {
    ["Danger Sense", "Steady Guard", "Second Deflect"].iter().enumerate().map(|(i, n)| AbilityIn { id: 1_000_000 + i as i32, name: n.to_string(), category: 2, ap: 3, priority: i as i32, owned: 1, equipped: 1, help: format!("{n}.") }).collect()
}

/// The camp menu on Goofy's Abilities screen (party Sora, Donald, Goofy), the host's answer given.
struct Walk {
    view: CampMenuView,
    menu: CampMenu,
    fonts: Fonts,
    sounds: Vec<UiSound>,
}

impl Walk {
    fn new(pack: &mut Pack, page: &ParryPage) -> Walk {
        let fonts = Fonts::load(pack).unwrap();
        let mut view = CampMenuView::load(pack, fonts.clone()).unwrap();
        let mut sounds = Vec::new();
        let mut menu = CampMenu::open(false, false, stats("Sora", 40), &mut sounds);
        menu.set_party(vec![stats("Sora", 40), stats("Donald", 40), stats("Goofy", page.level as i32)]);
        view.open(&menu);
        let mut walk = Walk { view, menu, fonts, sounds };
        walk.frames(45);
        // Menu > Abilities (entry 1) > Goofy (the third member)
        walk.key(CampKey::Down);
        walk.key(CampKey::Decide);
        walk.frames(20);
        walk.key(CampKey::Down);
        walk.key(CampKey::Down);
        let actions = walk.key(CampKey::Decide);
        assert_eq!(actions, vec![CampAction::Opened(CampScreen::Abilities)]);
        assert_eq!(walk.menu.stats.name, "Goofy");
        // the host's answer (kh-host hud.rs refresh_camp): his abilities and his page
        walk.menu.set_abilities(&goofy_abilities(), 30);
        walk.menu.set_parry(Some(page.clone()));
        walk.frames(30);
        walk
    }

    fn frames(&mut self, n: usize) {
        for _ in 0..n {
            self.view.sync(&mut self.menu);
            self.view.tick();
        }
    }

    fn key(&mut self, key: CampKey) -> Vec<CampAction> {
        let actions = self.menu.key(key, false, &mut self.sounds);
        self.frames(2);
        actions
    }

    fn shot(&mut self, name: &str) -> std::path::PathBuf {
        self.frames(40);
        let dir = shots_dir();
        std::fs::create_dir_all(&dir).ok();
        let mut canvas = Canvas::new(1920, 1080);
        let mut renderer = Renderer::new(self.fonts.clone());
        renderer.draw(&mut canvas, &self.view.movie, &Matrix::IDENTITY);
        let mut rgba = Vec::new();
        canvas.to_rgba8_over(|_, y| [0.30 + 0.1 * (y as f32 / 1080.0), 0.38, 0.46], &mut rgba);
        let path = dir.join(format!("ui_parry_{name}.png"));
        write_png(&path, 1920, 1080, &rgba).unwrap();
        println!("picture: {}", path.display());
        path
    }

    fn guide(&self) -> String {
        self.view.movie.text("stageFrame.stageObjR.btnInfo.btnInfoText.txText.textField").unwrap_or_default()
    }

    fn help(&self) -> String {
        self.view.movie.text("ability.helpText.txHelpText.textField").unwrap_or_default()
    }
}

/// The page's rows as the camp menu gets them: eight, learned or locked by his level, the ranks the bosses gave, a saved rank
/// waiting for its learn level.
#[test]
fn the_page_lists_eight_rows_learned_or_locked() {
    let none = RankBank::default();
    for level in [1u32, 5, 15, 26, 40, 99] {
        let p = parry_page::page("Goofy", level, &none);
        assert_eq!(p.rows.len(), 8, "level {level}");
        assert_eq!((p.title.as_str(), p.name.as_str(), p.level), ("Parry Techniques", "Goofy", level));
        for r in &p.rows {
            assert_eq!(r.learned(), level >= r.learn_level, "{} at level {level} (learned at {})", r.name, r.learn_level);
        }
    }
    let learned = |p: &ParryPage| p.rows.iter().filter(|r| r.learned()).map(|r| r.name.clone()).collect::<Vec<String>>();
    // the sheet's levels (THE USER 2026-11-02: all eight by level 40)
    let lv1 = parry_page::page("Goofy", 1, &none);
    assert_eq!(learned(&lv1), ["Deflect"]);
    assert_eq!(learned(&parry_page::page("Goofy", 40, &none)).len(), 8);
    let names: Vec<&str> = lv1.rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["Deflect", "Shield Reflect", "Hop Counter", "Pinning Guard", "Slip Counter", "Bull Brace", "Spell Ward", "Rhythm Deflect"], "the order he learns them");
    for r in &lv1.rows {
        assert_eq!(r.learn_level, kh_party::growth::RankLine::ALL.iter().find(|l| l.label() == r.name).unwrap().learn_level(), "{}: the sheet's level, never written down", r.name);
    }
    // bosses: Deflect at rank 3, Pinning Guard's rank 2 saved before he learns it
    let b = bank(&[(GoofyMove::Deflect, 2), (GoofyMove::PinningGuard, 1)]);
    let pin = GoofyMove::PinningGuard.learn_level();
    let early = parry_page::page("Goofy", pin - 1, &b);
    let row = |p: &ParryPage, n: &str| p.rows.iter().find(|r| r.name == n).cloned().unwrap();
    assert_eq!((row(&early, "Deflect").rank, row(&early, "Pinning Guard").rank, row(&early, "Pinning Guard").saved), (3, 0, 2));
    assert!(row(&early, "Pinning Guard").help().contains("saved Rank 2"));
    let at = parry_page::page("Goofy", pin, &b);
    assert_eq!((row(&at, "Pinning Guard").rank, row(&at, "Pinning Guard").saved), (2, 0), "learned at once at the saved rank");
    // perilous attacks carry the danger mark: thrust, sweep, grab, heavy
    let marked: Vec<&str> = lv1.rows.iter().filter(|r| r.perilous).map(|r| r.name.as_str()).collect();
    assert_eq!(marked, ["Hop Counter", "Pinning Guard", "Slip Counter", "Bull Brace"]);
}

/// (THE USER 2026-11-02: "Remove countering with 'A' after a parry technique ... Redesign the menu for this as well") THE PAGE
/// HAS NO COUNTER COLUMN: no heading, row word, help line, footer or node of it names an A counter; every row has its Stagger
/// (a number and 1 to 3 bars), and the footer leads the break to Goofy Breaker on Y.
#[test]
fn the_page_has_no_counter_column() {
    use kh_ui::camp_parry::{ParryArt, ParryScene, ParryScreen};
    let gone = ["then", "<icon(cross)>", "counter bash", "pin down", "goofy stomp", "pratfall", "return fire", "ward burst", "shield shove", "crescendo", "sekiro"];
    for level in [1u32, 40, 99] {
        let p = parry_page::page("Goofy", level, &bank(&[(GoofyMove::Deflect, 2)]));
        let mut texts: Vec<String> = vec![HEAD_TECHNIQUE.into(), HEAD_RANK.into(), HEAD_WHEN.into(), HEAD_HOW.into(), HEAD_STAGGER.into(), p.footer.clone()];
        for r in &p.rows {
            texts.extend([r.when.clone(), r.how.clone(), r.stagger.clone(), r.detail.clone(), r.help()]);
            assert!(r.stagger.starts_with('+') && r.stagger[1..].chars().next().is_some_and(|c| c.is_ascii_digit()), "{}: a Stagger number: {}", r.name, r.stagger);
            assert!((1..=3).contains(&r.stagger_bars), "{}: {} bars", r.name, r.stagger_bars);
        }
        for t in &texts {
            let lower = t.to_lowercase();
            for g in gone {
                assert!(!lower.contains(g), "level {level}: {g:?} in {t}");
            }
            // "counter" alone may only be part of a technique's name (Hop Counter, Slip Counter)
            let bare = lower.replace("hop counter", "").replace("slip counter", "");
            assert!(!bare.contains("counter"), "level {level}: a counter named in {t}");
        }
        assert!(p.footer.contains("<icon(Triangle)>") && p.footer.contains("Goofy Breaker") && p.footer.contains("0 damage"), "{}", p.footer);
        // the page's nodes: a Stagger number and three bars a row, the footer, nothing named counter
        let mut s = ParryScreen::default();
        s.set(Some(p.clone()));
        s.open(&mut Vec::new());
        let mut scene = ParryScene::new(&ParryArt::default(), &Fonts::default());
        scene.set(&s);
        let names: Vec<String> = scene.nodes().iter().filter_map(|n| n.name.as_deref().map(str::to_string)).collect();
        assert!(names.iter().all(|n| !n.contains("counter")), "{names:?}");
        for i in 0..p.rows.len() {
            assert!(names.contains(&format!("gp_r{i}_stagger")), "row {i}");
            assert!((0..3).all(|k| names.contains(&format!("gp_r{i}_bar{k}"))), "row {i}: its bars");
        }
        assert!(names.iter().any(|n| n == "gp_footer"));
    }
}

/// Every `<icon(...)>` of the page, its headings and its guide is a picture of the pack's `UI/IconText` (none falls back to
/// the placeholder disc), and the inputs use KH3's controller icons.
#[test]
fn the_icon_markup_resolves() {
    let Some(mut pack) = pack() else { return };
    let Some(set) = kh_ui::render::IconSet::load(&mut pack) else {
        eprintln!("SKIPPED: the pack has no icon_text");
        return;
    };
    let p = parry_page::page("Goofy", 40, &RankBank::default());
    let mut texts: Vec<String> = p.rows.iter().flat_map(|r| [r.when.clone(), r.how.clone(), r.stagger.clone(), r.detail.clone(), r.help()]).collect();
    texts.extend([HEAD_STAGGER.to_string(), p.footer.clone(), "<icon(SetAbility)>".into(), "<icon(Ability)>".into()]);
    let mut seen = std::collections::BTreeSet::new();
    for t in &texts {
        for run in kh_ui::render::parse_markup(t) {
            if let kh_ui::render::Run::Icon(name) = run {
                assert!(set.icons.contains_key(&name.to_lowercase()), "<icon({name})> is not in the pack ({t})");
                seen.insert(name);
            }
        }
    }
    for needed in ["Square", "Triangle", "Circle", "LstickUp", "LstickDown", "LstickLeftRight"] {
        assert!(seen.contains(needed), "the page draws {needed}: {seen:?}");
    }
    assert!(!seen.contains("Cross"), "no A counter on the page any more (THE USER 2026-11-02): {seen:?}");
    // the guide's KH3 strings: their icons too
    let fonts = Fonts::load(&mut pack).unwrap();
    let view = CampMenuView::load(&mut pack, fonts).unwrap();
    for key in ["kh3_camp.ability.button_0012", "kh3_camp.ability.button_0015", "kh3_camp.ability.button_0025"] {
        let text = view.string(key);
        assert!(text.contains("Show Next List"), "{key}: {text}");
        for run in kh_ui::render::parse_markup(&text) {
            if let kh_ui::render::Run::Icon(name) = run {
                assert!(set.icons.contains_key(&name.to_lowercase()), "{key}: <icon({name})>");
            }
        }
    }
}

/// Pad first: Menu > Abilities > Goofy, Y shows the page, the D-pad walks it, B (or Y) goes back to his list; the pictures
/// at level 1 (only Deflect) and level 40 (all eight, some ranked by bosses).
#[test]
fn the_page_on_goofys_abilities_screen() {
    let Some(mut pack) = pack() else { return };
    // level 1: Deflect only; a boss has saved Pinning Guard's rank 2 already
    let lv1 = parry_page::page("Goofy", 1, &bank(&[(GoofyMove::PinningGuard, 1)]));
    let mut w = Walk::new(&mut pack, &lv1);
    assert!(w.guide().contains("Show Next List"), "Goofy's list offers the page: {}", w.guide());
    w.shot("abilities_lv1");
    w.sounds.clear();
    w.key(CampKey::Triangle);
    assert!(w.menu.parry.on && w.view.parry_scene().is_some());
    assert_eq!(w.sounds, vec![UiSound::Play(SE_MENU_OPEN)]);
    for path in ["ability.abilityAnm.abilityList", "ability.abilityAnm.selectCharaImage1"] {
        assert!(!w.view.movie.node(path).unwrap().is_visible(), "{path} is under the page");
    }
    let nodes: Vec<String> = w.view.movie.root.extra.iter().filter_map(|n| n.name.as_deref().map(str::to_string)).filter(|n| n.starts_with("gp_")).collect();
    assert!(nodes.iter().any(|n| n == "gp_r0_pip0") && nodes.iter().any(|n| n == "gp_r7_learn"), "{nodes:?}");
    assert!(nodes.iter().any(|n| n == "gp_r3_ready"), "Pinning Guard's saved rank: {nodes:?}");
    assert!(w.help().starts_with("Tap <icon(Square)>"), "the focused row's help: {}", w.help());
    assert!(w.guide().contains("Show Next List") && w.guide().contains("Select Friend"), "KH3's guide of a next list: {}", w.guide());
    w.shot("lv1");
    // down to Pinning Guard: its help says the rank waits
    for _ in 0..3 {
        w.key(CampKey::Down);
    }
    assert_eq!(w.menu.parry.cursor, 3);
    assert!(w.help().contains("Pinning Guard") || w.help().contains("thrust"), "{}", w.help());
    w.shot("lv1_pinning");
    // B: back one level, to his abilities list (the list's clips again)
    w.sounds.clear();
    w.key(CampKey::Cancel);
    assert_eq!((w.menu.screen, w.menu.parry.on), (CampScreen::Abilities, false));
    assert_eq!(w.sounds, vec![UiSound::Play(SE_MENU_CLOSE)]);
    assert!(w.view.parry_scene().is_none() && w.view.movie.node("ability.abilityAnm.abilityList").unwrap().is_visible());
    assert!(!w.view.movie.root.extra.iter().any(|n| n.name.as_deref().is_some_and(|s| s.starts_with("gp_"))));
    // L2 to Donald: no page, Y does nothing
    w.key(CampKey::L2);
    w.menu.set_parry(None); // the host's answer for Donald
    w.frames(2);
    assert_eq!(w.menu.stats.name, "Donald");
    w.sounds.clear();
    w.key(CampKey::Triangle);
    assert!(!w.menu.parry.on && w.sounds.is_empty(), "Donald has no next list");
    assert!(!w.guide().contains("Show Next List"), "{}", w.guide());
    assert!(w.view.movie.misses.is_empty(), "paths not found: {:?}", w.view.movie.misses);

    // level 40: all eight; bosses gave Deflect rank 3, Shield Reflect and Hop Counter rank 2
    let lv40 = parry_page::page("Goofy", 40, &bank(&[(GoofyMove::Deflect, 2), (GoofyMove::ShieldReflect, 1), (GoofyMove::HopCounter, 1)]));
    let mut w = Walk::new(&mut pack, &lv40);
    w.key(CampKey::Triangle);
    assert!(w.menu.parry.on);
    assert_eq!(w.menu.parry.page.as_ref().map(|p| p.rows.iter().filter(|r| r.learned()).count()), Some(8));
    w.shot("lv40");
    // Y again: the next list is his abilities
    w.key(CampKey::Triangle);
    assert!(!w.menu.parry.on && w.view.parry_scene().is_none());
    // the Menu key on the page steps back one level, as on every deeper layer
    w.key(CampKey::Triangle);
    let actions = w.key(CampKey::Menu);
    assert!(actions.is_empty() && !w.menu.parry.on && w.menu.screen == CampScreen::Abilities);
    assert!(w.view.movie.misses.is_empty(), "paths not found: {:?}", w.view.movie.misses);
}
