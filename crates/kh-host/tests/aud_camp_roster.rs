//! (AUD, the user 2026-10-17: "When starting as non-Sora character, Sora still appears in the pause menu even tho he's
//! not apart of the party. Fix this so only active party members appear") THE CAMP MENU LISTS THE ACTIVE PARTY ONLY,
//! IN FORMATION ORDER, OFFLINE through the HUD (the path of both hosts: the formation + `set_leader(.., sora_in)` +
//! `set_party_menu` feed, then the pause / camp menu). Checked: the top panels (their characters and names), the
//! Items targets, Customize's friends and a friend's Equipment / Abilities going to that friend; the follow-on cases
//! (Sora joins later, a member leaves, the leader changes, a knocked-out member stays, the no-party host = Sora alone).
//! Pictures under build/sora_tmp/party/aud/menu/ (to be looked at). Needs the UI pack (skipped without it).

use kh_core::{PadButtons, Sora, TargetId, Vec3};
use kh_host::formation::Formation;
use kh_host::hud::{Frame, Hud, HudTargets, TargetView};
use kh_host::hud_layers::stage_root;
use kh_host::party::{FriendAiSettings, PartyChara, PartyMenuMember};

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
        hud.frame(&mut Frame { snapshot: &snapshot, sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 125, targets: &NoTargets, to_view: same, view: None, words: &w, dt: 1.0 / 60.0, ticks: 1, pressed: PadButtons::NONE, nav: Vec::new() });
    }
}

fn shoot(hud: &Hud, fonts: &kh_ui::Fonts, out: &std::path::Path, name: &str) {
    let movie = hud.movie("CampMenu").expect("the camp menu's movie");
    let (w, h) = (2560usize, 1440usize);
    let (root, _) = stage_root(w as f32, h as f32);
    let mut canvas = kh_ui::Canvas::new(w, h);
    kh_ui::Renderer::new(fonts.clone()).draw(&mut canvas, movie, &root);
    let mut rgba = Vec::new();
    canvas.to_rgba8_over(|_, _| [0.0, 0.0, 0.0], &mut rgba);
    kh_ui::def::write_png(&out.join(format!("{name}.png")), w as u32, h as u32, &rgba).unwrap();
}

fn sora() -> Sora {
    Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None)))
}

fn member(chara: PartyChara, hp: i32, hp_max: i32) -> PartyMenuMember {
    PartyMenuMember {
        chara,
        level: 1,
        exp: 0,
        exp_level: 0,
        exp_next: 30,
        hp,
        hp_max,
        mp: 100,
        mp_max: 100,
        ap_max: 20,
        ap_in_use: 0,
        strength: 5,
        magic: 5,
        defense: 5,
        resists: [0; 6],
        weapon: if chara == PartyChara::Donald { "WEP_DONALD_00".into() } else { "WEP_GOOFY_00".into() },
        armour: vec![None],
        accessories: vec![None],
        items: vec![None],
        abilities: Vec::new(),
        ai: FriendAiSettings::default(),
        custom: None,
        custom_costs: Vec::new(),
        kit: Vec::new(),
        guard: None, // (GOOFY-GG)
        item_reload: vec![false],    // (EQ)
        item_spent: vec![None],      // (EQ)
    }
}

/// The names on the visible top panels, panel order.
fn panel_names(hud: &Hud) -> Vec<String> {
    let Some(m) = hud.movie("CampMenu") else { return Vec::new() };
    (1..=5)
        .filter(|n| m.node(&format!("topMenu.statusAnm.status.charaWindow0{n}")).is_some_and(|p| p.is_visible()))
        .map(|n| {
            let p = format!("topMenu.statusAnm.status.charaWindow0{n}.charaName");
            m.text(&p).or_else(|| m.text(&format!("{p}.textField"))).unwrap_or_default()
        })
        .collect()
}

/// The host's feed for a formation: `set_formation`, `set_leader(leader, sora_in)`, the active friends' snapshots.
fn feed(hud: &mut Hud, f: &Formation, friends: &[PartyMenuMember]) {
    hud.set_formation(f);
    hud.set_leader(f.leader(), f.active().contains(&PartyChara::Sora));
    let list: Vec<PartyMenuMember> = friends.iter().filter(|m| f.active().contains(&m.chara)).cloned().collect();
    hud.set_party_menu(list);
}

fn open(hud: &mut Hud, sora: &Sora) {
    frames(hud, sora, 2, &["menu open"]);
    frames(hud, sora, 80, &[]);
}

fn close(hud: &mut Hud, sora: &Sora) {
    frames(hud, sora, 2, &["menu open"]);
    frames(hud, sora, 60, &[]);
}

#[test]
fn the_camp_menu_lists_only_the_active_party_in_formation_order() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root.join("build/sora_pack/ui");
    let Ok((mut hud, fonts)) = Hud::load(&dir) else {
        eprintln!("no UI pack at {}: skipped", dir.display());
        return;
    };
    let out = root.join("build/sora_tmp/party/aud/menu");
    let _ = std::fs::create_dir_all(&out);
    let sora = sora();
    let donald = member(PartyChara::Donald, 140, 140);
    let goofy = member(PartyChara::Goofy, 160, 160);
    let all = [donald.clone(), goofy.clone()];

    // 1. THE USER'S CASE: a Goofy start (Elden Ring: formation ["Goofy"], leader Goofy, Sora out)
    let f = Formation::new(&[PartyChara::Goofy], PartyChara::Goofy).unwrap();
    feed(&mut hud, &f, &all);
    frames(&mut hud, &sora, 30, &[]);
    assert_eq!(hud.camp_roster(), vec![PartyChara::Goofy]);
    open(&mut hud, &sora);
    assert_eq!(hud.camp_panels(), &[PartyChara::Goofy], "the panels' characters");
    let names = panel_names(&hud);
    eprintln!("goofy start: panels {names:?}");
    assert!(!names.iter().any(|n| n.contains("Sora")), "Sora is not on a panel: {names:?}");
    assert_eq!(names.len(), 1, "one panel: {names:?}");
    shoot(&hud, &fonts, &out, "a_goofy_start");
    // the camp slot 0 is Goofy's: Equipment shows HIS slots (a friend's sub screen), the Items target list is 1 long
    assert_eq!(hud.camp_member(), 0);
    close(&mut hud, &sora);

    // 2. A DONALD START, then GOOFY JOINS (formation [Donald, Goofy]); Sora still out
    let f = Formation::new(&[PartyChara::Donald, PartyChara::Goofy], PartyChara::Donald).unwrap();
    feed(&mut hud, &f, &all);
    frames(&mut hud, &sora, 30, &[]);
    open(&mut hud, &sora);
    assert_eq!(hud.camp_panels(), &[PartyChara::Donald, PartyChara::Goofy]);
    let names = panel_names(&hud);
    eprintln!("donald + goofy: panels {names:?}");
    assert!(!names.iter().any(|n| n.contains("Sora")), "{names:?}");
    shoot(&hud, &fonts, &out, "b_donald_goofy");
    close(&mut hud, &sora);

    // 3. SORA JOINS LATER (beating Margit): formation [Donald, Goofy, Sora], in that slot order
    let f = Formation::new(&[PartyChara::Donald, PartyChara::Goofy, PartyChara::Sora], PartyChara::Donald).unwrap();
    feed(&mut hud, &f, &all);
    frames(&mut hud, &sora, 30, &[]);
    open(&mut hud, &sora);
    assert_eq!(hud.camp_panels(), &[PartyChara::Donald, PartyChara::Goofy, PartyChara::Sora]);
    eprintln!("sora joined: panels {:?}", panel_names(&hud));
    shoot(&hud, &fonts, &out, "c_sora_joined");
    // 4. WHILE THE MENU IS OPEN a member leaves (Goofy out): the panels follow at once
    let f = Formation::new(&[PartyChara::Donald, PartyChara::Sora], PartyChara::Donald).unwrap();
    feed(&mut hud, &f, &all);
    frames(&mut hud, &sora, 40, &[]);
    assert_eq!(hud.camp_panels(), &[PartyChara::Donald, PartyChara::Sora]);
    // (the party cannot change under an open camp menu in either host: the world is paused and the formation is the save
    // point's own screen; the panels' texts are filled when the top screen opens) close and open: two panels
    close(&mut hud, &sora);
    open(&mut hud, &sora);
    let names = panel_names(&hud);
    eprintln!("goofy left: panels {names:?}");
    assert_eq!(names.len(), 2, "{names:?}");
    assert!(!names.iter().any(|n| n.contains("Goofy")), "{names:?}");
    shoot(&hud, &fonts, &out, "d_goofy_left");
    close(&mut hud, &sora);

    // 5. THE LEADER CHANGES (Sora leads, Donald and Goofy in): the formation's slot order
    let f = Formation::new(&[PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy], PartyChara::Sora).unwrap();
    feed(&mut hud, &f, &all);
    frames(&mut hud, &sora, 30, &[]);
    open(&mut hud, &sora);
    assert_eq!(hud.camp_panels(), &[PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy]);
    shoot(&hud, &fonts, &out, "e_sora_leads");
    close(&mut hud, &sora);

    // 6. A KNOCKED-OUT member stays listed (he is still in the party): Goofy at 0 HP
    let f = Formation::new(&[PartyChara::Goofy, PartyChara::Donald], PartyChara::Goofy).unwrap();
    feed(&mut hud, &f, &[donald.clone(), member(PartyChara::Goofy, 0, 160)]);
    frames(&mut hud, &sora, 30, &[]);
    open(&mut hud, &sora);
    assert_eq!(hud.camp_panels(), &[PartyChara::Goofy, PartyChara::Donald]);
    let names = panel_names(&hud);
    assert!(!names.iter().any(|n| n.contains("Sora")), "{names:?}");
    shoot(&hud, &fonts, &out, "f_goofy_ko");
    close(&mut hud, &sora);

    // 7. A HOST WITHOUT THE PARTY (no snapshot, the default formation): Sora alone, as before
    let (mut plain, _) = Hud::load(&dir).unwrap();
    frames(&mut plain, &sora, 10, &[]);
    assert_eq!(plain.camp_roster(), vec![PartyChara::Sora]);
    open(&mut plain, &sora);
    assert_eq!(plain.camp_panels(), &[PartyChara::Sora]);
    eprintln!("pictures: {}", out.display());
}

/// The routing of the menu's actions by the roster: with Goofy alone, a use of an item on target 0 is Goofy's (a
/// party edit for Goofy), not Sora's.
#[test]
fn an_item_used_on_the_first_slot_of_a_goofy_party_is_goofys() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root.join("build/sora_pack/ui");
    let Ok((mut hud, _fonts)) = Hud::load(&dir) else {
        eprintln!("no UI pack: skipped");
        return;
    };
    let mut sora = sora();
    kh_core::command::customize::ensure(&mut sora);
    sora.magic_mut().pouch.stock_add(kh_core::status::ItemKind::Potion, 3);
    let f = Formation::new(&[PartyChara::Goofy], PartyChara::Goofy).unwrap();
    feed(&mut hud, &f, &[member(PartyChara::Goofy, 60, 160)]);
    frames(&mut hud, &sora, 30, &[]);
    open(&mut hud, &sora);
    // Top: Equipment, Abilities, Items: down twice, decide = Items
    for word in ["menu down", "menu down", "menu decide"] {
        frames(&mut hud, &sora, 20, &[word]);
    }
    assert_eq!(hud.camp_screen(), Some(kh_ui::camp::CampScreen::Items), "the Items screen is up");
    // decide on the potion (the first row), decide on the first (only) target
    frames(&mut hud, &sora, 20, &["menu decide"]);
    frames(&mut hud, &sora, 20, &["menu decide"]);
    hud.apply_actions(&mut sora);
    let edits = hud.take_party_edits();
    eprintln!("edits: {edits:?}");
    assert!(edits.iter().any(|e| matches!(e, kh_host::party::PartyMenuEdit::UseItem { chara: PartyChara::Goofy, .. })), "the potion went to Goofy: {edits:?}");
}
