//! OWNER: the magic implementer. WHAT THE CAMP MENU NEEDS FROM THE COMMAND SYSTEM: the writes of
//! the Customize screen (docs/sora-ui-spec.md 10.6), the item slots of the Equipment screen and
//! the Items screen's "use" (10.5), as plain functions over a `&mut Sora` between steps (while
//! the menu holds the pause). Nothing here reads the pad: the menu's own navigation is the camp
//! menu's; these are the commits its Decide / Square / Triangle make.
//!
//! Storage (magic spec J 3.1): `shortcut[3][4]` = `CommandState::shortcuts`, `magicSlot[6]` =
//! `CommandState::magic_slots`, the "tier chosen by hand" bits S+0x5C = `CommandState::tier_chosen`;
//! the pouch (`record+0x118`) and the stock (`GW+0x86B4`) = `PawnMagic::pouch`. The command menu
//! reads them every step: a change shows in the next `UiSnapshot::command` without a rebuild call.
//!
//! Evidence level: the rules are the UI spec's (decompiled C). Where the spec names a function
//! without giving its rule the reading is marked INFERRED here and listed in the state note
//! (`build/sora_tmp/IMPL_MAGIC_STATE.md`).

use crate::magic::items::{command_of_item, item_of_command};
use crate::magic::spells::{self, Element};
use crate::magic::{Pouch, PouchSlot};
use crate::sora::Sora;
use crate::status::{ItemKind, ItemOutcome, Status};

/// The tabs of the shortcut list (UI spec 10.6: All, Magic, Links, Items; All = every entry).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Magic,
    Items,
}

/// One entry a shortcut slot can take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Assignable {
    /// `ETresCommandKind`.
    pub kind: u8,
    pub name: &'static str,
    pub tab: Tab,
    /// An item entry whose pouch slot has auto-reload (the screen shows the mark).
    pub auto_reload: bool,
}

/// Why an equip / use was refused (the dialogs of UI spec 10.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// No such slot (not owned), or the slot's content is fixed.
    Slot,
    /// The stock holds none of the item (static SE 4 on the Items screen).
    NoStock,
    /// The removed item does not fit back into the stock (`kh3_camp.equip.dialog_0002`).
    StockFull,
    /// `kh3_camp.item.dialog_0002`: he cannot benefit.
    NoBenefit,
    /// Not an item the Items screen can use (category 1 or 2, and not Panacea 0x10A: 10.5).
    NotUsable,
}

/// The element order of `UI/Data/CustomizeDataAsset.MagicCommands` (UI spec 10.6).
const ASSET_ORDER: [Element; 6] = [Element::Fire, Element::Blizzard, Element::Thunder, Element::Water, Element::Aero, Element::Cure];

fn learned_tier(status: &Status, element: Element) -> u8 {
    status.magic.iter().find(|m| Element::from_name(&m.name) == Some(element)).map_or(0, |m| m.tier.min(3) as u8)
}

/// Makes sure the slots and the pouch hold the loadout (they fill lazily on the first step; the
/// menu may open before it). Every function below calls it.
pub fn ensure(sora: &mut Sora) {
    let status = sora.status().clone();
    sora.command_mut().fill(&status);
    let magic = sora.magic_mut();
    if !magic.pouch.filled && !status.placeholder {
        magic.pouch = Pouch::from_status(&status);
    }
}

// ------------------------------------------------------------------------------- Magic layer

/// `magicSlot[0 .. 5]` in command-menu order (0 = empty).
pub fn magic_slots(sora: &mut Sora) -> [u8; 6] {
    ensure(sora);
    sora.command().magic_slots
}

/// The three tier buttons of a slot's element: (command kind, learned).
pub fn magic_tiers(sora: &mut Sora, slot: usize) -> Option<[(u8, bool); 3]> {
    ensure(sora);
    let unit = spells::unit(*sora.command().magic_slots.get(slot)?)?;
    let learned = learned_tier(sora.status(), unit.element);
    Some([1u8, 2, 3].map(|tier| (unit.element.command(tier), tier <= learned)))
}

/// Decide on a learned tier: writes the kind into the slot (0x140E970B0) and sets the slot's
/// hand-chosen bit when the kind changed (0x140E95E20). False: not that slot's element, or the
/// tier is not learned.
pub fn set_magic_tier(sora: &mut Sora, slot: usize, kind: u8) -> bool {
    ensure(sora);
    let status = sora.status().clone();
    sora.command_mut().set_magic_slot(&status, slot, kind)
}

/// Triangle "Reorder": the two slots swap, kind and bit.
pub fn swap_magic_slots(sora: &mut Sora, a: usize, b: usize) -> bool {
    ensure(sora);
    let c = sora.command_mut();
    if a >= 6 || b >= 6 {
        return false;
    }
    c.magic_slots.swap(a, b);
    c.tier_chosen.swap(a, b);
    true
}

// ---------------------------------------------------------------------------- Shortcut sets

/// `shortcut[page][0 .. 3]`: right, top, left, bottom face button (0 = empty).
pub fn shortcuts(sora: &mut Sora, page: usize) -> [u8; 4] {
    ensure(sora);
    sora.command().shortcuts[page.min(2)]
}

/// The list a shortcut button opens (0x1415EA9C0), in the game's order: every learned magic
/// kind of every element and tier in asset order (each tier its own entry), then every item
/// command whose item "has ever been obtained" (0x140E8F010). INFERRED for the port: an item
/// counts as obtained when a pouch slot holds it or the stock has an entry for it.
pub fn assignable(sora: &mut Sora) -> Vec<Assignable> {
    ensure(sora);
    let mut out = Vec::new();
    for element in ASSET_ORDER {
        for tier in 1..=learned_tier(sora.status(), element) {
            let kind = element.command(tier);
            out.push(Assignable { kind, name: spells::command_name(kind), tab: Tab::Magic, auto_reload: false });
        }
    }
    let pouch = &sora.magic().pouch;
    for kind in 58u8..=68 {
        let Some(item) = item_of_command(kind) else { continue };
        let slot = pouch.slots.iter().find(|s| s.exists && s.item == Some(item));
        if slot.is_some() || pouch.stock.iter().any(|s| s.0 == item) {
            out.push(Assignable { kind, name: spells::command_name(kind), tab: Tab::Items, auto_reload: slot.is_some_and(|s| s.auto_reload) });
        }
    }
    out
}

/// Decide on an entry: `shortcut[page][slot] = kind` (0x140E97F10). The same command may sit in
/// several slots and sets. False: not an entry of [`assignable`].
pub fn set_shortcut(sora: &mut Sora, page: usize, slot: usize, kind: u8) -> bool {
    if page >= 3 || slot >= 4 || !assignable(sora).iter().any(|a| a.kind == kind) {
        return false;
    }
    sora.command_mut().shortcuts[page][slot] = kind;
    true
}

/// Square "Remove": the slot is cleared.
pub fn clear_shortcut(sora: &mut Sora, page: usize, slot: usize) -> bool {
    ensure(sora);
    if page >= 3 || slot >= 4 {
        return false;
    }
    sora.command_mut().shortcuts[page][slot] = 0;
    true
}

/// The shortcut page the HUD panel shows (0 = A, 1 = B, 2 = C).
pub fn set_shortcut_page(sora: &mut Sora, page: usize) {
    sora.command_mut().shortcut_page = page.min(2) as u8;
}

// --------------------------------------------------------------- Equipment screen: item slots

/// The eight pouch entries (`exists` = Sora owns the slot) and the stock.
pub fn pouch_slots(sora: &mut Sora) -> [PouchSlot; 8] {
    ensure(sora);
    sora.magic().pouch.slots
}

pub fn stock(sora: &mut Sora) -> Vec<(ItemKind, u8)> {
    ensure(sora);
    sora.magic().pouch.stock.clone()
}

/// The equip rule for an item slot (0x1415EC620 -> 0x140E94F90; UI spec 10.5): the removed item
/// goes back to the stock (not when the slot was used and waiting for a refill) and must fit
/// (99); the new item leaves the stock and inherits the auto-reload setting of the other slots
/// that hold the same item. `None` = Square "Remove". The change is in force at once.
pub fn equip_item(sora: &mut Sora, slot: usize, item: Option<ItemKind>) -> Result<(), Refusal> {
    ensure(sora);
    let pouch = &mut sora.magic_mut().pouch;
    let Some(current) = pouch.slots.get(slot).copied().filter(|s| s.exists && !s.fixed) else { return Err(Refusal::Slot) };
    if current.item == item && !current.used {
        return Ok(());
    }
    if let Some(new) = item {
        // The slot's own item is row 0 of the stock list: it does not need stock.
        if pouch.stock_of(new) == 0 && !(current.item == Some(new) && !current.used) {
            return Err(Refusal::NoStock);
        }
    }
    let returns = current.item.filter(|_| !current.used);
    if let Some(old) = returns {
        if pouch.stock_of(old) >= 99 {
            return Err(Refusal::StockFull);
        }
    }
    if let Some(old) = returns {
        pouch.stock_add(old, 1);
    }
    match item {
        Some(new) => {
            if let Some(entry) = pouch.stock.iter_mut().find(|s| s.0 == new) {
                entry.1 = entry.1.saturating_sub(1);
            }
            let auto_reload = pouch.slots.iter().enumerate().any(|(k, s)| k != slot && s.exists && s.item == Some(new) && s.auto_reload);
            pouch.slots[slot] = PouchSlot { item: Some(new), exists: true, fixed: false, auto_reload, used: false };
        }
        None => pouch.slots[slot] = PouchSlot { exists: true, ..PouchSlot::default() },
    }
    sync(sora);
    Ok(())
}

/// Triangle on an item row: Auto-Reload toggles for that item kind on every slot holding it
/// (0x1415E8430 -> 0x140E94F60). Returns the new setting; `None` when no slot holds the item.
pub fn toggle_auto_reload(sora: &mut Sora, item: ItemKind) -> Option<bool> {
    ensure(sora);
    let pouch = &mut sora.magic_mut().pouch;
    let on = !pouch.slots.iter().find(|s| s.exists && s.item == Some(item))?.auto_reload;
    for slot in pouch.slots.iter_mut().filter(|s| s.exists && s.item == Some(item)) {
        slot.auto_reload = on;
        // A used slot without auto-reload is an empty slot (J 7.1: the id is zeroed on use).
        if !on && slot.used {
            *slot = PouchSlot { exists: true, ..PouchSlot::default() };
        }
    }
    sync(sora);
    Some(on)
}

fn sync(sora: &mut Sora) {
    let pouch = sora.magic().pouch.clone();
    pouch.sync_status(sora.status_mut());
}

// ---------------------------------------------------------------------------- Items screen

/// The benefit test of the Items screen (0x1415C3C30). INFERRED (the UI spec names the function
/// and its two dialogs, not its rule): the item would change something on Sora now.
pub fn would_benefit(status: &Status, item: ItemKind) -> bool {
    if status.hp <= 0 {
        return false;
    }
    let hp = status.hp < status.max_hp();
    let mp = status.mp_charging || status.mp < status.max_mp();
    let focus = status.focus_empty || status.focus < status.max_focus();
    match item {
        ItemKind::Potion | ItemKind::HiPotion | ItemKind::MegaPotion => hp,
        ItemKind::Ether | ItemKind::HiEther | ItemKind::MegaEther => mp,
        ItemKind::Elixir | ItemKind::Megalixir | ItemKind::Tent => hp || mp || focus,
        ItemKind::Refocuser | ItemKind::HiRefocuser => focus,
        ItemKind::Panacea => false,
    }
}

/// Decide on a member in the Items screen's target mode (0x140EBE1E0(GI, item, character, 1)):
/// one copy leaves the STOCK (not the pouch) and the item's effect lands at once (the effect of
/// the item command, magic spec E I5; no state, no 0.5 s delay: the game is paused). The screen
/// plays static SE 30 on success.
pub fn use_item_from_menu(sora: &mut Sora, item: ItemKind) -> Result<ItemOutcome, Refusal> {
    ensure(sora);
    if command_of_item(item).is_none() || item == ItemKind::Panacea {
        return Err(Refusal::NotUsable);
    }
    if sora.magic().pouch.stock_of(item) == 0 {
        return Err(Refusal::NoStock);
    }
    if !would_benefit(sora.status(), item) {
        return Err(Refusal::NoBenefit);
    }
    if let Some(entry) = sora.magic_mut().pouch.stock.iter_mut().find(|s| s.0 == item) {
        entry.1 -= 1;
    }
    Ok(sora.status_mut().item_effect(item))
}

/// Triangle "Discard" on the Items screen: `count` copies leave the stock. Returns how many did.
pub fn discard(sora: &mut Sora, item: ItemKind, count: u8) -> u8 {
    ensure(sora);
    match sora.magic_mut().pouch.stock.iter_mut().find(|s| s.0 == item) {
        Some(entry) => {
            let n = count.min(entry.1);
            entry.1 -= n;
            n
        }
        None => 0,
    }
}
