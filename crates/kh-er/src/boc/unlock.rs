//! (OUTFIT-UNLOCK) THE GAME HALF OF BOC'S NEEDLES (the rules: [`super::needles`]).
//!
//! - THE POLL (boc.rs `frame`, once a frame, twice a second; the first 1 s after Sora's start, when the game's constant flags
//!   6000 / 6001 read off / on): Elden Ring's "Sewing Needle given" (31159206) and "Gold Sewing Needle given" (1036489208)
//!   flags; a needle read given whose record (`boc:needle` / `boc:gold_needle`) Sora's `.khsave` lacks gets it now, in the
//!   HUD's records under the HUD's lock (additive; Elden Ring's save is never written). So the outfits open THE MOMENT his
//!   script sets the flag (the hand-over), and a loaded save that already has the flag opens them at its first poll.
//! - THE POPUP: KH3's OBTAINED popup (kh-host hud_obtained.rs, the Elden Ring key-item kind: the needle's own icon) naming the
//!   outfits, once per record added; held while Boc's talk is up and until 3 s after the start (a load's catch-up).
//! - THE TALK'S FLAG READS ([`talk_flag_read`], from the talk-script query detour on the game thread): the Gold Sewing
//!   Needle's offer by the user's rule (`boc.gold_anytime`).
//!
//! Logs: `boc: (OUTFIT-UNLOCK)` lines.

use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(windows)]
use std::sync::Mutex;

#[cfg(windows)]
use super::needles::{self, Tier};
#[cfg(windows)]
use crate::control::Control;
#[cfg(windows)]
use crate::erlog;

/// `boc.unlock` (default on): the needles' rule.
static ON: AtomicBool = AtomicBool::new(true);
/// The first poll of this start has read the flags.
static POLLED: AtomicBool = AtomicBool::new(false);
/// (OUTFIT-UNLOCK) Elden Ring's Tailoring Tools (or the golden ones) owned, read on the game thread by [`frame`].
static KIT: AtomicBool = AtomicBool::new(false);
/// (OUTFIT-UNLOCK) The Gold Sewing Needle held, as the frame's poll last read it (the talk detour's second source).
#[cfg(windows)]
static GOLD_HELD: AtomicBool = AtomicBool::new(false);
/// `boc.gold_anytime` (default on): the gold needle's offer by the user's rule.
#[cfg(windows)]
static GOLD_ANYTIME: AtomicBool = AtomicBool::new(true);
/// A line the talk detour leaves for the frame's log (the detour writes no file).
#[cfg(windows)]
static NOTE: Mutex<Option<String>> = Mutex::new(None);
/// The offer the detour last answered (0 none yet, 1 off, 2 on): its log line once per change.
#[cfg(windows)]
static LAST_OFFER: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Seconds after the start before the first poll, between polls, and before a catch-up's popup.
#[cfg(windows)]
const FIRST_POLL: f32 = 1.0;
#[cfg(windows)]
const POLL_EVERY: f32 = 0.5;
#[cfg(windows)]
const POPUP_AFTER: f32 = 3.0;

/// (PORT) THE GAME'S OWN FLAG CHECK the rule answers in: `case 0xf` (GetEventFlag) of the talk-script query function
/// (talkkh/hooks.rs ENV_FN, `CSEzStateTalkEnv::invoke`): `count = vt[2](q)`, `count >= 2`, the event-flag manager's global
/// tested (`cmp [rip+d], r12`), then argument 1 read (`vt[3](q, buf, 1)`). The same bytes at ENV_FN + 0x68f in 2.7.1.0 and
/// 2.7.2.0 (masked: the jumps' and the global's displacements), unique in either executable; capture 1 = the global
/// (2.7.1.0 0x143d6c4b8, as the decompiled case reads it). In the PORT offline check (gamever.rs).
#[cfg(windows)]
const PAT_FLAG_CASE: &str = "48 8b 03 48 8b cb ff 50 10 83 f8 02 0f 8c ? ? ? ? 4c 39 25 $ { ' } 0f 84 ? ? ? ? 48 8b 03 41 bf 01 00 00 00 45 8b c7 48 8d 55 90 48 8b cb ff 50 18";
#[cfg(windows)]
pub(crate) const LOCATED: [crate::havok::Located; 2] = [
    crate::havok::Located { name: "talk query GetEventFlag case (CSEzStateTalkEnv::invoke case 0xf)", pattern: PAT_FLAG_CASE, capture: 0, rva_2_7_1_0: 0x00ea_2b2f, rva_2_7_2_0: 0x00ea_421f },
    crate::havok::Located { name: "event-flag manager global (read by the talk query's case 0xf)", pattern: PAT_FLAG_CASE, capture: 1, rva_2_7_1_0: 0x03d6_c4b8, rva_2_7_2_0: 0x03d6_f4d8 },
];
/// The case's place inside the query function (both versions).
#[cfg(windows)]
pub(crate) const FLAG_CASE_IN_FN: usize = 0x68f;

/// The case is where the rule expects it in the running executable (looked up once; not found: the rule stays off).
#[cfg(windows)]
fn flag_case_ok() -> bool {
    static OK: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *OK.get_or_init(|| {
        let case = crate::havok::locate(&LOCATED[0]);
        // (the query function itself is detoured by then: its first bytes are the jump, so its recorded RVA for this
        // version stands for it; talkkh/hooks.rs located it by pattern at that very RVA before detouring it)
        let func = crate::talkkh::hooks::ENV_FN.recorded(crate::gamever::current()).map(|rva| crate::entry::exe_base() + rva as usize);
        let ok = matches!((case, func), (Some(c), Some(f)) if c == f + FLAG_CASE_IN_FN);
        if let Ok(mut n) = NOTE.lock() {
            *n = Some(if ok { format!("the talk query's GetEventFlag case is at its place (query function + {FLAG_CASE_IN_FN:#x}): the Gold Sewing Needle's offer can be answered") } else { format!("the talk query's GetEventFlag case was NOT found where this executable should have it (case {case:x?}, function {func:x?}): Boc's gold needle keeps Elden Ring's own requirements") });
        }
        ok
    })
}

#[cfg(windows)]
#[derive(Default)]
struct Poll {
    started: Option<std::time::Instant>,
    last: Option<std::time::Instant>,
    /// The flags were found not readable (told once).
    unreadable_told: bool,
    /// Tiers opened whose popup is not shown yet.
    pending: Vec<Tier>,
    off_told: bool,
}

#[cfg(windows)]
static POLL: Mutex<Option<Poll>> = Mutex::new(None);

/// The needles' rule is on (`boc.unlock`).
pub(super) fn on() -> bool {
    ON.load(Ordering::Relaxed)
}

/// (OUTFIT-UNLOCK) The sewing kit is owned (the camp menu's Outfits entry).
pub(super) fn kit_owned() -> bool {
    KIT.load(Ordering::Relaxed)
}

/// This start's first poll has read the flags (until then a save's chosen outfit is drawn as it is).
pub(super) fn polled() -> bool {
    POLLED.load(Ordering::Relaxed)
}

/// Sora's start (boc.rs `load`): the next poll is a catch-up.
pub(super) fn restart() {
    POLLED.store(false, Ordering::Relaxed);
    #[cfg(windows)]
    if let Ok(mut p) = POLL.lock() {
        *p = Some(Poll::default());
    }
    // (PORT) the game's flag check looked up at the start (a scan of the executable, once), not in a later frame
    #[cfg(windows)]
    flag_case_ok();
}

/// ONCE A FRAME (boc.rs `frame`, before its state lock): the keys, the poll, the popups. `boc_talk`: Boc's talk is up.
#[cfg(windows)]
pub(super) fn frame(control: &Control, boc_talk: bool) {
    // (the case is looked up at the start (restart), never inside the detour)
    GOLD_ANYTIME.store(control.flag("boc.gold_anytime") != Some(false) && flag_case_ok(), Ordering::Relaxed);
    if let Some(note) = NOTE.try_lock().ok().and_then(|mut n| n.take()) {
        erlog!("boc: (OUTFIT-UNLOCK) {note}");
    }
    let on = control.flag("boc.unlock") != Some(false);
    ON.store(on, Ordering::Relaxed);
    let Ok(mut guard) = POLL.lock() else { return };
    let p = guard.get_or_insert_with(Poll::default);
    if !on {
        if !std::mem::replace(&mut p.off_told, true) {
            erlog!("boc: (OUTFIT-UNLOCK) boc.unlock = 0: every outfit is offered (the rule before 2026-11-05); the needles' records are not written");
        }
        return;
    }
    p.off_told = false;
    let now = std::time::Instant::now();
    let since_start = now.duration_since(*p.started.get_or_insert(now)).as_secs_f32();
    let due = since_start >= FIRST_POLL && p.last.is_none_or(|l| now.duration_since(l).as_secs_f32() >= POLL_EVERY);
    if due {
        p.last = Some(now);
        poll(p);
        // (the sewing kit, for the camp menu's entry; logged when it changes)
        let kit = needles::KIT_GOODS.iter().any(|g| holds_goods(*g));
        let gold = holds_goods(needles::GOLD_SEWING_NEEDLE);
        if GOLD_HELD.swap(gold, Ordering::Relaxed) != gold {
            erlog!("boc: (OUTFIT-UNLOCK) the Gold Sewing Needle (goods {}) is {}", needles::GOLD_SEWING_NEEDLE, if gold { "held" } else { "not held" });
        }
        if KIT.swap(kit, Ordering::Relaxed) != kit {
            erlog!("boc: (OUTFIT-UNLOCK) Tailoring Tools {}: the camp menu's Outfits entry is {}", if kit { "owned" } else { "not owned" }, if kit { "SHOWN" } else { "hidden" });
        }
    }
    // the popups: not during Boc's talk (his hand-over plays out first), not before a load's catch-up time
    // (Elden Ring's talk id stays the last one after the talk: Boc's talk is "up" only while a held conversation runs)
    let boc_talk = boc_talk && crate::conv::running().is_some_and(|(id, _)| crate::conv::is_held(id));
    if !p.pending.is_empty() && !boc_talk && since_start >= POPUP_AFTER {
        let tiers = std::mem::take(&mut p.pending);
        drop(guard);
        for tier in tiers {
            popup(tier);
        }
    }
}

#[cfg(windows)]
fn poll(p: &mut Poll) {
    let read = crate::bossreward::game_flag;
    let readable = read(6000) == Some(false) && read(6001) == Some(true);
    if !readable {
        if !std::mem::replace(&mut p.unreadable_told, true) {
            erlog!("boc: (OUTFIT-UNLOCK) the event flags are not readable yet (6000 / 6001 read {:?} / {:?}): the needles are polled again", read(6000), read(6001));
        }
        return;
    }
    let given = |t: Tier| t.flag().and_then(read) == Some(true);
    let (needle, gold) = (given(Tier::Needle), given(Tier::Gold));
    let first = !POLLED.swap(true, Ordering::Relaxed);
    // the records under the HUD's lock (no HUD yet: the shared inventory the HUD takes over)
    let opened = crate::hud::with_hud(|hud| needles::take(&mut hud.inventory_mut().records, needle, gold)).unwrap_or_else(|| {
        let mut records = crate::hud::records();
        let opened = needles::take(&mut records, needle, gold);
        if !opened.is_empty() {
            crate::hud::set_records(records);
        }
        opened
    });
    if first {
        erlog!("boc: (OUTFIT-UNLOCK) first poll: Sewing Needle given (flag {}) {needle}, Gold Sewing Needle given (flag {}) {gold}{}", needles::SEWING_NEEDLE_GIVEN, needles::GOLD_NEEDLE_GIVEN, if opened.is_empty() { String::new() } else { format!(": THE LOADED SAVE OPENS {opened:?} AT ONCE") });
    }
    for tier in &opened {
        erlog!("boc: (OUTFIT-UNLOCK) OUTFITS OPENED by {}: {} (record \"{}\" in Sora's save at its next write; Elden Ring's save untouched)", if *tier == Tier::Needle { format!("the Sewing Needle given to Boc (flag {})", needles::SEWING_NEEDLE_GIVEN) } else { format!("the Gold Sewing Needle given to Boc (flag {})", needles::GOLD_NEEDLE_GIVEN) }, names(*tier).join(", "), tier.record().unwrap_or("-"));
    }
    p.pending.extend(opened);
}

/// The pack's outfits (id, shown name).
#[cfg(windows)]
fn pack_outfits() -> Vec<(String, String)> {
    super::STATE.lock().ok().and_then(|g| g.as_ref().map(|s| s.outfits.iter().map(|o| (o.id.clone(), o.name.clone())).collect())).unwrap_or_default()
}

#[cfg(windows)]
fn names(tier: Tier) -> Vec<String> {
    needles::opened_by(tier, &pack_outfits()).into_iter().map(|(_, n)| n.clone()).collect()
}

/// KH3's OBTAINED popup for a tier.
#[cfg(windows)]
fn popup(tier: Tier) {
    // once a session per needle: a map load without a save brings the save's records back and the record is added again
    static SHOWN: Mutex<Vec<Tier>> = Mutex::new(Vec::new());
    if let Ok(mut shown) = SHOWN.lock() {
        if shown.contains(&tier) {
            erlog!("boc: (OUTFIT-UNLOCK) the {tier:?} popup was shown in this session already: not again");
            return;
        }
        shown.push(tier);
    }
    let Some((name, caption)) = needles::popup_text(tier, &pack_outfits()) else { return };
    let icon_id = tier.goods().and_then(crate::loot::goods_icon_id);
    let what = kh_host::hud::ObtainedKind::ErKeyItem { name: name.clone(), caption: caption.clone(), icon_id };
    let shown = crate::hud::with_hud(|hud| hud.show_obtained(what));
    erlog!("boc: (OUTFIT-UNLOCK) KH3's OBTAINED popup \"{name}\" / \"{}\" (icon {icon_id:?}): {}", caption.replace('\n', " / "), match shown { Some(true) => "asked", Some(false) => "NOT available (no KeyItem movie or hud.obtained off)", None => "NOT shown (no HUD)" });
}

/// The player holds the Elden Ring goods `id` (the key items and the normal inventory): an entry of it is there. (In game the
/// talk detour read the Gold Sewing Needle as not held while it was: a key item's quantity can read 0, loot.rs counts it
/// `max(1)`; the frame's poll also keeps the answer, [`GOLD_HELD`], for the detour.)
#[cfg(windows)]
fn holds_goods(id: u32) -> bool {
    let Some(data) = crate::loot::player_data() else { return false };
    let raw = 0x4000_0000 | id;
    let items = &data.equipment.equip_inventory_data.items_data;
    // (seen in game: after Boc took the Sewing Needle the Gold one was no longer inside `current_key_entries` (the in-use
    // length), yet the game refused a second one: owned) EVERY allocated key-item slot is read (loot.rs `all_key_entries`)
    let key = crate::loot::all_key_entries(items).iter().filter_map(|e| e.as_option()).any(|e| e.item_id.into_inner() == raw);
    key || items.normal_entries().iter().filter_map(|e| e.as_option()).any(|e| e.item_id.into_inner() == raw)
}

/// (the talk-script query detour, game thread) Boc's GetEventFlag(`flag`) by the user's rule; None = the game's answer.
#[cfg(windows)]
pub(super) fn talk_flag_read(talk_id: i32, flag: u32) -> Option<bool> {
    if !needles::answers(flag) || !super::is_boc_talk(talk_id) || !GOLD_ANYTIME.load(Ordering::Relaxed) {
        return None;
    }
    let read = crate::bossreward::game_flag;
    let given = read(needles::SEWING_NEEDLE_GIVEN) == Some(true);
    // a flag not readable counts as given: no offer
    let gold_given = read(needles::GOLD_NEEDLE_GIVEN) != Some(false);
    let held = holds_goods(needles::GOLD_SEWING_NEEDLE) || GOLD_HELD.load(Ordering::Relaxed);
    let offer = needles::gold_offer(held, given, gold_given);
    if flag == needles::GOLD_OFFER && LAST_OFFER.swap(1 + u8::from(offer), Ordering::Relaxed) != 1 + u8::from(offer) {
        let line = if offer {
            format!("Boc's talk {talk_id} reads the Gold Sewing Needle's offer flag {flag}: SET by the user's rule (held, the Sewing Needle given, the gold one not): \"Give the Gold Sewing Needle\" is in his list whatever else Elden Ring asks; his other result-25 offers {:?} read 0 meanwhile", needles::SHARED_RESULT_OFFERS)
        } else {
            format!("Boc's talk {talk_id} reads the Gold Sewing Needle's offer flag {flag}: the game's own answer (held {held}, the Sewing Needle given {given}, the gold one given {gold_given})")
        };
        if let Ok(mut n) = NOTE.try_lock() {
            *n = Some(line);
        }
    }
    needles::flag_read(flag, offer)
}
