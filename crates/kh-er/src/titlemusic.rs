//! (TITLE-AUDIO) KH3'S TITLE MUSIC AND MENU SOUNDS ON ELDEN RING'S TITLE SCREEN. THE USER (2026-11-05): "replace elden
//! rings main menu music and sound effects with Kingdom Hearts'. So dearly beloved from KH3's main menu should replace the
//! main menu music", and "the main menu song should play in all of the sub menus accessed from the main menu, ending when the
//! game actually loads or a new game is started". State note: build/sora_tmp/weapons/TITLE-AUDIO/STATE.md.
//!
//! WHAT PLAYS: KH3's title HUD's own music (`TresHUDTitleBP.m_TitleBGM` = /Game/Sound/BGM/BGM_title, SEAD music Volume 0.7),
//! looped at KH3's loop points (the bank's: the whole stream, [0, 7377760) at 48 kHz), from the user's own install (the
//! extra audio root PACK/title/audio, tools/title_audio_pack.js), on OUR output (kh-audio), at the settings' Music level.
//!
//! WHEN (the game's own title flow, title.rs; pure rule [`Machine`]):
//! - it STARTS when the title appears (`CS::TitleStep` in BeginTitle or MenuJobWait) and plays on, never restarted, through
//!   every screen of the title's menu job: press any button, the menu, Continue / Load Game and its list (ours, KH3's Load
//!   screen), System (our settings screen), the game's own sub pages, Credits / Information;
//! - it ENDS (fades by KH3's BGM stop, `title.bgm_fade`, default 2.0 s) when a save actually loads (TitleStep PlayGame /
//!   GameStepWait: the loading screen into the world) or a new game starts (New Game confirmed: "Select Character Base" is
//!   made, the first screen of Elden Ring's new-game flow; or STEP_BeginNewGame); Elden Ring's own sound comes back there;
//! - it starts again only when the title's menu is made anew after an end (back on the title from the game or the creator).
//!
//! ELDEN RING'S SIDE ON THE TITLE: its whole mix (its title music, any music of a sub menu, its menu sounds) is silenced by
//! the game's own master-volume control (gosound.rs): CSSound's factor +0x34C, written by nothing but its constructor
//! (2.7.1.0 and 2.7.2.0), set to 0 and sent by the game's own setter; put back exactly at the end and at the payload's stop.
//! Its menu sounds are still HEARD BY US: the game's sound EVENT NAME builder ("Play_%s" of a `{type, id}`, which every Wwise
//! play of an id goes through; its menus' sounds are type 'f') is detoured, and so is its sound-instance maker (logged only:
//! the title's menu sounds do not go through it, SEEN); while the title plays, each `{type, id}` asked for is mapped to
//! KH3's counterpart ([`SE_MAP`]: move -> KH3 cursor, confirm -> decide, back / NO -> cancel, a message window -> KH3's
//! window open) and played through our menus' path (fx.rs `ui_sounds`), except under our own KH3 screens (they play
//! KH3's sounds themselves). Nothing of the game's memory but that one factor is written; in game nothing changes.
//!
//! KEYS: `title.music = 0` (all of it off: Elden Ring's title as before), `title.bgm_fade` (s), `title.trace = 1` (every
//! sound the game asks for while the title plays is logged, mapped or not).

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use crate::control::Control;
use crate::erlog;
use crate::havok::{locate, Located};

/// KH3's title music (the title HUD's `m_TitleBGM`), the asset of the extra audio root PACK/title/audio.
pub const TITLE_BGM_ASSET: &str = "/Game/Sound/BGM/BGM_title";
/// Its length = its loop end (KH3's loop: the whole stream, 7377760 samples at 48 kHz).
pub const TITLE_BGM_SECONDS: f64 = 7_377_760.0 / 48_000.0;
// ------------------------------------------------------------------------------------------ (TITLE-AUDIO) the user's own track
//
// YOUR OWN TITLE TRACK (STATE.md section 7): a track the user supplies is played instead of KH3's title music when it is there
// (nothing is downloaded or bundled):
// `<PACK>/title/audio/custom/title.wav` (PCM 16-bit WAV, mono or stereo, any rate), looped at `title.custom_loop = start,end`
// (seconds, kh-er.ini; read at the payload's start), else at the WAV's own smpl loop, else the whole file. Without the file
// KH3's title music plays as before. Everything else (when it plays, its end, Master x Music) is the same.

/// The user's own title track, relative to the pack folder.
pub const CUSTOM_FILE: &str = "title/audio/custom/title.wav";
/// Its asset in the merged pack.
pub const CUSTOM_ASSET: &str = "/Game/Sound/BGM/BGM_title_custom";
/// The user's own file is the title music (set by the audio loader).
static CUSTOM_ON: AtomicBool = AtomicBool::new(false);
/// The title track's first loop end and its loop length, s (f64 bits; KH3's: the whole 153.703 s stream).
static LOOP_FIRST: AtomicU64 = AtomicU64::new(0);
static LOOP_PERIOD: AtomicU64 = AtomicU64::new(0);
/// `title.custom_loop` (start, end s; end 0 = the file's end), read at the payload's start.
static CUSTOM_LOOP: Mutex<Option<(f64, f64)>> = Mutex::new(None);

/// The asset the title plays: the user's own track when it was found, else KH3's.
pub fn title_asset() -> &'static str {
    if CUSTOM_ON.load(Ordering::Relaxed) {
        CUSTOM_ASSET
    } else {
        TITLE_BGM_ASSET
    }
}

/// (frame.rs `init`, before the audio side starts) `title.custom_loop = start,end` in seconds.
pub fn configure(control: &Control) {
    let parsed = control.str("title.custom_loop").and_then(parse_loop);
    if let Ok(mut l) = CUSTOM_LOOP.lock() {
        *l = parsed;
    }
}

/// `"start,end"` (seconds; end 0 or left out = the file's end) -> (start, end).
pub fn parse_loop(text: &str) -> Option<(f64, f64)> {
    let mut parts = text.split(',').map(|p| p.trim().parse::<f64>());
    let start = parts.next()?.ok()?;
    let end = match parts.next() {
        Some(v) => v.ok()?,
        None => 0.0,
    };
    (start.is_finite() && end.is_finite() && start >= 0.0 && end >= 0.0).then_some((start, end))
}

/// The loop of the user's track in frames, [start, end): the setting (seconds) when it is valid for the file, else the
/// WAV's own smpl loop, else the whole file.
pub fn loop_frames(setting: Option<(f64, f64)>, wav_loop: Option<(u32, u32)>, frames: u32, rate: u32) -> (u32, u32) {
    if let Some((start, end)) = setting {
        let at = |s: f64| (s * f64::from(rate)).round().min(f64::from(frames)) as u32;
        let (a, b) = (at(start), if end > 0.0 { at(end) } else { frames });
        if a < b {
            return (a, b);
        }
    }
    match wav_loop {
        Some((a, b)) if a < b && b <= frames => (a, b),
        _ => (0, frames),
    }
}

/// The extra root's manifest for the user's track (the same shape tools/title_audio_pack.js writes; its material file is
/// `custom/title.wav` under PACK/title/audio). Asset volume 0.7 = KH3's title music's own.
pub fn custom_manifest(channels: u16, rate: u32, frames: u32, (start, end): (u32, u32)) -> String {
    let seconds = f64::from(frames) / f64::from(rate.max(1));
    serde_json::json!({
        "format": "kh3-sora-audio-pack/1",
        "generator": "kh-er titlemusic.rs (TITLE-AUDIO): the user's own title track",
        "config": { "categories": [{ "number": 1, "name": "BGM", "max_sounds": 4, "shares_pool_of": null, "used": true }] },
        "banks": { "BGM/BGM_title_custom": {
            "bank_number": 0,
            "sounds": { "0": {
                "index": 0, "number": 0, "category": { "number": 1, "name": "BGM" }, "priority": 0, "volume": 1.0,
                "main_bus": { "number": 0, "name": "BGM" }, "aux_sends": [], "flags": 0, "macro_start": 0, "macro_stop": 0, "release_seconds": 0.0,
                "selection": { "rule": "all", "start": [0] },
                "sequences": [{ "sequence": 0, "number": 0, "commands": [{ "name": "keyOn", "track": 0, "loopFlag": 1 }, { "name": "end" }] }],
                "tracks": { "0": { "type": "MaterialTrack", "commands": [{ "name": "end" }], "material": 0 } }
            } },
            "materials": { "0": { "file": "custom/title.wav", "channels": channels, "sample_rate": rate, "samples": frames, "seconds": seconds, "loop": { "start": start, "end": end } } }
        } },
        "sound_assets": { CUSTOM_ASSET: { "bank": "BGM/BGM_title_custom", "sound_index": 0, "volume": 0.7, "duration": seconds, "looping": true, "attenuation": null, "other_props": {} } },
        "triggers": []
    })
    .to_string()
}

/// (fx.rs, the audio loader thread) The user's own track: `Ok(None)` = no file (KH3's track plays); `Ok(Some((manifest,
/// words)))` = merge `manifest` as the root PACK/title/audio; `Err` = the file is there but cannot be used (KH3's plays).
pub fn custom_root(pack_dir: &std::path::Path) -> Result<Option<(String, String)>, String> {
    let path = pack_dir.join(CUSTOM_FILE);
    if !path.is_file() {
        return Ok(None);
    }
    let wav = kh_audio::wav::read(&path)?;
    if !(1..=2).contains(&wav.channels) || wav.sample_rate == 0 || wav.frames() == 0 {
        return Err(format!("{}: {} channels at {} Hz, {} frames (mono or stereo PCM 16-bit is needed)", path.display(), wav.channels, wav.sample_rate, wav.frames()));
    }
    let frames = wav.frames() as u32;
    let setting = CUSTOM_LOOP.lock().ok().and_then(|l| *l);
    let (start, end) = loop_frames(setting, wav.loop_points, frames, wav.sample_rate);
    let rate = f64::from(wav.sample_rate);
    let from = if setting.is_some() { "title.custom_loop" } else if wav.loop_points.is_some() { "the WAV's smpl loop" } else { "the whole file" };
    let words = format!("{} ({} ch, {} Hz, {:.1} s), looped at [{:.3}, {:.3}) s ({from})", path.display(), wav.channels, wav.sample_rate, f64::from(frames) / rate, f64::from(start) / rate, f64::from(end) / rate);
    Ok(Some((custom_manifest(wav.channels, wav.sample_rate, frames, (start, end)), words)))
}

/// (fx.rs) The title track in force and its loop (first end, period) in seconds.
pub fn set_title_track(custom: bool, first_end: f64, period: f64) {
    CUSTOM_ON.store(custom, Ordering::Relaxed);
    LOOP_FIRST.store(first_end.to_bits(), Ordering::Relaxed);
    LOOP_PERIOD.store(period.to_bits(), Ordering::Relaxed);
}

/// How many times the track has passed its loop end after `elapsed` s (told 1 s late: a track that was not looped is dead).
pub fn loop_passes(elapsed: f64, first_end: f64, period: f64) -> u64 {
    let t = elapsed - 1.0;
    if first_end <= 0.0 || t < first_end {
        return 0;
    }
    1 + if period > 0.0 { ((t - first_end) / period) as u64 } else { 0 }
}

/// KH3's own BGM fade on a stop: `USQEXSEADStatics::StopAllBGM_OptionalFade` (0x14586F500 -> 0x1458221C0) as KH3's game over
/// calls it (`0x140EAD210`: 2.0 s). The title's own end in KH3 is a level travel with no fade value of its own in the data.
pub const KH3_BGM_FADE: f32 = 2.0;

// ------------------------------------------------------------------------------------------ the title phase (pure)

/// How the title's music ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    /// A save loads into the world (TitleStep PlayGame / GameStepWait).
    Load,
    /// New Game was confirmed: "Select Character Base" was made (or STEP_BeginNewGame ran).
    NewGame,
    /// The title step left the title for another reason (its logo / init states).
    Left,
}

impl End {
    pub fn words(self) -> &'static str {
        match self {
            End::Load => "a save loads into the world (TitleStep PlayGame / GameStepWait: the loading screen)",
            End::NewGame => "a new game starts (New Game confirmed: Elden Ring's \"Select Character Base\" is made)",
            End::Left => "the title step left the title (not a load, not a new game)",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Phase {
    /// No title music yet in this run (or the feature is off).
    #[default]
    Off,
    Playing,
    Ended(End),
}

/// What the title's flow says this frame (title.rs; ticks in ms, 0 = never).
#[derive(Clone, Copy, Debug, Default)]
pub struct Signals {
    /// `CS::TitleStep`'s state as last set (-1: none seen).
    pub state: i32,
    /// When the title's menu (TitleTopDialog) was last made.
    pub top_at: u64,
    /// When "Select Character Base" (the new-game creation's first screen) was last made.
    pub base_at: u64,
    pub now: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Start,
    Stop(End),
}

/// THE RULE of the title's music: started on the title, kept through every screen of its menu job, ended by a load or a new
/// game, started again only by a title menu made after that end.
#[derive(Clone, Copy, Debug, Default)]
pub struct Machine {
    pub phase: Phase,
    started_at: u64,
    ended_at: u64,
}

const ST_BEGIN_TITLE: i32 = 3;
const ST_PLAY_GAME: i32 = 5;

impl Machine {
    pub fn frame(&mut self, s: Signals) -> Option<Act> {
        // THE TITLE APPEARS when its menu (TitleTopDialog: "PRESS ANY BUTTON", then CONTINUE / LOAD GAME / ...) is made: SEEN
        // 2026-11-05, the step runs InitMenu -> MenuJobWait (the boot's notices, 3.5 s) -> BeginLogo -> MenuJobWait + the
        // dialog made in the same frame. (-1: a payload whose hooks came after the step's last state change.)
        let title = s.state == ST_BEGIN_TITLE || s.state == crate::title::ST_MENU_JOB_WAIT || s.state == -1;
        let load = s.state == ST_PLAY_GAME || s.state == crate::title::ST_GAME_STEP_WAIT;
        match self.phase {
            Phase::Off if title && s.top_at != 0 => self.start(s.now),
            Phase::Ended(_) if title && s.top_at > self.ended_at => self.start(s.now),
            Phase::Playing => {
                let end = if load {
                    End::Load
                } else if s.state == crate::title::ST_BEGIN_NEW_GAME || s.base_at > self.started_at {
                    End::NewGame
                } else if !title {
                    End::Left
                } else {
                    return None;
                };
                self.phase = Phase::Ended(end);
                self.ended_at = s.now;
                Some(Act::Stop(end))
            }
            _ => None,
        }
    }

    fn start(&mut self, now: u64) -> Option<Act> {
        self.phase = Phase::Playing;
        self.started_at = now;
        Some(Act::Start)
    }

    /// The feature was switched off (or the payload stops) while playing: ended at once.
    pub fn release(&mut self, now: u64) -> Option<Act> {
        (self.phase == Phase::Playing).then(|| {
            self.phase = Phase::Ended(End::Left);
            self.ended_at = now;
            Act::Stop(End::Left)
        })
    }
}

// ------------------------------------------------------------------------------------------ the sound map (pure)

/// KH3's system SEs (`UTresUIParts::PlaySystemSE` 0x14176F7D0 -> the static SE table TresStaticSE, the pack's ui_sounds.json):
/// the menus' own sounds of KH3's title and camp menus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KhSe {
    /// Click (system SE 0 -> static 0, se00000_001): the cursor moves.
    Cursor,
    /// Decide (1 -> static 1, se00000_002): a choice is confirmed.
    Decide,
    /// Cancel (2 -> static 3, se00000_004): back / close.
    Cancel,
    // (KH3's Beep, system SE 3 -> static 4 se00000_005, refused / error: no title sound of Elden Ring's asks for it; none of its
    // title choices is refused in the runs, so it has no row)
    /// Tab / GameHelpOpen (4, 5 -> static 2, se00000_003): a tab changes, a window opens.
    Open,
}

impl KhSe {
    /// The static SE's asset name (the HUD's own name for it: kh-host hud.rs `se_names`).
    pub fn name(self) -> &'static str {
        match self {
            KhSe::Cursor => "se00000_001",
            KhSe::Decide => "se00000_002",
            KhSe::Open => "se00000_003",
            KhSe::Cancel => "se00000_004",
        }
    }

    /// Its index in KH3's static SE table.
    pub fn static_index(self) -> u32 {
        match self {
            KhSe::Cursor => 0,
            KhSe::Decide => 1,
            KhSe::Open => 2,
            KhSe::Cancel => 3,
        }
    }
}

/// One of Elden Ring's title-menu sounds and its KH3 counterpart: `{type, id}` as the game asks its sound maker for it (the
/// type is the index into the executable's letter table `acfopsmvxbiyzegdw`).
#[derive(Clone, Copy, Debug)]
pub struct SeRow {
    pub kind: i32,
    pub id: i32,
    pub what: &'static str,
    pub kh: KhSe,
}

/// THE MAP: every sound Elden Ring's title and its menus were SEEN asking for (in game, `title.trace = 1`, STATE.md), with
/// KH3's counterpart.
pub const SE_MAP: &[SeRow] = &[
    // SEEN 2026-11-05 (run 2, TITLE-AUDIO/run2.kh-er.log): "PRESS ANY BUTTON" pressed
    SeRow { kind: 2, id: 999_999_999, what: "PRESS ANY BUTTON is pressed", kh: KhSe::Decide },
    // the title list's cursor (each up / down), a dialog's choices
    SeRow { kind: 2, id: 0, what: "the cursor moves", kh: KhSe::Cursor },
    // the notice's OK, QUIT GAME
    SeRow { kind: 2, id: 100, what: "a choice is confirmed", kh: KhSe::Decide },
    // LOAD GAME chosen: its list's window opens (Elden Ring plays this one alone for the choice; KH3's title plays Decide)
    SeRow { kind: 2, id: 1000, what: "a menu window opens on a choice", kh: KhSe::Decide },
    // the notice window comes up by itself (the previous session's warning)
    SeRow { kind: 2, id: 1010, what: "a message window opens", kh: KhSe::Open },
    // the Load Game list closed by Back
    SeRow { kind: 2, id: 1020, what: "a menu window closes (Back)", kh: KhSe::Cancel },
    // SEEN (run 3): NO chosen in a question ("Cannot create any more profiles ... YES / NO"), the Load list's question declined
    SeRow { kind: 2, id: 200, what: "a question is declined (NO / back)", kh: KhSe::Cancel },
];

/// The letter of a sound type (the executable's table `acfopsmvxbiyzegdw`).
pub fn letter(kind: i32) -> char {
    b"acfopsmvxbiyzegdw".get(kind as usize).map_or('?', |b| *b as char)
}

/// KH3's sound for one of Elden Ring's (None: not one of its title-menu sounds).
pub fn kh_for(kind: i32, id: i32) -> Option<&'static SeRow> {
    SE_MAP.iter().find(|r| r.kind == kind && r.id == id)
}

// (the music's level: Master on the output x Music on its BGM category's config layer, fx.rs `apply_levels` / `category_level`:
// the Sound Effects row never reaches it)

// ------------------------------------------------------------------------------------------ the game's sound maker

/// The game's sound-instance maker `make_instance(CSSound, &{type, id}, extra) -> SoundIns*`: the one function its PlaySE
/// (`play_at` 0xDA7350), its 2D play (0xDA73F0, 28 callers) and its event player (0xDA7AA0) call. Read in both executables:
/// 0xDAD650 (2.7.1.0) / 0xDAED40 (2.7.2.0), found from inside `play_at` (capture 1: the call's target).
const PAT_MAKE: &str = "c7 44 24 20 00 00 00 00 45 33 c0 49 8b d1 e8 $ { ' } 48 8b d8 48 85 c0 74 08 48 8b c8 e8";

/// The game's sound EVENT NAME builder `play_name(&{type, id}, &out string)`: "Play_%s" of the id's text ("c800007500"),
/// the name every Wwise play of an id is posted by (16 callers: the sound instances, the engine's 2D / menu plays). Its body is
/// one template shared with the "Stop_%s" and "Dialogue" builders: the pattern tells it by the format string it loads.
/// 0x397DF0 in both executables.
const PAT_PLAY_NAME: &str = "40 55 56 57 48 8d 6c 24 b9 48 81 ec a0 00 00 00 48 c7 45 cf fe ff ff ff [159] 48 8d 15 $ { \"Play_%s\" 00 }";

pub(crate) const LOCATED: [Located; 2] = [
    Located { name: "sound instance maker (TITLE-AUDIO)", pattern: PAT_MAKE, capture: 1, rva_2_7_1_0: 0x00da_d650, rva_2_7_2_0: 0x00da_ed40 },
    Located { name: "sound event name builder Play_%s (TITLE-AUDIO)", pattern: PAT_PLAY_NAME, capture: 0, rva_2_7_1_0: 0x0039_7df0, rva_2_7_2_0: 0x0039_7df0 },
];

type MakeFn = unsafe extern "C" fn(usize, *const [i32; 2], usize, usize, usize, usize) -> usize;
type NameFn = unsafe extern "C" fn(*const [i32; 2], usize, usize, usize) -> usize;

static ORIG_MAKE: AtomicUsize = AtomicUsize::new(0);
static ORIG_NAME: AtomicUsize = AtomicUsize::new(0);

unsafe extern "C" fn name_hook(sid: *const [i32; 2], out: usize, a3: usize, a4: usize) -> usize {
    let original: NameFn = std::mem::transmute(ORIG_NAME.load(Ordering::Relaxed));
    take(sid, 1);
    original(sid, out, a3, a4)
}

/// The two places an ask is taken at, for the log.
const VIA: [&str; 2] = ["instance maker", "Play_ event"];

/// One ask of the game's (`via` 0: the instance maker, 1: the event name builder), kept while the title plays.
unsafe fn take(sid: *const [i32; 2], via: i32) {
    if (ARMED.load(Ordering::Relaxed) || LISTEN_UNTIL.load(Ordering::Relaxed) > now()) && !sid.is_null() {
        let [kind, id] = std::ptr::read_unaligned(sid);
        if let Ok(mut asked) = ASKED.try_lock() {
            if asked.len() < 64 {
                asked.push((kind, id, via));
            }
        }
    }
}
/// The title music plays: the game's sound asks are taken.
static ARMED: AtomicBool = AtomicBool::new(false);
/// Until this tick the asks are only logged (after a new game's start: what Elden Ring plays in its creator).
static LISTEN_UNTIL: AtomicU64 = AtomicU64::new(0);
static ASKED: Mutex<Vec<(i32, i32, i32)>> = Mutex::new(Vec::new());

unsafe extern "C" fn make_hook(sound: usize, sid: *const [i32; 2], extra: usize, a4: usize, a5: usize, a6: usize) -> usize {
    let original: MakeFn = std::mem::transmute(ORIG_MAKE.load(Ordering::Relaxed));
    take(sid, 0);
    original(sound, sid, extra, a4, a5, a6)
}

/// Detours the sound maker and the event name builder (once per payload; undone by the renderer's shutdown with every other
/// inline hook).
pub fn install() {
    let hooks: [(&Located, usize, &AtomicUsize, &'static str); 2] = [(&LOCATED[0], make_hook as *const () as usize, &ORIG_MAKE, "sound instance maker"), (&LOCATED[1], name_hook as *const () as usize, &ORIG_NAME, "sound event name builder")];
    for (item, hook, original, what) in hooks {
        let Some(at) = locate(item) else {
            erlog!("titlemusic: {} is not located: Elden Ring's title sounds through it are not mapped (its mix is still silenced on the title)", item.name);
            continue;
        };
        // SAFETY: the function's start at its located address (pattern + recorded RVA); our hook has its signature.
        match unsafe { kh_er_render::inline_hook(at, hook, original, what) } {
            Ok(()) => erlog!("titlemusic: (TITLE-AUDIO) {} is detoured (Elden Ring's title-menu sounds -> KH3's)", item.name),
            Err(e) => {
                original.store(0, Ordering::SeqCst);
                erlog!("titlemusic: {} was NOT detoured ({e})", item.name);
            }
        }
    }
}

pub fn uninstalled() {
    ORIG_MAKE.store(0, Ordering::SeqCst);
    ORIG_NAME.store(0, Ordering::SeqCst);
    ARMED.store(false, Ordering::SeqCst);
}

fn now() -> u64 {
    // SAFETY: plain getter.
    unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() }
}

// ------------------------------------------------------------------------------------------ the frame

struct State {
    machine: Machine,
    /// Elden Ring's factor before our silence (and the CSSound it was on).
    er_before: Option<(usize, f32)>,
    started: Option<Instant>,
    loops_told: u64,
    seen: Vec<(i32, i32, i32)>,
    /// The music's stop fade still running (the engine is pumped until it is over).
    stopping_until: Option<Instant>,
    told_off: bool,
    /// When one of our KH3 screens was last seen up on the title (ms tick).
    kh_screen_at: u64,
    /// (CC-MUTE) Elden Ring's mix is silent for the new game's creation (ngquiet.rs): the title's silence kept past "a new game
    /// starts", or put on with the title music off; given back when ngquiet.rs lets go.
    held: bool,
}

static STATE: Mutex<State> = Mutex::new(State { machine: Machine { phase: Phase::Off, started_at: 0, ended_at: 0 }, er_before: None, started: None, loops_told: 0, seen: Vec::new(), stopping_until: None, told_off: false, kh_screen_at: 0, held: false });

/// One of our KH3 screens is up on the title, or was up / was being raised (the Load list's factory) within 0.6 s.
fn kh_screen_near(state: &mut State, now: u64) -> bool {
    if crate::hud::title_mode_on() {
        state.kh_screen_at = now;
    }
    let near = |at: u64| at != 0 && now.saturating_sub(at) < 600;
    near(state.kh_screen_at) || near(crate::title::load_made_at()) || near(crate::title::delete_made_at())
}

fn fade_of(control: &Control) -> f32 {
    control.f32("title.bgm_fade").filter(|v| v.is_finite() && (0.0..=10.0).contains(v)).unwrap_or(KH3_BGM_FADE)
}

fn er_mix(state: &mut State, silent: bool, fade: f32) -> String {
    if silent {
        match crate::gosound::title_factor(0.0, fade) {
            Some((sound, before)) => {
                state.er_before = Some((sound, before));
                format!("Elden Ring's whole mix silenced over {fade:.2} s by its own master volume (CSSound + 0x34C {before:.2} -> 0; {})", crate::gosound::factors_now())
            }
            None => "Elden Ring's master volume is NOT located / made: its title music is NOT silenced".to_string(),
        }
    } else {
        match state.er_before.take() {
            Some((sound, before)) if crate::gosound::sound_now() == sound => {
                let _ = crate::gosound::title_factor(before, fade);
                format!("Elden Ring's mix given back over {fade:.2} s (CSSound + 0x34C back to {before:.2}; {})", crate::gosound::factors_now())
            }
            Some(_) => "CSSound was made anew (its factors are 1.0 again): nothing to give back".to_string(),
            None => "Elden Ring's mix was not silenced".to_string(),
        }
    }
}

/// (frame.rs, FRAME_BEGIN, every frame: the title and loading screens too; after every title-mode HUD frame)
pub fn frame(control: &Control, root: &std::path::Path, dt: f32) {
    static PACK_DIR: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    let pack_dir = PACK_DIR.get_or_init(|| crate::sora::data_paths(root).0).as_path();
    let Ok(mut state) = STATE.lock() else { return };
    let on = control.flag("title.music") != Some(false);
    let fade = fade_of(control);
    let vol_db = control.f32("snd.vol").unwrap_or(-6.0);
    let t = now();
    let act = if on {
        let signals = Signals { state: crate::title::state(), top_at: crate::title::top_made_at(), base_at: crate::title::base_made_at(), now: t };
        state.machine.frame(signals)
    } else {
        if !state.told_off {
            state.told_off = true;
            erlog!("titlemusic: title.music = 0: Elden Ring's own title music and sounds");
        }
        state.machine.release(t)
    };
    match act {
        Some(Act::Start) => {
            // (CC-MUTE) back on the title from the creation: Elden Ring's mix is still silent (kept for it): not taken again (its
            // value before is the one to give back)
            state.held = false;
            let er = if state.er_before.is_some() { "Elden Ring's mix is already silent (kept through the new game's creation)".to_string() } else { er_mix(&mut state, true, 0.3) };
            state.started = None;
            state.loops_told = 0;
            ARMED.store(true, Ordering::Relaxed);
            erlog!("titlemusic: THE TITLE APPEARS (TitleStep {}): the title music ({}) is asked; {er}", crate::title::state(), title_asset());
        }
        Some(Act::Stop(end)) => {
            ARMED.store(false, Ordering::Relaxed);
            if end == End::NewGame {
                LISTEN_UNTIL.store(t + 30_000, Ordering::Relaxed);
            }
            let fade_out = if on { fade } else { 0.5 };
            // (CC-MUTE) a new game: Elden Ring's creation is silent (ngquiet.rs): its mix stays off, and our music is gone within
            // the creation's own mute fade (the audio host's silence covers the rest)
            let hold = crate::ngquiet::er_wanted();
            let fade_out = if hold && end == End::NewGame { fade_out.min(crate::ngquiet::kh_fade()) } else { fade_out };
            let er = if hold && state.er_before.is_some() {
                state.held = true;
                "Elden Ring's mix STAYS silent: its character creation is muted until our first KH new-game pop-up (ngquiet.rs)".to_string()
            } else {
                er_mix(&mut state, false, 1.0)
            };
            let played = state.started.map_or(0.0, |s| s.elapsed().as_secs_f64());
            crate::fx::title_music(pack_dir, dt, vol_db, None, fade_out);
            state.stopping_until = Some(Instant::now() + std::time::Duration::from_secs_f32(fade_out + 0.5));
            erlog!("titlemusic: THE TITLE MUSIC ENDS: {} (TitleStep {}): KH3's music fades out over {fade_out:.2} s after {played:.1} s of play; {er}", end.words(), crate::title::state());
        }
        None => {}
    }
    // (CC-MUTE) THE NEW GAME'S CREATION outside the title's play: Elden Ring's mix silent while ngquiet.rs holds it
    if state.machine.phase != Phase::Playing {
        let hold = crate::ngquiet::er_wanted();
        if hold && !state.held {
            state.held = true;
            if state.er_before.is_none() {
                let er = er_mix(&mut state, true, crate::ngquiet::kh_fade());
                erlog!("titlemusic: (CC-MUTE) Elden Ring's character creation is muted: {er}");
            }
        } else if !hold && state.held {
            state.held = false;
            let er = er_mix(&mut state, false, 1.0);
            erlog!("titlemusic: (CC-MUTE) the creation's silence ends: {er}");
        }
    }
    // the game's asks of this frame (mapped while the title plays; logged after a new game's start)
    let asked: Vec<(i32, i32, i32)> = ASKED.try_lock().map(|mut a| std::mem::take(&mut *a)).unwrap_or_default();
    let playing = state.machine.phase == Phase::Playing;
    let trace = control.on("title.trace");
    let mut kh = Vec::new();
    // (under one of our KH3 screens, and 0.6 s around it, that screen plays KH3's own sounds: the game's are not doubled)
    let ours = kh_screen_near(&mut state, t);
    for (kind, id, via) in asked {
        let row = kh_for(kind, id);
        let new = !state.seen.contains(&(kind, id, via));
        if new && state.seen.len() < 256 {
            state.seen.push((kind, id, via));
        }
        // (one KH3 sound per play: the event name builder runs once for every Wwise post of an id; the instance maker's asks
        // are logged only)
        if playing && via == 1 && !ours {
            if let Some(row) = row {
                kh.push(row.kh);
            }
        }
        if new || trace {
            let place = if playing { "on the title" } else { "after the title's end (Elden Ring's own, heard)" };
            match row {
                Some(r) if playing && ours => erlog!("titlemusic: Elden Ring's sound {}{id:09} ({}) {place} [{}] under our KH3 screen: its own KH3 sound plays (not doubled); Elden Ring's own is silent", letter(kind), r.what, VIA[via as usize & 1]),
                Some(r) if playing => erlog!("titlemusic: Elden Ring's sound {}{id:09} ({}) {place} [{}] -> KH3's {} (static SE {}); Elden Ring's own is silent", letter(kind), r.what, VIA[via as usize & 1], r.kh.name(), r.kh.static_index()),
                _ => erlog!("titlemusic: Elden Ring's sound {}{id:09} (type {kind}) asked {place} [{}]{}", letter(kind), VIA[via as usize & 1], if playing { ": NOT IN THE MAP (silent: Elden Ring's mix is off)" } else { "" }),
            }
        }
    }
    if playing {
        let rows = crate::eroptions::sound_rows();
        match crate::fx::title_music(pack_dir, dt, vol_db, Some(1.0), fade) {
            Some(true) => {
                let started = *state.started.get_or_insert_with(Instant::now);
                // (told 1 s past each loop end: a sound that was not looped has ended by then and is not live)
                let (first, period) = (f64::from_bits(LOOP_FIRST.load(Ordering::Relaxed)), f64::from_bits(LOOP_PERIOD.load(Ordering::Relaxed)));
                let (first, period) = if first > 0.0 { (first, period) } else { (TITLE_BGM_SECONDS, TITLE_BGM_SECONDS) };
                let passes = loop_passes(started.elapsed().as_secs_f64(), first, period);
                if state.loops_told == 0 && passes == 0 {
                    state.loops_told = 1;
                    erlog!("titlemusic: the title music PLAYS ({}: its loop ends at {first:.3} s, then every {period:.3} s; Master {} x Music {} on the BGM category (Sound Effects {} and Voice {} are the other categories'))", if CUSTOM_ON.load(Ordering::Relaxed) { "THE USER'S OWN TRACK, PACK/title/audio/custom/title.wav" } else { "KH3's, looped at KH3's loop points [0, 7377760) of 48 kHz" }, rows.master, rows.music, rows.sfx, rows.voice);
                } else if passes + 1 > state.loops_told {
                    state.loops_told = passes + 1;
                    erlog!("titlemusic: the title music LOOPED at its loop end ({:.1} s since its start: pass {}), still playing", started.elapsed().as_secs_f64(), passes + 1);
                }
            }
            Some(false) => {}
            None => {}
        }
        if !kh.is_empty() {
            crate::fx::ui_sounds(kh.into_iter().map(ui_play).collect());
        }
    } else if state.stopping_until.is_some_and(|until| Instant::now() < until) {
        crate::fx::title_music(pack_dir, dt, vol_db, None, fade);
    } else if state.stopping_until.take().is_some() {
        erlog!("titlemusic: KH3's title music has stopped ({})", if crate::fx::title_music(pack_dir, dt, vol_db, None, fade) == Some(true) { "STILL LIVE" } else { "no voice left" });
    }
}

/// One of KH3's menu sounds as the HUD asks for it (kh-host hud.rs: a 2D static SE by its name).
fn ui_play(se: KhSe) -> kh_host::seam::AudioCommand {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let handle = 0x7A00_0000 + (NEXT.fetch_add(1, Ordering::Relaxed) as u32 & 0xFFFF);
    let play = kh_core::SoundPlay { id: kh_core::SoundId(kh_host::hud::UI_SOUND_IDS + se.static_index() + 1), name: se.name(), at: kh_core::Attach::SORA, volume: 1.0, pitch: 1.0, looping: false, spatial: false };
    kh_host::seam::AudioCommand::Play(kh_core::SoundHandle(handle), play)
}

/// (entry.rs, the payload's stop) Elden Ring's mix is given back at once.
pub fn shutdown() {
    ARMED.store(false, Ordering::SeqCst);
    let Ok(mut state) = STATE.lock() else { return };
    if state.er_before.is_some() {
        let er = er_mix(&mut state, false, 0.0);
        erlog!("titlemusic: the payload stops: {er}");
    }
    state.held = false;
    state.machine = Machine::default();
}

#[cfg(test)]
mod tests {
    use super::*;

    const BEGIN_TITLE: i32 = 3;
    const NEW_GAME: i32 = 4;
    const PLAY_GAME: i32 = 5;
    const STEP_WAIT: i32 = 6;
    const MENU: i32 = 10;

    fn at(state: i32, top_at: u64, base_at: u64, now: u64) -> Signals {
        Signals { state, top_at, base_at, now }
    }

    /// (TITLE-AUDIO) the title-only rule: the music starts with the title, plays through every sub menu (the title step stays
    /// in its menu job; dialogs made and closed: Load Game, System, Credits), never restarts on a back-out, and ends on a load
    #[test]
    fn ta_the_title_music_plays_through_the_sub_menus_and_ends_on_a_load() {
        let mut m = Machine::default();
        // the boot as SEEN: InitMenu, MenuJobWait (the boot's notices), BeginLogo: nothing yet
        for (state, k) in [(0, 50), (MENU, 60), (2, 100), (BEGIN_TITLE, 150)] {
            assert_eq!(m.frame(at(state, 0, 0, k)), None);
        }
        assert_eq!(m.phase, Phase::Off);
        // the title appears: its menu is made ("PRESS ANY BUTTON")
        assert_eq!(m.frame(at(MENU, 200, 0, 200)), Some(Act::Start));
        // press any button -> the menu (made), Load Game's list made, System, back out to the menu (made again): no restart
        for (k, top) in [(300, 300), (400, 300), (500, 300), (600, 600), (700, 600)] {
            assert_eq!(m.frame(at(MENU, top, 0, k)), None, "frame {k}");
            assert_eq!(m.phase, Phase::Playing);
        }
        // a save loads: PlayGame
        assert_eq!(m.frame(at(PLAY_GAME, 600, 0, 800)), Some(Act::Stop(End::Load)));
        // the world: nothing
        assert_eq!(m.frame(at(STEP_WAIT, 600, 0, 900)), None);
        // back on the title from the game (Quit Game): the title step's menu again, its menu made anew: it starts again
        assert_eq!(m.frame(at(BEGIN_TITLE, 600, 0, 1000)), None, "not before the menu is made anew");
        assert_eq!(m.frame(at(MENU, 1100, 0, 1100)), Some(Act::Start));
    }

    /// (TITLE-AUDIO) New Game: the creation's first screen ends it (the title step is still in its menu job), and the music does
    /// not come back over the creator; backing out of the creator to the title's menu starts it again
    #[test]
    fn ta_a_new_game_ends_the_title_music_and_the_creator_is_elden_rings() {
        let mut m = Machine::default();
        assert_eq!(m.frame(at(MENU, 100, 0, 100)), Some(Act::Start));
        // an old creation's screen (before this start) does not count
        let mut m2 = Machine::default();
        assert_eq!(m2.frame(at(MENU, 100, 50, 100)), Some(Act::Start));
        assert_eq!(m2.frame(at(MENU, 100, 50, 150)), None);
        // New Game confirmed: "Select Character Base" made
        assert_eq!(m.frame(at(MENU, 100, 300, 300)), Some(Act::Stop(End::NewGame)));
        for k in [400, 500, 600] {
            assert_eq!(m.frame(at(MENU, 100, 300, k)), None, "the creator is Elden Ring's");
        }
        // STEP_BeginNewGame after the creation: still nothing
        assert_eq!(m.frame(at(NEW_GAME, 100, 300, 700)), None);
        // the creator left back to the title (its menu made anew)
        let mut m3 = Machine::default();
        m3.frame(at(MENU, 100, 0, 100));
        m3.frame(at(MENU, 100, 300, 300));
        assert_eq!(m3.frame(at(MENU, 400, 300, 400)), Some(Act::Start));
        // STEP_BeginNewGame alone also ends it (the factory not seen)
        let mut m4 = Machine::default();
        m4.frame(at(MENU, 100, 0, 100));
        assert_eq!(m4.frame(at(NEW_GAME, 100, 0, 200)), Some(Act::Stop(End::NewGame)));
    }

    /// (TITLE-AUDIO) in game nothing happens: a payload whose title hooks saw nothing (-1) or a world (GameStepWait) never starts it
    #[test]
    fn ta_never_in_game() {
        let mut m = Machine::default();
        for (state, k) in [(-1, 100), (STEP_WAIT, 200), (PLAY_GAME, 300), (7, 400), (8, 500)] {
            assert_eq!(m.frame(at(state, 0, 0, k)), None);
        }
        assert_eq!(m.phase, Phase::Off);
        // a payload that came after the title step's last state change: the title menu being made starts it
        let mut late = Machine::default();
        assert_eq!(late.frame(at(-1, 0, 0, 100)), None);
        assert_eq!(late.frame(at(-1, 150, 0, 200)), Some(Act::Start));
        // switched off while playing: ended at once
        let mut p = Machine::default();
        p.frame(at(MENU, 1, 0, 1));
        assert_eq!(p.release(2), Some(Act::Stop(End::Left)));
        assert_eq!(p.release(3), None);
    }

    /// (TITLE-AUDIO, the coordinator: "Music must not depend on the Sound Effects setting") each category follows its own row:
    /// the music Music, the voices Voice, every SE (the menus' too) Sound Effects; 7 = as authored
    #[test]
    fn ta_the_music_row_sets_the_level() {
        use crate::eroptions::SoundRows;
        let rows = SoundRows { master: 10, music: 7, sfx: 0, voice: 7 };
        assert!((crate::fx::category_level("BGM", rows) - 1.0).abs() < 1e-6);
        assert!((crate::fx::category_level("BGM_LSI", rows) - 1.0).abs() < 1e-6);
        for se in ["SE", "SE_system", "SE_system2", "SE_weapon", "SE_footstep"] {
            assert_eq!(crate::fx::category_level(se, rows), 0.0, "{se}");
        }
        for voice in ["VOICE", "Vo_field", "Vo_BattlePC"] {
            assert!((crate::fx::category_level(voice, rows) - 1.0).abs() < 1e-6, "{voice}");
        }
        let music0 = SoundRows { master: 10, music: 0, sfx: 7, voice: 0 };
        assert_eq!(crate::fx::category_level("BGM", music0), 0.0);
        assert_eq!(crate::fx::category_level("Vo_event", music0), 0.0);
        assert!((crate::fx::category_level("SE_system", music0) - 1.0).abs() < 1e-6);
        assert!(crate::fx::category_level("BGM", SoundRows { music: 10, ..rows }) > crate::fx::category_level("BGM", SoundRows { music: 5, ..rows }));
    }

    /// (TITLE-AUDIO) THE ENGINE ITSELF with Sound Effects 0 and Music 7: KH3's title music starts AUDIBLE (its voice's volume
    /// is the asset's 0.7) and KH3's menu Decide (static SE 1) starts SILENT, from the real pack + the staged title root
    #[test]
    fn ta_sound_effects_zero_leaves_the_music_audible() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (main, stage) = (root.join("build/sora_pack/audio"), root.join("build/sora_tmp/weapons/TITLE-AUDIO/stage/title/audio"));
        let (Ok(mut pack), true) = (kh_audio::Pack::load(&main), stage.join("manifest.json").exists()) else {
            println!("no pack / no staged title root: skipped");
            return;
        };
        assert_eq!(pack.merge_party_root(&stage), Ok(true));
        let pack = std::sync::Arc::new(pack);
        let mut engine = kh_audio::Engine::new(pack.clone(), 1, kh_audio::Language::En);
        crate::fx::set_category_levels(&mut engine.driver, crate::eroptions::SoundRows { master: 10, music: 7, sfx: 0, voice: 7 });
        let bgm = engine.play_asset(TITLE_BGM_ASSET, "test music", kh_audio::AssetPlay::at(None, None)).expect("the title music starts");
        let se = engine.play_static_se(KhSe::Decide.static_index()).expect("KH3's Decide starts");
        let mut volumes = std::collections::HashMap::new();
        for _ in 0..3 {
            for command in engine.update(1.0 / 60.0, &kh_audio::Listener::default(), &|_| None) {
                if let kh_audio::VoiceCommand::Start { sound, params, .. } = command {
                    volumes.insert(sound, params.volume);
                }
            }
        }
        let music = volumes.get(&bgm).copied().expect("the music's voice started");
        assert!((music - 0.7).abs() < 1e-3, "music voice volume {music}");
        assert_eq!(volumes.get(&se).copied().unwrap_or(0.0), 0.0, "the SE is silent with Sound Effects 0");
        // the same SE with Sound Effects 7 (and Music 0): audible, the music silent
        let mut engine = kh_audio::Engine::new(pack, 1, kh_audio::Language::En);
        crate::fx::set_category_levels(&mut engine.driver, crate::eroptions::SoundRows { master: 10, music: 0, sfx: 7, voice: 7 });
        let bgm = engine.play_asset(TITLE_BGM_ASSET, "test music", kh_audio::AssetPlay::at(None, None)).expect("the title music starts");
        let se = engine.play_static_se(KhSe::Decide.static_index()).expect("KH3's Decide starts");
        let mut volumes = std::collections::HashMap::new();
        for _ in 0..3 {
            for command in engine.update(1.0 / 60.0, &kh_audio::Listener::default(), &|_| None) {
                if let kh_audio::VoiceCommand::Start { sound, params, .. } = command {
                    volumes.insert(sound, params.volume);
                }
            }
        }
        assert!(volumes.get(&se).copied().unwrap_or(0.0) > 0.0, "the SE is audible with Sound Effects 7: {volumes:?}");
        assert_eq!(volumes.get(&bgm).copied().unwrap_or(0.0), 0.0, "the music is silent with Music 0");
    }

    /// (TITLE-AUDIO) every sound Elden Ring's title was seen asking for resolves to one of KH3's menu sounds, and each KH3 sound
    /// is a static SE of the pack's table with the HUD's own name
    #[test]
    fn ta_every_er_title_sound_maps_to_kh3() {
        // every sound the game asked for on the title in the runs (TITLE-AUDIO/run2.kh-er.log, run3.kh-er.log)
        let seen = [(2, 999_999_999, KhSe::Decide), (2, 0, KhSe::Cursor), (2, 100, KhSe::Decide), (2, 1000, KhSe::Decide), (2, 1010, KhSe::Open), (2, 1020, KhSe::Cancel), (2, 200, KhSe::Cancel)];
        for (kind, id, kh) in seen {
            assert_eq!(kh_for(kind, id).map(|r| r.kh), Some(kh), "{}{id:09}", letter(kind));
        }
        assert!(kh_for(1, 0).is_none(), "a character's sound is not a menu sound");
        for row in SE_MAP {
            assert_eq!(kh_for(row.kind, row.id).map(|r| r.kh), Some(row.kh), "{}{:09}", letter(row.kind), row.id);
        }
        let mut ids: Vec<(i32, i32)> = SE_MAP.iter().map(|r| (r.kind, r.id)).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), SE_MAP.len(), "one KH3 sound per Elden Ring sound");
        let sounds = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/ui/data/ui_sounds.json"));
        if let Ok(text) = sounds {
            let json: serde_json::Value = serde_json::from_str(&text).unwrap();
            for se in [KhSe::Cursor, KhSe::Decide, KhSe::Cancel, KhSe::Open] {
                let row = &json["static_se"][se.static_index() as usize]["sound"];
                assert!(row.as_str().is_some_and(|s| s.ends_with(se.name())), "{se:?}: {row}");
            }
            assert_eq!(json["system_se"]["Click"], 0);
            assert_eq!(json["system_se"]["Decide"], 1);
            assert_eq!(json["system_se"]["Cancel"], 3);
            assert_eq!(json["system_se"]["Beep"], 4);
        }
        assert_eq!(letter(2), 'f');
        assert_eq!(letter(6), 'm');
    }

    /// (TITLE-AUDIO) the asset: the title HUD's music, packed by tools/title_audio_pack.js into the extra root, looped at KH3's
    /// loop points, merged into the main pack and resolved by the engine
    #[test]
    fn ta_the_title_music_resolves_in_the_pack() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let stage = root.join("build/sora_tmp/weapons/TITLE-AUDIO/stage/title/audio");
        let Ok(text) = std::fs::read_to_string(stage.join("manifest.json")) else {
            println!("no staged title audio root: skipped");
            return;
        };
        assert!(text.contains(TITLE_BGM_ASSET));
        let extra = kh_audio::Pack::load(&stage).unwrap();
        let asset = extra.asset(TITLE_BGM_ASSET).expect("the title music asset");
        assert!(asset.looping);
        assert!((asset.volume.unwrap_or(0.0) - 0.7).abs() < 1e-6, "SEAD music Volume 0.7");
        let material = &extra.bank(asset.bank).materials[&0];
        assert_eq!(material.loop_points, Some((0, 7_377_760)));
        assert_eq!(material.samples, 7_377_760);
        assert_eq!(material.channels, 2);
        assert!((f64::from(material.samples) / f64::from(material.sample_rate) - TITLE_BGM_SECONDS).abs() < 1e-6);
        let sound = extra.sound(asset.bank, asset.index).unwrap();
        assert_eq!(sound.category, 1, "KH3's BGM category");
        // merged as fx.rs does (PACK/title/audio) into a pack: the asset is there with its material path under the root
        let mut main = kh_audio::Pack::from_json(&kh_anim::json::Json::parse(r#"{"format":"kh3-sora-audio-pack/1","banks":{},"sound_assets":{},"triggers":[]}"#).unwrap(), None).unwrap();
        assert_eq!(main.merge_party_root(&stage), Ok(true));
        let merged = main.asset(TITLE_BGM_ASSET).unwrap();
        let path = main.material_path(merged.bank, 0).unwrap();
        assert!(path.exists(), "{}", path.display());
    }

    /// (TITLE-AUDIO) `title.custom_loop` and the loop rule of the user's own track
    #[test]
    fn ta_the_custom_loop_setting_and_its_fallbacks() {
        assert_eq!(parse_loop("12.5, 140"), Some((12.5, 140.0)));
        assert_eq!(parse_loop("3"), Some((3.0, 0.0)));
        assert_eq!(parse_loop("x,1"), None);
        assert_eq!(parse_loop("-1,4"), None);
        // the setting (s -> frames at 48 kHz); end 0 = the file's end; a setting past / before the start falls back
        assert_eq!(loop_frames(Some((1.0, 2.0)), Some((5, 9)), 480_000, 48_000), (48_000, 96_000));
        assert_eq!(loop_frames(Some((1.0, 0.0)), None, 480_000, 48_000), (48_000, 480_000));
        assert_eq!(loop_frames(Some((9.0, 2.0)), Some((5, 9)), 480_000, 48_000), (5, 9), "an empty setting: the WAV's smpl loop");
        assert_eq!(loop_frames(None, Some((5, 9)), 480_000, 48_000), (5, 9));
        assert_eq!(loop_frames(None, Some((5, 999_999)), 480_000, 48_000), (0, 480_000), "a smpl loop past the end: the whole file");
        assert_eq!(loop_frames(None, None, 480_000, 44_100), (0, 480_000));
        // the loop log: KH3's (first end = period) and a track with an intro (first end 10 s, then every 8 s)
        assert_eq!(loop_passes(150.0, TITLE_BGM_SECONDS, TITLE_BGM_SECONDS), 0);
        assert_eq!(loop_passes(155.0, TITLE_BGM_SECONDS, TITLE_BGM_SECONDS), 1);
        assert_eq!(loop_passes(10.5, 10.0, 8.0), 0);
        assert_eq!(loop_passes(11.5, 10.0, 8.0), 1);
        assert_eq!(loop_passes(19.5, 10.0, 8.0), 2);
    }

    /// (TITLE-AUDIO) THE USER'S OWN TRACK: with `custom/title.wav` in the title root it is picked up, merged as the extra root
    /// fx.rs merges, resolved with its loop and plays in the engine as the title music; without it there is nothing to merge
    /// (KH3's track stays the one). A small test WAV in a scratch pack folder (no real song).
    #[test]
    fn ta_the_users_own_track_is_picked_up_and_the_fallback_is_kh3s() {
        let pack = std::env::temp_dir().join(format!("kh_er_ta_custom_{}", std::process::id()));
        let title = pack.join("title").join("audio");
        std::fs::create_dir_all(title.join("custom")).unwrap();
        // the fallback first: no file, nothing to merge
        let _ = std::fs::remove_file(pack.join(CUSTOM_FILE));
        assert_eq!(custom_root(&pack), Ok(None));
        // an unusable file: refused with the reason (KH3's plays)
        std::fs::write(pack.join(CUSTOM_FILE), b"not a wav").unwrap();
        assert!(custom_root(&pack).is_err());
        // a 2 s stereo 44.1 kHz tone with a smpl loop [22050, 66150)
        let frames = 88_200usize;
        let mut samples = Vec::with_capacity(frames * 2);
        for i in 0..frames {
            let v = (i as f32 * 0.05).sin() * 0.2;
            samples.extend([v, v]);
        }
        let mut bytes = kh_audio::wav::encode(2, 44_100, &samples);
        // a smpl chunk appended (the RIFF size grown): one forward loop, end inclusive
        let mut smpl = vec![0u8; 8 + 60];
        smpl[..4].copy_from_slice(b"smpl");
        smpl[4..8].copy_from_slice(&60u32.to_le_bytes());
        smpl[8 + 28..8 + 32].copy_from_slice(&1u32.to_le_bytes());
        smpl[8 + 44..8 + 48].copy_from_slice(&22_050u32.to_le_bytes());
        smpl[8 + 48..8 + 52].copy_from_slice(&66_149u32.to_le_bytes());
        bytes.extend_from_slice(&smpl);
        let riff = (bytes.len() - 8) as u32;
        bytes[4..8].copy_from_slice(&riff.to_le_bytes());
        std::fs::write(pack.join(CUSTOM_FILE), &bytes).unwrap();
        let (manifest, words) = custom_root(&pack).unwrap().expect("the user's track is picked up");
        assert!(words.contains("44100 Hz") && words.contains("smpl"), "{words}");
        let mut merged = kh_audio::Pack::from_json(&kh_anim::json::Json::parse(r#"{"format":"kh3-sora-audio-pack/1","banks":{},"sound_assets":{},"triggers":[]}"#).unwrap(), None).unwrap();
        assert_eq!(merged.merge_root_text(&title, &manifest), Ok(true));
        let asset = merged.asset(CUSTOM_ASSET).expect("the custom asset").clone();
        assert!(asset.looping);
        let material = &merged.bank(asset.bank).materials[&0];
        assert_eq!((material.channels, material.sample_rate, material.samples, material.loop_points), (2, 44_100, 88_200, Some((22_050, 66_150))));
        assert_eq!(merged.material_path(asset.bank, 0).as_deref(), Some(pack.join(CUSTOM_FILE).as_path()));
        // it plays as the title music (BGM category, looped voice)
        let merged = std::sync::Arc::new(merged);
        let mut engine = kh_audio::Engine::new(merged, 1, kh_audio::Language::En);
        let key = engine.play_asset(CUSTOM_ASSET, "test custom", kh_audio::AssetPlay::at(None, None)).expect("it starts");
        let started = engine.update(1.0 / 60.0, &kh_audio::Listener::default(), &|_| None).into_iter().any(|c| matches!(c, kh_audio::VoiceCommand::Start { sound, looped: true, .. } if sound == key));
        assert!(started, "a looped voice of the user's track");
        // the asset the title plays follows the loader's choice
        set_title_track(true, 1.5, 1.0);
        assert_eq!(title_asset(), CUSTOM_ASSET);
        set_title_track(false, TITLE_BGM_SECONDS, TITLE_BGM_SECONDS);
        assert_eq!(title_asset(), TITLE_BGM_ASSET);
        let _ = std::fs::remove_dir_all(&pack);
    }
}
