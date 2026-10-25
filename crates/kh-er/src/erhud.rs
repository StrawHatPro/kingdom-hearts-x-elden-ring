//! ELDEN RING'S HUD, PART BY PART (build/sora_tmp/er/IMPL_ER8_STATE.md).
//!
//! WHAT THE GAME DOES (eldenring.exe 2.7.1.0, read from the file): `CSFeManImp` holds a `CS::CSFeAutoHideCtrl`
//! (RTTI name; at +0x4E70, the SDK's `auto_hide_ctrl`) with ONE "SHOWN" BYTE PER HUD GROUP, 14 of them at
//! +0x48. Every frame the front-end manager's HUD update (RVA 0x775fd0) sets them: option "HUD: On" = all 1,
//! "HUD: Auto" = 1 while the group's timer runs; then its last step, `apply(ctrl, values)` (RVA 0x768df0),
//! clears the enable byte of every group whose flag is 0 in `FrontEndViewValues` (CSFeManImp + 0x80), which
//! is what the Scaleform HUD reads. Group -> enable byte in the values: 0 +0x10B0, 1 +0x14 (HP), 2 +0x28
//! (FP), 3 +0x3C (stamina), 4 +0x42B2, 5 +0x48, 6 +0xEA0, 7 +0x238, 8 +0x428, 9 +0x578, 10 +0x6C8,
//! 11 +0x10AC, 12 +0xFF0, 13 +0x1178. What each group is on screen: `GROUP_NAMES` (measured in the game).
//!
//! WHAT WE DO: `apply` is detoured (found by pattern; the address must be the 2.7.1.0 one). While the frame
//! task keeps a mask alive (`set`, lapses 250 ms after its last renewal: a dead payload, a loading screen or
//! `hud.kh = 0` give the whole HUD back by themselves), the flags of the masked groups are cleared just
//! before the original runs. The game computes the flags afresh every frame, so nothing has to be put back.
//! Not while its main menu is up (`hud_state` 1): its menu shows its own bars. Bit 16 (`TAGS`): the HP tags
//! over enemies' heads are hidden too (`is_visible` of the 8 enemy tags, as the game's own "HUD: Off" does);
//! the boss bars and the friends' tags are never touched.

use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};

use windows_sys::Win32::System::SystemInformation::GetTickCount64;

use crate::erlog;
use crate::havok::{locate, Located};

pub(crate) const APPLY_FN: Located = Located {
    name: "CSFeAutoHideCtrl apply (HUD groups)",
    pattern: "4c 8b dc 48 81 ec 88 00 00 00 48 8b 05 ? ? ? ? 48 33 c4 48 89 44 24 70 48 8d 82 b0 10 00 00 4c 8b ca",
    capture: 0,
    rva_2_7_1_0: 0x0076_8df0, rva_2_7_2_0: 0x0076_92a0 /* (PORT) */,
};

pub const GROUPS: usize = 14;
/// Mask bit: the enemy HP tags.
pub const TAGS: u32 = 1 << 16;
/// Mask bit (IMPL_ER13, the user: "also remove all boss health bars"): the three boss bars (`boss_list_tag_data`, the
/// three entries behind the eight enemy tags in `FrontEndViewValues`).
pub const BOSS: u32 = 1 << 17;
const BOSS_TAGS: usize = ENEMY_TAGS_AT + 8 * TAG_SIZE;
const ENEMY_TAGS_AT: usize = 0x11D0;
const FLAGS: usize = 0x48;
/// `CSFeManImp::auto_hide_ctrl`, `hud_state`, `frontend_values` (offsets in CSFeManImp).
const CTRL_IN_FE: usize = 0x4E70;
const HUD_STATE_IN_FE: usize = 0x78;
const VALUES_IN_FE: usize = 0x80;
/// `FrontEndViewValues::enemy_chr_tag_data[k].is_visible`.
const ENEMY_TAGS: usize = 0x11D0;
const TAG_SIZE: usize = 0x128;

static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static MASK: AtomicU32 = AtomicU32::new(0);
static UNTIL: AtomicU64 = AtomicU64::new(0);
/// (IMPL_ER13, the user: "I don't want the elden ring HUD to show up even when Sora dies") THE MASK THAT STAYS: set once
/// Sora's HUD has been up for the loaded character, in force in every call of the game's HUD step from then on (Sora's
/// death, the game-over menu, Elden Ring's own death sequence, the respawn, in-session loads: the times between one
/// Sora and the next, when nothing of ours renews the timed mask), until it is taken away explicitly (`unstick`: the
/// title, `hud.kh = 0`, `hud.er = 1`, another mode, the payload's end).
static STICKY: AtomicU32 = AtomicU32::new(0);
/// Calls of the game's that went through the hook / in which groups were hidden (for the log).
pub static CALLS: AtomicU32 = AtomicU32::new(0);
pub static HIDDEN: AtomicU32 = AtomicU32::new(0);

type ApplyFn = unsafe extern "C" fn(*mut u8, *mut u8);

/// (nothing in here can panic: plain loads, byte writes into the front-end manager's own object)
unsafe extern "C" fn apply_hook(ctrl: *mut u8, values: *mut u8) {
    let original: ApplyFn = std::mem::transmute(ORIGINAL.load(Ordering::Relaxed));
    let timed = MASK.load(Ordering::Relaxed);
    let mask = if timed != 0 && GetTickCount64() < UNTIL.load(Ordering::Relaxed) { timed } else { STICKY.load(Ordering::Relaxed) };
    // Only the pair the front-end manager itself passes (the controller and the values are both inside it),
    // and not while its main menu shows the HUD (hud_state 1).
    let live = mask != 0
        && ctrl as usize > 0x10000 + CTRL_IN_FE
        && values as usize == ctrl as usize - CTRL_IN_FE + VALUES_IN_FE
        && *ctrl.sub(CTRL_IN_FE - HUD_STATE_IN_FE) != 1;
    if live {
        for group in 0..GROUPS {
            if mask >> group & 1 != 0 {
                *ctrl.add(FLAGS + group) = 0;
            }
        }
    }
    original(ctrl, values);
    if live {
        if mask & TAGS != 0 {
            for k in 0..8 {
                *values.add(ENEMY_TAGS + k * TAG_SIZE) = 0;
            }
        }
        if mask & BOSS != 0 {
            for k in 0..3 {
                *values.add(BOSS_TAGS + k * TAG_SIZE) = 0;
            }
        }
        HIDDEN.fetch_add(1, Ordering::Relaxed);
    }
    CALLS.fetch_add(1, Ordering::Relaxed);
}

/// Detours the game's HUD-group step (once per payload; undone by the renderer's shutdown with every other
/// inline hook). False: Elden Ring's HUD cannot be hidden part by part (logged; `hud.er = 0` then falls back
/// to the game's own "HUD: Off").
pub fn install() -> bool {
    let Some(f) = locate(&APPLY_FN) else {
        erlog!("erhud: the game's HUD-group step was not found for this executable (PORT: 2.7.1.0 / 2.7.2.0 table): its HUD cannot be hidden part by part");
        return false;
    };
    // SAFETY: the function's start, found by pattern at the recorded address; our hook has its signature.
    match unsafe { kh_er_render::inline_hook(f, apply_hook as *const () as usize, &ORIGINAL, "HUD groups") } {
        Ok(()) => {
            erlog!("erhud: the game's HUD-group step is detoured (Elden Ring's HUD can be hidden part by part while Sora's is up)");
            true
        }
        Err(e) => {
            ORIGINAL.store(0, Ordering::SeqCst);
            erlog!("erhud: the HUD-group step was NOT detoured ({e}): Elden Ring's HUD cannot be hidden part by part");
            false
        }
    }
}

/// The hooks are gone (the renderer's shutdown removed every inline hook).
pub fn uninstalled() {
    STICKY.store(0, Ordering::SeqCst);
    MASK.store(0, Ordering::SeqCst);
    ORIGINAL.store(0, Ordering::SeqCst);
}

pub fn hooked() -> bool {
    ORIGINAL.load(Ordering::Relaxed) != 0
}

/// Renews what is hidden of Elden Ring's HUD for the next 250 ms (0 = nothing: its whole HUD shows).
pub fn set(mask: u32) {
    MASK.store(mask, Ordering::Relaxed);
    // SAFETY: plain getter.
    UNTIL.store(unsafe { GetTickCount64() } + 250, Ordering::Relaxed);
}

/// The whole HUD is the game's again, now.
pub fn clear() {
    MASK.store(0, Ordering::SeqCst);
}

/// The mask that stays in force without renewal (0 = none).
pub fn stick(mask: u32) {
    if STICKY.swap(mask, Ordering::Relaxed) != mask {
        erlog!("erhud: {}", if mask != 0 { format!("Elden Ring's HUD parts {mask:#x} STAY hidden from now on (also while no Sora runs: a death, a respawn, a load)") } else { "Elden Ring's whole HUD is the game's again (the title, a switch, or the payload's end)".to_string() });
    }
}

pub fn unstick() {
    stick(0);
}

/// THE DEFAULT HIDE SET (`hud.er.hide` overrides it). (IMPL_ER23A, user decision 2026-10-13: "also remove the elden ring
/// compass") EVERY GROUP: 0 - 11 and 13 as before (bars, slots, rune counter, the two unseen ones), 12 = THE COMPASS (kept
/// until now), the enemy tags, the boss bars. What has no group stays: the action prompt, item windows, area banners.
pub const DEFAULT_HIDE: &str = "0,1,2,3,4,5,6,7,8,9,10,11,12,13,tags,boss";

/// `hud.er.hide = 1,2,3,tags` -> the mask (group numbers 0..13, `tags`); `None`: not understood.
pub fn parse_mask(text: &str) -> Option<u32> {
    let mut mask = 0;
    for part in text.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        if part == "tags" {
            mask |= TAGS;
        } else if part == "boss" {
            mask |= BOSS;
        } else {
            let group: usize = part.parse().ok().filter(|g| *g < GROUPS)?;
            mask |= 1 << group;
        }
    }
    Some(mask)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mask_text_is_groups_and_tags() {
        assert_eq!(parse_mask("1, 2,3"), Some(0b1110));
        assert_eq!(parse_mask("13,tags"), Some(1 << 13 | TAGS));
        assert_eq!(parse_mask("tags, boss"), Some(TAGS | BOSS));
        assert_eq!(parse_mask(""), Some(0));
        assert_eq!(parse_mask("14"), None);
        assert_eq!(parse_mask("x"), None);
    }

    /// (IMPL_ER23A) the default: all 14 groups (the compass, group 12, included), the enemy tags, the boss bars.
    #[test]
    fn the_default_hides_every_group_the_compass_too() {
        let mask = parse_mask(DEFAULT_HIDE).unwrap();
        assert_eq!(mask, 0x3fff | TAGS | BOSS);
        assert!(mask >> 12 & 1 == 1, "the compass");
        // the old default (the compass kept) is still sayable
        assert_eq!(parse_mask("0,1,2,3,4,5,6,7,8,9,10,11,13,tags,boss"), Some(mask & !(1 << 12)));
    }
}

// ---------------------------------------------------------------- a boss bar on demand (testing aid)

static BOSS_SAVED: std::sync::Mutex<Option<[u8; 0x20]>> = std::sync::Mutex::new(None);

/// (IMPL_ER22) The characters the game's three boss bars point at now (`CSFeManImp::boss_health_displays`: the
/// game's own word for "this is a boss", field and dungeon bosses alike). The caller matches the handles against
/// the characters it reads, so an unused entry (whatever its handle holds) matches nobody. Game thread only.
pub fn boss_bar_handles() -> Vec<eldenring::cs::FieldInsHandle> {
    use eldenring::cs::CSFeManImp;
    use fromsoftware_shared::FromStatic;
    // (the testing aid above points entry 0 at the player: not a boss)
    let test = BOSS_SAVED.lock().map(|s| s.is_some()).unwrap_or(false);
    // SAFETY: game thread (a task phase); the singleton lives as long as the game.
    let Ok(fe) = (unsafe { CSFeManImp::instance() }) else { return Vec::new() };
    fe.boss_health_displays.iter().enumerate().filter(|(k, e)| e.fmg_id > 0 && !(test && *k == 0)).map(|(_, e)| e.field_ins_handle).collect()
}

/// `dbg.bossbar = <NpcName fmg id>` (IMPL_ER8, testing aid; e.g. 903251600 = "Tree Sentinel"): the game's first
/// boss-bar entry (`CSFeManImp::boss_health_displays[0]`) is pointed at `handle` (the caller passes the player's
/// own character) under that name, so that the game's boss bar can be SEEN under the HUD mode without a boss at
/// hand; `None` puts the entry back as it was. Game thread only.
pub fn test_boss_bar(ask: Option<(i32, eldenring::cs::FieldInsHandle)>) {
    use eldenring::cs::CSFeManImp;
    use fromsoftware_shared::FromStatic;
    let Ok(mut saved) = BOSS_SAVED.lock() else { return };
    if ask.is_none() && saved.is_none() {
        return;
    }
    // SAFETY: game thread (a task phase); the singleton lives as long as the game.
    let Ok(fe) = (unsafe { CSFeManImp::instance_mut() }) else { return };
    let entry = &mut fe.boss_health_displays[0];
    // SAFETY: the entry is 0x20 plain bytes (asserted below).
    let bytes = unsafe { &mut *(entry as *mut eldenring::cs::BossHealthDisplayEntry as *mut [u8; 0x20]) };
    const _: () = assert!(std::mem::size_of::<eldenring::cs::BossHealthDisplayEntry>() == 0x20);
    match ask {
        Some((fmg, handle)) => {
            if saved.is_none() {
                *saved = Some(*bytes);
                erlog!("erhud: dbg.bossbar: the game's first boss-bar entry (was name {}) now shows name {fmg} for the given character", entry.fmg_id);
            }
            entry.fmg_id = fmg;
            entry.field_ins_handle = handle;
        }
        None => {
            if let Some(before) = saved.take() {
                *bytes = before;
                erlog!("erhud: dbg.bossbar: the game's boss-bar entry is put back");
            }
        }
    }
}
