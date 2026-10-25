//! The battle pouch and the item commands (docs/sora-magic-spec.md parts E and J 7).
//!
//! The pouch is 8 entries of `{item id, flags}` (flags: exists, fixed, auto-reload, used).
//! Sora owns 3 slots at the start and one more per ITEM_SLOT_UP1 victory bonus (6 after the
//! base game, 8 with Re Mind): the loadout's `item_slots`. An item in the pouch is already out
//! of the stock; a used auto-reload slot is refilled from the stock when the game leaves a
//! pause and at pawn set-up, never at battle end (J 7.5).
//!
//! The EFFECT of an item is the status implementer's `Status::item_effect` (E I5); this file is
//! the pouch, the command <-> item mapping, and the 0.5 s delayed effect queue (pawn+0x7E8).

use super::QueuedItemEffect;
use crate::host::{Attach, EffectId, EffectSpawn};
use crate::pawn::Ctx;
use crate::status::{ItemKind, Status};

/// `ItemOfCommand` (0x1411270C0; E I2).
pub fn item_of_command(cmd: u8) -> Option<ItemKind> {
    Some(match cmd {
        58 => ItemKind::Potion,
        59 => ItemKind::HiPotion,
        60 => ItemKind::MegaPotion,
        61 => ItemKind::Ether,
        62 => ItemKind::HiEther,
        63 => ItemKind::MegaEther,
        64 => ItemKind::Elixir,
        65 => ItemKind::Megalixir,
        66 => ItemKind::Refocuser,
        67 => ItemKind::HiRefocuser,
        68 => ItemKind::Panacea,
        _ => return None,
    })
}

/// The inverse (0x141127250).
pub fn command_of_item(item: ItemKind) -> Option<u8> {
    (58u8..=68).find(|&c| item_of_command(c) == Some(item))
}

/// One pouch entry (J 7.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct PouchSlot {
    /// The item (`None` = id 0: empty).
    pub item: Option<ItemKind>,
    /// Bit 0: the slot exists (Sora owns it).
    pub exists: bool,
    /// Bit 1: the content cannot be changed.
    pub fixed: bool,
    /// Bit 2: auto-reload.
    pub auto_reload: bool,
    /// Bit 3: the item is spent but the slot remembers it.
    pub used: bool,
}

/// The battle pouch and the stock behind it.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Pouch {
    pub slots: [PouchSlot; 8],
    /// The inventory counts (`save+0x86B4`): (item, count <= 99).
    pub stock: Vec<(ItemKind, u8)>,
    /// The pouch was filled from the status once.
    pub filled: bool,
}

impl Pouch {
    /// The pouch of a loadout: `item_slots` slots exist (at most 8), the loadout's items fill
    /// them in order. TEST-CONFIGURATION CHOICES (the loadout sheet does not say): every filled
    /// slot is auto-reload and the stock is empty.
    pub fn from_status(status: &Status) -> Pouch {
        let mut pouch = Pouch { filled: true, ..Pouch::default() };
        let count = (status.item_slots as usize).min(8);
        for slot in pouch.slots.iter_mut().take(count) {
            slot.exists = true;
        }
        let mut k = 0;
        for stack in &status.items {
            let Some(item) = ItemKind::from_name(&stack.item) else { continue };
            for _ in 0..stack.count {
                if k >= count {
                    break;
                }
                pouch.slots[k] = PouchSlot { item: Some(item), exists: true, fixed: false, auto_reload: true, used: false };
                k += 1;
            }
        }
        pouch
    }

    /// `Count(item)` (0x140E8CCF0): slots that exist, hold the item and are not used.
    pub fn count(&self, item: ItemKind) -> u32 {
        self.slots.iter().filter(|s| s.exists && s.item == Some(item) && !s.used).count() as u32
    }

    /// Usable slots in all.
    pub fn slot_count(&self) -> usize {
        self.slots.iter().filter(|s| s.exists).count()
    }

    /// Use (0x140E98B90): the first usable slot holding the item; without fixed / auto-reload
    /// the slot becomes empty, otherwise it is marked used.
    pub fn use_item(&mut self, item: ItemKind) -> bool {
        let Some(slot) = self.slots.iter_mut().find(|s| s.exists && s.item == Some(item) && !s.used) else { return false };
        if !slot.fixed && !slot.auto_reload {
            slot.item = None;
        } else {
            slot.used = true;
        }
        true
    }

    pub fn stock_of(&self, item: ItemKind) -> u8 {
        self.stock.iter().find(|s| s.0 == item).map_or(0, |s| s.1)
    }

    /// Adds to the stock (maximum 99 per battle item: `TresItemData.m_MaxNum`).
    pub fn stock_add(&mut self, item: ItemKind, n: u8) {
        match self.stock.iter_mut().find(|s| s.0 == item) {
            Some(entry) => entry.1 = entry.1.saturating_add(n).min(99),
            None => self.stock.push((item, n.min(99))),
        }
    }

    /// Refill of one character (0x1405B5B70): every slot that exists, is auto-reload and used,
    /// with stock left, is restored and takes one from the stock. Returns how many.
    pub fn refill(&mut self) -> u32 {
        let mut n = 0;
        for k in 0..self.slots.len() {
            let slot = self.slots[k];
            if slot.exists && slot.auto_reload && slot.used {
                if let Some(item) = slot.item {
                    if let Some(entry) = self.stock.iter_mut().find(|s| s.0 == item && s.1 > 0) {
                        entry.1 -= 1;
                        self.slots[k].used = false;
                        n += 1;
                    }
                }
            }
        }
        n
    }

    /// Puts an item into the first free slot (the camp menu's equip; a test and host helper).
    pub fn put(&mut self, item: ItemKind, auto_reload: bool) -> bool {
        match self.slots.iter_mut().find(|s| s.exists && s.item.is_none()) {
            Some(slot) => {
                *slot = PouchSlot { item: Some(item), exists: true, fixed: false, auto_reload, used: false };
                true
            }
            None => false,
        }
    }

    /// One more slot (a victory bonus ITEM_SLOT_UP1), up to 8.
    pub fn add_slot(&mut self) -> bool {
        match self.slots.iter_mut().find(|s| !s.exists) {
            Some(slot) => {
                slot.exists = true;
                true
            }
            None => false,
        }
    }

    /// The Items sub-list (J 2.3): for the slots in order, each slot that holds an item and is
    /// either unused or auto-reload contributes its item; duplicates are merged (the first
    /// occurrence keeps the position): (item, usable count).
    pub fn list(&self) -> Vec<(ItemKind, u32)> {
        let mut out: Vec<(ItemKind, u32)> = Vec::new();
        for slot in &self.slots {
            if let (true, Some(item)) = (slot.exists, slot.item) {
                if (!slot.used || slot.auto_reload) && !out.iter().any(|e| e.0 == item) {
                    out.push((item, self.count(item)));
                }
            }
        }
        out
    }

    /// Writes the usable counts back into the status' item stacks (what the status lines show).
    pub fn sync_status(&self, status: &mut Status) {
        status.items.clear();
        for (item, count) in self.list() {
            if count > 0 {
                status.items.push(crate::status::ItemStack { item: item.name().to_string(), count });
            }
        }
    }
}

/// The pawn's pouch, filled from the loadout on first use.
pub(crate) fn pouch<'a>(cx: &'a mut Ctx<'_, '_>) -> &'a mut Pouch {
    if !cx.pawn.magic.pouch.filled {
        cx.pawn.magic.pouch = Pouch::from_status(&cx.pawn.status);
    }
    &mut cx.pawn.magic.pouch
}

/// `UseItem(user, kind, target, consume = 1, delay = 0.5)` (0x1405BCBE0) at the `TresTiming`
/// notify: the slot is consumed and the effect queued on the target with its delay.
pub(crate) fn use_item(cx: &mut Ctx, cmd: u8, delay: f32) -> bool {
    let Some(item) = item_of_command(cmd) else { return false };
    if !pouch(cx).use_item(item) {
        return false;
    }
    let pouch = cx.pawn.magic.pouch.clone();
    pouch.sync_status(&mut cx.pawn.status);
    cx.pawn.magic.item_effects.push(QueuedItemEffect { delay, item });
    true
}

/// The queued effects count down on the pawn's clock; at 0 `ItemEffect` (0x1405BE820) runs.
pub(crate) fn tick(cx: &mut Ctx, dt: f32) {
    if cx.pawn.magic.item_effects.is_empty() {
        return;
    }
    let mut due = Vec::new();
    cx.pawn.magic.item_effects.retain_mut(|e| {
        e.delay -= dt;
        if e.delay <= 0.0 {
            due.push(e.item);
            false
        } else {
            true
        }
    });
    for item in due {
        let outcome = cx.pawn.status.item_effect(item);
        // The heal effect on the mesh (`Recover` 0x141445F40) with the HUD effect kind.
        let _ = outcome;
        // (audio implementer) the item's own heal effect where one is known (it carries the
        // item's sound); "sora_kaihuku" stays as the placeholder for the others.
        let name = crate::host::audio::item_heal_effect(item as u32).unwrap_or("sora_kaihuku");
        cx.spawn_effect(EffectSpawn { id: EffectId(u32::MAX), name, at: Attach::SORA, rotation: glam::Vec3::ZERO, scale: 1.0, persistent: false });
    }
}
