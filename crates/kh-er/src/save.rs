//! (save) SORA'S SAVE DATA IN ELDEN RING (`save.kh = 1`; OFF without the key: then nothing here reads or
//! writes a file and Sora starts as before). The format and the files are `kh_host::save`; this is who the
//! character is and when the file is read and written.
//!
//! WHERE: `<root>\saves\<character key>.khsave` (`<root>` = the folder above `mods`: `er\saves`), one backup
//! `.khsave.bak` beside it. Nothing is written into the game's or Steam's folders; ER0000.sl2 is never touched.
//!
//! WHO (`er_identity`, THE ONE FUNCTION TO CONFIRM IN THE GAME): the character's name (`PlayerGameData`, the
//! 17 UTF-16 units in front of `gender`: the field is private in the SDK), Elden Ring's save slot
//! (`GameMan::save_slot`) and its play-time counter (`GameDataMan::play_time`, ms). Key = `character_key(name,
//! slot)`. A file whose recorded Elden Ring play time is more than two minutes AHEAD of the character's
//! belongs to an earlier character of the same name in the same slot: it is moved aside (`.old`) and a new
//! game starts.
//!
//! WHEN:
//! - READ when Sora starts for a character not loaded yet in this session (`on_start`, a hook in
//!   `SoraHost::start`); a later start in the same session (a respawn) takes the session's latest state
//!   instead of the file, with full gauges (Elden Ring's respawn at a grace is a full recovery).
//! - WRITTEN (the events that exist in kh-er): when Elden Ring saves (`GameMan::save_requested` rising:
//!   UNTESTED field, the autosave and the grace use it by the SDK's notes), every `save.minutes` (default 5),
//!   when Sora stops with the player still there (mode change, payload shutdown: `stop`), when his frames
//!   stop (he went away: a death's loading screen, quit to the title: `er_phase`), and at a payload shutdown.
//!   NOT HOOKED (no event known): the exact moment of sitting at a grace (covered by Elden Ring's own save
//!   request, if that field behaves as described) and the game's exit without a payload shutdown (covered by
//!   the timer: at most `save.minutes` are lost).
//! - `sora.level = N` in the control file: the test loadout of that level, NOTHING is read or written (the
//!   save is not touched).

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

use kh_core::status::Loadout;
use kh_core::{Sora, Status};
use kh_host::save::{self, Loaded, SoraSave};

use crate::control::Control;
use crate::erlog;

struct State {
    dir: PathBuf,
    key: String,
    name: String,
    /// The file may be written (false: `sora.level` testing, a refused newer file, no identity).
    writable: bool,
    /// The latest state captured (written when Sora goes away without a `stop`).
    latest: Option<SoraSave>,
    dirty: bool,
    captured_at: Instant,
    written_at: Instant,
    save_requested_before: bool,
    /// Sora is running (frames arrive).
    live_at: Option<Instant>,
    /// (IMPL_ER11, new game) The opening's two choices have not been made: NOTHING IS WRITTEN until they are (a quit
    /// before the answer leaves no file: the menu comes again); the base loadout the chosen start is built from.
    choice_pending: Option<Loadout>,
    /// The base loadout sheet this character's Sora was built from.
    base: Loadout,
}

/// (testing aid, `dbg.newgame.choice`) The opening's choices are asked again for the running character.
pub fn ask_choice_again() {
    if let Ok(mut guard) = STATE.lock() {
        if let Some(state) = guard.as_mut() {
            state.choice_pending = Some(state.base.clone());
        }
    }
}

/// (IMPL_ER11) The pair the player confirmed in the choice menu (set by the HUD frame, taken by `choice_tick`).
static CHOSEN: Mutex<Option<kh_core::status::OpeningChoice>> = Mutex::new(None);

/// (new game) The opening's choices are still to be made for this character (the HUD opens the menu).
pub fn choice_pending() -> bool {
    // (an answer that `choice_tick` has not taken yet is an answer)
    if CHOSEN.lock().is_ok_and(|c| c.is_some()) {
        return false;
    }
    STATE.lock().is_ok_and(|g| g.as_ref().is_some_and(|s| s.choice_pending.is_some()))
}

/// (new game) The HUD's choice menu was answered.
pub fn set_choice(choice: kh_core::status::OpeningChoice) {
    if let Ok(mut c) = CHOSEN.lock() {
        *c = Some(choice);
    }
}

/// (new game) hook in `pre_physics` beside `tick`: the answered pair makes THE TRUE LEVEL-1 START of that pair
/// (munny and play time kept), and the file is written for the first time.
pub fn choice_tick(sora: &mut Sora) {
    let Some(choice) = CHOSEN.lock().ok().and_then(|mut c| c.take()) else { return };
    let Ok(mut guard) = STATE.lock() else { return };
    let Some(state) = guard.as_mut() else { return };
    let Some(base) = state.choice_pending.take() else { return };
    let munny = sora.status().munny;
    *sora.status_mut() = Status::from_loadout(&save::new_game_loadout_for(&base, choice));
    sora.status_mut().munny = munny;
    reset_command(sora);
    erlog!("save: THE OPENING'S CHOICES: {:?} + {:?}: Sora's start is rebuilt for them (HP {}, MP {}, {} abilities owned)", choice.desire, choice.power, sora.status().max_hp(), sora.status().max_mp(), sora.status().abilities.iter_owned().count());
    capture(state, sora);
    state.dirty = true;
    if !points_only() {
        write(state, "the opening's choices were made");
    }
}

static STATE: Mutex<Option<State>> = Mutex::new(None);
static WRITE_ASKED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// (testing aid) Sora's file is written at his next frame, whatever the saving mode.
pub fn ask_write() {
    WRITE_ASKED.store(true, std::sync::atomic::Ordering::Relaxed);
}
/// (savegate) Sora's status came from the file under `save.points_only`: his munny is the file's (the wallet
/// sets the game's counter to it instead of adopting the counter). Taken once by the wallet.
static FILE_MUNNY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// (savegate) `save.points_only`: Sora's file is written by the save point's Save only (with the game's own).
fn points_only() -> bool {
    crate::savegate::points_only()
}

/// (savegate) The munny of the file that was just loaded stands (peek / take-once for the wallet).
pub fn file_munny() -> bool {
    FILE_MUNNY.load(std::sync::atomic::Ordering::Relaxed)
}
pub fn take_file_munny() -> bool {
    FILE_MUNNY.swap(false, std::sync::atomic::Ordering::Relaxed)
}

/// (savegate) THE LAST SAVE IS BEING LOADED AGAIN (Continue after a death, the game's own load): this
/// session's state is dropped, the next start of Sora reads the file.
pub fn forget_session() {
    if let Ok(mut guard) = STATE.lock() {
        if guard.take().is_some() {
            erlog!("save: this session's state of Sora is dropped: his next start reads the loaded slot's save file");
        }
    }
    if let Ok(mut pending) = SLOT_SAVE.lock() {
        *pending = None;
    }
}

/// (IMPL_ER13) THE TITLE IS UP: the session is over. A character whose saving is still the game's own gets its
/// latest state written first (the game saved on its way out); then the state is dropped: whatever is loaded
/// next (the same slot or another) reads its own file.
pub fn left_for_title() {
    if let Ok(mut guard) = STATE.lock() {
        if let Some(state) = guard.as_mut() {
            if !points_only() {
                write(state, "the title screen is up");
            }
        }
        if guard.take().is_some() {
            erlog!("save: the title screen is up: this session's state of Sora is dropped (the next load reads its slot's file)");
        }
    }
    if let Ok(mut pending) = SLOT_SAVE.lock() {
        *pending = None;
    }
}

/// (IMPL_ER13) Where Sora's files are (`<root>\<save.dir>`), known from the first frame (the title needs it too).
static DIR: Mutex<Option<PathBuf>> = Mutex::new(None);

pub fn set_dir(root: &Path, control: &Control) {
    if let Ok(mut dir) = DIR.lock() {
        let now = root.join(control.str("save.dir").unwrap_or("saves"));
        if dir.as_ref() != Some(&now) {
            *dir = Some(now);
        }
    }
}

pub fn dir() -> Option<PathBuf> {
    DIR.lock().ok().and_then(|d| d.clone())
}

/// The running character's file key (`None`: Sora has no save data in this session).
pub fn current_key() -> Option<String> {
    STATE.lock().ok().and_then(|g| g.as_ref().map(|s| s.key.clone()))
}

/// (IMPL_ER13) What Sora's file of a slot says, for the slot lists: (level, munny, seconds played as Sora, the
/// game's play time it is paired with in ms). Read only; a damaged file is `None` (nothing is moved or repaired).
pub fn peek(name: &str, slot: i32) -> Option<(u32, i32, u64, u64)> {
    let path = save::save_path(&dir()?, &save::character_key(name, slot));
    let found = SoraSave::from_json(&std::fs::read_to_string(path).ok()?).ok()?;
    Some((found.level, found.munny, found.play_seconds, found.host_play_ms))
}

/// (ER party S6) The picture of the save slot: the party leader its file names (kh_host `slot_summary_of`; an old save
/// = Sora). `None`: Sora has no file there.
#[cfg(feature = "party")]
pub fn peek_portrait(name: &str, slot: i32) -> Option<kh_ui::saveload::Portrait> {
    let path = save::save_path(&dir()?, &save::character_key(name, slot));
    let found = SoraSave::from_json(&std::fs::read_to_string(path).ok()?).ok()?;
    Some(save::slot_summary_of(slot.max(0) as usize, &found).portrait)
}

/// (IMPL_ER13) The character of that name in that slot saves at save points only (its marker file is there).
pub fn armed_slot(name: &str, slot: i32) -> bool {
    dir().is_some_and(|d| marker(&d, &save::character_key(name, slot)).is_file())
}

/// (IMPL_ER13) A SAVE INTO A SLOT, Sora's half: (the target key, his state captured when the save was asked for).
static SLOT_SAVE: Mutex<Option<(String, SoraSave)>> = Mutex::new(None);

/// (IMPL_ER13, slots.rs) THE SESSION IS ABOUT TO BE SAVED INTO `slot`: Sora's state of this moment is captured for
/// the TARGET slot's key (nothing is written yet: the game's save comes first; `slot_save_finish` writes it or
/// drops it). `Ok(false)`: Sora has no save data in this session (`save.kh` off, a test level): the game's save
/// alone is made. `Err`: his file must not be written (a newer file, the opening's choices not made).
pub fn slot_save_begin(sora: &Sora, slot: i32) -> Result<bool, &'static str> {
    let Ok(mut guard) = STATE.lock() else { return Err("the save state is locked") };
    let Some(state) = guard.as_mut() else { return Ok(false) };
    if !state.writable {
        return Err("Sora's file of this character must not be written (a newer or unmovable file)");
    }
    if state.choice_pending.is_some() {
        return Err("the opening's choices are not made yet");
    }
    if sora.status().hp <= 0 {
        return Err("Sora is down");
    }
    capture(state, sora);
    let Some(latest) = state.latest.clone() else { return Err("nothing was captured") };
    let key = save::character_key(&state.name, slot);
    if let Ok(mut pending) = SLOT_SAVE.lock() {
        *pending = Some((key, latest));
    }
    Ok(true)
}

/// (IMPL_ER13, slots.rs) THE GAME'S SAVE INTO THE SLOT IS WRITTEN (`done`) or nothing came of it: with `done` the
/// captured state goes to the TARGET slot's file, paired with the game's play time of now, the session's key
/// becomes that slot's (later writes go there; the slot it was loaded from keeps its file untouched), and the
/// slot's save-points marker is made (`save.points_only`: the slot is armed). Returns whether everything of
/// Sora's was written (true also when he has no save data).
pub fn slot_save_finish(done: bool) -> bool {
    let Some((key, mut pending)) = SLOT_SAVE.lock().ok().and_then(|mut p| p.take()) else { return true };
    if !done {
        erlog!("save: the game did not save: Sora's state captured for {key} is dropped (nothing was written)");
        return false;
    }
    let Ok(mut guard) = STATE.lock() else { return false };
    let Some(state) = guard.as_mut() else { return false };
    pair_now();
    pending.host_play_ms = er_identity().map_or(pending.host_play_ms, |(_, _, ms)| ms);
    let path = save::save_path(&state.dir, &key);
    let written = match save::write(&path, &pending) {
        Ok(()) => {
            erlog!("save: {} written (the save into its slot): level {}, EXP {}, munny {}, {} s played, paired with the game's {:.1} min", path.display(), pending.level, pending.exp, pending.munny, pending.play_seconds, pending.host_play_ms as f64 / 60_000.0);
            true
        }
        Err(e) => {
            erlog!("save: {} could NOT be written (the save into its slot): {e}", path.display());
            false
        }
    };
    if key != state.key {
        erlog!("save: Sora's file key {} -> {key}: this session goes on in that slot; the old slot's file is left as it was saved", state.key);
        state.key = key;
    }
    state.latest = Some(pending);
    state.dirty = false;
    state.written_at = Instant::now();
    if written && crate::savegate::mode_on() {
        let path = marker(&state.dir, &state.key);
        match std::fs::write(&path, "This character saves at save points only (kh-er save.points_only). Delete this file to give its saving back to Elden Ring.\r\n") {
            Ok(()) => crate::savegate::set_armed(true, "a Save at a save point into this slot"),
            Err(e) => erlog!("save: the marker {} could not be written ({e}): the game goes on saving by itself", path.display()),
        }
    }
    written
}

/// WHO THE CHARACTER IS: (name, save slot, Elden Ring's play time in ms). TO CONFIRM IN THE GAME (the log
/// line `save: character ...` shows what was read): the name's place is taken from the SDK's layout (private
/// field `character_name: [u16; 17]` directly in front of `gender`).
fn er_identity() -> Option<(String, i32, u64)> {
    use eldenring::cs::{GameDataMan, GameMan};
    use fromsoftware_shared::FromStatic;
    // SAFETY: game thread; the singletons live as long as the game; plain reads.
    unsafe {
        let data = GameDataMan::instance().ok()?;
        let player = &*data.main_player_game_data;
        let end = std::ptr::addr_of!(player.gender) as *const u16;
        let units = std::slice::from_raw_parts(end.byte_sub(34), 17);
        let length = units.iter().position(|u| *u == 0).unwrap_or(16);
        let name = String::from_utf16_lossy(&units[..length]);
        let slot = GameMan::instance().map(|g| g.save_slot).unwrap_or(-1);
        Some((name, slot, u64::from(data.play_time)))
    }
}

fn er_save_requested() -> bool {
    use eldenring::cs::GameMan;
    use fromsoftware_shared::FromStatic;
    // SAFETY: game thread; a plain byte read (as a byte: an odd value must not become a bool).
    unsafe { GameMan::instance().map(|g| *(std::ptr::addr_of!(g.save_requested) as *const u8) != 0).unwrap_or(false) }
}

/// (DF) The difficulty of a new game: control key `difficulty` (critical / proud / standard / beginner), else the host sheet's
/// row `difficulty.new_game` (beside the loadout; STANDARD, decision dec032), else Standard.
fn new_game_level(loadout_path: &Path, control: &Control) -> kh_core::status::GameLevel {
    use kh_core::status::GameLevel;
    if let Some(text) = control.str("difficulty") {
        match GameLevel::from_name(text) {
            Some(level) => return level,
            None => erlog!("save: difficulty = {text} is not critical / proud / standard / beginner: the host sheet decides"),
        }
    }
    let sheet = std::fs::read_to_string(loadout_path.with_file_name("sora-er-host.json")).ok().and_then(|t| crate::ermap::Params::from_sheet(&t).ok()).map(|(p, _)| p.new_game_level);
    sheet.and_then(|n| GameLevel::ALL.get(n as usize).copied()).unwrap_or(GameLevel::Standard)
}

fn base_loadout(loadout_path: &Path) -> Option<Loadout> {
    let text = std::fs::read_to_string(loadout_path).ok()?;
    kh_host::rig::loadout::parse(&loadout_path.to_string_lossy(), &text).ok().map(|(loadout, _)| loadout)
}

/// (save) hook in `SoraHost::start`, after the loadout's status was given to `sora`: with `save.kh = 1` Sora
/// becomes the character's saved Sora, or a new game's (level 1).
pub fn on_start(sora: &mut Sora, root: &Path, loadout_path: &Path, control: &Control) {
    if !control.on("save.kh") {
        if let Ok(mut guard) = STATE.lock() {
            *guard = None;
        }
        crate::savegate::set_armed(false, "save.kh is off: Sora has no save data"); // (savegate)
        return;
    }
    let Some(base) = base_loadout(loadout_path) else {
        erlog!("save: the loadout sheet {} cannot be read: nothing is loaded or saved", loadout_path.display());
        return;
    };
    let levels_sheet = std::fs::read_to_string(loadout_path.with_file_name("sora-test-levels.json")).ok();
    // (DF, 2026-10-15; the user: "the overall difficulty of the mod should be like Kingdom Hearts critical mode") THE
    // DIFFICULTY OF A NEW GAME (control key `difficulty`, else the host sheet's `difficulty.new_game`: Standard). A LOADED
    // SAVE keeps its own (one written before DF: the base sheet's Standard, what it was made at) unless
    // `difficulty.existing = switch` (SoraSave::switch_difficulty, written with the next save).
    let new_level = new_game_level(loadout_path, control);
    let base_level = kh_core::status::GameLevel::from_name(&base.game_level).unwrap_or_default();
    let new_base = save::with_difficulty(&base, new_level);
    erlog!("save: difficulty of a new game: {} ({}); a save without one plays at {}", new_level.name(), if control.str("difficulty").is_some() { "control key difficulty" } else { "host sheet difficulty.new_game" }, base_level.name());
    let Ok(mut guard) = STATE.lock() else { return };
    // `sora.level = N`: the test loadout of that level; the save is not touched.
    if let Some(level) = control.i64("sora.level").filter(|l| (1..=99).contains(l)) {
        let loadout = save::test_loadout(&new_base, level as u32, levels_sheet.as_deref()); // (DF) the new-game difficulty
        *sora.status_mut() = Status::from_loadout(&loadout);
        reset_command(sora);
        erlog!("save: sora.level = {level}: the test loadout of that level (HP {}, MP {}); NOTHING is read or written while the key is there", sora.status().max_hp(), sora.status().max_mp());
        *guard = None;
        crate::savegate::set_armed(false, "sora.level testing: Sora has no save data"); // (savegate)
        return;
    }
    let Some((name, slot, play_ms)) = er_identity() else {
        erlog!("save: the character could not be identified (game data manager not there): level-1 new game in memory, nothing is written");
        *sora.status_mut() = Status::from_loadout(&save::new_game_loadout(&new_base, levels_sheet.as_deref())); // (DF)
        reset_command(sora);
        *guard = None;
        crate::savegate::set_armed(false, "the character could not be identified"); // (savegate)
        return;
    };
    let key = save::character_key(&name, slot);
    // (`save.dir`: another folder beside `mods`, for tests on a copy of the save)
    let dir = root.join(control.str("save.dir").unwrap_or("saves"));
    let marked = marker(&dir, &key).is_file();
    // (IMPL_ER21) `save.points_only = 1` IS ABSOLUTE: a character that exists (its save-points marker, its creation
    // marker, Sora's own file, or more than five minutes of play) is armed at once; a NEW one is armed when its
    // creation save is written (savegate.rs `CREATION`)
    let exists = marked || created_marker(&dir, &key).is_file() || save::save_path(&dir, &key).is_file() || play_ms > 300_000;
    let armed = if crate::savegate::absolute() { exists } else { marked };
    crate::savegate::set_creation_pending(crate::savegate::absolute() && !exists);
    crate::savegate::set_armed(armed, if marked { "its marker file is there" } else if armed { "save.points_only = 1: the game never saves an existing character by itself" } else if crate::savegate::absolute() { "a NEW character: the game saves until its creation save is made (in a few seconds)" } else { "no marker file" }); // (savegate)
    // A respawn in the same session: the session's latest state, gauges full.
    if let Some(state) = guard.as_mut().filter(|s| s.key == key) {
        if let Some(latest) = state.latest.clone() {
            let mut inventory = crate::hud::inventory();
            latest.apply(&base, sora, &mut inventory);
            crate::hud::set_formation_record(&latest.party); // (party P)
            #[cfg(feature = "party")]
            crate::partyui::load_record(&latest.party); // (ER party S6) the friends' progression, the formation, Zero EXP, the unlocks
            #[cfg(feature = "party")]
            crate::unlock::on_record(&latest.party); // (ER party S8) who is unlocked
            sora.status_mut().full_recover();
            crate::hud::set_inventory(inventory);
            crate::hud::set_play_seconds(latest.play_seconds as f64);
            erlog!("save: Sora starts again for \"{name}\" with this session's state: level {}, EXP {}, gauges full", sora.status().level, sora.status().exp);
            state.live_at = Some(Instant::now());
            return;
        }
    }
    erlog!("save: character \"{name}\", Elden Ring save slot {slot}, its play time {:.1} min -> key {key}", play_ms as f64 / 60_000.0);
    let mut writable = true;
    let mut loaded = save::load(&dir, &key, &mut |line| erlog!("{line}"));
    if let Loaded::Save(found) | Loaded::Backup(found) = &loaded {
        if found.host_play_ms > play_ms + 120_000 {
            let path = save::save_path(&dir, &key);
            let aside = path.with_extension(format!("khsave.old{}", found.host_play_ms / 1000));
            let moved = std::fs::rename(&path, &aside).is_ok();
            erlog!("save: the file was written at {:.1} min of Elden Ring play time but this character is at {:.1} min: it belongs to an EARLIER character of this name and slot: {} ; A NEW GAME", found.host_play_ms as f64 / 60_000.0, play_ms as f64 / 60_000.0, if moved { format!("moved aside as {}", aside.display()) } else { "it could not be moved aside: NOTHING is written this session".to_string() });
            writable = moved;
            loaded = Loaded::NewGame;
        }
    }
    // (IMPL_ER11) A version-1 file (made before the choices existed) of a character that is still at the very start
    // (level 1, no EXP) is asked too; an older character keeps Balance + the warrior's path (what its file was built with).
    let version_1 = std::fs::read_to_string(save::save_path(&dir, &key)).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()).is_some_and(|v| v["version"].as_u64() == Some(1));
    let mut ask = false;
    let mut chosen_on_title = false;
    let mut inventory = kh_host::hud::Inventory::default();
    let mut play_seconds = 0u64;
    FILE_MUNNY.store(false, std::sync::atomic::Ordering::Relaxed);
    match loaded {
        Loaded::Save(mut found) | Loaded::Backup(mut found) => {
            // (savegate; IMPL_ER13: armed or not) the loaded file's munny stands: the wallet sets the game's counter to it
            FILE_MUNNY.store(true, std::sync::atomic::Ordering::Relaxed);
            // (DF) an existing character keeps its difficulty unless the user asks for the one-time switch
            let have = found.game_level(base_level);
            if have != new_level {
                if control.str("difficulty.existing").map(str::trim) == Some("switch") && writable {
                    let lines = found.switch_difficulty(have, new_level);
                    erlog!("save: DIFFICULTY SWITCH of this character (difficulty.existing = switch): {}; written with the next save", lines.join("; "));
                } else {
                    erlog!("save: this character plays at {} (its save); a new game would be {}. difficulty.existing = switch switches it (once, written with the next save)", have.name(), new_level.name());
                }
            }
            found.apply(&base, sora, &mut inventory);
            crate::hud::set_formation_record(&found.party); // (party P)
            #[cfg(feature = "party")]
            crate::partyui::load_record(&found.party); // (ER party S6) the friends' progression, the formation, Zero EXP, the unlocks
            #[cfg(feature = "party")]
            crate::unlock::on_record(&found.party); // (ER party S8) who is unlocked
            // (IMPL_ER11) A LOADED SORA STARTS WITH FULL GAUGES, as a save loaded in KH3 does (its saves are made at save
            // points, healed). THE "ALWAYS FLASHING RED" REPORT: the user's file had been written by an earlier build at
            // the moment of his death (`"hp": 0`): every start after that put a Sora at 0 HP on screen = the low-HP alarm
            // (red ring + red face) for good. A file with low HP did the same until the first heal.
            let _ = found.hp;
            sora.status_mut().full_recover();
            play_seconds = found.play_seconds;
            if version_1 {
                ask = found.level <= 1 && found.exp == 0;
                erlog!("save: the file is of version 1 (before the opening's choices): {}", if ask { "the character is still at the very start: THE CHOICE MENU IS SHOWN" } else { "the character has progressed: it keeps Balance + the warrior's path" });
            }
        }
        other => {
            writable &= other != Loaded::Refused;
            ask = other == Loaded::NewGame;
            // (ER party S6) a new character: the friends start from KH3's init data (nothing of another character's party)
            #[cfg(feature = "party")]
            crate::partyui::load_record(&serde_json::Value::Null);
            *sora.status_mut() = Status::from_loadout(&save::new_game_loadout(&new_base, levels_sheet.as_deref())); // (DF)
            // (IMPL_ER12) THE PAIR CHOSEN ON THE TITLE'S BLACK, before this character existed: its true start is built
            // from it now and the after-intro question does not come.
            // (IMPL_ER13) ONLY A CHARACTER THAT IS NEW (under 10 minutes of the game's play time) takes it: a load of an
            // older slot that has no file of Sora's must not eat the pair that waits for the character being made.
            if ask && writable && play_ms < 600_000 {
                if let Some(choice) = crate::newgame::take_pending(&dir) {
                    *sora.status_mut() = Status::from_loadout(&save::new_game_loadout_for(&new_base, choice)); // (DF)
                    erlog!("save: THE OPENING'S CHOICES made on the title ({:?} + {:?}) are applied to this new character (HP {}, MP {})", choice.desire, choice.power, sora.status().max_hp(), sora.status().max_mp());
                    ask = false;
                    chosen_on_title = true;
                }
            }
            // (ER party S8) a new character: the starter chosen on the title (else the menu after the intro asks it)
            #[cfg(feature = "party")]
            crate::unlock::on_new_game(&dir, ask && writable && control.flag("newgame.choice") != Some(false));
            reset_command(sora);
        }
    }
    let status = sora.status();
    erlog!("save: Sora is level {} (EXP {}, HP {}/{}, MP {}/{}, munny {}, {} abilities owned, AP {}/{}); the file {} written", status.level, status.exp, status.hp, status.max_hp(), status.mp, status.max_mp(), status.munny, status.abilities.iter_owned().count(), status.ap_in_use(), status.max_ap(), if writable { "will be" } else { "is NOT" });
    crate::hud::set_inventory(inventory);
    crate::hud::set_play_seconds(play_seconds as f64);
    // (the loaded save is the pair of whatever is written until the game saves again)
    PAIRED_MS.store(play_ms.max(1), std::sync::atomic::Ordering::Relaxed);
    let now = Instant::now();
    *guard = Some(State { dir, key, name, writable, latest: None, dirty: false, captured_at: now, written_at: now, save_requested_before: er_save_requested(), live_at: Some(now), choice_pending: (ask && writable && control.flag("newgame.choice") != Some(false)).then(|| new_base.clone()), base: new_base /* (DF) */ });
    // (the pair must not be lost with the pending file gone: the first file is written at once)
    if chosen_on_title {
        if let Some(state) = guard.as_mut() {
            capture(state, sora);
            state.dirty = true;
            write(state, "the opening's choices were made on the title");
        }
    }
}

/// The command system and the pouch are filled afresh from a status that was replaced.
fn reset_command(sora: &mut Sora) {
    sora.command_mut().filled = false;
    sora.magic_mut().pouch = kh_core::magic::items::Pouch::default();
    kh_core::command::customize::ensure(sora);
}

/// (IMPL_ER12) ELDEN RING'S PLAY TIME AT THE GAME'S LAST SAVE of this character in this session (0: none yet): what
/// Sora's file records as its pair. THE "EARLIER CHARACTER" RULE compares the file's recorded time with the loaded
/// character's: a file written by the timer or when Sora went away used to carry the time OF THAT WRITE, minutes after
/// the game's own last save; after a crash or a quit without saving the game's save was then "older than Sora's file"
/// and the file was moved aside as another character's (SEEN in IMPL_ER11). Now a write never claims a later time than
/// the game's own save it belongs to.
static PAIRED_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn pair_now() {
    if let Some((_, _, ms)) = er_identity() {
        PAIRED_MS.store(ms.max(1), std::sync::atomic::Ordering::Relaxed);
    }
}

/// The play time a capture records: the game's at its last save (never later than now).
fn paired_ms(now_ms: u64) -> u64 {
    match PAIRED_MS.load(std::sync::atomic::Ordering::Relaxed) {
        0 => now_ms,
        paired => paired.min(now_ms),
    }
}

fn capture(state: &mut State, sora: &Sora) {
    let play_ms = paired_ms(er_identity().map_or(0, |(_, _, ms)| ms));
    let play_seconds = crate::hud::play_seconds().max(state.latest.as_ref().map_or(0.0, |l| l.play_seconds as f64)) as u64;
    let mut now = SoraSave::capture(sora, &crate::hud::inventory(), &state.name, play_seconds, play_ms);
    now.party = crate::hud::formation_record(); // (party P) the save point's Party Formation
    // (ER party S6) with the party live: format 2 + the v5 friends record + Zero EXP + the unlocks (save.kh version 5)
    #[cfg(feature = "party")]
    if let Some(record) = crate::partyui::party_record_now() {
        now.party = record;
    }
    #[cfg(feature = "party")]
    crate::unlock::stamp(&mut now.party); // (ER party S8) the unlocks and the starter (a fight guest is not saved)
    // (SV, ERT4) EVERY SAVE NAMES THE TRUE LEADER AND PARTY, the first one too: before the party runtime is live (the title-time
    // creation save, the first frames) the record above is the HUD's default formation (Sora leading); made true to the unlocks
    // it names the starter (a Goofy start: Goofy) and only unlocked members. A live record is already true (unchanged).
    if kh_host::formation::fit_to_unlocked(&mut now.party) {
        static SAID: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if !SAID.swap(true, std::sync::atomic::Ordering::Relaxed) {
            erlog!("save: (SV) the party record was made true to the unlocks: leader {}, party {} (the party was not live yet: the HUD's default formation was Sora's)", now.party["leader"], now.party["active"]);
        }
    }
    if state.latest.as_ref() != Some(&now) {
        state.latest = Some(now);
        state.dirty = true;
    }
    state.captured_at = Instant::now();
}

fn write(state: &mut State, why: &str) {
    if state.choice_pending.is_some() {
        return;
    }
    let Some(latest) = state.latest.as_ref().filter(|_| state.writable && state.dirty) else { return };
    let path = save::save_path(&state.dir, &state.key);
    match save::write(&path, latest) {
        Ok(()) => erlog!("save: {} written ({why}): level {}, EXP {}, munny {}, {} s played", path.display(), latest.level, latest.exp, latest.munny, latest.play_seconds),
        Err(e) => erlog!("save: {} could NOT be written ({why}): {e}", path.display()),
    }
    state.dirty = false;
    state.written_at = Instant::now();
}

/// (save) hook in `pre_physics`, once a frame while Sora runs: the latest state is captured every two
/// seconds; the file is written when Elden Ring saves and every `save.minutes`.
pub fn tick(sora: &Sora, control: &Control) {
    // (IMPL_ER13, testing aid `dbg.saveto = N`) the session is saved into slot N through the slot API
    if let Some(n) = crate::slots::take_dbg_save() {
        let asked = crate::slots::save_to_slot(n, sora, "dbg.saveto");
        erlog!("slots: dbg.saveto = {n}: {asked:?}");
    }
    let Ok(mut guard) = STATE.lock() else { return };
    let Some(state) = guard.as_mut() else { return };
    if !control.on("save.kh") {
        return;
    }
    state.live_at = Some(Instant::now());
    // (a dead Sora is not captured: the respawn takes the state from before the blow that killed him)
    if state.captured_at.elapsed().as_secs_f32() >= 2.0 && sora.status().hp > 0 {
        capture(state, sora);
    }
    // (IMPL_ER12, `dbg.save`) the testing aid's save: Sora's file is written with the game's (under the current key),
    // and an armed character's marker goes to the current key too
    if WRITE_ASKED.swap(false, std::sync::atomic::Ordering::Relaxed) {
        pair_now();
        capture(state, sora);
        state.dirty = true;
        write(state, "dbg.save");
        if points_only() {
            let _ = std::fs::write(marker(&state.dir, &state.key), "This character saves at save points only (kh-er save.points_only). Delete this file to give its saving back to Elden Ring.\r\n");
        }
    }
    // (savegate) `save.points_only`: no write on the game's own requests and none on the timer
    if points_only() {
        state.save_requested_before = false;
        state.written_at = Instant::now();
        return;
    }
    // (IMPL_ER13) a save INTO A SLOT is under way: its request is not "the game is saving" of the slot this session
    // was in (SEEN: the old slot's file was rewritten a moment before the session moved to the new slot)
    if SLOT_SAVE.lock().is_ok_and(|p| p.is_some()) {
        state.save_requested_before = true;
        return;
    }
    let requested = er_save_requested();
    if requested && !state.save_requested_before {
        pair_now();
        capture(state, sora);
        write(state, "Elden Ring is saving");
    }
    state.save_requested_before = requested;
    let minutes = control.f32("save.minutes").unwrap_or(5.0).max(0.2);
    if state.written_at.elapsed().as_secs_f32() >= minutes * 60.0 {
        capture(state, sora);
        write(state, "timer");
        state.written_at = Instant::now();
    }
}

/// (savepoint) The save point's Save: the state now goes to the file (nothing happens without `save.kh`).
pub fn write_now(sora: &Sora, why: &str) {
    let Ok(mut guard) = STATE.lock() else { return };
    let Some(state) = guard.as_mut() else { return };
    pair_now();
    if sora.status().hp > 0 {
        capture(state, sora);
    }
    state.dirty = true;
    write(state, why);
}

/// (IMPL_ER12, the save-slot experiment) The game's current save slot was changed under the running character: Sora's
/// file (and the save-points marker) are from now the TARGET slot's (`character_key(name, slot)`); the next write goes there.
pub fn retarget_slot() {
    let Some((name, slot, _)) = er_identity() else { return };
    let Ok(mut guard) = STATE.lock() else { return };
    let Some(state) = guard.as_mut() else { return };
    let key = save::character_key(&name, slot);
    if key != state.key {
        let was_armed = marker(&state.dir, &state.key).is_file();
        erlog!("save: Sora's file key {} -> {key} (the game's save slot is now {slot}){}", state.key, if was_armed { "; the save-points marker goes with it at the next Save" } else { "" });
        state.key = key;
        state.dirty = true;
    }
}

/// (IMPL_ER21) The marker of a character whose creation save was made under `save.points_only = 1`.
fn created_marker(dir: &Path, key: &str) -> PathBuf {
    dir.join(format!("{key}.created"))
}

/// (IMPL_ER21) THE NEW CHARACTER'S CREATION SAVE IS WRITTEN (savegate.rs): its marker is made and from now the
/// game never saves this character by itself.
pub fn creation_saved() {
    let Ok(guard) = STATE.lock() else { return };
    let Some(state) = guard.as_ref() else {
        erlog!("save: the creation save is written but Sora's state is gone: the character is armed at its next start");
        return;
    };
    let path = created_marker(&state.dir, &state.key);
    let _ = std::fs::create_dir_all(&state.dir);
    match std::fs::write(&path, "The creation save of this character was made (kh-er save.points_only = 1): from then only a Save at a save point writes.\r\n") {
        Ok(()) => crate::savegate::set_armed(true, "its creation save is written: from now ONLY a Save at a save point writes (quitting before the first one returns to this moment)"),
        Err(e) => erlog!("save: the marker {} could not be written ({e}): the game goes on saving by itself", path.display()),
    }
}

/// (savegate) The marker of a character whose saving belongs to the save points.
fn marker(dir: &Path, key: &str) -> PathBuf {
    dir.join(format!("{key}.points"))
}

/// (savegate) THE SAVE POINT'S SAVE WAS WRITTEN by the game and by us: from now this character saves at save
/// points only (the marker file is made; nothing happens without `save.points_only`).
pub fn arm_points_only() {
    if !crate::savegate::mode_on() {
        return;
    }
    let Ok(guard) = STATE.lock() else { return };
    let Some(state) = guard.as_ref() else { return };
    let path = marker(&state.dir, &state.key);
    let _ = std::fs::create_dir_all(&state.dir);
    match std::fs::write(&path, "This character saves at save points only (kh-er save.points_only). Delete this file to give its saving back to Elden Ring.\r\n") {
        Ok(()) => crate::savegate::set_armed(true, "its first Save at a save point"),
        Err(e) => erlog!("save: the marker {} could not be written ({e}): the game goes on saving by itself", path.display()),
    }
}

/// (save) hook in `SoraHost::stop`: Sora stops with the player still there.
pub fn on_stop(sora: &Sora) {
    let Ok(mut guard) = STATE.lock() else { return };
    let Some(state) = guard.as_mut() else { return };
    if sora.status().hp > 0 {
        capture(state, sora);
    }
    if !points_only() {
        write(state, "Sora stopped");
    }
    state.live_at = None;
}

/// Every phase (called by `crate::hud::er_phase`'s hook line in frame.rs): Sora's frames stopped without a
/// `stop` (a death's loading screen, quit to the title): the latest state goes to the file.
pub fn er_phase() {
    let Ok(mut guard) = STATE.lock() else { return };
    let Some(state) = guard.as_mut() else { return };
    if state.live_at.is_some_and(|at| at.elapsed().as_secs_f32() > 1.0) {
        state.live_at = None;
        if !points_only() {
            write(state, "Sora went away");
        }
    }
}

/// (save) hook in entry.rs: the payload goes away.
pub fn shutdown() {
    if let Ok(mut guard) = STATE.lock() {
        if let Some(state) = guard.as_mut() {
            if !points_only() {
                write(state, "payload shutdown");
            }
        }
    }
}
