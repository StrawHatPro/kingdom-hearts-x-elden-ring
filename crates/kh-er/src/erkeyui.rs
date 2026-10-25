//! (HUD2 round 2, ERUI) ELDEN RING'S OWN BANNERS AND ITEM-GET UI ARE GONE: KH3'S POPUPS ARE THE ONLY ONES.
//!
//! The user (FINAL15, picture party/ui4/ref/er_map_found_plus_kh_popup.jpg): "ALL of the Elden Ring UI for finding a key
//! item like this, including the MAP FOUND banner must be removed ... ONLY our KH3 item obtained UI should appear, remove
//! everything else related to this sort of thing". Then (ERUI, 2026-10-23, er/zfin/user_ref/er_ui_keyitem_demigod_felled.jpg,
//! after Malenia): "Elden Ring UI is still appearing when getting a key item, as well as the BOSS SLAIN and DEMIGOD SLAIN
//! banners, THAT ALL NEEDS TO GO!!". WHY IT STILL SHOWED (ERUI): this rule asked `control.on("hud.obtained")`, which is
//! only true with `hud.obtained = 1` in the file, while the popup itself is ON when the key is missing (hud.rs, loot.rs:
//! `!= Some(false)`): with the shipped control file nothing here ever ran (the user's log has no `erkeyui:` line).
//! Everything is switched off at its source; what the game GIVES is never touched (the item reaches the inventory, the
//! map reveals its region, the boss's flags and rewards are set: only the presentation is not made).
//!
//! 1. THE FULL-SCREEN BANNERS ("ENEMY FELLED", "GREAT ENEMY FELLED", "DEMIGOD FELLED", "LEGEND FELLED", "GOD SLAIN",
//!    "GREAT RUNE RESTORED", "MAP FOUND", "LOST GRACE DISCOVERED", the multiplayer ones): `CSMenuManImp::DisplayStatusMessage
//!    (menu, id)` (2.7.1.0 RVA 0x7671F0 / 2.7.2.0 0x7676A0, the SDK's `display_status_message`). (ERUI, read from both
//!    executables) its body is `mov [CSFeMan + 0x3760], id` through 0x14076F210 and `return 1`; 0x14076F210 has NO
//!    other caller, so EVERY banner request of the game goes through this one function. Its callers: the EMEVD banner
//!    command (0x140571D0E, inside the instruction dispatcher 0x140571370), a wrapper 0x140595AA0 called by 20 sites of
//!    the bank-2003 event code (the boss-defeat handlers among them), 0x1405F2B90 (id 2000) and the talk scripts
//!    (0x140EA89B0). Detoured: an id of `HIDDEN_BANNERS` returns 1 at once (the caller sees "shown"), so neither the
//!    banner nor its sound is ever requested. YOU DIED (5) and any id not in the SDK's list (2000) still pass: the death
//!    is death.rs's (KH3's game over under black), and an unknown id is not guessed at (it is logged instead).
//! 2. THE PICKUP WINDOW ("NEW / <item> / OK", the great-rune / remembrance / key-item window), THE ITEM LOG AT THE RIGHT
//!    ("<item> x1") AND THE PICKUP SOUND: (ERUI, read from both executables) one function makes all three for every
//!    item of every kind (2.7.1.0 RVA 0x77AA00 / 2.7.2.0 0x77AEB0, `fn(item_get_menu, *item_id, count, *?, 5 more)`,
//!    no result): it asks 0x847F10 -> 0x84A420 (the row's `showDialogCondType`, read per item table: weapon +0x109,
//!    protector +0xE3, accessory +0x40, goods +0x6F, gem +0x34) whether to queue the window, 0x847F50 (`showLogCondType`)
//!    whether to add the log line (0x844210, into CSFeMan + 0x6560), then plays the pickup sound. It writes nothing
//!    but those queues; its four callers (0x140255378, 0x140255628, the item give's loop 0x14056473A, 0x1405EB68E)
//!    use no result. Detoured: while the rule is on it returns at once, so no window is ever queued (nothing waits
//!    for a confirmation that cannot be seen: input can never get stuck behind it) and no log line is written.
//!    THE FALLBACK (the HUD2 route) when the detour cannot be made: the goods rows of the key-item kinds (goods types
//!    1 key item, 3 remembrance, 12 info item, 15 great rune) get `showDialogCondType` 0 and `showLogCondType` 0 in the
//!    game's memory (never the file; the originals written back when the rule goes and when Sora stops; every 10 s).
//!
//! KEYS: `hud.er_banners = 1`: the game's banners as before. `hud.er_item_ui = 1` (or the older `hud.er_keyitem_ui = 1`),
//! or KH3's popup off (`hud.obtained = 0`): the game's pickup window, item log and sound as before. Missing / 0 = hidden.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use eldenring::cs::{EquipParamGoods, SoloParamRepository};
use fromsoftware_shared::FromStatic;

use crate::control::Control;
use crate::erlog;
use crate::havok::{locate, Located};

pub(crate) const STATUS_FN: Located = Located {
    name: "CSMenuManImp::DisplayStatusMessage (the full-screen banners)",
    pattern: "40 53 48 83 ec 30 48 8d 4c 24 20 c7 44 24 20 ff ff ff ff 8b da 48 c7 44 24 28 00 00 00 00 e8",
    capture: 0,
    rva_2_7_1_0: 0x0076_71f0, rva_2_7_2_0: 0x0076_76a0 /* (PORT) */,
};

/// (ERUI) The item-get presentation: the pickup window's queue, the item log, the pickup sound (see the header).
pub(crate) const ITEM_GET_FN: Located = Located {
    name: "item-get presentation (pickup window, item log, pickup sound)",
    pattern: "48 8b c4 55 57 41 54 41 56 41 57 48 8d 68 c9 48 81 ec e0 00 00 00 48 c7 44 24 28 fe ff ff ff 48 89 58 08 48 89 70 18 49 8b f1 45 8b f0 4c 8b fa 48 8b d9 48 8b 05 ? ? ? ? 48 85 c0",
    capture: 0,
    rva_2_7_1_0: 0x0077_aa00, rva_2_7_2_0: 0x0077_aeb0 /* (ERUI) 589 of 589 bytes identical */,
};

/// `STATUS_MESSAGE_MAP_FOUND` (SDK menu_man.rs).
pub const MAP_FOUND: i32 = 17;
/// `STATUS_MESSAGE_YOU_DIED`: passes (the death is death.rs's).
pub const YOU_DIED: i32 = 5;
/// (ERUI) Every banner of the SDK's list but YOU DIED and MENU_TEXT (41, not a banner of this family): felled 1-4,
/// the multiplayer ones 7-9, 13-16, 23-25, 30-40, LOST GRACE DISCOVERED 11, MAP FOUND 17, GREAT RUNE RESTORED 21,
/// GOD SLAIN 22.
pub const HIDDEN_BANNERS: [i32; 28] = [1, 2, 3, 4, 7, 8, 9, 11, 13, 14, 15, 16, 17, 21, 22, 23, 24, 25, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39];
/// The goods types of the key-item inventory (the fallback's rows).
pub const KEY_GOODS_TYPES: [u8; 4] = [1, 3, 12, 15];

static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static ITEM_ORIGINAL: AtomicUsize = AtomicUsize::new(0);
/// The rules (see `Rules`), read by the detours.
static HIDE_ITEMS: AtomicBool = AtomicBool::new(false);
static HIDE_BANNERS: AtomicBool = AtomicBool::new(false);
static SKIPPED: AtomicU32 = AtomicU32::new(0);
static LAST_BANNER: AtomicI32 = AtomicI32::new(0);
static PASSED: AtomicU32 = AtomicU32::new(0);
static LAST_PASSED: AtomicI32 = AtomicI32::new(0);
static ITEMS_SKIPPED: AtomicU32 = AtomicU32::new(0);
static LAST_ITEM: AtomicU32 = AtomicU32::new(0);

/// (ERUI) What is hidden, from the control file's keys (missing = hidden).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Rules {
    pub banners: bool,
    pub items: bool,
}

/// (ERUI) The rules from the keys as the control file has them (`None` = the key is missing).
pub fn rules(obtained: Option<bool>, item_ui: Option<bool>, keyitem_ui: Option<bool>, banners_shown: Option<bool>) -> Rules {
    Rules { banners: banners_shown != Some(true), items: obtained != Some(false) && item_ui != Some(true) && keyitem_ui != Some(true) }
}

/// The banner request: a hidden banner is dropped while its rule is on.
unsafe extern "C" fn status_hook(menu: *mut c_void, id: i32) -> u8 {
    let r = Rules { banners: HIDE_BANNERS.load(Ordering::Relaxed), items: HIDE_ITEMS.load(Ordering::Relaxed) };
    if skip_banner(id, r) {
        LAST_BANNER.store(id, Ordering::Relaxed);
        SKIPPED.fetch_add(1, Ordering::Relaxed);
        return 1;
    }
    LAST_PASSED.store(id, Ordering::Relaxed);
    PASSED.fetch_add(1, Ordering::Relaxed);
    let original = ORIGINAL.load(Ordering::Relaxed);
    if original == 0 {
        return 0;
    }
    // SAFETY: the trampoline of the game's own function, same signature.
    let f: unsafe extern "C" fn(*mut c_void, i32) -> u8 = std::mem::transmute(original);
    f(menu, id)
}

type ItemGetFn = unsafe extern "C" fn(usize, usize, usize, usize, usize, usize, usize, usize, usize);

/// (ERUI) The item-get presentation: nothing of it is made while the rule is on (every argument slot is passed on
/// otherwise: four registers and the five stack slots the callers fill). Nothing in here can panic.
unsafe extern "C" fn item_get_hook(menu: usize, item: usize, count: usize, d: usize, e: usize, f: usize, g: usize, h: usize, i: usize) {
    if HIDE_ITEMS.load(Ordering::Relaxed) {
        if item > 0x10000 {
            // SAFETY: the caller's pointer to the item id (the function reads it first thing, `mov eax, [rdx]`)
            LAST_ITEM.store(std::ptr::read_unaligned(item as *const u32), Ordering::Relaxed);
        }
        ITEMS_SKIPPED.fetch_add(1, Ordering::Relaxed);
        return;
    }
    let original = ITEM_ORIGINAL.load(Ordering::Relaxed);
    if original != 0 {
        // SAFETY: the trampoline of the game's own function, same argument slots.
        let call: ItemGetFn = std::mem::transmute(original);
        call(menu, item, count, d, e, f, g, h, i);
    }
}

/// Whether a full-screen banner is one the rules remove.
pub fn skip_banner(id: i32, r: Rules) -> bool {
    (r.items && id == MAP_FOUND) || (r.banners && HIDDEN_BANNERS.contains(&id))
}

/// The SDK's name of a banner id (for the log).
pub fn banner_name(id: i32) -> &'static str {
    match id {
        1 => "DEMIGOD FELLED",
        2 => "LEGEND FELLED",
        3 => "GREAT ENEMY FELLED",
        4 => "ENEMY FELLED",
        YOU_DIED => "YOU DIED",
        11 => "LOST GRACE DISCOVERED",
        17 => "MAP FOUND",
        21 => "GREAT RUNE RESTORED",
        22 => "GOD SLAIN",
        7..=9 | 13..=16 | 23..=25 | 30..=40 => "a multiplayer banner",
        41 => "MENU_TEXT",
        _ => "not in the SDK's list",
    }
}

/// The two display bits of a goods row the fallback sets to "none": (show log, show dialog) wanted, given the row's
/// goods type. None: the row is not one of the key-item kinds (left alone).
pub fn hidden_bits(goods_type: u8) -> Option<(bool, u8)> {
    KEY_GOODS_TYPES.contains(&goods_type).then_some((false, 0))
}

#[derive(Default)]
struct State {
    hook_tried: bool,
    item_hook_tried: bool,
    /// The rows written: (id, original show log, original show dialog).
    rows: Vec<(u32, bool, u8)>,
    hidden: bool,
    checked: Option<Instant>,
    skipped_told: u32,
    passed_told: u32,
    items_told: u32,
    rules_told: Option<Rules>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn hook(item: &Located, hook: usize, original: &AtomicUsize, what: &'static str) -> Result<(), String> {
    let f = locate(item).ok_or_else(|| "not found for this executable (PORT: 2.7.1.0 / 2.7.2.0 table)".to_string())?;
    // SAFETY: the function's start, found by its bytes at its recorded address; the hook has its signature.
    unsafe { kh_er_render::inline_hook(f, hook, original, what) }.inspect_err(|_| original.store(0, Ordering::SeqCst))
}

/// Every frame (frame.rs): the rules, the detours made once, the fallback's rows written / given back.
pub fn frame(control: &Control) {
    let r = rules(control.flag("hud.obtained"), control.flag("hud.er_item_ui"), control.flag("hud.er_keyitem_ui"), control.flag("hud.er_banners"));
    HIDE_BANNERS.store(r.banners, Ordering::Relaxed);
    HIDE_ITEMS.store(r.items, Ordering::Relaxed);
    let Ok(mut guard) = STATE.lock() else { return };
    let s = guard.get_or_insert_with(State::default);
    if s.rules_told != Some(r) {
        s.rules_told = Some(r);
        erlog!("erkeyui: Elden Ring's banners {} (hud.er_banners = 1: shown); its pickup window, item log and pickup sound {} (hud.er_item_ui = 1 or hud.obtained = 0: shown)", if r.banners { "HIDDEN (ENEMY / GREAT ENEMY / DEMIGOD / LEGEND FELLED, GOD SLAIN, GREAT RUNE RESTORED, MAP FOUND, LOST GRACE DISCOVERED ...; YOU DIED passes)" } else { "shown" }, if r.items { "HIDDEN for every item (the items are given as always)" } else { "shown" });
    }
    if (r.banners || r.items) && !s.hook_tried {
        s.hook_tried = true;
        match hook(&STATUS_FN, status_hook as *const () as usize, &ORIGINAL, "DisplayStatusMessage") {
            Ok(()) => erlog!("erkeyui: Elden Ring's full-screen banner request is detoured (every banner of the game goes through it)"),
            Err(e) => erlog!("erkeyui: the banner request was NOT detoured ({e}): Elden Ring's banners still show"),
        }
    }
    if r.items && !s.item_hook_tried {
        s.item_hook_tried = true;
        match hook(&ITEM_GET_FN, item_get_hook as *const () as usize, &ITEM_ORIGINAL, "item-get presentation") {
            Ok(()) => erlog!("erkeyui: Elden Ring's item-get presentation is detoured: no pickup window, no item log, no pickup sound of its for any item while the rule is on"),
            Err(e) => erlog!("erkeyui: the item-get presentation was NOT detoured ({e}): the key-item goods rows are switched off instead (the fallback)"),
        }
    }
    let skipped = SKIPPED.load(Ordering::Relaxed);
    if skipped != s.skipped_told {
        s.skipped_told = skipped;
        let id = LAST_BANNER.load(Ordering::Relaxed);
        erlog!("erkeyui: Elden Ring's banner {id} ({}) was NOT shown ({skipped} so far)", banner_name(id));
    }
    let passed = PASSED.load(Ordering::Relaxed);
    if passed != s.passed_told {
        s.passed_told = passed;
        let id = LAST_PASSED.load(Ordering::Relaxed);
        erlog!("erkeyui: Elden Ring's banner {id} ({}) passed to the game ({passed} so far)", banner_name(id));
    }
    let items = ITEMS_SKIPPED.load(Ordering::Relaxed);
    if items != s.items_told {
        s.items_told = items;
        erlog!("erkeyui: Elden Ring's item-get UI for item {:#x} was NOT made ({items} so far): the item is in the inventory as always", LAST_ITEM.load(Ordering::Relaxed));
    }
    // THE FALLBACK (the detour is not there): the rows, written once the world is there (the SDK panics on params not
    // loaded), again every 10 s
    let fallback = r.items && ITEM_ORIGINAL.load(Ordering::Relaxed) == 0;
    if fallback && crate::game::player_age() > 0 && s.checked.map_or(true, |t| t.elapsed().as_secs_f32() >= 10.0) {
        s.checked = Some(Instant::now());
        let first = !s.hidden;
        let written = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| write_hidden(&mut s.rows))).unwrap_or(0);
        s.hidden = true;
        if first || written > 0 {
            let sample = s.rows.iter().find(|r| r.0 == 8600).map_or("not among them".to_string(), |r| format!("showLog {} showDialog {} before", r.1, r.2));
            erlog!("erkeyui: {written} key-item goods row(s) of {} set to no pickup window and no item note (goods types 1, 3, 12, 15; Map: Limgrave, West (8600): {sample}); the items are given as always", s.rows.len());
        }
    } else if !fallback && s.hidden {
        let rows = std::mem::take(&mut s.rows);
        let _ = std::panic::catch_unwind(|| restore(&rows));
        s.hidden = false;
        s.checked = None;
        erlog!("erkeyui: the fallback's {} key-item goods row(s) have their own values back", rows.len());
    }
}

/// Sets the rows of the key-item kinds to no window / no note; returns how many were written now (the originals of
/// rows not seen before are kept).
fn write_hidden(rows: &mut Vec<(u32, bool, u8)>) -> usize {
    // SAFETY: the singleton lives as long as the game; game thread.
    let Ok(repo) = (unsafe { SoloParamRepository::instance_mut() }) else { return 0 };
    let mut written = 0;
    for (id, row) in repo.rows_mut::<EquipParamGoods>() {
        let Some((log, dialog)) = hidden_bits(row.goods_type()) else { continue };
        if row.show_log_cond_type() == log && row.show_dialog_cond_type() == dialog {
            continue;
        }
        if !rows.iter().any(|r| r.0 == id) {
            rows.push((id, row.show_log_cond_type(), row.show_dialog_cond_type()));
        }
        row.set_show_log_cond_type(log);
        row.set_show_dialog_cond_type(dialog);
        written += 1;
    }
    written
}

fn restore(rows: &[(u32, bool, u8)]) {
    // SAFETY: as above.
    let Ok(repo) = (unsafe { SoloParamRepository::instance_mut() }) else { return };
    for (id, log, dialog) in rows {
        if let Some(row) = repo.get_mut::<EquipParamGoods>(*id) {
            row.set_show_log_cond_type(*log);
            row.set_show_dialog_cond_type(*dialog);
        }
    }
}

/// Sora stops (entry.rs): the rows get their own values back; banners and the item UI pass again (the detours
/// themselves go with kh_er_render's hooks).
pub fn release() {
    HIDE_ITEMS.store(false, Ordering::Relaxed);
    HIDE_BANNERS.store(false, Ordering::Relaxed);
    if let Ok(mut guard) = STATE.lock() {
        if let Some(s) = guard.as_mut() {
            let rows = std::mem::take(&mut s.rows);
            let _ = std::panic::catch_unwind(|| restore(&rows));
            s.hidden = false;
            s.checked = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (HUD2 round 2) Only the key-item kinds lose their window and note in the fallback (a crafting material, a
    /// consumable, a spirit ash keep the game's own).
    #[test]
    fn hud2_only_the_key_item_ui_is_removed() {
        for t in [1u8, 3, 12, 15] {
            assert_eq!(hidden_bits(t), Some((false, 0)), "type {t}");
        }
        for t in [0u8, 2, 5, 7, 8, 16] {
            assert_eq!(hidden_bits(t), None, "type {t}");
        }
    }

    /// (ERUI) The shipped control file has none of the keys: BOTH rules are on (the bug: the rule needed
    /// `hud.obtained = 1` while the popup is on without it, so nothing was ever hidden in the user's game).
    #[test]
    fn erui_missing_keys_hide_everything() {
        assert_eq!(rules(None, None, None, None), Rules { banners: true, items: true });
        assert_eq!(rules(Some(true), None, None, None), Rules { banners: true, items: true });
        assert_eq!(rules(Some(false), None, None, None), Rules { banners: true, items: false }, "KH3's popup off: the game's item UI back");
        assert_eq!(rules(None, Some(true), None, None).items, false, "hud.er_item_ui = 1");
        assert_eq!(rules(None, None, Some(true), None).items, false, "the older hud.er_keyitem_ui = 1");
        assert_eq!(rules(None, None, None, Some(true)).banners, false, "hud.er_banners = 1");
        assert_eq!(rules(None, Some(false), Some(false), Some(false)), Rules { banners: true, items: true });
    }

    /// (ERUI) Every felled / slain / found / restored banner goes; YOU DIED, MENU_TEXT and an id the SDK does not
    /// list (2000, from 0x1405F2B90) pass; the keys bring them back.
    #[test]
    fn erui_every_banner_of_the_family_is_dropped() {
        let all = Rules { banners: true, items: true };
        for (id, what) in [(1, "DEMIGOD FELLED"), (2, "LEGEND FELLED"), (3, "GREAT ENEMY FELLED"), (4, "ENEMY FELLED"), (22, "GOD SLAIN"), (21, "GREAT RUNE RESTORED"), (17, "MAP FOUND"), (11, "LOST GRACE DISCOVERED")] {
            assert!(skip_banner(id, all), "{what}");
            assert_eq!(banner_name(id), what);
        }
        for id in [7, 8, 9, 13, 14, 15, 16, 23, 24, 25, 30, 35, 39] {
            assert!(skip_banner(id, all), "multiplayer banner {id}");
        }
        for id in [YOU_DIED, 41, 2000, 0, -1] {
            assert!(!skip_banner(id, all), "banner {id} passes");
        }
        let shown = Rules { banners: false, items: false };
        for id in [1, 2, 3, 4, 17, 21, 22] {
            assert!(!skip_banner(id, shown), "the keys bring {id} back");
        }
        // MAP FOUND is item UI too: hidden by either rule
        assert!(skip_banner(MAP_FOUND, Rules { banners: false, items: true }));
        assert!(skip_banner(MAP_FOUND, Rules { banners: true, items: false }));
        assert!(!skip_banner(1, Rules { banners: false, items: true }), "the item rule alone keeps the felled banners");
    }
}
