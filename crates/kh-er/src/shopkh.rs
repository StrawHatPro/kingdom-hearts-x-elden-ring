//! (SHOP) THE KH SHOP IN ELDEN RING (the user, 2026-10-23: "shops need to be replaced with our own Kingdom Heart's styled
//! shops ... by shops I mean shop menus"; "just disable talking to the blacksmith at this point until we implement a smithing
//! menu"). State note: build/sora_tmp/party/IMPL_SHOP_STATE.md. UNTESTED IN THE GAME.
//! (ONLINEITEMS 2026-10-25, the user: "just restore the online items") Elden Ring's online items are Elden Ring's again
//! (lootmap.rs `online_item`): the conversion of held ones and the SHOP-3 / SHOP-4 repairs that lived here are gone.
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
//!
//! - (P10 MATERIALS, documentation/KH3_WEAPONS_PLAN.md 3) THE ORES OF THE MERCHANT'S OWN SMITHING STONE LINES: the KH shop
//!   also sells, at Elden Ring's own price, every line of the merchant's rows that sells a Smithing Stone (the Twin Maiden
//!   Husks' bell-bearing lines, a smith's own stones) as the KH3 ore the loot sheet makes of that stone (sora-er-items
//!   r2.300 .. r2.303: Fluorite, Damascus, Adamantite, Electrum) [`ore_lines`], the gloveworts' way (spirittune.rs
//!   `merchant_lines`): released lines only (`eventFlagForRelease`), paid in runes, an unlimited stock. Never Orichalcum or
//!   Orichalcum+ (fixed placements only, the user 2026-11-03).
//!
//! - (SHOP-TIERS, THE USER 2026-11-07: "for merchants with shops that get upgraded in the base game, allow their shows to
//!   upgrade their stock selection as well when their should would upgrade in elden ring. This means start them off with
//!   basic, limited selection") THE KH STOCK GROWS WITH THE MERCHANT'S OWN LINEUP [`lineup_growth`]: the share of the rows
//!   of the range the script opened that Elden Ring lists now (no release flag, or its `eventFlagForRelease` on: a bell
//!   bearing handed to the Twin Maiden Husks, a scroll / prayerbook handed over, a quest step), read live at every opening,
//!   so a save that progressed sees the bigger stock at once; the KH stock is cut to that share, cheapest first, the
//!   materials and the lines above always (kh-host shop_growth.rs, sheet sora-er-shop-tiers). A lineup without release flags
//!   (Kalé, most Nomadic Merchants) keeps the whole stock. A sell window (no rows) uses the merchant's own lineup.
//!
//! KEYS: `shop.menu` (1; 0 = Elden Ring's own buy / sell windows), `npc.smith_talk` (0; 1 = the smiths can be talked to:
//! Elden Ring's reinforcement menu), `shop.grow` (1; 0 = every merchant sells the whole KH stock, as before SHOP-TIERS).
//! Logs: `shop:` lines.

use std::collections::HashSet;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};

use eldenring::cs::{ShopLineupParam, SoloParamRepository};
use fromsoftware_shared::FromStatic;
use kh_host::hud::ShopOpen;
use kh_ui::shop::ShopTab;

use crate::control::Control;
use crate::erlog;
use crate::lootmap::{Cat, Ctx, Decision, Give, LootRules, Source};
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
    was: Option<(bool, bool)>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn loot() -> &'static LootRules {
    LOOT.get_or_init(LootRules::load)
}

/// (P10) The KH3 ores a Smithing Stone line may become (sheet sora-item-table: Fluorite, Damascus, Adamantite, Electrum).
pub const RESOLD_ORES: [&str; 4] = ["MAT_ITEM53", "MAT_ITEM54", "MAT_ITEM55", "MAT_ITEM58"];

/// (P10) THE ORE OF A SHOP LINE'S ITEM, from the loot sheet's decision for it as a shop's item: a Smithing Stone's own row
/// (`r2.30x`) that gives one of [`RESOLD_ORES`]; anything else (another item, Orichalcum, the ore line of an unnamed
/// reinforcement material, r2.309) is not resold here.
pub fn ore_of(decision: &Decision) -> Option<(u32, String)> {
    let stone_row = decision.rule.starts_with("r2.30") && decision.rule != "r2.309.reinforce_other";
    match &decision.give {
        Give::Kh { item_id, key } if stone_row && RESOLD_ORES.contains(&key.as_str()) => Some((*item_id, key.clone())),
        _ => None,
    }
}

/// (P10) Why a line is not resold (`None`: it is): the gloveworts' rules (spirittune.rs `merchant_lines`).
pub fn line_refused(released: bool, cost_type: u8, sell_quantity: i16, value: i32) -> Option<&'static str> {
    if !released {
        Some("not released yet")
    } else if cost_type != 0 {
        Some("not paid in runes")
    } else if sell_quantity >= 0 {
        Some("a limited stock")
    } else if value <= 0 {
        Some("no price")
    } else {
        None
    }
}

/// (P10 MATERIALS) THE MERCHANT'S OWN SMITHING STONE LINES as KH3 ores at Elden Ring's price (module text): the rows the
/// script named (`rows`), read from the game's ShopLineupParam at run time; each line refused is told once a session.
pub fn ore_lines(rows: (i32, i32)) -> Vec<kh_host::shop::Offer> {
    // SAFETY: the singleton lives as long as the game; its holder table is checked by the SDK.
    let Some(repo) = (unsafe { SoloParamRepository::instance() }).ok() else { return Vec::new() };
    static TOLD: Mutex<Option<HashSet<u32>>> = Mutex::new(None);
    let mut told = TOLD.lock().unwrap_or_else(|e| e.into_inner());
    let told = told.get_or_insert_with(HashSet::new);
    let mut out = Vec::new();
    for (id, row) in repo.rows::<ShopLineupParam>().filter(|(id, _)| (rows.0..=rows.1).contains(&(*id as i32))) {
        if loot().shop_category(row.equip_type()) != Some(Cat::Goods) || row.equip_id() <= 0 {
            continue;
        }
        let decision = loot().classify(&crate::loot::describe(Cat::Goods, row.equip_id() as u32), &Ctx::of(Source::Shop, DEPTH.load(Ordering::Relaxed)));
        let Some((item_id, key)) = ore_of(&decision) else { continue };
        let released = row.event_flag_for_release() == 0 || crate::bossreward::game_flag(row.event_flag_for_release()) == Some(true);
        match (line_refused(released, row.cost_type(), row.sell_quantity(), row.value()), kh_host::items::by_id(item_id)) {
            (None, Some(item)) => out.push(kh_host::shop::Offer { key, item_id, name: item.name.clone(), price: row.value() as u32, max: item.max }),
            (Some(why), _) if told.insert(id) => erlog!("shop: line {id} (goods {}, ore {key}): {why}: not sold", row.equip_id()),
            _ => {}
        }
    }
    out
}

/// (SHOP-TIERS) THE GROWTH OF THE LINEUP `rows` (module text): the game's own ShopLineupParam rows of the range, each row's
/// `eventFlag_forRelease` read live (0: sold from the start); the sheet sora-er-shop-tiers' `lineup.<first>-<last>` row when
/// the param cannot be read. `None`: the lineup never grows (no row with a release flag): the KH shop keeps its whole stock.
/// Also says what it read (for the log).
pub fn lineup_growth(rows: (i32, i32)) -> Option<(kh_host::shop_growth::Growth, String)> {
    let released = |flag: u32| crate::bossreward::game_flag(flag) == Some(true);
    // SAFETY: the singleton lives as long as the game; its holder table is checked by the SDK.
    let flags: Option<Vec<u32>> = (unsafe { SoloParamRepository::instance() }).ok().map(|repo| repo.rows::<ShopLineupParam>().filter(|(id, _)| (rows.0..=rows.1).contains(&(*id as i32))).map(|(_, row)| row.event_flag_for_release()).collect());
    let (flags, from) = match flags {
        Some(f) if !f.is_empty() => (f, "the game's rows"),
        _ => (kh_host::shop_growth::rules().lineups.get(&rows)?.release_flags(), "the sheet's row (the game's rows could not be read)"),
    };
    let growth = kh_host::shop_growth::lineup_share(&flags, &released)?;
    let mut on: Vec<u32> = flags.iter().copied().filter(|f| *f != 0 && released(*f)).collect();
    let mut off: Vec<u32> = flags.iter().copied().filter(|f| *f != 0 && !released(*f)).collect();
    on.sort_unstable();
    on.dedup();
    off.sort_unstable();
    off.dedup();
    let open = flags.iter().filter(|f| **f == 0 || released(**f)).count();
    let why = format!("{open} of {} rows released, tier {} of {} ({from}; release flags on {on:?}, not yet {off:?})", flags.len(), growth.reached + 1, growth.tiers + 1);
    Some((growth, why))
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
        erlog!("shop: sheet sora-er-shop: {} stock row(s), tiers {:?} by depth, markup {:?}, sell {} %, kill munny x{:?} (bosses x{}), online sort groups {:?} (Elden Ring's items), {} smith talk(s){}", rules.stock.len(), rules.area_tier, rules.markup, (rules.sell_share * 100.0).round(), rules.kill_scale, rules.boss_scale, rules.online_groups, rules.smith_talks.len(), if rules.problems.is_empty() { String::new() } else { format!(" | PROBLEM ROW(S): {}", rules.problems.join("; ")) });
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
                // (THE USER 2026-11-02: "make them appear where they usually would") the merchant's own glovewort lines (the
                // Twin Maiden Husks' after the bell bearings), read from the game's rows the script named, at their own price
                let mut extra = if r.sell { Vec::new() } else { crate::spirittune::merchant_lines(r.rows) };
                if !extra.is_empty() {
                    erlog!("shop: talk {}: {} spirit-tuning material line(s) of its own ({}..{}) sold too: {}", r.talk_id, extra.len(), r.rows.0, r.rows.1, extra.iter().map(|o| format!("{} {}", o.name, o.price)).collect::<Vec<_>>().join(", "));
                }
                // (P10) and its own Smithing Stone lines as KH3 ores
                let ores = if r.sell { Vec::new() } else { ore_lines(r.rows) };
                if !ores.is_empty() {
                    erlog!("shop: talk {}: {} Smithing Stone line(s) of its own ({}..{}) sold as KH3 ores: {}", r.talk_id, ores.len(), r.rows.0, r.rows.1, ores.iter().map(|o| format!("{} {}", o.name, o.price)).collect::<Vec<_>>().join(", "));
                }
                extra.extend(ores);
                // (P9b) what KH3's Moogle sold, by the Moogle level: Donald's / Goofy's weapons at the merchants of kh-weapon-sources
                // shop.*, KH3's other Moogle rows at every merchant, goal-unlocked materials (kh_host::shop::kh3_stock)
                let mut kh3_keys: Vec<String> = Vec::new();
                if !r.sell {
                    let kh3 = kh_host::shop::kh3_stock_live(r.talk_id);
                    kh3_keys = kh3.iter().map(|o| o.key.clone()).collect();
                    if !kh3.is_empty() {
                        erlog!("shop: talk {}: KH3's Moogle stock (Moogle level {}): {}", r.talk_id, kh_host::weapons::workshop().moogle_level, kh3.iter().map(|o| format!("{} {}", o.name, o.price)).collect::<Vec<_>>().join(", "));
                    }
                    extra.extend(kh3);
                }
                // (SHOP-TIERS) the KH stock as far as the merchant's own lineup has grown (its release flags, read live); the
                // host's own lines above (ER's glovewort / ore lines: their own flags already) are never cut
                let growth = if control.flag("shop.grow") == Some(false) {
                    None
                } else {
                    let keep: Vec<String> = extra.iter().filter(|o| !kh3_keys.contains(&o.key)).map(|o| o.key.clone()).collect();
                    let rows = if r.rows.0 > 0 { Some(r.rows) } else { kh_host::shop_growth::rules().lineup_of_talk(r.talk_id) };
                    rows.and_then(|rows| {
                        let (growth, why) = lineup_growth(rows)?;
                        erlog!("shop: talk {}: Elden Ring's lineup {}..{}{}: {why} -> the KH stock grown to {:.0} %", r.talk_id, rows.0, rows.1, if r.rows.0 > 0 { "" } else { " (the merchant's own: a sell window names none)" }, growth.share * 100.0);
                        Some(kh_host::shop_growth::Growth { keep, ..growth })
                    })
                };
                let open = ShopOpen { title: title.clone(), tier, tab: if r.sell { ShopTab::Sell } else { ShopTab::Buy }, extra, growth };
                match crate::hud::with_hud(|hud| hud.open_shop(open, sora)) {
                    Some(true) => {
                        st.wait = 0.0;
                        let serial = r.serial;
                        with_shared(|s| {
                            if let Some(q) = s.shop.request.as_mut().filter(|q| q.serial == serial) {
                                q.opened = true;
                            }
                        });
                        erlog!("shop: the KH shop opens for talk {} ({}): {} tab, shop tier {tier} (region depth {}), Elden Ring's rows {}..{} not used (but for their spirit-tuning materials)", r.talk_id, if title.is_empty() { "no name" } else { &title }, if r.sell { "SELL" } else { "BUY" }, DEPTH.load(Ordering::Relaxed), r.rows.0, r.rows.1);
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lootmap::ErItem;

    /// (P10 MATERIALS) A MERCHANT'S SMITHING STONE LINE IS RESOLD AS ITS KH3 ORE: every Smithing Stone [1] .. [8] and Somber
    /// [1] .. [9] as a shop's item is Fluorite / Damascus / Adamantite / Electrum by its own row; an Ancient Dragon stone in a
    /// shop is Electrum (never Orichalcum); a glovewort, a potion, an unnamed reinforcement material are not ore lines; the
    /// gloveworts' line rules (released, runes, unlimited, priced).
    #[test]
    fn p10_smithing_stone_lines_are_resold_as_ores() {
        let rules = LootRules::parse(crate::lootmap::SHEET, &crate::lootmap::kh_items());
        let ore = |id: u32| {
            let stone = ErItem { cat: Some(Cat::Goods), id, goods_type: 14, rarity: 1, known: true, ..ErItem::default() };
            ore_of(&rules.classify(&stone, &Ctx::of(Source::Shop, 2))).map(|(_, key)| key)
        };
        let want = |key: &str| Some(key.to_string());
        for (ids, key) in [(10_100u32..=10_101, "MAT_ITEM53"), (10_102..=10_103, "MAT_ITEM54"), (10_104..=10_105, "MAT_ITEM55"), (10_106..=10_107, "MAT_ITEM58"), (10_160..=10_161, "MAT_ITEM53"), (10_162..=10_163, "MAT_ITEM54"), (10_164..=10_165, "MAT_ITEM55"), (10_166..=10_167, "MAT_ITEM58")] {
            for id in ids {
                assert_eq!(ore(id), want(key), "goods {id}");
            }
        }
        assert_eq!([ore(10_200), ore(10_140), ore(10_168)], [want("MAT_ITEM58"), want("MAT_ITEM58"), want("MAT_ITEM58")], "Somber [9]; the Ancient Dragon stones in a shop: Electrum");
        assert_eq!([ore(10_900), ore(10_950)], [None, None], "a glovewort (its own line rule), an unnamed reinforcement material");
        let potion = ErItem { cat: Some(Cat::Goods), id: 900, known: true, consumable: true, effect: crate::lootmap::Effect::Hp, ..ErItem::default() };
        assert_eq!(ore_of(&rules.classify(&potion, &Ctx::of(Source::Shop, 0))), None);
        assert_eq!(line_refused(true, 0, -1, 600), None, "a bell-bearing line: released, runes, unlimited, priced");
        assert_eq!([line_refused(false, 0, -1, 600), line_refused(true, 1, -1, 600), line_refused(true, 0, 3, 600), line_refused(true, 0, -1, 0)], [Some("not released yet"), Some("not paid in runes"), Some("a limited stock"), Some("no price")]);
    }
}
