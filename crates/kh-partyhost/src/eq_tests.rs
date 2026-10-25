//! (EQ) A FRIEND'S CAMP-MENU EDITS IN THE RUNTIME AND THE SAVE, AS SORA'S: the ability copy under the cursor (not "the
//! first copies"), the AP budget, his item slots' Auto-Reload / use / refill / re-equip (Sora's pouch rules), a leader's
//! shortcut list with the shared stock's items, and every one of them through a save and a load.

use crate::*;
use kh_core::enums::ETresAbilityKind as A;
use kh_core::host::TargetId;
use kh_core::status::ItemKind;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Sora, Vec3};
use kh_host::party::{CustomEdit, PartyChara, PartyMenuEdit};

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
impl PartyTargets for NoEnemies {
    fn ai_actors(&self, _out: &mut Vec<kh_ai::ActorView>) {}
    fn attacked_by_enemies(&self) -> bool {
        false
    }
}

fn runtime() -> PartyRuntime {
    let mut rt = PartyRuntime::new(RuntimeConfig { wanted: true, test: PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack: None, ai_pack_source: None });
    rt.set_progression(true);
    rt
}

fn sora() -> Sora {
    let mut sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 });
    *sora.status_mut() = kh_core::status::Status::from_loadout(&kh_host::save::new_game_loadout(&kh_core::status::Loadout::default(), None));
    sora.status_mut().magic = ["Fire", "Blizzard", "Thunder", "Cure"].iter().map(|n| kh_core::status::KnownMagic { name: n.to_string(), tier: 1 }).collect();
    kh_core::command::customize::ensure(&mut sora);
    sora.magic_mut().pouch.stock_add(ItemKind::HiPotion, 4);
    sora.magic_mut().pouch.stock_add(ItemKind::Potion, 4);
    sora
}

fn step(rt: &mut PartyRuntime, sora: &mut Sora, world: &TestWorld) {
    let camera = kh_camera::CameraView { position: Vec3::new(-600.0, 0.0, 200.0), rotation: Default::default(), rotation_target: Default::default(), look_point: Vec3::ZERO, distance: 600.0, auto_power: 0.0, stick_wait: 0.0, still_wait: 0.0, reset_wait: 0.0, slope_pitch: 0.0, stick_active: false, followed: false, collided: false };
    let mut targets = NoEnemies;
    let input = Input::default();
    let _ = rt.route(RouteIn { sora, input: &input, dt: 1.0 / 60.0, targets: &targets, camera: &camera, asked_formation: None, asked_ai: Vec::new(), nav: None, now: 0.0 });
    let out = sora.output();
    let mut audio = kh_core::host::audio::NullAudio;
    let mut effects = kh_core::host::effects::NullEffects;
    rt.step(StepIn { dt: 1.0 / 60.0, sora, out: &out, lock_on: TargetId::NONE, camera: &camera, world, targets: &mut targets, audio: &mut audio, effects: &mut effects, nav: None, now: 0.0 });
    let _ = rt.after_step(&NoEnemies, 1.0 / 60.0, 0.0);
}

fn member(rt: &PartyRuntime, c: PartyChara) -> kh_host::party::PartyMenuMember {
    rt.menu_members().into_iter().find(|m| m.chara == c).expect("in the party")
}

/// Each character, the same flows (no names in the code path).
#[test]
fn abilities_take_the_copy_under_the_cursor_within_the_ap_for_each_friend() {
    let world = TestWorld::floor();
    let mut sora = sora();
    let mut rt = runtime();
    for _ in 0..2 {
        step(&mut rt, &mut sora, &world);
    }
    for c in [PartyChara::Donald, PartyChara::Goofy] {
        let id = rt.id(c).unwrap();
        // room in his AP for the test's copies (a level-1 friend's own abilities fill most of it)
        let s = rt.party.status_mut(id).unwrap();
        s.abilities = kh_core::status::AbilitySet::EMPTY;
        s.abilities.set(A::MpHaste, 2, 0);
        assert!(s.max_ap() - s.ap_in_use() >= 3, "{c:?}: AP {} / {}", s.ap_in_use(), s.max_ap());
        // the SECOND copy alone (the user's "glitchy": the host lit the first one)
        rt.apply_edit(&PartyMenuEdit::Ability { chara: c, kind: A::MpHaste, copy: 1, on: true });
        let m = member(&rt, c);
        assert_eq!(m.abilities.iter().find(|a| a.0 == A::MpHaste).map(|a| a.2), Some(0b10), "{c:?}: copy 2 equipped, copy 1 not");
        rt.apply_edit(&PartyMenuEdit::Ability { chara: c, kind: A::MpHaste, copy: 1, on: false });
        assert_eq!(member(&rt, c).abilities.iter().find(|a| a.0 == A::MpHaste).map(|a| a.2), Some(0));
        // the AP budget: copies of Luck Up (5 AP) until his AP is used up; the one over it is refused
        let s = rt.party.status_mut(id).unwrap();
        s.abilities.set(A::LuckUp, 3, 0);
        let room = s.max_ap() - s.ap_in_use();
        let fits = (room / 5).clamp(0, 3) as u8;
        for copy in 0..3u8 {
            rt.apply_edit(&PartyMenuEdit::Ability { chara: c, kind: A::LuckUp, copy, on: true });
        }
        let s = rt.party.status(id).unwrap();
        assert_eq!(s.abilities.equipped(A::LuckUp), fits, "{c:?}: {fits} copies fit in {room} AP");
        assert!(s.ap_in_use() <= s.max_ap(), "{c:?}: never over his AP");
    }
}

#[test]
fn item_slots_follow_soras_pouch_rules_for_each_friend() {
    let world = TestWorld::floor();
    let mut sora = sora();
    let mut rt = runtime();
    for _ in 0..2 {
        step(&mut rt, &mut sora, &world);
    }
    for c in [PartyChara::Donald, PartyChara::Goofy] {
        let id = rt.id(c).unwrap();
        // his slots: Potion, Potion (the menu's equip; Sora's rule: a new item takes the others' setting = off)
        rt.apply_edit(&PartyMenuEdit::ItemSlot { chara: c, slot: 0, item: Some(ItemKind::Potion) });
        rt.apply_edit(&PartyMenuEdit::ItemSlot { chara: c, slot: 1, item: Some(ItemKind::Potion) });
        assert_eq!(member(&rt, c).item_reload[..2], [false, false]);
        // without Auto-Reload a used slot is EMPTY (J 7.1)
        assert!(rt.party.status_mut(id).unwrap().take_item(ItemKind::Potion));
        let m = member(&rt, c);
        assert_eq!((m.items[0], m.item_spent[0]), (None, None), "{c:?}: emptied");
        assert_eq!(rt.party.status(id).unwrap().items[0].item, None);
        // Triangle: Auto-Reload of Potion on every slot holding it
        rt.apply_edit(&PartyMenuEdit::ItemSlot { chara: c, slot: 0, item: Some(ItemKind::Potion) });
        rt.apply_edit(&PartyMenuEdit::AutoReload { chara: c, item: ItemKind::Potion, on: true });
        assert_eq!(member(&rt, c).item_reload[..2], [true, true]);
        // used now: the slot remembers its item ("used, waiting for refill")
        assert!(rt.party.status_mut(id).unwrap().take_item(ItemKind::Potion));
        let m = member(&rt, c);
        assert_eq!((m.items[0], m.item_spent[0], m.item_reload[0]), (None, Some(ItemKind::Potion), true), "{c:?}: greyed with its item");
        // the refill (the HUD's close: the same item written back) keeps the Auto-Reload
        rt.apply_edit(&PartyMenuEdit::ItemSlot { chara: c, slot: 0, item: Some(ItemKind::Potion) });
        let m = member(&rt, c);
        assert_eq!((m.items[0], m.item_spent[0], m.item_reload[0]), (Some(ItemKind::Potion), None, true));
        // a NEW item into a slot takes the setting of the other slots holding it (Ether: none = off)
        rt.apply_edit(&PartyMenuEdit::ItemSlot { chara: c, slot: 1, item: Some(ItemKind::Ether) });
        assert_eq!(member(&rt, c).item_reload[1], false);
        // off while one waits: that slot is emptied (Sora's toggle)
        assert!(rt.party.status_mut(id).unwrap().take_item(ItemKind::Potion));
        rt.apply_edit(&PartyMenuEdit::AutoReload { chara: c, item: ItemKind::Potion, on: false });
        let m = member(&rt, c);
        assert_eq!((m.items[0], m.item_spent[0], m.item_reload[0]), (None, None, false));
    }
}

/// A leader's shortcut list has the shared stock's items as Sora's (Hi-Potion: in the stock, not in his slots), and
/// his edits are his block's, not Sora's.
#[test]
fn a_leaders_shortcuts_take_the_stocks_items_and_stay_his() {
    let world = TestWorld::floor();
    let mut sora = sora();
    let sora_sets = { kh_core::command::customize::ensure(&mut sora); sora.command().shortcuts };
    let mut rt = runtime();
    step(&mut rt, &mut sora, &world);
    let hi_potion = kh_core::magic::items::command_of_item(ItemKind::HiPotion).unwrap();
    for c in [PartyChara::Goofy, PartyChara::Donald] {
        rt.lead_word(&["leader", c.name()]);
        step(&mut rt, &mut sora, &world);
        step(&mut rt, &mut sora, &world);
        assert_eq!(rt.leader_chara(), c);
        assert!(rt.apply_custom_edit(c, CustomEdit::SetShortcut { page: 2, slot: 3, kind: hi_potion }), "{c:?}: the stock's Hi-Potion");
        assert_eq!(member(&rt, c).custom.unwrap().shortcuts[2][3], hi_potion);
        // Goofy has no magic: a spell is refused on his set; Donald's takes Sora's learned Fire
        let fire = kh_core::magic::spells::cmd::FIRE;
        // (DON-UI) the overhaul: Donald's block holds his own ids (his Fire 0x80), Sora's Fire is not his
        let don_fire = kh_core::command::leader::donald::line1(0, 1);
        assert!(!rt.apply_custom_edit(c, CustomEdit::SetShortcut { page: 2, slot: 2, kind: fire }), "{c:?}: Sora's Fire");
        assert_eq!(rt.apply_custom_edit(c, CustomEdit::SetShortcut { page: 2, slot: 2, kind: don_fire }), c == PartyChara::Donald, "{c:?}: his Fire");
    }
    assert_eq!(sora.command().shortcuts, sora_sets, "Sora's sets untouched");
}

/// THE SAVE ROUND TRIP of every flow: the ability copies, the item slots with their Auto-Reload (and a used one),
/// the shortcut sets, an armour piece, into a new runtime through the party record.
#[test]
fn every_friend_edit_survives_a_save_and_a_load() {
    let world = TestWorld::floor();
    let mut sora = sora();
    let mut rt = runtime();
    rt.lead_word(&["leader", "goofy"]);
    for _ in 0..3 {
        step(&mut rt, &mut sora, &world);
    }
    let hi_potion = kh_core::magic::items::command_of_item(ItemKind::HiPotion).unwrap();
    for c in [PartyChara::Donald, PartyChara::Goofy] {
        let id = rt.id(c).unwrap();
        // room in his AP for the test's copies (a level-1 friend's own abilities fill most of it)
        let s = rt.party.status_mut(id).unwrap();
        s.abilities = kh_core::status::AbilitySet::EMPTY;
        s.abilities.set(A::MpHaste, 2, 0);
        assert!(s.max_ap() - s.ap_in_use() >= 3, "{c:?}: AP {} / {}", s.ap_in_use(), s.max_ap());
        rt.apply_edit(&PartyMenuEdit::Ability { chara: c, kind: A::MpHaste, copy: 1, on: true });
        rt.apply_edit(&PartyMenuEdit::ItemSlot { chara: c, slot: 0, item: Some(ItemKind::Potion) });
        rt.apply_edit(&PartyMenuEdit::ItemSlot { chara: c, slot: 1, item: Some(ItemKind::HiPotion) });
        rt.apply_edit(&PartyMenuEdit::AutoReload { chara: c, item: ItemKind::HiPotion, on: true });
        assert!(rt.party.status_mut(id).unwrap().take_item(ItemKind::HiPotion), "a used auto-reload slot");
        rt.apply_edit(&PartyMenuEdit::Armour { chara: c, slot: 0, id: Some("PRT_ITEM01".into()) });
    }
    rt.apply_edit(&PartyMenuEdit::Custom { chara: PartyChara::Goofy, edit: CustomEdit::SetShortcut { page: 1, slot: 0, kind: hi_potion } });
    let before: Vec<_> = rt.menu_members();
    let record = rt.party_record();
    // a new runtime (a reload): the record onto KH3's init data
    let mut rt2 = runtime();
    let mut sora2 = self::sora();
    step(&mut rt2, &mut sora2, &world);
    rt2.reset_friends_for_load(sora2.status().exp);
    rt2.load_party_record(&record);
    for _ in 0..3 {
        step(&mut rt2, &mut sora2, &world);
    }
    for c in [PartyChara::Donald, PartyChara::Goofy] {
        let (a, b) = (before.iter().find(|m| m.chara == c).unwrap(), member(&rt2, c));
        assert_eq!(b.abilities.iter().find(|x| x.0 == A::MpHaste).map(|x| x.2), Some(0b10), "{c:?}: the second copy came back as the second");
        assert_eq!((&b.items, &b.item_spent, &b.item_reload), (&a.items, &a.item_spent, &a.item_reload), "{c:?}: the item slots and their Auto-Reload");
        assert_eq!(b.armour, a.armour, "{c:?}: armour");
        assert_eq!(b.custom.map(|x| x.shortcuts), a.custom.map(|x| x.shortcuts), "{c:?}: the shortcut sets");
    }
    assert_eq!(member(&rt2, PartyChara::Goofy).custom.unwrap().shortcuts[1][0], hi_potion);
}
