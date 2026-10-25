//! (loot) KH3'S ITEMS A SORA CAN HOLD, for both hosts: the table (sheets/port/sora-item-table.json, written by
//! tools/sora_item_table.js from the game's `TresItemData`: battle and camp items, armour, accessories, synthesis
//! materials) and THE ONE DOOR through which anything outside the HUD hands him an item: [`give`]. A prize box that
//! arrives (`crate::prize::PrizeHost`), a host's reward, the Elden Ring loot gate all call it; the HUD takes the
//! queue at its next frame (`Hud::frame`): a battle item goes into the pouch's stock, everything else into the camp
//! menu's stock (which the save keeps), and the game's item popup (`HudLeftGetItem`) names it, in its stock-full
//! variant when the stock already held the item's maximum.
//!
//! WHERE THINGS ARE KEPT: battle items the core knows (`kh_core::status::ItemKind`) in `Pouch::stock`; armour and
//! accessories in `Inventory::equipment` under their equipment row id (PRT_ITEM01 ...); MATERIALS AND CAMP ITEMS in
//! the same list under their own key (MAT_ITEM01, ITEM_POWERUP ...). Nothing uses a material or a camp item yet
//! (no synthesis, no boost): they are shown on the Items screen and kept.

use std::sync::{Mutex, OnceLock};

/// The sheet itself (the hosts carry it in the binary; a rebuilt sheet needs a rebuilt host).
pub const ITEM_TABLE_SHEET: &str = include_str!("../../../sheets/port/sora-item-table.json");

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Class {
    Battle,
    Camp,
    Armour,
    Accessory,
    Material,
}

/// One row of the game's item table.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    /// `m_KeyName`: ITEM_POTION, PRT_ITEM01, ACC_ITEM01, MAT_ITEM01. For armour and accessories the id of the
    /// equipment row.
    pub key: String,
    /// Category x 256 + index (0x101 Potion, 0x701 Blazing Shard): what a prize box holds.
    pub item_id: u32,
    pub class: Class,
    /// `m_ItemRank` as a grade: 0 none, 1 Shard, 2 Stone, 3 Gem, 4 Crystal and the ores.
    pub grade: u8,
    pub name: String,
    /// Materials: the name without its grade word (Blazing, Mythril ...).
    pub family: String,
    pub buy: u32,
    pub sell: u32,
    /// `m_MaxNum`.
    pub max: u32,
}

static TABLE: OnceLock<Vec<Item>> = OnceLock::new();

fn parse(text: &str) -> Vec<Item> {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else { return Vec::new() };
    let column = |name: &str| json["columns"].as_array().and_then(|c| c.iter().position(|v| v.as_str() == Some(name)));
    let (Some(id), Some(item_id), Some(kind), Some(grade), Some(name), Some(family), Some(buy), Some(sell), Some(max)) =
        (column("id"), column("item_id"), column("kind"), column("grade"), column("name"), column("family"), column("buy"), column("sell"), column("max"))
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for row in json["rows"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
        let class = match row[kind].as_str() {
            Some("battle") => Class::Battle,
            Some("camp") => Class::Camp,
            Some("armour") => Class::Armour,
            Some("accessory") => Class::Accessory,
            Some("material") => Class::Material,
            _ => continue,
        };
        let (Some(key), Some(item)) = (row[id].as_str(), row[item_id].as_u64()) else { continue };
        let number = |k: usize| row[k].as_u64().unwrap_or(0) as u32;
        out.push(Item { key: key.to_string(), item_id: item as u32, class, grade: number(grade) as u8, name: row[name].as_str().unwrap_or(key).to_string(), family: row[family].as_str().unwrap_or("-").to_string(), buy: number(buy), sell: number(sell), max: number(max).max(1) });
    }
    out
}

/// Every item, in the table's order.
pub fn table() -> &'static [Item] {
    TABLE.get_or_init(|| parse(ITEM_TABLE_SHEET))
}

pub fn by_id(item_id: u32) -> Option<&'static Item> {
    table().iter().find(|i| i.item_id == item_id)
}

pub fn by_key(key: &str) -> Option<&'static Item> {
    table().iter().find(|i| i.key.eq_ignore_ascii_case(key))
}

/// By key, by English name (the first row of that name: several accessories share one), or by item id ("0x701", "1793").
pub fn find(what: &str) -> Option<&'static Item> {
    let what = what.trim();
    let wanted = what.replace('_', " ");
    let id = what.strip_prefix("0x").or_else(|| what.strip_prefix("0X")).map_or_else(|| what.parse::<u32>().ok(), |hex| u32::from_str_radix(hex, 16).ok());
    by_key(what).or_else(|| table().iter().find(|i| i.name.eq_ignore_ascii_case(&wanted))).or_else(|| id.and_then(by_id))
}

/// The battle item of the core with this item id.
pub fn battle_kind(item_id: u32) -> Option<kh_core::status::ItemKind> {
    kh_core::status::ItemKind::ALL.into_iter().find(|k| *k as u32 == item_id)
}

/// (ER party S6, U4) THE FRIENDS' WEAPONS AS ITEMS: Donald's 14 staves and Goofy's 14 shields (TresItemData rows
/// I03021..I03034 / I03041..I03054, category 3 = weapons: item id 0x300 + the row's index, as the table's other rows:
/// I04005 = 0x405). They are not rows of the item table (it holds no weapons); a host hands one out through [`give`]
/// like any item, and the HUD keeps it in the camp menu's stock under its equip key (`WEP_DONALD_03`), where the
/// friend's Equipment screen lists it (kh-host hud.rs `friend_equip_in`).
pub fn party_weapon_key(item_id: u32) -> Option<String> {
    let index = item_id.checked_sub(0x300)?;
    match index {
        21..=34 => Some(format!("WEP_DONALD_{:02}", index - 21)),
        41..=54 => Some(format!("WEP_GOOFY_{:02}", index - 41)),
        _ => None,
    }
}

/// (ER party S6) The item id of a friend's weapon by its equip key (`WEP_GOOFY_05` -> 0x32E).
pub fn party_weapon_id(key: &str) -> Option<u32> {
    let (base, n) = if let Some(n) = key.strip_prefix("WEP_DONALD_") { (21, n) } else { (41, key.strip_prefix("WEP_GOOFY_")?) };
    let n: u32 = n.parse().ok().filter(|n| *n <= 13)?;
    Some(0x300 + base + n)
}

static PENDING: Mutex<Vec<(u32, u32)>> = Mutex::new(Vec::new());
/// (PU) The tests that read the door's queue ([`take_given`]: one for the whole test binary) hold this lock.
#[cfg(test)]
pub(crate) static TEST_DOOR: Mutex<()> = Mutex::new(());
/// More than this waiting means no HUD is taking them: the oldest are dropped (a host without a HUD).
const PENDING_MAX: usize = 256;

/// HANDS SORA `count` OF AN ITEM (by item id). Taken by the HUD at its next frame. False: no such item in the table
/// (nothing is queued).
pub fn give(item_id: u32, count: u32) -> bool {
    if count == 0 || (by_id(item_id).is_none() && party_weapon_key(item_id).is_none() /* (ER party S6) */) {
        return false;
    }
    if let Ok(mut pending) = PENDING.lock() {
        if pending.len() >= PENDING_MAX {
            pending.remove(0);
        }
        pending.push((item_id, count));
    }
    true
}

/// (SHOP-3) Copies to take back out of Sora's stock (item id, count): the HUD's `apply_actions` takes them (as far as
/// he holds them). A host's repair: a KH item that stood in for an Elden Ring item that is given back.
static TAKE: Mutex<Vec<(u32, u32)>> = Mutex::new(Vec::new());

pub fn take(item_id: u32, count: u32) -> bool {
    if count == 0 || by_id(item_id).is_none() {
        return false;
    }
    TAKE.lock().map(|mut t| t.push((item_id, count))).is_ok()
}

pub fn take_requests() -> Vec<(u32, u32)> {
    TAKE.lock().map(|mut t| std::mem::take(&mut *t)).unwrap_or_default()
}

/// What [`give`] queued since the last call (the HUD's; a test's).
pub fn take_given() -> Vec<(u32, u32)> {
    PENDING.lock().map(|mut p| std::mem::take(&mut *p)).unwrap_or_default()
}

/// (IMPL_ER28) WHAT SORA HOLDS, as the HUD last told it (`Hud::frame`): KH item key -> copies in the camp menu's
/// stock plus worn; "@acc_slots" / "@def_slots" -> his accessory / armour slot bonuses. A host's item rules ask it
/// through [`held`] (the Elden Ring loot's "never more than two of one piece").
static HELD: Mutex<Vec<(String, u32)>> = Mutex::new(Vec::new());

pub fn publish_held(held: Vec<(String, u32)>) {
    if let Ok(mut h) = HELD.lock() {
        *h = held;
    }
}

/// Copies of an item he holds: the HUD's last count plus what [`give`] queued and the HUD has not taken yet.
pub fn held(key: &str) -> u32 {
    let known = HELD.lock().map_or(0, |h| h.iter().filter(|e| e.0 == key).map(|e| e.1).sum::<u32>());
    let waiting = PENDING.lock().map_or(0, |p| p.iter().filter(|e| by_id(e.0).is_some_and(|i| i.key == key) || party_weapon_key(e.0).is_some_and(|k| k == key) /* (ER party S6) */).map(|e| e.1).sum::<u32>());
    known + waiting
}

/// (IMPL_ER28) THE HOST'S KEY ITEMS for the camp menu's Items screen, tab "Key Items" (KH3's own tab; read-only
/// rows). Elden Ring's key items are not KH3 items and are never in Sora's stock: the host lists
/// them here and the screen shows them.
static KEY_ITEMS: Mutex<Vec<KeyItem>> = Mutex::new(Vec::new());

/// (IMPL_ER28, IMPL_ER42) One of the host's key items: its name, how many, and (IMPL_ER42) its description for the item
/// pane (the host's own text, markup already removed; empty = none) and the host's icon number (Elden Ring's
/// `EquipParamGoods.iconId`: (IMPL_ER43) the pane shows Elden Ring's picture of it from the UI pack, `kh_ui::item_art`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct KeyItem {
    pub name: String,
    pub count: u32,
    pub help: String,
    pub icon_id: Option<u32>,
}

impl KeyItem {
    pub fn new(name: impl Into<String>, count: u32) -> KeyItem {
        KeyItem { name: name.into(), count, ..KeyItem::default() }
    }
}

pub fn set_key_items(list: Vec<KeyItem>) {
    if let Ok(mut k) = KEY_ITEMS.lock() {
        *k = list;
    }
}

pub fn key_items() -> Vec<KeyItem> {
    KEY_ITEMS.lock().map(|k| k.clone()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (IMPL_ER28) What he holds = the HUD's count + what is still queued; the host's key items are kept as given.
    #[test]
    fn held_counts_the_stock_and_the_queue() {
        // (the door's queue is shared by the tests of this file: only keys no other test gives are counted)
        let _door = TEST_DOOR.lock().unwrap_or_else(|e| e.into_inner()); // (PU)
        publish_held(vec![("ACC_ITEM13".to_string(), 1), ("@acc_slots".to_string(), 2)]);
        assert_eq!((held("ACC_ITEM13"), held("@acc_slots"), held("ACC_ITEM14")), (1, 2, 0));
        assert!(give(by_key("ACC_ITEM13").unwrap().item_id, 1));
        assert_eq!(held("ACC_ITEM13"), 2);
        take_given();
        publish_held(Vec::new());
        assert_eq!(held("ACC_ITEM13"), 0);
        let key = KeyItem { help: "Used to break imp statue seals.".to_string(), icon_id: Some(3080), ..KeyItem::new("Stonesword Key", 3) };
        set_key_items(vec![key.clone()]);
        assert_eq!(key_items(), vec![key]);
        set_key_items(Vec::new());
    }

    /// The table is the game's: 11 battle items, 5 camp items, 52 armour pieces, 112 accessories, 60 materials
    /// (13 families in four grades + 8 single ones); every armour and accessory row is an equipment row of the core;
    /// every battle item of the core is in it under its item id.
    #[test]
    fn the_item_table_is_the_games() {
        let count = |class: Class| table().iter().filter(|i| i.class == class).count();
        assert_eq!((count(Class::Battle), count(Class::Camp), count(Class::Armour), count(Class::Accessory), count(Class::Material)), (11, 5, 52, 112, 60));
        for item in table().iter().filter(|i| matches!(i.class, Class::Armour | Class::Accessory)) {
            assert!(kh_core::status::tables::equipment_row(&item.key).is_some(), "{}", item.key);
        }
        for kind in kh_core::status::ItemKind::ALL {
            let item = by_id(kind as u32).unwrap_or_else(|| panic!("{kind:?}"));
            assert!(matches!(item.class, Class::Battle | Class::Camp));
        }
        let grades = |family: &str| table().iter().filter(|i| i.family == family).map(|i| (i.grade, i.name.as_str())).collect::<Vec<_>>();
        assert_eq!(grades("Mythril"), vec![(1, "Mythril Shard"), (2, "Mythril Stone"), (3, "Mythril Gem"), (4, "Mythril Crystal")]);
        assert_eq!(by_id(0x735).map(|i| (i.name.as_str(), i.grade)), Some(("Fluorite", 4)));
        assert_eq!(find("Blazing Shard").map(|i| i.item_id), Some(0x701));
        assert_eq!(find("0x404").map(|i| i.name.as_str()), Some("Defense Belt"));
        assert!(table().iter().all(|i| i.max == 99), "every stock maximum is 99");
        assert!(!give(0x999, 1), "an item that is not in the table is not queued");
    }

    /// (ER party S6) The friends' weapons by item id and key, both ways; nothing else of category 3.
    #[test]
    fn the_friends_weapons_are_items_of_their_own() {
        assert_eq!(party_weapon_key(0x315).as_deref(), Some("WEP_DONALD_00"));
        assert_eq!(party_weapon_key(0x322).as_deref(), Some("WEP_DONALD_13"));
        assert_eq!(party_weapon_key(0x329).as_deref(), Some("WEP_GOOFY_00"));
        assert_eq!(party_weapon_key(0x336).as_deref(), Some("WEP_GOOFY_13"));
        for id in [0x314, 0x323, 0x328, 0x337, 0x301, 0x405] {
            assert_eq!(party_weapon_key(id), None, "{id:#x}");
        }
        for id in (0x315..=0x322).chain(0x329..=0x336) {
            assert_eq!(party_weapon_id(&party_weapon_key(id).unwrap()), Some(id));
            assert!(by_id(id).is_none(), "not a row of the item table");
        }
        assert_eq!(party_weapon_id("WEP_DONALD_14"), None);
        assert_eq!(party_weapon_id("WEP_KEYBLADE_SO_02"), None);
    }
}
