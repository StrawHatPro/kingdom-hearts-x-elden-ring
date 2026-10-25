//! (GOOFY-IT) THE ITEM'S PRESENTATION FOR EVERY CHARACTER. The user: "there are no effects or sounds when Goofy or Donald
//! use items". Root cause: the friends' item path (kh-party pawn UseItem -> Timing -> +0.5 s -> actions::item_effect)
//! changed HP / MP only; the item's effect, which carries the item's sound (its SeAttach), was spawned by kh-core for
//! Sora's own items alone (magic/items.rs tick). KH3 plays it for every pawn: ItemEffect 0x1405BE820 -> Recover
//! (vt+0x1878, base 0x1405B57D0 for Sora and the friends alike) -> 0x1405B8C30(pawn+0x440, 4, kind) -> the kind's group
//! of ead_com_abn0 at vfx_abn_center. Now one table (kh-core host/audio.rs `item_recover_kind` + `recover_kind_effect`)
//! serves Sora and every party member, keyed on the item and the member id only (no character list).
//! `cargo test -p kh-party --test it_item_fx -- --nocapture` prints the spawn log per character and item.

use glam::{Vec2, Vec3};
use kh_core::host::audio::{item_heal_effect, item_recover_kind, recover_kind_effect, Audio, SoundHandle, SoundPlay};
use kh_core::host::{Attach, EffectHandle, EffectSpawn, Effects, NullTargets, PartyMember};
use kh_core::input::{PadButton, PadButtons, PadState};
use kh_core::status::ItemKind;
use kh_core::test_world::TestWorld;
use kh_party::pad::PadFrame;
use kh_party::{Chara, CommandStatus, ItemSlot, MemberCommand, MemberId, NpcStateId, Party, PartyConfig, PartyHost, SoraHeal, SoraView, TargetRef};

const DT: f32 = 1.0 / 60.0;

/// The battle items (`item_of_command` 58..=68).
const ITEMS: [ItemKind; 11] = [ItemKind::Potion, ItemKind::HiPotion, ItemKind::MegaPotion, ItemKind::Ether, ItemKind::HiEther, ItemKind::MegaEther, ItemKind::Elixir, ItemKind::Megalixir, ItemKind::Refocuser, ItemKind::HiRefocuser, ItemKind::Panacea];

#[derive(Default)]
struct Rec(Vec<(EffectHandle, String, Attach, u32)>);

impl Effects for Rec {
    fn spawn(&mut self, handle: EffectHandle, spawn: &EffectSpawn) {
        self.0.push((handle, spawn.name.to_string(), spawn.at, spawn.id.0));
    }
    fn stop(&mut self, _handle: EffectHandle, _immediate: bool) {}
}

#[derive(Default)]
struct Ears {
    plays: Vec<String>,
    numbers: Vec<(u8, i32)>,
}

impl Audio for Ears {
    fn play(&mut self, _handle: SoundHandle, play: &SoundPlay) {
        self.plays.push(play.name.to_string());
    }
    fn stop(&mut self, _handle: SoundHandle, _fade_out: f32) {}
    fn party_voice_group_no(&mut self, member: PartyMember, number: i32) {
        self.numbers.push((member.0, number));
    }
}

struct Rig {
    world: TestWorld,
    party: Party,
    effects: Rec,
    ears: Ears,
    sora: SoraView,
    heals: Vec<SoraHeal>,
    states: Vec<(MemberId, NpcStateId)>,
    held: PadButtons,
}

impl Rig {
    fn new() -> Rig {
        Rig { world: TestWorld::floor(), party: Party::new(PartyConfig::test_level99()), effects: Rec::default(), ears: Ears::default(), sora: SoraView::default(), heals: Vec::new(), states: Vec::new(), held: PadButtons::NONE }
    }
    fn step(&mut self) {
        let mut targets = NullTargets;
        let mut host = PartyHost { world: &self.world, targets: &mut targets, audio: &mut self.ears, effects: &mut self.effects };
        let out = self.party.step(DT, &self.sora, &mut host);
        self.heals.extend(out.sora_heals.iter().copied());
        for slot in 0..8u8 {
            if let Some(m) = self.party.member(MemberId(slot)) {
                self.states.push((m.id, m.state));
            }
        }
    }
    fn give(&mut self, id: MemberId, item: ItemKind) {
        let s = self.party.status_mut(id).unwrap();
        s.item_slots = s.item_slots.max(1);
        s.items[0] = ItemSlot { item: Some(item), present: true };
        s.hp = 10;
        s.mp = 0;
    }
    fn pad(&mut self, id: MemberId, held: &[PadButton]) {
        let mut now = PadButtons::NONE;
        for b in held {
            now.set(*b, true);
        }
        let pad = PadState { held: now, pressed: PadButtons(now.0 & !self.held.0), released: PadButtons(self.held.0 & !now.0) };
        self.held = now;
        self.party.set_pad_frame(id, PadFrame { stick: Vec2::ZERO, camera_yaw: 0.0, pad });
        self.step();
    }
    /// The item effects spawned on member `id` since `from`.
    fn on_member(&self, from: usize, id: MemberId) -> Vec<(String, u32)> {
        self.effects.0[from..].iter().filter(|e| e.1.starts_with("vs_itm_") && matches!(e.2, Attach::Party { member, bone: Some("vfx_abn_center"), .. } if member.0 == id.0)).map(|e| (e.1.clone(), e.3)).collect()
    }
}

/// The traced table: ItemEffect's kind and 0x1405B8C30's group for each item; Sora's lookup is the same table.
#[test]
fn it_item_kind_and_group_table() {
    let want: [(ItemKind, u8, &str, u32); 11] = [
        (ItemKind::Potion, 3, "vs_itm_pot0", 3120),
        (ItemKind::HiPotion, 4, "vs_itm_hpo01", 3130),
        (ItemKind::MegaPotion, 4, "vs_itm_hpo01", 3130),
        (ItemKind::Ether, 5, "vs_itm_etr0", 3140),
        (ItemKind::HiEther, 0x12, "vs_itm_het0", 3141),
        (ItemKind::MegaEther, 0x12, "vs_itm_het0", 3141),
        (ItemKind::Elixir, 6, "vs_itm_elx0", 3150),
        (ItemKind::Megalixir, 6, "vs_itm_elx0", 3150),
        (ItemKind::Refocuser, 0x13, "vs_itm_fcr0", 3142),
        (ItemKind::HiRefocuser, 0x14, "vs_itm_hfr0", 3143),
        (ItemKind::Panacea, 0x15, "vs_itm_acu0", 3144),
    ];
    for (item, kind, name, group) in want {
        assert_eq!(item_recover_kind(item as u32), Some(kind), "{item:?}");
        assert_eq!(recover_kind_effect(kind), Some((name, "vfx_abn_center", group)), "{item:?}");
        assert_eq!(item_heal_effect(item as u32), Some(name), "Sora's lookup, {item:?}");
        assert_eq!(kh_party::actions::recover_effect(kind), Some((name, "vfx_abn_center", group)), "the party's lookup, {item:?}");
    }
    assert_eq!(item_recover_kind(ItemKind::Tent as u32), None);
    // the Cure kinds keep D5's effect; kind 1 (an item heal row) none
    assert_eq!(kh_party::actions::recover_effect(14), Some(("vm_cur_bds1", "vfx_abn_center", 1423)));
    assert_eq!(kh_party::actions::recover_effect(1), None);
}

/// A MOCKED FUTURE CHARACTER: the spawn is made from (member id, kind) alone, for a member id no current character has.
#[test]
fn it_future_member_gets_the_same_spawn() {
    for item in ITEMS {
        let kind = kh_party::actions::item_effect_kind(item, true).unwrap();
        let s = kh_party::actions::recover_spawn(PartyMember(7), kind).unwrap();
        assert!(matches!(s.at, Attach::Party { member: PartyMember(7), bone: Some("vfx_abn_center"), .. }), "{item:?}");
        assert_eq!(Some(s.name), item_heal_effect(item as u32));
        println!("future member 7: {:<12} -> kind {kind:#04x} -> {} group {}", format!("{item:?}"), s.name, s.id.0);
    }
    // no focus gauge (the friends): a Refocuser gives nothing, so no effect (ItemEffect returns before Recover)
    assert_eq!(kh_party::actions::item_effect_kind(ItemKind::Refocuser, false), None);
    assert_eq!(kh_party::actions::item_effect_kind(ItemKind::HiRefocuser, false), None);
    assert_eq!(kh_party::actions::item_effect_kind(ItemKind::Potion, false), Some(3));
}

/// Each character (AI command path) x each item on himself: ItemUse clip, the item spent at Timing, the effect 0.5 s
/// later on his own vfx_abn_center, with its group; a member slot no character normally takes (id 5) as well.
#[test]
fn it_every_character_every_item_on_himself() {
    let mut rows = Vec::new();
    for (chara, slot) in [(Chara::Donald, 0u8), (Chara::Goofy, 1), (Chara::Goofy, 5)] {
        for item in ITEMS {
            let mut r = Rig::new();
            let id = r.party.spawn_as(chara, MemberId(slot), Vec3::ZERO, 0.0);
            for _ in 0..3 {
                r.step();
            }
            r.give(id, item);
            let from = r.effects.0.len();
            let t = r.party.command(id, MemberCommand::UseItem { item, target: TargetRef::Member(id) });
            let mut steps = 0;
            let mut used_at = None;
            let mut fx_at = None;
            for k in 0..(4.0 / DT) as usize {
                r.step();
                steps = k;
                if used_at.is_none() && !r.party.status(id).unwrap().items[0].present {
                    used_at = Some(k);
                }
                if fx_at.is_none() && !r.on_member(from, id).is_empty() {
                    fx_at = Some(k);
                }
                if r.party.command_status(id, t) != CommandStatus::Running {
                    break;
                }
            }
            assert_eq!(r.party.command_status(id, t), CommandStatus::Succeeded, "{chara:?} {item:?}");
            assert!(r.states.iter().any(|s| s.0 == id && s.1 == NpcStateId::UseItem), "{chara:?} {item:?}: the UseItem state");
            assert!(used_at.is_some(), "{chara:?} {item:?}: the pouch slot is spent");
            let fx = r.on_member(from, id);
            let want = item_heal_effect(item as u32).unwrap();
            if matches!(item, ItemKind::Refocuser | ItemKind::HiRefocuser) {
                assert!(fx.is_empty(), "{chara:?} {item:?}: no focus gauge, KH3 shows nothing: {fx:?}");
            } else {
                assert_eq!(fx.len(), 1, "{chara:?} {item:?}: one effect: {fx:?}");
                assert_eq!(fx[0].0, want);
                let delay = (fx_at.unwrap() - used_at.unwrap()) as f32 * DT;
                assert!((delay - 0.5).abs() < 0.05, "{chara:?} {item:?}: the effect lands 0.5 s after the spend ({delay})");
            }
            rows.push(format!("{:<6} id {slot} {:<12} spent step {:>3} effect {:?} at step {:?} (state done step {steps})", chara.name(), format!("{item:?}"), used_at.unwrap(), fx.first(), fx_at));
        }
    }
    for row in rows {
        println!("{row}");
    }
}

/// An item on another member lands on HIM (his socket, his seam); on "the player" it goes to the host as a SoraHeal with the
/// item's kind (no HP / MP in that row: the item's own heal row carries them), which `heal_effect_on_sora` plays on Sora.
#[test]
fn it_items_on_others() {
    let mut r = Rig::new();
    let d = r.party.spawn(Chara::Donald, Vec3::new(0.0, 150.0, 0.0), 0.0);
    let g = r.party.spawn(Chara::Goofy, Vec3::new(0.0, -150.0, 0.0), 0.0);
    for _ in 0..3 {
        r.step();
    }
    r.give(d, ItemKind::HiPotion);
    r.party.status_mut(g).unwrap().hp = 10;
    let from = r.effects.0.len();
    let t = r.party.command(d, MemberCommand::UseItem { item: ItemKind::HiPotion, target: TargetRef::Member(g) });
    for _ in 0..(4.0 / DT) as usize {
        r.step();
        if r.party.command_status(d, t) != CommandStatus::Running {
            break;
        }
    }
    for _ in 0..40 {
        r.step();
    }
    assert_eq!(r.on_member(from, g), vec![("vs_itm_hpo01".to_string(), 3130)], "on Goofy");
    assert!(r.on_member(from, d).is_empty(), "not on Donald");
    assert!(r.party.status(g).unwrap().hp > 10, "Goofy healed");
    println!("Donald -> Goofy Hi-Potion: {:?}; voice numbers {:?}", r.on_member(from, g), r.ears.numbers);

    // on the player
    r.give(g, ItemKind::Ether);
    let t = r.party.command(g, MemberCommand::UseItem { item: ItemKind::Ether, target: TargetRef::Sora });
    for _ in 0..(5.0 / DT) as usize {
        r.step();
        if r.party.command_status(g, t) != CommandStatus::Running && r.heals.iter().any(|h| h.effect == 5) {
            break;
        }
    }
    let row = r.heals.iter().find(|h| h.effect == 5).copied().expect("the Ether's effect row for the player");
    assert_eq!((row.amount, row.mp), (0, 0));
    assert!(r.heals.iter().any(|h| h.mp > 0), "the Ether's MP row: {:?}", r.heals);
    let before = r.effects.0.len();
    r.party.heal_effect_on_sora(row.from, row.effect, &mut r.effects);
    let last = r.effects.0[before].clone();
    assert_eq!((last.1.as_str(), last.3), ("vs_itm_etr0", 3140));
    assert!(matches!(last.2, Attach::Sora { bone: Some("vfx_abn_center"), .. }));
    // a friend leading in the player's place: the host plays it on him (runtime: heal_effect_on(leader, kind))
    let before = r.effects.0.len();
    r.party.heal_effect_on(d, row.effect, &mut r.effects);
    assert!(matches!(r.effects.0[before].2, Attach::Party { member, bone: Some("vfx_abn_center"), .. } if member.0 == d.0));
    println!("Goofy -> player Ether: rows {:?}; on Sora {:?}", r.heals, last);
}

/// THE LEADER'S PAD: Items in his command menu (D-pad to Items, A, A) for each character; the same command path, so the
/// same effect on him.
#[test]
fn it_leader_menu_item_for_each_character() {
    for chara in Chara::ALL {
        let mut r = Rig::new();
        let id = r.party.spawn(chara, Vec3::ZERO, 0.0);
        r.party.set_pad(id, Some(kh_core::Tuning::USER));
        for _ in 0..10 {
            r.pad(id, &[]);
        }
        r.give(id, ItemKind::Potion);
        let from = r.effects.0.len();
        for buttons in [&[PadButton::DPadUp][..], &[], &[PadButton::FaceBottom], &[], &[PadButton::FaceBottom], &[]] {
            r.pad(id, buttons);
        }
        for _ in 0..(3.0 / DT) as usize {
            r.pad(id, &[]);
        }
        assert!(r.states.iter().any(|s| s.0 == id && s.1 == NpcStateId::UseItem), "{chara:?}: the menu started UseItem");
        assert!(!r.party.status(id).unwrap().items[0].present, "{chara:?}: the Potion is spent");
        assert_eq!(r.on_member(from, id), vec![("vs_itm_pot0".to_string(), 3120)], "{chara:?}");
        assert!(r.party.status(id).unwrap().hp > 10, "{chara:?} healed");
        println!("{} leading, menu Potion: {:?}, HP {}", chara.name(), r.on_member(from, id), r.party.status(id).unwrap().hp);
    }
}
