//! The camp menu's Equipment and Items screens (docs/sora-ui-spec.md 10.5), logic only: the
//! host supplies the slots, the stock and the inventory as plain data and applies the
//! [`crate::camp::CampAction`]s it gets back.
//!
//! WHAT THE PORT DOES NOT HAVE: the asset "loading" mark, and the parameter block of armour /
//! accessories in the status pane (`0x1415A9CE0` fills `parameterSet2..4` for them; only the
//! weapon set is drawn); those panes show the help text.
//!
//! (weapons P11, documentation/KH3_WEAPONS_PLAN.md 1.4) SORA'S THREE KEYBLADE SLOTS: the slots are
//! the core's (`kh_core::status::Keyblades`, P2), `current` marks the slot in hand (`record+6`);
//! the stock offers every owned Keyblade not in a slot (one of each). KH3's rules kept here:
//! Remove is refused for the CURRENT Keyblade only (`dialog_0001`; any other slot may be emptied,
//! so KH3 has no "slot 1 is never empty" rule, only "the Keyblade in hand stays"), a Keyblade in
//! a slot that is not the current one previews with Strength / Magic re-based on that slot's
//! Keyblade (`0x1415AA460`), and the status pane of a weapon is KH3's `parameterSet1`
//! (`0x1415A9CE0` -> `0x14154BB30`: Strength, Magic, up to three abilities; a Keyblade's values at
//! its Forge level, `0x14112EBD0`). A Keyblade can be neither discarded nor sold ([`can_discard`]).
//! The friends' weapon slot lists their own weapons (party-weapons stats, duplicates allowed).
//! BOOST ITEMS ([`BOOST_ITEMS`]) are used from Items with A alone: one use raises the WHOLE party (the user, 2026-11-03).

use crate::camp::{CampAction, CampKey, Dialog, DialogKind, Pending, StatsIn};
use crate::list::ListView;
use crate::menu::{SE_MENU_CLOSE, SE_MENU_CURSOR, SE_MENU_DECIDE, SE_MENU_OPEN, SE_MENU_REFUSED};
use crate::UiSound;

/// Slot kinds in the equipped list's order (row record +4).
pub const KIND_WEAPON: u8 = 0;
pub const KIND_ARMOUR: u8 = 1;
pub const KIND_ACCESSORY: u8 = 2;
pub const KIND_ITEM: u8 = 3;
/// (THE USER 2026-11-02: "In the equipment menu for Sora, make a new area for him to equip spirit ashes (4) and these
/// equipped spirit ashes are what appears in the Summon command menu.") THE SPIRIT-ASH AREA: four slots of their own kind,
/// after the item slots (KH3's list ends with the pouch; Link, the command the Summon row replaces, had no equip slot in
/// KH3, so the new area goes last and leaves KH3's order of the four kinds as it is). Sora's only (the friends' screens
/// never get these rows); the host fills them only when it has spirits for him.
pub const KIND_SPIRIT: u8 = 4;
/// The area's title row and help line: COMPOSED (KH3 has no such texts; worded as KH3's `kh3_camp.equip.group_*` and
/// `detail_000N` lines are).
pub const SPIRIT_GROUP: &str = "Spirit Ashes";
pub const SPIRIT_HELP: &str = "Select a spirit ash for this slot.";
/// The stock list's number column over the ashes: their MP cost instead of a stock count (every ash is owned once), so
/// the column is titled "MP" (COMPOSED; KH3's title there is "Stock").
pub const SPIRIT_COST_TITLE: &str = "MP";
/// The MP column of an ash that takes all his MP (the Mimic Tear): COMPOSED, the same word as kh-core
/// `summon::TEXT_ALL_MP` (kh-ui does not depend on kh-core).
pub const SPIRIT_COST_ALL: &str = "All";

/// (Summon) The row id of a spirit ash (an Elden Ring goods id, its level included): `SPIRIT_<goods>`.
pub fn spirit_id(goods: u32) -> String {
    format!("SPIRIT_{goods}")
}

/// (Summon) The goods id of a spirit-ash row id (`None`: not one).
pub fn spirit_goods(id: &str) -> Option<u32> {
    id.strip_prefix("SPIRIT_")?.parse().ok()
}

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
    /// (Summon) A spirit ash's MP cost (0: not an ash); shown in the stock list's number column.
    pub cost: i32,
    /// (THE USER 2026-11-03: "For the spirit ashes HUD picture, use the Elden Ring picture of the spirit ash.") Elden
    /// Ring's icon number of the piece (`EquipParamGoods.iconId`): the item pane shows Elden Ring's own picture through
    /// the shared route (`item_art::er_icon_key`, the pack's `er_icons`). `None`: the picture of the piece's id.
    pub icon: Option<u32>,
    /// (THE USER 2026-11-03: "mimic tear should make Sora instantly go into MP recharge") The ash takes all his MP (kh-core
    /// `summon::takes_all`): the MP column says [`SPIRIT_COST_ALL`] instead of `cost`.
    pub cost_all: bool,
    /// (weapons P11) A Keyblade's Keyblade Forge level (KH3's upgrade row the status pane reads, `0x14112EBD0`); `None`:
    /// not a Keyblade. `attack` / `magic` / `abilities` are then the values AT this level.
    pub level: Option<u8>,
    /// (weapons P11) One help line per entry of `abilities` (the ability's own KH3 help text, with kh-core
    /// `keyblade::help_note` appended where the ability has no effect in this world): the ability list's help line.
    pub ability_help: Vec<String>,
    /// (weapons P11) A Keyblade's shotlocks (KH3's names, `m_ShootflowAbilitys` -> `TresAbilityData.m_NameTextID`), for
    /// the status pane's text ([`Piece::pane_help`]).
    pub shotlocks: Vec<String>,
}

impl Piece {
    /// The picture id the item pane loads: Elden Ring's icon ([`Piece::icon`]) when the piece has one, else its own id.
    pub fn art_id(&self) -> String {
        self.icon.map_or_else(|| self.id.clone(), crate::item_art::er_icon_key)
    }

    /// (Summon) The stock list's MP column of an ash: its cost, or [`SPIRIT_COST_ALL`].
    pub fn cost_text(&self) -> String {
        if self.cost_all {
            SPIRIT_COST_ALL.to_string()
        } else {
            self.cost.to_string()
        }
    }

    /// (weapons P11) THE STATUS PANE'S TEXT of the piece: its help text (`base`: KH3's `kh3_item.weapon.detail_kb_NN`,
    /// whose second line already names the Keyblade's formchanges, "Formchanges: Second Form"), then for a Keyblade one
    /// line naming its shotlocks after KH3's own title `title` (`kh3_camp.ability.group_0050` "Shotlocks"), the names in
    /// KH3's strong colour as the formchange line has them. COMPOSED: KH3 lists the shotlocks on the Abilities screen, not
    /// in this pane; the line is added so the Equipment screen shows everything a Keyblade brings.
    pub fn pane_help(&self, base: &str, title: &str) -> String {
        if self.shotlocks.is_empty() {
            return base.to_string();
        }
        let names: Vec<String> = self.shotlocks.iter().map(|s| format!("<color(strong)>{s}<color>")).collect();
        let line = format!("{title}: {}", names.join(", "));
        if base.is_empty() {
            line
        } else {
            format!("{base}\n{line}")
        }
    }

    /// (weapons P11) The pane's parameter rows of a weapon (KH3's `parameterSet1`, `0x14154BB30`: Strength, Magic, then at
    /// most three of its abilities): `(strength, magic, level, abilities)`. `None` for anything else.
    pub fn weapon_rows(&self) -> Option<(i32, i32, Option<u8>, Vec<&str>)> {
        (self.kind == KIND_WEAPON).then(|| (self.attack, self.magic, self.level, self.abilities.iter().take(PANE_ABILITIES).map(String::as_str).collect()))
    }
}

/// (weapons P11) How many abilities KH3's weapon pane names (`0x14154BB30`: labels 4 .. 6; the rest are on the Triangle
/// ability list).
pub const PANE_ABILITIES: usize = 3;

/// (weapons P11) THE KEYBLADES' ITEM KEYS (`WEP_KEYBLADE_SO_00` ..): KH3 has one of each Keyblade, and a Keyblade can be
/// neither discarded nor sold (`TresItemData` rows of the Keyblades forbid it; the Items screen's test `0x141126C10`
/// shows `kh3_camp.item.dialog_0004` "You cannot discard this item.").
pub const KEYBLADE_KEY_PREFIX: &str = "WEP_KEYBLADE_";

/// (weapons P11) May the Items screen's Discard (Triangle) get rid of this entry? Not a Keyblade.
pub fn can_discard(id: &str) -> bool {
    !id.starts_with(KEYBLADE_KEY_PREFIX)
}

/// (weapons P11) THE BOOST ITEMS (KH3's camp items 0x202 .. 0x205: Strength / Magic / Defense / AP Boost; KH3's
/// Collector's Goals IC_46 / IC_47 / IC_44 / IC_45 give them): used from the Items screen with A (no target buttons: THE
/// USER 2026-11-03 chose "Whole party": one use raises Sora, Donald and Goofy by one for good). The index is the stat: 0 Strength, 1 Magic,
/// 2 Defense, 3 AP (kh-core `status::Boosts`' order).
pub const BOOST_ITEMS: [&str; 4] = ["ITEM_POWERUP", "ITEM_MAGICUP", "ITEM_GUARDUP", "ITEM_APUP"];

/// (weapons P11) A boost item's stat (index into [`BOOST_ITEMS`]).
pub fn boost_stat(id: &str) -> Option<usize> {
    BOOST_ITEMS.iter().position(|b| *b == id)
}

/// (weapons P11) Static SE of a boost used (spec 10.5: SE 31 for ids 0x202 - 0x205, 30 for the other items).
pub const SE_BOOST_USED: u32 = 31;

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
        // (Summon) THE DUPLICATE RULE: an ash sits in one slot only. KH3's own rule is that an equipped piece leaves the
        // stock; an ash is owned once, so an ash in ANOTHER spirit slot is not offered here (the player removes it there
        // first, as a KH3 accessory he owns once). The slot's own ash is row 0, as KH3's current piece.
        // (the host's stock is every ash he owns: the slot's own one is left out here too, it is row 0 already)
        let offered: Vec<StockRow> = self.data.stock.iter().filter(|e| e.piece.kind == slot.kind && e.count > 0 && !(slot.kind == KIND_SPIRIT && self.spirit_slot_of(&e.piece.id).is_some())).map(|e| StockRow { piece: e.piece.clone(), count: Some(e.count) }).collect();
        self.stock_rows.extend(offered);
        self.stock.set_count(self.stock_rows.len());
        self.stock.invalidate();
    }

    /// (Summon) The ash `id` is in a spirit slot other than `slot` (the same goods id, or the same ash at another level:
    /// an equipped slot keeps the ash, not its level: Elden Ring's one goods row per level, `base + level`, kh-core
    /// `command::summon::base_of`; kh-ui does not depend on kh-core, so the rule is repeated here).
    pub fn spirit_elsewhere(&self, slot: usize, id: &str) -> bool {
        self.spirit_slot_of(id).is_some_and(|s| s != slot)
    }

    /// (Summon) The spirit slot that holds the ash `id` (at any level), if one does.
    pub fn spirit_slot_of(&self, id: &str) -> Option<usize> {
        let base = |id: &str| spirit_goods(id).map(|g| g - g % 100);
        let wanted = base(id)?;
        self.data.slots.iter().find(|s| s.kind == KIND_SPIRIT && s.piece.as_ref().and_then(|p| base(&p.id)) == Some(wanted)).map(|s| s.slot)
    }

    /// `m_BackyardItemStatusB`: the character's values now and with the highlighted stock row
    /// in the slot. Max HP / MP never change; a Keyblade outside the current slot changes
    /// nothing.
    pub fn preview(&self, stats: &StatsIn) -> Preview {
        let now = [stats.hp_max, stats.mp_max, stats.ap_max, stats.strength, stats.magic, stats.defense, stats.resists[0], stats.resists[1], stats.resists[2], stats.resists[3], stats.resists[4], stats.resists[5]];
        let mut new = now;
        let (Some(slot), EquipLayer::Stock) = (self.slot(), self.layer) else { return Preview { now, new } };
        let Some(row) = usize::try_from(self.stock.focus).ok().and_then(|i| self.stock_rows.get(i)) else { return Preview { now, new } };
        // (Summon) a spirit ash changes no stat: the pane shows the values as they are
        if slot.kind == KIND_ITEM || slot.kind == KIND_SPIRIT {
            return Preview { now, new };
        }
        let none = Piece::default();
        let old = slot.piece.as_ref().unwrap_or(&none);
        // (weapons P11) A KEYBLADE SLOT THAT IS NOT THE CURRENT ONE (spec 10.5, `0x1415AA460`): Strength and Magic are first
        // RE-BASED on that slot's Keyblade (as if it were in hand: the current Keyblade's terms out, the slot's in), then
        // the highlighted Keyblade's difference to it is shown. The other values do not change (a Keyblade has no AP,
        // Defense or resist terms; an empty slot re-bases on no Keyblade).
        if slot.kind == KIND_WEAPON && !slot.current {
            let held = self.data.slots.iter().find(|s| s.kind == KIND_WEAPON && s.current).and_then(|s| s.piece.as_ref()).unwrap_or(&none);
            let mut base = now;
            base[3] += old.attack - held.attack;
            base[4] += old.magic - held.magic;
            let mut new = base;
            new[3] += row.piece.attack - old.attack;
            new[4] += row.piece.magic - old.magic;
            return Preview { now: base, new };
        }
        new[2] += row.piece.ap - old.ap;
        new[3] += row.piece.attack - old.attack;
        new[4] += row.piece.magic - old.magic;
        new[5] += row.piece.defense - old.defense;
        // The pieces that count: the current Keyblade, armour, accessories.
        let counts = |s: &&EquipSlot| s.kind != KIND_ITEM && s.kind != KIND_SPIRIT && (s.kind != KIND_WEAPON || s.current);
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
        // (Summon) a spirit ash has no AP and no stock limit (it is owned once): KH3's two refusals do not apply
        if slot.kind == KIND_SPIRIT {
            return Ok(());
        }
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
                    } else if slot.kind == KIND_SPIRIT && self.spirit_elsewhere(slot.slot, &row.piece.id) {
                        // (Summon) the duplicate rule (see `build_stock`): an ash already in another slot is refused with
                        // KH3's refused sound, should a stock ever offer one
                        sounds.push(UiSound::Play(SE_MENU_REFUSED));
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
            Some(KIND_SPIRIT) => "",
            Some(_) => "kh3_camp.equip.detail_0004",
            None => "",
        }
    }

    /// (Summon) A help line the pack has no key for: the spirit-ash slots' [`SPIRIT_HELP`] (COMPOSED). `None`: the text
    /// of [`EquipScreen::help_key`].
    pub fn local_help(&self) -> Option<&'static str> {
        (self.slot().map(|s| s.kind) == Some(KIND_SPIRIT)).then_some(SPIRIT_HELP)
    }

    /// (weapons P11) THE ABILITY LIST'S HELP LINE: the help of the ability under the cursor (its KH3 text with kh-core
    /// `keyblade::help_note` appended where it has no effect in this world, e.g. Harvester "(no effect in this world)").
    /// `None` outside the ability list, or when the host gave no help for that row.
    pub fn ability_help(&self) -> Option<&str> {
        if self.layer != EquipLayer::Abilities {
            return None;
        }
        let piece = self.highlighted()?;
        usize::try_from(self.abilities.focus).ok().and_then(|i| piece.ability_help.get(i)).map(String::as_str).filter(|h| !h.is_empty())
    }
}

/// (weapons P11) An ability's help line with kh-core's note (`keyblade::help_note`, "(no effect in this world)") after
/// KH3's text, on a line of its own.
pub fn with_note(help: &str, note: Option<&str>) -> String {
    match note {
        Some(note) if help.is_empty() => note.to_string(),
        Some(note) => format!("{help}\n{note}"),
        None => help.to_string(),
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
    /// (weapons P11) THE ITEM USED UP WHILE THE TARGET BUTTONS ARE UP (spec 10.5: "with no copies left static SE 4";
    /// the target pane's `lack` colour): KH3 keeps its row, at 0, until the target mode ends, so the cursor never
    /// slides onto the next item (a second press on the used-up Potion must not use the Hi-Potion below it).
    pub lack: Option<String>,
}

impl ItemsScreen {
    pub fn new() -> ItemsScreen {
        ItemsScreen { list: ListView::new("backyard.backyardAnm.backyardItem.backyardList", "CoBackyardListItem"), ..ItemsScreen::default() }
    }

    /// The rows of the selected tab.
    pub fn rows(&self) -> Vec<&InvEntry> {
        let filter = ITEM_TABS[self.tab.min(ITEM_TABS.len() - 1)].1;
        self.entries.iter().filter(|e| (e.count > 0 || (self.target && self.lack.as_deref() == Some(e.id.as_str()))) && filter.is_none_or(|c| e.category == c)).collect()
    }

    pub fn set(&mut self, entries: Vec<InvEntry>) {
        // (weapons P11) the target's item, if this answer has none of it left: kept at 0 in its place (see `lack`)
        let held = if self.target { self.focused().cloned().and_then(|e| self.entries.iter().position(|x| x.id == e.id).map(|at| (e, at))) } else { None };
        self.entries = entries;
        if let Some((entry, at)) = held.filter(|(e, _)| !self.entries.iter().any(|x| x.id == e.id && x.count > 0)) {
            self.entries.retain(|x| x.id != entry.id);
            let at = at.min(self.entries.len());
            self.lack = Some(entry.id.clone());
            self.entries.insert(at, InvEntry { count: 0, ..entry });
        }
        let count = self.rows().len();
        self.list.set_count(count);
        self.list.invalidate();
        if self.target && self.focused().is_none() {
            self.target = false;
        }
    }

    /// (weapons P11) The target mode ends: a used-up row kept at 0 goes.
    fn leave_target(&mut self) {
        self.target = false;
        if let Some(id) = self.lack.take() {
            self.entries.retain(|e| e.count > 0 || e.id != id);
            let count = self.rows().len();
            self.list.set_count(count);
            self.list.invalidate();
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
                    self.leave_target();
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
            CampKey::Decide => match self.focused().cloned() {
                // (weapons P11, THE USER 2026-11-03: "Whole party") a boost raises EVERY party member at once: no target
                // buttons, A uses one (KH3's static SE 31); refused when Sora's stat has no room left (the host's `benefit`)
                Some(entry) if entry.usable && boost_stat(&entry.id).is_some() => {
                    if entry.benefit {
                        self.target_cursor = 0;
                        sounds.push(UiSound::Play(SE_BOOST_USED));
                        actions.push(CampAction::UseItem { id: entry.id });
                    } else {
                        *dialog = Some(Dialog::message("kh3_camp.item.dialog_0003"));
                        sounds.push(UiSound::Play(SE_MENU_REFUSED));
                    }
                }
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
                    if can_discard(&entry.id) {
                        *dialog = Some(Dialog { text_key: "kh3_camp.item.dialog_0001", argument: entry.name, kind: DialogKind::YesNo, cursor: 1, pending: Pending::Discard(entry.id) });
                    } else {
                        // (weapons P11) KH3: a Keyblade cannot be discarded (`0x141126C10` -> `dialog_0004`)
                        *dialog = Some(Dialog::message("kh3_camp.item.dialog_0004"));
                        sounds.push(UiSound::Play(SE_MENU_REFUSED));
                    }
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

    /// (Summon) An ash row as the host makes it.
    fn ash(goods: u32, name: &str, cost: i32) -> Piece {
        Piece { id: spirit_id(goods), name: name.into(), kind: KIND_SPIRIT, cost, ..Piece::default() }
    }

    /// (Summon) The level-99 loadout with the four spirit-ash slots (slot 0 holds the Lone Wolf Ashes +3) and a stock of
    /// three ashes (one of them the equipped one, as `owned` lists every ash he has) beside a piece of armour.
    fn with_spirits() -> EquipIn {
        let mut data = loadout();
        for slot in 0..4 {
            data.slots.push(EquipSlot { kind: KIND_SPIRIT, slot, piece: (slot == 0).then(|| ash(200003, "Lone Wolf Ashes +3", 8)), ..EquipSlot::default() });
        }
        data.stock = vec![
            StockEntry { piece: piece("Test Belt", KIND_ARMOUR), count: 1 },
            StockEntry { piece: ash(200003, "Lone Wolf Ashes +3", 8), count: 1 },
            StockEntry { piece: ash(213000, "Jellyfish Ashes", 6), count: 1 },
            StockEntry { piece: ash(207000, "Mimic Tear Ashes", 99), count: 1 },
        ];
        data
    }

    #[test]
    fn summon_the_spirit_area_has_four_slots_that_list_only_ashes_and_equip_remove_them() {
        let (mut sounds, mut actions, mut dialog) = (Vec::new(), Vec::new(), None);
        let mut screen = EquipScreen::new();
        screen.set(with_spirits());
        let spirit_rows: Vec<usize> = screen.data.slots.iter().enumerate().filter(|(_, s)| s.kind == KIND_SPIRIT).map(|(i, _)| i).collect();
        assert_eq!(spirit_rows, vec![17, 18, 19, 20], "THE USER: four slots, after the item slots");
        assert_eq!((spirit_goods(&spirit_id(200003)), spirit_goods("Potion")), (Some(200003), None));
        // the second slot (empty): its stock is the ashes only, not the armour; the ash in slot 0 is not offered again
        screen.list.set_focus(18);
        assert_eq!(screen.local_help(), Some(SPIRIT_HELP));
        assert_eq!(screen.help_key(), "");
        screen.key(CampKey::Decide, false, &stats(), &mut sounds, &mut actions, &mut dialog);
        assert_eq!(screen.layer, EquipLayer::Stock);
        let names: Vec<&str> = screen.stock_rows.iter().map(|r| r.piece.name.as_str()).collect();
        assert_eq!(names, vec!["Jellyfish Ashes", "Mimic Tear Ashes"], "only ashes, and none that sits in another slot");
        // the preview: an ash changes no stat
        let preview = screen.preview(&stats());
        assert_eq!(preview.now, preview.new);
        // equip the Mimic Tear: KH3's equip sound, the host's action, back to the slots
        screen.key(CampKey::Down, false, &stats(), &mut sounds, &mut actions, &mut dialog);
        sounds.clear();
        screen.key(CampKey::Decide, false, &stats(), &mut sounds, &mut actions, &mut dialog);
        assert_eq!(sounds, vec![UiSound::Play(SE_EQUIP)]);
        assert_eq!(actions, vec![CampAction::Equip { kind: KIND_SPIRIT, slot: 1, id: Some(spirit_id(207000)) }]);
        assert_eq!(screen.layer, EquipLayer::Slots);
        // slot 0's own stock: its ash as row 0 (the equipped mark), then the others; Decide on row 0 changes nothing
        screen.list.set_focus(17);
        assert_eq!(screen.guide_key(), "kh3_camp.equip.button_0004", "KH3's Remove guide on a filled slot");
        screen.key(CampKey::Decide, false, &stats(), &mut sounds, &mut actions, &mut dialog);
        assert_eq!((screen.stock_rows[0].count, screen.stock_rows[0].piece.id.as_str()), (None, "SPIRIT_200003"));
        assert_eq!(screen.stock_rows.len(), 3);
        actions.clear();
        screen.key(CampKey::Decide, false, &stats(), &mut sounds, &mut actions, &mut dialog);
        assert!(actions.is_empty());
        // Square: KH3's Remove (static SE 3), no refusal (an ash has no AP)
        sounds.clear();
        screen.key(CampKey::Square, false, &stats(), &mut sounds, &mut actions, &mut dialog);
        assert_eq!((sounds.clone(), actions.clone(), dialog.is_none()), (vec![UiSound::Play(SE_MENU_CLOSE)], vec![CampAction::Equip { kind: KIND_SPIRIT, slot: 0, id: None }], true));
        // an empty slot: Square does nothing
        screen.list.set_focus(19);
        actions.clear();
        screen.key(CampKey::Square, false, &stats(), &mut sounds, &mut actions, &mut dialog);
        assert!(actions.is_empty());
    }

    #[test]
    fn summon_an_ash_shows_its_elden_ring_picture_through_the_shared_route() {
        // (THE USER 2026-11-03: "use the Elden Ring picture of the spirit ash")
        let with = Piece { icon: Some(117), ..ash(207000, "Mimic Tear Ashes", 99) };
        assert_eq!(with.art_id(), crate::item_art::er_icon_key(117));
        assert_eq!(ash(207000, "Mimic Tear Ashes", 99).art_id(), "SPIRIT_207000", "no icon known: the piece's own id");
        assert_eq!(piece("Potion", KIND_ITEM).art_id(), "Potion", "KH3's pieces as before");
        // (THE USER 2026-11-03: "mimic tear should make Sora instantly go into MP recharge") its MP column says "All"
        assert_eq!((Piece { cost_all: true, ..with.clone() }.cost_text(), with.cost_text()), ("All".to_string(), "99".to_string()));
    }

    #[test]
    fn summon_the_same_ash_never_sits_in_two_slots() {
        let (mut sounds, mut actions, mut dialog) = (Vec::new(), Vec::new(), None);
        let mut screen = EquipScreen::new();
        let mut data = with_spirits();
        // the same ash at another level (an upgrade at Roderika) counts as the same ash
        data.stock.push(StockEntry { piece: ash(200004, "Lone Wolf Ashes +4", 8), count: 1 });
        screen.set(data);
        assert!(screen.spirit_elsewhere(1, &spirit_id(200004)) && !screen.spirit_elsewhere(0, &spirit_id(200004)));
        screen.list.set_focus(20);
        screen.key(CampKey::Decide, false, &stats(), &mut sounds, &mut actions, &mut dialog);
        assert!(screen.stock_rows.iter().all(|r| !r.piece.id.starts_with("SPIRIT_2000")), "{:?}", screen.stock_rows);
        // a stock that offers it anyway (a host's list built before the change): refused with KH3's refused sound
        screen.stock_rows.push(StockRow { piece: ash(200003, "Lone Wolf Ashes +3", 8), count: Some(1) });
        screen.stock.set_count(screen.stock_rows.len());
        screen.stock.set_focus(screen.stock_rows.len() - 1);
        sounds.clear();
        screen.key(CampKey::Decide, false, &stats(), &mut sounds, &mut actions, &mut dialog);
        assert_eq!((sounds, actions.is_empty(), screen.layer), (vec![UiSound::Play(SE_MENU_REFUSED)], true, EquipLayer::Stock));
    }

    /// (weapons P11) A Keyblade piece as the host makes it (values at its Forge level).
    fn keyblade(id: &str, attack: i32, magic: i32, level: u8) -> Piece {
        Piece { id: id.into(), name: id.into(), kind: KIND_WEAPON, attack, magic, level: Some(level), abilities: vec!["Lucky Strike".into()], ability_help: vec!["Increases the chance of finding items.".into()], ..Piece::default() }
    }

    /// (weapons P11) Sora's three Keyblade slots with the SECOND one in hand (Hero's Origin 7 / 3), the Kingdom Key in
    /// slot 1 (5 / 4), slot 3 empty; the stock: Shooting Star (3 / 6).
    fn three_slots() -> EquipIn {
        let mut data = loadout();
        data.slots[0] = EquipSlot { kind: KIND_WEAPON, slot: 0, piece: Some(keyblade("WEP_KEYBLADE_SO_00", 5, 4, 2)), current: false, ..EquipSlot::default() };
        data.slots[1] = EquipSlot { kind: KIND_WEAPON, slot: 1, piece: Some(keyblade("WEP_KEYBLADE_SO_01", 7, 3, 0)), current: true, ..EquipSlot::default() };
        data.stock = vec![StockEntry { piece: keyblade("WEP_KEYBLADE_SO_02", 3, 6, 0), count: 1 }];
        data
    }

    #[test]
    fn p11_keyblade_slots_remove_only_off_the_current_slot_and_equip_from_the_stock() {
        let (mut sounds, mut actions, mut dialog) = (Vec::new(), Vec::new(), None);
        let mut screen = EquipScreen::new();
        screen.set(three_slots());
        // the Kingdom Key in slot 1 is NOT in hand: KH3 lets it be removed (the rule is "the current Keyblade", not "slot 1")
        screen.key(CampKey::Square, false, &stats(), &mut sounds, &mut actions, &mut dialog);
        assert_eq!((actions.clone(), dialog.is_none()), (vec![CampAction::Equip { kind: KIND_WEAPON, slot: 0, id: None }], true));
        // the Keyblade in hand: "You cannot remove this equipment."
        actions.clear();
        screen.list.set_focus(1);
        screen.key(CampKey::Square, false, &stats(), &mut sounds, &mut actions, &mut dialog);
        assert_eq!((dialog.take().map(|d| d.text_key), actions.is_empty()), (Some("kh3_camp.equip.dialog_0001"), true));
        // the empty third slot: Decide lists the stock's Keyblade; Decide equips it with KH3's equip sound
        screen.list.set_focus(2);
        screen.key(CampKey::Decide, false, &stats(), &mut sounds, &mut actions, &mut dialog);
        assert_eq!(screen.stock_rows.iter().map(|r| r.piece.id.as_str()).collect::<Vec<_>>(), vec!["WEP_KEYBLADE_SO_02"]);
        sounds.clear();
        screen.key(CampKey::Decide, false, &stats(), &mut sounds, &mut actions, &mut dialog);
        assert_eq!((sounds.clone(), actions.clone()), (vec![UiSound::Play(SE_EQUIP)], vec![CampAction::Equip { kind: KIND_WEAPON, slot: 2, id: Some("WEP_KEYBLADE_SO_02".into()) }]));
        // the guide of a Keyblade row: Abilities / Remove; Triangle opens its ability list, whose help line is the ability's
        screen.list.set_focus(1);
        assert_eq!(screen.guide_key(), "kh3_camp.equip.button_0014");
        screen.key(CampKey::Triangle, false, &stats(), &mut sounds, &mut actions, &mut dialog);
        assert_eq!((screen.layer, screen.ability_help()), (EquipLayer::Abilities, Some("Increases the chance of finding items.")));
        screen.key(CampKey::Cancel, false, &stats(), &mut sounds, &mut actions, &mut dialog);
        assert_eq!((screen.layer, screen.ability_help()), (EquipLayer::Slots, None));
    }

    #[test]
    fn p11_a_slot_out_of_hand_previews_rebased_on_its_own_keyblade() {
        let (mut sounds, mut actions, mut dialog) = (Vec::new(), Vec::new(), None);
        let mut screen = EquipScreen::new();
        screen.set(three_slots());
        // slot 1 (Kingdom Key 5 / 4, not in hand; Hero's Origin 7 / 3 is): Strength 66 -> 64 re-based, Shooting Star 3 / 6 -> 62
        screen.key(CampKey::Decide, false, &stats(), &mut sounds, &mut actions, &mut dialog);
        screen.key(CampKey::Down, false, &stats(), &mut sounds, &mut actions, &mut dialog);
        let p = screen.preview(&stats());
        assert_eq!((p.now[3], p.new[3], p.now[4], p.new[4]), (64, 62, 68, 70), "Strength 66 - 7 + 5, then - 5 + 3; Magic 67 - 3 + 4, then - 4 + 6");
        assert_eq!((p.now[2], p.new[2], p.now[5], p.new[5]), (180, 180, 59, 59), "no AP, no Defense");
        // the slot in hand: the plain difference (Hero's Origin 7 / 3 -> Shooting Star 3 / 6)
        screen.key(CampKey::Cancel, false, &stats(), &mut sounds, &mut actions, &mut dialog);
        screen.list.set_focus(1);
        screen.key(CampKey::Decide, false, &stats(), &mut sounds, &mut actions, &mut dialog);
        screen.key(CampKey::Down, false, &stats(), &mut sounds, &mut actions, &mut dialog);
        let p = screen.preview(&stats());
        assert_eq!((p.now[3], p.new[3], p.now[4], p.new[4]), (66, 62, 67, 70));
    }

    #[test]
    fn p11_the_weapon_pane_rows_and_the_shotlock_line() {
        let mut kb = keyblade("WEP_KEYBLADE_SO_07", 9, 9, 3);
        kb.abilities = vec!["Harvester".into(), "Form Extender".into(), "MP Haste".into(), "Lucky Strike".into()];
        let (s, m, level, abilities) = kb.weapon_rows().unwrap();
        assert_eq!((s, m, level, abilities), (9, 9, Some(3), vec!["Harvester", "Form Extender", "MP Haste"]), "KH3's three ability labels");
        assert_eq!(piece("Belt", KIND_ARMOUR).weapon_rows(), None);
        assert_eq!(kb.pane_help("A well-balanced Keyblade.", "Shotlocks"), "A well-balanced Keyblade.", "no shotlocks: KH3's text as it is");
        kb.shotlocks = vec!["Hunny Burst".into(), "Hunny Drizzle".into()];
        assert_eq!(kb.pane_help("A well-balanced Keyblade.", "Shotlocks"), "A well-balanced Keyblade.\nShotlocks: <color(strong)>Hunny Burst<color>, <color(strong)>Hunny Drizzle<color>");
        assert_eq!(with_note("Pick up one extra cooking ingredient each time you gather.", Some("(no effect in this world)")), "Pick up one extra cooking ingredient each time you gather.\n(no effect in this world)");
        assert_eq!(with_note("Text.", None), "Text.");
    }

    #[test]
    fn p11_a_keyblade_cannot_be_discarded_and_a_boost_is_used_with_a_alone() {
        let (mut sounds, mut actions, mut dialog) = (Vec::new(), Vec::new(), None);
        let mut screen = ItemsScreen::new();
        let kb = InvEntry { id: "WEP_KEYBLADE_SO_01".into(), name: "Hero's Origin".into(), category: CAT_WEAPON, count: 1, ..InvEntry::default() };
        let staff = InvEntry { id: "WEP_DONALD_02".into(), name: "Warhammer".into(), category: CAT_WEAPON, count: 2, ..InvEntry::default() };
        let boost = InvEntry { id: "ITEM_POWERUP".into(), name: "Strength Boost".into(), category: CAT_ITEM, count: 2, usable: true, benefit: true, benefits: vec![true, true], ..InvEntry::default() };
        screen.set(vec![kb, staff, boost]);
        screen.key(CampKey::Triangle, false, &mut sounds, &mut actions, &mut dialog);
        assert_eq!((dialog.take().map(|d| (d.text_key, d.pending)), sounds.last()), (Some(("kh3_camp.item.dialog_0004", Pending::None)), Some(&UiSound::Play(SE_MENU_REFUSED))));
        // a friend's weapon can go (KH3 sells / discards staves and shields)
        screen.key(CampKey::Down, false, &mut sounds, &mut actions, &mut dialog);
        screen.key(CampKey::Triangle, false, &mut sounds, &mut actions, &mut dialog);
        assert_eq!(dialog.take().map(|d| d.pending), Some(Pending::Discard("WEP_DONALD_02".into())));
        assert!(!can_discard("WEP_KEYBLADE_SO_14") && can_discard("WEP_GOOFY_00") && can_discard("ITEM_POWERUP"));
        // the boost: no target buttons (THE USER: "Whole party"), A uses one at once with SE 31
        screen.key(CampKey::Down, false, &mut sounds, &mut actions, &mut dialog);
        screen.party_size = 2;
        screen.target_cursor = 1; // a cursor left on a friend by an earlier Potion: a boost does not care
        sounds.clear();
        screen.key(CampKey::Decide, false, &mut sounds, &mut actions, &mut dialog);
        assert_eq!((screen.target, screen.target_cursor, sounds.clone(), actions.last().cloned()), (false, 0, vec![UiSound::Play(SE_BOOST_USED)], Some(CampAction::UseItem { id: "ITEM_POWERUP".into() })));
        assert_eq!((boost_stat("ITEM_POWERUP"), boost_stat("ITEM_MAGICUP"), boost_stat("ITEM_GUARDUP"), boost_stat("ITEM_APUP"), boost_stat("Potion")), (Some(0), Some(1), Some(2), Some(3), None));
        // a stat with no room left (the host's benefit false): KH3's "You cannot use any more of this item."
        let full = InvEntry { id: "ITEM_APUP".into(), name: "AP Boost".into(), category: CAT_ITEM, count: 1, usable: true, benefit: false, ..InvEntry::default() };
        screen.set(vec![full]);
        actions.clear();
        screen.key(CampKey::Decide, false, &mut sounds, &mut actions, &mut dialog);
        assert_eq!((actions.is_empty(), dialog.take().map(|d| d.text_key)), (true, Some("kh3_camp.item.dialog_0003")));
    }

    /// (weapons P11) KH3's "lack" rule for the items that still use the target buttons: the last Potion used on a member
    /// keeps its row at 0 while the buttons are up (no slide onto the Hi-Potion below); A is refused; B ends it.
    #[test]
    fn p11_a_used_up_item_keeps_its_row_under_the_target_buttons() {
        let (mut sounds, mut actions, mut dialog) = (Vec::new(), Vec::new(), None);
        let mut screen = ItemsScreen::new();
        let potion = InvEntry { id: "Potion".into(), name: "Potion".into(), category: CAT_ITEM, count: 1, usable: true, benefit: true, benefits: vec![true, true], ..InvEntry::default() };
        let hi = InvEntry { id: "HiPotion".into(), name: "Hi-Potion".into(), count: 2, ..potion.clone() };
        screen.set(vec![potion, hi.clone()]);
        screen.party_size = 2;
        screen.key(CampKey::Decide, false, &mut sounds, &mut actions, &mut dialog);
        screen.key(CampKey::Decide, false, &mut sounds, &mut actions, &mut dialog);
        assert_eq!(actions, vec![CampAction::UseItem { id: "Potion".into() }]);
        screen.set(vec![hi.clone()]);
        assert_eq!((screen.target, screen.focused().map(|e| (e.id.as_str(), e.count))), (true, Some(("Potion", 0))));
        actions.clear();
        sounds.clear();
        screen.key(CampKey::Decide, false, &mut sounds, &mut actions, &mut dialog);
        assert_eq!((actions.is_empty(), sounds.clone()), (true, vec![UiSound::Play(SE_MENU_REFUSED)]));
        screen.set(vec![hi]);
        assert_eq!(screen.focused().map(|e| e.id.as_str()), Some("Potion"), "a second answer keeps it too");
        screen.key(CampKey::Cancel, false, &mut sounds, &mut actions, &mut dialog);
        assert_eq!((screen.target, screen.lack.clone(), screen.rows().iter().map(|e| e.id.as_str()).collect::<Vec<_>>()), (false, None, vec!["HiPotion"]));
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
