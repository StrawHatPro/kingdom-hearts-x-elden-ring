//! (EQ) THE USER'S "VERY GLITCHY" FRIEND MENUS, OFFLINE THROUGH THE ELDEN RING PATH (the user, 2026-10-20: "equipping
//! items/abilities/shortcuts on a party leader that is not Sora is very glitchy as well, that all needs to be fixed to
//! work exactly like Sora does"). Frame by frame as the DLL runs it:
//! - sora.rs `pre_physics` -> party/mod.rs `PartySlot::frame`: (EQ) `partyui::menu_sync` every frame, paused or not;
//! - hud.rs `HudHost::frame`: `partyui::hud_frame`, kh-host `Hud::frame` + `apply_actions`, `partyui::hud_after`;
//! - the party's step only while the world runs (a menu of ours holds it), then `partyui::sync`.
//! THE CAUSE (before the fix): the runtime took the camp menu's edits only at the first party step after the menu
//! closed, while `hud_frame` handed the HUD the feed of before the edit EVERY FRAME: the screen put the old values back
//! (an ability lit off again, the old weapon / shortcut shown, the stock moved twice on a retry). Checked here: the
//! screen keeps the edit on every paused frame, the runtime has it at once, the save's record too, and after the close
//! the leader's command menu (the HUD's shortcut panel) shows the new shortcut.

use glam::Vec3;
use kh_core::enums::ETresAbilityKind as A;
use kh_core::status::ItemKind;
use kh_core::{Input, PadButtons, Sora, TargetId, UiSnapshot};
use kh_host::formation::Formation;
use kh_host::hud::{Frame, Hud, HudTargets, TargetView};
use kh_host::party::PartyChara;
use kh_partyhost::PartyRuntime;

use crate::control::Control;
use crate::partyui;

struct NoTargets;
impl HudTargets for NoTargets {
    fn target(&self, _id: TargetId) -> Option<TargetView> {
        None
    }
}

struct NoEnemies;
impl kh_core::Targets for NoEnemies {
    fn enumerate(&self, _out: &mut Vec<kh_core::host::TargetInfo>) {}
    fn info(&self, _id: TargetId) -> Option<kh_core::host::TargetInfo> {
        None
    }
    fn hurt_shapes(&self, _id: TargetId, _out: &mut Vec<kh_core::host::HurtShape>) {}
    fn apply_hit(&mut self, _hit: &kh_core::HitRecord) -> kh_core::host::HitResult {
        kh_core::host::HitResult::default()
    }
}
impl kh_partyhost::PartyTargets for NoEnemies {
    fn ai_actors(&self, _out: &mut Vec<kh_ai::ActorView>) {}
    fn attacked_by_enemies(&self) -> bool {
        false
    }
}

fn same(p: Vec3) -> Vec3 {
    p
}

struct Rig {
    hud: Hud,
    fonts: kh_ui::Fonts,
    sora: Sora,
    rt: PartyRuntime,
    world: kh_core::test_world::TestWorld,
    control: Control,
    t: f64,
}

impl Rig {
    /// One game frame of the Elden Ring host (a menu word = a pad key).
    fn frame(&mut self, word: Option<&str>) {
        // sora.rs pre_physics -> PartySlot::frame: (EQ) the menu's edits in force at once
        partyui::menu_sync(&mut self.rt);
        let led = partyui::hud_frame(&mut self.hud, &UiSnapshot::default(), &self.control);
        let snapshot = led.unwrap_or_else(|| self.sora.ui_snapshot());
        let words: Vec<String> = word.map(str::to_string).into_iter().collect();
        self.hud.frame(&mut Frame { snapshot: &snapshot, sora: &self.sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 100, targets: &NoTargets, to_view: same, view: None, words: &words, dt: 1.0 / 60.0, ticks: 1, pressed: PadButtons::NONE, nav: Vec::new() });
        self.hud.apply_actions(&mut self.sora);
        partyui::hud_after(&mut self.hud, &self.control);
        let _ = self.hud.take_sounds();
        if !self.hud.paused() {
            self.party_step();
        }
    }

    fn party_step(&mut self) {
        let camera = kh_camera::CameraView { position: Vec3::new(-600.0, 0.0, 200.0), rotation: Default::default(), rotation_target: Default::default(), look_point: Vec3::ZERO, distance: 600.0, auto_power: 0.0, stick_wait: 0.0, still_wait: 0.0, reset_wait: 0.0, slope_pitch: 0.0, stick_active: false, followed: false, collided: false };
        let mut targets = NoEnemies;
        let input = Input::default();
        let t = self.t;
        let _ = self.rt.route(kh_partyhost::RouteIn { sora: &mut self.sora, input: &input, dt: 1.0 / 60.0, targets: &targets, camera: &camera, asked_formation: None, asked_ai: Vec::new(), nav: None, now: t });
        let out = self.sora.output();
        let mut audio = kh_core::host::audio::NullAudio;
        let mut effects = kh_core::host::effects::NullEffects;
        self.rt.step(kh_partyhost::StepIn { dt: 1.0 / 60.0, sora: &mut self.sora, out: &out, lock_on: TargetId::NONE, camera: &camera, world: &self.world, targets: &mut targets, audio: &mut audio, effects: &mut effects, nav: None, now: t });
        let _ = self.rt.after_step(&NoEnemies, 1.0 / 60.0, t);
        let _ = partyui::sync(&mut self.rt, &mut self.sora, None, t);
        self.t += 1.0 / 60.0;
    }

    fn idle(&mut self, n: u32) {
        for _ in 0..n {
            self.frame(None);
        }
    }

    fn keys(&mut self, words: &[&str]) {
        for w in words {
            self.frame(Some(w));
            self.idle(13);
        }
    }

    fn shoot(&self, name: &str) {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let out = root.join("build/sora_tmp/party/eq");
        let _ = std::fs::create_dir_all(&out);
        let Some(movie) = self.hud.movie("CampMenu") else { return };
        let (w, h) = (1920usize, 1080usize);
        let (rootm, _) = kh_host::hud_layers::stage_root(w as f32, h as f32);
        let mut canvas = kh_ui::Canvas::new(w, h);
        kh_ui::Renderer::new(self.fonts.clone()).draw(&mut canvas, movie, &rootm);
        let mut rgba = Vec::new();
        canvas.to_rgba8_over(|_, _| [0.05, 0.05, 0.08], &mut rgba);
        let _ = kh_ui::def::write_png(&out.join(format!("{name}.png")), w as u32, h as u32, &rgba);
    }

    fn donald(&self) -> kh_host::party::PartyMenuMember {
        self.hud.party_menu().iter().find(|m| m.chara == PartyChara::Donald).cloned().expect("Donald on the menu")
    }
}

fn rig() -> Option<Rig> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = std::env::var_os("KH_UI_PACK").map(std::path::PathBuf::from).unwrap_or_else(|| root.join("build/sora_pack/ui"));
    let Ok((hud, fonts)) = Hud::load(&dir) else {
        eprintln!("no UI pack at {}: skipped", dir.display());
        return None;
    };
    let mut sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 });
    *sora.status_mut() = kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None));
    sora.status_mut().magic = ["Fire", "Blizzard", "Thunder", "Cure"].iter().map(|n| kh_core::status::KnownMagic { name: n.to_string(), tier: 1 }).collect();
    kh_core::command::customize::ensure(&mut sora);
    sora.magic_mut().pouch.stock_add(ItemKind::Potion, 5);
    sora.magic_mut().pouch.stock_add(ItemKind::HiPotion, 2);
    let mut rt = PartyRuntime::new(kh_partyhost::RuntimeConfig { wanted: true, test: kh_partyhost::PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack: None, ai_pack_source: None });
    partyui::configure(&mut rt);
    let control = Control::new(std::env::temp_dir().join("kh-er-eq-test-no-such.ctl"));
    Some(Rig { hud, fonts, sora, rt, world: kh_core::test_world::TestWorld::floor(), control, t: 0.0 })
}

#[test]
fn a_friend_leaders_menu_edits_stick_while_paused_and_reach_the_field() {
    let _serial = partyui::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    partyui::reset();
    let Some(mut r) = rig() else { return };
    r.idle(3);
    // Donald leads (Sora behind him in the formation: the menus still start on Donald)
    partyui::ask_formation(Formation::new(&[PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy], PartyChara::Donald).unwrap());
    r.idle(6);
    assert_eq!(r.rt.leader_chara(), PartyChara::Donald);
    // two MP Haste copies and room in his AP (a level-1 Donald's own abilities fill most of it)
    let d = r.rt.id(PartyChara::Donald).unwrap();
    {
        let s = r.rt.party.status_mut(d).unwrap();
        s.abilities = kh_core::status::AbilitySet::EMPTY;
        s.abilities.set(A::MpHaste, 2, 0);
        s.items[0] = kh_party::ItemSlot { item: Some(ItemKind::Potion), present: true };
    }
    r.hud.inventory_mut().add("PRT_ITEM01", 1);
    r.idle(4);
    assert_eq!(r.hud.camp_roster()[0], PartyChara::Donald, "slot 0 is the leader");

    // ---- ABILITIES: copy 2 of MP Haste
    r.keys(&["menu open"]);
    r.idle(60);
    assert!(r.hud.paused(), "the camp menu holds the world");
    r.keys(&["menu down", "menu decide", "menu decide"]);
    r.idle(20);
    assert_eq!(r.hud.camp_menu().unwrap().stats.name, "Donald");
    // the row of MP Haste's second copy (Zero EXP, the leader's, is on his page too)
    let rows = r.hud.camp_menu().unwrap().abilities.rows.clone();
    let at = rows.iter().position(|x| x.id == A::MpHaste.value() && x.copy == 1).expect("copy 2's row");
    for _ in 0..at {
        r.keys(&["menu down"]);
    }
    r.keys(&["menu decide"]);
    let haste = |r: &Rig| r.hud.camp_menu().unwrap().abilities.rows.iter().filter(|x| x.id == A::MpHaste.value()).map(|x| x.equipped).collect::<Vec<_>>();
    for k in 0..90 {
        r.frame(None);
        assert_eq!(r.donald().abilities.iter().find(|a| a.0 == A::MpHaste).map(|a| a.2), Some(0b10), "paused frame {k}: the screen keeps copy 2 (it reverted every frame before)");
        assert_eq!(haste(&r), vec![false, true]);
    }
    assert!(r.hud.paused());
    assert_eq!(r.rt.party.status(d).unwrap().abilities.equipped_mask(A::MpHaste), 0b10, "the runtime has it while the menu is open");
    r.shoot("6a_er_donald_abilities_paused");
    // (DON-UI) B is one level (the user's rule for all menus): the character list, then the top screen
    r.keys(&["menu cancel", "menu cancel"]);

    // ---- EQUIPMENT: armour from the stock, Auto-Reload on his Potion row
    r.keys(&["menu up", "menu decide", "menu decide"]);
    r.idle(20);
    // rows: weapon, armour, accessories (2 for Donald), items
    r.keys(&["menu down", "menu decide"]);
    r.keys(&["menu decide"]);
    r.idle(30);
    assert_eq!(r.donald().armour.first().cloned().flatten().as_deref(), Some("PRT_ITEM01"), "his armour slot");
    assert_eq!(r.hud.inventory().equipment.iter().find(|e| e.0 == "PRT_ITEM01").map_or(0, |e| e.1), 0, "the piece left the stock once (no second move on a retry)");
    let def_now = r.rt.party.status(d).unwrap().defense();
    assert_eq!(r.donald().defense, def_now, "the screen's Defense is the runtime's own answer, while paused");
    let items_row = r.hud.camp_menu().unwrap().equipment.data.slots.iter().position(|s| s.kind == kh_ui::camp_equip::KIND_ITEM).unwrap();
    for _ in 1..items_row {
        r.keys(&["menu down"]);
    }
    r.keys(&["menu triangle"]);
    r.idle(30);
    assert!(r.rt.party.status(d).unwrap().item_reload[0], "his Potion slot's Auto-Reload, in force at once");
    assert!(r.donald().item_reload[0]);
    r.shoot("6b_er_donald_equipment_paused");
    r.keys(&["menu cancel", "menu cancel"]); // (DON-UI) the character list, then the top screen

    // ---- CUSTOMIZE: Shortcut Set A's first button = the list's first entry
    r.keys(&["menu down", "menu down", "menu down", "menu decide"]);
    r.idle(20);
    r.keys(&["menu down", "menu decide", "menu decide", "menu down", "menu decide"]);
    r.idle(30);
    let kind = r.hud.camp_menu().unwrap().customize.data.shortcuts[0][0];
    assert_ne!(kind, 0);
    assert_eq!(r.rt.menu_members().iter().find(|m| m.chara == PartyChara::Donald).unwrap().custom.unwrap().shortcuts[0][0], kind, "his block in the runtime while paused");
    r.shoot("6c_er_donald_customize_paused");
    // the save made right now has every edit
    let record = partyui::party_record(&r.control).expect("the record");
    let friends = record["friends"].as_array().unwrap();
    let dr = friends.iter().find(|f| f["chara"] == "Donald").unwrap();
    assert_eq!(dr["armour"][0], "PRT_ITEM01");
    assert_eq!(dr["reload"][0], true);
    assert_eq!(record["custom"]["donald"]["shortcuts"][0][0], kind);

    // ---- CLOSE: the world runs; the leader's command menu (the HUD's shortcut panel) shows the new shortcut
    r.keys(&["menu cancel", "menu cancel"]);
    r.keys(&["menu open"]);
    r.idle(80);
    assert!(!r.hud.paused(), "closed");
    let led = partyui::feed(|f| f.led.clone()).flatten().expect("Donald's command menu");
    assert_eq!(led.command.shortcut[0].kind, kind, "LB panel's first button = the shortcut set in the menu");
    assert_eq!(r.rt.party.status(d).unwrap().abilities.equipped_mask(A::MpHaste), 0b10, "and it stays");
    partyui::reset();
}
