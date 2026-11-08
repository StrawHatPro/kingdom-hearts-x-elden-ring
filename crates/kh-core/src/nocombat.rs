//! (NOCOMBAT) THE NO-COMBAT RULE: one rule every character's menu and every attack path asks.
//!
//! THE USER (2026-11-03): "grey out all action commands except for item in the roundtable hold, you can't do anything
//! offensive there"; "this applies to all characters"; "you can't summon in round table hold either".
//!
//! The host says once a frame whether the player stands in a place where Elden Ring forbids attacks ([`set_zone`]:
//! kh-er `nocombat.rs` reads the game's own state). While it does:
//! - every [`Act`] that is offensive is refused for every [`Fighter`] ([`allows`]); a new fighter or a new act is a new
//!   variant of these enums and must be classified here (the matches are exhaustive), so it inherits the rule;
//! - every character's command menu (all of them are a [`CommandSnapshot`]) is greyed except Items ([`grey`]);
//! - the presses that would start something offensive from what the menu shows are taken off the pad before anything
//!   reads it, and a press on a greyed entry plays KH3's refused sound ([`gate_pad`]);
//! - a summoned spirit (an ash, Mimic Sora) does not stay ([`may_stay`]): it leaves as at a save point.
//!
//! Walking, jumping, the dodge, the guard, Items, the camp menu and talking are not offensive and stay.

use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::command::{summon, CommandRow, CommandSnapshot, MenuState};
use crate::input::{PadButton, PadState};

/// The Items row of the main list (`ETresCommandKind` 57).
pub const ITEMS_MAIN: u8 = 57;

static ZONE: AtomicBool = AtomicBool::new(false);

thread_local! {
    /// A thread's own answer (tests, the offline harness): wins over the host's while set.
    static LOCAL: Cell<Option<bool>> = const { Cell::new(None) };
}

/// THE HOST, once a frame: the player stands where attacks are forbidden. Returns whether the answer changed.
pub fn set_zone(on: bool) -> bool {
    ZONE.swap(on, Ordering::Relaxed) != on
}

/// Attacks are forbidden here now.
pub fn zone() -> bool {
    LOCAL.with(Cell::get).unwrap_or_else(|| ZONE.load(Ordering::Relaxed))
}

/// The answer for this thread only while the guard lives (tests and the offline pictures: the host's flag is not touched,
/// so tests running side by side never see each other's).
pub struct Scope(Option<bool>);

pub fn scope(on: bool) -> Scope {
    Scope(LOCAL.with(|l| l.replace(Some(on))))
}

impl Drop for Scope {
    fn drop(&mut self) {
        LOCAL.with(|l| l.set(self.0));
    }
}

/// Every character the port drives. A new one is a new variant: the exhaustive matches below make it classified before
/// it compiles, and `nocombat_tests` walks [`Fighter::ALL`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Fighter {
    Sora,
    Donald,
    Goofy,
    /// The Mimic Tear as Sora (kh-er `spirit_mimic.rs`).
    MimicSora,
    /// An Elden Ring spirit ash Sora summoned (kh-er `spirit.rs`).
    SpiritAsh,
}

impl Fighter {
    pub const ALL: [Fighter; 5] = [Fighter::Sora, Fighter::Donald, Fighter::Goofy, Fighter::MimicSora, Fighter::SpiritAsh];

    pub const fn name(self) -> &'static str {
        match self {
            Fighter::Sora => "Sora",
            Fighter::Donald => "Donald",
            Fighter::Goofy => "Goofy",
            Fighter::MimicSora => "Mimic Sora",
            Fighter::SpiritAsh => "a spirit ash",
        }
    }

    /// In the zone: the party stays, pacified; a summon is sent away (as at a save point).
    pub const fn stays(self) -> bool {
        match self {
            Fighter::Sora | Fighter::Donald | Fighter::Goofy => true,
            Fighter::MimicSora | Fighter::SpiritAsh => false,
        }
    }
}

/// What a character may try to do. A new kind of action is a new variant, classified in [`Act::offensive`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Act {
    /// The Attack command, a combo, a launch, an air attack, a form's attack, Donald's missiles and Wand Bonk, Goofy's string.
    Attack,
    /// A spell: Sora's Magic, Donald's spells and attunements, a grand magic, Cure too (the whole Magic list greys).
    Magic,
    /// Summon / Unsummon, the spirit ashes, Mimic Sora.
    Summon,
    /// The shotlock aim (RB held), its shots and Focus spending.
    Shotlock,
    /// A Y situation command of any kind not named below.
    Situation,
    /// A formchange (Keyblade transformation).
    FormChange,
    /// A Keyblade form's finisher.
    Finisher,
    /// A team attack / Link / unison.
    TeamAttack,
    /// Rage Form, Risk Charge.
    Rage,
    /// An XS ability's attack (Sonic Dash, Ancient Light, Light Wave, Diving Strike ...).
    Xs,
    /// Goofy's guard attacks, counters and Arts, a guard counter.
    GuardAttack,
    /// Goofy Breaker.
    Breaker,
    /// An AI character choosing an attack.
    AiAttack,
    /// A hit reaching a body (the world's last word).
    Hit,
    Item,
    Move,
    Jump,
    Dodge,
    Guard,
    Talk,
    Camp,
    /// Switching the held Keyblade / weapon (D-pad on Attack): nothing is struck.
    WeaponSwitch,
}

impl Act {
    pub const ALL: [Act; 22] = [
        Act::Attack,
        Act::Magic,
        Act::Summon,
        Act::Shotlock,
        Act::Situation,
        Act::FormChange,
        Act::Finisher,
        Act::TeamAttack,
        Act::Rage,
        Act::Xs,
        Act::GuardAttack,
        Act::Breaker,
        Act::AiAttack,
        Act::Hit,
        Act::Item,
        Act::Move,
        Act::Jump,
        Act::Dodge,
        Act::Guard,
        Act::Talk,
        Act::Camp,
        Act::WeaponSwitch,
    ];

    pub const fn offensive(self) -> bool {
        match self {
            Act::Attack | Act::Magic | Act::Summon | Act::Shotlock | Act::Situation | Act::FormChange | Act::Finisher | Act::TeamAttack | Act::Rage | Act::Xs | Act::GuardAttack | Act::Breaker | Act::AiAttack | Act::Hit => true,
            Act::Item | Act::Move | Act::Jump | Act::Dodge | Act::Guard | Act::Talk | Act::Camp | Act::WeaponSwitch => false,
        }
    }
}

/// THE RULE: may `who` do `act` now?
pub fn allows(who: Fighter, act: Act) -> bool {
    // every fighter answers to the same rule (the match makes a new fighter a compile error until it is placed here)
    match who {
        Fighter::Sora | Fighter::Donald | Fighter::Goofy | Fighter::MimicSora | Fighter::SpiritAsh => !(zone() && act.offensive()),
    }
}

/// The world's side, when the doer is not known (a hit arriving at a body, the targets listed to anyone): may ANY fighter
/// do `act` now?
pub fn world_allows(act: Act) -> bool {
    Fighter::ALL.iter().all(|f| allows(*f, act))
}

/// May `who` remain in the world now (a summon is sent away in the zone)?
pub fn may_stay(who: Fighter) -> bool {
    !zone() || who.stays()
}

/// A command kind's act (Sora's command kinds; the friends' menus use the same kinds for their items).
pub fn act_of_kind(kind: u8) -> Act {
    if kind == ITEMS_MAIN || crate::magic::items::item_of_command(kind).is_some() {
        Act::Item
    } else if kind == crate::magic::spells::cmd::ATTACK {
        Act::Attack
    } else if summon::is_summon_kind(kind) {
        Act::Summon
    } else {
        Act::Magic
    }
}

/// A situation command kind's act (combat/situation.rs `execute`'s kinds).
pub fn act_of_situation(kind: i32) -> Act {
    match kind {
        0x80..=0x9A | 0xDD..=0xE0 => Act::FormChange,
        // (Risk Charge 179 and Ancient Light 174 outside a form; a form's finisher of the same kind is offensive as well)
        179 => Act::Rage,
        174 | 248 => Act::Xs,
        0x6D | 0x9C..=0xA8 | 0xAA..=0xB3 | 0xE1 | 0xE2 => Act::Finisher,
        193 | 195 | 207 | 208 => Act::TeamAttack,
        110..=126 | 231 => Act::Magic,
        1..=26 => Act::GuardAttack,
        _ => Act::Situation,
    }
}

/// Sora's state's act, for the state queue's last word (states/host.rs `main_proc`): None = not an act of its own (moving,
/// guarding, a reaction). Exhaustive: a new state is classified before it compiles.
pub(crate) fn state_act(s: &crate::states::StateData) -> Option<Act> {
    use crate::states::StateData as S;
    match s {
        S::Attack(_) | S::ComboLimit(_) | S::ComboSh(_) | S::ComboCl(_) | S::ComboYo(_) | S::ComboSk(_) | S::ComboHl(_) | S::ComboFl(_) | S::ComboGm(_) | S::ComboDw(_) | S::ComboBz(_) | S::ComboWa(_) | S::ComboHm(_) | S::ComboDr(_) | S::ComboSw(_) | S::ComboLf(_) | S::ComboDf(_) | S::ComboTf(_) | S::ShooterMode(_) => Some(Act::Attack),
        S::ComboAnti(_) => Some(Act::Rage),
        S::FireMagic(_) => Some(Act::Magic),
        S::ShootFlow(_) => Some(Act::Shotlock),
        S::StyleChange(_) => Some(Act::FormChange),
        S::StyleFinish(_) | S::StyleFinishSh(_) | S::StyleFinishDr(_) => Some(Act::Finisher),
        S::FrShieldGuardian(_) | S::FrGoofyShoot(_) | S::FrCallMeteor(_) | S::FrFlareForce(_) => Some(Act::TeamAttack),
        S::SummonChant(_) => Some(Act::Summon),
        S::UseItem(_) => Some(Act::Item),
        S::Guard(_) | S::Reflect(_) => Some(Act::Guard),
        S::DodgeRoll(_) | S::RiskDodge(_) | S::AirSlide(_) => Some(Act::Dodge),
        S::WeaponChange(_) => Some(Act::WeaponSwitch),
        S::Idle(_) | S::Run(_) | S::Jump(_) | S::Damage(_) | S::Die(_) | S::AthleticFlow(_) | S::Glide(_) | S::WallKick(_) | S::SuperSlide(_) | S::WallRun(_) | S::PoleTurn(_) | S::RailSlide(_) | S::Dangle(_) | S::SlopeSlide(_) | S::Swaying(_) | S::Hopping(_) | S::Floating(_) | S::BadstatStun(_) | S::BadstatFreeze(_) | S::BadstatBurn(_) | S::OpenTreasureBox(_) => None,
    }
}

/// A row of a menu is still usable in the zone: Items and the item rows only (an empty slot has no use either).
pub fn row_allowed(row: &CommandRow) -> bool {
    row.kind != 0 && act_of_kind(row.kind) == Act::Item
}

/// THE MENU, AS SHOWN: in the zone every entry but Items greys (KH3's `disabled` look), the Y list greys, the shotlock
/// panel goes. Any character's menu (Sora's, a leading friend's, Donald's own) is a `CommandSnapshot`.
pub fn grey(snap: &mut CommandSnapshot) {
    if !zone() {
        return;
    }
    let rows = snap.main.iter_mut().chain(snap.lists.iter_mut().flatten()).chain(snap.shortcut.iter_mut());
    for row in rows {
        row.enabled &= row_allowed(row);
    }
    for s in &mut snap.situations {
        s.enabled = false;
    }
    snap.shoot_flow = None;
}

/// The face buttons of the shortcut panel (LB held), in the slot order of `CommandSnapshot::shortcut`.
const SHORTCUT_FACES: [PadButton; 4] = [PadButton::FaceRight, PadButton::FaceTop, PadButton::FaceLeft, PadButton::FaceBottom];

fn take(pad: &mut PadState, b: PadButton) {
    pad.pressed.set(b, false);
    pad.held.set(b, false);
}

/// THE PAD, BEFORE ANY CHARACTER READS IT (Sora's step, a pad-driven friend's decisions): in the zone the presses that
/// would start something offensive from the menu shown (`snap`) are taken off. Returns true when a press was refused on a
/// greyed entry (the caller plays KH3's refused sound, as for any refused confirm).
/// - LB + a face button on a shortcut that is not an item;
/// - A (decide) on Attack, or on a greyed main entry / list row; D-pad right on a greyed main entry (it would open it);
/// - Y (situation commands, finishers, Goofy Breaker, Donald's Arcane list): always; refused when `y_offers` (something
///   was on the Y list);
/// - RB and RT held or pressed (the shotlock aim, Donald's Spell Volley, the gun forms' aim).
///
/// Navigation (the D-pad, B, LB alone), the guard / dodge (X), the jump and the stick are left.
pub fn gate_pad(snap: &CommandSnapshot, pad: &mut PadState, y_offers: bool) -> bool {
    if !zone() {
        return false;
    }
    let mut refused = false;
    let pressed = |pad: &PadState, b: PadButton| pad.pressed.has(b);
    let lb = pad.held.has(PadButton::LeftShoulder) || pad.pressed.has(PadButton::LeftShoulder);
    if lb {
        // (an empty slot is taken too: Goofy's Arts fire on LB + a face button from his kit, whatever the panel shows)
        for (slot, b) in SHORTCUT_FACES.iter().enumerate() {
            let row = &snap.shortcut[slot];
            if !row_allowed(row) {
                refused |= pressed(pad, *b) && row.kind != 0;
                take(pad, *b);
            }
        }
    } else {
        if pressed(pad, PadButton::ATTACK) {
            let row = match snap.state {
                MenuState::SubSel => snap.lists.get(snap.list).and_then(|l| l.get(snap.row_cursor)).copied(),
                MenuState::ShootFlow => None,
                MenuState::MainSel | MenuState::Shortcut => snap.main.get(snap.main_cursor).copied(),
            };
            if !row.as_ref().is_some_and(row_allowed) {
                take(pad, PadButton::ATTACK);
                refused |= row.is_some();
            }
        }
        if snap.state == MenuState::MainSel && pressed(pad, PadButton::DPadRight) {
            let row = snap.main.get(snap.main_cursor).copied().unwrap_or_default();
            // (D-pad right on Attack is the Keyblade switch: nothing is struck)
            if row.kind != crate::magic::spells::cmd::ATTACK && !row_allowed(&row) {
                take(pad, PadButton::DPadRight);
                refused |= row.kind != 0 || !row.name.is_empty();
            }
        }
        if pad.pressed.has(PadButton::SITUATION) || pad.held.has(PadButton::SITUATION) {
            refused |= pad.pressed.has(PadButton::SITUATION) && y_offers;
            take(pad, PadButton::SITUATION);
        }
    }
    take(pad, PadButton::RightShoulder);
    take(pad, PadButton::RightTrigger);
    refused
}

#[cfg(test)]
#[path = "nocombat_tests.rs"]
mod nocombat_tests;
