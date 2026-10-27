//! (loot) ELDEN RING'S LOOT -> KH3 ITEMS (IMPL_ER25, IMPL_ER28). UNTESTED IN THE GAME: written by reading; the rules
//! and the arithmetic are unit-tested in `lootmap.rs`. EVERYTHING IS OFF BY DEFAULT.
//!
//! `loot.kh = 1` THE KILL DROP. The game keeps a killed enemy's loot in its `MapItemMan`, which the pinned SDK does
//! not describe (an opaque singleton): what the game rolled cannot be read safely. So the drop is rolled HERE, from
//! the same data the game rolls from: the enemy's `NpcParam.itemLotId_enemy` row chain of `ItemLotParam_enemy`
//! (rows id, id + 1, ... each one weighted pick; `lootmap::LotRow`), with KH3's Luck in the place of item discovery.
//! THE GAME'S OWN DROP IS SWITCHED OFF for such an enemy type by writing -1 into its row's `itemLotId_enemy` in the
//! game's memory (the original is kept and written back when Sora stops); that is done when the type is first seen
//! among the hostile targets, before any of them dies. A TYPE IS ONLY TAKEN OVER when every row of its chain is a
//! plain repeatable drop (no event flag: a flagged drop is a one-time or quest drop) and every item in it maps to
//! something by the sheet (no key item, nothing unknown): everything else keeps the game's own corpse loot.
//! At the kill each rolled item becomes a KH3 prize box at the corpse (`kh-prize`, thrown and collected like the
//! orbs; into Sora's stock through `kh_host::items::give`) or munny.
//! NOT VERIFIED: that the game reads `itemLotId_enemy` from the param row at the death (and not from a copy made
//! when the character was created): if corpses of a taken-over type still glow, it is a copy; the log line
//! `loot: type ... taken over` then tells which types to look at.
//!
//! `loot.gate = 1` THE GATE: EVERY OTHER WAY AN ITEM REACHES THE PLAYER (chests, items on placed corpses, ground
//! items, gathered plants, gifts, quest and boss rewards, the loot of enemy types that were not taken over). They
//! all go through one function (the "item give" of public tools; found by the public byte pattern `8B 02 83 F8 0A`
//! inside it, 2.7.1.0 RVA 0x561400: read from the user's executable on disk, build/sora_tmp/loot/scan_exe.js):
//! `fn(map_item_man, *ItemBuffer, out, i32)` with `ItemBuffer { count: u32 (at most 10), entries: [{ item id with
//! its table in the top four bits, quantity, i32, i32 }] }`. Elden Ring's own interaction stays (opening the chest,
//! picking up from the body, the glow): the swap happens when the game GIVES the item. The detour classifies every
//! entry (`lootmap::LootRules::classify`): a key item, anything unknown and anything no rule names PASSES; a mapped
//! item is taken out of the buffer the game then works on (so nothing of it reaches the inventory or the game's
//! popup) and its KH3 counterpart is handed to Sora with KH3's item popup (a slot / MP / AP reward: the BONUS! panel).
//! WHERE AN ITEM CAME FROM (`lootmap::Source`, part of its value): the gate sees only item ids, so the source is
//! read off the lot tables (an index of `ItemLotParam_enemy` / `ItemLotParam_map` made once: an item only enemies
//! drop, an item of a flagged map lot = a fixed pickup, of unflagged map lots = gathered) and, ONCE THE SHEET NAMES
//! THEM (`gate.callers`), off the call site: the return addresses of the function's callers are in every gate line.
//! Chest against corpse against gift is only told by the call site. HOW DEEP IN THE GAME: the map block the player
//! stands in (`region.*` rows).
//! NOT VERIFIED: that shops and the starting kit go through this function; what the game does with an emptied buffer
//! (it is handed over with count 0, which the function's own first test accepts: `cmp eax, 0Ah / ja end; test eax,
//! eax / je end`).
//!
//! `loot.log = 1` (alone, or with the others): one line per item the gate sees and per lot examined, without
//! changing anything: the data to tune the sheet with. Every line carries the item's NAME from the game's message
//! tables when it is readable there.
//!
//! `shop.kh = 1` MERCHANTS (IMPL_ER28). Every `ShopLineupParam` row is read: a key item's line stays; a line whose
//! item maps to a KH item gets that item's munny price written into the row (`value`; the rune counter is Sora's
//! munny: prize.rs) and is told in the log; the other lines are told as dropped. WHAT IS NOT DONE: the shop window
//! still shows Elden Ring's name and picture for the line (the game draws them from the item's own row; a KH name
//! needs a message file, see docs/sora-er-items.md section 5), and a dropped line is still offered. What is bought is
//! turned into its KH item by the gate when the purchase goes through it; `shop.kh = 2` also watches the inventory
//! and converts what arrives there by any other path (it uses the hand-written removal of `loot.purge`).
//!
//! `loot.purge` (IMPL_ER28; IRREVERSIBLE ON THE GAME'S SAVE once that is written): the non-key Elden Ring items the
//! character already holds are removed from the Elden Ring inventory, once per character (a marker file with the
//! list beside Sora's save). 1 = ONLY TELLS what it would remove; 2 = removes goods (stackable things without an
//! instance record); 3 = also armaments, armour, talismans and Ashes of War (their instance records in the game's
//! table are left behind: NOT VERIFIED harmless). Never: anything the rules pass (key items, tools, unknowns),
//! anything worn or in a quick slot / the pouch. The pinned SDK has no "remove item": the entry is emptied by hand
//! as the SDK lays the inventory out. Nothing is given for what is removed (user decision: "Remove them").
//!
//! `items.er_keys = 1` (IMPL_ER28): the Elden Ring KEY ITEMS the character holds are listed (name, count) on the camp
//! menu's Items screen, tab "Key Items" (KH3's own tab), read once a second from the game's key-item inventory.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU8, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use eldenring::cs::{EquipInventoryData, EquipParamAccessory, EquipParamGem, EquipParamGoods, EquipParamProtector, EquipParamWeapon, ItemId, ItemLotParam_enemy, ItemLotParam_map, MsgRepository, NpcParam, PlayerGameData, ShopLineupParam, SoloParamRepository, SpEffectParam};
use eldenring::param::ITEMLOT_PARAM_ST;
use fromsoftware_shared::FromStatic;
use glam::Vec3;
use kh_core::enums::ETresAbilityKind;
use kh_core::status::{BonusCounts, Reward};
use kh_core::{Sora, StatusRequest};
use kh_host::items::KeyItem;
use kh_host::prize::PrizeHost;
use kh_prize::Rng;

use crate::control::Control;
use crate::erlog;
use crate::havok::{locate, Located};
use crate::lootmap::{Cat, Ctx, Decision, Effect, Element, ErItem, Give, LootRules, LotRow, LotSlot, ShopLine, Source};

/// The game's item-give function (see the file header).
pub(crate) const GIVE_FN: Located = Located { name: "item give (MapItemMan)", pattern: "40 55 56 57 41 54 41 55 41 56 41 57 48 8d ac 24 70 ff ff ff 48 81 ec 90 01 00 00 48 c7 45 c8 fe ff ff ff 48 89 9c 24 d8 01 00 00 48 8b 05 ? ? ? ? 48 33 c4 48 89 85 80 00 00 00 44 89 4c 24 68 4d 8b f8 4c 89 44 24 70 4c 8b e9 33 ff 89 7c 24 5c 8b 02 83 f8 0a", capture: 0, rva_2_7_1_0: 0x0056_1400, rva_2_7_2_0: 0x0056_18b0 /* (PORT) */ };

static RULES: OnceLock<LootRules> = OnceLock::new();
static GATE_ORIGINAL: AtomicUsize = AtomicUsize::new(0);
/// The gate swallows mapped items (`loot.gate = 1`); it only tells what it sees (`loot.log = 1`).
static GATE_ON: AtomicBool = AtomicBool::new(false);
static GATE_LOG: AtomicBool = AtomicBool::new(false);
static GATE_CALLS: AtomicU32 = AtomicU32::new(0);
static GATE_SWALLOWED: AtomicU32 = AtomicU32::new(0);
/// Munny the gate owes Sora (taken by the game thread's frame).
static GATE_MUNNY: AtomicU32 = AtomicU32::new(0);
static IN_GATE: AtomicBool = AtomicBool::new(false);
/// Lines of the gate for the log (written from the game's thread inside the hook; told by the frame).
static GATE_LINES: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// (IMPL_ER28) Rewards the gate owes Sora ("acc_slot", "max_mp:5" ...), applied by the frame.
static GATE_REWARDS: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// (IMPL_ER28) The map block the player stands in and how deep in the game it lies (the frame writes them).
static BLOCK: AtomicI32 = AtomicI32::new(0);
static DEPTH: AtomicU8 = AtomicU8::new(0);
/// (IMPL_ER28) Where each item occurs in the lot tables: bit 0 an enemy lot, bit 1 a flagged map lot, bit 2 an
/// unflagged map lot. Made once.
static INDEX: OnceLock<HashMap<(Cat, u32), u8>> = OnceLock::new();
/// (PU) When the body last played a chest animation (sora.rs: chest.rs's list, whoever leads) and when a window of Elden
/// Ring's last had the pad (a merchant's): the gate's readings of a chest and of a purchase (`LootRules::gate_source`).
static CHEST_SEEN: Mutex<Option<std::time::Instant>> = Mutex::new(None);
static MENU_SEEN: Mutex<Option<std::time::Instant>> = Mutex::new(None);
/// (PU) `loot.chest_window` (s): how long after the chest animation an item still counts as the chest's.
static CHEST_WINDOW_MS: AtomicU32 = AtomicU32::new(3000);

/// (PU) ONCE A FRAME (sora.rs follow_phase): the body plays one of the chest animations (`chest.anims`, default 60110).
pub fn note_chest_anim(on: bool) {
    if on {
        if let Ok(mut t) = CHEST_SEEN.lock() {
            *t = Some(std::time::Instant::now());
        }
    }
}

/// (PU) ONCE A FRAME (sora.rs): a window of Elden Ring's has the pad (its interface state is not gameplay).
pub fn note_er_menu(on: bool) {
    if on {
        if let Ok(mut t) = MENU_SEEN.lock() {
            *t = Some(std::time::Instant::now());
        }
    }
}

fn seen_within(at: &Mutex<Option<std::time::Instant>>, ms: u32) -> bool {
    at.lock().ok().and_then(|t| *t).is_some_and(|t| t.elapsed().as_millis() <= ms as u128)
}

fn rules() -> &'static LootRules {
    RULES.get_or_init(LootRules::load)
}

fn repo() -> Option<&'static SoloParamRepository> {
    // SAFETY: the singleton lives as long as the game; its holder table is checked by the SDK.
    unsafe { SoloParamRepository::instance() }.ok()
}

/// What the params say about an item. `known` false: its row was not found.
pub fn describe(cat: Cat, id: u32) -> ErItem {
    let mut item = ErItem { cat: Some(cat), id, ..ErItem::default() };
    let Some(repo) = repo() else { return item };
    match cat {
        Cat::Goods => {
            if let Some(row) = repo.get::<EquipParamGoods>(id) {
                item.known = true;
                item.goods_type = row.goods_type();
                item.rarity = row.rarity();
                item.sort_group = row.sort_group_id();
                item.consumable = row.is_consume();
                item.sell_value = row.sell_value();
                // (ONLINEITEMS fix) a row Elden Ring's inventory never shows: a hidden goodsType, or no name text at all
                item.hidden = rules().goods_hidden(item.goods_type) || unnamed(Cat::Goods, id);
                // its use effect (a goods item that refers to no SpEffect row has none)
                let effect = row.ref_id_default();
                if effect > 0 {
                    if let Some(sp) = repo.get::<SpEffectParam>(effect as u32) {
                        let hp = sp.change_hp_point() as f32 + sp.change_hp_rate();
                        let mp = sp.change_mp_point() as f32 + sp.change_mp_rate();
                        item.effect = Effect::of_sp_effect(hp, mp, sp.soul());
                        item.soul = sp.soul().max(0);
                    }
                }
            }
        }
        Cat::Weapon => {
            // (a weapon's id carries its affinity and reinforcement level in the last four digits: the row read is the
            // plain one of its level, else the base weapon's)
            if let Some(row) = repo.get::<EquipParamWeapon>(id / 100 * 100).or_else(|| repo.get::<EquipParamWeapon>(id / 10000 * 10000)).or_else(|| repo.get::<EquipParamWeapon>(id)) {
                item.known = true;
                item.rarity = row.rarity();
                item.sort_group = row.sort_group_id();
                item.sell_value = row.sell_value();
                item.wep_kind = rules().wep_kind(row.wep_type());
                item.somber = rules().is_somber(row.material_set_id());
                item.element = Element::of_attack(row.attack_base_magic(), row.attack_base_fire(), row.attack_base_thunder(), row.attack_base_dark());
            }
        }
        Cat::Protector => {
            if let Some(row) = repo.get::<EquipParamProtector>(id) {
                item.known = true;
                item.rarity = row.rarity();
                item.sort_group = row.sort_group_id();
                item.sell_value = row.sell_value();
                item.element = Element::of_cut_rates(row.magic_damage_cut_rate(), row.fire_damage_cut_rate(), row.thunder_damage_cut_rate(), row.dark_damage_cut_rate(), rules().family_margin);
            }
        }
        Cat::Accessory => {
            if let Some(row) = repo.get::<EquipParamAccessory>(id) {
                item.known = true;
                item.rarity = row.rarity();
                item.sort_group = row.sort_group_id();
                item.sell_value = row.sell_value();
            }
        }
        Cat::Gem => {
            if let Some(row) = repo.get::<EquipParamGem>(id) {
                item.known = true;
                item.rarity = row.rarity();
                item.sort_group = row.sort_group_id();
                item.sell_value = row.sell_value();
            }
        }
    }
    item
}

/// (IMPL_ER28) The item's name in the game's message tables (the files the sheet row `msg.names` lists for its
/// table). `None`: not readable there. NOT VERIFIED IN THE GAME (the file numbers are public documentation).
pub fn item_name(cat: Cat, id: u32) -> Option<String> {
    item_name_in(cat, id).map(|(name, _)| name)
}

/// (ONLINEITEMS fix) The message files that have given a name this session (proven readable: `unnamed`).
static NAME_FILES_PROVEN: Mutex<Vec<u32>> = Mutex::new(Vec::new());

/// (ONLINEITEMS fix) An item with no name text in the game's message files, once every name file of its table is proven
/// readable (lootmap `unnamed_hidden`): Elden Ring never shows such a row. A file not proven yet hides nothing.
pub fn unnamed(cat: Cat, id: u32) -> bool {
    if unsafe { MsgRepository::instance() }.is_err() {
        return false;
    }
    let has_name = item_name_in(cat, id).is_some();
    let proven = NAME_FILES_PROVEN.lock().map(|v| v.clone()).unwrap_or_default();
    crate::lootmap::unnamed_hidden(rules().name_files(cat), &|f| proven.contains(&f), has_name)
}

fn item_name_in(cat: Cat, id: u32) -> Option<(String, u32)> {
    // SAFETY: the singleton lives as long as the game; the SDK checks the file table's bounds.
    let msgs = unsafe { MsgRepository::instance() }.ok()?;
    let ids = if cat == Cat::Weapon { vec![id, id / 100 * 100, id / 10000 * 10000] } else { vec![id] };
    for file in rules().name_files(cat) {
        for id in &ids {
            if let Some(text) = msgs.get_msg(*file, *id) {
                let end = text.iter().position(|c| *c == 0).unwrap_or(text.len()).min(80);
                let name = String::from_utf16_lossy(&text[..end]);
                if !name.trim().is_empty() && !name.contains("[ERROR]") {
                    if let Ok(mut proven) = NAME_FILES_PROVEN.lock() {
                        if !proven.contains(file) {
                            proven.push(*file);
                        }
                    }
                    return Some((name.trim().to_string(), *file));
                }
            }
        }
    }
    None
}

/// (IMPL_ER42) An item's DESCRIPTION in the game's message tables (the files of the sheet row `msg.texts`, first one that
/// has text; default GoodsCaption 24 / 321, then GoodsInfo 20 / 320), markup removed ([`plain_text`]): (text, file).
/// `None`: none readable. Guarded as [`item_name`] (the SDK checks the file table's bounds; at most 4000 characters).
/// NOT VERIFIED IN THE GAME (the file numbers are public documentation; the log line `loot: key item ...` shows them).
pub fn item_text(cat: Cat, id: u32) -> Option<(String, u32)> {
    // SAFETY: the singleton lives as long as the game; the SDK checks the file table's bounds.
    let msgs = unsafe { MsgRepository::instance() }.ok()?;
    for file in rules().text_files(cat) {
        if let Some(text) = msgs.get_msg(*file, id) {
            let end = text.iter().position(|c| *c == 0).unwrap_or(text.len()).min(4000);
            let text = plain_text(&String::from_utf16_lossy(&text[..end]));
            if !text.is_empty() && !text.contains("[ERROR]") {
                return Some((text, *file));
            }
        }
    }
    None
}

/// (IMPL_ER42) The game's message text without its markup: tags `<font color="#C0B194">` / `</font>` / `<?keyicon@31?>` /
/// `<img ...>` are removed, carriage returns dropped, spaces at line ends trimmed, no more than one empty line in a row, trimmed.
pub fn plain_text(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut depth = 0usize;
    for c in raw.chars() {
        match c {
            '<' => depth += 1,
            '>' if depth > 0 => depth -= 1,
            '\r' => {}
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    let mut lines: Vec<&str> = Vec::new();
    for line in out.lines().map(str::trim_end) {
        if line.is_empty() && lines.last().is_none_or(|l| l.is_empty()) {
            continue;
        }
        lines.push(line);
    }
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines.join("\n").trim().to_string()
}

/// (IMPL_ER42) A description for the KH item pane, whose help field holds about three lines: the lines and paragraphs run
/// on as one text (the pane cuts what does not fit: kh_ui::render::fit_text).
pub fn flowing(text: &str) -> String {
    text.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ")
}

fn tell_item(item: &ErItem, decision: &Decision) -> String {
    let name = item.cat.and_then(|c| item_name(c, item.id)).map_or(String::new(), |n| format!(" \"{n}\""));
    let weapon = match (item.cat, repo()) {
        (Some(Cat::Weapon), Some(repo)) => repo.get::<EquipParamWeapon>(item.id / 100 * 100).or_else(|| repo.get::<EquipParamWeapon>(item.id / 10000 * 10000)).map_or(String::new(), |row| format!(", wepType {} = {}, materialSet {}{}", row.wep_type(), item.wep_kind.name(), row.material_set_id(), if item.somber { " = somber" } else { "" })),
        _ => String::new(),
    };
    format!(
        "{} {}{name} (goods type {}, rarity {}, sort group {}, {}effect {}{}, sell {}{}{weapon}{}) -> {} by {}{}",
        item.cat.map_or("?", Cat::name),
        item.id,
        item.goods_type,
        item.rarity,
        item.sort_group,
        if item.consumable { "consumable, " } else { "" },
        item.effect.name(),
        if item.soul > 0 { format!(" {} runes", item.soul) } else { String::new() },
        item.sell_value,
        if item.element == Element::None { String::new() } else { format!(", element {}", item.element.name()) },
        if item.known { "" } else { ", ROW NOT FOUND" },
        tell_give(&decision.give),
        decision.rule,
        if decision.why.is_empty() { String::new() } else { format!(" [{}]", decision.why) }
    )
}

fn tell_give(give: &Give) -> String {
    match give {
        Give::Pass => "PASS (stays Elden Ring's)".to_string(),
        Give::Kh { item_id, key } => format!("{} ({key}, item {item_id:#x})", kh_host::items::by_id(*item_id).map_or("?", |i| i.name.as_str())),
        Give::Munny(n) => format!("{n} munny"),
        Give::Reward(what) => format!("REWARD {what}"),
        Give::Nothing => "nothing".to_string(),
    }
}

/// (IMPL_ER28) What Sora holds, for the rules: the HUD's count (stock + worn, slot bonuses) plus the slot rewards
/// the gate still owes him.
fn held(key: &str) -> u32 {
    let owed = |what: &str| GATE_REWARDS.lock().map_or(0, |r| r.iter().filter(|x| x.as_str() == what).count() as u32);
    kh_host::items::held(key)
        + match key {
            "@acc_slots" => owed("acc_slot"),
            "@def_slots" => owed("def_slot"),
            _ => 0,
        }
}

fn ctx(source: Source) -> Ctx<'static> {
    Ctx { source, depth: DEPTH.load(Ordering::Relaxed), state: &held, unit: 0.999 }
}

/// (er30) A random number in [0, 1) for the gate's `chance:` gives (the gate runs inside the game's call: its own
/// small generator, xorshift).
fn gate_unit() -> f32 {
    static SEED: AtomicU32 = AtomicU32::new(0x9E37_79B9);
    let mut x = SEED.load(Ordering::Relaxed);
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    SEED.store(x, Ordering::Relaxed);
    (x >> 8) as f32 * (1.0 / 16_777_216.0)
}

/// (IMPL_ER28) A reward text of the sheet -> the reward seam. The game's BONUS! panel shows it at his next step.
fn grant(sora: &mut Sora, what: &str) -> String {
    let (name, n) = what.split_once(':').unwrap_or((what, ""));
    let n = n.parse::<i32>().unwrap_or(0);
    let reward = match name {
        "acc_slot" => Some(Reward::Bonus(BonusCounts::ACC_SLOT_UP1)),
        "def_slot" => Some(Reward::Bonus(BonusCounts::DEF_SLOT_UP1)),
        "max_hp" => Reward::max_hp(n),
        "max_mp" => Reward::max_mp(n),
        "ap" => Some(Reward::Ap(n.clamp(1, 10) as u8)),
        _ => None,
    };
    // (IT) a reward that is a PERMANENT STAT (AP, max HP / MP; not a slot) goes to the whole roster too, unlocked or not
    // (the user, 2026-10-17 00:50): the party host's one total (kh-partyhost boosts.rs)
    #[cfg(feature = "party")]
    let roster = match reward {
        Some(Reward::Ap(n)) => Some(kh_party::boosts::Boost::Ap(n)),
        Some(Reward::Bonus(k)) if kh_party::boosts::Boost::is_stat_bonus(k) => Some(kh_party::boosts::Boost::Bonus(k)),
        _ => None,
    };
    #[cfg(feature = "party")]
    if let Some(b) = roster {
        crate::partyui::roster_boost(b, &format!("the Elden Ring item's reward {what}"));
    }
    match reward {
        Some(reward) => {
            let outcome = sora.grant(reward);
            format!("reward {what} granted{}", if outcome.changed { "" } else { " (NOTHING CHANGED: he had it all)" })
        }
        None => format!("reward {what}: NOT UNDERSTOOD, nothing given"),
    }
}

fn lot_row(id: u32, row: &ITEMLOT_PARAM_ST) -> LotRow {
    macro_rules! slot {
        ($id:ident, $cat:ident, $points:ident, $num:ident, $luck:ident, $flag:ident) => {
            LotSlot { category: row.$cat(), item: row.$id(), points: row.$points() as u32, count: row.$num(), luck: row.$luck(), flag: row.$flag() }
        };
    }
    LotRow {
        id,
        slots: [
            slot!(lot_item_id01, lot_item_category01, lot_item_base_point01, lot_item_num01, enable_luck01, get_item_flag_id01),
            slot!(lot_item_id02, lot_item_category02, lot_item_base_point02, lot_item_num02, enable_luck02, get_item_flag_id02),
            slot!(lot_item_id03, lot_item_category03, lot_item_base_point03, lot_item_num03, enable_luck03, get_item_flag_id03),
            slot!(lot_item_id04, lot_item_category04, lot_item_base_point04, lot_item_num04, enable_luck04, get_item_flag_id04),
            slot!(lot_item_id05, lot_item_category05, lot_item_base_point05, lot_item_num05, enable_luck05, get_item_flag_id05),
            slot!(lot_item_id06, lot_item_category06, lot_item_base_point06, lot_item_num06, enable_luck06, get_item_flag_id06),
            slot!(lot_item_id07, lot_item_category07, lot_item_base_point07, lot_item_num07, enable_luck07, get_item_flag_id07),
            slot!(lot_item_id08, lot_item_category08, lot_item_base_point08, lot_item_num08, enable_luck08, get_item_flag_id08),
        ],
        rarity: row.lot_item_rarity(),
        flag: row.get_item_flag_id(),
        cumulate: row.cumulate_num_max() > 0,
    }
}

/// The row chain of a lot id (`ItemLotParam_enemy`).
fn read_lot(lot: i32) -> Vec<LotRow> {
    let mut rows = Vec::new();
    let (Some(repo), true) = (repo(), lot > 0) else { return rows };
    for k in 0..rules().chain_max {
        let Some(row) = repo.get::<ItemLotParam_enemy>(lot as u32 + k) else { break };
        rows.push(lot_row(lot as u32 + k, row));
    }
    rows
}

/// The key of an item in the lot index (a weapon: without affinity and level).
fn index_key(cat: Cat, id: u32) -> (Cat, u32) {
    (cat, if cat == Cat::Weapon { id / 10000 * 10000 } else { id })
}

/// (IMPL_ER28) Where every item occurs in the two lot tables (read once, a few thousand rows).
fn index() -> &'static HashMap<(Cat, u32), u8> {
    INDEX.get_or_init(|| {
        let mut out: HashMap<(Cat, u32), u8> = HashMap::new();
        let Some(repo) = repo() else { return out };
        let mut add = |row: &LotRow, enemy: bool| {
            for slot in row.slots.iter().filter(|s| !s.empty()) {
                if let Some(cat) = rules().lot_category(slot.category) {
                    let bit = if enemy {
                        1
                    } else if row.flag != 0 || slot.flag != 0 {
                        2
                    } else {
                        4
                    };
                    *out.entry(index_key(cat, slot.item as u32)).or_default() |= bit;
                }
            }
        };
        let (mut enemy, mut map) = (0, 0);
        for (id, row) in repo.rows::<ItemLotParam_enemy>() {
            add(&lot_row(id, row), true);
            enemy += 1;
        }
        for (id, row) in repo.rows::<ItemLotParam_map>() {
            add(&lot_row(id, row), false);
            map += 1;
        }
        erlog!("loot: lot index: {enemy} enemy lot row(s), {map} map lot row(s), {} item(s): {} only from enemies, {} in flagged map lots (fixed pickups), {} only gathered", out.len(), out.values().filter(|b| **b == 1).count(), out.values().filter(|b| **b & 2 != 0).count(), out.values().filter(|b| **b == 4).count());
        out
    })
}

/// The item of a lot slot with the row's rarity over its own.
fn slot_item(row: &LotRow, slot: &LotSlot) -> ErItem {
    match rules().lot_category(slot.category) {
        Some(cat) => {
            let mut item = describe(cat, slot.item as u32);
            if row.rarity >= 0 {
                item.rarity = row.rarity as u8;
            }
            item
        }
        None => ErItem { cat: None, id: slot.item as u32, ..ErItem::default() },
    }
}

/// An enemy type's lot as it was examined.
struct Type {
    /// The row's own `itemLotId_enemy` (written back at the end).
    original: i32,
    rows: Vec<LotRow>,
    /// The game's drop is switched off and the drop is ours.
    taken: bool,
}

pub struct ErLoot {
    types: HashMap<i32, Type>,
    rng: Rng,
    kh: bool,
    told: HashSet<(u8, u32)>,
    pub kills: u32,
    pub boxes: u32,
    pub munny: i64,
    started: bool,
    /// (IMPL_ER28) `shop.kh`: the rows whose price was rewritten (row id, the row's own `value`).
    shop: Vec<(u32, i32)>,
    shop_on: bool,
    /// `shop.kh = 2`: the normal inventory as it was last seen (item id -> quantity).
    seen: Option<HashMap<u32, u32>>,
    /// `loot.purge`: done (or refused) for this session.
    purged: bool,
    clock: f32,
    keys_listed: bool,
    /// (HUD2) The key items held at the last scan (item id -> count; None = no baseline yet) and the frames to the next.
    key_seen: Option<HashMap<u32, u32>>,
    key_scan: u32,
    /// (KEYPOP) Normal-inventory goods by item id: whether they get the key-item popup (lootmap `key_item_popup`; their
    /// params read once a session).
    popup_goods: HashMap<u32, bool>,
}

impl Default for ErLoot {
    fn default() -> Self {
        ErLoot::new()
    }
}

impl ErLoot {
    pub fn new() -> ErLoot {
        ErLoot { types: HashMap::new(), rng: Rng::new(0x4C4F_4F54), kh: false, told: HashSet::new(), kills: 0, boxes: 0, munny: 0, started: false, shop: Vec::new(), shop_on: false, seen: None, purged: false, clock: 0.0, keys_listed: false, key_seen: None, key_scan: 0, popup_goods: HashMap::new() /* (KEYPOP) */ }
    }

    fn start(&mut self, control: &Control) {
        self.started = true;
        let (kh, gate, log) = (control.flag("loot.kh") == Some(true), control.flag("loot.gate") == Some(true), control.flag("loot.log") == Some(true));
        if !(kh || gate || log) {
            return;
        }
        let r = rules();
        erlog!("loot: sheet sora-er-items: generation {}, {} rule(s), {} override(s), {} KH piece(s) ranked into {} tiers, Luck x{} per Lucky Strike, base discovery {}, base drop {}{}", r.generation, r.rule_count().0, r.rule_count().1, r.ranked().len(), r.tier_count, r.luck_per_copy, r.base_discovery, if r.base_on { r.base_summary() } else { "OFF".to_string() }, if r.problems.is_empty() { String::new() } else { format!(" | {} PROBLEM ROW(S): {}", r.problems.len(), r.problems.join("; ")) });
        index();
        if (gate || log) && GATE_ORIGINAL.load(Ordering::SeqCst) == 0 {
            match locate(&GIVE_FN) {
                // SAFETY: the function's start, found by pattern at the recorded address; the hook has its signature.
                Some(f) => match unsafe { kh_er_render::inline_hook(f, give_hook as *const () as usize, &GATE_ORIGINAL, "item give") } {
                    Ok(()) => erlog!("loot: the game's item-give function is detoured (loot.gate = {}, loot.log = {}): {}", gate as u8, log as u8, if gate { "mapped items become KH3 items, key items and unknown items pass" } else { "it only tells what it sees" }),
                    Err(e) => {
                        GATE_ORIGINAL.store(0, Ordering::SeqCst);
                        erlog!("loot: the item-give function was NOT detoured ({e}): the gate is off");
                    }
                },
                None => erlog!("loot: the item-give function was not found for this executable (PORT: 2.7.1.0 / 2.7.2.0 table): the gate is off"),
            }
        }
    }

    /// ONCE PER FRAME (game thread): the keys; the enemy types among the hostile targets are examined and, with
    /// `loot.kh = 1`, taken over; what the gate owes Sora is paid; the gate's lines are told; (IMPL_ER28) the map
    /// block and its depth, the shops, the purge, the key-item list.
    pub fn frame(&mut self, control: &Control, sora: &mut Sora, hostile_types: &[i32]) {
        if !self.started {
            self.start(control);
        }
        let kh = control.flag("loot.kh") == Some(true);
        let log = control.flag("loot.log") == Some(true);
        let gate = control.flag("loot.gate") == Some(true);
        let level = |key: &str| control.f32(key).map_or(0, |v| v as i32);
        let (shop, purge, er_keys) = (level("shop.kh"), level("loot.purge"), control.flag("items.er_keys") == Some(true));
        GATE_ON.store(gate, Ordering::Relaxed);
        CHEST_WINDOW_MS.store((control.f32("loot.chest_window").unwrap_or(3.0).clamp(0.0, 20.0) * 1000.0) as u32, Ordering::Relaxed); // (PU)
        GATE_LOG.store(log || gate, Ordering::Relaxed);
        if kh || log || gate || shop > 0 {
            if let Some(player) = crate::game::player_mut() {
                let block: i32 = player.current_block_id.into();
                if BLOCK.swap(block, Ordering::Relaxed) != block {
                    if let Some(depth) = rules().depth_of_block(block) {
                        DEPTH.store(depth, Ordering::Relaxed);
                    }
                }
            }
        }
        if self.kh && !kh {
            self.restore("loot.kh was switched off");
        }
        if kh && !self.kh {
            // (types only told so far are examined again: now they may be taken over)
            self.types.clear();
        }
        self.kh = kh;
        if kh || log {
            for &npc_param in hostile_types {
                if npc_param > 0 && !self.types.contains_key(&npc_param) {
                    self.examine(npc_param, kh);
                }
            }
        }
        let munny = GATE_MUNNY.swap(0, Ordering::Relaxed);
        if munny > 0 {
            self.munny += munny as i64;
            sora.status_request(StatusRequest::PrizeCollected { kind: kh_core::status::PrizeKind::Munny1, amount: munny as i32 });
        }
        let rewards = GATE_REWARDS.lock().map(|mut r| std::mem::take(&mut *r)).unwrap_or_default();
        for what in rewards {
            erlog!("loot: gate: {}", grant(sora, &what));
        }
        if let Ok(mut lines) = GATE_LINES.lock() {
            for line in lines.drain(..) {
                erlog!("loot: gate: {line}");
            }
        }
        // ---- (IMPL_ER28) shops, the purge, the key items
        if (shop > 0) != self.shop_on {
            if shop > 0 {
                self.shop_start(log);
            } else {
                self.shop_restore("shop.kh was switched off");
            }
            self.shop_on = shop > 0;
        }
        self.clock += 1.0 / 60.0;
        let second = self.clock >= 1.0;
        if second {
            self.clock = 0.0;
        }
        if (purge > 0 && gate && !self.purged) || (shop >= 2 && second) || (er_keys && second) || (!er_keys && self.keys_listed) {
            if let Some(data) = player_data() {
                if purge > 0 && gate && !self.purged {
                    self.purged = true;
                    purge_inventory(data, purge);
                }
                if shop >= 2 && second {
                    self.watch_inventory(data, sora);
                }
                if er_keys && second {
                    kh_host::items::set_key_items(key_items(data));
                    self.keys_listed = true;
                } else if !er_keys && self.keys_listed {
                    kh_host::items::set_key_items(Vec::new());
                    self.keys_listed = false;
                }
            }
        }
        if shop < 2 {
            self.seen = None;
        }
        // (HUD2) A NEW ELDEN RING KEY ITEM -> KH3's OBTAINED popup (`hud.obtained = 1`; off by default)
        self.key_popups(control);
    }

    /// An enemy type seen for the first time: its lot is read and told; with `take` the game's drop is switched off
    /// when the whole chain can be replaced.
    fn examine(&mut self, npc_param: i32, take: bool) {
        let original = repo().and_then(|r| r.get::<NpcParam>(npc_param as u32)).map_or(0, |row| row.item_lot_id_enemy());
        let rows = read_lot(original);
        let mut why = None;
        if original <= 0 {
            why = Some("it has no enemy item lot".to_string());
        } else if rows.is_empty() {
            why = Some(format!("lot {original} has no row"));
        }
        let mut lines = Vec::new();
        for row in &rows {
            let chances = row.chances(rules().luck_factor(0));
            let mut parts = Vec::new();
            for (k, slot) in row.slots.iter().enumerate() {
                if slot.points == 0 {
                    continue;
                }
                if slot.empty() {
                    parts.push(format!("slot {}: nothing {:.1} %", k + 1, chances[k] * 100.0));
                    continue;
                }
                let item = slot_item(row, slot);
                let decision = rules().classify(&item, &ctx(Source::Enemy));
                if why.is_none() && matches!(decision.give, Give::Pass) {
                    why = Some(format!("lot row {} slot {} holds an item that stays Elden Ring's ({} {}, by {})", row.id, k + 1, item.cat.map_or("?", Cat::name), item.id, decision.rule));
                }
                parts.push(format!("slot {}: {:.1} %{}{} x{} {}", k + 1, chances[k] * 100.0, if slot.luck { " (luck)" } else { "" }, if slot.flag != 0 { format!(" FLAG {}", slot.flag) } else { String::new() }, slot.count, tell_item(&item, &decision)));
            }
            if why.is_none() && !row.plain() {
                why = Some(format!("lot row {} carries an event flag (a one-time or quest drop)", row.id));
            }
            lines.push(format!("loot:   row {} (row rarity {}{}{}): {}", row.id, row.rarity, if row.flag != 0 { format!(", FLAG {}", row.flag) } else { String::new() }, if row.cumulate { ", a pity counter (NOT MODELLED)" } else { "" }, parts.join(" | ")));
        }
        let mut taken = false;
        if take && why.is_none() {
            // SAFETY: game thread; the row is the game's own table in memory; the original is kept and written back.
            if let Some(row) = unsafe { SoloParamRepository::instance_mut() }.ok().and_then(|r| r.get_mut::<NpcParam>(npc_param as u32)) {
                row.set_item_lot_id_enemy(-1);
                taken = true;
            } else {
                why = Some("its NpcParam row cannot be written".to_string());
            }
        }
        erlog!("loot: type NpcParam {npc_param}: enemy lot {original}, {} row(s): {}", rows.len(), if taken { "TAKEN OVER (the game's drop is off for this type: KH3 boxes at the kill)".to_string() } else { format!("left to the game ({})", why.unwrap_or_else(|| "loot.kh is off: only told".to_string())) });
        for line in lines {
            erlog!("{line}");
        }
        self.types.insert(npc_param, Type { original, rows, taken });
    }

    /// AN ENEMY SORA KILLED: with its type taken over, every row of its lot is rolled and what comes out is thrown
    /// as KH3 prize boxes at `at` (cm, KH frame) or paid as munny.
    pub fn on_kill(&mut self, control: &Control, sora: &mut Sora, host: &mut PrizeHost, target: u32, npc_param: i32, max_hp: i32, at: Vec3) {
        if control.flag("loot.kh") != Some(true) {
            return;
        }
        let luck = sora.status().count(ETresAbilityKind::LuckUp);
        let height = control.f32("prize.height").filter(|v| (-200.0..=500.0).contains(v)).unwrap_or(80.0);
        // (er30) THE BASE CONSUMABLE DROP: every kill, whatever the enemy's own lot holds and whoever rolls that lot
        if rules().base_on && control.flag("loot.base") != Some(false) {
            let (class, depth) = (rules().base_class(max_hp), DEPTH.load(Ordering::Relaxed));
            let rng = &mut self.rng;
            let drops = rules().base_drops(class, depth, luck, &mut || rng.unit());
            let mut said = Vec::new();
            for (item_id, key, line, rate) in &drops {
                match host.prizes.drop_item_box(at + Vec3::Z * height, *item_id, false) {
                    Some(id) => {
                        self.boxes += 1;
                        said.push(format!("{} ({key}; line {line} at {rate} %) => box {id}", kh_host::items::by_id(*item_id).map_or("?", |i| i.name.as_str())));
                    }
                    None => said.push(format!("{key}: NO BOX")),
                }
            }
            erlog!("loot: target {target} (NpcParam {npc_param}, max HP {max_hp}) base drop: class {} ({}), depth {depth}, Luck x{luck}: {}", class, ["ordinary", "big", "boss"][class.min(2)], if said.is_empty() { "nothing".to_string() } else { said.join(" | ") });
        }
        let Some(kind) = self.types.get(&npc_param).filter(|t| t.taken) else {
            erlog!("loot: target {target} (NpcParam {npc_param}) KILLED: its type is not taken over: the game's own drop stands");
            host.take(sora);
            return;
        };
        self.kills += 1;
        let factor = rules().luck_factor(luck);
        for row in &kind.rows {
            let unit = self.rng.unit();
            let chances = row.chances(factor);
            let Some(k) = row.roll(factor, unit) else {
                erlog!("loot: target {target} lot row {}: no points: nothing", row.id);
                continue;
            };
            let slot = row.slots[k];
            if slot.empty() {
                erlog!("loot: target {target} lot row {}: rolled {unit:.4} -> slot {} = NOTHING ({:.1} %; Luck x{luck} = factor {factor:.2})", row.id, k + 1, chances[k] * 100.0);
                continue;
            }
            let item = slot_item(row, &slot);
            let decision = rules().classify(&item, &Ctx { unit: self.rng.unit(), ..ctx(Source::Enemy) });
            let count = if decision.one { 1 } else { slot.count.max(1) as i32 };
            let what = match &decision.give {
                Give::Kh { item_id, .. } => {
                    // one box; the lot's count rides on it (`Prize::amount`)
                    match host.prizes.drop_item_box(at + Vec3::Z * height, *item_id, false) {
                        Some(id) => {
                            self.boxes += 1;
                            host.set_amount(id, count);
                            format!("box {id} ({:?})", kh_prize::BoxKind::of_item(*item_id))
                        }
                        None => "NO BOX (item id 0)".to_string(),
                    }
                }
                Give::Munny(n) => {
                    let n = *n * count;
                    self.munny += n as i64;
                    sora.status_request(StatusRequest::PrizeCollected { kind: kh_core::status::PrizeKind::Munny1, amount: n });
                    format!("{n} munny paid")
                }
                Give::Reward(what) => grant(sora, what),
                Give::Nothing => "nothing given".to_string(),
                // (cannot happen for a taken-over type: its chain was checked; the item is lost, said loudly)
                Give::Pass => "A KEY / UNKNOWN ITEM IN A TAKEN-OVER LOT: NOT GIVEN (report this line)".to_string(),
            };
            erlog!("loot: target {target} lot row {}: rolled {unit:.4} -> slot {} ({:.1} %{}; Luck x{luck} = factor {factor:.2}) x{}: {} => {what}", row.id, k + 1, chances[k] * 100.0, if slot.luck { ", luck" } else { "" }, slot.count, tell_item(&item, &decision));
        }
        host.take(sora);
    }

    /// Every taken-over type gets its lot id back (Sora stops, the key is switched off); the shops their prices.
    pub fn restore(&mut self, why: &str) {
        let mut count = 0;
        for (npc_param, kind) in self.types.iter_mut().filter(|(_, t)| t.taken) {
            // SAFETY: as in `examine`.
            if let Some(row) = unsafe { SoloParamRepository::instance_mut() }.ok().and_then(|r| r.get_mut::<NpcParam>(*npc_param as u32)) {
                row.set_item_lot_id_enemy(kind.original);
                count += 1;
            }
            kind.taken = false;
        }
        self.types.clear();
        if count > 0 {
            erlog!("loot: {count} enemy type(s) got their own item lot back ({why})");
        }
        if why != "loot.kh was switched off" {
            self.shop_restore(why);
            self.shop_on = false;
        }
    }

    /// (IMPL_ER28) `shop.kh`: every merchant line is read and told; a line that sells a KH item gets its munny price.
    fn shop_start(&mut self, log: bool) {
        // SAFETY: game thread; the rows are the game's own table in memory; the originals are kept and written back.
        let Some(repo) = unsafe { SoloParamRepository::instance_mut() }.ok() else { return };
        let depth = DEPTH.load(Ordering::Relaxed);
        let (mut keep, mut sell, mut dropped, mut unknown) = (0, 0, 0, 0);
        let mut lines = Vec::new();
        for (id, row) in repo.rows_mut::<ShopLineupParam>() {
            let Some(cat) = rules().shop_category(row.equip_type()) else {
                unknown += 1;
                continue;
            };
            if row.equip_id() <= 0 {
                continue;
            }
            // (a shop line's price is the same wherever the merchant stands: depth 0, so a line is one KH item for good)
            let item = describe(cat, row.equip_id() as u32);
            let (line, decision) = rules().shop_line(&item, 0);
            let said = match &line {
                ShopLine::Keep => {
                    keep += 1;
                    "KEPT (Elden Ring's line)".to_string()
                }
                ShopLine::Sell { key, price, .. } => {
                    sell += 1;
                    self.shop.push((id, row.value()));
                    let was = row.value();
                    row.set_value(*price as i32);
                    format!("SELLS {} for {price} munny (the row's price was {was})", kh_host::items::by_key(key).map_or("?", |i| i.name.as_str()))
                }
                ShopLine::Drop(why) => {
                    dropped += 1;
                    format!("DROPPED ({why}; STILL OFFERED by the game's window)")
                }
            };
            if log && lines.len() < 600 {
                lines.push(format!("loot: shop line {id} (stock {}, cost type {}): {} => {said}", row.sell_quantity(), row.cost_type(), tell_item(&item, &decision)));
            }
        }
        erlog!("loot: shop.kh: {} merchant line(s) sell a KH item at its munny price, {keep} key line(s) kept, {dropped} with nothing to sell in their place (not hidden), {unknown} of an unknown item table (untouched); the block's depth was {depth}. THE SHOP WINDOW STILL SHOWS ELDEN RING'S NAMES AND PICTURES; what is bought becomes the KH item at the gate (loot.gate = 1) or by the inventory watch (shop.kh = 2){}", sell, if log { "" } else { " (loot.log = 1 lists every line)" });
        for line in lines {
            erlog!("{line}");
        }
    }

    fn shop_restore(&mut self, why: &str) {
        if self.shop.is_empty() {
            return;
        }
        // SAFETY: as in `shop_start`.
        if let Some(repo) = unsafe { SoloParamRepository::instance_mut() }.ok() {
            for (id, value) in self.shop.drain(..) {
                if let Some(row) = repo.get_mut::<ShopLineupParam>(id) {
                    row.set_value(value);
                }
            }
            erlog!("loot: the merchant lines got their own prices back ({why})");
        }
    }

    /// (IMPL_ER28) `shop.kh = 2`: what arrived in the Elden Ring inventory since the last look (a purchase, anything
    /// that does not go through the gate) is turned into its KH item and taken out of the inventory again.
    fn watch_inventory(&mut self, data: &mut PlayerGameData, sora: &mut Sora) {
        let inventory = &mut data.equipment.equip_inventory_data;
        let now: HashMap<u32, u32> = inventory.items_data.normal_entries().iter().filter_map(|e| e.as_option()).fold(HashMap::new(), |mut m, e| {
            *m.entry(e.item_id.into_inner()).or_default() += e.quantity;
            m
        });
        let Some(before) = self.seen.replace(now.clone()) else { return };
        for (raw, quantity) in &now {
            let gained = quantity.saturating_sub(before.get(raw).copied().unwrap_or(0));
            let Some(cat) = cat_of_item_id(*raw).filter(|_| gained > 0) else { continue };
            let item = describe(cat, raw & 0x0FFF_FFFF);
            // (PU) a purchase while (or just after) a window of the game's had the pad; anything else: unknown (materials only)
            let source = if seen_within(&MENU_SEEN, 3000) { Source::Shop } else { Source::Unknown };
            let decision = rules().classify(&item, &ctx(source));
            if matches!(decision.give, Give::Pass) {
                continue;
            }
            let removed = remove_items(inventory, *raw, gained, cat != Cat::Goods);
            let given = if removed == 0 {
                "NOT REMOVED (worn, in a slot, or of a kind the removal leaves alone): nothing given".to_string()
            } else {
                match &decision.give {
                    Give::Kh { item_id, .. } => format!("given: {}", kh_host::items::give(*item_id, if decision.one { 1 } else { removed })),
                    Give::Munny(n) => {
                        let n = *n * removed as i32;
                        sora.status_request(StatusRequest::PrizeCollected { kind: kh_core::status::PrizeKind::Munny1, amount: n });
                        format!("{n} munny paid")
                    }
                    Give::Reward(what) => grant(sora, what),
                    _ => "nothing given".to_string(),
                }
            };
            erlog!("loot: watch: the Elden Ring inventory gained x{gained} {} NOT THROUGH THE GATE: {removed} removed, {given}", tell_item(&item, &decision));
            if let Some(seen) = self.seen.as_mut() {
                *seen.entry(*raw).or_default() = quantity.saturating_sub(removed);
            }
        }
    }

    pub fn summary(&self) -> String {
        format!("loot: {} type(s) examined, {} taken over | {} kill(s) rolled, {} box(es), {} munny | gate: {} call(s), {} item(s) turned into KH3's", self.types.len(), self.types.values().filter(|t| t.taken).count(), self.kills, self.boxes, self.munny, GATE_CALLS.load(Ordering::Relaxed), GATE_SWALLOWED.load(Ordering::Relaxed))
    }

    pub fn active(&self) -> bool {
        !self.types.is_empty() || GATE_CALLS.load(Ordering::Relaxed) > 0
    }
}

impl ErLoot {
    /// (HUD2, the user: "When you get a key item this will appear instead of the standard item notification. It pauses the
    /// game and you must press A to close the popup. Key items from Elden Ring ... will trigger this") Every 10 frames
    /// the game's key-item inventory is read (ids and counts only); an item that came (or whose count rose) since the
    /// last read gets KH3's OBTAINED popup with ITS OWN name, caption (the message tables) and picture (iconId ->
    /// the pack's er_icons). The first read, and a read where more than 4 items came at once (a load, another
    /// character), only take the baseline. NOT DONE: Elden Ring's own item window for the pickup (the gate passes key
    /// items untouched, IMPL_ER25: no safe way to keep the item and drop only its window) still shows under the popup.
    /// (KEYPOP) The online items of the normal inventory (lootmap `key_item_popup`, by their category) are read with them
    /// and get the same popup.
    fn key_popups(&mut self, control: &Control) {
        if control.flag("hud.obtained") == Some(false) { // (coordinator 2026-10-20) ON by default
            self.key_seen = None;
            return;
        }
        if self.key_scan > 0 {
            self.key_scan -= 1;
            return;
        }
        self.key_scan = 10;
        let Some(data) = player_data() else {
            // a loading screen, no player: the next read starts afresh
            self.key_seen = None;
            return;
        };
        let mut now: HashMap<u32, u32> = HashMap::new();
        for entry in data.equipment.equip_inventory_data.items_data.current_key_entries().iter().filter_map(|e| e.as_option()) {
            *now.entry(entry.item_id.into_inner()).or_default() += entry.quantity.max(1);
        }
        // (KEYPOP, the user 2026-10-27: "also multiplayer items should give the "key item" pop up that maps do") ... and the
        // NORMAL inventory's goods whose category gets it (the online items: lootmap `key_item_popup`), counted the same way
        // (a pickup, a gift or a craft that raises the count comes once; using one does not)
        for entry in data.equipment.equip_inventory_data.items_data.normal_entries().iter().filter_map(|e| e.as_option()) {
            let raw = entry.item_id.into_inner();
            if cat_of_item_id(raw) != Some(Cat::Goods) {
                continue;
            }
            let popup = match self.popup_goods.get(&raw) {
                Some(p) => *p,
                None => {
                    let item = describe(Cat::Goods, raw & 0x0FFF_FFFF);
                    let p = crate::lootmap::key_item_popup(&item);
                    if item.known {
                        self.popup_goods.insert(raw, p); // (a row not readable yet is asked again at the next scan)
                    }
                    p
                }
            };
            if popup {
                *now.entry(raw).or_default() += entry.quantity.max(1);
            }
        }
        for (raw, gained) in key_gains(&mut self.key_seen, now) {
            let Some(item) = key_item_of(raw) else { continue };
            let what = kh_host::hud::ObtainedKind::ErKeyItem { name: item.name.clone(), caption: item.help.clone(), icon_id: item.icon_id };
            let shown = crate::hud::with_hud(|hud| hud.show_obtained(what));
            erlog!("loot: key item {raw:#x} \"{}\" x{gained} came: KH3's OBTAINED popup {} (icon {:?})", item.name, match shown { Some(true) => "asked", Some(false) => "NOT available (no KeyItem movie)", None => "NOT shown (no HUD)" }, item.icon_id);
        }
    }
}

/// (ONLINEITEMS fix) Whether Elden Ring's own inventory shows this item (goods only can be hidden; other tables always show).
fn shown_by_er(raw: u32) -> bool {
    match cat_of_item_id(raw) {
        Some(Cat::Goods) => crate::lootmap::listed_on_kh_screens(&describe(Cat::Goods, raw & 0x0FFF_FFFF)),
        _ => true,
    }
}

/// (HUD2) The key items that came since the last read: (item id, how many more). The first read, and a read where more
/// than 4 items came at once (a load, another character), take the baseline only.
fn key_gains(seen: &mut Option<HashMap<u32, u32>>, now: HashMap<u32, u32>) -> Vec<(u32, u32)> {
    let Some(before) = seen.as_ref() else {
        *seen = Some(now);
        return Vec::new();
    };
    let mut gains: Vec<(u32, u32)> = now.iter().filter_map(|(id, n)| { let b = before.get(id).copied().unwrap_or(0); (*n > b).then(|| (*id, *n - b)) }).collect();
    gains.sort();
    *seen = Some(now);
    if gains.len() > 4 {
        return Vec::new();
    }
    gains
}

/// (HUD2) One Elden Ring key item as the Key Items tab lists it (name, caption, icon) by its item id.
fn key_item_of(raw: u32) -> Option<KeyItem> {
    let cat = cat_of_item_id(raw)?;
    if !shown_by_er(raw) {
        return None; // (ONLINEITEMS fix) no popup for a row Elden Ring never shows
    }
    let id = raw & 0x0FFF_FFFF;
    let name = item_name(cat, id).unwrap_or_else(|| format!("Elden Ring {} {id}", cat.name()));
    let text = item_text(cat, id);
    let icon_id = if cat == Cat::Goods { repo().and_then(|r| r.get::<EquipParamGoods>(id)).map(|row| row.icon_id() as u32) } else { None };
    Some(KeyItem { name, count: 1, help: text.map(|t| flowing(&t.0)).unwrap_or_default(), icon_id })
}

#[cfg(test)]
mod hud2_key_tests {
    use super::*;

    fn map(v: &[(u32, u32)]) -> HashMap<u32, u32> {
        v.iter().copied().collect()
    }

    /// (HUD2) The first read is the baseline; a new item and a count that rose come once; a load's whole new inventory
    /// gives no popups.
    #[test]
    fn hud2_new_key_items_are_found_once() {
        let mut seen = None;
        assert!(key_gains(&mut seen, map(&[(0x4000_1F40, 3), (0x4000_2000, 1)])).is_empty(), "the baseline");
        assert_eq!(key_gains(&mut seen, map(&[(0x4000_1F40, 4), (0x4000_2000, 1), (0x4000_2100, 1)])), vec![(0x4000_1F40, 1), (0x4000_2100, 1)]);
        assert!(key_gains(&mut seen, map(&[(0x4000_1F40, 4), (0x4000_2000, 1), (0x4000_2100, 1)])).is_empty(), "nothing new");
        assert!(key_gains(&mut seen, map(&[(0x4000_1F40, 2), (0x4000_2000, 1), (0x4000_2100, 1)])).is_empty(), "two used: none");
        assert_eq!(key_gains(&mut seen, map(&[(0x4000_1F40, 3), (0x4000_2000, 1), (0x4000_2100, 1)])), vec![(0x4000_1F40, 1)], "picked up again");
        let load: Vec<(u32, u32)> = (0..9).map(|k| (0x4000_3000 + k, 1)).collect();
        assert!(key_gains(&mut seen, map(&load)).is_empty(), "a load: baseline only");
    }
}

/// The local player's game data (the inventory lives in it).
pub(crate) fn player_data() -> Option<&'static mut PlayerGameData> { // (SHOP) pub(crate)
    let player = crate::game::player_mut()?;
    if !crate::game::plausible(crate::word!(player.player_game_data)) {
        return None;
    }
    // SAFETY: the player's game data, alive with the player (the pointer was checked); game thread.
    Some(unsafe { player.player_game_data.as_mut() })
}

/// (IMPL_ER42) The key items already told in the log (one line per item id and session).
static KEYS_TOLD: Mutex<Option<HashSet<u32>>> = Mutex::new(None);

/// (IMPL_ER28) The Elden Ring key items the character holds: (name, count), in the inventory's order. A name the
/// message tables do not give is the item's number. (IMPL_ER42) With each item's description (the message tables) and
/// its `EquipParamGoods.iconId`; the first time an item is listed, one log line `loot: key item ...`.
fn key_items(data: &PlayerGameData) -> Vec<KeyItem> {
    let mut out: Vec<KeyItem> = Vec::new();
    let mut told = KEYS_TOLD.lock().ok();
    let mut add = |out: &mut Vec<KeyItem>, raw: u32, quantity: u32| {
        let Some(cat) = cat_of_item_id(raw) else { return };
        let id = raw & 0x0FFF_FFFF;
        let name = item_name(cat, id).unwrap_or_else(|| format!("Elden Ring {} {id}", cat.name()));
        match out.iter_mut().find(|e| e.name == name) {
            Some(e) => e.count += quantity.max(1),
            None => {
                let text = item_text(cat, id);
                let icon_id = if cat == Cat::Goods { repo().and_then(|r| r.get::<EquipParamGoods>(id)).map(|row| row.icon_id() as u32) } else { None };
                if let Some(told) = told.as_mut().map(|t| t.get_or_insert_with(HashSet::new)) {
                    if told.len() < 2000 && told.insert(raw) {
                        erlog!(
                            "loot: key item {} {id} \"{name}\": icon id {} (picture MENU_ItemIcon_{:05}: the item pane shows it from the UI pack, data/er_icons.json of tools/er_icons.js, IMPL_ER43), description {}",
                            cat.name(),
                            icon_id.map_or("?".to_string(), |i| i.to_string()),
                            icon_id.unwrap_or(0),
                            text.as_ref().map_or("none found".to_string(), |(t, file)| format!("{} characters from message file {file}: \"{}\"", t.chars().count(), t.chars().take(60).collect::<String>().replace('\n', " / "))),
                        );
                    }
                }
                out.push(KeyItem { name, count: quantity.max(1), help: text.map(|t| flowing(&t.0)).unwrap_or_default(), icon_id });
            }
        }
    };
    let items = &data.equipment.equip_inventory_data.items_data;
    for entry in items.current_key_entries().iter().filter_map(|e| e.as_option()) {
        // (ONLINEITEMS fix) a hidden row is not listed (Elden Ring's own inventory does not show it)
        if !shown_by_er(entry.item_id.into_inner()) {
            continue;
        }
        add(&mut out, entry.item_id.into_inner(), entry.quantity);
        if out.len() >= 400 {
            return out;
        }
    }
    // (SHOP, the user 2026-10-23: "keep quest items only, and make them and all other quest items show up in the KH3
    // inventory menu") the goods of the NORMAL inventory that the mod leaves Elden Ring's (the gate passes them: tools
    // such as the whistle and the lantern, (ONLINEITEMS) every online item (the fingers, the effigies, the Furlcalling
    // Finger Remedy ...: lootmap `online_item`), anything no rule names) are listed with the key items
    for entry in items.normal_entries().iter().filter_map(|e| e.as_option()) {
        let raw = entry.item_id.into_inner();
        if cat_of_item_id(raw) != Some(Cat::Goods) {
            continue;
        }
        let item = describe(Cat::Goods, raw & 0x0FFF_FFFF);
        // (ONLINEITEMS fix) never a row Elden Ring's own inventory hides (goods 166 and the like)
        if !item.known || !crate::lootmap::listed_on_kh_screens(&item) || !matches!(rules().classify(&item, &ctx(Source::Unknown)).give, Give::Pass) {
            continue;
        }
        add(&mut out, raw, entry.quantity);
        if out.len() >= 400 {
            break;
        }
    }
    out
}

/// (IMPL_ER28) The item ids and inventory places the removal never touches: what the character wears (armaments,
/// arrows, armour, talismans) and what sits in a quick slot or the pouch.
pub(crate) fn protected(data: &PlayerGameData) -> (HashSet<u32>, HashSet<u32>) { // (SHOP) pub(crate)
    let worn = &data.equipment.equipment_entries;
    // SAFETY: `ChrAsmEquipEntries` is a plain struct of 32-bit item ids (SDK layout).
    let ids: HashSet<u32> = unsafe { std::slice::from_raw_parts(worn as *const _ as *const u32, std::mem::size_of_val(worn) / 4) }.iter().copied().filter(|v| *v != u32::MAX && *v != 0).collect();
    let slots = &data.equipment.equip_item_data;
    let handles: HashSet<u32> = slots.quick_slots.iter().chain(slots.pouch_slots.iter()).chain(std::iter::once(&slots.great_rune)).map(|s| s.gaitem_handle.0).filter(|h| *h != 0 && *h != u32::MAX).collect();
    (ids, handles)
}

/// (IMPL_ER28) TAKES `count` OF AN ITEM OUT OF THE NORMAL INVENTORY BY HAND (the pinned SDK has no "remove item"):
/// a stack's quantity is lowered; an entry that reaches 0 is emptied (its item id becomes the game's "no item") and
/// the id's place in the inventory's lookup table is given up or moved to another entry of the same id. Entries
/// with an instance record (armaments, armour, talismans, Ashes of War) only with `instances`: the record is left
/// behind in the game's instance table. Returns how many were removed. UNTESTED IN THE GAME.
pub(crate) fn remove_items(inventory: &mut EquipInventoryData, raw: u32, count: u32, instances: bool) -> u32 { // (SHOP) pub(crate)
    let Ok(item_id) = ItemId::try_from(raw) else { return 0 };
    if cat_of_item_id(raw) != Some(Cat::Goods) && !instances {
        return 0;
    }
    let key_capacity = inventory.items_data.key_items_capacity;
    let (mut left, mut emptied) = (count, 0u32);
    for entry in inventory.items_data.normal_entries_mut().iter_mut() {
        if left == 0 {
            break;
        }
        let Some(e) = entry.as_option_mut().filter(|e| e.item_id.into_inner() == raw) else { continue };
        let take = left.min(e.quantity.max(1));
        left -= take;
        if e.quantity > take {
            e.quantity -= take;
        } else {
            e.quantity = 0;
            e.gaitem_handle.0 = 0;
            // SAFETY: `ItemId` is a transparent 32-bit value; all ones is the game's "no item" (`OptionalItemId::NONE`),
            // which is what makes an inventory entry empty (SDK: `IsEmpty for EquipInventoryDataListEntry`).
            unsafe { *(&mut e.item_id as *mut ItemId as *mut u32) = u32::MAX };
            emptied += 1;
        }
    }
    if emptied > 0 {
        inventory.total_item_entry_count = inventory.total_item_entry_count.saturating_sub(emptied);
        inventory.items_data.remove_item_id_mapping(item_id);
        // another entry of the same id: the lookup table points at it again (slot = its place after the key items)
        let again = inventory.items_data.normal_entries().iter().position(|e| e.as_option().is_some_and(|e| e.item_id.into_inner() == raw));
        if let Some(place) = again {
            inventory.items_data.update_item_id_mapping(item_id, (key_capacity as usize + place) as i16);
        }
    }
    count - left
}

/// (IMPL_ER28) `loot.purge`: the non-key Elden Ring items already held go, once per character.
fn purge_inventory(data: &mut PlayerGameData, level: i32) {
    let (Some(dir), Some(key)) = (crate::save::dir(), crate::save::current_key()) else {
        erlog!("loot: purge: NOT DONE: the character has no Sora save yet (no key to remember the purge by)");
        return;
    };
    let marker = dir.join(format!("{key}.purged.txt"));
    if marker.exists() {
        erlog!("loot: purge: already done for this character ({}): nothing removed", marker.display());
        return;
    }
    let (worn, slotted) = protected(data);
    let inventory = &mut data.equipment.equip_inventory_data;
    // what is there, entry by entry (collected first: nothing is changed while the list is read)
    let entries: Vec<(u32, u32, u32)> = inventory.items_data.normal_entries().iter().filter_map(|e| e.as_option()).map(|e| (e.item_id.into_inner(), e.quantity, e.gaitem_handle.0)).collect();
    let (mut lines, mut removed, mut kept) = (Vec::new(), 0u32, 0u32);
    let mut done: HashSet<u32> = HashSet::new();
    for (raw, quantity, handle) in entries {
        let Some(cat) = cat_of_item_id(raw) else {
            kept += 1;
            continue;
        };
        let item = describe(cat, raw & 0x0FFF_FFFF);
        let decision = rules().classify(&item, &ctx(Source::Unknown));
        let name = item_name(cat, item.id).unwrap_or_default();
        let verdict = if matches!(decision.give, Give::Pass) {
            Some(format!("KEPT: it passes by {}", decision.rule))
        } else if worn.contains(&raw) {
            Some("KEPT: the character wears it".to_string())
        } else if slotted.contains(&handle) {
            Some("KEPT: it is in a quick slot or the pouch".to_string())
        } else if cat != Cat::Goods && level < 3 {
            Some("KEPT: an item with an instance record (loot.purge = 3 removes these)".to_string())
        } else {
            None
        };
        match verdict {
            Some(why) => {
                kept += 1;
                lines.push(format!("{} {} \"{name}\" x{quantity}: {why}", cat.name(), item.id));
            }
            None if level < 2 => {
                removed += 1;
                lines.push(format!("{} {} \"{name}\" x{quantity}: WOULD BE REMOVED (by {})", cat.name(), item.id, decision.rule));
            }
            None => {
                // (several entries of one id, e.g. two swords: each call takes one entry's worth)
                let gone = remove_items(inventory, raw, quantity.max(1), true);
                removed += (gone > 0) as u32;
                done.insert(raw);
                lines.push(format!("{} {} \"{name}\" x{quantity}: REMOVED {gone} (by {})", cat.name(), item.id, decision.rule));
            }
        }
    }
    erlog!("loot: purge (loot.purge = {level}): {} entr(ies) {}, {kept} kept{}", removed, if level < 2 { "WOULD BE REMOVED (nothing was changed: 2 removes goods, 3 everything)" } else { "REMOVED from the Elden Ring inventory (IRREVERSIBLE once the game saves)" }, if level >= 2 { format!("; list: {}", marker.display()) } else { String::new() });
    for line in &lines {
        erlog!("loot: purge:   {line}");
    }
    if level >= 2 {
        if let Err(e) = std::fs::write(&marker, format!("loot.purge = {level}: removed from the Elden Ring inventory, once\n{}\n", lines.join("\n"))) {
            erlog!("loot: purge: the marker {} could not be written ({e}): the purge would run again", marker.display());
        }
    }
}

/// The gate is gone with the renderer's other inline hooks.
pub fn uninstalled() {
    GATE_ORIGINAL.store(0, Ordering::SeqCst);
    GATE_ON.store(false, Ordering::SeqCst);
}

/// The buffer the game's item-give function takes.
#[repr(C)]
#[derive(Clone, Copy)]
struct GiveEntry {
    item: u32,
    quantity: u32,
    a: i32,
    b: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct GiveBuffer {
    count: u32,
    entries: [GiveEntry; 10],
}

type GiveFn = unsafe extern "C" fn(usize, *const GiveBuffer, usize, i32) -> usize;

/// The table of an item id as the give function gets it (bits 28 .. 31).
pub fn cat_of_item_id(item: u32) -> Option<Cat> {
    Some(match item >> 28 {
        0 => Cat::Weapon,
        1 => Cat::Protector,
        2 => Cat::Accessory,
        4 => Cat::Goods,
        8 => Cat::Gem,
        _ => return None,
    })
}

/// (IMPL_ER28) The return addresses of the gate's callers as RVAs of the game's executable (the nearest first):
/// the call site tells a chest from a corpse from a gift once the sheet names the addresses.
fn callers() -> Vec<u32> {
    let mut frames = [std::ptr::null_mut::<core::ffi::c_void>(); 8];
    // SAFETY: the system's stack walk into a buffer of ours; frame 0 is this function's caller (the hook).
    let n = unsafe { windows_sys::Win32::System::Diagnostics::Debug::RtlCaptureStackBackTrace(1, frames.len() as u32, frames.as_mut_ptr(), std::ptr::null_mut()) } as usize;
    let base = crate::entry::exe_base();
    // (PORT) told in the 2.7.1.0 terms the sheet's gate.callers row is written in (crate::gamever::CALLERS)
    let version = crate::gamever::current();
    frames[..n.min(8)].iter().map(|f| *f as usize).filter(|f| *f > base && *f - base < 0x1000_0000).map(|f| crate::gamever::caller_as_2710(version, (f - base) as u32)).take(5).collect()
}

/// (SHOP-4) The map name of a block id (`mAA_BB_CC_00`: the area, its block, its region), as the shop sheet writes lot maps.
pub(crate) fn map_name(block: i32) -> String {
    let b = block as u32;
    format!("m{:02}_{:02}_{:02}_00", (b >> 24) & 0xFF, (b >> 16) & 0xFF, (b >> 8) & 0xFF)
}

/// (SHOP-4) Sets an event flag (the SDK's own writer; game thread). False: the flag manager is not there.
pub(crate) fn set_event_flag(flag: u32) -> bool {
    // SAFETY: game thread; the singleton lives as long as the game; the SDK finds the flag's block or does nothing.
    let Ok(man) = (unsafe { eldenring::cs::CSEventFlagMan::instance_mut() }) else { return false };
    if man.virtual_memory_flag.flag_blocks.is_null() {
        return false;
    }
    man.virtual_memory_flag.set_flag(flag, true);
    true
}

/// (SHOP-4) The satisfy rule of the gate for one entry: `Ok(Some(flags))` = convert and set these flags; `Ok(None)` = no
/// script waits on it; `Err(line)` = it waits but this pickup's flag cannot be told: the decision became a pass.
fn satisfy_check(decision: Decision, raw: u32) -> (Decision, Result<Option<Vec<u32>>, String>) {
    if matches!(decision.give, Give::Pass) {
        return (decision, Ok(None));
    }
    let Some(cat) = cat_of_item_id(raw) else { return (decision, Ok(None)) };
    let item = describe(cat, raw & 0x0FFF_FFFF);
    let t = crate::lootmap::item_type(cat);
    let rules = kh_host::shop::rules();
    let Some(row) = rules.satisfy.get(&(t, item.id)).or_else(|| rules.satisfy.get(&(t, item.base_id()))) else { return (decision, Ok(None)) };
    let map = map_name(BLOCK.load(Ordering::Relaxed));
    match row.pickup_flags(&map) {
        Some(flags) => (decision, Ok(Some(flags))),
        None => (Decision { give: Give::Pass, rule: "shop.script_keep".to_string(), tier: 0, why: String::new(), one: false }, Err(format!("  a script waits on its obtained flag but this pickup's flag cannot be told in {map} ({} lot flag(s)): it stays Elden Ring's", row.flags.len()))),
    }
}

/// THE GATE. Fail-safe at every step: a buffer that does not look like one, a second entry into the hook, an item
/// that is not positively mapped: the call goes to the game unchanged.
unsafe extern "C" fn give_hook(this: usize, buffer: *const GiveBuffer, out: usize, flag: i32) -> usize {
    let original: GiveFn = std::mem::transmute(GATE_ORIGINAL.load(Ordering::Relaxed));
    let (on, log) = (GATE_ON.load(Ordering::Relaxed), GATE_LOG.load(Ordering::Relaxed));
    if !(on || log) || (buffer as usize) < 0x10000 || IN_GATE.swap(true, Ordering::SeqCst) {
        return original(this, buffer, out, flag);
    }
    GATE_CALLS.fetch_add(1, Ordering::Relaxed);
    let count = (*buffer).count;
    let mut kept = GiveBuffer { count: 0, entries: [GiveEntry { item: 0, quantity: 0, a: 0, b: 0 }; 10] };
    let mut changed = false;
    if (1..=10).contains(&count) {
        let from = callers();
        let by_caller = rules().caller_source(&from);
        let mut lines = Vec::new();
        for k in 0..count as usize {
            let entry = (*buffer).entries[k];
            let decision = match cat_of_item_id(entry.item) {
                Some(cat) => {
                    let item = describe(cat, entry.item & 0x0FFF_FFFF);
                    let bits = index().get(&index_key(cat, item.id)).copied().unwrap_or(0);
                    // (PU) a chest (the body's chest animation just before), the call site, an Elden Ring window (a purchase), the lot tables
                    let (source, how) = rules().gate_source(seen_within(&CHEST_SEEN, CHEST_WINDOW_MS.load(Ordering::Relaxed)), by_caller, seen_within(&MENU_SEEN, 500), bits & 1 != 0, bits & 2 != 0, bits & 4 != 0);
                    let decision = rules().classify(&item, &Ctx { unit: gate_unit(), ..ctx(source) });
                    lines.push(format!("x{} [source {}{how}] {}", entry.quantity, source.name(), tell_item(&item, &decision)));
                    decision
                }
                None => {
                    lines.push(format!("x{} item {:#x}: no table: PASS", entry.quantity, entry.item));
                    Decision { give: Give::Pass, rule: "unknown".to_string(), tier: 0, why: String::new(), one: false }
                }
            };
            // (SHOP-4) an item whose "obtained" flag a script waits on: converted AND that pickup's flag set (as the game's own
            // pickup does); when the flag of this pickup cannot be told, it stays Elden Ring's
            let (decision, satisfy) = satisfy_check(decision, entry.item);
            if let Some(why) = satisfy.as_ref().err() {
                lines.push(why.clone());
            }
            let quantity = if decision.one { 1 } else { entry.quantity.clamp(1, 999) };
            let swallow = on
                && match &decision.give {
                    Give::Pass => false,
                    Give::Kh { item_id, .. } => kh_host::items::give(*item_id, quantity),
                    Give::Munny(n) => {
                        GATE_MUNNY.fetch_add((*n).max(0) as u32 * quantity, Ordering::Relaxed);
                        true
                    }
                    Give::Reward(what) => {
                        if let Ok(mut owed) = GATE_REWARDS.lock() {
                            for _ in 0..quantity.min(5) {
                                owed.push(what.clone());
                            }
                        }
                        true
                    }
                    Give::Nothing => true,
                };
            if swallow {
                if let Ok(Some(flags)) = &satisfy {
                    let set: Vec<String> = flags.iter().map(|f| format!("{f}{}", if set_event_flag(*f) { "" } else { " (NOT WRITTEN)" })).collect();
                    lines.push(format!("  its obtained flag(s) {} SET (a script waits on them; sheet sora-er-shop required.satisfy)", set.join(", ")));
                }
                changed = true;
                GATE_SWALLOWED.fetch_add(1, Ordering::Relaxed);
            } else {
                kept.entries[kept.count as usize] = entry;
                kept.count += 1;
            }
        }
        if let Ok(mut told) = GATE_LINES.lock() {
            if told.len() < 64 {
                let block = BLOCK.load(Ordering::Relaxed) as u32;
                told.push(format!("{} item(s){} | block m{:02}_{:02}_{:02}_{:02} depth {} | arg {flag} | callers {}: {}", count, if changed { format!(", {} kept for the game", kept.count) } else { String::new() }, block >> 24, (block >> 16) & 0xFF, (block >> 8) & 0xFF, block & 0xFF, DEPTH.load(Ordering::Relaxed), from.iter().map(|r| format!("{r:#x}")).collect::<Vec<_>>().join(" "), lines.join(" | ")));
            }
        }
    }
    let result = if changed { original(this, &kept, out, flag) } else { original(this, buffer, out, flag) };
    IN_GATE.store(false, Ordering::SeqCst);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (IMPL_ER42) The game's markup is removed from a description; lines and paragraphs stay.
    #[test]
    fn descriptions_lose_the_games_markup() {
        let raw = "Used to break the seals of <font color=\"#C0B194\">imp statues</font>.\r\n\r\n\r\nPress <?keyicon@31?> to use.  \r\n<img src='img://SB_ERR'>\n";
        assert_eq!(plain_text(raw), "Used to break the seals of imp statues.\n\nPress  to use.");
        assert_eq!(plain_text("  plain  "), "plain");
        assert_eq!(plain_text("<font>\n\n</font>"), "");
        assert_eq!(plain_text("a > b"), "a > b", "a lone > is text");
        assert_eq!(flowing(&plain_text(raw)), "Used to break the seals of imp statues. Press  to use.");
    }

    /// The gate's view of an item id: the table in the top four bits, the row id below.
    #[test]
    fn an_item_id_carries_its_table() {
        assert_eq!(cat_of_item_id(0x4000_2774), Some(Cat::Goods));
        assert_eq!(cat_of_item_id(0x0010_0000), Some(Cat::Weapon));
        assert_eq!(cat_of_item_id(0x1000_9C40), Some(Cat::Protector));
        assert_eq!(cat_of_item_id(0x2000_03E8), Some(Cat::Accessory));
        assert_eq!(cat_of_item_id(0x8000_2710), Some(Cat::Gem));
        assert_eq!(cat_of_item_id(0x3000_0000), None);
        assert_eq!(cat_of_item_id(0xFFFF_FFFF), None);
        assert_eq!(std::mem::size_of::<GiveBuffer>(), 4 + 10 * 16);
        assert_eq!(index_key(Cat::Weapon, 9000110), (Cat::Weapon, 9000000));
        assert_eq!(index_key(Cat::Goods, 9000110), (Cat::Goods, 9000110));
    }
}
