//! (MAP-GATE, the user 2026-11-08: "using a ladder disables anything to do with the map, fix this") ELDEN RING'S OWN GATE
//! ON ITS MAP, read from the game, and ITS OWN MAP REQUEST, made by us when that gate keeps refusing.
//! (NO-ER-PAUSE CORRECTION, 2026-11-09, read in the same copy: nopause.rs) `+ 0x121` IS THE IN-GAME TOP MENU'S REQUEST, NOT
//! THE MAP'S: the update builds `02_000_IngameTop` (CS::MainTopDialog) for it; the map is `+ 0x122` (set by 0x1407ef2c0 from
//! the View gate 0x140e2ce90; the update builds `02_120_WorldMap` for it). The request below writes `+ 0x122`. The gate read
//! by [`read`] is the top menu's gate (0x140e2d120); the map's gate (0x140e2ce90) asks the same parts of the game (its menus,
//! the waiting action-button press, the body's menu flags and actions), so its words stay right for the log.
//!
//! WHAT THE GAME DOES WITH VIEW (eldenring.exe 2.7.2.0, read in the copy build/sora_tmp/quit_crash/eldenring_272.exe; the
//! same bytes in 2.7.1.0: the setter below matched once there, relocations wildcarded):
//! - the field input handler (0x140b037b0) asks the map key's gate (0x140e2d120) and, when it passes, calls the map
//!   request (0x1407ef270): `CSPopupMenu + 0x121 = 1` (only while interface element 0x1c, the map, is not created);
//! - the popup menu's update (0x1407efb80, every frame from CSMenuMan's update) opens `02_120_WorldMap` when that byte is
//!   set and the map is not up, and clears it. The save point's "Select an Area" presses View, so it goes the same way.
//! - THE GATE (0x140e2d120), in its order: the map key pressed this frame; `CSMenuMan + 0x1c` == 0 and `+ 0x798` == 0 (no
//!   menu of its own busy); `CSActionButtonMan + 0x81` == 0 (NO ACTION-BUTTON PRESS WAITING to be carried out);
//!   a world object count above 0; then on the player: its menu flags' bit 3 set (`player_menu_ctrl` + 0x20: the reset
//!   value 9 each frame, cleared by TAE JumpTable 54), alive (data module hp > 0), no SpEffect of stateInfo 117 (the
//!   regulation has one: SpEffect 70), no use-item cancel waiting (action request bit 7), no camera fade, the interface
//!   element 0x41 not up. The handler itself also skips the map while a real-time cutscene plays
//!   (`CSRemo -> remo_man + 0x14C` bit 0).
//! - THE ACTION-BUTTON PRESS (0x140a65a90, the action-button manager's update): Y released at a prompt sets `+ 0x81`; whoever carries
//!   the action out sets `+ 0x80`, and the next update clears both. A press nobody carries out stays set as long as an
//!   action region is current, and the map gate refuses all that time.
//!
//! WHAT IS DONE (mapwatch.rs decides when): [`read`] names every part of that gate the game holds in data (logged with every
//! map request that fails); [`consume_action_press`] ends a stale action-button press the way the game ends one it carried
//! out; [`request_map`] sets the game's own map request byte (what its View handler sets) so the map opens through the
//! game's own code whatever part of the gate refused, and [`cancel_request`] takes it back if the map did not come.
//! No function of the game is called; every pointer is checked before it is followed.
//!
//! (CONT-CRASH, 2026-11-10: "the game just crashed pressing continue") THE PRESS BYTES ARE THE ACTION-BUTTON MANAGER'S
//! (`CSActionButtonManImp`, the SDK's `CSActionButtonMan`), NOT THE REGION SYSTEM'S. The gate reads the manager: 0x140e2d239
//! `mov rax, [0x143d75498]; cmp [rax + 0x81], dil` (0x143d75498 = the manager, RTTI `CSActionButtonManImp`, vtable
//! 0x142b4a230, its DLPlainLightMutex at + 0x90; the update 0x140a65a90 runs on it). Until DEPLOY10 these offsets were
//! applied to the singleton `CSActionButtonRegionSystem` = `CSActionButtonRegionSystemImp` (vtable 0x142b4a5a0, 0x490
//! bytes, 15 elements of 0x48 at + 0x18, each a DLPlainLightMutex at + 0x18): its + 0x80 is the DebugInfo word of
//! element 1's CRITICAL_SECTION (-1 = none, so + 0x81 read 0xFF: "a press is waiting" every time) and the "consume" wrote
//! 1 into it (DebugInfo 0xFFFFFFFFFFFFFF01). The next load tore the region system down (FieldArea's destructor 0x14061aee0
//! -> 0x140a66ff0 -> element destructor 0x140a670d0 -> DeleteCriticalSection) and ntdll faulted unlinking that debug
//! info (the user's dump eldenring.exe.3536.dmp: c0000005 at ntdll RtlDeleteCriticalSection+0x95, rdi 0xFFFFFFFFFFFFFF01;
//! the log's ladder let-go at 185.3 s "the action-button press that waited was ENDED", Continue at 202.6 s).
//! Now: the manager by the SDK's singleton, the mutex placement asserted at compile time, and a byte is written only when
//! both press bytes read as the flags they are (0 or 1).

use std::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};

use eldenring::cs::{CSActionButtonMan, CSMenuManImp, CSRemo, PlayerIns};
use fromsoftware_shared::FromStatic;

use crate::game::plausible;
use crate::word;

/// `CSPopupMenu` + this: the map request byte (0x1407ef2c0 writes 1, 0x1407efb80 opens the map and writes 0; NO-ER-PAUSE: was
/// 0x121, the top menu's byte, which nopause.rs now refuses).
pub const POPUP_MAP_REQUEST: usize = crate::nopause::MAP_REQUEST;
/// `CSMenuManImp` + these must be 0 for the map key (0x140e2d120).
pub const MENU_BUSY: usize = 0x1c;
pub const MENU_JOB: usize = 0x798;
/// `CSActionButtonManImp` (the SDK's `CSActionButtonMan`) + these: the press carried out (cleared next update with the
/// next two) / the press waiting / its follow-up byte (0x140a65a90; the gate 0x140e2d120 reads + 0x81 of the manager).
pub const PRESS_DONE: usize = 0x80;
pub const PRESS_WAITING: usize = 0x81;
/// `CSRemoMan` + this, bit 0: a real-time cutscene runs (0x140aaba80; set by its starts, cleared by its ends).
pub const REMO_FLAGS: usize = 0x14c;
/// `CSPlayerMenuCtrl` + this: its menu flags (the bits TAE JumpTable 54 clears; reset to 9 each frame).
pub const MENU_FLAGS: usize = 0x20;
/// (MAP-NOW) `CSPopupMenu + 8` -> `+ 0x90 + 0x1c` bit 0: the map's interface element (0x1c) is created. Read by the popup
/// update 0x1407efb80 (build/sora_tmp/no_er_pause/decomp_popup_update_272.c: `lVar7 = *(param_1 + 8)`, then
/// `*(byte *)(0x90 + 0x1c + lVar7) & 1`); `+ 0x90` is `CSMenuManImp::ui_states` in the SDK (asserted below).
pub const POPUP_MENU_MAN: usize = 0x8;
pub const UI_STATES: usize = 0x90;
pub const UI_MAP: usize = 0x1c;
/// (MAP-NOW) THE PARTS OF THE MAP'S OWN GATE 0x140e2ce90 THAT ARE LOGGED, NOT ACTED ON (one of them, or the two the game
/// keeps in globals, is what stays shut after the LADDER2 leap: build/sora_tmp/map_now/STATE.md):
/// - interface element 2 created and visible: 0x14076f390 reads `[[0x143d72910] + 0x18] + 0x90 + 2`, `& 3 == 3` refuses
///   (+ 0x18 is the third argument of its constructor 0x14076b9e0, given by 0x140767340 from the menu manager);
/// - `ChrCtrl.modifier + 0x24` bit 0: 0x1403f4740 reads `[[player + 0x58] + 0xc8] + 0x24 & 1`, set refuses;
/// - the event module's state: the gate reads `[[modules + 0x58] + 0x10]` as an int, 1 or 2 refuses (0x140e2d00f).
pub const UI_GATE_ELEMENT: usize = 2;
pub const MODIFIER_FLAGS: usize = 0x24;
pub const EVENT_STATE_PTR: usize = 0x10;

// The SDK's layout must still put its named fields where the exe reads them (a newer SDK pin fails here, not in game).
const _: () = assert!(std::mem::offset_of!(PlayerIns, player_menu_ctrl) == 0x6a0);
const _: () = assert!(std::mem::offset_of!(CSMenuManImp, popup_menu) == 0x80);
const _: () = assert!(std::mem::offset_of!(eldenring::cs::CSPlayerMenuCtrl, chr_menu_flags) == 0x18);
const _: () = assert!(std::mem::offset_of!(eldenring::cs::CSChrDataModule, hp) == 0x138);
// (MAP-NOW) the places the map gate 0x140e2ce90 and the popup update 0x1407efb80 read, as the SDK names them
const _: () = assert!(std::mem::offset_of!(CSMenuManImp, ui_states) == UI_STATES);
const _: () = assert!(std::mem::offset_of!(PlayerIns, chr_ins) == 0);
const _: () = assert!(std::mem::offset_of!(eldenring::cs::ChrIns, chr_ctrl) == 0x58);
const _: () = assert!(std::mem::offset_of!(eldenring::cs::ChrIns, modules) == 0x190);
const _: () = assert!(std::mem::offset_of!(eldenring::cs::ChrCtrl, modifier) == 0xc8);
const _: () = assert!(std::mem::offset_of!(eldenring::cs::ChrInsModuleContainer, event) == 0x58);
const _: () = assert!(std::mem::offset_of!(eldenring::cs::ChrInsModuleContainer, action_request) == 0x80);
// (MAP-NOW, NO-ER-PAUSE) the one popup byte written for the map is the map's own, never the top menu's
const _: () = assert!(POPUP_MAP_REQUEST == 0x122 && POPUP_MAP_REQUEST != crate::nopause::MENU_REQUEST);
// (CONT-CRASH) the press bytes lie in the manager's plain fields, before its mutex (the destructor 0x140a64210 deletes the
// mutex at + 0x90): a write there can never reach a CRITICAL_SECTION again
const _: () = assert!(std::mem::offset_of!(CSActionButtonMan, mutex) == 0x90);
const _: () = assert!(PRESS_DONE < 0x90 && PRESS_WAITING < 0x90 && PRESS_DONE >= 8);

/// (CONT-CRASH) The two press bytes as the flags they are: `Some(waiting)` only when both read 0 or 1 (pure). Anything
/// else is not the manager's press state (the old wrong object read 0xFF there) and is never written.
pub fn press_state(done: u8, waiting: u8) -> Option<bool> {
    (done <= 1 && waiting <= 1).then_some(waiting == 1)
}

/// The parts of the game's map gate held in data (None: not readable now). (MAP-NOW) The fields up to `item_cancel` are the
/// parts the map gate 0x140e2ce90 asks that we read and RESPECT (a refusal there keeps the map shut, as in Elden Ring);
/// the three after it are logged only (see the consts): one of the parts we cannot name for sure stays shut after the
/// LADDER2 leap, and the log names their values so the next session shows which.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Gate {
    pub menu_busy: Option<bool>,
    pub menu_job: Option<bool>,
    pub action_press_waiting: Option<bool>,
    pub cutscene: Option<bool>,
    pub start_inputs_off: Option<bool>,
    pub dead: Option<bool>,
    pub item_cancel: Option<bool>,
    /// (logged only) interface element 2 created and visible (0x14076f390).
    pub ui_element_2: Option<bool>,
    /// (logged only) `ChrCtrl.modifier + 0x24` bit 0 (0x1403f4740).
    pub modifier_bit: Option<bool>,
    /// (logged only) the event module's state int (1 or 2 refuses).
    pub event_state: Option<i32>,
}

impl Gate {
    /// What of it refuses the map, in words (empty: none of the parts we read and respect).
    pub fn refusals(&self) -> Vec<&'static str> {
        let parts: [(Option<bool>, &'static str); 7] = [
            (self.menu_busy, "a menu of the game's is busy (CSMenuMan +0x1c)"),
            (self.menu_job, "a menu job of the game's is pending (CSMenuMan +0x798)"),
            (self.action_press_waiting, "AN ACTION-BUTTON PRESS IS WAITING to be carried out (CSActionButtonMan +0x81)"),
            (self.cutscene, "a real-time cutscene runs (CSRemoMan +0x14C bit 0)"),
            (self.start_inputs_off, "the body's menu flags hold the menus shut (TAE JumpTable 54: bit 3 clear)"),
            (self.dead, "the body is dead (hp 0)"),
            (self.item_cancel, "a use-item cancel is waiting on the body"),
        ];
        parts.iter().filter(|(v, _)| *v == Some(true)).map(|(_, s)| *s).collect()
    }

    /// The respected parts that were not readable (for the log).
    pub fn unread(&self) -> usize {
        [self.menu_busy, self.menu_job, self.action_press_waiting, self.cutscene, self.start_inputs_off, self.dead, self.item_cancel].iter().filter(|v| v.is_none()).count()
    }

    /// (MAP-NOW) The game's own map request may be made now: none of the parts we read and respect refuses. A waiting
    /// action-button press is no refusal here: it is ended first, as the game ends a press it carried out (MAP-GATE).
    pub fn map_may_open(&self) -> bool {
        [self.menu_busy, self.menu_job, self.cutscene, self.start_inputs_off, self.dead, self.item_cancel].iter().all(|v| *v != Some(true))
    }

    /// (MAP-NOW) The logged-only parts, in words: what the game's gate would also have asked (values, `?` = not readable).
    pub fn logged_parts(&self) -> String {
        let b = |v: Option<bool>| v.map_or("?".to_string(), |x| if x { "SET".to_string() } else { "clear".to_string() });
        let refuses = self.ui_element_2 == Some(true) || self.modifier_bit == Some(true) || self.event_state.is_some_and(|s| s == 1 || s == 2);
        format!(
            "parts not acted on: interface element 2 {}, ChrCtrl modifier +0x24 bit 0 {}, event module state {}{}",
            b(self.ui_element_2),
            b(self.modifier_bit),
            self.event_state.map_or("?".to_string(), |s| s.to_string()),
            if refuses { " (ONE OF THESE WOULD KEEP THE GAME'S OWN ROUTE SHUT)" } else { "" }
        )
    }

    /// The line of it.
    pub fn text(&self) -> String {
        let r = self.refusals();
        let unread = self.unread();
        format!("{}{} | {}", if r.is_empty() { "nothing of the parts read refuses".to_string() } else { r.join("; ") }, if unread > 0 { format!(" ({unread} part(s) not readable)") } else { String::new() }, self.logged_parts())
    }
}

fn byte_at(addr: usize) -> Option<u8> {
    let mut b = [0u8; 1];
    crate::probe::read(addr, &mut b).then_some(b[0])
}

/// (CONT-CRASH) The action-button manager the gate reads (the SDK's `CSActionButtonMan`, as erprompt.rs reads it).
fn button_man() -> Option<usize> {
    // SAFETY: game thread; the singleton lives as long as the game (only its address is taken).
    let s = unsafe { CSActionButtonMan::instance() }.ok()? as *const CSActionButtonMan as usize;
    plausible(s).then_some(s)
}

/// (CONT-CRASH) The manager's press state, read through the checked reader: (manager, waiting) only when both bytes are flags.
fn press() -> Option<(usize, bool)> {
    let s = button_man()?;
    press_state(byte_at(s + PRESS_DONE)?, byte_at(s + PRESS_WAITING)?).map(|waiting| (s, waiting))
}

fn menu_man() -> Option<usize> {
    // SAFETY: game thread; the singleton lives as long as the game (only its address is taken).
    let m = unsafe { CSMenuManImp::instance() }.ok()? as *const CSMenuManImp as usize;
    plausible(m).then_some(m)
}

fn popup() -> Option<usize> {
    // SAFETY: game thread; the singleton lives as long as the game; the pointer is checked before it is followed.
    let menu = unsafe { CSMenuManImp::instance() }.ok()?;
    let p = word!(menu.popup_menu);
    plausible(p).then_some(p)
}

/// The gate as the game holds it now, for the player's body.
pub fn read(player: &PlayerIns) -> Gate {
    let menu = menu_man();
    let remo = {
        // SAFETY: game thread; the singleton lives as long as the game; the pointer is checked before it is followed.
        unsafe { CSRemo::instance() }.ok().map(|r| word!(r.remo_man)).filter(|m| plausible(*m))
    };
    let ctrl = word!(player.player_menu_ctrl);
    let modules = word!(player.chr_ins.modules);
    let (data, request, event) = if plausible(modules) {
        let m = &*player.chr_ins.modules;
        (Some(word!(m.data)).filter(|p| plausible(*p)), Some(word!(m.action_request)).filter(|p| plausible(*p)), Some(word!(m.event)).filter(|p| plausible(*p)))
    } else {
        (None, None, None)
    };
    // (MAP-NOW) the chr control's modifier (`[[player + 0x58] + 0xc8]`), read through the checked reader
    let modifier = Some(word!(player.chr_ins.chr_ctrl)).filter(|c| plausible(*c)).and_then(|c| crate::probe::word(c + 0xc8)).filter(|m| plausible(*m));
    Gate {
        menu_busy: menu.and_then(|m| crate::probe::u32_at(m + MENU_BUSY)).map(|v| v != 0),
        menu_job: menu.and_then(|m| crate::probe::word(m + MENU_JOB)).map(|v| v != 0),
        action_press_waiting: press().map(|(_, waiting)| waiting),
        cutscene: remo.and_then(|m| crate::probe::u32_at(m + REMO_FLAGS)).map(|v| v & 1 != 0),
        start_inputs_off: Some(ctrl).filter(|c| plausible(*c)).and_then(|c| crate::probe::u32_at(c + MENU_FLAGS)).map(|v| v & 8 == 0),
        dead: data.and_then(|d| crate::probe::u32_at(d + 0x138)).map(|hp| (hp as i32) <= 0),
        // (CSChrActionRequestModule.cancel_ready_actions at 0x30, bit 7 = ACTION_ARM_USE_ITEM: the gate's 0x1404079c0(.., 0, 7))
        item_cancel: request.and_then(|r| crate::probe::word(r + 0x30)).map(|v| v & (1 << 7) != 0),
        ui_element_2: menu.and_then(|m| byte_at(m + UI_STATES + UI_GATE_ELEMENT)).map(|v| v & 3 == 3),
        modifier_bit: modifier.and_then(|m| crate::probe::u32_at(m + MODIFIER_FLAGS)).map(|v| v & 1 != 0),
        event_state: event.and_then(|e| crate::probe::word(e + EVENT_STATE_PTR)).filter(|p| plausible(*p)).and_then(crate::probe::u32_at).map(|v| v as i32),
    }
}

/// THE STALE ACTION-BUTTON PRESS ENDED as the game ends a press it carried out (`+ 0x80 = 1`: its next update clears the
/// press). True: written.
pub fn consume_action_press() -> bool {
    let Some((s, true)) = press() else { return false };
    // SAFETY: the manager's own press flag (both press bytes just read as flags through the checked reader; the field lies
    // before its mutex, asserted above); game thread.
    unsafe { std::ptr::write_volatile((s + PRESS_DONE) as *mut u8, 1) };
    true
}

/// THE GAME'S OWN MAP REQUEST, as its View handler makes it: the map opens through its own code next frame. True: written.
pub fn request_map() -> bool {
    let Some(p) = popup() else { return false };
    if byte_at(p + POPUP_MAP_REQUEST).is_none() {
        return false;
    }
    // SAFETY: a byte inside the live popup menu (read just above through the checked reader); game thread.
    unsafe { std::ptr::write_volatile((p + POPUP_MAP_REQUEST) as *mut u8, 1) };
    true
}

/// The request taken back (the map did not come: it must not open by itself later). True: written.
pub fn cancel_request() -> bool {
    let Some(p) = popup() else { return false };
    if byte_at(p + POPUP_MAP_REQUEST).is_none_or(|v| v == 0) {
        return false;
    }
    // SAFETY: as in `request_map`.
    unsafe { std::ptr::write_volatile((p + POPUP_MAP_REQUEST) as *mut u8, 0) };
    disarm();
    true
}

// ------------------------------------------------------------------ (MAP-NOW) the request made at the popup menu's update
//
// (MAP-NOW, the user 2026-11-10: "pressing the map button there is a slight delay before the map actually shows up") The
// request a press earns is no longer made 0.6 s later from our frame: mapwatch.rs ARMS it on the press's own frame, and the
// popup menu's update (0x1407efb80, detoured by nopause.rs) writes the map byte `+ 0x122` right before the game's own update
// runs, in the same frame when the update comes after our frame, else in the next one. The update then opens
// `02_120_WorldMap` itself (decomp_popup_update_272.c: `+0x122` set -> 0x1407fc610, then `+0x122 = 0`).
// - The byte is written only while it is 0 (the game's own View handler 0x140b037b0 -> 0x1407ef2c0 may have set it: then
//   its own route took the press and nothing of ours is written) and while the map's element 0x1c is not created.
// - ONE MAP, NEVER TWO: the update opens the map for a set byte without asking whether one is up (its `+0x122` branch has
//   no such check). A byte set again within `ONE_MAP_MS` of a map the update just opened (ours and the game's own handler
//   on the same press) is put back at 0 before the update sees it.
// - Never `+ 0x121` (the top menu's: NO-ER-PAUSE refuses it in the same detour, before this).


/// An armed request lapses this long after the press (the update runs every frame; it is taken at the first one).
pub const ARM_MS: u64 = 150;
/// A second map request this soon after the update opened one is a duplicate of the same press.
pub const ONE_MAP_MS: u64 = 500;

static ARMED_UNTIL: AtomicU64 = AtomicU64::new(0);
static LAST_OPEN_AT: AtomicU64 = AtomicU64::new(0);
/// The latest armed request's outcome at the update (`Step as u8`, 0 = none yet), taken by mapwatch.rs.
static OUTCOME: AtomicU8 = AtomicU8::new(0);
/// Duplicate map requests put back at 0 so far.
pub static DROPPED: AtomicU32 = AtomicU32::new(0);

fn tick() -> u64 {
    // SAFETY: plain getter.
    unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() }
}

/// (MAP-NOW) What the popup update's detour does with the map byte this frame (pure).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Step {
    /// Nothing armed, the byte left as it is.
    Nothing = 1,
    /// Our request written: `+ 0x122 = 1`.
    Write = 2,
    /// The byte was already set by the game's own View route: it is the game's request (nothing written).
    GameHas = 3,
    /// The map's element is already created: nothing written.
    MapUp = 4,
    /// A duplicate of a map opened less than `ONE_MAP_MS` ago: `+ 0x122 = 0`.
    Drop = 5,
}

/// THE RULE (pure): `request` = the map byte as the update is about to see it, `map_created` = element 0x1c created,
/// `armed` = a press of ours waits, `since_open_ms` = since the update last opened a map (None: never).
pub fn popup_step(request: u8, map_created: bool, armed: bool, since_open_ms: Option<u64>) -> Step {
    if request != 0 {
        if since_open_ms.is_some_and(|ms| ms < ONE_MAP_MS) {
            return Step::Drop;
        }
        return if armed { Step::GameHas } else { Step::Nothing };
    }
    match (armed, map_created) {
        (false, _) => Step::Nothing,
        (true, true) => Step::MapUp,
        (true, false) => Step::Write,
    }
}

/// (MAP-NOW) The game's own map request for a press, made at the popup update's next run (or at once, the old way, when
/// that update could not be detoured). True: armed or written.
pub fn arm() -> bool {
    if !crate::nopause::detoured() {
        return request_map();
    }
    OUTCOME.store(0, Ordering::Relaxed);
    ARMED_UNTIL.store(tick() + ARM_MS, Ordering::Relaxed);
    true
}

pub fn disarm() {
    ARMED_UNTIL.store(0, Ordering::Relaxed);
}

/// (MAP-NOW) The armed request's outcome at the update, once (None: not taken yet, or nothing was armed).
pub fn take_outcome() -> Option<Step> {
    match OUTCOME.swap(0, Ordering::Relaxed) {
        2 => Some(Step::Write),
        3 => Some(Step::GameHas),
        4 => Some(Step::MapUp),
        5 => Some(Step::Drop),
        _ => None,
    }
}

/// (MAP-NOW, nopause.rs's detour, game thread) Before the game's own popup update: the step applied to the map byte.
/// Returns the byte as the update will see it (for `after_update`).
pub(crate) fn before_update(popup: usize) -> u8 {
    let Some(request) = byte_at(popup + POPUP_MAP_REQUEST) else { return 0 };
    let now = tick();
    let armed = now < ARMED_UNTIL.load(Ordering::Relaxed);
    let last = LAST_OPEN_AT.load(Ordering::Relaxed);
    let since = (last != 0).then(|| now.saturating_sub(last));
    let map_created = crate::probe::word(popup + POPUP_MENU_MAN).filter(|m| plausible(*m)).and_then(|m| byte_at(m + UI_STATES + UI_MAP)).is_none_or(|v| v & 1 != 0);
    let step = popup_step(request, map_created, armed, since);
    let out = match step {
        Step::Write => {
            // SAFETY: the map byte of the live popup menu the game's own update was just called with (read above through the
            // checked reader); game thread, inside that update.
            unsafe { std::ptr::write_volatile((popup + POPUP_MAP_REQUEST) as *mut u8, 1) };
            1
        }
        Step::Drop => {
            // SAFETY: as above.
            unsafe { std::ptr::write_volatile((popup + POPUP_MAP_REQUEST) as *mut u8, 0) };
            DROPPED.fetch_add(1, Ordering::Relaxed);
            0
        }
        _ => request,
    };
    if armed && step != Step::Nothing {
        disarm();
        OUTCOME.store(step as u8, Ordering::Relaxed);
    } else if step == Step::Drop {
        OUTCOME.store(step as u8, Ordering::Relaxed);
    }
    out
}

/// (MAP-NOW) After the game's own popup update: a byte it consumed is a map it opened (or its "cannot" message).
pub(crate) fn after_update(popup: usize, before: u8) {
    if before != 0 && byte_at(popup + POPUP_MAP_REQUEST) == Some(0) {
        LAST_OPEN_AT.store(tick(), Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn clear_gate() -> Gate {
        Gate { menu_busy: Some(false), menu_job: Some(false), action_press_waiting: Some(false), cutscene: Some(false), start_inputs_off: Some(false), dead: Some(false), item_cancel: Some(false), ui_element_2: Some(false), modifier_bit: Some(false), event_state: Some(0) }
    }

    /// (MAP-NOW) Every part we read is RESPECTED (the user: "when the game's gate refuses, don't force it"); a waiting press
    /// is ended, not a refusal; the logged-only parts never decide.
    #[test]
    fn the_gate_names_what_refuses_and_what_is_respected() {
        let clear = clear_gate();
        assert!(clear.refusals().is_empty() && clear.map_may_open() && clear.unread() == 0);
        let stale = Gate { action_press_waiting: Some(true), ..clear };
        assert!(stale.text().contains("ACTION-BUTTON PRESS IS WAITING"));
        assert!(stale.map_may_open(), "a stale press is ended, then the request is made");
        // (fails on the DEPLOY8-10 code: it went past the body's menu flags and the use-item cancel)
        for shut in [
            Gate { cutscene: Some(true), ..clear },
            Gate { dead: Some(true), ..clear },
            Gate { menu_busy: Some(true), ..clear },
            Gate { menu_job: Some(true), ..clear },
            Gate { start_inputs_off: Some(true), ..clear },
            Gate { item_cancel: Some(true), ..clear },
        ] {
            assert!(!shut.map_may_open(), "{shut:?}");
        }
        // the logged-only parts: named in the line, never deciding
        for logged in [Gate { ui_element_2: Some(true), ..clear }, Gate { modifier_bit: Some(true), ..clear }, Gate { event_state: Some(2), ..clear }] {
            assert!(logged.map_may_open() && logged.refusals().is_empty(), "{logged:?}");
            assert!(logged.text().contains("ONE OF THESE WOULD KEEP THE GAME'S OWN ROUTE SHUT"), "{}", logged.text());
        }
        assert!(clear.text().contains("parts not acted on: interface element 2 clear, ChrCtrl modifier +0x24 bit 0 clear, event module state 0") && !clear.text().contains("WOULD KEEP"));
        let unknown = Gate::default();
        assert!(unknown.map_may_open() && unknown.unread() == 7 && unknown.text().contains("7 part(s) not readable"));
    }

    /// (MAP-NOW) The popup update's step: our request only into an empty byte with no map up; the game's own byte is the
    /// game's; a second request right after a map was opened is dropped (one map, never two); nothing armed = nothing done.
    #[test]
    fn the_map_byte_at_the_popup_update() {
        assert_eq!(popup_step(0, false, true, None), Step::Write);
        assert_eq!(popup_step(0, false, true, Some(5000)), Step::Write);
        assert_eq!(popup_step(1, false, true, None), Step::GameHas, "the game's own View route set it: nothing of ours");
        assert_eq!(popup_step(0, true, true, None), Step::MapUp, "the map's element is created: nothing written");
        assert_eq!(popup_step(1, true, false, Some(16)), Step::Drop, "the same press asked twice: the second map never opens");
        assert_eq!(popup_step(1, false, true, Some(ONE_MAP_MS - 1)), Step::Drop);
        assert_eq!(popup_step(1, false, false, Some(ONE_MAP_MS)), Step::Nothing, "a request of the game's later on is its own");
        assert_eq!(popup_step(0, false, false, None), Step::Nothing, "nothing armed: the byte is never touched");
        assert_eq!(popup_step(1, false, false, None), Step::Nothing);
        // NO-ER-PAUSE: the byte this writes is the map's (0x122), never the top menu's (0x121)
        assert_eq!(POPUP_MAP_REQUEST, 0x122);
        assert_ne!(POPUP_MAP_REQUEST, crate::nopause::MENU_REQUEST);
    }

    /// (CONT-CRASH) Only flag bytes are a press state. The bytes the old code read and wrote were a CRITICAL_SECTION's
    /// DebugInfo word (-1: 0xFF 0xFF): never a press, never written.
    #[test]
    fn only_flag_bytes_are_a_press_state() {
        assert_eq!(press_state(0, 0), Some(false));
        assert_eq!(press_state(0, 1), Some(true));
        assert_eq!(press_state(1, 1), Some(true));
        assert_eq!(press_state(1, 0), Some(false));
        assert_eq!(press_state(0xFF, 0xFF), None, "a CRITICAL_SECTION's empty DebugInfo");
        assert_eq!(press_state(0x01, 0xFF), None, "the DebugInfo after the old write (0x..FF01)");
        assert_eq!(press_state(0, 2), None);
    }
}
