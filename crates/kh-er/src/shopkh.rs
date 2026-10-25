//! (SHOP) THE KH SHOP IN ELDEN RING (the user, 2026-10-23: "shops need to be replaced with our own Kingdom Heart's styled
//! shops ... by shops I mean shop menus"; "remove all online items, replace them with kingdom hearts consumable items";
//! "just disable talking to the blacksmith at this point until we implement a smithing menu"). State note:
//! build/sora_tmp/party/IMPL_SHOP_STATE.md. UNTESTED IN THE GAME.
//!
//! ONCE A FRAME (sora.rs, after the talk adapter):
//! - THE MERCHANTS: talkkh/shop.rs (the talk hooks) leaves a request when a merchant's script asks for its buy or sell
//!   window (which then never opens); here the KH shop (kh-host hud_shop.rs, kh-ui shop.rs) is opened for it: the
//!   merchant's name from his NpcParam, the SHOP TIER of the area the player stands in (the loot sheet's region depth of
//!   the map block -> sheet sora-er-shop `shop.area_tier`), the BUY or SELL tab as the script asked. When the KH shop has
//!   closed the request is done: the script reads its window as closed and its talk goes on (the talk list again).
//!   If the KH shop cannot open within 3 s (another menu of ours is up, the UI pack lacks the camp menu's movie) the
//!   request is given up the same way (logged): the merchant's talk goes on, Elden Ring's window still never opens.
//! - THE SMITHS: the talk hooks refuse their prompt (`npc.smith_talk = 0`, default); here the keys and the sheet's list
//!   reach the hooks.
//! - ONLINE ITEMS ALREADY HELD (a load, an old save; `shop.online_convert`, default on): once a second the normal
//!   inventory is read; an item of the "Multiplayer Items" group (sheet `online.sort_groups`) is taken out of it with the
//!   mod's own removal (loot.rs `remove_items`, as `loot.purge` / `shop.kh = 2` do) and ONE KH consumable of the area's
//!   stock is handed to Sora through the one door (`kh_host::items::give`: the KH stock, the khsave). One in a quick slot
//!   or worn is kept (logged once): the game's slot would point at nothing.
//!
//! KEYS: `shop.menu` (1; 0 = Elden Ring's own buy / sell windows), `npc.smith_talk` (0; 1 = the smiths can be talked to:
//! Elden Ring's reinforcement menu), `shop.online_convert` (1). Logs: `shop:` lines.

use std::collections::HashSet;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};

use fromsoftware_shared::FromStatic;
use kh_host::hud::ShopOpen;
use kh_ui::shop::ShopTab;

use crate::control::Control;
use crate::erlog;
use crate::lootmap::{Cat, LootRules};
use crate::talkkh::hooks::{with_shared, COMPLETE};

/// The loot sheet's region rows (the depth of a map block), read once.
static LOOT: OnceLock<LootRules> = OnceLock::new();
/// The region depth of the last block the sheet places (a cave keeps the depth of the land around it).
static DEPTH: AtomicU8 = AtomicU8::new(0);

#[derive(Default)]
struct State {
    told_rules: bool,
    /// Seconds a request has waited for the KH shop to open.
    wait: f32,
    online_clock: f32,
    /// Online items kept (in a slot / worn): told once each.
    kept_told: HashSet<u32>,
    was: Option<(bool, bool)>,
    /// (SHOP-3) The repair's state told in the log (goods id -> the last thing said).
    restore_told: std::collections::HashMap<u32, String>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn loot() -> &'static LootRules {
    LOOT.get_or_init(LootRules::load)
}

/// The talk hooks must run for the shop or the smiths (even with `talk.kh = 0`).
pub fn wants_talk_hooks(control: &Control) -> bool {
    COMPLETE.load(Ordering::Relaxed) && (control.flag("shop.menu") != Some(false) || control.flag("npc.smith_talk") != Some(true))
}

/// The shop tier of the area the player stands in now.
pub fn current_tier() -> u8 {
    kh_host::shop::rules().tier_of_depth(DEPTH.load(Ordering::Relaxed))
}

/// ONCE A FRAME (game thread). `cutscene`: a real-time cutscene plays (no shop opens in one).
pub fn frame(control: &Control, sora: &kh_core::Sora, cutscene: bool, dt: f32) {
    let rules = kh_host::shop::rules();
    let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let st = guard.get_or_insert_with(State::default);
    let menu_on = control.flag("shop.menu") != Some(false);
    let smith_block = control.flag("npc.smith_talk") != Some(true);
    if !st.told_rules {
        st.told_rules = true;
        erlog!("shop: sheet sora-er-shop: {} stock row(s), tiers {:?} by depth, markup {:?}, sell {} %, kill munny x{:?} (bosses x{}), online sort groups {:?}, {} smith talk(s){}", rules.stock.len(), rules.area_tier, rules.markup, (rules.sell_share * 100.0).round(), rules.kill_scale, rules.boss_scale, rules.online_groups, rules.smith_talks.len(), if rules.problems.is_empty() { String::new() } else { format!(" | PROBLEM ROW(S): {}", rules.problems.join("; ")) });
    }
    if st.was != Some((menu_on, smith_block)) {
        st.was = Some((menu_on, smith_block));
        erlog!("shop: merchants' windows {}; smiths {}{}", if menu_on { "-> the KH shop (shop.menu)" } else { "are Elden Ring's (shop.menu = 0)" }, if smith_block { "cannot be talked to (npc.smith_talk = 0)" } else { "can be talked to (npc.smith_talk = 1)" }, if COMPLETE.load(Ordering::Relaxed) { "" } else { " | THE TALK FUNCTIONS ARE NOT DETOURED: none of it works" });
    }
    // the area: the map block the player stands in
    if let Some(player) = crate::game::player_mut() {
        let block: i32 = player.current_block_id.into();
        if let Some(depth) = loot().depth_of_block(block) {
            DEPTH.store(depth, Ordering::Relaxed);
        }
    }
    // the hooks' side: keys, the sheet's numbers; the request and the notes
    let facts = with_shared(|s| {
        s.shop.menu_on = menu_on;
        s.shop.smith_block = smith_block;
        if s.shop.smith_ids != rules.smith_talks {
            s.shop.smith_ids = rules.smith_talks.clone();
        }
        s.shop.numbers = Some(rules.talk);
        (s.shop.request.clone(), std::mem::take(&mut s.shop.notes))
    });
    let Some((request, notes)) = facts else { return };
    for n in notes {
        erlog!("shop: {n}");
    }
    let mut done: Option<String> = None;
    match request {
        Some(r) if !r.opened => {
            if cutscene {
                done = Some("a cutscene began".into());
            } else {
                let tier = current_tier();
                let title = crate::talkkh::game::npc_of(r.ins).map(|(_, name)| name).unwrap_or_default();
                let open = ShopOpen { title: title.clone(), tier, tab: if r.sell { ShopTab::Sell } else { ShopTab::Buy } };
                match crate::hud::with_hud(|hud| hud.open_shop(open, sora)) {
                    Some(true) => {
                        st.wait = 0.0;
                        let serial = r.serial;
                        with_shared(|s| {
                            if let Some(q) = s.shop.request.as_mut().filter(|q| q.serial == serial) {
                                q.opened = true;
                            }
                        });
                        erlog!("shop: the KH shop opens for talk {} ({}): {} tab, shop tier {tier} (region depth {}), Elden Ring's rows {}..{} not used", r.talk_id, if title.is_empty() { "no name" } else { &title }, if r.sell { "SELL" } else { "BUY" }, DEPTH.load(Ordering::Relaxed), r.rows.0, r.rows.1);
                    }
                    _ => {
                        st.wait += dt;
                        if st.wait > 3.0 {
                            done = Some("the KH shop could not open (another menu is up, or no HUD)".into());
                        }
                    }
                }
            }
        }
        Some(_) => {
            let closed = crate::hud::with_hud(|hud| hud.take_shop_closed() || !hud.shop_open()).unwrap_or(true);
            if closed {
                done = Some("closed".into());
            }
        }
        None => st.wait = 0.0,
    }
    if let Some(why) = done {
        st.wait = 0.0;
        with_shared(|s| s.shop.done(&why));
        if why != "closed" {
            erlog!("shop: the merchant's window is given up: {why}");
        }
    }
    // ---- online items already held: once a second
    st.online_clock += dt;
    if st.online_clock >= 1.0 && control.flag("shop.online_convert") != Some(false) {
        st.online_clock = 0.0;
        convert_online(st);
    }
    if st.online_clock == 0.0 && control.flag("shop.online_restore") != Some(false) {
        restore_waited(st);
    }
}

/// (SHOP-3, the user 2026-10-24: "it is impossible to exit the church at the beginnign because you no longer get the finger,
/// fix this") THE REPAIR OF WHAT THE OLDER BUILD TOOK. Once per character (marker `<save dir>/<key>.shop_restored.txt`),
/// for each kept online item something waits on (sheet `online.wait_flags`, `restore` true: no event ever takes it away):
/// when the player does not hold it and either one of its "obtained" flags is on (he had it: the old build took it) or he
/// stands in a map its lot lies in and that map waits on it (the chapel of the start: event 10012504 of m10_01 enables
/// the exit 10011540 once flag 60210 is on), ITS FLAGS ARE SET (the waiting event goes on at once: the door opens) and
/// the item is given back through the game's own give function (loot.rs `give_er_goods`, as soon as a real give of
/// the session has shown its context); one KH consumable that stood in for it (the shop's replacement rule) is taken
/// back if he holds one. UNTESTED IN THE GAME.
fn restore_waited(st: &mut State) {
    let rules = kh_host::shop::rules();
    let (Some(dir), Some(key)) = (crate::save::dir(), crate::save::current_key()) else { return };
    let marker = dir.join(format!("{key}.shop_restored.txt"));
    let done: Vec<String> = std::fs::read_to_string(&marker).unwrap_or_default().split_whitespace().map(str::to_string).collect();
    let mark = |token: String| {
        let mut all = done.clone();
        all.push(token);
        let _ = std::fs::write(&marker, all.join(" "));
    };
    let Some(player) = crate::game::player_mut() else { return };
    let block: i32 = player.current_block_id.into();
    let map_now = crate::loot::map_name(block);
    let Some(data) = crate::loot::player_data() else { return };
    let items = &data.equipment.equip_inventory_data.items_data;
    let held: HashSet<u32> = items.normal_entries().iter().chain(items.current_key_entries().iter()).filter_map(|e| e.as_option()).map(|e| e.item_id.into_inner()).collect();
    // SAFETY: game thread; the singleton lives as long as the game; the SDK resolves the flag blocks itself.
    let Some(flags) = (unsafe { eldenring::cs::CSEventFlagMan::instance_mut() }).ok().filter(|m| !m.virtual_memory_flag.flag_blocks.is_null()) else { return };
    // (SHOP-4, the user: "for the chapel door, can't we just remove the item requirement to open it?") THE EARLY LOCAL REPAIR
    // of what the SHOP build converted: an online item only whose "obtained" flag an event of the SAME MAP waits on (sheet
    // required.satisfy `local`: the chapel's exit, flag 60210 of the Wizened Finger): in that map, its flag off and the item
    // not held, the flag is SET (the event goes on as the game's own new game plus has it: the door opens; the finger itself is
    // not needed) and the KH item the gate gives for it is handed over (as if it had been picked up). Once per character.
    for (&(t, id), row) in rules.satisfy.iter().filter(|(_, r)| !r.local.is_empty()) {
        let token = format!("s{t}:{id}");
        if t != 3 || done.contains(&token) || held.contains(&(0x4000_0000 | id)) {
            continue;
        }
        let item = crate::loot::describe(Cat::Goods, id);
        if !rules.is_online_group(item.sort_group) {
            continue; // (the other local items were converted by every earlier build: their flags are set at the gate from now on)
        }
        let Some(&(flag, _)) = row.local.iter().find(|(_, m)| *m == map_now) else { continue };
        if flags.virtual_memory_flag.get_flag(flag) {
            continue;
        }
        flags.virtual_memory_flag.set_flag(flag, true);
        let decision = loot().classify(&item, &crate::lootmap::Ctx::of(crate::lootmap::Source::World, DEPTH.load(Ordering::Relaxed)));
        let given = match &decision.give {
            crate::lootmap::Give::Kh { item_id, .. } => format!("{} handed over in its place", if kh_host::items::give(*item_id, 1) { kh_host::items::by_id(*item_id).map_or("?", |i| i.name.as_str()) } else { "NOTHING" }),
            other => format!("nothing handed over ({other:?})"),
        };
        let name = crate::loot::item_name(Cat::Goods, id).unwrap_or_else(|| format!("goods {id}"));
        erlog!("shop: repair: {map_now}: flag {flag} SET: what waits on \"{name}\" goes on without it (the chapel's exit opens); {given}");
        mark(token);
        return;
    }
    for w in rules.wait.iter().filter(|w| w.restore && !done.contains(&w.goods.to_string())) {
        let flag_on = |f: u32| flags.virtual_memory_flag.get_flag(f);
        if !w.restore_due(held.contains(&(0x4000_0000 | w.goods)), &flag_on, &map_now) {
            continue;
        }
        let here = w.maps.iter().any(|m| *m == map_now);
        let name = crate::loot::item_name(Cat::Goods, w.goods).unwrap_or_else(|| format!("goods {}", w.goods));
        for f in &w.flags {
            if !flags.virtual_memory_flag.get_flag(*f) {
                flags.virtual_memory_flag.set_flag(*f, true);
                erlog!("shop: repair: flag {f} of \"{name}\" SET (what waits on it goes on: {})", if here { format!("its map {map_now} waits on it") } else { "it was obtained before".to_string() });
            }
        }
        if !crate::loot::can_give_er() {
            let said = "waiting for a real item give of this session (its context) to give the item back".to_string();
            if st.restore_told.get(&w.goods) != Some(&said) {
                erlog!("shop: repair: \"{name}\" (goods {}) {said}; its flags are set", w.goods);
                st.restore_told.insert(w.goods, said);
            }
            continue;
        }
        match crate::loot::give_er_goods(w.goods, 1) {
            Ok(()) => {
                // one KH consumable that stood in for it (the replacement rule, the tier now first)
                let now = current_tier();
                let taken = std::iter::once(now).chain(1..=rules.top_tier()).filter_map(|t| rules.replacement(t, w.goods)).find(|o| kh_host::items::held(&o.key) > 0).map(|o| (kh_host::items::take(o.item_id, 1), o.name));
                mark(w.goods.to_string());
                erlog!("shop: repair: \"{name}\" (goods {}) GIVEN BACK through the game's give (it is kept from now on); {}; marker {}", w.goods, match taken { Some((_, n)) => format!("one {n} that stood in for it taken back"), None => "no KH consumable of its replacement is held: none taken".to_string() }, marker.display());
                return; // (one a second)
            }
            Err(e) => {
                let said = format!("NOT given back ({e})");
                if st.restore_told.get(&w.goods) != Some(&said) {
                    erlog!("shop: repair: \"{name}\" (goods {}) {said}", w.goods);
                    st.restore_told.insert(w.goods, said);
                }
            }
        }
    }
}

/// Takes every online item out of the Elden Ring inventory and hands Sora one KH consumable of the area for each kind.
fn convert_online(st: &mut State) {
    let rules = kh_host::shop::rules();
    let Some(data) = crate::loot::player_data() else { return };
    let (worn, slotted) = crate::loot::protected(data);
    let inventory = &mut data.equipment.equip_inventory_data;
    let entries: Vec<(u32, u32, u32)> = inventory.items_data.normal_entries().iter().filter_map(|e| e.as_option()).map(|e| (e.item_id.into_inner(), e.quantity, e.gaitem_handle.0)).collect();
    for (raw, quantity, handle) in entries {
        if crate::loot::cat_of_item_id(raw) != Some(Cat::Goods) {
            continue;
        }
        let item = crate::loot::describe(Cat::Goods, raw & 0x0FFF_FFFF);
        if !item.known || !rules.is_online_group(item.sort_group) || rules.is_quest_online(item.id) /* (a quest item: kept) */ {
            continue;
        }
        let name = crate::loot::item_name(Cat::Goods, item.id).unwrap_or_else(|| format!("goods {}", item.id));
        if worn.contains(&raw) || slotted.contains(&handle) {
            if st.kept_told.insert(raw) {
                erlog!("shop: online item \"{name}\" x{quantity} KEPT: it is in a quick slot / the pouch (the game's slot would point at nothing); take it out of the slot and it goes");
            }
            continue;
        }
        let removed = crate::loot::remove_items(inventory, raw, quantity, false);
        if removed == 0 {
            continue;
        }
        let tier = current_tier();
        let given = match rules.replacement(tier, item.id) {
            Some(o) => format!("{} given ({})", o.name, if kh_host::items::give(o.item_id, 1) { "the KH stock" } else { "NOT GIVEN" }),
            None => "nothing (the sheet has no stock)".to_string(),
        };
        erlog!("shop: online item \"{name}\" (goods {}, sortGroupId {}) x{removed} taken out of the Elden Ring inventory: {given} (shop tier {tier})", item.id, item.sort_group);
    }
}
