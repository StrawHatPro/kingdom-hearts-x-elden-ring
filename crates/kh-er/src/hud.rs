//! (hud) SORA'S HUD, CAMP MENU AND GAME-OVER MENU IN ELDEN RING (build/sora_tmp/er/IMPL_ER_HUD_STATE.md).
//!
//! EVERYTHING HERE IS OFF UNTIL ITS KEY IS IN THE CONTROL FILE:
//! - `hud.kh = 1`: the Kingdom Hearts HUD is drawn (kh-ui through `kh_host::hud`, pictures rasterised on the
//!   "hud-raster" thread, drawn by `kh_er_render::hud` at Present). `hud.gamma = 1`: display-space blending
//!   even into an 8-bit back buffer. `hud.cmd = WORDS` (one-shot): a `hud-cmd` debug word.
//! - `hud.er` (IMPL_ER8_STATE.md): absent / 0 = Elden Ring's HUD PART BY PART while ours is up (erhud.rs: its compass,
//!   prompts, banners, popups and boss bars stay; its bars, slots, rune counter and enemy tags go; `hud.er.hide` = the
//!   groups); `off` = its own option "HUD: Off" (compass and boss bars go too); `hideall` = the hud_state route (see
//!   `er_hud_phase`; the map / grace / cutscene are then told by interface elements only: `hud.menu_ui = 28,61,65`);
//!   1 = its whole HUD stays. While a full-screen menu of Elden Ring is up our overlay fades out (`hud.over_menus = 1`: not).
//! - `in.kh_menu = 1` (the gameplay side's key): Start reaches the HUD: the camp menu (or the Pause screen in
//!   battle). `menu.pause = 1`: Elden Ring's world is frozen while the menu is open (`CSFlipper::game_speed`;
//!   UNTESTED; `menu.pause_speed`, default 0; a watchdog gives the speed back if our frames stop coming).
//! - `gameover.kh = 1`: Sora's death shows KH3's game-over menu first; the Tarnished is kept alive and
//!   unhittable meanwhile and dies (Elden Ring's own death and respawn) when Continue is chosen.
//!   `gameover.banner = 0`: try to keep Elden Ring's YOU DIED text away (UNTESTED field write).
//!
//! HOOKS in the gameplay side's files, all marked `(hud)`: sora.rs (the field `hud`, its constructor line, the
//! core's UI sink in the two `Host { .. }` literals, `er_ui`, `menu_open` in the pause and Start lines, `input`,
//! `extra_mask`, `frame`, `gameover_holds` at the death and the invulnerability), frame.rs (`er_phase` after
//! `hud(state)`), entry.rs (`shutdown`), lib.rs (the mod line).
//!
//! What persists across a respawn (a new `SoraHost`) is in statics here: the movies, the rasterisers, Elden
//! Ring's HUD state as the game last wrote it, the pause we hold.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use glam::{Mat4, Vec2, Vec3, Vec4};
use kh_core::{Input, Output, PadButtons, PadState, Sora, TargetId, Targets, UiSnapshot};
use kh_host::hud::{Frame, Hud, HudTargets, TargetView, View};
use kh_host::hud_layers::{stage_root, LayerEvent, Layers};
use kh_host::menu::MenuInput;
use kh_ui::gameover::GameOverButton;

use crate::control::{Control, OneShot};
use crate::erlog;
use crate::inputs::{self, ErUi};
use crate::targets::ErTargets;

// ---------------------------------------------------------------------------------- what the gameplay side asks

/// A menu of ours holds the pause (camp menu, Pause screen): Sora gets no steps and no input.
static MENU_OPEN: AtomicBool = AtomicBool::new(false);
/// (HUD2 round 2) KH3's OBTAINED popup was up in the HUD's last frame.
static OBTAINED_UP: AtomicBool = AtomicBool::new(false);
/// Sora is dead and the game-over menu has not been answered: the Tarnished must stay alive.
static GAMEOVER_HOLD: AtomicBool = AtomicBool::new(false);
/// The game-over menu is on screen (the pad is its).
static GAMEOVER_MENU: AtomicBool = AtomicBool::new(false);
/// Milliseconds (since `START`) of the latest `HudHost::frame`.
static FRAME_AT: AtomicU64 = AtomicU64::new(0);
static START: Mutex<Option<Instant>> = Mutex::new(None);
/// The Sora pack's folder (its `ui` folder is read when the HUD is first switched on).
static PACK: Mutex<Option<std::path::PathBuf>> = Mutex::new(None);
/// Our threads that must have ended before the payload is unloaded: the pause watchdog, the pack reader.
static THREADS: Mutex<Vec<std::thread::JoinHandle<()>>> = Mutex::new(Vec::new());
static STOP: AtomicBool = AtomicBool::new(false);

fn now_ms() -> u64 {
    let mut start = START.lock().unwrap_or_else(|e| e.into_inner());
    start.get_or_insert_with(Instant::now).elapsed().as_millis() as u64 + 1
}

/// (IMPL_ER14, testing aid) `dbg.saveload = save | load`.
static SCREEN_SHOT: Mutex<Option<OneShot>> = Mutex::new(None);

/// (savegate) Words for the HUD from other parts of the host (`item Game_saved.`: the item plate with that text).
static WORDS: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// (savegate) A HUD word (as `hud.cmd` takes them) for the next HUD frame.
pub fn say(word: &str) {
    if let Ok(mut words) = WORDS.lock() {
        if words.len() < 16 {
            words.push(word.to_string());
        }
    }
}

/// (IMPL_ER10) A cutscene plays and Start belongs to KH3's cutscene pause menu (`cutscene.menu`); that menu is up.
static CUTSCENE_MENU_ON: AtomicBool = AtomicBool::new(false);
static CUTSCENE_PAUSE: AtomicBool = AtomicBool::new(false);

/// What Elden Ring must not see of the pad in a cutscene: Start (ours: the cutscene pause menu), and every
/// button while that menu is up. 0 = the whole pad is the game's.
pub fn cutscene_mask() -> u32 {
    if CUTSCENE_PAUSE.load(Ordering::Relaxed) {
        u32::from(inputs::XI_GAMEPLAY | inputs::XI_START | inputs::XI_BACK) | inputs::MASK_LEFT_STICK
    } else if CUTSCENE_MENU_ON.load(Ordering::Relaxed) {
        u32::from(inputs::XI_START)
    } else {
        0
    }
}

/// (IMPL_ER10) A real-time cutscene plays (set by the HUD frame).
static CUTSCENE: AtomicBool = AtomicBool::new(false);

// ---------------------------------------------------------------------------------- (IMPL_ER14) KH3's Save / Load screens
/// `saveload.kh = 1`: the save point's Save opens KH3's Save screen (ten slots), the game-over menu's Load its Load screen.
static SAVELOAD_ON: AtomicBool = AtomicBool::new(false);
/// The save point's Save was chosen in this frame: the Save screen opens in the HUD's frame (savepoint.rs asks).
static WANT_SAVE_SCREEN: AtomicBool = AtomicBool::new(false);
/// What the screens are waiting for: a save into a slot / a load of a slot, since when.
static SCREEN_JOB: Mutex<Option<(ScreenJob, Instant)>> = Mutex::new(None);
/// What a screen asked for in the HUD's frame, carried out at the start of the next one OUTSIDE the HUD's lock (the
/// save side asks the HUD for its stock and clock: under the lock that is a deadlock, MEASURED: the game hung), and
/// what came of asking.
static SCREEN_ASK: Mutex<Option<ScreenJob>> = Mutex::new(None);
static SCREEN_ASKED: Mutex<Option<(ScreenJob, Result<(), &'static str>)>> = Mutex::new(None);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScreenJob {
    Save(usize),
    Load(usize),
    /// (SV) KH3's Delete on a slot row: the slot's data goes, nothing is saved (slots.rs `delete_slot`)
    Delete(usize),
}

/// (SV) The save point's Load was chosen in this frame: the Load screen opens in the HUD's frame (savepoint.rs asks).
static WANT_LOAD_SCREEN: AtomicBool = AtomicBool::new(false);

/// (SV, savepoint.rs) The save-point menu's Load was chosen: KH3's Load screen is to open in this frame's HUD frame.
pub fn ask_load_screen() {
    WANT_LOAD_SCREEN.store(true, Ordering::Relaxed);
}

/// (SV) The screens' switches in play: KH3's Delete (`saveload.delete`, default on), the running game's own slot refused,
/// the load question (`saveload.confirm_load`, default on: the user asked for a confirmation).
fn play_screen_options(hud: &mut Hud, control: &Control) {
    hud.set_saveload_options(control.flag("saveload.delete") != Some(false), false, control.flag("saveload.confirm_load") != Some(false));
}

/// The screens are switched on (`saveload.kh`, with `hud.kh`) and the pack can show them.
pub fn saveload_on() -> bool {
    SAVELOAD_ON.load(Ordering::Relaxed)
}

/// (savepoint.rs) The save-point menu's Save was chosen: KH3's Save screen is to open in this frame's HUD frame.
pub fn ask_save_screen() {
    WANT_SAVE_SCREEN.store(true, Ordering::Relaxed);
}

/// A save started from the Save screen is under way: the world must run for it (no pause of ours meanwhile).
fn screen_saving() -> bool {
    SCREEN_JOB.lock().is_ok_and(|j| matches!(*j, Some((ScreenJob::Save(_), _))))
}

/// THE TEN SLOTS AS KH3's SCREENS SHOW THEM (slots.rs: the game's profile summary + Sora's own file per slot): Elden
/// Ring's "in use" decides; Sora's level and munny from his file (a character that never was Sora: level 1, munny 0);
/// the game's play time and place name; the mode field says when a slot saves at save points only.
/// (SP2, the user 2026-10-20: "Also for the world picture in the save screen, just use the Elden Ring logo") THE WORLD
/// PICTURE OF EVERY USED SLOT ROW (save, load, delete): the save-load pack's world "er" = Elden Ring's own title logo
/// (MENU_Title_EldenRing_01 of the user's install, build/sora_tmp/party/SP2/pack/saveload); a pack without it shows KH3's
/// "no world" picture as before.
pub const ER_WORLD: &str = "er";

pub fn ui_slots() -> Vec<kh_ui::saveload::SlotSummary> {
    use kh_ui::saveload::{SlotMode, SlotState, SlotSummary};
    crate::slots::slot_summaries()
        .into_iter()
        .map(|s| {
            if !s.used {
                return SlotSummary::empty(s.slot);
            }
            SlotSummary { index: s.slot, state: SlotState::Used, name: s.name.clone(), level: s.sora.as_ref().map_or(1, |k| k.level), munny: s.sora.as_ref().map_or(0, |k| k.munny.max(0) as u32), play_time_seconds: u64::from(s.play_seconds), location: s.location.clone(), world: Some(ER_WORLD.to_string()) /* (SP2) the Elden Ring logo */, mode: SlotMode::None, clear: false, is_current: s.current, portrait: slot_portrait(&s) }
        })
        .collect()
}
/// (ER party S6) THE SAVE SLOT'S PICTURE: the party leader who made the save (the user: "your save file should show the
/// picture of the party leader that made the save file"; the slot file's party record, kh_host `slot_summary_of`); an
/// old save without the record, or the party off (feature / `party.er = 0`), shows Sora as before.
fn slot_portrait(s: &crate::slots::SlotSummary) -> kh_ui::saveload::Portrait {
    #[cfg(feature = "party")]
    if crate::partyui::key_on() {
        if let Some(p) = crate::save::peek_portrait(&s.name, s.slot as i32) {
            return p;
        }
    }
    let _ = s;
    kh_ui::saveload::Portrait::Sora
}

// ---------------------------------------------------------------------------------- (IMPL_ER15) KH3's game-over scene
/// The overlay layer the sequence's plate lies under: "GameOver" (index 9 of `LAYERS`): over the cockpit, under the menu.
const GAMEOVER_LAYER: usize = 9;
/// (IMPL_ER22) The same place for the boss finish's white plate (finish.rs): over the world and the cockpit.
pub const PLATE_LAYER: usize = GAMEOVER_LAYER;

/// (IMPL_ER22) The payload is stopping: threads of other modules end themselves.
pub fn stopping() -> bool {
    STOP.load(Ordering::Relaxed)
}

/// (IMPL_ER22) A thread of another module that must be waited for when the payload stops.
pub fn adopt_thread(handle: std::thread::JoinHandle<()>) {
    THREADS.lock().unwrap_or_else(|e| e.into_inner()).push(handle);
}

/// (IMPL_ER22) The menus' old pause route holds `CSFlipper::game_speed` right now (`menu.pause = speed`).
pub fn speed_held() -> bool {
    PAUSE_FIELD.load(Ordering::SeqCst) != 0
}
/// The sequence's choice was carried out (a load, Elden Ring's own respawn): since when, and whether the camp menu is
/// to open when he stands again (Prepare & Retry). The black plate stays until the NEW Sora has the picture.
static CARRIED: Mutex<Option<(Instant, bool)>> = Mutex::new(None);
/// Prepare & Retry: the camp menu opens by itself as soon as the new Sora stands (until this moment at the latest).
static PREPARE: Mutex<Option<Instant>> = Mutex::new(None);
/// Retry = Elden Ring's own respawn: this death is handed to the game on purpose (no load of the last save after it).
static RETRY_RESPAWN: AtomicBool = AtomicBool::new(false);
/// A load the sequence asked for (Continue), carried out at the start of the next HUD frame outside the HUD's lock.
static GO_LOAD: Mutex<Option<usize>> = Mutex::new(None);
static HINT_NO: AtomicUsize = AtomicUsize::new(0);
/// The plate is ours to take off.
static PLATE_ON: AtomicBool = AtomicBool::new(false);

/// (IMPL_ER15) An in-session load was asked for: what the old session's Sora had open in the shared HUD must not meet
/// the loaded one (MEASURED: slot 9's new-game choice menu stayed up over slot 0's Sora after a load: answering it
/// would have rebuilt the WRONG character's start).
static SESSION_LOAD: AtomicBool = AtomicBool::new(false);

/// (savegate.rs) Every in-session load says so here (no lock is taken: the next HUD frame acts on it).
pub fn on_session_load() {
    SESSION_LOAD.store(true, Ordering::Relaxed);
}

/// (frame.rs) The death the game is carrying out is the game-over menu's Retry (Elden Ring's own respawn, the world
/// kept): once.
pub fn take_retry_respawn() -> bool {
    RETRY_RESPAWN.swap(false, Ordering::Relaxed)
}

fn carried() -> Option<(Instant, bool)> {
    CARRIED.lock().ok().and_then(|c| *c)
}

/// (IMPL_ER10) THE SCRIPTED DEATH (sheet rows death.scripted.*: the tutorial boss): Sora's death goes straight to
/// Elden Ring's own death there: no game-over menu, no hold, no load of the last save.
static SCRIPTED: AtomicBool = AtomicBool::new(false);

/// (ZFIN round 7) Zettaflare's cinematic has the view (party/team.rs, from kh-partyhost's runtime).
pub fn zetta_cinematic() -> bool {
    #[cfg(feature = "party")]
    {
        crate::party::team::zetta_cinematic()
    }
    #[cfg(not(feature = "party"))]
    {
        false
    }
}

/// (IMPL_ER11) A real-time cutscene plays (as of the latest HUD frame).
pub fn in_cutscene() -> bool {
    CUTSCENE.load(Ordering::Relaxed) && hud_live()
}

pub fn set_scripted_death(on: bool) {
    if SCRIPTED.swap(on, Ordering::Relaxed) != on {
        erlog!("game over: {}", if on { "THE SCRIPTED DEATH'S PLACE (the tutorial boss is here): a death of Sora's goes straight to Elden Ring's own death, without the game-over menu" } else { "the scripted death's place is left: deaths show the game-over menu again" });
    }
}

pub fn scripted_death() -> bool {
    SCRIPTED.load(Ordering::Relaxed)
}

/// A menu of ours has the input and holds the pause.
pub fn menu_open() -> bool {
    MENU_OPEN.load(Ordering::Relaxed)
}

/// (ER party S8) THE HUD FOR THE UNLOCK SCENES (unlock/host.rs: the talk window, the banner, the fanfare): `f` runs under
/// the HUD's lock on its kh_host Hud (plain calls only, never a game call). None: no HUD yet.
/// (CV1) not behind the `party` feature any more: the conversation system (conv.rs) uses it in every build.
pub fn with_hud<R>(f: impl FnOnce(&mut kh_host::hud::Hud) -> R) -> Option<R> {
    let mut guard = SHARED.lock().ok()?;
    let hud = guard.as_mut()?.hud.as_mut()?;
    Some(f(hud))
}

/// Sora died and the KH3 game-over menu is not answered yet: the Tarnished's HP is NOT set to 0, and he
/// cannot be hit.
pub fn gameover_holds() -> bool {
    GAMEOVER_HOLD.load(Ordering::Relaxed)
}

/// What Elden Ring must additionally not see of the pad while a menu of ours is up: Start, View (its map
/// would open under our menu), the right stick.
pub fn extra_mask() -> u32 {
    if menu_open() || GAMEOVER_MENU.load(Ordering::Relaxed) || crate::conv::holds_pad() /* (CV1) a conversation: no map / menu of the game's under it */ {
        u32::from(inputs::XI_START | inputs::XI_BACK) | inputs::MASK_RIGHT_STICK
    } else {
        0
    }
}

// ---------------------------------------------------------------------------------- (ERUI) borrowed interactions
/// (ERUI) THE PAD DURING A BORROWED INTERACTION OF ELDEN RING'S (the user, 2026-10-23: "pausing during going through a
/// fog gate opens the elden ring menu, make it open the pause menu used during boss battles instead, make this the case
/// for any borrowed elden ring interaction"). While the body performs the game's own action (sora.rs `Follow::Action`:
/// fog gates, doors, ladders, levers, elevators, the own.moves set; `own_hold`: the own.anims in place) the whole pad
/// was the game's, Start included: its menu opened. Now Start is OURS and opens KH3's PAUSE SCREEN (the battle pause:
/// kh_host `set_host_event`, KH3's own gate `in_event` picks Pause over the camp menu), the game never sees Start or
/// View (its menu, its map), and while our Pause is up it sees nothing at all: the world stands under it through the
/// same pause of ours a battle pause uses (pause.rs, `menu.pause`), and the action goes on where it stood when the
/// Pause closes. A real menu / window of the game's (a dialog, a standing world of its, a cutscene) keeps the pad as
/// before. `in.action_pause = 0`: as before (the game's pad, its menu).
static BORROWED: AtomicBool = AtomicBool::new(false);

/// (ERUI) What the routing gives: whether it applies, the PadButtons bits our HUD gets, whether it gets the
/// stick / D-pad navigation, and what Elden Ring must not see.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BorrowedPad {
    pub on: bool,
    pub hud_buttons: u32,
    pub hud_nav: bool,
    pub er_mask: u32,
}

/// (ERUI) `borrowed`: a borrowed interaction runs (and the rule is on); `er_menu`: a menu / window of the game's OWN is
/// up besides it; `our_menu`: a menu of ours (the Pause screen) is up.
pub fn borrowed_pad(borrowed: bool, er_menu: bool, our_menu: bool) -> BorrowedPad {
    const START: u32 = 1 << kh_core::PadButton::SpecialRight as u32;
    if !borrowed || er_menu {
        return BorrowedPad { on: false, hud_buttons: u32::MAX, hud_nav: true, er_mask: 0 };
    }
    if our_menu {
        return BorrowedPad { on: true, hud_buttons: u32::MAX, hud_nav: true, er_mask: 0xFFFF | inputs::MASK_LEFT_STICK | inputs::MASK_TRIGGERS | inputs::MASK_RIGHT_STICK };
    }
    BorrowedPad { on: true, hud_buttons: START, hud_nav: false, er_mask: u32::from(inputs::XI_START | inputs::XI_BACK) }
}

/// (ERUI) sora.rs tells the routing every frame.
pub fn set_borrowed(on: bool) {
    if BORROWED.swap(on, Ordering::Relaxed) != on {
        erlog!("hud: {}", if on { "A BORROWED INTERACTION OF ELDEN RING'S RUNS: Start is ours (KH3's Pause screen, as in a battle), Elden Ring sees no Start / View (no menu, no map of its)" } else { "the borrowed interaction is over: the pad as before" });
    }
}

pub fn borrowed() -> bool {
    BORROWED.load(Ordering::Relaxed)
}

#[cfg(test)]
mod erui_borrowed_tests {
    use super::*;

    /// (ERUI) The routing of a borrowed interaction: Start to our HUD (Pause screen), never Start / View to the game;
    /// with our Pause up the whole pad is ours; a window of the game's own keeps its pad; off = as before.
    #[test]
    fn erui_start_during_a_borrowed_interaction_is_ours() {
        let start = 1u32 << kh_core::PadButton::SpecialRight as u32;
        let p = borrowed_pad(true, false, false);
        assert!(p.on);
        assert_eq!(p.hud_buttons, start, "only Start reaches our HUD (no A / D-pad into the command menu)");
        assert!(!p.hud_nav);
        assert_ne!(p.er_mask & u32::from(inputs::XI_START), 0, "Elden Ring never sees Start: its menu cannot open");
        assert_ne!(p.er_mask & u32::from(inputs::XI_BACK), 0, "nor View: its map cannot open");
        assert_eq!(p.er_mask & u32::from(inputs::XI_GAMEPLAY), 0, "the action keeps the rest of the pad (a ladder's stick, its buttons)");
        assert_eq!(p.er_mask & inputs::MASK_LEFT_STICK, 0);
        let paused = borrowed_pad(true, false, true);
        assert!(paused.on && paused.hud_nav && paused.hud_buttons & start != 0, "our Pause is up: it gets the pad (Start / B close it)");
        assert_eq!(paused.er_mask & 0xFFFF, 0xFFFF, "the game sees no button under our Pause");
        assert_ne!(paused.er_mask & inputs::MASK_LEFT_STICK, 0);
        for (b, e, o) in [(false, false, false), (false, true, true), (true, true, false), (true, true, true)] {
            let q = borrowed_pad(b, e, o);
            assert_eq!(q, BorrowedPad { on: false, hud_buttons: u32::MAX, hud_nav: true, er_mask: 0 }, "({b}, {e}, {o}): as before");
        }
    }

    /// (ERUI) KH3's own selector: an event of the host's = the Pause screen, as a battle (never the camp menu).
    #[test]
    fn erui_a_host_event_selects_the_pause_screen() {
        use kh_ui::menu::{select_screen, GateInput, Screen};
        assert_eq!(select_screen(&GateInput::default()), Screen::Camp);
        assert_eq!(select_screen(&GateInput { in_event: true, ..GateInput::default() }), Screen::Pause(0));
        assert_eq!(select_screen(&GateInput { in_battle: true, ..GateInput::default() }), Screen::Pause(0), "the boss battle's pause: the same screen");
    }
}

// ---------------------------------------------------------------------------------- Elden Ring's own HUD

/// ELDEN RING'S HUD STATE under `hud.er = 0`. FINDINGS (SDK `CSFeManImp`, fork 79266c9; IMPL_ER2 / ER4):
/// the one switch is `hud_state` (0 HideAll, 1 ShowAll = its main menu, 2 PopupMenu, 3 Default): ALL OR
/// NOTHING for the status bars, the equipment slots, the compass (seen gone with HideAll, IMPL_ER2); what it
/// does to the interaction prompts, boss bars, banners and item popups was NOT looked at in the game. No
/// per-element switch was found: `CSMenuManImp::ui_states` are the game's own created / visible bits (read
/// by us for the prompt), `FrontEndViewValues::enable_equip_hud` is the one selective field (the equipment
/// slots; not used here: untested). The game writes `hud_state` when ITS state changes, so the rule here:
/// we write HideAll only over Default (3), remember that it was ours, and take any other value we read as
/// the game's (its menu opened: left alone until it writes Default again). What cannot be seen that way is a
/// change of the game's to HideAll itself (map, grace, cutscene: measured 0) while we already hold 0: for
/// those the visible interface elements of `hud.menu_ui` (default 28 main menu, 61 map, 65 popup) say
/// "a menu has the input". A GRACE MENU OR CUTSCENE WHOSE ELEMENT IS NOT IN THAT LIST IS NOT SEEN: that is why
/// the default is to leave Elden Ring's HUD alone.
struct ErHud {
    /// The byte holds our HideAll (written over the game's Default).
    ours: bool,
    /// The value the game last wrote.
    game: u8,
}

static ER_HUD: Mutex<ErHud> = Mutex::new(ErHud { ours: false, game: 3 });

fn hud_state_byte() -> Option<&'static mut u8> {
    use eldenring::cs::CSFeManImp;
    use fromsoftware_shared::FromStatic;
    // SAFETY: game thread (a task phase); the singleton lives as long as the game; a one-byte field.
    let fe = unsafe { CSFeManImp::instance_mut() }.ok()?;
    Some(unsafe { &mut *(std::ptr::addr_of_mut!(fe.hud_state) as *mut u8) })
}

/// Our HUD is up right now (frames are coming).
fn hud_live() -> bool {
    let at = FRAME_AT.load(Ordering::Relaxed);
    at != 0 && now_ms().saturating_sub(at) < 300
}

fn er_hud_phase(control: &Control) {
    let Ok(mut st) = ER_HUD.lock() else { return };
    // (IMPL_ER8: `hud.er = 0` hides Elden Ring's HUD PART BY PART (erhud.rs); `off` = the game's own option, `er_option_phase`;
    // this all-or-nothing route is `hud.er = hideall`)
    let mode = er_mode(control);
    if let Ok(mut key) = OWN_KEY.lock() {
        if let Some(v) = key.get_or_insert_with(|| OneShot::new("hud.er.option", control)).fired(control).and_then(|v| v.trim().parse::<u8>().ok()).filter(|v| *v <= 2) {
            OWN_ASKED.store(v, Ordering::Relaxed);
        }
    }
    let up = hud_live() && SHARED.lock().is_ok_and(|s| s.as_ref().is_some_and(|s| s.hud.is_some()));
    er_option_phase(mode, up);
    if mode == ErMode::Select && up {
        // (IMPL_ER10) in a real-time cutscene the compass goes too (group 12)
        crate::erhud::set(hide_mask(control) | if (CUTSCENE.load(Ordering::Relaxed) || gameover_holds() /* (IMPL_ER15) nothing of its HUD while he dies */) && FRAME_AT.load(Ordering::Relaxed) + 300 > now_ms() { 1 << 12 } else { 0 });
        // (IMPL_ER13) ... and the same parts stay hidden when Sora's frames stop (his death, the respawn, a load)
        crate::erhud::stick(hide_mask(control));
    } else {
        crate::erhud::clear();
        if mode != ErMode::Select || control.str("mode") != Some("sora") {
            crate::erhud::unstick();
        }
    }
    let asked = mode == ErMode::HideAll;
    if !asked && !st.ours {
        return;
    }
    let Some(byte) = hud_state_byte() else { return };
    let v = *byte;
    if v != 0 {
        if st.ours {
            erlog!("hud: Elden Ring wrote its hud_state {v} over our HideAll{}", if v == 3 { "" } else { ": a menu of its is up, its HUD is left alone until it is back at Default" });
        }
        st.game = v;
        st.ours = false;
    } else if !st.ours {
        st.game = 0;
    }
    let want = asked && up;
    if want {
        if v == 3 {
            *byte = 0;
            if !st.ours {
                erlog!("hud: Elden Ring's own HUD is hidden while Sora's is up (hud.er = 0; its hud_state was Default)");
            }
            st.ours = true;
        }
    } else if st.ours {
        if *byte == 0 {
            *byte = 3;
        }
        st.ours = false;
        st.game = 3;
        erlog!("hud: Elden Ring's own HUD is given back");
    }
}

/// ELDEN RING'S HUD THROUGH ITS OWN OPTION (`hud.er = 0`; IMPL_ER7_STATE.md): the game's setting "HUD: Off"
/// (`GameDataMan::game_settings.hud_type`, the SDK's field) is written IN MEMORY while Sora's HUD is up and put
/// back when it goes, at shutdown, and at the next start if a run ended without putting it back (the marker file
/// `erlogser-hud-type.txt` holds the user's value meanwhile). It is the game's own selective switch: what its
/// "HUD: Off" hides and keeps is the game's rule, and `hud_state` is not touched, so the detection of its menus,
/// map, graces and cutscenes stays what it was.
static ER_OPTION: Mutex<(Option<(u8, u8)>, Option<std::path::PathBuf>)> = Mutex::new((None, None));

/// WHAT hud.er ASKS FOR while Sora's HUD is up (IMPL_ER8_STATE.md):
/// absent / 0 (the default) = PART BY PART (erhud.rs): Elden Ring's compass, prompts, banners, popups and boss bars
/// stay, its HP / FP / stamina bars, equipment slots, rune counter and enemy tags go (hud.er.hide = groups overrides
/// which); off = the game's own option "HUD: Off" (the IMPL_ER7 route: also hides the compass and the boss bars);
/// hideall = the hud_state route; 1 = Elden Ring's whole HUD stays.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ErMode {
    All,
    Select,
    OptionOff,
    HideAll,
}

fn er_mode(control: &Control) -> ErMode {
    if !control.on("hud.kh") || control.flag("hud.hide") == Some(true) {
        return ErMode::All;
    }
    match control.str("hud.er").map(str::trim) {
        // (without the detour the nearest thing is the game's own "HUD: Off")
        Some("0") | Some("select") => {
            if crate::erhud::hooked() {
                ErMode::Select
            } else {
                ErMode::OptionOff
            }
        }
        Some("off") => ErMode::OptionOff,
        Some("hideall") => ErMode::HideAll,
        _ => ErMode::All,
    }
}

/// The groups hidden under ErMode::Select (erhud.rs group numbers; MEASURED in the game, IMPL_ER8_STATE.md):
/// 0 the frame left of the bars, 1 HP, 2 FP, 3 stamina, 4 (unseen: by its place in the values the status-effect icons),
/// 5 the spell slot and its name, 6 the skill name, 7 - 10 the other equipment slots, 11 the rune counter, 13 (unseen:
/// an icon entry beside the frame's); tags = the HP tags over enemies. NOT hidden: 12 = THE COMPASS.
/// (IMPL_ER23A, the user: "also remove the elden ring compass") 12 = the compass is hidden too now: erhud.rs `DEFAULT_HIDE`.
const DEFAULT_HIDE: &str = crate::erhud::DEFAULT_HIDE;
/// hud.er.option = 0 | 1 | 2 (one-shot): the game's HUD option is set to this AS THE USER'S OWN VALUE (the IMPL_ER7
/// build left Off in the mod save; this puts On back without a walk through Elden Ring's menu). 255 = nothing asked.
static OWN_ASKED: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(255);
static OWN_KEY: Mutex<Option<OneShot>> = Mutex::new(None);

fn hide_mask(control: &Control) -> u32 {
    let text = control.str("hud.er.hide").unwrap_or(DEFAULT_HIDE);
    crate::erhud::parse_mask(text).or_else(|| crate::erhud::parse_mask(DEFAULT_HIDE)).unwrap_or(0)
}

/// Where the marker file goes (frame.rs, once).
pub fn set_root(root: &std::path::Path) {
    if let Ok(mut st) = ER_OPTION.lock() {
        st.1 = Some(crate::log::dir().join("er-hud-type.txt"));
    }
    // (IMPL_ER12) the title mode needs the pack before any Sora exists
    if let Ok(mut pack) = PACK.lock() {
        if pack.is_none() {
            *pack = Some(crate::sora::data_paths(root).0);
            crate::erconfig::set_sheet(crate::sora::data_paths(root).1.with_file_name("sora-er-config.json")); // (IMPL_ER21)
        }
    }
}

fn hud_type_byte() -> Option<&'static mut u8> {
    use eldenring::cs::GameDataMan;
    use fromsoftware_shared::FromStatic;
    // SAFETY: game thread (a task phase); the singleton lives as long as the game; a one-byte field; the pointer is checked.
    let man = unsafe { GameDataMan::instance_mut() }.ok()?;
    if !crate::game::plausible(crate::word!(man.game_settings)) {
        return None;
    }
    Some(unsafe { &mut *(std::ptr::addr_of_mut!(man.game_settings.hud_type) as *mut u8) })
}

/// The marker file: "own,ours" (the user's value of the option, the value we wrote over it); a file of the
/// IMPL_ER7 build holds the user's value alone (ours was Off then).
fn read_marker(path: &std::path::Path) -> Option<(u8, u8)> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut parts = text.trim().split(',').map(|p| p.trim().parse::<u8>().ok().filter(|v| *v <= 2));
    let own = parts.next()??;
    Some((own, parts.next().flatten().unwrap_or(0)))
}

/// THE GAME'S OWN HUD OPTION, written IN MEMORY while Sora's HUD is up and put back when it goes, at shutdown, and
/// at the next start if a run ended without putting it back (the marker file holds the user's value meanwhile):
/// "hud.er = off" needs it Off; the part-by-part mode needs it NOT Off (an option left Off, by the user or by an
/// earlier build of ours, is held at On meanwhile: with Off the game hides everything itself, boss bars included).
fn er_option_phase(mode: ErMode, want: bool) {
    let Ok(mut st) = ER_OPTION.lock() else { return };
    let marker = st.1.clone();
    // (the value this mode needs written over the user's own, if any)
    let needed = |own: u8| match mode {
        ErMode::OptionOff if want && own != 0 => Some(0u8),
        ErMode::Select if want && own == 0 => Some(1u8),
        _ => None,
    };
    let stale = if st.0.is_none() { marker.as_ref().filter(|p| p.exists()).and_then(|p| read_marker(p)) } else { None };
    if st.0.is_none() && stale.is_none() && !(want && matches!(mode, ErMode::OptionOff | ErMode::Select)) && OWN_ASKED.load(Ordering::Relaxed) > 2 {
        return;
    }
    let Some(byte) = hud_type_byte() else { return };
    let asked_own = OWN_ASKED.swap(255, Ordering::Relaxed);
    if asked_own <= 2 {
        erlog!("hud: Elden Ring's HUD option is set to {asked_own} as the user's own value (hud.er.option; it read {})", *byte);
        *byte = asked_own;
        if let Some((_, ours)) = st.0 {
            st.0 = Some((asked_own, ours));
            *byte = ours;
        }
    }
    // A run that ended without giving the option back (the process was ended): what is in memory came from the
    // save with OUR value in it; the user's value returns first.
    if let Some((own, ours)) = stale {
        if *byte == ours && own != ours {
            *byte = own;
            erlog!("hud: Elden Ring's HUD option put back to {own} (an earlier run had left it at {ours})");
        }
        if let Some(path) = marker.as_ref() {
            let _ = std::fs::remove_file(path);
        }
    }
    match st.0 {
        None => {
            let own = *byte;
            if let Some(ours) = needed(own) {
                st.0 = Some((own, ours));
                if let Some(path) = marker.as_ref() {
                    let _ = std::fs::write(path, format!("{own},{ours}"));
                }
                *byte = ours;
                erlog!("hud: Elden Ring's HUD option is held at {ours} in memory (it was {own}; 0 Off, 1 On, 2 Auto) while Sora's HUD is up ({mode:?})");
            }
        }
        Some((was_own, ours)) => {
            // (the user changed the option in Elden Ring's menu: that is the value to give back)
            let own = if *byte != ours { *byte } else { was_own };
            match needed(own) {
                Some(now) => {
                    if (own, now) != (was_own, ours) || *byte != now {
                        *byte = now;
                        st.0 = Some((own, now));
                        if let Some(path) = marker.as_ref() {
                            let _ = std::fs::write(path, format!("{own},{now}"));
                        }
                    }
                }
                None => {
                    if *byte == ours {
                        *byte = own;
                    }
                    st.0 = None;
                    if let Some(path) = marker.as_ref() {
                        let _ = std::fs::remove_file(path);
                    }
                    erlog!("hud: Elden Ring's HUD option is given back ({own})");
                }
            }
        }
    }
}

/// (hud) hook in sora.rs: Elden Ring's interface as the game means it, when the `hud_state` byte holds OUR
/// HideAll (it then reads 0 = "a menu", which would take the pad from Sora).
pub fn er_ui(raw: ErUi, control: &Control) -> ErUi {
    let Ok(st) = ER_HUD.lock() else { return raw };
    if !st.ours {
        return raw;
    }
    let listed = control.str("hud.menu_ui").unwrap_or("28,61,65");
    let menu = inputs::ui_states().is_some_and(|states| listed.split(',').filter_map(|k| k.trim().parse::<usize>().ok()).any(|k| states.get(k).is_some_and(|s| s & 2 != 0)));
    ErUi { hud_state: if menu { 0 } else { 3 }, menu, cutscene: raw.cutscene }
}

// ---------------------------------------------------------------------------------- the world's pause

/// THE PAUSE (`menu.pause = 1`; UNTESTED IN THE GAME). FINDINGS: the SDK has no pause function. Candidates:
/// (a) `CSFlipper::game_speed` ("game speed multiplier", the task delta is scaled by it): 0 while a menu of
/// ours is open, the value before put back when it closes: USED HERE; (b) the state a tutorial popup puts the
/// world in (IMPL_ER4: the player's animation clock stands still, our tasks keep running): no switch for it
/// is known; (c) the debug pause flags of `ChrDebugFlags` (`disabled_updates`) per character: would have to
/// be set on every character, and do not stop bullets or the clock.
/// RISK of (a): if the game's tasks stop coming at speed 0, nothing of ours would run to close the menu: a
/// watchdog thread puts the speed back when no HUD frame arrived for 1.5 s, and the pause stays off for the
/// rest of the run.
struct Pause {
    /// The speed before ours (`Some` while we hold it).
    before: Option<f32>,
    tripped: bool,
    watchdog: bool,
}

static PAUSE: Mutex<Pause> = Mutex::new(Pause { before: None, tripped: false, watchdog: false });
/// The address of `CSFlipper::game_speed` while we hold the pause (0 = not held), and the speed to put back.
static PAUSE_FIELD: AtomicUsize = AtomicUsize::new(0);
static PAUSE_RESTORE: AtomicU64 = AtomicU64::new(0);
static PAUSE_TRIPPED: AtomicBool = AtomicBool::new(false);
static OWN_TOLD: AtomicBool = AtomicBool::new(false);
/// (IMPL_ER11) A loading screen covers the game (the in-game step is not running): Sora's HUD is not drawn.
static LOADING: AtomicBool = AtomicBool::new(false);
static LOADING_UNTIL: AtomicU64 = AtomicU64::new(0);
/// (IMPL_ER11) The game-over menu was answered: the world runs again for what follows (the load, Elden Ring's death).
static GAMEOVER_CHOSEN: AtomicBool = AtomicBool::new(false);

fn pause_phase(control: &Control) {
    use eldenring::cs::CSFlipper;
    use fromsoftware_shared::FromStatic;
    let Ok(mut p) = PAUSE.lock() else { return };
    if PAUSE_TRIPPED.swap(false, Ordering::SeqCst) {
        p.tripped = true;
        p.before = None;
        erlog!("pause: THE WATCHDOG GAVE THE GAME ITS SPEED BACK (no HUD frame for 1.5 s while the world was held): menu.pause is off for the rest of this run");
    }
    // (IMPL_ER11) `menu.pause = 1` = THE GAME'S OWN PAUSE (pause.rs: the in-game step's "hold the world" byte, renewed
    // every phase of ours while a menu is open; it lapses by itself 400 ms after our last phase: its own watchdog);
    // `menu.pause = speed` = the old route below (CSFlipper::game_speed; it upsets the game's shadows and grass),
    // which is also the fallback when the step's update could not be detoured.
    // (the game-over hold too: the world stands from Sora's death until the menu is answered; his own death plays on)
    // (the cutscene pause menu holds nothing by default: MEASURED on the opening movie, the picture plays on while the
    // game's subtitles stop; `cutscene.pause = 1` holds the world under it all the same)
    let cutscene_only = CUTSCENE_PAUSE.load(Ordering::Relaxed) && !control.on("cutscene.pause");
    // (IMPL_ER14) ... and not while the Save screen's save is being written (the game's save task needs its world to run)
    let asked = !LOADING.load(Ordering::Relaxed) && !screen_saving() && !crate::slots::saving() && carried().is_none() /* (IMPL_ER15) a choice of the game-over menu is being carried out: the world runs for it */ && ((menu_open() && !cutscene_only) || (gameover_holds() && !GAMEOVER_CHOSEN.load(Ordering::Relaxed))) && hud_live();
    let key = control.str("menu.pause").map(str::trim).unwrap_or("0");
    let own = (control.on("menu.pause") || key == "hold") && crate::pause::available();
    crate::pause::use_hold_byte(key == "hold");
    if own && asked {
        crate::pause::hold();
    } else if OWN_TOLD.load(Ordering::Relaxed) {
        crate::pause::let_go();
    }
    if OWN_TOLD.swap(own && asked, Ordering::Relaxed) != (own && asked) {
        erlog!("pause: {}", if own && asked { "a Kingdom Hearts menu is open: THE WORLD IS HELD THE GAME'S OWN WAY (menu.pause = 1)" } else { "the world is let go" });
    }
    let want = asked && !own && (key == "speed" || control.on("menu.pause")) && !p.tripped;
    if !want && p.before.is_none() {
        return;
    }
    // SAFETY: game thread; the singleton lives as long as the game.
    let Ok(flipper) = (unsafe { CSFlipper::instance_mut() }) else { return };
    match (want, p.before) {
        (true, None) => {
            let before = flipper.game_speed;
            let speed = control.f32("menu.pause_speed").unwrap_or(0.0).clamp(0.0, 1.0);
            PAUSE_RESTORE.store(u64::from(before.to_bits()), Ordering::SeqCst);
            PAUSE_FIELD.store(std::ptr::addr_of_mut!(flipper.game_speed) as usize, Ordering::SeqCst);
            flipper.game_speed = speed;
            p.before = Some(before);
            erlog!("pause: Elden Ring's game speed {before} -> {speed} while the Kingdom Hearts menu is open (menu.pause = 1; UNTESTED mechanism)");
            if !p.watchdog {
                if let Ok(handle) = std::thread::Builder::new().name("kh-pause-watchdog".into()).spawn(watchdog) {
                    p.watchdog = true;
                    THREADS.lock().unwrap_or_else(|e| e.into_inner()).push(handle);
                }
            }
        }
        (false, Some(before)) => {
            PAUSE_FIELD.store(0, Ordering::SeqCst);
            flipper.game_speed = before;
            p.before = None;
            erlog!("pause: Elden Ring's game speed back to {before}");
        }
        _ => {}
    }
}

fn watchdog() {
    while !STOP.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(100));
        let field = PAUSE_FIELD.load(Ordering::SeqCst);
        if field == 0 {
            continue;
        }
        if now_ms().saturating_sub(FRAME_AT.load(Ordering::Relaxed)) > 1500 && PAUSE_FIELD.swap(0, Ordering::SeqCst) != 0 {
            // SAFETY: the field of a singleton that lives as long as the game; a plain f32 write.
            unsafe { (field as *mut f32).write_volatile(f32::from_bits(PAUSE_RESTORE.load(Ordering::SeqCst) as u32)) };
            PAUSE_TRIPPED.store(true, Ordering::SeqCst);
        }
    }
}

// ---------------------------------------------------------------------------------- the shared HUD

struct Shared {
    hud: Option<Hud>,
    layers: Option<Layers>,
    loading: bool,
    failed: bool,
    output: (u32, u32),
    shown: bool,
    /// The Config entry was chosen: Elden Ring's system menu is opened when our menu has closed (ms deadline).
    system_menu: Option<u64>,
    /// What the save side handed over before the movies were loaded (the camp menu's stock).
    inventory: Option<kh_host::hud::Inventory>,
    play_seconds: f64,
    // per-second numbers
    frames: u32,
    logic: f64,
    pictures: f64,
    worst: f64,
    told_at: u64,
    cmd: Option<OneShot>,
    options: kh_er_render::hud::HudOptions,
    /// Game over: the Continue was given (Elden Ring's death follows), until this moment the banner is fought.
    banner_until: u64,
    /// The whole overlay's opacity: eased to 0 while a full-screen menu of Elden Ring is up (IMPL_ER8).
    opacity: f32,
    /// (IMPL_ER11) The dark backdrop of the new-game choice menu is up.
    backdrop: bool,
}

/// The overlay layer of the choice menu's backdrop: the cutscene pause menu's (index 11 of `LAYERS`, drawn under the
/// choice menu's two layers 12 and 13; that menu cannot be open at the same time).
const BACKDROP_LAYER: usize = 11;

static SHARED: Mutex<Option<Shared>> = Mutex::new(None);
/// The loader thread's result.
static LOADED: Mutex<Option<Result<(Hud, kh_ui::Fonts), String>>> = Mutex::new(None);

// SAFETY: the HUD's movies hold `Arc`s of plain data and are only ever used under the mutex.
unsafe impl Send for Shared {}

fn log_line(line: &str) {
    erlog!("{line}");
}

/// The camp menu's stock for the save side (the loaded HUD's, else what was handed over).
pub fn inventory() -> kh_host::hud::Inventory {
    let Ok(guard) = SHARED.lock() else { return kh_host::hud::Inventory::default() };
    let Some(s) = guard.as_ref() else { return kh_host::hud::Inventory::default() };
    let from = s.hud.as_ref().map(|h| h.inventory()).or(s.inventory.as_ref());
    from.map_or_else(kh_host::hud::Inventory::default, |i| kh_host::hud::Inventory { equipment: i.equipment.clone(), spare_weapons: i.spare_weapons.clone(), records: i.records.clone() })
}

/// (IMPL_ER31, boss rewards) The once-only records of the running character (`boss:<row id>`; kept by Sora's save).
pub fn records() -> Vec<String> {
    inventory().records
}

pub fn set_records(records: Vec<String>) {
    let Ok(mut guard) = SHARED.lock() else { return };
    let s = guard.get_or_insert_with(Shared::new);
    match s.hud.as_mut() {
        Some(hud) => hud.inventory_mut().records = records,
        None => s.inventory.get_or_insert_with(kh_host::hud::Inventory::default).records = records,
    }
}

/// The save side's stock (a character was loaded).
pub fn set_inventory(inventory: kh_host::hud::Inventory) {
    let Ok(mut guard) = SHARED.lock() else { return };
    let s = guard.get_or_insert_with(Shared::new);
    match s.hud.as_mut() {
        Some(hud) => *hud.inventory_mut() = inventory,
        None => s.inventory = Some(inventory),
    }
}

/// (savepoint) Opens the KH3 save-point menu (savepoint.rs: Y at a discovered site of grace). False: the HUD is
/// not up, or another menu is.
pub fn open_savepoint(commands: &[kh_ui::savepoint::SavePointCommand], disabled: &[kh_ui::savepoint::SavePointCommand], texts: &[(kh_ui::savepoint::SavePointCommand, &str, &str)]) -> bool {
    let Ok(mut guard) = SHARED.lock() else { return false };
    // (no menu of ours on a loading screen)
    if LOADING.load(Ordering::Relaxed) {
        return false;
    }
    let Some(hud) = guard.as_mut().filter(|s| s.shown).and_then(|s| s.hud.as_mut()) else { return false };
    let opened = hud.open_savepoint_with(commands, disabled, texts);
    if opened {
        MENU_OPEN.store(true, Ordering::Relaxed);
    }
    opened
}

/// (savepoint, IMPL_ER26) Opens the save-point menu with the entries of `kh_host::savepoint_menu` (Save / Select an Area
/// / Pass Time / Respawn Enemies / Title Screen). False: the HUD is not up, or another menu is.
pub fn open_savepoint_grace(menu: kh_host::savepoint_menu::GraceMenu) -> bool {
    let Ok(mut guard) = SHARED.lock() else { return false };
    if LOADING.load(Ordering::Relaxed) {
        return false;
    }
    let Some(hud) = guard.as_mut().filter(|s| s.shown).and_then(|s| s.hud.as_mut()) else { return false };
    let opened = hud.open_savepoint_grace(menu);
    if opened {
        MENU_OPEN.store(true, Ordering::Relaxed);
    }
    opened
}

/// (savepoint, IMPL_ER26) The button of the menu's choice window (Pass Time: 0 Morning, 1 Noon, 2 Nightfall), once.
pub fn take_savepoint_choice() -> Option<usize> {
    SHARED.lock().ok()?.as_mut()?.hud.as_mut()?.take_savepoint_choice()
}

/// (DON-UI) Frames left to reopen the save-point menu after the Party Formation screen closed (see
/// [`reopen_after_formation`]).
static FORMATION_REOPEN: AtomicUsize = AtomicUsize::new(0);

/// (DON-UI) B ON THE PARTY FORMATION SCREEN LEADS BACK TO THE SAVE-POINT MENU (the user, 2026-10-22, for ALL menus: "when
/// you press B it should bring you back to the previous menu, not to the origin menu"; the test level does the same,
/// kh-app savepoint.rs): once the screen has closed, the menu `menu` opens again as soon as the HUD takes it (tried for
/// 240 frames). Called every frame by the save point (savepoint.rs).
pub fn reopen_after_formation(menu: kh_host::savepoint_menu::GraceMenu) {
    let closed = SHARED.lock().ok().and_then(|mut g| g.as_mut().and_then(|s| s.hud.as_mut()).map(|h| h.take_formation_closed())).unwrap_or(false);
    if closed {
        FORMATION_REOPEN.store(240, Ordering::Relaxed);
    }
    let left = FORMATION_REOPEN.load(Ordering::Relaxed);
    if left > 0 {
        let opened = open_savepoint_grace(menu);
        FORMATION_REOPEN.store(if opened { 0 } else { left - 1 }, Ordering::Relaxed);
        if opened {
            erlog!("savepoint: (DON-UI) back from the Party Formation screen (B): the save-point menu again");
        }
    }
}

/// (party P) The save point's Party Formation screen (kh_host::formation; the HUD's own formation). False: no HUD or
/// another menu is up.
pub fn open_party_formation() -> bool {
    let Ok(mut guard) = SHARED.lock() else { return false };
    let Some(hud) = guard.as_mut().filter(|s| s.shown).and_then(|s| s.hud.as_mut()) else { return false };
    let f = hud.formation().clone();
    let _ = hud.take_formation_change();
    hud.open_party_formation(&f)
}

/// (party P) The formation record for Sora's save (`Formation::to_save`), null without a HUD.
pub fn formation_record() -> serde_json::Value {
    SHARED.lock().ok().and_then(|g| g.as_ref().and_then(|s| s.hud.as_ref().map(|h| h.formation().to_save()))).unwrap_or(serde_json::Value::Null)
}

/// (party P) The formation of a loaded save (`Formation::from_save`: a missing record = the default).
pub fn set_formation_record(record: &serde_json::Value) {
    if let Ok(mut guard) = SHARED.lock() {
        if let Some(hud) = guard.as_mut().and_then(|s| s.hud.as_mut()) {
            hud.set_formation(&kh_host::formation::Formation::from_save(record));
        }
    }
}

/// (savepoint) What the save-point menu told the host, once.
pub fn take_savepoint_result() -> Option<kh_ui::savepoint::SavePointResult> {
    SHARED.lock().ok()?.as_mut()?.hud.as_mut()?.take_savepoint_result()
}

/// Seconds played as Sora in this session's character (the camp menu's clock; the save side sets the start).
pub fn play_seconds() -> f64 {
    SHARED.lock().ok().and_then(|g| g.as_ref().map(|s| s.play_seconds)).unwrap_or(0.0)
}

pub fn set_play_seconds(seconds: f64) {
    if let Ok(mut guard) = SHARED.lock() {
        guard.get_or_insert_with(Shared::new).play_seconds = seconds;
    }
}

impl Shared {
    fn new() -> Shared {
        Shared { hud: None, layers: None, loading: false, failed: false, output: (0, 0), shown: false, system_menu: None, inventory: None, play_seconds: 0.0, frames: 0, logic: 0.0, pictures: 0.0, worst: 0.0, told_at: 0, cmd: None, options: kh_er_render::hud::HudOptions::default(), banner_until: 0, opacity: 1.0, backdrop: false }
    }
}

/// Elden Ring's characters as the HUD's targets (the cursor's place, the enemy gauge's HP).
struct ErHudTargets<'a>(&'a ErTargets);

impl HudTargets for ErHudTargets<'_> {
    fn target(&self, id: TargetId) -> Option<TargetView> {
        // (HUD2) THE GAME'S OWN HP (`Enemy::hp_game`), never the expected HP a bullet in flight leaves in the row until
        // the game applies it: KH3's gauge reads the pawn's real HP, and the guess made the green bar jump (down to the
        // guess, back up at the next read, down again when the bullet landed). A map object (no row) keeps its own value.
        self.0.info(id).map(|i| {
            let hp = self.0.get(id).map_or(i.hp, |e| e.hp_game as f32);
            TargetView { center: i.position + Vec3::Z * i.half_height, hp: hp * self.0.params.gauge_hp_scale, hp_max: i.hp_max * self.0.params.gauge_hp_scale, large: i.radius >= 100.0 }
        })
    }

    /// (HUD2) The gauge reads the game's real HP, which still holds the HP before the hit when Sora's hit event comes
    /// (the bullet lands later): the gauge starts from what it reads, KH3's red lag then runs from there.
    fn hit_prev_hp(&self, _id: TargetId, _hp_after: f32, _damage_dealt: f32) -> Option<i32> {
        None
    }
}

fn same(p: Vec3) -> Vec3 {
    p
}

/// GAME (KH3 cm) -> clip for the HUD's projection pass (docs/sora-ui-spec.md 7.2): x, y as the game's camera
/// draws (its vertical field of view, its aspect), then scaled from the whole back buffer to the 16:9 stage
/// rectangle; `z / w` = 0.1 m over the view depth (the scale the test app's projection has: the marks' offset
/// rule reads it).
fn hud_clip(camera: &crate::game::Camera, origin: Vec3, output: (f32, f32), stage: (f32, f32)) -> Mat4 {
    use kh_er_render::math::{host_from_game, view_from_host, Camera};
    let cam = Camera { right: camera.right, up: camera.up, forward: camera.forward, position: camera.position, fov: camera.fov, aspect: camera.aspect, near: camera.near, far: camera.far };
    let f = 1.0 / (camera.fov * 0.5).tan();
    let projection = Mat4::from_cols(Vec4::new(f / camera.aspect.max(0.01) * output.0 / stage.0.max(1.0), 0.0, 0.0, 0.0), Vec4::new(0.0, f * output.1 / stage.1.max(1.0), 0.0, 0.0), Vec4::new(0.0, 0.0, 0.0, 1.0), Vec4::new(0.0, 0.0, 0.1, 0.0));
    projection * view_from_host(&cam) * host_from_game(origin)
}

/// The movies: read once, on a thread of their own. True when the HUD is there.
fn load_step(s: &mut Shared, control: &Control) -> bool {
    if s.hud.is_some() {
        return true;
    }
    if s.failed {
        return false;
    }
    if !s.loading {
        s.loading = true;
        let Some(dir) = PACK.lock().ok().and_then(|p| p.as_ref().map(|p| p.join("ui"))) else { return false };
        // (IMPL_ER23B) `hud.glyphs`: the pad set of the button icons (xboxone = default | windows = the generic set as before | ps4)
        if let Some(set) = control.str("hud.glyphs").map(str::trim).filter(|s| ["xboxone", "windows", "ps4"].contains(s)) {
            kh_ui::render::set_pad_glyphs(set);
        }
        // (IMPL_ER23B) `hud.exp_unit = TEXT`: the unit after an EXP popup's number (default: the game's own "p")
        kh_ui::hud_target::set_exp_unit(control.str("hud.exp_unit").map(str::trim).filter(|s| !s.is_empty() && s.len() <= 8));
        // (IMPL_ER23B) `hud.localise = 0`: the movies' text fields keep their authoring placeholders (as before)
        if control.flag("hud.localise") == Some(false) {
            kh_ui::def::set_localise(false);
        }
        erlog!("hud: reading the UI pack {} ... (button icons: the {} pad set)", dir.display(), kh_ui::render::pad_glyphs());
        STOP.store(false, Ordering::Relaxed);
        let spawned = std::thread::Builder::new().name("kh-hud-load".into()).spawn(move || {
            let result = std::panic::catch_unwind(|| Hud::load(&dir)).unwrap_or_else(|_| Err("panic while reading the UI pack".to_string()));
            *LOADED.lock().unwrap_or_else(|e| e.into_inner()) = Some(result);
        });
        match spawned {
            Ok(handle) => THREADS.lock().unwrap_or_else(|e| e.into_inner()).push(handle),
            Err(_) => s.failed = true,
        }
        return false;
    }
    match LOADED.lock().ok().and_then(|mut l| l.take()) {
        None => false,
        Some(Err(why)) => {
            erlog!("hud: the UI pack could not be read ({why}): Sora's HUD stays off");
            s.failed = true;
            false
        }
        Some(Ok((mut hud, fonts))) => {
            hud.config_external = true;
            hud.camp_hook = Some(kh_host::menu::pad_camp_hook);
            hud.log = log_line;
            // (Elden Ring has no KH save to load from this menu and no title to go to from here)
            hud.gameover_disabled = vec![GameOverButton::Load, GameOverButton::TitleScreen];
            if let Some(inventory) = s.inventory.take() {
                *hud.inventory_mut() = inventory;
            }
            erlog!("hud: Sora's HUD is ACTIVE: {}", hud.status_line());
            let mut layers = Layers::new(fonts, false);
            // (every picture on the worker thread: the game's thread only snapshots the movies)
            layers.first_worker_layer = 0;
            s.hud = Some(hud);
            s.layers = Some(layers);
            s.cmd = Some(OneShot::new("hud.cmd", control));
            true
        }
    }
}

// ---------------------------------------------------------------------------------- (IMPL_ER12) TITLE MODE

/// THE TITLE MODE: the HUD's menus with NO WORLD (no player, no camera, no in-game step, no Sora of the game's):
/// driven from FRAME_BEGIN by `newgame.rs`, the pad read through XInput directly (and the keyboard's menu keys), the
/// stage frames on real time, a stand-in Sora that is never stepped (the HUD's frame wants one to read gauges from).
struct TitleHud {
    menu_input: MenuInput,
    held_before: PadButtons,
    last: Option<Instant>,
    pending: f32,
    reader: crate::pad::Reader,
    sora: Sora,
    out: Output,
    snapshot: UiSnapshot,
    targets: ErTargets,
}

// SAFETY: used on the game's thread only, under the mutex.
unsafe impl Send for TitleHud {}

static TITLE: Mutex<Option<TitleHud>> = Mutex::new(None);
/// The title mode is drawing (the per-phase "Sora's frames stopped" rule must not clear it).
static TITLE_ON: AtomicBool = AtomicBool::new(false);

/// What a title-mode frame of the choice menu answers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TitleChoice {
    /// The UI pack is still being read.
    Loading,
    /// The pack cannot be read / has no choice menu: the caller goes on without asking.
    Unavailable,
    /// The menu is up.
    Asking,
    Chosen(kh_core::status::OpeningChoice),
}

/// (IMPL_ER12) Starts reading the UI pack if that has not happened (the title is up: be ready for New Game).
pub fn title_prepare(control: &Control) {
    if let Ok(mut guard) = SHARED.lock() {
        let s = guard.get_or_insert_with(Shared::new);
        let _ = load_step(s, control);
    }
}

/// (IMPL_ER12) ONE TITLE-MODE FRAME OF THE NEW-GAME CHOICE MENU (call once a frame from FRAME_BEGIN while it is wanted;
/// the caller draws the black behind it). Opens the menu when it is not up.
pub fn title_choice_frame(control: &Control) -> TitleChoice {
    let Ok(mut guard) = SHARED.lock() else { return TitleChoice::Unavailable };
    let s = guard.get_or_insert_with(Shared::new);
    if !load_step(s, control) {
        return if s.failed { TitleChoice::Unavailable } else { TitleChoice::Loading };
    }
    let Ok(mut title_guard) = TITLE.lock() else { return TitleChoice::Unavailable };
    let t = title_guard.get_or_insert_with(|| {
        let sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 });
        let out = sora.output();
        TitleHud { menu_input: MenuInput::new(), held_before: PadButtons::NONE, last: None, pending: 0.0, reader: crate::pad::Reader::default(), sora, out, snapshot: UiSnapshot::default(), targets: ErTargets::new(crate::coords::Frame::at(Vec3::ZERO), crate::ermap::Params::default()) }
    });
    let Some(output) = kh_er_render::hud::output_size().filter(|(w, h)| *w >= 64 && *h >= 64) else { return TitleChoice::Loading };
    let resized = output != s.output;
    s.output = output;
    let started = Instant::now();
    let dt = t.last.replace(started).map_or(1.0 / 60.0, |at| started.duration_since(at).as_secs_f32()).clamp(0.0, 0.25);
    // the pad (XInput directly) and the keyboard's menu keys
    let (mut stick, mut held) = inputs::keyboard();
    if let Some(pad) = t.reader.read() {
        stick += pad.left;
        held = PadButtons(held.0 | pad.buttons.0);
    }
    let pad = PadState::from_held(held, t.held_before);
    t.held_before = held;
    t.menu_input.step(&Input { move_stick: stick.clamp_length_max(1.0), ..Input::default() }.with_pad(pad), dt.max(0.001));
    t.pending += dt;
    let mut ticks = 0;
    while t.pending + 1.0e-5 >= 1.0 / 60.0 && ticks < 6 {
        t.pending -= 1.0 / 60.0;
        ticks += 1;
    }
    if ticks == 6 {
        t.pending = 0.0;
    }
    let (Some(hud), Some(layers)) = (s.hud.as_mut(), s.layers.as_mut()) else { return TitleChoice::Unavailable };
    if !hud.newgame_choice_available() {
        return TitleChoice::Unavailable;
    }
    let options = kh_er_render::hud::HudOptions { on: true, gamma: control.on("hud.gamma"), linear_scale: control.f32("hud.scrgb").unwrap_or(1.0), opacity: 1.0 };
    if options != s.options || !TITLE_ON.load(Ordering::Relaxed) {
        kh_er_render::hud::set_options(options);
        s.options = options;
    }
    TITLE_ON.store(true, Ordering::Relaxed);
    let mut answer = TitleChoice::Asking;
    if let Some(choice) = hud.take_newgame_choice() {
        answer = TitleChoice::Chosen(choice);
        #[cfg(feature = "party")]
        crate::unlock::chosen_starter(hud.take_newgame_starter()); // (ER party S8)
    } else if !hud.newgame_choice_open() {
        t.menu_input.clear();
        #[cfg(feature = "party")]
        hud.set_newgame_starter_question(crate::unlock::starter_question(control)); // (ER party S8) the character select first
        if hud.open_newgame_choice() {
            erlog!("newgame: THE OPENING'S CHOICES are asked ON THE TITLE'S BLACK (KH3's choice menu; no world is loaded)");
        } else {
            return TitleChoice::Unavailable;
        }
    }
    let (root, stage) = stage_root(output.0 as f32, output.1 as f32);
    let _ = stage;
    let (pressed, nav) = t.menu_input.take();
    hud.gameover_on = false;
    hud.set_cutscene(false);
    hud.frame(&mut Frame { snapshot: &t.snapshot, sora: &t.sora, events: &[], state: t.out.state, step_dt: 1.0 / 60.0, play_seconds: 0, targets: &ErHudTargets(&t.targets), to_view: same, view: None, words: &[], dt, ticks, pressed, nav });
    crate::fx::ui_sounds(hud.take_sounds().into_iter().map(|(command, _)| command).collect());
    if let Some(pack) = PACK.lock().ok().and_then(|p| p.clone()) {
        crate::fx::title_pump(&pack, dt, control.f32("snd.vol").unwrap_or(-6.0));
    }
    if answer == TitleChoice::Asking {
        if let Some(choice) = hud.take_newgame_choice() {
            answer = TitleChoice::Chosen(choice);
            #[cfg(feature = "party")]
            crate::unlock::chosen_starter(hud.take_newgame_starter()); // (ER party S8)
        }
    }
    layers.update(hud, &root, (output.0 as f32, output.1 as f32), resized, &mut |index, event| match event {
        LayerEvent::Show { rect, rgba, dirty } => kh_er_render::hud::show_layer(index, rect, &rgba, dirty),
        LayerEvent::Hide => kh_er_render::hud::hide_layer(index),
    });
    answer
}

/// (IMPL_ER15) What a title-mode frame of KH3's Load screen answers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TitleLoad {
    /// The UI pack is still being read.
    Loading,
    /// The pack cannot be read / has no Save / Load screen: the caller leaves the game's own list to the player.
    Unavailable,
    /// The screen is up.
    Open,
    /// A used slot was decided (no question, as in KH3): the screen shows "Loading...".
    Load(usize),
    /// B: the screen is gone.
    Cancel,
}

/// (IMPL_ER15) ONE TITLE-MODE FRAME OF KH3'S LOAD SCREEN (call once a frame from FRAME_BEGIN while it is wanted; the
/// caller draws the black behind it and carries the load out). `open`: open the screen when it is not up (false
/// after a slot was decided: the screen then only shows its "Loading..." message).
pub fn title_load_frame(control: &Control, open: bool) -> TitleLoad {
    let Ok(mut guard) = SHARED.lock() else { return TitleLoad::Unavailable };
    let s = guard.get_or_insert_with(Shared::new);
    if !load_step(s, control) {
        return if s.failed { TitleLoad::Unavailable } else { TitleLoad::Loading };
    }
    let Ok(mut title_guard) = TITLE.lock() else { return TitleLoad::Unavailable };
    let t = title_guard.get_or_insert_with(|| {
        let sora = Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: Vec3::ZERO, yaw: 0.0 });
        let out = sora.output();
        TitleHud { menu_input: MenuInput::new(), held_before: PadButtons::NONE, last: None, pending: 0.0, reader: crate::pad::Reader::default(), sora, out, snapshot: UiSnapshot::default(), targets: ErTargets::new(crate::coords::Frame::at(Vec3::ZERO), crate::ermap::Params::default()) }
    });
    let Some(output) = kh_er_render::hud::output_size().filter(|(w, h)| *w >= 64 && *h >= 64) else { return TitleLoad::Loading };
    let resized = output != s.output;
    s.output = output;
    let started = Instant::now();
    let dt = t.last.replace(started).map_or(1.0 / 60.0, |at| started.duration_since(at).as_secs_f32()).clamp(0.0, 0.25);
    let (mut stick, mut held) = inputs::keyboard();
    if let Some(pad) = t.reader.read() {
        stick += pad.left;
        held = PadButtons(held.0 | pad.buttons.0);
    }
    let pad = PadState::from_held(held, t.held_before);
    t.held_before = held;
    t.menu_input.step(&Input { move_stick: stick.clamp_length_max(1.0), ..Input::default() }.with_pad(pad), dt.max(0.001));
    t.pending += dt;
    let mut ticks = 0;
    while t.pending + 1.0e-5 >= 1.0 / 60.0 && ticks < 6 {
        t.pending -= 1.0 / 60.0;
        ticks += 1;
    }
    if ticks == 6 {
        t.pending = 0.0;
    }
    let (Some(hud), Some(layers)) = (s.hud.as_mut(), s.layers.as_mut()) else { return TitleLoad::Unavailable };
    if !hud.saveload_available() {
        return TitleLoad::Unavailable;
    }
    let options = kh_er_render::hud::HudOptions { on: true, gamma: control.on("hud.gamma"), linear_scale: control.f32("hud.scrgb").unwrap_or(1.0), opacity: 1.0 };
    if options != s.options || !TITLE_ON.load(Ordering::Relaxed) {
        kh_er_render::hud::set_options(options);
        s.options = options;
    }
    TITLE_ON.store(true, Ordering::Relaxed);
    if open && !hud.saveload_open() {
        t.menu_input.clear();
        hud.saveload_background = true;
        hud.set_saveload_options(false, false, false); // (SV) at the title: no Delete (the title's own flow carries only a load)
        let slots = ui_slots();
        if hud.open_load(&slots) {
            erlog!("saveload: KH3'S LOAD SCREEN IS UP ON THE TITLE ({} of {} slots in use; no world is loaded)", slots.iter().filter(|s| s.state == kh_ui::saveload::SlotState::Used).count(), slots.len());
        } else {
            return TitleLoad::Unavailable;
        }
    }
    let (root, _) = stage_root(output.0 as f32, output.1 as f32);
    let (pressed, nav) = t.menu_input.take();
    hud.gameover_on = false;
    hud.set_cutscene(false);
    hud.frame(&mut Frame { snapshot: &t.snapshot, sora: &t.sora, events: &[], state: t.out.state, step_dt: 1.0 / 60.0, play_seconds: 0, targets: &ErHudTargets(&t.targets), to_view: same, view: None, words: &[], dt, ticks, pressed, nav });
    crate::fx::ui_sounds(hud.take_sounds().into_iter().map(|(command, _)| command).collect());
    if let Some(pack) = PACK.lock().ok().and_then(|p| p.clone()) {
        crate::fx::title_pump(&pack, dt, control.f32("snd.vol").unwrap_or(-6.0));
    }
    let answer = match hud.take_saveload_result() {
        Some(kh_ui::saveload::SaveLoadResult::Load(slot)) => TitleLoad::Load(slot),
        Some(kh_ui::saveload::SaveLoadResult::Cancel) => TitleLoad::Cancel,
        _ => TitleLoad::Open,
    };
    layers.update(hud, &root, (output.0 as f32, output.1 as f32), resized, &mut |index, event| match event {
        LayerEvent::Show { rect, rgba, dirty } => kh_er_render::hud::show_layer(index, rect, &rgba, dirty),
        LayerEvent::Hide => kh_er_render::hud::hide_layer(index),
    });
    answer
}

/// (IMPL_ER12) The title mode ends: the HUD's layers are taken off the screen (the caller's own cover layer is its own).
pub fn title_end() {
    if !TITLE_ON.swap(false, Ordering::Relaxed) {
        return;
    }
    if let Ok(mut guard) = SHARED.lock() {
        if let Some(s) = guard.as_mut() {
            if let Some(hud) = s.hud.as_mut() {
                hud.close_newgame_choice();
                hud.close_saveload(); // (IMPL_ER15) the title's Load screen
            }
            for index in 0..kh_host::hud_layers::LAYERS.len() {
                kh_er_render::hud::hide_layer(index);
            }
            if let Some(layers) = s.layers.as_mut() {
                layers.invalidate();
            }
            s.shown = false;
        }
    }
    if let Ok(mut t) = TITLE.lock() {
        *t = None;
    }
}
// ---------------------------------------------------------------------------------- per Sora

/// What a `SoraHost` holds of the HUD: the core's UI sink (the latest snapshot), the menus' pad input.
pub struct HudHost {
    pub keep: Keep,
    menu_input: MenuInput,
    held_before: PadButtons,
    last: Option<Instant>,
    pending: f32,
    died: bool,
    /// (savegate) The game-over menu asked for the last save: Elden Ring's death follows if nothing came of it by then.
    reload_deadline: Option<u64>,
    /// (IMPL_ER10) Elden Ring's own cutscene skip is being pressed: (presses made, the next one's time).
    skip: Option<(u8, u64)>,
    /// (testing aid) `dbg.newgame.choice`.
    choice_shot: Option<OneShot>,
    /// (IMPL_ER15) When this Sora's HUD half was made (a game-over choice carried out before that is done when he has the picture).
    born: Instant,
}

/// The core's UI sink: keeps the latest snapshot.
#[derive(Default)]
pub struct Keep {
    pub snapshot: UiSnapshot,
}

impl kh_core::UiSink for Keep {
    fn publish(&mut self, snapshot: &UiSnapshot) {
        self.snapshot = snapshot.clone();
    }
}

impl HudHost {
    pub fn new(pack: &std::path::Path) -> HudHost {
        *PACK.lock().unwrap_or_else(|e| e.into_inner()) = Some(pack.to_path_buf());
        STOP.store(false, Ordering::Relaxed);
        // (a new Sora: no menu of the one before holds anything)
        MENU_OPEN.store(false, Ordering::Relaxed);
        GAMEOVER_HOLD.store(false, Ordering::Relaxed);
        GAMEOVER_MENU.store(false, Ordering::Relaxed);
        HudHost { keep: Keep::default(), menu_input: MenuInput::new(), held_before: PadButtons::NONE, last: None, pending: 0.0, died: false, reload_deadline: None, skip: None, choice_shot: None, born: Instant::now() }
    }

    /// (hud) hook in `pre_physics`, after the pad was read and Elden Ring's menus took theirs: the menus' pad
    /// input of this frame; while a menu of ours is up Sora gets no stick and no button.
    pub fn input(&mut self, control: &Control, stick: &mut Vec2, right_stick: &mut Vec2, held: &mut PadButtons) {
        if !control.on("hud.kh") {
            return;
        }
        let pad = PadState::from_held(*held, self.held_before);
        self.held_before = *held;
        let input = Input { move_stick: *stick, ..Input::default() }.with_pad(pad);
        // (the key repeat runs on the frame's real time: the game's own delta is 0 while we hold its speed)
        let dt = self.last.map_or(1.0 / 60.0, |at| at.elapsed().as_secs_f32()).clamp(0.001, 0.1);
        self.menu_input.step(&input, dt);
        #[cfg(feature = "party")]
        crate::unlock::note_pad(stick, held); // (ER party S8) Y talks while the prompt is up; a talk line takes the pad (after the HUD saw it: A goes on)
        crate::conv::note_pad(stick, right_stick, held); // (CV1) a conversation holds the leader (after the HUD saw the pad: the bubble has A / B / the D-pad)
        if menu_open() || GAMEOVER_MENU.load(Ordering::Relaxed) {
            *stick = Vec2::ZERO;
            *right_stick = Vec2::ZERO;
            *held = PadButtons::NONE;
        }
    }

    /// (hud) hook at the end of `pre_physics`' simulation part: one HUD frame from the latest step.
    #[allow(clippy::too_many_arguments)]
    /// `down` (DTH): the leader whose death wants KH3's game over (death.rs `Flow::gameover`: whoever leads; never AI Sora
    /// knocked out while a friend leads, never a scripted death).
    pub fn frame(&mut self, sora: &mut Sora, out: &Output, targets: &ErTargets, origin: Vec3, control: &Control, ui: ErUi, down: Option<kh_host::party::PartyChara>) {
        let on = control.on("hud.kh");
        let gameover_on = on && control.on("gameover.kh") && !scripted_death();
        if !gameover_on && GAMEOVER_HOLD.swap(false, Ordering::Relaxed) {
            erlog!("game over: the key went away: Elden Ring's death follows now");
        }
        // (IMPL_ER14) a Save / Load the screen asked for in the frame before: asked of the game here, before the HUD's lock
        if let Some(job) = SCREEN_ASK.lock().ok().and_then(|mut a| a.take()) {
            let asked = match job {
                ScreenJob::Save(slot) => crate::slots::save_to_slot(slot, sora, "KH3's Save screen"),
                ScreenJob::Load(slot) => crate::slots::load_slot(slot),
                // (SV) the delete is carried out here (outside the HUD's lock); the screen shows "Deleting..." meanwhile
                ScreenJob::Delete(slot) => match crate::slots::delete_slot(slot) {
                    Ok(line) => {
                        erlog!("saveload: {line}");
                        Ok(())
                    }
                    Err(why) => {
                        erlog!("saveload: THE DELETE OF SLOT {slot} WAS NOT MADE ({why}); nothing was changed");
                        Err("the delete was not made")
                    }
                },
            };
            *SCREEN_ASKED.lock().unwrap_or_else(|e| e.into_inner()) = Some((job, asked));
        }
        // (IMPL_ER15) the game-over menu's Continue: the game's own load of the session's slot, asked here (outside the lock)
        if let Some(slot) = GO_LOAD.lock().ok().and_then(|mut a| a.take()) {
            match crate::slots::load_slot(slot) {
                Ok(()) => {
                    erlog!("game over: THE LAST SAVE (slot {slot}) IS LOADED AGAIN by the game's own load; the black plate stays until the loaded Sora has the picture");
                    self.reload_deadline = Some(now_ms() + 25_000);
                }
                Err(why) => {
                    erlog!("game over: the load of slot {slot} could NOT be asked for ({why}): Elden Ring's own death and respawn follow instead");
                    RETRY_RESPAWN.store(true, Ordering::Relaxed);
                    GAMEOVER_HOLD.store(false, Ordering::Relaxed);
                }
            }
        }
        let Ok(mut guard) = SHARED.lock() else { return };
        let s = guard.get_or_insert_with(Shared::new);
        // Sora's death under `gameover.kh` (only with the movies loaded: without the menu nothing could end the
        // hold): held from the step he died in until the menu is answered.
        // (ER party S6) a friend leader who falls ends the game as Sora's death does (P's rule; his KO pose in the void)
        // (DTH) WHOEVER LEADS: the down leader comes from the host's death flow (before: Sora's own Die state, also when AI
        // Sora was knocked out while a friend led, or the party's leader-down feed); a friend's feet from the party's feed
        #[cfg(feature = "party")]
        let feet = crate::partyui::leader_feet(control);
        #[cfg(not(feature = "party"))]
        let feet: Option<Vec3> = None;
        let fallen: Option<(kh_host::party::PartyChara, Vec3)> = down.filter(|c| *c != kh_host::party::PartyChara::Sora).map(|c| (c, feet.unwrap_or(out.position)));
        if gameover_on && s.hud.is_some() && down.is_some() && !self.died {
            self.died = true;
            GAMEOVER_CHOSEN.store(false, Ordering::Relaxed);
            GAMEOVER_HOLD.store(true, Ordering::Relaxed);
            erlog!("game over: {} died: the Tarnished is kept alive and unhittable until the game-over menu is answered (gameover.kh = 1)", fallen.map_or("Sora", |f| f.0.name()));
            // (IMPL_ER15) KH3'S GAME-OVER SCENE (`gameover.scene = 0`: the plain menu over the held world, as before): the
            // plate rises (black: the user's word; `gameover.fade = game` = KH3's own white), then the void with the posed
            // Sora and the menu of the Battle variant (`gameover.variant = field`: Continue / Load / Title Screen)
            if scene_key(control) {
                use kh_host::gameover::{GameOverFade, GameOverKind, GameOverSetup, GameOverVariant};
                if let Some(hud) = s.hud.as_mut() {
                    let hint = match control.str("gameover.hint").map(str::trim) {
                        // (the game's own general hints that name nothing the mod lacks, one after the other)
                        Some("general") => {
                            const IDS: [&str; 3] = ["general_0010", "general_0050", "general_0070"];
                            hud.gameover_hint(IDS[HINT_NO.fetch_add(1, Ordering::Relaxed) % IDS.len()])
                        }
                        Some(id) if id != "none" && id != "0" => hud.gameover_hint(id),
                        _ => None,
                    };
                    // (IMPL_ER23A, user decision 2026-10-13: "when Sora dies, just have Continue (which reloads the last save file),
                    // Load and Exit to Main Menu") THE DEFAULT IS KH3'S THREE-ENTRY MENU (menu type 0): Continue / Load / Title
                    // Screen (KH3's own text for leaving to the title: Elden Ring's title screen, nothing saved). Retry and
                    // Prepare & Retry are gone; `gameover.variant = battle` brings the four-entry battle menu back.
                    let variant = match control.str("gameover.variant").map(str::trim) {
                        Some("battle") => GameOverVariant::Battle,
                        Some("noprepare") => GameOverVariant::BattleNoPrepare,
                        _ => GameOverVariant::Field,
                    };
                    let fade = if control.str("gameover.fade").map(str::trim) == Some("game") { GameOverFade::game(GameOverKind::Normal) } else { GameOverFade::black() };
                    let mut disabled = Vec::new();
                    if !hud.saveload_available() || control.flag("saveload.gameover") == Some(false) {
                        disabled.push(GameOverButton::Load);
                    }
                    if control.flag("gameover.title") == Some(false) /* (IMPL_ER23A) live by default */ {
                        disabled.push(GameOverButton::TitleScreen);
                    }
                    let opened = hud.open_gameover(GameOverSetup { variant, hint, fade, disabled, ..GameOverSetup::default() });
                    erlog!("game over: KH3'S GAME-OVER SCENE {} (variant {variant:?}, plate {}, gameover.retry = {})", if opened { "begins" } else { "could NOT begin (a sequence runs already)" }, if fade.colour == [0.0; 3] { "black" } else { "the game's own white" }, control.str("gameover.retry").unwrap_or("respawn"));
                    if let Some(pack) = PACK.lock().ok().and_then(|p| p.clone()) {
                        crate::void::prepare(&pack);
                        // (ER party S6) the void shows the fallen leader in his KO pose
                        #[cfg(feature = "party")]
                        crate::void::set_leader(&pack, fallen.map(|f| f.0));
                    }
                    crate::void::set_death_camera(crate::game::camera().as_ref(), origin + crate::void::host_pos(fallen.map_or(out.position, |f| f.1)));
                }
            }
        }
        if !on {
            if s.shown {
                kh_er_render::hud::clear();
                kh_er_render::hud::set_options(kh_er_render::hud::HudOptions::default());
                s.shown = false;
                s.options = kh_er_render::hud::HudOptions::default();
                MENU_OPEN.store(false, Ordering::Relaxed);
                GAMEOVER_MENU.store(false, Ordering::Relaxed);
                erlog!("hud: Sora's HUD is off (hud.kh)");
            }
            return;
        }
        // ---- the movies: read once, on a thread of their own
        if s.hud.is_none() && !load_step(s, control) {
            return;
        }
        let started = Instant::now();
        let dt = self.last.replace(started).map_or(1.0 / 60.0, |at| started.duration_since(at).as_secs_f32()).clamp(0.0, 0.25);
        FRAME_AT.store(now_ms(), Ordering::Relaxed);
        // ELDEN RING'S FULL-SCREEN MENUS (IMPL_ER8): while its main menu, map, grace menu, a cutscene or a loading screen
        // has the screen (hud_state 1 or 0 with the input) Sora's HUD fades out (0.15 s) and comes back after (0.25 s);
        // a popup (hud_state 2: an item window, a tutorial) leaves it. hud.over_menus = 1: the old behaviour.
        // (IMPL_ER10) IN A CUTSCENE: with `cutscene.menu` the shared HUD code hides the cockpit itself and may show KH3's
        // cutscene pause menu (the overlay stays up for it); without the key the whole overlay fades out.
        let cutscene_menu = control.on("cutscene.menu");
        // (IMPL_ER11) A LOADING SCREEN (MEASURED: after a load the player object and hud_state 3 are back about 4 s before
        // the picture is; the in-game step's update is not called during that time): no step call lately = covered, and
        // the overlay starts from nothing whenever Sora's frames come back.
        // (and for one more second after the step is back: the game fades its picture in from black)
        if crate::pause::hooked() && !crate::pause::available() {
            LOADING_UNTIL.store(now_ms() + 1000, Ordering::Relaxed);
        }
        let loading = now_ms() < LOADING_UNTIL.load(Ordering::Relaxed) && control.flag("hud.loading") != Some(true);
        if !s.shown || loading {
            s.opacity = 0.0;
        }
        LOADING.store(loading, Ordering::Relaxed);
        // (HUD2 round 2, the user's picture: our OBTAINED popup dimmed under Elden Ring's pickup window) while the popup is
        // up the overlay is never faded for a menu of the game's: KH3's popup is on top, fully drawn
        let covered = loading || if OBTAINED_UP.load(Ordering::Relaxed) { false } else if ui.cutscene { !cutscene_menu } else { ui.menu && ui.hud_state != 2 && !control.on("hud.over_menus") && !crate::conv::story_bubble_shown() /* (CV3) a driven story rest whose talk is our bubble: shown */ };
        // (ZFIN round 7, the user's Malenia picture) Zettaflare's cinematic: KH3's HUD and the lock-on reticle go, from the cast's
        // camera take-over to the end of the hand-back (`hud.zetta_hide = 0`: kept)
        let covered = covered || (zetta_cinematic() && control.str("hud.zetta_hide") != Some("0"));
        CUTSCENE_MENU_ON.store(cutscene_menu && ui.cutscene, Ordering::Relaxed);
        CUTSCENE.store(ui.cutscene, Ordering::Relaxed);
        s.opacity = if covered { (s.opacity - dt / 0.15).max(0.0) } else { (s.opacity + dt / 0.25).min(1.0) };
        let options = kh_er_render::hud::HudOptions { on: true, gamma: control.on("hud.gamma"), linear_scale: control.f32("hud.scrgb").unwrap_or(1.0), opacity: s.opacity };
        if options != s.options {
            kh_er_render::hud::set_options(options);
            s.options = options;
        }
        s.shown = true;
        let Some(output) = kh_er_render::hud::output_size().filter(|(w, h)| *w >= 64 && *h >= 64) else { return };
        let resized = output != s.output;
        s.output = output;
        let (root, stage) = stage_root(output.0 as f32, output.1 as f32);
        // Stage frames: real time at 60 a second.
        self.pending += dt;
        let mut ticks = 0;
        while self.pending + 1.0e-5 >= 1.0 / 60.0 && ticks < 6 {
            self.pending -= 1.0 / 60.0;
            ticks += 1;
        }
        if ticks == 6 {
            self.pending = 0.0;
        }
        let (Some(hud), Some(layers)) = (s.hud.as_mut(), s.layers.as_mut()) else { return };
        // (IMPL_ER15) a load was asked for since the last frame: the old session's choice menu goes (its question stays
        // pending for ITS character: save.rs asks again when that one is loaded)
        if SESSION_LOAD.swap(false, Ordering::Relaxed) {
            // (IMPL_ER17) the old session's popups (items, munny, level up) do not belong to the loaded character
            hud.reset_popups();
            erlog!("hud: a load inside the session: the old session's popups are cleared");
            if hud.newgame_choice_open() {
                hud.close_newgame_choice();
                erlog!("newgame: a load was asked for while the opening's choice menu was up: the menu is closed (the loaded character is another one)");
            }
        }
        if !hud.paused() {
            s.play_seconds += dt as f64;
        }
        hud.gameover_on = gameover_on;
        // (HUD2) KH3's OBTAINED popup (key items, new party members): OFF until tested in the game (`hud.obtained = 1`: on)
        hud.obtained_on = control.flag("hud.obtained") != Some(false); // (coordinator 2026-10-20) ON by default: the user asked for it
        // (IMPL_ER14) KH3's Save / Load screens (`saveload.kh = 1`; `saveload.point = 0` / `saveload.gameover = 0` switch one off)
        let screens = control.on("saveload.kh") && hud.saveload_available();
        SAVELOAD_ON.store(screens && control.flag("saveload.point") != Some(false), Ordering::Relaxed);
        let load_screen = screens && control.flag("saveload.gameover") != Some(false);
        // (savegate) with `save.points_only` Continue and Load both load the last save; Title Screen leaves without saving
        // (IMPL_ER14: with the Load screen the Load button is there for every character)
        hud.gameover_disabled = if crate::savegate::points_only() || load_screen { let mut off = Vec::new(); if control.flag("gameover.title") == Some(false) /* (IMPL_ER23A) live by default */ { off.push(GameOverButton::TitleScreen); } if !crate::savegate::points_only() && !load_screen { off.push(GameOverButton::Load); } off } else { vec![GameOverButton::Load, GameOverButton::TitleScreen] };
        // THE SAVE SCREEN out of the save-point menu's Save (asked for by savepoint.rs earlier in this frame)
        if WANT_SAVE_SCREEN.swap(false, Ordering::Relaxed) {
            hud.saveload_background = false;
            play_screen_options(hud, control); // (SV)
            let current = crate::savegate::current_slot().filter(|s| *s >= 0).map(|s| s as usize);
            if hud.open_save(&ui_slots(), current) {
                erlog!("saveload: THE SAVE SCREEN is up (the save point's Save; the session's slot is {current:?})");
            } else {
                erlog!("saveload: the Save screen could NOT open (another menu is up): nothing is saved");
            }
        }
        // (SV) THE LOAD SCREEN out of the save-point menu's Load (asked for by savepoint.rs earlier in this frame): a used slot
        // asks "Load the save data in this slot?" (Yes / No, the cursor on No), Yes = the game's own load of that slot
        // (the in-session load the game-over menu's Load uses); B = back to the save-point menu
        if WANT_LOAD_SCREEN.swap(false, Ordering::Relaxed) {
            hud.saveload_background = false;
            play_screen_options(hud, control);
            let slots = ui_slots();
            if hud.open_load(&slots) {
                erlog!("saveload: THE LOAD SCREEN is up (the save point's Load; the session's slot is {:?})", crate::savegate::current_slot());
            } else {
                erlog!("saveload: the Load screen could NOT open (another menu is up): nothing is loaded");
            }
        }
        if let Some(text) = SCREEN_SHOT.lock().ok().and_then(|mut k| k.get_or_insert_with(|| OneShot::new("dbg.saveload", control)).fired(control)) {
            // (testing aid) dbg.saveload = save | load: a screen from anywhere
            hud.saveload_background = false;
            play_screen_options(hud, control); // (SV)
            let opened = if text.trim() == "save" { hud.open_save(&ui_slots(), crate::savegate::current_slot().filter(|s| *s >= 0).map(|s| s as usize)) } else { hud.open_load(&ui_slots()) };
            erlog!("saveload: dbg.saveload = {text}: {}", if opened { "the screen is up" } else { "it could not open (another menu is up)" });
        }
        // (IMPL_ER13, the user: "also remove all boss health bars") ELDEN RING'S OWN HP DISPLAYS ARE GONE (enemy tags, boss
        // bars): Sora's enemy gauge is the one place an enemy's HP can be read, so it is shown WITHOUT the Scan ability
        // by default (a departure from KH3, where the gauge needs LIBRA / "Scan"; `hud.scan = auto` = KH3's rule).
        // Which enemy: KH3's own rule (the shared HUD code): the one with the auto / lock-on cursor, else the one just hit.
        {
            static SAID: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(2);
            let want = u8::from(control.str("hud.scan") != Some("auto") && control.flag("hud.scan") != Some(false));
            if SAID.swap(want, Ordering::Relaxed) != want {
                say(if want == 1 { "libra on" } else { "libra auto" });
                erlog!("hud: the enemy gauge {}", if want == 1 { "is shown for every enemy (hud.scan: Elden Ring's own HP tags and boss bars are hidden; KH3 would need the Scan ability)" } else { "follows KH3's rule: only with the Scan ability (hud.scan = auto)" });
            }
        }
        let mut words: Vec<String> = s.cmd.as_mut().and_then(|c| c.fired(control)).into_iter().collect();
        if let Ok(mut said) = WORDS.lock() {
            words.append(&mut said);
        }
        // (INT) the HUD's world marks (lock-on, damage numbers, gauges over enemies) through the view drawn in THIS frame
        // (before: the game's camera at the frame task, i.e. the previous frame's: the marks lagged the picture by a frame)
        let view = crate::interp::drawn_camera().map(|camera| View { clip_from_world: hud_clip(&camera, origin, (output.0 as f32, output.1 as f32), (stage.2, stage.3)), size: (stage.2, stage.3) });
        // (an Elden Ring menu has the input: ours gets none, and Start does not open it)
        hud.cutscene_skip_allowed = true;
        hud.set_cutscene(cutscene_menu && ui.cutscene);
        hud.set_host_event(borrowed()); // (ERUI) a borrowed interaction: Start = KH3's Pause screen, as in a battle
        // (IMPL_ER11) ON A LOADING SCREEN NO MENU OF OURS MAY OPEN (MEASURED: a Y / Start pressed while the loading screen
        // was still up opened a menu nobody could see, the world was held under it and the loading screen never ended)
        // (ERUI) a borrowed interaction's "menu" is not one of the game's: our keys pass (sora.rs gives the HUD only Start then)
let (pressed, nav) = if loading || (ui.menu && !borrowed() && !(cutscene_menu && ui.cutscene) && !crate::conv::story_bubble_keys() /* (CV3) a driven story rest whose talk is our bubble: its keys */) { (self.menu_input.take(), (PadButtons::NONE, Vec::new())).1 } else { self.menu_input.take() };
        if control.on("hud.perf") && (pressed != PadButtons::NONE || !nav.is_empty()) {
            erlog!("hudperf: {} ms | INPUT pressed {:?} nav {:?} (dt {:.1} ms, {ticks} stage tick(s))", now_ms(), pressed, nav, dt * 1000.0);
        }
        // (ER party S6) the party into the HUD: the friends' slots, the leader's main slot (shrink / grow), the camp menu's
        // party pages, Customize's AI list, the Party Formation screen; a friend leading: his command menu over Sora's
        #[cfg(feature = "party")]
        let led = crate::partyui::hud_frame(hud, &self.keep.snapshot, control);
        #[cfg(not(feature = "party"))]
        let led: Option<UiSnapshot> = None;
        // (SP2) the save point's "Save" reaction command (KH3's green bar over the command menu, Y), whoever leads
        // (RC) ... and EVERY Y INTERACTION on the same element, in Y's order: Save, a party member's talk (CV3's
        // `offer_talk` owners: Donald's recruit and every later one), Elden Ring's own prompt as a KH3 command (erprompt.rs);
        // the first is the green bar Y answers, the others stack under it. `prompt.kh = 0`: the Save alone, as SP2 had it.
        if control.flag("prompt.kh") != Some(false) {
            let er_prompt = crate::erprompt::er_up().then(|| crate::erprompt::shown().and_then(|s| s.text));
            let offers = crate::erprompt::YOffers { save: crate::savepoint::prompt_shown(), party_talks: crate::conv::party_talks_offered(), er_prompt };
            let list = crate::erprompt::reactions(&offers);
            crate::erprompt::log_change(&list);
            hud.set_host_reactions(&list);
        } else {
            hud.set_host_command(crate::savepoint::prompt_shown().then_some(kh_host::hud::Hud::SAVE_COMMAND));
        }
        hud.frame(&mut Frame { snapshot: led.as_ref().unwrap_or(&self.keep.snapshot), sora, events: &out.events, state: out.state, step_dt: 1.0 / 60.0, play_seconds: s.play_seconds as u32, targets: &ErHudTargets(targets), to_view: same, view, words: &words, dt, ticks, pressed, nav });
        hud.apply_actions(sora);
        OBTAINED_UP.store(hud.obtained_open(), Ordering::Relaxed); // (HUD2)
        #[cfg(feature = "party")]
        crate::partyui::hud_after(hud, control); // (ER party S6) the camp menu's party edits, a Party Formation change
        // (IMPL_ER15) KH3'S GAME-OVER SCENE: the plate, the void, the choice, the way back
        let born = self.born;
        if let Some(deadline) = gameover_scene(hud, out, control, ui, dt, loading, s.opacity, born) {
            self.reload_deadline = deadline;
        }
        point_load_cover(control, loading, born); // (SV)
        // (IMPL_ER14) WHAT THE SAVE / LOAD SCREEN ASKS, and the busy / done handshake around the game's own save and load
        {
            use kh_ui::saveload::{Progress, SaveLoadResult};
            let timeout = control.f32("saveload.timeout").unwrap_or(25.0);
            match hud.take_saveload_result() {
                Some(SaveLoadResult::Save(slot)) => *SCREEN_ASK.lock().unwrap_or_else(|e| e.into_inner()) = Some(ScreenJob::Save(slot)),
                Some(SaveLoadResult::Load(slot)) => *SCREEN_ASK.lock().unwrap_or_else(|e| e.into_inner()) = Some(ScreenJob::Load(slot)),
                Some(SaveLoadResult::Delete(slot)) => *SCREEN_ASK.lock().unwrap_or_else(|e| e.into_inner()) = Some(ScreenJob::Delete(slot)), // (SV)
                Some(SaveLoadResult::Cancel) => erlog!("saveload: the screen was left (B){}", if hud.savepoint_open() { ": the save-point menu is back" } else { "" }),
                None => {}
            }
            match SCREEN_ASKED.lock().ok().and_then(|mut a| a.take()) {
                Some((ScreenJob::Save(slot), asked)) => match asked {
                    Ok(()) => {
                        erlog!("saveload: SAVE INTO SLOT {slot} chosen: the game's own save is asked for; the screen shows \"Saving...\" (the world is let go meanwhile)");
                        *SCREEN_JOB.lock().unwrap_or_else(|e| e.into_inner()) = Some((ScreenJob::Save(slot), Instant::now()));
                    }
                    Err(why) => {
                        erlog!("saveload: SAVE INTO SLOT {slot} chosen: NOT STARTED ({why})");
                        hud.set_saveload_busy(Progress::Failed);
                    }
                },
                // (SV) a delete: done at once (files moved, the byte cleared): the row shows NO DATA, the list has the pad again
                Some((ScreenJob::Delete(slot), asked)) => {
                    if asked.is_ok() {
                        hud.set_saveload_slots(&ui_slots(), crate::savegate::current_slot().filter(|s| *s >= 0).map(|s| s as usize));
                        hud.set_saveload_busy(Progress::Done);
                        erlog!("saveload: SLOT {slot} DELETED: the row shows NO DATA; nothing was saved");
                    } else {
                        hud.set_saveload_busy(Progress::Failed);
                    }
                }
                Some((ScreenJob::Load(slot), asked)) => match asked {
                    Ok(()) => {
                        erlog!("saveload: LOAD OF SLOT {slot} chosen (no question, as in KH3): the game's own load is asked for; the screen shows \"Loading...\"");
                        *SCREEN_JOB.lock().unwrap_or_else(|e| e.into_inner()) = Some((ScreenJob::Load(slot), Instant::now()));
                        // (SV) out of the save point's Load: KH3's fade to black, held until the loaded leader has the picture
                        if !gameover_holds() {
                            *POINT_LOAD.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
                        }
                        // (out of the game-over menu: the world runs for the load, the Tarnished stays alive until it takes him)
                        if gameover_holds() {
                            GAMEOVER_CHOSEN.store(true, Ordering::Relaxed);
                            self.reload_deadline = Some(now_ms() + 25_000);
                            // (IMPL_ER15) out of the game-over scene: black from here until the loaded Sora has the picture
                            if hud.gameover_active() {
                                *CARRIED.lock().unwrap_or_else(|e| e.into_inner()) = Some((Instant::now(), false));
                            }
                        }
                    }
                    Err(why) => {
                        erlog!("saveload: LOAD OF SLOT {slot} chosen: NOT STARTED ({why})");
                        hud.set_saveload_busy(Progress::Failed);
                    }
                },
                None => {}
            }
            let job = *SCREEN_JOB.lock().unwrap_or_else(|e| e.into_inner());
            match job {
                Some((ScreenJob::Save(slot), at)) => {
                    let outcome = match crate::slots::take_save_result() {
                        Some(Ok(_)) => Some(true),
                        Some(Err((_, why))) => {
                            erlog!("saveload: THE SAVE INTO SLOT {slot} FAILED ({why})");
                            Some(false)
                        }
                        None if at.elapsed().as_secs_f32() > timeout => {
                            erlog!("saveload: THE SAVE INTO SLOT {slot} did not end in {timeout} s");
                            Some(false)
                        }
                        None => None,
                    };
                    if let Some(ok) = outcome {
                        *SCREEN_JOB.lock().unwrap_or_else(|e| e.into_inner()) = None;
                        if ok {
                            hud.set_saveload_slots(&ui_slots(), Some(slot));
                            hud.set_saveload_busy(Progress::Done);
                            erlog!("saveload: SLOT {slot} IS SAVED in {:.2} s (Elden Ring's slot, its profile summary, Sora's file): the row shows it, the list has the pad again", at.elapsed().as_secs_f32());
                        } else {
                            hud.set_saveload_busy(Progress::Failed);
                        }
                    }
                }
                Some((ScreenJob::Load(slot), at)) => {
                    // (the world goes away under the screen: it leaves as soon as the game's load has begun; Sora's frames stop then)
                    if at.elapsed().as_millis() > 400 {
                        *SCREEN_JOB.lock().unwrap_or_else(|e| e.into_inner()) = None;
                        hud.set_saveload_busy(Progress::Done);
                        erlog!("saveload: the load of slot {slot} is under way: the screen leaves (the game's loading screen follows)");
                    }
                }
                // (SV) a delete is never waited for (it is done when asked)
                Some((ScreenJob::Delete(_), _)) => *SCREEN_JOB.lock().unwrap_or_else(|e| e.into_inner()) = None,
                None => {}
            }
        }
        // (IMPL_ER21) KH3's Config screen drives Elden Ring's options (erconfig.rs; `config.kh = 0`: as before)
        crate::erconfig::hud_frame(hud, control);
        if hud.take_config_change() {
            use kh_core::config::ConfigOption as O;
            let config = *hud.config();
            let command = &mut sora.command_mut().config;
            command.command_slow = config.get(O::CommandSelection) != 0;
            command.cursor_lock = config.get(O::CursorPosition) != 0;
            command.prioritize_newer = config.get(O::SituationCommands) != 0;
            command.control_type_b = config.control_type_b();
        }
        // (the UI's own sounds are not played in Elden Ring yet: the audio side takes its commands per
        // simulation step, and none runs while a menu holds the pause)
        // (IMPL_ER12) ... they are now: straight to the audio host, on this frame's real time
        crate::fx::ui_sounds(hud.take_sounds().into_iter().map(|(command, _)| command).collect());
        // (IMPL_ER11) THE NEW-GAME CHOICE MENU (KH3's two opening choices): shown as soon as a character whose choices are
        // not made has the screen (no cutscene, no loading screen, no menu of Elden Ring's), with the world held (it
        // counts as a menu of ours: `paused()`), over a dark backdrop of our own.
        if self.choice_shot.get_or_insert_with(|| OneShot::new("dbg.newgame.choice", control)).fired(control).is_some() {
            crate::save::ask_choice_again();
        }
        if let Some(choice) = hud.take_newgame_choice() {
            crate::save::set_choice(choice);
            #[cfg(feature = "party")]
            crate::unlock::chosen_starter(hud.take_newgame_starter()); // (ER party S8)
        }
        if crate::save::choice_pending() && !ui.cutscene && !ui.menu && !loading && s.opacity >= 1.0 && !hud.paused() && !hud.newgame_choice_open() && hud.newgame_choice_available() && sora.status().hp > 0 {
            // (ER party S8) a new character whose starter was not chosen on the title: the character select first
            #[cfg(feature = "party")]
            hud.set_newgame_starter_question(if crate::unlock::asks_starter(control) { crate::unlock::starter_question(control) } else { None });
            if hud.open_newgame_choice() {
                erlog!("newgame: THE OPENING'S CHOICES are asked (KH3's choice menu); the world is held");
            }
        }
        let backdrop = hud.newgame_choice_open();
        if backdrop != s.backdrop {
            s.backdrop = backdrop;
            if backdrop {
                let (w, h) = (output.0 as usize, output.1 as usize);
                let mut rgba = vec![0u8; w * h * 4];
                for px in rgba.chunks_exact_mut(4) {
                    px[3] = 255;
                }
                kh_er_render::hud::show_layer(BACKDROP_LAYER, (0, 0, w, h), &rgba, None);
            } else {
                kh_er_render::hud::hide_layer(BACKDROP_LAYER);
            }
        }
        MENU_OPEN.store(hud.paused(), Ordering::Relaxed);
        CUTSCENE_PAUSE.store(hud.cutscene_pause_open(), Ordering::Relaxed);
        // (IMPL_ER10) KH3's cutscene pause menu: SKIP SCENE = Elden Ring's own skip: its Start / Esc (the first press
        // brings its ":Skip" prompt up, the second skips: MEASURED on the opening movie with Esc, Esc 0.7 s apart)
        if let Some(result) = hud.take_cutscene_pause_result() {
            erlog!("cutscene: the pause menu: {result:?}{}", if result == kh_ui::cutscene_pause::CutscenePauseResult::SkipScene { ": Elden Ring's own skip is pressed for it (Start / Esc, twice)" } else { "" });
            if result == kh_ui::cutscene_pause::CutscenePauseResult::SkipScene {
                self.skip = Some((0, now_ms() + 350));
            }
        }
        if let Some((n, at)) = self.skip {
            if !ui.cutscene || n >= 4 {
                self.skip = None;
            } else if now_ms() >= at && !hud.paused() {
                let how = inputs::press_for_er_try(inputs::XI_START, inputs::SCAN_ESC, n);
                erlog!("cutscene: skip: Start / Esc pressed for the game ({})", how.unwrap_or("NO WAY: neither a pad it reads nor its window in front"));
                self.skip = Some((n + 1, now_ms() + 750));
            }
        }
        GAMEOVER_MENU.store(hud.gameover_open(), Ordering::Relaxed);
        // (IMPL_ER21) "Title Screen" of the camp menu (`menu.title = 0`: no such entry; the save-point menu's too)
        hud.title_entry = control.flag("menu.title") != Some(false);
        crate::boc::camp(control, hud); // (IMPL_ER59) the Outfits entry (after talking to Boc), a choice made there
        if hud.take_title_request() {
            let asked = crate::savegate::quit_to_title("the camp menu's Title Screen");
            erlog!("ui: the Kingdom Hearts menu's TITLE SCREEN entry, confirmed: {}", if asked { "Elden Ring's title screen is asked for; NOTHING IS SAVED (the last Save at a save point stands)" } else { "the game's title request could not be made" });
        }
        // The Config entry: Elden Ring's system menu, once our menu has closed (the pad hook presses for us).
        if hud.take_system_menu() {
            s.system_menu = Some(now_ms() + 3000);
            erlog!("ui: the Kingdom Hearts menu's Config entry: Elden Ring's system menu is opened when this menu has closed");
        }
        if let Some(deadline) = s.system_menu {
            if !hud.paused() {
                if inputs::open_er_system_menu(ui) {
                    erlog!("ui: Elden Ring's system menu: Start, Up, A are being pressed for it through the pad hook");
                    s.system_menu = None;
                } else if now_ms() > deadline {
                    erlog!("ui: Elden Ring's system menu could not be opened (a menu of its is up, or no pad is connected)");
                    s.system_menu = None;
                }
            }
        }
        // The game-over menu's answer: every button that can be chosen here means "go on": Elden Ring's own
        // death and respawn at the last site of grace follow.
        let mut gameover_choice = hud.take_gameover_choice();
        // (IMPL_ER14) LOAD = KH3's Load screen (a B there brings the game-over menu back: he is still dead)
        if gameover_choice == Some(GameOverButton::Load) && load_screen {
            hud.saveload_background = false;
            hud.set_saveload_options(control.flag("saveload.delete") != Some(false), false, false); // (SV) Delete; no question (KH3)
            if hud.open_load(&ui_slots()) {
                erlog!("game over: Load chosen: THE LOAD SCREEN is up (the Tarnished is kept alive meanwhile)");
                gameover_choice = None;
            } else {
                erlog!("game over: Load chosen: the Load screen could not open: the last save is loaded instead");
            }
        }
        if let Some(choice) = gameover_choice {
            GAMEOVER_CHOSEN.store(true, Ordering::Relaxed);
            // (savegate) `save.points_only`: CONTINUE / LOAD = THE LAST SAVE IS LOADED AGAIN by the game's own load (no
            // death of Elden Ring's: the Tarnished stays alive and unhittable until the load takes him away);
            // TITLE SCREEN = Elden Ring's title without saving.
            let asked = if !crate::savegate::points_only() {
                false
            } else if choice == GameOverButton::TitleScreen {
                crate::savegate::quit_to_title("game over: Title Screen")
            } else {
                crate::savegate::reload("game over: Continue")
            };
            if asked {
                erlog!("game over: {choice:?} chosen: {} (the Tarnished is kept alive meanwhile)", if choice == GameOverButton::TitleScreen { "Elden Ring's title screen is asked for, nothing is saved" } else { "THE LAST SAVE IS LOADED AGAIN" });
                self.reload_deadline = Some(now_ms() + 25_000);
            } else {
                erlog!("game over: {choice:?} chosen: the Tarnished's HP goes to 0 now (Elden Ring's death and respawn)");
                GAMEOVER_HOLD.store(false, Ordering::Relaxed);
                if control.flag("gameover.banner") == Some(false) {
                    s.banner_until = now_ms() + 15_000;
                }
            }
        }
        if self.reload_deadline.is_some_and(|at| now_ms() > at) {
            self.reload_deadline = None;
            erlog!("game over: NOTHING CAME OF THE LOAD in 25 s: Elden Ring's own death and respawn follow instead");
            GAMEOVER_HOLD.store(false, Ordering::Relaxed);
        }
        let logic = started.elapsed().as_secs_f64();
        let pictures_started = Instant::now();
        layers.update(hud, &root, (output.0 as f32, output.1 as f32), resized, &mut |index, event| match event {
            LayerEvent::Show { rect, rgba, dirty } => kh_er_render::hud::show_layer(index, rect, &rgba, dirty),
            LayerEvent::Hide => kh_er_render::hud::hide_layer(index),
        });
        // (IMPL_ER21) Sora's model in the camp menu's character panel (portrait.rs): only while that menu's movie is up.
        // (IMPL_ER34) AFTER the layers: he is placed by the menu state whose picture is on screen (the worker's pictures
        // come a frame or two late), and the menu's screen stack goes to the log (`menu:` lines).
        {
            let pack = PACK.lock().ok().and_then(|p| p.clone());
            let camp = layers.layers.get(crate::portrait::CAMP_LAYER).map_or((0, 0, false), |l| (l.sent, l.taken, l.busy()));
            // (ER party I, S4's request) Sora on HIS panel of the party's camp menu (kh-app's rule; Sora first = as before)
            #[cfg(feature = "party")]
            let sora_panel = Some((hud.camp_panels(), hud.camp_member()));
            #[cfg(not(feature = "party"))]
            let sora_panel = None;
            crate::portrait::frame(control, hud.movie("CampMenu"), &root, output, dt, pack.as_deref(), camp, sora_panel);
            // (ER party S4) the friends' pictures on their panels (their UI actors' captures and lights, the eyes open)
            #[cfg(feature = "party")]
            crate::partydraw::camp(control, hud.movie("CampMenu"), &root, output, dt, pack.as_deref(), hud.camp_panels(), hud.camp_member(), camp);
            crate::portrait::menu_trace(hud.camp_trace(), dt);
        }
        let pictures = pictures_started.elapsed().as_secs_f64();
        // (IMPL_ER14, hud.perf = 1) every picture of the layers, and every menu input, with the real time
        layers.trace = control.on("hud.perf");
        // (IMPL_ER21) `hud.scale9 = 0`: scaled sprites are stretched whole, as before the pack's scaling grids were applied
        {
            let grids = control.flag("hud.scale9") != Some(false);
            if grids != kh_ui::render::scale9_on() {
                kh_ui::render::set_scale9(grids);
                layers.invalidate();
                erlog!("hud: the movies' scaling grids are {}", if grids { "applied" } else { "NOT applied (hud.scale9 = 0)" });
            }
        }
        // (HB) `hud.solid_masks = 0`: a sprite mask lets through by its shapes' alpha, as before (Sora's HP ring and focus
        // gauge at half strength); default on = masks are coverage only (KH3 / Scaleform), the gauges vivid
        {
            let solid = control.flag("hud.solid_masks") != Some(false);
            if solid != kh_ui::render::solid_masks_on() {
                kh_ui::render::set_solid_masks(solid);
                layers.invalidate();
                erlog!("hud: (HB) sprite masks are {}", if solid { "coverage only (KH3)" } else { "drawn with their alpha, as before (hud.solid_masks = 0)" });
            }
        }
        // (IMPL_ER14, the user: "the camp menu is laggy") THE PICTURES' SWITCHES (kh-host hud_layers::Perf): the changed
        // pieces one rectangle each (`hud.split = 0`: their bounding box, as before), large rectangles in parallel
        // bands (`hud.bands = n`; default: the machine's threads less two, at most 12; 1 = as before)
        layers.perf = kh_host::hud_layers::Perf {
            split_dirty: control.flag("hud.split") != Some(false),
            bands: control.i64("hud.bands").map_or_else(|| std::thread::available_parallelism().map_or(4, |n| n.get().saturating_sub(2)).clamp(1, 12), |v| v.clamp(1, 32) as usize),
            band_pixels: control.i64("hud.band_px").map_or(150_000, |v| v.max(10_000) as usize),
        };
        // (ER party S6) KH3'S EXACTFIT 16:9 VIEWPORT FOR THE COCKPIT (0x1438F8E90): at an output wider or taller than 16:9
        // nothing of the status / command / side HUDs shows outside the stage (the friends' gauge plates of the party
        // layout reach 120 stage px past its right edge: at 21:9 they stood in the open beside it); the target HUD (its
        // cursors follow the world) and the menus are not clipped. `hud.clip = 0`: as before.
        #[cfg(feature = "party")]
        {
            let clip = (crate::partyui::key_on() && control.flag("hud.clip") != Some(false)).then_some(stage);
            if layers.clip != clip {
                layers.clip = clip;
                layers.clip_only = vec![1, 2, 3, 4];
                layers.invalidate();
            }
        }
        for line in std::mem::take(&mut layers.trace_lines) {
            erlog!("hudperf: {} ms | {line} | this frame: logic {:.2} ms, pictures {:.2} ms", now_ms(), logic * 1000.0, pictures * 1000.0);
        }
        s.frames += 1;
        s.logic += logic;
        s.pictures += pictures;
        s.worst = s.worst.max(logic + pictures);
    }
}

/// (IMPL_ER15) `gameover.scene`: KH3's game-over scene instead of the plain menu over the held world. DEFAULT ON.
fn scene_key(control: &Control) -> bool {
    control.flag("gameover.scene") != Some(false)
}

/// (IMPL_ER15) KH3'S GAME-OVER SCENE, one frame (after the shared HUD's frame): the plate over the world and the cockpit
/// (under the menu), the void (the renderer clears the picture to black and draws the posed Sora), the menu's choice
/// carried out, and the way back: the plate stays black until the NEW Sora has the picture, then leaves in 0.5 s.
/// `Some(deadline)`: the caller's "nothing came of the load" deadline is set / cleared.
#[allow(clippy::too_many_arguments)]
fn gameover_scene(hud: &mut Hud, out: &Output, control: &Control, ui: ErUi, dt: f32, loading: bool, opacity: f32, born: Instant) -> Option<Option<u64>> {
    use kh_ui::gameover::GameOverResult as R;
    let mut deadline = None;
    // ---- the way back: the choice was carried out before this Sora began, and he has the picture now
    if let Some((at, prepare)) = carried() {
        let fresh = born > at && !loading && out.state != kh_core::StateId::Die && !ui.cutscene;
        if fresh || at.elapsed().as_secs() > 60 {
            erlog!("game over: {} {:.1} s after the choice: the void goes, the plate leaves over the world in 0.5 s{}", if fresh { "THE NEW SORA HAS THE PICTURE" } else { "NOTHING CAME OF THE CHOICE" }, at.elapsed().as_secs_f32(), if prepare { "; the camp menu opens by itself when he stands (Prepare & Retry)" } else { "" });
            hud.end_gameover(true);
            RETRY_DEATH.store(false, Ordering::Relaxed);
            *CARRIED.lock().unwrap_or_else(|e| e.into_inner()) = None;
            GAMEOVER_CHOSEN.store(false, Ordering::Relaxed);
            if prepare && fresh {
                *PREPARE.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now() + std::time::Duration::from_secs(20));
            }
            deadline = Some(None);
        }
    }
    // ---- Prepare & Retry: the camp menu by itself once he stands and the plate has left
    if let Some(until) = PREPARE.lock().ok().and_then(|p| *p) {
        if Instant::now() > until {
            *PREPARE.lock().unwrap_or_else(|e| e.into_inner()) = None;
            erlog!("game over: Prepare & Retry: the camp menu could not open within 20 s (he never stood free): dropped");
        } else if !hud.gameover_active() && !loading && !ui.menu && !ui.cutscene && opacity >= 1.0 && out.on_floor && !hud.paused() && hud.open_camp_menu() {
            *PREPARE.lock().unwrap_or_else(|e| e.into_inner()) = None;
            erlog!("game over: Prepare & Retry: THE CAMP MENU OPENS BY ITSELF (he stands again; the world is held as for any camp menu)");
        }
    }
    let view = hud.gameover_view();
    // ---- the void: the posed Sora under the plate (numbers: kh_host::gameover; LOCAL: the exposure in front of the tone curve)
    let scene = view.and_then(|v| v.scene);
    crate::void::frame(scene.as_ref(), dt, control.f32("gameover.exposure").unwrap_or(crate::void::EXPOSURE), control.f32("gameover.light").unwrap_or(1.0), control.f32("gameover.cam_dz").unwrap_or(0.0));
    // ---- the choice
    if let Some(result) = hud.take_gameover_result() {
        let retry_load = control.str("gameover.retry").map(str::trim) == Some("load");
        match result {
            R::Load => {
                // (the movie's own dark backdrop: the list is alone on the screen, as in KH3; the void waits under it)
                hud.saveload_background = true;
                hud.set_saveload_options(control.flag("saveload.delete") != Some(false), false, false); // (SV) Delete; no question (KH3)
                if hud.open_load(&ui_slots()) {
                    erlog!("game over: Load chosen: THE LOAD SCREEN is up over the void (B brings the menu back)");
                } else {
                    erlog!("game over: Load chosen: the Load screen could not open");
                }
            }
            R::Retry | R::PrepareAndRetry if !retry_load => {
                // ELDEN RING'S OWN RESPAWN at the last grace / Stake of Marika, the world kept: his HP goes to 0 under the
                // black plate (frame.rs does not load the last save after this death)
                erlog!("game over: {result:?} chosen: ELDEN RING'S OWN DEATH AND RESPAWN follow under the black plate (gameover.retry = respawn; the world is kept)");
                RETRY_RESPAWN.store(true, Ordering::Relaxed);
                RETRY_DEATH.store(true, Ordering::Relaxed);
                GAMEOVER_CHOSEN.store(true, Ordering::Relaxed);
                GAMEOVER_HOLD.store(false, Ordering::Relaxed);
                // (MEASURED in IMPL_ER15 run 1: with the request fought the game never respawned (it lay dead for good): only on the explicit key)
                BANNER_ASK.store(control.flag("gameover.banner") == Some(false), Ordering::Relaxed);
                *CARRIED.lock().unwrap_or_else(|e| e.into_inner()) = Some((Instant::now(), result == R::PrepareAndRetry));
            }
            R::TitleScreen | R::Exit | R::GiveUp => {
                let asked = crate::savegate::quit_to_title("game over: Title Screen");
                erlog!("game over: {result:?} chosen: Elden Ring's title screen {}", if asked { "is asked for" } else { "could NOT be asked for" });
                GAMEOVER_CHOSEN.store(true, Ordering::Relaxed);
                *CARRIED.lock().unwrap_or_else(|e| e.into_inner()) = Some((Instant::now(), false));
                deadline = Some(Some(now_ms() + 25_000));
            }
            _ => {
                // CONTINUE (and Retry / Prepare & Retry under `gameover.retry = load`) = the last save is loaded again
                match crate::savegate::current_slot().filter(|s| *s >= 0) {
                    Some(slot) => {
                        erlog!("game over: {result:?} chosen: the last save of slot {slot} is asked for at the next frame");
                        *GO_LOAD.lock().unwrap_or_else(|e| e.into_inner()) = Some(slot as usize);
                    }
                    None => {
                        erlog!("game over: {result:?} chosen: the session's slot is not known: Elden Ring's own death and respawn follow instead");
                        RETRY_RESPAWN.store(true, Ordering::Relaxed);
                        GAMEOVER_HOLD.store(false, Ordering::Relaxed);
                    }
                }
                GAMEOVER_CHOSEN.store(true, Ordering::Relaxed);
                *CARRIED.lock().unwrap_or_else(|e| e.into_inner()) = Some((Instant::now(), matches!(result, R::PrepareAndRetry)));
            }
        }
    }
    // ---- the plate: over the world and the cockpit, under the menu's layer; while a choice is carried out it must
    // outlive this Sora's frames (a load, a respawn: no frames of ours for seconds): held for 40 s unless renewed
    let view = hud.gameover_view();
    let plate = match (carried(), view) {
        // (a choice is being carried out: full black whatever the sequence says: the Load screen's load has no plate of its own)
        (Some(_), _) => Some(([0.0, 0.0, 0.0, 1.0], 40_000)),
        (None, Some(v)) if v.fade > 0.0 => Some(([v.fade_colour[0], v.fade_colour[1], v.fade_colour[2], v.fade], 400)),
        _ => None,
    };
    match plate {
        Some((colour, hold)) if control.flag("dbg.go.plate") != Some(false) => {
            kh_er_render::hud::set_plate(Some((GAMEOVER_LAYER, colour, hold)));
            PLATE_ON.store(true, Ordering::Relaxed);
        }
        _ => {
            if PLATE_ON.swap(false, Ordering::Relaxed) {
                kh_er_render::hud::set_plate(None);
            }
        }
    }
    deadline
}

/// (SV) A load out of the save point's Load screen was asked for at this moment: the black cover is up.
static POINT_LOAD: Mutex<Option<Instant>> = Mutex::new(None);
/// (SV) The cover leaves: since when.
static POINT_LOAD_OUT: Mutex<Option<Instant>> = Mutex::new(None);

/// (SV) THE SAVE POINT'S LOAD, ITS WAY THROUGH BLACK (no half-loaded picture): the screen fades to black over
/// `saveload.fade_in` s (default 0.25) as the game's own load begins; black stays (over the world and the cockpit, under the
/// menus) through the game's loading until the LOADED leader has the picture (his first frames after the load), then it
/// leaves in `saveload.fade_out` s (default 0.5). Nothing came of the load in 60 s: it leaves. `saveload.cover = 0`: none.
fn point_load_cover(control: &Control, loading: bool, born: Instant) {
    let at = POINT_LOAD.lock().ok().and_then(|p| *p);
    let out = POINT_LOAD_OUT.lock().ok().and_then(|p| *p);
    if at.is_none() && out.is_none() {
        return;
    }
    if control.flag("saveload.cover") == Some(false) {
        *POINT_LOAD.lock().unwrap_or_else(|e| e.into_inner()) = None;
        *POINT_LOAD_OUT.lock().unwrap_or_else(|e| e.into_inner()) = None;
        kh_er_render::hud::set_plate(None);
        return;
    }
    let fade_in = control.f32("saveload.fade_in").filter(|v| v.is_finite() && *v >= 0.0).unwrap_or(0.25);
    let fade_out = control.f32("saveload.fade_out").filter(|v| v.is_finite() && *v > 0.0).unwrap_or(0.5);
    if let Some(at) = at {
        let fresh = born > at && !loading;
        if fresh || at.elapsed().as_secs() > 60 {
            erlog!("saveload: (SV) the save point's load: {} {:.1} s after the choice: the black leaves in {fade_out:.1} s", if fresh { "THE LOADED LEADER HAS THE PICTURE" } else { "NOTHING CAME OF THE LOAD" }, at.elapsed().as_secs_f32());
            *POINT_LOAD.lock().unwrap_or_else(|e| e.into_inner()) = None;
            *POINT_LOAD_OUT.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
            return;
        }
        let alpha = if fade_in <= 0.0 { 1.0 } else { (at.elapsed().as_secs_f32() / fade_in).min(1.0) };
        kh_er_render::hud::set_plate(Some((GAMEOVER_LAYER, [0.0, 0.0, 0.0, alpha], 40_000)));
        return;
    }
    if let Some(out) = out {
        let left = 1.0 - out.elapsed().as_secs_f32() / fade_out;
        if left <= 0.0 {
            *POINT_LOAD_OUT.lock().unwrap_or_else(|e| e.into_inner()) = None;
            kh_er_render::hud::set_plate(None);
        } else {
            kh_er_render::hud::set_plate(Some((GAMEOVER_LAYER, [0.0, 0.0, 0.0, left], 400)));
        }
    }
}

/// (IMPL_ER15) The Retry's death is being carried out by Elden Ring (its death, its "Choose revival location" window, its respawn).
static RETRY_DEATH: AtomicBool = AtomicBool::new(false);
/// The revival window's answer: (presses made, ms of the next one, the window was seen).
static REVIVE: Mutex<(u8, u64, bool)> = Mutex::new((0, 0, false));

/// (IMPL_ER15) ELDEN RING'S "Choose revival location" WINDOW (Stake of Marika / Last site of grace visited; MEASURED: it
/// comes ~11 s after the death when a stake is near, and waits for an answer for ever: under our black plate nobody
/// would see it): answered for the player while the Retry's death is carried out. `gameover.revive = stake` (default:
/// confirm = the stake, the nearest thing to "retry the fight") | `grace` (right, confirm) | `ask` (our plate is
/// lifted while the window is up: the player answers it himself). The window = interface element `gameover.revive_ui`
/// (default 4, MEASURED: visible from the moment the window comes, not in play; it stays visible through the respawn's loading, where a confirm does nothing).
fn revive_phase(control: &Control) {
    if !RETRY_DEATH.load(Ordering::Relaxed) {
        return;
    }
    let Some((at, _)) = carried() else { return };
    let Ok(mut st) = REVIVE.lock() else { return };
    if at.elapsed().as_millis() < 500 {
        *st = (0, 0, false);
    }
    let element = control.i64("gameover.revive_ui").map_or(4, |v| v.clamp(0, 0x45) as usize);
    let states = inputs::ui_states();
    let up = states.is_some_and(|s| s[element] & 2 != 0) && at.elapsed().as_secs() >= 4 && crate::game::player_mut().is_some_and(|p| !crate::game::player_alive(p));
    let now = now_ms();
    if !up {
        if st.2 {
            erlog!("game over: Elden Ring's revival window is gone after {} press(es): its respawn follows", st.0);
            *st = (0, 0, false);
        }
        return;
    }
    let mode = control.str("gameover.revive").map(str::trim).unwrap_or("stake");
    if !st.2 {
        st.2 = true;
        st.1 = now + 600;
        erlog!("game over: ELDEN RING'S \"Choose revival location\" WINDOW IS UP {:.1} s after the Retry (interface element {element}): {}", at.elapsed().as_secs_f32(), match mode { "ask" => "our plate is lifted: the player answers it (gameover.revive = ask)", "grace" => "answered for the player: Last site of grace visited (gameover.revive = grace)", _ => "answered for the player: Stake of Marika (the window's own first choice; gameover.revive = stake)" });
    }
    if mode == "ask" {
        kh_er_render::hud::set_plate(None);
        return;
    }
    if now >= st.1 && st.0 < 40 {
        // (grace: one step right first; then confirm; every press by the pad hook when the game reads a pad, else its keyboard key)
        let how = if mode == "grace" && st.0 % 2 == 0 { inputs::press_for_er_try(0x0008, 0x4D, 0) } else { inputs::press_for_er_try(inputs::XI_A, inputs::SCAN_E, st.0 / 2) };
        if st.0 < 6 {
            erlog!("game over: revival window: press {} ({})", st.0 + 1, how.unwrap_or("NO WAY: neither a pad the game reads nor its window in front"));
        }
        st.0 += 1;
        st.1 = now + 700;
    }
}

/// (IMPL_ER15) `gameover.banner` is not 1: Elden Ring's "YOU DIED" request is fought after the Retry's death.
static BANNER_ASK: AtomicBool = AtomicBool::new(false);

/// (DTH) A scripted death that fell back to the game's own death (death.rs `death.wait`): its YOU DIED text is held away
/// as after the Retry's death (the same UNTESTED field write, 20 s).
pub fn hold_banner() {
    BANNER_ASK.store(true, Ordering::Relaxed);
}

// ---------------------------------------------------------------------------------- every phase

/// (hud) hook in frame.rs, every phase of ours: Elden Ring's own HUD, the world's pause, the YOU DIED text,
/// the HUD going away with Sora (a death, a loading screen), the once-a-second log line.
pub fn er_phase(control: &Control) {
    er_hud_phase(control);
    pause_phase(control);
    revive_phase(control); // (IMPL_ER15)
    let Ok(mut guard) = SHARED.lock() else { return };
    let Some(s) = guard.as_mut() else { return };
    let now = now_ms();
    // Sora's frames stopped (he is gone: a death, a loading screen, another mode): nothing of ours stays on screen.
    if s.shown && now.saturating_sub(FRAME_AT.load(Ordering::Relaxed)) > 300 {
        kh_er_render::hud::clear();
        if let Some(layers) = s.layers.as_mut() {
            layers.invalidate();
        }
        s.shown = false;
        s.backdrop = false; // (IMPL_ER15) the layers were cleared: the choice menu's black is put up again when it is wanted
        MENU_OPEN.store(false, Ordering::Relaxed);
        GAMEOVER_MENU.store(false, Ordering::Relaxed);
        erlog!("hud: Sora's frames stopped: his HUD is taken off the screen until he is back");
    }
    // `gameover.banner = 0` (UNTESTED): Elden Ring's full-screen text request is held at "none" for 15 s
    // after the Continue (IMPL_ER4: `FrontEndViewValues::full_screen_message_request_id`).
    if BANNER_ASK.swap(false, Ordering::Relaxed) {
        s.banner_until = now + 20_000;
    }
    if now < s.banner_until {
        use eldenring::cs::CSFeManImp;
        use fromsoftware_shared::FromStatic;
        // SAFETY: game thread; the singleton lives as long as the game; a plain i32 field.
        if let Ok(fe) = unsafe { CSFeManImp::instance_mut() } {
            if fe.frontend_values.full_screen_message_request_id != -1 {
                fe.frontend_values.full_screen_message_request_id = -1;
            }
        }
    }
    if s.shown && now.saturating_sub(s.told_at) >= 1000 {
        s.told_at = now;
        let n = s.frames.max(1) as f64;
        erlog!(
            "hud: on | {} frames | game thread: logic {:.2} ms, pictures {:.2} ms per frame, worst frame {:.2} ms | menu {} | {}\n  {}",
            s.frames,
            s.logic * 1000.0 / n,
            s.pictures * 1000.0 / n,
            s.worst * 1000.0,
            if menu_open() { "OPEN (Sora is paused)" } else if gameover_holds() { "GAME OVER" } else { "closed" },
            s.layers.as_ref().map_or(String::new(), |l| l.report()),
            kh_er_render::hud::status()
        );
        (s.frames, s.logic, s.pictures, s.worst) = (0, 0.0, 0.0, 0.0);
    }
}

/// (hud) hook in entry.rs: the payload goes away: Elden Ring gets its HUD and its speed back.
pub fn shutdown() {
    crate::erhud::clear();
    crate::erhud::unstick();
    crate::erhud::test_boss_bar(None);
    if let Ok(mut st) = ER_OPTION.lock() {
        if let Some((own, ours)) = st.0.take() {
            if let Some(byte) = hud_type_byte() {
                if *byte == ours {
                    *byte = own;
                }
            }
            if let Some(path) = st.1.as_ref() {
                let _ = std::fs::remove_file(path);
            }
        }
    }
    if let Ok(mut st) = ER_HUD.lock() {
        if st.ours {
            if let Some(byte) = hud_state_byte() {
                if *byte == 0 {
                    *byte = 3;
                }
            }
            st.ours = false;
        }
    }
    let field = PAUSE_FIELD.swap(0, Ordering::SeqCst);
    if field != 0 {
        // SAFETY: as in `watchdog`.
        unsafe { (field as *mut f32).write_volatile(f32::from_bits(PAUSE_RESTORE.load(Ordering::SeqCst) as u32)) };
    }
    if let Ok(mut p) = PAUSE.lock() {
        p.before = None;
    }
    MENU_OPEN.store(false, Ordering::Relaxed);
    GAMEOVER_HOLD.store(false, Ordering::Relaxed);
    GAMEOVER_MENU.store(false, Ordering::Relaxed);
    kh_er_render::hud::clear();
    kh_er_render::hud::set_plate(None);
    crate::void::shutdown();
    crate::portrait::shutdown(); // (IMPL_ER21)
    #[cfg(feature = "party")]
    crate::partydraw::shutdown(); // (ER party S4)
    // (the rasteriser's thread ends with its `Layers`; ours are waited for: the payload may be unloaded next)
    if let Ok(mut guard) = SHARED.lock() {
        *guard = None;
    }
    STOP.store(true, Ordering::SeqCst);
    for handle in std::mem::take(&mut *THREADS.lock().unwrap_or_else(|e| e.into_inner())) {
        let _ = handle.join();
    }
    *LOADED.lock().unwrap_or_else(|e| e.into_inner()) = None;
    TITLE_ON.store(false, Ordering::Relaxed);
    if let Ok(mut t) = TITLE.lock() {
        *t = None;
    }
}

/// (HUD2) THE ENEMY GAUGE READS THE GAME'S OWN HP. The user: "the green bar [of the Scan gauge] sometimes jumps when the
/// enemy is damaged before settling". The rows of `ErTargets` are read from the game every frame; a hit by bullet leaves
/// the EXPECTED HP in `hp` until the next read, and the game applies the bullet a frame or more later (or the bullet waits
/// its turn, or misses and is fired again). The gauge used `hp`: down to the guess, back UP to the untouched HP at the
/// next read, down again to what the game really took. It now reads `hp_game`.
#[cfg(test)]
mod hud2_tests {
    use super::*;
    use crate::coords::Frame as ErFrame;
    use crate::ermap::{Params, Weight};
    use crate::targets::Enemy;

    /// The rows as the frames leave them: (hp, hp_game) per frame, as refresh / apply_hit / reconcile write them.
    fn feed(rows: &[(i32, i32)]) -> Vec<(f32, f32)> {
        let mut t = ErTargets::new(ErFrame::at(Vec3::ZERO), Params::default());
        let mut out = Vec::new();
        for (hp, game) in rows {
            let mut e = Enemy::stub(7, Vec3::new(300.0, 0.0, 0.0), 40.0, 90.0, 4000, Weight::Small);
            e.hp = *hp;
            e.hp_game = *game;
            t.list = vec![e];
            let view = ErHudTargets(&t).target(TargetId(7)).expect("a view");
            out.push((view.hp, view.hp_max));
        }
        out
    }

    #[test]
    fn hud2_the_gauge_never_shows_a_bullets_guess() {
        // frame 0 untouched; frame 1 Sora's hit: the row's guess 3874 (attack 126 x ratio 1), the game still 4000;
        // frame 2 re-read, the bullet not landed yet: 4000 / 4000; frame 3 landed: the game took 70 -> 3930.
        let rows = [(4000, 4000), (3874, 4000), (4000, 4000), (3930, 3930), (3930, 3930)];
        let shown: Vec<f32> = feed(&rows).iter().map(|v| v.0).collect();
        assert_eq!(shown, vec![4000.0, 4000.0, 4000.0, 3930.0, 3930.0], "the game's HP only: one clean drop");
        assert!(shown.windows(2).all(|w| w[1] <= w[0]), "never back up: {shown:?}");
        // what the old feed (`hp`) showed: down to the guess, back up, down again
        let old: Vec<i32> = rows.iter().map(|r| r.0).collect();
        assert!(old.windows(2).any(|w| w[1] > w[0]), "the old feed jumped back up: {old:?}");
        // a bullet that waits its turn (one a frame) and a miss fired again: the guess is held for several frames,
        // the game's HP does not move until a bullet lands
        let rows = [(4000, 4000), (3874, 4000), (4000, 4000), (3874, 4000), (4000, 4000), (4000, 4000), (3930, 3930), (3800, 3930), (3860, 3860)];
        let shown: Vec<f32> = feed(&rows).iter().map(|v| v.0).collect();
        assert!(shown.windows(2).all(|w| w[1] <= w[0]), "multi-hits / waiting bullets: never back up: {shown:?}");
        assert_eq!(*shown.last().unwrap(), 3860.0);
        // the maximum is the game's, scaled like the HP (hud.enemy_gauge.hp_scale)
        assert!(feed(&[(10, 10)]).iter().all(|v| v.1 == 4000.0));
    }

    #[test]
    fn hud2_a_hit_points_the_gauge_without_a_mixed_unit_prev_hp() {
        let t = ErTargets::new(ErFrame::at(Vec3::ZERO), Params::default());
        // the core's hit result is the guess in Elden Ring HP plus the damage in KH3 units: no start value from it
        assert_eq!(ErHudTargets(&t).hit_prev_hp(TargetId(7), 3874.0, 12.6), None);
        // a host without that trait answer keeps KH3's prevHP = hp after + damage
        assert_eq!(kh_host::hud::NoTargets.hit_prev_hp(TargetId(7), 90.0, 10.0), Some(100));
    }
}
