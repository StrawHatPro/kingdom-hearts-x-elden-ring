//! (GOOFY, THE USER 2026-11-02: "Can we have a way to visually show in the camp menu what parry techniques Goofy currently
//! has, when to use them and how to perform them?") GOOFY'S PARRY TECHNIQUES PAGE THROUGH THE REAL HUD, as a pad drives
//! it (the HUD's menu words = the pad's keys: Y is "menu triangle"): Menu > Abilities > Goofy > Y, while Goofy leads and
//! while he is an AI member; Sora's and Donald's Abilities have no page; the page follows the snapshot the host hands in
//! (his level, his ranks). Pictures: build/sora_tmp/party/GOOFY/shots/hud_parry_*.png. Needs the UI pack (skipped without).

use kh_core::{PadButtons, Sora, TargetId, Vec3};
use kh_host::formation::Formation;
use kh_host::hud::{Frame, Hud, HudTargets, TargetView};
use kh_host::hud_layers::stage_root;
use kh_host::party::{FriendAiSettings, ParryPage, ParryRow, PartyChara, PartyMenuMember};
use kh_party::goofy::{guide, RankBank};
use kh_ui::camp::CampScreen;

struct NoTargets;
impl HudTargets for NoTargets {
    fn target(&self, _id: TargetId) -> Option<TargetView> {
        None
    }
}

fn same(p: Vec3) -> Vec3 {
    p
}

struct Rig {
    hud: Hud,
    fonts: kh_ui::Fonts,
    sora: Sora,
    out: std::path::PathBuf,
}

impl Rig {
    fn frames(&mut self, n: u32, word: Option<&str>) {
        let snapshot = self.sora.ui_snapshot();
        for k in 0..n {
            let w: Vec<String> = if k == 0 { word.map(str::to_string).into_iter().collect() } else { Vec::new() };
            self.hud.frame(&mut Frame { snapshot: &snapshot, sora: &self.sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 100, targets: &NoTargets, to_view: same, view: None, words: &w, dt: 1.0 / 60.0, ticks: 1, pressed: PadButtons::NONE, nav: Vec::new() });
            self.hud.apply_actions(&mut self.sora);
            let _ = self.hud.take_party_edits();
            let _ = self.hud.take_sounds();
        }
    }

    fn keys(&mut self, words: &[&str]) {
        for w in words {
            self.frames(14, Some(w));
        }
    }

    fn open(&mut self) {
        self.frames(2, Some("menu open"));
        self.frames(80, None);
    }

    fn close(&mut self) {
        self.frames(2, Some("menu open"));
        self.frames(70, None);
    }

    fn shoot(&self, name: &str) {
        let Some(movie) = self.hud.movie("CampMenu") else { return };
        let (w, h) = (1920usize, 1080usize);
        let (root, _) = stage_root(w as f32, h as f32);
        let mut canvas = kh_ui::Canvas::new(w, h);
        kh_ui::Renderer::new(self.fonts.clone()).draw(&mut canvas, movie, &root);
        let mut rgba = Vec::new();
        canvas.to_rgba8_over(|_, _| [0.05, 0.05, 0.08], &mut rgba);
        let _ = kh_ui::def::write_png(&self.out.join(format!("hud_parry_{name}.png")), w as u32, h as u32, &rgba);
    }

    fn feed(&mut self, f: &Formation, friends: &[PartyMenuMember]) {
        self.hud.set_formation(f);
        self.hud.set_leader(f.leader(), f.active().contains(&PartyChara::Sora));
        self.hud.set_party_menu(friends.iter().filter(|m| f.active().contains(&m.chara)).cloned().collect());
        self.frames(30, None);
    }

    fn parry_on(&self) -> bool {
        self.hud.camp_menu().is_some_and(|c| c.parry.on)
    }
}

fn rig() -> Option<Rig> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root.join("build/sora_pack/ui");
    let Ok((hud, fonts)) = Hud::load(&dir) else {
        eprintln!("no UI pack at {}: skipped", dir.display());
        return None;
    };
    let out = root.join("build/sora_tmp/party/GOOFY/shots");
    let _ = std::fs::create_dir_all(&out);
    let sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None)));
    Some(Rig { hud, fonts, sora, out })
}

/// Goofy's page as kh-partyhost builds it (kh-party's rows).
fn page(level: u32) -> ParryPage {
    let rows = guide::rows(&RankBank::default(), level).into_iter().map(|r| ParryRow { name: r.name.into(), learn_level: r.learn_level, rank: r.rank, saved: r.saved, perilous: r.perilous, when: r.when.into(), how: r.how.into(), stagger: r.stagger, stagger_bars: r.stagger_bars, detail: r.detail }).collect();
    ParryPage { title: guide::TITLE.into(), name: "Goofy".into(), level, rows, footer: guide::FOOTER.into() }
}

fn member(chara: PartyChara, level: u32) -> PartyMenuMember {
    PartyMenuMember {
        chara,
        level: level as i32,
        exp: 300,
        exp_level: 200,
        exp_next: 400,
        hp: 100,
        hp_max: 140,
        mp: 50,
        mp_max: 100,
        ap_max: 20,
        ap_in_use: 0,
        strength: 8,
        magic: 9,
        defense: 7,
        resists: [0; 6],
        weapon: if chara == PartyChara::Donald { "WEP_DONALD_00".into() } else { "WEP_GOOFY_00".into() },
        armour: vec![None],
        accessories: vec![None],
        items: vec![None],
        abilities: vec![(kh_core::enums::ETresAbilityKind::MpHaste, 1, 0)],
        ai: FriendAiSettings::default(),
        custom: None,
        custom_costs: Vec::new(),
        kit: Vec::new(),
        guard: None,
        item_reload: vec![false],
        item_spent: vec![None],
        parry: (chara == PartyChara::Goofy).then(|| page(level)),
    }
}

/// Goofy as an AI member (Sora leads), then Goofy leading: Abilities > Goofy > Y shows his page, B takes it down; Sora's and
/// Donald's Abilities have none; a new snapshot (a level up) changes the page.
#[test]
fn goofys_abilities_screen_shows_his_parry_techniques() {
    let Some(mut r) = rig() else { return };
    // Sora leads, Donald and Goofy are AI members
    let f = Formation::new(&[PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy], PartyChara::Sora).unwrap();
    r.feed(&f, &[member(PartyChara::Donald, 12), member(PartyChara::Goofy, 12)]);
    r.open();
    // Abilities > Sora: Y does nothing (no next list in the port for him)
    r.keys(&["menu down", "menu decide", "menu decide"]);
    r.frames(20, None);
    assert_eq!(r.hud.camp_menu().map(|c| (c.screen, c.stats.name.clone())), Some((CampScreen::Abilities, "Sora".into())));
    r.keys(&["menu triangle"]);
    assert!(!r.parry_on(), "Sora has no Parry Techniques page");
    // R2 twice: Donald (none), then Goofy
    r.keys(&["menu r2"]);
    r.frames(10, None);
    assert_eq!(r.hud.camp_menu().unwrap().stats.name, "Donald");
    r.keys(&["menu triangle"]);
    assert!(!r.parry_on(), "Donald has none");
    r.keys(&["menu r2"]);
    r.frames(10, None);
    assert_eq!(r.hud.camp_menu().unwrap().stats.name, "Goofy");
    assert!(r.hud.camp_menu().unwrap().parry.available(), "the host fed his page with his abilities");
    r.keys(&["menu triangle"]);
    assert!(r.parry_on(), "Y: his Parry Techniques");
    let p = r.hud.camp_menu().unwrap().parry.page.clone().unwrap();
    assert_eq!(p.rows.len(), 8);
    assert_eq!(p.rows.iter().filter(|x| x.learned()).count(), p.rows.iter().filter(|x| x.learn_level <= 12).count(), "learned = the sheet's levels at his level 12");
    r.keys(&["menu down", "menu down"]);
    assert_eq!(r.hud.camp_menu().unwrap().parry.cursor, 2);
    r.shoot("ai_goofy_lv12");
    // B: back to his abilities list (one level), B again: the character list
    r.keys(&["menu cancel"]);
    assert!(!r.parry_on());
    assert_eq!(r.hud.camp_menu().unwrap().screen, CampScreen::Abilities);
    r.keys(&["menu cancel"]);
    assert_eq!(r.hud.camp_menu().unwrap().screen, CampScreen::Character(kh_ui::camp::TopEntry::Abilities));
    r.keys(&["menu cancel"]);
    r.close();

    // Goofy leads (Sora out): his own slot comes first; level 40 now: all eight
    let f = Formation::new(&[PartyChara::Goofy, PartyChara::Donald], PartyChara::Goofy).unwrap();
    r.feed(&f, &[member(PartyChara::Donald, 40), member(PartyChara::Goofy, 40)]);
    r.open();
    r.keys(&["menu down", "menu decide", "menu decide"]);
    r.frames(20, None);
    assert_eq!(r.hud.camp_menu().unwrap().stats.name, "Goofy");
    r.keys(&["menu triangle"]);
    assert!(r.parry_on());
    let p = r.hud.camp_menu().unwrap().parry.page.clone().unwrap();
    assert!(p.rows.iter().all(|x| x.learned()) && p.level == 40);
    // (THE USER 2026-11-02: "Redesign the menu for this as well") each row's Stagger, the footer to Goofy Breaker, no counter
    assert!(p.rows.iter().all(|x| x.stagger.starts_with('+') && (1..=3).contains(&x.stagger_bars) && !x.detail.contains("<icon(Cross)>")));
    assert!(p.footer.contains("Goofy Breaker"));
    r.shoot("leader_goofy_lv40");
    // L2 to Donald: the page goes, Donald's list shows
    r.keys(&["menu l2"]);
    r.frames(10, None);
    assert!(!r.parry_on());
    assert_eq!(r.hud.camp_menu().unwrap().stats.name, "Donald");
    r.keys(&["menu cancel", "menu cancel"]);
    r.close();
}
