//! (NOCOMBAT) The rule itself: every fighter, every act, on and off; the menu model; the pad gate; the state queue's word.

use super::*;
use crate::command::{CommandRow, CommandSnapshot, MenuState, SituationRow};
use crate::input::{PadButtons, PadState};
use crate::magic::items;
use crate::status::ItemKind;

/// The position of a fighter in [`Fighter::ALL`], by an exhaustive match: a new fighter does not compile until it is
/// given a place, and `every_fighter_is_listed_once` fails until it is in `ALL`.
fn index_of(f: Fighter) -> usize {
    match f {
        Fighter::Sora => 0,
        Fighter::Donald => 1,
        Fighter::Goofy => 2,
        Fighter::MimicSora => 3,
        Fighter::SpiritAsh => 4,
    }
}

#[test]
fn nocombat_every_fighter_is_listed_once() {
    for (i, f) in Fighter::ALL.iter().enumerate() {
        assert_eq!(index_of(*f), i, "{} out of place in Fighter::ALL", f.name());
    }
}

#[test]
fn nocombat_every_fighter_every_act_on_and_off() {
    for on in [false, true] {
        let _zone = scope(on);
        assert_eq!(zone(), on);
        for who in Fighter::ALL {
            for act in Act::ALL {
                let expect = !(on && act.offensive());
                assert_eq!(allows(who, act), expect, "{} / {act:?} with the zone {on}", who.name());
            }
            assert_eq!(may_stay(who), !on || who.stays(), "{}", who.name());
        }
        for act in Act::ALL {
            assert_eq!(world_allows(act), !(on && act.offensive()), "{act:?}");
        }
    }
}

#[test]
fn nocombat_what_stays_and_what_goes() {
    // the brief's lists: these never go, these always go
    for act in [Act::Item, Act::Move, Act::Jump, Act::Dodge, Act::Guard, Act::Talk, Act::Camp, Act::WeaponSwitch] {
        assert!(!act.offensive(), "{act:?} stays");
    }
    for act in [Act::Attack, Act::Magic, Act::Summon, Act::Shotlock, Act::Situation, Act::FormChange, Act::Finisher, Act::TeamAttack, Act::Rage, Act::Xs, Act::GuardAttack, Act::Breaker, Act::AiAttack, Act::Hit] {
        assert!(act.offensive(), "{act:?} goes");
    }
    // the party stays (pacified), the summons are sent away
    assert!(Fighter::Sora.stays() && Fighter::Donald.stays() && Fighter::Goofy.stays());
    assert!(!Fighter::MimicSora.stays() && !Fighter::SpiritAsh.stays());
}

#[test]
fn nocombat_command_kinds_and_situation_kinds() {
    let potion = items::command_of_item(ItemKind::Potion).expect("Potion has a command");
    assert_eq!(act_of_kind(potion), Act::Item);
    assert_eq!(act_of_kind(ITEMS_MAIN), Act::Item);
    assert_eq!(act_of_kind(crate::magic::spells::cmd::ATTACK), Act::Attack);
    assert_eq!(act_of_kind(summon::KIND_SUMMON), Act::Summon);
    assert_eq!(act_of_kind(summon::KIND_UNSUMMON), Act::Summon);
    assert_eq!(act_of_kind(28), Act::Magic);
    assert_eq!(act_of_situation(0x80), Act::FormChange);
    assert_eq!(act_of_situation(0x9C), Act::Finisher);
    assert_eq!(act_of_situation(179), Act::Rage);
    assert_eq!(act_of_situation(193), Act::TeamAttack);
    assert_eq!(act_of_situation(248), Act::Xs);
    assert_eq!(act_of_situation(110), Act::Magic);
    assert_eq!(act_of_situation(3), Act::GuardAttack);
    // every kind a situation can carry is offensive
    for kind in 0..=255 {
        assert!(act_of_situation(kind).offensive(), "situation kind {kind}");
    }
}

fn row(kind: u8, name: &'static str) -> CommandRow {
    CommandRow { kind, name, enabled: true, ..CommandRow::default() }
}

/// A menu like any character's: Attack, Magic, Items, Summon; a spell and an item on the panel; a Y row.
fn menu() -> CommandSnapshot {
    let potion = items::command_of_item(ItemKind::Potion).unwrap_or(0);
    CommandSnapshot {
        state: MenuState::MainSel,
        main: [row(crate::magic::spells::cmd::ATTACK, "Attack"), row(28, "Magic"), row(ITEMS_MAIN, "Items"), row(0, "Summon")],
        lists: [Vec::new(), vec![row(29, "Fire")], vec![row(potion, "Potion")], vec![row(summon::KIND_SUMMON, "Summon")]],
        shortcut: [row(29, "Fire"), CommandRow::default(), row(potion, "Potion"), row(30, "Blizzard")],
        situations: vec![SituationRow { serial: 1, kind: 0x80, name: "Second Form", icon_type: 1, enabled: true, ..SituationRow::default() }],
        situation_selected: Some(0),
        ..CommandSnapshot::default()
    }
}

#[test]
fn nocombat_grey_leaves_items_only() {
    let mut off = menu();
    {
        let _out = scope(false);
        grey(&mut off);
    }
    assert_eq!(off, menu(), "outside the zone nothing changes");
    let _zone = scope(true);
    let mut on = menu();
    grey(&mut on);
    assert_eq!(on.main.map(|r| r.enabled), [false, false, true, false], "Attack, Magic, Summon greyed; Items lit");
    assert!(!on.lists[1][0].enabled && on.lists[2][0].enabled && !on.lists[3][0].enabled);
    assert_eq!(on.shortcut.map(|r| r.enabled), [false, false, true, false], "only the Potion on the panel");
    assert!(on.situations.iter().all(|s| !s.enabled), "the Y list greys");
    // an item row already greyed (none left) stays greyed
    let mut empty = menu();
    empty.lists[2][0].enabled = false;
    grey(&mut empty);
    assert!(!empty.lists[2][0].enabled);
}

fn pad(pressed: &[PadButton], held: &[PadButton]) -> PadState {
    let set = |bs: &[PadButton]| bs.iter().fold(PadButtons::NONE, |a, b| a.with(*b));
    PadState { held: PadButtons(set(held).0 | set(pressed).0), pressed: set(pressed), released: PadButtons::NONE }
}

#[test]
fn nocombat_pad_gate_each_route() {
    let m = menu();
    // outside the zone: untouched
    {
        let _out = scope(false);
        let mut p = pad(&[PadButton::ATTACK, PadButton::SITUATION], &[PadButton::RightShoulder]);
        let before = p;
        assert!(!gate_pad(&m, &mut p, true));
        assert_eq!(p, before);
    }
    let _zone = scope(true);
    // A on Attack
    let mut p = pad(&[PadButton::ATTACK], &[]);
    assert!(gate_pad(&m, &mut p, false) && !p.pressed.has(PadButton::ATTACK), "A on Attack: taken, refused");
    // A / D-pad right on Magic
    let magic = CommandSnapshot { main_cursor: 1, ..menu() };
    let mut p = pad(&[PadButton::ATTACK], &[]);
    assert!(gate_pad(&magic, &mut p, false) && !p.pressed.has(PadButton::ATTACK));
    let mut p = pad(&[PadButton::DPadRight], &[]);
    assert!(gate_pad(&magic, &mut p, false) && !p.pressed.has(PadButton::DPadRight), "D-pad right would open Magic");
    // A / D-pad right on Summon (the row of kind 0 with a name)
    let sum = CommandSnapshot { main_cursor: 3, ..menu() };
    let mut p = pad(&[PadButton::DPadRight], &[]);
    assert!(gate_pad(&sum, &mut p, false), "Summon refused");
    // A on Items, A in the Items list: kept
    let items_row = CommandSnapshot { main_cursor: 2, ..menu() };
    let mut p = pad(&[PadButton::ATTACK], &[]);
    assert!(!gate_pad(&items_row, &mut p, false) && p.pressed.has(PadButton::ATTACK), "Items opens");
    let in_items = CommandSnapshot { state: MenuState::SubSel, list: 2, row_cursor: 0, ..menu() };
    let mut p = pad(&[PadButton::ATTACK], &[]);
    assert!(!gate_pad(&in_items, &mut p, false) && p.pressed.has(PadButton::ATTACK), "the Potion is used");
    let in_magic = CommandSnapshot { state: MenuState::SubSel, list: 1, row_cursor: 0, ..menu() };
    let mut p = pad(&[PadButton::ATTACK], &[]);
    assert!(gate_pad(&in_magic, &mut p, false), "a spell row (a list left open on arrival) is refused");
    // D-pad right on Attack is the Keyblade switch: kept
    let mut p = pad(&[PadButton::DPadRight], &[]);
    assert!(!gate_pad(&m, &mut p, false) && p.pressed.has(PadButton::DPadRight));
    // LB + a face button: the spell refused, the Potion kept, the empty slot taken silently
    let mut p = pad(&[PadButton::FaceRight], &[PadButton::LeftShoulder]);
    assert!(gate_pad(&m, &mut p, false) && !p.pressed.has(PadButton::FaceRight), "LB + B: Fire refused");
    let mut p = pad(&[PadButton::FaceLeft], &[PadButton::LeftShoulder]);
    assert!(!gate_pad(&m, &mut p, false) && p.pressed.has(PadButton::FaceLeft), "LB + X: the Potion");
    let mut p = pad(&[PadButton::FaceTop], &[PadButton::LeftShoulder]);
    assert!(!gate_pad(&m, &mut p, false) && !p.pressed.has(PadButton::FaceTop), "LB + Y on an empty slot: taken, silent");
    // Y: taken; refused only when something was on the Y list
    let mut p = pad(&[PadButton::SITUATION], &[]);
    assert!(gate_pad(&m, &mut p, true) && !p.pressed.has(PadButton::SITUATION));
    let mut p = pad(&[PadButton::SITUATION], &[]);
    assert!(!gate_pad(&m, &mut p, false) && !p.pressed.has(PadButton::SITUATION), "Goofy Breaker / an empty Y list: taken, silent");
    // RB / RT held: the shotlock aim, Spell Volley, the gun aim
    let mut p = pad(&[], &[PadButton::RightShoulder, PadButton::RightTrigger]);
    gate_pad(&m, &mut p, false);
    assert!(!p.held.has(PadButton::RightShoulder) && !p.held.has(PadButton::RightTrigger));
    // what stays: B (jump), X (guard / dodge), the D-pad, LB alone, the stick (not on the pad)
    let mut p = pad(&[PadButton::JUMP, PadButton::GUARD, PadButton::DPadDown], &[PadButton::LeftThumb]);
    let before = p;
    assert!(!gate_pad(&m, &mut p, false));
    assert_eq!(p, before);
}

#[test]
fn nocombat_the_state_queue_refuses_offensive_states() {
    use crate::states::StateData;
    let _zone = scope(true);
    assert_eq!(state_act(&StateData::DEFAULT), None, "Idle is no act");
    let attack = StateData::Attack(crate::states::attack::Attack::RESET);
    assert_eq!(state_act(&attack), Some(Act::Attack));
    assert!(!allows(Fighter::Sora, Act::Attack));
    let item = StateData::UseItem(crate::states::use_item::UseItem::RESET);
    assert_eq!(state_act(&item), Some(Act::Item));
    assert!(allows(Fighter::Sora, Act::Item));
}
