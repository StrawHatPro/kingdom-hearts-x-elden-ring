//! (THE USER 2026-11-02: "Re-add Grave Gloveworts and Ghost Gloveworts into Elden Ring ... make them work completely with
//! our KH3 systems"; DECIDED BY THE USER the same day: "Roderika's spirit tuning opens a KH3-STYLE SCREEN, not Elden Ring's
//! menu ... The upgrade spends our KH3 inventory's Gloveworts and raises the ash's level in the game's inventory (so the
//! summoned spirit uses it)") RODERIKA'S SPIRIT TUNING, THE GAME HALF. UNTESTED IN THE GAME.
//!
//! WHY THIS ROUTE: Elden Ring's own window works off the game's inventory, where the gloveworts no longer are (the loot gate
//! hands every glovewort to the KH3 inventory, sheet sora-er-items `id.goods.10900 .. 10919`); feeding it would mean writing
//! copies of them into the game's inventory and reading back what it spent. The KH screen spends the KH3 stock directly
//! (one truth, kept by the KH save), and the one write into the game is the ash's own entry, done the way the game keeps it.
//!
//! - THE TALK: talkkh/tuning.rs swallows her script's `c1_136()` and leaves a request; [`talk_frame`] opens the KH Spirit
//!   Tuning screen (kh-ui tuning.rs through kh-host `Hud::open_tuning`, pad-first, KH3's Keyblade Forge texts and sounds) and,
//!   when it has closed, lets her script read its window as closed: her talk list comes back. Not up within 3 s or a
//!   cutscene: given up the same way. `roderika.kh = 0`: her window is Elden Ring's.
//! - THE ROWS, READ FROM THE GAME AT RUN TIME (no table of ours): every goods entry of the game's inventory (key items and
//!   normal items) whose `EquipParamGoods.goodsType` is 7 or 8 (a spirit ash). ITS LEVEL: its place in the chain of rows
//!   `reinforceGoodsId` links (the base is the row no other ash row points at; +10 = the end of the chain). THE NEXT LEVEL'S
//!   COST: its `reinforceMaterialId` row of EquipMtrlSetParam (the goods material and the count; Grave Gloveworts for the
//!   ordinary ashes, Ghost for the renowned ones, the grade the game asks per level) mapped to its KH material by the loot
//!   sheet, and its `reinforcePrice` in munny (the rune counter is Sora's munny, prize.rs). WHAT IT GIVES: the spirit's
//!   BuddyParam row (`triggerSpEffectId` = the base row's `refId_default`) names a doping SpEffect per level
//!   (`dopingSpEffect_lv0 .. lv10`); its `maxHpRate`, `physicsAttackPowerRate` and `physicsDiffenceRate` are shown, and the
//!   extra summon time per level of the Summon command's timer ([`summon_time_per_level`], by the ash's tier).
//! - AN UPGRADE (Yes on the screen; the host checks again): the ash still held at that level, the KH3 stock holds the
//!   gloveworts, Sora has the munny. Then the ash's ENTRY in the game's inventory becomes the next level's row (its item id
//!   and its goods handle `0xB0000000 | id`, the inventory's id lookup moved with it), and every quick-item / pouch slot that
//!   named it names the new row (`ChrAsmEquipEntries.quick_tems / pouch`, `EquipItemData.quick_slots / pouch_slots`); a
//!   handle of another form is refused (nothing changed). Then the HUD is charged (`Hud::charge_tuning`: the gloveworts out
//!   of the stock, the munny), and the KH OBTAINED popup's key-item watch takes a fresh baseline (the new row is not "new").
//!   The Summon command (another agent's) reads the ash and its level from the game's inventory: it sees the new level.
//!
//! KEYS: `roderika.kh` (1; 0 = Elden Ring's own window); `test.tuning` (TESTING AID, a one-shot only ever written into the
//! test profile's `er\mods_test\kh-er.ini`: a change of its value opens the screen without a talk, to test it in the game
//! without reaching Roderika). Logs: `tuning:` lines.

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

use eldenring::cs::{BuddyParam, EquipMtrlSetParam, EquipParamGoods, GaitemCategory, GaitemHandle, ItemId, OptionalItemId, PlayerGameData, ShopLineupParam, SoloParamRepository, SpEffectParam};
use fromsoftware_shared::FromStatic;
use kh_ui::tuning::{TuneCost, TuneData, TuneRow};

use crate::control::Control;
use crate::erlog;
use crate::lootmap::{Cat, Ctx, Give, LootRules, Source};

/// The goods types of a spirit ash (`EquipParamGoods.goodsType`: 7 and 8, the loot sheet's `r2.510.spirit_ash`).
pub const ASH_TYPES: [u8; 2] = [7, 8];
/// EquipMtrlSetParam's material category of a goods item (read in the game's rows: every spirit ash's set names its
/// glovewort with category 4).
const MATERIAL_GOODS: u8 = 4;
/// The goods table's bits of an item id (`0x40000000 | goods id`).
const GOODS_BITS: u32 = 0x4000_0000;

static LOOT: OnceLock<LootRules> = OnceLock::new();

fn loot() -> &'static LootRules {
    LOOT.get_or_init(LootRules::load)
}

fn repo() -> Option<&'static SoloParamRepository> {
    // SAFETY: the singleton lives as long as the game; its holder table is checked by the SDK.
    unsafe { SoloParamRepository::instance() }.ok()
}

// ---------------------------------------------------------------------------------------------------------- pure rules

/// AN ASH'S LEVEL AND ITS CHAIN'S HIGHEST, from every spirit ash row (id, `reinforceGoodsId`): the base of a chain is the
/// row no other row points at; each step of `reinforceGoodsId` is one level. id -> (level, highest level, the base's id).
pub fn chain_levels(rows: &[(u32, i32)]) -> HashMap<u32, (u8, u8, u32)> {
    let ids: HashSet<u32> = rows.iter().map(|r| r.0).collect();
    let next: HashMap<u32, u32> = rows.iter().filter(|r| r.1 > 0 && ids.contains(&(r.1 as u32))).map(|r| (r.0, r.1 as u32)).collect();
    let pointed: HashSet<u32> = next.values().copied().collect();
    let mut out = HashMap::new();
    for base in rows.iter().map(|r| r.0).filter(|id| !pointed.contains(id)) {
        let mut chain = vec![base];
        while let Some(n) = next.get(chain.last().unwrap()).copied() {
            if chain.contains(&n) || chain.len() > 32 {
                break; // (a loop in the data: what was walked counts)
            }
            chain.push(n);
        }
        let top = (chain.len() - 1).min(u8::MAX as usize) as u8;
        for (level, id) in chain.into_iter().enumerate() {
            out.insert(id, (level.min(u8::MAX as usize) as u8, top, base));
        }
    }
    out
}

/// The name without a level the game's text may carry ("Black Knife Tiche +3" -> "Black Knife Tiche").
pub fn plain_name(name: &str) -> &str {
    match name.rsplit_once(" +") {
        Some((head, level)) if !level.is_empty() && level.chars().all(|c| c.is_ascii_digit()) => head,
        _ => name,
    }
}

/// What a level gives, for the pane: its doping SpEffect's rates (HP, attack, defence) and the summon time it adds.
pub fn gives_lines(hp: f32, attack: f32, defense: f32, time: Option<f32>) -> Vec<String> {
    let mut out = vec![format!("HP x{hp:.2}"), format!("Attack x{attack:.2}"), format!("Defense x{defense:.2}")];
    if let Some(t) = time.filter(|t| *t > 0.0) {
        out.push(format!("Time +{t:.0}s"));
    }
    out
}

/// Whether an upgrade may go through now: (held, needed) of the glovewort, (munny, price).
pub fn affordable(held: u32, needed: u32, munny: i64, price: u32) -> bool {
    held >= needed && munny >= price as i64
}

/// THE SUMMON TIMER'S EXTRA TIME PER LEVEL (seconds) for an ash of this tier (THE USER 2026-11-02: "upgrading the spirit ash
/// increases the timer"): the Summon command's own rule, kh-core `command::summon::duration` (sheet sora-magic-mp rows
/// `summon.time.per_level.normal` / `.renowned`: an ordinary ash +6 s, a renowned one, Elden Ring goods type 8, +4.5 s).
/// Compiled into the core's tables, so the released DLL has it too (no file is read at run time).
pub fn summon_time_per_level(renowned: bool) -> Option<f32> {
    use kh_core::command::summon::duration;
    Some(duration(renowned, 1) - duration(renowned, 0)).filter(|t| *t > 0.0)
}

/// A renowned ash (Elden Ring goods type 8: the Ghost Glovewort ones), the Summon timer's tier.
fn renowned(goods: u32) -> bool {
    repo().and_then(|r| r.get::<EquipParamGoods>(goods).map(|g| g.goods_type() == 8)).unwrap_or(false)
}

// ---------------------------------------------------------------------------------------------------------- the game's data

/// One spirit ash row as the params describe it.
#[derive(Clone, Debug)]
struct AshRow {
    level: u8,
    top: u8,
    base: u32,
    next: Option<u32>,
    /// The next level's goods material and how many.
    material: Option<(u32, u32)>,
    price: u32,
    icon: u16,
    sort: i32,
}

/// Every spirit ash row, read once (the params do not change in a session).
fn ash_rows() -> Option<&'static HashMap<u32, AshRow>> {
    static ROWS: OnceLock<HashMap<u32, AshRow>> = OnceLock::new();
    if let Some(rows) = ROWS.get() {
        return Some(rows);
    }
    let repo = repo()?;
    let raw: Vec<(u32, i32)> = repo.rows::<EquipParamGoods>().filter(|(_, r)| ASH_TYPES.contains(&r.goods_type())).map(|(id, r)| (id, r.reinforce_goods_id())).collect();
    if raw.is_empty() {
        return None;
    }
    let levels = chain_levels(&raw);
    let mut out = HashMap::new();
    for (id, _) in &raw {
        let Some(row) = repo.get::<EquipParamGoods>(*id) else { continue };
        let (level, top, base) = levels.get(id).copied().unwrap_or((0, 0, *id));
        let next = (row.reinforce_goods_id() > 0).then_some(row.reinforce_goods_id() as u32);
        let material = repo.get::<EquipMtrlSetParam>(row.reinforce_material_id().max(0) as u32).filter(|_| row.reinforce_material_id() > 0).and_then(|m| {
            let slots = [(m.material_id01(), m.item_num01(), m.material_cate01()), (m.material_id02(), m.item_num02(), m.material_cate02()), (m.material_id03(), m.item_num03(), m.material_cate03())];
            slots.into_iter().find(|(id, n, cate)| *id > 0 && *n > 0 && *cate == MATERIAL_GOODS).map(|(id, n, _)| (id as u32, n as u32))
        });
        out.insert(*id, AshRow { level, top, base, next, material, price: row.reinforce_price().max(0) as u32, icon: row.icon_id(), sort: row.sort_id() });
    }
    erlog!("tuning: {} spirit ash rows read from EquipParamGoods ({} chains), {} spirit-tuning material(s) named by their EquipMtrlSetParam rows", out.len(), out.values().filter(|r| r.level == 0).count(), out.values().filter_map(|r| r.material.map(|m| m.0)).collect::<HashSet<_>>().len());
    // (THE USER 2026-11-03) each material's KH item shows Elden Ring's icon of the sheet's er_icon: the game's own iconId is checked
    let mut materials: Vec<u32> = out.values().filter_map(|r| r.material.map(|m| m.0)).collect::<HashSet<_>>().into_iter().collect();
    materials.sort();
    let wrong: Vec<String> = materials
        .iter()
        .filter_map(|goods| {
            let (item_id, key) = kh_material(*goods)?;
            let sheet = kh_host::items::by_id(item_id).and_then(|i| i.er_icon);
            let game = crate::ericon::goods_icon_id(*goods);
            (sheet != game).then(|| format!("{key} (goods {goods}): sheet {sheet:?}, game {game:?}"))
        })
        .collect();
    erlog!("tuning: the tuning materials' Elden Ring icons (sheet sora-item-table er_icon against EquipParamGoods.iconId): {}", if wrong.is_empty() { "all match".to_string() } else { format!("DIFFER: {}", wrong.join("; ")) });
    Some(ROWS.get_or_init(|| out))
}

/// (THE USER 2026-11-02) THE SPIRIT-TUNING MATERIALS: every goods item a spirit ash's next level asks for (the gloveworts,
/// read from the game's own rows; empty until the params are there).
pub fn tuning_materials() -> HashSet<u32> {
    ash_rows().map(|rows| rows.values().filter_map(|r| r.material.map(|m| m.0)).collect()).unwrap_or_default()
}

/// The doping SpEffect of an ash's level (its BuddyParam row by the base row's `refId_default`).
fn doping(base: u32, level: u8) -> Option<(f32, f32, f32)> {
    static BUDDY: OnceLock<HashMap<i32, u32>> = OnceLock::new();
    let repo = repo()?;
    let by_trigger = BUDDY.get_or_init(|| repo.rows::<BuddyParam>().map(|(id, b)| (b.trigger_sp_effect_id(), id)).filter(|(t, _)| *t > 0).fold(HashMap::new(), |mut m, (t, id)| {
        m.entry(t).or_insert(id);
        m
    }));
    let trigger = repo.get::<EquipParamGoods>(base)?.ref_id_default();
    let b = repo.get::<BuddyParam>(*by_trigger.get(&trigger)?)?;
    let sp = match level {
        0 => b.doping_sp_effect_lv0(),
        1 => b.doping_sp_effect_lv1(),
        2 => b.doping_sp_effect_lv2(),
        3 => b.doping_sp_effect_lv3(),
        4 => b.doping_sp_effect_lv4(),
        5 => b.doping_sp_effect_lv5(),
        6 => b.doping_sp_effect_lv6(),
        7 => b.doping_sp_effect_lv7(),
        8 => b.doping_sp_effect_lv8(),
        9 => b.doping_sp_effect_lv9(),
        _ => b.doping_sp_effect_lv10(),
    };
    let e = repo.get::<SpEffectParam>(u32::try_from(sp).ok()?)?;
    Some((e.max_hp_rate(), e.physics_attack_power_rate(), e.physics_diffence_rate()))
}

/// The KH material an Elden Ring goods material is (the loot sheet's mapping: the gloveworts' own rows).
fn kh_material(goods: u32) -> Option<(u32, String)> {
    match loot().classify(&crate::loot::describe(Cat::Goods, goods), &Ctx::of(Source::Unknown, 0)).give {
        Give::Kh { item_id, key } => Some((item_id, key)),
        _ => None,
    }
}

/// Every spirit ash entry of the game's inventory: (goods id, its row).
fn held_ashes(data: &PlayerGameData) -> Vec<(u32, &'static AshRow)> {
    let Some(rows) = ash_rows() else { return Vec::new() };
    let items = &data.equipment.equip_inventory_data.items_data;
    // (OUTFIT-UNLOCK) every key-item slot (crate::loot::all_key_entries), not only those inside the in-use length
    let mut out: Vec<(u32, &'static AshRow)> = crate::loot::all_key_entries(items)
        .iter()
        .chain(items.normal_entries().iter())
        .filter_map(|e| e.as_option())
        .map(|e| e.item_id.into_inner())
        .filter(|raw| raw >> 28 == GOODS_BITS >> 28)
        .filter_map(|raw| rows.get(&(raw & 0x0FFF_FFFF)).map(|r| (raw & 0x0FFF_FFFF, r)))
        .collect();
    out.sort_by_key(|(id, r)| (r.sort, *id));
    out.dedup_by_key(|(id, _)| *id);
    out
}

/// THE SCREEN'S ROWS from the game (the stock and the munny are filled in by the HUD).
fn tune_data(title: &str) -> Option<TuneData> {
    let data = crate::loot::player_data()?;
    let rows = held_ashes(data)
        .into_iter()
        .map(|(id, r)| {
            let time = summon_time_per_level(renowned(id));
            let name = crate::loot::item_name(Cat::Goods, id).map(|n| plain_name(&n).to_string()).unwrap_or_else(|| format!("Spirit ash {id}"));
            let cost = r.material.filter(|_| r.next.is_some()).map(|(goods, count)| {
                let (key, kh_name) = kh_material(goods).map(|(item_id, key)| (key, kh_host::items::by_id(item_id).map_or_else(String::new, |i| i.name.clone()))).unwrap_or_else(|| (String::new(), crate::loot::item_name(Cat::Goods, goods).unwrap_or_else(|| format!("goods {goods}"))));
                TuneCost { key, name: kh_name, count, owned: 0 }
            });
            let shown = if r.next.is_some() { r.level.saturating_add(1) } else { r.level };
            let gives = doping(r.base, shown).map(|(hp, atk, def)| gives_lines(hp, atk, def, time.map(|t| t * shown as f32))).unwrap_or_default();
            // (THE USER 2026-11-03) the ash's own Elden Ring icon (the shared route, ericon.rs; its row's iconId)
            TuneRow { id, icon: kh_ui::item_art::er_icon_key(r.icon as u32), name, level: r.level, max_level: r.top, cost, munny: r.price, gives }
        })
        .collect();
    Some(TuneData { title: title.to_string(), munny: 0, rows })
}

/// THE ASH'S ENTRY BECOMES THE NEXT LEVEL'S ROW (see the file header). `Err`: nothing was changed.
fn retune(data: &mut PlayerGameData, old: u32, new: u32) -> Result<String, String> {
    let (old_raw, new_raw) = (GOODS_BITS | old, GOODS_BITS | new);
    let (Ok(old_id), Ok(new_id)) = (ItemId::try_from(old_raw), ItemId::try_from(new_raw)) else { return Err("not an item id".into()) };
    let (old_handle, new_handle) = (GaitemHandle::from_parts(old, GaitemCategory::Goods), GaitemHandle::from_parts(new, GaitemCategory::Goods));
    let items = &mut data.equipment.equip_inventory_data.items_data;
    let key_cap = items.key_items_capacity as usize;
    // (OUTFIT-UNLOCK) every key-item slot (crate::loot::all_key_entries): the slot number is the same in both views
    let key_at = crate::loot::all_key_entries(items).iter().position(|e| e.as_option().is_some_and(|e| e.item_id.into_inner() == old_raw));
    let normal_at = items.normal_entries().iter().position(|e| e.as_option().is_some_and(|e| e.item_id.into_inner() == old_raw));
    let (entry, slot) = match (key_at, normal_at) {
        (Some(k), _) => (crate::loot::all_key_entries_mut(items).get_mut(k).and_then(|e| e.as_option_mut()), k),
        (None, Some(k)) => (items.normal_entries_mut().get_mut(k).and_then(|e| e.as_option_mut()), key_cap + k),
        _ => return Err(format!("goods {old} is not in the game's inventory any more")),
    };
    let Some(entry) = entry else { return Err("the entry could not be written".into()) };
    if entry.gaitem_handle.0 != old_handle.0 {
        return Err(format!("its handle {:#x} is not a goods handle of {old} ({:#x}): left alone", entry.gaitem_handle.0, old_handle.0));
    }
    entry.item_id = new_id;
    entry.gaitem_handle = new_handle;
    let slot = i16::try_from(slot).map_err(|_| "the slot is out of range".to_string())?;
    items.remove_item_id_mapping(old_id);
    items.update_item_id_mapping(new_id, slot);
    let equipment = &mut data.equipment;
    let mut slots = 0;
    for q in equipment.equipment_entries.quick_tems.iter_mut().chain(equipment.equipment_entries.pouch.iter_mut()) {
        if *q == OptionalItemId::from(old_raw) {
            *q = OptionalItemId::from(new_raw);
            slots += 1;
        }
    }
    for s in equipment.equip_item_data.quick_slots.iter_mut().chain(equipment.equip_item_data.pouch_slots.iter_mut()) {
        if s.gaitem_handle.0 == old_handle.0 {
            s.gaitem_handle = new_handle;
        }
    }
    Ok(format!("inventory slot {slot}, {slots} quick-item / pouch slot(s) moved along"))
}

/// AN UPGRADE THE SCREEN ASKED FOR: checked again against the game and the KH3 stock, then carried out. Ok: (the KH
/// material's key, how many, the munny) for the HUD's charge.
fn upgrade(id: u32, to_level: u8, munny: i64) -> Result<(String, u32, u32, String), String> {
    let rows = ash_rows().ok_or("the params are not there")?;
    let data = crate::loot::player_data().ok_or("no player")?;
    let r = rows.get(&id).ok_or_else(|| format!("goods {id} is not a spirit ash"))?;
    if r.level + 1 != to_level {
        return Err(format!("it is +{} now, +{to_level} was asked", r.level));
    }
    let next = r.next.ok_or("it is at its highest level")?;
    let (goods, count) = r.material.ok_or("its row names no material")?;
    let (_, key) = kh_material(goods).ok_or_else(|| format!("its material goods {goods} is no KH item (sheet sora-er-items)"))?;
    let held = kh_host::items::held(&key);
    if !affordable(held, count, munny, r.price) {
        return Err(format!("{key} x{count} (held {held}), {} munny (Sora has {munny})", r.price));
    }
    let how = retune(data, id, next)?;
    crate::loot::rebaseline_key_items();
    Ok((key, count, r.price, format!("goods {id} -> {next} (+{to_level}): {how}")))
}

// ---------------------------------------------------------------------------------------------------------- the merchants

/// (THE USER 2026-11-02: "make them appear where they usually would ... shops that sell them") THE MERCHANT'S OWN
/// SPIRIT-TUNING MATERIAL LINES for the KH shop: the rows the script's OpenRegularShop named (first ..= last) whose item is a
/// spirit-tuning material, released (`eventFlag_forRelease` 0 or set: the Twin Maiden Husks' after the glovewort bell
/// bearings), paid in runes (`costType` 0), with no stock limit (`sellQuantity` < 0; a limited line is told and left out:
/// its count lives in the game's shop state), at the row's own price (`value`, munny = runes). Read at run time.
pub fn merchant_lines(rows: (i32, i32)) -> Vec<kh_host::shop::Offer> {
    let (Some(repo), materials) = (repo(), tuning_materials()) else { return Vec::new() };
    static TOLD: Mutex<Option<HashSet<u32>>> = Mutex::new(None);
    let mut told = TOLD.lock().unwrap_or_else(|e| e.into_inner());
    let told = told.get_or_insert_with(HashSet::new);
    let mut out = Vec::new();
    for (id, row) in repo.rows::<ShopLineupParam>().filter(|(id, _)| (rows.0..=rows.1).contains(&(*id as i32))) {
        if loot().shop_category(row.equip_type()) != Some(Cat::Goods) || row.equip_id() <= 0 || !materials.contains(&(row.equip_id() as u32)) {
            continue;
        }
        let released = row.event_flag_for_release() == 0 || crate::bossreward::game_flag(row.event_flag_for_release()) == Some(true);
        let why = if !released {
            Some("not released yet")
        } else if row.cost_type() != 0 {
            Some("not paid in runes")
        } else if row.sell_quantity() >= 0 {
            Some("a limited stock")
        } else if row.value() <= 0 {
            Some("no price")
        } else {
            None
        };
        let kh = kh_material(row.equip_id() as u32).and_then(|(item_id, key)| kh_host::items::by_id(item_id).map(|i| (item_id, key, i.name.clone(), i.max)));
        match (why, kh) {
            (None, Some((item_id, key, name, max))) => out.push(kh_host::shop::Offer { key, item_id, name, price: row.value() as u32, max }),
            // (each row told once a session)
            (Some(why), _) if told.insert(id) => erlog!("tuning: shop line {id} (goods {}): {why}: not sold", row.equip_id()),
            (None, None) if told.insert(id) => erlog!("tuning: shop line {id} (goods {}): no KH item for it (sheet sora-er-items): not sold", row.equip_id()),
            _ => {}
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------------------- the frame

/// The talk hooks must run for Roderika's spirit tuning (even with `talk.kh = 0`).
pub fn wants_talk_hooks(control: &Control) -> bool {
    crate::talkkh::hooks::COMPLETE.load(std::sync::atomic::Ordering::Relaxed) && control.flag("roderika.kh") != Some(false)
}

/// Seconds a request may wait for the screen.
const WAIT: f32 = 3.0;

/// ONCE A FRAME (sora.rs, after the talk adapter, the KH shop and Boc): see the file header.
pub fn talk_frame(control: &Control, sora: &kh_core::Sora, cutscene: bool, dt: f32) {
    use crate::talkkh::hooks::with_shared;
    static WAITED: Mutex<f32> = Mutex::new(0.0);
    static AID: Mutex<Option<crate::control::OneShot>> = Mutex::new(None);
    let on = control.flag("roderika.kh") != Some(false);
    let aid = AID.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(|| crate::control::OneShot::new("test.tuning", control)).fired(control).is_some();
    let Some((request, notes)) = with_shared(|s| {
        s.tuning.on = on;
        if aid {
            s.tuning.test_request();
        }
        (s.tuning.request.clone(), std::mem::take(&mut s.tuning.notes))
    }) else {
        return;
    };
    for n in notes {
        erlog!("tuning: {n}");
    }
    let mut waited = WAITED.lock().unwrap_or_else(|e| e.into_inner());
    let mut done: Option<&str> = None;
    match request {
        None => *waited = 0.0,
        Some(r) if !r.opened => {
            if cutscene {
                done = Some("a cutscene began");
            } else {
                let title = crate::talkkh::game::npc_of(r.ins).map(|(_, name)| name).unwrap_or_default();
                let data = tune_data(&title);
                let rows = data.as_ref().map_or(0, |d| d.rows.len());
                match data.and_then(|d| crate::hud::with_hud(|hud| hud.open_tuning(d, sora))) {
                    Some(true) => {
                        *waited = 0.0;
                        with_shared(|s| {
                            if let Some(q) = s.tuning.request.as_mut().filter(|q| q.serial == r.serial) {
                                q.opened = true;
                            }
                        });
                        erlog!("tuning: the Spirit Tuning screen opens for talk {} ({}) in place of Elden Ring's window: {rows} spirit ash(es) held", r.talk_id, if title.is_empty() { "no name" } else { &title });
                    }
                    _ => {
                        *waited += dt;
                        if *waited > WAIT {
                            done = Some("the screen could not open (another menu is up, no HUD, or no player)");
                        }
                    }
                }
            }
        }
        Some(r) => {
            let munny = sora.status().munny as i64;
            for (id, to_level) in crate::hud::with_hud(|hud| hud.take_tuning_requests()).unwrap_or_default() {
                match upgrade(id, to_level, munny) {
                    Ok((key, count, price, line)) => {
                        crate::hud::with_hud(|hud| hud.charge_tuning(&key, count, price));
                        erlog!("tuning: POWERED UP {line}; charged {key} x{count} and {price} munny");
                    }
                    Err(why) => erlog!("tuning: goods {id} to +{to_level} REFUSED (nothing changed): {why}"),
                }
            }
            let title = crate::talkkh::game::npc_of(r.ins).map(|(_, name)| name).unwrap_or_default();
            if let Some(data) = tune_data(&title) {
                crate::hud::with_hud(|hud| hud.set_tuning_data(data));
            }
            let closed = crate::hud::with_hud(|hud| hud.take_tuning_closed() || !hud.tuning_open()).unwrap_or(true);
            if closed {
                done = Some("closed");
            } else if cutscene {
                crate::hud::with_hud(|hud| hud.close_tuning());
            }
        }
    }
    if let Some(why) = done {
        *waited = 0.0;
        with_shared(|s| s.tuning.done(why));
        if why != "closed" {
            erlog!("tuning: her spirit-tuning window is given up: {why}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (THE USER 2026-11-02) The levels come from the chain the rows link, whatever the ids: Black Knife Tiche's rows
    /// 200000 .. 200010 (regulation 2.7.2.0: each `reinforceGoodsId` the next, +10 none), a second chain, a stray row.
    #[test]
    fn ash_levels_follow_the_game_s_reinforce_chain() {
        let mut rows: Vec<(u32, i32)> = (0..10).map(|k| (200_000 + k, 200_001 + k as i32)).collect();
        rows.push((200_010, -1));
        rows.extend([(214_002, 214_003), (214_000, 214_001), (214_001, 214_002), (214_003, -1)]);
        rows.push((299_999, 123)); // points at nothing held: a chain of its own
        let l = chain_levels(&rows);
        assert_eq!((l[&200_000], l[&200_003], l[&200_010]), ((0, 10, 200_000), (3, 10, 200_000), (10, 10, 200_000)));
        assert_eq!((l[&214_000], l[&214_002], l[&214_003]), ((0, 3, 214_000), (2, 3, 214_000), (3, 3, 214_000)), "the order of the rows does not matter");
        assert_eq!(l[&299_999], (0, 0, 299_999));
        assert_eq!(chain_levels(&[(1, 2), (2, 1)]).len(), 0, "a loop with no base: no levels rather than a hang");
    }

    #[test]
    fn names_effects_and_the_affordable_check() {
        assert_eq!((plain_name("Black Knife Tiche +3"), plain_name("Black Knife Tiche"), plain_name("Ashes +x"), plain_name("Mimic Tear Ashes +10")), ("Black Knife Tiche", "Black Knife Tiche", "Ashes +x", "Mimic Tear Ashes"));
        // the doping SpEffect 290004 of regulation 2.7.2.0 rounded, no Summon sheet row
        assert_eq!(gives_lines(2.43, 1.83, 1.09, None), vec!["HP x2.43", "Attack x1.83", "Defense x1.09"]);
        assert_eq!(gives_lines(1.0, 1.0, 1.0, Some(8.0)).last().map(String::as_str), Some("Time +8s"));
        assert!(affordable(1, 1, 5600, 5600) && !affordable(0, 1, 9999, 1) && !affordable(3, 1, 5599, 5600));
    }
}
