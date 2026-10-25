//! The camp menu's Equipment and Items screens (docs/sora-ui-spec.md 10.5), logic only: the
//! host supplies the slots, the stock and the inventory as plain data and applies the
//! [`crate::camp::CampAction`]s it gets back.
//!
//! WHAT THE PORT DOES NOT HAVE: other party members (L2 / R2 do nothing with Sora alone), the
//! asset "loading" mark, the Keyblade upgrade rows of the status pane, and the read-only pane
//! `ItemStatusA1`'s parameter block (which authored parameter set belongs to which item kind
//! is decided by native code that was not read: `0x1415A9CE0`); the pane shows the help text.

use crate::camp::{CampAction, CampKey, Dialog, DialogKind, Pending, StatsIn};
use crate::list::ListView;
use crate::menu::{SE_MENU_CLOSE, SE_MENU_CURSOR, SE_MENU_DECIDE, SE_MENU_OPEN, SE_MENU_REFUSED};
use crate::UiSound;

/// Slot kinds in the equipped list's order (row record +4).
pub const KIND_WEAPON: u8 = 0;
pub const KIND_ARMOUR: u8 = 1;
pub const KIND_ACCESSORY: u8 = 2;
pub const KIND_ITEM: u8 = 3;

/// Static SE of the two screens (spec 10.5): equip 29, item used 30.
pub const SE_EQUIP: u32 = 29;
pub const SE_ITEM_USED: u32 = 30;

/// One piece of equipment or one battle item.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Piece {
    /// The host's id (`WEP_KEYBLADE_SO_00`, `PRT_ITEM01`, `Potion` ...).
    pub id: String,
    pub name: String,
    pub kind: u8,
    /// `m_AP`, `m_AttackPlus`, `m_MagicPlus`, `m_DefensePlus`.
    pub ap: i32,
    pub attack: i32,
    pub magic: i32,
    pub defense: i32,
    /// Fire, Blizzard, Thunder, Water, Aero, Dark, percent.
    pub resist: [i32; 6],
    /// The abilities it supplies while worn (no AP).
    pub abilities: Vec<String>,
    pub help: String,
}

/// One open slot: a row of the equipped list.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct EquipSlot {
    pub kind: u8,
    pub slot: usize,
    pub piece: Option<Piece>,
    /// Item slots: flag bit 2, and "used, waiting for refill".
    pub auto_reload: bool,
    pub used: bool,
    /// The weapon slot that is the current Keyblade (`record+6`).
    pub current: bool,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct StockEntry {
    pub piece: Piece,
    pub count: u32,
}

/// What the host supplies for the Equipment screen (again after every change).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct EquipIn {
    pub slots: Vec<EquipSlot>,
    /// The character's equippable stock, sorted.
    pub stock: Vec<StockEntry>,
    /// AP in use (sum of `m_UseAP` x equipped copies) and the stock maximum per piece.
    pub ap_in_use: i32,
    pub stock_max: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum EquipLayer {
    #[default]
    Slots,
    /// The stock list of the slot under the cursor.
    Stock,
    /// The read-only ability list of the highlighted piece.
    Abilities,
}

/// A row of the stock list: row 0 is the slot's current piece.
#[derive(Clone, Debug, PartialEq)]
pub struct StockRow {
    pub piece: Piece,
    /// `None`: the equipped piece (the row carries the "equipped" mark).
    pub count: Option<u32>,
}

/// The preview pane `ItemStatusB`: max HP, max MP, AP, Strength, Magic, Defense, six resists.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Preview {
    pub now: [i32; 12],
    pub new: [i32; 12],
}

/// The multiplicative resist rule (stats spec 2.7): `100 - ceil(100 * product(1 - r / 100))`.
pub fn resist_total(pieces: &[&Piece], attribute: usize) -> i32 {
    let product: f64 = pieces.iter().map(|p| 1.0 - p.resist[attribute] as f64 / 100.0).product();
    100 - (100.0 * product - 1.0e-9).ceil() as i32
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct EquipScreen {
    pub data: EquipIn,
    pub layer: EquipLayer,
    /// The equipped list (`m_EquippedList`) and the stock list (`m_BackyardEquipmentList`).
    pub list: ListView,
    pub stock: ListView,
    pub stock_rows: Vec<StockRow>,
    pub abilities: ListView,
}

impl EquipScreen {
    pub fn new() -> EquipScreen {
        EquipScreen {
            list: ListView::new("equip.equipDetailAnm.equipDetail.equipListVisibleAnm.equipListVisible.equipList", "CoEquipListItem"),
            stock: ListView::new("equip.equipReplaceAnm.equipReplace.replaceListVisibleAnm.replaceListVisible.replaceList", "CoReplaceListItem1"),
            abilities: ListView::new("equip.abilityListVisibleAnm.abilityListVisible.abilityList", "CoEquipListItem"),
            ..EquipScreen::default()
        }
    }

    pub fn set(&mut self, data: EquipIn) {
        self.data = data;
        self.list.set_count(self.data.slots.len());
        self.list.invalidate();
        if self.layer == EquipLayer::Stock {
            self.build_stock();
        }
    }

    pub fn slot(&self) -> Option<&EquipSlot> {
        usize::try_from(self.list.focus).ok().and_then(|i| self.data.slots.get(i))
    }

    /// The piece the panes describe: the stock row under the cursor, else the slot's piece.
    pub fn highlighted(&self) -> Option<&Piece> {
        match self.layer {
            EquipLayer::Stock => usize::try_from(self.stock.focus).ok().and_then(|i| self.stock_rows.get(i)).map(|r| &r.piece),
            _ => self.slot().and_then(|s| s.piece.as_ref()),
        }
    }

    /// The stock list builder `0x1415E66D0`: the stock filtered by the slot's kind, the slot's
    /// current piece inserted as row 0.
    fn build_stock(&mut self) {
        let Some(slot) = self.slot().cloned() else { return };
        self.stock_rows.clear();
        if let Some(piece) = slot.piece {
            self.stock_rows.push(StockRow { piece, count: None });
        }
        self.stock_rows.extend(self.data.stock.iter().filter(|e| e.piece.kind == slot.kind && e.count > 0).map(|e| StockRow { piece: e.piece.clone(), count: Some(e.count) }));
        self.stock.set_count(self.stock_rows.len());
        self.stock.invalidate();
    }

    /// `m_BackyardItemStatusB`: the character's values now and with the highlighted stock row
    /// in the slot. Max HP / MP never change; a Keyblade outside the current slot changes
    /// nothing.
    pub fn preview(&self, stats: &StatsIn) -> Preview {
        let now = [stats.hp_max, stats.mp_max, stats.ap_max, stats.strength, stats.magic, stats.defense, stats.resists[0], stats.resists[1], stats.resists[2], stats.resists[3], stats.resists[4], stats.resists[5]];
        let mut new = now;
        let (Some(slot), EquipLayer::Stock) = (self.slot(), self.layer) else { return Preview { now, new } };
        let Some(row) = usize::try_from(self.stock.focus).ok().and_then(|i| self.stock_rows.get(i)) else { return Preview { now, new } };
        if slot.kind == KIND_ITEM || (slot.kind == KIND_WEAPON && !slot.current) {
            return Preview { now, new };
        }
        let none = Piece::default();
        let old = slot.piece.as_ref().unwrap_or(&none);
        new[2] += row.piece.ap - old.ap;
        new[3] += row.piece.attack - old.attack;
        new[4] += row.piece.magic - old.magic;
        new[5] += row.piece.defense - old.defense;
        // The pieces that count: the current Keyblade, armour, accessories.
        let counts = |s: &&EquipSlot| s.kind != KIND_ITEM && (s.kind != KIND_WEAPON || s.current);
        let worn: Vec<&Piece> = self.data.slots.iter().filter(counts).filter_map(|s| s.piece.as_ref()).collect();
        let swapped: Vec<&Piece> = self.data.slots.iter().filter(counts).filter_map(|s| if s.kind == slot.kind && s.slot == slot.slot { Some(&row.piece) } else { s.piece.as_ref() }).collect();
        for attribute in 0..6 {
            new[6 + attribute] += resist_total(&swapped, attribute) - resist_total(&worn, attribute);
        }
        Preview { now, new }
    }

    /// The equip rule `0x1415EC620(newItem, row)`: `Ok` = commit, `Err` = the dialog's text key.
    fn may_equip(&self, new: Option<&Piece>, stats: &StatsIn) -> Result<(), &'static str> {
        let Some(slot) = self.slot() else { return Err("") };
        let none = Piece::default();
        let old = slot.piece.as_ref().unwrap_or(&none);
        let new_ap = new.map_or(0, |p| p.ap);
        if slot.kind != KIND_ITEM && old.ap > 0 && new_ap < old.ap && self.data.ap_in_use > stats.ap_max + (new_ap - old.ap) {
            return Err("kh3_camp.equip.dialog_0003");
        }
        if let Some(piece) = &slot.piece {
            let in_stock = self.data.stock.iter().find(|e| e.piece.id == piece.id).map_or(0, |e| e.count);
            if !slot.used && in_stock >= self.data.stock_max {
                return Err("kh3_camp.equip.dialog_0002");
            }
        }
        Ok(())
    }

    /// One event. `Some(true)` = leave the screen (back to the top screen).
    pub fn key(&mut self, key: CampKey, repeat: bool, stats: &StatsIn, sounds: &mut Vec<UiSound>, actions: &mut Vec<CampAction>, dialog: &mut Option<Dialog>) -> bool {
        match self.layer {
            EquipLayer::Slots => match key {
                CampKey::Up | CampKey::Down => {
                    if self.list.move_anchor(key == CampKey::Down, repeat) {
                        sounds.push(UiSound::Play(SE_MENU_CURSOR));
                    }
                }
                CampKey::L1 | CampKey::R1 => {
                    let moved = if key == CampKey::R1 { self.list.next_page(repeat) } else { self.list.prev_page(repeat) };
                    if moved {
                        sounds.push(UiSound::Play(SE_MENU_CURSOR));
                    }
                }
                CampKey::Decide => {
                    if self.slot().is_some() {
                        sounds.push(UiSound::Play(SE_MENU_DECIDE));
                        self.layer = EquipLayer::Stock;
                        self.stock.focus = 0;
                        self.stock.scroll = 0;
                        self.build_stock();
                    }
                }
                // "Remove": equip of item 0, static SE 3; refused for the current Keyblade.
                CampKey::Square => {
                    let Some(slot) = self.slot().cloned() else { return false };
                    if slot.piece.is_none() {
                        return false;
                    }
                    if slot.kind == KIND_WEAPON && slot.current {
                        *dialog = Some(Dialog::message("kh3_camp.equip.dialog_0001"));
                        sounds.push(UiSound::Play(SE_MENU_REFUSED));
                    } else if let Err(text) = self.may_equip(None, stats) {
                        *dialog = Some(Dialog::message(text));
                        sounds.push(UiSound::Play(SE_MENU_REFUSED));
                    } else {
                        sounds.push(UiSound::Play(SE_MENU_CLOSE));
                        actions.push(CampAction::Equip { kind: slot.kind, slot: slot.slot, id: None });
                    }
                }
                CampKey::Triangle => {
                    let Some(slot) = self.slot().cloned() else { return false };
                    let Some(piece) = slot.piece else { return false };
                    if slot.kind == KIND_ITEM {
                        // Auto-Reload for that item kind on every slot holding it; turning it
                        // off while a slot of that item waits for a refill asks first.
                        let on = !slot.auto_reload;
                        let waiting = self.data.slots.iter().any(|s| s.kind == KIND_ITEM && s.used && s.piece.as_ref().is_some_and(|p| p.id == piece.id));
                        if !on && waiting {
                            *dialog = Some(Dialog { text_key: "kh3_camp.equip.dialog_0004", argument: String::new(), kind: DialogKind::YesNo, cursor: 1, pending: Pending::AutoReloadOff(piece.id) });
                        } else {
                            sounds.push(UiSound::Play(SE_MENU_DECIDE));
                            actions.push(CampAction::AutoReload { item: piece.id, on });
                        }
                    } else if !piece.abilities.is_empty() {
                        sounds.push(UiSound::Play(SE_MENU_OPEN));
                        self.layer = EquipLayer::Abilities;
                        self.abilities.set_count(piece.abilities.len());
                        self.abilities.focus = 0;
                        self.abilities.scroll = 0;
                        self.abilities.invalidate();
                    }
                }
                CampKey::Cancel => return true,
                _ => {}
            },
            EquipLayer::Stock => match key {
                CampKey::Up | CampKey::Down => {
                    if self.stock.move_anchor(key == CampKey::Down, repeat) {
                        sounds.push(UiSound::Play(SE_MENU_CURSOR));
                    }
                }
                CampKey::L1 | CampKey::R1 => {
                    let moved = if key == CampKey::R1 { self.stock.next_page(repeat) } else { self.stock.prev_page(repeat) };
                    if moved {
                        sounds.push(UiSound::Play(SE_MENU_CURSOR));
                    }
                }
                CampKey::Decide => {
                    let Some(slot) = self.slot().cloned() else { return false };
                    let Some(row) = usize::try_from(self.stock.focus).ok().and_then(|i| self.stock_rows.get(i)).cloned() else {
                        // "No items.": nothing to choose.
                        sounds.push(UiSound::Play(SE_MENU_REFUSED));
                        return false;
                    };
                    if row.count.is_none() {
                        // Row 0 with nothing to change just returns.
                        sounds.push(UiSound::Play(SE_MENU_DECIDE));
                        self.layer = EquipLayer::Slots;
                    } else if let Err(text) = self.may_equip(Some(&row.piece), stats) {
                        *dialog = Some(Dialog::message(text));
                        sounds.push(UiSound::Play(SE_MENU_REFUSED));
                    } else {
                        sounds.push(UiSound::Play(SE_EQUIP));
                        actions.push(CampAction::Equip { kind: slot.kind, slot: slot.slot, id: Some(row.piece.id) });
                        self.layer = EquipLayer::Slots;
                    }
                }
                CampKey::Cancel => {
                    sounds.push(UiSound::Play(SE_MENU_CLOSE));
                    self.layer = EquipLayer::Slots;
                }
                _ => {}
            },
            EquipLayer::Abilities => match key {
                CampKey::Up | CampKey::Down => {
                    if self.abilities.move_anchor(key == CampKey::Down, repeat) {
                        sounds.push(UiSound::Play(SE_MENU_CURSOR));
                    }
                }
                CampKey::Cancel | CampKey::Triangle => {
                    sounds.push(UiSound::Play(SE_MENU_CLOSE));
                    self.layer = EquipLayer::Slots;
                }
                _ => {}
            },
        }
        false
    }

    /// The button guide of the equipped list (`kh3_camp.equip.button_*`, Sora alone).
    pub fn guide_key(&self) -> &'static str {
        match (self.layer, self.slot()) {
            (EquipLayer::Slots, Some(slot)) if slot.kind == KIND_ITEM && slot.piece.is_some() => {
                if slot.auto_reload {
                    "kh3_camp.equip.button_0016"
                } else {
                    "kh3_camp.equip.button_0015"
                }
            }
            (EquipLayer::Slots, Some(slot)) if slot.piece.as_ref().is_some_and(|p| !p.abilities.is_empty()) => "kh3_camp.equip.button_0014",
            (EquipLayer::Slots, Some(slot)) if slot.piece.is_some() => "kh3_camp.equip.button_0004",
            (EquipLayer::Abilities, _) => "kh3_camp.equip.button_0031",
            _ => "",
        }
    }

    /// The help line: `kh3_camp.equip.detail_0001 ..` by the slot's kind.
    pub fn help_key(&self) -> &'static str {
        match self.slot().map(|s| s.kind) {
            Some(KIND_WEAPON) => "kh3_camp.equip.detail_0001",
            Some(KIND_ARMOUR) => "kh3_camp.equip.detail_0002",
            Some(KIND_ACCESSORY) => "kh3_camp.equip.detail_0003",
            Some(_) => "kh3_camp.equip.detail_0004",
            None => "",
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Items (`UTresUIP_CampMenuBackyard`).

/// Item categories of the tab table (`0x146A8E240`: weapon 3, armour 4, accessory 5, item 1).
pub const CAT_ITEM: u8 = 1;
pub const CAT_WEAPON: u8 = 3;
pub const CAT_ARMOUR: u8 = 4;
pub const CAT_ACCESSORY: u8 = 5;
pub const CAT_MATERIAL: u8 = 6;
pub const CAT_KEY: u8 = 7;

/// The tabs (`kh3_camp.item.tab_*`): text key and the category shown (`None` = all).
/// (IMPL_ER23B) THE ORDER IS THE GAME'S: the static table the Items screen builds its tabs from (initialiser
/// 0x14010F4B0: seven rows of text key, icon index, number; read by `UTresUIP_CampMenuBackyard` 0x1415C1950) lists
/// tab_0001, 0002, 0003, 0007, 0008, 0005, 0006 [decompiled C only].
pub const ITEM_TABS: [(&str, Option<u8>); 7] = [
    ("kh3_camp.item.tab_0001", None),
    ("kh3_camp.item.tab_0002", Some(CAT_ITEM)),
    ("kh3_camp.item.tab_0003", Some(CAT_WEAPON)),
    ("kh3_camp.item.tab_0007", Some(CAT_ARMOUR)),
    ("kh3_camp.item.tab_0008", Some(CAT_ACCESSORY)),
    ("kh3_camp.item.tab_0005", Some(CAT_MATERIAL)),
    ("kh3_camp.item.tab_0006", Some(CAT_KEY)),
];
/// The same table's icon indices (cells of `ItemListLibrary_bmpIcon`, three a row): cell 5 is not used here.
pub const ITEM_TAB_ICONS: [i32; 7] = [0, 1, 2, 3, 4, 6, 7];

/// One inventory entry.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct InvEntry {
    pub id: String,
    pub name: String,
    pub category: u8,
    pub count: u32,
    /// Category 1 or 2 and not id 0x10A: decide opens the target mode.
    pub usable: bool,
    /// The benefit test of a use on Sora right now (the host's: it has his status).
    pub benefit: bool,
    pub help: String,
    /// (party) The benefit test per party slot (0 Sora, 1.. the friends); empty = `benefit` for Sora alone.
    pub benefits: Vec<bool>,
}

impl InvEntry {
    /// (party) The benefit test of a use on party slot `member` (`0x1415C3C30`, the host's answer).
    pub fn benefit_for(&self, member: usize) -> bool {
        if member == 0 && self.benefits.is_empty() {
            self.benefit
        } else {
            self.benefits.get(member).copied().unwrap_or(false)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct ItemsScreen {
    pub entries: Vec<InvEntry>,
    pub tab: usize,
    pub list: ListView,
    /// Target mode: the party buttons are up (`m_TargetButtons`, one per party slot).
    pub target: bool,
    /// The last tab change went right (for the switch arrow that flashes).
    pub tab_flash: Option<bool>,
    /// (party) The party buttons of the target mode: how many (Sora + friends) and the one focused.
    pub party_size: usize,
    pub target_cursor: usize,
}

impl ItemsScreen {
    pub fn new() -> ItemsScreen {
        ItemsScreen { list: ListView::new("backyard.backyardAnm.backyardItem.backyardList", "CoBackyardListItem"), ..ItemsScreen::default() }
    }

    /// The rows of the selected tab.
    pub fn rows(&self) -> Vec<&InvEntry> {
        let filter = ITEM_TABS[self.tab.min(ITEM_TABS.len() - 1)].1;
        self.entries.iter().filter(|e| e.count > 0 && filter.is_none_or(|c| e.category == c)).collect()
    }

    pub fn set(&mut self, entries: Vec<InvEntry>) {
        self.entries = entries;
        let count = self.rows().len();
        self.list.set_count(count);
        self.list.invalidate();
        if self.target && self.focused().is_none() {
            self.target = false;
        }
    }

    pub fn focused(&self) -> Option<&InvEntry> {
        usize::try_from(self.list.focus).ok().and_then(|i| self.rows().get(i).copied())
    }

    /// One event; true = leave the screen.
    pub fn key(&mut self, key: CampKey, repeat: bool, sounds: &mut Vec<UiSound>, actions: &mut Vec<CampAction>, dialog: &mut Option<Dialog>) -> bool {
        if self.target {
            match key {
                // (party) the party buttons: up / down walk them (the anchors' chain, [?] wrap as the top list)
                CampKey::Up | CampKey::Down if self.party_size > 1 => {
                    if let Some(to) = crate::camp::move_wrapping(self.target_cursor, self.party_size, key == CampKey::Down, repeat) {
                        self.target_cursor = to;
                        sounds.push(UiSound::Play(SE_MENU_CURSOR));
                    }
                }
                CampKey::Decide => {
                    let Some(entry) = self.focused().cloned() else { return false };
                    if entry.count == 0 {
                        sounds.push(UiSound::Play(SE_MENU_REFUSED));
                    } else if !entry.benefit_for(self.target_cursor) {
                        *dialog = Some(Dialog::message("kh3_camp.item.dialog_0002"));
                    } else {
                        sounds.push(UiSound::Play(SE_ITEM_USED));
                        actions.push(CampAction::UseItem { id: entry.id });
                    }
                }
                CampKey::Cancel => {
                    sounds.push(UiSound::Play(SE_MENU_CLOSE));
                    self.target = false;
                }
                _ => {}
            }
            return false;
        }
        match key {
            CampKey::Up | CampKey::Down => {
                if self.list.move_anchor(key == CampKey::Down, repeat) {
                    sounds.push(UiSound::Play(SE_MENU_CURSOR));
                }
            }
            CampKey::L1 | CampKey::R1 => {
                let moved = if key == CampKey::R1 { self.list.next_page(repeat) } else { self.list.prev_page(repeat) };
                if moved {
                    sounds.push(UiSound::Play(SE_MENU_CURSOR));
                }
            }
            // Tabs (events 11 / 12): `prevTab` / `nextTab` with `enableLoop`; static SE 2.
            CampKey::L2 | CampKey::R2 => {
                let n = ITEM_TABS.len();
                let at_end = if key == CampKey::R2 { self.tab + 1 >= n } else { self.tab == 0 };
                if at_end && repeat {
                    return false;
                }
                self.tab = if key == CampKey::R2 { (self.tab + 1) % n } else { (self.tab + n - 1) % n };
                self.tab_flash = Some(key == CampKey::R2);
                sounds.push(UiSound::Play(SE_MENU_OPEN));
                self.list.focus = 0;
                self.list.scroll = 0;
                let count = self.rows().len();
                self.list.set_count(count);
                self.list.invalidate();
            }
            CampKey::Decide => match self.focused() {
                Some(entry) if entry.usable => {
                    sounds.push(UiSound::Play(SE_MENU_DECIDE));
                    self.target = true;
                    self.target_cursor = 0;
                }
                _ => sounds.push(UiSound::Play(SE_MENU_REFUSED)),
            },
            // Discard, after `dialog_0001`.
            CampKey::Triangle => {
                if let Some(entry) = self.focused().cloned() {
                    *dialog = Some(Dialog { text_key: "kh3_camp.item.dialog_0001", argument: entry.name, kind: DialogKind::YesNo, cursor: 1, pending: Pending::Discard(entry.id) });
                }
            }
            CampKey::Cancel => return true,
            _ => {}
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn piece(id: &str, kind: u8) -> Piece {
        Piece { id: id.into(), name: id.into(), kind, ..Piece::default() }
    }

    fn stats() -> StatsIn {
        StatsIn { name: "Sora".into(), level: 99, hp: 265, hp_max: 265, mp: 169, mp_max: 169, ap_max: 180, strength: 66, magic: 67, defense: 59, ..StatsIn::default() }
    }

    /// The level-99 loadout: 3 + 3 + 3 + 8 rows.
    fn loadout() -> EquipIn {
        let mut slots = Vec::new();
        for slot in 0..3 {
            slots.push(EquipSlot { kind: KIND_WEAPON, slot, piece: (slot == 0).then(|| piece("Kingdom Key", KIND_WEAPON)), current: slot == 0, ..EquipSlot::default() });
        }
        for kind in [KIND_ARMOUR, KIND_ACCESSORY] {
            for slot in 0..3 {
                slots.push(EquipSlot { kind, slot, ..EquipSlot::default() });
            }
        }
        for (slot, item) in ["Potion", "Potion", "Hi-Potion", "Hi-Potion", "Ether", "Ether", "", ""].into_iter().enumerate() {
            slots.push(EquipSlot { kind: KIND_ITEM, slot, piece: (!item.is_empty()).then(|| piece(item, KIND_ITEM)), ..EquipSlot::default() });
        }
        EquipIn { slots, stock: Vec::new(), ap_in_use: 180, stock_max: 99 }
    }

    #[test]
    fn spec_11b_test_13_equipment() {
        let (mut sounds, mut actions, mut dialog) = (Vec::new(), Vec::new(), None);
        let mut screen = EquipScreen::new();
        screen.set(loadout());
        assert_eq!(screen.data.slots.len(), 17);
        // Square on the Kingdom Key row: "You cannot remove this equipment."
        screen.key(CampKey::Square, false, &stats(), &mut sounds, &mut actions, &mut dialog);
        assert_eq!(dialog.take().map(|d| d.text_key), Some("kh3_camp.equip.dialog_0001"));
        assert!(actions.is_empty());
        // Decide on an empty armour row: the stock list, empty ("No items.").
        screen.list.set_focus(3);
        screen.key(CampKey::Decide, false, &stats(), &mut sounds, &mut actions, &mut dialog);
        assert_eq!((screen.layer, screen.stock_rows.len()), (EquipLayer::Stock, 0));
        screen.key(CampKey::Cancel, false, &stats(), &mut sounds, &mut actions, &mut dialog);
        // With a test armour of Defense +2 in stock: the preview shows 59 -> 61, max HP unchanged.
        let mut data = loadout();
        data.stock.push(StockEntry { piece: Piece { defense: 2, resist: [20, 0, 0, 0, 0, 0], ..piece("Test Belt", KIND_ARMOUR) }, count: 1 });
        screen.set(data);
        screen.key(CampKey::Decide, false, &stats(), &mut sounds, &mut actions, &mut dialog);
        let preview = screen.preview(&stats());
        assert_eq!((preview.now[5], preview.new[5], preview.now[0], preview.new[0], preview.new[6]), (59, 61, 265, 265, 20));
        sounds.clear();
        screen.key(CampKey::Decide, false, &stats(), &mut sounds, &mut actions, &mut dialog);
        assert_eq!(sounds, vec![UiSound::Play(SE_EQUIP)]);
        assert_eq!(actions, vec![CampAction::Equip { kind: KIND_ARMOUR, slot: 0, id: Some("Test Belt".into()) }]);
        assert_eq!(screen.layer, EquipLayer::Slots);
        // Removing an accessory that gives AP +5 (max AP 185): allowed with 180 in use, refused with 181.
        let mut data = loadout();
        data.slots[6].piece = Some(Piece { ap: 5, ..piece("AP Ring", KIND_ACCESSORY) });
        let with_ring = StatsIn { ap_max: 185, ..stats() };
        screen.set(data.clone());
        screen.list.set_focus(6);
        actions.clear();
        screen.key(CampKey::Square, false, &with_ring, &mut sounds, &mut actions, &mut dialog);
        assert_eq!(actions, vec![CampAction::Equip { kind: KIND_ACCESSORY, slot: 0, id: None }]);
        data.ap_in_use = 181;
        screen.set(data);
        actions.clear();
        screen.key(CampKey::Square, false, &with_ring, &mut sounds, &mut actions, &mut dialog);
        assert!(actions.is_empty());
        assert_eq!(dialog.take().map(|d| d.text_key), Some("kh3_camp.equip.dialog_0003"));
        // Triangle on a Potion row: auto-reload for that item, static SE 1; the guide switches.
        screen.set(loadout());
        screen.list.set_focus(9);
        assert_eq!(screen.guide_key(), "kh3_camp.equip.button_0015");
        sounds.clear();
        screen.key(CampKey::Triangle, false, &stats(), &mut sounds, &mut actions, &mut dialog);
        assert_eq!((sounds.clone(), actions.last().cloned()), (vec![UiSound::Play(1)], Some(CampAction::AutoReload { item: "Potion".into(), on: true })));
        let mut data = loadout();
        data.slots[9].auto_reload = true;
        data.slots[10].auto_reload = true;
        screen.set(data);
        assert_eq!(screen.guide_key(), "kh3_camp.equip.button_0016");
    }

    #[test]
    fn resist_rule_is_multiplicative() {
        let (a, b) = (Piece { resist: [20, 0, 0, 0, 0, 0], ..Piece::default() }, Piece { resist: [20, 0, 0, 0, 0, 0], ..Piece::default() });
        assert_eq!(resist_total(&[&a, &b], 0), 36);
        assert_eq!(resist_total(&[&a], 0), 20);
        assert_eq!(resist_total(&[], 0), 0);
    }

    #[test]
    fn spec_11b_test_14_items() {
        let potion = InvEntry { id: "Potion".into(), name: "Potion".into(), category: CAT_ITEM, count: 5, usable: true, benefit: true, help: String::new(), benefits: Vec::new() };
        let belt = InvEntry { id: "PRT".into(), name: "Belt".into(), category: CAT_ARMOUR, count: 1, ..InvEntry::default() };
        let (mut sounds, mut actions, mut dialog) = (Vec::new(), Vec::new(), None);
        let mut screen = ItemsScreen::new();
        screen.set(vec![potion.clone(), belt]);
        assert_eq!(screen.rows().len(), 2);
        // Tabs with L2 / R2: Items shows the potion only; wrapping on a fresh press.
        screen.key(CampKey::R2, false, &mut sounds, &mut actions, &mut dialog);
        assert_eq!((screen.tab, screen.rows().len()), (1, 1));
        screen.key(CampKey::L2, false, &mut sounds, &mut actions, &mut dialog);
        screen.key(CampKey::L2, false, &mut sounds, &mut actions, &mut dialog);
        assert_eq!(screen.tab, 6);
        screen.key(CampKey::R2, false, &mut sounds, &mut actions, &mut dialog);
        assert_eq!(screen.tab, 0);
        // Decide on Potion: target mode; decide on Sora: static SE 30 and the use.
        sounds.clear();
        screen.key(CampKey::Decide, false, &mut sounds, &mut actions, &mut dialog);
        assert!(screen.target);
        screen.key(CampKey::Decide, false, &mut sounds, &mut actions, &mut dialog);
        assert_eq!(sounds, vec![UiSound::Play(1), UiSound::Play(SE_ITEM_USED)]);
        assert_eq!(actions, vec![CampAction::UseItem { id: "Potion".into() }]);
        // At full HP: "You cannot benefit from this item right now."
        screen.set(vec![InvEntry { benefit: false, count: 4, ..potion }]);
        actions.clear();
        screen.key(CampKey::Decide, false, &mut sounds, &mut actions, &mut dialog);
        assert!(actions.is_empty());
        assert_eq!(dialog.take().map(|d| d.text_key), Some("kh3_camp.item.dialog_0002"));
        // Cancel: out of target mode, then out of the screen.
        assert!(!screen.key(CampKey::Cancel, false, &mut sounds, &mut actions, &mut dialog));
        assert!(screen.key(CampKey::Cancel, false, &mut sounds, &mut actions, &mut dialog));
    }
}
