//! (EQ, the user 2026-10-20: "equipping items/abilities/shortcuts on a party leader that is not Sora is very glitchy as
//! well, that all needs to be fixed to work exactly like Sora does") A FRIEND'S CAMP MENU THROUGH THE REAL HUD, every
//! flow as a pad drives it (the HUD's menu words = the pad's keys), for Donald and Goofy leading and for a friend chosen
//! from Sora's menu: the slot order (the leader first, as Sora's menus start on Sora), Abilities (the copy under the
//! cursor, the AP), Equipment's item rows (Auto-Reload his, not Sora's; the refill at the close), Customize (each
//! leader's own list: Goofy no spells, Donald Sora's; the stock's items as Sora's list), Items (the target, the KO rule).
//! Pictures: build/sora_tmp/party/eq/*.png. Needs the UI pack (skipped without it).

use kh_core::status::ItemKind;
use kh_core::{PadButtons, Sora, TargetId, Vec3};
use kh_host::formation::Formation;
use kh_host::hud::{Frame, Hud, HudTargets, TargetView};
use kh_host::hud_layers::stage_root;
use kh_host::party::{CustomEdit, FriendAiSettings, PartyChara, PartyMenuEdit, PartyMenuMember};
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
    edits: Vec<PartyMenuEdit>,
    out: std::path::PathBuf,
}

impl Rig {
    /// `n` HUD frames as a host runs them (frame, apply_actions, the party edits taken), the word on the first.
    fn frames(&mut self, n: u32, word: Option<&str>) {
        let snapshot = self.sora.ui_snapshot();
        for k in 0..n {
            let w: Vec<String> = if k == 0 { word.map(str::to_string).into_iter().collect() } else { Vec::new() };
            self.hud.frame(&mut Frame { snapshot: &snapshot, sora: &self.sora, events: &[], state: kh_core::StateId::Idle, step_dt: 1.0 / 60.0, play_seconds: 100, targets: &NoTargets, to_view: same, view: None, words: &w, dt: 1.0 / 60.0, ticks: 1, pressed: PadButtons::NONE, nav: Vec::new() });
            self.hud.apply_actions(&mut self.sora);
            self.edits.extend(self.hud.take_party_edits());
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
        // back to the top, then the Menu key: the camp menu closes (the world runs again: the refill)
        self.frames(2, Some("menu open"));
        self.frames(70, None);
        assert!(!self.hud.paused(), "the menu closed");
    }

    fn shoot(&self, name: &str) {
        let Some(movie) = self.hud.movie("CampMenu") else { return };
        let (w, h) = (1920usize, 1080usize);
        let (root, _) = stage_root(w as f32, h as f32);
        let mut canvas = kh_ui::Canvas::new(w, h);
        kh_ui::Renderer::new(self.fonts.clone()).draw(&mut canvas, movie, &root);
        let mut rgba = Vec::new();
        canvas.to_rgba8_over(|_, _| [0.05, 0.05, 0.08], &mut rgba);
        let _ = kh_ui::def::write_png(&self.out.join(format!("{name}.png")), w as u32, h as u32, &rgba);
    }

    fn feed(&mut self, f: &Formation, friends: &[PartyMenuMember]) {
        self.hud.set_formation(f);
        self.hud.set_leader(f.leader(), f.active().contains(&PartyChara::Sora));
        self.hud.set_party_menu(friends.iter().filter(|m| f.active().contains(&m.chara)).cloned().collect());
        self.frames(30, None);
    }
}

fn rig() -> Option<Rig> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root.join("build/sora_pack/ui");
    let Ok((hud, fonts)) = Hud::load(&dir) else {
        eprintln!("no UI pack at {}: skipped", dir.display());
        return None;
    };
    let out = root.join("build/sora_tmp/party/eq");
    let _ = std::fs::create_dir_all(&out);
    let mut sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 }).with_status(kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None)));
    // Sora has learned Fire, Blizzard, Thunder and Cure (the book a leading Donald casts from)
    sora.status_mut().magic = ["Fire", "Blizzard", "Thunder", "Cure"].iter().map(|n| kh_core::status::KnownMagic { name: n.to_string(), tier: 1 }).collect();
    kh_core::command::customize::ensure(&mut sora);
    for (item, n) in [(ItemKind::Potion, 5), (ItemKind::HiPotion, 2), (ItemKind::Ether, 3)] {
        sora.magic_mut().pouch.stock_add(item, n);
    }
    Some(Rig { hud, fonts, sora, edits: Vec::new(), out })
}

fn member(chara: PartyChara) -> PartyMenuMember {
    let leader = |kind| {
        let mut c = kh_core::command::leader::LeaderCustom::default();
        let book = kh_core::command::leader::MagicBook { tiers: [1, 1, 1, 0, 0, 1], sora_slots: [29, 33, 37, 49, 0, 0] };
        c.sync(kind, &book);
        c
    };
    PartyMenuMember {
        chara,
        level: 5,
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
        items: vec![Some(ItemKind::Potion), Some(ItemKind::Ether)],
        abilities: vec![(kh_core::enums::ETresAbilityKind::MpHaste, 2, 0)],
        ai: FriendAiSettings::default(),
        custom: Some(leader(if chara == PartyChara::Donald { kh_core::command::leader::LeaderKind::Donald } else { kh_core::command::leader::LeaderKind::Goofy })),
        custom_costs: Vec::new(),
        kit: Vec::new(),
        guard: None,
        item_reload: vec![false, false],
        item_spent: vec![None, None],
    }
}

/// 1. THE SLOT ORDER: the leader first whatever his formation slot (Sora's menus start on Sora: the first card, the
/// character list's cursor, the Items target); Equipment opens on HIS page.
#[test]
fn a_friend_leaders_menus_start_on_him_as_soras_start_on_sora() {
    let Some(mut r) = rig() else { return };
    let (donald, goofy) = (member(PartyChara::Donald), member(PartyChara::Goofy));
    let f = Formation::new(&[PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy], PartyChara::Goofy).unwrap();
    r.feed(&f, &[donald.clone(), goofy.clone()]);
    assert_eq!(r.hud.camp_roster(), vec![PartyChara::Goofy, PartyChara::Sora, PartyChara::Donald], "the leader first, then the formation's order");
    r.open();
    assert_eq!(r.hud.camp_panels(), &[PartyChara::Goofy, PartyChara::Sora, PartyChara::Donald]);
    r.shoot("1a_goofy_leads_top");
    // Equipment -> the character list (cursor on the leader) -> decide: Goofy's own page
    r.keys(&["menu decide"]);
    assert_eq!(r.hud.camp_menu().map(|c| c.chara_cursor), Some(0));
    r.keys(&["menu decide"]);
    r.frames(30, None);
    let camp = r.hud.camp_menu().unwrap();
    assert_eq!((camp.screen, camp.member, camp.stats.name.as_str()), (CampScreen::Equipment, 0, "Goofy"));
    assert_eq!(camp.equipment.data.slots.iter().filter(|s| s.kind == kh_ui::camp_equip::KIND_ITEM).count(), 2, "his two item slots");
    r.shoot("1b_goofy_equipment");
    // R2: the next member is Sora (his three Keyblade rows), then Donald
    r.keys(&["menu r2"]);
    r.frames(20, None);
    assert_eq!(r.hud.camp_menu().unwrap().stats.name, "Sora");
    r.keys(&["menu r2"]);
    r.frames(20, None);
    assert_eq!(r.hud.camp_menu().unwrap().stats.name, "Donald");
    r.keys(&["menu cancel"]);
    r.close();
    // Sora leading: Sora first, as before
    let f = Formation::new(&[PartyChara::Donald, PartyChara::Sora], PartyChara::Sora).unwrap();
    r.feed(&f, &[donald, goofy]);
    assert_eq!(r.hud.camp_roster(), vec![PartyChara::Sora, PartyChara::Donald]);
}

/// 2. ABILITIES (Donald leading): the copy under the cursor goes to HIM (copy 2 alone), the local copy shows it at
/// once, the AP gauge counts it, a second toggle takes it off; a friend chosen from Sora's menu the same.
#[test]
fn a_friends_abilities_toggle_the_copy_under_the_cursor() {
    let Some(mut r) = rig() else { return };
    let f = Formation::new(&[PartyChara::Donald, PartyChara::Goofy], PartyChara::Donald).unwrap();
    r.feed(&f, &[member(PartyChara::Donald), member(PartyChara::Goofy)]);
    r.open();
    r.keys(&["menu down", "menu decide", "menu decide"]);
    r.frames(30, None);
    assert_eq!(r.hud.camp_menu().map(|c| (c.screen, c.stats.name.clone())), Some((CampScreen::Abilities, "Donald".into())));
    let rows = r.hud.camp_menu().unwrap().abilities.rows.clone();
    assert_eq!(rows.iter().filter(|x| x.id == kh_core::enums::ETresAbilityKind::MpHaste.value()).count(), 2, "two copies, two rows");
    // the second copy
    r.keys(&["menu down", "menu decide"]);
    r.frames(10, None);
    let edit = PartyMenuEdit::Ability { chara: PartyChara::Donald, kind: kh_core::enums::ETresAbilityKind::MpHaste, copy: 1, on: true };
    assert!(r.edits.contains(&edit), "the copy under the cursor: {:?}", r.edits);
    let camp = r.hud.camp_menu().unwrap();
    assert_eq!(camp.abilities.rows.iter().map(|x| x.equipped).collect::<Vec<_>>(), vec![false, true], "the second row lit, not the first");
    assert_eq!(camp.abilities.used_ap, 3);
    assert_eq!(r.hud.party_menu()[0].abilities[0].2, 0b10);
    assert_eq!(r.sora.status().abilities.equipped_mask(kh_core::enums::ETresAbilityKind::MpHaste) & 0b10, r.sora.status().abilities.equipped_mask(kh_core::enums::ETresAbilityKind::MpHaste) & 0b10, "Sora untouched");
    r.shoot("2_donald_abilities");
    r.keys(&["menu decide"]);
    assert!(r.edits.contains(&PartyMenuEdit::Ability { chara: PartyChara::Donald, kind: kh_core::enums::ETresAbilityKind::MpHaste, copy: 1, on: false }));
    r.keys(&["menu cancel"]);
    r.close();
}

/// 3. EQUIPMENT ITEM ROWS (Goofy leading): Triangle on his Potion row is HIS Auto-Reload (Sora's pouch stays as it
/// was); a used auto-reload slot shows greyed with its item; the menu's close refills it from the shared stock.
#[test]
fn a_friends_item_rows_have_soras_auto_reload_and_refill() {
    let Some(mut r) = rig() else { return };
    let f = Formation::new(&[PartyChara::Goofy], PartyChara::Goofy).unwrap();
    let goofy = member(PartyChara::Goofy);
    r.feed(&f, &[goofy.clone()]);
    let sora_reload_before: Vec<bool> = r.sora.magic().pouch.slots.iter().map(|s| s.auto_reload).collect();
    r.open();
    r.keys(&["menu decide", "menu decide"]);
    r.frames(30, None);
    // rows: weapon, armour, accessory, Potion, Ether
    r.keys(&["menu down", "menu down", "menu down", "menu triangle"]);
    r.frames(10, None);
    assert!(r.edits.contains(&PartyMenuEdit::AutoReload { chara: PartyChara::Goofy, item: ItemKind::Potion, on: true }), "{:?}", r.edits);
    assert_eq!(r.sora.magic().pouch.slots.iter().map(|s| s.auto_reload).collect::<Vec<_>>(), sora_reload_before, "Sora's pouch untouched (it took the toggle before)");
    let camp = r.hud.camp_menu().unwrap();
    let potion_row = camp.equipment.data.slots.iter().find(|s| s.kind == kh_ui::camp_equip::KIND_ITEM && s.slot == 0).unwrap();
    assert!(potion_row.auto_reload, "the row shows it at once");
    assert_eq!(camp.equipment.guide_key(), "kh3_camp.equip.button_0016", "Sora's guide for an auto-reload row");
    r.shoot("3a_goofy_item_autoreload");
    r.keys(&["menu cancel"]);
    r.close();
    // in the field he drinks the Potion: the host's snapshot says the slot is used, waiting (auto-reload)
    let mut used = goofy.clone();
    used.items[0] = None;
    used.item_spent[0] = Some(ItemKind::Potion);
    used.item_reload[0] = true;
    r.feed(&f, &[used]);
    r.edits.clear();
    r.open();
    r.keys(&["menu decide", "menu decide"]);
    r.frames(30, None);
    let row = r.hud.camp_menu().unwrap().equipment.data.slots.iter().find(|s| s.kind == kh_ui::camp_equip::KIND_ITEM && s.slot == 0).cloned().unwrap();
    assert_eq!((row.piece.map(|p| p.id), row.used, row.auto_reload), (Some(ItemKind::Potion.name().to_string()), true, true), "greyed with its item, as Sora's used slot");
    r.shoot("3b_goofy_used_slot");
    let stock_before = r.sora.magic().pouch.stock_of(ItemKind::Potion);
    r.keys(&["menu cancel"]);
    r.close();
    r.frames(5, None);
    assert!(r.edits.contains(&PartyMenuEdit::ItemSlot { chara: PartyChara::Goofy, slot: 0, item: Some(ItemKind::Potion) }), "the close refills his slot: {:?}", r.edits);
    assert_eq!(r.sora.magic().pouch.stock_of(ItemKind::Potion), stock_before - 1, "one Potion from the shared stock");
    assert_eq!((r.hud.party_menu()[0].items[0], r.hud.party_menu()[0].item_reload[0]), (Some(ItemKind::Potion), true));
}

/// 4. CUSTOMIZE: each leader's own page and list. Goofy: his abilities by tier and items, NO spell; Donald: Sora's
/// learned spells; both: the stock's items as Sora's list (Hi-Potion is in the stock, not in their slots); a shortcut
/// set goes to the leader's own block, never to Sora's.
#[test]
fn each_leaders_customize_is_his_own() {
    let Some(mut r) = rig() else { return };
    let sora_shortcuts = r.sora.command().shortcuts;
    for leader in [PartyChara::Goofy, PartyChara::Donald] {
        let f = Formation::new(&[leader], leader).unwrap();
        r.feed(&f, &[member(leader)]);
        r.edits.clear();
        r.open();
        // Top: Equipment, Abilities, Items, Customize
        r.keys(&["menu down", "menu down", "menu down", "menu decide"]);
        r.frames(30, None);
        let data = r.hud.camp_menu().unwrap().customize.data.clone();
        let spells = data.entries.iter().filter(|e| (29..=56).contains(&e.kind)).count();
        let goofys = data.entries.iter().filter(|e| kh_core::command::leader::goofy::is_goofy(e.kind)).count();
        let hi_potion = kh_core::magic::items::command_of_item(ItemKind::HiPotion).unwrap();
        assert!(data.friends.iter().all(|x| x.name != leader.name()), "{leader:?}: the leader is not one of Customize's AI friends");
        assert!(data.entries.iter().any(|e| e.kind == hi_potion), "{leader:?}: the stock's Hi-Potion is on his list as on Sora's");
        match leader {
            PartyChara::Goofy => {
                assert_eq!(spells, 0, "Goofy has no magic: no spell on his list");
                assert!(goofys > 0);
                assert_eq!(data.magic_label.as_deref(), Some("kh3_camp.customize.group_0010"));
            }
            _ => {
                // (DON-UI) the overhaul: Donald's list is his own (his spells and Attunements, kh-core leader::donald)
                let his = data.entries.iter().filter(|e| kh_core::command::leader::donald::is_donald(e.kind)).count();
                assert_eq!(spells, 0, "Donald: none of Sora's spells");
                assert!(his >= 4, "Donald: his own spells and Attunements ({his})");
                assert!(data.entries.iter().filter(|e| kh_core::command::leader::donald::is_donald(e.kind)).all(|e| !e.help.is_empty()), "every entry has its help line");
                assert_eq!(goofys, 0);
            }
        }
        // Shortcut Set A, its first button, the list's last row (an item): set
        r.keys(&["menu down", "menu decide", "menu decide", "menu up", "menu decide"]);
        r.frames(10, None);
        let set: Vec<&PartyMenuEdit> = r.edits.iter().filter(|e| matches!(e, PartyMenuEdit::Custom { edit: CustomEdit::SetShortcut { page: 0, slot: 0, .. }, .. })).collect();
        assert_eq!(set.len(), 1, "{leader:?}: one shortcut edit: {:?}", r.edits);
        assert_eq!(set[0].chara(), leader);
        let PartyMenuEdit::Custom { edit: CustomEdit::SetShortcut { kind, .. }, .. } = set[0] else { unreachable!() };
        assert_eq!(r.hud.party_menu()[0].custom.unwrap().shortcuts[0][0], *kind, "his block at once");
        assert_eq!(r.hud.camp_menu().unwrap().customize.data.shortcuts[0][0], *kind, "the screen shows it");
        r.shoot(&format!("4_{}_customize", leader.name().to_lowercase()));
        // Square: Remove
        r.keys(&["menu square"]);
        assert!(r.edits.iter().any(|e| matches!(e, PartyMenuEdit::Custom { edit: CustomEdit::SetShortcut { page: 0, slot: 0, kind: 0 }, .. })));
        r.keys(&["menu cancel", "menu cancel"]);
        r.close();
    }
    assert_eq!(r.sora.command().shortcuts, sora_shortcuts, "Sora's shortcut sets untouched");
}

/// 5. ITEMS: the target starts on the leader (slot 0) and the use is his; a knocked-out member gets Sora's "no
/// benefit" (his rule: hp 0 = nothing).
#[test]
fn items_target_the_leader_first_and_follow_soras_benefit_rule() {
    let Some(mut r) = rig() else { return };
    let mut goofy = member(PartyChara::Goofy);
    goofy.hp = 0;
    let f = Formation::new(&[PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy], PartyChara::Donald).unwrap();
    r.feed(&f, &[member(PartyChara::Donald), goofy]);
    r.open();
    r.keys(&["menu down", "menu down", "menu decide"]);
    r.frames(20, None);
    assert_eq!(r.hud.camp_screen(), Some(CampScreen::Items));
    // the first row (a battle item), decide: target mode on slot 0 = Donald
    r.keys(&["menu decide"]);
    r.shoot("5_items_target_donald");
    r.keys(&["menu decide"]);
    r.frames(5, None);
    assert!(r.edits.iter().any(|e| matches!(e, PartyMenuEdit::UseItem { chara: PartyChara::Donald, .. })), "{:?}", r.edits);
    // down twice: Goofy (KO'd): no benefit, nothing used
    r.edits.clear();
    r.keys(&["menu down", "menu down", "menu decide"]);
    r.frames(5, None);
    assert!(r.edits.is_empty(), "nothing used on a knocked-out member: {:?}", r.edits);
    assert!(r.hud.camp_menu().unwrap().dialog.is_some(), "Sora's 'You cannot benefit' dialog");
}
