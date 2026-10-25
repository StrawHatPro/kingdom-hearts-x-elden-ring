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
//! KEYS: `shop.menu` (1; 0 = Elden Ring's own buy / sell windows), `npc.smith_talk` (0; 1 = the smiths can be talked to:
//! Elden Ring's reinforcement menu). Logs: `shop:` lines.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};

use kh_host::hud::ShopOpen;
use kh_ui::shop::ShopTab;

use crate::control::Control;
use crate::erlog;
use crate::lootmap::LootRules;
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
}
